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
        "codec" => codec_cmd(rest),
        "squash" => squash_cmd(rest),
        "rescale" => rescale_cmd(rest),
        "certify" => certify_cmd(rest),
        "field0" => field0_cmd(rest),
        "transplant" => transplant_cmd(rest),
        "swaparchive" => swaparchive_cmd(rest),
        "chunkcopy" => chunkcopy_cmd(rest),
        "valset" => valset_cmd(rest),
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
    // --shift-b MS: add MS to B's time labels before pairing (fk trace labels are 10 ms
    // behind the telemetry convention since player)
    let shift_b: i64 = flag(rest, "--shift-b").map(|s| s.parse().unwrap_or_else(|_| die("--shift-b MS"))).unwrap_or(0);
    // --from MS --every MS: print |dpos| on that grid over the whole run (no 30-row cap)
    let from_ms: i64 = flag(rest, "--from").map(|s| s.parse().unwrap_or_else(|_| die("--from MS"))).unwrap_or(i64::MIN);
    let every_ms: Option<i64> = flag(rest, "--every").map(|s| s.parse().unwrap_or_else(|_| die("--every MS")));
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
    let mb: std::collections::HashMap<i64, ([f64; 3], f64, f64)> = rb.iter().map(|r| (r.0 + shift_b, (r.1, r.2, r.3))).collect();
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
            let show = match every_ms {
                Some(e) => *t >= from_ms && (*t - from_ms.max(0)) % e == 0,
                None => first.is_some() && printed < 30 && (*t - first.unwrap()) % 10 == 0,
            };
            if show {
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

// ---------------------------------------------------------------------------
// I4: verbatim vs explicit — does the explicit writer preserve every field?
// ---------------------------------------------------------------------------

/// `tminput codec FILE|DIR... [--oracle N --map-root DIR] [--out DIR]`
///
/// For every ghost: (1) the verbatim identity control; (2) an EXPLICIT
/// re-encode spliced into the file's own body and re-decoded, compared packet
/// by packet on word0/flags/mode/respawn/mouse/steer/accel/brake/tri; (3) the
/// coding census (lit / prev / prev2 state words, vsame vehicle fields, tail
/// bytes). With `--oracle N`, the first N ghosts that carry an action-key word
/// (and N without) are written explicitly and validated on their own map
/// (`<map-root>/<uid>/map.Map.Gbx`) beside the original — same time required.
pub fn codec_cmd(rest: &[String]) {
    let mut files = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        if rest[i].starts_with("--") {
            i += 2;
            continue;
        }
        collect_ghosts(&rest[i], &mut files);
        i += 1;
    }
    files.sort();
    // the same ghost can sit in the bank twice (extracted dir + tar): dedupe by content
    {
        let mut seen = std::collections::HashSet::new();
        files.retain(|f| {
            let b = std::fs::read(f).unwrap_or_default();
            let mut h: u64 = 0xcbf29ce484222325;
            for x in &b {
                h ^= *x as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
            seen.insert((h, b.len()))
        });
    }
    let oracle_n: usize = flag(rest, "--oracle").map(|s| s.parse().unwrap_or(0)).unwrap_or(0);
    let map_root = flag(rest, "--map-root").cloned();
    let outdir = flag(rest, "--out").cloned().unwrap_or_else(|| "/tmp/tminput-codec".to_string());
    std::fs::create_dir_all(&outdir).unwrap_or_else(|e| die(e));
    let (mut n_ok, mut n_verb_fail, mut n_expl_fail, mut n_err) = (0usize, 0usize, 0usize, 0usize);
    let (mut lit, mut prev, mut prev2, mut vsame, mut packets, mut tail_bytes, mut mouse) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut with_ak: Vec<(String, String)> = Vec::new();
    let mut without: Vec<(String, String)> = Vec::new();
    for f in &files {
        let c = match Container::load(f) {
            Ok(c) => c,
            Err(_) => {
                n_err += 1;
                continue;
            }
        };
        let t = match Tape::from_file(f) {
            Ok(t) => t,
            Err(_) => {
                n_err += 1;
                continue;
            }
        };
        if let Err(e) = t.verbatim_is_identity() {
            n_verb_fail += 1;
            println!("VERBATIM FAIL {f}: {e}");
            continue;
        }
        let ar = &t.archives[0];
        packets += ar.packets.len() as u64;
        tail_bytes += ar.tail.len() as u64;
        for p in &ar.packets {
            match p.state {
                StateEnc::Lit(_) => lit += 1,
                StateEnc::Prev => prev += 1,
                StateEnc::Prev2(..) => prev2 += 1,
            }
            vsame += p.vsame as u64;
            mouse += p.mouse.is_some() as u64;
        }
        let body = match t.splice_into(c.body(), Encoding::Explicit) {
            Ok(b) => b,
            Err(e) => {
                n_expl_fail += 1;
                println!("EXPLICIT SPLICE FAIL {f}: {e}");
                continue;
            }
        };
        let back = match Tape::from_body(&body) {
            Ok(b) => b,
            Err(e) => {
                n_expl_fail += 1;
                println!("EXPLICIT READ-BACK FAIL {f}: {e}");
                continue;
            }
        };
        let mut bad = 0;
        for (p, q) in ar.packets.iter().zip(back.archives[0].packets.iter()) {
            if p.word0 != q.word0 || p.flags != q.flags || p.mode != q.mode || p.respawn() != q.respawn() || p.mouse != q.mouse
                || p.steer != q.steer || p.accel != q.accel || p.brake != q.brake || p.tri != q.tri
            {
                bad += 1;
            }
        }
        if bad > 0 || back.archives[0].packets.len() != ar.packets.len() {
            n_expl_fail += 1;
            println!("EXPLICIT FIELD MISMATCH {f}: {bad} packets");
            continue;
        }
        n_ok += 1;
        let has_ak = ar.packets.iter().any(|p| [0x404u32, 0x1020, 0x4000, 0x10000, 0x40000].iter().any(|m| p.flags & m == *m));
        let uid = std::path::Path::new(f)
            .parent()
            .map(|d| if d.file_name().map(|s| s == "ghosts").unwrap_or(false) { d.parent().unwrap_or(d) } else { d })
            .and_then(|d| d.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        // tm-pop is Summer 2026 - 01
        let uid = if uid == "tm-pop" { "buNzfsVlp2NF2oWtHM3729dEylg".to_string() } else { uid };
        let list = if has_ak { &mut with_ak } else { &mut without };
        if list.len() < oracle_n && list.iter().filter(|(u, _)| *u == uid).count() < 2 {
            let name = format!("{}/{}-{}", outdir, uid, std::path::Path::new(f).file_name().unwrap().to_string_lossy());
            let out = name.replace(".Ghost.Gbx", "-explicit.Ghost.Gbx");
            container::write_gbx(&c.gbx, body, &out).unwrap_or_else(|e| die(e));
            std::fs::copy(f, &name).unwrap_or_else(|e| die(e));
            list.push((uid, name));
        }
    }
    println!(
        "codec: {} files, {} ok, {} verbatim-identity failures, {} explicit failures, {} unreadable",
        files.len(),
        n_ok,
        n_verb_fail,
        n_expl_fail,
        n_err
    );
    println!(
        "coding census: {packets} packets; state word lit {lit} ({:.2} %), prev {prev} ({:.2} %), prev2 {prev2} ({:.3} %); vehicle same-bit {vsame} ({:.2} %); mouse present {mouse}; tail bytes {tail_bytes} ({:.1} per file)",
        100.0 * lit as f64 / packets as f64,
        100.0 * prev as f64 / packets as f64,
        100.0 * prev2 as f64 / packets as f64,
        100.0 * vsame as f64 / packets as f64,
        tail_bytes as f64 / n_ok.max(1) as f64
    );
    if oracle_n > 0 {
        let root = map_root.unwrap_or_else(|| die("--oracle needs --map-root DIR"));
        let mut by_map: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
        for (uid, name) in with_ak.iter().chain(without.iter()) {
            let e = by_map.entry(uid.clone()).or_default();
            e.push(name.clone());
            e.push(name.replace(".Ghost.Gbx", "-explicit.Ghost.Gbx"));
        }
        let (mut same, mut differ, mut void) = (0, 0, 0);
        for (uid, names) in &by_map {
            let map = format!("{root}/{uid}/map.Map.Gbx");
            let out = std::process::Command::new("tmauto")
                .arg("verdict")
                .args(names)
                .arg("--map")
                .arg(&map)
                .output()
                .unwrap_or_else(|e| die(format!("tmauto: {e}")));
            let txt = String::from_utf8_lossy(&out.stdout).to_string();
            let mut verdicts: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            for l in txt.lines().skip(1) {
                let cols: Vec<&str> = l.split_whitespace().collect();
                if cols.len() >= 2 {
                    verdicts.insert(cols[0].to_string(), cols[1].to_string());
                }
            }
            for n in names.iter().filter(|n| !n.contains("-explicit")) {
                let base = std::path::Path::new(n).file_name().unwrap().to_string_lossy().to_string();
                let expl = base.replace(".Ghost.Gbx", "-explicit.Ghost.Gbx");
                let a = verdicts.get(&base).cloned().unwrap_or_default();
                let b = verdicts.get(&expl).cloned().unwrap_or_default();
                let ak = with_ak.iter().any(|(_, x)| x == n);
                if a.is_empty() || b.is_empty() {
                    void += 1;
                } else if a == b {
                    same += 1;
                } else {
                    differ += 1;
                }
                println!("oracle {} {:<50} original {:<12} explicit {:<12} {}", if ak { "AK " } else { "-- " }, base, a, b, if a == b { "same" } else { "DIFFER" });
            }
        }
        println!("oracle: {same} same, {differ} differ, {void} void");
    }
}

fn collect_ghosts(p: &str, out: &mut Vec<String>) {
    let path = std::path::Path::new(p);
    if path.is_dir() {
        if let Ok(rd) = std::fs::read_dir(path) {
            for e in rd.flatten() {
                collect_ghosts(&e.path().to_string_lossy(), out);
            }
        }
    } else if p.ends_with(".Ghost.Gbx") || p.ends_with(".Replay.Gbx") {
        out.push(p.to_string());
    }
}

/// `tminput squash IN OUT --ticks A..B --above X --to Y`: every tick in [A,B)
/// whose |steer| ≥ X gets steer sign·Y (the vehicle fields are re-coded
/// explicitly where changed; the state words are untouched). Under a CLAMP at
/// c with Y ≥ 127c this edit is physics-neutral; under a SCALE it is not.
pub fn squash_cmd(rest: &[String]) {
    let inp = rest.first().unwrap_or_else(|| die("tminput squash IN OUT --ticks A..B --above X --to Y"));
    let out = rest.get(1).unwrap_or_else(|| die("tminput squash IN OUT --ticks A..B --above X --to Y"));
    let (a, b) = range(need(rest, "--ticks"));
    let above: i32 = need(rest, "--above").parse().unwrap_or_else(|_| die("--above X"));
    let to: i32 = need(rest, "--to").parse().unwrap_or_else(|_| die("--to Y"));
    let c = Container::load(inp).unwrap_or_else(|e| die(e));
    let mut t = Tape::from_file(inp).unwrap_or_else(|e| die(e));
    t.verbatim_is_identity().unwrap_or_else(|e| die(e));
    let mut n = 0;
    {
        let ar = &mut t.archives[0];
        let hi = b.min(ar.packets.len());
        for i in a..hi {
            let s = ar.packets[i].steer_i8() as i32;
            if s.abs() >= above {
                let v = if s < 0 { -to } else { to } as i8;
                ar.packets[i].steer = (v as u8) as u32;
                ar.packets[i].vsame = false;
                n += 1;
            }
        }
    }
    println!("squashed {n} ticks in {a}..{b} with |steer| >= {above} to ±{to}");
    write_back(&c, &t, out, Encoding::Verbatim);
    println!("wrote {out}");
}

/// `tminput rescale IN OUT --ticks A..B (--mul F | --clamp N)`: rewrite the
/// steer over [A,B) as round(steer × F) or clamp(steer, ±N). State words are
/// untouched (strip them separately). The "replace the action key by its
/// effect" reconstruction: exact only up to i8 rounding.
pub fn rescale_cmd(rest: &[String]) {
    let inp = rest.first().unwrap_or_else(|| die("tminput rescale IN OUT --ticks A..B (--mul F | --clamp N)"));
    let out = rest.get(1).unwrap_or_else(|| die("tminput rescale IN OUT --ticks A..B (--mul F | --clamp N)"));
    let (a, b) = range(need(rest, "--ticks"));
    let mul: Option<f32> = flag(rest, "--mul").map(|s| s.parse().unwrap_or_else(|_| die("--mul F")));
    let clamp: Option<i32> = flag(rest, "--clamp").map(|s| s.parse().unwrap_or_else(|_| die("--clamp N")));
    let c = Container::load(inp).unwrap_or_else(|e| die(e));
    let mut t = Tape::from_file(inp).unwrap_or_else(|e| die(e));
    t.verbatim_is_identity().unwrap_or_else(|e| die(e));
    let mut n = 0;
    {
        let ar = &mut t.archives[0];
        let hi = b.min(ar.packets.len());
        for i in a..hi {
            let s = ar.packets[i].steer_i8() as i32;
            let v = match (mul, clamp) {
                (Some(f), _) => (s as f32 * f).round() as i32,
                (_, Some(k)) => s.clamp(-k, k),
                _ => die("--mul F or --clamp N"),
            };
            if v != s {
                ar.packets[i].steer = (v as i8 as u8) as u32;
                ar.packets[i].vsame = false;
                n += 1;
            }
        }
    }
    println!("rescaled {n} ticks in {a}..{b}");
    write_back(&c, &t, out, Encoding::Verbatim);
    println!("wrote {out}");
}

// ---------------------------------------------------------------------------
// Cross-box certification of a finish: fresh plain oracle, control beside it.
// ---------------------------------------------------------------------------

/// `tminput certify --tape T.Ghost.Gbx --map M.Map.Gbx --control C.Ghost.Gbx --out CERT.md [--expect MS]`
/// Runs `tmauto verdict` (a separate process, no shim) on the tape and the
/// control in one batch, prints the transcript, and writes a certificate with
/// the md5s, the map md5, the box, the verdicts and the timestamp. Exit 3 when
/// the tape did not finish or the control did not reproduce its declared time.
pub fn certify_cmd(rest: &[String]) {
    let tape = need(rest, "--tape");
    let map = need(rest, "--map");
    let control = need(rest, "--control");
    let out = need(rest, "--out");
    let expect: Option<i64> = flag(rest, "--expect").map(|s| s.parse().unwrap_or_else(|_| die("--expect MS")));
    let md5 = |p: &str| -> String {
        let o = std::process::Command::new("md5sum").arg(p).output().unwrap_or_else(|e| die(format!("md5sum: {e}")));
        String::from_utf8_lossy(&o.stdout).split_whitespace().next().unwrap_or("").to_string()
    };
    let o = std::process::Command::new("tmauto")
        .args(["verdict", tape, control, "--map", map])
        .output()
        .unwrap_or_else(|e| die(format!("tmauto: {e}")));
    let txt = String::from_utf8_lossy(&o.stdout).to_string();
    print!("{txt}");
    let verdict_of = |name: &str| -> String {
        let base = std::path::Path::new(name).file_name().unwrap().to_string_lossy().to_string();
        txt.lines().find(|l| l.starts_with(&base)).map(|l| l[base.len()..].trim().to_string()).unwrap_or_else(|| "(no verdict line)".into())
    };
    let vt = verdict_of(tape);
    let vc = verdict_of(control);
    let ctrl_declared = gbx::tape::Tape::from_file(control).ok().and_then(|_| {
        Container::load(control).ok().and_then(|c| c.declared_times().first().map(|t| t.1))
    });
    let tape_ms: Option<i64> = vt.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()).map(|s| (s * 1000.0).round() as i64);
    let ctrl_ms: Option<i64> = vc.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()).map(|s| (s * 1000.0).round() as i64);
    let host = std::process::Command::new("hostname").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let ok_tape = tape_ms.is_some() && expect.map(|e| Some(e) == tape_ms).unwrap_or(true);
    let ok_ctrl = ctrl_ms.is_some() && ctrl_declared.map(|d| d as i64 == ctrl_ms.unwrap()).unwrap_or(true);
    let now = std::process::Command::new("date").arg("+%Y-%m-%d %H:%M %Z").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let cert = format!(
        "# {} — cross-box re-simulation ({now}, box {host}, plain `tmauto verdict`, no shim)\n\
         Tape: {tape} (md5 {})\nMap: {map} (md5 {})\nControl: {control} (md5 {}, declared {})\n\n\
         | file | verdict |\n|---|---|\n| tape | {vt} |\n| control | {vc} |\n\n{}\n",
        match tape_ms { Some(ms) => format!("{}: {:.3}", if ok_tape { "CERTIFIED" } else { "NOT CERTIFIED" }, ms as f64 / 1000.0), None => "NOT CERTIFIED: no finish".to_string() },
        md5(tape), md5(map), md5(control),
        ctrl_declared.map(|d| format!("{:.3}", d as f64 / 1000.0)).unwrap_or_else(|| "?".into()),
        if ok_tape && ok_ctrl { "Both hold: the tape finishes at the time claimed and the control reproduces its declared time on this box.".to_string() }
        else { format!("PROBLEM: tape ok = {ok_tape}, control ok = {ok_ctrl}{}", expect.map(|e| format!(" (expected {:.3})", e as f64 / 1000.0)).unwrap_or_default()) }
    );
    std::fs::write(out, &cert).unwrap_or_else(|e| die(format!("{out}: {e}")));
    print!("{cert}");
    if !(ok_tape && ok_ctrl) {
        std::process::exit(3);
    }
}

