//! The lightmapper's BLOCK RECORDS, its SCENE BOX and the PEEL TILING RULE — the three CPU inputs of the light-camera
//! fit (`lightcam::fit_camera`) that a bake WITHOUT a GPU capture has to produce itself. Everything here is a
//! transcription of the client (Trackmania.exe 2025-08-24, RE child 6, 2026-09-25; addresses in the item notes):
//!
//! * `CBox` — the engine's axis-aligned box {centre, half} (6 floats; `half.x < 0` = invalid), with the two box
//!   primitives every step below uses: `union_into` = FUN_140184fa0 (min = min(c − h), max = max(c + h) per axis,
//!   then c = (max + min)·0.5, h = (max − min)·0.5 — a RE-CENTRING per union step, which is where the last bits of a
//!   multi-record box come from) and `transformed` = FUN_140185f70 (c' = ((m_i0·c_x + m_i1·c_y) + m_i2·c_z) + t_i,
//!   h' = (|m_i0|·h_x + |m_i1|·h_y) + |m_i2|·h_z, f32 in that order, no FMA).
//! * the MODEL BOX of an item = its CPlugSolid's TREE bounding box (CHmsItem+0x18 → CPlugSolid+0x18 = the CPlugTree,
//!   bbox at tree+0xc8/+0xd4; CPlugTree::UpdateBBox 0x1403f2430 with flag 0 — the load path SetTree(…, 0) at
//!   0x1404141fd): a leaf's box is its CPlugVisual's STORED bounding box (visual+0x88, the file's `bounding_box` of
//!   chunk 0x0902C004 — NOT recomputed from the vertices: UpdateBBox(0) skips the visual's vtbl+0x1f8 recompute, and
//!   the wall's ±0.02 thickness in the capture is exactly the tiny library writer's `max(half, 0.02)`), the first
//!   valid child COPIED, every further child unioned with FUN_140184fa0 in child order, the tree's Location applied
//!   when flag 4 (none for an item's solid). `model_box` does this over the shaded geoms' visuals in order.
//! * a BLOCK RECORD (0x58 bytes at CHmsLightMap+0xd8, filled by FUN_14020e3c0): +0x38 centre, +0x44 half = the model
//!   box through the mobil's Iso4 (FUN_140185f70), +0x50 = qualityByte/255 — `block_record`. The mobil's Iso4 for a
//!   map item is RE 4's chain (`item_iso4`: yaw/pitch/roll → quaternion 0x140193710 → 3×3 0x1401886d0 → the
//!   translation of the [I | pivot]·[m | pos] product 0x140183fd0), the engine's row-major layout m[3i + j],
//!   world_i = Σ_j m[3i + j]·local_j + t_i.
//! * the SCENE BOX computeParams+0xb0 (the sun camera's focus box, the world peel's before the probe-chunk union) =
//!   FUN_140226e80: the FUN_140184fa0 fold of every block record's box from the invalid box (the lm+0x484 ≠ 0 branch;
//!   the other branch folds the same boxes per packed static-geometry group inside FUN_140254d60 — the same numbers
//!   up to the fold order) — `scene_box`.
//! * the PEEL TILING RULE FUN_140230080 — `peel_tiling`, see its doc — gives the target size (4096² or 2048²), the
//!   grid n×n and the FITTED peel boxes {c, h} (one per non-empty, unmerged cell); the fit takes those {c, h}
//!   DIRECTLY (`lightcam::fit_camera_ch`) — the tile's c = (xmin + xmax)·0.5 is not the min + (max − min)·0.5 an
//!   Aabb round trip would give.
//!
//! Verified against passcap/pwc-day (`lmtool frustum-check ROOT --frame F --map MAP [--scene …]`): from the map's three
//! items alone, with the editor's alloc scale 31.75 layout units/m, the FITTED peel cameras of frame 127449 (eid 4902,
//! D (0.445, 0.293, 0.846); eid 10898, D (0.244, −0.075, 0.967)) and of frame 7530 (eid 110, D (−0.061, 0.994, 0.088);
//! eid 4970, D (−0.602, −0.746, 0.286)) are ALL 55 SceneV VALUES BIT-IDENTICAL, their accumulates' WorldPw01Shadow
//! within 1 ulp; frame 127448's direction 0 (eid 4330) is within 3 ulps — the same 3 ulps as its WORLD peel, i.e.
//! that direction's basis last bit (engineer B's open 1/√ item), not the box (B's Aabb route was at 26 ulps). The
//! tiling on pwc-day: ext = 2048 m · 31.75 = 65 024 layout units > 3072 → 4096² targets, n = ceil(65 024 / 4096) = 16
//! → clamped to 4, the three records in cell (1, 0) of the 4×4 grid, every other cell empty → ONE fitted tile =
//! x [861.01, 880] × y [S] × z [336.98, 369] — the capture's one fitted peel per direction.

use crate::geometry::ItemPose;

/// The engine's axis-aligned box: centre and half extents (GmBoxAligned as the lightmapper stores it — 6 floats,
/// `h.x < 0` marks an invalid/empty box, FUN_140184fa0's test `comiss 0, h.x; ja`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CBox {
    pub c: [f32; 3],
    pub h: [f32; 3],
}

impl CBox {
    /// The invalid box (centre 0, half −1): what FUN_140226e80 / CPlugTree start their folds from.
    pub const INVALID: CBox = CBox { c: [0.0; 3], h: [-1.0; 3] };

