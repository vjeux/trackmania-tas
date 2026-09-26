//! THE SKY DOME AS THE GAME RASTERISES IT — the mesh of draw eid 1051 (pwc-day frame 127448) through
//! VS 16773 `GbxSkyV0` and PS 16774, transcribed (rows 5/6, port engineer D).
//!
//! The dome is a mesh (e001051: 2143 vertices in 33 rings of 65, 3968 triangles; POSITION f32×3 @0,
//! TEXCOORD f32×2 @20, stride 28) of an ellipsoid of radii 22265.2265625 (x, z) / 9751.1611328125 (y)
//! at the world origin (`VisualToWorld` = identity). VS 16773:
//!
//! ```text
//!   o0.xy = world · GbxV_WorldPrCamera[0..1]      (the peel's orthographic projection)
//!   o0.z  = (world · WorldPrCamera[3]) · GbxV_ZBufferNearFar_Free2.y = 0   (Free2.y = 0: the dome sits ON the far plane)
//!   o0.w  = world · WorldPrCamera[3] = 1
//!   o1.x  = GradientV_ForceX ≥ 0 ? ForceX : v1.x − LightDirAngle_m11Zx     (ForceX = −1: u = u_mesh − the sun's azimuth/π)
//!   o1.y  = GradientV_InvertY ? 1 − v1.y : v1.y                              (InvertY = 1)
//!   o2    = world − GbxV_EyeInWorld
//! ```
//!
//! The rasteriser state: cull Back with FrontCounterClockwise — the far hemisphere's inner faces
//! render (in the capture their NDC winding is counter-clockwise with y up), the near hemisphere's are
//! culled; depth GreaterEqual against the environment clear 0.0 (the dome passes where nothing nearer
//! was drawn); the attributes are interpolated LINEARLY in screen space (w = 1). So a peel pixel's
//! (u, v) is the barycentric interpolation of the vertex uvs of the ONE dome triangle covering it — a
//! handful of huge triangles per 2.4 km frame (7 for the capture's first direction) — not the analytic
//! atan2/height of the pixel ray's ellipsoid hit (`SkyGradient::dome_radiance`, the model this
//! replaces: across a facet the two differ by up to a third of a texel in u, which in the sun-glow
//! band is a 4–16 % colour difference — the contour lines of the `peel_sky` heat map).
//!
//! PS 16774 (`SkyGradient::sky_ps`): `r1 = TMapGradientV(uv)·ScaleGrad0 + TMapGradientV1(uv)·ScaleGrad1`
//! (SMapGradientV: Linear, u MIRROR, v ClampEdge — `logs/samplers-frame127448.json` eid 1051), the sun
//! disc `exp2(SunPower·log2(max(0, view·−L)))·SunPower·LightDirRgb` only when SunIsVisible (0 here), the
//! two Atmo lobes `exp2(Power_i·log2(cos))·Scale_i·Rgb_i`, `lerp(Fog, ·, sat(1 − FogIntens))`,
//! × GlobalScale, min 16375.

use crate::geometry::V3;
use crate::peel::PeelFrame;
use std::path::Path;

/// The dome mesh: world positions and texture coordinates per vertex, triangle indices.
#[derive(Clone, Debug)]
pub struct DomeMesh {
    pub pos: Vec<V3>,
    pub uv: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
}

/// One kept (front-facing, frame-touching) dome triangle: its vertices' pixel positions and VS outputs.
#[derive(Clone, Copy, Debug)]
pub struct DomeTri {
    pub px: [[f32; 2]; 3],
    pub uv: [[f32; 2]; 3],
    pub view: [V3; 3],
}

/// One peel frame's rasterised dome: per pixel the kept triangle covering its centre (`u16::MAX` =
/// none); `at(x, y)` recomputes the pixel's barycentrics with the rasteriser's own edge functions and
/// interpolates o1 (the texture coordinate, sun shift and v inversion applied) and o2 (`world − eye`).
/// Two bytes a pixel instead of the interpolated attributes (a 4096² frame is 16.7 M pixels).
pub struct DomeRaster {
    pub w: u32,
    pub h: u32,
    /// The per-pixel triangle table when filled (`rasterise_filled`); empty = LAZY: `at` asks the kept
    /// triangles in order with the rasteriser's own coverage test (the last covering one wins, as the
    /// fill's overwrites) — the frame has 16.7 M pixels and two to four kept triangles, the peel reads a
    /// million of them.
    pub tri: Vec<u16>,
    pub tris: Vec<DomeTri>,
    pub inset: u32,
}

