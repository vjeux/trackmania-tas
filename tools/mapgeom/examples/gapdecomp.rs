//! Decompose bake gaps: normal vs tangent vs other. Usage: gapdecomp HIS.ITEM MINE.ITEM SUBSTR
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // his verts (full key) vs my verts (full key): match by full key, count unmatched both sides.
    let load = |path: &str| -> BTreeMap<Vec<u32>, usize> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<Vec<u32>, usize> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if !mat.contains(&a[3]) { continue; }
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
                        let mut k = vec![pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits(),
                                         nrm[i][0].to_bits(), nrm[i][1].to_bits(), nrm[i][2].to_bits(),
                                         uv[i][0].to_bits(), uv[i][1].to_bits()];
                        if has_uv1 && i < uv1.len() {
                            k.push(uv1[i][0].to_bits());
                            k.push(uv1[i][1].to_bits());
                        }
                        if has_tan && i < tu.len() {
                            k.push(tu[i][0].to_bits());
                            k.push(tu[i][1].to_bits());
                            k.push(tu[i][2].to_bits());
                            k.push(tv[i][0].to_bits());
                            k.push(tv[i][1].to_bits());
                            k.push(tv[i][2].to_bits());
                        }
                        *out.entry(k).or_insert(0) += 1;
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    // his-only keys vs mine-only keys (by full key). Then strip U/V and compare (to isolate tangent contribution).
    let strip_tan = |k: &Vec<u32>| -> Vec<u32> {
        // key layout: pos(3) n(3) uv(2) [uv1(2)] [U(3) V(3)]. Strip last 6 if present (layouts with tangents have uv1? Full/White have uv1+tangent (2+6=8 extra). Decal has tangent but no uv1? (uv(2)+U(3)+V(3)=8, no uv1).
        // Heuristic: if len>10, strip last 6.
        if k.len() > 10 {
            k[..k.len()-6].to_vec()
        } else {
            k.clone()
        }
    };
    let mut r_notan: BTreeMap<Vec<u32>, usize> = BTreeMap::new();
    let mut m_notan: BTreeMap<Vec<u32>, usize> = BTreeMap::new();
    for (k, c) in &r {
        *r_notan.entry(strip_tan(k)).or_insert(0) += c;
    }
    for (k, c) in &m {
        *m_notan.entry(strip_tan(k)).or_insert(0) += c;
    }
    // his-only and mine-only (full key)
    let mut his_only = 0;
    let mut mine_only = 0;
    for (k, c) in &r {
        let mc = m.get(k).unwrap_or(&0);
        if *c > *mc { his_only += c - mc; }
    }
    for (k, c) in &m {
        let rc = r.get(k).unwrap_or(&0);
        if *c > *rc { mine_only += c - rc; }
    }
    // no-tangent key diffs
    let mut his_only_nt = 0;
    let mut mine_only_nt = 0;
    for (k, c) in &r_notan {
        let mc = m_notan.get(k).unwrap_or(&0);
        if *c > *mc { his_only_nt += c - mc; }
    }
    for (k, c) in &m_notan {
        let rc = r_notan.get(k).unwrap_or(&0);
        if *c > *rc { mine_only_nt += c - rc; }
    }
    println!("{}: fullkey his_only={his_only} mine_only={mine_only} | notan his_only={his_only_nt} mine_only={mine_only_nt}", a[3]);
    let _ = BTreeSet::<u8>::new();
}
