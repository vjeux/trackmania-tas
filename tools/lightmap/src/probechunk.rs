//! The PROBE-CHUNK RECORDS of a bake — the third CPU input of the world peel's focus box (`lmtiles::world_peel_box`
//! unions their AABB into the scene box) and the layout of the probe atlas — transcribed from the client (RE child
//! 6, 2026-09-25):
//!
//! 1. `grid_def` — CGameCtnApp::HmsLightMapCompute 0x140c53c70 → FUN_140c53ab0(out = computeParams+0x10, &map.size
//!    (int3, challenge+0x268), &blockSize (the collection's, challenge+0x7e0: (32, 8, 32)), &offset (0, the
//!    decoration's base height challenge+0x7e8, 0)): n = size, cell = blockSize; flat blocks (bs.y ≤ 0.51·bs.x and
//!    ≤ 0.51·bs.z, n.y even) → cell.y doubled, n.y halved, and when that still fits (≤ 0.51·bs.x/z) the x/z cells
//!    halve and n.x/n.z double → (16, 16, 16) m over (2·sx, sy/2, 2·sz) probes for a 32×8×32 collection; with the
//!    chunk knob (DAT_14205c808) a cell.x > 24 m halves (doubling n) while n.x ≤ 255; origin = cell·0.5 + offset;
//!    then origin.y += frac⁺((h + 2 − origin.y)/cell.y)·cell.y (the fractional part, +1 when negative: the probe
//!    rows are aligned on the collection's level h + 2 — h = FUN_140d124c0(collection id): 0 / 1 / 2 (+8, +8) by a
//!    table of interned collection ids that cannot be read statically; BlueBay's h ≡ 0 (mod 16) per the capture).
//! 2. the PROBE BOXES (RenderLighting_Frames 0x14021e340 l.1048–1147): every block record whose quality scale squared
//!    is > 0.9 contributes its own box; a record below that contributes its model's probe box (the lm-data object's
//!    +0x100 {c, h} through the mobil Iso4, when +0x10c ≥ 0 — not carried by the tiny items: `None` here); the
//!    zone's special static meshes (zone+0x248, FUN_140264e10) and the forest (zone+0x260, FUN_14026e590) add theirs;
//!    without any record: the scene box's x/z with y −32..128. Their min/max corners drive the grid expansion.
//! 3. `expand_grid` (l.1296–1370): the corners in index space (world · 1/cell − origin/cell), ceil'd to ints, clamped
//!    to [−32, n + 32]; a negative low index grows n by −lo and moves the origin by lo·cell; a high index beyond the
//!    ORIGINAL n grows n by the excess.
//! 4. `probe_chunks` — FUN_14021d730 → FUN_14021c980(out, records, slots, {&grid, boxes, count}): the grid cut into
//!    chunks of 30 × 14 × 30 probes (atlas tiles of 32 × 16 × 32 = a one-probe border); per box the index-space
//!    corners − (1.01, 0.01, 1.01) / + (1.01, 1.01, 1.01), ceil'd, clamped to [0, n − 1]; every chunk they touch
//!    tightens its imin/imax to the box, clamped to the chunk's own range; non-empty chunks get atlas slots in
//!    z-major, then y, then x order over columns = max(1, floor(√count)) and rows = ceil(count/columns) (a slot =
//!    column·32, row·16); the record (0x54 bytes): +0 the slot, +0xc = slot + (imin − chunk origin), +0x18 = that +
//!    (imax − imin) + 3, +0x24 the cell, +0x30 the world origin of atlas index 0 = (imin − 1 − (+0xc))·cell + origin,
//!    +0x3c/+0x48 the inverse; no box at all → the centre chunk. The atlas (columns·32, rows·16, 32) must fit:
//!    x ≤ 32 && z ≤ 32 → each ≤ the device's max 3D dimension; else z ≤ 32 → x, y ≤ 256; else x, y, z ≤ 128 — otherwise
//!    the cell doubles (n = (n + 1) >> 1) and the chunking reruns, at most 4 times.
//! 5. `chunks_aabb` — FUN_140233150: the AABB over c(i) = (float(i) − 0.5)·cell + origin' for i = +0xc and +0x18 of
//!    every record, as {c = (max + min)·0.5, h = (max − min)·0.5}.
//!
//! pwc-day (`lmtool probe-chunks MAP`): size (64, 64, 64), BlueBay (32, 8, 32), offset y −38 (the decoration's base
//! height — probes.rs's empirical origin table has the same −38) → 128 × 32 × 128 probes of 16 m at origin (8, −30,
//! 8); the three items (q = 1) touch one chunk, (1, 0, 0) → the record's world origin (472, −46, −8), atlas 32 × 16 ×
//! 32, +0xc = (23, 4, 20), +0x18 = (26, 12, 26) — engineer B's ProbeToShadow-derived grid and RE 5's imax.y = 12
//! exactly; the world peel's y max 138 = (12 − 0.5)·16 − 46.

