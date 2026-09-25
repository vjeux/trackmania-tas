//! THE ITEM CHART RULE — which placed items get a chart record in the editor's lightmap allocation,
//! the per-record quality and extent, and the multi-sub-visual two-record case
//! (RE child 7, 2026-09-25, typed decompiles of Trackmania.exe Aug 2025; NOTES.md 11:xxZ block).
//!
//! Three record builders append to the ONE record array (CHmsLightMap+0xa8/+0xb0, 0x58-byte records) that
//! `AllocateBlocks_` packs and `SHmsLightMapCacheMapping` (FUN_140288530) writes to the file — every record,
//! in radix-key order; a placed thing without a record is neither packed nor in the chart table:
//!
//! * kind 2 — a STATIC item (CPlugStaticObjectModel → the zone's static pool, FUN_1401e9a50 → FUN_14020e7b0):
//!   record iff the Solid2Model's PreLightGen exists, its `u01` (the file's i32, byte PLG+0x50) ≠ 0, the uv-set-0
//!   bounds are non-degenerate (u0 < u1 AND v0 < v1) and the model's lightmap geometry (FUN_14045b730: the
//!   LOD-selected indexed-triangle visuals that carry the material's lightmap TEXCOORD set, positions ≤ uvs)
//!   is non-empty — else "Mesh has invalid LightMap texture coordinates, lighting may be wrong." and NO record.
//!   Also skipped: a model entry flagged bit 1 = a material with CPlugMaterial+0x18 & 0x6000 while the
//!   BSS knob DAT_14205c82c is on (the "CustomLM" shader define; no game material file carries those bits
//!   — every checked Stadium material's chunk 0x09079016 flags = 0x240).
//!   The record's quality (+0x50) is the pool entry's FLOAT (entry+0x34) — `quality_float`, no byte.
//! * kind 0 — a CHmsItem mobil (a CPlugSolid: legacy items, dyna/moving solids; the Solid2Model → CPlugSolid
//!   conversion FUN_1401fcc30 copies the PreLightGen whole (FUN_1401fb700)): record iff the CPlugSolid's
//!   PreLightGen exists and its u01 byte ≠ 0 (alloc mode 1 reads the byte directly; other modes through
//!   FUN_14020d390). Quality = `quality_byte(f) / 255` (FUN_14020e3c0 / FUN_14021d8e0: `(float)byte / 255.0`).
//! * kind 1 — the CHmsZone+0x1f8 manager's entries (FUN_14020eea0): PreLightGen with u01 ≠ 0 → a record with
//!   quality 1.0, flags 9, centre (0,0,0) / half (−1,−1,−1); a PreLightGen whose stored sprite size w·h ≠ 0
//!   only gets a caster record (the +0xb8 array, never packed).
//!
//! Extent per record (FUN_1402917e0 + GetPreLightGenMeterByUv 0x14028f480): `f = MeterByUv × blockScale`
//! (blockScale = BlockInfo float × quality for blocks, quality × 1 for items), `ext = (Δu·f, Δv·f)` on the
//! uv-set-0 bounds (set 1 when the alloc mode is 1), `area = ext.y·ext.x`; a non-finite f → (0, 0) (the packer
//! then gives `m·mins`); `f < 0` → f = 0.1 with the warning "GetPreLightGenMeterByUv() < 0  !". No placement
//! scale enters (the PreLightGen is per model).
//!
//! Two records (FUN_140291450): a PreLightGen with ≥ 2 `uvGroups` (in the file: count × 20 bytes
//! `{f32 MeterByUv, u0, v0, u1, v1}` — FUN_140283cc0/FUN_140283d60, stride 0x14) gets `{flags 0x10000 | sub 0}`
//! = sub-visual 0 with ITS OWN group f and rect, and `{flags 0x20000, group g}` = sub-visuals 1..n−1 merged:
//! f = the average of the finite group f's (FUN_14028f1d0), bounds = the union of the sub rects placed by the
//! uv rect packer FUN_141402b00 (seed 1; NOT transcribed — no tiny item has uvGroups). n < 2 → one record.
//!
//! Quality (CGameCtnApp::HmsLightMapUpdateBlocksAndItemsQuality 0x140dcc290 → FUN_140dccde0 per item):
//! `f = powf(√2, e) · G`, e = {0: 0, 1: +1, 2: +2, 3: +3, 4: −1, 5: −2, 6: −3, else 0} of the item's
//! MapElemLightmapQuality byte (chunk 0x03043068), G = 1.0 for the map's own objects (0.0625 / 0.5 for a
//! decoration's, RE 2/6). A static-pool item: FUN_1401eab80 stores f as the entry FLOAT; a CHmsItem:
//! `clamp(int(f·255), 1, 255)` into CHmsItem+0x38.

/// The PreLightGen of a `CPlugSolid2Model` (chunk 0x090BB000 v ≥ 3; 0x58 bytes in memory, ctor 0x1404e5e00,
/// archive 0x1404e5e70). Field names follow the repo's reader (`mapgeom::static_item::solid2::PreLightGen`).
#[derive(Clone, Debug, PartialEq)]
pub struct PreLightGen {
    /// The file's i32 `u01`, kept as a byte at PLG+0x50: 0 = the model takes no lightmap chart.
    pub u01: i32,
    /// `u02` = MeterByUv (metres per uv unit of the lightmap set), PLG+0.
    pub meter_by_uv: f32,
    /// `u04[0..4]` = uv-set-0 bounds (u0, v0, u1, v1), PLG+4..+0x10.
    pub uv0: [f32; 4],
    /// `u04[4..8]` = uv-set-1 bounds, PLG+0x14..+0x20 (alloc mode 1 only).
    pub uv1: [f32; 4],
    /// `spriteCount` (i32 pair at PLG+0x24): a stored chart size for the sprite/billboard models.
    pub sprite_count: [i32; 2],
    /// `uvGroups`: one `{MeterByUv, u0, v0, u1, v1}` per sub-visual chart group (PLG+0x40, count +0x48).
    pub uv_groups: Vec<[f32; 5]>,
}

