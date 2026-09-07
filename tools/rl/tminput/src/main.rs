//! tminput -- the INPUT-SEMANTICS arm's editing tool for the packet STATE WORD.
//!
//! `ghost tape poke` edits steer/accel/brake over a tick range; nothing edited
//! the state word (mode nibble + 22 flag bits) except by hand in a `.gtape`.
//! These commands do that one edit, and only that one, so a minimal pair is a
//! minimal pair: the tape is re-encoded VERBATIM (every same-bit kept) and the
//! only packets whose coding changes are the first tick of the window (an
//! explicit literal carrying the new word) and the first tick after it (an
//! explicit literal restoring the word that was there).
//!
//!   tminput setword IN OUT --ticks A..B --flags HEX [--word0 HEX]
//!   tminput strip   IN OUT [--ticks A..B]          every non-plain word -> plain
//!   tminput words   FILE                            the runs of non-plain words
//!
//! Every write is read back and the tape compared field by field.

use gbx::container::{self, Container};
use gbx::tape::{literal_for, unpack_word_pub, Encoding, Packet, StateEnc, Tape};

fn die<T>(m: impl std::fmt::Display) -> T {
    eprintln!("tminput: {m}");
    std::process::exit(2)
}

fn flag<'a>(a: &'a [String], k: &str) -> Option<&'a String> {
    a.iter().position(|s| s == k).and_then(|i| a.get(i + 1))
}

fn need<'a>(a: &'a [String], k: &str) -> &'a String {
    flag(a, k).unwrap_or_else(|| die(format!("missing {k}")))
}

fn hex(s: &str) -> u32 {
    let t = s.trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(t, 16).unwrap_or_else(|_| die(format!("not hex: {s}")))
}

fn range(s: &str) -> (usize, usize) {
    let mut it = s.splitn(2, "..");
    let a: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| die("--ticks A..B"));
    let b: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| die("--ticks A..B"));
    if b <= a {
        die::<()>("--ticks A..B needs B > A");
    }
    (a, b)
}

fn is_plain(p: &Packet) -> bool {
    let w0 = p.word0 & !0x20;
    p.flags == 0 && w0 == (w0 & 0xF)
}

/// Set (word0, flags) on ticks [a, b) and make the coding carry it: an explicit
/// literal at `a`, `prev` inside, and at `b` an explicit literal of whatever
/// word tick `b` had (unless it already carries one).
fn set_word(t: &mut Tape, a: usize, b: usize, word0: u32, flags: u32) {
    let ar = t.archives.first_mut().unwrap_or_else(|| die("no archive"));
    let n = ar.packets.len();
    if b > n {
        die::<()>(format!("--ticks {a}..{b} but the tape has {n} ticks"));
    }
    let lit = literal_for(word0, flags);
    let (w0, fl) = unpack_word_pub(lit);
    if fl != flags || (w0 & 0xF) != (word0 & 0xF) {
        die::<()>(format!("literal 0x{lit:x} does not round-trip word0 0x{word0:x} flags 0x{flags:x} (got 0x{w0:x}/0x{fl:x})"));
    }
    // what tick b must be restored to
    let restore = if b < n {
        let q = &ar.packets[b];
        match q.state {
            StateEnc::Lit(_) => None,
            _ => Some((q.word0, q.flags)),
        }
    } else {
        None
    };
    for i in a..b {
        let p = &mut ar.packets[i];
        p.word0 = (p.word0 & 0x20) | (w0 & !0x20); // keep the respawn bit
        p.flags = flags;
        p.mode = p.word0 & 0xF;
        p.state = if i == a { StateEnc::Lit(lit) } else { StateEnc::Prev };
    }
    if let Some((rw, rf)) = restore {
        let l = literal_for(rw, rf);
        ar.packets[b].state = StateEnc::Lit(l);
    }
}

