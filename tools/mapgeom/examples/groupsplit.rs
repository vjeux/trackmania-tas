//! Do stubborn splits cluster by crystal group? Usage: groupsplit SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    // Technics (crystal mat 11) faces: group -> count; and positions per group
    let mut grp_faces: BTreeMap<u32, usize> = BTreeMap::new();
    let mut grp_pos: BTreeMap<u32, BTreeMap<[u32; 3], Vec<[f32; 3]>>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.material != 11 { continue; }
            *grp_faces.entry(f.group).or_insert(0) += 1;
            for tri in face_triangles(cr, f, 0.5) {
                for cc in 0..3 {
                    // face normal (flat, same for tri)
                    let n = tri[cc].normal;
                    grp_pos.entry(f.group).or_default().entry([tri[cc].pos[0].to_bits(), tri[cc].pos[1].to_bits(), tri[cc].pos[2].to_bits()]).or_default().push(n);
                }
            }
        }
    }
    println!("Technics faces by group: {grp_faces:?}");
    // per group: positions with 2+ distinct face normals (potential splits at any θ>0)
    for (grp, pm) in &grp_pos {
        let mut multi = 0;
        for (_, ns) in pm {
            let mut d: Vec<[f32; 3]> = Vec::new();
            for n in ns {
                if !d.iter().any(|x| (x[0]-n[0]).abs() < 1e-9 && (x[1]-n[1]).abs() < 1e-9 && (x[2]-n[2]).abs() < 1e-9) {
                    d.push(*n);
                }
            }
            if d.len() > 1 { multi += 1; }
        }
        println!("group {grp}: positions={} multi-normal={multi}", pm.len());
    }
}
