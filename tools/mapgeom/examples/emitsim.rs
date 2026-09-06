//! Simulate max-vert emission; compare order to his. Usage: emitsim HIS.ITEM SRCFILE SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i64, i64, i64) {
    ((p[0]*100.0).round() as i64, (p[1]*100.0).round() as i64, (p[2]*100.0).round() as i64)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let t = [-0.0019760131836f32, -0.0000076293945312, -0.023214340210];
    // my material index for substr: find crystal mat with link containing substr
    let mut cmats: Vec<usize> = Vec::new();
    for (i, mm) in c.materials.iter().enumerate() {
        let link = mm.inst().map(|x| x.link().unwrap_or("").to_string()).unwrap_or_default();
        if link.contains(&a[3]) { cmats.push(i); }
    }
    // collect faces of these mats in file order (visible layers forward)
    let mut facetris: Vec<[[f32; 3]; 3]> = Vec::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.material < 0 || !cmats.contains(&(f.material as usize)) { continue; }
            for tri in face_triangles(cr, f, 0.5) {
                let h = [[tri[0].pos[0]+t[0], tri[0].pos[1]+t[1], tri[0].pos[2]+t[2]],
                         [tri[1].pos[0]+t[0], tri[1].pos[1]+t[1], tri[1].pos[2]+t[2]],
                         [tri[2].pos[0]+t[0], tri[2].pos[1]+t[1], tri[2].pos[2]+t[2]]];
                facetris.push(h);
            }
        }
    }
    // weld by exact position (bits), creation rank; sort tris by max rank (stable)
    let mut rank: BTreeMap<[u32; 3], usize> = BTreeMap::new();
    let mut keyed: Vec<([(i64, i64, i64); 3], [usize; 3])> = Vec::new();
    for tri in &facetris {
        let mut r = [0usize; 3];
        for cc in 0..3 {
            let b = [tri[cc][0].to_bits(), tri[cc][1].to_bits(), tri[cc][2].to_bits()];
            let n = rank.len();
            r[cc] = *rank.entry(b).or_insert(n);
        }
        let mut k = [mk(&tri[0]), mk(&tri[1]), mk(&tri[2])];
        k.sort();
        keyed.push((k, r));
    }
    let mut order: Vec<usize> = (0..keyed.len()).collect();
    order.sort_by_key(|&i| keyed[i].1.iter().max().unwrap());
    // his order (keys)
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut hiskeys: Vec<[(i64, i64, i64); 3]> = Vec::new();
    for gg in &s2.shaded_geoms {
        let vi = gg.visual_index.max(0) as usize;
        let mi = gg.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(&a[3]) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (dd2, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if dd2.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for tt in idx.chunks(3) {
                    if tt.len() < 3 { continue; }
                    let mut k = [mk(&pos[tt[0] as usize]), mk(&pos[tt[1] as usize]), mk(&pos[tt[2] as usize])];
                    k.sort();
                    hiskeys.push(k);
                }
            }
        }
    }
    // compare sequences (multiset per prefix? just exact positional match rate)
    let simkeys: Vec<[(i64, i64, i64); 3]> = order.iter().map(|&i| keyed[i].0).collect();
    // handle duplicates: greedy match
    let mut used = vec![false; simkeys.len()];
    let mut posmap: BTreeMap<[(i64, i64, i64); 3], Vec<usize>> = BTreeMap::new();
    for (i, k) in simkeys.iter().enumerate() {
        posmap.entry(*k).or_default().push(i);
    }
    // LCS length via patience? simpler: positional exact matches
    let mut exact = 0;
    for (i, k) in hiskeys.iter().enumerate() {
        if i < simkeys.len() && simkeys[i] == *k { exact += 1; }
    }
    println!("{}: ntri his={} sim={} positional_exact={} ({:.1}%)", a[3], hiskeys.len(), simkeys.len(), exact, 100.0*exact as f32/hiskeys.len() as f32);
    let _ = used;
}
fn hiskey_len(_v: &Vec<[(i64, i64, i64); 3]>) -> usize { 0 }
