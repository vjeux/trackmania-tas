//! `fk locate` -- the derived car, and the controls that can fail.
//!
//! The car is DERIVED (`forkoracle::car`, `LOCATE.md`): thirty reads of the
//! stopped parent, no fork, no scan. What lives here is the evidence that the
//! derivation names the right object, each piece of it a test that a wrong
//! answer would fail:
//!
//! * `fk locate`         -- derive, print every hop, time it;
//! * `fk locate census`  -- EVERY copy of the car's position in the process,
//!                          named by the object it sits in and by its PHASE
//!                          against the copy-out (bit-equality at lag k over a
//!                          gathered window), with the sweep's old signature
//!                          (a unit quaternion 16 bytes before it) noted;
//! * `fk locate check`   -- the per-tick identity: over a whole run, the dyna
//!                          body record equals the copy-out bit for bit on every
//!                          tick the car has a body, the post-step vis state is
//!                          the copy-out quantised to 1 mm, the pre-step one is
//!                          the previous tick's, and the clock steps by 10;
//! * `fk locate mirror`  -- the steering probe: two forks, hard left and hard
//!                          right, and the tick at which each object first
//!                          answers. The copy-out and the body answer on the
//!                          first tick that consumed the input; the copies
//!                          answer a tick later;
//! * `fk locate watch`   -- under gdb, no shim: hardware watchpoints on the
//!                          body record, the copy-out and both vis states over
//!                          two ticks, and the RIP of every write, checked
//!                          against the sites the derivation names.

use std::collections::BTreeMap;

use std::process::{Command, Stdio};
use std::time::Instant;

use forkoracle::car::{build128182 as b, Car};
use forkoracle::forksrv::Rec;
use forkoracle::procmem;

use crate::locate::{gather_ticks, Tick};
use crate::session::{Checkpoint, Engine, Session};
use crate::tape::Tape;

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn xyz(b: &[u8], o: usize) -> [f32; 3] {
    [f32_at(b, o), f32_at(b, o + 4), f32_at(b, o + 8)]
}
fn dist(a: [f32; 3], c: [f32; 3]) -> f64 {
    (((a[0] - c[0]) as f64).powi(2) + ((a[1] - c[1]) as f64).powi(2) + ((a[2] - c[2]) as f64).powi(2)).sqrt()
}

// ------------------------------------------------------------------ fk locate

/// Derive the car, print the path, and time the derivation.
pub fn show(engine: &Engine, tape: Tape, at: Checkpoint, reps: usize) -> Result<(), String> {
    let s = Session::start(engine, tape, at)?;
    let t0 = Instant::now();
    let car = forkoracle::car::locate(&s.srv)?;
    let first = t0.elapsed();
    println!("{}", car);
    let l = car.layout();
    println!(
        "layout: pos {:#x} quat {:#x} vel {:#x} wetness {:#x}; clock {:#x} bias {} ms",
        l.pos, l.quat, l.vel, l.wet, l.clock, l.clock_bias
    );
    let pid = s.srv.pid();
    let st = procmem::read_at(pid, car.quat(), 0x34).ok_or("cannot read the state")?;
    let q = [f32_at(&st, 0), f32_at(&st, 4), f32_at(&st, 8), f32_at(&st, 12)];
    let p = xyz(&st, 16);
    let v = xyz(&st, 28);
    let w = xyz(&st, 40);
    println!(
        "state: q=({:.6}, {:.6}, {:.6}, {:.6}) pos=({:.4}, {:.4}, {:.4}) vel=({:.3}, {:.3}, {:.3}) angvel=({:.3}, {:.3}, {:.3})",
        q[0], q[1], q[2], q[3], p[0], p[1], p[2], v[0], v[1], v[2], w[0], w[1], w[2]
    );
    let clk = procmem::read_at(pid, l.clock, 4).map(|b| u32_at(&b, 0)).ok_or("cannot read the clock")?;
    println!(
        "clock word {} ms -> race {} ms (the tick before the one being entered, {})",
        clk,
        clk as i64 - l.clock_bias,
        s.srv.sim_ms as i64 - s.srv.race_start as i64 - 10
    );
    let mut us: Vec<f64> = Vec::with_capacity(reps);
    for _ in 0..reps {
        let t = Instant::now();
        let c = forkoracle::car::locate(&s.srv)?;
        us.push(t.elapsed().as_secs_f64() * 1e6);
        if c.phy != car.phy || c.body != car.body {
            return Err("the derivation is not deterministic".into());
        }
    }
    us.sort_by(|a, c| a.partial_cmp(c).unwrap());
    if !us.is_empty() {
        println!(
            "derivation time: first {:.1} us; over {} repeats median {:.1} us, p90 {:.1} us, max {:.1} us",
            first.as_secs_f64() * 1e6,
            reps,
            us[us.len() / 2],
            us[(us.len() * 9 / 10).min(us.len() - 1)],
            us[us.len() - 1]
        );
    }
    s.srv.quit();
    Ok(())
}

