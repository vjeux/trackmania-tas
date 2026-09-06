//! Emission-order hypothesis test: his faces come out chart by chart (charts
//! by descending seed face index), each chart traversed from its seed face.
//! Simulates traversal variants over the SOURCE face adjacency restricted to
//! his charts (from his uv1 partition) and scores the face sequence against
//! his. Usage: triorder2 HIS.ITEM SRC.ITEM STEM
use mapgeom::static_item::vstream::Elem;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
fn pk(p: [f32; 3]) -> [u32; 3] { [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()] }
fn main() {
    let a: Vec<String> = std::env::args().collect();
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
    // source face adjacency via shared (sorted) vertex-index edges, per edge slot
    let mut edge_faces: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for (fi, f) in cr.faces.iter().enumerate() { for k in 0..f.verts.len() { let (p, q) = (f.verts[k], f.verts[(k + 1) % f.verts.len()]); edge_faces.entry((p.min(q), p.max(q))).or_default().push(fi); } }
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
        // his face sequence (dedup consecutive) and chart per face
        let mut his_faces: Vec<usize> = Vec::new();
        let mut face_chart: BTreeMap<usize, usize> = BTreeMap::new();
        let mut unmapped = 0;
        for t in &tris {
            let ch = find(&mut parent, t[0] as usize);
            match face_of([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]) {
                Some(fi) => { if his_faces.last() != Some(&fi) { his_faces.push(fi); } face_chart.insert(fi, ch); }
                None => unmapped += 1,
            }
        }
        // charts -> member faces
        let mut chart_faces: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for (fi, ch) in &face_chart { chart_faces.entry(*ch).or_default().insert(*fi); }
        println!("{}: his tris={} unmapped={} faces-in-seq={} charts={}", stem, tris.len(), unmapped, his_faces.len(), chart_faces.len());
        // neighbours of a face within its chart, in edge order
        let nbrs = |fi: usize, ch: usize| -> Vec<usize> {
            let f = &cr.faces[fi];
            let mut out = Vec::new();
            for k in 0..f.verts.len() {
                let (p, q) = (f.verts[k], f.verts[(k + 1) % f.verts.len()]);
                if let Some(fs) = edge_faces.get(&(p.min(q), p.max(q))) {
                    for &g in fs { if g != fi && face_chart.get(&g) == Some(&ch) && !out.contains(&g) { out.push(g); } }
                }
            }
            out
        };
        // chart order: by descending max face index (seed = max face)
        let mut chart_order: Vec<(usize, usize)> = chart_faces.iter().map(|(ch, fs)| (*fs.iter().max().unwrap(), *ch)).collect();
        chart_order.sort_by(|a, b| b.0.cmp(&a.0));
        let variants: Vec<(&str, Box<dyn Fn(usize, usize) -> Vec<usize>>)> = vec![
            ("bfs-edge-order", Box::new(|seed, ch| { let mut out = vec![seed]; let mut seen: BTreeSet<usize> = [seed].into(); let mut q = VecDeque::from(vec![seed]); while let Some(f) = q.pop_front() { for g in nbrs(f, ch) { if seen.insert(g) { out.push(g); q.push_back(g); } } } out })),
            ("bfs-desc", Box::new(|seed, ch| { let mut out = vec![seed]; let mut seen: BTreeSet<usize> = [seed].into(); let mut q = VecDeque::from(vec![seed]); while let Some(f) = q.pop_front() { let mut ns = nbrs(f, ch); ns.sort_by(|a, b| b.cmp(a)); for g in ns { if seen.insert(g) { out.push(g); q.push_back(g); } } } out })),
            ("bfs-asc", Box::new(|seed, ch| { let mut out = vec![seed]; let mut seen: BTreeSet<usize> = [seed].into(); let mut q = VecDeque::from(vec![seed]); while let Some(f) = q.pop_front() { let mut ns = nbrs(f, ch); ns.sort(); for g in ns { if seen.insert(g) { out.push(g); q.push_back(g); } } } out })),
            ("dfs-edge-order", Box::new(|seed, ch| { let mut out = Vec::new(); let mut seen: BTreeSet<usize> = BTreeSet::new(); let mut stack = vec![seed]; while let Some(f) = stack.pop() { if !seen.insert(f) { continue; } out.push(f); let ns = nbrs(f, ch); for g in ns.into_iter().rev() { if !seen.contains(&g) { stack.push(g); } } } out })),
            ("dfs-desc", Box::new(|seed, ch| { let mut out = Vec::new(); let mut seen: BTreeSet<usize> = BTreeSet::new(); let mut stack = vec![seed]; while let Some(f) = stack.pop() { if !seen.insert(f) { continue; } out.push(f); let mut ns = nbrs(f, ch); ns.sort(); for g in ns { if !seen.contains(&g) { stack.push(g); } } } out })),
            ("desc-index", Box::new(|_seed, ch| { let mut v: Vec<usize> = chart_faces[&ch].iter().copied().collect(); v.sort_by(|a, b| b.cmp(a)); v })),
        ];
        for (name, walk) in &variants {
            let mut seq: Vec<usize> = Vec::new();
            for &(seed, ch) in &chart_order {
                let mut w = walk(seed, ch);
                // faces unreachable in-chart (non-edge-adjacent): append desc
                let mut rest: Vec<usize> = chart_faces[&ch].iter().copied().filter(|f| !w.contains(f)).collect();
                rest.sort_by(|a, b| b.cmp(a));
                w.extend(rest);
                seq.extend(w);
            }
            let m = his_faces.iter().zip(seq.iter()).filter(|(a, b)| a == b).count();
            let prefix = his_faces.iter().zip(seq.iter()).take_while(|(a, b)| a == b).count();
            println!("  {:<16} positional match {}/{} ({:.1}%) common prefix {}", name, m, his_faces.len(), 100.0 * m as f64 / his_faces.len() as f64, prefix);
        }
        return;
    }
}
