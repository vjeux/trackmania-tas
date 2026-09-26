//! THE LOCAL-LIGHT FRAME DRIVER — lightmap FRAME 1 (engineer F, 2026-09-26). The game's lightmap carries two frames per
//! mood; frame 1 = THE LOCAL LIGHTS (the CPlugLight spots / balls of the items and of the blocks' and clips' prefabs),
//! rendered only when the mood blender has the lights ON (`moods::BlenderCurve::local_lights_on`: Sunrise / Sunset /
//! Night; Day → a black frame with MaxHDR 1e-5). Everything below is the driver that ties the transcribed kernels
//! together, per lamp in the light list's order (RE 7's `lmtool map-lights` set):
//!
//! 1. the RECORD CULL `lightcull::local_light_sees` over the layout's records (box within R_eff of the lamp, bounding
//!    sphere in the spot cone) → the drawn instance set (`LmScene::rec_of`: chart k ↔ record k ↔ LM instance);
//! 2. the FLAT-CUBE SHADOW MAP (`locallight::render_flat_cube`: six 173² D16 faces, the casters = the drawn set's
//!    triangles, Greater on a 0 clear, bias −1 / −1.0, cull Back);
//! 3. the probe draw (PS 7351 `locallight::ps_7351` over the lamp's probe chunks into the R8 volume) → CS 7357
//!    (`locallight::cs_7357`: the probe light lists);
//! 4. NINE raster jitters (`moods::local_light_jitter` → LM01_Trans_RasterSS, OutScale 1/9) × the drawn set through
//!    VS 7303 (= the LM vertex path: `lmaccum::{chart_st, lm_clip, world_pos, rotate}`) → PS 7343 (`locallight::ps_7343`)
//!    blended One / One into the 3072 × 2048 RGBA16F accumulation (f16 source truncation, RTNE sum — the measured ROP);
//! 5. CS 7348 (`locallight::cs_7348`) over the touched texels (± the one-texel dilation ring) into the per-texel light
//!    lists (TexLightId / TexLightW / TexLightIsLit; the cleared id is 0xffff); the accumulation is cleared for the next lamp.
//!
//! After the last lamp the lists are COMPOSED into the frame-1 image (`compose`: the literal Σ over a texel's list of the
//! √-decoded weights × the lamp's LightRgb — the exact compose dispatch is being pinned by RE 7 from capture 2's tail;
//! `ComposeRule` selects the candidates), MaxHDR = the image max, the frame-1 record {StoreLAmbient 0, LocalLight_Storage 1,
//! HBasis234 −1, LAmbient −1}, the WebP through the transcribed writer.
//!
//! The lamp constants the capture pins and the file does not derive yet (RE 7's open items): the EFFECTIVE radius
//! (40.707722 for the RoadBorderSpot whose file radius is 40 — `effective_radius`), `SpotFalloffBackOffset` (0.33362 for
//! it — `spot_falloff_back_offset`) and the LightRgb factor of the local-light frame (`light_rgb`). The flat-cube face size
//! (173 for R = 40.71) follows FUN_14023cb30's ceilf(2·R·density·lm+0x10c) — `face_size` carries the pinned value.
//!
//! Oracles (passcap/stpad-sunrise, f4936): 9768 after eids 547 / 1670 (the accumulation after lamps A / B), 9810 after
//! 1080 (lamp B's flat cube), 9783 after 1081 (lamp B's probe volume), the CS UAVs after 563 / 572 (lamp A) and 1671
//! (lamp B); the editor's stpad save (the frame-1 WebP) — `lmtool local-lights … --check DIR`.

use crate::geometry::LightDef;
use crate::gpufmt::{quantise_f16, Rounding};
use crate::lmaccum::{blend_f16, chart_st, lm_clip, rotate, world_pos, LmRasterCb, LmScene};
use crate::locallight::{cs_7348, cs_7357, flat_cube_face_viewport, ps_7343, ps_7351, render_flat_cube, FlatCubeFace, FlatCubeMap, LightCb, LightList, ProbeLightCb, ProbeLightList};
use crate::sunpass::{rasterise_triangle_rows, rotation_rows, BlendModel};

/// The accumulation target of the pass on stpad: 3072 × 2048 (the 2048² layout in its left part; the jitter offsets and the
/// chart ST are relative to it — RE 7's NOTES 00:10Z).
pub const TARGET: (u32, u32) = (3072, 2048);
/// The flat-cube shadow depth target (D16 4096²) and the pinned face size of the RoadBorderSpot lamps.
pub const SHADOW_TARGET: u32 = 4096;
pub const FACE_SIZE_R40: u32 = 173;

/// One local light of the frame: its id (the frame light list's index), its owner tag and the constants the pass binds.
#[derive(Clone, Debug)]
pub struct Lamp {
    pub id: u16,
    pub owner: String,
    pub light: LightDef,
    /// The instance radius the pass reads (the attenuation zero, the cull radius, the shadow far plane).
    pub r_eff: f32,
    pub back_offset: f32,
    pub face_size: u32,
    /// LightSH[0].xyz = the lamp colour the frame composes with.
    pub rgb: [f32; 3],
}

/// The pinned instance radius: 40.707722 for a radius-40 lamp with hyper2 (−1.24, 0.206) (the RoadBorderSpot; RE 7 pins
/// the source of the +0.7077 from the light instance's +0xa0 table), else the file radius.
pub fn effective_radius(l: &LightDef) -> f32 {
    if (l.radius - 40.0).abs() < 1e-4 && (l.hyper2[0] + 1.24).abs() < 1e-4 && (l.hyper2[1] - 0.206).abs() < 1e-4 {
        40.707722
    } else {
        l.radius
    }
}

/// `SpotFalloffBackOffset` of the lamp's cbuffer — 0.33362 on the RoadBorderSpot (capture f4936 eid 34); its derivation
/// from the light file is open (RE 7). Other lamps: 0 until pinned.
pub fn spot_falloff_back_offset(l: &LightDef) -> f32 {
    if (l.radius - 40.0).abs() < 1e-4 && (l.cone.1 - 170.0).abs() < 1e-3 {
        0.3336227238178253
    } else {
        0.0
    }
}

/// LightSH[0].xyz of the frame: the desc's premultiplied colour · intensity (RE 7's binder, 23:58Z) — the local-light frame's
/// extra curve factor is 1 until the capture's cbuffer pins it.
pub fn light_rgb(l: &LightDef) -> [f32; 3] {
    [l.color[0] * l.intensity, l.color[1] * l.intensity, l.color[2] * l.intensity]
}

/// The face size of the lamp's flat cube: FUN_14023cb30's ceilf(2·R·density) clamped to [8, max] — 173 for the pinned
/// lamp (R 40.7077); scaled with R for the others (the density is the pinned ratio).
pub fn face_size(r_eff: f32) -> u32 {
    ((2.0 * r_eff * (FACE_SIZE_R40 as f32 / (2.0 * 40.707722))).ceil() as u32).clamp(8, 1024)
}

/// The six flat-cube faces' Scale_MaxAbs / Trans for `size`² faces in the `target`² D16 texture: face f's tile at
/// `flat_cube_face_viewport(f, size)`, Scale = ±size/(2·target) (the capture's sign pattern), MaxAbs = (size − 1)/(2·target),
/// Trans = the tile centre. The captured eid 34 values for size 173 / target 4096 to the bit.
pub fn flat_cube_faces(size: u32, target: u32) -> [FlatCubeFace; 6] {
    let a = size as f32 / (2.0 * target as f32);
    let m = (size as f32 - 1.0) / (2.0 * target as f32);
    let signs: [[f32; 2]; 6] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [1.0, -1.0], [1.0, -1.0], [-1.0, -1.0]];
    let mut out = [FlatCubeFace { scale: [0.0; 2], max_abs: [m, m], trans: [0.0; 2] }; 6];
    for f in 0..6 {
        let (ox, oy) = flat_cube_face_viewport(f, size);
        out[f] = FlatCubeFace { scale: [signs[f][0] * a, signs[f][1] * a], max_abs: [m, m], trans: [(ox as f32 + size as f32 * 0.5) / target as f32, (oy as f32 + size as f32 * 0.5) / target as f32] };
    }
    out
}

impl Lamp {
    pub fn new(id: u16, owner: &str, light: LightDef) -> Lamp {
        let r_eff = effective_radius(&light);
        Lamp { id, owner: owner.to_string(), light, r_eff, back_offset: spot_falloff_back_offset(&light), face_size: face_size(r_eff), rgb: light_rgb(&light) }
    }

    pub fn is_spot(&self) -> bool {
        self.light.cone.1 < 180.0
    }

    /// PS 7343's cbuffer for this lamp: the flat-cube faces, ZScale = −1/999, ZTrans = R/999, InvRadius2, the cone terms from
    /// the FULL cone angles halved (CosOuter = cos(outer/2), InvCosRange = 1/(cos(inner/2) − cos(outer/2))), SpotDirNeg = −dir,
    /// AttHN2 = moods::att_hn2(R, hyper2), OutScale 1/9.
    pub fn light_cb(&self) -> LightCb {
        let l = &self.light;
        let s = 1.0f32 / 999.0;
        let cos_outer = (l.cone.1.to_radians() * 0.5).cos();
        let cos_inner = (l.cone.0.to_radians() * 0.5).cos();
        let inv_cos_range = 1.0 / (cos_inner - cos_outer);
        let d = l.dir;
        LightCb {
            faces: flat_cube_faces(self.face_size, SHADOW_TARGET),
            z_scale: -s,
            z_trans: self.r_eff * s,
            light_pos: l.pos,
            inv_radius2: 1.0 / (self.r_eff * self.r_eff),
            inv_cos_range,
            cos_outer,
            spot_dir_neg: [-d[0], -d[1], -d[2]],
            spot_falloff_back_offset: self.back_offset,
            att_hn2: crate::moods::att_hn2(self.r_eff, l.hyper2[0], l.hyper2[1]),
            out_scale: [1.0 / 9.0; 4],
            is_light_spot: self.is_spot(),
            is_att_hn2: true,
        }
    }

    /// PS 7351's cbuffer for one probe chunk: ProbeStWorld = (cell, the world of atlas index 0) per axis.
    pub fn probe_cb(&self, st: [[f32; 2]; 3]) -> ProbeLightCb {
        let cb = self.light_cb();
        ProbeLightCb {
            st,
            faces: cb.faces,
            z_scale: cb.z_scale,
            z_trans: cb.z_trans,
            world_pw01_shadow: [[0.0; 4]; 4],
            light_pos_or_dir: cb.light_pos,
            is_light_pos: true,
            inv_radius2: cb.inv_radius2,
            inv_cos_range: cb.inv_cos_range,
            cos_outer: cb.cos_outer,
            spot_dir_neg: cb.spot_dir_neg,
            samples_per_axis: 3,
            att_hn2: cb.att_hn2,
            is_light_spot: cb.is_light_spot,
            is_shadow_cube: true,
            is_att_hn2: true,
            is_att_1minus_d2: false,
        }
    }
}

/// The frame's light list: the items' lights (the scene's, in item order) then the blocks' and clips' (records::MapRecords::
/// block_lights) — `lmtool map-lights`' order; the ids are the list indices (the capture numbers lamp B 243 in the scene
/// instance-list order — the ORDER is verified only for the two captured lamps' neighbourhood).
pub fn lamps(item_lights: &[(usize, LightDef)], block_lights: &[(String, LightDef)]) -> Vec<Lamp> {
    let mut out = Vec::with_capacity(item_lights.len() + block_lights.len());
    for (i, l) in item_lights {
        let id = out.len() as u16;
        out.push(Lamp::new(id, &format!("item {i}"), *l));
    }
    for (o, l) in block_lights {
        let id = out.len() as u16;
        out.push(Lamp::new(id, o, *l));
    }
    out
}

/// THE RECORD CULL of one lamp over the layout's records → the LM instances to draw (in LM scene order = the draw order).
pub fn cull(gl: &crate::layout::GameLayout, sc: &LmScene, lamp: &Lamp) -> Vec<usize> {
    let mut seen = vec![false; gl.records.len()];
    for (k, r) in gl.records.iter().enumerate() {
        seen[k] = crate::lightcull::local_light_sees(r.centre, r.half, &lamp.light, lamp.r_eff);
    }
    (0..sc.instances.len()).filter(|&ii| sc.rec_of.get(ii).map(|&k| k < seen.len() && seen[k]).unwrap_or(false)).collect()
}

/// The mesh of an LM instance (the mesh whose instance range holds it).
pub fn mesh_of(sc: &LmScene, ii: usize) -> Option<usize> {
    (0..sc.meshes.len()).find(|&m| ii >= sc.inst_first[m] && ii < sc.inst_first[m] + sc.inst_count[m])
}

/// The world triangles of the drawn instances (the casters of the flat cube), in the LM mesh's index order.
pub fn casters(sc: &LmScene, insts: &[usize]) -> Vec<[[f32; 3]; 3]> {
    let mut out = Vec::new();
    for &ii in insts {
        let Some(m) = mesh_of(sc, ii) else { continue };
        let mesh = &sc.meshes[m];
        let inst = &sc.instances[ii];
        let rows = rotation_rows(inst.q);
        let wp: Vec<[f32; 3]> = mesh.verts.iter().map(|v| world_pos(v, inst, &rows)).collect();
        for t in mesh.indices.chunks_exact(3) {
            out.push([wp[t[0] as usize], wp[t[1] as usize], wp[t[2] as usize]]);
        }
    }
    out
}

/// The RGBA16F accumulation target (values as the f16 target holds them) with the rectangle its lamp touched.
pub struct Accum {
    pub w: u32,
    pub h: u32,
    pub px: Vec<[f32; 4]>,
    /// x0, y0, x1, y1 (exclusive) of the texels written since the last clear.
    pub touched: Option<[u32; 4]>,
    /// Per texel the LM instance that wrote it last (+1; 0 = none) when the diagnostics ask for it (`Accum::with_owner`).
    pub owner: Vec<u32>,
    /// The texels written since the last clear: a bit per texel and the list in first-write order (perf 8: the clear and
    /// the list resolve visit these, not the touched box — a lamp's charts sit all over the atlas).
    pub hit: Vec<u64>,
    pub touched_list: Vec<u32>,
}