use crate::lmtiles::{BlockRecord, CBox};

/// The probe grid of a bake: `n` probes per axis, `cell` metres apart, the first probe at `origin` (computeParams+
/// 0x10: u32×3, f32×3, f32×3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridDef {
    pub n: [u32; 3],
    pub cell: [f32; 3],
    pub origin: [f32; 3],
}

/// The fractional part with its integer part dropped toward zero (modff), +1 when negative — the `frac⁺` of
/// FUN_140c53ab0's y alignment.
fn frac_pos(v: f32) -> f32 {
    let f = v - v.trunc();
    if f < 0.0 { f + 1.0 } else { f }
}

/// FUN_140c53ab0 — see the module doc, step 1. `size` = the map's size words, `block_size` = the collection's block
/// size in metres ((32, 8, 32) for the TM2020 collections), `offset` = (0, the decoration's base height, 0), `level_h`
/// = FUN_140d124c0(collection) (0 for BlueBay as the capture has it), `chunk_knob` = DAT_14205c808 (on).
pub fn grid_def(size: [u32; 3], block_size: [f32; 3], offset: [f32; 3], level_h: f32, chunk_knob: bool) -> GridDef {
    let mut n = size;
    let (bx, by, bz) = (block_size[0], block_size[1], block_size[2]);
    let mut cell = [bx, by, bz];
    if by <= bx * 0.51 && by <= bz * 0.51 && (n[1] & 1) == 0 {
        let cy = by + by;
        n[1] >>= 1;
        cell[1] = cy;
        if cy <= bx * 0.51 && cy <= bz * 0.51 {
            n[0] *= 2;
            n[2] *= 2;
            cell[0] = bx * 0.5;
            cell[2] = bz * 0.5;
        }
    }
    if chunk_knob && cell[0] > 24.0 {
        let mut cx = cell[0];
        let mut nx = n[0];
        loop {
            if 0xff < nx {
                break;
            }
            cx *= 0.5;
            cell[0] = cx;
            cell[1] *= 0.5;
            cell[2] *= 0.5;
            n[1] *= 2;
            nx = n[0] * 2;
            n[0] = nx;
            n[2] *= 2;
            if !(24.0 < cx) {
                break;
            }
        }
    }
    let mut origin = [cell[0] * 0.5 + offset[0], cell[1] * 0.5 + offset[1], cell[2] * 0.5 + offset[2]];
    let v = ((level_h + 2.0) - origin[1]) / cell[1];
    origin[1] = frac_pos(v) * cell[1] + origin[1];
    GridDef { n, cell, origin }
}

/// The probe boxes of the block records (RenderLighting_Frames l.1085–1147): a record with quality² > 0.9 gives its
/// own box. Records below that would give their model's probe box — none on the tiny items (the lm-data +0x100 box
/// is invalid); the zone's special meshes and the forest are not represented here.
pub fn probe_boxes(records: &[BlockRecord]) -> Vec<CBox> {
    records.iter().filter(|r| 0.9 < r.quality * r.quality).map(|r| r.world).collect()
}

/// The box used when there is no record at all (l.1073–1083): the scene box's x/z, y from −32 to 128.
pub fn default_probe_box(scene: &CBox) -> CBox {
    CBox::from_min_max([scene.c[0] - scene.h[0], -32.0, scene.c[2] - scene.h[2]], [scene.h[0] + scene.c[0], 128.0, scene.h[2] + scene.c[2]])
}

