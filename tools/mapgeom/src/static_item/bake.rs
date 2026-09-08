//! Baking a `CPlugCrystal` (a Nadeo crystal item's mesh) into static-item
//! geometry: the enabled, visible Geometry layers' faces are fan-triangulated
//! into one visual per material (positions, flat normals, UVs, tangents in
//! the reference vertex layout), the collidable layers' faces (or, failing
//! that, the visible faces) become the collision mesh.

use super::build::Merged;
use crate::crystal_model::CPlugMaterialUserInst;
use super::R;
use crate::crystal_model::{CPlugCrystal, Crystal, LayerKind};

/// How one material family is smoothed and welded: the per-material recipe
/// fitted against Granady's reference items in stage 1/2 (2026-09-05/06,
/// `partdiff`/`triseq`/`uweight` measurements on Tiny_Road_17, BRoad3,
/// Snow2…). It lived in `/tmp/tiny3/recipe.env` as TINY_CREASE_MAP /
/// TINY_WEIGHT_MAP / TINY_NMODE_MAP / TINY_UMODE_MAP / TINY_TAN_MAP /
/// TINY_UMAG_MAP until 2026-09-08; it is the one setting there ever was, so
/// it is a constant now. A material not listed takes [`DEFAULT_RECIPE`].
#[derive(Clone, Copy, Debug)]
pub struct MaterialRecipe {
    /// The material link's stem (`TrackBorders` of `Stadium\Media\Material\TrackBorders`).
    pub stem: &'static str,
    /// Crease angle (degrees) within which face normals at one position
    /// average; harder creases stay split. Seed-largest plateaus per
    /// material: TrackBorders ~58–65, Trims ~45, TSpecials ~43–55, Technics
    /// ~52, Road ~20, SFX ~39–55 — no one angle fits all six.
    pub crease_deg: f32,
    /// Corner-angle weighted normal average instead of uniform (angle beats
    /// uniform on TrackBorders 98 % vs 62 %, Technics 70 vs 29, TSpecials 69
    /// vs 60; Road/Sign stay uniform — already bit-exact that way).
    pub angle_weight: bool,
    /// Per-corner threshold averaging ([`smooth_normals_corner`]) instead of
    /// clustering ([`smooth_normals_angle`]) — the rule read off his
    /// TSpecials partitions.
    pub corner_mode: bool,
    /// Which uv gradient is the primary tangent axis: `du` (proven for
    /// Road/TB/Technics coils), `range` (whichever spans more uv), `duoff`
    /// (du frames, but no U-key splits: TrackBorders' det-coincident
    /// twins over-split), `off` (range frames, no U-key splits).
    pub umode: &'static str,
    /// U-cluster smoothing angle (degrees) for the V-primary frames.
    pub tan_deg: f32,
    /// MikkTSpace-style |du-gradient| weights in the U-cluster average
    /// (TSpecials: 88.8 % vs 85.0 %).
    pub umag_weight: bool,
}

/// The recipe of a material nobody measured: 12° crease, uniform weights,
/// clustering, range-primary frames, 40° U clusters.
pub const DEFAULT_RECIPE: MaterialRecipe = MaterialRecipe { stem: "", crease_deg: 12.0, angle_weight: false, corner_mode: false, umode: "range", tan_deg: 40.0, umag_weight: false };

/// The measured materials.
pub const RECIPE: &[MaterialRecipe] = &[
    MaterialRecipe { stem: "TrackBorders", crease_deg: 65.0, angle_weight: true, corner_mode: false, umode: "duoff", tan_deg: 40.0, umag_weight: false },
    MaterialRecipe { stem: "TechnicsTrims", crease_deg: 45.0, angle_weight: true, corner_mode: false, umode: "du", tan_deg: 40.0, umag_weight: false },
    MaterialRecipe { stem: "TechnicsSpecials", crease_deg: 55.0, angle_weight: false, corner_mode: true, umode: "du", tan_deg: 60.0, umag_weight: true },
    MaterialRecipe { stem: "Technics", crease_deg: 52.0, angle_weight: true, corner_mode: false, umode: "du", tan_deg: 12.0, umag_weight: false },
    MaterialRecipe { stem: "RoadTech", crease_deg: 20.0, angle_weight: false, corner_mode: false, umode: "range", tan_deg: 40.0, umag_weight: false },
    MaterialRecipe { stem: "SpecialFXTurbo", crease_deg: 55.0, angle_weight: false, corner_mode: true, umode: "range", tan_deg: 40.0, umag_weight: false },
    MaterialRecipe { stem: "LightSpot", crease_deg: 44.0, angle_weight: false, corner_mode: false, umode: "range", tan_deg: 40.0, umag_weight: false },
    MaterialRecipe { stem: "DecalMarksRamp", crease_deg: 44.0, angle_weight: false, corner_mode: false, umode: "range", tan_deg: 40.0, umag_weight: false },
    MaterialRecipe { stem: "TrackWallClips", crease_deg: 12.0, angle_weight: false, corner_mode: false, umode: "du", tan_deg: 40.0, umag_weight: false },
];

/// The recipe of a material link (by its stem).
pub fn recipe_for(link: &str) -> MaterialRecipe {
    let stem = link.rsplit('\\').next().unwrap_or(link);
    RECIPE.iter().copied().find(|r| r.stem == stem).unwrap_or(DEFAULT_RECIPE)
}

/// One triangle corner with everything the vertex layout needs.
#[derive(Clone, Copy, Debug)]
pub struct Corner {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Lightmap UV, assigned by [`assign_lightmap_uvs`] (a weld-preserving
    /// global planar projection in [0,1]^2). Defaults to the diffuse UV.
    pub uv1: [f32; 2],
    /// Smoothed tangent basis (U/V), assigned by [`smooth_tangents_angle`]
    /// (seed-largest clustering on face tangents, per-material angle).
    /// Defaults to per-face tangent (set by [`face_triangles`]).
    pub tan_u: [f32; 3],
    pub tan_v: [f32; 3],
    /// Source polygon id (per material stream; the fan tris of one crystal
    /// face share it). 0 when unknown. Used for one-vote-per-face normal
    /// averaging (`smooth_normals_corner`).
    pub face: u32,
    /// Source crystal group (`Face::group`); lightmap charts never cross
    /// groups (measured on every Tiny_Road_17 material). 0 when unknown.
    pub group: u32,
}

/// Triangle area (for area-weighted smoothing): borrows one tri only.
fn tri_area_of(t: &[Corner; 3]) -> f32 {
    let e1 = [t[1].pos[0] - t[0].pos[0], t[1].pos[1] - t[0].pos[1], t[1].pos[2] - t[0].pos[2]];
    let e2 = [t[2].pos[0] - t[0].pos[0], t[2].pos[1] - t[0].pos[1], t[2].pos[2] - t[0].pos[2]];
    let cr = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt() / 2.0
}

/// |du-gradient| of a tri (unnormalized MikkTSpace-style magnitude):
/// weight for U-cluster averaging (uweight: mag-weighted beats uniform
/// 88.8% vs 85.0% on TSpecials, ties elsewhere). Slivers (|det|->0)
/// dominate, matching his pipeline.
fn du_grad_mag(t: &[Corner; 3]) -> f64 {
    let e1 = sub(t[1].pos, t[0].pos);
    let e2 = sub(t[2].pos, t[0].pos);
    let du1 = t[1].uv[0] - t[0].uv[0];
    let dv1 = t[1].uv[1] - t[0].uv[1];
    let du2 = t[2].uv[0] - t[0].uv[0];
    let dv2 = t[2].uv[1] - t[0].uv[1];
    let det = du1 * dv2 - du2 * dv1;
    if det.abs() < 1e-12 {
        return 1.0;
    }
    let r = 1.0 / det;
    let tx = (e1[0] * dv2 - e2[0] * dv1) * r;
    let ty = (e1[1] * dv2 - e2[1] * dv1) * r;
    let tz = (e1[2] * dv2 - e2[2] * dv1) * r;
    ((tx * tx + ty * ty + tz * tz) as f64).sqrt().max(1e-30)
}
/// Corner angle at corner k of a tri (radians): the standard
/// angle-weight for vertex-normal averaging (wavtest: angle-weighted
/// beats uniform TB 98.1% vs 61.9%, Technics 69.8% vs 29.2%, TSpecials
/// 68.7% vs 59.6%; area loses to uniform everywhere).
fn corner_angle(t: &[Corner; 3], k: usize) -> f32 {
    let p = t[k].pos;
    let a = t[(k + 1) % 3].pos;
    let b = t[(k + 2) % 3].pos;
    let v1 = [a[0] - p[0], a[1] - p[1], a[2] - p[2]];
    let v2 = [b[0] - p[0], b[1] - p[1], b[2] - p[2]];
    let l1 = (v1[0] * v1[0] + v1[1] * v1[1] + v1[2] * v1[2]).sqrt().max(1e-30);
    let l2 = (v2[0] * v2[0] + v2[1] * v2[1] + v2[2] * v2[2]).sqrt().max(1e-30);
    ((v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2]) / (l1 * l2)).clamp(-1.0, 1.0).acos()
}

