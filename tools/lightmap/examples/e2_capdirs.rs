// E2 scratch: the captured hbasis0 planes' direction vectors by sweep_direction_index (pwc-day manifest), for the (676,84) cross-check
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let txt = std::fs::read_to_string(&a[1]).expect("manifest");
    let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
    let want: std::collections::HashSet<u32> = a.get(2).map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let mut rows: Vec<(u32, u32, String, Option<[f32; 3]>)> = Vec::new();
    for e in m.passes.iter().filter(|e| e.pass == "hbasis0" && e.sweep.unwrap_or(0) == 0) {
        if let Some(i) = e.sweep_direction_index { if want.is_empty() || want.contains(&i) { rows.push((i, e.frame.unwrap_or(0), e.file.clone(), e.dir)); } }
    }
    rows.sort_by_key(|r| (r.0, r.1));
    for (i, fr, f, d) in rows { println!("idx {i:>3} frame {fr} {f} dir {}", d.map(|d| format!("({:+.5},{:+.5},{:+.5})", d[0], d[1], d[2])).unwrap_or("-".into())); }
}
