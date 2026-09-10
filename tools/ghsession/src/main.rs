//! `ghsession` — the uploader's own GitHub session, on the render box.
//!
//! ```text
//! ghsession seed --cookie-header FILE     ONE-TIME: adopt a fresh browser login as the uploader's session
//! ghsession status                        is the jar logged in? prints the login; exit 3 when it is not
//! ghsession keepalive                     status + ordinary page views (daily, under the ship lock)
//! ghsession upload FILE [--content-type T]   user-attachments upload; prints the asset URL
//! ```
//!
//! Why this exists (tools/tinyctl/box/UPLOADER-OWN-SESSION.md has the whole
//! story): every clip published so far rode a `Cookie:` header copied out of
//! vjeux's browser, and those died within the hour — killed by parallel
//! replays, by a curl jar that dropped session cookies, by a datacenter IP, and
//! by vjeux signing in again to mint the next one. A session does not need a
//! browser to live; it needs ONE client, one IP, no logins beside it, and a jar
//! that keeps every cookie the server sets. That jar is
//! `/home/vjeux/.gh-upload/session.json`; this binary is its only client, and
//! it is seeded once from a login that nothing else ever uses again.
//!
//! The upload is the protocol `ghvid.sh` proved (CSRF token off the README edit
//! page, `POST /upload/policies/assets`, the S3 form post, `PUT
//! /upload/assets/<id>`), with the same browser headers, followed by an
//! ordinary page view — what a person doing this by hand would generate.

mod jar;

use jar::Jar;
use std::path::{Path, PathBuf};

const HOST: &str = "github.com";
const REPO_URL: &str = "https://github.com/vjeux/trackmania-tas";
const EDIT_URL: &str = "https://github.com/vjeux/trackmania-tas/edit/main/README.md";
const REPO_ID: &str = "1338960733";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36";
const ACCEPT_HTML: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

fn jar_path() -> PathBuf {
    if let Ok(p) = std::env::var("GHSESSION_JAR") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
    PathBuf::from(home).join(".gh-upload/session.json")
}

fn machine() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// The one client. Every response's `Set-Cookie`s go into the jar and the jar
/// is saved before the response is looked at, so nothing GitHub tells the
/// session is ever lost, whatever happens to the command afterwards.
struct Client {
    agent: ureq::Agent,
    jar: Jar,
    path: PathBuf,
}

struct Reply {
    status: u16,
    location: Option<String>,
    body: Vec<u8>,
}

impl Reply {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// One multipart/form-data part: a text field or a file.
enum Part<'a> {
    Text(&'a str, &'a str),
    File { field: &'a str, name: &'a str, mime: &'a str, bytes: &'a [u8] },
}

fn multipart(parts: &[Part]) -> (String, Vec<u8>) {
    let boundary = format!("----ghsession{:x}{:x}", std::process::id(), jar::now_unix());
    let mut body = Vec::new();
    for p in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match p {
            Part::Text(k, v) => {
                body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n").as_bytes());
            }
            Part::File { field, name, mime, bytes } => {
                body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{field}\"; filename=\"{name}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
                body.extend_from_slice(bytes);
                body.extend_from_slice(b"\r\n");
            }
        }
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

impl Client {
    fn open(path: PathBuf) -> Result<Client, String> {
        let jar = Jar::load(&path)?;
        let here = machine();
        if jar.host != here && std::env::var("GHSESSION_ANY_HOST").is_err() {
            return Err(format!(
                "this jar was seeded on {:?} and this is {here:?}: the session is used from ONE machine only (a datacenter IP logged one out everywhere within minutes on 2026-09-08). Seed a jar here instead.",
                jar.host
            ));
        }
        Ok(Client { agent: agent(), jar, path })
    }

    /// GET or POST/PUT with the jar's cookies for the URL's host; redirects are
    /// NOT followed (a 302 to /login is the answer we want to see).
    fn send(&mut self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<(&str, &[u8])>) -> Result<Reply, String> {
        let (host, path) = host_path(url)?;
        let cookie = self.jar.header_for(&host, &path);
        fn dress<S>(mut r: ureq::RequestBuilder<S>, cookie: &str, headers: &[(&str, &str)]) -> ureq::RequestBuilder<S> {
            r = r.header("user-agent", UA);
            if !cookie.is_empty() {
                r = r.header("cookie", cookie);
            }
            for (k, v) in headers {
                r = r.header(*k, *v);
            }
            r
        }
        let resp = match (method, body) {
            ("GET", None) => dress(self.agent.get(url), &cookie, headers).call(),
            ("POST", Some((ct, bytes))) => dress(self.agent.post(url), &cookie, headers).header("content-type", ct).send(bytes),
            ("PUT", Some((ct, bytes))) => dress(self.agent.put(url), &cookie, headers).header("content-type", ct).send(bytes),
            _ => return Err(format!("{method} with{} a body", if body.is_some() { "" } else { "out" })),
        };
        let mut resp = resp.map_err(|e| format!("{method} {url}: {e}"))?;
        // every Set-Cookie into the jar, saved BEFORE anything else can fail
        let mut changed = false;
        for v in resp.headers().get_all("set-cookie").iter() {
            if let Ok(s) = v.to_str() {
                if self.jar.apply_set_cookie(s, &host) != jar::Applied::Ignored {
                    changed = true;
                }
            }
        }
        if changed {
            self.jar.save(&self.path)?;
        }
        let status = resp.status().as_u16();
        let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).map(String::from);
        let body = resp.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(|e| format!("{method} {url}: reading the body: {e}"))?;
        Ok(Reply { status, location, body })
    }

    fn get_page(&mut self, url: &str) -> Result<Reply, String> {
        self.send(
            "GET",
            url,
            &[
                ("accept", ACCEPT_HTML),
                ("accept-language", "en-US,en;q=0.9"),
                ("sec-fetch-site", "same-origin"),
                ("sec-fetch-mode", "navigate"),
                ("sec-fetch-dest", "document"),
                ("upgrade-insecure-requests", "1"),
            ],
            None,
        )
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(std::time::Duration::from_secs(1800)))
            .user_agent(UA)
            .build(),
    )
}

