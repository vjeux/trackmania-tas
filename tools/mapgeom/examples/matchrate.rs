//! Per-material triangle match rate + residual stats. Usage: matchrate REF.HIS MINE.BAKED dx dy dz
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn load(path: &str) -> Vec<(Vec<[f32; 3]>, String)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
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
                    out.push((vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], mat.clone()));
                }
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], usize> = BTreeMap::new();
    for (t, _) in &m {
        let mut k = [key(&[t[0][0]+d[0], t[0][1]+d[1], t[0][2]+d[2]]), key(&[t[1][0]+d[0], t[1][1]+d[1], t[1][2]+d[2]]), key(&[t[2][0]+d[0], t[2][1]+d[1], t[2][2]+d[2]])];
        k.sort();
        *mmap.entry(k).or_insert(0) += 1;
    }
    let mut stat: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (t, mat) in &r {
        let mut k = [key(&t[0]), key(&t[1]), key(&t[2])];
        k.sort();
        let e = stat.entry(mat.rsplit('\\').next().unwrap_or(mat).to_string()).or_insert((0, 0));
        e.0 += 1;
        if mmap.get(&k).unwrap_or(&0) > &0 { e.1 += 1; }
    }
    for (mat, (tot, hit)) in &stat {
        println!("{mat}: matched {hit}/{tot} ({:.0}%)", 100.0 * *hit as f32 / *tot as f32);
    }
}
