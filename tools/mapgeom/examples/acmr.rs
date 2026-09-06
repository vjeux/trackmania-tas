//! ACMR (vertex cache miss ratio) with LRU cache. Usage: acmr FILE CACHESIZE
use mapgeom::static_item::vstream::Elem;
use std::collections::VecDeque;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cache: usize = a[2].parse().unwrap();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let (mut tot_tri, mut tot_miss) = (0, 0);
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut lru: VecDeque<u32> = VecDeque::new();
                let (mut tri, mut miss) = (0, 0);
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    tri += 1;
                    for k in 0..3 {
                        if let Some(p) = lru.iter().position(|x| *x == t[k]) {
                            lru.remove(p);
                            lru.push_back(t[k]);
                        } else {
                            miss += 1;
                            lru.push_back(t[k]);
                            if lru.len() > cache { lru.pop_front(); }
                        }
                    }
                }
                tot_tri += tri;
                tot_miss += miss;
            }
        }
    }
    println!("{}: tris={tot_tri} ACMR(cache={cache})={:.4}", a[1].rsplit('/').next().unwrap(), tot_miss as f64 / tot_tri as f64);
}
