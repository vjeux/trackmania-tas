//! Fit ref = s*R*src + t between a reference item and a source crystal.
//! Usage: fittrans REF SRC [MATLINK-SUBSTR]
//! Matches verts by mm-rounded (ref*2 vs src), then reports per-axis scale,
//! translation, residual stats, and the best rigid rotation (if any).
use std::collections::BTreeMap;

fn key(p: &[f32; 3], s: f32) -> (i32, i32, i32) {
    (((p[0]*s) * 1000.0).round() as i32, ((p[1]*s) * 1000.0).round() as i32, ((p[2]*s) * 1000.0).round() as i32)
}

fn ref_pos(r: &str) -> Vec<[f32; 3]> {
    let data = std::fs::read(r).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for v in &s2.visuals {
        if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
            if let Some(st) = vis.stream() {
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let mapgeom::static_item::vstream::Elem::Float3(p) = e {
                        if d.name() == 0 {
                            out.extend_from_slice(p);
                            break;
                        }
                    }
                }
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rp = ref_pos(&a[1]);
    let srcdata = std::fs::read(&a[2]).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&srcdata).unwrap();
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    let filt = a.get(3).map(|s| s.to_string());
    let mut srcmap: BTreeMap<(i32, i32, i32), Vec<[f32; 3]>> = BTreeMap::new();
    for (fi, fa) in c.faces.iter().enumerate() {
        let _ = fi;
        if let Some(f) = &filt {
            let mi = fa.material as usize;
            let link = it.model.materials.get(mi).map(|m| m.name.clone()).unwrap_or_default();
            if !link.contains(f.as_str()) {
                continue;
            }
        }
        for vi in &fa.verts {
            let p = c.positions[*vi as usize];
            srcmap.entry(key(&p, 1.0)).or_default().push(p);
        }
    }
    // match
    let mut pairs: Vec<([f32; 3], [f32; 3])> = Vec::new();
    let mut unmatched = 0;
    for p in &rp {
        match srcmap.get(&key(p, 2.0)) {
            Some(v) => pairs.push((*p, v[0])),
            None => unmatched += 1,
        }
    }
    println!("ref verts {} matched {} unmatched {}", rp.len(), pairs.len(), unmatched);
    // per-axis scale + offset via least squares: ref = s*src + t
    for k in 0..3 {
        let n = pairs.len() as f64;
        let (sx, sy, sxx, sxy): (f64, f64, f64, f64) = pairs.iter().fold((0.0, 0.0, 0.0, 0.0), |(sx, sy, sxx, sxy), (r, s)| {
            (sx + s[k] as f64, sy + r[k] as f64, sxx + (s[k] as f64).powi(2), sxy + s[k] as f64 * r[k] as f64)
        });
        let s = (n * sxy - sx * sy) / (n * sxx - sx * sx);
        let t = (sy - s * sx) / n;
        let mut maxres = 0f64;
        let mut ss = 0f64;
        for (r, src) in &pairs {
            let res = (r[k] as f64 - (s * src[k] as f64 + t)).abs();
            maxres = maxres.max(res);
            ss += res * res;
        }
        println!("axis {k}: scale={s:.9} offset={t:.9} rms={:.3e} maxres={:.3e}", (ss / n).sqrt(), maxres);
    }
    // global similarity residual with s=0.5 t=0
    let mut maxres = 0f64;
    for (r, src) in &pairs {
        for k in 0..3 {
            maxres = maxres.max((r[k] as f64 - 0.5 * src[k] as f64).abs());
        }
    }
    println!("exact-half maxres={maxres:.3e}");
    // worst pairs vs exact half
    let mut worst: Vec<(f64, [f32; 3], [f32; 3])> = Vec::new();
    for (r, src) in &pairs {
        let mut d = 0f64;
        for k in 0..3 {
            d = d.max((r[k] as f64 - 0.5 * src[k] as f64).abs());
        }
        worst.push((d, *r, *src));
    }
    worst.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    for (d, r, src) in worst.iter().take(8) {
        println!("  res {d:.3e} ref [{:.6},{:.6},{:.6}] src [{:.6},{:.6},{:.6}]", r[0], r[1], r[2], src[0], src[1], src[2]);
    }
    // full affine least squares: ref = M*src + t (row-major 3x4)
    // solve normal equations via Gaussian elimination on 12 unknowns
    let n = pairs.len() as f64;
    // A^T A (12x12) and A^T b (12)
    let mut ata = vec![vec![0f64; 12]; 12];
    let mut atb = vec![0f64; 12];
    for (r, src) in &pairs {
        for row in 0..3 {
            let mut arow = [0f64; 12];
            arow[row * 4] = src[0] as f64;
            arow[row * 4 + 1] = src[1] as f64;
            arow[row * 4 + 2] = src[2] as f64;
            arow[row * 4 + 3] = 1.0;
            let b = r[row] as f64;
            for i in 0..12 {
                atb[i] += arow[i] * b;
                for j in 0..12 {
                    ata[i][j] += arow[i] * arow[j];
                }
            }
        }
    }
    // gaussian elimination with partial pivot
    let mut m = ata;
    let mut v = atb;
    for col in 0..12 {
        let mut piv = col;
        for r in col..12 {
            if m[r][col].abs() > m[piv][col].abs() {
                piv = r;
            }
        }
        m.swap(col, piv);
        v.swap(col, piv);
        let d = m[col][col];
        for r in col + 1..12 {
            let f = m[r][col] / d;
            for c in col..12 {
                m[r][c] -= f * m[col][c];
            }
            v[r] -= f * v[col];
        }
    }
    let mut x = vec![0f64; 12];
    for r in (0..12).rev() {
        let mut s = v[r];
        for c in r + 1..12 {
            s -= m[r][c] * x[c];
        }
        x[r] = s / m[r][r];
    }
    println!("affine rows (M|t):");
    for row in 0..3 {
        println!("  {:.9} {:.9} {:.9} | {:.9}", x[row*4], x[row*4+1], x[row*4+2], x[row*4+3]);
    }
    let mut mx = 0f64;
    let _ = (n, mx);
    for (r, src) in &pairs {
        for row in 0..3 {
            let pred = x[row*4] * src[0] as f64 + x[row*4+1] * src[1] as f64 + x[row*4+2] * src[2] as f64 + x[row*4+3];
            mx = mx.max((r[row] as f64 - pred).abs());
        }
    }
    println!("affine maxres={mx:.3e}");
}
