//! `tinyctl loadloop --maps A.Map.Gbx[,B…] --tag T [--seq 0,1,…] [--n N] [--how play|edit]
//! [--timeout 300] [--settle-ms 3000] [--fresh|--fresh-first] [--shot-on-fail] [--outdir /tmp/tiny3]`
//!
//! The load-reliability loop on the render box (`shootctl loadloop`), driven
//! from the devserver: the maps are pushed once, the loop runs detached over
//! there under the render lock, and the per-load table comes back as
//! `OUTDIR/loadloop-<tag>.tsv` with a one-line tally printed here:
//! how many loads OPENED, how many raised a DIALOG (and what it said), how
//! many timed out (the black screen), how many crashed the client.
//!
//! This is the instrument of the "Missing Items: AC00000000.Item.Gbx" failure
//! of the big embedded archives (2026-09-07/08): 21 fails roughly one load in
//! three while the named item is present and valid. Whether the trigger is
//! the entry count, the archive bytes or the order maps are switched in is a
//! question of `--maps`/`--seq`/`--fresh`, not of opinion.

use std::path::PathBuf;
use std::time::Duration;

use crate::wsx::Wsx;

const STAGE: &str = "/home/vjeux/shoot/_stage";
const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<PathBuf> = f("--maps").ok_or("loadloop needs --maps A[,B,…]")?.split(',').filter(|s| !s.is_empty()).map(PathBuf::from).collect();
    let tag = f("--tag").ok_or("loadloop needs --tag T")?;
    if !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("--tag {tag}: letters, digits, - and _ only"));
    }
    let timeout: u64 = f("--timeout").map(|s| s.parse().map_err(|_| "--timeout wants seconds")).transpose()?.unwrap_or(300);
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/tmp/tiny3".into()));
    let shootctl = f("--box-shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let wsx = Wsx::new(args);
    let mut remote = Vec::new();
    for (i, m) in maps.iter().enumerate() {
        if !m.exists() {
            return Err(format!("{}: no such file", m.display()));
        }
        // the box keeps the local file name (the table names maps by it)
        let name = m.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("map path has no file name")?;
        let r = format!("{STAGE}/ll-{tag}-{i}-{name}");
        eprintln!("pushing {} to the box …", m.display());
        wsx.push(m, &r)?;
        remote.push(r);
    }
    let remote_dir = format!("{SHOTS}/loadloop-{tag}");
    let mut pass = String::new();
    for k in ["--seq", "--n", "--how", "--settle-ms"] {
        if let Some(v) = f(k) {
            pass.push_str(&format!(" {k} {v}"));
        }
    }
    for k in ["--fresh", "--fresh-first", "--shot-on-fail"] {
        if tmmaps::cli::has(args, k) {
            pass.push_str(&format!(" {k}"));
        }
    }
    let cmd = format!("{shootctl} loadloop --detach --maps {} --outdir {remote_dir} --tag {tag} --timeout {timeout}{pass}", remote.join(","));
    let n_loads = f("--n").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1) * f("--seq").map(|s| s.split(',').count()).unwrap_or(maps.len());
    eprintln!("running {n_loads} loads on the box — waits for the render lock if another thread holds the game …");
    let started = wsx.sh(&cmd)?;
    if wsx.verbose {
        eprintln!("{}", started.trim());
    }
    // the lock wait (1500 s) + every load's timeout + the menu round trips
    let budget = Duration::from_secs(1500 + n_loads as u64 * (timeout + 60));
    let done = wsx.wait_done(&format!("{remote_dir}/done-loadloop.txt"), &format!("{remote_dir}/loadloop.log"), budget, &format!("loadloop {tag}"));
    // the table is worth pulling even when the loop failed part-way
    let tsv = format!("loadloop-{tag}.tsv");
    let local = outdir.join(&tsv);
    match wsx.pull(&format!("{remote_dir}/{tsv}"), &local) {
        Ok(n) => eprintln!("  pulled {tsv} ({n} B)"),
        Err(e) => eprintln!("  {tsv}: {e}"),
    }
    let log_local = outdir.join(format!("loadloop-{tag}.log"));
    if let Ok(n) = wsx.pull(&format!("{remote_dir}/loadloop.log"), &log_local) {
        eprintln!("  pulled loadloop-{tag}.log ({n} B)");
    }
    done?;
    let text = std::fs::read_to_string(&local).map_err(|e| format!("{}: {e}", local.display()))?;
    print!("{}", summarize(&text));
    println!("table {}", local.display());
    Ok(())
}

