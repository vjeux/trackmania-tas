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
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
    Twin(String, i32),
    Drop(&'static str),
}

/// The measured twins: variant \t plain twin \t rows up (`mapgeom blockinfo` unit
/// heights over the four vista packs and Stadium.pak, 2026-09-22; the two variants
/// no pack lists took their digit).
fn twin_table() -> &'static BTreeMap<String, (String, i32)> {
    static T: std::sync::OnceLock<BTreeMap<String, (String, i32)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        include_str!("stadium-on-twins.tsv")
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                (f.len() >= 3).then(|| (f[0].to_string(), (f[1].to_string(), f[2].trim().parse().unwrap_or(0))))
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
    if let Some((t, dy)) = twin_table().get(name) {
        if known.contains(&t.to_ascii_lowercase()) {
            return Fate::Twin(t.clone(), *dy);
        }
    }
    if let Some(t) = strip_on(name) {
        if known.contains(&t.to_ascii_lowercase()) {
            // an unmeasured variant: a flat piece rises by its digit, a slope stays
            let digit: i32 = name.rsplit(|c: char| !c.is_ascii_digit()).next().and_then(|d| d.parse().ok()).unwrap_or(if name.contains("Slope") { 0 } else { 1 });
            return Fate::Twin(t, if name.contains("Slope") { 0 } else { digit });
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
    let collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
    if collection == STADIUM {
        return Err("already a Stadium map".into());
    }
    let ground = crate::map::ground_y(collection);
    let mut fates: Vec<Fate> = m.blocks.iter().map(|b| fate(&b.name)).collect();
    let item_keep: Vec<bool> = m.items.iter().map(|it| item_kept(&it.model)).collect();
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
    let dx = (SIZE[0] - (hi[0] - lo[0] + 1)) / 2 - lo[0];
    let dz = (SIZE[2] - (hi[2] - lo[2] + 1)) / 2 - lo[2];
    let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
    let mut twins: BTreeMap<String, usize> = BTreeMap::new();
    for (b, f) in m.blocks.iter().zip(&fates) {
        match f {
            Fate::Drop(_) => *dropped.entry(b.name.clone()).or_default() += 1,
            Fate::Twin(t, dy) => *twins.entry(format!("{} → {t}{}", b.name, if *dy != 0 { format!(" (+{dy} rows)") } else { String::new() })).or_default() += 1,
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
            if let Fate::Twin(t, _) = fte {
                m.set_block_name(i, t);
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
        let mut moved = 0usize;
        let mut risen = 0usize;
        for i in 0..m.blocks.len() {
            let b = m.blocks[i].clone();
            let up = match &kept_fates[i] {
                Fate::Twin(_, rows) => *rows,
                _ => 0,
            };
            if up != 0 {
                risen += 1;
            }
            match b.free_pos {
                Some(pos) => m.move_block_free(i, [pos[0] + world[0], pos[1] + world[1] + up as f32 * 8.0, pos[2] + world[2]]),
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
        // the stadium's ground is 64 higher in the cell frame than the vista's: world y
        // of the SAME cell row differs by (ground_stadium − ground_vista)
        let ground_delta = crate::map::ground_y(STADIUM) - p.ground;
        crate::tiny::set_ground(STADIUM);
        for i in 0..m.items.len() {
            let it = m.items[i].clone();
            let pos = [it.pos[0] + world[0], it.pos[1] + world[1] + ground_delta, it.pos[2] + world[2]];
            m.move_item(i, pos, it.yaw, crate::tiny::cell_for(pos));
        }
        // free blocks: the same world-frame correction
        for i in 0..m.blocks.len() {
            if let Some(pos) = m.blocks[i].free_pos {
                if ground_delta != 0.0 {
                    m.move_block_free(i, [pos[0], pos[1] + ground_delta, pos[2]]);
                }
            }
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
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
        let uid_len = hdr.as_ref().map(|h| h.uid.len()).unwrap_or(27);
        let new_name = f("--name").unwrap_or_else(|| format!("{old_name} [Stadium]"));
        m.set_map_uid(&crate::altered::new_uid("Stad", uid_len));
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
