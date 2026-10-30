//! The ice start/finish decals re-UVed for the plain decal materials (2026-10-29).
//!
//! `RoadIce` Start_Air / Finish_Airv2 paint their sponsor logos and finish marks with
//! `…OnRoadIce` materials: a LAYERED shader that tiles the ice through TexCoord0
//! (block-space coordinates, u = 1 + z/32-ish, v = x/32-ish — nothing a plain decal
//! texture can use) and finds the logo through the material's own tables. Every
//! non-ice set swaps them to the plain twins (`DecalPaintSponsor4x1*`,
//! `DecalSponsor1x1BigA`, `DecalMarks`), whose `DecalGeom` shader samples the logo with
//! TexCoord0 straight — so a converted bowl showed slivers of logo and mostly blank
//! blue (Everios96 2026-10-29 14:53: "only parts of them are showing").
//!
//! The tech start/finish (`RoadTech` Start_Air / Finish_Air) carry the SAME decal
//! layout with the plain materials, so they are the reference: every decal PATCH of
//! the tech prefab (one connected component of one decal visual) yields the
//! directions its u and v run in across the block (an affine fit of (x,z) → (u,v)).
//! An ice decal patch takes the directions of the nearest tech patch of the same
//! logo and gets 0..1 along each, over its own extent — the logo fills the patch the
//! way the tech one does, read the same way round. A patch with no tech twin
//! (another layout) runs u along its long axis, v across.

use super::merged::{visual_triangles, Merged};
use super::visual::CPlugVisualIndexedTriangles;
use super::vstream::{Elem, N_TEXCOORD0};

/// One reference decal patch: which logo, where (xz centroid), and the directions
/// u and v grow in across the block (unit vectors in xz).
#[derive(Clone, Debug)]
pub struct RefPatch {
    pub key: String,
    pub centroid: [f32; 2],
    pub u_dir: [f32; 2],
    pub v_dir: [f32; 2],
    /// the uv band the reference patch occupies (the finish line is the `DecalMarks`
    /// atlas's u 0.75..1.0 band, a sponsor logo the whole 0..1)
    pub u_range: (f32, f32),
    pub v_range: (f32, f32),
}

/// The decal family of a material link: `DecalPaintSponsor4x1B`, `DecalSponsor4x1BOnRoadIce`
/// and `DecalPaint2Sponsor4x1B` are all `4x1B`; `DecalSponsor1x1BigA[OnRoadIce]` is `1x1BigA`;
/// `DecalMarks[OnRoadIce]` is `Marks`, `DecalMarksStart[OnRoadIce]` is `MarksStart`.
pub fn decal_key(link: &str) -> Option<String> {
    let stem = link.rsplit('\\').next().unwrap_or(link).trim_end_matches(".Material.Gbx").trim_end_matches(".Material.gbx");
    if !stem.starts_with("Decal") {
        return None;
    }
    let mut k = stem.to_string();
    for p in ["DecalPaint2", "DecalPaint", "Decal"] {
        if let Some(r) = k.strip_prefix(p) {
            k = r.to_string();
            break;
        }
    }
    for s in ["OnRoadIce", "NoColorize"] {
        if let Some(r) = k.strip_suffix(s) {
            k = r.to_string();
        }
    }
    k = k.replace("Sponsor", "");
    if k.is_empty() || !(k.contains("4x1") || k.contains("BigA") || k.starts_with("Marks")) {
        return None;
    }
    Some(k)
}

