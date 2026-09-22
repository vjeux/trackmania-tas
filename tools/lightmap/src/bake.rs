//! The baker: sky + sun (+ one bounce) irradiance per lightmap texel of every
//! item, from the map's own geometry (`geometry.rs`) through a BVH
//! (`bvh.rs`). Output is HDR per chart; `synth` packs and encodes it.

use crate::bvh::{Bvh, WTri};
use crate::geometry::{add, cross, dot, mul, norm, sub, xf_normal, xf_point, Scene, V3};

#[derive(Clone, Debug)]
pub struct BakeParams {
    /// Irradiance of an unoccluded, up-facing surface from the sky alone.
    pub sky: [f32; 3],
    /// A constant ambient floor added everywhere (the fitted fill of Nadeo's bakes).
    pub ambient: [f32; 3],
    /// An "upness" term × (0.5 + 0.5·n.y): the fitted zenith-weighted part of the sky.
    pub up: [f32; 3],
    /// Sun irradiance at normal incidence.
    pub sun: [f32; 3],
    /// Unit vector towards the sun.
    pub sun_dir: V3,
    /// Angular radius of the sun disc (radians) for soft shadows.
    pub sun_radius: f32,
    pub sky_samples: usize,
    pub sun_samples: usize,
    /// Layout texels (2048 space) per metre; Nadeo ≈ 1.1 on the tiny maps.
    pub texels_per_m: f32,
    /// Pixel cap per chart side (1024-space pixels).
    pub max_px: u32,
    pub min_px: u32,
    /// The sea / ground plane: rays going below it are blocked.
    pub ground_y: f32,
    /// One-bounce factor (0 = off) and the average albedo it uses.
    pub bounce: f32,
    pub albedo: f32,
    /// Flip the chart's V axis (uv v = 0 at the bottom row instead of the top).
    pub flip_v: bool,
    /// Map the model's uv1 BOUNDS (PreLightGen u04) to the chart instead of [0,1]².
    pub uv_bounds: bool,
    /// Sky model: 0 = uniform hemisphere, 1 = horizon-darkened (cos-weighted zenith brightening).
    pub sky_model: u32,
    pub threads: usize,
    /// Which third regressor the component fit uses: 0 bounce estimate, 1 upness (0.5+0.5·n.y), 2 skyVis².
    pub fit_regressor: u32,
    /// Pixels of chart border the uv square is inset by on each side (0 = the uv square fills the chart).
    pub inset_px: f32,
    /// Compute the bounce estimate even when `bounce` is 0 (for the component fit).
    pub want_bounce: bool,
    /// Debug: paint texels by world position (a 4 m checkerboard) instead of lighting.
    pub pattern: bool,
    /// Point-light scale for frame 1 (K = 1 units per unit light intensity); 0 = frame 1 not baked.
    pub light_k: f32,
}

impl Default for BakeParams {
    fn default() -> Self {
        BakeParams {
            sky: [0.407, 0.458, 0.546],
            ambient: [0.0; 3],
            up: [0.0; 3],
            sun: [1.9, 1.387, 0.399],
            sun_dir: norm([0.5, 0.6, 0.6]),
            sun_radius: 0.01,
            sky_samples: 64,
            sun_samples: 8,
            texels_per_m: 1.1,
            max_px: 96,
            min_px: 2,
            ground_y: -1.0e9,
            bounce: 0.0,
            albedo: 0.5,
            flip_v: false,
            uv_bounds: false,
            sky_model: 0,
            threads: 0,
            want_bounce: false,
            inset_px: 0.0,
            fit_regressor: 0,
            pattern: false,
            light_k: 0.27,
        }
    }
}

/// One item's baked chart: HDR irradiance per pixel, and which pixels hold
/// geometry (the rest are dilated copies).
#[derive(Clone, Debug)]
pub struct ChartBake {
    pub item: usize,
    pub w: u32,
    pub h: u32,
    pub rgb: Vec<[f32; 3]>,
    /// Frame 1: the point lights' irradiance per texel (empty when not baked).
    pub rgb1: Vec<[f32; 3]>,
    pub covered: Vec<bool>,
    /// Sun-visibility mean over the chart (diagnostics / fitting).
    pub sun_vis: f32,
    pub sky_vis: f32,
}

