//! Fit secondary transform to outlier verts. Usage: outlfit HIS.ITEM SRCFILE
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
    let d = [-0.001976f32, -0.000008, -0.023215];
    let t = [-0.0019760131836f64, -0.0000076293945312, -0.023214340210];
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<[[f64; 3]; 3]>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let pts: Vec<[f64; 3]> = f.verts.iter().map(|i| {
                let p = cr.positions[*i as usize];
                [p[0] as f64, p[1] as f64, p[2] as f64]
            }).collect();
            if pts.len() < 3 { continue; }
            let mut tris: Vec<[[f64; 3]; 3]> = Vec::new();
            if pts.len() == 3 { tris.push([pts[0], pts[1], pts[2]]); }
            else { for i in 2..pts.len() { tris.push([pts[1], pts[i], pts[(i+1) % pts.len()]]); } }
            for tri in &tris {
                let h = [((tri[0][0]*0.5) as f32 + d[0]), ((tri[0][1]*0.5) as f32 + d[1]), ((tri[0][2]*0.5) as f32 + d[2])];
                let h1 = [((tri[1][0]*0.5) as f32 + d[0]), ((tri[1][1]*0.5) as f32 + d[1]), ((tri[1][2]*0.5) as f32 + d[2])];
                let h2 = [((tri[2][0]*0.5) as f32 + d[0]), ((tri[2][1]*0.5) as f32 + d[1]), ((tri[2][2]*0.5) as f32 + d[2])];
                let mut k = [mk(&h), mk(&h1), mk(&h2)];
                k.sort();
                mmap.entry(k).or_default().push(*tri);
            }
        }
    }
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // collect outlier (src, his) pairs
    let mut pairs: Vec<([f64; 3], [f64; 3], String)> = Vec::new();
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
                                let ex = [((s[perm[cc]][0]*0.5) as f32 + d[0]), ((s[perm[cc]][1]*0.5) as f32 + d[1]), ((s[perm[cc]][2]*0.5) as f32 + d[2])];
                                let dd = ((ex[0]-p[cc][0]).powi(2)+(ex[1]-p[cc][1]).powi(2)+(ex[2]-p[cc][2]).powi(2)).sqrt();
                                if dd > 0.0005 { ok = false; break; }
                            }
                            if ok {
                                for cc in 0..3 {
                                    let mut miss = false;
                                    for ax in 0..3 {
                                        if ((s[perm[cc]][ax]*0.5 + t[ax]) as f32).to_bits() != p[cc][ax].to_bits() { miss = true; }
                                    }
                                    if miss {
                                        pairs.push((s[perm[cc]], [p[cc][0] as f64, p[cc][1] as f64, p[cc][2] as f64], short.clone()));
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("outlier pairs={}", pairs.len());
    // residual vectors (his - formula) stats per axis + spatial extent
    for ax in 0..3 {
        let mut r: Vec<f64> = pairs.iter().map(|(s, h, _)| h[ax] - (s[ax]*0.5 + t[ax])).collect();
        r.sort_by(|x, y| x.partial_cmp(y).unwrap());
        println!("ax{ax} resid_um: min={:.2} p10={:.2} p50={:.2} p90={:.2} max={:.2}",
            r[0]*1e6, r[r.len()/10]*1e6, r[r.len()/2]*1e6, r[r.len()*9/10]*1e6, r[r.len()-1]*1e6);
    }
    // spatial bounding box of outliers vs whole block
    let (mut mn, mut mx): ([f64; 3], [f64; 3]) = ([1e9; 3], [-1e9; 3]);
    let mut bymat: BTreeMap<String, usize> = BTreeMap::new();
    for (_, h, m) in &pairs {
        for ax in 0..3 {
            mn[ax] = mn[ax].min(h[ax]);
            mx[ax] = mx[ax].max(h[ax]);
        }
        *bymat.entry(m.clone()).or_insert(0) += 1;
    }
    println!("outlier bbox: [{:.3},{:.3},{:.3}]-[{:.3},{:.3},{:.3}]", mn[0], mn[1], mn[2], mx[0], mx[1], mx[2]);
    println!("by material: {bymat:?}");
}