impl Accum {
    pub fn new(w: u32, h: u32) -> Accum {
        let n = (w * h) as usize;
        Accum { w, h, px: vec![[0.0; 4]; n], touched: None, owner: Vec::new(), hit: vec![0u64; (n + 63) / 64], touched_list: Vec::new() }
    }
    pub fn with_owner(w: u32, h: u32) -> Accum {
        let mut a = Accum::new(w, h);
        a.owner = vec![0; (w * h) as usize];
        a
    }
    /// The 9-jitter sum CS 7348 reads: (light, shadow, coverage) — TMapLightSum.xyw, 0 outside the target.
    pub fn sum(&self, x: i32, y: i32) -> [f32; 3] {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return [0.0; 3];
        }
        let p = self.px[(y as u32 * self.w + x as u32) as usize];
        [p[0], p[1], p[3]]
    }
    pub fn clear_touched(&mut self) {
        // (the written texels only — the box could be most of the atlas)
        for &i in &self.touched_list {
            self.px[i as usize] = [0.0; 4];
            self.hit[i as usize >> 6] &= !(1u64 << (i & 63));
        }
        self.touched_list.clear();
        self.touched = None;
    }
    fn touch(&mut self, x: u32, y: u32) {
        self.touched = Some(match self.touched {
            None => [x, y, x + 1, y + 1],
            Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)],
        });
        let i = y * self.w + x;
        let (wi, b) = ((i >> 6) as usize, 1u64 << (i & 63));
        if self.hit[wi] & b == 0 {
            self.hit[wi] |= b;
            self.touched_list.push(i);
        }
    }
    /// CS 7348's weights for this lamp at every texel it can change — the written texels and their 3×3 rings (a texel whose
    /// own and ring sums are all 0 gets weight 0 and keeps its list, so the rest of the atlas needs no visit): the list of
    /// (texel, weight byte, lit byte) with non-zero weight, in no particular order (the texels are independent). `visited`
    /// is scratch of the accumulation's bitmap size, left clear.
    pub fn lamp_weights(&self, visited: &mut [u64]) -> Vec<(u32, u8, u8)> {
        let sum = |x: i32, y: i32| self.sum(x, y);
        let mut out = Vec::with_capacity(self.touched_list.len() * 2);
        let mut marked: Vec<u32> = Vec::with_capacity(self.touched_list.len() * 4);
        let (w, h) = (self.w as i32, self.h as i32);
        for &t in &self.touched_list {
            let (tx, ty) = ((t % self.w) as i32, (t / self.w) as i32);
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let (x, y) = (tx + dx, ty + dy);
                    if x < 0 || y < 0 || x >= w || y >= h { continue; }
                    let i = (y as u32) * self.w + x as u32;
                    let (wi, b) = ((i >> 6) as usize, 1u64 << (i & 63));
                    if visited[wi] & b != 0 { continue; }
                    visited[wi] |= b;
                    marked.push(i);
                    let (w8, sh) = crate::locallight::cs_7348_weight(&sum, x, y);
                    if w8 != 0 {
                        out.push((i, w8 as u8, (sh * 255.0) as u32 as u8));
                    }
                }
            }
        }
        for &i in &marked { visited[i as usize >> 6] &= !(1u64 << (i & 63)); }
        out
    }
}

/// What a lamp's draw writes into: the dense `Accum` (the serial frame and the diagnostics) or the paged one (the parallel
/// frame's workers). `slot` hands out the texel's RGBA16F value to blend into and marks it touched.
pub trait LampTarget {
    fn size(&self) -> (u32, u32);
    fn slot(&mut self, x: u32, y: u32) -> &mut [f32; 4];
    fn set_owner(&mut self, x: u32, y: u32, inst: u32);
}

impl LampTarget for Accum {
    fn size(&self) -> (u32, u32) { (self.w, self.h) }
    #[inline]
    fn slot(&mut self, x: u32, y: u32) -> &mut [f32; 4] {
        self.touch(x, y);
        &mut self.px[(y * self.w + x) as usize]
    }
    #[inline]
    fn set_owner(&mut self, x: u32, y: u32, inst: u32) {
        if !self.owner.is_empty() { self.owner[(y * self.w + x) as usize] = inst; }
    }
}

/// THE PAGED ACCUMULATION TARGET (perf 8): a lamp's charts sit all over the 2048² atlas but cover a small part of it, and a
/// dense 64 MB target per worker had 64 workers' random blends thrashing DRAM (the draw ran 4× slower than alone). The atlas
/// in 64 × 64-texel pages allocated on first touch from an arena kept across lamps: a lamp's working set is a few MB, in
/// cache. Reads (`sum`) see 0 where no page is — as the cleared dense target does.
pub struct PagedAccum {
    pub w: u32,
    pub h: u32,
    tiles_x: u32,
    /// Per page its arena slot, or NONE.
    page_of: Vec<u32>,
    arena: Vec<[f32; 4]>,
    used: Vec<u32>,
    pub hit: Vec<u64>,
    pub touched_list: Vec<u32>,
}

const PAGE_SHIFT: u32 = 6;
const PAGE_TEXELS: usize = 1 << (2 * PAGE_SHIFT);
const NO_PAGE: u32 = u32::MAX;

impl PagedAccum {
    pub fn new(w: u32, h: u32) -> PagedAccum {
        let tiles_x = (w + (1 << PAGE_SHIFT) - 1) >> PAGE_SHIFT;
        let tiles_y = (h + (1 << PAGE_SHIFT) - 1) >> PAGE_SHIFT;
        PagedAccum { w, h, tiles_x, page_of: vec![NO_PAGE; (tiles_x * tiles_y) as usize], arena: Vec::new(), used: Vec::new(), hit: vec![0u64; ((w * h) as usize + 63) / 64], touched_list: Vec::new() }
    }
    #[inline]
    fn tile_of(&self, x: u32, y: u32) -> usize {
        ((y >> PAGE_SHIFT) * self.tiles_x + (x >> PAGE_SHIFT)) as usize
    }
    #[inline]
    fn offset_in_page(x: u32, y: u32) -> usize {
        (((y & ((1 << PAGE_SHIFT) - 1)) << PAGE_SHIFT) | (x & ((1 << PAGE_SHIFT) - 1))) as usize
    }
    pub fn sum(&self, x: i32, y: i32) -> [f32; 3] {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return [0.0; 3];
        }
        let (x, y) = (x as u32, y as u32);
        let pg = self.page_of[self.tile_of(x, y)];
        if pg == NO_PAGE {
            return [0.0; 3];
        }
        let p = self.arena[pg as usize * PAGE_TEXELS + Self::offset_in_page(x, y)];
        [p[0], p[1], p[3]]
    }
    /// The lamp's CS 7348 weights — as `Accum::lamp_weights`, over the paged values.
    pub fn lamp_weights(&self, visited: &mut [u64]) -> Vec<(u32, u8, u8)> {
        let sum = |x: i32, y: i32| self.sum(x, y);
        let mut out = Vec::with_capacity(self.touched_list.len() * 2);
        let mut marked: Vec<u32> = Vec::with_capacity(self.touched_list.len() * 4);
        let (w, h) = (self.w as i32, self.h as i32);
        for &t in &self.touched_list {
            let (tx, ty) = ((t % self.w) as i32, (t / self.w) as i32);
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let (x, y) = (tx + dx, ty + dy);
                    if x < 0 || y < 0 || x >= w || y >= h { continue; }
                    let i = (y as u32) * self.w + x as u32;
                    let (wi, b) = ((i >> 6) as usize, 1u64 << (i & 63));
                    if visited[wi] & b != 0 { continue; }
                    visited[wi] |= b;
                    marked.push(i);
                    let (w8, sh) = crate::locallight::cs_7348_weight(&sum, x, y);
                    if w8 != 0 {
                        out.push((i, w8 as u8, (sh * 255.0) as u32 as u8));
                    }
                }
            }
        }
        for &i in &marked { visited[i as usize >> 6] &= !(1u64 << (i & 63)); }
        out
    }
    /// Back to the cleared state: the used pages zeroed (and kept in the arena), the hit bits and the list dropped.
    pub fn clear(&mut self) {
        for &t in &self.used {
            let pg = self.page_of[t as usize];
            if pg != NO_PAGE {
                self.arena[pg as usize * PAGE_TEXELS..(pg as usize + 1) * PAGE_TEXELS].fill([0.0; 4]);
                self.page_of[t as usize] = NO_PAGE;
            }
        }
        self.used.clear();
        for &i in &self.touched_list {
            self.hit[i as usize >> 6] &= !(1u64 << (i & 63));
        }
        self.touched_list.clear();
    }
}

impl LampTarget for PagedAccum {
    fn size(&self) -> (u32, u32) { (self.w, self.h) }
    #[inline]
    fn slot(&mut self, x: u32, y: u32) -> &mut [f32; 4] {
        let i = y * self.w + x;
        let (wi, b) = ((i >> 6) as usize, 1u64 << (i & 63));
        if self.hit[wi] & b == 0 {
            self.hit[wi] |= b;
            self.touched_list.push(i);
        }
        let t = self.tile_of(x, y);
        let mut pg = self.page_of[t];
        if pg == NO_PAGE {
            // a fresh page: the arena grows by one (zeroed) page the first time, else a cleared one is reused in place
            pg = (self.used.len()) as u32;
            if (pg as usize + 1) * PAGE_TEXELS > self.arena.len() {
                self.arena.resize((pg as usize + 1) * PAGE_TEXELS, [0.0; 4]);
            }
            self.page_of[t] = pg;
            self.used.push(t as u32);
        }
        &mut self.arena[pg as usize * PAGE_TEXELS + Self::offset_in_page(x, y)]
    }
    #[inline]
    fn set_owner(&mut self, _x: u32, _y: u32, _inst: u32) {}
}

/// The LM01 raster constants of jitter `n` for a `w` × `h` target: Trans = (−1 + ox·2/W, 1 + oy·2/H) (RE 7's 00:35Z; eid 34's
/// (−0.99985534, 0.99956596) for n = 2 on 3072 × 2048), Scale (2, −2).
pub fn jitter_cb(n: u32, w: u32, h: u32) -> LmRasterCb {
    let (ox, oy) = crate::moods::local_light_jitter(n);
    LmRasterCb { scale_ss: [2.0, -2.0], trans_ss: [-1.0 + ox * 2.0 / w as f32, 1.0 + oy * 2.0 / h as f32] }
}

/// A vertex after VS 7303: the LM clip xy, the world position (o1), the world normal (o3).
#[derive(Clone, Copy, Debug)]
pub struct Vs7303Out {
    pub clip: [f32; 2],
    pub world: [f32; 3],
    pub normal: [f32; 3],
}

/// VS 7303 (the lighting draw's vertex shader = the LM vertex path of VS 17111 / 17118 with the jitter constants).
pub fn vs_7303(v: &crate::sunpass::LmVertex, inst: &crate::sunpass::LmInstance, table: &[[f32; 4]], cb: &LmRasterCb) -> Vs7303Out {
    let rows = rotation_rows(inst.q);
    let st = chart_st(v, inst, table);
    Vs7303Out { clip: lm_clip(v, st, cb), world: world_pos(v, inst, &rows), normal: rotate(v.normal, &rows) }
}

/// The shadow lookup of the pass: the LINEAR comparison sample (UseSoftShadow 1 — the captured shadow channel holds values
/// between the ninths); LMTOOL_LL_SHADOW=point for the point sample, LMTOOL_LL_SHADOW_BITS=N for the weights' sub-texel bits.
pub fn shadow_sample(shadow: &FlatCubeMap, uv: [f32; 2], r: f32) -> f32 {
    // (the two study switches read ONCE — perf 8: two std::env::var per fragment and per probe were 52 % of a frame-1 bake's
    // samples, the environment's RwLock contended by every worker: getenv + CStr + read_contended)
    static MODE: std::sync::OnceLock<(u8, u32)> = std::sync::OnceLock::new();
    let (mode, bits) = *MODE.get_or_init(|| {
        let mode = match std::env::var("LMTOOL_LL_SHADOW").as_deref() { Ok("point") => 1u8, Ok("none") => 2, _ => 0 };
        // f32 weights (0 bits) measure best against the captured accumulation (lamp A: 433 list texels differ vs 512 at 8 bits, 862 at 4)
        let bits: u32 = std::env::var("LMTOOL_LL_SHADOW_BITS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        (mode, bits)
    });
    match mode {
        1 => return shadow.sample_cmp_ge(uv, r, SHADOW_TARGET),
        2 => return 1.0,
        _ => {}
    }
    shadow.sample_cmp_ge_linear(uv, r, SHADOW_TARGET, bits)
}

/// The blend of the pass: the source truncated to f16, the sum rounded to nearest even (the measured RGBA16F ROP).
pub const BLEND: BlendModel = BlendModel::TruncSrcRoundSum;

/// THE LIGHTING DRAWS of one lamp: nine jitters × the drawn instances (jitter-major, then the LM scene's mesh order, the
/// instances in record order — the capture's 9 × 11 draws) through VS 7303 → the D3D11 raster (`rasterise_triangle_rows`,
/// the pixel-centre barycentrics, no depth, NoCull) → PS 7343 → One / One into `acc`.
pub fn draw_lamp<T: LampTarget>(sc: &LmScene, insts: &[usize], cb: &LightCb, shadow: &FlatCubeMap, acc: &mut T) -> u64 {
    draw_lamp_partial(sc, insts, cb, shadow, acc, 0..9, &[])
}

/// `draw_lamp` over the jitters `jitters` only, the instances `skip_last` left out of the LAST jitter of the range — the
/// capture's export points (9768 after eid 547 = lamp A's 9 jitters minus the final draw group; after eid 1670 = lamp B's
/// first 7 jitters, the frame ended there).
pub fn draw_lamp_partial<T: LampTarget>(sc: &LmScene, insts: &[usize], cb: &LightCb, shadow: &FlatCubeMap, acc: &mut T, jitters: std::ops::Range<u32>, skip_last: &[usize]) -> u64 {
    let (w, h) = acc.size();
    let last = jitters.end.saturating_sub(1);
    let sample = |uv: [f32; 2], r: f32| shadow_sample(shadow, uv, r);
    let mut frags = 0u64;
    // the instances grouped by mesh in the scene's mesh order (the draw order)
    let mut by_mesh: Vec<Vec<usize>> = vec![Vec::new(); sc.meshes.len()];
    for &ii in insts {
        if let Some(m) = mesh_of(sc, ii) {
            by_mesh[m].push(ii);
        }
    }
    for n in jitters {
        let rcb = jitter_cb(n, w, h);
        for (m, list) in by_mesh.iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            let mesh = &sc.meshes[m];
            for &ii in list {
                if n == last && skip_last.contains(&ii) {
                    continue;
                }
                let inst = &sc.instances[ii];
                let vs: Vec<Vs7303Out> = mesh.verts.iter().map(|v| vs_7303(v, inst, &sc.table, &rcb)).collect();
                for t in mesh.indices.chunks_exact(3) {
                    let (a, b, c) = (&vs[t[0] as usize], &vs[t[1] as usize], &vs[t[2] as usize]);
                    rasterise_triangle_rows([a.clip, b.clip, c.clip], w, h, 0, h as i64, |x, y, b0, b1, b2| {
                        let p = [a.world[0] * b0 + b.world[0] * b1 + c.world[0] * b2, a.world[1] * b0 + b.world[1] * b1 + c.world[1] * b2, a.world[2] * b0 + b.world[2] * b1 + c.world[2] * b2];
                        let nrm = [a.normal[0] * b0 + b.normal[0] * b1 + c.normal[0] * b2, a.normal[1] * b0 + b.normal[1] * b1 + c.normal[1] * b2, a.normal[2] * b0 + b.normal[2] * b1 + c.normal[2] * b2];
                        let o = ps_7343(cb, p, nrm, &sample);
                        let slot = acc.slot(x, y);
                        for ch in 0..4 {
                            slot[ch] = blend_f16(slot[ch], o[ch], BLEND);
                        }
                        acc.set_owner(x, y, ii as u32 + 1);
                        frags += 1;
                    });
                }
            }
        }
    }
    frags
}

