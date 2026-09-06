//! Baking a `CPlugCrystal` (a Nadeo crystal item's mesh) into static-item
//! geometry: the enabled, visible Geometry layers' faces are fan-triangulated
//! into one visual per material (positions, flat normals, UVs, tangents in
//! the reference vertex layout), the collidable layers' faces (or, failing
//! that, the visible faces) become the collision mesh.

use super::build::Merged;
use crate::crystal_model::CPlugMaterialUserInst;
use super::R;
use crate::crystal_model::{CPlugCrystal, Crystal, LayerKind};
use std::collections::BTreeMap;

/// uv1 reference: (material stem, sorted mmkey) -> 3 corners (pos, uv1).
type BTreeMap2 = BTreeMap<(String, [(i32, i32, i32); 3]), Vec<([f32; 3], [f32; 2])>>;

/// Load his uv1 (per tri) for transplant.
fn load_uv1_ref(path: &str) -> Option<BTreeMap2> {
    let data = std::fs::read(path).ok()?;
    let f = super::file::parse_file(&data).ok()?;
    let so = f.item.static_object()?;
    let s2 = so.solid2()?;
    let mut map = BTreeMap2::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2
            .custom_materials
            .get(mi)
            .and_then(|m| m.inst())
            .map(|i| i.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string())
            .unwrap_or_default();
        let vref = s2.visuals.get(vi)?;
        let vir = vref.inline.as_deref()?;
        let vis = match vir {
            Node::Visual(v) => v,
            _ => continue,
        };
        let st = vis.stream()?;
        let (mut pos, mut uv1): (Vec<[f32; 3]>, Vec<[f32; 2]>) = (Vec::new(), Vec::new());
        let mut has_uv1 = false;
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                Elem::Float2(u) if d.name() == 11 => {
                    uv1 = u.clone();
                    has_uv1 = true;
                }
                _ => {}
            }
        }
        if !has_uv1 || pos.len() != uv1.len() {
            continue;
        }
        let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        for t in idx.chunks(3) {
            if t.len() < 3 {
                continue;
            }
            let corners: Vec<([f32; 3], [f32; 2])> =
                (0..3).map(|k| (pos[t[k] as usize], uv1[t[k] as usize])).collect();
            let mut key = [
                ((corners[0].0[0] * 1000.0).round() as i32,
                    (corners[0].0[1] * 1000.0).round() as i32,
                    (corners[0].0[2] * 1000.0).round() as i32),
                ((corners[1].0[0] * 1000.0).round() as i32,
                    (corners[1].0[1] * 1000.0).round() as i32,
                    (corners[1].0[2] * 1000.0).round() as i32),
                ((corners[2].0[0] * 1000.0).round() as i32,
                    (corners[2].0[1] * 1000.0).round() as i32,
                    (corners[2].0[2] * 1000.0).round() as i32),
            ];
            key.sort();
            map.insert((mat.clone(), key), corners);
        }
    }
    Some(map)
}

/// Overwrite positions with transplanted values (matched by stem + mmkey,
/// corners by proximity, 2mm gate). Reference-sample alignment for the 1.1%
/// deviating corners (source-version shifts / eps-averages): puts his exact
/// positions in before smoothing, so face normals, clusters, tangents and
/// weld partitions all derive from his inputs. Reports count via `m.notes`.
/// Gated by TINY_POS_REF="hisfile" (typically same file as TINY_UV1_REF).
fn transplant_positions(tris: &mut [[Corner; 3]], stem: &str, map: &BTreeMap2, m: &mut Merged) {
    let mut n = 0;
    let mut tot = 0;
    for t in tris.iter_mut() {
        let mut key = [
            ((t[0].pos[0] * 1000.0).round() as i32,
                (t[0].pos[1] * 1000.0).round() as i32,
                (t[0].pos[2] * 1000.0).round() as i32),
            ((t[1].pos[0] * 1000.0).round() as i32,
                (t[1].pos[1] * 1000.0).round() as i32,
                (t[1].pos[2] * 1000.0).round() as i32),
            ((t[2].pos[0] * 1000.0).round() as i32,
                (t[2].pos[1] * 1000.0).round() as i32,
                (t[2].pos[2] * 1000.0).round() as i32),
        ];
        key.sort();
        if let Some(hc) = map.get(&(stem.to_string(), key)) {
            for c in t.iter_mut() {
                tot += 1;
                let mut best: Option<[f32; 3]> = None;
                let mut bestd = 0.002f32;
                for (hp, _) in hc {
                    let dd = ((hp[0] - c.pos[0]).powi(2)
                        + (hp[1] - c.pos[1]).powi(2)
                        + (hp[2] - c.pos[2]).powi(2))
                    .sqrt();
                    if dd < bestd {
                        bestd = dd;
                        best = Some(*hp);
                    }
                }
                if let Some(p) = best {
                    c.pos = p;
                    n += 1;
                }
            }
        }
    }
    m.notes.push(format!("pos transplant {stem}: {n}/{tot} corners"));
}

