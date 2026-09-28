// E2 scratch: join the SET texel trace (bake5.log: per direction/peel the depth texel + layer verdicts) with the peel's LAYERDBG
// (bake7.log: per pixel/direction the fragments accepted as layers, with tri/inst/card/front) — for the traced texel, per
// direction and peel: which fragment IS each passing layer
use std::collections::HashMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let set = std::fs::read_to_string(&a[1]).unwrap();
    let dbg = std::fs::read_to_string(&a[2]).unwrap();
    // LAYERDBG blocks keyed by (dir vector rounded, px, py)
    let mut blocks: HashMap<(String, u32, u32), Vec<String>> = HashMap::new();
    let mut key: Option<(String, u32, u32)> = None;
    for l in dbg.lines() {
        if let Some(r) = l.strip_prefix("LAYERDBG frame=") {
            let d = r.split("dir=(").nth(1).unwrap().split(')').next().unwrap().to_string();
            let px: u32 = r.split("px=").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
            let py: u32 = r.split("py=").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
            let dk: String = d.split(',').map(|v| format!("{:.3}", v.parse::<f32>().unwrap())).collect::<Vec<_>>().join(",");
            key = Some((dk, px, py));
            blocks.entry(key.clone().unwrap()).or_default();
        } else if l.starts_with("LAYERDBG px=") {
            if let Some(k) = &key { if l.contains("accepted") { blocks.get_mut(k).unwrap().push(l.to_string()); } }
        }
    }
    // the SET trace: "set-texel-trace: dir N (x,y,z) world|FITTED peel, ..." then "    frag 0: ... depth texel (px,py) ... has n layers: [L0 ...] [L1 ...]"
    let mut cur: Option<(String, String, String)> = None;
    let mut seen_first = std::collections::HashSet::new(); let mut sweep0 = true;
    for l in set.lines() {
        if let Some(r) = l.strip_prefix("set-texel-trace: dir ") {
            if r.contains("→ L") { continue; }
            let idx = r.split(' ').next().unwrap().to_string();
            let d = r.split('(').nth(1).unwrap().split(')').next().unwrap();
            let dk: String = d.split(',').map(|v| format!("{:.3}", v.parse::<f32>().unwrap())).collect::<Vec<_>>().join(",");
            let peel = if r.contains("FITTED") { "FITTED" } else { "world" };
            if peel == "world" && !seen_first.insert(idx.clone()) { sweep0 = false; }
            cur = if sweep0 { Some((idx, dk, peel.to_string())) } else { None };
        } else if l.trim_start().starts_with("frag 0:") {
            let Some((idx, dk, peel)) = &cur else { continue };
            let Some(t) = l.split("depth texel (").nth(1) else { continue };
            let px: u32 = t.split(',').next().unwrap().parse().unwrap();
            let py: u32 = t.split(',').nth(1).unwrap().split(')').next().unwrap().parse().unwrap();
            let layers: Vec<&str> = l.split('[').skip(1).map(|s| s.split(']').next().unwrap()).collect();
            let passing_black: Vec<&str> = layers.iter().copied().filter(|s| s.contains("PASS") && s.contains("rgb (0.0000,0.0000,0.0000)") && !s.starts_with("L0 d 0.0000000")).collect();
            if passing_black.is_empty() { continue; }
            println!("dir {idx} {peel} peel px ({px},{py}): passing black layers {:?}", passing_black);
            match blocks.get(&(dk.clone(), px, py)) {
                None => println!("    (no LAYERDBG block for this pixel/direction)"),
                Some(b) => for s in b { println!("    {}", s.trim_start_matches("LAYERDBG ").replace(&format!("px={px} py={py} "), "")); },
            }
        }
    }
}
