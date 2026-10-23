//! tmxup — drive the TMX upload form in a Chrome window on the render box.
//!
//!   tmxup targets                         the pages Chrome has open (GET /json)
//!   tmxup eval --expr JS [--target S]     Runtime.evaluate, value printed as JSON
//!   tmxup nav --url U [--wait-s N]        Page.navigate, then wait for readyState complete
//!   tmxup html [--selector CSS]           outerHTML of the document or one element
//!   tmxup setfile --selector CSS --file WINPATH   put a file into an <input type=file>
//!   tmxup shot --out FILE.png             a screenshot of the page
//!   tmxup click --selector CSS            a real mouse click at the element's centre
//!   tmxup type --selector CSS --text T|--text-file F   focus the element and insert the text
//!
//! Every command talks to Chrome's DevTools port on the loopback (`--port`, default
//! 9222): Chrome binds it to 127.0.0.1, so this runs ON Windows (built for
//! x86_64-pc-windows-msvc, like inputidle.exe), never from WSL. The window is the
//! dedicated `C:\Users\vjeux\chrome-tmx` profile the owner logged into TMX once;
//! nothing here reads or stores a credential.

mod ws;

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn http_get(port: u16, path: &str) -> Result<String, String> {
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| format!("connect 127.0.0.1:{port}: {e} (is Chrome up with --remote-debugging-port={port}?)"))?;
    s.set_read_timeout(Some(Duration::from_secs(10))).ok();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").as_bytes()).map_err(|e| e.to_string())?;
    // Chrome's DevTools HTTP server does not close after the body: read the head, then
    // exactly Content-Length bytes (a read_to_end sat on the socket until the timeout).
    let mut raw = Vec::new();
    let mut tmp = [0u8; 8192];
    let head_end = loop {
        let n = s.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            break raw.len();
        }
        raw.extend_from_slice(&tmp[..n]);
        if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&raw[..head_end.min(raw.len())]).to_string();
    let want: Option<usize> = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)));
    if let Some(len) = want {
        while raw.len() < head_end + len {
            let n = s.read(&mut tmp).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&tmp[..n]);
        }
    } else {
        loop {
            match s.read(&mut tmp) {
                Ok(0) | Err(_) => break,
                Ok(n) => raw.extend_from_slice(&tmp[..n]),
            }
        }
    }
    Ok(String::from_utf8_lossy(&raw[head_end.min(raw.len())..]).to_string())
}

struct Cdp {
    ws: ws::WebSocket,
    next: u64,
}

impl Cdp {
    fn attach(port: u16, target: Option<&str>) -> Result<Cdp, String> {
        let list = http_get(port, "/json")?;
        let v: Value = serde_json::from_str(&list).map_err(|e| format!("/json: {e}: {}", list.chars().take(200).collect::<String>()))?;
        let pages: Vec<&Value> = v.as_array().map(|a| a.iter().filter(|t| t["type"] == "page").collect()).unwrap_or_default();
        let pick = match target {
            Some(s) => pages.iter().find(|t| t["url"].as_str().unwrap_or("").contains(s) || t["title"].as_str().unwrap_or("").contains(s)).copied(),
            None => pages.first().copied(),
        };
        let page = pick.ok_or_else(|| format!("no page target{} among {} page(s)", target.map(|t| format!(" matching {t:?}")).unwrap_or_default(), pages.len()))?;
        let url = page["webSocketDebuggerUrl"].as_str().ok_or("target without webSocketDebuggerUrl")?;
        let ws = ws::WebSocket::connect(url, Duration::from_secs(60))?;
        Ok(Cdp { ws, next: 1 })
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        let msg = json!({"id": id, "method": method, "params": params});
        self.ws.send_text(&msg.to_string())?;
        loop {
            let text = self.ws.recv_text()?;
            let v: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if v["id"] == json!(id) {
                if let Some(err) = v.get("error") {
                    return Err(format!("{method}: {err}"));
                }
                return Ok(v["result"].clone());
            }
            // events and other ids are dropped
        }
    }

    fn eval(&mut self, expr: &str) -> Result<Value, String> {
        let r = self.call("Runtime.evaluate", json!({"expression": expr, "returnByValue": true, "awaitPromise": true}))?;
        if let Some(ex) = r.get("exceptionDetails") {
            return Err(format!("JS exception: {}", ex["exception"]["description"].as_str().unwrap_or(&ex.to_string())));
        }
        Ok(r["result"]["value"].clone())
    }

    fn node(&mut self, selector: &str) -> Result<i64, String> {
        let doc = self.call("DOM.getDocument", json!({"depth": 0}))?;
        let root = doc["root"]["nodeId"].as_i64().ok_or("DOM.getDocument: no root")?;
        let q = self.call("DOM.querySelector", json!({"nodeId": root, "selector": selector}))?;
        match q["nodeId"].as_i64() {
            Some(n) if n > 0 => Ok(n),
            _ => Err(format!("no element matches {selector:?}")),
        }
    }

