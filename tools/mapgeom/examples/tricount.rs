//! Find sources by triangle count, then test 8 orientations (4 yaws x mirror).
//! Usage: tricount NADEO.zip REFDIR
use std::collections::{BTreeMap, BTreeSet};

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0] * 1000.0).round() as i32, ((p[1] * 1000.0).round() as i32), ((p[2] * 1000.0).round() as i32))
}

fn ref_tris(path: &str) -> BTreeMap<[(i32, i32, i32); 3], usize> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = BTreeMap::new();
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
                    let mut k = [key(&pos[t[0] as usize]), key(&pos[t[1] as usize]), key(&pos[t[2] as usize])];
                    k.sort();
                    *out.entry(k).or_default() += 1;
                }
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let zip = std::fs::read(&a[1]).expect("nadeo zip");
    let refdir = &a[2];
    let files = mapgeom::embedded::unzip(&zip).expect("zip");
    // tri counts per nadeo item (fan v1-diagonal to match bake)
    let mut counts: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut meshes: BTreeMap<String, Vec<[[f32; 3]; 3]>> = BTreeMap::new();
    for (name, bytes) in &files {
        if !name.ends_with(".Item.Gbx") {
            continue;
        }
        let (_, mesh) = mapgeom::crystal::decode_template(bytes);
        // need faces: re-decode with faces
        let it = mapgeom::crystal::ItemCrystal::open(bytes).unwrap();
        let layer = it.model.first_geometry().unwrap();
        let c = layer.kind.crystal().unwrap();
        let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
        for fa in &c.faces {
            let pts: Vec<[f32; 3]> = fa.verts.iter().map(|i| c.positions[*i as usize]).collect();
            if pts.len() == 3 {
                tris.push([pts[0], pts[1], pts[2]]);
            } else {
                for i in 2..pts.len() {
                    tris.push([pts[1], pts[i], pts[(i + 1) % pts.len()]]);
                }
            }
        }
        counts.entry(tris.len()).or_default().push(name.clone());
        meshes.insert(name.clone(), tris);
    }
    let mut refs: Vec<String> = std::fs::read_dir(refdir).unwrap().flatten().map(|e| e.path().to_string_lossy().to_string()).filter(|p| p.ends_with(".Item.Gbx")).collect();
    refs.sort();
    for r in &refs {
        let short = r.rsplit('/').next().unwrap();
        let rt = ref_tris(r);
        let n: usize = rt.values().sum();
        // candidates with tri count within +-2
        let mut cands = Vec::new();
        for (c, names) in &counts {
            if (*c as i64 - n as i64).abs() <= 2 {
                for nm in names {
                    cands.push(nm.clone());
                }
            }
        }
        if cands.is_empty() {
            println!("{short}: ntris={n} NO count-match");
            continue;
        }
        if std::env::var("DUMPCANDS").is_ok() {
            println!("{short}: ntris={n} candidates:");
            for nm in &cands { println!("    {nm}"); }
            continue;
        }
        // 8 orientations: rot k*90 about Y (in FULL-size coords: ref*2), optional x-mirror
        let rset: BTreeSet<[(i32, i32, i32); 3]> = rt.keys().cloned().collect();
        let mut best = (0.0, String::new(), String::new());
        for nm in &cands {
            // source tris scaled 0.5
            let st: Vec<[[f32; 3]; 3]> = meshes[nm.as_str()].iter().map(|t| [[t[0][0]*0.5, t[0][1]*0.5, t[0][2]*0.5], [t[1][0]*0.5, t[1][1]*0.5, t[1][2]*0.5], [t[2][0]*0.5, t[2][1]*0.5, t[2][2]*0.5]]).collect();
            for mir in [false, true] {
                for rot in 0..4 {
                    let xf = |p: &[f32; 3]| -> (i32, i32, i32) {
                        let (mut x, z) = (p[0], p[2]);
                        if mir {
                            x = -x;
                        }
                        let (x, z) = match rot {
                            0 => (x, z),
                            1 => (z, -x),
                            2 => (-x, -z),
                            _ => (-z, x),
                        };
                        key(&[x, p[1], z])
                    };
                    let mut inter = 0;
                    for t in &st {
                        let mut k = [xf(&t[0]), xf(&t[1]), xf(&t[2])];
                        k.sort();
                        if rset.contains(&k) {
                            inter += 1;
                        }
                    }
                    let rec = inter as f64 / n.max(1) as f64;
                    if rec > best.0 {
                        best = (rec, nm.clone(), format!("mir={mir} rot={rot}"));
                    }
                }
            }
        }
        println!("{short}: ntris={n} ncands={} best recall={:.3} {} {}", cands.len(), best.0, best.1, best.2);
    }
}
