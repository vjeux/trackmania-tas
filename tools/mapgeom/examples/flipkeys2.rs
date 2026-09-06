//! Flip keys via best-perm (exact). Usage: flipkeys2 HIS.ITEM MINE.ITEM
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<Vec<[f32; 3]>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
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
                        out.push(vec![pos[tt[0] as usize], pos[tt[1] as usize], pos[tt[2] as usize]]);
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, tt) in m.iter().enumerate() {
        let mut k = [mk(&tt[0]), mk(&tt[1]), mk(&tt[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let mut rmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, tt) in r.iter().enumerate() {
        let mut k = [mk(&tt[0]), mk(&tt[1]), mk(&tt[2])];
        k.sort();
        rmap.entry(k).or_default().push(i);
    }
    let mut flipkeys: BTreeSet<[(i32, i32, i32); 3]> = BTreeSet::new();
    for (k, ri) in &rmap {
        if let Some(mi) = mmap.get(k) {
            let ht = &r[ri[0]];
            let mt = &m[mi[0]];
            let mut best = (7, f32::MAX);
            for (pi, cand) in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]].iter().enumerate() {
                let mut mx = 0.0f32;
                for cc in 0..3 {
                    let dd = ((mt[cand[cc]][0]-ht[cc][0]).powi(2)+(mt[cand[cc]][1]-ht[cc][1]).powi(2)+(mt[cand[cc]][2]-ht[cc][2]).powi(2)).sqrt();
                    mx = mx.max(dd);
                }
                if mx < best.1 {
                    best = (pi, mx);
                }
            }
            if best.1 < 0.002 {
                let even = best.0 == 0 || best.0 == 3 || best.0 == 4;
                if !even {
                    flipkeys.insert(*k);
                }
            }
        }
    }
    let s: Vec<String> = flipkeys.iter().map(|k| format!("{},{},{};{},{},{};{},{},{}", k[0].0, k[0].1, k[0].2, k[1].0, k[1].1, k[1].2, k[2].0, k[2].1, k[2].2)).collect();
    println!("{}", s.join("|"));
    eprintln!("flipkeys2: {} keys", flipkeys.len());
}