/// The min/max corners of the boxes as the game accumulates them (c − h / c + h per box, strict min/max).
pub fn corners(boxes: &[CBox]) -> Option<([f32; 3], [f32; 3])> {
    let mut mn = [f32::MAX; 3];
    let mut mx = [f32::MIN; 3];
    for b in boxes {
        for k in 0..3 {
            let lo = b.c[k] - b.h[k];
            let hi = b.c[k] + b.h[k];
            if lo < mn[k] {
                mn[k] = lo;
            }
            if mx[k] < hi {
                mx[k] = hi;
            }
        }
    }
    if boxes.is_empty() { None } else { Some((mn, mx)) }
}

/// world → probe index space: (world − origin)/cell as the inverted Iso4 applies it (scale then offset).
fn to_index(g: &GridDef, w: [f32; 3]) -> [f32; 3] {
    let mut o = [0f32; 3];
    for k in 0..3 {
        let s = 1.0 / g.cell[k];
        o[k] = w[k] * s + (-g.origin[k] * s);
    }
    o
}

/// The CRT int-ceil (FUN_1418f6954).
fn ceil_i(x: f32) -> i32 {
    if !x.is_finite() { 0 } else { x.ceil() as i32 }
}

/// RenderLighting_Frames l.1296–1370: the grid grown to the boxes' corners (index-space ceil, clamped to −32 / n + 32).
pub fn expand_grid(g: &GridDef, min: [f32; 3], max: [f32; 3]) -> GridDef {
    let lo_f = to_index(g, min);
    let hi_f = to_index(g, max);
    let mut out = *g;
    for k in 0..3 {
        let n0 = g.n[k] as i32;
        let mut lo = ceil_i(lo_f[k]);
        let mut hi = ceil_i(hi_f[k]);
        if lo < -0x20 {
            lo = -0x20;
        }
        if n0 + 0x20 < hi {
            hi = n0 + 0x20;
        }
        let mut n = n0;
        if lo < 0 {
            n = n0 - lo;
            out.origin[k] = out.origin[k] + lo as f32 * g.cell[k];
        }
        if 0 < hi - n0 {
            n += hi - n0;
        }
        out.n[k] = n as u32;
    }
    out
}

/// A probe chunk record (0x54 bytes at lm218+0x3b0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkRecord {
    /// +0: the atlas slot (column·32, row·16, 0)
    pub slot: [i32; 3],
    /// +0xc: the atlas index of the chunk's first probe = slot + (imin − chunk origin)
    pub amin: [i32; 3],
    /// +0x18: amin + (imax − imin) + 3
    pub amax: [i32; 3],
    /// +0x24
    pub cell: [f32; 3],
    /// +0x30: the world position of atlas index 0 — (imin − 1 − amin)·cell + grid origin
    pub origin: [f32; 3],
    /// The chunk's grid coordinates and its probe index range (imin..=imax) — not in the record, for printing.
    pub chunk: [u32; 3],
    pub imin: [i32; 3],
    pub imax: [i32; 3],
}

impl ChunkRecord {
    /// world(i) for an atlas index i: i·cell + origin (ProbeToWorld, FUN_140195860).
    pub fn world_of(&self, i: [f32; 3]) -> [f32; 3] {
        [i[0] * self.cell[0] + self.origin[0], i[1] * self.cell[1] + self.origin[1], i[2] * self.cell[2] + self.origin[2]]
    }
}

/// FUN_14021c980's output: the atlas dimensions (columns·32, rows·16, 32) and the records.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunking {
    pub grid: GridDef,
    pub chunk_counts: [u32; 3],
    pub atlas: [u32; 3],
    pub columns: u32,
    pub rows: u32,
    pub records: Vec<ChunkRecord>,
}

pub const CHUNK: [i32; 3] = [30, 14, 30];
pub const TILE: [i32; 3] = [32, 16, 32];

