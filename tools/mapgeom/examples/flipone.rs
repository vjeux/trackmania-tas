//! Show one flipped tri his vs mine (ordered). Usage: flipone HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let flips: std::collections::BTreeSet<[(i32, i32, i32); 3]> = std::fs::read_to_string("/tmp/flip17.txt").unwrap().trim().split('|').map(|s| {
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
    let mut rmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, tt) in r.iter().enumerate() {
        let mut k = [mk(&tt[0]), mk(&tt[1]), mk(&tt[2])];
        k.sort();
        rmap.entry(k).or_default().push(i);
    }
    let k = flips.iter().next().unwrap();
    println!("key={k:?}");
    if let Some(ri) = rmap.get(k) {
        let t = &r[ri[0]];
        println!("his : [{:.5},{:.5},{:.5}] [{:.5},{:.5},{:.5}] [{:.5},{:.5},{:.5}]", t[0][0], t[0][1], t[0][2], t[1][0], t[1][1], t[1][2], t[2][0], t[2][1], t[2][2]);
    }
    if let Some(mi) = mmap.get(k) {
        for i in mi.iter().take(3) {
            let t = &m[*i];
            println!("mine: [{:.5},{:.5},{:.5}] [{:.5},{:.5},{:.5}] [{:.5},{:.5},{:.5}]", t[0][0], t[0][1], t[0][2], t[1][0], t[1][1], t[1][2], t[2][0], t[2][1], t[2][2]);
        }
    }
}
