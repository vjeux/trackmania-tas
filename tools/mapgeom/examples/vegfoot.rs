//! Footprint of vegetation items: the entity positions of the item's
//! prefab-like entity model (VegetTreeModel refs), their bounding box and
//! count. Usage: vegfoot --pak ... NAME [NAME...]  (names under Items\Vegetation)
use mapgeom::store::DataStore;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut store = DataStore::empty();
    let mut names = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if a[i] == "--pak" {
            let spec = &a[i + 1];
            let (p, k) = spec.rsplit_once(':').unwrap();
            store.add_pak(p, k).unwrap();
            i += 2;
        } else {
            names.push(a[i].clone());
            i += 1;
        }
    }
    for n in names {
        let logical = format!("BlueBay\\Items\\Vegetation\\{n}.Item.Gbx");
        let m = match store.load_model(&logical) { Ok(m) => m, Err(e) => { println!("{n}: {e}"); continue; } };
        let mut c = mapgeom::geom::Collector::new(&mut store);
        c.model(&m, &mapgeom::geom::IDENTITY, 0);
        let ents: Vec<[f32; 3]> = c.veget_places.iter().map(|(_, p)| *p).collect();
        if ents.is_empty() {
            println!("{n}: no vegetation entity positions collected (externals: {})", m.externals.len());
            continue;
        }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &ents { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
        println!("{n}: {} trees, footprint x {:.1}..{:.1} ({:.1} m) z {:.1}..{:.1} ({:.1} m)", ents.len(), lo[0], hi[0], hi[0]-lo[0], lo[2], hi[2], hi[2]-lo[2]);
    }
}