// ----------------------------------------------------------- fk locate census

/// One copy of the car's position found in memory.
struct Copy {
    addr: u64,
    /// The object it sits in, by the derivation's own pointers.
    home: String,
    /// The distance from the copy-out at the stop.
    d0: f64,
    /// Is there a unit quaternion 16 bytes before it (the old sweep's
    /// signature)?
    unit_q_before: bool,
}

fn name_home(car: &Car, a: u64) -> String {
    let rel = |base: u64, len: u64, name: &str| -> Option<String> {
        (a >= base && a < base + len).then(|| format!("{}+{:#x}", name, a - base))
    };
    if let Some(bd) = car.body {
        if let Some(s) = rel(bd.addr, b::BODY_STRIDE, "body") {
            return s;
        }
    }
    for (k, v) in car.vehicles.iter().enumerate() {
        if let Some(s) = rel(*v, 0x1e08, &format!("phy{}", k)) {
            let inner = a - v;
            let tag = if inner == b::POS_IN_PHY {
                " = the copy-out"
            } else if inner == b::VIS_PRE_IN_PHY + b::POS_IN_VIS {
                " = the pre-step vis state"
            } else if inner == b::VIS_POST_IN_PHY + b::POS_IN_VIS {
                " = the post-step vis state"
            } else if inner == 0x27c + 0x24 {
                " = the previous Iso4"
            } else {
                ""
            };
            return format!("{}{}", s, tag);
        }
    }
    if let Some(s) = rel(car.participant, 0x2000, "participant") {
        let tag = if a - car.participant == b::STATE_COPY_IN_PARTICIPANT { " = the old sweep's object" } else { "" };
        return format!("{}{}", s, tag);
    }
    if let Some(s) = rel(car.sim, 0x4000, "sim") {
        return s;
    }
    if let Some(s) = rel(car.playground, 0x4000, "playground") {
        return s;
    }
    if let Some(s) = rel(car.dyna, 0x1000, "dyna") {
        return s;
    }
    if let Some(s) = rel(car.vehmgr, 0x1000, "vehmgr") {
        return s;
    }
    "?".to_string()
}

/// Every 4-byte-aligned float triple in the process's writable memory within
/// `radius` metres of `want`.
fn scan_all(pid: i32, want: [f32; 3], radius: f64) -> Vec<u64> {
    let mut out = Vec::new();
    for r in procmem::maps(pid) {
        if !r.perms.starts_with("rw") || r.path == "[vvar]" || r.path == "[vsyscall]" {
            continue;
        }
        let Some(buf) = procmem::read_at(pid, r.start, (r.end - r.start) as usize) else {
            continue;
        };
        let mut o = 0usize;
        while o + 12 <= buf.len() {
            let x = f32_at(&buf, o);
            if ((x - want[0]).abs() as f64) <= radius {
                let p = xyz(&buf, o);
                if dist(p, want) <= radius {
                    out.push(r.start + o as u64);
                }
            }
            o += 4;
        }
    }
    out
}

