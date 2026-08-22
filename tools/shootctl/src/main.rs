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
}

fn load_api(path: &str) -> Result<Api, String> {
    let raw = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut p = P { b: &raw, i: 0 };
    let v = p.val()?;
    let ns = v.get("ns").ok_or("no ns")?;
    let mut classes = HashSet::new();
    let mut own: HashMap<String, HashMap<String, bool>> = HashMap::new();
    let mut parent: HashMap<String, String> = HashMap::new();
    if let J::Obj(namespaces) = ns {
        for (_nsname, cs) in namespaces {
            if let J::Obj(list) = cs {
                for (cname, info) in list {
                    classes.insert(cname.clone());
                    if let Some(p) = info.get("p").and_then(|x| x.as_str()) {
                        parent.insert(cname.clone(), p.to_string());
                    }
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
    // fold in inherited members
    let mut members = HashMap::new();
    for c in classes.iter() {
        let mut acc: HashMap<String, bool> = HashMap::new();
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
            cur = parent.get(&name).cloned();
        }
        members.insert(c.clone(), acc);
    }
    Ok(Api { classes, members })
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
        match ctx() {
            Some(0) => return Ok(()),
            Some(2) => { let _ = http_get("/mtquit", 20); }
            _ => { let _ = http_get("/back", 20); }
        }
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
