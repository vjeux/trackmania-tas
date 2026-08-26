//! Banking: getting work off the box, continuously, and proving it arrived.
//!
//! Three layers, deliberately independent, because each one fails differently:
//!
//! 1. **Commit** to the checkout. Cheap, always available, protects against a
//!    crashed process. Protects against nothing else — a box that vanishes
//!    takes its commits with it.
//! 2. **Mirror** the state tree off the box as a `HAULPACK` in a Phabricator
//!    paste. Needs only the x509 cert every internal box already has, so it
//!    works on a fresh on-demand box with no GitHub credential at all. This is
//!    the layer that makes "a fresh box plus the repo is a complete recovery"
//!    true rather than aspirational.
//! 3. **Push** to GitHub, which is the state of record a human reads. No
//!    on-demand box holds a GitHub credential; the working route is the bridge
//!    to the render box, which has a repo-scoped deploy key.
//!
//! Every layer reports whether it actually ran. A bank that silently did
//! nothing is the failure shape this project keeps paying for, so `bank`
//! returns a receipt naming each layer's outcome, and `unbanked_drift` alarms
//! on the gap.

use crate::codemirror;
use crate::gitcmd::{self, git};
use crate::md5::md5_file;
use crate::pack;
use crate::paths::Layout;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirror {
    None,
    Paste,
    Dir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Push {
    None,
    Direct,
    Whitestick,
}

pub fn mirror_from_str(s: &str) -> Mirror {
    match s {
        "paste" => Mirror::Paste,
        "dir" => Mirror::Dir,
        _ => Mirror::None,
    }
}

pub fn push_from_str(s: &str) -> Push {
    match s {
        "direct" => Push::Direct,
        "whitestick" => Push::Whitestick,
        _ => Push::None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct Receipt {
    pub committed: Option<String>,
    pub commit_skipped_reason: Option<String>,
    pub mirror: Option<String>,
    pub mirror_error: Option<String>,
    pub pushed: Option<String>,
    pub push_error: Option<String>,
    /// Set only when the push failed and unpushed commits were sent down the
    /// paste transport instead.
    pub code_mirror: Option<String>,
    pub code_mirror_error: Option<String>,
    pub files_hashed: usize,
}

impl Receipt {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        match (&self.committed, &self.commit_skipped_reason) {
            (Some(sha), _) => parts.push(format!("commit {}", &sha[..12.min(sha.len())])),
            (None, Some(r)) => parts.push(format!("commit skipped ({r})")),
            (None, None) => parts.push("commit not attempted".into()),
        }
        match (&self.mirror, &self.mirror_error) {
            (Some(id), _) => parts.push(format!("mirror {id}")),
            (None, Some(e)) => parts.push(format!("MIRROR FAILED: {}", brief_error(e))),
            (None, None) => parts.push("mirror off".into()),
        }
        match (&self.pushed, &self.push_error) {
            (Some(w), _) => parts.push(format!("push {w}")),
            (None, Some(e)) => parts.push(format!("PUSH FAILED: {}", brief_error(e))),
            (None, None) => parts.push("push off".into()),
        }
        match (&self.code_mirror, &self.code_mirror_error) {
            (Some(id), _) => parts.push(format!("code mirrored {id}")),
            (None, Some(e)) => parts.push(format!("CODE MIRROR FAILED: {}", brief_error(e))),
            (None, None) => {}
        }
        parts.join(" · ")
    }
}

/// One short line, safe to commit to a **public** repository.
///
/// Two reasons, both learned the hard way on 2026-08-26, when the render box
/// went offline and every 10 minutes a bank wrote the bridge's whole retry
/// transcript into the journal:
///
/// 1. **A transport's error body is not ours to publish.** The bridge is
///    credentialed, and a failing credentialed call can echo request context.
///    Everywhere else in this crate refuses to fold a bridge error body into a
///    result; the receipt was the hole in that rule, and the receipt is the one
///    string that gets committed and pushed.
/// 2. **A multi-line error breaks the page a human reads.** It is banked into a
///    record log and rendered into a markdown table; a newline in it corrupts
///    both, precisely when something is wrong.
///
/// The full text still reaches the operator: the supervisor prints it to its
/// own stderr, which stays on the box.
pub fn brief_error(e: &str) -> String {
    let flat: String = e
        .chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .collect();
    let mut squeezed = String::new();
    for c in flat.chars() {
        if c == ' ' && squeezed.ends_with(' ') {
            continue;
        }
        squeezed.push(c);
    }
    let squeezed = squeezed.trim().to_string();
    // A recognised shape says the operative fact in four words. Anything else
    // is truncated rather than interpreted — a guess about an unknown error is
    // worse than the first 160 characters of it.
    if squeezed.contains("instance offline") {
        return "the render box is offline (bridge reports instance offline)".into();
    }
    if squeezed.chars().count() > 160 {
        let cut: String = squeezed.chars().take(160).collect();
        format!("{cut}…")
    } else {
        squeezed
    }
}

// ------------------------------------------------------------------ secrets

/// Patterns that must never reach `github.com/vjeux/trackmania-tas`, which is
/// a **public** repository.
///
/// The harness handles a navi bridge token and runs on boxes holding x509
/// keys; one careless `cp` into the state tree would publish them. This runs
/// before every commit and refuses the whole bank rather than committing and
/// asking forgiveness — a secret is unpublishable once pushed.
const SECRET_MARKERS: &[&str] = &[
    "BEGIN OPENSSH PRIVATE KEY",
    "BEGIN RSA PRIVATE KEY",
    "BEGIN EC PRIVATE KEY",
    "BEGIN PRIVATE KEY",
    "BEGIN CERTIFICATE",
    "ghp_",
    "github_pat_",
    "\"naviUrl\"",
    "x-api-key",
];

const SECRET_FILENAMES: &[&str] = &["credentials.json", "id_rsa", "id_ed25519", ".netrc", ".pem"];

pub fn scan_for_secrets(dir: &Path) -> Result<Vec<String>, String> {
    let mut hits = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if !d.exists() {
            continue;
        }
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())?.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if SECRET_FILENAMES.iter().any(|m| name == *m || name.ends_with(m)) {
                hits.push(format!("{} (filename looks like a credential)", p.display()));
                continue;
            }
            let Ok(bytes) = std::fs::read(&p) else { continue };
            if bytes.len() > 4_000_000 {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            for m in SECRET_MARKERS {
                if text.contains(m) {
                    hits.push(format!("{} (contains {m:?})", p.display()));
                    break;
                }
            }
        }
    }
    hits.sort();
    Ok(hits)
}

