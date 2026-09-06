//! Transplant his uv1 onto my bake (matched tris), then report vert counts.
//! Usage: uvtransplant HIS.ITEM MINE.ITEM OUT.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    // NOTE: transplanting requires rewriting streams; instead just MEASURE:
    // for each of MY tris (matched), get his 3 uv1; check if my planar uv1 ever matches his
    // (quantify uv1 agreement rate). Simplified to agreement stats.
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<(Vec<[f32; 3]>, Vec<[f32; 2]>, String)> {
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
                    let (mut pos, mut uv1) = (Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                            _ => {}
                        }
                    }
                    if pos.len() != uv1.len() { continue; }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        out.push((vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]],
                                  vec![uv1[t[0] as usize], uv1[t[1] as usize], uv1[t[2] as usize]], mat.clone()));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _, _)) in m.iter().enumerate() {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let mut bymat: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (t, u, mat) in &r {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            let (mt, mu, _) = &m[v[0]];
            for ck in 0..3 {
                for mk2 in 0..3 {
                    if (mt[mk2][0]-t[ck][0]).abs() < 0.002 && (mt[mk2][1]-t[ck][1]).abs() < 0.002 && (mt[mk2][2]-t[ck][2]).abs() < 0.002 {
                        let e = bymat.entry(mat.rsplit('\\').next().unwrap_or(mat).to_string()).or_insert((0, 0));
                        e.0 += 1;
                        if (mu[mk2][0]-u[ck][0]).abs() < 1e-6 && (mu[mk2][1]-u[ck][1]).abs() < 1e-6 {
                            e.1 += 1;
                        }
                        break;
                    }
                }
            }
        }
    }
    for (mat, (tot, agr)) in &bymat {
        println!("{mat}: uv1_corners_matched={tot} agree={agr} ({:.1}%)", 100.0**agr as f32/MathMax(*tot,1) as f32);
    }
}
fn MathMax(a: usize, b: usize) -> usize { a.max(b) }
