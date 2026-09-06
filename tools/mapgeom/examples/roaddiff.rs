//! Full element-wise diff of matched visual corners. Usage:
//! roaddiff REF MINE dx dy dz SUBSTR
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

struct Corner {
    pos: [f32; 3],
    f2: BTreeMap<u32, [f32; 2]>,
    w: BTreeMap<u32, u32>,
}

fn load(path: &str, substr: &str) -> Vec<(Corner, String)> {
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
                        Elem::Float2(u) => { f2.insert(d.name(), u.clone()); }
                        Elem::Word(x) => { w.insert(d.name(), x.clone()); }
                        _ => {}
                    }
                }
                for (i, p) in pos.iter().enumerate() {
                    let mut c = Corner { pos: *p, f2: BTreeMap::new(), w: BTreeMap::new() };
                    for (k, v) in &f2 {
                        c.f2.insert(*k, v[i]);
                    }
                    for (k, v) in &w {
                        c.w.insert(*k, v[i]);
                    }
                    out.push((c, mat.clone()));
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
    // index mine by mm-key (translated)
    let mut mmap: BTreeMap<(i32, i32, i32), Vec<usize>> = BTreeMap::new();
    for (i, (c, _)) in m.iter().enumerate() {
        mmap.entry(key(&[c.pos[0] + d[0], c.pos[1] + d[1], c.pos[2] + d[2]])).or_default().push(i);
    }
    let (mut n, mut miss) = (0, 0);
    let mut maxd: BTreeMap<String, f32> = BTreeMap::new();
    let mut worstd: BTreeMap<String, ([f32; 3], [f32; 3])> = BTreeMap::new();
    for (c, _) in &r {
        match mmap.get(&key(&c.pos)) {
            None => miss += 1,
            Some(v) => {
                let (mc, _) = &m[v[0]];
                n += 1;
                let mut chk = |maxd: &mut BTreeMap<String, f32>, worstd: &mut BTreeMap<String, ([f32; 3], [f32; 3])>, name: &str, a: [f32; 3], b: [f32; 3]| {
                    let dd = ((a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs()));
                    if dd > *maxd.get(name).unwrap_or(&0.0) {
                        maxd.insert(name.to_string(), dd);
                        worstd.insert(name.to_string(), (a, b));
                    }
                };
                for (k, f) in &c.f2 {
                    if let Some(g) = mc.f2.get(k) {
                        chk(&mut maxd, &mut worstd, &format!("uv{k}"), [f[0], f[1], 0.0], [g[0], g[1], 0.0]);
                    }
                }
                for (k, x) in &c.w {
                    if let Some(y) = mc.w.get(k) {
                        let (a3, b3) = (dec(*x), dec(*y));
                        chk(&mut maxd, &mut worstd, &format!("w{k}"), a3, b3);
                        if x != y {
                            *maxd.entry(format!("w{k}_bitdiff")).or_insert(0.0) += 1.0;
                        }
                    }
                }
            }
        }
    }
    println!("ref corners={} matched={n} missed={miss}", r.len());
    for (k, v) in &maxd {
        println!("  max|d| {k} = {v:.5} worst ref={:?} mine={:?}", worstd.get(k).map(|(a, _)| a).unwrap_or(&[0.0; 3]), worstd.get(k).map(|(_, b)| b).unwrap_or(&[0.0; 3]));
    }
}
