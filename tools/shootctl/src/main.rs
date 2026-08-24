//! shootctl -- the render pipeline's control tool.
//!
//! Rust, std only, no dependencies: it builds anywhere the render box can build,
//! including the WSL side, with no vendoring and no network.
//!
//! It replaces the shell and Python that had accumulated around the renderer:
//!
//!   shootctl lint  <api.json> <plugin.as ...>   check AngelScript before install
//!   shootctl stamp <Main.as> <STAMP>            write a build stamp into a route
//!   shootctl get   <route>                      one call to the plugin
//!   shootctl wait  --ctx N [--timeout S]        wait for a STATE, never a sleep
//!   shootctl drive --map <path>                 map -> editor -> MediaTracker
//!
//! Two rules this tool exists to enforce:
//!   * nothing is driven by screen coordinates;
//!   * nothing waits a fixed number of seconds -- every wait is a condition on
//!     the game's own object graph, with a timeout that reports what it saw.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

mod host;
use host::plugin_addrs;
mod lock;

use std::sync::OnceLock;
static ADDR: OnceLock<String> = OnceLock::new();

/// The first candidate address that accepts a connection, remembered for the
/// rest of the run.
///
/// CACHED ONLY ON SUCCESS. The first version fell back to "127.0.0.1:29800"
/// when nothing answered AND cached that -- so a `run` that probed while the
/// game was still starting locked itself to the WSL loopback (a different
/// machine from the game's) and then dialled it for three minutes while the
/// plugin sat there answering on the real address. Exactly the trap host.rs
/// documents, wearing a different hat: never remember a guess.
fn plugin_addr() -> String {
    if let Some(a) = ADDR.get() { return a.clone(); }
    for a in plugin_addrs() {
        let Ok(sa) = a.parse::<SocketAddr>() else { continue };
        if TcpStream::connect_timeout(&sa, Duration::from_millis(400)).is_ok() {
            let _ = ADDR.set(a.clone());
            return a;
        }
    }
    "127.0.0.1:29800".to_string()
}

