//! Per visual: decls, uv0 range + sample values, main fields. Usage: visdump FILE [MATSUBSTR]
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let filt = a.get(2).map(|s| s.to_string());
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for (gi, g) in s2.shaded_geoms.iter().enumerate() {
        let mi = g.material_index.max(0) as usize;
        let mname = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").to_string()).unwrap_or("?".into());
        if let Some(f) = &filt {
            if !mname.contains(f.as_str()) {
                continue;
            }
        }
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let m = vis.main.as_ref().unwrap();
                println!("geom{gi} vis{vi} mat={mname} flags=0x{:x} count={} u02={} u03={} uvgroups={:?} texsets={:?} bitmaps={}", m.chunk_flags, m.count, m.u02, m.u03, m.uv_groups, m.tex_coord_sets, m.bitmap_elems.len());
                if let Some(st) = vis.stream() {
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float2(u) => {
                                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                                for q in u {
                                    for k in 0..2 {
                                        lo[k] = lo[k].min(q[k]);
                                        hi[k] = hi[k].max(q[k]);
                                    }
                                }
                                println!("  decl name={} type={} n={} uvrange [{:.3},{:.3}]-[{:.3},{:.3}] sample {:?}", d.name(), d.ty(), u.len(), lo[0], lo[1], hi[0], hi[1], &u[..u.len().min(4)]);
                            }
                            Elem::Float3(p) => println!("  decl name={} type={} n={}", d.name(), d.ty(), p.len()),
                            Elem::Word(w) => println!("  decl name={} type={} n={} first={:08x}", d.name(), d.ty(), w.len(), w[0]),
                            _ => println!("  decl other"),
                        }
                    }
                }
            }
        }
    }
}
