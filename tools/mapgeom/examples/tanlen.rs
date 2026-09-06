use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or("?".into());
            if !mat.contains("Road") {
                continue;
            }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Word(w) = e {
                            if d.name() == 18 || d.name() == 20 {
                                let (mut n, mut bad, mut lmin, mut lmax) = (0, 0, 99.0f32, 0.0f32);
                                for v in w {
                                    let t = dec(*v);
                                    let l = (t[0]*t[0] + t[1]*t[1] + t[2]*t[2]).sqrt();
                                    n += 1;
                                    if !l.is_finite() || l < 0.5 || l > 1.5 {
                                        bad += 1;
                                    }
                                    lmin = lmin.min(l);
                                    lmax = lmax.max(l);
                                }
                                println!("{} road decl{}: n={n} bad={bad} lenrange=[{lmin:.2},{lmax:.2}]", path.rsplit('/').next().unwrap(), d.name());
                            }
                        }
                    }
                }
            }
        }
    }
}
