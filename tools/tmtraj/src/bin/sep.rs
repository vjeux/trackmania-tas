// sep -- separation between two ghosts at the same race time.
//
//   sep A.Ghost.Gbx B.Ghost.Gbx
//
// Prints, per 50 ms sample, the 3D distance between where run A and run B are
// at that instant. This exists to make the two-car render test decisive: to
// prove a second car is DRAWN you need a frame where the two runs are far
// enough apart to be two distinct cars, but close enough that the second is
// inside a chase cam trained on the first. "Not drawn" and "out of frame" are
// indistinguishable without that number.
//
// Also reports which run is AHEAD along its own path length, because a car
// behind the camera target is off-screen no matter how close it is.
use std::env;
use tmtraj::entrec;

fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    if a.len() < 2 {
        eprintln!("usage: sep A.Ghost.Gbx B.Ghost.Gbx");
        std::process::exit(2);
    }
    let da = entrec::decode_ghost(&a[0]).expect("decode A");
    let db = entrec::decode_ghost(&a[1]).expect("decode B");
    eprintln!(
        "A {} samples race={:?}   B {} samples race={:?}",
        da.samples.len(),
        da.race_time_ms,
        db.samples.len(),
        db.race_time_ms
    );

    // cumulative path length, so "ahead" is along the track rather than in a
    // straight line
    let plen = |s: &[entrec::Sample]| -> Vec<f64> {
        let mut v = Vec::with_capacity(s.len());
        let mut acc = 0.0f64;
        for (i, p) in s.iter().enumerate() {
            if i > 0 {
                let q = &s[i - 1];
                acc += ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
            }
            v.push(acc);
        }
        v
    };
    let la = plen(&da.samples);
    let lb = plen(&db.samples);

    println!("t_ms\tdist_m\tdlen_m\tax\tay\taz\tbx\tby\tbz");
    let n = da.samples.len().min(db.samples.len());
    for i in 0..n {
        let p = &da.samples[i];
        let q = &db.samples[i];
        if p.time_ms != q.time_ms {
            eprintln!("time grids diverge at i={} ({} vs {})", i, p.time_ms, q.time_ms);
            break;
        }
        let d = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
        println!(
            "{}\t{:.6}\t{:+.6}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}",
            p.time_ms,
            d,
            lb[i] - la[i], // + => B is further along than A
            p.x,
            p.y,
            p.z,
            q.x,
            q.y,
            q.z
        );
    }
}