impl DomeRaster {
    /// The kept triangle covering pixel (x, y): the table, or the lazy search.
    #[inline]
    fn tri_at(&self, x: u32, y: u32) -> u16 {
        if !self.tri.is_empty() {
            return self.tri[(y * self.w + x) as usize];
        }
        if x < self.inset || y < self.inset || x + self.inset >= self.w || y + self.inset >= self.h {
            return u16::MAX;
        }
        let mut hit = u16::MAX;
        for (i, tr) in self.tris.iter().enumerate() {
            if crate::raster::covers(tr.px, x, y) {
                hit = i as u16;
            }
        }
        hit
    }
    /// The interpolated (uv, view) at pixel (x, y), None where no front-facing dome triangle covers it.
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> Option<([f32; 2], V3)> {
        let t = self.tri_at(x, y);
        if t == u16::MAX {
            return None;
        }
        let tr = &self.tris[t as usize];
        let b = barycentrics(tr.px, [x as f32 + 0.5, y as f32 + 0.5]);
        let uv = [tr.uv[0][0] * b[0] + tr.uv[1][0] * b[1] + tr.uv[2][0] * b[2], tr.uv[0][1] * b[0] + tr.uv[1][1] * b[1] + tr.uv[2][1] * b[2]];
        let view = [
            tr.view[0][0] * b[0] + tr.view[1][0] * b[1] + tr.view[2][0] * b[2],
            tr.view[0][1] * b[0] + tr.view[1][1] * b[1] + tr.view[2][1] * b[2],
            tr.view[0][2] * b[0] + tr.view[1][2] * b[1] + tr.view[2][2] * b[2],
        ];
        Some((uv, view))
    }
    #[inline]
    pub fn covered(&self, x: u32, y: u32) -> bool {
        self.tri_at(x, y) != u16::MAX
    }
    /// Kept triangles.
    pub fn triangles(&self) -> usize {
        self.tris.len()
    }
}

/// The barycentric weights of `q` in the triangle `p` (vertex order kept): each vertex's weight is the
/// edge function of the opposite edge over the triangle's signed area — `raster::triangle`'s formula.
#[inline]
pub fn barycentrics(p: [[f32; 2]; 3], q: [f32; 2]) -> [f32; 3] {
    let edge = |a: [f32; 2], b: [f32; 2]| -> f32 { (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]) };
    let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
    let inv = 1.0 / area;
    [edge(p[1], p[2]) * inv, edge(p[2], p[0]) * inv, edge(p[0], p[1]) * inv]
}

fn read_gz_or_plain(p: &Path) -> Result<Vec<u8>, String> {
    if p.exists() {
        return std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()));
    }
    let gz = std::path::PathBuf::from(format!("{}.gz", p.display()));
    let d = std::fs::read(&gz).map_err(|e| format!("{}: {e}", gz.display()))?;
    crate::passdiff::gunzip(&d)
}

impl DomeMesh {
    /// Load the dome mesh of the capture under `passcap` (the pwc-day layout: `mesh/frame127448/
    /// e001051_vb0_16776.bin[.gz]` + `e001051_vsout_indices.bin[.gz]`, or the same under `env/`).
    pub fn load(passcap: &Path) -> Result<DomeMesh, String> {
        let frame = 127448u32;
        let eid = 1051u32;
        let vb = read_gz_or_plain(&passcap.join(format!("mesh/frame{frame}/e{eid:06}_vb0_16776.bin"))).or_else(|_| read_gz_or_plain(&passcap.join(format!("env/frame{frame}/mesh/vb_16776.bin"))))?;
        let ib = read_gz_or_plain(&passcap.join(format!("mesh/frame{frame}/e{eid:06}_vsout_indices.bin"))).or_else(|_| read_gz_or_plain(&passcap.join(format!("env/frame{frame}/mesh/e{eid:06}_vsout_indices.bin"))))?;
        Self::from_bytes(&vb, 28, 0, 20, &ib)
    }

