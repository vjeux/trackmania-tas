//! Quad-mate agreement for flipped tris. Usage: quadmate HIS.ITEM MINE.ITEM SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
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
    // my tris with stored normals + dots
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
    // flipped keys (mine disagrees, his agrees) -- need his dots; approximate: face-opposite + stored match
    // simpler: use flipdir logic inline is complex; instead find MY tris with dot<0, then check quad-mate
    // my dots:
    let mut myneg: Vec<usize> = Vec::new();
    for (i, (t, tn)) in m.iter().enumerate() {
        let fn_ = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        let l = (fn_[0]*fn_[0]+fn_[1]*fn_[1]+fn_[2]*fn_[2]).sqrt();
        if l < 1e-15 { continue; }
        let avg = [(tn[0][0]+tn[1][0]+tn[2][0])/3.0, (tn[0][1]+tn[1][1]+tn[2][1])/3.0, (tn[0][2]+tn[1][2]+tn[2][2])/3.0];
        if (fn_[0]*avg[0]+fn_[1]*avg[1]+fn_[2]*avg[2])/l < 0.0 {
            myneg.push(i);
        }
    }
    println!("my neg-dot tris={}", myneg.len());
    // source quads: for each myneg tri, find its quad + mate tri, check mate dot
    let data = std::fs::read(&a[3]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let d = [-0.001976f32, -0.000008, -0.023215];
    // quad tri pairs (my fan): key -> mate key
    let mut mate: BTreeMap<[(i32, i32, i32); 3], [(i32, i32, i32); 3]> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.verts.len() != 4 { continue; }
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            let mut ks = Vec::new();
            for tri in [[pts[1], pts[2], pts[3]], [pts[1], pts[3], pts[0]]] {
                let h = [[tri[0][0]*0.5+d[0], tri[0][1]*0.5+d[1], tri[0][2]*0.5+d[2]],
                         [tri[1][0]*0.5+d[0], tri[1][1]*0.5+d[1], tri[1][2]*0.5+d[2]],
                         [tri[2][0]*0.5+d[0], tri[2][1]*0.5+d[1], tri[2][2]*0.5+d[2]]];
                let mut k = [mk(&h[0]), mk(&h[1]), mk(&h[2])];
                k.sort();
                ks.push(k);
            }
            // wait: my fan is (v0,v1,v2),(v0,v2,v3) = indices (0,1,2),(0,2,3); fix:
            ks.clear();
            for tri in [[pts[0], pts[1], pts[2]], [pts[0], pts[2], pts[3]]] {
                let h = [[tri[0][0]*0.5+d[0], tri[0][1]*0.5+d[1], tri[0][2]*0.5+d[2]],
                         [tri[1][0]*0.5+d[0], tri[1][1]*0.5+d[1], tri[1][2]*0.5+d[2]],
                         [tri[2][0]*0.5+d[0], tri[2][1]*0.5+d[1], tri[2][2]*0.5+d[2]]];
                let mut k = [mk(&h[0]), mk(&h[1]), mk(&h[2])];
                k.sort();
                ks.push(k);
            }
            mate.insert(ks[0], ks[1]);
            mate.insert(ks[1], ks[0]);
        }
    }
    // myneg tris: mate dot distribution (need mate tri index in m; map key->m index)
    let mut key2idx: BTreeMap<[(i32, i32, i32); 3], usize> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        key2idx.entry(k).or_insert(i);
    }
    let (mut mate_pos, mut mate_neg, mut mate_none) = (0, 0, 0);
    for i in &myneg {
        let (t, tn) = &m[*i];
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        match mate.get(&k).and_then(|mk2| key2idx.get(mk2)) {
            Some(j) => {
                let (u, un) = &m[*j];
                let fn_ = cross(sub(u[1], u[0]), sub(u[2], u[0]));
                let l = (fn_[0]*fn_[0]+fn_[1]*fn_[1]+fn_[2]*fn_[2]).sqrt();
                if l < 1e-15 { mate_none += 1; continue; }
                let avg = [(un[0][0]+un[1][0]+un[2][0])/3.0, (un[0][1]+un[1][1]+un[2][1])/3.0, (un[0][2]+un[1][2]+un[2][2])/3.0];
                if (fn_[0]*avg[0]+fn_[1]*avg[1]+fn_[2]*avg[2])/l < 0.0 { mate_neg += 1; } else { mate_pos += 1; }
            }
            None => mate_none += 1,
        }
    }
    println!("myneg mate_pos={mate_pos} mate_neg={mate_neg} mate_none={mate_none}");
    let _ = r;
}
