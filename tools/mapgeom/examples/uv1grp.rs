//! His uv1 charts vs the source crystal's groups: chart id per tri (union-find
//! over vertex indices), source face/group per tri (exact position lookup under
//! f32 scale 0.5 + TINY_POS_T). Reports chart group-purity and, for edge pairs
//! split at dihedral < 5deg, whether the two sides are different groups/faces.
//! Usage: uv1grp HIS.ITEM SRC.ITEM STEM
use mapgeom::static_item::vstream::Elem;
use std::collections::{BTreeMap, BTreeSet};
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn norm(v: [f32; 3]) -> [f32; 3] { let l = dot(v, v).sqrt().max(1e-30); [v[0] / l, v[1] / l, v[2] / l] }
fn pk(p: [f32; 3]) -> [u32; 3] { [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()] }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // source
    let sdata = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&sdata);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let t: [f32; 3] = std::env::var("TINY_POS_T").ok().and_then(|s| { let w: Vec<&str> = s.split(',').collect(); if w.len() != 3 { return None; } let mut t = [0f32; 3]; for (i, x) in w.iter().enumerate() { t[i] = f32::from_bits(u32::from_str_radix(x.trim(), 16).ok()?); } Some(t) }).unwrap_or([0.0; 3]);
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    let (cr, _, _) = layers.iter().find(|(_, vis, _)| *vis).unwrap();
    let mut pos2v: BTreeMap<[u32; 3], u32> = BTreeMap::new();
    for (i, p) in cr.positions.iter().enumerate() { pos2v.insert(pk([p[0] * 0.5 + t[0], p[1] * 0.5 + t[1], p[2] * 0.5 + t[2]]), i as u32); }
    let mut v2f: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (fi, f) in cr.faces.iter().enumerate() { for v in &f.verts { v2f.entry(*v).or_default().push(fi); } }
    let face_of = |p: [[f32; 3]; 3]| -> Option<usize> {
        let vs: Vec<u32> = p.iter().map(|q| pos2v.get(&pk(*q)).copied()).collect::<Option<Vec<u32>>>()?;
        let fs = v2f.get(&vs[0])?;
        fs.iter().copied().find(|fi| vs[1..].iter().all(|v| v2f.get(v).map(|l| l.contains(fi)).unwrap_or(false)))
    };
    // his
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for gg in &s2.shaded_geoms {
        let vi = gg.visual_index.max(0) as usize;
        let mi = gg.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        let Some(vref) = s2.visuals.get(vi) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() else { continue };
        let st = vis.stream().unwrap();
        let (mut pos, mut uv1) = (Vec::new(), Vec::new());
        for (d, e) in st.decls.iter().zip(st.elems.iter()) { match e { Elem::Float3(p) if d.name() == 0 => pos = p.clone(), Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(), _ => {} } }
        if uv1.is_empty() { println!("{}: no uv1", stem); return; }
        let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        let tris: Vec<[u32; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0], t[1], t[2]]).collect();
        let n = pos.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize { let mut r = x; while p[r] != r { r = p[r]; } let mut c = x; while p[c] != r { let nx = p[c]; p[c] = r; c = nx; } r }
        for t in &tris { let r0 = find(&mut parent, t[0] as usize); let r1 = find(&mut parent, t[1] as usize); parent[r1] = r0; let r0 = find(&mut parent, t[0] as usize); let r2 = find(&mut parent, t[2] as usize); parent[r2] = r0; }
        let chart: Vec<usize> = tris.iter().map(|t| find(&mut parent, t[0] as usize)).collect();
        let sface: Vec<Option<usize>> = tris.iter().map(|t| face_of([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]])).collect();
        let unmapped = sface.iter().filter(|f| f.is_none()).count();
        let mut chart_groups: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
        let mut chart_faces: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for ti in 0..tris.len() { if let Some(fi) = sface[ti] { chart_groups.entry(chart[ti]).or_default().insert(cr.faces[fi].group); chart_faces.entry(chart[ti]).or_default().insert(fi); } }
        let multi = chart_groups.values().filter(|s| s.len() > 1).count();
        println!("{}: tris={} charts={} unmapped-tris={} charts spanning >1 source group: {}", stem, tris.len(), chart_groups.len(), unmapped, multi);
        // groups -> number of charts
        let mut grp_charts: BTreeMap<u32, BTreeSet<usize>> = BTreeMap::new();
        for (ch, gs) in &chart_groups { for g in gs { grp_charts.entry(*g).or_default().insert(*ch); } }
        let mut hist: BTreeMap<usize, usize> = BTreeMap::new();
        for (_, chs) in &grp_charts { *hist.entry(chs.len()).or_insert(0) += 1; }
        println!("   source groups={} ; charts-per-group histogram: {:?}", grp_charts.len(), hist);
        // faces per chart vs group size
        let fnorm: Vec<[f32; 3]> = tris.iter().map(|t| norm(cross(sub(pos[t[1] as usize], pos[t[0] as usize]), sub(pos[t[2] as usize], pos[t[0] as usize])))).collect();
        let mut edges: BTreeMap<([u32; 3], [u32; 3]), Vec<usize>> = BTreeMap::new();
        for (ti, t) in tris.iter().enumerate() { for k in 0..3 { let (p, q) = (pk(pos[t[k] as usize]), pk(pos[t[(k + 1) % 3] as usize])); let key = if p < q { (p, q) } else { (q, p) }; edges.entry(key).or_default().push(ti); } }
        let (mut flat_split_same_grp, mut flat_split_diff_grp, mut flat_split_unk) = (0, 0, 0);
        let mut ex = 0;
        for (_, ts) in &edges { for i in 0..ts.len() { for j in i + 1..ts.len() {
            let ang = dot(fnorm[ts[i]], fnorm[ts[j]]).clamp(-1.0, 1.0).acos().to_degrees();
            if ang < 5.0 && chart[ts[i]] != chart[ts[j]] {
                match (sface[ts[i]], sface[ts[j]]) {
                    (Some(fa), Some(fb)) => {
                        if cr.faces[fa].group == cr.faces[fb].group { flat_split_same_grp += 1; if ex < 4 { ex += 1; println!("   flat split SAME group g{}: faces f{}(n{}) f{}(n{}) chart sizes {} {}", cr.faces[fa].group, fa, cr.faces[fa].verts.len(), fb, cr.faces[fb].verts.len(), chart_faces[&chart[ts[i]]].len(), chart_faces[&chart[ts[j]]].len()); } }
                        else { flat_split_diff_grp += 1; }
                    }
                    _ => flat_split_unk += 1,
                }
            }
        } } }
        println!("   flat (<5deg) splits: same-group={} different-group={} unmapped={}", flat_split_same_grp, flat_split_diff_grp, flat_split_unk);
        return;
    }
}
