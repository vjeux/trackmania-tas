//! Weld comparison via matched triangles (precise).
//! Usage: hisplit REF.HIS MINE.BAKED dx dy dz SUBSTR
//! For each of OUR (pos,uv) keys: our distinct normals vs his distinct normals.
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }

struct Tri { v: [[f32; 3]; 3], n: [[f32; 3]; 3], u: [[f32; 2]; 3] }

fn load(path: &str, substr: &str) -> Vec<(Tri, String)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(substr) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm, mut uv) = (Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                if pos.len() != nrm.len() || pos.len() != uv.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let g = |a: &Vec<[f32; 3]>, i: usize| a[t[i] as usize];
                    let gu = |i: usize| uv[t[i] as usize];
                    out.push((Tri { v: [g(&pos,0), g(&pos,1), g(&pos,2)], n: [g(&nrm,0), g(&nrm,1), g(&nrm,2)], u: [gu(0), gu(1), gu(2)] }, mat.clone()));
                }
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let r = load(&a[1], &a[6]);
    let m = load(&a[2], &a[6]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [key(&[t.v[0][0]+d[0], t.v[0][1]+d[1], t.v[0][2]+d[2]]), key(&[t.v[1][0]+d[0], t.v[1][1]+d[1], t.v[1][2]+d[2]]), key(&[t.v[2][0]+d[0], t.v[2][1]+d[1], t.v[2][2]+d[2]])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    // our key (posbits, uvbits) -> (set of our normals, set of his normals)
    let mut agg: BTreeMap<(String, [u32; 5]), (Vec<[f32; 3]>, Vec<[f32; 3]>)> = BTreeMap::new();
    let mut ntri = 0;
    for (t, mat) in &r {
        let mut k = [key(&t.v[0]), key(&t.v[1]), key(&t.v[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            ntri += 1;
            let (u, _) = &m[v[0]];
            for ck in 0..3 {
                for mk in 0..3 {
                    let mp = [u.v[mk][0]+d[0], u.v[mk][1]+d[1], u.v[mk][2]+d[2]];
                    if (mp[0]-t.v[ck][0]).abs() < 0.002 && (mp[1]-t.v[ck][1]).abs() < 0.002 && (mp[2]-t.v[ck][2]).abs() < 0.002 {
                        let up = u.v[mk];
                        let key2 = (mat.clone(), [up[0].to_bits(), up[1].to_bits(), up[2].to_bits(), u.u[mk][0].to_bits(), u.u[mk][1].to_bits()]);
                        let e = agg.entry(key2).or_insert((Vec::new(), Vec::new()));
                        if !e.0.iter().any(|x| dot(*x, u.n[mk]) > 0.99999) { e.0.push(u.n[mk]); }
                        if !e.1.iter().any(|x| dot(*x, t.n[ck]) > 0.99999) { e.1.push(t.n[ck]); }
                        break;
                    }
                }
            }
        }
    }
    // per material: cases (our_n, his_n)
    let mut stat: BTreeMap<String, BTreeMap<(usize, usize), usize>> = BTreeMap::new();
    let mut angles: BTreeMap<String, Vec<f32>> = BTreeMap::new();
    for ((mat, _), (ours, hiss)) in &agg {
        let short = mat.rsplit('\\').next().unwrap_or(mat).to_string();
        *stat.entry(short.clone()).or_default().entry((ours.len(), hiss.len())).or_insert(0) += 1;
        if ours.len() == 1 && hiss.len() == 2 {
            angles.entry(short).or_default().push(dot(hiss[0], hiss[1]).acos().to_degrees());
        }
    }
    println!("matched tris={ntri}");
    for (mat, cases) in &stat {
        println!("{mat}: {cases:?}");
        if let Some(v) = angles.get(mat) {
            let mut s = v.clone();
            s.sort_by(|x, y| x.partial_cmp(y).unwrap());
            println!("  he-splits-where-we-weld angles min={:.1} p50={:.1} max={:.1} n={}", s[0], s[s.len()/2], s[s.len()-1], s.len());
        }
    }
}
