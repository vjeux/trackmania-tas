//! Technics positions: stored=1 but face44=2+. Usage: techsplit HIS.ITEM
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
        if !mat.contains("Technics\"") && !mat.ends_with("\\Technics") { continue; }
        if mat.contains("Special") || mat.contains("Trim") { continue; }
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
                // per position: stored normals + face normals
                let mut bypos: BTreeMap<[u32; 3], Vec<([f32; 3], [f32; 3], f32)>> = BTreeMap::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt();
                    if l < 1e-15 { continue; }
                    let nn = [cr[0]/l, cr[1]/l, cr[2]/l];
                    let area = l/2.0;
                    for k in 0..3 {
                        bypos.entry([p[k][0].to_bits(), p[k][1].to_bits(), p[k][2].to_bits()]).or_default().push((nn, nrm[t[k] as usize], area));
                    }
                }
                let mut nshown = 0;
                for (pb, corners) in &bypos {
                    if corners.len() < 2 { continue; }
                    // stored distinct
                    let mut sd: Vec<[f32; 3]> = Vec::new();
                    for (_, sn, _) in corners {
                        if !sd.iter().any(|x| (x[0]-sn[0]).abs() < 1e-7 && (x[1]-sn[1]).abs() < 1e-7 && (x[2]-sn[2]).abs() < 1e-7) {
                            sd.push(*sn);
                        }
                    }
                    if sd.len() != 1 { continue; }
                    // face44 clusters
                    let cos_max = 44.0f32.to_radians().cos();
                    let mut ord: Vec<usize> = (0..corners.len()).collect();
                    ord.sort_by(|a, b| corners[*b].2.partial_cmp(&corners[*a].2).unwrap());
                    let mut seeds: Vec<[f32; 3]> = Vec::new();
                    for gi in ord {
                        let n = corners[gi].0;
                        if !seeds.iter().any(|s| s[0]*n[0]+s[1]*n[1]+s[2]*n[2] >= cos_max) {
                            seeds.push(n);
                        }
                    }
                    if seeds.len() > 1 && nshown < 10 {
                        nshown += 1;
                        let p = [f32::from_bits(pb[0]), f32::from_bits(pb[1]), f32::from_bits(pb[2])];
                        // min angle between seeds + areas
                        let mut mind = 2.0f32;
                        for x in 0..seeds.len() {
                            for y in (x+1)..seeds.len() {
                                mind = mind.min(seeds[x][0]*seeds[y][0]+seeds[x][1]*seeds[y][1]+seeds[x][2]*seeds[y][2]);
                            }
                        }
                        println!("pos=[{:.4},{:.4},{:.4}] ncorners={} seeds={} minseed_dot={:.4} stored=[{:.3},{:.3},{:.3}]",
                            p[0], p[1], p[2], corners.len(), seeds.len(), mind, sd[0][0], sd[0][1], sd[0][2]);
                    }
                }
                println!("total shown={nshown}");
                return;
            }
        }
    }
}
