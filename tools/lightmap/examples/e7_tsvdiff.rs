//! `e7_tsvdiff A.tsv B.tsv [--min-texels N] [--top N]` — two `lmtool classcmp --by name --tsv` tables of the SAME oracle joined
//! by class: the per-class mean ratio to the editor before (A) and after (B), sorted by the size of the move, with the texel
//! counts (E7, 2026-09-30: the attribution of a patch per class — which classes' bytes moved, by how much, toward or away).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e7_tsvdiff A.tsv B.tsv [--min-texels N] [--top N] [--match SUBSTR]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let min_texels: usize = f("--min-texels").and_then(|v| v.parse().ok()).unwrap_or(30);
    let top: usize = f("--top").and_then(|v| v.parse().ok()).unwrap_or(40);
    let filt = f("--match");
    let read = |p: &str| -> std::collections::BTreeMap<String, (usize, [f32; 3], [f32; 3], [f32; 3])> {
        let txt = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let mut lines = txt.lines();
        let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
        let col = |n: &str| head.iter().position(|h| *h == n).unwrap_or_else(|| panic!("{p}: no column {n}"));
        let (ci, ct, cmo, cme, cr) = (col("class"), col("texels"), col("mean_ours_rgb"), col("mean_editor_rgb"), col("ratio_rgb"));
        let v3 = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split('/').map(|x| x.trim().parse().unwrap_or(f32::NAN)).collect(); if v.len() == 3 { [v[0], v[1], v[2]] } else { [f32::NAN; 3] } };
        let mut out = std::collections::BTreeMap::new();
        for l in lines {
            let v: Vec<&str> = l.split('\t').collect();
            if v.len() <= cr { continue; }
            out.insert(v[ci].to_string(), (v[ct].trim().parse().unwrap_or(0), v3(v[cmo]), v3(v[cme]), v3(v[cr])));
        }
        out
    };
    let ta = read(&a[1]);
    let tb = read(&a[2]);
    let mut rows: Vec<(f32, String, usize, [f32; 3], [f32; 3], [f32; 3], [f32; 3])> = Vec::new();
    for (k, (n, oa, ea, ra)) in &ta {
        let Some((nb, ob, _eb, rb)) = tb.get(k) else { continue };
        if *n < min_texels || n != nb { continue; }
        if let Some(fl) = &filt { if !k.contains(fl.as_str()) { continue; } }
        // the move: our mean's relative change per channel (the editor's mean is the same in both)
        let mv: [f32; 3] = [ob[0] / oa[0].max(1e-6) - 1.0, ob[1] / oa[1].max(1e-6) - 1.0, ob[2] / oa[2].max(1e-6) - 1.0];
        let size = mv.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        rows.push((size, k.clone(), *n, *oa, *ob, *ra, *rb));
        let _ = ea;
    }
    rows.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    println!("{} classes in both tables (≥ {min_texels} texels); the {} biggest moves (B/A − 1 of OUR mean per channel; ratio to the editor A → B):", rows.len(), top.min(rows.len()));
    println!("class\ttexels\tmove_rgb\tratio_A\tratio_B\ttoward_editor");
    for (_, k, n, oa, ob, ra, rb) in rows.iter().take(top) {
        let mv = [ob[0] / oa[0].max(1e-6) - 1.0, ob[1] / oa[1].max(1e-6) - 1.0, ob[2] / oa[2].max(1e-6) - 1.0];
        let toward = (0..3).filter(|&c| (rb[c] - 1.0).abs() < (ra[c] - 1.0).abs()).count();
        println!("{k}\t{n}\t{:+.3} / {:+.3} / {:+.3}\t{:.3} / {:.3} / {:.3}\t{:.3} / {:.3} / {:.3}\t{toward}/3", mv[0], mv[1], mv[2], ra[0], ra[1], ra[2], rb[0], rb[1], rb[2]);
    }
    // the tally: classes moved by more than 0.5 %, texels behind them
    let moved: Vec<&(f32, String, usize, [f32; 3], [f32; 3], [f32; 3], [f32; 3])> = rows.iter().filter(|r| r.0 > 0.005).collect();
    let tex: usize = moved.iter().map(|r| r.2).sum();
    let toward: usize = moved.iter().filter(|r| { let (ra, rb) = (r.5, r.6); (0..3).filter(|&c| (rb[c] - 1.0).abs() < (ra[c] - 1.0).abs()).count() >= 2 }).count();
    println!("moved > 0.5 %: {} classes / {tex} texels; {toward} of them toward the editor on ≥ 2 channels", moved.len());
}