/// Is the plugin answering anywhere? Tries every candidate, caches on success.
/// This is the launch's gate -- it must not depend on an address chosen before
/// the server existed.
fn plugin_up() -> bool {
    if let Some(a) = ADDR.get() {
        if let Ok(sa) = a.parse::<SocketAddr>() {
            return TcpStream::connect_timeout(&sa, Duration::from_millis(500)).is_ok();
        }
    }
    for a in plugin_addrs() {
        let Ok(sa) = a.parse::<SocketAddr>() else { continue };
        if TcpStream::connect_timeout(&sa, Duration::from_millis(500)).is_ok() {
            let _ = ADDR.set(a);
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// the plugin's HTTP interface
// ---------------------------------------------------------------------------

fn http_get(route: &str, timeout_s: u64) -> Result<String, String> {
    let mut s = TcpStream::connect(plugin_addr().as_str()).map_err(|e| format!("connect: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(timeout_s))).ok();
    s.set_write_timeout(Some(Duration::from_secs(timeout_s))).ok();
    let req = format!("GET {route} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).map_err(|e| format!("read: {e}"))?;
    let text = String::from_utf8_lossy(&buf).to_string();
    match text.find("\r\n\r\n") {
        Some(i) => Ok(text[i + 4..].to_string()),
        None => Ok(text),
    }
}

/// A map path the GAME can resolve, or a refusal.
///
/// `EditMap` is handed this string and does not validate it: given
/// `/mnt/c/Users/...` -- the WSL spelling of a path that is perfectly real from
/// this side of the bridge -- the plugin answers `ok`, the title API reports
/// `IsReady: true` with no dialog, the client keeps rendering, and `ctx` sits
/// at 0 until the wait times out. That is indistinguishable from a map the game
/// cannot load, and it cost most of an evening: four "hangs" were read as
/// evidence about three map files and a title-API race, and every one of them
/// was this spelling. The successful loads in the same session's logs all read
/// `C:/Users/...`.
///
/// So the conversion happens here, once, and anything still unresolvable is
/// REFUSED rather than handed over -- a wiring error must not be able to come
/// back as a fact about a map.
fn game_path(p: &str) -> Result<String, String> {
    // /mnt/<drive>/rest  ->  <DRIVE>:/rest
    if let Some(rest) = p.strip_prefix("/mnt/") {
        let mut it = rest.splitn(2, '/');
        if let (Some(d), Some(tail)) = (it.next(), it.next()) {
            if d.len() == 1 && d.chars().next().unwrap().is_ascii_alphabetic() {
                return Ok(format!("{}:/{}", d.to_ascii_uppercase(), tail));
            }
        }
        return Err(format!("{p}: looks like a WSL path but names no drive"));
    }
    // Already a Windows path: C:/... or C:\...
    let b = p.as_bytes();
    if b.len() >= 3 && (b[0] as char).is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\') {
        return Ok(p.to_string());
    }
    Err(format!(
        "{p}: not a path the game can resolve. EditMap accepts anything and \
         silently loads nothing, so this is refused here. Give a Windows path \
         (C:/Users/...) or a WSL path under /mnt/<drive>/."
    ))
}

/// The one number that says where the game is: 0 menu, 1 track editor,
/// 2 MediaTracker, 3 in a race.
fn ctx() -> Option<i64> {    let body = http_get("/ctx", 5).ok()?;
    let key = "\"ctx\":";
    let i = body.find(key)? + key.len();
    let rest = &body[i..];
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// WAIT FOR A CONDITION -- and do not poll for it.
///
/// `/await` is a long poll: HttpServer runs the request handler inside a
/// coroutine, so the plugin yields frame by frame until the condition holds and
/// only then answers. This blocks on the socket. Nothing sleeps, the answer
/// arrives on the frame the thing actually happened, and the reply carries how
/// many milliseconds and frames it took.
///
/// Conditions (colon, never '=' -- the plugin's query splitter does no URL
/// decoding, so "ctx%3D0" would arrive literally and time out looking correct):
///   ctx:N  ready  nodialog  shootdlg  noshootdlg  ghosts:N  tracks:N
fn await_cond(cond: &str, timeout_s: u64) -> Result<f64, String> {
    let ms = timeout_s * 1000;
    let body = http_get(&format!("/await?c={cond}&ms={ms}"), timeout_s + 15)?;
    if body.contains("\"ok\":true") {
        let key = "\"ms\":";
        let took = body.find(key)
            .and_then(|i| {
                let r = &body[i + key.len()..];
                let e = r.find(|c: char| !c.is_ascii_digit())?;
                r[..e].parse::<f64>().ok()
            })
            .unwrap_or(0.0);
        return Ok(took / 1000.0);
    }
    Err(format!("waiting for {cond}: {}", body.trim()))
}

fn wait_ctx(want: i64, timeout_s: u64) -> Result<f64, String> {
    await_cond(&format!("ctx:{want}"), timeout_s)
}

// ---------------------------------------------------------------------------
// a minimal JSON reader -- enough for Openplanet's class dump
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, k: &str) -> Option<&J> {
        if let J::Obj(m) = self {
            m.iter().find(|(kk, _)| kk == k).map(|(_, v)| v)
        } else {
            None
        }
    }
    fn as_str(&self) -> Option<&str> {
        if let J::Str(s) = self { Some(s) } else { None }
    }
    fn as_arr(&self) -> Option<&Vec<J>> {
        if let J::Arr(a) = self { Some(a) } else { None }
    }
    fn truthy(&self) -> bool {
        match self {
            J::Bool(b) => *b,
            J::Num(n) => *n != 0.0,
            J::Null => false,
            _ => true,
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn val(&mut self) -> Result<J, String> {
        self.ws();
        if self.i >= self.b.len() {
            return Err("eof".into());
        }
        match self.b[self.i] {
            b'{' => self.obj(),
            b'[' => self.arr(),
            b'"' => Ok(J::Str(self.string()?)),
            b't' => { self.i += 4; Ok(J::Bool(true)) }
            b'f' => { self.i += 5; Ok(J::Bool(false)) }
            b'n' => { self.i += 4; Ok(J::Null) }
            _ => self.num(),
        }
    }
    fn obj(&mut self) -> Result<J, String> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            self.ws();
            if self.i < self.b.len() && self.b[self.i] == b'}' {
                self.i += 1;
                return Ok(J::Obj(out));
            }
            let k = self.string()?;
            self.ws();
            if self.i >= self.b.len() || self.b[self.i] != b':' {
                return Err("expected :".into());
            }
            self.i += 1;
            let v = self.val()?;
            out.push((k, v));
            self.ws();
            if self.i < self.b.len() && self.b[self.i] == b',' {
                self.i += 1;
            }
        }
    }
    fn arr(&mut self) -> Result<J, String> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            self.ws();
            if self.i < self.b.len() && self.b[self.i] == b']' {
                self.i += 1;
                return Ok(J::Arr(out));
            }
            out.push(self.val()?);
            self.ws();
            if self.i < self.b.len() && self.b[self.i] == b',' {
                self.i += 1;
            }
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.ws();
        if self.i >= self.b.len() || self.b[self.i] != b'"' {
            return Err("expected string".into());
        }
        self.i += 1;
        let mut s = String::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return Ok(s),
                b'\\' => {
                    let e = self.b[self.i];
                    self.i += 1;
                    s.push(match e {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'u' => {
                            let h = std::str::from_utf8(&self.b[self.i..self.i + 4]).unwrap_or("0000");
                            self.i += 4;
                            char::from_u32(u32::from_str_radix(h, 16).unwrap_or(63)).unwrap_or('?')
                        }
                        other => other as char,
                    });
                }
                _ => s.push(c as char),
            }
        }
        Err("unterminated string".into())
    }
    fn num(&mut self) -> Result<J, String> {
        let st = self.i;
        while self.i < self.b.len()
            && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        {
            self.i += 1;
        }
        std::str::from_utf8(&self.b[st..self.i])
            .unwrap_or("0")
            .parse()
            .map(J::Num)
            .map_err(|e| format!("num: {e}"))
    }
}

// ---------------------------------------------------------------------------
// the linter
// ---------------------------------------------------------------------------

/// Openplanet script-side types the game's class dump does not carry.
///
/// EVIDENCE, NOT GUESSWORK: every name here appears in plugin source the game
/// has actually compiled. Adding one on a hunch re-opens the hole this linter
/// exists to close -- `CControlList` looked exactly as plausible as
/// `CControlBase`, and does not exist.
const SCRIPT_TYPES: &[&str] = &[
    "CMwNod",
    "CControlBase",
    "CControlContainer",
    "CGameCtnApp",
    "CTrackMania",
    "CGameManiaPlanet",
];

struct Api {
    classes: HashSet<String>,
    /// class -> member -> is_const (const means "no set accessor")
    members: HashMap<String, HashMap<String, bool>>,
    /// class -> the enum type names it actually declares.
    ///
    /// A member's enum is often called "UnnamedEnum" in the dump, which means
    /// AngelScript has NO name for it: `CGameDialogShootParams::EExtVideo(1)`
    /// looks obvious, compiles nowhere, and costs a game restart.
    enums: HashMap<String, HashSet<String>>,
}

fn load_api(path: &str) -> Result<Api, String> {
    let raw = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut p = P { b: &raw, i: 0 };
    let v = p.val()?;
    let ns = v.get("ns").ok_or("no ns")?;
    let mut classes = HashSet::new();
    let mut own: HashMap<String, HashMap<String, bool>> = HashMap::new();
    let mut enums: HashMap<String, HashSet<String>> = HashMap::new();
    let mut parent: HashMap<String, String> = HashMap::new();
    if let J::Obj(namespaces) = ns {
        for (_nsname, cs) in namespaces {
            if let J::Obj(list) = cs {
                for (cname, info) in list {
                    classes.insert(cname.clone());
                    if let Some(p) = info.get("p").and_then(|x| x.as_str()) {
                        parent.insert(cname.clone(), p.to_string());
                    }
                    // The enum type names this class really declares. "e" at
                    // class level is the named list; a member's own "e" is
                    // usually "UnnamedEnum" and is deliberately NOT collected.
                    let mut es = HashSet::new();
                    if let Some(list) = info.get("e").and_then(|x| x.as_arr()) {
                        for e in list {
                            if let Some(n) = e.get("n").and_then(|x| x.as_str()) {
                                if n != "UnnamedEnum" { es.insert(n.to_string()); }
                            }
                        }
                    }
                    enums.insert(cname.clone(), es);
                    let mut m = HashMap::new();
                    if let Some(ms) = info.get("m").and_then(|x| x.as_arr()) {
                        for mem in ms {
                            if let Some(n) = mem.get("n").and_then(|x| x.as_str()) {
                                let is_const = mem.get("c").map(|c| c.truthy()).unwrap_or(false);
                                m.insert(n.to_string(), is_const);
                            }
                        }
                    }
                    own.insert(cname.clone(), m);
                }
            }
        }
    }
    // fold in inherited members AND inherited enums
    let mut members = HashMap::new();
    let mut all_enums: HashMap<String, HashSet<String>> = HashMap::new();
    for c in classes.iter() {
        let mut acc: HashMap<String, bool> = HashMap::new();
        let mut eacc: HashSet<String> = HashSet::new();
        let mut cur = Some(c.clone());
        let mut guard = 0;
        while let Some(name) = cur {
            if guard > 32 {
                break;
            }
            guard += 1;
            if let Some(m) = own.get(&name) {
                for (k, v) in m {
                    acc.entry(k.clone()).or_insert(*v);
                }
            }
            if let Some(e) = enums.get(&name) {
                for k in e { eacc.insert(k.clone()); }
            }
            cur = parent.get(&name).cloned();
        }
        members.insert(c.clone(), acc);
        all_enums.insert(c.clone(), eacc);
    }
    Ok(Api { classes, members, enums: all_enums })
}

fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Every `CFoo` that looks like a type reference on this line.
fn type_refs(code: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let b: Vec<char> = code.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == 'C' && (i == 0 || !b[i - 1].is_alphanumeric() && b[i - 1] != '_') {
            let st = i;
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                i += 1;
            }
            let word: String = b[st..i].iter().collect();
            if word.len() > 4 {
                // a cast<> or a declaration is a type use; anything else may be a
                // string or a comment word, so only those two shapes are checked.
                let before: String = b[..st].iter().collect();
                let after: String = b[i..].iter().collect();
                let is_cast = before.trim_end().ends_with("cast<");
                let is_decl = after.trim_start().starts_with('@');
                if is_cast || is_decl {
                    out.push((word, is_decl));
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

fn lint(api_path: &str, files: &[String]) -> i32 {
    let api = match load_api(api_path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("aslint: {e}");
            return 2;
        }
    };
    let known: HashSet<&str> = api
        .classes
        .iter()
        .map(|s| s.as_str())
        .chain(SCRIPT_TYPES.iter().copied())
        .collect();

    let mut problems: Vec<String> = Vec::new();
    let mut defined: HashSet<String> = HashSet::new();
    let mut sources: Vec<(String, String)> = Vec::new();

    for f in files {
        let src = match std::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                problems.push(format!("{f}: {e}"));
                continue;
            }
        };
        for line in src.lines() {
            let code = strip_comment(line).trim_start();
            for kw in ["string ", "void ", "bool ", "int ", "uint ", "uint16 "] {
                if let Some(rest) = code.strip_prefix(kw) {
                    if let Some(par) = rest.find('(') {
                        let name = rest[..par].trim();
                        if !name.is_empty()
                            && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            defined.insert(name.to_string());
                        }
                    }
                }
            }
        }
        sources.push((f.clone(), src));
    }

    for (f, src) in &sources {
        let base = f.rsplit('/').next().unwrap_or(f);
        let mut vartype: HashMap<String, String> = HashMap::new();
        for (n, line) in src.lines().enumerate() {
            let ln = n + 1;
            let code = strip_comment(line);

            for (t, is_decl) in type_refs(code) {
                if !known.contains(t.as_str()) {
                    problems.push(format!("{base}:{ln}: unknown class: {t}"));
                    continue;
                }
                if is_decl {
                    // CFoo@ name = ...
                    if let Some(pos) = code.find(&format!("{t}@")) {
                        let rest = code[pos + t.len() + 1..].trim_start();
                        let name: String =
                            rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                        if !name.is_empty() {
                            vartype.insert(name, t.clone());
                        }
                    }
                }
            }
            // auto x = cast<CFoo>(
            if let Some(a) = code.find("auto ") {
                let rest = &code[a + 5..];
                let name: String =
                    rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if let Some(c) = rest.find("cast<") {
                    let t: String = rest[c + 5..].chars().take_while(|ch| *ch != '>').collect();
                    if !name.is_empty() && !t.is_empty() {
                        vartype.insert(name, t.trim().to_string());
                    }
                }
            }

            // assignment to a member of a variable whose class we know
            for (i, _) in code.match_indices('=') {
                if i == 0 || code.as_bytes()[i - 1] == b'=' || code.as_bytes()[i - 1] == b'!'
                    || code.as_bytes()[i - 1] == b'<' || code.as_bytes()[i - 1] == b'>'
                {
                    continue;
                }
                if code.as_bytes().get(i + 1) == Some(&b'=') {
                    continue;
                }
                let lhs = code[..i].trim_end();
                let dot = match lhs.rfind('.') {
                    Some(d) => d,
                    None => continue,
                };
                let member: String = lhs[dot + 1..].trim().to_string();
                let vstart = lhs[..dot]
                    .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|x| x + 1)
                    .unwrap_or(0);
                let var = lhs[vstart..dot].trim();
                if member.is_empty() || !member.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                if let Some(t) = vartype.get(var) {
                    if let Some(ms) = api.members.get(t) {
                        match ms.get(&member) {
                            None => problems
                                .push(format!("{base}:{ln}: {t} has no member {member}")),
                            Some(true) => problems.push(format!(
                                "{base}:{ln}: {t}.{member} is const -- 'the property has no set \
                                 accessor'. Write it with Dev::SetOffset and an offset looked up \
                                 by name."
                            )),
                            Some(false) => {}
                        }
                    }
                }
            }

            // A MEMBER READ THAT DOES NOT EXIST. The const-write check only
            // covered assignments; `mt.EditorInterface` on a class that has no
            // such member is the same mistake on the read side, and it cost a
            // restart. Only checked for variables whose class we know, and only
            // when that class has members in the dump (an empty member list
            // means the dump has nothing to say, not that the member is absent).
            for (i, _) in code.match_indices('.') {
                let before = &code[..i];
                let vstart = before
                    .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|x| x + 1)
                    .unwrap_or(0);
                let var = before[vstart..].trim();
                let after = &code[i + 1..];
                let member: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if var.is_empty() || member.is_empty() { continue; }
                // a call, not a field read -- methods are not in the dump the
                // same way, so leave them to the compiler
                if after[member.len()..].trim_start().starts_with('(') { continue; }
                if let Some(t) = vartype.get(var) {
                    if let Some(ms) = api.members.get(t) {
                        if !ms.is_empty() && !ms.contains_key(&member) {
                            problems.push(format!(
                                "{base}:{ln}: {t} has no member {member}"
                            ));
                        }
                    }
                }
            }

            // A RESERVED WORD USED AS A VARIABLE NAME.
            //
            // `array<CGameMenu@> out;` compiles nowhere: `out` is a parameter
            // modifier in AngelScript, and the compiler's message ("Expected
            // expression value / Instead found reserved keyword 'out'") points
            // at the USE, several lines from the declaration. It cost a game
            // restart, which is the entire reason this linter exists.
            //
            // Only words that plausibly get typed as a variable name are
            // checked, and only in DECLARATION position (a type token straight
            // in front). The first cut of this rule listed `null` and `return`
            // too and fired on eleven correct lines -- a linter that cries wolf
            // is worse than none.
            for kw in ["out", "in", "inout", "cast", "class", "interface",
                       "namespace", "funcdef", "mixin", "shared", "enum"] {
                for tail in [";", " =", ")", ","] {
                    let pat = format!(" {kw}{tail}");
                    let Some(at) = code.find(&pat) else { continue };
                    // What is in front must be a TYPE: `>` or `@` from a
                    // template/handle, or a bare identifier that is itself not
                    // a keyword (so `return out;` and `case in:` do not match).
                    let before = code[..at].trim_end();
                    let ends_type = before.ends_with('>') || before.ends_with('@');
                    let word: String = before.chars().rev()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect::<Vec<_>>().into_iter().rev().collect();
                    let ident_type = !word.is_empty()
                        && !["return", "case", "is", "and", "or", "not", "if",
                             "while", "for", "else", "const"].contains(&word.as_str());
                    if ends_type || ident_type {
                        problems.push(format!(
                            "{base}:{ln}: '{kw}' is a reserved word and cannot be a variable name"
                        ));
                        break;
                    }
                }
            }

            // A SCOPED NAME THE CLASS DOES NOT DECLARE.
            //
            // `CGameDialogShootParams::EExtVideo(ext)` reads like the obvious
            // way to build that enum value. There is no such symbol: the dump
            // calls the enum "UnnamedEnum", which means AngelScript has no name
            // for it at all. Cost a game restart. Members are checked too, so
            // `CFoo::Bar` typos are caught the same way.
            {
                let mut rest = code;
                while let Some(p) = rest.find("::") {
                    let before = &rest[..p];
                    let cls: String = before.chars().rev()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect::<Vec<_>>().into_iter().rev().collect();
                    let after = &rest[p + 2..];
                    let sym: String = after.chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    rest = after;
                    if cls.is_empty() || sym.is_empty() { continue; }
                    // only classes we know about; namespaces (Dev::, IO::,
                    // Reflection::, Text::, Meta::) are not in the dump
                    if !known.contains(cls.as_str()) { continue; }
                    let has_enum = api.enums.get(&cls).map(|e| e.contains(&sym)).unwrap_or(false);
                    let has_member = api.members.get(&cls)
                        .map(|m| m.is_empty() || m.contains_key(&sym)).unwrap_or(true);
                    if !has_enum && !has_member {
                        let known_enums: Vec<&str> = api.enums.get(&cls)
                            .map(|e| e.iter().map(|s| s.as_str()).collect())
                            .unwrap_or_default();
                        let hint = if known_enums.is_empty() {
                            " (it declares no named enum -- the dump calls it UnnamedEnum)".to_string()
                        } else {
                            format!(" (it declares: {})", known_enums.join(", "))
                        };
                        problems.push(format!("{base}:{ln}: {cls} has no {sym}{hint}"));
                    }
                }
            }

            // a route calling a helper nobody defined -- routes and helpers live
            // in different files, so only a whole-plugin check can see this.
            if code.contains("HttpResponse(") {
                let mut rest = code;
                while let Some(p) = rest.find("HttpResponse(") {
                    let after = &rest[p + 13..];
                    if let Some(comma) = after.find(',') {
                        let arg = after[comma + 1..].trim_start();
                        let name: String = arg
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        let is_call = arg[name.len()..].trim_start().starts_with('(');
                        if is_call && !name.is_empty() && !defined.contains(&name) {
                            problems.push(format!(
                                "{base}:{ln}: route calls undefined function: {name}()"
                            ));
                        }
                    }
                    rest = &rest[p + 13..];
                }
            }
        }
    }

    for p in &problems {
        println!("{p}");
    }
    println!("aslint: {} problem(s)", problems.len());
    if problems.is_empty() { 0 } else { 1 }
}

