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
            (None, Some(e)) => parts.push(format!("MIRROR FAILED: {e}")),
            (None, None) => parts.push("mirror off".into()),
        }
        match (&self.pushed, &self.push_error) {
            (Some(w), _) => parts.push(format!("push {w}")),
            (None, Some(e)) => parts.push(format!("PUSH FAILED: {e}")),
            (None, None) => parts.push("push off".into()),
        }
        parts.join(" · ")
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
    let fetch = gitcmd::try_run(&l.repo, "git", &["fetch", "-q", "origin", branch])?;
    if fetch.code != 0 {
        // No network or no read access: not fatal here. The push will fail
        // next and say so with its own error.
        return Ok(None);
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
    let rebase = gitcmd::try_run(&l.repo, "git", &["rebase", "FETCH_HEAD"])?;
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
pub fn push_via_whitestick(l: &Layout, branch: &str) -> Result<String, String> {
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
    git(&l.repo, &["bundle", "create", &bundle.to_string_lossy(), branch])?;
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
         git fetch {remote_bundle} +{branch}:refs/heads/tmhaul-incoming; \
         git push origin refs/heads/tmhaul-incoming:refs/heads/{branch}; \
         git rev-parse refs/heads/tmhaul-incoming; \
         rm -f {remote_bundle}"
    );
    let out = gitcmd::run(&l.repo, &ws, &[&script])?;
    let remote_sha = out.stdout.trim().lines().last().unwrap_or("").to_string();
    let local_sha = gitcmd::head_sha(&l.repo)?;
    if remote_sha != local_sha {
        return Err(format!(
            "the box pushed {remote_sha} but our HEAD is {local_sha} — the bundle did not carry what we think it did"
        ));
    }
    let _ = std::fs::remove_file(&bundle);
    Ok(format!(
        "whitestick→github {} (bundle md5 {}){}",
        &local_sha[..12],
        &local_md5[..8],
        rebased.map(|r| format!(" [{r}]")).unwrap_or_default()
    ))
}

pub fn push_direct(l: &Layout, branch: &str) -> Result<String, String> {
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

    Ok(r)
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