/// FUN_14021c980 for one grid (module doc, step 4).
pub fn chunk_once(g: &GridDef, boxes: &[CBox]) -> Chunking {
    let cx = (g.n[0] + 29) / 30;
    let cy = (g.n[1] + 13) / 14;
    let cz = (g.n[2] + 29) / 30;
    let nchunks = (cx * cy * cz) as usize;
    // per chunk {imin xyz, imax xyz}
    let mut ch: Vec<([i32; 3], [i32; 3])> = vec![([i32::MAX; 3], [i32::MIN; 3]); nchunks];
    let idx = |i: u32, j: u32, k: u32| -> usize { ((k * cy + j) * cx + i) as usize };
    let mut non_empty = 0u32;
    if boxes.is_empty() {
        // the centre chunk: one probe at ((cx/2 + 0.5)·30, (cy − 1 + 0.5)·14, (cz/2 + 0.5)·30)
        let (i, j, k) = (cx / 2, cy - 1, cz / 2);
        let p = [((i as f32 + 0.5) * 30.0).floor() as i32, ((j as f32 + 0.5) * 14.0).floor() as i32, ((k as f32 + 0.5) * 30.0).floor() as i32];
        ch[idx(i, j, k)] = (p, p);
        non_empty = 1;
    } else {
        let last = [g.n[0] as i32 - 1, g.n[1] as i32 - 1, g.n[2] as i32 - 1];
        for b in boxes {
            let lo_w = [b.c[0] - b.h[0], b.c[1] - b.h[1], b.c[2] - b.h[2]];
            let hi_w = [b.c[0] + b.h[0], b.c[1] + b.h[1], b.c[2] + b.h[2]];
            let mut lo = to_index(g, lo_w);
            let mut hi = to_index(g, hi_w);
            lo[0] += -1.01;
            lo[1] += -0.01;
            lo[2] += -1.01;
            hi[0] += 1.01;
            hi[1] += 1.01;
            hi[2] += 1.01;
            let cl = |v: f32, last: i32| -> i32 { let i = ceil_i(v); if i < 0 { 0 } else if last < i { last } else { i } };
            let (x0, y0, z0) = (cl(lo[0], last[0]), cl(lo[1], last[1]), cl(lo[2], last[2]));
            let (x1, y1, z1) = (cl(hi[0], last[0]), cl(hi[1], last[1]), cl(hi[2], last[2]));
            for k in (z0 / 30)..=(z1 / 30) {
                for j in (y0 / 14)..=(y1 / 14) {
                    for i in (x0 / 30)..=(x1 / 30) {
                        let c = &mut ch[idx(i as u32, j as u32, k as u32)];
                        if !(c.0[0] <= c.1[0]) {
                            non_empty += 1;
                        }
                        let start = [i * 30, j * 14, k * 30];
                        let lo_i = [x0, y0, z0];
                        let hi_i = [x1, y1, z1];
                        for a in 0..3 {
                            if lo_i[a] < c.0[a] {
                                c.0[a] = lo_i[a];
                            }
                            if c.0[a] < start[a] {
                                c.0[a] = start[a];
                            }
                            if c.1[a] < hi_i[a] {
                                c.1[a] = hi_i[a];
                            }
                            let end = start[a] + CHUNK[a] - 1;
                            if end < c.1[a] {
                                c.1[a] = end;
                            }
                        }
                    }
                }
            }
        }
        if non_empty == 0 {
            let (i, j, k) = (cx / 2, cy - 1, cz / 2);
            let p = [((i as f32 + 0.5) * 30.0).floor() as i32, ((j as f32 + 0.5) * 14.0).floor() as i32, ((k as f32 + 0.5) * 30.0).floor() as i32];
            ch[idx(i, j, k)] = (p, p);
            non_empty = 1;
        }
    }
    let columns = ((non_empty as f32).sqrt() as i32).max(1) as u32;
    let rows = (non_empty + columns - 1) / columns;
    let mut records = Vec::with_capacity(non_empty as usize);
    let (mut col, mut row) = (0u32, 0u32);
    for k in 0..cz {
        for j in 0..cy {
            for i in 0..cx {
                let (imin, imax) = ch[idx(i, j, k)];
                if imax[0] < imin[0] {
                    continue;
                }
                let slot = [col as i32 * 32, row as i32 * 16, 0];
                let start = [i as i32 * 30, j as i32 * 14, k as i32 * 30];
                let mut amin = [0i32; 3];
                let mut amax = [0i32; 3];
                let mut origin = [0f32; 3];
                for a in 0..3 {
                    amin[a] = slot[a] + (imin[a] - start[a]);
                    amax[a] = amin[a] + (imax[a] - imin[a]) + 3;
                    origin[a] = ((imin[a] - amin[a] - 1) as f32) * g.cell[a] + g.origin[a];
                }
                records.push(ChunkRecord { slot, amin, amax, cell: g.cell, origin, chunk: [i, j, k], imin, imax });
                col += 1;
                if col == columns {
                    col = 0;
                    row += 1;
                }
            }
        }
    }
    Chunking { grid: *g, chunk_counts: [cx, cy, cz], atlas: [columns * 32, rows * 16, 32], columns, rows, records }
}

