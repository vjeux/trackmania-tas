//! Nearest-clean distance+direction for deviated verts. Usage: devnear HIS.ITEM MINE.ITEM SUBSTR
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<[f32;3]> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 { out.extend(p.iter().cloned()); }
                        }
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut grid: BTreeMap<(i64,i64,i64), Vec<[f32;3]>> = BTreeMap::new();
    for v in &m {
        grid.entry(((v[0]*1e3).floor() as i64, (v[1]*1e3).floor() as i64, (v[2]*1e3).floor() as i64)).or_default().push(*v);
    }
    let mut myset = BTreeSet::new();
    for v in &m { myset.insert([v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]); }
    println!("dist_to_nearest_clean dir(x,y,z signs) his_pos");
    let mut n = 0;
    for hv in &r {
        if myset.contains(&[hv[0].to_bits(), hv[1].to_bits(), hv[2].to_bits()]) { continue; }
        let c = ((hv[0]*1e3).floor() as i64, (hv[1]*1e3).floor() as i64, (hv[2]*1e3).floor() as i64);
        let mut best = (1e9f32, [0.0;3]);
        for dx in -2..=2 { for dy in -2..=2 { for dz in -2..=2 {
            if let Some(vs) = grid.get(&(c.0+dx, c.1+dy, c.2+dz)) {
                for v in vs {
                    let d = ((hv[0]-v[0]).powi(2)+(hv[1]-v[1]).powi(2)+(hv[2]-v[2]).powi(2)).sqrt();
                    if d < best.0 { best = (d, *v); }
                }
            }
        }}}
        println!("  d={:.2e} sgn=({:+},{:+},{:+}) his=({:.6},{:.6},{:.6})", best.0,
            (hv[0]-best.1[0] > 0.0) as u8 as f32 * 2.0 - 1.0, 0.0, 0.0, hv[0], hv[1], hv[2]);
        n += 1;
        if n > 30 { break; }
    }
}
