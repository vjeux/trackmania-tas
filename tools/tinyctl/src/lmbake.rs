//! `tinyctl lmbake --maps A.Map.Gbx,B.Map.Gbx,… --lit-dir DIR [--base auto|N] [--template-probes] [--report R.tsv] [--jobs J]`
//! — our own lightmap (`lmtool bake`) for shipped giant files, grafted back
//! compressed: for every map, `lmtool bake SHIPPED --out TMP --base B [--template-probes]`
//! then the transplant of chunk 0x0304305B into the shipped file
//! (`lightmap-graft`), the 25 MiB cap checked, one report row per map.
//!
//! `--base auto` (the default) is the rule measured on eight editor bakes
//! (2026-09-22, tm-player/giant-summer/ref/editor-bakes/README.md): the items
//! are the LAST objects of the lightmap's object table, at
//! `P + N_authored + S_x·S_z + G` — P = 16384 on Stadium (the 48x48 decorations,
//! NoStadium included), 0 on the terrain collections; G = the game's generated
//! pieces, 0 when the file has no authored block. A file WITH authored blocks
//! (the pool-tile maps) is refused here: its G is map-specific (2108 for the 604
//! tiles of 05 x2) and only an editor bake knows it — `tinyctl lightmap-batch`
//! is that path. Baked records in the file (BlueBay's Sea) do not count.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn item_base(m: &tmmaps::map::MapFile) -> Result<u32, String> {
    if !m.blocks.is_empty() {
        return Err(format!("{} authored blocks: the generated-piece count G is map-specific — use the editor bake (tinyctl lightmap-batch)", m.blocks.len()));
    }
    let coll = crate::views::collection_of(m);
    let p: u32 = if coll == 0x1a { 16384 } else { 0 };
    Ok(p + (m.size[0] as u32) * (m.size[2] as u32))
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<PathBuf> = f("--maps").ok_or("lmbake needs --maps A,B,…")?.split(',').filter(|s| !s.trim().is_empty()).map(|s| PathBuf::from(s.trim())).collect();
    let lit_dir = PathBuf::from(f("--lit-dir").ok_or("lmbake needs --lit-dir DIR")?);
    std::fs::create_dir_all(&lit_dir).map_err(|e| format!("{}: {e}", lit_dir.display()))?;
    let report = PathBuf::from(f("--report").unwrap_or_else(|| lit_dir.join("lmbake.tsv").display().to_string()));
    let base_flag = f("--base").unwrap_or_else(|| "auto".into());
    let template_probes = tmmaps::cli::has(args, "--template-probes");
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(4).max(1);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let lmtool = exe.with_file_name("lmtool");
    if !lmtool.exists() {
        return Err(format!("{}: no lmtool next to tinyctl", lmtool.display()));
    }
    if !report.exists() {
        std::fs::write(&report, "shipped\tlit\tbase\tverdict\tchunk_bytes\tlit_bytes\tseconds\n").map_err(|e| format!("{}: {e}", report.display()))?;
    }
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(maps.clone()));
    let rows = std::sync::Mutex::new(Vec::<String>::new());
    let failed = std::sync::atomic::AtomicUsize::new(0);
    let one = |map: &Path| -> String {
        let t0 = std::time::Instant::now();
        let file = map.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let lit = lit_dir.join(&file);
        let tmp = lit_dir.join(format!("{}.lmtool.Map.Gbx", file.trim_end_matches(".Map.Gbx")));
        let step = || -> Result<(u32, u64, u64), String> {
            let m = tmmaps::map::MapFile::load(map);
            let base: u32 = if base_flag == "auto" { item_base(&m)? } else { base_flag.parse().map_err(|_| "--base auto|N")? };
            let mut c = Command::new(&lmtool);
            c.arg("bake").arg(map).arg("--out").arg(&tmp).arg("--base").arg(base.to_string());
            if template_probes {
                c.arg("--template-probes");
            }
            let out = c.output().map_err(|e| format!("lmtool: {e}"))?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                return Err(format!("lmtool bake: {}", err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(200).collect::<String>()));
            }
            let g = Command::new(&exe).args(["lightmap-graft", "--from"]).arg(&tmp).arg("--into").arg(format!("{}={}", map.display(), lit.display())).output().map_err(|e| format!("graft: {e}"))?;
            let text = String::from_utf8_lossy(&g.stdout).to_string() + &String::from_utf8_lossy(&g.stderr);
            if !g.status.success() {
                return Err(format!("graft: {}", text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(200).collect::<String>()));
            }
            let _ = std::fs::remove_file(&tmp);
            let lm = tmmaps::map::MapFile::load(&lit);
            let chunk = tmmaps::map::skip_chunks(&lm.gbx.body).into_iter().find(|(id, ..)| *id == 0x0304_305B).map(|(_, _, _, size)| size as u64).unwrap_or(0);
            let (hu, lu) = (tmmaps::header::read(map.to_str().unwrap_or_default())?.uid, tmmaps::header::read(lit.to_str().unwrap_or_default())?.uid);
            if lm.items.len() != m.items.len() || hu != lu {
                return Err(format!("the lit file differs: {} vs {} items, uid {} vs {}", lm.items.len(), m.items.len(), lu, hu));
            }
            Ok((base, chunk, std::fs::metadata(&lit).map(|x| x.len()).unwrap_or(0)))
        };
        let (base, verdict, chunk, bytes) = match step() {
            Ok((b, c, s)) => (b.to_string(), "ok".to_string(), c, s),
            Err(e) => {
                failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let _ = std::fs::remove_file(&lit);
                ("-".to_string(), format!("FAILED: {e}"), 0, 0)
            }
        };
        println!("{file}: base {base}, {verdict} ({} s)", t0.elapsed().as_secs());
        format!("{}\t{}\t{base}\t{verdict}\t{chunk}\t{bytes}\t{}\n", map.display(), lit.display(), t0.elapsed().as_secs())
    };
    std::thread::scope(|s| {
        for _ in 0..jobs.min(maps.len()) {
            s.spawn(|| loop {
                let map = match queue.lock().unwrap().pop_front() {
                    Some(m) => m,
                    None => break,
                };
                let row = one(&map);
                rows.lock().unwrap().push(row);
            });
        }
    });
    let mut fh = std::fs::OpenOptions::new().append(true).open(&report).map_err(|e| format!("{}: {e}", report.display()))?;
    let mut rows = rows.into_inner().unwrap();
    rows.sort();
    for r in &rows {
        std::io::Write::write_all(&mut fh, r.as_bytes()).map_err(|e| e.to_string())?;
    }
    let n_failed = failed.load(std::sync::atomic::Ordering::Relaxed);
    println!("lmbake: {} maps, {} failed; report {}", maps.len(), n_failed, report.display());
    if n_failed > 0 {
        return Err(format!("{n_failed} of {} lmtool bakes failed", maps.len()));
    }
    Ok(())
}