    pub fn new(c: [f32; 3], h: [f32; 3]) -> CBox {
        CBox { c, h }
    }
    /// From corners: c = (max + min)·0.5, h = (max − min)·0.5 (FUN_140184fa0's re-centring form).
    pub fn from_min_max(min: [f32; 3], max: [f32; 3]) -> CBox {
        let mut b = CBox { c: [0.0; 3], h: [0.0; 3] };
        for k in 0..3 {
            b.c[k] = (max[k] + min[k]) * 0.5;
            b.h[k] = (max[k] - min[k]) * 0.5;
        }
        b
    }
    /// `0 > h.x` (comiss + ja): the box is empty/invalid.
    pub fn is_valid(&self) -> bool {
        !(0.0 > self.h[0])
    }
    pub fn min(&self) -> [f32; 3] {
        [self.c[0] - self.h[0], self.c[1] - self.h[1], self.c[2] - self.h[2]]
    }
    pub fn max(&self) -> [f32; 3] {
        [self.c[0] + self.h[0], self.c[1] + self.h[1], self.c[2] + self.h[2]]
    }
    /// As lightcam's min/max box.
    pub fn aabb(&self) -> crate::lightcam::Aabb {
        crate::lightcam::Aabb { min: self.min(), max: self.max() }
    }

    /// FUN_140184fa0(acc = self, b): `if 0 > acc.h.x { *acc = *b; return }`, `if 0 > b.h.x { return }`, else per
    /// axis min = minss(b.c − b.h, acc.c − acc.h), max = maxss(b.h + b.c, acc.c + acc.h); acc.c = (max + min)·0.5,
    /// acc.h = (max − min)·0.5 (asm 140184fa0..14018516a, every op an f32 scalar).
    pub fn union_into(&mut self, b: &CBox) {
        if 0.0 > self.h[0] {
            *self = *b;
            return;
        }
        if 0.0 > b.h[0] {
            return;
        }
        for k in 0..3 {
            let bmin = b.c[k] - b.h[k];
            let amin = self.c[k] - self.h[k];
            let bmax = b.h[k] + b.c[k];
            let amax = self.c[k] + self.h[k];
            // minss/maxss: the second operand when unordered — no NaN reaches here
            let mn = if bmin < amin { bmin } else { amin };
            let mx = if bmax > amax { bmax } else { amax };
            self.c[k] = (mx + mn) * 0.5;
            self.h[k] = (mx - mn) * 0.5;
        }
    }

    /// FUN_140185f70(out, self, m): the box through an Iso4 in the engine's layout — c'_i = ((m[3i]·c_x + m[3i+1]·c_y)
    /// + m[3i+2]·c_z) + t_i, h'_i = (|m[3i]|·h_x + |m[3i+1]|·h_y) + |m[3i+2]|·h_z (the abs an `andps 0x7fffffff`),
    /// f32 mulss/addss in that order.
    pub fn transformed(&self, m: &Iso4) -> CBox {
        let (c, h) = (self.c, self.h);
        let mut o = CBox { c: [0.0; 3], h: [0.0; 3] };
        for i in 0..3 {
            let (m0, m1, m2) = (m[3 * i], m[3 * i + 1], m[3 * i + 2]);
            o.c[i] = ((m0 * c[0] + m1 * c[1]) + m2 * c[2]) + m[9 + i];
            o.h[i] = (m0.abs() * h[0] + m1.abs() * h[1]) + m2.abs() * h[2];
        }
        o
    }
}

/// The engine's Iso4: the row-major 3×3 m[3i + j] (world_i = Σ_j m[3i + j]·local_j) then the translation t = m[9..12]
/// — the layout FUN_140185f70 and RE 4's pose chain (mapgeom::veget_instance) use. mapgeom's `Xform` is its transpose
/// (`from_xform`).
pub type Iso4 = [f32; 12];

/// The mapgeom Xform (columns = the axes: world = m[0..3]·x + m[3..6]·y + m[6..9]·z + m[9..12]) as an engine Iso4.
pub fn from_xform(x: &mapgeom::geom::Xform) -> Iso4 {
    [x[0], x[3], x[6], x[1], x[4], x[7], x[2], x[5], x[8], x[9], x[10], x[11]]
}

/// A map item's mobil Iso4 — RE 4's chain (bit-exact against 1969 live forest instances): yaw/pitch/roll →
/// quaternion (0x140193710, the double Cephes sincos), quaternion → the row-major 3×3 (0x1401886d0), the translation
/// of [I | pivot] · [m | pos] (0x140183fd0: t_i = ((m[3i+1]·pv.y + pv.x·m[3i]) + m[3i+2]·pv.z) + pos_i). A non-unit
/// item scale multiplies the 3×3 (INFERRED: the placement's `Scale` field is a uniform scale of the mobil; the
/// pwc-day items are unscaled and the chain was pinned on unscaled trees).
pub fn item_iso4(p: &ItemPose) -> Iso4 {
    use mapgeom::veget_instance::{iso4_translation, quat_to_mat, ypr_to_quat};
    let q = ypr_to_quat(p.yaw, p.pitch, p.roll);
    let mut m = quat_to_mat(q);
    let t = iso4_translation(&m, p.pivot, p.pos);
    if p.scale > 0.0 && p.scale != 1.0 {
        for v in m.iter_mut() {
            *v *= p.scale;
        }
    }
    [m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], t[0], t[1], t[2]]
}

/// The CPlugTree bounding box of a solid whose leaves carry these visual boxes, in child order — CPlugTree::UpdateBBox
/// (0x1403f2430) with flag 0 for a root without its own visual: `None` when no child has a valid box, else the first
/// valid child's box COPIED and every later valid one unioned with FUN_140184fa0 (0x1403f24d7..0x1403f2613). No
/// Location on an item's solid (flag 4 clear) — the box stays in model space.
pub fn model_box(visual_boxes: &[CBox]) -> Option<CBox> {
    let mut acc: Option<CBox> = None;
    for b in visual_boxes {
        if !b.is_valid() {
            continue;
        }
        match acc.as_mut() {
            None => acc = Some(*b),
            Some(a) => a.union_into(b),
        }
    }
    acc
}