/// The per-texel light lists of the frame (TexLightId / TexLightW / TexLightIsLit as CS 7348 keeps them; the cleared id is
/// 0xffff, the cleared weight 0).
pub struct Lists {
    pub w: u32,
    pub h: u32,
    pub l: Vec<LightList>,
}

impl Lists {
    pub fn cleared(w: u32, h: u32) -> Lists {
        Lists { w, h, l: vec![LightList { id: [0xffff; 8], w8: [0; 8], lit8: [0; 8] }; (w * h) as usize] }
    }
    /// The three UAV textures' bytes as the capture exports them: TexLightId (4 R16G16 slices: slots (2s, 2s+1) as lo / hi),
    /// TexLightW and TexLightIsLit (2 RGBA8 slices: slots 4s..4s+3 as the bytes).
    pub fn id_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.l.len() * 16);
        for s in 0..4 {
            for t in &self.l {
                out.extend_from_slice(&t.id[2 * s].to_le_bytes());
                out.extend_from_slice(&t.id[2 * s + 1].to_le_bytes());
            }
        }
        out
    }
    pub fn w_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.l.len() * 8);
        for s in 0..2 {
            for t in &self.l {
                out.extend_from_slice(&t.w8[4 * s..4 * s + 4]);
            }
        }
        out
    }
    pub fn lit_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.l.len() * 8);
        for s in 0..2 {
            for t in &self.l {
                out.extend_from_slice(&t.lit8[4 * s..4 * s + 4]);
            }
        }
        out
    }
    /// The lists from the captured UAV payloads (the DDS bodies of 9771 / 9775 / 9779: slice-major, tight rows).
    pub fn from_bytes(w: u32, h: u32, id: &[u8], wt: &[u8], lit: &[u8]) -> Lists {
        let n = (w * h) as usize;
        let mut l = vec![LightList { id: [0xffff; 8], w8: [0; 8], lit8: [0; 8] }; n];
        for s in 0..4 {
            for i in 0..n {
                let o = (s * n + i) * 4;
                if o + 4 <= id.len() {
                    l[i].id[2 * s] = u16::from_le_bytes([id[o], id[o + 1]]);
                    l[i].id[2 * s + 1] = u16::from_le_bytes([id[o + 2], id[o + 3]]);
                }
            }
        }
        for s in 0..2 {
            for i in 0..n {
                let o = (s * n + i) * 4;
                if o + 4 <= wt.len() {
                    l[i].w8[4 * s..4 * s + 4].copy_from_slice(&wt[o..o + 4]);
                }
                if o + 4 <= lit.len() {
                    l[i].lit8[4 * s..4 * s + 4].copy_from_slice(&lit[o..o + 4]);
                }
            }
        }
        Lists { w, h, l }
    }
}

/// CS 7348 over the lamp's touched rectangle grown by the dilation ring: the lamp enters the lists where its weight byte is
/// non-zero. (The game dispatches over the whole target; a texel outside the ring has coverage 0 in itself and its ring, so
/// nothing changes there.)
pub fn resolve_lists(acc: &Accum, lists: &mut Lists, light_id: u16) -> u64 {
    let Some([x0, y0, x1, y1]) = acc.touched else { return 0 };
    let (x0, y0) = (x0.saturating_sub(1), y0.saturating_sub(1));
    let (x1, y1) = ((x1 + 1).min(acc.w), (y1 + 1).min(acc.h));
    let sum = |x: i32, y: i32| acc.sum(x, y);
    let mut n = 0u64;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * lists.w + x) as usize;
            let before = lists.l[i];
            let after = cs_7348(&sum, x as i32, y as i32, light_id, before);
            if after != before {
                lists.l[i] = after;
                n += 1;
            }
        }
    }
    n
}

/// The probe volume of the pass (R8 UNORM, 96 × 80 × 32 on stpad) and the probe light lists.
pub struct ProbeState {
    pub n: [u32; 3],
    /// The lamp's R8 volume after its probe draw (cleared per lamp), as bytes.
    pub volume: Vec<u8>,
    pub lists: Vec<ProbeLightList>,
    pub touched: Vec<usize>,
}

impl ProbeState {
    pub fn new(n: [u32; 3]) -> ProbeState {
        let c = (n[0] * n[1] * n[2]) as usize;
        ProbeState { n, volume: vec![0; c], lists: vec![ProbeLightList { id: [0xffff; 8], w8: [0; 8] }; c], touched: Vec::new() }
    }
    pub fn index(&self, x: u32, y: u32, z: u32) -> usize {
        ((z * self.n[1] + y) * self.n[0] + x) as usize
    }
}

/// The chunk range of a lamp: the atlas indices whose world cell can hold a lit sample — the lamp's box ± (R + a cell) in the
/// chunk's ProbeStWorld; None when the lamp misses the chunk.
pub fn probe_range(rec: &crate::probechunk::ChunkRecord, lamp: &Lamp) -> Option<([i32; 3], [i32; 3])> {
    let mut lo = [0i32; 3];
    let mut hi = [0i32; 3];
    for k in 0..3 {
        let r = lamp.r_eff + rec.cell[k];
        let a = ((lamp.light.pos[k] - r - rec.origin[k]) / rec.cell[k]).floor() as i32;
        let b = ((lamp.light.pos[k] + r - rec.origin[k]) / rec.cell[k]).ceil() as i32;
        lo[k] = a.max(rec.amin[k]);
        hi[k] = b.min(rec.amax[k]);
        if lo[k] > hi[k] {
            return None;
        }
    }
    Some((lo, hi))
}

/// UNORM8 of a render-target store (round to nearest).
pub fn unorm8_rt(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8
}

/// THE PROBE PASS of one lamp: PS 7351 over every chunk the lamp reaches (blend MAX into the R8 volume cleared per lamp),
/// then CS 7357 per touched probe into the probe lists.
/// `probe_pass` split for the parallel frame (perf 8): the lamp's R8 volume values — PS 7351 at every probe of its range in
/// every chunk, the max per probe as the volume's `b > volume[i]` writes leave it — as the list of (probe, byte) with a non-zero
/// byte, computed on a worker with its own scratch volume (the same size as the frame's, left clear). No dependence on the
/// frame's lists, so lamps can run side by side; the lists then take the lamps in order (`probe_apply`).
pub fn probe_values(chunks: &[crate::probechunk::ChunkRecord], lamp: &Lamp, shadow: &FlatCubeMap, scratch: &mut ProbeState) -> Vec<(u32, u8)> {
    let sample = |uv: [f32; 2], r: f32| shadow_sample(shadow, uv, r);
    let st = scratch;
    for rec in chunks {
        let Some((lo, hi)) = probe_range(rec, lamp) else { continue };
        let cb = lamp.probe_cb([[rec.cell[0], rec.origin[0]], [rec.cell[1], rec.origin[1]], [rec.cell[2], rec.origin[2]]]);
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    if x < 0 || y < 0 || z < 0 || x as u32 >= st.n[0] || y as u32 >= st.n[1] || z as u32 >= st.n[2] {
                        continue;
                    }
                    let v = ps_7351(&cb, x as u32, y as u32, z as u32, &sample);
                    let b = unorm8_rt(v);
                    let i = st.index(x as u32, y as u32, z as u32);
                    if b > st.volume[i] {
                        if st.volume[i] == 0 {
                            st.touched.push(i);
                        }
                        st.volume[i] = b;
                    }
                }
            }
        }
    }
    let out: Vec<(u32, u8)> = st.touched.iter().map(|&i| (i as u32, st.volume[i])).collect();
    for &i in &st.touched {
        st.volume[i] = 0;
    }
    st.touched.clear();
    out
}

/// `probe_pass`'s second half: the lamp's values into the frame's volume (cleared of the previous lamp's, as the pass leaves it)
/// and its lists (CS 7357, in lamp order). Returns the lists changed.
pub fn probe_apply(st: &mut ProbeState, lamp_id: u16, vals: &[(u32, u8)]) -> u64 {
    for &i in &st.touched {
        st.volume[i] = 0;
    }
    st.touched.clear();
    let mut lit = 0u64;
    for &(i, b) in vals {
        let i = i as usize;
        st.volume[i] = b;
        st.touched.push(i);
        let w = b as f32 / 255.0;
        let l = cs_7357(w, lamp_id, st.lists[i]);
        if l != st.lists[i] {
            st.lists[i] = l;
            lit += 1;
        }
    }
    lit
}

pub fn probe_pass(chunks: &[crate::probechunk::ChunkRecord], lamp: &Lamp, shadow: &FlatCubeMap, st: &mut ProbeState) -> u64 {
    let sample = |uv: [f32; 2], r: f32| shadow_sample(shadow, uv, r);
    for &i in &st.touched {
        st.volume[i] = 0;
    }
    st.touched.clear();
    let mut lit = 0u64;
    for rec in chunks {
        let Some((lo, hi)) = probe_range(rec, lamp) else { continue };
        let cb = lamp.probe_cb([[rec.cell[0], rec.origin[0]], [rec.cell[1], rec.origin[1]], [rec.cell[2], rec.origin[2]]]);
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    if x < 0 || y < 0 || z < 0 || x as u32 >= st.n[0] || y as u32 >= st.n[1] || z as u32 >= st.n[2] {
                        continue;
                    }
                    let v = ps_7351(&cb, x as u32, y as u32, z as u32, &sample);
                    let b = unorm8_rt(v);
                    let i = st.index(x as u32, y as u32, z as u32);
                    if b > st.volume[i] {
                        if st.volume[i] == 0 {
                            st.touched.push(i);
                        }
                        st.volume[i] = b;
                    }
                }
            }
        }
    }
    for &i in &st.touched {
        let w = st.volume[i] as f32 / 255.0;
        let l = cs_7357(w, lamp.id, st.lists[i]);
        if l != st.lists[i] {
            st.lists[i] = l;
            lit += 1;
        }
    }
    lit
}

/// How the lists become the frame-1 image (RE 7 is pinning the game's compose dispatch; the candidates until then).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposeRule {
    /// Σ over the list of the √-decoded weights (w8/255)² × the lamp's LightRgb.
    SumDecoded,
    /// Σ of the stored bytes/255 (√w) × LightRgb — the byte itself as the value.
    SumSqrt,
    /// max over the list of (w8/255)² × LightRgb.
    MaxDecoded,
}

/// THE COMPOSE: the frame-1 image (w × h RGBA, alpha 1 where any lamp reached) from the per-texel lists and the lamps' colours.
pub fn compose(lists: &Lists, lamps: &[Lamp], rule: ComposeRule) -> crate::passdiff::Buf {
    let mut out = crate::passdiff::Buf::new(lists.w, lists.h, 4);
    // the lamps by id (the gate may have dropped some, so id ≠ index)
    let by_id: std::collections::HashMap<u16, &Lamp> = lamps.iter().map(|l| (l.id, l)).collect();
    for y in 0..lists.h {
        for x in 0..lists.w {
            let t = &lists.l[(y * lists.w + x) as usize];
            let mut acc = [0.0f32; 3];
            let mut any = false;
            for j in 0..8 {
                if t.w8[j] == 0 || t.id[j] == 0xffff {
                    continue;
                }
                let Some(lamp) = by_id.get(&t.id[j]).copied() else { continue };
                any = true;
                let s = t.w8[j] as f32 / 255.0;
                let wv = match rule {
                    ComposeRule::SumDecoded | ComposeRule::MaxDecoded => s * s,
                    ComposeRule::SumSqrt => s,
                };
                for c in 0..3usize {
                    let v = wv * lamp.rgb[c];
                    acc[c] = if rule == ComposeRule::MaxDecoded { acc[c].max(v) } else { acc[c] + v };
                }
            }
            for c in 0..3u32 {
                out.set(x, y, c, acc[c as usize]);
            }
            out.set(x, y, 3, if any { 1.0 } else { 0.0 });
        }
    }
    out
}

/// The image max (the frame record's MaxHDR: the max-reduce over the three channels, f16 like the frame-0 reduce).
pub fn image_max(img: &crate::passdiff::Buf) -> f32 {
    let mut m = 0.0f32;
    for y in 0..img.h {
        for x in 0..img.w {
            for c in 0..3 {
                m = m.max(img.get(x, y, c));
            }
        }
    }
    quantise_f16(m, Rounding::NearestEven)
}

/// One lamp's outputs kept for the checks.
pub struct LampResult {
    pub drawn: Vec<usize>,
    pub frags: u64,
    pub list_updates: u64,
    pub probe_updates: u64,
}

/// THE FRAME: every lamp through steps 1–5; `keep_accum_for` = a lamp id whose accumulation and flat cube are returned
/// un-cleared (the check against the capture's exports). Returns the lists, the probe state and the kept intermediates.
pub struct FrameOut {
    pub lists: Lists,
    pub probes: ProbeState,
    pub kept: Option<(Accum, FlatCubeMap)>,
    pub results: Vec<LampResult>,
}

/// One lamp's independent work (perf 8): the cull, the casters, the flat cube, the 9-jitter draw on a worker's own target,
/// and CS 7348's weights at the texels it can change; the frame's lists then take the lamps in order (`cs_7348_insert`).
struct LampWork {
    drawn: Vec<usize>,
    n_casters: usize,
    frags: u64,
    weights: Vec<(u32, u8, u8)>,
    probe_vals: Vec<(u32, u8)>,
    /// The worker's seconds on this lamp: cull + casters, the flat cube, the probes, the draw, the weights.
    secs: [f32; 5],
}

/// How many lamps are drawn at once (each worker holds a full accumulation target: 64 MB at 2048²); LMTOOL_LAMP_WORKERS=N.
fn lamp_workers(n_lamps: usize) -> usize {
    let threads = crate::pool::pool().threads.max(1);
    let cap = std::env::var("LMTOOL_LAMP_WORKERS").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(64);
    threads.min(cap).min(n_lamps).max(1)
}

