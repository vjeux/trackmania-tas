//! dot(face_normal, avg_corner_normal) distribution. Usage: facedot FILE
use mapgeom::static_item::vstream::Elem;
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
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut dots: Vec<f32> = Vec::new();
    let mut neg = 0;
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
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let n = [nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]];
                    let fn_ = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                    let l = (fn_[0]*fn_[0]+fn_[1]*fn_[1]+fn_[2]*fn_[2]).sqrt();
                    if l < 1e-15 { continue; }
                    let avg = [(n[0][0]+n[1][0]+n[2][0])/3.0, (n[0][1]+n[1][1]+n[2][1])/3.0, (n[0][2]+n[1][2]+n[2][2])/3.0];
                    let dot = (fn_[0]*avg[0]+fn_[1]*avg[1]+fn_[2]*avg[2])/l;
                    dots.push(dot);
                    if dot < 0.0 { neg += 1; }
                }
            }
        }
    }
    dots.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("n={} neg_dots={neg} min={:.4} p1={:.4} p50={:.4}", dots.len(), dots[0], dots[dots.len()/100], dots[dots.len()/2]);
}
