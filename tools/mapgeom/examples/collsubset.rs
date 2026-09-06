//! Is collision verts subset of visual verts? + distinct counts. Usage: collsubset FILE
use std::collections::BTreeSet;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    // visual positions
    let s2 = so.solid2().unwrap();
    let mut vset: BTreeSet<[u32; 3]> = BTreeSet::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 {
                            for pp in p {
                                vset.insert([pp[0].to_bits(), pp[1].to_bits(), pp[2].to_bits()]);
                            }
                        }
                    }
                }
            }
        }
    }
    // collision verts
    if let Some(surf) = so.surface() {
        if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles: _, version: _ } = &surf.surf {
            let mut cset: BTreeSet<[u32; 3]> = BTreeSet::new();
            for v in vertices {
                cset.insert([v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]);
            }
            let inter = cset.intersection(&vset).count();
            println!("{}: coll_verts={} coll_distinct={} visual_distinct={} coll_in_visual={} ({:.1}%)",
                a[1].rsplit('/').next().unwrap(), vertices.len(), cset.len(), vset.len(), inter,
                100.0*inter as f32/cset.len() as f32);
        } else {
            println!("not a mesh surface");
        }
    } else {
        println!("no surface");
    }
}
