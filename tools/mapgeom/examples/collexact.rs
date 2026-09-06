//! Collision bit-exact rate vs formula-E. Usage: collexact HIS.ITEM SRCFILE
use std::collections::BTreeSet;
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = [-0.0019760131836f32, -0.0000076293945312, -0.023214340210];
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    // expected collision positions: all collidable faces (any phys? use all collidable layers, all faces)
    let mut expected: BTreeSet<[u32; 3]> = BTreeSet::new();
    for (cr, _, coll) in &layers {
        if !coll { continue; }
        for f in &cr.faces {
            for tri in face_triangles(cr, f, 0.5) {
                for cc in 0..3 {
                    let p = [tri[cc].pos[0]+t[0], tri[cc].pos[1]+t[1], tri[cc].pos[2]+t[2]];
                    expected.insert([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]);
                }
            }
        }
    }
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    if let Some(surf) = so.surface() {
        if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles: _, version: _ } = &surf.surf {
            let mut hit = 0;
            for v in vertices {
                if expected.contains(&[v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]) {
                    hit += 1;
                }
            }
            println!("his_coll={} bitexact_vs_formulaE={hit} ({:.1}%)", vertices.len(), 100.0*hit as f32/vertices.len() as f32);
        }
    }
}
