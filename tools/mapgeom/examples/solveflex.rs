//! Solve ref = T*R*0.5*src for matched pairs. Usage: solveflex PAIRS.TSV
//! Rows: REF<TAB>ITEM<TAB>SRC<TAB>SCALE. Tries 8 yaw/mirror rotations with
//! centroid alignment; reports best recall. Verifies shape-match sources and
//! reveals the editor transform per item.
use std::collections::BTreeSet;

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    let q: f32 = std::env::var("KEYQ").ok().and_then(|v| v.parse().ok()).unwrap_or(1000.0);
    ((p[0] * q).round() as i32, ((p[1] * q).round() as i32), ((p[2] * q).round() as i32))
}

fn ref_tris(path: &str) -> Vec<[[f32; 3]; 3]> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos: Vec<[f32; 3]> = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let mapgeom::static_item::vstream::Elem::Float3(p) = e {
                        if d.name() == 0 {
                            pos = p.clone();
                            break;
                        }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    out.push([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                }
            }
        }
    }
    out
}

fn src_tris(path: &str) -> Vec<[[f32; 3]; 3]> {
    let data = std::fs::read(path).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&data).unwrap();
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    let mut out = Vec::new();
    for fa in &c.faces {
        let pts: Vec<[f32; 3]> = fa.verts.iter().map(|i| c.positions[*i as usize]).collect();
        if pts.len() == 3 {
            out.push([pts[0], pts[1], pts[2]]);
        } else {
            for i in 2..pts.len() {
                out.push([pts[1], pts[i], pts[(i + 1) % pts.len()]]);
            }
        }
    }
    out
}

fn centroid(tris: &[[[f32; 3]; 3]]) -> [f64; 3] {
    let mut s = [0.0; 3];
    let mut n = 0;
    for t in tris {
        for v in t {
            s[0] += v[0] as f64;
            s[1] += v[1] as f64;
            s[2] += v[2] as f64;
            n += 1;
        }
    }
    [s[0] / n as f64, s[1] / n as f64, s[2] / n as f64]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let pairs = std::fs::read_to_string(&a[1]).unwrap();
    for line in pairs.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let (refp, src, scale): (&str, &str, f32) = (f[0], f[2], f[3].parse().unwrap());
        let short = refp.rsplit('/').next().unwrap_or(refp);
        let rt = ref_tris(refp);
        let st = src_tris(src);
        let rset: BTreeSet<[(i32, i32, i32); 3]> = rt.iter().map(|t| { let mut k = [key(&t[0]), key(&t[1]), key(&t[2])]; k.sort(); k }).collect();
        let n = rt.len();
        let rc = centroid(&rt);
        // source scaled then centroid
        let sts: Vec<[[f32; 3]; 3]> = st.iter().map(|t| [[t[0][0]*scale, t[0][1]*scale, t[0][2]*scale], [t[1][0]*scale, t[1][1]*scale, t[1][2]*scale], [t[2][0]*scale, t[2][1]*scale, t[2][2]*scale]]).collect();
        let sc = centroid(&sts);
        let mut best = (0.0, String::new());
        // 24 proper cube rotations: axis permutation x signs with det +1
        let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        let psgn = [1.0, -1.0, -1.0, 1.0, 1.0, -1.0]; // permutation parity
        for (pi, perm) in perms.iter().enumerate() {
            for sx in [1.0, -1.0] {
                for sy in [1.0, -1.0] {
                    for sz in [1.0, -1.0] {
                        let _det = psgn[pi] * sx * sy * sz;
                        let s = [sx, sy, sz];
                        let xf = |p: &[f32; 3]| -> (i32, i32, i32) {
                            let v = [p[0] as f64 - sc[0], p[1] as f64 - sc[1], p[2] as f64 - sc[2]];
                            let w = [v[perm[0]] * s[0], v[perm[1]] * s[1], v[perm[2]] * s[2]];
                            // w is image in src axes; map components to ref x,y,z directly
                            key(&[(w[0] + rc[0]) as f32, (w[1] + rc[1]) as f32, (w[2] + rc[2]) as f32])
                        };
                        let mut inter = 0;
                        for t in &sts {
                            let mut k = [xf(&t[0]), xf(&t[1]), xf(&t[2])];
                            k.sort();
                            if rset.contains(&k) {
                                inter += 1;
                            }
                        }
                        let rec = inter as f64 / n.max(1) as f64;
                        if rec > best.0 {
                            best = (rec, format!("perm={pi} s=({sx},{sy},{sz})"));
                        }
                    }
                }
            }
        }
        println!("{short}: ntris={n} best recall={:.3} {}", best.0, best.1);
    }
}
