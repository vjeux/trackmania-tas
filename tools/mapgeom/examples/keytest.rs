//! Is normal determined by (pos,uv,uv1)? Compare |pos+uv+uv1| vs |pos+n+uv+uv1|.
//! Usage: keytest FILE
use std::collections::BTreeSet;
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
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or("?".into());
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm, mut uv, mut uv1) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                let mut has_uv1 = false;
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut s_nouv: BTreeSet<Vec<u32>> = BTreeSet::new();
                let mut s_ouv: BTreeSet<Vec<u32>> = BTreeSet::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let vi2 = t[k] as usize;
                        let p = [pos[vi2][0].to_bits(), pos[vi2][1].to_bits(), pos[vi2][2].to_bits()];
                        let n = [nrm[vi2][0].to_bits(), nrm[vi2][1].to_bits(), nrm[vi2][2].to_bits()];
                        let u = [uv[vi2][0].to_bits(), uv[vi2][1].to_bits()];
                        let u1 = if has_uv1 && vi2 < uv1.len() { [uv1[vi2][0].to_bits(), uv1[vi2][1].to_bits()] } else { [0, 0] };
                        let mut k1 = Vec::new();
                        k1.extend(p);
                        k1.extend(n);
                        k1.extend(u);
                        k1.extend(u1);
                        s_nouv.insert(k1);
                        let mut k2 = Vec::new();
                        k2.extend(p);
                        k2.extend(u);
                        k2.extend(u1);
                        s_ouv.insert(k2);
                    }
                }
                println!("{}: stream={} |pos+n+uv+uv1|={} |pos+uv+uv1|={} diff={}",
                    mat, pos.len(), s_nouv.len(), s_ouv.len(), s_nouv.len() as i64 - s_ouv.len() as i64);
            }
        }
    }
}
