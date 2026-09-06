//! His lightmap atlas, chart by chart: charts = connected components of tris
//! over shared vertex indices; per chart tri count, world area, uv1 area,
//! uv1 bbox, the least-squares affine fit local-2D -> uv1 (scales, rotation,
//! shear), and the atlas-level facts a generator must reproduce (uniform
//! scale?, margins, bbox overlaps, gutters).
//! Usage: uv1charts FILE STEM [max_charts_to_list]
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;

fn load(path: &str, stem_want: &str) -> Option<(Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<[u32; 3]>)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != stem_want {
            continue;
        }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv1) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let tris: Vec<[u32; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0], t[1], t[2]]).collect();
                return Some((pos, uv1, tris));
            }
        }
    }
    None
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt().max(1e-30);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// Solve least squares for x in A x = b (A: n x m, small m) via normal equations.
fn lsq(a: &[Vec<f64>], b: &[f64], m: usize) -> Vec<f64> {
    let mut ata = vec![vec![0.0f64; m]; m];
    let mut atb = vec![0.0f64; m];
    for (row, &bv) in a.iter().zip(b.iter()) {
        for i in 0..m {
            atb[i] += row[i] * bv;
            for j in 0..m {
                ata[i][j] += row[i] * row[j];
            }
        }
    }
    // Gaussian elimination
    let mut mtx: Vec<Vec<f64>> = ata.iter().enumerate().map(|(i, r)| {
        let mut v = r.clone();
        v.push(atb[i]);
        v
    }).collect();
    for c in 0..m {
        let mut p = c;
        for r in c + 1..m {
            if mtx[r][c].abs() > mtx[p][c].abs() {
                p = r;
            }
        }
        mtx.swap(c, p);
        let d = mtx[c][c];
        if d.abs() < 1e-18 {
            continue;
        }
        for r in 0..m {
            if r != c {
                let f = mtx[r][c] / d;
                for k in c..=m {
                    let v = mtx[c][k];
                    mtx[r][k] -= f * v;
                }
            }
        }
    }
    (0..m).map(|i| if mtx[i][i].abs() < 1e-18 { 0.0 } else { mtx[i][m] / mtx[i][i] }).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let max_list: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(20);
    let Some((pos, uv1, tris)) = load(&a[1], &a[2]) else {
        println!("{}: not found", a[2]);
        return;
    };
    if uv1.is_empty() {
        println!("{}: no uv1", a[2]);
        return;
    }
    // union-find over vertex indices through tris
    let n = pos.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, x: usize) -> usize {
        let mut r = x;
        while p[r] != r {
            r = p[r];
        }
        let mut c = x;
        while p[c] != r {
            let nx = p[c];
            p[c] = r;
            c = nx;
        }
        r
    }
    for t in &tris {
        let r0 = find(&mut parent, t[0] as usize);
        let r1 = find(&mut parent, t[1] as usize);
        let r2 = find(&mut parent, t[2] as usize);
        parent[r1] = r0;
        let r0b = find(&mut parent, r0);
        parent[r2] = r0b;
    }
    let mut charts: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (ti, t) in tris.iter().enumerate() {
        let r = find(&mut parent, t[0] as usize);
        charts.entry(r).or_default().push(ti);
    }
    println!("{}: verts={} tris={} charts={}", a[2], n, tris.len(), charts.len());
    struct C {
        ntri: usize,
        area3: f64,
        area2: f64,
        bb: [[f32; 2]; 2],
        su: f64,
        sv: f64,
        rot_deg: f64,
        shear_deg: f64,
        planarity: f64,
        nverts: usize,
    }
    let mut rows: Vec<C> = Vec::new();
    for (_, tl) in &charts {
        let mut area3 = 0.0f64;
        let mut area2 = 0.0f64;
        let mut nacc = [0.0f32; 3];
        let mut bb = [[f32::MAX; 2], [f32::MIN; 2]];
        let mut vset: std::collections::BTreeSet<u32> = Default::default();
        for &ti in tl {
            let t = tris[ti];
            let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
            let c = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            let a3 = dot(c, c).sqrt() as f64 / 2.0;
            area3 += a3;
            for k in 0..3 {
                nacc[k] += c[k];
            }
            let q = [uv1[t[0] as usize], uv1[t[1] as usize], uv1[t[2] as usize]];
            area2 += (((q[1][0] - q[0][0]) * (q[2][1] - q[0][1]) - (q[2][0] - q[0][0]) * (q[1][1] - q[0][1])) as f64 / 2.0).abs();
            for v in t {
                vset.insert(v);
                let u = uv1[v as usize];
                bb[0][0] = bb[0][0].min(u[0]);
                bb[0][1] = bb[0][1].min(u[1]);
                bb[1][0] = bb[1][0].max(u[0]);
                bb[1][1] = bb[1][1].max(u[1]);
            }
        }
        // planarity: |sum of area-weighted normals| / sum of areas
        let planarity = (dot(nacc, nacc).sqrt() as f64 / 2.0) / area3.max(1e-12);
        // local 2D basis on the mean plane
        let nz = norm(nacc);
        let ax = if nz[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let ex = norm(cross(ax, nz));
        let ey = cross(nz, ex);
        let mut arows: Vec<Vec<f64>> = Vec::new();
        let (mut bu, mut bv): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
        for &v in &vset {
            let p = pos[v as usize];
            let x = dot(p, ex) as f64;
            let y = dot(p, ey) as f64;
            arows.push(vec![x, y, 1.0]);
            bu.push(uv1[v as usize][0] as f64);
            bv.push(uv1[v as usize][1] as f64);
        }
        let cu = lsq(&arows, &bu, 3);
        let cv = lsq(&arows, &bv, 3);
        // Jacobian J = [[cu0, cu1],[cv0, cv1]]: singular values ~ scales
        let (j00, j01, j10, j11) = (cu[0], cu[1], cv[0], cv[1]);
        let e = (j00 + j11) / 2.0;
        let f = (j00 - j11) / 2.0;
        let g = (j10 + j01) / 2.0;
        let h = (j10 - j01) / 2.0;
        let q = (e * e + h * h).sqrt();
        let r = (f * f + g * g).sqrt();
        let (su, sv) = (q + r, q - r);
        let a1 = g.atan2(f);
        let a2 = h.atan2(e);
        let rot = ((a2 + a1) / 2.0).to_degrees();
        let shear = ((a2 - a1) / 2.0).to_degrees();
        rows.push(C { ntri: tl.len(), area3, area2, bb, su, sv, rot_deg: rot, shear_deg: shear, planarity, nverts: vset.len() });
    }
    rows.sort_by(|x, y| y.area2.partial_cmp(&x.area2).unwrap());
    let tot3: f64 = rows.iter().map(|r| r.area3).sum();
    let tot2: f64 = rows.iter().map(|r| r.area2).sum();
    println!("total world area={:.3} uv1 area={:.4} (coverage of unit square) ; global density sqrt(uv1/world)={:.5}", tot3, tot2, (tot2 / tot3).sqrt());
    // atlas bbox
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for r in &rows {
        lo[0] = lo[0].min(r.bb[0][0]);
        lo[1] = lo[1].min(r.bb[0][1]);
        hi[0] = hi[0].max(r.bb[1][0]);
        hi[1] = hi[1].max(r.bb[1][1]);
    }
    println!("atlas bbox [{:.4},{:.4}]x[{:.4},{:.4}]", lo[0], hi[0], lo[1], hi[1]);
    // per-chart density spread
    let mut dens: Vec<f64> = rows.iter().filter(|r| r.area3 > 1e-6).map(|r| (r.area2 / r.area3).sqrt()).collect();
    dens.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if !dens.is_empty() {
        println!("per-chart density sqrt(uv1/world): min={:.5} p10={:.5} median={:.5} p90={:.5} max={:.5}", dens[0], dens[dens.len() / 10], dens[dens.len() / 2], dens[dens.len() * 9 / 10], dens[dens.len() - 1]);
    }
    // bbox overlaps between charts
    let mut overlaps = 0;
    let mut min_gap = f32::MAX;
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            let (a, b) = (&rows[i].bb, &rows[j].bb);
            let ox = a[1][0].min(b[1][0]) - a[0][0].max(b[0][0]);
            let oy = a[1][1].min(b[1][1]) - a[0][1].max(b[0][1]);
            if ox > 1e-6 && oy > 1e-6 {
                overlaps += 1;
            } else {
                // gap along the separating axis
                let gap = if ox <= 1e-6 && oy <= 1e-6 { (-ox).max(-oy) } else if ox <= 1e-6 { -ox } else { -oy };
                if gap >= 0.0 {
                    min_gap = min_gap.min(gap);
                }
            }
        }
    }
    println!("chart bbox overlapping pairs={} min bbox gap={:.5}", overlaps, min_gap);
    // rotation histogram (mod 90)
    let mut rot_hist: BTreeMap<i32, usize> = BTreeMap::new();
    for r in &rows {
        let m = ((r.rot_deg % 90.0) + 90.0) % 90.0;
        *rot_hist.entry((m / 5.0).round() as i32 * 5).or_insert(0) += 1;
    }
    println!("rotation mod 90 histogram (5deg bins): {:?}", rot_hist);
    println!("{:>5} {:>6} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7} {:>6}  bbox", "ntri", "nverts", "area3", "area2", "su", "sv", "rot", "shear", "planar");
    for r in rows.iter().take(max_list) {
        println!(
            "{:>5} {:>6} {:>8.4} {:>8.5} {:>7.5} {:>7.5} {:>7.1} {:>7.1} {:>6.3}  [{:.4},{:.4}]x[{:.4},{:.4}]",
            r.ntri, r.nverts, r.area3, r.area2, r.su, r.sv, r.rot_deg, r.shear_deg, r.planarity, r.bb[0][0], r.bb[1][0], r.bb[0][1], r.bb[1][1]
        );
    }
}
