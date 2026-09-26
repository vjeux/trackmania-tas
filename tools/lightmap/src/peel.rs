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

/// LMTOOL_ABUF_DEBUG_LIST=FILE (engineer 5's veg-diag pixel list: lines of `x y` or `x,y`, an optional leading
/// frame token): the peel pixels whose layer derivation is printed as LAYERDBG lines (every fragment in
/// (z, tri) order: accepted or skipped, the stored depth, the colour, its layer index; then the frame's cut).
pub static LAYER_DEBUG_SET: std::sync::LazyLock<Option<std::collections::HashSet<(u32, u32)>>> = std::sync::LazyLock::new(|| {
    let path = std::env::var("LMTOOL_ABUF_DEBUG_LIST").ok()?;
    let txt = std::fs::read_to_string(&path).ok()?;
    let mut set = std::collections::HashSet::new();
    for line in txt.lines() {
        let toks: Vec<u32> = line.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|t| t.parse().ok()).collect();
        if toks.len() >= 2 { let n = toks.len(); set.insert((toks[n - 2], toks[n - 1])); }
    }
    Some(set)
});

/// Whether the visit-level debug print applies at (x, y): LMTOOL_ABUF_DEBUG's one pixel or any pixel of the
/// LMTOOL_ABUF_DEBUG_LIST set.
#[inline(always)]
pub fn abuf_debug_at(x: u32, y: u32) -> bool {
    if let Some((dx, dy)) = *ABUF_DEBUG { if x == dx && y == dy { return true; } }
    match LAYER_DEBUG_SET.as_ref() { Some(set) => set.contains(&(x, y)), None => false }
}

/// The same list for perf engineer 5's veg-diag: the listed pixels' fragments and alpha-tested candidates as machine-readable
/// `ABUFDBG` lines from the raster (before the derivation) — keyed by (peel, x, y) when the line carries the peel token, else
/// by (x, y) for every peel (kind, triangle, instance, model triangle, mask, uv, the alpha verdict, z, z01, the stored q the
/// layer would carry, front/back); `lmtool veg-diag --frags LOG` reads them.
pub static ABUF_DEBUG_LIST: std::sync::LazyLock<Option<std::collections::HashSet<(u32, u32, u32)>>> = std::sync::LazyLock::new(|| {
    let path = std::env::var("LMTOOL_ABUF_DEBUG_LIST").ok()?;
    let txt = std::fs::read_to_string(&path).ok()?;
    let mut set = std::collections::HashSet::new();
    for line in txt.lines() {
        let v: Vec<u32> = line.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|x| x.parse().ok()).collect();
        match v.len() { 3 => { set.insert((v[0], v[1], v[2])); } 2 => { set.insert((u32::MAX, v[0], v[1])); } _ => {} }
    }
    Some(set)
});
/// The peel index the A-buffer build is running for (the debug list is keyed by it).
pub static CURRENT_PEEL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// The peel state's depth bias ((DepthBias, SlopeScaledDepthBias), depth bits) the debug lines quantise the stored depth with.
pub static ABUF_DEBUG_BIAS: std::sync::Mutex<((i32, f32), u32)> = std::sync::Mutex::new(((1, 1.0), 16));
#[inline]
pub fn abuf_debug_wants(x: u32, y: u32) -> bool {
    match ABUF_DEBUG_LIST.as_ref() {
        None => false,
        Some(set) => { let p = CURRENT_PEEL.load(std::sync::atomic::Ordering::Relaxed); set.contains(&(p, x, y)) || set.contains(&(u32::MAX, x, y)) }
    }
}
/// One ABUFDBG line for a fragment (or alpha-tested candidate) of a listed pixel.
pub fn abuf_debug_line(x: u32, y: u32, kind: &str, ti: u32, t: &WTri, frame: &PeelFrame, uv: Option<(f32, f32)>, op: Option<bool>, z: f32) {
    let bias = *ABUF_DEBUG_BIAS.lock().unwrap();
    let z01 = frame.z01(z);
    let (slope, zmax_prim) = tri_slope(t, frame);
    let dd = z01 + d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), bias.0, bias.1);
    let q = if bias.1 == 16 { (dd.clamp(0.0, 1.0) * 65535.0).round() as u32 } else { (dd * 65535.0) as u32 };
    let front = dot(cross(t.e1, t.e2), frame.d) < 0.0;
    let (u, v) = uv.unwrap_or((f32::NAN, f32::NAN));
    eprintln!("ABUFDBG peel={} x={x} y={y} kind={kind} ti={ti} inst={} mtri={} mask={} u={u:.5} v={v:.5} op={} z={z:.4} z01={z01:.6} q={q} front={}", CURRENT_PEEL.load(std::sync::atomic::Ordering::Relaxed), t.inst, t.tri, t.alpha, op.map(|b| if b { 1 } else { 0 }).unwrap_or(1), front as u8);
}

/// The cards' alpha test threshold: GbxShadowAlphaThreshold = 128/255 (the capture's ShaderP cbuffer).
pub const ALPHA_THRESHOLD: f32 = 0.501_960_813_999_176;
/// LMTOOL_ALPHA_POINT=1: the point-sampled cut-out mask instead of the filtered texture (a probe).
pub static ALPHA_POINT: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_ALPHA_POINT").map(|v| v == "1").unwrap_or(false));
/// LMTOOL_ALPHA_ANISO=N: the alpha sampler's anisotropy (16 = the capture's card sampler; 1 = trilinear).
pub static ALPHA_ANISO: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_ALPHA_ANISO").ok().and_then(|v| v.parse().ok()).unwrap_or(16));
/// LMTOOL_ALPHA_QUEUE=0: the alpha test inline per fragment instead of queued sixteen wide (alphasimd; the A/B
/// switch — identical results either way).
pub fn alpha_queue_on() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_ALPHA_QUEUE").map(|v| v != "0").unwrap_or(true))
}

/// THE NON-EXACT `--alpha-point` (perf engineer 7): the cards' alpha test as one nearest-mip point sample
/// (`AlphaTex::passes_point`) instead of the filtered sampler — set once from the flags before the bake.
static ALPHA_POINT_MIP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_alpha_point_mip(on: bool) {
    ALPHA_POINT_MIP.store(on, std::sync::atomic::Ordering::Relaxed);
}
#[inline(always)]
pub fn alpha_point_mip() -> bool {
    ALPHA_POINT_MIP.load(std::sync::atomic::Ordering::Relaxed)
}

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
        // (perf 8: a task of 819 words is a 5 µs popcount under a ~100 µs hand-off — a quarter of the threads, 8 k words each)
        let chunk = (n / (threads / 4).max(1)).max(4096);
        let n_chunks = (n + chunk - 1) / chunk;
        // (perf 8: the counts on the caller — a popcount over the 2 MB of words is ~60 µs, a quarter of a pool hand-off)
        let counts: Vec<u32> = (0..n_chunks).map(|ci| words[ci * chunk..((ci + 1) * chunk).min(n)].iter().map(|w| w.count_ones()).sum()).collect();
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
    /// A vector with at least `cap` capacity: the smallest recycled one that fits, else the largest one grown
    /// (with a quarter of slack, so the next frame's slightly larger table fits without another growth — a
    /// table above glibc's 32 MB mmap ceiling is a fresh mapping every time it grows, and its page faults land
    /// in the parallel fill: 4.7 ms per direction of the tiny's layer CSR).
    pub fn take_with_capacity(&self, cap: usize) -> Vec<T> {
        let mut v = {
            let mut p = self.pool.lock().unwrap();
            let fit = p.iter().enumerate().filter(|(_, v)| v.capacity() >= cap).min_by_key(|(_, v)| v.capacity()).map(|(i, _)| i);
            match fit {
                Some(i) => p.swap_remove(i),
                None => match p.iter().enumerate().max_by_key(|(_, v)| v.capacity()).map(|(i, _)| i) { Some(i) => p.swap_remove(i), None => Vec::new() },
            }
        };
        v.clear();
        if v.capacity() < cap {
            v.reserve(cap + cap / 4);
        }
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
        } else if let Some((i, _)) = p.iter().enumerate().min_by_key(|(_, w)| w.capacity()) {
            // the pool is full: keep the twelve largest
            if p[i].capacity() < v.capacity() { p[i] = v; }
        }
    }
}

/// Drops a set of heap-owning values on the pool, 64 per task (perf 6): the tiled raster's ~4 500 per-job output lists
/// and its 2 000 per-chunk binning tables were freed by the calling thread alone at the end of every build — 1.3 ms
/// per giant frame of serial time with 128 workers idle (each glibc free locks the allocating thread's arena, and
/// the caller owns none of them). Measured and rejected first: a global recycler for the same lists (raster +7 %:
/// a list last written through another core's cache is colder than a fresh thread-local malloc — glibc's
/// per-thread arenas hand a job memory it freed itself a frame earlier). So the frees stay; they are spread.
pub fn parallel_drop<T: Send>(items: Vec<T>) {
    const PER: usize = 64;
    if items.len() <= PER {
        return;
    }
    let slots: Vec<std::sync::Mutex<Vec<T>>> = {
        let mut slots = Vec::with_capacity(items.len() / PER + 1);
        let mut it = items.into_iter();
        loop {
            let batch: Vec<T> = it.by_ref().take(PER).collect();
            if batch.is_empty() { break; }
            slots.push(std::sync::Mutex::new(batch));
        }
        slots
    };
    crate::pool::pool().run(slots.len(), |i| {
        drop(std::mem::take(&mut *slots[i].lock().unwrap()));
    });
}

pub static LAYER_FRAGS: Recycle<LayerFrag> = Recycle::new();
pub static ABUF_FRAGS: Recycle<Frag> = Recycle::new();
pub static U32S: Recycle<u32> = Recycle::new();
/// The wanted bitmaps' words (one 2 MB vector per frame of a direction).
pub static U64S: Recycle<u64> = Recycle::new();

/// THE OR PASS WITHOUT CONTENTION (perf 8): `n_items` items, each naming up to two pixels (`pixels(i) -> [Option<usize>; 2]`),
/// OR-ed into the shared bitmap `m` of `n_words` words. One task per pool participant: each ORs its contiguous share of the
/// items into a PRIVATE bitmap (plain stores, no atomics), then every participant merges one word range across all the
/// private bitmaps into `m`. The LM path's items arrive in instance order — neighbouring instances are neighbouring pixels,
/// so 160 threads OR-ing atomically into the same cache lines spent 1.4 µs per item on the line ping-pong (72 ms per peel
/// for the tiny map's 8 M LM fragments); this takes ~1 ms. The private bitmaps (threads × 2 MB) are kept across calls.
pub fn or_pass_private<F: Fn(usize) -> [Option<usize>; 2] + Sync>(m: &[std::sync::atomic::AtomicU64], n_items: usize, pixels: F) {
    let n_words = m.len();
    let p = crate::pool::pool().threads.max(1);
    // the private bitmaps, kept CLEAN between calls: the merge zeroes what it reads, and only the pages (512 words) a task
    // dirtied are read — the dirty page set per bitmap (`DIRTY`, 8 words = 512 pages of a 2 MB bitmap) says which
    const PAGE_WORDS: usize = 512;
    let n_pages = (n_words + PAGE_WORDS - 1) / PAGE_WORDS;
    let dirty_words = (n_pages + 63) / 64;
    static PRIVATE: std::sync::Mutex<(Vec<Vec<u64>>, Vec<Vec<u64>>)> = std::sync::Mutex::new((Vec::new(), Vec::new()));
    let (mut bitmaps, mut dirty) = std::mem::take(&mut *PRIVATE.lock().unwrap());
    bitmaps.resize_with(p, Vec::new);
    dirty.resize_with(p, Vec::new);
    for b in bitmaps.iter_mut() { if b.len() < n_words { b.resize(n_words, 0); } }
    for d in dirty.iter_mut() { if d.len() < dirty_words { d.resize(dirty_words, 0); } }
    let ptrs: Vec<usize> = bitmaps.iter_mut().map(|b| b.as_mut_ptr() as usize).collect();
    let dptrs: Vec<usize> = dirty.iter_mut().map(|d| d.as_mut_ptr() as usize).collect();
    let per = (n_items + p - 1) / p;
    {
        let (ptrs, dptrs) = (&ptrs, &dptrs);
        let pixels = &pixels;
        crate::pool::pool().run(p, |t| {
            // SAFETY: task t alone writes bitmap t and its dirty set
            let bm: &mut [u64] = unsafe { std::slice::from_raw_parts_mut(ptrs[t] as *mut u64, n_words) };
            let dr: &mut [u64] = unsafe { std::slice::from_raw_parts_mut(dptrs[t] as *mut u64, dirty_words) };
            for d in dr.iter_mut() { *d = 0; }
            for i in (t * per).min(n_items)..((t + 1) * per).min(n_items) {
                for px in pixels(i).into_iter().flatten() {
                    let w = px >> 6;
                    bm[w] |= 1u64 << (px & 63);
                    let pg = w / PAGE_WORDS;
                    dr[pg >> 6] |= 1u64 << (pg & 63);
                }
            }
        });
    }
    {
        // one task per page range: for each dirty (bitmap, page) the words are OR-ed into the page's accumulator and zeroed
        let (ptrs, dptrs) = (&ptrs, &dptrs);
        let pg_per = (n_pages + p - 1) / p;
        crate::pool::pool().run(p, |t| {
            let mut acc = [0u64; PAGE_WORDS];
            for pg in (t * pg_per).min(n_pages)..((t + 1) * pg_per).min(n_pages) {
                let (a, e) = (pg * PAGE_WORDS, ((pg + 1) * PAGE_WORDS).min(n_words));
                let mut any = false;
                for b in 0..p {
                    // SAFETY: the OR run above returned; this task alone touches page pg of every bitmap
                    let d = unsafe { *(dptrs[b] as *const u64).add(pg >> 6) };
                    if (d >> (pg & 63)) & 1 == 0 { continue; }
                    let bm = unsafe { std::slice::from_raw_parts_mut((ptrs[b] as *mut u64).add(a), e - a) };
                    if !any { acc[..e - a].fill(0); any = true; }
                    for (k, w) in bm.iter_mut().enumerate() { acc[k] |= *w; *w = 0; }
                }
                if any {
                    for k in 0..e - a { if acc[k] != 0 { m[a + k].fetch_or(acc[k], std::sync::atomic::Ordering::Relaxed); } }
                }
            }
        });
    }
    *PRIVATE.lock().unwrap() = (bitmaps, dirty);
}

impl Drop for PixelIndex {
    fn drop(&mut self) {
        U64S.give(std::mem::take(&mut self.words));
        U32S.give(std::mem::take(&mut self.rank));
        U32S.give(std::mem::take(&mut self.pixels));
    }
}

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
    /// The fragments of the wanted pixel of dense rank `k` (the sparse A-buffer's own order: no rank query).
    #[inline(always)]
    pub fn at_rank(&self, k: usize) -> &[Frag] {
        let b = &self.bands[0];
        &b.1[b.0[k] as usize..b.0[k + 1] as usize]
    }
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
    build_abuffer_sparse_items(tris, ranges, None, None, frame, threads, zmin, zmax, masks, px, count)
}