impl ChartBake {
    pub fn max_channel(&self) -> f32 {
        self.rgb.iter().flat_map(|c| c.iter().copied()).fold(0.0, f32::max)
    }
    pub fn mean(&self) -> [f32; 3] {
        let mut s = [0f32; 3];
        let mut n = 0f32;
        for (c, &cov) in self.rgb.iter().zip(self.covered.iter()) {
            if cov {
                for k in 0..3 {
                    s[k] += c[k];
                }
                n += 1.0;
            }
        }
        if n > 0.0 {
            for k in 0..3 {
                s[k] /= n;
            }
        }
        s
    }
}

/// World-space triangles of every instance, for the BVH.
pub fn world_tris(scene: &Scene) -> Vec<WTri> {
    let mut out = Vec::with_capacity(scene.tri_count());
    for (ii, inst) in scene.instances.iter().enumerate() {
        let m = &scene.models[inst.model];
        for t in &m.tris {
            let p0 = xf_point(&inst.xf, t.p[0]);
            let p1 = xf_point(&inst.xf, t.p[1]);
            let p2 = xf_point(&inst.xf, t.p[2]);
            out.push(WTri { p0, e1: sub(p1, p0), e2: sub(p2, p0), inst: ii as u32 });
        }
    }
    out
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// Orthonormal frame around `n`.
fn frame(n: V3) -> (V3, V3) {
    let a = if n[0].abs() > 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let t = norm(cross(a, n));
    let b = cross(n, t);
    (t, b)
}

/// A texel to shade: world position, shading normal, item.
#[derive(Clone, Copy)]
struct Sample {
    p: V3,
    n: V3,
    px: u32,
    py: u32,
}

/// Chart size in pixels (w, h). With a PreLightGen the game's own rule is
/// followed: layout texels = u02 × 0.5625 × uv extent (Nadeo: u02 32 → 18,
/// u02 104 × 0.326 → 20); `texels_per_m` scales that (1.0 = Nadeo density).
fn chart_size(m: &crate::geometry::ModelGeom, scale: f32, prm: &BakeParams) -> (u32, u32) {
    let (ew, eh) = match (prm.uv_bounds, m.plg_bounds) {
        (true, Some(b)) => ((b[2] - b[0]).clamp(0.05, 1.0), (b[3] - b[1]).clamp(0.05, 1.0)),
        _ => (1.0, 1.0),
    };
    let base = if m.plg_u02 > 0.0 { m.plg_u02 * 0.5625 } else { m.metres_per_uv * 1.1 };
    let tw = base * scale * prm.texels_per_m * ew;
    let th = base * scale * prm.texels_per_m * eh;
    let px = |t: f32| ((t / 2.0).ceil() as u32).clamp(prm.min_px, prm.max_px);
    (px(tw), px(th))
}

/// Rasterise one instance's uv1 triangles into a w×h chart; returns the
/// samples (one per covered pixel centre) plus the coverage mask.
fn rasterise_mode(scene: &Scene, ii: usize, w: u32, h: u32, flip_v: bool, use_bounds: bool) -> (Vec<Sample>, Vec<bool>) {
    rasterise_inset(scene, ii, w, h, flip_v, use_bounds, 0.0)
}

fn rasterise_inset(scene: &Scene, ii: usize, w: u32, h: u32, flip_v: bool, use_bounds: bool, inset: f32) -> (Vec<Sample>, Vec<bool>) {
    let inst = &scene.instances[ii];
    let m = &scene.models[inst.model];
    let (u0, v0, su, sv) = match (use_bounds, m.plg_bounds) {
        (true, Some(b)) => (b[0], b[1], 1.0 / (b[2] - b[0]), 1.0 / (b[3] - b[1])),
        _ => (0.0, 0.0, 1.0, 1.0),
    };
    let mut covered = vec![false; (w * h) as usize];
    let mut samples = Vec::new();
    let (fw, fh) = (w as f32, h as f32);
    let (iw, ih) = (fw - 2.0 * inset, fh - 2.0 * inset);
    for t in &m.tris {
        let uvp: Vec<[f32; 2]> = t
            .uv
            .iter()
            .map(|uv| {
                let u = (uv[0] - u0) * su;
                let v = (uv[1] - v0) * sv;
                let v = if flip_v { 1.0 - v } else { v };
                [inset + u * iw, inset + v * ih]
            })
            .collect();
        let minx = uvp.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
        let maxx = uvp.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil().min(fw) as i32;
        let miny = uvp.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
        let maxy = uvp.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil().min(fh) as i32;
        if minx >= maxx || miny >= maxy {
            continue;
        }
        let (a, b, c) = (uvp[0], uvp[1], uvp[2]);
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if det.abs() < 1e-12 {
            continue;
        }
        let wp = [xf_point(&inst.xf, t.p[0]), xf_point(&inst.xf, t.p[1]), xf_point(&inst.xf, t.p[2])];
        let wn = [xf_normal(&inst.xf, t.n[0]), xf_normal(&inst.xf, t.n[1]), xf_normal(&inst.xf, t.n[2])];
        // a triangle smaller than a pixel still owns the pixel its centroid sits in
        let cx = ((a[0] + b[0] + c[0]) / 3.0).floor().clamp(0.0, fw - 1.0) as i32;
        let cy = ((a[1] + b[1] + c[1]) / 3.0).floor().clamp(0.0, fh - 1.0) as i32;
        let mut any = false;
        for py in miny..maxy {
            for px in minx..maxx {
                let x = px as f32 + 0.5;
                let y = py as f32 + 0.5;
                let l1 = ((b[0] - x) * (c[1] - y) - (c[0] - x) * (b[1] - y)) / det;
                let l2 = ((c[0] - x) * (a[1] - y) - (a[0] - x) * (c[1] - y)) / det;
                let l3 = 1.0 - l1 - l2;
                let eps = -0.002;
                if l1 < eps || l2 < eps || l3 < eps {
                    continue;
                }
                let p = add(add(mul(wp[0], l1), mul(wp[1], l2)), mul(wp[2], l3));
                let n = norm(add(add(mul(wn[0], l1), mul(wn[1], l2)), mul(wn[2], l3)));
                let idx = (py as u32 * w + px as u32) as usize;
                if !covered[idx] {
                    covered[idx] = true;
                    samples.push(Sample { p, n, px: px as u32, py: py as u32 });
                }
                any = true;
            }
        }
        if !any {
            let idx = (cy as u32 * w + cx as u32) as usize;
            if !covered[idx] {
                covered[idx] = true;
                let p = mul(add(add(wp[0], wp[1]), wp[2]), 1.0 / 3.0);
                let n = norm(add(add(wn[0], wn[1]), wn[2]));
                samples.push(Sample { p, n, px: cx as u32, py: cy as u32 });
            }
        }
    }
    (samples, covered)
}

/// Shade one sample. Returns (irradiance, sky visibility, sun visibility).
/// Per-texel lighting components (the bounce estimate is per unit albedo·bounce).
pub struct Shaded {
    pub e: [f32; 3],
    pub sky_vis: f32,
    pub sun_vis: f32,
    pub bounce: [f32; 3],
}

fn shade(bvh: &Bvh, prm: &BakeParams, s: &Sample, ii: u32, rng: &mut Rng) -> ([f32; 3], f32, f32) {
    let r = shade_full(bvh, prm, s, ii, rng);
    (r.e, r.sky_vis, r.sun_vis)
}

fn shade_full(bvh: &Bvh, prm: &BakeParams, s: &Sample, ii: u32, rng: &mut Rng) -> Shaded {
    if prm.pattern {
        // 4 m checkerboard in x/z, hue by height band: continuous across items iff the uv mapping is right
        let c = ((s.p[0] / 4.0).floor() as i64 + (s.p[2] / 4.0).floor() as i64).rem_euclid(2);
        let band = ((s.p[1] / 4.0).floor() as i64).rem_euclid(3);
        let base = if c == 0 { 1.0 } else { 0.25 };
        let col = match band { 0 => [1.0, 0.3, 0.3], 1 => [0.3, 1.0, 0.3], _ => [0.3, 0.3, 1.0] };
        return Shaded { e: [col[0] * base, col[1] * base, col[2] * base], sky_vis: 1.0, sun_vis: 1.0, bounce: [0.0; 3] };
    }
    let o = add(s.p, mul(s.n, 0.03));
    let (t, b) = frame(s.n);
    let mut sky_vis = 0f32;
    let mut bounce = [0f32; 3];
    let nsky = prm.sky_samples.max(1);
    let rot = rng.next();
    // stratified cosine-weighted hemisphere
    let side = (nsky as f32).sqrt().ceil() as usize;
    let mut count = 0;
    for i in 0..side {
        for j in 0..side {
            if count >= nsky {
                break;
            }
            count += 1;
            let u = (i as f32 + rng.next()) / side as f32;
            let v = ((j as f32 + rng.next()) / side as f32 + rot).fract();
            let r = u.sqrt();
            let phi = 2.0 * std::f32::consts::PI * v;
            let (x, y) = (r * phi.cos(), r * phi.sin());
            let z = (1.0 - u).max(0.0).sqrt();
            let d = norm(add(add(mul(t, x), mul(b, y)), mul(s.n, z)));
            // the ground plane
            let mut tmax = 1.0e4f32;
            let mut ground_hit = false;
            if d[1] < 0.0 && o[1] > prm.ground_y {
                let tg = (prm.ground_y - o[1]) / d[1];
                if tg < tmax {
                    tmax = tg;
                    ground_hit = true;
                }
            }
            let blocked = bvh.occluded(o, d, tmax, ii, 0.05);
            if !blocked && !ground_hit {
                // sky radiance weight: uniform (cosine weighting is in the sampling)
                let wgt = if prm.sky_model == 1 { 0.5 + 0.5 * d[1].max(0.0) } else { 1.0 };
                sky_vis += wgt;
            } else if prm.bounce > 0.0 || prm.want_bounce {
                // one bounce: the hit surface's own direct sun + half sky, times albedo
                let hit = if ground_hit && !blocked { None } else { bvh.closest(o, d, tmax) };
                let (hp, hn) = match hit {
                    Some(h) => {
                        let tri = &bvh.tris[h.tri as usize];
                        let hn = norm(cross(tri.e1, tri.e2));
                        let hn = if dot(hn, d) > 0.0 { mul(hn, -1.0) } else { hn };
                        (add(o, mul(d, h.t)), hn)
                    }
                    None => (add(o, mul(d, tmax)), [0.0, 1.0, 0.0]),
                };
                let ho = add(hp, mul(hn, 0.03));
                let ndl = dot(hn, prm.sun_dir).max(0.0);
                let sun_v = if ndl > 0.0 && !bvh.occluded(ho, prm.sun_dir, 1.0e4, u32::MAX, 0.0) { 1.0 } else { 0.0 };
                let sky_half = 0.5 * (0.5 + 0.5 * hn[1]);
                for k in 0..3 {
                    bounce[k] += prm.sun[k] * ndl * sun_v + prm.sky[k] * sky_half;
                }
            }
        }
    }
    let sky_vis = sky_vis / count as f32;
    // sun
    let ndl = dot(s.n, prm.sun_dir);
    let mut sun_vis = 0f32;
    if ndl > 0.0 {
        let nsun = prm.sun_samples.max(1);
        let (st, sb) = frame(prm.sun_dir);
        for k in 0..nsun {
            let d = if k == 0 && nsun == 1 {
                prm.sun_dir
            } else {
                let r = prm.sun_radius * rng.next().sqrt();
                let phi = 2.0 * std::f32::consts::PI * rng.next();
                norm(add(prm.sun_dir, add(mul(st, r * phi.cos()), mul(sb, r * phi.sin()))))
            };
            if !bvh.occluded(o, d, 1.0e4, ii, 0.05) {
                sun_vis += 1.0;
            }
        }
        sun_vis /= nsun as f32;
    }
    let mut e = [0f32; 3];
    let bounce_n = [bounce[0] / count as f32, bounce[1] / count as f32, bounce[2] / count as f32];
    for k in 0..3 {
        e[k] = prm.ambient[k] + prm.up[k] * (0.5 + 0.5 * s.n[1]) + prm.sky[k] * sky_vis + prm.sun[k] * ndl.max(0.0) * sun_vis + prm.bounce * prm.albedo * bounce_n[k];
    }
    Shaded { e, sky_vis, sun_vis, bounce: bounce_n }
}

/// Bake every instance. Returns one chart per instance (index = instance).
/// `lights`: the scene's point lights (frame 1), used when `prm.light_k > 0`.
pub fn bake(scene: &Scene, bvh: &Bvh, prm: &BakeParams, lights: &[(usize, crate::geometry::LightDef)]) -> Vec<ChartBake> {
    let n = scene.instances.len();
    let threads = if prm.threads == 0 { std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160) } else { prm.threads };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<ChartBake>>> = (0..n).map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let ii = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if ii >= n {
                    break;
                }
                let inst = &scene.instances[ii];
                let m = &scene.models[inst.model];
                let scale = {
                    let c0 = [inst.xf[0], inst.xf[1], inst.xf[2]];
                    dot(c0, c0).sqrt()
                };
                let (w, h) = chart_size(m, scale, prm);
                let (samples, covered) = rasterise_inset(scene, ii, w, h, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rgb = vec![[0f32; 3]; (w * h) as usize];
                let want_lights = prm.light_k > 0.0 && !lights.is_empty();
                let mut rgb1 = if want_lights { vec![[0f32; 3]; (w * h) as usize] } else { Vec::new() };
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let (mut sv, mut kv) = (0f32, 0f32);
                for s in &samples {
                    let (e, sky_vis, sun_vis) = shade(bvh, prm, s, ii as u32, &mut rng);
                    rgb[(s.py * w + s.px) as usize] = e;
                    if want_lights {
                        let o = add(s.p, mul(s.n, 0.03));
                        rgb1[(s.py * w + s.px) as usize] = crate::probes::light_sum(bvh, lights, o, Some(s.n), prm.light_k, ii as u32);
                    }
                    sv += sun_vis;
                    kv += sky_vis;
                }
                let ns = samples.len().max(1) as f32;
                dilate(&mut rgb, &covered, w, h, prm.sky);
                if want_lights {
                    dilate(&mut rgb1, &covered, w, h, [0.0; 3]);
                }
                *results[ii].lock().unwrap() = Some(ChartBake { item: inst.item, w, h, rgb, rgb1, covered, sun_vis: sv / ns, sky_vis: kv / ns });
            });
        }
    });
    results.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect()
}