/// The tally of a loadloop table: outcomes, load times of the opens, the
/// dialog texts seen.
pub fn summarize(tsv: &str) -> String {
    let mut out = String::new();
    let rows: Vec<Vec<&str>> = tsv.lines().skip(1).filter(|l| !l.trim().is_empty()).map(|l| l.split('\t').collect()).collect();
    let n = rows.len();
    let count = |o: &str| rows.iter().filter(|r| r.get(2) == Some(&o)).count();
    let (opened, dialog, timeout, crash) = (count("OPENED"), count("DIALOG"), count("TIMEOUT"), count("CRASH"));
    out.push_str(&format!("{n} loads: {opened} OPENED, {dialog} DIALOG, {timeout} TIMEOUT, {crash} CRASH\n"));
    let mut secs: Vec<f64> = rows.iter().filter(|r| r.get(2) == Some(&"OPENED")).filter_map(|r| r.get(3).and_then(|s| s.parse().ok())).collect();
    if !secs.is_empty() {
        secs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out.push_str(&format!("  open times: min {:.1} s, median {:.1} s, max {:.1} s\n", secs[0], secs[secs.len() / 2], secs[secs.len() - 1]));
    }
    let no_car = rows.iter().filter(|r| r.get(2) == Some(&"OPENED") && r.get(4).map(|c| c.starts_with("NO")).unwrap_or(false)).count();
    if no_car > 0 {
        out.push_str(&format!("  {no_car} opened WITHOUT a car\n"));
    }
    for r in rows.iter().filter(|r| r.get(2) != Some(&"OPENED")) {
        out.push_str(&format!("  #{} {} {} after {} s: {} {}\n", r.first().unwrap_or(&""), r.get(1).unwrap_or(&""), r.get(2).unwrap_or(&""), r.get(3).unwrap_or(&""), r.get(5).unwrap_or(&""), r.get(6).unwrap_or(&"")));
    }
    // per map, when several
    let mut maps: Vec<&str> = rows.iter().filter_map(|r| r.get(1).copied()).collect();
    maps.sort();
    maps.dedup();
    if maps.len() > 1 {
        for m in maps {
            let mine: Vec<&Vec<&str>> = rows.iter().filter(|r| r.get(1) == Some(&m)).collect();
            let ok = mine.iter().filter(|r| r.get(2) == Some(&"OPENED")).count();
            out.push_str(&format!("  {m}: {ok} of {} opened\n", mine.len()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tally_counts_outcomes() {
        let t = "iter\tmap\toutcome\tseconds\tcar\tdialog_frame\tdialog_text\tctx\tnote\n\
                 0\ta\tOPENED\t14.0\tyes [1 2 3]\t-\t-\t{}\t\n\
                 1\ta\tDIALOG\t9.5\t-\tFrameMessage\tMissing Items: X\t{}\t\n\
                 2\ta\tOPENED\t16.0\tNO (err)\t-\t-\t{}\t\n";
        let s = summarize(t);
        assert!(s.contains("3 loads: 2 OPENED, 1 DIALOG, 0 TIMEOUT, 0 CRASH"), "{s}");
        assert!(s.contains("median 16.0"), "{s}");
        assert!(s.contains("1 opened WITHOUT a car"), "{s}");
        assert!(s.contains("Missing Items: X"), "{s}");
    }
}
