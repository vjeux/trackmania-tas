//! Tri-pair element diff (correct pairing in dense meshes). Usage:
//! tridiff REF MINE dx dy dz SUBSTR
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}

struct Tri {
    v: [[f32; 3]; 3],
    f2: Vec<(u32, [[f32; 2]; 3])>,
    w: Vec<(u32, [u32; 3])>,
}

fn load(path: &str, substr: &str) -> Vec<(Tri, String)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(substr) {
            continue;
        }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos: Vec<[f32; 3]> = Vec::new();
                let mut f2: BTreeMap<u32, Vec<[f32; 2]>> = BTreeMap::new();
                let mut w: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) => { f2.insert(d.name() as u32, u.clone()); }
                        Elem::Word(x) => { w.insert(d.name() as u32, x.clone()); }
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    let mut tri = Tri { v: [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], f2: Vec::new(), w: Vec::new() };
                    for (k, v) in &f2 {
                        tri.f2.push((*k, [v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]]));
                    }
                    for (k, v) in &w {
                        tri.w.push((*k, [v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]]));
                    }
                    out.push((tri, mat.clone()));
                }
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let r = load(&a[1], &a[6]);
    let m = load(&a[2], &a[6]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [key(&[t.v[0][0]+d[0], t.v[0][1]+d[1], t.v[0][2]+d[2]]), key(&[t.v[1][0]+d[0], t.v[1][1]+d[1], t.v[1][2]+d[2]]), key(&[t.v[2][0]+d[0], t.v[2][1]+d[1], t.v[2][2]+d[2]])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let (mut ntri, mut miss, mut ncorn) = (0, 0, 0);
    let mut maxd: BTreeMap<String, f32> = BTreeMap::new();
    let mut bitdiff: BTreeMap<String, usize> = BTreeMap::new();
    for (t, _) in &r {
        let mut k = [key(&t.v[0]), key(&t.v[1]), key(&t.v[2])];
        k.sort();
        match mmap.get(&k) {
            None => miss += 1,
            Some(v) => {
                ntri += 1;
                let (u, _) = &m[v[0]];
                for ck in 0..3 {
                    // mine corner at ref corner ck
                    let mut mk = None;
                    for q in 0..3 {
                        let mp = [u.v[q][0]+d[0], u.v[q][1]+d[1], u.v[q][2]+d[2]];
                        if (mp[0]-t.v[ck][0]).abs() < 0.002 && (mp[1]-t.v[ck][1]).abs() < 0.002 && (mp[2]-t.v[ck][2]).abs() < 0.002 {
                            mk = Some(q);
                            break;
                        }
                    }
                    if let Some(q) = mk {
                        ncorn += 1;
                        for (dk, rf) in &t.f2 {
                            if let Some(mf) = u.f2.iter().find(|(k, _)| k == dk).map(|(_, v)| v) {
                                let dd = (rf[ck][0]-mf[q][0]).abs().max((rf[ck][1]-mf[q][1]).abs());
                                let e = maxd.entry(format!("uv{dk}")).or_insert(0.0);
                                if dd > *e {
                                    *e = dd;
                                }
                            }
                        }
                        for (dk, rw) in &t.w {
                            if let Some(mw) = u.w.iter().find(|(k, _)| k == dk).map(|(_, v)| v) {
                                let (a3, b3) = (dec(rw[ck]), dec(mw[q]));
                                let dd = (a3[0]-b3[0]).abs().max((a3[1]-b3[1]).abs()).max((a3[2]-b3[2]).abs());
                                let e = maxd.entry(format!("w{dk}")).or_insert(0.0);
                                if dd > *e {
                                    *e = dd;
                                }
                                if rw[ck] != mw[q] {
                                    *bitdiff.entry(format!("w{dk}")).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("ref tris={} matched={ntri} missed={miss} corners={ncorn}", r.len());
    for (k, v) in &maxd {
        println!("  max|d| {k} = {v:.5} bitdiff={}", bitdiff.get(k).unwrap_or(&0));
    }
}
