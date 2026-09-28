//! The lightmapper's per-direction ACCUMULATE passes, TRANSCRIBED from the capture (passcap/pwc-day; ids of
//! capture pwc2 frame 127448, pwc1 frame 40648 in brackets — the DXBC is byte-identical across the captures):
//!
//! * `LmILightDir_Set` — VS 17111 [17525] (Vertex_17111.txt) + PS 17112 [17526] (Pixel_17112.txt). After every
//!   peel layer the LM raster of every object (the 3 items + the 4096 zone tiles, 4 draws) writes into
//!   `TMapILightDir` (2048² R11G11B10, cleared to 0 before the direction's first block): a texel facing the peel
//!   direction (`n·PeelDirInW ≥ 0`, else discard) projects its world position through `WorldPw01Shadow`, point-
//!   samples the layer's depth with the comparison sampler s1 (ClampEdge, GreaterEqual: passes when the texel's
//!   z01 ≥ the stored layer depth — the layer is beyond the texel on the sky side) and, when it passes, writes the
//!   layer's colour point-sampled at the same uv (s0 ClampEdge). Last write wins: the block runs after every layer
//!   of both peels (world, then fitted), far-to-near, so a texel ends with the layer nearest to it on the sky side.
//! * the H-BASIS accumulate — VS 17118 [17532] (Vertex_17118.txt) + PS 17122 [17536] (Pixel_17122.txt), once per
//!   direction after both peels: the same LM raster reads `TMapILightDir` at its own pixel (`ld`) and blends
//!   (One/One) into four 2048² RGBA16F MRTs: `C0 += (4π/N)·L·(0.093506(3sz²−1) + 0.398928 sz + 0.199472)`,
//!   `C1 += (4π/N)·L·(−0.230330 sy − 0.161951 sy sz)`, `C2 += (4π/N)·L·(−0.230330 sz − 0.107966(3sz²−1))`,
//!   `C3 += (4π/N)·L·(−0.230330 sx − 0.161951 sx sz)`, alpha += InvDirCount (1/N). (sx, sy, sz) = the direction
//!   in the texel's tangent frame, built as the bytecode does from the vertex stream's PSIZE mode and TANGENT.
//! * `AddAmbient` — CS 17125 [17539] (Compute_17539.txt), one thread per direction, before the direction's first
//!   accumulate: `BufAmbientAccum.xyz += Scale · TexColorPeeled[W/2, H/2]` (the centre pixel of the peel colour
//!   target as the environment render left it), `.w += Scale · 0.5`; `Scale` = the cbuffer's
//!   `g_LmILightDir_AddAmbient.Scale` (= D.y/32 = 8·D.y/N for N = 256 on every logged dispatch).
//!
//! Nothing here is modelled: the vertex formats are the draws' input layouts (env/frame127448/mesh.json), the
//! constants the cbuffers of logs/draws-frame*.json.gz, the sampler modes logs/samplers-frame127448.json, the
//! rasteriser the D3D11 rules (sunpass::rasterise_triangle), the target conversions the hardware facts measured on
//! this GPU (R11G11B10 store truncates; a blended RGBA16F target truncates the source to f16 and rounds the sum to
//! nearest even — sunpass::BlendModel::TruncSrcRoundSum). `lmtool ilightdir-check`, `hbasis-check` and
//! `ambient-check` run these kernels on the CAPTURED inputs and compare with the CAPTURED outputs to the quantum.

use crate::gpufmt::{quantise_f16, Rounding};
use crate::passdiff::Buf;
use crate::sunpass::{rasterise_triangle, rotation_rows, LmInstance, LmMesh, LmVertex};

/// The LM raster constants of one draw (cb ShaderV.g_CBufferV): `LM01_Scale_RasterSS` (2, −2) and
/// `LM01_Trans_RasterSS` (−1 + ox·2/(9·2048), 1 − oy·2/(9·2048)) for the direction's raster offset (ox, oy).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LmRasterCb {
    pub scale_ss: [f32; 2],
    pub trans_ss: [f32; 2],
}

/// The nine raster offsets of `LM01_Trans_RasterSS`, in the sweep's cycle (direction k uses offset k mod 9;
/// read off the capture's per-direction cbuffers — the port's `BakeParams::jitter_cycle`).
pub const RASTER_OFFSETS: [[f32; 2]; 9] = [[-4.0, 2.0], [-1.0, 3.0], [2.0, 4.0], [-3.0, -1.0], [0.0, 0.0], [3.0, 1.0], [-2.0, -4.0], [1.0, -3.0], [4.0, -2.0]];

impl LmRasterCb {
    /// The constants of raster offset `k mod 9` for a `w`×`h` LM target (`w` = 2048 in the capture): what the
    /// cbuffer holds for directions 0 and 2 of the capture to the last f32 bit (unit-tested).
    pub fn for_offset(k: usize, w: u32, h: u32) -> LmRasterCb {
        let o = RASTER_OFFSETS[k % 9];
        LmRasterCb { scale_ss: [2.0, -2.0], trans_ss: [-1.0 + o[0] / 9.0 * (2.0 / w as f32), 1.0 - o[1] / 9.0 * (2.0 / h as f32)] }
    }
}

/// The chart ST of a vertex (VS instructions 0–8, shared by every LM vertex shader): `idx = (b1 << 8) | b0`;
/// `idx < 0xffff` → `g_TcLM_ST_LM01[idx + asint(inst.st.x)]`, else the instance's own ST (the tiles' path).
#[inline]
pub fn chart_st(v: &LmVertex, inst: &LmInstance, table: &[[f32; 4]]) -> [f32; 4] {
    if v.chart_idx < 0xffff {
        let i = v.chart_idx.wrapping_add(inst.st_x_bits) as usize;
        table.get(i).copied().unwrap_or([0.0; 4])
    } else {
        inst.st
    }
}

/// The LM raster position of a vertex (VS 17111 9–10, 34 / VS 17118 9–10, 29): `clip.xy = (ST.xy·Scale)·uv +
/// (Scale·ST.zw + Trans)`, z = 0.5, w = 1.
#[inline]
/// LM_CLIP_UNFUSED=1, read ONCE: `std::env::var_os` takes the process-wide environment lock — per vertex, from 166 threads,
/// it was the whole cost of the vertex stage (stpad's sun pass: 3.8 s per 48 M vertices, 71 s in all; the same call sat in
/// sunpass::vs_15183).
pub fn clip_unfused() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("LM_CLIP_UNFUSED").is_some())
}

pub fn lm_clip(v: &LmVertex, st: [f32; 4], cb: &LmRasterCb) -> [f32; 2] {
    // the DXBC's two `mad`s are FUSED on this GPU (as the pixel shaders' are): r5.zw = mad(Scale, ST.zw, Trans),
    // o0.xy = mad(r5.xy, uv, r5.zw) — the unfused form (LM_CLIP_UNFUSED=1) leaves 1-ulp clip positions that snap one 1/256
    // step off at the half-way ties
    let sxy = [st[0] * cb.scale_ss[0], st[1] * cb.scale_ss[1]];
    if clip_unfused() {
        let tzw = [cb.scale_ss[0] * st[2] + cb.trans_ss[0], cb.scale_ss[1] * st[3] + cb.trans_ss[1]];
        return [sxy[0] * v.uv[0] + tzw[0], sxy[1] * v.uv[1] + tzw[1]];
    }
    let tzw = [cb.scale_ss[0].mul_add(st[2], cb.trans_ss[0]), cb.scale_ss[1].mul_add(st[3], cb.trans_ss[1])];
    [sxy[0].mul_add(v.uv[0], tzw[0]), sxy[1].mul_add(v.uv[1], tzw[1])]
}

// (perf 8's lm_clip_unfused and E's clip_unfused were the same OnceLock fix; `clip_unfused` above is the one kept)

/// `rows · p·scale + t` (the world position of a vertex, VS 17111 20–33 / VS 17118 30–37).
#[inline]
pub fn world_pos(v: &LmVertex, inst: &LmInstance, rows: &[[f32; 3]; 3]) -> [f32; 3] {
    let p = [v.pos[0] * inst.scale, v.pos[1] * inst.scale, v.pos[2] * inst.scale];
    [
        p[0] * rows[0][0] + p[1] * rows[0][1] + p[2] * rows[0][2] + inst.t[0],
        p[0] * rows[1][0] + p[1] * rows[1][1] + p[2] * rows[1][2] + inst.t[1],
        p[0] * rows[2][0] + p[1] * rows[2][1] + p[2] * rows[2][2] + inst.t[2],
    ]
}

/// `rows · n` (the world normal, VS 17111 35–37 / VS 17118 22–28; the same for any object-space vector).
#[inline]
pub fn rotate(n: [f32; 3], rows: &[[f32; 3]; 3]) -> [f32; 3] {
    [
        n[0] * rows[0][0] + n[1] * rows[0][1] + n[2] * rows[0][2],
        n[0] * rows[1][0] + n[1] * rows[1][1] + n[2] * rows[1][2],
        n[0] * rows[2][0] + n[1] * rows[2][1] + n[2] * rows[2][2],
    ]
}

#[inline]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `cross(a, b)` in the bytecode's form (`mul r, a.yzx, b.zxy; mad r, a.zxy, b.yzx, -r` → the component order of
/// the DXBC: r.x = a.y·b.z − a.z·b.y, r.y = a.z·b.x − a.x·b.z, r.z = a.x·b.y − a.y·b.x).
#[inline]
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

// ---------------------------------------------------------------------------------------------------------------
// LmILightDir_Set
// ---------------------------------------------------------------------------------------------------------------

/// The PS 17112 constants (cb ShaderP.g_CBufferP): the peel's `WorldPw01Shadow` (HLSL rows as the log prints
/// them; register k = column k) and `PeelDirInW`.
#[derive(Clone, Copy, Debug)]
pub struct SetCb {
    pub world_pw01_shadow: [[f32; 4]; 4],
    pub peel_dir: [f32; 3],
}

/// A vertex after VS 17111: LM clip xy, world position (o1), world normal (o2).
#[derive(Clone, Copy, Debug)]
pub struct SetVsOut {
    pub clip: [f32; 2],
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
}

pub fn vs_17111(v: &LmVertex, inst: &LmInstance, table: &[[f32; 4]], cb: &LmRasterCb) -> SetVsOut {
    let rows = rotation_rows(inst.q);
    let st = chart_st(v, inst, table);
    SetVsOut { clip: lm_clip(v, st, cb), pos: world_pos(v, inst, &rows), nrm: rotate(v.normal, &rows) }
}

/// How the comparison sampler sees the D16 layer depth against the texel's f32 reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DepthCompare {
    /// the reference converted to the target's UNORM16 fixed point (round to nearest) and compared as integers
    Unorm16Round,
    /// the reference kept in f32 against the stored value q/65535
    Float,
}

/// The layer targets the block samples: the peel colour (R11G11B10, decoded) and depth (D16 as UNORM16 → f32).
pub struct LayerTargets<'a> {
    pub color: &'a Buf,
    pub depth: &'a Buf,
}

/// A peel layer's two targets as the pixel shaders read them (PS 17112, the probe passes): the depth and the colour at a
/// texel, each with its own size — the dense `LayerTargets` pair, or `LayerSparse` reading the sparse layer table in
/// place (perf 8: no per-layer 4096² images materialised for the transcribed accumulate).
pub trait LayerRead: Sync {
    fn depth_size(&self) -> (u32, u32);
    fn color_size(&self) -> (u32, u32);
    fn depth(&self, x: u32, y: u32) -> f32;
    fn rgb(&self, x: u32, y: u32) -> [f32; 3];
}

impl LayerRead for LayerTargets<'_> {
    #[inline]
    fn depth_size(&self) -> (u32, u32) { (self.depth.w, self.depth.h) }
    #[inline]
    fn color_size(&self) -> (u32, u32) { (self.color.w, self.color.h) }
    #[inline]
    fn depth(&self, x: u32, y: u32) -> f32 { self.depth.get(x, y, 0) }
    #[inline]
    fn rgb(&self, x: u32, y: u32) -> [f32; 3] { [self.color.get(x, y, 0), self.color.get(x, y, 1), self.color.get(x, y, 2)] }
}

/// Layer `k` of a peel's layer table read in place: `start` = the DENSE per-pixel fragment offsets (`w · h + 1` entries,
/// `Layers::dense_start`), `frags` = the fragments (stored depths increase with the layer index). A pixel with fewer than
/// k + 1 layers reads the targets' CLEAR — depth 1.0 (the near plane), colour black — exactly what `Layers::depth_image` /
/// `colour_image` put there.
pub struct LayerSparse<'a> {
    pub w: u32,
    pub h: u32,
    /// The layer table's `start` — dense (per pixel, `start[p]..start[p + 1]`) when `px` is None; per WANTED pixel
    /// (rank k from `px`, the list `start[k]..start[k] + cnt[k]` — the in-place derive's gapped layout, or
    /// `start[k]..start[k + 1]` without `cnt`) otherwise. (perf 3: the dense offsets were a 67 MB array built per frame
    /// plus a compaction copy of the fragments; the rank lookup is two loads and a popcount.)
    pub start: &'a [u32],
    pub cnt: Option<&'a [u8]>,
    pub px: Option<&'a crate::peel::PixelIndex>,
    pub frags: &'a [crate::peel::LayerFrag],
    pub k: usize,
}

impl LayerSparse<'_> {
    /// Pixel (x, y)'s slots in `frags`: (first, count) — None for a pixel outside the wanted set (no layers). The same
    /// resolution under both layouts; `replay_set_layers` resolves it ONCE per fragment and walks the layers from it
    /// (perf 8.23) — layer k of the pixel is `frags[first + k]` when k < count, else the targets' clear.
    #[inline(always)]
    pub fn range(&self, x: u32, y: u32) -> Option<(usize, usize)> {
        match self.px {
            Some(px) => {
                let k = px.index(x, y)? as usize;
                let a = self.start[k] as usize;
                Some((a, match self.cnt { Some(c) => c[k] as usize, None => self.start[k + 1] as usize - a }))
            }
            None => {
                let p = (y * self.w + x) as usize;
                let a = self.start[p] as usize;
                Some((a, self.start[p + 1] as usize - a))
            }
        }
    }
    /// Pixel (x, y)'s layer `k`, if it has one.
    #[inline(always)]
    fn frag(&self, x: u32, y: u32) -> Option<&crate::peel::LayerFrag> {
        let (a, n) = self.range(x, y)?;
        if self.k < n { Some(&self.frags[a + self.k]) } else { None }
    }
}

impl LayerRead for LayerSparse<'_> {
    #[inline]
    fn depth_size(&self) -> (u32, u32) { (self.w, self.h) }
    #[inline]
    fn color_size(&self) -> (u32, u32) { (self.w, self.h) }
    #[inline]
    fn depth(&self, x: u32, y: u32) -> f32 {
        match self.frag(x, y) { Some(f) => f.d, None => 1.0 }
    }
    #[inline]
    fn rgb(&self, x: u32, y: u32) -> [f32; 3] {
        match self.frag(x, y) { Some(f) => f.rgb, None => [0.0; 3] }
    }
}

/// Point sampling with ClampEdge addressing (s0 / s1 of PS 17112: `SGbxClamp_Point`): the texel `floor(u·W)`,
/// clamped to the edge.
#[inline]
pub fn point_texel(u: f32, w: u32) -> u32 {
    let x = (u * w as f32).floor();
    if x <= 0.0 { 0 } else if x >= (w - 1) as f32 { w - 1 } else { x as u32 }
}

/// PS 17112 for one pixel: `Some(rgb)` written to the target, `None` discarded. `p` = v1 (world position), `n` =
/// v2 (world normal, interpolated, not normalised).
#[inline]
pub fn ps_17112(p: [f32; 3], n: [f32; 3], cb: &SetCb, layer: &LayerTargets, cmp: DepthCompare) -> Option<[f32; 3]> {
    // 0-2: dp3 n·PeelDirInW; discard if < 0 (0 passes)
    if dot(n, cb.peel_dir) < 0.0 {
        return None;
    }
    // 3-8: (p, 1) · the four registers = columns of the printed matrix (row-vector convention)
    let m = &cb.world_pw01_shadow;
    let z = p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2] + m[3][2];
    let u = p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0] + m[3][0];
    let v = p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1] + m[3][1];
    let w = p[0] * m[0][3] + p[1] * m[1][3] + p[2] * m[2][3] + m[3][3];
    // 9: the colour lookup's uv = (u, v)/w; 10: the compare samples at the UNDIVIDED (u, v) (w = 1 for the ortho)
    let (cu, cv) = (u / w, v / w);
    let (tx, ty) = (point_texel(u, layer.depth.w), point_texel(v, layer.depth.h));
    let stored = layer.depth.get(tx, ty, 0);
    let pass = match cmp {
        DepthCompare::Unorm16Round => {
            let rq = (z.clamp(0.0, 1.0) * 65535.0).round();
            let sq = (stored * 65535.0).round();
            rq >= sq
        }
        DepthCompare::Float => z >= stored,
    };
    // 11-13: discard when the compare result (1 or 0) − 0.5 < 0
    if !pass {
        return None;
    }
    // 14: the layer colour at the same point
    let (cx, cy) = (point_texel(cu, layer.color.w), point_texel(cv, layer.color.h));
    let mut c = [layer.color.get(cx, cy, 0), layer.color.get(cx, cy, 1), layer.color.get(cx, cy, 2)];
    // 15-18: max 0, min 1e38, then zero where ≥ 5e37 (Inf/NaN guard)
    for k in 0..3 {
        let v = c[k].max(0.0).min(99999996802856930000000000000000000000.0);
        c[k] = if v < 49999998401428460000000000000000000000.0 { v } else { 0.0 };
    }
    Some(c)
}

