//! `tinyctl retile --shipped LIT.Map.Gbx --built UNLIT.Map.Gbx --out OUT.Map.Gbx [--base N] [--no-clone]`
//!
//! The shipped file's editor lightmap carried onto a REBUILD whose item LIST
//! changed but whose item MODELS did not (2026-10-02: the terrain-tile rule fix —
//! Fall 06 lost 67 Dirt tiles the game hides under pillars and gained the
//! DirtCliff4 the game draws beside the reactor gate). The two item lists are
//! matched placement by placement (model, position, rotation, pivot, scale);
//! every shipped item's charts are renumbered to its new index (`lmtool
//! transplant --kept`), a shipped item with no counterpart loses its charts
//! (`-` in the kept list), a new item with no chart takes copies of the charts
//! of a same-model shipped item at the same yaw (`--clone`; the nearest such
//! placement — a cliff tile reads another cliff tile's atlas rect) unless
//! `--no-clone`. The embedded libraries must be identical byte for byte, or
//! nothing is written: a changed model would make every chart of that model
//! wrong, and the cache's TimeWriteMostRecentSolid check would reject the bake.
//!
//! The cache's TotalLmSurfaceMeter (chunk 0x0602200B) is the editor's sum over
//! the items IT lit; this tool leaves it as stored. Whether the client accepts a
//! cache whose scene gained or lost lightmapped items is not established — if it
//! does not, the map loads with the load-time lighting (as an unlit file does);
//! the clean path stays an editor re-bake of the rebuilt file on the box.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

fn key(it: &tmmaps::map::ItemRec) -> (String, [u32; 3], [u32; 3], [u32; 3], u32) {
    (
        it.model.clone(),
        [it.pos[0].to_bits(), it.pos[1].to_bits(), it.pos[2].to_bits()],
        [it.yaw.to_bits(), it.pitch.to_bits(), it.roll.to_bits()],
        [it.pivot[0].to_bits(), it.pivot[1].to_bits(), it.pivot[2].to_bits()],
        it.scale.to_bits(),
    )
}

