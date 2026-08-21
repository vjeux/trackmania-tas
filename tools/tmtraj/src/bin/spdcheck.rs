// spdcheck -- does the car's OWN recorded speed corroborate the implied speed?
//
// The C3 argument is about whether a big position step is a teleport or real
// driving. There is a decisive witness we were both ignoring: CSceneVehicleVis
// records the car's SCALAR SPEED per sample, independently of its position.
// If position says 212 m/s and the car's own speedometer says 212 m/s, it drove
// there. If the speedometer says 40 m/s, the position moved without the car.
use std::env;
use tmtraj::entrec;
fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    let d = entrec::decode_ghost(&a[0]).expect("decode");
    let s = &d.samples;
    let mut rows: Vec<(f64, f64, f64, i32, i32)> = Vec::new();
    for i in 1..s.len() {
        let (p, q) = (&s[i], &s[i - 1]);
        let dt = p.time_ms - q.time_ms;
        if dt <= 0 { continue; }
        let dist = ((p.x-q.x).powi(2)+(p.y-q.y).powi(2)+(p.z-q.z).powi(2)).sqrt();
        if !dist.is_finite() { continue; }
        rows.push((dist/(dt as f64/1000.0), p.speed_ms, dist, dt, p.time_ms));
    }
    rows.sort_by(|a,b| b.0.partial_cmp(&a.0).unwrap());
    println!("  implied   recorded   ratio   dist    dt   at        verdict");
    for (v, rec, dist, dt, t) in rows.iter().take(5) {
        let ratio = if *rec > 0.1 { v/rec } else { 999.0 };
        let verdict = if ratio < 1.5 { "DRIVEN (speedometer agrees)" } else { "TELEPORT (car was not moving that fast)" };
        println!("  {:8.1} {:9.1} {:7.2} {:7.2} {:4} {:8.3}s  {}", v, rec, ratio, dist, dt, *t as f64/1000.0, verdict);
    }
}
