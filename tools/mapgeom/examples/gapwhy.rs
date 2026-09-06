//! Why gaps? For positions where his has more verts, what differs? Usage: gapwhy HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> BTreeMap<[u32; 3], Vec<(Vec<u32>, Vec<u32>)>> {
        // pos bits -> list of (normal+uv+uv1 bits, U+V bits) [U/V decoded bits; empty if no tangent]
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[u32; 3], Vec<(Vec<u32>, Vec<u32>)>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string(); if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm, mut uv, mut uv1, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                    let mut has_uv1 = false;
                    let mut has_tan = false;
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                            Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                            Elem::Word(w) if d.name() == 18 => { tu = w.iter().map(|v| dec(*v)).collect(); has_tan = true; }
                            Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        let mut base = vec![pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits(),
                                            nrm[i][0].to_bits(), nrm[i][1].to_bits(), nrm[i][2].to_bits(),
                                            uv[i][0].to_bits(), uv[i][1].to_bits()];
                        if has_uv1 && i < uv1.len() {
                            base.push(uv1[i][0].to_bits());
                            base.push(uv1[i][1].to_bits());
                        }
                        let mut tan = Vec::new();
                        if has_tan && i < tu.len() {
                            tan.push(tu[i][0].to_bits());
                            tan.push(tu[i][1].to_bits());
                            tan.push(tu[i][2].to_bits());
                            tan.push(tv[i][0].to_bits());
                            tan.push(tv[i][1].to_bits());
                            tan.push(tv[i][2].to_bits());
                        }
                        out.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push((base, tan));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    // match positions fuzzily (mmkey)? Positions differ 1% (exact bits fail). Use mmkey grouping.
    // (Simplify: exact-bit positions only; 1% miss is acceptable for pattern.)
    let (mut u_split, mut n_split, mut base_split, mut same) = (0, 0, 0, 0);
    for (pb, rverts) in &r {
        if let Some(mverts) = m.get(pb) {
            if rverts.len() <= mverts.len() { continue; }
            // his has more verts at this exact position. What differs?
            // Compare base keys (pos,n,uv,uv1) and tan keys.
            let mut rbase: BTreeSet2 = BTreeSet::new();
            let mut mbase: BTreeSet2 = BTreeSet::new();
            let mut rtan: BTreeSet2 = BTreeSet::new();
            let mut mtan: BTreeSet2 = BTreeSet::new();
            for (b, t) in rverts {
                rbase.insert(b.clone());
                rtan.insert(t.clone());
            }
            for (b, t) in mverts {
                mbase.insert(b.clone());
                mtan.insert(t.clone());
            }
            // (rbase/mbase should match if normals/uv/uv1 same; rtan/mtan differ if tangents split)
            // Count: extra his verts = rverts.len() - mverts.len(). Attribute to base (N/uv/uv1) or tan?
            // If rbase has more distinct than mbase -> base splits (N/uv/uv1 differ). Else tan splits.
            if rbase.len() > mbase.len() {
                base_split += rverts.len() - mverts.len();
            } else {
                u_split += rverts.len() - mverts.len();
            }
            let _ = (rbase, mbase, rtan, mtan, same, n_split);
        }
    }
    println!("{}: extra-his-verts base-driven={base_split} tan-driven={u_split}", a[3]);
    use std::collections::BTreeSet;
    type BTreeSet2 = BTreeSet<Vec<u32>>;
}
