//! Compare tri order his-vs-mine. Usage: triorder HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i64, i64, i64) {
    ((p[0]*100.0).round() as i64, (p[1]*100.0).round() as i64, (p[2]*100.0).round() as i64)
}
fn load(path: &str, substr: &str) -> Vec<[(i64, i64, i64); 3]> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(substr) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let mut k = [mk(&pos[t[0] as usize]), mk(&pos[t[1] as usize]), mk(&pos[t[2] as usize])];
                    k.sort();
                    out.push(k);
                }
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = load(&a[1], &a[3]);
    let m = load(&a[2], &a[3]);
    // longest common prefix + overall order correlation
    let mut pre = 0;
    while pre < r.len() && pre < m.len() && r[pre] == m[pre] { pre += 1; }
    // position of each my-tri in his order
    let mut hpos: BTreeMap<[(i64, i64, i64); 3], Vec<usize>> = BTreeMap::new();
    for (i, k) in r.iter().enumerate() {
        hpos.entry(*k).or_default().push(i);
    }
    let mut seq: Vec<usize> = Vec::new();
    let mut missing = 0;
    for k in &m {
        match hpos.get(k) {
            Some(v) => seq.push(v[0]),
            None => missing += 1,
        }
    }
    // count increasing runs (order preserved?)
    let mut runs = 1;
    for w in seq.windows(2) {
        if w[1] < w[0] { runs += 1; }
    }
    println!("{}: ntri his={} mine={} common_prefix={} missing={} runs={}", a[3], r.len(), m.len(), pre, missing, runs);
}
