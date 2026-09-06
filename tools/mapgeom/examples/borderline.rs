//! Borderline dihedral (35-60°): weld/split vs edge length. Usage: borderline HIS.ITEM
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
    // per position: face normals (from tris) + stored; for pairs with dihedral 35-60°, record (welded?, edgelen?)
    // edgelen: need shared edge; approximate by min face area? Use position-pair distance? Simplest: face areas.
    let mut weld_area: Vec<f32> = Vec::new();
    let mut split_area: Vec<f32> = Vec::new();
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
                // per tri: face normal + area
                let mut trin: Vec<[f32; 3]> = Vec::new();
                let mut tria: Vec<f32> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { trin.push([0.0; 3]); tria.push(0.0); continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt();
                    if l < 1e-15 { trin.push([0.0; 3]); tria.push(0.0); continue; }
                    trin.push([cr[0]/l, cr[1]/l, cr[2]/l]);
                    tria.push(l/2.0);
                }
                // per position: pairs
                let mut bypos: BTreeMap<[u32; 3], Vec<(usize, [f32; 3])>> = BTreeMap::new();
                for (ti, t) in idx.chunks(3).enumerate() {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let p = pos[t[k] as usize];
                        bypos.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push((ti, nrm[t[k] as usize]));
                    }
                }
                for (_, corners) in &bypos {
                    if corners.len() < 2 { continue; }
                    for x in 0..corners.len() {
                        for y in (x+1)..corners.len() {
                            let n0 = trin[corners[x].0];
                            let n1 = trin[corners[y].0];
                            if n0 == [0.0; 3] || n1 == [0.0; 3] { continue; }
                            let dih = (n0[0]*n1[0]+n0[1]*n1[1]+n0[2]*n1[2]).clamp(-1.0, 1.0).acos().to_degrees();
                            if dih < 35.0 || dih > 60.0 { continue; }
                            let s0 = corners[x].1;
                            let s1 = corners[y].1;
                            let welded = (s0[0]-s1[0]).abs() < 1e-7 && (s0[1]-s1[1]).abs() < 1e-7 && (s0[2]-s1[2]).abs() < 1e-7;
                            let a2 = tria[corners[x].0].min(tria[corners[y].0]);
                            if welded { weld_area.push(a2); } else { split_area.push(a2); }
                        }
                    }
                }
            }
        }
    }
    weld_area.sort_by(|x, y| x.partial_cmp(y).unwrap());
    split_area.sort_by(|x, y| x.partial_cmp(y).unwrap());
    if !weld_area.is_empty() && !split_area.is_empty() {
        println!("borderline35-60: welded_n={} area p10={:.2e} p50={:.2e} p90={:.2e}", weld_area.len(), weld_area[weld_area.len()/10], weld_area[weld_area.len()/2], weld_area[weld_area.len()*9/10]);
        println!("borderline35-60: split_n={} area p10={:.2e} p50={:.2e} p90={:.2e}", split_area.len(), split_area[split_area.len()/10], split_area[split_area.len()/2], split_area[split_area.len()*9/10]);
    }
}
