// E2 scratch: per covering direction at (676,84): our final L (from the SET trace) → the C0 increment 2·(4π/N)·L·basis(sz), vs the game's Δ per banked interval
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let log = std::fs::read_to_string(&a[1]).unwrap();
    let tsv = std::fs::read_to_string(&a[2]).unwrap();
    let n = [-0.336f32, 0.592, -0.667]; // the fragment's normal at (676,84) (both fragments)
    let nn = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt(); let n = [n[0] / nn, n[1] / nn, n[2] / nn];
    // ours: per (sweep-0 direction) the world and fitted L; final = fitted if Some else world
    let mut ours: std::collections::BTreeMap<u32, ([f32; 3], Option<[f32; 3]>, Option<[f32; 3]>)> = std::collections::BTreeMap::new();
    let mut seen_first_sweep = std::collections::HashSet::new();
    let mut sweep0 = true;
    for l in log.lines() {
        let Some(rest) = l.strip_prefix("set-texel-trace: dir ") else { continue };
        if !rest.contains("→ L ") { continue; }
        let idx: u32 = rest[..rest.find(' ').unwrap()].parse().unwrap();
        let p = rest.find('(').unwrap(); let q = rest.find(')').unwrap();
        let v: Vec<f32> = rest[p + 1..q].split(',').map(|s| s.trim().parse().unwrap()).collect();
        let d = [v[0], v[1], v[2]];
        let lval: Option<[f32; 3]> = if rest.contains("→ L None") { None } else { let s = &rest[rest.find("Some([").unwrap() + 6..]; let s = &s[..s.find(']').unwrap()]; let c: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); Some([c[0], c[1], c[2]]) };
        let fitted = rest.contains("FITTED");
        // sweep 0 = until an index repeats
        if !fitted && !seen_first_sweep.insert(idx) { sweep0 = false; }
        if !sweep0 { continue; }
        let e = ours.entry(idx).or_insert((d, None, None));
        if fitted { e.2 = lval; } else { e.1 = lval; }
    }
    // the game's tsv: dir_idx → Δrgb, Δa (banked intervals)
    let mut game: Vec<(u32, [f32; 3], f32)> = Vec::new();
    for l in tsv.lines().skip(2) { let c: Vec<&str> = l.split('\t').collect(); if c.len() > 11 { if let Ok(i) = c[0].parse::<u32>() { let da: f32 = c[10].parse().unwrap_or(0.0); if da != 0.0 && i < 100000 { game.push((i, [c[7].parse().unwrap(), c[8].parse().unwrap(), c[9].parse().unwrap()], da)); } } } }
    let basis = |d: [f32; 3]| -> f32 { let sz = d[0] * n[0] + d[1] * n[1] + d[2] * n[2]; 0.093506 * (3.0 * sz * sz - 1.0) + 0.398928 * sz + 0.199472 };
    let k = 4.0 * std::f32::consts::PI / 256.0;
    println!("{:>4} {:>30} {:>8} {:>8}  {:>26}  {:>26}  {}", "dir", "D", "sz", "basis", "ours ΔC0 (2 frags)", "game Δ (banked interval)", "ratio G");
    let (mut so, mut sg) = ([0f32; 3], [0f32; 3]);
    let mut prev_game_end = 0u32;
    for (gi, drgb, da) in &game {
        // our directions inside the banked interval (prev, gi]
        let mut oursum = [0f32; 3]; let mut names = Vec::new();
        for (idx, (d, w, f)) in ours.range(prev_game_end + 1..=*gi) {
            let lv = f.or(*w).unwrap_or([0.0; 3]);
            let b = basis(*d);
            for c in 0..3 { oursum[c] += 2.0 * k * lv[c] * b; }
            names.push(format!("{idx}(sz {:.2}, b {:.3}, L {:.3})", d[0] * n[0] + d[1] * n[1] + d[2] * n[2], b, lv[1]));
        }
        for c in 0..3 { so[c] += oursum[c]; sg[c] += drgb[c]; }
        println!("game≤{gi:>3} Δa {da:.5}: ours Σ ({:+.5},{:+.5},{:+.5}) game ({:+.5},{:+.5},{:+.5}) ratio G {:.3}  [{}]", oursum[0], oursum[1], oursum[2], drgb[0], drgb[1], drgb[2], if drgb[1] != 0.0 { oursum[1] / drgb[1] } else { f32::NAN }, names.join(" "));
        prev_game_end = *gi;
    }
    println!("TOTAL ours ({:.5},{:.5},{:.5}) game ({:.5},{:.5},{:.5}) → ours/game ({:.4},{:.4},{:.4})", so[0], so[1], so[2], sg[0], sg[1], sg[2], so[0] / sg[0], so[1] / sg[1], so[2] / sg[2]);
}
