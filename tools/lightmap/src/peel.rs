//! The dome peel as the game rasterises it (§2.3 of the RE doc, `RenderLightIndirectDome`): for every
//! direction `D` of the sweep's sphere set the whole scene is rendered ORTHOGRAPHICALLY along `D`
//! (`PeelZDiffuse_p`), each fragment carrying the surface's current outgoing radiance — the
//! lightmap-so-far sampled in the surface's own lightmap UV (÷ BounceFactor on the bounce sweeps, 0 on
//! the first) plus the sun on that surface, times its material albedo (MDiffuse); FRONT faces only,
//! back faces are black. Then for every lightmap texel facing `D` (`n·D ≥ 0`) the deepest peel layer
//! still in front of the texel gives the incoming radiance from `D` (`LmILightDir_Set_p`); texels no
//! layer covers see the sky (the target is cleared to the sky's radiance in `D`). The accumulation is
//! `E += Scale·max(0, n·D)·L` with `Scale = 4/N` over the sweep's `N` directions
//! (`LmLBumpILighting_Inst_p`); with the atlas rendered at `ss` sub-samples per axis the directions are
//! interleaved into `ss²` groups, one per sub-sample, and the resolve box-averages the sub-samples.
//!
//! Software form: an A-buffer per direction (every fragment of every triangle at the peel's pixel
//! centres, sorted by depth per pixel — depth peeling without a layer cap), built in parallel over
//! triangle chunks; the gather runs in parallel over the sub-samples of the direction's group.

use crate::bake::{hit_albedo, sky_radiance, BakeParams, ChartBake};
use crate::bvh::{Bvh, Hit, WTri};
use crate::geometry::{add, cross, dot, norm, sub, Scene, V3, DECOR_INST};
use crate::raster;

/// The peel's orthographic frame: pixel (x, y) ↔ (p·r, p·u) scaled into `res`×`res_y` pixels over
/// the scene's projected bounds; the port's depth = −p·d (the camera sits at +d∞ looking along −d,
/// nearer = smaller). The GAME's camera looks the other way (along +D, `PeelDirInW`, from the
/// receivers' side towards the sky) with a reversed depth `z01 = 0.5 + (c·D − p·D)/(2·halfD)`
/// (1 = its near plane, 0 = far); `z01()` converts, `frustum()` records the frame in those terms and
/// `from_frustum()` builds a frame from a captured one so both sides rasterise the same pixel grid.
#[derive(Clone, Debug)]
pub struct PeelFrame {
    pub d: V3,
    pub r: V3,
    pub u: V3,
    pub s0: f32,
    pub t0: f32,
    /// Pixels per metre along r (x) and u (y).
    pub scale: f32,
    pub scale_y: f32,
    pub res: u32,
    pub res_y: u32,
    /// The frustum centre's `c·d` and its depth half extent: `z01 = 0.5 + (zc + z_port)/(2·half_d)`.
    pub zc: f32,
    pub half_d: f32,
    /// The render viewport's inset in pixels: the game's peel draws use (1, 1, 4094, 4094) on the 4096²
    /// target, so its outer ring is never written (a captured frame sets 1; the port's own frames 0).
    pub inset_px: u32,
}

impl PeelFrame {
    pub fn new(d: V3, bmin: V3, bmax: V3, res: u32) -> PeelFrame {
        let d = norm(d);
        let helper = if d[1].abs() < 0.99 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let r = norm(cross(helper, d));
        let u = norm(cross(d, r));
        let (mut smin, mut smax, mut tmin, mut tmax, mut zmin, mut zmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for i in 0..8 {
            let p = [if i & 1 == 0 { bmin[0] } else { bmax[0] }, if i & 2 == 0 { bmin[1] } else { bmax[1] }, if i & 4 == 0 { bmin[2] } else { bmax[2] }];
            let (s, t, z) = (dot(p, r), dot(p, u), -dot(p, d));
            smin = smin.min(s);
            smax = smax.max(s);
            tmin = tmin.min(t);
            tmax = tmax.max(t);
            zmin = zmin.min(z);
            zmax = zmax.max(z);
        }
        let extent = (smax - smin).max(tmax - tmin).max(1e-3);
        // a half-pixel margin so the bounds' own points land inside
        let scale = (res as f32 - 1.0) / extent;
        let half_d = (0.5 * (zmax - zmin)).max(1e-3);
        PeelFrame { d, r, u, s0: smin - 0.5 / scale, t0: tmin - 0.5 / scale, scale, scale_y: scale, res, res_y: res, zc: -0.5 * (zmin + zmax), half_d, inset_px: 0 }
    }
    /// A frame that rasterises exactly the captured frustum's pixel grid (`w`×`h` pixels): the game's
    /// pixel x grows along `right`, pixel y along −`up`, its camera looks along `forward` = the port's d.
    pub fn from_frustum(f: &crate::passdump::Frustum, w: u32, h: u32) -> PeelFrame {
        let d = norm(f.forward);
        let r = norm(f.right);
        let u = norm([-f.up[0], -f.up[1], -f.up[2]]);
        let scale = w as f32 / (2.0 * f.half[0]);
        let scale_y = h as f32 / (2.0 * f.half[1]);
        PeelFrame { d, r, u, s0: dot(f.center, r) - f.half[0], t0: dot(f.center, u) - f.half[1], scale, scale_y, res: w, res_y: h, zc: dot(f.center, d), half_d: f.half[2], inset_px: 1 }
    }
    /// This frame as the game records a peel frustum.
    pub fn frustum(&self) -> crate::passdump::Frustum {
        let half_w = self.res as f32 / (2.0 * self.scale);
        let half_h = self.res_y as f32 / (2.0 * self.scale_y);
        let sc = self.s0 + half_w;
        let tc = self.t0 + half_h;
        let mut c = [0f32; 3];
        for k in 0..3 {
            c[k] = sc * self.r[k] + tc * self.u[k] + self.zc * self.d[k];
        }
        crate::passdump::Frustum { ortho: true, center: c, half: [half_w, half_h, self.half_d], right: self.r, up: [-self.u[0], -self.u[1], -self.u[2]], forward: self.d, depth: crate::passdump::Frustum::REVERSED.into() }
    }
    /// Pixel-space x, y and the depth of a world point.
    #[inline]
    pub fn project(&self, p: V3) -> (f32, f32, f32) {
        ((dot(p, self.r) - self.s0) * self.scale, (dot(p, self.u) - self.t0) * self.scale_y, -dot(p, self.d))
    }
    /// Where an axis-aligned box sits against this frame's volume (x ∈ [0, res], y ∈ [0, res_y], port z ∈
    /// [zmin, zmax]): None = outside, Some(true) = wholly inside, Some(false) = partly — from the box's eight
    /// corners projected (the box's image is convex, so the corner intervals bound it: a conservative test).
    pub fn box_class(&self, bmin: V3, bmax: V3, zmin: f32, zmax: f32) -> Option<bool> {
        let (mut x0, mut x1, mut y0, mut y1, mut z0, mut z1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for i in 0..8 {
            let p = [if i & 1 == 0 { bmin[0] } else { bmax[0] }, if i & 2 == 0 { bmin[1] } else { bmax[1] }, if i & 4 == 0 { bmin[2] } else { bmax[2] }];
            let (x, y, z) = self.project(p);
            x0 = x0.min(x); x1 = x1.max(x); y0 = y0.min(y); y1 = y1.max(y); z0 = z0.min(z); z1 = z1.max(z);
        }
        if x1 < 0.0 || y1 < 0.0 || x0 > self.res as f32 || y0 > self.res_y as f32 || z1 < zmin || z0 >= zmax {
            return None;
        }
        Some(x0 >= 0.0 && x1 <= self.res as f32 && y0 >= 0.0 && y1 <= self.res_y as f32 && z0 >= zmin && z1 < zmax)
    }
    /// The game's reversed depth of a port depth.
    #[inline]
    pub fn z01(&self, z_port: f32) -> f32 {
        0.5 + (self.zc + z_port) / (2.0 * self.half_d)
    }
    /// The port depth of a reversed z01.
    #[inline]
    pub fn z_from_z01(&self, z01: f32) -> f32 {
        (z01 - 0.5) * 2.0 * self.half_d - self.zc
    }
    /// The world point of pixel-space (x, y) at port depth z.
    #[inline]
    pub fn unproject(&self, x: f32, y: f32, z: f32) -> V3 {
        let s = x / self.scale + self.s0;
        let t = y / self.scale_y + self.t0;
        let mut p = [0f32; 3];
        for k in 0..3 {
            p[k] = s * self.r[k] + t * self.u[k] - z * self.d[k];
        }
        p
    }
    /// The peel's `WorldPw01Shadow` as the accumulate pixel shader (PS 17112) consumes it: the four registers
    /// (columns) mapping a world point to `(u, v, z01, 1)` — u = x_px / W, v = y_px / H over this frame, z01 the
    /// reversed depth. `m[i][k]` multiplies `p[i]`, `m[3][k]` is the constant.
    pub fn world_pw01(&self) -> [[f32; 4]; 4] {
        let (w, h) = (self.res as f32, self.res_y as f32);
        let mut m = [[0f32; 4]; 4];
        for i in 0..3 {
            m[i][0] = self.r[i] * self.scale / w;
            m[i][1] = self.u[i] * self.scale_y / h;
            m[i][2] = -self.d[i] / (2.0 * self.half_d);
        }
        m[3][0] = -self.s0 * self.scale / w;
        m[3][1] = -self.t0 * self.scale_y / h;
        m[3][2] = 0.5 + self.zc / (2.0 * self.half_d);
        m[3][3] = 1.0;
        m
    }
    /// The world size of one peel pixel.
    pub fn pixel_m(&self) -> f32 {
        1.0 / self.scale
    }
    /// Push the far plane out to cover every vertex of `tris` (the occluders below / beyond the
    /// receivers: the ground, the sea, the decoration), keeping the near plane where it is — without
    /// it everything beyond the receivers' bbox would pancake onto one far-plane layer and merge.
    pub fn extend_far(&mut self, bvh: &Bvh) {
        let zmax = self.z_from_z01(1.0);
        let zmin0 = self.z_from_z01(0.0);
        // z = −p·d: its minimum over every vertex = the BVH's exact extreme along −d (a pruned descent)
        let zmin = zmin0.min(bvh.min_dot([-self.d[0], -self.d[1], -self.d[2]]));
        self.half_d = (0.5 * (zmax - zmin)).max(1e-3);
        self.zc = -0.5 * (zmin + zmax);
    }
}

/// LMTOOL_ABUF_DEBUG=x,y: print every fragment (and every alpha-tested candidate) of one peel pixel.
pub static ABUF_DEBUG: std::sync::LazyLock<Option<(u32, u32)>> = std::sync::LazyLock::new(|| {
    let s = std::env::var("LMTOOL_ABUF_DEBUG").ok()?;
    let v: Vec<u32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    if v.len() == 2 { Some((v[0], v[1])) } else { None }
});

/// The cards' alpha test threshold: GbxShadowAlphaThreshold = 128/255 (the capture's ShaderP cbuffer).
pub const ALPHA_THRESHOLD: f32 = 0.501_960_813_999_176;
/// LMTOOL_ALPHA_POINT=1: the point-sampled cut-out mask instead of the filtered texture (a probe).
pub static ALPHA_POINT: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_ALPHA_POINT").map(|v| v == "1").unwrap_or(false));
/// LMTOOL_ALPHA_ANISO=N: the alpha sampler's anisotropy (16 = the capture's card sampler; 1 = trilinear).
pub static ALPHA_ANISO: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_ALPHA_ANISO").ok().and_then(|v| v.parse().ok()).unwrap_or(16));

/// One fragment of the A-buffer: depth and the world triangle (index into the BVH's triangle list).
#[derive(Clone, Copy, Debug)]
pub struct Frag {
    pub z: f32,
    pub tri: u32,
}

/// All fragments of a peel, CSR by pixel, sorted by depth within a pixel.
/// The peel's item-layer cap: the client's state machine stops after 21 renders, the environment block
/// included (sweep 0: 20 item layers; sweep ≥ 1, no environment block: 21).
pub const MAX_LAYERS: usize = 21;

/// The WANTED pixels of a peel as a rank structure: a bitmap over the frame plus the prefix popcount
/// per 64-pixel word, so a pixel's dense index among the wanted ones is O(1) — the sparse A-buffer and
/// layer tables are sized by the wanted count (a million) instead of the frame (17 million).
pub struct PixelIndex {
    pub res: u32,
    pub res_y: u32,
    pub words: Vec<u64>,
    pub rank: Vec<u32>,
    /// The wanted pixels in index order (pixel id = y·res + x).
    pub pixels: Vec<u32>,
    /// Their bounding rectangle (x0, y0, x1, y1) inclusive; (0, 0, −1, −1) when empty.
    pub bbox: (i32, i32, i32, i32),
}

impl PixelIndex {
    pub fn new(res: u32, res_y: u32, words: Vec<u64>) -> PixelIndex {
        // in parallel chunks of words: the popcounts, a prefix over the chunks, then each chunk fills its
        // ranks and its slice of the pixel list (a 4096² frame is 262 k words; ten frames per direction)
        let n = words.len();
        let threads = crate::pool::pool().threads.max(1);
        let chunk = (n / (threads * 2).max(1)).max(1024);
        let n_chunks = (n + chunk - 1) / chunk;
        let counts: Vec<u32> = crate::pool::pool().map(n_chunks, |ci| words[ci * chunk..((ci + 1) * chunk).min(n)].iter().map(|w| w.count_ones()).sum());
        let mut base = Vec::with_capacity(n_chunks + 1);
        base.push(0u32);
        for c in &counts { let last = *base.last().unwrap(); base.push(last + c); }
        let total = *base.last().unwrap() as usize;
        let mut rank: Vec<u32> = Vec::with_capacity(n + 1);
        let mut pixels: Vec<u32> = Vec::with_capacity(total);
        // SAFETY: every slot is written exactly once below by the chunk that owns it
        unsafe { rank.set_len(n + 1); pixels.set_len(total); }
        rank[n] = total as u32;
        let (rp, pp) = (rank.as_mut_ptr() as usize, pixels.as_mut_ptr() as usize);
        let boxes: Vec<(i32, i32, i32, i32)> = {
            let words = &words;
            let base = &base;
            crate::pool::pool().map(n_chunks, |ci| {
                let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
                let mut acc = base[ci];
                for wi in ci * chunk..((ci + 1) * chunk).min(n) {
                    let w = words[wi];
                    unsafe { *(rp as *mut u32).add(wi) = acc; }
                    let mut m = w;
                    while m != 0 {
                        let b = m.trailing_zeros();
                        m &= m - 1;
                        let id = wi as u32 * 64 + b;
                        unsafe { *(pp as *mut u32).add(acc as usize) = id; }
                        acc += 1;
                        let (x, y) = ((id % res) as i32, (id / res) as i32);
                        x0 = x0.min(x); x1 = x1.max(x); y0 = y0.min(y); y1 = y1.max(y);
                    }
                }
                (x0, y0, x1, y1)
            })
        };
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for b in &boxes { x0 = x0.min(b.0); y0 = y0.min(b.1); x1 = x1.max(b.2); y1 = y1.max(b.3); }
        let bbox = if x0 > x1 { (0, 0, -1, -1) } else { (x0, y0, x1, y1) };
        PixelIndex { res, res_y, words, rank, pixels, bbox }
    }
    pub fn len(&self) -> usize {
        self.pixels.len()
    }
    /// The dense index of pixel (x, y), or None when it is not wanted.
    #[inline]
    pub fn index(&self, x: u32, y: u32) -> Option<u32> {
        let i = (y as usize) * self.res as usize + x as usize;
        let w = self.words[i >> 6];
        let b = (i & 63) as u32;
        if (w >> b) & 1 == 0 {
            return None;
        }
        Some(self.rank[i >> 6] + (w & ((1u64 << b) - 1)).count_ones())
    }
    #[inline]
    pub fn index_of_id(&self, id: u32) -> u32 {
        let i = id as usize;
        let w = self.words[i >> 6];
        let b = (i & 63) as u32;
        self.rank[i >> 6] + (w & ((1u64 << b) - 1)).count_ones()
    }
}

/// Recycled buffers: the per-frame fragment tables (tens of MB, above glibc's mmap threshold) would be
/// mapped and unmapped every direction — 60 k page faults per direction on a tiny map, a tenth of its time.
/// A dropped table hands its allocation back here; the next frame takes one with its capacity (and its
/// pages) intact.
pub struct Recycle<T> {
    pool: std::sync::Mutex<Vec<Vec<T>>>,
}

impl<T> Recycle<T> {
    pub const fn new() -> Recycle<T> {
        Recycle { pool: std::sync::Mutex::new(Vec::new()) }
    }
    /// An empty vector, with a recycled capacity when one is available.
    pub fn take(&self) -> Vec<T> {
        self.pool.lock().unwrap().pop().unwrap_or_default()
    }
    /// A vector with at least `cap` capacity (the largest recycled one is preferred when it fits).
    pub fn take_with_capacity(&self, cap: usize) -> Vec<T> {
        let mut v = {
            let mut p = self.pool.lock().unwrap();
            match p.iter().position(|v| v.capacity() >= cap) {
                Some(i) => p.swap_remove(i),
                None => p.pop().unwrap_or_default(),
            }
        };
        v.clear();
        v.reserve(cap);
        v
    }
    pub fn give(&self, mut v: Vec<T>) {
        if v.capacity() == 0 {
            return;
        }
        v.clear();
        let mut p = self.pool.lock().unwrap();
        if p.len() < 12 {
            p.push(v);
        }
    }
}

pub static LAYER_FRAGS: Recycle<LayerFrag> = Recycle::new();
pub static ABUF_FRAGS: Recycle<Frag> = Recycle::new();
pub static U32S: Recycle<u32> = Recycle::new();

impl Drop for Layers {
    fn drop(&mut self) {
        LAYER_FRAGS.give(std::mem::take(&mut self.frags));
        U32S.give(std::mem::take(&mut self.start));
    }
}

impl Drop for ABuffer {
    fn drop(&mut self) {
        for (start, frags) in self.bands.drain(..) {
            U32S.give(start);
            ABUF_FRAGS.give(frags);
        }
    }
}

pub struct ABuffer {
    pub res: u32,
    /// Rows per band; the bands' CSR tables stay separate (no serial stitch of ~100 M fragments).
    pub band_h: u32,
    pub bands: Vec<(Vec<u32>, Vec<Frag>)>,
    /// The sparse form: one CSR over the wanted pixels' dense indices (`bands[0]`), `sparse` the index.
    pub sparse: Option<std::sync::Arc<PixelIndex>>,
}

impl ABuffer {
    /// The fragments of pixel (x, y), nearest first.
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> &[Frag] {
        if let Some(px) = &self.sparse {
            let Some(k) = px.index(x, y) else { return &[] };
            let b = &self.bands[0];
            let (a, c) = (b.0[k as usize] as usize, b.0[k as usize + 1] as usize);
            static LAYERS_NEAR2: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            return if *LAYERS_NEAR2.get_or_init(|| std::env::var_os("LMTOOL_LAYERS_NEAR").is_some()) { &b.1[c.saturating_sub(MAX_LAYERS).max(a)..c] } else { &b.1[a..c.min(a + MAX_LAYERS)] };
        }
        let b = &self.bands[(y / self.band_h) as usize];
        let i = ((y - (y / self.band_h) * self.band_h) * self.res + x) as usize;
        // the game peels at most 20 layers per direction (RenderLightIndirectPeel, counter > 0x13 stops):
        // the 20 kept here are the 20 nearest the SKY (smallest z); a texel deeper than all of them takes
        // the 20th. (Keeping the 20 nearest the receivers instead — "layer 1 = the deepest surface" — was
        // tried 2026-09-24 02:00Z: tiny 16 went from +42 % to +57 %; neither explains the editor's darker
        // hills under dense canopies. LMTOOL_LAYERS_NEAR=1 selects the receiver-side reading.)
        let (a, c) = (b.0[i] as usize, b.0[i + 1] as usize);
        // (the environment is read ONCE: std::env takes a process-wide lock, and this runs per pixel on
        // every thread — it serialised the whole gather)
        static LAYERS_NEAR: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *LAYERS_NEAR.get_or_init(|| std::env::var_os("LMTOOL_LAYERS_NEAR").is_some()) {
            &b.1[c.saturating_sub(MAX_LAYERS).max(a)..c]
        } else {
            &b.1[a..c.min(a + MAX_LAYERS)]
        }
    }
    /// Every fragment of pixel (x, y), nearest the sky first, without the layer cap.
    #[inline]
    pub fn at_all(&self, x: u32, y: u32) -> &[Frag] {
        if let Some(px) = &self.sparse {
            let Some(k) = px.index(x, y) else { return &[] };
            let b = &self.bands[0];
            return &b.1[b.0[k as usize] as usize..b.0[k as usize + 1] as usize];
        }
        let b = &self.bands[(y / self.band_h) as usize];
        let i = ((y - (y / self.band_h) * self.band_h) * self.res + x) as usize;
        &b.1[b.0[i] as usize..b.0[i + 1] as usize]
    }
    /// Total fragment count.
    pub fn len(&self) -> usize {
        self.bands.iter().map(|b| b.1.len()).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Every fragment, band by band.
    pub fn iter(&self) -> impl Iterator<Item = &Frag> {
        self.bands.iter().flat_map(|b| b.1.iter())
    }
}

/// Raster every world triangle into the frame's A-buffer: the raster runs in parallel over triangle
/// chunks, each thread binning its fragments into horizontal BANDS of the target; the per-band CSR
/// build (counting sort by pixel, depth sort within a pixel) then runs in parallel over the bands.
pub fn build_abuffer(tris: &[WTri], frame: &PeelFrame, threads: usize) -> ABuffer {
    build_abuffer_upto(tris, frame, threads, f32::INFINITY, &[])
}

/// `build_abuffer` keeping only fragments with depth < `zmax` — a fragment deeper than every receiver
/// can occlude nothing (the gather looks for surfaces in FRONT of a texel), and the decoration's sea
/// and ground planes would otherwise fill every pixel of every peel.
/// Whether the vegetation cards (alpha-tested materials) occlude in the peel and the sun shadow map.
/// LMTOOL_CARDS_OCCLUDE=0 takes them out (they stay receivers): the hill test map's slopes under a
/// dense bush canopy read 0.19 in the editor at Day where cards that occlude give 0.035 (2026-09-23).
pub fn cards_occlude() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_CARDS_OCCLUDE").map(|v| v != "0").unwrap_or(true))
}

/// Whether the cards cast SUN shadows (the bake's sun shadow map). LMTOOL_CARDS_SHADOW=0 leaves them out.
pub fn cards_shadow() -> bool {
    // default OFF (DIFFERENTIAL, the hill test map 2026-09-23 23:50Z: the slopes under a dense canopy read
    // 0.19 of the editor's at Day with the cards in the sun shadow map, 0.76–1.4 without); =1 puts them in
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_CARDS_SHADOW").map(|v| v == "1").unwrap_or(false))
}

pub fn build_abuffer_upto(tris: &[WTri], frame: &PeelFrame, threads: usize, zmax: f32, masks: &[crate::geometry::AlphaMask]) -> ABuffer {
    build_abuffer_range(tris, frame, threads, f32::NEG_INFINITY, zmax, masks)
}

/// `build_abuffer_upto` with both depth planes: fragments with depth outside [zmin, zmax) are clipped
/// (the D3D depth clip of a fragment outside the frustum's near/far planes).
pub fn build_abuffer_range(tris: &[WTri], frame: &PeelFrame, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask]) -> ABuffer {
    build_abuffer_wanted(tris, frame, threads, zmin, zmax, masks, None)
}

/// A pixel bitmap (row-major, one bit per pixel) — the peel pixels some texel reads.
#[inline]
pub fn bit(mask: &[u64], i: usize) -> bool {
    (mask[i >> 6] >> (i & 63)) & 1 == 1
}

/// `build_abuffer_range` keeping only the fragments of the `wanted` pixels (the layers of a pixel depend
/// on that pixel's fragments alone, so the pixels no texel looks up are never derived — the speed of the
/// harness; the dump of a direction wants every pixel and passes None).
pub static RS_TRIS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static RS_VISITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static RS_TESTED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// LMTOOL_RASTER_STATS=1 counts the rasterised triangles and pixel visits per peel (read once).
fn raster_stats_on() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("LMTOOL_RASTER_STATS").is_some())
}

/// The sparse A-buffer of the wanted pixels: the raster runs in parallel over horizontal BANDS of the
/// wanted rectangle (every thread walks every triangle that reaches its rows — a huge triangle no longer
/// pins one thread), fragments bucketed by the dense index, one CSR over the wanted pixels. A pixel's
/// fragments are produced by one band thread in triangle order, as the dense build orders them.
/// What the combined build counts alongside the sparse A-buffer: the exact item-layer statistic over
/// the whole frame (`exact_item_layers_direct`'s work, from the same raster pass).
pub struct CountCtx<'a> {
    pub scene: &'a Scene,
    pub bvh: &'a Bvh,
    pub prm: &'a BakeParams,
}

pub fn build_abuffer_sparse(tris: &[WTri], frame: &PeelFrame, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask], px: &std::sync::Arc<PixelIndex>) -> ABuffer {
    build_abuffer_sparse_counted(tris, frame, threads, zmin, zmax, masks, px, None).0
}

/// `build_abuffer_sparse`, and — with a `CountCtx` — the exact layer-count statistic from the SAME raster
/// pass: every pixel of the frame is visited, every fragment goes to the count, the wanted ones also to
/// the A-buffer (one walk over the triangles instead of two).
pub fn build_abuffer_sparse_counted(tris: &[WTri], frame: &PeelFrame, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask], px: &std::sync::Arc<PixelIndex>, count: Option<CountCtx>) -> (ABuffer, Option<(usize, Vec<f64>)>) {
    build_abuffer_sparse_ranges(tris, &[(0, tris.len() as u32)], frame, threads, zmin, zmax, masks, px, count)
}

