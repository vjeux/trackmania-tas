//! Optimal pairing: prefer bit-exact source; count genuine misses.
//! Usage: bestpair HIS.ITEM SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
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
    // Usage: bestpair HIS.ITEM SRCFILE dx dy dz tx ty tz
    // (d = map translation for mmkey, t = exact f32 translation for formula E)
    let d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let t = [a[6].parse::<f32>().unwrap(), a[7].parse::<f32>().unwrap(), a[8].parse::<f32>().unwrap()];
    // ALL exact-src tris (f32 src) for candidate enumeration
    let mut stris: Vec<[[f32; 3]; 3]> = Vec::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            if pts.len() < 3 { continue; }
            if pts.len() == 3 { stris.push([pts[0], pts[1], pts[2]]); }
            else { for i in 2..pts.len() { stris.push([pts[1], pts[i], pts[(i+1) % pts.len()]]); } }
        }
    }
    // spatial hash on transformed (u+t approx with d) mmkey -> src corner list
    let mut mmap: BTreeMap<(i32, i32, i32), Vec<[f32; 3]>> = BTreeMap::new();
    for tri in &stris {
        for cc in 0..3 {
            let h = [tri[cc][0]*0.5+d[0], tri[cc][1]*0.5+d[1], tri[cc][2]*0.5+d[2]];
            mmap.entry(mk(&h)).or_default().push(tri[cc]);
        }
    }
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // every his corner: bit-exact source? (search 27 neighbor cells)
    let (mut tot, mut exact, mut near_only) = (0, 0, 0);
    let mut miss_dist: Vec<f32> = Vec::new();
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
                        tot += 1;
                        let h = pos[tt[cc] as usize];
                        let kc = mk(&h);
                        let mut found_exact = false;
                        let mut bestd = f32::MAX;
                        for dx in -1..=1 {
                            for dy in -1..=1 {
                                for dz in -1..=1 {
                                    if let Some(v) = mmap.get(&((kc.0+dx), (kc.1+dy), (kc.2+dz))) {
                                        for s in v {
                                            let e = [s[0]*0.5f32+t[0], s[1]*0.5f32+t[1], s[2]*0.5f32+t[2]];
                                            let dd = ((e[0]-h[0]).powi(2)+(e[1]-h[1]).powi(2)+(e[2]-h[2]).powi(2)).sqrt();
                                            if dd < bestd { bestd = dd; }
                                            if e[0].to_bits() == h[0].to_bits() && e[1].to_bits() == h[1].to_bits() && e[2].to_bits() == h[2].to_bits() {
                                                found_exact = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if found_exact { exact += 1; }
                        else {
                            near_only += 1;
                            miss_dist.push(bestd*1e6);
                        }
                    }
                }
            }
        }
    }
    miss_dist.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("corners={tot} bitexact={exact} ({:.2}%) genuine_miss={near_only}", 100.0*exact as f32/tot as f32);
    if !miss_dist.is_empty() {
        println!("miss nearest-source dist um: min={:.2} p50={:.2} max={:.2}", miss_dist[0], miss_dist[miss_dist.len()/2], miss_dist[miss_dist.len()-1]);
    }
}