    /// From raw vertex-buffer bytes (stride, the POSITION and TEXCOORD byte offsets) and u16 indices.
    pub fn from_bytes(vb: &[u8], stride: usize, pos_off: usize, uv_off: usize, ib: &[u8]) -> Result<DomeMesh, String> {
        if stride < 8 || vb.len() % stride != 0 {
            return Err(format!("dome VB: {} bytes is not a multiple of the stride {stride}", vb.len()));
        }
        let f = |o: usize| f32::from_le_bytes([vb[o], vb[o + 1], vb[o + 2], vb[o + 3]]);
        let n = vb.len() / stride;
        let mut pos = Vec::with_capacity(n);
        let mut uv = Vec::with_capacity(n);
        for i in 0..n {
            let o = i * stride;
            pos.push([f(o + pos_off), f(o + pos_off + 4), f(o + pos_off + 8)]);
            uv.push([f(o + uv_off), f(o + uv_off + 4)]);
        }
        let indices: Vec<u16> = ib.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        if indices.iter().any(|&i| i as usize >= n) {
            return Err("dome indices out of range".into());
        }
        Ok(DomeMesh { pos, uv, indices })
    }

    /// The dome rasterised in `frame` as VS 16773 + the rasteriser state would: `eye` = GbxV_EyeInWorld
    /// (the peel camera's position: o2 = world − eye), `light_dir_angle` = GbxSkyV0.LightDirAngle_m11Zx
    /// (the sun's azimuth/π, subtracted from u), `force_x` = GradientV_ForceX (≥ 0 forces u to it),
    /// `invert_y` = GradientV_InvertY. Triangles are culled by their winding (front = counter-clockwise
    /// in NDC with y up = a negative signed area in pixel space, y down) and rasterised with the D3D
    /// rules (`raster::triangle`); the depth test GreaterEqual at the common depth 0 lets every dome
    /// fragment through and the mesh's front faces do not overlap, so the draw order is immaterial.
    /// The lazy raster (no per-pixel table): the kept triangles, coverage decided per queried pixel.
    pub fn rasterise(&self, frame: &PeelFrame, eye: V3, light_dir_angle: f32, force_x: f32, invert_y: bool) -> DomeRaster {
        self.rasterise_impl(frame, eye, light_dir_angle, force_x, invert_y, false)
    }

    /// The filled raster (the per-pixel table, as the GPU's target).
    pub fn rasterise_filled(&self, frame: &PeelFrame, eye: V3, light_dir_angle: f32, force_x: f32, invert_y: bool) -> DomeRaster {
        self.rasterise_impl(frame, eye, light_dir_angle, force_x, invert_y, true)
    }