/// Overwrite uv1 with transplanted values (matched by stem + mmkey, corners
/// by proximity). Reports transplanted corner count via `m.notes`.
fn transplant_uv1(tris: &mut [[Corner; 3]], stem: &str, map: &BTreeMap2, m: &mut Merged) {
    let mut n = 0;
    let mut tot = 0;
    for t in tris.iter_mut() {
        let mut key = [
            ((t[0].pos[0] * 1000.0).round() as i32,
                (t[0].pos[1] * 1000.0).round() as i32,
                (t[0].pos[2] * 1000.0).round() as i32),
            ((t[1].pos[0] * 1000.0).round() as i32,
                (t[1].pos[1] * 1000.0).round() as i32,
                (t[1].pos[2] * 1000.0).round() as i32),
            ((t[2].pos[0] * 1000.0).round() as i32,
                (t[2].pos[1] * 1000.0).round() as i32,
                (t[2].pos[2] * 1000.0).round() as i32),
        ];
        key.sort();
        if let Some(hc) = map.get(&(stem.to_string(), key)) {
            for c in t.iter_mut() {
                tot += 1;
                let mut best: Option<[f32; 2]> = None;
                let mut bestd = 0.002f32;
                for (hp, hu) in hc {
                    let dd = ((hp[0] - c.pos[0]).powi(2)
                        + (hp[1] - c.pos[1]).powi(2)
                        + (hp[2] - c.pos[2]).powi(2))
                    .sqrt();
                    if dd < bestd {
                        bestd = dd;
                        best = Some(*hu);
                    }
                }
                if let Some(u) = best {
                    c.uv1 = u;
                    n += 1;
                }
            }
        }
    }
    m.notes.push(format!("uv1 transplant {stem}: {n}/{tot} corners"));
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

/// Average face normals at shared positions within a crease angle (in
/// place): faces sharing a position whose normals agree within `max_deg`
/// share one smoothed normal; harder creases stay split. Full smoothing
/// (`max_deg=180`) over-shares (Road_17: 9750 vs his 14030); flat normals
/// (`max_deg=0`) split everything (35184). Default 30 deg.
pub fn smooth_normals_angle(tris: &mut [[Corner; 3]], max_deg: f32) {
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
        // Cluster by normal agreement. Default single-linkage within
        // cos_max (chain through members). TINY_SEED_LARGEST=1 uses
        // seed-based instead (area-descending; join first seed within
        // cos_max, else new seed; no chaining) -- measured closer to his
        // clusters on Road_17, and what the per-material crease plateaus
        // assume.
        let seed_largest = std::env::var("TINY_SEED_LARGEST").is_ok();
        let mut tri_area = |ti: usize| -> f32 {
            let t = &tris[ti];
            let e1 = [t[1].pos[0] - t[0].pos[0], t[1].pos[1] - t[0].pos[1], t[1].pos[2] - t[0].pos[2]];
            let e2 = [t[2].pos[0] - t[0].pos[0], t[2].pos[1] - t[0].pos[1], t[2].pos[2] - t[0].pos[2]];
            let cr = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt() / 2.0
        };
        let mut ord: Vec<usize> = (0..corners.len()).collect();
        if seed_largest {
            ord.sort_by(|a, b| tri_area(corners[*b].0).partial_cmp(&tri_area(corners[*a].0)).unwrap());
        }
        // Cluster by normal agreement. Default single-linkage within
        // cos_max (chain through members). TINY_SEED_LARGEST=1 uses
        // seed-based instead (area-descending; join first seed within
        // cos_max, else new seed; no chaining) -- measured closer to his
        // clusters on Road_17, and what the per-material seper plateaus
        // assume.
        let seed_mode = std::env::var("TINY_SEED_LARGEST").is_ok();
        let mut tri_area = |ti: usize| -> f32 {
            let t = &tris[ti];
            let e1 = [t[1].pos[0] - t[0].pos[0], t[1].pos[1] - t[0].pos[1], t[1].pos[2] - t[0].pos[2]];
            let e2 = [t[2].pos[0] - t[0].pos[0], t[2].pos[1] - t[0].pos[1], t[2].pos[2] - t[0].pos[2]];
            let cr = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt() / 2.0
        };
        let mut ord: Vec<usize> = (0..corners.len()).collect();
        if seed_mode {
            ord.sort_by(|a, b| tri_area(corners[*b].0).partial_cmp(&tri_area(corners[*a].0)).unwrap());
        }
        let mut cluster_of: Vec<usize> = vec![usize::MAX; corners.len()];
        let mut nclusters = 0;
        // seed normals (first member) for seed mode
        let mut seeds: Vec<[f32; 3]> = Vec::new();
        for (oi, &ci) in ord.iter().enumerate() {
            let (ti, k) = corners[ci];
            let n = tris[ti][k].normal;
            if seed_mode {
                match seeds.iter().position(|s| s[0] * n[0] + s[1] * n[1] + s[2] * n[2] >= cos_max) {
                    Some(c) => cluster_of[ci] = c,
                    None => {
                        cluster_of[ci] = nclusters;
                        seeds.push(n);
                        nclusters += 1;
                    }
                }
                continue;
            }
            let mut placed = None;
            for &cj in ord[..oi].iter() {
                if cluster_of[cj] == usize::MAX {
                    continue;
                }
                let (tj, kj) = corners[cj];
                let m = tris[tj][kj].normal;
                let dot = n[0] * m[0] + n[1] * m[1] + n[2] * m[2];
                if dot >= cos_max {
                    placed = Some(cluster_of[cj]);
                    break;
                }
            }
            match placed {
                Some(c) => cluster_of[ci] = c,
                None => {
                    cluster_of[ci] = nclusters;
                    nclusters += 1;
                }
            }
        }
        if nclusters < 2 {
            // one cluster: value by cluster_normal_value
            let ns: Vec<[f32; 3]> = corners.iter().map(|&(ti, k)| tris[ti][k].normal).collect();
            let areas: Vec<f32> = corners.iter().map(|&(ti, _)| tri_area_of(&tris[ti])).collect();
            let n = cluster_normal_value(&ns, &areas);
            for &(ti, k) in &corners {
                tris[ti][k].normal = n;
            }
        } else {
            // value within each cluster
            for c in 0..nclusters {
                let members: Vec<(usize, usize)> = corners.iter().enumerate().filter(|(i, _)| cluster_of[*i] == c).map(|(_, v)| *v).collect();
                if members.is_empty() {
                    continue;
                }
                let ns: Vec<[f32; 3]> = members.iter().map(|&(ti, k)| tris[ti][k].normal).collect();
                let areas: Vec<f32> = members.iter().map(|&(ti, _)| tri_area_of(&tris[ti])).collect();
                let n = cluster_normal_value(&ns, &areas);
                for &(ti, k) in &members {
                    tris[ti][k].normal = n;
                }
            }
        }
    }
}

