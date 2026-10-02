//! `tinyctl discard-table --out-root R --tags tiny,x2 --maps 01-25 --out DIR` —
//! the DISCARD INVENTORY over a set of builds: every build's `discard.tsv`
//! (tmmaps::discard, written by `tinyctl build`) read from
//! `<out-root>/tinyNN/<tag>/discard.tsv`, concatenated into `DIR/detail.tsv`
//! (one `build` column prepended) and summed into `DIR/table.tsv` (rows = the
//! codes with their loss kind, columns = the builds, cells = placement counts)
//! plus `DIR/by-class.tsv` (per code: the loss kind, total placements, the
//! builds it touches, the distinct names). The table is what the inventory
//! reads first (2026-10-01).

use std::path::PathBuf;

fn maps_of(spec: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                for n in a..=b {
                    out.push(format!("{n:02}"));
                }
                continue;
            }
        }
        if !part.is_empty() {
            out.push(part.to_string());
        }
    }
    out
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let out_root = PathBuf::from(f("--out-root").ok_or("discard-table needs --out-root DIR")?);
    let tags: Vec<String> = f("--tags").unwrap_or_else(|| "tiny".into()).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let maps = maps_of(&f("--maps").unwrap_or_else(|| "01-25".into()));
    let out = PathBuf::from(f("--out").ok_or("discard-table needs --out DIR")?);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut reports: Vec<(String, String)> = Vec::new();
    let mut detail = format!("build\t{}\n", tmmaps::discard::HEADER);
    let mut missing: Vec<String> = Vec::new();
    for tag in &tags {
        for nn in &maps {
            let label = format!("{tag}-{nn}");
            let p = out_root.join(format!("tiny{nn}")).join(tag).join("discard.tsv");
            match std::fs::read_to_string(&p) {
                Ok(t) => {
                    for l in t.lines().skip(1).filter(|l| !l.trim().is_empty()) {
                        detail.push_str(&label);
                        detail.push('\t');
                        detail.push_str(l);
                        detail.push('\n');
                    }
                    reports.push((label, t));
                }
                Err(_) => missing.push(label),
            }
        }
    }
    std::fs::write(out.join("detail.tsv"), &detail).map_err(|e| format!("detail.tsv: {e}"))?;
    let table = tmmaps::discard::table(&reports);
    std::fs::write(out.join("table.tsv"), &table).map_err(|e| format!("table.tsv: {e}"))?;
    // per class: loss, total, builds touched, distinct names (up to 12)
    {
        use std::collections::{BTreeMap, BTreeSet};
        let mut per: BTreeMap<String, (usize, BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
        for (label, text) in &reports {
            for l in text.lines().skip(1) {
                let fl: Vec<&str> = l.split('\t').collect();
                if fl.len() < 9 {
                    continue;
                }
                let e = per.entry(fl[3].to_string()).or_default();
                e.0 += fl[7].parse::<usize>().unwrap_or(0);
                e.1.insert(label.clone());
                e.2.insert(fl[5].to_string());
            }
        }
        let mut s = String::from("code\tloss\tplacements\tbuilds\tdistinct_names\tnames\n");
        for (code, (n, builds, names)) in &per {
            let mut shown: Vec<&str> = names.iter().map(|x| x.as_str()).take(12).collect();
            if names.len() > 12 {
                shown.push("…");
            }
            s.push_str(&format!("{code}\t{}\t{n}\t{}\t{}\t{}\n", tmmaps::discard::loss_of(code), builds.len(), names.len(), shown.join(" ")));
        }
        std::fs::write(out.join("by-class.tsv"), &s).map_err(|e| format!("by-class.tsv: {e}"))?;
        print!("{s}");
    }
    println!("{} builds read, {} rows -> {}", reports.len(), detail.lines().count().saturating_sub(1), out.display());
    if !missing.is_empty() {
        println!("no discard.tsv for: {}", missing.join(" "));
    }
    Ok(())
}
