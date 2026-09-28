//! `e3_chartof MAP.Map.Gbx --obj K` — the chart(s) bound to object K (bind obj = obj_group_idx / 4: a zone tile's block index,
//! an item's record index): chart index, image position/size (chart_own_px) and the RASTER texel of the chart's centre for a
//! `LMTOOL_SET_TEXEL_TRACE` (= 2 × the image coordinate on the 2048² Default atlas). E3 2026-09-28.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let map = lightmap::mapio::load(&a[1]).unwrap_or_else(|e| panic!("{e}"));
    let d = map.chunk.data.as_ref().expect("no lightmap");
    let m = d.cache.mapping().expect("no mapping");
    let want: Option<u32> = f("--obj").map(|s| s.parse().expect("K"));
    // --at X,Y: every chart whose image rectangle contains the image texel (X, Y)
    let at: Option<(u32, u32)> = f("--at").map(|s| { let v: Vec<u32> = s.split(',').map(|x| x.parse().expect("X,Y")).collect(); (v[0], v[1]) });
    let img = lightmap::img::decode_webp(d.frames[0].images.first().expect("image 0")).unwrap_or_else(|e| panic!("{e}"));
    let maxhdr = lightmap::classcmp::record_maxhdr(&m, 0).expect("record");
    for i in 0..m.count as usize {
        let obj = m.binds[i].obj_group_idx / 4;
        let (px, py, pw, ph) = lightmap::classcmp::chart_own_px(m.pos[i], m.size[i]);
        if let Some(w) = want { if obj != w { continue; } }
        if let Some((ax, ay)) = at { if !(ax >= px && ax < px + pw && ay >= py && ay < py + ph) { continue; } }
        let (cx, cy) = (px + pw / 2, py + ph / 2);
        let fb = m.frame_bytes[0][i];
        let idx = ((cy * img.w + cx) * 3) as usize;
        let c: Vec<f64> = (0..3).map(|ch| lightmap::classcmp::texel_hdr(0, img.px[idx + ch], fb, maxhdr)).collect();
        println!("chart {i}: bind obj {obj} sub {} image ({px},{py}) {pw}×{ph} → centre image ({cx},{cy}) = RASTER ({},{}); centre HDR ({:.5},{:.5},{:.5})", m.binds[i].obj_group_idx % 4, 2 * cx, 2 * cy, c[0], c[1], c[2]);
    }
}
