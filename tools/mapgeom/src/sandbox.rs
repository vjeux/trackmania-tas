//! `mapgeom sandbox` — Hugo's "Sandbox" alteration (2026-10-07): keep only the
//! WAYPOINT and EFFECT pieces, each as its Tech PLATFORM twin on the lowest
//! placeable row, fill every other cell of that row with `PlatformTechBase`.
//!
//! Classification is read off the packs, never off names:
//! * a block is a WAYPOINT when its block info's waypoint type is not None;
//! * a block is an EFFECT when its block info's material-modifier slot 1 is a
//!   `Modifier\<Kind>.TerrainModifier.Gbx` of a gameplay kind (Turbo, Turbo2,
//!   Boost, Boost2, SlowMotion, NoEngine, NoSteering, NoBrake, Cruise, Fragile,
//!   Reset, FreeWheel, …) — the surface modifiers (Grass, Dirt, Ice, Snow, …)
//!   are not effects;
//! * an item is a WAYPOINT when its model carries the waypoint chunk, an EFFECT
//!   when its model references a `Media\Modifier\<Kind>` gate folder of a
//!   gameplay kind.
//! Everything else is OTHER and goes (with every generated record).

use crate::sttc::{cell_str, pos_str, wp_str, Ctx, Row};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use tmmaps::map::MapFile;

/// The gameplay effect kinds (the `Modifier\<Kind>` folders that are physics,
/// not surfaces).
pub const EFFECT_KINDS: &[&str] = &["Turbo", "Turbo2", "Boost", "Boost2", "SlowMotion", "NoEngine", "NoSteering", "NoBrake", "Cruise", "CruiseControl", "Fragile", "Reset", "FreeWheel", "TurboRoulette", "NoGrip"];

#[derive(Clone, Debug, PartialEq)]
pub enum Class {
    Waypoint(i32),
    Effect(String),
    Other,
}

impl Class {
    pub fn label(&self) -> String {
        match self {
            Class::Waypoint(t) => format!("WAYPOINT({})", wp_str(Some(*t))),
            Class::Effect(k) => format!("EFFECT({k})"),
            Class::Other => "OTHER".into(),
        }
    }
}

/// The effect kind of a modifier path (`Stadium\Media\Modifier\Turbo.TerrainModifier.Gbx`
/// → `Turbo`; a gate folder `Stadium\Media\Modifier\Turbo\…` → `Turbo`).
fn effect_kind_of(path: &str) -> Option<String> {
    let p = path.replace('/', "\\");
    let rest = p.split("\\Modifier\\").nth(1)?;
    let kind = rest.split(['\\', '.']).next()?.to_string();
    EFFECT_KINDS.iter().find(|k| k.eq_ignore_ascii_case(&kind)).map(|k| k.to_string())
}

pub fn block_class(ctx: &mut Ctx, name: &str) -> Result<Class, String> {
    let Some(path) = ctx.idx.resolve_one(ctx.store, name) else {
        return Err(format!("{name}: not in the packs"));
    };
    let bi = ctx.idx.load(ctx.store, &path).map_err(|e| format!("{name}: {e}"))?;
    if let Some(t) = bi.waypoint_type {
        if t != crate::sttc::WP_NONE {
            return Ok(Class::Waypoint(t));
        }
    }
    for slot in bi.material_modifier_slots.iter().flatten().chain(bi.material_modifier.iter()) {
        if let Some(k) = effect_kind_of(slot) {
            return Ok(Class::Effect(k));
        }
    }
    Ok(Class::Other)
}

pub fn item_class(ctx: &mut Ctx, model: &str) -> Result<Class, String> {
    let (wp, resolved) = ctx.item_wp(model);
    if !resolved {
        return Err(format!("{model}: not in any pack"));
    }
    if let Some(t) = wp {
        if t != crate::sttc::WP_NONE {
            return Ok(Class::Waypoint(t));
        }
    }
    // the effect gates: the item file's references name the modifier folder
    let refs = ctx.item_refs(model);
    for r in &refs {
        if let Some(k) = effect_kind_of(r) {
            return Ok(Class::Effect(k));
        }
    }
    Ok(Class::Other)
}

/// The Tech platform twin of a kept block: `PlatformTech<Waypoint>` /
/// `PlatformTechSpecial<Kind>`, verified in the packs with the same class.
pub fn platform_twin(ctx: &mut Ctx, c: &Class) -> Result<String, String> {
    let cand = match c {
        Class::Waypoint(t) => match *t {
            crate::sttc::WP_START => "PlatformTechStart".to_string(),
            crate::sttc::WP_FINISH => "PlatformTechFinish".to_string(),
            crate::sttc::WP_CHECKPOINT => "PlatformTechCheckpoint".to_string(),
            crate::sttc::WP_STARTFINISH => "PlatformTechMultilap".to_string(),
            other => return Err(format!("waypoint type {other}: no platform twin known")),
        },
        Class::Effect(k) => format!("PlatformTechSpecial{k}"),
        Class::Other => return Err("OTHER has no twin".into()),
    };
    let got = block_class(ctx, &cand)?;
    if &got != c {
        return Err(format!("{cand}: classifies as {} in the pack, wanted {}", got.label(), c.label()));
    }
    Ok(cand)
}

/// An item model's height in metres (the top of its visual/collision geometry over its
/// origin plane), from the pack prefab the item references; None when nothing could be read.
pub fn item_height(ctx: &mut Ctx, model: &str) -> Option<f32> {
    let refs = ctx.item_refs(model);
    let prefab = refs.iter().find(|r| r.to_lowercase().ends_with(".prefab.gbx"))?.clone();
    // the item file's references are relative to its pack root (`Media\Prefab\…`): try the collections
    let mut loaded = None;
    let cands: Vec<String> = if prefab.contains('\\') && !prefab.starts_with("Media\\") {
        vec![prefab.clone()]
    } else {
        ["Stadium", "BlueBay", "RedIsland", "WhiteShore", "GreenCoast"].iter().map(|c| format!("{c}\\{prefab}")).collect()
    };
    for c in cands {
        if let Ok(m) = ctx.store.load_model(&c) {
            loaded = Some(m);
            break;
        }
    }
    let loaded = loaded?;
    let mut c = crate::geom::Collector::new(ctx.store);
    c.model(&loaded, &crate::geom::IDENTITY, 0);
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for g in c.scene.groups.values() {
        for v in &g.verts {
            lo = lo.min(v[1]);
            hi = hi.max(v[1]);
        }
    }
    if hi.is_finite() && lo.is_finite() {
        Some(hi.max(0.0))
    } else {
        None
    }
}

