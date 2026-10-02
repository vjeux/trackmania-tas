//! `mapgeom watercells SRC GIANT` — the WATER CENSUS of a giant (×N) build:
//! every water cell of the SOURCE (every authored block whose block info
//! declares a water volume: the pools, the shallow grass/dirt/ice ponds and
//! streams, the water roads, the water walls, the transitions) against what
//! the GIANT carries in the same place — the engine's water volumes of its
//! native tiles and free custom water blocks — and the rims the engine draws
//! around those tiles (`crate::bake`, the client's own free-clip algorithm)
//! where the source has none (vjeux 2026-10-01 20:51 PT: "Shallow pools are
//! not water … The full pools have a border and are not always there").
//!
//! The yardstick is the SOURCE: a source water cell must be water in the
//! giant (a volume covering the scaled cell), and a rim is right exactly
//! where the source draws one (its own bake), wrong where the source's water
//! continues.
//!
//! ```text
//! mapgeom watercells SRC.Map.Gbx GIANT.Map.Gbx --anchor sx,sy,sz:tx,ty,tz [--scale 2]
//!                    [--collection Stadium] [--out TSV] [--rims TSV]
//! ```
//!
//! Verdicts per source block: `native` (≥ 95 % of its scaled volume is
//! engine water), `partial`, `missing`; the reason names the converter rule
//! behind it (`giantwater::POOL_BLOCKS`, roads off, the grid, a free source
//! block). The rims table lists every drawn giant clip on a water tile's side
//! face that stands INSIDE the source's (scaled) water — a wall or lip across
//! the pool — and the boundary rims the source itself does not draw.

use std::collections::{BTreeMap, HashMap, HashSet};

use tmmaps::map::{BlockRec, MapFile, CELL_XZ, CELL_Y};

use crate::blockmap::{BlockInfoIndex, FLAG_ADDITIONAL_SHIFT, FLAG_GROUND, FLAG_SUBVARIANT_SHIFT, FLAG_VARIANT_MASK};
use crate::store::DataStore;

/// A world-space axis-aligned box, metres, [x0, y0, z0, x1, y1, z1].
pub type WBox = [f32; 6];

/// The water kind of a block from its volume height within the cell (the
/// Stadium pack's three forms): `full` = the whole 8 m row (DecoWallWater*),
/// `shallow` = 3 m at 4..7 (Water*, WaterGrass/Dirt/Ice*, WaterWall*),
/// `road` = 2 m at the bottom (RoadWater*, PlatformWater*, TrackWallWater* at
/// 0..8 count as full), anything else `other` (slopes, diagonals).
pub fn kind_of(name: &str, boxes_local: &[([f32; 6], ())]) -> &'static str {
    let mut ys: Vec<(f32, f32)> = boxes_local.iter().map(|(b, _)| (b[1], b[4])).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup();
    if name.starts_with("RoadWater") || name.starts_with("PlatformWater") && !name.contains("ToDecoWallWater") {
        return "road";
    }
    if ys.len() == 1 {
        let (y0, y1) = ys[0];
        let (a, b) = (y0.rem_euclid(CELL_Y), (y1 - y0));
        if (a - 0.0).abs() < 1e-3 && (b - 8.0).abs() < 1e-3 {
            return "full";
        }
        if (a - 4.0).abs() < 1e-3 && (b - 3.0).abs() < 1e-3 {
            return "shallow";
        }
        if (a - 0.0).abs() < 1e-3 && (b - 2.0).abs() < 1e-3 {
            return "road";
        }
    }
    "other"
}

/// Rotate a local point of a block frame (footprint w×d cells) by `dir`
/// quarter turns, the `blockmap::footprint` convention.
fn rot_local(lx: f32, lz: f32, w: i32, d: i32, dir: u8) -> (f32, f32) {
    let (wm, dm) = (w as f32 * CELL_XZ, d as f32 * CELL_XZ);
    match dir & 3 {
        0 => (lx, lz),
        1 => (dm - lz, lx),
        2 => (wm - lx, dm - lz),
        _ => (lz, wm - lx),
    }
}

