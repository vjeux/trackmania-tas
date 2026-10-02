//! `tinyctl campaign-tracker --title T --shipped-dir D --lit-dir L --sources S
//!   [--publish results.tsv] [--lightmap-source MAP=editor|editor-reduced|lmtool,…]
//!   [--startcheck R.tsv,…] [--out TRACKER.md]` — the campaign's tracker as one
//! Markdown table, one row per lit file in `--lit-dir` (`<Prefix>-NN-<Label>.Map.Gbx`):
//! map, name, uid, author/gold/silver/bronze (seconds), items, blocks, collection,
//! lightmap source, lightmap chunk bytes, file bytes (and the 25 MiB cap verdict),
//! start check, Nadeo mapId + stored md5 verdict from the publish results.
//! The sources' header gives the original times beside ours. Missing columns print `-`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CAP: u64 = 26_214_400;
const LIGHTMAP_CHUNK: u32 = 0x0304_305B;

fn nn_of(name: &str) -> Option<String> {
    // Fall-07-Tiny.Map.Gbx → 07
    let mut it = name.split('-');
    it.next()?;
    let nn = it.next()?;
    (nn.len() == 2 && nn.chars().all(|c| c.is_ascii_digit())).then(|| nn.to_string())
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let title = f("--title").unwrap_or_else(|| "Campaign tracker".into());
    let lit_dir = PathBuf::from(f("--lit-dir").ok_or("campaign-tracker needs --lit-dir DIR")?);
    let src_dir = f("--sources").map(PathBuf::from);
    let out = PathBuf::from(f("--out").unwrap_or_else(|| lit_dir.join("TRACKER.md").display().to_string()));
    // lightmap sources: "01=editor,02=editor-reduced,03=lmtool"
    let lm_src: BTreeMap<String, String> = f("--lightmap-source").map(|s| s.split(',').filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect()).unwrap_or_default();
    // publish results: path name uid mapId how bytes md5 verdict
    let mut publish: BTreeMap<String, (String, String, String)> = BTreeMap::new();
    if let Some(p) = f("--publish") {
        for l in std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?.lines().skip(1) {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() >= 8 && !l.starts_with('#') {
                if let Some(nn) = Path::new(c[0]).file_name().and_then(|n| n.to_str()).and_then(nn_of) {
                    publish.insert(nn, (c[3].to_string(), c[6].chars().take(8).collect(), c[7].to_string()));
                }
            }
        }
    }
    // start checks: lightmap-run reports (copy out verdict compute_s wall_s out_bytes start) — the last word per map wins
    let mut starts: BTreeMap<String, String> = BTreeMap::new();
    if let Some(list) = f("--startcheck") {
        for p in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            for l in std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?.lines().skip(1) {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() >= 7 && c[6] != "-" {
                    if let Some(nn) = Path::new(c[1]).file_name().and_then(|n| n.to_str()).and_then(nn_of) {
                        starts.insert(nn, c[6].to_string());
                    }
                }
            }
        }
    }
    // --filetime F.tsv[,…] (lmtool filetime-check --tsv): the cache word verdict per file
    let mut ft: BTreeMap<String, String> = BTreeMap::new();
    if let Some(list) = f("--filetime") {
        for p in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            for l in std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?.lines().skip(1) {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() >= 9 {
                    if let Some(nn) = Path::new(c[0]).file_name().and_then(|n| n.to_str()).and_then(nn_of) {
                        ft.insert(nn, if c[8].starts_with("EQUAL") { "EQUAL".into() } else { c[8].chars().take(40).collect() });
                    }
                }
            }
        }
    }
    // --census C.tsv[,…] (tinyctl genealogy-census): the zone-table verdict per file
    let mut cz: BTreeMap<String, String> = BTreeMap::new();
    if let Some(list) = f("--census") {
        for p in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            for l in std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?.lines().skip(1) {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() >= 14 {
                    if let Some(nn) = Path::new(c[13]).file_name().and_then(|n| n.to_str()).and_then(nn_of) {
                        cz.insert(nn, c[12].to_string());
                    }
                }
            }
        }
    }
    let secs = |ms: &str| tmmaps::secs::secs_str(ms);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&lit_dir)
        .map_err(|e| format!("{}: {e}", lit_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            n.ends_with(".Map.Gbx") && !n.contains(".copy") && !n.contains("resaved") && !n.contains("reduced") && !n.contains("check") && !n.contains("bakecopy") && nn_of(n).is_some()
        })
        .collect();
    files.sort();
    let mut md = format!("# {title}\n\n");
    md.push_str("| map | name | uid | author | gold | silver | bronze | source AT | items | blocks | collection | lightmap | lm bytes | bytes | cap | filetime | census | start | Nadeo mapId | stored md5 |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let mut n_over = 0usize;
    for p in &files {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let nn = nn_of(&name).unwrap_or_default();
        let h = tmmaps::header::read(p.to_str().unwrap_or_default())?;
        let m = tmmaps::map::MapFile::load(p);
        let lm = tmmaps::map::skip_chunks(&m.gbx.body).into_iter().find(|(id, ..)| *id == LIGHTMAP_CHUNK).map(|(_, _, _, size)| size).unwrap_or(0);
        let bytes = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let cap = if bytes < CAP { "ok".to_string() } else { n_over += 1; "OVER".to_string() };
        let src_at = src_dir
            .as_ref()
            .and_then(|d| std::fs::read_dir(d).ok())
            .and_then(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).find(|q| q.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with(&format!("{nn}-"))).unwrap_or(false)))
            .and_then(|q| tmmaps::header::read(q.to_str().unwrap_or_default()).ok())
            .map(|sh| secs(&sh.authortime))
            .unwrap_or_else(|| "-".into());
        let (map_id, md5, verdict) = publish.get(&nn).cloned().unwrap_or_else(|| ("-".into(), "-".into(), "-".into()));
        md.push_str(&format!(
            "| {nn} | {} | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} {} |\n",
            h.name,
            h.uid,
            secs(&h.authortime),
            secs(&h.gold),
            secs(&h.silver),
            secs(&h.bronze),
            src_at,
            m.items.len(),
            m.blocks.len(),
            h.envir,
            lm_src.get(&nn).cloned().unwrap_or_else(|| "-".into()),
            lm,
            bytes,
            cap,
            ft.get(&nn).cloned().unwrap_or_else(|| "-".into()),
            cz.get(&nn).cloned().unwrap_or_else(|| "-".into()),
            starts.get(&nn).cloned().unwrap_or_else(|| "-".into()),
            map_id,
            md5,
            verdict
        ));
    }
    md.push_str(&format!("\n{} files; {} over the 25 MiB cap.\n", files.len(), n_over));
    std::fs::write(&out, &md).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("wrote {} ({} rows)", out.display(), files.len());
    Ok(())
}
