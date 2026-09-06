//! Analyze his uv1 structure. Usage: uvanal FILE SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
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
        if !mat.contains(&a[2]) { continue; }
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
                // range + distinct count + continuity: group by pos, count distinct uv1
                let mut bypos: BTreeMap<[u32; 3], Vec<[f32; 2]>> = BTreeMap::new();
                let (mut mnx, mut mxx, mut mny, mut mxy) = (1e9f32, -1e9f32, 1e9f32, -1e9f32);
                for i in 0..pos.len() {
                    mnx = mnx.min(uv1[i][0]); mxx = mxx.max(uv1[i][0]);
                    mny = mny.min(uv1[i][1]); mxy = mxy.max(uv1[i][1]);
                    bypos.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(uv1[i]);
                }
                let mut split_pos = 0;
                for (_, v) in &bypos {
                    let mut dist = 0;
                    for (i, u) in v.iter().enumerate() {
                        if v[..i].iter().all(|x| (x[0]-u[0]).abs() > 1e-7 || (x[1]-u[1]).abs() > 1e-7) {
                            dist += 1;
                        }
                    }
                    if dist > 1 { split_pos += 1; }
                }
                println!("{}: corners={} uv1_range=[{:.4},{:.4}]x[{:.4},{:.4}] pos_with_2+_uv1={}/{}",
                    mat.rsplit('\\').next().unwrap_or(&mat), pos.len(), mnx, mxx, mny, mxy, split_pos, bypos.len());
                // sample values
                for i in (0..pos.len()).step_by(pos.len()/8+1).take(8) {
                    println!("  pos=[{:.3},{:.3},{:.3}] uv1=[{:.5},{:.5}]", pos[i][0], pos[i][1], pos[i][2], uv1[i][0], uv1[i][1]);
                }
                return;
            }
        }
    }
}