/// One accumulate draw: mesh + instance range + the two cbuffers. `world_box` = the FITTED peel's variant of the
/// vertex shader (VS 17115 [pwc1: 17529]): four clip distances `(x − MinX, z − MinZ, MaxX − x, MaxZ − z)` of the
/// world position against `WorldBoxMinXZ` / `WorldBoxMaxXZ` — the LM raster of the fitted blocks reaches only the
/// texels whose world (x, z) lies in the items' box; every other texel keeps the world peel's value. The world
/// blocks run VS 17111 (no clip distances; the cbuffer's box is 0).
#[derive(Clone, Debug)]
pub struct SetDraw {
    pub eid: u64,
    pub mesh: usize,
    pub instance_first: usize,
    pub instance_count: usize,
    pub raster: LmRasterCb,
    pub cb: SetCb,
    pub world_box: Option<[[f32; 2]; 2]>,
}

/// VS 17115's clip distances of a vertex (instructions 38–39): `(x − MinX, z − MinZ, MaxX − x, MaxZ − z)`.
#[inline]
pub fn clip_distances(pos: [f32; 3], world_box: &[[f32; 2]; 2]) -> [f32; 4] {
    [pos[0] - world_box[0][0], pos[2] - world_box[0][1], -pos[0] + world_box[1][0], -pos[2] + world_box[1][1]]
}

/// Write one element per page of a fresh zeroed buffer so its pages are mapped here, on one thread, rather than by the
/// first parallel pass over it (the kernel's fault path contended by every pool thread). The writes are zeros into
/// zeroed memory — volatile, so they are not elided.
pub fn touch_pages<T: Copy + Default>(v: &mut [T]) {
    let step = (4096 / std::mem::size_of::<T>().max(1)).max(1);
    let mut i = 0;
    while i < v.len() {
        // SAFETY: i < len
        unsafe { std::ptr::write_volatile(v.as_mut_ptr().add(i), T::default()); }
        i += step;
    }
}

/// The 2048² R11G11B10 `TMapILightDir` target as the GPU holds it (packed u32 per pixel).
pub struct DirTarget {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u32>,
}

impl DirTarget {
    pub fn cleared(w: u32, h: u32) -> DirTarget {
        let mut px = vec![0u32; (w * h) as usize];
        touch_pages(&mut px);
        DirTarget { w, h, px }
    }
    /// The target cleared for the next direction (the same allocation: no fresh pages to fault in).
    pub fn clear(&mut self) {
        self.px.fill(0);
    }
    #[inline]
    pub fn rgb(&self, x: u32, y: u32) -> [f32; 3] {
        crate::gpufmt::unpack_r11g11b10(self.px[(y * self.w + x) as usize])
    }
}

/// Run one block of accumulate draws (the 4 objects after one peel layer) over the layer's captured targets.
/// The R11G11B10 store truncates (the colour is already an R11G11B10 value, so the store is exact either way).
pub fn run_set_block(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[SetDraw], layer: &LayerTargets, cmp: DepthCompare, tgt: &mut DirTarget) {
    run_set_block_probe(meshes, instances, table, draws, layer, cmp, tgt, None);
}

/// `run_set_block` with an optional probe pixel: every fragment landing on it prints its whole path (the draw, the
/// interpolated position / normal, the projected uv / z, the sampled layer depth and colour, the verdict). With
/// `trace`, the last fragment of every pixel is recorded: (u, v, z, stored depth, n·D).
pub fn run_set_block_probe(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[SetDraw], layer: &LayerTargets, cmp: DepthCompare, tgt: &mut DirTarget, probe: Option<(u32, u32)>) {
    if probe.is_none() && std::env::var_os("LMTOOL_LMACCUM_SERIAL").is_none() {
        return run_set_block_par(meshes, instances, table, draws, layer, cmp, tgt);
    }
    run_set_block_trace(meshes, instances, table, draws, layer, cmp, tgt, probe, None);
}

/// The LM raster's band decomposition: every (draw, instance) transformed once, in parallel; every triangle
/// binned into the pixel-row bands it touches, in (draw, instance, triangle) order; each band rasterises its
/// triangles with the rows clipped to the band. Per PIXEL the fragments arrive in the serial loops' order (a
/// pixel lies in one band, whose list keeps the global order), so every per-pixel fold is unchanged.
pub(crate) struct BandPlan {
    /// (pair index, triangle index) per band.
    pub(crate) lists: Vec<Vec<(u32, u32)>>,
    pub(crate) rows: usize,
    pub(crate) n_bands: usize,
}

pub(crate) fn band_plan<P: Sync>(preps: &[P], clip_of: impl Fn(&P, usize) -> [[f32; 2]; 3] + Sync, tri_count: impl Fn(&P) -> usize + Sync, h: u32, threads: usize) -> BandPlan {
    let n_bands = (threads * 4).clamp(1, h as usize);
    let rows = (h as usize + n_bands - 1) / n_bands;
    // per pair its per-band lists, then the bands' lists as the concatenation over the pairs in order
    let per_pair: Vec<Vec<Vec<u32>>> = crate::pool::pool().map(preps.len(), |k| {
        let p = &preps[k];
        let mut out: Vec<Vec<u32>> = vec![Vec::new(); n_bands];
        for t in 0..tri_count(p) {
            let (lo, hi) = crate::sunpass::raster_rows(clip_of(p, t), h);
            if lo >= hi { continue; }
            let (b0, b1) = ((lo as usize) / rows, ((hi as usize - 1) / rows).min(n_bands - 1));
            for b in b0..=b1 { out[b].push(t as u32); }
        }
        out
    });
    let lists: Vec<Vec<(u32, u32)>> = crate::pool::pool().map(n_bands, |b| {
        let mut v = Vec::new();
        for (k, pp) in per_pair.iter().enumerate() { for &t in &pp[b] { v.push((k as u32, t)); } }
        v
    });
    BandPlan { lists, rows, n_bands }
}

struct SetPrep {
    di: usize,
    vs: Vec<SetVsOut>,
    cds: Vec<[f32; 4]>,
}

/// An instance's world AABB from its mesh's local bounds (the 8 corners through the instance transform).
pub fn instance_aabb(local: &([f32; 3], [f32; 3]), inst: &LmInstance) -> ([f32; 3], [f32; 3]) {
    let rows = crate::sunpass::rotation_rows(inst.q);
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for k in 0..8 {
        let c = [if k & 1 == 0 { local.0[0] } else { local.1[0] }, if k & 2 == 0 { local.0[1] } else { local.1[1] }, if k & 4 == 0 { local.0[2] } else { local.1[2] }];
        let v = LmVertex { pos: c, chart_idx: 0, normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], psize: 0.0, tangent: [0.0, 0.0, 0.0, 1.0] };
        let p = world_pos(&v, inst, &rows);
        for a in 0..3 { lo[a] = lo[a].min(p[a]); hi[a] = hi[a].max(p[a]); }
    }
    (lo, hi)
}

/// The local bounds of every mesh (min, max over its vertex positions).
pub fn mesh_bounds(meshes: &[LmMesh]) -> Vec<([f32; 3], [f32; 3])> {
    meshes.iter().map(|m| { let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]); for v in &m.verts { for a in 0..3 { lo[a] = lo[a].min(v.pos[a]); hi[a] = hi[a].max(v.pos[a]); } } (lo, hi) }).collect()
}

/// THE WORLD-BLOCK CULL: an instance whose world AABB lies wholly beyond one of the block's four clip planes (VS 17115's
/// `(x − MinX, z − MinZ, MaxX − x, MaxZ − z)`) has every vertex's clip distance on that plane negative, so every fragment
/// of every triangle is rejected — skipping it changes nothing (Stadium stpad: 9 216 Grass tiles × 9 889 vertices per
/// world block, 17 blocks). A 1 m margin keeps the test conservative.
pub fn culled_by_world_box(aabb: &([f32; 3], [f32; 3]), world_box: &[[f32; 2]; 2]) -> bool {
    let m = 1.0f32;
    aabb.1[0] < world_box[0][0] - m || aabb.0[0] > world_box[1][0] + m || aabb.1[2] < world_box[0][1] - m || aabb.0[2] > world_box[1][1] + m
}

pub fn run_set_block_par(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[SetDraw], layer: &LayerTargets, cmp: DepthCompare, tgt: &mut DirTarget) {
    let (w, h) = (tgt.w, tgt.h);
    let threads = crate::pool::pool().threads.max(1);
    let bounds = mesh_bounds(meshes);
    let cull = *BLOCK_CULL;
    let pairs: Vec<(usize, usize)> = draws.iter().enumerate().flat_map(|(di, d)| (d.instance_first..d.instance_first + d.instance_count).map(move |ii| (di, ii))).filter(|&(di, ii)| {
        if !cull { return true; }
        match &draws[di].world_box { Some(b) => !culled_by_world_box(&instance_aabb(&bounds[draws[di].mesh], &instances[ii]), b), None => true }
    }).collect();
    let preps: Vec<SetPrep> = crate::pool::pool().map(pairs.len(), |k| {
        let (di, ii) = pairs[k];
        let d = &draws[di];
        let mesh = &meshes[d.mesh];
        let inst = &instances[ii];
        let vs: Vec<SetVsOut> = mesh.verts.iter().map(|v| vs_17111(v, inst, table, &d.raster)).collect();
        let cds: Vec<[f32; 4]> = match &d.world_box { Some(b) => vs.iter().map(|o| clip_distances(o.pos, b)).collect(), None => Vec::new() };
        SetPrep { di, vs, cds }
    });
    let plan = band_plan(&preps, |p, t| { let idx = &meshes[draws[p.di].mesh].indices[t * 3..t * 3 + 3]; [p.vs[idx[0] as usize].clip, p.vs[idx[1] as usize].clip, p.vs[idx[2] as usize].clip] }, |p| meshes[draws[p.di].mesh].indices.len() / 3, h, threads);
    let px_ptr = tgt.px.as_mut_ptr() as usize;
    crate::pool::pool().run(plan.n_bands, |b| {
        let (y_lo, y_hi) = ((b * plan.rows) as i64, (((b + 1) * plan.rows).min(h as usize)) as i64);
        for &(k, t) in &plan.lists[b] {
            let p = &preps[k as usize];
            let d = &draws[p.di];
            let mesh = &meshes[d.mesh];
            let tri = &mesh.indices[t as usize * 3..t as usize * 3 + 3];
            let (a, bb, c) = (&p.vs[tri[0] as usize], &p.vs[tri[1] as usize], &p.vs[tri[2] as usize]);
            let cd: Option<[[f32; 4]; 3]> = if p.cds.is_empty() { None } else { Some([p.cds[tri[0] as usize], p.cds[tri[1] as usize], p.cds[tri[2] as usize]]) };
            crate::sunpass::rasterise_triangle_rows([a.clip, bb.clip, c.clip], w, h, y_lo, y_hi, |x, y, b0, b1, b2| {
                if let Some(cd) = &cd {
                    for i in 0..4 {
                        if cd[0][i] * b0 + cd[1][i] * b1 + cd[2][i] * b2 < 0.0 {
                            return;
                        }
                    }
                }
                let pos = [a.pos[0] * b0 + bb.pos[0] * b1 + c.pos[0] * b2, a.pos[1] * b0 + bb.pos[1] * b1 + c.pos[1] * b2, a.pos[2] * b0 + bb.pos[2] * b1 + c.pos[2] * b2];
                let n = [a.nrm[0] * b0 + bb.nrm[0] * b1 + c.nrm[0] * b2, a.nrm[1] * b0 + bb.nrm[1] * b1 + c.nrm[1] * b2, a.nrm[2] * b0 + bb.nrm[2] * b1 + c.nrm[2] * b2];
                if let Some(rgb) = ps_17112(pos, n, &d.cb, layer, cmp) {
                    // SAFETY: the bands own disjoint pixel rows
                    unsafe { *(px_ptr as *mut u32).add((y * w + x) as usize) = crate::gpufmt::pack_r11g11b10(rgb, Rounding::Truncate); }
                }
            });
        }
    });
}

/// THE WORLD BLOCKS FUSED (Stadium: 21 blocks × 45.8 M triangles per direction): the vertex shader and the band plan run ONCE
/// and every fragment is compared against the layer targets of all the blocks in `layers` (block k0 + j reads layers[j]).
/// The sequential semantics are kept exactly: the blocks' passes run in order and the last write wins, so a pixel ends as the
/// LAST fragment (in raster order) of the HIGHEST block that wrote it — here `best_k[px]` carries the highest block that has
/// written the pixel so far (across calls: the fitted peels' blocks follow the world peel's, hence `k0`); a fragment of block k
/// writes when k ≥ best_k[px]. The fragment order is the plan's, the same for every block, so the result is identical to
/// `run_set_block_par` called once per block. `best_k` is w × h, u16::MAX = never written.
pub fn run_set_layers_par(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[SetDraw], layers: &[LayerTargets], cmp: DepthCompare, tgt: &mut DirTarget, best_k: &mut Vec<u16>, k0: usize) {
    let (w, h) = (tgt.w, tgt.h);
    if best_k.len() != (w * h) as usize { *best_k = vec![u16::MAX; (w * h) as usize]; }
    let threads = crate::pool::pool().threads.max(1);
    let bounds = mesh_bounds(meshes);
    let cull = *BLOCK_CULL;
    let pairs: Vec<(usize, usize)> = draws.iter().enumerate().flat_map(|(di, d)| (d.instance_first..d.instance_first + d.instance_count).map(move |ii| (di, ii))).filter(|&(di, ii)| {
        if !cull { return true; }
        match &draws[di].world_box { Some(b) => !culled_by_world_box(&instance_aabb(&bounds[draws[di].mesh], &instances[ii]), b), None => true }
    }).collect();
    let preps: Vec<SetPrep> = crate::pool::pool().map(pairs.len(), |k| {
        let (di, ii) = pairs[k];
        let d = &draws[di];
        let mesh = &meshes[d.mesh];
        let inst = &instances[ii];
        let vs: Vec<SetVsOut> = mesh.verts.iter().map(|v| vs_17111(v, inst, table, &d.raster)).collect();
        let cds: Vec<[f32; 4]> = match &d.world_box { Some(b) => vs.iter().map(|o| clip_distances(o.pos, b)).collect(), None => Vec::new() };
        SetPrep { di, vs, cds }
    });
    let plan = band_plan(&preps, |p, t| { let idx = &meshes[draws[p.di].mesh].indices[t * 3..t * 3 + 3]; [p.vs[idx[0] as usize].clip, p.vs[idx[1] as usize].clip, p.vs[idx[2] as usize].clip] }, |p| meshes[draws[p.di].mesh].indices.len() / 3, h, threads);
    let px_ptr = tgt.px.as_mut_ptr() as usize;
    let bk_ptr = best_k.as_mut_ptr() as usize;
    crate::pool::pool().run(plan.n_bands, |b| {
        let (y_lo, y_hi) = ((b * plan.rows) as i64, (((b + 1) * plan.rows).min(h as usize)) as i64);
        for &(k, t) in &plan.lists[b] {
            let p = &preps[k as usize];
            let d = &draws[p.di];
            let mesh = &meshes[d.mesh];
            let tri = &mesh.indices[t as usize * 3..t as usize * 3 + 3];
            let (a, bb, c) = (&p.vs[tri[0] as usize], &p.vs[tri[1] as usize], &p.vs[tri[2] as usize]);
            let cd: Option<[[f32; 4]; 3]> = if p.cds.is_empty() { None } else { Some([p.cds[tri[0] as usize], p.cds[tri[1] as usize], p.cds[tri[2] as usize]]) };
            crate::sunpass::rasterise_triangle_rows([a.clip, bb.clip, c.clip], w, h, y_lo, y_hi, |x, y, b0, b1, b2| {
                if let Some(cd) = &cd {
                    for i in 0..4 {
                        if cd[0][i] * b0 + cd[1][i] * b1 + cd[2][i] * b2 < 0.0 {
                            return;
                        }
                    }
                }
                let pos = [a.pos[0] * b0 + bb.pos[0] * b1 + c.pos[0] * b2, a.pos[1] * b0 + bb.pos[1] * b1 + c.pos[1] * b2, a.pos[2] * b0 + bb.pos[2] * b1 + c.pos[2] * b2];
                let n = [a.nrm[0] * b0 + bb.nrm[0] * b1 + c.nrm[0] * b2, a.nrm[1] * b0 + bb.nrm[1] * b1 + c.nrm[1] * b2, a.nrm[2] * b0 + bb.nrm[2] * b1 + c.nrm[2] * b2];
                let pi = (y * w + x) as usize;
                // SAFETY: the bands own disjoint pixel rows
                let cur = unsafe { *(bk_ptr as *const u16).add(pi) };
                for (j, layer) in layers.iter().enumerate() {
                    let kk = (k0 + j) as u16;
                    if cur != u16::MAX && kk < cur { continue; }
                    if kk == 0 && d.world_box.is_some() && !*crate::peel::TILE_SKY { continue; } // LMTOOL_TILE_SKY=0 (study): no dome layer in a tile pass
                    if let Some(rgb) = ps_17112(pos, n, &d.cb, layer, cmp) {
                        unsafe {
                            *(px_ptr as *mut u32).add(pi) = crate::gpufmt::pack_r11g11b10(rgb, Rounding::Truncate);
                            *(bk_ptr as *mut u16).add(pi) = kk;
                        }
                    }
                }
            });
        }
    });
}

