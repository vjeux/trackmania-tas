//! `tinyctl probe SRC.Map.Gbx [--paks "--pak F:KEY ..."]` — everything the
//! per-environment onboarding used to measure by hand, in one read.
//!
//! What it prints, and what each number was for on the maps done so far:
//!
//! - collection, decoration, counts (blocks / baked / free / items / models);
//! - the ground row (`ground_y`) the code has for this collection, and the
//!   MEASUREMENT that decides it: for every item, `pos.y - cell_y*8` — the
//!   items stand on block tops, so the most common value is
//!   `ground_y + local plane offset` (BlueBay: 2052 palms at 10 on cell 6
//!   → -40 + 2). A collection whose table entry disagrees with the
//!   measurement is flagged;
//! - the fixed plane the tiny transform keeps (`fixed_plane`) and the anchor
//!   `tmmaps tiny` will print by default;
//! - the genealogy zones (chunk 0x03043043), the ambient zone, and which
//!   policy `tmmaps tiny` applies (clear / fill / keep);
//! - a census of the zone (terrain) blocks with their cell rows, and of the
//!   items standing in the cells of each zone block name;
//! - the waypoints;
//! - with `--paks`, a dry library build: every model of the map is baked from
//!   the packs into a temporary library and the failures are listed — the
//!   "models missing from the loaded paks" answer, before any map is built.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tmmaps::map::MapFile;

use crate::views::{block_pos, collection_name, collection_of, default_anchor};

