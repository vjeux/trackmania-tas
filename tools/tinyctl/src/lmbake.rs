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
            // --base auto: lmtool's own rule (moods::base_rule), checked against ours (item_base)
            let mine: Option<u32> = if base_flag == "auto" { Some(item_base(&m)?) } else { None };
            // the atlas is 1024² whatever the map: a 254² grid (64516 ground charts) plus 20k items
            // can overflow it ("atlas full even at 20 % chart size", 24 x4) — the item texel
            // density steps down until the pack fits (1.1 default, then 0.8, 0.6, 0.45, 0.3)
            let mut err = String::new();
            let mut tpm_used: Option<&str> = None;
            let mut baked = false;
            for tpm in [None, Some("0.8"), Some("0.6"), Some("0.45"), Some("0.3")] {
                let mut c = Command::new(&lmtool);
                c.arg("bake").arg(map).arg("--out").arg(&tmp).arg("--base").arg(&base_flag);
                if template_probes {
                    c.arg("--template-probes");
                }
                if let Some(t) = tpm {
                    c.arg("--tpm").arg(t);
                }
                let out = c.output().map_err(|e| format!("lmtool: {e}"))?;
                err = String::from_utf8_lossy(&out.stderr).to_string();
                if out.status.success() {
                    tpm_used = tpm;
                    baked = true;
                    break;
                }
                if !err.contains("atlas full") {
                    return Err(format!("lmtool bake: {}", err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(200).collect::<String>()));
                }
            }
            if !baked {
                return Err("lmtool bake: atlas full even at 0.3 texels/m".into());
            }
            if let Some(t) = tpm_used {
                eprintln!("{}: the atlas was full at the default density; baked at {t} texels/m", map.display());
            }
            // the base lmtool used ("base auto: N = …")
            let used: u32 = err.lines().find_map(|l| l.strip_prefix("base auto: ")).and_then(|r| r.split(' ').next()).and_then(|n| n.parse().ok()).or_else(|| base_flag.parse().ok()).unwrap_or(0);
            if let Some(mine) = mine {
                if mine != used {
                    return Err(format!("base DISAGREES: lmtool auto {used}, the measured rule {mine} (P + S²)"));
                }
            }
            let base = used;
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

/// `tinyctl lit-verify --pairs SHIPPED=LIT,… [--report R.tsv]` — every lit file against
/// its shipped source: the header (uid, name, author time, medals, collection,
/// decoration), the item count and the block count unchanged, a lightmap chunk
/// present and bigger than a stale stub, the 25 MiB cap. One row per pair.
pub fn verify(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let pairs: Vec<(PathBuf, PathBuf)> = f("--pairs").ok_or("lit-verify needs --pairs SHIPPED=LIT,…")?.split(',').filter(|s| s.contains('=')).map(|s| { let (a, b) = s.split_once('=').unwrap(); (PathBuf::from(a.trim()), PathBuf::from(b.trim())) }).collect();
    let report = f("--report").map(PathBuf::from);
    let mut rows = vec!["lit\tuid\tname\tauthortime\tgold\tsilver\tbronze\titems\tblocks\tlightmap_bytes\tbytes\tverdict".to_string()];
    let mut bad = 0usize;
    for (ship, lit) in &pairs {
        let row = (|| -> Result<String, String> {
            let hs = tmmaps::header::read(ship.to_str().unwrap_or_default())?;
            let hl = tmmaps::header::read(lit.to_str().unwrap_or_default())?;
            let ms = tmmaps::map::MapFile::load(ship);
            let ml = tmmaps::map::MapFile::load(lit);
            let mut problems: Vec<String> = Vec::new();
            for (what, a, b) in [("uid", &hs.uid, &hl.uid), ("name", &hs.name, &hl.name), ("authortime", &hs.authortime, &hl.authortime), ("gold", &hs.gold, &hl.gold), ("silver", &hs.silver, &hl.silver), ("bronze", &hs.bronze, &hl.bronze), ("envir", &hs.envir, &hl.envir), ("mood", &hs.mood, &hl.mood)] {
                if a != b { problems.push(format!("{what} {a} -> {b}")); }
            }
            if ms.items.len() != ml.items.len() { problems.push(format!("items {} -> {}", ms.items.len(), ml.items.len())); }
            if ms.blocks.len() != ml.blocks.len() { problems.push(format!("blocks {} -> {}", ms.blocks.len(), ml.blocks.len())); }
            if ms.decoration_id != ml.decoration_id { problems.push(format!("decoration {} -> {}", ms.decoration_id, ml.decoration_id)); }
            if ms.size != ml.size { problems.push(format!("size words {:?} -> {:?}", ms.size, ml.size)); }
            let lm_ship = tmmaps::map::skip_chunks(&ms.gbx.body).into_iter().find(|(id, ..)| *id == 0x0304_305B).map(|(_, _, _, s)| s).unwrap_or(0);
            let lm = tmmaps::map::skip_chunks(&ml.gbx.body).into_iter().find(|(id, ..)| *id == 0x0304_305B).map(|(_, _, _, s)| s).unwrap_or(0);
            if lm < 100_000 { problems.push(format!("lightmap chunk {lm} bytes")); }
            if lm == lm_ship { problems.push("lightmap chunk unchanged from the shipped file".into()); }
            let bytes = std::fs::metadata(lit).map(|m| m.len()).unwrap_or(0);
            if bytes > 25 * 1024 * 1024 { problems.push(format!("{bytes} bytes over the 25 MiB cap")); }
            let verdict = if problems.is_empty() { "OK".to_string() } else { format!("BAD: {}", problems.join("; ")) };
            Ok(format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{lm}\t{bytes}\t{verdict}", lit.display(), hl.uid, hl.name, hl.authortime, hl.gold, hl.silver, hl.bronze, ml.items.len(), ml.blocks.len()))
        })().unwrap_or_else(|e| format!("{}\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\tBAD: {e}", lit.display()));
        if row.contains("\tBAD") { bad += 1; }
        println!("{}", row.rsplit('\t').next().unwrap_or(""));
        rows.push(row);
    }
    if let Some(r) = report {
        std::fs::write(&r, rows.join("\n") + "\n").map_err(|e| format!("{}: {e}", r.display()))?;
    }
    println!("lit-verify: {} pairs, {bad} bad", pairs.len());
    if bad > 0 { return Err(format!("{bad} lit files failed the verification")); }
    Ok(())
}