/// `tminput field0 IN OUT --value N`: rewrite archive 0's `field0` header word
/// (the u32 after format_version) and nothing else. For the container-identity
/// question: what in a ghost, outside the packets, changes how the engine reads
/// a partial steer?
pub fn field0_cmd(rest: &[String]) {
    let inp = rest.first().unwrap_or_else(|| die("tminput field0 IN OUT --value N"));
    let out = rest.get(1).unwrap_or_else(|| die("tminput field0 IN OUT --value N"));
    let v: u32 = need(rest, "--value").parse().unwrap_or_else(|_| die("--value N"));
    let c = Container::load(inp).unwrap_or_else(|e| die(e));
    let mut t = Tape::from_file(inp).unwrap_or_else(|e| die(e));
    t.verbatim_is_identity().unwrap_or_else(|e| die(e));
    println!("field0 {} -> {}", t.archives[0].field0, v);
    t.archives[0].field0 = v;
    write_back(&c, &t, out, Encoding::Verbatim);
    println!("wrote {out}");
}

/// `tminput transplant --from A --into B --out OUT [--explicit]`: B's container and
/// packets, with A's steer/accel/brake at every tick whose RACE time A also has
/// (race = start_offset + 10·tick, so containers with different countdown
/// lengths align on race time, not tick index). Ticks A does not cover keep B's.
pub fn transplant_cmd(rest: &[String]) {
    let a = need(rest, "--from");
    let b = need(rest, "--into");
    let out = need(rest, "--out");
    let ta = Tape::from_file(a).unwrap_or_else(|e| die(e));
    let c = Container::load(b).unwrap_or_else(|e| die(e));
    let mut tb = Tape::from_file(b).unwrap_or_else(|e| die(e));
    tb.verbatim_is_identity().unwrap_or_else(|e| die(e));
    let (aa, ab) = (&ta.archives[0], &mut tb.archives[0]);
    let (mut copied, mut kept) = (0, 0);
    for (i, p) in ab.packets.iter_mut().enumerate() {
        let race = ab.start_offset_ms as i64 + 10 * i as i64;
        let j = (race - aa.start_offset_ms as i64) / 10;
        if j >= 0 && (j as usize) < aa.packets.len() && (race - aa.start_offset_ms as i64) % 10 == 0 {
            let q = &aa.packets[j as usize];
            if p.steer != q.steer || p.accel != q.accel || p.brake != q.brake {
                p.steer = q.steer;
                p.accel = q.accel;
                p.brake = q.brake;
                p.vsame = false;
            }
            copied += 1;
        } else {
            kept += 1;
        }
    }
    println!("transplanted {copied} ticks by race time ({kept} of B's ticks outside A kept); A start {} B start {}", aa.start_offset_ms, ab.start_offset_ms);
    let enc = if rest.iter().any(|s| s == "--explicit") { Encoding::Explicit } else { Encoding::Verbatim };
    write_back(&c, &tb, out, enc);
    println!("wrote {out}");
}