fn host_path(url: &str) -> Result<(String, String), String> {
    let rest = url.strip_prefix("https://").ok_or_else(|| format!("{url}: only https"))?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let path = path.split('?').next().unwrap_or("/");
    Ok((host.to_ascii_lowercase(), path.to_string()))
}

/// Who the page says is signed in: `<meta name="user-login" content="vjeux">`.
pub fn user_login(html: &str) -> Option<String> {
    let i = html.find("name=\"user-login\"")?;
    let tail = &html[i..];
    let j = tail.find("content=\"")? + "content=\"".len();
    let rest = &tail[j..];
    let k = rest.find('"')?;
    let s = &rest[..k];
    if s.is_empty() { None } else { Some(s.to_string()) }
}

/// The upload CSRF token off the README edit page: either the JSON blob
/// `"/upload/policies/assets":{"post":"<token>"}` or the classic form field.
pub fn upload_csrf_token(html: &str) -> Option<String> {
    let needle = "\"/upload/policies/assets\":{\"post\":\"";
    if let Some(i) = html.find(needle) {
        let rest = &html[i + needle.len()..];
        let j = rest.find('"')?;
        return Some(rest[..j].to_string());
    }
    let i = html.find("action=\"/upload/policies/assets\"")?;
    let rest = &html[i..];
    let j = rest.find("name=\"authenticity_token\"")?;
    let rest = &rest[j..];
    let k = rest.find("value=\"")? + "value=\"".len();
    let rest = &rest[k..];
    let l = rest.find('"')?;
    Some(rest[..l].to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("seed") => seed(&args[1..]),
        Some("status") => status(false),
        Some("keepalive") => status(true),
        Some("upload") => upload(&args[1..]),
        Some("-V") | Some("--version") => {
            println!("ghsession {} ({})", env!("CARGO_PKG_VERSION"), option_env!("TAS_BUILD").unwrap_or("dev"));
            Ok(0)
        }
        _ => {
            eprintln!(
                "ghsession -- the uploader's own GitHub session (tools/tinyctl/box/UPLOADER-OWN-SESSION.md)\n\n\
                 ghsession seed --cookie-header FILE       ONE-TIME: adopt a fresh browser login as the uploader's session\n\
                 ghsession status                          is the jar logged in? prints the login; exit 3 when not\n\
                 ghsession keepalive                       status + ordinary page views (daily, under the ship lock)\n\
                 ghsession upload FILE [--content-type T]  user-attachments upload; prints the asset URL\n\n\
                 jar: $GHSESSION_JAR or ~/.gh-upload/session.json (mode 600, this machine only)"
            );
            Ok(2)
        }
    };
    match r {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("ghsession: {e}");
            std::process::exit(1)
        }
    }
}

