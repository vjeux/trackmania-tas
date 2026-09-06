//! Per-flip-key winding agreement. Usage: flipverify HIS.ITEM MINE.ITEM FLIPKEYS
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let flips: BTreeSet<[(i32, i32, i32); 3]> = std::fs::read_to_string(&a[3]).unwrap().trim().split('|').map(|s| {
        let v: Vec<&str> = s.split(';').collect();
        let p = |i: usize| {
            let c: Vec<&str> = v[i].split(',').collect();
            (c[0].parse().unwrap(), c[1].parse().unwrap(), c[2].parse().unwrap())
        };
        let mut k = [p(0), p(1), p(2)];
        k.sort();
        k
    }).collect();
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
    // his tri order per key (for winding compare, need ordered corners; use first his tri with key)
    let mut rmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, tt) in r.iter().enumerate() {
        let mut k = [mk(&tt[0]), mk(&tt[1]), mk(&tt[2])];
        k.sort();
        rmap.entry(k).or_default().push(i);
    }
    let (mut agree, mut disagree, mut missing) = (0, 0, 0);
    for k in &flips {
        match (rmap.get(k), mmap.get(k)) {
            (Some(ri), Some(mi)) => {
                let ht = &r[ri[0]];
                let mt = &m[mi[0]];
                // parity: find best perm, check even/odd (like flipperm but count all)
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
                if best.1 > 0.002 {
                    missing += 1;
                } else {
                    let even = best.0 == 0 || best.0 == 3 || best.0 == 4;
                    if even { agree += 1; } else { disagree += 1; }
                }
            }
            _ => missing += 1,
        }
    }
    println!("flipkeys={} agree={agree} disagree={disagree} missing={missing}", flips.len());
}
