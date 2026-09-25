//! `re7_frame1 EDITOR.Map.Gbx F1.rgb` — which charts frame 1 lights: per chart the lit fraction, grouped by size and by
//! the frame-byte, with the chart rects (2048 layout: X = 2x + 1, W = 2(w − 1)) mapped to the 1024 image (RE 7).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = lightmap::mapio::load(&a[1]).expect("map");
    let d = m.chunk.data.as_ref().expect("data");
    let mp = d.cache.mapping().expect("mapping");
    let px = std::fs::read(&a[2]).expect("rgb");
    let w = 1024usize;
    let mut by_size: std::collections::BTreeMap<(u16, u16), (usize, usize, f64)> = Default::default();
    let mut lit_charts = 0;
    let mut rows: Vec<(usize, f64, (u16, u16), (u16, u16), u32, u32, u8, u8)> = Vec::new();
    for i in 0..mp.pos.len() {
        let (x2, y2) = mp.pos[i];
        let (w2, h2) = mp.size[i];
        // 2048 layout → 1024 image: x = (X − 1)/2, w = W/2 + 1
        let x0 = ((x2 as usize).saturating_sub(1)) / 2;
        let y0 = ((y2 as usize).saturating_sub(1)) / 2;
        let cw = (w2 as usize) / 2 + 1;
        let ch = (h2 as usize) / 2 + 1;
        let mut n = 0usize;
        let mut lit = 0usize;
        let mut sum = 0u64;
        for y in y0..(y0 + ch).min(1024) {
            for x in x0..(x0 + cw).min(1024) {
                let o = (y * w + x) * 3;
                n += 1;
                let v = px[o] as u64 + px[o + 1] as u64 + px[o + 2] as u64;
                if v > 0 { lit += 1; sum += v; }
            }
        }
        let f = if n > 0 { lit as f64 / n as f64 } else { 0.0 };
        if f > 0.5 { lit_charts += 1; }
        let e = by_size.entry((w2, h2)).or_insert((0, 0, 0.0));
        e.0 += 1;
        if f > 0.5 { e.1 += 1; }
        e.2 += f;
        let fb0 = mp.frame_bytes.first().and_then(|v| v.get(i)).copied().unwrap_or(0);
        let fb1 = mp.frame_bytes.get(1).and_then(|v| v.get(i)).copied().unwrap_or(0);
        rows.push((i, f, (x2, y2), (w2, h2), mp.binds[i].obj_idx, mp.binds[i].obj_group_idx, fb0, fb1));
        let _ = sum;
    }
    println!("{} charts, {} lit (> 50 % of texels non-zero); frame bytes tables {}", mp.pos.len(), lit_charts, mp.frame_bytes.len());
    println!("by chart size (w, h): (count, lit, mean lit fraction)");
    for (k, v) in &by_size { println!("  {:?}: {} charts, {} lit, mean {:.3}", k, v.0, v.1, v.2 / v.0 as f64); }
    // frame byte 1 distribution for lit vs unlit
    let mut fb: std::collections::BTreeMap<(bool, u8), usize> = Default::default();
    for r in &rows { *fb.entry((r.1 > 0.5, r.7)).or_default() += 1; }
    println!("frame-1 byte by lit: {:?}", fb);
    let mut fb0: std::collections::BTreeMap<(bool, u8), usize> = Default::default();
    for r in &rows { *fb0.entry((r.1 > 0.5, r.6)).or_default() += 1; }
    println!("frame-0 byte by lit: {:?}", fb0);
    // object indices of lit charts
    let mut objs: std::collections::BTreeMap<u32, (usize, usize)> = Default::default();
    for r in &rows { let e = objs.entry(r.4).or_default(); e.0 += 1; if r.1 > 0.5 { e.1 += 1; } }
    let lit_objs: Vec<(u32, (usize, usize))> = objs.iter().filter(|(_, v)| v.1 > 0).map(|(k, v)| (*k, *v)).collect();
    println!("{} objects, {} with lit charts; first 30: {:?}", objs.len(), lit_objs.len(), &lit_objs[..lit_objs.len().min(30)]);
    println!("first lit charts: {:?}", rows.iter().filter(|r| r.1 > 0.5).take(12).collect::<Vec<_>>());
}
