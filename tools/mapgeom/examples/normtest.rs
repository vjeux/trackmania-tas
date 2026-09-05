//! Which smooth-normal scheme reproduces the reference item's stored normals?
//! Usage: normtest REF.Item.Gbx SRC-CRYSTAL.Item.Gbx MATINDEX
//! For the ref visual using material MATINDEX: map each vert position to its
//! stored normal; compute simple/angle/area-weighted normals from the source
//! crystal's same-material faces (scaled 0.5); report match rates after Dec3N.
use mapgeom::static_item::vstream::Elem;
use std::collections::{BTreeMap, BTreeSet};

fn dec3n_unpack(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn dec3n_pack(n: [f32; 3]) -> u32 {
    let q = |x: f32| ((x * 511.0).round() as i32).clamp(-511, 511) as u32 & 0x3FF;
    q(n[0]) | (q(n[1]) << 10) | (q(n[2]) << 20)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();
    if l < 1e-12 { [0.0, 1.0, 0.0] } else { [v[0]/l, v[1]/l, v[2]/l] }
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0]+a[1]*b[1]+a[2]*b[2] }

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let refdata = std::fs::read(&a[1]).unwrap();
    let srcdata = std::fs::read(&a[2]).unwrap();
    let matindex: usize = a[3].parse().unwrap();
    // ref visual with material matindex: pos -> packed normal set
    let f = mapgeom::static_item::file::parse_file(&refdata).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let gi = s2.shaded_geoms.iter().position(|g| g.material_index as usize == matindex).unwrap();
    let vi = s2.shaded_geoms[gi].visual_index as usize;
    let mut pos2n: BTreeMap<[u32; 3], BTreeSet<u32>> = BTreeMap::new();
    if let Some(mapgeom::static_item::Node::Visual(vis)) = s2.visuals[vi].inline.as_deref() {
        let st = vis.stream().unwrap();
        let mut pos: &Vec<[f32; 3]> = &Vec::new();
        let mut nrm: &Vec<u32> = &Vec::new();
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => pos = p,
                Elem::Word(w) if d.name() == 5 => nrm = w,
                _ => {}
            }
        }
        for (p, n) in pos.iter().zip(nrm.iter()) {
            pos2n.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().insert(*n);
        }
    }
    println!("ref visual: {} verts, {} distinct positions", pos2n.values().map(|s| s.len()).sum::<usize>(), pos2n.len());
    // source crystal faces of the equivalent material: match by link
    let it = mapgeom::crystal::ItemCrystal::open(&srcdata).unwrap();
    let srclink = s2.custom_materials[matindex].inst().unwrap().link().unwrap().to_string();
    let srcmi = it.model.materials.iter().position(|m| m.inst().map(|i| i.link().unwrap_or("").to_string()) == Some(srclink.clone())).unwrap();
    println!("src material {srcmi} link {srclink}");
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    // triangulate (fan) scaled faces of srcmi
    let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
    for fa in &c.faces {
        if fa.material as usize != srcmi { continue; }
        let pts: Vec<[f32; 3]> = fa.verts.iter().map(|i| { let p = c.positions[*i as usize]; [p[0]*0.5, p[1]*0.5, p[2]*0.5] }).collect();
        for i in 1..pts.len()-1 { tris.push([pts[0], pts[i], pts[i+1]]); }
    }
    println!("src tris: {}", tris.len());
    // per position: adjacent (face normal, angle at vert, area)
    let mut adj: BTreeMap<[u32; 3], Vec<([f32; 3], f32, f32)>> = BTreeMap::new();
    for t in &tris {
        let e1 = sub(t[1], t[0]); let e2 = sub(t[2], t[0]); let e3 = sub(t[2], t[1]);
        let fn_ = norm(cross(e1, e2));
        let area = (dot(cross(e1, e2), cross(e1, e2))).sqrt() / 2.0;
        let ang = |u: [f32; 3], v: [f32; 3]| dot(norm(u), norm(v)).clamp(-1.0, 1.0).acos();
        let angs = [ang(e1, e2), ang(sub(t[0], t[1]), e3), ang(sub(t[0], t[2]), sub(t[1], t[2]))];
        for k in 0..3 {
            adj.entry([t[k][0].to_bits(), t[k][1].to_bits(), t[k][2].to_bits()]).or_default().push((fn_, angs[k], area));
        }
    }
    let schemes = ["simple", "angle", "area"];
    let mut match_count = [0, 0, 0];
    let mut total = 0;
    let mut mism = Vec::new();
    for (p, ns) in &pos2n {
        // ref position (half scale) -> source position (full): x2
        let pf = [f32::from_bits(p[0])*2.0, f32::from_bits(p[1])*2.0, f32::from_bits(p[2])*2.0];
        let key = [pf[0].to_bits(), pf[1].to_bits(), pf[2].to_bits()];
        let Some(ad) = adj.get(&key) else { println!("ref pos {pf:?} not in source!"); continue; };
        // candidate normals
        let mut acc = [[0f32; 3]; 3];
        for (fn_, ang, area) in ad {
            for k in 0..3 { acc[k][k] += fn_[k]; } // simple
            let w = [1.0, *ang, *area];
            for s in 0..3 { /* filled below */ }
            let _ = w;
        }
        let cands: Vec<[f32; 3]> = vec![
            norm([ad.iter().map(|(n, _, _)| n[0]).sum::<f32>(), ad.iter().map(|(n, _, _)| n[1]).sum::<f32>(), ad.iter().map(|(n, _, _)| n[2]).sum::<f32>()]),
            norm([ad.iter().map(|(n, a, _)| n[0]*a).sum::<f32>(), ad.iter().map(|(n, a, _)| n[1]*a).sum::<f32>(), ad.iter().map(|(n, a, _)| n[2]*a).sum::<f32>()]),
            norm([ad.iter().map(|(n, _, r)| n[0]*r).sum::<f32>(), ad.iter().map(|(n, _, r)| n[1]*r).sum::<f32>(), ad.iter().map(|(n, _, r)| n[2]*r).sum::<f32>()]),
        ];
        total += 1;
        for (s, cn) in cands.iter().enumerate() {
            if ns.contains(&dec3n_pack(*cn)) { match_count[s] += 1; }
            else if mism.len() < 5 { mism.push((pf, *cn, ns.clone())); }
        }
    }
    println!("positions: {total}; matches simple={} angle={} area={}", match_count[0], match_count[1], match_count[2]);
    for (pf, cn, ns) in mism.iter().take(3) {
        println!("  pos {pf:?} computed {cn:?} packed {:08x} vs stored {:?}", dec3n_pack(*cn), ns.iter().map(|n| { let d = dec3n_unpack(*n); format!("{d:?}") }).collect::<Vec<_>>());
    }
    let _ = schemes;
}