/// `tminput swaparchive --from A --into B --out OUT`: B's container with A's WHOLE
/// input archive (every packet, start_offset, length) in place of B's.
pub fn swaparchive_cmd(rest: &[String]) {
    let a = need(rest, "--from");
    let b = need(rest, "--into");
    let out = need(rest, "--out");
    let ta = Tape::from_file(a).unwrap_or_else(|e| die(e));
    let c = Container::load(b).unwrap_or_else(|e| die(e));
    let mut tb = Tape::from_file(b).unwrap_or_else(|e| die(e));
    tb.archives[0] = ta.archives[0].clone();
    let body = tb.splice_into(c.body(), Encoding::Verbatim).unwrap_or_else(|e| die(e));
    container::write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
    let back = Tape::from_file(out).unwrap_or_else(|e| die(e));
    println!("wrote {out}: {} ticks, start_offset {} (A had {} ticks, {})", back.n(), back.archives[0].start_offset_ms, ta.n(), ta.archives[0].start_offset_ms);
}

fn find_skippable(body: &[u8], id: u32) -> Option<(usize, usize, usize)> {
    let mut i = 0usize;
    while i + 12 <= body.len() {
        if u32::from_le_bytes(body[i..i + 4].try_into().unwrap()) == id && &body[i + 4..i + 8] == gbx::container::SKIP_MAGIC {
            let size = u32::from_le_bytes(body[i + 8..i + 12].try_into().unwrap()) as usize;
            if i + 12 + size <= body.len() {
                return Some((i, i + 12, size));
            }
        }
        i += 1;
    }
    None
}

