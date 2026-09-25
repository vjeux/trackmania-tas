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
