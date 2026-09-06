use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;
fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}
fn load(path: &str) -> Vec<([[f32; 3]; 3], String)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").to_string()).unwrap_or("?".into());
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos: Vec<[f32; 3]> = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 { pos = p.clone(); break; }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    out.push(([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], mat.clone()));
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
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [key(&[t[0][0]+d[0], t[0][1]+d[1], t[0][2]+d[2]]), key(&[t[1][0]+d[0], t[1][1]+d[1], t[1][2]+d[2]]), key(&[t[2][0]+d[0], t[2][1]+d[1], t[2][2]+d[2]])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let mut stat: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (t, mat) in &r {
        let mut k = [key(&t[0]), key(&t[1]), key(&t[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            let (u, _) = &m[v[0]];
            let fr = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let fm = cross(sub(u[1], u[0]), sub(u[2], u[0]));
            let dot = fr[0]*fm[0] + fr[1]*fm[1] + fr[2]*fm[2];
            let short = mat.rsplit('\\').next().unwrap_or(mat).to_string();
            let e = stat.entry(short).or_default();
            e.0 += 1;
            if dot < 0.0 { e.1 += 1; }
        }
    }
    for (k, (n, f)) in &stat {
        println!("{k}: matched={n} flipped={f}");
    }
}
