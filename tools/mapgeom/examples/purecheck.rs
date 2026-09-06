//! Pure rule-diffs: same incident-face count, different vert count. Usage: purecheck HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // per material per position: (vert count, incident tri count)
    let load = |path: &str| -> BTreeMap<(String,[u32;3]), (usize, usize)> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut vc: BTreeMap<(String,[u32;3]), usize> = BTreeMap::new();
        let mut tc: BTreeMap<(String,[u32;3]), usize> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let mut pos = Vec::new();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 { pos = p.clone(); }
                        }
                    }
                    for q in &pos { *vc.entry((stem.clone(), [q[0].to_bits(), q[1].to_bits(), q[2].to_bits()])).or_insert(0) += 1; }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        for k in 0..3 {
                            let q = pos[t[k] as usize];
                            *tc.entry((stem.clone(), [q[0].to_bits(), q[1].to_bits(), q[2].to_bits()])).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
        vc.into_iter().map(|(k, v)| (k.clone(), (v, *tc.get(&k).unwrap_or(&0)))).collect()
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let (mut pure_over, mut pure_under, mut conf) = (0, 0, 0);
    for (k, (mv, mt)) in &m {
        if let Some((rv, rt)) = r.get(k) {
            if mt == rt {
                if *mv > *rv { pure_over += 1; }
                else if *mv < *rv { pure_under += 1; }
            } else { conf += 1; }
        }
    }
    // also his positions missing from mine entirely (deviated away)
    let mut his_only = 0;
    for k in r.keys() { if !m.contains_key(k) { his_only += 1; } }
    let mut mine_only = 0;
    for k in m.keys() { if !r.contains_key(k) { mine_only += 1; } }
    println!("pure_over(mine>his,same faces)={pure_over} pure_under={pure_under} confounded(diff faces)={conf} his_only_pos={his_only} mine_only_pos={mine_only}");
}