/// `tminput chunkcopy --from A --into B --out OUT --chunk 0x0309202D`: B with A's
/// payload for one skippable chunk (the validation block 0x0309202D, say).
/// For the container-identity question: which chunk carries the input setting?
pub fn chunkcopy_cmd(rest: &[String]) {
    let a = need(rest, "--from");
    let b = need(rest, "--into");
    let out = need(rest, "--out");
    let id = hex(need(rest, "--chunk"));
    let ca = Container::load(a).unwrap_or_else(|e| die(e));
    let cb = Container::load(b).unwrap_or_else(|e| die(e));
    let (_, pa, sa) = find_skippable(ca.body(), id).unwrap_or_else(|| die(format!("{a}: no chunk {id:#x}")));
    let (ob, pb, sb) = find_skippable(cb.body(), id).unwrap_or_else(|| die(format!("{b}: no chunk {id:#x}")));
    let mut body = Vec::with_capacity(cb.body().len() + sa);
    body.extend_from_slice(&cb.body()[..ob + 8]);
    body.extend_from_slice(&(sa as u32).to_le_bytes());
    body.extend_from_slice(&ca.body()[pa..pa + sa]);
    body.extend_from_slice(&cb.body()[pb + sb..]);
    container::write_gbx(&cb.gbx, body, out).unwrap_or_else(|e| die(e));
    println!("wrote {out}: chunk {id:#x} payload {sb} B -> {sa} B from {a}");
}

