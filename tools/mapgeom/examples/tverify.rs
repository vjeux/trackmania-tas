//! Direct verify: his == round(u+t)? count bit-exact per axis and per formula.
//! Usage: tverify HIS.ITEM SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let d = [-0.001976f32, -0.000008, -0.023215];
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<[[f64; 3]; 3]>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            for t in face_triangles(cr, f, 0.5) {
                let h = [[t[0].pos[0]+d[0], t[0].pos[1]+d[1], t[0].pos[2]+d[2]],
                         [t[1].pos[0]+d[0], t[1].pos[1]+d[1], t[1].pos[2]+d[2]],
                         [t[2].pos[0]+d[0], t[2].pos[1]+d[1], t[2].pos[2]+d[2]]];
                let mut k = [mk(&h[0]), mk(&h[1]), mk(&h[2])];
                k.sort();
                mmap.entry(k).or_default().push([
                    [(t[0].pos[0]/0.5) as f64, (t[0].pos[1]/0.5) as f64, (t[0].pos[2]/0.5) as f64],
                    [(t[1].pos[0]/0.5) as f64, (t[1].pos[1]/0.5) as f64, (t[1].pos[2]/0.5) as f64],
                    [(t[2].pos[0]/0.5) as f64, (t[2].pos[1]/0.5) as f64, (t[2].pos[2]/0.5) as f64]]);
            }
        }
    }
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // candidate t per axis: try affine t and neighbors at ulp steps
    let base = [-0.001975040f64, -0.000007558, -0.023214310];
    let mut best = (0, [0.0; 3]);
    // coarse: use base
    for cand in 0..3 {
        let t = match cand {
            0 => base,
            1 => [base[0], base[1], base[2]],
            _ => base,
        };
        let (mut hit, mut tot) = ([0usize; 3], [0usize; 3]);
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
                        let p = [pos[tt[0] as usize], pos[tt[1] as usize], pos[tt[2] as usize]];
                        let mut k = [mk(&p[0]), mk(&p[1]), mk(&p[2])];
                        k.sort();
                        if let Some(v) = mmap.get(&k) {
                            let s = &v[0];
                            for perm in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                                let mut ok = true;
                                for cc in 0..3 {
                                    let ex = [(s[perm[cc]][0]*0.5+d[0] as f64) as f32, (s[perm[cc]][1]*0.5+d[1] as f64) as f32, (s[perm[cc]][2]*0.5+d[2] as f64) as f32];
                                    let dd = ((ex[0]-p[cc][0]).powi(2)+(ex[1]-p[cc][1]).powi(2)+(ex[2]-p[cc][2]).powi(2)).sqrt();
                                    if dd > 0.0005 { ok = false; break; }
                                }
                                if ok {
                                    for cc in 0..3 {
                                        for ax in 0..3 {
                                            tot[ax] += 1;
                                            let u = (s[perm[cc]][ax]*0.5) as f32;
                                            // formula A: f32(u + t)
                                            let candv = (u as f64 + t[ax]) as f32;
                                            if candv.to_bits() == p[cc][ax].to_bits() { hit[ax] += 1; }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        let tot_all: usize = tot.iter().sum();
        let hit_all: usize = hit.iter().sum();
        if hit_all > best.0 { best = (hit_all, t); }
        println!("cand{cand} t=({:.9},{:.9},{:.9}) bitexact x={}/{} y={}/{} z={}/{} total={hit_all}/{tot_all}",
            t[0], t[1], t[2], hit[0], tot[0], hit[1], tot[1], hit[2], tot[2]);
    }
    println!("best={:?}", best.1);
}
