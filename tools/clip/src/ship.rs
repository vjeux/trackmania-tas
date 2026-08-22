//! `clip ship` -- publish one rendered mp4 so that a LOGGED-OUT VISITOR can
//! watch it.
//!
//! Five steps, in this order, each one REFUSING rather than warning:
//!
//!   1. settle + probe the local file
//!   2. upload the full-quality original to the `videos-v1` release (the
//!      download mirror)
//!   3. upload to GitHub's user-attachments store (the inline player) -> URL
//!   4. REGISTER that URL in the release body                <- makes it public
//!   5. ANONYMOUS GATE: fetch the URL with no credential at all; require 200
//!      and bytes that probe as playable.
//!
//! Why step 4 exists: **a pushed commit does NOT authorise an attachment for
//! public serving.** Only a reference in content GitHub renders at save time
//! does. 19 clips were shipped before that was learned and 18 of them were 404
//! to everyone but their author. Steps 4 and 5 are what make the claim true.
//!
//! Why step 5 scrubs the environment: every check run for a whole night carried
//! a session cookie, so the failure was invisible. **A gate that runs with
//! credentials is not a gate.**
//!
//! Why step 5 retries: registration takes up to ~45 s to propagate. On 208024
//! the gate read 404 the instant after the release-body edit and 200 with the
//! full playable file 45 s later; trap 10 measured the same window on the
//! takedown side, so it exists in both directions. **One reading is not a
//! verdict.**

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::fmt::secs;
use crate::platform::Ff;
use crate::proc::{capture, filesize, scratch_dir};

pub const DEFAULT_REPO: &str = "vjeux/trackmania-tas";
pub const DEFAULT_RELEASE: &str = "videos-v1";
/// Absolute on purpose -- see [`Cfg::curl`].
pub const DEFAULT_CURL: &str = "/usr/bin/curl";
pub const ASSET_PREFIX: &str = "https://github.com/user-attachments/assets/";

pub struct Cfg {
    pub repo: String,
    pub release: String,
    pub gh: PathBuf,
    /// The user-attachments uploader (`ghvid.sh`). It is not a repo script and
    /// not portable to Rust: it posts to GitHub's private upload endpoint with
    /// a live browser session cookie and a CSRF token scraped from a rendered
    /// page. Driving it is the honest arrangement.
    pub ghvid: PathBuf,
    /// Named ABSOLUTELY. A `curl` found on PATH could be a wrapper or an alias
    /// that supplies a cookie jar or a netrc, which would turn the one step
    /// that decides publication into a step that proves nothing.
    pub curl: PathBuf,
    pub attempts: u32,
    pub retry_delay: Duration,
    pub settle_delay: Duration,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            repo: DEFAULT_REPO.to_string(),
            release: DEFAULT_RELEASE.to_string(),
            gh: PathBuf::from("gh"),
            ghvid: default_ghvid(),
            curl: PathBuf::from(DEFAULT_CURL),
            attempts: 10,
            retry_delay: Duration::from_secs(10),
            settle_delay: Duration::from_secs(1),
        }
    }
}

impl Cfg {
    /// Overrides, for a box where something lives somewhere else.
    pub fn from_env() -> Cfg {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let d = Cfg::default();
        Cfg {
            repo: var("REPO").unwrap_or(d.repo),
            release: var("RELEASE").unwrap_or(d.release),
            gh: var("CLIP_GH").map(PathBuf::from).unwrap_or(d.gh),
            ghvid: var("GHVID").map(PathBuf::from).unwrap_or(d.ghvid),
            curl: var("CLIP_CURL").map(PathBuf::from).unwrap_or(d.curl),
            ..d
        }
    }
}

fn default_ghvid() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    let a = PathBuf::from(&home).join("bin/ghvid.sh");
    if a.is_file() {
        return a;
    }
    PathBuf::from(&home).join("tas-test/.ghvid/ghvid.sh")
}

// ---------------------------------------------------------------------------
// the pure parts
// ---------------------------------------------------------------------------

/// The only URL shape the inline player serves.
///
/// Anything else means the uploader printed a login page, an error, or a
/// redirect target -- all of which look like success to a shell that only
/// checked the exit code.
pub fn check_asset_url(url: &str) -> Result<(), String> {
    if url.starts_with(ASSET_PREFIX) && url.len() > ASSET_PREFIX.len() {
        Ok(())
    } else {
        Err(format!("unexpected asset url: {url}"))
    }
}

