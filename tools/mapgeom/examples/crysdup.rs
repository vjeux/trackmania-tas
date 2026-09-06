//! Source-crystal topology census: per geometry layer, positions vs distinct
//! positions (duplicate vertices at one location), face count, edge list
//! size vs the face-derived edge set (are `edges` all edges or a subset?).
//! Usage: crysdup SRC.ITEM.GBX
use std::collections::{BTreeMap, BTreeSet};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let data = std::fs::read(&path).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let mats: Vec<String> = c
        .materials
        .iter()
        .map(|m| m.inst().map(|x| x.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or_else(|| m.name.clone()))
        .collect();
    for (li, (cr, vis, col)) in mapgeom::static_item::bake::geometry_layers(&c).iter().enumerate() {
        let mut by_pos: BTreeMap<[u32; 3], Vec<u32>> = BTreeMap::new();
        for (i, p) in cr.positions.iter().enumerate() {
            by_pos.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push(i as u32);
        }
        let dups = by_pos.values().filter(|v| v.len() > 1).count();
        let mut face_edges: BTreeSet<(u32, u32)> = BTreeSet::new();
        let mut per_mat: BTreeMap<i32, usize> = BTreeMap::new();
        for f in &cr.faces {
            *per_mat.entry(f.material).or_insert(0) += 1;
            for i in 0..f.verts.len() {
                let a = f.verts[i];
                let b = f.verts[(i + 1) % f.verts.len()];
                face_edges.insert((a.min(b), a.max(b)));
            }
        }
        let listed: BTreeSet<(u32, u32)> = cr.edges.iter().map(|e| (e[0].min(e[1]), e[0].max(e[1]))).collect();
        let listed_in_faces = listed.intersection(&face_edges).count();
        println!(
            "layer {} vis={} col={} v{}: positions={} distinct={} dup-positions={} faces={} face-edges={} edges-listed={} (edge_count word {}) listed∩face={} groups={}",
            li, vis, col, cr.version, cr.positions.len(), by_pos.len(), dups, cr.faces.len(), face_edges.len(), cr.edges.len(), cr.edge_count, listed_in_faces, cr.groups.len()
        );
        for (m, n) in &per_mat {
            println!("   mat {} {} faces={}", m, mats.get(*m as usize).cloned().unwrap_or("?".into()), n);
        }
        // Are duplicate positions used by faces of the same material?
        let mut shown = 0;
        for (p, idxs) in &by_pos {
            if idxs.len() > 1 && shown < 5 {
                shown += 1;
                let fp = cr.positions[idxs[0] as usize];
                let users: Vec<String> = idxs
                    .iter()
                    .map(|&vi| {
                        let fs: Vec<String> = cr.faces.iter().enumerate().filter(|(_, f)| f.verts.contains(&vi)).map(|(fi, f)| format!("f{}m{}", fi, f.material)).collect();
                        format!("v{}:[{}]", vi, fs.join(","))
                    })
                    .collect();
                println!("   dup pos {:x}{:x}{:x} ({:.3},{:.3},{:.3}) -> {}", p[0], p[1], p[2], fp[0], fp[1], fp[2], users.join(" "));
            }
        }
    }
}
