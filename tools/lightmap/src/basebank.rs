//! `lmtool base-bank` — THE LANDING, banked in one command (baker-6, 2026-09-28): what baker-3/4/5 typed as a shell chain
//! at every base (format-patch → bundle → binary → TSV → guards → gate log, every store write .tmp + mv), so the store
//! layout is written once and the announcement text comes out ready.
//!
//!     lmtool base-bank --repo DIR --prev TIP --bin LMTOOL --gate-md GATE.md --note TEXT
//!                      [--store DIR] [--branch base-2026-09-25] [--guards DIR] [--giants-md FILE] [--refs NAME] [--dry-run]
//!
//! `--repo` is the integration clone (HEAD = the new base, clean tree, on `--branch`); `--prev` the previous base's tip
//! (must be an ancestor); `--bin` the GATED binary (its md5 must be the one GATE.md names — the base's binary is the
//! gate's own build, never a re-build: tools/mapgeom/build.rs embeds the commit hash into every lmtool); `--gate-md` the
//! harness-gate table (must read ALL PASS); `--note` the "landed on PREV: …" line; `--giants-md` the giant-rule read
//! (corpus-gate report of the two exact giants) appended to the gate log; `--guards` the pinned d1 baselines
//! ({stpad,tiny16,tiny04ac}-d1-base.Map.Gbx; default STORE-independent /tmp/pd/guard).
//!
//! Writes, under `--store` (default ~/persistent/private-30d/tm-player):
//!   tiny/patches/lightmap-integration/NNNN-*.patch   the series prev..HEAD, numbered after the last one present
//!   bundles/trackmania-tas-base-<tip>.bundle          + trackmania-tas-base-CURRENT.bundle (the same bytes)
//!   mh/lmtool-<tip>                                   the gated binary
//!   mh/corpus-gate/corpus-gate-<tip>.tsv              the repo's tools/lightmap/data/corpus-gate.tsv
//!   mh/corpus-gate/guard-{stpad,tiny16,tiny04ac}-d1-<tip>.Map.Gbx
//!   mh/corpus-gate/gate-<tip>.md                      note + giants read + the gate table
//! and prints the announcement (tip, series, bundle md5, binary md5, refs, gate line) to paste to the lanes.
use std::path::{Path, PathBuf};
use std::process::Command;

fn flag(args: &[String], k: &str) -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() }
fn has(args: &[String], k: &str) -> bool { args.iter().any(|a| a == k) }