// ---------------------------------------------------------------------------
// commands
// ---------------------------------------------------------------------------

fn stamp(main_as: &str, stamp: &str) -> i32 {
    let src = match std::fs::read_to_string(main_as) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{main_as}: {e}");
            return 2;
        }
    };
    let anchor = "    if (r == \"/ping\") return HttpResponse(200, \"pong\");";
    let mut out = String::new();
    for line in src.lines() {
        if line.trim_start().starts_with("if (r == \"/build\")") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if line == anchor {
            out.push_str(&format!(
                "    if (r == \"/build\") return HttpResponse(200, \"{stamp}\");\n"
            ));
        }
    }
    if std::fs::write(main_as, out).is_err() {
        return 2;
    }
    println!("stamped {stamp}");
    0
}

/// The plugin dev loop, in one command: lint, install, reload, prove.
///
/// This was `gsdev`, a bash script with `sleep 0.5` in its reload poll and a
/// `sleep 4` before relaunching. Both are gone: the reload is proven by a build
/// stamp, and the wait for it is paced by the HTTP round trip.
///
/// WHY THERE IS A LINT STEP AT ALL: self-reload takes about a second. What costs
/// a 90-second game restart is a COMPILE ERROR -- Openplanet unloads the plugin,
/// the HTTP server goes with it, and there is no /reload left to call. So the
/// source is checked against the game's own class dump before the game sees it.
fn install(plugin_dir: &str, good_dir: &str, api: &str) -> i32 {
    let files: Vec<String> = match std::fs::read_dir(plugin_dir) {
        Ok(rd) => {
            let mut v: Vec<String> = rd.flatten()
                .map(|e| e.path().to_string_lossy().to_string())
                .filter(|p| p.ends_with(".as"))
                .collect();
            v.sort();
            v
        }
        Err(e) => { eprintln!("{plugin_dir}: {e}"); return 2; }
    };
    if files.is_empty() { eprintln!("no .as files in {plugin_dir}"); return 2; }

    if lint(api, &files) != 0 { eprintln!("REFUSING to install"); return 1; }

    let mark = format!("B{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    if stamp(&format!("{plugin_dir}/Main.as"), &mark) != 0 { return 2; }
    let _ = http_get("/reload", 20);

    // The reload is done when the plugin reports the stamp we just wrote.
    // Paced by the HTTP round trip; nothing sleeps.
    let t0 = Instant::now();
    while t0.elapsed().as_secs() < 15 {
        if http_get("/build", 5).map(|b| b.trim() == mark).unwrap_or(false) {
            println!("RELOADED ok ({mark})");
            return 0;
        }
    }

    eprintln!("RELOAD FAILED -- errors the linter did not catch:");
    if let Ok(log) = std::fs::read_to_string("/mnt/c/Users/vjeux/OpenplanetNext/Openplanet.log") {
        for line in log.lines().filter(|l| l.contains("ERR") && !l.contains("UltimateMedals"))
                       .rev().take(5).collect::<Vec<_>>().into_iter().rev() {
            eprintln!("  {}", line.trim());
        }
    }
    // Put the last good plugin back. A file that is NEW and broken has no
    // known-good counterpart, so it has to be deleted rather than overwritten --
    // otherwise it keeps failing the compile forever.
    for f in &files {
        let base = f.rsplit('/').next().unwrap_or(f);
        if !std::path::Path::new(&format!("{good_dir}/{base}")).exists() {
            let _ = std::fs::remove_file(f);
            eprintln!("removed new-and-broken {base}");
        }
    }
    if let Ok(rd) = std::fs::read_dir(good_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("as") { continue; }
            let base = p.file_name().unwrap().to_string_lossy().to_string();
            let _ = std::fs::copy(&p, format!("{plugin_dir}/{base}"));
        }
        eprintln!("restored the last good plugin");
    }
    // A compile error unloads the plugin, so the server may be gone with it.
    if http_get("/ping", 5).is_err() {
        eprintln!("the plugin server is down; restarting the game");
        launch(180, true);  // the plugin is gone, so a restart is the point
    }
    1
}

/// Save the current plugin as the known-good one to fall back to.
fn save_good(plugin_dir: &str, good_dir: &str) -> i32 {
    let _ = std::fs::create_dir_all(good_dir);
    if let Ok(rd) = std::fs::read_dir(good_dir) {
        for e in rd.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) == Some("as") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(plugin_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("as") { continue; }
            let base = p.file_name().unwrap().to_string_lossy().to_string();
            if std::fs::copy(&p, format!("{good_dir}/{base}")).is_ok() { n += 1; }
        }
    }
    println!("saved {n} files as the known-good plugin");
    0
}