/// `build_abuffer_sparse_ranges` over the instance hierarchy's jobs when `hier` is given (the ranges are then
/// unused: the jobs hold the frame's triangles).
pub fn build_abuffer_sparse_items(tris: &[WTri], ranges: &[(u32, u32)], hier: Option<(&crate::insthier::InstHier, &[crate::insthier::GeomJob])>, soa: Option<&crate::bvh::TriSoa>, frame: &PeelFrame, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask], px: &std::sync::Arc<PixelIndex>, count: Option<CountCtx>) -> (ABuffer, Option<(usize, Vec<f64>)>) {
    let res = frame.res;
    let res_y = frame.res_y;
    let t_clip = std::time::Instant::now(); crate::pool::stats::stage("clip");
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
    // THE TILE JOBS: the clip rectangle cut into cells of TILE_ROWS × TILE_COLS pixels; every triangle
    // that reaches the frame's depth range is binned into the cells its projected bounding box touches — a
    // per-chunk CSR of triangle indices per cell (4 bytes each: tiny 16 has 2.4 M triangles, the giants
    // 27 M — a projected record per triangle would be gigabytes per direction), so a job walks its own
    // triangles only and projects them again (cheaper than storing the projection). One job per non-empty
    // cell (a heavy cell — a canopy tile with many times the mean cost — split into row strips of its own
    // re-binned lists), the jobs handed out LONGEST FIRST by an estimated cost, so the pass's tail is a
    // small job, not a canopy tile (the 7-row bands had the slowest band 3.5–5.4× the mean: 17 % idle).
    // Triangle order within a cell is the scene order (chunks concatenated in order), as the dense build's;
    // a pixel belongs to exactly one job, so its fragments arrive in triangle order.
    // Why cells: a leaf triangle of two or three rows straddled a 7-row band a third of the time and a
    // 64-row cell a twentieth (a fifth fewer band-triangle pairs, i.e. setups); a cell's count tables
    // (4 k pixels: 8 KB of u16 counts, 16 KB of environment depths) sit in L1 instead of L2; its row
    // spans are 64 pixels long.
    let raster_stats = raster_stats_on();
    // LMTOOL_MICRO_CULL_MAX=N (default 16; 0 = off): the candidate-centre count up to which a triangle is tested
    // exactly at the binning (`rows_of` below) — engineer 5's knob
    let micro_cull_max: u32 = { static V: std::sync::OnceLock<u32> = std::sync::OnceLock::new(); *V.get_or_init(|| std::env::var("LMTOOL_MICRO_CULL_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(16)) };
    let bitmap: &[u64] = px.words.as_slice();
    let rows = (clip.3 - clip.1 + 1).max(0) as usize;
    let cols = (clip.2 - clip.0 + 1).max(0) as usize;
    // (powers of two: the binning divides by them per triangle — shifts, not divisions)
    let (tile_rows, tile_cols) = ((*TILE_ROWS).next_power_of_two().max(8) as usize, (*TILE_COLS).next_power_of_two().max(16) as usize);
    let (row_shift, col_shift) = (tile_rows.trailing_zeros(), tile_cols.trailing_zeros());
    let n_brows = ((rows + tile_rows - 1) / tile_rows).max(1);
    let n_bcols = ((cols + tile_cols - 1) / tile_cols).max(1);
    // (the binning's hit records hold the cell coordinates in a byte and the rows in a u16)
    assert!(n_brows <= 256 && n_bcols <= 256 && res_y <= 65535, "the tiled raster: at most 256 cell rows and columns (LMTOOL_TILE_ROWS/COLS too small for this frame)");
    let n_cells = n_brows * n_bcols;
    let brow_of = |y: i32| -> usize { (((y - clip.1).max(0) as usize) >> row_shift).min(n_brows - 1) };
    let bcol_of = |x: i32| -> usize { (((x - clip.0).max(0) as usize) >> col_shift).min(n_bcols - 1) };
    // THE CULL BETWEEN PIXEL CENTRES, EXACT (perf engineer 5's hook, OpenSWR's "cull between pixel centres",
    // here as one 16-lane pass): a small triangle — up to `micro_cull_max` candidate centres in its clipped
    // bounding box — is tested at every candidate with the raster's own inside predicate (`raster::CoverTest`:
    // the same edge functions, orientation and top-left rule, on the same projected f32 vertices), and binned
    // into the rows AND columns holding a covered centre only — none covered, not binned at all. The raster's
    // visited set is unchanged (a cell it would have walked without a covered centre produced nothing); half of
    // the giant's leaf pairs cover no centre. Shared by the scalar `rows_of_projected` and the 16-wide binning.
    // the covered rows and columns from a candidate box's coverage mask (row-major, w = rx1 − rx0 + 1)
    let rect_of_mask = |m: u16, ry0: i32, rx0: i32, bw: u32, bh: u32| -> Option<(i32, i32, i32, i32)> {
        if m == 0 {
            return None;
        }
        let (mut cy0, mut cy1) = (i32::MAX, i32::MIN);
        let rowbits = (1u32 << bw) - 1;
        let mut colmask = 0u32;
        for j in 0..bh {
            let r = (m as u32 >> (j * bw)) & rowbits;
            if r != 0 {
                colmask |= r;
                cy0 = cy0.min(ry0 + j as i32);
                cy1 = ry0 + j as i32;
            }
        }
        let cx0 = rx0 + colmask.trailing_zeros() as i32;
        let cx1 = rx0 + (31 - colmask.leading_zeros()) as i32;
        Some((cy0, cy1, cx0, cx1))
    };
    let micro_rect = |p: [[f32; 2]; 3], ry0: i32, ry1: i32, rx0: i32, rx1: i32| -> Option<(i32, i32, i32, i32)> {
        if micro_cull_max > 0 {
            let (bw, bh) = ((rx1 - rx0 + 1) as u32, (ry1 - ry0 + 1) as u32);
            if bw * bh <= micro_cull_max.min(16) {
                let cover = raster::CoverTest::new(p)?;
                return rect_of_mask(cover.covers_box(rx0, ry0, rx1, ry1), ry0, rx0, bw, bh);
            }
        }
        Some((ry0, ry1, rx0, rx1))
    };
    // a triangle's projected pixel rows and columns (as raster::bounds computes them), clipped to the rectangle
    // (the projected form: the binning projects sixteen records at a time — binproj::project16 — and hands each
    // lane's nine values here; `rows_of` projects one record itself)
    let rows_of_projected = |t: &WTri, (x0, y0, z0): (f32, f32, f32), (x1, y1, z1): (f32, f32, f32), (x2, y2, z2): (f32, f32, f32)| -> Option<(i32, i32, i32, i32)> {
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
        let (rx0, rx1) = (rx0 as i32, rx1 as i32);
        micro_rect([[x0, y0], [x1, y1], [x2, y2]], ry0, ry1, rx0, rx1)
    };
    let rows_of = |t: &WTri| -> Option<(i32, i32, i32, i32)> {
        let (a, b, c) = crate::binproj::project_one(t, frame);
        rows_of_projected(t, a, b, c)
    };
    // THE ITEMS TO BIN: with the instance hierarchy (perf engineer 5's insthier.rs) an item is a GeomJob — a
    // contiguous range of `hier.tris` with a conservative pixel rectangle, produced from the model trees without
    // touching a triangle record — and the raster job expands it per triangle (the same `rows_of` test: depth
    // range, culls, the exact centre cull) inside its cell; without it an item is one culled triangle projected
    // here. `tri_src` / `tri_id` are the records and their BVH indices (the fragments' `tri` keys) either way.
    let (tri_src, tri_id): (&[WTri], Option<&[u32]>) = match hier { Some((h, _)) => (h.tris.as_slice(), Some(h.bvh_id.as_slice())), None => (tris, None) };
    let simd_bin = hier.is_none() && crate::binsimd::available() && !cull_back && cards_occlude && soa.map_or(false, |s| s.blocks.len() * 16 >= tris.len());
    let simd_check = simd_bin && crate::binsimd::check_on();
    let framek = crate::binsimd::FrameK::new(frame, zmin, zmax, clip);
    let n_items: usize = match hier { Some((_, jobs)) => jobs.len(), None => ranges.iter().map(|r| (r.1 - r.0) as usize).sum() };
    let prep_chunk = (n_items / (threads * 4).max(1)).max(if hier.is_some() { 256 } else { 1024 });
    // a chunk = consecutive (sub)ranges totalling about prep_chunk items, in ascending index order (the BVH cull
    // hands over thousands of small ranges: one chunk each would mean thousands of CSRs per frame)
    let mut chunks: Vec<Vec<(u32, u32)>> = Vec::new();
    {
        let mut cur: Vec<(u32, u32)> = Vec::new();
        let mut cur_n = 0usize;
        let job_range = [(0u32, n_items as u32)];
        let src_ranges: &[(u32, u32)] = if hier.is_some() { &job_range } else { ranges };
        for &(a, b) in src_ranges {
            let mut x = a;
            while x < b {
                let room = prep_chunk.saturating_sub(cur_n).max(1);
                let y = (x as usize + room).min(b as usize) as u32;
                cur.push((x, y));
                cur_n += (y - x) as usize;
                x = y;
                if cur_n >= prep_chunk {
                    chunks.push(std::mem::take(&mut cur));
                    cur_n = 0;
                }
            }
        }
        if !cur.is_empty() { chunks.push(cur); }
    }
    let n_prep = chunks.len();
    // the cell (brow, bcol) as an index
    let cell_at = |br: usize, bc: usize| -> usize { br * n_bcols + bc };
    let cell_rect = |cell: usize| -> (i32, i32, i32, i32) {
        let (br, bc) = (cell / n_bcols, cell % n_bcols);
        let by0 = clip.1 + (br * tile_rows) as i32;
        let by1 = (clip.1 + ((br + 1) * tile_rows) as i32 - 1).min(clip.3);
        let bx0 = clip.0 + (bc * tile_cols) as i32;
        let bx1 = (clip.0 + ((bc + 1) * tile_cols) as i32 - 1).min(clip.2);
        (bx0, by0, bx1, by1)
    };
    // THE COST ESTIMATE of a pair: its setup plus the pixels of its bounding box inside the cell (the row
    // spans test about half of them, the visits are fewer still); a pair with visits pays the body per
    // visit — alpha tests, count pushes — so a pair weighs about fifty pixel tests (LMTOOL_PAIR_COST)
    let pair_cost: u64 = *PAIR_COST_V;
    // THE SUB-STRIPS: a cell's rows in eight strips; every entry carries the bits of the strips its rows
    // touch, so a heavy cell's job for some strips walks the cell's list and skips the other entries without
    // projecting them — the split costs no re-binning
    const SUB: usize = 8;
    let sub_shift = row_shift.saturating_sub(3); // tile_rows / SUB rows per sub-strip (SUB = 8)
    let sub_of = |y: i32, by0: i32| -> usize { (((y - by0).max(0) as usize) >> sub_shift).min(SUB - 1) };
    // per chunk: the CSR over the cells (offsets, then the item indices in item order with their strip bits)
    // and the cells' estimated costs. `place` takes one item (index, rows, columns, weight in pairs) — a plain
    // closure the item loops call directly (an iterator through `dyn Iterator` cost a virtual call and three
    // uninlined closures per triangle); `finish` builds the CSR from the placed items.
    #[derive(Clone, Copy)]
    struct Hit { idx: u32, br0: u8, br1: u8, bc0: u8, bc1: u8, ry0: u16, ry1: u16 }
    struct BinState {
        hits: Vec<Hit>,
        counts: Vec<u32>,
        cost: Vec<u64>,
    }
    let bin_state = |n_hint: usize| -> BinState {
        // (sized up front: a 50 k-triangle chunk grew the hits by doubling — 2.6 MB of copies per chunk)
        BinState { hits: Vec::with_capacity(n_hint), counts: vec![0; n_cells + 1], cost: vec![0; n_cells] }
    };
    let place = |st: &mut BinState, idx: u32, ry0: i32, ry1: i32, rx0: i32, rx1: i32, weight: u64| {
        let (br0, br1) = (brow_of(ry0), brow_of(ry1));
        let (bc0, bc1) = (bcol_of(rx0), bcol_of(rx1));
        if br0 == br1 && bc0 == bc1 {
            // THE SINGLE-CELL ITEM (the large majority of the survivors: a leaf's box inside one cell) — its whole
            // box is inside the cell, so the box itself is the cost, no clipping, one count
            let cell = cell_at(br0, bc0);
            st.counts[cell + 1] += 1;
            st.cost[cell] += weight * pair_cost + (rx1 - rx0 + 1) as u64 * (ry1 - ry0 + 1) as u64;
        } else {
            for br in br0..=br1 {
                // the rows of the bounding box inside this cell row (no division: the cell's rows from br)
                let cy0 = clip.1 + (br * tile_rows) as i32;
                let h_in = (ry1.min(cy0 + tile_rows as i32 - 1) - ry0.max(cy0) + 1).max(0) as u64;
                for bc in bc0..=bc1 {
                    let cell = cell_at(br, bc);
                    st.counts[cell + 1] += 1;
                    let cx0 = clip.0 + (bc * tile_cols) as i32;
                    let w_in = (rx1.min(cx0 + tile_cols as i32 - 1) - rx0.max(cx0) + 1).max(0) as u64;
                    st.cost[cell] += weight * pair_cost + w_in * h_in;
                }
            }
        }
        // (12 bytes a hit — the cell coordinates fit a byte with 128-pixel cells over a 4 096 frame, the rows a u16;
        // the 24-byte form streamed 1.3 GB per giant direction through this list)
        st.hits.push(Hit { idx, br0: br0 as u8, br1: br1 as u8, bc0: bc0 as u8, bc1: bc1 as u8, ry0: ry0 as u16, ry1: ry1 as u16 });
    };
    let finish = |st: BinState| -> (Vec<u32>, Vec<u32>, Vec<u8>, Vec<u64>) {
        let BinState { hits, mut counts, cost } = st;
        for i in 0..n_cells { counts[i + 1] += counts[i]; }
        let mut entries: Vec<u32> = vec![0; counts[n_cells] as usize];
        let mut ebits: Vec<u8> = vec![0; counts[n_cells] as usize];
        let mut fill = counts.clone();
        for &Hit { idx, br0, br1, bc0, bc1, ry0, ry1 } in &hits {
            for br in br0 as usize..=br1 as usize {
                let by0 = clip.1 + (br * tile_rows) as i32;
                let (s0, s1) = (sub_of(ry0 as i32, by0), sub_of(ry1 as i32, by0));
                let bits = (((1u16 << (s1 + 1)) - 1) & !((1u16 << s0) - 1)) as u8;
                for bc in bc0 as usize..=bc1 as usize {
                    let cell = cell_at(br, bc);
                    entries[fill[cell] as usize] = idx;
                    ebits[fill[cell] as usize] = bits;
                    fill[cell] += 1;
                }
            }
        }
        (counts, entries, ebits, cost)
    };
    let bin_ns: Vec<std::sync::atomic::AtomicU64> = if raster_stats { (0..n_prep).map(|_| std::sync::atomic::AtomicU64::new(0)).collect() } else { Vec::new() };
    let binned: Vec<(Vec<u32>, Vec<u32>, Vec<u8>, Vec<u64>)> = crate::pool::pool().map(n_prep, |ci| {
        let t_ch = std::time::Instant::now();
        let subs = &chunks[ci];
        let mut st = bin_state(subs.iter().map(|&(a, b)| (b - a) as usize).sum());
        match hier {
            Some((_, jobs)) => {
                // a job's rectangle is already clipped to the frame; its weight is its triangle count
                for &(a, b) in subs.iter() {
                    for j in a..b {
                        let g = &jobs[j as usize];
                        place(&mut st, j, g.y0, g.y1, g.x0, g.x1, g.count as u64);
                    }
                }
            }
            None => {
                // THE BINNING 16 WIDE (PERF 5.3 / 5.3b, binsimd.rs): the projection, depth test and candidate test
                // over blocks of 16 triangles from the BVH's position table (bvh::TriSoa, 36 B per triangle), the
                // ≤ 4-candidate lanes' exact centre cull sixteen triangles at a time (micro_16), covers_box for the
                // 5–16-candidate lanes — the same decisions as `rows_of` (checked lane by lane under
                // LMTOOL_BIN_SIMD_CHECK=1), the survivors placed in ascending index order. The scalar loop below
                // stays for the switches that read the record (cull_back, cards that do not occlude), for machines
                // without AVX-512 and for LMTOOL_BIN_SIMD=0. (Engineer 3's binproj::project16 — gathers from the
                // 72-byte records — measured slower than the scalar stream; the SoA table is what makes it pay.)
                if simd_bin {
                    let soa = soa.unwrap();
                    let mut blk = crate::binsimd::Block16::ZERO;
                    let mut mic = crate::binsimd::Micro16::ZERO;
                    let mic_ok = micro_cull_max >= 4;
                    for &(a, b) in subs.iter() {
                        let (bl0, bl1) = ((a / 16) as usize, ((b - 1) / 16) as usize);
                        for bi in bl0..=bl1 {
                            let lo = (bi as u32 * 16).max(a);
                            let hi = (bi as u32 * 16 + 16).min(b);
                            let lanes: u16 = (((1u32 << (hi - bi as u32 * 16)) - 1) & !((1u32 << (lo - bi as u32 * 16)) - 1)) as u16;
                            crate::binsimd::rows_of_16(&soa.blocks[bi], &framek, lanes, &mut blk);
                            if mic_ok { crate::binsimd::micro_16(&blk, &framek, &mut mic); } else { mic.small = 0; }
                            let mut m = blk.mask as u32;
                            let mut decided: u16 = 0;
                            while m != 0 {
                                let l = m.trailing_zeros() as usize;
                                m &= m - 1;
                                let ti = bi as u32 * 16 + l as u32;
                                // the clipped ranges (micro_16 clamps the block's ceil/floor'd extents in lanes exactly as
                                // `rows_of_projected` does — its unit test checks the equality — else the scalar clamps)
                                let (ry0, ry1, rx0, rx1) = if mic_ok { (mic.ry0[l], mic.ry1[l], mic.rx0[l], mic.rx1[l]) } else { (((blk.cy0[l] as i64).max(clip.1 as i64)) as i32, ((blk.fy1[l] as i64).min(clip.3 as i64)) as i32, ((blk.cx0[l] as i64).max(clip.0 as i64)) as i32, ((blk.fx1[l] as i64).min(clip.2 as i64)) as i32) };
                                let rect = if (mic.small >> l) & 1 == 1 {
                                    rect_of_mask(mic.cov[l] as u16, ry0, rx0, (rx1 - rx0 + 1) as u32, (ry1 - ry0 + 1) as u32)
                                } else {
                                    micro_rect([[blk.xy[0][l], blk.xy[1][l]], [blk.xy[2][l], blk.xy[3][l]], [blk.xy[4][l], blk.xy[5][l]]], ry0, ry1, rx0, rx1)
                                };
                                if simd_check {
                                    assert_eq!(rect, rows_of(&tris[ti as usize]), "binning 16 wide: triangle {ti} differs from the scalar decision");
                                    decided |= 1 << l;
                                }
                                if let Some((ry0, ry1, rx0, rx1)) = rect {
                                    let t = &tris[ti as usize];
                                    place(&mut st, ti, ry0, ry1, rx0, rx1, if t.alpha != u16::MAX { *CARD_WEIGHT_V } else { 1 });
                                }
                            }
                            if simd_check {
                                let mut rej = (lanes & !blk.mask & !decided) as u32;
                                while rej != 0 {
                                    let l = rej.trailing_zeros() as usize;
                                    rej &= rej - 1;
                                    let ti = bi as u32 * 16 + l as u32;
                                    assert_eq!(rows_of(&tris[ti as usize]), None, "binning 16 wide: triangle {ti} rejected by the lanes, kept by the scalar path");
                                }
                            }
                        }
                    }
                } else {
                for &(a, b) in subs.iter() {
                    for (k, t) in tris[a as usize..b as usize].iter().enumerate() {
                        if let Some((ry0, ry1, rx0, rx1)) = rows_of(t) {
                            // (a card triangle weighs three: its visits pay the alpha queue and the per-lane records)
                            place(&mut st, a + k as u32, ry0, ry1, rx0, rx1, if t.alpha != u16::MAX { *CARD_WEIGHT_V } else { 1 });
                        }
                    }
                }
                }
            }
        }
        let r = finish(st);
        if raster_stats { bin_ns[ci].store(t_ch.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed); }
        r
    });
    if raster_stats {
        let v: Vec<u64> = bin_ns.iter().map(|a| a.load(std::sync::atomic::Ordering::Relaxed)).collect();
        let (mx, sum) = (v.iter().copied().max().unwrap_or(0), v.iter().sum::<u64>());
        eprintln!("binning: {} items in {n_prep} chunks: slowest chunk {:.2} ms, mean {:.2} ms, ideal wall {:.1} ms, wall {:.1} ms", n_items, mx as f64 / 1e6, sum as f64 / n_prep.max(1) as f64 / 1e6, sum as f64 / (crate::pool::pool().threads + 1) as f64 / 1e6, t_clip.elapsed().as_secs_f64() * 1e3);
    }
    prof::add(&prof::B_CLIP, t_clip);
    let t_raster = std::time::Instant::now(); crate::pool::stats::stage("raster");
    // the cells' totals over the chunks (chunk-outer, cell-inner: sequential over each chunk's arrays — the
    // cell-outer form touched 512 arrays per cell, 40 ms of cache misses per frame); a cell without a wanted
    // pixel in the wanted-only build has no work. The same pass zeroes the sparse CSR's per-pixel counts
    // (`csr_start`): the raster jobs count their wanted fragments per pixel as they push them (a pixel has
    // one owner job, so the increments never collide), sparing the CSR a counting pass over the fragments.
    let npx = px.len();
    let mut csr_start: Vec<u32> = U32S.take_with_capacity(npx + 1);
    // SAFETY: zeroed below in parallel blocks before any job runs; every slot written
    unsafe { csr_start.set_len(npx + 1); }
    let csr_sp = csr_start.as_mut_ptr() as usize;
    // (a cell's wanted flag — any wanted pixel inside it — is computed in the parallel totals pass below;
    // serially per cell it was ~0.4 ms of the frame's setup)
    let cell_has_wanted = |cell: usize| -> bool {
        if counting { return true; }
        let (bx0, by0, bx1, by1) = cell_rect(cell);
        for y in by0..=by1 {
            let (i0, i1) = (y as usize * res as usize + bx0 as usize, y as usize * res as usize + bx1 as usize);
            for wi in (i0 >> 6)..=(i1 >> 6) {
                let mut word = bitmap[wi];
                if wi == i0 >> 6 { word &= u64::MAX << (i0 & 63); }
                if wi == i1 >> 6 { word &= u64::MAX >> (63 - (i1 & 63)); }
                if word != 0 { return true; }
            }
        }
        false
    };
    // (the same pass lists, per cell, the chunks with an entry for it — `cell_chunks` as a CSR over the cells —
    // so a job walks its ~30 chunks instead of testing all ~600: 2 M random reads per tiny frame, 4–8 % of it)
    let (cell_pairs, cell_cost, cell_wanted_v, cell_chunks): (Vec<usize>, Vec<u64>, Vec<bool>, (Vec<u32>, Vec<u32>)) = {
        // (four cells per block: with 128 × 128 cells the 64-cell blocks were 16 tasks on 128 threads — 0.7 ms of a
        // tiny frame's setup for a pass that takes 0.1 ms spread out)
        let n_blk = (n_cells / 4).clamp(1, 256);
        let blk = (n_cells + n_blk - 1) / n_blk;
        let zblk = (npx + 1 + n_blk - 1) / n_blk;
        let per_blk: Vec<(Vec<usize>, Vec<u64>, Vec<bool>, Vec<u32>, Vec<u32>)> = crate::pool::pool().map(n_blk, |b| {
            let (z0, z1) = ((b * zblk).min(npx + 1), ((b + 1) * zblk).min(npx + 1));
            unsafe { std::ptr::write_bytes((csr_sp as *mut u32).add(z0), 0, z1 - z0); }
            let (c0, c1) = ((b * blk).min(n_cells), ((b + 1) * blk).min(n_cells));
            let mut pairs = vec![0usize; c1 - c0];
            let mut cost = vec![0u64; c1 - c0];
            let mut n_ch = vec![0u32; c1 - c0 + 1];
            for (counts, _, _, cst) in &binned {
                for cell in c0..c1 {
                    let n = counts[cell + 1] - counts[cell];
                    pairs[cell - c0] += n as usize;
                    cost[cell - c0] += cst[cell];
                    n_ch[cell - c0 + 1] += (n > 0) as u32;
                }
            }
            for i in 0..c1 - c0 { n_ch[i + 1] += n_ch[i]; }
            let mut ids = vec![0u32; n_ch[c1 - c0] as usize];
            let mut fill = n_ch.clone();
            for (ci, (counts, _, _, _)) in binned.iter().enumerate() {
                for cell in c0..c1 {
                    if counts[cell + 1] > counts[cell] { ids[fill[cell - c0] as usize] = ci as u32; fill[cell - c0] += 1; }
                }
            }
            let wanted: Vec<bool> = (c0..c1).map(|cell| pairs[cell - c0] > 0 && cell_has_wanted(cell)).collect();
            (pairs, cost, wanted, n_ch, ids)
        });
        let mut pairs = Vec::with_capacity(n_cells);
        let mut cost = Vec::with_capacity(n_cells);
        let mut wanted = Vec::with_capacity(n_cells);
        let mut off: Vec<u32> = Vec::with_capacity(n_cells + 1);
        let mut ids: Vec<u32> = Vec::new();
        off.push(0);
        for (p, c, w, n_ch, id) in per_blk {
            pairs.extend(p); cost.extend(c); wanted.extend(w);
            let base = ids.len() as u32;
            for i in 1..n_ch.len() { off.push(base + n_ch[i]); }
            ids.extend(id);
        }
        (pairs, cost, wanted, (off, ids))
    };
    let cell_wanted = |cell: usize| -> bool { cell_wanted_v[cell] };
    let t_totals_done = std::time::Instant::now();
    crate::pool::stats::checkpoint("(cell totals)");
    // THE JOBS: (cell, rows y0..=y1, columns x0..=x1, the strip bits its entries must carry) with an
    // estimated cost; a cell above the split limit (a thread's share of the frame over LMTOOL_TILE_SPLIT) is
    // split into 2, 4 or 8 row strips, each a job over the same list filtered by the strip bits
    struct Job { cell: usize, x0: i32, y0: i32, x1: i32, y1: i32, smask: u8, cost: u64 }
    let total_cost: u64 = cell_cost.iter().sum();
    let split_limit = (total_cost / (threads.max(1) as u64 * *TILE_SPLIT_V)).max(pair_cost * 64);
    let mut jobs: Vec<Job> = Vec::with_capacity(n_cells + 256);
    let mut n_heavy = 0usize;
    for cell in 0..n_cells {
        if cell_pairs[cell] == 0 || !cell_wanted(cell) { continue; }
        let (bx0, by0, bx1, by1) = cell_rect(cell);
        let cell_h = (by1 - by0 + 1).max(1) as usize;
        if cell_cost[cell] > split_limit && cell_h >= 2 {
            n_heavy += 1;
            let n_strips = (((cell_cost[cell] + split_limit - 1) / split_limit) as usize).next_power_of_two().clamp(2, SUB.min(cell_h));
            let per = SUB / n_strips; // sub-strips per strip
            for s in 0..n_strips {
                let (sub0, sub1) = (s * per, (s + 1) * per - 1);
                // the strip's rows: the sub-strips' rows (sub k = rows [k·tile_rows/SUB, (k+1)·tile_rows/SUB))
                let (sy0, sy1) = (by0 + (sub0 * tile_rows / SUB) as i32, (by0 + ((sub1 + 1) * tile_rows / SUB) as i32 - 1).min(by1));
                if sy0 > sy1 { continue; }
                let smask = (((1u16 << (sub1 + 1)) - 1) & !((1u16 << sub0) - 1)) as u8;
                jobs.push(Job { cell, x0: bx0, y0: sy0, x1: bx1, y1: sy1, smask, cost: cell_cost[cell] / n_strips as u64 });
            }
        } else {
            jobs.push(Job { cell, x0: bx0, y0: by0, x1: bx1, y1: by1, smask: u8::MAX, cost: cell_cost[cell] });
        }
    }
    crate::pool::stats::checkpoint("(job list)");
    // longest first (the pool claims job indices in order): the tail of the pass is then a small job
    jobs.sort_by(|p, q| q.cost.cmp(&p.cost));
    crate::pool::stats::checkpoint("(job sort)");
    let n_jobs = jobs.len();
    let t_jobs_ready = std::time::Instant::now();
    if std::env::var_os("LMTOOL_SETUP_TRACE").is_some() { eprintln!("setup trace: totals pass {:.3} ms, jobs build+sort {:.3} ms ({} jobs)", (t_totals_done - t_raster).as_secs_f64() * 1e3, (t_jobs_ready - t_totals_done).as_secs_f64() * 1e3, jobs.len()); }
    let band_ns: Vec<std::sync::atomic::AtomicU64> = if raster_stats { (0..n_jobs).map(|_| std::sync::atomic::AtomicU64::new(0)).collect() } else { Vec::new() };
    // THE COUNT'S LAYER LOGIC per pixel (counting mode): `list` = the pixel's item fragments sorted by (z,
    // triangle) each carrying its triangle's depth-bias term, `env_d` = the environment layer's z01 maximum
    // at the pixel (0 = none) — `extract_layers`' depth rules, no colour
    let count_run = |list: &[CFrag], env_d: f32| -> usize {
        let cx = count.as_ref().unwrap();
        layer_walk_sorted(list, env_d, cx.prm.dome_layer, cx.prm.depth_bits, frame)
    };
    // LMTOOL_BOUND_STATS=1 (measurement): per frame the fragment-count histogram (an upper bound on the layer
    // fractions) and the exact layer histogram on the census pixels (every 4th / 8th in x and y: a lower
    // bound) beside the exact one — how often would the two bounds decide the game's stop without the full
    // exact count?
    let bound_stats = std::env::var_os("LMTOOL_BOUND_STATS").is_some();
    let bound_acc: std::sync::Mutex<([usize; MAX_LAYERS + 1], [usize; MAX_LAYERS + 1], [usize; MAX_LAYERS + 1])> = std::sync::Mutex::new(([0; MAX_LAYERS + 1], [0; MAX_LAYERS + 1], [0; MAX_LAYERS + 1]));
    let parts_all: Vec<(Vec<(u32, Frag)>, [usize; MAX_LAYERS + 1], usize)> = crate::pool::pool().map(n_jobs, |j| {
        let t_band = std::time::Instant::now();
        let Job { cell, x0: bx0, y0: by0, x1: bx1, y1: by1, smask, cost: _ } = jobs[j];
        let bw = (bx1 - bx0 + 1).max(0) as usize;
        let mut hist_u = [0usize; MAX_LAYERS + 1];
        let mut hist_l4 = [0usize; MAX_LAYERS + 1];
        let mut hist_l8 = [0usize; MAX_LAYERS + 1];
        let (mut rs_tris, mut rs_tested, mut rs_visits) = (0u64, 0u64, 0u64);
        // the job's wanted fragments (dense rank, fragment) in visit order — one list per job
        let mut out: Vec<(u32, Frag)> = Vec::new();
        let mut hist = [0usize; MAX_LAYERS + 1];
        let mut covered = 0usize;
        // (counting) THE BAND'S PIXEL SLOTS: per pixel the environment layer's z01 maximum (0 = none drawn)
        // and up to SLOT_K item fragments inline, the rest in the overflow list — no fragment list to sort
        // (the tiles of a giant produce 400 M fragments per direction), the band's slots stay in cache
        let band_px = if counting { ((by1 - by0 + 1).max(0) as usize) * bw } else { 0 };
        // THE COUNT'S TABLES, per band: a fragment COUNT per pixel (u16: L1-resident) and the environment
        // layer's z01 maximum; the item fragments themselves go to a sequential LIST (pixel, fragment) — the
        // scan then counting-sorts the list by pixel (a prefix over the counts, one scatter) and runs the layer
        // logic per pixel. (The previous form wrote each fragment into a 64-byte per-pixel slot at visit time:
        // 600 M random cache-line writes per direction into a table larger than L2 — a quarter of the raster.)
        // The tables come from a per-thread pool; the scan resets what it read.
        let mut bufs = SLOT_BUFS.take();
        let (mut cnt, mut env_max, list_v, mut csr, mut offs, mut fill) = (std::mem::take(&mut bufs.0), std::mem::take(&mut bufs.1), std::mem::take(&mut bufs.2), std::mem::take(&mut bufs.3), std::mem::take(&mut bufs.4), std::mem::take(&mut bufs.5));
        if cnt.len() < band_px { cnt.resize(band_px, 0); }
        if env_max.len() < band_px { env_max.resize(band_px, 0.0); }
        // the slot form applies where the lane walk does (the dome layer, a 16-bit store, the biased order) and
        // no measurement mode wants the full list
        let slot_mode = counting && *SCAN_SLOTS && *BIASED_ORDER && *SCAN16_ON && !bound_stats && count.as_ref().map_or(false, |cx| cx.prm.dome_layer && cx.prm.depth_bits == 16);
        let mut slots_v = if slot_mode { SLOT_TABLE.take() } else { Vec::new() };
        let slot_rows = ((band_px + 15) / 16) * SLOT_ROWS_PER_BLOCK;
        if slot_mode && slots_v.len() < slot_rows { slots_v.resize(slot_rows, SlotBlock([0u32; 16])); }
        let mut list = Recs { list: list_v, slots: slots_v, slot_mode };
        list.list.clear();
        let mut saturated = false;
        if by0 > by1 || bx0 > bx1 {
            let Recs { list: list_v, slots: slots_v, .. } = list;
            if slot_mode { SLOT_TABLE.set(slots_v); }
            SLOT_BUFS.set((cnt, env_max, list_v, csr, offs, fill));
            return (out, hist, covered);
        }
        let band_clip = (bx0, by0, bx1, by1);
        // THE ALPHA QUEUE (perf engineer 4, alphasimd.rs): the card fragments of the job are not tested one by
        // one in the visit loop — they are queued (uv + pixel, depth, triangle) and tested sixteen at a time,
        // one fragment per SIMD lane, the same arithmetic per lane; the passing ones then take the tail below
        // (`emit_frag`) in push order. The tail's inputs do not depend on when it runs — the count list is
        // sorted (z, triangle) at the scan, the CSR's per-pixel sort is keyed (z, triangle) too — so the
        // deferred order gives the same A-buffer and the same count. LMTOOL_ALPHA_QUEUE=0 keeps the scalar
        // test inline (the A/B switch; the point-sampled probe and the pixel debug print use it too).
        // (perf 7's non-exact --alpha-point knob takes the inline path: the queue samples the filtered sampler only)
        let alpha_queue = alpha_queue_on() && !*ALPHA_POINT && !alpha_point_mip() && ABUF_DEBUG.is_none();
        let mut aq = crate::alphasimd::AlphaQueues::take();
        // the depth-bias term of a triangle from its index (a deferred fragment's; the same expression as
        // `bias_term_of` below), cached for the run of one triangle's fragments
        let tri_bias = |ti: u32| -> f32 {
            let cx = count.as_ref().unwrap();
            let t = &tris[ti as usize];
            let p0 = t.p0;
            let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
            let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
            let (slope, zmax_prim) = tri_slope_projected(frame, frame.project(p0), frame.project(p1), frame.project(p2));
            d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), cx.prm.depth_bias, cx.prm.depth_bits)
        };
        let mut bias_run: (u32, f32) = (u32::MAX, 0.0);
        let mut rank_run: (u32, u32) = (u32::MAX, 0);
        // THE TAIL of a visited fragment (an alpha-tested item fragment that passed, or any fragment the block
        // path did not record): the count record, the wanted test, the fragment push. `bias` = Some(the
        // triangle's bias term) from the inline path, None for a deferred fragment (computed here from its
        // triangle). The job's tables come in as arguments (the opaque block path writes them directly).
        let mut emit_frag = |cnt: &mut Vec<u16>, list: &mut Recs, saturated: &mut bool, out: &mut Vec<(u32, Frag)>, x: u32, y: u32, z: f32, ti: u32, count_it: bool, bias: Option<f32>| {
            let id = y * res + x;
            if count_it {
                let bias = bias.unwrap_or_else(|| {
                    if bias_run.0 != ti { bias_run = (ti, tri_bias(ti)); }
                    bias_run.1
                });
                let li = ((y as i32 - by0) as usize) * bw + (x as i32 - bx0) as usize;
                let c = &mut cnt[li];
                if *c == u16::MAX { *saturated = true; } else { *c += 1; }
                let c_after = *c;
                // (the record's tie key is the triangle's DRAW RANK under the biased order — looked up once
                // per triangle here rather than per fragment in the scan)
                if rank_run.0 != ti { rank_run = (ti, draw_rank_of(ti)); }
                list.push(li as u32, c_after, CFrag { z, tri: rank_run.1, bias });
            }
            if counting && !bit(bitmap, id as usize) {
                return;
            }
            let k = px.index_of_id(id);
            out.push((k, Frag { z, tri: ti }));
            // SAFETY: pixel k belongs to this job alone (the jobs partition the frame)
            unsafe { *(csr_sp as *mut u32).add(k as usize + 1) += 1; }
        };
        // the cell's entries gathered from the chunk slices in chunk order into a per-thread list — a strip job
        // keeps only the entries whose strip bits touch its rows (one sequential list to walk instead of
        // hundreds of slices). With the instance hierarchy an entry is a GeomJob: its contiguous triangles are
        // walked here (576 bytes per eight, prefetch-friendly) and each is kept when `rows_of` — the depth range,
        // the culls, the exact centre test — leaves it a candidate centre inside this job's rows and columns;
        // a triangle the hierarchy placed here without one visits nothing, exactly as the per-triangle binning
        // would not have listed it.
        let mut job_list: Vec<u32> = JOB_LIST.take();
        job_list.clear();
        // (engineer 3, 500dd311: only the chunks holding an entry for this cell — the per-cell chunk CSR from the totals pass)
        for &ci in &cell_chunks.1[cell_chunks.0[cell] as usize..cell_chunks.0[cell + 1] as usize] {
            let (counts, entries, ebits, _) = &binned[ci as usize];
            let (c0, c1) = (counts[cell] as usize, counts[cell + 1] as usize);
            if c0 == c1 { continue; }
            match hier {
                None => {
                    if smask == u8::MAX {
                        job_list.extend_from_slice(&entries[c0..c1]);
                    } else {
                        for (ei, &ti) in entries[c0..c1].iter().enumerate() {
                            if ebits[c0 + ei] & smask != 0 { job_list.push(ti); }
                        }
                    }
                }
                Some((_, jobs)) => {
                    for (ei, &ji) in entries[c0..c1].iter().enumerate() {
                        if smask != u8::MAX && ebits[c0 + ei] & smask == 0 { continue; }
                        let g = &jobs[ji as usize];
                        for i in g.first..g.first + g.count {
                            let Some((ry0, ry1, rx0, rx1)) = rows_of(&tri_src[i as usize]) else { continue };
                            if ry1 < by0 || ry0 > by1 || rx1 < bx0 || rx0 > bx1 { continue; }
                            job_list.push(i);
                        }
                    }
                }
            }
        }
        {
            for &tidx in job_list.iter() {
                let t = &tri_src[tidx as usize];
                let ti = match tri_id { Some(ids) => ids[tidx as usize], None => tidx };
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
                // the record's tie key under the biased order: the triangle's draw rank, once per triangle
                let rank_ti = draw_rank_of(ti);
                let bias_term_of = |cache: &mut Option<f32>| -> f32 {
                    *cache.get_or_insert_with(|| {
                        let cx = count.as_ref().unwrap();
                        let (slope, zmax_prim) = tri_slope_projected(frame, (x0, y0, z0), (x1, y1, z1), (x2, y2, z2));
                        d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), cx.prm.depth_bias, cx.prm.depth_bits)
                    })
                };
                // THE VISIT BODY, SIXTEEN PIXELS AT A TIME (engineer 3's span interface: one call per block of up
                // to 16 consecutive pixels of a row, `cov` bit l = pixel x0 + l inside and wanted-or-counted,
                // `bary[k][l]` = vertex k's weight there, exactly the per-pixel raster's). Per lane the same f32
                // operations in the same order as the scalar body — z = (z0·b0 + z1·b1) + z2·b2, the range test,
                // z01 = 0.5 + (zc + z) / (2·half_d), the strict maximum — so every lane's value is the scalar
                // body's to the bit; the per-fragment scalar work (the alpha test, the count record, the wanted
                // fragment) runs over the set lanes. The environment triangles (the ground plane: every pixel of
                // every frame, twice, on a tiny map) never leave the lanes.
                let env_drawn_t = matches!(env_t, Some(true));
                let env_skip_t = matches!(env_t, Some(false));
                raster::triangle_clipped_masked_spans(res, res_y, [[x0, y0], [x1, y1], [x2, y2]], band_clip, if counting { None } else { Some(bitmap) }, |bx, y, cov, bary| {
                    if raster_stats { rs_visits += cov.count_ones() as u64; }
                    let lanes = lanes_z_range(bary, [z0, z1, z2], zmin, zmax, cov);
                    let mut live = lanes.live;
                    if live == 0 { return; }
                    let id0 = y * res + bx;
                    // the wanted bits of the 16 pixels (bitmap bits id0..id0+16)
                    let wanted16: u16 = wanted_bits16(bitmap, id0 as usize);
                    if counting && env_skip_t {
                        // an environment face not drawn for this view: nothing counted, only the wanted fragments
                        live &= wanted16;
                    }
                    if counting && env_drawn_t {
                        // the environment layer's depth maximum, sixteen lanes: z01 in [0, 1] and strictly greater
                        let li0 = ((y as i32 - by0) as usize) * bw + (bx as i32 - bx0) as usize;
                        env_max_update16(&mut env_max[li0..(li0 + 16).min(band_px)], &lanes.z, live, frame);
                        live &= wanted16;
                        if live == 0 { return; }
                    }
                    if counting && env_t.is_none() && mask.is_none() {
                        // an opaque item triangle: every live lane is a fragment — the block's count records at
                        // once (the counters bumped sixteen wide, the records pushed per lane), then only the
                        // wanted lanes go on to the fragment push. INVARIANT the scan's sort relies on: a
                        // triangle visits a pixel once, so a pixel's records have distinct (z, tri) keys and one
                        // bias per tri — the (z, tri) order is total over them
                        let li0 = ((y as i32 - by0) as usize) * bw + (bx as i32 - bx0) as usize;
                        count_add16(&mut cnt[li0..(li0 + 16).min(band_px)], live, &mut saturated);
                        let bias = bias_term_of(&mut bias_term_cache);
                        let mut m = live;
                        while m != 0 {
                            let l = m.trailing_zeros() as usize;
                            m &= m - 1;
                            list.push((li0 + l) as u32, cnt[li0 + l], CFrag { z: lanes.z[l], tri: rank_ti, bias });
                        }
                        live &= wanted16;
                        if live == 0 { return; }
                    }
                    let block_counted = counting && env_t.is_none() && mask.is_none();
                    let count_it = counting && env_t.is_none() && !block_counted;
                    // the card's tap plan: computed on the triangle's first block with live lanes (the same value
                    // as on its first live lane — the plan is the triangle's), ONCE per block rather than a
                    // ten-capture closure built on every lane (engineer 4's census: 88 instructions per block)
                    let fp_block: Option<(&crate::alphatex::AlphaTex, crate::alphatex::TapPlan)> = match mask {
                        Some(mk) => *fp_tex.get_or_insert_with(|| mk.tex.as_ref().map(|tx| (tx.as_ref(), tx.plan_for(&crate::alphatex::Footprint::of_triangle([[x0, y0], [x1, y1], [x2, y2]], t.uv0, tx.w(), tx.h()), *ALPHA_ANISO, !alpha_queue)))),
                        None => None,
                    };
                    let mut m = live;
                    while m != 0 {
                        let l = m.trailing_zeros() as usize;
                        m &= m - 1;
                        let x = bx + l as u32;
                        let z = lanes.z[l];
                        let bc = [bary[0][l], bary[1][l], bary[2][l]];
                        // (the game's viewport ring is outside the clip rectangle: never visited)
                        debug_assert!(!(x < inset || y < inset || x + inset >= res || y + inset >= res_y));
                        if let Some(mk) = mask {
                            let u = t.uv0[0][0] * bc[0] + t.uv0[1][0] * bc[1] + t.uv0[2][0] * bc[2];
                            let v = t.uv0[0][1] * bc[0] + t.uv0[1][1] * bc[1] + t.uv0[2][1] * bc[2];
                            let fp = &fp_block;
                            if alpha_queue {
                                if let Some((tx, plan)) = fp {
                                    // queued: tested sixteen at a time, the passing ones emitted at the flush
                                    let q = aq.of(plan);
                                    if q.push(tx, plan, ti as u64, u, v, crate::alphasimd::Pend { x, y, z, ti, count: count_it }) {
                                        q.flush(ALPHA_THRESHOLD, |p| emit_frag(&mut cnt, &mut list, &mut saturated, &mut out, p.x, p.y, p.z, p.ti, p.count, None));
                                    }
                                    continue;
                                }
                            }
                            let op = match fp {
                                Some((tx, fp)) if !*ALPHA_POINT => if alpha_point_mip() { tx.passes_point(u, v, fp, ALPHA_THRESHOLD) } else { tx.passes_planned(u, v, fp, ALPHA_THRESHOLD, crate::alphatex::Address::ClampEdge) },
                                _ => mk.opaque(u, v),
                            };
                            if abuf_debug_wants(x, y) { abuf_debug_line(x, y, "card", ti, t, frame, Some((u, v)), Some(op), z); }
                            if abuf_debug_at(x, y) { let plan_s = match fp { Some((tx, p)) => format!(" lod {:.3} levels {}/{} two {} taps {} axis ({:.4},{:.4}) alpha {:.5} one-tap-l0 {:.5} one-tap-l1 {:.5} by-taps {} taps-of-plan [{}]", p.lod, p.l0, p.l1, p.two, p.n, p.axis[0], p.axis[1], tx.sample_planned_clamp(u, v, p), tx.bilinear_tap(u, v, 0), tx.bilinear_tap(u, v, 1), (1..=8usize).map(|k| { let mut pk = p.clone(); pk.n = k; format!("{k}:{:.5}", tx.sample_planned_clamp(u, v, &pk)) }).collect::<Vec<_>>().join(" "), (0..p.n).map(|i| { let sft = if p.n > 1 { (i as f32 + 0.5) / p.n as f32 - 0.5 } else { 0.0 }; format!("{:.5}", tx.bilinear_tap(u + p.axis[0] * sft, v + p.axis[1] * sft, p.l0)) }).collect::<Vec<_>>().join(" ")), None => String::new() }; eprintln!("abuf debug ({x},{y}) hd={:.2}: card tri {ti} inst {} model tri {} mask {} uv ({u:.4},{v:.4}) opaque {op} z {z:.3} z01 {:.5}{plan_s}", frame.half_d, t.inst, t.tri, t.alpha, frame.z01(z));  }
                            if !op {
                                continue;
                            }
                        } else if abuf_debug_wants(x, y) { abuf_debug_line(x, y, "opaque", ti, t, frame, None, None, z); } else if abuf_debug_at(x, y) { eprintln!("abuf debug ({x},{y}) hd={:.2}: tri {ti} inst {} model tri {} z {z:.3} z01 {:.5}", frame.half_d, t.inst, t.tri, frame.z01(z));  }
                        // the tail (the wanted bit of lane l is the bitmap's bit at id0 + l, as emit_frag reads it)
                        let bias = if count_it { Some(bias_term_of(&mut bias_term_cache)) } else { None };
                        emit_frag(&mut cnt, &mut list, &mut saturated, &mut out, x, y, z, ti, count_it, bias);
                    }
                });
            }
        }
        // the job's last card fragments
        aq.flush_all(ALPHA_THRESHOLD, |p| emit_frag(&mut cnt, &mut list, &mut saturated, &mut out, p.x, p.y, p.z, p.ti, p.count, None));
        drop(emit_frag);
        aq.give();
        if counting {
            // THE SCAN: the list counting-sorted by pixel (the prefix over the counts gives every pixel's range,
            // the scatter fills it in visit order), then per pixel the fragments ordered by (z, triangle) and
            // the layer logic. The prefix comes from the u16 counts (one pass over the band's pixels) unless
            // a pixel saturated them — then from the list.
            if offs.len() < band_px + 1 { offs.resize(band_px + 1, 0); }
            if fill.len() < band_px { fill.resize(band_px, 0); }
            offs[0] = 0;
            if slot_mode {
                // THE SLOT FORM: the first SLOTS_PER_PX records of every pixel sit in its slots already; only the
                // overflow (the canopy's pixels beyond the fourth record) is counting-sorted here — offs/csr hold
                // the overflow ranges
                assert!(!saturated, "a pixel of the job holds 65535 records: the slot form has no saturation path");
                let mut acc = 0u32;
                for i in 0..band_px { let o = (cnt[i] as u32).saturating_sub(SLOTS_PER_PX as u32); fill[i] = acc; acc += o; offs[i + 1] = acc; }
                if csr.len() < list.list.len() { csr.resize(list.list.len(), CFrag { z: 0.0, tri: 0, bias: 0.0 }); }
                for (li, cf) in &list.list {
                    let f = &mut fill[*li as usize];
                    csr[*f as usize] = *cf;
                    *f += 1;
                }
            } else {
                if saturated {
                    offs[..band_px + 1].fill(0);
                    for (li, _) in &list.list { offs[*li as usize + 1] += 1; }
                    for i in 0..band_px { offs[i + 1] += offs[i]; }
                } else {
                    let mut acc = 0u32;
                    for i in 0..band_px { fill[i] = acc; acc += cnt[i] as u32; offs[i + 1] = acc; }
                }
                if saturated { fill[..band_px].copy_from_slice(&offs[..band_px]); }
                if csr.len() < list.list.len() { csr.resize(list.list.len(), CFrag { z: 0.0, tri: 0, bias: 0.0 }); }
                // the scatter: fill[li] walks forward as its pixel's fragments land
                for (li, cf) in &list.list {
                    let f = &mut fill[*li as usize];
                    csr[*f as usize] = *cf;
                    *f += 1;
                }
            }
            // THE WALK IN LANES (scan_block16): the pixels with one or two fragments — nearly all — sixteen at a
            // time; the others (and every pixel when the frame's depth rules differ) take the scalar walk below
            let cx0 = count.as_ref().unwrap();
            let vector_scan = cx0.prm.dome_layer && cx0.prm.depth_bits == 16 && !bound_stats && *SCAN16_ON;
            let mut pixels: Vec<(usize, f32, u32)> = Vec::new();
            // (the fragment-count statistic per job, added to the shared counters once — an atomic per pixel
            // made the stats mode 20× slower)
            let mut nfrag_local = [0u64; 9];
            #[allow(unused_mut, unused_variables)]
            let mut vector_done = false;
            #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
            if vector_scan {
                let mut li0 = 0usize;
                while li0 + 16 <= band_px {
                    // (64-byte aligned: a 16-lane array split across two cache lines by the stack address made the
                    // zmm loads/stores 5–10 % slower — engineer 4's alignment finding)
                    let mut env16_a = Align64([0f32; 16]);
                    let env16 = &mut env16_a.0;
                    env16.copy_from_slice(&env_max[li0..li0 + 16]);
                    let (h, has, big) = if slot_mode { scan_block16_slots(&cnt[li0..li0 + 16], &list.slots[(li0 / 16) * SLOT_ROWS_PER_BLOCK..(li0 / 16 + 1) * SLOT_ROWS_PER_BLOCK], env16, frame) } else { scan_block16(&offs[li0..li0 + 17], &csr, env16, frame) };
                    if raster_stats { for l in 0..16 { let n = if slot_mode { cnt[li0 + l] as usize } else { (offs[li0 + l + 1] - offs[li0 + l]) as usize }; if n >= 1 { nfrag_local[match n { 1 | 2 => 0, 3 => 1, 4 => 2, 5 => 3, 6..=8 => 4, 9..=16 => 5, 17..=32 => 6, 33..=64 => 7, _ => 8 }] += 1; } } }
                    covered += has as usize;
                    for k in 0..=SCAN_K { hist[k] += h[k] as usize; }
                    let mut m = big;
                    while m != 0 { let l = m.trailing_zeros() as usize; m &= m - 1; pixels.push((li0 + l, env16[l], cnt[li0 + l] as u32)); }
                    env_max[li0..li0 + 16].fill(0.0);
                    cnt[li0..li0 + 16].fill(0);
                    li0 += 16;
                }
                for li in li0..band_px { let e = std::mem::replace(&mut env_max[li], 0.0); let c = std::mem::replace(&mut cnt[li], 0); if (slot_mode && c > 0) || (!slot_mode && offs[li] != offs[li + 1]) { covered += 1; pixels.push((li, e, c as u32)); } }
                vector_done = true;
            }
            if raster_stats { for k in 0..9 { if nfrag_local[k] > 0 { RS_NFRAG[k].fetch_add(nfrag_local[k], std::sync::atomic::Ordering::Relaxed); } } }
            // (the scalar walk over the pixels the lanes left — every pixel without the lane walk; no list then)
            let mut pi = 0usize;
            let mut li_scalar = 0usize;
            let mut slot_buf: Vec<CFrag> = Vec::new();
            loop {
                let (li, env_d, n_slot) = if vector_done {
                    if pi >= pixels.len() { break; }
                    let p = pixels[pi]; pi += 1; p
                } else {
                    if li_scalar >= band_px { break; }
                    let li = li_scalar; li_scalar += 1;
                    if cnt[li] == 0 && !saturated { env_max[li] = 0.0; continue; }
                    let env_d = std::mem::replace(&mut env_max[li], 0.0);
                    let c = std::mem::replace(&mut cnt[li], 0);
                    if !slot_mode && offs[li] == offs[li + 1] { continue; }
                    covered += 1;
                    (li, env_d, c as u32)
                };
                let (a, c) = (offs[li] as usize, offs[li + 1] as usize);
                if slot_mode {
                    // the pixel's records: its slots, then its overflow range
                    let ns = (n_slot as usize).min(SLOTS_PER_PX);
                    slot_buf.clear();
                    for j in 0..ns { slot_buf.push(list.slot(li, j)); }
                    slot_buf.extend_from_slice(&csr[a..c]);
                    if *MEASURE_NOWALK { hist[1] += 1; continue; }
                    let cx = count.as_ref().unwrap();
                    hist[layer_walk_biased_cfrags(&slot_buf, env_d, cx.prm.dome_layer, cx.prm.depth_bits, frame).min(MAX_LAYERS)] += 1;
                    continue;
                }
                let n = c - a;
                if bound_stats {
                    // how many item fragments the depth rules drop without needing their alpha result:
                    // behind the environment layer, or inside an accepted fragment's bias window
                    {
                        let mut tmp: Vec<CFrag> = csr[a..c].to_vec();
                        tmp.sort_by(|p, q| p.z.total_cmp(&q.z).then_with(|| p.tri.cmp(&q.tri)));
                        let cx = count.as_ref().unwrap();
                        let prm = cx.prm;
                        let mut d_prev = f32::NEG_INFINITY;
                        let env_q = if prm.dome_layer { if env_d > 0.0 { if prm.depth_bits == 16 { (env_d * 65535.0).round() / 65535.0 } else { env_d } } else { 0.0 } } else { f32::NEG_INFINITY };
                        d_prev = d_prev.max(env_q);
                        let mut items = 0usize;
                        for f in &tmp {
                            let z01 = frame.z01(f.z).max(0.0);
                            if z01 < env_q { DROP_STATS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed); continue; }
                            if z01 < d_prev { DROP_STATS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); continue; }
                            if items >= MAX_LAYERS { DROP_STATS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); continue; }
                            let mut dd = z01 + f.bias;
                            if prm.depth_bits == 16 { dd = (dd.clamp(0.0, 1.0) * 65535.0).round() / 65535.0; }
                            items += 1; d_prev = dd;
                            DROP_STATS[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    hist_u[n.min(MAX_LAYERS)] += 1;
                    let (x, y) = (bx0 as usize + li % bw, by0 as usize + li / bw);
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
                if *BIASED_ORDER {
                    if *MEASURE_NOWALK { hist[1] += 1; continue; }
                    let cx = count.as_ref().unwrap();
                    hist[layer_walk_biased_cfrags(buf, env_d, cx.prm.dome_layer, cx.prm.depth_bits, frame).min(MAX_LAYERS)] += 1;
                    continue;
                }
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
                    _ => {
                        // many fragments (the canopy): `layer_walk_keyed` — the (z, tri) order as ONE u64 key
                        // (z's bits in the total order above the triangle index), an insertion sort, z01 once per
                        // fragment, then the walk; the same order and operations as the sort + count_run above
                        let cx = count.as_ref().unwrap();
                        hist[layer_walk_keyed(buf, env_d, cx.prm.dome_layer, cx.prm.depth_bits, frame).min(MAX_LAYERS)] += 1;
                        continue;
                    }
                }
                hist[count_run(buf, env_d).min(MAX_LAYERS)] += 1;
            }
        }
        if bound_stats { let mut g = bound_acc.lock().unwrap(); for k in 0..=MAX_LAYERS { g.0[k] += hist_u[k]; g.1[k] += hist_l4[k]; g.2[k] += hist_l8[k]; } }
        let Recs { list: list_v, slots: slots_v, .. } = list;
        if slot_mode { SLOT_TABLE.set(slots_v); }
        SLOT_BUFS.set((cnt, env_max, list_v, csr, offs, fill));
        JOB_LIST.set(job_list);
        if raster_stats { band_ns[j].store(t_band.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed); RS_TRIS.fetch_add(rs_tris, std::sync::atomic::Ordering::Relaxed); RS_TESTED.fetch_add(rs_tested, std::sync::atomic::Ordering::Relaxed); RS_VISITS.fetch_add(rs_visits, std::sync::atomic::Ordering::Relaxed); }
        (out, hist, covered)
    });
    if raster_stats {
        let v: Vec<u64> = band_ns.iter().map(|a| a.load(std::sync::atomic::Ordering::Relaxed)).collect();
        let mx = v.iter().copied().max().unwrap_or(0);
        let sum_ns: u64 = v.iter().sum();
        // the slowest jobs against their estimated cost (their rank in the longest-first order): a slow job
        // ranked late is a cost-model miss, a slow job ranked first is the tail the split limit must cut
        let mut order: Vec<usize> = (0..n_jobs).collect();
        order.sort_by_key(|&j| std::cmp::Reverse(v[j]));
        let worst: Vec<String> = order.iter().take(6).map(|&j| format!("[rank {j}: {:.1} ms, cost {}, {} pairs in the cell{}]", v[j] as f64 / 1e6, jobs[j].cost, cell_pairs[jobs[j].cell], if jobs[j].smask != u8::MAX { format!(", strip {:08b}", jobs[j].smask) } else { String::new() })).collect();
        eprintln!("raster slowest jobs: {} (split limit {} cost units)", worst.join(" "), split_limit);
        let mean = sum_ns as f64 / v.len().max(1) as f64;
        // the balance: the pass's wall against the per-thread share of the jobs' total
        let ideal = sum_ns as f64 / (crate::pool::pool().threads + 1) as f64;
        eprintln!("raster jobs: {} jobs over {} cells of {}×{} ({} heavy cells split), slowest {:.1} ms, mean {:.2} ms, ideal wall {:.1} ms, jobs wall {:.1} ms ({:.0} % idle), setup before the jobs {:.1} ms", n_jobs, n_cells, tile_cols, tile_rows, n_heavy, mx as f64 / 1e6, mean / 1e6, ideal / 1e6, t_jobs_ready.elapsed().as_secs_f64() * 1e3, (1.0 - ideal / t_jobs_ready.elapsed().as_nanos().max(1) as f64) * 100.0, (t_jobs_ready - t_raster).as_secs_f64() * 1e3);
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
            // the census LOWER bounds on the rendered count (the rule walked with the lower-bound fractions
            // stops no later than with the exact ones): for the wanted-pixel certification measured in
            // extract_layers
            CENSUS_LB[0].store(prm.peel_stop.layers_rendered(&l4), std::sync::atomic::Ordering::Relaxed);
            CENSUS_LB[1].store(prm.peel_stop.layers_rendered(&l8), std::sync::atomic::Ordering::Relaxed);
            eprintln!("bound stats: frame {}×{}: exact rendered {rendered} (fractions {:?}); U {:?}; certified with census/4 {:?}, census/8 {:?}", res, res_y, fractions.iter().take(rendered + 1).map(|f| format!("{f:.4}")).collect::<Vec<_>>(), u.iter().take(rendered + 1).map(|f| format!("{f:.4}")).collect::<Vec<_>>(), c4, c8);
            BOUND_TOTALS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if c4 == Some(rendered) { BOUND_TOTALS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } else if c4.is_some() { BOUND_TOTALS[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
            if c8 == Some(rendered) { BOUND_TOTALS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } else if c8.is_some() { BOUND_TOTALS[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
        }
        (rendered, fractions)
    });
    let parts: Vec<Vec<(u32, Frag)>> = parts_all.into_iter().map(|(o, _, _)| o).collect();
    prof::add(&prof::B_RASTER, t_raster);
    // the binning's per-chunk tables are read no more: DROPPED IN PARALLEL (perf 6) — the caller freeing 2 000 vectors the
    // workers allocated (glibc: each free locks the allocating thread's arena) was 0.5 ms of serial time per frame;
    // a global recycler instead was measured SLOWER (raster +7 %: a list last written by another core's cache is
    // colder than a fresh thread-local malloc), so the frees stay, spread over the pool
    parallel_drop(binned);
    if raster_stats { eprintln!("raster stats (sparse, {n_jobs} jobs over {n_cells} cells): {} triangles rasterised, {} bbox pixels tested, {} pixel visits, clip {:?}, wanted {}, {:.3}s", RS_TRIS.swap(0, std::sync::atomic::Ordering::Relaxed), RS_TESTED.swap(0, std::sync::atomic::Ordering::Relaxed), RS_VISITS.swap(0, std::sync::atomic::Ordering::Relaxed), clip, px.len(), t_raster.elapsed().as_secs_f32()); }
        if raster_stats { let f = |i: usize| RS_NFRAG[i].swap(0, std::sync::atomic::Ordering::Relaxed); eprintln!("scan pixels by fragment count: 1–2: {}, 3: {}, 4: {}, 5: {}, 6–8: {}, 9–16: {}, 17–32: {}, 33–64: {}, > 64: {}", f(0), f(1), f(2), f(3), f(4), f(5), f(6), f(7), f(8)); }
    if *raster::EDGE_AUDIT { let t: Vec<u64> = raster::EDGE_AUDIT_TALLY.iter().map(|a| a.load(std::sync::atomic::Ordering::Relaxed)).collect(); eprintln!("edge audit (cumulative): {} candidate pixels, f32 inside only {} ({:.4} %), integer inside only {} ({:.4} %), {} triangles degenerate after snapping", t[0], t[1], 100.0 * t[1] as f64 / t[0].max(1) as f64, t[2], 100.0 * t[2] as f64 / t[0].max(1) as f64, t[3]); }
    let t_sort = std::time::Instant::now(); crate::pool::stats::stage("csr");
    // THE SPARSE CSR: a counting sort of every job's fragments by wanted rank — the counts were taken by the
    // jobs (`csr_start[k + 1]`); here the prefix in parallel blocks (block sums, then offsets and cursors),
    // then every job scatters its own fragments at its pixels' cursors and sorts its own pixels (a pixel
    // belongs to ONE job: disjoint writes, no atomics; its fragments arrive in triangle order). Per pixel
    // the (depth, triangle) order — the one order a stable sort by depth alone gave on the triangle-ordered
    // input (a triangle visits a pixel once, so the keys are distinct), whatever order the jobs ran in.
    let total: usize = parts.iter().map(|p| p.len()).sum();
    let mut start = csr_start;
    let mut frags: Vec<Frag> = ABUF_FRAGS.take_with_capacity(total);
    let mut fill: Vec<u32> = U32S.take_with_capacity(npx + 1);
    // SAFETY: `fill` is fully written by the prefix pass; every `frags` slot is written exactly once by the
    // job owning its pixel
    unsafe { fill.set_len(npx + 1); frags.set_len(total); }
    let (sp, cp, fp) = (start.as_mut_ptr() as usize, fill.as_mut_ptr() as usize, frags.as_mut_ptr() as usize);
    crate::pool::stats::stage("csr-prefix");
    // THE PREFIX IN ONE PASS: its blocks are the CELL ROWS — a block's fragments are exactly its row's jobs'
    // (a job pushes only its own cell's pixels, the ring rows hold none), so the block bases come from the
    // jobs' list lengths (5 000 adds, serial) instead of a parallel sum pass over the counts (one pool call
    // and one read of the array fewer). Block br's slots: [rank(first pixel of its first row) + 1, the next
    // block's) — slot i holds pixel (i − 1)'s count; block 0 starts at slot 0, the last ends at npx.
    let mut row_frags: Vec<u32> = vec![0; n_brows];
    for (j, p) in parts.iter().enumerate() { row_frags[jobs[j].cell / n_bcols] += p.len() as u32; }
    let mut row_slot: Vec<usize> = Vec::with_capacity(n_brows + 1);
    row_slot.push(0);
    for br in 1..n_brows {
        let y = (clip.1 as usize + br * tile_rows).min(res_y as usize);
        row_slot.push(px.index_of_id((y * res as usize) as u32) as usize + 1);
    }
    row_slot.push(npx + 1);
    let mut base: Vec<u32> = Vec::with_capacity(n_brows + 1);
    base.push(0);
    for s in &row_frags { let l = *base.last().unwrap(); base.push(l + s); }
    {
        let (base, row_slot) = (&base, &row_slot);
        crate::pool::pool().run(n_brows, |b| {
            let (i0, i1) = (row_slot[b], row_slot[b + 1]);
            let mut acc = base[b];
            for i in i0..i1 {
                unsafe {
                    // the INCLUSIVE prefix: slot i held pixel (i − 1)'s count, so Σ_{j ≤ i} = the first
                    // fragment of pixel i = its cursor, and start[npx] = the total (an exclusive prefix
                    // dropped the last pixel's fragments — caught by the check below)
                    acc += *(sp as *mut u32).add(i);
                    *(sp as *mut u32).add(i) = acc;
                    *(cp as *mut u32).add(i) = acc;
                }
            }
        });
    }
    if start[npx] as usize != total { panic!("sparse CSR: the jobs counted {} wanted fragments but pushed {} (npx {npx}, {} jobs)", start[npx], total, parts.len()); }
    // the scatter jobs longest-first (a job's cost here is its fragment count, not its raster estimate)
    let mut scatter_order: Vec<u32> = (0..parts.len() as u32).collect();
    scatter_order.sort_unstable_by_key(|&j| std::cmp::Reverse(parts[j as usize].len()));
    let t_c2 = std::time::Instant::now(); crate::pool::stats::stage("csr-scatter");
    let (ns_scatter, ns_sort) = (std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0));
    {
        let parts = &parts;
        let start = &start;
        let scatter_order = &scatter_order;
        crate::pool::pool().run(parts.len(), |t| {
            let j = scatter_order[t] as usize;
            // (the fused loop has no scatter/sort split: the stats line below books the whole as sort time)
            let t_s = std::time::Instant::now();
            let t_m = t_s;
            // the scatter, and each pixel sorted the moment its last fragment lands (perf 8: the cursor reaching the pixel's
            // end — a pixel belongs to this one job, which writes all its fragments here — replaces the second walk over the
            // job's entries with its cursor read and marking per fragment: a cache miss each)
            let part = &parts[j];
            for (idx, (k, f)) in part.iter().enumerate() {
                // (the cursor and the range of the entry sixteen ahead prefetched: two random reads per fragment)
                if let Some((ka, _)) = part.get(idx + 16) {
                    #[cfg(target_arch = "x86_64")]
                    unsafe {
                        std::arch::x86_64::_mm_prefetch((cp as *const i8).add(*ka as usize * 4), std::arch::x86_64::_MM_HINT_T0);
                        std::arch::x86_64::_mm_prefetch((sp as *const i8).add(*ka as usize * 4), std::arch::x86_64::_MM_HINT_T0);
                    }
                }
                let (a, e) = (start[*k as usize] as usize, start[*k as usize + 1] as usize);
                let done = unsafe {
                    let c = (cp as *mut u32).add(*k as usize);
                    *(fp as *mut Frag).add(*c as usize) = *f;
                    *c += 1;
                    *c as usize == e
                };
                if done && e - a > 1 {
                    let buf = unsafe { std::slice::from_raw_parts_mut((fp as *mut Frag).add(a), e - a) };
                    // the (z, triangle) order — the keys are distinct within a pixel (a triangle visits a pixel once),
                    // so any correct sort gives the one order: networks for the common two and three
                    let after = |p: &Frag, q: &Frag| p.z.total_cmp(&q.z).then_with(|| p.tri.cmp(&q.tri)) == std::cmp::Ordering::Greater;
                    match e - a {
                        2 => { if after(&buf[0], &buf[1]) { buf.swap(0, 1); } }
                        3 => {
                            if after(&buf[0], &buf[1]) { buf.swap(0, 1); }
                            if after(&buf[1], &buf[2]) { buf.swap(1, 2); }
                            if after(&buf[0], &buf[1]) { buf.swap(0, 1); }
                        }
                        _ => buf.sort_unstable_by(|p, q| p.z.total_cmp(&q.z).then_with(|| p.tri.cmp(&q.tri))),
                    }
                }
            }
            if raster_stats { ns_scatter.fetch_add((t_m - t_s).as_nanos() as u64, std::sync::atomic::Ordering::Relaxed); ns_sort.fetch_add(t_m.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed); }
        });
    }
    U32S.give(fill);
    parallel_drop(parts);
    if raster_stats { eprintln!("csr phases: prefix {:.2} ms, scatter + sort {:.2} ms (per-thread: scatter {:.2} ms, sort {:.2} ms) ({} fragments, {} wanted)", (t_c2 - t_sort).as_secs_f64() * 1e3, t_c2.elapsed().as_secs_f64() * 1e3, ns_scatter.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6 / (crate::pool::pool().threads + 1) as f64, ns_sort.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6 / (crate::pool::pool().threads + 1) as f64, total, npx); }
    prof::add(&prof::B_SORT, t_sort);
    (ABuffer { res, band_h: res_y.max(1), bands: vec![(start, frags)], sparse: Some(px.clone()) }, counted)
}