pub fn run_frame(gl: &crate::layout::GameLayout, sc: &LmScene, lamps: &[Lamp], chunks: &[crate::probechunk::ChunkRecord], probe_n: [u32; 3], target: (u32, u32), keep_accum_for: Option<u16>, mut lists: Lists, log: &mut dyn FnMut(&str)) -> FrameOut {
    let (w, h) = target;
    // THE LAMPS IN PARALLEL (perf 8): a lamp's cull, casters, flat-cube shadow and 9-jitter draw depend on nothing but the
    // lamp, so K workers each draw lamps on their own accumulation target and hand the touched rectangle over; the frame's
    // per-texel lists (CS 7348) and the probe volume / lists (PS 7351 / CS 7357) take the lamps IN ORDER on this thread, as
    // before — the same bytes. The serial loop stays for the diagnostics that keep a lamp's full target (`keep_accum_for`)
    // and under LMTOOL_LAMPS_SERIAL=1. (982 lamps on tiny 16 were 287 s, 0.29 s each, one thread.)
    let parallel = keep_accum_for.is_none() && std::env::var_os("LMTOOL_LAMPS_SERIAL").is_none() && lamps.len() > 1;
    if parallel {
        let k = lamp_workers(lamps.len());
        let n = lamps.len();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let slots: Vec<std::sync::Mutex<Option<LampWork>>> = (0..n).map(|_| std::sync::Mutex::new(None)).collect();
        let done = std::sync::atomic::AtomicUsize::new(0);
        let mut probes = ProbeState::new(probe_n);
        let mut results = Vec::with_capacity(n);
        let t0 = std::time::Instant::now();
        std::thread::scope(|sc_| {
            for _ in 0..k {
                sc_.spawn(|| {
                    let mut acc = PagedAccum::new(w, h);
                    let mut visited = vec![0u64; ((w * h) as usize + 63) / 64];
                    let mut probe_scratch = ProbeState::new(probe_n);
                    loop {
                        let li = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if li >= n { break; }
                        let lamp = &lamps[li];
                        let t = std::time::Instant::now();
                        let drawn = cull(gl, sc, lamp);
                        let tris = casters(sc, &drawn);
                        let t1 = t.elapsed().as_secs_f32();
                        let shadow = render_flat_cube(lamp.light.pos, lamp.r_eff, lamp.face_size, &tris, true);
                        let t2 = t.elapsed().as_secs_f32();
                        let cb = lamp.light_cb();
                        let probe_vals = probe_values(chunks, lamp, &shadow, &mut probe_scratch);
                        let t3 = t.elapsed().as_secs_f32();
                        let frags = draw_lamp(sc, &drawn, &cb, &shadow, &mut acc);
                        let t4 = t.elapsed().as_secs_f32();
                        let weights = acc.lamp_weights(&mut visited);
                        acc.clear();
                        let t5 = t.elapsed().as_secs_f32();
                        *slots[li].lock().unwrap() = Some(LampWork { drawn, n_casters: tris.len(), frags, weights, probe_vals, secs: [t1, t2 - t1, t3 - t2, t4 - t3, t5 - t4] });
                        done.fetch_add(1, std::sync::atomic::Ordering::Release);
                    }
                });
            }
            // the consumer, in lamp order
            let (mut t_wait, mut t_probe, mut t_lists) = (0.0f64, 0.0f64, 0.0f64);
            let mut worker_secs = [0.0f64; 5];
            for li in 0..n {
                let tw = std::time::Instant::now();
                let work = loop {
                    if let Some(wk) = slots[li].lock().unwrap().take() { break wk; }
                    std::thread::sleep(std::time::Duration::from_micros(200));
                };
                t_wait += tw.elapsed().as_secs_f64();
                let lamp = &lamps[li];
                let tp = std::time::Instant::now();
                let probe_updates = probe_apply(&mut probes, lamp.id, &work.probe_vals);
                t_probe += tp.elapsed().as_secs_f64();
                let tl = std::time::Instant::now();
                // the lists, in lamp order: the same insertion `resolve_lists` made, at the texels whose weight is not 0
                let mut list_updates = 0u64;
                for &(i, w8, lit8) in &work.weights {
                    let before = lists.l[i as usize];
                    let after = crate::locallight::cs_7348_insert(before, lamp.id, w8, lit8);
                    if after != before {
                        lists.l[i as usize] = after;
                        list_updates += 1;
                    }
                }
                t_lists += tl.elapsed().as_secs_f64();
                for (a, b) in worker_secs.iter_mut().zip(work.secs) { *a += b as f64; }
                if li % 25 == 0 || li + 1 == n {
                    log(&format!("lamp {}/{} id {} ({}): {} records drawn, {} casters, {} fragments, {list_updates} list texels, {probe_updates} probes ({:.1} s; {k} workers; the consumer waited {t_wait:.1} s, probes {t_probe:.1} s, lists {t_lists:.1} s; worker-seconds so far: cull+casters {:.1}, flat cube {:.1}, probes {:.1}, draw {:.1}, weights {:.1})", li + 1, n, lamp.id, lamp.owner, work.drawn.len(), work.n_casters, work.frags, t0.elapsed().as_secs_f32(), worker_secs[0], worker_secs[1], worker_secs[2], worker_secs[3], worker_secs[4]));
                }
                results.push(LampResult { drawn: work.drawn, frags: work.frags, list_updates, probe_updates });
            }
        });
        return FrameOut { lists, probes, kept: None, results };
    }
    let mut acc = Accum::new(w, h);
    let mut probes = ProbeState::new(probe_n);
    let mut kept = None;
    let mut results = Vec::with_capacity(lamps.len());
    let t0 = std::time::Instant::now();
    for (li, lamp) in lamps.iter().enumerate() {
        let drawn = cull(gl, sc, lamp);
        let tris = casters(sc, &drawn);
        let shadow = render_flat_cube(lamp.light.pos, lamp.r_eff, lamp.face_size, &tris, true);
        let cb = lamp.light_cb();
        let probe_updates = probe_pass(chunks, lamp, &shadow, &mut probes);
        let frags = draw_lamp(sc, &drawn, &cb, &shadow, &mut acc);
        let list_updates = resolve_lists(&acc, &mut lists, lamp.id);
        if li % 25 == 0 || li + 1 == lamps.len() {
            log(&format!("lamp {}/{} id {} ({}): {} records drawn, {} casters, {frags} fragments, {list_updates} list texels, {probe_updates} probes ({:.1} s)", li + 1, lamps.len(), lamp.id, lamp.owner, drawn.len(), tris.len(), t0.elapsed().as_secs_f32()));
        }
        results.push(LampResult { drawn, frags, list_updates, probe_updates });
        if keep_accum_for == Some(lamp.id) {
            let mut snap = Accum::new(w, h);
            snap.px.copy_from_slice(&acc.px);
            snap.touched = acc.touched;
            snap.owner = acc.owner.clone();
            kept = Some((snap, shadow));
        }
        acc.clear_touched();
    }
    FrameOut { lists, probes, kept, results }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flat_cube_faces_are_the_captured_cbuffer() {
        let f = flat_cube_faces(173, 4096);
        let cap = LightCb::stpad_f4936_eid34();
        for k in 0..6 {
            assert_eq!(f[k], cap.faces[k], "face {k}");
        }
    }

    #[test]
    fn the_lamp_cbuffer_is_the_captured_one() {
        // the RoadBorderSpot of block 97 (lamp A): the file fields → the eid 34 cbuffer
        let l = LightDef { pos: [1504.5673828125, 23.358840942382812, 1655.9000244140625], dir: [0.9939168691635132, -0.11013312637805939, -3.9683811792201595e-09], color: [0.945, 0.929, 0.886], intensity: 1.1, radius: 40.0, cone: (140.0, 170.0), hyper2: [-1.24, 0.206], att_htnlr: [0.0, 8.121213], ..Default::default() };
        let lamp = Lamp::new(242, "clipE waterfccenter of block 97", l);
        let cb = lamp.light_cb();
        let cap = LightCb::stpad_f4936_eid34();
        let close = |a: f32, b: f32| (a - b).abs() <= 4.0 * f32::EPSILON * b.abs().max(1e-6);
        assert!(close(cb.inv_radius2, cap.inv_radius2), "{} vs {}", cb.inv_radius2, cap.inv_radius2);
        // the game's cosf(70°) sits 2 ulps under Rust's (0.34202007 vs 0.34202014: its libm); InvCosRange follows at 3 ulps
        assert!((cb.inv_cos_range - cap.inv_cos_range).abs() <= 8.0 * f32::EPSILON * cap.inv_cos_range, "{} vs {}", cb.inv_cos_range, cap.inv_cos_range);
        assert!(close(cb.cos_outer, cap.cos_outer), "{} vs {}", cb.cos_outer, cap.cos_outer);
        assert!(close(cb.z_scale, cap.z_scale) && close(cb.z_trans, cap.z_trans), "{} {} vs {} {}", cb.z_scale, cb.z_trans, cap.z_scale, cap.z_trans);
        for k in 0..3 {
            assert!(close(cb.spot_dir_neg[k], cap.spot_dir_neg[k]), "{k}: {} vs {}", cb.spot_dir_neg[k], cap.spot_dir_neg[k]);
        }
        for k in 0..4 {
            assert!(close(cb.att_hn2[k], cap.att_hn2[k]), "{k}: {} vs {}", cb.att_hn2[k], cap.att_hn2[k]);
        }
        assert_eq!(cb.spot_falloff_back_offset, cap.spot_falloff_back_offset);
        assert_eq!(lamp.face_size, 173);
    }

    #[test]
    fn the_jitter_constants_are_the_captured_ones() {
        // eid 34 (jitter n = 2 of lamp A): LM01_Trans_RasterSS (−0.9998553395271301, 0.9995659589767456)
        let cb = jitter_cb(2, 3072, 2048);
        assert_eq!(cb.trans_ss[0].to_bits(), (-0.9998553395271301f32).to_bits(), "{}", cb.trans_ss[0]);
        assert_eq!(cb.trans_ss[1].to_bits(), (0.9995659589767456f32).to_bits(), "{}", cb.trans_ss[1]);
    }

    #[test]
    fn the_list_bytes_round_trip() {
        let mut l = Lists::cleared(4, 2);
        l.l[5] = LightList { id: [1, 2, 3, 4, 5, 6, 7, 0xffff], w8: [10, 20, 30, 40, 50, 60, 70, 0], lit8: [255, 200, 0, 1, 2, 3, 4, 5] };
        let (a, b, c) = (l.id_bytes(), l.w_bytes(), l.lit_bytes());
        let r = Lists::from_bytes(4, 2, &a, &b, &c);
        assert_eq!(r.l, l.l);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The setup from the map (what `bake --lm-from-map --layout-game` builds) and the checks against the capture
// ---------------------------------------------------------------------------------------------------------------

/// Everything the frame needs from the map and the packs.
pub struct Setup {
    pub scene: crate::geometry::Scene,
    pub gl: crate::layout::GameLayout,
    pub sc: LmScene,
    pub lamps: Vec<Lamp>,
    pub daytime: Option<u32>,
    pub lights_on: Option<bool>,
    pub chunks: Vec<crate::probechunk::ChunkRecord>,
    pub probe_n: [u32; 3],
    pub collection: String,
}

/// The LM scene, the layout's records, the light list (the mood gate) and the probe chunking of a map — the bake's
/// `--lm-from-map --layout-game --pak` path (main.rs) in one call. `paks` = (file, key) pairs (the collection's first),
/// `quality` = the editor quality (3 = q3 → layout quality index 2).
pub fn setup_from_map(map_path: &str, paks: &[(String, String)], collection: &str, quality: u32, log: &mut dyn FnMut(&str)) -> Result<Setup, String> {
    let t0 = std::time::Instant::now();
    let scene = crate::geometry::Scene::from_map(map_path)?;
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let (pp, key) = paks.first().ok_or("--pak FILE:KEY needed")?;
    let base: u32 = 4096;
    let zone = crate::layout::ground_zone(&mf, collection);
    let gl = crate::layout::for_map(map_path, &scene, base, quality.saturating_sub(1), crate::layout::TilePlg::BLUEBAY_SEA, Some((pp.as_str(), key.as_str())), collection, &zone, None)?;
    log(&format!("layout: {} charts / {} records ({:.1} s)", gl.charts.len(), gl.records.len(), t0.elapsed().as_secs_f32()));
    let mut store = mapgeom::store::DataStore::empty();
    for (p, k) in paks {
        store.add_pak(p, k).map_err(|e| format!("pak {p}: {e}"))?;
    }
    let tile_world_y = crate::layout::tile_level(&mf, collection) as f32 * 8.0 + crate::layout::CollectionProfile::of(collection).yoff;
    let tile_mesh = crate::lmmesh::lm_mesh_of_zone(&mut store, collection, &zone)?;
    let files = mapgeom::embedded::files(&mf)?;
    let by_name: std::collections::BTreeMap<String, Vec<u8>> = files.iter().map(|(k, v)| (k.rsplit(['/', '\\']).next().unwrap_or(k).to_string(), v.clone())).collect();
    let tile_plg = gl.records.iter().find(|r| r.class == "tile").map(|r| crate::layout::TilePlg { meter_by_uv: r.meter_by_uv, bounds: r.uv }).unwrap_or(crate::layout::TilePlg::BLUEBAY_SEA);
    let mut sc = crate::lmmesh::lm_scene_from_map_at(&scene, &gl, base, &|name| by_name.get(name).cloned(), tile_mesh, tile_plg, 2048.0, tile_world_y)?;
    let n_ent = crate::lmmesh::lm_scene_add_entities(&mut store, &gl, &mut sc, 2048.0)?;
    log(&format!("LM scene: {} meshes, {} instances ({n_ent} prefab entities), rec_of {} ({:.1} s)", sc.meshes.len(), sc.instances.len(), sc.rec_of.len(), t0.elapsed().as_secs_f32()));
    // the lights and the gate
    let opts = crate::records::BuildOpts { collection: collection.to_string(), zone: Some(zone.clone()), kept: None, tile_level: None, yoff: None, grid: None, items_3d: false, ghost_marks: false, no_block_cells: false, clip_order_sim: false, face_order: None, one_class: Vec::new() };
    let mr = crate::records::build_map_records(map_path, &scene, &mut store, &opts)?;
    let daytime = crate::mapio::daytime(&mf.gbx.body).filter(|v| *v != 0xffff_ffff);
    let gate = crate::moods::BlenderCurve::for_collection(collection);
    let lights_on = daytime.map(|w| gate.local_lights_on(w));
    // THE GATE (RE 10, 2026-09-26): the mood switch gates only the NightOnly lamps (CPlugLight flags bit 0); the others are baked
    // at any DayTime — the lamps kept are the baked ones (LMTOOL_LL_ALL_LAMPS=1 keeps every lamp for a study)
    let all = lamps(&scene.world_lights(), &mr.block_lights);
    let n_all = all.len();
    let n_night = all.iter().filter(|l| l.light.night_only).count();
    let lamps: Vec<Lamp> = if std::env::var_os("LMTOOL_LL_ALL_LAMPS").is_some() { all } else { all.into_iter().filter(|l| daytime.map(|w| crate::moods::lamp_is_baked(l.light.night_only, &gate, w)).unwrap_or(!l.light.night_only)).collect() };
    log(&format!("{n_all} lamps ({} item, {} block/clip), {n_night} NightOnly; DayTime {:?} → the mood switch {} → {} lamps baked", scene.world_lights().len(), mr.block_lights.len(), daytime.map(|w| format!("{w:#x}")), match lights_on { Some(true) => "ON", Some(false) => "OFF", None => "n/a (no DayTime word)" }, lamps.len()));
    // the probe chunking (the bake's probe-boxes rule: quality² > 0.9 records; the collection's offset and level height)
    let recs: Vec<crate::lmtiles::BlockRecord> = gl.records.iter().map(|r| crate::lmtiles::BlockRecord { world: crate::lmtiles::CBox::new(r.centre, r.half), quality: r.quality }).collect();
    let size = [mf.size[0].max(0) as u32, mf.size[1].max(0) as u32, mf.size[2].max(0) as u32];
    let coll_id: u32 = match collection { "Stadium" | "Stadium256" => 26, "GreenCoast" => 15, "RedIsland" => 16, "WhiteShore" => 29, _ => 28 };
    let probe_off = [0.0, crate::probechunk::deco_offsets(coll_id).map(|(_, p)| p + 2.0).unwrap_or(-38.0), 0.0];
    let scene_ch = crate::lmtiles::scene_box(&recs);
    let (_, _, chunking, _) = crate::probechunk::for_records(size, [32.0, 8.0, 32.0], probe_off, crate::probechunk::level_h(coll_id), &recs, &scene_ch, 2048);
    log(&format!("probe atlas {:?}: {} chunks", chunking.atlas, chunking.records.len()));
    Ok(Setup { scene, gl, sc, lamps, daytime, lights_on, chunks: chunking.records.clone(), probe_n: chunking.atlas, collection: collection.to_string() })
}

/// A DDS export's payload (DX10 header → offset 148) and its header fields (w, h, depth, array size, dxgi).
pub fn dds_payload(bytes: &[u8]) -> Result<(&[u8], u32, u32, u32, u32, u32), String> {
    if bytes.len() < 148 || &bytes[..4] != b"DDS " {
        return Err("not a DX10 DDS".into());
    }
    let u = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let (h, w, depth) = (u(12), u(16), u(24).max(1));
    // a DX10 header (fourcc "DX10") adds 20 bytes; the legacy 8-bit luminance header (the R8 probe volume export) does not
    if &bytes[84..88] == b"DX10" {
        let dxgi = u(128);
        let arr = u(140).max(1);
        Ok((&bytes[148..], w, h, depth, arr, dxgi))
    } else {
        Ok((&bytes[128..], w, h, depth, 1, 0))
    }
}

/// The accumulation export (RGBA16F) as an Accum.
pub fn accum_from_dds(bytes: &[u8]) -> Result<Accum, String> {
    let (p, w, h, _, _, _) = dds_payload(bytes)?;
    let mut a = Accum::new(w, h);
    for i in 0..(w * h) as usize {
        for c in 0..4 {
            let o = (i * 4 + c) * 2;
            a.px[i][c] = crate::gpufmt::decode_f16(u16::from_le_bytes([p[o], p[o + 1]]));
        }
        if a.px[i][3] > 0.0 {
            a.touch(i as u32 % w, i as u32 / w);
        }
    }
    Ok(a)
}

/// Two accumulations texel by texel: per channel the identical / within one f16 ulp / beyond counts over the texels either
/// side wrote (alpha > 0), plus the alpha (coverage) agreement = the drawn set and the raster.
pub fn compare_accum(ours: &Accum, game: &Accum) -> String {
    let n = ours.px.len().min(game.px.len());
    let (mut drawn_both, mut drawn_ours_only, mut drawn_game_only) = (0usize, 0usize, 0usize);
    let mut same = [0usize; 4];
    let mut ulp1 = [0usize; 4];
    let mut far = [0usize; 4];
    let mut maxd = [0.0f32; 4];
    let mut far_ex: Option<(usize, [f32; 4], [f32; 4])> = None;
    for i in 0..n {
        let (a, b) = (ours.px[i], game.px[i]);
        let (da, db) = (a[3] > 0.0, b[3] > 0.0);
        if !da && !db {
            continue;
        }
        match (da, db) {
            (true, true) => drawn_both += 1,
            (true, false) => drawn_ours_only += 1,
            (false, true) => drawn_game_only += 1,
            _ => {}
        }
        for c in 0..4 {
            let (x, y) = (a[c], b[c]);
            if x.to_bits() == y.to_bits() || (x == 0.0 && y == 0.0) {
                same[c] += 1;
            } else {
                let ex = crate::gpufmt::encode_f16(x, Rounding::NearestEven) as i32;
                let ey = crate::gpufmt::encode_f16(y, Rounding::NearestEven) as i32;
                if (ex - ey).abs() <= 1 {
                    ulp1[c] += 1;
                } else {
                    far[c] += 1;
                    if far_ex.is_none() && c == 0 { far_ex = Some((i, a, b)); }
                }
            }
            maxd[c] = maxd[c].max((x - y).abs());
        }
    }
    let ex = far_ex.map(|(i, a, b)| format!("; first far texel ({}, {}): ours {:?} game {:?}", i as u32 % ours.w, i as u32 / ours.w, a, b)).unwrap_or_default();
    // the coverage pairs (ours × 9, game × 9) where they differ
    let mut pairs: std::collections::BTreeMap<(i32, i32), usize> = Default::default();
    for i in 0..n {
        let (a, b) = (ours.px[i][3], game.px[i][3]);
        if a == 0.0 && b == 0.0 { continue; }
        let (ka, kb) = ((a * 9.0).round() as i32, (b * 9.0).round() as i32);
        if ka != kb { *pairs.entry((ka, kb)).or_default() += 1; }
    }
    // is the game's shadow channel a count of ninths (a point comparison) or finer (PCF)?
    let (mut ninths, mut finer) = (0usize, 0usize);
    for i in 0..n { let y = game.px[i][1]; if game.px[i][3] > 0.0 && y > 0.0 { let k = y * 9.0; if (k - k.round()).abs() < 0.03 { ninths += 1; } else { finer += 1; } } }
    let ex = format!("{ex}; coverage pairs (ours/9, game/9) differing: {:?}; game shadow channel: {ninths} texels at k/9, {finer} finer", pairs);
    format!("drawn texels: both {drawn_both}, ours only {drawn_ours_only}, game only {drawn_game_only}; per channel (light, shadow, 0, coverage) identical {:?}, within 1 f16 ulp {:?}, beyond {:?}, max |Δ| {:?}{ex}", same, ulp1, far, maxd)
}

/// The flat-cube export (D16 4096²) against ours: the six face tiles, texels identical / ±1 step / beyond, and the count of
/// written texels each side.
pub fn compare_flat_cube(ours: &FlatCubeMap, bytes: &[u8]) -> Result<String, String> {
    let (p, w, _h, _, _, _) = dds_payload(bytes)?;
    let mut s = String::new();
    let (mut tot_same, mut tot_1, mut tot_far, mut ow, mut gw) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for f in 0..6 {
        let (ox, oy) = flat_cube_face_viewport(f, ours.size);
        let (mut same, mut one, mut far, mut o_w, mut g_w, mut plus) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
        let mut ex = String::new();
        for y in 0..ours.size {
            for x in 0..ours.size {
                let o = ((oy + y) as usize * ours.width() as usize + (ox + x) as usize) as usize;
                let g = ((oy + y) as usize * w as usize + (ox + x) as usize) * 2;
                let gv = u16::from_le_bytes([p[g], p[g + 1]]) as i32;
                let ov = (ours.depth[o] * 65535.0).round() as i32;
                if ov > 0 { o_w += 1; }
                if gv > 0 { g_w += 1; }
                let d = ov - gv;
                if d == 0 { same += 1; } else if d.abs() == 1 { one += 1; if d > 0 { plus += 1; } } else { far += 1; if ex.is_empty() { ex = format!(" e.g. ({x}, {y}) ours {ov} game {gv}"); } }
            }
        }
        s.push_str(&format!("  face {f}: identical {same}, ±1 {one} (ours above in {plus}), beyond {far} (written ours {o_w} / game {g_w}){ex}\n"));
        tot_same += same; tot_1 += one; tot_far += far; ow += o_w; gw += g_w;
    }
    s.push_str(&format!("  all faces: identical {tot_same}, ±1 {tot_1}, beyond {tot_far}; written ours {ow} / game {gw}"));
    Ok(s)
}

/// The probe volume export (R8 96 × 80 × 32) against ours.
pub fn compare_probe_volume(ours: &ProbeState, bytes: &[u8]) -> Result<String, String> {
    let (p, w, h, d, _, _) = dds_payload(bytes)?;
    if [w, h, d] != ours.n {
        return Err(format!("probe volume {w}×{h}×{d} vs ours {:?}", ours.n));
    }
    let n = ((w * h * d) as usize).min(p.len());
    let (mut same, mut one, mut far, mut o_nz, mut g_nz) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut ex = String::new();
    for i in 0..n {
        let (a, b) = (ours.volume[i] as i32, p[i] as i32);
        if a > 0 { o_nz += 1; }
        if b > 0 { g_nz += 1; }
        let dd = (a - b).abs();
        if dd == 0 { same += 1; } else if dd == 1 { one += 1; } else { far += 1; if ex.is_empty() { let (x, y, z) = (i as u32 % w, (i as u32 / w) % h, i as u32 / (w * h)); ex = format!(" e.g. ({x}, {y}, {z}) ours {a} game {b}"); } }
    }
    Ok(format!("probe volume: identical {same}, ±1 {one}, beyond {far} of {n}; non-zero ours {o_nz} / game {g_nz}{ex}"))
}

/// Two list states byte by byte (ids, weights, lit bytes), with the texels whose lists differ.
pub fn compare_lists(ours: &Lists, game: &Lists) -> String {
    let n = ours.l.len().min(game.l.len());
    let (mut same, mut diff, mut nonempty) = (0usize, 0usize, 0usize);
    let mut ex = String::new();
    // the differing texels by kind: the lamp entered one side only (ours / game), or both with the weight byte differing by |Δ|
    let (mut only_ours, mut only_game, mut lit_only) = (0usize, 0usize, 0usize);
    let mut dw: std::collections::BTreeMap<i32, usize> = Default::default();
    for i in 0..n {
        let (a, b) = (&ours.l[i], &game.l[i]);
        if b.w8.iter().any(|&w| w > 0) { nonempty += 1; }
        if a == b { same += 1; } else {
            diff += 1;
            if ex.len() < 600 { ex.push_str(&format!("\n    ({}, {}): ours {:?}/{:?}/{:?} game {:?}/{:?}/{:?}", i as u32 % ours.w, i as u32 / ours.w, a.id, a.w8, a.lit8, b.id, b.w8, b.lit8)); }
            // the slot the two sides disagree on: the entry present in one and not the other, else the weight difference
            let ids_a: Vec<u16> = a.id.iter().copied().filter(|&x| x != 0xffff).collect();
            let ids_b: Vec<u16> = b.id.iter().copied().filter(|&x| x != 0xffff).collect();
            let new_a: Vec<u16> = ids_a.iter().copied().filter(|x| !ids_b.contains(x)).collect();
            let new_b: Vec<u16> = ids_b.iter().copied().filter(|x| !ids_a.contains(x)).collect();
            if !new_a.is_empty() && new_b.is_empty() { only_ours += 1; }
            else if new_a.is_empty() && !new_b.is_empty() { only_game += 1; }
            else {
                let mut any_w = false;
                for j in 0..8 { if a.id[j] == b.id[j] && a.id[j] != 0xffff && a.w8[j] != b.w8[j] { *dw.entry(a.w8[j] as i32 - b.w8[j] as i32).or_default() += 1; any_w = true; } }
                if !any_w { lit_only += 1; }
            }
        }
    }
    let mut dwv: Vec<(i32, usize)> = dw.into_iter().collect();
    dwv.sort_by_key(|x| std::cmp::Reverse(x.1));
    format!("lists: {same} texels identical, {diff} differ ({nonempty} non-empty in the game's); of the differing: entered ours-only {only_ours}, game-only {only_game}, lit byte only {lit_only}, weight Δ (ours − game → count, top 12) {:?}{ex}", &dwv[..dwv.len().min(12)])
}

/// The scene's instances with their chart STs recomputed for a `w` × `h` accumulation target (the local-light frame's
/// 3072 × 2048: the layout rects sit 1:1 in its left 2048 columns, so the ST's x terms are over 3072 — the captured draws'
/// clip positions; a captured scene without `st_src` keeps its STs).
pub fn instances_for_target(sc: &LmScene, w: u32, h: u32) -> Vec<crate::sunpass::LmInstance> {
    let mut out = sc.instances.clone();
    for (i, (rect, bounds)) in sc.st_src.iter().enumerate() {
        if i < out.len() {
            let st = crate::peelcolor::chart_st_target(*rect, *bounds, w as f32, h as f32, 2048.0, 2048.0);
            out[i].st = st;
            out[i].st_x_bits = st[0].to_bits();
        }
    }
    out
}

/// The LM scene with the target's STs (a shallow copy of the meshes is avoided: the caller keeps `sc` and passes the
/// instance vector) — `draw_lamp` takes the scene, so build a scene value whose instances are the target's.
pub fn scene_for_target(sc: &LmScene, w: u32, h: u32) -> LmScene {
    LmScene { meshes: sc.meshes.clone(), inst_first: sc.inst_first.clone(), inst_count: sc.inst_count.clone(), instances: instances_for_target(sc, w, h), table: sc.table.clone(), eids: sc.eids.clone(), rec_of: sc.rec_of.clone(), st_src: sc.st_src.clone(), frag_lists: Default::default() /* perf 8: the LM fragment lists are per target size — a fresh set for this frame */ }
}

/// The post-VS export of a captured lighting draw (mesh/e<EID>_vsout.bin, stride 80: o0 xyzw, o1 xyz, o2 xyzw, o3 xyz, o4 xyz,
/// o5 xyz) against our VS 7303 for the same instances: `insts` = our drawn instance indices of the mesh with the draw's index
/// count (matched by world position of the first vertex), per matched instance the max |Δ| of the clip xy (in target
/// texels), the world position and the normal.
pub fn compare_vsout(sc: &LmScene, insts: &[usize], jitter: u32, target: (u32, u32), bytes: &[u8], indices: &[u8]) -> String {
    let n = bytes.len() / 80;
    let f = |i: usize, k: usize| f32::from_le_bytes(bytes[i * 80 + k * 4..i * 80 + k * 4 + 4].try_into().unwrap());
    let ni = indices.len() / 2;
    let per = (0..ni).map(|i| u16::from_le_bytes([indices[2 * i], indices[2 * i + 1]]) as usize).max().map(|m| m + 1).unwrap_or(n);
    let count = if per > 0 { n / per } else { 0 };
    let cb = jitter_cb(jitter, target.0, target.1);
    let mut s = format!("vsout: {n} vertices = {count} instances × {per} (index count {ni})");
    // our meshes with `ni` indices
    let cands: Vec<usize> = insts.iter().copied().filter(|&ii| mesh_of(sc, ii).map(|m| sc.meshes[m].indices.len() == ni).unwrap_or(false)).collect();
    s.push_str(&format!("; ours: {} drawn instances with that index count", cands.len()));
    let mut matched = 0usize;
    for gi in 0..count {
        let gw0 = [f(gi * per, 4), f(gi * per, 5), f(gi * per, 6)];
        // the instance whose first vertex lands there
        let mut best: Option<(usize, f32)> = None;
        for &ii in &cands {
            let m = mesh_of(sc, ii).unwrap();
            let v0 = vs_7303(&sc.meshes[m].verts[0], &sc.instances[ii], &sc.table, &cb);
            let d = (0..3).map(|k| (v0.world[k] - gw0[k]).abs()).fold(0.0f32, f32::max);
            if best.map(|b| d < b.1).unwrap_or(true) { best = Some((ii, d)); }
        }
        let Some((ii, d0)) = best else { continue };
        if d0 > 0.01 { s.push_str(&format!("\n  game instance {gi} (first vertex {gw0:?}): no instance of ours within 1 cm (nearest {d0:.3} m)")); continue; }
        matched += 1;
        let m = mesh_of(sc, ii).unwrap();
        let mesh = &sc.meshes[m];
        let (mut dclip, mut dworld, mut dnorm) = (0.0f32, 0.0f32, 0.0f32);
        let (mut bit_x, mut bit_y) = (0usize, 0usize);
        let mut worst = String::new();
        for (vi, v) in mesh.verts.iter().enumerate().take(per) {
            let o = vs_7303(v, &sc.instances[ii], &sc.table, &cb);
            let g = gi * per + vi;
            let gc = [f(g, 0), f(g, 1)];
            let dcx = ((o.clip[0] - gc[0]) * 0.5 * target.0 as f32).abs();
            let dcy = ((o.clip[1] - gc[1]) * 0.5 * target.1 as f32).abs();
            let dc = dcx.max(dcy);
            if dc > dclip { dclip = dc; worst = format!("v{vi}: ours clip {:?} game {:?} (Δ {dcx:.4}, {dcy:.4} px)", o.clip, gc); }
            if o.clip[0].to_bits() == gc[0].to_bits() { bit_x += 1; }
            if o.clip[1].to_bits() == gc[1].to_bits() { bit_y += 1; }
            for k in 0..3 {
                dworld = dworld.max((o.world[k] - f(g, 4 + k)).abs());
                dnorm = dnorm.max((o.normal[k] - f(g, 11 + k)).abs());
            }
        }
        // the game's ST solved from two vertices of different u / v (our uv = the game's snorm16 stream): clip = 2·S·uv + (2·T + Trans)
        let mut st_note = String::new();
        if per >= 2 {
            let (u0, v0) = (mesh.verts[0].uv[0], mesh.verts[0].uv[1]);
            let ju = (1..per).find(|&j| (mesh.verts[j].uv[0] - u0).abs() > 1e-3);
            let jv = (1..per).find(|&j| (mesh.verts[j].uv[1] - v0).abs() > 1e-3);
            if let (Some(ju), Some(jv)) = (ju, jv) {
                let g = |j: usize, k: usize| f(gi * per + j, k);
                let sx = (g(ju, 0) - g(0, 0)) / (2.0 * (mesh.verts[ju].uv[0] - u0));
                let tx = (g(0, 0) - 2.0 * sx * u0 - cb.trans_ss[0]) / 2.0;
                let sy = (g(jv, 1) - g(0, 1)) / (-2.0 * (mesh.verts[jv].uv[1] - v0));
                let ty = (g(0, 1) - (-2.0) * sy * v0 - cb.trans_ss[1]) / (-2.0);
                let o = sc.instances[ii].st;
                let src = sc.st_src.get(ii).map(|(r, b)| format!(" rect {r:?} bounds {b:?}")).unwrap_or_default();
                st_note = format!("\n     ST ours {:?} | game (solved) [{sx}, {sy}, {tx}, {ty}]; ΔT·W = ({:.4}, {:.4}) texels{src}", o, (o[2] - tx) * target.0 as f32, (o[3] - ty) * target.1 as f32);
            }
        }
        s.push_str(&format!("\n  game instance {gi} ↔ ours {ii}: max |Δclip| {dclip:.5} px (clip bits identical x {bit_x} y {bit_y} of {per}), |Δworld| {dworld:.5} m, |Δnormal| {dnorm:.6}{}{st_note}", if dclip > 0.002 { format!(" — {worst}") } else { String::new() }));
    }
    s.push_str(&format!("\n  {matched} of {count} game instances matched"));
    s
}

/// The instances of ours whose first vertex (through VS 7303) coincides with a captured draw's post-VS instances — the
/// game's grouping of a mesh into two draws (lamp A's eids 64 / 84) recovered from the export.
pub fn instances_of_vsout(sc: &LmScene, insts: &[usize], jitter: u32, target: (u32, u32), bytes: &[u8], indices: &[u8]) -> Vec<usize> {
    let n = bytes.len() / 80;
    let f = |i: usize, k: usize| f32::from_le_bytes(bytes[i * 80 + k * 4..i * 80 + k * 4 + 4].try_into().unwrap());
    let ni = indices.len() / 2;
    let per = (0..ni).map(|i| u16::from_le_bytes([indices[2 * i], indices[2 * i + 1]]) as usize).max().map(|m| m + 1).unwrap_or(n);
    let count = if per > 0 { n / per } else { 0 };
    let cb = jitter_cb(jitter, target.0, target.1);
    let cands: Vec<usize> = insts.iter().copied().filter(|&ii| mesh_of(sc, ii).map(|m| sc.meshes[m].indices.len() == ni).unwrap_or(false)).collect();
    let mut out = Vec::new();
    for gi in 0..count {
        let gw0 = [f(gi * per, 4), f(gi * per, 5), f(gi * per, 6)];
        for &ii in &cands {
            let m = mesh_of(sc, ii).unwrap();
            let v0 = vs_7303(&sc.meshes[m].verts[0], &sc.instances[ii], &sc.table, &cb);
            if (0..3).all(|k| (v0.world[k] - gw0[k]).abs() < 0.001) {
                out.push(ii);
                break;
            }
        }
    }
    out
}

/// The composed frame-1 image against the editor save's frame-1 WebP (decoded to 1024² RGB): our 2048² region encoded by a
/// candidate (`sqrt`: byte = 255·√(v/m); else linear 255·v/m; m = the image max), 2×2-averaged to 1024², chart-normalised
/// like frame 0 (filecheck::chart_normalise over the mapping's rects), then per lit texel the byte difference. Returns the
/// report line and (ours, fb) for a dump.
/// The frame-1 image's standard tail before the encode: the 2048² left part of the composed target (the atlas) dilated eight
/// times with PS 1332 (a texel with coverage < 1e-4 takes the coverage-weighted mean of its eight neighbours — the chart
/// gutters the editor's WebP shows lit), alpha = the texel's coverage (1 where a lamp entered its list).
pub fn frame1_dilated(img: &crate::passdiff::Buf, atlas: u32) -> crate::passdiff::Buf {
    frame1_dilated_n(img, atlas, 8)
}

/// `frame1_dilated` with `passes` dilation passes.
pub fn frame1_dilated_n(img: &crate::passdiff::Buf, atlas: u32, passes: u32) -> crate::passdiff::Buf {
    let mut base = crate::passdiff::Buf::new(atlas, atlas, 4);
    for y in 0..atlas {
        for x in 0..atlas {
            for c in 0..4u32 {
                base.set(x, y, c, img.get(x, y, c));
            }
        }
    }
    let mut d = base;
    for _ in 0..passes {
        d = crate::gpuenc::dilate_ps1332(&d);
    }
    d
}

/// The encode scale the editor's frame bytes imply, per lit chart: fb = round(255·√(v_max/M)) → M = v_max/(fb/255)² with
/// v_max our chart's brightest 2×2-averaged value; the (count, median, lower / upper quartile) over the charts.
pub fn implied_scale(img: &crate::passdiff::Buf, charts: &[(u32, u32, u32, u32)], fb_editor: &[u8]) -> (usize, f32, f32, f32) {
    let mut ms: Vec<f32> = Vec::new();
    for (i, &(x, y, w, h)) in charts.iter().enumerate() {
        let Some(&fb) = fb_editor.get(i) else { continue };
        if fb == 0 || fb == 255 { continue; }
        let (x0, y0, cw, ch) = crate::filecheck::chart_px(x, y, w, h);
        let mut vmax = 0.0f32;
        for py in y0..y0 + ch {
            for px in x0..x0 + cw {
                if 2 * px + 1 >= img.w || 2 * py + 1 >= img.h { continue; }
                for c in 0..3u32 {
                    let mut s = 0.0f32;
                    for dy in 0..2 { for dx in 0..2 { s += img.get(2 * px + dx, 2 * py + dy, c).max(0.0); } }
                    vmax = vmax.max(s * 0.25);
                }
            }
        }
        if vmax > 0.0 {
            let q = fb as f32 / 255.0;
            ms.push(vmax / (q * q));
        }
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = ms.len();
    if n == 0 { return (0, 0.0, 0.0, 0.0); }
    (n, ms[n / 2], ms[n / 4], ms[3 * n / 4])
}

pub fn frame1_compare(img: &crate::passdiff::Buf, editor_rgb: &[u8], charts: &[(u32, u32, u32, u32)], sqrt: bool) -> (String, Vec<u8>, Vec<u8>, Vec<u8>) {
    frame1_compare_scaled(img, editor_rgb, charts, sqrt, image_max(img).max(1e-6))
}

/// `frame1_compare` with an explicit encode scale `m` (byte = 255·√(v/m)).
pub fn frame1_compare_scaled(img: &crate::passdiff::Buf, editor_rgb: &[u8], charts: &[(u32, u32, u32, u32)], sqrt: bool, m: f32) -> (String, Vec<u8>, Vec<u8>, Vec<u8>) {
    let (ow, oh) = (1024u32, 1024u32);
    // LMTOOL_LL_SHIFT=dx,dy: the 2×2 blocks taken at (2x + dx, 2y + dy) — the down-sample's pairing under test
    let (sx, sy): (i64, i64) = std::env::var("LMTOOL_LL_SHIFT").ok().and_then(|v| { let (a, b) = v.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).unwrap_or((0, 0));
    let mut ours = vec![0u8; (ow * oh * 3) as usize];
    for y in 0..oh {
        for x in 0..ow {
            for c in 0..3u32 {
                let mut s = 0.0f32;
                for dy in 0..2i64 {
                    for dx in 0..2i64 {
                        let (px, py) = (2 * x as i64 + dx + sx, 2 * y as i64 + dy + sy);
                        let v = if px < 0 || py < 0 || px >= img.w as i64 || py >= img.h as i64 { 0.0 } else { img.get(px as u32, py as u32, c) / m };
                        let e = if sqrt { v.max(0.0).sqrt() } else { v.max(0.0) };
                        s += e.min(1.0);
                    }
                }
                ours[((y * ow + x) * 3 + c) as usize] = (s * 0.25 * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
            }
        }
    }
    if let Ok(n) = std::env::var("LMTOOL_LL_FILL1024") { if let Ok(n) = n.parse::<u32>() { fill_1024(&mut ours, ow, oh, n); } }
    let pre = ours.clone();
    let fb = crate::filecheck::chart_normalise(&mut ours, ow, oh, charts);
    // through the codec too (libwebp q 91 like the game's blob, decoded by the same decoder as the editor's): LMTOOL_LL_NO_WEBP=1 skips
    if std::env::var_os("LMTOOL_LL_NO_WEBP").is_none() {
        if let Some(enc) = crate::webpenc::encode_rgb(&ours, ow, oh, 91.0) {
            if let Ok(dec) = crate::img::decode_webp(&enc) {
                if dec.px.len() == ours.len() { ours = dec.px; }
            }
        }
    }
    let n = (ow * oh) as usize;
    let (mut lit_both, mut lit_ours, mut lit_ed) = (0usize, 0usize, 0usize);
    let (mut sum_abs, mut within3, mut within8) = (0u64, 0usize, 0usize);
    let mut hist = [0usize; 8];
    for i in 0..n {
        let (a, b) = (ours[3 * i] as i32, editor_rgb[3 * i] as i32);
        let (la, lb) = (a > 0, b > 0);
        if la && lb { lit_both += 1; } else if la { lit_ours += 1; } else if lb { lit_ed += 1; }
        if la || lb {
            let d = (a - b).abs();
            sum_abs += d as u64;
            if d <= 3 { within3 += 1; }
            if d <= 8 { within8 += 1; }
            hist[(d.min(255) as usize * 8 / 256).min(7)] += 1;
        }
    }
    let lit = lit_both + lit_ours + lit_ed;
    // the chart INTERIORS (the rect without its gutter row / column and its last row / column: where the tail's dilation and the
    // 2×2 straddle cannot reach) — the local-light pass's own agreement
    let (mut in_n, mut in_abs, mut in3, mut in8) = (0usize, 0u64, 0usize, 0usize);
    for &(x, y, cw, ch) in charts {
        let (x0, y0, pw, ph) = crate::filecheck::chart_px(x, y, cw, ch);
        if pw < 4 || ph < 4 { continue; }
        for py in y0 + 1..(y0 + ph - 1).min(oh) {
            for px in x0 + 1..(x0 + pw - 1).min(ow) {
                let i = (py * ow + px) as usize;
                let (a, b) = (ours[3 * i] as i32, editor_rgb[3 * i] as i32);
                if a == 0 && b == 0 { continue; }
                in_n += 1;
                let d = (a - b).abs();
                in_abs += d as u64;
                if d <= 3 { in3 += 1; }
                if d <= 8 { in8 += 1; }
            }
        }
    }
    let line = format!("frame-1 WebP ({}): lit texels both {lit_both}, ours only {lit_ours}, editor only {lit_ed}; over the {lit} lit: mean |Δ| {:.2}, within ±3 {:.1} %, within ±8 {:.1} %; |Δ| histogram (32-wide bins) {:?}; chart interiors ({in_n} lit texels): mean |Δ| {:.2}, within ±3 {:.1} %, within ±8 {:.1} %; image max {m}", if sqrt { "sqrt encode" } else { "linear encode" }, sum_abs as f64 / lit.max(1) as f64, 100.0 * within3 as f64 / lit.max(1) as f64, 100.0 * within8 as f64 / lit.max(1) as f64, hist, in_abs as f64 / in_n.max(1) as f64, 100.0 * in3 as f64 / in_n.max(1) as f64, 100.0 * in8 as f64 / in_n.max(1) as f64);
    (line, ours, fb, pre)
}

/// Per-chart diagnostics of the frame-1 image against the editor's: which charts the editor lights and we do not (by record
/// class, with examples), and the ratio of our pre-normalisation chart max byte to the editor's fb1 over the charts both light.
pub fn frame1_chart_report(ours_prenorm: &[u8], editor_rgb: &[u8], charts: &[(u32, u32, u32, u32)], fb_editor: &[u8], records: &[crate::records::Rec]) -> String {
    let (w, h) = (1024u32, 1024u32);
    let lit_frac = |rgb: &[u8], x0: u32, y0: u32, cw: u32, ch: u32| -> (usize, usize, u8) {
        let (mut n, mut lit, mut m) = (0usize, 0usize, 0u8);
        for y in y0..(y0 + ch).min(h) {
            for x in x0..(x0 + cw).min(w) {
                let o = ((y * w + x) * 3) as usize;
                n += 1;
                let v = rgb[o].max(rgb[o + 1]).max(rgb[o + 2]);
                if v > 0 { lit += 1; }
                m = m.max(v);
            }
        }
        (n, lit, m)
    };
    let mut missing: std::collections::BTreeMap<&str, (usize, Vec<String>, Vec<u8>)> = Default::default();
    let mut extra = 0usize;
    let mut ratios: Vec<f32> = Vec::new();
    let mut both = 0usize;
    let mut by_class: std::collections::BTreeMap<&str, (usize, usize, Vec<String>, usize)> = Default::default();
    for (i, &(x, y, cw, ch)) in charts.iter().enumerate() {
        let (x0, y0, pw, ph) = crate::filecheck::chart_px(x, y, cw, ch);
        let (_, le, me) = lit_frac(editor_rgb, x0, y0, pw, ph);
        let (_, lo, mo) = lit_frac(ours_prenorm, x0, y0, pw, ph);
        let r = records.get(i);
        if le > 0 && lo == 0 {
            let e = missing.entry(r.map(|r| r.class).unwrap_or("?")).or_default();
            e.0 += 1;
            if e.1.len() < 4 { e.1.push(format!("chart {i} rect ({x}, {y}, {cw}, {ch}) editor lit {le} px max {me} fb1 {}{}", fb_editor.get(i).copied().unwrap_or(0), r.map(|r| format!(" centre {:?} half {:?} q {}", r.centre, r.half, r.quality)).unwrap_or_default())); }
            e.2.push(fb_editor.get(i).copied().unwrap_or(0));
        } else if lo > 0 && le == 0 {
            extra += 1;
        } else if lo > 0 && le > 0 {
            both += 1;
            let fe = fb_editor.get(i).copied().unwrap_or(0);
            if fe > 0 && fe < 255 && mo > 0 {
                let ratio = mo as f32 / fe as f32;
                ratios.push(ratio);
                let class = r.map(|r| r.class).unwrap_or("?");
                let e = by_class.entry(class).or_default();
                e.0 += 1;
                if ratio < 0.85 { e.1 += 1; if e.2.len() < 3 { e.2.push(format!("chart {i} rect ({x}, {y}, {cw}, {ch}) ours max {mo} editor fb1 {fe} (ratio {ratio:.3}){}", r.map(|r| format!(" centre {:?} q {}", r.centre, r.quality)).unwrap_or_default())); } }
                if ratio > 1.15 { e.3 += 1; }
            }
        }
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = ratios.len();
    let q = |p: f64| if n == 0 { 0.0 } else { ratios[((n as f64 - 1.0) * p) as usize] };
    let mut s = format!("charts lit by both {both}, by us only {extra}; the editor lights and we do not:\n");
    for (c, (k, ex, fbs)) in &missing {
        let mut fbs = fbs.clone(); fbs.sort_unstable();
        s.push_str(&format!("  {c}: {k} charts (editor fb1 quartiles {} / {} / {}, max {})\n", fbs[fbs.len() / 4], fbs[fbs.len() / 2], fbs[3 * fbs.len() / 4], fbs[fbs.len() - 1]));
        for e in ex { s.push_str(&format!("    {e}\n")); }
    }
    s.push_str(&format!("  our chart max byte / the editor's fb1 over {n} charts: quartiles {:.3} / {:.3} / {:.3}, 5–95 % {:.3}–{:.3}\n", q(0.25), q(0.5), q(0.75), q(0.05), q(0.95)));
    for (c, (k, low, ex, high)) in &by_class {
        s.push_str(&format!("  {c}: {k} charts, {low} with ratio < 0.85, {high} with ratio > 1.15\n"));
        for e in ex { s.push_str(&format!("    {e}\n")); }
    }
    s
}

/// The accumulation's disagreements (light or shadow beyond one f16 ulp) attributed to the LM instance that drew the texel:
/// per (mesh index count, record class) the count of drawn texels and of disagreeing ones.
pub fn attribute_accum_diff(ours: &Accum, game: &Accum, sc: &LmScene, records: &[crate::records::Rec]) -> String {
    if ours.owner.is_empty() { return "no owner map".into(); }
    let mut by: std::collections::BTreeMap<(usize, &str), (usize, usize, usize)> = Default::default();
    for i in 0..ours.px.len().min(game.px.len()) {
        let o = ours.owner[i];
        if o == 0 { continue; }
        let ii = (o - 1) as usize;
        let m = mesh_of(sc, ii).map(|m| sc.meshes[m].indices.len()).unwrap_or(0);
        let class = sc.rec_of.get(ii).and_then(|&k| records.get(k)).map(|r| r.class).unwrap_or("?");
        let e = by.entry((m, class)).or_default();
        e.0 += 1;
        let (a, b) = (ours.px[i], game.px[i]);
        let far = |c: usize| { let ex = crate::gpufmt::encode_f16(a[c], Rounding::NearestEven) as i32; let ey = crate::gpufmt::encode_f16(b[c], Rounding::NearestEven) as i32; (ex - ey).abs() > 1 };
        if far(0) { e.1 += 1; }
        if far(1) { e.2 += 1; }
    }
    let mut s = String::from("disagreements by the drawing instance's (mesh index count, class): drawn texels / light beyond 1 ulp / shadow beyond 1 ulp\n");
    for ((m, c), (n, l, sh)) in &by { s.push_str(&format!("  {m} idx {c}: {n} / {l} / {sh}\n")); }
    s
}

/// A captured draw's post-VS instances against ours as VERTEX SETS (the game's vertex order may differ from the LM
/// builder's): per game instance the nearest instance of ours by world bounding box, then how many of its vertices have a
/// vertex of ours at the same world position (< 1 mm) with the same normal (< 1e-3) and the same clip position (< 1e-3 px).
pub fn compare_vsout_sets(sc: &LmScene, insts: &[usize], jitter: u32, target: (u32, u32), bytes: &[u8], indices: &[u8]) -> String {
    let n = bytes.len() / 80;
    let f = |i: usize, k: usize| f32::from_le_bytes(bytes[i * 80 + k * 4..i * 80 + k * 4 + 4].try_into().unwrap());
    let ni = indices.len() / 2;
    let per = (0..ni).map(|i| u16::from_le_bytes([indices[2 * i], indices[2 * i + 1]]) as usize).max().map(|m| m + 1).unwrap_or(n);
    let count = if per > 0 { n / per } else { 0 };
    let cb = jitter_cb(jitter, target.0, target.1);
    let cands: Vec<usize> = insts.iter().copied().filter(|&ii| mesh_of(sc, ii).map(|m| sc.meshes[m].indices.len() == ni).unwrap_or(false)).collect();
    let mut s = format!("vsout sets ({ni} indices, {count} game instances × {per} vertices; ours with that index count: {})", cands.len());
    for gi in 0..count {
        let mut gb = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in gi * per..(gi + 1) * per { for k in 0..3 { let p = f(v, 4 + k); gb.0[k] = gb.0[k].min(p); gb.1[k] = gb.1[k].max(p); } }
        let mut best: Option<(usize, f32, Vec<Vs7303Out>)> = None;
        for &ii in &cands {
            let m = mesh_of(sc, ii).unwrap();
            let vs: Vec<Vs7303Out> = sc.meshes[m].verts.iter().map(|v| vs_7303(v, &sc.instances[ii], &sc.table, &cb)).collect();
            let mut ob = ([f32::MAX; 3], [f32::MIN; 3]);
            for v in &vs { for k in 0..3 { ob.0[k] = ob.0[k].min(v.world[k]); ob.1[k] = ob.1[k].max(v.world[k]); } }
            let d = (0..3).map(|k| (ob.0[k] - gb.0[k]).abs().max((ob.1[k] - gb.1[k]).abs())).fold(0.0f32, f32::max);
            if best.as_ref().map(|b| d < b.1).unwrap_or(true) { best = Some((ii, d, vs)); }
        }
        let Some((ii, d, vs)) = best else { continue };
        let (mut pos_hit, mut nrm_hit, mut clip_hit) = (0usize, 0usize, 0usize);
        let mut worst_clip = 0.0f32;
        for v in gi * per..(gi + 1) * per {
            let gw = [f(v, 4), f(v, 5), f(v, 6)];
            let gn = [f(v, 11), f(v, 12), f(v, 13)];
            let gc = [f(v, 0), f(v, 1)];
            // the vertex of ours at that position with the closest normal
            let mut hit: Option<&Vs7303Out> = None;
            for o in &vs {
                if (0..3).all(|k| (o.world[k] - gw[k]).abs() < 1e-3) {
                    if hit.map(|h| (0..3).map(|k| (o.normal[k] - gn[k]).abs()).sum::<f32>() < (0..3).map(|k| (h.normal[k] - gn[k]).abs()).sum::<f32>()).unwrap_or(true) { hit = Some(o); }
                }
            }
            if let Some(o) = hit {
                pos_hit += 1;
                if (0..3).all(|k| (o.normal[k] - gn[k]).abs() < 1e-3) { nrm_hit += 1; }
                let dc = ((o.clip[0] - gc[0]).abs() * 0.5 * target.0 as f32).max((o.clip[1] - gc[1]).abs() * 0.5 * target.1 as f32);
                if dc < 1e-3 { clip_hit += 1; }
                worst_clip = worst_clip.max(dc);
            }
        }
        s.push_str(&format!("\n  game instance {gi} (box {:?}..{:?}) ↔ ours {ii} (box Δ {d:.4} m): of {per} vertices {pos_hit} at our positions, {nrm_hit} with our normal, {clip_hit} at our clip position (worst clip Δ {worst_clip:.3} px)", gb.0, gb.1));
    }
    s
}

/// One chart's row profile: our pre-normalisation bytes, our normalised bytes, the editor's bytes and the ratio editor/ours
/// (normalised) along the chart's middle row — the tail's nonlinearity in one line.
pub fn chart_profile(pre: &[u8], ours: &[u8], editor: &[u8], chart: (u32, u32, u32, u32), fb_ours: u8, fb_editor: u8) -> String {
    let w = 1024u32;
    let (x0, y0, pw, ph) = crate::filecheck::chart_px(chart.0, chart.1, chart.2, chart.3);
    let y = y0 + ph / 2;
    let mut s = format!("chart ({}, {}, {}, {}) → px ({x0}, {y0}, {pw}, {ph}) row {y}: fb ours {fb_ours} editor {fb_editor}\n  pre   :", chart.0, chart.1, chart.2, chart.3);
    for x in x0..x0 + pw { s.push_str(&format!(" {:3}", pre[((y * w + x) * 3) as usize])); }
    s.push_str("\n  ours  :");
    for x in x0..x0 + pw { s.push_str(&format!(" {:3}", ours[((y * w + x) * 3) as usize])); }
    s.push_str("\n  editor:");
    for x in x0..x0 + pw { s.push_str(&format!(" {:3}", editor[((y * w + x) * 3) as usize])); }
    s.push_str("\n  ed/our:");
    for x in x0..x0 + pw { let (a, b) = (ours[((y * w + x) * 3) as usize] as f32, editor[((y * w + x) * 3) as usize] as f32); s.push_str(&if a > 0.0 { format!(" {:.2}", b / a) } else { "   -".into() }); }
    s
}

/// The empirical byte mapping editor = F(ours) over charts whose editor fb1 ≥ 250 (their normalisation is ≈ identity): per
/// 8-wide bin of our pre-normalisation byte the median editor byte and the count — the tail's curve for RE 7.
pub fn byte_curve(pre: &[u8], editor: &[u8], charts: &[(u32, u32, u32, u32)], fb_editor: &[u8]) -> String {
    let w = 1024u32;
    let mut bins: Vec<Vec<u8>> = vec![Vec::new(); 32];
    for (i, &(x, y, cw, ch)) in charts.iter().enumerate() {
        if fb_editor.get(i).map(|&f| f < 250).unwrap_or(true) { continue; }
        let (x0, y0, pw, ph) = crate::filecheck::chart_px(x, y, cw, ch);
        if pw < 6 || ph < 6 { continue; }
        for py in y0 + 1..y0 + ph - 1 {
            for px in x0 + 1..x0 + pw - 1 {
                let o = ((py * w + px) * 3) as usize;
                let (a, b) = (pre[o], editor[o]);
                if a == 0 && b == 0 { continue; }
                bins[(a / 8) as usize].push(b);
            }
        }
    }
    let mut s = String::from("byte curve (charts with editor fb1 ≥ 250, interiors): our byte bin → median editor byte (count)");
    for (k, v) in bins.iter_mut().enumerate() {
        if v.len() < 20 { continue; }
        v.sort_unstable();
        s.push_str(&format!("\n  {:3}–{:3} → {:3} ({})", k * 8, k * 8 + 7, v[v.len() / 2], v.len()));
    }
    s
}

// ---------------------------------------------------------------------------------------------------------------
// The frame-1 file images
// ---------------------------------------------------------------------------------------------------------------

/// The frame-1 slot of the written file: the WebP (1024², libwebp q 91), the per-chart frame bytes and the record's MaxHDR.
#[derive(Clone, Debug)]
pub struct Frame1Image {
    pub webp: Vec<u8>,
    pub fb1: Vec<u8>,
    pub max_hdr: f32,
    pub lit_texels: usize,
}

/// THE FRAME-1 IMAGES from the lists: the compose (`rule`), `dilate` passes of PS 1332 over the 2048² atlas part, MaxHDR = the
/// image max (f16), the encode byte = 255·√(v / MaxHDR) (the frame-0 encoder's colour curve; the exact frame-1 tail is RE 7's
/// pin), the 2 × 2 average to 1024², the per-chart normalisation (filecheck::chart_normalise → fb1), libwebp at q 91. None
/// when nothing was lit (the writer then keeps the black frame with MaxHDR 1e-5). `charts` = the mapping's rects in order.
pub fn frame1_images(lists: &Lists, lamps: &[Lamp], charts: &[(u32, u32, u32, u32)], rule: ComposeRule, dilate: u32, atlas: u32) -> Option<Frame1Image> {
    let img0 = compose(lists, lamps, rule);
    let img = frame1_dilated_n(&img0, atlas, dilate);
    let m = image_max(&img);
    if m <= 0.0 {
        return None;
    }
    let (ow, oh) = (atlas / 2, atlas / 2);
    let mut rgb = vec![0u8; (ow * oh * 3) as usize];
    let mut lit = 0usize;
    for y in 0..oh {
        for x in 0..ow {
            let mut any = false;
            for c in 0..3u32 {
                let mut s = 0.0f32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let v = img.get(2 * x + dx, 2 * y + dy, c) / m;
                        s += v.max(0.0).sqrt().min(1.0);
                    }
                }
                let b = (s * 0.25 * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
                if b > 0 { any = true; }
                rgb[((y * ow + x) * 3 + c) as usize] = b;
            }
            if any { lit += 1; }
        }
    }
    let fb1 = crate::filecheck::chart_normalise(&mut rgb, ow, oh, charts);
    let webp = crate::webpenc::encode_rgb(&rgb, ow, oh, 91.0)?;
    Some(Frame1Image { webp, fb1, max_hdr: m, lit_texels: lit })
}

/// The frame-1 slot of a lightmap chunk replaced: frame 1 image 0 = the WebP, the mapping's frame bytes 1 = fb1, the frame-1
/// record's MaxHDR (record 1 at head 60 + 66, the word at +20) = `max_hdr`. The chunk is re-serialised by the caller.
pub fn graft_frame1(d: &mut crate::format::LightmapData, f1: &Frame1Image) -> Result<(), String> {
    let fr = d.frames.get_mut(1).ok_or("the chunk has no frame 1")?;
    if fr.images.is_empty() {
        fr.images.push(Vec::new());
    }
    fr.images[0] = f1.webp.clone();
    let mp = d.cache.chunks.iter_mut().find_map(|c| match &mut c.body { crate::format::ChunkBody::Mapping(m) => Some(m), _ => None }).ok_or("no mapping chunk")?;
    if mp.frame_bytes.len() < 2 {
        return Err(format!("the mapping has {} frame byte tables", mp.frame_bytes.len()));
    }
    if f1.fb1.len() != mp.count as usize {
        return Err(format!("fb1 has {} bytes for {} charts", f1.fb1.len(), mp.count));
    }
    mp.frame_bytes[1] = f1.fb1.clone();
    mp.mark_edited();
    let r = 60 + 66;
    if mp.head.len() >= r + 24 {
        mp.head[r + 20..r + 24].copy_from_slice(&f1.max_hdr.to_le_bytes());
    }
    Ok(())
}

/// A vertical profile through a chart's middle column from 3 pixels above its rect to 3 below: ours (pre-normalisation), the
/// editor — where the editor's lit texels extend beyond the raster footprint.
pub fn chart_vprofile(pre: &[u8], editor: &[u8], chart: (u32, u32, u32, u32)) -> String {
    let w = 1024u32;
    let (x0, y0, pw, ph) = crate::filecheck::chart_px(chart.0, chart.1, chart.2, chart.3);
    let x = x0 + pw / 2;
    let ys = y0.saturating_sub(3)..(y0 + ph + 3).min(1024);
    let mut s = format!("chart ({}, {}, {}, {}) px rect ({x0}, {y0}, {pw}, {ph}), column {x}, rows {}..{}:\n  y     :", chart.0, chart.1, chart.2, chart.3, ys.start, ys.end);
    for y in ys.clone() { s.push_str(&format!(" {:3}", y % 1000)); }
    s.push_str("\n  ours  :");
    for y in ys.clone() { s.push_str(&format!(" {:3}", pre[((y * w + x) * 3) as usize])); }
    s.push_str("\n  editor:");
    for y in ys { s.push_str(&format!(" {:3}", editor[((y * w + x) * 3) as usize])); }
    s
}

/// PS 1332's rule on the 1024² byte image (a study switch: LMTOOL_LL_FILL1024=N passes): a texel with no lit channel takes the
/// mean of its lit 8-neighbours (each weighted 1).
pub fn fill_1024(rgb: &mut [u8], w: u32, h: u32, passes: u32) {
    for _ in 0..passes {
        let src = rgb.to_vec();
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                let o = ((y * w as i64 + x) * 3) as usize;
                if src[o] > 0 || src[o + 1] > 0 || src[o + 2] > 0 { continue; }
                let (mut acc, mut n) = ([0u32; 3], 0u32);
                for dy in -1..=1i64 { for dx in -1..=1i64 {
                    if dx == 0 && dy == 0 { continue; }
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 { continue; }
                    let p = ((ny * w as i64 + nx) * 3) as usize;
                    if src[p] > 0 || src[p + 1] > 0 || src[p + 2] > 0 { for c in 0..3 { acc[c] += src[p + c] as u32; } n += 1; }
                } }
                if n > 0 { for c in 0..3 { rgb[o + c] = ((acc[c] as f32 / n as f32) + 0.5) as u8; } }
            }
        }
    }
}

/// WHY IS THIS TEXEL SHADOWED — for `lamp`, the fragments of jitter 4 (the centred one) covering the target texel (x, y): the
/// world position and normal, the flat-cube face / uv / reference depth, the four PCF taps' stored depths (D16 steps) and the
/// caster triangle that wrote each (re-rendered with an owner map: instance / record class / triangle), and the light terms.
pub fn probe_shadow(gl: &crate::layout::GameLayout, sc: &LmScene, lamp: &Lamp, x: u32, y: u32, target: (u32, u32)) -> String {
    let drawn = cull(gl, sc, lamp);
    let tris = casters(sc, &drawn);
    // the owner of every caster triangle: (instance, triangle index within the instance)
    let mut owners: Vec<(usize, usize)> = Vec::new();
    for &ii in &drawn {
        if let Some(m) = mesh_of(sc, ii) { for t in 0..sc.meshes[m].indices.len() / 3 { owners.push((ii, t)); } }
    }
    let shadow = render_flat_cube(lamp.light.pos, lamp.r_eff, lamp.face_size, &tris, true);
    // the owner map: render again per triangle, noting which triangle wrote the final depth
    let mut owner = vec![usize::MAX; shadow.depth.len()];
    {
        let size = lamp.face_size;
        let w = (size * 3) as usize;
        for (ti, t) in tris.iter().enumerate() {
            let one = render_flat_cube(lamp.light.pos, lamp.r_eff, size, &[*t], true);
            for i in 0..one.depth.len() {
                if one.depth[i] > 0.0 && (one.depth[i] - shadow.depth[i]).abs() < 1e-9 { owner[i] = ti; }
            }
            let _ = w;
        }
    }
    let cb = lamp.light_cb();
    let mut s = format!("lamp id {} ({}) at {:?} dir {:?}, R {}: texel ({x}, {y})", lamp.id, lamp.owner, lamp.light.pos, lamp.light.dir, lamp.r_eff);
    let mut found = 0;
    for jit in 0..9u32 {
    let rcb = jitter_cb(jit, target.0, target.1);
    let verbose = jit == 4;
    for &ii in &drawn {
        let Some(m) = mesh_of(sc, ii) else { continue };
        let mesh = &sc.meshes[m];
        let inst = &sc.instances[ii];
        let vs: Vec<Vs7303Out> = mesh.verts.iter().map(|v| vs_7303(v, inst, &sc.table, &rcb)).collect();
        for (t, tri) in mesh.indices.chunks_exact(3).enumerate() {
            let (a, b, c) = (&vs[tri[0] as usize], &vs[tri[1] as usize], &vs[tri[2] as usize]);
            rasterise_triangle_rows([a.clip, b.clip, c.clip], target.0, target.1, y as i64, y as i64 + 1, |px, py, b0, b1, b2| {
                if px != x || py != y { return; }
                found += 1;
                let p = [a.world[0] * b0 + b.world[0] * b1 + c.world[0] * b2, a.world[1] * b0 + b.world[1] * b1 + c.world[1] * b2, a.world[2] * b0 + b.world[2] * b1 + c.world[2] * b2];
                let n = [a.normal[0] * b0 + b.normal[0] * b1 + c.normal[0] * b2, a.normal[1] * b0 + b.normal[1] * b1 + c.normal[1] * b2, a.normal[2] * b0 + b.normal[2] * b1 + c.normal[2] * b2];
                let l = [cb.light_pos[0] - p[0], cb.light_pos[1] - p[1], cb.light_pos[2] - p[2]];
                let d = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
                let (face, uv, dref) = crate::locallight::flat_cube_lookup(&cb, l[0], l[1], l[2]);
                let k = sc.rec_of.get(ii).copied().unwrap_or(usize::MAX);
                let class = gl.records.get(k).map(|r| r.class).unwrap_or("?");
                s.push_str(&format!("\n  jitter {jit}: fragment of instance {ii} ({class} record {k}, mesh {} idx, tri {t}): world {:?} normal {:?}, |L| {d:.3} m, N·L {:.4}; face {face}, uv {:?}, ref {dref:.7} ({:.1} steps); linear sample = {:.3}, point = {}", mesh.indices.len(), p, n, (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]) / d, uv, dref * 65535.0, shadow.sample_cmp_ge_linear(uv, dref, SHADOW_TARGET, 0), shadow.sample_cmp_ge(uv, dref, SHADOW_TARGET)));
                if !verbose { return; }
                let fx = uv[0] * SHADOW_TARGET as f32 - 0.5;
                let fy = uv[1] * SHADOW_TARGET as f32 - 0.5;
                let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
                for (dx, dy) in [(0i64, 0i64), (1, 0), (0, 1), (1, 1)] {
                    let (tx, ty) = (x0 + dx, y0 + dy);
                    let wsh = shadow.width() as i64;
                    if tx < 0 || ty < 0 || tx >= wsh || ty >= (shadow.size * 2) as i64 { continue; }
                    let i = (ty * wsh + tx) as usize;
                    let st = shadow.depth[i];
                    let own = owner[i];
                    let who = if own == usize::MAX { "nothing".to_string() } else { let (oi, ot) = owners[own]; let om = mesh_of(sc, oi).map(|m| sc.meshes[m].indices.len()).unwrap_or(0); let ok = sc.rec_of.get(oi).copied().unwrap_or(usize::MAX); format!("instance {oi} ({} record {ok}, mesh {om} idx, tri {ot}: {:?})", gl.records.get(ok).map(|r| r.class).unwrap_or("?"), tris[own]) };
                    s.push_str(&format!("\n    tap ({tx}, {ty}) [weights ({:.2}, {:.2})]: stored {:.7} ({:.1} steps) → {} — written by {who}", fx - x0 as f32, fy - y0 as f32, st, st * 65535.0, if dref >= st { "LIT" } else { "SHADOWED" }));
                }
            });
        }
    }
    }
    if found == 0 { s.push_str("\n  no fragment of the drawn set covers the texel"); }
    s
}

/// A study filter over the composed 2048² image (LMTOOL_LL_BLUR): `box3` = the 3×3 mean over the LIT neighbours (weights 1,
/// normalised by the lit count), `tent3` = the 3×3 tent (1 2 1 ⊗ 1 2 1) over lit neighbours, `box3all` = the plain 3×3 mean
/// (zeros included). The editor's frame 1 shows a blur signature (lower maxima, softer shadow edges than the lists imply).
pub fn blur_study(img: &crate::passdiff::Buf, kind: &str) -> crate::passdiff::Buf {
    let (w, h) = (img.w as i64, img.h as i64);
    let mut out = crate::passdiff::Buf::new(img.w, img.h, 4);
    let tent = kind.starts_with("tent");
    let all = kind.ends_with("all");
    for y in 0..h {
        for x in 0..w {
            let here = [img.get(x as u32, y as u32, 0), img.get(x as u32, y as u32, 1), img.get(x as u32, y as u32, 2), img.get(x as u32, y as u32, 3)];
            let lit_here = here[0] > 0.0 || here[1] > 0.0 || here[2] > 0.0;
            if !lit_here && !all {
                for c in 0..4 { out.set(x as u32, y as u32, c, here[c as usize]); }
                continue;
            }
            let (mut acc, mut wsum) = ([0f32; 3], 0f32);
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h { continue; }
                    let v = [img.get(nx as u32, ny as u32, 0), img.get(nx as u32, ny as u32, 1), img.get(nx as u32, ny as u32, 2)];
                    let lit = v[0] > 0.0 || v[1] > 0.0 || v[2] > 0.0;
                    let wt = if tent { ((2 - dx.abs()) * (2 - dy.abs())) as f32 } else { 1.0 };
                    if lit || all { for c in 0..3 { acc[c] += v[c] * wt; } wsum += wt; }
                }
            }
            if wsum > 0.0 { for c in 0..3 { out.set(x as u32, y as u32, c as u32, acc[c] / wsum); } }
            out.set(x as u32, y as u32, 3, here[3]);
        }
    }
    out
}

