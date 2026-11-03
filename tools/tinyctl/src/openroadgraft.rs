//! `tinyctl openroad-graft` — the open-road fix alone, grafted into a LIVE map:
//!
//! ```text
//! tinyctl openroad-graft --live LIVE.Map.Gbx --orig ORIGINAL.Map.Gbx --built BUILT.Map.Gbx
//!                        --report REPORT.tsv --items-dir LIBX/Items --out OUT.Map.Gbx [--alias AC22710]
//! ```
//!
//! Everios96 2026-11-02 on Tarmac 22's new thumbnail: "penalty grass there which should be
//! sand" — the live V1 maps still carry the OpenTech twin of a foreign-family open road
//! (grass shoulders), the 147 class fixed in the pipeline on 11-01 (the path re-dressed, the
//! shoulders kept). A full rebuild would change more than that (the cover rules moved on
//! since the live build: map 24 lost 250 wall covers in a rebuild), and V1 is being hunted —
//! so only the open roads move:
//!
//! 1. every live `Open*` block standing where the ORIGINAL had an open road of ANOTHER
//!    family (same cell, same quarter turn) is dropped — those are the twins;
//! 2. the fresh build's items whose source block is such an `Open*` block (the report's
//!    `alias → source` rows) are embedded under a fresh alias range and placed exactly as the
//!    build placed them (position + yaw).
//!
//! Nothing else in the live file changes; the uid stays (an in-place update, records kept).
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmmaps_bin() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("tmmaps"))).filter(|p| p.exists()).unwrap_or_else(|| PathBuf::from("tmmaps"))
}

