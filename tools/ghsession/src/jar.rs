//! The jar: every cookie GitHub sets, kept exactly (with or without an expiry),
//! stored as JSON on the box, and turned back into a `Cookie:` header the way a
//! browser would. This is the part `curl -c` got wrong twice on 2026-09-09 (it
//! keeps only cookies with an expiry, and would not keep a `__Host-` one), which
//! is why it is written out here rather than borrowed.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    /// The domain the cookie is scoped to, without a leading dot. `host_only`
    /// says whether it came without a `Domain` attribute (then it matches the
    /// request host exactly, never a subdomain).
    pub domain: String,
    pub host_only: bool,
    pub path: String,
    /// Unix seconds; `None` = a session cookie, kept until replaced or deleted.
    pub expires: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Jar {
    /// The machine this jar was seeded on. The session must only ever be used
    /// from one place (a datacenter IP logged one out everywhere in minutes).
    pub host: String,
    pub seeded_unix: i64,
    pub cookies: Vec<Cookie>,
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Jar {
    pub fn load(path: &Path) -> Result<Jar, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: not a jar: {e}", path.display()))
    }

    /// Written through a temporary and renamed, mode 600: a crash mid-write
    /// leaves the previous jar, and nobody else on the box reads the session.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let tmp = path.with_extension(format!("json.new.{}", std::process::id()));
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&tmp, path).map_err(|e| format!("{} -> {}: {e}", tmp.display(), path.display()))
    }

    /// A jar out of a browser's `Cookie:` request header — the one-time seed.
    /// Every cookie becomes a host-only, path `/` session cookie of `host`; the
    /// first response's `Set-Cookie`s then fill in whatever GitHub re-sets.
    pub fn from_cookie_header(header: &str, host: &str, machine: &str) -> Jar {
        let header = header.trim().trim_start_matches("Cookie:").trim_start_matches("cookie:").trim();
        let mut jar = Jar { host: machine.to_string(), seeded_unix: now_unix(), cookies: Vec::new() };
        for pair in header.split(';') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            let (name, value) = match pair.split_once('=') {
                Some((n, v)) => (n.trim(), v.trim()),
                None => continue,
            };
            if name.is_empty() {
                continue;
            }
            jar.cookies.push(Cookie {
                name: name.to_string(),
                value: value.to_string(),
                domain: host.to_string(),
                host_only: true,
                path: "/".to_string(),
                expires: None,
                secure: true,
                http_only: false,
            });
        }
        jar
    }

    /// Apply one `Set-Cookie` header received from `request_host`: replace the
    /// cookie of the same (name, domain, path), delete it when the server says
    /// so (`Max-Age=0` or an `Expires` in the past — GitHub's way of logging a
    /// session out), otherwise store it whether or not it has an expiry.
    pub fn apply_set_cookie(&mut self, header: &str, request_host: &str) -> Applied {
        let Some(c) = parse_set_cookie(header, request_host) else { return Applied::Ignored };
        let key = |k: &Cookie| (k.name.clone(), k.domain.clone(), k.path.clone());
        let k = key(&c);
        self.cookies.retain(|x| key(x) != k);
        let dead = c.value.is_empty() && c.expires.is_some() || matches!(c.expires, Some(t) if t <= now_unix());
        if dead {
            Applied::Deleted(c.name)
        } else {
            let name = c.name.clone();
            self.cookies.push(c);
            Applied::Stored(name)
        }
    }

    /// The `Cookie:` header for a request to `https://host/path`, RFC 6265
    /// order (longer paths first), expired cookies dropped.
    pub fn header_for(&self, host: &str, path: &str) -> String {
        let now = now_unix();
        let mut v: Vec<&Cookie> = self
            .cookies
            .iter()
            .filter(|c| c.expires.map(|t| t > now).unwrap_or(true))
            .filter(|c| if c.host_only { c.domain == host } else { host == c.domain || host.ends_with(&format!(".{}", c.domain)) })
            .filter(|c| path_matches(path, &c.path))
            .collect();
        v.sort_by(|a, b| b.path.len().cmp(&a.path.len()));
        v.iter().map(|c| format!("{}={}", c.name, c.value)).collect::<Vec<_>>().join("; ")
    }

    pub fn get(&self, name: &str) -> Option<&Cookie> {
        self.cookies.iter().find(|c| c.name == name)
    }
}

