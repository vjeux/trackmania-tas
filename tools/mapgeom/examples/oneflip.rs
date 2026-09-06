//! Inspect first flipkeys2 key. Usage: oneflip HIS.ITEM MINE.ITEM
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
    let fk: Vec<String> = std::fs::read_to_string("/tmp/flip17b.txt").unwrap().trim().split('|').map(|s| s.to_string()).collect();
    let v: Vec<&str> = fk[0].split(';').collect();
    let p = |i: usize| -> (i32, i32, i32) {
        let c: Vec<&str> = v[i].split(',').collect();
        (c[0].parse::<i32>().unwrap(), c[1].parse::<i32>().unwrap(), c[2].parse::<i32>().unwrap())
    };
    let mut k = [p(0), p(1), p(2)];
    k.sort();
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
    let mut rmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, tt) in r.iter().enumerate() {
        let mut kk = [mk(&tt[0]), mk(&tt[1]), mk(&tt[2])];
        kk.sort();
        rmap.entry(kk).or_default().push(i);
    }
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, tt) in m.iter().enumerate() {
        let mut kk = [mk(&tt[0]), mk(&tt[1]), mk(&tt[2])];
        kk.sort();
        mmap.entry(kk).or_default().push(i);
    }
    if let (Some(ri), Some(mi)) = (rmap.get(&k), mmap.get(&k)) {
        println!("key count his={} mine={}", ri.len(), mi.len());
        let ht = &r[ri[0]];
        let mt = &m[mi[0]];
        println!("his : [{:.6},{:.6},{:.6}] [{:.6},{:.6},{:.6}] [{:.6},{:.6},{:.6}]", ht[0][0], ht[0][1], ht[0][2], ht[1][0], ht[1][1], ht[1][2], ht[2][0], ht[2][1], ht[2][2]);
        println!("mine: [{:.6},{:.6},{:.6}] [{:.6},{:.6},{:.6}] [{:.6},{:.6},{:.6}]", mt[0][0], mt[0][1], mt[0][2], mt[1][0], mt[1][1], mt[1][2], mt[2][0], mt[2][1], mt[2][2]);
        let fh = cross(sub(ht[1], ht[0]), sub(ht[2], ht[0]));
        let fm = cross(sub(mt[1], mt[0]), sub(mt[2], mt[0]));
        let lh = (fh[0]*fh[0]+fh[1]*fh[1]+fh[2]*fh[2]).sqrt();
        let lm = (fm[0]*fm[0]+fm[1]*fm[1]+fm[2]*fm[2]).sqrt();
        println!("face dot={:.4}", (fh[0]*fm[0]+fh[1]*fm[1]+fh[2]*fm[2])/(lh*lm));
    } else {
        println!("key not found in one file");
    }
}