    fn wait_ready(&mut self, wait_s: u64) -> Result<String, String> {
        let t0 = std::time::Instant::now();
        loop {
            let st = self.eval("document.readyState + ' ' + location.href").unwrap_or(Value::Null);
            let s = st.as_str().unwrap_or("").to_string();
            if s.starts_with("complete ") {
                return Ok(s[9..].to_string());
            }
            if t0.elapsed().as_secs() > wait_s {
                return Err(format!("page not complete after {wait_s} s ({s})"));
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}

fn b64dec(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn run(args: &[String]) -> Result<(), String> {
    let port: u16 = flag(args, "--port").and_then(|p| p.parse().ok()).unwrap_or(9222);
    let target = flag(args, "--target");
    let cmd = args.first().map(String::as_str).unwrap_or("");
    match cmd {
        "targets" => {
            let list = http_get(port, "/json")?;
            let v: Value = serde_json::from_str(&list).map_err(|e| e.to_string())?;
            for t in v.as_array().unwrap_or(&vec![]) {
                println!("{}\t{}\t{}", t["type"].as_str().unwrap_or(""), t["title"].as_str().unwrap_or(""), t["url"].as_str().unwrap_or(""));
            }
            Ok(())
        }
        "eval" => {
            let expr = flag(args, "--expr").ok_or("eval needs --expr JS")?;
            let mut c = Cdp::attach(port, target.as_deref())?;
            let v = c.eval(&expr)?;
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            Ok(())
        }
        "nav" => {
            let url = flag(args, "--url").ok_or("nav needs --url")?;
            let wait: u64 = flag(args, "--wait-s").and_then(|w| w.parse().ok()).unwrap_or(60);
            let mut c = Cdp::attach(port, target.as_deref())?;
            c.call("Page.navigate", json!({"url": url}))?;
            std::thread::sleep(Duration::from_millis(500));
            let at = c.wait_ready(wait)?;
            println!("{at}");
            Ok(())
        }
        "html" => {
            let mut c = Cdp::attach(port, target.as_deref())?;
            let expr = match flag(args, "--selector") {
                Some(sel) => format!("(document.querySelector({}) || {{outerHTML: ''}}).outerHTML", serde_json::to_string(&sel).unwrap()),
                None => "document.documentElement.outerHTML".to_string(),
            };
            let v = c.eval(&expr)?;
            println!("{}", v.as_str().unwrap_or(""));
            Ok(())
        }
        "setfile" => {
            let sel = flag(args, "--selector").ok_or("setfile needs --selector CSS")?;
            let file = flag(args, "--file").ok_or("setfile needs --file WINPATH")?;
            let mut c = Cdp::attach(port, target.as_deref())?;
            let n = c.node(&sel)?;
            c.call("DOM.setFileInputFiles", json!({"files": [file], "nodeId": n}))?;
            println!("set {file} on {sel}");
            Ok(())
        }
        "shot" => {
            let out = flag(args, "--out").ok_or("shot needs --out FILE.png")?;
            let mut c = Cdp::attach(port, target.as_deref())?;
            let r = c.call("Page.captureScreenshot", json!({"format": "png"}))?;
            let data = r["data"].as_str().ok_or("no screenshot data")?;
            let bytes = b64dec(data);
            std::fs::write(&out, &bytes).map_err(|e| format!("{out}: {e}"))?;
            println!("{out}: {} B", bytes.len());
            Ok(())
        }
        "click" => {
            let sel = flag(args, "--selector").ok_or("click needs --selector CSS")?;
            let mut c = Cdp::attach(port, target.as_deref())?;
            let js = format!("(function(){{const e=document.querySelector({}); if(!e) return null; e.scrollIntoView({{block:'center'}}); const r=e.getBoundingClientRect(); return [r.left+r.width/2, r.top+r.height/2];}})()", serde_json::to_string(&sel).unwrap());
            let p = c.eval(&js)?;
            let (x, y) = match p.as_array() {
                Some(a) if a.len() == 2 => (a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)),
                _ => return Err(format!("no element matches {sel:?}")),
            };
            for (t, n) in [("mouseMoved", 0), ("mousePressed", 1), ("mouseReleased", 1)] {
                c.call("Input.dispatchMouseEvent", json!({"type": t, "x": x, "y": y, "button": "left", "clickCount": n}))?;
            }
            println!("clicked {sel} at {x:.0},{y:.0}");
            Ok(())
        }
        "type" => {
            let sel = flag(args, "--selector").ok_or("type needs --selector CSS")?;
            // `--text-file F`: the text from a file (a secret never rides a command line)
            let text = match flag(args, "--text-file") {
                Some(f) => std::fs::read_to_string(&f).map_err(|e| format!("{f}: {e}"))?.trim_end_matches(['\r', '\n']).to_string(),
                None => flag(args, "--text").ok_or("type needs --text T or --text-file F")?,
            };
            let mut c = Cdp::attach(port, target.as_deref())?;
            let js = format!("(function(){{const e=document.querySelector({}); if(!e) return false; e.focus(); e.select && e.select(); return true;}})()", serde_json::to_string(&sel).unwrap());
            if c.eval(&js)? != json!(true) {
                return Err(format!("no element matches {sel:?}"));
            }
            c.call("Input.insertText", json!({"text": text}))?;
            println!("typed {} chars into {sel}", text.chars().count());
            Ok(())
        }
        _ => Err("usage: tmxup targets | eval --expr JS | nav --url U [--wait-s N] | html [--selector CSS] | setfile --selector CSS --file WINPATH | shot --out FILE.png | click --selector CSS | type --selector CSS --text T   [--port 9222] [--target SUBSTR]".into()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("tmxup: {e}");
        std::process::exit(2);
    }
}