/// Flood-fill the uncovered texels of a chart from their covered neighbours
/// (repeated until full); anything still uncovered takes the covered mean
/// (or `fallback` on an empty chart).
fn dilate(rgb: &mut [[f32; 3]], covered: &[bool], w: u32, h: u32, fallback: [f32; 3]) {
    let mut cov = covered.to_vec();
    for _ in 0..256 {
        if cov.iter().all(|&c| c) {
            break;
        }
        let src = rgb.to_vec();
        let scov = cov.clone();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if scov[i] {
                    continue;
                }
                let mut acc = [0f32; 3];
                let mut cnt = 0;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = (ny as u32 * w + nx as u32) as usize;
                    if scov[j] {
                        for k in 0..3 {
                            acc[k] += src[j][k];
                        }
                        cnt += 1;
                    }
                }
                if cnt > 0 {
                    for k in 0..3 {
                        rgb[i][k] = acc[k] / cnt as f32;
                    }
                    cov[i] = true;
                }
            }
        }
    }
    let mut s = [0f32; 3];
    let mut c = 0f32;
    for (i, v) in rgb.iter().enumerate() {
        if cov[i] {
            for k in 0..3 {
                s[k] += v[k];
            }
            c += 1.0;
        }
    }
    let mean = if c > 0.0 { [s[0] / c, s[1] / c, s[2] / c] } else { fallback };
    for (i, v) in rgb.iter_mut().enumerate() {
        if !cov[i] {
            *v = mean;
        }
    }
}

