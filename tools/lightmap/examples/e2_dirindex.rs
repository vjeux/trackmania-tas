// E2 scratch: sweep-0 direction index (issue order, q3) of each vector named in a set-texel-trace log, joined with the game's per-direction tsv
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ps = lightmap::dome::PointSets::load(&lightmap::dome::default_path()).expect("point sets");
    let q: u32 = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(3);
    let dirs = lightmap::dome::sweep_directions(&ps, q, 0, false).expect("sweep 0");
    let log = std::fs::read_to_string(&a[1]).expect("log");
    // the game's tsv: dir_idx → (Δa, Δr, Δg, Δb)
    let mut game: std::collections::HashMap<u32, (f32, f32, f32, f32)> = std::collections::HashMap::new();
    if let Some(p) = a.get(2) { for l in std::fs::read_to_string(p).unwrap().lines().skip(2) { let c: Vec<&str> = l.split('\t').collect(); if c.len() > 11 { if let Ok(i) = c[0].parse::<u32>() { game.insert(i, (c[10].parse().unwrap_or(0.0), c[7].parse().unwrap_or(0.0), c[8].parse().unwrap_or(0.0), c[9].parse().unwrap_or(0.0))); } } } }
    // our trace: per direction vector, the world then the FITTED result; the final L = the last peel that wrote
    let mut rows: Vec<(usize, [f32; 3], String, String)> = Vec::new();
    let mut cur: Option<([f32; 3], String)> = None; let mut cur_idx = usize::MAX;
    for l in log.lines() {
        if let Some(rest0) = l.strip_prefix("set-texel-trace: dir ") { let Some(rest) = rest0.find('(').map(|p| &rest0[p + 1..]) else { continue }; let bake_idx: usize = rest0[..rest0.find(' ').unwrap_or(0)].trim().parse().unwrap_or(usize::MAX);
            if let Some(end) = rest.find(')') {
                let v: Vec<f32> = rest[..end].split(',').map(|s| s.trim().parse().unwrap()).collect();
                let d = [v[0], v[1], v[2]];
                if let Some(pos) = rest.find("→ L ") {
                    let lval = rest[pos + "→ L ".len()..].to_string();
                    let peel = if rest.contains("FITTED") { "F" } else { "W" };
                    match &mut cur {
                        Some((cd, s)) if *cd == d => { s.push_str(&format!(" {peel}:{lval}")); }
                        _ => { if let Some((cd, s)) = cur.take() { rows.push((cur_idx, cd, s, String::new())); } cur = Some((d, format!("{peel}:{lval}"))); cur_idx = bake_idx; }
                    }
                }
            }
        }
    }
    if let Some((cd, s)) = cur.take() { rows.push((cur_idx, cd, s, String::new())); }
    println!("{} directions in the trace; sweep 0 has {} directions", rows.len(), dirs.len());
    for (idx, d, s, _) in rows.iter_mut() {
        let i = *idx; let ang = 0.0f32;
        let g = game.get(&(i as u32)).map(|(da, dr, dg, db)| format!("game Δa {da:+.5} Δrgb ({dr:+.5},{dg:+.5},{db:+.5})")).unwrap_or_else(|| "game: not banked".into());
        println!("dir {:>3} ({:+.4},{:+.4},{:+.4}) angle {ang:.3}° ours {s} | {g}", if *idx == usize::MAX { 9999 } else { *idx }, d[0], d[1], d[2]);
    }
}
