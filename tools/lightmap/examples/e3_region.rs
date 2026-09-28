//! `e3_region MAP.Map.Gbx --at X0,Y0:X1,Y1 [--against EDITOR.Map.Gbx]` — the frame-0 HDR texels of an image region (per texel: the
//! chart it belongs to, or "-" for a gutter texel) and, with `--against`, the other map's texels beside them — the direct look at a
//! spill (E3 2026-09-28: the Stadium giant's flag-pole item AI06220000 lands on tile 20095's 2×2 chart at (808, 730)).
fn load(p: &str) -> (lightmap::img::Rgb, Vec<(u32, u32, u32, u32, u32)>, Vec<(f32, u8)>) {
    let map = lightmap::mapio::load(p).unwrap_or_else(|e| panic!("{e}"));
    let d = map.chunk.data.as_ref().expect("no lightmap");
    let m = d.cache.mapping().expect("no mapping");
    let img = lightmap::img::decode_webp(d.frames[0].images.first().expect("image 0")).unwrap_or_else(|e| panic!("{e}"));
    let maxhdr = lightmap::classcmp::record_maxhdr(&m, 0).expect("record");
    let mut rects = Vec::new();
    let mut fbs = Vec::new();
    for i in 0..m.count as usize {
        let (px, py, pw, ph) = lightmap::classcmp::chart_own_px(m.pos[i], m.size[i]);
        rects.push((i as u32, px, py, pw, ph));
        fbs.push((maxhdr, m.frame_bytes[0][i]));
    }
    (img, rects, fbs)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let at = f("--at").expect("--at X0,Y0:X1,Y1");
    let (p0, p1) = at.split_once(':').expect("X0,Y0:X1,Y1");
    let c = |s: &str| -> (u32, u32) { let v: Vec<u32> = s.split(',').map(|x| x.parse().expect("n")).collect(); (v[0], v[1]) };
    let ((x0, y0), (x1, y1)) = (c(p0), c(p1));
    let (img, rects, fbs) = load(&a[1]);
    let other = f("--against").map(|p| load(&p));
    let chart_at = |rects: &[(u32, u32, u32, u32, u32)], x: u32, y: u32| rects.iter().find(|r| x >= r.1 && x < r.1 + r.3 && y >= r.2 && y < r.2 + r.4).map(|r| r.0);
    let hdr = |img: &lightmap::img::Rgb, fbs: &[(f32, u8)], ch: Option<u32>, x: u32, y: u32| -> [f64; 3] {
        let idx = ((y * img.w + x) * 3) as usize;
        let (maxhdr, fb) = ch.map(|c| fbs[c as usize]).unwrap_or((fbs[0].0, 0));
        [0, 1, 2].map(|k| lightmap::classcmp::texel_hdr(0, img.px[idx + k], fb, maxhdr))
    };
    for y in y0..=y1 {
        for x in x0..=x1 {
            let ch = chart_at(&rects, x, y);
            let v = hdr(&img, &fbs, ch, x, y);
            let chs = ch.map(|c| c.to_string()).unwrap_or_else(|| "-".into());
            let mut line = format!("({x},{y}) chart {:>6}  ours ({:.3},{:.3},{:.3})", chs, v[0], v[1], v[2]);
            if let Some((oimg, orects, ofbs)) = &other {
                let och = chart_at(orects, x, y);
                let ov = hdr(oimg, ofbs, och, x, y);
                line += &format!("  editor chart {:>6} ({:.3},{:.3},{:.3})", och.map(|c| c.to_string()).unwrap_or_else(|| "-".into()), ov[0], ov[1], ov[2]);
            }
            println!("{line}");
        }
    }
}
