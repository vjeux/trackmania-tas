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
//!   tmxup upload --map WINPATH --name N [--difficulty 0-5] [--routes 0-2] --tags ID,ID,ID
//!                --comments-file F [--unlisted] [--dry-run]   the whole form for one map
//!   tmxup comments --id MAPID --comments-file F    re-set an uploaded map's comments
//!   tmxup listed --id MAPID                        switch an uploaded map from unlisted to listed
//!   tmxup mappack --name N --ids-file F [--type 0] [--description-file F] [--tags ID,…] [--dry-run]
//!   tmxup batch --playlist P.json --sheet S.csv --maps-dir DIR --alteration Tiny --done D.tsv
//!               [--surface-tag ID] [--also-tag ID] [--difficulty N] [--limit N] [--unlisted] [--dry-run] [--not-agent-built]
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

    fn set_comments(&mut self, comments: &str) -> Result<(), String> {
        self.set_editor(".bbeditor .cm-content", comments)
    }

    /// Replace the text of a CodeMirror bbcode editor (`sel` = its `.cm-content`).
    fn set_editor(&mut self, sel: &str, comments: &str) -> Result<(), String> {
        // Select the editor's whole content with a DOM Range (a synthetic Ctrl+A and
        // execCommand('selectAll') select nothing inside CodeMirror); the first insertText
        // then REPLACES it — the append of the 22:26 pass on 367061 came from that.
        // CodeMirror restores its own selection when focus arrives by .focus(): a real
        // click first (focus + cursor), THEN the Range — CM's selectionchange handler
        // adopts a DOM selection made while it already has the focus.
        let p = self.eval(&format!("(function(){{const e=document.querySelector({sel}); if(!e) return null; e.scrollIntoView({{block:'center'}}); const r=e.getBoundingClientRect(); return [r.left+Math.min(r.width/2, 200), r.top+Math.min(r.height/2, 20)];}})()", sel = serde_json::to_string(sel).unwrap()))?;
        let (x, y) = match p.as_array() {
            Some(a) if a.len() == 2 => (a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)),
            _ => return Err("no CodeMirror comments editor (.bbeditor .cm-content) on the page".into()),
        };
        for (t, n) in [("mouseMoved", 0), ("mousePressed", 1), ("mouseReleased", 1)] {
            self.call("Input.dispatchMouseEvent", json!({"type": t, "x": x, "y": y, "button": "left", "clickCount": n}))?;
        }
        std::thread::sleep(Duration::from_millis(200));
        let selected = self.eval(&format!("(function(){{const e=document.querySelector({sel}); const r=document.createRange(); r.selectNodeContents(e); const s=window.getSelection(); s.removeAllRanges(); s.addRange(r); return s.toString().length;}})()", sel = serde_json::to_string(sel).unwrap()))?;
        let have = self.eval(&format!("document.querySelector({}).innerText.trim().length", serde_json::to_string(sel).unwrap()))?;
        eprintln!("  comments editor: {} chars present, {} selected", have, selected);
        if comments.trim().is_empty() {
            self.call("Input.insertText", json!({"text": " "}))?;
        }
        // one insertText per line, Enter between them (CodeMirror splits lines on Enter)
        for (i, line) in comments.split('\n').enumerate() {
            if i > 0 {
                self.call("Input.dispatchKeyEvent", json!({"type": "keyDown", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "text": "\r"}))?;
                self.call("Input.dispatchKeyEvent", json!({"type": "keyUp", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13}))?;
            }
            if !line.is_empty() {
                self.call("Input.insertText", json!({"text": line}))?;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
        let shown = self.eval(&format!("document.querySelector({}).innerText.length", serde_json::to_string(sel).unwrap()))?;
        eprintln!("  editor {sel}: {} chars (the textarea syncs on submit)", shown);
        Ok(())
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

struct UploadSpec {
    map: String,
    name: String,
    difficulty: String,
    routes: String,
    tags: Vec<String>,
    comments: String,
    unlisted: bool,
}

/// The whole upload form for one map; `Ok(Some(url))` is the map page it landed on,
/// `Ok(None)` a dry run.
/// Set a map's tags the only way that survives TMX's DropdownFilter (2026-10-31): navigate
/// to its edit form and re-POST it via fetch() with a FormData whose `Tags[i]` entries we
/// append ourselves. The native widget blanks directly-set hidden inputs on submit; a
/// programmatic POST skips the widget. Verifies via the read API and retries once.
fn force_tags(c: &mut Cdp, id: u64, tags: &[String]) -> Result<(), String> {
    let want: Vec<String> = tags.iter().filter(|t| !t.trim().is_empty()).cloned().collect();
    if want.is_empty() {
        return Ok(());
    }
    for attempt in 0..2 {
        c.call("Page.navigate", json!({"url": format!("https://trackmania.exchange/mapupload/{id}")}))?;
        std::thread::sleep(Duration::from_millis(1500));
        c.wait_ready(30)?;
        // build FormData from the edit form, drop every Tags[*] it carries, append ours
        let appends: String = want.iter().enumerate().map(|(i, t)| format!("fd.append('Tags[{i}]', {});", serde_json::to_string(t).unwrap())).collect();
        let js = format!(
            "(async function(){{var f=document.querySelector('#mapUploadForm'); if(!f) return 'noform'; var fd=new FormData(f);              [...fd.keys()].filter(function(k){{return k.indexOf('Tags[')===0}}).forEach(function(k){{fd.delete(k)}}); {appends}              var r=await fetch(f.action,{{method:'POST',body:fd,redirect:'manual'}}); return 'status '+r.status+' '+r.type;}})()"
        );
        let res = c.eval(&js)?;
        let _ = res;
        std::thread::sleep(Duration::from_millis(800));
        // verify
        let got = c.eval(&format!("fetch('https://trackmania.exchange/api/maps?id={id}&fields=Tags').then(function(r){{return r.text()}})"))?;
        let got = got.as_str().unwrap_or("");
        let ok = want.iter().all(|t| {
            let needle = format!("\"TagId\":{t},");
            let needle2 = format!("\"TagId\":{t}}}");
            got.contains(&needle) || got.contains(&needle2)
        });
        if ok {
            println!("  tags set via POST: {} on {id}", want.join(","));
            return Ok(());
        }
        if attempt == 1 {
            return Err(format!("tags not confirmed on {id} after 2 tries; want {:?} got {got}", want));
        }
    }
    Ok(())
}

fn upload_one(c: &mut Cdp, u: &UploadSpec, dry: bool) -> Result<Option<String>, String> {
    c.call("Page.navigate", json!({"url": "https://trackmania.exchange/mapupload"}))?;
    std::thread::sleep(Duration::from_millis(800));
    let at = c.wait_ready(60)?;
    if !at.contains("/mapupload") {
        return Err(format!("not on the upload form but at {at} — logged out?"));
    }
    // the map file (the page reads the Gbx and prefills name/length/author)
    let n = c.node("#mapUploadForm input[type=file][accept='.map.gbx']")?;
    c.call("DOM.setFileInputFiles", json!({"files": [u.map], "nodeId": n}))?;
    std::thread::sleep(Duration::from_millis(2500));
    let prefilled = c.eval("document.querySelector('#TrackName').value")?;
    eprintln!("  file set; TrackName prefilled: {prefilled}");
    c.eval(&format!("(function(){{const e=document.querySelector('#TrackName'); e.value={}; e.dispatchEvent(new Event('input',{{bubbles:true}})); e.dispatchEvent(new Event('change',{{bubbles:true}})); return e.value;}})()", serde_json::to_string(&u.name).unwrap()))?;
    for (sel, val) in [("#Difficulty", &u.difficulty), ("#Routes", &u.routes)] {
        c.eval(&format!("(function(){{const e=document.querySelector('{sel}'); e.value={}; e.dispatchEvent(new Event('change',{{bubbles:true}})); return e.value;}})()", serde_json::to_string(val).unwrap()))?;
    }
    // tags: one hidden Tags[i] per tag — the + button adds a row
    for _ in 1..u.tags.len() {
        c.eval("document.querySelector('#trackTagAdd').click(); 1")?;
        std::thread::sleep(Duration::from_millis(300));
    }
    for (i, t) in u.tags.iter().enumerate() {
        let set = c.eval(&format!("(function(){{const e=document.getElementById('Tags[{i}]'); if(!e) return 'missing'; e.value={}; e.dispatchEvent(new Event('change',{{bubbles:true}})); return e.value;}})()", serde_json::to_string(t).unwrap()))?;
        if set.as_str() != Some(t.as_str()) {
            return Err(format!("Tags[{i}] did not take {t}: {set}"));
        }
    }
    // the comments: a CodeMirror 6 editor (`.cm-content`, contenteditable) that syncs to the
    // hidden #AuthorComments textarea on submit — only typed text reaches its state (a value
    // set on the textarea was dropped: map 367061 first came out with "(No text defined)")
    c.set_comments(&u.comments)?;
    if u.unlisted {
        c.eval("(function(){const e=document.querySelector('#Unlisted'); if(!e.checked){e.click();} return e.checked;})()")?;
    }
    let summary = c.eval("JSON.stringify({name:document.querySelector('#TrackName').value, len:document.querySelector('#MapLengthFormatted').value, difficulty:document.querySelector('#Difficulty').value, routes:document.querySelector('#Routes').value, tags:[...document.querySelectorAll('#trackTagList input[type=hidden]')].map(e=>e.value), unlisted:document.querySelector('#Unlisted').checked, visible:document.querySelector('#Visible').checked, comments:document.querySelector('.bbeditor .cm-content').innerText.length, errors:[...document.querySelectorAll('.field-validation-error')].map(e=>e.innerText).filter(Boolean)})")?;
    let sum = summary.as_str().unwrap_or("").to_string();
    println!("form: {sum}");
    if sum.contains("\"errors\":[\"") {
        return Err(format!("form validation errors: {sum}"));
    }
    if dry {
        println!("dry run: not submitted");
        return Ok(None);
    }
    let before = c.eval("location.href")?.as_str().unwrap_or("").to_string();
    c.eval("document.querySelector('#mapUploadForm button[type=submit]').click(); 1")?;
    let t0 = std::time::Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(1000));
        let h = c.eval("document.readyState + ' ' + location.href").unwrap_or(Value::Null).as_str().unwrap_or("").to_string();
        if h.starts_with("complete ") && !h.ends_with(&before) && !h.contains("/mapupload") {
            let url = h[9..].to_string();
            if !url.contains("/mapshow/") {
                return Err(format!("submitted but landed on {url}"));
            }
            // the page's own check: the name and that it is ours
            let title = c.eval("document.title")?.as_str().unwrap_or("").to_string();
            if !title.starts_with(&u.name) {
                return Err(format!("landed on {url} but its title is {title:?}, not {:?}", u.name));
            }
            // TAGS: the DropdownFilter widget blanks any Tags[i] hidden input that was not
            // picked through its own UI before the native submit (TMX changed it after
            // 2026-10-22; direct-set + native submit now lands Mini-only). So set the tags
            // authoritatively by RE-POSTing the map's edit form through fetch() with a
            // FormData whose Tags[] we control — the widget never runs on a programmatic
            // POST. Verified 2026-10-31 on maps 368922/368923. Idempotent; retried once.
            if let Some(id) = url.rsplit('/').next().and_then(|x| x.parse::<u64>().ok()) {
                force_tags(c, id, &u.tags)?;
            }
            return Ok(Some(url));
        }
        if t0.elapsed().as_secs() > 180 {
            let errs = c.eval("JSON.stringify([...document.querySelectorAll('.field-validation-error, .validation-summary-errors, .alert-danger')].map(e=>e.innerText).filter(Boolean))").unwrap_or(Value::Null);
            return Err(format!("no new page after 180 s (still at {h}); form errors: {errs}"));
        }
    }
}