/// Bake a SUBSET scene whose instances are a selection of a full scene's;
/// `full_ids[i]` is the full-scene instance id of `sub.instances[i]` (the BVH
/// was built over the full scene, so self-hit filtering needs those ids).
pub fn bake_subset(sub: &Scene, full_ids: &[u32], bvh: &Bvh, prm: &BakeParams) -> Vec<ChartBake> {
    let n = sub.instances.len();
    let threads = if prm.threads == 0 { std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160) } else { prm.threads };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<ChartBake>>> = (0..n).map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let ii = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if ii >= n {
                    break;
                }
                let inst = &sub.instances[ii];
                let m = &sub.models[inst.model];
                let scale = {
                    let c0 = [inst.xf[0], inst.xf[1], inst.xf[2]];
                    dot(c0, c0).sqrt()
                };
                let (pw0, ph0) = chart_size(m, scale, prm);
                let px = pw0.max(ph0).min(16);
                let (samples, covered) = rasterise_mode(sub, ii, px, px, prm.flip_v, prm.uv_bounds);
                let mut rgb = vec![[0f32; 3]; (px * px) as usize];
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let (mut sv, mut kv) = (0f32, 0f32);
                for s in &samples {
                    let (e, sky_vis, sun_vis) = shade(bvh, prm, s, full_ids[ii], &mut rng);
                    rgb[(s.py * px + s.px) as usize] = e;
                    sv += sun_vis;
                    kv += sky_vis;
                }
                let ns = samples.len().max(1) as f32;
                *results[ii].lock().unwrap() = Some(ChartBake { item: inst.item, w: px, h: px, rgb, rgb1: Vec::new(), covered, sun_vis: sv / ns, sky_vis: kv / ns });
            });
        }
    });
    results.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect()
}

