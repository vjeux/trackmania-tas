//! Diff collision triangle sets of ref vs baked. Usage: surfdiff REF BAKED
use std::collections::BTreeSet;

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0] * 1000.0).round() as i32, ((p[1] * 1000.0).round() as i32), ((p[2] * 1000.0).round() as i32))
}

fn surf(path: &str, t: [f64; 3]) -> BTreeSet<([(i32, i32, i32); 3], u8)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let sf = so.surface().unwrap();
    let mut out = BTreeSet::new();
    let tkey = |p: &[f32; 3]| {
        (((p[0] as f64 + t[0]) * 1000.0).round() as i32, ((p[1] as f64 + t[1]) * 1000.0).round() as i32, ((p[2] as f64 + t[2]) * 1000.0).round() as i32)
    };
    match &sf.surf {
        mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, .. } => {
            for t in triangles {
                let mut k = [tkey(&vertices[t.indices[0] as usize]), tkey(&vertices[t.indices[1] as usize]), tkey(&vertices[t.indices[2] as usize])];
                k.sort();
                out.insert((k, t.material_id));
            }
        }
        _ => {}
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    // optional trailing dx,dy,dz (mm, float) applied to BAKED
    let t = if a.len() > 4 { [a[3].parse().unwrap_or(0.0), a[4].parse().unwrap_or(0.0), a[5].parse().unwrap_or(0.0)] } else { [0.0, 0.0, 0.0] };
    let r = surf(&a[1], [0.0, 0.0, 0.0]);
    let b = surf(&a[2], t);
    println!("ref {} baked {}", r.len(), b.len());
    let only_r: Vec<_> = r.difference(&b).collect();
    let only_b: Vec<_> = b.difference(&r).collect();
    println!("only in ref ({}):", only_r.len());
    for (t, m) in only_r.iter().take(12) {
        println!("  phys={m} {t:?}");
    }
    println!("only in baked ({}):", only_b.len());
    for (t, m) in only_b.iter().take(12) {
        println!("  phys={m} {t:?}");
    }
}
