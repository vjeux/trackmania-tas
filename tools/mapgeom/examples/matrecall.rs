use mapgeom::static_item::vstream::Elem;
use std::collections::{BTreeMap, BTreeSet};
fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn load(path: &str, d: [f32; 3]) -> BTreeMap<String, BTreeSet<[(i32,i32,i32); 3]>> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out: BTreeMap<String, BTreeSet<[(i32,i32,i32); 3]>> = BTreeMap::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or("?".into());
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos: Vec<[f32; 3]> = Vec::new();
                for (dd, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if dd.name() == 0 { pos = p.clone(); break; }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let mut k = [key(&[pos[t[0] as usize][0]+d[0], pos[t[0] as usize][1]+d[1], pos[t[0] as usize][2]+d[2]]), key(&[pos[t[1] as usize][0]+d[0], pos[t[1] as usize][1]+d[1], pos[t[1] as usize][2]+d[2]]), key(&[pos[t[2] as usize][0]+d[0], pos[t[2] as usize][1]+d[1], pos[t[2] as usize][2]+d[2]])];
                    k.sort();
                    out.entry(mat.clone()).or_default().insert(k);
                }
            }
        }
    }
    out
}
fn stem(s: &str) -> String {
    // normalize version-skewed links: Turbo/Sign ~ SpecialSignTurbo
    let s = s.replace("SpecialSignTurbo", "Sign").replace("SpecialSignOff", "SignOff").replace("SpecialFXTurbo", "SpecialFX").replace("DecalSpecialTurbo", "Decal").replace("Modifier\\Turbo\\", "");
    s
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = load(&a[1], [0.0, 0.0, 0.0]);
    let m = load(&a[2], [a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()]);
    let mut rm: BTreeMap<String, BTreeSet<[(i32,i32,i32); 3]>> = BTreeMap::new();
    for (k, v) in &r { rm.entry(stem(k)).or_default().extend(v.iter().cloned()); }
    let mut mm: BTreeMap<String, BTreeSet<[(i32,i32,i32); 3]>> = BTreeMap::new();
    for (k, v) in &m { mm.entry(stem(k)).or_default().extend(v.iter().cloned()); }
    for (k, rv) in &rm {
        match mm.get(k) {
            None => println!("{k}: ref={} NO MATCHING BAKED MATERIAL", rv.len()),
            Some(mv) => {
                let inter = rv.intersection(mv).count();
                println!("{k}: ref={} bake={} recall={:.2}", rv.len(), mv.len(), inter as f64 / rv.len().max(1) as f64);
            }
        }
    }
}