/// Public rasteriser (positions/normals per covered pixel) for analysis tools.
pub struct PubSample {
    pub p: V3,
    pub n: V3,
    pub px: u32,
    pub py: u32,
}
pub fn rasterise_pub(scene: &Scene, ii: usize, w: u32, h: u32, flip_v: bool, use_bounds: bool) -> (Vec<PubSample>, Vec<bool>) {
    let (s, c) = rasterise_mode(scene, ii, w, h, flip_v, use_bounds);
    (s.into_iter().map(|x| PubSample { p: x.p, n: x.n, px: x.px, py: x.py }).collect(), c)
}

/// `bake_subset` at a fixed chart size.
pub fn bake_subset_px(sub: &Scene, full_ids: &[u32], bvh: &Bvh, prm: &BakeParams, w: u32, h: u32) -> Vec<ChartBake> {
    let mut out = Vec::new();
    for (ii, inst) in sub.instances.iter().enumerate() {
        let (samples, covered) = rasterise_mode(sub, ii, w, h, prm.flip_v, prm.uv_bounds);
        let mut rgb = vec![[0f32; 3]; (w * h) as usize];
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
        let (mut sv, mut kv) = (0f32, 0f32);
        for s in &samples {
            let (e, sky_vis, sun_vis) = shade(bvh, prm, s, full_ids[ii], &mut rng);
            rgb[(s.py * w + s.px) as usize] = e;
            sv += sun_vis;
            kv += sky_vis;
        }
        let ns = samples.len().max(1) as f32;
        out.push(ChartBake { item: inst.item, w, h, rgb, rgb1: Vec::new(), covered, sun_vis: sv / ns, sky_vis: kv / ns });
    }
    out
}

