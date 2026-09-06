//! Total uv1 area + world area. Usage: uv1area FILE
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let (mut auv1, mut aworld) = (0.0f64, 0.0f64);
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv1) = (Vec::new(), Vec::new());
                let mut has_uv1 = false;
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                        _ => {}
                    }
                }
                if !has_uv1 { continue; }
                if pos.len() != uv1.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let u = [uv1[t[0] as usize], uv1[t[1] as usize], uv1[t[2] as usize]];
                    let au = ((u[1][0]-u[0][0])*(u[2][1]-u[0][1])-(u[2][0]-u[0][0])*(u[1][1]-u[0][1])).abs()/2.0;
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let aw = ((cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]) as f64).sqrt()/2.0;
                    auv1 += au as f64;
                    aworld += aw;
                }
            }
        }
    }
    println!("{}: uv1_area={:.4} world_area={:.2}", a[1].rsplit('/').next().unwrap(), auv1, aworld);
}
