//! `clientre lmimages MAP.Gbx [OUTDIR]` — every WEBP of the map's lightmap
//! chunk split into its RIFF sub-images (a blob may hold several files
//! back to back: frame-0 image 1 holds the three H-basis directional
//! coefficients, frame-0 image 2 the four probe images), decoded, with the
//! statistics that test the encoding model: histogram centre, share of grey
//! pixels, the 128 = zero convention of the sign-sqrt coefficients.

use std::fmt::Write as _;

/// (offset, len) of each RIFF file inside a blob.
pub fn riff_parts(b: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 12 <= b.len() && &b[i..i + 4] == b"RIFF" {
        let n = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize + 8;
        let n = n.min(b.len() - i);
        out.push((i, n));
        i += n + (n & 1);
    }
    out
}

pub fn run(path: &str, out: Option<&str>) -> Result<String, String> {
    let m = lightmap::mapio::load(path)?;
    let d = m.chunk.data.as_ref().ok_or("no lightmap data (hasLightmaps = 0)")?;
    let mut s = String::new();
    let _ = writeln!(s, "{}: lightmap version {}, {} frames", path, d.lightmap_version, d.frames.len());
    // frame records from the mapping head
    for c in &d.cache.chunks {
        if let lightmap::format::ChunkBody::Mapping(mp) = &c.body {
            let h = &mp.head;
            // head: u32 1, u32[5] 0, u32 256,64,25, u32 1,2,1,3,1,4, then three 66-byte SFrame records
            if h.len() >= 4 * 17 {
                let n = 3usize; // three records follow the constant head (no count word)
                let _ = writeln!(s, "frame records ({}):", n);
                for k in 0..n {
                    let o = 60 + 66 * k;
                    if o + 66 > h.len() {
                        break;
                    }
                    let r = &h[o..o + 66];
                    let u = |i: usize| u32::from_le_bytes(r[i..i + 4].try_into().unwrap());
                    let f = |i: usize| f32::from_le_bytes(r[i..i + 4].try_into().unwrap());
                    let h16 = |i: usize| half_to_f32(u16::from_le_bytes(r[i..i + 2].try_into().unwrap()));
                    let _ = writeln!(
                        s,
                        "  [{}] bump {} u {} daytime 0x{:04x} replay {} MaxHDR_Mood {} MaxHDR {} bounce {} sky {} clouds {} HBasis234 ({:.3}, {:.3}, {:.3}) storeLAmb {} storage {} switch {} LAmbient ({:.3}, {:.3}, {:.3})",
                        k, u(0), u(4), u(8), f(12), f(16), f(20), f(24), f(28), u(32), h16(36), h16(38), h16(40), u(42), u(46), u(50), f(54), f(58), f(62)
                    );
                }
            }
        }
    }
    if let Some(dir) = out {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    for (fi, fr) in d.frames.iter().enumerate() {
        for (ii, blob) in fr.images.iter().enumerate() {
            if blob.is_empty() {
                continue;
            }
            let parts = riff_parts(blob);
            let _ = writeln!(s, "frame {} image {}: {} bytes, {} RIFF file(s)", fi, ii, blob.len(), parts.len());
            for (k, &(o, n)) in parts.iter().enumerate() {
                let sub = &blob[o..o + n];
                let img = match lightmap::img::decode_webp(sub) {
                    Ok(i) => i,
                    Err(e) => {
                        let _ = writeln!(s, "  [{}] {} B: decode failed: {}", k, n, e);
                        continue;
                    }
                };
                let npx = (img.w * img.h) as usize;
                let mut grey = 0usize;
                let mut hist = [0usize; 8];
                let mut sum = [0f64; 3];
                let mut at128 = 0usize;
                let mut below = 0usize;
                let mut above = 0usize;
                for p in img.px.chunks(3) {
                    if (p[0] as i32 - p[1] as i32).abs() <= 2 && (p[1] as i32 - p[2] as i32).abs() <= 2 {
                        grey += 1;
                    }
                    let l = (p[0] as usize + p[1] as usize + p[2] as usize) / 3;
                    hist[l / 32] += 1;
                    for c in 0..3 {
                        sum[c] += p[c] as f64;
                    }
                    if (126..=130).contains(&l) {
                        at128 += 1;
                    } else if l < 126 {
                        below += 1;
                    } else {
                        above += 1;
                    }
                }
                let _ = writeln!(
                    s,
                    "  [{}] {} B  {}x{}  grey {:.1}%  mean ({:.1}, {:.1}, {:.1})  <126: {:.1}%  126..130: {:.1}%  >130: {:.1}%  hist/32 {:?}",
                    k,
                    n,
                    img.w,
                    img.h,
                    100.0 * grey as f64 / npx as f64,
                    sum[0] / npx as f64,
                    sum[1] / npx as f64,
                    sum[2] / npx as f64,
                    100.0 * below as f64 / npx as f64,
                    100.0 * at128 as f64 / npx as f64,
                    100.0 * above as f64 / npx as f64,
                    hist
                );
                if let Some(dir) = out {
                    let p = format!("{}/f{}i{}_{}.ppm", dir, fi, ii, k);
                    lightmap::img::write_ppm(&img, &p).map_err(|e| e.to_string())?;
                    std::fs::write(format!("{}/f{}i{}_{}.webp", dir, fi, ii, k), sub).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(s)
}

fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1f) as i32;
    let frac = (h & 0x3ff) as u32;
    let v = if exp == 0 {
        (frac as f32) * 2f32.powi(-24)
    } else if exp == 31 {
        if frac == 0 { f32::INFINITY } else { f32::NAN }
    } else {
        (1.0 + frac as f32 / 1024.0) * 2f32.powi(exp - 15)
    };
    if sign == 1 { -v } else { v }
}