/// `shootctl probe` — ask the game to load ONE map and record everything it
/// says while it does or does not.
///
/// # Why this exists
///
/// 146612 ("Spaghetti Nights 2") is loaded and re-simulated by the dedicated
/// server and has never opened in this client's editor: `EditMap` returns,
/// `ctx` stays 0, `/ping` keeps answering. Every diagnosis of it so far has
/// been made from those two facts alone, because nothing printed the rest of
/// what the game knows:
///
/// * `CGameManiaTitleControlScriptAPI::LatestResult` — the title API's own
///   error channel. `EditMap` returns `void`; this enum is the only thing it
///   can ever say about a map it declined.
/// * `CustomResultType` / `CustomResultData` — what a title script fills in
///   when IT is the one refusing.
/// * whether a modal is up (a map that raises a dialog nobody dismisses looks
///   exactly like a map that silently did nothing).
/// * `PlayMap` instead of `EditMap` (`--play`): the same title API, the same
///   path, the same file, a DIFFERENT loader. It separates "the editor rejects
///   this map" from "this client cannot load this map at all", which is the
///   distinction the whole investigation turns on.
///
/// Every line is a sample of the live game with the elapsed time on it, so a
/// map that fails instantly and one that works for 40 s and gives up are
/// distinguishable — they are not, from `ctx` alone.
///
/// A negative needs a positive control: run this on a map that DOES open, in
/// the same session, before believing anything it prints about one that does
/// not.
fn probe(map: &str, how: &str, mode: &str, timeout_s: u64) -> i32 {
    // `editreplay` names a REPLAY, and the game resolves it RELATIVE to the
    // Replays folder — the same rule the ghost import follows. A relative path
    // is exactly what `game_path` refuses (correctly, for a map), so this one
    // door hands the string over as written.
    let gp = if how == "editreplay" {
        map.to_string()
    } else {
        match game_path(map) {
            Ok(m) => m,
            Err(e) => { eprintln!("{e}"); return 2; }
        }
    };
    let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
    let _ = std::fs::create_dir_all(store);
    if std::fs::write(format!("{store}/editmap.txt"), &gp).is_err() {
        eprintln!("could not write editmap.txt");
        return 2;
    }
    let wsl = gp.strip_prefix("C:/").map(|r| format!("/mnt/c/{r}")).unwrap_or_else(|| gp.clone());
    let want = map_uid(&wsl);
    println!("probe {} [{}]", gp, how);
    println!("  file uid {}", want.clone().unwrap_or_else(|| "(unreadable)".into()));

    // Start from the menu and prove it, then prove the title will accept a
    // command. Both of these have been the real cause of a "map did not open".
    if let Err(e) = to_menu() { eprintln!("{e}"); return 1; }
    if let Err(e) = await_cond("ready", 60) { eprintln!("{e}"); return 1; }
    println!("  before: /ready {}", http_get("/ready", 10).unwrap_or_default().trim());

    // WHICH DOOR. Every one of these is the same title API, the same path and
    // the same file; they differ only in which loader the game runs. That is
    // the whole point: 146612 hangs forever in `edit` and is in a playground in
    // 6.2 s through `play`.
    let call = match how {
        "edit" => "/editmap".to_string(),
        "play" => format!("/playmap?mode={mode}"),
        "editmap2" => format!("/editmap2?dec={mode}"),
        "editghosts" => "/editghosts2".to_string(),
        "editmap3" => "/editmap3?adv=0".to_string(),
        "editmap3adv" => "/editmap3?adv=1".to_string(),
        "editreplay" => format!("/editreplay?kind={}", if mode.is_empty() { "shoot" } else { mode }),
        other => {
            eprintln!("probe: unknown --how `{other}` (edit | play | editmap2 | editghosts)");
            return 2;
        }
    };
    let t0 = Instant::now();
    println!("  call {call}: {}", http_get(&call, 30).unwrap_or_default().trim());

    // Sample the game once a second. Not a sleep dressed up as a wait: this is
    // a RECORDING, and the interesting case is the one where nothing ever
    // happens, which no /await condition can describe.
    let mut last = String::new();
    let mut settled = false;
    while t0.elapsed().as_secs() < timeout_s {
        std::thread::sleep(Duration::from_millis(1000));
        let c = http_get("/ctx", 10).unwrap_or_default();
        let ctx_now = ctx();
        let ready = http_get("/ready", 10).unwrap_or_default();
        let line = format!("{} | {}", c.trim(), ready.trim());
        if line != last {
            println!("  [{:5.1}s] {line}", t0.elapsed().as_secs_f64());
            last = line;
        }
        // Anywhere but the menu is a load that happened. Which context it is
        // says which door opened, and the line above has already printed it.
        if matches!(ctx_now, Some(n) if n != 0) { settled = true; break; }
    }
    println!("  [{:5.1}s] final /ctx   {}", t0.elapsed().as_secs_f64(),
             http_get("/ctx", 10).unwrap_or_default().trim());
    println!("           final /ready {}", http_get("/ready", 10).unwrap_or_default().trim());
    println!("           dialog text  {}", http_get("/dlgtext", 10).unwrap_or_default().trim());
    if settled {
        let have = loaded_uid();
        println!("           loaded uid   {}", have.clone().unwrap_or_else(|| "none".into()));
        match (&want, &have) {
            (Some(w), Some(h)) if w == h => println!("OPENED — and it is the map we asked for"),
            (Some(w), Some(h)) => println!("OPENED SOMETHING ELSE — asked {w}, got {h}"),
            _ => println!("OPENED — uid unavailable"),
        }
        0
    } else {
        println!("DID NOT OPEN in {timeout_s}s — ctx never left the menu");
        1
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "usage:\n  shootctl lint <api.json> <file.as ...>\n  shootctl stamp <Main.as> <STAMP>\
             \n  shootctl get <route>\n  shootctl wait --ctx N [--timeout S]\n  shootctl drive --map <path>\
             \n  shootctl launch [timeout_s]\
             \n  shootctl setup --map <map> [--cam N] <ghost...>\n        --cam: 2 External (default), 1 Internal, 6 Ext2, 3 Helico\
             \n  shootctl shoot [timeout_s] --name <out>\
             \n  shootctl run --map <map> --name <out> [--cam N] <ghost...>\
             \n  shootctl probe --map <map> [--how edit|play|editmap2|editghosts] [--timeout S]\
             \n        load ONE map and print everything the game says while it does\
             \n        or does not: ctx, IsReady, LatestResult, CustomResult, dialogs.\
             \n        --how picks WHICH DOOR into the same file: EditMap (the track\
             \n        editor), PlayMap, EditMap2 (--mode names the decoration), or\
             \n        EditGhosts. 146612 hangs forever in the first and is in a\
             \n        playground in 6.2 s through the second.\
             \n  shootctl lock acquire|release|status [--owner WHO] [--wait S] [--max-age S]
             \n        one game, one driver -- take this before setup/shoot"
        );
        std::process::exit(2);
    }
    let code = match args[0].as_str() {
        // ONE GAME, ONE DRIVER. See `lock.rs`: two concurrent renders do not
        // fail, they produce two plausible clips of which one is of the wrong
        // run. Every arm driving this box takes the lock first.
        "lock" => {
            let val = |k: &str| -> Option<String> {
                args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
            };
            let owner = val("--owner").unwrap_or_else(|| {
                std::env::var("SHOOTCTL_OWNER").unwrap_or_else(|_| format!("pid-{}", std::process::id()))
            });
            let num = |k: &str, d: u64| val(k).and_then(|v| v.parse().ok()).unwrap_or(d);
            let d = lock::lock_dir();
            match args.get(1).map(|s| s.as_str()) {
                Some("acquire") => match lock::acquire(&d, &owner, num("--wait", 0), num("--max-age", 0)) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{e}");
                        1
                    }
                },
                Some("release") => match lock::release(&d, &owner) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{e}");
                        1
                    }
                },
                Some("status") => lock::status(&d),
                _ => {
                    eprintln!(
                        "shootctl lock acquire|release|status [--owner WHO] [--wait S] [--max-age S]"
                    );
                    2
                }
            }
        }
        "lint" => {
            if args.len() < 3 {
                eprintln!("lint needs <api.json> and at least one .as");
                2
            } else {
                lint(&args[1], &args[2..])
            }
        }
        "webms" => {
            // debug: what the driver can see in the screenshots folder
            let v = webm_snapshot();
            println!("{} webm files", v.len());
            let mut byt: Vec<_> = v.into_iter().collect();
            byt.sort_by_key(|(_, (t, _))| *t);
            for (p, (_, n)) in byt.iter().rev().take(3) { println!("  {p}  {n} bytes"); }
            0
        }
        "install" => {
            // the dev loop: lint, reload, prove -- what gsdev used to be
            let p = "/mnt/c/Users/vjeux/OpenplanetNext/Plugins/GhostShooter";
            let g = "/home/vjeux/gs-good";
            let a = "/mnt/c/Users/vjeux/OpenplanetNext/OpenplanetNext.json";
            install(p, g, a)
        }
        "save" => {
            let p = "/mnt/c/Users/vjeux/OpenplanetNext/Plugins/GhostShooter";
            save_good(p, "/home/vjeux/gs-good")
        }
        "launch" => {
            let force = args.iter().any(|a| a == "--force");
            let to = args.iter().skip(1).find_map(|a| a.parse::<u64>().ok()).unwrap_or(180);
            launch(to, force)
        }
        // ONE MAP, EVERYTHING THE GAME SAYS WHILE IT LOADS IT (or does not).
        "probe" => {
            let val = |k: &str| -> Option<String> {
                args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
            };
            let Some(map) = val("--map") else {
                eprintln!("shootctl probe --map <path> [--how edit|play|editmap2|editghosts] [--mode M] [--timeout S]");
                std::process::exit(2);
            };
            // `--play` is kept as the spelling for `--how play`.
            let how = val("--how").unwrap_or_else(|| {
                if args.iter().any(|a| a == "--play") { "play".into() } else { "edit".into() }
            });
            let mode = val("--mode").unwrap_or_default();
            let to = val("--timeout").and_then(|v| v.parse().ok()).unwrap_or(90);
            probe(&map, &how, &mode, to)
        }
        "run" => {
            // THE WHOLE THING: cold game to finished video, one command.
            let mut map = String::new();
            let mut name = String::new();
            let mut gs: Vec<String> = Vec::new();
            let mut bad = String::new();
            let mut cam: u8 = 2;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--map"  => { map  = args[i + 1].clone(); i += 2; }
                    "--name" => { name = args[i + 1].clone(); i += 2; }
                    "--cam" => {
                        match args.get(i + 1).and_then(|v| v.parse::<u8>().ok()) {
                            Some(c) if c <= 6 => cam = c,
                            _ => { bad = "--cam (takes 0..6)".into(); }
                        }
                        i += 2;
                    }
                    "--force" => { i += 1; }   // handled below; not a ghost
                    other if other.starts_with("--") => {
                        // An unknown flag silently became a GHOST PATH and the
                        // run died on "stage --force: No such file". Refuse.
                        bad = other.to_string();
                        i += 1;
                    }
                    _ => { gs.push(args[i].clone()); i += 1; }
                }
            }
            if !bad.is_empty() { eprintln!("unknown option: {bad}"); }
            if !bad.is_empty() { 2 }
            else if map.is_empty() || gs.is_empty() || name.is_empty() {
                eprintln!("run [--force] --map <map> --name <out> [--cam N] <tas.Ghost.Gbx> [opponent.Ghost.Gbx]");
                2
            } else {
                let rc = launch(180, args.iter().any(|a| a == "--force"));
                if rc != 0 { rc }
                else {
                    let rc = setup(&map, &gs, cam);
                    if rc != 0 { rc } else { shoot(3600, &name) }
                }
            }
        }
        "shoot" => {
            // shoot [timeout_s] [--name NAME]
            let mut to = 3600u64;
            let mut name = String::new();
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--name" { name = args[i + 1].clone(); i += 2; }
                else { to = args[i].parse().unwrap_or(3600); i += 1; }
            }
            if name.is_empty() {
                name = format!("shoot_{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
            }
            shoot(to, &name)
        }
        "setup" => {
            let mut map = String::new();
            let mut gs: Vec<String> = Vec::new();
            let mut cam: u8 = 2;
            let mut badcam = false;
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--map" { map = args[i + 1].clone(); i += 2; }
                else if args[i] == "--cam" {
                    match args.get(i + 1).and_then(|v| v.parse::<u8>().ok()) {
                        Some(c) if c <= 6 => cam = c,
                        _ => badcam = true,
                    }
                    i += 2;
                }
                else { gs.push(args[i].clone()); i += 1; }
            }
            if badcam { eprintln!("--cam takes 0..6 (2 External, 1 Internal, 6 Ext2, 3 Helico)"); 2 }
            else if map.is_empty() || gs.is_empty() { eprintln!("setup --map <path> [--cam N] <ghost> [ghost]"); 2 }
            else { setup(&map, &gs, cam) }
        }
        "import" => {
            if args.len() < 2 { 2 } else { stage_and_import(&args[1..]) }
        }
        "route" => {
            if args.len() < 4 { 2 } else { add_route(&args[1], &args[2], &args[3]) }
        }
        "stamp" => {
            if args.len() < 3 { 2 } else { stamp(&args[1], &args[2]) }
        }
        "get" => match http_get(&args[1], 25) {
            Ok(b) => {
                println!("{b}");
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        },
        "wait" => {
            let mut want = 0i64;
            let mut to = 60u64;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--ctx" => { want = args[i + 1].parse().unwrap_or(0); i += 2; }
                    "--timeout" => { to = args[i + 1].parse().unwrap_or(60); i += 2; }
                    _ => i += 1,
                }
            }
            match wait_ctx(want, to) {
                Ok(t) => { println!("ctx={want} after {t:.1}s"); 0 }
                Err(e) => { eprintln!("{e}"); 1 }
            }
        }
        "drive" => {
            let mut map = String::new();
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--map" { map = args[i + 1].clone(); i += 2; } else { i += 1; }
            }
            if map.is_empty() {
                eprintln!("drive needs --map <path>");
                std::process::exit(2);
            }
            let map = match game_path(&map) {
                Ok(m) => m,
                Err(e) => { eprintln!("{e}"); std::process::exit(2); }
            };
            // The plugin reads the path from a file: paths carry backslashes and
            // spaces, and a hand-rolled URL decoder is one more thing to be wrong
            // about.
            let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
            let _ = std::fs::create_dir_all(store);
            if let Err(e) = std::fs::write(format!("{store}/editmap.txt"), &map) {
                eprintln!("editmap.txt: {e}");
                std::process::exit(2);
            }
            println!("editmap: {}", http_get("/editmap", 30).unwrap_or_default().trim());
            match wait_ctx(1, 90) {
                Ok(t) => println!("  editor open after {t:.1}s"),
                Err(e) => { eprintln!("{e}"); std::process::exit(1); }
            }
            println!("mediatracker: {}", http_get("/mt2", 30).unwrap_or_default().trim());
            match wait_ctx(2, 60) {
                Ok(t) => { println!("  MediaTracker after {t:.1}s"); 0 }
                Err(e) => { eprintln!("{e}"); 1 }
            }
        }
        other => {
            eprintln!("unknown command: {other}");
            2
        }
    };
    std::process::exit(code);
}

