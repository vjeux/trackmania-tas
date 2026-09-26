//! `f_crop A.ppm B.ppm X Y W H OUT.png [SCALE]` — two same-size P6 images side by side (A | B | 4·|A−B|), cropped to the rect and
//! scaled up by SCALE (nearest), as a PNG (engineer F: eyeballing the frame-1 compose against the editor's WebP).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |p: &str| -> (usize, usize, Vec<u8>) {
        let d = std::fs::read(p).expect("read");
        let mut idx = 0;
        let mut fields = Vec::new();
        while fields.len() < 4 {
            let s = idx;
            while d[idx] != b' ' && d[idx] != b'\n' { idx += 1; }
            fields.push(std::str::from_utf8(&d[s..idx]).unwrap().to_string());
            idx += 1;
        }
        (fields[1].parse().unwrap(), fields[2].parse().unwrap(), d[idx..].to_vec())
    };
    let (w, _h, pa) = load(&a[1]);
    let (_, _, pb) = load(&a[2]);
    let (x0, y0, cw, ch): (usize, usize, usize, usize) = (a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap());
    let scale: usize = a.get(8).map(|s| s.parse().unwrap()).unwrap_or(4);
    let (ow, oh) = ((cw * 3 + 8) * scale, ch * scale);
    let mut out = vec![0u8; ow * oh * 3];
    for y in 0..ch {
        for x in 0..cw {
            let i = ((y0 + y) * w + x0 + x) * 3;
            let pa3 = [pa[i], pa[i + 1], pa[i + 2]];
            let pb3 = [pb[i], pb[i + 1], pb[i + 2]];
            let d3 = [0, 1, 2].map(|k| ((pa3[k] as i32 - pb3[k] as i32).abs() * 4).min(255) as u8);
            for (panel, px) in [(0usize, pa3), (1, pb3), (2, d3)] {
                for sy in 0..scale {
                    for sx in 0..scale {
                        let ox = (panel * (cw + 4) + x) * scale + sx;
                        let oy = y * scale + sy;
                        let o = (oy * ow + ox) * 3;
                        out[o..o + 3].copy_from_slice(&px);
                    }
                }
            }
        }
    }
    lightmap::png::write_rgb(&a[7], ow as u32, oh as u32, &out).expect("png");
    println!("→ {} ({ow}×{oh})", a[7]);
}