fn rot_box(b: [f32; 6], w: i32, d: i32, dir: u8) -> [f32; 6] {
    let (ax, az) = rot_local(b[0], b[2], w, d, dir);
    let (bx, bz) = rot_local(b[3], b[5], w, d, dir);
    [ax.min(bx), b[1], az.min(bz), ax.max(bx), b[4], az.max(bz)]
}

/// One source or giant water-carrying block with its world boxes.
#[derive(Clone, Debug)]
pub struct WaterBlock {
    pub index: usize,
    pub name: String,
    pub cell: (i32, i32, i32),
    pub dir: u8,
    pub flags: u32,
    pub free: bool,
    pub variant: String,
    /// local boxes (block frame, metres) and the world boxes
    pub local: Vec<[f32; 6]>,
    pub world: Vec<WBox>,
    pub kind: &'static str,
    pub footprint: (i32, i32),
}

/// The water blocks of a map: every authored block whose picked variant
/// declares volumes. Free custom water blocks (`Water\X.Block.Gbx_CustomBlock`)
/// take the archetype X's volumes at the free position.
pub fn water_blocks(store: &mut DataStore, idx: &mut BlockInfoIndex, m: &MapFile, ground: f32) -> (Vec<WaterBlock>, Vec<String>) {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    let mut unresolved: BTreeMap<String, usize> = BTreeMap::new();
    for b in &m.blocks {
        let (arch, free) = if b.free_pos.is_some() {
            // a custom block: the archetype is the file stem under Blocks\Water\
            let n = b.name.trim_end_matches("_CustomBlock");
            let stem = n.rsplit(['\\', '/']).next().unwrap_or(n).trim_end_matches(".Block.Gbx").to_string();
            (stem, true)
        } else {
            (b.name.clone(), false)
        };
        let Some(path) = idx.resolve_one(store, &arch) else {
            if free || b.name.contains("Water") {
                *unresolved.entry(arch.clone()).or_insert(0) += 1;
            }
            continue;
        };
        let Ok(bi) = idx.load(store, &path) else { continue };
        let ground_bit = b.flags & FLAG_GROUND != 0;
        let variant = (b.flags & FLAG_VARIANT_MASK) as usize;
        let sub = ((b.flags >> FLAG_SUBVARIANT_SHIFT) & 63) as usize;
        let additional = ((b.flags >> FLAG_ADDITIONAL_SHIFT) & 0x7f) as usize;
        let Some(p) = bi.pick_placement_add(ground_bit, variant, sub, additional) else { continue };
        if p.variant.water_volume_list.is_empty() {
            continue;
        }
        // the footprint extents (cells) of the variant for the rotation pivot
        let (mut minx, mut maxx, mut minz, mut maxz) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for u in &p.variant.block_units {
            minx = minx.min(u.offset[0]);
            maxx = maxx.max(u.offset[0]);
            minz = minz.min(u.offset[2]);
            maxz = maxz.max(u.offset[2]);
        }
        let (w, d) = if p.variant.block_units.is_empty() { (1, 1) } else { (maxx - minx + 1, maxz - minz + 1) };
        let mut local: Vec<[f32; 6]> = Vec::new();
        for wv in &p.variant.water_volume_list {
            let (ux, uy, uz) = (f32::from_bits(wv.words[4]), f32::from_bits(wv.words[5]), f32::from_bits(wv.words[6]));
            let (ux, uy, uz) = (if ux > 0.0 { ux } else { CELL_XZ }, if uy > 0.0 { uy } else { CELL_Y }, if uz > 0.0 { uz } else { CELL_XZ });
            for bx in &wv.boxes {
                local.push([bx[0] as f32 * ux, bx[1] as f32 * uy, bx[2] as f32 * uz, (bx[3] + 1) as f32 * ux, (bx[4] + 1) as f32 * uy, (bx[5] + 1) as f32 * uz]);
            }
        }
        let kind = kind_of(&arch, &local.iter().map(|b| (*b, ())).collect::<Vec<_>>());
        let world: Vec<WBox> = if let (Some(pos), Some(rot)) = (b.free_pos, b.free_rot) {
            // a free block: yaw in quarter turns (dir 1 = -pi/2, 2 = pi, 3 = +pi/2),
            // the position is the turned frame's origin corner
            let q = (-rot[0] / std::f32::consts::FRAC_PI_2).round() as i32;
            let dir = q.rem_euclid(4) as u8;
            let (s, c) = rot[0].sin_cos();
            local
                .iter()
                .map(|lb| {
                    let corners = [(lb[0], lb[2]), (lb[3], lb[5])];
                    let pts: Vec<(f32, f32)> = corners.iter().map(|(lx, lz)| (pos[0] + lx * c + lz * s, pos[2] - lx * s + lz * c)).collect();
                    let (x0, x1) = (pts[0].0.min(pts[1].0), pts[0].0.max(pts[1].0));
                    let (z0, z1) = (pts[0].1.min(pts[1].1), pts[0].1.max(pts[1].1));
                    let _ = dir;
                    [x0, pos[1] + lb[1], z0, x1, pos[1] + lb[4], z1]
                })
                .collect()
        } else {
            let (cx, cy, cz) = b.coords();
            local
                .iter()
                .map(|lb| {
                    let r = rot_box(*lb, w, d, b.dir);
                    [cx as f32 * CELL_XZ + r[0], cy as f32 * CELL_Y + ground + r[1], cz as f32 * CELL_XZ + r[2], cx as f32 * CELL_XZ + r[3], cy as f32 * CELL_Y + ground + r[4], cz as f32 * CELL_XZ + r[5]]
                })
                .collect()
        };
        out.push(WaterBlock { index: b.index, name: b.name.clone(), cell: b.coords(), dir: b.dir, flags: b.flags, free, variant: p.label.clone(), local, world, kind, footprint: (w, d) });
    }
    for (n, k) in unresolved {
        notes.push(format!("{k} block(s) named {n}: no block info (volumes unknown)"));
    }
    (out, notes)
}

