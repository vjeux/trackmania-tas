//! Build items from templates through the public crystal API and write them
//! out, so two builds of the writer can be compared byte for byte:
//!
//!     crystal_build_check <out dir> <item>...
//!
//! For each item: `full` (its own decoded mesh and materials), `three` (the
//! same mesh on the first three materials, so the node count shrinks) and
//! `more` (the materials doubled with a variant, so the node count grows).
use mapgeom::crystal::{build_item, decode_template, MaterialSpec};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let out = std::path::Path::new(&a[0]);
    std::fs::create_dir_all(out).unwrap();
    for p in &a[1..] {
        let bytes = std::fs::read(p).unwrap();
        let stem = std::path::Path::new(p).file_name().unwrap().to_string_lossy().replace(".Item.Gbx", "");
        let (mats, mesh) = decode_template(&bytes);
        let item = build_item(&bytes, "X.Item.Gbx", "X.Item.Gbx", &mats, &mesh);
        std::fs::write(out.join(format!("{stem}.full.Item.Gbx")), &item).unwrap();
        let k = mats.len().min(3);
        let mut m3 = mesh.clone();
        for f in &mut m3.faces {
            f.material = f.material.min(k as u32 - 1);
        }
        let item = build_item(&bytes, "X.Item.Gbx", "X.Item.Gbx", &mats[..k], &m3);
        std::fs::write(out.join(format!("{stem}.three.Item.Gbx")), &item).unwrap();
        let mut more: Vec<MaterialSpec> = mats.clone();
        for m in &mats {
            more.push(MaterialSpec { link: format!("{}X", m.link), physics: m.physics });
        }
        let mut mm = mesh.clone();
        for (i, f) in mm.faces.iter_mut().enumerate() {
            if i % 2 == 1 {
                f.material += mats.len() as u32;
            }
        }
        let item = build_item(&bytes, "X.Item.Gbx", "X.Item.Gbx", &more, &mm);
        std::fs::write(out.join(format!("{stem}.more.Item.Gbx")), &item).unwrap();
        println!("{stem}: {} materials, {} faces", mats.len(), mesh.faces.len());
    }
}
