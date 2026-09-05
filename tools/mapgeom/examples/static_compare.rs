//! Field-by-field diff of two static items: visuals (verts, tris, pos/uv
//! ranges), materials, collision. Usage: static_compare A.Item.Gbx B.Item.Gbx
use mapgeom::static_item::{parse_file, vstream::Elem, Node};

struct Vis {
    verts: usize,
    tris: usize,
    plo: [f32; 3],
    phi: [f32; 3],
    ulo: [f32; 2],
    uhi: [f32; 2],
    has_uv: bool,
    mat: String,
    phys: u8,
}

struct Sum {
    name: String,
    visuals: Vec<Vis>,
    surf_tris: usize,
    surf_ids: Vec<u16>,
}

fn summarize(path: &str) -> Sum {
    let f = parse_file(&std::fs::read(path).unwrap()).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mats: Vec<(String, u8)> = s2
        .custom_materials
        .iter()
        .map(|m| {
            m.inst().map(|i| (i.link().unwrap_or("?").to_string(), i.physics())).unwrap_or(("?".into(), 99))
        })
        .collect();
    let mut visuals = Vec::new();
    for (gi, g) in s2.shaded_geoms.iter().enumerate() {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let (mat, phys) = mats.get(mi).cloned().unwrap_or(("?".into(), 99));
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut plo = [f32::MAX; 3];
                let mut phi = [f32::MIN; 3];
                let mut verts = 0;
                let mut ulo = [f32::MAX; 2];
                let mut uhi = [f32::MIN; 2];
                let mut has_uv = false;
                for e in &st.elems {
                    match e {
                        Elem::Float3(p) if verts == 0 => {
                            verts = p.len();
                            for q in p {
                                for k in 0..3 {
                                    plo[k] = plo[k].min(q[k]);
                                    phi[k] = phi[k].max(q[k]);
                                }
                            }
                        }
                        Elem::Float2(u) => {
                            has_uv = true;
                            for q in u {
                                for k in 0..2 {
                                    ulo[k] = ulo[k].min(q[k]);
                                    uhi[k] = uhi[k].max(q[k]);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let tris = vis.index_buffer.as_ref().map(|ib| ib.indices.len()).unwrap_or(0) / 3;
                let _ = gi;
                visuals.push(Vis { verts, tris, plo, phi, ulo, uhi, has_uv, mat, phys });
            }
        }
    }
    let (surf_tris, surf_ids) = so
        .surface()
        .map(|s| {
            let n = match &s.surf {
                mapgeom::static_item::surface::Surf::Mesh { triangles, .. } => triangles.len(),
                _ => 0,
            };
            (n, s.material_ids.clone())
        })
        .unwrap_or((0, vec![]));
    Sum { name: path.to_string(), visuals, surf_tris, surf_ids }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let x = summarize(&a[1]);
    let y = summarize(&a[2]);
    println!("{}: {} visuals, {} surf tris, surf ids {:?}", x.name, x.visuals.len(), x.surf_tris, x.surf_ids);
    println!("{}: {} visuals, {} surf tris, surf ids {:?}", y.name, y.visuals.len(), y.surf_tris, y.surf_ids);
    let n = x.visuals.len().max(y.visuals.len());
    for i in 0..n {
        match (x.visuals.get(i), y.visuals.get(i)) {
            (Some(v), Some(w)) => {
                let pm = if v.mat == w.mat && v.phys == w.phys { "same" } else { "DIFF" };
                println!("vis{i}: mat {pm} [{}|{} vs {}|{}] verts {} vs {} tris {} vs {}",
                    v.mat, v.phys, w.mat, w.phys, v.verts, w.verts, v.tris, w.tris);
                println!("   pos [{:.2},{:.2},{:.2}]-[{:.2},{:.2},{:.2}] vs [{:.2},{:.2},{:.2}]-[{:.2},{:.2},{:.2}]",
                    v.plo[0], v.plo[1], v.plo[2], v.phi[0], v.phi[1], v.phi[2],
                    w.plo[0], w.plo[1], w.plo[2], w.phi[0], w.phi[1], w.phi[2]);
                println!("   uv {} vs {}",
                    if v.has_uv { format!("{:.2},{:.2}-{:.2},{:.2}", v.ulo[0], v.ulo[1], v.uhi[0], v.uhi[1]) } else { "NONE".into() },
                    if w.has_uv { format!("{:.2},{:.2}-{:.2},{:.2}", w.ulo[0], w.ulo[1], w.uhi[0], w.uhi[1]) } else { "NONE".into() });
            }
            (Some(v), None) => println!("vis{i}: only in A: {}|{} {} verts", v.mat, v.phys, v.verts),
            (None, Some(w)) => println!("vis{i}: only in B: {}|{} {} verts", w.mat, w.phys, w.verts),
            (None, None) => {}
        }
    }
}
