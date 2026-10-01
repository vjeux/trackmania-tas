//! `e8_capturelayout VB.bin [--map MAP.Map.Gbx] [--records LAYOUT.tsv] [--w 1024]` — the CAPTURE's own chart layout from the SET
//! draws' per-instance stream (10 948 × 48 B: TEXCOORD5 quat, TEXCOORD6 translation + scale, TEXCOORD7 ST = (sx, sy, tx, ty) with
//! the game's ST = ((w − 1)/W, (x + 0.5)/W) convention → rect texel x = tx·W − 0.5, w = sx·W + 1; layout units ×2), its size
//! histogram, and — with --map — the match against a mapping's rects by position (the capture's record order is not the file's)
//! (E8, 2026-10-01: did the capture bake the shipped x2 or the bake copy?).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 { eprintln!("usage: e8_capturelayout VB.bin [--map MAP] [--w 1024]"); std::process::exit(2); }
    let flag = |n: &str| a.iter().position(|x| x == n).and_then(|i| a.get(i + 1).cloned());
    let w_tex: f32 = flag("--w").map(|v| v.parse().unwrap()).unwrap_or(1024.0);
    let vb = std::fs::read(&a[1]).expect("vb");
    let n = vb.len() / 48;
    let f = |i: usize, k: usize| -> f32 { f32::from_le_bytes(vb[i * 48 + k * 4..i * 48 + k * 4 + 4].try_into().unwrap()) };
    println!("{n} instances");
    for i in [0usize, 1, 2, 269, 270, 271, 9485, 9486, 10033, 10034, 10947] { if i < n { println!("  #{i}: quat ({:.4}, {:.4}, {:.4}, {:.4}) trans ({:.2}, {:.2}, {:.2}) scale {:.4} ST ({:.6}, {:.6}, {:.6}, {:.6})", f(i, 0), f(i, 1), f(i, 2), f(i, 3), f(i, 4), f(i, 5), f(i, 6), f(i, 7), f(i, 8), f(i, 9), f(i, 10), f(i, 11)); } }
    // rects in layout units: x = 2·(tx·W − 0.5) + 1 → the mapping's pos convention: stored texel px = (X + 1)/2 → X = 2·px − 1; px = tx·W − 0.5 + 0.5? The game's
    // ST.z = (x_tex + 0.5)/W where x_tex is the rect's first texel → x_tex = tx·W − 0.5; the mapping pos X = 2·x_tex + 1 (odd); size W_l = 2·w_tex where
    // w_tex = sx·W + 1.
    let mut rects: Vec<(i32, i32, i32, i32)> = Vec::with_capacity(n);
    let mut hist: std::collections::HashMap<(i32, i32), usize> = Default::default();
    for i in 0..n {
        let (sx, sy, tx, ty) = (f(i, 8), f(i, 9), f(i, 10), f(i, 11));
        let x_tex = tx * w_tex - 0.5; let y_tex = ty * w_tex - 0.5;
        let wt = sx * w_tex + 1.0; let ht = sy * w_tex + 1.0;
        let r = ((2.0 * x_tex + 1.0).round() as i32, (2.0 * y_tex + 1.0).round() as i32, (2.0 * wt).round() as i32, (2.0 * ht).round() as i32);
        rects.push(r);
        *hist.entry((r.2, r.3)).or_default() += 1;
    }
    let mut hv: Vec<_> = hist.into_iter().collect(); hv.sort_by(|x, y| y.1.cmp(&x.1));
    println!("capture chart size histogram (layout units, top 16): {:?}", hv.iter().take(16).collect::<Vec<_>>());
    println!("first rects: {:?}", rects.iter().take(6).collect::<Vec<_>>());
    // the record ranges: runs of records sharing (ST size class, trans.y) — the capture's record set structure
    {
        let key = |i: usize| -> (i32, i32, i32) { ((f(i, 8) * 2048.0 * 100.0).round() as i32, (f(i, 9) * 2048.0 * 100.0).round() as i32, (f(i, 5) * 10.0).round() as i32) };
        let mut runs: Vec<(usize, usize, (i32, i32, i32))> = Vec::new();
        let mut start = 0;
        for i in 1..=n { if i == n || key(i) != key(start) { runs.push((start, i - 1, key(start))); start = i; } }
        println!("{} runs of identical (ST.x·2048 ×100, ST.y·2048 ×100, trans.y ×10); the first 12 and every run ≥ 100 long:", runs.len());
        for (k, r) in runs.iter().enumerate() { if k < 12 || r.1 - r.0 + 1 >= 100 { println!("  records {}..{} ({}): size·100 ({}, {}) trans.y {:.1} first trans ({:.1}, {:.1}, {:.1})", r.0, r.1, r.1 - r.0 + 1, r.2 .0, r.2 .1, r.2 .2 as f32 / 10.0, f(r.0, 4), f(r.0, 5), f(r.0, 6)); } }
    }
    if let Some(mp) = flag("--map") {
        let lb = lightmap::mapio::load(&mp).unwrap_or_else(|e| panic!("{mp}: {e}"));
        let db = lb.chunk.data.as_ref().expect("no lightmap");
        let mb = db.cache.mapping().expect("no mapping");
        let mut set: std::collections::HashSet<(i32, i32, i32, i32)> = Default::default();
        for c in 0..mb.count as usize { set.insert((mb.pos[c].0 as i32, mb.pos[c].1 as i32, mb.size[c].0 as i32, mb.size[c].1 as i32)); }
        let hit = rects.iter().filter(|r| set.contains(r)).count();
        let mut sizes: std::collections::HashSet<(i32, i32)> = Default::default();
        for c in 0..mb.count as usize { sizes.insert((mb.size[c].0 as i32, mb.size[c].1 as i32)); }
        let hit_size = rects.iter().filter(|r| sizes.contains(&(r.2, r.3))).count();
        println!("vs {mp}: {hit} of {n} capture rects exist in the mapping (position + size); {hit_size} have a size the mapping has");
        let miss: Vec<_> = rects.iter().enumerate().filter(|(_, r)| !set.contains(r)).take(8).collect();
        println!("  first capture rects absent from the mapping: {miss:?}");
        // RE 18's rule: origin = ST.zw × 2048 (the SET target's texel grid = the layout grid), size = box × ST.xy × 2048 with box = the
        // model's PreLightGen uv extent (unknown here) → match by ORIGIN within ±1 unit, then read the implied box = size/(ST.x·2048)
        let mut by_pos: std::collections::HashMap<(i32, i32), Vec<usize>> = Default::default();
        for c in 0..mb.count as usize { by_pos.entry((mb.pos[c].0 as i32, mb.pos[c].1 as i32)).or_default().push(c); }
        let (mut found, mut ambiguous) = (0usize, 0usize);
        let mut boxes: Vec<f32> = Vec::new();
        let mut matched_charts: std::collections::HashSet<usize> = Default::default();
        let mut examples: Vec<String> = Vec::new();
        for i in 0..n {
            let (sx, sy, tx, ty) = (f(i, 8), f(i, 9), f(i, 10), f(i, 11));
            let (x, y) = ((tx * 2048.0).round() as i32, (ty * 2048.0).round() as i32);
            let mut cands: Vec<usize> = Vec::new();
            for dx in -1..=1 { for dy in -1..=1 { if let Some(v) = by_pos.get(&(x + dx, y + dy)) { cands.extend(v.iter().copied()); } } }
            if cands.is_empty() { if examples.len() < 6 { examples.push(format!("#{i} origin ({x}, {y}) ST ({sx:.6}, {sy:.6}) trans ({:.1}, {:.1}, {:.1}): no chart", f(i, 4), f(i, 5), f(i, 6))); } continue; }
            if cands.len() > 1 { ambiguous += 1; }
            let c = cands[0];
            found += 1;
            matched_charts.insert(c);
            boxes.push(mb.size[c].0 as f32 / (sx * 2048.0));
            if examples.len() < 6 && i % 2000 == 0 { examples.push(format!("#{i} origin ({x}, {y}) → chart {c} obj {} pos ({}, {}) size {}×{} → box_x {:.4}", mb.binds[c].obj_group_idx / 4, mb.pos[c].0, mb.pos[c].1, mb.size[c].0, mb.size[c].1, mb.size[c].0 as f32 / (sx * 2048.0))); }
        }
        boxes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("by origin ±1: {found} of {n} capture records find a mapping chart ({ambiguous} ambiguous; {} distinct charts matched); implied box_x percentiles 5/50/95: {:.4} {:.4} {:.4}", matched_charts.len(), boxes.get(boxes.len() / 20).unwrap_or(&0.0), boxes.get(boxes.len() / 2).unwrap_or(&0.0), boxes.get(boxes.len() * 19 / 20).unwrap_or(&0.0));
        for e in &examples { println!("  {e}"); }
    }
}
