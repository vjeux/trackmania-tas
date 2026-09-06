//! Combined weld key simulation. Usage: combinedweld HIS.ITEM SRCFILE
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::bake::{geometry_layers, face_triangles, tangent};
use mapgeom::static_item::bake::Corner;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // per-material normal theta (Road_17)
    let ntheta: BTreeMap<&str, f32> = [
        ("TrackBorders", 58.0), ("TechnicsTrims", 45.0), ("TechnicsSpecials", 43.1),
        ("Technics", 52.0), ("RoadTech", 20.0), ("SpecialFXTurbo", 39.5),
    ].iter().cloned().collect();
    let utheta = 20.0f32;
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let stems: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or_default()).collect();
    // resolve stems
    let rstem = |s: &str| -> String {
        let full = format!("Stadium\\Media\\Material\\{s}");
        let r = mapgeom::static_item::build::resolve_crystal_link(&full);
        r.rsplit('\\').next().unwrap_or(r).to_string()
    };
    // per crystal mat: tris (pos, uv)
    let mut permat: BTreeMap<usize, Vec<[[f32; 3]; 3]>> = BTreeMap::new();
    let mut peruv: BTreeMap<usize, Vec<[[f32; 2]; 3]>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.material < 0 { continue; }
            for tri in face_triangles(cr, f, 0.5) {
                permat.entry(f.material as usize).or_default().push([tri[0].pos, tri[1].pos, tri[2].pos]);
                peruv.entry(f.material as usize).or_default().push([tri[0].uv, tri[1].uv, tri[2].uv]);
            }
        }
    }
    // his uv1 transplant? SKIP (use planar? No - need his uv1 for splits. Instead compare |pos+n+uv+U| (no uv1) to his |pos+n+uv| (no uv1)? 
    // His |pos+n+uv| (from splitcause, no uv1): TB 1086, Trims 1162, TSpecials 6376, Technics 1412, Road 279, SFX 764.
    // Simulate: normal clusters (per-mat theta) + tangent clusters (20°) + uv; count |(pos, nclust, uv, uclust)|.
    // (Approximate clusters by ids; weld key = (pos bits, nclust id, uv bits, uclust id).)
    let targets: Vec<(&str, usize)> = vec![
        ("TrackBorders", 1086), ("TechnicsTrims", 1162), ("TechnicsSpecials", 6376),
        ("Technics", 1412), ("RoadTech", 279), ("SpecialFXTurbo", 764),
    ];
    for (stem, want) in &targets {
        // find crystal mat
        let ci = stems.iter().position(|s| rstem(s) == *stem || s == *stem);
        let ci = match ci { Some(i) => i, None => continue };
        let tris = match permat.get(&ci) { Some(v) => v, None => continue };
        let uvs = match peruv.get(&ci) { Some(v) => v, None => continue };
        let theta = ntheta.get(stem).copied().unwrap_or(44.0);
        let cos_max = theta.to_radians().cos();
        let cos_u = utheta.to_radians().cos();
        // face normals + face tangents per tri
        let mut fnorms: Vec<[f32; 3]> = Vec::new();
        let mut ftans: Vec<[f32; 3]> = Vec::new();
        for (ti, tri) in tris.iter().enumerate() {
            let uv = uvs[ti];
            let e1 = [tri[1][0]-tri[0][0], tri[1][1]-tri[0][1], tri[1][2]-tri[0][2]];
            let e2 = [tri[2][0]-tri[0][0], tri[2][1]-tri[0][1], tri[2][2]-tri[0][2]];
            let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
            let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
            fnorms.push([cr[0]/l, cr[1]/l, cr[2]/l]);
            let cc = [Corner { pos: tri[0], normal: [cr[0]/l, cr[1]/l, cr[2]/l], uv: uv[0], uv1: uv[0] },
                      Corner { pos: tri[1], normal: [cr[0]/l, cr[1]/l, cr[2]/l], uv: uv[1], uv1: uv[1] },
                      Corner { pos: tri[2], normal: [cr[0]/l, cr[1]/l, cr[2]/l], uv: uv[2], uv1: uv[2] }];
            let (fu, _) = tangent(&cc);
            ftans.push(fu);
        }
        // corners: (pos, uv, tri id)
        let mut bypos: BTreeMap<[u32; 3], Vec<(usize, usize)>> = BTreeMap::new(); // pos -> (tri, corner)
        for (ti, tri) in tris.iter().enumerate() {
            for k in 0..3 {
                bypos.entry([tri[k][0].to_bits(), tri[k][1].to_bits(), tri[k][2].to_bits()]).or_default().push((ti, k));
            }
        }
        // normal clusters (seed-largest by tri area? need area; approximate by order? use seed-largest with equal area = face order? 
        // Simplify: single-linkage by normal (position order) for N, seed by area for U? For counts, use seed-largest both (need areas).
        // (Compute tri areas)
        let mut areas: Vec<f32> = Vec::new();
        for tri in tris {
            let e1 = [tri[1][0]-tri[0][0], tri[1][1]-tri[0][1], tri[1][2]-tri[0][2]];
            let e2 = [tri[2][0]-tri[0][0], tri[2][1]-tri[0][1], tri[2][2]-tri[0][2]];
            let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
            areas.push((cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt()/2.0);
        }
        // assign cluster ids (normal + tangent) per corner, then count distinct (pos, nclust, uv, uclust)
        let mut keys: BTreeSet<(String, usize, String, usize)> = BTreeSet::new();
        // (Use position bits string + uv bits string for key; cluster ids ints.)
        for (pb, corners) in &bypos {
            // normal seeds
            let mut ord: Vec<usize> = (0..corners.len()).collect();
            ord.sort_by(|a, b| areas[corners[*b].0].partial_cmp(&areas[corners[*a].0]).unwrap());
            let mut nseeds: Vec<[f32; 3]> = Vec::new();
            let mut nclust = vec![0usize; corners.len()];
            for (oi, &ci2) in ord.iter().enumerate() {
                let n = fnorms[corners[ci2].0];
                match nseeds.iter().position(|s| s[0]*n[0]+s[1]*n[1]+s[2]*n[2] >= cos_max) {
                    Some(c) => nclust[oi] = c,
                    None => { nclust[oi] = nseeds.len(); nseeds.push(n); }
                }
            }
            // tangent seeds (seed-largest by area, same order)
            let mut useeds: Vec<[f32; 3]> = Vec::new();
            let mut uclust = vec![0usize; corners.len()];
            for (oi, &ci2) in ord.iter().enumerate() {
                let n = ftans[corners[ci2].0];
                match useeds.iter().position(|s| s[0]*n[0]+s[1]*n[1]+s[2]*n[2] >= cos_u) {
                    Some(c) => uclust[oi] = c,
                    None => { uclust[oi] = useeds.len(); useeds.push(n); }
                }
            }
            for (oi, &(ti, k)) in corners.iter().enumerate() {
                // position index in ord
                let oii = ord.iter().position(|x| *x == oi).unwrap();
                let uvb = format!("{:x}{:x}", uvs[ti][k][0].to_bits(), uvs[ti][k][1].to_bits());
                let pbstr = format!("{:x}{:x}{:x}", pb[0], pb[1], pb[2]);
                keys.insert((pbstr, nclust[oii], uvb, uclust[oii]));
            }
        }
        println!("{stem}: combined |(pos,nclust,uv,uclust)|={} (want his |pos+n+uv|={want})", keys.len());
    }
    let _ = a;
}
