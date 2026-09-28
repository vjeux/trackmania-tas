//! `e3_tracesum BAKE.log` — the per-direction CONTRIBUTION DUMP of one texel, from a `LMTOOL_SET_TEXEL_TRACE` bake log
//! (E3, 2026-09-28): every `set-texel-trace: dir k (dx,dy,dz) <phase> peel → L Some([r,g,b]) via <source>` line is one
//! direction's read for the traced texel; the texel's normal comes from its `frag 0: … n (nx,ny,nz)` line. The read is
//! weighted by the accumulate's cosine max(n·d, 0) and summed per SOURCE CLASS (dome / env-surface / item layer / none),
//! so the question "which surface supplies the R excess" reads off one table. Directions traced in both the world and the
//! FITTED peel count the fitted one (it overwrites where the texel is inside the fitted box).
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let txt = std::fs::read_to_string(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let mut normal: Option<[f32; 3]> = None;
    // dir index → (d, phase, read, source)
    let mut dirs: BTreeMap<u32, ([f32; 3], String, Option<[f32; 3]>, String)> = BTreeMap::new();
    let num = |s: &str| -> f32 { s.trim().trim_matches(|c| c == '(' || c == ')' || c == '[' || c == ']' || c == ',').parse::<f32>().unwrap_or(f32::NAN) };
    for line in txt.lines() {
        if normal.is_none() && line.contains("frag 0:") {
            if let Some(i) = line.find(" n (") {
                let rest = &line[i + 4..];
                let end = rest.find(')').unwrap_or(rest.len());
                let v: Vec<f32> = rest[..end].split(',').map(num).collect();
                if v.len() == 3 { normal = Some([v[0], v[1], v[2]]); }
            }
        }
        let Some(rest) = line.strip_prefix("set-texel-trace: dir ") else { continue };
        if !rest.contains(" → L ") { continue; }
        // "k (dx,dy,dz) PHASE peel → L Some([r, g, b]) via SRC" | "→ L None via none"
        let mut it = rest.splitn(2, ' ');
        let k: u32 = it.next().unwrap_or("").parse().unwrap_or(u32::MAX);
        let rest = it.next().unwrap_or("");
        let close = rest.find(')').unwrap_or(0);
        let dv: Vec<f32> = rest[1..close].split(',').map(num).collect();
        if dv.len() != 3 { continue; }
        let phase = if rest.contains("FITTED peel") { "fitted" } else { "world" };
        let read = if let Some(i) = rest.find("Some([") {
            let r = &rest[i + 6..];
            let e = r.find(']').unwrap_or(r.len());
            let v: Vec<f32> = r[..e].split(',').map(num).collect();
            if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None }
        } else { None };
        let src = rest.find(" via ").map(|i| rest[i + 5..].trim().to_string()).unwrap_or_else(|| "?".into());
        let class = if src.starts_with("item") { if read.map_or(true, |c| c.iter().all(|v| *v == 0.0)) { "item (black)".to_string() } else { "item (lit)".to_string() } } else if src.starts_with("env-surface") { if read.map_or(true, |c| c.iter().all(|v| *v == 0.0)) { "env-surface (black)".to_string() } else { "env-surface (lit)".to_string() } } else { src.clone() };
        let e = dirs.entry(k).or_insert(([dv[0], dv[1], dv[2]], phase.into(), read, class.clone()));
        // the fitted phase overwrites the world's
        if phase == "fitted" || e.1 != "fitted" { *e = ([dv[0], dv[1], dv[2]], phase.into(), read, class); }
    }
    let n = normal.unwrap_or([0.0, 1.0, 0.0]);
    println!("{}: {} directions traced, texel normal ({:.3},{:.3},{:.3})", a[1], dirs.len(), n[0], n[1], n[2]);
    let mut per: BTreeMap<String, (usize, f64, [f64; 3], [f64; 3])> = BTreeMap::new();
    let (mut wsum, mut tot) = (0.0f64, [0.0f64; 3]);
    for (_, (d, _, read, class)) in &dirs {
        let w = (n[0] * d[0] + n[1] * d[1] + n[2] * d[2]).max(0.0) as f64;
        let e = per.entry(class.clone()).or_insert((0, 0.0, [0.0; 3], [0.0; 3]));
        e.0 += 1;
        e.1 += w;
        if let Some(c) = read {
            for ch in 0..3 { e.2[ch] += w * c[ch] as f64; e.3[ch] += c[ch] as f64; tot[ch] += w * c[ch] as f64; }
        }
        wsum += w;
    }
    println!("{:22} {:>5} {:>8} {:>26} {:>26} {:>8}", "source", "dirs", "Σcos", "Σ cos·L (r g b)", "mean L (r g b)", "share");
    for (class, (cnt, w, s, raw)) in &per {
        let share = if tot.iter().sum::<f64>() > 0.0 { s.iter().sum::<f64>() / tot.iter().sum::<f64>() } else { 0.0 };
        println!("{:22} {:>5} {:>8.3} ({:>7.4} {:>7.4} {:>7.4}) ({:>7.4} {:>7.4} {:>7.4}) {:>7.1}%", class, cnt, w, s[0], s[1], s[2], raw[0] / *cnt as f64, raw[1] / *cnt as f64, raw[2] / *cnt as f64, 100.0 * share);
    }
    println!("TOTAL Σcos {wsum:.3}, Σ cos·L ({:.4} {:.4} {:.4}) → the texel's mean irradiance estimate (Σ cos·L / Σcos) ({:.4} {:.4} {:.4}); r/b {:.3}", tot[0], tot[1], tot[2], tot[0] / wsum.max(1e-9), tot[1] / wsum.max(1e-9), tot[2] / wsum.max(1e-9), tot[0] / tot[2].max(1e-9));
    // the per-direction list by elevation band (upward = d.y > 0)
    let mut bands: BTreeMap<i32, (usize, [f64; 3], f64)> = BTreeMap::new();
    for (_, (d, _, read, _)) in &dirs {
        let el = (d[1].asin().to_degrees() / 15.0).floor() as i32;
        let w = (n[0] * d[0] + n[1] * d[1] + n[2] * d[2]).max(0.0) as f64;
        let e = bands.entry(el).or_insert((0, [0.0; 3], 0.0));
        e.0 += 1;
        e.2 += w;
        if let Some(c) = read { for ch in 0..3 { e.1[ch] += w * c[ch] as f64; } }
    }
    println!("by elevation band (15°): band dirs Σcos Σcos·L");
    for (b, (cnt, s, w)) in &bands { println!("  {:>4}°..{:>4}° {:>4} {:>7.3} ({:.4} {:.4} {:.4})", b * 15, b * 15 + 15, cnt, w, s[0], s[1], s[2]); }
}
