fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    if a[1].contains("DecoWall") && !a[1].contains("Tiny") {
        let it = mapgeom::crystal::ItemCrystal::open(&data).unwrap();
        let layer = it.model.first_geometry().unwrap();
        let c = layer.kind.crystal().unwrap();
        let mut v = c.positions.clone(); v.sort_by(|x,y| x[1].partial_cmp(&y[1]).unwrap());
        for p in v.iter().filter(|p| p[1] > 15.0) { println!("{p:?}"); }
    } else {
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        use mapgeom::static_item::vstream::Elem;
        for vv in &s2.visuals {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vv.inline.as_deref() {
                if let Some(st) = vis.stream() {
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 {
                                let mut v = p.clone(); v.sort_by(|x,y| x[1].partial_cmp(&y[1]).unwrap());
                                for q in v.iter().filter(|q| q[1] > 7.9) { println!("{q:?}"); }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}
