//! Face-normal agreement his-vs-mine per matched tri. Usage: facenormcmp HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[2]]
}
fn load(path: &str) -> Vec<(Vec<[f32; 3]>, String)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
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
                    out.push((vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], mat.clone()));
                }
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let mut angs: Vec<f32> = Vec::new();
    let mut bad: Vec<(f32, String, f32)> = Vec::new(); // angle, mat, area
    for (t, mat) in &r {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            let (u, _) = &m[v[0]];
            // pair corners
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
            if lr < 1e-12 || lm < 1e-12 { continue; }
            let dot = (fnr[0]*fnm[0]+fnr[1]*fnm[1]+fnr[2]*fnm[2])/(lr*lm);
            let ang = dot.clamp(-1.0, 1.0).acos().to_degrees();
            angs.push(ang);
            if ang > 1.0 {
                bad.push((ang, mat.rsplit('\\').next().unwrap_or(mat).to_string(), lr/2.0));
            }
        }
    }
    angs.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("n={} face-normal-angle p50={:.4} p99={:.4} max={:.2}", angs.len(), angs[angs.len()/2], angs[angs.len()*99/100], angs[angs.len()-1]);
    bad.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    for (ang, mat, area) in bad.iter().take(10) {
        println!("  {ang:.1}deg {mat} area={area:.2e}");
    }
}
