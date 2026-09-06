//! Simulate clustering rules; compare counts to his |pos+n|.
//! Usage: clustersim HIS.ITEM SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // his |pos+n| per material (from splitcause; hardcode targets)
    let targets: Vec<(&str, usize)> = vec![
        ("TrackBorders", 1078), ("SignOff", 22), ("TechnicsTrims", 1136), ("Sign", 230),
        ("TechnicsSpecials", 6074), ("Technics", 1280), ("TrackWallClips", 99), ("RoadTech", 279),
        ("SpecialFXTurbo", 764), ("DecalPaint2Logo4x1", 16), ("Decal", 99),
    ];
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    // crystal mat index -> link stem
    let stems: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or_default()).collect();
    // faces per material (visible, forward): (pos, normal, area, group)
    let mut permat: BTreeMap<usize, Vec<([f32; 3], [f32; 3], f32, u32)>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.material < 0 { continue; }
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            if pts.len() < 3 { continue; }
            // Newell
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
            // fan tris
            let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
            if pts.len() == 3 { tris.push([pts[0], pts[1], pts[2]]); }
            else { for i in 2..pts.len() { tris.push([pts[1], pts[i], pts[(i+1)%pts.len()]]); } }
            for tri in &tris {
                // tri area
                let e1 = [tri[1][0]-tri[0][0], tri[1][1]-tri[0][1], tri[1][2]-tri[0][2]];
                let e2 = [tri[2][0]-tri[0][0], tri[2][1]-tri[0][1], tri[2][2]-tri[0][2]];
                let cr2 = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                let area = (cr2[0]*cr2[0]+cr2[1]*cr2[1]+cr2[2]*cr2[2]).sqrt()/2.0;
                for cc in 0..3 {
                    permat.entry(f.material as usize).or_default().push((tri[cc], nn, area, f.group));
                }
            }
        }
    }
    // rules: (name, linkage, group_req, angles)
    for angle in [30.0f32, 35.0, 40.0, 45.0, 50.0] {
        for linkage in ["single"] {
            for group_req in [false] {
                let cos_max = angle.to_radians().cos();
                let mut total_err = 0i64;
                let mut line = format!("{linkage} grp={group_req} deg={angle}:");
                for (stem, want) in &targets {
                    // find crystal mat with resolved stem == *stem (resolve Special*Turbo)
                    let ci = stems.iter().position(|s| {
                        let full = format!("Stadium\\Media\\Material\\{s}");
                        let r = mapgeom::static_item::build::resolve_crystal_link(&full);
                        r.rsplit('\\').next().unwrap_or(r) == *stem
                    });
                    let ci = match ci {
                        Some(i) => i,
                        None => {
                            // try direct stem match (TrackBorders etc.)
                            match stems.iter().position(|s| s == *stem) {
                                Some(i) => i,
                                None => continue,
                            }
                        }
                    };
                    let corners = match permat.get(&ci) {
                        Some(v) => v,
                        None => continue,
                    };
                    // group by position bits
                    let mut bypos: BTreeMap<[u32; 3], Vec<usize>> = BTreeMap::new();
                    for (i, (p, _, _, _)) in corners.iter().enumerate() {
                        bypos.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push(i);
                    }
                    let mut clusters = 0;
                    for (_, idxs) in &bypos {
                        if idxs.len() < 2 { clusters += 1; continue; }
                        // cluster
                        let mut cof: Vec<usize> = vec![usize::MAX; idxs.len()];
                        let mut nc = 0;
                        if linkage == "single" {
                            for (ci2, &gi) in idxs.iter().enumerate() {
                                let mut placed = None;
                                for cj in 0..ci2 {
                                    if cof[cj] == usize::MAX { continue; }
                                    if group_req && corners[idxs[cj]].3 != corners[gi].3 { continue; }
                                    let dot = corners[idxs[cj]].1[0]*corners[gi].1[0]+corners[idxs[cj]].1[1]*corners[gi].1[1]+corners[idxs[cj]].1[2]*corners[gi].1[2];
                                    if dot >= cos_max { placed = Some(cof[cj]); break; }
                                }
                                match placed {
                                    Some(cc2) => cof[ci2] = cc2,
                                    None => { cof[ci2] = nc; nc += 1; }
                                }
                            }
                        } else {
                            // complete linkage: join cluster iff agrees with ALL members
                            for (ci2, &gi) in idxs.iter().enumerate() {
                                let mut placed = None;
                                'outer: for cc2 in 0..nc {
                                    for cj in 0..ci2 {
                                        if cof[cj] != cc2 { continue; }
                                        if group_req && corners[idxs[cj]].3 != corners[gi].3 { continue 'outer; }
                                        let dot = corners[idxs[cj]].1[0]*corners[gi].1[0]+corners[idxs[cj]].1[1]*corners[gi].1[1]+corners[idxs[cj]].1[2]*corners[gi].1[2];
                                        if dot < cos_max { continue 'outer; }
                                    }
                                    placed = Some(cc2);
                                    break;
                                }
                                match placed {
                                    Some(cc2) => cof[ci2] = cc2,
                                    None => { cof[ci2] = nc; nc += 1; }
                                }
                            }
                        }
                        // count distinct (group_req splits groups even if same cluster? No: group_req prevents merging across groups)
                        // clusters = nc, but positions with 1 corner = 1
                        // NOTE: group_req as implemented only blocks cross-group joins; same-group still clusters. Good.
                        clusters += nc.max(1);
                    }
                    let err = clusters as i64 - *want as i64;
                    total_err += err.abs();
                    line += &format!(" {stem}={clusters}({err:+})");
                }
                println!("{line} TOTERR={total_err}");
            }
        }
    }
    let _ = a;
}