/// `tminput valset IN OUT --field seed|u02|u03|u04|wall_start|wall_end|checksum|os|cpu --value N`
/// Rewrite one u32 of the validation block (0x0309202D) in place. Layout (gbx::manifest):
/// u01, exe string, checksum, os, cpu, wall_start, wall_end, title string, 32-byte title checksum,
/// u02 (settings flags), u03 (start cp index), seed, u04, settings string.
pub fn valset_cmd(rest: &[String]) {
    let inp = rest.first().unwrap_or_else(|| die("tminput valset IN OUT --field F --value N"));
    let out = rest.get(1).unwrap_or_else(|| die("tminput valset IN OUT --field F --value N"));
    let field = need(rest, "--field").as_str();
    let value: i64 = need(rest, "--value").parse().unwrap_or_else(|_| die("--value N"));
    let c = Container::load(inp).unwrap_or_else(|e| die(e));
    let mut body = c.body().to_vec();
    let (_, p, n) = find_skippable(&body, 0x0309_202D).unwrap_or_else(|| die("no validation chunk"));
    let end = p + n;
    let mut o = p;
    let rd = |o: &mut usize, body: &[u8]| -> u32 { let v = u32::from_le_bytes(body[*o..*o + 4].try_into().unwrap()); *o += 4; v };
    let skip_string = |o: &mut usize, body: &[u8]| { let l = u32::from_le_bytes(body[*o..*o + 4].try_into().unwrap()) as usize; *o += 4 + l; };
    let flag = rd(&mut o, &body);
    if flag != 0 { die::<()>("embedded-inputs validation block: unsupported"); }
    skip_string(&mut o, &body); // exe
    let off_checksum = o; o += 4;
    let off_os = o; o += 4;
    let off_cpu = o; o += 4;
    let off_ws = o; o += 4;
    let off_we = o; o += 4;
    skip_string(&mut o, &body); // title
    o += 32; // title checksum
    let off_u02 = o; o += 4;
    let off_u03 = o; o += 4;
    let off_seed = o; o += 4;
    let off_u04 = o;
    let _ = end;
    let off = match field {
        "seed" => off_seed, "u02" | "settings_flags" => off_u02, "u03" => off_u03, "u04" => off_u04,
        "wall_start" => off_ws, "wall_end" => off_we, "checksum" => off_checksum, "os" => off_os, "cpu" => off_cpu,
        _ => die(format!("unknown field {field}")),
    };
    let old = u32::from_le_bytes(body[off..off + 4].try_into().unwrap());
    body[off..off + 4].copy_from_slice(&(value as u32).to_le_bytes());
    container::write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
    println!("wrote {out}: {field} {old} ({}) -> {value}", old as i32);
}
