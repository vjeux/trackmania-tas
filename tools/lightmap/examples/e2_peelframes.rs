// E2 scratch: the pwc-day capture's frames with a banked sweep-0 peel_color layer 0 (the dome plane), with the direction vector and its elevation
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let txt = std::fs::read_to_string(&a[1]).expect("manifest");
    let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
    let mut rows: Vec<(u32, u32, f32, [f32; 3], String)> = Vec::new();
    for e in m.passes.iter().filter(|e| e.pass == "peel_color" && e.sweep.unwrap_or(0) == 0 && e.layer == Some(0) && e.phase.as_deref() == Some("world")) {
        if let (Some(d), Some(fr)) = (e.dir, e.frame) { rows.push((e.direction.unwrap_or(9999), fr, d[1].asin().to_degrees(), d, e.file.clone())); }
    }
    rows.sort_by(|p, q| p.0.cmp(&q.0).then(p.1.cmp(&q.1)));
    rows.dedup_by(|p, q| p.0 == q.0 && p.1 == q.1);
    for (i, fr, el, d, f) in &rows { println!("{i}\t{fr}\t{el:.1}\t({:+.4},{:+.4},{:+.4})\t{f}", d[0], d[1], d[2]); }
    eprintln!("{} (direction, frame) rows", rows.len());
}