// ------------------------------------------------------------------ manifest

/// md5 every file in the state tree, into `state/MANIFEST.md5`.
///
/// "Verify banked state by md5 before releasing anything" is a standing rule
/// of this project, and the reason it is a rule is that a release destroys the
/// box's files: a checksum recorded *after* the fact is worth nothing.
pub fn write_manifest(l: &Layout) -> Result<usize, String> {
    let mut lines = Vec::new();
    let mut stack = vec![l.state()];
    while let Some(d) = stack.pop() {
        if !d.exists() {
            continue;
        }
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())?.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() && p != l.manifest() {
                let rel = p.strip_prefix(&l.repo).unwrap_or(&p).to_string_lossy().to_string();
                lines.push(format!("{}  {rel}", md5_file(&p).map_err(|e| e.to_string())?));
            }
        }
    }
    lines.sort_by(|a, b| a[34..].cmp(&b[34..]));
    let n = lines.len();
    std::fs::write(l.manifest(), format!("{}\n", lines.join("\n"))).map_err(|e| e.to_string())?;
    Ok(n)
}

/// Re-hash everything the manifest names and report every disagreement.
///
/// `Source::Committed` is the one that matters before releasing a box: it
/// checks the bytes **git has**, not the bytes on a disk that is about to be
/// destroyed. The working tree legitimately runs ahead of the manifest — the
/// journal is append-only and gains a record the moment banking finishes — so
/// verifying the working tree would fail for a healthy run and teach everyone
/// to ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Committed,
    WorkingTree,
}