/// A block record's lightmapper fields (0x58 bytes at CHmsLightMap+0xd8, FUN_14020e3c0): the world box (+0x38
/// centre, +0x44 half) and the quality scale +0x50 = byte/255.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockRecord {
    pub world: CBox,
    /// qualityByte / 255 — the per-element MapElemLightmapQuality through FUN_140dcc1c0 (`quality_byte`).
    pub quality: f32,
}

/// FUN_14020e3c0's record fill: the model box through the mobil's Iso4 (FUN_140185f70) and `(float)byte / 255.0`.
pub fn block_record(model: &CBox, m: &Iso4, quality_byte: u8) -> BlockRecord {
    BlockRecord { world: model.transformed(m), quality: quality_byte as f32 / 255.0 }
}

/// The scene-bound quality byte of an element (RE 2, `CGameCtnApp::HmsLightMapUpdateBlocksAndItemsQuality`
/// 0x140dcc290 → FUN_140dcc1c0): f = (√2)^e · G with e = {0 Normal: 0, 1 High: +1, 2 VeryHigh: +2, 3 Highest: +3,
/// 4 Lowest: −1, 5 VeryLow: −2, 6 Low: −3, other: 0} (FUN_140dcc160) and G = 1.0 for the map's own objects, 0.0625
/// for the decoration challenge's on the Stadium-family collection, 0.5 on the other collections;
/// byte = clamp(int(f · 255), 1, 255).
pub fn quality_byte(elem_quality: u8, global: f32) -> u8 {
    let e: i32 = match elem_quality {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => -1,
        5 => -2,
        6 => -3,
        _ => 0,
    };
    let f = std::f32::consts::SQRT_2.powi(e) * global;
    ((f * 255.0) as i32).clamp(1, 255) as u8
}

/// The 0.51 gate of the tiling rule (FUN_140230080 l.170, `comiss 0.51, rec+0x50; jae skip`): a record takes part
/// in the fitted tiles when its quality scale is STRICTLY above 0.51 — the map's own objects down to Lowest
/// (√2⁻¹ = 0.707), never the decoration's (G ≤ 0.5 → 127/255 = 0.498).
pub fn in_fitted_tiles(r: &BlockRecord) -> bool {
    0.51 < r.quality
}

/// computeParams+0xb0 — FUN_140226e80's record fold (lm+0x484 ≠ 0): the invalid box, then FUN_140184fa0 with every
/// record's box in record (bind) order. The lm+0x484 == 0 branch folds the same boxes per packed static group
/// (FUN_140254a00 → FUN_140254d60, `FUN_140184fa0(outBox, geom+0)`), so the fold ORDER there is the packing order,
/// not the bind order — the same box up to its last bits.
pub fn scene_box(records: &[BlockRecord]) -> CBox {
    let mut b = CBox::INVALID;
    for r in records {
        b.union_into(&r.world);
    }
    b
}

/// The WORLD peel's focus box (RenderLightIndirectDome 0x140233b50 l.171–184): the scene box copied into the peel
/// state (+0x18), then FUN_140184fa0 with the probe-chunk AABB (FUN_140233150) when the lightmapper has a probe grid
/// (lm+0x370 ≠ 0). The sun camera uses the scene box alone.
pub fn world_peel_box(scene: &CBox, probe_chunks_aabb: Option<&CBox>) -> CBox {
    let mut w = *scene;
    if let Some(p) = probe_chunks_aabb {
        w.union_into(p);
    }
    w
}

/// The knobs FUN_140230080 reads besides the scene box and the records.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileParams {
    /// desc+0x8 = lm218+0x488 = the chart allocation's final scale (UpdateMapping 0x14020f510 l.167 ← alloc+0x14 =
    /// AllocateWithScale_BlockSplit's s_final = √(scale_lo·D), LAYOUT UNITS per metre — 2048 layout units across the
    /// 1024² Default atlas). Ignored (1.0) when there are no records.
    pub alloc_scale: f32,
    /// EHmsLightMapQuality (0 VFast … 5 Ultra2): below 2 the extent is halved when `half_at_low_quality`.
    pub quality: u32,
    /// desc+0x20 (RenderLightIndirectBounces sets 1): halve the extent at quality < 2.
    pub half_at_low_quality: bool,
    /// desc+0x24: a forced target size (0 = none; RenderLightIndirectBounces passes 0).
    pub size_override: u32,
    /// The device's video memory in bytes (device+0x360): 4096² needs > 1536 MB.
    pub vram_bytes: i64,
    /// CHmsLightMapParam+0xb4: the maximum n per axis (4 in the ctor).
    pub max_tiles: u32,
}

impl TileParams {
    /// The pwc-day capture's knobs: quality 3 (High), the bounce pass's desc (+0x20 = 1, +0x24 = 0), a card over
    /// 1.5 GB (the capture chose 4096²), max tiles 4.
    pub fn pwc_day(alloc_scale: f32) -> TileParams {
        TileParams { alloc_scale, quality: 3, half_at_low_quality: true, size_override: 0, vram_bytes: 8 << 30, max_tiles: 4 }
    }
}

/// FUN_140230080's output: the peel target size, the grid n (cells per axis over the scene box's x/z) and the fitted
/// peel boxes (none = the world pass only).
#[derive(Clone, Debug, PartialEq)]
pub struct Tiling {
    pub size: u32,
    pub n: u32,
    /// The extent the rule sized from: max(2hx, 2hz)·s (× 0.5 / × 4.2 as applicable), in layout units.
    pub ext: f32,
    pub tiles: Vec<CBox>,
}