/// A ring-shaped BLOCK's geometry: (is_ring, centre of the ring in the block's own frame
/// [x, y, z] metres from the cell origin corner, measurement note). Hugo round 4: ring
/// pieces stay rings — the Stadium `GateCheckpoint` / `GateFinish` blocks are 32 m rings
/// standing in a 1×4×1 column (their prefab's silhouette in x/y: 72 % of the vertices in
/// a thin annulus, all 12 sectors populated); `GateSpecial*` / `GateExpandable*` are frames.
pub fn block_ring(ctx: &mut Ctx, name: &str) -> Option<(bool, [f32; 3], String)> {
    let path = ctx.idx.resolve_one(ctx.store, name)?;
    let bi = ctx.idx.load(ctx.store, &path).ok()?.clone();
    let v = bi.variant_base_air.as_ref().or(bi.variant_base_ground.as_ref())?;
    let prefab = v.mobils.first().and_then(|m| m.first()).and_then(|m| m.prefab.clone())?;
    let loaded = ctx.store.load_model(&prefab).ok()?;
    let mut c = crate::geom::Collector::new(ctx.store);
    c.model(&loaded, &crate::geom::IDENTITY, 0);
    let mut pts: Vec<[f32; 3]> = Vec::new();
    for g in c.scene.groups.values() {
        pts.extend(g.verts.iter().copied());
    }
    if pts.len() < 12 {
        return None;
    }
    let (mut xlo, mut xhi, mut ylo, mut yhi, mut zlo, mut zhi) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    for p in &pts {
        xlo = xlo.min(p[0]); xhi = xhi.max(p[0]);
        ylo = ylo.min(p[1]); yhi = yhi.max(p[1]);
        zlo = zlo.min(p[2]); zhi = zhi.max(p[2]);
    }
    let (w, h) = (xhi - xlo, yhi - ylo);
    let (cx, cy, cz) = ((xlo + xhi) / 2.0, (ylo + yhi) / 2.0, (zlo + zhi) / 2.0);
    let r = w.max(h) / 2.0;
    let mut sectors = [0usize; 12];
    let mut annulus = 0usize;
    for p in &pts {
        let (dx, dy) = (p[0] - cx, p[1] - cy);
        let d = (dx * dx + dy * dy).sqrt();
        if (d - r).abs() <= 0.15 * r {
            annulus += 1;
            let a = dy.atan2(dx).rem_euclid(2.0 * std::f32::consts::PI);
            sectors[((a / (std::f32::consts::PI / 6.0)) as usize).min(11)] += 1;
        }
    }
    let frac = annulus as f32 / pts.len() as f32;
    let min_sector = annulus / 100;
    let all_round = annulus > 0 && sectors.iter().all(|n| *n > min_sector);
    let is_ring = (w - h).abs() <= 0.15 * w.max(h) && frac > 0.6 && all_round;
    Some((is_ring, [cx, cy, cz], format!("w {w:.1} h {h:.1} annulus {:.0} % sectors {}/12", frac * 100.0, sectors.iter().filter(|n| **n > min_sector).count())))
}

/// An item model's geometry centre in its own frame (the x/z centre of its visual/collision
/// footprint; y = its mid-height) — the point that must stay put when a pose is reset.
pub fn item_centre(ctx: &mut Ctx, model: &str) -> Option<[f32; 3]> {
    let refs = ctx.item_refs(model);
    let prefab = refs.iter().find(|r| r.to_lowercase().ends_with(".prefab.gbx"))?.clone();
    let cands: Vec<String> = if prefab.contains('\\') && !prefab.starts_with("Media\\") { vec![prefab.clone()] } else { ["Stadium", "BlueBay", "RedIsland", "WhiteShore", "GreenCoast"].iter().map(|c| format!("{c}\\{prefab}")).collect() };
    let mut loaded = None;
    for c in cands {
        if let Ok(m) = ctx.store.load_model(&c) {
            loaded = Some(m);
            break;
        }
    }
    let loaded = loaded?;
    let mut c = crate::geom::Collector::new(ctx.store);
    c.model(&loaded, &crate::geom::IDENTITY, 0);
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let mut n = 0;
    for g in c.scene.groups.values() {
        for v in &g.verts {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
            n += 1;
        }
    }
    if n < 3 {
        return None;
    }
    Some([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0])
}

