//! **Getting a code fix onto a fresh box when GitHub push is down.**
//!
//! The harness has three durability layers for *state* — commit, HAULPACK
//! mirror into a paste, push to GitHub — and until 2026-08-26 exactly one for
//! *code*: GitHub. That asymmetry was invisible while the push bridge worked
//! and became the whole problem the morning the render box went offline:
//!
//! - a fresh box bootstraps by cloning `github.com/vjeux/trackmania-tas` and
//!   building `tools/`, so it gets whatever code GitHub last received;
//! - `tmhaul recover` restores `autopilot/state/` from the paste mirror, and
//!   nothing else — a HAULPACK is state, deliberately;
//! - so with the bridge down, a commit that fixes the harness lives only on
//!   the box that wrote it, and rotation is *designed* to throw that box away.
//!
//! One outage plus one routine rotation is enough to lose a day of work while
//! every alarm stays quiet, because nothing in the system considers unpushed
//! source to be at risk. The state mirror's transport does not depend on the
//! bridge at all — it is a Phabricator paste, reached with the box's own x509
//! cert — so this module sends the *commits* the same way: a git bundle,
//! base64'd into a paste, titled so a fresh box can find it with nothing but
//! its cert.
//!
//! Deliberately a bundle against `origin/main` rather than a patch: git
//! verifies it, it carries the real commits (messages, authorship, parents)
//! rather than a reconstruction, and a receiver that already has the base
//! applies it with `git fetch`. A bundle whose base the receiver lacks is
//! refused loudly by git, which is the right failure.

use std::path::Path;

use crate::gitcmd::{self, git};
use crate::md5::md5_hex;
use crate::pack::{b64_decode, b64_encode};
use crate::paths::Layout;

pub const CODE_TITLE_PREFIX: &str = "TMHAUL-CODE";

/// The text that goes in the paste: a header a human can read, then base64.
///
/// The header is not decoration. A fresh box has to decide *before decoding*
/// whether this bundle is one it can use — which base commit it needs, and
/// what the bytes should hash to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodePack {
    pub base: String,
    pub head: String,
    pub bytes: usize,
    pub md5: String,
    pub body: String,
}

impl CodePack {
    pub fn render(&self) -> String {
        format!(
            "TMHAULCODE 1\nbase {}\nhead {}\nbytes {}\nmd5 {}\n--\n{}\n",
            self.base, self.head, self.bytes, self.md5, self.body
        )
    }

    /// Parse and *check*. A truncated paste is the failure this guards: it
    /// decodes to plausible bytes and produces a bundle git will reject in a
    /// confusing way, hours later, on a box with no operator.
    pub fn parse(text: &str) -> Result<CodePack, String> {
        let mut base = String::new();
        let mut head = String::new();
        let mut bytes = 0usize;
        let mut md5 = String::new();
        let mut body = String::new();
        let mut in_body = false;
        let mut saw_magic = false;

        for line in text.lines() {
            if in_body {
                body.push_str(line.trim());
                continue;
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "--" {
                in_body = true;
                continue;
            }
            if let Some(v) = line.strip_prefix("TMHAULCODE ") {
                if v.trim() != "1" {
                    return Err(format!("unknown code-pack version {v:?}"));
                }
                saw_magic = true;
            } else if let Some(v) = line.strip_prefix("base ") {
                base = v.to_string();
            } else if let Some(v) = line.strip_prefix("head ") {
                head = v.to_string();
            } else if let Some(v) = line.strip_prefix("bytes ") {
                bytes = v.parse().map_err(|_| format!("bad byte count {v:?}"))?;
            } else if let Some(v) = line.strip_prefix("md5 ") {
                md5 = v.to_string();
            }
        }
        if !saw_magic {
            return Err("not a code pack: no TMHAULCODE header".into());
        }
        if base.is_empty() || head.is_empty() || md5.is_empty() {
            return Err("code pack header is missing base, head or md5".into());
        }
        if !in_body || body.is_empty() {
            return Err("code pack has no body".into());
        }
        let decoded = b64_decode(&body)?;
        if decoded.len() != bytes {
            return Err(format!(
                "code pack is truncated: header says {bytes} bytes, body decodes to {}",
                decoded.len()
            ));
        }
        let got = md5_hex(&decoded);
        if got != md5 {
            return Err(format!("code pack md5 mismatch: header {md5}, body {got}"));
        }
        Ok(CodePack { base, head, bytes, md5, body })
    }

    pub fn decode(&self) -> Result<Vec<u8>, String> {
        b64_decode(&self.body)
    }
}

/// A temp path no other call in this process will pick.
///
/// Pid alone is not enough: two `build`s in one process reuse the name, and
/// git leaves a `<name>.lock` behind when the first one fails — after which
/// every later bundle in that process fails for a reason that has nothing to
/// do with the bundle.
fn scratch(kind: &str, ext: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("tmhaul-{kind}-{}-{nanos}-{n}.{ext}", std::process::id()))
}