/// Add a route to the plugin's dispatch table if it is not already there.
/// Keeps Main.as edits out of shell one-liners and out of Python.
fn add_route(main_as: &str, route: &str, expr: &str) -> i32 {
    let src = match std::fs::read_to_string(main_as) {
        Ok(s) => s,
        Err(e) => { eprintln!("{main_as}: {e}"); return 2; }
    };
    let key = format!("\"{route}\"");
    if src.contains(&key) {
        println!("route {route} already present");
        return 0;
    }
    let anchor = "    if (r == \"/ping\") return HttpResponse(200, \"pong\");";
    if !src.contains(anchor) {
        eprintln!("anchor route not found in {main_as}");
        return 2;
    }
    let line = format!("    if (r == \"{route}\") return HttpResponse(200, {expr});");
    let out = src.replace(anchor, &format!("{anchor}\n{line}"));
    if std::fs::write(main_as, out).is_err() { return 2; }
    println!("added route {route}");
    0
}

/// Write the plugin's argument file. Paths carry backslashes and spaces, and a
/// hand-rolled URL decoder is one more thing to be wrong about, so the plugin
/// reads them from disk instead of from the query string.
fn set_arg(v: &str) -> Result<(), String> {
    let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
    std::fs::create_dir_all(store).map_err(|e| e.to_string())?;
    std::fs::write(format!("{store}/arg.txt"), v).map_err(|e| e.to_string())
}

/// Import one ghost and PROVE it landed: the plugin reports the ghost-block
/// count before and after, and this refuses to report success unless it rose.
/// Get the game to a state with no modal up, and PROVE it -- dismissing what
/// will not go away by itself.
///
/// Importing a ghost raises `FrameMessage` "Updating data..." with a progress
/// bar stuck at 0. It is not transient: measured, it sat there for the full
/// 120 s a `nodialog` wait allowed, and the next `ImportGhosts` opened its file
/// dialog underneath it, reporting `before:N after:N` -- a silent no-op that
/// reads exactly like a missing file. `/dismiss` clears it in one frame, and it
/// answers each frame id its own correct way (message -> Ok, ask-yes-no -> No,
/// save-as -> Cancel), so the default is always to decline rather than to save
/// the map we are filming.
///
/// So: wait briefly, and if something is still up, dismiss it and wait again.
/// The short wait is what makes this cheap when nothing is wrong; the dismiss
/// is what makes it work when something is.
fn clear_dialogs(what: &str) -> Result<(), String> {
    for attempt in 0..6 {
        if await_cond("nodialog", 5).is_ok() {
            return Ok(());
        }
        let d = http_get("/dismiss", 15).unwrap_or_default();
        println!("  dialog before {what}: dismissed ({}) [{}]", d.trim(), attempt + 1);
    }
    Err(format!("a dialog will not clear before {what}: {}", http_get("/ctx", 15).unwrap_or_default().trim()))
}

fn import_ghost(rel: &str) -> Result<(), String> {
    // RETRY, because the modal that eats the import ARRIVES DURING IT.
    //
    // Clearing dialogs first is necessary and not sufficient: entering the
    // MediaTracker and importing both queue an "Updating data..." FrameMessage
    // that can surface a frame or two AFTER the driver has looked and found
    // nothing. The import then opens its file dialog underneath it and returns
    // `before:N after:N` with the message frame named in its own reply -- the
    // whole no-op is visible in that one line, which is what makes a retry
    // safe: success here is the ghost-block count going UP, never an
    // assumption, so a retry cannot import twice or import the wrong thing.
    //
    // Measured 2026-08-22: with only the up-front clear, two of three queued
    // renders died here, each on a different ghost of the pair, on a game that
    // was working perfectly.
    let mut last = String::new();
    for attempt in 1..=4 {
        clear_dialogs(rel)?;
        set_arg(rel)?;
        let body = http_get("/import", 30)?;
        let num = |k: &str| -> i64 {
            let key = format!("\"{k}\":");
            match body.find(&key) {
                Some(i) => {
                    let rest = &body[i + key.len()..];
                    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
                    rest[..end].parse().unwrap_or(-1)
                }
                None => -1,
            }
        };
        let (before, after) = (num("before"), num("after"));
        if after > before {
            println!("  imported {rel}  ({before} -> {after} ghost blocks, attempt {attempt})");
            return Ok(());
        }
        println!("  import of {rel} did not take (attempt {attempt}): {}", body.trim());
        last = body;
    }
    Err(format!(
        "import of {rel} did not take in 4 attempts, last: {last}. The ghost-block count never \
         rose, so nothing was imported -- this is not a timing wobble."
    ))
}

