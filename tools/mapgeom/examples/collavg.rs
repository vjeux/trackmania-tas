//! Collision average-weld test. Usage: collavg HIS.ITEM MINE.ITEM
use std::collections::{BTreeMap, BTreeSet};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut sets = Vec::new();
    for path in [&a[1], &a[2]] {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let mut s = Vec::new();
        if let Some(surf) = so.surface() {
            if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles: _, version: _ } = &surf.surf {
                for v in vertices {
                    s.push(*v);
                }
            }
        }
        sets.push(s);
    }
    let (his, mine) = (&sets[0], &sets[1]);
    // for each his-vert, mine-verts within 0.1mm; check mean
    let eps = 0.0001f32;
    let (mut single_exact, mut multi_mean, mut multi_other, mut none) = (0, 0, 0, 0);
    for h in his {
        let mut nb: Vec<[f32; 3]> = Vec::new();
        for m in mine {
            let dd = ((m[0]-h[0]).powi(2)+(m[1]-h[1]).powi(2)+(m[2]-h[2]).powi(2)).sqrt();
            if dd < eps {
                nb.push(*m);
            }
        }
        let mut distinct: BTreeSet<[u32; 3]> = BTreeSet::new();
        for n in &nb {
            distinct.insert([n[0].to_bits(), n[1].to_bits(), n[2].to_bits()]);
        }
        if distinct.len() <= 1 {
            // check exact?
            if distinct.len() == 1 && distinct.contains(&[h[0].to_bits(), h[1].to_bits(), h[2].to_bits()]) {
                single_exact += 1;
            } else if distinct.is_empty() {
                none += 1;
            } else {
                multi_other += 1;
            }
            continue;
        }
        let n = nb.len() as f32;
        let mean = [nb.iter().map(|p| p[0]).sum::<f32>()/n, nb.iter().map(|p| p[1]).sum::<f32>()/n, nb.iter().map(|p| p[2]).sum::<f32>()/n];
        let dd = ((mean[0]-h[0]).powi(2)+(mean[1]-h[1]).powi(2)+(mean[2]-h[2]).powi(2)).sqrt();
        if dd < 2e-6 {
            multi_mean += 1;
        } else {
            multi_other += 1;
        }
    }
    println!("his_coll={}: single_exact={single_exact} multi_mean={multi_mean} multi_other={multi_other} none={none}", his.len());
    let _ = BTreeMap::<u8, u8>::new();
}
