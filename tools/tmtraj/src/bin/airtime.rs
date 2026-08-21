// airtime -- does the run follow the built track, or leave it?
//
//   airtime GHOST.Gbx
//
// The question "does the car fly off the map" is answerable from the recording
// rather than from squinting at frames: CSceneVehicleVis carries a ground-contact
// flag per 50 ms sample, alongside altitude and speed. Prints a compact profile
// plus the summary numbers: fraction of the run on the ground, the longest
// unbroken airborne stretch, and the altitude range.
use std::env;
use tmtraj::entrec;

fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: airtime GHOST.Gbx");
        std::process::exit(2);
    }
    let d = entrec::decode_ghost(&a[0]).expect("decode");
    let s = &d.samples;
    let n = s.len();
    let ground = s.iter().filter(|p| p.is_ground_contact).count();

    // longest airborne run
    let (mut cur, mut best, mut best_at) = (0usize, 0usize, 0i32);
    for p in s.iter() {
        if p.is_ground_contact {
            cur = 0;
        } else {
            cur += 1;
            if cur > best {
                best = cur;
                best_at = p.time_ms - (cur as i32 - 1) * 50;
            }
        }
    }
    let ymin = s.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let ymax = s.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);

    println!("samples        {}", n);
    println!(
        "on the ground  {} / {}  ({:.1}%)",
        ground,
        n,
        100.0 * ground as f64 / n as f64
    );
    println!(
        "longest air    {:.2}s starting at {:.3}s",
        best as f64 * 0.05,
        best_at as f64 / 1000.0
    );
    println!("altitude       {:.1} .. {:.1} m  (range {:.1} m)", ymin, ymax, ymax - ymin);
    println!();
    println!("  t_s     x      y      z    km/h  ground");
    for p in s.iter().step_by(5) {
        println!(
            "{:6.2} {:7.1} {:6.1} {:7.1} {:7.1}  {}",
            p.time_ms as f64 / 1000.0,
            p.x,
            p.y,
            p.z,
            p.speed_kmh,
            if p.is_ground_contact { "yes" } else { "AIR" }
        );
    }
}
