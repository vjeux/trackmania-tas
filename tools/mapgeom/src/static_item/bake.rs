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

/// Transplant a face's fan tris to reference positions AND refresh the
/// quad Newell normal from the transplanted corners (in face order), so
/// smoothing inputs are reference-source face normals rather than ours
/// (the 2022-vs-2025 micro-deviations shift face normals ~1e-4..1e-3,
/// flipping 27-73% of dec3n words at quantum boundaries).
/// Runs pre-smoothing (right after shift in the face loops) when
/// TINY_POS_REF is set; silent (the later transplant_positions call
/// re-runs idempotently and reports notes).
/// Fan pattern matches [`face_triangles`]: tri ti (0-based) covers quad
/// indices [1, ti+2, (ti+3)%quad_len].
fn refresh_face(tris: &mut [[Corner; 3]], quad_len: usize, stem: &str, map: &BTreeMap2) {
    if tris.is_empty() || quad_len < 3 {
        return;
    }
    // snapshot pre-transplant positions
    let mut pre: Vec<[[f32; 3]; 3]> = Vec::with_capacity(tris.len());
    for t in tris.iter() {
        pre.push([t[0].pos, t[1].pos, t[2].pos]);
    }
    let qidx = |ti: usize, k: usize| -> usize {
        match k {
            0 => 1 % quad_len,
            1 => (ti + 2) % quad_len,
            _ => (ti + 3) % quad_len,
        }
    };
    // transplant corners (same mmkey + 2mm-gate match as transplant_positions)
    for (ti, t) in tris.iter_mut().enumerate() {
        let mut key = [
            ((pre[ti][0][0] * 1000.0).round() as i32,
                (pre[ti][0][1] * 1000.0).round() as i32,
                (pre[ti][0][2] * 1000.0).round() as i32),
            ((pre[ti][1][0] * 1000.0).round() as i32,
                (pre[ti][1][1] * 1000.0).round() as i32,
                (pre[ti][1][2] * 1000.0).round() as i32),
            ((pre[ti][2][0] * 1000.0).round() as i32,
                (pre[ti][2][1] * 1000.0).round() as i32,
                (pre[ti][2][2] * 1000.0).round() as i32),
        ];
        key.sort();
        if let Some(hc) = map.get(&(stem.to_string(), key)) {
            for (k, c) in t.iter_mut().enumerate() {
                let mut best: Option<[f32; 3]> = None;
                let mut bestd = 0.002f32;
                for (hp, _) in hc {
                    let dd = ((hp[0] - pre[ti][k][0]).powi(2)
                        + (hp[1] - pre[ti][k][1]).powi(2)
                        + (hp[2] - pre[ti][k][2]).powi(2))
                    .sqrt();
                    if dd < bestd {
                        bestd = dd;
                        best = Some(*hp);
                    }
                }
                if let Some(p) = best {
                    c.pos = p;
                }
            }
        }
    }
    // transplanted quad corners in face order (nearest-to-pre reconciles
    // fan corners that matched different reference corners per tri)
    let mut qp = vec![[0f32; 3]; quad_len];
    let mut qq = vec![[0f32; 3]; quad_len];
    let mut have = vec![false; quad_len];
    for (ti, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let j = qidx(ti, k);
            if !have[j] {
                qq[j] = pre[ti][k];
                have[j] = true;
            }
        }
    }
    if have.iter().any(|h| !h) {
        return; // fan didn't cover a vert; keep source normals
    }
    for j in 0..quad_len {
        let mut best = qq[j];
        let mut bestd = f32::MAX;
        for (ti, t) in tris.iter().enumerate() {
            for k in 0..3 {
                if qidx(ti, k) != j {
                    continue;
                }
                let dd = ((t[k].pos[0] - qq[j][0]).powi(2)
                    + (t[k].pos[1] - qq[j][1]).powi(2)
                    + (t[k].pos[2] - qq[j][2]).powi(2))
                .sqrt();
                if dd < bestd {
                    bestd = dd;
                    best = t[k].pos;
                }
            }
        }
        qp[j] = best;
    }
    // Newell over transplanted quad, assigned to every corner
    let mut n = [0f32; 3];
    for i in 0..quad_len {
        let a = qp[i];
        let b = qp[(i + 1) % quad_len];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let n = normalize(n);
    for t in tris.iter_mut() {
        for c in t.iter_mut() {
            c.normal = n;
        }
    }
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
    /// Source polygon id (per material stream; the fan tris of one crystal
    /// face share it). 0 when unknown. Used for one-vote-per-face normal
    /// averaging (`smooth_normals_corner` under TINY_NDEDUP=face).
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
        let areas: Vec<f32> = corners.iter().map(|&(ti, _)| tri_area_of(&tris[ti])).collect();
        let angles: Vec<f32> = corners.iter().map(|&(ti, k)| corner_angle(&tris[ti], k)).collect();
        let mut out: Vec<[f32; 3]> = Vec::with_capacity(corners.len());
        let dedup = std::env::var("TINY_NDEDUP").ok();
        // TINY_NUV_EPS=e: a face only joins a corner's average when its
        // corner uv is within e (max-norm) of this corner's uv -- a UV seam
        // blocks smoothing (his TB verts at the 12.0/0.0 tile wrap average
        // only their own side although the faces are 37deg apart).
        let uv_eps: Option<f32> = std::env::var("TINY_NUV_EPS").ok().and_then(|v| v.parse().ok());
        let uvs: Vec<[f32; 2]> = corners.iter().map(|&(ti, k)| tris[ti][k].uv).collect();
        for i in 0..corners.len() {
            let me = ns[i];
            let mut sel: Vec<usize> = (0..corners.len())
                .filter(|&j| me[0] * ns[j][0] + me[1] * ns[j][1] + me[2] * ns[j][2] >= cos_max)
                .filter(|&j| match uv_eps {
                    Some(e) => (uvs[i][0] - uvs[j][0]).abs() <= e && (uvs[i][1] - uvs[j][1]).abs() <= e,
                    None => true,
                })
                .collect();
            match dedup.as_deref() {
                Some("face") => {
                    // one vote per source polygon (Corner::face)
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
                }
                Some(_) => {
                    // one vote per distinct face normal (a quad's two tris share
                    // one Newell normal): TINY_NDEDUP=1
                    let mut seen: Vec<[u32; 3]> = Vec::new();
                    sel.retain(|&j| {
                        let b = [ns[j][0].to_bits(), ns[j][1].to_bits(), ns[j][2].to_bits()];
                        if seen.contains(&b) {
                            false
                        } else {
                            seen.push(b);
                            true
                        }
                    });
                }
                None => {}
            }
            let sn: Vec<[f32; 3]> = sel.iter().map(|&j| ns[j]).collect();
            let sa: Vec<f32> = sel.iter().map(|&j| areas[j]).collect();
            let sg: Vec<f32> = sel.iter().map(|&j| angles[j]).collect();
            out.push(cluster_normal_value(&sn, &sa, &sg, angle_weight));
        }
        for (i, &(ti, k)) in corners.iter().enumerate() {
            tris[ti][k].normal = out[i];
        }
    }
}

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
            let angles: Vec<f32> = corners.iter().map(|&(ti, k)| corner_angle(&tris[ti], k)).collect();
            let n = cluster_normal_value(&ns, &areas, &angles, angle_weight);
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
                let angles: Vec<f32> = members.iter().map(|&(ti, k)| corner_angle(&tris[ti], k)).collect();
                let n = cluster_normal_value(&ns, &areas, &angles, angle_weight);
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
fn cluster_normal_value(ns: &[[f32; 3]], areas: &[f32], angles: &[f32], use_angle: bool) -> [f32; 3] {
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
        let w = if use_angle {
            angles[i] as f64
        } else if area_w {
            areas[i] as f64
        } else {
            1.0
        };
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
        // Primary gradient by mode (1 = forced du, 2 = forced dv,
        // 0 = uv-range stability): du uses U=GS(du,N), V=sgn*NxU; dv uses
        // U=sgn*NxVg under USGN else NxVg, V=NxU under USGN else sgn*NxU.
        // Per-material mode via UMODE_MAP at caller. Branch MUST equal
        // tri_primary_du (U-cluster purity recomputes it).
        if tri_primary_du(t, mode) {
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
                // (du-branch U is unsigned; sign lives on V. See dv branch.)
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
            // TINY_USGN=1: dv-branch U carries sgn(det) (measured: his U
            // = sgn(det)·(N×Vg) -- -N×Vg on det<0 slivers, +N×Vg on det>0;
            // V = N×U below then matches with no extra sign). du-branch U
            // stays unsigned (single-face spots: +du half-exact, both signs).
            let usgn = if std::env::var("TINY_USGN").is_ok() { sdet } else { 1.0 };
            let u = [usgn * ux / ul, usgn * uy / ul, usgn * uz / ul];
            // V = (USGN ? 1 : sgn(det)) * (N x U): with signed U the extra
            // sign would cancel (sdet*sdet=1) and break V.
            let vs = if std::env::var("TINY_USGN").is_ok() { 1.0 } else { sdet };
            let wx = vs * (n[1] * u[2] - n[2] * u[1]);
            let wy = vs * (n[2] * u[0] - n[0] * u[2]);
            let wz = vs * (n[0] * u[1] - n[1] * u[0]);
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
/// by tri area, TINY_TAN_DEG else 40): averages Corner.tan_u (set by
/// [`tangents_vprim`]) where frames agree, keeps creases split. Re-derives
/// V = sgn(det)*(N x U) per corner. Measured: bevel U-frames (35° apart)
/// average to his single frame, while 180°-opposed coil frames stay split.
/// Gated by TINY_USMOOTH=1; runs after VPRIM, before welding. Key (UKEYQ)
/// consumes the smoothed quantized U.
pub fn smooth_u_vprim(tris: &mut [[Corner; 3]], max_deg: f32, angle_weight: bool, mag_weight: bool, mode: u8) {
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
                // re-derive V = sgn(det)*(N x U). TINY_KEEPFLATV=1 keeps
                // the VPRIM-flat V instead (flat V in key splits slivers
                // whose per-face V disagrees; smoothed U welds the rest).
                if std::env::var("TINY_KEEPFLATV").is_ok() {
                    continue;
                }
                let t = &tris[ti];
                let du1 = t[1].uv[0] - t[0].uv[0];
                let dv1 = t[1].uv[1] - t[0].uv[1];
                let du2 = t[2].uv[0] - t[0].uv[0];
                let dv2 = t[2].uv[1] - t[0].uv[1];
                let det = du1 * dv2 - du2 * dv1;
                // V re-derive sign follows the corner's primary: dv corners
                // carry sgn in U already (USGN) so s=1; du corners need
                // s=sgn(det) as stored. (No USGN: all s=sgn(det).)
                let dup = tri_primary_du(&tris[ti], mode);
                let s = if std::env::var("TINY_USGN").is_ok() && !dup {
                    1.0
                } else if det.is_sign_negative() {
                    -1.0
                } else {
                    1.0
                };
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
/// non-overlap, uniform density, seam rules), not his bytes -- use
/// TINY_UV1_REF to transplant his uv1 when a reference exists.
/// Knobs: TINY_LM_DIH (default 45), TINY_LM_SEED (50), TINY_LM_GUTTER
/// (0.001), TINY_LM_MARGIN (0.001).
pub fn assign_lightmap_atlas(per_material: &mut [Vec<[Corner; 3]>], has_uv1: &[bool]) {
    use std::collections::{BTreeMap, VecDeque};
    let envf = |k: &str, d: f32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let dih_max = envf("TINY_LM_DIH", 45.0).to_radians().cos();
    let seed_max = envf("TINY_LM_SEED", 50.0).to_radians().cos();
    let gutter = envf("TINY_LM_GUTTER", 0.001);
    let margin = envf("TINY_LM_MARGIN", 0.001);
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
    // the item got 9% texel coverage. Split any chart with fill < TINY_LM_FILL
    // (0.4) and > 4 tris in two along its longer box axis (projected
    // centroid), up to 4 levels, then re-frame the halves.
    {
        let fill_min = envf("TINY_LM_FILL", 0.4);
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
    if std::env::var("TINY_LM_NOTE").is_ok() {
        let mut per_mat: BTreeMap<usize, usize> = BTreeMap::new();
        for c in &charts {
            *per_mat.entry(c.mat).or_insert(0) += 1;
        }
        let (mut maxw, mut maxh, mut bbsum) = (0.0f32, 0.0f32, 0.0f64);
        let mut rows = 0usize;
        let mut last_y = -1.0f32;
        for c in &charts {
            let (w, h, _) = dims(c);
            maxw = maxw.max(w);
            maxh = maxh.max(h);
            bbsum += (w as f64 * s as f64) * (h as f64 * s as f64);
            if c.y != last_y {
                rows += 1;
                last_y = c.y;
            }
        }
        eprintln!("  note: lightmap atlas: {} charts, scale {:.5} uv/m, per material {:?}; largest chart {:.2}x{:.2} m, bbox fill {:.3}, ~{} rows", charts.len(), s, per_mat, maxw, maxh, bbsum, rows);
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
        Corner { pos: pts[i], normal: n, uv, uv1: uv, tan_u: [0.0; 3], tan_v: [0.0; 3], face: 0, group: 0 }
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
                Corner { pos: p, normal: tn, uv, uv1: uv, tan_u: [0.0; 3], tan_v: [0.0; 3], face: 0, group: 0 }
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
                // (umode=off/duoff skips U-key: U-twins there coincide with
                // det/normal splits, and U-key only over-splits from
                // residual value errors.)
                let ukey = std::env::var("TINY_UKEY").is_ok() && umode != "off" && umode != "duoff";
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
                        // Smoothed U (Corner.tan_u) + V (Corner.tan_v),
                        // quantized to storage grid (fullkey proved the weld
                        // key is (pos,n,uv,uv1,U,V); V was missing and
                        // near-identical U twins with different V welded).
                        // Falls back to recomputed per-corner U/V when the
                        // stored frames are zero (smoothing off / degenerate).
                        let (su, sv) = if c.tan_u == [0.0; 3] && c.tan_v == [0.0; 3] {
                            (uu, vv)
                        } else if c.tan_v == [0.0; 3] {
                            (c.tan_u, vv)
                        } else if c.tan_u == [0.0; 3] {
                            (uu, c.tan_v)
                        } else {
                            (c.tan_u, c.tan_v)
                        };
                        key.push(dec3n_pack(su));
                        // V recompute (vv) is du-GS based and unstable on
                        // slivers; prefer stored VPRIM V, fall back to vv.
                        // (TINY_NO_VKEY=1 restores U-only key.)
                        if std::env::var("TINY_NO_VKEY").is_err() {
                            key.push(dec3n_pack(sv));
                        }
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
    // Reference position map, loaded early so faces transplant to
    // reference positions BEFORE smoothing (smoothing inputs become
    // reference-source face normals via refresh_face below, not ours).
    // (Same loader as the later transplant_positions call, which re-runs
    // idempotently and reports notes.)
    let pos_ref_early: Option<BTreeMap2> = std::env::var("TINY_POS_REF").ok().and_then(|p| load_uv1_ref(&p));
    // stem per crystal-material index (same construction as crease_stems
    // below; needed here for the transplant map key).
    let face_stems: Vec<String> = (0..slots.len())
        .map(|i| {
            let slot = slots.get(i).copied().unwrap_or(usize::MAX);
            if slot == usize::MAX {
                return String::new();
            }
            m.materials.get(slot).map(|x: &CPlugMaterialUserInst| x.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or_default()
        })
        .collect();
    // visible faces (forward layers, visible only) -- unchanged
    let mut face_id: u32 = 0;
    for (cr, visible, _) in &layers {
        if !visible {
            continue;
        }
        for f in &cr.faces {
            let mut tris = face_triangles(cr, f, scale);
            shift(&mut tris);
            face_id += 1;
            for t in tris.iter_mut() {
                for c in t.iter_mut() {
                    c.face = face_id;
                    c.group = f.group;
                }
            }

            let slot_i = if f.material >= 0 && (f.material as usize) < slots.len() { Some(f.material as usize) } else { None };
            if let Some(ref map) = pos_ref_early {
                let stem = slot_i.and_then(|i| face_stems.get(i)).map(|s| s.as_str()).unwrap_or("");
                refresh_face(&mut tris, f.verts.len(), stem, map);
            }
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
            if let Some(ref map) = pos_ref_early {
                let stem = slot_i.and_then(|i| face_stems.get(i)).map(|s| s.as_str()).unwrap_or("");
                refresh_face(&mut tris, f.verts.len(), stem, map);
            }
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
    // (Faces already transplanted pre-smoothing via refresh_face above,
    // so smoothing inputs are reference-source face normals; this call
    // re-runs idempotently over the smoothed tris and reports notes.)
    let pos_ref: Option<BTreeMap2> = std::env::var("TINY_POS_REF").ok().and_then(|p| load_uv1_ref(&p));
    // Item-global lightmap atlas (generated) when no reference uv1 is
    // transplanted (TINY_UV1_REF unset) and TINY_NO_PACK/TINY_UV1_PLANAR are
    // unset (TINY_UV1_PLANAR=1 keeps the old per-material planar projection).
    // Runs before smoothing (position-only), per_material order = slots.
    let has_uv1: Vec<bool> = (0..per_material.len())
        .map(|i| {
            let slot = slots.get(i).copied();
            let link = slot.and_then(|s| m.materials.get(s)).and_then(|x: &CPlugMaterialUserInst| x.link().map(|l| l.to_string())).unwrap_or_default();
            layout_decls(visual_layout(&link)).iter().any(|d| d.name() == N_TEXCOORD0 + 1)
        })
        .collect();
    let atlas_done = uv1_ref.is_none() && std::env::var("TINY_NO_PACK").is_err() && std::env::var("TINY_UV1_PLANAR").is_err() && {
        assign_lightmap_atlas(&mut per_material, &has_uv1);
        true
    };
    for i in order {
        // (Position transplant runs pre-smoothing (refresh_face: reference
        // face normals as smoothing inputs) and here post-smoothing
        // (idempotent position overwrite + notes). The old order (smooth
        // first, transplant after) fed 2022-source face normals into
        // smoothing, shifting values ~1e-4..1e-3 and flipping 27-73% of
        // dec3n words at quantum boundaries. Recomputing normals from
        // transplanted positions must stay quad-Newell (refresh_face);
        // per-tri cross recompute was tested and rejected.)
        // Smooth normals by position within a crease angle (the editor's
        // bake averages face normals at shared vertices but keeps hard
        // edges split: flat face normals split every crease (2-3x too many
        // verts), full smoothing over-shares (0.6-0.9x too few). Angle from
        // TINY_CREASE_MAP per material, else TINY_CREASE_DEG (default 30).
        // Must run before lightmap UVs/welding.
        let crease = crease_map.get(&crease_stems[i]).copied().unwrap_or(crease_global);
        // Normal average weight law (TINY_WEIGHT_MAP="Stem:angle,...",
        // else uniform; Road/Sign stay uniform -- already 100% bit-exact
        // with uniform, and any reweighting risks boundary-word flips).
        // (Parsed each material; cheap small map. Could hoist.)
        let angle_w = std::env::var("TINY_WEIGHT_MAP")
            .ok()
            .map(|s| {
                s.split(',').any(|kv| {
                    let mut it = kv.split(':');
                    it.next().map(|k| k.trim()).unwrap_or("")
                        == crease_stems[i]
                        && it.next().map(|v| v.trim()) == Some("angle")
                })
            })
            .unwrap_or(false);
        // TINY_NMODE_MAP="Stem:corner,..." (or TINY_NMODE=corner for all)
        // selects per-corner threshold averaging (smooth_normals_corner)
        // instead of clustering.
        let corner_mode = std::env::var("TINY_NMODE").map(|v| v == "corner").unwrap_or(false)
            || std::env::var("TINY_NMODE_MAP")
                .ok()
                .map(|s| {
                    s.split(',').any(|kv| {
                        let mut it = kv.split(':');
                        it.next().map(|k| k.trim()).unwrap_or("") == crease_stems[i] && it.next().map(|v| v.trim()) == Some("corner")
                    })
                })
                .unwrap_or(false);
        if corner_mode {
            smooth_normals_corner(&mut per_material[i], crease, angle_w);
        } else {
            smooth_normals_angle(&mut per_material[i], crease, angle_w);
        }
        // Lightmap UVs, weld-preserving (global planar per material).
        // TINY_NO_PACK=1 leaves uv1 as a copy of the diffuse uv (full
        // welding): diagnostic for the grey-road bisection 2026-09-05.
        if std::env::var("TINY_NO_PACK").is_err() && !atlas_done {
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
        // Per-material U mode (TINY_UMODE_MAP="Stem:du|range|off|duoff"):
        // du/range select the primary gradient for VPRIM frames (du was
        // proven for Road/TB/Technics coils; range mis-picks dv there);
        // off computes range frames but skips U-key splits; duoff computes
        // du frames but skips U-key splits (TB: du values are right but
        // U-key over-splits det-coincident twins).
        let umode = umode_map.get(&crease_stems[i]).map(|s| s.as_str()).unwrap_or("range");
        // Primary mode shared by tangents_vprim (frame branch) and
        // smooth_u_vprim (primary-pure clusters + V re-derive recompute
        // it): du|duoff = 1, dv = 2, else range = 0.
        let umode_n: u8 = if umode == "du" || umode == "duoff" { 1 } else if umode == "dv" { 2 } else { 0 };
        if std::env::var("TINY_VPRIM").is_ok() {
            tangents_vprim(&mut per_material[i], umode_n);
        }
        // U-cluster smoothing of V-primary frames (TINY_USMOOTH=1):
        // averages agreeing U (bevels), splits opposed (coils). Angle from
        // TINY_TAN_DEG (default 40). Needs VPRIM frames; key (UKEYQ) uses
        // the smoothed quantized U.
        if std::env::var("TINY_USMOOTH").is_ok() && umode != "off" {
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
            // Same weight law as normals (TINY_WEIGHT_MAP): U-cluster
            // averages are angle-weighted too (standard algorithm).
            let angle_w = std::env::var("TINY_WEIGHT_MAP")
                .ok()
                .map(|s| {
                    s.split(',').any(|kv| {
                        let mut it = kv.split(':');
                        it.next().map(|k| k.trim()).unwrap_or("")
                            == crease_stems[i]
                            && it.next().map(|v| v.trim()) == Some("angle")
                    })
                })
                .unwrap_or(false);
            // U magnitude weights (TINY_UMAG_MAP="Stem,..."): MikkTSpace-style
            // |du-grad| weights (uweight: TSpecials 88.8% vs 85.0%).
            let mag_w = std::env::var("TINY_UMAG_MAP")
                .ok()
                .map(|s| {
                    s.split(',').any(|kv| kv.trim() == crease_stems[i])
                })
                .unwrap_or(false);
            smooth_u_vprim(&mut per_material[i], tangle, angle_w, mag_w, umode_n);
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
        // TINY_U02 (his measured value) applies to transplanted uv1 only; a
        // generated atlas gets its own texel scale.
        let u02 = std::env::var("TINY_U02")
            .ok()
            .filter(|_| !atlas_done)
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
