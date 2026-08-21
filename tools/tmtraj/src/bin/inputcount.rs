// inputcount -- count INPUT EVENTS in a .Ghost.Gbx, or emit the per-tick tape.
//
//   inputcount GHOST.Gbx [GHOST.Gbx ...]     -> one summary line per file
//   inputcount --meta GHOST.Gbx              -> leadin_ms race_ms nsamp period span_ms
//   inputcount --csv GHOST.Gbx               -> t_race_ms,steer,accel,brake
//
// An input event = a sample where the (steer, gas, brake) triple changes.
//
// TIME BASE FOR --csv: the column is RACE time, not record time.
//
// A recording begins before the countdown ends and ends after the line, so its
// span is longer than the run: 126859's 23.545 run is a 27.85 s record. The
// rendered video covers the whole record, so an overlay that treats video frame
// 0 as race 0 is wrong by the lead-in on every map that has one. untitled 02
// only verified clean because its lead-in happens to be zero.
//
// `Decoded::start_ms` is NOT that offset -- it is 0 for every ghost here. The
// lead-in is measured instead, from the telemetry itself: the first sample whose
// position has moved more than LEADIN_M from where the car was sitting. That is
// the moment the car leaves the line, to within one 50 ms sample. It is a
// measurement, so it is checkable: film the map and look at the frame where the
// clock reads 0.000.
//
// NOTE ON THE SOURCE OF TRUTH: steer/gas/brake here come from the
// CPlugEntRecordData telemetry samples on a 50 ms grid, NOT from the 10 ms input
// chunk 0x0309201D. For ranking tapes by input count that was shown to be
// unreliable (six known counts, six mismatches) -- use the measured
// inputcounts_v1.tsv for that. For an OVERLAY it is adequate and honest.
use std::env;
use tmtraj::entrec;

/// the car is "still on the line" while it has moved less than this from where
/// the recording found it. A LOOSE threshold measures acceleration rather than
/// the start -- 0.5 m takes a TAS car 200-300 ms of full throttle, which is
/// exactly the error it then bakes into the clock.
const STILL_M: f64 = 0.05;

fn lead_in_ms(d: &entrec::Decoded) -> i32 {
    let s = &d.samples;
    if s.is_empty() {
        return 0;
    }
    let o = &s[0];
    let mut last_still = 0;
    for p in s.iter() {
        let dist =
            ((p.x - o.x).powi(2) + (p.y - o.y).powi(2) + (p.z - o.z).powi(2)).sqrt();
        if !dist.is_finite() {
            continue;
        }
        if dist < STILL_M {
            last_still = p.time_ms;
        } else {
            break;
        }
    }
    last_still
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: inputcount [--csv|--meta] GHOST.Gbx [GHOST.Gbx ...]");
        std::process::exit(2);
    }
    let csv = args[1] == "--csv";
    let meta = args[1] == "--meta";
    let files = if csv || meta { &args[2..] } else { &args[1..] };

    for path in files {
        match entrec::decode_ghost(path) {
            Err(e) => {
                if csv || meta {
                    eprintln!("DECODE-FAIL {}: {}", path, e);
                    std::process::exit(1);
                }
                println!("{}\tDECODE-FAIL\t{}", path, e);
            }
            Ok(d) => {
                let li = lead_in_ms(&d);
                let period = d.sample_period_ms.unwrap_or(50);
                if meta {
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        li,
                        d.race_time_ms.unwrap_or(0),
                        d.samples.len(),
                        period,
                        d.samples.len() as i32 * period
                    );
                } else if csv {
                    println!("t_race_ms,steer,accel,brake");
                    for s in &d.samples {
                        // steer is -1..1 in the decode; the overlay wants -127..127
                        let steer = (s.steer * 127.0).round() as i32;
                        let accel = if s.gas > 0.5 { 1 } else { 0 };
                        let brake = if s.brake > 0.5 { 1 } else { 0 };
                        println!("{},{},{},{}", s.time_ms - li, steer, accel, brake);
                    }
                } else {
                    let mut events = 0usize;
                    let mut prev: Option<(i64, i64, i64)> = None;
                    let q = |v: f64| -> i64 { (v * 1000.0).round() as i64 };
                    for s in &d.samples {
                        let cur = (q(s.steer), q(s.gas), q(s.brake));
                        if let Some(p) = prev {
                            if p != cur {
                                events += 1;
                            }
                        }
                        prev = Some(cur);
                    }
                    println!(
                        "{}\tevents={}\tsamples={}\tleadin_ms={}\trace_ms={:?}\tperiod={:?}",
                        path, events, d.samples.len(), li, d.race_time_ms, d.sample_period_ms
                    );
                }
            }
        }
    }
}
