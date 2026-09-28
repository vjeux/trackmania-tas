//! `e3_waterdesc PAK:KEY… COLLECTION…` — each collection's water descriptor from its Collection.Gbx (mapgeom::terrain::collection_water):
//! the LOCAL surface top / floor / fog depth, and the WORLD plane the port's sea-zone branch should use = the water zone's cell row
//! (layout::CollectionProfile) × 8 + yoff + top (E3 2026-09-28: the sea-zone branch used the local top as a world height).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut colls = Vec::new();
    for x in &a[1..] { if let Some((p, k)) = x.rsplit_once(':') { if std::path::Path::new(p).exists() { store.add_pak(p, k).expect("pak"); continue; } } colls.push(x.clone()); }
    for c in colls {
        match mapgeom::terrain::collection_water(&mut store, &c) {
            Ok(d) => {
                let prof = lightmap::layout::CollectionProfile::of(&c);
                println!("{c}: {:?}", d);
                println!("    profile grid {} ground_row {} yoff {} flat_zones {:?} → ground origin y {}; local top {} floor {} fog_max_depth {}", prof.grid, prof.ground_row, prof.yoff, prof.flat_zones, prof.ground_row as f32 * 8.0 + prof.yoff, d.top, d.floor, d.fog_max_depth);
            }
            Err(e) => println!("{c}: {e}"),
        }
    }
}