/// A digest of everything in the repo that the STATE mirror does not carry.
///
/// Why not just the head sha: while the push route is down, the supervisor
/// commits state every ten minutes, so the head moves constantly. Keying the
/// code mirror on the head would publish a paste every ten minutes saying the
/// same thing about the same source — 144 a day, each one indistinguishable
/// from a real change.
///
/// The state mirror carries `autopilot/state/` and nothing else, so what is at
/// risk on this box is: `tools/` (the harness itself) and everything under
/// `autopilot/` that is not `state/` (HARNESS.md, OPS-LOG.md, the job spec,
/// the map registry — the documents a fresh box and a human both need).
pub fn content_key(repo: &Path) -> Result<String, String> {
    let tools = git(repo, &["rev-parse", "HEAD:tools"])
        .map(|o| o.stdout.trim().to_string())
        .unwrap_or_else(|_| "no-tools".into());
    let autopilot = git(repo, &["ls-tree", "HEAD:autopilot"])
        .map(|o| o.stdout)
        .unwrap_or_default();
    let mut material = format!("tools:{tools}\n");
    for line in autopilot.lines() {
        // `<mode> <type> <sha>\t<name>`
        let name = line.split('\t').nth(1).unwrap_or("");
        if name == "state" {
            continue;
        }
        material.push_str(line.trim());
        material.push('\n');
    }
    Ok(md5_hex(material.as_bytes()))
}

/// Build a pack of everything on `branch` that `origin/<branch>` has not got.
///
/// Returns `Ok(None)` when the remote is already level — mirroring nothing is
/// not a failure, it is the ordinary case on a healthy day.
pub fn build(repo: &Path, branch: &str) -> Result<Option<CodePack>, String> {
    // Read-only fetch: this is the half of git that still works without the
    // push credential, and it is what makes `base` mean "what GitHub has"
    // rather than "what we last heard".
    let _ = gitcmd::git_env(repo, &["fetch", "-q", "origin", branch]);
    let base = git(repo, &["rev-parse", &format!("origin/{branch}")])?.stdout.trim().to_string();
    let head = git(repo, &["rev-parse", branch])?.stdout.trim().to_string();
    if base == head {
        return Ok(None);
    }
    let range = format!("{base}..{head}");
    let tmp = scratch("code", "bundle");
    // Same discipline as the state bundle: pin the range through a ref we own,
    // so a concurrent commit cannot move what we are packing between the
    // rev-parse and the bundle.
    let send_ref = format!("refs/tmhaul/code-{}", &head[..12.min(head.len())]);
    git(repo, &["update-ref", &send_ref, &head])?;
    let made = git(
        repo,
        &["bundle", "create", &tmp.to_string_lossy(), &format!("{base}..{send_ref}")],
    );
    let _ = git(repo, &["update-ref", "-d", &send_ref]);
    made.map_err(|e| format!("git bundle create {range}: {e}"))?;

    let raw = std::fs::read(&tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    Ok(Some(CodePack {
        base,
        head,
        bytes: raw.len(),
        md5: md5_hex(&raw),
        body: b64_encode(&raw),
    }))
}

/// Publish the pack as a paste. Same transport as the state mirror, same
/// reason: it needs the box's own cert and nothing else.
pub fn publish(node: &str, p: &CodePack) -> Result<String, String> {
    let tmp = scratch("codepack", "txt");
    std::fs::write(&tmp, p.render()).map_err(|e| e.to_string())?;
    let title = format!(
        "{CODE_TITLE_PREFIX} {node} {} head={} base={}",
        crate::time::iso(crate::time::now()),
        &p.head[..12.min(p.head.len())],
        &p.base[..12.min(p.base.len())]
    );
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "meta phabricator.paste create --title {} --stdin --output=json < {}",
            shell_quote(&title),
            shell_quote(&tmp.to_string_lossy())
        ))
        .output()
        .map_err(|e| format!("spawn meta: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        return Err(format!(
            "paste create failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    stdout
        .split("\"id\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no paste id in: {}", stdout.trim()))
}