/// Whether an item model is a RING — a circular gate (Hugo 02:21 PT: only rings sink
/// halfway into the deck; arches, pole gates and frames stand on it). Read off the
/// prefab's visual geometry in the item's x/y plane: a ring is as tall as it is wide
/// (within 15 %) and its vertices sit in a thin annulus around the centre (0, h/2)
/// — more than 60 % within ±15 % of the radius. The Fall 2026 gate items are 8–32 m
/// wide and 9–11 m tall rectangular arches/frames: none is a ring.
pub fn item_is_ring(ctx: &mut Ctx, model: &str) -> Option<(bool, String)> {
    let refs = ctx.item_refs(model);
    let prefab = refs.iter().find(|r| r.to_lowercase().ends_with(".prefab.gbx"))?.clone();
    let cands: Vec<String> = if prefab.contains('\\') && !prefab.starts_with("Media\\") { vec![prefab.clone()] } else { ["Stadium", "BlueBay", "RedIsland", "WhiteShore", "GreenCoast"].iter().map(|c| format!("{c}\\{prefab}")).collect() };
    let mut loaded = None;
    for c in cands {
        if let Ok(m) = ctx.store.load_model(&c) {
            loaded = Some(m);
            break;
        }
    }
    let loaded = loaded?;
    let mut c = crate::geom::Collector::new(ctx.store);
    c.model(&loaded, &crate::geom::IDENTITY, 0);
    let mut pts: Vec<[f32; 3]> = Vec::new();
    for g in c.scene.groups.values() {
        pts.extend(g.verts.iter().copied());
    }
    if pts.len() < 12 {
        return None;
    }
    let (mut xlo, mut xhi, mut ylo, mut yhi) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    for p in &pts {
        xlo = xlo.min(p[0]);
        xhi = xhi.max(p[0]);
        ylo = ylo.min(p[1]);
        yhi = yhi.max(p[1]);
    }
    let w = xhi - xlo;
    let h = yhi - ylo;
    let cx = (xlo + xhi) / 2.0;
    let cy = (ylo + yhi) / 2.0;
    let r = h.max(w) / 2.0;
    // the annulus share, and whether every 30° sector of it is populated (a rectangular frame
    // whose vertices happen to lie near the circle still leaves its lower-side sectors empty:
    // GateSpecial8m is 8 × 9 m with 60 % of its vertices in the annulus and nothing at y 1..3)
    let mut sectors = [0usize; 12];
    let mut annulus = 0usize;
    for p in &pts {
        let (dx, dy) = (p[0] - cx, p[1] - cy);
        let d = (dx * dx + dy * dy).sqrt();
        if (d - r).abs() <= 0.15 * r {
            annulus += 1;
            let a = dy.atan2(dx).rem_euclid(2.0 * std::f32::consts::PI);
            sectors[((a / (std::f32::consts::PI / 6.0)) as usize).min(11)] += 1;
        }
    }
    let frac = annulus as f32 / pts.len() as f32;
    let square = (w - h).abs() <= 0.15 * w.max(h);
    let min_sector = annulus / 100; // 1 % of the annulus vertices per sector
    let all_round = annulus > 0 && sectors.iter().all(|n| *n > min_sector);
    let is_ring = square && frac > 0.6 && all_round;
    Some((is_ring, format!("w {w:.1} h {h:.1} annulus {:.0} % sectors {}/12", frac * 100.0, sectors.iter().filter(|n| **n > min_sector).count())))
}

/// Whether the collection's base row is WATER (the most common genealogy zone names water):
/// then blocks are built one row up, on top of the water (Hugo: "build on top of the water").
pub fn water_base(m: &MapFile) -> Option<String> {
    // the four vista collections' ambient terrain IS water (BlueBay Sea, RedIsland / WhiteShore
    // Water, GreenCoast Lake): every map of theirs is an island in it, whatever its own zone
    // histogram says (16 has more Dirt than Water cells) — a per-collection fact, not per map
    let coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
    if !matches!(coll, 0x10 | 0x1c | 0x1d | 0xf) {
        return None;
    }
    let chunks = tmmaps::gbx::all_skip_chunks(&m.gbx.body);
    let &(_, _, payload, size) = chunks.iter().find(|(c, ..)| *c == 0x0304_3043)?;
    let recs = tmmaps::map::genealogy_full(&m.gbx.body[payload..payload + size]).ok()?;
    let mut hist: BTreeMap<String, usize> = BTreeMap::new();
    for r in &recs {
        *hist.entry(r.current.clone()).or_default() += 1;
    }
    // the water zone the map carries (the most common of Sea / Water / Lake)
    let water = ["Sea", "Water", "Lake"].iter().filter_map(|z| hist.get(*z).map(|n| (*n, z.to_string()))).max().map(|(_, z)| z);
    Some(water.unwrap_or_else(|| match coll { 0x1c => "Sea".into(), 0xf => "Lake".into(), _ => "Water".into() }))
}

pub struct InvRow {
    pub kind: &'static str,
    pub index: usize,
    pub model: String,
    pub class: Class,
    pub twin: Option<String>,
    pub cell: Option<(i32, i32, i32)>,
    pub pos: [f32; 3],
    pub dir: u8,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub free: bool,
    pub tag: Option<String>,
}

/// The inventory of one map: every block (authored) and item, classified.
pub fn inventory(ctx: &mut Ctx, m: &MapFile) -> Result<Vec<InvRow>, String> {
    let mut out = Vec::new();
    let mut bcache: HashMap<String, Result<Class, String>> = HashMap::new();
    for b in m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF) {
        let c = bcache.entry(b.name.clone()).or_insert_with(|| block_class(ctx, &b.name)).clone();
        let class = match c {
            Ok(c) => c,
            Err(_) => Class::Other, // an unresolvable block info is not a waypoint/effect we can keep
        };
        let twin = if class == Class::Other { None } else { platform_twin(ctx, &class).ok() };
        let coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
        let ground = tmmaps::map::ground_y(coll);
        let pos = match b.free_pos {
            Some(p) => p,
            None => {
                let c = b.coords();
                [c.0 as f32 * 32.0 + 16.0, c.1 as f32 * 8.0 + ground, c.2 as f32 * 32.0 + 16.0]
            }
        };
        let rot = b.free_rot.unwrap_or([0.0; 3]);
        out.push(InvRow { kind: "block", index: b.index, model: b.name.clone(), class, twin, cell: Some(b.coords()), pos, dir: b.dir, yaw: rot[0], pitch: rot[1], roll: rot[2], free: b.free_pos.is_some(), tag: b.waypoint_tag.clone() });
    }
    let mut icache: HashMap<String, Result<Class, String>> = HashMap::new();
    for it in &m.items {
        let c = icache.entry(it.model.clone()).or_insert_with(|| item_class(ctx, &it.model)).clone();
        let class = c.unwrap_or(Class::Other);
        out.push(InvRow { kind: "item", index: it.index, model: it.model.clone(), class, twin: None, cell: None, pos: it.pos, dir: 0, yaw: it.yaw, pitch: it.pitch, roll: it.roll, free: true, tag: it.waypoint_tag.clone() });
    }
    Ok(out)
}

