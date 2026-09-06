//! Grid test: are his positions multiples of g? Usage: gridtest FILE
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut vals: Vec<f64> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 {
                            for pp in p {
                                vals.push(pp[0] as f64);
                                vals.push(pp[1] as f64);
                                vals.push(pp[2] as f64);
                            }
                        }
                    }
                }
            }
        }
    }
    for g in [1e-6f64, 5e-7, 2e-7, 1e-7, 5e-8] {
        // fraction of values within 1% of a grid multiple (allowing fp noise 1e-12 relative?)
        let mut on = 0;
        for v in &vals {
            let q = (v / g).round();
            let dev = ((v - q*g)/g).abs();
            if dev < 0.01 || dev > 0.99 {
                on += 1;
            }
        }
        println!("grid {g:.0e}: on-grid {on}/{} ({:.1}%)", vals.len(), 100.0*on as f64/vals.len() as f64);
    }
}
