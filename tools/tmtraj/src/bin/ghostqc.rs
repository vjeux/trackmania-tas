// ghostqc -- is this ghost RENDERABLE, before we spend a render on it?
//
//   ghostqc GHOST.Gbx [GHOST.Gbx ...]
//
// A tape that validates as a time is not necessarily a tape that draws a car:
// validation reads the input chunk, the video draws the CPlugEntRecordData
// telemetry. Several staged ghosts have a telemetry track that is missing,
// constant, or non-finite -- they import fine, report the right race time, and
// then render an empty scene or a car frozen at the origin.
//
// Checks, in order of how badly they break a render:
//   NAN      any non-finite position
//   STATIC   position never moves (a placeholder like (1,0,0))
//   ORIGIN   the run starts at/near the world origin -- no real start block is
//   SHORT    fewer than 10 samples
//   CREEP    total path length under 5 m -- the car never really drives
// Anything else is OK, and the line carries start position, path length and top
// speed so the numbers can be eyeballed rather than trusted blindly.
use std::env;
use tmtraj::entrec;

fn main() {
    let files: Vec<String> = env::args().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: ghostqc GHOST.Gbx [GHOST.Gbx ...]");
        std::process::exit(2);
    }
    println!("verdict\trace_ms\tnsamp\tpathlen_m\tvmax_kmh\tstart_xyz\tpath");
    for path in &files {
        match entrec::decode_ghost(path) {
            Err(e) => println!("DECODE-FAIL\t-\t-\t-\t-\t-\t{}\t{}", path, e),
            Ok(d) => {
                let s = &d.samples;
                if s.len() < 10 {
                    println!("SHORT\t{:?}\t{}\t-\t-\t-\t{}", d.race_time_ms, s.len(), path);
                    continue;
                }
                let nan = s.iter().any(|p| {
                    !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite() || !p.speed_kmh.is_finite()
                });
                let mut len = 0.0f64;
                let mut vmax = 0.0f64;
                for i in 1..s.len() {
                    let (p, q) = (&s[i], &s[i - 1]);
                    let d3 = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
                    if d3.is_finite() {
                        len += d3;
                    }
                    if p.speed_kmh.is_finite() && p.speed_kmh > vmax {
                        vmax = p.speed_kmh;
                    }
                }
                let f = &s[0];
                let origin = f.x.abs() < 2.0 && f.y.abs() < 2.0 && f.z.abs() < 2.0;
                let verdict = if nan {
                    "NAN"
                } else if len < 0.01 {
                    "STATIC"
                } else if origin {
                    "ORIGIN"
                } else if len < 5.0 {
                    "CREEP"
                } else {
                    "OK"
                };
                println!(
                    "{}\t{:?}\t{}\t{:.1}\t{:.1}\t({:.1},{:.1},{:.1})\t{}",
                    verdict,
                    d.race_time_ms,
                    s.len(),
                    len,
                    vmax,
                    f.x,
                    f.y,
                    f.z,
                    path
                );
            }
        }
    }
}
