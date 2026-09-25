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
        PeelFrame { d, r, u, s0: smin - 0.5 / scale, t0: tmin - 0.5 / scale, scale, scale_y: scale, res, res_y: res, zc: -0.5 * (zmin + zmax), half_d }
    }
    /// A frame that rasterises exactly the captured frustum's pixel grid (`w`×`h` pixels): the game's
    /// pixel x grows along `right`, pixel y along −`up`, its camera looks along `forward` = the port's d.
    pub fn from_frustum(f: &crate::passdump::Frustum, w: u32, h: u32) -> PeelFrame {
        let d = norm(f.forward);
        let r = norm(f.right);
        let u = norm([-f.up[0], -f.up[1], -f.up[2]]);
        let scale = w as f32 / (2.0 * f.half[0]);
        let scale_y = h as f32 / (2.0 * f.half[1]);
        PeelFrame { d, r, u, s0: dot(f.center, r) - f.half[0], t0: dot(f.center, u) - f.half[1], scale, scale_y, res: w, res_y: h, zc: dot(f.center, d), half_d: f.half[2] }
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
    /// The world size of one peel pixel.
    pub fn pixel_m(&self) -> f32 {
        1.0 / self.scale
    }
    /// Push the far plane out to cover every vertex of `tris` (the occluders below / beyond the
    /// receivers: the ground, the sea, the decoration), keeping the near plane where it is — without
    /// it everything beyond the receivers' bbox would pancake onto one far-plane layer and merge.
    pub fn extend_far(&mut self, tris: &[WTri]) {
        let zmax = self.z_from_z01(1.0);
        let mut zmin = self.z_from_z01(0.0);
        for t in tris {
            for p in [t.p0, [t.p0[0] + t.e1[0], t.p0[1] + t.e1[1], t.p0[2] + t.e1[2]], [t.p0[0] + t.e2[0], t.p0[1] + t.e2[1], t.p0[2] + t.e2[2]]] {
                let z = -dot(p, self.d);
                if z.is_finite() {
                    zmin = zmin.min(z);
                }
            }
        }
        self.half_d = (0.5 * (zmax - zmin)).max(1e-3);
        self.zc = -0.5 * (zmin + zmax);
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
    /// Every fragment of pixel (x, y), nearest the sky first, without the layer cap.
    #[inline]
    pub fn at_all(&self, x: u32, y: u32) -> &[Frag] {
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
    std::env::var("LMTOOL_CARDS_OCCLUDE").map(|v| v != "0").unwrap_or(true)
}

/// Whether the cards cast SUN shadows (the bake's sun shadow map). LMTOOL_CARDS_SHADOW=0 leaves them out.
pub fn cards_shadow() -> bool {
    // default OFF (DIFFERENTIAL, the hill test map 2026-09-23 23:50Z: the slopes under a dense canopy read
    // 0.19 of the editor's at Day with the cards in the sun shadow map, 0.76–1.4 without); =1 puts them in
    std::env::var("LMTOOL_CARDS_SHADOW").map(|v| v == "1").unwrap_or(false)
}

pub fn build_abuffer_upto(tris: &[WTri], frame: &PeelFrame, threads: usize, zmax: f32, masks: &[crate::geometry::AlphaMask]) -> ABuffer {
    build_abuffer_range(tris, frame, threads, f32::NEG_INFINITY, zmax, masks)
}

/// `build_abuffer_upto` with both depth planes: fragments with depth outside [zmin, zmax) are clipped
/// (the D3D depth clip of a fragment outside the frustum's near/far planes).
pub fn build_abuffer_range(tris: &[WTri], frame: &PeelFrame, threads: usize, zmin: f32, zmax: f32, masks: &[crate::geometry::AlphaMask]) -> ABuffer {
    let res = frame.res;
    let res_y = frame.res_y;
    // LMTOOL_PEEL_CULL_BACK=1 (hypothesis under test): the peel renders only the faces turned toward
    // the receivers' side (geometric normal against D); back faces are culled, not drawn black — so a
    // thin wall's own far face does not occlude its texels and a hollow tower sees out
    let cull_back = std::env::var("LMTOOL_PEEL_CULL_BACK").map(|v| v == "1").unwrap_or(false);
    let cards_occlude = cards_occlude();
    let d = frame.d;
    let bands = 128u32.min(res_y);
    let band_h = (res_y + bands - 1) / bands;
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
                        raster::triangle(res, res_y, [[x0, y0], [x1, y1], [x2, y2]], |x, y, b| {
                            let z = z0 * b[0] + z1 * b[1] + z2 * b[2];
                            if z < zmax && z >= zmin {
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
}

impl Layers {
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> &[LayerFrag] {
        let i = (y * self.w + x) as usize;
        &self.frags[self.start[i] as usize..self.start[i + 1] as usize]
    }
    /// Layer `k`'s depth image (the clear value 0 = far where the pixel has fewer layers).
    pub fn depth_image(&self, k: usize) -> Vec<f32> {
        (0..(self.w * self.h) as usize).map(|i| { let (a, c) = (self.start[i] as usize, self.start[i + 1] as usize); if a + k < c { self.frags[a + k].d } else { 0.0 } }).collect()
    }
    /// Layer `k`'s colour image (black where the pixel has fewer layers).
    pub fn colour_image(&self, k: usize) -> Vec<[f32; 3]> {
        (0..(self.w * self.h) as usize).map(|i| { let (a, c) = (self.start[i] as usize, self.start[i + 1] as usize); if a + k < c { self.frags[a + k].rgb } else { [0.0; 3] } }).collect()
    }
}

/// The D3D11 rasteriser depth bias of a fragment on a D32_FLOAT target: `DepthBias · 2^(exponent(max
/// z01 of the primitive) − 23) + SlopeScaledDepthBias · max(|∂z01/∂x|, |∂z01/∂y|)` (per pixel step).
pub fn d3d_depth_bias(zmax_prim: f32, slope: f32, bias: (i32, f32)) -> f32 {
    let e = if zmax_prim > 0.0 { zmax_prim.log2().floor() } else { -126.0 };
    bias.0 as f32 * 2f32.powf(e - 23.0) + bias.1 * slope
}

/// The depth gradient (per pixel) of a world triangle in a frame, in z01 units.
fn tri_slope(wt: &WTri, frame: &PeelFrame) -> (f32, f32) {
    let p1 = [wt.p0[0] + wt.e1[0], wt.p0[1] + wt.e1[1], wt.p0[2] + wt.e1[2]];
    let p2 = [wt.p0[0] + wt.e2[0], wt.p0[1] + wt.e2[1], wt.p0[2] + wt.e2[2]];
    let (x0, y0, z0) = frame.project(wt.p0);
    let (x1, y1, z1) = frame.project(p1);
    let (x2, y2, z2) = frame.project(p2);
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
/// previous layer are never rendered again — merged), at most `MAX_LAYERS` layers. The colour is the
/// fragment's `ILightInput` radiance at the pixel centre, quantised as the colour target stores it.
fn extract_layers(ab: &ABuffer, frame: &PeelFrame, scene: &Scene, bvh: &Bvh, prm: &BakeParams, shadow: Option<&ShadowMap>, sun_bias: f32, sky: [f32; 3], threads: usize) -> Layers {
    let (w, h) = (frame.res, frame.res_y);
    let n = (w * h) as usize;
    let rows_per = ((h as usize) / threads.max(1)).max(1);
    let sky_q = prm.quant_peel.apply(sky, prm.rounding);
    let parts: Vec<(Vec<u32>, Vec<LayerFrag>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..h as usize)
            .step_by(rows_per)
            .map(|y0| {
                let y1 = (y0 + rows_per).min(h as usize);
                sc.spawn(move || {
                    let mut counts = Vec::with_capacity((y1 - y0) * w as usize);
                    let mut out: Vec<LayerFrag> = Vec::new();
                    for y in y0..y1 {
                        for x in 0..w as usize {
                            let list = ab.at_all(x as u32, y as u32);
                            let before = out.len();
                            let mut d_prev = f32::NEG_INFINITY;
                            if prm.dome_layer {
                                out.push(LayerFrag { d: 0.0, rgb: sky_q });
                                d_prev = 0.0;
                            }
                            for f in list {
                                // pancaking: a fragment beyond the far plane lands on it (z01 = 0)
                                let z01 = frame.z01(f.z).max(0.0);
                                if z01 < d_prev {
                                    continue;
                                }
                                if out.len() - before >= MAX_LAYERS {
                                    break;
                                }
                                let wt = &bvh.tris[f.tri as usize];
                                let (slope, zmax_prim) = tri_slope(wt, frame);
                                // an edge-on triangle's slope is huge (D3D applies it uncapped, DepthBiasClamp 0)
                                let d = z01 + d3d_depth_bias(zmax_prim, slope.min(1e6), prm.depth_bias);
                                let hit_p = frame.unproject(x as f32 + 0.5, y as f32 + 0.5, f.z);
                                let rgb = prm.quant_peel.apply(fragment_radiance(scene, bvh, prm, shadow, f.tri, frame.d, hit_p, sun_bias), prm.rounding);
                                out.push(LayerFrag { d, rgb });
                                d_prev = d;
                            }
                            counts.push((out.len() - before) as u32);
                        }
                    }
                    (counts, out)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut start = Vec::with_capacity(n + 1);
    let mut frags = Vec::with_capacity(parts.iter().map(|p| p.1.len()).sum());
    start.push(0u32);
    for (counts, out) in parts {
        for c in counts {
            let last = *start.last().unwrap();
            start.push(last + c);
        }
        frags.extend(out);
    }
    Layers { w, h, start, frags, max_layers: MAX_LAYERS }
}

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
    for (ii, _inst) in scene.instances.iter().enumerate() {
        let (cw, ch) = sizes[ii];
        chart_meta.push((cw, ch));
        let (lw, lh) = (cw * 2, ch * 2);
        let r = crate::chartraster::raster_chart(scene, ii, lw, lh, ss, prm.flip_v, prm.uv_bounds);
        let (gw, gh) = (lw * ss_eff, lh * ss_eff);
        let mut geo = if dumping { (vec![[0.0f32; 3]; (gw * gh) as usize], vec![[0.0f32; 3]; (gw * gh) as usize], vec![[0.0f32; 3]; (gw * gh) as usize]) } else { (Vec::new(), Vec::new(), Vec::new()) };
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
        }
        if dumping { chart_geo.push(geo); }
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
            subs.push(SubSample { p: s.p, n: s.n, own_tri: bvh.perm[(tri_base[ii] + s.tri) as usize], group, chart: ii as u32, texel: ty.min(ch - 1) * cw + tx.min(cw - 1), sx: s.sx, sy: s.sy });
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
            let lit = if ndl > 0.0 && prm.sun_dir[1] > 0.0 { shadow.as_ref().map(|sm| sm.lit(s.p, sun_bias)).unwrap_or(1.0) } else { 0.0 };
            let mut v = [0f32; 3];
            for k in 0..3 { v[k] = alb[k] * (stored[k] + prm.sun[k] * ndl * lit); }
            ilight[ii][gi] = v;
        }
        for ii in 0..chart_meta.len() {
            let (cw, ch) = chart_meta[ii];
            let (gw, gh) = (cw * 2 * ss_eff, ch * 2 * ss_eff);
            if prm.sweep == 0 {
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
    let game_dbg: Option<(usize, usize, usize)> = std::env::var("LMTOOL_GAME_PEEL_DEBUG").ok().and_then(|s| { let v: Vec<usize> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 3 { Some((v[0], v[1], v[2])) } else { None } });
    let game_dbg = &game_dbg;
    let dbg_printed = &dbg_printed;
    for (di, d) in dirs.iter().enumerate() {
        let g = di % groups;
        let scale = 4.0 / group_count[g].max(1) as f32;
        let frame = match prm.frustums.as_ref().and_then(|fs| fs.get(di)) {
            Some(fr) => PeelFrame::from_frustum(fr, prm.peel_res, prm.peel_res),
            None => {
                let mut fr = PeelFrame::new(*d, bmin, bmax, prm.peel_res);
                // the game's frustum covers its whole scene (the ground tiles included); ours is fit to
                // the receivers, so the far plane is pushed out to every occluder — otherwise the ground,
                // the sea and the decoration would pancake onto one far-plane layer and merge into its
                // farthest (often invisible / black) member (found by the dry run, 2026-09-24 18:10 PT)
                if prm.game_peel { fr.extend_far(&bvh.tris); }
                fr
            }
        };
        let tb = std::time::Instant::now();
        // the deepest receiver along this direction: nothing beyond it can occlude
        let zmax = (0..8).map(|i| { let p = [if i & 1 == 0 { bmin[0] } else { bmax[0] }, if i & 2 == 0 { bmin[1] } else { bmax[1] }, if i & 4 == 0 { bmin[2] } else { bmax[2] }]; frame.project(p).2 }).fold(f32::MIN, f32::max);
        // the game's peel renders everything inside the frustum's depth range; beyond the far plane the
        // fragments are CLAMPED to it unless `prm.depth_clip` (D3D DepthClipEnable: not read off the state
        // yet — pancaking keeps the ground far below the receivers as an occluder, clipping would drop it);
        // fragments nearer than the near plane can never be selected (they are on the receivers' side)
        let ab = if prm.game_peel { build_abuffer_range(&bvh.tris, &frame, threads, if prm.depth_clip { frame.z_from_z01(0.0) } else { f32::NEG_INFINITY }, frame.z_from_z01(1.0), &prm.alpha_masks) } else { build_abuffer_upto(&bvh.tris, &frame, threads, zmax, &prm.alpha_masks) };
        let t_build = tb.elapsed().as_secs_f32();
        // the sky term's per-direction constant is w·4·d.y·SkyFactor (RE child 3, AddSkyVisibility /
        // SetILightDir 0x140234df0): the sky colour along d is weighted by the direction's elevation cosine
        // and below-horizon directions carry no sky (they see the ground); SkyFactor rides in sky_radiance
        let sky = { let s = sky_radiance(prm, *d); let dy = if sky_dy { d[1].max(0.0) } else if d[1] > 0.0 { 1.0 } else { 0.0 }; [s[0] * dy, s[1] * dy, s[2] * dy] };
        let bias = bias_m;
        let range = &order[group_start[g]..group_start[g + 1]];
        let chunk = (range.len() / threads.max(1)).max(1024);
        // the game's layers of this direction (game-peel mode), and their dump
        let want_dir_dump = prm.dump.as_ref().map(|dm| dm.lock().unwrap().wants_dir(di as u32)).unwrap_or(false);
        // (the layers are also extracted for the dump alone, so the port's own gather can be dumped and compared)
        let layers: Option<Layers> = if prm.game_peel || want_dir_dump { Some(extract_layers(&ab, &frame, scene, bvh, prm, shadow.as_ref(), sun_bias, sky, threads)) } else { None };
        if let (Some(ly), Some(dump), true) = (&layers, &prm.dump, want_dir_dump) {
            let mut dmp = dump.lock().unwrap();
            let nl = (0..(ly.w * ly.h) as usize).map(|i| (ly.start[i + 1] - ly.start[i]) as usize).max().unwrap_or(0);
            for k in 0..nl {
                let mut e = crate::passdump::entry("peel_depth", format!("peel_depth/s{}/d{di:03}/l{k:02}.bin", prm.sweep), "peel");
                e.sweep = Some(prm.sweep); e.direction = Some(di as u32); e.layer = Some(k as u32);
                e.dir = Some(*d); e.frustum = Some(frame.frustum()); e.format = "R32_FLOAT".into();
                e.cleared_to = Some(serde_json::json!(0.0));
                e.notes = Some(format!("stored depth = z01 + D3D bias ({}, {:.2}) on D32; layer 0 = {}; far-to-near", prm.depth_bias.0, prm.depth_bias.1, if prm.dome_layer { "the sky dome (synthetic: depth 0, the sky radiance)" } else { "the farthest surface" }));
                dmp.write_f32(e, ly.w, ly.h, 1, &ly.depth_image(k)).expect("dump peel_depth");
                let mut e = crate::passdump::entry("peel_color", format!("peel_color/s{}/d{di:03}/l{k:02}.bin", prm.sweep), "peel");
                e.sweep = Some(prm.sweep); e.direction = Some(di as u32); e.layer = Some(k as u32);
                e.dir = Some(*d); e.frustum = Some(frame.frustum());
                e.cleared_to = Some(serde_json::json!([0.0, 0.0, 0.0]));
                e.notes = Some("ILightInput of the layer's surface at the pixel centre (front faces; back faces black)".into());
                dmp.write_rgb(e, ly.w, ly.h, &ly.colour_image(k), prm.quant_peel, prm.rounding).expect("dump peel_color");
            }
        }
        // the per-sub-sample incoming radiance of this direction (TMapILightDir), kept when dumped
        let mut ldir: Vec<[f32; 3]> = if want_dir_dump { vec![[0.0; 3]; subs.len()] } else { Vec::new() };
        let ldir_ptr = ldir.as_mut_ptr() as usize;
        let ldir_on = want_dir_dump;
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
                let layers = layers.as_ref();
                sc.spawn(move || {
                    for &i in ch {
                        let s = &subs[i as usize];
                        let ndd = dot(s.n, *d);
                        if ndd <= 0.0 {
                            continue;
                        }
                        let (x, y, z) = frame.project(s.p);
                        let mut l = sky;
                        let mut occluded = false;
                        if let (Some(ly), true) = (layers, prm.game_peel) {
                            // THE GAME'S LOOKUP (LmILightDir_Set_p): point-sample the layer targets after the one-texel
                            // inset, at the texel's own reversed depth; the nearest layer still beyond the texel by its
                            // stored bias gives the colour (the dome layer catches the open sky); nothing → the clear
                            let (px, py) = (lookup_pixel(x, ly.w, prm.peel_inset), lookup_pixel(y, ly.h, prm.peel_inset));
                            let z01 = frame.z01(z);
                            // LMTOOL_GAME_PEEL_DEBUG=dir,chart,count: print the layer list and the selection of the
                            // first `count` gathering sub-samples of that chart for that direction
                            if let Some(&(dd, cc, nn)) = game_dbg.as_ref() {
                                if dd == di && cc == s.chart as usize && dbg_printed.load(std::sync::atomic::Ordering::Relaxed) < nn {
                                    let k = dbg_printed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    if k < nn {
                                        let list = ly.at(px, py);
                                        let sel = select_layer(list, z01).map(|f| format!("d {:.5} rgb ({:.3},{:.3},{:.3})", f.d, f.rgb[0], f.rgb[1], f.rgb[2])).unwrap_or("NONE".into());
                                        eprintln!("game-peel debug: dir {di} d ({:.3},{:.3},{:.3}) chart {} sub ({},{}) p ({:.2},{:.2},{:.2}) n ({:.2},{:.2},{:.2}) ndd {ndd:.3}: px ({x:.2},{y:.2}) → lookup ({px},{py}), z_port {z:.3} z01 {z01:.5}; {} layers: [{}]; selected {sel}", d[0], d[1], d[2], s.chart, s.sx, s.sy, s.p[0], s.p[1], s.p[2], s.n[0], s.n[1], s.n[2], list.len(), list.iter().map(|f| format!("d {:.5} rgb ({:.2},{:.2},{:.2})", f.d, f.rgb[0], f.rgb[1], f.rgb[2])).collect::<Vec<_>>().join(" | "));
                                    }
                                }
                            }
                            match select_layer(ly.at(px, py), z01) {
                                Some(f) => { l = f.rgb; occluded = f.d > 0.0 || !prm.dome_layer; }
                                None => { l = if prm.dome_layer { [0.0; 3] } else { sky }; }
                            }
                        } else {
                        let (xi, yi) = (x.round() as i64, y.round() as i64);
                        if xi >= 0 && yi >= 0 && xi < frame.res as i64 && yi < frame.res_y as i64 {
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
                        }
                        // LMTOOL_SKY_NO_COS=1: the sky pass (AddSkyVisibility) without the receiver's cosine — the
                        // per-direction constant 4·w·d.y·SkyFactor times the visibility only (a hypothesis under test:
                        // the editor's walls exceed its open floors even at sunrise)
                        let hit_sky = !occluded;
                        let w = if hit_sky && sky_no_cos { scale } else { scale * ndd };
                        // the TMapILightDir value as its target stores it
                        let l = prm.quant_ilightdir.apply(l, prm.rounding);
                        if ldir_on {
                            // SAFETY: as for acc — disjoint indices per chunk
                            let lslot = unsafe { &mut *(ldir_ptr as *mut [f32; 3]).add(i as usize) };
                            *lslot = l;
                        }
                        // SAFETY: each chunk owns a disjoint set of indices i; no other thread touches acc[i]
                        let slot = unsafe { &mut *(acc_ptr as *mut [f32; 3]).add(i as usize) };
                        for c in 0..3 {
                            slot[c] += w * l[c];
                        }
                        // the accumulation target's own storage (an f16 target rounds after every add)
                        *slot = prm.quant_accum.apply(*slot, prm.rounding);
                    }
                });
            }
        });
        if want_dir_dump {
            if let Some(dump) = &prm.dump {
                let mut dmp = dump.lock().unwrap();
                let mut imgs: Vec<Vec<[f32; 3]>> = chart_meta.iter().map(|(cw, ch)| vec![[0.0f32; 3]; (cw * 2 * ss_eff * ch * 2 * ss_eff) as usize]).collect();
                for (i, s) in subs.iter().enumerate() {
                    let gw = chart_meta[s.chart as usize].0 * 2 * ss_eff;
                    imgs[s.chart as usize][(s.sy * gw + s.sx) as usize] = ldir[i];
                }
                for ii in 0..chart_meta.len() {
                    let (cw, ch) = chart_meta[ii];
                    let mut e = crate::passdump::entry("ilightdir", chart_file("ilightdir", Some(prm.sweep), Some(di as u32), ii), "chart_ss");
                    e.chart = Some(chart_ref(ii)); e.sweep = Some(prm.sweep); e.direction = Some(di as u32); e.dir = Some(*d);
                    e.cleared_to = Some(serde_json::json!([0.0, 0.0, 0.0]));
                    e.notes = Some("incoming radiance from D per sub-sample (0 where n·D ≤ 0 or uncovered); the accumulate adds 4/N·max(0,n·D) × this".into());
                    dmp.write_rgb(e, cw * 2 * ss_eff, ch * 2 * ss_eff, &imgs[ii], prm.quant_ilightdir, prm.rounding).expect("dump ilightdir");
                }
            }
        }
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
    // --- the differential harness: this sweep's accumulation target and its resolve ---
    if let Some(dump) = &prm.dump {
        let mut dmp = dump.lock().unwrap();
        let mut imgs: Vec<Vec<[f32; 3]>> = chart_meta.iter().map(|(cw, ch)| vec![[0.0f32; 3]; (cw * 2 * ss_eff * ch * 2 * ss_eff) as usize]).collect();
        for (i, s) in subs.iter().enumerate() {
            let gw = chart_meta[s.chart as usize].0 * 2 * ss_eff;
            imgs[s.chart as usize][(s.sy * gw + s.sx) as usize] = acc[i];
        }
        for ii in 0..chart_meta.len() {
            let (cw, ch) = chart_meta[ii];
            let mut e = crate::passdump::entry("lightsum", chart_file("lightsum", Some(prm.sweep), None, ii), "chart_ss");
            e.chart = Some(chart_ref(ii)); e.sweep = Some(prm.sweep);
            e.notes = Some(format!("E = Σ_D 4/N·max(0,n·D)·L_D over the sweep's {} directions per sub-sample (uncovered = 0)", dirs.len()));
            dmp.write_rgb(e, cw * 2 * ss_eff, ch * 2 * ss_eff, &imgs[ii], prm.quant_accum, prm.rounding).expect("dump lightsum");
            let mut e = crate::passdump::entry("lightsum_resolved", chart_file("lightsum_resolved", Some(prm.sweep), None, ii), "chart");
            e.chart = Some(chart_ref(ii)); e.sweep = Some(prm.sweep);
            e.notes = Some("the ss resolve: mean over the texel's covered sub-samples (LmSSResolve + LmSSNormWithA), at stored resolution".into());
            let rgb: Vec<[f32; 3]> = if out[ii].rgb_irr.is_empty() { out[ii].rgb.clone() } else { out[ii].rgb_irr.clone() };
            dmp.write_rgb(e, cw, ch, &rgb, crate::gpufmt::Quant::None, prm.rounding).expect("dump lightsum_resolved");
        }
        dmp.finish().expect("write MANIFEST.json");
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
