//! Formula shootout on near-zero-Y verts. Usage: zeroform HIS.ITEM SRCFILE
use std::collections::BTreeSet;
use mapgeom::static_item::bake::geometry_layers;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let t64 = [-0.001976013f64, -0.000007629, -0.023214340];
    let mut svals: BTreeSet<u32> = BTreeSet::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            for vi in &f.verts {
                svals.insert(cr.positions[*vi as usize][1].to_bits());
            }
        }
    }
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut hvals: BTreeSet<u32> = BTreeSet::new();
    for gg in &s2.shaded_geoms {
        let vi = gg.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (dd2, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if dd2.name() == 0 {
                            for pp in p {
                                if pp[1].abs() < 0.01 { hvals.insert(pp[1].to_bits()); }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("distinct srcY={} distinct hisY(near0)={}", svals.len(), hvals.len());
    // for each distinct hisY, try to express as f(srcY) for formulas; report best formula coverage
    let formulas: Vec<(&str, Box<dyn Fn(f64) -> f32>)> = vec![
        ("A:(s/2+t)f64", Box::new(|s: f64| (s*0.5 + t64[1]) as f32)),
        ("C:(s+2t)/2f64", Box::new(|s: f64| ((s + 2.0*t64[1])/2.0) as f32)),
        ("E:s*.5f32+t32", Box::new(|s: f64| (s as f32)*0.5f32 + t64[1] as f32)),
        ("F:(s+t)+s)/2", Box::new(|s: f64| (((s + t64[1]) + s)/2.0) as f32)),
        ("G:s/2 (no t)", Box::new(|s: f64| (s*0.5) as f32)),
        ("H:(s/2+t/2)", Box::new(|s: f64| (s*0.5 + t64[1]*0.5) as f32)),
    ];
    for (name, f) in &formulas {
        let mut hit = 0;
        for h in &hvals {
            let hf = f32::from_bits(*h);
            let mut ok = false;
            for s in &svals {
                if f(f32::from_bits(*s) as f64).to_bits() == *h { ok = true; break; }
            }
            if ok { hit += 1; }
            let _ = hf;
        }
        println!("{name}: {hit}/{}", hvals.len());
    }
    // show raw pairs for first few hisY
    let mut i = 0;
    for h in &hvals {
        if i >= 5 { break; }
        println!("hisY={:.9}({:08x})", f32::from_bits(*h), h);
        i += 1;
    }
}
