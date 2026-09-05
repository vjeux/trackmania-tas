//! Colour census of a screenshot region: how much of it is grey (road asphalt)
//! versus green/blue/sky. A blind check for "did the road draw".
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d = std::fs::read(&a[1]).unwrap();
    let img = png_decode(&d);
    let (w, h, px) = img;
    let (x0, y0, x1, y1): (usize, usize, usize, usize) = if a.len() > 5 { (a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()) } else { (0, 0, w, h) };
    let (mut grey, mut green, mut blue, mut other, mut n) = (0, 0, 0, 0, 0);
    let mut red = 0;
    for y in y0..y1.min(h) { for x in x0..x1.min(w) {
        let i = (y * w + x) * 4; let (r, g, b) = (px[i] as i32, px[i + 1] as i32, px[i + 2] as i32);
        n += 1;
        let mx = r.max(g).max(b); let mn = r.min(g).min(b);
        if r > 150 && g < 80 && b < 80 { red += 1 } else if mx - mn < 18 && mx > 60 && mx < 200 { grey += 1 } else if g > r + 15 && g > b + 15 { green += 1 } else if b > r + 15 && b >= g { blue += 1 } else { other += 1 }
    }}
    println!("{}x{} region {}..{} x {}..{}: red {:.2}% grey {:.1}% green {:.1}% blue {:.1}% other {:.1}%", w, h, x0, x1, y0, y1, 100.0 * red as f64 / n as f64, 100.0 * grey as f64 / n as f64, 100.0 * green as f64 / n as f64, 100.0 * blue as f64 / n as f64, 100.0 * other as f64 / n as f64);
}
fn png_decode(d: &[u8]) -> (usize, usize, Vec<u8>) {
    // minimal PNG: 8-bit RGB/RGBA, non-interlaced, zlib via miniz-free inflate
    let mut o = 8; let (mut w, mut h, mut ct) = (0usize, 0usize, 0u8); let mut idat = Vec::new();
    while o + 8 <= d.len() {
        let len = u32::from_be_bytes(d[o..o + 4].try_into().unwrap()) as usize; let ty = &d[o + 4..o + 8]; let body = &d[o + 8..o + 8 + len];
        match ty { b"IHDR" => { w = u32::from_be_bytes(body[0..4].try_into().unwrap()) as usize; h = u32::from_be_bytes(body[4..8].try_into().unwrap()) as usize; ct = body[9]; assert_eq!(body[8], 8); assert_eq!(body[12], 0); }
                   b"IDAT" => idat.extend_from_slice(body), b"IEND" => break, _ => {} }
        o += 12 + len;
    }
    let raw = mapgeom_inflate(&idat[2..]);
    let bpp = if ct == 6 { 4 } else { 3 }; let stride = w * bpp; let mut out = vec![0u8; w * h * 4]; let mut prev = vec![0u8; stride]; let mut cur = vec![0u8; stride]; let mut p = 0;
    for y in 0..h { let f = raw[p]; p += 1; cur.copy_from_slice(&raw[p..p + stride]); p += stride;
        for i in 0..stride { let a = if i >= bpp { cur[i - bpp] } else { 0 } as i32; let b = prev[i] as i32; let c = if i >= bpp { prev[i - bpp] } else { 0 } as i32;
            let x = cur[i] as i32; let v = match f { 0 => x, 1 => x + a, 2 => x + b, 3 => x + ((a + b) >> 1), 4 => { let pp = a + b - c; let pa = (pp - a).abs(); let pb = (pp - b).abs(); let pc = (pp - c).abs(); x + if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c } } _ => x };
            cur[i] = (v & 0xFF) as u8; }
        for x in 0..w { for k in 0..3 { out[(y * w + x) * 4 + k] = cur[x * bpp + k]; } }
        std::mem::swap(&mut prev, &mut cur); }
    (w, h, out)
}
fn mapgeom_inflate(z: &[u8]) -> Vec<u8> { miniz_oxide::inflate::decompress_to_vec(z).expect("png inflate") }