/// The triangles of a visual grouped into connected components (shared vertices).
fn components(n_verts: usize, idx: &[u32]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..n_verts).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for t in idx.chunks(3) {
        if t.len() < 3 {
            break;
        }
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        if a >= n_verts || b >= n_verts || c >= n_verts {
            continue;
        }
        let ra = find(&mut parent, a);
        let rb = find(&mut parent, b);
        parent[rb] = ra;
        let ra = find(&mut parent, a);
        let rc = find(&mut parent, c);
        parent[rc] = ra;
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for i in 0..n_verts {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    groups.into_values().filter(|g| g.len() >= 3).collect()
}

/// Least-squares fit of `c = a·(x,z) + b` over the points; returns `a` (the gradient).
fn gradient(pts: &[[f32; 2]], vals: &[f32]) -> Option<[f32; 2]> {
    let n = pts.len() as f64;
    if n < 3.0 {
        return None;
    }
    let (mx, mz, mc) = (pts.iter().map(|p| p[0] as f64).sum::<f64>() / n, pts.iter().map(|p| p[1] as f64).sum::<f64>() / n, vals.iter().map(|v| *v as f64).sum::<f64>() / n);
    let (mut sxx, mut sxz, mut szz, mut sxc, mut szc) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for (p, c) in pts.iter().zip(vals.iter()) {
        let (dx, dz, dc) = (p[0] as f64 - mx, p[1] as f64 - mz, *c as f64 - mc);
        sxx += dx * dx;
        sxz += dx * dz;
        szz += dz * dz;
        sxc += dx * dc;
        szc += dz * dc;
    }
    let det = sxx * szz - sxz * sxz;
    if det.abs() < 1e-9 {
        // a degenerate (collinear) patch: the gradient along the one axis it spans
        return if sxx > szz { Some([(sxc / sxx.max(1e-9)) as f32, 0.0]) } else { Some([0.0, (szc / szz.max(1e-9)) as f32]) };
    }
    let ax = (sxc * szz - szc * sxz) / det;
    let az = (szc * sxx - sxc * sxz) / det;
    Some([ax as f32, az as f32])
}

fn unit(v: [f32; 2]) -> Option<[f32; 2]> {
    let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
    (l > 1e-6).then(|| [v[0] / l, v[1] / l])
}

/// The reference patches of a prefab's plain decals (the tech start/finish).
pub fn reference_patches(store: &mut crate::store::DataStore, prefab: &str) -> Result<Vec<RefPatch>, String> {
    let mut m = Merged::default();
    super::build::add_prefab(store, prefab, &crate::geom::IDENTITY, 1.0, &mut m, 0)?;
    let mut out = Vec::new();
    for mv in &m.visuals {
        let Some(link) = m.materials.get(mv.material).and_then(|x| x.link()) else { continue };
        let Some(key) = decal_key(link) else { continue };
        let (pos, idx) = visual_triangles(&mv.visual);
        let uv: Vec<[f32; 2]> = mv.visual.stream().and_then(|s| s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == N_TEXCOORD0).and_then(|(_, e)| if let Elem::Float2(u) = e { Some(u.clone()) } else { None })).unwrap_or_default();
        if pos.is_empty() || uv.len() != pos.len() {
            continue;
        }
        for comp in components(pos.len(), &idx) {
            let pts: Vec<[f32; 2]> = comp.iter().map(|&i| [pos[i][0], pos[i][2]]).collect();
            let us: Vec<f32> = comp.iter().map(|&i| uv[i][0]).collect();
            let vs: Vec<f32> = comp.iter().map(|&i| uv[i][1]).collect();
            let (Some(gu), Some(gv)) = (gradient(&pts, &us), gradient(&pts, &vs)) else { continue };
            let (Some(u_dir), Some(v_dir)) = (unit(gu), unit(gv)) else { continue };
            let n = pts.len() as f32;
            let centroid = [pts.iter().map(|p| p[0]).sum::<f32>() / n, pts.iter().map(|p| p[1]).sum::<f32>() / n];
            let u_range = us.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(*q), b.max(*q)));
            let v_range = vs.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(*q), b.max(*q)));
            out.push(RefPatch { key: key.clone(), centroid, u_dir, v_dir, u_range, v_range });
        }
    }
    Ok(out)
}