/// Average face normals at shared positions within a crease angle (in
/// place): faces sharing a position whose normals agree within `max_deg`
/// share one smoothed normal; harder creases stay split. Full smoothing
/// (`max_deg=180`) over-shares (Road_17: 9750 vs his 14030); flat normals
/// (`max_deg=0`) split everything (35184). Default 30 deg.
/// Per-CORNER threshold averaging (non-transitive, no clustering): each
/// corner's normal is the weighted mean of the face normals at its position
/// whose angle to ITS OWN face normal is <= max_deg. Corners with the same
/// set weld; corners whose sets differ split even when their faces are
/// nearly coplanar. Read off his Tiny_Road_17 TSpecials partitions with
/// `partdiff` (2026-09-06): at one vertex three near-coplanar tris (1-4deg
/// apart) split 1+2 because only two of them sit within theta of the flat
/// faces; the cluster model cannot produce that.
pub fn smooth_normals_corner(tris: &mut [[Corner; 3]], max_deg: f32, angle_weight: bool) {
    use std::collections::BTreeMap;
    if max_deg <= 0.0 {
        return;
    }
    let cos_max = (max_deg.to_radians()).cos();
    let mut by_pos: BTreeMap<[u32; 3], Vec<(usize, usize)>> = BTreeMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for (k, c) in t.iter().enumerate() {
            by_pos.entry([c.pos[0].to_bits(), c.pos[1].to_bits(), c.pos[2].to_bits()]).or_default().push((ti, k));
        }
    }
    for (_, corners) in by_pos {
        if corners.len() < 2 {
            continue;
        }
        let ns: Vec<[f32; 3]> = corners.iter().map(|&(ti, k)| tris[ti][k].normal).collect();
        let angles: Vec<f32> = corners.iter().map(|&(ti, k)| corner_angle(&tris[ti], k)).collect();
        let mut out: Vec<[f32; 3]> = Vec::with_capacity(corners.len());
        for i in 0..corners.len() {
            let me = ns[i];
            let mut sel: Vec<usize> = (0..corners.len()).filter(|&j| me[0] * ns[j][0] + me[1] * ns[j][1] + me[2] * ns[j][2] >= cos_max).collect();
            // one vote per source polygon (Corner::face): a quad's two fan
            // triangles share one Newell normal and count once
            let mut seen: Vec<u32> = Vec::new();
            sel.retain(|&j| {
                let f = tris[corners[j].0][corners[j].1].face;
                if seen.contains(&f) {
                    false
                } else {
                    seen.push(f);
                    true
                }
            });
            let sn: Vec<[f32; 3]> = sel.iter().map(|&j| ns[j]).collect();
            let sg: Vec<f32> = sel.iter().map(|&j| angles[j]).collect();
            out.push(cluster_normal_value(&sn, &sg, angle_weight));
        }
        for (i, &(ti, k)) in corners.iter().enumerate() {
            tris[ti][k].normal = out[i];
        }
    }
}

/// Average face normals at shared positions within a crease angle (in
/// place), clustering by normal agreement: the corners at one position are
/// visited largest triangle first, each joining the first cluster whose seed
/// normal is within `max_deg` of its own, else seeding a new one (no chaining
/// through members — single-linkage over-shared; seed-largest measured
/// closest to his clusters on Road_17 and is what the per-material crease
/// plateaus assume). Full smoothing over-shares (Road_17: 9750 vs his
/// 14030), flat normals split everything (35184).
pub fn smooth_normals_angle(tris: &mut [[Corner; 3]], max_deg: f32, angle_weight: bool) {
    use std::collections::BTreeMap;
    if max_deg <= 0.0 {
        return;
    }
    let cos_max = (max_deg.to_radians()).cos();
    // group corners by position
    let mut by_pos: BTreeMap<[u32; 3], Vec<(usize, usize)>> = BTreeMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for (k, c) in t.iter().enumerate() {
            by_pos.entry([c.pos[0].to_bits(), c.pos[1].to_bits(), c.pos[2].to_bits()]).or_default().push((ti, k));
        }
    }
    for (_, corners) in by_pos {
        if corners.len() < 2 {
            continue;
        }
        let mut ord: Vec<usize> = (0..corners.len()).collect();
        ord.sort_by(|a, b| tri_area_of(&tris[corners[*b].0]).partial_cmp(&tri_area_of(&tris[corners[*a].0])).unwrap());
        let mut cluster_of: Vec<usize> = vec![usize::MAX; corners.len()];
        let mut seeds: Vec<[f32; 3]> = Vec::new();
        for &ci in &ord {
            let (ti, k) = corners[ci];
            let n = tris[ti][k].normal;
            match seeds.iter().position(|s| s[0] * n[0] + s[1] * n[1] + s[2] * n[2] >= cos_max) {
                Some(c) => cluster_of[ci] = c,
                None => {
                    cluster_of[ci] = seeds.len();
                    seeds.push(n);
                }
            }
        }
        for c in 0..seeds.len() {
            let members: Vec<(usize, usize)> = corners.iter().enumerate().filter(|(i, _)| cluster_of[*i] == c).map(|(_, v)| *v).collect();
            if members.is_empty() {
                continue;
            }
            let ns: Vec<[f32; 3]> = members.iter().map(|&(ti, k)| tris[ti][k].normal).collect();
            let angles: Vec<f32> = members.iter().map(|&(ti, k)| corner_angle(&tris[ti], k)).collect();
            let n = cluster_normal_value(&ns, &angles, angle_weight);
            for &(ti, k) in &members {
                tris[ti][k].normal = n;
            }
        }
    }
}

/// A cluster's normal: the members' mean, uniform or corner-angle weighted
/// (`MaterialRecipe::angle_weight`).
fn cluster_normal_value(ns: &[[f32; 3]], angles: &[f32], use_angle: bool) -> [f32; 3] {
    let mut acc = [0.0f64; 3];
    let mut wsum = 0.0f64;
    for (i, n) in ns.iter().enumerate() {
        let w = if use_angle { angles[i] as f64 } else { 1.0 };
        for d in 0..3 {
            acc[d] += n[d] as f64 * w;
        }
        wsum += w;
    }
    normalize([(acc[0] / wsum) as f32, (acc[1] / wsum) as f32, (acc[2] / wsum) as f32])
}
/// V-primary tangent frames (per corner, no smoothing): V from the
/// dv-gradient, U = N x V, V = N x U (re-derived). Measured: his U
/// equals N x V_grad (Tri B: (0.0029,-0.3875,-0.9198) vs his
/// (0.002,-0.387,-0.920)), i.e. V is primary (from dv), U derived --
/// not du-gradient + Gram-Schmidt. Degenerate (det~=0) corners keep
/// zero tangents (fallback downstream).
pub fn tangents_vprim(tris: &mut [[Corner; 3]], mode: u8) {
    for t in tris.iter_mut() {
        let e1 = sub(t[1].pos, t[0].pos);
        let e2 = sub(t[2].pos, t[0].pos);
        let du1 = t[1].uv[0] - t[0].uv[0];
        let dv1 = t[1].uv[1] - t[0].uv[1];
        let du2 = t[2].uv[0] - t[0].uv[0];
        let dv2 = t[2].uv[1] - t[0].uv[1];
        let det = du1 * dv2 - du2 * dv1;
        if det.abs() < 1e-12 {
            // Degenerate uv: fallback U = sign(det) * (up x N) with
            // up=(0,1,0) (measured: two det~=0 tris at one position give
            // opposite +-x frames; sign from signed-zero det).
            let up = [0.0f32, 1.0, 0.0];
            for c in t.iter_mut() {
                let n = c.normal;
                let fx = up[1] * n[2] - up[2] * n[1];
                let fy = up[2] * n[0] - up[0] * n[2];
                let fz = up[0] * n[1] - up[1] * n[0];
                let fl = (fx * fx + fy * fy + fz * fz).sqrt().max(1e-30);
                // (is_sign_negative distinguishes det=+0 from det=-0.)
                let s = if det.is_sign_negative() { -1.0 } else { 1.0 };
                let u = [s * fx / fl, s * fy / fl, s * fz / fl];
                let wx = n[1] * u[2] - n[2] * u[1];
                let wy = n[2] * u[0] - n[0] * u[2];
                let wz = n[0] * u[1] - n[1] * u[0];
                let wl = (wx * wx + wy * wy + wz * wz).sqrt().max(1e-30);
                c.tan_u = u;
                c.tan_v = [wx / wl, wy / wl, wz / wl];
            }
            continue;
        }
        let r = 1.0 / det;
        // Primary gradient by mode (1 = forced du, 2 = forced dv,
        // 0 = uv-range stability): du uses U=GS(du,N), V=sgn*NxU; dv uses
        // U=sgn*NxVg under USGN else NxVg, V=NxU under USGN else sgn*NxU.
        // Per-material mode via UMODE_MAP at caller. Branch MUST equal
        // tri_primary_du (U-cluster purity recomputes it).
        if tri_primary_du(t, mode) {
            // du-primary: U = GS(du-grad, N), V = sgn(det)*(NxU).
            let sdet = if det.is_sign_negative() { -1.0f32 } else { 1.0f32 };
            let tx = (e1[0] * dv2 - e2[0] * dv1) * r;
            let ty = (e1[1] * dv2 - e2[1] * dv1) * r;
            let tz = (e1[2] * dv2 - e2[2] * dv1) * r;
            let tl = (tx * tx + ty * ty + tz * tz).sqrt().max(1e-30);
            let tg = [tx / tl, ty / tl, tz / tl];
            for c in t.iter_mut() {
                let n = c.normal;
                let dd = tg[0] * n[0] + tg[1] * n[1] + tg[2] * n[2];
                let ox = tg[0] - dd * n[0];
                let oy = tg[1] - dd * n[1];
                let oz = tg[2] - dd * n[2];
                let ol = (ox * ox + oy * oy + oz * oz).sqrt().max(1e-30);
                // (du-branch U is unsigned; sign lives on V. See dv branch.)
                let u = [ox / ol, oy / ol, oz / ol];
                let wx = sdet * (n[1] * u[2] - n[2] * u[1]);
                let wy = sdet * (n[2] * u[0] - n[0] * u[2]);
                let wz = sdet * (n[0] * u[1] - n[1] * u[0]);
                let wl = (wx * wx + wy * wy + wz * wz).sqrt().max(1e-30);
                c.tan_u = u;
                c.tan_v = [wx / wl, wy / wl, wz / wl];
            }
            continue;
        }
        // dv-gradient (V direction), normalized
        let sdet = if det.is_sign_negative() { -1.0f32 } else { 1.0f32 };
        let vx = (e1[0] * du2 - e2[0] * du1) * r;
        let vy = (e1[1] * du2 - e2[1] * du1) * r;
        let vz = (e1[2] * du2 - e2[2] * du1) * r;
        let vl = (vx * vx + vy * vy + vz * vz).sqrt().max(1e-30);
        let vg = [vx / vl, vy / vl, vz / vl];
        for c in t.iter_mut() {
            let n = c.normal;
            // U = N x Vg
            let ux = n[1] * vg[2] - n[2] * vg[1];
            let uy = n[2] * vg[0] - n[0] * vg[2];
            let uz = n[0] * vg[1] - n[1] * vg[0];
            let ul = (ux * ux + uy * uy + uz * uz).sqrt().max(1e-30);
            // U unsigned; the handedness rides on V = sgn(det) * (N x U)
            let u = [ux / ul, uy / ul, uz / ul];
            let wx = sdet * (n[1] * u[2] - n[2] * u[1]);
            let wy = sdet * (n[2] * u[0] - n[0] * u[2]);
            let wz = sdet * (n[0] * u[1] - n[1] * u[0]);
            let wl = (wx * wx + wy * wy + wz * wz).sqrt().max(1e-30);
            c.tan_u = u;
            c.tan_v = [wx / wl, wy / wl, wz / wl];
        }
    }
}

