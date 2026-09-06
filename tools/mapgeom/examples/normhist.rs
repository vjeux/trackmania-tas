use mapgeom::static_item::vstream::Elem;
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        // geometric face normals from index+positions
        let (mut up, mut down, mut side, mut tot) = (0, 0, 0, 0);
        // stored normal histogram
        let mut stored: std::collections::BTreeMap<(bool, bool), usize> = std::collections::BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm) = (Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.clone(),
                            _ => {}
                        }
                    }
                    for n in &nrm {
                        let z = (((*n >> 20 & 0x3FF) as i32) << 22 >> 22) as f32 / 511.0;
                        // y is middle 10 bits
                        let y = (((*n >> 10 & 0x3FF) as i32) << 22 >> 22) as f32 / 511.0;
                        *stored.entry((y > 0.9, y < -0.9)).or_default() += 1;
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        tot += 1;
                        let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
                        let e1 = [b[0]-a[0], b[1]-a[1], b[2]-a[2]];
                        let e2 = [c[0]-a[0], c[1]-a[1], c[2]-a[2]];
                        let ny = e1[2]*e2[0]-e1[0]*e2[2];
                        if ny > 0.0 { up += 1; } else if ny < 0.0 { down += 1; } else { side += 1; }
                    }
                }
            }
        }
        println!("{}: tot={} up={} down={} side={} stored(up,down)={:?}", path.rsplit('/').next().unwrap(), tot, up, down, side, stored);
    }
}
