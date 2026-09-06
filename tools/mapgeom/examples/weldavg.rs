//! Test weld-averaging: got == mean of (u+t) over neighbors within eps?
//! Usage: weldavg HIS.ITEM SRCFILE
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
    let t = [-0.001976013f64, -0.000007629, -0.023214340];
    // all src (u+t) as f32, per axis lists for neighbor search (use x/y/z separately? need 3D)
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
    // his verts (unique by bits)
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
    // for each his-vert, neighbors among spts within eps; compare mean to his
    for eps in [0.0001f32, 0.0003, 0.001] {
        let (mut ntest, mut ok) = (0, 0);
        let mut worst = 0.0f32;
        for h in hvis.values() {
            let mut nb: Vec<[f32; 3]> = Vec::new();
            for s in &spts {
                let dd = ((s[0]-h[0]).powi(2)+(s[1]-h[1]).powi(2)+(s[2]-h[2]).powi(2)).sqrt();
                if dd < eps { nb.push(*s); }
            }
            if nb.len() > 1 {
                ntest += 1;
                let n = nb.len() as f32;
                let mean = [nb.iter().map(|p| p[0]).sum::<f32>()/n, nb.iter().map(|p| p[1]).sum::<f32>()/n, nb.iter().map(|p| p[2]).sum::<f32>()/n];
                let dd = ((mean[0]-h[0]).powi(2)+(mean[1]-h[1]).powi(2)+(mean[2]-h[2]).powi(2)).sqrt();
                worst = worst.max(dd);
                if dd < 2e-6 { ok += 1; }
            }
        }
        println!("eps={eps}: multi-neighbor his-verts={ntest} mean_match={ok} worst_mean_dev={worst:.2e}");
    }
}
