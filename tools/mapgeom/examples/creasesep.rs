//! Welded vs split by (dihedral, edge length). Usage: creasesep HIS.ITEM SUBSTR
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
    // welded/split (dihedral, edgelen) samples
    let mut weld: Vec<(f32, f32)> = Vec::new();
    let mut split: Vec<(f32, f32)> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(&a[2]) { continue; }
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
                // edge -> tris
                let mut edge: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
                for (ti, t) in idx.chunks(3).enumerate() {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let a2 = t[k].min(t[(k+1)%3]);
                        let b = t[k].max(t[(k+1)%3]);
                        edge.entry((a2, b)).or_default().push(ti);
                    }
                }
                for (_, tris) in &edge {
                    if tris.len() != 2 { continue; }
                    // shared edge endpoints' positions
                    // find the two shared verts
                    let t0 = idx[tris[0]*3..tris[0]*3+3].to_vec();
                    let t1 = idx[tris[1]*3..tris[1]*3+3].to_vec();
                    let mut shared = Vec::new();
                    for a2 in &t0 {
                        for b in &t1 {
                            if a2 == b { shared.push(*a2); }
                        }
                    }
                    if shared.len() != 2 { continue; }
                    let p0 = pos[shared[0] as usize];
                    let p1 = pos[shared[1] as usize];
                    let el = ((p0[0]-p1[0]).powi(2)+(p0[1]-p1[1]).powi(2)+(p0[2]-p1[2]).powi(2)).sqrt();
                    // stored normals at shared verts (from each tri)
                    let mut dots = Vec::new();
                    for s in &shared {
                        // find corner in each tri
                        let c0 = t0.iter().position(|x| x == s).unwrap();
                        let c1 = t1.iter().position(|x| x == s).unwrap();
                        let n0 = nrm[t0[c0] as usize];
                        let n1 = nrm[t1[c1] as usize];
                        dots.push(n0[0]*n1[0]+n0[1]*n1[1]+n0[2]*n1[2]);
                    }
                    // welded if both shared verts agree
                    let welded = dots.iter().all(|d| *d > 0.9999);
                    // dihedral via face normals
                    let fn_of = |t: &[u32]| {
                        let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                        let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                        let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                        [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]]
                    };
                    let f0 = fn_of(&t0);
                    let f1 = fn_of(&t1);
                    let l0 = (f0[0]*f0[0]+f0[1]*f0[1]+f0[2]*f0[2]).sqrt();
                    let l1 = (f1[0]*f1[0]+f1[1]*f1[1]+f1[2]*f1[2]).sqrt();
                    if l0 < 1e-15 || l1 < 1e-15 { continue; }
                    let dihedral = ((f0[0]*f1[0]+f0[1]*f1[1]+f0[2]*f1[2])/(l0*l1)).clamp(-1.0, 1.0).acos().to_degrees();
                    if welded { weld.push((dihedral, el)); } else { split.push((dihedral, el)); }
                }
            }
        }
    }
    // report: welded max dihedral? split min dihedral? by edge length bands
    weld.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    split.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    if !weld.is_empty() && !split.is_empty() {
        println!("{}: welded_n={} max_dihedral={:.1} | split_n={} min_dihedral={:.1}",
            a[2], weld.len(), weld[weld.len()-1].0, split.len(), split[0].0);
        // edge length separation: welded min el? split max el?
        let mut wel: Vec<f32> = weld.iter().map(|x| x.1).collect();
        let mut spl: Vec<f32> = split.iter().map(|x| x.1).collect();
        wel.sort_by(|x, y| x.partial_cmp(y).unwrap());
        spl.sort_by(|x, y| x.partial_cmp(y).unwrap());
        println!("  welded edgelen p10={:.4} p50={:.4} p90={:.4}", wel[wel.len()/10], wel[wel.len()/2], wel[wel.len()*9/10]);
        println!("  split edgelen p10={:.4} p50={:.4} p90={:.4}", spl[spl.len()/10], spl[spl.len()/2], spl[spl.len()*9/10]);
    }
    let _ = Elem::Float3(vec![[0.0; 3]]);
}