/// `build_abuffer_sparse_counted` over the triangle index RANGES the caller culled (the BVH's leaf ranges
/// meeting the frame: `Bvh::ranges_where` with `PeelFrame::box_class`) — a tile of a giant considers a
/// ninth of the scene. The ranges must be sorted and disjoint (the triangle order within a pixel).
pub fn build_abuffer_sparse_ranges(tris: &[WTri], ranges: &[(u32, u32)], frame: &PeelFrame, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask], px: &std::sync::Arc<PixelIndex>, count: Option<CountCtx>) -> (ABuffer, Option<(usize, Vec<f64>)>) {
    let res = frame.res;
    let res_y = frame.res_y;
    let t_clip = std::time::Instant::now();
    let counting = count.is_some();
    // the count needs every pixel: the whole frame is the clip then — minus the game's viewport ring (the
    // viewport is (1, 1, w−2, h−2): the outer ring is never drawn; clipping it here spares the per-visit test)
    let ins = frame.inset_px as i32;
    let clip = if counting { (0i32, 0i32, res as i32 - 1, res_y as i32 - 1) } else { px.bbox };
    let clip = (clip.0.max(ins), clip.1.max(ins), clip.2.min(res as i32 - 1 - ins), clip.3.min(res_y as i32 - 1 - ins));
    // in counting mode the environment fragments (the dome layer's sea box / terrain, `is_env`) feed a
    // per-pixel MAX of their z01 instead of the fragment list: the layer logic reads only that maximum of
    // them (the environment layer's depth), and they are the bulk of the pixel visits (32 m ground quads)
    let env_class = |t: &WTri| -> Option<bool> {
        let cx = count.as_ref()?;
        if !cx.prm.dome_layer || t.inst != DECOR_INST { return None; }
        match cx.scene.decor.get(t.tri as usize) {
            Some(dt) if dt.env => Some(match dt.env_far_only { true => { let n = cross(t.e1, t.e2); dot(n, frame.d) > 0.0 } false => true }),
            _ => None,
        }
    };
    let cull_back = {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var("LMTOOL_PEEL_CULL_BACK").map(|v| v == "1").unwrap_or(false))
    };
    let cards_occlude = cards_occlude();
    let d = frame.d;
    let inset = frame.inset_px;
    // THE BANDS: the wanted rectangle's rows split evenly over the threads; every triangle that reaches
    // the frame's depth range is binned into the bands its rows touch — a per-band list of triangle
    // indices (4 bytes each: tiny 16 has 2.4 M triangles, the giants 27 M — a projected record per
    // triangle would be gigabytes per direction) — so a band thread walks its own triangles only and
    // projects them again (cheaper than storing the projection). Triangle order within a band is the
    // scene order (chunks concatenated in order), as the dense build's.
    let raster_stats = raster_stats_on();
    let bitmap: &[u64] = px.words.as_slice();
    let rows = (clip.3 - clip.1 + 1).max(0) as usize;
    // BANDS_PER_THREAD bands per pool thread: the pool hands them out dynamically, so a band of dense
    // forest no longer pins the whole pass to one thread (with one band per thread the slowest band was
    // 3–4× the mean and the other threads idled — LMTOOL_RASTER_STATS prints the band times)
    let n_bands = (threads.max(1) * *BANDS_PER_THREAD).min(rows.max(1));
    let band_rows = (rows + n_bands - 1) / n_bands.max(1);
    let band_of = |y: i32| -> usize { (((y - clip.1).max(0) as usize) / band_rows.max(1)).min(n_bands - 1) };
    // a triangle's projected rows (as raster::bounds computes them), clipped to the wanted rectangle
    let rows_of = |t: &WTri| -> Option<(i32, i32)> {
        let p0 = t.p0;
        let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
        let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
        let (x0, y0, z0) = frame.project(p0);
        let (x1, y1, z1) = frame.project(p1);
        let (x2, y2, z2) = frame.project(p2);
        if z0.min(z1).min(z2) >= zmax || z0.max(z1).max(z2) < zmin {
            return None;
        }
        if cull_back && t.inst != DECOR_INST {
            let ng = cross(t.e1, t.e2);
            if dot(ng, d) > 0.0 {
                return None;
            }
        }
        if !cards_occlude && t.alpha != u16::MAX {
            return None;
        }
        let (miny, maxy) = (y0.min(y1).min(y2), y0.max(y1).max(y2));
        if !(miny.is_finite() && maxy.is_finite()) {
            return None;
        }
        let ry0 = ((miny - 0.5).ceil() as i64).max(clip.1 as i64) as i32;
        let ry1 = ((maxy - 0.5).floor() as i64).min(clip.3 as i64) as i32;
        if ry0 > ry1 {
            return None;
        }
        // no pixel centre between the columns either → the raster would visit nothing (most leaf
        // triangles: the same test `raster::bounds` makes, so the visited set is unchanged)
        let (minx, maxx) = (x0.min(x1).min(x2), x0.max(x1).max(x2));
        if !(minx.is_finite() && maxx.is_finite()) {
            return None;
        }
        let rx0 = ((minx - 0.5).ceil() as i64).max(clip.0 as i64);
        let rx1 = ((maxx - 0.5).floor() as i64).min(clip.2 as i64);
        if rx0 > rx1 {
            return None;
        }
        Some((ry0, ry1))
    };
    // the culled triangle set as chunks of index ranges (each chunk ~ n_total / (4·threads) triangles)
    let n_total: usize = ranges.iter().map(|r| (r.1 - r.0) as usize).sum();
    let prep_chunk = (n_total / (threads * 4).max(1)).max(4096);
    let mut chunks: Vec<(u32, u32)> = Vec::new();
    for &(a, b) in ranges {
        let mut x = a;
        while x < b {
            let y = (x as usize + prep_chunk).min(b as usize) as u32;
            chunks.push((x, y));
            x = y;
        }
    }
    let n_prep = chunks.len();
    // per chunk: per band the triangle indices (in triangle order)
    let binned: Vec<Vec<Vec<u32>>> = crate::pool::pool().map(n_prep, |ci| {
        let mut out: Vec<Vec<u32>> = (0..n_bands).map(|_| Vec::new()).collect();
        let (a, b) = chunks[ci];
        for (k, t) in tris[a as usize..b as usize].iter().enumerate() {
            if let Some((ry0, ry1)) = rows_of(t) {
                let (b0, b1) = (band_of(ry0), band_of(ry1));
                for bb in b0..=b1 {
                    out[bb].push(a + k as u32);
                }
            }
        }
        out
    });
    prof::add(&prof::B_CLIP, t_clip);
    let t_raster = std::time::Instant::now();
    // (one CSR bucket per pool thread: the per-pixel depth sorts of a dense canopy are the cost)
    let sparse_buckets = (threads as u32).clamp(1, 256);
    let sparse_bucket_size = ((px.len() as u32 + sparse_buckets - 1) / sparse_buckets).max(1);
    let band_ns: Vec<std::sync::atomic::AtomicU64> = if raster_stats { (0..n_bands).map(|_| std::sync::atomic::AtomicU64::new(0)).collect() } else { Vec::new() };
    // THE COUNT'S LAYER LOGIC per pixel (counting mode): `list` = the pixel's item fragments sorted by (z,
    // triangle) each carrying its triangle's depth-bias term, `env_d` = the environment layer's z01 maximum
    // at the pixel (0 = none) — `extract_layers`' depth rules, no colour
    let count_run = |list: &[CFrag], env_d: f32| -> usize {
        let cx = count.as_ref().unwrap();
        let prm = cx.prm;
        let mut d_prev = f32::NEG_INFINITY;
        if prm.dome_layer {
            d_prev = if env_d > 0.0 { if prm.depth_bits == 16 { (env_d * 65535.0).round() / 65535.0 } else { env_d } } else { 0.0 };
        }
        let mut items = 0usize;
        for f in list {
            let z01 = frame.z01(f.z).max(0.0);
            if z01 < d_prev {
                continue;
            }
            if items >= MAX_LAYERS {
                break;
            }
            let mut dd = z01 + f.bias;
            if prm.depth_bits == 16 {
                dd = (dd.clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
            }
            items += 1;
            d_prev = dd;
        }
        items
    };
    // LMTOOL_BOUND_STATS=1 (measurement): per frame the fragment-count histogram (an upper bound on the layer
    // fractions) and the exact layer histogram on the census pixels (every 4th / 8th in x and y: a lower
    // bound) beside the exact one — how often would the two bounds decide the game's stop without the full
    // exact count?
    let bound_stats = std::env::var_os("LMTOOL_BOUND_STATS").is_some();
    let bound_acc: std::sync::Mutex<([usize; MAX_LAYERS + 1], [usize; MAX_LAYERS + 1], [usize; MAX_LAYERS + 1])> = std::sync::Mutex::new(([0; MAX_LAYERS + 1], [0; MAX_LAYERS + 1], [0; MAX_LAYERS + 1]));
    let parts_all: Vec<(Vec<Vec<(u32, Frag)>>, [usize; MAX_LAYERS + 1], usize)> = crate::pool::pool().map(n_bands, |b| {
        let t_band = std::time::Instant::now();
        let mut hist_u = [0usize; MAX_LAYERS + 1];
        let mut hist_l4 = [0usize; MAX_LAYERS + 1];
        let mut hist_l8 = [0usize; MAX_LAYERS + 1];
        let (mut rs_tris, mut rs_tested, mut rs_visits) = (0u64, 0u64, 0u64);
        let by0 = clip.1 + (b * band_rows) as i32;
        let by1 = (clip.1 + ((b + 1) * band_rows) as i32 - 1).min(clip.3);
        let mut out: Vec<Vec<(u32, Frag)>> = (0..sparse_buckets).map(|_| Vec::new()).collect();
        let mut hist = [0usize; MAX_LAYERS + 1];
        let mut covered = 0usize;
        // (counting) THE BAND'S PIXEL SLOTS: per pixel the environment layer's z01 maximum (0 = none drawn)
        // and up to SLOT_K item fragments inline, the rest in the overflow list — no fragment list to sort
        // (the tiles of a giant produce 400 M fragments per direction), the band's slots stay in cache
        let band_px = if counting { ((by1 - by0 + 1).max(0) as usize) * res as usize } else { 0 };
        // THE COUNT'S TABLES, per band: a fragment COUNT per pixel (u16: L1-resident) and the environment
        // layer's z01 maximum; the item fragments themselves go to a sequential LIST (pixel, fragment) — the
        // scan then counting-sorts the list by pixel (a prefix over the counts, one scatter) and runs the layer
        // logic per pixel. (The previous form wrote each fragment into a 64-byte per-pixel slot at visit time:
        // 600 M random cache-line writes per direction into a table larger than L2 — a quarter of the raster.)
        // The tables come from a per-thread pool; the scan resets what it read.
        let mut bufs = SLOT_BUFS.take();
        let (mut cnt, mut env_max, mut list, mut csr, mut offs, mut fill) = (std::mem::take(&mut bufs.0), std::mem::take(&mut bufs.1), std::mem::take(&mut bufs.2), std::mem::take(&mut bufs.3), std::mem::take(&mut bufs.4), std::mem::take(&mut bufs.5));
        if cnt.len() < band_px { cnt.resize(band_px, 0); }
        if env_max.len() < band_px { env_max.resize(band_px, 0.0); }
        list.clear();
        let mut saturated = false;
        if by0 > by1 {
            SLOT_BUFS.set((cnt, env_max, list, csr, offs, fill));
            return (out, hist, covered);
        }
        let band_clip = (clip.0, by0, clip.2, by1);
        for chunk_lists in &binned {
            for &ti in &chunk_lists[b] {
                let t = &tris[ti as usize];
                let p0 = t.p0;
                let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                let (x0, y0, z0) = frame.project(p0);
                let (x1, y1, z1) = frame.project(p1);
                let (x2, y2, z2) = frame.project(p2);
                let mask = if t.alpha != u16::MAX { masks.get(t.alpha as usize) } else { None };
                // THE FILTERED ALPHA TEST (engineer D; PS 17134: the opacity texture sampled at TexCoord0 with the
                // material sampler, alpha ≥ GbxShadowAlphaThreshold): the triangle's uv footprint gives the level of detail
                // (the footprint is computed on the first fragment that needs it: most leaf triangles cover
                // no pixel centre at all)
                let mut fp_tex: Option<Option<(&crate::alphatex::AlphaTex, crate::alphatex::TapPlan)>> = None;
                if raster_stats { rs_tris += 1; rs_tested += raster::bbox_pixels([[x0, y0], [x1, y1], [x2, y2]], res, res_y, band_clip); }
                // (counting) an environment triangle: Some(drawn); an item triangle: its depth-bias term (once
                // per triangle — `tri_slope` per fragment was 8 % of a bake)
                let env_t = if counting { env_class(t) } else { None };
                // (computed on the first item fragment of the triangle in this band: most triangles in a band's
                // list cover none of its pixel centres)
                let mut bias_term_cache: Option<f32> = None;
                let bias_term_of = |cache: &mut Option<f32>| -> f32 {
                    *cache.get_or_insert_with(|| {
                        let cx = count.as_ref().unwrap();
                        let (slope, zmax_prim) = tri_slope_projected(frame, (x0, y0, z0), (x1, y1, z1), (x2, y2, z2));
                        d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), cx.prm.depth_bias, cx.prm.depth_bits)
                    })
                };
                raster::triangle_clipped_masked(res, res_y, [[x0, y0], [x1, y1], [x2, y2]], band_clip, if counting { None } else { Some(bitmap) }, |x, y, bc| {
                    if raster_stats { rs_visits += 1; }
                    // (the game's viewport ring is outside the clip rectangle: never visited)
                    debug_assert!(!(x < inset || y < inset || x + inset >= res || y + inset >= res_y));
                    let z = z0 * bc[0] + z1 * bc[1] + z2 * bc[2];
                    if z < zmax && z >= zmin {
                        if let Some(m) = mask {
                            let u = t.uv0[0][0] * bc[0] + t.uv0[1][0] * bc[1] + t.uv0[2][0] * bc[2];
                            let v = t.uv0[0][1] * bc[0] + t.uv0[1][1] * bc[1] + t.uv0[2][1] * bc[2];
                            let fp = fp_tex.get_or_insert_with(|| m.tex.as_ref().map(|tx| (tx.as_ref(), tx.plan(&crate::alphatex::Footprint::of_triangle([[x0, y0], [x1, y1], [x2, y2]], t.uv0, tx.w(), tx.h()), *ALPHA_ANISO))));
                            let op = match fp {
                                Some((tx, fp)) if !*ALPHA_POINT => tx.passes_planned(u, v, fp, ALPHA_THRESHOLD, crate::alphatex::Address::ClampEdge),
                                _ => m.opaque(u, v),
                            };
                            if let Some((dx, dy)) = *ABUF_DEBUG { if x == dx && y == dy { eprintln!("abuf debug ({x},{y}): card tri {ti} inst {} model tri {} mask {} uv ({u:.4},{v:.4}) opaque {op} z {z:.3} z01 {:.5}", t.inst, t.tri, t.alpha, frame.z01(z)); } }
                            if !op {
                                return;
                            }
                        } else if let Some((dx, dy)) = *ABUF_DEBUG { if x == dx && y == dy { eprintln!("abuf debug ({x},{y}): tri {ti} inst {} model tri {} z {z:.3} z01 {:.5}", t.inst, t.tri, frame.z01(z)); } }
                        let id = y * res + x;
                        if counting {
                            let li = ((y as i32 - by0) as usize) * res as usize + x as usize;
                            match env_t {
                                Some(drawn) => {
                                    if drawn {
                                        let z01 = frame.z01(z);
                                        if z01 >= 0.0 && z01 <= 1.0 {
                                            let e = &mut env_max[li];
                                            if z01 > *e { *e = z01; }
                                        }
                                    }
                                }
                                None => {
                                    let c = &mut cnt[li];
                                    if *c == u16::MAX { saturated = true; } else { *c += 1; }
                                    list.push((li as u32, CFrag { z, tri: ti, bias: bias_term_of(&mut bias_term_cache) }));
                                }
                            }
                            if !bit(bitmap, id as usize) {
                                return;
                            }
                        }
                        let k = px.index_of_id(id);
                        out[(k / sparse_bucket_size) as usize].push((k, Frag { z, tri: ti }));
                    }
                });
            }
        }
        if counting {
            // THE SCAN: the list counting-sorted by pixel (the prefix over the counts gives every pixel's range,
            // the scatter fills it in visit order), then per pixel the fragments ordered by (z, triangle) and
            // the layer logic. The prefix comes from the u16 counts (one pass over the band's pixels) unless
            // a pixel saturated them — then from the list.
            if offs.len() < band_px + 1 { offs.resize(band_px + 1, 0); }
            if fill.len() < band_px { fill.resize(band_px, 0); }
            offs[0] = 0;
            if saturated {
                offs[..band_px + 1].fill(0);
                for (li, _) in &list { offs[*li as usize + 1] += 1; }
                for i in 0..band_px { offs[i + 1] += offs[i]; }
            } else {
                let mut acc = 0u32;
                for i in 0..band_px { fill[i] = acc; acc += cnt[i] as u32; offs[i + 1] = acc; }
            }
            if saturated { fill[..band_px].copy_from_slice(&offs[..band_px]); }
            if csr.len() < list.len() { csr.resize(list.len(), CFrag { z: 0.0, tri: 0, bias: 0.0 }); }
            // the scatter: fill[li] walks forward as its pixel's fragments land
            for (li, cf) in &list {
                let f = &mut fill[*li as usize];
                csr[*f as usize] = *cf;
                *f += 1;
            }
            for li in 0..band_px {
                if cnt[li] == 0 && !saturated {
                    // no item fragment: nothing to count (an environment-only pixel resets its maximum)
                    env_max[li] = 0.0;
                    continue;
                }
                let env_d = std::mem::replace(&mut env_max[li], 0.0);
                let (a, c) = (offs[li] as usize, offs[li + 1] as usize);
                cnt[li] = 0;
                if a == c { continue; }
                covered += 1;
                let n = c - a;
                if bound_stats {
                    hist_u[n.min(MAX_LAYERS)] += 1;
                    let (x, y) = (li % res as usize, by0 as usize + li / res as usize);
                    if x % 4 == 0 && y % 4 == 0 {
                        let mut tmp: Vec<CFrag> = csr[a..c].to_vec();
                        tmp.sort_by(|p, q| p.z.total_cmp(&q.z).then_with(|| p.tri.cmp(&q.tri)));
                        let layers = count_run(&tmp, env_d).min(MAX_LAYERS);
                        hist_l4[layers] += 1;
                        if x % 8 == 0 && y % 8 == 0 { hist_l8[layers] += 1; }
                    }
                }
                if n == 1 && env_d == 0.0 {
                    // one item fragment, no environment layer at the pixel: the layer logic accepts it
                    // (z01.max(0) ≥ the initial d_prev of 0 or −∞) — one layer, no sort, no z01
                    hist[1] += 1;
                    continue;
                }
                let buf = &mut csr[a..c];
                // the (z, triangle) order — the keys are distinct within a pixel (a triangle visits a pixel
                // once), so any correct sort gives the one order: tiny networks for the common sizes
                let key = |f: &CFrag, g: &CFrag| f.z.total_cmp(&g.z).then_with(|| f.tri.cmp(&g.tri)) == std::cmp::Ordering::Greater;
                match n {
                    1 => {}
                    2 => { if key(&buf[0], &buf[1]) { buf.swap(0, 1); } }
                    3 => {
                        if key(&buf[0], &buf[1]) { buf.swap(0, 1); }
                        if key(&buf[1], &buf[2]) { buf.swap(1, 2); }
                        if key(&buf[0], &buf[1]) { buf.swap(0, 1); }
                    }
                    _ => buf.sort_by(|p, q| p.z.total_cmp(&q.z).then_with(|| p.tri.cmp(&q.tri))),
                }
                hist[count_run(buf, env_d).min(MAX_LAYERS)] += 1;
            }
        }
        if bound_stats { let mut g = bound_acc.lock().unwrap(); for k in 0..=MAX_LAYERS { g.0[k] += hist_u[k]; g.1[k] += hist_l4[k]; g.2[k] += hist_l8[k]; } }
        SLOT_BUFS.set((cnt, env_max, list, csr, offs, fill));
        if raster_stats { band_ns[b].store(t_band.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed); RS_TRIS.fetch_add(rs_tris, std::sync::atomic::Ordering::Relaxed); RS_TESTED.fetch_add(rs_tested, std::sync::atomic::Ordering::Relaxed); RS_VISITS.fetch_add(rs_visits, std::sync::atomic::Ordering::Relaxed); }
        (out, hist, covered)
    });
    if raster_stats {
        let v: Vec<u64> = band_ns.iter().map(|a| a.load(std::sync::atomic::Ordering::Relaxed)).collect();
        let mx = v.iter().copied().max().unwrap_or(0);
        let mean = v.iter().sum::<u64>() as f64 / v.len().max(1) as f64;
        eprintln!("raster bands: {} bands of {} rows, slowest {:.1} ms, mean {:.1} ms (ratio {:.2}), wall {:.1} ms", n_bands, band_rows, mx as f64 / 1e6, mean / 1e6, mx as f64 / mean.max(1.0), t_raster.elapsed().as_secs_f64() * 1e3);
    }
    // the exact statistic from the bands' histograms
    let counted: Option<(usize, Vec<f64>)> = count.as_ref().map(|cx| {
        let prm = cx.prm;
        let n = (res * res_y) as usize;
        let mut hist = [0usize; MAX_LAYERS + 1];
        let mut covered = 0usize;
        for (_, hh, c) in &parts_all { for k in 0..=MAX_LAYERS { hist[k] += hh[k]; } covered += c; }
        hist[0] += n - covered;
        let mut fractions: Vec<f64> = Vec::with_capacity(MAX_LAYERS);
        let mut at_least = n;
        for k in 0..MAX_LAYERS {
            at_least -= hist[k];
            fractions.push(at_least as f64 / n.max(1) as f64);
        }
        let rendered = prm.peel_stop.layers_rendered(&fractions);
        if bound_stats {
            // the bounds' verdict: U from the fragment counts (pixels with ≥ k+1 fragments / N), L from the
            // census exact counts (pixels with ≥ k+1 layers among the census / N — a subset's count is a
            // lower bound on the whole's); the stop is certified when every fraction the rule reads is on
            // one side of the threshold for both bounds
            let g = bound_acc.lock().unwrap();
            let frac_of = |h: &[usize; MAX_LAYERS + 1], covered_h: usize| -> Vec<f64> {
                let mut hh = *h; hh[0] += n - covered_h;
                let mut out = Vec::with_capacity(MAX_LAYERS); let mut al = n;
                for k in 0..MAX_LAYERS { al -= hh[k]; out.push(al as f64 / n.max(1) as f64); }
                out
            };
            let cov_u: usize = g.0.iter().sum();
            let u = frac_of(&g.0, cov_u);
            let l4 = frac_of(&g.1, g.1.iter().sum());
            let l8 = frac_of(&g.2, g.2.iter().sum());
            let certify = |l: &[f64]| -> Option<usize> {
                // walk the rule with both bounds: certain while U and L agree on each side
                let mut valid_lo = 1.0f64; let mut valid_hi = 1.0f64; let mut r = 0usize;
                let lag = 0usize; let thr = 0.001f64; let cap = 20usize;
                loop {
                    r += 1;
                    if r >= lag + 1 { let j = r - 1 - lag; valid_lo = l.get(j).copied().unwrap_or(0.0); valid_hi = u.get(j).copied().unwrap_or(0.0); }
                    if valid_hi < thr { return Some(r); }
                    if valid_lo < thr { return None; } // ambiguous: L below, U above
                    if r >= cap { return Some(r); }
                }
            };
            let c4 = certify(&l4); let c8 = certify(&l8);
            eprintln!("bound stats: frame {}×{}: exact rendered {rendered} (fractions {:?}); U {:?}; certified with census/4 {:?}, census/8 {:?}", res, res_y, fractions.iter().take(rendered + 1).map(|f| format!("{f:.4}")).collect::<Vec<_>>(), u.iter().take(rendered + 1).map(|f| format!("{f:.4}")).collect::<Vec<_>>(), c4, c8);
            BOUND_TOTALS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if c4 == Some(rendered) { BOUND_TOTALS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } else if c4.is_some() { BOUND_TOTALS[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
            if c8 == Some(rendered) { BOUND_TOTALS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } else if c8.is_some() { BOUND_TOTALS[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
        }
        (rendered, fractions)
    });
    let parts: Vec<Vec<Vec<(u32, Frag)>>> = parts_all.into_iter().map(|(o, _, _)| o).collect();
    prof::add(&prof::B_RASTER, t_raster);
    if raster_stats { eprintln!("raster stats (sparse, {n_bands} bands): {} triangles rasterised, {} bbox pixels tested, {} pixel visits, clip {:?}, wanted {}, {:.3}s", RS_TRIS.swap(0, std::sync::atomic::Ordering::Relaxed), RS_TESTED.swap(0, std::sync::atomic::Ordering::Relaxed), RS_VISITS.swap(0, std::sync::atomic::Ordering::Relaxed), clip, px.len(), t_raster.elapsed().as_secs_f32()); }
    let t_sort = std::time::Instant::now();
    let npx = px.len();
    let bucket_results: Vec<(Vec<u32>, Vec<Frag>)> = crate::pool::pool().map(sparse_buckets as usize, |b| {
                let b = b as u32;
                let parts = &parts;
                {
                    let k0 = (b * sparse_bucket_size) as usize;
                    let k1 = ((b + 1) * sparse_bucket_size).min(npx as u32) as usize;
                    let nb = k1.saturating_sub(k0);
                    let mut count = vec![0u32; nb + 1];
                    for part in parts {
                        for (k, _) in &part[b as usize] {
                            count[*k as usize - k0 + 1] += 1;
                        }
                    }
                    for i in 0..nb {
                        count[i + 1] += count[i];
                    }
                    let total = count[nb] as usize;
                    let mut frags = vec![Frag { z: 0.0, tri: 0 }; total];
                    let mut fill = count.clone();
                    for part in parts {
                        for (k, f) in &part[b as usize] {
                            let i = fill[*k as usize - k0] as usize;
                            frags[i] = *f;
                            fill[*k as usize - k0] += 1;
                        }
                    }
                    for i in 0..nb {
                        let (a, c) = (count[i] as usize, count[i + 1] as usize);
                        if c - a > 1 {
                            frags[a..c].sort_by(|p, q| p.z.partial_cmp(&q.z).unwrap_or(std::cmp::Ordering::Equal));
                        }
                    }
                    (count, frags)
                }
    });
    // the buckets concatenated in parallel: a prefix over the buckets' totals, each bucket writes its ranks
    // and copies its fragments into its slice
    let total: usize = bucket_results.iter().map(|b| b.1.len()).sum();
    let mut bucket_base: Vec<u32> = Vec::with_capacity(bucket_results.len() + 1);
    bucket_base.push(0);
    for (_, f) in &bucket_results { let last = *bucket_base.last().unwrap(); bucket_base.push(last + f.len() as u32); }
    let mut start: Vec<u32> = U32S.take_with_capacity(npx + 1);
    let mut frags: Vec<Frag> = ABUF_FRAGS.take_with_capacity(total);
    // SAFETY: every slot is written exactly once below by the bucket that owns it
    unsafe { start.set_len(npx + 1); frags.set_len(total); }
    start[npx] = total as u32;
    if npx > 0 { start[0] = 0; }
    let (sp, fp) = (start.as_mut_ptr() as usize, frags.as_mut_ptr() as usize);
    {
        let bucket_results = &bucket_results;
        let bucket_base = &bucket_base;
        crate::pool::pool().run(bucket_results.len(), |b| {
            let (count, f) = &bucket_results[b];
            let k0 = (b as u32 * sparse_bucket_size) as usize;
            let base = bucket_base[b];
            // count[i] = the offset of pixel k0 + i within the bucket (count[0] = 0); the global start of pixel
            // k0 + i = base + count[i], written for i in 0..nb (pixel npx's entry is the total)
            let nb = count.len() - 1;
            for i in 0..nb {
                unsafe { *(sp as *mut u32).add(k0 + i) = base + count[i]; }
            }
            unsafe { std::ptr::copy_nonoverlapping(f.as_ptr(), (fp as *mut Frag).add(base as usize), f.len()); }
        });
    }
    prof::add(&prof::B_SORT, t_sort);
    (ABuffer { res, band_h: res_y.max(1), bands: vec![(start, frags)], sparse: Some(px.clone()) }, counted)
}

pub fn build_abuffer_wanted(tris: &[WTri], frame: &PeelFrame, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask], wanted: Option<&std::sync::Arc<PixelIndex>>) -> ABuffer {
    if let Some(px) = wanted {
        return build_abuffer_sparse(tris, frame, threads, zmin, zmax, masks, px);
    }
    let res = frame.res;
    let res_y = frame.res_y;
    let t_clip = std::time::Instant::now();
    // the wanted pixels' bounding rectangle: the raster visits nothing outside it
    let clip: (i32, i32, i32, i32) = match wanted {
        Some(px) => px.bbox,
        None => (0, 0, res as i32 - 1, res_y as i32 - 1),
    };
    prof::add(&prof::B_CLIP, t_clip);
    let t_raster = std::time::Instant::now();
    let raster_stats = raster_stats_on();
    let bitmap: Option<&[u64]> = wanted.map(|p| p.words.as_slice());
    // the sparse form's buckets: ranges of the dense index (one CSR builder thread per bucket)
    let sparse_buckets = 32u32;
    let sparse_bucket_size = wanted.map(|p| ((p.len() as u32 + sparse_buckets - 1) / sparse_buckets).max(1)).unwrap_or(1);
    // LMTOOL_PEEL_CULL_BACK=1 (hypothesis under test): the peel renders only the faces turned toward
    // the receivers' side (geometric normal against D); back faces are culled, not drawn black — so a
    // thin wall's own far face does not occlude its texels and a hollow tower sees out
    let cull_back = std::env::var("LMTOOL_PEEL_CULL_BACK").map(|v| v == "1").unwrap_or(false);
    let cards_occlude = cards_occlude();
    let d = frame.d;
    let inset = frame.inset_px;
    let bands = 128u32.min(res_y);
    let band_h = (res_y + bands - 1) / bands;
    // small chunks so the few huge decoration triangles (each covering the whole frame) spread over
    // the threads; the per-chunk overhead is a band vector set
    let chunk = if wanted.is_some() { (tris.len() / threads.max(1)).max(64) } else { (tris.len() / (threads.max(1) * 4)).max(256) };
    // parts[thread][band] = (pixel, frag)
    let parts: Vec<Vec<Vec<(u32, Frag)>>> = std::thread::scope(|sc| {
        let hs: Vec<_> = tris
            .chunks(chunk)
            .enumerate()
            .map(|(ci, ch)| {
                let frame = frame.clone();
                sc.spawn(move || {
                    let t_start = t_raster.elapsed().as_secs_f32();
                    let mut out: Vec<Vec<(u32, Frag)>> = (0..bands.max(if wanted.is_some() { sparse_buckets } else { 0 })).map(|_| Vec::new()).collect();
                    for (k, t) in ch.iter().enumerate() {
                        let ti = (ci * chunk + k) as u32;
                        let p0 = t.p0;
                        let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                        let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                        let (mut x0, mut y0, z0) = frame.project(p0);
                        let (mut x1, mut y1, z1) = frame.project(p1);
                        let (mut x2, mut y2, z2) = frame.project(p2);
                        // LMTOOL_PEEL_SNAP=1: the GPU's fixed-point vertex snap (1/256 px, ties to even) before the edge tests (a probe
                        // of the item raster's edge coverage against the captured layers)
                        if *PEEL_SNAP {
                            let sn = |v: f32| { let s = v * 256.0; let r = s.round(); let r = if (s - s.trunc()).abs() == 0.5 && (r as i64) % 2 != 0 { r - s.signum() } else { r }; r / 256.0 };
                            x0 = sn(x0); y0 = sn(y0); x1 = sn(x1); y1 = sn(y1); x2 = sn(x2); y2 = sn(y2);
                        }
                        if z0.min(z1).min(z2) >= zmax || z0.max(z1).max(z2) < zmin {
                            continue;
                        }
                        if cull_back && t.inst != DECOR_INST {
                            let ng = cross(t.e1, t.e2);
                            if dot(ng, d) > 0.0 {
                                continue;
                            }
                        }
                        if !cards_occlude && t.alpha != u16::MAX {
                            continue;
                        }
                        let mask = if t.alpha != u16::MAX { masks.get(t.alpha as usize) } else { None };
                        // THE FILTERED ALPHA TEST (PS 17134: the material's opacity texture sampled at TexCoord0 with
                        // the material sampler, alpha ≥ GbxShadowAlphaThreshold = 128/255): the triangle's uv footprint
                        // per pixel gives the level of detail (alphatex::Footprint), the sample is trilinear / anisotropic
                        // (the footprint is computed on the first fragment that needs it: most leaf triangles cover
                // no pixel centre at all)
                let mut fp_tex: Option<Option<(&crate::alphatex::AlphaTex, crate::alphatex::TapPlan)>> = None;
                        raster::triangle(res, res_y, [[x0, y0], [x1, y1], [x2, y2]], |x, y, b| {
                            // the game's viewport (1, 1, w−2, h−2): the outer ring is never drawn
                            if x < inset || y < inset || x + inset >= res || y + inset >= res_y {
                                return;
                            }
                            let z = z0 * b[0] + z1 * b[1] + z2 * b[2];
                            if z < zmax && z >= zmin {
                                // the alpha test: the cut-out texture at the fragment's TexCoord0
                                if let Some(m) = mask {
                                    let u = t.uv0[0][0] * b[0] + t.uv0[1][0] * b[1] + t.uv0[2][0] * b[2];
                                    let v = t.uv0[0][1] * b[0] + t.uv0[1][1] * b[1] + t.uv0[2][1] * b[2];
                                    let fp = fp_tex.get_or_insert_with(|| m.tex.as_ref().map(|tx| (tx.as_ref(), tx.plan(&crate::alphatex::Footprint::of_triangle([[x0, y0], [x1, y1], [x2, y2]], t.uv0, tx.w(), tx.h()), *ALPHA_ANISO))));
                                    let op = match fp {
                                        Some((tx, fp)) if !*ALPHA_POINT => tx.passes_planned(u, v, fp, ALPHA_THRESHOLD, crate::alphatex::Address::ClampEdge),
                                        _ => m.opaque(u, v),
                                    };
                                    if CARD_DUMP.is_some() {
                                        // the footprint's derivatives (the TapPlan no longer carries them): recomputed for the dump only
                                        let (fdx, fdy) = fp.as_ref().map(|(tx, _)| { let f = crate::alphatex::Footprint::of_triangle([[x0, y0], [x1, y1], [x2, y2]], t.uv0, tx.w(), tx.h()); (f.dx, f.dy) }).unwrap_or(([0.0; 2], [0.0; 2]));
                                        let z01 = frame.z01(z);
                                        let (slope, zmax_prim) = tri_slope(t, &frame);
                                        let dd = z01 + d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), CARD_DUMP_BIAS.0, CARD_DUMP_BIAS.1);
                                        let zq = if CARD_DUMP_BIAS.1 == 16 { (dd.clamp(0.0, 1.0) * 65535.0).round() / 65535.0 } else { dd };
                                        CARD_FRAGS.lock().unwrap().push(CardFrag { x, y, z01, tri: ti, u, v, mask: t.alpha as u32, fp_dx: fdx, fp_dy: fdy, port_pass: op as u32, zq });
                                    }
                                    if let Some((dx, dy)) = *ABUF_DEBUG { if x == dx && y == dy { eprintln!("abuf debug ({x},{y}): card tri {ti} inst {} model tri {} mask {} uv ({u:.4},{v:.4}) opaque {op} z {z:.3} z01 {:.5}", t.inst, t.tri, t.alpha, frame.z01(z)); } }
                                    if !op {
                                        return;
                                    }
                                } else if let Some((dx, dy)) = *ABUF_DEBUG { if x == dx && y == dy { eprintln!("abuf debug ({x},{y}): tri {ti} inst {} model tri {} z {z:.3} z01 {:.5}", t.inst, t.tri, frame.z01(z)); } }
                                out[(y / band_h) as usize].push((y * res + x, Frag { z, tri: ti }));
                            }
                        });
                    }
                    if raster_stats && ci < 2 { eprintln!("raster: worker {ci} started at {t_start:.3}s, done at {:.3}s ({} triangles)", t_raster.elapsed().as_secs_f32(), ch.len()); }
                    out
                })
            })
            .collect();
        if raster_stats { eprintln!("raster: {} threads spawned at {:.3}s", hs.len(), t_raster.elapsed().as_secs_f32()); }
        let r: Vec<Vec<Vec<(u32, Frag)>>> = hs.into_iter().map(|h| h.join().unwrap()).collect();
        if raster_stats { eprintln!("raster: joined at {:.3}s", t_raster.elapsed().as_secs_f32()); }
        r
    });
    prof::add(&prof::B_RASTER, t_raster);
    if raster_stats { eprintln!("raster stats: {} triangles rasterised, {} pixel visits, clip {:?}, wanted {}, {:.3}s", RS_TRIS.swap(0, std::sync::atomic::Ordering::Relaxed), RS_VISITS.swap(0, std::sync::atomic::Ordering::Relaxed), clip, wanted.map(|p| p.len()).unwrap_or(0), t_raster.elapsed().as_secs_f32()); }
    let t_sort = std::time::Instant::now();
    if let Some(px) = wanted {
        // SPARSE: per bucket of the dense index (in parallel) a counting sort by index and the depth sort
        // per pixel — the same fragments per pixel in the same order as the dense form; the buckets are
        // then stitched into one CSR (a million cells)
        let npx = px.len();
        let bucket_results: Vec<(Vec<u32>, Vec<Frag>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..sparse_buckets)
                .map(|b| {
                    let parts = &parts;
                    sc.spawn(move || {
                        let k0 = (b * sparse_bucket_size) as usize;
                        let k1 = ((b + 1) * sparse_bucket_size).min(npx as u32) as usize;
                        let nb = k1.saturating_sub(k0);
                        let mut count = vec![0u32; nb + 1];
                        for part in parts {
                            for (k, _) in &part[b as usize] {
                                count[*k as usize - k0 + 1] += 1;
                            }
                        }
                        for i in 0..nb {
                            count[i + 1] += count[i];
                        }
                        let total = count[nb] as usize;
                        let mut frags = vec![Frag { z: 0.0, tri: 0 }; total];
                        let mut fill = count.clone();
                        for part in parts {
                            for (k, f) in &part[b as usize] {
                                let i = fill[*k as usize - k0] as usize;
                                frags[i] = *f;
                                fill[*k as usize - k0] += 1;
                            }
                        }
                        for i in 0..nb {
                            let (a, c) = (count[i] as usize, count[i + 1] as usize);
                            if c - a > 1 {
                                frags[a..c].sort_by(|p, q| p.z.partial_cmp(&q.z).unwrap_or(std::cmp::Ordering::Equal));
                            }
                        }
                        (count, frags)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut start = Vec::with_capacity(npx + 1);
        let mut frags = Vec::with_capacity(bucket_results.iter().map(|b| b.1.len()).sum());
        start.push(0u32);
        for (count, f) in bucket_results {
            let base = *start.last().unwrap();
            for c in count.iter().skip(1) {
                start.push(base + c);
            }
            frags.extend(f);
        }
        prof::add(&prof::B_SORT, t_sort);
        return ABuffer { res, band_h: res_y.max(1), bands: vec![(start, frags)], sparse: Some(px.clone()) };
    }
    // per band: counting sort by pixel + depth sort, in parallel
    let n = (res * res_y) as usize;
    let band_results: Vec<(Vec<u32>, Vec<Frag>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..bands)
            .map(|b| {
                let parts = &parts;
                sc.spawn(move || {
                    let y0 = b * band_h;
                    let y1 = ((b + 1) * band_h).min(res_y);
                    let npx = ((y1 - y0) * res) as usize;
                    let base = (y0 * res) as usize;
                    let mut count = vec![0u32; npx + 1];
                    for part in parts {
                        for (px, _) in &part[b as usize] {
                            count[*px as usize - base + 1] += 1;
                        }
                    }
                    for i in 0..npx {
                        count[i + 1] += count[i];
                    }
                    let total = count[npx] as usize;
                    let mut frags = vec![Frag { z: 0.0, tri: 0 }; total];
                    let mut fill = count.clone();
                    for part in parts {
                        for (px, f) in &part[b as usize] {
                            let i = fill[*px as usize - base] as usize;
                            frags[i] = *f;
                            fill[*px as usize - base] += 1;
                        }
                    }
                    for i in 0..npx {
                        let (a, c) = (count[i] as usize, count[i + 1] as usize);
                        if c - a > 1 {
                            frags[a..c].sort_by(|p, q| p.z.partial_cmp(&q.z).unwrap_or(std::cmp::Ordering::Equal));
                        }
                    }
                    (count, frags)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let _ = n;
    ABuffer { res, band_h, bands: band_results, sparse: None }
}

/// A depth-only orthographic raster along the sun (the direct pass' shadow map).
pub struct ShadowMap {
    pub frame: PeelFrame,
    pub depth: raster::Depth,
}

impl ShadowMap {
    pub fn build(tris: &[WTri], sun_dir: V3, bmin: V3, bmax: V3, res: u32, masks: &[crate::geometry::AlphaMask]) -> ShadowMap {
        let frame = PeelFrame::new(sun_dir, bmin, bmax, res);
        Self::build_in(tris, frame, masks)
    }
    /// A shadow map rasterised in a given frame (a captured frustum, or the default fit).
    pub fn build_in(tris: &[WTri], frame: PeelFrame, masks: &[crate::geometry::AlphaMask]) -> ShadowMap {
        let (res, res_y) = (frame.res, frame.res_y);
        let mut depth = raster::Depth::new(res, res_y);
        for t in tris {
            let p0 = t.p0;
            let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
            let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
            let (x0, y0, z0) = frame.project(p0);
            let (x1, y1, z1) = frame.project(p1);
            let (x2, y2, z2) = frame.project(p2);
            if (!cards_occlude() || !cards_shadow()) && t.alpha != u16::MAX {
                continue;
            }
            let mask = if t.alpha != u16::MAX { masks.get(t.alpha as usize) } else { None };
            raster::triangle(res, res_y, [[x0, y0], [x1, y1], [x2, y2]], |x, y, b| {
                if let Some(m) = mask {
                    let u = t.uv0[0][0] * b[0] + t.uv0[1][0] * b[1] + t.uv0[2][0] * b[2];
                    let v = t.uv0[0][1] * b[0] + t.uv0[1][1] * b[1] + t.uv0[2][1] * b[2];
                    if !m.opaque(u, v) {
                        return;
                    }
                }
                let z = z0 * b[0] + z1 * b[1] + z2 * b[2];
                depth.test_write(x, y, z);
            });
        }
        ShadowMap { frame, depth }
    }
    /// 1 when the point sees the sun (its depth is not behind the map's by more than `bias` metres).
    #[inline]
    pub fn lit(&self, p: V3, bias: f32) -> f32 {
        let (x, y, z) = self.frame.project(p);
        let (xi, yi) = (x.round() as i64, y.round() as i64);
        if xi < 0 || yi < 0 || xi >= self.frame.res as i64 || yi >= self.frame.res_y as i64 {
            return 1.0;
        }
        let zm = self.depth.get(xi as u32, yi as u32);
        if z <= zm + bias { 1.0 } else { 0.0 }
    }
}

/// The world bounds of the RECEIVERS (the items' triangles — the decoration, which reaches ±96 km,
/// is left out: the peel frame must resolve the play area, and geometry beyond it is simply clipped),
/// padded.
pub fn scene_bounds(tris: &[WTri]) -> (V3, V3) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for t in tris.iter().filter(|t| t.inst != DECOR_INST) {
        for p in [t.p0, [t.p0[0] + t.e1[0], t.p0[1] + t.e1[1], t.p0[2] + t.e1[2]], [t.p0[0] + t.e2[0], t.p0[1] + t.e2[1], t.p0[2] + t.e2[2]]] {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    for k in 0..3 {
        lo[k] -= 1.0;
        hi[k] += 1.0;
    }
    (lo, hi)
}

/// The outgoing radiance of a peel fragment: albedo × (lightmap-so-far ÷ decode + sun), 0 for a back
/// face (the peel's camera at +D sees the face whose normal points toward +D).
fn fragment_radiance(scene: &Scene, bvh: &Bvh, prm: &BakeParams, shadow: Option<&ShadowMap>, frame: &PeelFrame, tri: u32, d: V3, hit_p: V3, sun_bias: f32) -> [f32; 3] {
    let wt = &bvh.tris[tri as usize];
    let ng = norm(cross(wt.e1, wt.e2));
    // The face that counts is the one turned TOWARD the receiving texel, i.e. whose normal points
    // against D (the light travels along −D from the occluder to the texel). A pad under a plate sees
    // the plate's underside — unlit by the sun and by the sky, hence black on the first sweep — which
    // is what the editor's bakes show (a pad under an 8 m plate keeps exactly its sky visibility, 51 %
    // of open, at Day); the face turned away from the texel is a back face → black.
    let facing = -dot(ng, d);
    let n = if facing >= 0.0 { ng } else { [-ng[0], -ng[1], -ng[2]] };
    // THE GAME'S FRONT FACE (PS 17131/17134: `and o0.xyz, rgb, isfrontface`; the rasteriser's
    // FrontCounterClockwise with NoCull): a triangle is front-facing when its vertices wind
    // counter-clockwise in NDC (y up), i.e. when its winding normal (p1 − p0) × (p2 − p0) points against
    // the camera's view direction D — read off the capture's dome triangles (the rendered far faces have
    // n_g·D < 0). The same test for every triangle, the zone tiles and the decoration included (for the
    // capture's upward directions the seabed's top is a back face → black, as the game's layer shows)
    let is_front = if prm.game_peel {
        dot(cross(wt.e1, wt.e2), d) < 0.0
    } else if wt.inst == DECOR_INST {
        true // the decoration is authored single-sided facing out; take it as front either way
    } else {
        // the mesh's own vertex normal decides the true front side
        let inst = &scene.instances[wt.inst as usize];
        let m = &scene.models[inst.model];
        match m.tris.get(wt.tri as usize) {
            Some(t) => {
                let vn = crate::geometry::xf_normal(&inst.xf, [t.n[0][0] + t.n[1][0] + t.n[2][0], t.n[0][1] + t.n[1][1] + t.n[2][1], t.n[0][2] + t.n[1][2] + t.n[2][2]]);
                dot(vn, d) <= 0.0
            }
            None => facing >= 0.0,
        }
    };
    if !is_front {
        return [0.0; 3];
    }
    // THE GAME'S COLOUR PATH (PS 17131/17134, `peelcolor`): the ILightInput atlas sampled at the fragment's LM
    // uv — TexCoord1 interpolated at the hit, through the instance's chart ST — with SGbxClamp_Aniso (one
    // bilinear tap at magnification, up to 16 along the footprint's major axis at minification), G ≥ 1e-5.
    // Runs when the harness holds the atlas (--ilightinput-from) and the instance a layout rect; the
    // decoration (the zone tiles: no rect in the port's layout) keeps the per-fragment model below.
    if let (Some(atlas), Some(rects), true) = (&prm.ilight_atlas, &prm.chart_rects, wt.inst != DECOR_INST) {
        if let Some(Some(rect)) = rects.get(wt.inst as usize) {
            let inst = &scene.instances[wt.inst as usize];
            let m = &scene.models[inst.model];
            if let Some(t) = m.tris.get(wt.tri as usize) {
                let st = crate::peelcolor::chart_st(*rect, m.plg_bounds.unwrap_or([0.0, 0.0, 1.0, 1.0]), atlas.w as f32);
                // the barycentrics of the hit in the world triangle (p0 + b1·e1 + b2·e2)
                let v = sub(hit_p, wt.p0);
                let (d00, d01, d11, d20, d21) = (dot(wt.e1, wt.e1), dot(wt.e1, wt.e2), dot(wt.e2, wt.e2), dot(v, wt.e1), dot(v, wt.e2));
                let den = d00 * d11 - d01 * d01;
                if den.abs() > 1e-18 {
                    let b1 = ((d11 * d20 - d01 * d21) / den).clamp(0.0, 1.0);
                    let b2 = ((d00 * d21 - d01 * d20) / den).clamp(0.0, 1.0);
                    let b0 = (1.0 - b1 - b2).max(0.0);
                    let uv1 = [t.uv[0][0] * b0 + t.uv[1][0] * b1 + t.uv[2][0] * b2, t.uv[0][1] * b0 + t.uv[1][1] * b1 + t.uv[2][1] * b2];
                    let uv_lm = [uv1[0] * st[0] + st[2], uv1[1] * st[1] + st[3]];
                    // the footprint of the LM uv per peel pixel: the triangle's uv Jacobian in atlas texels
                    let p1 = add(wt.p0, wt.e1);
                    let p2 = add(wt.p0, wt.e2);
                    let (x0, y0, _) = frame.project(wt.p0);
                    let (x1, y1, _) = frame.project(p1);
                    let (x2, y2, _) = frame.project(p2);
                    let uv_lm_of = |uv: [f32; 2]| [uv[0] * st[0] + st[2], uv[1] * st[1] + st[3]];
                    let fp = crate::alphatex::Footprint::of_triangle([[x0, y0], [x1, y1], [x2, y2]], [uv_lm_of(t.uv[0]), uv_lm_of(t.uv[1]), uv_lm_of(t.uv[2])], atlas.w, atlas.h);
                    let taps = fp.taps(16);
                    let axis = fp.major_axis();
                    return crate::peelcolor::peel_color(atlas, uv_lm, axis, taps, true);
                }
            }
        }
    }
    // the zone tiles (decoration quads on the 32 m grid): their chart ST per cell, the LM uv from the tile
    // mesh's TexCoord1 bounds (TILE_UV_BOUNDS), the footprint from the cell's projected corners
    if let (Some(atlas), Some(tst), true) = (&prm.ilight_atlas, &prm.tile_st, wt.inst == DECOR_INST) {
        let is_tile = scene.decor.get(wt.tri as usize).map(|d| !d.env && !d.water).unwrap_or(false);
        if is_tile {
            let (cx, cz) = ((hit_p[0] / 32.0).floor(), (hit_p[2] / 32.0).floor());
            if cx >= 0.0 && cx < tst.grid as f32 && cz >= 0.0 && cz < tst.grid as f32 {
                if let Some(st) = tst.get(cx as i64, cz as i64) {
                    let b = tst.uv_bounds;
                    let uv1_of = |x: f32, z: f32| -> [f32; 2] { [b[0] + (x - cx * 32.0) / 32.0 * (b[2] - b[0]), b[3] - (z - cz * 32.0) / 32.0 * (b[3] - b[1])] };
                    let uv_lm_of = |uv: [f32; 2]| [uv[0] * st[0] + st[2], uv[1] * st[1] + st[3]];
                    let uv_lm = uv_lm_of(uv1_of(hit_p[0], hit_p[2]));
                    let y = hit_p[1];
                    let (x0, z0) = (cx * 32.0, cz * 32.0);
                    let c = [[x0, y, z0], [x0 + 32.0, y, z0], [x0, y, z0 + 32.0]];
                    let px: Vec<[f32; 2]> = c.iter().map(|q| { let (x, yy, _) = frame.project(*q); [x, yy] }).collect();
                    let fp = crate::alphatex::Footprint::of_triangle([px[0], px[1], px[2]], [uv_lm_of(uv1_of(c[0][0], c[0][2])), uv_lm_of(uv1_of(c[1][0], c[1][2])), uv_lm_of(uv1_of(c[2][0], c[2][2]))], atlas.w, atlas.h);
                    return crate::peelcolor::peel_color(atlas, uv_lm, fp.major_axis(), fp.taps(16), true);
                }
            }
        }
    }
    // THE GAME'S COLOUR (PS 17131 / 17134): TMapILightInput sampled at the fragment's lightmap coordinate — the
    // transcribed setup chain's atlas when the harness has it (`--ilightinput-from`); a fragment without a lightmap
    // coordinate (the environment block) falls through to the port's model
    if let Some(il) = &prm.ilatlas {
        if let Some(c) = il.colour(scene, wt, hit_p, true) {
            return c;
        }
    }
    let h = Hit { t: 0.0, tri };
    let alb = hit_albedo(scene, bvh, prm, &h);
    // the lightmap so far at this surface point (0 on the first sweep), read back ÷ bounce_decode
    let stored: [f32; 3] = match &prm.field {
        Some(f) if wt.inst != DECOR_INST => {
            let v = sub(hit_p, wt.p0);
            let (d00, d01, d11, d20, d21) = (dot(wt.e1, wt.e1), dot(wt.e1, wt.e2), dot(wt.e2, wt.e2), dot(v, wt.e1), dot(v, wt.e2));
            let den = d00 * d11 - d01 * d01;
            if den.abs() < 1e-12 {
                [0.0; 3]
            } else {
                let b1 = ((d11 * d20 - d01 * d21) / den).clamp(0.0, 1.0);
                let b2 = ((d00 * d21 - d01 * d20) / den).clamp(0.0, 1.0);
                f.lookup(scene, wt.inst, wt.tri, b1, b2).map(|e| [e[0] / prm.bounce_decode, e[1] / prm.bounce_decode, e[2] / prm.bounce_decode]).unwrap_or([0.0; 3])
            }
        }
        // the decoration (and any geometry without a lightmap) has no C0 of its own: the game's
        // GeomILightIn0 reads the ambient in its place — the mood's LAmbient (the undersides of the
        // tiny maps' ground-level items read 0.2 over the sea in the editor, our water gave 0.05 with
        // nothing but the low sun on it; DIFFERENTIAL: the factor is 1, --decor-ambient K overrides)
        // (nothing is accumulated yet in the first sweep: the stand-in is 0 there, like every C0)
        _ if wt.inst == DECOR_INST && (prm.sweep > 0 || prm.sweep0_sun) => { let s = prm.decor_sky_up; [s[0] * prm.decor_ambient, s[1] * prm.decor_ambient, s[2] * prm.decor_ambient] },
        _ => [0.0; 3],
    };
    // a vegetation card is lit from both sides (thin foliage: the leaf shader's sun term does not care
    // which face the peel sees) — LMTOOL_CARD_ONE_SIDED=1 restores the plain n·L
    let is_card = wt.alpha != u16::MAX;
    let ndl = if is_card && !prm.card_one_sided { dot(n, prm.sun_dir).abs() } else { dot(n, prm.sun_dir).max(0.0) };
    // THE CAPTURE (2026-09-24): the first sweep's peel colours are black — no sun on the peeled surfaces
    // in sweep 0 (the sun's bounce enters with the stored C0 from sweep 1 on); --sweep0-sun restores the
    // RE reading of FUN_140234df0 (sun-visibility scale 1.0 in the first sweep)
    let sun_on = prm.sweep > 0 || prm.sweep0_sun;
    let lit = if sun_on && ndl > 0.0 && prm.sun_dir[1] > 0.0 { shadow.map(|s| s.lit(hit_p, sun_bias)).unwrap_or(1.0) } else { 0.0 };
    // (the counters are three atomics every thread hammers — one contended cache line per fragment;
    // they are kept only under LMTOOL_SUN_STATS=1)
    if sun_stats_on() {
        SUN_STATS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if ndl > 0.0 { SUN_STATS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
        if lit > 0.0 { SUN_STATS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
    }
    let mut out = [0f32; 3];
    for k in 0..3 {
        out[k] = alb[k] * (stored[k] + prm.sun[k] * ndl * lit);
    }
    // a water surface mirrors the sky: the reflected direction's sky colour × the reflectance
    // (--water-reflect K, default 0.5; the sea under a sunset glow lights the faces that look at it)
    if wt.inst == DECOR_INST && prm.water_reflect > 0.0 {
        if let Some(dt) = scene.decor.get(wt.tri as usize) {
            if dt.water {
                // the light arrives along −D at the texel, i.e. it left the water travelling along −D; it came
                // from the sky direction r = reflect(−D about the water normal (up)) = (−D) − 2(−D·n)n
                // (d points from the texel down onto the water; the sky ray that reflects into −d comes from
                // the direction (d.x, −d.y, d.z) seen from the water)
                let r = [d[0], -d[1], d[2]];
                if r[1] > 0.0 {
                    let s = sky_radiance(prm, r);
                    for k in 0..3 { out[k] += prm.water_reflect * s[k]; }
                    // the sun's glitter: a broad specular lobe of the sun on the sea (--water-sun K [--water-sun-pow P]);
                    // the editor's Sunset slopes facing the sea carry the sun's own colour (R:G:B 1:0.8:0.16)
                    if prm.water_sun > 0.0 && prm.sun_dir[1] > 0.0 {
                        let c = dot(r, prm.sun_dir).max(0.0);
                        let f = prm.water_sun * c.powf(prm.water_sun_pow);
                        for k in 0..3 { out[k] += f * prm.sun[k]; }
                    }
                }
            }
        }
    }
    out
}

/// Diagnostics: fragment radiance calls, of which facing the sun, of which lit.
pub static SUN_STATS: [std::sync::atomic::AtomicUsize; 3] = [std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0)];
fn sun_stats_on() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("LMTOOL_SUN_STATS").is_some())
}

/// One lightmap sub-sample awaiting its dome gather.
struct SubSample {
    p: V3,
    n: V3,
    /// The receiver's own world triangle (BVH index) — never its own occluder.
    own_tri: u32,
    /// Which interleave group (0..ss²) this sub-sample belongs to.
    group: u8,
    /// Index of the chart and of the colour texel it resolves into.
    chart: u32,
    texel: u32,
    /// Its pixel in the chart's supersampled raster (the dump's `chart_ss` space).
    sx: u32,
    sy: u32,
}

/// One peeled layer fragment of a pixel: the stored (biased) reversed depth and the layer's colour.
#[derive(Clone, Copy, Debug)]
pub struct LayerFrag {
    pub d: f32,
    pub rgb: [f32; 3],
}

/// The game's depth-peel layers of one direction: per pixel the layers far-to-near (CSR), stored
/// depths biased like the D3D rasteriser stores them, colours as the peel colour target holds them.
pub struct Layers {
    pub w: u32,
    pub h: u32,
    pub start: Vec<u32>,
    pub frags: Vec<LayerFrag>,
    pub max_layers: usize,
    /// The sparse form: `start` indexed by the wanted pixels' dense index.
    pub sparse: Option<std::sync::Arc<PixelIndex>>,
    /// How many ITEM layers this peel rendered (the game's stop rule or the captured count).
    pub item_layers: usize,
    /// The written fraction of every candidate item layer (index k = the k-th item layer).
    pub fractions: Vec<f64>,
}

impl Layers {
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> &[LayerFrag] {
        if let Some(px) = &self.sparse {
            let Some(k) = px.index(x, y) else { return &[] };
            return &self.frags[self.start[k as usize] as usize..self.start[k as usize + 1] as usize];
        }
        let i = (y * self.w + x) as usize;
        &self.frags[self.start[i] as usize..self.start[i + 1] as usize]
    }
    /// Layer `k`'s depth image as the game's target holds it after the layer render: the stored
    /// (biased, quantised) depth where the pixel has that layer, the CLEAR (1.0 = the near plane, so
    /// that the LESS test lets the farthest fragment through) elsewhere. `k` counts the REAL layers —
    /// the synthetic dome layer (`skip` = 1) is not a render target of the game's.
    pub fn depth_image(&self, k: usize, skip: usize) -> Vec<f32> {
        self.layer_image(k + skip, 1.0, |f| f.d)
    }
    /// Layer `k`'s colour image (the clear = black where the pixel has fewer layers).
    pub fn colour_image(&self, k: usize, skip: usize) -> Vec<[f32; 3]> {
        self.layer_image(k + skip, [0.0; 3], |f| f.rgb)
    }
    /// `colour_image` as a flat RGB buffer (3 floats per pixel — a `passdiff::Buf`'s data), built in parallel.
    pub fn colour_buf(&self, k: usize, skip: usize) -> crate::passdiff::Buf {
        let img = self.colour_image(k, skip);
        // the [f32; 3] array is three contiguous floats: reinterpret without a copy per element
        let n = img.len();
        let mut img = std::mem::ManuallyDrop::new(img);
        let data: Vec<f32> = unsafe { Vec::from_raw_parts(img.as_mut_ptr() as *mut f32, n * 3, img.capacity() * 3) };
        crate::passdiff::Buf { w: self.w, h: self.h, channels: 3, data }
    }
    /// A per-pixel image of layer `k` (`clear` where a pixel has fewer layers), built in parallel: the dense
    /// form by pixel chunks, the sparse form as the clear everywhere then the wanted pixels' values.
    fn layer_image<T: Copy + Send + Sync>(&self, k: usize, clear: T, get: impl Fn(&LayerFrag) -> T + Sync) -> Vec<T> {
        let n = (self.w * self.h) as usize;
        let mut img: Vec<T> = Vec::with_capacity(n);
        // SAFETY: every element is written below (the dense form directly, the sparse form by the fill then the scatter)
        unsafe { img.set_len(n); }
        let ip = img.as_mut_ptr() as usize;
        let threads = crate::pool::pool().threads.max(1);
        match &self.sparse {
            None => {
                let chunk = (n / (threads * 4).max(1)).max(4096);
                crate::pool::pool().run((n + chunk - 1) / chunk, |ci| {
                    for i in ci * chunk..((ci + 1) * chunk).min(n) {
                        let (a, c) = (self.start[i] as usize, self.start[i + 1] as usize);
                        unsafe { *(ip as *mut T).add(i) = if a + k < c { get(&self.frags[a + k]) } else { clear }; }
                    }
                });
            }
            Some(px) => {
                let chunk = (n / (threads * 4).max(1)).max(4096);
                crate::pool::pool().run((n + chunk - 1) / chunk, |ci| {
                    for i in ci * chunk..((ci + 1) * chunk).min(n) { unsafe { *(ip as *mut T).add(i) = clear; } }
                });
                let m = px.pixels.len();
                let chunk = (m / (threads * 4).max(1)).max(1024);
                crate::pool::pool().run((m + chunk - 1) / chunk, |ci| {
                    for r in ci * chunk..((ci + 1) * chunk).min(m) {
                        let (a, c) = (self.start[r] as usize, self.start[r + 1] as usize);
                        if a + k < c { unsafe { *(ip as *mut T).add(px.pixels[r] as usize) = get(&self.frags[a + k]); } }
                    }
                });
            }
        }
        img
    }
}

/// The D3D11 rasteriser depth bias of a fragment on a D32_FLOAT target: `DepthBias · 2^(exponent(max
/// z01 of the primitive) − 23) + SlopeScaledDepthBias · max(|∂z01/∂x|, |∂z01/∂y|)` (per pixel step).
pub fn d3d_depth_bias(zmax_prim: f32, slope: f32, bias: (i32, f32)) -> f32 {
    let e = if zmax_prim > 0.0 { zmax_prim.log2().floor() } else { -126.0 };
    bias.0 as f32 * 2f32.powf(e - 23.0) + bias.1 * slope
}

/// `d3d_depth_bias` for the target's format: a UNORM depth buffer's unit is its smallest step
/// (D16: 1/65535), a float one's the ulp of the primitive's maximum depth.
pub fn d3d_depth_bias_fmt(zmax_prim: f32, slope: f32, bias: (i32, f32), bits: u32) -> f32 {
    if bits == 16 {
        bias.0 as f32 / 65535.0 + bias.1 * slope
    } else {
        d3d_depth_bias(zmax_prim, slope, bias)
    }
}

/// The depth gradient (per pixel) of a world triangle in a frame, in z01 units.
fn tri_slope(wt: &WTri, frame: &PeelFrame) -> (f32, f32) {
    let p1 = [wt.p0[0] + wt.e1[0], wt.p0[1] + wt.e1[1], wt.p0[2] + wt.e1[2]];
    let p2 = [wt.p0[0] + wt.e2[0], wt.p0[1] + wt.e2[1], wt.p0[2] + wt.e2[2]];
    tri_slope_projected(frame, frame.project(wt.p0), frame.project(p1), frame.project(p2))
}

/// `tri_slope` from the vertices already projected into the frame (the raster projects them anyway; the
/// same values, so the same result).
fn tri_slope_projected(frame: &PeelFrame, (x0, y0, z0): (f32, f32, f32), (x1, y1, z1): (f32, f32, f32), (x2, y2, z2): (f32, f32, f32)) -> (f32, f32) {
    let (z0, z1, z2) = (frame.z01(z0), frame.z01(z1), frame.z01(z2));
    let det = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    if det.abs() < 1e-12 {
        return (f32::INFINITY, z0.max(z1).max(z2));
    }
    let dzdx = ((z1 - z0) * (y2 - y0) - (z2 - z0) * (y1 - y0)) / det;
    let dzdy = ((z2 - z0) * (x1 - x0) - (z1 - z0) * (x2 - x0)) / det;
    (dzdx.abs().max(dzdy.abs()), z0.max(z1).max(z2))
}

/// Peel the A-buffer into the game's layers (`RenderLightIndirectPeel`): layer 0 = the dome (when
/// `prm.dome_layer`), then far-to-near, each layer keeping the farthest fragment nearer than the
/// previous layer's STORED depth by at least the rasteriser bias (fragments within the bias of the
/// previous layer are never rendered again — merged), at most `MAX_LAYERS` item layers. The colour is
/// the fragment's `ILightInput` radiance at the pixel centre, quantised as the colour target stores it.
///
/// THE LAYER COUNT (0x140234df0 l.413–437, the capture's per-layer pixel counts — `lmtool peel-layers`):
/// the game renders item layers until the pixel-count query of a layer reports fewer than 0.1 % of the
/// viewport's pixels written, read `lag` layers late (the query is polled without waiting), or until
/// 20 item layers — a GLOBAL rule per peel, applied here after the per-pixel peel: every pixel's list is
/// cut to the number of item layers the game renders (`fixed_layers` = the captured count when the
/// harness has it, else `prm.peel_stop`). An empty layer still counts as rendered (the capture's first
/// direction ran 18 empty layers to the cap while its query never answered).
fn extract_layers(ab: &ABuffer, frame: &PeelFrame, scene: &Scene, bvh: &Bvh, prm: &BakeParams, shadow: Option<&ShadowMap>, sun_bias: f32, sky: [f32; 3], threads: usize, wanted: Option<&std::sync::Arc<PixelIndex>>, fixed_layers: Option<usize>, dome_img: Option<&[[f32; 3]]>) -> Layers {
    let (w, h) = (frame.res, frame.res_y);
    let n = (w * h) as usize;
    // THE CAPTURE (pwc6 frame 7534, the first sweep-1 direction's layer 0): the environment render of a LATER sweep is
    // BLACK — the sky enters the accumulation once, in sweep 0 (the sweep-1 layer-0 snapshots hold 0 at every dome
    // pixel; the geometry layers carry the bounce ILightInput)
    let sky_q = if prm.sweep > 0 { [0.0f32; 3] } else { prm.quant_peel.apply(sky, prm.rounding) };
    let dome_img = if prm.sweep > 0 { None } else { dome_img };
    let skip_n = if prm.dome_layer { 1usize } else { 0 };
    // one pixel's layers appended to `out`
    let derive_pixel = |x: usize, y: usize, out: &mut Vec<LayerFrag>| {
        let list = ab.at_all(x as u32, y as u32);
        let before = out.len();
        let mut d_prev = f32::NEG_INFINITY;
        // THE ENVIRONMENT LAYER (the game's first render of every peel: the sea box and the
        // terrain over the cleared depth 0, nearest wins, then the sky dome at depth 0 where
        // nothing else was drawn): black where an environment surface sits, the dome elsewhere
        let is_env = |tri: u32| -> bool { let wt = &bvh.tris[tri as usize]; wt.inst == DECOR_INST && scene.decor.get(wt.tri as usize).map(|d| d.env).unwrap_or(false) };
        // a sea-box face is drawn only when it is a FAR face for this view (outward normal along D)
        let env_drawn = |tri: u32| -> bool {
            let wt = &bvh.tris[tri as usize];
            match scene.decor.get(wt.tri as usize) {
                Some(dt) if dt.env_far_only => { let n = cross(wt.e1, wt.e2); dot(n, frame.d) > 0.0 }
                _ => true,
            }
        };
        if prm.dome_layer {
            let mut env_d = 0.0f32;
            for f in list {
                if is_env(f.tri) && env_drawn(f.tri) {
                    let z01 = frame.z01(f.z);
                    if z01 >= 0.0 && z01 <= 1.0 { env_d = env_d.max(z01); }
                }
            }
            if env_d > 0.0 {
                let d = if prm.depth_bits == 16 { (env_d * 65535.0).round() / 65535.0 } else { env_d };
                out.push(LayerFrag { d, rgb: [0.0; 3] });
                d_prev = d;
            } else {
                // the environment render's dome pixel: the transcribed dome per pixel when given, else the direction's uniform sky
                out.push(LayerFrag { d: 0.0, rgb: dome_img.map(|img| img[y * w as usize + x]).unwrap_or(sky_q) });
                d_prev = 0.0;
            }
        }
        for f in list {
            // (the environment is not re-drawn in the geometry layers; in a sweep without an environment block
            // it is not drawn at all)
            if (prm.dome_layer || !prm.env_in_peel) && is_env(f.tri) {
                continue;
            }
            // pancaking: a fragment beyond the far plane lands on it (z01 = 0)
            let z01 = frame.z01(f.z).max(0.0);
            if z01 < d_prev {
                continue;
            }
            if out.len() - before - skip_n >= MAX_LAYERS {
                break;
            }
            let wt = &bvh.tris[f.tri as usize];
            let (slope, zmax_prim) = tri_slope(wt, frame);
            // an edge-on triangle's slope is huge (D3D applies it uncapped, DepthBiasClamp 0)
            let mut d = z01 + d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), prm.depth_bias, prm.depth_bits);
            if prm.depth_bits == 16 {
                // the D16_UNORM target stores 65535 steps
                d = (d.clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
            }
            let hit_p = frame.unproject(x as f32 + 0.5, y as f32 + 0.5, f.z);
            let rgb = prm.quant_peel.apply(fragment_radiance(scene, bvh, prm, shadow, frame, f.tri, frame.d, hit_p, sun_bias), prm.rounding);
            out.push(LayerFrag { d, rgb });
            d_prev = d;
        }
    };
    if let Some(px) = wanted {
        // SPARSE: the wanted pixels only, in parallel chunks of the dense index
        let npx = px.len();
        let chunk = (npx / threads.max(1)).max(2048);
        let n_chunks = (npx + chunk - 1) / chunk;
        let t_par = std::time::Instant::now();
        let parts: Vec<(Vec<u32>, Vec<LayerFrag>)> = crate::pool::pool().map(n_chunks, |ci| {
            let ids = &px.pixels[ci * chunk..((ci + 1) * chunk).min(npx)];
            let mut counts = Vec::with_capacity(ids.len());
            let mut out: Vec<LayerFrag> = Vec::new();
            for &id in ids {
                let before = out.len();
                derive_pixel((id % w) as usize, (id / w) as usize, &mut out);
                counts.push((out.len() - before) as u32);
            }
            (counts, out)
        });
        prof::add(&prof::L_PAR, t_par);
        // THE LAYER COUNT on the sparse form: with a known count (captured, fixed, or the exact pass) nothing
        // is estimated; otherwise (--layers-estimate) the written fraction per item layer is estimated on the
        // CENSUS pixels (every CENSUS_STEP-th pixel in x and y inside the texels' rectangle — the wanted pixels
        // alone sit where the items are and would overstate the fractions; the census positions outside the
        // rectangle hold no item layer and count as 0-layer pixels)
        let mut hist = vec![0usize; MAX_LAYERS + 1];
        let mut n_census = 0usize;
        if fixed_layers.is_none() {
            let mut ci_all = 0usize;
            for (counts, _) in &parts {
                for c in counts {
                    let id = px.pixels[ci_all];
                    ci_all += 1;
                    let (x, y) = (id % w, id / w);
                    if x % CENSUS_STEP == 0 && y % CENSUS_STEP == 0 {
                        n_census += 1;
                        let items = (*c as usize).saturating_sub(skip_n).min(MAX_LAYERS);
                        hist[items] += 1;
                    }
                }
            }
        }
        let n_census_total = if fixed_layers.is_none() { ((w as usize + CENSUS_STEP as usize - 1) / CENSUS_STEP as usize) * ((h as usize + CENSUS_STEP as usize - 1) / CENSUS_STEP as usize) } else { 0 };
        let n_census_total = n_census_total.max(n_census);
        hist[0] += n_census_total - n_census;
        let n_census = n_census_total;
        let mut fractions: Vec<f64> = Vec::with_capacity(MAX_LAYERS);
        let mut at_least = n_census;
        for k in 0..MAX_LAYERS {
            at_least -= hist[k];
            fractions.push(if n_census > 0 { at_least as f64 / n_census as f64 } else { 0.0 });
        }
        let kept = match fixed_layers {
            Some(k) => k.min(MAX_LAYERS),
            None => prm.peel_stop.layers_rendered_after(&fractions, skip_n),
        };
        let candidates = fractions.iter().take_while(|f| **f > 0.0).count();
        LAYER_STATS.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if kept < candidates {
            LAYER_STATS.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        if peel_layers_debug() && fixed_layers.is_none() {
            eprintln!("peel layers (sparse census, {n_census} census pixels): {} candidate item layers, fractions {:?} → {} rendered (the stop rule)", candidates, fractions.iter().take(candidates).map(|f| format!("{f:.6}")).collect::<Vec<_>>(), kept);
        }
        // THE CSR over the wanted pixels, assembled in parallel: per part the kept count of every pixel
        // (cut to skip_n + kept) and its total, a prefix over the parts, then every part copies its kept
        // fragments into its slice
        let cap = skip_n + kept;
        let part_totals: Vec<usize> = crate::pool::pool().map(parts.len(), |pi| parts[pi].0.iter().map(|c| (*c as usize).min(cap)).sum());
        let mut part_base = Vec::with_capacity(parts.len() + 1);
        part_base.push(0usize);
        for t in &part_totals { let last = *part_base.last().unwrap(); part_base.push(last + t); }
        let mut part_pix = Vec::with_capacity(parts.len() + 1);
        part_pix.push(0usize);
        for (counts, _) in &parts { let last = *part_pix.last().unwrap(); part_pix.push(last + counts.len()); }
        let total = *part_base.last().unwrap();
        let mut start: Vec<u32> = U32S.take_with_capacity(npx + 1);
        let mut frags: Vec<LayerFrag> = LAYER_FRAGS.take_with_capacity(total);
        // SAFETY: every slot of both vectors is written exactly once below, by the part that owns it
        unsafe { start.set_len(npx + 1); frags.set_len(total); }
        start[npx] = total as u32;
        let (sp, fp) = (start.as_mut_ptr() as usize, frags.as_mut_ptr() as usize);
        {
            let parts = &parts;
            let part_base = &part_base;
            let part_pix = &part_pix;
            crate::pool::pool().run(parts.len(), |pi| {
                let (counts, out) = &parts[pi];
                let mut o = 0usize;
                let mut dst = part_base[pi];
                let mut pix = part_pix[pi];
                for c in counts {
                    let c = *c as usize;
                    let keep = c.min(cap);
                    unsafe {
                        *(sp as *mut u32).add(pix) = dst as u32;
                        std::ptr::copy_nonoverlapping(out.as_ptr().add(o), (fp as *mut LayerFrag).add(dst), keep);
                    }
                    o += c;
                    dst += keep;
                    pix += 1;
                }
            });
        }
        return Layers { w, h, start, frags, max_layers: MAX_LAYERS, sparse: Some(px.clone()), item_layers: kept, fractions };
    }
    let rows_per = ((h as usize) / threads.max(1)).max(1);
    let parts: Vec<(Vec<u32>, Vec<LayerFrag>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..h as usize)
            .step_by(rows_per)
            .map(|y0| {
                let y1 = (y0 + rows_per).min(h as usize);
                let derive_pixel = &derive_pixel;
                sc.spawn(move || {
                    let mut counts = Vec::with_capacity((y1 - y0) * w as usize);
                    let mut out: Vec<LayerFrag> = Vec::new();
                    for y in y0..y1 {
                        for x in 0..w as usize {
                            let before = out.len();
                            derive_pixel(x, y, &mut out);
                            counts.push((out.len() - before) as u32);
                        }
                    }
                    (counts, out)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    // THE LAYER COUNT (engineer D, 0x140234df0): the written fraction of every candidate item layer over
    // the whole viewport, the game's stop rule (or the captured count) → every pixel's list is cut
    let mut hist = vec![0usize; MAX_LAYERS + 1];
    for (counts, _) in &parts {
        for c in counts {
            let items = (*c as usize).saturating_sub(skip_n).min(MAX_LAYERS);
            hist[items] += 1;
        }
    }
    let mut fractions: Vec<f64> = Vec::with_capacity(MAX_LAYERS);
    let mut at_least = n;
    for k in 0..MAX_LAYERS {
        at_least -= hist[k];
        fractions.push(at_least as f64 / n.max(1) as f64);
    }
    let kept = match fixed_layers {
        Some(k) => k.min(MAX_LAYERS),
        None => prm.peel_stop.layers_rendered_after(&fractions, skip_n),
    };
    let candidates = fractions.iter().take_while(|f| **f > 0.0).count();
    LAYER_STATS.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if kept < candidates {
        LAYER_STATS.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    if peel_layers_debug() {
        eprintln!("peel layers: {} candidate item layers, fractions {:?} → {} rendered ({})", candidates, fractions.iter().take(candidates).map(|f| format!("{f:.6}")).collect::<Vec<_>>(), kept, if fixed_layers.is_some() { "the captured count" } else { "the stop rule" });
    }
    let mut start = Vec::with_capacity(n + 1);
    let mut frags = Vec::with_capacity(parts.iter().map(|p| p.1.len()).sum());
    start.push(0u32);
    for (counts, out) in parts {
        let mut o = 0usize;
        for c in counts {
            let c = c as usize;
            let keep = c.min(skip_n + kept);
            frags.extend_from_slice(&out[o..o + keep]);
            o += c;
            let last = *start.last().unwrap();
            start.push(last + keep as u32);
        }
    }
    Layers { w, h, start, frags, max_layers: MAX_LAYERS, sparse: None, item_layers: kept, fractions }
}

/// THE EXACT LAYER-COUNT STATISTIC (the coordinator's requirement): over EVERY pixel of the viewport the
/// number of item layers the peel renders — the depth logic of `extract_layers` (env layer, bias, D16
/// quantisation, the cap) on a dense depth-only A-buffer, no colour, no lookups — then the game's stop
/// rule (`prm.peel_stop`, with its readback lag) on the exact written fractions. Cost: the dense raster of
/// the frame plus a per-pixel scan.
pub fn exact_item_layers(ab: &ABuffer, frame: &PeelFrame, scene: &Scene, bvh: &Bvh, prm: &BakeParams, threads: usize) -> (usize, Vec<f64>) {
    let (w, h) = (frame.res, frame.res_y);
    let n = (w * h) as usize;
    let skip_n = if prm.dome_layer { 1usize } else { 0 };
    let is_env = |tri: u32| -> bool { let wt = &bvh.tris[tri as usize]; wt.inst == DECOR_INST && scene.decor.get(wt.tri as usize).map(|d| d.env).unwrap_or(false) };
    let env_drawn = |tri: u32| -> bool {
        let wt = &bvh.tris[tri as usize];
        match scene.decor.get(wt.tri as usize) {
            Some(dt) if dt.env_far_only => { let n = cross(wt.e1, wt.e2); dot(n, frame.d) > 0.0 }
            _ => true,
        }
    };
    let count_pixel = |x: u32, y: u32| -> usize {
        let list = ab.at_all(x, y);
        let mut d_prev = f32::NEG_INFINITY;
        if prm.dome_layer {
            let mut env_d = 0.0f32;
            for f in list {
                if is_env(f.tri) && env_drawn(f.tri) {
                    let z01 = frame.z01(f.z);
                    if z01 >= 0.0 && z01 <= 1.0 { env_d = env_d.max(z01); }
                }
            }
            d_prev = if env_d > 0.0 { if prm.depth_bits == 16 { (env_d * 65535.0).round() / 65535.0 } else { env_d } } else { 0.0 };
        }
        let mut items = 0usize;
        for f in list {
            if prm.dome_layer && is_env(f.tri) {
                continue;
            }
            let z01 = frame.z01(f.z).max(0.0);
            if z01 < d_prev {
                continue;
            }
            if items >= MAX_LAYERS {
                break;
            }
            let wt = &bvh.tris[f.tri as usize];
            let (slope, zmax_prim) = tri_slope(wt, frame);
            let mut d = z01 + d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), prm.depth_bias, prm.depth_bits);
            if prm.depth_bits == 16 {
                d = (d.clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
            }
            items += 1;
            d_prev = d;
        }
        let _ = skip_n;
        items
    };
    let rows_per = ((h as usize) / (threads * 2).max(1)).max(1);
    let n_chunks = (h as usize + rows_per - 1) / rows_per;
    let hists: Vec<[usize; MAX_LAYERS + 1]> = crate::pool::pool().map(n_chunks, |ci| {
        let mut hist = [0usize; MAX_LAYERS + 1];
        for y in ci * rows_per..((ci + 1) * rows_per).min(h as usize) {
            for x in 0..w as usize {
                hist[count_pixel(x as u32, y as u32).min(MAX_LAYERS)] += 1;
            }
        }
        hist
    });
    let mut hist = [0usize; MAX_LAYERS + 1];
    for hh in &hists { for k in 0..=MAX_LAYERS { hist[k] += hh[k]; } }
    let mut fractions: Vec<f64> = Vec::with_capacity(MAX_LAYERS);
    let mut at_least = n;
    for k in 0..MAX_LAYERS {
        at_least -= hist[k];
        fractions.push(at_least as f64 / n.max(1) as f64);
    }
    let kept = prm.peel_stop.layers_rendered(&fractions);
    (kept, fractions)
}

/// `exact_item_layers` without the dense A-buffer: the band raster of the whole frame (the same visits,
/// the same z per fragment), each band's fragments sorted by (pixel, z, triangle) — equal depths keep the
/// triangle order, as the dense build's stable sort — and scanned run by run with the layer logic of
/// `extract_layers`; pixels without a fragment hold no item layer. No 16.7 M-entry tables.
pub fn exact_item_layers_direct(tris: &[WTri], frame: &PeelFrame, scene: &Scene, bvh: &Bvh, prm: &BakeParams, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask]) -> (usize, Vec<f64>) {
    let (w, h) = (frame.res, frame.res_y);
    let n = (w * h) as usize;
    let cull_back = {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var("LMTOOL_PEEL_CULL_BACK").map(|v| v == "1").unwrap_or(false))
    };
    let cards_occlude = cards_occlude();
    let d = frame.d;
    let inset = frame.inset_px;
    let clip = (0i32, 0i32, w as i32 - 1, h as i32 - 1);
    let rows = h as usize;
    let n_bands = threads.max(1).min(rows.max(1));
    let band_rows = (rows + n_bands - 1) / n_bands.max(1);
    let band_of = |y: i32| -> usize { ((y.max(0) as usize) / band_rows.max(1)).min(n_bands - 1) };
    let rows_of = |t: &WTri| -> Option<(i32, i32)> {
        let p0 = t.p0;
        let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
        let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
        let (x0, y0, z0) = frame.project(p0);
        let (x1, y1, z1) = frame.project(p1);
        let (x2, y2, z2) = frame.project(p2);
        if z0.min(z1).min(z2) >= zmax || z0.max(z1).max(z2) < zmin {
            return None;
        }
        if cull_back && t.inst != DECOR_INST {
            let ng = cross(t.e1, t.e2);
            if dot(ng, d) > 0.0 {
                return None;
            }
        }
        if !cards_occlude && t.alpha != u16::MAX {
            return None;
        }
        let (miny, maxy) = (y0.min(y1).min(y2), y0.max(y1).max(y2));
        if !(miny.is_finite() && maxy.is_finite()) {
            return None;
        }
        let ry0 = ((miny - 0.5).ceil() as i64).max(clip.1 as i64) as i32;
        let ry1 = ((maxy - 0.5).floor() as i64).min(clip.3 as i64) as i32;
        if ry0 > ry1 {
            return None;
        }
        // no pixel centre between the columns either → the raster would visit nothing (most leaf
        // triangles: the same test `raster::bounds` makes, so the visited set is unchanged)
        let (minx, maxx) = (x0.min(x1).min(x2), x0.max(x1).max(x2));
        if !(minx.is_finite() && maxx.is_finite()) {
            return None;
        }
        let rx0 = ((minx - 0.5).ceil() as i64).max(clip.0 as i64);
        let rx1 = ((maxx - 0.5).floor() as i64).min(clip.2 as i64);
        if rx0 > rx1 {
            return None;
        }
        Some((ry0, ry1))
    };
    let prep_chunk = (tris.len() / (threads * 4).max(1)).max(4096);
    let n_prep = (tris.len() + prep_chunk - 1) / prep_chunk;
    let binned: Vec<Vec<Vec<u32>>> = crate::pool::pool().map(n_prep, |ci| {
        let mut out: Vec<Vec<u32>> = (0..n_bands).map(|_| Vec::new()).collect();
        let a = ci * prep_chunk;
        for (k, t) in tris[a..(a + prep_chunk).min(tris.len())].iter().enumerate() {
            if let Some((ry0, ry1)) = rows_of(t) {
                for b in band_of(ry0)..=band_of(ry1) {
                    out[b].push((a + k) as u32);
                }
            }
        }
        out
    });
    // the layer logic per pixel run (fragments sorted by z, triangle order on ties)
    let is_env = |tri: u32| -> bool { let wt = &bvh.tris[tri as usize]; wt.inst == DECOR_INST && scene.decor.get(wt.tri as usize).map(|d| d.env).unwrap_or(false) };
    let env_drawn = |tri: u32| -> bool {
        let wt = &bvh.tris[tri as usize];
        match scene.decor.get(wt.tri as usize) {
            Some(dt) if dt.env_far_only => { let n = cross(wt.e1, wt.e2); dot(n, frame.d) > 0.0 }
            _ => true,
        }
    };
    let count_run = |list: &[(u32, Frag)]| -> usize {
        let mut d_prev = f32::NEG_INFINITY;
        if prm.dome_layer {
            let mut env_d = 0.0f32;
            for (_, f) in list {
                if is_env(f.tri) && env_drawn(f.tri) {
                    let z01 = frame.z01(f.z);
                    if z01 >= 0.0 && z01 <= 1.0 { env_d = env_d.max(z01); }
                }
            }
            d_prev = if env_d > 0.0 { if prm.depth_bits == 16 { (env_d * 65535.0).round() / 65535.0 } else { env_d } } else { 0.0 };
        }
        let mut items = 0usize;
        for (_, f) in list {
            if prm.dome_layer && is_env(f.tri) {
                continue;
            }
            let z01 = frame.z01(f.z).max(0.0);
            if z01 < d_prev {
                continue;
            }
            if items >= MAX_LAYERS {
                break;
            }
            let wt = &bvh.tris[f.tri as usize];
            let (slope, zmax_prim) = tri_slope(wt, frame);
            let mut dd = z01 + d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), prm.depth_bias, prm.depth_bits);
            if prm.depth_bits == 16 {
                dd = (dd.clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
            }
            items += 1;
            d_prev = dd;
        }
        items
    };
    // per band: raster its rows, sort, scan
    let hists: Vec<([usize; MAX_LAYERS + 1], usize)> = crate::pool::pool().map(n_bands, |b| {
        let by0 = (b * band_rows) as i32;
        let by1 = (((b + 1) * band_rows) as i32 - 1).min(clip.3);
        let mut hist = [0usize; MAX_LAYERS + 1];
        if by0 > by1 {
            return (hist, 0);
        }
        let band_clip = (clip.0, by0, clip.2, by1);
        let mut frags: Vec<(u32, Frag)> = Vec::new();
        for chunk_lists in &binned {
            for &ti in &chunk_lists[b] {
                let t = &tris[ti as usize];
                let p0 = t.p0;
                let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                let (x0, y0, z0) = frame.project(p0);
                let (x1, y1, z1) = frame.project(p1);
                let (x2, y2, z2) = frame.project(p2);
                let mask = if t.alpha != u16::MAX { masks.get(t.alpha as usize) } else { None };
                let mut fp_tex: Option<Option<(&crate::alphatex::AlphaTex, crate::alphatex::TapPlan)>> = None;
                raster::triangle_clipped_masked(w, h, [[x0, y0], [x1, y1], [x2, y2]], band_clip, None, |x, y, bc| {
                    if x < inset || y < inset || x + inset >= w || y + inset >= h {
                        return;
                    }
                    let z = z0 * bc[0] + z1 * bc[1] + z2 * bc[2];
                    if z < zmax && z >= zmin {
                        if let Some(m) = mask {
                            let u = t.uv0[0][0] * bc[0] + t.uv0[1][0] * bc[1] + t.uv0[2][0] * bc[2];
                            let v = t.uv0[0][1] * bc[0] + t.uv0[1][1] * bc[1] + t.uv0[2][1] * bc[2];
                            let fp = fp_tex.get_or_insert_with(|| m.tex.as_ref().map(|tx| (tx.as_ref(), tx.plan(&crate::alphatex::Footprint::of_triangle([[x0, y0], [x1, y1], [x2, y2]], t.uv0, tx.w(), tx.h()), *ALPHA_ANISO))));
                            let op = match fp {
                                Some((tx, fp)) if !*ALPHA_POINT => tx.passes_planned(u, v, fp, ALPHA_THRESHOLD, crate::alphatex::Address::ClampEdge),
                                _ => m.opaque(u, v),
                            };
                            if !op {
                                return;
                            }
                        }
                        frags.push((y * w + x, Frag { z, tri: ti }));
                    }
                });
            }
        }
        // (pixel, z, triangle): the z key orders as f32 (z is finite; total_cmp handles the sign)
        frags.sort_by(|p, q| p.0.cmp(&q.0).then_with(|| p.1.z.total_cmp(&q.1.z)).then_with(|| p.1.tri.cmp(&q.1.tri)));
        let mut covered = 0usize;
        let mut i = 0usize;
        while i < frags.len() {
            let mut j = i + 1;
            while j < frags.len() && frags[j].0 == frags[i].0 { j += 1; }
            hist[count_run(&frags[i..j]).min(MAX_LAYERS)] += 1;
            covered += 1;
            i = j;
        }
        (hist, covered)
    });
    let mut hist = [0usize; MAX_LAYERS + 1];
    let mut covered = 0usize;
    for (hh, c) in &hists { for k in 0..=MAX_LAYERS { hist[k] += hh[k]; } covered += c; }
    hist[0] += n - covered;
    let mut fractions: Vec<f64> = Vec::with_capacity(MAX_LAYERS);
    let mut at_least = n;
    for k in 0..MAX_LAYERS {
        at_least -= hist[k];
        fractions.push(at_least as f64 / n.max(1) as f64);
    }
    let kept = prm.peel_stop.layers_rendered(&fractions);
    (kept, fractions)
}

/// A fragment of the exact layer count: depth, triangle (the tie order), the triangle's depth-bias term.
#[derive(Clone, Copy, Debug)]
pub struct CFrag {
    pub z: f32,
    pub tri: u32,
    pub bias: f32,
}

/// Inline item fragments per pixel slot of a raster band (the rest overflow).
pub const SLOT_K: usize = 5;

/// One cache line per pixel (n + five 12-byte fragments = 64 bytes; the alignment keeps a slot from straddling
/// two lines).
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Slot {
    pub n: u32,
    pub f: [CFrag; SLOT_K],
}

impl Slot {
    pub const EMPTY: Slot = Slot { n: 0, f: [CFrag { z: 0.0, tri: 0, bias: 0.0 }; SLOT_K] };
}

/// LMTOOL_BOUND_STATS: [frames, decided-and-right (census/4), decided-and-right (census/8), decided-WRONG].
pub static BOUND_TOTALS: [std::sync::atomic::AtomicU64; 4] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];

thread_local! {
    /// The raster band's slot and environment tables, kept per pool thread across bands (see the fused count).
    static SLOT_BUFS: std::cell::Cell<(Vec<u16>, Vec<f32>, Vec<(u32, CFrag)>, Vec<CFrag>, Vec<u32>, Vec<u32>)> = const { std::cell::Cell::new((Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())) };
}

/// Raster bands per pool thread (LMTOOL_BANDS_PER_THREAD, default 4).
pub static BANDS_PER_THREAD: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_BANDS_PER_THREAD").ok().and_then(|v| v.parse().ok()).unwrap_or(4));

/// The census stride of the sparse layer-count estimate (every 8th pixel in x and y: 1/64 of the frame).
pub const CENSUS_STEP: u32 = 8;

pub static LAYER_STATS: (std::sync::atomic::AtomicUsize, std::sync::atomic::AtomicUsize) = (std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0));
fn peel_layers_debug() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("LMTOOL_PEEL_LAYERS_DEBUG").is_some())
}

/// Stage timers for `--profile` (nanoseconds, summed over the bake).
pub mod prof {
    use std::sync::atomic::{AtomicU64, Ordering};
    pub static BUILD: AtomicU64 = AtomicU64::new(0);
    pub static LAYERS: AtomicU64 = AtomicU64::new(0);
    pub static DUMP: AtomicU64 = AtomicU64::new(0);
    pub static GATHER: AtomicU64 = AtomicU64::new(0);
    pub static ACCUM: AtomicU64 = AtomicU64::new(0);
    pub static SNAP: AtomicU64 = AtomicU64::new(0);
    pub static B_CLIP: AtomicU64 = AtomicU64::new(0);
    pub static B_RASTER: AtomicU64 = AtomicU64::new(0);
    pub static B_SORT: AtomicU64 = AtomicU64::new(0);
    pub static B_INDEX: AtomicU64 = AtomicU64::new(0);
    pub static DIR: AtomicU64 = AtomicU64::new(0);
    pub static FRAMES: AtomicU64 = AtomicU64::new(0);
    pub static EXACT: AtomicU64 = AtomicU64::new(0);
    pub static L_PAR: AtomicU64 = AtomicU64::new(0);
    /// The per-direction glue outside the stages: the dome raster, the wanted bitmap, the sel/occl clear.
    pub static DOME: AtomicU64 = AtomicU64::new(0);
    pub static BITMAP: AtomicU64 = AtomicU64::new(0);
    pub static CLEAR: AtomicU64 = AtomicU64::new(0);
    pub static CONTRIB: AtomicU64 = AtomicU64::new(0);
    pub static CULL: AtomicU64 = AtomicU64::new(0);
    pub fn add(c: &AtomicU64, t: std::time::Instant) {
        c.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    pub fn report(label: &str, total: f32) {
        let g = |c: &AtomicU64| c.load(Ordering::Relaxed) as f64 / 1e9;
        let staged = g(&BUILD) + g(&LAYERS) + g(&DUMP) + g(&GATHER) + g(&ACCUM) + g(&SNAP) + g(&FRAMES) + g(&EXACT);
        crate::alphatex::alpha_stats_report();
        if crate::peel::BOUND_TOTALS[0].load(Ordering::Relaxed) > 0 { eprintln!("bound stats [{label}]: {} frames, stop certified right by the census/4 bounds {} and by census/8 {}, certified WRONG {}", crate::peel::BOUND_TOTALS[0].swap(0, Ordering::Relaxed), crate::peel::BOUND_TOTALS[1].swap(0, Ordering::Relaxed), crate::peel::BOUND_TOTALS[2].swap(0, Ordering::Relaxed), crate::peel::BOUND_TOTALS[3].swap(0, Ordering::Relaxed)); }
        eprintln!("profile [{label}] glue: dome raster {:.2}s, wanted bitmap {:.2}s, BVH cull {:.2}s, sel/occl clear {:.2}s, contribution {:.2}s", g(&DOME), g(&BITMAP), g(&CULL), g(&CLEAR), g(&CONTRIB));
        eprintln!("profile [{label}]: A-buffer build {:.2}s (wanted index {:.2}s, clip {:.2}s, raster {:.2}s, CSR {:.2}s), exact layer count {:.2}s, layer derivation {:.2}s (parallel part {:.2}s), per-direction dumps {:.2}s, gather {:.2}s, accumulate {:.2}s, accumulation snapshots {:.2}s, frames {:.2}s; directions total {:.2}s (unstaged {:.2}s); sweep total {total:.2}s", g(&BUILD), g(&B_INDEX), g(&B_CLIP), g(&B_RASTER), g(&B_SORT), g(&EXACT), g(&LAYERS), g(&L_PAR), g(&DUMP), g(&GATHER), g(&ACCUM), g(&SNAP), g(&FRAMES), g(&DIR), g(&DIR) - staged);
        for c in [&BUILD, &LAYERS, &DUMP, &GATHER, &ACCUM, &SNAP, &B_CLIP, &B_RASTER, &B_SORT, &B_INDEX, &DIR, &FRAMES, &EXACT, &L_PAR, &DOME, &BITMAP, &CLEAR, &CONTRIB, &CULL] { c.store(0, Ordering::Relaxed); }
    }
}

/// Diagnostics: peels extracted, of which the stop rule cut candidate layers.

/// The game's texel lookup into a layer target: POINT sampling after the one-texel inset
/// `u = 0.5 + (u − 0.5)·(w − 2)/w` (the Bias rows of `WorldPw01Shadow`), clamped — pixel index of a
/// continuous render-space pixel coordinate.
#[inline]
pub fn lookup_pixel(px: f32, w: u32, inset: bool) -> u32 {
    let w_f = w as f32;
    let p = if inset { px * (w_f - 2.0) / w_f + 1.0 } else { px };
    (p.floor().max(0.0) as u32).min(w.saturating_sub(1))
}

/// The layer a texel at reversed depth `z01` reads (`LmILightDir_Set_p`, GREATER_EQUAL, last written
/// wins): the nearest layer whose stored depth is ≤ z01 — the first surface beyond the texel along D.
#[inline]
pub fn select_layer(list: &[LayerFrag], z01: f32) -> Option<&LayerFrag> {
    // stored depths increase with the layer index: binary search the last d ≤ z01
    let mut lo = 0usize;
    let mut hi = list.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if list[mid].d <= z01 { lo = mid + 1 } else { hi = mid }
    }
    if lo == 0 { None } else { Some(&list[lo - 1]) }
}

/// The whole dome sweep, rasterised. `sizes[ii]` = the colour-resolution chart size of instance `ii`
/// (the layout rect is twice that; the raster runs at `prm.ss` sub-samples per layout texel).
pub fn bake_peel_raster(scene: &Scene, bvh: &Bvh, prm: &BakeParams, sizes: &[(u32, u32)]) -> Vec<ChartBake> {
    // the decoration's stand-in lightmap = what an unoccluded up-facing surface gets from this sky
    // (the sea and sand are lit by the sky like everything else; they have no lightmap of their own)
    let mut prm_local = prm.clone();
    if prm.decor_sky_up == [0.0; 3] {
        let dirs = &prm.sphere_dirs;
        let n = dirs.len().max(1) as f32;
        let mut e = [0f32; 3];
        for d in dirs.iter() {
            if d[1] <= 0.0 { continue; }
            let s = sky_radiance(prm, *d);
            for k in 0..3 { e[k] += 4.0 / n * d[1] * s[k]; }
        }
        prm_local.decor_sky_up = e;
        eprintln!("peel: open-sky irradiance of an up-facing surface ({:.3},{:.3},{:.3}) → the decoration's stand-in lightmap (× {})", e[0], e[1], e[2], prm.decor_ambient);
    }
    let prm = &prm_local;
    let t0 = std::time::Instant::now();
    let threads = if prm.threads == 0 { std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160) } else { prm.threads };
    let ss = prm.ss.max(1);
    // 1. every chart's sub-samples
    let mut subs: Vec<SubSample> = Vec::new();
    let mut chart_meta: Vec<(u32, u32)> = Vec::with_capacity(scene.instances.len());
    // the BVH triangle index of (instance, model tri): the BVH lists instances in order with their
    // model's triangles contiguous — recover the offsets
    let mut tri_base: Vec<u32> = Vec::with_capacity(scene.instances.len());
    {
        let mut acc = 0u32;
        let tiles_cast = std::env::var("LMTOOL_TILES_CAST").map(|v| v != "0").unwrap_or(true);
        for inst in &scene.instances {
            tri_base.push(acc);
            let m = &scene.models[inst.model];
            if tiles_cast || !crate::bake::is_flat_tile(m) {
                acc += m.tris.len() as u32;
            }
        }
    }
    // Per-texel gather: the editor's charts are smooth, so every layout texel gathers ALL the sweep's
    // directions at the centroid of its covered sub-samples (the ss² sub-samples decide coverage and the
    // resolve weight; interleaving the directions over the sub-samples — my first reading of the ss²
    // groups — gave block-correlated noise the editor does not show). `groups` is then 1.
    // `prm.per_subsample` (the differential harness's default) gathers at every covered sub-sample
    // instead — the game's supersampled raster as read — and the resolve averages them.
    let groups = 1usize;
    // the dump's chart_ss raster: (2·cw·ss_eff) × (2·ch·ss_eff)
    let ss_eff = if prm.per_subsample { ss } else { 1 };
    // chart metadata for the dump: (item, chart_ss w, h, covered mask per texel is implied by counts)
    let mut chart_geo: Vec<(Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 3]>)> = Vec::new(); // pos, nrm, albedo per chart_ss pixel
    let dumping = prm.dump.is_some();
    // THE LAYOUT RASTER, one chart per pool task (independent), the results concatenated in chart order
    type ChartOut = (Vec<SubSample>, (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 3]>));
    let n_inst = scene.instances.len();
    let per_chart: Vec<ChartOut> = crate::pool::pool().map(n_inst, |ii| {
        let (cw, ch) = sizes[ii];
        let (lw, lh) = (cw * 2, ch * 2);
        let r = crate::chartraster::raster_chart(scene, ii, lw, lh, ss, prm.flip_v, prm.uv_bounds);
        let (gw, gh) = (lw * ss_eff, lh * ss_eff);
        let mut geo = if dumping { (vec![[0.0f32; 3]; (gw * gh) as usize], vec![[0.0f32; 3]; (gw * gh) as usize], vec![[0.0f32; 3]; (gw * gh) as usize]) } else { (Vec::new(), Vec::new(), Vec::new()) };
        let mut subs: Vec<SubSample> = Vec::new();
        if prm.per_subsample {
            for s in &r.subs {
                let (tx, ty) = (s.sx / ss / 2, s.sy / ss / 2);
                let own = bvh.perm[(tri_base[ii] + s.tri) as usize];
                if dumping {
                    let gi = (s.sy * gw + s.sx) as usize;
                    geo.0[gi] = s.p;
                    geo.1[gi] = s.n;
                    geo.2[gi] = hit_albedo(scene, bvh, prm, &Hit { t: 0.0, tri: own });
                }
                subs.push(SubSample { p: s.p, n: s.n, own_tri: own, group: 0, chart: ii as u32, texel: ty.min(ch - 1) * cw + tx.min(cw - 1), sx: s.sx, sy: s.sy });
            }
        } else {
            // fold the sub-samples into layout texels: centroid position, mean normal, majority triangle
            let mut tex: std::collections::HashMap<(u32, u32), (V3, V3, u32, u32)> = std::collections::HashMap::new();
            for s in &r.subs {
                let key = (s.sx / ss, s.sy / ss);
                let e = tex.entry(key).or_insert(([0.0; 3], [0.0; 3], s.tri, 0));
                for k in 0..3 { e.0[k] += s.p[k]; e.1[k] += s.n[k]; }
                e.3 += 1;
            }
            for ((lx, ly), (psum, nsum, tri, cnt)) in tex {
                let c = cnt as f32;
                let p = [psum[0] / c, psum[1] / c, psum[2] / c];
                let n = norm(nsum);
                let (tx, ty) = (lx / 2, ly / 2);
                let own = bvh.perm[(tri_base[ii] + tri) as usize];
                if dumping {
                    let gi = (ly * gw + lx) as usize;
                    geo.0[gi] = p;
                    geo.1[gi] = n;
                    geo.2[gi] = hit_albedo(scene, bvh, prm, &Hit { t: 0.0, tri: own });
                }
                subs.push(SubSample { p, n, own_tri: own, group: 0, chart: ii as u32, texel: ty.min(ch - 1) * cw + tx.min(cw - 1), sx: lx, sy: ly });
            }
            // (the hash map's order is random per process: a deterministic set order — one sub-sample per
            // texel, so nothing downstream depends on it but the split's contribution files)
            subs.sort_by_key(|s| (s.sy, s.sx));
        }
        (subs, geo)
    });
    for (ii, (chart_subs, geo)) in per_chart.into_iter().enumerate() {
        chart_meta.push(sizes[ii]);
        if dumping { chart_geo.push(geo); }
        subs.extend(chart_subs);
    }
    eprintln!("peel: {} layout texels over {} charts (ss {ss} for coverage) ({:.1}s)", subs.len(), scene.instances.len(), t0.elapsed().as_secs_f32());
    // THE GAME'S RASTER JITTER: nine sub-sample sets, one per LM01_Trans_RasterSS offset — direction k
    // gathers and accumulates at set (k mod 9); the pixel centre samples the geometry at centre − shift/9
    // (the geometry itself is drawn shifted by +shift/9 texels)
    let jitter = prm.raster_jitter && ss == 1 && prm.per_subsample;
    let jit_sets: Vec<Vec<SubSample>> = if jitter {
        (0..9).map(|j| {
            let sh = prm.jitter_cycle[j];
            let shift = [prm.jitter_sign * -1.0 * sh[0] / 9.0, prm.jitter_sign * -1.0 * sh[1] / 9.0];
            // the charts in parallel (each chart's raster is independent), concatenated in chart order
            let n_inst = scene.instances.len();
            let per_chart: Vec<Vec<SubSample>> = crate::pool::pool().map(n_inst, |ii| {
                let (cw, ch) = sizes[ii];
                let (lw, lh) = (cw * 2, ch * 2);
                let r = crate::chartraster::raster_chart_shifted(scene, ii, lw, lh, 1, prm.flip_v, prm.uv_bounds, shift);
                let mut set: Vec<SubSample> = Vec::with_capacity(r.subs.len());
                for s in &r.subs {
                    let (tx, ty) = (s.sx / 2, s.sy / 2);
                    let own = bvh.perm[(tri_base[ii] + s.tri) as usize];
                    set.push(SubSample { p: s.p, n: s.n, own_tri: own, group: 0, chart: ii as u32, texel: ty.min(ch - 1) * cw + tx.min(cw - 1), sx: s.sx, sy: s.sy });
                }
                set
            });
            let mut set: Vec<SubSample> = Vec::with_capacity(per_chart.iter().map(|v| v.len()).sum());
            for v in per_chart { set.extend(v); }
            set
        }).collect()
    } else { Vec::new() };
    if jitter { eprintln!("peel: raster jitter on — nine sub-sample sets ({} .. {} samples), direction k at offset k mod 9 of {:?}/9 texels", jit_sets.iter().map(|s| s.len()).min().unwrap_or(0), jit_sets.iter().map(|s| s.len()).max().unwrap_or(0), prm.jitter_cycle); }
    // 2. group the sub-samples so each direction touches one contiguous range
    let mut order: Vec<u32> = (0..subs.len() as u32).collect();
    order.sort_by_key(|&i| subs[i as usize].group);
    let mut group_start = vec![0usize; groups + 1];
    for &i in &order {
        group_start[subs[i as usize].group as usize + 1] += 1;
    }
    for g in 0..groups {
        group_start[g + 1] += group_start[g];
    }
    // the accumulation target, per chart_ss PIXEL (the game's 2048² atlas texel): every sub-sample set
    // addresses it by (chart, sx, sy); `acc_of` reads a sub's cell
    let mut acc_tex: Vec<Vec<[f32; 3]>> = chart_meta.iter().map(|(cw, ch)| vec![[0.0f32; 3]; (cw * 2 * ss_eff * ch * 2 * ss_eff) as usize]).collect();
    let mut cover: Vec<Vec<u16>> = chart_meta.iter().map(|(cw, ch)| vec![0u16; (cw * 2 * ss_eff * ch * 2 * ss_eff) as usize]).collect();
    let pix_of = |s: &SubSample| -> (usize, usize) { (s.chart as usize, (s.sy * chart_meta[s.chart as usize].0 * 2 * ss_eff + s.sx) as usize) };
    // 3. the sun shadow map for the fragment radiance
    let (bmin, bmax) = scene_bounds(&bvh.tris);
    let shadow = if prm.sun_dir[1] > 0.0 && prm.sun.iter().any(|c| *c > 0.0) {
        let frame = match &prm.shadow_frustum {
            Some(f) => PeelFrame::from_frustum(f, prm.peel_res.max(1024), prm.peel_res.max(1024)),
            None => PeelFrame::new(prm.sun_dir, bmin, bmax, prm.peel_res.max(1024)),
        };
        Some(ShadowMap::build_in(&bvh.tris, frame, &prm.alpha_masks))
    } else { None };
    let sun_bias = 2.5 * (bmax[0] - bmin[0]).max(bmax[2] - bmin[2]) / prm.peel_res.max(1024) as f32 + 0.05;
    // --- the differential harness: the once-per-bake buffers and this sweep's ILightInput ---
    let chart_file = |pass: &str, sweep: Option<u32>, dir: Option<u32>, ii: usize| -> String {
        match (sweep, dir) {
            (Some(s), Some(d)) => format!("{pass}/s{s}/d{d:03}/chart{ii:04}.bin"),
            (Some(s), None) => format!("{pass}/s{s}/chart{ii:04}.bin"),
            _ => format!("{pass}/chart{ii:04}.bin"),
        }
    };
    let chart_ref = |ii: usize| crate::passdump::ChartRef { obj: prm.obj_base + scene.instances[ii].item as u32, item: scene.instances[ii].item as u32, sub: 0 };
    if let Some(dump) = &prm.dump {
        let mut dmp = dump.lock().unwrap();
        // the per-sub-sample ILightInput of this sweep: (C0 so far ÷ decode + sun·max(0, n·L)·shadow) × MDiffuse
        let mut ilight: Vec<Vec<[f32; 3]>> = chart_meta.iter().map(|(cw, ch)| vec![[0.0f32; 3]; (cw * 2 * ss_eff * ch * 2 * ss_eff) as usize]).collect();
        let mut sun_direct: Vec<Vec<[f32; 3]>> = chart_meta.iter().map(|(cw, ch)| vec![[0.0f32; 3]; (cw * 2 * ss_eff * ch * 2 * ss_eff) as usize]).collect();
        for s in &subs {
            let ii = s.chart as usize;
            let gw = chart_meta[ii].0 * 2 * ss_eff;
            let gi = (s.sy * gw + s.sx) as usize;
            let alb = &chart_geo[ii].2[gi];
            let stored: [f32; 3] = match &prm.field {
                Some(f) => match &f.charts[ii] { Some((w, _h, rgb)) => { let t = s.texel as usize; let _ = w; rgb.get(t).map(|e| [e[0] / prm.bounce_decode, e[1] / prm.bounce_decode, e[2] / prm.bounce_decode]).unwrap_or([0.0; 3]) } None => [0.0; 3] },
                None => [0.0; 3],
            };
            // the same sun term the peel gives a fragment of this surface (a card is lit from both sides)
            let is_card = bvh.tris[s.own_tri as usize].alpha != u16::MAX;
            let ndl = if is_card && !prm.card_one_sided { dot(s.n, prm.sun_dir).abs() } else { dot(s.n, prm.sun_dir).max(0.0) };
            let sun_on = prm.sweep > 0 || prm.sweep0_sun;
            let lit = if sun_on && ndl > 0.0 && prm.sun_dir[1] > 0.0 { shadow.as_ref().map(|sm| sm.lit(s.p, sun_bias)).unwrap_or(1.0) } else { 0.0 };
            let mut v = [0f32; 3];
            for k in 0..3 { v[k] = alb[k] * (stored[k] + prm.sun[k] * ndl * lit); }
            ilight[ii][gi] = v;
            // the game's `sun_direct` pass (frame 127448, PS 15187: LightRgb·max(0, n·L)·shadow, no albedo, the
            // 9 raster jitters × 1/9): the sun on this sub-sample whatever the sweep
            let lit_any = if ndl > 0.0 && prm.sun_dir[1] > 0.0 { shadow.as_ref().map(|sm| sm.lit(s.p, sun_bias)).unwrap_or(1.0) } else { 0.0 };
            sun_direct[ii][gi] = [prm.sun[0] * ndl * lit_any, prm.sun[1] * ndl * lit_any, prm.sun[2] * ndl * lit_any];
        }
        for ii in 0..chart_meta.len() {
            let (cw, ch) = chart_meta[ii];
            let (gw, gh) = (cw * 2 * ss_eff, ch * 2 * ss_eff);
            if prm.sweep == 0 {
                let mut e = crate::passdump::entry("sun_direct", chart_file("sun_direct", None, None, ii), "chart_ss");
                e.chart = Some(chart_ref(ii));
                e.notes = Some("LDirSun·max(0, n·L)·shadow per sub-sample, no albedo (the game's direct-sun atlas pass; its 9 raster jitters average to this)".into());
                dmp.write_rgb(e, gw, gh, &sun_direct[ii], crate::gpufmt::Quant::None, prm.rounding).expect("dump sun_direct");
                let mut e = crate::passdump::entry("lm_pos", chart_file("lm_pos", None, None, ii), "chart_ss");
                e.chart = Some(chart_ref(ii));
                e.notes = Some(format!("world position of every covered sub-sample (0 where uncovered); item {} = model {}", scene.instances[ii].item, scene.instances[ii].model_name));
                let flat: Vec<f32> = chart_geo[ii].0.iter().flat_map(|c| c.iter().copied()).collect();
                dmp.write_f32(e, gw, gh, 3, &flat).expect("dump lm_pos");
                let mut e = crate::passdump::entry("lm_nrm", chart_file("lm_nrm", None, None, ii), "chart_ss");
                e.chart = Some(chart_ref(ii));
                let flat: Vec<f32> = chart_geo[ii].1.iter().flat_map(|c| c.iter().copied()).collect();
                dmp.write_f32(e, gw, gh, 3, &flat).expect("dump lm_nrm");
                let mut e = crate::passdump::entry("mdiffuse", chart_file("mdiffuse", None, None, ii), "chart_ss");
                e.chart = Some(chart_ref(ii));
                e.notes = Some("the bounce albedo per sub-sample (per-material / texture mean; the game rasterises the diffuse texture)".into());
                dmp.write_rgb(e, gw, gh, &chart_geo[ii].2, crate::gpufmt::Quant::None, prm.rounding).expect("dump mdiffuse");
            }
            let mut e = crate::passdump::entry("ilightinput", chart_file("ilightinput", Some(prm.sweep), None, ii), "chart_ss");
            e.chart = Some(chart_ref(ii));
            e.sweep = Some(prm.sweep);
            e.notes = Some(format!("(C0 ÷ {} + LDirSun·max(0,n·L)·shadow) × MDiffuse per sub-sample; sweep 0 has C0 = 0", prm.bounce_decode));
            dmp.write_rgb(e, gw, gh, &ilight[ii], prm.quant_peel, prm.rounding).expect("dump ilightinput");
        }
        if prm.sweep == 0 {
            if let Some(sm) = &shadow {
                let mut e = crate::passdump::entry("sun_shadow", "sun_shadow/depth.bin".into(), "peel");
                e.format = "R32_FLOAT".into();
                e.dir = Some(prm.sun_dir);
                e.frustum = Some(sm.frame.frustum());
                e.notes = Some("nearest-to-the-sun surface per pixel; z01 = 0.5 + (center·forward − p·forward)/(2·half.z) with forward = the direction TOWARDS the sun (so the far plane is on the sun's side, z01 = 0 there); no rasteriser bias; empty pixels = 0".into());
                let z: Vec<f32> = sm.depth.z.iter().map(|&z| if z.is_finite() && z < f32::MAX { sm.frame.z01(z) } else { 0.0 }).collect();
                dmp.write_f32(e, sm.frame.res, sm.frame.res_y, 1, &z).expect("dump sun_shadow");
            }
        }
    }
    // 4. the directions, interleaved into the groups
    // (--max-dirs N: the harness stops the sweep after N directions — the accumulation snapshots against
    // the capture's first directions take a minute instead of twenty; the weights stay those of the full set)
    let dirs: Vec<V3> = prm.sphere_dirs.iter().copied().take(if prm.max_dirs > 0 { prm.max_dirs } else { usize::MAX }).collect();
    let n_dirs = dirs.len().max(1);
    // (the weight 4/N — and InvDirCount — is the FULL set's count, whatever --max-dirs cuts the loop to)
    let mut group_count = vec![0usize; groups];
    for di in 0..prm.sphere_dirs.len().max(dirs.len()) {
        group_count[di % groups] += 1;
    }
    let bias_m = prm.peel_bias;
    let sky_no_cos = std::env::var("LMTOOL_SKY_NO_COS").map(|v| v == "1").unwrap_or(false);
    // LMTOOL_SKY_DY=0: the sky colour along d without the d.y factor (plain receiver-cosine weighting)
    // default OFF (DIFFERENTIAL, 2026-09-23 17:45Z): with plain receiver-cosine weighting the editor's
    // wall/floor structure at Day is reproduced (walls 1.0–1.1 × the open floor, faces 1.3–1.5 like the
    // editor's 1.2–1.5); with the d.y factor the walls come out at 0.4 × — RE child 3's 4·w·d.y·SkyFactor
    // constant is then the AddSkyVisibility scalar's weight, not the radiance weight (to be confirmed)
    let sky_dy = std::env::var("LMTOOL_SKY_DY").map(|v| v == "1").unwrap_or(false);
    // LMTOOL_PEEL_DEBUG=x,y,z,r: trace the gather of the sub-samples within r of a world point
    let dbg: Option<(V3, f32)> = std::env::var("LMTOOL_PEEL_DEBUG").ok().and_then(|s| { let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 4 { Some(([v[0], v[1], v[2]], v[3])) } else { None } });
    let mut dbg_subs: Vec<u32> = match dbg { Some((c, r)) => subs.iter().enumerate().filter(|(_, s)| { let e = sub(s.p, c); dot(e, e) < r * r }).map(|(i, _)| i as u32).take(3).collect(), None => Vec::new() };
    // LMTOOL_PEEL_DEBUG_SIDE=1: instead, three side-facing sub-samples of chart 0 spread over its height
    if std::env::var_os("LMTOOL_PEEL_DEBUG_SIDE").is_some() {
        dbg_subs = subs.iter().enumerate().filter(|(_, s)| s.chart == 0 && s.n[1].abs() < 0.3).step_by(70000).map(|(i, _)| i as u32).take(3).collect();
    }
    if let Some((c, _)) = dbg { eprintln!("peel debug: {} sub-samples near {:?}: {:?}", dbg_subs.len(), c, dbg_subs.iter().map(|&i| (subs[i as usize].p, subs[i as usize].n, subs[i as usize].group)).collect::<Vec<_>>()); }
    let dbg_printed = std::sync::atomic::AtomicUsize::new(0);
    let game_dbg: Option<(usize, usize, usize)> = std::env::var("LMTOOL_GAME_PEEL_DEBUG").ok().and_then(|s| { let v: Vec<usize> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 3 { Some((v[0], v[1], v[2])) } else { None } });
    let game_dbg = &game_dbg;
    let dbg_printed = &dbg_printed;
    // THE TRANSCRIBED ACCUMULATE in the harness (--lm-from PASSCAP): the game's own LM raster geometry (the capture's LM
    // meshes: vertex normals, tangents, PSIZE frame modes, the two-sided cards) runs LmILightDir_Set (lmaccum::run_set_block)
    // over OUR peel layers after every layer of both peels, then the H-basis draws (lmaccum::run_hbasis) into four RGBA16F
    // MRTs kept across the sweep — rows 7–9 as transcribed, so that what remains against the capture is the peel content
    let mut hb_lm: Option<crate::lmaccum::HbTargets> = prm.lm_scene.as_ref().map(|_| crate::lmaccum::HbTargets::cleared(2048, 2048));
    let mut lm_rows: Vec<String> = Vec::new();
    // the per-direction buffers, allocated once (their fills run on the pool): the selected radiance and
    // occlusion flag per sub-sample, and the identity index range of the jitter sets
    let max_set = if jitter { jit_sets.iter().map(|s| s.len()).max().unwrap_or(0) } else { subs.len() };
    let mut sel_buf: Vec<[f32; 3]> = vec![[0.0; 3]; max_set];
    let mut occl_buf: Vec<bool> = vec![false; max_set];
    let range_all_buf: Vec<u32> = if jitter { (0..max_set as u32).collect() } else { Vec::new() };
    // the tiles' world-XZ clip boxes per peel index (None = the world peel: no clip), see the gather
    let tile_clip: Vec<Option<[f32; 4]>> = prm.peel_tile_clip.as_ref().map(|v| v.as_ref().clone()).unwrap_or_default();
    // THE DIRECTION-RANGE SPLIT (contrib.rs): a box bakes the directions of its range and writes what each
    // contributes; the merge replays every direction's contribution through the accumulate below in order
    // (eight writers: a file on the shared store costs a second of latency; one writer throttled the bake to
    // its pace — 41 of 176 cores busy)
    // a `.contribs` target = ONE pack file for the range, assembled in memory and written at the end (the
    // shared store takes seconds per small file); a directory = one file per direction
    let pack_entries: std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<u32, Vec<u8>>>> = std::sync::Arc::new(std::sync::Mutex::new(std::collections::BTreeMap::new()));
    let pack_mode = prm.contrib_out.as_ref().map(|p| crate::contrib::is_pack(p)).unwrap_or(false);
    let mut contrib_writers: Vec<std::thread::JoinHandle<()>> = Vec::new();
    let contrib_tx: Option<std::sync::mpsc::SyncSender<crate::contrib::DirContrib>> = prm.contrib_out.as_ref().map(|dir| {
        let (tx, rx) = std::sync::mpsc::sync_channel::<crate::contrib::DirContrib>(64);
        let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
        for _ in 0..8 {
            let (dir, rx, pe) = (dir.clone(), rx.clone(), pack_entries.clone());
            contrib_writers.push(std::thread::spawn(move || loop {
                let c = match rx.lock().unwrap().recv() { Ok(c) => c, Err(_) => break };
                if pack_mode { let b = c.file_bytes(); pe.lock().unwrap().insert(c.di, b); } else { c.write(&dir).expect("write contribution"); }
            }));
        }
        tx
    });
    if prm.contrib_out.is_some() {
        if let Some(pb) = &prm.probe_bake { pb.lock().unwrap().sky_log = Some(Vec::new()); }
    }
    let mut n_replayed = 0usize;
    let prefetch: Option<crate::contrib::Prefetch> = prm.merge_contrib.as_ref().map(|dirs_in| {
        let order: Vec<u32> = (0..dirs.len()).filter(|di| prm.dir_range.map(|(a, b)| *di >= a && *di < b).unwrap_or(true)).map(|di| di as u32).collect();
        crate::contrib::Prefetch::new(dirs_in.clone(), prm.sweep, order, 8, 24)
    });
    // the in-process merge: the directions outside the live range from the other boxes' packs
    let mut range_reader: Option<crate::contrib::RangeReader> = prm.merge_ranges.as_ref().map(|r| crate::contrib::RangeReader::new(r.clone(), prm.sweep));
    for (di, d) in dirs.iter().enumerate() {
        let live = prm.dir_range.map(|(a, b)| di >= a && di < b).unwrap_or(true);
        if !live && range_reader.is_none() {
            continue;
        }
        let replay: Option<crate::contrib::DirContrib> = if live {
            prefetch.as_ref().map(|pf| {
                let c = pf.take(di as u32).unwrap_or_else(|e| panic!("merge-contrib: {e}"));
                n_replayed += 1;
                c
            })
        } else {
            let rr = range_reader.as_mut().unwrap();
            if rr.range_of(di).is_none() { continue; }
            let c = rr.take(di).unwrap_or_else(|e| panic!("merge: {e}"));
            n_replayed += 1;
            Some(c)
        };
        let t_dir = std::time::Instant::now();
        let g = di % groups;
        let scale = 4.0 / group_count[g].max(1) as f32;
        // THE PEELS of this direction: the captured frustums (the game runs two — the whole-scene frustum,
        // then one fitted to the items — and the accumulate takes the later peel's layer wherever it has
        // one), or the port's own single frame fit to the receivers
        let peels: Vec<PeelFrame> = match prm.frustums.as_ref().and_then(|fs| fs.get(di)).filter(|v| !v.is_empty()) {
            Some(frs) => frs.iter().map(|fr| PeelFrame::from_frustum(fr, prm.peel_res, prm.peel_res)).collect(),
            None => {
                let mut fr = PeelFrame::new(*d, bmin, bmax, prm.peel_res);
                // the game's frustum covers its whole scene (the ground tiles included); ours is fit to
                // the receivers, so the far plane is pushed out to every occluder — otherwise the ground,
                // the sea and the decoration would pancake onto one far-plane layer and merge into its
                // farthest (often invisible / black) member (found by the dry run, 2026-09-24 18:10 PT)
                if prm.game_peel { fr.extend_far(bvh); }
                vec![fr]
            }
        };
        prof::add(&prof::FRAMES, t_dir);
        // THE PROBES: the direction's volume is cleared (ClearRenderTargetView right after the direction's start)
        if let Some(pb) = &prm.probe_bake { pb.lock().unwrap().begin_direction_of(prm.sweep, *d); }
        let tb = std::time::Instant::now();
        // the sky term's per-direction constant is w·4·d.y·SkyFactor (RE child 3, AddSkyVisibility /
        // SetILightDir 0x140234df0): the sky colour along d is weighted by the direction's elevation cosine
        // and below-horizon directions carry no sky (they see the ground); SkyFactor rides in sky_radiance
        let sky = { let s = sky_radiance(prm, *d); let dy = if sky_dy { d[1].max(0.0) } else if d[1] > 0.0 { 1.0 } else { 0.0 }; [s[0] * dy, s[1] * dy, s[2] * dy] };
        let bias = bias_m;
        let range = &order[group_start[g]..group_start[g + 1]];
        let want_dir_dump = prm.dump.as_ref().map(|dm| dm.lock().unwrap().wants_dir(di as u32)).unwrap_or(false);
        // the per-sub-sample incoming radiance of this direction (TMapILightDir): the game's target is
        // cleared, the first accumulate fills the facing texels with the sky, every peel's layers then
        // overwrite where they pass — `sel` holds the running value, `occl` whether a surface wrote it
        // the sub-sample set of this direction (the jittered raster) and its index range
        let cur: &Vec<SubSample> = if jitter { &jit_sets[di % 9] } else { &subs };
        let range: &[u32] = if jitter { &range_all_buf[..cur.len()] } else { range };
        let chunk = (range.len() / threads.max(1)).max(1024);
        let sky_fill = prm.quant_ilightdir.apply(prm.quant_peel.apply(sky, prm.rounding), prm.rounding);
        // THE DOME PER PIXEL (the transcribed sky dome, `SkyGradient::dome_radiance`): the peel pixel's ray
        // meets the ellipsoid at a point whose azimuth/height differ slightly from D's across a frame (the
        // dome sits at the world origin, 22 km out); the colour the layer target holds is R11G11B10
        // THE DOME MESH RASTERISED (domemesh.rs: VS 16773 + the rasteriser state, the game's own dome triangles
        // covering the frame; `dome_r` per peel below): the pixel's (u, v) and view vector are the screen-space
        // interpolation of the covering triangle's vertex attributes, then PS 16774 (`sky_ps`). Without the mesh
        // (--dome-analytic) the analytic ellipsoid model stands in.
        let dome_px = |frame: &PeelFrame, dome_r: Option<&crate::domemesh::DomeRaster>, px: u32, py: u32| -> [f32; 3] {
            match &prm.sky_grad {
                Some(sg) if prm.dome_exact => {
                    let v = match dome_r {
                        // the mesh: the covering triangle's interpolated attributes; a pixel no front face covers
                        // (the viewport's outer ring) keeps the clear colour, as the game's target does
                        Some(r) => match r.at(px, py) { Some((uv, view)) => sg.sky_ps(uv, view), None => [0.0; 3] },
                        None => {
                            let q = frame.unproject(px as f32 + 0.5, py as f32 + 0.5, frame.z_from_z01(1.0));
                            let c = frame.frustum().center;
                            sg.dome_radiance(q, *d, c)
                        }
                    };
                    // (the dome layer's colour goes through the R11G11B10 target, then the ILightDir target)
                    prm.quant_ilightdir.apply(prm.quant_peel.apply(v, prm.rounding), prm.rounding)
                }
                _ => sky_fill,
            }
        };
        // (the reused buffers, filled in parallel)
        let sel: &mut [[f32; 3]] = &mut sel_buf[..cur.len()];
        let occl: &mut [bool] = &mut occl_buf[..cur.len()];
        let t_clear = std::time::Instant::now();
        {
            let n = cur.len();
            let per = (n / (threads * 2).max(1)).max(4096);
            let (sp, op) = (sel.as_mut_ptr() as usize, occl.as_mut_ptr() as usize);
            crate::pool::pool().run((n + per - 1) / per, |ci| {
                for i in ci * per..((ci + 1) * per).min(n) {
                    // SAFETY: disjoint index ranges per task
                    unsafe { *(sp as *mut [f32; 3]).add(i) = sky_fill; *(op as *mut bool).add(i) = false; }
                }
            });
        }
        prof::add(&prof::CLEAR, t_clear);
        let mut t_build_total = 0.0f32;
        // the transcribed accumulate's TMapILightDir of this direction (cleared before the first block)
        let mut dir_lm: Option<crate::lmaccum::DirTarget> = prm.lm_scene.as_ref().map(|_| crate::lmaccum::DirTarget::cleared(2048, 2048));
        // the fused blocks' per-pixel "highest block that wrote" and the block numbering across this direction's peels
        let mut dir_best_k: Vec<u16> = Vec::new();
        let mut dir_block_base: usize = 0;
        let mut frag_total = 0usize;
        if let Some(c) = &replay {
            // THE REPLAY: the contribution's sel for the facing sub-samples (in set order), its occl bits, the
            // probe volume and the sky-visibility adds — then the same accumulate as a live direction
            assert_eq!(c.n_subs as usize, cur.len(), "merge-contrib: direction {di}: the sub-sample set differs ({} vs {}) — a different map, layout or jitter", c.n_subs, cur.len());
            let mut k = 0usize;
            for (i, s) in cur.iter().enumerate() {
                if dot(s.n, *d) > 0.0 {
                    sel[i] = c.sel[k];
                    k += 1;
                }
                occl[i] = c.occl(i);
            }
            assert_eq!(k, c.sel.len(), "merge-contrib: direction {di}: {} facing sub-samples, {} stored", k, c.sel.len());
            if let Some(pb) = &prm.probe_bake { pb.lock().unwrap().import_direction(&c.probe_cur, &c.sky_adds); }
        } else {
        for (pi, frame) in peels.iter().enumerate() {
            let tb2 = std::time::Instant::now();
            let t_dome = std::time::Instant::now();
            // the game's dome mesh rasterised in this peel's frame (the eye = GbxV_EyeInWorld = the frustum's
            // centre — the scene bbox the frustum is fit to; a metre off moves the 22 km dome's view vector by
            // 5e-5 rad), the sun shift = LightDirAngle_m11Zx, InvertY on, ForceX off — the capture's GbxSkyV0
            let dome_r: Option<crate::domemesh::DomeRaster> = match (&prm.dome_mesh, &prm.sky_grad) {
                (Some(m), Some(sg)) if prm.dome_exact => Some(m.rasterise(frame, frame.frustum().center, sg.light_dir_angle(), -1.0, true)),
                _ => None,
            };
            if let Some(r) = &dome_r { if want_dir_dump || std::env::var_os("LMTOOL_PEEL_LAYERS_DEBUG").is_some() { eprintln!("peel: direction {di} peel {pi}: the dome mesh covers the frame with {} front-facing triangles", r.triangles()); } }
            let dome_r = dome_r.as_ref();
            prof::add(&prof::DOME, t_dome);
            // the deepest receiver along this direction: nothing beyond it can occlude
            let zmax = (0..8).map(|i| { let p = [if i & 1 == 0 { bmin[0] } else { bmax[0] }, if i & 2 == 0 { bmin[1] } else { bmax[1] }, if i & 4 == 0 { bmin[2] } else { bmax[2] }]; frame.project(p).2 }).fold(f32::MIN, f32::max);
            // the game's peel renders everything inside the frustum's depth range; beyond the far plane the
            // fragments are dropped (DepthClipEnable, the capture) or pancaked onto it (--no-depth-clip);
            // fragments nearer than the near plane can never be selected (they are on the receivers' side)
            // the pixels this direction's texels read (the game's lookup of every sub-sample of the
            // current set): only their fragments are kept and only their layers derived — unless the
            // direction is dumped, when every pixel is wanted
            let t_idx = std::time::Instant::now();
            // (the transcribed accumulate reads every pixel of the layers: the dense path when --lm-from is on)
            let wanted: Option<std::sync::Arc<PixelIndex>> = if prm.game_peel && !want_dir_dump && prm.lm_scene.is_none() {
                let n = (frame.res as usize * frame.res_y as usize + 63) / 64;
                // one shared bitmap, the bits OR-ed in atomically (neighbouring sub-samples share words,
                // and neighbours sit in the same chunk — the contention is nil)
                let nch = (threads * 2).max(1);
                let per = (cur.len() + nch - 1) / nch;
                let m: Vec<std::sync::atomic::AtomicU64> = (0..n).map(|_| std::sync::atomic::AtomicU64::new(0)).collect();
                // a tile's frame: only the sub-samples inside the tile's world-XZ cell read it (the gather's
                // clip rule below) — the others' pixels are not wanted (a giant's tile holds a ninth of them)
                let clip_box: Option<[f32; 4]> = tile_clip.get(pi).copied().flatten();
                crate::pool::pool().run(nch, |ci| {
                    for s in &cur[ci * per..((ci + 1) * per).min(cur.len())] {
                        if let Some(b) = clip_box {
                            if !(s.p[0] - b[0] >= 0.0 && s.p[2] - b[1] >= 0.0 && b[2] - s.p[0] >= 0.0 && b[3] - s.p[2] >= 0.0) { continue; }
                        }
                        let (x, y, _) = frame.project(s.p);
                        let (px, py) = (lookup_pixel(x, frame.res, prm.peel_inset), lookup_pixel(y, frame.res_y, prm.peel_inset));
                        let i = py as usize * frame.res as usize + px as usize;
                        m[i >> 6].fetch_or(1u64 << (i & 63), std::sync::atomic::Ordering::Relaxed);
                    }
                });
                let mut m: Vec<u64> = m.into_iter().map(|a| a.into_inner()).collect();
                // THE PROBES' PIXELS (the transcribed probe passes read the world peel's layer targets at every
                // probe's shadow coordinate — a 2×2 comparison filter and a point colour sample): the 3×3 around
                // each probe's texel joins the wanted set, so the sparse layers hold what the passes read
                if pi == 0 {
                    if let Some(pb) = &prm.probe_bake {
                        let pb = pb.lock().unwrap();
                        let pw01 = frame.world_pw01();
                        let (w, h) = (frame.res as i64, frame.res_y as i64);
                        for b in &pb.blocks {
                            let dr = b.draw(&pw01, 1.0);
                            for z in b.min[2]..b.max[2] { for y in b.min[1]..b.max[1] { for x in b.min[0]..b.max[0] {
                                let p = crate::probepass::probe_point(x, y, z, pb.offsets.as_ref());
                                let sh = crate::probepass::to_shadow(p, &dr.regs, pb.opts.fma);
                                let (fx, fy) = ((sh[0] * w as f32 - 0.5).floor(), (sh[1] * h as f32 - 0.5).floor());
                                if !(fx.is_finite() && fy.is_finite()) { continue; }
                                for dy in -1..=2i64 { for dx in -1..=2i64 {
                                    let (px, py) = ((fx as i64 + dx).clamp(0, w - 1), (fy as i64 + dy).clamp(0, h - 1));
                                    let i = (py * w + px) as usize;
                                    m[i >> 6] |= 1u64 << (i & 63);
                                } }
                            } } }
                        }
                    }
                }
                // the CENSUS pixels for the layer-count rule's written fractions (every CENSUS_STEP-th pixel in x
                // and y), unless this peel's item-layer count is known (captured or fixed)
                let fixed_layers_known = if prm.layers_from_capture { prm.peel_layer_counts.as_ref().and_then(|c| c.get(di)).and_then(|v| v.get(pi)).copied().flatten().is_some() } else { prm.peel_layers_fixed.is_some() };
                if !fixed_layers_known && prm.layers_estimate {
                    // only inside the texels' bounding rectangle: an item layer can only be written where an
                    // item's texels project (every item has charts), so the census pixels outside it hold no
                    // item layer and are counted analytically (extract_layers)
                    let (w, h) = (frame.res as usize, frame.res_y as usize);
                    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
                    for (wi, word) in m.iter().enumerate() {
                        if *word == 0 { continue; }
                        let mut v = *word;
                        while v != 0 {
                            let b = v.trailing_zeros() as usize;
                            v &= v - 1;
                            let i = wi * 64 + b;
                            let (x, y) = (i % w, i / w);
                            x0 = x0.min(x); x1 = x1.max(x); y0 = y0.min(y); y1 = y1.max(y);
                        }
                    }
                    if x0 <= x1 {
                        let step = CENSUS_STEP as usize;
                        let mut y = (y0 / step) * step;
                        while y <= y1 && y < h {
                            let mut x = (x0 / step) * step;
                            while x <= x1 && x < w {
                                if x >= x0 && y >= y0 {
                                    let i = y * w + x;
                                    m[i >> 6] |= 1u64 << (i & 63);
                                }
                                x += step;
                            }
                            y += step;
                        }
                    }
                }
                prof::add(&prof::BITMAP, t_idx);
                // THE PROBES' texels (probebake.rs): the world peel's layers must exist where the probe draws sample them —
                // every block cell's (u, v) texel and its 3×3 neighbourhood (the sky visibility's 2×2 PCF footprint)
                if let (Some(pb), true) = (&prm.probe_bake, pi == 0) {
                    let pbl = pb.lock().unwrap();
                    let pw01 = frame.world_pw01();
                    let (w, h) = (frame.res as usize, frame.res_y as usize);
                    for b in &pbl.blocks {
                        let d = b.draw(&pw01, 1.0);
                        for z in b.min[2]..b.max[2] { for y in b.min[1]..b.max[1] { for x in b.min[0]..b.max[0] {
                            let p = crate::probepass::probe_point(x, y, z, pbl.offsets.as_ref());
                            let sh = crate::probepass::to_shadow(p, &d.regs, pbl.opts.fma);
                            let (tx, ty) = (crate::probepass::texel_point(sh[0], frame.res) as i64, crate::probepass::texel_point(sh[1], frame.res_y) as i64);
                            for dy in -1..=1i64 { for dx in -1..=1i64 {
                                let (px, py) = (tx + dx, ty + dy);
                                if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h { let i = py as usize * w + px as usize; m[i >> 6] |= 1u64 << (i & 63); }
                            } }
                        } } }
                    }
                }
                Some(std::sync::Arc::new(PixelIndex::new(frame.res, frame.res_y, m)))
            } else { None };
            prof::add(&prof::B_INDEX, t_idx);
            // THE EXACT LAYER COUNT (default): with no captured or fixed count for this peel, a dense depth-only
            // build of the whole frame gives the written fraction of every item layer exactly, then the stop
            // rule; --layers-estimate takes the census estimate on the sparse build instead
            let t_exact = std::time::Instant::now();
            let need_exact = prm.game_peel && !want_dir_dump && !prm.layers_estimate && {
                let known = if prm.layers_from_capture { prm.peel_layer_counts.as_ref().and_then(|c| c.get(di)).and_then(|v| v.get(pi)).copied().flatten() } else { prm.peel_layers_fixed };
                known.is_none()
            };
            // (the separate forms of the exact pass, for the check: LMTOOL_EXACT_DENSE=1 the dense A-buffer,
            // LMTOOL_EXACT_SEPARATE=1 the direct pass on its own; the default fuses it with the sparse build)
            let mut exact_layers: Option<usize> = None;
            let fuse = need_exact && std::env::var_os("LMTOOL_EXACT_DENSE").is_none() && std::env::var_os("LMTOOL_EXACT_SEPARATE").is_none();
            if need_exact && !fuse {
                let (kept, fractions) = if std::env::var_os("LMTOOL_EXACT_DENSE").is_some() {
                    let dense = build_abuffer_wanted(&bvh.tris, frame, threads, if prm.depth_clip { frame.z_from_z01(0.0) } else { f32::NEG_INFINITY }, frame.z_from_z01(1.0), &prm.alpha_masks, None);
                    exact_item_layers(&dense, frame, scene, bvh, prm, threads)
                } else {
                    exact_item_layers_direct(&bvh.tris, frame, scene, bvh, prm, threads, if prm.depth_clip { frame.z_from_z01(0.0) } else { f32::NEG_INFINITY }, frame.z_from_z01(1.0), &prm.alpha_masks)
                };
                if peel_layers_debug() { eprintln!("peel layers (exact, sweep {} direction {di} peel {pi}): fractions {:?} → {kept} rendered", prm.sweep, fractions.iter().take_while(|f| **f > 0.0).map(|f| format!("{f:.6}")).collect::<Vec<_>>()); }
                exact_layers = Some(kept);
            }
            prof::add(&prof::EXACT, t_exact);
            let ab = if prm.game_peel {
                match wanted.as_ref() {
                    Some(px) => {
                        // the triangles the frame's volume can hold, as BVH leaf ranges (a tile's frame holds a part of the scene)
                        let (zmin_f, zmax_f) = (if prm.depth_clip { frame.z_from_z01(0.0) } else { f32::NEG_INFINITY }, frame.z_from_z01(1.0));
                        let t_cull = std::time::Instant::now();
                        let ranges = if std::env::var_os("LMTOOL_NO_CULL").is_some() { vec![(0u32, bvh.tris.len() as u32)] } else { bvh.ranges_where(|lo, hi| frame.box_class(lo, hi, zmin_f, zmax_f)) };
                        if std::env::var_os("LMTOOL_CULL_DEBUG").is_some() {
                            let n: u32 = ranges.iter().map(|r| r.1 - r.0).sum();
                            // how many triangles project a vertex inside the frame (the ideal)
                            let inside = bvh.tris.iter().filter(|t| { let (x, y, z) = frame.project(t.p0); x >= 0.0 && x <= frame.res as f32 && y >= 0.0 && y <= frame.res_y as f32 && z >= zmin_f && z < zmax_f }).count();
                            eprintln!("cull: direction {di} peel {pi}: {} of {} triangles in {} BVH ranges (p0 inside the frame: {inside}); frame res {}×{} scale {:.4} s0 {:.1} t0 {:.1} zc {:.1} half_d {:.1}", n, bvh.tris.len(), ranges.len(), frame.res, frame.res_y, frame.scale, frame.s0, frame.t0, frame.zc, frame.half_d);
                        }
                        prof::add(&prof::CULL, t_cull);
                        let (ab, counted) = build_abuffer_sparse_ranges(&bvh.tris, &ranges, frame, threads, zmin_f, zmax_f, &prm.alpha_masks, px, if fuse { Some(CountCtx { scene, bvh, prm }) } else { None });
                        if let Some((kept, fractions)) = counted {
                            if peel_layers_debug() { eprintln!("peel layers (exact, sweep {} direction {di} peel {pi}): fractions {:?} → {kept} rendered", prm.sweep, fractions.iter().take_while(|f| **f > 0.0).map(|f| format!("{f:.6}")).collect::<Vec<_>>()); }
                            exact_layers = Some(kept);
                        }
                        ab
                    }
                    None => build_abuffer_wanted(&bvh.tris, frame, threads, if prm.depth_clip { frame.z_from_z01(0.0) } else { f32::NEG_INFINITY }, frame.z_from_z01(1.0), &prm.alpha_masks, None),
                }
            } else { build_abuffer_upto(&bvh.tris, frame, threads, zmax, &prm.alpha_masks) };
            card_dump_flush(prm.sweep, di, pi, frame, &prm.alpha_masks);
            t_build_total += tb2.elapsed().as_secs_f32();
            prof::add(&prof::BUILD, tb2);
            frag_total += ab.len();
            // the game's layers of this peel (game-peel mode), and their dump
            // (the layers are also extracted for the dump alone, so the port's own gather can be dumped and compared)
            let tl = std::time::Instant::now();
            // the item-layer count: the captured one for this direction's peel when the harness has it, else the stop rule
            let fixed_layers: Option<usize> = if prm.layers_from_capture { prm.peel_layer_counts.as_ref().and_then(|c| c.get(di)).and_then(|v| v.get(pi)).copied().flatten() } else { prm.peel_layers_fixed };
            // the environment render's dome colour PER PIXEL for the transcribed accumulate's layer-0 colour target: the
            // dome as `dome_px` transcribes it (the game's dome mesh rasterised in this frame, PS 16774, the R11G11B10 target)
            // — the game's env render is per pixel, the uniform sky is only the gather's fallback
            let dome_img: Option<Vec<[f32; 3]>> = if prm.lm_scene.is_some() && prm.sky_grad.is_some() && prm.dome_exact {
                let (w, h) = (frame.res as usize, frame.res_y as usize);
                let mut img = vec![[0.0f32; 3]; w * h];
                let rows_per = (h / threads.max(1)).max(1);
                let dome_px = &dome_px;
                std::thread::scope(|sc| {
                    for (ti, chunk) in img.chunks_mut(rows_per * w).enumerate() {
                        let y0 = ti * rows_per;
                        sc.spawn(move || {
                            for (i, px) in chunk.iter_mut().enumerate() {
                                // (dome_px quantises through the peel target and the ILightDir target — the same R11G11B10 twice)
                                *px = dome_px(frame, dome_r, (i % w) as u32, (y0 + i / w) as u32);
                            }
                        });
                    }
                });
                Some(img)
            } else {
                None
            };
            let fixed_layers = fixed_layers.or(exact_layers);
            let layers: Option<Layers> = if prm.game_peel || want_dir_dump { Some(extract_layers(&ab, frame, scene, bvh, prm, shadow.as_ref(), sun_bias, sky, threads, wanted.as_ref(), fixed_layers, dome_img.as_deref())) } else { None };
            prof::add(&prof::LAYERS, tl);
            let td = std::time::Instant::now();
            if let Some(ly) = &layers {
                if want_dir_dump || std::env::var_os("LMTOOL_PEEL_LAYERS_DEBUG").is_some() {
                    let cand = ly.fractions.iter().take_while(|f| **f > 0.0).count();
                    eprintln!("peel: direction {di} peel {pi}: {} item layers rendered of {} with content (fractions {}); {}", ly.item_layers, cand, ly.fractions.iter().take(cand.max(ly.item_layers).min(ly.fractions.len())).map(|f| format!("{f:.6}")).collect::<Vec<_>>().join(" "), match fixed_layers { Some(k) => format!("the captured count {k}"), None => format!("the stop rule (< {}, lag {})", prm.peel_stop.threshold, prm.peel_stop.lag) });
                }
            }
            // AddAmbient (CS 17125, lmaccum.rs): once per UPWARD direction of the first sweep, after the world peel's environment
            // render — `accum.xyz += Scale · colour[W/2, H/2]`, `.w += Scale/2`, Scale = 4·w·D.y·Sky with w = 2/N (= D.y/32 at N = 256)
            // (the accumulator after every direction is kept: entry di = the value once direction di has been issued)
            if let (Some(acc), Some(ly), true) = (&prm.ambient_out, &layers, prm.sweep == 0 && pi == 0) {
                let mut g = acc.lock().unwrap();
                let mut v = g.last().copied().unwrap_or([0.0; 4]);
                if d[1] > 0.0 {
                    let img = ly.colour_image(0, 0);
                    let centre = img[((ly.h / 2) * ly.w + ly.w / 2) as usize];
                    let scale = 4.0 * (2.0 / prm.sphere_dirs.len().max(1) as f32) * d[1] * 1.0;
                    crate::lmaccum::cs_17125(&mut v, centre, scale);
                }
                while g.len() < di { let l = g.last().copied().unwrap_or([0.0; 4]); g.push(l); }
                g.push(v);
            }
            // the transcribed LmILightDir_Set blocks over this peel's layers: block k reads layer k's colour + depth targets
            // (k = 0 the environment render; the clear 1.0 / black where a pixel has fewer layers); the fitted peel's blocks
            // clip to the items' world box (VS 17115)
            // the two consumers of this peel's layer targets: the transcribed accumulate (every peel) and the probes (the
            // world peel only) — the colour / depth Bufs of a layer are built once for both
            let want_probes = pi == 0 && prm.probe_bake.is_some();
            if let Some(ly) = layers.as_ref().filter(|_| prm.lm_scene.is_some() || want_probes) {
                let tlm = std::time::Instant::now();
                // the layer count over the pixels present (`start` is per wanted pixel in the sparse form)
                let nl = ly.start.windows(2).map(|w| (w[1] - w[0]) as usize).max().unwrap_or(0).max(ly.max_layers.min(ly.item_layers + 1));
                let lm_draws: Option<(&crate::lmaccum::LmScene, Vec<crate::lmaccum::SetDraw>)> = prm.lm_scene.as_ref().filter(|_| dir_lm.is_some()).map(|lm| {
                    let raster = crate::lmaccum::LmRasterCb::for_offset(di, 2048, 2048);
                    let cb = crate::lmaccum::SetCb { world_pw01_shadow: frame.world_pw01(), peel_dir: *d };
                    let world_box = if pi > 0 { prm.fitted_world_box } else { None };
                    (lm.as_ref(), (0..lm.meshes.len()).map(|m| crate::lmaccum::SetDraw { eid: 0, mesh: m, instance_first: lm.inst_first[m], instance_count: lm.inst_count[m], raster, cb, world_box }).collect())
                });
                let pw01 = frame.world_pw01();
                // THE BLOCKS FUSED (lmaccum::run_set_layers_par): the LM raster runs once per chunk of layers instead of once per
                // block — the same result (the last write of the highest block wins, tracked per pixel across chunks and peels);
                // LMTOOL_SET_LAYER_CHUNK=N (default 8) bounds the layer buffers held at once; LMTOOL_SET_PER_BLOCK=1 keeps the
                // block-by-block path
                let chunk: usize = std::env::var("LMTOOL_SET_LAYER_CHUNK").ok().and_then(|v| v.parse().ok()).unwrap_or(8).max(1);
                let per_block = std::env::var_os("LMTOOL_SET_PER_BLOCK").is_some() || lm_draws.is_none();
                let mut k = 0usize;
                while k < nl {
                    let k_end = if per_block { k + 1 } else { (k + chunk).min(nl) };
                    let bufs: Vec<(crate::passdiff::Buf, crate::passdiff::Buf)> = (k..k_end).map(|kk| (ly.colour_buf(kk, 0), crate::passdiff::Buf { w: ly.w, h: ly.h, channels: 1, data: ly.depth_image(kk, 0) })).collect();
                    if let (Some((lm, draws)), Some(dt)) = (&lm_draws, dir_lm.as_mut()) {
                        if per_block {
                            crate::lmaccum::run_set_block(&lm.meshes, &lm.instances, &lm.table, draws, &crate::lmaccum::LayerTargets { color: &bufs[0].0, depth: &bufs[0].1 }, crate::lmaccum::DepthCompare::Float, dt);
                        } else {
                            let layers: Vec<crate::lmaccum::LayerTargets> = bufs.iter().map(|(c, dd)| crate::lmaccum::LayerTargets { color: c, depth: dd }).collect();
                            crate::lmaccum::run_set_layers_par(&lm.meshes, &lm.instances, &lm.table, draws, &layers, crate::lmaccum::DepthCompare::Float, dt, &mut dir_best_k, dir_block_base + k);
                        }
                    }
                    // THE PROBES after every WORLD-peel layer (PS 17151 per block; the sky visibility after layer 1 and the
                    // AddAmbient dispatch after layer 0 for the sky sweep's upward directions) — probebake.rs
                    if want_probes {
                        if let Some(pb) = &prm.probe_bake {
                            for (j, (color, depth)) in bufs.iter().enumerate() {
                                pb.lock().unwrap().world_layer(k + j, &pw01, color, depth, *d, prm.sphere_dirs.len(), prm.sweep == 0);
                            }
                        }
                    }
                    k = k_end;
                }
                dir_block_base += nl;
                if (di < 2 || di % 32 == 0) && lm_draws.is_some() { eprintln!("lm-accumulate: direction {di} peel {pi}: {nl} blocks over the transcribed LM raster ({:.1}s)", tlm.elapsed().as_secs_f32()); }
            }
            if let (Some(ly), Some(dump), true) = (&layers, &prm.dump, want_dir_dump) {
                let mut dmp = dump.lock().unwrap();
                let skip = if prm.dome_layer { 1 } else { 0 };
                if prm.dome_layer {
                    // the sky layer the game draws into the peel targets before the geometry layers (the dome
                    // mesh at the far plane): our colour is the mood's Sky_p radiance along D, uniform
                    let mut e = crate::passdump::entry("peel_sky", format!("peel_sky/s{}/d{di:03}/p{pi}/color.bin", prm.sweep), "peel");
                    e.sweep = Some(prm.sweep); e.direction = Some(di as u32); e.peel = Some(pi as u32);
                    e.dir = Some(*d); e.frustum = Some(frame.frustum());
                    e.notes = Some("the environment render: the transcribed sky dome per pixel, black where the sea box / terrain is nearer (see peel_sky_depth)".into());
                    // the environment layer's depth (0 = the dome) — dumped as `peel_sky_depth`, and blackening the colour
                    let env_depth: Vec<f32> = (0..(ly.w * ly.h) as usize).map(|i| { let (a, c) = (ly.start[i] as usize, ly.start[i + 1] as usize); if a < c { ly.frags[a].d } else { 0.0 } }).collect();
                    let mut ed = crate::passdump::entry("peel_sky_depth", format!("peel_sky/s{}/d{di:03}/p{pi}/depth.bin", prm.sweep), "peel");
                    ed.sweep = Some(prm.sweep); ed.direction = Some(di as u32); ed.peel = Some(pi as u32);
                    ed.dir = Some(*d); ed.frustum = Some(frame.frustum()); ed.format = "R32_FLOAT".into();
                    ed.cleared_to = Some(serde_json::json!(0.0));
                    ed.notes = Some("the environment render's depth: 0 = the sky dome, else the nearest sea-box / terrain surface (D16 steps)".into());
                    dmp.write_f32(ed, ly.w, ly.h, 1, &env_depth).expect("dump peel_sky_depth");
                    let sky_img: Vec<[f32; 3]> = if prm.dome_exact && prm.sky_grad.is_some() {
                        let sg = prm.sky_grad.as_ref().unwrap();
                        let c = frame.frustum().center;
                        let mut img = vec![[0.0f32; 3]; (ly.w * ly.h) as usize];
                        let rows_per = (ly.h as usize / threads.max(1)).max(1);
                        std::thread::scope(|sc| {
                            for (ti, chunk) in img.chunks_mut(rows_per * ly.w as usize).enumerate() {
                                let y0 = ti * rows_per;
                                sc.spawn(move || {
                                    for (i, px) in chunk.iter_mut().enumerate() {
                                        let (xi, yi) = ((i % ly.w as usize) as u32, (y0 + i / ly.w as usize) as u32);
                                        let v = match dome_r {
                                            Some(r) => match r.at(xi, yi) { Some((uv, view)) => sg.sky_ps(uv, view), None => [0.0; 3] },
                                            None => { let q = frame.unproject(xi as f32 + 0.5, yi as f32 + 0.5, frame.z_from_z01(1.0)); sg.dome_radiance(q, *d, c) }
                                        };
                                        *px = prm.quant_peel.apply(v, prm.rounding);
                                    }
                                });
                            }
                        });
                        img
                    } else { vec![prm.quant_peel.apply(sky, prm.rounding); (ly.w * ly.h) as usize] };
                    let sky_img: Vec<[f32; 3]> = sky_img.into_iter().zip(env_depth.iter()).map(|(c, d)| if *d > 0.0 { [0.0; 3] } else { c }).collect();
                    dmp.write_rgb(e, ly.w, ly.h, &sky_img, prm.quant_peel, prm.rounding).expect("dump peel_sky");
                }
                let nl = (0..(ly.w * ly.h) as usize).map(|i| (ly.start[i + 1] - ly.start[i]) as usize).max().unwrap_or(0).saturating_sub(skip);
                for k in 0..nl {
                    let mut e = crate::passdump::entry("peel_depth", format!("peel_depth/s{}/d{di:03}/p{pi}/l{k:02}.bin", prm.sweep), "peel");
                    e.sweep = Some(prm.sweep); e.direction = Some(di as u32); e.layer = Some(k as u32); e.peel = Some(pi as u32);
                    e.dir = Some(*d); e.frustum = Some(frame.frustum()); e.format = "R32_FLOAT".into();
                    e.cleared_to = Some(serde_json::json!(1.0));
                    e.notes = Some(format!("stored depth = z01 + D3D bias ({}, {:.2}) on D{}, quantised to the target; layer 0 = the farthest real surface (the sky fills the uncovered texels through the accumulate, as the game's first accumulate does); clear 1.0 = near", prm.depth_bias.0, prm.depth_bias.1, prm.depth_bits));
                    dmp.write_f32(e, ly.w, ly.h, 1, &ly.depth_image(k, skip)).expect("dump peel_depth");
                    let mut e = crate::passdump::entry("peel_color", format!("peel_color/s{}/d{di:03}/p{pi}/l{k:02}.bin", prm.sweep), "peel");
                    e.sweep = Some(prm.sweep); e.direction = Some(di as u32); e.layer = Some(k as u32); e.peel = Some(pi as u32);
                    e.dir = Some(*d); e.frustum = Some(frame.frustum());
                    e.cleared_to = Some(serde_json::json!([0.0, 0.0, 0.0]));
                    e.notes = Some("ILightInput of the layer's surface at the pixel centre (front faces; back faces black)".into());
                    dmp.write_rgb(e, ly.w, ly.h, &ly.colour_image(k, skip), prm.quant_peel, prm.rounding).expect("dump peel_color");
                }
            }
            if !dbg_subs.is_empty() && di < 40 {
                if let Ok(k) = std::env::var("LMTOOL_PEEL_DEBUG_ITEM") {
                    let k: u32 = k.parse().unwrap_or(0);
                    let n = ab.iter().filter(|f| bvh.tris[f.tri as usize].inst == k).count();
                    eprintln!("  dir {di} peel {pi}: instance {k} has {n} fragments in the A-buffer (res {}×{}); frame r ({:.2},{:.2},{:.2}) u ({:.2},{:.2},{:.2}) s0 {:.1} t0 {:.1} scale {:.3}", frame.res, frame.res_y, frame.r[0], frame.r[1], frame.r[2], frame.u[0], frame.u[1], frame.u[2], frame.s0, frame.t0, frame.scale);
                }
            }
            prof::add(&prof::DUMP, td);
            let tg = std::time::Instant::now();
            // THE GATHER of this peel: per sub-sample the layer it reads (game mode: the game's lookup;
            // else the port's A-buffer walk), written into `sel` (last peel wins where it has a layer)
            let sel_ptr = sel.as_mut_ptr() as usize;
            let occl_ptr = occl.as_mut_ptr() as usize;
            let n_chunks = (range.len() + chunk - 1) / chunk;
            crate::pool::pool().run(n_chunks, |ci| {
                let ch = &range[ci * chunk..((ci + 1) * chunk).min(range.len())];
                {
                    let ab = &ab;
                    let frame = &frame;
                    let subs = cur;
                    let shadow = shadow.as_ref();
                    let layers = layers.as_ref();
                    {
                        for &i in ch {
                            let s = &subs[i as usize];
                            let ndd = dot(s.n, *d);
                            if ndd <= 0.0 {
                                continue;
                            }
                            // THE TILE PASS'S WORLD-XZ CLIP (RE 7: LmRasterPosNrm_Inst_v's ClipWorldBoxXZ permutation emits
                            // SV_ClipDistance0 = (wx − MinX, wz − MinZ, MaxX − wx, MaxZ − wz) from the tile record's
                            // {cx ± hx, cz ± hz}; the hardware drops every LM fragment whose world XZ lies outside the
                            // tile's cell — no y test, distance 0 kept; the world pass has the clip off): so a texel
                            // reads exactly the tile whose cell holds its world position
                            if let Some(Some(b)) = tile_clip.get(pi) {
                                if !(s.p[0] - b[0] >= 0.0 && s.p[2] - b[1] >= 0.0 && b[2] - s.p[0] >= 0.0 && b[3] - s.p[2] >= 0.0) {
                                    continue;
                                }
                            }
                            let (x, y, z) = frame.project(s.p);
                            let mut hit: Option<([f32; 3], bool)> = None;
                            if let (Some(ly), true) = (layers, prm.game_peel) {
                                // THE GAME'S LOOKUP (LmILightDir_Set_p): point-sample the layer targets (the Bias rows'
                                // inset registers to the 1-px-inset render viewport, so no extra shift) at the texel's own
                                // reversed depth; the nearest layer still beyond the texel by its stored bias gives the
                                // colour; nothing → this peel leaves the texel as it was
                                let (px, py) = (lookup_pixel(x, ly.w, prm.peel_inset), lookup_pixel(y, ly.h, prm.peel_inset));
                                let z01 = frame.z01(z);
                                if let Some(&(dd, cc, nn)) = game_dbg.as_ref() {
                                    if dd == di && cc == s.chart as usize && dbg_printed.load(std::sync::atomic::Ordering::Relaxed) < nn {
                                        let k = dbg_printed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        if k < nn {
                                            let list = ly.at(px, py);
                                            let selq = select_layer(list, z01).map(|f| format!("d {:.5} rgb ({:.3},{:.3},{:.3})", f.d, f.rgb[0], f.rgb[1], f.rgb[2])).unwrap_or("NONE".into());
                                            eprintln!("game-peel debug: dir {di} peel {pi} d ({:.3},{:.3},{:.3}) chart {} sub ({},{}) p ({:.2},{:.2},{:.2}) n ({:.2},{:.2},{:.2}) ndd {ndd:.3}: px ({x:.2},{y:.2}) → lookup ({px},{py}), z_port {z:.3} z01 {z01:.5}; {} layers: [{}]; selected {selq}", d[0], d[1], d[2], s.chart, s.sx, s.sy, s.p[0], s.p[1], s.p[2], s.n[0], s.n[1], s.n[2], list.len(), list.iter().map(|f| format!("d {:.5} rgb ({:.2},{:.2},{:.2})", f.d, f.rgb[0], f.rgb[1], f.rgb[2])).collect::<Vec<_>>().join(" | "));
                                        }
                                    }
                                }
                                if px < ly.w && py < ly.h && z01 >= 0.0 && z01 <= 1.0 {
                                    // (a texel outside this peel's frustum reads nothing from it)
                                    match select_layer(ly.at(px, py), z01) {
                                        Some(f) if f.d > 0.0 || !prm.dome_layer => hit = Some((f.rgb, true)),
                                        Some(_) => hit = Some((dome_px(frame, dome_r, px, py), false)), // the dome: the sky at this pixel
                                        None => {}
                                    }
                                }
                            } else {
                                let (xi, yi) = (x.round() as i64, y.round() as i64);
                                if xi >= 0 && yi >= 0 && xi < frame.res as i64 && yi < frame.res_y as i64 {
                                    let list = ab.at(xi as u32, yi as u32);
                                    // the first surface along D beyond the texel by more than the rasteriser's depth bias:
                                    // one depth unit plus one pixel's worth of the FRAGMENT's own depth slope (a sea plane half a
                                    // metre under a deck counts, a coplanar neighbour of the texel's own surface at grazing
                                    // angle does not); fragments are sorted by z, the walk goes from the texel outward
                                    let limit_min = z - bias;
                                    let mut lo = 0usize;
                                    let mut hi = list.len();
                                    while lo < hi {
                                        let mid = (lo + hi) / 2;
                                        if list[mid].z < limit_min { lo = mid + 1 } else { hi = mid }
                                    }
                                    let px_m = frame.pixel_m();
                                    let mut k = lo;
                                    while k > 0 {
                                        k -= 1;
                                        let f = list[k];
                                        if f.tri == s.own_tri {
                                            continue;
                                        }
                                        let wt = &bvh.tris[f.tri as usize];
                                        let nf = norm(cross(wt.e1, wt.e2));
                                        let c = dot(nf, *d).abs().max(1e-3);
                                        let slope_f = ((1.0 - c * c).max(0.0).sqrt() / c).min(64.0);
                                        if f.z >= z - bias.max(px_m * slope_f) {
                                            continue;
                                        }
                                        hit = Some((fragment_radiance(scene, bvh, prm, shadow, frame, f.tri, *d, [s.p[0] + d[0] * (z - f.z), s.p[1] + d[1] * (z - f.z), s.p[2] + d[2] * (z - f.z)], sun_bias), true));
                                        break;
                                    }
                                }
                            }
                            if let Some((l, occluded)) = hit {
                                // SAFETY: each chunk owns a disjoint set of indices i; no other thread touches sel[i] / occl[i]
                                let slot = unsafe { &mut *(sel_ptr as *mut [f32; 3]).add(i as usize) };
                                *slot = prm.quant_ilightdir.apply(l, prm.rounding);
                                let o = unsafe { &mut *(occl_ptr as *mut bool).add(i as usize) };
                                *o = occluded;
                            }
                        }
                    }
                }
            });
            prof::add(&prof::GATHER, tg);
        }
        } // (the live direction)
        let t_contrib = std::time::Instant::now();
        if let (Some(tx), true) = (&contrib_tx, replay.is_none()) {
            // THE CONTRIBUTION of this direction: sel of the facing sub-samples, the occl bits, the probes
            let mut c = crate::contrib::DirContrib { sweep: prm.sweep, di: di as u32, n_subs: cur.len() as u32, ..Default::default() };
            c.occl_bits = vec![0u64; (cur.len() + 63) / 64];
            for (i, s) in cur.iter().enumerate() {
                if dot(s.n, *d) > 0.0 { c.sel.push(sel[i]); }
                if occl[i] { c.occl_bits[i >> 6] |= 1u64 << (i & 63); }
            }
            if let Some(pb) = &prm.probe_bake {
                let mut pb = pb.lock().unwrap();
                c.probe_cur = pb.export_cur();
                c.sky_adds = pb.sky_log.replace(Vec::new()).unwrap_or_default();
            }
            tx.send(c).expect("contribution writer");
        }
        prof::add(&prof::CONTRIB, t_contrib);
        let t_build = t_build_total;
        let ta = std::time::Instant::now();
        // THE PROBES: the direction's two folds (PS 1112) — the game issues them after the H-basis, the order is immaterial
        // (they read the direction's volume, which the fitted peel and the H-basis never touch)
        if let Some(pb) = &prm.probe_bake { pb.lock().unwrap().end_direction(*d, prm.sphere_dirs.len(), prm.sweep); }
        // THE ACCUMULATE (LmLBumpILighting): E += 4/N · max(0, n·D) · TMapILightDir[texel]
        // the transcribed H-basis accumulate of this direction, and the comparison with the capture's banked buffers
        if let (Some(lm), Some(dt), Some(hb)) = (&prm.lm_scene, &dir_lm, hb_lm.as_mut()) {
            let n_full = prm.sphere_dirs.len().max(1) as f32;
            let cb = crate::lmaccum::HbCb { peel_dir: *d, inv_dir_count: 1.0 / n_full };
            let raster = crate::lmaccum::LmRasterCb::for_offset(di, 2048, 2048);
            let draws: Vec<crate::lmaccum::HbDraw> = (0..lm.meshes.len()).map(|m| crate::lmaccum::HbDraw { eid: 0, mesh: m, instance_first: lm.inst_first[m], instance_count: lm.inst_count[m], raster, cb }).collect();
            let mut owner = vec![0u8; 2048 * 2048];
            crate::lmaccum::run_hbasis_probe(&lm.meshes, &lm.instances, &lm.table, &draws, dt, hb, crate::sunpass::BlendModel::TruncSrcRoundSum, Some(&mut owner), None);
            // (the capture's sweep-1 snapshots come from another run (pwc6) with its own MRT history: sweep 0 only)
            if let Some((root, entries)) = prm.hbasis_game.as_ref().filter(|_| prm.sweep == 0) {
                // the game's ilightdir at its H-basis draw and its four MRTs after this direction (by the true issue index)
                let sweep = prm.sweep;
                let cap = |pass: &str| entries.iter().filter(|e| e.pass == pass && e.banked && e.sweep_direction_index == Some(di as u32) && e.sweep.unwrap_or(0) == sweep).max_by_key(|e| (e.frame, e.eid_last));
                // the object names by what the mesh IS (its index / instance counts), not by its position in the scene — the
                // capture's LM scene comes pad, wall, vegetation, tiles; the map-built one in item order
                let names: Vec<String> = lm.meshes.iter().enumerate().map(|(k, m)| if lm.inst_count[k] > 1 { "tiles".to_string() } else { match m.indices.len() { 24 => "pad".to_string(), 12 => "wall".to_string(), 5751 => "vegetation".to_string(), n => format!("mesh{k}({n} idx)") } }).collect();
                if let Some(ge) = cap("ilightdir_final") {
                    if let Ok(g) = ge.load(root) {
                        let c = crate::lmaccum::compare_dir(dt, &g);
                        for (px, py) in [(1200u32, 200u32), (1200, 400), (300, 800), (300, 1500), (700, 200), (1700, 900), (400, 1900)] { let o = dt.rgb(px, py); eprintln!("lm-accumulate:   probe ({px},{py}): ours ({:.4},{:.4},{:.4}) game ({:.4},{:.4},{:.4})", o[0], o[1], o[2], g.get(px, py, 0), g.get(px, py, 1), g.get(px, py, 2)); }
                        // per object
                        let mut per = String::new();
                        for (mi, nm) in names.iter().enumerate() {
                            let (mut n, mut ex) = (0usize, 0usize);
                            for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] != mi as u8 + 1 { continue; } let o = dt.rgb(x, y); let gg = [g.get(x, y, 0), g.get(x, y, 1), g.get(x, y, 2)]; if o == [0.0; 3] && gg == [0.0; 3] { continue; } n += 1; if crate::gpufmt::pack_r11g11b10(gg, crate::gpufmt::Rounding::Truncate) == dt.px[i] { ex += 1; } } }
                            per += &format!(" {nm} {ex}/{n} ({:.2} %)", 100.0 * ex as f64 / n.max(1) as f64);
                        }
                        let row = format!("direction {di} (D {:.3},{:.3},{:.3}) ilightdir vs the capture: {} |{per}", d[0], d[1], d[2], crate::lmaccum::fmt_dircmp(&c));
                        eprintln!("lm-accumulate: {row}");
                        lm_rows.push(row);
                    }
                }
                let mrts: Vec<Option<&crate::lmaccum::CapEntry>> = (0..4).map(|m| cap(&format!("hbasis{m}"))).collect();
                if mrts.iter().all(|e| e.is_some()) {
                    let gs: Vec<crate::passdiff::Buf> = mrts.iter().map(|e| e.unwrap().load(root).expect("captured MRT")).collect();
                    let mut per = String::new();
                    for (mi, nm) in names.iter().enumerate() {
                        let (mut n, mut ex, mut u1) = (0usize, 0usize, 0usize);
                        for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] != mi as u8 + 1 { continue; } for m in 0..4 { for ch in 0..3 { let gg = gs[m].get(x, y, ch as u32); let o = hb.mrt[m][i][ch]; if gg == 0.0 && o == 0.0 { continue; } n += 1; let dd = (o - gg).abs(); if dd == 0.0 { ex += 1; } else { let ulp = (crate::gpufmt::decode_f16(crate::gpufmt::encode_f16(gg, crate::gpufmt::Rounding::NearestEven).wrapping_add(1)) - gg).abs(); if dd <= ulp * 1.001 { u1 += 1; } } } } } }
                        per += &format!(" {nm} exact {ex}/{n} ({:.2} %), 1 ulp {u1}", 100.0 * ex as f64 / n.max(1) as f64);
                    }
                    let (mut n, mut within) = (0usize, 0usize);
                    for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] == 0 { continue; } for ch in 0..3 { let gg = gs[0].get(x, y, ch as u32); let o = hb.mrt[0][i][ch]; if gg == 0.0 && o == 0.0 { continue; } n += 1; if (o - gg).abs() <= 0.02 * o.abs().max(gg.abs()) + 1e-5 { within += 1; } } } }
                    let row = format!("direction {di} H-basis C0..C3 vs the capture after it: C0 within 2 %: {within}/{n} ({:.2} %);{per}", 100.0 * within as f64 / n.max(1) as f64);
                    eprintln!("lm-accumulate: {row}");
                    lm_rows.push(row);
                    // per MRT (the signed C1 / C3 residue): exact / within 1 f16 ulp / beyond, and of the beyond values the sign flips,
                    // the near-zero captured values (|g| < 2^-7), the rel-2 % misses, the largest |Δ|, per object class
                    for m in 0..4 {
                        let mut line = String::new();
                        for (mi, nm) in names.iter().enumerate() {
                            let (mut n, mut ex, mut u1, mut beyond, mut flips, mut near0, mut rel2, mut mx) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0f32);
                            let mut worst = (0u32, 0u32, 0u32, 0f32, 0f32);
                            for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] != mi as u8 + 1 { continue; } for ch in 0..3 { let gg = gs[m].get(x, y, ch as u32); let o = hb.mrt[m][i][ch]; n += 1; if o == gg { ex += 1; continue; } let d = (o - gg).abs(); let ulp = crate::gpufmt::f16_ulp(gg.abs().max(o.abs())); if d <= ulp { u1 += 1; continue; } beyond += 1; if o * gg < 0.0 { flips += 1; } if gg.abs() < 0.0078125 { near0 += 1; } if d > 0.02 * gg.abs() { rel2 += 1; } if d > mx { mx = d; worst = (x, y, ch as u32, gg, o); } } } }
                            line += &format!(" {nm}: {ex} exact / {u1} ±1 ulp / {beyond} beyond (of {n}; flips {flips}, |g|<2^-7 {near0}, rel>2% {rel2}, max |Δ| {mx:.5} at ({}, {}) ch {} g {:.5} o {:.5});", worst.0, worst.1, worst.2, worst.3, worst.4);
                        }
                        eprintln!("lm-accumulate:   MRT {m} after direction {di}:{line}");
                    }
                }
            }
        }
        let acc_ptrs: Vec<usize> = acc_tex.iter_mut().map(|v| v.as_mut_ptr() as usize).collect();
        let cover_ptrs: Vec<usize> = cover.iter_mut().map(|v| v.as_mut_ptr() as usize).collect();
        let mut ldir: Vec<[f32; 3]> = if want_dir_dump { vec![[0.0; 3]; cur.len()] } else { Vec::new() };
        let ldir_ptr = ldir.as_mut_ptr() as usize;
        let ldir_on = want_dir_dump;
        let n_chunks = (range.len() + chunk - 1) / chunk;
        crate::pool::pool().run(n_chunks, |ci| {
            let ch = &range[ci * chunk..((ci + 1) * chunk).min(range.len())];
            {
                let subs = cur;
                let sel = &sel;
                let occl = &occl;
                let acc_ptrs = &acc_ptrs;
                let cover_ptrs = &cover_ptrs;
                let pix_of = &pix_of;
                {
                    for &i in ch {
                        let s = &subs[i as usize];
                        let (pc, pi) = pix_of(s);
                        // (a covered pixel counts whether or not it faces D: the game's draw writes alpha += 1/N for it)
                        // SAFETY: a set holds at most one sub-sample per pixel; chunks own disjoint sub indices
                        unsafe { *(cover_ptrs[pc] as *mut u16).add(pi) += 1; }
                        let ndd = dot(s.n, *d);
                        // (the H-basis accumulate draws every texel: LmILightDir_Set discards the texels facing away,
                        // so their ILightDir stays the clear = 0 and P(sz < 0)·0 adds nothing — same as skipping)
                        if ndd <= 0.0 {
                            continue;
                        }
                        let l = sel[i as usize];
                        // LMTOOL_SKY_NO_COS=1: the sky pass (AddSkyVisibility) without the receiver's cosine — the
                        // per-direction constant 4·w·d.y·SkyFactor times the visibility only (a hypothesis under test)
                        let hit_sky = !occl[i as usize];
                        // THE GAME'S H-BASIS ACCUMULATE (PS 17536 read off the capture, 2026-09-24): C0 += (4π/N)·L·P(n·D)
                        // with P(s) = 0.093506·(3s² − 1) + 0.398928·s + 0.199472 (no clamp; P(1) = π/4, ∫₀¹ P = 0.399,
                        // P ≈ 0 below the horizon), i.e. the H-basis projection of the clamped cosine; the constant
                        // coefficient is kept in the port's units through κ = 1/√(2π) (a uniform sky gives E = L as
                        // with the RNM-style Σ 4/N·max(0,n·D)·L, which `--accum rnm` restores). Below-horizon
                        // directions contribute slightly negative weights, as in the game.
                        let w = if prm.accum_hbasis {
                            let sz = ndd;
                            let pz = 0.093506 * (3.0 * sz * sz - 1.0) + 0.398928 * sz + 0.199472;
                            (std::f32::consts::PI * scale) * pz * prm.hbasis_kappa
                        } else if hit_sky && sky_no_cos { scale } else { scale * ndd };
                        if ldir_on {
                            // SAFETY: as for acc — disjoint indices per chunk
                            let lslot = unsafe { &mut *(ldir_ptr as *mut [f32; 3]).add(i as usize) };
                            *lslot = l;
                        }
                        // SAFETY: as above — one sub-sample per pixel per set
                        let slot = unsafe { &mut *(acc_ptrs[pc] as *mut [f32; 3]).add(pi) };
                        for c in 0..3 {
                            slot[c] += w * l[c];
                        }
                        // the accumulation target's own storage (an f16 target rounds after every add)
                        *slot = prm.quant_accum.apply(*slot, prm.rounding);
                    }
                }
            }
        });
        prof::add(&prof::ACCUM, ta);
        let ab_len = frag_total;
        let tsn = std::time::Instant::now();
        // the harness: the accumulation target after this direction (the game's H-basis MRT snapshot after
        // its k-th direction, `--dump-lightsum-after`); direction = the index in the sweep's issue order
        if let Some(dump) = &prm.dump {
            if prm.lightsum_after.contains(&(di as u32)) {
                let mut dmp = dump.lock().unwrap();
                let imgs = &acc_tex;
                for ii in 0..chart_meta.len() {
                    let (cw, ch) = chart_meta[ii];
                    let mut e = crate::passdump::entry("lightsum", chart_file("lightsum", Some(prm.sweep), Some(di as u32), ii), "chart_ss");
                    e.chart = Some(chart_ref(ii)); e.sweep = Some(prm.sweep); e.direction = Some(di as u32); e.dir = Some(*d);
                    e.notes = Some(format!("the accumulation target after direction {di} of the sweep (issue order) — {}", if prm.accum_hbasis { format!("H-basis C0 × κ {}", prm.hbasis_kappa) } else { "RNM E".into() }));
                    dmp.write_rgb(e, cw * 2 * ss_eff, ch * 2 * ss_eff, &imgs[ii], prm.quant_accum, prm.rounding).expect("dump lightsum snapshot");
                }
            }
        }
        prof::add(&prof::SNAP, tsn);
        if want_dir_dump {
            if let Some(dump) = &prm.dump {
                let mut dmp = dump.lock().unwrap();
                let mut imgs: Vec<Vec<[f32; 3]>> = chart_meta.iter().map(|(cw, ch)| vec![[0.0f32; 3]; (cw * 2 * ss_eff * ch * 2 * ss_eff) as usize]).collect();
                for (i, s) in cur.iter().enumerate() {
                    let gw = chart_meta[s.chart as usize].0 * 2 * ss_eff;
                    imgs[s.chart as usize][(s.sy * gw + s.sx) as usize] = ldir[i];
                }
                for ii in 0..chart_meta.len() {
                    let (cw, ch) = chart_meta[ii];
                    let mut e = crate::passdump::entry("ilightdir", chart_file("ilightdir", Some(prm.sweep), Some(di as u32), ii), "chart_ss");
                    e.chart = Some(chart_ref(ii)); e.sweep = Some(prm.sweep); e.direction = Some(di as u32); e.dir = Some(*d);
                    e.cleared_to = Some(serde_json::json!([0.0, 0.0, 0.0]));
                    e.notes = Some(format!("incoming radiance from D per sub-sample after the direction's {} peel(s) (0 where n·D ≤ 0 or uncovered); the accumulate adds 4/N·max(0,n·D) × this", peels.len()));
                    dmp.write_rgb(e, cw * 2 * ss_eff, ch * 2 * ss_eff, &imgs[ii], prm.quant_ilightdir, prm.rounding).expect("dump ilightdir");
                }
            }
        }
        if di % 64 == 0 || di + 1 == n_dirs {
            eprintln!("peel: direction {}/{} ({} peel(s), {} fragments, build {:.2}s, gather {:.2}s; {:.1}s)", di + 1, n_dirs, peels.len(), ab_len, t_build, tb.elapsed().as_secs_f32() - t_build, t0.elapsed().as_secs_f32());
        }
        prof::add(&prof::DIR, t_dir);
    }
    // 5. resolve: per colour texel the mean over its covered sub-samples
    let mut out: Vec<ChartBake> = scene
        .instances
        .iter()
        .enumerate()
        .map(|(ii, inst)| {
            let (w, h) = chart_meta[ii];
            ChartBake { item: inst.item, w, h, rgb: vec![[0.0; 3]; (w * h) as usize], rgb1: Vec::new(), covered: vec![false; (w * h) as usize], sun_vis: 0.0, sky_vis: 0.0, rgb_irr: Vec::new() }
        })
        .collect();
    let mut counts: Vec<Vec<u16>> = chart_meta.iter().map(|(w, h)| vec![0u16; (w * h) as usize]).collect();
    // the vegetation cards' texels are PRELIT: the editor writes light × the leaf colour into them (the hill
    // test map, 2026-09-23 23:10Z: the card cells of a bush-bearing hill's chart are saturated yellow-green
    // at Day and orange at Sunset, B ≈ 0, where a bare irradiance would be sky-coloured) — the card LOD
    // shaders draw the lightmap as their colour. LMTOOL_CARD_PRELIT=0 stores the plain irradiance.
    let card_prelit = std::env::var("LMTOOL_CARD_PRELIT").map(|v| v != "0").unwrap_or(true);
    let card_open = std::env::var("LMTOOL_CARD_OPEN").map(|v| v != "0").unwrap_or(true);
    // the plain irradiance alongside (the bounce of the next sweep reads THIS, not the prelit value — with
    // the prelit fed back, tiny 16's 21 M card triangles lit the whole map +40 %)
    let mut irr: Vec<Vec<[f32; 3]>> = chart_meta.iter().map(|(w, h)| vec![[0.0; 3]; (w * h) as usize]).collect();
    for (i, s) in subs.iter().enumerate() {
        let c = &mut out[s.chart as usize];
        let t = s.texel as usize;
        let acc_i = { let (pc, pi) = pix_of(s); acc_tex[pc][pi] };
        let _ = i;
        for k in 0..3 { irr[s.chart as usize][t][k] += acc_i[k]; }
        let mut v = acc_i;
        if card_prelit {
            let wt = &bvh.tris[s.own_tri as usize];
            if wt.alpha != u16::MAX && wt.inst != DECOR_INST {
                let inst = &scene.instances[wt.inst as usize];
                let m = &scene.models[inst.model];
                if let Some(tri) = m.tris.get(wt.tri as usize) {
                    if let Some(file) = m.alpha_tex.get(tri.alpha as usize) {
                        if let Some(alb) = scene.card_albedo.get(file) {
                            // the vegetation path lights the cards WITHOUT the peel's occlusion (the hill
                            // test map: the card texels read 0.53 at Day under a canopy whose slopes read
                            // 0.19 — the open sky × the leaf colour; INFERRED from RE 4's per-vertex tree
                            // lighting): the card's texel = the open-sky irradiance for its normal (both
                            // sides: a card is thin) × the leaf colour. LMTOOL_CARD_OPEN=0 keeps the gathered E.
                            if card_open {
                                let n = s.n;
                                let mut e = [0f32; 3];
                                let nd = dirs.len().max(1) as f32;
                                for d in dirs.iter() {
                                    if d[1] <= 0.0 { continue; }
                                    let c = dot(n, *d).abs();
                                    let sky = sky_radiance(prm, *d);
                                    for k in 0..3 { e[k] += 4.0 / nd * c * sky[k]; }
                                }
                                v = e;
                            }
                            for k in 0..3 { v[k] *= alb[k]; }
                        }
                    }
                }
            }
        }
        for k in 0..3 {
            c.rgb[t][k] += v[k];
        }
        counts[s.chart as usize][t] += 1;
        c.covered[t] = true;
    }
    for (ci, c) in out.iter_mut().enumerate() {
        for t in 0..c.rgb.len() {
            let n = counts[ci][t];
            if n > 0 {
                for k in 0..3 {
                    c.rgb[t][k] /= n as f32;
                    irr[ci][t][k] /= n as f32;
                }
            }
        }
        if card_prelit { c.rgb_irr = std::mem::take(&mut irr[ci]); }
    }
    if std::env::var_os("LMTOOL_PROFILE").is_some() || prm.profile {
        prof::report(&format!("sweep {}", prm.sweep), t0.elapsed().as_secs_f32());
    }
    // --- the differential harness: this sweep's accumulation target and its resolve ---
    if let Some(dump) = &prm.dump {
        let mut dmp = dump.lock().unwrap();
        let imgs = &acc_tex;
        for ii in 0..chart_meta.len() {
            let (cw, ch) = chart_meta[ii];
            let mut e = crate::passdump::entry("lightsum", chart_file("lightsum", Some(prm.sweep), None, ii), "chart_ss");
            e.chart = Some(chart_ref(ii)); e.sweep = Some(prm.sweep); e.direction = Some(dirs.len() as u32 - 1);
            e.notes = Some(format!("the accumulation target after the sweep's {} directions per sub-sample (uncovered = 0): {}", dirs.len(), if prm.accum_hbasis { format!("H-basis C0 = Σ_D (4π/N)·P(n·D)·L_D × κ {}", prm.hbasis_kappa) } else { "E = Σ_D 4/N·max(0,n·D)·L_D".into() }));
            dmp.write_rgb(e, cw * 2 * ss_eff, ch * 2 * ss_eff, &imgs[ii], prm.quant_accum, prm.rounding).expect("dump lightsum");
            let mut e = crate::passdump::entry("lightsum_resolved", chart_file("lightsum_resolved", Some(prm.sweep), None, ii), "chart");
            e.chart = Some(chart_ref(ii)); e.sweep = Some(prm.sweep);
            e.notes = Some("the ss resolve: mean over the texel's covered sub-samples (LmSSResolve + LmSSNormWithA), at stored resolution".into());
            let rgb: Vec<[f32; 3]> = if out[ii].rgb_irr.is_empty() { out[ii].rgb.clone() } else { out[ii].rgb_irr.clone() };
            dmp.write_rgb(e, cw, ch, &rgb, crate::gpufmt::Quant::None, prm.rounding).expect("dump lightsum_resolved");
        }
        dmp.finish().expect("write MANIFEST.json");
    }
    let acc_at = |s: &SubSample| -> [f32; 3] { let (pc, pi) = pix_of(s); acc_tex[pc][pi] };
    if std::env::var_os("LMTOOL_PEEL_DEBUG_BLACK").is_some() {
        // re-gather two black sub-samples of chart 0 with prints
        let black: Vec<usize> = subs.iter().enumerate().filter(|(_, s)| s.chart == 0 && { let a = acc_at(s); a[0] + a[1] + a[2] < 1e-6 }).map(|(i, _)| i).step_by(100000).take(2).collect();
        for &i in &black {
            let s = &subs[i];
            eprintln!("peel debug BLACK sub {i}: p ({:.2},{:.2},{:.2}) n ({:.2},{:.2},{:.2}) group {} own_tri {}", s.p[0], s.p[1], s.p[2], s.n[0], s.n[1], s.n[2], s.group, s.own_tri);
            for (di, d) in dirs.iter().enumerate() {
                if di % groups != s.group as usize { continue; }
                let ndd = dot(s.n, *d);
                let frame = PeelFrame::new(*d, bmin, bmax, prm.peel_res);
                let (x, y, z) = frame.project(s.p);
                let (xi, yi) = (x.round() as i64, y.round() as i64);
                if ndd <= 0.0 { eprintln!("   dir {di} d ({:.2},{:.2},{:.2}) ndd {ndd:.2}: facing away", d[0], d[1], d[2]); continue; }
                let ab = build_abuffer(&bvh.tris, &frame, threads);
                let list = if xi >= 0 && yi >= 0 && xi < frame.res as i64 && yi < frame.res as i64 { ab.at(xi as u32, yi as u32) } else { &[] };
                let bias = bias_m;
                let front: Vec<String> = list.iter().filter(|f| f.z < z - bias).map(|f| { let wt = &bvh.tris[f.tri as usize]; format!("tri {} inst {} z {:.2}{}", f.tri, wt.inst, f.z, if f.tri == s.own_tri { " OWN" } else { "" }) }).collect();
                eprintln!("   dir {di} d ({:.2},{:.2},{:.2}) ndd {ndd:.2}: pixel ({xi},{yi}) z {z:.2}; {} frags [{}]; in front: [{}]", d[0], d[1], d[2], list.len(), list.iter().map(|f| format!("{:.2}", f.z)).collect::<Vec<_>>().join(","), front.join(" | "));
            }
        }
    }
    if std::env::var_os("LMTOOL_PEEL_DEBUG").is_some() {
        for (ci, c) in out.iter().enumerate().take(4) {
            let black = c.rgb.iter().zip(counts[ci].iter()).filter(|(v, n)| **n > 0 && v[0] + v[1] + v[2] < 1e-4).count();
            let covered = counts[ci].iter().filter(|n| **n > 0).count();
            // normals of the black texels' sub-samples
            let mut nsum = [0f64; 3];
            let mut nn = 0usize;
            for s in subs.iter() { if s.chart as usize == ci && { let a = acc_at(s); a[0] + a[1] + a[2] < 1e-6 } { for k in 0..3 { nsum[k] += s.n[k] as f64; } nn += 1; } }
            eprintln!("peel debug: chart {ci} {}×{}: {covered} covered texels, {black} black; {nn} zero sub-samples, mean normal ({:.2},{:.2},{:.2})", c.w, c.h, nsum[0] / nn.max(1) as f64, nsum[1] / nn.max(1) as f64, nsum[2] / nn.max(1) as f64);
        }
    }
    for &i in &dbg_subs {
        let s = &subs[i as usize];
        let c = &out[s.chart as usize];
        eprintln!("peel debug: sub {i} acc {:?} → chart {} texel {} ({}×{}) rgb {:?} over {} subs", acc_at(s), s.chart, s.texel, c.w, c.h, c.rgb[s.texel as usize], counts[s.chart as usize][s.texel as usize]);
    }
    if !lm_rows.is_empty() {
        eprintln!("lm-accumulate: THE TRANSCRIBED ROWS 7–9 OVER OUR PEEL LAYERS vs the capture (sweep {}):", prm.sweep);
        for r in &lm_rows { eprintln!("  {r}"); }
    }
    drop(contrib_tx);
    for h in contrib_writers { h.join().expect("contribution writer"); }
    if prm.merge_contrib.is_some() || prm.merge_ranges.is_some() { eprintln!("merge: sweep {}: {n_replayed} directions replayed in issue order{}", prm.sweep, if let Some((a, b)) = prm.dir_range { format!(" around the live range {a}..{b}") } else { String::new() }); }
    if let Some(dir) = &prm.contrib_out {
        if pack_mode {
            let entries = std::mem::take(&mut *pack_entries.lock().unwrap());
            let bytes: usize = entries.values().map(|b| b.len()).sum();
            let t_pack = std::time::Instant::now();
            crate::contrib::ContribPack { sweep: prm.sweep, entries }.write(dir).expect("write contribution pack");
            eprintln!("contrib-out: sweep {}: the directions {:?} packed into {} ({:.1} MB, {:.1}s)", prm.sweep, prm.dir_range.unwrap_or((0, dirs.len())), dir.display(), bytes as f64 / 1e6, t_pack.elapsed().as_secs_f32());
        } else {
            eprintln!("contrib-out: sweep {}: the directions {:?} written to {}", prm.sweep, prm.dir_range.unwrap_or((0, dirs.len())), dir.display());
        }
    }
    // the sweep's transcribed H-basis MRTs to the caller (the sweep-transition chain / the finalisation)
    if let (Some(slot), Some(hb)) = (&prm.hb_out, hb_lm.take()) {
        *slot.0.lock().unwrap() = Some(hb);
    }
    if let Some(il) = &prm.ilatlas {
        eprintln!("ilightinput atlas (sweep {}): {} fragments coloured from the atlas, {} without a lightmap coordinate (the port's model)", prm.sweep, il.atlas.hits.load(std::sync::atomic::Ordering::Relaxed), il.atlas.misses.load(std::sync::atomic::Ordering::Relaxed));
    }
    eprintln!("peel: done, {} directions over {} sub-samples ({:.1}s); fragment radiance calls {}, facing the sun {}, lit {}", n_dirs, subs.len(), t0.elapsed().as_secs_f32(), SUN_STATS[0].load(std::sync::atomic::Ordering::Relaxed), SUN_STATS[1].load(std::sync::atomic::Ordering::Relaxed), SUN_STATS[2].load(std::sync::atomic::Ordering::Relaxed));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_inset_squeezes_towards_the_centre_by_one_texel_at_the_edges() {
        // u = 0.5 + (u − 0.5)(w − 2)/w: the render-space edge pixels read one pixel inward, the centre stays
        let w = 2048;
        assert_eq!(lookup_pixel(0.0, w, true), 1);
        assert_eq!(lookup_pixel(2047.9, w, true), 2046);
        assert_eq!(lookup_pixel(1024.0, w, true), 1024);
        assert_eq!(lookup_pixel(1024.0, w, false), 1024);
        assert_eq!(lookup_pixel(0.0, w, false), 0);
        assert_eq!(lookup_pixel(-3.0, w, false), 0, "clamped");
        assert_eq!(lookup_pixel(5000.0, w, false), 2047, "clamped");
        // without the inset the pixel index is the floor (pixel k spans [k, k+1)), not the rounding
        assert_eq!(lookup_pixel(1.5, w, false), 1);
        assert_eq!(lookup_pixel(1.99, w, false), 1);
    }

    #[test]
    fn layer_selection_takes_the_nearest_layer_still_beyond_the_texel() {
        // far-to-near stored depths: the dome at 0, the ground at 0.1, a plate at 0.4, the texel's own
        // surface pushed past it by the bias at 0.6001
        let list = [LayerFrag { d: 0.0, rgb: [1.0; 3] }, LayerFrag { d: 0.1, rgb: [2.0; 3] }, LayerFrag { d: 0.4, rgb: [3.0; 3] }, LayerFrag { d: 0.6001, rgb: [4.0; 3] }];
        assert_eq!(select_layer(&list, 0.6).unwrap().rgb, [3.0; 3], "the plate beyond the texel, not its own surface");
        assert_eq!(select_layer(&list, 0.05).unwrap().rgb, [1.0; 3], "a texel beyond everything but the dome sees the dome");
        assert_eq!(select_layer(&list, 0.9).unwrap().rgb, [4.0; 3]);
        assert!(select_layer(&list[1..], 0.05).is_none(), "nothing beyond → the clear");
        assert!(select_layer(&[], 0.5).is_none());
    }

    #[test]
    fn d3d_depth_bias_on_d32_is_one_ulp_plus_the_slope() {
        // z near 1: exponent −1 → 2^−24; slope 0.001 per pixel × 1.0
        let b = d3d_depth_bias(0.9, 0.001, (1, 1.0));
        assert!((b - (2f32.powi(-24) + 0.001)).abs() < 1e-9, "{b}");
        // the constant term follows the primitive's max depth exponent
        let b2 = d3d_depth_bias(0.3, 0.0, (1, 1.0));
        assert!((b2 - 2f32.powi(-25)).abs() < 1e-12, "{b2}");
        assert_eq!(d3d_depth_bias(0.9, 0.5, (0, 0.0)), 0.0);
        assert!((d3d_depth_bias(0.9, 0.5, (0, 2.0)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn frame_round_trips_through_its_frustum() {
        let d = norm([0.3, 0.5, 0.8]);
        let f = PeelFrame::new(d, [800.0, 10.0, 300.0], [900.0, 90.0, 380.0], 512);
        let fr = f.frustum();
        let g = PeelFrame::from_frustum(&fr, 512, 512);
        for p in [[850.0f32, 40.0, 340.0], [800.0, 10.0, 300.0], [899.0, 89.0, 379.0]] {
            let (x0, y0, z0) = f.project(p);
            let (x1, y1, z1) = g.project(p);
            assert!((x0 - x1).abs() < 1e-2 && (y0 - y1).abs() < 1e-2, "{p:?}: ({x0},{y0}) vs ({x1},{y1})");
            assert!((f.z01(z0) - g.z01(z1)).abs() < 1e-5);
            // the Frustum's own projection agrees with the frame's
            let (px, py, z01) = fr.project(p, 512, 512);
            assert!((px - x0).abs() < 1e-2 && (py - y0).abs() < 1e-2 && (z01 - f.z01(z0)).abs() < 1e-5, "{p:?}: frustum ({px},{py},{z01}) vs frame ({x0},{y0},{})", f.z01(z0));
            // unproject inverts project
            let q = f.unproject(x0, y0, z0);
            assert!((0..3).all(|k| (q[k] - p[k]).abs() < 1e-2), "{q:?} vs {p:?}");
        }
        // the receivers' bbox spans z01 0..1: the corner farthest along d is the far plane
        let zs: Vec<f32> = (0..8).map(|i| { let p = [if i & 1 == 0 { 800.0 } else { 900.0 }, if i & 2 == 0 { 10.0 } else { 90.0 }, if i & 4 == 0 { 300.0 } else { 380.0 }]; f.z01(f.project(p).2) }).collect();
        let (lo, hi) = (zs.iter().cloned().fold(f32::MAX, f32::min), zs.iter().cloned().fold(f32::MIN, f32::max));
        assert!(lo.abs() < 1e-5 && (hi - 1.0).abs() < 1e-5, "{lo} {hi}");
        // extend_far pushes the far plane out to a triangle below the box, the near plane stays
        let mut f2 = f.clone();
        // (beyond = farther along d, the sky side; a point behind the near plane would not move it)
        let far = [850.0 + 300.0 * d[0], 40.0 + 300.0 * d[1], 340.0 + 300.0 * d[2]];
        let tri = WTri { p0: far, e1: [1.0, 0.0, 0.0], e2: [0.0, 0.0, 1.0], inst: DECOR_INST, tri: 0, alpha: u16::MAX, uv0: [[0.0; 2]; 3] };
        f2.extend_far(&Bvh::build(vec![tri]));
        assert!((f2.z_from_z01(1.0) - f.z_from_z01(1.0)).abs() < 1e-3, "near plane unchanged");
        let (_, _, zt) = f2.project(tri.p0);
        assert!(f2.z01(zt) >= 0.0 && f2.z01(zt) < 0.05, "the far triangle is now inside, at the far end: {}", f2.z01(zt));
    }
}

/// LMTOOL_CARD_DUMP=DIR: every card fragment of the A-buffer builds (before the alpha test) — pixel, depth (z01), the
/// BVH triangle, TexCoord0, the alpha mask, the footprint and the port's alpha-test answer — written per (direction,
/// peel) as `DIR/cardfrags-d{di}-p{pi}.bin` for `lmtool card-fit` (the anisotropic footprint rule against the capture).
pub static CARD_DUMP: std::sync::LazyLock<Option<std::path::PathBuf>> = std::sync::LazyLock::new(|| std::env::var_os("LMTOOL_CARD_DUMP").map(std::path::PathBuf::from));
pub static CARD_FRAGS: std::sync::Mutex<Vec<CardFrag>> = std::sync::Mutex::new(Vec::new());
/// LMTOOL_PEEL_SNAP=1: snap the projected item vertices to the GPU's 1/256-pixel grid (ties to even) in the A-buffer raster.
pub static PEEL_SNAP: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_PEEL_SNAP").map(|v| v == "1").unwrap_or(false));
/// The peel item draws' depth state for the dump's `zq` (the capture's: DepthBias 1, SlopeScaledDepthBias 1.0, a D16 target).
pub const CARD_DUMP_BIAS: ((i32, f32), u32) = ((1, 1.0), 16);

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct CardFrag {
    pub x: u32,
    pub y: u32,
    pub z01: f32,
    pub tri: u32,
    pub u: f32,
    pub v: f32,
    pub mask: u32,
    pub fp_dx: [f32; 2],
    pub fp_dy: [f32; 2],
    /// the port's answer (1 = passes the alpha test)
    pub port_pass: u32,
    /// the depth the game's buffer holds for this fragment: z01 + the D3D11 depth bias (DepthBias 1, SlopeScaled 1.0 on the
    /// peel's item draws) quantised to the target's D16 step
    pub zq: f32,
}

impl CardFrag {
    pub const BYTES: usize = 52;
    pub fn write(&self, out: &mut Vec<u8>) {
        for v in [self.x, self.y] { out.extend_from_slice(&v.to_le_bytes()); }
        out.extend_from_slice(&self.z01.to_le_bytes());
        out.extend_from_slice(&self.tri.to_le_bytes());
        for v in [self.u, self.v] { out.extend_from_slice(&v.to_le_bytes()); }
        out.extend_from_slice(&self.mask.to_le_bytes());
        for v in [self.fp_dx[0], self.fp_dx[1], self.fp_dy[0], self.fp_dy[1]] { out.extend_from_slice(&v.to_le_bytes()); }
        out.extend_from_slice(&self.port_pass.to_le_bytes());
        out.extend_from_slice(&self.zq.to_le_bytes());
    }
    pub fn read(b: &[u8]) -> CardFrag {
        let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let f32_at = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        CardFrag { x: u32_at(0), y: u32_at(4), z01: f32_at(8), tri: u32_at(12), u: f32_at(16), v: f32_at(20), mask: u32_at(24), fp_dx: [f32_at(28), f32_at(32)], fp_dy: [f32_at(36), f32_at(40)], port_pass: u32_at(44), zq: f32_at(48) }
    }
}

/// Write and clear the collected card fragments of one peel.
pub fn card_dump_flush(sweep: u32, di: usize, pi: usize, frame: &PeelFrame, masks: &[crate::geometry::AlphaMask]) {
    let Some(dir) = CARD_DUMP.as_ref() else { return };
    let mut v = CARD_FRAGS.lock().unwrap();
    if v.is_empty() { return; }
    let _ = std::fs::create_dir_all(dir);
    // the alpha mip chains the fragments reference, once per mask: "CMSK0001", levels, then per level w, h, bytes
    let used: std::collections::BTreeSet<u32> = v.iter().map(|f| f.mask).collect();
    for k in used {
        let p = dir.join(format!("cardmask-{k}.bin"));
        if p.exists() { continue; }
        let Some(tex) = masks.get(k as usize).and_then(|m| m.tex.as_ref()) else { continue };
        let mut out = Vec::new();
        out.extend_from_slice(b"CMSK0001");
        out.extend_from_slice(&(tex.levels.len() as u32).to_le_bytes());
        out.extend_from_slice(&(tex.flipped as u32).to_le_bytes());
        for l in &tex.levels { out.extend_from_slice(&(l.w as u32).to_le_bytes()); out.extend_from_slice(&(l.h as u32).to_le_bytes()); out.extend_from_slice(&l.a); }
        let _ = std::fs::write(&p, &out);
    }
    let mut out = Vec::with_capacity(v.len() * CardFrag::BYTES + 64);
    // header: magic, count, the frame's resolution and direction
    out.extend_from_slice(b"CFRG0001");
    out.extend_from_slice(&(v.len() as u64).to_le_bytes());
    out.extend_from_slice(&frame.res.to_le_bytes());
    out.extend_from_slice(&frame.res_y.to_le_bytes());
    for c in frame.d { out.extend_from_slice(&c.to_le_bytes()); }
    for f in v.iter() { f.write(&mut out); }
    let p = dir.join(format!("cardfrags-s{sweep}-d{di}-p{pi}.bin"));
    if let Err(e) = std::fs::write(&p, &out) { eprintln!("card dump: {}: {e}", p.display()); } else { eprintln!("card dump: {} fragments → {}", v.len(), p.display()); }
    v.clear();
}
