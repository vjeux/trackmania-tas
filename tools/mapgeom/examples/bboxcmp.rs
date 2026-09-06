//! BBox min/max compare. Usage: bboxcmp A.ITEM B.ITEM
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut bbs = Vec::new();
    for path in [&a[1], &a[2]] {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let (mut mn, mut mx) = ([1e9f32; 3], [-1e9f32; 3]);
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 {
                                for pp in p {
                                    for ax in 0..3 {
                                        mn[ax] = mn[ax].min(pp[ax]);
                                        mx[ax] = mx[ax].max(pp[ax]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        bbs.push((mn, mx));
    }
    println!("A min=[{:.6},{:.6},{:.6}] max=[{:.6},{:.6},{:.6}]", bbs[0].0[0], bbs[0].0[1], bbs[0].0[2], bbs[0].1[0], bbs[0].1[1], bbs[0].1[2]);
    println!("B min=[{:.6},{:.6},{:.6}] max=[{:.6},{:.6},{:.6}]", bbs[1].0[0], bbs[1].0[1], bbs[1].0[2], bbs[1].1[0], bbs[1].1[1], bbs[1].1[2]);
    println!("t(min)=[{:.6},{:.6},{:.6}] t(max)=[{:.6},{:.6},{:.6}]",
        bbs[0].0[0]-bbs[1].0[0], bbs[0].0[1]-bbs[1].0[1], bbs[0].0[2]-bbs[1].0[2],
        bbs[0].1[0]-bbs[1].1[0], bbs[0].1[1]-bbs[1].1[1], bbs[0].1[2]-bbs[1].1[2]);
}