/// Per-texel Pearson correlation between our bake and a reference atlas for a
/// set of instances (`sel`, full-scene ids) whose charts are `(px, py, pw, ph)`
/// pixel rects in `atlas`. Returns the mean per-chart correlation and count.
pub fn texel_correlation(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32)], atlas: &crate::img::Rgb, prm: &BakeParams) -> (f64, usize) {
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new((0f64, 0usize));
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let (mut xs, mut ys) = (Vec::new(), Vec::new());
                for s in &samples {
                    let (e, _, _) = shade(bvh, prm, s, ii as u32, &mut rng);
                    let lum_m = 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
                    let n = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                    let lum_n = 0.2126 * n[0] as f32 + 0.7152 * n[1] as f32 + 0.0722 * n[2] as f32;
                    xs.push(lum_m as f64);
                    ys.push(lum_n as f64);
                }
                if xs.len() < 12 {
                    continue;
                }
                let n = xs.len() as f64;
                let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
                let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
                for (p, q) in xs.iter().zip(&ys) {
                    sxy += (p - mx) * (q - my);
                    sxx += (p - mx) * (p - mx);
                    syy += (q - my) * (q - my);
                }
                if sxx > 1e-9 && syy > 1e-9 {
                    let mut g = acc.lock().unwrap();
                    g.0 += sxy / (sxx * syy).sqrt();
                    g.1 += 1;
                }
            });
        }
    });
    let (s, n) = acc.into_inner().unwrap();
    (s / n.max(1) as f64, n)
}

