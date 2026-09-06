//! Compare position formulas by bit-exact rate. Usage: formulacmp HIS.ITEM SRCFILE
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
    // src corners (unscaled f64) keyed by halved+translated mmkey
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
    let t = [-0.001976013f64, -0.000007629, -0.023214340];
    let mut pairs: Vec<([f64; 3], [f32; 3])> = Vec::new();
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
                                for cc in 0..3 { pairs.push((s[perm[cc]], p[cc])); }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("pairs={}", pairs.len());
    // A: (0.5s + t) f64->f32 ; C: ((s+2t)/2) f64->f32 ; D: f32(s + f32(2t))/2 f32 ; E: f32(0.5)*s+t in f32
    let names = ["A:0.5s+t(f64)", "C:(s+2t)/2(f64)", "D:f32(s+2t)/2", "E:f32mul+f32add"];
    for (fi, _name) in names.iter().enumerate() {
        let mut hit = [0usize; 3];
        for (s, h) in &pairs {
            for ax in 0..3 {
                let v: f32 = match fi {
                    0 => (s[ax]*0.5 + t[ax]) as f32,
                    1 => ((s[ax] + 2.0*t[ax])/2.0) as f32,
                    2 => {
                        let tt = (2.0*t[ax]) as f32;
                        (((s[ax] as f32) + tt) as f32) / 2.0
                    }
                    _ => {
                        let u = (s[ax] as f32) * 0.5f32;
                        u + t[ax] as f32
                    }
                };
                if v.to_bits() == h[ax].to_bits() { hit[ax] += 1; }
            }
        }
        println!("{_name}: x={}/{} y={}/{} z={}/{}", hit[0], pairs.len(), hit[1], pairs.len(), hit[2], pairs.len());
    }
}
