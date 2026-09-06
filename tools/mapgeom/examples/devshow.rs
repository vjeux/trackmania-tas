//! Show deviating corners: his pos vs my pos per matched tri. Usage: devshow HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> BTreeMap<[(i32,i32,i32);3], Vec<[f32;3]>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[(i32,i32,i32);3], Vec<[f32;3]>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if !mat.contains(&a[3]) { continue; }
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
                        out.entry(k).or_default().extend(ps);
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut shown = 0;
    for (k, rps) in &r {
        if let Some(mps) = m.get(k) {
            // match corners greedily by nearest
            let mut used = vec![false; mps.len()];
            for rp in rps {
                let mut best = (1e9f32, 0usize);
                for (j, mp) in mps.iter().enumerate() {
                    if used[j] { continue; }
                    let d = ((rp[0]-mp[0]).powi(2)+(rp[1]-mp[1]).powi(2)+(rp[2]-mp[2]).powi(2)).sqrt();
                    if d < best.0 { best = (d, j); }
                }
                used[best.1] = true;
                if best.0 > 1e-6 {
                    let mp = mps[best.1];
                    println!("his=({:.6},{:.6},{:.6}) mine=({:.6},{:.6},{:.6}) d={:.2e} dh=({:+.1e},{:+.1e},{:+.1e})",
                        rp[0],rp[1],rp[2], mp[0],mp[1],mp[2], best.0, rp[0]-mp[0], rp[1]-mp[1], rp[2]-mp[2]);
                    shown += 1;
                    if shown > 24 { return; }
                }
            }
        }
    }
}
