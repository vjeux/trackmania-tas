//! Are stubborn positions T-junctions? Usage: tjunction HIS.ITEM
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
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.ends_with("\\Technics") { continue; }
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
                // all segment endpoints (edges) for T-junction test
                let mut edges: Vec<([f32; 3], [f32; 3])> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        edges.push((pos[t[k] as usize], pos[t[(k+1)%3] as usize]));
                    }
                }
                // stubborn positions: stored=1 but seed52=2+
                let mut bypos: BTreeMap<[u32; 3], Vec<[f32; 3]>> = BTreeMap::new();
                // face normals per tri
                let mut trin: Vec<[f32; 3]> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { triu_push(&mut trin, [0.0; 3]); continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt();
                    if l < 1e-15 { triu_push(&mut trin, [0.0; 3]); continue; }
                    triu_push(&mut trin, [cr[0]/l, cr[1]/l, cr[2]/l]);
                }
                for (ti, t) in idx.chunks(3).enumerate() {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let p = pos[t[k] as usize];
                        bypos.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push(trin[ti]);
                    }
                }
                let cos_max = 52.0f32.to_radians().cos();
                let mut stubborn = 0;
                let mut tjunct = 0;
                for (pb, fns) in &bypos {
                    if fns.len() < 2 { continue; }
                    // stored distinct?
                    // (skip stored check; use seed52 split as proxy for stubborn)
                    let mut ord: Vec<usize> = (0..fns.len()).collect();
                    // seed52: just count seeds (no area here; use first-seen order... approximate with single-linkage at 52)
                    // simpler: max dihedral > 52?
                    let mut maxdih = 0.0f32;
                    for x in 0..fns.len() {
                        for y in (x+1)..fns.len() {
                            let dot = (fns[x][0]*fns[y][0]+fns[x][1]*fns[y][1]+fns[x][2]*fns[y][2]).clamp(-1.0, 1.0);
                            maxdih = maxdih.max(dot.acos().to_degrees());
                        }
                    }
                    if maxdih < 52.0 { continue; }
                    // check stored=1 (need stored normals; approximate: skip, assume stubborn if maxdih>52 and ... hmm need stored)
                    stubborn += 1;
                    // T-junction? position lies on an edge (not at endpoints)
                    let p = [f32::from_bits(pb[0]), f32::from_bits(pb[1]), f32::from_bits(pb[2])];
                    let mut is_tj = false;
                    for (e0, e1) in &edges {
                        // distance from p to segment e0-e1, excluding endpoints
                        let d = dist_seg(p, *e0, *e1);
                        if d < 0.0001 {
                            // check not an endpoint
                            let d0 = ((p[0]-e0[0]).powi(2)+(p[1]-e0[1]).powi(2)+(p[2]-e0[2]).powi(2)).sqrt();
                            let d1 = ((p[0]-e1[0]).powi(2)+(p[1]-e1[1]).powi(2)+(p[2]-e1[2]).powi(2)).sqrt();
                            if d0 > 0.0002 && d1 > 0.0002 {
                                is_tj = true;
                                break;
                            }
                        }
                    }
                    if is_tj { tjunct += 1; }
                }
                println!("Technics positions maxdih>52: {stubborn}, of_which_Tjunction={tjunct}");
                return;
            }
        }
    }
}
fn triu_push(v: &mut Vec<[f32; 3]>, x: [f32; 3]) { v.push(x); }
fn dist_seg(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let ab = [b[0]-a[0], b[1]-a[1], b[2]-a[2]];
    let t = ((p[0]-a[0])*ab[0]+(p[1]-a[1])*ab[1]+(p[2]-a[2])*ab[2] / (ab[0]*ab[0]+ab[1]*ab[1]+ab[2]*ab[2]).max(1e-30)).clamp(0.0, 1.0);
    let q = [a[0]+t*ab[0], a[1]+t*ab[1], a[2]+t*ab[2]];
    ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt()
}