/// One line of a CSV with quoted fields.
fn csv_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if q && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => q = !q,
            ',' if !q => out.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    out.push(cur);
    out
}

/// The TMX tag id a word of Everios96's description sheet stands for (None: not a tag).
fn sheet_tag(word: &str) -> Option<u32> {
    Some(match word {
        "Dirt" | "Dirt_Road" | "Penalty_Sand" => 15,
        "Grass" | "Penalty_Grass" => 33,
        "Ice" | "Snow" => 14,
        "Plastic" | "Plastic_Bounce" => 39,
        "Magnet" => 66,
        "Wood" => 51,
        "Water" | "Wet_Wheels" => 38,
        "Reactor_Up" | "Reactor_Down" | "Orange_Reactor" | "Yellow_Reactor" => 17,
        "Bob" => 44,
        "Sausage" => 43,
        "Bugslide" => 56,
        "Platform" => 18,
        "Tech" => 3,
        "Fullspeed" => 2,
        "LOL" => 5,
        "Slow-Mo" | "Quantum" => 19,
        "Rally" => 54,
        "Desert" => 59,
        "Mixed" => 27,
        "Precision" => 68,
        "Speed_Slide" => 29,
        "Wallride" => 41,
        _ => return None,
    })
}

/// Everios96's rule: Mini, then up to two of the sheet's words as TMX tags, Race when the
/// sheet knows nothing. `surface` (a surface alteration's tag id) takes the second slot and
/// drops the sheet's surface words — the surface changed.
fn tags_for(sheet_row: Option<&[String]>, surface: Option<u32>) -> Vec<u32> {
    tags_for_with(sheet_row, surface, None)
}