/// Re-UV the connected patches of `v` (positions `src_pos` in the visual's vertex order,
/// prefab frame): the patches matched to one reference patch form one logo and get
/// the reference's uv band along its u and v directions over their joint extent (the
/// finish line is one strip across the trough even where its mesh is in pieces).
/// Returns (patches re-UVed, patches that fell back to the long-axis rule).
pub fn retarget(v: &mut CPlugVisualIndexedTriangles, src_pos: &[[f32; 3]], refs: &[RefPatch], key: &str) -> (usize, usize) {
    let idx = v.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
    let n = src_pos.len();
    let comps = components(n, &idx);
    let Some(main) = v.main.as_mut() else { return (0, 0) };
    let Some(super::Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { return (0, 0) };
    let Some(uv) = s.decls.iter().zip(s.elems.iter_mut()).find(|(d, _)| d.name() == N_TEXCOORD0).and_then(|(_, e)| if let Elem::Float2(u) = e { Some(u) } else { None }) else { return (0, 0) };
    if uv.len() != n {
        return (0, 0);
    }
    let same: Vec<&RefPatch> = refs.iter().filter(|r| r.key == key).collect();
    // every patch → its reference (the nearest of the key, within 12 m; a key with one
    // reference takes every patch), or the fallback group
    let mut groups: std::collections::BTreeMap<Option<usize>, Vec<usize>> = Default::default();
    for comp in comps {
        let pts: Vec<[f32; 2]> = comp.iter().map(|&i| [src_pos[i][0], src_pos[i][2]]).collect();
        let cn = pts.len() as f32;
        let c = [pts.iter().map(|p| p[0]).sum::<f32>() / cn, pts.iter().map(|p| p[1]).sum::<f32>() / cn];
        let best = same.iter().enumerate().map(|(k, r)| (((r.centroid[0] - c[0]).powi(2) + (r.centroid[1] - c[1]).powi(2)).sqrt(), k)).filter(|(d, _)| *d < 12.0 || same.len() == 1).min_by(|a, b| a.0.partial_cmp(&b.0).unwrap()).map(|(_, k)| k);
        groups.entry(best).or_default().extend(comp);
    }
    let (mut done, mut fallback) = (0usize, 0usize);
    for (g, verts) in groups {
        let pts: Vec<[f32; 2]> = verts.iter().map(|&i| [src_pos[i][0], src_pos[i][2]]).collect();
        let (u_dir, v_dir, ur, vr) = match g {
            Some(k) => {
                let r = same[k];
                (r.u_dir, r.v_dir, r.u_range, r.v_range)
            }
            None => {
                fallback += 1;
                let (xl, xh) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
                let (zl, zh) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
                if zh - zl >= xh - xl { ([0.0, 1.0], [1.0, 0.0], (0.0, 1.0), (0.0, 1.0)) } else { ([1.0, 0.0], [0.0, 1.0], (0.0, 1.0), (0.0, 1.0)) }
            }
        };
        let su: Vec<f32> = pts.iter().map(|p| p[0] * u_dir[0] + p[1] * u_dir[1]).collect();
        let sv: Vec<f32> = pts.iter().map(|p| p[0] * v_dir[0] + p[1] * v_dir[1]).collect();
        let (ul, uh) = su.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(*q), b.max(*q)));
        let (vl, vh) = sv.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(*q), b.max(*q)));
        for (k, &i) in verts.iter().enumerate() {
            let tu = if uh - ul > 1e-4 { (su[k] - ul) / (uh - ul) } else { 0.5 };
            let tv = if vh - vl > 1e-4 { (sv[k] - vl) / (vh - vl) } else { 0.5 };
            uv[i] = [ur.0 + tu * (ur.1 - ur.0), vr.0 + tv * (vr.1 - vr.0)];
        }
        done += 1;
    }
    (done, fallback)
}

/// The stream without the named declarations (offsets and stride recomputed): the
/// plain decal materials of the tech start/finish carry position, normal, colour,
/// TexCoord0 and tangents only — the ice decals' TexCoord1 (lightmap chart) and
/// TexCoord2 (the layered shader's logo uv) under a plain decal material made the
/// whole BigA footprint a lighter square on the wood pad (Everios96 2026-10-29
/// 17:11 "the shadow calculation is weird"). Returns how many were dropped.
pub fn strip_decls(v: &mut CPlugVisualIndexedTriangles, names: &[u32]) -> usize {
    use super::vstream::Decl;
    let Some(main) = v.main.as_mut() else { return 0 };
    let Some(super::Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { return 0 };
    let compress = s.compress_local3d.unwrap_or(false);
    let keep: Vec<(Decl, Elem)> = s.decls.iter().zip(s.elems.iter()).filter(|(d, _)| !names.contains(&d.name())).map(|(d, e)| (d.clone(), e.clone())).collect();
    let dropped = s.decls.len() - keep.len();
    if dropped == 0 {
        return 0;
    }
    let stride: u32 = keep.iter().map(|(d, _)| super::vstream::type_size(d.stored_type(compress)).unwrap_or(4) as u32).sum();
    let mut offset = 0u32;
    let mut decls = Vec::with_capacity(keep.len());
    let mut elems = Vec::with_capacity(keep.len());
    for (d, e) in keep {
        let size = super::vstream::type_size(d.stored_type(compress)).unwrap_or(4) as u32;
        decls.push(Decl::with_stride(d.name(), d.ty(), d.space(), offset, stride / 4));
        offset += size;
        elems.push(e);
    }
    s.decls = decls;
    s.elems = elems;
    dropped
}
