//! `tmmaps stadium-port MAP --out OUT` — a vista map (BlueBay / RedIsland /
//! WhiteShore / GreenCoast) as a STADIUM map: the built blocks (roads, platforms,
//! structures — Stadium blocks already) and the Stadium items kept and re-centred
//! in the 48×40×48 stadium, the terrain-conformed `On…` variants swapped for their
//! plain twins, the terrain (Land, hills, beach, cliffs, water) and the vista
//! vegetation dropped, collection / decoration / size / header rewritten.
//! json9841, 2026-09-22: "Is it possible to migrate vista maps to stadium to
//! unblock this? I think there are several alterations blocked on similar issues."
//!
//! `tmmaps stadium-scan MAP…` — the feasibility table (what fits, what is dropped).

use crate::map::MapFile;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Every block info of the Stadium pack (mapgeom ls "" --pak Stadium.pak, the
/// GameCtnBlockInfo entries by stem, 2026-09-22) — lower-cased for the lookup
/// (the pack spells `PlatFormTechBase`).
fn stadium_blocks() -> &'static BTreeSet<String> {
    static SET: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    SET.get_or_init(|| include_str!("stadium-blocks.txt").lines().map(|l| l.trim().to_ascii_lowercase()).filter(|l| !l.is_empty()).collect())
}
fn stadium_items() -> &'static BTreeSet<String> {
    static SET: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    SET.get_or_init(|| include_str!("stadium-items.txt").lines().map(|l| l.trim().to_ascii_lowercase()).filter(|l| !l.is_empty()).collect())
}

pub const STADIUM: u32 = 26;
/// The Stadium ground: the Grass fillers sit at cell row 9 (floor 8 m; ground_y −64).
pub const GROUND_CELL: i32 = 9;
pub const SIZE: [i32; 3] = [48, 40, 48];

#[derive(Debug, Clone, PartialEq)]
pub enum Fate {
    Keep,
    /// the plain twin of a terrain-conformed variant, and how many rows UP its deck
    /// sits from the variant's origin (a flat road on a 2-high hill: +2; a slope: 0)
    Twin(String, i32, u8),
    Drop(&'static str),
}

/// The measured twins: variant \t plain twin \t rows up (`mapgeom blockinfo` unit
/// heights over the four vista packs and Stadium.pak, 2026-09-22; the two variants
/// no pack lists took their digit).
/// … and a 4th column since 2026-09-22: the quarter TURNS the plain twin needs. Every
/// On-terrain prefab is a wrapper around the plain Stadium prefab placed with a position
/// and a quaternion (`mapgeom prefab-ents`): the slopes carry it turned 180° about Y
/// (pos (32, 0, 32), rot (0, 1, 0, 0)) — renamed in place they ran the other way
/// (json9841: "first downhill is backwards"); the flat pieces sit raised (pos (0, 16, 0)
/// = +2 rows), unturned.
fn twin_table() -> &'static BTreeMap<String, (String, i32, u8)> {
    static T: std::sync::OnceLock<BTreeMap<String, (String, i32, u8)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        include_str!("stadium-on-twins.tsv")
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                (f.len() >= 3).then(|| (f[0].to_string(), (f[1].to_string(), f[2].trim().parse().unwrap_or(0), f.get(3).and_then(|t| t.trim().parse().ok()).unwrap_or(0u8))))
            })
            .collect()
    })
}

/// `RoadTechOnLandHillSlopeBase2x1` → `RoadTechSlopeBase2x1`; `PlatformTechBaseOnGrassCliff2`
/// → `PlatformTechBase`; `RoadDirtStraightOnWaterHill` → `RoadDirtStraight`.
pub fn strip_on(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if &name[i..i + 2] == "On" && i > 0 {
            let rest = &name[i + 2..];
            for terr in ["Land", "Dirt", "Grass", "Sea", "Snow", "Water", "Ice", "Beach"] {
                if let Some(after) = rest.strip_prefix(terr) {
                    // the terrain word, an optional Hill/Cliff/Shore, digits
                    let mut j = 0;
                    for kind in ["Hill", "Cliff", "Shore"] {
                        if after[j..].starts_with(kind) {
                            j += kind.len();
                            break;
                        }
                    }
                    let k = after[j..].chars().take_while(|c| c.is_ascii_digit()).count();
                    j += k;
                    return Some(format!("{}{}", &name[..i], &after[j..]));
                }
            }
        }
        i += 1;
    }
    None
}

pub fn fate(name: &str) -> Fate {
    if name.contains("_CustomBlock") {
        return Fate::Keep;
    }
    let known = stadium_blocks();
    if known.contains(&name.to_ascii_lowercase()) {
        return Fate::Keep;
    }
    if let Some((t, dy, turn)) = twin_table().get(name) {
        if known.contains(&t.to_ascii_lowercase()) {
            return Fate::Twin(t.clone(), *dy, *turn);
        }
    }
    if let Some(t) = strip_on(name) {
        if known.contains(&t.to_ascii_lowercase()) {
            // an unmeasured variant: a flat piece rises by its digit, a slope stays
            let digit: i32 = name.rsplit(|c: char| !c.is_ascii_digit()).next().and_then(|d| d.parse().ok()).unwrap_or(if name.contains("Slope") { 0 } else { 1 });
            return Fate::Twin(t, if name.contains("Slope") { 0 } else { digit }, 0);
        }
        return Fate::Drop("terrain variant without a twin");
    }
    Fate::Drop("not a Stadium block")
}

fn item_kept(model: &str) -> bool {
    if model.contains('\\') || model.contains('/') || model.ends_with(".Item.Gbx") {
        return true; // an embedded custom item travels with the map
    }
    stadium_items().contains(&model.to_ascii_lowercase())
}

struct Plan {
    collection: u32,
    ground: f32,
    fates: Vec<Fate>,
    item_keep: Vec<bool>,
    /// cell shift (dx, dy, dz)
    shift: [i32; 3],
    /// kept-block cell bounds before the shift
    bounds: ([i32; 3], [i32; 3]),
    fits: bool,
    dropped: BTreeMap<String, usize>,
    twins: BTreeMap<String, usize>,
    items_dropped: BTreeMap<String, usize>,
    /// kept blocks outside the 48-window (scenery far from the route), dropped
    trimmed: usize,
    deco: String,
}

/// The cell a block occupies: a grid block's cell; a free block's world position.
fn block_cell(b: &crate::map::BlockRec, ground: f32) -> [i32; 3] {
    match b.free_pos {
        Some(p) => [(p[0] / 32.0).floor() as i32, ((p[1] - ground) / 8.0).floor() as i32, (p[2] / 32.0).floor() as i32],
        None => {
            let (x, y, z) = b.coords();
            [x, y, z]
        }
    }
}