    fn rasterise_impl(&self, frame: &PeelFrame, eye: V3, light_dir_angle: f32, force_x: f32, invert_y: bool, fill: bool) -> DomeRaster {
        let (w, h) = (frame.res, frame.res_y);
        let mut tri = if fill { vec![u16::MAX; (w * h) as usize] } else { Vec::new() };
        let mut px: Vec<[f32; 2]> = Vec::with_capacity(self.pos.len());
        let mut o1: Vec<[f32; 2]> = Vec::with_capacity(self.pos.len());
        let mut o2: Vec<V3> = Vec::with_capacity(self.pos.len());
        for (p, t) in self.pos.iter().zip(self.uv.iter()) {
            let (x, y, _z) = frame.project(*p);
            px.push([x, y]);
            let u = if force_x >= 0.0 { force_x } else { t[0] - light_dir_angle };
            let v = if invert_y { 1.0 - t[1] } else { t[1] };
            o1.push([u, v]);
            o2.push([p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]]);
        }
        let mut tris: Vec<DomeTri> = Vec::new();
        for t in self.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let (pa, pb, pc) = (px[a], px[b], px[c]);
            // back-face cull: front = counter-clockwise in NDC (y up) = a negative signed area with y down
            let area = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pc[0] - pa[0]) * (pb[1] - pa[1]);
            if area >= 0.0 {
                continue;
            }
            // the bbox cull (the frame is a small window on a 22 km dome)
            let (minx, maxx) = (pa[0].min(pb[0]).min(pc[0]), pa[0].max(pb[0]).max(pc[0]));
            let (miny, maxy) = (pa[1].min(pb[1]).min(pc[1]), pa[1].max(pb[1]).max(pc[1]));
            if maxx < 0.0 || maxy < 0.0 || minx > w as f32 || miny > h as f32 {
                continue;
            }
            if tris.len() >= u16::MAX as usize {
                break;
            }
            let id = tris.len() as u16;
            tris.push(DomeTri { px: [pa, pb, pc], uv: [o1[a], o1[b], o1[c]], view: [o2[a], o2[b], o2[c]] });
            let inset = frame.inset_px;
            if !fill {
                continue;
            }
            crate::raster::triangle(w, h, [pa, pb, pc], |x, y, _bary| {
                // the game's viewport (1, 1, w−2, h−2): the outer ring is never drawn
                if x < inset || y < inset || x + inset >= w || y + inset >= h {
                    return;
                }
                tri[(y * w + x) as usize] = id;
            });
        }
        DomeRaster { w, h, tri, tris, inset: frame.inset_px }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A far-side facet at +1000 along D, wound so that it faces the camera (counter-clockwise in NDC),
    /// and its mirror on the near side (clockwise from the camera): the far one renders, the near one is
    /// culled; u interpolates linearly across the facet.
    #[test]
    fn the_far_facet_renders_and_the_near_one_is_culled() {
        let d = [0.0f32, 0.0, 1.0];
        let frame = PeelFrame::new(d, [-10.0, -10.0, -10.0], [10.0, 10.0, 10.0], 64);
        // PeelFrame::new for d = +z: r = +x, u = +y (pixel y grows with world y); the game's up = −u.
        // NDC (x right, y up = −pixel y): the quad (−100,−100) → (100,−100) → (100,100) is counter-clockwise
        // in world xy, i.e. clockwise in NDC (y flipped) → wind it the other way for a front face
        let far: Vec<V3> = vec![[-100.0, -100.0, 1000.0], [100.0, -100.0, 1000.0], [100.0, 100.0, 1000.0], [-100.0, 100.0, 1000.0]];
        let near: Vec<V3> = far.iter().map(|p| [p[0], p[1], -1000.0]).collect();
        let mut pos = far.clone();
        pos.extend(near.iter().copied());
        let uv: Vec<[f32; 2]> = vec![[0.0, 0.5], [1.0, 0.5], [1.0, 0.5], [0.0, 0.5], [0.0, 0.9], [1.0, 0.9], [1.0, 0.9], [0.0, 0.9]];
        // far: reversed winding (front); near: the mirror facet whose inner face turns away (back)
        let mesh = DomeMesh { pos, uv, indices: vec![0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7] };
        let r = mesh.rasterise(&frame, [0.0; 3], 0.0, -1.0, false);
        assert_eq!(r.triangles(), 2, "exactly the far facet's two triangles render");
        let (uv, view) = r.at(32, 32).expect("the centre pixel is covered");
        assert!((uv[1] - 0.5).abs() < 1e-4, "the far quad's v, not the near quad's: {uv:?}");
        assert!((uv[0] - 0.5).abs() < 0.01, "u linear across the facet: {}", uv[0]);
        assert!((view[2] - 1000.0).abs() < 1e-2, "the view vector is world − eye: {view:?}");
        // every pixel of the frame is covered by the far facet
        assert!((0..64).all(|y| (0..64).all(|x| r.covered(x, y))));
    }

    #[test]
    fn the_vertex_shader_shifts_u_by_the_sun_and_inverts_v() {
        let d = [0.0f32, 0.0, 1.0];
        let frame = PeelFrame::new(d, [-10.0, -10.0, -10.0], [10.0, 10.0, 10.0], 16);
        let pos: Vec<V3> = vec![[-100.0, -100.0, 1000.0], [100.0, -100.0, 1000.0], [100.0, 100.0, 1000.0], [-100.0, 100.0, 1000.0]];
        let mesh = DomeMesh { pos, uv: vec![[0.3, 0.2]; 4], indices: vec![0, 2, 1, 0, 3, 2] };
        let r = mesh.rasterise(&frame, [0.0; 3], 0.8137630820274353, -1.0, true);
        let (uv, _) = r.at(8, 8).expect("covered");
        assert!((uv[0] - (0.3 - 0.8137630820274353)).abs() < 1e-5, "{uv:?}");
        assert!((uv[1] - 0.8).abs() < 1e-5, "{uv:?}");
        // ForceX ≥ 0 forces u; InvertY off keeps v
        let r2 = mesh.rasterise(&frame, [0.0; 3], 0.8137630820274353, 0.25, false);
        let (uv2, _) = r2.at(8, 8).unwrap();
        assert!((uv2[0] - 0.25).abs() < 1e-6 && (uv2[1] - 0.2).abs() < 1e-6, "{uv2:?}");
    }

    #[test]
    fn the_lazy_raster_answers_as_the_filled_one() {
        let d = [0.0f32, 0.0, 1.0];
        let mut frame = PeelFrame::new(d, [-10.0, -10.0, -10.0], [10.0, 10.0, 10.0], 96);
        frame.inset_px = 1;
        // two far facets side by side (front-facing: reversed winding, as the first test) whose shared edge
        // crosses the frame, plus a back-facing one
        let pos: Vec<V3> = vec![[-100.0, -100.0, 1000.0], [3.3, -100.0, 1000.0], [3.3, 100.0, 1000.0], [-100.0, 100.0, 1000.0], [100.0, -100.0, 1000.0], [100.0, 100.0, 1000.0], [-100.0, -100.0, -1000.0], [100.0, -100.0, -1000.0], [100.0, 100.0, -1000.0]];
        let uv: Vec<[f32; 2]> = pos.iter().map(|p| [p[0] * 0.001 + 0.5, p[1] * 0.001 + 0.5]).collect();
        let mesh = DomeMesh { pos, uv, indices: vec![0, 2, 1, 0, 3, 2, 1, 5, 4, 1, 2, 5, 6, 7, 8] };
        let filled = mesh.rasterise_filled(&frame, [0.0; 3], 0.3, -1.0, true);
        let lazy = mesh.rasterise(&frame, [0.0; 3], 0.3, -1.0, true);
        assert_eq!(filled.triangles(), lazy.triangles());
        let mut covered = 0;
        for y in 0..96 { for x in 0..96 {
            assert_eq!(filled.covered(x, y), lazy.covered(x, y), "({x},{y})");
            if let (Some(a), Some(b)) = (filled.at(x, y), lazy.at(x, y)) { assert_eq!(a.0, b.0, "({x},{y})"); assert_eq!(a.1, b.1); covered += 1; }
        } }
        assert!(covered > 8000, "the facets cover the frame but the inset ring: {covered}");
    }

    #[test]
    fn barycentrics_are_the_edge_functions_over_the_area() {
        let p = [[0.0f32, 0.0], [4.0, 0.0], [0.0, 4.0]];
        let b = barycentrics(p, [1.0, 1.0]);
        assert!((b[0] - 0.5).abs() < 1e-6 && (b[1] - 0.25).abs() < 1e-6 && (b[2] - 0.25).abs() < 1e-6, "{b:?}");
        // the vertices themselves
        let b0 = barycentrics(p, [0.0, 0.0]);
        assert!((b0[0] - 1.0).abs() < 1e-6 && b0[1].abs() < 1e-6 && b0[2].abs() < 1e-6);
        // the same weights whatever the winding
        let q = [[0.0f32, 0.0], [0.0, 4.0], [4.0, 0.0]];
        let bq = barycentrics(q, [1.0, 1.0]);
        assert!((bq[0] - 0.5).abs() < 1e-6 && (bq[1] - 0.25).abs() < 1e-6 && (bq[2] - 0.25).abs() < 1e-6, "{bq:?}");
    }
}

