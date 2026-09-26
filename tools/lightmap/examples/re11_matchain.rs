//! `re11_matchain --pak F:K LINK…` — the material chain (parent material, shader, shader flags) of pack material links (RE 11).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut links = Vec::new();
    let mut it = a.iter().skip(1);
    while let Some(x) = it.next() {
        if x == "--pak" {
            let spec = it.next().expect("--pak FILE:KEY");
            let (pp, key) = spec.rsplit_once(':').expect("--pak FILE:KEY");
            store.add_pak(pp, key).unwrap_or_else(|e| panic!("{pp}: {e}"));
        } else {
            links.push(x.clone());
        }
    }
    for l in &links {
        let c = mapgeom::envblock::material_chain(&mut store, l);
        println!("{l}: parent {:?} shader {:?} flags {:?} custom {} bitmaps {:?}", c.parent_material, c.shader, c.flags.map(|f| format!("A {:#x} B {:#x} pass {:#x} never_casts {}", f.a, f.b, f.pass_bits, f.never_casts())), c.has_custom, c.bitmaps.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>());
    }
}