/// `(int)ceilf(x)` — FUN_1418f6954 (the CRT's int ceil: NaN/inf → 0; here the argument is finite and positive).
fn ceil_int(x: f32) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    x.ceil() as i32
}

/// (int)floorf(x) — FUN_14195c7b8 then cvttss2si.
fn floor_int(x: f32) -> i32 {
    x.floor() as i32
}

/// THE PEEL TILING RULE — FUN_140230080(tiles, &size, &desc, qualityEnum, lm+8, LightMapParam), asm 140230080..
/// 140230ab3, every float op an f32 scalar in this order:
///
/// ```text
/// s     = count ? desc.allocScale : 1.0
/// ext   = max(2hx·s, 2hz·s)            (maxss)
/// if quality < 2 && desc.halfFlag: ext *= 0.5
/// if count == 0:                   ext *= 4.2
/// vramMB = vramBytes / 2^20  (signed, toward zero)
/// size  = ((ext > 3072 || count == 0) && vramMB > 1536) ? 4096 : 2048;  desc.override ≠ 0 → size = override
/// n     = ceil(ext / size);  n < 2 → n = 1  else n = min(n, maxTiles)
/// count ≠ 0 && n < 2 → NO tiles (the world pass only)                              [count == 0: n = 1 is fine]
/// cellW = 2hx / n, cellD = 2hz / n;  ix(x) = floor((x − (cx − hx))·(n / 2hx)), iz likewise
/// count == 0: n×n plain cells {cx = (i·cellW + (i+1)·cellW)·0.5, 48.0, cz likewise, hx = ((i+1)·cellW − i·cellW)·0.5, 80.0, hz}
///             — NOTE: measured from 0, not from the scene box's min (the game's own arithmetic)
/// count ≠ 0: cell[i][j] = {xmin, zmin, xmax, zmax} = {+∞, +∞, −∞, −∞}
///   for each record with quality > 0.51: (x0, x1) = (c − h, c + h), (z0, z1) likewise
///       i0..i1 = clamp(ix(x0)), clamp(ix(x1)); j0..j1 likewise (clamped to 0..n−1)
///       for j in j0..=j1, i in i0..=i1:
///           lo_x = i > 0     ? max(x0, i·cellW)     : x0        (again from 0, not the box min)
///           hi_x = i < n − 1 ? min(x1, (i+1)·cellW) : x1        (the outer cells are unclamped)
///           the same in z;  cell.xmin = min(cell.xmin, lo_x) … (strict < / > updates)
///   merge, row by row, each cell with its RIGHT neighbour when both are non-empty and nb.xmax − cell.xmin ≤ cellW
///     (cell takes nb.xmax, min/max of z; nb emptied); then each cell with the one BELOW when nb.zmax − cell.zmin ≤ cellD
///   every non-empty cell → {c = (xmin + xmax)·0.5, cy = (ymin + ymax)·0.5, cz…; h = (xmax − xmin)·0.5, (ymax − ymin)·0.5, …}
///   with ymin = cy_box − hy_box, ymax = cy_box + hy_box (the scene box's y range)
/// ```
pub fn peel_tiling(scene: &CBox, records: &[BlockRecord], p: &TileParams) -> Tiling {
    let count = records.len();
    let s = if count == 0 { 1.0f32 } else { p.alloc_scale };
    let (cx, cy, cz) = (scene.c[0], scene.c[1], scene.c[2]);
    let (hx, hy, hz) = (scene.h[0], scene.h[1], scene.h[2]);
    let two_hz = hz * 2.0;
    let two_hx = hx * 2.0;
    let ymin = cy - hy;
    let ymax = cy + hy;
    let a = two_hx * s;
    let b = two_hz * s;
    let mut ext = if a <= b { b } else { a };
    if p.quality < 2 && p.half_at_low_quality {
        ext *= 0.5;
    }
    if count == 0 {
        ext *= 4.2;
    }
    // cqo; and edx, 0xfffff; add rax, rdx; sar rax, 20 — a signed division by 2^20 rounding toward zero
    let vram_mb = (p.vram_bytes + ((p.vram_bytes >> 63) & 0xfffff)) >> 20;
    let mut size: u32 = if (ext > 3072.0 || count == 0) && vram_mb > 0x600 { 0x1000 } else { 0x800 };
    if p.size_override != 0 {
        size = p.size_override;
    }
    let n_raw = ceil_int(ext / size as f32);
    let n: u32 = if n_raw < 2 {
        1
    } else {
        let mut n = n_raw as u32;
        if p.max_tiles <= n {
            n = p.max_tiles;
        }
        n
    };
    if n < 2 && count != 0 {
        return Tiling { size, n, ext, tiles: Vec::new() };
    }
    let nf = n as f32;
    let inv_w = nf / two_hx; // fVar37: cells per metre in x
    let inv_d = nf / two_hz; // fVar25
    let cell_w = two_hx / nf; // fVar29
    let cell_d = two_hz / nf; // fVar30
    let off_x = -((cx - hx) * inv_w); // fVar22: −xmin·n/2hx
    let off_z = -((cz - hz) * inv_d); // fVar27 (recomputed per record in the asm, the same value)
    let mut tiles = Vec::new();
    if count == 0 {
        for row in 0..n {
            for col in 0..n {
                let z1 = (row + 1) as f32 * cell_d;
                let z0 = row as f32 * cell_d;
                let x1 = (col + 1) as f32 * cell_w;
                let x0 = col as f32 * cell_w;
                tiles.push(CBox { c: [(x0 + x1) * 0.5, 48.0, (z1 + z0) * 0.5], h: [(x1 - x0) * 0.5, 80.0, (z1 - z0) * 0.5] });
            }
        }
        return Tiling { size, n, ext, tiles };
    }
    let nn = (n * n) as usize;
    // {xmin, zmin, xmax, zmax} per cell, row-major (z rows, x columns)
    let mut cells: Vec<[f32; 4]> = vec![[f32::MAX, f32::MAX, f32::MIN, f32::MIN]; nn];
    let last = n as i32 - 1;
    let clamp = |i: i32| -> i32 {
        if i < 0 {
            0
        } else if last < i {
            last
        } else {
            i
        }
    };
    for r in records {
        if !in_fitted_tiles(r) {
            continue;
        }
        let (rc, rh) = (r.world.c, r.world.h);
        let x0 = rc[0] - rh[0];
        let x1 = rc[0] + rh[0];
        let z0 = rc[2] - rh[2];
        let z1 = rc[2] + rh[2];
        let i0 = clamp(floor_int(inv_w * x0 + off_x));
        let j0 = clamp(floor_int(inv_d * z0 + off_z));
        let i1 = clamp(floor_int(inv_w * x1 + off_x));
        let j1 = clamp(floor_int(inv_d * z1 + off_z));
        let mut j = j0;
        while j <= j1 {
            let mut i = i0;
            while i <= i1 {
                let cell_z1 = (j + 1) as f32 * cell_d; // fVar31
                let cell_x1 = (i + 1) as f32 * cell_w; // fVar32
                let lo_x = if 0 < i { let e = i as f32 * cell_w; if e <= x0 { x0 } else { e } } else { x0 };
                let lo_z = if 0 < j { let e = j as f32 * cell_d; if e <= z0 { z0 } else { e } } else { z0 };
                let hi_x = if i < last && cell_x1 <= x1 { cell_x1 } else { x1 };
                let hi_z = if j < last && cell_z1 <= z1 { cell_z1 } else { z1 };
                let cell = &mut cells[(j as u32 * n + i as u32) as usize];
                if lo_x < cell[0] {
                    cell[0] = lo_x;
                }
                if lo_z < cell[1] {
                    cell[1] = lo_z;
                }
                if cell[2] < hi_x {
                    cell[2] = hi_x;
                }
                if cell[3] < hi_z {
                    cell[3] = hi_z;
                }
                i += 1;
            }
            j += 1;
        }
    }
    let non_empty = |c: &[f32; 4]| c[0] <= c[2] && c[1] <= c[3];
    let empty = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    // merge with the right neighbour (asm 140230750..140230856)
    for row in 0..n as usize {
        for col in 0..n as usize {
            let k = row * n as usize + col;
            if !non_empty(&cells[k]) || col + 1 >= n as usize {
                continue;
            }
            let kn = k + 1;
            if !non_empty(&cells[kn]) {
                continue;
            }
            if cells[kn][2] - cells[k][0] > cell_w {
                continue;
            }
            let nb = cells[kn];
            cells[k][2] = nb[2];
            if cells[k][3] <= nb[3] {
                cells[k][3] = nb[3];
            }
            if nb[1] <= cells[k][1] {
                cells[k][1] = nb[1];
            }
            cells[kn] = empty;
        }
    }
    // merge with the neighbour below (asm 14023085c..140230967)
    for row in 0..n as usize {
        for col in 0..n as usize {
            let k = row * n as usize + col;
            if !non_empty(&cells[k]) || row + 1 >= n as usize {
                continue;
            }
            let kn = (row + 1) * n as usize + col;
            if !non_empty(&cells[kn]) {
                continue;
            }
            if cells[kn][3] - cells[k][1] > cell_d {
                continue;
            }
            let nb = cells[kn];
            cells[k][3] = nb[3];
            if cells[k][2] <= nb[2] {
                cells[k][2] = nb[2];
            }
            if nb[0] <= cells[k][0] {
                cells[k][0] = nb[0];
            }
            cells[kn] = empty;
        }
    }
    for c in &cells {
        if non_empty(c) {
            tiles.push(CBox { c: [(c[0] + c[2]) * 0.5, (ymin + ymax) * 0.5, (c[1] + c[3]) * 0.5], h: [(c[2] - c[0]) * 0.5, (ymax - ymin) * 0.5, (c[3] - c[1]) * 0.5] });
        }
    }
    Tiling { size, n, ext, tiles }
}

