//! `e8_envgroups --pak FILE:KEY… --collection C` — the collection's decoration Scene3d groups as the env-block filter sees them: name,
//! triangle count, the y range and xz extent, and whether `envcap::env_meshes_from_pak`'s name filter DROPS it (contains "sky" or "water")
//! (E8, 2026-10-01: RI's AC06207064 gathers the DOME under the island — is the lake bed a dropped "water" group?).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let flag = |n: &str| a.iter().position(|x| x == n).and_then(|i| a.get(i + 1).cloned());
    let coll = flag("--collection").unwrap_or_else(|| "RedIsland".into());
    let mut store = mapgeom::store::DataStore::empty();
    for (i, x) in a.iter().enumerate() { if x == "--pak" { if let Some((pp, key)) = a[i + 1].rsplit_once(':') { store.add_pak(pp, key).unwrap_or_else(|e| panic!("pak {pp}: {e}")); } } }
    let s3 = mapgeom::envblock::layout_path(&store, &coll);
    println!("decoration layout: {s3}");
    let model = store.load_model(&s3).unwrap_or_else(|e| panic!("{s3}: {e}"));
    let mut c = mapgeom::geom::Collector::new(&mut store);
    c.model(&model, &mapgeom::geom::IDENTITY, 0);
    let mut total = 0usize;
    for (name, g) in &c.scene.groups {
        let lower = name.to_ascii_lowercase();
        let dropped = lower.contains("sky") || lower.contains("water");
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &g.verts { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
        total += g.tris.len();
        println!("  {}{name}: {} tris, {} verts; x [{:.0}, {:.0}] y [{:.1}, {:.1}] z [{:.0}, {:.0}]", if dropped { "DROPPED " } else { "kept    " }, g.tris.len(), g.verts.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
    }
    println!("{} groups, {total} triangles", c.scene.groups.len());
}