/// FUN_14021d730: `chunk_once`, then the atlas-size test — (x ≤ 32 && z ≤ 32) → x, y, z ≤ `max_dim` (device+0x640);
/// else z ≤ 32 → x ≤ 256 && y ≤ 256; else x ≤ 128 && y ≤ 128 && z ≤ 128 — failing which the cell doubles (n = (n + 1)
/// >> 1) and the chunking reruns, at most 4 more times.
pub fn probe_chunks(g: &GridDef, boxes: &[CBox], max_dim: u32) -> Chunking {
    let mut g = *g;
    let mut tries = 0u32;
    loop {
        let c = chunk_once(&g, boxes);
        let [x, y, z] = c.atlas;
        let ok = if x <= 32 && z <= 32 {
            x <= max_dim && y <= max_dim && z <= max_dim
        } else if z <= 32 {
            x <= 256 && y <= 256
        } else {
            x <= 128 && y <= 128 && z <= 128
        };
        tries += 1;
        if ok || tries > 4 {
            return c;
        }
        for k in 0..3 {
            g.cell[k] += g.cell[k];
            g.n[k] = (g.n[k] + 1) >> 1;
        }
    }
}

/// FUN_140233150: the AABB of the chunk records' probe cells — c(i) = (float(i) − 0.5)·cell + origin for i = amin and
/// amax, min/max over both corners of every record, as {c = (max + min)·0.5, h = (max − min)·0.5}.
pub fn chunks_aabb(records: &[ChunkRecord]) -> Option<CBox> {
    if records.is_empty() {
        return None;
    }
    let mut mn = [f32::MAX; 3];
    let mut mx = [f32::MIN; 3];
    for r in records {
        for corner in [r.amin, r.amax] {
            for k in 0..3 {
                let c = (corner[k] as f32 - 0.5) * r.cell[k] + r.origin[k];
                if c <= mn[k] {
                    mn[k] = c;
                }
                if mx[k] <= c {
                    mx[k] = c;
                }
            }
        }
    }
    Some(CBox { c: [(mn[0] + mx[0]) * 0.5, (mn[1] + mx[1]) * 0.5, (mn[2] + mx[2]) * 0.5], h: [(mx[0] - mn[0]) * 0.5, (mx[1] - mn[1]) * 0.5, (mx[2] - mn[2]) * 0.5] })
}

