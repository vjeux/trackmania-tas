//! Cross-material lightmap atlas check: chart bboxes (charts = connected tris
//! over shared vertex indices) from EVERY visual with uv1; counts overlapping
//! bbox pairs within a material vs across materials, and the union coverage.
//! Usage: uv1cross FILE
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // (material stem, bbox, ntri, area2)
    let mut boxes: Vec<(String, [[f32; 2]; 2], usize, f64)> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        let Some(vref) = s2.visuals.get(vi) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() else { continue };
        let st = vis.stream().unwrap();
        let (mut pos, mut uv1) = (Vec::new(), Vec::new());
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                _ => {}
            }
        }
        if uv1.is_empty() {
            continue;
        }
        let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        let tris: Vec<[u32; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0], t[1], t[2]]).collect();
        let n = pos.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize {
            let mut r = x;
            while p[r] != r {
                r = p[r];
            }
            let mut c = x;
            while p[c] != r {
                let nx = p[c];
                p[c] = r;
                c = nx;
            }
            r
        }
        for t in &tris {
            let r0 = find(&mut parent, t[0] as usize);
            let r1 = find(&mut parent, t[1] as usize);
            parent[r1] = r0;
            let r0 = find(&mut parent, t[0] as usize);
            let r2 = find(&mut parent, t[2] as usize);
            parent[r2] = r0;
        }
        let mut charts: BTreeMap<usize, ([[f32; 2]; 2], usize, f64)> = BTreeMap::new();
        for t in &tris {
            let r = find(&mut parent, t[0] as usize);
            let e = charts.entry(r).or_insert(([[f32::MAX; 2], [f32::MIN; 2]], 0, 0.0));
            e.1 += 1;
            let q = [uv1[t[0] as usize], uv1[t[1] as usize], uv1[t[2] as usize]];
            e.2 += (((q[1][0] - q[0][0]) * (q[2][1] - q[0][1]) - (q[2][0] - q[0][0]) * (q[1][1] - q[0][1])) as f64 / 2.0).abs();
            for v in t {
                let u = uv1[*v as usize];
                e.0[0][0] = e.0[0][0].min(u[0]);
                e.0[0][1] = e.0[0][1].min(u[1]);
                e.0[1][0] = e.0[1][0].max(u[0]);
                e.0[1][1] = e.0[1][1].max(u[1]);
            }
        }
        for (_, (bb, nt, a2)) in charts {
            boxes.push((stem.clone(), bb, nt, a2));
        }
    }
    let (mut within, mut across) = (0usize, 0usize);
    let mut across_area = 0.0f64;
    let mut examples: Vec<String> = Vec::new();
    for i in 0..boxes.len() {
        for j in i + 1..boxes.len() {
            let (a, b) = (&boxes[i].1, &boxes[j].1);
            let ox = a[1][0].min(b[1][0]) - a[0][0].max(b[0][0]);
            let oy = a[1][1].min(b[1][1]) - a[0][1].max(b[0][1]);
            if ox > 1e-5 && oy > 1e-5 {
                if boxes[i].0 == boxes[j].0 {
                    within += 1;
                } else {
                    across += 1;
                    across_area += (ox * oy) as f64;
                    if examples.len() < 6 {
                        examples.push(format!("{}({}t) x {}({}t) overlap {:.5}x{:.5}", boxes[i].0, boxes[i].2, boxes[j].0, boxes[j].2, ox, oy));
                    }
                }
            }
        }
    }
    let tot2: f64 = boxes.iter().map(|b| b.3).sum();
    println!("charts={} uv1 area total={:.4} within-material bbox overlaps={} across-material bbox overlaps={} (area {:.5})", boxes.len(), tot2, within, across, across_area);
    for e in examples {
        println!("  {}", e);
    }
}
