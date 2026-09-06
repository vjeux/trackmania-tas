fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        if let Some(p) = &s2.pre_light_gen {
            // bounds + counts
            let mut lo = [f32::MAX; 3];
            let mut hi = [f32::MIN; 3];
            let (mut nv, mut nt) = (0, 0);
            for v in &s2.visuals {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
                    nv += vis.main.as_ref().map(|m| m.count).unwrap_or(0);
                    nt += vis.index_buffer.as_ref().map(|b| b.indices.len()).unwrap_or(0) / 3;
                    if let Some(st) = vis.stream() {
                        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                            if let mapgeom::static_item::vstream::Elem::Float3(p) = e {
                                if d.name() == 0 {
                                    for q in p {
                                        for k in 0..3 { lo[k] = lo[k].min(q[k]); hi[k] = hi[k].max(q[k]); }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            println!("{} u02={:.3} u04={:.3?} bounds=[{:.1},{:.1},{:.1}]-[{:.1},{:.1},{:.1}] verts={} tris={}", path.rsplit('/').next().unwrap(), p.u02, &p.u04[..4], lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], nv, nt);
        }
    }
}