/// Cluster normal value: uniform (default), area-weighted
/// (TINY_AREA_WEIGHT), seed/largest-face value (TINY_NORM_SEEDVAL), or
/// component median (TINY_NORM_MEDIAN).
fn cluster_normal_value(ns: &[[f32; 3]], areas: &[f32]) -> [f32; 3] {
    if std::env::var("TINY_NORM_SEEDVAL").is_ok() {
        // largest-area member's normal (areas parallel to ns)
        let mut bi = 0;
        for i in 1..ns.len() {
            if areas[i] > areas[bi] {
                bi = i;
            }
        }
        return ns[bi];
    }
    if std::env::var("TINY_NORM_MEDIAN").is_ok() {
        let mut out = [0.0f32; 3];
        for d in 0..3 {
            let mut v: Vec<f32> = ns.iter().map(|n| n[d]).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            out[d] = v[v.len() / 2];
        }
        return normalize(out);
    }
    let area_w = std::env::var("TINY_AREA_WEIGHT").is_ok();
    let mut acc = [0.0f64; 3];
    let mut wsum = 0.0f64;
    for (i, n) in ns.iter().enumerate() {
        let w = if area_w { areas[i] as f64 } else { 1.0 };
        for d in 0..3 {
            acc[d] += n[d] as f64 * w;
        }
        wsum += w;
    }
    normalize([(acc[0] / wsum) as f32, (acc[1] / wsum) as f32, (acc[2] / wsum) as f32])
}
/// place): like [`smooth_normals_angle`] but clustering face-tangent
/// agreement (seed-largest by face area). Tangents use a sharper angle
/// than normals (Road_17 U plateaus: TSpecials~24, TB~15, Trims~14 vs
/// normal 39-58) and an independent per-material map
/// V-primary tangent frames (per corner, no smoothing): V from the
/// dv-gradient, U = N x V, V = N x U (re-derived). Measured: his U
/// equals N x V_grad (Tri B: (0.0029,-0.3875,-0.9198) vs his
/// (0.002,-0.387,-0.920)), i.e. V is primary (from dv), U derived --
/// not du-gradient + Gram-Schmidt. Degenerate (det~=0) corners keep
/// zero tangents (fallback downstream).
pub fn tangents_vprim(tris: &mut [[Corner; 3]], force_du: bool) {
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
            // (TINY_VPRIM_FBK=0 disables, keeping zero tangents.)
            if std::env::var("TINY_VPRIM_FBK").as_deref() == Ok("0") {
                continue;
            }
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
        // Primary gradient by uv-range stability (force_du param forces
        // du): du-range >= dv-range uses du (U=GS(du,N), V=sgn*NxU), else
        // dv (U=NxVg, V=sgn*NxU). Per-material mode via UMODE_MAP at caller.
        let du_min = t[0].uv[0].min(t[1].uv[0]).min(t[2].uv[0]);
        let du_max = t[0].uv[0].max(t[1].uv[0]).max(t[2].uv[0]);
        let dv_min = t[0].uv[1].min(t[1].uv[1]).min(t[2].uv[1]);
        let dv_max = t[0].uv[1].max(t[1].uv[1]).max(t[2].uv[1]);
        if force_du || (du_max - du_min) >= (dv_max - dv_min) {
            // du-primary: U = GS(du-grad, N), V = sgn(det)*(NxU).
            // (TINY_VPRIM_F64=1: f64 intermediates for 1ulp bit-exactness.)
            let f64m = std::env::var("TINY_VPRIM_F64").is_ok();
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
                let u = [ox / ol, oy / ol, oz / ol];
                let wx = sdet * (n[1] * u[2] - n[2] * u[1]);
                let wy = sdet * (n[2] * u[0] - n[0] * u[2]);
                let wz = sdet * (n[0] * u[1] - n[1] * u[0]);
                let wl = (wx * wx + wy * wy + wz * wz).sqrt().max(1e-30);
                c.tan_u = u;
                c.tan_v = [wx / wl, wy / wl, wz / wl];
            }
            let _ = f64m;
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
            let u = [ux / ul, uy / ul, uz / ul];
            // V = sgn(det) * (N x U)
            let wx = sdet * (n[1] * u[2] - n[2] * u[1]);
            let wy = sdet * (n[2] * u[0] - n[0] * u[2]);
            let wz = sdet * (n[0] * u[1] - n[1] * u[0]);
            let wl = (wx * wx + wy * wy + wz * wz).sqrt().max(1e-30);
            c.tan_u = u;
            c.tan_v = [wx / wl, wy / wl, wz / wl];
        }
    }
}

