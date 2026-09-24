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
/// The peel's layer cap (the client's state machine stops after layer 20).
pub const MAX_LAYERS: usize = 20;

pub struct ABuffer {
    pub res: u32,
    /// Rows per band; the bands' CSR tables stay separate (no serial stitch of ~100 M fragments).
    pub band_h: u32,
    pub bands: Vec<(Vec<u32>, Vec<Frag>)>,
}

impl ABuffer {
    /// The fragments of pixel (x, y), nearest first.
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> &[Frag] {
        let b = &self.bands[(y / self.band_h) as usize];
        let i = ((y - (y / self.band_h) * self.band_h) * self.res + x) as usize;
        // the game peels at most 20 layers per direction (RenderLightIndirectPeel, counter > 0x13 stops):
        // the 20 kept here are the 20 nearest the SKY (smallest z); a texel deeper than all of them takes
        // the 20th. (Keeping the 20 nearest the receivers instead — "layer 1 = the deepest surface" — was
        // tried 2026-09-24 02:00Z: tiny 16 went from +42 % to +57 %; neither explains the editor's darker
        // hills under dense canopies. LMTOOL_LAYERS_NEAR=1 selects the receiver-side reading.)
        let (a, c) = (b.0[i] as usize, b.0[i + 1] as usize);
        if std::env::var_os("LMTOOL_LAYERS_NEAR").is_some() {
            &b.1[c.saturating_sub(MAX_LAYERS).max(a)..c]
        } else {
            &b.1[a..c.min(a + MAX_LAYERS)]
        }
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
    std::env::var("LMTOOL_CARDS_OCCLUDE").map(|v| v != "0").unwrap_or(true)
}

/// Whether the cards cast SUN shadows (the bake's sun shadow map). LMTOOL_CARDS_SHADOW=0 leaves them out.
pub fn cards_shadow() -> bool {
    // default OFF (DIFFERENTIAL, the hill test map 2026-09-23 23:50Z: the slopes under a dense canopy read
    // 0.19 of the editor's at Day with the cards in the sun shadow map, 0.76–1.4 without); =1 puts them in
    std::env::var("LMTOOL_CARDS_SHADOW").map(|v| v == "1").unwrap_or(false)
}

pub fn build_abuffer_upto(tris: &[WTri], frame: &PeelFrame, threads: usize, zmax: f32, masks: &[crate::geometry::AlphaMask]) -> ABuffer {
    let res = frame.res;
    // LMTOOL_PEEL_CULL_BACK=1 (hypothesis under test): the peel renders only the faces turned toward
    // the receivers' side (geometric normal against D); back faces are culled, not drawn black — so a
    // thin wall's own far face does not occlude its texels and a hollow tower sees out
    let cull_back = std::env::var("LMTOOL_PEEL_CULL_BACK").map(|v| v == "1").unwrap_or(false);
    let cards_occlude = cards_occlude();
    let d = frame.d;
    let bands = 128u32.min(res);
    let band_h = (res + bands - 1) / bands;
    // small chunks so the few huge decoration triangles (each covering the whole frame) spread over
    // the threads; the per-chunk overhead is a band vector set
    let chunk = (tris.len() / (threads.max(1) * 4)).max(256);
    // parts[thread][band] = (pixel, frag)
    let parts: Vec<Vec<Vec<(u32, Frag)>>> = std::thread::scope(|sc| {
        let hs: Vec<_> = tris
            .chunks(chunk)
            .enumerate()
            .map(|(ci, ch)| {
                let frame = frame.clone();
                sc.spawn(move || {
                    let mut out: Vec<Vec<(u32, Frag)>> = (0..bands).map(|_| Vec::new()).collect();
                    for (k, t) in ch.iter().enumerate() {
                        let ti = (ci * chunk + k) as u32;
                        let p0 = t.p0;
                        let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                        let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                        let (x0, y0, z0) = frame.project(p0);
                        let (x1, y1, z1) = frame.project(p1);
                        let (x2, y2, z2) = frame.project(p2);
                        if z0.min(z1).min(z2) >= zmax {
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
                        raster::triangle(res, res, [[x0, y0], [x1, y1], [x2, y2]], |x, y, b| {
                            let z = z0 * b[0] + z1 * b[1] + z2 * b[2];
                            if z < zmax {
                                // the alpha test: the cut-out texture at the fragment's TexCoord0
                                if let Some(m) = mask {
                                    let u = t.uv0[0][0] * b[0] + t.uv0[1][0] * b[1] + t.uv0[2][0] * b[2];
                                    let v = t.uv0[0][1] * b[0] + t.uv0[1][1] * b[1] + t.uv0[2][1] * b[2];
                                    if !m.opaque(u, v) {
                                        return;
                                    }
                                }
                                out[(y / band_h) as usize].push((y * res + x, Frag { z, tri: ti }));
                            }
                        });
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    // per band: counting sort by pixel + depth sort, in parallel
    let n = (res * res) as usize;
    let band_results: Vec<(Vec<u32>, Vec<Frag>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..bands)
            .map(|b| {
                let parts = &parts;
                sc.spawn(move || {
                    let y0 = b * band_h;
                    let y1 = ((b + 1) * band_h).min(res);
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
    ABuffer { res, band_h, bands: band_results }
}

/// A depth-only orthographic raster along the sun (the direct pass' shadow map).
pub struct ShadowMap {
    pub frame: PeelFrame,
    pub depth: raster::Depth,
}

impl ShadowMap {
    pub fn build(tris: &[WTri], sun_dir: V3, bmin: V3, bmax: V3, res: u32, masks: &[crate::geometry::AlphaMask]) -> ShadowMap {
        let frame = PeelFrame::new(sun_dir, bmin, bmax, res);
        let mut depth = raster::Depth::new(res, res);
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
            raster::triangle(res, res, [[x0, y0], [x1, y1], [x2, y2]], |x, y, b| {
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
        // the decoration (and any geometry without a lightmap) has no C0 of its own: the game's
        // GeomILightIn0 reads the ambient in its place — the mood's LAmbient (the undersides of the
        // tiny maps' ground-level items read 0.2 over the sea in the editor, our water gave 0.05 with
        // nothing but the low sun on it; DIFFERENTIAL: the factor is 1, --decor-ambient K overrides)
        _ if wt.inst == DECOR_INST => { let s = prm.decor_sky_up; [s[0] * prm.decor_ambient, s[1] * prm.decor_ambient, s[2] * prm.decor_ambient] },
        _ => [0.0; 3],
    };
    // a vegetation card is lit from both sides (thin foliage: the leaf shader's sun term does not care
    // which face the peel sees) — LMTOOL_CARD_ONE_SIDED=1 restores the plain n·L
    let is_card = wt.alpha != u16::MAX;
    let ndl = if is_card && !prm.card_one_sided { dot(n, prm.sun_dir).abs() } else { dot(n, prm.sun_dir).max(0.0) };
    let lit = if ndl > 0.0 && prm.sun_dir[1] > 0.0 { shadow.map(|s| s.lit(hit_p, sun_bias)).unwrap_or(1.0) } else { 0.0 };
    SUN_STATS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if ndl > 0.0 { SUN_STATS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
    if lit > 0.0 { SUN_STATS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
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
    let groups = 1usize;
    for (ii, _inst) in scene.instances.iter().enumerate() {
        let (cw, ch) = sizes[ii];
        chart_meta.push((cw, ch));
        let (lw, lh) = (cw * 2, ch * 2);
        let r = crate::chartraster::raster_chart(scene, ii, lw, lh, ss, prm.flip_v, prm.uv_bounds);
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
            subs.push(SubSample { p, n, own_tri: bvh.perm[(tri_base[ii] + tri) as usize], group: 0, chart: ii as u32, texel: ty.min(ch - 1) * cw + tx.min(cw - 1) });
        }
        if false {
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
            subs.push(SubSample { p: s.p, n: s.n, own_tri: bvh.perm[(tri_base[ii] + s.tri) as usize], group, chart: ii as u32, texel: ty.min(ch - 1) * cw + tx.min(cw - 1) });
        }
        }
    }
    eprintln!("peel: {} layout texels over {} charts (ss {ss} for coverage) ({:.1}s)", subs.len(), scene.instances.len(), t0.elapsed().as_secs_f32());
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
    let shadow = if prm.sun_dir[1] > 0.0 && prm.sun.iter().any(|c| *c > 0.0) { Some(ShadowMap::build(&bvh.tris, prm.sun_dir, bmin, bmax, prm.peel_res.max(1024), &prm.alpha_masks)) } else { None };
    let sun_bias = 2.5 * (bmax[0] - bmin[0]).max(bmax[2] - bmin[2]) / prm.peel_res.max(1024) as f32 + 0.05;
    // 4. the directions, interleaved into the groups
    let dirs: Vec<V3> = prm.sphere_dirs.iter().copied().collect();
    let n_dirs = dirs.len().max(1);
    let mut group_count = vec![0usize; groups];
    for (di, _) in dirs.iter().enumerate() {
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
    for (di, d) in dirs.iter().enumerate() {
        let g = di % groups;
        let scale = 4.0 / group_count[g].max(1) as f32;
        let frame = PeelFrame::new(*d, bmin, bmax, prm.peel_res);
        let tb = std::time::Instant::now();
        // the deepest receiver along this direction: nothing beyond it can occlude
        let zmax = (0..8).map(|i| { let p = [if i & 1 == 0 { bmin[0] } else { bmax[0] }, if i & 2 == 0 { bmin[1] } else { bmax[1] }, if i & 4 == 0 { bmin[2] } else { bmax[2] }]; frame.project(p).2 }).fold(f32::MIN, f32::max);
        let ab = build_abuffer_upto(&bvh.tris, &frame, threads, zmax, &prm.alpha_masks);
        let t_build = tb.elapsed().as_secs_f32();
        // the sky term's per-direction constant is w·4·d.y·SkyFactor (RE child 3, AddSkyVisibility /
        // SetILightDir 0x140234df0): the sky colour along d is weighted by the direction's elevation cosine
        // and below-horizon directions carry no sky (they see the ground); SkyFactor rides in sky_radiance
        let sky = { let s = sky_radiance(prm, *d); let dy = if sky_dy { d[1].max(0.0) } else if d[1] > 0.0 { 1.0 } else { 0.0 }; [s[0] * dy, s[1] * dy, s[2] * dy] };
        let bias = bias_m;
        let range = &order[group_start[g]..group_start[g + 1]];
        let chunk = (range.len() / threads.max(1)).max(1024);
        // the gather writes acc[i] for i in its own range only
        if !dbg_subs.is_empty() && di < 40 {
            if let Ok(k) = std::env::var("LMTOOL_PEEL_DEBUG_ITEM") {
                let k: u32 = k.parse().unwrap_or(0);
                let n = ab.iter().filter(|f| bvh.tris[f.tri as usize].inst == k).count();
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
        if !dbg_subs.is_empty() && (dbg_printed.load(std::sync::atomic::Ordering::Relaxed) < 40 || std::env::var_os("LMTOOL_PEEL_DEBUG_ALL").is_some()) {
            for &i in &dbg_subs {
                let s = &subs[i as usize];
                if s.group as usize != g { continue; }
                let ndd = dot(s.n, *d);
                if ndd <= 0.0 { continue; }
                let (x, y, z) = frame.project(s.p);
                let (xi, yi) = (x.round() as i64, y.round() as i64);
                let list = if xi >= 0 && yi >= 0 && xi < frame.res as i64 && yi < frame.res as i64 { ab.at(xi as u32, yi as u32) } else { &[] };
                let bias = bias_m;
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
                        let mut occluded = false;
                        if xi >= 0 && yi >= 0 && xi < frame.res as i64 && yi < frame.res as i64 {
                            let list = ab.at(xi as u32, yi as u32);
                            // the first surface along D beyond the texel by more than the rasteriser's depth bias
                            // (DepthBias 1 + SlopeScaledDepthBias 1.0, RE child 3): one depth unit plus one pixel's
                            // worth of the receiver's own depth slope, tan θ = √(1 − (n·D)²)/(n·D), capped
                            let _ = ndd;
                            // The game's peel layers are rasterised with D3D DepthBias 1 + SlopeScaledDepthBias 1.0
                            // (RE child 4): every stored layer depth is pushed toward the camera by one depth ulp
                            // plus ONE PIXEL of the fragment's own depth slope, and the texel takes the last layer
                            // still beyond it — so a surface occludes when it lies beyond the texel by more than
                            // (ε + pixel · tan θ_f), θ_f the angle between that surface and the peel direction: a
                            // sea plane half a metre under a deck counts (its slope is ~0 along a downward peel),
                            // a coplanar neighbour of the texel's own surface at grazing angle does not.
                            let limit_min = z - bias;
                            // fragments are sorted by z: binary search the first with z >= limit_min
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
                                // the fragment's slope: tan of the angle between its triangle and D
                                let wt = &bvh.tris[f.tri as usize];
                                let nf = norm(cross(wt.e1, wt.e2));
                                let c = dot(nf, *d).abs().max(1e-3);
                                let slope_f = ((1.0 - c * c).max(0.0).sqrt() / c).min(64.0);
                                if f.z >= z - bias.max(px_m * slope_f) {
                                    continue;
                                }
                                l = fragment_radiance(scene, bvh, prm, shadow, f.tri, *d, [s.p[0] + d[0] * (z - f.z), s.p[1] + d[1] * (z - f.z), s.p[2] + d[2] * (z - f.z)], sun_bias);
                                occluded = true;
                                break;
                            }
                        }
                        // LMTOOL_SKY_NO_COS=1: the sky pass (AddSkyVisibility) without the receiver's cosine — the
                        // per-direction constant 4·w·d.y·SkyFactor times the visibility only (a hypothesis under test:
                        // the editor's walls exceed its open floors even at sunrise)
                        let hit_sky = !occluded;
                        let w = if hit_sky && sky_no_cos { scale } else { scale * ndd };
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
            eprintln!("peel: direction {}/{} ({} fragments, build {:.2}s, gather {:.2}s; {:.1}s)", di + 1, n_dirs, ab.len(), t_build, tb.elapsed().as_secs_f32() - t_build, t0.elapsed().as_secs_f32());
        }
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
        for k in 0..3 { irr[s.chart as usize][t][k] += acc[i][k]; }
        let mut v = acc[i];
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
            for (i, s) in subs.iter().enumerate() { if s.chart as usize == ci && acc[i][0] + acc[i][1] + acc[i][2] < 1e-6 { for k in 0..3 { nsum[k] += s.n[k] as f64; } nn += 1; } }
            eprintln!("peel debug: chart {ci} {}×{}: {covered} covered texels, {black} black; {nn} zero sub-samples, mean normal ({:.2},{:.2},{:.2})", c.w, c.h, nsum[0] / nn.max(1) as f64, nsum[1] / nn.max(1) as f64, nsum[2] / nn.max(1) as f64);
        }
    }
    for &i in &dbg_subs {
        let s = &subs[i as usize];
        let c = &out[s.chart as usize];
        eprintln!("peel debug: sub {i} acc {:?} → chart {} texel {} ({}×{}) rgb {:?} over {} subs", acc[i as usize], s.chart, s.texel, c.w, c.h, c.rgb[s.texel as usize], counts[s.chart as usize][s.texel as usize]);
    }
    eprintln!("peel: done, {} directions over {} sub-samples ({:.1}s); fragment radiance calls {}, facing the sun {}, lit {}", n_dirs, subs.len(), t0.elapsed().as_secs_f32(), SUN_STATS[0].load(std::sync::atomic::Ordering::Relaxed), SUN_STATS[1].load(std::sync::atomic::Ordering::Relaxed), SUN_STATS[2].load(std::sync::atomic::Ordering::Relaxed));
    out
}
