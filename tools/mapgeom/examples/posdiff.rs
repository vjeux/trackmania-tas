//! Find positions in MINE missing from HIS and vice versa. Usage: posdiff HIS MINE dx dy dz SUBSTR
use std::collections::BTreeSet;
use mapgeom::static_item::vstream::Elem;
fn loadpos(path: &str, substr: &str) -> Vec<[f32; 3]> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(substr) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 { out.extend(p.iter().cloned()); }
                    }
                }
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let r = loadpos(&a[1], &a[6]);
    let m = loadpos(&a[2], &a[6]);
    let rset: BTreeSet<[u32; 3]> = r.iter().map(|p| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).collect();
    // our positions (translated) missing from his exact set; find nearest his-dist
    let mut missing: Vec<([f32; 3], f32)> = Vec::new();
    for p in &m {
        let q = [p[0]+d[0], p[1]+d[1], p[2]+d[2]];
        if !rset.contains(&[q[0].to_bits(), q[1].to_bits(), q[2].to_bits()]) {
            let mut best = f32::MAX;
            for h in &r {
                let dd = ((h[0]-q[0]).powi(2)+(h[1]-q[1]).powi(2)+(h[2]-q[2]).powi(2)).sqrt();
                if dd < best { best = dd; }
            }
            missing.push((q, best * 1000.0));
        }
    }
    missing.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
    println!("ours-missing-from-his: n={} nearest-dist-mm min={:.4} p50={:.4} max={:.4}",
        missing.len(), missing[0].1, missing[missing.len()/2].1, missing[missing.len()-1].1);
    for (p, dd) in missing.iter().take(5) {
        println!("  [{:.6},{:.6},{:.6}] nearest {:.4}mm", p[0], p[1], p[2], dd);
    }
    // and reverse
    let mset: BTreeSet<[u32; 3]> = m.iter().map(|p| [(p[0]+d[0]).to_bits(), (p[1]+d[1]).to_bits(), (p[2]+d[2]).to_bits()]).collect();
    let mut missing2 = 0;
    for h in &r {
        if !mset.contains(&[h[0].to_bits(), h[1].to_bits(), h[2].to_bits()]) { missing2 += 1; }
    }
    println!("his-missing-from-ours(translated): n={missing2}");
}