/// `also` (--also-tag): a tag every map carries, right after Mini — TMX staff + Everios96
/// agreed on 2026-11-07 that every altered map is tagged "Altered Community" (73); it takes
/// the second slot, the surface tag the third, and the sheet's words fill what is left.
fn tags_for_with(sheet_row: Option<&[String]>, surface: Option<u32>, also: Option<u32>) -> Vec<u32> {
    let mut out = vec![25u32];
    if let Some(a) = also {
        if !out.contains(&a) {
            out.push(a);
        }
    }
    if let Some(s) = surface {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    if let Some(r) = sheet_row {
        for w in r.iter().skip(3) {
            let w = w.trim();
            if let Some(t) = sheet_tag(w) {
                let is_surface = matches!(t, 15 | 33 | 14 | 39 | 66 | 51);
                if surface.is_some() && is_surface {
                    continue;
                }
                if !out.contains(&t) {
                    out.push(t);
                }
            }
            // Everios96's rule: two slots beyond Mini (+ the Altered tag) — `== 3` let every
            // sheet word through once `also` made the list 3 long before the loop (Icy 63 got
            // 6 tags and TMX refused the 6th, 2026-11-09)
            if out.len() >= 3 + usize::from(also.is_some()) {
                break;
            }
        }
    }
    if out.len() == 1 {
        out.push(1);
    }
    out
}

fn description(alteration: &str, num: &str, orig_link: &str, hunter: &str, agent_built: bool) -> String {
    let orig = if orig_link.is_empty() { format!("U10S_{num}") } else { format!("[url={orig_link}]U10S_{num}[/url]") };
    // `agent_built` false (`--not-agent-built`): a map the community altered by hand (the
    // Altered club's part 1/2 sets of 2026-10-31) — no AI-agent line, nothing else differs
    let head = if agent_built { "This map was created using vjeux's AI agent (automated alteration tool).\n\n" } else { "" };
    format!(
        "{head}{alteration} alteration of U10S_{num} from the Under 10 Seconds maps by Everios96. Original map: {orig}\n\nAuthor times hunted by the Everios96 Community; the AT on this map was set by: {hunter}\n\nAll released maps are playable in the Altered U10S club. Discord for the club: https://discord.gg/zNbJN5sH5D"
    )
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
        // The whole form for ONE map, then the page it lands on. Every value is a flag
        // (the comments from a file: bbcode with newlines); the tags are TMX tag ids.
        "upload" => {
            let map = flag(args, "--map").ok_or("upload needs --map WINPATH (.Map.Gbx)")?;
            let name = flag(args, "--name").ok_or("upload needs --name")?;
            let difficulty = flag(args, "--difficulty").unwrap_or_else(|| "1".into());
            let routes = flag(args, "--routes").unwrap_or_else(|| "0".into());
            let tags: Vec<String> = flag(args, "--tags").map(|t| t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
            let comments = match flag(args, "--comments-file") {
                Some(f) => std::fs::read_to_string(&f).map_err(|e| format!("{f}: {e}"))?,
                None => flag(args, "--comments").unwrap_or_default(),
            };
            let unlisted = args.iter().any(|a| a == "--unlisted");
            let dry = args.iter().any(|a| a == "--dry-run");
            let mut c = Cdp::attach(port, target.as_deref())?;
            let landed = upload_one(&mut c, &UploadSpec { map, name, difficulty, routes, tags, comments, unlisted }, dry)?;
            if let Some(url) = landed {
                println!("landed: {url}");
            }
            Ok(())
        }
        // Re-set the comments of an uploaded map through its edit form (/mapupload/<id>).
        "comments" => {
            let id = flag(args, "--id").ok_or("comments needs --id MAPID")?;
            let comments = match flag(args, "--comments-file") {
                Some(f) => std::fs::read_to_string(&f).map_err(|e| format!("{f}: {e}"))?,
                None => flag(args, "--comments").ok_or("comments needs --comments-file F")?,
            };
            let mut c = Cdp::attach(port, target.as_deref())?;
            c.call("Page.navigate", json!({"url": format!("https://trackmania.exchange/mapupload/{id}")}))?;
            std::thread::sleep(Duration::from_millis(800));
            let at = c.wait_ready(60)?;
            if !at.contains(&format!("/mapupload/{id}")) {
                return Err(format!("not on the edit form but at {at}"));
            }
            c.set_comments(&comments)?;
            c.eval("document.querySelector('#mapUploadForm button[type=submit]').click(); 1")?;
            let t0 = std::time::Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(1000));
                let h = c.eval("document.readyState + ' ' + location.href").unwrap_or(Value::Null).as_str().unwrap_or("").to_string();
                if h.starts_with("complete ") && !h.contains("/mapupload") {
                    println!("landed: {}", &h[9..]);
                    break;
                }
                if t0.elapsed().as_secs() > 120 {
                    let errs = c.eval("JSON.stringify([...document.querySelectorAll('.field-validation-error, .validation-summary-errors, .alert-danger')].map(e=>e.innerText).filter(Boolean))").unwrap_or(Value::Null);
                    return Err(format!("no new page after 120 s (still at {h}); form errors: {errs}"));
                }
            }
            let shown = c.eval("(document.querySelector('.authorcomments, [class*=authorcomment], .bbcode') || {innerText: ''}).innerText.slice(0, 400)")?;
            println!("comments shown: {}", shown.as_str().unwrap_or(""));
            Ok(())
        }
        // A whole campaign folder: the trackmania.io playlist JSON (name, mapUid, filename,
        // authorplayer.name), the sheet CSV for the tags, the files in --maps-dir; one map
        // per submission, each landing appended to --done (num, uid, tmx url) so a rerun
        // skips it; the first error stops the batch.
        "batch" => {
            let playlist = flag(args, "--playlist").ok_or("batch needs --playlist playlist.json")?;
            let sheet = flag(args, "--sheet").ok_or("batch needs --sheet u10s-tags.csv")?;
            let maps_dir = flag(args, "--maps-dir").ok_or("batch needs --maps-dir DIR")?;
            let agent_built = !args.iter().any(|a| a == "--not-agent-built");
            let alteration = flag(args, "--alteration").ok_or("batch needs --alteration Tiny|Dirt|…")?;
            let surface: Option<u32> = flag(args, "--surface-tag").and_then(|t| t.parse().ok());
            let also: Option<u32> = flag(args, "--also-tag").and_then(|t| t.parse().ok());
            let done_path = flag(args, "--done").ok_or("batch needs --done DONE.tsv")?;
            let difficulty = flag(args, "--difficulty").unwrap_or_else(|| "1".into());
            let limit: usize = flag(args, "--limit").and_then(|l| l.parse().ok()).unwrap_or(usize::MAX);
            let unlisted = args.iter().any(|a| a == "--unlisted");
            let dry = args.iter().any(|a| a == "--dry-run");
            let rows: Value = serde_json::from_str(&std::fs::read_to_string(&playlist).map_err(|e| format!("{playlist}: {e}"))?).map_err(|e| format!("{playlist}: {e}"))?;
            let rows = rows.as_array().ok_or("playlist: not a JSON array")?;
            let sheet_rows: std::collections::HashMap<String, Vec<String>> = std::fs::read_to_string(&sheet)
                .map_err(|e| format!("{sheet}: {e}"))?
                .lines()
                .map(csv_fields)
                .filter(|f| f.first().map(|x| x.starts_with("U10S_")).unwrap_or(false))
                .map(|f| (f[0].trim_start_matches("U10S_").trim_start_matches('0').to_string(), f))
                .collect();
            let done_text = std::fs::read_to_string(&done_path).unwrap_or_default();
            let done: std::collections::HashSet<String> = done_text.lines().filter_map(|l| l.split('\t').nth(1).map(String::from)).collect();
            let mut c = Cdp::attach(port, target.as_deref())?;
            let mut n = 0usize;
            for r in rows {
                let uid = r["mapUid"].as_str().unwrap_or("").to_string();
                let name = r["name"].as_str().unwrap_or("").to_string();
                if done.contains(&uid) {
                    continue;
                }
                if n >= limit {
                    break;
                }
                let num = name.strip_prefix("U10S_").and_then(|x| x.split(' ').next()).unwrap_or("").trim_start_matches('0').to_string();
                let hunter = r["authorplayer"]["name"].as_str().unwrap_or("").to_string();
                let file = r["filename"].as_str().unwrap_or("").to_string();
                let row = sheet_rows.get(&num).map(|v| v.as_slice());
                let tags: Vec<String> = tags_for_with(row, surface, also).iter().map(|t| t.to_string()).collect();
                let link = row.and_then(|f| f.get(1)).cloned().unwrap_or_default();
                let spec = UploadSpec {
                    map: format!("{}\\{}", maps_dir.trim_end_matches(['\\', '/']), file),
                    name: name.clone(),
                    difficulty: difficulty.clone(),
                    routes: "0".into(),
                    tags: tags.clone(),
                    comments: description(&alteration, &num, &link, &hunter, agent_built),
                    unlisted,
                };
                println!("[{}] {name} tags {} hunter {hunter}", n + 1, tags.join(","));
                match upload_one(&mut c, &spec, dry)? {
                    Some(url) => {
                        println!("  -> {url}");
                        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&done_path).map_err(|e| format!("{done_path}: {e}"))?;
                        writeln!(f, "{num}\t{uid}\t{url}\t{}", tags.join(",")).map_err(|e| e.to_string())?;
                    }
                    None => {}
                }
                n += 1;
                std::thread::sleep(Duration::from_secs(2));
            }
            println!("batch: {n} map(s) this run; {} already done before", done.len());
            Ok(())
        }
        // Flip an uploaded map to listed (the edit form's Unlisted switch off).
        "listed" => {
            let id = flag(args, "--id").ok_or("listed needs --id MAPID")?;
            let mut c = Cdp::attach(port, target.as_deref())?;
            c.call("Page.navigate", json!({"url": format!("https://trackmania.exchange/mapupload/{id}")}))?;
            std::thread::sleep(Duration::from_millis(800));
            c.wait_ready(60)?;
            let was = c.eval("(function(){const e=document.querySelector('#Unlisted'); const was=e.checked; if(e.checked){e.click();} return was;})()")?;
            println!("unlisted was {was}, now off");
            c.eval("document.querySelector('#mapUploadForm button[type=submit]').click(); 1")?;
            let t0 = std::time::Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(1000));
                let h = c.eval("document.readyState + ' ' + location.href").unwrap_or(Value::Null).as_str().unwrap_or("").to_string();
                if h.starts_with("complete ") && !h.contains("/mapupload") {
                    println!("landed: {}", &h[9..]);
                    break;
                }
                if t0.elapsed().as_secs() > 120 {
                    return Err(format!("no new page after 120 s (still at {h})"));
                }
            }
            let still = c.eval("!!document.body.innerText.match(/Map is unlisted/)")?;
            println!("page says unlisted: {still}");
            Ok(())
        }
        // A mappack from uploaded maps: /mappackcreate — name, type, the maps added one by
        // one through the page's own "enter MapId → Add" (it looks each one up), the
        // description editor, Mini tag, submit; lands on /mappackshow/<id>.
        "mappack" => {
            let name = flag(args, "--name").ok_or("mappack needs --name")?;
            let ids_file = flag(args, "--ids-file").ok_or("mappack needs --ids-file F (one TMX map id per line, or a done.tsv whose 3rd column is the map url)")?;
            let mtype = flag(args, "--type").unwrap_or_else(|| "0".into());
            let desc = match flag(args, "--description-file") {
                Some(f) => std::fs::read_to_string(&f).map_err(|e| format!("{f}: {e}"))?,
                None => String::new(),
            };
            let tags: Vec<String> = flag(args, "--tags").map(|t| t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
            let dry = args.iter().any(|a| a == "--dry-run");
            let ids: Vec<String> = std::fs::read_to_string(&ids_file)
                .map_err(|e| format!("{ids_file}: {e}"))?
                .lines()
                .filter_map(|l| {
                    let l = l.trim();
                    if l.is_empty() {
                        return None;
                    }
                    let f: Vec<&str> = l.split('\t').collect();
                    let cand = if f.len() >= 3 { f[2] } else { f[0] };
                    cand.rsplit('/').next().map(|x| x.trim().to_string()).filter(|x| x.chars().all(|c| c.is_ascii_digit()) && !x.is_empty())
                })
                .collect();
            if ids.is_empty() {
                return Err("no map ids in the ids file".into());
            }
            let mut c = Cdp::attach(port, target.as_deref())?;
            c.call("Page.navigate", json!({"url": "https://trackmania.exchange/mappackcreate"}))?;
            std::thread::sleep(Duration::from_millis(800));
            let at = c.wait_ready(60)?;
            if !at.contains("/mappackcreate") {
                return Err(format!("not on the mappack form but at {at} — logged out?"));
            }
            std::thread::sleep(Duration::from_millis(2000));
            c.eval(&format!("(function(){{const e=document.querySelector('#Name'); e.value={}; e.dispatchEvent(new Event('input',{{bubbles:true}})); e.dispatchEvent(new Event('change',{{bubbles:true}})); return e.value;}})()", serde_json::to_string(&name).unwrap()))?;
            c.eval(&format!("(function(){{const e=document.querySelector('#MappackType'); e.value={}; e.dispatchEvent(new Event('change',{{bubbles:true}})); return e.value;}})()", serde_json::to_string(&mtype).unwrap()))?;
            let input_sel = "input[placeholder=\"Search by name or enter MapId\"]";
            for (i, id) in ids.iter().enumerate() {
                // the page reads the id from real key input (a value set by script + an
                // input event added nothing): focus + select, insert as typing, then Add
                // a real click into the field, the id as typed text, then the page's lookup
                // needs a moment (a debounced search) before Add uses its result
                let p = c.eval(&format!("(function(){{const e=document.querySelector('{input_sel}'); e.scrollIntoView({{block:'center'}}); e.value=''; const r=e.getBoundingClientRect(); return [r.left+r.width/2, r.top+r.height/2];}})()"))?;
                let (x, y) = match p.as_array() {
                    Some(a) if a.len() == 2 => (a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)),
                    _ => return Err("no map-id input on the mappack form".into()),
                };
                for (t, n) in [("mouseMoved", 0), ("mousePressed", 1), ("mouseReleased", 1)] {
                    c.call("Input.dispatchMouseEvent", json!({"type": t, "x": x, "y": y, "button": "left", "clickCount": n}))?;
                }
                // the explicit focus + select after the click is what the working CLI
                // sequence (click, type, Add) did; without it the typed id was ignored
                // the page looks the id up on input and Add uses the answer; a slow or
                // missed lookup leaves Add a no-op — so: type, wait, Add, and when nothing
                // lands within 4 s type it again (up to 5 rounds)
                let t0 = std::time::Instant::now();
                let mut round = 0;
                loop {
                    c.eval(&format!("(function(){{const e=document.querySelector('{input_sel}'); e.focus(); e.select && e.select(); e.value=''; return 1;}})()"))?;
                    c.call("Input.insertText", json!({"text": id}))?;
                    std::thread::sleep(Duration::from_millis(3000));
                    c.eval(&format!("(function(){{const inp=document.querySelector('{input_sel}'); inp.nextElementSibling.click(); return 1;}})()"))?;
                    let t1 = std::time::Instant::now();
                    let mut landed = false;
                    while t1.elapsed().as_secs() < 4 {
                        std::thread::sleep(Duration::from_millis(400));
                        let has = c.eval(&format!("document.querySelector('#MapEntries').value.includes('\"MapId\":{id},')"))?;
                        if has == json!(true) {
                            landed = true;
                            break;
                        }
                    }
                    if landed {
                        break;
                    }
                    round += 1;
                    if round >= 5 || t0.elapsed().as_secs() > 60 {
                        let diag = c.eval(&format!("JSON.stringify({{value: document.querySelector('{input_sel}').value, active: document.activeElement && (document.activeElement.tagName + '#' + document.activeElement.id + '.' + document.activeElement.className), entries: document.querySelector('#MapEntries').value.slice(0, 120), modals: [...document.querySelectorAll('.modal.show, .swal2-container, .featherlight')].map(e => e.innerText.slice(0, 100)), errs: [...document.querySelectorAll('.field-validation-error, .text-danger')].map(e => e.innerText).filter(Boolean)}})")).unwrap_or(Value::Null);
                        return Err(format!("map {id} was not added after {round} rounds (entry {}): {diag}", i + 1));
                    }
                }
                if (i + 1) % 10 == 0 {
                    eprintln!("  {} of {} maps added", i + 1, ids.len());
                }
            }
            if !desc.trim().is_empty() {
                c.set_editor(".bbeditor .cm-content", &desc)?;
            }
            for _ in 1..tags.len() {
                c.eval("document.querySelector('#trackTagAdd').click(); 1")?;
                std::thread::sleep(Duration::from_millis(300));
            }
            for (i, t) in tags.iter().enumerate() {
                c.eval(&format!("(function(){{const e=document.getElementById('Tags[{i}]'); if(!e) return 'missing'; e.value={}; e.dispatchEvent(new Event('change',{{bubbles:true}})); return e.value;}})()", serde_json::to_string(t).unwrap()))?;
            }
            let summary = c.eval("JSON.stringify({name:document.querySelector('#Name').value, type:document.querySelector('#MappackType').value, maps:JSON.parse(document.querySelector('#MapEntries').value).length, visible:document.querySelector('#TrackVisible').checked, tags:[...document.querySelectorAll('#trackTagList input[type=hidden]')].map(e=>e.value), errors:[...document.querySelectorAll('.field-validation-error')].map(e=>e.innerText).filter(Boolean)})")?;
            println!("form: {}", summary.as_str().unwrap_or(""));
            if dry {
                println!("dry run: not submitted");
                return Ok(());
            }
            c.eval("document.querySelector('form[enctype] button[type=submit]').click(); 1")?;
            let t0 = std::time::Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(1000));
                let h = c.eval("document.readyState + ' ' + location.href").unwrap_or(Value::Null).as_str().unwrap_or("").to_string();
                if h.starts_with("complete ") && !h.contains("/mappackcreate") {
                    println!("landed: {}", &h[9..]);
                    break;
                }
                if t0.elapsed().as_secs() > 120 {
                    let errs = c.eval("JSON.stringify([...document.querySelectorAll('.field-validation-error, .validation-summary-errors, .alert-danger')].map(e=>e.innerText).filter(Boolean))").unwrap_or(Value::Null);
                    return Err(format!("no new page after 120 s (still at {h}); form errors: {errs}"));
                }
            }
            let title = c.eval("document.title")?;
            println!("title: {}", title.as_str().unwrap_or(""));
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