#[derive(Debug, PartialEq)]
pub enum Applied {
    Stored(String),
    Deleted(String),
    Ignored,
}

/// RFC 6265 §5.1.4 path-match.
fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    request_path == cookie_path
        || (request_path.starts_with(cookie_path) && (cookie_path.ends_with('/') || request_path[cookie_path.len()..].starts_with('/')))
}

/// One `Set-Cookie` header → a cookie scoped to `request_host` (host-only
/// unless it names a `Domain`). `None` for a malformed one.
pub fn parse_set_cookie(header: &str, request_host: &str) -> Option<Cookie> {
    let mut parts = header.split(';');
    let first = parts.next()?.trim();
    let (name, value) = first.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let mut c = Cookie {
        name: name.to_string(),
        value: value.trim().to_string(),
        domain: request_host.to_string(),
        host_only: true,
        path: "/".to_string(),
        expires: None,
        secure: false,
        http_only: false,
    };
    let mut max_age: Option<i64> = None;
    let mut expires: Option<i64> = None;
    for attr in parts {
        let attr = attr.trim();
        let (k, v) = match attr.split_once('=') {
            Some((k, v)) => (k.trim().to_ascii_lowercase(), v.trim()),
            None => (attr.to_ascii_lowercase(), ""),
        };
        match k.as_str() {
            "domain" => {
                let d = v.trim_start_matches('.').to_ascii_lowercase();
                if !d.is_empty() {
                    c.domain = d;
                    c.host_only = false;
                }
            }
            "path" => {
                if v.starts_with('/') {
                    c.path = v.to_string();
                }
            }
            "max-age" => max_age = v.parse::<i64>().ok(),
            "expires" => expires = parse_http_date(v),
            "secure" => c.secure = true,
            "httponly" => c.http_only = true,
            _ => {}
        }
    }
    // Max-Age wins over Expires (RFC 6265 §5.3 step 3)
    c.expires = match max_age {
        Some(a) => Some(now_unix() + a),
        None => expires,
    };
    // a __Host- cookie is host-only and path / by definition
    if c.name.starts_with("__Host-") {
        c.host_only = true;
        c.domain = request_host.to_string();
        c.path = "/".to_string();
    }
    Some(c)
}

/// `Wdy, DD Mon YYYY HH:MM:SS GMT` and the dashed `Wdy, DD-Mon-YYYY …` form,
/// to unix seconds. Anything else → `None` (the cookie is then a session one,
/// which errs on the side of keeping it).
pub fn parse_http_date(s: &str) -> Option<i64> {
    let s = s.trim();
    let rest = s.split_once(',').map(|(_, r)| r).unwrap_or(s).trim();
    let tokens: Vec<&str> = rest.split(|ch: char| ch == ' ' || ch == '-').filter(|t| !t.is_empty()).collect();
    // DD Mon YYYY HH:MM:SS GMT
    if tokens.len() < 4 {
        return None;
    }
    let day: i64 = tokens[0].parse().ok()?;
    let mon = match tokens[1].to_ascii_lowercase().as_str() {
        "jan" => 1, "feb" => 2, "mar" => 3, "apr" => 4, "may" => 5, "jun" => 6,
        "jul" => 7, "aug" => 8, "sep" => 9, "oct" => 10, "nov" => 11, "dec" => 12,
        _ => return None,
    };
    let year: i64 = tokens[2].parse().ok()?;
    let year = if year < 100 { if year < 70 { 2000 + year } else { 1900 + year } } else { year };
    let mut hms = tokens[3].split(':');
    let h: i64 = hms.next()?.parse().ok()?;
    let m: i64 = hms.next()?.parse().ok()?;
    let sec: i64 = hms.next()?.parse().ok()?;
    Some(days_from_civil(year, mon, day) * 86400 + h * 3600 + m * 60 + sec)
}

