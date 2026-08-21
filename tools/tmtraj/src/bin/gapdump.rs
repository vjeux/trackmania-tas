// gapdump -- every non-nominal sample interval, and the speed it implies.
// Written to settle a disagreement: two tools read 212 m/s and 196 m/s on the
// same file across a 200 m/s bar. If a check can straddle its own threshold
// depending on implementation detail, the number is not usable.
use std::env;
use tmtraj::entrec;
fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    let d = entrec::decode_ghost(&a[0]).expect("decode");
    let s = &d.samples;
    let period = d.sample_period_ms.unwrap_or(50);
    println!("period={}ms samples={}", period, s.len());
    let mut worst: Vec<(f64, f64, i32, i32)> = Vec::new();
    for i in 1..s.len() {
        let (p, q) = (&s[i], &s[i - 1]);
        let dt = p.time_ms - q.time_ms;
        if dt <= 0 { continue; }
        let dist = ((p.x-q.x).powi(2)+(p.y-q.y).powi(2)+(p.z-q.z).powi(2)).sqrt();
        if !dist.is_finite() { continue; }
        worst.push((dist / (dt as f64/1000.0), dist, dt, p.time_ms));
    }
    worst.sort_by(|a,b| b.0.partial_cmp(&a.0).unwrap());
    println!("top 8 by implied speed:");
    for (v,dist,dt,t) in worst.iter().take(8) {
        let flag = if *dt != period { "  <-- GAP, not a nominal step" } else { "" };
        println!("  {:8.1} m/s  {:8.2} m over {:3} ms  at {:.3}s{}", v, dist, dt, *t as f64/1000.0, flag);
    }
    // what does the worst NOMINAL-interval step look like?
    let nom = worst.iter().find(|(_,_,dt,_)| *dt == period);
    if let Some((v,dist,dt,t)) = nom {
        println!("worst step at the NOMINAL {}ms interval: {:.1} m/s ({:.2} m at {:.3}s)", dt, v, dist, *t as f64/1000.0);
    }
}