impl DomeMesh {
    /// THE DOME MESH FROM THE PACKS: the decoration Scene3d places `Sky\Media\Solid\SkyDomeMirror.Solid.Gbx` (Maniaplanet.pak,
    /// CPlugSolid) — mapgeom's scene3d collector yields its triangles under the "Tech3 Sky" material with the world transform
    /// applied and, since the collector carries the first texture coordinate set, the gradient (u, v) PS 16774 reads.
    pub fn from_scene3d(store: &mut mapgeom::store::DataStore, scene3d_path: &str) -> Result<DomeMesh, String> {
        // the generic model walk (mapgeom's `model` command): the Scene3d's tree with its external solids — the sky dome among them
        let model = store.load_model(scene3d_path)?;
        let mut c = mapgeom::geom::Collector::new(store);
        c.model(&model, &mapgeom::geom::IDENTITY, 0);
        let (name, g) = c.scene.groups.iter().find(|(n, _)| n.to_ascii_lowercase().contains("sky")).ok_or_else(|| format!("{scene3d_path}: no Sky group among {:?}", c.scene.groups.keys().collect::<Vec<_>>()))?;
        if g.uvs.len() != g.verts.len() {
            return Err(format!("{name}: {} vertices but {} texture coordinates", g.verts.len(), g.uvs.len()));
        }
        let mut indices = Vec::with_capacity(g.tris.len() * 3);
        for t in &g.tris {
            for &i in t {
                if i > u16::MAX as u32 {
                    return Err(format!("{name}: vertex index {i} beyond u16"));
                }
                indices.push(i as u16);
            }
        }
        Ok(DomeMesh { pos: g.verts.clone(), uv: g.uvs.clone(), indices })
    }

