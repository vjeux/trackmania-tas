// E2 scratch: texels whose bounce sweep own-mean exceeds the direct sweep's (a self-feeding bounce), per chart object
use lightmap::passdiff::load_file;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    let map = &a[2];
    let mut planes = Vec::new();
    for s in 0..8 { let f = format!("sweep{s}_mrt0.f32"); let p = dir.join(&f); if !p.exists() { break; } let n = std::fs::metadata(&p).unwrap().len() / 16; let side = (n as f64).sqrt() as u32; planes.push(load_file(dir, &f, "R32G32B32A32_FLOAT", side, side, 0).unwrap()); }
    let (w, h) = (planes[0].w, planes[0].h);
    // rects from the baked map
    let mf = lightmap::passdiff::read_manifest("{}").unwrap();
    let rects = lightmap::passdiff::chart_rects(&mf, Some(map));
    let mut rect_id = vec![u32::MAX; (w * h) as usize];
    for (i, r) in rects.iter().enumerate() { for yy in r.y.max(0)..(r.y + r.h).min(h as i32) { for xx in r.x.max(0)..(r.x + r.w).min(w as i32) { rect_id[(yy as u32 * w + xx as u32) as usize] = i as u32; } } }
    let names: std::collections::HashMap<u32, String> = a.get(3).map(|p| std::fs::read_to_string(p).unwrap().lines().skip(1).filter_map(|l| { let c: Vec<&str> = l.split('\t').collect(); Some((c[0].parse().ok()?, c[4].to_string())) }).collect()).unwrap_or_default();
    // per chart: texels where any bounce sweep's own mean (G) > direct's own mean; the max ratio
    let mut per: std::collections::HashMap<u32, (usize, usize, f32)> = std::collections::HashMap::new();
    let mut tot = (0usize, 0usize);
    for y in 0..h { for x in 0..w {
        let rid = rect_id[(y * w + x) as usize]; if rid == u32::MAX { continue; }
        let a0 = planes[0].get(x, y, 3); if a0 <= 0.0 { continue; }
        let d0 = planes[0].get(x, y, 1) / a0;
        let mut worst = 0f32;
        for s in 1..planes.len() { let al = planes[s].get(x, y, 3); if al > 0.0 { let v = planes[s].get(x, y, 1) / al; if d0 > 0.3 { worst = worst.max(v / d0); } } }
        let e = per.entry(rid).or_insert((0, 0, 0.0)); e.0 += 1; tot.0 += 1;
        if worst > 1.0 { e.1 += 1; tot.1 += 1; }
        e.2 = e.2.max(worst);
    } }
    println!("covered texels {} ; with a bounce sweep own mean > a SKY-LIT direct own mean (G > 0.3): {} ({:.2} %)", tot.0, tot.1, 100.0 * tot.1 as f64 / tot.0.max(1) as f64);
    let mut v: Vec<_> = per.into_iter().filter(|(_, e)| e.1 > 0).collect();
    v.sort_by(|p, q| q.1 .1.cmp(&p.1 .1));
    // aggregate by model name
    let mut by_model: std::collections::HashMap<String, (usize, usize, usize, f32)> = std::collections::HashMap::new();
    for (rid, (n, nb, mx)) in &v { let nm = names.get(rid).cloned().unwrap_or_else(|| format!("chart{rid}")); let e = by_model.entry(nm).or_insert((0, 0, 0, 0.0)); e.0 += 1; e.1 += n; e.2 += nb; e.3 = e.3.max(*mx); }
    let mut bm: Vec<_> = by_model.into_iter().collect(); bm.sort_by(|p, q| q.1 .2.cmp(&p.1 .2));
    println!("by model (charts with feedback texels, covered texels, feedback texels, max bounce/direct ratio):");
    for (nm, (c, n, nb, mx)) in bm.iter().take(25) { println!("  {nm:<28} charts {c:>5} texels {n:>7} feedback {nb:>6} ({:.1} %) max {mx:.2}", 100.0 * *nb as f64 / *n as f64); }
}