/// Shadow agreement: on charts whose reference luminance is bimodal (lit and
/// shadowed texels), the fraction of texels whose predicted sun visibility
/// (one ray towards `prm.sun_dir`) agrees with the reference's lit/shadowed
/// label. Returns (agreement, texels used). Only charts with a real
/// dark/bright split count; charts without shadows say nothing about the sun.
pub fn shadow_agreement(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32)], atlas: &crate::img::Rgb, prm: &BakeParams) -> (f64, usize) {
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new((0usize, 0usize));
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                if samples.len() < 24 {
                    continue;
                }
                let mut lums: Vec<f32> = samples
                    .iter()
                    .map(|s| {
                        let n = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                        0.2126 * n[0] as f32 + 0.7152 * n[1] as f32 + 0.0722 * n[2] as f32
                    })
                    .collect();
                let mut sorted = lums.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let (lo, hi) = (sorted[sorted.len() / 5], sorted[sorted.len() * 4 / 5]);
                if hi - lo < 40.0 {
                    continue; // no shadow split on this chart
                }
                let thr = 0.5 * (lo + hi);
                let (mut agree, mut n) = (0usize, 0usize);
                for (s, l) in samples.iter().zip(lums.drain(..)) {
                    let o = add(s.p, mul(s.n, 0.03));
                    let facing = dot(s.n, prm.sun_dir) > 0.05;
                    let lit = facing && !bvh.occluded(o, prm.sun_dir, 1.0e4, ii as u32, 0.05);
                    if lit == (l >= thr) {
                        agree += 1;
                    }
                    n += 1;
                }
                let mut g = acc.lock().unwrap();
                g.0 += agree;
                g.1 += n;
            });
        }
    });
    let (a, n) = acc.into_inner().unwrap();
    (a as f64 / n.max(1) as f64, n)
}

/// Pooled (all texels of all selected charts) comparison: our HDR luminance
/// vs the reference atlas luminance × its chart scale. Returns (pearson,
/// slope reference/ours, count). `sel` = (inst, px, py, pw, ph, fb0).
pub fn pooled_fit(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32, u8)], atlas: &crate::img::Rgb, prm: &BakeParams) -> (f64, f64, usize) {
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new(Vec::<(f64, f64)>::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph, fb0) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let mut local = Vec::with_capacity(samples.len());
                for s in &samples {
                    let (e, _, _) = shade(bvh, prm, s, ii as u32, &mut rng);
                    let lum_m = 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
                    let n = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                    let lum_n = (0.2126 * n[0] as f32 + 0.7152 * n[1] as f32 + 0.0722 * n[2] as f32) / 255.0 * fb0 as f32 / 255.0;
                    local.push((lum_m as f64, lum_n as f64));
                }
                acc.lock().unwrap().extend(local);
            });
        }
    });
    let v = acc.into_inner().unwrap();
    let n = v.len() as f64;
    if n < 3.0 {
        return (0.0, 0.0, 0);
    }
    let (mx, my) = (v.iter().map(|p| p.0).sum::<f64>() / n, v.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (p, q) in &v {
        sxy += (p - mx) * (q - my);
        sxx += (p - mx) * (p - mx);
        syy += (q - my) * (q - my);
    }
    (sxy / (sxx * syy).sqrt().max(1e-12), sxy / sxx.max(1e-12), v.len())
}

