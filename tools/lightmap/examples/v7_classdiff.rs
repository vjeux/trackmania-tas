//! `v7_classdiff OLD.tsv NEW.tsv [--min-texels 500] [--moved 0.01] [--tol 0.03] [--out TSV]` — two `classcmp --tsv` tables of the same cell
//! (two bases, or two sources) class by class: the ratio r/g/b old → new, the move (max channel |Δ|), TOWARD or AWAY from 1 (by the
//! channel-summed distance to 1), and the 3 % flips (within → beyond / beyond → within); sorted by the move; the TOTAL rows first.
//! The regression detector's per-class half (V7, 2026-10-01): a base that moves a cell is attributed by WHICH classes moved and how.
fn parse_ratio(s: &str) -> Option<[f64; 3]> {
    let p: Vec<f64> = s.split('/').map(|t| t.trim().parse::<f64>().ok()).collect::<Option<Vec<f64>>>()?;
    if p.len() == 3 { Some([p[0], p[1], p[2]]) } else { None }
}
fn read(path: &str) -> Result<Vec<(String, usize, usize, Option<[f64; 3]>)>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut lines = txt.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |names: &[&str]| names.iter().find_map(|n| head.iter().position(|h| h.trim() == *n));
    let (Some(ci), Some(cc), Some(ct), Some(cr)) = (col(&["class"]), col(&["charts"]), col(&["texels"]), col(&["ratio_rgb", "ratio ours/editor (r/g/b)"])) else { return Err(format!("{path}: need class/charts/texels/ratio_rgb columns, have {head:?}")) };
    let mut out = Vec::new();
    for l in lines {
        if l.starts_with('#') { continue; }
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() <= cr.max(ct) { continue; }
        out.push((f[ci].trim().to_string(), f[cc].trim().parse().unwrap_or(0), f[ct].trim().parse().unwrap_or(0), parse_ratio(f[cr])));
    }
    Ok(out)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: v7_classdiff OLD.tsv NEW.tsv [--min-texels 500] [--moved 0.01] [--tol 0.03] [--out TSV]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let min_texels: usize = f("--min-texels").map(|v| v.parse().expect("--min-texels N")).unwrap_or(500);
    let moved: f64 = f("--moved").map(|v| v.parse().expect("--moved F")).unwrap_or(0.01);
    let tol: f64 = f("--tol").map(|v| v.parse().expect("--tol F")).unwrap_or(0.03);
    let old = read(&a[1]).unwrap_or_else(|e| panic!("{e}"));
    let new = read(&a[2]).unwrap_or_else(|e| panic!("{e}"));
    let old_of: std::collections::HashMap<String, (usize, usize, Option<[f64; 3]>)> = old.iter().map(|(c, n, t, r)| (c.clone(), (*n, *t, *r))).collect();
    let dist = |r: [f64; 3]| (r[0] - 1.0).abs() + (r[1] - 1.0).abs() + (r[2] - 1.0).abs();
    let beyond = |r: [f64; 3]| r.iter().any(|v| (v - 1.0).abs() > tol);
    let mut rows: Vec<(f64, String)> = Vec::new();
    let (mut toward, mut away, mut flip_in, mut flip_out, mut unchanged, mut only_new, mut only_old) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut lines: Vec<String> = vec!["class\tcharts\ttexels\told r/g/b\tnew r/g/b\tmove\tdirection\tflip".into()];
    for (c, n, t, r_new) in &new {
        let is_total = c == "TOTAL" || !c.contains(':');
        if !is_total && *t < min_texels { continue; }
        let Some((_, _, r_old)) = old_of.get(c) else { only_new += 1; continue };
        let (Some(ro), Some(rn)) = (r_old, r_new) else { continue };
        let mv = (0..3).map(|k| (rn[k] - ro[k]).abs()).fold(0.0, f64::max);
        let dir = if mv < moved { "unchanged" } else if dist(*rn) < dist(*ro) { "TOWARD" } else { "AWAY" };
        let flip = match (beyond(*ro), beyond(*rn)) { (true, false) => "→ within 3 %", (false, true) => "→ BEYOND 3 %", _ => "" };
        if !is_total { match dir { "TOWARD" => toward += 1, "AWAY" => away += 1, _ => unchanged += 1 } ; if flip.starts_with("→ within") { flip_in += 1 } else if flip.starts_with("→ BEYOND") { flip_out += 1 } }
        let line = format!("{c}\t{n}\t{t}\t{:.3}/{:.3}/{:.3}\t{:.3}/{:.3}/{:.3}\t{mv:.3}\t{dir}\t{flip}", ro[0], ro[1], ro[2], rn[0], rn[1], rn[2]);
        rows.push((if is_total { f64::INFINITY } else { mv }, line));
    }
    for (c, _, t, _) in &old { if *t >= min_texels && !new.iter().any(|(d, _, _, _)| d == c) { only_old += 1; } }
    rows.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
    for (_, l) in &rows { println!("{l}"); lines.push(l.clone()); }
    eprintln!("classes ≥ {min_texels} texels: {toward} toward, {away} away, {unchanged} unchanged (< {moved}); flips: {flip_in} → within {:.0} %, {flip_out} → beyond; only in new {only_new}, only in old {only_old}", tol * 100.0);
    if let Some(o) = f("--out") { std::fs::write(&o, lines.join("\n") + "\n").unwrap_or_else(|e| panic!("{o}: {e}")); eprintln!("wrote {o}"); }
}