fn transform(p: [f32; 3], s: [f32; 3], t: [f32; 3], scale: f32) -> [f32; 3] {
    [t[0] + (p[0] - s[0]) * scale, t[1] + (p[1] - s[1]) * scale, t[2] + (p[2] - s[2]) * scale]
}

pub fn scale_box(b: &WBox, s: [f32; 3], t: [f32; 3], scale: f32) -> WBox {
    let a = transform([b[0], b[1], b[2]], s, t, scale);
    let c = transform([b[3], b[4], b[5]], s, t, scale);
    [a[0].min(c[0]), a[1].min(c[1]), a[2].min(c[2]), a[0].max(c[0]), a[1].max(c[1]), a[2].max(c[2])]
}

/// The 1 m voxels (by their min corner) a box covers; boxes are metre-aligned
/// in both maps (×2 of integer metres).
fn voxels(b: &WBox) -> impl Iterator<Item = (i32, i32, i32)> {
    let (x0, y0, z0) = (b[0].round() as i32, b[1].round() as i32, b[2].round() as i32);
    let (x1, y1, z1) = (b[3].round() as i32, b[4].round() as i32, b[5].round() as i32);
    (x0..x1).flat_map(move |x| (y0..y1).flat_map(move |y| (z0..z1).map(move |z| (x, y, z))))
}

pub struct Row {
    pub src: WaterBlock,
    pub total: usize,
    pub covered: usize,
    pub giant_names: BTreeMap<String, usize>,
    pub verdict: &'static str,
    pub reason: String,
    /// drawn giant clips standing inside this block's scaled water (face records)
    pub inner_rims: Vec<String>,
}

pub struct Census {
    pub rows: Vec<Row>,
    pub notes: Vec<String>,
    pub giant_blocks: Vec<WaterBlock>,
    /// giant tiles whose water lies in no source water (extra water)
    pub extra_giant: Vec<String>,
    /// boundary rims the giant draws that the source does not (face records)
    pub boundary_extra: Vec<String>,
    pub boundary_ok: usize,
    pub inner_total: usize,
}

