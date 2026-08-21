// c3speed -- re-read every C3 "jump" as an IMPLIED SPEED, not a distance.
//
//   c3speed GHOST.Ghost.Gbx
//
// WHY
//
// C3 refused a file when consecutive recorded positions were far apart. But a
// record can carry a GAP: 286279 has a 650 ms hole, and at 131 km/h that is
// 23.7 m of perfectly ordinary driving -- which C3 called a teleport, on the
// PUBLISHED ORIGINAL. The quantity that distinguishes a splice from a gap is
// distance DIVIDED BY THE ELAPSED TIME, and the sample period is not constant.
//
// The bar is 200 m/s (720 km/h). The fastest thing in these recordings is a
// 546 km/h reactor run; a genuine graft seam reads tens of thousands of m/s.
//
// This is the same shape as every other correction tonight: a fixed threshold
// on the wrong quantity. C8's 0.36 m assumed the Stadium wheel; gravity's
// -22.3 assumed Earth; C3's metres assumed a fixed sample period.
use std::env;
use tmtraj::entrec;

const LIMIT_MS: f64 = 200.0; // m/s

fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: c3speed GHOST.Ghost.Gbx");
        std::process::exit(2);
    }
    let d = match entrec::decode_ghost(&a[0]) {
        Ok(d) => d,
        Err(e) => {
            println!("DECODE-FAIL\t{}", e);
            std::process::exit(1);
        }
    };
    let s = &d.samples;
    if s.len() < 2 {
        println!("SHORT");
        return;
    }
    let (mut worst_d, mut worst_v, mut worst_t, mut worst_dt) = (0.0f64, 0.0f64, 0i32, 0i32);
    let mut gaps = 0usize;
    let period = d.sample_period_ms.unwrap_or(50);
    for i in 1..s.len() {
        let (p, q) = (&s[i], &s[i - 1]);
        let dt_ms = p.time_ms - q.time_ms;
        if dt_ms <= 0 {
            continue;
        }
        if dt_ms > period {
            gaps += 1;
        }
        let dist =
            ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
        if !dist.is_finite() {
            continue;
        }
        let v = dist / (dt_ms as f64 / 1000.0);
        if v > worst_v {
            worst_v = v;
            worst_d = dist;
            worst_t = p.time_ms;
            worst_dt = dt_ms;
        }
    }
    let verdict = if worst_v > LIMIT_MS { "REFUSE" } else { "PASS" };
    println!(
        "{}\tworst {:.2} m over {} ms = {:.1} m/s ({:.0} km/h) at {:.3}s\tgaps={}\tperiod={}ms",
        verdict,
        worst_d,
        worst_dt,
        worst_v,
        worst_v * 3.6,
        worst_t as f64 / 1000.0,
        gaps,
        period
    );
}
