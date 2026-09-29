//! The chart raster: the lightmapper renders each object's geometry in its TexCoord1 (lightmap UV)
//! space into the object's packed rect of the atlas, at `ss` sub-samples per axis (the per-quality
//! `cSamplePerAxe`: 1 / 2 / 3 / 3 / 3 / 3 — `LM01_Scale_RasterSS`), then `LmSSResolve_*` box-averages
//! the ss² sub-texels and `LmSSNormWithA` divides the accumulated rgb by the accumulated weight.
//!
//! The rect ↔ UV mapping is the game's `ST = ((x + 0.5)/W, (w − 1)/W)`: local u = 0 sits on the centre
//! of the rect's first texel and u = 1 on the centre of its last, so in chart-local pixels
//! `px = 0.5 + u·(w − 1)`, and at ss× that is `ss·px`. Every covered sub-sample carries the world
//! position and the interpolated normal of its surface point (orthographic → linear barycentrics).

use crate::geometry::{add, mul, norm, xf_normal, xf_point, Scene, V3};
use crate::raster;

/// One covered sub-sample of a chart.
#[derive(Clone, Copy, Debug)]
pub struct Sub {
    /// Sub-pixel coordinates in the ss× raster of the chart (0..w·ss, 0..h·ss).
    pub sx: u32,
    pub sy: u32,
    pub p: V3,
    pub n: V3,
    /// Triangle index in the model (the receiver's own surface, for self-hit filtering).
    pub tri: u32,
}

/// A chart's raster: `subs` in raster order (row-major over sub-pixels, at most one per sub-pixel —
/// the last triangle covering a sub-pixel centre wins, as a depth-less GPU raster would; UV charts of
/// the game's objects do not overlap), plus per-texel counts.
pub struct ChartRaster {
    pub w: u32,
    pub h: u32,
    pub ss: u32,
    pub subs: Vec<Sub>,
    /// Number of covered sub-samples per texel (row-major w×h): the resolve weight ("alpha").
    pub count: Vec<u8>,
}

impl ChartRaster {
    /// Texels with at least one covered sub-sample.
    pub fn covered(&self) -> Vec<bool> {
        self.count.iter().map(|c| *c > 0).collect()
    }
}

/// Rasterise instance `ii`'s chart of w×h texels at `ss` sub-samples per axis. `flip_v` and
/// `use_bounds` are the atlas conventions the rest of the baker uses (the PreLightGen uv bounds
/// normalise the object's TexCoord1 to [0, 1]).
pub fn raster_chart(scene: &Scene, ii: usize, w: u32, h: u32, ss: u32, flip_v: bool, use_bounds: bool) -> ChartRaster {
    raster_chart_shifted(scene, ii, w, h, ss, flip_v, use_bounds, [0.0, 0.0])
}

