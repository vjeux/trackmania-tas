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
use crate::geometry::{cross, dot, norm, sub, Scene, V3, DECOR_INST};
use crate::raster;

/// The peel's orthographic frame: pixel (x, y) ↔ (p·r, p·u) scaled into `res` pixels over the scene's
/// projected bounds; depth = −p·d (the camera sits at +d∞ looking along −d, nearer = smaller).
#[derive(Clone, Debug)]
pub struct PeelFrame {
    pub d: V3,
    pub r: V3,
    pub u: V3,
    pub s0: f32,
    pub t0: f32,
    /// Pixels per metre.
    pub scale: f32,
    pub res: u32,
}

impl PeelFrame {
    pub fn new(d: V3, bmin: V3, bmax: V3, res: u32) -> PeelFrame {
        let d = norm(d);
        let helper = if d[1].abs() < 0.99 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let r = norm(cross(helper, d));
        let u = norm(cross(d, r));
        let (mut smin, mut smax, mut tmin, mut tmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for i in 0..8 {
            let p = [if i & 1 == 0 { bmin[0] } else { bmax[0] }, if i & 2 == 0 { bmin[1] } else { bmax[1] }, if i & 4 == 0 { bmin[2] } else { bmax[2] }];
            let (s, t) = (dot(p, r), dot(p, u));
            smin = smin.min(s);
            smax = smax.max(s);
            tmin = tmin.min(t);
            tmax = tmax.max(t);
        }
        let extent = (smax - smin).max(tmax - tmin).max(1e-3);
        // a half-pixel margin so the bounds' own points land inside
        let scale = (res as f32 - 1.0) / extent;
        PeelFrame { d, r, u, s0: smin - 0.5 / scale, t0: tmin - 0.5 / scale, scale, res }
    }
    /// Pixel-space x, y and the depth of a world point.
    #[inline]
    pub fn project(&self, p: V3) -> (f32, f32, f32) {
        ((dot(p, self.r) - self.s0) * self.scale, (dot(p, self.u) - self.t0) * self.scale, -dot(p, self.d))
    }
    /// The world size of one peel pixel.
    pub fn pixel_m(&self) -> f32 {
        1.0 / self.scale
    }
}

/// One fragment of the A-buffer: depth and the world triangle (index into the BVH's triangle list).
#[derive(Clone, Copy, Debug)]
pub struct Frag {
    pub z: f32,
    pub tri: u32,
}

/// All fragments of a peel, CSR by pixel, sorted by depth within a pixel.
pub struct ABuffer {
    pub res: u32,
    pub start: Vec<u32>,
    pub frags: Vec<Frag>,
}

impl ABuffer {
    /// The fragments of pixel (x, y), nearest first.
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> &[Frag] {
        let i = (y * self.res + x) as usize;
        &self.frags[self.start[i] as usize..self.start[i + 1] as usize]
    }
}

/// Raster every world triangle into the frame's A-buffer.
pub fn build_abuffer(tris: &[WTri], frame: &PeelFrame, threads: usize) -> ABuffer {
    let res = frame.res;
    let chunk = (tris.len() / threads.max(1)).max(4096);
    let parts: Vec<Vec<(u32, Frag)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = tris
            .chunks(chunk)
            .enumerate()
            .map(|(ci, ch)| {
                let frame = frame.clone();
                sc.spawn(move || {
                    let mut out: Vec<(u32, Frag)> = Vec::new();
                    for (k, t) in ch.iter().enumerate() {
                        let ti = (ci * chunk + k) as u32;
                        let p0 = t.p0;
                        let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                        let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                        let (x0, y0, z0) = frame.project(p0);
                        let (x1, y1, z1) = frame.project(p1);
                        let (x2, y2, z2) = frame.project(p2);
                        raster::triangle(res, res, [[x0, y0], [x1, y1], [x2, y2]], |x, y, b| {
                            let z = z0 * b[0] + z1 * b[1] + z2 * b[2];
                            out.push((y * res + x, Frag { z, tri: ti }));
                        });
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    // counting sort by pixel
    let n = (res * res) as usize;
    let mut count = vec![0u32; n + 1];
    for part in &parts {
        for (px, _) in part {
            count[*px as usize + 1] += 1;
        }
    }
    for i in 0..n {
        count[i + 1] += count[i];
    }
    let total = count[n] as usize;
    let mut frags = vec![Frag { z: 0.0, tri: 0 }; total];
    let mut fill = count.clone();
    for part in &parts {
        for (px, f) in part {
            let i = fill[*px as usize] as usize;
            frags[i] = *f;
            fill[*px as usize] += 1;
        }
    }
    // depth order within a pixel (nearest first)
    for i in 0..n {
        let (a, b) = (count[i] as usize, count[i + 1] as usize);
        if b - a > 1 {
            frags[a..b].sort_by(|p, q| p.z.partial_cmp(&q.z).unwrap_or(std::cmp::Ordering::Equal));
        }
    }
    ABuffer { res, start: count, frags }
}

/// A depth-only orthographic raster along the sun (the direct pass' shadow map).
pub struct ShadowMap {
    pub frame: PeelFrame,
    pub depth: raster::Depth,
}

impl ShadowMap {
    pub fn build(tris: &[WTri], sun_dir: V3, bmin: V3, bmax: V3, res: u32) -> ShadowMap {
        let frame = PeelFrame::new(sun_dir, bmin, bmax, res);
        let mut depth = raster::Depth::new(res, res);
        for t in tris {
            let p0 = t.p0;
            let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
            let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
            let (x0, y0, z0) = frame.project(p0);
            let (x1, y1, z1) = frame.project(p1);
            let (x2, y2, z2) = frame.project(p2);
            raster::triangle(res, res, [[x0, y0], [x1, y1], [x2, y2]], |x, y, b| {
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
        if xi < 0 || yi < 0 || xi >= self.frame.res as i64 || yi >= self.frame.res as i64 {
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
fn fragment_radiance(scene: &Scene, bvh: &Bvh, prm: &BakeParams, shadow: Option<&ShadowMap>, tri: u32, d: V3, hit_p: V3, sun_bias: f32) -> [f32; 3] {
    let wt = &bvh.tris[tri as usize];
    let ng = norm(cross(wt.e1, wt.e2));
    // The face that counts is the one turned TOWARD the receiving texel, i.e. whose normal points
    // against D (the light travels along −D from the occluder to the texel). A pad under a plate sees
    // the plate's underside — unlit by the sun and by the sky, hence black on the first sweep — which
    // is what the editor's bakes show (a pad under an 8 m plate keeps exactly its sky visibility, 51 %
    // of open, at Day); the face turned away from the texel is a back face → black.
    let facing = -dot(ng, d);
    let n = if facing >= 0.0 { ng } else { [-ng[0], -ng[1], -ng[2]] };
    let is_front = if wt.inst == DECOR_INST {
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
        _ => [0.0; 3],
    };
    let ndl = dot(n, prm.sun_dir).max(0.0);
    let lit = if ndl > 0.0 && prm.sun_dir[1] > 0.0 { shadow.map(|s| s.lit(hit_p, sun_bias)).unwrap_or(1.0) } else { 0.0 };
    let mut out = [0f32; 3];
    for k in 0..3 {
        out[k] = alb[k] * (stored[k] + prm.sun[k] * ndl * lit);
    }
    out
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
}

/// The whole dome sweep, rasterised. `sizes[ii]` = the colour-resolution chart size of instance `ii`
/// (the layout rect is twice that; the raster runs at `prm.ss` sub-samples per layout texel).
pub fn bake_peel_raster(scene: &Scene, bvh: &Bvh, prm: &BakeParams, sizes: &[(u32, u32)]) -> Vec<ChartBake> {
    let t0 = std::time::Instant::now();
    let threads = if prm.threads == 0 { std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160) } else { prm.threads };
    let ss = prm.ss.max(1);
    let groups = (ss * ss) as usize;
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
    for (ii, _inst) in scene.instances.iter().enumerate() {
        let (cw, ch) = sizes[ii];
        chart_meta.push((cw, ch));
        let (lw, lh) = (cw * 2, ch * 2);
        let r = crate::chartraster::raster_chart(scene, ii, lw, lh, ss, prm.flip_v, prm.uv_bounds);
        if std::env::var_os("LMTOOL_PEEL_DEBUG").is_some() && ii < 3 {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for s in &r.subs { for k in 0..3 { lo[k] = lo[k].min(s.p[k]); hi[k] = hi[k].max(s.p[k]); } }
            let up = r.subs.iter().filter(|s| s.n[1] > 0.9).count();
            let side = r.subs.iter().filter(|s| s.n[1].abs() < 0.1).count();
            eprintln!("peel: chart {ii} ({}) {lw}×{lh}: {} subs, bbox {:?}..{:?}, {up} up-facing, {side} side-facing", _inst.model_name, r.subs.len(), lo, hi);
            let (old, _) = crate::bake::rasterise_pub(scene, ii, lw, lh, prm.flip_v, prm.uv_bounds);
            let old_side = old.iter().filter(|s| s.n[1].abs() < 0.1).count();
            let old_north = old.iter().filter(|s| s.n[2] > 0.9).count();
            let new_north = r.subs.iter().filter(|s| s.n[2] > 0.9).count();
            eprintln!("    old rasteriser at {lw}×{lh}: {} samples, {old_side} side-facing, {old_north} north-facing (new: {new_north} north-facing sub-samples)", old.len());
            // a few side-facing samples with their normals, to aim the probe
            for s in r.subs.iter().filter(|s| s.n[2] > 0.9).step_by(20000).take(8) { eprintln!("    north sample p ({:.1},{:.1},{:.1}) n ({:.2},{:.2},{:.2})", s.p[0], s.p[1], s.p[2], s.n[0], s.n[1], s.n[2]); }
        }
        if r.subs.is_empty() && std::env::var_os("LMTOOL_PEEL_DEBUG").is_some() {
            let m = &scene.models[_inst.model];
            eprintln!("peel: chart {ii} ({}) {lw}×{lh} has NO sub-samples: {} tris, plg_bounds {:?}, uv {:?}..{:?}", _inst.model_name, m.tris.len(), m.plg_bounds, m.uv_min, m.uv_max);
        }
        for s in &r.subs {
            let (tx, ty) = (s.sx / ss / 2, s.sy / ss / 2);
            let group = ((s.sy % ss) * ss + s.sx % ss) as u8;
            subs.push(SubSample { p: s.p, n: s.n, own_tri: tri_base[ii] + s.tri, group, chart: ii as u32, texel: ty.min(ch - 1) * cw + tx.min(cw - 1) });
        }
    }
    eprintln!("peel: {} sub-samples over {} charts (ss {ss}, {} groups) ({:.1}s)", subs.len(), scene.instances.len(), groups, t0.elapsed().as_secs_f32());
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
    let mut acc: Vec<[f32; 3]> = vec![[0.0; 3]; subs.len()];
    // 3. the sun shadow map for the fragment radiance
    let (bmin, bmax) = scene_bounds(&bvh.tris);
    let shadow = if prm.sun_dir[1] > 0.0 && prm.sun.iter().any(|c| *c > 0.0) { Some(ShadowMap::build(&bvh.tris, prm.sun_dir, bmin, bmax, prm.peel_res.max(1024))) } else { None };
    let sun_bias = 2.5 * (bmax[0] - bmin[0]).max(bmax[2] - bmin[2]) / prm.peel_res.max(1024) as f32 + 0.05;
    // 4. the directions, interleaved into the groups
    let dirs: Vec<V3> = prm.sphere_dirs.iter().copied().collect();
    let n_dirs = dirs.len().max(1);
    let mut group_count = vec![0usize; groups];
    for (di, _) in dirs.iter().enumerate() {
        group_count[di % groups] += 1;
    }
    let bias_m = prm.peel_bias;
    // LMTOOL_PEEL_DEBUG=x,y,z,r: trace the gather of the sub-samples within r of a world point
    let dbg: Option<(V3, f32)> = std::env::var("LMTOOL_PEEL_DEBUG").ok().and_then(|s| { let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 4 { Some(([v[0], v[1], v[2]], v[3])) } else { None } });
    let mut dbg_subs: Vec<u32> = match dbg { Some((c, r)) => subs.iter().enumerate().filter(|(_, s)| { let e = sub(s.p, c); dot(e, e) < r * r }).map(|(i, _)| i as u32).take(3).collect(), None => Vec::new() };
    // LMTOOL_PEEL_DEBUG_SIDE=1: instead, three side-facing sub-samples of chart 0 spread over its height
    if std::env::var_os("LMTOOL_PEEL_DEBUG_SIDE").is_some() {
        dbg_subs = subs.iter().enumerate().filter(|(_, s)| s.chart == 0 && s.n[1].abs() < 0.3).step_by(70000).map(|(i, _)| i as u32).take(3).collect();
    }
    if let Some((c, _)) = dbg { eprintln!("peel debug: {} sub-samples near {:?}: {:?}", dbg_subs.len(), c, dbg_subs.iter().map(|&i| (subs[i as usize].p, subs[i as usize].n, subs[i as usize].group)).collect::<Vec<_>>()); }
    let dbg_printed = std::sync::atomic::AtomicUsize::new(0);
    for (di, d) in dirs.iter().enumerate() {
        let g = di % groups;
        let scale = 4.0 / group_count[g].max(1) as f32;
        let frame = PeelFrame::new(*d, bmin, bmax, prm.peel_res);
        let ab = build_abuffer(&bvh.tris, &frame, threads);
        let sky = sky_radiance(prm, *d);
        let bias = bias_m.max(0.5 * frame.pixel_m());
        let range = &order[group_start[g]..group_start[g + 1]];
        let chunk = (range.len() / threads.max(1)).max(1024);
        // the gather writes acc[i] for i in its own range only
        if !dbg_subs.is_empty() && di < 40 {
            if let Ok(k) = std::env::var("LMTOOL_PEEL_DEBUG_ITEM") {
                let k: u32 = k.parse().unwrap_or(0);
                let n = ab.frags.iter().filter(|f| bvh.tris[f.tri as usize].inst == k).count();
                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                let mut ntri = 0;
                for t in bvh.tris.iter().filter(|t| t.inst == k) {
                    for p in [t.p0, [t.p0[0] + t.e1[0], t.p0[1] + t.e1[1], t.p0[2] + t.e1[2]], [t.p0[0] + t.e2[0], t.p0[1] + t.e2[1], t.p0[2] + t.e2[2]]] { let (x, y, _) = frame.project(p); lo[0] = lo[0].min(x); lo[1] = lo[1].min(y); hi[0] = hi[0].max(x); hi[1] = hi[1].max(y); }
                    ntri += 1;
                }
                let (mut wlo, mut whi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for t in bvh.tris.iter().filter(|t| t.inst == k) { for q in [t.p0, [t.p0[0] + t.e1[0], t.p0[1] + t.e1[1], t.p0[2] + t.e1[2]], [t.p0[0] + t.e2[0], t.p0[1] + t.e2[1], t.p0[2] + t.e2[2]]] { for c in 0..3 { wlo[c] = wlo[c].min(q[c]); whi[c] = whi[c].max(q[c]); } } }
                eprintln!("  dir {di}: instance {k} has {ntri} tris, world {:?}..{:?}, projecting to x {:.1}..{:.1} y {:.1}..{:.1}, {n} fragments in the A-buffer (res {}); frame r ({:.2},{:.2},{:.2}) u ({:.2},{:.2},{:.2}) s0 {:.1} t0 {:.1} scale {:.3}", wlo, whi, lo[0], hi[0], lo[1], hi[1], frame.res, frame.r[0], frame.r[1], frame.r[2], frame.u[0], frame.u[1], frame.u[2], frame.s0, frame.t0, frame.scale);
            }
        }
        if !dbg_subs.is_empty() && dbg_printed.load(std::sync::atomic::Ordering::Relaxed) < 40 {
            for &i in &dbg_subs {
                let s = &subs[i as usize];
                if s.group as usize != g { continue; }
                let ndd = dot(s.n, *d);
                if ndd <= 0.0 { continue; }
                let (x, y, z) = frame.project(s.p);
                let (xi, yi) = (x.round() as i64, y.round() as i64);
                let list = if xi >= 0 && yi >= 0 && xi < frame.res as i64 && yi < frame.res as i64 { ab.at(xi as u32, yi as u32) } else { &[] };
                let bias = bias_m.max(0.5 * frame.pixel_m());
                let front: Vec<String> = list.iter().filter(|f| f.z < z - bias).map(|f| { let wt = &bvh.tris[f.tri as usize]; format!("tri {} inst {} z {:.2}{}", f.tri, wt.inst, f.z, if f.tri == s.own_tri { " OWN" } else { "" }) }).collect();
                eprintln!("  dir {di} d ({:.2},{:.2},{:.2}) ndd {ndd:.2}: pixel ({xi},{yi}) z {z:.2} bias {bias:.2}; {} frags, in front: [{}]; sky ({:.2},{:.2},{:.2})", d[0], d[1], d[2], list.len(), front.join(" | "), sky[0], sky[1], sky[2]);
                dbg_printed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        let acc_ptr = acc.as_mut_ptr() as usize;
        std::thread::scope(|sc| {
            for ch in range.chunks(chunk) {
                let ab = &ab;
                let frame = &frame;
                let subs = &subs;
                let shadow = shadow.as_ref();
                sc.spawn(move || {
                    for &i in ch {
                        let s = &subs[i as usize];
                        let ndd = dot(s.n, *d);
                        if ndd <= 0.0 {
                            continue;
                        }
                        let (x, y, z) = frame.project(s.p);
                        let (xi, yi) = (x.round() as i64, y.round() as i64);
                        let mut l = sky;
                        if xi >= 0 && yi >= 0 && xi < frame.res as i64 && yi < frame.res as i64 {
                            let list = ab.at(xi as u32, yi as u32);
                            // the deepest fragment still in front of the texel (depth < z − bias), not its own surface
                            let limit = z - bias;
                            // fragments are sorted by z: binary search the first with z >= limit
                            let mut lo = 0usize;
                            let mut hi = list.len();
                            while lo < hi {
                                let mid = (lo + hi) / 2;
                                if list[mid].z < limit { lo = mid + 1 } else { hi = mid }
                            }
                            let mut k = lo;
                            while k > 0 {
                                k -= 1;
                                let f = list[k];
                                if f.tri == s.own_tri {
                                    continue;
                                }
                                l = fragment_radiance(scene, bvh, prm, shadow, f.tri, *d, [s.p[0] + d[0] * (z - f.z), s.p[1] + d[1] * (z - f.z), s.p[2] + d[2] * (z - f.z)], sun_bias);
                                break;
                            }
                        }
                        let w = scale * ndd;
                        // SAFETY: each chunk owns a disjoint set of indices i; no other thread touches acc[i]
                        let slot = unsafe { &mut *(acc_ptr as *mut [f32; 3]).add(i as usize) };
                        for c in 0..3 {
                            slot[c] += w * l[c];
                        }
                    }
                });
            }
        });
        if di % 64 == 0 || di + 1 == n_dirs {
            eprintln!("peel: direction {}/{} ({} fragments, {:.1}s)", di + 1, n_dirs, ab.frags.len(), t0.elapsed().as_secs_f32());
        }
    }
    // 5. resolve: per colour texel the mean over its covered sub-samples
    let mut out: Vec<ChartBake> = scene
        .instances
        .iter()
        .enumerate()
        .map(|(ii, inst)| {
            let (w, h) = chart_meta[ii];
            ChartBake { item: inst.item, w, h, rgb: vec![[0.0; 3]; (w * h) as usize], rgb1: Vec::new(), covered: vec![false; (w * h) as usize], sun_vis: 0.0, sky_vis: 0.0 }
        })
        .collect();
    let mut counts: Vec<Vec<u16>> = chart_meta.iter().map(|(w, h)| vec![0u16; (w * h) as usize]).collect();
    for (i, s) in subs.iter().enumerate() {
        let c = &mut out[s.chart as usize];
        let t = s.texel as usize;
        for k in 0..3 {
            c.rgb[t][k] += acc[i][k];
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
                }
            }
        }
    }
    if std::env::var_os("LMTOOL_PEEL_DEBUG_BLACK").is_some() {
        // re-gather two black sub-samples of chart 0 with prints
        let black: Vec<usize> = subs.iter().enumerate().filter(|(i, s)| s.chart == 0 && acc[*i][0] + acc[*i][1] + acc[*i][2] < 1e-6).map(|(i, _)| i).step_by(100000).take(2).collect();
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
                let bias = bias_m.max(0.5 * frame.pixel_m());
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
            for (i, s) in subs.iter().enumerate() { if s.chart as usize == ci && acc[i][0] + acc[i][1] + acc[i][2] < 1e-6 { for k in 0..3 { nsum[k] += s.n[k] as f64; } nn += 1; } }
            eprintln!("peel debug: chart {ci} {}×{}: {covered} covered texels, {black} black; {nn} zero sub-samples, mean normal ({:.2},{:.2},{:.2})", c.w, c.h, nsum[0] / nn.max(1) as f64, nsum[1] / nn.max(1) as f64, nsum[2] / nn.max(1) as f64);
        }
    }
    for &i in &dbg_subs {
        let s = &subs[i as usize];
        let c = &out[s.chart as usize];
        eprintln!("peel debug: sub {i} acc {:?} → chart {} texel {} ({}×{}) rgb {:?} over {} subs", acc[i as usize], s.chart, s.texel, c.w, c.h, c.rgb[s.texel as usize], counts[s.chart as usize][s.texel as usize]);
    }
    eprintln!("peel: done, {} directions over {} sub-samples ({:.1}s)", n_dirs, subs.len(), t0.elapsed().as_secs_f32());
    out
}
