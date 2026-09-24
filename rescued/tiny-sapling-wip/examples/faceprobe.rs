//! Source face ids for baked tris (by exact-bit position triple, 1mm fallback).
//! Usage: TINY_POS_T=.. faceprobe CRYSTAL.ITEM.GBX [12 hex words = tri1] [12 more = tri2] ...
use std::collections::BTreeMap;
fn pk(p: [f32; 3]) -> [u32; 3] { [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()] }
fn mmk(p: [f32; 3]) -> [i32; 3] { [(p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32] }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let t: [f32; 3] = std::env::var("TINY_POS_T").ok().and_then(|s| {
        let w: Vec<&str> = s.split(',').collect();
        if w.len() != 3 { return None; }
        Some([f32::from_bits(u32::from_str_radix(w[0].trim(), 16).ok()?), f32::from_bits(u32::from_str_radix(w[1].trim(), 16).ok()?), f32::from_bits(u32::from_str_radix(w[2].trim(), 16).ok()?)])
    }).unwrap();
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    let (cr, _, _) = layers.iter().find(|(_, vis, _)| *vis).unwrap();
    let mut pos2v: BTreeMap<[u32; 3], u32> = BTreeMap::new();
    let mut mm2v: BTreeMap<[i32; 3], u32> = BTreeMap::new();
    for (i, p) in cr.positions.iter().enumerate() {
        let q = [p[0]*0.5+t[0], p[1]*0.5+t[1], p[2]*0.5+t[2]];
        pos2v.entry(pk(q)).or_insert(i as u32);
        mm2v.entry(mmk(q)).or_insert(i as u32);
    }
    let mut v2f: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (fi, f) in cr.faces.iter().enumerate() {
        for v in &f.verts { v2f.entry(*v).or_default().push(fi); }
    }
    let w: Vec<u32> = a[2..].iter().map(|x| u32::from_str_radix(x.trim_start_matches("0x"), 16).unwrap()).collect();
    for tri in w.chunks(9) {
        if tri.len() < 9 { break; }
        let mut ps = [[0f32; 3]; 3];
        for k in 0..3 {
            for d in 0..3 {
                // input layout: 12 words per tri? No: 3 positions x 3 words + ... we take 9 words per tri (3x3) + skip 3 (uv?) — caller passes 9.
                ps[k][d] = f32::from_bits(tri[k*3+d]);
            }
        }
        let vs: Option<Vec<u32>> = ps.iter().map(|q| pos2v.get(&pk(*q)).copied()).collect::<Option<Vec<_>>>();
        let (vsm, how) = match vs {
            Some(v) => (Some(v), "exact"),
            None => {
                let vm: Option<Vec<u32>> = ps.iter().map(|q| mm2v.get(&mmk(*q)).copied()).collect::<Option<Vec<_>>>();
                (vm, "mm1")
            }
        };
        match vsm {
            Some(vs) => {
                let mut faces = v2f.get(&vs[0]).cloned().unwrap_or_default();
                faces.retain(|fi| vs[1..].iter().all(|v| v2f.get(v).map(|l| l.contains(fi)).unwrap_or(false)));
                let info: Vec<String> = faces.iter().map(|fi| format!("f{}nv{}mat{}", fi, cr.faces[*fi].verts.len(), cr.faces[*fi].material)).collect();
                println!("tri {:?} -> {} [{}]", vs, how, info.join(" "));
            }
            None => println!("tri UNMAPPED"),
        }
    }
}