fn flag<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// The one-time step: a `Cookie:` header (one line, from a fresh private-window
/// login that is then closed WITHOUT signing out) becomes the jar, verified by
/// one page view, and the seed file is deleted — the header lives nowhere else.
fn seed(args: &[String]) -> Result<i32, String> {
    let file = flag(args, "--cookie-header").ok_or("seed needs --cookie-header FILE")?;
    let header = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let path = jar_path();
    if path.exists() && !args.iter().any(|a| a == "--replace") {
        return Err(format!("{} exists — `ghsession status` first; --replace to overwrite it with this seed", path.display()));
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let jar = Jar::from_cookie_header(&header, HOST, &machine());
    let names: Vec<&str> = jar.cookies.iter().map(|c| c.name.as_str()).collect();
    for want in ["user_session", "__Host-user_session_same_site", "logged_in"] {
        if !names.contains(&want) {
            return Err(format!("the header carries no {want} cookie — copy the whole `Cookie:` request header of a logged-in page (names seen: {})", names.join(" ")));
        }
    }
    jar.save(&path)?;
    let mut c = Client::open(path.clone())?;
    let r = c.get_page(REPO_URL)?;
    match (r.status, user_login(&r.text())) {
        (200, Some(u)) => {
            let _ = std::fs::remove_file(file);
            println!("seeded {} on {}: logged in as {u} ({} cookies); the seed file is deleted — close the browser window WITHOUT signing out", path.display(), c.jar.host, c.jar.cookies.len());
            Ok(0)
        }
        (s, _) => {
            let _ = std::fs::remove_file(&path);
            Err(format!("the header is not a live session: HTTP {s}{} — nothing kept", r.location.map(|l| format!(" → {l}")).unwrap_or_default()))
        }
    }
}

/// `status`: one page view; `keepalive`: two more ordinary ones. The jar is
/// updated either way. Exit 3 = logged out (the ship script's STOP signal).
fn status(keepalive: bool) -> Result<i32, String> {
    let mut c = Client::open(jar_path())?;
    let r = c.get_page(REPO_URL)?;
    let login = user_login(&r.text());
    match (r.status, login) {
        (200, Some(u)) => {
            let exp = c.jar.get("user_session").and_then(|k| k.expires).map(|t| format!(", user_session valid {:.1} more days", (t - jar::now_unix()) as f64 / 86400.0)).unwrap_or_default();
            println!("logged in as {u} ({} cookies{exp})", c.jar.cookies.len());
            if keepalive {
                std::thread::sleep(std::time::Duration::from_secs(4));
                let _ = c.get_page("https://github.com/vjeux/trackmania-tas/releases/tag/videos-v1")?;
                std::thread::sleep(std::time::Duration::from_secs(3));
                let _ = c.get_page("https://github.com/notifications")?;
                println!("keepalive: 3 page views");
            }
            Ok(0)
        }
        (s, _) => {
            println!("logged out: HTTP {s}{} — the session has ended (200 = the page came back anonymous); repeat the one-time seed (UPLOADER-OWN-SESSION.md)", r.location.map(|l| format!(" → {l}")).unwrap_or_default());
            Ok(3)
        }
    }
}

/// The three requests `ghvid.sh` proved, then a normal page view. Prints the
/// asset URL alone on stdout. Exit 3 = the session is gone (no CSRF token /
/// a 302), the same code `clip ship` already reads as "renew the session".
fn upload(args: &[String]) -> Result<i32, String> {
    let file = args.first().filter(|a| !a.starts_with("--")).ok_or("upload needs FILE")?;
    let ct = flag(args, "--content-type").unwrap_or("video/mp4");
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let name = Path::new(file).file_name().and_then(|s| s.to_str()).ok_or("file name")?.to_string();
    let mut c = Client::open(jar_path())?;

    // 0. the CSRF token, from the page the browser posts from
    let page = c.get_page(EDIT_URL)?;
    if page.status != 200 {
        eprintln!("ghsession: the edit page answered HTTP {}{} — logged out", page.status, page.location.as_deref().map(|l| format!(" → {l}")).unwrap_or_default());
        return Ok(3);
    }
    let Some(token) = upload_csrf_token(&page.text()) else {
        eprintln!("ghsession: no upload CSRF token on {EDIT_URL} — is the session still valid?");
        return Ok(3);
    };
    let ajax: [(&str, &str); 9] = [
        ("accept", "application/json"),
        ("origin", "https://github.com"),
        ("referer", EDIT_URL),
        ("x-requested-with", "XMLHttpRequest"),
        ("accept-language", "en-US,en;q=0.9"),
        ("sec-fetch-site", "same-origin"),
        ("sec-fetch-mode", "cors"),
        ("sec-fetch-dest", "empty"),
        ("github-verified-fetch", "true"),
    ];

    // 1. the upload policy
    let size = bytes.len().to_string();
    let (ct1, b1) = multipart(&[
        Part::Text("name", &name),
        Part::Text("size", &size),
        Part::Text("content_type", ct),
        Part::Text("authenticity_token", &token),
        Part::Text("repository_id", REPO_ID),
        Part::Text("upload_container_type", "blob"),
        Part::Text("upload_container_id", REPO_ID),
    ]);
    let r1 = c.send("POST", "https://github.com/upload/policies/assets", &ajax, Some((&ct1, &b1)))?;
    if r1.status != 201 && r1.status != 200 {
        return Err(format!("step 1 (policy) returned HTTP {}: {}", r1.status, r1.text().chars().take(400).collect::<String>()));
    }
    let policy: serde_json::Value = serde_json::from_slice(&r1.body).map_err(|e| format!("policy is not JSON: {e}"))?;
    let s = |k: &[&str]| -> Result<String, String> {
        let mut v = &policy;
        for key in k {
            v = v.get(*key).ok_or_else(|| format!("policy has no {}", k.join(".")))?;
        }
        v.as_str().map(String::from).or_else(|| v.as_i64().map(|n| n.to_string())).ok_or_else(|| format!("policy {} is not a string", k.join(".")))
    };
    let upload_url = s(&["upload_url"])?;
    let asset_href = s(&["asset", "href"])?;
    let asset_id = s(&["asset", "id"])?;
    let asset_token = s(&["asset_upload_authenticity_token"])?;
    let asset_put = s(&["asset_upload_url"]).unwrap_or_else(|_| format!("/upload/assets/{asset_id}"));
    let form = policy.get("form").and_then(|f| f.as_object()).ok_or("policy has no form")?;

    // 2. the bytes to S3, the policy's form fields first, no cookies (other host)
    let fields: Vec<(String, String)> = form.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect();
    let mut parts: Vec<Part> = fields.iter().map(|(k, v)| Part::Text(k, v)).collect();
    parts.push(Part::File { field: "file", name: &name, mime: ct, bytes: &bytes });
    let (ct2, b2) = multipart(&parts);
    let r2 = c.send("POST", &upload_url, &[("origin", "https://github.com"), ("referer", EDIT_URL)], Some((&ct2, &b2)))?;
    if !matches!(r2.status, 200 | 201 | 204) {
        return Err(format!("step 2 (S3) returned HTTP {}: {}", r2.status, r2.text().chars().take(400).collect::<String>()));
    }

    // 3. finalise
    let (ct3, b3) = multipart(&[Part::Text("authenticity_token", &asset_token)]);
    let put_url = if asset_put.starts_with("https://") { asset_put } else { format!("https://github.com{asset_put}") };
    let r3 = c.send("PUT", &put_url, &ajax, Some((&ct3, &b3)))?;
    if !matches!(r3.status, 200 | 201) {
        return Err(format!("step 3 (finalise) returned HTTP {}: {}", r3.status, r3.text().chars().take(400).collect::<String>()));
    }

    // 4. what a person does next: look at the repo page
    let _ = c.get_page(REPO_URL);
    println!("{asset_href}");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_login_and_the_token_come_off_the_page() {
        let html = r#"<meta name="user-login" content="vjeux"><script>{"csrf_tokens":{"/upload/policies/assets":{"post":"abc+/def="}}}</script>"#;
        assert_eq!(user_login(html), Some("vjeux".into()));
        assert_eq!(upload_csrf_token(html), Some("abc+/def=".into()));
        let anon = r#"<meta name="user-login" content="">"#;
        assert_eq!(user_login(anon), None);
        assert_eq!(upload_csrf_token(anon), None);
        let classic = r#"<form action="/upload/policies/assets" method="post"><input type="hidden" name="authenticity_token" value="tok"/>"#;
        assert_eq!(upload_csrf_token(classic), Some("tok".into()));
    }

    #[test]
    fn multipart_is_well_formed() {
        let (ct, body) = multipart(&[Part::Text("a", "1"), Part::File { field: "file", name: "x.mp4", mime: "video/mp4", bytes: b"\x00\x01" }]);
        let boundary = ct.strip_prefix("multipart/form-data; boundary=").unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with(&format!("--{boundary}\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--{boundary}\r\n")));
        assert!(text.contains("name=\"file\"; filename=\"x.mp4\"\r\nContent-Type: video/mp4\r\n\r\n\x00\x01\r\n"));
        assert!(text.ends_with(&format!("--{boundary}--\r\n")));
    }

    #[test]
    fn urls_split_into_host_and_path() {
        assert_eq!(host_path("https://github.com/upload/policies/assets?x=1").unwrap(), ("github.com".into(), "/upload/policies/assets".into()));
        assert_eq!(host_path("https://GitHub.com").unwrap(), ("github.com".into(), "/".into()));
        assert!(host_path("http://github.com/").is_err());
    }
}
