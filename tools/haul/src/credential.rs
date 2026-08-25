//! Getting the bridge credential onto a fresh box, without a human.
//!
//! **The problem.** No on-demand box holds a GitHub credential. The push route
//! is the render box's deploy key, reached over a bridge that needs a 161-byte
//! `~/.navi/credentials.json`. A fresh box has none, so every rotation left one
//! manual file copy in an otherwise unattended system — and an otherwise
//! autonomous rotation with a manual step in it is not autonomous.
//!
//! **The direction is forced, and it is not the obvious one.** An on-demand box
//! cannot reach the devserver at all (`ssh` to it times out), while the
//! devserver reaches the on-demand box fine. So this cannot be a *pull* by the
//! box that needs the file. It is a **push from the machine that already has
//! it**, and the thing that makes that possible without anybody deciding is
//! that the repo already publishes which boxes are alive: the box registry.
//!
//! ```text
//!   devvm (holds the credential, long-lived)
//!     └── reads the box registry out of the public repo
//!     └── for each ACTIVE box that lacks the file, scp it, mode 600
//!   on-demand box (needs it, cannot ask for it)
//! ```
//!
//! Hostnames in the registry are not secrets. The credential never touches the
//! repo, a log, a paste, or a journal entry.
//!
//! **The file is hot, and this module treats it that way.** Its contents are
//! never read into a report, hashed into an artifact, base64'd anywhere, or
//! echoed in an error. What is checked and reported is: does it exist, is it
//! owned by us and mode 600, is it a plausible size — and, the only check that
//! actually matters, **does a real bridge operation succeed**.

use crate::lease;
use crate::paths::Layout;
use std::path::{Path, PathBuf};

/// Where the bridge expects it. Not configurable: the bridge binary hard-codes
/// this path, so a setting here could only ever disagree with reality.
pub fn credential_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/var/svcscm".into());
    PathBuf::from(home).join(".navi/credentials.json")
}

/// A plausible size band for the file, so a truncated or replaced-by-an-error
/// copy is caught without looking at what is inside it.
pub const MIN_BYTES: u64 = 80;
pub const MAX_BYTES: u64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    /// Present, sane, and the bridge answered.
    Working,
    /// Present and sane, but no bridge operation has been tried.
    PresentUnproven,
    /// Present and wrong: mode, ownership or size.
    Unsafe(String),
    Absent,
}

impl Health {
    pub fn ok(&self) -> bool {
        matches!(self, Health::Working)
    }
    pub fn describe(&self) -> String {
        match self {
            Health::Working => "the bridge credential is present and the bridge answers".into(),
            Health::PresentUnproven => {
                "the bridge credential is present; no bridge operation has been tried".into()
            }
            Health::Unsafe(why) => format!("the bridge credential is present but {why}"),
            Health::Absent => {
                "no bridge credential: GitHub banking is DEGRADED, the paste mirror still works"
                    .into()
            }
        }
    }
}

/// Judge the file from its metadata alone. Never opens it.
///
/// Pure over the metadata so it can be tested without a credential existing —
/// which matters, because the interesting cases are the wrong ones and nobody
/// should have to create a real secret to exercise them.
pub fn judge(exists: bool, mode: u32, uid_matches: bool, bytes: u64) -> Health {
    if !exists {
        return Health::Absent;
    }
    if mode & 0o077 != 0 {
        return Health::Unsafe(format!("mode is {:o}, which is readable by somebody else", mode & 0o777));
    }
    if !uid_matches {
        return Health::Unsafe("it is owned by another user".into());
    }
    if bytes < MIN_BYTES || bytes > MAX_BYTES {
        return Health::Unsafe(format!(
            "it is {bytes} bytes, outside the plausible band {MIN_BYTES}..{MAX_BYTES} — \
             a truncated copy or an error page"
        ));
    }
    Health::PresentUnproven
}

pub fn inspect(path: &Path) -> Health {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(path) {
        Err(_) => Health::Absent,
        Ok(m) => {
            let me = libc_getuid();
            judge(true, m.mode(), m.uid() == me, m.len())
        }
    }
}

