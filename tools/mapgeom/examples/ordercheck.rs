//! Is tri order sorted by min/max vertex? Usage: ordercheck FILE
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
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or("?".into());
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let tris: Vec<[u32; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0], t[1], t[2]]).collect();
                let mut sort_min = true;
                let mut sort_max = true;
                let mut sort_first = true;
                for w in tris.windows(2) {
                    let m0 = w[0][0].min(w[0][1]).min(w[0][2]);
                    let m1 = w[1][0].min(w[1][1]).min(w[1][2]);
                    if m1 < m0 { sort_min = false; }
                    let x0 = w[0][0].max(w[0][1]).max(w[0][2]);
                    let x1 = w[1][0].max(w[1][1]).max(w[1][2]);
                    if x1 < x0 { sort_max = false; }
                    if w[1][0] < w[0][0] { sort_first = false; }
                }
                println!("{mat}: ntri={} sorted_by_min={sort_min} sorted_by_max={sort_max} sorted_by_first={sort_first}", tris.len());
            }
        }
    }
    let _ = Elem::Float3(vec![[0.0; 3]]);
}
