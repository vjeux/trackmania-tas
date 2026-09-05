//! Print crystal faces (mat 0) with vert order, position indices, coords.
//! Usage: faceorder SRC
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let srcdata = std::fs::read(&a[1]).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&srcdata).unwrap();
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    for (i, fa) in c.faces.iter().enumerate() {
        if fa.material != 0 { continue; }
        let pts: Vec<String> = fa.verts.iter().map(|v| { let p = c.positions[*v as usize]; format!("{v}:({:.1},{:.1},{:.1})", p[0], p[1], p[2]) }).collect();
        println!("face{i}: {}", pts.join(" "));
    }
}