pub fn inventory_tsv(map_label: &str, rows: &[InvRow]) -> String {
    let mut s = String::from("map\tkind\tindex\tmodel\tclass\ttwin\tcell\tpos\tdir\tyaw\tfree\ttag\n");
    for r in rows {
        s.push_str(&format!(
            "{map_label}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{}\t{}\n",
            r.kind,
            r.index,
            r.model,
            r.class.label(),
            r.twin.clone().unwrap_or_else(|| if r.class == Class::Other { "-".into() } else if r.kind == "item" { "(kept as item)".into() } else { "NO TWIN".into() }),
            r.cell.map(cell_str).unwrap_or_else(|| "-".into()),
            pos_str(r.pos),
            r.dir,
            r.yaw,
            r.free,
            r.tag.clone().unwrap_or_default()
        ));
    }
    s
}

/// Per-model summary over many maps: model → (class, twin, count, maps).
pub fn summarize(rows: &[(String, Vec<InvRow>)]) -> BTreeMap<(String, String), (String, String, usize, Vec<String>)> {
    let mut m: BTreeMap<(String, String), (String, String, usize, Vec<String>)> = BTreeMap::new();
    for (map, rs) in rows {
        for r in rs.iter().filter(|r| r.class != Class::Other) {
            let e = m.entry((r.kind.to_string(), r.model.clone())).or_insert_with(|| (r.class.label(), r.twin.clone().unwrap_or_else(|| "NO TWIN".into()), 0, Vec::new()));
            e.2 += 1;
            if !e.3.contains(map) {
                e.3.push(map.clone());
            }
        }
    }
    m
}

// ------------------------------------------------------------------ build

#[derive(Clone, Debug)]
pub struct SandboxOpts {
    pub fill: String,
    /// the fill records' flags (0x1000 = the ground variant)
    pub fill_flags: u32,
    pub clear_genealogy: bool,
    /// the row for everything; None = the collection's base row, +1 when that row is water
    /// (the deck then stands on top of the water: Hugo, 2026-10-07 02:04 PT)
    pub row: Option<i32>,
    /// sink the kept items half into the deck (origin at deck − height/2) and reset pitch/roll
    pub sink_items: bool,
    /// ring-shaped BLOCKS keep their model (a FREE block, ring centre at the deck top) instead of a platform twin
    pub keep_ring_blocks: bool,
    pub uid_prefix: String,
    pub name_suffix: String,
    pub unlock: bool,
    pub unvalidated: bool,
    pub strip_lightmap: bool,
}

pub struct SandboxOutcome {
    pub rows: Vec<Row>,
    pub kept_blocks: usize,
    pub rings: usize,
    pub kept_items: usize,
    pub removed_blocks: usize,
    pub removed_items: usize,
    pub removed_baked: usize,
    pub fill: usize,
    pub row: i32,
    pub new_name: String,
    pub new_uid: String,
    pub inventory: String,
}

/// The lowest row a ground block of the collection sits on: the Stadium ground
/// row is 9 (world y 8 = `ground_y(26)` + 9·8 − 72 … the file cell `cy` whose
/// world y equals the collection's ground level), read as the cy of the source's
/// lowest ground-flagged authored block when there is one, else the collection's
/// nominal ground row.
pub fn ground_row(m: &MapFile) -> i32 {
    // the lowest row the source's own GROUND-variant grid blocks sit on — the collection's
    // terrain base (Stadium 9 = world y 8; RedIsland 14; the Fall 2026 terrain collections 4 and 5)
    let lowest_ground = m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && b.flags & 0x1000 != 0 && b.free_pos.is_none()).map(|b| b.coords().1).min();
    match lowest_ground {
        Some(r) if r >= 0 => r,
        _ => 9,
    }
}