/// Stage exactly the ghosts for this render into their own folder, in import
/// order, and import them. The isolated folder is the point: the picker used to
/// be a 12-row paged list indexed by `ls | sort`, and one stray file -- or an
/// `old/` subdirectory -- silently imported the wrong car.
fn stage_and_import(files: &[String]) -> i32 {
    let shoot = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Replays/_shoot";
    let _ = std::fs::create_dir_all(shoot);
    if let Ok(rd) = std::fs::read_dir(shoot) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let mut names = Vec::new();
    for (i, f) in files.iter().enumerate() {
        let name = format!("{}_{}.Ghost.Gbx", i + 1, if i == 0 { "TAS" } else { "OPP" });
        if let Err(e) = std::fs::copy(f, format!("{shoot}/{name}")) {
            eprintln!("stage {f}: {e}");
            return 2;
        }
        names.push(name);
    }
    println!("staged {} ghost(s) into _shoot", names.len());
    if let Err(e) = http_get("/rmtracks", 15) {

        eprintln!("{e}");
        return 1;
    }
    // The TAS car goes in FIRST: the camera follows clip entity 1, and the clip
    // belongs to our run.
    for n in &names {
        if let Err(e) = import_ghost(&format!("_shoot/{n}")) {
            eprintln!("{e}");
            return 1;
        }
    }
    0
}

/// The whole scene, set up and PROVEN, with no clicks and no sleeps:
///   map -> editor -> MediaTracker -> ghosts -> camera on our car.
/// The MapUid of a .Map.Gbx, read from its header.
///
/// Not a GBX parser: the uid is the first string in the header string chunk and
/// is plainly there in the first few hundred bytes -- 27 base64-ish characters,
/// length-prefixed. Reading it costs a 4 KB read and lets `setup` tell whether
/// the map it wants is ALREADY open, which is worth 11.5 s.
///
/// Returns None rather than guessing; a None simply means "reload it", which is
/// the old behaviour.
fn map_uid(path: &str) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 4096];
    let n = f.read(&mut buf).ok()?;
    let b = &buf[..n];
    if &b[..3] != b"GBX" { return None; }
    // scan for a u32 length followed by that many printable ASCII bytes,
    // 20..=40 long -- the uid, then the map's collection/author strings.
    let mut i = 8;
    while i + 4 < b.len() {
        let len = u32::from_le_bytes([b[i], b[i+1], b[i+2], b[i+3]]) as usize;
        if (20..=40).contains(&len) && i + 4 + len <= b.len() {
            let s = &b[i + 4..i + 4 + len];
            if s.iter().all(|c| c.is_ascii_graphic()) {
                return Some(String::from_utf8_lossy(s).to_string());
            }
        }
        i += 1;
    }
    None
}

/// The uid of the map the game currently has open, if any.
fn loaded_uid() -> Option<String> {
    let b = http_get("/loaded", 10).ok()?;
    if !b.contains("\"loaded\":true") { return None; }
    let k = "\"uid\":\"";
    let i = b.find(k)? + k.len();
    let rest = &b[i..];
    let e = rest.find('"')?;
    let u = rest[..e].to_string();
    if u.is_empty() { None } else { Some(u) }
}

///
/// Every step is checked against the object graph before the next one runs, so
/// a failure names the step instead of surfacing later as a black video.
/// The game's camera modes, from the plugin's own enum. Named so a log line and
/// a `--cam` argument can both say what was actually filmed instead of a digit.
///
/// Which one suits a map is a property of the MAP:
///   * **2 External** — the stock chase, and the right default. It keeps the
///     WORLD's up-vector, which is what makes a normal run readable.
///   * **1 Internal** — cockpit. Rolls WITH the car, so a magnet map's
///     ceiling-driving reads as driving rather than as a car stuck to the sky.
///   * **6 Ext2** — the alternate external, framed closer and car-relative.
///   * **3 Helico** — overhead. Good for a route, useless for car attitude.
///   * **0 Default / 4 Free / 5 Spectator** — not aimed at our entity in a
///     useful way here; listed because the plugin accepts them and a typo
///     should say what it asked for.
pub fn cam_name(cam: u8) -> &'static str {
    match cam {
        0 => "Default",
        1 => "Internal (cockpit)",
        2 => "External (stock chase)",
        3 => "Helico (overhead)",
        4 => "Free",
        5 => "Spectator",
        6 => "Ext2",
        _ => "unknown",
    }
}

fn setup(map: &str, ghosts: &[String], cam: u8) -> i32 {
    let map = &match game_path(map) {
        Ok(m) => m,
        Err(e) => { eprintln!("{e}"); return 2; }
    };
    let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
    let _ = std::fs::create_dir_all(store);
    if std::fs::write(format!("{store}/editmap.txt"), map).is_err() {
        eprintln!("could not write editmap.txt");
        return 2;
    }

    // DO NOT RELOAD A MAP THAT IS ALREADY OPEN.
    //
    // This is the most expensive step there is: 11.5 s for the 1.9 MB
    // Underwater map. Only ~3.5 s of that is entering the editor -- measured
    // against a 115 KB map, which took 3.8 s -- the rest is the game building
    // the scene, which is real work and cannot be made faster. It CAN be not
    // repeated. Identity is the MapUid, from the file's header and from
    // RootMap.MapInfo; comparing names would not do, since our own edited
    // copies of a map all carry the same MapName.
    let wsl = map.strip_prefix("C:/").map(|r| format!("/mnt/c/{r}"))
                 .unwrap_or_else(|| map.to_string());
    let want = map_uid(&wsl);
    let have = loaded_uid();
    let already = want.is_some() && want == have && matches!(ctx(), Some(1) | Some(2));

    if already {
        println!("map already open ({}) -- not reloading", want.clone().unwrap_or_default());
        // The MediaTracker may or may not be open on top of it.
        if ctx() != Some(2) {
            let _ = http_get("/mt2", 30);
            if let Err(e) = wait_ctx(2, 60) { eprintln!("{e}"); return 1; }
        }
        // A previous render's tracks are still in the clip; start clean.
        let _ = http_get("/rmtracks", 20);
    } else {
        // EditMap refuses while any editor is open, so get to the menu FIRST and
        // prove it -- the old code slept and hoped.
        if let Err(e) = to_menu() { eprintln!("{e}"); return 1; }
        // AND THE TITLE MUST BE READY -- here, where it means something.
        // EditMap on a not-ready ManiaTitleControlScriptAPI returns without
        // error and loads nothing, which is the failure that reads as "the map
        // did not open". IsReady is false while an editor is open, so this can
        // only be asked once to_menu() has proved we left it.
        if let Err(e) = await_cond("ready", 60) { eprintln!("{e}"); return 1; }
        println!("editmap: {}", http_get("/editmap", 30).unwrap_or_default().trim());
        match wait_ctx(1, 120) {
            Ok(t) => println!("  editor after {t:.1}s"),
            Err(e) => { eprintln!("{e}"); return 1; }
        }
        let _ = http_get("/mt2", 30);
        match wait_ctx(2, 60) {
            Ok(t) => println!("  MediaTracker after {t:.1}s"),
            Err(e) => { eprintln!("{e}"); return 1; }
        }
        // AND THE GAME MUST HAVE LOADED WHAT WE ASKED FOR. EditMap on a bad
        // path returns "ok" and loads nothing; the uid says whether it landed.
        if let (Some(w), Some(h)) = (&want, &loaded_uid()) {
            if w != h {
                eprintln!("asked for map {w} but the game loaded {h}");
                return 1;
            }
        }
    }
    let rc = stage_and_import(ghosts);
    if rc != 0 { return rc; }

    // The camera track: type 23 is CGameCtnMediaBlockCameraGame. It is NOT
    // creatable until at least one ghost is in the clip, which is why this runs
    // after the import.
    let mk = http_get("/mktrack?type=23", 20).unwrap_or_default();
    if !mk.contains("->") {
        eprintln!("camera track: {mk}");
        return 1;
    }
    // ent=1 is the FIRST imported ghost -- our car, always. cam=2 is External,
    // the stock chase. A fresh block targets entity 0 (nobody) and renders
    // black, which used to pass every size and duration check we had.
    // ent=1 is the FIRST imported ghost -- our car, always. `cam` is the game
    // camera: 2 (External, the stock chase) is the default and what every clip
    // in the repo before 2026-08-24 used. A fresh block targets entity 0
    // (nobody) and renders black, which used to pass every size and duration
    // check we had.
    //
    // WHY THIS IS A FLAG NOW. External holds the WORLD's up-vector, so on a
    // magnet map -- where the car drives on ceilings and walls -- the shot is
    // of an upside-down car in a frame that never rolls with it, and you cannot
    // see what the run is doing. The camera is a property of the MAP, not of
    // the pipeline, and it was hardcoded.
    let set = http_get(&format!("/camset?ent=1&cam={cam}"), 20).unwrap_or_default();
    println!("  camera: {} ({})", set.trim(), cam_name(cam));
    let st = http_get("/camstate", 20).unwrap_or_default();
    if st.contains("\"entid\":0") || !st.contains(&format!("\"gamecam\":{cam}")) {
        eprintln!("camera did not take: {st}");
        return 1;
    }
    println!("  {}", st.trim());
    println!("scene ready");
    0
}