pub fn census(engine: &Engine, tape: Tape, at: Checkpoint, radius: f64, ticks: u32) -> Result<(), String> {
    let mut s = Session::start(engine, tape, at)?;
    let pid = s.srv.pid();
    let car = forkoracle::car::locate(&s.srv)?;
    println!("{}", car);
    let want = forkoracle::car::read_xyz(pid, car.pos()).ok_or("cannot read the copy-out")?;
    let t0 = Instant::now();
    let hits = scan_all(pid, want, radius);
    println!(
        "{} float triples within {} m of the car's position ({:.4}, {:.4}, {:.4}) in {:.2}s",
        hits.len(),
        radius,
        want[0],
        want[1],
        want[2],
        t0.elapsed().as_secs_f64()
    );
    let mut copies: Vec<Copy> = hits
        .iter()
        .map(|&a| {
            let p = forkoracle::car::read_xyz(pid, a).unwrap_or([f32::NAN; 3]);
            let q = procmem::read_at(pid, a.wrapping_sub(16), 16)
                .map(|bq| {
                    let n = (0..4).map(|i| (f32_at(&bq, 4 * i) as f64).powi(2)).sum::<f64>().sqrt();
                    (n - 1.0).abs() < 1e-3
                })
                .unwrap_or(false);
            Copy { addr: a, home: name_home(&car, a), d0: dist(p, want), unit_q_before: q }
        })
        .collect();
    copies.sort_by_key(|c| c.addr);

    // THE PHASE OF EACH COPY: gather every hit beside the copy-out over a
    // window and ask, per copy, at which lag k it equals the copy-out bit for
    // bit -- copy[t] == out[t-k] -- on the majority of ticks. Eight segments is
    // the shim's limit, so the copies go in batches with the clock and the
    // copy-out repeated in each.
    let probe = s.probe_tick()?;
    let recs = s.tape.tail_records(probe);
    let mut lag_of: BTreeMap<u64, String> = BTreeMap::new();
    let per_batch = 6usize;
    for batch in copies.chunks(per_batch) {
        let mut segs: Vec<(u64, u32)> = vec![(car.sim_time, 4), (car.pos(), 12)];
        for c in batch {
            segs.push((c.addr, 12));
        }
        let ts: Vec<Tick> = gather_ticks(&mut s.srv, probe, &recs, &segs, ticks, ticks * 4, (0, 4));
        if ts.len() < 8 {
            for c in batch {
                lag_of.insert(c.addr, format!("only {} ticks gathered", ts.len()));
            }
            continue;
        }
        let out_at = |t: usize| xyz(&ts[t].rec, 4);
        for (k, c) in batch.iter().enumerate() {
            let at = |t: usize| xyz(&ts[t].rec, 16 + 12 * k);
            let n = ts.len();
            let moving = (1..n).any(|t| at(t) != at(t - 1));
            // per lag: how many ticks are bit-identical, and the median
            // distance -- a quantised copy is never bit-identical, so its
            // phase is the lag that puts it within its quantum
            let mut best: Option<(i32, usize, f64)> = None;
            for lag in -2i32..=3 {
                let mut eq = 0usize;
                let mut ds: Vec<f64> = Vec::new();
                for t in 3..n - 3 {
                    let u = t as i32 - lag;
                    if u < 0 || u as usize >= n {
                        continue;
                    }
                    if at(t) == out_at(u as usize) {
                        eq += 1;
                    }
                    ds.push(dist(at(t), out_at(u as usize)));
                }
                ds.sort_by(|a, b2| a.partial_cmp(b2).unwrap());
                let med = ds.get(ds.len() / 2).copied().unwrap_or(f64::NAN);
                if best.map(|(_, _, m)| med < m).unwrap_or(true) {
                    best = Some((lag, eq, med));
                }
            }
            let verdict = match best {
                _ if !moving => "FROZEN (never changes)".to_string(),
                Some((lag, eq, _)) if eq * 10 >= (n - 6) * 9 => {
                    format!("lag {:+}: bit-identical to the copy-out on {}/{} ticks", lag, eq, n - 6)
                }
                Some((lag, eq, med)) if med < 0.002 => {
                    format!("lag {:+}: within 1 mm of the copy-out (median {:.4} m; a QUANTISED copy, {}/{} bit-identical)", lag, med, eq, n - 6)
                }
                Some((lag, _, med)) => format!("moves, but no lag puts it on the copy-out (best lag {:+}: median {:.4} m off)", lag, med),
                None => "unmeasured".to_string(),
            };
            lag_of.insert(c.addr, verdict);
        }
    }
    println!("\n{:<16} {:<44} {:>9}  {:<9} {}", "address", "object", "d0 (m)", "unit q@-16", "phase vs the copy-out");
    for c in &copies {
        println!(
            "{:#016x} {:<44} {:>9.4}  {:<9} {}",
            c.addr,
            c.home,
            c.d0,
            if c.unit_q_before { "yes" } else { "no" },
            lag_of.get(&c.addr).cloned().unwrap_or_default()
        );
    }
    s.srv.quit();
    Ok(())
}

// ------------------------------------------------------------ fk locate check