/// Put `<name>: <url>` into the release body, or `None` if it is already there.
///
/// THIS IS THE STEP THAT MAKES THE ASSET PUBLIC. The listing lives inside a
/// `<details>` block, and a new line goes immediately before the closing tag of
/// the FIRST such block so the block stays the listing; a body with no block
/// gets the line appended. Both spellings are content GitHub re-renders on
/// save, which is what authorises the attachment.
pub fn insert_registration(body: &str, name: &str, url: &str) -> Option<String> {
    if body.contains(url) {
        return None;
    }
    let mut out = String::new();
    if body.lines().any(|l| l.contains("</details>")) {
        let mut done = false;
        for line in body.lines() {
            if line.contains("</details>") && !done {
                out.push_str(&format!("{name}: {url}\n\n"));
                done = true;
            }
            out.push_str(line);
            out.push('\n');
        }
    } else {
        out.push_str(body);
        out.push_str("\n\n");
        out.push_str(&format!("{name}: {url}\n"));
    }
    Some(out)
}

pub fn release_upload_argv(release: &str, staged: &Path, repo: &str) -> Vec<String> {
    vec![
        "release".into(),
        "upload".into(),
        release.into(),
        staged.to_string_lossy().into_owned(),
        "-R".into(),
        repo.into(),
        "--clobber".into(),
    ]
}

pub fn release_view_argv(release: &str, repo: &str) -> Vec<String> {
    vec![
        "release".into(),
        "view".into(),
        release.into(),
        "-R".into(),
        repo.into(),
        "--json".into(),
        "body".into(),
        "-q".into(),
        ".body".into(),
    ]
}

pub fn release_edit_argv(release: &str, repo: &str, notes_file: &Path) -> Vec<String> {
    vec![
        "release".into(),
        "edit".into(),
        release.into(),
        "-R".into(),
        repo.into(),
        "--notes-file".into(),
        notes_file.to_string_lossy().into_owned(),
    ]
}

/// The gate's fetch. `-w '%{http_code}'` puts the status on stdout; the body
/// goes to `out` so it can be probed.
pub fn curl_argv(out: &Path, url: &str) -> Vec<String> {
    vec![
        "-s".into(),
        "-L".into(),
        "--retry".into(),
        "3".into(),
        "--max-time".into(),
        "300".into(),
        "-o".into(),
        out.to_string_lossy().into_owned(),
        "-w".into(),
        "%{http_code}".into(),
        url.into(),
    ]
}

// ---------------------------------------------------------------------------
// the gate
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Passed {
    pub bytes: u64,
    pub duration: f64,
}

/// STEP 5. What a logged-out visitor gets, and the only step that decides
/// whether the clip is published.
///
/// `probe` is injected so the gate can be tested where no ffprobe exists; in
/// production it is [`Ff::probe_duration`].
pub fn gate<P>(cfg: &Cfg, url: &str, out: &Path, probe: P) -> Result<Passed, String>
where
    P: Fn(&Path) -> Result<f64, String>,
{
    if !cfg.curl.is_absolute() {
        return Err(format!(
            "the anonymous gate needs an absolute curl path (got {}): a PATH lookup could \
             find a wrapper that carries a cookie jar, and a gate with credentials is not a gate",
            cfg.curl.display()
        ));
    }
    let mut last_code = String::new();
    let mut last_bytes = 0u64;
    for attempt in 1..=cfg.attempts {
        // env -i: no cookie jar, no GH_TOKEN, no netrc, no proxy can leak in.
        let mut c = Command::new(&cfg.curl);
        c.env_clear().args(curl_argv(out, url));
        let r = capture(&mut c).map_err(|e| {
            format!("ANONYMOUS GATE CANNOT RUN: {e} -- a gate that did not run is not a pass")
        })?;
        last_code = r.stdout.trim().to_string();
        if last_code == "200" {
            last_bytes = filesize(out)?;
            match probe(out) {
                Ok(d) => return Ok(Passed { bytes: last_bytes, duration: d }),
                Err(_) => println!(
                    "ship: attempt {attempt} fetched {last_bytes} bytes that do not probe yet — retrying"
                ),
            }
        } else {
            println!("ship: attempt {attempt} http {last_code} — not public yet, retrying");
        }
        if attempt < cfg.attempts {
            std::thread::sleep(cfg.retry_delay);
        }
    }
    let window = secs(cfg.retry_delay.as_secs_f64() * (cfg.attempts.saturating_sub(1)) as f64);
    if last_code != "200" {
        Err(format!(
            "ANONYMOUS GATE FAILED: http {last_code} for {url} after {} tries over ~{window} s — NOT published",
            cfg.attempts
        ))
    } else {
        Err(format!(
            "ANONYMOUS GATE FAILED: fetched {last_bytes} bytes that do not probe — NOT published"
        ))
    }
}

// ---------------------------------------------------------------------------
// the chain
// ---------------------------------------------------------------------------

