//! Positions shared across materials: compare normals. Usage: xmat FILE [N]
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
    let n: usize = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(10);
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut at: BTreeMap<[u32;3], Vec<(String,[f32;3])>> = BTreeMap::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
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
                for i in 0..pos.len() {
                    at.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push((stem.clone(), nrm[i]));
                }
            }
        }
    }
    let mut shown = 0;
    let (mut shared, mut samenorm) = (0, 0);
    for (k, v) in &at {
        let mut mats: BTreeMap<String, Vec<[f32;3]>> = BTreeMap::new();
        for (s, nn) in v { mats.entry(s.clone()).or_default().push(*nn); }
        if mats.len() < 2 { continue; }
        shared += 1;
        // compare first normals across materials
        let ns: Vec<[f32;3]> = mats.values().map(|x| x[0]).collect();
        let same = ns.iter().all(|x| (x[0]-ns[0][0]).abs()<0.002 && (x[1]-ns[0][1]).abs()<0.002 && (x[2]-ns[0][2]).abs()<0.002);
        if same { samenorm += 1; }
        if shown < n {
            let p = [f32::from_bits(k[0]), f32::from_bits(k[1]), f32::from_bits(k[2])];
            println!("pos=({:.5},{:.5},{:.5}) sameN={} {}", p[0], p[1], p[2], same as u8,
                mats.iter().map(|(s, x)| format!("{s}:({:.3},{:.3},{:.3})x{}", x[0][0], x[0][1], x[0][2], x.len())).collect::<Vec<_>>().join(" "));
            shown += 1;
        }
    }
    println!("shared_positions={shared} same_normal_across={samenorm}");
}
