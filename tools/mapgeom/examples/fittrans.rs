//! Fit translation: median residual over mmkey-matched corners. Usage: fittrans REF.HIS MINE.BAKED dx dy dz
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn load(path: &str) -> Vec<Vec<[f32; 3]>> {
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
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    out.push(vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                }
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    for _ in 0..6 {
        let r = load(&a[1]);
        let m = load(&a[2]);
        let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
        for (i, t) in m.iter().enumerate() {
            let mut k = [key(&[t[0][0]+d[0], t[0][1]+d[1], t[0][2]+d[2]]), key(&[t[1][0]+d[0], t[1][1]+d[1], t[1][2]+d[2]]), key(&[t[2][0]+d[0], t[2][1]+d[1], t[2][2]+d[2]])];
            k.sort();
            mmap.entry(k).or_default().push(i);
        }
        let (mut xs, mut ys, mut zs) = (Vec::new(), Vec::new(), Vec::new());
        let mut ntri = 0;
        for t in &r {
            let mut k = [key(&t[0]), key(&t[1]), key(&t[2])];
            k.sort();
            if let Some(v) = mmap.get(&k) {
                ntri += 1;
                let u = &m[v[0]];
                for ck in 0..3 {
                    for mk in 0..3 {
                        let mp = [u[mk][0]+d[0], u[mk][1]+d[1], u[mk][2]+d[2]];
                        if (mp[0]-t[ck][0]).abs() < 0.002 && (mp[1]-t[ck][1]).abs() < 0.002 && (mp[2]-t[ck][2]).abs() < 0.002 {
                            // residual = his - mine_translated; correction = -median
                            xs.push(t[ck][0]-mp[0]); ys.push(t[ck][1]-mp[1]); zs.push(t[ck][2]-mp[2]);
                            break;
                        }
                    }
                }
            }
        }
        xs.sort_by(|x, y| x.partial_cmp(y).unwrap());
        ys.sort_by(|x, y| x.partial_cmp(y).unwrap());
        zs.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let med = |v: &Vec<f32>| v[v.len()/2];
        println!("matched_tris={ntri} med_resid=({:.6},{:.6},{:.6})", med(&xs), med(&ys), med(&zs));
        d[0] += med(&xs); d[1] += med(&ys); d[2] += med(&zs);
    }
    println!("FINAL d=({:.6},{:.6},{:.6})", d[0], d[1], d[2]);
}