/// Why a placed static item has no chart record (FUN_14020e7b0's tests in the game's order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoChart {
    /// The Solid2Model has no PreLightGen (`visCstType` ≠ 1 models).
    NoPreLightGen,
    /// PLG+0x50 (the file's `u01`) is 0.
    U01Zero,
    /// uv-set-0 bounds with u0 ≥ u1 or v0 ≥ v1 (the ctor's empty box: FLT_MAX / −FLT_MAX).
    DegenerateUv0,
    /// No LOD-selected indexed-triangle visual carries the material's lightmap TEXCOORD set (positions ≤ uvs),
    /// or the item's lightmap uvs are not finite: "Mesh has invalid LightMap texture coordinates".
    NoLightmapUvGeometry,
    /// The zone's model entry has flag bit 1: a material with CPlugMaterial+0x18 & 0x6000 ("CustomLM") while
    /// DAT_14205c82c is on.
    CustomLmMaterial,
}

/// One LOD-selected shaded geom of the model as the lightmap-geometry builder sees it (FUN_14045b730 →
/// FUN_14045ad70).
#[derive(Clone, Copy, Debug)]
pub struct LmGeom {
    /// The shaded geom's `lodMask & lodBit` ≠ 0 (lodBit = 1, or 2 when the model has ≥ 1 lodMaxDist and its
    /// second per-LOD word is non-zero — see the module notes; every tiny item's LOD-0 and LOD-1 visuals
    /// carry the same uv sets, so the bit does not change the verdict there).
    pub lod_selected: bool,
    /// `CPlugVisual` kind 2 = `CPlugVisualIndexedTriangles` (vtbl+0xf8 → 2); other kinds add nothing.
    pub indexed_triangles: bool,
    /// The material's lightmap texcoord index (compiled CPlugMaterial+0x40 bits 15..19, sign-extended; the
    /// uncompiled material's +0x34); −1 = the material takes no lightmap → the geom adds nothing.
    pub material_lm_set: i32,
    /// Vertex count of the POSITION element.
    pub positions: u32,
    /// Vertex count of the TEXCOORD`material_lm_set` element (0 when the visual has no such set).
    pub lm_uvs: u32,
}

/// FUN_14045ad70's test per geom: a block is appended iff the visual is indexed triangles, the material has
/// a lightmap set, and `positions != 0 && positions <= lm_uvs`. The model's lightmap geometry is non-empty
/// iff any LOD-selected geom passes — the stats (+0x18 vertices, +0x28 triangles) that FUN_14020e7b0 tests.
pub fn lm_geometry_nonempty<'a>(geoms: impl IntoIterator<Item = &'a LmGeom>) -> bool {
    geoms.into_iter().any(|g| g.lod_selected && g.indexed_triangles && g.material_lm_set != -1 && g.positions != 0 && g.positions <= g.lm_uvs)
}

/// FUN_14020e7b0 (kind 2): the record filter for a static item, in the game's order.
pub fn static_item_record(plg: Option<&PreLightGen>, lm_geometry_nonempty: bool, custom_lm_material: bool) -> Result<(), NoChart> {
    // FUN_1401e9a50: the model entry is flagged before the record is attempted; FUN_1402008b0 marks "none"
    if custom_lm_material {
        return Err(NoChart::CustomLmMaterial);
    }
    let Some(p) = plg else { return Err(NoChart::NoPreLightGen) };
    // `*(char *)(lVar2 + 0x50) == '\0'`
    if (p.u01 & 0xff) == 0 {
        return Err(NoChart::U01Zero);
    }
    // `u1 <= u0 && u0 != u1` (i.e. u1 < u0 in the decompiler's rendering of `!(u0 < u1)` with NaN care) — a
    // degenerate or inverted set-0 box; the ctor's empty box is (FLT_MAX, FLT_MAX, −FLT_MAX, −FLT_MAX)
    if !(p.uv0[0] < p.uv0[2]) || !(p.uv0[1] < p.uv0[3]) {
        return Err(NoChart::DegenerateUv0);
    }
    if !lm_geometry_nonempty {
        return Err(NoChart::NoLightmapUvGeometry);
    }
    Ok(())
}

/// FUN_14020e3c0 (kind 0, a CHmsItem's CPlugSolid): record iff the solid's PreLightGen exists and its u01
/// byte ≠ 0 (the conversion from a Solid2Model copies the PreLightGen whole, FUN_1401fb700).
pub fn mobil_item_record(plg: Option<&PreLightGen>) -> Result<(), NoChart> {
    let Some(p) = plg else { return Err(NoChart::NoPreLightGen) };
    if (p.u01 & 0xff) == 0 {
        return Err(NoChart::U01Zero);
    }
    Ok(())
}

/// FUN_140dcc160: the MapElemLightmapQuality byte → the √2 exponent.
pub fn quality_exponent(elem_quality: u8) -> i32 {
    match elem_quality {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => -1,
        5 => -2,
        6 => -3,
        _ => 0,
    }
}

/// FUN_140dcc1c0: `powf(1.4142135f, (float)e) * G` (CRT powf, then one f32 multiply). For the seven exponents
/// the CRT result is the correctly rounded power (computed here in f64 and rounded once).
pub fn quality_float(elem_quality: u8, g: f32) -> f32 {
    let e = quality_exponent(elem_quality);
    let p = (f32::from_bits(0x3fb504f3) as f64).powi(e) as f32;
    p * g
}

/// The byte a CHmsItem record carries (FUN_140dccde0 / FUN_140dcc8e0): `i = (int)(f * 255.0f)`; `i < 2 → 1`;
/// `i > 254 → 255`; else `i`. The record's +0x50 is then `byte as f32 / 255.0`.
pub fn quality_byte(f: f32) -> u8 {
    let i = (f * 255.0f32) as i32;
    if i < 2 {
        1
    } else if i > 0xfe {
        0xff
    } else {
        i as u8
    }
}

