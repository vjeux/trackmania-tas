//! `re15_maphead MAP…` — the lightmap mapping header words of each map: version, map_version, u01, atlas w/h, bbox, u02, count,
//! the frame records' MaxHDR (RE 15: which atlas size the saved mapping carries per map).
fn main() {
    for p in std::env::args().skip(1) {
        match lightmap::mapio::load(&p) {
            Ok(map) => {
                let Some(d) = map.chunk.data.as_ref() else { println!("{p}: no lightmap data"); continue };
                let Some(m) = d.cache.mapping() else { println!("{p}: no mapping"); continue };
                let mh: Vec<String> = (0..3).filter_map(|f| lightmap::classcmp::record_maxhdr(&m, f).map(|v| format!("{v}"))).collect();
                println!("{p}: mapping v{} map_v{} u01 {} atlas {}×{} bbox {:?}..{:?} u02 {} charts {} head {} B, MaxHDR {:?}", m.version, m.map_version, m.m_u01, m.atlas_w, m.atlas_h, m.bbox_min, m.bbox_max, m.m_u02, m.count, m.head.len(), mh);
            }
            Err(e) => println!("{p}: {e}"),
        }
    }
}
