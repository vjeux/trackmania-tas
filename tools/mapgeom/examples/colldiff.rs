//! Diff collision position sets (proximity). Usage: colldiff HIS.ITEM MINE.ITEM
use std::collections::BTreeSet;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut sets = Vec::new();
    for path in [&a[1], &a[2]] {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let mut s = BTreeSet::new();
        if let Some(surf) = so.surface() {
            if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles: _, version: _ } = &surf.surf {
                for v in vertices {
                    s.insert([v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]);
                }
            }
        }
        sets.push(s);
    }
    // proximity match (0.2mm): his-only vs mine-only
    let hv: Vec<[f32; 3]> = sets[0].iter().map(|b| [f32::from_bits(b[0]), f32::from_bits(b[1]), f32::from_bits(b[2])]).collect();
    let mv: Vec<[f32; 3]> = sets[1].iter().map(|b| [f32::from_bits(b[0]), f32::from_bits(b[1]), f32::from_bits(b[2])]).collect();
    let mut his_only = 0;
    let mut his_only_ex: Vec<[f32; 3]> = Vec::new();
    for h in &hv {
        let mut found = false;
        for m in &mv {
            if (h[0]-m[0]).abs() < 0.0002 && (h[1]-m[1]).abs() < 0.0002 && (h[2]-m[2]).abs() < 0.0002 {
                found = true;
                break;
            }
        }
        if !found {
            his_only += 1;
            if his_only_ex.len() < 8 {
                his_only_ex.push(*h);
            }
        }
    }
    let mut mine_only = 0;
    for m in &mv {
        let mut found = false;
        for h in &hv {
            if (h[0]-m[0]).abs() < 0.0002 && (h[1]-m[1]).abs() < 0.0002 && (h[2]-m[2]).abs() < 0.0002 {
                found = true;
                break;
            }
        }
        if !found { mine_only += 1; }
    }
    println!("his_coll={} mine_coll={} his_only(no mine within 0.2mm)={his_only} mine_only={mine_only}", hv.len(), mv.len());
    for h in &his_only_ex {
        println!("  his-only [{:.4},{:.4},{:.4}]", h[0], h[1], h[2]);
    }
    let _ = Elem::Float3(vec![[0.0; 3]]);
}
