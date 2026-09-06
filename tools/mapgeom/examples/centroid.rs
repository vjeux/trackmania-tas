//! Centroid difference (coarse translation). Usage: centroid A.ITEM B.ITEM
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    for path in [&a[1], &a[2]] {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let (mut sx, mut sy, mut sz, mut n) = (0.0f64, 0.0f64, 0.0f64, 0u64);
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 {
                                for pp in p {
                                    sx += pp[0] as f64; sy += pp[1] as f64; sz += pp[2] as f64; n += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("{}: centroid=[{:.6},{:.6},{:.6}] n={n}", path.rsplit('/').next().unwrap(), sx/n as f64, sy/n as f64, sz/n as f64);
    }
}