/// Howard Hinnant's days-from-civil (proleptic Gregorian), days since 1970-01-01.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_browser_header_becomes_a_jar_and_comes_back_whole() {
        let j = Jar::from_cookie_header("Cookie: _octo=GH1.1.1; logged_in=yes; __Host-user_session_same_site=abc; user_session=abc; _gh_sess=x%3D%3D", "github.com", "WhiteStick");
        assert_eq!(j.cookies.len(), 5);
        assert_eq!(j.header_for("github.com", "/vjeux/trackmania-tas"), "_octo=GH1.1.1; logged_in=yes; __Host-user_session_same_site=abc; user_session=abc; _gh_sess=x%3D%3D");
        // host-only: not for a subdomain, and never for another site
        assert_eq!(j.header_for("api.github.com", "/"), "");
        assert_eq!(j.header_for("github-production-user-asset-6210df.s3.amazonaws.com", "/"), "");
    }

    #[test]
    fn set_cookie_replaces_deletes_and_keeps_session_cookies() {
        let mut j = Jar::from_cookie_header("user_session=old; _gh_sess=one", "github.com", "box");
        // a rotated Rails session, no expiry: kept (curl -c would have dropped it)
        assert_eq!(j.apply_set_cookie("_gh_sess=two; path=/; secure; HttpOnly; SameSite=Lax", "github.com"), Applied::Stored("_gh_sess".into()));
        assert_eq!(j.get("_gh_sess").unwrap().value, "two");
        assert_eq!(j.cookies.iter().filter(|c| c.name == "_gh_sess").count(), 1);
        // the way GitHub logs a session out
        assert_eq!(j.apply_set_cookie("user_session=; path=/; expires=Thu, 01 Jan 1970 00:00:01 GMT; secure; HttpOnly", "github.com"), Applied::Deleted("user_session".into()));
        assert!(j.get("user_session").is_none());
        // a renewed session with a two-week expiry
        assert_eq!(j.apply_set_cookie("user_session=new; path=/; expires=Thu, 24 Sep 2026 05:09:03 GMT; secure; HttpOnly; SameSite=Lax", "github.com"), Applied::Stored("user_session".into()));
        assert_eq!(j.get("user_session").unwrap().expires, Some(1790226543));
        // a domain cookie reaches subdomains, a __Host- one never leaves the host
        j.apply_set_cookie("_device_id=d; domain=.github.com; path=/; expires=Fri, 10 Sep 2027 00:00:00 GMT", "github.com");
        j.apply_set_cookie("__Host-user_session_same_site=s; domain=.github.com; path=/x; secure", "github.com");
        assert!(j.header_for("gist.github.com", "/").contains("_device_id=d"));
        assert!(!j.header_for("gist.github.com", "/").contains("__Host-"));
        assert!(j.header_for("github.com", "/anything").contains("__Host-user_session_same_site=s"));
    }

    #[test]
    fn http_dates_both_shapes() {
        assert_eq!(parse_http_date("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert_eq!(parse_http_date("Thu, 01-Jan-1970 00:00:10 GMT"), Some(10));
        assert_eq!(parse_http_date("Sat, 10 Sep 2026 04:39:35 GMT"), Some(1789015175));
        assert_eq!(parse_http_date("garbage"), None);
    }

    #[test]
    fn path_matching_is_rfc_6265() {
        assert!(path_matches("/vjeux/x", "/"));
        assert!(path_matches("/vjeux/x", "/vjeux"));
        assert!(!path_matches("/vjeuxy", "/vjeux"));
        assert!(!path_matches("/", "/vjeux"));
    }
}
