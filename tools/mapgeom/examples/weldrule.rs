//! Per-position: face-normal span, uv/det unanimity, his vs my vert counts. Usage: weldrule HIS.ITEM MINE.ITEM SUBSTR
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
    // my tris (face order): pos + uv per corner, face normals recomputed
    let load_tris = |path: &str| -> Vec<[[f32;3];3]> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if !mat.contains(&a[3]) { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut uv) = (Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                            _ => {}
                        }
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        out.push([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                    }
                }
            }
        }
        out
    };
    // vert counts per position (exact bits)
    let load_vc = |path: &str| -> BTreeMap<[u32;3], usize> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if !mat.contains(&a[3]) { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 {
                                for q in p { *out.entry([q[0].to_bits(), q[1].to_bits(), q[2].to_bits()]).or_insert(0) += 1; }
                            }
                        }
                    }
                }
            }
        }
        out
    };
    // his UVs per position (for unanimity + det we need his tri uvs; use HIS file tris)
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // per position: incident face normals (from his tris), corner uvs, tri dets
    let mut pos_faces: BTreeMap<[u32;3], Vec<[f32;3]>> = BTreeMap::new();
    let mut pos_uvs: BTreeMap<[u32;3], Vec<[f32;2]>> = BTreeMap::new();
    let mut pos_dets: BTreeMap<[u32;3], Vec<f32>> = BTreeMap::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(&a[3]) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                if pos.is_empty() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let u = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    let fn_ = [cr[0]/l, cr[1]/l, cr[2]/l];
                    let det = (u[1][0]-u[0][0])*(u[2][1]-u[0][1])-(u[2][0]-u[0][0])*(u[1][1]-u[0][1]);
                    for k in 0..3 {
                        let key = [p[k][0].to_bits(), p[k][1].to_bits(), p[k][2].to_bits()];
                        pos_faces.entry(key).or_default().push(fn_);
                        pos_uvs.entry(key).or_default().push(u[k]);
                        pos_dets.entry(key).or_default().push(det);
                    }
                }
            }
        }
    }
    let hvc = load_vc(&a[1]);
    let mvc = load_vc(&a[2]);
    let _ = load_tris;
    // report positions with span>30deg: span, uv_unanimous, det_unanimous, his_n, my_n
    println!("pos(span>30deg): span_deg uv1 det1 his_n my_n");
    let mut n = 0;
    for (key, fns) in &pos_faces {
        if fns.len() < 2 { continue; }
        let mut mind = 1.0f32;
        for i in 0..fns.len() {
            for j in (i+1)..fns.len() {
                let d = fns[i][0]*fns[j][0]+fns[i][1]*fns[j][1]+fns[i][2]*fns[j][2];
                mind = mind.min(d);
            }
        }
        let span = mind.clamp(-1.0,1.0).acos().to_degrees();
        if span > 30.0 {
            let uvs = &pos_uvs[key];
            let uv1 = uvs.iter().all(|u| u[0].to_bits()==uvs[0][0].to_bits() && u[1].to_bits()==uvs[0][1].to_bits());
            let dets = &pos_dets[key];
            let det1 = dets.iter().all(|d| (*d>=0.0)==(dets[0]>=0.0));
            let hn = hvc.get(key).unwrap_or(&0);
            let mn = mvc.get(key).unwrap_or(&0);
            println!("  {:x}{:x}{:x} span={:.1} uv1={} det1={} his={} mine={}", key[0],key[1],key[2], span, uv1 as u8, det1 as u8, hn, mn);
            n += 1;
            if n > 40 { break; }
        }
    }
    let _ = dec;
}
