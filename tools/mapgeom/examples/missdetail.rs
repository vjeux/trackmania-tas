//! Detail on miss verts. Usage: missdetail HIS.ITEM SRCFILE N
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a[3].parse().unwrap();
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let d = [-0.001976f32, -0.000008, -0.023215];
    let t = [-0.001976013f64, -0.000007629, -0.023214340];
    let mut spts: Vec<[f32; 3]> = Vec::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            for tt in face_triangles(cr, f, 0.5) {
                for cc in 0..3 {
                    spts.push([(tt[cc].pos[0] + t[0] as f32), (tt[cc].pos[1] + t[1] as f32), (tt[cc].pos[2] + t[2] as f32)]);
                }
            }
        }
    }
    // his verts with per-axis miss flags (reuse tight-pair logic is complex; instead: his-vert whose nearest spts distance > 2um on the min axis...)
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut hvis: BTreeMap<[u32; 3], [f32; 3]> = BTreeMap::new();
    for gg in &s2.shaded_geoms {
        let vi = gg.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (dd2, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if dd2.name() == 0 {
                            for pp in p { hvis.insert([pp[0].to_bits(), pp[1].to_bits(), pp[2].to_bits()], *pp); }
                        }
                    }
                }
            }
        }
    }
    // rank his-verts by distance to nearest spts
    let mut ranked: Vec<(f32, [f32; 3])> = Vec::new();
    for h in hvis.values() {
        let mut best = f32::MAX;
        for s in &spts {
            let dd = ((s[0]-h[0]).powi(2)+(s[1]-h[1]).powi(2)+(s[2]-h[2]).powi(2)).sqrt();
            if dd < best { best = dd; }
        }
        ranked.push((best, *h));
    }
    ranked.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    for (dd, h) in ranked.iter().take(n) {
        println!("his=[{:.7},{:.7},{:.7}] nearest_src_dist={:.1}um", h[0], h[1], h[2], dd*1e6);
        let mut distinct: BTreeSet<[u32; 3]> = BTreeSet::new();
        for s in &spts {
            let d2 = ((s[0]-h[0]).powi(2)+(s[1]-h[1]).powi(2)+(s[2]-h[2]).powi(2)).sqrt();
            if d2 < 0.001 && distinct.insert([s[0].to_bits(), s[1].to_bits(), s[2].to_bits()]) {
                println!("   src_nb=[{:.7},{:.7},{:.7}] d={:.1}um", s[0], s[1], s[2], d2*1e6);
            }
        }
    }
    let _ = (mk, d);
}