/// A CHmsItem record's quality float from its byte (`(float)bVar1 / 255.0`).
pub fn quality_from_byte(b: u8) -> f32 {
    b as f32 / 255.0f32
}

/// GetPreLightGenMeterByUv (0x14028f480) for a plain record: `f = MeterByUv × blockScale`, `f < 0 → 0.1`
/// (+ the warning). `alloc_mode == 1` reads the uv-set-1 bounds, every other mode set 0.
pub fn meter_by_uv(plg: &PreLightGen, block_scale: f32, alloc_mode: u32) -> (f32, [f32; 4]) {
    let bounds = if alloc_mode == 1 { plg.uv1 } else { plg.uv0 };
    let f = plg.meter_by_uv * block_scale;
    if f < 0.0 {
        return (0.1, bounds);
    }
    (f, bounds)
}

/// FUN_1402917e0 per chart: `ext = ((u1−u0)·f, (v1−v0)·f)` in f32, `(0, 0)` when f is not finite; the area the
/// game sums into TotalLmSurfaceMeter is `ext.y * ext.x`.
pub fn chart_ext(plg: &PreLightGen, block_scale: f32, alloc_mode: u32) -> [f32; 2] {
    let (f, b) = meter_by_uv(plg, block_scale, alloc_mode);
    if !f.is_finite() {
        return [0.0, 0.0];
    }
    [(b[2] - b[0]) * f, (b[3] - b[1]) * f]
}

/// The chart records a PreLightGen yields (FUN_140291450): one for < 2 uvGroups, else sub-visual 0 on its own
/// and the merged group of sub-visuals 1..n−1.
#[derive(Clone, Debug, PartialEq)]
pub enum ChartRecord {
    /// flags 0, group −1: the model's uv-set bounds and MeterByUv.
    Whole,
    /// flags 0x10000 | sub: the sub-visual's own `uvGroups[sub]` = {f, u0, v0, u1, v1}.
    SubVisual { sub: u16, f: f32, rect: [f32; 4] },
    /// flags 0x20000, group g: the average of the finite f's of the other sub-visuals (FUN_14028f1d0); the
    /// merged bounds come from the uv rect packer FUN_141402b00 (not transcribed).
    MergedGroup { avg_f: f32, subs: Vec<u16> },
}

/// FUN_14028f1d0's average: over the sub-visuals ≠ `excluded` whose f is finite, `Σf / n` (f32 running sum,
/// one division).
pub fn merged_group_avg_f(plg: &PreLightGen, excluded: u16) -> f32 {
    let mut sum = 0f32;
    let mut n = 0u32;
    for (i, g) in plg.uv_groups.iter().enumerate() {
        if i as u16 != excluded && g[0].is_finite() {
            sum += g[0];
            n += 1;
        }
    }
    sum / n as f32
}

pub fn chart_records(plg: &PreLightGen) -> Vec<ChartRecord> {
    let n = plg.uv_groups.len();
    if n < 2 {
        return vec![ChartRecord::Whole];
    }
    let g0 = plg.uv_groups[0];
    vec![
        ChartRecord::SubVisual { sub: 0, f: g0[0], rect: [g0[1], g0[2], g0[3], g0[4]] },
        ChartRecord::MergedGroup { avg_f: merged_group_avg_f(plg, 0), subs: (1..n as u16).collect() },
    ]
}


// ─────────────────────────────────────────────────────────────────────────────────────────────────────
// THE MODEL GRIDS (RE 7, 2026-09-25 14:00Z): with no multi-sub-visual chart in the map (AllocateWithScale_
// state+0x138 = 1 — every reference bake), the layout does NOT pack one chart per record. FUN_1402938c0 groups
// the records by (PreLightGen pointer, blockparam = quality × BlockInfo scale) into ONE chart per group: a grid
// of nb × na cells (FUN_140293d70), packed as one rect of extent (nb·ext.x, na·ext.y) with the minimum size
// m·(nb, na) (TryPack's `mins`), area (nb·na)·ext.y·ext.x (FUN_140294220) — the sum of those areas is the
// TotalLmSurfaceMeter of the file, and the grid SLACK (nb·na − n empty cells) is the area no record owns.
// A record whose own extent × D₁ exceeds 100 on either side is a group of its own (D₁ = W·H / Σ of the
// per-record areas, the first density). Each instance then gets one cell: the k-th cell in Z-ORDER
// (FUN_140291b10: deinterleave i = 0..M², M = 2^ceil(log2 max(nb,na)), keep x < nb, y < na) by its ordinal in
// the group (order of appearance in the record array), cell edges spread over the placed rect with an
// error-diffused remainder in steps of g (FUN_140291c80), 2·pad gutter per cell (FUN_140291f20).

/// FUN_140293d70's grid for a group of `n` instances whose per-instance extent is `ext` (metres), at the first
/// density `d1` (texel²/m²) with the minimum chart size `m`: returns (nb, na) = (columns along x, rows along y).
/// Transcribed: X = max(m, ext.x·d1), Y = max(m, ext.y·d1), r = X/Y; na₀ = iround(√(n·r)), nb₀ = iround(√(n/r))
/// (CRT lroundf: half away from zero), floored at 1; while nb·na < n: with rem = n − nb·na, if (nb+1 < rem &&
/// na+1 < rem) grow nb when (float)(nb / na) [integer division] ≤ 1/r else na, otherwise grow nb when nb ≤ na
/// else na; then trim: while (nb−1)·na ≥ n → nb−−; else while (na−1)·nb ≥ n → na−−.
pub fn grid_dims(n: u32, ext: [f32; 2], d1: f32, m: u32) -> (u32, u32) {
    if n == 0 {
        return (1, 1);
    }
    let mf = m as f32;
    let x = mf.max(ext[0] * d1);
    let y = mf.max(ext[1] * d1);
    let r = x / y;
    let inv = 1.0f32 / r;
    let iround = |v: f32| -> u32 {
        if !v.is_finite() || v == 0.0 {
            return 0;
        }
        v.round() as u32 // roundf: half away from zero
    };
    let nf = n as f32;
    let a = iround((nf * r).sqrt());
    let b = iround((nf / r).sqrt());
    let mut na = a.max(1);
    let mut nb = b.max(1);
    if na * nb < n {
        let mut na1 = na + 1;
        let mut nb1 = nb + 1;
        while na * nb < n {
            let rem = n - na * nb;
            let take_b = if nb1 < rem && na1 < rem { ((nb / na) as f32) <= inv } else { nb <= na };
            if take_b {
                nb += 1;
                nb1 += 1;
            } else {
                na += 1;
                na1 += 1;
            }
        }
    }
    loop {
        if (nb - 1) * na >= n {
            nb -= 1;
            continue;
        }
        if (na - 1) * nb >= n {
            na -= 1;
            continue;
        }
        break;
    }
    (nb, na)
}

