//! `tinyctl gh-release upload --tag TAG --repo OWNER/REPO FILE… [--token-file F]`
//! — a release asset straight from the DEVSERVER, no box, no relay copy.
//!
//! Why this exists (vjeux, 2026-09-29 02:40Z: "why don't you upload the maps to
//! github from the devserver since it's way faster?"): the cup zips were being
//! pushed over the wsx relay to the render box and uploaded from there — 120 MB
//! across the relay first, then up to GitHub, minutes per file. He was right that
//! the devserver should do it, and the reason it "couldn't" was wrong:
//!
//!   * the devserver REACHES github fine (api.github.com answers in ~0.6 s);
//!   * `gh` cannot authenticate through fwdproxy, which demands a CLIENT
//!     CERTIFICATE (`~/.curlrc`: proxy-cert / proxy-cacert). `gh` has no way to
//!     present one, so it dies with "error connecting to api.github.com" in
//!     0.1 s — which reads like "no network" and is not;
//!   * `curl` DOES present the certificate, so the plain REST upload endpoint
//!     works from the devserver. Measured 2026-09-29: 120 MB, HTTP 201, 68 s,
//!     and the asset re-downloaded from the public link was byte-identical.
//!
//! So: talk to the API with curl, never `gh`. The token is the one `gh` stores
//! (`~/.config/gh/hosts.yml`, or the box's copy) — it is a normal PAT and
//! authenticates fine from here; only `gh`'s transport was the problem.
//!
//! Replacing an asset is delete-then-upload: the REST API refuses a duplicate
//! name, and `--clobber` is a `gh` convenience that does exactly this.

use std::path::Path;
use std::process::Command;

const API: &str = "https://api.github.com";
const UPLOADS: &str = "https://uploads.github.com";

