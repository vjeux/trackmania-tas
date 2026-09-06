//! Key overlap his-vs-mine. Usage: keyoverlap HIS.ITEM MINE.ITEM
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut maps = Vec::new();
    for path in [&a[1], &a[2]] {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut m: BTreeMap<[(i32, i32, i32); 3], usize> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let mut pos = Vec::new();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 { pos = p.clone(); }
                        }
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for tt in idx.chunks(3) {
                        if tt.len() < 3 { continue; }
                        let mut k = [mk(&pos[tt[0] as usize]), mk(&pos[tt[1] as usize]), mk(&pos[tt[2] as usize])];
                        k.sort();
                        *m.entry(k).or_insert(0) += 1;
                    }
                }
            }
        }
        maps.push(m);
    }
    let rk: BTreeSet<_> = maps[0].keys().collect();
    let mk2: BTreeSet<_> = maps[1].keys().collect();
    let inter = rk.intersection(&mk2).count();
    println!("his_keys={} my_keys={} shared={} his_only={} my_only={}", rk.len(), mk2.len(), inter, rk.len()-inter, mk2.len()-inter);
}
