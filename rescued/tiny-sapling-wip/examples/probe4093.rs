//! Focused triple-census probe (resolves detfit census paradox).
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;
fn pk(p: [f32; 3]) -> [u32; 3] { [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()] }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let want: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(4093);
    let mut meshes = Vec::new();
    for path in [&a[1], &a[2]] {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut pos: Vec<[f32; 3]> = vec![];
        let mut tris: Vec<[u32; 3]> = vec![];
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if mat.rsplit('\\').next().unwrap_or(&mat) != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e { if d.name() == 0 { pos = p.clone(); } }
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) { if t.len() == 3 { tris.push([t[0], t[1], t[2]]); } }
                    break;
                }
            }
        }
        meshes.push((pos, tris));
    }
    let tk = |m: &(Vec<[f32;3]>, Vec<[u32;3]>), t: &[u32; 3]| -> [[u32; 3]; 3] {
        let mut k = [pk(m.0[t[0] as usize]), pk(m.0[t[1] as usize]), pk(m.0[t[2] as usize])];
        k.sort(); k
    };
    let mt = meshes[1].1[want];
    let tkv = tk(&meshes[1], &mt);
    println!("mine tri{want} = {mt:?} triple={tkv:08x?}");
    for (li, m) in meshes.iter().enumerate() {
        let matches: Vec<usize> = m.1.iter().enumerate().filter(|(_, t)| tk(m, t) == tkv).map(|(j, _)| j).collect();
        println!("mesh{li}: ntris={} triple-matches={matches:?}", m.1.len());
    }
    // bucket + pop order
    let mut b: BTreeMap<[[u32;3];3], Vec<usize>> = BTreeMap::new();
    for (j, t) in meshes[0].1.iter().enumerate() { b.entry(tk(&meshes[0], t)).or_default().push(j); }
    println!("bucket[tk] = {:?}", b.get(&tkv));
}
