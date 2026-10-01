//! `e5_envextent --collection C --pak FILE:KEY … [--water]` — THE ENVIRONMENT BLOCK'S EXTENT AS THE PORT DRAWS IT (E5, 2026-09-28):
//! every leaf group of the collection's decoration layout (envblock::layout_path → envcap::env_meshes_from_pak, the exact meshes
//! that become the peel's env layer) with its triangle count and its world-space bounding box, and per "warp" group the xz range
//! of its vertices — the census the coordinator asked for (22:36Z): does the skirt end at the mesh's own vertices (the game's) or
//! does the port run it to ±97 km? The Collector applies the Scene3d's own node transforms and nothing else.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let coll = f("--collection").unwrap_or_else(|| "WhiteShore".into());
    let mut store = mapgeom::store::DataStore::empty();
    for (i, arg) in a.iter().enumerate() { if arg == "--pak" { if let Some(v) = a.get(i + 1) { let (pp, key) = v.rsplit_once(':').expect("--pak FILE:KEY"); store.add_pak(pp, key).expect("pak"); } } }
    if a.iter().any(|x| x == "--water") { std::env::set_var("LMTOOL_ENV_WATER", "black"); }
    let s3 = mapgeom::envblock::layout_path(&store, &coll);
    println!("{coll}: decoration layout {s3}");
    let (meshes, dropped) = lightmap::envcap::env_meshes_from_pak(&mut store, &s3).unwrap_or_else(|e| panic!("{e}"));
    println!("{} leaf groups kept, {} water / sky triangles left out", meshes.len(), dropped);
    let mut all = ([f32::MAX; 3], [f32::MIN; 3]);
    for m in &meshes {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for t in &m.tris { for v in t { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); all.0[k] = all.0[k].min(v[k]); all.1[k] = all.1[k].max(v[k]); } } }
        // a histogram of the vertices' |x − 1024| / |z − 1024| (the 64×64 footprint centre when anchored at the origin) in 1 km bins
        let mut far = std::collections::BTreeMap::<i32, usize>::new();
        for t in &m.tris { for v in t { let r = ((v[0] - 1024.0).abs().max((v[2] - 1024.0).abs()) / 1000.0).floor() as i32; *far.entry(r).or_insert(0) += 1; } }
        let hist: Vec<String> = far.iter().map(|(k, n)| format!("{}–{} km: {n}", k, k + 1)).collect();
        println!("  {:<28} {:>6} tris  x [{:>10.1}, {:>10.1}]  y [{:>8.2}, {:>8.2}]  z [{:>10.1}, {:>10.1}]  normals {}", m.name, m.tris.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], if m.norms.is_empty() { "no" } else { "yes" });
        if m.name.to_ascii_lowercase().contains("warp") { println!("      vertices by Chebyshev distance from (1024, ·, 1024): {}", hist.join(", ")); }
        // THE WINDING CENSUS (E7 2026-09-30, RI's k736 vs g23's k38 env layers): the geometric normal cross(e1, e2) of every triangle —
        // how many point up (y > 0) / down — and, when the file carries vertex normals, how many geometric normals AGREE with them
        // (the hardware's front face = the screen-space winding; PS 16752 blackens back faces, so a winding the port reads inverted
        // paints the skirt black from the side the game lights)
        {
            let (mut up, mut down, mut agree, mut disagree) = (0usize, 0usize, 0usize, 0usize);
            for (ti, t) in m.tris.iter().enumerate() {
                let e1 = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
                let e2 = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                if n[1] > 0.0 { up += 1; } else if n[1] < 0.0 { down += 1; }
                if let Some(vn) = m.norms.get(ti) { let s: f32 = (0..3).map(|k| n[0] * vn[k][0] + n[1] * vn[k][1] + n[2] * vn[k][2]).sum(); if s > 0.0 { agree += 1; } else if s < 0.0 { disagree += 1; } }
            }
            println!("      winding: geometric normal up {up} / down {down}{}", if m.norms.is_empty() { String::new() } else { format!("; agrees with the file's vertex normals {agree} / disagrees {disagree}") });
        }
    }
    println!("ALL kept leaves: x [{:.1}, {:.1}] y [{:.2}, {:.2}] z [{:.1}, {:.1}]", all.0[0], all.1[0], all.0[1], all.1[1], all.0[2], all.1[2]);
}
