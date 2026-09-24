//! Index-fan smoothing test. Usage: indextest SRC.ITEM HIS.ITEM SUBSTR X Y Z
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-30);
    [v[0]/l, v[1]/l, v[2]/l]
}
fn qw(v: [f32; 3]) -> [i32; 3] {
    [(v[0].clamp(-1.0, 1.0) * 511.0) as i32, (v[1].clamp(-1.0, 1.0) * 511.0) as i32, (v[2].clamp(-1.0, 1.0) * 511.0) as i32]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: [f32;3] = [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()];
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let t = [f32::from_bits(0xbb018000), f32::from_bits(0xb7000000), f32::from_bits(0xbcbe2c00)];
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    // faces: (vertex indices, half-size positions, newell)
    let mut faces: Vec<(Vec<u32>, Vec<[f32; 3]>, [f32; 3])> = Vec::new();
    for (cr, vis, _col) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let idx: Vec<u32> = f.verts.clone();
            let pts: Vec<[f32; 3]> = idx.iter().map(|i| cr.positions[*i as usize]).map(|p| [p[0]*0.5+t[0], p[1]*0.5+t[1], p[2]*0.5+t[2]]).collect();
            let n = pts.len();
            let mut nv = [0f32; 3];
            for i in 0..n {
                let aa = pts[i];
                let b = pts[(i + 1) % n];
                nv[0] += (aa[1] - b[1]) * (aa[2] + b[2]);
                nv[1] += (aa[2] - b[2]) * (aa[0] + b[0]);
                nv[2] += (aa[0] - b[0]) * (aa[1] + b[1]);
            }
            faces.push((idx, pts, norm(nv)));
        }
    }
    // index -> faces
    let mut idx2faces: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (fi, (idx, _, _)) in faces.iter().enumerate() {
        for vi in idx {
            idx2faces.entry(*vi).or_default().push(fi);
        }
    }
    // q-corners: (face, corner pos index) with half-size pos near q
    println!("q-corners (face, vertidx):");
    let mut qidx: BTreeSet<u32> = BTreeSet::new();
    for (fi, (idx, pts, _)) in faces.iter().enumerate() {
        for (k, p) in pts.iter().enumerate() {
            let d = ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt();
            if d < 1e-6 {
                println!("  face{fi} corner{k} vertidx={} fan_size={}", idx[k], idx2faces[&idx[k]].len());
                qidx.insert(idx[k]);
            }
        }
    }
    // his words
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut hisv: Vec<[i32; 3]> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrmw) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrmw = w.clone(),
                        _ => {}
                    }
                }
                for i in 0..pos.len() {
                    let d = ((pos[i][0]-q[0]).powi(2)+(pos[i][1]-q[1]).powi(2)+(pos[i][2]-q[2]).powi(2)).sqrt();
                    if d < 2e-4 {
                        let w = nrmw[i];
                        hisv.push([((w & 0x3FF) as i32) << 22 >> 22, ((w >> 10 & 0x3FF) as i32) << 22 >> 22, ((w >> 20 & 0x3FF) as i32) << 22 >> 22]);
                    }
                }
            }
        }
    }
    println!("his words: {hisv:?}");
    // per index: uniform avg over fan newells
    for vi in &qidx {
        let fan = &idx2faces[vi];
        let mut acc = [0.0f64; 3];
        for fi in fan { acc[0] += faces[*fi].2[0] as f64; acc[1] += faces[*fi].2[1] as f64; acc[2] += faces[*fi].2[2] as f64; }
        let nn = fan.len() as f64;
        let cand = norm([(acc[0]/nn) as f32, (acc[1]/nn) as f32, (acc[2]/nn) as f32]);
        let w = qw(cand);
        let hit = hisv.iter().position(|h| *h == w).map(|i| format!("HISv{i}")).unwrap_or("none".to_string());
        println!("index{vi} fan={fan:?} words={w:?} -> {hit}");
    }
    let _ = BTreeMap::<u32,u32>::new();
    let _ = dec;
}
