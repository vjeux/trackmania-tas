//! True weld test: distinct source positions per his-vert, mean match?
//! Usage: welddistinct HIS.ITEM SRCFILE
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
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
    for eps in [0.0001f32, 0.0003, 0.001] {
        let (mut trivial, mut weld_ok, mut weld_bad) = (0, 0, 0);
        let mut worst = 0.0f32;
        let mut baddest = [0.0f32; 3];
        for h in hvis.values() {
            let mut distinct: BTreeSet<[u32; 3]> = BTreeSet::new();
            let mut nb: Vec<[f32; 3]> = Vec::new();
            for s in &spts {
                let dd = ((s[0]-h[0]).powi(2)+(s[1]-h[1]).powi(2)+(s[2]-h[2]).powi(2)).sqrt();
                if dd < eps && distinct.insert([s[0].to_bits(), s[1].to_bits(), s[2].to_bits()]) {
                    nb.push(*s);
                }
            }
            if nb.len() <= 1 {
                trivial += 1;
                continue;
            }
            let n = nb.len() as f32;
            let mean = [nb.iter().map(|p| p[0]).sum::<f32>()/n, nb.iter().map(|p| p[1]).sum::<f32>()/n, nb.iter().map(|p| p[2]).sum::<f32>()/n];
            let dd = ((mean[0]-h[0]).powi(2)+(mean[1]-h[1]).powi(2)+(mean[2]-h[2]).powi(2)).sqrt();
            if dd < 2e-6 { weld_ok += 1; }
            else {
                weld_bad += 1;
                if dd > worst { worst = dd; baddest = *h; }
            }
        }
        println!("eps={eps}: trivial={trivial} weld_mean_ok={weld_ok} weld_mean_bad={weld_bad} worst={worst:.2e} at {baddest:?}");
    }
}