/// Primary mode: 0 = range (du iff du-range >= dv-range), 1 = forced du,
/// 2 = forced dv (TSpecials: his U = sgn(det)·(N×Vg) even where du-range
/// wins -- per-face range + signed-dv exploded +1327 from mixed clusters).
fn tri_primary_du(t: &[Corner; 3], mode: u8) -> bool {
    if mode == 1 {
        return true;
    }
    if mode == 2 {
        return false;
    }
    let du_min = t[0].uv[0].min(t[1].uv[0]).min(t[2].uv[0]);
    let du_max = t[0].uv[0].max(t[1].uv[0]).max(t[2].uv[0]);
    let dv_min = t[0].uv[1].min(t[1].uv[1]).min(t[2].uv[1]);
    let dv_max = t[0].uv[1].max(t[1].uv[1]).max(t[2].uv[1]);
    (du_max - du_min) >= (dv_max - dv_min)
}
/// Smooth V-primary U within angle clusters (per position, seed-largest
/// by tri area, `MaterialRecipe::tan_deg`): averages Corner.tan_u (set by
/// [`tangents_vprim`]) where frames agree, keeps creases split. Re-derives
/// V = sgn(det)*(N x U) per corner. Measured: bevel U-frames (35° apart)
/// average to his single frame, while 180°-opposed coil frames stay split.
/// Runs after VPRIM, before welding; the weld key consumes the smoothed
/// quantized U.
pub fn smooth_u_vprim(tris: &mut [[Corner; 3]], max_deg: f32, angle_weight: bool, mag_weight: bool) {
    use std::collections::BTreeMap;
    if max_deg <= 0.0 {
        return;
    }
    let cos_max = (max_deg.to_radians()).cos();
    // U clusters group by position only (primary-purity by (pos,primary)
    // was tested: it only ADDS splits on range materials, helping nothing
    // -- Sign/SignOff regressed +4 each with zero gains elsewhere. The
    // 56deg TSpecials spot is a primary disagreement (his -dv vs our du),
    // not a mixed-cluster artifact: TSpecials is forced-du, already pure.)
    let mut by_pos: BTreeMap<[u32; 3], Vec<(usize, usize)>> = BTreeMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for (k, c) in t.iter().enumerate() {
            // (only corners carrying V-primary frames participate; zeros
            // from degenerate tris keep zero (fallback stored downstream))
            if c.tan_u == [0.0; 3] {
                continue;
            }
            by_pos
                .entry([c.pos[0].to_bits(), c.pos[1].to_bits(), c.pos[2].to_bits()])
                .or_default()
                .push((ti, k));
        }
    }
    for (_, corners) in by_pos {
        if corners.len() < 2 {
            continue;
        }
        // seed-largest by tri area
        let area = |ti: usize| tri_area_of(&tris[ti]);
        let mut ord: Vec<usize> = (0..corners.len()).collect();
        ord.sort_by(|a, b| area(corners[*b].0).partial_cmp(&area(corners[*a].0)).unwrap());
        let mut seeds: Vec<[f32; 3]> = Vec::new();
        let mut cluster_of: Vec<usize> = vec![usize::MAX; corners.len()];
        let mut nclusters = 0;
        for &oi in &ord {
            let (ti, k) = corners[oi];
            let u = tris[ti][k].tan_u;
            match seeds.iter().position(|s| s[0] * u[0] + s[1] * u[1] + s[2] * u[2] >= cos_max) {
                Some(c) => cluster_of[oi] = c,
                None => {
                    cluster_of[oi] = nclusters;
                    seeds.push(u);
                    nclusters += 1;
                }
            }
        }
        for c in 0..nclusters {
            let members: Vec<usize> = (0..corners.len()).filter(|i| cluster_of[*i] == c).collect();
            if members.is_empty() {
                continue;
            }
            let mut acc = [0.0f64; 3];
            let mut wsum = 0.0f64;
            for &oi in &members {
                let (ti, k) = corners[oi];
                let w = if mag_weight {
                    du_grad_mag(&tris[ti])
                } else if angle_weight {
                    corner_angle(&tris[ti], k) as f64
                } else {
                    1.0
                };
                for d in 0..3 {
                    acc[d] += tris[ti][k].tan_u[d] as f64 * w;
                }
                wsum += w;
            }
            let lavg = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt().max(1e-30);
            let uavg = [(acc[0] / lavg) as f32, (acc[1] / lavg) as f32, (acc[2] / lavg) as f32];
            for &oi in &members {
                let (ti, k) = corners[oi];
                tris[ti][k].tan_u = uavg;
                // re-derive V = sgn(det)*(N x U)
                let t = &tris[ti];
                let du1 = t[1].uv[0] - t[0].uv[0];
                let dv1 = t[1].uv[1] - t[0].uv[1];
                let du2 = t[2].uv[0] - t[0].uv[0];
                let dv2 = t[2].uv[1] - t[0].uv[1];
                let det = du1 * dv2 - du2 * dv1;
                let s = if det.is_sign_negative() { -1.0 } else { 1.0 };
                let nn = tris[ti][k].normal;
                let wx = s * (nn[1] * uavg[2] - nn[2] * uavg[1]);
                let wy = s * (nn[2] * uavg[0] - nn[0] * uavg[2]);
                let wz = s * (nn[0] * uavg[1] - nn[1] * uavg[0]);
                let wl = (wx * wx + wy * wy + wz * wz).sqrt().max(1e-30);
                tris[ti][k].tan_v = [wx / wl, wy / wl, wz / wl];
            }
        }
    }
}


