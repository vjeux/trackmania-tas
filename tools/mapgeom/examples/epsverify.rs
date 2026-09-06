//! Epsilon-average weld mine, compare to his. Usage: epsverify HIS.ITEM MINE.ITEM EPSMM
use std::collections::{BTreeMap, BTreeSet};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let eps: f32 = a[3].parse().unwrap();
    let load = |path: &str| -> Vec<[f32; 3]> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let mut out = Vec::new();
        if let Some(surf) = so.surface() {
            if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles: _, version: _ } = &surf.surf {
                for v in vertices {
                    out.push(*v);
                }
            }
        }
        out
    };
    let his = load(&a[1]);
    let mine = load(&a[2]);
    // union-find mine within eps
    let n = mine.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, x: usize) -> usize {
        if p[x] != x { p[x] = find(p, p[x]); }
        p[x]
    }
    let mut grid: BTreeMap<(i32, i32, i32), Vec<usize>> = BTreeMap::new();
    for (i, v) in mine.iter().enumerate() {
        grid.entry(((v[0]/eps).floor() as i32, (v[1]/eps).floor() as i32, (v[2]/eps).floor() as i32)).or_default().push(i);
    }
    for (i, v) in mine.iter().enumerate() {
        let c = ((v[0]/eps).floor() as i32, (v[1]/eps).floor() as i32, (v[2]/eps).floor() as i32);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(js) = grid.get(&(c.0+dx, c.1+dy, c.2+dz)) {
                        for j in js {
                            if *j <= i { continue; }
                            let w = &mine[*j];
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
    // groups -> average
    let mut groups: BTreeMap<usize, Vec<[f32; 3]>> = BTreeMap::new();
    for i in 0..n {
        groups.entry(find(&mut parent, i)).or_default().push(mine[i]);
    }
    let mut welded: Vec<[f32; 3]> = Vec::new();
    for (_, g) in &groups {
        if g.len() == 1 {
            welded.push(g[0]);
        } else {
            let n2 = g.len() as f32;
            welded.push([g.iter().map(|p| p[0]).sum::<f32>()/n2, g.iter().map(|p| p[1]).sum::<f32>()/n2, g.iter().map(|p| p[2]).sum::<f32>()/n2]);
        }
    }
    // compare welded set to his set (bit-exact)
    let hset: BTreeSet<[u32; 3]> = his.iter().map(|v| [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]).collect();
    let mut hit = 0;
    for w in &welded {
        if hset.contains(&[w[0].to_bits(), w[1].to_bits(), w[2].to_bits()]) {
            hit += 1;
        }
    }
    println!("eps={} mine {} -> welded {} (his {}) bitexact {} ({:.1}%)",
        eps, n, welded.len(), his.len(), hit, 100.0*hit as f32/welded.len() as f32);
}
