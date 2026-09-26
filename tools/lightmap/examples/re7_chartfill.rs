//! `re7_chartfill EDITOR.Map.Gbx [--chart I…] [--obj-min N]` — the per-chart FILL question (RE 7, 2026-09-26): for the item
//! charts of an editor save, the chart rect in the 1024 image, the lit fraction of the frame-0 colour image, of each grey plane,
//! and an ASCII map of the chart (`#` lit colour, `.` zero) to see whether the game fills texels beyond the triangles.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let m = lightmap::mapio::load(&a[1]).expect("map");
    let d = m.chunk.data.as_ref().expect("data");
    let mp = d.cache.mapping().expect("mapping");
    let frames = &d.frames;
    let parts = |im: &[u8]| -> Vec<Vec<u8>> {
        let mut v = Vec::new();
        let mut off = 0usize;
        while off + 12 <= im.len() && &im[off..off + 4] == b"RIFF" {
            let sz = u32::from_le_bytes([im[off + 4], im[off + 5], im[off + 6], im[off + 7]]) as usize + 8;
            v.push(im[off..(off + sz).min(im.len())].to_vec());
            off += sz;
        }
        v
    };
    let img0 = lightmap::img::decode_webp(&parts(&frames[0].images[0])[0]).expect("webp 0");
    let greys: Vec<lightmap::img::Rgb> = parts(&frames[0].images[1]).iter().map(|p| lightmap::img::decode_webp(p).expect("grey")).collect();
    let w = img0.w as usize;
    println!("colour {}×{}, greys {} planes", img0.w, img0.h, greys.len());
    let want: Vec<usize> = f("--chart").map(|v| v.split(',').map(|t| t.parse().unwrap()).collect()).unwrap_or_default();
    let obj_min: u32 = f("--obj-min").map(|v| v.parse().unwrap()).unwrap_or(0);
    for i in 0..mp.pos.len() {
        if !want.is_empty() && !want.contains(&i) { continue; }
        if want.is_empty() && mp.binds[i].obj_idx < obj_min { continue; }
        let (x2, y2) = mp.pos[i];
        let (w2, h2) = mp.size[i];
        let x0 = ((x2 as usize).saturating_sub(1)) / 2;
        let y0 = ((y2 as usize).saturating_sub(1)) / 2;
        let cw = (w2 as usize) / 2 + 1;
        let ch = (h2 as usize) / 2 + 1;
        let mut n = 0usize;
        let mut lit = 0usize;
        let mut lit_g = vec![0usize; greys.len()];
        let mut art = String::new();
        for y in y0..(y0 + ch).min(w) {
            for x in x0..(x0 + cw).min(w) {
                let o = (y * w + x) * 3;
                n += 1;
                let v = img0.px[o] as u32 + img0.px[o + 1] as u32 + img0.px[o + 2] as u32;
                if v > 0 { lit += 1; }
                for (k, g) in greys.iter().enumerate() { if g.px[o] > 0 { lit_g[k] += 1; } }
                let step = ((cw + 119) / 120).max((ch + 79) / 80).max(1);
                if (x - x0) % step == 0 && (y - y0) % step == 0 { art.push(if v > 0 { if v > 600 { '#' } else { '+' } } else { '.' }); }
            }
            let step = ((cw + 119) / 120).max((ch + 79) / 80).max(1);
            if (y - y0) % step == 0 { art.push('\n'); }
        }
        println!("chart {i}: obj {} group {} rect2048 ({x2},{y2}) {w2}×{h2} → img ({x0},{y0}) {cw}×{ch}: colour lit {lit}/{n} = {:.3}; greys lit {:?}; fb {:?}", mp.binds[i].obj_idx, mp.binds[i].obj_group_idx, lit as f64 / n.max(1) as f64, lit_g, mp.frame_bytes.iter().map(|t| t.get(i).copied().unwrap_or(0)).collect::<Vec<_>>());
        if !art.is_empty() { print!("{art}"); }
    }
}