pub fn sandbox(ctx: &mut Ctx, src: &Path, out: &Path, o: &SandboxOpts, dry: bool) -> Result<SandboxOutcome, String> {
    let map_label = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let m = MapFile::try_load(src)?;
    let hdr = tmmaps::header::read(src.to_str().unwrap_or_default())?;
    let inv = inventory(ctx, &m)?;
    let base_row = ground_row(&m);
    let water = water_base(&m);
    let row = o.row.unwrap_or(if water.is_some() { base_row + 1 } else { base_row });
    // the deck's variant: GROUND (0x1000) when it sits on the collection's ground row, AIR (0) when it
    // floats over the water — the AIR variant's unit carries the bottom clip (PlatFormBaseFCB) the engine
    // derives at load, which draws the underside (Hugo 02:21 PT: "from below the faces are missing";
    // source 11's floating PlatformTechBase records are flags 0 with a PlatformBaseFCB under each)
    let fill_flags = if row > base_row { o.fill_flags & !0x1000 } else { o.fill_flags | 0x1000 };
    let size = m.size;
    let coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
    let ground = tmmaps::map::ground_y(coll);
    // PlatformTechBase's deck (its Base_Air prefab's asphalt plane) is at local y 2.0 over the cell floor
    let platform_top = ground + row as f32 * 8.0 + 2.0;
    let mut rows: Vec<Row> = Vec::new();
    // ---- plan
    let mut keep_blocks: Vec<(usize, String, (i32, i32, i32), u8, String, bool)> = Vec::new(); // (index, twin, new cell, dir, class, was free)
    // ring blocks kept as FREE blocks: (index, pos, rot[yaw, pitch, roll], class)
    let mut ring_blocks: Vec<(usize, [f32; 3], [f32; 3], String)> = Vec::new();
    let mut ring_cache: HashMap<String, Option<(bool, [f32; 3], String)>> = HashMap::new();
    let mut occupied: HashSet<(i32, i32)> = HashSet::new();
    let mut no_twin: Vec<String> = Vec::new();
    for r in inv.iter().filter(|r| r.kind == "block") {
        let mut row_r = Row::new(&map_label, "sandbox", "block", r.index, &r.model);
        row_r.tag = r.tag.clone().unwrap_or_default();
        row_r.from = if r.free { format!("free {}", pos_str(r.pos)) } else { cell_str(r.cell.unwrap()) };
        row_r.dir_from = r.dir.to_string();
        match (&r.class, &r.twin) {
            (Class::Other, _) => {
                row_r.action = "removed".into();
                row_r.note = "OTHER".into();
            }
            (c, Some(twin)) => {
                // the cell: a free block lands on the cell under its position
                let (cx, cz) = if r.free { ((r.pos[0] / 32.0).floor() as i32, (r.pos[2] / 32.0).floor() as i32) } else { (r.cell.unwrap().0, r.cell.unwrap().2) };
                let ring = if o.keep_ring_blocks { ring_cache.entry(r.model.clone()).or_insert_with(|| block_ring(ctx, &r.model)).clone() } else { None };
                if let Some((true, centre, note)) = ring {
                    // a RING stays a ring: the same block, FREE-placed, upright, heading kept, its ring
                    // centre at the deck top (half of it inside the platform). A grid ring's heading is
                    // its dir (quarter turns clockwise: yaw = −dir·π/2); a free ring keeps its yaw.
                    // The block's origin corner = ring centre − the centre offset turned by the yaw.
                    let yaw = if r.free { r.yaw } else { -(r.dir as f32) * std::f32::consts::FRAC_PI_2 };
                    let (sy, cyw) = yaw.sin_cos();
                    // local (cx, cz) turned by yaw about y: x' = cx·cos + cz·sin, z' = −cx·sin + cz·cos
                    let ox = centre[0] * cyw + centre[2] * sy;
                    let oz = -centre[0] * sy + centre[2] * cyw;
                    // the ring centre in the world: the source piece's own centre (x/z), the deck top (y)
                    let src_centre = if r.free {
                        let m = crate::place::free(r.pos, [r.yaw, r.pitch, r.roll]);
                        crate::geom::apply(&m, centre)
                    } else {
                        let c0 = r.cell.unwrap();
                        let (sy0, cy0) = (-(r.dir as f32) * std::f32::consts::FRAC_PI_2).sin_cos();
                        // dir0 origin (x0,z0); dir1 (x0+32,z0); dir2 (x0+32,z0+32); dir3 (x0,z0+32) — the engine's pose rule
                        let (x0, z0) = (c0.0 as f32 * 32.0, c0.2 as f32 * 32.0);
                        let (ox0, oz0) = match r.dir & 3 { 0 => (x0, z0), 1 => (x0 + 32.0, z0), 2 => (x0 + 32.0, z0 + 32.0), _ => (x0, z0 + 32.0) };
                        [ox0 + centre[0] * cy0 + centre[2] * sy0, 0.0, oz0 - centre[0] * sy0 + centre[2] * cy0]
                    };
                    let pos = [src_centre[0] - ox, platform_top - centre[1], src_centre[2] - oz];
                    row_r.action = "ring-kept".into();
                    row_r.to_name = r.model.clone();
                    row_r.to = format!("free {} yaw {:.4} pitch 0 roll 0", pos_str(pos), yaw);
                    row_r.dir_to = "-".into();
                    row_r.note = format!("{}; RING ({note}): kept as a FREE {} with its centre ({:.1},{:.1},{:.1} in the block frame) at the deck top {platform_top:.3} — half inside the platform{}", c.label(), r.model, centre[0], centre[1], centre[2], if r.pitch.abs() > 1e-4 || r.roll.abs() > 1e-4 { format!("; POSE RESET upright (was pitch {:.4} roll {:.4})", r.pitch, r.roll) } else { String::new() });
                    ring_blocks.push((r.index, pos, [yaw, 0.0, 0.0], c.label()));
                    rows.push(row_r);
                    continue;
                }
                if occupied.contains(&(cx, cz)) {
                    row_r.action = "removed".into();
                    row_r.note = format!("{}: cell ({cx},{cz}) already holds a kept piece — dropped (stacked waypoints/effects on one cell)", c.label());
                } else {
                    occupied.insert((cx, cz));
                    // a free block's quarter turn from its yaw (dir1 = −π/2, dir2 = π, dir3 = +π/2: the engine's own pose rule)
                    let dir = if r.free { ((-(r.yaw) / std::f32::consts::FRAC_PI_2).round() as i32).rem_euclid(4) as u8 } else { r.dir };
                    row_r.action = "to-platform".into();
                    row_r.to_name = twin.clone();
                    row_r.to = cell_str((cx, row, cz));
                    row_r.dir_to = dir.to_string();
                    row_r.note = format!("{}{}", c.label(), if r.free { format!("; was FREE-placed (yaw {:.4}): the cell under its position, dir {dir}", r.yaw) } else { String::new() });
                    keep_blocks.push((r.index, twin.clone(), (cx, row, cz), dir, c.label(), r.free));
                }
            }
            (c, None) => {
                row_r.action = "removed".into();
                row_r.note = format!("{}: NO PLATFORM TWIN", c.label());
                if !no_twin.contains(&r.model) {
                    no_twin.push(r.model.clone());
                }
            }
        }
        rows.push(row_r);
    }
    // (index, new pos, new rotation [yaw, pitch, roll])
    let mut keep_items: Vec<(usize, [f32; 3], [f32; 3])> = Vec::new();
    let mut heights: HashMap<String, Option<f32>> = HashMap::new();
    let mut rings: HashMap<String, Option<(bool, String)>> = HashMap::new();
    let mut centres: HashMap<String, Option<[f32; 3]>> = HashMap::new();
    for r in inv.iter().filter(|r| r.kind == "item") {
        let mut row_r = Row::new(&map_label, "sandbox", "item", r.index, &r.model);
        row_r.tag = r.tag.clone().unwrap_or_default();
        row_r.from = format!("{} yaw {:.4} pitch {:.4} roll {:.4}", pos_str(r.pos), r.yaw, r.pitch, r.roll);
        match &r.class {
            Class::Other => {
                row_r.action = "removed".into();
                row_r.note = "OTHER".into();
            }
            c => {
                // a START gate spawns the car at its ORIGIN plane (the SSpawn entity at local (0,0,−10.6)):
                // sunk, it would spawn the car inside the platform — a start item stays on the deck
                let is_start = matches!(c, Class::Waypoint(t) if *t == crate::sttc::WP_START || *t == crate::sttc::WP_STARTFINISH);
                // only a RING sinks (Hugo 02:21 PT); arches / pole gates / frames stand on the deck
                let ring = if o.sink_items && !is_start { rings.entry(r.model.clone()).or_insert_with(|| item_is_ring(ctx, &r.model)).clone() } else { None };
                let is_ring = ring.as_ref().map(|(b, _)| *b).unwrap_or(false);
                let h = if is_ring { *heights.entry(r.model.clone()).or_insert_with(|| item_height(ctx, &r.model)) } else { None };
                let (y, how) = match (is_ring, h) {
                    (true, Some(h)) => (platform_top - h / 2.0, format!("RING ({}): half-sunk, model height {h:.2} m, origin at deck − {:.2}", ring.as_ref().map(|(_, d)| d.as_str()).unwrap_or(""), h / 2.0)),
                    (true, None) => (platform_top, "RING but height unreadable: on the deck".to_string()),
                    (false, _) if is_start => (platform_top, "on the deck (a start gate spawns the car at its origin plane)".to_string()),
                    (false, _) => (platform_top, format!("on the deck (not a ring{})", ring.as_ref().map(|(_, d)| format!(": {d}")).unwrap_or_default())),
                };
                // The pose reset pivots about the item's GEOMETRY CENTRE, not its placement origin
                // (Hugo round 4: 19's Boost2 gate, placed upside down with pivot (−8, −8, 0), moved
                // 16 m when its pitch was reset about the origin). The model's footprint centre in
                // the source pose (full placement rule: position = pivot, rotation, pivot offset)
                // is where the upright gate's centre goes (x/z); the pivot field is zeroed and the
                // position becomes the upright model's origin = centre − turned local centre.
                let it_rec = m.items.iter().find(|it| it.index == r.index).unwrap();
                let centre_local = if o.sink_items { *centres.entry(r.model.clone()).or_insert_with(|| item_centre(ctx, &r.model)) } else { None };
                let (np, nrot, moved_note) = if o.sink_items {
                    let nrot = [r.yaw, 0.0f32, 0.0f32];
                    match centre_local {
                        Some(cl) => {
                            let src_m = crate::place::anchored(r.pos, [r.yaw, r.pitch, r.roll], it_rec.pivot, it_rec.scale);
                            let world_c = crate::geom::apply(&src_m, cl);
                            // the upright model's origin so that its centre lands on (world_c.x, ·, world_c.z)
                            let (sy, cy) = r.yaw.sin_cos();
                            let ox = cl[0] * cy + cl[2] * sy;
                            let oz = -cl[0] * sy + cl[2] * cy;
                            let np = [world_c[0] - ox, y, world_c[2] - oz];
                            let d = ((np[0] - r.pos[0]).powi(2) + (np[2] - r.pos[2]).powi(2)).sqrt();
                            (np, nrot, if d > 0.01 || it_rec.pivot != [0.0; 3] { format!("; pivot about the geometry centre ({:.1},{:.1},{:.1} local): origin moved {d:.2} m in x/z, placement pivot {:?} -> 0", cl[0], cl[1], cl[2], it_rec.pivot) } else { String::new() })
                        }
                        None => ([r.pos[0], y, r.pos[2]], nrot, "; geometry centre unreadable: origin kept".to_string()),
                    }
                } else {
                    ([r.pos[0], y, r.pos[2]], [r.yaw, r.pitch, r.roll], String::new())
                };
                let reset = o.sink_items && (r.pitch.abs() > 1e-4 || r.roll.abs() > 1e-4);
                row_r.action = "set-down".into();
                row_r.to = format!("{} yaw {:.4} pitch {:.4} roll {:.4}", pos_str(np), nrot[0], nrot[1], nrot[2]);
                row_r.note = format!("{}; deck {platform_top:.3}; {how}{}{moved_note}", c.label(), if reset { format!("; POSE RESET upright (was pitch {:.4} roll {:.4})", r.pitch, r.roll) } else { String::new() });
                keep_items.push((r.index, np, nrot));
            }
        }
        rows.push(row_r);
    }
    // the fill
    let kept_cells: HashSet<(i32, i32)> = keep_blocks.iter().map(|k| (k.2 .0, k.2 .2)).collect();
    let mut fill: Vec<(i32, i32)> = Vec::new();
    for cx in 0..size[0] as i32 {
        for cz in 0..size[2] as i32 {
            if !kept_cells.contains(&(cx, cz)) {
                fill.push((cx, cz));
            }
        }
    }
    let kept_blocks = keep_blocks.len() + ring_blocks.len();
    let kept_items = keep_items.len();
    let removed_blocks = m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF).count() - kept_blocks;
    let removed_items = m.items.len() - kept_items;
    let removed_baked = m.baked.len();
    let new_name = format!("{}{}", hdr.name, o.name_suffix);
    let old_uid = hdr.uid.clone();
    let new_uid = if old_uid.len() > o.uid_prefix.len() { format!("{}{}", o.uid_prefix, &old_uid[..old_uid.len() - o.uid_prefix.len()]) } else { old_uid.clone() };
    {
        let mut r = Row::new(&map_label, "sandbox", "map", 0, &new_name);
        r.action = "identity".into();
        r.to_name = new_uid.clone();
        r.note = format!("row {row} (base row {base_row}{}; deck y {platform_top}; {} variant); fill {} × {}; {} kept blocks, {} kept items; {} blocks / {} items / {} generated records removed; no-twin models: {}; genealogy: {}", match &water { Some(z) => format!(", a WATER collection ({z}): built one row up, on top of the water"), None => String::new() }, if fill_flags & 0x1000 != 0 { "GROUND" } else { "AIR (bottom clip)" }, fill.len(), o.fill, kept_blocks, kept_items, removed_blocks, removed_items, removed_baked, if no_twin.is_empty() { "none".to_string() } else { no_twin.join(", ") }, if water.is_some() { "filled with the water zone in every cell (the empty-vista form)" } else if o.clear_genealogy { "cleared" } else { "kept (land collection)" });
        rows.push(r);
    }
    let inventory = inventory_tsv(&map_label, &inv);
    if dry {
        return Ok(SandboxOutcome { rows, kept_blocks, rings: ring_blocks.len(), kept_items, removed_blocks, removed_items, removed_baked, fill: fill.len(), row, new_name, new_uid, inventory });
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // ---- write, in passes of compatible edits (tmmaps' rules: splices never share a
    // write with renames; the block/item removers want a fresh load)
    let kept_set: HashSet<usize> = keep_blocks.iter().map(|k| k.0).chain(ring_blocks.iter().map(|k| k.0)).collect();
    // pass 1: the OTHER items go; the lightmap goes (the source's cannot bind to a rebuilt body)
    {
        let mut m1 = MapFile::try_load(src)?;
        let keep_items_set: HashSet<usize> = keep_items.iter().map(|k| k.0).collect();
        m1.remove_items(|it| !keep_items_set.contains(&it.index));
        m1.write_to(out).map_err(|e| e.to_string())?;
    }
    if o.strip_lightmap {
        let mut m1b = MapFile::try_load(out)?;
        m1b.strip_lightmap();
        m1b.strip_validation_ghost_to(tmmaps::map::GhostForm::Remove);
        if o.unlock {
            m1b.remove_password();
        }
        m1b.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 2: every OTHER block and every generated record go; the fill is appended as GRID records;
    // a kept FREE block becomes a GRID record on the cell under it in the same rewrite
    {
        let mut m2 = MapFile::try_load(out)?;
        let fill_specs: Vec<tmmaps::map::FreeBlockSpec> = fill
            .iter()
            .map(|(cx, cz)| tmmaps::map::FreeBlockSpec { name: o.fill.clone(), author: None, flags: fill_flags, pos: [0.0; 3], rot: [0.0; 3], grid: Some([*cx, row, *cz]), dir: 0 })
            .collect();
        let to_grid: Vec<(usize, [i32; 3])> = keep_blocks.iter().filter(|k| k.5).map(|k| (k.0, [k.2 .0, k.2 .1, k.2 .2])).collect();
        m2.remove_and_add_blocks_ext2(|b| !kept_set.contains(&b.index), |_| true, &fill_specs, &[], &to_grid);
        m2.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 3: the kept blocks become their twins (renames) at the sandbox row (cell/dir patches);
    // the kept items drop onto the deck (patches); the map name (splice) waits for pass 4.
    // The kept records come first in the rebuilt list, in their source order (twins and rings mixed).
    let mut order: Vec<(usize, bool)> = keep_blocks.iter().map(|k| (k.0, false)).chain(ring_blocks.iter().map(|k| (k.0, true))).collect();
    order.sort_by_key(|k| k.0);
    {
        let mut m3 = MapFile::try_load(out)?;
        for (new_index, (src_index, is_ring)) in order.iter().enumerate() {
            let b = m3.blocks[new_index].clone();
            if b.flags == 0xFFFF_FFFF {
                return Err(format!("{map_label}: rebuilt block#{new_index} is a dead record where kept block#{src_index} was expected"));
            }
            if *is_ring {
                continue; // pass 3b
            }
            let k = keep_blocks.iter().find(|k| k.0 == *src_index).unwrap();
            m3.set_block_name(new_index, &k.1);
            m3.move_block_cell(new_index, k.2);
            if b.dir != k.3 {
                m3.set_block_dir(new_index, k.3);
            }
            // the ground-variant bit: the fill's convention (the twin sits on the same row)
            let want = (b.flags & !0x1000) | (fill_flags & 0x1000);
            if want != b.flags {
                m3.set_block_flags(new_index, want);
            }
        }
        for (new_index, (_, np, nrot)) in keep_items.iter().enumerate() {
            m3.move_item_pos(new_index, *np);
            m3.set_item_rotation(new_index, nrot[0], nrot[1], nrot[2]);
            if o.sink_items {
                m3.set_item_pivot(new_index, [0.0, 0.0, 0.0]);
            }
            // the record's cell bytes follow the new position (x/z unchanged, the row = the cell the origin is in)
            let it = m3.items[new_index].clone();
            let cy = ((np[1] - ground) / 8.0).floor().clamp(0.0, 255.0) as u8;
            m3.set_item_cell(new_index, [it.file_cell[0], cy, it.file_cell[2]]);
        }
        m3.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 3b: the ring blocks → FREE-placed at their new pose (grid records converted in DESCENDING
    // index order: each conversion splices a 24-byte entry into 0x0304305F at the block's rank; a
    // source-FREE ring is already free: its entry is rewritten in place)
    if !ring_blocks.is_empty() {
        let mut m3b = MapFile::try_load(out)?;
        let mut todo: Vec<(usize, &(usize, [f32; 3], [f32; 3], String))> = Vec::new();
        for (new_index, (src_index, is_ring)) in order.iter().enumerate() {
            if *is_ring {
                todo.push((new_index, ring_blocks.iter().find(|k| k.0 == *src_index).unwrap()));
            }
        }
        todo.sort_by_key(|(i, _)| std::cmp::Reverse(*i));
        for (new_index, k) in todo {
            let b = m3b.blocks[new_index].clone();
            if b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0 {
                m3b.move_block_free(new_index, k.1);
                m3b.set_block_free_rot(new_index, k.2);
            } else {
                m3b.convert_block_to_free(new_index, k.1, k.2);
            }
            // a free block carries no ground bit
            let want = b.flags & !0x1000;
            if want != b.flags && b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0 {
                m3b.set_block_flags(new_index, want);
            }
        }
        m3b.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 4 (splices): the name, the unvalidated times
    {
        let mut m4 = MapFile::try_load(out)?;
        let (h, b) = m4.set_map_name(&hdr.name, &new_name);
        if h + b == 0 {
            return Err(format!("{map_label}: the map does not declare the name {:?}", hdr.name));
        }
        if o.unvalidated {
            m4.set_unvalidated(&hdr);
        }
        m4.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 5: the uid
    if new_uid != old_uid {
        let mut m5 = MapFile::try_load(out)?;
        m5.set_map_uid_any_len(&new_uid);
        m5.write_to(out).map_err(|e| e.to_string())?;
    }
    // the genealogy: a water collection becomes the empty vista — the water zone in every cell
    // (the game regenerates a water tile per cell at the base row, under the deck one row up);
    // a land collection (Stadium) keeps its records (every cell holds a block: nothing regenerates)
    if water.is_some() && !o.clear_genealogy {
        let (zone, n) = MapFile::fill_genealogy_file(out)?;
        let z = zone.to_lowercase();
        if !z.contains("water") && !z.contains("sea") && !z.contains("lake") {
            return Err(format!("{map_label}: the genealogy fill template is {zone} ({n} cells), not a water zone"));
        }
    } else if o.clear_genealogy {
        let _ = MapFile::clear_genealogy_file(out)?;
    }
    Ok(SandboxOutcome { rows, kept_blocks, rings: ring_blocks.len(), kept_items, removed_blocks, removed_items, removed_baked, fill: fill.len(), row, new_name, new_uid, inventory })
}

/// Round-trip checks.
pub fn verify_sandbox(out: &Path, oc: &SandboxOutcome, o: &SandboxOpts) -> Result<Vec<String>, String> {
    let b = MapFile::try_load(out)?;
    let mut bad = Vec::new();
    let n_auth = b.blocks.iter().filter(|x| x.flags != 0xFFFF_FFFF).count();
    if n_auth != oc.kept_blocks + oc.fill {
        bad.push(format!("authored blocks {} != kept {} + fill {}", n_auth, oc.kept_blocks, oc.fill));
    }
    if !b.baked.is_empty() {
        bad.push(format!("{} generated records left", b.baked.len()));
    }
    if b.items.len() != oc.kept_items {
        bad.push(format!("items {} != kept {}", b.items.len(), oc.kept_items));
    }
    let grid = |x: &&tmmaps::map::BlockRec| x.flags != 0xFFFF_FFFF && x.free_pos.is_none();
    let rows: HashSet<i32> = b.blocks.iter().filter(grid).map(|x| x.coords().1).collect();
    if rows.len() != 1 || !rows.contains(&oc.row) {
        bad.push(format!("rows in use {:?}, want only {}", rows, oc.row));
    }
    let mut cells: HashSet<(i32, i32)> = HashSet::new();
    for x in b.blocks.iter().filter(grid) {
        let c = x.coords();
        if !cells.insert((c.0, c.2)) {
            bad.push(format!("cell ({},{}) used twice", c.0, c.2));
            break;
        }
    }
    if cells.len() != (b.size[0] * b.size[2]) as usize {
        bad.push(format!("{} cells covered of {}", cells.len(), b.size[0] * b.size[2]));
    }
    let free_n = b.blocks.iter().filter(|x| x.flags != 0xFFFF_FFFF && x.free_pos.is_some()).count();
    if free_n != oc.rings {
        bad.push(format!("{free_n} free blocks, want {} (the kept rings)", oc.rings));
    }
    let spawns = b.blocks.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Spawn")).count() + b.items.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Spawn")).count();
    if spawns != 1 {
        bad.push(format!("{spawns} Spawn placements, want 1"));
    }
    let hb = tmmaps::header::read(out.to_str().unwrap_or_default())?;
    if hb.validated != "0" {
        bad.push(format!("validated={}", hb.validated));
    }
    if hb.name != oc.new_name || hb.uid != oc.new_uid {
        bad.push(format!("header name/uid {:?}/{} != {:?}/{}", hb.name, hb.uid, oc.new_name, oc.new_uid));
    }
    if o.unvalidated && hb.authortime != "-1" {
        bad.push(format!("authortime {} != -1", hb.authortime));
    }
    if o.unlock && crate::sttc::has_chunk(&b.gbx.body, 0x0304_3029) {
        bad.push("password chunk still present".into());
    }
    Ok(bad)
}

pub struct PipelineOpts {
    pub sandbox: SandboxOpts,
    pub dry: bool,
    pub out_by_map_name: bool,
}

pub fn pipeline(store: &mut crate::store::DataStore, src: &Path, out_dir: &Path, o: &PipelineOpts) -> Result<(Vec<Row>, String, SandboxOutcome), String> {
    let stem = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let stem = stem.trim_end_matches(".Map.Gbx").to_string();
    let hdr = tmmaps::header::read(src.to_str().unwrap_or_default())?;
    let final_out: PathBuf = if o.out_by_map_name {
        out_dir.join(format!("{}.Map.Gbx", format!("{}{}", hdr.name, o.sandbox.name_suffix).replace(['/', '\\', ':'], "-")))
    } else {
        out_dir.join(format!("{stem}-Sandbox.Map.Gbx"))
    };
    let m = MapFile::try_load(src)?;
    let mut ctx = Ctx::new(store, &m);
    let oc = sandbox(&mut ctx, src, &final_out, &o.sandbox, o.dry)?;
    let mut rows = oc.rows.clone();
    let bad = if o.dry { Vec::new() } else { verify_sandbox(&final_out, &oc, &o.sandbox)? };
    let mut r = Row::new(&format!("{stem}.Map.Gbx"), "summary", "map", 0, &format!("{}", m.size[0]));
    r.action = format!("kept {} blocks ({} -> platform twins, {} rings kept as free rings) + {} items set down; removed {} blocks / {} items / {} generated; fill {} × {} at row {}", oc.kept_blocks, oc.kept_blocks - oc.rings, oc.rings, oc.kept_items, oc.removed_blocks, oc.removed_items, oc.removed_baked, oc.fill, o.sandbox.fill, oc.row);
    r.to_name = oc.new_name.clone();
    if !bad.is_empty() {
        r.note = format!("VERIFY FAILED: {}", bad.join("; "));
        rows.push(r.clone());
        return Err(format!("{stem}: {}", r.note));
    }
    r.note = if o.dry { "dry run".into() } else { "verified".into() };
    rows.push(r.clone());
    if !o.dry {
        std::fs::write(out_dir.join(format!("{stem}.inventory.tsv")), &oc.inventory).map_err(|e| e.to_string())?;
    }
    let summary = r.tsv();
    Ok((rows, summary, oc))
}
