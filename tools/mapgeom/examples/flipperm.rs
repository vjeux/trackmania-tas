//! Permutation distances for flipped tris. Usage: flipperm HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[2]]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<Vec<[f32; 3]>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgym_parse(&data);
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
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        out.push(vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, t) in m.iter().enumerate() {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let mut nflip = 0;
    for t in &r {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            let u = &m[v[0]];
            let fnr = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let lr = (fnr[0]*fnr[0]+fnr[1]*fnr[1]+fnr[2]*fnr[2]).sqrt();
            if lr < 1e-15 { continue; }
            // all 6 perms: max corner dist + parity
            let mut best_even = f32::MAX;
            let mut best_odd = f32::MAX;
            for (pi, cand) in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]].iter().enumerate() {
                let mut mx = 0.0f32;
                for cc in 0..3 {
                    let dd = ((u[cand[cc]][0]-t[cc][0]).powi(2)+(u[cand[cc]][1]-t[cc][1]).powi(2)+(u[cand[cc]][2]-t[cc][2]).powi(2)).sqrt();
                    mx = mx.max(dd);
                }
                let even = pi == 0 || pi == 3 || pi == 4; // identity, and 3-cycles are even
                if even { best_even = best_even.min(mx); } else { best_odd = best_odd.min(mx); }
            }
            // flip test with identity
            let fnm = cross(sub(u[1], u[0]), sub(u[2], u[0]));
            let lm = (fnm[0]*fnm[0]+fnm[1]*fnm[1]+fnm[2]*fnm[2]).sqrt();
            if lm < 1e-15 { continue; }
            let dot = (fnr[0]*fnm[0]+fnr[1]*fnm[1]+fnr[2]*fnm[2])/(lr*lm);
            if dot < 0.0 && nflip < 6 {
                nflip += 1;
                println!("flip best_even_perm_dist={:.1}um best_odd_perm_dist={:.1}um",
                    best_even*1e6, best_odd*1e6);
            }
        }
    }
    println!("total flips checked: {nflip}");
}
fn mapgym_parse(data: &[u8]) -> mapgeom::static_item::file::StaticItemFile {
    mapgeom::static_item::file::parse_file(data).unwrap()
}