/// Why a source water block is not (fully) in the giant, from the converter's rules.
fn reason_for(src: &WaterBlock, frac: f32, giant_names: &BTreeMap<String, usize>, grid: [i32; 3], s: [f32; 3], t: [f32; 3], scale: f32) -> String {
    if src.free {
        return "FREE source block (giantwater tiles grid blocks only)".into();
    }
    if frac >= 0.95 {
        return format!("tiled natively ({})", giant_names.iter().map(|(n, k)| format!("{n}×{k}")).collect::<Vec<_>>().join(", "));
    }
    let n = scale.round() as i32;
    if crate::giantwater::POOL_BLOCKS.contains(&src.name.as_str()) {
        // in the mapping: the grid or the top-row-only WaterBase form
        let corner = [src.cell.0 as f32 * CELL_XZ, src.cell.1 as f32 * CELL_Y, src.cell.2 as f32 * CELL_XZ];
        let g = transform([corner[0], corner[1] + tmmaps::map::ground_y(0x1a), corner[2]], s, t, scale);
        let (cx, cz) = ((g[0] / CELL_XZ).round() as i32, (g[2] / CELL_XZ).round() as i32);
        if cx < 0 || cz < 0 || cx + n > grid[0] || cz + n > grid[2] {
            return format!("pool tiles outside the {}x{}x{} grid (clipped)", grid[0], grid[1], grid[2]);
        }
        if src.name == "WaterBase" {
            return format!("WaterBase tiled in the TOP row only: volume 4..7 of one 8 m row = {:.0} % of the ×{n} depth", frac * 100.0);
        }
        return format!("in POOL_BLOCKS but {:.0} % covered (unexpected)", frac * 100.0);
    }
    if crate::giantwater::road_family(&src.name) {
        return "water road: roads off (TINY_GIANT_ROAD_TILES=0; the volume tiles grow the archetype's end walls)".into();
    }
    format!("no native mapping: {} is not in giantwater::POOL_BLOCKS (only WaterBase, DecoWallWaterBase are tiled)", src.name)
}

