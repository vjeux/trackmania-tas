//! Debug neighbor distances. Usage: dbgavg HIS.ITEM SRCFILE
use std::collections::BTreeMap;
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
                    let u = [(tt[cc].pos[0] as f64/0.5), (tt[cc].pos[1] as f64/0.5), (tt[cc].pos[2] as f64/0.5)];
                    spts.push([((u[0]+t[0]) as f32), ((u[1]+t[1]) as f32), ((u[2]+t[2]) as f32)]);
                }
            }
        }
    }
    println!("spts={}", spts.len());
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
    println!("hvis={}", hvis.len());
    let h0 = hvis.values().next().cloned().unwrap_or([0.0; 3]);
    println!("sample his={:?}", h0);
    let mut ddmin = f32::MAX;
    for s in &spts {
        let dd = ((s[0]-h0[0]).powi(2)+(s[1]-h0[1]).powi(2)+(s[2]-h0[2]).powi(2)).sqrt();
        ddmin = ddmin.min(dd);
    }
    println!("nearest spts to sample={:.3}mm", ddmin*1000.0);
}