pub fn build_abuffer_wanted(tris: &[WTri], frame: &PeelFrame, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask], wanted: Option<&std::sync::Arc<PixelIndex>>) -> ABuffer {
    if let Some(px) = wanted {
        return build_abuffer_sparse(tris, frame, threads, zmin, zmax, masks, px);
    }
    let res = frame.res;
    let res_y = frame.res_y;
    let t_clip = std::time::Instant::now(); crate::pool::stats::stage("clip");
    // the wanted pixels' bounding rectangle: the raster visits nothing outside it
    let clip: (i32, i32, i32, i32) = match wanted {
        Some(px) => px.bbox,
        None => (0, 0, res as i32 - 1, res_y as i32 - 1),
    };
    prof::add(&prof::B_CLIP, t_clip);
    let t_raster = std::time::Instant::now(); crate::pool::stats::stage("raster");
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
                                        Some((tx, fp)) if !*ALPHA_POINT => if alpha_point_mip() { tx.passes_point(u, v, fp, ALPHA_THRESHOLD) } else { tx.passes_planned(u, v, fp, ALPHA_THRESHOLD, crate::alphatex::Address::ClampEdge) },
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
                                    if abuf_debug_wants(x, y) { abuf_debug_line(x, y, "card", ti, t, &frame, Some((u, v)), Some(op), z); }
                                    if abuf_debug_at(x, y) { let plan_s = match fp { Some((tx, p)) => format!(" lod {:.3} levels {}/{} two {} taps {} axis ({:.4},{:.4}) alpha {:.5} one-tap-l0 {:.5} one-tap-l1 {:.5} by-taps {} taps-of-plan [{}]", p.lod, p.l0, p.l1, p.two, p.n, p.axis[0], p.axis[1], tx.sample_planned_clamp(u, v, p), tx.bilinear_tap(u, v, 0), tx.bilinear_tap(u, v, 1), (1..=8usize).map(|k| { let mut pk = p.clone(); pk.n = k; format!("{k}:{:.5}", tx.sample_planned_clamp(u, v, &pk)) }).collect::<Vec<_>>().join(" "), (0..p.n).map(|i| { let sft = if p.n > 1 { (i as f32 + 0.5) / p.n as f32 - 0.5 } else { 0.0 }; format!("{:.5}", tx.bilinear_tap(u + p.axis[0] * sft, v + p.axis[1] * sft, p.l0)) }).collect::<Vec<_>>().join(" ")), None => String::new() }; eprintln!("abuf debug ({x},{y}) hd={:.2}: card tri {ti} inst {} model tri {} mask {} uv ({u:.4},{v:.4}) opaque {op} z {z:.3} z01 {:.5}{plan_s}", frame.half_d, t.inst, t.tri, t.alpha, frame.z01(z));  }
                                    if !op {
                                        return;
                                    }
                                } else if abuf_debug_wants(x, y) { abuf_debug_line(x, y, "opaque", ti, t, &frame, None, None, z); } else if abuf_debug_at(x, y) { eprintln!("abuf debug ({x},{y}) hd={:.2}: tri {ti} inst {} model tri {} z {z:.3} z01 {:.5}", frame.half_d, t.inst, t.tri, frame.z01(z));  }
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
    let t_sort = std::time::Instant::now(); crate::pool::stats::stage("csr");
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

impl std::fmt::Debug for ShadowMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShadowMap {{ {}×{} }}", self.frame.res, self.frame.res_y)
    }
}

