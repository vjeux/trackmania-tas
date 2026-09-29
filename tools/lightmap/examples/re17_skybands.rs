//! `re17_skybands A.dds B.dds [--bands N]` — the mood SkyColor gradients (BC6H, u = azimuth relative to the sun,
//! v = elevation with the zenith at the top row) as per-row-band means: for each of N elevation bands (horizon → zenith)
//! the mean linear rgb over all u, and its hue words G/R, B/R; then the ratio A/B per band. RE 17 2026-09-29 (the
//! WhiteShore Day vs BlueBay Day dome: the two moods' XML differ only in Latitude and SkyFactor, their textures differ).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let bands: usize = f("--bands").map(|s| s.parse().expect("N")).unwrap_or(9);
    let paths: Vec<&String> = a[1..].iter().filter(|s| s.ends_with(".dds")).collect();
    let mut all: Vec<Vec<[f64; 3]>> = Vec::new();
    for p in &paths {
        let g = lightmap::skygrad::SkyGradient::load(p).expect("SkyColor.dds");
        println!("{p}: {}×{} texels", g.w, g.h);
        let mut rows: Vec<[f64; 3]> = Vec::new();
        for b in 0..bands {
            // band b: rows from the bottom (horizon) up; v_top_is_zenith → row 0 = zenith, so the horizon is the last rows
            let y0 = g.h - (b + 1) * g.h / bands;
            let y1 = g.h - b * g.h / bands;
            let mut s = [0.0f64; 3];
            let mut n = 0usize;
            for y in y0..y1 {
                for x in 0..g.w {
                    let t = g.px[y * g.w + x];
                    s[0] += t[0] as f64;
                    s[1] += t[1] as f64;
                    s[2] += t[2] as f64;
                    n += 1;
                }
            }
            let m = [s[0] / n as f64, s[1] / n as f64, s[2] / n as f64];
            let el0 = 90.0 * b as f64 / bands as f64;
            let el1 = 90.0 * (b + 1) as f64 / bands as f64;
            println!("  el {el0:5.1}–{el1:5.1}°: mean ({:.4}, {:.4}, {:.4})  G/R {:.3}  B/R {:.3}", m[0], m[1], m[2], m[1] / m[0], m[2] / m[0]);
            rows.push(m);
        }
        all.push(rows);
    }
    if a.iter().any(|x| x == "--irradiance") {
        for p in &paths {
            let g = lightmap::skygrad::SkyGradient::load(p).expect("SkyColor.dds");
            for v_sin in [false, true] {
                let (eh, ev) = irradiance(&g, v_sin);
                println!("{p} v_sin={v_sin}: E_horizontal ({:.3}, {:.3}, {:.3}) G/R {:.3} B/R {:.3} | E_vertical ({:.3}, {:.3}, {:.3}) G/R {:.3} B/R {:.3} | vertical/horizontal {:.3}", eh[0], eh[1], eh[2], eh[1]/eh[0], eh[2]/eh[0], ev[0], ev[1], ev[2], ev[1]/ev[0], ev[2]/ev[0], ev[0]/eh[0]);
            }
        }
    }
    if all.len() == 2 {
        println!("ratio {} / {} per band:", paths[0], paths[1]);
        for b in 0..bands {
            let (p, q) = (all[0][b], all[1][b]);
            println!("  band {b}: ({:.3}, {:.3}, {:.3})", p[0] / q[0], p[1] / q[1], p[2] / q[2]);
        }
    }
}

/// Hemispheric irradiance of the az-averaged gradient on a horizontal (up) surface and on a vertical face, for the two
/// candidate v mappings (row ↔ elevation linear in v, or v = sin(elevation)); printed by `--irradiance`.
#[allow(dead_code)]
pub fn irradiance(g: &lightmap::skygrad::SkyGradient, v_sin: bool) -> ([f64; 3], [f64; 3]) {
    let mut eh = [0.0f64; 3];
    let mut ev = [0.0f64; 3];
    let n = 900usize; // elevation steps of 0.1°
    for i in 0..n {
        let el = (i as f64 + 0.5) / n as f64 * std::f64::consts::FRAC_PI_2;
        let v = if v_sin { el.sin() } else { el / std::f64::consts::FRAC_PI_2 }; // 0 = horizon, 1 = zenith
        let y = (((1.0 - v) * g.h as f64) as usize).min(g.h - 1); // row 0 = zenith
        let mut m = [0.0f64; 3];
        for x in 0..g.w {
            let t = g.px[y * g.w + x];
            m[0] += t[0] as f64;
            m[1] += t[1] as f64;
            m[2] += t[2] as f64;
        }
        for c in 0..3 {
            m[c] /= g.w as f64;
        }
        let del = std::f64::consts::FRAC_PI_2 / n as f64;
        for c in 0..3 {
            eh[c] += m[c] * el.sin() * el.cos() * del * 2.0 * std::f64::consts::PI; // ∫ L cos θ dΩ, θ = 90° − el
            ev[c] += m[c] * el.cos() * el.cos() * del * 2.0; // ∫ over az ∈ (−90°, 90°) of cos(az) = 2
        }
    }
    (eh, ev)
}
