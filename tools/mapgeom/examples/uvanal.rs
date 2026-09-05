//! UV analysis on clips visual: distinct positions, his (pos,uv) pairs,
//! crystal (pos,uv) pairs. Usage: uvanal REF SRC MATINDEX
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeSet;

fn key2(p: &[f32; 2]) -> (i32, i32) {
    ((p[0]*1000000.0).round() as i32, ((p[1]*1000000.0).round() as i32))
}
fn key3(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, ((p[1]*1000.0).round() as i32), ((p[2]*1000.0).round() as i32))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let refdata = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&refdata).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let matindex: usize = a[3].parse().unwrap();
    let vi = s2.shaded_geoms.iter().find(|g| g.material_index as usize == matindex).unwrap().visual_index as usize;
    let mut his: BTreeSet<((i32,i32,i32),(i32,i32))> = BTreeSet::new();
    let mut hispos: BTreeSet<(i32,i32,i32)> = BTreeSet::new();
    let mut hisuv: BTreeSet<(i32,i32)> = BTreeSet::new();
    if let Some(mapgeom::static_item::Node::Visual(vis)) = s2.visuals[vi].inline.as_deref() {
        let st = vis.stream().unwrap();
        println!("visual {vi}: decls={} elems={}", st.decls.len(), st.elems.len());
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            let n = match e {
                Elem::Float3(p) => p.len(),
                Elem::Float2(u) => u.len(),
                Elem::Word(w) => w.len(),
                _ => 0,
            };
            println!("  decl name={} len={n}", d.name());
        }
        let st = vis.stream().unwrap();
        let (mut pos, mut uv) = (Vec::new(), Vec::new());
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                _ => {}
            }
        }
        for (p, u) in pos.iter().zip(uv.iter()) {
            his.insert((key3(p), key2(u)));
            hispos.insert(key3(p));
            hisuv.insert(key2(u));
        }
    }
    println!("his: {} verts, {} distinct pos, {} distinct uv, {} distinct (pos,uv)", his.len(), hispos.len(), hisuv.len(), his.len());
    // crystal pairs for same material link
    let srcdata = std::fs::read(&a[2]).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&srcdata).unwrap();
    let srclink = s2.custom_materials[matindex].inst().unwrap().link().unwrap().to_string();
    let srcmi = it.model.materials.iter().position(|m| m.inst().map(|i| i.link().unwrap_or("").to_string()) == Some(srclink.clone())).unwrap();
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    let mut cry: BTreeSet<((i32,i32,i32),(i32,i32))> = BTreeSet::new();
    let mut crypos: BTreeSet<(i32,i32,i32)> = BTreeSet::new();
    for fa in &c.faces {
        if fa.material as usize != srcmi { continue; }
        let pts: Vec<[f32; 3]> = fa.verts.iter().map(|i| { let p = c.positions[*i as usize]; [p[0]*0.5, p[1]*0.5, p[2]*0.5] }).collect();
        let uvs = c.face_uvs(fa);
        for i in 0..pts.len() {
            cry.insert((key3(&pts[i]), key2(&uvs[i])));
            crypos.insert(key3(&pts[i]));
        }
    }
    println!("crystal: {} distinct pos, {} distinct (pos,uv)", crypos.len(), cry.len());
    println!("his-pos == crystal-pos: {}", hispos == crypos);
    // uv range compare
    let mins = |s: &BTreeSet<(i32,i32)>| s.iter().fold(((i32::MAX,i32::MAX),(i32::MIN,i32::MIN)), |(a,b),x| ((a.0.min(x.0),a.1.min(x.1)),(b.0.max(x.0),b.1.max(x.1))));
    println!("his uv range: {:?}", mins(&hisuv));
    for u in &hisuv { println!("  his uv {u:?}"); }
    // crystal distinct uvs full precision
    let mut cuv: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut cuv_full: BTreeSet<String> = BTreeSet::new();
    for fa in &c.faces {
        if fa.material as usize != srcmi { continue; }
        for u in c.face_uvs(fa) {
            cuv.insert(key2(&u));
            cuv_full.insert(format!("{u:?}"));
        }
    }
    println!("crystal distinct uv (1e-6): {}, full: {}", cuv.len(), cuv_full.len());
    for u in cuv_full.iter().take(15) { println!("  cry uv {u}"); }
}
