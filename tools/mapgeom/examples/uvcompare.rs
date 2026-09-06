//! Per-triangle UV0 compare on matching tris (with translation). Usage:
//! uvcompare REF MINE dx dy dz
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}

struct Tri { v: [[f32; 3]; 3], uv: [[f32; 2]; 3] }

fn load(path: &str) -> Vec<(Tri, String)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").to_string()).unwrap_or("?".into());
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    out.push((Tri { v: [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], uv: [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]] }, mat.clone()));
                }
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [key(&[t.v[0][0]+d[0], t.v[0][1]+d[1], t.v[0][2]+d[2]]), key(&[t.v[1][0]+d[0], t.v[1][1]+d[1], t.v[1][2]+d[2]]), key(&[t.v[2][0]+d[0], t.v[2][1]+d[1], t.v[2][2]+d[2]])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let (mut n, mut same, mut du, mut dv) = (0, 0, 0.0, 0.0);
    for (t, mat) in &r {
        if !mat.contains("RoadTech") && !mat.contains("SpecialFX") {
            continue;
        }
        let mut k = [key(&t.v[0]), key(&t.v[1]), key(&t.v[2])];
        k.sort();
        // try all 3 rotations of ref tri against mine (vertex order may differ)
        if let Some(v) = mmap.get(&k) {
            let (u, _) = &m[v[0]];
            // match corners by position (with translation)
            for ck in 0..3 {
                let rp = t.v[ck];
                let ru = t.uv[ck];
                // find mine corner at rp-d
                let mut found = None;
                for mk in 0..3 {
                    let mp = u.v[mk];
                    if (mp[0]+d[0]-rp[0]).abs() < 0.002 && (mp[1]+d[1]-rp[1]).abs() < 0.002 && (mp[2]+d[2]-rp[2]).abs() < 0.002 {
                        found = Some(u.uv[mk]);
                        break;
                    }
                }
                if let Some(mu) = found {
                    n += 1;
                    du += (ru[0]-mu[0]).abs();
                    dv += (ru[1]-mu[1]).abs();
                    if (ru[0]-mu[0]).abs() < 1e-4 && (ru[1]-mu[1]).abs() < 1e-4 {
                        same += 1;
                    } else if n <= 5 {
                        println!("  refuv {ru:?} mineuv {mu:?} mat={mat}");
                    }
                }
            }
        }
    }
    println!("corners compared={n} identical={same} mean|du|={:.4} mean|dv|={:.4}", du / n.max(1) as f32, dv / n.max(1) as f32);
}
