//! How his lightmap charts are segmented: for every pair of tris sharing a
//! 3D edge (by exact positions) within one visual, the dihedral angle and
//! whether they sit in the same uv1 chart. Histogram of both populations, and
//! the angle between each tri's normal and its chart's mean normal.
//! Usage: uv1seg FILE STEM
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn norm(v: [f32; 3]) -> [f32; 3] { let l = dot(v, v).sqrt().max(1e-30); [v[0] / l, v[1] / l, v[2] / l] }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[2] { continue; }
        let Some(vref) = s2.visuals.get(vi) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() else { continue };
        let st = vis.stream().unwrap();
        let (mut pos, mut uv1) = (Vec::new(), Vec::new());
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                _ => {}
            }
        }
        if uv1.is_empty() { println!("{}: no uv1", stem); return; }
        let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        let tris: Vec<[u32; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0], t[1], t[2]]).collect();
        // chart id per tri via union-find on vertex indices
        let n = pos.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize { let mut r = x; while p[r] != r { r = p[r]; } let mut c = x; while p[c] != r { let nx = p[c]; p[c] = r; c = nx; } r }
        for t in &tris { let r0 = find(&mut parent, t[0] as usize); let r1 = find(&mut parent, t[1] as usize); parent[r1] = r0; let r0 = find(&mut parent, t[0] as usize); let r2 = find(&mut parent, t[2] as usize); parent[r2] = r0; }
        let chart: Vec<usize> = tris.iter().map(|t| find(&mut parent, t[0] as usize)).collect();
        let fnorm: Vec<[f32; 3]> = tris.iter().map(|t| norm(cross(sub(pos[t[1] as usize], pos[t[0] as usize]), sub(pos[t[2] as usize], pos[t[0] as usize])))).collect();
        let pk = |p: [f32; 3]| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
        // edges by position pair
        let mut edges: BTreeMap<([u32; 3], [u32; 3]), Vec<usize>> = BTreeMap::new();
        for (ti, t) in tris.iter().enumerate() {
            for k in 0..3 {
                let (p, q) = (pk(pos[t[k] as usize]), pk(pos[t[(k + 1) % 3] as usize]));
                let key = if p < q { (p, q) } else { (q, p) };
                edges.entry(key).or_default().push(ti);
            }
        }
        let mut same: BTreeMap<i32, usize> = BTreeMap::new();
        let mut split: BTreeMap<i32, usize> = BTreeMap::new();
        let mut same_max = 0.0f32;
        let mut split_min = 180.0f32;
        for (_, ts) in &edges {
            for i in 0..ts.len() {
                for j in i + 1..ts.len() {
                    let ang = dot(fnorm[ts[i]], fnorm[ts[j]]).clamp(-1.0, 1.0).acos().to_degrees();
                    let bin = (ang / 5.0).floor() as i32 * 5;
                    if chart[ts[i]] == chart[ts[j]] { *same.entry(bin).or_insert(0) += 1; same_max = same_max.max(ang); }
                    else { *split.entry(bin).or_insert(0) += 1; split_min = split_min.min(ang); }
                }
            }
        }
        println!("{}: tris={} charts={} | edge pairs same-chart by dihedral (5deg bins): {:?} max={:.1}", stem, tris.len(), chart.iter().collect::<std::collections::BTreeSet<_>>().len(), same, same_max);
        println!("   split by dihedral: {:?} min={:.1}", split, split_min);
        // angle to chart mean normal
        let mut cm: BTreeMap<usize, ([f32; 3], usize)> = BTreeMap::new();
        for (ti, t) in tris.iter().enumerate() {
            let c = cross(sub(pos[t[1] as usize], pos[t[0] as usize]), sub(pos[t[2] as usize], pos[t[0] as usize]));
            let e = cm.entry(chart[ti]).or_insert(([0.0; 3], 0));
            for k in 0..3 { e.0[k] += c[k]; }
            e.1 += 1;
        }
        let mut devh: BTreeMap<i32, usize> = BTreeMap::new();
        let mut devmax = 0.0f32;
        for ti in 0..tris.len() {
            let m = norm(cm[&chart[ti]].0);
            let ang = dot(fnorm[ti], m).clamp(-1.0, 1.0).acos().to_degrees();
            *devh.entry((ang / 10.0).floor() as i32 * 10).or_insert(0) += 1;
            devmax = devmax.max(ang);
        }
        println!("   tri-vs-chart-mean-normal (10deg bins): {:?} max={:.1}", devh, devmax);
        // chart size histogram
        let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
        for (_, (_, n)) in &cm { *sizes.entry(*n).or_insert(0) += 1; }
        println!("   chart tri-count histogram: {:?}", sizes);
        return;
    }
}
