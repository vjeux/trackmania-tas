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

use std::sync::OnceLock;
static ADDR: OnceLock<String> = OnceLock::new();

/// The first candidate address that accepts a connection, remembered for the
/// rest of the run.
fn plugin_addr() -> &'static str {
    ADDR.get_or_init(|| {
        for a in plugin_addrs() {
            if TcpStream::connect_timeout(
                &a.parse::<SocketAddr>().unwrap(),
                Duration::from_millis(400),
            ).is_ok() {
                return a;
            }
        }
        "127.0.0.1:29800".to_string()
    })
}

// ---------------------------------------------------------------------------
// the plugin's HTTP interface
// ---------------------------------------------------------------------------

fn http_get(route: &str, timeout_s: u64) -> Result<String, String> {
    let mut s = TcpStream::connect(plugin_addr()).map_err(|e| format!("connect: {e}"))?;
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

/// The one number that says where the game is: 0 menu, 1 track editor,
/// 2 MediaTracker, 3 in a race.
fn ctx() -> Option<i64> {
    let body = http_get("/ctx", 5).ok()?;
    let key = "\"ctx\":";
    let i = body.find(key)? + key.len();
    let rest = &body[i..];
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Wait for a CONDITION, with a deadline, and say what was seen if it never
/// came. The old pipeline slept a fixed 5 or 8 seconds here and then carried on
/// regardless, which is how a click landed on a screen that had not appeared.
fn wait_ctx(want: i64, timeout_s: u64) -> Result<f64, String> {
    let t0 = Instant::now();
    let mut last = None;
    while t0.elapsed().as_secs() < timeout_s {
        let c = ctx();
        if c == Some(want) {
            return Ok(t0.elapsed().as_secs_f64());
        }
        last = c;
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!(
        "timed out after {timeout_s}s waiting for ctx={want}; last seen ctx={last:?}"
    ))
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "usage:\n  shootctl lint <api.json> <file.as ...>\n  shootctl stamp <Main.as> <STAMP>\
             \n  shootctl get <route>\n  shootctl wait --ctx N [--timeout S]\n  shootctl drive --map <path>"
        );
        std::process::exit(2);
    }
    let code = match args[0].as_str() {
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
            let v = webm_times();
            println!("{} webm files", v.len());
            let mut byt: Vec<_> = v.into_iter().collect();
            byt.sort_by_key(|(_, t)| *t);
            for (p, _) in byt.iter().rev().take(3) { println!("  {p}"); }
            0
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
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--map" { map = args[i + 1].clone(); i += 2; }
                else { gs.push(args[i].clone()); i += 1; }
            }
            if map.is_empty() || gs.is_empty() { eprintln!("setup --map <path> <ghost> [ghost]"); 2 }
            else { setup(&map, &gs) }
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
fn import_ghost(rel: &str) -> Result<(), String> {
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
        println!("  imported {rel}  ({before} -> {after} ghost blocks)");
        Ok(())
    } else {
        Err(format!("import of {rel} did not take: {body}"))
    }
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
///
/// Every step is checked against the object graph before the next one runs, so
/// a failure names the step instead of surfacing later as a black video.
fn setup(map: &str, ghosts: &[String]) -> i32 {
    let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
    let _ = std::fs::create_dir_all(store);
    if std::fs::write(format!("{store}/editmap.txt"), map).is_err() {
        eprintln!("could not write editmap.txt");
        return 2;
    }
    // EditMap refuses while any editor is open, so get to the menu FIRST and
    // prove it -- the old code slept and hoped.
    if let Err(e) = to_menu() { eprintln!("{e}"); return 1; }
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
    let set = http_get("/camset?ent=1&cam=2", 20).unwrap_or_default();
    println!("  camera: {}", set.trim());
    let st = http_get("/camstate", 20).unwrap_or_default();
    if st.contains("\"entid\":0") || !st.contains("\"gamecam\":2") {
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
        // per dialog and defaults to declining.
        for _ in 0..6 {
            let c = http_get("/ctx", 10).unwrap_or_default();
            if c.contains("\"dialog\":null") { break; }
            let _ = http_get("/dismiss", 10);
            std::thread::sleep(Duration::from_millis(300));
        }
        if wait_ctx(0, 8).is_ok() { return Ok(()); }
    }
    Err(format!("could not get back to the menu; ctx={:?}", ctx()))
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

/// IS THE GAME STILL WRITING THIS FILE? The OS's answer, not ours.
///
/// Nothing in the object graph moves during a shoot -- measured over a whole
/// 53-second render: Operation_InProgress false, MTApi IsPlaying false,
/// CurrentTimer 0, no dialog, no progress bar. So the game itself will not say
/// when it is done, and the old gate was "the file size has not changed for
/// three polls", which cannot tell a finished render from a stalled one.
///
/// Windows can. The encoder holds the output open while it writes, so opening
/// it for writing with no sharing fails until the game closes it. That is the
/// writer's own release, and it is exact.
fn file_locked(win_path: &str) -> Option<bool> {
    let ps = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe";
    let script = format!(
        "try {{ $f=[IO.File]::Open('{win_path}','Open','Write','None'); $f.Close(); 'UNLOCKED' }} \
         catch [System.IO.FileNotFoundException] {{ 'MISSING' }} \
         catch {{ 'LOCKED' }}");
    let out = std::process::Command::new(ps)
        .args(["-NoProfile", "-Command", &script])
        .output().ok()?;
    let body = String::from_utf8_lossy(&out.stdout);
    if body.contains("UNLOCKED") { Some(false) }
    else if body.contains("LOCKED") { Some(true) }
    else { None }
}

/// The WSL path `/mnt/c/...` as the Windows path `C:\...`.
fn to_win(p: &str) -> String {
    p.strip_prefix("/mnt/c/")
        .map(|r| format!("C:\\{}", r.replace('/', "\\")))
        .unwrap_or_else(|| p.to_string())
}

/// Shoot, and wait for the file to appear AND stop growing.
///
/// The wait is on the artefact rather than on a duration: a render that dies
/// leaves the file short, and one that is still going keeps growing, so
/// "unchanged for three consecutive polls" is the completion signal.
// op_in_progress() is GONE too. CGameManiaPlanet::Operation_InProgress reads
// FALSE for the entire duration of a shoot -- measured across a 53-second
// render, along with MTApi IsPlaying (false), CurrentTimer (0) and every
// progress field (empty). The game exposes no render-in-progress signal at all;
// the encoder file handle is the signal. See file_locked().

/// Is the shoot dialog up? The plugin answers from the dialog nod itself
/// (`"shootdlg":true`), not from a frame name -- see ShootNod.as.
fn shoot_dialog_up() -> bool {
    http_get("/shootstatus", 10).unwrap_or_default().contains("\"shootdlg\":true")
}

fn wait_shoot_dialog(want: bool, secs: u64) -> Result<f64, String> {
    let t0 = Instant::now();
    while t0.elapsed().as_secs() < secs {
        if shoot_dialog_up() == want { return Ok(t0.elapsed().as_secs_f64()); }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!("the shoot dialog never became {}", if want { "visible" } else { "closed" }))
}

/// Every .webm in the screenshots folder, with its modification time.
///
/// NOT a set of names. The game does NOT always create a new file: it takes the
/// lowest free VideoNN, and if that name already exists it OVERWRITES it --
/// measured, Video56.webm was rewritten in place and the folder listing never
/// changed length. A driver watching for a new NAME waits forever on a render
/// that is running perfectly well. That is exactly what happened here.
fn webm_times() -> Vec<(String, std::time::SystemTime)> {
    let dir = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/ScreenShots";
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("webm") { continue; }
            if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
                v.push((p.to_string_lossy().to_string(), t));
            }
        }
    }
    v
}