/// THE FRAME-1 IMAGES THROUGH FRAME 0's FINALISATION CHAIN (RE 7, NOTES 03:50Z: RenderLighting_Frames keeps the frame's regular
/// final-output slot for frame 1, so after the compose it runs the same chain as frame 0): the composed 2048² image (× `scale`,
/// the sweep finalisation's ×2 under test) as coefficient image 0 with zero H-basis images → e2e::finalise_tail (PS 1034, PS 1332
/// × 8, the max-reduce, CS 23025's YCbCr 4:2:0 encode against Mood_MaxHdr = MaxHdrMood·√(2π)) → filecheck::frame0_blobs (the CPU
/// Down2x2, chart_normalise → fb1, libwebp q 91) and record_scales (MaxHDR = κ·max). `charts` = the mapping's rects in order.
pub fn frame1_images_chain(lists: &Lists, lamps: &[Lamp], charts: &[(u32, u32, u32, u32)], rule: ComposeRule, mood_max_hdr_word: f32, scale: f32, atlas: u32) -> Option<Frame1Image> {
    let img0 = compose(lists, lamps, rule);
    let mut c0 = crate::passdiff::Buf::new(atlas, atlas, 4);
    let mut lit = 0usize;
    for y in 0..atlas {
        for x in 0..atlas {
            let a0 = img0.get(x, y, 3);
            if a0 > 0.0 { lit += 1; }
            // LMTOOL_LL_CHAIN_ALPHA1=1: the compose target's alpha 1 everywhere (PS 1332 then fills nothing — the editor's frame 1 has
            // no 4-texel halo); default: alpha = the texel's coverage
            let a = if std::env::var_os("LMTOOL_LL_CHAIN_ALPHA1").is_some() { 1.0 } else { a0 };
            for c in 0..3u32 {
                // the accumulation target is f16: the compose's values quantised like a resolve
                c0.set(x, y, c, crate::gpufmt::quantise_f16(img0.get(x, y, c) * scale, crate::gpufmt::Rounding::NearestEven));
            }
            c0.set(x, y, 3, a);
        }
    }
    let zero = crate::passdiff::Buf::new(atlas, atlas, 4);
    let finals = [c0, zero.clone(), zero.clone(), zero];
    let mood = mood_max_hdr_word * 2.506_628_3;
    let (_imgs, maxhdr, enc) = crate::e2e::finalise_tail(&finals, mood);
    let (blob0, _blob1, _sizes, fb) = crate::filecheck::frame0_blobs(&enc.y4, &enc.cb4, &enc.cr4, enc.w as usize, enc.h as usize, charts)?;
    let (max_hdr, _h234) = crate::filecheck::record_scales(maxhdr, mood);
    Some(Frame1Image { webp: blob0, fb1: fb, max_hdr, lit_texels: lit })
}

/// The frame-1 tail selector: `simple` (the default: MaxHDR = the image max, byte = 255·√(v/MaxHDR), 2×2 mean, chart_normalise,
/// libwebp — no dilation) or `chain[:S]` (frame 0's finalisation chain on the compose × S with alpha 1 everywhere — RE 7's
/// reading of RenderLighting_Frames; the compose dispatch itself is still being pinned from capture 2's tail).
pub fn frame1_images_by(tail: &str, lists: &Lists, lamps: &[Lamp], charts: &[(u32, u32, u32, u32)], mood_max_hdr_word: f32, dilate: u32, atlas: u32) -> Option<Frame1Image> {
    if let Some(rest) = tail.strip_prefix("chain") {
        let scale: f32 = rest.strip_prefix(':').and_then(|s| s.parse().ok()).unwrap_or(1.0);
        std::env::set_var("LMTOOL_LL_CHAIN_ALPHA1", "1");
        return frame1_images_chain(lists, lamps, charts, ComposeRule::SumDecoded, mood_max_hdr_word, scale, atlas);
    }
    frame1_images(lists, lamps, charts, ComposeRule::SumDecoded, dilate, atlas)
}