fn git(repo: &Path, a: &[&str]) -> Result<String, String> {
    let o = Command::new("git").arg("-C").arg(repo).args(a).output().map_err(|e| format!("git: {e}"))?;
    if !o.status.success() { return Err(format!("git {}: {}", a.join(" "), String::from_utf8_lossy(&o.stderr).trim())); }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Copy `from` to `to` as `to.tmp` + rename (the store serves torn views of a file being overwritten).
fn bank(from: &Path, to: &Path, dry: bool) -> Result<(), String> {
    if dry { println!("  (dry) {} → {}", from.display(), to.display()); return Ok(()); }
    let tmp = PathBuf::from(format!("{}.tmp", to.display()));
    std::fs::copy(from, &tmp).map_err(|e| format!("copy {} → {}: {e}", from.display(), tmp.display()))?;
    std::fs::rename(&tmp, to).map_err(|e| format!("mv {} → {}: {e}", tmp.display(), to.display()))?;
    println!("  banked {}", to.display());
    Ok(())
}

fn md5_of(p: &Path) -> Result<String, String> { Ok(crate::corpusgate::md5_hex(&std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?)) }

pub fn run(args: &[String]) -> Result<(), String> {
    let repo = PathBuf::from(flag(args, "--repo").ok_or("--repo DIR (the integration clone at the new base)")?);
    let prev = flag(args, "--prev").ok_or("--prev TIP (the previous base)")?;
    let bin = PathBuf::from(flag(args, "--bin").ok_or("--bin LMTOOL (the gated binary)")?);
    let gate_md = PathBuf::from(flag(args, "--gate-md").ok_or("--gate-md GATE.md (the harness-gate table)")?);
    let note = flag(args, "--note").ok_or("--note TEXT (what landed)")?;
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
    let store = PathBuf::from(flag(args, "--store").unwrap_or_else(|| format!("{home}/persistent/private-30d/tm-player")));
    let branch = flag(args, "--branch").unwrap_or_else(|| "base-2026-09-25".into());
    let guards = PathBuf::from(flag(args, "--guards").unwrap_or_else(|| "/tmp/pd/guard".into()));
    let giants_md = flag(args, "--giants-md").map(PathBuf::from);
    let dry = has(args, "--dry-run");

    // the repo: on the branch, clean, prev an ancestor
    let on = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if on != branch { return Err(format!("{} is on {on}, not {branch}", repo.display())); }
    let dirty = git(&repo, &["status", "--porcelain"])?;
    if !dirty.is_empty() { return Err(format!("{} has uncommitted changes:\n{dirty}", repo.display())); }
    let tip = git(&repo, &["rev-parse", "--short=10", "HEAD"])?;
    git(&repo, &["merge-base", "--is-ancestor", &prev, "HEAD"]).map_err(|_| format!("{prev} is not an ancestor of HEAD {tip}"))?;
    let prev_full = git(&repo, &["rev-parse", &prev])?;
    let n_commits: usize = git(&repo, &["rev-list", "--count", &format!("{prev_full}..HEAD")])?.parse().map_err(|e| format!("rev-list: {e}"))?;
    if n_commits == 0 { return Err(format!("nothing to land: HEAD {tip} == {prev}")); }

    // the gate table: ALL PASS, and its binary md5 = --bin's
    let gate = std::fs::read_to_string(&gate_md).map_err(|e| format!("{}: {e}", gate_md.display()))?;
    let head_line = gate.lines().next().unwrap_or("").to_string();
    if !head_line.contains("ALL PASS") { return Err(format!("{}: first line is not ALL PASS: {head_line}", gate_md.display())); }
    let bin_md5 = md5_of(&bin)?;
    let gate_md5 = gate.lines().find_map(|l| l.strip_prefix("binary ")).and_then(|l| l.split(" md5 ").nth(1)).map(|s| s.split(';').next().unwrap_or("").trim().to_string()).unwrap_or_default();
    if gate_md5 != bin_md5 { return Err(format!("--bin md5 {bin_md5} is not the gate's binary ({gate_md5}): bank the gate's own build")); }
    println!("base {tip} ({n_commits} commit(s) on {prev}); gate: {head_line}; binary md5 {bin_md5}");

    // the series: numbered after the last patch present
    let intdir = store.join("tiny/patches/lightmap-integration");
    let mut last = 0usize;
    for e in std::fs::read_dir(&intdir).map_err(|e| format!("{}: {e}", intdir.display()))?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Ok(n) = name.chars().take(4).collect::<String>().parse::<usize>() { last = last.max(n); }
    }
    let first_no = last + 1;
    let tmp = PathBuf::from(format!("/tmp/base-bank-{tip}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    git(&repo, &["format-patch", "--start-number", &first_no.to_string(), "-o", &tmp.to_string_lossy(), &format!("{prev_full}..HEAD")])?;
    let mut patches: Vec<PathBuf> = std::fs::read_dir(&tmp).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "patch").unwrap_or(false)).collect();
    patches.sort();
    if patches.len() != n_commits { return Err(format!("format-patch wrote {} patches for {n_commits} commits", patches.len())); }
    println!("series {:04}–{:04}:", first_no, first_no + n_commits - 1);
    for p in &patches { bank(p, &intdir.join(p.file_name().unwrap()), dry)?; }

    // the bundle: per-hash + CURRENT
    let bundle = tmp.join(format!("trackmania-tas-base-{tip}.bundle"));
    git(&repo, &["bundle", "create", &bundle.to_string_lossy(), &branch])?;
    git(&repo, &["bundle", "verify", &bundle.to_string_lossy()])?;
    let bundle_md5 = md5_of(&bundle)?;
    bank(&bundle, &store.join(format!("bundles/trackmania-tas-base-{tip}.bundle")), dry)?;
    bank(&bundle, &store.join("bundles/trackmania-tas-base-CURRENT.bundle"), dry)?;

    // the binary, the TSV, the guards
    bank(&bin, &store.join(format!("mh/lmtool-{tip}")), dry)?;
    let tsv = repo.join("tools/lightmap/data/corpus-gate.tsv");
    bank(&tsv, &store.join(format!("mh/corpus-gate/corpus-gate-{tip}.tsv")), dry)?;
    let n_cells = std::fs::read_to_string(&tsv).map(|t| t.lines().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#') && !l.starts_with("cell\t")).count()).unwrap_or(0);
    for g in ["stpad", "tiny16", "tiny04ac"] {
        let src = guards.join(format!("{g}-d1-base.Map.Gbx"));
        if !src.is_file() { return Err(format!("no guard baseline {}", src.display())); }
        bank(&src, &store.join(format!("mh/corpus-gate/guard-{g}-d1-{tip}.Map.Gbx")), dry)?;
    }

    // the gate log
    let now = Command::new("date").args(["-u", "+%Y-%m-%d %H:%MZ"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let mut log = format!("# gate — base {tip} (baker-6, {now})\nlanded on {prev}: {note}\n\n");
    if let Some(g) = &giants_md {
        let t = std::fs::read_to_string(g).map_err(|e| format!("{}: {e}", g.display()))?;
        log += &format!("## GIANT RULE — the two exact giant cells baked with the candidate ({})\n\n{t}\n\n", g.display());
    }
    log += &gate;
    let log_path = tmp.join(format!("gate-{tip}.md"));
    std::fs::write(&log_path, &log).map_err(|e| format!("{}: {e}", log_path.display()))?;
    bank(&log_path, &store.join(format!("mh/corpus-gate/gate-{tip}.md")), dry)?;

    // the announcement
    // the refs in force: --refs NAME, else the newest mh/harness-refs-* by mtime (names do not sort by age: fc7a… > f07c…)
    let refs = flag(args, "--refs").unwrap_or_else(|| std::fs::read_dir(store.join("mh")).map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("harness-refs-")).max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok()).map(|e| e.file_name().to_string_lossy().to_string()).unwrap_or_default()).unwrap_or_default());
    println!("\n★ BASE {tip} (series 0001–{:04}) — {note}", first_no + n_commits - 1);
    println!("bundle bundles/trackmania-tas-base-{tip}.bundle (= CURRENT; md5 {bundle_md5} — fetch PER-HASH), branch {branch}; binary mh/lmtool-{tip} (md5 {bin_md5}, the gated build); refs mh/{refs}; TSV mh/corpus-gate/corpus-gate-{tip}.tsv ({n_cells} cells); guards guard-*-d1-{tip}; gate log mh/corpus-gate/gate-{tip}.md");
    println!("gate: {head_line}");
    if giants_md.is_some() { println!("giant rule: see the gate log's GIANT RULE section (the two exact giants with the candidate)"); }
    Ok(())
}