/// FUN_140294220: the group chart's extent and area from the grid — `ext' = (nb·ext.x, na·ext.y)`,
/// `area' = ((float)(nb·na) · ext.y) · ext.x` (f32, in this order).
pub fn grid_chart(ext: [f32; 2], nb: u32, na: u32) -> ([f32; 2], f32) {
    ([nb as f32 * ext[0], na as f32 * ext[1]], ((nb * na) as f32 * ext[1]) * ext[0])
}

/// FUN_1402938c0's solo test: a record whose extent × D₁ exceeds 100 is not grouped. The asm (0x140293ac4: two `ja`
/// to the solo label) reads as EITHER side; the stpad dump says BOTH (the 152 + 60 records of ext (32, 8) — 163 × 41
/// texels — are grouped into grids, and only the AND count (735 entries) gives the observed chunk size 8 and Σ):
/// `is_solo` follows the data (AND); tiny 16 cannot tell the two apart (no side over 339 m).
pub fn is_solo(ext: [f32; 2], d1: f32) -> bool {
    100.0 < ext[0] * d1 && 100.0 < ext[1] * d1
}

/// FUN_140291b10: the Z-order cell list of an nb × na grid — the k-th instance of the group takes `cells[k]`
/// = (column, row); x from the even bits, y from the odd bits of i = 0..M², M = 2^ceil(log2(max(nb, na))).
pub fn zorder_cells(nb: u32, na: u32) -> Vec<(u32, u32)> {
    let mx = nb.max(na).max(1);
    let mut m = 1u32;
    while m < mx {
        m <<= 1;
    }
    let mut out = Vec::with_capacity((nb * na) as usize);
    for i in 0..m * m {
        let (mut x, mut y) = (0u32, 0u32);
        for b in 0..16 {
            x |= ((i >> (2 * b)) & 1) << b;
            y |= ((i >> (2 * b + 1)) & 1) << b;
        }
        if x < nb && y < na {
            out.push((x, y));
        }
    }
    out
}

/// FUN_140291c80 (partial read — the exact f32 accumulation is not pinned): the cell boundaries along one axis —
/// `w` placed units spread over `n` cells in steps of `g`: base = floor((w / n) / g)·g per cell, the remainder
/// `w/n − base` diffused (a cell gets +g when the running remainder reaches g). Returns the n+1 edge offsets.
pub fn cell_edges(w: u32, n: u32, g: u32) -> Vec<u32> {
    let per = w as f32 / n as f32;
    let base = (per as u32) - (per as u32) % g;
    let frac = per - base as f32;
    let mut edges = vec![0u32];
    let mut acc = 0f32;
    let mut pos = 0u32;
    for i in 0..n {
        // THE LAST CELL TAKES THE REMAINDER (E, tiny 16's editor table: w 34 / 3 → [10, 12, 12], w 46 / 3 → [14, 16, 16],
        // h 154 / 3 → [50, 52, 52], w 50 / 3 → [16, 16, 18]; the diffused f32 accumulation alone leaves the last cell short
        // by g when the remainders sum to 1.9999998)
        if i + 1 == n {
            edges.push(w);
            break;
        }
        acc += frac;
        let mut c = base;
        if acc >= g as f32 {
            acc -= g as f32;
            c += g;
        }
        pos += c;
        edges.push(pos);
    }
    edges
}


// ─────────────────────────────────────────────────────────────────────────────────────────────────────
// THE CHUNKS (RE 7, 2026-09-25 14:25Z — the last piece of the Σ term, verified 55/55 groups and 731/731 cells on the
// baker's tiny-16 record dump). After the grouping, FUN_1402938c0's tail runs FUN_140292a50 then FUN_140292740:
// * FUN_140292a50 re-assigns the ordinals inside every group by a 3-D MORTON sort of the rounded record centres
//   (FUN_141401210: per coordinate lroundf(max(0, c)); key bits per level = (z, y, x) with x least significant; a
//   stable LSD radix sort in three 10-bit passes): the record at group position i (record order) receives
//   ordinal = perm[i], perm[i] = the group position of the i-th smallest key (the sort's output list, stored
//   positionally — NOT the rank).
// * FUN_140292740 splits every group into CHUNKS of c instances: k = (int)ceilf(1000 / n_models) (no split when
//   k ≤ 1), c = FUN_1402926c0(trunc(n_records / (k·n_models))) with FUN_1402926c0(x) = max(1, x) then, for x < 12,
//   x → x+1 when x equals 3, 5, 7 or 11 (checked in that order). A group with count > c becomes ceil(count/c)
//   model entries: entry 0 keeps the group's place in the model list, the others are appended at the END of the
//   list (for each model in list order); a record goes to entry ordinal / c with cell ordinal ordinal % c.
//   tiny 16: 458 groups → k = 3, c = trunc(12214 / 1374) = 8 → the file's blocks of 8 (3×3 grids, one empty cell).
// The model list then goes through grid_dims / grid_chart, and Σ = TotalLmSurfaceMeter is the f32 running sum of
// the entries' area' in ascending-area order (BlockSplit sums along the radix order).

