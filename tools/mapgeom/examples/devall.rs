//! Dump all deviating corners to TSV. Usage: devall HIS.ITEM MINE.ITEM > out.tsv
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> BTreeMap<[(i32,i32,i32);3], Vec<([f32;3],String)>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[(i32,i32,i32);3], Vec<([f32;3],String)>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
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
                        let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                        let mut k = [mk(&ps[0]), mk(&ps[1]), mk(&ps[2])];
                        k.sort();
                        for p in ps { out.entry(k).or_default().push((p, stem.clone())); }
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    println!("mat\thx\thy\thz\tmx\tmy\tmz\tdx\tdy\tdz");
    for (k, rps) in &r {
        if let Some(mps) = m.get(k) {
            let mut used = vec![false; mps.len()];
            for (rp, _) in rps {
                let mut best = (1e9f32, 0usize);
                for (j, (mp, _)) in mps.iter().enumerate() {
                    if used[j] { continue; }
                    let d = ((rp[0]-mp[0]).powi(2)+(rp[1]-mp[1]).powi(2)+(rp[2]-mp[2]).powi(2)).sqrt();
                    if d < best.0 { best = (d, j); }
                }
                used[best.1] = true;
                if best.0 > 1e-6 {
                    let (mp, stem) = &mps[best.1];
                    println!("{}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:+.2e}\t{:+.2e}\t{:+.2e}",
                        stem, rp[0],rp[1],rp[2], mp[0],mp[1],mp[2], rp[0]-mp[0], rp[1]-mp[1], rp[2]-mp[2]);
                }
            }
        }
    }
}