/// Newest code pack, by paste title.
pub fn latest() -> Result<Option<(String, String)>, String> {
    let out = std::process::Command::new("meta")
        .args([
            "phabricator.paste",
            "list",
            &format!("--title-contains={CODE_TITLE_PREFIX}"),
            "--limit=20",
            "--output=json",
        ])
        .output()
        .map_err(|e| format!("spawn meta: {e}"))?;
    if !out.status.success() {
        return Err(format!("paste list failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    Ok(newest_in_listing(&text))
}

/// Pure over the listing text, so the ordering rule is testable without a
/// paste service: newest `created` wins, and a listing with no code packs is
/// `None` rather than an error.
pub fn newest_in_listing(text: &str) -> Option<(String, String)> {
    let mut best: Option<(String, String, i64)> = None;
    for chunk in text.split("{\"id\":\"").skip(1) {
        let Some(id) = chunk.split('"').next() else { continue };
        let title = chunk
            .split("\"title\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("")
            .to_string();
        if !title.starts_with(CODE_TITLE_PREFIX) {
            continue;
        }
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
    best.map(|(i, t, _)| (i, t))
}

/// What happened when a box tried to take a code pack on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// Fast-forwarded onto the pack's head.
    Advanced { from: String, to: String, commits: usize },
    /// The checkout already contains it.
    AlreadyHave(String),
    /// The pack does not apply here, and why. Never a silent no-op.
    Refused(String),
}

/// Apply a pack to a checkout, refusing anything that is not a fast-forward.
///
/// A code mirror that could rewrite a box's checkout would be a worse hazard
/// than the gap it fills; the only move it is allowed to make is the one a
/// fresh box needs — take commits it does not have, on top of the base it
/// already has.
pub fn apply(repo: &Path, branch: &str, p: &CodePack) -> Result<Applied, String> {
    let head = git(repo, &["rev-parse", branch])?.stdout.trim().to_string();
    if head == p.head {
        return Ok(Applied::AlreadyHave(p.head.clone()));
    }
    if !gitcmd::is_clean(repo)? {
        return Ok(Applied::Refused(
            "the checkout has uncommitted changes; not touching it".into(),
        ));
    }
    // The base has to be an ancestor of nothing-in-particular here: what
    // matters is that we HAVE it. Without it the bundle is unusable and git
    // says so, but a clear refusal beats a git error a woken agent has to
    // interpret at 03:00.
    if git(repo, &["cat-file", "-e", &format!("{}^{{commit}}", p.base)]).is_err() {
        return Ok(Applied::Refused(format!(
            "this checkout does not have the pack's base commit {} — clone from GitHub first",
            &p.base[..12.min(p.base.len())]
        )));
    }
    let raw = p.decode()?;
    let tmp = scratch("code-in", "bundle");
    std::fs::write(&tmp, &raw).map_err(|e| e.to_string())?;
    let verified = git(repo, &["bundle", "verify", &tmp.to_string_lossy()]);
    if let Err(e) = verified {
        let _ = std::fs::remove_file(&tmp);
        return Ok(Applied::Refused(format!("git refused the bundle: {e}")));
    }
    let fetch_ref = format!("refs/tmhaul/incoming-{}", &p.head[..12.min(p.head.len())]);
    let r = git(
        repo,
        &["fetch", "-q", &tmp.to_string_lossy(), &format!("+{}:{}", p.head, fetch_ref)],
    );
    let _ = std::fs::remove_file(&tmp);
    r?;

    // Fast-forward only. A divergence means two boxes wrote code at once,
    // which is a thing to report, not to merge unattended.
    let ff = git(repo, &["merge-base", "--is-ancestor", &head, &p.head]).is_ok();
    if !ff {
        let _ = git(repo, &["update-ref", "-d", &fetch_ref]);
        return Ok(Applied::Refused(format!(
            "not a fast-forward: this box is at {} and the pack is at {} — \
             two boxes have written code; a human should look",
            &head[..12.min(head.len())],
            &p.head[..12.min(p.head.len())]
        )));
    }
    let count: usize = git(repo, &["rev-list", "--count", &format!("{head}..{}", p.head)])?
        .stdout
        .trim()
        .parse()
        .unwrap_or(0);
    git(repo, &["merge", "-q", "--ff-only", &p.head])?;
    let _ = git(repo, &["update-ref", "-d", &fetch_ref]);
    Ok(Applied::Advanced { from: head, to: p.head.clone(), commits: count })
}

pub fn read_paste(id: &str) -> Result<String, String> {
    let out = std::process::Command::new("meta")
        .args(["phabricator.paste", "read", &format!("--id={id}")])
        .output()
        .map_err(|e| format!("spawn meta: {e}"))?;
    if !out.status.success() {
        return Err(format!("paste read failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// How many commits this checkout is holding that GitHub has not got.
///
/// `None` means *unknown*, not zero: no `origin/<branch>` ref (a checkout that
/// has never fetched, a test rig, a box mid-bootstrap). The distinction is the
/// point — "I cannot tell" printed as "0 commits at risk" is exactly the class
/// of quiet zero this project keeps paying for.
pub fn unpushed_code(l: &Layout, branch: &str) -> Result<Option<usize>, String> {
    let n = gitcmd::unpushed(&l.repo, &format!("origin/{branch}"))?;
    Ok(if n == usize::MAX { None } else { Some(n) })
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("haul-code-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn g(d: &Path, args: &[&str]) {
        gitcmd::git(d, args).unwrap();
    }

    fn origin_and_clone(root: &Path, name: &str) -> std::path::PathBuf {
        let dir = root.join(name);
        gitcmd::run(root, "git", &["clone", "-q", &root.join("origin.git").to_string_lossy(), name])
            .unwrap();
        g(&dir, &["config", "user.email", "t@t"]);
        g(&dir, &["config", "user.name", name]);
        dir
    }

    /// The whole point, end to end: a box with an unpushed fix, a fresh box
    /// that only ever saw GitHub, and no push route between them.
    #[test]
    fn a_fix_reaches_a_fresh_box_with_the_push_route_dead() {
        let root = tmp("roundtrip");
        gitcmd::run(&root, "git", &["init", "-q", "--bare", "-b", "main", "origin.git"]).unwrap();
        let a = origin_and_clone(&root, "boxA");
        std::fs::write(a.join("README.md"), "root").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "root"]);
        g(&a, &["push", "-q", "origin", "main"]);

        // A fresh box clones what GitHub has — the state of the world before
        // the fix.
        let b = origin_and_clone(&root, "boxB");
        assert!(!b.join("fix.rs").exists());

        // boxA writes the fix and CANNOT push it.
        std::fs::write(a.join("fix.rs"), "the fix").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "fix the status page"]);

        let pack = build(&a, "main").unwrap().expect("there is something to mirror");
        assert_eq!(pack.md5.len(), 32);

        // Round-trip through the text a paste would carry.
        let text = pack.render();
        let back = CodePack::parse(&text).unwrap();
        assert_eq!(back.head, pack.head);

        match apply(&b, "main", &back).unwrap() {
            Applied::Advanced { commits, to, .. } => {
                assert_eq!(commits, 1);
                assert_eq!(to, pack.head);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(std::fs::read_to_string(b.join("fix.rs")).unwrap(), "the fix");

        // Idempotent: applying it again is a no-op, not a second merge.
        assert_eq!(apply(&b, "main", &back).unwrap(), Applied::AlreadyHave(pack.head.clone()));

        // Control: with nothing unpushed there is nothing to mirror.
        g(&a, &["push", "-q", "origin", "main"]);
        assert_eq!(build(&a, "main").unwrap(), None);
    }

    #[test]
    fn a_state_only_commit_does_not_count_as_new_code() {
        // The flood this prevents: with the push route down the supervisor
        // commits state every ten minutes, so the head moves constantly. Keyed
        // on the head, the code mirror would publish a paste every ten minutes
        // about source nobody had touched.
        let root = tmp("contentkey");
        gitcmd::run(&root, "git", &["init", "-q", "--bare", "-b", "main", "origin.git"]).unwrap();
        let a = origin_and_clone(&root, "boxA");
        std::fs::create_dir_all(a.join("tools/haul/src")).unwrap();
        std::fs::create_dir_all(a.join("autopilot/state/journal")).unwrap();
        std::fs::write(a.join("tools/haul/src/lib.rs"), "// v1").unwrap();
        std::fs::write(a.join("autopilot/HARNESS.md"), "# how it works").unwrap();
        std::fs::write(a.join("autopilot/state/journal/boxA-1.rec"), "1\tsample\n").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "root"]);
        let k0 = content_key(&a).unwrap();

        // A bank: state only.
        std::fs::write(a.join("autopilot/state/journal/boxA-1.rec"), "1\tsample\n2\tsample\n").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "autopilot: periodic"]);
        assert_eq!(content_key(&a).unwrap(), k0, "state churn is not new code");

        // A source change.
        std::fs::write(a.join("tools/haul/src/lib.rs"), "// v2").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "a fix"]);
        assert_ne!(content_key(&a).unwrap(), k0);

        // A document that the state mirror does not carry counts too: losing
        // HARNESS.md with the box is losing the recovery instructions.
        let k2 = content_key(&a).unwrap();
        std::fs::write(a.join("autopilot/HARNESS.md"), "# how it works, corrected").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "docs"]);
        assert_ne!(content_key(&a).unwrap(), k2);
    }

    #[test]
    fn a_truncated_paste_is_refused_rather_than_decoded() {
        // The failure that matters for this transport: a paste that came back
        // short. It decodes to *something*; only the length and the digest
        // catch it.
        let p = CodePack {
            base: "a".repeat(40),
            head: "b".repeat(40),
            bytes: 6,
            md5: md5_hex(b"abcdef"),
            body: b64_encode(b"abcdef"),
        };
        assert!(CodePack::parse(&p.render()).is_ok());

        let mut short = p.clone();
        short.body = b64_encode(b"abc");
        let e = CodePack::parse(&short.render()).unwrap_err();
        assert!(e.contains("truncated"), "{e}");

        let mut wrong = p.clone();
        wrong.md5 = md5_hex(b"something else");
        let e = CodePack::parse(&wrong.render()).unwrap_err();
        assert!(e.contains("md5"), "{e}");

        let e = CodePack::parse("hello, not a code pack").unwrap_err();
        assert!(e.contains("TMHAULCODE"), "{e}");
    }

    #[test]
    fn a_pack_whose_base_is_missing_is_refused_clearly() {
        let root = tmp("nobase");
        gitcmd::run(&root, "git", &["init", "-q", "--bare", "-b", "main", "origin.git"]).unwrap();
        let a = origin_and_clone(&root, "boxA");
        std::fs::write(a.join("README.md"), "root").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "root"]);
        let p = CodePack {
            base: "0".repeat(40),
            head: "1".repeat(40),
            bytes: 3,
            md5: md5_hex(b"abc"),
            body: b64_encode(b"abc"),
        };
        match apply(&a, "main", &p).unwrap() {
            Applied::Refused(why) => assert!(why.contains("base commit"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn divergent_code_is_refused_not_merged() {
        let root = tmp("diverge");
        gitcmd::run(&root, "git", &["init", "-q", "--bare", "-b", "main", "origin.git"]).unwrap();
        let a = origin_and_clone(&root, "boxA");
        std::fs::write(a.join("README.md"), "root").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "root"]);
        g(&a, &["push", "-q", "origin", "main"]);
        let b = origin_and_clone(&root, "boxB");

        std::fs::write(a.join("a.rs"), "a").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "A's work"]);
        let pack = build(&a, "main").unwrap().unwrap();

        // boxB wrote its own commit in the meantime.
        std::fs::write(b.join("b.rs"), "b").unwrap();
        g(&b, &["add", "-A"]);
        g(&b, &["commit", "-q", "-m", "B's work"]);

        match apply(&b, "main", &pack).unwrap() {
            Applied::Refused(why) => assert!(why.contains("fast-forward"), "{why}"),
            other => panic!("{other:?}"),
        }
        assert!(b.join("b.rs").exists(), "B's own work must be untouched");
        assert!(!b.join("a.rs").exists());
    }

    #[test]
    fn a_dirty_checkout_is_left_alone() {
        let root = tmp("dirty");
        gitcmd::run(&root, "git", &["init", "-q", "--bare", "-b", "main", "origin.git"]).unwrap();
        let a = origin_and_clone(&root, "boxA");
        std::fs::write(a.join("README.md"), "root").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "root"]);
        std::fs::write(a.join("scratch.txt"), "half-written").unwrap();
        g(&a, &["add", "-A"]);
        std::fs::write(a.join("scratch.txt"), "still writing").unwrap();
        let p = CodePack {
            base: gitcmd::head_sha(&a).unwrap(),
            head: "1".repeat(40),
            bytes: 3,
            md5: md5_hex(b"abc"),
            body: b64_encode(b"abc"),
        };
        match apply(&a, "main", &p).unwrap() {
            Applied::Refused(why) => assert!(why.contains("uncommitted"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_newest_code_paste_wins_and_other_pastes_are_ignored() {
        let listing = concat!(
            "[{\"id\":\"P1\",\"title\":\"TMHAUL-STATE boxA sha=aaa\",\"created\":\"300\"},",
            "{\"id\":\"P2\",\"title\":\"TMHAUL-CODE boxA head=bbb\",\"created\":\"100\"},",
            "{\"id\":\"P3\",\"title\":\"TMHAUL-CODE boxB head=ccc\",\"created\":\"200\"}]"
        );
        let (id, title) = newest_in_listing(listing).unwrap();
        assert_eq!(id, "P3");
        assert!(title.contains("head=ccc"));
        assert_eq!(newest_in_listing("[]"), None);
    }
}
