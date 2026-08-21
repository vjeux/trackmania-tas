// whlvar -- are the WHEEL BYTES ALIVE?
//
//   whlvar GHOST.Ghost.Gbx
//   prints: <distinct-values> <samples> <changes> <span>
//
// This exists because "can we infer a wheel radius" and "are the wheel bytes
// present" are different questions, and conflating them produced a false
// refusal of Nadeo's own recording. C8's classifier needs samples it can class
// as ground-supported; a run that descends the whole way has none, so C8
// reports n/a -- and an n/a is a statement about the CHECK, not the file.
//
// The direct question is whether the bytes vary. Dead or donor-blanked
// telemetry is constant or near-constant; a real recording's wheel rotation
// takes many distinct values as the wheels turn. 145875's download carries
// 88-109 distinct values per wheel; a zeroed field carries 1.
//
// Reports the LEAST varying of the four wheels, because one dead wheel is
// enough to render wrongly.
use std::collections::BTreeSet;
use std::env;
use tmtraj::entrec;

fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: whlvar GHOST.Ghost.Gbx");
        std::process::exit(2);
    }
    let d = match entrec::decode_ghost(&a[0]) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    let s = &d.samples;
    if s.is_empty() {
        println!("0 0 0 0");
        return;
    }
    // quantise to milliradians so float noise does not inflate the count
    let q = |v: f64| -> i64 { (v * 1000.0).round() as i64 };
    let mut sets: [BTreeSet<i64>; 4] = Default::default();
    let mut changes = [0usize; 4];
    let mut prev = [i64::MIN; 4];
    for p in s {
        let w = [p.fl_wheel_rot, p.fr_wheel_rot, p.rr_wheel_rot, p.rl_wheel_rot];
        for i in 0..4 {
            if !w[i].is_finite() {
                continue;
            }
            let v = q(w[i]);
            sets[i].insert(v);
            if prev[i] != i64::MIN && prev[i] != v {
                changes[i] += 1;
            }
            prev[i] = v;
        }
    }
    let min_distinct = sets.iter().map(|x| x.len()).min().unwrap_or(0);
    let min_changes = changes.iter().copied().min().unwrap_or(0);
    println!(
        "{} distinct(min-of-4-wheels) {} samples {} changes {} span_ms",
        min_distinct,
        s.len(),
        min_changes,
        s.len() as i32 * d.sample_period_ms.unwrap_or(50)
    );
}