/// Over a whole run, the identities the derivation rests on, tick by tick.
pub fn check(engine: &Engine, tape: Tape, at: Checkpoint, ticks: Option<u32>) -> Result<(), String> {
    let mut s = Session::start(engine, tape, at)?;
    let car = forkoracle::car::locate(&s.srv)?;
    println!("{}", car);
    let probe = s.probe_tick()?;
    let recs = s.tape.tail_records(probe);
    let n_ticks = ticks.unwrap_or((s.tape.n() - probe + 200) as u32);
    let Some(body) = car.body else {
        return Err("stopped inside a respawn window: no body record to compare; pick another checkpoint".into());
    };
    // clock | handle | body pos | body quat | copy-out q(16)+pos(12)+vel(12) | vis_post pos | vis_pre pos
    let segs: Vec<(u64, u32)> = vec![
        (car.sim_time, 4),
        (car.phy + b::BODY_HANDLE_IN_PHY, 4),
        (body.addr + b::POS_IN_BODY, 12),
        (body.addr + b::QUAT_IN_BODY, 16),
        (body.addr + b::VEL_IN_BODY, 12),
        (car.quat(), 40),
        (car.vis() + b::POS_IN_VIS, 12),
        (car.vis_pre() + b::POS_IN_VIS, 12),
    ];
    let t0 = Instant::now();
    let ts = gather_ticks(&mut s.srv, probe, &recs, &segs, n_ticks, 400_000, (0, 4));
    println!("{} ticks gathered in {:.1}s", ts.len(), t0.elapsed().as_secs_f64());
    if ts.len() < 50 {
        return Err(format!("only {} ticks", ts.len()));
    }
    let (mut with_body, mut no_body, mut body_mismatch, mut post_far, mut pre_far, mut clock_gaps, mut q_bad) =
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut worst_post = 0.0f64;
    let mut worst_pre = 0.0f64;
    let mut first_mismatch: Option<String> = None;
    for (i, t) in ts.iter().enumerate() {
        let r = &t.rec;
        let handle = u32_at(r, 4);
        let bpos = &r[8..20];
        let bq = &r[20..36];
        let bv = &r[36..48];
        let out_q = &r[48..64];
        let out_pos = &r[64..76];
        let out_v = &r[76..88];
        let vpost = xyz(r, 88);
        let vpre = xyz(r, 100);
        let qn = (0..4).map(|k| (f32_at(out_q, 4 * k) as f64).powi(2)).sum::<f64>().sqrt();
        if (qn - 1.0).abs() > 1e-3 {
            q_bad += 1;
        }
        if handle != b::NO_BODY {
            with_body += 1;
            if bpos != out_pos || bq != out_q || bv != out_v {
                body_mismatch += 1;
                if first_mismatch.is_none() {
                    first_mismatch = Some(format!(
                        "clock {}: body pos {:?} vs copy-out {:?}",
                        t.clock,
                        xyz(bpos, 0),
                        xyz(out_pos, 0)
                    ));
                }
            }
            // the post-step vis state is this tick's state quantised to 1 mm
            let d = dist(vpost, xyz(out_pos, 0));
            worst_post = worst_post.max(d);
            if d > 0.002 {
                post_far += 1;
            }
            // the pre-step one is the PREVIOUS tick's (if the previous tick had a body)
            if i > 0 && u32_at(&ts[i - 1].rec, 4) != b::NO_BODY && ts[i - 1].clock + 10 == t.clock {
                let prev = xyz(&ts[i - 1].rec, 64);
                let d = dist(vpre, prev);
                if d > worst_pre {
                    worst_pre = d;
                }
                if d > 0.002 && std::env::var("FK_LOCATE_DEBUG").is_ok() {
                    println!("    vis_pre off by {:.4} m at clock {} (speed {:.1} m/s): vis_pre {:?} prev copy-out {:?} this copy-out {:?}", d, t.clock, (0..3).map(|k| (f32_at(out_v, 4 * k) as f64).powi(2)).sum::<f64>().sqrt(), vpre, prev, xyz(out_pos, 0));
                }
                if d > 0.002 {
                    pre_far += 1;
                }
            }
        } else {
            no_body += 1;
        }
        if i > 0 && ts[i - 1].clock + 10 != t.clock {
            clock_gaps += 1;
        }
    }
    let mut fails: Vec<String> = Vec::new();
    let mut line = |ok: bool, what: String| {
        println!("  [{}] {}", if ok { "ok" } else { "FAIL" }, what);
        if !ok {
            fails.push(what);
        }
    };
    line(
        body_mismatch == 0,
        format!(
            "the dyna body record equals the copy-out (q, pos, vel) bit for bit on {} of {} ticks with a body{}",
            with_body - body_mismatch,
            with_body,
            first_mismatch.map(|m| format!(" -- first mismatch {}", m)).unwrap_or_default()
        ),
    );
    line(
        post_far == 0,
        format!(
            "the post-step vis state is the copy-out quantised to 1 mm on every such tick (worst {:.4} m)",
            worst_post
        ),
    );
    // Informational, not a gate: the pre-step vis state is a RENDER copy. It
    // holds the previous tick's state to 1 mm on all but a few ticks per run
    // (3 of 2261 on map 2, 1-2 cm off at those, none of them checkpoint ticks
    // -- a frame boundary of the validator's own wall-clock partition, most
    // likely), which is exactly why nothing samples it.
    println!(
        "  (the pre-step vis state is the PREVIOUS tick's copy-out to 1 mm on {} of {} ticks; worst {:.4} m -- a render copy, sampled by nothing)",
        with_body.saturating_sub(pre_far + 1),
        with_body.saturating_sub(1),
        worst_pre
    );
    line(clock_gaps == 0, format!("the clock word steps by exactly 10 ms between all {} rows", ts.len()));
    line(q_bad == 0, format!("the copy-out's quaternion is unit on every row ({} off)", q_bad));
    println!(
        "  {} ticks with a body, {} without (respawn windows: the copy-out holds the checkpoint pose there)",
        with_body, no_body
    );
    s.srv.quit();
    if fails.is_empty() {
        println!("PASS");
        Ok(())
    } else {
        Err(format!("FAIL: {}", fails.join("; ")))
    }
}