impl ShadowMap {
    pub fn build(tris: &[WTri], sun_dir: V3, bmin: V3, bmax: V3, res: u32, masks: &[crate::geometry::AlphaMask]) -> ShadowMap {
        let frame = PeelFrame::new(sun_dir, bmin, bmax, res);
        Self::build_in(tris, frame, masks)
    }
    /// A shadow map rasterised in a given frame (a captured frustum, or the default fit).
    pub fn build_in(tris: &[WTri], frame: PeelFrame, masks: &[crate::geometry::AlphaMask]) -> ShadowMap {
        if std::env::var_os("LMTOOL_SHADOW_SERIAL").is_none() {
            return Self::build_in_bands(tris, frame, masks);
        }
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
    /// `build_in` BAND-PARALLEL (perf 8): the triangles binned into pixel-row bands (in triangle order), every band
    /// rasterising its list with the rows clipped to the band — a pixel lies in one band, whose triangles arrive in the
    /// serial order, so its depth test sees the same fragments in the same order: the same map (the serial build
    /// took 8.2 s on Stadium stpad, 1 s on the giant; LMTOOL_SHADOW_SERIAL=1 keeps it).
    pub fn build_in_bands(tris: &[WTri], frame: PeelFrame, masks: &[crate::geometry::AlphaMask]) -> ShadowMap {
        let (res, res_y) = (frame.res, frame.res_y);
        let threads = crate::pool::pool().threads.max(1);
        let n_bands = (threads * 2).clamp(1, res_y as usize);
        let rows = (res_y as usize + n_bands - 1) / n_bands;
        let skip_cards = !cards_occlude() || !cards_shadow();
        // 1. the bands each triangle touches (from its projected y range, as the rasteriser bounds it), per chunk of
        //    triangles in order
        let n = tris.len();
        let n_chunks = (threads * 4).max(1);
        let per = (n + n_chunks - 1) / n_chunks;
        let frame_ref = &frame;
        let binned: Vec<Vec<Vec<u32>>> = crate::pool::pool().map(n_chunks, |ci| {
            let mut out: Vec<Vec<u32>> = vec![Vec::new(); n_bands];
            for ti in (ci * per).min(n)..((ci + 1) * per).min(n) {
                let t = &tris[ti];
                if skip_cards && t.alpha != u16::MAX { continue; }
                let p0 = t.p0;
                let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                let ys = [frame_ref.project(p0).1, frame_ref.project(p1).1, frame_ref.project(p2).1];
                if !ys.iter().all(|y| y.is_finite()) { continue; }
                let (miny, maxy) = (ys.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(f32::MIN, f32::max));
                // pixel centres y + 0.5 in [miny, maxy] (raster::bounds), a row each side to be safe
                let y0 = ((miny - 0.5).floor() as i64 - 1).max(0);
                let y1 = ((maxy - 0.5).ceil() as i64 + 1).min(res_y as i64 - 1);
                if y0 > y1 { continue; }
                let (b0, b1) = ((y0 as usize) / rows, ((y1 as usize) / rows).min(n_bands - 1));
                for b in b0..=b1 { out[b].push(ti as u32); }
            }
            out
        });
        // 2. the bands, each over its triangles in order with the rows clipped to it
        let mut depth = raster::Depth::new(res, res_y);
        let zp = depth.z.as_mut_ptr() as usize;
        crate::pool::pool().run(n_bands, |b| {
            let (y_lo, y_hi) = ((b * rows).min(res_y as usize) as i32, (((b + 1) * rows).min(res_y as usize)) as i32);
            if y_lo >= y_hi { return; }
            let clip = (0i32, y_lo, res as i32 - 1, y_hi - 1);
            for chunk in &binned {
                for &ti in &chunk[b] {
                    let t = &tris[ti as usize];
                    let p0 = t.p0;
                    let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                    let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                    let (x0, y0, z0) = frame_ref.project(p0);
                    let (x1, y1, z1) = frame_ref.project(p1);
                    let (x2, y2, z2) = frame_ref.project(p2);
                    let mask = if t.alpha != u16::MAX { masks.get(t.alpha as usize) } else { None };
                    raster::triangle_clipped(res, res_y, [[x0, y0], [x1, y1], [x2, y2]], clip, |x, y, bb| {
                        if let Some(m) = mask {
                            let u = t.uv0[0][0] * bb[0] + t.uv0[1][0] * bb[1] + t.uv0[2][0] * bb[2];
                            let v = t.uv0[0][1] * bb[0] + t.uv0[1][1] * bb[1] + t.uv0[2][1] * bb[2];
                            if !m.opaque(u, v) {
                                return;
                            }
                        }
                        let z = z0 * bb[0] + z1 * bb[1] + z2 * bb[2];
                        let i = (y * res + x) as usize;
                        // SAFETY: the bands own disjoint pixel rows
                        unsafe {
                            let slot = (zp as *mut f32).add(i);
                            if z < *slot { *slot = z; }
                        }
                    });
                }
            }
        });
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
    // (a min / max fold: order-free, so in parallel chunks — the serial fold over the giant's 27 M triangles was 0.2 s per sweep)
    let n = tris.len();
    let threads = crate::pool::pool().threads.max(1);
    let chunk = (n / (threads * 2).max(1)).max(4096);
    let parts: Vec<(V3, V3)> = crate::pool::pool().map((n + chunk - 1) / chunk, |ci| {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for t in tris[ci * chunk..((ci + 1) * chunk).min(n)].iter().filter(|t| t.inst != DECOR_INST) {
            for p in [t.p0, [t.p0[0] + t.e1[0], t.p0[1] + t.e1[1], t.p0[2] + t.e1[2]], [t.p0[0] + t.e2[0], t.p0[1] + t.e2[1], t.p0[2] + t.e2[2]]] {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        (lo, hi)
    });
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (l, h) in &parts {
        for k in 0..3 {
            lo[k] = lo[k].min(l[k]);
            hi[k] = hi[k].max(h[k]);
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
    // THE FIRST SWEEP WITHOUT A COLOUR SOURCE (no ILightInput atlas, no transcribed atlas, no stored field, the
    // sun off): an item fragment's colour below is alb · (0 + sun · ndl · 0) = alb · (+0) — the sign of alb on a
    // zero — or +0 for a back face; computed as such without the normal, the shadow and the field lookups
    // (the game-peel front test is one cross product and one dot, as below)
    if prm.sweep == 0 && !prm.sweep0_sun && prm.ilight_atlas.is_none() && prm.ilatlas.is_none() && prm.field.is_none() && wt.inst != DECOR_INST && prm.game_peel {
        if !(dot(cross(wt.e1, wt.e2), d) < 0.0) {
            return [0.0; 3];
        }
        let alb = hit_albedo(scene, bvh, prm, &Hit { t: 0.0, tri });
        return [alb[0] * 0.0, alb[1] * 0.0, alb[2] * 0.0];
    }
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
                // (÷ 1 is the identity — the default BounceFactor — so the three divisions run only for another factor)
                f.lookup(scene, wt.inst, wt.tri, b1, b2).map(|e| if prm.bounce_decode == 1.0 { e } else { [e[0] / prm.bounce_decode, e[1] / prm.bounce_decode, e[2] / prm.bounce_decode] }).unwrap_or([0.0; 3])
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

/// THE PER-TRIANGLE HALF OF `fragment_radiance` (the layer derivation's cache): everything the port's own colour
/// model derives from the triangle and the frame alone — the front test, the oriented normal, the albedo, n·L,
/// the decoration's constant stand-in for the stored lightmap, the water's sky reflection — computed once per
/// triangle per chunk by exactly the statements `fragment_radiance` runs (the same functions on the same inputs);
/// `fragment_radiance_at` then adds the per-fragment part (the stored field at the hit, the sun visibility) in
/// `fragment_radiance`'s order of operations. Not for the harness's atlas paths (`--ilightinput-from`, the
/// transcribed atlas, the tiles' ST) nor the first sweep's short cut — `tri_shade_applies` says when.
#[derive(Clone, Copy)]
pub struct TriShade {
    pub front: bool,
    pub n: V3,
    pub alb: [f32; 3],
    /// The decoration's stand-in for the stored lightmap (a constant); None = read the field at the hit.
    pub stored_const: Option<[f32; 3]>,
    pub ndl: f32,
    /// The water's additions, in order: the reflected sky × reflectance, then the sun's glitter (when any).
    pub water: Option<([f32; 3], Option<[f32; 3]>)>,
}

impl TriShade {
    pub const NONE: TriShade = TriShade { front: false, n: [0.0; 3], alb: [0.0; 3], stored_const: None, ndl: 0.0, water: None };
}

pub fn tri_shade_applies(prm: &BakeParams) -> bool {
    let shortcut = prm.sweep == 0 && !prm.sweep0_sun && prm.ilight_atlas.is_none() && prm.ilatlas.is_none() && prm.field.is_none() && prm.game_peel;
    !shortcut && prm.ilight_atlas.is_none() && prm.ilatlas.is_none()
}

pub fn tri_shade(scene: &Scene, bvh: &Bvh, prm: &BakeParams, tri: u32, d: V3) -> TriShade {
    let wt = &bvh.tris[tri as usize];
    let ng = norm(cross(wt.e1, wt.e2));
    let facing = -dot(ng, d);
    let n = if facing >= 0.0 { ng } else { [-ng[0], -ng[1], -ng[2]] };
    let is_front = if prm.game_peel {
        dot(cross(wt.e1, wt.e2), d) < 0.0
    } else if wt.inst == DECOR_INST {
        true
    } else {
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
        return TriShade { front: false, n, alb: [0.0; 3], stored_const: None, ndl: 0.0, water: None };
    }
    let h = Hit { t: 0.0, tri };
    let alb = hit_albedo(scene, bvh, prm, &h);
    let stored_const: Option<[f32; 3]> = match &prm.field {
        Some(_) if wt.inst != DECOR_INST => None,
        _ if wt.inst == DECOR_INST && (prm.sweep > 0 || prm.sweep0_sun) => { let s = prm.decor_sky_up; Some([s[0] * prm.decor_ambient, s[1] * prm.decor_ambient, s[2] * prm.decor_ambient]) },
        _ => Some([0.0; 3]),
    };
    let is_card = wt.alpha != u16::MAX;
    let ndl = if is_card && !prm.card_one_sided { dot(n, prm.sun_dir).abs() } else { dot(n, prm.sun_dir).max(0.0) };
    let mut water = None;
    if wt.inst == DECOR_INST && prm.water_reflect > 0.0 {
        if let Some(dt) = scene.decor.get(wt.tri as usize) {
            if dt.water {
                let r = [d[0], -d[1], d[2]];
                if r[1] > 0.0 {
                    let s = sky_radiance(prm, r);
                    let w1 = [prm.water_reflect * s[0], prm.water_reflect * s[1], prm.water_reflect * s[2]];
                    let w2 = if prm.water_sun > 0.0 && prm.sun_dir[1] > 0.0 {
                        let c = dot(r, prm.sun_dir).max(0.0);
                        let f = prm.water_sun * c.powf(prm.water_sun_pow);
                        Some([f * prm.sun[0], f * prm.sun[1], f * prm.sun[2]])
                    } else { None };
                    water = Some((w1, w2));
                }
            }
        }
    }
    TriShade { front: true, n, alb, stored_const, ndl, water }
}

/// `fragment_radiance` from its cached per-triangle half: the stored field at the hit (an item with a field),
/// the sun's visibility, then `alb · (stored + sun · ndl · lit)` and the water's additions — the same
/// expressions in the same order as the full function.
pub fn fragment_radiance_at(scene: &Scene, bvh: &Bvh, prm: &BakeParams, shadow: Option<&ShadowMap>, ts: &TriShade, tri: u32, hit_p: V3, sun_bias: f32) -> [f32; 3] {
    if !ts.front {
        return [0.0; 3];
    }
    let wt = &bvh.tris[tri as usize];
    let stored: [f32; 3] = match ts.stored_const {
        Some(s) => s,
        None => {
            let f = prm.field.as_ref().unwrap();
            let v = sub(hit_p, wt.p0);
            let (d00, d01, d11, d20, d21) = (dot(wt.e1, wt.e1), dot(wt.e1, wt.e2), dot(wt.e2, wt.e2), dot(v, wt.e1), dot(v, wt.e2));
            let den = d00 * d11 - d01 * d01;
            if den.abs() < 1e-12 {
                [0.0; 3]
            } else {
                let b1 = ((d11 * d20 - d01 * d21) / den).clamp(0.0, 1.0);
                let b2 = ((d00 * d21 - d01 * d20) / den).clamp(0.0, 1.0);
                // (÷ 1 is the identity — the default BounceFactor — so the three divisions run only for another factor)
                f.lookup(scene, wt.inst, wt.tri, b1, b2).map(|e| if prm.bounce_decode == 1.0 { e } else { [e[0] / prm.bounce_decode, e[1] / prm.bounce_decode, e[2] / prm.bounce_decode] }).unwrap_or([0.0; 3])
            }
        }
    };
    let ndl = ts.ndl;
    let sun_on = prm.sweep > 0 || prm.sweep0_sun;
    let lit = if sun_on && ndl > 0.0 && prm.sun_dir[1] > 0.0 { shadow.map(|s| s.lit(hit_p, sun_bias)).unwrap_or(1.0) } else { 0.0 };
    if sun_stats_on() {
        SUN_STATS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if ndl > 0.0 { SUN_STATS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
        if lit > 0.0 { SUN_STATS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
    }
    let mut out = [0f32; 3];
    for k in 0..3 {
        out[k] = ts.alb[k] * (stored[k] + prm.sun[k] * ndl * lit);
    }
    if let Some((w1, w2)) = ts.water {
        for k in 0..3 { out[k] += w1[k]; }
        if let Some(w2) = w2 { for k in 0..3 { out[k] += w2[k]; } }
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
/// Where the layer derivation writes a pixel's layers: a run of `LayerFrag` slots (a chunk's slice of the final
/// array, or a vector's spare capacity) — `push` never grows it; the caller sized it from the A-buffer.
pub struct LayerSink {
    ptr: *mut LayerFrag,
    len: usize,
    cap: usize,
}
impl LayerSink {
    /// SAFETY (of the use): `ptr` must stay valid for `cap` slots while the sink lives, and no other sink may
    /// cover the same slots.
    pub unsafe fn new(ptr: *mut LayerFrag, cap: usize) -> LayerSink { LayerSink { ptr, len: 0, cap } }
    #[inline(always)]
    pub fn len(&self) -> usize { self.len }
    /// Slot i (written: i < len).
    #[inline(always)]
    pub fn get(&self, i: usize) -> LayerFrag {
        assert!(i < self.len, "layer sink read past the written slots");
        // SAFETY: i < len, and slots below len were written by push
        unsafe { std::ptr::read(self.ptr.add(i)) }
    }
    #[inline(always)]
    pub fn push(&mut self, f: LayerFrag) {
        debug_assert!(self.len < self.cap, "layer sink overflow");
        // SAFETY: len < cap (the caller bounds every pixel's layers by its fragments + the dome layer)
        unsafe { std::ptr::write(self.ptr.add(self.len), f); }
        self.len += 1;
    }
}

pub struct Layers {
    pub w: u32,
    pub h: u32,
    pub start: Vec<u32>,
    /// The layers per pixel when the lists are not contiguous (the sparse derive writes every chunk's lists
    /// straight into `frags` at a base sized from the A-buffer, leaving a gap after each chunk); None = pixel i's
    /// list ends where pixel i + 1's starts.
    pub cnt: Option<Vec<u8>>,
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
    /// Pixel (or wanted rank) i's slots in `frags`.
    #[inline(always)]
    pub fn range(&self, i: usize) -> (usize, usize) {
        let a = self.start[i] as usize;
        match &self.cnt {
            Some(c) => (a, a + c[i] as usize),
            None => (a, self.start[i + 1] as usize),
        }
    }
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> &[LayerFrag] {
        if let Some(px) = &self.sparse {
            let Some(k) = px.index(x, y) else { return &[] };
            let (a, c) = self.range(k as usize);
            return &self.frags[a..c];
        }
        let i = (y * self.w + x) as usize;
        let (a, c) = self.range(i);
        &self.frags[a..c]
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
    /// The per-pixel fragment offsets of the WHOLE frame (`w · h + 1` entries): the dense form's `start` as it is; the sparse
    /// form's spread over every pixel — an unwanted pixel gets an empty range (its offset = the next wanted pixel's), so
    /// `frags[start[p] + k]` for `start[p] + k < start[p + 1]` is layer k at pixel p, the clear otherwise — what `layer_image`
    /// materialises per layer, read in place instead (perf 8, `lmaccum::LayerSparse`).
    /// The layers with every pixel's list contiguous (`start[k]..start[k + 1]`): the layers themselves when they are
    /// (`cnt` None), else a compacted copy of the in-place derive's gapped array — for the readers that index
    /// `start[p + 1]` directly (`dense_start` / `LayerSparse`); `at`, `range` and the images need no copy.
    pub fn contiguous(&self) -> Option<Layers> {
        let cnt = self.cnt.as_ref()?;
        let n = cnt.len();
        let mut start: Vec<u32> = U32S.take_with_capacity(n + 1);
        start.clear();
        // the prefix sum of the counts in parallel (perf 8.19: 5 M serial pushes per peel, ten peels per giant direction —
        // 0.1 s of serial glue per direction): per-chunk totals, then each chunk's offsets from its base; integer sums, so
        // the same array
        let threads = crate::pool::pool().threads.max(1);
        let chunk = (n / (threads * 4).max(1)).max(4096);
        let n_chunks = (n + chunk - 1) / chunk;
        let sums: Vec<u32> = crate::pool::pool().map(n_chunks, |ci| cnt[ci * chunk..((ci + 1) * chunk).min(n)].iter().map(|&c| c as u32).sum());
        let mut base: Vec<u32> = Vec::with_capacity(n_chunks + 1);
        base.push(0);
        for s in &sums { let l = *base.last().unwrap(); base.push(l + s); }
        // SAFETY: every entry 0..=n is written below (entry n = the total, the chunks their own ranges)
        unsafe { start.set_len(n + 1); }
        start[n] = base[n_chunks];
        let sp = start.as_mut_ptr() as usize;
        crate::pool::pool().run(n_chunks, |ci| {
            let mut l = base[ci];
            for i in ci * chunk..((ci + 1) * chunk).min(n) {
                unsafe { *(sp as *mut u32).add(i) = l; }
                l += cnt[i] as u32;
            }
        });
        let total = start[n] as usize;
        let mut frags: Vec<LayerFrag> = LAYER_FRAGS.take_with_capacity(total);
        // SAFETY: every slot [start[i], start[i + 1]) is written below by the chunk owning pixel i
        unsafe { frags.set_len(total); }
        let (fp, sp) = (frags.as_mut_ptr() as usize, &start);
        let threads = crate::pool::pool().threads.max(1);
        let chunk = (n / (threads * 4).max(1)).max(1024);
        crate::pool::pool().run((n + chunk - 1) / chunk, |ci| {
            for i in ci * chunk..((ci + 1) * chunk).min(n) {
                let (a, c) = self.range(i);
                unsafe { std::ptr::copy_nonoverlapping(self.frags.as_ptr().add(a), (fp as *mut LayerFrag).add(sp[i] as usize), c - a); }
            }
        });
        Some(Layers { w: self.w, h: self.h, start, cnt: None, frags, max_layers: self.max_layers, sparse: self.sparse.clone(), item_layers: self.item_layers, fractions: self.fractions.clone() })
    }
    pub fn dense_start(&self) -> std::borrow::Cow<'_, [u32]> {
        let Some(px) = &self.sparse else { return std::borrow::Cow::Borrowed(&self.start) };
        let n = (self.w * self.h) as usize;
        let mut out: Vec<u32> = Vec::with_capacity(n + 1);
        // SAFETY: every entry is written below by the chunk that owns it
        unsafe { out.set_len(n + 1); }
        out[n] = *self.start.last().unwrap_or(&0);
        let op = out.as_mut_ptr() as usize;
        let threads = crate::pool::pool().threads.max(1);
        let chunk = (n / (threads * 4).max(1)).max(4096);
        crate::pool::pool().run((n + chunk - 1) / chunk, |ci| {
            for p in ci * chunk..((ci + 1) * chunk).min(n) {
                // the rank of pixel p = the wanted pixels before it = its dense index when wanted, else the next one's
                let k = px.index_of_id(p as u32) as usize;
                unsafe { *(op as *mut u32).add(p) = self.start[k]; }
            }
        });
        std::borrow::Cow::Owned(out)
    }
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
                        let (a, c) = self.range(i);
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
                        let (a, c) = self.range(r);
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
fn extract_layers(ab: &ABuffer, frame: &PeelFrame, scene: &Scene, bvh: &Bvh, prm: &BakeParams, shadow: Option<&ShadowMap>, sun_bias: f32, sky: [f32; 3], threads: usize, wanted: Option<&std::sync::Arc<PixelIndex>>, fixed_layers: Option<usize>, dome_img: Option<&(dyn Fn(u32, u32) -> [f32; 3] + Sync)>) -> Layers {
    let (w, h) = (frame.res, frame.res_y);
    let n = (w * h) as usize;
    // THE CAPTURE (pwc6 frame 7534, the first sweep-1 direction's layer 0): the environment render of a LATER sweep is
    // BLACK — the sky enters the accumulation once, in sweep 0 (the sweep-1 layer-0 snapshots hold 0 at every dome
    // pixel; the geometry layers carry the bounce ILightInput)
    let sky_q = if prm.sweep > 0 { [0.0f32; 3] } else { prm.quant_peel.apply(sky, prm.rounding) };
    let dome_img = if prm.sweep > 0 { None } else { dome_img };
    let skip_n = if prm.dome_layer { 1usize } else { 0 };
    // the item layers derived per pixel: MAX_LAYERS (the exact path, untouched), or under the non-exact
    // `--max-layers N` (prm.peel_stop.max_renders < the game's 21) the cap's item count — the stop rule
    // never keeps more, so the derivations past it were wasted
    let derive_cap: usize = if prm.peel_stop.max_renders < crate::peelcap::PeelStop::default().max_renders { prm.peel_stop.max_renders.saturating_sub(skip_n).clamp(1, MAX_LAYERS) } else { MAX_LAYERS };
    // THE FIRST SWEEP WITHOUT A COLOUR SOURCE, STORED AS R11G11B10: every item fragment's colour is ±0 (see
    // fragment_radiance's short cut: black for a back face, alb · 0 for a front face) and the R11G11B10 target
    // stores +0 for both (the format has no sign bit; the decode of 0 is +0.0) — so the colour path (the
    // albedo lookup, the front test, the quantiser) is skipped for item fragments: the value is [0, 0, 0]
    // either way. (F16 keeps the sign of −0 and Quant::None keeps the value: no short cut there.)
    let fast_black = prm.sweep == 0 && !prm.sweep0_sun && prm.ilight_atlas.is_none() && prm.ilatlas.is_none() && prm.field.is_none() && prm.game_peel && prm.quant_peel == crate::gpufmt::Quant::R11G11B10;
    // THE BIAS TERM PER TRIANGLE, cached: `tri_slope` re-projects the triangle's three vertices per fragment
    // (five divisions); a chunk's pixels see the same triangles again and again (the ground, a card over
    // many pixels), but a pixel's own fragments alternate triangles, so a one-entry cache missed — this is a
    // direct-mapped table of 1024 (triangle, term) entries per chunk, the term computed exactly as before on
    // a miss (the same function on the same inputs: the same value)
    const BIAS_CACHE: usize = 4096;
    let bias_term = |cache: &mut [(u32, f32); BIAS_CACHE], tri: u32, wt: &WTri| -> f32 {
        let slot = &mut cache[(tri as usize) & (BIAS_CACHE - 1)];
        if slot.0 == tri { return slot.1; }
        let (slope, zmax_prim) = tri_slope(wt, frame);
        // an edge-on triangle's slope is huge (D3D applies it uncapped, DepthBiasClamp 0)
        let term = d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), prm.depth_bias, prm.depth_bits);
        *slot = (tri, term);
        term
    };
    // one pixel's layers appended to `out`
    let shade_cached = tri_shade_applies(prm);
    // (`cap_total` = the most layers a pixel keeps, the dome layer included: skip_n + the rendered item layers when
    // the count is known before the derive, else skip_n + derive_cap and the cut comes after)
    let derive_pixel = |x: usize, y: usize, list: &[Frag], out: &mut LayerSink, cache: &mut [(u32, f32); BIAS_CACHE], shade: &mut [(u32, TriShade); BIAS_CACHE], cap_total: usize| {
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
                // (perf 8: the dome colour evaluated HERE, at the pixels the environment mesh leaves uncovered — not an image
                // over every wanted pixel first: on the giant 14 core-seconds per direction, most of it under the terrain)
                out.push(LayerFrag { d: 0.0, rgb: dome_img.map(|f| f(x as u32, y as u32)).unwrap_or(sky_q) });
                d_prev = 0.0;
            }
        }
        let traced = LAYER_DEBUG_SET.as_ref().map(|set| set.contains(&(x as u32, y as u32))).unwrap_or(false);
        if traced {
            eprintln!("LAYERDBG frame={}x{} dir=({:.6},{:.6},{:.6}) px={x} py={y} nfrag={} env_layer_d={} sweep={} fixed_layers={:?}", frame.res, frame.res_y, frame.d[0], frame.d[1], frame.d[2], list.len(), if out.len() > before { format!("{:.6}", out.get(before).d) } else { "none".into() }, prm.sweep, fixed_layers);
        }
        // THE BIASED ORDER (BIASED_ORDER): the item fragments walked in the order of their STORED depth — the
        // layer is the smallest stored depth among the fragments whose unbiased depth passes the previous
        // layer — via `layer_walk_biased`; the accepted ones then take the colour path below in that order.
        // The unbiased rule (the former one) walks `list` in its (z, tri) order.
        // the walk order: the biased rule's accepted fragments (thread-local scratch — no allocation per pixel),
        // or every fragment of the list under the former rule
        let biased = *BIASED_ORDER;
        let mut scratch = if biased { Some(DERIVE_SCRATCH.take()) } else { None };
        if let Some((order_buf, acc)) = scratch.as_mut() {
            order_buf.clear();
            acc.clear();
            for (i, f) in list.iter().enumerate() {
                if (prm.dome_layer || !prm.env_in_peel) && is_env(f.tri) {
                    if traced { eprintln!("LAYERDBG px={x} py={y} frag tri={} z={:.6} env=1 skipped=env", f.tri, f.z); }
                    continue;
                }
                let z01 = frame.z01(f.z).max(0.0);
                let wt = &bvh.tris[f.tri as usize];
                order_buf.push(WalkFrag::from_depths(z01, bias_term(cache, f.tri, wt), prm.depth_bits, draw_rank_of(f.tri), i as u32));
            }
            // (d_prev = the env layer's stored depth, or 0 at the dome, or −∞ without the dome layer)
            layer_walk_biased_capped(order_buf, d_prev, item_cap(prm.dome_layer), prm.depth_bits, acc);
            if traced {
                for wf in order_buf.iter() {
                    if !acc.contains(&wf.idx) { let f = &list[wf.idx as usize]; let wt = &bvh.tris[f.tri as usize]; eprintln!("LAYERDBG px={x} py={y} frag tri={} inst={} mtri={} z={:.6} z01={:.6} q16={} skipped=merged (biased order)", f.tri, wt.inst, wt.tri, f.z, wf.z01, (wf.dd(prm.depth_bits) * 65535.0).round() as u32); }
                }
            }
        }
        let n_order = match scratch.as_ref() { Some((_, acc)) => acc.len(), None => list.len() };
        for oi in 0..n_order {
            let fi = match scratch.as_ref() { Some((_, acc)) => acc[oi] as usize, None => oi };
            let f = &list[fi];
            // (the environment is not re-drawn in the geometry layers; in a sweep without an environment block
            // it is not drawn at all)
            if !biased && (prm.dome_layer || !prm.env_in_peel) && is_env(f.tri) {
                if traced { eprintln!("LAYERDBG px={x} py={y} frag tri={} z={:.6} env=1 skipped=env", f.tri, f.z); }
                continue;
            }
            // pancaking: a fragment beyond the far plane lands on it (z01 = 0)
            let z01 = frame.z01(f.z).max(0.0);
            if !biased && z01 < d_prev {
                if traced { let wt = &bvh.tris[f.tri as usize]; eprintln!("LAYERDBG px={x} py={y} frag tri={} inst={} mtri={} z={:.6} z01={:.6} q16={} skipped=merged d_prev={:.6}", f.tri, wt.inst, wt.tri, f.z, z01, (z01 * 65535.0).round() as u32, d_prev); }
                continue;
            }
            if out.len() - before >= cap_total {
                if traced { eprintln!("LAYERDBG px={x} py={y} frag tri={} z={:.6} z01={:.6} skipped=cap", f.tri, f.z, z01); }
                break;
            }
            let wt = &bvh.tris[f.tri as usize];
            let mut d = z01 + bias_term(cache, f.tri, wt);
            if prm.depth_bits == 16 {
                // the D16_UNORM target stores 65535 steps
                d = (d.clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
            }
            // (the decoration too, except the water: its sky reflection is the one non-zero colour of the sweep)
            let rgb = if fast_black && (wt.inst != DECOR_INST || !scene.decor.get(wt.tri as usize).map_or(false, |dt| dt.water)) {
                [0.0f32; 3]
            } else {
                let hit_p = frame.unproject(x as f32 + 0.5, y as f32 + 0.5, f.z);
                if shade_cached {
                    // the per-triangle half from the chunk's cache (computed on a miss by the same statements), the
                    // per-fragment half in the full function's order
                    let slot = &mut shade[(f.tri as usize) & (BIAS_CACHE - 1)];
                    if slot.0 != f.tri { *slot = (f.tri, tri_shade(scene, bvh, prm, f.tri, frame.d)); }
                    prm.quant_peel.apply(fragment_radiance_at(scene, bvh, prm, shadow, &slot.1, f.tri, hit_p, sun_bias), prm.rounding)
                } else {
                    prm.quant_peel.apply(fragment_radiance(scene, bvh, prm, shadow, frame, f.tri, frame.d, hit_p, sun_bias), prm.rounding)
                }
            };
            if traced {
                let wt = &bvh.tris[f.tri as usize];
                let front = dot(cross(wt.e1, wt.e2), frame.d) < 0.0;
                eprintln!("LAYERDBG px={x} py={y} frag tri={} inst={} mtri={} card={} z={:.6} z01={:.6} bias={:.3e} q16={} accepted layer={} front={} rgb=({:.5},{:.5},{:.5})", f.tri, wt.inst, wt.tri, wt.alpha != u16::MAX, f.z, z01, bias_term(cache, f.tri, wt), (d * 65535.0).round() as u32, out.len() - before, front, rgb[0], rgb[1], rgb[2]);
            }
            out.push(LayerFrag { d, rgb });
            d_prev = d;
        }
        if traced { eprintln!("LAYERDBG px={x} py={y} derived layers={} (env {} + items {}); the frame's cut applies skip_n + rendered", out.len() - before, skip_n, out.len() - before - skip_n); }
        if let Some(sc) = scratch { DERIVE_SCRATCH.set(sc); }
    };
    if let Some(px) = wanted {
        // SPARSE: the wanted pixels only, in parallel chunks of the dense index
        let npx = px.len();
        // (eight tasks per thread: a chunk in a dense region — many fragments per pixel — takes several times a
        // chunk of open ground, and one chunk per thread left the others waiting for it)
        let chunk = (npx / (threads.max(1) * 8)).max(1024);
        let n_chunks = (npx + chunk - 1) / chunk;
        crate::pool::stats::checkpoint("(layers preamble)");
        let t_par = std::time::Instant::now(); crate::pool::stats::stage("layers");
        // a chunk's output never exceeds its A-buffer fragments plus one dome layer per pixel
        let bound_of = |ci: usize| -> usize {
            let (k0, k1) = (ci * chunk, ((ci + 1) * chunk).min(npx));
            ab.bands[0].0[k1] as usize - ab.bands[0].0[k0] as usize + (k1 - k0)
        };
        let t_par = std::time::Instant::now();
        if let (Some(kept), false) = (fixed_layers, *INPLACE_OFF || INPLACE_OFF_FORCE.load(std::sync::atomic::Ordering::Relaxed)) {
            // THE DIRECT FORM (the count known before the derive — the exact count pass or the captured one):
            // every chunk writes its pixels' lists straight into the final array at a base sized from its
            // A-buffer bound, the pixels' starts and counts beside them — no per-chunk vectors, no totals pass,
            // no copy (a 100 MB read and write per tiny world frame); the gap after each chunk is dead space
            // the per-pixel counts hide (`Layers::range`)
            let kept = kept.min(derive_cap);
            let cap_total = skip_n + kept;
            let mut base: Vec<usize> = Vec::with_capacity(n_chunks + 1);
            base.push(0);
            for ci in 0..n_chunks { let l = *base.last().unwrap(); base.push(l + bound_of(ci)); }
            let total_bound = *base.last().unwrap();
            let mut start: Vec<u32> = U32S.take_with_capacity(npx + 1);
            let mut cnt: Vec<u8> = vec![0u8; npx];
            let mut frags: Vec<LayerFrag> = LAYER_FRAGS.take_with_capacity(total_bound);
            // SAFETY: every start/cnt slot is written by the chunk owning the pixel; the frag slots read later are
            // exactly the written ones (start + cnt per pixel); the gaps are never read
            unsafe { start.set_len(npx + 1); frags.set_len(total_bound); }
            start[npx] = total_bound as u32;
            let (sp, cp, fp) = (start.as_mut_ptr() as usize, cnt.as_mut_ptr() as usize, frags.as_mut_ptr() as usize);
            let base = &base;
            crate::pool::pool().run(n_chunks, |ci| {
                let ids = &px.pixels[ci * chunk..((ci + 1) * chunk).min(npx)];
                let k0 = ci * chunk;
                let mut out = unsafe { LayerSink::new((fp as *mut LayerFrag).add(base[ci]), base[ci + 1] - base[ci]) };
                let mut cache = [(u32::MAX, 0.0f32); BIAS_CACHE];
                let mut shade = [(u32::MAX, TriShade::NONE); BIAS_CACHE];
                for (i, &id) in ids.iter().enumerate() {
                    let before = out.len();
                    if i + 1 < ids.len() {
                        for f in ab.at_rank(k0 + i + 1).iter().take(4) {
                            crate::peel::prefetch(&bvh.tris[f.tri as usize]);
                        }
                    }
                    derive_pixel((id % w) as usize, (id / w) as usize, ab.at_rank(k0 + i), &mut out, &mut cache, &mut shade, cap_total);
                    unsafe {
                        *(sp as *mut u32).add(k0 + i) = (base[ci] + before) as u32;
                        *(cp as *mut u8).add(k0 + i) = (out.len() - before) as u8;
                    }
                }
            });
            prof::add(&prof::L_PAR, t_par);
            LAYER_STATS.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let inplace = Layers { w, h, start, cnt: Some(cnt), frags, max_layers: MAX_LAYERS, sparse: Some(px.clone()), item_layers: kept, fractions: Vec::new() };
            if std::env::var_os("LMTOOL_INPLACE_CHECK").is_some() {
                // THE CHECK: the vector form of the same derive, pixel by pixel
                INPLACE_OFF_FORCE.store(true, std::sync::atomic::Ordering::Relaxed);
                let vector = extract_layers(ab, frame, scene, bvh, prm, shadow, sun_bias, sky, threads, wanted, fixed_layers, dome_img);
                INPLACE_OFF_FORCE.store(false, std::sync::atomic::Ordering::Relaxed);
                let mut bad = 0usize;
                for k in 0..npx {
                    let (a, c) = inplace.range(k); let (a2, c2) = vector.range(k);
                    let (la, lb) = (&inplace.frags[a..c], &vector.frags[a2..c2]);
                    let same = la.len() == lb.len() && la.iter().zip(lb.iter()).all(|(p, q)| p.d.to_bits() == q.d.to_bits() && p.rgb.iter().zip(q.rgb.iter()).all(|(u, v)| u.to_bits() == v.to_bits()));
                    if !same { bad += 1; if bad <= 5 { let id = px.pixels[k]; eprintln!("INPLACE CHECK: pixel ({}, {}) rank {k}: in-place {:?} vs vector {:?}", id % w, id / w, la, lb); } }
                }
                eprintln!("INPLACE CHECK: {} of {npx} pixels differ (kept {kept}, cap_total {cap_total}, skip_n {skip_n}, item_layers {} vs {})", bad, inplace.item_layers, vector.item_layers);
            }
            return inplace;
        }
        let parts: Vec<(Vec<u32>, Vec<LayerFrag>)> = crate::pool::pool().map(n_chunks, |ci| {
            let ids = &px.pixels[ci * chunk..((ci + 1) * chunk).min(npx)];
            let mut counts = Vec::with_capacity(ids.len());
            let k0 = ci * chunk;
            // (sized from the A-buffer's fragment count for the chunk: no growth copies)
            let bound = bound_of(ci);
            let mut out_v: Vec<LayerFrag> = Vec::with_capacity(bound);
            let mut out = unsafe { LayerSink::new(out_v.as_mut_ptr(), bound) };
            let mut cache = [(u32::MAX, 0.0f32); BIAS_CACHE];
            let mut shade = [(u32::MAX, TriShade::NONE); BIAS_CACHE];
            for (i, &id) in ids.iter().enumerate() {
                let before = out.len();
                // (the next pixel's fragments' triangles fetched ahead: the per-fragment `bvh.tris[tri]` read is a
                // cache miss for anything but the ground)
                if i + 1 < ids.len() {
                    for f in ab.at_rank(k0 + i + 1).iter().take(4) {
                        crate::peel::prefetch(&bvh.tris[f.tri as usize]);
                    }
                }
                derive_pixel((id % w) as usize, (id / w) as usize, ab.at_rank(k0 + i), &mut out, &mut cache, &mut shade, skip_n + derive_cap);
                counts.push((out.len() - before) as u32);
            }
            // SAFETY: the sink wrote out.len() ≤ bound slots of the vector's capacity
            unsafe { out_v.set_len(out.len()); }
            (counts, out_v)
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
            Some(k) => k.min(derive_cap),
            None => prm.peel_stop.layers_rendered_after(&fractions, skip_n),
        };
        let candidates = fractions.iter().take_while(|f| **f > 0.0).count();
        LAYER_STATS.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if kept < candidates {
            LAYER_STATS.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        if std::env::var_os("LMTOOL_BOUND_STATS").is_some() && fixed_layers.is_some() {
            // THE WANTED-PIXEL CERTIFICATION (measurement): the exact stop matters to the output only where a
            // wanted pixel has more item layers than the game rendered; M_w = the most item layers any wanted
            // pixel has (before the cut); a census lower bound L_lb ≥ M_w certifies the frame without the
            // full-frame count
            let m_w = parts.iter().flat_map(|(counts, _)| counts.iter()).map(|c| (*c as usize).saturating_sub(skip_n)).max().unwrap_or(0);
            let (lb4, lb8) = (CENSUS_LB[0].load(std::sync::atomic::Ordering::Relaxed), CENSUS_LB[1].load(std::sync::atomic::Ordering::Relaxed));
            CERT_STATS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if m_w <= lb4 { CERT_STATS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
            if m_w <= lb8 { CERT_STATS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
            if m_w <= kept { CERT_STATS[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
            eprintln!("cert stats: frame {w}×{h}: exact rendered {kept}, census lower bounds {lb4} (1/16) / {lb8} (1/64), most item layers at a wanted pixel {m_w} → certified {}/{}", m_w <= lb4, m_w <= lb8);
        }
        if peel_layers_debug() && fixed_layers.is_none() {
            eprintln!("peel layers (sparse census, {n_census} census pixels): {} candidate item layers, fractions {:?} → {} rendered (the stop rule)", candidates, fractions.iter().take(candidates).map(|f| format!("{f:.6}")).collect::<Vec<_>>(), kept);
        }
        // THE CSR over the wanted pixels, assembled in parallel: per part the kept count of every pixel
        // (cut to skip_n + kept) and its total, a prefix over the parts, then every part copies its kept
        // fragments into its slice
        let t_csr = std::time::Instant::now();
        let cap = skip_n + kept;
        if LAYER_DEBUG_SET.is_some() { eprintln!("LAYERDBG frame={}x{} dir=({:.6},{:.6},{:.6}) rendered_items={kept} cap={cap} candidates={candidates} fractions={:?}", frame.res, frame.res_y, frame.d[0], frame.d[1], frame.d[2], fractions.iter().take(candidates.max(kept) + 1).map(|f| format!("{f:.5}")).collect::<Vec<_>>()); }
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
        prof::add(&prof::L_CSR, t_csr);
        // the parts (8 per thread: 2 000 vectors the workers allocated) are read no more — freed on the pool, not by the
        // caller alone (perf 6: 1.4 ms per tiny direction of serial time after the layer derivation)
        parallel_drop(parts);
        return Layers { w, h, start, cnt: None, frags, max_layers: MAX_LAYERS, sparse: Some(px.clone()), item_layers: kept, fractions };
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
                    // (the sink bounded by the rows' fragments plus a dome layer per pixel)
                    let bound: usize = (y0..y1).map(|y| (0..w as usize).map(|x| ab.at_all(x as u32, y as u32).len() + 1).sum::<usize>()).sum();
                    let mut out_v: Vec<LayerFrag> = Vec::with_capacity(bound);
                    let mut out = unsafe { LayerSink::new(out_v.as_mut_ptr(), bound) };
                    let mut cache = [(u32::MAX, 0.0f32); BIAS_CACHE];
                    let mut shade = [(u32::MAX, TriShade::NONE); BIAS_CACHE];
                    for y in y0..y1 {
                        for x in 0..w as usize {
                            let before = out.len();
                            derive_pixel(x, y, ab.at_all(x as u32, y as u32), &mut out, &mut cache, &mut shade, skip_n + derive_cap);
                            counts.push((out.len() - before) as u32);
                        }
                    }
                    // SAFETY: the sink wrote out.len() ≤ bound slots of the vector's capacity
                    unsafe { out_v.set_len(out.len()); }
                    (counts, out_v)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    // THE LAYER COUNT (engineer D, 0x140234df0): the written fraction of every candidate item layer over
    // the whole viewport, the game's stop rule (or the captured count) → every pixel's list is cut
    let part_hists: Vec<[usize; MAX_LAYERS + 1]> = crate::pool::pool().map(parts.len(), |pi| {
        let mut h = [0usize; MAX_LAYERS + 1];
        for c in &parts[pi].0 { h[(*c as usize).saturating_sub(skip_n).min(MAX_LAYERS)] += 1; }
        h
    });
    let mut hist = vec![0usize; MAX_LAYERS + 1];
    for h in &part_hists { for k in 0..=MAX_LAYERS { hist[k] += h[k]; } }
    let mut fractions: Vec<f64> = Vec::with_capacity(MAX_LAYERS);
    let mut at_least = n;
    for k in 0..MAX_LAYERS {
        at_least -= hist[k];
        fractions.push(at_least as f64 / n.max(1) as f64);
    }
    let kept = match fixed_layers {
        Some(k) => k.min(derive_cap),
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
    // THE DENSE CSR, assembled in parallel (the product path — the transcribed accumulate reads every pixel —
    // came through here with a serial per-pixel copy: ~1 s per giant direction, ≈ 1000 s of a sweep): per
    // part the kept total, a prefix over the parts, then every part writes its rows' starts and copies its
    // kept lists into its slice
    let cap = skip_n + kept;
    let part_totals: Vec<usize> = crate::pool::pool().map(parts.len(), |pi| parts[pi].0.iter().map(|c| (*c as usize).min(cap)).sum());
    let mut part_base = Vec::with_capacity(parts.len() + 1);
    part_base.push(0usize);
    for t in &part_totals { let last = *part_base.last().unwrap(); part_base.push(last + t); }
    let total = *part_base.last().unwrap();
    let mut start: Vec<u32> = U32S.take_with_capacity(n + 1);
    let mut frags: Vec<LayerFrag> = LAYER_FRAGS.take_with_capacity(total);
    // SAFETY: every slot of both is written below (each part its own rows / its own slice) before any read
    unsafe { start.set_len(n + 1); frags.set_len(total); }
    start[n] = total as u32;
    {
        let (sp, fp) = (start.as_mut_ptr() as usize, frags.as_mut_ptr() as usize);
        let parts = &parts;
        let part_base = &part_base;
        // part pi covers pixels [pix_base[pi], pix_base[pi] + counts.len())
        let mut pix_base = Vec::with_capacity(parts.len() + 1);
        pix_base.push(0usize);
        for (counts, _) in parts.iter() { let last = *pix_base.last().unwrap(); pix_base.push(last + counts.len()); }
        let pix_base = &pix_base;
        crate::pool::pool().run(parts.len(), |pi| {
            let (counts, out) = &parts[pi];
            let mut o = 0usize;
            let mut acc = part_base[pi];
            let p0 = pix_base[pi];
            for (i, c) in counts.iter().enumerate() {
                let c = *c as usize;
                let keep = c.min(cap);
                // SAFETY: pixel p0 + i and the slice [acc, acc + keep) belong to this part alone
                unsafe {
                    *(sp as *mut u32).add(p0 + i) = acc as u32;
                    std::ptr::copy_nonoverlapping(out.as_ptr().add(o), (fp as *mut LayerFrag).add(acc), keep);
                }
                o += c;
                acc += keep;
            }
        });
    }
    Layers { w, h, start, cnt: None, frags, max_layers: MAX_LAYERS, sparse: None, item_layers: kept, fractions }
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
        if *BIASED_ORDER {
            let mut v: Vec<WalkFrag> = Vec::with_capacity(list.len());
            for (i, f) in list.iter().enumerate() {
                if prm.dome_layer && is_env(f.tri) { continue; }
                let z01 = frame.z01(f.z).max(0.0);
                let wt = &bvh.tris[f.tri as usize];
                let (slope, zmax_prim) = tri_slope(wt, frame);
                v.push(WalkFrag::from_depths(z01, d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), prm.depth_bias, prm.depth_bits), prm.depth_bits, draw_rank_of(f.tri), i as u32));
            }
            return layer_walk_biased_count(&mut v, d_prev, item_cap(prm.dome_layer), prm.depth_bits);
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
        if *BIASED_ORDER {
            let mut v: Vec<WalkFrag> = Vec::with_capacity(list.len());
            for (i, (_, f)) in list.iter().enumerate() {
                if prm.dome_layer && is_env(f.tri) { continue; }
                let z01 = frame.z01(f.z).max(0.0);
                let wt = &bvh.tris[f.tri as usize];
                let (slope, zmax_prim) = tri_slope(wt, frame);
                v.push(WalkFrag::from_depths(z01, d3d_depth_bias_fmt(zmax_prim, slope.min(1e6), prm.depth_bias, prm.depth_bits), prm.depth_bits, draw_rank_of(f.tri), i as u32));
            }
            return layer_walk_biased_count(&mut v, d_prev, item_cap(prm.dome_layer), prm.depth_bits);
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
                                Some((tx, fp)) if !*ALPHA_POINT => if alpha_point_mip() { tx.passes_point(u, v, fp, ALPHA_THRESHOLD) } else { tx.passes_planned(u, v, fp, ALPHA_THRESHOLD, crate::alphatex::Address::ClampEdge) },
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

/// A fragment of the exact layer count: depth, the tie key (under the biased order the triangle's DRAW RANK —
/// `DRAW_RANK` — pushed once per triangle; under the former rule the world triangle index), the triangle's
/// depth-bias term.
/// (`repr(C)`: scan_block16 gathers the fields at byte offsets 0 / 4 / 8 of the 12-byte record — the layout is
/// part of the contract, not left to the compiler's field reordering.)
#[derive(Clone, Copy, Debug)]
#[repr(C)]
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

/// LMTOOL_BOUND_STATS: item fragments (alpha-passing) [behind the environment, inside a bias window, past the layer cap, accepted].
pub static DROP_STATS: [std::sync::atomic::AtomicU64; 4] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];
/// LMTOOL_BOUND_STATS: the last frame's census lower bounds on the rendered item layers (every 4th / 8th pixel in x and y).
pub static CENSUS_LB: [std::sync::atomic::AtomicUsize; 2] = [std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0)];
/// LMTOOL_BOUND_STATS: wanted-pixel certification [frames, certified by census/4 (1/16 of the pixels), by census/8 (1/64), M_w ≤ exact (sanity)].
pub static CERT_STATS: [std::sync::atomic::AtomicU64; 4] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];
/// LMTOOL_BOUND_STATS: [frames, decided-and-right (census/4), decided-and-right (census/8), decided-WRONG].
/// The gather's sky lookups (the dome colour per sub-sample, `dome_px`) and its surface hits, summed per sweep (perf 8).
pub static GATHER_COUNTS: [std::sync::atomic::AtomicU64; 2] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];
pub static BOUND_TOTALS: [std::sync::atomic::AtomicU64; 4] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];

/// A 64-byte aligned wrapper for a 16-lane stack array (one cache line for the zmm loads and stores).
#[repr(C, align(64))]
pub struct Align64<T>(pub T);

/// The sixteen lanes' z and the in-range mask of a block (see the visit body): `z[l] = (z0·b0 + z1·b1) + z2·b2`
/// exactly as the scalar body computes it, `live` = cov ∧ (z < zmax) ∧ (z ≥ zmin) — the same f32 operations per
/// lane, in the same order (no fused multiply-add: the scalar form has none).
/// (64-byte aligned: the 512-bit store of z and the body's reloads never split a cache line — see raster::Bary16.)
#[repr(C, align(64))]
pub struct LaneZ {
    pub z: [f32; 16],
    pub live: u16,
}

#[inline(always)]
pub fn lanes_z_range(bary: &[[f32; 16]; 3], zv: [f32; 3], zmin: f32, zmax: f32, cov: u16) -> LaneZ {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    unsafe {
        use std::arch::x86_64::*;
        let b0 = _mm512_loadu_ps(bary[0].as_ptr());
        let b1 = _mm512_loadu_ps(bary[1].as_ptr());
        let b2 = _mm512_loadu_ps(bary[2].as_ptr());
        let t0 = _mm512_mul_ps(_mm512_set1_ps(zv[0]), b0);
        let t1 = _mm512_mul_ps(_mm512_set1_ps(zv[1]), b1);
        let t2 = _mm512_mul_ps(_mm512_set1_ps(zv[2]), b2);
        let z = _mm512_add_ps(_mm512_add_ps(t0, t1), t2);
        let lt = _mm512_cmp_ps_mask::<_CMP_LT_OQ>(z, _mm512_set1_ps(zmax));
        let ge = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(z, _mm512_set1_ps(zmin));
        let mut out = LaneZ { z: [0.0; 16], live: 0 };
        _mm512_storeu_ps(out.z.as_mut_ptr(), z);
        out.live = cov & lt & ge;
        out
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
    {
        let mut out = LaneZ { z: [0.0; 16], live: 0 };
        for l in 0..16 {
            let z = zv[0] * bary[0][l] + zv[1] * bary[1][l] + zv[2] * bary[2][l];
            out.z[l] = z;
            if (cov >> l) & 1 == 1 && z < zmax && z >= zmin { out.live |= 1 << l; }
        }
        out
    }
}

/// The wanted bits of pixels id0..id0+16 from the wanted bitmap (bit l = pixel id0 + l; pixels past the bitmap
/// read as not wanted).
#[inline(always)]
pub fn wanted_bits16(bitmap: &[u64], id0: usize) -> u16 {
    let w = id0 >> 6;
    let s = id0 & 63;
    let lo = bitmap.get(w).copied().unwrap_or(0) >> s;
    let v = if s + 16 > 64 { lo | (bitmap.get(w + 1).copied().unwrap_or(0) << (64 - s)) } else { lo };
    v as u16
}

/// The environment layer's depth maximum over sixteen lanes: `z01 = 0.5 + (zc + z) / (2·half_d)` per lane (the
/// scalar `PeelFrame::z01`), kept where `0 ≤ z01 ≤ 1` and `z01 > e` (the strict maximum the scalar body takes);
/// `env` holds the lanes' current maxima (its length may fall short of 16 at a band's end: those lanes are off).
#[inline(always)]
pub fn env_max_update16(env: &mut [f32], z: &[f32; 16], live: u16, frame: &PeelFrame) {
    let n = env.len().min(16);
    let lane_mask: u16 = if n >= 16 { 0xffff } else { ((1u32 << n) - 1) as u16 };
    let live = live & lane_mask;
    if live == 0 { return; }
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    unsafe {
        use std::arch::x86_64::*;
        let zv = _mm512_loadu_ps(z.as_ptr());
        let zc = _mm512_set1_ps(frame.zc);
        let den = _mm512_set1_ps(2.0 * frame.half_d);
        let z01 = _mm512_add_ps(_mm512_set1_ps(0.5), _mm512_div_ps(_mm512_add_ps(zc, zv), den));
        let ge0 = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(z01, _mm512_setzero_ps());
        let le1 = _mm512_cmp_ps_mask::<_CMP_LE_OQ>(z01, _mm512_set1_ps(1.0));
        let e = _mm512_maskz_loadu_ps(live, env.as_ptr());
        let gt = _mm512_mask_cmp_ps_mask::<_CMP_GT_OQ>(live & ge0 & le1, z01, e);
        _mm512_mask_storeu_ps(env.as_mut_ptr(), gt, z01);
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f")))]
    {
        let mut m = live;
        while m != 0 {
            let l = m.trailing_zeros() as usize;
            m &= m - 1;
            let z01 = frame.z01(z[l]);
            if z01 >= 0.0 && z01 <= 1.0 {
                let e = &mut env[l];
                if z01 > *e { *e = z01; }
            }
        }
    }
}

/// The count's per-pixel fragment counters of a block bumped by one for the live lanes (u16, saturating; a lane
/// already at the ceiling sets `saturated`); `cnt` starts at pixel li0 and may fall short of 16 at the band's end.
#[inline(always)]
pub fn count_add16(cnt: &mut [u16], live: u16, saturated: &mut bool) {
    let n = cnt.len().min(16);
    let lane_mask: u16 = if n >= 16 { 0xffff } else { ((1u32 << n) - 1) as u16 };
    let live = live & lane_mask;
    if live == 0 { return; }
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl"))]
    unsafe {
        use std::arch::x86_64::*;
        let v = _mm256_maskz_loadu_epi16(live, cnt.as_ptr() as *const i16);
        let full = _mm256_mask_cmpeq_epu16_mask(live, v, _mm256_set1_epi16(-1));
        if full != 0 { *saturated = true; }
        let bumped = _mm256_mask_adds_epu16(v, live, v, _mm256_set1_epi16(1));
        _mm256_mask_storeu_epi16(cnt.as_mut_ptr() as *mut i16, live, bumped);
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw", target_feature = "avx512vl")))]
    {
        let mut m = live;
        while m != 0 {
            let l = m.trailing_zeros() as usize;
            m &= m - 1;
            let c = &mut cnt[l];
            if *c == u16::MAX { *saturated = true; } else { *c += 1; }
        }
    }
}

/// A hint to fetch `p`'s cache line (no effect on any value).
#[inline(always)]
pub fn prefetch<T>(p: &T) {
    #[cfg(target_arch = "x86_64")]
    unsafe { std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(p as *const T as *const i8); }
    #[cfg(not(target_arch = "x86_64"))]
    { let _ = p; }
}

/// THE SCAN SIXTEEN PIXELS AT A TIME — the layer walk of the pixels holding one or two fragments (nearly all of
/// them), in lanes, with the scalar walk's operations per lane: the environment depth stored as D16
/// (`(env_d·65535).round()/65535`), each fragment's `z01 = (0.5 + (zc + z)/(2·half_d)).max(0)`, the (z, tri)
/// order of two fragments by the total order on their bits, the accept test `z01 ≥ d_prev`, the accepted
/// depth `((z01 + bias).clamp(0, 1)·65535).round()/65535` — the round-half-away built from truncate, the
/// division kept as a division. Lanes with three or more fragments are reported for the scalar walk. Returns
/// (pixels with fragments, pixels with one layer, pixels with two layers, the lanes for the scalar walk).
/// Only for a frame with the environment layer and a 16-bit depth store — the scalar walk's initial d_prev
/// and quantisation otherwise differ.
#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
#[inline(always)]
pub fn scan_block16(offs: &[u32], csr: &[CFrag], env_d: &[f32; 16], frame: &PeelFrame) -> ([u32; SCAN_K + 1], u32, u16) {
    use std::arch::x86_64::*;
    unsafe {
        let o0 = _mm512_loadu_si512(offs.as_ptr() as *const _);
        let o1 = _mm512_loadu_si512(offs.as_ptr().add(1) as *const _);
        let n = _mm512_sub_epi32(o1, o0);
        // the records of lane l start at csr[o0[l]] (12-byte records: ×3 in u32 units); slot j = three gathers
        // (a missing record — j ≥ n — is masked by the caller's `valid`; here its address is still in bounds
        // only if masked, so the loader takes the mask)
        let base = csr.as_ptr() as *const f32;
        let idx0 = _mm512_mullo_epi32(o0, _mm512_set1_epi32(3));
        let zero = _mm512_setzero_ps();
        let maxk = _mm512_set1_epi32(i32::MAX);
        let three = _mm512_set1_epi32(3);
        scan_block16_core_masked(n, |j, valid| {
            let idx = _mm512_add_epi32(idx0, _mm512_mullo_epi32(three, _mm512_set1_epi32(j as i32)));
            (_mm512_mask_i32gather_ps::<4>(zero, valid, idx, base), _mm512_mask_i32gather_epi32::<4>(maxk, valid, _mm512_add_epi32(idx, _mm512_set1_epi32(1)), base as *const i32), _mm512_mask_i32gather_ps::<4>(zero, valid, _mm512_add_epi32(idx, _mm512_set1_epi32(2)), base))
        }, env_d, frame)
    }
}

/// `scan_block16_core` with an unmasked slot loader (the slot form: every slot row exists; the lanes past their
/// count hold stale values that the `valid` mask blends away).
#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
#[inline(always)]
unsafe fn scan_block16_core<L: Fn(usize) -> (std::arch::x86_64::__m512, std::arch::x86_64::__m512i, std::arch::x86_64::__m512)>(n: std::arch::x86_64::__m512i, load: L, env_d: &[f32; 16], frame: &PeelFrame) -> ([u32; SCAN_K + 1], u32, u16) {
    scan_block16_core_masked(n, |j, _valid| load(j), env_d, frame)
}

/// `scan_block16` over the SLOT form: the counts from the u16 table, lane l's records at slots[l·SLOTS_PER_PX..]
/// (`slots` starts at the block's first pixel); lanes with more records than slots go to the scalar walk.
#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
#[inline(always)]
pub fn scan_block16_slots(cnt16: &[u16], rows: &[SlotBlock], env_d: &[f32; 16], frame: &PeelFrame) -> ([u32; SCAN_K + 1], u32, u16) {
    use std::arch::x86_64::*;
    unsafe {
        debug_assert!(cnt16.len() >= 16 && rows.len() >= SLOT_ROWS_PER_BLOCK);
        let n = _mm512_cvtepu16_epi32(_mm256_loadu_si256(cnt16.as_ptr() as *const _));
        // (a lane with more records than slots must not read past its slots: SCAN_K ≤ SLOTS_PER_PX)
        const _: () = assert!(SCAN_K <= SLOTS_PER_PX);
        scan_block16_core(n, |j| {
            let r = &rows[j * 3..j * 3 + 3];
            (_mm512_load_ps(r[0].0.as_ptr() as *const f32), _mm512_load_si512(r[1].0.as_ptr() as *const _), _mm512_load_ps(r[2].0.as_ptr() as *const f32))
        }, env_d, frame)
    }
}

/// The lane walk proper: `n` records per lane, lane l's first record at `base[idx0[l]]` (u32 units, 12-byte
/// records: z at +0, the tie key at +1, the bias at +2).
#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
#[inline(always)]
unsafe fn scan_block16_core_masked<L: Fn(usize, u16) -> (std::arch::x86_64::__m512, std::arch::x86_64::__m512i, std::arch::x86_64::__m512)>(n: std::arch::x86_64::__m512i, load: L, env_d: &[f32; 16], frame: &PeelFrame) -> ([u32; SCAN_K + 1], u32, u16) {
    use std::arch::x86_64::*;
    unsafe {
        let zero_i = _mm512_setzero_si512();
        let has = _mm512_cmpgt_epi32_mask(n, zero_i);
        if has == 0 { return ([0; SCAN_K + 1], 0, 0); }
        // the lanes the network handles: one to four fragments; the rest go to the scalar walk (an nmax-adaptive
        // 8-element network for the canopy blocks measured neutral twice: the extra gathers eat the sort's saving)
        let k: usize = SCAN_K;
        let small = _mm512_cmple_epi32_mask(n, _mm512_set1_epi32(k as i32));
        let lanes = has & small;
        let big = has & !small;
        let ed = _mm512_loadu_ps(env_d.as_ptr());
        let zero = _mm512_setzero_ps();
        let k65535 = _mm512_set1_ps(65535.0);
        // round half away from zero of a value in [0, 65535]: truncate, then one more where the remainder ≥ ½
        let round_away = |x: __m512| -> __m512 {
            let t = _mm512_roundscale_ps::<{ _MM_FROUND_TO_ZERO | _MM_FROUND_NO_EXC }>(x);
            let r = _mm512_sub_ps(x, t);
            let up = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(r, _mm512_set1_ps(0.5));
            _mm512_mask_add_ps(t, up, t, _mm512_set1_ps(1.0))
        };
        let q16 = |x: __m512| -> __m512 { _mm512_div_ps(round_away(_mm512_mul_ps(x, k65535)), k65535) };
        let env_pos = _mm512_cmp_ps_mask::<_CMP_GT_OQ>(ed, zero);
        let env_q = _mm512_mask_blend_ps(env_pos, zero, q16(ed));
        // the fragments j = 0..K of every lane through the loader (z, key, bias vectors of slot j); a missing
        // fragment (j ≥ n) sorts last with the largest key
        let maxk = _mm512_set1_epi32(i32::MAX);
        let one_i = _mm512_set1_epi32(1);
        // the (z, tri) order by `total_cmp`: the float's bits as a signed key (negative floats reversed)
        let tkey = |z: __m512| -> __m512i {
            let bits = _mm512_castps_si512(z);
            let sign = _mm512_srai_epi32::<31>(bits);
            _mm512_xor_si512(bits, _mm512_srli_epi32::<1>(sign))
        };
        let mut key = [zero_i; SCAN_K];
        let mut tri = [zero_i; SCAN_K];
        let mut zf = [zero; SCAN_K];
        let mut bf = [zero; SCAN_K];
        // the walk's per-lane quantities
        let zc = _mm512_set1_ps(frame.zc);
        let den = _mm512_set1_ps(2.0 * frame.half_d);
        let half = _mm512_set1_ps(0.5);
        let one = _mm512_set1_ps(1.0);
        let z01 = |z: __m512| -> __m512 { _mm512_max_ps(_mm512_add_ps(half, _mm512_div_ps(_mm512_add_ps(zc, z), den)), zero) };
        let biased = *BIASED_ORDER;
        for j in 0..k {
            let valid = lanes & _mm512_cmpgt_epi32_mask(n, _mm512_set1_epi32(j as i32));
            let (z, t, b) = load(j, valid);
            // (the invalid lanes' key is forced to the maximum below; their z / bias values are never read after
            // the blend, but keep them finite for the arithmetic)
            let z = _mm512_mask_blend_ps(valid, zero, z);
            tri[j] = _mm512_mask_blend_epi32(valid, maxk, t);
            let b = _mm512_mask_blend_ps(valid, zero, b);
            if biased {
                // THE BIASED ORDER: zf carries the unbiased depth z01 (the compare), bf the stored depth (the key
                // and the next d_prev): q16((z01 + bias).clamp(0, 1)) — non-negative, so its bits order as ints;
                // the tie key is the triangle's DRAW RANK (E's (class, instance, model triangle))
                let zj = z01(z);
                let dd = q16(_mm512_min_ps(_mm512_max_ps(_mm512_add_ps(zj, b), zero), one));
                zf[j] = zj;
                bf[j] = dd;
                key[j] = _mm512_mask_blend_epi32(valid, maxk, _mm512_castps_si512(dd));
            } else {
                zf[j] = z;
                bf[j] = b;
                key[j] = _mm512_mask_blend_epi32(valid, maxk, tkey(z));
            }
        }
        // the sorting network (ascending (key, tri)): compare-exchange pairs
        let mut cex = |i: usize, j: usize, key: &mut [__m512i; SCAN_K], tri: &mut [__m512i; SCAN_K], zf: &mut [__m512; SCAN_K], bf: &mut [__m512; SCAN_K]| {
            // (dd, tri, z01) ascending — the third key only matters where two records share the stored depth
            // and the triangle, which the raster never produces; it keeps the order total for the tests
            let eq_key = _mm512_cmpeq_epi32_mask(key[i], key[j]);
            let z_gt = if biased { eq_key & _mm512_cmpeq_epi32_mask(tri[i], tri[j]) & _mm512_cmpgt_epi32_mask(_mm512_castps_si512(zf[i]), _mm512_castps_si512(zf[j])) } else { 0 };
            let gt = _mm512_cmpgt_epi32_mask(key[i], key[j]) | (eq_key & _mm512_cmpgt_epi32_mask(tri[i], tri[j])) | z_gt;
            let (ki, kj) = (_mm512_mask_blend_epi32(gt, key[i], key[j]), _mm512_mask_blend_epi32(gt, key[j], key[i]));
            let (ti, tj) = (_mm512_mask_blend_epi32(gt, tri[i], tri[j]), _mm512_mask_blend_epi32(gt, tri[j], tri[i]));
            let (zi, zj) = (_mm512_mask_blend_ps(gt, zf[i], zf[j]), _mm512_mask_blend_ps(gt, zf[j], zf[i]));
            let (bi, bj) = (_mm512_mask_blend_ps(gt, bf[i], bf[j]), _mm512_mask_blend_ps(gt, bf[j], bf[i]));
            key[i] = ki; key[j] = kj; tri[i] = ti; tri[j] = tj; zf[i] = zi; zf[j] = zj; bf[i] = bi; bf[j] = bj;
        };
        // 4 elements: (0,1) (2,3) (0,2) (1,3) (1,2)
        cex(0, 1, &mut key, &mut tri, &mut zf, &mut bf);
        cex(2, 3, &mut key, &mut tri, &mut zf, &mut bf);
        cex(0, 2, &mut key, &mut tri, &mut zf, &mut bf);
        cex(1, 3, &mut key, &mut tri, &mut zf, &mut bf);
        cex(1, 2, &mut key, &mut tri, &mut zf, &mut bf);
        // the walk (biased: zf = z01 and bf = the stored depth already; unbiased: computed here as before)
        let mut d_prev = env_q;
        let mut items = zero_i;
        for j in 0..k {
            let valid = lanes & _mm512_cmpgt_epi32_mask(n, _mm512_set1_epi32(j as i32));
            let (zj, dd) = if biased { (zf[j], bf[j]) } else { let zj = z01(zf[j]); (zj, q16(_mm512_min_ps(_mm512_max_ps(_mm512_add_ps(zj, bf[j]), zero), one))) };
            let acc = valid & _mm512_cmp_ps_mask::<_CMP_GE_OQ>(zj, d_prev);
            d_prev = _mm512_mask_blend_ps(acc, d_prev, dd);
            items = _mm512_mask_add_epi32(items, acc, items, one_i);
        }
        let mut hist = [0u32; SCAN_K + 1];
        for c in 0..=k {
            hist[c] = (lanes & _mm512_cmpeq_epi32_mask(items, _mm512_set1_epi32(c as i32))).count_ones();
        }
        (hist, has.count_ones(), big)
    }
}
/// MEASUREMENT ONLY (LMTOOL_MEASURE_NOWALK=1): the scalar count walk skipped (every such pixel counted as one layer —
/// wrong output) to time it. (Read once: an env read per pixel serialises the threads on the environment lock.)
pub static MEASURE_NOWALK: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("LMTOOL_MEASURE_NOWALK").is_some());

/// The most fragments per pixel the lane walk takes (a 4-element sorting network).
pub const SCAN_K: usize = 4;

/// `f32::total_cmp`'s order as an UNSIGNED 32-bit key: the float's bits with the sign folded (a negative float's
/// magnitude bits reversed) and the sign bit flipped, so that `a.total_cmp(&b) == key(a).cmp(&key(b))`.
#[inline(always)]
pub fn total_order_key(z: f32) -> u32 {
    let bits = z.to_bits() as i32;
    let folded = bits ^ (((bits >> 31) as u32) >> 1) as i32;
    (folded as u32) ^ 0x8000_0000
}


/// THE LAYER WALK over a pixel's fragments already in (z, tri) order (`extract_layers`' depth rules, no colour):
/// `d_prev` starts at the environment layer's D16-stored depth (dome layer on; 0 when none), a fragment is a
/// layer when its `z01 = frame.z01(z).max(0)` is not in front of `d_prev`, and the accepted depth
/// `z01 + bias` — quantised to D16 when the depth store is 16-bit — becomes the next `d_prev`; at most
/// MAX_LAYERS. The scalar reference every lane and keyed variant is held to.
/// THE LAYER ORDER (engineer 5's split of the pwc-day residue, 2026-09-25): the GPU forms layer j+1 from the
/// fragment with the smallest BIASED, stored depth among those whose UNBIASED depth passes the previous layer's
/// stored depth — the peel compare in the shader reads the interpolated depth, the depth test that picks the
/// nearest fragment of the pass runs on the rasteriser's biased depth. The port ordered by the unbiased (z, tri),
/// which swaps a steep card (slope bias ~100 quanta) and a flatter trunk within a few quanta. THE DEFAULT since
/// 2026-09-26 (E's rule stated as a function; engineer 4's differential test: the count and the stored-depth
/// sequence equal the iterated argmin on 60 000 tie-heavy pixels; the tie key at an equal stored depth is the
/// DRAW ORDER — class, instance, model triangle — as `DRAW_RANK`). LMTOOL_LAYER_ORDER=unbiased restores the
/// former rule (the references cut before the flip were made with it). Verified on engineer 5's worked example
/// (fitted pixel (3263, 2742) of pwc-day direction 0): layer 1 = the trunk (q 34891, the game's), the cards
/// (q 34937) merged away, as the game does.
pub static BIASED_ORDER: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_LAYER_ORDER").map(|v| v != "unbiased").unwrap_or(true));

/// THE DRAW ORDER of the world triangles (E's tie key at an equal stored depth: LESS keeps the first writer):
/// class (0 = the opaque item draw, 1 = the alpha-tested card draw), then the scene instance, then the
/// triangle's index in the model — as ONE u32 rank per world (BVH) triangle, computed once per bake.
pub static DRAW_RANK: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();

pub fn draw_rank_table(bvh: &Bvh) -> Vec<u32> {
    let mut order: Vec<u32> = (0..bvh.tris.len() as u32).collect();
    let key = |t: u32| -> (u8, u32, u32) { let w = &bvh.tris[t as usize]; ((w.alpha != u16::MAX) as u8, w.inst, w.tri) };
    order.sort_unstable_by_key(|t| key(*t));
    let mut rank = vec![0u32; bvh.tris.len()];
    for (r, t) in order.iter().enumerate() { rank[*t as usize] = r as u32; }
    rank
}

#[inline(always)]
fn draw_rank_of(tri: u32) -> u32 {
    match DRAW_RANK.get() { Some(r) => r[tri as usize], None => tri }
}

thread_local! {
    /// The derive's per-pixel walk buffers (the biased order's fragments and the accepted indices), reused
    /// across the pixels of a thread.
    static DERIVE_SCRATCH: std::cell::Cell<(Vec<WalkFrag>, Vec<u32>)> = const { std::cell::Cell::new((Vec::new(), Vec::new())) };
}

/// One fragment of a pixel for the biased walk: the stored (biased, quantised) depth, the unbiased depth, the
/// tie key (the draw rank), and the caller's index into its own list.
#[derive(Clone, Copy, Debug)]
pub struct WalkFrag {
    /// The sort key: the stored depth's bits (non-negative f32 — its bits order as its value) above the tie key
    /// (the triangle's draw rank, `DRAW_RANK`) — one u64 compare per step of the sort.
    pub key: u64,
    pub z01: f32,
    pub idx: u32,
}

impl WalkFrag {
    /// From the unbiased depth and the bias: the upper key is the D16 QUANTUM (no division — the division to
    /// the stored value happens once per ACCEPTED fragment in the walk) when the store is 16-bit, else the
    /// bits of the stored f32 (non-negative: they order as the value).
    #[inline(always)]
    pub fn from_depths(z01: f32, bias: f32, depth_bits: u32, tie: u32, idx: u32) -> WalkFrag {
        let dd = z01 + bias;
        // (round half away from zero on a non-negative value = trunc + (frac ≥ 0.5): the lane walk's q16,
        // proven equal to `.round()` by the lane-vs-scalar tests; cheaper than the library round)
        let hi: u32 = if depth_bits == 16 {
            let v = dd.clamp(0.0, 1.0) * 65535.0;
            let t = v.trunc();
            (t as u32) + ((v - t) >= 0.5) as u32
        } else {
            dd.to_bits()
        };
        WalkFrag { key: (hi as u64) << 32 | tie as u64, z01, idx }
    }
    /// The stored depth back from the key (`stored_depth`'s value: q / 65535 for the 16-bit store).
    #[inline(always)]
    pub fn dd(&self, depth_bits: u32) -> f32 {
        let hi = (self.key >> 32) as u32;
        if depth_bits == 16 { hi as f32 / 65535.0 } else { f32::from_bits(hi) }
    }
}

/// The stored depth of a fragment: `(z01 + bias)`, clamped and quantised to D16 when the store is 16-bit.
#[inline(always)]
pub fn stored_depth(z01: f32, bias: f32, depth_bits: u32) -> f32 {
    let dd = z01 + bias;
    if depth_bits == 16 { (dd.clamp(0.0, 1.0) * 65535.0).round() / 65535.0 } else { dd }
}

/// The environment layer's stored depth (the walk's initial d_prev): 0 when no environment surface, else the
/// maximum quantised as the store does; −∞ without the dome layer.
#[inline(always)]
pub fn env_start(env_d: f32, dome_layer: bool, depth_bits: u32) -> f32 {
    if !dome_layer { return f32::NEG_INFINITY; }
    if env_d > 0.0 { if depth_bits == 16 { (env_d * 65535.0).round() / 65535.0 } else { env_d } } else { 0.0 }
}

/// THE BIASED WALK, the reference form: `frags` sorted here by (stored depth, triangle), then walked — accept
/// when the unbiased depth is not below the previous stored depth, the accepted fragment's stored depth becomes
/// the previous. Returns the item count; `frags` is left in the walk order with `accept(i)` telling the caller
/// which were accepted (the derive uses it for the layers).
pub fn layer_walk_biased(frags: &mut [WalkFrag], env_d: f32, dome_layer: bool, depth_bits: u32, accepted: &mut Vec<u32>) -> usize {
    layer_walk_biased_capped(frags, env_start(env_d, dome_layer, depth_bits), item_cap(dome_layer), depth_bits, accepted)
}

/// `layer_walk_biased` from a given starting stored depth (the derive's environment layer already resolved).
pub fn layer_walk_biased_from(frags: &mut [WalkFrag], d_start: f32, depth_bits: u32, accepted: &mut Vec<u32>) -> usize {
    layer_walk_biased_capped(frags, d_start, MAX_LAYERS, depth_bits, accepted)
}

/// The item cap of a peel: the render counter's 21 minus the environment render (sweep 0: 20 item layers;
/// a sweep without the environment block: 21) — E's function stops at |layers| = the cap.
#[inline(always)]
pub fn item_cap(dome_layer: bool) -> usize {
    MAX_LAYERS - dome_layer as usize
}

/// The count alone (no accepted list — the scan's and the census' form: no allocation).
pub fn layer_walk_biased_count(frags: &mut [WalkFrag], d_start: f32, cap: usize, depth_bits: u32) -> usize {
    sort_walk_frags(frags);
    let mut d_prev = d_start;
    let mut items = 0usize;
    for f in frags.iter() {
        if f.z01 < d_prev {
            continue;
        }
        if items >= cap {
            break;
        }
        items += 1;
        d_prev = f.dd(depth_bits);
    }
    items
}

/// The walk's order: (stored depth, tie key, unbiased depth) ascending — insertion for the short lists.
#[inline(always)]
fn sort_walk_frags(frags: &mut [WalkFrag]) {
    // (the key is total over a pixel's real records — a triangle visits a pixel once, so the ranks differ;
    // the z01 tie-break only orders synthetic duplicates in the tests)
    #[inline(always)]
    fn gt(a: &WalkFrag, b: &WalkFrag) -> bool { a.key > b.key || (a.key == b.key && a.z01.to_bits() > b.z01.to_bits()) }
    let n = frags.len();
    if n <= 64 {
        for i in 1..n {
            let cur = frags[i];
            let mut j = i;
            while j > 0 && gt(&frags[j - 1], &cur) {
                frags[j] = frags[j - 1];
                j -= 1;
            }
            frags[j] = cur;
        }
    } else {
        frags.sort_unstable_by(|p, q| p.key.cmp(&q.key).then_with(|| p.z01.to_bits().cmp(&q.z01.to_bits())));
    }
}

/// `layer_walk_biased_from` with an explicit item cap.
pub fn layer_walk_biased_capped(frags: &mut [WalkFrag], d_start: f32, cap: usize, depth_bits: u32, accepted: &mut Vec<u32>) -> usize {
    sort_walk_frags(frags);
    let mut d_prev = d_start;
    let mut items = 0usize;
    for f in frags.iter() {
        if f.z01 < d_prev {
            continue;
        }
        if items >= cap {
            break;
        }
        items += 1;
        d_prev = f.dd(depth_bits);
        accepted.push(f.idx);
    }
    items
}

/// `layer_walk_biased` over a pixel's count records (the scan's scalar walk).
pub fn layer_walk_biased_cfrags(buf: &[CFrag], env_d: f32, dome_layer: bool, depth_bits: u32, frame: &PeelFrame) -> usize {
    let (d0, cap) = (env_start(env_d, dome_layer, depth_bits), item_cap(dome_layer));
    if buf.len() <= 64 {
        // (uninitialised: zeroing the kilobyte per pixel was 45 % of this function's samples)
        let mut tmp: [std::mem::MaybeUninit<WalkFrag>; 64] = [const { std::mem::MaybeUninit::uninit() }; 64];
        for (i, f) in buf.iter().enumerate() {
            let z01 = frame.z01(f.z).max(0.0);
            tmp[i].write(WalkFrag::from_depths(z01, f.bias, depth_bits, f.tri, i as u32));
        }
        // SAFETY: the first buf.len() slots were written just above; WalkFrag is Copy with no drop
        let init: &mut [WalkFrag] = unsafe { std::slice::from_raw_parts_mut(tmp.as_mut_ptr() as *mut WalkFrag, buf.len()) };
        layer_walk_biased_count(init, d0, cap, depth_bits)
    } else {
        let mut v: Vec<WalkFrag> = buf.iter().enumerate().map(|(i, f)| { let z01 = frame.z01(f.z).max(0.0); WalkFrag::from_depths(z01, f.bias, depth_bits, f.tri, i as u32) }).collect();
        layer_walk_biased_count(&mut v, d0, cap, depth_bits)
    }
}

pub fn layer_walk_sorted(list: &[CFrag], env_d: f32, dome_layer: bool, depth_bits: u32, frame: &PeelFrame) -> usize {
    let mut d_prev = f32::NEG_INFINITY;
    if dome_layer {
        d_prev = if env_d > 0.0 { if depth_bits == 16 { (env_d * 65535.0).round() / 65535.0 } else { env_d } } else { 0.0 };
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
        if depth_bits == 16 {
            dd = (dd.clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
        }
        items += 1;
        d_prev = dd;
    }
    items
}

/// `layer_walk_sorted` of an UNSORTED pixel list with many fragments (the canopy: 4–20): the (z, tri) order as one
/// u64 key — `total_order_key(z)` above the triangle index — sorted by insertion in a fixed buffer, z01 taken once
/// per fragment, then the walk. Beyond 64 fragments the general sort. The same order and operations as
/// `sort_unstable_by(total_cmp, tri)` + `layer_walk_sorted`.
pub fn layer_walk_keyed(buf: &mut [CFrag], env_d: f32, dome_layer: bool, depth_bits: u32, frame: &PeelFrame) -> usize {
    let n = buf.len();
    if n > 64 {
        buf.sort_unstable_by(|p, q| p.z.total_cmp(&q.z).then_with(|| p.tri.cmp(&q.tri)));
        return layer_walk_sorted(buf, env_d, dome_layer, depth_bits, frame);
    }
    let mut keyed: [(u64, f32, f32); 64] = [(0, 0.0, 0.0); 64];
    for (i, f) in buf.iter().enumerate() {
        keyed[i] = ((total_order_key(f.z) as u64) << 32 | f.tri as u64, frame.z01(f.z).max(0.0), f.bias);
    }
    for i in 1..n {
        let cur = keyed[i];
        let mut j = i;
        while j > 0 && keyed[j - 1].0 > cur.0 {
            keyed[j] = keyed[j - 1];
            j -= 1;
        }
        keyed[j] = cur;
    }
    let mut d_prev = f32::NEG_INFINITY;
    if dome_layer {
        d_prev = if env_d > 0.0 { if depth_bits == 16 { (env_d * 65535.0).round() / 65535.0 } else { env_d } } else { 0.0 };
    }
    let mut items = 0usize;
    for &(_, z01, bias) in &keyed[..n] {
        if z01 < d_prev {
            continue;
        }
        if items >= MAX_LAYERS {
            break;
        }
        let mut dd = z01 + bias;
        if depth_bits == 16 {
            dd = (dd.clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
        }
        items += 1;
        d_prev = dd;
    }
    items
}

#[cfg(test)]
mod keyed_walk_audit_tests {
    use super::*;

    /// The keyed walk (0072's path for pixels with five or more fragments) equals the sort + walk reference on
    /// random pixels of 4–80 fragments — depths of BOTH signs (the unsigned key's sign fold), ±0.0, equal depths
    /// on different triangles, biases of both signs (the triangle's term), the environment depth absent and
    /// present, the dome layer and the depth store in all four combinations.
    #[test]
    fn the_keyed_walk_is_the_sorted_walk_with_negative_depths() {
        let frame = PeelFrame::new([0.0, -1.0, 0.0], [-10.0, -3.0, -10.0], [10.0, 7.0, 10.0], 64);
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let zs: [f32; 12] = [-3.0, -2.5, -1.0, -0.0, 0.0, 0.25, 1.0, 2.0, 3.0, 4.0, 4.999, 7.5];
        let biases: [f32; 5] = [0.0, 1.5e-5, -1.5e-5, 3e-4, 1.0 / 65535.0];
        for round in 0..20_000 {
            let n = 4 + (rnd() % 77) as usize;
            let mut list: Vec<CFrag> = (0..n)
                .map(|_| {
                    let z = if rnd() % 3 == 0 { zs[(rnd() % 12) as usize] } else { (rnd() % 20000) as f32 / 1000.0 - 5.0 };
                    let tri = (rnd() % 40) as u32;
                    CFrag { z, tri, bias: biases[(tri % 5) as usize] }
                })
                .collect();
            let env_d = match rnd() % 4 { 0 => 0.0, 1 => 0.5, 2 => (rnd() % 65535) as f32 / 65535.0, _ => ((rnd() % 100000) as f32 / 100000.0).max(1e-6) };
            let (dome, bits) = (round % 2 == 0, if round % 4 < 2 { 16 } else { 32 });
            let mut sorted = list.clone();
            sorted.sort_by(|p, q| p.z.total_cmp(&q.z).then_with(|| p.tri.cmp(&q.tri)));
            let want = layer_walk_sorted(&sorted, env_d, dome, bits, &frame);
            let got = layer_walk_keyed(&mut list, env_d, dome, bits, &frame);
            assert_eq!(got, want, "round {round}: n {n} env {env_d} dome {dome} bits {bits}");
        }
    }
}

#[cfg(test)]
mod key_probe {
    use super::*;
    #[test]
    fn the_key_is_total_cmp() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut vals: Vec<f32> = vec![0.0, -0.0, 1.0, -1.0, f32::MIN_POSITIVE, -f32::MIN_POSITIVE, f32::MAX, f32::MIN, 1e-40, -1e-40, 65535.0, 0.5, -0.5];
        for _ in 0..200_000 {
            seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17;
            let v = f32::from_bits((seed >> 11) as u32);
            if v.is_nan() { continue; }
            vals.push(v);
        }
        for i in 0..vals.len().min(3000) {
            for j in 0..vals.len().min(3000) {
                let (a, b) = (vals[i], vals[j]);
                assert_eq!(a.total_cmp(&b), total_order_key(a).cmp(&total_order_key(b)), "{a} vs {b}");
            }
        }
    }
}

/// LMTOOL_RASTER_STATS: pixels with 1–2 fragments, 3, 4, 5, ≥ 6.
pub static RS_NFRAG: [std::sync::atomic::AtomicU64; 9] = [const { std::sync::atomic::AtomicU64::new(0) }; 9];
/// LMTOOL_NO_SCAN16=1 keeps the scan's per-pixel walk scalar (the A/B switch).
pub static SCAN16_ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("LMTOOL_NO_SCAN16").is_none());

/// THE SLOT FORM of the count records (the default; LMTOOL_SCAN_SLOTS=0 restores the list form): the records
/// written into per-pixel SLOTS at visit time (`SLOTS_PER_PX` × 12 B per pixel of the job's rect, the rest to an
/// overflow list) instead of a sequential list counting-sorted by a prefix + scatter pass; the lane walk gathers
/// at li·SLOTS_PER_PX + j and only the overflow is scattered. (The 0053 form wrote 64-byte slots into a per-band
/// table larger than L2 and lost; a 128 × 128 job's 4-slot table is 786 KB, L2-resident: tiny raster −3 %, giant
/// neutral, measured 2026-09-26.)
pub static SCAN_SLOTS: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_SCAN_SLOTS").map(|v| v != "0").unwrap_or(true));
pub const SLOTS_PER_PX: usize = 4;

/// The count records of a job: the sequential list (the scatter form), or the per-pixel slots + the overflow list.
pub struct Recs {
    pub list: Vec<(u32, CFrag)>,
    /// The slot table, STRUCTURE-OF-ARRAYS PER 16-PIXEL BLOCK: block b (pixels 16b..16b+16), slot j, field f
    /// (0 = z bits, 1 = the tie key, 2 = bias bits) → the 16 lanes contiguous at `((b·SLOTS_PER_PX + j)·3 + f)·16`
    /// — the lane walk loads a slot's z / key / bias as three 64-byte vectors instead of three gathers.
    pub slots: Vec<SlotBlock>,
    pub slot_mode: bool,
}
/// One 16-lane row of the slot table (64-byte aligned: the lane walk's loads are whole lines).
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct SlotBlock(pub [u32; 16]);
pub const SLOT_ROWS_PER_BLOCK: usize = SLOTS_PER_PX * 3;
impl Recs {
    /// Record `rec` of pixel `li` whose count (after the bump) is `c_after`.
    #[inline(always)]
    pub fn push(&mut self, li: u32, c_after: u16, rec: CFrag) {
        if self.slot_mode && (c_after as usize) <= SLOTS_PER_PX && c_after >= 1 {
            let (b, lane) = ((li as usize) >> 4, (li as usize) & 15);
            let row = (b * SLOTS_PER_PX + c_after as usize - 1) * 3;
            // (the table is sized for the job's blocks before its visits)
            let rows = &mut self.slots[row..row + 3];
            rows[0].0[lane] = rec.z.to_bits();
            rows[1].0[lane] = rec.tri;
            rows[2].0[lane] = rec.bias.to_bits();
        } else {
            self.list.push((li, rec));
        }
    }
    /// The slot record `j` of pixel `li` (j < its count).
    #[inline(always)]
    pub fn slot(&self, li: usize, j: usize) -> CFrag {
        let (b, lane) = (li >> 4, li & 15);
        let row = (b * SLOTS_PER_PX + j) * 3;
        CFrag { z: f32::from_bits(self.slots[row].0[lane]), tri: self.slots[row + 1].0[lane], bias: f32::from_bits(self.slots[row + 2].0[lane]) }
    }
}

thread_local! {
    /// The raster band's slot and environment tables, kept per pool thread across bands (see the fused count).
    static SLOT_BUFS: std::cell::Cell<(Vec<u16>, Vec<f32>, Vec<(u32, CFrag)>, Vec<CFrag>, Vec<u32>, Vec<u32>)> = const { std::cell::Cell::new((Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())) };
    /// The per-pixel slot table of the slot form, per pool thread.
    static SLOT_TABLE: std::cell::Cell<Vec<SlotBlock>> = const { std::cell::Cell::new(Vec::new()) };
    /// A raster job's gathered triangle list, kept per pool thread across jobs.
    static JOB_LIST: std::cell::Cell<Vec<u32>> = const { std::cell::Cell::new(Vec::new()) };
}

/// Raster bands per pool thread (LMTOOL_BANDS_PER_THREAD, default 4).
pub static BANDS_PER_THREAD: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_BANDS_PER_THREAD").ok().and_then(|v| v.parse().ok()).unwrap_or(4));

/// The sparse raster's tile cells (LMTOOL_TILE_ROWS × LMTOOL_TILE_COLS pixels, default 128 × 128: 16 k pixels of count
/// tables — L1-resident — and 64-pixel row spans; see `build_abuffer_sparse_ranges`).
// (128 × 128 cells: measured on the aligned tree — tiny 32 dirs 2.20 → 2.17 s (the CSR 0.27 → 0.24: fewer, longer per-pixel
// runs to scatter), giant within noise; 64 × 64 was the first choice)
pub static TILE_ROWS: std::sync::LazyLock<u32> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_TILE_ROWS").ok().and_then(|v| v.parse().ok()).filter(|&v: &u32| v > 0).unwrap_or(128));
pub static TILE_COLS: std::sync::LazyLock<u32> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_TILE_COLS").ok().and_then(|v| v.parse().ok()).filter(|&v: &u32| v > 0).unwrap_or(128));
/// The cost estimate of one band-triangle pair in pixel-test units (LMTOOL_PAIR_COST, default 48).
pub static PAIR_COST_V: std::sync::LazyLock<u64> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_PAIR_COST").ok().and_then(|v| v.parse().ok()).unwrap_or(48));
/// The cost weight of a card (alpha-tested) triangle relative to an opaque one (LMTOOL_CARD_WEIGHT, default 3).
pub static CARD_WEIGHT_V: std::sync::LazyLock<u64> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_CARD_WEIGHT").ok().and_then(|v| v.parse().ok()).unwrap_or(3));
/// The heavy-cell split limit as a divisor of a thread's share of the frame (LMTOOL_TILE_SPLIT, default 8).
pub static TILE_SPLIT_V: std::sync::LazyLock<u64> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_TILE_SPLIT").ok().and_then(|v| v.parse().ok()).filter(|&v: &u64| v > 0).unwrap_or(8));

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
    pub static L_CSR: AtomicU64 = AtomicU64::new(0);
    pub static PROBE_PX: AtomicU64 = AtomicU64::new(0);
    /// The wanted set's parts (perf 8): the LM fragment list's build (once per offset), the LM fragments' texels pass, the
    /// census pixels, PixelIndex::new.
    pub static W_FRAGLIST: AtomicU64 = AtomicU64::new(0);
    pub static W_LMPASS: AtomicU64 = AtomicU64::new(0);
    pub static W_CENSUS: AtomicU64 = AtomicU64::new(0);
    pub static W_INDEX: AtomicU64 = AtomicU64::new(0);
    /// The per-direction glue outside the stages: the dome raster, the wanted bitmap, the sel/occl clear.
    pub static DOME: AtomicU64 = AtomicU64::new(0);
    pub static BITMAP: AtomicU64 = AtomicU64::new(0);
    pub static CLEAR: AtomicU64 = AtomicU64::new(0);
    pub static CONTRIB: AtomicU64 = AtomicU64::new(0);
    pub static CULL: AtomicU64 = AtomicU64::new(0);
    /// The non-raster stages inside a direction (perf 8): the transcribed accumulate's pieces — the layer Bufs handed to
    /// the LM raster and the probes, LmILightDir_Set (the LM raster per chunk of layers), the probe passes per world layer,
    /// the dome image per pixel (inside the layer-derivation timer), the H-basis draw, the probes' end-of-direction folds,
    /// the sub-sample accumulate — and the pieces of the gather (the dome colour per sky sub-sample).
    pub static LM_BUFS: AtomicU64 = AtomicU64::new(0);
    pub static LM_SET: AtomicU64 = AtomicU64::new(0);
    pub static PROBE_LAYER: AtomicU64 = AtomicU64::new(0);
    pub static DOME_IMG: AtomicU64 = AtomicU64::new(0);
    pub static HB: AtomicU64 = AtomicU64::new(0);
    pub static PROBE_END: AtomicU64 = AtomicU64::new(0);
    pub static ACC_SUB: AtomicU64 = AtomicU64::new(0);
    /// The gather's and the accumulate's summed task times (CPU busy inside the pool tasks) beside their wall times.
    pub static GATHER_CPU: AtomicU64 = AtomicU64::new(0);
    pub static ACC_CPU: AtomicU64 = AtomicU64::new(0);
    pub static AMBIENT: AtomicU64 = AtomicU64::new(0);
    /// The sweep's stages outside the direction loop: the layout raster (the sub-samples), the nine jitter sets, the
    /// shadow map, the harness dumps before the loop, the resolve after it.
    pub static PRE_SUBS: AtomicU64 = AtomicU64::new(0);
    pub static PRE_JITTER: AtomicU64 = AtomicU64::new(0);
    pub static PRE_SHADOW: AtomicU64 = AtomicU64::new(0);
    pub static PRE_DUMP: AtomicU64 = AtomicU64::new(0);
    pub static POST_RESOLVE: AtomicU64 = AtomicU64::new(0);
    pub fn add(c: &AtomicU64, t: std::time::Instant) {
        c.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    pub fn report(label: &str, total: f32) {
        let g = |c: &AtomicU64| c.load(Ordering::Relaxed) as f64 / 1e9;
        let staged = g(&BUILD) + g(&LAYERS) + g(&DUMP) + g(&GATHER) + g(&ACCUM) + g(&SNAP) + g(&FRAMES) + g(&EXACT);
        crate::alphatex::alpha_stats_report();
        crate::alphasimd::alpha_queue_report();
        if crate::peel::DROP_STATS[3].load(Ordering::Relaxed) > 0 { let d = |i: usize| crate::peel::DROP_STATS[i].swap(0, Ordering::Relaxed); let (a, b, c, k) = (d(0), d(1), d(2), d(3)); let t = (a + b + c + k).max(1); eprintln!("drop stats [{label}]: of {t} alpha-passing item fragments: behind the environment {a} ({:.1} %), in a bias window {b} ({:.1} %), past the cap {c} ({:.1} %), accepted as layers {k} ({:.1} %)", a as f64 * 100.0 / t as f64, b as f64 * 100.0 / t as f64, c as f64 * 100.0 / t as f64, k as f64 * 100.0 / t as f64); }
        if *crate::pool::POOL_STATS {
          let mut log = crate::pool::RUN_LOG.lock().unwrap();
          let (wall_all, runs_all): (u64, usize) = (log.iter().map(|r| r.0).sum(), log.len());
          eprintln!("pool regions [{label}]: {} runs, {:.2}s inside the parallel regions (the rest of the directions total is serial glue)", runs_all, wall_all as f64 * 1e-9);
          // group by task count n: wall, busy, threads·wall (capacity), the mean max-task share
          let mut by_n: std::collections::BTreeMap<usize, (u64, u64, u64, u64, usize)> = std::collections::BTreeMap::new();
          for (wall, busy, maxt, n, thr) in log.iter() { let e = by_n.entry(*n).or_insert((0, 0, 0, 0, 0)); e.0 += wall; e.1 += busy; e.2 += wall * *thr as u64; e.3 += maxt; e.4 += 1; }
          let mut rows: Vec<_> = by_n.into_iter().collect();
          rows.sort_by(|a, b| b.1.0.cmp(&a.1.0));
          eprintln!("pool regions by task count (top 12 by wall): n | runs | wall s | busy s | busy / capacity | mean max-task / mean wall");
          for (n, (wall, busy, cap, maxt, runs)) in rows.iter().take(12) { eprintln!("  {n:6} | {runs:5} | {:7.3} | {:8.3} | {:5.1} % | {:5.1} %", *wall as f64 * 1e-9, *busy as f64 * 1e-9, *busy as f64 * 100.0 / (*cap).max(1) as f64, *maxt as f64 * 100.0 / (*wall).max(1) as f64); }
          log.clear(); }
        if crate::peel::CERT_STATS[0].load(Ordering::Relaxed) > 0 { let c = |i: usize| crate::peel::CERT_STATS[i].swap(0, Ordering::Relaxed); eprintln!("cert stats [{label}]: {} frames; the wanted pixels' layers all within the census lower bound: {} frames (1/16 census), {} (1/64); within the exact count {} (sanity: must be all)", c(0), c(1), c(2), c(3)); }
        if crate::peel::BOUND_TOTALS[0].load(Ordering::Relaxed) > 0 { eprintln!("bound stats [{label}]: {} frames, stop certified right by the census/4 bounds {} and by census/8 {}, certified WRONG {}", crate::peel::BOUND_TOTALS[0].swap(0, Ordering::Relaxed), crate::peel::BOUND_TOTALS[1].swap(0, Ordering::Relaxed), crate::peel::BOUND_TOTALS[2].swap(0, Ordering::Relaxed), crate::peel::BOUND_TOTALS[3].swap(0, Ordering::Relaxed)); }
        eprintln!("profile [{label}] glue: dome raster {:.2}s, wanted bitmap {:.2}s, BVH cull {:.2}s, sel/occl clear {:.2}s, contribution {:.2}s; layer CSR {:.2}s, probe pixels {:.2}s; wanted parts: LM frag list {:.2}s, LM texels {:.2}s, census {:.2}s, PixelIndex::new {:.2}s", g(&DOME), g(&BITMAP), g(&CULL), g(&CLEAR), g(&CONTRIB), g(&L_CSR), g(&PROBE_PX), g(&W_FRAGLIST), g(&W_LMPASS), g(&W_CENSUS), g(&W_INDEX));
        eprintln!("profile [{label}]: A-buffer build {:.2}s (wanted index {:.2}s, clip {:.2}s, raster {:.2}s, CSR {:.2}s), exact layer count {:.2}s, layer derivation {:.2}s (parallel part {:.2}s), per-direction dumps {:.2}s, gather {:.2}s, accumulate {:.2}s, accumulation snapshots {:.2}s, frames {:.2}s; directions total {:.2}s (unstaged {:.2}s); sweep total {total:.2}s", g(&BUILD), g(&B_INDEX), g(&B_CLIP), g(&B_RASTER), g(&B_SORT), g(&EXACT), g(&LAYERS), g(&L_PAR), g(&DUMP), g(&GATHER), g(&ACCUM), g(&SNAP), g(&FRAMES), g(&DIR), g(&DIR) - staged);
        // the pool's per-stage utilisation table (per-thread busy time; pool::stats), against the directions total
        crate::pool::stats::report(label, g(&DIR));
        if crate::hugealloc::HUGE_ALLOCS.load(Ordering::Relaxed) > 0 { eprintln!("profile [{label}] {}", crate::hugealloc::report()); }
        eprintln!("profile [{label}] non-raster: layer bufs {:.2}s, LmILightDir_Set {:.2}s, probe layers {:.2}s, dome image {:.2}s (in layer derivation), AddAmbient {:.2}s, H-basis {:.2}s, probe folds {:.2}s, sub-sample accumulate {:.2}s; outside the loop: sub-samples {:.2}s, jitter sets {:.2}s, shadow map {:.2}s, pre-loop dumps {:.2}s, resolve {:.2}s; gather lookups: {} sky (dome_px), {} surface; task CPU: gather {:.2}s, accumulate {:.2}s", g(&LM_BUFS), g(&LM_SET), g(&PROBE_LAYER), g(&DOME_IMG), g(&AMBIENT), g(&HB), g(&PROBE_END), g(&ACC_SUB), g(&PRE_SUBS), g(&PRE_JITTER), g(&PRE_SHADOW), g(&PRE_DUMP), g(&POST_RESOLVE), crate::peel::GATHER_COUNTS[0].swap(0, Ordering::Relaxed), crate::peel::GATHER_COUNTS[1].swap(0, Ordering::Relaxed), g(&GATHER_CPU), g(&ACC_CPU));
        for c in [&BUILD, &LAYERS, &DUMP, &GATHER, &ACCUM, &SNAP, &B_CLIP, &B_RASTER, &B_SORT, &B_INDEX, &DIR, &FRAMES, &EXACT, &L_PAR, &DOME, &BITMAP, &CLEAR, &CONTRIB, &CULL, &L_CSR, &W_FRAGLIST, &W_LMPASS, &W_CENSUS, &W_INDEX, &LM_BUFS, &LM_SET, &PROBE_LAYER, &DOME_IMG, &AMBIENT, &HB, &PROBE_END, &ACC_SUB, &PRE_SUBS, &PRE_JITTER, &PRE_SHADOW, &PRE_DUMP, &POST_RESOLVE, &GATHER_CPU, &ACC_CPU] { c.store(0, Ordering::Relaxed); }
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
    if *BIASED_ORDER {
        // (one BVH per bake; a second call with another BVH would need a per-bake table instead of the static)
        let _ = DRAW_RANK.set(draw_rank_table(bvh));
        assert_eq!(DRAW_RANK.get().map(|r| r.len()), Some(bvh.tris.len()), "the draw-rank table belongs to another BVH");
    }
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
    if prm.profile || std::env::var_os("LMTOOL_PROFILE").is_some() || std::env::var_os("LMTOOL_POOL_STATS").is_some() { crate::pool::stats::enable(); }
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
    // THE INSTANCE HIERARCHY (perf engineer 5's insthier.rs; LMTOOL_INSTHIER=1 turns it on — OFF by default: measured on
    // the giant it moves the per-triangle test from the binning into the cell jobs and inflates it (the world frame's
    // jobs hold 27 M triangles for 9.8 M with a candidate centre, ×1.3 cells per job): binning 0.32 → 0.03 s but raster
    // 1.30 → 1.75 s per 4 directions; the visited set is identical (VIOLATIONS 0). LMTOOL_INSTHIER_GRAIN =
    // the jobs' pixel grain, default 16): built once per bake; per peel frame its jobs replace the per-triangle
    // binning's projection of every culled triangle (see build_abuffer_sparse_items)
    let insthier_on = std::env::var("LMTOOL_INSTHIER").map(|v| v == "1").unwrap_or(false);
    let insthier_grain: i32 = std::env::var("LMTOOL_INSTHIER_GRAIN").ok().and_then(|v| v.parse().ok()).unwrap_or(16);
    let insthier_check = std::env::var_os("LMTOOL_INSTHIER_CHECK").is_some();
    let insthier: Option<crate::insthier::InstHier> = if insthier_on && prm.game_peel { Some(crate::insthier::InstHier::build(scene, bvh, &tri_base)) } else { None };
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
    prof::add(&prof::PRE_SUBS, t0);
    let t_jit = std::time::Instant::now();
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
    prof::add(&prof::PRE_JITTER, t_jit);
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
    // LMTOOL_SUBS_DUPCHECK=1: do two sub-samples of a set share an accumulation cell? (the accumulate's parallel chunks write
    // `acc_tex[chart][cell] +=` without atomics on the promise that they never do — a duplicate would be a data race)
    if std::env::var_os("LMTOOL_SUBS_DUPCHECK").is_some() {
        let sets: Vec<&Vec<SubSample>> = if jitter { jit_sets.iter().collect() } else { vec![&subs] };
        for (si, set) in sets.iter().enumerate() {
            let mut keys: Vec<(u32, u32)> = set.iter().map(|s| { let (c, p) = pix_of(s); (c as u32, p as u32) }).collect();
            keys.sort_unstable();
            let dups = keys.windows(2).filter(|w| w[0] == w[1]).count();
            eprintln!("subs dupcheck: set {si}: {} sub-samples, {} duplicate accumulation cells", set.len(), dups);
        }
    }
    // 3. the sun shadow map for the fragment radiance
    let t_shadow = std::time::Instant::now();
    let (bmin, bmax) = scene_bounds(&bvh.tris);
    // (built once per bake when the caller shares a cache across the sweeps: the same scene, sun and frame every sweep)
    let shadow: Option<std::sync::Arc<ShadowMap>> = if prm.sun_dir[1] > 0.0 && prm.sun.iter().any(|c| *c > 0.0) {
        let cached = prm.shadow_cache.as_ref().and_then(|c| c.lock().unwrap().clone());
        match cached {
            Some(sm) => Some(sm),
            None => {
                let frame = match &prm.shadow_frustum {
                    Some(f) => PeelFrame::from_frustum(f, prm.peel_res.max(1024), prm.peel_res.max(1024)),
                    None => PeelFrame::new(prm.sun_dir, bmin, bmax, prm.peel_res.max(1024)),
                };
                let sm = std::sync::Arc::new(ShadowMap::build_in(&bvh.tris, frame, &prm.alpha_masks));
                if let Some(c) = &prm.shadow_cache { *c.lock().unwrap() = Some(sm.clone()); }
                Some(sm)
            }
        }
    } else { None };
    prof::add(&prof::PRE_SHADOW, t_shadow);
    let t_predump = std::time::Instant::now();
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
    prof::add(&prof::PRE_DUMP, t_predump);
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
    let mut dir_lm_buf: Option<crate::lmaccum::DirTarget> = None;
    let mut hb_lm: Option<crate::lmaccum::HbTargets> = prm.lm_scene.as_ref().map(|_| { let t = std::time::Instant::now(); let hb = crate::lmaccum::HbTargets::cleared(2048, 2048); if crate::lmaccum::lmaccum_trace() { eprintln!("lmaccum trace: HbTargets::cleared {:.1} ms", t.elapsed().as_secs_f64() * 1e3); } hb });
    let mut lm_rows: Vec<String> = Vec::new();
    // the per-direction buffers, allocated once (their fills run on the pool): the selected radiance and
    // occlusion flag per sub-sample, and the identity index range of the jitter sets
    let max_set = if jitter { jit_sets.iter().map(|s| s.len()).max().unwrap_or(0) } else { subs.len() };
    let mut sel_buf: Vec<[f32; 3]> = vec![[0.0; 3]; max_set];
    let mut occl_buf: Vec<bool> = vec![false; max_set];
    // THE GENERATION STAMP (perf 8): instead of clearing `sel` to the sky and `occl` to false for every sub-sample before
    // each direction (38 MB of writes on the tiny map, 0.8 ms per direction), a hit stamps its sub-sample with the
    // direction's generation; a reader takes (sel, occl) where the stamp matches and (sky_fill, false) elsewhere — the
    // same values the clear provided. The generation never repeats within the bake.
    let mut stamp_buf: Vec<u32> = vec![0u32; max_set];
    let mut stamp_gen: u32 = 0;
    let range_all_buf: Vec<u32> = if jitter { (0..max_set as u32).collect() } else { Vec::new() };
    // the tiles' world-XZ clip boxes per peel index (None = the world peel: no clip), see the gather
    let tile_clip: Vec<Option<[f32; 4]>> = prm.peel_tile_clip.as_ref().map(|v| v.as_ref().clone()).unwrap_or_default();
    // THE TILES' SUB-SAMPLES (perf 8): a tile peel's clip box is fixed for the bake, so which sub-samples of a jitter set
    // lie inside it is decided once here — the wanted bitmap and the gather of a tile peel then walk that list instead
    // of every sub-sample with the clip test (the tests dropped most of some chunks and none of others: the runs were
    // 40–60 % busy; a giant's nine tiles each walked all the sub-samples). Indexed [jitter set][peel]; None = no clip
    // (the world peel) or the non-jitter path, which keeps the plain walk. The same sub-samples in a different order
    // (each is independent): the same bits, the same selections.
    // (the sets: the nine jitter sets, or the one sub-sample set of the plain path when it is one group — then a
    // direction's range is every sub-sample)
    let tile_sets: Vec<&Vec<SubSample>> = if jitter { jit_sets.iter().collect() } else if groups == 1 { vec![&subs] } else { Vec::new() };
    // THE SUB-SAMPLES' POSITIONS, COMPACT (perf 8): the wanted-bitmap pass reads one field of the 48-byte SubSample per
    // sub-sample — 140 MB streamed per peel on the tiny map, the pass's cost was the bandwidth, not the atomic OR (a plain
    // store timed the same). A 12-byte position array per set, built once, is what it streams.
    let set_pos: Vec<Vec<[f32; 3]>> = if jitter { jit_sets.iter().map(|s| s.iter().map(|x| x.p).collect()).collect() } else { vec![subs.iter().map(|x| x.p).collect()] };
    let tile_subs: Vec<Vec<Option<Vec<u32>>>> = if !tile_sets.is_empty() && tile_clip.iter().any(|c| c.is_some()) {
        let t0 = std::time::Instant::now();
        let lists: Vec<Vec<Option<Vec<u32>>>> = tile_sets.iter().map(|set| {
            tile_clip.iter().map(|clip| clip.map(|b| {
                let n = set.len();
                let nch = (threads * 2).max(1);
                let per = (n + nch - 1) / nch;
                let parts: Vec<Vec<u32>> = crate::pool::pool().map(nch, |ci| {
                    let (a, e) = ((ci * per).min(n), ((ci + 1) * per).min(n));
                    let mut out = Vec::new();
                    for i in a..e {
                        let s = &set[i];
                        if s.p[0] - b[0] >= 0.0 && s.p[2] - b[1] >= 0.0 && b[2] - s.p[0] >= 0.0 && b[3] - s.p[2] >= 0.0 { out.push(i as u32); }
                    }
                    out
                });
                parts.concat()
            })).collect()
        }).collect();
        let total: usize = lists.iter().flatten().flatten().map(|v| v.len()).sum();
        eprintln!("peel: the tiles' sub-sample lists: {} tile peels × {} sub-sample sets, {} entries ({:.2}s)", tile_clip.iter().filter(|c| c.is_some()).count(), tile_sets.len(), total, t0.elapsed().as_secs_f32());
        lists
    } else { Vec::new() };
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
    if *crate::pool::POOL_STATS { crate::pool::RUN_LOG.lock().unwrap().clear(); }
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
        let t_dir = std::time::Instant::now(); crate::pool::stats::stage("frames"); crate::pool::stats::epoch();
        let g = di % groups;
        let scale = 4.0 / group_count[g].max(1) as f32;
        // THE PEELS of this direction: the captured frustums (the game runs two — the whole-scene frustum,
        // then one fitted to the items — and the accumulate takes the later peel's layer wherever it has
        // one), or the port's own single frame fit to the receivers
        let peels: Vec<PeelFrame> = match prm.frustums.as_ref().and_then(|fs| fs.get(di)).filter(|v| !v.is_empty()) {
            // (the non-exact --tile-res: the fitted tiles — every peel after the world's — at their own size)
            Some(frs) => frs.iter().enumerate().map(|(pi, fr)| { let r = if pi > 0 && prm.tile_res > 0 { prm.tile_res } else { prm.peel_res }; PeelFrame::from_frustum(fr, r, r) }).collect(),
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
        let chunk = (range.len() / (threads.max(1) * 8)).max(1024); // (eight tasks per thread: the sky-hit sub-samples cost the dome evaluation, the others a lookup)
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
        stamp_gen += 1;
        let gen = stamp_gen;
        let stamp: &mut [u32] = &mut stamp_buf[..cur.len()];
        let t_clear = std::time::Instant::now();
        // (no clear: the generation stamp above stands in for it)
        prof::add(&prof::CLEAR, t_clear);
        let mut t_build_total = 0.0f32;
        // the transcribed accumulate's TMapILightDir of this direction (cleared before the first block)
        // (one allocation per sweep, cleared per direction — perf 8: a fresh 16 MB target per direction faulted its pages in
        // from every pool thread at once)
        let mut dir_lm: Option<crate::lmaccum::DirTarget> = match dir_lm_buf.take() { Some(mut t) => { t.clear(); Some(t) } None => prm.lm_scene.as_ref().map(|_| crate::lmaccum::DirTarget::cleared(2048, 2048)) };
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
                } else {
                    sel[i] = sky_fill;
                }
                occl[i] = c.occl(i);
                stamp[i] = gen;
            }
            assert_eq!(k, c.sel.len(), "merge-contrib: direction {di}: {} facing sub-samples, {} stored", k, c.sel.len());
            if let Some(pb) = &prm.probe_bake { pb.lock().unwrap().import_direction(&c.probe_cur, &c.sky_adds); }
        } else {
        for (pi, frame) in peels.iter().enumerate() {
            CURRENT_PEEL.store(pi as u32, std::sync::atomic::Ordering::Relaxed);
            if ABUF_DEBUG_LIST.is_some() { *ABUF_DEBUG_BIAS.lock().unwrap() = (prm.depth_bias, prm.depth_bits); }
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
            let t_idx = std::time::Instant::now(); crate::pool::stats::stage("bitmap+index");
            // (the transcribed accumulate reads every pixel of the layers: the dense path when --lm-from is on)
            // THE TRANSCRIBED ACCUMULATE'S PIXELS (perf 8): the LM raster reads the peel layers only where its fragments project
            // (PS 17112: the depth compare at the undivided (u, v), the colour at (u, v)/w — the same texel for the ortho peel), and
            // the fragment list of the direction's raster offset names them all; so with the LM scene the wanted set is the sub-
            // samples' pixels ∪ the LM fragments' texels ∪ the probes' texels ∪ the centre pixel (AddAmbient) — not every pixel
            // (Stadium stpad: 3.2 M of the 16.8 M per peel). LMTOOL_LM_DENSE=1 keeps the dense path.
            let lm_sparse = prm.lm_scene.is_some() && crate::lmaccum::frag_list_on() && std::env::var_os("LMTOOL_LM_DENSE").is_none();
            let wanted: Option<std::sync::Arc<PixelIndex>> = if prm.game_peel && !want_dir_dump && (prm.lm_scene.is_none() || lm_sparse) {
                let n = (frame.res as usize * frame.res_y as usize + 63) / 64;
                // one shared bitmap, the bits OR-ed in atomically (neighbouring sub-samples share words,
                // and neighbours sit in the same chunk — the contention is nil)
                let nch = (threads * 2).max(1);
                let per = (cur.len() + nch - 1) / nch;
                // (the bitmap's words come from the recycler and are cleared in parallel — a fresh 2 MB vector
                // per frame was a serial fill, then a serial copy out of the atomics)
                let mut m_words: Vec<u64> = U64S.take_with_capacity(n);
                // SAFETY: capacity ≥ n; every word is written by the parallel clear below before any read
                unsafe { m_words.set_len(n); }
                {
                    // (perf 8: sixteen 128 KB tasks — a serial 2 MB memset of the cold words was 0.3 ms; the pool now takes no
                    // more workers than tasks, so this run wakes sixteen)
                    let mp = m_words.as_mut_ptr() as usize;
                    let ncl = 16usize;
                    let zc = (n + ncl - 1) / ncl;
                    crate::pool::stats::stage("bitmap-clear");
                    crate::pool::pool().run(ncl, |ci| {
                        let (a, b) = (ci * zc, ((ci + 1) * zc).min(n));
                        if a < b {
                            // SAFETY: the chunks partition the vector
                            unsafe { std::ptr::write_bytes((mp as *mut u64).add(a), 0, b - a); }
                        }
                    });
                }
                // SAFETY: AtomicU64 has u64's size, alignment and bit validity
                let m: Vec<std::sync::atomic::AtomicU64> = unsafe {
                    let mut v = std::mem::ManuallyDrop::new(m_words);
                    Vec::from_raw_parts(v.as_mut_ptr() as *mut std::sync::atomic::AtomicU64, v.len(), v.capacity())
                };
                // a tile's frame: only the sub-samples inside the tile's world-XZ cell read it (the gather's
                // clip rule below) — the others' pixels are not wanted (a giant's tile holds a ninth of them)
                let clip_box: Option<[f32; 4]> = tile_clip.get(pi).copied().flatten();
                let tile_list: Option<&Vec<u32>> = tile_subs.get(if jitter { di % 9 } else { 0 }).and_then(|v| v.get(pi)).and_then(|o| o.as_ref());
                crate::pool::stats::stage("bitmap-or");
                let pix_of_pos = |p: [f32; 3]| -> usize {
                    let (x, y, _) = frame.project(p);
                    let (px, py) = (lookup_pixel(x, frame.res, prm.peel_inset), lookup_pixel(y, frame.res_y, prm.peel_inset));
                    py as usize * frame.res as usize + px as usize
                };
                let pix_of_sub = |s: &SubSample| -> usize { pix_of_pos(s.p) };
                let cur_pos: &[[f32; 3]] = &set_pos[if jitter { di % 9 } else { 0 }];
                let mark_pos = |p: [f32; 3]| {
                    let i = pix_of_pos(p);
                    m[i >> 6].fetch_or(1u64 << (i & 63), std::sync::atomic::Ordering::Relaxed);
                };
                match tile_list {
                    // the LM path (perf 8): the sub-samples in chart order OR-ed by 160 threads into the same lines — through the
                    // private bitmaps instead (the tile lists are not built on this path: one group, jitter off... they may be)
                    _ if lm_sparse => {
                        crate::pool::stats::stage("or-subs");
                        let items: &[SubSample] = cur;
                        match tile_list {
                            Some(list) => or_pass_private(&m, list.len(), |i| [Some(pix_of_sub(&items[list[i] as usize])), None]),
                            None => or_pass_private(&m, items.len(), |i| {
                                let s = &items[i];
                                if let Some(b) = clip_box {
                                    if !(s.p[0] - b[0] >= 0.0 && s.p[2] - b[1] >= 0.0 && b[2] - s.p[0] >= 0.0 && b[3] - s.p[2] >= 0.0) { return [None, None]; }
                                }
                                [Some(pix_of_sub(s)), None]
                            }),
                        }
                    }
                    // a tile peel: its sub-samples, listed once per sweep (eight tasks per thread)
                    Some(list) => {
                        let nl = list.len();
                        let nch = (threads * 8).max(1);
                        let per = (nl + nch - 1) / nch;
                        crate::pool::pool().run(nch, |ci| {
                            for &i in &list[(ci * per).min(nl)..((ci + 1) * per).min(nl)] { mark_pos(cur_pos[i as usize]); }
                        });
                    }
                    None => {
                        let nch = (threads * 8).max(1);
                        let per = (cur.len() + nch - 1) / nch;
                        crate::pool::pool().run(nch, |ci| {
                            for p in &cur_pos[(ci * per).min(cur.len())..((ci + 1) * per).min(cur.len())] {
                                if let Some(b) = clip_box {
                                    if !(p[0] - b[0] >= 0.0 && p[2] - b[1] >= 0.0 && b[2] - p[0] >= 0.0 && b[3] - p[2] >= 0.0) { continue; }
                                }
                                mark_pos(*p);
                            }
                        });
                    }
                }
                if let (Some(lm), true) = (prm.lm_scene.as_ref(), lm_sparse) {
                    let t_fl = std::time::Instant::now();
                    let fl = lm.frag_list(di, 2048, 2048);
                    prof::add(&prof::W_FRAGLIST, t_fl);
                    let t_lm = std::time::Instant::now(); crate::pool::stats::stage("bitmap-lm");
                    let pw01 = frame.world_pw01();
                    let (fw, fh) = (frame.res, frame.res_y);
                    let nf = fl.frags.len();
                    // (the fragments in instance order name neighbouring pixels: through the private bitmaps, no atomics)
                    let frags = &fl.frags;
                    or_pass_private(&m, nf, |i| {
                        let p = frags[i].pos;
                        let u = p[0] * pw01[0][0] + p[1] * pw01[1][0] + p[2] * pw01[2][0] + pw01[3][0];
                        let v = p[0] * pw01[0][1] + p[1] * pw01[1][1] + p[2] * pw01[2][1] + pw01[3][1];
                        let w = p[0] * pw01[0][3] + p[1] * pw01[1][3] + p[2] * pw01[2][3] + pw01[3][3];
                        let mut out = [None, None];
                        if u.is_finite() && v.is_finite() {
                            let (tx, ty) = (crate::lmaccum::point_texel(u, fw), crate::lmaccum::point_texel(v, fh));
                            out[0] = Some(ty as usize * fw as usize + tx as usize);
                        }
                        // (the colour texel at (u, v)/w: the same texel for the orthographic peel, where w is exactly 1)
                        if w != 1.0 {
                            let (uu, vv) = (u / w, v / w);
                            if uu.is_finite() && vv.is_finite() {
                                let (tx, ty) = (crate::lmaccum::point_texel(uu, fw), crate::lmaccum::point_texel(vv, fh));
                                let i = ty as usize * fw as usize + tx as usize;
                                if out[0] != Some(i) { out[1] = Some(i); }
                            }
                        }
                        out
                    });
                    // the centre pixel: AddAmbient (CS 17125) reads layer 0's colour there
                    let c = (fh as usize / 2) * fw as usize + fw as usize / 2;
                    m[c >> 6].fetch_or(1u64 << (c & 63), std::sync::atomic::Ordering::Relaxed);
                    prof::add(&prof::W_LMPASS, t_lm);
                }
                // SAFETY: as above, back to plain words (no copy)
                let mut m: Vec<u64> = unsafe {
                    let mut v = std::mem::ManuallyDrop::new(m);
                    Vec::from_raw_parts(v.as_mut_ptr() as *mut u64, v.len(), v.capacity())
                };
                // THE PROBES' PIXELS (the transcribed probe passes read the world peel's layer targets at every
                // probe's shadow coordinate — a 2×2 comparison filter and a point colour sample): the 3×3 around
                // each probe's texel joins the wanted set, so the sparse layers hold what the passes read
                if pi == 0 {
                    if let Some(pb) = &prm.probe_bake {
                        let t_probe_px = std::time::Instant::now(); crate::pool::stats::stage("probe-px");
                        let pb = pb.lock().unwrap();
                        let pw01 = frame.world_pw01();
                        let (w, h) = (frame.res as i64, frame.res_y as i64);
                        // (in parallel over the blocks' z slices: the bits are OR-ed atomically into the words)
                        let slices: Vec<(usize, u32)> = pb.blocks.iter().enumerate().flat_map(|(bi, b)| (b.min[2]..b.max[2]).map(move |z| (bi, z))).collect();
                        let draws: Vec<_> = pb.blocks.iter().map(|b| b.draw(&pw01, 1.0)).collect();
                        let mp = m.as_mut_ptr() as usize;
                        let pbr = &*pb;
                        crate::pool::pool().run(slices.len(), |si| {
                            let (bi, z) = slices[si];
                            let b = &pbr.blocks[bi];
                            let dr = &draws[bi];
                            // SAFETY: the words are shared read-write across the tasks through atomic ORs only
                            let words: &[std::sync::atomic::AtomicU64] = unsafe { std::slice::from_raw_parts(mp as *const std::sync::atomic::AtomicU64, n) };
                            for y in b.min[1]..b.max[1] { for x in b.min[0]..b.max[0] {
                                let p = crate::probepass::probe_point(x, y, z, pbr.offsets.as_ref());
                                let sh = crate::probepass::to_shadow(p, &dr.regs, pbr.opts.fma);
                                let (fx, fy) = ((sh[0] * w as f32 - 0.5).floor(), (sh[1] * h as f32 - 0.5).floor());
                                if fx.is_finite() && fy.is_finite() {
                                    for dy in -1..=2i64 { for dx in -1..=2i64 {
                                        let (px, py) = ((fx as i64 + dx).clamp(0, w - 1), (fy as i64 + dy).clamp(0, h - 1));
                                        let i = (py * w + px) as usize;
                                        words[i >> 6].fetch_or(1u64 << (i & 63), std::sync::atomic::Ordering::Relaxed);
                                    } }
                                }
                                // and the probe draws' point texel with its 3×3 neighbourhood (the sky visibility's 2×2 PCF footprint) —
                                // perf 8: this was a second, sequential loop over every probe after the census
                                let (tx, ty) = (crate::probepass::texel_point(sh[0], frame.res) as i64, crate::probepass::texel_point(sh[1], frame.res_y) as i64);
                                for dy in -1..=1i64 { for dx in -1..=1i64 {
                                    let (px, py) = (tx + dx, ty + dy);
                                    if px >= 0 && py >= 0 && px < w && py < h { let i = (py * w + px) as usize; words[i >> 6].fetch_or(1u64 << (i & 63), std::sync::atomic::Ordering::Relaxed); }
                                } }
                            } }
                        });
                        prof::add(&prof::PROBE_PX, t_probe_px);
                    }
                }
                // the CENSUS pixels for the layer-count rule's written fractions (every CENSUS_STEP-th pixel in x
                // and y), unless this peel's item-layer count is known (captured or fixed)
                let t_census = std::time::Instant::now();
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
                prof::add(&prof::W_CENSUS, t_census);
                prof::add(&prof::BITMAP, t_idx);
                // (the probe draws' 3×3 texel neighbourhoods were marked above, in the parallel pass)
                crate::pool::stats::stage("pixel-index");
                let t_pix = std::time::Instant::now();
                let px_new = PixelIndex::new(frame.res, frame.res_y, m);
                prof::add(&prof::W_INDEX, t_pix);
                Some(std::sync::Arc::new(px_new))
            } else { None };
            prof::add(&prof::B_INDEX, t_idx);
            // THE EXACT LAYER COUNT (default): with no captured or fixed count for this peel, a dense depth-only
            // build of the whole frame gives the written fraction of every item layer exactly, then the stop
            // rule; --layers-estimate takes the census estimate on the sparse build instead
            let t_exact = std::time::Instant::now(); crate::pool::stats::stage("exact");
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
                        let t_cull = std::time::Instant::now(); crate::pool::stats::stage("cull");
                        let ranges = if std::env::var_os("LMTOOL_NO_CULL").is_some() { vec![(0u32, bvh.tris.len() as u32)] } else { bvh.ranges_where(|lo, hi| frame.box_class(lo, hi, zmin_f, zmax_f)) };
                        if std::env::var_os("LMTOOL_CULL_DEBUG").is_some() {
                            let n: u32 = ranges.iter().map(|r| r.1 - r.0).sum();
                            // how many triangles project a vertex inside the frame (the ideal)
                            let inside = bvh.tris.iter().filter(|t| { let (x, y, z) = frame.project(t.p0); x >= 0.0 && x <= frame.res as f32 && y >= 0.0 && y <= frame.res_y as f32 && z >= zmin_f && z < zmax_f }).count();
                            eprintln!("cull: direction {di} peel {pi}: {} of {} triangles in {} BVH ranges (p0 inside the frame: {inside}); frame res {}×{} scale {:.4} s0 {:.1} t0 {:.1} zc {:.1} half_d {:.1}", n, bvh.tris.len(), ranges.len(), frame.res, frame.res_y, frame.scale, frame.s0, frame.t0, frame.zc, frame.half_d);
                        }
                        prof::add(&prof::CULL, t_cull);
                        // the frame's jobs from the instance hierarchy (its BVH ranges then only serve the check / the fallback)
                        let hier_jobs: Option<Vec<crate::insthier::GeomJob>> = insthier.as_ref().map(|h| {
                            let clip = (frame.inset_px as i32, frame.inset_px as i32, frame.res as i32 - 1 - frame.inset_px as i32, frame.res_y as i32 - 1 - frame.inset_px as i32);
                            let t_j = std::time::Instant::now();
                            let jobs = h.frame_jobs(scene, frame, zmin_f, zmax_f, clip, insthier_grain);
                            prof::add(&prof::CULL, t_j);
                            if insthier_check {
                                let (nj, ntj, held, bad, worst) = h.check_frame(scene, frame, zmin_f, zmax_f, clip, &jobs);
                                eprintln!("insthier: direction {di} peel {pi}: {nj} jobs holding {ntj} triangles (the frame holds {held}), VIOLATIONS {bad}, worst excursion {worst:.4} px (margin {})", crate::insthier::MARGIN_PX);
                            }
                            jobs
                        });
                        let (ab, counted) = build_abuffer_sparse_items(&bvh.tris, &ranges, insthier.as_ref().zip(hier_jobs.as_deref()), Some(&bvh.soa), frame, threads, zmin_f, zmax_f, &prm.alpha_masks, px, if fuse { Some(CountCtx { scene, bvh, prm }) } else { None });
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
            crate::pool::stats::checkpoint("(build tail)");
            frag_total += ab.len();
            // the game's layers of this peel (game-peel mode), and their dump
            // (the layers are also extracted for the dump alone, so the port's own gather can be dumped and compared)
            let tl = std::time::Instant::now(); crate::pool::stats::stage("layers");
            // the item-layer count: the captured one for this direction's peel when the harness has it, else the stop rule
            let fixed_layers: Option<usize> = if prm.layers_from_capture { prm.peel_layer_counts.as_ref().and_then(|c| c.get(di)).and_then(|v| v.get(pi)).copied().flatten() } else { prm.peel_layers_fixed };
            // the environment render's dome colour PER PIXEL for the transcribed accumulate's layer-0 colour target: the
            // dome as `dome_px` transcribes it (the game's dome mesh rasterised in this frame, PS 16774, the R11G11B10 target)
            // — the game's env render is per pixel, the uniform sky is only the gather's fallback
            // (the derivation takes the dome image in sweep 0 only — the bounce sweeps' layer 0 is black: not computed there)
            // (perf 8.18: no image any more — the derivation calls `dome_at` at the pixels whose environment layer is the dome;
            // dome_px quantises through the peel target and the ILightDir target — the same R11G11B10 twice)
            let dome_at = |x: u32, y: u32| -> [f32; 3] { dome_px(frame, dome_r, x, y) };
            let dome_img: Option<&(dyn Fn(u32, u32) -> [f32; 3] + Sync)> = if prm.lm_scene.is_some() && prm.sky_grad.is_some() && prm.dome_exact && prm.sweep == 0 { Some(&dome_at) } else { None };
            let fixed_layers = fixed_layers.or(exact_layers);
            let layers: Option<Layers> = if prm.game_peel || want_dir_dump { Some(extract_layers(&ab, frame, scene, bvh, prm, shadow.as_deref(), sun_bias, sky, threads, wanted.as_ref(), fixed_layers, dome_img)) } else { None };
            crate::pool::stats::checkpoint("(layers tail)");
            prof::add(&prof::LAYERS, tl);
            let td = std::time::Instant::now(); crate::pool::stats::stage("dump");
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
                let t_amb = std::time::Instant::now();
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
                prof::add(&prof::AMBIENT, t_amb);
            }
            // the transcribed LmILightDir_Set blocks over this peel's layers: block k reads layer k's colour + depth targets
            // (k = 0 the environment render; the clear 1.0 / black where a pixel has fewer layers); the fitted peel's blocks
            // clip to the items' world box (VS 17115)
            // the two consumers of this peel's layer targets: the transcribed accumulate (every peel) and the probes (the
            // world peel only) — the colour / depth Bufs of a layer are built once for both
            let want_probes = pi == 0 && prm.probe_bake.is_some();
            if let Some(ly) = layers.as_ref().filter(|_| prm.lm_scene.is_some() || want_probes) {
                // (the transcribed accumulate and the probes read the layers through `dense_start` and `start[p]..start[p + 1]`:
                // the in-place derive's gapped lists are compacted first — a copy this path pays, the raster paths do not)
                let ly_contig = ly.contiguous();
                let ly: &Layers = ly_contig.as_ref().unwrap_or(ly);
                let tlm = std::time::Instant::now(); crate::pool::stats::stage("lmaccum");
                // the layer count over the pixels present (`start` is per wanted pixel in the sparse form; the in-place
                // derive's lists are not contiguous — `range`)
                // (the deepest pixel's layer count, in parallel — perf 8.19: a serial scan of every pixel per peel was 0.1 s per giant direction)
                let nl = {
                    let n = ly.start.len() - 1;
                    let threads = crate::pool::pool().threads.max(1);
                    let chunk = (n / (threads * 4).max(1)).max(4096);
                    let maxes: Vec<usize> = crate::pool::pool().map((n + chunk - 1) / chunk, |ci| (ci * chunk..((ci + 1) * chunk).min(n)).map(|i| { let (a, c) = ly.range(i); c - a }).max().unwrap_or(0));
                    maxes.into_iter().max().unwrap_or(0).max(ly.max_layers.min(ly.item_layers + 1))
                };
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
                // LMTOOL_SET_PROBE=x,y: every LM fragment landing on that atlas texel prints its path through every block (the
                // interpolated position / normal, the projected peel uv / z, the layer depth it met, the verdict) — the block-by-block loop
                let set_probe: Option<(u32, u32)> = std::env::var("LMTOOL_SET_PROBE").ok().and_then(|v| { let p: Vec<u32> = v.split(',').filter_map(|t| t.trim().parse().ok()).collect(); if p.len() == 2 { Some((p[0], p[1])) } else { None } });
                let per_block = std::env::var_os("LMTOOL_SET_PER_BLOCK").is_some() || lm_draws.is_none() || set_probe.is_some();
                // THE LM RASTER ONCE (perf 8, lmaccum::replay_set_layers): the direction's blocks replay the fragment list of its
                // raster offset — the same fragments in the same order, no raster per block; LMTOOL_LMACCUM_FRAGLIST=0 keeps the
                // fused raster path (and the serial / per-block study switches take theirs)
                let replay = !per_block && crate::lmaccum::frag_list_on() && std::env::var_os("LMTOOL_LMACCUM_SERIAL").is_none();
                let frag_list = if replay { lm_draws.as_ref().map(|(lm, _)| lm.frag_list(di, 2048, 2048)) } else { None };
                // THE LAYERS READ IN PLACE (perf 8, lmaccum::LayerSparse): the replay and the probe passes read the layer table
                // through the frame's dense offsets — no per-layer 4096² colour / depth images (stpad: 13 layers × 268 MB per
                // direction, 0.4–0.7 s); the raster paths keep the images
                let in_place = frag_list.is_some() || lm_draws.is_none();
                let t_bufs = std::time::Instant::now();
                let dstart: Option<std::borrow::Cow<[u32]>> = if in_place { Some(ly.dense_start()) } else { None };
                prof::add(&prof::LM_BUFS, t_bufs);
                let mut k = 0usize;
                while k < nl {
                    let k_end = if per_block { k + 1 } else { (k + chunk).min(nl) };
                    if let Some(ds) = &dstart {
                        let layers: Vec<crate::lmaccum::LayerSparse> = (k..k_end).map(|kk| crate::lmaccum::LayerSparse { w: ly.w, h: ly.h, start: ds, frags: &ly.frags, k: kk }).collect();
                        let t_set = std::time::Instant::now();
                        if let (Some((lm, draws)), Some(dt), Some(fl)) = (&lm_draws, dir_lm.as_mut(), &frag_list) {
                            if !draws.is_empty() { crate::lmaccum::replay_set_layers(fl, lm, &draws[0].cb, draws[0].world_box, &layers, crate::lmaccum::DepthCompare::Float, dt); }
                        }
                        prof::add(&prof::LM_SET, t_set);
                        if want_probes {
                            let t_pl = std::time::Instant::now();
                            if let Some(pb) = &prm.probe_bake {
                                for (j, l) in layers.iter().enumerate() {
                                    pb.lock().unwrap().world_layer(k + j, &pw01, l, *d, prm.sphere_dirs.len(), prm.sweep == 0);
                                }
                            }
                            prof::add(&prof::PROBE_LAYER, t_pl);
                        }
                        k = k_end;
                        continue;
                    }
                    let t_bufs = std::time::Instant::now();
                    let bufs: Vec<(crate::passdiff::Buf, crate::passdiff::Buf)> = (k..k_end).map(|kk| (ly.colour_buf(kk, 0), crate::passdiff::Buf { w: ly.w, h: ly.h, channels: 1, data: ly.depth_image(kk, 0) })).collect();
                    prof::add(&prof::LM_BUFS, t_bufs);
                    let t_set = std::time::Instant::now();
                    if let (Some((lm, draws)), Some(dt)) = (&lm_draws, dir_lm.as_mut()) {
                        if per_block {
                            // the probe fires on the first two directions, or on the steep ones (D.y > 0.8) with LMTOOL_SET_PROBE_UP=1
                            let fire = set_probe.is_some() && (if std::env::var_os("LMTOOL_SET_PROBE_UP").is_some() { d[1] > 0.8 } else { di < 2 });
                            if fire { eprintln!("set probe: direction {di} peel {pi} block {k} (D {:.3},{:.3},{:.3})", d[0], d[1], d[2]); }
                            crate::lmaccum::run_set_block_probe(&lm.meshes, &lm.instances, &lm.table, draws, &crate::lmaccum::LayerTargets { color: &bufs[0].0, depth: &bufs[0].1 }, crate::lmaccum::DepthCompare::Float, dt, if fire { set_probe } else { None });
                        } else {
                            let layers: Vec<crate::lmaccum::LayerTargets> = bufs.iter().map(|(c, dd)| crate::lmaccum::LayerTargets { color: c, depth: dd }).collect();
                            crate::lmaccum::run_set_layers_par(&lm.meshes, &lm.instances, &lm.table, draws, &layers, crate::lmaccum::DepthCompare::Float, dt, &mut dir_best_k, dir_block_base + k);
                        }
                    }
                    prof::add(&prof::LM_SET, t_set);
                    // THE PROBES after every WORLD-peel layer (PS 17151 per block; the sky visibility after layer 1 and the
                    // AddAmbient dispatch after layer 0 for the sky sweep's upward directions) — probebake.rs
                    if want_probes {
                        let t_pl = std::time::Instant::now();
                        if let Some(pb) = &prm.probe_bake {
                            for (j, (color, depth)) in bufs.iter().enumerate() {
                                pb.lock().unwrap().world_layer(k + j, &pw01, &crate::lmaccum::LayerTargets { color, depth }, *d, prm.sphere_dirs.len(), prm.sweep == 0);
                            }
                        }
                        prof::add(&prof::PROBE_LAYER, t_pl);
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
                    let env_depth: Vec<f32> = (0..(ly.w * ly.h) as usize).map(|i| { let (a, c) = ly.range(i); if a < c { ly.frags[a].d } else { 0.0 } }).collect();
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
                let nl = (0..(ly.w * ly.h) as usize).map(|i| { let (a, c) = ly.range(i); c - a }).max().unwrap_or(0).saturating_sub(skip);
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
            let tg = std::time::Instant::now(); crate::pool::stats::stage("gather");
            // THE GATHER of this peel: per sub-sample the layer it reads (game mode: the game's lookup;
            // else the port's A-buffer walk), written into `sel` (last peel wins where it has a layer)
            let sel_ptr = sel.as_mut_ptr() as usize;
            let occl_ptr = occl.as_mut_ptr() as usize;
            let stamp_ptr = stamp.as_mut_ptr() as usize;
            // a tile peel walks its own sub-sample list (perf 8; the clip test below still guards every one of them)
            let range: &[u32] = match tile_subs.get(if jitter { di % 9 } else { 0 }).and_then(|v| v.get(pi)).and_then(|o| o.as_ref()) { Some(list) => list.as_slice(), None => range };
            let chunk = (range.len() / (threads.max(1) * 8)).max(1024);
            let n_chunks = (range.len() + chunk - 1) / chunk;
            crate::pool::pool().run(n_chunks, |ci| {
                let ch = &range[ci * chunk..((ci + 1) * chunk).min(range.len())];
                let (mut n_dome, mut n_surface) = (0u64, 0u64);
                let t_chunk = std::time::Instant::now();
                // THE GATHER PIPELINED (perf 8): the game-peel lookup is four dependent random reads per sub-sample (the
                // wanted bitmap's word and rank, the layer table's start, the fragment list — a cache miss each: 68 % of the
                // gather's samples sat on the binary search's first load), so the sub-samples go through in blocks of
                // 32 with each stage's reads prefetched a stage ahead. Every sub-sample gets the same computations on the
                // same inputs — only the order of independent memory accesses changes (the study print keeps the plain loop).
                let pipelined = prm.game_peel && layers.is_some() && game_dbg.is_none();
                if pipelined {
                    let ly = layers.as_ref().unwrap();
                    let frame = &frame;
                    let subs = cur;
                    let clip_box: Option<[f32; 4]> = tile_clip.get(pi).copied().flatten();
                    let (lw, lh) = (ly.w, ly.h);
                    #[inline(always)]
                    fn prefetch<T>(p: *const T) {
                        #[cfg(target_arch = "x86_64")]
                        unsafe { std::arch::x86_64::_mm_prefetch(p as *const i8, std::arch::x86_64::_MM_HINT_T0); }
                        #[cfg(not(target_arch = "x86_64"))]
                        let _ = p;
                    }
                    const B: usize = 32;
                    // per pending sub-sample: (index, pixel id, z01, the dense layer-table index, the fragment range)
                    let mut pend: [(u32, u32, f32, u32, u32, u32); B] = [(0, 0, 0.0, 0, 0, 0); B];
                    for blk in ch.chunks(B) {
                        // stage 1: the projection and the pixel; the bitmap word / rank (sparse) or the table start (dense) prefetched
                        let mut n = 0usize;
                        for &i in blk {
                            let s = &subs[i as usize];
                            if dot(s.n, *d) <= 0.0 { continue; }
                            if let Some(b) = clip_box {
                                if !(s.p[0] - b[0] >= 0.0 && s.p[2] - b[1] >= 0.0 && b[2] - s.p[0] >= 0.0 && b[3] - s.p[2] >= 0.0) { continue; }
                            }
                            let (x, y, z) = frame.project(s.p);
                            let (px, py) = (lookup_pixel(x, lw, prm.peel_inset), lookup_pixel(y, lh, prm.peel_inset));
                            let z01 = frame.z01(z);
                            if !(px < lw && py < lh && z01 >= 0.0 && z01 <= 1.0) { continue; }
                            let pix = py * lw + px;
                            match &ly.sparse {
                                Some(sp) => { prefetch(sp.words.as_ptr().wrapping_add((pix >> 6) as usize)); prefetch(sp.rank.as_ptr().wrapping_add((pix >> 6) as usize)); }
                                None => prefetch(ly.start.as_ptr().wrapping_add(pix as usize)),
                            }
                            pend[n] = (i, pix, z01, 0, 0, 0);
                            n += 1;
                        }
                        // stage 2: the dense index (sparse form), the table start prefetched
                        let mut m = 0usize;
                        for j in 0..n {
                            let (i, pix, z01, _, _, _) = pend[j];
                            let k = match &ly.sparse {
                                Some(sp) => match sp.index(pix % lw, pix / lw) { Some(k) => k, None => continue },
                                None => pix,
                            };
                            if ly.sparse.is_some() { prefetch(ly.start.as_ptr().wrapping_add(k as usize)); }
                            pend[m] = (i, pix, z01, k, 0, 0);
                            m += 1;
                        }
                        // stage 3: the fragment range, its first fragment prefetched
                        let mut q = 0usize;
                        for j in 0..m {
                            let (i, pix, z01, k, _, _) = pend[j];
                            // (through `range`: the in-place derive's lists are not contiguous across its chunks)
                            let (a, b) = { let (a, b) = ly.range(k as usize); (a as u32, b as u32) };
                            if a == b { continue; }
                            prefetch(ly.frags.as_ptr().wrapping_add(a as usize));
                            pend[q] = (i, pix, z01, k, a, b);
                            q += 1;
                        }
                        // stage 4: the game's layer selection, the colour, the store
                        for j in 0..q {
                            let (i, pix, z01, _, a, b) = pend[j];
                            let list = &ly.frags[a as usize..b as usize];
                            let hit: Option<([f32; 3], bool)> = match select_layer(list, z01) {
                                Some(f) if f.d > 0.0 || !prm.dome_layer => { n_surface += 1; Some((f.rgb, true)) }
                                Some(_) => { n_dome += 1; Some((dome_px(frame, dome_r, pix % lw, pix / lw), false)) }
                                None => None,
                            };
                            if let Some((l, occluded)) = hit {
                                // SAFETY: each chunk owns a disjoint set of indices i; no other thread touches sel[i] / occl[i]
                                let slot = unsafe { &mut *(sel_ptr as *mut [f32; 3]).add(i as usize) };
                                *slot = prm.quant_ilightdir.apply(l, prm.rounding);
                                let o = unsafe { &mut *(occl_ptr as *mut bool).add(i as usize) };
                                *o = occluded;
                                unsafe { *(stamp_ptr as *mut u32).add(i as usize) = gen; }
                            }
                        }
                    }
                } else {
                    let ab = &ab;
                    let frame = &frame;
                    let subs = cur;
                    let shadow = shadow.as_deref();
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
                                        Some(f) if f.d > 0.0 || !prm.dome_layer => { hit = Some((f.rgb, true)); n_surface += 1; }
                                        Some(_) => { hit = Some((dome_px(frame, dome_r, px, py), false)); n_dome += 1; } // the dome: the sky at this pixel
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
                                unsafe { *(stamp_ptr as *mut u32).add(i as usize) = gen; }
                            }
                        }
                    }
                }
                if n_dome + n_surface > 0 {
                    GATHER_COUNTS[0].fetch_add(n_dome, std::sync::atomic::Ordering::Relaxed);
                    GATHER_COUNTS[1].fetch_add(n_surface, std::sync::atomic::Ordering::Relaxed);
                }
                prof::add(&prof::GATHER_CPU, t_chunk);
            });
            prof::add(&prof::GATHER, tg);
        }
        } // (the live direction)
        let t_contrib = std::time::Instant::now(); crate::pool::stats::stage("contrib");
        if let (Some(tx), true) = (&contrib_tx, replay.is_none()) {
            // THE CONTRIBUTION of this direction: sel of the facing sub-samples, the occl bits, the probes
            let mut c = crate::contrib::DirContrib { sweep: prm.sweep, di: di as u32, n_subs: cur.len() as u32, ..Default::default() };
            c.occl_bits = vec![0u64; (cur.len() + 63) / 64];
            for (i, s) in cur.iter().enumerate() {
                let live = stamp[i] == gen;
                if dot(s.n, *d) > 0.0 { c.sel.push(if live { sel[i] } else { sky_fill }); }
                if live && occl[i] { c.occl_bits[i >> 6] |= 1u64 << (i & 63); }
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
        let ta = std::time::Instant::now(); crate::pool::stats::stage("accum");
        // THE PROBES: the direction's two folds (PS 1112) — the game issues them after the H-basis, the order is immaterial
        // (they read the direction's volume, which the fitted peel and the H-basis never touch)
        if let Some(pb) = &prm.probe_bake { pb.lock().unwrap().end_direction(*d, prm.sphere_dirs.len(), prm.sweep); }
        prof::add(&prof::PROBE_END, ta);
        let t_hb = std::time::Instant::now();
        // THE ACCUMULATE (LmLBumpILighting): E += 4/N · max(0, n·D) · TMapILightDir[texel]
        // the transcribed H-basis accumulate of this direction, and the comparison with the capture's banked buffers
        if let (Some(lm), Some(dt), Some(hb)) = (&prm.lm_scene, &dir_lm, hb_lm.as_mut()) {
            let n_full = prm.sphere_dirs.len().max(1) as f32;
            let cb = crate::lmaccum::HbCb { peel_dir: *d, inv_dir_count: 1.0 / n_full };
            let raster = crate::lmaccum::LmRasterCb::for_offset(di, 2048, 2048);
            let draws: Vec<crate::lmaccum::HbDraw> = (0..lm.meshes.len()).map(|m| crate::lmaccum::HbDraw { eid: 0, mesh: m, instance_first: lm.inst_first[m], instance_count: lm.inst_count[m], raster, cb }).collect();
            // (the per-pixel owner is for the comparison with the capture's banked MRTs only)
            let mut owner = if prm.hbasis_game.is_some() { vec![0u8; 2048 * 2048] } else { Vec::new() };
            let owner_ref = if owner.is_empty() { None } else { Some(&mut owner) };
            // THE LM RASTER ONCE (perf 8): the H-basis draw replays the direction's fragment list (the sequential fragment model,
            // the barycentric interpolation — the study switches keep the raster path)
            let replay_hb = crate::lmaccum::frag_list_on() && std::env::var_os("LMTOOL_LMACCUM_SERIAL").is_none() && crate::lmaccum::interp_mode() == 0 && crate::lmaccum::frag_model() == crate::lmaccum::FragModel::Sequential;
            if replay_hb {
                let fl = lm.frag_list(di, 2048, 2048);
                crate::lmaccum::replay_hbasis(&fl, lm, &cb, dt, hb, crate::sunpass::BlendModel::TruncSrcRoundSum, owner_ref);
            } else {
                crate::lmaccum::run_hbasis_probe(&lm.meshes, &lm.instances, &lm.table, &draws, dt, hb, crate::sunpass::BlendModel::TruncSrcRoundSum, owner_ref, None);
            }
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
                        // the SIGN of the residue per object: Σ ours / Σ game over C0's rgb where they differ (1 = no bias)
                        let (mut so, mut sg, mut nd, mut ours0, mut game0) = (0f64, 0f64, 0usize, 0usize, 0usize);
                        for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] != mi as u8 + 1 { continue; } for ch in 0..3 { let gg = gs[0].get(x, y, ch as u32); let o = hb.mrt[0][i][ch]; if gg != o { so += o as f64; sg += gg as f64; nd += 1; if o == 0.0 { ours0 += 1; } if gg == 0.0 { game0 += 1; } } } } }
                        if nd > 0 { per += &format!(" [{nm} C0 differing {nd}: Σours/Σgame {:.4}; ours 0 in {ours0}, game 0 in {game0}]", so / sg.max(1e-12)); }
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
        prof::add(&prof::HB, t_hb);
        let t_acc_sub = std::time::Instant::now();
        let acc_ptrs: Vec<usize> = acc_tex.iter_mut().map(|v| v.as_mut_ptr() as usize).collect();
        let cover_ptrs: Vec<usize> = cover.iter_mut().map(|v| v.as_mut_ptr() as usize).collect();
        let mut ldir: Vec<[f32; 3]> = if want_dir_dump { vec![[0.0; 3]; cur.len()] } else { Vec::new() };
        let ldir_ptr = ldir.as_mut_ptr() as usize;
        let ldir_on = want_dir_dump;
        // (eight tasks per thread: the facing fraction differs per chunk — perf 8)
        let chunk_acc = (range.len() / (threads * 8).max(1)).max(1024);
        let n_chunks = (range.len() + chunk_acc - 1) / chunk_acc;
        crate::pool::pool().run(n_chunks, |ci| {
            let ch = &range[ci * chunk_acc..((ci + 1) * chunk_acc).min(range.len())];
            let t_chunk = std::time::Instant::now();
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
                        // (the stamp: a peel wrote this sub-sample this direction; else the cleared sky and no occluder)
                        let live = stamp[i as usize] == gen;
                        let l = if live { sel[i as usize] } else { sky_fill };
                        // LMTOOL_SKY_NO_COS=1: the sky pass (AddSkyVisibility) without the receiver's cosine — the
                        // per-direction constant 4·w·d.y·SkyFactor times the visibility only (a hypothesis under test)
                        let hit_sky = !(live && occl[i as usize]);
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
            prof::add(&prof::ACC_CPU, t_chunk);
        });
        prof::add(&prof::ACC_SUB, t_acc_sub);
        prof::add(&prof::ACCUM, ta);
        let ab_len = frag_total;
        let tsn = std::time::Instant::now(); crate::pool::stats::stage("snap");
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
        // the direction's ILightDir target kept for the next direction
        dir_lm_buf = dir_lm.take();
        prof::add(&prof::DIR, t_dir);
    }
    // 5. resolve: per colour texel the mean over its covered sub-samples
    let t_resolve = std::time::Instant::now();
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
        prof::add(&prof::POST_RESOLVE, t_resolve);
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

#[cfg(all(test, target_arch = "x86_64", target_feature = "avx512f"))]
mod scan16_audit_tests {
    use super::*;

    /// The scalar walk of `count_run` (dome layer on, a 16-bit depth store), as the spec of `scan_block16`.
    fn scalar_walk(list: &mut Vec<CFrag>, env_d: f32, frame: &PeelFrame) -> usize {
        // under the biased order (the default) the reference is the biased scalar walk; the unbiased one below
        // is the former rule, checked when LMTOOL_LAYER_ORDER=unbiased
        if *BIASED_ORDER {
            return layer_walk_biased_cfrags(list, env_d, true, 16, frame);
        }
        list.sort_by(|p, q| p.z.total_cmp(&q.z).then_with(|| p.tri.cmp(&q.tri)));
        let mut d_prev = if env_d > 0.0 { (env_d * 65535.0).round() / 65535.0 } else { 0.0 };
        let mut items = 0usize;
        for f in list.iter() {
            let z01 = frame.z01(f.z).max(0.0);
            if z01 < d_prev {
                continue;
            }
            if items >= MAX_LAYERS {
                break;
            }
            let mut dd = z01 + f.bias;
            dd = (dd.clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
            items += 1;
            d_prev = dd;
        }
        items
    }

    #[test]
    fn the_record_layout_is_the_gathers_offsets() {
        assert_eq!(std::mem::size_of::<CFrag>(), 12);
        assert_eq!(std::mem::offset_of!(CFrag, z), 0);
        assert_eq!(std::mem::offset_of!(CFrag, tri), 4);
        assert_eq!(std::mem::offset_of!(CFrag, bias), 8);
    }

    /// Random pixels — zero to six fragments each, depths across and outside the frame's range, ±0.0 depths,
    /// equal depths on different triangles (the tie order), biases of both signs, environment depths absent
    /// and present: the lane walk's layer counts equal the scalar walk's on every lane it takes, and the
    /// lanes it hands to the scalar walk are exactly those with more than SCAN_K fragments.
    #[test]
    fn the_lane_walk_is_the_scalar_walk() {
        let frame = PeelFrame::new([0.0, -1.0, 0.0], [-10.0, -3.0, -10.0], [10.0, 7.0, 10.0], 64);
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let zs: [f32; 12] = [-3.0, -2.5, -1.0, -0.0, 0.0, 0.25, 1.0, 2.0, 3.0, 4.0, 4.999, 7.5];
        let biases: [f32; 5] = [0.0, 1.5e-5, -1.5e-5, 3e-4, 1.0 / 65535.0];
        for _round in 0..50_000 {
            let mut offs = [0u32; 17];
            let mut csr: Vec<CFrag> = Vec::new();
            let mut env = [0f32; 16];
            let mut lists: Vec<Vec<CFrag>> = Vec::new();
            for l in 0..16 {
                let n = match rnd() % 10 { 0 | 1 => 0, 2 | 3 => 1, 4 | 5 => 2, 6 => 3, 7 => 4, 8 => 5, _ => 6 };
                let mut list = Vec::new();
                for _ in 0..n {
                    let z = if rnd() % 3 == 0 { zs[(rnd() % 12) as usize] } else { (rnd() % 20000) as f32 / 1000.0 - 5.0 };
                    let tri = (rnd() % 6) as u32;
                    // (the bias is the TRIANGLE's term: equal (z, tri) records are identical records)
                    let bias = biases[(tri % 5) as usize];
                    list.push(CFrag { z, tri, bias });
                }
                // the records land in the scatter's (insertion) order, unsorted
                csr.extend_from_slice(&list);
                offs[l + 1] = csr.len() as u32;
                env[l] = match rnd() % 4 { 0 => 0.0, 1 => 0.5, 2 => (rnd() % 65535) as f32 / 65535.0, _ => ((rnd() % 100000) as f32 / 100000.0).max(1e-6) };
                lists.push(list);
            }
            let (hist, has, big) = scan_block16(&offs, &csr, &env, &frame);
            let mut want_hist = [0u32; SCAN_K + 1];
            let mut want_has = 0u32;
            let mut want_big = 0u16;
            for l in 0..16 {
                let n = lists[l].len();
                if n == 0 {
                    continue;
                }
                want_has += 1;
                if n > SCAN_K {
                    want_big |= 1 << l;
                    continue;
                }
                let items = scalar_walk(&mut lists[l], env[l], &frame);
                want_hist[items] += 1;
            }
            assert_eq!(has, want_has);
            assert_eq!(big, want_big);
            assert_eq!(hist, want_hist, "lists {:?} env {:?}", lists, env);
        }
    }
}

/// LMTOOL_NO_INPLACE=1: the sparse layer derivation's vector form even with a known layer count (the check's
/// reference); the force flag serves the in-process comparison.
pub static INPLACE_OFF: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("LMTOOL_NO_INPLACE").is_some());
pub static INPLACE_OFF_FORCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
mod biased_order_tests {
    use super::*;

    /// The GPU's layer formation, brute force: layer j+1 = the fragment with the smallest stored (biased) depth
    /// among those whose unbiased depth is not below layer j's stored depth (ties: the lower triangle index);
    /// the walk must equal it on random pixels.
    fn brute(frags: &[WalkFrag], d_start: f32) -> (usize, Vec<u32>) {
        let mut used = vec![false; frags.len()];
        let mut d_prev = d_start;
        let mut acc = Vec::new();
        loop {
            if acc.len() >= MAX_LAYERS { break; }
            let mut best: Option<usize> = None;
            for (i, f) in frags.iter().enumerate() {
                if used[i] || f.z01 < d_prev { continue; }
                match best {
                    None => best = Some(i),
                    Some(b) => { if (f.key, f.z01.to_bits()) < (frags[b].key, frags[b].z01.to_bits()) { best = Some(i); } }
                }
            }
            let Some(b) = best else { break };
            used[b] = true;
            d_prev = frags[b].dd(16);
            acc.push(frags[b].idx);
        }
        (acc.len(), acc)
    }

    #[test]
    fn the_biased_walk_is_the_gpus_layer_formation() {
        let mut seed = 0x1357_9bdf_2468_aceu64;
        let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
        for round in 0..30_000 {
            let n = 1 + (rnd() % 12) as usize;
            let frags: Vec<WalkFrag> = (0..n).map(|i| {
                let z01 = (rnd() % 65536) as f32 / 65535.0;
                let bias = match rnd() % 4 { 0 => 0.0, 1 => 8.0 / 65535.0, 2 => 100.0 / 65535.0, _ => (rnd() % 300) as f32 / 65535.0 };
                WalkFrag::from_depths(z01, bias, 16, i as u32 * 7 % 13, i as u32)
            }).collect();
            let d_start = if round % 3 == 0 { 0.0 } else { (rnd() % 65536) as f32 / 65535.0 };
            let (want, want_acc) = brute(&frags, d_start);
            let mut fr = frags.clone();
            let mut acc = Vec::new();
            let got = layer_walk_biased_from(&mut fr, d_start, 16, &mut acc);
            assert_eq!(got, want, "round {round}: {frags:?}");
            assert_eq!(acc, want_acc, "round {round}: the accepted order");
        }
    }
}

#[cfg(test)]
mod biased_walk_vs_gpu_rule_tests {
    //! E's layer rule as a pure function (2026-09-25 23:27 PT), held against engineer 2's biased walk with the DRAW
    //! RANK tie key and the item cap (0081/0082, df635375): count, stored-depth sequence AND accepted identity.
    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct F {
        z01: f32,
        q: f32,
        key: (u8, u32, u32),
    }

    /// E's WALK, literally: d_prev := d_env; repeat { C := { f ∉ taken : f.z01 ≥ d_prev }; stop when C is empty
    /// or |layers| = cap; f* := argmin over C of (q, key); push; d_prev := f*.q }.
    fn gpu_layers(frags: &[F], d_env: f32, cap: usize) -> Vec<usize> {
        let mut taken = vec![false; frags.len()];
        let mut d_prev = d_env;
        let mut layers = Vec::new();
        loop {
            if layers.len() >= cap { break; }
            let mut best: Option<usize> = None;
            for (i, f) in frags.iter().enumerate() {
                if taken[i] || f.z01 < d_prev { continue; }
                match best {
                    None => best = Some(i),
                    Some(b) => { let g = &frags[b]; if (f.q.to_bits(), f.key) < (g.q.to_bits(), g.key) { best = Some(i); } }
                }
            }
            let Some(b) = best else { break };
            taken[b] = true;
            layers.push(b);
            d_prev = frags[b].q;
        }
        layers
    }

    /// The draw rank of each fragment's key, as `draw_rank_table` ranks the scene's triangles: the position in the
    /// (class, inst, model tri) order — monotone in the key, so the walk's tie order is E's.
    fn ranks(frags: &[F]) -> Vec<u32> {
        let mut order: Vec<usize> = (0..frags.len()).collect();
        order.sort_by_key(|&i| frags[i].key);
        let mut rank = vec![0u32; frags.len()];
        for (r, &i) in order.iter().enumerate() { rank[i] = r as u32; }
        rank
    }

    #[test]
    fn the_biased_walk_is_the_gpu_rule_count_depths_and_identities() {
        let mut seed = 0xa5a5_1234_9876_5432u64;
        let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
        let d16 = |x: f32| -> f32 { (x.clamp(0.0, 1.0) * 65535.0).round() / 65535.0 };
        let mut pixels = 0usize;
        let mut capped = 0usize;
        for round in 0..60_000 {
            let n = match rnd() % 7 { 0 => 1, 1 => 2, 2 => 3, 3 => 5, 4 => 9, 5 => 26, _ => 48 };
            let mut frags: Vec<F> = Vec::new();
            let mut mtri = 100u32;
            while frags.len() < n {
                let z01 = (rnd() % 65536) as f32 / 65535.0 * 0.4 + 0.5;
                let kind = rnd() % 5;
                let steep = rnd() % 2 == 0;
                let bias = if steep { 100.0 / 65535.0 + (rnd() % 20) as f32 / 65535.0 } else { 8.0 / 65535.0 + (rnd() % 4) as f32 / 65535.0 };
                let class = if steep { 1u8 } else { 0u8 };
                let inst = (rnd() % 3) as u32;
                let q = d16(z01 + bias);
                match kind {
                    0 => {
                        // coincident copies: equal z01 and q, keys in either order relative to their creation
                        let (m0, m1) = if rnd() % 2 == 0 { (mtri, mtri + 1) } else { (mtri + 1, mtri) };
                        frags.push(F { z01, q, key: (class, inst, m0) });
                        frags.push(F { z01, q, key: (class, inst, m1) });
                    }
                    1 => {
                        // a q tie by rounding with different z01 and a different class
                        let z2 = z01 + 0.4 / 65535.0;
                        frags.push(F { z01, q, key: (class, inst, mtri) });
                        frags.push(F { z01: z2, q: d16(z2 + bias), key: (1 - class, inst, mtri + 1) });
                    }
                    2 => {
                        // the crossing: a steep card just nearer than a flat trunk
                        let zt = z01 + (rnd() % 30) as f32 / 65535.0;
                        frags.push(F { z01, q: d16(z01 + 110.0 / 65535.0), key: (1, inst, mtri) });
                        frags.push(F { z01: zt, q: d16(zt + 8.0 / 65535.0), key: (0, inst, mtri + 1) });
                    }
                    3 => {
                        // an exact q tie ACROSS classes at equal z01 (the opaque draw must win)
                        frags.push(F { z01, q, key: (1, inst, mtri) });
                        frags.push(F { z01, q, key: (0, inst, mtri + 1) });
                    }
                    _ => frags.push(F { z01, q, key: (class, inst, mtri) }),
                }
                mtri += 2;
            }
            frags.truncate(n.max(1));
            let dome = rnd() % 4 != 0;
            let d_env = if !dome { f32::NEG_INFINITY } else { match rnd() % 3 { 0 => 0.0, 1 => d16(0.55 + (rnd() % 1000) as f32 / 65535.0), _ => d16((rnd() % 65536) as f32 / 65535.0) } };
            let cap = item_cap(dome);
            let want = gpu_layers(&frags, d_env, cap);
            if want.len() == cap { capped += 1; }
            let rk = ranks(&frags);
            let mut wf: Vec<WalkFrag> = frags.iter().enumerate().map(|(i, f)| WalkFrag { key: (((f.q * 65535.0).round() as u64) << 32) | rk[i] as u64, z01: f.z01, idx: i as u32 }).collect();
            let mut acc: Vec<u32> = Vec::new();
            let got = layer_walk_biased_capped(&mut wf, d_env, cap, 16, &mut acc);
            assert_eq!(got, want.len(), "round {round}: count; frags {frags:?} d_env {d_env} cap {cap}");
            let want_idx: Vec<u32> = want.iter().map(|&i| i as u32).collect();
            assert_eq!(acc, want_idx, "round {round}: the accepted identities (and so the stored depths); frags {frags:?} d_env {d_env}");
            pixels += 1;
        }
        eprintln!("biased walk (draw rank, item cap) vs E's rule: {pixels} pixels identical in count, stored depths and accepted identity; {capped} pixels hit the cap");
    }
}