/// Item-global lightmap atlas, the way the editor's bake lays one out
/// (measured on Tiny_Road_17 with `uv1charts`/`uv1seg`/`uv1rule`/`uv1cross`):
///
/// * ONE atlas per item: the charts of every visual with a uv1 stream are
///   packed together into [0.001, 0.999]^2 (cross-material overlaps: none
///   beyond bbox artifacts); a single uniform scale (0.01892 uv/m on
///   Road_17, i.e. shrink-to-fit -- coverage 0.21..0.74 across references).
/// * Charts never cross source groups; a uv0 seam always splits; dihedral
///   >= 50 deg always splits; charts stay within ~45-50 deg of a seed normal
///   (max tri-vs-chart-mean 35-53 deg per material), so a planar projection
///   along the chart normal cannot fold over.
/// * Charts are rotated freely (min-area box) and placed axis-aligned with
///   a small gutter (min bbox gap 0.0004).
///
/// Corners at one position inside a chart share uv1 (projection is a
/// function of position); chart borders split, like his weld structure.
/// The exact unwrapper is unknown: this reproduces the STRUCTURE (coverage,
/// non-overlap, uniform density, seam rules), not his bytes.
pub fn assign_lightmap_atlas(per_material: &mut [Vec<[Corner; 3]>], has_uv1: &[bool]) {
    use std::collections::{BTreeMap, VecDeque};
    // a chart never crosses a dihedral of 45 deg, stays within 50 deg of its
    // seed normal, keeps a 0.001 gutter from its neighbours and a 0.001
    // margin from the atlas edge (all measured on the reference items)
    let dih_max = 45.0f32.to_radians().cos();
    let seed_max = 50.0f32.to_radians().cos();
    let gutter = 0.001f32;
    let margin = 0.001f32;
    let pk = |p: [f32; 3]| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
    // A chart: material index, tri indices, and its 2D frame.
    struct Chart {
        mat: usize,
        tris: Vec<usize>,
        ex: [f32; 3],
        ey: [f32; 3],
        min: [f32; 2],
        w: f32,
        h: f32,
        // placement (atlas units), set by the packer
        x: f32,
        y: f32,
        rot90: bool,
    }
    let mut charts: Vec<Chart> = Vec::new();
    for (mi, tris) in per_material.iter().enumerate() {
        if !has_uv1.get(mi).copied().unwrap_or(false) || tris.is_empty() {
            continue;
        }
        let fnorm: Vec<[f32; 3]> = tris.iter().map(|t| normalize(cross(sub(t[1].pos, t[0].pos), sub(t[2].pos, t[0].pos)))).collect();
        // edges by position pair -> (tri, k)
        let mut edges: BTreeMap<([u32; 3], [u32; 3]), Vec<(usize, usize)>> = BTreeMap::new();
        for (ti, t) in tris.iter().enumerate() {
            for k in 0..3 {
                let (p, q) = (pk(t[k].pos), pk(t[(k + 1) % 3].pos));
                let key = if p < q { (p, q) } else { (q, p) };
                edges.entry(key).or_default().push((ti, k));
            }
        }
        // adjacency with the merge rules (group, uv0 continuity, dihedral)
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); tris.len()];
        for (_, es) in &edges {
            for i in 0..es.len() {
                for j in i + 1..es.len() {
                    let (ti, ki) = es[i];
                    let (tj, kj) = es[j];
                    if tris[ti][0].group != tris[tj][0].group {
                        continue;
                    }
                    let n1 = fnorm[ti];
                    let n2 = fnorm[tj];
                    if n1[0] * n2[0] + n1[1] * n2[1] + n1[2] * n2[2] < dih_max {
                        continue;
                    }
                    // uv0 continuity across the shared edge
                    let (a1, b1) = (&tris[ti][ki], &tris[ti][(ki + 1) % 3]);
                    let (a2, b2) = (&tris[tj][kj], &tris[tj][(kj + 1) % 3]);
                    let (a2, b2) = if pk(a1.pos) == pk(a2.pos) { (a2, b2) } else { (b2, a2) };
                    if a1.uv != a2.uv || b1.uv != b2.uv {
                        continue;
                    }
                    adj[ti].push(tj);
                    adj[tj].push(ti);
                }
            }
        }
        // seed-based BFS region growing
        let mut chart_of: Vec<usize> = vec![usize::MAX; tris.len()];
        for seed in 0..tris.len() {
            if chart_of[seed] != usize::MAX {
                continue;
            }
            let id = charts.len();
            let sn = fnorm[seed];
            let mut members = vec![seed];
            chart_of[seed] = id;
            let mut q: VecDeque<usize> = VecDeque::from(vec![seed]);
            while let Some(t) = q.pop_front() {
                for &u in &adj[t] {
                    if chart_of[u] != usize::MAX {
                        continue;
                    }
                    let n = fnorm[u];
                    if sn[0] * n[0] + sn[1] * n[1] + sn[2] * n[2] < seed_max {
                        continue;
                    }
                    chart_of[u] = id;
                    members.push(u);
                    q.push_back(u);
                }
            }
            charts.push(Chart { mat: mi, tris: members, ex: [0.0; 3], ey: [0.0; 3], min: [0.0; 2], w: 0.0, h: 0.0, x: 0.0, y: 0.0, rot90: false });
        }
    }
    // 2D frame per chart: mean normal, then min-area bounding box over the
    // convex hull of the projected positions.
    let frame = |c: &mut Chart, per_material: &[Vec<[Corner; 3]>]| {
        let tris = &per_material[c.mat];
        let mut nacc = [0.0f32; 3];
        for &ti in &c.tris {
            let t = &tris[ti];
            let cr = cross(sub(t[1].pos, t[0].pos), sub(t[2].pos, t[0].pos));
            for k in 0..3 {
                nacc[k] += cr[k];
            }
        }
        let n = normalize(nacc);
        let ax = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let ex0 = normalize(cross(ax, n));
        let ey0 = cross(n, ex0);
        let mut pts: Vec<[f32; 2]> = Vec::new();
        let mut seen: std::collections::BTreeSet<[u32; 3]> = Default::default();
        for &ti in &c.tris {
            for k in 0..3 {
                let p = tris[ti][k].pos;
                if seen.insert(pk(p)) {
                    pts.push([p[0] * ex0[0] + p[1] * ex0[1] + p[2] * ex0[2], p[0] * ey0[0] + p[1] * ey0[1] + p[2] * ey0[2]]);
                }
            }
        }
        // convex hull (monotone chain)
        pts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let crs = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
        let mut hull: Vec<[f32; 2]> = Vec::new();
        for &p in &pts {
            while hull.len() >= 2 && crs(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        let lower_len = hull.len() + 1;
        for &p in pts.iter().rev().skip(1) {
            while hull.len() >= lower_len && crs(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
        // best rotation among hull edge directions
        let mut best = (f32::MAX, 0.0f32);
        let angles: Vec<f32> = if hull.len() >= 2 { (0..hull.len()).map(|i| { let a = hull[i]; let b = hull[(i + 1) % hull.len()]; (b[1] - a[1]).atan2(b[0] - a[0]) }).collect() } else { vec![0.0] };
        for th in angles {
            let (cs, sn) = (th.cos(), th.sin());
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for p in &pts {
                let x = p[0] * cs + p[1] * sn;
                let y = -p[0] * sn + p[1] * cs;
                lo[0] = lo[0].min(x);
                lo[1] = lo[1].min(y);
                hi[0] = hi[0].max(x);
                hi[1] = hi[1].max(y);
            }
            let area = (hi[0] - lo[0]) * (hi[1] - lo[1]);
            if area < best.0 {
                best = (area, th);
            }
        }
        let th = best.1;
        let (cs, sn) = (th.cos(), th.sin());
        // rotated basis: ex = cos*ex0 + sin*ey0 ; ey = -sin*ex0 + cos*ey0
        c.ex = [cs * ex0[0] + sn * ey0[0], cs * ex0[1] + sn * ey0[1], cs * ex0[2] + sn * ey0[2]];
        c.ey = [-sn * ex0[0] + cs * ey0[0], -sn * ex0[1] + cs * ey0[1], -sn * ex0[2] + cs * ey0[2]];
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for &ti in &c.tris {
            for k in 0..3 {
                let p = tris[ti][k].pos;
                let x = p[0] * c.ex[0] + p[1] * c.ex[1] + p[2] * c.ex[2];
                let y = p[0] * c.ey[0] + p[1] * c.ey[1] + p[2] * c.ey[2];
                lo[0] = lo[0].min(x);
                lo[1] = lo[1].min(y);
                hi[0] = hi[0].max(x);
                hi[1] = hi[1].max(y);
            }
        }
        c.min = lo;
        c.w = hi[0] - lo[0];
        c.h = hi[1] - lo[1];
    };
    for c in charts.iter_mut() {
        frame(c, per_material);
    }
    // Injectivity: a chart whose projection self-overlaps (source geometry
    // with stacked coplanar layers, e.g. RoadTechRampMed's top: 1.1% of the
    // texels double-booked) cannot share one lightmap region. Eject the
    // most-conflicting tris into singleton charts until no two tris of a
    // chart that share no position intersect in 2D.
    {
        let proj = |c: &Chart, p: [f32; 3]| [p[0] * c.ex[0] + p[1] * c.ex[1] + p[2] * c.ex[2], p[0] * c.ey[0] + p[1] * c.ey[1] + p[2] * c.ey[2]];
        let mut extra: Vec<Chart> = Vec::new();
        for ci in 0..charts.len() {
            for _round in 0..4 {
                let c = &charts[ci];
                if c.tris.len() < 2 {
                    break;
                }
                let tris = &per_material[c.mat];
                let q: Vec<[[f32; 2]; 3]> = c.tris.iter().map(|&ti| [proj(c, tris[ti][0].pos), proj(c, tris[ti][1].pos), proj(c, tris[ti][2].pos)]).collect();
                let keys: Vec<[[u32; 3]; 3]> = c.tris.iter().map(|&ti| [pk(tris[ti][0].pos), pk(tris[ti][1].pos), pk(tris[ti][2].pos)]).collect();
                let bb: Vec<[[f32; 2]; 2]> = q.iter().map(|t| [[t[0][0].min(t[1][0]).min(t[2][0]), t[0][1].min(t[1][1]).min(t[2][1])], [t[0][0].max(t[1][0]).max(t[2][0]), t[0][1].max(t[1][1]).max(t[2][1])]]).collect();
                let mut conflicts = vec![0usize; c.tris.len()];
                let mut any = false;
                for i in 0..q.len() {
                    for j in i + 1..q.len() {
                        if bb[i][1][0] <= bb[j][0][0] || bb[j][1][0] <= bb[i][0][0] || bb[i][1][1] <= bb[j][0][1] || bb[j][1][1] <= bb[i][0][1] {
                            continue;
                        }
                        if keys[i].iter().any(|k| keys[j].contains(k)) {
                            continue;
                        }
                        if tri2d_overlap(&q[i], &q[j]) {
                            conflicts[i] += 1;
                            conflicts[j] += 1;
                            any = true;
                        }
                    }
                }
                if !any {
                    break;
                }
                // eject every tri with conflicts, worst first, re-checking
                // cheaply: ejecting all conflicting tris at once is safe
                // (singletons cannot overlap anything).
                let mut ejected: Vec<usize> = Vec::new();
                let c = &mut charts[ci];
                let mut keep: Vec<usize> = Vec::new();
                for (k, &ti) in c.tris.iter().enumerate() {
                    if conflicts[k] > 0 {
                        ejected.push(ti);
                    } else {
                        keep.push(ti);
                    }
                }
                c.tris = keep;
                for ti in ejected {
                    let mut nc = Chart { mat: c.mat, tris: vec![ti], ex: [0.0; 3], ey: [0.0; 3], min: [0.0; 2], w: 0.0, h: 0.0, x: 0.0, y: 0.0, rot90: false };
                    frame(&mut nc, per_material);
                    extra.push(nc);
                }
                if charts[ci].tris.is_empty() {
                    break;
                }
                frame(&mut charts[ci], per_material);
            }
        }
        charts.retain(|c| !c.tris.is_empty());
        charts.extend(extra);
    }
    // Hollow charts (arcs, L-shapes: world area << bbox area) waste atlas
    // space -- GateCheckpoint's arch charts filled 12% of their boxes and
    // the item got 9% texel coverage. Split any chart with fill < 0.4 and > 4 tris in two along its longer box axis (projected
    // centroid), up to 4 levels, then re-frame the halves.
    {
        let fill_min = 0.4f32;
        let proj = |c: &Chart, p: [f32; 3]| [p[0] * c.ex[0] + p[1] * c.ex[1] + p[2] * c.ex[2], p[0] * c.ey[0] + p[1] * c.ey[1] + p[2] * c.ey[2]];
        let mut queue: Vec<(Chart, u8)> = charts.drain(..).map(|c| (c, 0u8)).collect();
        let mut done: Vec<Chart> = Vec::new();
        while let Some((c, depth)) = queue.pop() {
            let tris = &per_material[c.mat];
            let area3: f32 = c.tris.iter().map(|&ti| tri_area_of(&tris[ti])).sum();
            let box_area = c.w * c.h;
            if depth >= 4 || c.tris.len() <= 4 || box_area <= 1e-12 || area3 / box_area >= fill_min {
                done.push(c);
                continue;
            }
            // split along the longer axis at the median centroid coordinate
            let axis = if c.w >= c.h { 0 } else { 1 };
            let mut cents: Vec<(f32, usize)> = c
                .tris
                .iter()
                .map(|&ti| {
                    let t = &tris[ti];
                    let q = [proj(&c, t[0].pos), proj(&c, t[1].pos), proj(&c, t[2].pos)];
                    ((q[0][axis] + q[1][axis] + q[2][axis]) / 3.0, ti)
                })
                .collect();
            cents.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let half = cents.len() / 2;
            let (a, b): (Vec<usize>, Vec<usize>) = (cents[..half].iter().map(|x| x.1).collect(), cents[half..].iter().map(|x| x.1).collect());
            if a.is_empty() || b.is_empty() {
                done.push(c);
                continue;
            }
            for part in [a, b] {
                let mut nc = Chart { mat: c.mat, tris: part, ex: [0.0; 3], ey: [0.0; 3], min: [0.0; 2], w: 0.0, h: 0.0, x: 0.0, y: 0.0, rot90: false };
                frame(&mut nc, per_material);
                queue.push((nc, depth + 1));
            }
        }
        charts = done;
    }
    if charts.is_empty() {
        return;
    }
    let avail = 1.0 - 2.0 * margin;
    let mut order: Vec<usize> = (0..charts.len()).collect();
    let dims = |c: &Chart| if c.w >= c.h { (c.w, c.h, false) } else { (c.h, c.w, true) };
    order.sort_by(|&a, &b| {
        let ha = dims(&charts[a]).1;
        let hb = dims(&charts[b]).1;
        hb.partial_cmp(&ha).unwrap().then(dims(&charts[b]).0.partial_cmp(&dims(&charts[a]).0).unwrap())
    });
    // Skyline bottom-left packing at scale s (items by height desc, long
    // side horizontal): each item goes where the skyline is lowest among
    // the positions it fits, ties to the left. Returns false when an item
    // cannot be placed inside the unit square minus margins.
    // (Shelf rows were tried first: 9% texel coverage on GateCheckpoint's
    // 2737 arc charts; skyline lifts the bbox fill.)
    let try_pack = |s: f32, charts: &mut Vec<Chart>| -> bool {
        // skyline: sorted list of (x_start, height); segment ends at the next x_start
        let mut sky: Vec<(f32, f32)> = vec![(0.0, 0.0)];
        for &ci in &order {
            let (w, h, r) = dims(&charts[ci]);
            let (w, h) = (w * s + gutter, h * s + gutter);
            if w > avail + 1e-6 || h > avail + 1e-6 {
                return false;
            }
            // candidate x positions: every segment start
            let mut best: Option<(f32, f32, usize)> = None; // (y, x, seg index)
            for i in 0..sky.len() {
                let x0 = sky[i].0;
                if x0 + w > avail + 1e-6 {
                    break;
                }
                // max height over segments covering [x0, x0+w)
                let mut y = 0.0f32;
                let mut j = i;
                while j < sky.len() && sky[j].0 < x0 + w - 1e-9 {
                    y = y.max(sky[j].1);
                    j += 1;
                }
                if y + h > avail + 1e-6 {
                    continue;
                }
                let better = match best {
                    None => true,
                    Some((by, bx, _)) => y < by - 1e-9 || (y <= by + 1e-9 && x0 < bx),
                };
                if better {
                    best = Some((y, x0, i));
                }
            }
            let Some((y, x0, _)) = best else { return false };
            charts[ci].x = margin + x0;
            charts[ci].y = margin + y;
            charts[ci].rot90 = r;
            // update skyline: remove segments fully covered, clip the one
            // straddling the right edge, insert the new plateau
            let x1 = x0 + w;
            let mut newsky: Vec<(f32, f32)> = Vec::with_capacity(sky.len() + 2);
            let mut inserted = false;
            for i in 0..sky.len() {
                let (sx, sh) = sky[i];
                let sx_end = if i + 1 < sky.len() { sky[i + 1].0 } else { avail };
                if sx_end <= x0 + 1e-9 || sx >= x1 - 1e-9 {
                    // untouched segment (left or right of the item)
                    if sx >= x1 - 1e-9 && !inserted {
                        newsky.push((x0, y + h));
                        inserted = true;
                    }
                    newsky.push((sx, sh));
                    continue;
                }
                // overlapping segment
                if sx < x0 - 1e-9 {
                    newsky.push((sx, sh)); // left remainder keeps its height
                }
                if !inserted {
                    newsky.push((x0, y + h));
                    inserted = true;
                }
                if sx_end > x1 + 1e-9 {
                    newsky.push((x1, sh)); // right remainder
                }
            }
            if !inserted {
                newsky.push((x0, y + h));
            }
            // merge equal-height neighbours
            let mut merged: Vec<(f32, f32)> = Vec::with_capacity(newsky.len());
            for seg in newsky {
                if let Some(last) = merged.last() {
                    if (last.1 - seg.1).abs() < 1e-9 {
                        continue;
                    }
                }
                merged.push(seg);
            }
            sky = merged;
        }
        true
    };
    // bracket: halve until it fits, then bisect between the last failure
    // and the first success
    let (mut lo, mut hi) = (1.0f32, 2.0f32);
    while !try_pack(lo, &mut charts) {
        hi = lo;
        lo *= 0.5;
        if lo < 1e-6 {
            break;
        }
    }
    for _ in 0..40 {
        let mid = (lo + hi) * 0.5;
        if try_pack(mid, &mut charts) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let s = lo;
    try_pack(s, &mut charts);
    // assign uv1
    for c in &charts {
        let tris = &mut per_material[c.mat];
        for &ti in &c.tris {
            for k in 0..3 {
                let p = tris[ti][k].pos;
                let lx = p[0] * c.ex[0] + p[1] * c.ex[1] + p[2] * c.ex[2] - c.min[0];
                let ly = p[0] * c.ey[0] + p[1] * c.ey[1] + p[2] * c.ey[2] - c.min[1];
                let (u, v) = if c.rot90 { (c.x + ly * s, c.y + lx * s) } else { (c.x + lx * s, c.y + ly * s) };
                tris[ti][k].uv1 = [u, v];
            }
        }
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

/// Fan-triangulate a face; corners carry the face normal and their UV.
/// Quads (and n-gons) use the (v1,v3) diagonal: the editor's bake starts
/// faces at v1 (measured: every reference quad splits v1-v3, never v0-v2).
pub fn face_triangles(c: &Crystal, f: &crate::crystal_model::Face, scale: f32) -> Vec<[Corner; 3]> {
    let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| c.positions[*i as usize]).map(|p| [p[0] * scale, p[1] * scale, p[2] * scale]).collect();
    if pts.len() < 3 {
        return Vec::new();
    }
    let uvs = c.face_uvs(f);
    // Newell normal: right for concave polygons too (per-triangle cross
    // normals as the smoothing input were tested and rejected)
    let mut n = [0f32; 3];
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let n = normalize(n);
    let corner = |i: usize| {
        let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        Corner { pos: pts[i], normal: n, uv, uv1: uv, tan_u: [0.0; 3], tan_v: [0.0; 3], face: 0, group: 0 }
    };
    if pts.len() == 3 {
        return vec![[corner(0), corner(1), corner(2)]];
    }
    // Fan from v1: (v1,v2,v3), (v1,v3,v4), ...
    (2..pts.len()).map(|i| [corner(1), corner(i), corner((i + 1) % pts.len())]).collect()
}

/// Tangent along +u of a triangle's UV mapping (falls back to any vector
/// perpendicular to the normal).
pub fn tangent(t: &[Corner; 3]) -> ([f32; 3], [f32; 3]) {
    let e1 = sub(t[1].pos, t[0].pos);
    let e2 = sub(t[2].pos, t[0].pos);
    let du1 = t[1].uv[0] - t[0].uv[0];
    let dv1 = t[1].uv[1] - t[0].uv[1];
    let du2 = t[2].uv[0] - t[0].uv[0];
    let dv2 = t[2].uv[1] - t[0].uv[1];
    let det = du1 * dv2 - du2 * dv1;
    let n = t[0].normal;
    if det.abs() < 1e-12 {
        let up = if n[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let tu = normalize(cross(up, n));
        return (tu, normalize(cross(n, tu)));
    }
    let r = 1.0 / det;
    let tu = normalize([(e1[0] * dv2 - e2[0] * dv1) * r, (e1[1] * dv2 - e2[1] * dv1) * r, (e1[2] * dv2 - e2[2] * dv1) * r]);
    let tv = normalize([(e2[0] * du1 - e1[0] * du2) * r, (e2[1] * du1 - e1[1] * du2) * r, (e2[2] * du1 - e1[2] * du2) * r]);
    (tu, tv)
}

/// Layers whose faces are drawn / collided against.
pub fn geometry_layers(c: &CPlugCrystal) -> Vec<(&Crystal, bool, bool)> {
    let mut out = Vec::new();
    if let Some((_, cr)) = &c.single_layer {
        out.push((cr, true, true));
    }
    for l in &c.layers {
        if let LayerKind::Geometry { crystal, is_visible, collidable, .. } = &l.kind {
            if l.base.is_enabled {
                out.push((crystal, *is_visible, *collidable));
            }
        }
    }
    out
}

use super::build::{dec3n_pack, MergedVisual};
use super::surface::Triangle;
use super::visual::{CPlugVisualIndexedTriangles, IndexBuffer, VisualMain};
use super::vstream::*;
use super::{Node, NodeRef};
use std::collections::HashMap;

/// Reference vertex layout: Position Float3 | Normal Dec3N | TexCoord0 |
/// TexCoord1 | TangentU Dec3N | TangentV Dec3N (vertex = 40 bytes).
pub fn reference_decls() -> Vec<Decl> {
    vec![
        Decl::new(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0),
        Decl::new(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC),
        Decl::new(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x10),
        Decl::new(N_TEXCOORD0 + 1, T_FLOAT2, SPACE_GLOBAL2D, 0x18),
        Decl::new(N_TANGENT_U, T_DEC3N, SPACE_LOCAL3D, 0x20),
        Decl::new(N_TANGENT_V, T_DEC3N, SPACE_LOCAL3D, 0x24),
    ]
}

/// Materials whose game definition is built on a `PyPxz` triplanar base get a
/// flat white vertex-color layer and visual flags 0x78 from the editor's bake
/// (measured on all 26 reference items: exactly TrackWall/DecoHill/DecoHill2
/// carry it, and all three resolve to a `Tech3 Block PyPxz...` base while
/// every other material resolves to TDSN/PDiff).
pub fn material_has_vertex_color(link: &str) -> bool {
    let name = link.rsplit('\\').next().unwrap_or(link);
    matches!(name, "TrackWall" | "DecoHill" | "DecoHill2")
}

/// The editor's bake emits five vertex layouts by material family (measured
/// over all 26 reference items, every visual):
/// - `SpecialFX*`: pos/normal/uv0 only (no lightmap, no tangents, no color)
/// - `DecalPaint*` + `RaceTriggerFXFinish`: +white color (no lightmap/tangents)
/// - other `Decal*`: +white color +tangents (no lightmap)
/// - Pxz white bases: +white color +lightmap +tangents (flags 0x78)
/// - everything else: +lightmap +tangents (flags 0x38).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VisualLayout {
    Full,
    White,
    Decal,
    DecalPaint,
    SpecialFX,
}

pub fn visual_layout(link: &str) -> VisualLayout {
    let name = link.rsplit('\\').next().unwrap_or(link);
    if name.starts_with("SpecialFX") {
        VisualLayout::SpecialFX
    } else if name.starts_with("DecalPaint") || name == "RaceTriggerFXFinish" {
        VisualLayout::DecalPaint
    } else if name.starts_with("Decal") {
        VisualLayout::Decal
    } else if material_has_vertex_color(link) {
        VisualLayout::White
    } else {
        VisualLayout::Full
    }
}

fn layout_decls(layout: VisualLayout) -> Vec<Decl> {
    // (name, type, space, offset, stride_words)
    let d = Decl::with_stride;
    match layout {
        VisualLayout::Full => vec![
            d(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0, 10),
            d(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC, 10),
            d(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x10, 10),
            d(N_TEXCOORD0 + 1, T_FLOAT2, SPACE_GLOBAL2D, 0x18, 10),
            d(N_TANGENT_U, T_DEC3N, SPACE_LOCAL3D, 0x20, 10),
            d(N_TANGENT_V, T_DEC3N, SPACE_LOCAL3D, 0x24, 10),
        ],
        VisualLayout::White => vec![
            d(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0, 11),
            d(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC, 11),
            d(N_COLOR0, T_COLOR, SPACE_GLOBAL2D, 0x10, 11),
            d(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x14, 11),
            d(N_TEXCOORD0 + 1, T_FLOAT2, SPACE_GLOBAL2D, 0x1C, 11),
            d(N_TANGENT_U, T_DEC3N, SPACE_LOCAL3D, 0x24, 11),
            d(N_TANGENT_V, T_DEC3N, SPACE_LOCAL3D, 0x28, 11),
        ],
        VisualLayout::Decal => vec![
            d(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0, 9),
            d(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC, 9),
            d(N_COLOR0, T_COLOR, SPACE_GLOBAL2D, 0x10, 9),
            d(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x14, 9),
            d(N_TANGENT_U, T_DEC3N, SPACE_LOCAL3D, 0x1C, 9),
            d(N_TANGENT_V, T_DEC3N, SPACE_LOCAL3D, 0x20, 9),
        ],
        VisualLayout::DecalPaint => vec![
            d(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0, 7),
            d(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC, 7),
            d(N_COLOR0, T_COLOR, SPACE_GLOBAL2D, 0x10, 7),
            d(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x14, 7),
        ],
        VisualLayout::SpecialFX => vec![
            d(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0, 6),
            d(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC, 6),
            d(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x10, 6),
        ],
    }
}

/// Color-layout decls: Color u32 slots between the normal and TexCoord0
/// (stride 44/4 = 11; unused by the bake, which goes through
/// [`layout_decls`] -- kept honest so a future caller doesn't inherit the
/// old hardcoded stride-10 base).
pub fn color_decls() -> Vec<Decl> {
    vec![
        Decl::with_stride(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0, 11),
        Decl::with_stride(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC, 11),
        Decl::with_stride(N_COLOR0, T_COLOR, SPACE_GLOBAL2D, 0x10, 11),
        Decl::with_stride(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x14, 11),
        Decl::with_stride(N_TEXCOORD0 + 1, T_FLOAT2, SPACE_GLOBAL2D, 0x1C, 11),
        Decl::with_stride(N_TANGENT_U, T_DEC3N, SPACE_LOCAL3D, 0x24, 11),
        Decl::with_stride(N_TANGENT_V, T_DEC3N, SPACE_LOCAL3D, 0x28, 11),
    ]
}

/// Visuals (at most 65000 vertices each) over triangles; identical corners
/// share a vertex. The vertex layout follows the material family (see
/// [`visual_layout`]): color is always flat white, flags 0x78 only on the
/// Pxz white bases.
pub fn make_visuals(tris: &[[Corner; 3]], layout: VisualLayout, umode: &str) -> Vec<CPlugVisualIndexedTriangles> {
    let want_color = !matches!(layout, VisualLayout::Full | VisualLayout::SpecialFX);
    let want_uv1 = matches!(layout, VisualLayout::Full | VisualLayout::White);
    let want_tan = matches!(layout, VisualLayout::Full | VisualLayout::White | VisualLayout::Decal);
    let flags: u32 = if layout == VisualLayout::White { 0x78 } else { 0x38 };
    let mut out = Vec::new();
    let mut start = 0;
    while start < tris.len() {
        let mut pos: Vec<[f32; 3]> = Vec::new();
        let mut nrm: Vec<u32> = Vec::new();
        let mut col: Vec<u32> = Vec::new();
        let mut uv: Vec<[f32; 2]> = Vec::new();
        let mut uv1: Vec<[f32; 2]> = Vec::new();
        let mut tu: Vec<u32> = Vec::new();
        let mut tv: Vec<u32> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        // Weld key: exactly the emitted attributes (a shared index must
        // agree on everything the vertex carries).
        let mut seen: HashMap<Vec<u32>, u32> = HashMap::new();
        let mut end = start;
        while end < tris.len() && pos.len() + 3 <= 65000 {
            let t = &tris[end];
            // UV determinant sign (mirroring): his weld splits verts
            // shared by normal-det and mirrored-det tris (measured: 87-100%
            // of tangent-only splits are det-mixed; welded verts are NEVER
            // det-mixed). Robust 1-bit splitter, no float issues.
            let det = (t[1].uv[0] - t[0].uv[0]) * (t[2].uv[1] - t[0].uv[1])
                - (t[2].uv[0] - t[0].uv[0]) * (t[1].uv[1] - t[0].uv[1]);
            let det_sign = if det >= 0.0 { 1u32 } else { 0u32 };
            for c in t {
                let mut key = Vec::with_capacity(18);
                for v in [c.pos[0], c.pos[1], c.pos[2], c.normal[0], c.normal[1], c.normal[2]] {
                    key.push(v.to_bits());
                }
                if want_color {
                    key.push(0xFFFF_FFFF);
                }
                key.push(c.uv[0].to_bits());
                key.push(c.uv[1].to_bits());
                if want_uv1 {
                    key.push(c.uv1[0].to_bits());
                    key.push(c.uv1[1].to_bits());
                }
                // UV determinant sign (mirroring): his weld splits verts
                // shared by normal-det and mirrored-det tris (measured: 87-100%
                // of tangent-only splits are det-mixed; welded verts are NEVER
                // det-mixed). Only for tangent-carrying layouts (Full/White/
                // Decal): without stored tangents there's nothing for mirroring
                // to split (SFX/stripped weld mixed-det). Robust 1-bit splitter.
                if want_tan {
                    key.push(det_sign);
                }
                // Per-corner U in the weld key: his U is per-corner flat
                // (proven by 3 distinct frames at one position) and splits
                // same-det corners by uv orientation; keyed QUANTIZED to the
                // dec3n storage grid (float U is bit-unique almost everywhere
                // and explodes the vertex count; his weld keys on storable
                // values — welds near-equal, splits truly-different), U and V
                // both (fullkey: near-identical U twins with different V
                // welded until V joined the key). umode off/duoff skips the
                // U-key: U-twins there coincide with det/normal splits, and
                // the key only over-splits from residual value errors.
                let ukey = umode != "off" && umode != "duoff";
                if want_tan && ukey {
                    // the stored VPRIM frames; a corner without one (a
                    // degenerate uv) keys on its recomputed per-corner U
                    let (su, sv) = if c.tan_u == [0.0; 3] && c.tan_v == [0.0; 3] {
                        let du1 = t[1].uv[0] - t[0].uv[0];
                        let dv1 = t[1].uv[1] - t[0].uv[1];
                        let du2 = t[2].uv[0] - t[0].uv[0];
                        let dv2 = t[2].uv[1] - t[0].uv[1];
                        let ddet = du1 * dv2 - du2 * dv1;
                        let n = c.normal;
                        if ddet.abs() < 1e-12 {
                            ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
                        } else {
                            let r = 1.0 / ddet;
                            let e1 = sub(t[1].pos, t[0].pos);
                            let e2 = sub(t[2].pos, t[0].pos);
                            let tx = (e1[0] * dv2 - e2[0] * dv1) * r;
                            let ty = (e1[1] * dv2 - e2[1] * dv1) * r;
                            let tz = (e1[2] * dv2 - e2[2] * dv1) * r;
                            let l = (tx * tx + ty * ty + tz * tz).sqrt().max(1e-30);
                            let (tx, ty, tz) = (tx / l, ty / l, tz / l);
                            let dd = tx * n[0] + ty * n[1] + tz * n[2];
                            let ox = tx - dd * n[0];
                            let oy = ty - dd * n[1];
                            let oz = tz - dd * n[2];
                            let l2 = (ox * ox + oy * oy + oz * oz).sqrt().max(1e-30);
                            ([ox / l2, oy / l2, oz / l2], [0.0, 0.0, 0.0])
                        }
                    } else {
                        (c.tan_u, c.tan_v)
                    };
                    key.push(dec3n_pack(su));
                    key.push(dec3n_pack(sv));
                }
                let (a, b) = if want_tan {
                    if c.tan_u == [0.0; 3] && c.tan_v == [0.0; 3] {
                        tangent(&[c.clone(), c.clone(), c.clone()])
                    } else {
                        (c.tan_u, c.tan_v)
                    }
                } else {
                    ([0.0; 3], [0.0; 3])
                };
                let i = *seen.entry(key).or_insert_with(|| {
                    pos.push(c.pos);
                    nrm.push(dec3n_pack(c.normal));
                    if want_color {
                        col.push(0xFFFF_FFFF);
                    }
                    uv.push(c.uv);
                    if want_uv1 {
                        uv1.push(c.uv1);
                    }
                    if want_tan {
                        tu.push(dec3n_pack(a));
                        tv.push(dec3n_pack(b));
                    }
                    (pos.len() - 1) as u32
                });
                idx.push(i);
            }
            end += 1;
        }
        start = end;
        let n = pos.len() as i32;
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in &pos {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let mut elems = vec![Elem::Float3(pos), Elem::Word(nrm)];
        if want_color {
            elems.push(Elem::Word(col));
        }
        elems.push(Elem::Float2(uv));
        if want_uv1 {
            elems.push(Elem::Float2(uv1));
        }
        if want_tan {
            elems.push(Elem::Word(tu));
            elems.push(Elem::Word(tv));
        }
        let stream = CPlugVertexStream {
            version: 1,
            count: n,
            flags: 1,
            base: super::null_ref(),
            decls: layout_decls(layout),
            compress_local3d: Some(true),
            elems,
        };
        let main = VisualMain {
            version: 6,
            chunk_flags: flags,
            tex_coord_sets: Vec::new(),
            count: n,
            vertex_streams: vec![NodeRef { index: 0, inline: Some(Box::new(Node::VertexStream(stream))) }],
            skin: None,
            // half-extents at least 2 cm: a FLAT decal (the `_FC_Ground` road/zone
            // pieces) is dropped by the editor on re-save with a zero-height box
            bounding_box: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, ((hi[0] - lo[0]) / 2.0).max(0.02), ((hi[1] - lo[1]) / 2.0).max(0.02), ((hi[2] - lo[2]) / 2.0).max(0.02)],
            bitmap_elems: Vec::new(),
            uv_groups: Vec::new(),
            u02: 0,
            u03: 0,
            u04: Vec::new(),
        };
        out.push(CPlugVisualIndexedTriangles {
            chunks: vec![0x09006001, 0x09006005, 0x09006009, 0x0900600B, 0x0900600F, 0x09006010, 0x0902C002, 0x0902C004, 0x0906A001],
            id: super::Id::Null,
            u_node: super::null_ref(),
            sub_visuals: Vec::new(),
            u_float: 0.0,
            splits: Vec::new(),
            main: Some(main),
            morph: Some((0, 0)),
            v3d_node: super::null_ref(),
            tangents: Some((Vec::new(), Vec::new())),
            index_buffer: Some(IndexBuffer::delta(idx)),
            inline_form: false,
            inline_uv_sets: 1,
            inline_uv_flags: 256,
            inline_tangents: false,
        });
    }
    out
}

/// Bake a crystal into `m`: one visual per material over the visible faces,
/// collision from the collidable layers (else the visible faces).
pub fn add_crystal(c: &CPlugCrystal, scale: f32, m: &mut Merged) -> R<()> {
    let layers = geometry_layers(c);
    if layers.is_empty() {
        return Err("crystal has no geometry layer".into());
    }
    // Bake timestamp, like the editor writes (his filetime is his bake
    // time; ours was 0/unset for crystal bakes -- an unbaked look).
    if m.file_write_time == 0 {
        let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        m.file_write_time = unix * 10_000_000 + 116444736000000000;
    }
    // material slot per crystal material index (editors mode remaps onto
    // the mesh-editor family, like the prefab path; otherwise the
    // editor-resolved link, so the baked item carries real
    // `.Material.Gbx` paths instead of dangling virtual names).
    let editors = m.editors;
    let slots: Vec<usize> = c
        .materials
        .iter()
        .map(|mat| match mat.inst() {
            Some(inst) => {
                if editors {
                    let link = inst.link().unwrap_or("").to_string();
                    let stem = link.rsplit('\\').next().unwrap_or(&link).to_string();
                    m.material_slot(crate::tiny_assets::editors_link_for_stadium_material(&stem), inst.physics())
                } else {
                    m.resolved_inst_slot(inst)
                }
            }
            None => m.link_slot(&mat.name, 0, editors),
        })
        .collect();
    let mut per_material: Vec<Vec<[Corner; 3]>> = vec![Vec::new(); slots.len().max(1)];
    // Surface buckets: one `material_ids` entry per crystal material in
    // first-sight order over the collision traversal (collidable layers
    // REVERSED, faces forward), so duplicate values survive (his turbo
    // table holds 9 twice, 4 three times, 28 three times, 32 twice).
    // Boost-Decal faces synthesize the (phys 0, gameplay 1) trigger entry;
    // dropped phys-28 visuals keep their entries; the Decal visual's own
    // entry is appended last (its faces went to the trigger). Tri
    // (mat,u03,si) rows then match his 1:1 (Road_17: 10 rows + trigger).
    // Slot key: crystal material index; trigger key: -1. (His entry order
    // is merge history, not derivable: the traversal emits Road before
    // SpecialFX where his table has SpecialFX 4th and Road 11th — 8
    // traversal hypotheses tested; the traversal order stands.)
    // tri records: (slot key, phys, gameplay, positions)
    let mut surf_recs: Vec<(i32, u8, u8, [[f32; 3]; 3])> = Vec::new();
    let mut surf_tris: Vec<(Triangle, [[f32; 3]; 3])> = Vec::new();
    let mut surf_entries: Vec<(i32, u16)> = Vec::new();
    let any_collidable = layers.iter().any(|(_, _, col)| *col);
    // visible faces (forward layers, visible only) -- unchanged
    let mut face_id: u32 = 0;
    for (cr, visible, _) in &layers {
        if !visible {
            continue;
        }
        for f in &cr.faces {
            let mut tris = face_triangles(cr, f, scale);
            face_id += 1;
            for t in tris.iter_mut() {
                for c in t.iter_mut() {
                    c.face = face_id;
                    c.group = f.group;
                }
            }
            let slot_i = if f.material >= 0 && (f.material as usize) < slots.len() { Some(f.material as usize) } else { None };
            match slot_i {
                Some(i) => per_material[i].extend(tris.iter().cloned()),
                None => per_material[0].extend(tris.iter().cloned()),
            }
        }
    }
    // collision faces (collidable layers reversed, faces forward)
    // records carry the slot key; entries + si resolve below
    for (cr, visible, collidable) in layers.iter().rev() {
        if !(*collidable || (!any_collidable && *visible)) {
            continue;
        }
        for f in &cr.faces {
            let tris = face_triangles(cr, f, scale);
            if tris.is_empty() {
                continue;
            }
            let slot_i = if f.material >= 0 && (f.material as usize) < slots.len() { Some(f.material as usize) } else { None };
            let phys = slot_i.map(|i| m.materials[slots[i]].physics()).unwrap_or(0);
            // NotCollidable (28) faces stay out of the collision -- except
            // the turbo chevron Decal, which the editor keeps with
            // physics 0 + gameplay 1 (boost trigger): measured his Road_17
            // surf holds exactly the 128 Decal tris as (mat=0,u03=1) while
            // SpecialFX/DecalPaint (also 28) are absent. Other phys-28
            // faces (lights/32 stay -- those are physics 32, not 28).
            if phys == 28 {
                let is_boost_decal = slot_i
                    .map(|i| {
                        let link = m.materials[slots[i]].link().unwrap_or("").to_string();
                        let stem = link.rsplit('\\').next().unwrap_or(&link).to_string();
                        stem == "Decal"
                    })
                    .unwrap_or(false);
                if !is_boost_decal {
                    continue;
                }
                for t in &tris {
                    surf_recs.push((-1, 0, 1, [t[0].pos, t[1].pos, t[2].pos]));
                }
                continue;
            }
            // Physics-0 helpers (Editors Rubber/Concrete collidable-only
            // faces) stay out too: his surf has no (mat=0,u03=0) entry,
            // only the Decal (mat=0,u03=1) above.
            if phys == 0 {
                continue;
            }
            let key = slot_i.map(|i| i as i32).unwrap_or(-2);
            for t in &tris {
                surf_recs.push((key, phys, 0, [t[0].pos, t[1].pos, t[2].pos]));
            }
        }
    }
    // table entries: spec order, else first-sight over the records.
    // Every seen slot keeps an entry (even dropped phys-28 visuals: his
    // table holds unreferenced 28s), plus every drawn visual material
    // (the Decal visual: its faces went to the trigger).
    {
        let mut seen: std::collections::BTreeSet<i32> = Default::default();
        let mut keys: Vec<i32> = Vec::new();
        for (key, _, _, _) in &surf_recs {
            if seen.insert(*key) {
                keys.push(*key);
            }
        }
        // dropped-28 slots have no records; recover them from the faces
        // (always: the spec only reorders, coverage must match)
        for (cr, visible, collidable) in layers.iter().rev() {
                if !(*collidable || (!any_collidable && *visible)) {
                    continue;
                }
                for f in &cr.faces {
                    let slot_i = if f.material >= 0 && (f.material as usize) < slots.len() { Some(f.material as usize) } else { None };
                    let phys = slot_i.map(|i| m.materials[slots[i]].physics()).unwrap_or(0);
                    if phys != 28 {
                        continue;
                    }
                    let is_boost_decal = slot_i
                        .map(|i| {
                            let link = m.materials[slots[i]].link().unwrap_or("").to_string();
                            let stem = link.rsplit('\\').next().unwrap_or(&link).to_string();
                            stem == "Decal"
                        })
                        .unwrap_or(false);
                    if is_boost_decal {
                        continue;
                    }
                    if let Some(i) = slot_i {
                        if seen.insert(i as i32) {
                            keys.push(i as i32);
                        }
                    }
                }
        }
        // drawn visuals without records (Decal visual -> trigger)
        for i in 0..per_material.len() {
            if per_material[i].is_empty() {
                continue;
            }
            if seen.insert(i as i32) {
                keys.push(i as i32);
            }
        }
        // the entries in traversal (first-sight) order
        let ordered: Vec<i32> = keys;
        let value_of = |key: i32| -> u16 {
            if key == -1 {
                return 256;
            }
            if key == -2 {
                return 0;
            }
            let si = key as usize;
            if si < slots.len() {
                // dropped-28 slots read 28; drawn slots read their physics
                let phys = m.materials[slots[si]].physics();
                // trigger-consumed Decal visual: its own entry still reads 28
                return phys as u16;
            }
            0
        };
        let mut si_of: std::collections::BTreeMap<i32, i32> = Default::default();
        for key in &ordered {
            if !si_of.contains_key(key) {
                surf_entries.push((*key, value_of(*key)));
                si_of.insert(*key, (surf_entries.len() - 1) as i32);
            }
        }
        for (key, phys, gameplay, ps) in &surf_recs {
            if let Some(si) = si_of.get(key) {
                surf_tris.push((Triangle { indices: [0; 3], material_id: *phys, u03: *gameplay, surface_index: *si as i16 }, *ps));
            } else {
                m.notes.push(format!("surface slot {key} missing from entry table"));
            }
        }
    }
    // The editor's bake orders materials by triangle count, most first, for
    // clean single-pass bakes (Snow2 and the BlueBay terrain all match).
    // Items edited after baking carry merge-history order instead (Road_17
    // and Road_18 both break tri-desc: appended signs/decals sit out of
    // count order) — edit history, not derivable from the crystal.
    let mut order: Vec<usize> = (0..per_material.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(per_material[i].len()));
    // the recipe per crystal material index (by its link's stem)
    let recipes: Vec<MaterialRecipe> = (0..slots.len())
        .map(|i| {
            let slot = slots.get(i).copied().unwrap_or(usize::MAX);
            if slot == usize::MAX {
                return DEFAULT_RECIPE;
            }
            recipe_for(m.materials.get(slot).and_then(|x: &CPlugMaterialUserInst| x.link()).unwrap_or(""))
        })
        .collect();
    // Item-global lightmap atlas, before smoothing (position-only), in
    // per_material order = slots.
    let has_uv1: Vec<bool> = (0..per_material.len())
        .map(|i| {
            let slot = slots.get(i).copied();
            let link = slot.and_then(|s| m.materials.get(s)).and_then(|x: &CPlugMaterialUserInst| x.link().map(|l| l.to_string())).unwrap_or_default();
            layout_decls(visual_layout(&link)).iter().any(|d| d.name() == N_TEXCOORD0 + 1)
        })
        .collect();
    assign_lightmap_atlas(&mut per_material, &has_uv1);
    for i in order {
        let r = recipes.get(i).copied().unwrap_or(DEFAULT_RECIPE);
        // Smooth normals by position within the material's crease angle (the
        // editor's bake averages face normals at shared vertices but keeps
        // hard edges split: flat face normals split every crease (2-3x too
        // many verts), full smoothing over-shares (0.6-0.9x too few)). Must
        // run before welding.
        if r.corner_mode {
            smooth_normals_corner(&mut per_material[i], r.crease_deg, r.angle_weight);
        } else {
            smooth_normals_angle(&mut per_material[i], r.crease_deg, r.angle_weight);
        }
        // V-primary tangent frames with the material's primary gradient
        // (du/range; off/duoff compute range/du frames but skip the U-key
        // splits). After normal smoothing (uses the smoothed N), before
        // welding. Corner.tan_u/tan_v carry the frames.
        let umode_n: u8 = if r.umode == "du" || r.umode == "duoff" { 1 } else if r.umode == "dv" { 2 } else { 0 };
        tangents_vprim(&mut per_material[i], umode_n);
        // U-cluster smoothing of the V-primary frames: averages agreeing U
        // (bevels), splits opposed (coils); the weld key consumes the
        // smoothed quantized U.
        if r.umode != "off" {
            smooth_u_vprim(&mut per_material[i], r.tan_deg, r.angle_weight, r.umag_weight);
        }
        let tris = &per_material[i];
        if tris.is_empty() {
            continue;
        }
        let slot = slots.get(i).copied().unwrap_or_else(|| m.material_slot("Stadium\\Media\\Material\\PlatformTech", 0));
        let layout = visual_layout(&m.materials[slot].link().unwrap_or("").to_string());
        for v in make_visuals(tris, layout, r.umode) {
            m.visuals.push(MergedVisual::every_level(v, slot));
        }
    }
    // collision: shared vertices by exact position; `surface_index` per
    // tri already points at `surf_entries` (per-slot table, duplicates
    // kept), so `surf_ids` takes the entry values in order -- NOT the
    // deduped `surf_id_slot` path (that would merge his split rows).
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut seen: HashMap<[u32; 3], u32> = HashMap::new();
    let mut tris: Vec<Triangle> = Vec::new();
    for (t, ps) in &surf_tris {
        let mut ix = [0u32; 3];
        for (k, p) in ps.iter().enumerate() {
            ix[k] = *seen.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_insert_with(|| {
                verts.push(*p);
                (verts.len() - 1) as u32
            });
        }
        tris.push(Triangle { indices: ix, ..*t });
    }
    m.surf_vertices = verts;
    m.surf_triangles = tris;
    m.surf_ids = surf_entries.iter().map(|(_, v)| *v).collect();
    m.surface_built = true;
    // Prelight: u02 (lightmap texel scale = sqrt(world/uv1), measured
    // across 26 refs) and u04 (uv1 bounds).
    // (Boxes/uv-groups/sprites empty, u01/u03/version default -- match his.)
    {
        let (mut mnx, mut mxx, mut mny, mut mxy) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let (mut aworld, mut auv1) = (0.0f64, 0.0f64);
        // Only visuals that carry a uv1 stream count (others hold the uv0
        // copy, which pollutes the bounds and the density).
        for (mi, tris) in per_material.iter().enumerate() {
            if !has_uv1.get(mi).copied().unwrap_or(false) {
                continue;
            }
            for t in tris {
                for c in t {
                    mnx = mnx.min(c.uv1[0]);
                    mxx = mxx.max(c.uv1[0]);
                    mny = mny.min(c.uv1[1]);
                    mxy = mxy.max(c.uv1[1]);
                }
                let au = ((t[1].uv1[0] - t[0].uv1[0]) * (t[2].uv1[1] - t[0].uv1[1])
                    - (t[2].uv1[0] - t[0].uv1[0]) * (t[1].uv1[1] - t[0].uv1[1]))
                .abs() as f64
                    / 2.0;
                let e1 = sub(t[1].pos, t[0].pos);
                let e2 = sub(t[2].pos, t[0].pos);
                let cr = cross(e1, e2);
                let aw = ((cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]) as f64).sqrt() / 2.0;
                auv1 += au;
                aworld += aw;
            }
        }
        // the generated atlas gets its own texel scale (u04[4..8] stay
        // MAX/MIN sentinels)
        let u02 = if auv1 > 1e-12 { (aworld / auv1).sqrt() as f32 } else { 32.14457 };
        let mut pl = super::solid2::PreLightGen {
            version: 1,
            u01: 1,
            u02,
            u03: true,
            u04: [mnx, mny, mxx, mxy, f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            sprite_count: [0, 0],
            boxes: Vec::new(),
            uv_groups: Vec::new(),
        };
        let _ = &mut pl;
        m.pre_light_gen = Some(pl);
        m.notes.push(format!("prelight u02={u02:.3} uv1bounds=[{mnx:.3},{mny:.3},{mxx:.3},{mxy:.3}]"));
    }
    m.notes.push(format!("crystal baked: {} layers, {} triangles, {} collision triangles, {} surface entries", layers.len(), per_material.iter().map(|v| v.len()).sum::<usize>(), m.surf_triangles.len(), m.surf_ids.len()));
    Ok(())
}

/// Do two 2D triangles overlap with positive area (separating-axis test over
/// the six edge normals)? Touching along an edge or at a vertex is not an
/// overlap.
fn tri2d_overlap(a: &[[f32; 2]; 3], b: &[[f32; 2]; 3]) -> bool {
    let axes = |t: &[[f32; 2]; 3]| -> [[f32; 2]; 3] {
        let mut out = [[0.0f32; 2]; 3];
        for i in 0..3 {
            let p = t[i];
            let q = t[(i + 1) % 3];
            out[i] = [-(q[1] - p[1]), q[0] - p[0]];
        }
        out
    };
    let eps = 1e-7f32;
    for ax in axes(a).iter().chain(axes(b).iter()) {
        let pa: Vec<f32> = a.iter().map(|p| p[0] * ax[0] + p[1] * ax[1]).collect();
        let pb: Vec<f32> = b.iter().map(|p| p[0] * ax[0] + p[1] * ax[1]).collect();
        let (amin, amax) = (pa.iter().cloned().fold(f32::MAX, f32::min), pa.iter().cloned().fold(f32::MIN, f32::max));
        let (bmin, bmax) = (pb.iter().cloned().fold(f32::MAX, f32::min), pb.iter().cloned().fold(f32::MIN, f32::max));
        let scale = (amax - amin).max(bmax - bmin).max(1e-12);
        if amax <= bmin + eps * scale || bmax <= amin + eps * scale {
            return false;
        }
    }
    true
}