/// Wait for the file to stop growing.
///
/// A render that is still being written probes short, or probes fine and then
/// ships a truncated upload. Two identical sizes a second apart, exactly as the
/// shell did it.
fn settle(file: &Path, delay: Duration) -> Result<u64, String> {
    let mut a = filesize(file)?;
    loop {
        std::thread::sleep(delay);
        let b = filesize(file)?;
        if a == b {
            return Ok(b);
        }
        a = b;
    }
}

pub fn run(
    ff: &Ff,
    cfg: &Cfg,
    file: &Path,
    mapdir: &Path,
    asset_name: Option<&str>,
) -> Result<(), String> {
    let asset_name = match asset_name {
        Some(n) => n.to_string(),
        None => file
            .file_name()
            .ok_or_else(|| format!("no filename in {}", file.display()))?
            .to_string_lossy()
            .into_owned(),
    };
    let map_name = basename(mapdir);

    // --- 1. the local file, settled and playable ---------------------------
    if !file.is_file() {
        return Err(format!("no such file: {}", file.display()));
    }
    let bytes = settle(file, cfg.settle_delay)?;
    let local_dur = ff.probe_duration(file)?;
    println!(
        "ship: local {}  {bytes} bytes  {}s",
        file.display(),
        secs(local_dur)
    );

    // --- 2. the stable download mirror -------------------------------------
    // Staged in a private directory, not /tmp/<asset-name>: the shell copied
    // with `|| true`, so a failed copy silently uploaded whatever file of that
    // name was already in /tmp -- yesterday's clip, under today's name.
    let stage = scratch_dir("clip-ship")?;
    let staged = stage.join(&asset_name);
    let copy = std::fs::copy(file, &staged)
        .map_err(|e| format!("cannot stage {} as {}: {e}", file.display(), staged.display()));
    let upload = copy.and_then(|_| {
        let mut c = Command::new(&cfg.gh);
        c.args(release_upload_argv(&cfg.release, &staged, &cfg.repo));
        capture(&mut c)
    });
    let upload = finish(&stage, upload)?;
    if !upload.ok() {
        return Err(format!("release upload failed: {}", upload.why()));
    }
    println!("ship: release asset {asset_name} uploaded");

    // --- 3. the inline player ----------------------------------------------
    let mut c = Command::new(&cfg.ghvid);
    c.arg(file);
    let up = capture(&mut c)
        .map_err(|e| format!("attachment upload failed: {e} (uploader: {})", cfg.ghvid.display()))?;
    if !up.ok() {
        // Exit 3 is the uploader's "no upload CSRF token": the browser session
        // cookie at ~/.gh-upload/cookie has expired and needs replacing from a
        // logged-in browser. Nothing else in the pipeline needs renewing.
        let hint = if up.code == Some(3) {
            "  (exit 3 = no upload CSRF token — replace ~/.gh-upload/cookie with a fresh one)"
        } else {
            ""
        };
        return Err(format!("attachment upload failed: {}{hint}", up.why()));
    }
    let url = up.stdout.trim().to_string();
    check_asset_url(&url)?;
    println!("ship: asset {url}");

    // --- 4. authorise it for the public ------------------------------------
    let mut c = Command::new(&cfg.gh);
    c.args(release_view_argv(&cfg.release, &cfg.repo));
    let view = capture(&mut c)?;
    if !view.ok() {
        return Err(format!("cannot read the {} body: {}", cfg.release, view.why()));
    }
    let body = view.stdout.trim_end_matches('\n').to_string();
    if let Some(new_body) = insert_registration(&body, &map_name, &url) {
        let notes_dir = scratch_dir("clip-notes")?;
        let notes = notes_dir.join("body.md");
        let edit = std::fs::write(&notes, &new_body)
            .map_err(|e| format!("cannot write {}: {e}", notes.display()))
            .and_then(|_| {
                let mut c = Command::new(&cfg.gh);
                c.args(release_edit_argv(&cfg.release, &cfg.repo, &notes));
                capture(&mut c)
            });
        let edit = finish(&notes_dir, edit)?;
        if !edit.ok() {
            return Err(format!("release body edit failed: {}", edit.why()));
        }
        println!(
            "ship: registered in the {} body (this is what makes it public)",
            cfg.release
        );
    }

    // --- 5. THE GATE --------------------------------------------------------
    let gate_dir = scratch_dir("clip-anon")?;
    let out = gate_dir.join("anon.mp4");
    let passed = gate(cfg, &url, &out, |p| ff.probe_duration(p));
    let _ = std::fs::remove_dir_all(&gate_dir);
    let passed = passed?;
    println!(
        "ship: ANONYMOUS GATE PASSED  http 200  {} bytes  {}s",
        passed.bytes,
        secs(passed.duration)
    );

    println!();
    println!("PUBLISHED  {url}");
    println!(
        "Embed it on its own line in {}/README.md, under the caption line, then",
        mapdir.display()
    );
    println!("commit only that README. Re-run the gate after the push:");
    println!(
        "  env -i {} -s -o /dev/null -w '%{{http_code}}\\n' -L {url}",
        cfg.curl.display()
    );
    Ok(())
}

