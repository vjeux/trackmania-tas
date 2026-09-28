//! `e5_recbox records.tsv` — the union of the layout records' world boxes (centre ± half from a `--records-tsv` dump), per class and
//! overall: the SCENE BOX the game would fold from its records (lmtiles::scene_box) beside the LM-vertex box the port prints as
//! "scene box" — which bottom bounds the world peel box W on a giant (E5, 2026-09-28 22:50Z; the coordinator's W.ymin question).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let txt = std::fs::read_to_string(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let mut per: std::collections::BTreeMap<String, ([f32; 3], [f32; 3], usize)> = std::collections::BTreeMap::new();
    let mut lowest: Vec<(f32, String, String)> = Vec::new();
    for (i, line) in txt.lines().enumerate() {
        if i == 0 || line.starts_with('#') { continue; }
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 12 { continue; }
        let p = |k: usize| -> f32 { c[k].trim().parse::<f32>().unwrap_or(f32::NAN) };
        let (cy, cx, cz, hx, hy, hz) = (p(6), p(7), p(8), p(9), p(10), p(11));
        if !cy.is_finite() || !hy.is_finite() { continue; }
        let lo = [cx - hx, cy - hy, cz - hz];
        let hi = [cx + hx, cy + hy, cz + hz];
        let e = per.entry(c[1].to_string()).or_insert(([f32::MAX; 3], [f32::MIN; 3], 0));
        for k in 0..3 { e.0[k] = e.0[k].min(lo[k]); e.1[k] = e.1[k].max(hi[k]); }
        e.2 += 1;
        lowest.push((lo[1], c[1].to_string(), c[4].to_string()));
    }
    let mut all = ([f32::MAX; 3], [f32::MIN; 3]);
    for (cls, (lo, hi, n)) in &per {
        println!("{cls:<8} {n:>6} records: x [{:.2}, {:.2}] y [{:.4}, {:.4}] z [{:.2}, {:.2}]", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
        for k in 0..3 { all.0[k] = all.0[k].min(lo[k]); all.1[k] = all.1[k].max(hi[k]); }
    }
    println!("ALL records: x [{:.2}, {:.2}] y [{:.4}, {:.4}] z [{:.2}, {:.2}]", all.0[0], all.1[0], all.0[1], all.1[1], all.0[2], all.1[2]);
    lowest.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    println!("the 12 lowest record bottoms:");
    for (y, cls, name) in lowest.iter().take(12) { println!("  y {y:.4}  {cls}  {name}"); }
}
