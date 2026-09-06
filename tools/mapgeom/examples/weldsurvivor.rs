//! Weld-survivor test: his-vert bit-matches one source AND has other distinct sources nearby?
//! Usage: weldsurvivor HIS.ITEM SRCFILE
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
    // for each his-vert: exact source match? + count distinct sources within 0.5mm
    let (mut exact_only, mut exact_plus_others, mut noexact_with_others, mut noexact_noothers) = (0, 0, 0, 0);
    let mut examples: Vec<([f32; 3], Vec<[f32; 3]>)> = Vec::new();
    for h in hvis.values() {
        let mut distinct: BTreeSet<[u32; 3]> = BTreeSet::new();
        let mut exact = false;
        for s in &spts {
            let dd = ((s[0]-h[0]).powi(2)+(s[1]-h[1]).powi(2)+(s[2]-h[2]).powi(2)).sqrt();
            if dd < 0.0005 {
                distinct.insert([s[0].to_bits(), s[1].to_bits(), s[2].to_bits()]);
                if s[0].to_bits() == h[0].to_bits() && s[1].to_bits() == h[1].to_bits() && s[2].to_bits() == h[2].to_bits() {
                    exact = true;
                }
            }
        }
        if exact && distinct.len() == 1 { exact_only += 1; }
        else if exact { exact_plus_others += 1; }
        else if distinct.is_empty() { noexact_noothers += 1; }
        else {
            noexact_with_others += 1;
            if examples.len() < 8 {
                examples.push((*h, distinct.iter().map(|b| [f32::from_bits(b[0]), f32::from_bits(b[1]), f32::from_bits(b[2])]).collect()));
            }
        }
    }
    println!("exact_only={exact_only} exact_plus_others={exact_plus_others} noexact_with_others={noexact_with_others} noexact_noothers={noexact_noothers}");
    for (h, ds) in &examples {
        println!("his=[{:.7},{:.7},{:.7}] nsrc={}", h[0], h[1], h[2], ds.len());
        for s in ds.iter().take(6) {
            println!("   src=[{:.7},{:.7},{:.7}]", s[0], s[1], s[2]);
        }
    }
}