/// Drop a scratch directory whichever way the step went.
fn finish<T>(dir: &Path, r: Result<T, String>) -> Result<T, String> {
    let _ = std::fs::remove_dir_all(dir);
    r
}

/// The last path component, with any trailing slash ignored.
pub fn basename(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://github.com/user-attachments/assets/0a1b2c3d";

    #[test]
    fn only_a_user_attachments_url_is_an_asset() {
        assert!(check_asset_url(URL).is_ok());
        assert!(check_asset_url("https://github.com/user-attachments/assets/").is_err());
        assert!(check_asset_url("https://github.com/vjeux/trackmania-tas/x.mp4").is_err());
        assert!(check_asset_url("").is_err());
        // a login page printed by an expired uploader session
        assert!(check_asset_url("<!DOCTYPE html>").is_err());
    }

    #[test]
    fn registration_goes_inside_the_listing_block() {
        let body = "# clips\n\n<details>\n<summary>all</summary>\n\n126859: https://x/1\n\n</details>\n\ntail\n";
        let out = insert_registration(body, "208024-mirus-hell-2", URL).unwrap();
        let at_new = out.find("208024-mirus-hell-2").unwrap();
        let at_close = out.find("</details>").unwrap();
        assert!(at_new < at_close, "must land before the closing tag:\n{out}");
        assert!(out.contains("126859: https://x/1"), "must keep what was there");
        assert!(out.ends_with("tail\n"));
    }

    #[test]
    fn registration_appends_when_there_is_no_block() {
        let out = insert_registration("# clips", "270053-fall-2025-18-cp1-end", URL).unwrap();
        assert_eq!(
            out,
            format!("# clips\n\n270053-fall-2025-18-cp1-end: {URL}\n")
        );
    }

    #[test]
    fn only_the_first_closing_tag_gets_the_line() {
        let body = "<details>\na\n</details>\n<details>\nb\n</details>\n";
        let out = insert_registration(body, "m", URL).unwrap();
        assert_eq!(out.matches(URL).count(), 1);
        assert_eq!(out.lines().filter(|l| l.contains("</details>")).count(), 2);
    }

    #[test]
    fn registering_twice_is_not_an_edit() {
        let body = format!("<details>\nm: {URL}\n</details>\n");
        assert!(insert_registration(&body, "m", URL).is_none());
    }

    #[test]
    fn gh_argv_is_what_the_release_needs() {
        assert_eq!(
            release_upload_argv("videos-v1", Path::new("/tmp/x/a.mp4"), "vjeux/trackmania-tas"),
            vec![
                "release",
                "upload",
                "videos-v1",
                "/tmp/x/a.mp4",
                "-R",
                "vjeux/trackmania-tas",
                "--clobber"
            ]
        );
        assert_eq!(
            release_view_argv("videos-v1", "r"),
            vec!["release", "view", "videos-v1", "-R", "r", "--json", "body", "-q", ".body"]
        );
        assert_eq!(
            release_edit_argv("videos-v1", "r", Path::new("/tmp/n/body.md")),
            vec!["release", "edit", "videos-v1", "-R", "r", "--notes-file", "/tmp/n/body.md"]
        );
    }

    #[test]
    fn the_gate_asks_for_the_status_and_keeps_the_body() {
        let a = curl_argv(Path::new("/tmp/anon.mp4"), URL);
        assert_eq!(a.iter().filter(|s| *s == "-w").count(), 1);
        assert!(a.contains(&"%{http_code}".to_string()));
        assert_eq!(a[a.len() - 1], URL);
        // follow redirects (the store 302s to a CDN) and survive a flaky hop
        assert!(a.contains(&"-L".to_string()));
        assert!(a.windows(2).any(|w| w[0] == "--retry" && w[1] == "3"));
        assert!(a.windows(2).any(|w| w[0] == "--max-time" && w[1] == "300"));
    }

    #[test]
    fn a_relative_curl_is_refused_before_anything_is_fetched() {
        let cfg = Cfg { curl: PathBuf::from("curl"), ..Cfg::default() };
        let e = gate(&cfg, URL, Path::new("/tmp/none"), |_| Ok(1.0)).unwrap_err();
        assert!(e.contains("absolute curl path"), "{e}");
        assert!(e.contains("not a gate"), "{e}");
    }

    #[test]
    fn basenames() {
        assert_eq!(basename(Path::new("/a/b/208024-mirus-hell-2")), "208024-mirus-hell-2");
        assert_eq!(basename(Path::new("208024-mirus-hell-2/")), "208024-mirus-hell-2");
    }
}