/// The per-pixel trace of a block: the last fragment's projected (u, v, z), the layer depth it compared with, n·D.
pub type SetTrace = Vec<Option<(f32, f32, f32, f32, f32)>>;

pub fn run_set_block_trace(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[SetDraw], layer: &LayerTargets, cmp: DepthCompare, tgt: &mut DirTarget, probe: Option<(u32, u32)>, mut trace: Option<&mut SetTrace>) {
    let (w, h) = (tgt.w, tgt.h);
    for (di, d) in draws.iter().enumerate() {
        let mesh = &meshes[d.mesh];
        for ii in d.instance_first..d.instance_first + d.instance_count {
            let inst = &instances[ii];
            let vs: Vec<SetVsOut> = mesh.verts.iter().map(|v| vs_17111(v, inst, table, &d.raster)).collect();
            // the fitted variant's clip distances per vertex (SV_ClipDistance: linear over the primitive, a pixel with
            // any distance < 0 is clipped — the clipper's cut evaluated at the pixel centre)
            let cds: Vec<[f32; 4]> = match &d.world_box { Some(b) => vs.iter().map(|o| clip_distances(o.pos, b)).collect(), None => Vec::new() };
            for (ti, tri) in mesh.indices.chunks_exact(3).enumerate() {
                let (a, b, c) = (&vs[tri[0] as usize], &vs[tri[1] as usize], &vs[tri[2] as usize]);
                let cd: Option<[[f32; 4]; 3]> = if cds.is_empty() { None } else { Some([cds[tri[0] as usize], cds[tri[1] as usize], cds[tri[2] as usize]]) };
                rasterise_triangle([a.clip, b.clip, c.clip], w, h, |x, y, b0, b1, b2| {
                    if let Some(cd) = &cd {
                        for i in 0..4 {
                            if cd[0][i] * b0 + cd[1][i] * b1 + cd[2][i] * b2 < 0.0 {
                                if probe == Some((x, y)) { eprintln!("probe ({x},{y}): draw {di} tri {ti} clipped by clip distance {i} ({:.4})", cd[0][i] * b0 + cd[1][i] * b1 + cd[2][i] * b2); }
                                return;
                            }
                        }
                    }
                    let p = [a.pos[0] * b0 + b.pos[0] * b1 + c.pos[0] * b2, a.pos[1] * b0 + b.pos[1] * b1 + c.pos[1] * b2, a.pos[2] * b0 + b.pos[2] * b1 + c.pos[2] * b2];
                    let n = [a.nrm[0] * b0 + b.nrm[0] * b1 + c.nrm[0] * b2, a.nrm[1] * b0 + b.nrm[1] * b1 + c.nrm[1] * b2, a.nrm[2] * b0 + b.nrm[2] * b1 + c.nrm[2] * b2];
                    let r = ps_17112(p, n, &d.cb, layer, cmp);
                    if probe == Some((x, y)) || trace.is_some() {
                        let m = &d.cb.world_pw01_shadow;
                        let u = p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0] + m[3][0];
                        let v = p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1] + m[3][1];
                        let z = p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2] + m[3][2];
                        let (tx, ty) = (point_texel(u, layer.depth.w), point_texel(v, layer.depth.h));
                        let stored = layer.depth.get(tx, ty, 0);
                        if let Some(t) = trace.as_deref_mut() { t[(y * w + x) as usize] = Some((u, v, z, stored, dot(n, d.cb.peel_dir))); }
                        if probe == Some((x, y)) {
                            eprintln!("probe ({x},{y}): draw {di} (eid {}, mesh {}) instance {ii} tri {ti} bary ({b0:.3},{b1:.3},{b2:.3}) p ({:.3},{:.3},{:.3}) n ({:.3},{:.3},{:.3}) n·D {:.4}; uv ({u:.5},{v:.5}) → texel ({tx},{ty}) z {z:.6} stored {stored:.6} (q {}) → {}; layer colour there ({:.4},{:.4},{:.4}); result {:?}", d.eid, d.mesh, p[0], p[1], p[2], n[0], n[1], n[2], dot(n, d.cb.peel_dir), (stored * 65535.0).round(), if z >= stored { "pass" } else { "FAIL" }, layer.color.get(tx, ty, 0), layer.color.get(tx, ty, 1), layer.color.get(tx, ty, 2), r);
                        }
                    }
                    if let Some(rgb) = r {
                        tgt.px[(y * w + x) as usize] = crate::gpufmt::pack_r11g11b10(rgb, Rounding::Truncate);
                    }
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The H-basis accumulate
// ---------------------------------------------------------------------------------------------------------------

/// The PS 17122 / VS 17118 constants: `PeelDirInW` (both stages) and `InvDirCount`.
#[derive(Clone, Copy, Debug)]
pub struct HbCb {
    pub peel_dir: [f32; 3],
    pub inv_dir_count: f32,
}

/// A vertex after VS 17118: LM clip xy, `o2` (the world normal in mode A, else the direction in the vertex's
/// tangent frame) and `o3` (the object-space tangent with w = 1 in mode A, else 0). `mode` = |PSIZE| class.
#[derive(Clone, Copy, Debug)]
pub struct HbVsOut {
    pub clip: [f32; 2],
    pub o2: [f32; 3],
    pub o3: [f32; 4],
}

/// VS 17118, the tangent-frame modes by the vertex's PSIZE (`v4.x`): |v4.x| > 2.5 → the pixel shader builds the
/// frame (o2 = the world normal, o3 = the raw tangent, w = 1); |v4.x| < 1.5 → the vertex's own frame with the
/// bitangent `cross(n, t)·sign(v4.x)`, both rotated to the world; else → the frame from `normalize(t × n_world)`
/// and `n_world × that`. In the last two the direction is projected in the vertex shader and interpolated.
pub fn vs_17118(v: &LmVertex, inst: &LmInstance, table: &[[f32; 4]], cb: &LmRasterCb, peel_dir: [f32; 3]) -> HbVsOut {
    let rows = rotation_rows(inst.q);
    let st = chart_st(v, inst, table);
    let clip = lm_clip(v, st, cb);
    let (o2, o3) = vs_17118_o23(v, &rows, peel_dir);
    HbVsOut { clip, o2, o3 }
}

/// VS 17118's `o2` / `o3` alone (the instructions after the clip position): the direction-dependent part the fragment-list
/// replay recomputes per direction from the vertex and the instance's rotation rows. The same arithmetic as `vs_17118`.
#[inline]
pub fn vs_17118_o23(v: &LmVertex, rows: &[[f32; 3]; 3], peel_dir: [f32; 3]) -> ([f32; 3], [f32; 4]) {
    // 22-28: r6 = the world normal (dp3 with the rows)
    let r6 = rotate(v.normal, rows);
    let t = [v.tangent[0], v.tangent[1], v.tangent[2]];
    // 38-39: 2.5 < |v4.x|
    if 2.5 < v.psize.abs() {
        return (r6, [t[0], t[1], t[2], 1.0]);
    }
    let (r2, r3) = if v.psize.abs() < 1.5 {
        // 46-47: r0 = cross(v2, v3); 48-52: × sign(v4.x) (0 when v4.x = 0)
        let r0 = cross(v.normal, t);
        let s = if 0.0 < v.psize { 1.0 } else if v.psize < 0.0 { -1.0 } else { 0.0 };
        let r0 = [r0[0] * s, r0[1] * s, r0[2] * s];
        // 53-55: r2 = rows·v3 ; 56-58: r3 = rows·r0
        (rotate(t, rows), rotate(r0, rows))
    } else {
        // 60-61: r0 = cross(v3, r6) (the object-space tangent against the WORLD normal, as written)
        let r0 = cross(t, r6);
        // 62-66: r2 = normalize(r0), 0 when degenerate
        let l2 = dot(r0, r0);
        let r2 = if 0.0 < l2 { let s = 1.0 / l2.sqrt(); [r0[0] * s, r0[1] * s, r0[2] * s] } else { [0.0; 3] };
        // 67-68: r3 = cross(r6, r2)
        (r2, cross(r6, r2))
    };
    // 70-72: o2 = (D·r2, D·r3, D·r6); 73: o3 = 0
    ([dot(peel_dir, r2), dot(peel_dir, r3), dot(peel_dir, r6)], [0.0; 4])
}

/// The literals of PS 17122 as the token stream holds them (`lmtool dxbc-literals shaders-frame127448/bin/
/// Pixel_17122.dxbc`; the disassembly prints six decimals).
pub const HB_FOUR_PI: f32 = f32::from_bits(0x41490fdb); //  12.566370964
pub const HB_C_LIN: f32 = f32::from_bits(0xbe6bdbc1); //  −0.230330482 (the sx / sy / sz linear terms)
pub const HB_C_Z: f32 = f32::from_bits(0x3ecc403f); //   0.398927659 (C0's sz term)
pub const HB_C_ONE: f32 = f32::from_bits(0x3e4c4267); //   0.199472055 (C0's constant)
pub const HB_C_Q2: f32 = f32::from_bits(0x3ddd1d80); //   0.107966423 (C2's (3sz² − 1) term)
pub const HB_C_Q0: f32 = f32::from_bits(0x3dbf7fec); //   0.093505710 (C0's (3sz² − 1) term)
pub const HB_C_XZ: f32 = f32::from_bits(0xbe25d689); //  −0.161951199 (C1 / C3's sy·sz / sx·sz term)
pub const HB_SWITCH: f32 = f32::from_bits(0x3f3504f3); //   0.707106769 (the |n.y| frame switch)

/// Whether `mad` is executed as a fused multiply-add (the GPU's FFMA; default) or as mul + add.
pub static HB_FMA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

#[inline]
fn mad(a: f32, b: f32, c: f32) -> f32 {
    if HB_FMA.load(std::sync::atomic::Ordering::Relaxed) { a.mul_add(b, c) } else { a * b + c }
}

/// `dp3` as the GPU evaluates it (a multiply then two fused adds when `HB_FMA`).
#[inline]
fn dp3(a: [f32; 3], b: [f32; 3]) -> f32 {
    if HB_FMA.load(std::sync::atomic::Ordering::Relaxed) { a[2].mul_add(b[2], a[1].mul_add(b[1], a[0] * b[0])) } else { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
}

/// `mul r, a.yzx, b.zxy; mad r, a.zxy, b.yzx, -r`: r.x = a.z·b.y ... as the bytecode pairs them —
/// r = (a.y b.z − a.z b.y, a.z b.x − a.x b.z, a.x b.y − a.y b.x) with the second product fused when `HB_FMA`.
#[inline]
fn cross_mad(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    // the DXBC: mul r, a.zxy(=a.z,a.x,a.y), b.yzx(=b.y,b.z,b.x) → r = (a.z b.y, a.x b.z, a.y b.x); mad r, a.yzx, b.zxy, -r
    let r = [a[2] * b[1], a[0] * b[2], a[1] * b[0]];
    [mad(a[1], b[2], -r[0]), mad(a[2], b[0], -r[1]), mad(a[0], b[1], -r[2])]
}

/// The four MRT outputs of PS 17122 for one pixel: `v2` = the interpolated o2, `v3` = the provoking vertex's o3
/// (nointerpolation), `l` = `TMapILightDir[px, py]`. Instruction by instruction (Pixel_17122.txt), the literals
/// from the token stream, `mad` fused (`HB_FMA`).
#[inline]
pub fn ps_17122(v2: [f32; 3], v3: [f32; 4], l: [f32; 3], cb: &HbCb) -> [[f32; 4]; 4] {
    // 1-34: (sx, sy, sz) = r1
    let r1: [f32; 3] = if 0.5 < v3[3] {
        let t = [v3[0], v3[1], v3[2]];
        // 3-4: r1 = cross(v2, v3); 5-9: normalize (0 when degenerate) — the bitangent from the tangent
        let c = cross_mad(v2, t);
        let l2 = dp3(c, c);
        let b = if 0.0 < l2 { let s = 1.0 / l2.sqrt(); [c[0] * s, c[1] * s, c[2] * s] } else { [0.0; 3] };
        // 10-11: r2 = cross(r1, v2) — the re-orthogonalised tangent
        let tt = cross_mad(b, v2);
        // 12-19: r3 = normalize(v2.z, 0, −v2.x) (0 when degenerate): 12: r3.xy = (v2.z, −v2.x); 13: dp2
        let r3xy = [v2[2] * 1.0, v2[0] * -1.0];
        let l2 = if HB_FMA.load(std::sync::atomic::Ordering::Relaxed) { r3xy[1].mul_add(r3xy[1], r3xy[0] * r3xy[0]) } else { r3xy[0] * r3xy[0] + r3xy[1] * r3xy[1] };
        let t0 = if 0.0 < l2 { let s = 1.0 / l2.sqrt(); [(v2[2] * 1.0) * s, 0.0 * s, (v2[0] * -1.0) * s] } else { [0.0; 3] };
        // 20-21: r4 = cross(r3, v2)?? — 20: mul r4, r3.yzx, v2.zxy ; 21: mad r4, v2.yzx, r3.zxy, -r4 → r4 = cross(v2, r3)
        let b0 = cross_mad(v2, t0);
        // 22-26: |v2.y| > 0.707107 → the mesh tangent's frame (r3 = r2, r4 = r1)
        let (tx, bx) = if HB_SWITCH < v2[1].abs() { (tt, b) } else { (t0, b0) };
        // 27-29
        [dp3(cb.peel_dir, tx), dp3(cb.peel_dir, bx), dp3(cb.peel_dir, v2)]
    } else {
        // 31-33: normalize(v2)
        let l2 = dp3(v2, v2);
        let s = 1.0 / l2.sqrt();
        [v2[0] * s, v2[1] * s, v2[2] * s]
    };
    let (sx, sy, sz) = (r1[0], r1[1], r1[2]);
    // 35: r1.w = InvDirCount · 4π ; 38: L' = L · r1.w
    let k = cb.inv_dir_count * HB_FOUR_PI;
    let lp = [l[0] * k, l[1] * k, l[2] * k];
    // 39: r2 = (sz·sz, sy·sz, sx·sz)
    let r2 = [sz * sz, sy * sz, sx * sz];
    // 40: r0.w = 3·sz² − 1 (mad)
    let q = mad(r2[0], 3.0, -1.0);
    // 41: r1.xy = (sy, sx) · −0.230330
    let mut r1x = sy * HB_C_LIN;
    let mut r1y = sx * HB_C_LIN;
    // 42: r1.w = sz · 0.398928 + 0.199472 (mad)
    let r1w = mad(sz, HB_C_Z, HB_C_ONE);
    // 43: r2.x = q · 0.107966 ; 44: r0.w = q · 0.093506 + r1.w (mad)
    let r2x = q * HB_C_Q2;
    let p0 = mad(q, HB_C_Q0, r1w);
    // 45: o0 = P0 · L'
    let o0 = [p0 * lp[0], p0 * lp[1], p0 * lp[2], cb.inv_dir_count];
    // 46: r1.xy = (sy·sz, sx·sz) · −0.161951 + r1.xy (mad)
    r1x = mad(r2[1], HB_C_XZ, r1x);
    r1y = mad(r2[2], HB_C_XZ, r1y);
    // 47: o1 = L' · r1.x
    let o1 = [lp[0] * r1x, lp[1] * r1x, lp[2] * r1x, cb.inv_dir_count];
    // 48: r0.w = sz · −0.230330 − r2.x (mad) ; 49: o2 = r0.w · L'
    let p2 = mad(sz, HB_C_LIN, -r2x);
    let o2 = [p2 * lp[0], p2 * lp[1], p2 * lp[2], cb.inv_dir_count];
    // 50: o3 = L' · r1.y
    let o3 = [lp[0] * r1y, lp[1] * r1y, lp[2] * r1y, cb.inv_dir_count];
    [o0, o1, o2, o3]
}

/// One H-basis draw.
#[derive(Clone, Debug)]
pub struct HbDraw {
    pub eid: u64,
    pub mesh: usize,
    pub instance_first: usize,
    pub instance_count: usize,
    pub raster: LmRasterCb,
    pub cb: HbCb,
}

/// The four RGBA16F MRTs (values as the f16 targets hold them, decoded to f32).
pub struct HbTargets {
    pub w: u32,
    pub h: u32,
    pub mrt: [Vec<[f32; 4]>; 4],
}

impl HbTargets {
    pub fn cleared(w: u32, h: u32) -> HbTargets {
        // four zeroed targets, each PRE-TOUCHED here (perf 8): a fresh zeroed allocation is mapped lazily, and the first
        // direction's H-basis draw then took 16 k page faults per target from 160 threads at once — 250 ms of contended
        // faulting per sweep (the three clones were touched by the copy, the fourth was not)
        let n = (w * h) as usize;
        let mk = || { let mut v = vec![[0.0f32; 4]; n]; touch_pages(&mut v); v };
        HbTargets { w, h, mrt: [mk(), mk(), mk(), mk()] }
    }
    /// From four captured buffers (RGBA16F decoded).
    pub fn from_bufs(b: [&Buf; 4]) -> HbTargets {
        let (w, h) = (b[0].w, b[0].h);
        let mut t = HbTargets::cleared(w, h);
        for k in 0..4 {
            for y in 0..h {
                for x in 0..w {
                    let i = (y * w + x) as usize;
                    t.mrt[k][i] = [b[k].get(x, y, 0), b[k].get(x, y, 1), b[k].get(x, y, 2), b[k].get(x, y, 3)];
                }
            }
        }
        t
    }
}

/// Blend one pixel-shader output into the f16 target the way this GPU does (`sunpass::BlendModel`).
#[inline]
pub fn blend_f16(dst: f32, src: f32, model: crate::sunpass::BlendModel) -> f32 {
    use crate::sunpass::BlendModel as B;
    let (s, r) = match model {
        B::RoundSum => (src, Rounding::NearestEven),
        B::RoundSrcAndSum => (quantise_f16(src, Rounding::NearestEven), Rounding::NearestEven),
        B::TruncSum => (src, Rounding::Truncate),
        B::TruncSrcAndSum => (quantise_f16(src, Rounding::Truncate), Rounding::Truncate),
        B::TruncSrcRoundSum => (quantise_f16(src, Rounding::Truncate), Rounding::NearestEven),
    };
    quantise_f16(dst + s, r)
}

/// Run the direction's four H-basis draws over `ilightdir` (the target after the direction's last accumulate)
/// into `tgt` (the MRTs before the direction). The provoking vertex of a triangle is its first index (D3D11).
/// `owner[i]` (when given) receives the mesh index of the last draw that touched pixel i; `probe` prints every
/// fragment of that pixel with its unquantised outputs.
pub fn run_hbasis(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[HbDraw], ilightdir: &DirTarget, tgt: &mut HbTargets, blend: crate::sunpass::BlendModel) {
    run_hbasis_probe(meshes, instances, table, draws, ilightdir, tgt, blend, None, None);
}

/// How the fragments of ONE pixel within the direction's draws reach the f16 target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FragModel {
    /// every fragment blended on its own: dst = f16(dst + f16(src)) per `BlendModel`
    Sequential,
    /// the ROP keeps the pixel in f32 while its fragments arrive: acc = dst + Σ src (f32), rounded once at the end —
    /// RTNE when two or more fragments landed, the lone fragment as the sequential model (src truncated, sum RTNE)
    F32Coalesced,
}

pub static HB_FRAG_MODEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn frag_model() -> FragModel {
    if HB_FRAG_MODEL.load(std::sync::atomic::Ordering::Relaxed) == 1 { FragModel::F32Coalesced } else { FragModel::Sequential }
}

struct HbPrep {
    di: usize,
    ii: usize,
    vs: Vec<HbVsOut>,
}

/// `run_hbasis_probe` band-parallel (no probe): see `band_plan`.
pub fn run_hbasis_par(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[HbDraw], ilightdir: &DirTarget, tgt: &mut HbTargets, blend: crate::sunpass::BlendModel, owner: Option<&mut Vec<u8>>) {
    let (w, h) = (tgt.w, tgt.h);
    let model = frag_model();
    let imode = interp_mode();
    let threads = crate::pool::pool().threads.max(1);
    let pairs: Vec<(usize, usize)> = draws.iter().enumerate().flat_map(|(di, d)| (d.instance_first..d.instance_first + d.instance_count).map(move |ii| (di, ii))).collect();
    let preps: Vec<HbPrep> = crate::pool::pool().map(pairs.len(), |k| {
        let (di, ii) = pairs[k];
        let d = &draws[di];
        let mesh = &meshes[d.mesh];
        let inst = &instances[ii];
        HbPrep { di, ii, vs: mesh.verts.iter().map(|v| vs_17118(v, inst, table, &d.raster, d.cb.peel_dir)).collect() }
    });
    let plan = band_plan(&preps, |p, t| { let idx = &meshes[draws[p.di].mesh].indices[t * 3..t * 3 + 3]; [p.vs[idx[0] as usize].clip, p.vs[idx[1] as usize].clip, p.vs[idx[2] as usize].clip] }, |p| meshes[draws[p.di].mesh].indices.len() / 3, h, threads);
    // the coalesced model's per-pixel running sums, first fragments and counts
    let n_px = (w * h) as usize;
    let mut acc: Vec<[[f32; 4]; 4]> = if model == FragModel::F32Coalesced { vec![[[0.0; 4]; 4]; n_px] } else { Vec::new() };
    let mut count: Vec<u8> = if model == FragModel::F32Coalesced { vec![0; n_px] } else { Vec::new() };
    let mut first: Vec<[[f32; 4]; 4]> = if model == FragModel::F32Coalesced { vec![[[0.0; 4]; 4]; n_px] } else { Vec::new() };
    let mrt_ptrs: [usize; 4] = [tgt.mrt[0].as_mut_ptr() as usize, tgt.mrt[1].as_mut_ptr() as usize, tgt.mrt[2].as_mut_ptr() as usize, tgt.mrt[3].as_mut_ptr() as usize];
    let (acc_p, count_p, first_p) = (acc.as_mut_ptr() as usize, count.as_mut_ptr() as usize, first.as_mut_ptr() as usize);
    let owner_p: Option<usize> = owner.as_ref().map(|o| o.as_ptr() as usize);
    let _ = &owner;
    crate::pool::pool().run(plan.n_bands, |b| {
        let (y_lo, y_hi) = ((b * plan.rows) as i64, (((b + 1) * plan.rows).min(h as usize)) as i64);
        for &(k, t) in &plan.lists[b] {
            let p = &preps[k as usize];
            let d = &draws[p.di];
            let mesh = &meshes[d.mesh];
            let tri = &mesh.indices[t as usize * 3..t as usize * 3 + 3];
            let (a, bb, c) = (&p.vs[tri[0] as usize], &p.vs[tri[1] as usize], &p.vs[tri[2] as usize]);
            let v3 = a.o3;
            let _ = p.ii;
            crate::sunpass::rasterise_triangle_rows([a.clip, bb.clip, c.clip], w, h, y_lo, y_hi, |x, y, b0, b1, b2| {
                let v2 = if imode == 0 { [a.o2[0] * b0 + bb.o2[0] * b1 + c.o2[0] * b2, a.o2[1] * b0 + bb.o2[1] * b1 + c.o2[1] * b2, a.o2[2] * b0 + bb.o2[2] * b1 + c.o2[2] * b2] } else { interp3(imode, [crate::sunpass::screen_snapped([a.clip[0], a.clip[1]], w, h), crate::sunpass::screen_snapped([bb.clip[0], bb.clip[1]], w, h), crate::sunpass::screen_snapped([c.clip[0], c.clip[1]], w, h)], [a.o2, bb.o2, c.o2], [b0, b1, b2], x as f32 + 0.5, y as f32 + 0.5) };
                let l = ilightdir.rgb(x, y);
                let o = ps_17122(v2, v3, l, &d.cb);
                let i = (y * w + x) as usize;
                // SAFETY: the bands own disjoint pixel rows
                unsafe {
                    if let Some(op) = owner_p { *(op as *mut u8).add(i) = d.mesh as u8 + 1; }
                    match model {
                        FragModel::Sequential => {
                            for k in 0..4 {
                                let slot = &mut *(mrt_ptrs[k] as *mut [f32; 4]).add(i);
                                for ch in 0..4 { slot[ch] = blend_f16(slot[ch], o[k][ch], blend); }
                            }
                        }
                        FragModel::F32Coalesced => {
                            let cnt = &mut *(count_p as *mut u8).add(i);
                            if *cnt == 0 { *(first_p as *mut [[f32; 4]; 4]).add(i) = o; }
                            let ac = &mut *(acc_p as *mut [[f32; 4]; 4]).add(i);
                            for k in 0..4 { for ch in 0..4 { ac[k][ch] += o[k][ch]; } }
                            *cnt = cnt.saturating_add(1);
                        }
                    }
                }
            });
        }
    });
    if model == FragModel::F32Coalesced {
        for i in 0..n_px {
            match count[i] {
                0 => {}
                1 => { for k in 0..4 { for ch in 0..4 { tgt.mrt[k][i][ch] = blend_f16(tgt.mrt[k][i][ch], first[i][k][ch], blend); } } }
                _ => { for k in 0..4 { for ch in 0..4 { tgt.mrt[k][i][ch] = quantise_f16(tgt.mrt[k][i][ch] + acc[i][k][ch], Rounding::NearestEven); } } }
            }
        }
    }
}

pub fn run_hbasis_probe(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[HbDraw], ilightdir: &DirTarget, tgt: &mut HbTargets, blend: crate::sunpass::BlendModel, mut owner: Option<&mut Vec<u8>>, probe: Option<(u32, u32)>) {
    if probe.is_none() && std::env::var_os("LMTOOL_LMACCUM_SERIAL").is_none() {
        return run_hbasis_par(meshes, instances, table, draws, ilightdir, tgt, blend, owner);
    }
    let (w, h) = (tgt.w, tgt.h);
    let model = frag_model();
    let imode = interp_mode();
    // the coalesced model: the f32 running sum of this direction's fragments per pixel and their count
    let mut acc: Vec<[[f32; 4]; 4]> = if model == FragModel::F32Coalesced { vec![[[0.0; 4]; 4]; (w * h) as usize] } else { Vec::new() };
    let mut count: Vec<u8> = if model == FragModel::F32Coalesced { vec![0; (w * h) as usize] } else { Vec::new() };
    let mut first: Vec<[[f32; 4]; 4]> = if model == FragModel::F32Coalesced { vec![[[0.0; 4]; 4]; (w * h) as usize] } else { Vec::new() };
    for d in draws {
        let mesh = &meshes[d.mesh];
        for ii in d.instance_first..d.instance_first + d.instance_count {
            let inst = &instances[ii];
            let vs: Vec<HbVsOut> = mesh.verts.iter().map(|v| vs_17118(v, inst, table, &d.raster, d.cb.peel_dir)).collect();
            for (ti, tri) in mesh.indices.chunks_exact(3).enumerate() {
                let (a, b, c) = (&vs[tri[0] as usize], &vs[tri[1] as usize], &vs[tri[2] as usize]);
                let v3 = a.o3;
                rasterise_triangle([a.clip, b.clip, c.clip], w, h, |x, y, b0, b1, b2| {
                    let v2 = if imode == 0 { [a.o2[0] * b0 + b.o2[0] * b1 + c.o2[0] * b2, a.o2[1] * b0 + b.o2[1] * b1 + c.o2[1] * b2, a.o2[2] * b0 + b.o2[2] * b1 + c.o2[2] * b2] } else { interp3(imode, [crate::sunpass::screen_snapped([a.clip[0], a.clip[1]], w, h), crate::sunpass::screen_snapped([b.clip[0], b.clip[1]], w, h), crate::sunpass::screen_snapped([c.clip[0], c.clip[1]], w, h)], [a.o2, b.o2, c.o2], [b0, b1, b2], x as f32 + 0.5, y as f32 + 0.5) };
                    let l = ilightdir.rgb(x, y);
                    let o = ps_17122(v2, v3, l, &d.cb);
                    let i = (y * w + x) as usize;
                    if probe == Some((x, y)) {
                        eprintln!("probe ({x},{y}): mesh {} instance {ii} tri {ti} [{},{},{}] bary ({b0:.6},{b1:.6},{b2:.6}) v2 ({:.7},{:.7},{:.7}) v3 {:?} L ({:.5},{:.5},{:.5}); before {:?}", d.mesh, tri[0], tri[1], tri[2], v2[0], v2[1], v2[2], v3, l[0], l[1], l[2], [tgt.mrt[0][i], tgt.mrt[1][i], tgt.mrt[2][i], tgt.mrt[3][i]]);
                        for k in 0..4 { eprintln!("    C{k} src f32 {:?} → f16 rtz {:?}", o[k], o[k].map(|v| quantise_f16(v, Rounding::Truncate))); }
                        // the interpolation study: the third weight derived (1 − the other two) for each vertex, and the plane-equation
                        // form a = A·x + B·y + C from the snapped vertex positions — which one moves C0.r across its f16 boundary?
                        let bary = [b0, b1, b2];
                        for dv in 0..3 {
                            let mut bb = bary; bb[dv] = 1.0 - bary[(dv + 1) % 3] - bary[(dv + 2) % 3];
                            let v2b = [a.o2[0] * bb[0] + b.o2[0] * bb[1] + c.o2[0] * bb[2], a.o2[1] * bb[0] + b.o2[1] * bb[1] + c.o2[1] * bb[2], a.o2[2] * bb[0] + b.o2[2] * bb[1] + c.o2[2] * bb[2]];
                            let ob = ps_17122(v2b, v3, l, &d.cb);
                            eprintln!("    derived weight for vertex slot {dv}: b {:?} v2 {:?} C0 {:?} → rtz {:?}", bb, v2b, ob[0], ob[0].map(|v| quantise_f16(v, Rounding::Truncate)));
                        }
                        for dv in 0..3 {
                            // a(x,y) = a_dv + (a_j − a_dv)·b_j + (a_k − a_dv)·b_k (the differences from the reference vertex, two fmas)
                            let os = [&a.o2, &b.o2, &c.o2];
                            let (j, k) = ((dv + 1) % 3, (dv + 2) % 3);
                            let mut v2b = [0f32; 3];
                            for ch in 0..3 { v2b[ch] = (os[k][ch] - os[dv][ch]).mul_add(bary[k], (os[j][ch] - os[dv][ch]).mul_add(bary[j], os[dv][ch])); }
                            let ob = ps_17122(v2b, v3, l, &d.cb);
                            eprintln!("    reference vertex slot {dv} (delta form, fma): v2 {:?} C0 {:?} → rtz {:?}", v2b, ob[0], ob[0].map(|v| quantise_f16(v, Rounding::Truncate)));
                        }
                    }
                    if let Some(ow) = owner.as_deref_mut() { ow[i] = d.mesh as u8 + 1; }
                    match model {
                        FragModel::Sequential => {
                            for k in 0..4 {
                                for ch in 0..4 {
                                    tgt.mrt[k][i][ch] = blend_f16(tgt.mrt[k][i][ch], o[k][ch], blend);
                                }
                            }
                        }
                        FragModel::F32Coalesced => {
                            if count[i] == 0 { first[i] = o; }
                            for k in 0..4 { for ch in 0..4 { acc[i][k][ch] += o[k][ch]; } }
                            count[i] = count[i].saturating_add(1);
                        }
                    }
                });
            }
        }
    }
    if model == FragModel::F32Coalesced {
        for i in 0..(w * h) as usize {
            match count[i] {
                0 => {}
                1 => { for k in 0..4 { for ch in 0..4 { tgt.mrt[k][i][ch] = blend_f16(tgt.mrt[k][i][ch], first[i][k][ch], blend); } } }
                _ => { for k in 0..4 { for ch in 0..4 { tgt.mrt[k][i][ch] = quantise_f16(tgt.mrt[k][i][ch] + acc[i][k][ch], Rounding::NearestEven); } } }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// AddAmbient
// ---------------------------------------------------------------------------------------------------------------

/// CS 17125 for one dispatch: `accum.xyz += Scale · color[W >> 1, H >> 1]`, `accum.w += Scale · 0.5` — an f32 buffer,
/// the `mad`s FUSED (the captured UAV reproduces bit for bit with fma, one f32 ulp off without). The dispatch runs
/// for the UPWARD directions of the first sweep only (FUN_140234df0 l.562–594: `0 < 4·w·D.y·Sky`; w = 2/N), with
/// Scale = 4·w·D.y·Sky = D.y/32 for N = 256 — the downward directions add nothing.
#[inline]
pub fn cs_17125(accum: &mut [f32; 4], centre_rgb: [f32; 3], scale: f32) {
    for k in 0..3 {
        accum[k] = centre_rgb[k].mul_add(scale, accum[k]);
    }
    accum[3] = scale.mul_add(0.5, accum[3]);
}

/// The pixel the compute shader reads: `resinfo` → (W, H) >> 1.
#[inline]
pub fn ambient_pixel(w: u32, h: u32) -> (u32, u32) {
    (w >> 1, h >> 1)
}

// ---------------------------------------------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------------------------------------------

/// Compare two R11G11B10 targets: (pixels where either side is non-zero, exact pixels, pixels off by ≤ 1 quantum in
/// every channel, worse, ours-only non-zero, game-only non-zero).
pub struct DirCmp {
    pub touched: usize,
    pub exact: usize,
    pub quantum: usize,
    pub worse: usize,
    pub ours_only: usize,
    pub game_only: usize,
}

pub fn compare_dir(ours: &DirTarget, game: &Buf) -> DirCmp {
    let mut c = DirCmp { touched: 0, exact: 0, quantum: 0, worse: 0, ours_only: 0, game_only: 0 };
    for y in 0..ours.h {
        for x in 0..ours.w {
            let o = ours.rgb(x, y);
            let g = [game.get(x, y, 0), game.get(x, y, 1), game.get(x, y, 2)];
            let on = o.iter().any(|v| *v != 0.0);
            let gn = g.iter().any(|v| *v != 0.0);
            if !on && !gn {
                continue;
            }
            c.touched += 1;
            if on && !gn { c.ours_only += 1; }
            if gn && !on { c.game_only += 1; }
            let go = crate::gpufmt::pack_r11g11b10(g, Rounding::Truncate);
            if go == ours.px[(y * ours.w + x) as usize] {
                c.exact += 1;
            } else {
                // one quantum = the next representable value of the game's channel
                let within = (0..3).all(|k| {
                    let (mb, sh) = if k == 2 { (5, 22) } else { (6, k * 11) };
                    let gq = (go >> sh) & ((1 << (mb + 5)) - 1);
                    let oq = (ours.px[(y * ours.w + x) as usize] >> sh) & ((1 << (mb + 5)) - 1);
                    (gq as i64 - oq as i64).abs() <= 1
                });
                if within { c.quantum += 1 } else { c.worse += 1 }
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_offsets_reproduce_the_captured_cbuffers_bit_for_bit() {
        // frame 127448 (direction 0 of sweep 0): LM01_Trans_RasterSS (−1.0004340410232544, 0.9997829794883728)
        let c0 = LmRasterCb::for_offset(0, 2048, 2048);
        assert_eq!(c0.trans_ss, [-1.0004340410232544f64 as f32, 0.9997829794883728f64 as f32]);
        // frame 40648 (offset 2 of the cycle): (−0.9997829794883728, 0.9995659589767456)
        let c2 = LmRasterCb::for_offset(2, 2048, 2048);
        assert_eq!(c2.trans_ss, [-0.9997829794883728f64 as f32, 0.9995659589767456f64 as f32]);
        // offset 4 = (0, 0): the un-jittered raster
        assert_eq!(LmRasterCb::for_offset(4, 2048, 2048).trans_ss, [-1.0, 1.0]);
    }

    #[test]
    fn point_texel_clamps_to_the_edge() {
        assert_eq!(point_texel(-0.2, 4096), 0);
        assert_eq!(point_texel(0.0, 4096), 0);
        assert_eq!(point_texel(0.5, 4096), 2048);
        assert_eq!(point_texel(1.0, 4096), 4095);
        assert_eq!(point_texel(1.7, 4096), 4095);
        assert_eq!(point_texel(0.99999, 4096), 4095);
    }

    #[test]
    fn hbasis_c0_of_a_texel_facing_the_direction_is_pi_over_four_times_l_over_n() {
        // mode A vertex data: n = up, t = x; D = up → sz = 1, sx = sy = 0: P0(1) = 0.093506·2 + 0.398928 + 0.199472 = 0.785412
        let cb = HbCb { peel_dir: [0.0, 1.0, 0.0], inv_dir_count: 1.0 / 256.0 };
        let o = ps_17122([0.0, 1.0, 0.0], [1.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0], &cb);
        let k = 12.566371f32 / 256.0;
        assert!((o[0][0] - 0.785412 * k).abs() < 1e-6, "{:?}", o[0]);
        assert_eq!(o[0][3], 1.0 / 256.0);
        // C1 (−0.23033 sy − 0.161951 sy sz) and C3 (sx) vanish at the zenith; C2 = −0.23033 − 0.107966·2
        assert!(o[1][0].abs() < 1e-7 && o[3][0].abs() < 1e-7);
        assert!((o[2][0] - (-0.230330 - 0.107966 * 2.0) * k).abs() < 1e-6, "{:?}", o[2]);
    }

    #[test]
    fn hbasis_frame_switches_on_the_normals_y_at_0_707107() {
        // a wall (n = +z) takes T0 = normalize(n.z, 0, −n.x) = +x, B0 = n × T0 = +y: D = +y → (sx, sy, sz) = (0, 1, 0)
        let cb = HbCb { peel_dir: [0.0, 1.0, 0.0], inv_dir_count: 1.0 };
        let o = ps_17122([0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0], &cb);
        let k = 12.566371f32;
        // C1 = −0.23033·sy·L' with sy = 1
        assert!((o[1][0] - -0.230330 * k).abs() < 1e-5, "{:?}", o[1]);
        assert!(o[3][0].abs() < 1e-6);
        // a floor (n = +y) with the mesh tangent t = +z: B = normalize(n × t) = +x, T = B × n = +z; D = +z → sx = 1
        let o = ps_17122([0.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0], [1.0, 0.0, 0.0], &HbCb { peel_dir: [0.0, 0.0, 1.0], inv_dir_count: 1.0 });
        assert!((o[3][0] - -0.230330 * k).abs() < 1e-5, "{:?}", o[3]);
        assert!(o[1][0].abs() < 1e-6, "{:?}", o[1]);
    }

    #[test]
    fn set_pass_discards_back_faces_and_failed_compares_and_copies_the_layer_pixel() {
        let mut color = Buf::new(4, 4, 3);
        let mut depth = Buf::new(4, 4, 1);
        for y in 0..4 { for x in 0..4 { color.set(x, y, 0, 0.25 + x as f32 * 0.1); depth.set(x, y, 0, 0.5); } }
        let layer = LayerTargets { color: &color, depth: &depth };
        // an identity-like projection: u = x/4 + 0.125.., z = the position's y
        let mut m = [[0.0f32; 4]; 4];
        m[0][0] = 0.25; m[2][1] = 0.25; m[1][2] = 1.0; m[3][3] = 1.0;
        let cb = SetCb { world_pw01_shadow: m, peel_dir: [0.0, 1.0, 0.0] };
        // facing away → discarded
        assert!(ps_17112([2.0, 0.7, 0.0], [0.0, -1.0, 0.0], &cb, &layer, DepthCompare::Float).is_none());
        // z = 0.7 ≥ stored 0.5 → the layer pixel at u = 0.5 → texel 2 → 0.45
        assert_eq!(ps_17112([2.0, 0.7, 0.0], [0.0, 1.0, 0.0], &cb, &layer, DepthCompare::Float), Some([0.45, 0.0, 0.0]));
        // z = 0.3 < 0.5 → discarded
        assert!(ps_17112([2.0, 0.3, 0.0], [0.0, 1.0, 0.0], &cb, &layer, DepthCompare::Float).is_none());
        // exactly at the stored depth passes (GreaterEqual)
        assert!(ps_17112([2.0, 0.5, 0.0], [0.0, 1.0, 0.0], &cb, &layer, DepthCompare::Float).is_some());
    }

    #[test]
    fn add_ambient_is_a_running_sum_with_half_scale_in_w() {
        let mut acc = [0.0f32; 4];
        cs_17125(&mut acc, [0.3, 0.5, 0.9], 0.010867852717638016f64 as f32);
        cs_17125(&mut acc, [0.3, 0.5, 0.9], 0.010867852717638016f64 as f32);
        let s = 0.010867852717638016f64 as f32;
        assert_eq!(acc[3], s * 0.5 + (s * 0.5 + 0.0));
        assert!((acc[0] - 2.0 * 0.3 * s).abs() < 1e-7);
        assert_eq!(ambient_pixel(4096, 4096), (2048, 2048));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The capture's data (passcap/pwc-day) and the check drivers behind `lmtool ilightdir-check / hbasis-check /
// ambient-check`
// ---------------------------------------------------------------------------------------------------------------

use serde_json::Value;
use std::path::Path;

/// The LM scene as the capture holds it: the four LM meshes in the order of frame 127448's accumulate block
/// (the pad (24 indices, 1 instance), the wall (12, 1), the vegetation item (5751, 1), the zone tile (24, 4096)),
/// each with its first instance in the instance stream, the 4099 instances (vb_17033.bin) and the chart ST table
/// (g_TcLM_ST_LM01).
#[derive(Debug)]
pub struct LmScene {
    pub meshes: Vec<LmMesh>,
    pub inst_first: Vec<usize>,
    pub inst_count: Vec<usize>,
    pub instances: Vec<LmInstance>,
    pub table: Vec<[f32; 4]>,
    /// The mesh.json eids the meshes came from.
    pub eids: Vec<u64>,
    /// THE LM RASTER ONCE PER JITTER OFFSET (perf 8): the fragment lists of the nine raster offsets, built on first use and
    /// kept for the whole bake (`frag_list`) — every LmILightDir_Set block and every H-basis draw replays them.
    pub frag_lists: [std::sync::OnceLock<std::sync::Arc<LmFragList>>; 9],
    /// The fitted blocks' world box the tile peels clip to (LmRasterPosNrm_Inst_v's ClipWorldBoxXZ) — one box per bake, so
    /// each fragment list carries the clip decision per fragment, computed once (perf 8.20).
    pub fitted_world_box: Option<[[f32; 2]; 2]>,
    /// Per instance: the layout record behind it (chart k ↔ record k) when the scene came from the map through the record
    /// pipeline (lmmesh::lm_scene_from_map_at / lm_scene_add_entities) — the local-light pass culls records and draws
    /// their instances; empty for a captured scene.
    pub rec_of: Vec<usize>,
    /// Per instance: the layout rect and the uv bounds its ST was computed from (peelcolor::chart_st) — the local-light frame
    /// recomputes the STs for its own target size (localdrive::instances_for_target); empty for a captured scene.
    pub st_src: Vec<([i32; 4], [f32; 4])>,
    /// Per LM instance: the PORT scene instance it was built from (`lm_scene_from_map_at`: the items grouped by model), or
    /// usize::MAX for a synthetic instance (the zone tiles) — the exact port → LM map the atlas colour lookup needs (two items at
    /// one translation — np-tk3's pillar on its plate — defeat a nearest-translation search: the plate read the pillar's chart
    /// and the pillar's base lost the plate's bounce; port engineer G, 2026-09-26). Empty for a captured scene.
    pub port_inst: Vec<usize>,
    /// Per LM mesh: the prefab entity's FULL visual as caster triangles in the entity's local frame (every geom, lightmapped or not —
    /// the flat cube's caster set is the record's whole visual, RE 13 2026-09-26 21:20Z: the lamp housing's idx-276 draw in the caster
    /// pass; RE 7's f4936 caster draws are all CullMode.Back). Empty for the item and tile meshes (items take the scene visual through
    /// localdrive::CasterSource; tiles are their LM mesh).
    pub caster_tris: Vec<Vec<[[f32; 3]; 3]>>,
}

impl LmScene {
    /// The mesh drawn with `idx` indices × `inst` instances (the accumulate / H-basis draws name no vertex
    /// buffer in the log; the pair identifies the object).
    pub fn mesh_for(&self, idx: usize, inst: usize) -> Option<usize> {
        (0..self.meshes.len()).find(|&i| self.meshes[i].indices.len() == idx && self.inst_count[i] == inst)
    }
}

fn json_f32(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn json_v3(v: &Value) -> [f32; 3] {
    [json_f32(&v[0]), json_f32(&v[1]), json_f32(&v[2])]
}

fn json_v2(v: &Value) -> [f32; 2] {
    [json_f32(&v[0]), json_f32(&v[1])]
}

fn json_m4(v: &Value) -> [[f32; 4]; 4] {
    let mut o = [[0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            o[i][j] = json_f32(&v[i][j]);
        }
    }
    o
}

/// The draws log of a frame (`logs/draws-frame<N>.json[.gz]`, or the top-level `draws-frame<N>.json.gz`).
pub fn load_draws(root: &Path, frame: u32) -> Result<Vec<Value>, String> {
    let bytes = crate::passdiff::read_entry_bytes(root, &format!("logs/draws-frame{frame}.json")).or_else(|_| crate::passdiff::read_entry_bytes(root, &format!("draws-frame{frame}.json")))?;
    let v: Value = serde_json::from_slice(&bytes).map_err(|e| format!("draws-frame{frame}: {e}"))?;
    v.as_array().cloned().ok_or_else(|| "draws log is not an array".into())
}

/// The LM scene from `env/frame<N>/` (mesh.json's records with the 40-byte LM stream + the instance stream) and
/// the instance counts of the matching sun draws (PS 15187) in that frame's log.
pub fn load_lm_scene(root: &Path, env_frame: u32) -> Result<LmScene, String> {
    let env = root.join(format!("env/frame{env_frame}"));
    let mesh_json: Value = serde_json::from_str(&std::fs::read_to_string(env.join("mesh.json")).map_err(|e| format!("mesh.json: {e}"))?).map_err(|e| format!("mesh.json: {e}"))?;
    let draws = load_draws(root, env_frame)?;
    // the sun draws of the first block name the four objects with their instance counts
    let sun: Vec<&Value> = draws.iter().filter(|e| e.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some("15187")).take(4).collect();
    if sun.len() != 4 {
        return Err(format!("frame {env_frame}: {} sun draws (PS 15187) in the log, 4 expected", sun.len()));
    }
    let mut sc = LmScene { caster_tris: Vec::new(), meshes: Vec::new(), inst_first: Vec::new(), inst_count: Vec::new(), instances: Vec::new(), table: Vec::new(), eids: Vec::new(), frag_lists: Default::default(), fitted_world_box: None, rec_of: Vec::new(), st_src: Vec::new(), port_inst: Vec::new() };
    let mut instance_bytes: Option<Vec<u8>> = None;
    for e in &sun {
        let eid = e["eid"].as_u64().unwrap();
        let rec = mesh_json.as_array().unwrap().iter().find(|r| r["eid"].as_u64() == Some(eid)).ok_or_else(|| format!("mesh.json has no eid {eid}"))?;
        let vbs = rec["vertex_buffers"].as_array().ok_or("vertex_buffers")?;
        let vb0 = std::fs::read(env.join("mesh").join(vbs[0]["file"].as_str().unwrap())).map_err(|e| format!("vb0: {e}"))?;
        let idx_file = rec["vsout"]["index_file"].as_str().ok_or("index file")?;
        let ib = std::fs::read(env.join("mesh").join(idx_file)).map_err(|e| format!("indices: {e}"))?;
        let indices: Vec<u16> = ib.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        sc.meshes.push(LmMesh { verts: crate::sunpass::parse_lm_vertices(&vb0), indices });
        sc.inst_first.push((vbs[1]["offset"].as_u64().unwrap_or(0) / 48) as usize);
        sc.inst_count.push(e["inst"].as_u64().unwrap_or(1).max(1) as usize);
        sc.eids.push(eid);
        if instance_bytes.is_none() {
            instance_bytes = Some(std::fs::read(env.join("mesh").join(vbs[1]["file"].as_str().unwrap())).map_err(|e| format!("instance vb: {e}"))?);
        }
    }
    sc.instances = crate::sunpass::parse_instances(instance_bytes.as_deref().unwrap_or(&[]));
    // the chart table: the VS SRV 0 of the first sun draw
    let first = sc.eids[0];
    let dir = env.join("bufs");
    let table_file = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.filter_map(|d| d.ok()).map(|d| d.file_name().to_string_lossy().to_string()).find(|n| n.starts_with(&format!("e{first:06}_Vertex_srv0_")));
    if let Some(f) = table_file {
        let b = std::fs::read(dir.join(&f)).map_err(|e| format!("{f}: {e}"))?;
        sc.table = b.chunks_exact(16).map(|c| [f32::from_le_bytes(c[0..4].try_into().unwrap()), f32::from_le_bytes(c[4..8].try_into().unwrap()), f32::from_le_bytes(c[8..12].try_into().unwrap()), f32::from_le_bytes(c[12..16].try_into().unwrap())]).collect();
    }
    Ok(sc)
}

/// A manifest entry as the capture writes it (a `serde_json::Value` view; the fields the checks need).
#[derive(Clone, Debug)]
pub struct CapEntry {
    pub pass: String,
    pub capture: String,
    pub frame: u32,
    pub direction: Option<u32>,
    pub sweep_direction_index: Option<u32>,
    pub sweep: Option<u32>,
    pub phase: Option<String>,
    pub layer: Option<u32>,
    pub eid_first: u64,
    pub eid_last: u64,
    pub file: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub dir: Option<[f32; 3]>,
    pub banked: bool,
    pub cbuffers: Value,
}

pub fn load_capture_entries(manifest: &Path) -> Result<Vec<CapEntry>, String> {
    let txt = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let v: Value = serde_json::from_str(&txt).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let u = |x: &Value| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()));
    let mut out = Vec::new();
    for e in v["passes"].as_array().ok_or("manifest has no `passes`")? {
        let eid = u(&e["eid"]);
        out.push(CapEntry {
            pass: e["pass"].as_str().unwrap_or("").to_string(),
            capture: e["capture"].as_str().unwrap_or("").to_string(),
            frame: u(&e["frame"]).unwrap_or(0) as u32,
            direction: u(&e["direction"]).map(|x| x as u32),
            sweep_direction_index: u(&e["sweep_direction_index"]).map(|x| x as u32),
            sweep: u(&e["sweep"]).map(|x| x as u32),
            phase: e["phase"].as_str().map(|s| s.to_string()),
            layer: u(&e["layer"]).map(|x| x as u32),
            eid_first: u(&e["eid_first"]).or(eid).unwrap_or(0),
            eid_last: u(&e["eid_last"]).or(eid).unwrap_or(0),
            file: e["file"].as_str().unwrap_or("").to_string(),
            format: e["format"].as_str().unwrap_or("").to_string(),
            width: u(&e["width"]).unwrap_or(0) as u32,
            height: u(&e["height"]).unwrap_or(0) as u32,
            dir: if e["dir"].is_array() { Some(json_v3(&e["dir"])) } else { None },
            banked: e["banked"].as_bool().unwrap_or(true),
            cbuffers: e["cbuffers"].clone(),
        });
    }
    Ok(out)
}

impl CapEntry {
    pub fn load(&self, root: &Path) -> Result<Buf, String> {
        crate::passdiff::load_file(root, &self.file, &self.format, self.width, self.height, 0)
    }
    /// The H-basis draw constants an `hbasis*` entry carries.
    pub fn hb_constants(&self) -> Option<(HbCb, LmRasterCb)> {
        let p = &self.cbuffers["ShaderP"]["ShaderP"]["g_CBufferP"];
        let v = &self.cbuffers["ShaderV"]["ShaderV"]["g_CBufferV"];
        let dir = if p["PeelDirInW"].is_array() { json_v3(&p["PeelDirInW"]) } else { self.dir? };
        let inv = if p["InvDirCount"].is_number() { json_f32(&p["InvDirCount"]) } else { return None };
        let raster = if v["LM01_Trans_RasterSS"].is_array() { LmRasterCb { scale_ss: json_v2(&v["LM01_Scale_RasterSS"]), trans_ss: json_v2(&v["LM01_Trans_RasterSS"]) } } else { LmRasterCb::for_offset(self.sweep_direction_index? as usize, self.width.max(1), self.height.max(1)) };
        Some((HbCb { peel_dir: dir, inv_dir_count: inv }, raster))
    }
}

/// One accumulate block of a frame's log: its four draws with their constants and the eid range.
pub struct SetBlock {
    pub eid_first: u64,
    pub eid_last: u64,
    pub draws: Vec<SetDraw>,
}

/// The accumulate blocks (PS 17112 / 17526) of a frame's draws log, in order, grouped four by four.
pub fn set_blocks(draws: &[Value], scene: &LmScene) -> Result<Vec<SetBlock>, String> {
    set_blocks_with(draws, scene, &["17112", "17526"], &["17115", "17529"])
}

/// `set_blocks` with the capture's own shader ids (RenderDoc ids are per capture: pwc6's accumulate is PS 8507
/// with the fitted-frustum VS 8510).
pub fn set_blocks_with(draws: &[Value], scene: &LmScene, ps_ids: &[&str], fitted_vs_ids: &[&str]) -> Result<Vec<SetBlock>, String> {
    let mut blocks: Vec<SetBlock> = Vec::new();
    for e in draws {
        let ps = e.pointer("/Pixel/shader").and_then(|v| v.as_str()).unwrap_or("");
        if !ps_ids.contains(&ps) {
            continue;
        }
        let eid = e["eid"].as_u64().unwrap_or(0);
        let idx = e["idx"].as_u64().unwrap_or(0) as usize;
        let inst = e["inst"].as_u64().unwrap_or(1).max(1) as usize;
        let mesh = scene.mesh_for(idx, inst).ok_or_else(|| format!("eid {eid}: no LM mesh with {idx} indices × {inst} instances"))?;
        let pcb = &e["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"];
        let vcb = &e["Vertex"]["cbuffers"]["ShaderV"]["g_CBufferV"];
        let vs_id = e.pointer("/Vertex/shader").and_then(|v| v.as_str()).unwrap_or("");
        // the fitted blocks' vertex shader clips to the items' world box (VS 17115 / pwc1 17529)
        let world_box = if fitted_vs_ids.contains(&vs_id) { Some([json_v2(&vcb["WorldBoxMinXZ"]), json_v2(&vcb["WorldBoxMaxXZ"])]) } else { None };
        let d = SetDraw {
            eid,
            mesh,
            instance_first: scene.inst_first[mesh],
            instance_count: inst,
            raster: LmRasterCb { scale_ss: json_v2(&vcb["LM01_Scale_RasterSS"]), trans_ss: json_v2(&vcb["LM01_Trans_RasterSS"]) },
            cb: SetCb { world_pw01_shadow: json_m4(&pcb["WorldPw01Shadow"]), peel_dir: json_v3(&pcb["PeelDirInW"]) },
            world_box,
        };
        match blocks.last_mut() {
            Some(b) if b.draws.len() < 4 && eid - b.eid_last <= 6 => { b.eid_last = eid; b.draws.push(d); }
            _ => blocks.push(SetBlock { eid_first: eid, eid_last: eid, draws: vec![d] }),
        }
    }
    Ok(blocks)
}

/// The report of one comparison line.
pub fn fmt_dircmp(c: &DirCmp) -> String {
    let pct = |n: usize| if c.touched > 0 { 100.0 * n as f64 / c.touched as f64 } else { 0.0 };
    format!("touched {:>8}  exact {:>8} ({:6.2} %)  ±1 quantum {:>6} ({:5.2} %)  worse {:>6} ({:5.2} %)  ours-only {:>6}  game-only {:>6}", c.touched, c.exact, pct(c.exact), c.quantum, pct(c.quantum), c.worse, pct(c.worse), c.ours_only, c.game_only)
}

/// Per-channel f16 comparison of one MRT: (values compared, exact, within 1 ulp of the game's value, worse, max |Δ|),
/// over the pixels where either side is non-zero in that channel.
pub fn compare_mrt(ours: &[[f32; 4]], game: &Buf, ch: usize) -> (usize, usize, usize, usize, f32, (u32, u32, f32, f32)) {
    let (mut n, mut exact, mut ulp1, mut worse, mut maxd) = (0usize, 0usize, 0usize, 0usize, 0f32);
    let mut worst = (0u32, 0u32, 0f32, 0f32);
    for y in 0..game.h {
        for x in 0..game.w {
            let i = (y * game.w + x) as usize;
            let g = game.get(x, y, ch as u32);
            let o = ours[i][ch];
            if g == 0.0 && o == 0.0 {
                continue;
            }
            n += 1;
            let d = (o - g).abs();
            if d == 0.0 {
                exact += 1;
            } else {
                let ulp = (crate::gpufmt::decode_f16(crate::gpufmt::encode_f16(g, Rounding::NearestEven).wrapping_add(1)) - g).abs();
                if d <= ulp * 1.001 { ulp1 += 1 } else { worse += 1 }
                if d > maxd { maxd = d; worst = (x, y, g, o); }
            }
        }
    }
    (n, exact, ulp1, worse, maxd, worst)
}

/// The attribute interpolation model of the H-basis raster (HB_INTERP): `bary` (Σ a_k·b_k, the default), `plane-abs`
/// (the plane a = A·x + B·y + C through the three snapped vertices, evaluated at the absolute pixel centre — the setup
/// engine's form), `plane-ref0` (a = a₀ + A·(x − x₀) + B·(y − y₀) from the first vertex), `plane-fma` (plane-abs with
/// fused evaluation). The vegetation's varying normals are the only attribute that tells them apart (the flat meshes
/// interpolate constants).
pub fn interp_mode() -> u8 {
    match std::env::var("HB_INTERP").as_deref() {
        Ok("plane-abs") => 1,
        Ok("plane-ref0") => 2,
        Ok("plane-fma") => 3,
        Ok("plane-abs-sep") => 4,
        _ => 0,
    }
}

/// Interpolate one 3-vector attribute at the pixel centre (cx, cy) under `mode` — `p` = the snapped screen positions of
/// the three vertices, `at` = their attribute values, `b` = the rasteriser's barycentrics.
pub fn interp3(mode: u8, p: [[f32; 2]; 3], at: [[f32; 3]; 3], b: [f32; 3], cx: f32, cy: f32) -> [f32; 3] {
    if mode == 0 {
        return [at[0][0] * b[0] + at[1][0] * b[1] + at[2][0] * b[2], at[0][1] * b[0] + at[1][1] * b[1] + at[2][1] * b[2], at[0][2] * b[0] + at[1][2] * b[1] + at[2][2] * b[2]];
    }
    let (x0, y0) = (p[0][0], p[0][1]);
    let (dx1, dy1, dx2, dy2) = (p[1][0] - x0, p[1][1] - y0, p[2][0] - x0, p[2][1] - y0);
    let area = dx1 * dy2 - dx2 * dy1;
    let mut out = [0f32; 3];
    for ch in 0..3 {
        let (a0, da1, da2) = (at[0][ch], at[1][ch] - at[0][ch], at[2][ch] - at[0][ch]);
        let aa = (da1 * dy2 - da2 * dy1) / area; // ∂a/∂x
        let bb = (da2 * dx1 - da1 * dx2) / area; // ∂a/∂y
        out[ch] = match mode {
            1 => { let c = a0 - aa * x0 - bb * y0; aa * cx + bb * cy + c }
            2 => a0 + aa * (cx - x0) + bb * (cy - y0),
            3 => { let c = a0 - aa * x0 - bb * y0; aa.mul_add(cx, bb.mul_add(cy, c)) }
            _ => { let c = (a0 - aa * x0) - bb * y0; (aa * cx + bb * cy) + c }
        };
    }
    out
}

/// LMTOOL_NO_BLOCK_CULL, read once (the block cull runs per instance per block per direction from every thread).
pub static BLOCK_CULL: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("LMTOOL_NO_BLOCK_CULL").is_none());

// ---------------------------------------------------------------------------------------------------------------
// THE LM RASTER ONCE PER JITTER OFFSET (perf 8): the fragment list
// ---------------------------------------------------------------------------------------------------------------
//
// Every LmILightDir_Set block and every H-basis draw of a direction rasterises the SAME geometry (the LM scene's meshes
// × instances, in draw order) at the direction's raster offset (k mod 9) — the peel layers and the direction only enter
// the PIXEL shaders. On Stadium stpad that raster is 45.8 M triangles (9 216 Grass tiles × 4 948) for a few hundred
// thousand fragments: the per-triangle setup was the whole cost of the fused blocks (16 s per direction), and it was
// paid again for the H-basis draw (8 s). Here the raster runs ONCE per offset for the whole bake: the fragments of every
// pixel, in the serial draw order (draw, instance, triangle), with the rasteriser's own barycentrics and the VS 17111
// outputs interpolated at the pixel (world position and normal — direction-independent); the blocks and the H-basis
// draw then REPLAY the list: per pixel, per block in order, the last fragment passing the block's depth test wins —
// exactly `run_set_layers_par`'s result (the highest block that wrote the pixel, its last fragment); the H-basis draw
// blends every fragment in order into the four MRTs, recomputing VS 17118's direction-dependent `o2` per vertex.
// Nothing in the per-fragment arithmetic changes: the same functions on the same inputs in the same order.

/// One fragment of the LM raster: its pixel (row-major index), the (pair, triangle) it came from, the rasteriser's
/// barycentrics in vertex order, and VS 17111's world position / normal interpolated at the pixel.
#[derive(Clone, Copy, Debug, Default)]
pub struct LmFrag {
    pub px: u32,
    pub pair: u32,
    pub tri: u32,
    pub b: [f32; 3],
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
}

/// The fragment list of one raster offset: per pixel (row-major, `w × h`) the fragments in draw order.
pub struct LmFragList {
    pub w: u32,
    pub h: u32,
    pub offset: usize,
    /// `start[p]..start[p + 1]` = pixel p's fragments; `w · h + 1` entries.
    pub start: Vec<u32>,
    pub frags: Vec<LmFrag>,
    /// The (mesh, instance) pairs in draw order (mesh m's instances `inst_first[m]..+inst_count[m]`), indexed by `LmFrag::pair`.
    pub pairs: Vec<(u32, u32)>,
    /// Per fragment (a bit each): clipped by the scene's fitted world box — the tile peels' SV_ClipDistance test, the same
    /// expression as before, evaluated once here instead of per peel and per layer chunk (perf 8.20). Empty when no box.
    pub clip_mask: Vec<u64>,
    pub clip_box: Option<[[f32; 2]; 2]>,
}

impl LmFragList {
    /// Fragment `i` clipped by the fitted world box (false without a box).
    #[inline(always)]
    pub fn clipped(&self, i: usize) -> bool {
        !self.clip_mask.is_empty() && (self.clip_mask[i >> 6] >> (i & 63)) & 1 != 0
    }
}

impl std::fmt::Debug for LmFragList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LmFragList {{ {}×{}, offset {}, {} fragments over {} pairs }}", self.w, self.h, self.offset, self.frags.len(), self.pairs.len())
    }
}

/// Whether the fragment-list replay is on (default; `LMTOOL_LMACCUM_FRAGLIST=0` keeps the per-direction raster).
pub fn frag_list_on() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_LMACCUM_FRAGLIST").map(|v| v != "0").unwrap_or(true))
}

/// The LM scene's (mesh, instance) pairs in draw order.
pub fn lm_pairs(sc: &LmScene) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(sc.instances.len());
    for m in 0..sc.meshes.len() {
        for ii in sc.inst_first[m]..sc.inst_first[m] + sc.inst_count[m] {
            out.push((m as u32, ii as u32));
        }
    }
    out
}

impl LmScene {
    /// The fragment list of raster offset `offset` (mod 9) for a `w × h` target, built on first use and kept.
    pub fn frag_list(&self, offset: usize, w: u32, h: u32) -> std::sync::Arc<LmFragList> {
        self.frag_lists[offset % 9].get_or_init(|| {
            let t = std::time::Instant::now();
            let fl = build_frag_list(self, offset % 9, w, h);
            eprintln!("lm-accumulate: the LM raster of offset {} built once: {} fragments over {} pixels ({} pairs, {:.2}s)", offset % 9, fl.frags.len(), fl.start.windows(2).filter(|s| s[1] > s[0]).count(), fl.pairs.len(), t.elapsed().as_secs_f32());
            // LMTOOL_LM_TEXEL_TRACE=x0,y0,x1,y1 (diagnostic, port engineer G): every fragment of the atlas rect [x0,x1)×[y0,y1) of this
            // offset — pixel, (mesh, instance), triangle, VS 17111's world position and normal — the anatomy of a small chart
            // LMTOOL_LM_TRI_CENSUS=INST (diagnostic, port engineer G2): for LM instance INST, per raster offset, how many of its mesh's
            // triangles produce at least one fragment, the fragment count and the pixels touched — the sub-texel card question
            // (a 21×20-texel fir holds 18 362 lod1 cards + twins; which of them the raster ever sees)
            if let Some(inst) = std::env::var("LMTOOL_LM_TRI_CENSUS").ok().and_then(|s| s.trim().parse::<u32>().ok()) {
                let pair = fl.pairs.iter().position(|(_, ii)| *ii == inst);
                if let Some(pi) = pair {
                    let (m, _) = fl.pairs[pi];
                    let n_tris = self.meshes[m as usize].indices.len() / 3;
                    let mut seen = vec![false; n_tris];
                    let (mut n_frag, mut n_px) = (0usize, 0usize);
                    for p in 0..(w * h) as usize {
                        let (a, b) = (fl.start[p] as usize, fl.start[p + 1] as usize);
                        let mut any = false;
                        for f in &fl.frags[a..b] { if f.pair as usize == pi { n_frag += 1; any = true; if (f.tri as usize) < n_tris { seen[f.tri as usize] = true; } } }
                        if any { n_px += 1; }
                    }
                    let n_seen = seen.iter().filter(|s| **s).count();
                    eprintln!("lm-tri-census: offset {} instance {inst} (mesh {m}, {n_tris} LM triangles): {n_seen} triangles with a fragment ({:.1} %), {n_frag} fragments over {n_px} raster pixels", offset % 9, 100.0 * n_seen as f64 / n_tris.max(1) as f64);
                } else {
                    eprintln!("lm-tri-census: offset {}: instance {inst} is not in the LM scene", offset % 9);
                }
            }
            if let Some(r) = std::env::var("LMTOOL_LM_TEXEL_TRACE").ok().and_then(|s| { let v: Vec<u32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 4 { Some(v) } else { None } }) {
                for y in r[1]..r[3].min(h) {
                    for x in r[0]..r[2].min(w) {
                        let p = (y * w + x) as usize;
                        for i in fl.start[p] as usize..fl.start[p + 1] as usize {
                            let f = &fl.frags[i];
                            let (m, ii) = fl.pairs[f.pair as usize];
                            eprintln!("lm-texel-trace: offset {} px ({x},{y}) mesh {m} inst {ii} tri {} bary ({:.3},{:.3},{:.3}) pos ({:.3},{:.3},{:.3}) n ({:.3},{:.3},{:.3})", offset % 9, f.tri, f.b[0], f.b[1], f.b[2], f.pos[0], f.pos[1], f.pos[2], f.nrm[0], f.nrm[1], f.nrm[2]);
                        }
                        if fl.start[p] == fl.start[p + 1] { eprintln!("lm-texel-trace: offset {} px ({x},{y}) NO fragment", offset % 9); }
                    }
                }
            }
            std::sync::Arc::new(fl)
        }).clone()
    }
}

/// The LM raster of every (draw, instance) pair at raster offset `offset`: the fragments per pixel in draw order.
pub fn build_frag_list(sc: &LmScene, offset: usize, w: u32, h: u32) -> LmFragList {
    let pairs = lm_pairs(sc);
    let cb = LmRasterCb::for_offset(offset, w, h);
    let threads = crate::pool::pool().threads.max(1);
    // the pixel-row bands the per-pair lists are split into (the sort by pixel runs per band, in parallel)
    let n_bands = (threads * 2).clamp(1, h as usize);
    let rows = (h as usize + n_bands - 1) / n_bands;
    let band_of = |px: u32| (px as usize / w as usize) / rows;
    // 1. every pair rasterised on its own (parallel over the pairs): its fragments in (triangle, scan) order, then
    //    stably bucketed by band — `frags` sorted by band, `off[b]..off[b + 1]` the band's range
    struct PairFrags {
        frags: Vec<LmFrag>,
        off: Vec<u32>,
    }
    crate::pool::stats::stage("lm-fraglist");
    // (perf 8: the pairs in PIECES of at most 512 triangles — one task per pair left the run 12 % busy behind the few large
    // meshes (the slowest task 248× the mean); a pair's pieces concatenated in order are its fragments in triangle order,
    // as one task produced them)
    const PIECE_TRIS: usize = 512;
    let pieces: Vec<(u32, u32, u32)> = pairs.iter().enumerate().flat_map(|(k, &(m, _))| {
        let n_tris = sc.meshes[m as usize].indices.len() / 3;
        (0..n_tris.max(1)).step_by(PIECE_TRIS).map(move |t0| (k as u32, t0 as u32, ((t0 + PIECE_TRIS).min(n_tris)) as u32))
    }).collect();
    let per_piece: Vec<Vec<LmFrag>> = crate::pool::pool().map(pieces.len(), |pi| {
        let (k, t0, t1) = pieces[pi];
        let k = k as usize;
        let (m, ii) = pairs[k];
        let mesh = &sc.meshes[m as usize];
        let inst = &sc.instances[ii as usize];
        let rows_q = rotation_rows(inst.q);
        let mut out: Vec<LmFrag> = Vec::new();
        // the clip position of every vertex (VS 17111 / 17118 instructions 0–10: the same for both shaders)
        let clip: Vec<[f32; 2]> = mesh.verts.iter().map(|v| lm_clip(v, chart_st(v, inst, &sc.table), &cb)).collect();
        for (t, tri) in mesh.indices.chunks_exact(3).enumerate().skip(t0 as usize).take((t1 - t0) as usize) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            // the VS outputs of the three vertices, computed on the triangle's first fragment
            let mut vs: Option<[SetVsOut; 3]> = None;
            rasterise_triangle([clip[i0], clip[i1], clip[i2]], w, h, |x, y, b0, b1, b2| {
                let [a, bb, c] = vs.get_or_insert_with(|| {
                    let f = |i: usize| { let v = &mesh.verts[i]; SetVsOut { clip: clip[i], pos: world_pos(v, inst, &rows_q), nrm: rotate(v.normal, &rows_q) } };
                    [f(i0), f(i1), f(i2)]
                });
                let pos = [a.pos[0] * b0 + bb.pos[0] * b1 + c.pos[0] * b2, a.pos[1] * b0 + bb.pos[1] * b1 + c.pos[1] * b2, a.pos[2] * b0 + bb.pos[2] * b1 + c.pos[2] * b2];
                let n = [a.nrm[0] * b0 + bb.nrm[0] * b1 + c.nrm[0] * b2, a.nrm[1] * b0 + bb.nrm[1] * b1 + c.nrm[1] * b2, a.nrm[2] * b0 + bb.nrm[2] * b1 + c.nrm[2] * b2];
                out.push(LmFrag { px: y * w + x, pair: k as u32, tri: t as u32, b: [b0, b1, b2], pos, nrm: n });
            });
        }
        out
    });
    // the pieces of each pair, in order (the first piece's index per pair), then the pair's band bucketing
    let mut first_piece = vec![0usize; pairs.len() + 1];
    for (pi, &(k, _, _)) in pieces.iter().enumerate() { first_piece[k as usize + 1] = pi + 1; }
    for k in 0..pairs.len() { if first_piece[k + 1] == 0 { first_piece[k + 1] = first_piece[k]; } }
    let per_pair: Vec<PairFrags> = crate::pool::pool().map(pairs.len(), |k| {
        let ps = &per_piece[first_piece[k]..first_piece[k + 1]];
        let n_out: usize = ps.iter().map(|v| v.len()).sum();
        let mut out: Vec<LmFrag> = Vec::with_capacity(n_out);
        for v in ps { out.extend_from_slice(v); }
        // the stable bucket sort by band
        let mut counts = vec![0u32; n_bands + 1];
        for f in &out { counts[band_of(f.px) + 1] += 1; }
        for b in 0..n_bands { counts[b + 1] += counts[b]; }
        let off = counts.clone();
        let mut sorted: Vec<LmFrag> = vec![LmFrag::default(); out.len()];
        for f in &out {
            let b = band_of(f.px);
            sorted[counts[b] as usize] = *f;
            counts[b] += 1;
        }
        PairFrags { frags: sorted, off }
    });
    // 2. per band (parallel): the pairs' band ranges concatenated in pair order = the band's fragments in draw order,
    //    then a stable counting sort by pixel → the band's CSR
    struct BandOut {
        start: Vec<u32>,
        frags: Vec<LmFrag>,
    }
    let per_band: Vec<BandOut> = crate::pool::pool().map(n_bands, |b| {
        // (the last bands may lie beyond the target: empty)
        let (y0, y1) = ((b * rows).min(h as usize), ((b + 1) * rows).min(h as usize));
        let p0 = y0 * w as usize;
        let n_px = (y1 - y0) * w as usize;
        let total: usize = per_pair.iter().map(|pf| (pf.off[b + 1] - pf.off[b]) as usize).sum();
        let mut counts = vec![0u32; n_px + 1];
        for pf in &per_pair {
            for f in &pf.frags[pf.off[b] as usize..pf.off[b + 1] as usize] { counts[f.px as usize - p0 + 1] += 1; }
        }
        for i in 0..n_px { counts[i + 1] += counts[i]; }
        let start = counts.clone();
        let mut frags: Vec<LmFrag> = vec![LmFrag::default(); total];
        for pf in &per_pair {
            for f in &pf.frags[pf.off[b] as usize..pf.off[b + 1] as usize] {
                let p = f.px as usize - p0;
                frags[counts[p] as usize] = *f;
                counts[p] += 1;
            }
        }
        BandOut { start, frags }
    });
    drop(per_pair);
    // 3. the bands concatenated (their pixel ranges are consecutive)
    let total: usize = per_band.iter().map(|b| b.frags.len()).sum();
    let mut start: Vec<u32> = Vec::with_capacity((w * h) as usize + 1);
    let mut frags: Vec<LmFrag> = Vec::with_capacity(total);
    for bo in per_band {
        let base = frags.len() as u32;
        // (the band's `start` has n_px + 1 entries; its last equals the next band's first)
        start.extend(bo.start[..bo.start.len() - 1].iter().map(|s| s + base));
        frags.extend_from_slice(&bo.frags);
    }
    start.push(frags.len() as u32);
    debug_assert_eq!(start.len(), (w * h) as usize + 1);
    // the tile peels' clip decision per fragment, once (the box is the bake's)
    let (clip_mask, clip_box) = match sc.fitted_world_box {
        Some(wb) => {
            let n = frags.len();
            let mut mask = vec![0u64; (n + 63) / 64];
            let mp = mask.as_mut_ptr() as usize;
            let words = mask.len();
            let per_w = (words / (threads * 4).max(1)).max(64);
            let frags_r = &frags;
            let pairs_r = &pairs;
            crate::pool::pool().run((words + per_w - 1) / per_w, |ci| {
                for wi in ci * per_w..((ci + 1) * per_w).min(words) {
                    let mut word = 0u64;
                    for bit in 0..64 {
                        let i = wi * 64 + bit;
                        if i >= n { break; }
                        let f = &frags_r[i];
                        let (m, ii) = pairs_r[f.pair as usize];
                        let mesh = &sc.meshes[m as usize];
                        let inst = &sc.instances[ii as usize];
                        let rows = rotation_rows(inst.q);
                        let tri = &mesh.indices[f.tri as usize * 3..f.tri as usize * 3 + 3];
                        let cd = [clip_distances(world_pos(&mesh.verts[tri[0] as usize], inst, &rows), &wb), clip_distances(world_pos(&mesh.verts[tri[1] as usize], inst, &rows), &wb), clip_distances(world_pos(&mesh.verts[tri[2] as usize], inst, &rows), &wb)];
                        if (0..4).any(|k| cd[0][k] * f.b[0] + cd[1][k] * f.b[1] + cd[2][k] * f.b[2] < 0.0) { word |= 1u64 << bit; }
                    }
                    // SAFETY: the chunks own disjoint words
                    unsafe { *(mp as *mut u64).add(wi) = word; }
                }
            });
            (mask, Some(wb))
        }
        None => (Vec::new(), None),
    };
    LmFragList { w, h, offset, start, frags, pairs, clip_mask, clip_box }
}

/// PS 17112 split for the replay: the layer-independent part of a fragment — `None` when the texel faces away from
/// the peel direction (`n·PeelDirInW < 0`), else its projected depth `z` and the point-sampled texel of the depth
/// compare `(u, v)` and of the colour lookup `(u, v)/w`. The same instructions as `ps_17112`, evaluated once per
/// fragment instead of once per (fragment, layer).
#[inline]
fn ps_17112_project(p: [f32; 3], n: [f32; 3], cb: &SetCb) -> Option<(f32, f32, f32, f32, f32)> {
    if dot(n, cb.peel_dir) < 0.0 {
        return None;
    }
    let m = &cb.world_pw01_shadow;
    let z = p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2] + m[3][2];
    let u = p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0] + m[3][0];
    let v = p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1] + m[3][1];
    let w = p[0] * m[0][3] + p[1] * m[1][3] + p[2] * m[2][3] + m[3][3];
    Some((z, u, v, u / w, v / w))
}

/// PS 17112's layer-dependent tail: the depth compare at the undivided (u, v), then the colour at (u, v)/w — instructions
/// 10–18 of `ps_17112`, bit for bit.
#[inline]
fn ps_17112_layer<L: LayerRead>((z, u, v, cu, cv): (f32, f32, f32, f32, f32), layer: &L, cmp: DepthCompare) -> Option<[f32; 3]> {
    let (dw, dh) = layer.depth_size();
    let (tx, ty) = (point_texel(u, dw), point_texel(v, dh));
    let stored = layer.depth(tx, ty);
    let pass = match cmp {
        DepthCompare::Unorm16Round => {
            let rq = (z.clamp(0.0, 1.0) * 65535.0).round();
            let sq = (stored * 65535.0).round();
            rq >= sq
        }
        DepthCompare::Float => z >= stored,
    };
    if !pass {
        return None;
    }
    let (cw, ch) = layer.color_size();
    let (cx, cy) = (point_texel(cu, cw), point_texel(cv, ch));
    let mut c = layer.rgb(cx, cy);
    for k in 0..3 {
        let v = c[k].max(0.0).min(99999996802856930000000000000000000000.0);
        c[k] = if v < 49999998401428460000000000000000000000.0 { v } else { 0.0 };
    }
    Some(c)
}

/// The pixel chunks of a replay: `w × h` pixels in `n` consecutive ranges.
fn pixel_chunks(n_px: usize, threads: usize) -> (usize, usize) {
    let n = (threads * 8).max(1);
    let per = (n_px + n - 1) / n;
    ((n_px + per - 1) / per, per)
}

/// LmILightDir_Set over the fragment list: the blocks `layers[j]` (in order) of one peel — `cb` = the peel's constants
/// (`WorldPw01Shadow`, `PeelDirInW`), `world_box` = the fitted peel's clip box (VS 17115; None for the world peel).
/// Per pixel, per block in order, the last fragment passing the block's depth test writes the target — the sequential
/// blocks' result (`run_set_layers_par` called once per block), without the raster. The clip distances of a fitted
/// block are VS 17115's per-vertex `(x − MinX, z − MinZ, MaxX − x, MaxZ − z)` interpolated with the fragment's
/// barycentrics, as the raster path evaluates them.
/// `replay_set_layers` over the in-place layer table: the layers are `LayerSparse` views of ONE table (the same `start` /
/// `cnt` / `px` / `frags`, k = 0, 1, 2 …), so a fragment's depth texel — and its colour texel, the same one on the ortho
/// peel (w = 1) — resolves to a (first, count) range ONCE and every layer k reads `frags[first + k]` from it (perf 8.23:
/// the trait path re-resolved the pixel per layer — up to 22 rank lookups per fragment, 230 M random loads per tiny
/// direction). The walk keeps the layer-major, fragment-minor order and every comparison of `ps_17112_layer`, so the last
/// passing (layer, fragment) — the written colour — is the same. Returns false when the views are not one table (the
/// caller takes the general path).
pub fn replay_set_layers_sparse(fl: &LmFragList, sc: &LmScene, cb: &SetCb, world_box: Option<[[f32; 2]; 2]>, layers: &[LayerSparse<'_>], cmp: DepthCompare, tgt: &mut DirTarget) -> bool {
    assert_eq!((fl.w, fl.h), (tgt.w, tgt.h));
    let tile_no_sky = world_box.is_some() && !*crate::peel::TILE_SKY;
    let Some(l0) = layers.first() else { return true };
    // one table: the same slices and index, the layers 0, 1, 2 … in order
    let one_table = layers.iter().enumerate().all(|(k, l)| {
        l.k == k && l.w == l0.w && l.h == l0.h && std::ptr::eq(l.start, l0.start) && std::ptr::eq(l.frags, l0.frags)
            && match (l.cnt, l0.cnt) { (Some(a), Some(b)) => std::ptr::eq(a, b), (None, None) => true, _ => false }
            && match (l.px, l0.px) { (Some(a), Some(b)) => std::ptr::eq(a, b), (None, None) => true, _ => false }
    });
    if !one_table { return false; }
    let nl = layers.len();
    let (dw, dh) = l0.depth_size();
    let threads = crate::pool::pool().threads.max(1);
    let n_px = (fl.w * fl.h) as usize;
    let (n_chunks, per) = pixel_chunks(n_px, threads);
    let px_ptr = tgt.px.as_mut_ptr() as usize;
    crate::pool::pool().run(n_chunks, |ci| {
        let (p0, p1) = (ci * per, ((ci + 1) * per).min(n_px));
        if fl.start[p0] == fl.start[p1] { return; }
        // per fragment of the chunk: the projection (None = facing away or clipped) with its depth texel's range and its
        // colour texel's range (the same one when the texels coincide)
        struct Pf { z: f32, drange: Option<(usize, usize)>, crange: Option<(usize, usize)>, px: (u32, u32), cpx: (u32, u32) }
        let t_proj = std::time::Instant::now();
        let mut proj: Vec<Option<Pf>> = Vec::with_capacity((fl.start[p1] - fl.start[p0]) as usize);
        for (k, f) in fl.frags[fl.start[p0] as usize..fl.start[p1] as usize].iter().enumerate() {
            let fi = fl.start[p0] as usize + k;
            let clipped = match &world_box {
                Some(wb) if fl.clip_box.as_ref() == Some(wb) => fl.clipped(fi),
                Some(wb) => {
                    let (m, ii) = fl.pairs[f.pair as usize];
                    let mesh = &sc.meshes[m as usize];
                    let inst = &sc.instances[ii as usize];
                    let rows = rotation_rows(inst.q);
                    let tri = &mesh.indices[f.tri as usize * 3..f.tri as usize * 3 + 3];
                    let cd = [clip_distances(world_pos(&mesh.verts[tri[0] as usize], inst, &rows), wb), clip_distances(world_pos(&mesh.verts[tri[1] as usize], inst, &rows), wb), clip_distances(world_pos(&mesh.verts[tri[2] as usize], inst, &rows), wb)];
                    (0..4).any(|i| cd[0][i] * f.b[0] + cd[1][i] * f.b[1] + cd[2][i] * f.b[2] < 0.0)
                }
                None => false,
            };
            let pr = if clipped { None } else { ps_17112_project(f.pos, f.nrm, cb) };
            proj.push(pr.map(|(z, u, v, cu, cv)| {
                let (tx, ty) = (point_texel(u, dw), point_texel(v, dh));
                let (cx, cy) = (point_texel(cu, dw), point_texel(cv, dh));
                let drange = l0.range(tx, ty);
                let crange = if (cx, cy) == (tx, ty) { drange } else { l0.range(cx, cy) };
                Pf { z, drange, crange, px: (tx, ty), cpx: (cx, cy) }
            }));
        }
        SET_PROJ_NS.fetch_add(t_proj.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        let t_walk = std::time::Instant::now();
        let base = fl.start[p0] as usize;
        // LMTOOL_SET_TEXEL_TRACE=x,y (diagnostic, E2 2026-09-28): the SET's whole walk at ONE pixel, per direction and peel — every
        // fragment's projection (z, depth texel, layers there), each layer's stored depth and the compare's verdict, the colour taken,
        // the pixel's final L — the record cell's question (pwc-day (676, 84): which of the 58 covering directions read what)
        let trace_px: Option<usize> = set_texel_trace().filter(|&(x, y)| x < fl.w && y < fl.h).map(|(x, y)| (y * fl.w + x) as usize);
        for p in p0..p1 {
            let (a, b) = (fl.start[p] as usize, fl.start[p + 1] as usize);
            if a == b { continue; }
            let tracing = trace_px == Some(p);
            let mut out: Option<[f32; 3]> = None;
            // THE LAYERS WORTH WALKING (perf 8.24): past the deepest layer any of the pixel's fragments can read, every read is
            // the clear (depth 1.0) and the compare `z ≥ 1.0` fails for every fragment with z < 1 — so when all of them have
            // z < 1 the walk stops at that layer; a fragment at z ≥ 1 keeps the full walk (it would pass on the clear and write
            // black, as before)
            let frs = &proj[a - base..b - base];
            let mut kmax = 0usize;
            let mut all_below = true;
            for pf in frs.iter().flatten() {
                if let Some((_, n)) = pf.drange { kmax = kmax.max(n); }
                if !(pf.z < 1.0) { all_below = false; }
            }
            let k_end = if all_below { kmax.min(nl) } else { nl };
            if tracing {
                let mut s = format!("set-texel-trace: dir {} ({:.5},{:.5},{:.5}) {} peel, {} layers (walk {k_end}), {} fragments:", SET_TRACE_DIR.load(std::sync::atomic::Ordering::Relaxed), cb.peel_dir[0], cb.peel_dir[1], cb.peel_dir[2], if world_box.is_some() { "FITTED" } else { "world" }, nl, b - a);
                for (fi, f) in fl.frags[a..b].iter().enumerate() {
                    let (m, ii) = fl.pairs[f.pair as usize];
                    match &proj[a - base + fi] {
                        None => s += &format!("\n    frag {fi}: mesh {m} inst {ii} tri {} pos ({:.4},{:.4},{:.4}) n ({:.3},{:.3},{:.3}) → facing away / clipped", f.tri, f.pos[0], f.pos[1], f.pos[2], f.nrm[0], f.nrm[1], f.nrm[2]),
                        Some(pf) => {
                            let nd = pf.drange.map(|(_, n)| n).unwrap_or(0);
                            s += &format!("\n    frag {fi}: mesh {m} inst {ii} tri {} pos ({:.4},{:.4},{:.4}) n ({:.3},{:.3},{:.3}) → z {:.7}, depth texel ({},{}) colour texel ({},{}) has {nd} layers:", f.tri, f.pos[0], f.pos[1], f.pos[2], f.nrm[0], f.nrm[1], f.nrm[2], pf.z, pf.px.0, pf.px.1, pf.cpx.0, pf.cpx.1);
                            for k in 0..k_end {
                                let stored = match pf.drange { Some((f0, n)) if k < n => l0.frags[f0 + k].d, _ => 1.0 };
                                let pass = match cmp { DepthCompare::Unorm16Round => (pf.z.clamp(0.0, 1.0) * 65535.0).round() >= (stored * 65535.0).round(), DepthCompare::Float => pf.z >= stored };
                                let c = match pf.crange { Some((f0, n)) if k < n => l0.frags[f0 + k].rgb, _ => [0.0; 3] };
                                s += &format!(" [L{k} d {:.7} {} rgb ({:.4},{:.4},{:.4})]", stored, if pass { "PASS" } else { "fail" }, c[0], c[1], c[2]);
                            }
                        }
                    }
                }
                eprintln!("{s}");
            }
            for k in 0..k_end {
                // LMTOOL_TILE_SKY=0 (study, F 2026-09-27): a tile pass (world_box set) leaves the dome layer (layer 0) out — a texel whose
                // every surface layer fails keeps the world pass's value instead of the tile's sky
                if tile_no_sky && l0.k + k == 0 { continue; }
                for pf in frs.iter().flatten() {
                    // the layer's stored depth at the depth texel: the fragment when the pixel has layer k, else the clear (1.0)
                    let stored = match pf.drange { Some((f0, n)) if k < n => l0.frags[f0 + k].d, _ => 1.0 };
                    let pass = match cmp {
                        DepthCompare::Unorm16Round => {
                            let rq = (pf.z.clamp(0.0, 1.0) * 65535.0).round();
                            let sq = (stored * 65535.0).round();
                            rq >= sq
                        }
                        DepthCompare::Float => pf.z >= stored,
                    };
                    if !pass { continue; }
                    let mut c = match pf.crange { Some((f0, n)) if k < n => l0.frags[f0 + k].rgb, _ => [0.0; 3] };
                    for ch in 0..3 {
                        let v = c[ch].max(0.0).min(99999996802856930000000000000000000000.0);
                        c[ch] = if v < 49999998401428460000000000000000000000.0 { v } else { 0.0 };
                    }
                    out = Some(c);
                }
            }
            if let Some(rgb) = out {
                // SAFETY: the chunks own disjoint pixel ranges
                unsafe { *(px_ptr as *mut u32).add(p) = crate::gpufmt::pack_r11g11b10(rgb, Rounding::Truncate); }
            }
            if tracing { eprintln!("set-texel-trace: dir {} ({:.5},{:.5},{:.5}) {} peel → L {:?}", SET_TRACE_DIR.load(std::sync::atomic::Ordering::Relaxed), cb.peel_dir[0], cb.peel_dir[1], cb.peel_dir[2], if world_box.is_some() { "FITTED" } else { "world" }, out); }
        }
        SET_WALK_NS.fetch_add(t_walk.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    });
    if lmaccum_trace() {
        eprintln!("lmaccum trace: replay_set_layers_sparse {} fragments, {} layers: projection {:.1} ms, walk {:.1} ms of task time", fl.frags.len(), nl, SET_PROJ_NS.swap(0, std::sync::atomic::Ordering::Relaxed) as f64 * 1e-6, SET_WALK_NS.swap(0, std::sync::atomic::Ordering::Relaxed) as f64 * 1e-6);
    }
    true
}

static SET_PROJ_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SET_WALK_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The sweep direction index the SET trace labels its lines with (set by the caller per direction when the trace is on).
pub static SET_TRACE_DIR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);

/// LMTOOL_SET_TEXEL_TRACE=x,y, read once (the environment lock is not for the per-pixel path).
pub fn set_texel_trace() -> Option<(u32, u32)> {
    static V: std::sync::OnceLock<Option<(u32, u32)>> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_SET_TEXEL_TRACE").ok().and_then(|s| { let v: Vec<u32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 2 { Some((v[0], v[1])) } else { None } }))
}

pub fn replay_set_layers<L: LayerRead>(fl: &LmFragList, sc: &LmScene, cb: &SetCb, world_box: Option<[[f32; 2]; 2]>, layers: &[L], cmp: DepthCompare, tgt: &mut DirTarget) {
    assert_eq!((fl.w, fl.h), (tgt.w, tgt.h));
    if layers.is_empty() { return; }
    let threads = crate::pool::pool().threads.max(1);
    let n_px = (fl.w * fl.h) as usize;
    let (n_chunks, per) = pixel_chunks(n_px, threads);
    let px_ptr = tgt.px.as_mut_ptr() as usize;
    crate::pool::pool().run(n_chunks, |ci| {
        let (p0, p1) = (ci * per, ((ci + 1) * per).min(n_px));
        if fl.start[p0] == fl.start[p1] { return; }
        // per fragment of the chunk: the layer-independent projection (None = facing away or clipped)
        let mut proj: Vec<Option<(f32, f32, f32, f32, f32)>> = Vec::with_capacity((fl.start[p1] - fl.start[p0]) as usize);
        for (k, f) in fl.frags[fl.start[p0] as usize..fl.start[p1] as usize].iter().enumerate() {
            let fi = fl.start[p0] as usize + k;
            let clipped = match &world_box {
                // the list's own clip decision when it was built for this box (perf 8.20), else the test here
                Some(wb) if fl.clip_box.as_ref() == Some(wb) => fl.clipped(fi),
                Some(wb) => {
                    let (m, ii) = fl.pairs[f.pair as usize];
                    let mesh = &sc.meshes[m as usize];
                    let inst = &sc.instances[ii as usize];
                    let rows = rotation_rows(inst.q);
                    let tri = &mesh.indices[f.tri as usize * 3..f.tri as usize * 3 + 3];
                    let cd = [clip_distances(world_pos(&mesh.verts[tri[0] as usize], inst, &rows), wb), clip_distances(world_pos(&mesh.verts[tri[1] as usize], inst, &rows), wb), clip_distances(world_pos(&mesh.verts[tri[2] as usize], inst, &rows), wb)];
                    (0..4).any(|i| cd[0][i] * f.b[0] + cd[1][i] * f.b[1] + cd[2][i] * f.b[2] < 0.0)
                }
                None => false,
            };
            proj.push(if clipped { None } else { ps_17112_project(f.pos, f.nrm, cb) });
        }
        let base = fl.start[p0] as usize;
        for p in p0..p1 {
            let (a, b) = (fl.start[p] as usize, fl.start[p + 1] as usize);
            if a == b { continue; }
            let mut out: Option<[f32; 3]> = None;
            for layer in layers {
                for pr in proj[a - base..b - base].iter().flatten() {
                    if let Some(rgb) = ps_17112_layer(*pr, layer, cmp) {
                        out = Some(rgb);
                    }
                }
            }
            if let Some(rgb) = out {
                // SAFETY: the chunks own disjoint pixel ranges
                unsafe { *(px_ptr as *mut u32).add(p) = crate::gpufmt::pack_r11g11b10(rgb, Rounding::Truncate); }
            }
        }
    });
}

/// The H-basis draw over the fragment list: every fragment of every pixel, in draw order, blended into the four MRTs
/// (`run_hbasis_par` with the sequential fragment model and the barycentric interpolation, without the raster).
/// VS 17118's `o2` / `o3` are recomputed per vertex for this direction (`vs_17118_o23`); the provoking vertex is the
/// triangle's first index. `owner[i]` (when given) receives the mesh index + 1 of the pixel's last fragment.
/// LMTOOL_LMACCUM_TRACE=1: the replays' wall times per call.
pub fn lmaccum_trace() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("LMTOOL_LMACCUM_TRACE").is_some())
}

static HB_TASK_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn replay_hbasis(fl: &LmFragList, sc: &LmScene, cb: &HbCb, ilightdir: &DirTarget, tgt: &mut HbTargets, blend: crate::sunpass::BlendModel, owner: Option<&mut Vec<u8>>) {
    assert_eq!((fl.w, fl.h), (tgt.w, tgt.h));
    let t_trace = std::time::Instant::now();
    let threads = crate::pool::pool().threads.max(1);
    let n_px = (fl.w * fl.h) as usize;
    let (n_chunks, per) = pixel_chunks(n_px, threads);
    let mrt_ptrs: [usize; 4] = [tgt.mrt[0].as_mut_ptr() as usize, tgt.mrt[1].as_mut_ptr() as usize, tgt.mrt[2].as_mut_ptr() as usize, tgt.mrt[3].as_mut_ptr() as usize];
    let owner_p: Option<usize> = owner.as_ref().map(|o| o.as_ptr() as usize);
    let _ = &owner;
    crate::pool::pool().run(n_chunks, |ci| {
        let (p0, p1) = (ci * per, ((ci + 1) * per).min(n_px));
        if fl.start[p0] == fl.start[p1] { return; }
        let t_task = std::time::Instant::now();
        for p in p0..p1 {
            let (a, b) = (fl.start[p] as usize, fl.start[p + 1] as usize);
            if a == b { continue; }
            let (x, y) = ((p % fl.w as usize) as u32, (p / fl.w as usize) as u32);
            let l = ilightdir.rgb(x, y);
            for f in &fl.frags[a..b] {
                let (m, ii) = fl.pairs[f.pair as usize];
                let mesh = &sc.meshes[m as usize];
                let inst = &sc.instances[ii as usize];
                let rows = rotation_rows(inst.q);
                let tri = &mesh.indices[f.tri as usize * 3..f.tri as usize * 3 + 3];
                let (o2a, o3a) = vs_17118_o23(&mesh.verts[tri[0] as usize], &rows, cb.peel_dir);
                let (o2b, _) = vs_17118_o23(&mesh.verts[tri[1] as usize], &rows, cb.peel_dir);
                let (o2c, _) = vs_17118_o23(&mesh.verts[tri[2] as usize], &rows, cb.peel_dir);
                let [b0, b1, b2] = f.b;
                let v2 = [o2a[0] * b0 + o2b[0] * b1 + o2c[0] * b2, o2a[1] * b0 + o2b[1] * b1 + o2c[1] * b2, o2a[2] * b0 + o2b[2] * b1 + o2c[2] * b2];
                let o = ps_17122(v2, o3a, l, cb);
                if set_texel_trace() == Some((x, y)) { eprintln!("hb-texel-trace: dir {} D ({:.5},{:.5},{:.5}) frag mesh {m} inst {ii} tri {} L ({:.5},{:.5},{:.5}) o2 ({:.4},{:.4},{:.4}) o3 ({:.3},{:.3},{:.3},{:.3}) → C0 += ({:+.6},{:+.6},{:+.6}) a += {:.6}", SET_TRACE_DIR.load(std::sync::atomic::Ordering::Relaxed), cb.peel_dir[0], cb.peel_dir[1], cb.peel_dir[2], f.tri, l[0], l[1], l[2], v2[0], v2[1], v2[2], o3a[0], o3a[1], o3a[2], o3a[3], o[0][0], o[0][1], o[0][2], o[0][3]); }
                // SAFETY: the chunks own disjoint pixel ranges
                unsafe {
                    if let Some(op) = owner_p { *(op as *mut u8).add(p) = m as u8 + 1; }
                    for k in 0..4 {
                        let slot = &mut *(mrt_ptrs[k] as *mut [f32; 4]).add(p);
                        for ch in 0..4 { slot[ch] = blend_f16(slot[ch], o[k][ch], blend); }
                    }
                    if set_texel_trace() == Some((x, y)) { let s0 = &*(mrt_ptrs[0] as *const [f32; 4]).add(p); eprintln!("hb-texel-trace: dir {} MRT0 after this fragment = ({:.7},{:.7},{:.7},{:.7}) [blend {:?}]", SET_TRACE_DIR.load(std::sync::atomic::Ordering::Relaxed), s0[0], s0[1], s0[2], s0[3], blend); }
                }
            }
        }
        HB_TASK_NS.fetch_add(t_task.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    });
    if lmaccum_trace() {
        // the per-task busy sum beside the wall: the difference is the dispatch / the first touch of the targets
        eprintln!("lmaccum trace: replay_hbasis {} fragments, {} chunks: {:.1} ms wall, {:.1} ms of task time", fl.frags.len(), n_chunks, t_trace.elapsed().as_secs_f64() * 1e3, HB_TASK_NS.swap(0, std::sync::atomic::Ordering::Relaxed) as f64 / 1e6);
    }
}