/// FUN_1402926c0: max(1, x), then for x < 12 bump 3 → 4, 5 → 6, 7 → 8, 11 → 12 (sequential compares).
pub fn avoid_bad_chunk(x: u32) -> u32 {
    let mut v = x.max(1);
    if x < 12 {
        for t in [3u32, 5, 7, 11] {
            if v == t {
                v += 1;
            }
        }
    }
    v
}

/// FUN_140292740's chunk size: `Some(c)` when the split runs (k > 1), `None` when the map has so many models that
/// k = ceil(1000 / n_models) ≤ 1 (≥ 1000 groups).
pub fn chunk_size(n_records: u32, n_models: u32) -> Option<u32> {
    if n_models == 0 {
        return None;
    }
    let k = (1000.0f32 / n_models as f32).ceil() as i32;
    if k <= 1 {
        return None;
    }
    let x = (n_records as f32 / (k as u32 * n_models) as f32) as u32;
    Some(avoid_bad_chunk(x))
}

/// FUN_141401210's key of a rounded centre: x in bit 0, y in bit 1, z in bit 2 of every 3-bit level (30 bits per
/// coordinate; the three 10-bit radix passes are equivalent to one stable sort on this key).
pub fn morton3(c: [f32; 3]) -> u128 {
    let r = |v: f32| -> u32 { (v.max(0.0) + 0.5).floor() as u32 };
    let (x, y, z) = (r(c[0]), r(c[1]), r(c[2]));
    let mut m = 0u128;
    for b in 0..30 {
        m |= (((x >> b) & 1) as u128) << (3 * b);
        m |= (((y >> b) & 1) as u128) << (3 * b + 1);
        m |= (((z >> b) & 1) as u128) << (3 * b + 2);
    }
    m
}

/// FUN_140292a50: the ordinals of one group. `centres` in record order; returns `ordinal[i]` for the record at
/// group position i: the group position of the i-th smallest Morton key (ties keep record order).
pub fn group_ordinals(centres: &[[f32; 3]]) -> Vec<u32> {
    let mut perm: Vec<usize> = (0..centres.len()).collect();
    perm.sort_by_key(|&i| (morton3(centres[i]), i));
    perm.iter().map(|&p| p as u32).collect()
}

/// One chart entry of the model list after the chunk split.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelChart {
    /// Index of the (PreLightGen, blockparam) group this entry belongs to (order of first appearance).
    pub group: usize,
    /// Chunk number within the group (0 = the original entry, ≥ 1 = appended entries).
    pub chunk: u32,
    /// Instances in this entry.
    pub count: u32,
}

/// FUN_140292740 applied to a model list: `counts[g]` instances per group (first-appearance order). Returns the
/// entries in the game's model-list order (originals, then for each group in order its appended chunks) and, per
/// group, the ordinal → (entry index, cell ordinal) mapping through `chunk`.
pub fn split_chunks(counts: &[u32], chunk: Option<u32>) -> Vec<ModelChart> {
    let mut out: Vec<ModelChart> = counts.iter().enumerate().map(|(g, &n)| ModelChart { group: g, chunk: 0, count: n }).collect();
    let Some(c) = chunk else { return out };
    for (g, &n) in counts.iter().enumerate() {
        if n > c {
            let parts = (n - 1) / c + 1;
            out[g].count = c;
            for k in 1..parts {
                out.push(ModelChart { group: g, chunk: k, count: if k + 1 == parts { n - c * (parts - 1) } else { c } });
            }
        }
    }
    out
}


// ─────────────────────────────────────────────────────────────────────────────────────────────────────
// THE GROUPED PACK ORDER (RE 7, 2026-09-25 15:00Z). BlockSplit (0x1402954f0) receives AllocateBlocks_'s radix sorter
// (state+0x18, passed down as AllocateWithScale_'s 7th argument → BlockSplit's 11th; asm 0x140290da9 / 0x140294e30 /
// 0x1402955ec) and sorts it by the chart areas with one more stable LSD pass (FUN_14012c850). For the per-record
// path the sorter still holds the (|h|², x, y, z) passes over the records → the 5-key order E has (area, z, y, x,
// |h|², index). For the GROUPED path the key array is the model list (count = entries ≠ records): FUN_14012c850 sees
// the count change and RESETS the index array to the identity (FUN_14012cce0: `for i < cap: idx[i] = i`) before the
// area pass → the grouped order is (area' f32 bits ascending, then MODEL-LIST INDEX) — no centre, no |h|², no record
// key enters. The model-list index = order of first appearance of the (PreLightGen, blockparam) key in record order
// (solo entries at their record's position), then the appended chunk entries (for each original in list order, its
// chunks 1..k−1). Only when nModels == nRecords (no group of ≥ 2) does the old (z, y, x, |h|²) order survive as the
// tie-break. TryPack walks the order from the largest down (`order[count − 1 − i]`) and the mins are m·(nb, na).