fn plan(m: &MapFile, input: &Path) -> Result<Plan, String> {
    // the collection: the items' (every vista item carries it), else the header's
    // environment (an `isolate`d probe map has no items)
    let collection = m.items.first().map(|it| it.collection_raw).unwrap_or_else(|| {
        match crate::header::read(input.to_str().unwrap_or("")).map(|h| h.envir).as_deref() {
            Ok("RedIsland") => 0x10,
            Ok("Desert") => 0x1c,
            Ok("Rally") => 0x1d,
            Ok("Snow") => 0x1e,
            _ => 26,
        }
    });
    if collection == STADIUM {
        return Err("already a Stadium map".into());
    }
    let ground = crate::map::ground_y(collection);
    let mut fates: Vec<Fate> = m.blocks.iter().map(|b| fate(&b.name)).collect();
    let mut item_keep: Vec<bool> = m.items.iter().map(|it| item_kept(&it.model)).collect();
    // bisection knobs (a port the game refused, 2026-09-22): drop every item, every
    // free block, or every block whose name contains STADIUM_DROP
    if std::env::var("STADIUM_DROP_ITEMS").is_ok() {
        item_keep.iter_mut().for_each(|k| *k = false);
    }
    if std::env::var("STADIUM_DROP_FREE").is_ok() {
        for (b, f) in m.blocks.iter().zip(fates.iter_mut()) {
            if b.free_pos.is_some() {
                *f = Fate::Drop("bisect: free");
            }
        }
    }
    if let Ok(pat) = std::env::var("STADIUM_DROP_ITEM") {
        for (it, k) in m.items.iter().zip(item_keep.iter_mut()) {
            if pat.split(',').any(|p| !p.is_empty() && it.model.contains(p)) {
                *k = false;
            }
        }
    }
    if let Ok(pat) = std::env::var("STADIUM_KEEP_ITEM_ONLY") {
        for (it, k) in m.items.iter().zip(item_keep.iter_mut()) {
            if !pat.split(',').any(|p| !p.is_empty() && it.model.contains(p)) {
                *k = false;
            }
        }
    }
    if let Ok(pat) = std::env::var("STADIUM_DROP") {
        for (b, f) in m.blocks.iter().zip(fates.iter_mut()) {
            if pat.split(',').any(|p| !p.is_empty() && b.name.contains(p)) {
                *f = Fate::Drop("bisect: name");
            }
        }
    }
    // the route: waypoints + the validation ghost's cells (x, z)
    let mut route: Vec<[i32; 3]> = Vec::new();
    for b in &m.blocks {
        if b.waypoint_tag.is_some() {
            route.push(block_cell(b, ground));
        }
    }
    for it in &m.items {
        if it.waypoint_tag.is_some() {
            route.push([(it.pos[0] / 32.0).floor() as i32, ((it.pos[1] - ground) / 8.0).floor() as i32, (it.pos[2] / 32.0).floor() as i32]);
        }
    }
    if let Ok(d) = gbx::record::decode_ghost(input.to_str().unwrap_or_default()) {
        for s in &d.samples {
            route.push([(s.x / 32.0).floor() as i32, ((s.y - ground) / 8.0).floor() as i32, (s.z / 32.0).floor() as i32]);
        }
    }
    let route_box = |cells: &[[i32; 3]]| -> Option<([i32; 3], [i32; 3])> {
        if cells.is_empty() {
            return None;
        }
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for c in cells {
            for k in 0..3 {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
        Some((lo, hi))
    };
    // the kept blocks' cells
    let kept_cells: Vec<[i32; 3]> = m.blocks.iter().zip(&fates).filter(|(_, f)| !matches!(f, Fate::Drop(_))).map(|(b, _)| block_cell(b, ground)).collect();
    let (mut lo, mut hi) = route_box(&kept_cells).ok_or("no built blocks")?;
    // … widened to every WAYPOINT ITEM: a checkpoint gate standing on landscape past the
    // last kept block was trimmed as scenery (Spring 2026 - 03's 2nd gate — json9841
    // 2026-09-23 "incorrect block placement after cp 3": the map had one CP fewer)
    for it in &m.items {
        if it.waypoint_tag.is_some() && item_keep[m.items.iter().position(|x| std::ptr::eq(x, it)).unwrap_or(0)] {
            let c = [(it.pos[0] / 32.0).floor() as i32, ((it.pos[1] - ground) / 8.0).floor() as i32, (it.pos[2] / 32.0).floor() as i32];
            for k in [0usize, 2] {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
    }
    let route_bb = route_box(&route);
    // x/z window: 48 cells centred on the route box (kept blocks trimmed to it); the
    // route box itself must fit
    let mut trimmed = 0usize;
    let mut fits = true;
    for k in [0usize, 2] {
        let (rlo, rhi) = match route_bb {
            Some((rl, rh)) => (rl[k].min(hi[k]).max(lo[k]), rh[k].max(lo[k]).min(hi[k])),
            None => (lo[k], hi[k]),
        };
        if rhi - rlo + 1 > SIZE[k] {
            fits = false;
        }
        if hi[k] - lo[k] + 1 > SIZE[k] {
            // centre the window on the route box, clamp inside the kept box
            let centre = (rlo + rhi) / 2;
            let mut wlo = centre - SIZE[k] / 2;
            wlo = wlo.max(lo[k]).min(hi[k] - SIZE[k] + 1);
            lo[k] = wlo;
            hi[k] = wlo + SIZE[k] - 1;
        }
    }
    for (b, f) in m.blocks.iter().zip(fates.iter_mut()) {
        if matches!(f, Fate::Drop(_)) {
            continue;
        }
        let c = block_cell(b, ground);
        if c[0] < lo[0] || c[0] > hi[0] || c[2] < lo[2] || c[2] > hi[2] {
            *f = Fate::Drop("outside the 48-cell window");
            trimmed += 1;
        }
    }
    // items outside the window are scenery too
    let mut items_trimmed = 0usize;
    for (it, k) in m.items.iter().zip(item_keep.iter_mut()) {
        if !*k {
            continue;
        }
        let cx = (it.pos[0] / 32.0).floor() as i32;
        let cz = (it.pos[2] / 32.0).floor() as i32;
        if cx < lo[0] || cx > hi[0] || cz < lo[2] || cz > hi[2] {
            if it.waypoint_tag.is_some() {
                fits = false; // a waypoint past the 48-cell window: the port cannot hold the route
                continue;
            }
            // (an `isolate`d probe map keeps its one template item wherever it is)
            if m.items.len() == 1 {
                continue;
            }
            *k = false;
            items_trimmed += 1;
        }
    }
    if items_trimmed > 0 {
        trimmed += items_trimmed;
    }
    // y: the lowest kept block on the ground; the top must stay under row 40
    let kept_cells: Vec<[i32; 3]> = m.blocks.iter().zip(&fates).filter(|(_, f)| !matches!(f, Fate::Drop(_))).map(|(b, _)| block_cell(b, ground)).collect();
    let (klo, khi) = route_box(&kept_cells).ok_or("no built blocks in the window")?;
    let mut dy = GROUND_CELL - klo[1];
    if khi[1] + dy > SIZE[1] - 1 {
        dy = SIZE[1] - 1 - khi[1];
        if klo[1] + dy < 7 {
            fits = false; // even sunk two rows it does not fit under the roof
        }
    }
    let mut dx = (SIZE[0] - (hi[0] - lo[0] + 1)) / 2 - lo[0];
    let mut dz = (SIZE[2] - (hi[2] - lo[2] + 1)) / 2 - lo[2];
    // TMMAPS_PORT_SHIFT=dx,dy,dz: a fixed shift (an `isolate`d probe map ported with the
    // full map's shift, so the same relative cameras frame both — the piece A/B of 2026-09-24)
    if let Some(v) = std::env::var("TMMAPS_PORT_SHIFT").ok().map(|v| v.split(',').filter_map(|x| x.trim().parse::<i32>().ok()).collect::<Vec<_>>()).filter(|v| v.len() == 3) {
        dx = v[0];
        dy = v[1];
        dz = v[2];
    }
    for (it, k) in m.items.iter().zip(item_keep.iter_mut()) {
        if *k {
            let row = ((it.pos[1] - ground) / 8.0).floor() as i32 + dy;
            if row < 0 || row >= SIZE[1] {
                if it.waypoint_tag.is_some() {
                    fits = false;
                    continue;
                }
                *k = false;
                trimmed += 1;
            }
        }
    }
    let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
    let mut twins: BTreeMap<String, usize> = BTreeMap::new();
    for (b, f) in m.blocks.iter().zip(&fates) {
        match f {
            Fate::Drop(_) => *dropped.entry(b.name.clone()).or_default() += 1,
            Fate::Twin(t, dy, turn) => *twins.entry(format!("{} → {t}{}{}", b.name, if *dy != 0 { format!(" (+{dy} rows)") } else { String::new() }, if *turn != 0 { format!(" ({}°)", *turn as u32 * 90) } else { String::new() })).or_default() += 1,
            Fate::Keep => {}
        }
    }
    let mut items_dropped: BTreeMap<String, usize> = BTreeMap::new();
    for (it, k) in m.items.iter().zip(&item_keep) {
        if !k {
            *items_dropped.entry(it.model.clone()).or_default() += 1;
        }
    }
    let mood = m.decoration_id.trim_end_matches("64").to_string();
    let mood = if ["Day", "Sunrise", "Sunset", "Night"].contains(&mood.as_str()) { mood } else { "Day".to_string() };
    Ok(Plan { collection, ground, fates, item_keep, shift: [dx, dy, dz], bounds: (klo, khi), fits, dropped, twins, items_dropped, trimmed, deco: format!("48x48Screen155{mood}") })
}

pub fn scan_cmd(args: &[String]) -> Result<(), String> {
    println!("map\tcoll\tkept\ttwins\tdropped\ttrimmed\titems_kept\titems_dropped\tbounds\tshift\tfits\tno_twin\tdeco");
    for input in args.iter().skip(1).filter(|a| a.ends_with(".Map.Gbx")) {
        let m = MapFile::load(Path::new(input));
        match plan(&m, Path::new(input)) {
            Err(e) => println!("{}\t-\t{e}", Path::new(input).file_name().unwrap().to_string_lossy()),
            Ok(p) => {
                let kept = p.fates.iter().filter(|f| matches!(f, Fate::Keep)).count();
                let twins = p.fates.iter().filter(|f| matches!(f, Fate::Twin(..))).count();
                let dropped = p.fates.iter().filter(|f| matches!(f, Fate::Drop(_))).count();
                let no_twin: Vec<String> = m.blocks.iter().zip(&p.fates).filter(|(_, f)| **f == Fate::Drop("terrain variant without a twin")).map(|(b, _)| b.name.clone()).collect::<BTreeSet<_>>().into_iter().collect();
                println!(
                    "{}\t{:#x}\t{kept}\t{twins}\t{dropped}\t{}\t{}\t{}\t{:?}..{:?}\t{:?}\t{}\t{}\t{}",
                    Path::new(input).file_name().unwrap().to_string_lossy(),
                    p.collection,
                    p.trimmed,
                    p.item_keep.iter().filter(|k| **k).count(),
                    p.item_keep.iter().filter(|k| !**k).count(),
                    p.bounds.0,
                    p.bounds.1,
                    p.shift,
                    if p.fits { "FITS" } else { "NO-FIT" },
                    no_twin.join(","),
                    p.deco
                );
            }
        }
    }
    Ok(())
}

pub fn port_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let input = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("tmmaps stadium-port MAP --out OUT [--name N] [--force]")?);
    let out = PathBuf::from(f("--out").ok_or("--out")?);
    let source = MapFile::load(&input);
    // a Stadium input under --copy: the same map with a fresh uid (and --name) — the
    // campaign's native Stadium maps ride along with the ports
    if crate::cli::has(args, "--copy") && source.items.first().map(|it| it.collection_raw).unwrap_or(26) == STADIUM {
        let mut m = MapFile::load(&input);
        let hdr = crate::header::read(input.to_str().unwrap_or_default()).ok();
        let old_name = hdr.as_ref().map(|h| h.name.clone()).unwrap_or_default();
        // deterministic: `Stad` + the source uid's tail, so a rebuild updates the same record
        let src_uid = hdr.as_ref().map(|h| h.uid.clone()).unwrap_or_default();
        m.set_map_uid(&stable_uid("Stad", &src_uid));
        m.write_to(&out).map_err(|e| e.to_string())?;
        let n = gen_name(&f("--name").unwrap_or_else(|| old_name.clone()));
        if !old_name.is_empty() && n != old_name {
            let mut m = MapFile::load(&out);
            m.set_map_name(&old_name, &n);
            m.write_to(&out).map_err(|e| e.to_string())?;
        }
        println!("{}: a Stadium map already — copied with a fresh uid → {}", input.display(), out.display());
        return Ok(());
    }
    let p = plan(&source, &input)?;
    let kept = p.fates.iter().filter(|f| matches!(f, Fate::Keep)).count();
    let twins = p.fates.iter().filter(|f| matches!(f, Fate::Twin(..))).count();
    println!("{}: collection {:#x}, {} blocks → {kept} kept + {twins} twinned, {} dropped ({} trimmed as far scenery); items {} kept / {} dropped; shift {:?}; {}", input.display(), p.collection, source.blocks.len(), p.fates.iter().filter(|f| matches!(f, Fate::Drop(_))).count(), p.trimmed, p.item_keep.iter().filter(|k| **k).count(), p.item_keep.iter().filter(|k| !**k).count(), p.shift, if p.fits { "fits" } else { "DOES NOT FIT" });
    if !p.fits && !crate::cli::has(args, "--force") {
        return Err("the route does not fit the 48×40×48 stadium (--force ports the window anyway)".into());
    }
    let mut v: Vec<(&String, &usize)> = p.twins.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1));
    if !v.is_empty() {
        println!("  twins: {}", v.iter().take(12).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(", "));
    }
    let mut v: Vec<(&String, &usize)> = p.dropped.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1));
    println!("  dropped blocks: {}", v.iter().take(10).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(", "));
    let mut v: Vec<(&String, &usize)> = p.items_dropped.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1));
    println!("  dropped items: {}", v.iter().take(8).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(", "));

    let tmp = out.with_extension("tmp1.Map.Gbx");
    // 1. drop blocks (all baked too: the vista terrain's generated pieces) and items
    {
        let mut m = MapFile::load(&input);
        let fates = p.fates.clone();
        let r = m.remove_and_add_blocks(|b| matches!(fates[b.index], Fate::Drop(_)), |_| true, &[]);
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        println!("  stage 1: {} blocks + {} baked removed", r.blocks, r.baked);
        let mut m = MapFile::load(&tmp);
        let n = m.remove_items(&p.item_keep);
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        println!("  stage 1b: {n} items removed");
    }
    // 2. twins (renames), collection words, decoration, size
    let kept_fates: Vec<Fate> = p.fates.iter().filter(|f| !matches!(f, Fate::Drop(_))).cloned().collect();
    {
        let mut m = MapFile::load(&tmp);
        assert_eq!(m.blocks.len(), kept_fates.len(), "kept block count");
        for (i, fte) in kept_fates.iter().enumerate() {
            if let Fate::Twin(t, _, turn) = fte {
                m.set_block_name(i, t);
                if *turn != 0 {
                    let d = m.blocks[i].dir;
                    m.set_block_dir(i, (d + *turn) & 3);
                }
            }
        }
        m.set_decoration(&p.deco);
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m = MapFile::load(&tmp);
        let n = m.set_map_collection(STADIUM);
        if !crate::cli::has(args, "--keep-size") {
            m.set_map_size([SIZE[0] as u32, SIZE[1] as u32, SIZE[2] as u32]);
        }
        for i in 0..m.items.len() {
            m.set_item_collection(i, STADIUM);
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        println!("  stage 2: {twins} twins renamed, decoration {}, {n} collection words → 26, size 48×40×48", p.deco);
    }
    // 3. the shift (cells + world positions)
    {
        let mut m = MapFile::load(&tmp);
        let [dx, dy, dz] = p.shift;
        let world = [dx as f32 * 32.0, dy as f32 * 8.0, dz as f32 * 32.0];
        // world y of the SAME cell row differs between the vista's frame and the stadium's
        // by (ground_stadium − ground_vista): items and free blocks carry metres
        let ground_delta = crate::map::ground_y(STADIUM) - p.ground;
        let mut moved = 0usize;
        let mut risen = 0usize;
        for i in 0..m.blocks.len() {
            let b = m.blocks[i].clone();
            let up = match &kept_fates[i] {
                Fate::Twin(_, rows, _) => *rows,
                _ => 0,
            };
            if up != 0 {
                risen += 1;
            }
            match b.free_pos {
                // one move: the cell shift + the stadium's cell frame (the vista's ground row
                // is not the stadium's) — a second move would start from the OLD position
                Some(pos) => m.move_block_free(i, [pos[0] + world[0], pos[1] + world[1] + up as f32 * 8.0 + ground_delta, pos[2] + world[2]]),
                None => {
                    let (x, y, z) = b.coords();
                    m.move_block_cell(i, (x + dx, y + dy + up, z + dz));
                }
            }
            moved += 1;
        }
        if risen > 0 {
            println!("  {risen} twins raised to their deck row");
        }
        crate::tiny::set_ground(STADIUM);
        for i in 0..m.items.len() {
            let it = m.items[i].clone();
            let pos = [it.pos[0] + world[0], it.pos[1] + world[1] + ground_delta, it.pos[2] + world[2]];
            m.move_item(i, pos, it.yaw, crate::tiny::cell_for(pos));
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        // every block and item inside the stadium (a block outside makes the load fail)
        let mut m = MapFile::load(&tmp);
        let mut outside = 0usize;
        for b in &m.blocks {
            let c = block_cell(b, crate::map::ground_y(STADIUM));
            if c[0] < 0 || c[0] >= SIZE[0] || c[1] < 0 || c[1] >= SIZE[1] || c[2] < 0 || c[2] >= SIZE[2] {
                outside += 1;
                println!("  OUTSIDE: {} at {:?} (free {:?})", b.name, c, b.free_pos);
            }
        }
        for it in &m.items {
            if it.pos[0] < 0.0 || it.pos[0] >= 48.0 * 32.0 || it.pos[2] < 0.0 || it.pos[2] >= 48.0 * 32.0 || it.pos[1] < -64.0 || it.pos[1] >= 40.0 * 8.0 - 64.0 {
                outside += 1;
                println!("  OUTSIDE item: {} at {:?}", it.model, it.pos);
            }
        }
        if outside > 0 {
            return Err(format!("{outside} blocks/items outside the 48×40×48 stadium"));
        }
        println!("  stage 3: {moved} blocks and {} items shifted by {:?} cells (world y +{ground_delta:.0} for the stadium's cell frame)", m.items.len(), p.shift);
        // 3b. the GROUND bit (0x1000): a native Stadium map carries it only on row 9 —
        // a vista block wore it on its hill; off the ground row it becomes the air variant
        if !crate::cli::has(args, "--keep-ground-bit") {
            let mut m = MapFile::load(&tmp);
            let mut cleared = 0usize;
            for i in 0..m.blocks.len() {
                let b = m.blocks[i].clone();
                let (_, y, _) = b.coords();
                if b.flags & 0x1000 != 0 && b.free_pos.is_none() && y != GROUND_CELL {
                    m.set_block_flags(i, b.flags & !0x1000);
                    cleared += 1;
                }
            }
            m.write_to(&tmp).map_err(|e| e.to_string())?;
            println!("  stage 3b: the ground bit cleared on {cleared} blocks off row {GROUND_CELL}");
        }
    }
    // 3a. the MediaTracker (its own load/write: a variable-length splice)
    {
        let mut m = MapFile::load(&tmp);
        let [dx, dy, dz] = p.shift;
        let world = [dx as f32 * 32.0, dy as f32 * 8.0, dz as f32 * 32.0];
        let ground_delta = crate::map::ground_y(STADIUM) - p.ground;
        // camera keys are world positions, the in-game clip TRIGGERS
        // are cells of the trigger grid (ts per block) — both follow the shift, or the
        // camera changes fire nowhere (json9841 2026-09-22: "Cam switches are removed")
        match m.mediatracker() {
            None => {}
            Some(Err(e)) => println!("  stage 3a: MediaTracker left untouched ({e})"),
            Some(Ok(mut mt)) => {
                let ts = mt.trigger_size.unwrap_or([3, 1, 3]);
                let point = move |q: [f32; 3]| [q[0] + world[0], q[1] + ground_delta + world[1], q[2] + world[2]];
                let cell = move |c: [i32; 3]| vec![[(c[0] + dx * ts[0]).max(0), (c[1] + dy * ts[1]).max(0), (c[2] + dz * ts[2]).max(0)]];
                let (keys, verts, (c0, c1), left) = mt.transform(&point, 1.0, &cell);
                m.set_mediatracker(&mt);
                println!("  stage 3a: MediaTracker {} clips: {keys} camera keys, {verts} vertices moved, trigger cells {c0} → {c1}, {left} blocks left alone", mt.clips().len());
            }
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
    }
    // 4. header (collection, decoration, envir), ghost, lightmap, genealogy, uid, name
    {
        let mut m = MapFile::load(&tmp);
        m.set_header_common(STADIUM, &p.deco);
        m.edit_header_xml(&|x: &str| {
            let mut s = x.to_string();
            for e in ["BlueBay", "RedIsland", "WhiteShore", "GreenCoast"] {
                s = s.replace(&format!("envir=\"{e}\""), "envir=\"Stadium\"");
            }
            if s != x { Some(s) } else { None }
        });
        let g = m.strip_validation_ghost();
        let l = m.strip_lightmap();
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let gen = MapFile::clear_genealogy_file(&tmp).map(|n| n).unwrap_or(0);
        // chunk 0x0304305D: a 726-byte structure in the vista maps (a Stadium map's is
        // `version 1, count 0`) — emptied to the Stadium form
        if !crate::cli::has(args, "--keep-5d") {
            let _ = MapFile::set_chunk_payload_file(&tmp, 0x0304_305D, &[1u8, 0, 0, 0, 0, 0, 0, 0]);
        }
        // chunk 0x03043052: `version 0, 8` in every Stadium map, `0, 0` in the vista ones —
        // the 8 rows of the stadium under its ground (row 9 = 8 m: the cell frame's −64)
        if !crate::cli::has(args, "--keep-52") {
            let _ = MapFile::set_chunk_payload_file(&tmp, 0x0304_3052, &[0u8, 0, 0, 0, 8, 0, 0, 0]);
        }
        if crate::cli::has(args, "--stadium-59") {
            let mut b = vec![3u8, 0, 0, 0];
            b.extend_from_slice(&[0u8; 16]);
            b.extend_from_slice(&20.0f32.to_le_bytes());
            b.extend_from_slice(&3.0f32.to_le_bytes());
            let _ = MapFile::set_chunk_payload_file(&tmp, 0x0304_3059, &b);
        }
        let mut m = MapFile::load(&tmp);
        let hdr = crate::header::read(tmp.to_str().unwrap_or_default()).ok();
        let old_name = hdr.as_ref().map(|h| h.name.clone()).unwrap_or_default();
        let src_uid = hdr.as_ref().map(|h| h.uid.clone()).unwrap_or_default();
        let new_name = gen_name(&f("--name").unwrap_or_else(|| format!("{old_name} [Stadium]")));
        m.set_map_uid(&stable_uid("Stad", &src_uid));
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m = MapFile::load(&tmp);
        if !old_name.is_empty() && new_name != old_name {
            m.set_map_name(&old_name, &new_name);
        }
        m.write_to(&out).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
        println!("  stage 4: header → Stadium/{}, ghost {g}, lightmap {l}, genealogy {gen} cleared → {} ({new_name})", p.deco, out.display());
    }
    Ok(())
}

/// `tmmaps stadium-info MAP`: the body's map/decoration idents and size (the port's checks).
pub fn info_cmd(args: &[String]) -> Result<(), String> {
    for input in args.iter().skip(1).filter(|a| a.ends_with(".Map.Gbx")) {
        let m = MapFile::load(Path::new(input));
        let ids: Vec<String> = m.body_ids.iter().take(6).map(|f| match &f.name { Some(s) => s.clone(), None => format!("#{}", f.raw) }).collect();
        let f5 = &m.body_ids[5];
        let so = f5.off + f5.len;
        let w = |o: usize| u32::from_le_bytes(m.gbx.body[o..o + 4].try_into().unwrap());
        println!("{}: body ids {:?} decoration {} size {:?} needUnlock {} blocks-version {} nb {} items coll {:?}", input, ids, m.decoration_id, m.size, w(so + 12), w(so + 16), w(so + 20), m.items.first().map(|it| it.collection_raw));
    }
    Ok(())
}

/// `tmmaps stadium-chunkdump MAP --chunk 0x0304305D [--max N]`: a body chunk's payload as hex + u32/f32 views.
pub fn chunkdump_cmd(args: &[String]) -> Result<(), String> {
    let input = args.iter().skip(1).find(|a| a.ends_with(".Map.Gbx")).ok_or("MAP")?;
    let cid = u32::from_str_radix(crate::cli::flag(args, "--chunk").ok_or("--chunk")?.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let max: usize = crate::cli::flag(args, "--max").and_then(|v| v.parse().ok()).unwrap_or(96);
    let g = crate::gbx::Gbx::parse(&std::fs::read(input).map_err(|e| e.to_string())?);
    for (c, _off, payload, size) in crate::gbx::all_skip_chunks(&g.body) {
        if c != cid {
            continue;
        }
        let d = &g.body[payload..payload + size.min(max)];
        println!("{input}: chunk {cid:#010x} {size} bytes");
        println!("  hex {}", d.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "));
        let u: Vec<String> = d.chunks(4).filter(|w| w.len() == 4).map(|w| format!("{}", u32::from_le_bytes(w.try_into().unwrap()))).collect();
        println!("  u32 {}", u.join(" "));
        let fl: Vec<String> = d.chunks(4).filter(|w| w.len() == 4).map(|w| format!("{:.2}", f32::from_le_bytes(w.try_into().unwrap()))).collect();
        println!("  f32 {}", fl.join(" "));
    }
    Ok(())
}

/// `tmmaps stadium-strip MAP --out OUT [--no-genealogy] [--no-5d] [--no-lightmap] [--no-baked]`:
/// the port's stage-4 strips on ANY map (the control: a native Stadium map put
/// through them must play exactly as before).
pub fn strip_cmd(args: &[String]) -> Result<(), String> {
    let input = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("MAP")?);
    let out = PathBuf::from(crate::cli::flag(args, "--out").ok_or("--out")?);
    std::fs::copy(&input, &out).map_err(|e| e.to_string())?;
    let mut m = MapFile::load(&out);
    if !crate::cli::has(args, "--no-baked") {
        let r = m.remove_and_add_blocks(|_| false, |_| true, &[]);
        println!("  baked removed: {}", r.baked);
        m.write_to(&out).map_err(|e| e.to_string())?;
        m = MapFile::load(&out);
    }
    let g = m.strip_validation_ghost();
    let l = if crate::cli::has(args, "--no-lightmap") { 0 } else { m.strip_lightmap() };
    m.write_to(&out).map_err(|e| e.to_string())?;
    let gen = if crate::cli::has(args, "--no-genealogy") { 0 } else { MapFile::clear_genealogy_file(&out).unwrap_or(0) };
    if !crate::cli::has(args, "--no-5d") {
        let _ = MapFile::set_chunk_payload_file(&out, 0x0304_305D, &[1u8, 0, 0, 0, 0, 0, 0, 0]);
    }
    let mut m = MapFile::load(&out);
    let hdr = crate::header::read(out.to_str().unwrap_or_default()).ok();
    let uid_len = hdr.as_ref().map(|h| h.uid.len()).unwrap_or(27);
    m.set_map_uid(&crate::altered::new_uid("Strp", uid_len));
    m.write_to(&out).map_err(|e| e.to_string())?;
    println!("{} -> {}: ghost {g}, lightmap {l}, genealogy {gen}", input.display(), out.display());
    Ok(())
}

/// `tmmaps stadium-verify SRC.Map.Gbx PORT.Map.Gbx`: the source's validation ghost mapped
/// through the port's shift, then for every sample whether a kept block sits under it
/// (its cell column, the sample within −1..+12 m of a row the block occupies). A 1:1
/// port supports the whole route; a wrongly raised twin or a missing piece shows as
/// unsupported samples — printed with the nearest block name.
pub fn verify_cmd(args: &[String]) -> Result<(), String> {
    let maps: Vec<&String> = args.iter().skip(1).filter(|a| a.ends_with(".Map.Gbx")).collect();
    let (src, port) = (Path::new(maps.first().ok_or("SRC PORT")?), Path::new(maps.get(1).ok_or("SRC PORT")?));
    let s = MapFile::load(src);
    let p = plan(&s, src)?;
    // --self: the source checked against itself (shift 0, its own frame) — the control
    // that says what this crude model reports for a map that IS right
    let selfcheck = crate::cli::has(args, "--self");
    let m = if selfcheck { MapFile::load(src) } else { MapFile::load(port) };
    let d = gbx::record::decode_ghost(src.to_str().unwrap_or_default()).map_err(|e| format!("no ghost: {e}"))?;
    let gsrc = p.ground;
    let gst = if selfcheck { gsrc } else { crate::map::ground_y(STADIUM) };
    let mut p = p;
    if selfcheck {
        p.shift = [0, 0, 0];
    }
    // the port's blocks by (cx, cz): rows occupied (origin row .. origin row + height − 1; height
    // from the plain block's units is unknown here → 2 rows for every block)
    let mut cols: HashMap<(i32, i32), Vec<(i32, String)>> = HashMap::new();
    for b in &m.blocks {
        let c = block_cell(b, gst);
        cols.entry((c[0], c[2])).or_default().push((c[1], b.name.clone()));
        // a multi-cell piece covers its neighbours too (footprint unknown: 1 cell each way)
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            cols.entry((c[0] + dx, c[2] + dz)).or_default().push((c[1], format!("~{}", b.name)));
        }
    }
    // the SOURCE's blocks by column (dropped ones included): what the route really rode on
    let mut scols: HashMap<(i32, i32), Vec<(i32, String)>> = HashMap::new();
    for b in &s.blocks {
        let c = block_cell(b, gsrc);
        scols.entry((c[0], c[2])).or_default().push((c[1], b.name.clone()));
    }
    let mut on_dropped: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0usize;
    let mut supported = 0usize;
    let mut misses: Vec<(f32, [f32; 3], String)> = Vec::new();
    for smp in d.samples.iter().step_by(5) {
        // world → cell (source frame) → shifted cell → stadium world
        let cx = (smp.x / 32.0).floor() as i32 + p.shift[0];
        let cz = (smp.z / 32.0).floor() as i32 + p.shift[2];
        let y = smp.y - gsrc + gst + p.shift[1] as f32 * 8.0; // stadium world y
        total += 1;
        let mut ok = false;
        let mut nearest: Option<(f32, String)> = None;
        if let Some(v) = cols.get(&(cx, cz)) {
            for (row, name) in v {
                let floor = *row as f32 * 8.0 + gst;
                let dy = y - floor;
                if dy >= -1.0 && dy <= 12.0 {
                    ok = true;
                    break;
                }
                if nearest.as_ref().map(|(d, _)| dy.abs() < *d).unwrap_or(true) {
                    nearest = Some((dy.abs(), format!("{name} {dy:+.0} m")));
                }
            }
        }
        if ok {
            supported += 1;
        } else {
            // what the source had in this column within reach of the sample
            let sc = ((smp.x / 32.0).floor() as i32, (smp.z / 32.0).floor() as i32);
            if let Some(v) = scols.get(&sc) {
                for (row, name) in v {
                    let dy = smp.y - (*row as f32 * 8.0 + gsrc);
                    if dy >= -1.0 && dy <= 12.0 {
                        *on_dropped.entry(name.clone()).or_default() += 1;
                    }
                }
            }
            misses.push((smp.time_ms as f32 / 1000.0, [smp.x, smp.y, smp.z], nearest.map(|(_, n)| n).unwrap_or_else(|| "nothing in the column".into())));
        }
    }
    println!("{}: {} samples, {} supported ({:.0}%), shift {:?}", port.file_name().unwrap().to_string_lossy(), total, supported, supported as f32 * 100.0 / total.max(1) as f32, p.shift);
    // the misses grouped by nearest name (airborne stretches show as "nothing in the column")
    let mut by: BTreeMap<String, (usize, f32)> = BTreeMap::new();
    for (t, _, n) in &misses {
        let key = n.split(' ').next().unwrap_or(n).to_string();
        let e = by.entry(key).or_insert((0, *t));
        e.0 += 1;
    }
    let mut v: Vec<_> = by.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (name, (n, t)) in v.iter().take(8) {
        println!("  unsupported ×{n} near {name} (first at {t:.1} s)");
    }
    if let Some((t, pos, _)) = misses.first() {
        let y = pos[1] - gsrc + gst + p.shift[1] as f32 * 8.0;
        println!("  first unsupported sample at {t:.1} s: target frame ({:.0}, {y:.0}, {:.0})", pos[0] + p.shift[0] as f32 * 32.0, pos[2] + p.shift[2] as f32 * 32.0);
    }
    let mut od: Vec<_> = on_dropped.into_iter().collect();
    od.sort_by(|a, b| b.1.cmp(&a.1));
    if !od.is_empty() {
        println!("  the source had under those samples: {}", od.iter().take(10).map(|(n, k)| format!("{n}×{k}")).collect::<Vec<_>>().join(" "));
    }
    Ok(())
}

/// `tmmaps stadium-terrain PORT.Map.Gbx --source SRC.Map.Gbx --out OUT.Map.Gbx [--mode route|window]
/// [--mapgeom BIN] --pak FILE:KEY [--pak FILE:KEY] [--keep-work]`: the vista landscape the port
/// dropped, baked into ×1 static items (mapgeom tiny-library on a copy of the source pruned to
/// the chosen terrain blocks, block infos from the vista pack, items written for Stadium) and
/// placed in the port at the same cells — the route rides on landscape for whole stretches
/// (Summer 2026 - 01: 20% of the lap on flat Land; json9841 2026-09-22 "many maps are broken",
/// vjeux "we are reproducing all the block geometry anyway"). `route` bakes the terrain within
/// one cell of the validation ghost's path (the drivable fix); `window` every landscape piece
/// inside the 48-cell window (the look). Water zone tiles (Water/Sea/Lake/WaterHill*) are never
/// baked: an item carries no water volume.
pub fn terrain_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    // the first positional argument (a flag's value is not one)
    let mut positional: Option<String> = None;
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        if a.starts_with("--") {
            if !matches!(a.as_str(), "--keep-work") {
                it.next();
            }
            continue;
        }
        positional = Some(a.clone());
        break;
    }
    let port_path = PathBuf::from(positional.ok_or("PORT.Map.Gbx")?);
    let src_path = PathBuf::from(f("--source").ok_or("--source SRC.Map.Gbx")?);
    let out = PathBuf::from(f("--out").ok_or("--out OUT.Map.Gbx")?);
    let mode = f("--mode").unwrap_or_else(|| "route".into());
    let paks: Vec<String> = args.windows(2).filter(|w| w[0] == "--pak").map(|w| w[1].clone()).collect();
    if paks.is_empty() {
        return Err("--pak FILE:KEY (the vista pack and Stadium.pak)".into());
    }
    let mapgeom = f("--mapgeom").map(PathBuf::from).unwrap_or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("mapgeom"))).unwrap_or_else(|| PathBuf::from("mapgeom")));
    let s = MapFile::load(&src_path);
    let p = plan(&s, &src_path)?;
    let gsrc = p.ground;
    let gst = crate::map::ground_y(STADIUM);
    let is_water_zone = |n: &str| {
        let l = n.to_ascii_lowercase();
        l == "water" || l == "sea" || l == "lake" || l.starts_with("waterhill") || l.starts_with("lakehill") || l.starts_with("seahill")
    };
    // the route's columns (±1 cell) from the validation ghost
    let mut route_cols: HashSet<(i32, i32)> = HashSet::new();
    // `near`: the columns within two cells of any KEPT block — the route's surroundings
    // without a ghost (France 2026 / Poland 2026 carry none)
    let mode = if mode == "route" && gbx::record::decode_ghost(src_path.to_str().unwrap_or_default()).is_err() {
        println!("  no validation ghost: --mode route → near");
        "near".to_string()
    } else {
        mode
    };
    if mode == "route" {
        let d = gbx::record::decode_ghost(src_path.to_str().unwrap_or_default()).map_err(|e| format!("no validation ghost for --mode route: {e}"))?;
        for smp in &d.samples {
            let (cx, cz) = ((smp.x / 32.0).floor() as i32, (smp.z / 32.0).floor() as i32);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    route_cols.insert((cx + dx, cz + dz));
                }
            }
        }
    } else if mode == "near" || mode == "near1" {
        let r: i32 = if mode == "near1" { 1 } else { 2 };
        for (i, b) in s.blocks.iter().enumerate() {
            if matches!(p.fates[i], Fate::Drop(_)) {
                continue;
            }
            let c = block_cell(b, gsrc);
            for dx in -r..=r {
                for dz in -r..=r {
                    route_cols.insert((c[0] + dx, c[2] + dz));
                }
            }
        }
    }
    let lo = [-p.shift[0], -p.shift[1], -p.shift[2]];
    let hi = [lo[0] + 47, lo[1] + 39, lo[2] + 47];
    let mut selected: Vec<usize> = Vec::new();
    let mut by_name: BTreeMap<String, usize> = BTreeMap::new();
    for (i, b) in s.blocks.iter().enumerate() {
        if !matches!(p.fates[i], Fate::Drop(_)) || is_water_zone(&b.name) || b.free_pos.is_some() {
            continue;
        }
        // the tree decorations bake with embedded texture pictures — kept out (look-only,
        // and the console's item loader is the open question of 2026-09-22)
        if b.name.contains("Tree") {
            continue;
        }
        let c = block_cell(b, gsrc);
        if c[0] < lo[0] || c[0] > hi[0] || c[2] < lo[2] || c[2] > hi[2] || c[1] < lo[1] || c[1] > hi[1] {
            continue;
        }
        if (mode == "route" || mode == "near" || mode == "near1") && !route_cols.contains(&(c[0], c[2])) {
            continue;
        }
        // the tightest mode keeps the ground pieces only (no decoration/vegetation blocks)
        if mode == "near1" && (b.name.starts_with("Deco") || b.name.contains("Tree")) {
            continue;
        }
        selected.push(i);
        *by_name.entry(b.name.clone()).or_default() += 1;
    }
    println!("{}: {} landscape blocks selected ({mode}): {}", src_path.file_name().unwrap().to_string_lossy(), selected.len(), by_name.iter().map(|(n, k)| format!("{n}×{k}")).collect::<Vec<_>>().join(" "));
    if selected.is_empty() {
        std::fs::copy(&port_path, &out).map_err(|e| e.to_string())?;
        println!("  nothing to bake; port copied to {}", out.display());
        return Ok(());
    }
    // the pruned source: only the selected blocks (their original indices, in order), no items
    let work = out.with_extension(format!("terrain-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let pruned = work.join("pruned.Map.Gbx");
    {
        let keep: HashSet<usize> = selected.iter().cloned().collect();
        let mut m = MapFile::load(&src_path);
        let r = m.remove_and_add_blocks(|b| !keep.contains(&b.index), |_| true, &[]);
        m.write_to(&pruned).map_err(|e| e.to_string())?;
        let mut m = MapFile::load(&pruned);
        let n = m.items.len();
        if n > 1 {
            let mut keep_items = vec![false; n];
            keep_items[0] = true; // one record stays: tiny-library reads the collection off it
            m.remove_items(&keep_items);
            m.write_to(&pruned).map_err(|e| e.to_string())?;
        }
        println!("  pruned source: {} blocks kept ({:?})", keep.len(), r);
    }
    // the bake
    let lib = work.join("lib.zip");
    let mapping = work.join("placements.tsv");
    let report = work.join("report.tsv");
    let env = match p.collection {
        0x1c => "BlueBay",
        0x10 => "RedIsland",
        0x1d => "WhiteShore",
        0xf => "GreenCoast",
        _ => "Stadium",
    };
    let mut cmd = std::process::Command::new(&mapgeom);
    for pk in &paks {
        cmd.arg("--pak").arg(pk);
    }
    cmd.arg("tiny-library").arg(&pruned).arg("--scale").arg("1").arg("--library-out").arg(&lib).arg("--mapping-out").arg(&mapping).arg("--report").arg(&report).arg("--collection").arg(env);
    cmd.env("MAPGEOM_ITEM_COLLECTION", "26").env("TINY_WATER_BLOCKS", "0");
    // --alias-base N: the AC item numbering starts at N — the game caches embedded item
    // MODELS by file name for the whole session, so every map needs its own range
    // (json9841 2026-09-22: "Missing Items: AC00000005…" on the second map opened)
    if let Some(base) = f("--alias-base") {
        cmd.env("TINY_ALIAS_BASE", &base);
        cmd.env("TINY_PICTURE_SUFFIX", format!("_t{base}"));
    }
    let o = cmd.output().map_err(|e| format!("{}: {e}", mapgeom.display()))?;
    let log = work.join("tiny-library.log");
    std::fs::write(&log, [o.stdout.as_slice(), o.stderr.as_slice()].concat()).ok();
    if !o.status.success() {
        return Err(format!("mapgeom tiny-library failed ({}); log {}", o.status, log.display()));
    }
    // the mapping: @pruned_index → item file + footprint
    let text = std::fs::read_to_string(&mapping).map_err(|e| e.to_string())?;
    let mut placements: Vec<(String, [f32; 3], f32)> = Vec::new();
    let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
    crate::tiny::set_ground(p.collection);
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let Some(idx) = fields[0].strip_prefix('@').and_then(|v| v.parse::<usize>().ok()) else { continue };
        let model = fields.get(1).copied().unwrap_or("-");
        let src_i = *selected.get(idx).ok_or_else(|| format!("mapping index {idx} beyond the {} pruned blocks", selected.len()))?;
        let b = &s.blocks[src_i];
        if !model.ends_with(".Item.Gbx") {
            *skipped.entry(format!("{} ({model})", b.name)).or_default() += 1;
            continue;
        }
        let sx: u32 = fields.get(3).and_then(|v| v.parse().ok()).unwrap_or(1);
        let sz: u32 = fields.get(4).and_then(|v| v.parse().ok()).unwrap_or(1);
        let o = crate::tiny::block_origin(b, (sx, sz));
        let pos = [o[0] + p.shift[0] as f32 * 32.0, o[1] - gsrc + gst + p.shift[1] as f32 * 8.0, o[2] + p.shift[2] as f32 * 32.0];
        placements.push((model.to_string(), pos, crate::tiny::block_yaw(b)));
    }
    println!("  bake: {} items placed{}", placements.len(), if skipped.is_empty() { String::new() } else { format!("; not baked: {}", skipped.iter().map(|(n, k)| format!("{n}×{k}")).collect::<Vec<_>>().join(" ")) });
    // into the port
    let tmp = out.with_extension(format!("tmp-{}.Map.Gbx", std::process::id()));
    std::fs::copy(&port_path, &tmp).map_err(|e| e.to_string())?;
    {
        let mut m = MapFile::load(&tmp);
        if m.items.is_empty() {
            let g = crate::gbx::Gbx::parse(&std::fs::read(&tmp).map_err(|e| e.to_string())?);
            let body = crate::map::seed_item_record(&g.body, STADIUM)?;
            std::fs::write(&tmp, g.write_body_recompressed(&body)).map_err(|e| e.to_string())?;
            let mut m = MapFile::load(&tmp);
            m.move_item(0, [8.0, -900.0, 8.0], 0.0, crate::tiny::cell_for([8.0, -900.0, 8.0]));
            m.write_to(&tmp).map_err(|e| e.to_string())?;
            m = MapFile::load(&tmp);
        }
        crate::tiny::set_ground(STADIUM);
        let first = m.items.len();
        m.append_item_clones(first + placements.len());
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m = MapFile::load(&tmp);
        for (k, (model, pos, yaw)) in placements.iter().enumerate() {
            let i = first + k;
            m.set_item_model(i, model);
            m.set_item_author(i, &crate::surface::item_author(model)); // TINY_ITEM_AUTHOR or the item's own name
            m.set_item_collection(i, STADIUM);
            m.move_item(i, *pos, *yaw, crate::tiny::cell_for(*pos));
            m.set_item_frame(i, [*yaw, 0.0, 0.0], [0.0; 3]);
            m.set_item_scale(i, 1.0);
            m.clear_item_variant(i);
            m.set_item_flags(i, 1);
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m = MapFile::load(&tmp);
        for k in 0..placements.len() {
            m.set_item_waypoint(first + k, None, 0);
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
    }
    // the archive: the port's files + the library's
    {
        let mut m = MapFile::load(&tmp);
        let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut rows: Vec<(String, String)> = Vec::new();
        if let Some((src_rows, src_zip)) = m.embedded_objects() {
            for (n, b) in crate::header::zip_entries(&src_zip) {
                files.insert(n, b);
            }
            rows = src_rows;
        }
        let zip_bytes = std::fs::read(&lib).map_err(|e| e.to_string())?;
        let used: HashSet<&str> = placements.iter().map(|(m, _, _)| m.as_str()).collect();
        let mut added = 0usize;
        for (n, b) in crate::header::zip_entries(&zip_bytes) {
            let entry = n.replace('\\', "/");
            let file = entry.rsplit('/').next().unwrap_or(&entry).to_string();
            // only the items placed ride along (the library also holds the terrain's
            // vegetation species and their texture pictures, not placed here)
            if !used.contains(file.as_str()) {
                continue;
            }
            // the manifest row is the item's IDENT as the placements name it — the full
            // file name, author = the same (the Tiny/Giant maps' form); the bare stem was
            // wrong (2026-09-22)
            if file.ends_with(".Item.Gbx") && !rows.iter().any(|(i, _)| *i == file) {
                rows.push((file.clone(), crate::surface::item_author(&file)));
            }
            files.entry(entry).or_insert(b);
            added += 1;
        }
        let zip = crate::header::deflated_zip(&files);
        let manifest: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        m.replace_embedded_objects(&manifest, &zip);
        m.write_to(&out).map_err(|e| e.to_string())?;
        println!("  archive: {} files ({added} from the terrain library), {} manifest rows, {} bytes", files.len(), rows.len(), zip.len());
    }
    let _ = std::fs::remove_file(&tmp);
    if !crate::cli::has(args, "--keep-work") {
        let _ = std::fs::remove_dir_all(&work);
    }
    // --max-bytes N: a window build over the server cap falls back to the route terrain
    if let Some(cap) = f("--max-bytes").and_then(|v| v.parse::<u64>().ok()) {
        let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        let next = match mode.as_str() {
            "window" => Some("route"),
            "route" | "near" => Some("near1"),
            _ => None,
        };
        if size > cap {
            if let Some(next) = next {
                println!("  {size} B over the {cap} B cap: rebuilding with --mode {next}");
                let mut a2: Vec<String> = args.to_vec();
                if let Some(i) = a2.iter().position(|x| x == "--mode") {
                    a2[i + 1] = next.into();
                } else {
                    a2.push("--mode".into());
                    a2.push(next.into());
                }
                return terrain_cmd(&a2);
            }
            println!("  {size} B over the {cap} B cap even in --mode {mode}: kept as is");
        }
    }
    println!("  -> {}", out.display());
    Ok(())
}


/// `<prefix>` + the source uid's tail: the same length as the source's, and the same
/// value on every rebuild, so a re-publish updates the record instead of adding one.
/// `ALTERED_NAME_SUFFIX=<text>`: appended to every generated map NAME (a formatting code such
/// as `$z` is invisible in game) — the console remembers a map by NAME as well as by id
/// (json9841 2026-09-23: fresh ids under the old names still failed; new names loaded).
pub fn gen_name(name: &str) -> String {
    match std::env::var("ALTERED_NAME_SUFFIX") {
        Ok(sfx) if !sfx.is_empty() && !name.ends_with(sfx.as_str()) => format!("{name}{sfx}"),
        _ => name.to_string(),
    }
}

/// `ALTERED_UID_GEN=<letter>`: a publish GENERATION — the console keeps a stale copy of a
/// map id it has opened, so every fix round ships under NEW ids (json9841 2026-09-23: the
/// same bytes loaded under a fresh id, not under the old one). The letter replaces the
/// character after the prefix; unset = the first generation (the id as before).
pub fn stable_uid(prefix: &str, src_uid: &str) -> String {
    if src_uid.len() <= prefix.len() + 1 {
        return crate::altered::new_uid(prefix, 27);
    }
    let Some(gen) = std::env::var("ALTERED_UID_GEN").ok().filter(|g| !g.is_empty()) else {
        return format!("{prefix}{}", &src_uid[prefix.len()..]);
    };
    // a generation: the tail is a deterministic hash of (source uid, generation) — NOT the
    // source tail with one letter changed: Nadeo's uid lookup is case-insensitive
    // (`StadBvJYD…` resolved to the live `StadbvJYD…` → "Mismatching map uid", Winter 14)
    let alphabet: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-".chars().collect();
    let mut x: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format!("{src_uid}:{gen}").bytes() {
        x ^= b as u64;
        x = x.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // the SAME length as the source uid (26 or 27 chars: set_map_uid keeps the record size —
    // Spring 2026 - 10's 26-char uid asserted on the box)
    let mut out = prefix.to_string();
    while out.len() < src_uid.len().max(prefix.len() + 8) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push(alphabet[(x % 64) as usize]);
    }
    out
}

/// `tmmaps stadium-slopedir MAP… [--block SUBSTR]`: per block NAME, the validation ghost's
/// height gradient over the block's footprint, in the block's LOCAL frame (dir 0: local
/// +x = world +x, local +z = world +z; dir 1..3 rotate clockwise seen from above). One
/// number per placement — the ghost's rise per metre along local z and along local x —
/// aggregated as the mean sign. The plain Stadium slopes (U10S maps) and the vista
/// On-terrain variants (the Nadeo 2026 maps) compared tell which twins are authored
/// the other way round (json9841 2026-09-22: "first downhill is backwards").
pub fn slopedir_cmd(args: &[String]) -> Result<(), String> {
    let want = crate::cli::flag(args, "--block").map(String::from);
    let mut acc: BTreeMap<String, Vec<(f32, f32)>> = BTreeMap::new();
    for path in args.iter().skip(1).filter(|a| a.ends_with(".Map.Gbx")) {
        let m = MapFile::load(Path::new(path));
        let coll = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
        let ground = crate::map::ground_y(coll);
        let Ok(d) = gbx::record::decode_ghost(path) else { continue };
        for b in &m.blocks {
            if b.free_pos.is_some() {
                continue;
            }
            if let Some(w) = &want {
                if !b.name.contains(w.as_str()) {
                    continue;
                }
            }
            if !b.name.contains("Slope") {
                continue;
            }
            let c = block_cell(b, ground);
            let x0 = c[0] as f32 * 32.0;
            let z0 = c[2] as f32 * 32.0;
            let floor = c[1] as f32 * 8.0 + ground;
            // samples over this cell column, within the block's height band
            let mut pts: Vec<(f32, f32, f32)> = Vec::new();
            for s in &d.samples {
                if s.x >= x0 && s.x < x0 + 32.0 && s.z >= z0 && s.z < z0 + 32.0 && s.y >= floor - 2.0 && s.y <= floor + 40.0 {
                    // local frame: undo the block's dir (clockwise quarter turns)
                    let (u, v) = (s.x - x0 - 16.0, s.z - z0 - 16.0);
                    let (lx, lz) = match b.dir & 3 {
                        0 => (u, v),
                        1 => (-v, u),
                        2 => (-u, -v),
                        _ => (v, -u),
                    };
                    pts.push((lx, lz, s.y));
                }
            }
            if pts.len() < 4 {
                continue;
            }
            // the path through the cell: first → last sample; the rise along it, attributed
            // to the local axis the path mostly follows (+1: y rises toward +axis)
            let (f, l) = (pts[0], pts[pts.len() - 1]);
            let (dx, dz, dy) = (l.0 - f.0, l.1 - f.1, l.2 - f.2);
            if dy.abs() < 0.5 || (dx.abs() < 4.0 && dz.abs() < 4.0) {
                continue;
            }
            let (ax, az) = if dz.abs() >= dx.abs() { (0.0, (dy / dz).signum()) } else { ((dy / dx).signum(), 0.0) };
            acc.entry(b.name.clone()).or_default().push((ax, az));
        }
    }
    println!("block\tplacements\trises toward +z\ttoward -z\ttoward +x\ttoward -x");
    for (name, v) in &acc {
        let pz = v.iter().filter(|p| p.1 > 0.0).count();
        let nz = v.iter().filter(|p| p.1 < 0.0).count();
        let px = v.iter().filter(|p| p.0 > 0.0).count();
        let nx = v.iter().filter(|p| p.0 < 0.0).count();
        println!("{name}\t{}\t{pz}\t{nz}\t{px}\t{nx}", v.len());
    }
    Ok(())
}

/// `tmmaps stadium-deckprofile MAP --block SUBSTR`: the validation ghost's height over the cell
/// floor while it crosses blocks whose name contains SUBSTR — the driving surface height of a
/// block family, measured (RoadDirt is bumpy: 0.5..2.8 in the collision mesh).
pub fn deckprofile_cmd(args: &[String]) -> Result<(), String> {
    let want = crate::cli::flag(args, "--block").ok_or("--block SUBSTR")?;
    let mut hs: Vec<f32> = Vec::new();
    for path in args.iter().skip(1).filter(|a| a.ends_with(".Map.Gbx")) {
        let m = MapFile::load(Path::new(path));
        let coll = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
        let ground = crate::map::ground_y(coll);
        let Ok(d) = gbx::record::decode_ghost(path) else { continue };
        let mut cells: HashMap<(i32, i32), Vec<i32>> = HashMap::new();
        for b in &m.blocks {
            if b.free_pos.is_none() && b.name.contains(want) {
                let c = block_cell(b, ground);
                cells.entry((c[0], c[2])).or_default().push(c[1]);
            }
        }
        for s in &d.samples {
            let key = ((s.x / 32.0).floor() as i32, (s.z / 32.0).floor() as i32);
            if let Some(rows) = cells.get(&key) {
                for r in rows {
                    let h = s.y - (*r as f32 * 8.0 + ground);
                    if (-1.0..8.0).contains(&h) {
                        hs.push(h);
                    }
                }
            }
        }
    }
    hs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if hs.is_empty() {
        return Err("no samples over such blocks".into());
    }
    let q = |p: f32| hs[((hs.len() - 1) as f32 * p) as usize];
    println!("{want}: {} samples over the floor — car centre height p05 {:.2} p25 {:.2} p50 {:.2} p75 {:.2} p95 {:.2}", hs.len(), q(0.05), q(0.25), q(0.5), q(0.75), q(0.95));
    Ok(())
}

/// `tmmaps stadium-cporder MAP`: the order the validation ghost crosses the map's waypoint
/// blocks (grid cell match, first sample within the cell column), with the time.
pub fn cporder_cmd(args: &[String]) -> Result<(), String> {
    let path = args.iter().skip(1).find(|a| a.ends_with(".Map.Gbx")).ok_or("MAP")?;
    let m = MapFile::load(Path::new(path));
    let coll = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let ground = crate::map::ground_y(coll);
    let d = gbx::record::decode_ghost(path).map_err(|e| format!("ghost: {e}"))?;
    let mut hits: Vec<(i32, String, [i32; 3])> = Vec::new();
    for b in &m.blocks {
        let Some(tag) = b.waypoint_tag.as_deref() else { continue };
        if !matches!(tag, "Checkpoint" | "LinkedCheckpoint" | "Goal" | "Multilap" | "StartFinish" | "Spawn") {
            continue;
        }
        let c = block_cell(b, ground);
        let first = d.samples.iter().find(|s| ((s.x / 32.0).floor() as i32 == c[0] || (s.x / 32.0).floor() as i32 == c[0] + 1 || (s.x / 32.0).floor() as i32 == c[0] - 1) && ((s.z / 32.0).floor() as i32 == c[2] || (s.z / 32.0).floor() as i32 == c[2] + 1 || (s.z / 32.0).floor() as i32 == c[2] - 1) && (s.y - (c[1] as f32 * 8.0 + ground)).abs() < 12.0);
        hits.push((first.map(|s| s.time_ms).unwrap_or(i32::MAX), format!("{} [{tag}]", b.name), c));
    }
    for it in &m.items {
        let Some(tag) = it.waypoint_tag.as_deref() else { continue };
        if !matches!(tag, "Checkpoint" | "LinkedCheckpoint" | "Goal" | "Multilap" | "StartFinish" | "Spawn") {
            continue;
        }
        let first = d.samples.iter().find(|s| (s.x - it.pos[0]).abs() < 24.0 && (s.z - it.pos[2]).abs() < 24.0 && (s.y - it.pos[1]).abs() < 12.0);
        hits.push((first.map(|s| s.time_ms).unwrap_or(i32::MAX), format!("{} [{tag}] (item)", it.model), [it.pos[0] as i32, it.pos[1] as i32, it.pos[2] as i32]));
    }
    hits.sort();
    for (t, n, c) in hits {
        if t == i32::MAX {
            println!("  never   {n} at {c:?}");
        } else {
            println!("  {:6.2} s  {n} at {c:?}", t as f32 / 1000.0);
        }
    }
    Ok(())
}