/// The embedded library as name → md5 of the bytes.
fn library(m: &tmmaps::map::MapFile) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some((zip, _names)) = tmmaps::header::embedded_zip_bytes(&m.gbx.body) {
        for (name, bytes) in tmmaps::header::zip_entries(&zip) {
            out.insert(name, crate::publish::md5_hex(&bytes));
        }
    }
    out
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let shipped = PathBuf::from(f("--shipped").ok_or("retile needs --shipped LIT.Map.Gbx (the file whose lightmap carries over)")?);
    let built = PathBuf::from(f("--built").ok_or("retile needs --built UNLIT.Map.Gbx (the rebuilt file, same models, new item list)")?);
    let out = PathBuf::from(f("--out").ok_or("retile needs --out OUT.Map.Gbx")?);
    let dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("exe dir")?.to_path_buf();
    let lmtool = f("--lmtool").map(PathBuf::from).unwrap_or_else(|| dir.join("lmtool"));
    if !lmtool.exists() {
        return Err(format!("{}: no lmtool beside tinyctl (--lmtool PATH)", lmtool.display()));
    }
    let s = tmmaps::map::MapFile::load(&shipped);
    let b = tmmaps::map::MapFile::load(&built);
    // 1. the libraries must be identical
    let (ls, lb) = (library(&s), library(&b));
    let only_s: Vec<&String> = ls.keys().filter(|k| !lb.contains_key(*k)).collect();
    let only_b: Vec<&String> = lb.keys().filter(|k| !ls.contains_key(*k)).collect();
    let differ: Vec<&String> = ls.iter().filter(|(k, v)| lb.get(*k).map(|w| w != *v).unwrap_or(false)).map(|(k, _)| k).collect();
    if !only_s.is_empty() || !only_b.is_empty() || !differ.is_empty() {
        return Err(format!(
            "embedded libraries differ — not retiled: {} files only in the shipped map ({}), {} only in the rebuild ({}), {} with other bytes ({})",
            only_s.len(),
            only_s.iter().take(5).map(|s| s.as_str()).collect::<Vec<_>>().join(" "),
            only_b.len(),
            only_b.iter().take(5).map(|s| s.as_str()).collect::<Vec<_>>().join(" "),
            differ.len(),
            differ.iter().take(5).map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
        ));
    }
    println!("libraries identical: {} embedded files", ls.len());
    // 2. match the placements
    let mut by_key: HashMap<_, Vec<usize>> = HashMap::new();
    for it in &b.items {
        by_key.entry(key(it)).or_default().push(it.index);
    }
    for v in by_key.values_mut() {
        v.reverse(); // pop() takes the lowest index first
    }
    let mut kept: Vec<Option<usize>> = Vec::with_capacity(s.items.len());
    let mut matched_new = vec![false; b.items.len()];
    for it in &s.items {
        let m = by_key.get_mut(&key(it)).and_then(|v| v.pop());
        if let Some(i) = m {
            matched_new[i] = true;
        }
        kept.push(m);
    }
    let dropped: Vec<&tmmaps::map::ItemRec> = s.items.iter().filter(|it| kept[it.index].is_none()).collect();
    let new: Vec<&tmmaps::map::ItemRec> = b.items.iter().filter(|it| !matched_new[it.index]).collect();
    // the order of the common items must be preserved (the editor's tables are sorted by object)
    let mut last = 0usize;
    let mut monotonic = true;
    for k in kept.iter().flatten() {
        if *k < last {
            monotonic = false;
        }
        last = *k;
    }
    let hist = |v: &[&tmmaps::map::ItemRec]| {
        let mut h: BTreeMap<&str, usize> = BTreeMap::new();
        for it in v {
            *h.entry(it.model.as_str()).or_default() += 1;
        }
        h.iter().map(|(m, n)| format!("{n} × {m}")).collect::<Vec<_>>().join(", ")
    };
    println!("items: shipped {} → rebuilt {}; {} matched, {} dropped ({}), {} new ({}); common order {}", s.items.len(), b.items.len(), kept.iter().flatten().count(), dropped.len(), hist(&dropped), new.len(), hist(&new), if monotonic { "preserved" } else { "NOT preserved (charts re-sorted)" });
    for it in &new {
        println!("  new item i{} {} at ({:.1}, {:.2}, {:.1}) yaw {:.4}", it.index, it.model, it.pos[0], it.pos[1], it.pos[2], it.yaw);
    }
    // 3. the object base: P + authored blocks + S_x·S_z + G. Only the 0-block terrain form is derived
    // here (P 0, G 0): base = the size words' x·z. Anything else needs --base.
    let base: u32 = match f("--base") {
        Some(v) => v.parse().map_err(|_| "--base N".to_string())?,
        None => {
            if !b.blocks.is_empty() || !s.blocks.is_empty() {
                return Err(format!("the maps carry authored blocks ({} shipped / {} rebuilt): the item base is P + blocks + S² + G — pass --base N (lmtool itembase LIT measures it)", s.blocks.len(), b.blocks.len()));
            }
            if s.size != b.size {
                return Err(format!("size words differ: shipped {:?} vs rebuilt {:?}", s.size, b.size));
            }
            (s.size[0] * s.size[2]) as u32
        }
    };
    // 4. the clone list: every new item takes the charts of the nearest same-model, same-yaw shipped item that is kept
    let mut clones: Vec<String> = Vec::new();
    let mut chartless_new = 0usize;
    if !tmmaps::cli::has(args, "--no-clone") {
        for it in &new {
            let mut best: Option<(f32, usize)> = None;
            for (r, sit) in s.items.iter().enumerate() {
                if sit.model != it.model || kept[r].is_none() || sit.yaw.to_bits() != it.yaw.to_bits() || sit.pitch.to_bits() != it.pitch.to_bits() || sit.roll.to_bits() != it.roll.to_bits() {
                    continue;
                }
                let d = (0..3).map(|k| (sit.pos[k] - it.pos[k]).powi(2)).sum::<f32>();
                if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                    best = Some((d, r));
                }
            }
            match best {
                Some((d, r)) => {
                    println!("  i{} {} takes the charts of shipped item {} ({} m away, same model and yaw)", it.index, it.model, r, d.sqrt());
                    clones.push(format!("{}={}", it.index, r));
                }
                None => {
                    println!("  i{} {}: no same-model, same-yaw shipped item to copy charts from — stays chartless", it.index, it.model);
                    chartless_new += 1;
                }
            }
        }
    } else {
        chartless_new = new.len();
    }
    // 5. the transplant
    let kept_list = kept.iter().map(|k| k.map(|i| i.to_string()).unwrap_or_else(|| "-".into())).collect::<Vec<_>>().join(",");
    let kept_path = out.with_extension("kept");
    std::fs::write(&kept_path, &kept_list).map_err(|e| format!("{}: {e}", kept_path.display()))?;
    let mut a: Vec<String> = vec!["transplant".into(), "--from".into(), shipped.display().to_string(), "--into".into(), built.display().to_string(), "--kept".into(), format!("@{}", kept_path.display()), "--base".into(), base.to_string(), "--out".into(), out.display().to_string()];
    if !clones.is_empty() {
        a.push("--clone".into());
        a.push(clones.join(","));
    }
    let o = std::process::Command::new(&lmtool).args(&a).output().map_err(|e| format!("{}: {e}", lmtool.display()))?;
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    if !o.status.success() {
        return Err(format!("lmtool transplant failed: {}", text.lines().last().unwrap_or("")));
    }
    for l in text.lines().filter(|l| l.starts_with("wrote") || l.contains("--clone")) {
        println!("  {l}");
    }
    // 6. the output's items are the rebuild's, byte for byte (the chunk swap touches nothing else)
    let r = tmmaps::map::MapFile::load(&out);
    if r.items.len() != b.items.len() || library(&r) != lb {
        return Err(format!("{}: items {} vs the rebuild's {} / library differs — the transplant changed more than the lightmap chunk", out.display(), r.items.len(), b.items.len()));
    }
    let same_placements = r.items.iter().zip(b.items.iter()).all(|(x, y)| key(x) == key(y));
    println!("{}: {} items, placements {} the rebuild's, library identical, {} base {base}, {} new items chartless", out.display(), r.items.len(), if same_placements { "=" } else { "≠ (!!)" }, if clones.is_empty() { "no clones".to_string() } else { format!("{} clones", clones.len()) }, chartless_new);
    if !same_placements {
        return Err("placements differ between the output and the rebuild".into());
    }
    // 7. lmtool check's summary
    let c = std::process::Command::new(&lmtool).arg("check").arg(&out).output().map_err(|e| format!("{}: {e}", lmtool.display()))?;
    let ct = String::from_utf8_lossy(&c.stdout).to_string();
    let fails: Vec<&str> = ct.lines().filter(|l| l.contains("[FAIL]")).collect();
    println!("lmtool check: {} FAIL line(s){}", fails.len(), if fails.is_empty() { String::new() } else { format!(": {}", fails.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" | ")) });
    let _ = Path::new(&out);
    Ok(())
}
