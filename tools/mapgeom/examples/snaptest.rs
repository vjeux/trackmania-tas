//! Test position quantization hypotheses. Usage: snaptest HIS.MINE... (pairs) -- hypothesis check
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn loadpos(path: &str) -> Vec<[f32; 3]> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
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
    // translation fitted earlier
    let d = [-0.001976f32, -0.000008, -0.023215];
    let r = loadpos(&a[1]);
    // hypothesis: round(x, 4 decimals). residual of his vs rounded-his should be ~0 (float noise)
    // and his-vs-(rounded ours) should match micro-edit distribution
    let m = loadpos(&a[2]);
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    let mut res4: Vec<f32> = Vec::new();
    for p in &r {
        // undo translation, unscale? test raw: is (p-d) a multiple of 1e-4?
        for ax in 0..3 {
            let v = p[ax] - d[ax];
            let q = (v * 10000.0).round() / 10000.0;
            res4.push((v - q).abs() * 1e6); // micrometers
        }
    }
    res4.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("his-vs-dec4round: n={} p50={:.2}um p90={:.2}um p99={:.2}um max={:.2}um",
        res4.len(), res4[res4.len()/2], res4[res4.len()*9/10], res4[res4.len()*99/100], res4[res4.len()-1]);
    // same test for OUR positions (should fail: exact halves are multiples of ...?)
    let mut res4m: Vec<f32> = Vec::new();
    for p in &m {
        for ax in 0..3 {
            let q = (p[ax] * 10000.0).round() / 10000.0;
            res4m.push((p[ax] - q).abs() * 1e6);
        }
    }
    res4m.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("ours-vs-dec4round: n={} p50={:.2}um p90={:.2}um max={:.2}um",
        res4m.len(), res4m[res4m.len()/2], res4m[res4m.len()*9/10], res4m[res4m.len()-1]);
    let _ = hist;
}