#[allow(clippy::too_many_arguments)]
pub fn census(store: &mut DataStore, idx: &mut BlockInfoIndex, src: &MapFile, giant: &MapFile, s: [f32; 3], t: [f32; 3], scale: f32, ground: f32) -> Census {
    let (src_blocks, mut notes) = water_blocks(store, idx, src, ground);
    let (giant_blocks, gnotes) = water_blocks(store, idx, giant, ground);
    notes.extend(gnotes.into_iter().map(|n| format!("giant: {n}")));
    // the giant's water voxels, with the tile names behind each
    let mut gv: HashMap<(i32, i32, i32), u32> = HashMap::new();
    let mut gname_of: Vec<String> = Vec::new();
    let mut gname_idx: HashMap<String, u32> = HashMap::new();
    for gb in &giant_blocks {
        let k = *gname_idx.entry(gb.name.clone()).or_insert_with(|| {
            gname_of.push(gb.name.clone());
            (gname_of.len() - 1) as u32
        });
        for b in &gb.world {
            for v in voxels(b) {
                gv.insert(v, k);
            }
        }
    }
    // the source's scaled water voxels → which source block (first wins)
    let mut sv: HashMap<(i32, i32, i32), usize> = HashMap::new();
    let mut rows: Vec<Row> = Vec::new();
    for (i, sb) in src_blocks.iter().enumerate() {
        let mut total = 0usize;
        let mut covered = 0usize;
        let mut names: BTreeMap<String, usize> = BTreeMap::new();
        for b in &sb.world {
            let sbx = scale_box(b, s, t, scale);
            for v in voxels(&sbx) {
                total += 1;
                sv.entry(v).or_insert(i);
                if let Some(&k) = gv.get(&v) {
                    covered += 1;
                    *names.entry(gname_of[k as usize].clone()).or_insert(0) += 1;
                }
            }
        }
        let frac = if total > 0 { covered as f32 / total as f32 } else { 0.0 };
        let verdict = if total == 0 {
            "no-volume"
        } else if frac >= 0.95 {
            "native"
        } else if frac > 0.0 {
            "partial"
        } else {
            "missing"
        };
        let reason = reason_for(sb, frac, &names, giant.size, s, t, scale);
        rows.push(Row { src: sb.clone(), total, covered, giant_names: names, verdict, reason, inner_rims: Vec::new() });
    }
    // extra giant water: voxels in no source water
    let mut extra: BTreeMap<String, usize> = BTreeMap::new();
    for (v, k) in &gv {
        if !sv.contains_key(v) {
            *extra.entry(gname_of[*k as usize].clone()).or_insert(0) += 1;
        }
    }
    let extra_giant: Vec<String> = extra.into_iter().map(|(n, k)| format!("{n}: {k} m³ of water where the source has none")).collect();

    // RIMS: the engine's free clips of the giant's water tiles vs the source's water
    let faces_g = crate::fillers::faces(store, idx, giant);
    let dirs_g: HashMap<usize, u8> = giant.blocks.iter().map(|b| (b.index, b.dir)).collect();
    let grounds_g = crate::bake::record_grounds(&faces_g, giant);
    let clips_g = crate::bake::simulate(&faces_g, &dirs_g, &grounds_g);
    let faces_s = crate::fillers::faces(store, idx, src);
    let dirs_s: HashMap<usize, u8> = src.blocks.iter().map(|b| (b.index, b.dir)).collect();
    let grounds_s = crate::bake::record_grounds(&faces_s, src);
    let clips_s = crate::bake::simulate(&faces_s, &dirs_s, &grounds_s);
    // the source's drawn side clips by (cell, face)
    let mut src_drawn: HashSet<([i32; 3], usize)> = HashSet::new();
    // `bake` cells are FILE cells (game cell + (1, 0, 1))
    for c in clips_s.iter().filter(|c| c.drawn() && c.face < 4) {
        src_drawn.insert(([c.cell[0] as i32 - 1, c.cell[1] as i32, c.cell[2] as i32 - 1], c.face));
    }
    let water_owner: HashSet<usize> = giant_blocks.iter().map(|g| g.index).collect();
    let n = scale.round() as i32;
    let mut boundary_extra = Vec::new();
    let mut boundary_ok = 0usize;
    let mut inner_total = 0usize;
    // a side clip is a WALL/RIM piece only when its name says so; the water sheet
    // (FCT) and floors (FCB) are top/bottom clips and never counted here
    for c in clips_g.iter().filter(|c| c.drawn() && c.face < 4 && water_owner.contains(&c.owner_index)) {
        let (dx, _, dz) = crate::bake::step(c.face);
        let cell = [c.cell[0] as i32 - 1, c.cell[1] as i32, c.cell[2] as i32 - 1];
        // probe voxels just beyond the face, at every metre of the cell's water height
        let (fx, fz) = match c.face {
            0 => (cell[0] * 32 + 16, cell[2] * 32 + 32),
            1 => (cell[0] * 32, cell[2] * 32 + 16),
            2 => (cell[0] * 32 + 16, cell[2] * 32),
            _ => (cell[0] * 32 + 32, cell[2] * 32 + 16),
        };
        let y0 = (cell[1] as f32 * CELL_Y + ground) as i32;
        let beyond = (fx + dx, fz + dz);
        let inside = (fx - dx, fz - dz);
        let mut src_beyond: Option<usize> = None;
        let mut water_inside = false;
        for y in y0..y0 + 8 {
            if let Some(&i) = sv.get(&(beyond.0, y, beyond.1)) {
                src_beyond = Some(i);
            }
            if sv.contains_key(&(inside.0, y, inside.1)) {
                water_inside = true;
            }
        }
        let rec = format!("({},{},{}) face {} {} owner {}#{}", cell[0], cell[1], cell[2], c.face, c.name, c.owner, c.owner_index);
        if let Some(i) = src_beyond {
            if water_inside {
                // the source's water continues across this face: a rim inside the pool
                inner_total += 1;
                rows[i].inner_rims.push(rec);
                continue;
            }
        }
        // a boundary face: does the source draw a clip on the corresponding face?
        let sx = (((cell[0] * 32 + 16) as f32 - t[0]) / scale + s[0]).div_euclid(CELL_XZ) as i32;
        let sy = (((cell[1] as f32 * CELL_Y + ground + 4.0) - t[1]) / scale + s[1] - ground).div_euclid(CELL_Y) as i32;
        let sz = (((cell[2] * 32 + 16) as f32 - t[2]) / scale + s[2]).div_euclid(CELL_XZ) as i32;
        if src_drawn.contains(&([sx, sy, sz], c.face)) {
            boundary_ok += 1;
        } else {
            boundary_extra.push(format!("{rec} (source cell ({sx},{sy},{sz}) draws no clip on that face)"));
        }
    }
    let _ = n;
    Census { rows, notes, giant_blocks, extra_giant, boundary_extra, boundary_ok, inner_total }
}

