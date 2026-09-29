//! `e6_lmnormals MAP.Map.Gbx NAME_SUBSTR…` — the LM MESH of each named embedded item (lmmesh::lm_mesh_of_item, the accumulate's
//! receiver geometry): vertex count, triangle count, the NORMAL length histogram (|n| < 0.9 / 0.9–0.99 / 0.99–1.01 / 1.01–1.1 / > 1.1),
//! the mean |n|, the psize modes, and the tangent-frame kinds — a non-unit normal scales the sun term (PS 15187 uses n as stored) and
//! the accumulate's cosine. E6 2026-09-29 (V5 17:55Z: the tall hills' planes ×0.6 with signs 100 %).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    let files = mapgeom::embedded::files(&m).expect("embedded files");
    let mut by_name: std::collections::BTreeMap<String, &Vec<u8>> = Default::default();
    for (k, v) in &files { by_name.insert(k.rsplit(['/', '\\']).next().unwrap_or(k).to_string(), v); }
    for (name, bytes) in &by_name {
        if !name.to_ascii_lowercase().ends_with(".item.gbx") { continue; }
        if a.len() > 2 && !a[2..].iter().any(|s| name.contains(s.as_str())) { continue; }
        let mesh = match lightmap::lmmesh::lm_mesh_of_item(bytes) { Ok(Some(m)) => m, Ok(None) => { println!("{name}: no LM mesh"); continue } Err(e) => { println!("{name}: {e}"); continue } };
        let mut hist = [0usize; 5];
        let mut sum = 0f64;
        let mut psize: std::collections::BTreeMap<i32, usize> = Default::default();
        let (mut ymin, mut ymax) = (f32::MAX, f32::MIN);
        for v in &mesh.verts {
            let l = (v.normal[0] * v.normal[0] + v.normal[1] * v.normal[1] + v.normal[2] * v.normal[2]).sqrt();
            sum += l as f64;
            let b = if l < 0.9 { 0 } else if l < 0.99 { 1 } else if l <= 1.01 { 2 } else if l <= 1.1 { 3 } else { 4 };
            hist[b] += 1;
            *psize.entry(v.psize.round() as i32).or_default() += 1;
            ymin = ymin.min(v.pos[1]); ymax = ymax.max(v.pos[1]);
        }
        let n = mesh.verts.len().max(1);
        println!("{name}: {} verts, {} tris, local y [{ymin:.1}, {ymax:.1}]; |n| mean {:.4}; <0.9 {} | 0.9–0.99 {} | 0.99–1.01 {} | 1.01–1.1 {} | >1.1 {}; psize {:?}; first normal {:?} tangent {:?}",
            mesh.verts.len(), mesh.indices.len() / 3, sum / n as f64, hist[0], hist[1], hist[2], hist[3], hist[4], psize, mesh.verts.first().map(|v| v.normal), mesh.verts.first().map(|v| v.tangent));
    }
}