/// One map item as the tiling needs it: its record (model box × pose) and the source numbers, for printing.
#[derive(Clone, Debug)]
pub struct ItemRecord {
    pub item: usize,
    pub model_name: String,
    pub model_box: Option<CBox>,
    pub iso4: Iso4,
    pub record: Option<BlockRecord>,
}

/// The block records of a map's items: per instance the model box (`model_box` over the stored visual boxes —
/// `lod0_only` restricts it to the LOD-0 shaded geoms, the default takes every geom as the solid's tree holds every
/// visual), the mobil Iso4 (`item_iso4`) and the quality byte (`quality_byte(elem, global)`).
pub fn item_records(scene: &crate::geometry::Scene, global_quality: f32, lod0_only: bool) -> Vec<ItemRecord> {
    scene
        .instances
        .iter()
        .map(|inst| {
            let mdl = &scene.models[inst.model];
            let boxes: Vec<CBox> = if lod0_only {
                mdl.stored_boxes.iter().map(|(c, h)| CBox::new(*c, *h)).collect()
            } else {
                mdl.stored_boxes_all.iter().map(|(_, c, h)| CBox::new(*c, *h)).collect()
            };
            let mb = model_box(&boxes);
            let iso4 = item_iso4(&inst.pose);
            let record = mb.map(|b| block_record(&b, &iso4, quality_byte(inst.lm_quality, global_quality)));
            ItemRecord { item: inst.item, model_name: inst.model_name.clone(), model_box: mb, iso4, record }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(c: [f32; 3], h: [f32; 3]) -> CBox {
        CBox::new(c, h)
    }

    #[test]
    fn union_recentres_per_step_and_copies_into_an_invalid_box() {
        let mut acc = CBox::INVALID;
        acc.union_into(&b([1.0, 2.0, 3.0], [0.5, 0.5, 0.5]));
        assert_eq!(acc, b([1.0, 2.0, 3.0], [0.5, 0.5, 0.5]), "the first box is copied bit for bit");
        acc.union_into(&CBox::INVALID);
        assert_eq!(acc, b([1.0, 2.0, 3.0], [0.5, 0.5, 0.5]), "an invalid box is skipped");
        acc.union_into(&b([4.0, 2.0, 3.0], [1.0, 0.5, 0.5]));
        assert_eq!(acc.min(), [0.5, 1.5, 2.5]);
        assert_eq!(acc.max(), [5.0, 2.5, 3.5]);
        assert_eq!(acc.c, [2.75, 2.0, 3.0]);
    }

    #[test]
    fn transform_is_row_major_with_abs_halves() {
        // a yaw of 90° about y: x → −z, z → x (engine rows: world_x = m0·x + m1·y + m2·z)
        let m: Iso4 = [0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 100.0, 10.0, 200.0];
        let o = b([1.0, 2.0, 3.0], [0.5, 0.25, 0.125]).transformed(&m);
        assert_eq!(o.c, [103.0, 12.0, 199.0]);
        assert_eq!(o.h, [0.125, 0.25, 0.5]);
    }

    #[test]
    fn item_iso4_of_an_unrotated_item_is_the_translation_by_pos_plus_pivot() {
        let m = item_iso4(&ItemPose { yaw: 0.0, pitch: 0.0, roll: 0.0, pos: [864.0, 32.0, 353.0], pivot: [0.0, 0.5, 0.0], scale: 1.0 });
        assert_eq!(&m[0..9], &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        assert_eq!(&m[9..12], &[864.0, 32.5, 353.0]);
        // and from_xform transposes mapgeom's column layout into the engine's rows
        let x: mapgeom::geom::Xform = [0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 5.0, 6.0, 7.0];
        assert_eq!(from_xform(&x), [0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 5.0, 6.0, 7.0]);
    }

    #[test]
    fn quality_bytes_follow_re_2s_table() {
        assert_eq!(quality_byte(0, 1.0), 255);
        assert_eq!(quality_byte(4, 1.0), 180, "Lowest: √2⁻¹·255 = 180.3 → 180");
        assert_eq!(quality_byte(5, 1.0), 127, "VeryLow: 0.5·255 = 127.5 → 127 — below the 0.51 gate");
        assert_eq!(quality_byte(0, 0.5), 127, "a decoration object on a non-Stadium collection");
        assert_eq!(quality_byte(0, 0.0625), 15);
        assert_eq!(quality_byte(3, 1.0), 255, "clamped");
        assert!(in_fitted_tiles(&BlockRecord { world: CBox::INVALID, quality: 180.0 / 255.0 }));
        assert!(!in_fitted_tiles(&BlockRecord { world: CBox::INVALID, quality: 127.0 / 255.0 }));
    }

    /// pwc-day's three items (the model boxes and Iso4s `lmtool tiles` prints for pwc-day-source.Map.Gbx) and its
    /// scene box: the tiling rule gives ONE fitted tile = the records' x/z union with the scene's y range, on a 4×4
    /// grid of 4096² targets — what the capture shows (one fitted peel per direction, 4096² targets).
    fn pwc_day() -> (CBox, Vec<BlockRecord>) {
        let scene = CBox::from_min_max([-1.5258789e-5, 3.9999785, 7.6293945e-6], [2048.0, 95.5, 2048.0]);
        let recs = vec![
            BlockRecord { world: b([863.34534, 34.1102, 358.4614], [2.3353567, 1.7935166, 2.3573089]), quality: 1.0 },
            BlockRecord { world: b([872.0, 32.49999, 361.0], [8.000003, 0.01999998, 8.000001]), quality: 1.0 },
            BlockRecord { world: b([872.0, 63.5, 337.0], [8.0, 32.0, 0.02000068]), quality: 1.0 },
        ];
        (scene, recs)
    }

    #[test]
    fn pwc_day_tiles_once_around_the_items_at_4096() {
        let (scene, recs) = pwc_day();
        let t = peel_tiling(&scene, &recs, &TileParams::pwc_day(31.75));
        assert_eq!(t.size, 4096, "2048 m × 31.75 layout units/m = 65 024 > 3072 with > 1.5 GB");
        assert_eq!(t.n, 4, "ceil(65 024 / 4096) = 16, clamped to LightMapParam+0xb4 = 4");
        assert_eq!(t.tiles.len(), 1, "the three records share cell (1, 0); every other cell is empty and dropped");
        let f = t.tiles[0];
        let (mn, mx) = (f.min(), f.max());
        assert_eq!((mn[0], mx[0]), (861.01, 880.0), "x = the card record's min (861.01 in the capture's back-solve too), the pad's max");
        assert!((mn[2] - 336.98).abs() < 4e-5 && mx[2] == 369.0, "z {:?}..{:?} = the wall's ±0.02 face (337 − 0.02000068 in f32), the pad's far edge", mn, mx);
        assert_eq!(f.c[1], (3.9999785f32 + 95.5) * 0.5, "y = the scene box's range, (ymin + ymax)·0.5");
        assert_eq!(f.h[1], (95.5f32 - 3.9999785) * 0.5);
        // the tile's centre is (xmin + xmax)·0.5 of the clipped record union, not a re-centred Aabb
        assert_eq!(f.c[0], (861.01f32 + 880.0) * 0.5);
    }

    #[test]
    fn a_small_extent_or_a_small_card_means_no_tiles_and_2048() {
        let (scene, recs) = pwc_day();
        // s = 1: ext = 2048 ≤ 3072 → 2048² and n = ceil(2048/2048) = 1 → no fitted tiles
        let t = peel_tiling(&scene, &recs, &TileParams::pwc_day(1.0));
        assert_eq!((t.size, t.n, t.tiles.len()), (2048, 1, 0));
        // ext > 3072 but a 1 GB card: 2048², n = ceil(65 024 / 2048) = 32 → 4 → tiles still
        let t = peel_tiling(&scene, &recs, &TileParams { vram_bytes: 1 << 30, ..TileParams::pwc_day(31.75) });
        assert_eq!((t.size, t.n, t.tiles.len()), (2048, 4, 1));
        // VFast/Fast halve the extent: 32 512 → still 4096, n 8 → 4
        let t = peel_tiling(&scene, &recs, &TileParams { quality: 1, ..TileParams::pwc_day(31.75) });
        assert_eq!((t.size, t.n), (4096, 4));
        // a decoration-quality record (0.498) never contributes
        let mut deco = recs.clone();
        for r in &mut deco {
            r.quality = 127.0 / 255.0;
        }
        let t = peel_tiling(&scene, &deco, &TileParams::pwc_day(31.75));
        assert_eq!(t.tiles.len(), 0, "cells with no record are dropped");
        // the exact 1536 MB boundary: 1536 MB is NOT enough (> 0x600 MB)
        let t = peel_tiling(&scene, &recs, &TileParams { vram_bytes: 1536 << 20, ..TileParams::pwc_day(31.75) });
        assert_eq!(t.size, 2048);
        let t = peel_tiling(&scene, &recs, &TileParams { vram_bytes: 1537 << 20, ..TileParams::pwc_day(31.75) });
        assert_eq!(t.size, 4096);
    }

    #[test]
    fn records_are_clipped_to_their_cells_and_neighbours_merge_when_the_pair_fits_a_cell() {
        // a 64 m scene from 0: s such that n = 4 (cells of 16 m); one record spanning x 10..40 in row 0
        let scene = CBox::from_min_max([0.0, 0.0, 0.0], [64.0, 10.0, 64.0]);
        let p = TileParams { alloc_scale: 200.0, quality: 3, half_at_low_quality: true, size_override: 2048, vram_bytes: 8 << 30, max_tiles: 4 };
        let r = |x0: f32, x1: f32, z0: f32, z1: f32| BlockRecord { world: CBox::from_min_max([x0, 0.0, z0], [x1, 10.0, z1]), quality: 1.0 };
        let t = peel_tiling(&scene, &[r(10.0, 40.0, 2.0, 6.0)], &p);
        assert_eq!(t.n, 4);
        // cells 0, 1, 2 of row 0 get [10,16], [16,32], [32,40]; the merges: cell 0 + cell 1 → [10,32] (32 − 10 = 22 > 16: NO)
        // — the merge needs nb.xmax − cell.xmin ≤ cellW, so only pairs that together fit one cell width merge
        let xs: Vec<[f32; 2]> = t.tiles.iter().map(|c| [c.min()[0], c.max()[0]]).collect();
        assert_eq!(xs, vec![[10.0, 16.0], [16.0, 32.0], [32.0, 40.0]]);
        // two thin records in adjacent cells that fit one cell width together DO merge
        let t = peel_tiling(&scene, &[r(14.0, 15.0, 2.0, 6.0), r(17.0, 18.0, 2.0, 6.0)], &p);
        let xs: Vec<[f32; 2]> = t.tiles.iter().map(|c| [c.min()[0], c.max()[0]]).collect();
        assert_eq!(xs, vec![[14.0, 18.0]]);
        // and the same downwards: rows 0 and 1 merge when the pair fits a cell depth
        let t = peel_tiling(&scene, &[r(2.0, 3.0, 14.0, 15.0), r(2.0, 3.0, 17.0, 18.0)], &p);
        let zs: Vec<[f32; 2]> = t.tiles.iter().map(|c| [c.min()[2], c.max()[2]]).collect();
        assert_eq!(zs, vec![[14.0, 18.0]]);
        // the outer cells are unclamped: a record poking out of the scene box keeps its own extent there
        let t = peel_tiling(&scene, &[r(-5.0, 3.0, 1.0, 2.0)], &p);
        assert_eq!(t.tiles[0].min()[0], -5.0);
    }

    #[test]
    fn without_records_the_cells_are_plain_and_measured_from_zero() {
        let scene = CBox::from_min_max([0.0, 0.0, 0.0], [1024.0, 10.0, 1024.0]);
        let p = TileParams { alloc_scale: 1.0, quality: 3, half_at_low_quality: true, size_override: 0, vram_bytes: 8 << 30, max_tiles: 4 };
        let t = peel_tiling(&scene, &[], &p);
        // ext = 1024 · 4.2 = 4300.8 → 4096 (count 0 skips the 3072 test), n = ceil(4300.8/4096) = 2
        assert_eq!((t.size, t.n, t.tiles.len()), (4096, 2, 4));
        assert_eq!(t.tiles[0], CBox { c: [256.0, 48.0, 256.0], h: [256.0, 80.0, 256.0] });
        assert_eq!(t.tiles[3], CBox { c: [768.0, 48.0, 768.0], h: [256.0, 80.0, 256.0] });
    }

    #[test]
    fn scene_box_is_the_fold_of_the_records_and_the_world_box_adds_the_probe_chunks() {
        let (_, recs) = pwc_day();
        let s = scene_box(&recs);
        assert!((s.min()[0] - 861.01).abs() < 1e-4 && (s.max()[1] - 95.5).abs() < 1e-5);
        let w = world_peel_box(&s, Some(&CBox::from_min_max([464.0, -54.0, -16.0], [976.0, 202.0, 496.0])));
        assert_eq!(w.max()[1], 202.0);
        assert_eq!(world_peel_box(&s, None), s);
    }

    #[test]
    fn model_box_copies_a_single_visual_and_unions_the_rest_in_order() {
        let v = b([-0.0573, 1.7762, 0.0299], [1.7676, 1.7935, 1.797]);
        assert_eq!(model_box(&[v]), Some(v), "one visual: the stored box itself, no re-centring");
        assert_eq!(model_box(&[CBox::INVALID, v]), Some(v));
        assert_eq!(model_box(&[]), None);
        let two = model_box(&[b([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]), b([3.0, 0.0, 0.0], [1.0, 1.0, 1.0])]).unwrap();
        assert_eq!((two.c[0], two.h[0]), (1.5, 2.5));
    }
}

/// The zone tiles' block records of a map the way the game builds them (`lmtool tile-boxes`; RE child 6): per cell
/// the genealogy's CurrentZoneId + Dir (or `zone_default` on every cell of a map without a genealogy — the capture's
/// 4096 Sea tiles), the zone block info's ground variant → its mobil's prefab → entity 0's solid → the stored visual
/// boxes in geom order → `model_box`; the block Iso4 = the Dir rotation (Ry(dir·90°)) about the unit's centre
/// (16, ·, 16), translation (cx·32, cell_y·8 + yoff, cz·32) — BlueBay: cell_y 5, yoff −40 → the tiles at y 0
/// (the sea quad at 7.0, the seabed at 3.9999785 as the capture has it); quality = the decoration's 0.5 → byte 127.
/// Returns (cell x, cell z, zone, dir, record) in cell order (z rows, x columns = the genealogy's x·64 + z order
/// transposed — the game's bind order for generated tiles is its own BakedBlocks order; the fold is order-independent
/// up to the last bit).
pub fn tile_records(store: &mut mapgeom::store::DataStore, collection: &str, size: [usize; 3], genealogy: &[(String, u32)], zone_default: &str, cell_y: f32, yoff: f32, global_quality: f32) -> Result<Vec<(usize, usize, String, u32, BlockRecord)>, String> {
    let (sx, sz) = (size[0], size[2]);
    let cells: Vec<(usize, usize, String, u32)> = if genealogy.len() == sx * sz {
        (0..genealogy.len()).map(|i| (i / sz, i % sz, genealogy[i].0.clone(), genealogy[i].1)).collect()
    } else {
        let mut v = Vec::new();
        for cz in 0..sz {
            for cx in 0..sx {
                v.push((cx, cz, zone_default.to_string(), 0u32));
            }
        }
        v
    };
    let mut zone_boxes: std::collections::BTreeMap<String, Option<CBox>> = Default::default();
    let mut out = Vec::with_capacity(cells.len());
    for (cx, cz, zone, dir) in cells {
        if !zone_boxes.contains_key(&zone) {
            let mut mb: Option<CBox> = None;
            for (fam, ext) in [("GameCtnBlockInfoFlat", "EDFlat"), ("GameCtnBlockInfoFrontier", "EDFrontier"), ("GameCtnBlockInfoTransition", "EDTransition"), ("GameCtnBlockInfoClassic", "EDClassic")] {
                let path = format!("{collection}\\GameCtnBlockInfo\\{fam}\\{zone}.{ext}.Gbx");
                let Ok(bi) = mapgeom::blockinfo::load(store, &path) else { continue };
                let Some(v) = bi.variant_base_ground.as_ref() else { continue };
                let Some(pp) = v.mobils.iter().flatten().find_map(|m| m.prefab.clone()) else { continue };
                let pm = store.load_model(&pp)?;
                let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
                let mut boxes = Vec::new();
                for e in &pf.ents {
                    let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
                    let Some(s2) = so.solid2() else { continue };
                    for sg in &s2.shaded_geoms {
                        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
                        let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
                        if let Some(mm) = vis.main.as_ref() {
                            let b = mm.bounding_box;
                            boxes.push(CBox::new([b[0], b[1], b[2]], [b[3], b[4], b[5]]));
                        }
                    }
                    break;
                }
                mb = model_box(&boxes);
                break;
            }
            zone_boxes.insert(zone.clone(), mb);
        }
        let Some(mb) = zone_boxes[&zone] else { continue };
        let (s, c) = match dir % 4 {
            0 => (0.0f32, 1.0f32),
            1 => (1.0, 0.0),
            2 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        };
        let (px, pz) = (16.0f32, 16.0f32);
        let tx = cx as f32 * 32.0 + (px - (c * px + s * pz));
        let tz = cz as f32 * 32.0 + (pz - (-s * px + c * pz));
        let iso: Iso4 = [c, 0.0, s, 0.0, 1.0, 0.0, -s, 0.0, c, tx, cell_y * 8.0 + yoff, tz];
        out.push((cx, cz, zone, dir, block_record(&mb, &iso, quality_byte(0, global_quality))));
    }
    Ok(out)
}