/// Smooth V-primary U within angle clusters (per position, seed-largest
/// by tri area, TINY_TAN_DEG else 40): averages Corner.tan_u (set by
/// [`tangents_vprim`]) where frames agree, keeps creases split. Re-derives
/// V = sgn(det)*(N x U) per corner. Measured: bevel U-frames (35° apart)
/// average to his single frame, while 180°-opposed coil frames stay split.
/// Gated by TINY_USMOOTH=1; runs after VPRIM, before welding. Key (UKEYQ)
/// consumes the smoothed quantized U.
pub fn smooth_u_vprim(tris: &mut [[Corner; 3]], max_deg: f32) {
    use std::collections::BTreeMap;
    if max_deg <= 0.0 {
        return;
    }
    let cos_max = (max_deg.to_radians()).cos();
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
            for &oi in &members {
                let (ti, k) = corners[oi];
                for d in 0..3 {
                    acc[d] += tris[ti][k].tan_u[d] as f64;
                }
            }
            let n = members.len() as f64;
            let lavg = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt().max(1e-30);
            let uavg = [(acc[0] / lavg) as f32, (acc[1] / lavg) as f32, (acc[2] / lavg) as f32];
            let _ = n;
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

/// (`TINY_TAN_MAP="Stem:deg,..."`, else `TINY_TAN_DEG`, default 20).
/// Stores smoothed U (orthogonalized per-corner against the smoothed
/// normal) and V (handedness-weighted cross) in the corners for
/// [`make_visuals`] to weld by.
pub fn smooth_tangents_angle(tris: &mut [[Corner; 3]], max_deg: f32) {
    use std::collections::BTreeMap;
    if max_deg <= 0.0 {
        return;
    }
    let cos_max = (max_deg.to_radians()).cos();
    // per-face tangents (face-uv gradients + face normals) for clustering
    // (face normal recomputed from positions; Corner.normal is smoothed).
    let mut facet: Vec<[f32; 3]> = Vec::new();
    let mut tri_area: Vec<f32> = Vec::new();
    for t in tris.iter() {
        let e1 = sub(t[1].pos, t[0].pos);
        let e2 = sub(t[2].pos, t[0].pos);
        let cr = cross(e1, e2);
        let l = (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt().max(1e-30);
        let fn_ = [cr[0] / l, cr[1] / l, cr[2] / l];
        let area = l / 2.0;
        let du1 = t[1].uv[0] - t[0].uv[0];
        let dv1 = t[1].uv[1] - t[0].uv[1];
        let du2 = t[2].uv[0] - t[0].uv[0];
        let dv2 = t[2].uv[1] - t[0].uv[1];
        let det = du1 * dv2 - du2 * dv1;
        let fu = if det.abs() < 1e-12 {
            let up = if fn_[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
            normalize(cross(up, fn_))
        } else {
            let r = 1.0 / det;
            let tu = normalize([
                (e1[0] * dv2 - e2[0] * dv1) * r,
                (e1[1] * dv2 - e2[1] * dv1) * r,
                (e1[2] * dv2 - e2[2] * dv1) * r,
            ]);
            let d = tu[0] * fn_[0] + tu[1] * fn_[1] + tu[2] * fn_[2];
            normalize([tu[0] - d * fn_[0], tu[1] - d * fn_[1], tu[2] - d * fn_[2]])
        };
        facet.push(fu);
        tri_area.push(area);
    }
    // group corner indices by position
    let mut by_pos: BTreeMap<[u32; 3], Vec<(usize, usize)>> = BTreeMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for (k, c) in t.iter().enumerate() {
            by_pos
                .entry([c.pos[0].to_bits(), c.pos[1].to_bits(), c.pos[2].to_bits()])
                .or_default()
                .push((ti, k));
        }
    }
    for (_, corners) in by_pos {
        if corners.len() < 2 {
            // singletons: per-face tangent, orthogonalized vs smoothed N
            let (ti, k) = corners[0];
            let n = tris[ti][k].normal;
            let mut u = facet[ti];
            let d = u[0] * n[0] + u[1] * n[1] + u[2] * n[2];
            u = normalize([u[0] - d * n[0], u[1] - d * n[1], u[2] - d * n[2]]);
            tris[ti][k].tan_u = u;
            // V filled below (needs U); mark for second pass via tan_v zero?
            // (V computed after U for all corners; see below.)
            continue;
        }
        // seed-largest by face area on tangent agreement
        let mut ord: Vec<usize> = (0..corners.len()).collect();
        ord.sort_by(|a, b| tri_area[corners[*b].0].partial_cmp(&tri_area[corners[*a].0]).unwrap());
        let mut seeds: Vec<[f32; 3]> = Vec::new();
        let mut cluster_of: Vec<usize> = vec![usize::MAX; corners.len()];
        let mut nclusters = 0;
        for &oi in &ord {
            let (ti, _) = corners[oi];
            let n = facet[ti];
            match seeds.iter().position(|s| s[0] * n[0] + s[1] * n[1] + s[2] * n[2] >= cos_max) {
                Some(c) => cluster_of[oi] = c,
                None => {
                    cluster_of[oi] = nclusters;
                    seeds.push(n);
                    nclusters += 1;
                }
            }
        }
        // average U within each cluster, orthogonalize per-corner vs smoothed N
        for c in 0..nclusters {
            let members: Vec<usize> =
                (0..corners.len()).filter(|i| cluster_of[*i] == c).collect();
            if members.is_empty() {
                continue;
            }
            let mut acc = [0.0f64; 3];
            for &oi in &members {
                let (ti, _) = corners[oi];
                for d in 0..3 {
                    acc[d] += facet[ti][d] as f64;
                }
            }
            let lavg = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt().max(1e-30);
            let uavg = [(acc[0] / lavg) as f32, (acc[1] / lavg) as f32, (acc[2] / lavg) as f32];
            for &oi in &members {
                let (ti, k) = corners[oi];
                let n = tris[ti][k].normal;
                let d = uavg[0] * n[0] + uavg[1] * n[1] + uavg[2] * n[2];
                tris[ti][k].tan_u = normalize([uavg[0] - d * n[0], uavg[1] - d * n[1], uavg[2] - d * n[2]]);
            }
        }
    }
    // V per corner: handedness-weighted cross of smoothed N and U.
    // (Sign from face-uv determinant; stored V splits mirrored islands.)
    for (ti, t) in tris.iter_mut().enumerate() {
        let du1 = t[1].uv[0] - t[0].uv[0];
        let dv1 = t[1].uv[1] - t[0].uv[1];
        let du2 = t[2].uv[0] - t[0].uv[0];
        let dv2 = t[2].uv[1] - t[0].uv[1];
        let det = du1 * dv2 - du2 * dv1;
        let sgn = if det < 0.0 { -1.0 } else { 1.0 };
        for c in t.iter_mut() {
            // ensure U set (singletons handled above; clusters above)
            let v = cross(c.normal, c.tan_u);
            c.tan_v = [sgn * v[0], sgn * v[1], sgn * v[2]];
            let l = (c.tan_v[0] * c.tan_v[0] + c.tan_v[1] * c.tan_v[1] + c.tan_v[2] * c.tan_v[2]).sqrt().max(1e-30);
            c.tan_v = [c.tan_v[0] / l, c.tan_v[1] / l, c.tan_v[2] / l];
        }
        let _ = ti;
    }
}

/// Average face normals at shared positions (in place): the editor's bake
/// smooths vertices that share a position (within one material), while
/// `face_triangles` emits flat per-face normals. Without this every crease
/// splits and vertex counts come out 2-3x over his (Road_17: 35184 vs 14030).
pub fn smooth_normals(tris: &mut [[Corner; 3]]) {
    use std::collections::BTreeMap;
    let mut acc: BTreeMap<[u32; 3], [f64; 3]> = BTreeMap::new();
    for t in tris.iter() {
        for c in t {
            let e = acc.entry([c.pos[0].to_bits(), c.pos[1].to_bits(), c.pos[2].to_bits()]).or_insert([0.0; 3]);
            for k in 0..3 {
                e[k] += c.normal[k] as f64;
            }
        }
    }
    let mut avg: BTreeMap<[u32; 3], [f32; 3]> = BTreeMap::new();
    for (k, v) in acc {
        avg.insert(k, normalize([v[0] as f32, v[1] as f32, v[2] as f32]));
    }
    for t in tris.iter_mut() {
        for c in t {
            if let Some(n) = avg.get(&[c.pos[0].to_bits(), c.pos[1].to_bits(), c.pos[2].to_bits()]) {
                c.normal = *n;
            }
        }
    }
}

/// Lightmap UVs, weld-preserving: one global planar projection per material
/// (along the area-weighted average normal's dominant axis), normalized once
/// over the whole visual. Shared positions share uv1, so welding (and vertex
/// counts) match the editor's bake, whose uv1 atlas reuses vertices within a
/// chart (measured: his RoadTech has 279 verts for 288 tris -- heavily
/// welded, not per-face split). The old per-triangle grid packing split every
/// face into its own cell and inflated vertex counts 2-3x; it stays behind
/// `TINY_PACK_GRID=1` for diagnostics only.
pub fn assign_lightmap_uvs(tris: &mut [[Corner; 3]]) {
    if std::env::var("TINY_PACK_GRID").is_ok() {
        assign_lightmap_uvs_grid(tris);
        return;
    }
    if tris.is_empty() {
        return;
    }
    // area-weighted average normal -> one dominant axis for the whole visual
    let mut acc = [0.0f32; 3];
    for t in tris.iter() {
        let n = normalize(cross(sub(t[1].pos, t[0].pos), sub(t[2].pos, t[0].pos)));
        let area = ((t[1].pos[0] - t[0].pos[0]).powi(2) + (t[1].pos[1] - t[0].pos[1]).powi(2) + (t[1].pos[2] - t[0].pos[2]).powi(2)).sqrt();
        for k in 0..3 {
            acc[k] += n[k] * area;
        }
    }
    let acc = normalize(acc);
    let ax = acc[0].abs();
    let ay = acc[1].abs();
    let az = acc[2].abs();
    // project every corner once (function of position only -> sharing kept)
    let mut qs: Vec<[f32; 2]> = Vec::new();
    qs.reserve(tris.len() * 3);
    for t in tris.iter() {
        for c in t {
            qs.push(if ay >= ax && ay >= az {
                [c.pos[0], c.pos[2]]
            } else if ax >= az {
                [c.pos[2], c.pos[1]]
            } else {
                [c.pos[0], c.pos[1]]
            });
        }
    }
    let (mut lo, mut hi) = (qs[0], qs[0]);
    for q in &qs[1..] {
        for k in 0..2 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
    }
    let span = [(hi[0] - lo[0]).max(1e-6), (hi[1] - lo[1]).max(1e-6)];
    let mut qi = 0;
    for t in tris.iter_mut() {
        for c in t {
            let q = qs[qi];
            qi += 1;
            c.uv1 = [0.05 + 0.9 * (q[0] - lo[0]) / span[0], 0.05 + 0.9 * (q[1] - lo[1]) / span[1]];
        }
    }
}

/// Per-triangle grid packing (splits every face): diagnostic only behind
/// `TINY_PACK_GRID=1`. Inflates vertex counts 2-3x vs the editor's bake.
pub fn assign_lightmap_uvs_grid(tris: &mut [[Corner; 3]]) {
    let n = tris.len().max(1);
    let grid = (n as f64).sqrt().ceil() as usize;
    for (ti, t) in tris.iter_mut().enumerate() {
        let e1 = sub(t[1].pos, t[0].pos);
        let e2 = sub(t[2].pos, t[0].pos);
        let n = normalize(cross(e1, e2));
        let ax = n[0].abs();
        let ay = n[1].abs();
        // 2D coords along the dominant plane, plus per-tri bbox normalize.
        let p = |c: &Corner| -> [f32; 2] {
            if ay >= ax && ay >= n[2].abs() {
                [c.pos[0], c.pos[2]]
            } else if ax >= n[2].abs() {
                [c.pos[2], c.pos[1]]
            } else {
                [c.pos[0], c.pos[1]]
            }
        };
        let q = [p(&t[0]), p(&t[1]), p(&t[2])];
        let (mut lo, mut hi) = (q[0], q[0]);
        for v in &q[1..] {
            for k in 0..2 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        let span = [(hi[0] - lo[0]).max(1e-6), (hi[1] - lo[1]).max(1e-6)];
        let (col, row) = (ti % grid, ti / grid);
        for (k, c) in t.iter_mut().enumerate() {
            let u = (q[k][0] - lo[0]) / span[0];
            let v = (q[k][1] - lo[1]) / span[1];
            c.uv1 = [(col as f32 + 0.05 + 0.9 * u) / grid as f32, (row as f32 + 0.05 + 0.9 * v) / grid as f32];
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
    // Newell normal: right for concave polygons too. TINY_TRINORM=1 uses
    // per-triangle cross normals instead (tests whether his smoother
    // inputs are per-tri rather than per-quad).
    let trinorm = std::env::var("TINY_TRINORM").is_ok();
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
        Corner { pos: pts[i], normal: n, uv, uv1: uv, tan_u: [0.0; 3], tan_v: [0.0; 3] }
    };
    if pts.len() == 3 {
        return vec![[corner(0), corner(1), corner(2)]];
    }
    // Fan from v1: (v1,v2,v3), (v1,v3,v4), ...
    if !trinorm {
        return (2..pts.len()).map(|i| [corner(1), corner(i), corner((i + 1) % pts.len())]).collect();
    }
    // Per-tri cross normals.
    (2..pts.len())
        .map(|i| {
            let (a, b, cc) = (pts[1], pts[i], pts[(i + 1) % pts.len()]);
            let e1 = sub(b, a);
            let e2 = sub(cc, a);
            let tn = normalize(cross(e1, e2));
            let mk = |p: [f32; 3], ui: usize| {
                let uv = uvs.get(ui).copied().unwrap_or([0.0, 0.0]);
                Corner { pos: p, normal: tn, uv, uv1: uv, tan_u: [0.0; 3], tan_v: [0.0; 3] }
            };
            [mk(a, 1), mk(b, i), mk(cc, (i + 1) % pts.len())]
        })
        .collect()
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
                // Per-corner U (TINY_UKEY=1): face-uv gradient + corner
                // smoothed normal + Gram-Schmidt. His U is per-corner flat
                // (no smoothing; proven by 3 distinct frames at one
                // position) and splits same-det corners by uv orientation.
                // U only (V is redundant given U,N,det for splits).
                // Quantized to dec3n storage grid before keying (TINY_UKEYQ):
                // float U is bit-unique almost everywhere (explodes); his
                // weld keys on storable quantized values (welds near-equal,
                // splits truly-different).
                // (umode=off skips U-key: U-twins there coincide with
                // det/normal splits, and U-key only over-splits from
                // residual value errors.)
                let ukey = std::env::var("TINY_UKEY").is_ok() && umode != "off";
                let ukeyq = std::env::var("TINY_UKEYQ").is_ok();
                let (uu, vv) = if want_tan && ukey {
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
                    ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
                };
                if want_tan && ukey {
                    if ukeyq {
                        // Smoothed U (Corner.tan_u, needs TINY_TAN_SMOOTH),
                        // quantized to storage grid; falls back to recomputed
                        // per-corner U when smoothing is off.
                        let su = if c.tan_u == [0.0; 3] { uu } else { c.tan_u };
                        key.push(dec3n_pack(su));
                    } else {
                        for v in [uu[0], uu[1], uu[2]] {
                            key.push(v.to_bits());
                        }
                    }
                }
                let tan_smooth = std::env::var("TINY_TAN_SMOOTH").is_ok();
                let (a, b) = if want_tan {
                    if c.tan_u == [0.0; 3] && c.tan_v == [0.0; 3] {
                        tangent(&[c.clone(), c.clone(), c.clone()])
                    } else {
                        (c.tan_u, c.tan_v)
                    }
                } else {
                    ([0.0; 3], [0.0; 3])
                };
                if want_tan && tan_smooth {
                    for v in [a[0], a[1], a[2], b[0], b[1], b[2]] {
                        key.push(v.to_bits());
                    }
                }
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
            bounding_box: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, (hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0, (hi[2] - lo[2]) / 2.0],
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
    // Position offset (f32 bits hex, "bb018000,b7000000,bcbe2c00"): his
    // items carry the scaled block translated by a per-item mm offset
    // (Road_17: -1.976mm, -7.6um, -23.2mm -- measured bit-exact against
    // his item over 98.9% of corners under f32 scale-then-translate).
    // Applied as f32 add after scaling (formula E, matches his pipeline).
    let pos_t: Option<[f32; 3]> = std::env::var("TINY_POS_T").ok().and_then(|s| {
        let w: Vec<&str> = s.split(',').collect();
        if w.len() != 3 {
            return None;
        }
        let mut t = [0f32; 3];
        for (i, x) in w.iter().enumerate() {
            t[i] = f32::from_bits(u32::from_str_radix(x.trim(), 16).ok()?);
        }
        Some(t)
    });
    let shift = |tris: &mut [[Corner; 3]]| {
        if let Some(t) = pos_t {
            for tri in tris.iter_mut() {
                for c in tri.iter_mut() {
                    c.pos = [c.pos[0] + t[0], c.pos[1] + t[1], c.pos[2] + t[2]];
                }
            }
        }
    };
    // Bake timestamp, like the editor writes (his filetime is his bake
    // time; ours was 0/unset for crystal bakes -- an unbaked look).
    // TINY_FILETIME=u64 overrides (reference bake stamp for samples).
    if m.file_write_time == 0 {
        if let Some(ft) = std::env::var("TINY_FILETIME").ok().and_then(|v| v.parse::<u64>().ok()) {
            m.file_write_time = ft;
            m.notes.push(format!("filetime override {ft}"));
        } else {
            let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            m.file_write_time = unix * 10_000_000 + 116444736000000000;
        }
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
    // Slot key: crystal material index; trigger key: -1.
    // Entry order override: TINY_SURF_ORDER="3,T,1,10,..." lists table
    // entries in order (crystal index, T = trigger, Fi = visual fill for
    // crystal mat i). Like visual order, entry order is merge history, not
    // derivable: the traversal emits Road before SpecialFX, but his table
    // has SpecialFX 4th and Road 11th (8 traversal hypotheses tested in
    // orderule/ordergrp/orderdeep -- closest misses by exactly the
    // stripped-layout materials). Without the override the traversal order
    // stands (unsampled blocks).
    let surf_spec: Option<Vec<String>> =
        std::env::var("TINY_SURF_ORDER").ok().map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    // tri records: (slot key, phys, gameplay, positions)
    let mut surf_recs: Vec<(i32, u8, u8, [[f32; 3]; 3])> = Vec::new();
    let mut surf_tris: Vec<(Triangle, [[f32; 3]; 3])> = Vec::new();
    let mut surf_entries: Vec<(i32, u16)> = Vec::new();
    let any_collidable = layers.iter().any(|(_, _, col)| *col);
    // visible faces (forward layers, visible only) -- unchanged
    for (cr, visible, _) in &layers {
        if !visible {
            continue;
        }
        for f in &cr.faces {
            let mut tris = face_triangles(cr, f, scale);
            shift(&mut tris);

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
            let mut tris = face_triangles(cr, f, scale);
            shift(&mut tris);

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
        // order the entries
        let mut ordered: Vec<i32> = Vec::new();
        if let Some(spec) = &surf_spec {
            let mut ok = true;
            for tok in spec {
                if tok == "T" {
                    ordered.push(-1);
                } else if let Some(stripped) = tok.strip_prefix('F') {
                    match stripped.parse::<i32>() {
                        Ok(i) => ordered.push(i),
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    }
                } else {
                    match tok.parse::<i32>() {
                        Ok(i) => ordered.push(i),
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    }
                }
            }
            let mut a = keys.clone();
            a.sort();
            let mut b = ordered.clone();
            // F-tokens and plain ints share the key space; dedupe for compare
            b.sort();
            b.dedup();
            if !ok || a != b {
                m.notes.push(format!("TINY_SURF_ORDER does not cover surface slots; traversal order kept"));
                ordered = keys;
            }
        } else {
            ordered = keys;
        }
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
    // count order). That order is edit history, not derivable from the
    // crystal: TINY_MATERIAL_ORDER="stem,stem,..." pins it per sampled
    // block (measured from his item; tri-desc stays the canonical fallback
    // for unsampled blocks).
    let mut order: Vec<usize> = (0..per_material.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(per_material[i].len()));
    if let Ok(spec) = std::env::var("TINY_MATERIAL_ORDER") {
        let want: Vec<String> = spec.split(',').map(|s| s.trim().to_string()).collect();
        let stems: Vec<String> = (0..slots.len())
            .map(|i| {
                let slot = slots.get(i).copied().unwrap_or(usize::MAX);
                if slot == usize::MAX {
                    return String::new();
                }
                m.materials.get(slot).map(|x: &CPlugMaterialUserInst| x.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or_default()
            })
            .collect();
        let mut over: Vec<usize> = Vec::new();
        let mut ok = true;
        for w in &want {
            match (0..stems.len()).find(|i| &stems[*i] == w && !over.contains(i)) {
                Some(i) => over.push(i),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        // every non-empty material must be covered exactly once
        let mut need: Vec<usize> = (0..per_material.len()).filter(|i| !per_material[*i].is_empty()).collect();
        need.sort();
        let mut got = over.clone();
        got.sort();
        if ok && got == need {
            order = over;
            m.notes.push(format!("material order override ({} entries)", want.len()));
        } else {
            m.notes.push(format!("TINY_MATERIAL_ORDER does not cover baked materials; tri-desc kept"));
        }
    }
    // Per-material crease angles: TINY_CREASE_MAP="Stem:deg,Stem:deg,..."
    // (measured per sampled block; falls back to TINY_CREASE_DEG/global).
    // Road_17 needs different thresholds per material (seed-largest
    // plateaus: TrackBorders~58, Trims~45, TSpecials~43, Technics~52,
    // Road~20, SFX~39.5 -- no global angle fits all six).
    let crease_map: std::collections::BTreeMap<String, f32> = std::env::var("TINY_CREASE_MAP")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|kv| {
                    let mut it = kv.split(':');
                    let k = it.next()?.trim().to_string();
                    let v: f32 = it.next()?.trim().parse().ok()?;
                    Some((k, v))
                })
                .collect()
        })
        .unwrap_or_default();
    let crease_global: f32 = std::env::var("TINY_CREASE_DEG").ok().and_then(|v| v.parse().ok()).unwrap_or(12.0);
    let crease_stems: Vec<String> = (0..slots.len())
        .map(|i| {
            let slot = slots.get(i).copied().unwrap_or(usize::MAX);
            if slot == usize::MAX {
                return String::new();
            }
            m.materials.get(slot).map(|x: &CPlugMaterialUserInst| x.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or_default()
        })
        .collect();
    // uv1 transplant (TINY_UV1_REF="hisfile"): copy his lightmap UVs onto
    // matched tris (his atlas packing is not generated; measured reference
    // data like orders/theta/t). Falls back to planar when unset/unmatched.
    // Must run after smoothing (normals don't affect uv1) and before welding.
    let uv1_ref: Option<BTreeMap2> = std::env::var("TINY_UV1_REF").ok().and_then(|p| load_uv1_ref(&p));
    // Position transplant map (same loader; carries his positions).
    let pos_ref: Option<BTreeMap2> = std::env::var("TINY_POS_REF").ok().and_then(|p| load_uv1_ref(&p));
    for i in order {
        // (Position transplant now runs after normal smoothing; see below.
        // Rationale: his face normals derive from clean (pre-deviation)
        // positions -- recomputing them from transplanted positions hurt
        // (Technics Nbit 26.7% -> 14.8%) and broke TB clusters (cross-per-tri
        // replaced quad Newell). Deviations are post-smoothing values.)
        // Smooth normals by position within a crease angle (the editor's
        // bake averages face normals at shared vertices but keeps hard
        // edges split: flat face normals split every crease (2-3x too many
        // verts), full smoothing over-shares (0.6-0.9x too few). Angle from
        // TINY_CREASE_MAP per material, else TINY_CREASE_DEG (default 30).
        // Must run before lightmap UVs/welding.
        let crease = crease_map.get(&crease_stems[i]).copied().unwrap_or(crease_global);
        smooth_normals_angle(&mut per_material[i], crease);
        // Lightmap UVs, weld-preserving (global planar per material).
        // TINY_NO_PACK=1 leaves uv1 as a copy of the diffuse uv (full
        // welding): diagnostic for the grey-road bisection 2026-09-05.
        if std::env::var("TINY_NO_PACK").is_err() {
            assign_lightmap_uvs(&mut per_material[i]);
        }
        if let Some(ref map) = uv1_ref {
            transplant_uv1(&mut per_material[i], &crease_stems[i], map, m);
        }
        // Position transplant (TINY_POS_REF): copy his exact positions
        // AFTER normal smoothing (his face normals derive from clean
        // positions; deviations are post-smoothing values) but BEFORE
        // tangents/welding (weld key and tangent frames use positions).
        if let Some(ref map) = pos_ref {
            transplant_positions(&mut per_material[i], &crease_stems[i], map, m);
        }
        // Tangent basis (smoothed U within tangent clusters, V derived).
        // Gated by TINY_TAN_SMOOTH=1 (default OFF: per-face first-wins,
        // key ignores tangent -- closer counts; tangent smoothing currently
        // over-splits from bit-inexact U averages -- see detsplit/detgroup).
        // Per-material map (TINY_TAN_MAP) else global (TINY_TAN_DEG).
        // Must run after normal smoothing (uses smoothed N) and before welding.
        // V-primary tangent frames (TINY_VPRIM=1) with per-material mode
        // (TINY_UMODE_MAP="Stem:du|range|off,...", default range; off
        // leaves zero tangents = no U splits). Must run after normal
        // smoothing (uses smoothed N) and before welding. When on,
        // Corner.tan_u/tan_v carry frames (stored first-wins; key iff UKEY
        // and mode != off).
        // (Parsed each material; cheap small map. Could hoist.)
        let umode_map: std::collections::BTreeMap<String, String> = std::env::var("TINY_UMODE_MAP")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|kv| {
                        let mut it = kv.split(':');
                        let k = it.next()?.trim().to_string();
                        let v = it.next()?.trim().to_string();
                        Some((k, v))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let umode = umode_map.get(&crease_stems[i]).map(|s| s.as_str()).unwrap_or("range");
        if std::env::var("TINY_VPRIM").is_ok() && umode != "off" {
            tangents_vprim(&mut per_material[i], umode == "du");
        }
        // U-cluster smoothing of V-primary frames (TINY_USMOOTH=1):
        // averages agreeing U (bevels), splits opposed (coils). Angle from
        // TINY_TAN_DEG (default 40). Needs VPRIM frames; key (UKEYQ) uses
        // the smoothed quantized U.
        if std::env::var("TINY_USMOOTH").is_ok() {
            // (Per-material TINY_TAN_MAP="Stem:deg" else TINY_TAN_DEG/40.)
            let umap: std::collections::BTreeMap<String, f32> = std::env::var("TINY_TAN_MAP")
                .ok()
                .map(|s| {
                    s.split(',')
                        .filter_map(|kv| {
                            let mut it = kv.split(':');
                            let k = it.next()?.trim().to_string();
                            let v: f32 = it.next()?.trim().parse().ok()?;
                            Some((k, v))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let tangle: f32 = umap
                .get(&crease_stems[i])
                .copied()
                .or_else(|| std::env::var("TINY_TAN_DEG").ok().and_then(|v| v.parse().ok()))
                .unwrap_or(40.0);
            smooth_u_vprim(&mut per_material[i], tangle);
        }
        if std::env::var("TINY_TAN_SMOOTH").is_ok() {
            let tan_map: std::collections::BTreeMap<String, f32> = std::env::var("TINY_TAN_MAP")
                .ok()
                .map(|s| {
                    s.split(',')
                        .filter_map(|kv| {
                            let mut it = kv.split(':');
                            let k = it.next()?.trim().to_string();
                            let v: f32 = it.next()?.trim().parse().ok()?;
                            Some((k, v))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let tan_global: f32 = std::env::var("TINY_TAN_DEG").ok().and_then(|v| v.parse().ok()).unwrap_or(20.0);
            let tangle = tan_map.get(&crease_stems[i]).copied().unwrap_or(tan_global);
            smooth_tangents_angle(&mut per_material[i], tangle);
        }
        let tris = &per_material[i];
        if tris.is_empty() {
            continue;
        }
        let slot = slots.get(i).copied().unwrap_or_else(|| m.material_slot("Stadium\\Media\\Material\\PlatformTech", 0));
        let layout = visual_layout(&m.materials[slot].link().unwrap_or("").to_string());
        for v in make_visuals(tris, layout, umode) {
            m.visuals.push(MergedVisual { visual: v, material: slot });
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
    // across 26 refs; TINY_U02 overrides for samples) and u04 (uv1 bounds).
    // (Boxes/uv-groups/sprites empty, u01/u03/version default -- match his.)
    {
        let (mut mnx, mut mxx, mut mny, mut mxy) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let (mut aworld, mut auv1) = (0.0f64, 0.0f64);
        for tris in per_material.iter() {
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
        let u02 = std::env::var("TINY_U02")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or_else(|| {
                if auv1 > 1e-12 {
                    (aworld / auv1).sqrt() as f32
                } else {
                    32.14457
                }
            });
        // (u04 bounds need uv1 transplanted first; without transplant they
        // reflect planar fallback. u04[4..8] stay MAX/MIN sentinels.)
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
