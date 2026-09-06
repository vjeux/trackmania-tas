//! Map baked tri indices to source quad IDs. Usage: quadsrc SRC.ITEM BAKED.ITEM SUBSTR I0 I1 ...
use std::collections::BTreeMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let want: Vec<usize> = a[4..].iter().map(|x| x.parse().unwrap()).collect();
    // source quads: half-size corner positions per quad (per material? need material match; approximate: all quads)
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let t = [f32::from_bits(0xbb018000), f32::from_bits(0xb7000000), f32::from_bits(0xbcbe2c00)];
    // quad id -> set of half-size mmkeys (sorted triple)
    let mut quadtris: BTreeMap<usize, Vec<[(i32,i32,i32);3]>> = BTreeMap::new();
    // need per-layer crystals like bake's geometry_layers
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    let mut qid = 0;
    let mut quadcorners: BTreeMap<usize, Vec<[f32;3]>> = BTreeMap::new(); // qid -> half-size corners in order
    for (cr, vis, _col) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let pts: Vec<[f32;3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).map(|p| [p[0]*0.5+t[0], p[1]*0.5+t[1], p[2]*0.5+t[2]]).collect();
            // fan tris mmkeys
            let mut tris = Vec::new();
            if pts.len() == 3 {
                let mut k = [((pts[0][0]*1000.0).round() as i32, (pts[0][1]*1000.0).round() as i32, (pts[0][2]*1000.0).round() as i32),
                    ((pts[1][0]*1000.0).round() as i32, (pts[1][1]*1000.0).round() as i32, (pts[1][2]*1000.0).round() as i32),
                    ((pts[2][0]*1000.0).round() as i32, (pts[2][1]*1000.0).round() as i32, (pts[2][2]*1000.0).round() as i32)];
                k.sort();
                tris.push(k);
            } else {
                for i in 2..pts.len() {
                    let mut k = [((pts[1][0]*1000.0).round() as i32, (pts[1][1]*1000.0).round() as i32, (pts[1][2]*1000.0).round() as i32),
                        ((pts[i][0]*1000.0).round() as i32, (pts[i][1]*1000.0).round() as i32, (pts[i][2]*1000.0).round() as i32),
                        ((pts[(i+1)%pts.len()][0]*1000.0).round() as i32, (pts[(i+1)%pts.len()][1]*1000.0).round() as i32, (pts[(i+1)%pts.len()][2]*1000.0).round() as i32)];
                    k.sort();
                    tris.push(k);
                }
            }
            quadtris.insert(qid, tris);
            quadcorners.insert(qid, pts);
            qid += 1;
        }
    }
    // baked tris mmkeys
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    use mapgeom::static_item::vstream::Elem;
    let mut btris: Vec<[(i32,i32,i32);3]> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
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
                for tt in idx.chunks(3) {
                    if tt.len() < 3 { continue; }
                    let ps = [pos[tt[0] as usize], pos[tt[1] as usize], pos[tt[2] as usize]];
                    let mut k = [((ps[0][0]*1000.0).round() as i32, (ps[0][1]*1000.0).round() as i32, (ps[0][2]*1000.0).round() as i32),
                        ((ps[1][0]*1000.0).round() as i32, (ps[1][1]*1000.0).round() as i32, (ps[1][2]*1000.0).round() as i32),
                        ((ps[2][0]*1000.0).round() as i32, (ps[2][1]*1000.0).round() as i32, (ps[2][2]*1000.0).round() as i32)];
                    k.sort();
                    btris.push(k);
                }
            }
        }
    }
    // invert: mmkey -> qid
    let mut mm2q: BTreeMap<[(i32,i32,i32);3], usize> = BTreeMap::new();
    for (qid, ts) in &quadtris {
        for k in ts { mm2q.insert(*k, *qid); }
    }
    for i in &want {
        match mm2q.get(&btris[*i]) {
            Some(qid) => {
                let qc = &quadcorners[qid];
                println!("baked tri{i} -> source quad{qid} ({} verts)", qc.len());
            }
            None => println!("baked tri{i} -> NO MATCH"),
        }
    }
}