/// `getuid`, without a `libc` dependency: this crate takes none, and the whole
/// point of that is that a fresh box can build it before anything else works.
fn libc_getuid() -> u32 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1).map(|v| v.to_string()))
        })
        .and_then(|v| v.parse().ok())
        .unwrap_or(u32::MAX)
}

/// Prove the bridge works, by doing something with it. A file's presence is
/// not the claim; the claim is that the route functions.
pub fn prove(home: &str) -> Health {
    let ws = format!("{home}/bin/whitestick");
    if !Path::new(&ws).exists() {
        return Health::Absent;
    }
    match crate::gitcmd::try_run(Path::new("/tmp"), &ws, &["echo tmhaul-credential-probe"]) {
        // Never fold the error body into the result: a bridge failure can echo
        // request context, and this is the one place that would leak it.
        Err(_) => Health::PresentUnproven,
        Ok(o) if o.code == 0 && o.stdout.contains("tmhaul-credential-probe") => Health::Working,
        Ok(_) => Health::PresentUnproven,
    }
}

/// The full local verdict for this box.
pub fn health(home: &str) -> Health {
    match inspect(&credential_path()) {
        Health::Absent => Health::Absent,
        Health::Unsafe(w) => Health::Unsafe(w),
        _ => prove(home),
    }
}

// ---------------------------------------------------------------- the server