fn write_back(c: &Container, t: &Tape, out: &str, enc: Encoding) {
    let body = t.splice_into(c.body(), enc).unwrap_or_else(|e| die(e));
    container::write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
    let back = Tape::from_file(out).unwrap_or_else(|e| die(e));
    let mut bad = 0;
    for (na, ba) in t.archives.iter().zip(back.archives.iter()) {
        if na.packets.len() != ba.packets.len() {
            die::<()>("read-back tick count differs");
        }
        for (i, (p, q)) in na.packets.iter().zip(ba.packets.iter()).enumerate() {
            if p.steer != q.steer
                || p.accel != q.accel
                || p.brake != q.brake
                || p.respawn() != q.respawn()
                || p.mode != q.mode
                || p.mouse != q.mouse
                || p.flags != q.flags
                || (p.word0 & !0x20) != (q.word0 & !0x20)
            {
                if bad < 5 {
                    eprintln!("  tick {i}: read back differently: {:?} vs {:?}", p, q);
                }
                bad += 1;
            }
        }
    }
    if bad > 0 {
        die::<()>(format!("{bad} ticks read back differently; {out} is NOT what was asked for"));
    }
}

fn runs(t: &Tape) -> Vec<(usize, usize, u32, u32)> {
    let mut out = Vec::new();
    let ar = match t.archives.first() {
        Some(a) => a,
        None => return out,
    };
    let n = ar.packets.len();
    let mut i = 0;
    while i < n {
        let p = &ar.packets[i];
        if is_plain(p) {
            i += 1;
            continue;
        }
        let key = (p.word0 & !0x20, p.flags);
        let mut j = i;
        while j < n && (ar.packets[j].word0 & !0x20, ar.packets[j].flags) == key {
            j += 1;
        }
        out.push((i, j, key.0, key.1));
        i = j;
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let op = a.first().map(|s| s.as_str()).unwrap_or_else(|| die("tminput <setword|strip|words> ..."));
    let rest = &a[1..];
    match op {
        "words" => {
            let f = rest.first().unwrap_or_else(|| die("tminput words FILE"));
            let t = Tape::from_file(f).unwrap_or_else(|e| die(e));
            let so = t.archives[0].start_offset_ms as i64;
            println!("tick_from\ttick_to\trace_s_from\trace_s_to\tword0\tflags\tsteer_on\tsteer_last");
            for (i, j, w0, fl) in runs(&t) {
                let p = &t.archives[0].packets;
                println!(
                    "{i}\t{j}\t{:.3}\t{:.3}\t0x{w0:x}\t0x{fl:x}\t{}\t{}",
                    (so + 10 * i as i64) as f64 / 1000.0,
                    (so + 10 * j as i64) as f64 / 1000.0,
                    p[i].steer_i8(),
                    p[j - 1].steer_i8()
                );
            }
        }
        "setword" | "strip" => {
            let inp = rest.first().unwrap_or_else(|| die("tminput {op} IN OUT ..."));
            let out = rest.get(1).unwrap_or_else(|| die("tminput {op} IN OUT ..."));
            let c = Container::load(inp).unwrap_or_else(|e| die(e));
            let mut t = Tape::from_file(inp).unwrap_or_else(|e| die(e));
            t.verbatim_is_identity().unwrap_or_else(|e| die(format!("codec identity failed on {inp}: {e}")));
            let enc = if rest.iter().any(|s| s == "--explicit") { Encoding::Explicit } else { Encoding::Verbatim };
            if op == "setword" {
                let (a0, b0) = range(need(rest, "--ticks"));
                let flags = hex(need(rest, "--flags"));
                let word0 = flag(rest, "--word0").map(|s| hex(s)).unwrap_or(t.archives[0].packets[a0].word0 & 0xF);
                set_word(&mut t, a0, b0, word0, flags);
                println!("set ticks {a0}..{b0} to word0 0x{word0:x} flags 0x{flags:x}");
            } else {
                let (lo, hi) = flag(rest, "--ticks").map(|s| range(s)).unwrap_or((0, t.n()));
                let rs = runs(&t);
                let mut k = 0;
                for (i, j, w0, fl) in rs {
                    if i >= lo && j <= hi {
                        set_word(&mut t, i, j, w0 & 0xF, 0);
                        println!("stripped ticks {i}..{j} (was word0 0x{w0:x} flags 0x{fl:x})");
                        k += 1;
                    }
                }
                println!("{k} runs stripped");
            }
            write_back(&c, &t, out, enc);
            println!("wrote {out}");
        }
        "trace" => trace_cmd(rest),
        "tdiff" => tdiff_cmd(rest),
        o => die(format!("unknown op {o}")),
    }
}

// ---------------------------------------------------------------------------
// Engine trajectory of a tape, in one piece (tmenv::control::flat_trace).
// ---------------------------------------------------------------------------

/// `tminput trace --tape FILE --map MAP --out CSV [--ticks N] [--work DIR]`
///
/// One fresh server, one child running the WHOLE tape from the earliest stop,
/// the car resolved from validator ownership (no scan), one row per engine
/// tick: `race_ms,x,y,z,vx,vy,vz,qw,qx,qy,qz,wetness,cps,speed,yaw,yaw_rate`.
/// Yaw is about the map's up axis (y), from the quaternion; yaw_rate is its
/// per-tick difference in rad/s.
pub fn trace_cmd(rest: &[String]) {
    let tape = need(rest, "--tape");
    let map = need(rest, "--map");
    let out = need(rest, "--out");
    let server = std::env::var("TM_SERVER").unwrap_or_else(|_| die("TM_SERVER not set"));
    let shim = std::env::var("FK_SHIM").unwrap_or_else(|_| die("FK_SHIM not set"));
    let work = flag(rest, "--work").cloned().unwrap_or_else(|| format!("/tmp/tminput-trace-{}", std::process::id()));
    let t = Tape::from_file(tape).unwrap_or_else(|e| die(e));
    let ticks: u64 = flag(rest, "--ticks").map(|s| s.parse().unwrap_or_else(|_| die("--ticks N"))).unwrap_or(t.n() as u64 + 300);
    std::fs::create_dir_all(&work).unwrap_or_else(|e| die(e));
    let rows = tmenv::control::flat_trace(
        std::path::Path::new(&server),
        std::path::Path::new(map),
        std::path::Path::new(&shim),
        std::path::Path::new(&work),
        std::path::Path::new(tape),
        ticks,
    )
    .unwrap_or_else(|e| die(e));
    let mut s = String::from("race_ms,x,y,z,vx,vy,vz,qw,qx,qy,qz,wetness,cps,speed,yaw,yaw_rate\n");
    let mut prev_yaw: Option<f64> = None;
    for r in &rows {
        let speed = (r.vx * r.vx + r.vy * r.vy + r.vz * r.vz).sqrt();
        // car forward axis = rotate (0,0,1) by q; yaw = atan2(fx, fz)
        let (w, x, y, z) = (r.qw, r.qx, r.qy, r.qz);
        let fx = 2.0 * (x * z + w * y);
        let fz = 1.0 - 2.0 * (x * x + y * y);
        let yaw = fx.atan2(fz);
        let rate = match prev_yaw {
            Some(p) => {
                let mut d = yaw - p;
                while d > std::f64::consts::PI {
                    d -= 2.0 * std::f64::consts::PI;
                }
                while d < -std::f64::consts::PI {
                    d += 2.0 * std::f64::consts::PI;
                }
                d * 100.0
            }
            None => 0.0,
        };
        prev_yaw = Some(yaw);
        s.push_str(&format!(
            "{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.7},{:.7},{:.7},{:.7},{:.4},{},{:.4},{:.6},{:.5}\n",
            r.time_ms, r.x, r.y, r.z, r.vx, r.vy, r.vz, r.qw, r.qx, r.qy, r.qz, r.wetness,
            if r.cps == u32::MAX { -1 } else { r.cps as i64 },
            speed, yaw, rate
        ));
    }
    std::fs::write(out, s).unwrap_or_else(|e| die(format!("{out}: {e}")));
    eprintln!(
        "{} rows, race {:.3} .. {:.3}, final cps {}",
        rows.len(),
        rows.first().map(|r| r.time_ms).unwrap_or(0) as f64 / 1000.0,
        rows.last().map(|r| r.time_ms).unwrap_or(0) as f64 / 1000.0,
        rows.last().map(|r| r.cps as i64).unwrap_or(-1)
    );
}

/// `tminput tdiff A.csv B.csv [--tol M]`: first tick where two traces diverge
/// (position distance > tol), and the distance at a few later instants.
pub fn tdiff_cmd(rest: &[String]) {
    let a = rest.first().unwrap_or_else(|| die("tminput tdiff A.csv B.csv"));
    let b = rest.get(1).unwrap_or_else(|| die("tminput tdiff A.csv B.csv"));
    let tol: f64 = flag(rest, "--tol").map(|s| s.parse().unwrap_or_else(|_| die("--tol M"))).unwrap_or(1e-4);
    // --to MS: only compare ticks up to this race time (a press window, say)
    let to_ms: i64 = flag(rest, "--to").map(|s| s.parse().unwrap_or_else(|_| die("--to MS"))).unwrap_or(i64::MAX);
    // Reads `fk trace` / `tmtraj` 30-column CSVs (time_ms,x,y,z,...,yaw at 9,
    // steer at 18) and tminput's own 16-column trace (yaw at 14, yaw_rate 15).
    let load = |p: &str| -> Vec<(i64, [f64; 3], f64, f64)> {
        let txt = std::fs::read_to_string(p).unwrap_or_else(|e| die(format!("{p}: {e}")));
        let mut lines = txt.lines();
        let head: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
        let col = |name: &str| head.iter().position(|h| *h == name);
        let (cx, cy, cz) = (col("x").unwrap_or(1), col("y").unwrap_or(2), col("z").unwrap_or(3));
        let cyaw = col("yaw").unwrap_or(9);
        let csteer = col("steer");
        let mut prev: Option<f64> = None;
        lines
            .filter_map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                if f.len() <= cyaw {
                    return None;
                }
                let g = |i: usize| f[i].parse::<f64>().ok();
                let yaw = g(cyaw)?;
                let extra = match csteer {
                    Some(c) => g(c).unwrap_or(f64::NAN),
                    None => {
                        let r = prev.map(|p| (yaw - p) * 100.0).unwrap_or(0.0);
                        r
                    }
                };
                prev = Some(yaw);
                Some((f[0].parse().ok()?, [g(cx)?, g(cy)?, g(cz)?], yaw, extra))
            })
            .collect()
    };
    let ra = load(a);
    let rb = load(b);
    let mb: std::collections::HashMap<i64, ([f64; 3], f64, f64)> = rb.iter().map(|r| (r.0, (r.1, r.2, r.3))).collect();
    let mut first: Option<i64> = None;
    let mut shared = 0;
    let mut maxd = 0.0f64;
    let mut printed = 0;
    for (t, p, yaw, rate) in &ra {
        if *t > to_ms {
            break;
        }
        if let Some((q, yb, rb_)) = mb.get(t) {
            shared += 1;
            let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
            maxd = maxd.max(d);
            if d > tol && first.is_none() {
                first = Some(*t);
            }
            if first.is_some() && printed < 30 && (*t - first.unwrap()) % 10 == 0 {
                println!(
                    "race {:.3}  |dpos| {:.6} m  yaw A {:.5} B {:.5}  steer|rate A {:.4} B {:.4}",
                    *t as f64 / 1000.0,
                    d,
                    yaw,
                    yb,
                    rate,
                    rb_
                );
                printed += 1;
            }
        }
    }
    match first {
        Some(t) => println!("FIRST DIVERGENCE at race {:.3} (tol {tol} m); {shared} shared ticks, max |dpos| {maxd:.4} m", t as f64 / 1000.0),
        None => println!("IDENTICAL over {shared} shared ticks (tol {tol} m), max |dpos| {maxd:.2e} m"),
    }
}
