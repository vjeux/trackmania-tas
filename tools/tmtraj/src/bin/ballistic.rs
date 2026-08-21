// ballistic -- is the car ACTUALLY in free fall, judged by its own motion?
//
//   ballistic GHOST.Gbx
//
// A recorded contact flag is a claim; the trajectory is evidence. In free fall
// the only force is gravity, so vertical acceleration equals G and horizontal
// velocity is constant. This measures both from the positions and reports, per
// sample, whether the motion is ballistic -- then cross-tabulates that against
// what the file's contact flag says.
//
// G comes from the fleet measurement (-25.20 m/s^2, from ten recordings split on
// their own contact flag), not from Earth physics: Trackmania's gravity is its
// own number and using -22.3 makes the airborne class come out empty, so every
// "not airborne" assertion passes vacuously.
//
// Central differences, so a sample is judged by its neighbours rather than by a
// one-sided slope that smears the transition.
use std::env;
use tmtraj::entrec;

const G: f64 = -25.20;
/// tolerance on vertical acceleration, m/s^2 -- generous, because 50 ms sampling
/// of a 25 m/s^2 field leaves real quantisation noise
const TOL: f64 = 6.0;

fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: ballistic GHOST.Gbx");
        std::process::exit(2);
    }
    let d = entrec::decode_ghost(&a[0]).expect("decode");
    let s = &d.samples;
    if s.len() < 3 {
        eprintln!("too few samples");
        std::process::exit(1);
    }
    let dt = d.sample_period_ms.unwrap_or(50) as f64 / 1000.0;

    // contingency: ballistic? x flag-says-grounded?
    let (mut bal_g, mut bal_a, mut nb_g, mut nb_a) = (0usize, 0usize, 0usize, 0usize);
    let mut runs: Vec<(f64, f64)> = Vec::new(); // (start_s, len_s) of ballistic stretches
    let mut cur_start: Option<f64> = None;

    println!("  t_s      y     ay_m/s2  ballistic  flag");
    for i in 1..s.len() - 1 {
        let (p, q, r) = (&s[i - 1], &s[i], &s[i + 1]);
        // central second difference of altitude
        let ay = (r.y - 2.0 * q.y + p.y) / (dt * dt);
        if !ay.is_finite() {
            continue;
        }
        let ballistic = (ay - G).abs() < TOL;
        let grounded = q.is_ground_contact;
        match (ballistic, grounded) {
            (true, true) => bal_g += 1,
            (true, false) => bal_a += 1,
            (false, true) => nb_g += 1,
            (false, false) => nb_a += 1,
        }
        let t = q.time_ms as f64 / 1000.0;
        match (ballistic, cur_start) {
            (true, None) => cur_start = Some(t),
            (false, Some(st)) => {
                runs.push((st, t - st));
                cur_start = None;
            }
            _ => {}
        }
        if i % 5 == 0 {
            println!(
                "{:6.2} {:7.1} {:9.1}  {:9}  {}",
                t,
                q.y,
                ay,
                if ballistic { "AIR" } else { "-" },
                if grounded { "grounded" } else { "airborne" }
            );
        }
    }
    if let Some(st) = cur_start {
        let t = s[s.len() - 2].time_ms as f64 / 1000.0;
        runs.push((st, t - st));
    }

    let n = bal_g + bal_a + nb_g + nb_a;
    println!();
    println!("--- motion says vs flag says, {} samples", n);
    println!("  ballistic motion, flag GROUNDED : {:4}   <- flag is wrong (the old defect)", bal_g);
    println!("  ballistic motion, flag airborne : {:4}", bal_a);
    println!("  driving motion,   flag grounded : {:4}", nb_g);
    println!("  driving motion,   flag AIRBORNE : {:4}   <- flag is wrong the other way", nb_a);
    println!();
    let total_air: f64 = runs.iter().map(|r| r.1).sum();
    println!(
        "ballistic by motion: {:.2}s of {:.2}s ({:.1}%)",
        total_air,
        s.len() as f64 * dt,
        100.0 * total_air / (s.len() as f64 * dt)
    );
    runs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (st, len) in runs.iter().take(5) {
        println!("  free-fall stretch {:.2}s starting at {:.3}s", len, st);
    }
}
