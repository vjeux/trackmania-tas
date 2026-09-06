//! Are charts coplanar? Usage: chartplanar FILE SUBSTR
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(&a[2]) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv1) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let tris: Vec<[usize; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect();
                let mut parent: Vec<usize> = (0..tris.len()).collect();
                fn find(p: &mut Vec<usize>, x: usize) -> usize {
                    if p[x] != x { p[x] = find(p, p[x]); }
                    p[x]
                }
                let mut corner_map: BTreeMap<[u32; 5], Vec<usize>> = BTreeMap::new();
                for (ti, t) in tris.iter().enumerate() {
                    for k in 0..3 {
                        let key = [pos[t[k]][0].to_bits(), pos[t[k]][1].to_bits(), pos[t[k]][2].to_bits(), uv1[t[k]][0].to_bits(), uv1[t[k]][1].to_bits()];
                        corner_map.entry(key).or_default().push(ti);
                    }
                }
                for (_, tis) in &corner_map {
                    for w in tis.windows(2) {
                        let a2 = find(&mut parent, w[0]);
                        let b = find(&mut parent, w[1]);
                        if a2 != b { parent[a2] = b; }
                    }
                }
                // face normals
                let mut fns: Vec<[f32; 3]> = Vec::new();
                for t in &tris {
                    let p = [pos[t[0]], pos[t[1]], pos[t[2]]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    fns.push([cr[0]/l, cr[1]/l, cr[2]/l]);
                }
                // per chart: max dihedral + tri count
                let mut charts: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
                for i in 0..tris.len() {
                    charts.entry(find(&mut parent, i)).or_default().push(i);
                }
                let mut maxdih_hist = [0usize; 5]; // <1, 1-5, 5-15, 15-45, >45
                let mut sizes: Vec<usize> = Vec::new();
                for (_, tis) in &charts {
                    sizes.push(tis.len());
                    let mut md = 0.0f32;
                    for x in 0..tis.len() {
                        for y in (x+1)..tis.len() {
                            let dot = (fns[tis[x]][0]*fns[tis[y]][0]+fns[tis[x]][1]*fns[tis[y]][1]+fns[tis[x]][2]*fns[tis[y]][2]).clamp(-1.0, 1.0);
                            md = md.max(dot.acos().to_degrees());
                        }
                    }
                    if md < 1.0 { maxdih_hist[0] += 1; }
                    else if md < 5.0 { maxdih_hist[1] += 1; }
                    else if md < 15.0 { maxdih_hist[2] += 1; }
                    else if md < 45.0 { maxdih_hist[3] += 1; }
                    else { maxdih_hist[4] += 1; }
                }
                sizes.sort();
                println!("{}: charts={} maxdih [<1:{},1-5:{},5-15:{},15-45:{},>45:{}] size p50={} max={}",
                    a[2], charts.len(), maxdih_hist[0], maxdih_hist[1], maxdih_hist[2], maxdih_hist[3], maxdih_hist[4],
                    sizes[sizes.len()/2], sizes[sizes.len()-1]);
                return;
            }
        }
    }
}