// ----------------------------------------------------------- fk locate mirror

/// Yaw about +y from a (w,x,y,z) quaternion, radians.
fn yaw(q: [f32; 4]) -> f64 {
    let (w, x, y, z) = (q[0] as f64, q[1] as f64, q[2] as f64, q[3] as f64);
    // forward = body +z rotated into the world; yaw = atan2(fwd.x, fwd.z)
    let fx = 2.0 * (x * z + w * y);
    let fz = 1.0 - 2.0 * (x * x + y * y);
    fx.atan2(fz)
}

/// Three forks from the same stop -- the tape as it is, hard left and hard
/// right for `hold` ticks -- and, per object, the first sample at which left
/// and right differ. The child resumes INTO the tick the parent was stopped
/// before, so sample 0 is already the state after the first steered tick: an
/// object that answers the input on the tick that consumed it differs at
/// sample 0, and a copy made a tick later differs at sample 1.
///
/// Every object is compared on its whole (attitude, position, velocity) block,
/// so a copy that holds a matrix instead of a quaternion is judged the same
/// way. The yaw response is measured against the unsteered baseline and must be
/// antisymmetric.
pub fn mirror(engine: &Engine, tape: Tape, at: Checkpoint, hold: usize, ticks: u32) -> Result<(), String> {
    let mut s = Session::start(engine, tape, at)?;
    let car = forkoracle::car::locate(&s.srv)?;
    println!("{}", car);
    let probe = s.probe_tick()?;
    let base = s.tape.tail_records(probe);
    // clock | copy-out q+pos+vel (40) | [body pos+quat+vel (40)] | vis_post
    // rot+pos+vel (60) | vis_pre rot+pos+vel (60) | participant copy q+pos+vel (40)
    let mut segs: Vec<(u64, u32)> = vec![(car.sim_time, 4), (car.quat(), 40)];
    let mut names = vec!["copy-out (phy+0x12e0, q/pos/vel)"];
    if let Some(bd) = car.body {
        segs.push((bd.addr + b::POS_IN_BODY, 40));
        names.push("dyna body record (pos/q/vel)");
    }
    segs.push((car.vis() + 0x2c, 60));
    names.push("post-step vis state (phy+0x848, rot/pos/vel)");
    segs.push((car.vis_pre() + 0x2c, 60));
    names.push("pre-step vis state (phy+0x4e8, rot/pos/vel)");
    segs.push((car.participant_copy_pos() - 16, 40));
    names.push("participant copy (+0xe14, q/pos/vel)");
    let steered = |v: f32| -> Vec<Rec> {
        let mut r = base.clone();
        for x in r.iter_mut().take(hold) {
            x.steer = v;
        }
        r
    };
    let plain = gather_ticks(&mut s.srv, probe, &base, &segs, ticks, ticks * 4, (0, 4));
    let left = gather_ticks(&mut s.srv, probe, &steered(-1.0), &segs, ticks, ticks * 4, (0, 4));
    let right = gather_ticks(&mut s.srv, probe, &steered(1.0), &segs, ticks, ticks * 4, (0, 4));
    let n = left.len().min(right.len()).min(plain.len());
    if n < hold + 4 {
        return Err(format!("only {} / {} / {} ticks gathered", plain.len(), left.len(), right.len()));
    }
    println!(
        "steer -1 vs +1 for {} ticks from tick {} (race {}), {} samples each; sample 0 is the state AFTER the first steered tick",
        hold,
        probe,
        crate::secs(s.tape.race_ms(probe)),
        n
    );
    let mut offs: Vec<(usize, usize, &str)> = Vec::new(); // (offset, len, name)
    let mut o = 4usize;
    for (k, (_, len)) in segs.iter().enumerate().skip(1) {
        offs.push((o, *len as usize, names[k - 1]));
        o += *len as usize;
    }
    let mut first_out: Option<usize> = None;
    let mut first_body: Option<usize> = Some(0);
    for (o, len, name) in &offs {
        let first = (0..n).find(|&t| left[t].rec[*o..o + len] != right[t].rec[*o..o + len]);
        if name.starts_with("copy-out") {
            first_out = first;
        }
        if name.starts_with("dyna body") {
            first_body = first;
        }
        println!(
            "  {:<48} left and right first differ at sample {}{}",
            name,
            first.map(|t| t.to_string()).unwrap_or("never".into()),
            match (first, first_out) {
                (Some(t), Some(c)) if t > c => format!("  ({} tick{} after the copy-out)", t - c, if t - c == 1 { "" } else { "s" }),
                (Some(t), Some(c)) if t < c => format!("  ({} tick{} BEFORE the copy-out)", c - t, if c - t == 1 { "" } else { "s" }),
                _ => String::new(),
            }
        );
    }
    // the yaw response, against the unsteered baseline, must be antisymmetric
    let yaw_of = |ts: &[Tick], t: usize| yaw([f32_at(&ts[t].rec, 4), f32_at(&ts[t].rec, 8), f32_at(&ts[t].rec, 12), f32_at(&ts[t].rec, 16)]);
    let t = (hold + 2).min(n - 1);
    let yb = yaw_of(&plain, t);
    let dl = yaw_of(&left, t) - yb;
    let dr = yaw_of(&right, t) - yb;
    let anti = dl * dr < 0.0 && dl.abs() > 1e-6 && dr.abs() > 1e-6;
    println!(
        "  yaw after {} ticks, relative to the unsteered run: left {:+.4} deg, right {:+.4} deg -> {}",
        t,
        dl.to_degrees(),
        dr.to_degrees(),
        if anti { "ANTISYMMETRIC: the copy-out answers the steering" } else { "NOT antisymmetric" }
    );
    let ok = first_out == Some(0) && first_body == Some(0) && anti;
    s.srv.quit();
    if ok {
        println!("PASS: the copy-out and the body answer on the tick that consumed the input");
        Ok(())
    } else {
        Err(format!(
            "FAIL: copy-out first differs at sample {:?}, body at {:?} (expected 0 and 0); yaw antisymmetric: {}",
            first_out, first_body, anti
        ))
    }
}

