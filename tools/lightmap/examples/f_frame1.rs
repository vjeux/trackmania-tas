//! `f_frame1 MAP OUTBASE` — the map's frame-1 image (the local-light frame): writes OUTBASE.webp and OUTBASE.rgb (decoded),
//! prints its size, the grey check (r = g = b?), a 16-bin histogram of the red channel and the count of saturated texels
//! (engineer F).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = lightmap::mapio::load(&a[1]).expect("map");
    let d = m.chunk.data.as_ref().expect("data");
    let f1 = d.frames.get(1).expect("no frame 1");
    let blob = &f1.images[0];
    std::fs::write(format!("{}.webp", a[2]), blob).expect("write");
    let im = lightmap::img::decode_webp(blob).expect("decode");
    std::fs::write(format!("{}.rgb", a[2]), &im.px).expect("write");
    let n = (im.w * im.h) as usize;
    let mut grey = 0usize;
    let mut hist = [0usize; 16];
    let mut sat = 0usize;
    let mut nz = 0usize;
    let mut ratio: std::collections::BTreeMap<(i32, i32), usize> = Default::default();
    for i in 0..n {
        let (r, g, b) = (im.px[3 * i], im.px[3 * i + 1], im.px[3 * i + 2]);
        if r == g && g == b { grey += 1; }
        hist[(r / 16) as usize] += 1;
        if r == 255 { sat += 1; }
        if r > 0 || g > 0 || b > 0 { nz += 1; }
        if r > 32 { *ratio.entry(((g as i32 - r as i32), (b as i32 - r as i32))).or_default() += 1; }
    }
    println!("frame 1 image 0: {} bytes WEBP → {}×{}; grey (r=g=b) {grey}/{n}; non-zero {nz}; r == 255: {sat}", blob.len(), im.w, im.h);
    println!("red histogram (16 bins): {hist:?}");
    let mut r: Vec<((i32, i32), usize)> = ratio.into_iter().collect();
    r.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("(g − r, b − r) of texels with r > 32, top 12: {:?}", &r[..r.len().min(12)]);
    for lo in [64u8, 128, 192] {
        let (mut sr, mut sg, mut sb) = (0u64, 0u64, 0u64);
        for i in 0..n { let (r, g, b) = (im.px[3 * i], im.px[3 * i + 1], im.px[3 * i + 2]); if r >= lo { sr += r as u64; sg += g as u64; sb += b as u64; } }
        println!("texels with r >= {lo}: Σg/Σr {:.4} Σb/Σr {:.4} (linear lamp colour 0.9834 / 0.9376; sqrt 0.9917 / 0.9683)", sg as f64 / sr as f64, sb as f64 / sr as f64);
    }
}
