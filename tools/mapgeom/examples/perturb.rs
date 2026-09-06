//! Perturb positions by ±1µm random; re-cluster; compare counts.
//! Usage: perturb SRCFILE SEED
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u64 = a[2].parse().unwrap();
    let targets: Vec<(&str, usize)> = vec![
        ("TrackBorders", 1078), ("TechnicsTrims", 1136),
        ("TechnicsSpecials", 6074), ("Technics", 1280), ("RoadTech", 279),
        ("SpecialFXTurbo", 764),
    ];
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let stems: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or_default()).collect();
    // simple LCG
    let mut rng = seed;
    let mut rnd = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng >> 33) as f64) / (u32::MAX as f64) - 0.5
    };
    let mut permat: BTreeMap<usize, Vec<([f32; 3], [f32; 3], f32)>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.material < 0 { continue; }
            // perturb each vert once (by index) for consistency
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| {
                let p = cr.positions[*i as usize];
                // deterministic perturb per position index
                let mut r = (*i as u64).wrapping_mul(1099511628211).wrapping_add(seed);
                r ^= r >> 29; r = r.wrapping_mul(0xbf58476d1ce4e5b9); r ^= r >> 27;
                let j1 = ((r >> 20) & 0xfff) as f32 / 4096.0 - 0.5;
                let j2 = ((r >> 8) & 0xfff) as f32 / 4096.0 - 0.5;
                let j3 = (r & 0xfff) as f32 / 4096.0 - 0.5;
                [p[0] + j1*2e-6, p[1] + j2*2e-6, p[2] + j3*2e-6]
            }).collect();
            if pts.len() < 3 { continue; }
            let mut n = [0f32; 3];
            for i in 0..pts.len() {
                let a2 = pts[i];
                let b = pts[(i+1)%pts.len()];
                n[0] += (a2[1]-b[1])*(a2[2]+b[2]);
                n[1] += (a2[2]-b[2])*(a2[0]+b[0]);
                n[2] += (a2[0]-b[0])*(a2[1]+b[1]);
            }
            let l = (n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt();
            if l < 1e-12 { continue; }
            let nn = [n[0]/l, n[1]/l, n[2]/l];
            let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
            if pts.len() == 3 { tris.push([pts[0], pts[1], pts[2]]); }
            else { for i in 2..pts.len() { tris.push([pts[1], pts[i], pts[(i+1)%pts.len()]]); } }
            for tri in &tris {
                let e1 = [tri[1][0]-tri[0][0], tri[1][1]-tri[0][1], tri[1][2]-tri[0][2]];
                let e2 = [tri[2][0]-tri[0][0], tri[2][1]-tri[0][1], tri[2][2]-tri[0][2]];
                let cr2 = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                let area = (cr2[0]*cr2[0]+cr2[1]*cr2[1]+cr2[2]*cr2[2]).sqrt()/2.0;
                for cc in 0..3 {
                    permat.entry(f.material as usize).or_default().push((tri[cc], nn, area));
                }
            }
        }
    }
    let _ = rnd;
    let cidx = |stem: &str| -> Option<usize> {
        stems.iter().position(|s| {
            let full = format!("Stadium\\Media\\Material\\{s}");
            let r = mapgeom::static_item::build::resolve_crystal_link(&full);
            r.rsplit('\\').next().unwrap_or(r) == stem
        }).or_else(|| stems.iter().position(|s| s == stem))
    };
    let cos_max = 45.5729f32.to_radians().cos();
    let mut total_err = 0i64;
    let mut line = format!("perturb seed={seed}:");
    for (stem, want) in &targets {
        let ci = match cidx(stem) { Some(i) => i, None => continue };
        let corners = match permat.get(&ci) { Some(v) => v, None => continue };
        let mut bypos: BTreeMap<[u32; 3], Vec<usize>> = BTreeMap::new();
        for (i, (p, _, _)) in corners.iter().enumerate() {
            bypos.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push(i);
        }
        let mut clusters = 0;
        for (_, idxs) in &bypos {
            if idxs.len() < 2 { clusters += 1; continue; }
            let mut ord = idxs.clone();
            ord.sort_by(|a, b| corners[*b].2.partial_cmp(&corners[*a].2).unwrap());
            let mut seeds: Vec<[f32; 3]> = Vec::new();
            for gi in ord {
                let n = corners[gi].1;
                if !seeds.iter().any(|s| s[0]*n[0]+s[1]*n[1]+s[2]*n[2] >= cos_max) {
                    seeds.push(n);
                }
            }
            clusters += seeds.len().max(1);
        }
        let err = clusters as i64 - *want as i64;
        total_err += err.abs();
        line += &format!(" {stem}={clusters}({err:+})");
    }
    println!("{line} TOTERR={total_err}");
}
