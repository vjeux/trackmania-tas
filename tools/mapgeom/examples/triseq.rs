//! His triangle emission order, annotated: for each tri in index-buffer order,
//! the source face (exact position lookup under f32 0.5 + TINY_POS_T), its
//! group, its uv1 chart id (union-find over vertex indices), and whether it
//! introduced new vertices. Usage: triseq HIS.ITEM SRC.ITEM STEM [N]
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;
fn pk(p: [f32; 3]) -> [u32; 3] { [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()] }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n_show: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(60);
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
        let mut pos = Vec::new();
        for (d, e) in st.decls.iter().zip(st.elems.iter()) { if let Elem::Float3(p) = e { if d.name() == 0 { pos = p.clone(); } } }
        let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        let tris: Vec<[u32; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0], t[1], t[2]]).collect();
        let n = pos.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize { let mut r = x; while p[r] != r { r = p[r]; } let mut c = x; while p[c] != r { let nx = p[c]; p[c] = r; c = nx; } r }
        for t in &tris { let r0 = find(&mut parent, t[0] as usize); let r1 = find(&mut parent, t[1] as usize); parent[r1] = r0; let r0 = find(&mut parent, t[0] as usize); let r2 = find(&mut parent, t[2] as usize); parent[r2] = r0; }
        let mut chart_ids: BTreeMap<usize, usize> = BTreeMap::new();
        let mut seen_v = vec![false; n];
        let mut maxv_prev = 0u32;
        println!("{}: tris={} verts={}", stem, tris.len(), n);
        let mut face_order_runs = 0usize;
        let mut prev_face: Option<usize> = None;
        let mut chart_changes = 0usize;
        let mut prev_chart = usize::MAX;
        for (ti, t) in tris.iter().enumerate() {
            let root = find(&mut parent, t[0] as usize);
            let nid = chart_ids.len();
            let ch = *chart_ids.entry(root).or_insert(nid);
            let fi = face_of([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
            let newv = t.iter().filter(|&&v| !seen_v[v as usize]).count();
            for &v in t { seen_v[v as usize] = true; }
            let maxv = t[0].max(t[1]).max(t[2]);
            let delayed = maxv < maxv_prev;
            maxv_prev = maxv_prev.max(maxv);
            if ch != prev_chart { chart_changes += 1; prev_chart = ch; }
            if let (Some(pf), Some(f)) = (prev_face, fi) { if f < pf { face_order_runs += 1; } }
            if fi.is_some() { prev_face = fi; }
            if ti < n_show {
                println!("  t{:>4} idx=({:>4},{:>4},{:>4}) new={} {} chart={:>3} face={} grp={} verts={}", ti, t[0], t[1], t[2], newv, if delayed { "DELAYED" } else { "       " }, ch,
                    fi.map(|f| format!("f{}", f)).unwrap_or("?".into()), fi.map(|f| cr.faces[f].group.to_string()).unwrap_or("?".into()), fi.map(|f| cr.faces[f].verts.len().to_string()).unwrap_or("?".into()));
            }
        }
        println!("  chart changes along the sequence: {} (charts={}) ; face-index descents: {}", chart_changes, chart_ids.len(), face_order_runs);
        return;
    }
}