/// The .webm files touched since `since` -- the render's output, by definition.
fn webms_since(since: std::time::SystemTime) -> Vec<String> {
    let mut v: Vec<String> = webm_times().into_iter()
        .filter(|(_, t)| *t >= since)
        .map(|(p, _)| p)
        .collect();
    v.sort();
    v
}

/// Shoot, and prove every step of it.
fn shoot(timeout_s: u64, name: &str) -> i32 {
    // WHEN WE STARTED. The output is "the .webm the game touched after this
    // moment" -- which covers both cases, a fresh VideoNN and an existing one
    // overwritten in place. Identifying it by newest mtime alone was a guess
    // that pointed at the wrong file whenever anything else had written one.
    // One second back, because a filesystem timestamp is not that precise.
    let t_start = std::time::SystemTime::now() - Duration::from_secs(1);

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
    let t0 = Instant::now();
    let mut out = String::new();
    while t0.elapsed().as_secs() < 30 {
        let touched = webms_since(t_start);
        match touched.len() {
            0 => {}
            1 => { out = touched[0].clone(); break; }
            n => { eprintln!("{n} .webm files were touched; cannot tell which is ours"); return 1; }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    if out.is_empty() { eprintln!("the dialog closed but no render started"); return 1; }
    let short = out.rsplit('/').next().unwrap_or(&out).to_string();
    println!("  writing {short}");

    // AND THE GAME MUST LET GO OF IT.
    //
    // Nothing in the object graph moves during a shoot -- measured across a
    // whole 53-second render. The old gate was "size unchanged for three
    // polls", which cannot tell a finished render from a stalled one. The
    // encoder's file handle can: see file_locked().
    let win = to_win(&out);
    let t0 = Instant::now();
    let mut last = 0u64;
    while t0.elapsed().as_secs() < timeout_s {
        std::thread::sleep(Duration::from_secs(5));
        let sz = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        match file_locked(&win) {
            Some(false) => {
                // Released. Anything that never got written is a failed render.
                if sz == 0 { eprintln!("\n{out} is empty"); return 1; }
                println!("\nrendered: {short} ({sz} bytes, {:.1}s)", t0.elapsed().as_secs_f64());
                // GET IT OUT OF THE WAY. The game reuses VideoNN names and
                // overwrites without asking, so a render left in the
                // screenshots folder is one render away from being destroyed.
                let keep = format!(
                    "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/ScreenShots/{name}.webm");
                return match std::fs::copy(&out, &keep) {
                    Ok(n) => { println!("done: {keep} ({n} bytes)"); 0 }
                    Err(e) => { eprintln!("could not keep a named copy: {e}"); 1 }
                };
            }
            Some(true) => {
                print!("\r  {short} {sz} bytes   ");
                let _ = std::io::stdout().flush();
                last = sz;
            }
            None => { /* transient -- the probe itself failed, keep waiting */ }
        }
    }
    eprintln!("\ntimed out after {timeout_s}s; {out} is at {last} bytes and still open");
    1
}
