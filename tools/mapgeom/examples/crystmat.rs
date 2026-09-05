//! Dump a crystal item's material order, colors, and face counts per material.
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let data = std::fs::read(&path).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&data).unwrap();
    for (i, m) in it.model.materials.iter().enumerate() {
        match m.inst() {
            Some(inst) => println!("mat{i}: link={:?} phys={} color={:?}", inst.link(), inst.physics(), inst.main.as_ref().map(|x| &x.color)),
            None => println!("mat{i}: name={} (no inst)", m.name),
        }
    }
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    let mut counts: std::collections::BTreeMap<(i32, usize, u32), usize> = std::collections::BTreeMap::new();
    for f in &c.faces {
        *counts.entry((f.material, f.verts.len(), f.group)).or_default() += 1;
    }
    println!("faces (mat, ncorners, group): {counts:?}; positions={}", c.positions.len());
}