/// The grouped path's radix order: ascending area' (by f32 bits), ties by model-list index. Returns the permutation
/// (index list) TryPack walks backwards.
pub fn grouped_pack_order(areas: &[f32]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..areas.len()).collect();
    idx.sort_by_key(|&i| (areas[i].to_bits(), i));
    idx
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────────
// THE STADIUM DECORATION'S RECORDS (RE 7, 2026-09-25 15:30Z — CORRECTED by the baker's /lmrecords dump of stpad,
// passcap/stpad-records/: 12 141 records, ALL kind 2, 40 model pointers, 49 (PLG, q) groups). The decoration meshes
// (Stade4096/Stade1536/NoStadium prefabs of the decoration map) produce NO record on stpad, and there are NO kind-0
// records (the Grass.EDFlat old Solid is not charted). What the "≈8 500 decoration charts" are:
// * 9 216 = 96 × 96 terrain TILES of the "Grass" zone (model 32.0641 m MeterByUv, uv0 [0.001 0.001 0.999 0.999], one
//   record per cell over the whole 3072-m map at y 8.125), FLOAT q by the ring rule (0.0442 × 7 073, 1.0 × 8, 0.707 ×
//   319, 0.5 × 240, 0.354 × 274, 0.25 × 280, 0.177 × 270, 0.125 × 272, 0.0884 × 246, 0.0625 × 234) — E's BlueBay tile
//   rule, Stadium tile prefab.
// * 2 925 records of the map's 180 WaterBase blocks: EVERY prefab entity with a PreLightGen of every mobil the block
//   instantiates — its own prefab (Water\Base_Air entity 0 → 1 record) AND its CLIPS' prefabs (WaterFCCenter →
//   FCCenter_Air = the 32×8 border mesh (MeterByUv 32.1998 uv0 [0.0224 0.0142 0.5535 0.2601]) + 2 nested TreeGen\
//   RoadBorderSpot (11.7644, [0.0075 0.0079 0.1204 0.1191], with a light) + 2 nested TreeGen\BarrierSupport (7.6495,
//   [0.0085 0.0065 0.1938 0.1317]) → 5 records under the block's object id (key sub 0..4); the HFC left/right clips →
//   3 records) — nested external prefab entities are flattened into the static pool with the parent's object id and
//   a running sub index. q = 1.0 (the map's own blocks; the decoration G would apply only to the decoration map's
//   blocks, which produce nothing here). 40 distinct model pointers = one Solid2Model CLONE per (prefab entity, block
//   variant) — the same mesh reached through two clips is two clones with two PLG pointers (e.g. BarrierSupport
//   912 + 360), so the (PLG, q) grouping splits them.
// * stpad has NO multi-sub-visual chart (uvGroups = 0 everywhere) → the GROUPED path runs there too (state+0x138 = 1);
//   the file's 3-/5-chart objects are these per-entity records, not two-record items.
// Σ check on the dump: per-record Σ₁ = 820 854.3 (f32); grouped with the 2048² D₁ (946 model entries: 916 solos,
// 30 groups) and chunks of c = 8 → Σ = 843 229.5 vs the file's 843 235.75 (f32 order) — the grid/chunk rule holds,
// BUT `chunk_size(12141, 946)` gives 6 (k = 2, 12141/1892 = 6.4), not 8: the divisor in FUN_140292740 is not
// k·nModels as read (c = 8 needs k·nModels ∈ (1349, 1517]); OPEN — E: use the chunk that reproduces Σ, and the exact
// formula needs a third data point (a map with a different nModels).
/// The decoration challenge's G (CGameCtnApp::HmsLightMapUpdateBlocksAndItemsQuality, RE 6 + RE 2): 1.0 for the
/// map's own objects; a decoration challenge's objects get 0.0625 on the Stadium collection (id 25) and 0.5 elsewhere
/// (kept for completeness — on stpad no decoration-map object produced a record).
pub fn quality_g(is_decoration: bool, collection_id: u32) -> f32 {
    if !is_decoration {
        1.0
    } else if collection_id == 25 {
        0.0625
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_plg(mbu: f32, uv0: [f32; 4]) -> PreLightGen {
        PreLightGen { u01: 1, meter_by_uv: mbu, uv0, uv1: [f32::MAX, f32::MAX, f32::MIN, f32::MIN], sprite_count: [0, 0], uv_groups: Vec::new() }
    }

    fn geom(lm_uvs: u32) -> LmGeom {
        LmGeom { lod_selected: true, indexed_triangles: true, material_lm_set: 1, positions: 24, lm_uvs }
    }

    #[test]
    fn the_filter_in_the_games_order() {
        let p = tiny_plg(32.1446, [0.001, 0.001, 0.9971, 0.999]);
        assert_eq!(static_item_record(None, true, false), Err(NoChart::NoPreLightGen));
        let mut z = p.clone();
        z.u01 = 0;
        assert_eq!(static_item_record(Some(&z), true, false), Err(NoChart::U01Zero));
        // u01 is a byte in memory: 256 reads as 0
        z.u01 = 0x100;
        assert_eq!(static_item_record(Some(&z), true, false), Err(NoChart::U01Zero));
        let mut d = p.clone();
        d.uv0 = [f32::MAX, f32::MAX, f32::MIN, f32::MIN]; // the ctor's empty box
        assert_eq!(static_item_record(Some(&d), true, false), Err(NoChart::DegenerateUv0));
        d.uv0 = [0.2, 0.1, 0.2, 0.9]; // u0 == u1
        assert_eq!(static_item_record(Some(&d), true, false), Err(NoChart::DegenerateUv0));
        assert_eq!(static_item_record(Some(&p), false, false), Err(NoChart::NoLightmapUvGeometry));
        assert_eq!(static_item_record(Some(&p), true, true), Err(NoChart::CustomLmMaterial));
        assert_eq!(static_item_record(Some(&p), true, false), Ok(()));
    }

    #[test]
    fn ac16497076_one_uv_set_has_no_lightmap_geometry() {
        // tiny 16 items 4816/4817 (RaceTriggerFXFinish, one visual of 24 vertices with TEXCOORD0 only): kept in
        // the editor bake, no chart in its table — the only two of 8120 kept items
        assert!(!lm_geometry_nonempty(&[geom(0)]));
        // a model where one LOD-0 visual has the set is charted (the pylons: 0/1-set bits among 2-set visuals)
        assert!(lm_geometry_nonempty(&[geom(0), geom(24)]));
        // positions must not exceed the lightmap uvs
        assert!(!lm_geometry_nonempty(&[LmGeom { positions: 25, ..geom(24) }]));
        // a non-selected LOD, a non-indexed visual or a material without a lightmap set adds nothing
        assert!(!lm_geometry_nonempty(&[LmGeom { lod_selected: false, ..geom(24) }]));
        assert!(!lm_geometry_nonempty(&[LmGeom { indexed_triangles: false, ..geom(24) }]));
        assert!(!lm_geometry_nonempty(&[LmGeom { material_lm_set: -1, ..geom(24) }]));
        assert!(!lm_geometry_nonempty(&[]));
    }

    #[test]
    fn quality_table_and_bytes() {
        assert_eq!((0..8).map(quality_exponent).collect::<Vec<_>>(), vec![0, 1, 2, 3, -1, -2, -3, 0]);
        assert_eq!(quality_float(0, 1.0), 1.0);
        assert_eq!(quality_float(1, 1.0).to_bits(), 0x3fb504f3); // √2 itself
        assert_eq!(quality_float(4, 1.0).to_bits(), 0x3f3504f3); // 1/√2 = 0.70710677
        // (0x3fb504f3)² = 1.99999993 → the nearest f32 is 1.9999999 (0x3fffffff), not 2.0
        assert_eq!(quality_float(2, 1.0).to_bits(), 0x3fffffff);
        // (0x3fb504f3)⁻³ = 0.353553404 → 0x3eb504f4 (0.35355341; the decimal-literal 0.35355338 would be one ulp under)
        assert_eq!(quality_float(6, 1.0).to_bits(), 0x3eb504f4);
        assert_eq!(quality_float(0, 0.5), 0.5); // a decoration's G
        // the CHmsItem byte: trunc, floor 1, cap 255
        assert_eq!(quality_byte(1.0), 255);
        assert_eq!(quality_byte(quality_float(1, 1.0)), 255); // 360 → capped
        assert_eq!(quality_byte(quality_float(4, 1.0)), 180); // 0.70710677·255 = 180.31 → 180
        assert_eq!(quality_byte(f32::from_bits(0x3d3504f3)), 11); // the far tile's 0.5^4.5 → 11 (RE 6's byte)
        assert_eq!(quality_byte(0.0), 1);
        assert_eq!(quality_byte(0.005), 1); // int(1.275) = 1 → 1
        assert_eq!(quality_from_byte(255), 1.0);
        assert_eq!(quality_from_byte(11), 11.0f32 / 255.0);
    }

    #[test]
    fn extent_and_area_as_the_game_computes_them() {
        // tiny 16's AC16497397 screen (dropped from the editor bake, but the numbers E printed): MeterByUv
        // 446.6406, uv0 [0.001, 0.001, 0.999, 0.84792197] → ext (445.7, 378.3), 168 613 m²
        let p = tiny_plg(446.6406, [0.001, 0.001, 0.999, 0.84792197]);
        let e = chart_ext(&p, 1.0, 0);
        assert!((e[0] - 445.7473).abs() < 1e-3 && (e[1] - 378.2697).abs() < 1e-3, "{e:?}");
        assert!(((e[1] * e[0]) - 168613.0).abs() < 2.0);
        // the quality enters as the block scale: Lowest (e = −3) → ext/2.828
        let q = quality_float(6, 1.0);
        let e3 = chart_ext(&p, q, 0);
        assert!((e3[0] - e[0] * q).abs() < 1e-3);
        // a negative MeterByUv → f = 0.1 (the "GetPreLightGenMeterByUv() < 0 !" warning)
        let mut neg = p.clone();
        neg.meter_by_uv = -3.0;
        assert_eq!(meter_by_uv(&neg, 1.0, 0).0, 0.1);
        // a non-finite f → (0, 0): the packer gives the minimum m·mins chart
        let mut nan = p.clone();
        nan.meter_by_uv = f32::NAN;
        assert_eq!(chart_ext(&nan, 1.0, 0), [0.0, 0.0]);
        // alloc mode 1 reads the second set
        let mut two = p.clone();
        two.uv1 = [0.0, 0.0, 0.5, 0.5];
        assert_eq!(chart_ext(&two, 1.0, 1), [0.5 * 446.6406, 0.5 * 446.6406]);
    }

    #[test]
    fn model_grids_as_on_tiny16() {
        // tiny 16 (baker's /lmrecords dump, D₁ = 2048² / 14 207 847 = 0.29521): 32-m items with n instances →
        // the blocks the file shows: 6 → 3×2 (cols × rows), 7 → 3×3 (2 empty), 3 → 2×2, 2 → 2×1, 12 → 4×3
        let d1 = 0.2952104f32;
        let e = [32.02f32, 32.08];
        assert_eq!(grid_dims(6, e, d1, 6), (3, 2));
        assert_eq!(grid_dims(7, e, d1, 6), (3, 3));
        assert_eq!(grid_dims(3, e, d1, 6), (2, 2));
        assert_eq!(grid_dims(2, e, d1, 6), (2, 1));
        assert_eq!(grid_dims(12, e, d1, 6), (4, 3));
        assert_eq!(grid_dims(1, e, d1, 6), (1, 1));
        assert_eq!(grid_dims(8, e, d1, 6), (3, 3));
        // the far tiles (q 0.0442, ext 1.41 m → both sides under m → r = 1): 2572 → 51 × 51
        assert_eq!(grid_dims(2572, [1.41, 1.42], d1, 6), (51, 51));
        // a wide chart (r = 4): 4 instances stack in a column of rows
        assert_eq!(grid_dims(4, [64.0, 16.0], d1, 6), (1, 4));
        let (ext2, area2) = grid_chart(e, 3, 3);
        assert_eq!(ext2, [3.0 * 32.02, 3.0 * 32.08]);
        assert!((area2 - 9.0 * 32.08 * 32.02).abs() < 0.01);
        assert!(!is_solo(e, d1));
        assert!(is_solo([400.0, 400.0], d1)); // 400·0.295 = 118 > 100 on both sides
        // stpad (D₁ 5.1097): the 66×14 border charts (32 × 8 m) are grouped, the 32 × 32 bases and the 22.6-m tiles solo
        assert!(!is_solo([32.0, 8.0], 5.1097));
        assert!(is_solo([32.0, 32.0], 5.1097));
        assert!(is_solo([22.63, 22.63], 5.1097));
        // stpad's chunk: 735 entries (AND) → k = 2 → 12141 / 1470 = 8.26 → 8
        assert_eq!(chunk_size(12141, 735), Some(8));
        // Z-order cells of a 3×3 grid: the 7 instances fill (0,0) (1,0) (0,1) (1,1) (2,0) (2,1) (0,2) — the two
        // empty cells are (1,2) and (2,2)
        assert_eq!(zorder_cells(3, 3), vec![(0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (2, 1), (0, 2), (1, 2), (2, 2)]);
        assert_eq!(zorder_cells(2, 1), vec![(0, 0), (1, 0)]);
        // 48 units over 3 cells at g = 2: 16 each (the file's 12/14/16 cell sizes come from this spread; the exact
        // f32 accumulation order of FUN_140291c80 is only partially read — E: match against the dump's cells)
        assert_eq!(cell_edges(48, 3, 2), vec![0, 16, 32, 48]);
    }

    #[test]
    fn chunks_of_eight_on_tiny16() {
        assert_eq!(chunk_size(12214, 458), Some(8)); // k = ceil(1000/458) = 3; 12214 / 1374 = 8.89 → 8
        assert_eq!(chunk_size(4099, 3), Some(4)); // a pwc-day-like count: k = 334 → 4099 / 1002 = 4
        assert_eq!(chunk_size(20000, 1000), None); // k = 1 → no split
        assert_eq!(avoid_bad_chunk(3), 4);
        assert_eq!(avoid_bad_chunk(7), 8);
        assert_eq!(avoid_bad_chunk(11), 12);
        assert_eq!(avoid_bad_chunk(0), 1);
        assert_eq!(avoid_bad_chunk(9), 9);
        assert_eq!(avoid_bad_chunk(13), 13);
        // the 12-instance model of the dump (records 4953 … 6159 in record order, world centres): the file's blocks are
        // {4953,4954,4957,4967,4968,5114,5170,6159} (3×3 grid, 8 used) and {5113,5167,5168,5169} (2×2)
        let centres = [
            [977.0f32, 34.0, 457.0], [980.0, 34.0, 461.0], [982.0, 34.0, 457.0], [998.0, 34.0, 479.0], [996.0, 34.0, 467.0],
            [1316.0, 65.0, 478.0], [1314.0, 62.0, 468.0], [1301.0, 65.0, 484.0], [1295.0, 60.0, 491.0], [1290.0, 58.0, 495.0],
            [1282.0, 59.0, 492.0], [1263.0, 54.0, 511.0],
        ];
        let ords = group_ordinals(&centres);
        let blocks: Vec<u32> = ords.iter().map(|o| o / 8).collect();
        assert_eq!(blocks, vec![0, 0, 0, 0, 0, 1, 0, 1, 1, 1, 0, 0]);
        let list = split_chunks(&[1, 12, 3, 9], Some(8));
        assert_eq!(list.len(), 6);
        assert_eq!(list[1], ModelChart { group: 1, chunk: 0, count: 8 });
        assert_eq!(list[4], ModelChart { group: 1, chunk: 1, count: 4 });
        assert_eq!(list[5], ModelChart { group: 3, chunk: 1, count: 1 });
        assert_eq!(split_chunks(&[1, 12], None).len(), 2);
    }

    #[test]
    fn grouped_order_and_decoration_quality() {
        // equal areas keep the model-list order; the identity reset means no centre key enters
        let order = grouped_pack_order(&[4.0, 1.0, 4.0, 0.5, 1.0]);
        assert_eq!(order, vec![3, 1, 4, 0, 2]);
        assert_eq!(quality_g(false, 25), 1.0);
        assert_eq!(quality_g(true, 25), 0.0625);
        assert_eq!(quality_g(true, 26), 0.5);
        // stpad's WaterBase clip entities (the baker's dump): FCCenter_Air border mesh and its nested RoadBorderSpot /
        // BarrierSupport — one kind-2 record each at q 1.0
        let border = PreLightGen { u01: 1, meter_by_uv: 32.1997643, uv0: [0.0223656, 0.014208376, 0.5534582, 0.26005277], uv1: [f32::MAX, f32::MAX, f32::MIN, f32::MIN], sprite_count: [0, 0], uv_groups: Vec::new() };
        let e = chart_ext(&border, 1.0, 0);
        assert!((e[0] - 17.10).abs() < 0.01 && (e[1] - 7.92).abs() < 0.01, "{e:?}");
        let spot = PreLightGen { u01: 1, meter_by_uv: 11.7644405, uv0: [0.0074898615, 0.007874399, 0.12044389, 0.11907996], ..border.clone() };
        let e = chart_ext(&spot, 1.0, 0);
        assert!((e[0] - 1.329).abs() < 0.002 && (e[1] - 1.308).abs() < 0.002, "{e:?}");
        // the 5-record WaterBase object: all five pass the static filter
        assert_eq!(static_item_record(Some(&spot), true, false), Ok(()));
    }

    #[test]
    fn two_records_only_with_two_or_more_uv_groups() {
        let mut p = tiny_plg(10.0, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(chart_records(&p), vec![ChartRecord::Whole]);
        p.uv_groups = vec![[10.0, 0.0, 0.0, 0.5, 0.5]];
        assert_eq!(chart_records(&p), vec![ChartRecord::Whole]);
        p.uv_groups = vec![[10.0, 0.0, 0.0, 0.5, 0.5], [20.0, 0.5, 0.0, 1.0, 0.5], [f32::INFINITY, 0.0, 0.5, 0.5, 1.0], [40.0, 0.5, 0.5, 1.0, 1.0]];
        let r = chart_records(&p);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0], ChartRecord::SubVisual { sub: 0, f: 10.0, rect: [0.0, 0.0, 0.5, 0.5] });
        // the merged f averages the FINITE f's of subs 1..n−1: (20 + 40) / 2
        assert_eq!(r[1], ChartRecord::MergedGroup { avg_f: 30.0, subs: vec![1, 2, 3] });
    }
}
