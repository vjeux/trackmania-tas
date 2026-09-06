//! Isolate smoothing: cluster MY face normals (from MY positions) with rule R,
//! compare cluster COUNTS to his stored (his positions). If counts match at some R,
//! positions are not the issue. Already done via clustersim (TOTERR 164 at 45.6°).
//! This tool instead checks: do MY clusters match HIS clusters per-position?
//! Usage: normisolate HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // load both meshes' (pos, stored_normal) per visual, group by visual mat
    let load = |path: &str| -> BTreeMap<String, Vec<([f32; 3], [f32; 3])>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<String, Vec<([f32; 3], [f32; 3])>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or("?".into());
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm) = (Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    if pos.len() != nrm.len() { continue; }
                    let e = out.entry(mat).or_default();
                    for i in 0..pos.len() {
                        e.push((pos[i], nrm[i]));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    // per material, per position: his distinct normals vs mine; agreement rate
    for (mat, rc) in &r {
        let mc = match m.get(mat) {
            Some(v) => v,
            None => continue,
        };
        // his positions -> normals
        let mut rh: BTreeMap<[u32; 3], Vec<[f32; 3]>> = BTreeMap::new();
        for (p, n) in rc {
            rh.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push(*n);
        }
        // mine: for each of MY corners, find HIS position (nearest, exact bits? use mine bits -> his bits via proximity is complex)
        // simpler: compare DISTRIBUTIONS: histogram of (distinct normals per position)
        let mut mh: BTreeMap<[u32; 3], Vec<[f32; 3]>> = BTreeMap::new();
        for (p, n) in mc {
            mh.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push(*n);
        }
        let hist = |h: &BTreeMap<[u32; 3], Vec<[f32; 3]>>| -> BTreeMap<usize, usize> {
            let mut hh: BTreeMap<usize, usize> = BTreeMap::new();
            for (_, ns) in h {
                let mut d: Vec<[f32; 3]> = Vec::new();
                for n in ns {
                    if !d.iter().any(|x| (x[0]-n[0]).abs() < 1e-7 && (x[1]-n[1]).abs() < 1e-7 && (x[2]-n[2]).abs() < 1e-7) {
                        d.push(*n);
                    }
                }
                *hh.entry(d.len()).or_insert(0) += 1;
            }
            hh
        };
        println!("{mat}: his_n_per_pos={:?} mine_n_per_pos={:?}", hist(&rh), hist(&mh));
    }
    let _ = mk(&[0.0; 3]);
}