fn family(name: &str) -> Option<&'static str> {
    for f in ["OpenTech", "OpenDirt", "OpenGrass", "OpenIce"] {
        if name.starts_with(f) {
            return Some(f);
        }
    }
    None
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let live = PathBuf::from(f("--live").ok_or("openroad-graft needs --live LIVE.Map.Gbx")?);
    let orig = PathBuf::from(f("--orig").ok_or("openroad-graft needs --orig ORIGINAL.Map.Gbx")?);
    let built = PathBuf::from(f("--built").ok_or("openroad-graft needs --built BUILT.Map.Gbx")?);
    let report = PathBuf::from(f("--report").ok_or("openroad-graft needs --report REPORT.tsv")?);
    let items_dir = PathBuf::from(f("--items-dir").ok_or("openroad-graft needs --items-dir LIBX/Items")?);
    let out = PathBuf::from(f("--out").ok_or("openroad-graft needs --out OUT.Map.Gbx")?);
    let alias = f("--alias").unwrap_or_else(|| "AC99900".into());
    let o = tmmaps::map::MapFile::load(&orig);
    let l = tmmaps::map::MapFile::load(&live);
    // 1. the twins: a live Open* block where the original had an Open* block of another family
    let orig_open: HashMap<((i32, i32, i32), u8), &'static str> = o.blocks.iter().filter_map(|b| family(&b.name).map(|fam| ((b.coords(), b.dir), fam))).collect();
    let twins: Vec<((i32, i32, i32), u8, String, &'static str)> = l
        .blocks
        .iter()
        .filter_map(|b| {
            let fam = family(&b.name)?;
            let ofam = orig_open.get(&(b.coords(), b.dir))?;
            if *ofam != fam { Some((b.coords(), b.dir, b.name.clone(), *ofam)) } else { None }
        })
        .collect();
    // the families the build re-dressed (the originals' families at the twin cells)
    let want_fams: HashSet<&'static str> = twins.iter().map(|t| t.3).collect();
    if twins.is_empty() {
        return Err(format!("{}: no open-road twin found (nothing to graft)", live.display()));
    }
    // 2. the build's open-road items: report rows `block <alias> <n> <status> <source …>` whose source is an Open* block of a foreign family
    let text = std::fs::read_to_string(&report).map_err(|e| format!("{}: {e}", report.display()))?;
    let mut aliases: HashMap<String, String> = HashMap::new();
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 5 || c[0] != "block" {
            continue;
        }
        let src_name = c[4].split(' ').next().unwrap_or("");
        if let Some(fam) = family(src_name) {
            if want_fams.contains(fam) {
                aliases.insert(c[1].to_string(), src_name.to_string());
            }
        }
    }
    // the built map's placements of those aliases — the WHOLE record frame (pivot, pitch/roll,
    // scale, flags/variant), not just position + yaw: a block item's frame is the block's
    // (the first graft went through `add-items`' zero pivot and the roads landed as dark
    // plates over the cells, 2026-11-02)
    let b = tmmaps::map::MapFile::load(&built);
    let placements: Vec<tmmaps::map::ItemRec> = b.items.iter().filter(|it| aliases.contains_key(it.model.trim_end_matches(".Item.Gbx"))).cloned().collect();
    if placements.is_empty() {
        return Err("the build placed no open-road items (report/census mismatch?)".into());
    }
    let used: HashSet<String> = placements.iter().map(|it| it.model.trim_end_matches(".Item.Gbx").to_string()).collect();
    // the item files under a fresh alias range (the live archive has its own ACnnn names)
    let tmpdir = std::env::temp_dir().join(format!("openroad-graft-{}", std::process::id()));
    std::fs::create_dir_all(&tmpdir).map_err(|e| e.to_string())?;
    let mut rename: HashMap<String, String> = HashMap::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for (i, a) in used.iter().enumerate() {
        let new = format!("{alias}{:03}", i);
        let src = items_dir.join(format!("{a}.Item.Gbx"));
        let dst = tmpdir.join(format!("{new}.Item.Gbx"));
        // the item file carries its own ident inside (header chunk + body strings): a plain
        // file rename leaves them pointing at the old name and the game draws nothing (the
        // first graft's dark plates, 2026-11-02) — `mapgeom item-rename` rebuilds them
        let mapgeom = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("mapgeom"))).filter(|p| p.exists()).unwrap_or_else(|| PathBuf::from("mapgeom"));
        let o = Command::new(&mapgeom).args(["item-rename", &src.to_string_lossy(), "--out", &dst.to_string_lossy(), "--ident", &format!("{new}.Item.Gbx")]).output().map_err(|e| e.to_string())?;
        if !o.status.success() || !dst.exists() {
            return Err(format!("item-rename {a}: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("")));
        }
        rename.insert(a.clone(), new);
        files.push(dst);
    }
    // drop the twins (by name; every twin name in the live file is a twin — a genuine OpenTech
    // road of the original is not in `twins`, and the names are checked below)
    let twin_names: HashSet<String> = twins.iter().map(|t| t.2.clone()).collect();
    let genuine: Vec<&tmmaps::map::BlockRec> = l.blocks.iter().filter(|bk| twin_names.contains(&bk.name) && !twins.iter().any(|t| t.0 == bk.coords() && t.1 == bk.dir)).collect();
    if !genuine.is_empty() {
        return Err(format!("{} genuine {} block(s) share a twin's name — a by-name drop would take them; refusing", genuine.len(), genuine[0].name));
    }
    let step1 = tmpdir.join("step1.Map.Gbx");
    let names: Vec<String> = twin_names.iter().cloned().collect();
    let o1 = Command::new(tmmaps_bin()).args(["drop-blocks", &live.to_string_lossy(), "--name", &names.join(","), "--out", &step1.to_string_lossy()]).output().map_err(|e| e.to_string())?;
    if !o1.status.success() || !step1.exists() {
        return Err(format!("drop-blocks: {}", String::from_utf8_lossy(&o1.stderr).lines().last().unwrap_or("")));
    }
    // the item files into the archive (`add-items` embeds them; its placements are parked under
    // the map and re-framed below)
    let park: Vec<String> = files.iter().map(|f| format!("{}@8,-900,8@0", f.display())).collect();
    let step2 = tmpdir.join("step2.Map.Gbx");
    let o2 = Command::new(tmmaps_bin()).args(["add-items", &step1.to_string_lossy(), "--out", &step2.to_string_lossy(), "--items", &park.join(";")]).output().map_err(|e| e.to_string())?;
    if !o2.status.success() || !step2.exists() {
        return Err(format!("add-items: {}", String::from_utf8_lossy(&o2.stderr).lines().last().unwrap_or("")));
    }
    // the real placements: one record per built placement, framed like the build's
    // the parked records become the first placements (every file has at least one), the
    // rest are clones — nothing stays parked under the map (an item outside the 48×40×48
    // volume is what the editor refused on 2026-09-24)
    let mut m = tmmaps::map::MapFile::load(&step2);
    let parked_first = m.items.len() - files.len();
    let total = parked_first + placements.len();
    if total > m.items.len() {
        tmmaps::tiny::set_ground(26);
        m.append_item_clones(total);
        m.write_to(&step2).map_err(|e| e.to_string())?;
        m = tmmaps::map::MapFile::load(&step2);
    }
    for (k, src) in placements.iter().enumerate() {
        let i = parked_first + k;
        let ident = format!("{}.Item.Gbx", rename[src.model.trim_end_matches(".Item.Gbx")]);
        m.set_item_model(i, &ident);
        m.set_item_author(i, &tmmaps::surface::item_author(&ident));
        m.set_item_collection(i, 26);
        m.move_item(i, src.pos, src.yaw, tmmaps::tiny::cell_for(src.pos));
        m.set_item_frame(i, [src.yaw, src.pitch, src.roll], src.pivot);
        m.set_item_scale(i, src.scale);
        m.set_item_flags(i, src.flags);
    }
    m.write_to(&out).map_err(|e| e.to_string())?;
    // the live V1 file still carries the ORIGINAL's stored lightmap (401 KB on 22); an object
    // the lightmap never saw renders black — the grafted roads came out as dark plates
    // (2026-11-02). The fresh builds ship without one (16-byte chunk): same here.
    let mut m3 = tmmaps::map::MapFile::load(&out);
    let stripped = m3.strip_lightmap();
    if stripped > 0 {
        m3.write_to(&out).map_err(|e| e.to_string())?;
    }
    println!("lightmap: {stripped} bytes stripped");
    let _ = std::fs::remove_dir_all(&tmpdir);
    println!(
        "{}: {} twin block(s) dropped ({}), {} open-road item placement(s) added from {} model(s)",
        out.display(),
        twins.len(),
        names.join(","),
        placements.len(),
        used.len()
    );
    Ok(())
}
