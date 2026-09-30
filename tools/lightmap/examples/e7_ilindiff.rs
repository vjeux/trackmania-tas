//! `e7_ilindiff GAME.tsv OURS.tsv [--top N] [--min-texels N]` — two `e7_ilin --all-names --tsv` tables joined by name: per emitter class
//! the texel count, the game's and our mean ILightInput, ours/game per channel, sorted by the summed absolute difference weighted by
//! the texel count (the emitters whose sweep-0 colour differs most, in the order they matter) (E7, 2026-09-30).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e7_ilindiff GAME.tsv OURS.tsv [--top N] [--min-texels N]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let top: usize = f("--top").and_then(|v| v.parse().ok()).unwrap_or(40);
    let min_tex: usize = f("--min-texels").and_then(|v| v.parse().ok()).unwrap_or(100);
    let load = |p: &str| -> std::collections::HashMap<String, (usize, usize, usize, [f64; 3])> {
        std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}")).lines().skip(1).filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() < 10 { return None; }
            Some((c[0].to_string(), (c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?, [c[4].parse().ok()?, c[5].parse().ok()?, c[6].parse().ok()?])))
        }).collect()
    };
    let g = load(&a[1]);
    let o = load(&a[2]);
    let mut rows: Vec<(f64, String, usize, usize, usize, [f64; 3], [f64; 3])> = Vec::new();
    let (mut tot_g, mut tot_o, mut tot_n) = ([0f64; 3], [0f64; 3], 0usize);
    for (name, (charts, n, nz_g, mg)) in &g {
        let Some((_, _, nz_o, mo)) = o.get(name) else { continue };
        if *n < min_tex { continue; }
        for k in 0..3 { tot_g[k] += mg[k] * *n as f64; tot_o[k] += mo[k] * *n as f64; }
        tot_n += n;
        let w = *n as f64 * (0..3).map(|k| (mo[k] - mg[k]).abs()).sum::<f64>();
        rows.push((w, name.clone(), *charts, *n, *nz_g, *mg, *mo));
        let _ = nz_o;
    }
    rows.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    println!("{} classes ≥ {min_tex} texels; whole-atlas texel-weighted mean: game ({:.5}, {:.5}, {:.5}) ours ({:.5}, {:.5}, {:.5})", rows.len(), tot_g[0] / tot_n as f64, tot_g[1] / tot_n as f64, tot_g[2] / tot_n as f64, tot_o[0] / tot_n as f64, tot_o[1] / tot_n as f64, tot_o[2] / tot_n as f64);
    println!("{:<28} {:>7} {:>8}  {:<26} {:<26} {:<20} {:>10}", "name", "charts", "texels", "game mean", "ours mean", "ours/game", "Σ|Δ|·n");
    for (w, name, charts, n, _nz, mg, mo) in rows.iter().take(top) {
        let r: Vec<String> = (0..3).map(|k| if mg[k] > 1e-6 { format!("{:.2}", mo[k] / mg[k]) } else { "-".into() }).collect();
        println!("{:<28} {:>7} {:>8}  ({:.4}, {:.4}, {:.4})  ({:.4}, {:.4}, {:.4})  {:<20} {:>10.1}", name, charts, n, mg[0], mg[1], mg[2], mo[0], mo[1], mo[2], r.join("/"), w);
    }
}
