//! Dihedral distribution his-positions vs my-positions. Usage: dihdist HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn load(path: &str, substr: &str) -> Vec<Vec<[f32; 3]>> {
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
                let mut pos = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    out.push(vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                }
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    for path in [&a[1], &a[2]] {
        let tris = load(path, &a[3]);
        // face normals
        let mut fns: Vec<[f32; 3]> = Vec::new();
        for t in &tris {
            let e1 = [t[1][0]-t[0][0], t[1][1]-t[0][1], t[1][2]-t[0][2]];
            let e2 = [t[2][0]-t[0][0], t[2][1]-t[0][1], t[2][2]-t[0][2]];
            let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
            let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
            fns.push([cr[0]/l, cr[1]/l, cr[2]/l]);
        }
        // dihedral histogram (adjacent tris sharing an edge? approximate: all pairs is O(n^2), too slow. Use per-position pairs.)
        // per position pairs:
        let mut bypos: BTreeMap<[u32; 3], Vec<usize>> = BTreeMap::new();
        for (i, t) in tris.iter().enumerate() {
            for k in 0..3 {
                bypos.entry([t[k][0].to_bits(), t[k][1].to_bits(), t[k][2].to_bits()]).or_default().push(i);
            }
        }
        let mut hist = [0usize; 90];
        for (_, idxs) in &bypos {
            let mut seen = std::collections::BTreeSet::new();
            for x in 0..idxs.len() {
                for y in (x+1)..idxs.len() {
                    if idxs[x] == idxs[y] { continue; }
                    let key = (idxs[x].min(idxs[y]), idxs[x].max(idxs[y]));
                    if !seen.insert(key) { continue; }
                    let dot = (fns[idxs[x]][0]*fns[idxs[y]][0]+fns[idxs[x]][1]*fns[idxs[y]][1]+fns[idxs[x]][2]*fns[idxs[y]][2]).clamp(-1.0, 1.0);
                    let d = dot.acos().to_degrees() as usize;
                    if d < 90 { hist[d] += 1; }
                }
            }
        }
        println!("{}:", path.rsplit('/').next().unwrap());
        for d in (35..55).step_by(1) {
            println!("  {d}deg: {}", hist[d]);
        }
    }
}
