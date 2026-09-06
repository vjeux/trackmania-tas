//! Collision winding on flip faces. Usage: collwind HIS.ITEM MINE.ITEM FLIPKEYS
use std::collections::BTreeMap;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[2]]
}
fn load_coll(path: &str) -> Vec<[[f32; 3]; 3]> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let mut out = Vec::new();
    if let Some(surf) = so.surface() {
        if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, version: _ } = &surf.surf {
            for t in triangles {
                out.push([vertices[t.indices[0] as usize], vertices[t.indices[1] as usize], vertices[t.indices[2] as usize]]);
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let flips: std::collections::BTreeSet<[(i32, i32, i32); 3]> = std::fs::read_to_string(&a[3]).unwrap().trim().split('|').map(|s| {
        let v: Vec<&str> = s.split(';').collect();
        let p = |i: usize| {
            let c: Vec<&str> = v[i].split(',').collect();
            (c[0].parse().unwrap(), c[1].parse().unwrap(), c[2].parse().unwrap())
        };
        let mut k = [p(0), p(1), p(2)];
        k.sort();
        k
    }).collect();
    let r = load_coll(&a[1]);
    let m = load_coll(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, t) in m.iter().enumerate() {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let (mut flip, mut same) = (0, 0);
    for t in &r {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        if !flips.contains(&k) { continue; }
        if let Some(v) = mmap.get(&k) {
            let u = &m[v[0]];
            // compare winding via first-corner alignment? Simply: check if orders are even or odd permutations.
            // Find perm mapping u->t (nearest), check parity.
            let mut best: Option<(usize, f32)> = None;
            for (pi, cand) in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]].iter().enumerate() {
                let mut mx = 0.0f32;
                for cc in 0..3 {
                    let dd = ((u[cand[cc]][0]-t[cc][0]).powi(2)+(u[cand[cc]][1]-t[cc][1]).powi(2)+(u[cand[cc]][2]-t[cc][2]).powi(2)).sqrt();
                    mx = mx.max(dd);
                }
                if best.is_none() || mx < best.unwrap().1 {
                    best = Some((pi, mx));
                }
            }
            let (pi, mx) = best.unwrap();
            if mx > 0.002 { continue; }
            let even = pi == 0 || pi == 3 || pi == 4;
            if even { same += 1; } else { flip += 1; }
        }
    }
    println!("collision flip-faces: same-winding={same} opposite-winding={flip}");
}