/// The whole chain for a map's records: grid def → probe boxes → expansion → chunking → AABB.
pub fn for_records(size: [u32; 3], block_size: [f32; 3], offset: [f32; 3], level_h: f32, records: &[BlockRecord], scene: &CBox, max_dim: u32) -> (GridDef, Vec<CBox>, Chunking, Option<CBox>) {
    let g0 = grid_def(size, block_size, offset, level_h, true);
    let mut boxes = probe_boxes(records);
    if records.is_empty() {
        boxes.push(default_probe_box(scene));
    }
    let g = match corners(&boxes) {
        Some((mn, mx)) => expand_grid(&g0, mn, mx),
        None => g0,
    };
    let c = probe_chunks(&g, &boxes, 2048);
    let aabb = chunks_aabb(&c.records);
    (g, boxes, c, aabb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pwc_records() -> Vec<BlockRecord> {
        let b = |c: [f32; 3], h: [f32; 3]| CBox::new(c, h);
        vec![
            BlockRecord { world: b([863.34534, 34.1102, 358.4614], [2.3353567, 1.7935166, 2.3573089]), quality: 1.0 },
            BlockRecord { world: b([872.0, 32.49999, 361.0], [8.000003, 0.01999998, 8.000001]), quality: 1.0 },
            BlockRecord { world: b([872.0, 63.5, 337.0], [8.0, 32.0, 0.02000068]), quality: 1.0 },
        ]
    }

    #[test]
    fn bluebay_64_cubed_is_128x32x128_probes_of_16m_at_8_minus30_8() {
        let g = grid_def([64, 64, 64], [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 0.0, true);
        assert_eq!(g, GridDef { n: [128, 32, 128], cell: [16.0, 16.0, 16.0], origin: [8.0, -30.0, 8.0] });
        // a level h that is not on the grid moves the rows up onto h + 2: h = 5 → rows at −30 + frac⁺((7 + 30)/16)·16
        let g5 = grid_def([64, 64, 64], [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 5.0, true);
        assert_eq!(g5.origin[1], -30.0 + (37.0f32 / 16.0 - 2.0) * 16.0);
        // a cubic collection keeps its block size as the cell
        let gs = grid_def([48, 40, 48], [32.0, 32.0, 32.0], [0.0, -62.0, 0.0], 0.0, false);
        assert_eq!((gs.n, gs.cell), ([48, 40, 48], [32.0, 32.0, 32.0]));
    }

    #[test]
    fn pwc_day_items_land_in_chunk_1_0_0_with_the_captured_origin_and_atlas() {
        let scene = CBox::from_min_max([-1.5258789e-5, 3.9999785, 7.6293945e-6], [2048.0, 95.5, 2048.0]);
        let (g, boxes, c, aabb) = for_records([64, 64, 64], [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 0.0, &pwc_records(), &scene, 2048);
        assert_eq!(boxes.len(), 3, "quality 1 records are probe boxes");
        assert_eq!(g.n, [128, 32, 128], "no expansion: the items lie inside the map grid");
        assert_eq!(c.chunk_counts, [5, 3, 5], "= the source map's own probe volume slot grid [5, 3, 5]");
        assert_eq!(c.records.len(), 1);
        let r = &c.records[0];
        assert_eq!(r.chunk, [1, 0, 0]);
        assert_eq!(r.origin, [472.0, -46.0, -8.0], "engineer B's ProbeToShadow-derived origin");
        assert_eq!(c.atlas, [32, 16, 32]);
        assert_eq!(r.amax[1], 12, "RE 5's imax.y = 12 → the world peel's y max (12 − 0.5)·16 − 46 = 138");
        let a = aabb.unwrap();
        assert_eq!(a.max()[1], 138.0);
        assert_eq!((r.amin, r.amax), ([23, 4, 20], [29, 12, 27]), "slot + (imin − 30·1), + (imax − imin) + 3");
        assert_eq!(r.imin, [53, 4, 20]);
        assert_eq!(r.imax, [56, 9, 24], "the pad/wall reach x 880 → ceil(54.5 + 1.01) = 56, z 369 → ceil(22.56 + 1.01) = 24");
    }

    #[test]
    fn many_chunks_lay_out_in_a_near_square_atlas_and_a_huge_grid_doubles_its_cell() {
        let g = grid_def([64, 64, 64], [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 0.0, true);
        // one box over the whole map at ground level: 5 × 1 × 5 = 25 chunks → 5 columns × 5 rows
        let big = CBox::from_min_max([0.0, 0.0, 0.0], [2048.0, 10.0, 2048.0]);
        let c = probe_chunks(&g, &[big], 2048);
        assert_eq!(c.records.len(), 25);
        assert_eq!((c.columns, c.rows, c.atlas), (5, 5, [160, 80, 32]));
        // the chunks are numbered z-major, then y, then x; the slots row by row
        assert_eq!(c.records[0].chunk, [0, 0, 0]);
        assert_eq!(c.records[1].chunk, [1, 0, 0]);
        assert_eq!(c.records[5].slot, [0, 16, 0]);
        // a 192³ map (384 × 32 × 384 probes of 16 m): a full-height box touches 13 × 3 × 13 = 507 chunks → 22 columns → an
        // atlas 704 wide > 256 → the cell doubles to 32 m (192 × 16 × 192 → 7 × 2 × 7 = 98 chunks, 9 columns, 288 > 256), then
        // to 64 m (96 × 8 × 96 → 4 × 1 × 4 = 16 chunks, a 128 × 64 atlas)
        let g192 = grid_def([192, 64, 192], [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 0.0, true);
        let tall = CBox::from_min_max([0.0, -30.0, 0.0], [6144.0, 480.0, 6144.0]);
        let c = probe_chunks(&g192, &[tall], 2048);
        assert_eq!(c.grid.cell, [64.0, 64.0, 64.0]);
        assert_eq!(c.grid.n, [96, 8, 96]);
        assert_eq!((c.records.len(), c.atlas), (16, [128, 64, 32]));
    }

    #[test]
    fn the_grid_expands_to_boxes_outside_the_map() {
        let g = grid_def([64, 64, 64], [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 0.0, true);
        let e = expand_grid(&g, [-100.0, -30.0, 8.0], [2100.0, 100.0, 2000.0]);
        // x: lo = ceil((−100 − 8)/16) = −6 → n += 6, origin −= 96; hi = ceil((2100 − 8)/16) = 131 → n += 3
        assert_eq!(e.n[0], 128 + 6 + 3);
        assert_eq!(e.origin[0], 8.0 - 96.0);
        assert_eq!(e.n[2], 128);
        // clamped at 32 cells each way
        let e = expand_grid(&g, [-10000.0, 0.0, 0.0], [10000.0, 1.0, 1.0]);
        assert_eq!(e.n[0], 128 + 32 + 32);
    }

    #[test]
    fn no_records_means_the_default_box_and_no_box_means_the_centre_chunk() {
        let scene = CBox::from_min_max([0.0, 3.0, 0.0], [2048.0, 95.0, 2048.0]);
        let d = default_probe_box(&scene);
        assert_eq!((d.min(), d.max()), ([0.0, -32.0, 0.0], [2048.0, 128.0, 2048.0]));
        let g = grid_def([64, 64, 64], [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 0.0, true);
        let c = chunk_once(&g, &[]);
        assert_eq!(c.records.len(), 1);
        assert_eq!(c.records[0].chunk, [2, 2, 2]);
    }
}

/// FUN_140d124c0(collection) = the "level" the probe rows are aligned on (h + 2), by the collection NUMBER (the
/// engine compares interned Ids; the Id table 0x141e71130 is filled by 0x140ae6370 slot by slot with these numbers —
/// slot 9 = 26 Stadium, 17 = 15 GreenCoast, 18 = 16 RedIsland, 19 = 28 BlueBay, 20 = 29 WhiteShore, 0 = 12 Canyon,
/// 1 = 18 Valley, 2 = 19 Lagoon, 3 = 11, 4 = 20, 5 = 21, 6 = 13, 7 = 22, 8 = 23, 10 = 24, 11 = 25, 12 = 202, 27 = 6):
/// FUN_140d123c0 gives 1.0 for {202, 6, 24, 25}, 0.0 for {26, 12, 18, 19}, 2.0 for {11, 20, 21, 13, 22, 23} and 0 for
/// every other collection (its `FUN_140ae66a0() ? 2·FUN_140d12190 : 0` branch: FUN_140ae66a0 tests the slot lookup
/// against 0x21, which the lookup can never return → always 0); then +8 for {13, 22, 23} and +8 for {6, 26}.
/// TM2020: Stadium (26) → 8 (the Grass plane at 8 + 2 = 10 m), BlueBay / WhiteShore / RedIsland / GreenCoast → 0.
pub fn level_h(collection: u32) -> f32 {
    let base = match collection {
        202 | 6 | 24 | 25 => 1.0,
        26 | 12 | 18 | 19 => 0.0,
        11 | 20 | 21 | 13 | 22 | 23 => 2.0,
        _ => 0.0,
    };
    let a = if matches!(collection, 13 | 22 | 23) { 8.0 } else { 0.0 };
    let b = if matches!(collection, 6 | 26) { 8.0 } else { 0.0 };
    base + a + b
}

/// The decoration's vertical offsets per TM2020 collection [INFERRED from the fixed water/ground planes
/// (tiny::fixed_plane) and the port's probe-origin table (probes.rs), consistent with the capture]: `yoff` = the
/// block placement offset (world y = cell·8 + yoff), the probe grid's `offset.y` (challenge+0x7e8) = yoff + 2.
/// BlueBay / GreenCoast (sea cell 5 / lake cell 4 → planes 7.0 / −0.8): −40 → −38; RedIsland / WhiteShore (cell 14
/// → −0.5 / −1.0): −120 → −118; Stadium (Grass cell 9, plane 10.0): −64 → −62.
pub fn deco_offsets(collection: u32) -> Option<(f32, f32)> {
    match collection {
        28 | 15 => Some((-40.0, -38.0)),
        16 | 29 => Some((-120.0, -118.0)),
        26 => Some((-64.0, -62.0)),
        _ => None,
    }
}

#[cfg(test)]
mod level_tests {
    use super::*;

    #[test]
    fn stadium_rows_sit_on_the_grass_plane_and_the_island_collections_on_zero() {
        assert_eq!(level_h(26), 8.0);
        assert_eq!(level_h(28), 0.0);
        assert_eq!(level_h(15), 0.0);
        assert_eq!(level_h(16), 0.0);
        assert_eq!(level_h(29), 0.0);
        assert_eq!(level_h(13), 10.0, "2 + 8");
        assert_eq!(level_h(6), 9.0, "1 + 8");
        // Stadium 48×40×48 with 32 m cubes: rows aligned on 10 m → origin.y = −62 + 16 + frac⁺((10 − (−46))/32)·32 = −46 + 24 = −22
        let g = grid_def([48, 40, 48], [32.0, 32.0, 32.0], [0.0, -62.0, 0.0], level_h(26), false);
        assert_eq!(g.origin[1], -22.0);
        assert_eq!(deco_offsets(28), Some((-40.0, -38.0)));
    }
}

/// The MODEL PROBE BOX of an item's lm-data object (the 0x120-byte object at record+0x18, ctor 0x140454a20, filled by
/// FUN_140454590 at bind): the object walks the model's geometry — a CPlugSolid2Model's shaded geoms with LOD bit 0
/// (geom+0xc & 1) at the identity location, or a CPlugSolid's tree with each leaf's Location — and folds each visual's
/// STORED box (visual+0x88) through FUN_140185f70 into TWO boxes with FUN_140184fa0: obj+0xd4 = the LIGHTMAP box (geoms
/// whose material has flag 0x80 at material+0x158; the material class bits +0x144 & 0x600000 == 0x400000 set obj
/// flag 4) and obj+0x100 = the PROBE box (the other geoms whose material has flag 0x1000 at material+0x244).
/// RenderLighting_Frames uses obj+0x100 (valid when its h.x = obj+0x10c ≥ 0) through the mobil Iso4 as the probe box
/// of a record whose quality² ≤ 0.9 (Lowest/VeryLow/Low elements: 0.706² = 0.498). Which game materials carry the
/// 0x1000 flag is NOT read (a CPlugMaterial flags word) — `model_probe_box` takes the caller's selection of visual
/// boxes; with every LOD-0 geom it is the whole model. Both folds start from the invalid box (the first box copied).
pub fn model_probe_box(visual_boxes: &[CBox]) -> Option<CBox> {
    let mut acc = CBox::INVALID;
    for b in visual_boxes {
        acc.union_into(b);
    }
    if acc.is_valid() { Some(acc) } else { None }
}

/// The probe boxes with the model boxes for the low-quality records (RenderLighting_Frames l.1085–1147 both
/// branches): `(record, model probe box in model space)` pairs; a record with quality² > 0.9 gives its own box, one
/// below gives its model's probe box through its Iso4 when that box is valid, else nothing.
pub fn probe_boxes_with_models(items: &[(BlockRecord, Option<CBox>, crate::lmtiles::Iso4)]) -> Vec<CBox> {
    let mut out = Vec::new();
    for (r, model_probe, iso) in items {
        if 0.9 < r.quality * r.quality {
            out.push(r.world);
        } else if let Some(mb) = model_probe {
            if mb.h[0] >= 0.0 {
                out.push(mb.transformed(iso));
            }
        }
    }
    out
}

#[cfg(test)]
mod model_probe_tests {
    use super::*;

    #[test]
    fn low_quality_records_use_their_models_probe_box() {
        let own = BlockRecord { world: CBox::new([10.0, 5.0, 10.0], [1.0, 1.0, 1.0]), quality: 1.0 };
        let low = BlockRecord { world: CBox::new([50.0, 5.0, 50.0], [4.0, 4.0, 4.0]), quality: 180.0 / 255.0 };
        let mp = model_probe_box(&[CBox::new([0.0, 1.0, 0.0], [2.0, 1.0, 2.0])]);
        let iso: crate::lmtiles::Iso4 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 50.0, 4.0, 50.0];
        let boxes = probe_boxes_with_models(&[(own, None, iso), (low, mp, iso), (low, None, iso)]);
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[1], CBox::new([50.0, 5.0, 50.0], [2.0, 1.0, 2.0]));
        assert_eq!(model_probe_box(&[]), None);
    }
}