/// Get back to the menu from wherever we are, and PROVE it.
///
/// EditMap refuses while an editor is open ("already in an editor - /back
/// first") and the old code's answer was to sleep and hope. Leaving is a chain,
/// not one call: the MediaTracker has to be quit before the map editor can be
/// left, and leaving a modified map raises a dialog that must be ANSWERED
/// CORRECTLY -- "yes" to all of them saves the map, silently editing the very
/// maps we are meant to be filming unmodified.
fn to_menu() -> Result<(), String> {
    for _ in 0..12 {
        // MEASURED, not assumed: MTApi::Quit() returns "ok" and leaves the
        // MediaTracker open, while BackToMainMenu() steps MT -> map editor ->
        // menu one level at a time, raising the save prompt on the way out.
        // So /back is the only mover, and it is called repeatedly.
        if ctx() == Some(0) { return Ok(()); }
        let _ = http_get("/back", 20);
        // Answer whatever modal the exit raised; /dismiss picks the right answer
        // per dialog and defaults to declining. Each answer is followed by a
        // WAIT ON THE DIALOG GOING AWAY, not by a sleep -- and if a new one
        // takes its place, /dismiss handles that one on the next turn.
        for _ in 0..6 {
            let c = http_get("/ctx", 10).unwrap_or_default();
            if c.contains("\"dialog\":null") { break; }
            let _ = http_get("/dismiss", 10);
            let _ = await_cond("nodialog", 5);
        }
        if wait_ctx(0, 8).is_ok() { return Ok(()); }
    }
    Err(format!("could not get back to the menu; ctx={:?}", ctx()))
}

/// START THE GAME -- unless it is already up and usable.
///
/// WHERE THE 11.5 SECONDS GO. From Openplanet's own log, cold:
///
///   +0.0s   process created
///   +1.3s   "Setting up hook callbacks..."
///   +1.9s   app config request (network, 5 s timeout)
///   +7.2s   NadeoServices account ID          <- 5.3 s, a web-services LOGIN
///   +8.3s   loop entry init, sockets, audio
///   +8.6s   registered 2659 classes
///   +9.1s   wrote Openplanet.h and the two json dumps
///   +9.3s   ~20 plugins loaded; GhostShooter's server up
///
/// So more than half of it is Openplanet authenticating against Nadeo before it
/// will finish initialising, and essentially all the rest is its own start-up.
/// None of it is ours and none of it is avoidable -- but it is a ONCE cost, and
/// the pipeline used to pay it on every single render by killing the game
/// first. So: if the plugin answers and the title is ready, this does nothing.
/// `--force` restarts anyway, which is what you want after editing the plugin.
///
/// Nothing here sleeps. Each wait is a blocking operation whose duration IS the
/// wait: tasklist for the processes, and connect() for the plugin -- a WSL
/// connect to a Windows port that is not listening is dropped rather than
/// refused, so it blocks the full timeout and paces the retry by itself.
///
/// WHY EXPLORER: Openplanet is a dinput8.dll proxy beside Trackmania.exe, and
/// the PreferSystem32 process-creation mitigation makes the loader take
/// System32's real dinput8 instead -- the game runs perfectly and the plugin
/// never loads. The mitigation is INHERITED, and every shell we have has it ON
/// while Explorer has it OFF. `explorer.exe <path>` makes the running Explorer
/// create the process, so it inherits OFF. (Measured 2026-08-21.)
fn launch(timeout_s: u64, force: bool) -> i32 {
    let t0 = Instant::now();
    let el = |t: &Instant| t.elapsed().as_secs_f64();

    // LIVENESS IS THE PLUGIN ANSWERING, nothing more. The plugin only exists if
    // the game is up AND Openplanet injected, which is the whole question.
    //
    // NOT IsReady: that is ManiaTitleControlScriptAPI, and it means "ready to
    // accept a command like EditMap" -- it reads FALSE whenever an editor is
    // open. Using it as a liveness check killed a perfectly good game between
    // two renders and paid the whole 12 s over again. It is checked where it
    // actually applies: immediately before EditMap, in setup().
    if !force && plugin_up() {
        println!("[0.0s] game already up -- not restarting");
        return 0;
    }

    for exe in ["Trackmania.exe", "UbisoftGameLauncher.exe"] {
        let _ = std::process::Command::new("/mnt/c/Windows/System32/taskkill.exe")
            .args(["/F", "/IM", exe]).output();
    }
    // Wait for them to be GONE. tasklist is a process spawn, ~100 ms; that is
    // the pacing, not a sleep.
    while tm_running() {
        if el(&t0) > 30.0 { eprintln!("Trackmania will not die"); return 1; }
    }
    println!("[{:.1}s] processes gone", el(&t0));

    // TRY, DIAGNOSE, RETRY. The launch fails intermittently and it is not our
    // fault: Openplanet injects, initialises, and then HANGS on the Nadeo
    // web-services login. Its own log ends at "Did not find update to
    // install." and never reaches "Loop entry initialization" -- so the script
    // engine never starts and our plugin never exists. Measured 2026-08-22:
    // one launch stalled there for the full 180 s while the game ran fine.
    //
    // The old code waited out the timeout and then blamed PreferSystem32,
    // which was simply wrong -- the log proves Openplanet was in the process.
    // Now the log IS the diagnosis, and a hung login is retried rather than
    // reported as a broken install.
    for attempt in 1..=3 {
        let game = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Trackmania\\Trackmania.exe";
        let _ = std::process::Command::new("/mnt/c/Windows/explorer.exe").arg(game).output();
        println!("[{:.1}s] launched via explorer (attempt {attempt})", el(&t0));

        // Wait for the plugin socket, trying EVERY candidate address each time
        // -- the right one cannot be known before the server exists.
        // connect() blocks, so it paces itself; nothing sleeps.
        let a0 = Instant::now();
        let mut up = false;
        while a0.elapsed().as_secs() < 45 {
            if plugin_up() { up = true; break; }
        }
        if up {
            println!("[{:.1}s] plugin up", el(&t0));
            // AND THE TITLE MUST BE READY. EditMap on a not-ready
            // ManiaTitleControlScriptAPI returns without error and loads
            // nothing -- the failure that reads as "the map did not open".
            return match await_cond("ready", 60) {
                Ok(_) => { println!("[{:.1}s] title ready", el(&t0)); 0 }
                Err(e) => { eprintln!("{e}"); 1 }
            };
        }

        match openplanet_stage() {
            OpStage::NotInjected => {
                eprintln!("[{:.1}s] Openplanet never injected -- its log did not open.", el(&t0));
                eprintln!("  The launch inherited PreferSystem32=ON and the dinput8 proxy");
                eprintln!("  was skipped. Retrying will not help; check the launch parent.");
                return 1;
            }
            OpStage::StalledAtLogin => {
                eprintln!("[{:.1}s] Openplanet hung on the Nadeo login (attempt {attempt}) -- restarting",
                          el(&t0));
                for exe in ["Trackmania.exe", "UbisoftGameLauncher.exe"] {
                    let _ = std::process::Command::new("/mnt/c/Windows/System32/taskkill.exe")
                        .args(["/F", "/IM", exe]).output();
                }
                while tm_running() {}
            }
            OpStage::Running => {
                eprintln!("[{:.1}s] Openplanet finished starting but the plugin is not listening.",
                          el(&t0));
                eprintln!("  That is a GhostShooter problem, not a launch one -- check the log");
                eprintln!("  for a compile error, which unloads the plugin and takes the server.");
                return 1;
            }
        }
        if el(&t0) > timeout_s as f64 { break; }
    }
    eprintln!("[{:.1}s] gave up after 3 launches", el(&t0));
    1
}

/// How far Openplanet got, from its own log.
enum OpStage {
    /// The log never opened for this launch: Openplanet is not in the process.
    NotInjected,
    /// It injected and stopped at the web-services login -- the intermittent
    /// hang. Everything after that line ("Loop entry initialization", the
    /// script engine, our plugin) never happens.
    StalledAtLogin,
    /// It finished starting. If the plugin still is not listening, that is the
    /// plugin's fault, not the launch's.
    Running,
}

fn openplanet_stage() -> OpStage {
    let Ok(log) = std::fs::read_to_string("/mnt/c/Users/vjeux/OpenplanetNext/Openplanet.log")
    else { return OpStage::NotInjected };
    if !log.contains("Openplanet starting on") { return OpStage::NotInjected; }
    if log.contains("Loop entry initialization") { return OpStage::Running; }
    OpStage::StalledAtLogin
}

fn tm_running() -> bool {
    std::process::Command::new("/mnt/c/Windows/System32/tasklist.exe")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("trackmania.exe"))
        .unwrap_or(false)
}

/// SYNTHETIC INPUT IS GONE FROM THIS PIPELINE. Nothing here clicks or types.
///
/// The history, so nobody re-derives it: the shoot dialog was driven by a click
/// at a screen coordinate, then briefly by a raw-scancode Enter, and both were
/// wrong. The click was read off a 1568-wide view of a 3840x2160 screen, landed
/// 2.45x out and hit the Import Ghosts dialog behind. The Enter was worse: the
/// dialog opens with EnumFileFormat focused, not OK, so it cycled the output
/// format and produced an AVI nobody asked for.
///
/// The dialog is a frame of the GAME dialog menu -- CGameCtnMenus::Dialogs,
/// MenuOrder 5 -- not of BasicDialogs (MenuOrder 11), which is where every
/// earlier search looked. Its controls are bound to the CGameDialogShootParams,
/// so OnOk is a plain API call. See ShootNod.as.

