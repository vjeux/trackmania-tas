//! Residual map: exact-matched tris, tight corner pairing, worst residuals.
//! Usage: residmap HIS.ITEM SRCFILE
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
    // src tri mmkey -> src corners (unscaled f64)
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
    let mut allres: Vec<f32> = Vec::new();
    let mut worst: Vec<(f32, String, [f32; 3])> = Vec::new();
    let (mut ntri, mut unmatch, mut unpair) = (0, 0, 0);
    for gg in &s2.shaded_geoms {
        let vi = gg.visual_index.max(0) as usize;
        let mi = gg.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let short = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
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
                    match mmap.get(&k) {
                        None => unmatch += 1,
                        Some(v) => {
                            ntri += 1;
                            let s = &v[0];
                            // optimal assignment (6 perms), tight gate 0.5mm
                            let mut best: Option<(f32, [f32; 3], [f64; 3])> = None;
                            for perm in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                                let mut ok = true;
                                let mut mx = 0.0f32;
                                for cc in 0..3 {
                                    let ex = [(s[perm[cc]][0]*0.5+d[0] as f64) as f32, (s[perm[cc]][1]*0.5+d[1] as f64) as f32, (s[perm[cc]][2]*0.5+d[2] as f64) as f32];
                                    let dd = ((ex[0]-p[cc][0]).powi(2)+(ex[1]-p[cc][1]).powi(2)+(ex[2]-p[cc][2]).powi(2)).sqrt();
                                    if dd > 0.0005 { ok = false; break; }
                                    mx = mx.max(dd);
                                }
                                if ok {
                                    // record per-corner residuals of this perm (use first ok)
                                    for cc in 0..3 {
                                        let ex = [(s[perm[cc]][0]*0.5+d[0] as f64) as f32, (s[perm[cc]][1]*0.5+d[1] as f64) as f32, (s[perm[cc]][2]*0.5+d[2] as f64) as f32];
                                        let dd = ((ex[0]-p[cc][0]).powi(2)+(ex[1]-p[cc][1]).powi(2)+(ex[2]-p[cc][2]).powi(2)).sqrt();
                                        allres.push(dd*1e6);
                                        if dd*1e6 > 50.0 {
                                            worst.push((dd*1e6, short.clone(), p[cc]));
                                        }
                                    }
                                    best = Some((mx, [0.0; 3], [0.0; 3]));
                                    break;
                                }
                            }
                            if best.is_none() { unpair += 1; }
                        }
                    }
                }
            }
        }
    }
    allres.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("matched_tris={ntri} unmatched_tris={unmatch} unpaired_cornersets={unpair} residuals_um n={} p50={:.2} p99={:.2} p999={:.2} max={:.2}",
        allres.len(), allres[allres.len()/2], allres[allres.len()*99/100], allres[allres.len()*999/1000], allres[allres.len()-1]);
    worst.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    for (r, m, p) in worst.iter().take(15) {
        println!("  {r:.0}um {m} [{:.4},{:.4},{:.4}]", p[0], p[1], p[2]);
    }
}