/// Least-squares fit of the reference luminance against our per-texel
/// components: ref ≈ a·skyVis + b·(N·L·sunVis) + c. Returns (a, b, c, r²).
pub fn component_fit(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32, u8)], atlas: &crate::img::Rgb, prm: &BakeParams) -> (f64, f64, f64, f64) {
    let (c, r2) = component_fit_rgb(scene, bvh, sel, atlas, prm);
    let lum = |k: usize| 0.2126 * c[0][k] + 0.7152 * c[1][k] + 0.0722 * c[2][k];
    (lum(0), lum(1), lum(3), r2)
}

/// Per-channel least squares: ref_c ≈ a_c·skyVis + b_c·(N·L·sunVis) + k_c. Returns ([r,g,b] × [a,b,k], mean r²).
pub fn component_fit_rgb(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32, u8)], atlas: &crate::img::Rgb, prm: &BakeParams) -> ([[f64; 4]; 3], f64) {
    let mut prm = prm.clone();
    prm.want_bounce = true;
    let prm = &prm;
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new(Vec::<[f64; 6]>::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph, fb0) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let mut local = Vec::with_capacity(samples.len());
                for s in &samples {
                    let sh = shade_full(bvh, prm, s, ii as u32, &mut rng);
                    let ndl = dot(s.n, prm.sun_dir).max(0.0);
                    let n = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                    let sc = fb0 as f64 / 255.0 / 255.0;
                    let bl = if prm.fit_regressor == 1 { (0.5 + 0.5 * s.n[1]) as f64 } else if prm.fit_regressor == 2 { (sh.sky_vis * sh.sky_vis) as f64 } else { (0.2126 * sh.bounce[0] + 0.7152 * sh.bounce[1] + 0.0722 * sh.bounce[2]) as f64 };
                    local.push([sh.sky_vis as f64, (ndl * sh.sun_vis) as f64, bl, n[0] as f64 * sc, n[1] as f64 * sc, n[2] as f64 * sc]);
                }
                acc.lock().unwrap().extend(local);
            });
        }
    });
    let v = acc.into_inner().unwrap();
    let mut out = [[0f64; 4]; 3];
    let mut r2sum = 0.0;
    for ch in 0..3 {
        let mut m = [[0f64; 4]; 4];
        let mut r = [0f64; 4];
        for p in &v {
            let x = [p[0], p[1], p[2], 1.0];
            for i in 0..4 {
                for j in 0..4 {
                    m[i][j] += x[i] * x[j];
                }
                r[i] += x[i] * p[3 + ch];
            }
        }
        let Some(coef) = solve4(m, r) else { continue };
        let mean = v.iter().map(|p| p[3 + ch]).sum::<f64>() / v.len() as f64;
        let (mut ss_res, mut ss_tot) = (0.0, 0.0);
        for p in &v {
            let pred = coef[0] * p[0] + coef[1] * p[1] + coef[2] * p[2] + coef[3];
            ss_res += (p[3 + ch] - pred) * (p[3 + ch] - pred);
            ss_tot += (p[3 + ch] - mean) * (p[3 + ch] - mean);
        }
        out[ch] = coef;
        r2sum += 1.0 - ss_res / ss_tot.max(1e-12);
    }
    (out, r2sum / 3.0)
}

fn solve4(mut m: [[f64; 4]; 4], mut r: [f64; 4]) -> Option<[f64; 4]> {
    for c in 0..4 {
        let mut p = c;
        for i in c + 1..4 {
            if m[i][c].abs() > m[p][c].abs() {
                p = i;
            }
        }
        if m[p][c].abs() < 1e-12 {
            return None;
        }
        m.swap(c, p);
        r.swap(c, p);
        for i in 0..4 {
            if i == c {
                continue;
            }
            let f = m[i][c] / m[c][c];
            for j in 0..4 {
                m[i][j] -= f * m[c][j];
            }
            r[i] -= f * r[c];
        }
    }
    Some([r[0] / m[0][0], r[1] / m[1][1], r[2] / m[2][2], r[3] / m[3][3]])
}