// CGameSwitcher::FocusDialogCount is not used: it does not distinguish WHICH
// dialog, it does not stack, and the ghost import used to leave it at 1 with
// nothing on screen. /focusdlg is still there for probing by hand.

// newest_video() is GONE. "the newest .webm in the folder" was how the driver
// found its own result, and it is a guess: another render, an unrelated
// capture, or a shoot that died before writing all point it at the wrong file
// with no way to notice. The render names its output now -- see shoot().

// file_locked() over PowerShell is GONE, and so is to_win(). The check now
// lives in the plugin (/awaitfile) where it costs one CreateFile per frame
// instead of a process spawn four times a second FOR THE WHOLE RENDER,
// competing with the encoder for the machine. The finding it was built on
// stands and is worth keeping: a read-open that also denies writers fails while
// the game holds the file, and that release is the only exact completion
// signal there is.

/// Are these two paths the SAME FILE? Not a string compare: fs::copy onto
/// itself TRUNCATES, and that destroyed a finished 24 MB render -- the only
/// reason it was caught is that the tool printed "done: 0 bytes". Asked of the
/// filesystem, so a differently-spelled path cannot slip through.
/// The WSL path `/mnt/c/...` as a Windows path with forward slashes, which is
/// what Openplanet's IO takes.
fn to_win_fwd(p: &str) -> String {
    p.strip_prefix("/mnt/c/").map(|r| format!("C:/{r}")).unwrap_or_else(|| p.to_string())
}

fn same_file(a: &str, b: &str) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

// op_in_progress() is GONE too. CGameManiaPlanet::Operation_InProgress reads
// FALSE for the entire duration of a shoot -- measured across a 53-second
// render, along with MTApi IsPlaying (false), CurrentTimer (0) and every
// progress field (empty). The game exposes no render-in-progress signal at all;
// the encoder file handle is the signal -- checked in the plugin, /awaitfile.

fn wait_shoot_dialog(want: bool, secs: u64) -> Result<f64, String> {
    await_cond(if want { "shootdlg" } else { "noshootdlg" }, secs)
}
/// Every .webm in the screenshots folder, with its mtime and size.
///
/// NOT a set of names. The game does NOT always create a new file: it takes the
/// lowest free VideoNN, and if that name already exists it OVERWRITES it --
/// measured, Video56.webm was rewritten in place and the folder listing never
/// changed length. A driver watching for a new NAME waits forever on a render
/// that is running perfectly.
fn webm_snapshot() -> std::collections::HashMap<String, (std::time::SystemTime, u64)> {
    let dir = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/ScreenShots";
    let mut m = std::collections::HashMap::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("webm") { continue; }
            if let Ok(md) = e.metadata() {
                if let Ok(t) = md.modified() {
                    m.insert(p.to_string_lossy().to_string(), (t, md.len()));
                }
            }
        }
    }
    m
}

/// Which .webm changed since the snapshot -- created, or rewritten in place.
///
/// A SNAPSHOT, NOT A TIMESTAMP WINDOW. The window version asked for "files
/// touched in the last second" and matched the PREVIOUS render as well, because
/// two back-to-back runs finish and start inside the same filesystem second --
/// it failed with "2 .webm files were touched; cannot tell which is ours" on a
/// render that was working perfectly. That is not a tuning problem: comparing
/// against what was actually there before is exact, at any clock granularity.
fn webms_changed(
    before: &std::collections::HashMap<String, (std::time::SystemTime, u64)>,
) -> Vec<String> {
    let mut v: Vec<String> = webm_snapshot()
        .into_iter()
        .filter(|(p, now)| before.get(p).map(|was| was != now).unwrap_or(true))
        .map(|(p, _)| p)
        .collect();
    v.sort();
    v
}

/// Shoot, and prove every step of it.
fn shoot(timeout_s: u64, name: &str) -> i32 {
    // WHAT WAS THERE BEFORE. The output is the one .webm this changed -- which
    // covers both cases, a fresh VideoNN and an existing one overwritten in
    // place. Identifying it by newest mtime was a guess that pointed at the
    // wrong file whenever anything else had written one.
    let before = webm_snapshot();

    // A LEFTOVER MODAL EATS THE SHOOT DIALOG. The ghost import raises a sticky
    // "Updating data..." FrameMessage and nothing downstream clears it; `/shoot`
    // then opens the shoot params underneath it and `wait_shoot_dialog` times
    // out on a game that did exactly what it was told.
    if let Err(e) = clear_dialogs("the shoot dialog") {
        eprintln!("{e}");
        return 1;
    }

    println!("rewind: {}", http_get("/rewind", 20).unwrap_or_default().trim());

    println!("shoot:  {}", http_get("/shoot", 20).unwrap_or_default().trim());

    // WAIT FOR THE DIALOG ITSELF. Not for a frame name, not for a modal count:
    // the plugin holds the CGameDialogShootParams nod and says whether it is
    // there. Both of the earlier gates were wrong in the same way -- they
    // matched the ghost-import file dialog left over from the previous step and
    // passed instantly.
    match wait_shoot_dialog(true, 30) {
        Ok(t) => println!("  shoot dialog up after {t:.1}s"),
        Err(e) => { eprintln!("{e}"); return 1; }
    }

    // Set what is worth setting and CHECK the container.
    //
    // ShootName is written and reads back, but it does NOT name the output:
    // measured -- the dialog reported "name":"uw_deck_v1" and the game wrote
    // Video54.webm off its own counter anyway. It is set as a label, and this
    // copies the result to that name at the end instead. ExtVideo is CHECKED
    // rather than written: the dump gives its enum no name to construct.

    if let Err(e) = set_arg(name) { eprintln!("{e}"); return 1; }
    let params = http_get("/shootsetup?ext=1", 15).unwrap_or_default();
    println!("  params: {}", params.trim());
    if params.contains("\"err\"") { eprintln!("refusing to render"); return 1; }

    // ACCEPT IT: CGameDialogShootParams::OnOk, the call the OK button makes.
    // NO KEYSTROKE, NO CLICK. Enter is actively harmful here -- the dialog opens
    // with EnumFileFormat focused, not OK, so a blind Enter cycles the output
    // format (that is how a render silently came out as AVI).
    println!("  ok: {}", http_get("/shootok", 20).unwrap_or_default().trim());
    match wait_shoot_dialog(false, 15) {
        Ok(t) => println!("  accepted after {t:.1}s"),
        Err(e) => { eprintln!("{e}"); return 1; }
    }

    // EXACTLY ONE FILE MUST START BEING WRITTEN.
    //
    // The pacing is the directory read itself -- a stat of ~100 files over
    // DrvFs, which is not free. No sleep.
    let t_render = Instant::now();
    let mut out = String::new();
    while t_render.elapsed().as_secs() < 30 {
        let touched = webms_changed(&before);
        match touched.len() {
            0 => {}
            1 => { out = touched[0].clone(); break; }
            n => { eprintln!("{n} .webm files were touched; cannot tell which is ours"); return 1; }
        }
    }
    if out.is_empty() { eprintln!("the dialog closed but no render started"); return 1; }
    let short = out.rsplit('/').next().unwrap_or(&out).to_string();
    println!("  writing {short}");

    // AND THE GAME MUST LET GO OF IT.
    //
    // Nothing in the object graph moves during a shoot -- measured across a
    // whole 53-second render -- so there is no state to wait on. The ENCODER's
    // file handle is the signal: a read-open that also denies writers fails
    // while the game holds it. Exact, and unlike "the size stopped changing" it
    // tells a finished render from a stalled one.
    //
    // THE PLUGIN DOES THE CHECK, once per frame, and answers the instant the
    // handle is released. The driver had been doing it over PowerShell -- a
    // process spawn four times a second for the whole render, competing with
    // the encoder for the machine. This is one blocking HTTP call.
    if let Err(e) = set_arg(&to_win_fwd(&out)) { eprintln!("{e}"); return 1; }
    let body = match http_get(&format!("/awaitfile?ms={}", timeout_s * 1000), timeout_s + 30) {
        Ok(b) => b,
        Err(e) => { eprintln!("waiting for the render: {e}"); return 1; }
    };
    if !body.contains("\"ok\":true") {
        eprintln!("the render never finished: {}", body.trim());
        return 1;
    }
    let sz = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    if sz == 0 { eprintln!("{out} is empty"); return 1; }
    println!("rendered: {short} ({sz} bytes, {:.1}s)", t_render.elapsed().as_secs_f64());

    // GET IT OUT OF THE WAY. The game reuses VideoNN names and overwrites
    // without asking, so a render left under one of those names is one render
    // away from being destroyed.
    //
    // UNLESS IT ALREADY HAS THE NAME. The game honours ShootName SOMETIMES --
    // Video54.webm on one run, uw_deck_v2.webm on the next, same code -- and
    // std::fs::copy onto itself TRUNCATES. That destroyed a finished 24 MB
    // render and reported "done: 0 bytes", which is the only reason it was
    // noticed. Never copy a file onto itself.
    let keep = format!(
        "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/ScreenShots/{name}.webm");
    if same_file(&out, &keep) {
        println!("done: {out} ({sz} bytes) -- the game used the name itself");
        return 0;
    }
    match std::fs::copy(&out, &keep) {
        Ok(n) if n == sz => { println!("done: {keep} ({n} bytes)"); 0 }
        Ok(n) => { eprintln!("copy is {n} bytes but the render was {sz}"); 1 }
        Err(e) => { eprintln!("could not keep a named copy: {e}"); 1 }
    }
}
