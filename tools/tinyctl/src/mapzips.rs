//! `tinyctl mapzips` — publish the downloadable map files, one zip per row, as
//! GitHub user-attachments through the box's paced uploader, and write each
//! link into `rowbuilds.tsv` so `page-status` puts it on the row.
//!
//! ```text
//! tinyctl mapzips --dir ZIPS --out /tmp/tinyvid --build ship15 [--maps 05,15] [--wsx P] [--dry-run]
//! ```
//!
//! Each `ZIPS/tiny-summer-2026-NN-<build>.zip` (made by the caller: the exact
//! store map file + a README.txt with build, md5, collhash) is pushed to the
//! box once and handed to `tinyfile.sh` — the same flock, probe, page view and
//! 5-minute cooldown as a clip's ship (`tinyship.sh`), so clips and zips share
//! one queue and never overlap on the session. A row that already carries a
//! link for this build is skipped; a `FAILED cookie probe` stops the run (the
//! session is dead; nothing else would go up either). Bridge economy: one
//! push + one launch per zip, then one probe per minute.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::pagestatus::{parse_rowbuilds, RowBuild};
use crate::wsx::Wsx;

const BOX_TOOLS_DIR: &str = "/home/vjeux/trackmania-tas/tools/tinyctl/box";
const BOX_ZIPS: &str = "/home/vjeux/shoot/_mapzips";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k);
    let dir = PathBuf::from(f("--dir").ok_or("--dir ZIPS (tiny-summer-2026-NN-<build>.zip files)")?);
    let out = PathBuf::from(f("--out").ok_or("--out DIR (rowbuilds.tsv lives there)")?);
    let build = f("--build").ok_or("--build shipNN")?;
    let only: Option<Vec<String>> = f("--maps").map(|s| s.split(',').map(|m| m.trim().to_string()).collect());
    let dry = tmmaps::cli::has(args, "--dry-run");
    let wsx = Wsx::new(args);
    let rb_path = out.join("rowbuilds.tsv");

    let mut zips: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let stem = name.strip_prefix("tiny-summer-2026-")?.strip_suffix(&format!("-{build}.zip"))?;
            (stem.len() == 2 && stem.chars().all(|c| c.is_ascii_digit())).then(|| (stem.to_string(), e.path()))
        })
        .collect();
    zips.sort();
    if let Some(only) = &only {
        zips.retain(|(nn, _)| only.contains(nn));
    }
    if zips.is_empty() {
        return Err(format!("no tiny-summer-2026-NN-{build}.zip in {}", dir.display()));
    }
    let mut done = 0;
    for (nn, zip) in &zips {
        let rows = parse_rowbuilds(&std::fs::read_to_string(&rb_path).unwrap_or_default());
        if let Some(rb) = rows.get(nn.as_str()) {
            if rb.build == build && !rb.link.is_empty() {
                println!("{nn}: row already links the {build} map ({}) — skipped", rb.link);
                continue;
            }
        }
        let size = std::fs::metadata(zip).map(|m| m.len()).map_err(|e| format!("{}: {e}", zip.display()))?;
        let md5 = crate::video::md5_of(zip)?;
        println!("{nn}: {} ({:.1} MB, md5 {md5})", zip.display(), size as f64 / 1e6);
        if dry {
            continue;
        }
        let remote = format!("{BOX_ZIPS}/{}", zip.file_name().unwrap().to_string_lossy());
        wsx.push(zip, &remote)?;
        let slug = format!("mapzip-{nn}-{build}");
        let outp = format!("/mnt/c/Users/vjeux/tinyvid/ship/{slug}");
        let launch = format!(
            "mkdir -p /mnt/c/Users/vjeux/tinyvid/ship && rm -f '{outp}.done' && nohup sh {BOX_TOOLS_DIR}/tinyfile.sh '{remote}' '{slug}' '{outp}' application/zip > /dev/null 2>&1 < /dev/null & echo LAUNCHED"
        );
        let o = wsx.sh(&launch)?;
        if !o.contains("LAUNCHED") {
            return Err(format!("{nn}: launch did not answer LAUNCHED: {o}"));
        }
        println!("{nn}: launched {slug} on the box — waiting (the lock, the upload, the 5-min cooldown) …");
        let t0 = Instant::now();
        let verdict = loop {
            std::thread::sleep(Duration::from_secs(60));
            let probe = format!("if [ -f '{outp}.done' ]; then cat '{outp}.done'; else tail -n 1 '{outp}.log' 2>/dev/null; fi");
            let o = wsx.sh(&probe).unwrap_or_default();
            let o = o.trim().to_string();
            if o.starts_with("URL ") || o.starts_with("FAILED") {
                break o;
            }
            if t0.elapsed() > Duration::from_secs(45 * 60) {
                break format!("FAILED: no verdict after 45 min (last log line: {o})");
            }
            println!("  [{:>4}s] {}", t0.elapsed().as_secs(), o.chars().take(100).collect::<String>());
        };
        if let Some(url) = verdict.strip_prefix("URL ") {
            let url = url.trim().to_string();
            write_link(&rb_path, nn, &build, &url, &md5)?;
            done += 1;
            println!("{nn}: PUBLISHED {url} → rowbuilds.tsv");
        } else {
            println!("{nn}: {verdict}");
            if verdict.contains("cookie probe") {
                return Err(format!("the session is dead ({verdict}) — stopping; {done} zip(s) published this run"));
            }
        }
    }
    println!("{done} zip(s) published");
    Ok(())
}

