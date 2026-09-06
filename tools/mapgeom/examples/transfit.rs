//! Fit his = a*src + b per axis over matched tris. Usage: transfit HIS.ITEM SRCFILE
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
    // src tris halved+translated, mmkey -> src corner positions (unscaled f64)
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<([f64; 3], [f64; 3], [f64; 3])>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            for t in face_triangles(cr, f, 0.5) {
                let h = [[t[0].pos[0]+d[0], t[0].pos[1]+d[1], t[0].pos[2]+d[2]],
                         [t[1].pos[0]+d[0], t[1].pos[1]+d[1], t[1].pos[2]+d[2]],
                         [t[2].pos[0]+d[0], t[2].pos[1]+d[1], t[2].pos[2]+d[2]]];
                let mut k = [mk(&h[0]), mk(&h[1]), mk(&h[2])];
                k.sort();
                // recover unscaled src pos: crystal positions via face verts
                mmap.entry(k).or_default().push((
                    [(t[0].pos[0]/0.5) as f64, (t[0].pos[1]/0.5) as f64, (t[0].pos[2]/0.5) as f64],
                    [(t[1].pos[0]/0.5) as f64, (t[1].pos[1]/0.5) as f64, (t[1].pos[2]/0.5) as f64],
                    [(t[2].pos[0]/0.5) as f64, (t[2].pos[1]/0.5) as f64, (t[2].pos[2]/0.5) as f64]));
            }
        }
    }
    // his tris
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut pairs: Vec<([f64; 3], [f64; 3])> = Vec::new();
    let mut ntri = 0;
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
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let mut k = [mk(&p[0]), mk(&p[1]), mk(&p[2])];
                    k.sort();
                    if let Some(v) = mmap.get(&k) {
                        ntri += 1;
                        let (s0, s1, s2) = &v[0];
                        let s = [s0, s1, s2];
                        for ck in 0..3 {
                            for sk in 0..3 {
                                let hp = [s[sk][0]*0.5+d[0] as f64, s[sk][1]*0.5+d[1] as f64, s[sk][2]*0.5+d[2] as f64];
                                if (hp[0]-p[ck][0] as f64).abs() < 0.002 && (hp[1]-p[ck][1] as f64).abs() < 0.002 && (hp[2]-p[ck][2] as f64).abs() < 0.002 {
                                    pairs.push((*s[sk], [p[ck][0] as f64, p[ck][1] as f64, p[ck][2] as f64]));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("matched_tris={ntri} pairs={}", pairs.len());
    for ax in 0..3 {
        let n = pairs.len() as f64;
        let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
        for (s, h) in &pairs {
            sx += s[ax]; sy += h[ax]; sxx += s[ax]*s[ax]; sxy += s[ax]*h[ax];
        }
        let slope = (n*sxy - sx*sy) / (n*sxx - sx*sx);
        let off = (sy - slope*sx) / n;
        let mut res: Vec<f64> = pairs.iter().map(|(s, h)| (h[ax] - (slope*s[ax]+off)).abs() * 1e6).collect();
        res.sort_by(|x, y| x.partial_cmp(y).unwrap());
        println!("ax{ax}: scale={slope:.10} offset={off:.9} resid_um p50={:.3} p99={:.3} max={:.3}",
            res[res.len()/2], res[res.len()*99/100], res[res.len()-1]);
    }
}
