//! Near-duplicate source check for 1-ulp misses. Usage: dupcheck HIS.ITEM SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn ulp(x: f32) -> f64 {
    let a = (x as f64).abs().max(1e-300);
    2f64.powi(a.log2().floor() as i32 - 23)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let d = [-0.001976f32, -0.000008, -0.023215];
    let t = [-0.0019760131836f32, -0.0000076293945312, -0.023214340210];
    let mut mmap: BTreeMap<(i32, i32, i32), Vec<[f32; 3]>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            if pts.len() < 3 { continue; }
            let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
            if pts.len() == 3 { tris.push([pts[0], pts[1], pts[2]]); }
            else { for i in 2..pts.len() { tris.push([pts[1], pts[i], pts[(i+1) % pts.len()]]); } }
            for tri in &tris {
                for cc in 0..3 {
                    let h = [tri[cc][0]*0.5+d[0], tri[cc][1]*0.5+d[1], tri[cc][2]*0.5+d[2]];
                    mmap.entry(mk(&h)).or_default().push(tri[cc]);
                }
            }
        }
    }
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let (mut oneulp, mut dup1ulp, mut other) = (0, 0, 0);
    for gg in &s2.shaded_geoms {
        let vi = gg.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (dd2, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if dd2.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for tt in idx.chunks(3) {
                    if tt.len() < 3 { continue; }
                    for cc in 0..3 {
                        let h = pos[tt[cc] as usize];
                        let kc = mk(&h);
                        let mut found_exact = false;
                        let mut bestd = f32::MAX;
                        let mut bests = [0f32; 3];
                        for dx in -1..=1 {
                            for dy in -1..=1 {
                                for dz in -1..=1 {
                                    if let Some(v) = mmap.get(&((kc.0+dx), (kc.1+dy), (kc.2+dz))) {
                                        for s in v {
                                            let e = [s[0]*0.5f32+t[0], s[1]*0.5f32+t[1], s[2]*0.5f32+t[2]];
                                            let dd = ((e[0]-h[0]).powi(2)+(e[1]-h[1]).powi(2)+(e[2]-h[2]).powi(2)).sqrt();
                                            if dd < bestd { bestd = dd; bests = *s; }
                                            if e[0].to_bits() == h[0].to_bits() && e[1].to_bits() == h[1].to_bits() && e[2].to_bits() == h[2].to_bits() {
                                                found_exact = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if !found_exact {
                            // deviation profile of best
                            let e = [bests[0]*0.5f32+t[0], bests[1]*0.5f32+t[1], bests[2]*0.5f32+t[2]];
                            let du = [(h[0]-e[0]) as f64 / ulp(h[0]), (h[1]-e[1]) as f64 / ulp(h[1]), (h[2]-e[2]) as f64 / ulp(h[2])];
                            let m = du[0].abs().max(du[1].abs()).max(du[2].abs());
                            if m < 1.5 {
                                oneulp += 1;
                                // near-duplicate source within 3ulp (in SOURCE bits)?
                                let mut dup = false;
                                for dx in -1..=1 {
                                    for dy in -1..=1 {
                                        for dz in -1..=1 {
                                            if let Some(v) = mmap.get(&((kc.0+dx), (kc.1+dy), (kc.2+dz))) {
                                                for s2v in v {
                                                    // same source position in 2+ axes but 1-2ulp off in one?
                                                    let same_axes = [s2v[0].to_bits() == bests[0].to_bits(), s2v[1].to_bits() == bests[1].to_bits(), s2v[2].to_bits() == bests[2].to_bits()].iter().filter(|x| **x).count();
                                                    if same_axes == 2 {
                                                        dup = true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if dup { dup1ulp += 1; }
                            } else {
                                other += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("1ulp_misses={oneulp} of_which_have_near_dup_source={dup1ulp} bigger_misses={other}");
}