/// Set the row's link (and build) in rowbuilds.tsv, keeping its note; a row
/// that does not exist yet is added with an empty note. The link text carries
/// the zip's md5 as a fragment so the page states which bytes it points at.
fn write_link(path: &Path, nn: &str, build: &str, url: &str, zip_md5: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut rows = parse_rowbuilds(&text);
    let e = rows.entry(nn.to_string()).or_insert_with(RowBuild::default);
    e.build = build.to_string();
    e.link = format!("{url}#md5-{}", &zip_md5[..8.min(zip_md5.len())]);
    let header: Vec<&str> = text.lines().filter(|l| l.starts_with('#')).collect();
    let mut keys: Vec<&String> = rows.keys().collect();
    keys.sort();
    let mut s = String::new();
    if header.is_empty() {
        s.push_str("# nn\tbuild\tmap_link\tnote\n");
    } else {
        for h in header {
            s.push_str(h);
            s.push('\n');
        }
    }
    for k in keys {
        let r = &rows[k];
        s.push_str(&format!("{k}\t{}\t{}\t{}\n", r.build, r.link, r.note));
    }
    let tmp = path.with_extension("tsv.tmp");
    std::fs::write(&tmp, s).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_lands_in_the_row_and_keeps_its_note() {
        let dir = std::env::temp_dir().join(format!("mapzips-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("rowbuilds.tsv");
        std::fs::write(&p, "# nn\tbuild\tmap_link\tnote\n13\tship15\t\truns on un-skinned ship15 surfaces at 2.25 s — re-drive pending\n").unwrap();
        write_link(&p, "13", "ship15", "https://github.com/user-attachments/files/1/x.zip", "0123456789abcdef").unwrap();
        write_link(&p, "05", "ship15", "https://github.com/user-attachments/files/2/y.zip", "fedcba9876543210").unwrap();
        let rows = parse_rowbuilds(&std::fs::read_to_string(&p).unwrap());
        assert_eq!(rows["13"].link, "https://github.com/user-attachments/files/1/x.zip#md5-01234567");
        assert_eq!(rows["13"].note, "runs on un-skinned ship15 surfaces at 2.25 s — re-drive pending");
        assert_eq!(rows["05"].build, "ship15");
        assert_eq!(rows["05"].note, "");
        assert!(std::fs::read_to_string(&p).unwrap().starts_with("# nn\tbuild\tmap_link\tnote\n05\t"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