fn hist_line<K: std::fmt::Display>(h: &BTreeMap<K, usize>, top: usize) -> String {
    let mut v: Vec<(&K, &usize)> = h.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1));
    v.iter().take(top).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join("  ")
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let src = Path::new(args.get(0).ok_or("probe needs SRC.Map.Gbx")?);
    let m = MapFile::load(src);
    let coll = collection_of(&m);
    let hdr = tmmaps::header::read(src.to_str().unwrap_or_default()).ok();
    println!("map        {}", src.display());
    if let Some(h) = &hdr {
        println!("name       {}  uid {}  author {}  envir {}  AT {}", h.name, h.uid, h.author, h.envir, h.authortime);
    }
    println!("collection {:#x} {}  decoration {}  size {:?}", coll, collection_name(coll), m.decoration_id, m.size);
    let free = m.blocks.iter().filter(|b| b.free_pos.is_some()).count();
    let mut models: Vec<&str> = m.items.iter().map(|i| i.model.as_str()).collect();
    models.sort_unstable();
    models.dedup();
    println!("blocks     {} authored ({} free) + {} baked;  items {} ({} models)", m.blocks.len(), free, m.baked.len(), m.items.len(), models.len());

    // --- ground row: the table vs the measurement
    let ground = tmmaps::map::ground_y(coll);
    let mut offs: BTreeMap<i64, usize> = BTreeMap::new();
    for it in &m.items {
        let cy_raw = it.coords().1;
        if cy_raw >= 64 {
            continue; // 0xFF: the placement carries no grid cell
        }
        let cy = cy_raw as f32;
        let off = it.pos[1] - cy * tmmaps::map::CELL_Y;
        *offs.entry((off * 4.0).round() as i64).or_default() += 1;
    }
    let offs_disp: BTreeMap<String, usize> = offs.iter().map(|(k, n)| (format!("{:.2}", *k as f32 / 4.0), *n)).collect();
    println!("ground_y   table {ground}   measured item.y - cell.y*8 (= ground_y + plane offset): {}", hist_line(&offs_disp, 6));
    if let Some((k, _)) = offs.iter().max_by_key(|(_, n)| **n) {
        let measured = *k as f32 / 4.0;
        let plane_off = measured - ground;
        if !(0.0..=8.0).contains(&plane_off) {
            println!("  ⚠ the most common offset {measured} is {plane_off:+} from the table's ground row — check ground_y for this collection");
        } else {
            println!("  ok: most items stand {plane_off} above the cell floor");
        }
    }
    // --- fixed plane + anchor
    let plane = tmmaps::tiny::fixed_plane(coll);
    println!("fixed plane {plane}  (world y the tiny transform keeps)");
    match default_anchor(&m, 0.5) {
        Some((a, b)) => println!("anchor     {},{},{}:{},{},{}   (spawn -> tiny; pass as --anchor)", a[0], a[1], a[2], b[0], b[1], b[2]),
        None => println!("anchor     ⚠ no Spawn waypoint found"),
    }
    // --- genealogy
    let zones = m.genealogy_zones();
    let mut zh: BTreeMap<String, usize> = BTreeMap::new();
    for z in &zones {
        *zh.entry(z.clone()).or_default() += 1;
    }
    let policy = match coll {
        0x1c => "clear (BlueBay: the sea is decoration)",
        0x10 | 0x1d | 0xf => "fill with the ambient zone (the game regenerates it full size)",
        0x1a => "keep (Stadium: the grass floor is the foundation)",
        _ => "⚠ none coded for this collection — decide, then add it to tmmaps::tiny",
    };
    println!("genealogy  {} records; ambient {:?}; zones: {}", zones.len(), m.ambient_zone(), hist_line(&zh, 8));
    println!("  policy   {policy}");
    // --- zone blocks census + what stands on them
    let zone_names: std::collections::BTreeSet<&str> = zh.keys().map(|s| s.as_str()).collect();
    let mut zb: BTreeMap<(String, i32), usize> = BTreeMap::new();
    let mut cell_zone: BTreeMap<(i32, i32), (String, i32)> = BTreeMap::new();
    for b in &m.blocks {
        if zone_names.contains(b.name.as_str()) || b.name.starts_with("Land") || b.name.starts_with("Water") || b.name.starts_with("Grass") || b.name.starts_with("Dirt") || b.name.starts_with("Lake") || b.name.starts_with("Beach") {
            let c = b.coords();
            *zb.entry((b.name.clone(), c.1)).or_default() += 1;
            cell_zone.insert((c.0, c.2), (b.name.clone(), c.1));
        }
    }
    if !zb.is_empty() {
        let zd: BTreeMap<String, usize> = zb.iter().map(|((n, y), c)| (format!("{n}@row{y}"), *c)).collect();
        println!("zone blocks {}", hist_line(&zd, 12));
        let mut standing: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
        for it in &m.items {
            let c = it.coords();
            if let Some((zname, zrow)) = cell_zone.get(&(c.0, c.2)) {
                let rel = it.pos[1] - (*zrow as f32 * tmmaps::map::CELL_Y + ground);
                *standing.entry(zname.clone()).or_default().entry(format!("{rel:+.2}")).or_default() += 1;
            }
        }
        for (z, h) in &standing {
            println!("  items in {z} cells stand at (y - cell floor): {}", hist_line(h, 5));
        }
    }
    // --- waypoints
    for w in m.waypoints() {
        let pos = match w.kind {
            tmmaps::map::Kind::Block => block_pos(&m, &m.blocks[w.index]),
            tmmaps::map::Kind::Item => m.items[w.index].pos,
        };
        println!("waypoint   {:<10} {:<28} at {:.1},{:.1},{:.1} yaw {:.2}", w.tag, w.name, pos[0], pos[1], pos[2], w.yaw.unwrap_or(0.0));
    }
    // --- block model histogram (authored)
    let mut bh: BTreeMap<String, usize> = BTreeMap::new();
    for b in &m.blocks {
        *bh.entry(b.name.clone()).or_default() += 1;
    }
    println!("block models {} distinct: {}", bh.len(), hist_line(&bh, 14));
    let mut ih: BTreeMap<String, usize> = BTreeMap::new();
    for it in &m.items {
        *ih.entry(it.model.clone()).or_default() += 1;
    }
    println!("item models {} distinct: {}", ih.len(), hist_line(&ih, 14));

    // --- dry library build against the packs
    if let Some(paks) = tmmaps::cli::flag(args, "--paks") {
        let mut store = mapgeom::store::DataStore::empty();
        let toks: Vec<&str> = paks.split_whitespace().collect();
        let mut i = 0;
        while i < toks.len() {
            if toks[i] == "--pak" {
                let spec = toks.get(i + 1).ok_or("--paks: --pak needs FILE:KEY")?;
                let (p, k) = spec.rsplit_once(':').ok_or_else(|| format!("--paks: `{spec}` is not FILE:KEY"))?;
                store.add_pak(p, k)?;
                i += 2;
            } else {
                i += 1;
            }
        }
        let tmp = std::env::temp_dir().join(format!("tinyctl-probe-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        let report: PathBuf = tmp.join("report.tsv");
        println!("library    dry build into {} …", tmp.display());
        mapgeom::tiny_library::build(&mut store, src, &tmp.join("lib.zip"), &tmp.join("placements.tsv"), Some(&report), 0.5, None, None, "substitute", "", None);
        let rep = std::fs::read_to_string(&report).unwrap_or_default();
        let fails: Vec<&str> = rep.lines().filter(|l| l.split('\t').nth(3) == Some("FAIL")).collect();
        if fails.is_empty() {
            println!("library    every model baked from the packs");
        } else {
            println!("library    ⚠ {} models FAILED:", fails.len());
            for f in fails {
                println!("  {f}");
            }
        }
        if !tmmaps::cli::has(args, "--keep") {
            let _ = std::fs::remove_dir_all(&tmp);
        }
    } else {
        println!("library    (pass --paks \"$PAKS\" for the dry build that lists models missing from the packs)");
    }
    Ok(())
}
