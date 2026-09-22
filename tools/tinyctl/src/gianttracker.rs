//! `tinyctl giant-tracker --summary S.tsv [--startcheck R2.tsv,R3.tsv,R4.tsv] [--lightmap L.tsv]
//!                        [--publish P2.tsv,P3.tsv,P4.tsv] [--out T.md]` — the giant
//! campaigns' tracker as one Markdown table (2026-09-22): one row per (scale,
//! map) joined from the build summary (name, uid, times, grid, decoration, pool
//! tiles, bytes), the start-check reports, the lightmap batch report and the
//! publish results (mapId, stored md5 verdict). Missing columns print `-`.

use std::collections::HashMap;
use std::path::PathBuf;

fn rows(path: &str) -> Vec<Vec<String>> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).map(|l| l.split('\t').map(String::from).collect()).collect()
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let summary = f("--summary").ok_or("giant-tracker needs --summary S.tsv (scale, nn, uid, name, times, grid, decoration, tiles, bytes, note)")?;
    let out = PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/giant/TRACKER.md".into()));
    // start-check rows keyed by the map path's tinyNN/xK
    let key_of = |p: &str| -> Option<String> {
        let mut it = p.split('/').rev();
        let _file = it.next()?;
        let tag = it.next()?.to_string(); // xK
        let dir = it.next()?.to_string(); // tinyNN
        Some(format!("{}\t{}", tag, dir.trim_start_matches("tiny")))
    };
    let mut startcheck: HashMap<String, String> = HashMap::new();
    for p in f("--startcheck").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
        for r in rows(p).into_iter().skip(1) {
            if let (Some(k), Some(v)) = (r.first().and_then(|p| key_of(p)), r.get(2)) {
                startcheck.insert(k, v.clone());
            }
        }
    }
    let mut lightmap: HashMap<String, (String, String)> = HashMap::new();
    for p in f("--lightmap").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
        for r in rows(p).into_iter().skip(1) {
            // copy path …/tinyNN/xK-bake/…: key from the OUT path (…/lit/xK/Summer-NN-Giant.Map.Gbx)
            if let Some(outp) = r.get(1) {
                let file = outp.rsplit('/').next().unwrap_or("");
                let nn = file.split('-').nth(1).unwrap_or("").to_string();
                let tag = outp.rsplit('/').nth(1).unwrap_or("").to_string();
                lightmap.insert(format!("{tag}\t{nn}"), (r.get(2).cloned().unwrap_or_default(), r.get(5).cloned().unwrap_or_default()));
            }
        }
    }
    // publish results: path name uid mapId how bytes md5 verdict
    let mut publish: HashMap<String, (String, String, String)> = HashMap::new();
    for p in f("--publish").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
        for r in rows(p) {
            if r.len() >= 8 {
                if let Some(k) = r.get(2).map(|uid| uid.clone()) {
                    publish.insert(k, (r[3].clone(), r[6].clone(), r[7].clone()));
                }
            }
        }
    }
    let mut md = String::new();
    md.push_str("| scale | map | name | uid | author | gold | silver | bronze | grid | decoration | pool tiles | bytes | item-check | start-check | lightmap | Nadeo mapId | stored md5 |\n");
    md.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let mut n = 0usize;
    for r in rows(&summary) {
        if r.len() < 8 {
            continue;
        }
        let (scale, nn, uid, name) = (&r[0], &r[1], &r[2], &r[3]);
        let times: Vec<&str> = r[4].split_whitespace().collect();
        let t = |i: usize| times.get(i).map(|s| tmmaps::secs::secs_str(s)).unwrap_or_else(|| "-".into());
        let key = format!("{scale}\t{nn}");
        let sc = startcheck.get(&key).cloned().unwrap_or_else(|| "-".into());
        let (lm, lm_bytes) = lightmap.get(&key).cloned().unwrap_or_else(|| ("-".into(), String::new()));
        let bytes = if lm == "ok" && !lm_bytes.is_empty() { lm_bytes.clone() } else { r.get(8).cloned().unwrap_or_default() };
        let (map_id, md5, verdict) = publish.get(uid).cloned().unwrap_or_else(|| ("-".into(), "-".into(), String::new()));
        let md5_col = if verdict.is_empty() { "-".to_string() } else { format!("{} {}", &md5[..md5.len().min(8)], verdict) };
        let over = r.get(9).map(|o| o.trim()).filter(|o| !o.is_empty()).map(|o| format!(" ({o})")).unwrap_or_default();
        md.push_str(&format!("| {scale} | {nn} | {name} | `{uid}` | {} | {} | {} | {} | {}{over} | {} | {} | {} | ok | {} | {} | {} | {} |\n", t(0), t(1), t(2), t(3), r[5], r[6], r[7], bytes, sc, lm, map_id, md5_col));
        n += 1;
    }
    std::fs::write(&out, &md).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("{}: {n} rows", out.display());
    Ok(())
}