/// The PAT: `--token-file`, else `GH_TOKEN`, else the `gh` config (this host's,
/// then the box's copy pulled earlier), so the caller usually passes nothing.
fn token(args: &[String]) -> Result<String, String> {
    if let Some(f) = tmmaps::cli::flag(args, "--token-file") {
        let t = std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"))?;
        return Ok(t.trim().to_string());
    }
    if let Ok(t) = std::env::var("GH_TOKEN") {
        if !t.trim().is_empty() {
            return Ok(t.trim().to_string());
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
    for p in [format!("{home}/.config/gh/hosts.yml"), "/tmp/gh-hosts-from-box.yml".to_string()] {
        if let Ok(text) = std::fs::read_to_string(&p) {
            for line in text.lines() {
                if let Some(rest) = line.trim().strip_prefix("oauth_token:") {
                    let t = rest.trim();
                    if !t.is_empty() {
                        return Ok(t.to_string());
                    }
                }
            }
        }
    }
    Err("no GitHub token: pass --token-file, set GH_TOKEN, or have gh's hosts.yml".into())
}

/// curl with the devserver's proxy certificate (its `~/.curlrc` supplies it) and
/// the API headers. Returns (body, http status).
fn curl(token: &str, args: &[&str]) -> Result<(String, u32), String> {
    let mut c = Command::new("curl");
    c.args(["-s", "-w", "\n%{http_code}"])
        .args(["-H", &format!("Authorization: token {token}")])
        .args(["-H", "Accept: application/vnd.github+json"])
        .args(args);
    let out = c.output().map_err(|e| format!("curl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').ok_or("curl: no status line")?;
    Ok((body.to_string(), code.trim().parse().unwrap_or(0)))
}

/// `tinyctl gh-release create --tag TAG --repo OWNER/REPO [--name TITLE] [--notes TEXT] [--token-file F]`
/// — the release itself (upload needs one to exist; it used to be made by hand with
/// `gh` on the box, 2026-10-30). An existing release of the tag is reused.
fn create(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let tag = f("--tag").ok_or("gh-release create needs --tag TAG")?;
    let repo = f("--repo").ok_or("gh-release create needs --repo OWNER/REPO")?;
    let tok = token(args)?;
    let (body, code) = curl(&tok, &[&format!("{API}/repos/{repo}/releases/tags/{tag}")])?;
    if code == 200 {
        let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        println!("release {tag} exists: {}", v.get("html_url").and_then(|u| u.as_str()).unwrap_or("?"));
        return Ok(());
    }
    let payload = serde_json::json!({
        "tag_name": tag,
        "name": f("--name").unwrap_or_else(|| tag.clone()),
        "body": f("--notes").unwrap_or_default(),
        "draft": false,
        "prerelease": false,
    });
    let (body, code) = curl(&tok, &["-X", "POST", "-H", "Content-Type: application/json", "-d", &payload.to_string(), &format!("{API}/repos/{repo}/releases")])?;
    if code != 201 {
        return Err(format!("create release {tag}: HTTP {code} ({})", body.chars().take(200).collect::<String>()));
    }
    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    println!("release {tag} created: {}", v.get("html_url").and_then(|u| u.as_str()).unwrap_or("?"));
    Ok(())
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    if args.first().map(|a| a == "create").unwrap_or(false) {
        return create(args);
    }
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let tag = f("--tag").ok_or("gh-release upload needs --tag TAG")?;
    let repo = f("--repo").ok_or("gh-release upload needs --repo OWNER/REPO")?;
    let tok = token(args)?;
    let files: Vec<&String> = {
        let mut v = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--tag" | "--repo" | "--token-file" => i += 2,
                "upload" => i += 1,
                a if a.starts_with("--") => return Err(format!("unknown flag {a}")),
                _ => {
                    v.push(&args[i]);
                    i += 1;
                }
            }
        }
        v
    };
    if files.is_empty() {
        return Err("no files to upload".into());
    }
    // the release id for the tag
    let (body, code) = curl(&tok, &[&format!("{API}/repos/{repo}/releases/tags/{tag}")])?;
    if code != 200 {
        return Err(format!("release {tag}: HTTP {code} ({})", body.chars().take(160).collect::<String>()));
    }
    let relv: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("release JSON: {e}"))?;
    let rel = relv.get("id").and_then(|v| v.as_u64()).ok_or("release record has no id")?.to_string();
    // existing assets, so a replacement deletes first (the API refuses a duplicate name)
    let (assets_body, _) = curl(&tok, &[&format!("{API}/repos/{repo}/releases/{rel}/assets?per_page=100")])?;
    let assets: Vec<serde_json::Value> = serde_json::from_str(&assets_body).unwrap_or_default();
    let n_files = files.len();
    for path in files {
        let p = Path::new(path);
        let name = p.file_name().and_then(|s| s.to_str()).ok_or("bad file name")?;
        let bytes = std::fs::metadata(p).map_err(|e| format!("{path}: {e}"))?.len();
        // an asset of this name already there → delete it first (the API refuses a duplicate)
        if let Some(a) = assets.iter().find(|a| a.get("name").and_then(|n| n.as_str()) == Some(name)) {
            if let Some(id) = a.get("id").and_then(|v| v.as_u64()) {
                let (_, dc) = curl(&tok, &["-X", "DELETE", &format!("{API}/repos/{repo}/releases/assets/{id}")])?;
                println!("  {name}: replacing (old asset {id} deleted, HTTP {dc})");
            }
        }
        let t0 = std::time::Instant::now();
        let (body, code) = curl(
            &tok,
            &[
                "-X",
                "POST",
                "-H",
                "Content-Type: application/octet-stream",
                "--data-binary",
                &format!("@{path}"),
                &format!("{UPLOADS}/repos/{repo}/releases/{rel}/assets?name={name}"),
            ],
        )?;
        if code != 201 {
            return Err(format!("{name}: upload HTTP {code} ({})", body.chars().take(200).collect::<String>()));
        }
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let state = resp.get("state").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let got: u64 = resp.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        if got != bytes {
            return Err(format!("{name}: uploaded {got} B but the file is {bytes} B"));
        }
        println!("  {name}: {bytes} B, HTTP 201, state {state}, {:.0}s", t0.elapsed().as_secs_f64());
    }
    println!("gh-release upload: {n_files} file(s) to {repo} @ {tag} — from the devserver, no box");
    Ok(())
}
