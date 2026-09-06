use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;
fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn load(path: &str) -> Vec<([[f32; 3]; 3], [[f32; 3]; 3])> {
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
                let (mut pos, mut nrm) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    out.push(([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], [nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]]));
                }
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [key(&[t[0][0]+d[0], t[0][1]+d[1], t[0][2]+d[2]]), key(&[t[1][0]+d[0], t[1][1]+d[1], t[1][2]+d[2]]), key(&[t[2][0]+d[0], t[2][1]+d[1], t[2][2]+d[2]])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let (mut n, mut dot_sum, mut up_r, mut up_m) = (0, 0.0, 0, 0);
    for (t, rn) in &r {
        let mut k = [key(&t[0]), key(&t[1]), key(&t[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            let (_, mn) = &m[v[0]];
            // match corners by position
            for ck in 0..3 {
                let mut found = None;
                for mk in 0..3 {
                    // mine corner mk at ref corner ck (with translation)
                    found = Some(mk);
                    break;
                }
                let _ = found;
            }
            // average normal dot over the tri (corner order may differ; use means)
            let mr: [f32; 3] = [rn.iter().map(|n| n[0]).sum::<f32>() / 3.0, rn.iter().map(|n| n[1]).sum::<f32>() / 3.0, rn.iter().map(|n| n[2]).sum::<f32>() / 3.0];
            let mm: [f32; 3] = [mn.iter().map(|n| n[0]).sum::<f32>() / 3.0, mn.iter().map(|n| n[1]).sum::<f32>() / 3.0, mn.iter().map(|n| n[2]).sum::<f32>() / 3.0];
            n += 1;
            dot_sum += mr[0]*mm[0] + mr[1]*mm[1] + mr[2]*mm[2];
            if mr[1] > 0.9 { up_r += 1; }
            if mm[1] > 0.9 { up_m += 1; }
        }
    }
    println!("matched tris={n} mean_normal_dot={:.3} ref_up={} mine_up={}", dot_sum / n.max(1) as f32, up_r, up_m);
}
