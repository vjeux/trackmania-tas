//! Print exact uv bits + U words for a his tri. Usage: hisuv HIS STEM TRIIDX
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let want: usize = a[3].parse().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if mat.rsplit('\\').next().unwrap_or(&mat) != a[2] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv, mut tu): (Vec<[f32;3]>, Vec<[f32;2]>, Vec<u32>) = (vec![], vec![], vec![]);
                // raw U words: re-read decl 18 as u32 words
                let mut tuw: Vec<u32> = vec![];
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Word(w) if d.name() == 18 => tuw = w.clone(),
                        _ => {}
                    }
                }
                let _ = tu;
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let t = [idx[want*3], idx[want*3+1], idx[want*3+2]];
                for k in 0..3 {
                    let p = pos[t[k] as usize]; let u = uv[t[k] as usize];
                    let w = tuw[t[k] as usize];
                    let dx = ((w & 0x3FF) as i32) << 22 >> 22;
                    let dy = ((w >> 10 & 0x3FF) as i32) << 22 >> 22;
                    let dz = ((w >> 20 & 0x3FF) as i32) << 22 >> 22;
                    println!("c{k} pos=({:08x},{:08x},{:08x}) uv=({:08x},{:08x}) Uword={:08x} U=({:+.6},{:+.6},{:+.6})",
                        p[0].to_bits(), p[1].to_bits(), p[2].to_bits(), u[0].to_bits(), u[1].to_bits(),
                        w, dx as f32 / 511.0, dy as f32 / 511.0, dz as f32 / 511.0);
                }
                return;
            }
        }
    }
}
