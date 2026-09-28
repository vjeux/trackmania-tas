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
    }
    println!("ALL kept leaves: x [{:.1}, {:.1}] y [{:.2}, {:.2}] z [{:.1}, {:.1}]", all.0[0], all.1[0], all.0[1], all.1[1], all.0[2], all.1[2]);
}