fn git_show_bytes(repo: &Path, rel: &str) -> Result<Vec<u8>, String> {
    // Manifest paths are relative to the layout root, which is not always the
    // git toplevel (a test harness may nest one inside another). Ask git where
    // we are rather than assuming.
    let prefix = std::process::Command::new("git")
        .args(["rev-parse", "--show-prefix"])
        .current_dir(repo)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let out = std::process::Command::new("git")
        .args(["show", &format!("HEAD:{prefix}{rel}")])
        .current_dir(repo)
        .output()
        .map_err(|e| format!("spawn git show: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "not in the commit: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

pub fn verify(l: &Layout, source: Source) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(l.manifest()).map_err(|e| e.to_string())?;
    let mut bad = Vec::new();
    for line in text.lines().filter(|s| !s.trim().is_empty()) {
        let (want, rel) = line.split_at(32);
        let rel = rel.trim();
        let got = match source {
            Source::WorkingTree => md5_file(&l.repo.join(rel)).map_err(|e| e.to_string()),
            Source::Committed => git_show_bytes(&l.repo, rel).map(|b| crate::md5::md5_hex(&b)),
        };
        match got {
            Ok(got) if got == want => {}
            Ok(got) => bad.push(format!("{rel}: {got} != {want}")),
            Err(e) => bad.push(format!("{rel}: {e}")),
        }
    }
    Ok(bad)
}

pub fn verify_manifest(l: &Layout) -> Result<Vec<String>, String> {
    verify(l, Source::WorkingTree)
}

// ------------------------------------------------------------------ mirror

pub const MIRROR_TITLE_PREFIX: &str = "TMHAUL-STATE";

/// Write the state tree to a paste and return its `P<id>`.
pub fn mirror_to_paste(l: &Layout, node: &str, sha: &str) -> Result<String, String> {
    let body = pack::pack(&l.state(), &l.root(), node).map_err(|e| e.to_string())?;
    let tmp = std::env::temp_dir().join(format!("tmhaul-mirror-{}.haulpack", std::process::id()));
    std::fs::write(&tmp, &body).map_err(|e| e.to_string())?;

    let title = format!(
        "{MIRROR_TITLE_PREFIX} {node} {} sha={}",
        crate::time::iso(crate::time::now()),
        &sha[..12.min(sha.len())]
    );
    // `meta phabricator.paste create --stdin` reads the body from stdin.
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "meta phabricator.paste create --title {} --stdin --output=json < {}",
            shell_quote(&title),
            shell_quote(&tmp.to_string_lossy())
        ))
        .output()
        .map_err(|e| format!("spawn meta: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        return Err(format!("paste create failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let id = stdout
        .split("\"id\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .ok_or_else(|| format!("no paste id in: {}", stdout.trim()))?
        .to_string();
    let _ = std::fs::remove_file(&tmp);
    Ok(id)
}

/// Newest mirror paste id, discovered by title — the entry point for a fresh
/// box that has nothing but a cert.
pub fn latest_mirror() -> Result<Option<(String, String)>, String> {
    let out = std::process::Command::new("meta")
        .args([
            "phabricator.paste",
            "list",
            &format!("--title-contains={MIRROR_TITLE_PREFIX}"),
            "--limit=20",
            "--output=json",
        ])
        .output()
        .map_err(|e| format!("spawn meta: {e}"))?;
    if !out.status.success() {
        return Err(format!("paste list failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    // Newest first is the service's own ordering; parse ids and titles in order.
    let mut best: Option<(String, String, i64)> = None;
    for chunk in text.split("{\"id\":\"").skip(1) {
        let Some(id) = chunk.split('"').next() else { continue };
        let title = chunk
            .split("\"title\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("")
            .to_string();
        let created: i64 = chunk
            .split("\"created\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if best.as_ref().map(|(_, _, c)| created > *c).unwrap_or(true) {
            best = Some((id.to_string(), title, created));
        }
    }
    Ok(best.map(|(id, title, _)| (id, title)))
}

pub fn read_mirror(id: &str) -> Result<String, String> {
    let out = std::process::Command::new("meta")
        .args(["phabricator.paste", "read", &format!("--id={id}")])
        .output()
        .map_err(|e| format!("spawn meta: {e}"))?;
    if !out.status.success() {
        return Err(format!("paste read failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ------------------------------------------------------------------ push

/// Make our branch a fast-forward of the remote, rebasing if somebody else
/// pushed while we were working.
///
/// **This is not a rare case and it is not the harness's own fault.** The repo
/// has other authors: on the first afternoon this ran, an unrelated session
/// pushed `entorder can put the car in the MIDDLE of the entity list` between
/// two banks, and the next push was rejected as a non-fast-forward. A
/// long-haul system that stops banking the moment a colleague commits is not a
/// long-haul system.
///
/// A rebase is safe here by construction: every state file is append-only and
/// sharded by writer, so our commits and theirs touch disjoint paths. When
/// that turns out not to be true, the rebase is aborted and the error says so
/// rather than leaving a half-rebased checkout behind.
pub fn sync_with_remote(l: &Layout, branch: &str) -> Result<Option<String>, String> {
    let fetch = gitcmd::git_env(&l.repo, &["fetch", "-q", "origin", branch])?;
    if fetch.code != 0 {
        // A FAILED FETCH IS NOT "NOTHING TO DO".
        //
        // This returned `Ok(None)` — "no network, not fatal, carry on" — and
        // the push then went ahead against a remote nobody had looked at. On a
        // box with no proxy in its environment the fetch fails every time, so
        // the rebase never ran, the push was rejected, and the retry loop
        // re-ran the same silent no-op three times and reported that "the
        // remote kept moving". It had not moved at all.
        //
        // A fetch we could not do means the remote's state is UNKNOWN, and
        // pushing on unknown is precisely what this project forbids. Fail.
        return Err(format!(
            "cannot see the remote, so there is nothing safe to push onto: {}. \
             On an on-demand box this is usually the proxy — git needs \
             https_proxy=http://fwdproxy:8080, and a DETACHED supervisor does not \
             inherit your shell's",
            fetch.stderr.trim()
        ));
    }
    let behind = gitcmd::try_run(
        &l.repo,
        "git",
        &["merge-base", "--is-ancestor", "FETCH_HEAD", "HEAD"],
    )?;
    if behind.code == 0 {
        return Ok(None); // already a fast-forward
    }
    let before = gitcmd::head_sha(&l.repo)?;
    // `--autostash`, because a supervisor is usually running while this
    // happens: the worker appends to the journal continuously, so between the
    // commit a moment ago and this rebase the working tree has almost
    // certainly moved again. Without it the rebase refuses with "you have
    // unstaged changes" and the harness reports a conflict that is not one.
    let rebase = gitcmd::try_run(&l.repo, "git", &["rebase", "--autostash", "FETCH_HEAD"])?;
    if rebase.code != 0 {
        let _ = gitcmd::try_run(&l.repo, "git", &["rebase", "--abort"]);
        return Err(format!(
            "the remote has commits we do not, and rebasing onto them conflicts — \
             a human needs to look. git said: {}",
            rebase.stderr.trim()
        ));
    }
    let after = gitcmd::head_sha(&l.repo)?;
    Ok(Some(format!(
        "rebased {} onto the remote, now {}",
        &before[..8.min(before.len())],
        &after[..8.min(after.len())]
    )))
}

/// Push through the render-box bridge: bundle the new commits, copy them over
/// with `wsx` (which md5-checks both ends), and let the box — which holds the
/// repo-scoped deploy key — do the push.
///
/// It uses its own clone at `~/haul-push` rather than the shared
/// `~/trackmania-tas` checkout, because that one is in the middle of somebody
/// else's render.
/// Push through the render-box bridge, retrying if the remote moves under us.
///
/// **The race is real and it is not ours to prevent.** `sync_with_remote`
/// rebases onto the remote as it was a moment ago; between that fetch and the
/// push, another session can land a commit — which happened twice in the first
/// hour, both times from unrelated work in the same repo. A single-shot
/// sync-then-push is a time-of-check-to-time-of-use bug, and on a system that
/// banks every thirty minutes for months it would fire regularly.
///
/// So: bounded retry, re-syncing each time. It gives up rather than looping,
/// and the mirror layer has already succeeded by the time this runs, so a
/// give-up costs freshness of the repo and never work.
pub fn push_via_whitestick(l: &Layout, branch: &str) -> Result<String, String> {
    let mut last = String::new();
    for attempt in 1..=3 {
        match push_via_whitestick_once(l, branch) {
            Ok(s) => {
                return Ok(if attempt == 1 {
                    s
                } else {
                    format!("{s} [after {attempt} attempts: the remote moved under us]")
                })
            }
            Err(e) => {
                let racy = e.contains("fetch first")
                    || e.contains("non-fast-forward")
                    || e.contains("rejected");
                last = e;
                if !racy {
                    return Err(last);
                }
                // Someone else landed a commit. Rebase onto it and try again.
                if let Err(sync_err) = sync_with_remote(l, branch) {
                    return Err(format!("{last} — and re-syncing failed: {sync_err}"));
                }
            }
        }
    }
    Err(format!("{last} (gave up after 3 attempts; the remote kept moving)"))
}

fn push_via_whitestick_once(l: &Layout, branch: &str) -> Result<String, String> {
    let rebased = sync_with_remote(l, branch)?;
    let home = std::env::var("HOME").unwrap_or_else(|_| "/var/svcscm".into());
    let ws = format!("{home}/bin/whitestick");
    let wsx = format!("{home}/bin/wsx");
    if !Path::new(&ws).exists() {
        return Err(format!("no bridge binary at {ws}"));
    }

    // A unique name per push. The bridge's file copier skips a transfer whose
    // md5 already matches on the far side, so a FIXED remote name left by an
    // earlier push is a stale file the next push can collide with — observed
    // once as `md5 mismatch after push`, with the remote still holding the
    // previous bundle. A fresh name each time removes the class.
    let stamp = format!("{}-{}", std::process::id(), crate::time::now());
    let bundle = std::env::temp_dir().join(format!("tmhaul-{stamp}.bundle"));
    let _ = std::fs::remove_file(&bundle);

    // RESOLVE THE SHA FIRST, THEN BUNDLE THAT EXACT SHA.
    //
    // Bundling `branch` and then asking what `branch` points at is two reads
    // of a moving target. A supervisor banking concurrently commits between
    // them — observed: the bundle carried the heartbeat's commit and the
    // rev-parse eight seconds later returned the supervisor's, and the push
    // was reported as a failed transfer when it had delivered exactly what it
    // was given.
    //
    // This is the SECOND time this check has cried wolf: first comparing
    // against HEAD after the push, now against the branch after the bundle.
    // Narrowing the window was the wrong instinct both times. Pinning the sha
    // into a ref of our own closes it by construction — the bundle cannot
    // carry anything else, whatever the branch does next.
    let sent_sha = git(&l.repo, &["rev-parse", branch])?.stdout.trim().to_string();
    let send_ref = format!("refs/tmhaul/send-{stamp}");
    git(&l.repo, &["update-ref", &send_ref, &sent_sha])?;
    let bundled = git(&l.repo, &["bundle", "create", &bundle.to_string_lossy(), &send_ref]);
    let _ = git(&l.repo, &["update-ref", "-d", &send_ref]);
    bundled?;
    let local_md5 = md5_file(&bundle).map_err(|e| e.to_string())?;

    let remote_bundle = format!("~/tmhaul-{stamp}.bundle");
    gitcmd::run(&l.repo, &wsx, &["push", &bundle.to_string_lossy(), &remote_bundle])?;

    // The `+` force applies only to `tmhaul-incoming`, a scratch ref on the
    // render box that this harness owns: a rebase gives our commits new shas,
    // so updating it is legitimately not a fast-forward. The push to `main` is
    // NOT forced, so a real divergence still fails loudly instead of
    // clobbering somebody's work.
    let script = format!(
        "set -e; \
         if [ ! -d ~/haul-push ]; then git clone -q github-tmtas:vjeux/trackmania-tas.git ~/haul-push; fi; \
         cd ~/haul-push; \
         git fetch {remote_bundle} +{send_ref}:refs/heads/tmhaul-incoming; \
         git push origin refs/heads/tmhaul-incoming:refs/heads/{branch}; \
         git rev-parse refs/heads/tmhaul-incoming; \
         rm -f {remote_bundle}"
    );
    let out = gitcmd::run(&l.repo, &ws, &[&script])?;
    let remote_sha = out.stdout.trim().lines().last().unwrap_or("").to_string();
    if remote_sha != sent_sha {
        return Err(format!(
            "the box pushed {remote_sha} but the bundle carried {sent_sha} — the transfer did \
             not deliver what we built"
        ));
    }
    let _ = std::fs::remove_file(&bundle);
    Ok(format!(
        "whitestick→github {} (bundle md5 {}){}",
        &sent_sha[..12],
        &local_md5[..8],
        rebased.map(|r| format!(" [{r}]")).unwrap_or_default()
    ))
}

/// Same bounded retry as the bridge route: the race is the remote moving, not
/// the transport.
/// Push an arbitrary local ref to a NAMED BRANCH on the remote, through the
/// bridge. Never `main`.
///
/// This exists for work that must be made durable on GitHub without being
/// merged: two sessions can produce lineages that genuinely conflict, and the
/// honest move is to bank both and let whoever owns the code adjudicate.
/// Resolving somebody else's engine source by picking a side would silently
/// drop landed work, and in this project a wrong resolution produces
/// plausible wrong numbers rather than an error.
pub fn push_ref_via_whitestick(l: &Layout, local_ref: &str, remote_branch: &str) -> Result<String, String> {
    if remote_branch == "main" || remote_branch == "master" {
        return Err(format!(
            "refusing to push {local_ref} straight at {remote_branch}: that is what the normal \
             bank does, with a rebase and a conflict check in front of it"
        ));
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/var/svcscm".into());
    let ws = format!("{home}/bin/whitestick");
    let wsx = format!("{home}/bin/wsx");
    if !Path::new(&ws).exists() {
        return Err(format!("no bridge binary at {ws}"));
    }
    let sha = gitcmd::git(&l.repo, &["rev-parse", local_ref])?.stdout.trim().to_string();
    let stamp = format!("{}-{}", std::process::id(), crate::time::now());
    let bundle = std::env::temp_dir().join(format!("tmhaul-ref-{stamp}.bundle"));
    let _ = std::fs::remove_file(&bundle);
    gitcmd::git(&l.repo, &["bundle", "create", &bundle.to_string_lossy(), local_ref])?;
    let remote_bundle = format!("~/tmhaul-ref-{stamp}.bundle");
    gitcmd::run(&l.repo, &wsx, &["push", &bundle.to_string_lossy(), &remote_bundle])?;
    let script = format!(
        "set -e; \
         if [ ! -d ~/haul-push ]; then git clone -q github-tmtas:vjeux/trackmania-tas.git ~/haul-push; fi; \
         cd ~/haul-push; \
         git fetch {remote_bundle} +{local_ref}:refs/heads/tmhaul-ref-incoming; \
         git push origin refs/heads/tmhaul-ref-incoming:refs/heads/{remote_branch}; \
         git rev-parse refs/heads/tmhaul-ref-incoming; \
         rm -f {remote_bundle}"
    );
    let out = gitcmd::run(&l.repo, &ws, &[&script])?;
    let remote_sha = out.stdout.trim().lines().last().unwrap_or("").to_string();
    if remote_sha != sha {
        return Err(format!(
            "the box pushed {remote_sha} but {local_ref} is {sha} — the bundle did not carry what \
             we think it did"
        ));
    }
    let _ = std::fs::remove_file(&bundle);
    Ok(format!("{remote_branch} -> {sha}"))
}

pub fn push_direct(l: &Layout, branch: &str) -> Result<String, String> {
    let mut last = String::new();
    for _ in 0..3 {
        let rebased = sync_with_remote(l, branch)?;
        let o = gitcmd::git_env(&l.repo, &["push", "origin", &format!("HEAD:{branch}")])?;
        if o.code == 0 {
            return Ok(format!(
                "direct→github {}{}",
                &gitcmd::head_sha(&l.repo)?[..12],
                rebased.map(|r| format!(" [{r}]")).unwrap_or_default()
            ));
        }
        last = o.stderr.trim().to_string();
        if !(last.contains("fetch first") || last.contains("non-fast-forward") || last.contains("rejected")) {
            return Err(last);
        }
    }
    Err(format!("{last} (gave up after 3 attempts; the remote kept moving)"))
}

#[allow(dead_code)]
fn push_direct_unused(l: &Layout, branch: &str) -> Result<String, String> {
    let rebased = sync_with_remote(l, branch)?;
    git(&l.repo, &["push", "origin", &format!("HEAD:{branch}")])?;
    Ok(format!(
        "direct→github {}{}",
        &gitcmd::head_sha(&l.repo)?[..12],
        rebased.map(|r| format!(" [{r}]")).unwrap_or_default()
    ))
}

// ------------------------------------------------------------------ bank

pub struct Options {
    pub message: String,
    pub mirror: Mirror,
    pub mirror_dir: Option<std::path::PathBuf>,
    pub push: Push,
    pub branch: String,
}

pub fn bank(l: &Layout, node: &str, o: &Options) -> Result<Receipt, String> {
    let mut r = Receipt::default();

    let secrets = scan_for_secrets(&l.root())?;
    if !secrets.is_empty() {
        return Err(format!(
            "refusing to bank: the state tree contains what look like credentials, and the repo is PUBLIC:\n  {}",
            secrets.join("\n  ")
        ));
    }

    r.files_hashed = write_manifest(l)?;

    git(&l.repo, &["add", "-A", "autopilot"])?;
    let staged = gitcmd::try_run(&l.repo, "git", &["diff", "--cached", "--quiet"])?;
    if staged.code == 0 {
        r.commit_skipped_reason = Some("no change".into());
    } else {
        git(&l.repo, &["commit", "-q", "-m", &o.message])?;
        r.committed = Some(gitcmd::head_sha(&l.repo)?);
    }

    let sha = gitcmd::head_sha(&l.repo)?;
    match o.mirror {
        Mirror::None => {}
        Mirror::Paste => match mirror_to_paste(l, node, &sha) {
            Ok(id) => r.mirror = Some(id),
            Err(e) => r.mirror_error = Some(e),
        },
        Mirror::Dir => {
            let dir = o.mirror_dir.clone().ok_or("mirror=dir with no directory")?;
            match (|| -> Result<String, String> {
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let body = pack::pack(&l.state(), &l.root(), node).map_err(|e| e.to_string())?;
                let f = dir.join(format!("state-{}-{}.haulpack", crate::time::now(), node));
                std::fs::write(&f, body).map_err(|e| e.to_string())?;
                Ok(f.to_string_lossy().to_string())
            })() {
                Ok(p) => r.mirror = Some(p),
                Err(e) => r.mirror_error = Some(e),
            }
        }
    }

    match o.push {
        Push::None => {}
        Push::Direct => match push_direct(l, &o.branch) {
            Ok(s) => r.pushed = Some(s),
            Err(e) => r.push_error = Some(e),
        },
        Push::Whitestick => match push_via_whitestick(l, &o.branch) {
            Ok(s) => r.pushed = Some(s),
            Err(e) => r.push_error = Some(e),
        },
    }

    // When the push route is unavailable, the *state* is still safe — the
    // paste mirror carries it — but any COMMIT that is not state (a harness
    // fix, a new tool, a corrected HARNESS.md) exists only on this box, and
    // this box is designed to be thrown away at the end of its lease. Send
    // those commits down the transport that still works.
    //
    // The trigger is simply "GitHub does not have them", not "a push failed".
    // The first version ran only on a push error, which silently excluded the
    // box that fails hardest: one with no credential at all, where push is
    // switched *off* and there is no error to react to. `code_mirror_if_new`
    // is a no-op when GitHub is level and when this source was already sent,
    // so this is cheap on a healthy day.
    match code_mirror_if_new(l, node, &o.branch) {
        Ok(Some(id)) => r.code_mirror = Some(id),
        Ok(None) => {}
        Err(e) => r.code_mirror_error = Some(brief_error(&e)),
    }

    Ok(r)
}

/// Mirror unpushed commits as a code pack, unless this head was already sent.
///
/// "Already sent" is read from the journal rather than kept in memory: the
/// supervisor restarts, and a restart that re-sends every pack would be the
/// same bug as the budget that re-counted its resume point.
/// Should this box send its unpushed commits, and what would it send?
///
/// Separated from the publishing so the *decision* is testable without a paste
/// service — and so it is obvious by reading it that the decision does not
/// consult the push settings at all. It asks only: does GitHub have this
/// source, and have we already sent this exact source?
pub fn code_mirror_needed(
    l: &Layout,
    branch: &str,
) -> Result<Option<(codemirror::CodePack, String)>, String> {
    let Some(pack) = codemirror::build(&l.repo, branch)? else {
        return Ok(None);
    };
    // Keyed on the CONTENT the state mirror does not carry, not on the head:
    // the head moves every time the supervisor banks, and re-sending the same
    // source every ten minutes would bury the one paste that matters.
    let key = codemirror::content_key(&l.repo)?;
    let already = crate::log::read_all(&l.journal_dir())?
        .iter()
        .any(|rec| rec.kind == "code_mirror" && rec.get("key") == Some(key.as_str()));
    if already {
        return Ok(None);
    }
    Ok(Some((pack, key)))
}

/// Mirror unpushed commits as a code pack, unless this source was already sent.
///
/// "Already sent" is read from the journal rather than kept in memory: the
/// supervisor restarts, and a restart that re-sent every pack would be the
/// same bug as the budget that re-counted its resume point.
pub fn code_mirror_if_new(l: &Layout, node: &str, branch: &str) -> Result<Option<String>, String> {
    let Some((pack, key)) = code_mirror_needed(l, branch)? else {
        return Ok(None);
    };
    let id = codemirror::publish(node, &pack)?;
    let lg = crate::log::Log::shard(&l.journal_dir(), node, crate::time::now())
        .map_err(|e| e.to_string())?;
    lg.append(
        &crate::rec::Rec::new("code_mirror")
            .f("head", &pack.head)
            .f("base", &pack.base)
            .f("key", &key)
            .f("paste", &id)
            .f("bytes", pack.bytes),
    )
    .map_err(|e| e.to_string())?;
    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("haul-bank-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn a_box_with_no_push_credential_still_mirrors_its_code() {
        // The hole the 15:30Z rotation opened: the first version mirrored code
        // only when a push FAILED. A fresh box with no credential has push
        // switched off — no failure, no error, and so no mirror. That is the
        // box most likely to be holding the only copy of something, because it
        // is the one that cannot push at all.
        let root = tmp("nopush");
        gitcmd::run(&root, "git", &["init", "-q", "--bare", "-b", "main", "origin.git"]).unwrap();
        gitcmd::run(
            &root,
            "git",
            &["clone", "-q", &root.join("origin.git").to_string_lossy(), "box"],
        )
        .unwrap();
        let repo = root.join("box");
        let g = |args: &[&str]| gitcmd::git(&repo, args).unwrap();
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "boxA"]);
        std::fs::create_dir_all(repo.join("tools")).unwrap();
        std::fs::write(repo.join("tools/lib.rs"), "// v1").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "root"]);
        g(&["push", "-q", "origin", "main"]);

        let l = Layout::new(&repo);
        for d in l.all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }

        // Level with GitHub: nothing to send, whatever the push setting.
        assert!(code_mirror_needed(&l, "main").unwrap().is_none());

        // A local fix. The decision must say yes without ever being told how
        // (or whether) this box pushes.
        std::fs::write(repo.join("tools/lib.rs"), "// v2 — the fix").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "a fix nobody else has"]);
        let (pack, key) = code_mirror_needed(&l, "main").unwrap().expect("must want to mirror");
        assert_eq!(pack.head, gitcmd::head_sha(&repo).unwrap());

        // And once it has been sent, it stops asking — across a restart,
        // because the record is on disk rather than in memory.
        crate::log::Log::shard(&l.journal_dir(), "boxA", 1)
            .unwrap()
            .append(&crate::rec::Rec::at(1, "code_mirror").f("key", &key).f("paste", "P1"))
            .unwrap();
        assert!(code_mirror_needed(&l, "main").unwrap().is_none());
    }

    #[test]
    fn a_banked_receipt_never_carries_a_transports_error_body() {
        // The receipt is committed to a PUBLIC repo and rendered into a
        // markdown table. A credentialed transport's error body belongs in
        // neither: it can echo request context, and it is multi-line.
        let r = Receipt {
            push_error: Some(
                "/bin/wsx push /tmp/x.bundle exited 1: wsx: retrying (remote command failed \
                 (exit status: 1): printf %s \"$HOME\"\n  stderr: [whitestick] error: instance \
                 offline)\n\nwsx: gave up"
                    .into(),
            ),
            ..Default::default()
        };
        let s = r.summary();
        assert!(!s.contains('\n'), "one line: {s}");
        assert!(s.contains("PUSH FAILED"), "{s}");
        assert!(s.contains("render box is offline"), "{s}");
        assert!(!s.contains("$HOME"), "no request context: {s}");

        // An unrecognised error is truncated, not interpreted.
        let long = "x".repeat(500);
        let r2 = Receipt { push_error: Some(long), ..Default::default() };
        let s2 = r2.summary();
        assert!(s2.chars().count() < 260, "{}", s2.len());
        assert!(s2.ends_with('…'), "{s2}");
    }

    #[test]
    fn brief_error_squeezes_whitespace() {
        assert_eq!(brief_error("  a\n\n  b\t c  "), "a b c");
    }

    #[test]
    fn the_secret_guard_fires_on_a_private_key() {
        let d = tmp("secret-key");
        std::fs::write(d.join("oops.txt"), "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n").unwrap();
        let hits = scan_for_secrets(&d).unwrap();
        assert_eq!(hits.len(), 1, "{hits:?}");
    }

    #[test]
    fn the_secret_guard_fires_on_the_bridge_token_file() {
        // The literal thing this harness handled today: a 161-byte navi
        // credentials.json. It must never reach a public repo.
        let d = tmp("secret-navi");
        std::fs::write(
            d.join("credentials.json"),
            "{\"token\":\"x\",\"naviUrl\":\"https://navibot.dev\"}",
        )
        .unwrap();
        assert_eq!(scan_for_secrets(&d).unwrap().len(), 1);
    }

    #[test]
    fn the_secret_guard_is_quiet_on_ordinary_state() {
        // The control. A guard that flags everything gets turned off.
        let d = tmp("secret-clean");
        std::fs::write(
            d.join("journal.rec"),
            "2026-08-24T18:00:00Z\tsample\tevals=1200\tbest=25\n",
        )
        .unwrap();
        std::fs::write(d.join("STATUS.md"), "# status\n\nrung 1, 23.144 to beat\n").unwrap();
        assert_eq!(scan_for_secrets(&d).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_push_survives_somebody_else_committing_to_the_repo() {
        // The real event, on the first afternoon: an unrelated session pushed
        // between two banks and the next push was rejected. A long-haul system
        // that stops banking when a colleague commits is not one.
        let root = tmp("rebase");
        let origin = root.join("origin.git");
        let mine = root.join("mine");
        let theirs = root.join("theirs");
        let g = |d: &std::path::Path, args: &[&str]| gitcmd::git(d, args).unwrap();

        gitcmd::run(&root, "git", &["init", "-q", "--bare", "-b", "main", "origin.git"]).unwrap();
        for (dir, who) in [(&mine, "mine"), (&theirs, "theirs")] {
            gitcmd::run(
                &root,
                "git",
                &["clone", "-q", &origin.to_string_lossy(), &dir.to_string_lossy()],
            )
            .unwrap();
            g(dir, &["config", "user.email", "t@t"]);
            g(dir, &["config", "user.name", who]);
        }
        // A first commit so both clones share a root.
        std::fs::write(mine.join("README.md"), "x").unwrap();
        g(&mine, &["add", "-A"]);
        g(&mine, &["commit", "-q", "-m", "root"]);
        g(&mine, &["push", "-q", "origin", "main"]);
        g(&theirs, &["pull", "-q", "origin", "main"]);

        // They push something of their own.
        std::fs::write(theirs.join("their-file.md"), "their work").unwrap();
        g(&theirs, &["add", "-A"]);
        g(&theirs, &["commit", "-q", "-m", "their work"]);
        g(&theirs, &["push", "-q", "origin", "main"]);

        // We commit ours, not knowing.
        let l = Layout::new(&mine);
        for d in l.all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(l.journal_dir().join("boxA-1.rec"), "2026-08-24T00:00:00Z\tsample\n").unwrap();
        g(&mine, &["add", "-A"]);
        g(&mine, &["commit", "-q", "-m", "our bank"]);

        let before = gitcmd::head_sha(&mine).unwrap();
        let note = sync_with_remote(&l, "main").unwrap();
        assert!(note.is_some(), "a diverged branch must be rebased");
        assert_ne!(gitcmd::head_sha(&mine).unwrap(), before);

        // Now the push is a fast-forward, and both pieces of work survive.
        g(&mine, &["push", "-q", "origin", "main"]);
        assert!(mine.join("their-file.md").exists(), "their commit must still be there");
        assert!(l.journal_dir().join("boxA-1.rec").exists(), "and so must ours");

        // Control: with nothing new upstream, sync must do nothing at all.
        let steady = gitcmd::head_sha(&mine).unwrap();
        assert_eq!(sync_with_remote(&l, "main").unwrap(), None);
        assert_eq!(gitcmd::head_sha(&mine).unwrap(), steady);
    }

    #[test]
    fn a_push_retries_when_the_remote_moves_between_the_fetch_and_the_push() {
        // The TOCTOU race, reproduced: sync, then let somebody else land a
        // commit, then push. A single-shot sync-then-push fails here, and it
        // failed twice in this harness's first hour from unrelated work in
        // the same repo.
        let root = tmp("race");
        let origin = root.join("origin.git");
        let mine = root.join("mine");
        let theirs = root.join("theirs");
        let g = |d: &std::path::Path, args: &[&str]| gitcmd::git(d, args).unwrap();
        gitcmd::run(&root, "git", &["init", "-q", "--bare", "-b", "main", "origin.git"]).unwrap();
        for (dir, who) in [(&mine, "mine"), (&theirs, "theirs")] {
            gitcmd::run(&root, "git", &["clone", "-q", &origin.to_string_lossy(), &dir.to_string_lossy()]).unwrap();
            g(dir, &["config", "user.email", "t@t"]);
            g(dir, &["config", "user.name", who]);
        }
        std::fs::write(mine.join("README.md"), "x").unwrap();
        g(&mine, &["add", "-A"]);
        g(&mine, &["commit", "-q", "-m", "root"]);
        g(&mine, &["push", "-q", "origin", "main"]);
        g(&theirs, &["pull", "-q", "origin", "main"]);

        let l = Layout::new(&mine);
        std::fs::write(mine.join("ours.md"), "our bank").unwrap();
        g(&mine, &["add", "-A"]);
        g(&mine, &["commit", "-q", "-m", "our bank"]);

        // We sync against the remote as it is NOW...
        sync_with_remote(&l, "main").unwrap();
        // ...and only then does somebody else land theirs.
        std::fs::write(theirs.join("theirs.md"), "their work").unwrap();
        g(&theirs, &["add", "-A"]);
        g(&theirs, &["commit", "-q", "-m", "their work"]);
        g(&theirs, &["push", "-q", "origin", "main"]);

        // A single-shot push would be rejected here. The retrying one wins.
        let out = push_direct(&l, "main").unwrap();
        assert!(out.starts_with("direct→github"), "{out}");
        assert!(mine.join("theirs.md").exists(), "their commit must survive");
        assert!(mine.join("ours.md").exists(), "and so must ours");
    }

    #[test]
    fn the_manifest_notices_a_changed_byte() {
        let repo = tmp("manifest");
        let l = Layout::new(&repo);
        for d in l.all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(l.journal_dir().join("a.rec"), "2026-08-24T18:00:00Z\tstart\n").unwrap();
        assert_eq!(write_manifest(&l).unwrap(), 1);
        assert_eq!(verify_manifest(&l).unwrap(), Vec::<String>::new());

        std::fs::write(l.journal_dir().join("a.rec"), "2026-08-24T18:00:00Z\tstop\n").unwrap();
        let bad = verify_manifest(&l).unwrap();
        assert_eq!(bad.len(), 1, "a changed file must be caught: {bad:?}");
    }

    #[test]
    fn a_deleted_file_is_a_verification_failure_not_a_pass() {
        // An absent row is not a passing row — the manifest must fail loudly
        // when a file it names has gone.
        let repo = tmp("manifest-gone");
        let l = Layout::new(&repo);
        for d in l.all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(l.journal_dir().join("a.rec"), "2026-08-24T18:00:00Z\tstart\n").unwrap();
        write_manifest(&l).unwrap();
        std::fs::remove_file(l.journal_dir().join("a.rec")).unwrap();
        assert_eq!(verify_manifest(&l).unwrap().len(), 1);
    }
}
