//! `e5_envdepth PASSCAP [--frame 127448] [--points x,y,z;x,y,z;…]` — THE GAME'S LAYER-0 DEPTH BEHIND A RECEIVER (E5, 2026-09-28):
//! for each world point (default: nine points on pwc-day's pad plane y = 4 around (1024, 4, 1024)), project it into the frame's
//! captured layer-0 frustum, read the captured environment layer's depth (`peel_depth`, layer 0) and colour at that pixel, and
//! print them beside the point's own z01 and the sea floor's (y −6) z01 along the same pixel ray. The question it answers box-free:
//! when an environment surface (the sea floor / skirt) lies BEHIND a receiver along the peel direction — nearer the dark-end
//! camera, i.e. a LARGER reversed z01 — does the game's env layer hold that surface's depth (our rule: nearest env wins, the
//! receiver then fails PS 17112's compare and reads nothing), or the dome's / the clear value (the dome wins where it covers, or
//! the terrain is not drawn there)? pwc-day's pad texels are bit-exact with the captured layers fed in, so whatever the captured
//! layer holds at the pad's pixels is what the game's receivers saw.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 { eprintln!("usage: e5_envdepth PASSCAP [--frame N] [--points x,y,z;…]"); std::process::exit(2); }
    let root = std::path::PathBuf::from(&a[1]);
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let frame_no: u32 = f("--frame").map(|v| v.parse().unwrap()).unwrap_or(127448);
    let points: Vec<[f32; 3]> = match f("--points") {
        Some(s) => s.split(';').map(|p| { let v: Vec<f32> = p.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] }).collect(),
        None => {
            let mut v = Vec::new();
            for dz in [-400.0f32, 0.0, 400.0] { for dx in [-400.0f32, 0.0, 400.0] { v.push([1024.0 + dx, 4.0, 1024.0 + dz]); } }
            v
        }
    };
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
    let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
    let mut ents: Vec<&lightmap::passdump::Entry> = m.passes.iter().filter(|e| e.frame == Some(frame_no) && e.layer == Some(0) && (e.pass == "peel_color" || e.pass == "peel_depth")).collect();
    ents.sort_by_key(|e| e.eid_last.unwrap_or(0));
    let col = ents.iter().find(|e| e.pass == "peel_color").expect("the captured environment colour layer");
    let dep = ents.iter().find(|e| e.pass == "peel_depth" && e.eid_last == col.eid_last).or_else(|| ents.iter().find(|e| e.pass == "peel_depth")).expect("the captured environment depth layer");
    let fr = col.frustum.clone().expect("the entry's frustum");
    let (w, h) = (if col.width > 0 { col.width } else { 4096 }, if col.height > 0 { col.height } else { 4096 });
    let frame = lightmap::peel::PeelFrame::from_frustum(&fr, w, h);
    let game = lightmap::passdiff::load_entry(&root, col).expect("captured colour");
    let gdepth = lightmap::passdiff::load_entry(&root, dep).expect("captured depth");
    println!("frame {frame_no}: colour {} ({}×{}, {} ch), depth {} ({} ch); frustum centre {:?} half {:?} forward {:?}", col.file, game.w, game.h, game.channels, dep.file, gdepth.channels, fr.center, fr.half, fr.forward);
    println!("D (the peel direction) = forward; a receiver passes layer 0 when z01_receiver ≥ z01_layer (reversed z: 1 = nearest the dark-end camera, 0 = the far plane / the dome).");
    // the depth of the pixel ray's crossing of the sea-floor plane y = −6 and the water plane y = −1 (WhiteShore)
    let plane_z01 = |x: f32, y: f32, plane_y: f32| -> Option<f32> {
        // the ray of pixel (x, y): p(z) = unproject(x, y, z); y-component linear in z
        let p0 = frame.unproject(x, y, 0.0);
        let p1 = frame.unproject(x, y, 1.0);
        let dy = p1[1] - p0[1];
        if dy.abs() < 1e-9 { return None; }
        let z = (plane_y - p0[1]) / dy;
        Some(frame.z01(z))
    };
    for p in &points {
        let (px, py, zp) = frame.project(*p);
        let z01 = frame.z01(zp);
        let (xi, yi) = (px.floor() as i64, py.floor() as i64);
        if xi < 0 || yi < 0 || xi >= w as i64 || yi >= h as i64 { println!("point {:?}: outside the frame ({px:.1}, {py:.1})", p); continue; }
        let gd = gdepth.get(xi as u32, yi as u32, 0);
        let gc = [game.get(xi as u32, yi as u32, 0), game.get(xi as u32, yi as u32, 1), game.get(xi as u32, yi as u32, 2)];
        let floor = plane_z01(px, py, -6.0);
        let water = plane_z01(px, py, -1.0);
        let verdict = if gd <= 1e-6 { "CLEAR/far (no env surface written — the dome or nothing)" } else if gd > z01 + 1e-6 { "an env surface BEHIND the receiver (nearer the camera): our rule would FAIL the receiver here" } else { "an env surface in FRONT of the receiver (toward the light)" };
        println!("point ({:.1}, {:.1}, {:.1}) → pixel ({xi}, {yi}) z01 {z01:.6}; captured layer 0: depth {gd:.6} colour ({:.4}, {:.4}, {:.4}); sea floor y −6 on this ray z01 {}; water y −1 z01 {} → {verdict}", p[0], p[1], p[2], gc[0], gc[1], gc[2], floor.map(|v| format!("{v:.6}")).unwrap_or("-".into()), water.map(|v| format!("{v:.6}")).unwrap_or("-".into()));
    }
    // a census over the whole frame: how many pixels hold an env depth, and their depth range
    let (mut written, mut total) = (0usize, 0usize);
    let (mut dmin, mut dmax) = (f32::MAX, f32::MIN);
    for y in 0..gdepth.h { for x in 0..gdepth.w { total += 1; let d = gdepth.get(x, y, 0); if d > 1e-6 { written += 1; dmin = dmin.min(d); dmax = dmax.max(d); } } }
    println!("census: {written} of {total} pixels carry an environment depth > 0 ({:.2} %), range {dmin:.6}..{dmax:.6}", 100.0 * written as f64 / total as f64);
}
