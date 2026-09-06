//! Whose winding faces outward (agrees with stored)? Usage: flipdir HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[2]]
}
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<(Vec<[f32; 3]>, Vec<[f32; 3]>)> {
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
                    if pos.len() != nrm.len() { continue; }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        out.push((vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]],
                                  vec![nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]]));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let mut agree_his = 0;
    let mut agree_mine = 0;
    let mut total = 0;
    for (t, tn) in &r {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            let (u, un) = &m[v[0]];
            let mut perm = [0, 1, 2];
            'outer: for cand in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                let mut ok = true;
                for cc in 0..3 {
                    if (u[cand[cc]][0]-t[cc][0]).abs() > 0.002 || (u[cand[cc]][1]-t[cc][1]).abs() > 0.002 || (u[cand[cc]][2]-t[cc][2]).abs() > 0.002 {
                        ok = false; break;
                    }
                }
                if ok { perm = cand; break 'outer; }
            }
            let fnr = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let fnm = cross(sub(u[perm[1]], u[perm[0]]), sub(u[perm[2]], u[perm[0]]));
            let lr = (fnr[0]*fnr[0]+fnr[1]*fnr[1]+fnr[2]*fnr[2]).sqrt();
            let lm = (fnm[0]*fnm[0]+fnm[1]*fnm[1]+fnm[2]*fnm[2]).sqrt();
            if lr < 1e-15 || lm < 1e-15 { continue; }
            let dot = (fnr[0]*fnm[0]+fnr[1]*fnm[1]+fnr[2]*fnm[2])/(lr*lm);
            if dot < 0.0 {
                total += 1;
                // dot(face, stored_avg) for his and mine
                let avgh = [(tn[0][0]+tn[1][0]+tn[2][0])/3.0, (tn[0][1]+tn[1][1]+tn[2][1])/3.0, (tn[0][2]+tn[1][2]+tn[2][2])/3.0];
                let avgm = [(un[perm[0]][0]+un[perm[1]][0]+un[perm[2]][0])/3.0, (un[perm[0]][1]+un[perm[1]][1]+un[perm[2]][1])/3.0, (un[perm[0]][2]+un[perm[1]][2]+un[perm[2]][2])/3.0];
                let dh = (fnr[0]*avgh[0]+fnr[1]*avgh[1]+fnr[2]*avgh[2])/lr;
                let dm = (fnm[0]*avgm[0]+fnm[1]*avgm[1]+fnm[2]*avgm[2])/lm;
                if dh > 0.0 { agree_his += 1; }
                if dm > 0.0 { agree_mine += 1; }
            }
        }
    }
    println!("flips={total} his_face_agrees_stored={agree_his} mine_face_agrees_stored={agree_mine}");
}