    /// THE DOME MESH FROM THE COLLECTION'S ENVIRONMENT BLOCK (RE 9's `mapgeom::envblock`: the decoration Scene3d's SkyDome mobil —
    /// BlueBay / GreenCoast SkyDomeMirror at the origin, RedIsland / WhiteShore at (1024, 0, 1024), Stadium's Base16x12 Scene3d
    /// with SkyDomeDouble at (0, 3000, 0) — Maniaplanet.pak carries the solids): the leaf's world positions, uv0 and indices.
    pub fn from_envblock(store: &mut mapgeom::store::DataStore, collection: &str) -> Result<DomeMesh, String> {
        let env = mapgeom::envblock::load(store, collection)?;
        let leaf = env.sky_dome().ok_or_else(|| format!("{collection}: no sky dome leaf in the environment block"))?;
        if leaf.uv0.len() != leaf.positions.len() {
            return Err(format!("{}: {} vertices but {} texture coordinates", leaf.solid, leaf.positions.len(), leaf.uv0.len()));
        }
        let mut indices = Vec::with_capacity(leaf.indices.len());
        for &i in &leaf.indices {
            if i > u16::MAX as u32 { return Err(format!("{}: vertex index {i} beyond u16", leaf.solid)); }
            indices.push(i as u16);
        }
        Ok(DomeMesh { pos: leaf.world_positions(), uv: leaf.uv0.clone(), indices })
    }

    /// The mesh's RING TABLE: the distinct vertex heights (world y, rounded to 0.01 m) with the v range of the vertices on each
    /// (and the u range) — the dome's elevation → texture-v mapping as the asset defines it (port engineer G: the Stadium
    /// SkyDomeDouble vs the BlueBay SkyDomeMirror).
    pub fn rings(&self) -> Vec<(f32, usize, f32, f32, f32, f32)> {
        let mut m: std::collections::BTreeMap<i64, (usize, f32, f32, f32, f32)> = std::collections::BTreeMap::new();
        for (p, uv) in self.pos.iter().zip(self.uv.iter()) {
            let e = m.entry((p[1] * 100.0).round() as i64).or_insert((0, f32::MAX, f32::MIN, f32::MAX, f32::MIN));
            e.0 += 1;
            e.1 = e.1.min(uv[1]);
            e.2 = e.2.max(uv[1]);
            e.3 = e.3.min(uv[0]);
            e.4 = e.4.max(uv[0]);
        }
        m.into_iter().map(|(y, (n, v0, v1, u0, u1))| (y as f32 / 100.0, n, v0, v1, u0, u1)).collect()
    }

    /// Compare two dome meshes as triangle sets (the vertex order may differ): triangles whose three (position, uv) match.
    pub fn compare(&self, other: &DomeMesh) -> (usize, usize, usize) {
        let key = |m: &DomeMesh, i: u16| -> ([i64; 3], [i64; 2]) { let p = m.pos[i as usize]; let u = m.uv[i as usize]; ([(p[0] * 64.0).round() as i64, (p[1] * 64.0).round() as i64, (p[2] * 64.0).round() as i64], [(u[0] * 1e6).round() as i64, (u[1] * 1e6).round() as i64]) };
        let set: std::collections::HashSet<Vec<([i64; 3], [i64; 2])>> = other.indices.chunks_exact(3).map(|t| { let mut v = vec![key(other, t[0]), key(other, t[1]), key(other, t[2])]; v.sort(); v }).collect();
        let mut same = 0;
        for t in self.indices.chunks_exact(3) {
            let mut v = vec![key(self, t[0]), key(self, t[1]), key(self, t[2])];
            v.sort();
            if set.contains(&v) { same += 1; }
        }
        (same, self.indices.len() / 3, other.indices.len() / 3)
    }
}
