//! Dump color elems + normals of each visual. Usage: coldump FILE
use mapgeom::static_item::vstream::Elem;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let data = std::fs::read(&path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for (i, v) in s2.visuals.iter().enumerate() {
        if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
            if let Some(st) = vis.stream() {
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Word(w) if d.name() == 8 => {
                            let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
                            for x in w {
                                *counts.entry(*x).or_default() += 1;
                            }
                            println!("vis{i} colors: {counts:?}");
                        }
                        Elem::Word(w) if d.name() == 5 => {
                            let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
                            for x in w {
                                *counts.entry(*x).or_default() += 1;
                            }
                            let mut v: Vec<_> = counts.into_iter().collect();
                            v.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
                            println!("vis{i} normals: {} distinct, top {:?}", v.len(), &v[..v.len().min(5)]);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