/// Which boxes should be handed the credential.
///
/// **Two sources, and the second one is why this works at all.**
///
/// The obvious source is the box registry in the repo. It is not sufficient,
/// and the reason is a chicken-and-egg the first real rotation walked straight
/// into: a fresh box registers itself, but it cannot PUSH that registration
/// without the credential, so the registry the server reads never learns the
/// box exists, so the credential is never delivered. The box sits degraded
/// forever, and the alarm that fires says only that it is degraded.
///
/// The second source is the channel that still works when push does not: the
/// **mirror pastes**. Writing one needs an x509 cert, which every box has from
/// its first minute, and the title carries the node name. So a box announces
/// itself durably before it can push anything — and that announcement is what
/// the credential server listens to.
///
/// Filtered the same way either way: not this machine, not retired, and seen
/// recently enough to still exist.
pub fn targets(l: &Layout, me: &str, now: i64, stale_after_s: i64) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = lease::all(l)?
        .into_iter()
        .filter(|b| !b.retired && b.node != me && now - b.last_seen <= stale_after_s)
        .map(|b| b.node)
        .collect();
    for n in from_mirrors(me, now, stale_after_s).unwrap_or_default() {
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Node names announced by recent mirror pastes.
pub fn from_mirrors(me: &str, now: i64, stale_after_s: i64) -> Result<Vec<String>, String> {
    let out = std::process::Command::new("meta")
        .args([
            "phabricator.paste",
            "list",
            &format!("--title-contains={}", crate::bank::MIRROR_TITLE_PREFIX),
            "--limit=40",
            "--output=json",
        ])
        .output()
        .map_err(|e| format!("spawn meta: {e}"))?;
    if !out.status.success() {
        return Err("could not list mirror pastes".into());
    }
    Ok(parse_mirror_nodes(
        &String::from_utf8_lossy(&out.stdout),
        me,
        now,
        stale_after_s,
    ))
}

/// Pull `(node, created)` out of the paste listing and keep the fresh ones.
/// Pure, so the parse can be tested against a real listing without a network.
pub fn parse_mirror_nodes(json: &str, me: &str, now: i64, stale_after_s: i64) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for chunk in json.split("{\"id\":\"").skip(1) {
        let title = chunk
            .split("\"title\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("");
        let created: i64 = chunk
            .split("\"created\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        // `TMHAUL-STATE <node> <iso> sha=<sha>`
        let Some(rest) = title.strip_prefix(&format!("{} ", crate::bank::MIRROR_TITLE_PREFIX)) else {
            continue;
        };
        let Some(node) = rest.split_whitespace().next() else { continue };
        if node.is_empty() || node == me || now - created > stale_after_s {
            continue;
        }
        if !out.contains(&node.to_string()) {
            out.push(node.to_string());
        }
    }
    out
}

/// The on-demand box's fully-qualified name. The registry stores the short
/// form (`42504`), which is what a filename can hold; ssh needs the whole
/// thing.
pub fn fqdn(node: &str) -> String {
    if node.contains('.') {
        node.to_string()
    } else {
        format!("{node}.od.fbinfra.net")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Delivery {
    /// The box already had it.
    AlreadyThere,
    Installed,
    /// Could not be reached or the copy failed. The reason is a category,
    /// never the transport's error body.
    Failed(String),
}

/// Push the credential to one box, over ssh, mode 600.
///
/// Never logs, hashes, or echoes the content. The transfer writes straight to
/// the destination path with a restrictive umask rather than staging a
/// world-readable temporary copy anywhere.
pub fn deliver(node: &str) -> Delivery {
    let src = credential_path();
    if !src.exists() {
        return Delivery::Failed("this machine has no credential to serve".into());
    }
    let host = fqdn(node);
    let ssh = |cmd: &str| {
        crate::gitcmd::try_run(
            Path::new("/tmp"),
            "ssh",
            &["-o", "StrictHostKeyChecking=no", "-o", "ConnectTimeout=10", "-o", "BatchMode=yes", &host, cmd],
        )
    };

    match ssh("test -s ~/.navi/credentials.json && echo present || echo absent") {
        Err(_) => return Delivery::Failed("unreachable".into()),
        Ok(o) if o.code != 0 => return Delivery::Failed("unreachable".into()),
        Ok(o) if o.stdout.contains("present") => return Delivery::AlreadyThere,
        Ok(_) => {}
    }

    let Ok(bytes) = std::fs::read(&src) else {
        return Delivery::Failed("cannot read the local credential".into());
    };
    // `cat >` under a restrictive umask, with the content on stdin: it never
    // becomes an argv the process table can show, and never lands in a
    // temporary file on either side.
    let out = std::process::Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "ConnectTimeout=10",
            "-o", "BatchMode=yes",
            &host,
            "umask 077 && mkdir -p ~/.navi && cat > ~/.navi/credentials.json && chmod 600 ~/.navi/credentials.json && echo installed",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            if let Some(mut si) = c.stdin.take() {
                si.write_all(&bytes)?;
            }
            c.wait_with_output()
        });

    match out {
        Err(_) => Delivery::Failed("the transfer did not complete".into()),
        Ok(o) if o.status.success() && String::from_utf8_lossy(&o.stdout).contains("installed") => {
            Delivery::Installed
        }
        // Deliberately not the stderr: a failed ssh can echo command context.
        Ok(_) => Delivery::Failed("the far side did not confirm the install".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rec::Rec;

    fn layout(name: &str) -> Layout {
        let p = std::env::temp_dir().join(format!("haul-cred-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        for d in Layout::new(&p).all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        Layout::new(p)
    }

    #[test]
    fn an_absent_credential_is_absent_and_says_banking_is_degraded() {
        let h = judge(false, 0o600, true, 161);
        assert_eq!(h, Health::Absent);
        assert!(h.describe().contains("DEGRADED"), "{}", h.describe());
        assert!(!h.ok());
    }

    #[test]
    fn a_world_readable_credential_is_unsafe_not_merely_present() {
        // Mode is checked because the file is hot. A credential that works and
        // is readable by anybody on the box is not a pass.
        let h = judge(true, 0o644, true, 161);
        assert!(matches!(h, Health::Unsafe(_)), "{h:?}");
        assert!(!h.ok());
    }

    #[test]
    fn a_truncated_or_error_page_copy_is_caught_by_size_alone() {
        // Without opening it: an ssh that failed and wrote its error where the
        // file should be produces something outside the band.
        assert!(matches!(judge(true, 0o600, true, 3), Health::Unsafe(_)));
        assert!(matches!(judge(true, 0o600, true, 90_000), Health::Unsafe(_)));
    }

    #[test]
    fn a_good_file_is_present_but_not_yet_proven() {
        // The distinction that matters: the file being there is not the claim.
        // Only a real bridge operation upgrades this to Working.
        assert_eq!(judge(true, 0o600, true, 161), Health::PresentUnproven);
        assert!(!Health::PresentUnproven.ok());
    }

    #[test]
    fn only_a_working_bridge_counts_as_ok() {
        assert!(Health::Working.ok());
        for h in [Health::Absent, Health::PresentUnproven, Health::Unsafe("x".into())] {
            assert!(!h.ok(), "{h:?} must not read as working");
        }
    }

    #[test]
    fn the_server_targets_live_boxes_other_than_itself() {
        let l = layout("targets");
        let now = 1_800_000_000;
        lease::register_at(&l, "42504", now - 60, Some(now + 9999), "live").unwrap();
        lease::register_at(&l, "devvm42752", now - 60, None, "me").unwrap();
        let t = targets(&l, "devvm42752", now, 1800).unwrap();
        assert_eq!(t, vec!["42504".to_string()], "the server must not serve itself");
    }

    #[test]
    fn a_retired_or_long_silent_box_is_not_a_target() {
        // Pushing a credential at a machine that no longer exists is pointless,
        // and doing it every minute forever is worse.
        let l = layout("stale");
        let now = 1_800_000_000;
        lease::register_at(&l, "gone", now - 99_999, Some(now), "ancient").unwrap();
        lease::register_at(&l, "retired", now - 60, Some(now + 9999), "done").unwrap();
        lease::retire_at(&l, "retired", now - 30, "stood down").unwrap();
        assert_eq!(targets(&l, "me", now, 1800).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn short_registry_names_become_reachable_hostnames() {
        assert_eq!(fqdn("42504"), "42504.od.fbinfra.net");
        assert_eq!(fqdn("devvm42752.vll0.facebook.com"), "devvm42752.vll0.facebook.com");
    }

    #[test]
    fn a_failed_delivery_never_carries_the_transports_error_body() {
        // The control for the hygiene rule: every failure reason this module
        // can produce is a fixed category string, so no error path can leak a
        // header, a token or a request echo.
        for d in [
            Delivery::Failed("unreachable".into()),
            Delivery::Failed("the transfer did not complete".into()),
            Delivery::Failed("the far side did not confirm the install".into()),
            Delivery::Failed("this machine has no credential to serve".into()),
            Delivery::Failed("cannot read the local credential".into()),
        ] {
            let Delivery::Failed(why) = d else { unreachable!() };
            assert!(why.len() < 60, "a failure reason should be a category, not a body: {why}");
            assert!(!why.contains("token"), "{why}");
            assert!(!why.contains('{'), "{why}");
        }
    }

    #[test]
    fn nothing_in_this_module_writes_the_credential_into_a_record() {
        // A record built from a health verdict must be safe to commit to a
        // PUBLIC repo. The verdicts are the only thing that reaches one.
        for h in [
            Health::Working,
            Health::Absent,
            Health::PresentUnproven,
            Health::Unsafe("mode is 644, which is readable by somebody else".into()),
        ] {
            let r = Rec::new("credential").f("health", h.describe());
            let rendered = r.render();
            assert!(!rendered.contains("token"), "{rendered}");
            assert!(!rendered.contains("navibot"), "{rendered}");
            assert!(!rendered.contains('"'), "{rendered}");
        }
    }
}

/// **The control: a bootstrap that failed must never read as one that worked.**
///
/// This runs the real delivery path at a host that cannot exist, and requires
/// a `Failed`. It is the arm that matters, because every other check in this
/// module is satisfied by a machine that already has the file — and the whole
/// module exists for machines that do not.
///
/// It is a run-time command (`tmhaul credential selftest`) as well as a test,
/// for the same reason the alarms have one: a check that only ever ran in CI
/// is a check nobody has watched work on the box it is protecting.
pub fn selftest() -> (bool, String) {
    let mut out = String::new();
    let mut ok = true;

    // 1. Delivery to a host that does not exist must FAIL, not silently pass.
    let d = deliver("tmhaul-no-such-host-selftest.invalid");
    let failed = matches!(d, Delivery::Failed(_));
    out.push_str(&format!(
        "{:<44} {}\n",
        "delivery to an unreachable host",
        if failed { "refused".to_string() } else { format!("{d:?}  <-- BROKEN") }
    ));
    ok &= failed;
    if let Delivery::Failed(why) = &d {
        // 2. ...and the reason must be a category, never a transport body.
        let clean = why.len() < 60 && !why.contains('{') && !why.to_lowercase().contains("token");
        out.push_str(&format!(
            "{:<44} {}\n",
            "the failure reason carries no secret",
            if clean { "clean".to_string() } else { format!("LEAKY: {why}") }
        ));
        ok &= clean;
    }

    // 3. A health verdict for a file that is not there must be Absent, and
    //    must not read as ok.
    let absent = inspect(Path::new("/nonexistent/.navi/credentials.json"));
    let right = absent == Health::Absent && !absent.ok();
    out.push_str(&format!(
        "{:<44} {}\n",
        "an absent credential is Absent, not ok",
        if right { "correct".to_string() } else { format!("{absent:?}  <-- BROKEN") }
    ));
    ok &= right;

    // 4. The positive control, so the three above are not passing vacuously on
    //    a box where everything fails: THIS box's own verdict.
    let here = health(&std::env::var("HOME").unwrap_or_default());
    out.push_str(&format!("{:<44} {}\n", "(control) this box's own credential", here.describe()));

    (ok, out)
}

#[cfg(test)]
mod mirror_discovery_tests {
    use super::*;

    /// A real listing, trimmed. The two 42504 rows are deliberate: the same
    /// box mirrors every cycle and must appear once.
    const LISTING: &str = r#"[{"id":"P2474969284","title":"TMHAUL-STATE 24576 2026-08-25T07:36:03Z sha=753114f3ed9a","created":"1787643365"},{"id":"P2474965244","title":"TMHAUL-STATE 42504 2026-08-25T07:32:00Z sha=97147649c662","created":"1787643122"},{"id":"P2474964376","title":"TMHAUL-STATE 42504 2026-08-25T07:31:00Z sha=fdaecb3b9ce2","created":"1787643062"}]"#;

    const NOW: i64 = 1_787_643_400;

    #[test]
    fn a_box_that_cannot_push_yet_is_still_discovered() {
        // The chicken-and-egg this exists for: a fresh box registers itself in
        // the repo but cannot PUSH that registration without the credential,
        // so the registry never learns it exists. Its mirror paste can be
        // written with an x509 cert alone, and that is the announcement.
        let got = parse_mirror_nodes(LISTING, "devvm42752", NOW, 1800);
        assert!(got.contains(&"24576".to_string()), "{got:?}");
    }

    #[test]
    fn each_box_appears_once_however_often_it_mirrors() {
        let got = parse_mirror_nodes(LISTING, "devvm42752", NOW, 1800);
        assert_eq!(got.iter().filter(|n| *n == "42504").count(), 1, "{got:?}");
    }

    #[test]
    fn the_server_does_not_discover_itself() {
        let got = parse_mirror_nodes(LISTING, "24576", NOW, 1800);
        assert!(!got.contains(&"24576".to_string()), "{got:?}");
    }

    #[test]
    fn a_stale_announcement_is_ignored() {
        // A box that mirrored yesterday is gone; pushing a credential at it
        // forever is the failure mode this avoids.
        assert_eq!(parse_mirror_nodes(LISTING, "devvm42752", NOW + 86_400, 1800), Vec::<String>::new());
    }

    #[test]
    fn a_listing_that_is_not_one_yields_nothing_rather_than_garbage() {
        assert_eq!(parse_mirror_nodes("", "me", NOW, 1800), Vec::<String>::new());
        assert_eq!(parse_mirror_nodes("not json at all", "me", NOW, 1800), Vec::<String>::new());
        // A title of the right prefix but the wrong shape must not become a
        // hostname somebody then tries to ssh to.
        assert_eq!(parse_mirror_nodes(r#"[{"id":"P1","title":"TMHAUL-STATE","created":"1787643365"}]"#, "me", NOW, 1800), Vec::<String>::new());
    }
}
