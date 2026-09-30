//! `re17_itempose MAP.Gbx [NAME-SUBSTRING]` — every item placement's (yaw, pitch, roll) in degrees, position, pivot, scale, and the
//! port's rotation applied to model-up (0, 1, 0) (mapgeom::place::anchored) — V6-1c's "hills pitched 105–165° about x": which
//! way the model's top faces in OUR world. RE 17 2026-09-30 16:10Z.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    let filt = a.get(2).cloned().unwrap_or_default();
    let mut n = 0usize; let mut pitched = 0usize;
    for it in &m.items {
        if !filt.is_empty() && !it.model.contains(&filt) { continue; }
        n += 1;
        if it.pitch.abs() > 0.01 || it.roll.abs() > 0.01 { pitched += 1; }
        let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
        // the transform as 3×4 rows (mapgeom Xform = [r00 r01 r02 r10 r11 r12 r20 r21 r22 tx ty tz]?) — apply to the model-up direction
        let up = mapgeom::geom::apply_normal(&xf, [0.0, 1.0, 0.0]);
        if !filt.is_empty() || it.pitch.abs() > 0.01 {
            println!("{:<26} pos ({:.2}, {:.2}, {:.2}) yaw {:7.2}° pitch {:7.2}° roll {:7.2}° pivot {:?} scale {}  model-up → world ({:+.3}, {:+.3}, {:+.3})", it.model, it.pos[0], it.pos[1], it.pos[2], it.yaw.to_degrees(), it.pitch.to_degrees(), it.roll.to_degrees(), it.pivot, it.scale, up[0], up[1], up[2]);
        }
    }
    println!("{n} items ({pitched} with a pitch or roll)");
}