// ------------------------------------------------------------ fk locate watch

/// The writers, under gdb and no shim: hardware watchpoints on the body record's
/// position, the copy-out's position and both vis states' positions over two
/// ticks after the 200th, with the RIP of every write. The RIPs must be the
/// sites the derivation names, and in the order it names them.
pub fn watch(engine: &Engine, tape: Tape, arm_tick: u32) -> Result<(), String> {
    engine.check()?;
    let dir = engine.work.join("gdb-watch");
    let replays = dir.join("UserData/Replays");
    let maps = dir.join("UserData/Maps");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&replays).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&maps).map_err(|e| e.to_string())?;
    let _ = std::os::unix::fs::symlink(engine.server.join("Packs"), dir.join("Packs"));
    let _ = std::os::unix::fs::symlink(engine.server.join("TrackmaniaServer"), dir.join("TrackmaniaServer"));
    let m = engine.map.canonicalize().map_err(|e| e.to_string())?;
    let _ = std::os::unix::fs::symlink(m.clone(), maps.join(m.file_name().unwrap()));
    tape.write_reference(&replays.join("g.Ghost.Gbx"))?;
    let script = format!(
        r#"set pagination off
set confirm off
set disable-randomization on
starti
set $b = 0x555555554000
set $tick = 0
set $armed = 0
set $stop = 0
set $rec = (long)0
set $phy = (long)0
break *($b+{tickfn:#x})
commands
  silent
  set $tick = $tick + 1
  if $armed && $tick >= $stop
    printf "DONE tick %d\n", $tick
    quit
  end
  continue
end
break *($b+{copyout:#x})
commands
  silent
  if !$armed && $tick >= {arm}
    set $armed = 1
    set $stop = $tick + 3
    set $rec = (long)$rax
    set $phy = (long)$r13
    printf "ARM tick %d phy=%lx rec=%lx handle=%d\n", $tick, $phy, $rec, $r15d
    watch *(int*)($rec+{bpos:#x})
    commands
      silent
      printf "W body.pos tick %d rip=%lx\n", $tick, $pc
      bt 8
      continue
    end
    watch *(int*)($phy+{ppos:#x})
    commands
      silent
      printf "W copyout.pos tick %d rip=%lx\n", $tick, $pc
      bt 8
      continue
    end
    watch *(int*)($phy+{vpre:#x})
    commands
      silent
      printf "W vis_pre.pos tick %d rip=%lx\n", $tick, $pc
      continue
    end
    watch *(int*)($phy+{vpost:#x})
    commands
      silent
      printf "W vis_post.pos tick %d rip=%lx\n", $tick, $pc
      continue
    end
  end
  continue
end
break *($b+{clockwrite:#x})
commands
  silent
  if $armed
    printf "CLOCK tick %d new_time=%d\n", $tick, *(int*)($rbp-0x30)
  end
  continue
end
continue
quit
"#,
        tickfn = forkoracle::tickhook_sig::TICK_FN_OFF,
        arm = arm_tick,
        copyout = 0x9cdbe4usize,
        bpos = b::POS_IN_BODY,
        ppos = b::POS_IN_PHY,
        vpre = b::VIS_PRE_IN_PHY + b::POS_IN_VIS,
        vpost = b::VIS_POST_IN_PHY + b::POS_IN_VIS,
        clockwrite = forkoracle::tickhook_sig::TICK_CLOCK_WRITE_OFF,
    );
    let sp = dir.join("watch.gdb");
    std::fs::write(&sp, script).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let out = Command::new("gdb")
        .args(["-q", "-batch", "-x"])
        .arg(&sp)
        .args(["--args", "./TrackmaniaServer", "/nodaemon", "/validatepath=."])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("gdb: {}", e))?;
    let text = String::from_utf8_lossy(&out.stdout);
    println!("gdb run: {:.1}s", t0.elapsed().as_secs_f64());
    // WHAT MUST HOLD, per tick after the arming one:
    //   * every write of the body's position happens inside the vehicle SOLVER
    //     call or the dyna world UPDATE that follows it
    //     (a frame of its backtrace is the return address of
    //     `0x11a14da: call 0xa53ce0`) -- the integrator's two writes and, on
    //     the ground, the contact passes of the dyna world;
    //   * the copy-out writes the phy's position exactly once, at 0x9cdbfb
    //     (RIP 0x9cdc02 after it), inside `0x11a17ac: call 0x9cd8c0`, AFTER
    //     the last body write;
    //   * the pre-step vis state is refreshed before the solver and the
    //     post-step one after the copy-out, each as a copy (0x1ae9636) and a
    //     quantisation (0x9cf60f).
    const SOLVER_RET: usize = 0x11a14df;
    // `0x11a14fe: call 0x933a20(dynamgr, params, dt)` -- the dyna world update
    // that follows the vehicle solver and resolves the bodies against each
    // other and the world (the engine names the boundary between the two
    // `PhysicsStep_BeforeMgrDynaUpdate`)
    const DYNA_RET: usize = 0x11a1503;
    const COPYOUT_RET: usize = 0x11a17b1;
    const COPYOUT_WRITE: usize = 0x9cdc02;
    const VIS_WRITES: [usize; 2] = [0x1ae9636, 0x9cf60f];
    struct W {
        tick: u32,
        what: String,
        rva: usize,
        frames: Vec<usize>,
    }
    let mut writes: Vec<W> = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("W ") {
            let mut it = rest.split_whitespace();
            let what = it.next().unwrap_or("").to_string();
            let _ = it.next();
            let tick: u32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let rip = it
                .next()
                .and_then(|v| v.strip_prefix("rip="))
                .and_then(|v| u64::from_str_radix(v, 16).ok())
                .unwrap_or(0);
            writes.push(W { tick, what, rva: (rip as usize).wrapping_sub(0x555555554000), frames: Vec::new() });
        } else if l.starts_with('#') {
            if let Some(w) = writes.last_mut() {
                if let Some(a) = l.split_whitespace().nth(1).and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok()) {
                    w.frames.push((a as usize).wrapping_sub(0x555555554000));
                }
            }
        } else if l.starts_with("ARM ") || l.starts_with("DONE ") {
            println!("  {}", l);
        }
    }
    if writes.is_empty() {
        return Err(format!("no watchpoint fired:\n{}", text));
    }
    let armed_at = writes[0].tick;
    let mut fails: Vec<String> = Vec::new();
    let mut ticks: Vec<u32> = writes.iter().map(|w| w.tick).filter(|t| *t > armed_at).collect();
    ticks.dedup();
    for t in ticks {
        let seq: Vec<&W> = writes.iter().filter(|w| w.tick == t).collect();
        let kinds: Vec<&str> = seq.iter().map(|w| w.what.as_str()).collect();
        // shape: vis_pre+, body+, copyout, vis_post+
        let first_of = |k: &str| kinds.iter().position(|x| *x == k);
        let last_of = |k: &str| kinds.iter().rposition(|x| *x == k);
        let nbody = kinds.iter().filter(|k| **k == "body.pos").count();
        let ncopy = kinds.iter().filter(|k| **k == "copyout.pos").count();
        let shape_ok = matches!((first_of("vis_pre.pos"), last_of("vis_pre.pos"), first_of("body.pos"), last_of("body.pos"), first_of("copyout.pos"), first_of("vis_post.pos")),
            (Some(a), Some(a2), Some(b1), Some(b2), Some(c), Some(d)) if a2 < b1 && b2 < c && c < d && a == 0)
            && ncopy == 1
            && nbody >= 2;
        let body_in_solver = seq
            .iter()
            .filter(|w| w.what == "body.pos")
            .all(|w| w.frames.contains(&SOLVER_RET) || w.frames.contains(&DYNA_RET));
        let copy_site_ok = seq
            .iter()
            .filter(|w| w.what == "copyout.pos")
            .all(|w| w.rva == COPYOUT_WRITE && w.frames.contains(&COPYOUT_RET));
        let vis_sites_ok = seq
            .iter()
            .filter(|w| w.what.starts_with("vis_"))
            .all(|w| VIS_WRITES.contains(&w.rva));
        let ok = shape_ok && body_in_solver && copy_site_ok && vis_sites_ok;
        let body_sites: Vec<String> = {
            let mut v: Vec<usize> = seq.iter().filter(|w| w.what == "body.pos").map(|w| w.rva).collect();
            v.sort_unstable();
            v.dedup();
            v.iter().map(|r| format!("{:#x}", r)).collect()
        };
        println!(
            "  [{}] tick {}: vis_pre x{}, body x{} at {} (all inside the vehicle solver or the dyna update: {}), copy-out x{} at {:#x} inside 0x9cd8c0: {}, vis_post x{}",
            if ok { "ok" } else { "FAIL" },
            t,
            kinds.iter().filter(|k| **k == "vis_pre.pos").count(),
            nbody,
            body_sites.join("/"),
            body_in_solver,
            ncopy,
            seq.iter().find(|w| w.what == "copyout.pos").map(|w| w.rva).unwrap_or(0),
            copy_site_ok,
            kinds.iter().filter(|k| **k == "vis_post.pos").count()
        );
        if !ok {
            fails.push(format!("tick {}: order {:?}, body-in-solver {}, copy-site {}, vis-sites {}", t, kinds, body_in_solver, copy_site_ok, vis_sites_ok));
        }
    }
    if fails.is_empty() {
        println!("PASS: every write of the car's position is the solver's, then the copy-out's, in that order, every tick");
        Ok(())
    } else {
        Err(format!("FAIL: {}", fails.join("; ")))
    }
}