/// `raster_chart` with the geometry shifted by `shift` layout pixels before the raster — the game's
/// `LM01_Trans_RasterSS` sub-texel jitter (the pixel centre then samples the geometry at centre − shift).
pub fn raster_chart_shifted(scene: &Scene, ii: usize, w: u32, h: u32, ss: u32, flip_v: bool, use_bounds: bool, shift: [f32; 2]) -> ChartRaster {
    if w == 0 || h == 0 { return ChartRaster { w, h, ss, subs: Vec::new(), count: Vec::new() }; }
    let inst = &scene.instances[ii];
    let m = &scene.models[inst.model];
    let (u0, v0, su, sv) = match (use_bounds, m.plg_bounds) {
        (true, Some(b)) => (b[0], b[1], 1.0 / (b[2] - b[0]), 1.0 / (b[3] - b[1])),
        _ => (0.0, 0.0, 1.0, 1.0),
    };
    let ss = ss.max(1);
    let (rw, rh) = (w * ss, h * ss);
    let mut slot: Vec<u32> = vec![u32::MAX; (rw * rh) as usize];
    let mut subs: Vec<Sub> = Vec::new();
    let fss = ss as f32;
    for (ti, t) in m.tris.iter().enumerate() {
        // a material whose PreLightGen switch skips the map has no lightmap texels (the flag rule; `ModelGeom::mat_no_lm`)
        if m.tri_no_lm(t) { continue; }
        let pix: Vec<[f32; 2]> = t
            .uv
            .iter()
            .map(|uv| {
                let u = (uv[0] - u0) * su;
                let v = (uv[1] - v0) * sv;
                let v = if flip_v { 1.0 - v } else { v };
                [fss * (0.5 + u * (w as f32 - 1.0) + shift[0]), fss * (0.5 + v * (h as f32 - 1.0) + shift[1])]
            })
            .collect();
        let wp = [xf_point(&inst.xf, t.p[0]), xf_point(&inst.xf, t.p[1]), xf_point(&inst.xf, t.p[2])];
        let wn = [xf_normal(&inst.xf, t.n[0]), xf_normal(&inst.xf, t.n[1]), xf_normal(&inst.xf, t.n[2])];
        raster::triangle(rw, rh, [pix[0], pix[1], pix[2]], |sx, sy, b| {
            let p = add(add(mul(wp[0], b[0]), mul(wp[1], b[1])), mul(wp[2], b[2]));
            let n = norm(add(add(mul(wn[0], b[0]), mul(wn[1], b[1])), mul(wn[2], b[2])));
            let i = (sy * rw + sx) as usize;
            let s = Sub { sx, sy, p, n, tri: ti as u32 };
            if slot[i] == u32::MAX {
                slot[i] = subs.len() as u32;
                subs.push(s);
            } else {
                subs[slot[i] as usize] = s;
            }
        });
    }
    let mut count = vec![0u8; (w * h) as usize];
    for s in &subs {
        count[((s.sy / ss) * w + s.sx / ss) as usize] += 1;
    }
    ChartRaster { w, h, ss, subs, count }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Instance, ModelGeom, Tri};

    fn quad_scene() -> Scene {
        // one 8 m × 8 m quad at y = 0 with uv1 = the unit square
        let p = |x: f32, z: f32| [x, 0.0, z];
        let n = [0.0, 1.0, 0.0];
        let tris = vec![
            Tri { p: [p(0.0, 0.0), p(8.0, 0.0), p(8.0, 8.0)], n: [n; 3], uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], uv0: [[0.0; 2]; 3], alpha: u16::MAX, mat: u16::MAX, diff: u16::MAX },
            Tri { p: [p(0.0, 0.0), p(8.0, 8.0), p(0.0, 8.0)], n: [n; 3], uv: [[0.0, 0.0], [1.0, 1.0], [0.0, 1.0]], uv0: [[0.0; 2]; 3], alpha: u16::MAX, mat: u16::MAX, diff: u16::MAX },
        ];
        let m = ModelGeom { tris, metres_per_uv: 8.0, uv_min: [0.0, 0.0], uv_max: [1.0, 1.0], ..ModelGeom::default() };
        Scene {
            models: vec![m],
            model_names: vec!["quad".into()],
            instances: vec![Instance { item: 0, model: 0, xf: crate::geometry::identity_xf(), model_name: "quad".into(), pose: crate::geometry::ItemPose { scale: 1.0, ..Default::default() }, lm_quality: 0, colour: 0 }],
            item_count: 1,
            decor: Vec::new(),
            warp_vs: Vec::new(),
            warp: None,
            alpha_masks: Default::default(),
            card_albedo: Default::default(),
            tex_albedo: Default::default(), stock_models: Default::default(), veget_poses: Vec::new() }
    }

    #[test]
    fn unit_quad_chart_interior_full_boundary_partial() {
        // the ST convention puts u = 0 / u = 1 exactly on the first / last texel CENTRES, so the
        // boundary texels' centres lie on the geometry's edges: the top-left rule keeps the top/left
        // ones and drops the bottom/right ones at ss = 1, and at ss ≥ 2 every boundary texel keeps some
        // of its sub-samples (the resolve then normalises by the count) — interior texels get all ss²
        let s = quad_scene();
        for ss in [1u32, 2, 3] {
            let r = raster_chart(&s, 0, 8, 8, ss, false, false);
            for y in 1..7u32 {
                for x in 1..7u32 {
                    assert_eq!(r.count[(y * 8 + x) as usize] as u32, ss * ss, "ss {ss}: interior texel ({x},{y})");
                }
            }
            if ss >= 2 {
                assert!(r.count.iter().all(|c| *c > 0), "ss {ss}: every texel keeps a sub-sample, got {:?}", &r.count);
            } else {
                assert!(r.count[0] > 0 && r.count[(7 * 8 + 7) as usize] == 0, "ss 1: top-left corner kept, bottom-right dropped by the fill rule");
            }
        }
    }

    #[test]
    fn texel_centres_map_to_the_st_convention() {
        // local u = 0 at the first texel's centre, u = 1 at the last: texel x's centre is at u = x/(w−1)
        let s = quad_scene();
        let r = raster_chart(&s, 0, 8, 8, 1, false, false);
        let first = r.subs.iter().find(|q| q.sx == 0 && q.sy == 0).unwrap();
        let last = r.subs.iter().find(|q| q.sx == 6 && q.sy == 6).unwrap();
        assert!((first.p[0] - 0.0).abs() < 1e-4 && (first.p[2] - 0.0).abs() < 1e-4);
        assert!((last.p[0] - 8.0 * 6.0 / 7.0).abs() < 1e-4 && (last.p[2] - 8.0 * 6.0 / 7.0).abs() < 1e-4);
        let mid = r.subs.iter().find(|q| q.sx == 3 && q.sy == 0).unwrap();
        assert!((mid.p[0] - 8.0 * 3.0 / 7.0).abs() < 1e-4);
    }
}
