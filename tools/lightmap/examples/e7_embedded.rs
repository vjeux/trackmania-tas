//! `e7_embedded MAP.Map.Gbx [--grep SUBSTR] [--extract NAME OUT] [--mipstat NAME LEVEL]` — the map's embedded files (name, bytes);
//! extract one; or the statistics of one DDS mip level: alpha ≥ 128/255 fraction and the rgb mean (stored and sRGB→linear) over the
//! passing texels (E7, 2026-09-30: the giant firs' leaf texture's 16×16 mip against the game's green leaf texels).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 { eprintln!("usage: e7_embedded MAP [--grep S] [--extract NAME OUT] [--mipstat NAME LEVEL]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).map(|i| i + 1);
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    let files = mapgeom::embedded::files(&mf).expect("embedded");
    let base = |k: &str| k.rsplit(['/', '\\']).next().unwrap_or(k).to_string();
    if let Some(i) = f("--extract") { let (name, out) = (&a[i], &a[i + 1]); let (k, b) = files.iter().find(|(k, _)| base(k).eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("{name}: not embedded")); std::fs::write(out, b).expect("write"); eprintln!("{k}: {} B → {out}", b.len()); return; }
    if let Some(i) = f("--mipstat") {
        let (name, level): (&str, usize) = (&a[i], a[i + 1].parse().expect("LEVEL"));
        let (k, b) = files.iter().find(|(k, _)| base(k).eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("{name}: not embedded"));
        let tx = lightmap::texsample::parse_dds(b, lightmap::texsample::Bc1Decode::Expand8Round).unwrap_or_else(|e| panic!("{k}: {e}"));
        println!("{k}: {:?} {}×{} mips {} (complete {})", tx.fmt, tx.w, tx.h, tx.mips, tx.complete);
        let lv = &tx.levels[0][level.min(tx.levels[0].len() - 1)];
        let (mut n, mut pass, mut s, mut sl) = (0usize, 0usize, [0f64; 3], [0f64; 3]);
        let mut rows = Vec::new();
        for y in 0..lv.h { let mut row = Vec::new(); for x in 0..lv.w { let c = lv.get(x, y); n += 1; if c[3] >= 128.0 / 255.0 { pass += 1; for k in 0..3 { s[k] += c[k] as f64; sl[k] += lightmap::gpufmt::srgb_to_linear(c[k]) as f64; } } row.push(format!("{:.2}/{:.2}/{:.2}/a{:.2}", c[0], c[1], c[2], c[3])); } rows.push(row.join(" ")); }
        println!("level {level}: {}×{}, alpha ≥ 0.502 on {pass} of {n} texels ({:.1} %); passing texels' mean stored ({:.4}, {:.4}, {:.4}), linear ({:.4}, {:.4}, {:.4})", lv.w, lv.h, 100.0 * pass as f64 / n.max(1) as f64, s[0] / pass.max(1) as f64, s[1] / pass.max(1) as f64, s[2] / pass.max(1) as f64, sl[0] / pass.max(1) as f64, sl[1] / pass.max(1) as f64, sl[2] / pass.max(1) as f64);
        if lv.w <= 16 { for r in rows { println!("  {r}"); } }
        return;
    }
    let g = f("--grep").map(|i| a[i].to_ascii_lowercase());
    for (k, b) in &files { if g.as_ref().map(|s| k.to_ascii_lowercase().contains(s.as_str())).unwrap_or(true) { println!("{k}\t{} B", b.len()); } }
}
