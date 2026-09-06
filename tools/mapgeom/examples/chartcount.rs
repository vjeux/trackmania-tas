//! Count uv1 charts (flood fill over shared (pos,uv1)). Usage: chartcount FILE SUBSTR
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
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
        if !mat.contains(&a[2]) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv1) = (Vec::new(), Vec::new());
                let mut has_uv1 = false;
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                        _ => {}
                    }
                }
                if !has_uv1 { println!("{}: no uv1", a[2]); return; }
                if pos.len() != uv1.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let tris: Vec<[usize; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect();
                // union-find over tris sharing (pos,uv1) corners
                let mut parent: Vec<usize> = (0..tris.len()).collect();
                fn find(p: &mut Vec<usize>, x: usize) -> usize {
                    if p[x] != x { p[x] = find(p, p[x]); }
                    p[x]
                }
                let mut corner_map: BTreeMap<[u32; 5], Vec<usize>> = BTreeMap::new();
                for (ti, t) in tris.iter().enumerate() {
                    for k in 0..3 {
                        let key = [pos[t[k]][0].to_bits(), pos[t[k]][1].to_bits(), pos[t[k]][2].to_bits(), uv1[t[k]][0].to_bits(), uv1[t[k]][1].to_bits()];
                        corner_map.entry(key).or_default().push(ti);
                    }
                }
                for (_, tis) in &corner_map {
                    for w in tis.windows(2) {
                        let a2 = find(&mut parent, w[0]);
                        let b = find(&mut parent, w[1]);
                        if a2 != b { parent[a2] = b; }
                    }
                }
                let mut roots = BTreeSet::new();
                for i in 0..tris.len() {
                    roots.insert(find(&mut parent, i));
                }
                println!("{}: ntri={} uv1_charts={}", a[2], tris.len(), roots.len());
                return;
            }
        }
    }
}
