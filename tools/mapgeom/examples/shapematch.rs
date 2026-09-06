//! Source discovery by triangle SHAPE (edge lengths): invariant to rotation,
//! translation, mirror. Usage: shapematch NADEO.zip REFDIR
//! For each ref: shape-multiset jaccard vs every Nadeo item (ref shapes x2).
use std::collections::{BTreeMap, BTreeSet};

fn shape(t: &[[f32; 3]; 3]) -> (i64, i64, i64) {
    let d = |a: &[f32; 3], b: &[f32; 3]| {
        (((a[0]-b[0]).powi(2) + (a[1]-b[1]).powi(2) + (a[2]-b[2]).powi(2)).sqrt() * 2000.0).round() as i64
    };
    let mut e = [d(&t[0], &t[1]), d(&t[1], &t[2]), d(&t[2], &t[0])];
    e.sort();
    (e[0], e[1], e[2])
}

fn ref_shapes(path: &str) -> BTreeMap<(i64, i64, i64), usize> {
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
                    let mut tri2 = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    // scale x2 for source comparison: shapes scale linearly
                    for v in tri2.iter_mut() {
                        v[0] *= 2.0; v[1] *= 2.0; v[2] *= 2.0;
                    }
                    *out.entry(shape(&tri2)).or_default() += 1;
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
    let mut nadeo: BTreeMap<String, BTreeMap<(i64, i64, i64), usize>> = BTreeMap::new();
    for (name, bytes) in &files {
        if !name.ends_with(".Item.Gbx") {
            continue;
        }
        let it = mapgeom::crystal::ItemCrystal::open(bytes).unwrap();
        let layer = it.model.first_geometry().unwrap();
        let c = layer.kind.crystal().unwrap();
        let mut m = BTreeMap::new();
        for fa in &c.faces {
            let pts: Vec<[f32; 3]> = fa.verts.iter().map(|i| c.positions[*i as usize]).collect();
            if pts.len() == 3 {
                *m.entry(shape(&[pts[0], pts[1], pts[2]])).or_default() += 1;
            } else {
                for i in 2..pts.len() {
                    *m.entry(shape(&[pts[1], pts[i], pts[(i + 1) % pts.len()]])).or_default() += 1;
                }
            }
        }
        nadeo.insert(name.clone(), m);
    }
    println!("nadeo: {}", nadeo.len());
    let mut refs: Vec<String> = std::fs::read_dir(refdir).unwrap().flatten().map(|e| e.path().to_string_lossy().to_string()).filter(|p| p.ends_with(".Item.Gbx")).collect();
    refs.sort();
    for r in &refs {
        let short = r.rsplit('/').next().unwrap();
        let rs = ref_shapes(r);
        let rn: usize = rs.values().sum();
        let rset: BTreeSet<(i64, i64, i64)> = rs.keys().cloned().collect();
        let mut scored: Vec<(f64, String)> = Vec::new();
        for (name, ns) in &nadeo {
            let nset: BTreeSet<(i64, i64, i64)> = ns.keys().cloned().collect();
            let inter = rset.intersection(&nset).count();
            let uni = rset.union(&nset).count().max(1);
            let j = inter as f64 / uni as f64;
            if j > 0.1 {
                scored.push((j, name.clone()));
            }
        }
        scored.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
        println!("{short}: ntris={rn}");
        for (j, name) in scored.iter().take(5) {
            println!("    shape-jaccard={j:.3} {name}");
        }
    }
}
