// spawnq -- does this ghost SPAWN like a human's on the same map?
//
//   spawnq HUMAN_REFERENCE.Gbx OURS.Gbx [OURS.Gbx ...]
//
// Two fields decide whether a render shows our run at all, and every check we
// had was positional, so both got through:
//
//   POSITION     a tape can carry another MAP's telemetry entirely. 276874's
//                first roof candidate began at (1552, 34, 560) -- a point on a
//                different map -- with a clean container and a clean identity.
//
//   ORIENTATION  197047's FILMED tape carried the IDENTITY quaternion where all
//                26 human recordings on that map read (3.39e-05,-0.7071,0,
//                0.7071). Its positions matched 1917 of 1917 samples, and the
//                car faced the wrong way for the entire clip. Position lives at
//                +208 and the quaternion at +192 of a 452-byte record, so
//                "the positions are identical" is silent about the facing.
//
// COMPARE THE ORIENTATION AS A ROTATION, NEVER AS BYTES.
// q and -q are the SAME rotation. Five 199100 files read (-0.7071,0,0.7071,0)
// against the humans' (0.7071,0,-0.7071,0) and are perfectly correct; a naive
// equality test condemns them, including our regenerated 47.483. The test is
// |dot(q_ours, q_human)| ~= 1. A check that cries wolf gets switched off, and
// then it is not a check.
//
// The reference is free on every map: every run spawns identically, so any
// downloaded human recording is a valid zero.
//
// Exit 0 all clear, 1 if any file is refused, 2 on usage.
use std::env;
use tmtraj::entrec;

const POS_TOL_M: f64 = 2.0;
const DOT_TOL: f64 = 0.99;

fn first(path: &str) -> Result<(f64, f64, f64, [f64; 4]), String> {
    let d = entrec::decode_ghost(path).map_err(|e| e.to_string())?;
    let s = d.samples.first().ok_or_else(|| "no samples".to_string())?;
    Ok((s.x, s.y, s.z, [s.qx, s.qy, s.qz, s.qw]))
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: spawnq HUMAN_REFERENCE.Gbx OURS.Gbx [OURS.Gbx ...]");
        eprintln!("  the reference must be a DOWNLOADED human recording of the same map");
        std::process::exit(2);
    }
    let (rx, ry, rz, rq) = match first(&args[0]) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("spawnq: reference {} unreadable: {}", args[0], e);
            std::process::exit(2);
        }
    };
    println!("reference {}", args[0]);
    println!("  spawn ({:.3}, {:.3}, {:.3})  q ({:.6}, {:.6}, {:.6}, {:.6})", rx, ry, rz, rq[0], rq[1], rq[2], rq[3]);
    println!("verdict\tdist_m\tabs_dot\tfile");

    let mut bad = false;
    for path in &args[1..] {
        match first(path) {
            Err(e) => {
                println!("UNREADABLE\t-\t-\t{}\t{}", path, e);
                bad = true;
            }
            Ok((x, y, z, q)) => {
                let dist = ((x - rx).powi(2) + (y - ry).powi(2) + (z - rz).powi(2)).sqrt();
                let dot: f64 = (0..4).map(|i| q[i] * rq[i]).sum::<f64>().abs();
                let verdict = if dist > POS_TOL_M {
                    "REFUSED-POSITION"
                } else if dot < DOT_TOL {
                    "REFUSED-ORIENTATION"
                } else {
                    "ok"
                };
                if verdict != "ok" {
                    bad = true;
                }
                println!("{}\t{:.3}\t{:.4}\t{}", verdict, dist, dot, path);
                if verdict == "REFUSED-ORIENTATION" {
                    println!("    q ({:.6}, {:.6}, {:.6}, {:.6}) is a different rotation from the human spawn", q[0], q[1], q[2], q[3]);
                    println!("    the car will face the wrong way for the whole render");
                } else if verdict == "REFUSED-POSITION" {
                    println!("    starts {:.1} m from where every run on this map starts -- another map's telemetry?", dist);
                }
            }
        }
    }
    std::process::exit(if bad { 1 } else { 0 });
}
