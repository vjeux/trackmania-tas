//! `re15_texeltrace ROOT x,y [--sweep 0] [--pass hbasis0]` — one texel's accumulation history through the capture's banked
//! per-direction H-basis planes: for every banked plane of the sweep (in sweep_direction_index order) the texel's rgba and the
//! DELTA from the previous banked plane (the contributions of the directions in between; alpha delta × N = how many of them
//! covered the texel). The game's per-direction contribution list for the record's hot texel (RE 15, read 1 follow-up for E2/F).
use lightmap::passdiff::{load_entry, read_manifest};
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: re15_texeltrace ROOT x,y [--sweep S] [--pass hbasis0]");
        std::process::exit(2);
    }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let root = Path::new(&a[1]);
    let xy: Vec<u32> = a[2].split(',').map(|s| s.parse().unwrap()).collect();
    let (x, y) = (xy[0], xy[1]);
    let sweep: u32 = f("--sweep").map(|v| v.parse().unwrap()).unwrap_or(0);
    let pass = f("--pass").unwrap_or_else(|| "hbasis0".to_string());
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
    let m = read_manifest(&txt).unwrap_or_else(|e| panic!("{e}"));
    let mut entries: Vec<_> = m.passes.iter().filter(|e| e.pass == pass && e.sweep == Some(sweep)).collect();
    entries.sort_by_key(|e| (e.sweep_direction_index.unwrap_or(u32::MAX), e.frame, e.file.clone()));
    let n = if sweep == 0 { 256.0f32 } else { 128.0 };
    println!("{} banked {pass} planes of sweep {sweep}; texel ({x},{y}); N = {n}", entries.len());
    println!("dir_idx\tframe\tfile\tr\tg\tb\ta\tΔr\tΔg\tΔb\tΔa\tcovering dirs (Δa·N)");
    let mut prev: Option<([f32; 4], u32)> = None;
    for e in &entries {
        let b = match load_entry(root, e) { Ok(b) => b, Err(err) => { eprintln!("{}: {err}", e.file); continue; } };
        let v = [b.get(x, y, 0), b.get(x, y, 1), b.get(x, y, 2), if b.channels > 3 { b.get(x, y, 3) } else { 1.0 }];
        let di = e.sweep_direction_index.unwrap_or(u32::MAX);
        let (d, span) = match prev { Some((p, pi)) => ([v[0] - p[0], v[1] - p[1], v[2] - p[2], v[3] - p[3]], di.saturating_sub(pi)), None => (v, di + 1) };
        println!("{di}\t{}\t{}\t{:.5}\t{:.5}\t{:.5}\t{:.5}\t{:+.5}\t{:+.5}\t{:+.5}\t{:+.5}\t{:.1} of {span}", e.frame.unwrap_or(0), e.file.rsplit('/').next().unwrap_or(&e.file), v[0], v[1], v[2], v[3], d[0], d[1], d[2], d[3], d[3] * n);
        prev = Some((v, di));
    }
}
