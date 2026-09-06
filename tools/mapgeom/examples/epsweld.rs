//! Epsilon-weld collision positions; count. Usage: epsweld MINE.ITEM EPSMM
use std::collections::BTreeMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let eps: f32 = a[2].parse().unwrap();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    if let Some(surf) = so.surface() {
        if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles: _, version: _ } = &surf.surf {
            // grid-hash epsilon weld (cell = eps, merge within cell + neighbors? simple: quantize to eps grid)
            // Actually epsilon-weld (not grid): union-find within eps. O(n^2) too slow (6303^2=40M, ok in Rust).
            let n = vertices.len();
            let mut parent: Vec<usize> = (0..n).collect();
            fn find(p: &mut Vec<usize>, x: usize) -> usize {
                if p[x] != x { p[x] = find(p, p[x]); }
                p[x]
            }
            // spatial hash for speed
            let mut grid: BTreeMap<(i32, i32, i32), Vec<usize>> = BTreeMap::new();
            for (i, v) in vertices.iter().enumerate() {
                grid.entry(((v[0]/eps).floor() as i32, (v[1]/eps).floor() as i32, (v[2]/eps).floor() as i32)).or_default().push(i);
            }
            for (i, v) in vertices.iter().enumerate() {
                let c = ((v[0]/eps).floor() as i32, (v[1]/eps).floor() as i32, (v[2]/eps).floor() as i32);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if let Some(js) = grid.get(&(c.0+dx, c.1+dy, c.2+dz)) {
                                for j in js {
                                    if *j <= i { continue; }
                                    let w = &vertices[*j];
                                    let dd = ((v[0]-w[0]).powi(2)+(v[1]-w[1]).powi(2)+(v[2]-w[2]).powi(2)).sqrt();
                                    if dd < eps {
                                        let a2 = find(&mut parent, i);
                                        let b = find(&mut parent, *j);
                                        if a2 != b { parent[a2] = b; }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let mut roots = std::collections::BTreeSet::new();
            for i in 0..n {
                roots.insert(find(&mut parent, i));
            }
            println!("eps={eps}mm: welded {} -> {} verts (want 6140)", n, roots.len());
        }
    }
}