pub fn parse_anchor(a: &str) -> Result<([f32; 3], [f32; 3]), String> {
    crate::giantwater::parse_anchor(a)
}

/// The anchor from a build log line `anchor: source [x, y, z] -> target [x, y, z]; scale N`.
pub fn anchor_from_log(text: &str) -> Option<([f32; 3], [f32; 3], f32)> {
    for l in text.lines() {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("anchor: source [") {
            let (sv, rest) = rest.split_once("] -> target [")?;
            let (tv, rest) = rest.split_once("]; scale ")?;
            let p = |s: &str| -> Option<[f32; 3]> {
                let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().ok()).collect::<Option<_>>()?;
                (v.len() == 3).then(|| [v[0], v[1], v[2]])
            };
            return Some((p(sv)?, p(tv)?, rest.trim().parse().ok()?));
        }
    }
    None
}

pub fn cmd(store: &mut DataStore, rest: &[String]) {
    fn die<T>(m: String) -> T {
        eprintln!("{m}");
        std::process::exit(2)
    }
    let pos: Vec<&String> = rest.iter().filter(|a| !a.starts_with("--")).take_while(|_| true).collect();
    let flag = |k: &str| rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1)).cloned();
    let (src_p, giant_p) = match (rest.first(), rest.get(1)) {
        (Some(a), Some(b)) if !a.starts_with("--") && !b.starts_with("--") => (a.clone(), b.clone()),
        _ => die("usage: mapgeom watercells SRC.Map.Gbx GIANT.Map.Gbx --anchor sx,sy,sz:tx,ty,tz|from-tiles [--scale 2] [--collection Stadium] [--out TSV] [--rims TSV]".into()),
    };
    let _ = pos;
    let scale: f32 = flag("--scale").unwrap_or_else(|| "2".into()).parse().unwrap_or_else(|_| die("--scale N".into()));
    let src = MapFile::load(std::path::Path::new(&src_p));
    let giant = MapFile::load(std::path::Path::new(&giant_p));
    let collection_raw = src.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let ground = tmmaps::map::ground_y(collection_raw);
    let coll = flag("--collection").unwrap_or_else(|| "Stadium".to_string());
    let anchor = flag("--anchor").unwrap_or_else(|| "from-tiles".into());
    let (s, t) = if anchor == "from-tiles" {
        let n = scale.round() as i32;
        let mut found = None;
        for name in ["DecoWallWaterBase", "WaterBase"] {
            let sb: Vec<(i32, i32, i32)> = src.blocks.iter().filter(|b| b.free_pos.is_none() && b.name == name).map(|b| b.coords()).collect();
            let gb: Vec<(i32, i32, i32)> = giant.blocks.iter().filter(|b| b.free_pos.is_none() && b.name == name).map(|b| b.coords()).collect();
            if sb.is_empty() || gb.is_empty() {
                continue;
            }
            let smin = (sb.iter().map(|c| c.0).min().unwrap(), sb.iter().map(|c| c.1).min().unwrap(), sb.iter().map(|c| c.2).min().unwrap());
            let gmin = (gb.iter().map(|c| c.0).min().unwrap(), gb.iter().map(|c| c.1).min().unwrap(), gb.iter().map(|c| c.2).min().unwrap());
            let gy = if name == "WaterBase" { gmin.1 - (n - 1) } else { gmin.1 };
            found = Some(([smin.0 as f32 * CELL_XZ, smin.1 as f32 * CELL_Y + ground, smin.2 as f32 * CELL_XZ], [gmin.0 as f32 * CELL_XZ, gy as f32 * CELL_Y + ground, gmin.2 as f32 * CELL_XZ]));
            break;
        }
        found.unwrap_or_else(|| die("--anchor from-tiles: the giant has no pool tiles; pass --anchor sx,sy,sz:tx,ty,tz (the build log's `anchor: source … -> target …` line)".into()))
    } else if let Some(log) = anchor.strip_prefix("from-log:") {
        let text = std::fs::read_to_string(log).unwrap_or_else(|e| die(format!("{log}: {e}")));
        let (s, t, _) = anchor_from_log(&text).unwrap_or_else(|| die(format!("{log}: no `anchor: source … -> target …` line")));
        (s, t)
    } else {
        parse_anchor(&anchor).unwrap_or_else(die)
    };
    let mut idx = BlockInfoIndex::build(store, &coll);
    let c = census(store, &mut idx, &src, &giant, s, t, scale, ground);
    println!("# watercells: {} -> {} (anchor {:?} -> {:?}, scale {scale}); giant grid {:?}; {} source water blocks, {} giant water tiles", src_p, giant_p, s, t, giant.size, c.rows.len(), c.giant_blocks.len());
    for n in &c.notes {
        println!("# note: {n}");
    }
    let mut tsv = String::from("index\tname\tkind\tcell\tdir\tflags\tvariant\tscaled_m3\tcovered_m3\tcovered_pct\tverdict\tgiant_blocks\tinner_rims\treason\n");
    let mut by: BTreeMap<(String, &str, &str), (usize, usize, usize)> = BTreeMap::new();
    for r in &c.rows {
        let pct = if r.total > 0 { 100.0 * r.covered as f32 / r.total as f32 } else { 0.0 };
        tsv.push_str(&format!(
            "{}\t{}\t{}\t({},{},{})\t{}\t{:08x}\t{}\t{}\t{}\t{:.0}\t{}\t{}\t{}\t{}\n",
            r.src.index,
            r.src.name,
            r.src.kind,
            r.src.cell.0,
            r.src.cell.1,
            r.src.cell.2,
            r.src.dir,
            r.src.flags,
            r.src.variant,
            r.total,
            r.covered,
            pct,
            r.verdict,
            r.giant_names.iter().map(|(n, k)| format!("{n}×{k}")).collect::<Vec<_>>().join("+"),
            r.inner_rims.len(),
            r.reason
        ));
        let e = by.entry((r.src.name.clone(), r.src.kind, r.verdict)).or_insert((0, 0, 0));
        e.0 += 1;
        e.1 += r.total;
        e.2 += r.inner_rims.len();
    }
    if let Some(o) = flag("--out") {
        std::fs::write(&o, &tsv).unwrap_or_else(|e| die(format!("{o}: {e}")));
        println!("# wrote {o}");
    } else {
        print!("{tsv}");
    }
    println!("# SUMMARY (name kind verdict: blocks, scaled m³, inner rims)");
    let mut tot: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for ((n, k, v), (b, m3, rims)) in &by {
        println!("#   {n}\t{k}\t{v}\t{b} block(s)\t{m3} m³\t{rims} inner rim(s)");
        let e = tot.entry(v).or_insert((0, 0));
        e.0 += b;
        e.1 += m3;
    }
    for (v, (b, m3)) in &tot {
        println!("# TOTAL {v}: {b} block(s), {m3} m³");
    }
    println!("# rims: {} drawn side clips of the giant's water tiles stand INSIDE the source's water (walls/lips across a pool); {} boundary rims match a source clip; {} boundary rims the source does not draw", c.inner_total, c.boundary_ok, c.boundary_extra.len());
    for e in &c.extra_giant {
        println!("# extra: {e}");
    }
    if let Some(o) = flag("--rims") {
        let mut r = String::from("kind\trecord\n");
        for row in &c.rows {
            for x in &row.inner_rims {
                r.push_str(&format!("inner\t{x}\t(source {} #{} at ({},{},{}))\n", row.src.name, row.src.index, row.src.cell.0, row.src.cell.1, row.src.cell.2));
            }
        }
        for x in &c.boundary_extra {
            r.push_str(&format!("boundary-extra\t{x}\n"));
        }
        std::fs::write(&o, &r).unwrap_or_else(|e| die(format!("{o}: {e}")));
        println!("# wrote {o}");
    }
    let _ = BlockRec::coords;
}
