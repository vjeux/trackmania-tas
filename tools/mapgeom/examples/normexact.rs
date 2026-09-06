//! Normal bit-exact %. Usage: normexact HIS.ITEM MINE.ITEM
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
    let load = |path: &str| -> BTreeMap<[(i32, i32, i32); 3], Vec<u32>> {
        // mmkey -> list of (normal, drawable?) Actually need per-corner matching (tri+corner).
        // Simplified: tri mmkey -> sorted normal bits (multiset per tri).
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[(i32, i32, i32); 3], Vec<u32>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
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
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        let mut k = [mk(&pos[t[0] as usize]), mk(&pos[t[1] as usize]), mk(&pos[t[2] as usize])];
                        k.sort();
                        // normal words (raw, for bit-exact compare need raw words not decoded; approximate via decoded bits)
                        // (Use decoded float bits; dec injective so fine.)
                        let mut ns: Vec<u32> = vec![
                            nrm[t[0] as usize][0].to_bits() ^ nrm[t[0] as usize][1].to_bits().wrapping_mul(31) ^ nrm[t[0] as usize][2].to_bits().wrapping_mul(37),
                            nrm[t[1] as usize][0].to_bits() ^ nrm[t[1] as usize][1].to_bits().wrapping_mul(31) ^ nrm[t[1] as usize][2].to_bits().wrapping_mul(37),
                            nrm[t[2] as usize][0].to_bits() ^ nrm[t[2] as usize][1].to_bits().wrapping_mul(31) ^ nrm[t[2] as usize][2].to_bits().wrapping_mul(37),
                        ];
                        ns.sort();
                        out.entry(k).or_default().extend(ns);
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let (mut hit, mut tot) = (0, 0);
    for (k, rns) in &r {
        if let Some(mns) = m.get(k) {
            // multiset compare (sorted)
            let mut a2 = rns.clone();
            let mut b2 = mns.clone();
            a2.sort();
            b2.sort();
            for (x, y) in a2.iter().zip(b2.iter()) {
                tot += 1;
                if x == y { hit += 1; }
            }
        }
    }
    println!("normal-word multiset match {hit}/{tot} ({:.1}%)", 100.0*hit as f32/tot as f32);
}
