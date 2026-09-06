//! Fit 12-param affine his = A*src + t over tight pairs; report residual collapse.
//! Usage: affinefit HIS.ITEM SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
// solve 12x12 normal equations via Gaussian elimination
fn solve(mut a: [[f64; 12]; 12], mut b: [f64; 12]) -> [f64; 12] {
    for col in 0..12 {
        let mut piv = col;
        for r in (col+1)..12 {
            if a[r][col].abs() > a[piv][col].abs() { piv = r; }
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let d = a[col][col];
        for r in 0..12 {
            if r == col { continue; }
            let f = a[r][col] / d;
            for c in col..12 { a[r][c] -= f * a[col][c]; }
            b[r] -= f * b[col];
        }
    }
    let mut x = [0.0; 12];
    for i in 0..12 { x[i] = b[i] / a[i][i]; }
    x
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let d = [-0.001976f32, -0.000008, -0.023215];
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
    let mut pairs: Vec<([f64; 3], [f64; 3])> = Vec::new();
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
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
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
                                for cc in 0..3 {
                                    pairs.push((s[perm[cc]], [p[cc][0] as f64, p[cc][1] as f64, p[cc][2] as f64]));
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("pairs={}", pairs.len());
    // normal equations for his_row = A_row . src + t_row (12 unknowns)
    let mut nmat = [[0.0f64; 12]; 12];
    let mut nvec = [0.0f64; 12];
    for (s, h) in &pairs {
        for row in 0..3 {
            let mut j = [0.0f64; 12];
            j[row*4] = s[0]; j[row*4+1] = s[1]; j[row*4+2] = s[2]; j[row*4+3] = 1.0;
            for x in 0..12 {
                nvec[x] += j[x] * h[row];
                for y in 0..12 { nmat[x][y] += j[x] * j[y]; }
            }
        }
    }
    let x = solve(nmat, nvec);
    println!("A=[{:.9},{:.9},{:.9};{:.9},{:.9},{:.9};{:.9},{:.9},{:.9}] t=[{:.9},{:.9},{:.9}]",
        x[0], x[1], x[2], x[4], x[5], x[6], x[8], x[9], x[10], x[3], x[7], x[11]);
    let mut res: Vec<f64> = Vec::new();
    for (s, h) in &pairs {
        for row in 0..3 {
            let pred = x[row*4]*s[0] + x[row*4+1]*s[1] + x[row*4+2]*s[2] + x[row*4+3];
            res.push((h[row]-pred).abs()*1e6);
        }
    }
    res.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("affine residuals_um n={} p50={:.2} p99={:.2} max={:.2}", res.len(), res[res.len()/2], res[res.len()*99/100], res[res.len()-1]);
}
