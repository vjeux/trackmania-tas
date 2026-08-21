//! `tmtraj whl ...` -- the SURFACE AND CONTACT instrument (arm `whl`,
//! 2026-08-20).
//!
//! WHY THIS EXISTS
//! ---------------
//! The published ghosts carry regenerated position/orientation/speed and the
//! CARRIER's wheel and surface bytes. On screen that is a car kicking up dirt
//! in mid-air. The defect is visible; the fix must be checked by something
//! stronger than "the effect stopped", because **a zeroed field also stops
//! it**.
//!
//! THE TEST THAT CANNOT PASS ON A ZEROED FIELD
//! -------------------------------------------
//! Airborne is decidable from the TRAJECTORY ALONE, with no contact flag
//! involved: a car with no contact force on it accelerates at exactly
//! `(0, -g, 0)` with `g = 22.3 m/s^2` in this engine. So classify every
//! interior sample from the second difference of its own position:
//!
//!   BALLISTIC  |a - (0,-g,0)| < tol            -- nothing is touching the car
//!   SUPPORTED  a_y > -g + margin, sustained    -- something is holding it up
//!   UNKNOWN    everything else (collisions, reactors, sample-rate artefacts)
//!
//! and then ask TWO questions of the contact flag, not one:
//!
//!   A. is it OFF on every BALLISTIC sample?   (a zeroed field passes this)
//!   B. is it ON  on every SUPPORTED sample?   (a zeroed field FAILS this)
//!
//! A gate that only asks A is the gate the brief warns about. Both are
//! reported, always, and the verdict needs both.
//!
//! The `-g` calibration is checked, not assumed: `whl grav` fits g on the
//! longest ballistic stretch of a file and prints it, so a wrong constant
//! shows up as a wrong fit rather than as a silently empty BALLISTIC class.

use crate::entrec::{find_entrecord_blob, load_body, parse_record_data, read_transform_pub};

pub const G_DEFAULT: f64 = 25.2;

/// One decoded sample, with the raw bytes kept: the surface fields are single
/// bytes whose semantics are partly a guess, so every consumer here reads the
/// BYTE, not a derived float.
pub struct R {
    pub ms: i64,
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub speed: f64,
    pub raw: Vec<u8>,
}

impl R {
    pub fn b(&self, i: usize) -> u8 {
        *self.raw.get(i).unwrap_or(&0)
    }
    pub fn contact(&self) -> bool {
        (self.b(89) & 0x1) != 0
    }
    pub fn dirt_max(&self) -> u8 {
        [93usize, 95, 97, 99].iter().map(|o| self.b(*o)).max().unwrap_or(0)
    }
    pub fn ice_max(&self) -> u8 {
        [81usize, 82, 83, 84].iter().map(|o| self.b(*o)).max().unwrap_or(0)
    }
    pub fn finite(&self) -> bool {
        self.pos.iter().all(|v| v.is_finite()) && self.vel.iter().all(|v| v.is_finite())
    }
}

pub fn decode(path: &str) -> Result<Vec<R>, String> {
    let body = load_body(path)?;
    let (ver, blob) = find_entrecord_blob(&body)?;
    let rd = parse_record_data(&blob, ver)?;
    let ent = rd
        .ents
        .iter()
        .filter(|e| e.sample_size >= 100 && !e.times.is_empty())
        .max_by_key(|e| e.times.len())
        .ok_or("no CSceneVehicleVis entity")?;
    let ss = ent.sample_size;
    let mut out = Vec::with_capacity(ent.times.len());
    for (i, t) in ent.times.iter().enumerate() {
        let raw = ent.raw[i * ss..(i + 1) * ss].to_vec();
        let (pos, _q, speed, vel) = read_transform_pub(&raw, 47);
        out.push(R { ms: *t as i64, pos, vel, speed, raw });
    }
    out.sort_by_key(|r| r.ms);
    Ok(out)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cls {
    Ballistic,
    Supported,
    Unknown,
}

pub struct Classified {
    pub cls: Vec<Cls>,
    /// |a - (0,-g,0)| per sample, NaN where undecidable
    pub aerr: Vec<f64>,
    pub ay: Vec<f64>,
}

/// Classify from the second difference of position alone.
///
/// `tol` is the metres/s^2 ball around free fall that counts as BALLISTIC;
/// `margin` is how far above -g the vertical acceleration must sit before a
/// sample counts as SUPPORTED. Between the two is UNKNOWN and is never
/// asserted on.
///
/// Both classes additionally require `run` consecutive samples of the same
/// kind, so a single collision spike or a sample-rate artefact cannot create a
/// one-sample class of its own. That is what makes a violation meaningful.
pub fn classify(r: &[R], g: f64, tol: f64, margin: f64, run: usize) -> Classified {
    let n = r.len();
    let mut cls = vec![Cls::Unknown; n];
    let mut aerr = vec![f64::NAN; n];
    let mut ay = vec![f64::NAN; n];
    // raw per-sample decision first
    let mut raw = vec![Cls::Unknown; n];
    for i in 1..n.saturating_sub(1) {
        let dt0 = (r[i].ms - r[i - 1].ms) as f64 / 1000.0;
        let dt1 = (r[i + 1].ms - r[i].ms) as f64 / 1000.0;
        if dt0 <= 0.0 || dt1 <= 0.0 || (dt0 - dt1).abs() > 1e-6 {
            continue;
        }
        if !r[i - 1].finite() || !r[i].finite() || !r[i + 1].finite() {
            continue;
        }
        let mut a = [0f64; 3];
        for k in 0..3 {
            a[k] = (r[i + 1].pos[k] - 2.0 * r[i].pos[k] + r[i - 1].pos[k]) / (dt0 * dt1);
        }
        ay[i] = a[1];
        let e = (a[0] * a[0] + (a[1] + g) * (a[1] + g) + a[2] * a[2]).sqrt();
        aerr[i] = e;
        // SUPPORTED is deliberately narrower than "not falling at g".
        // Measured on 276874: from 4.9 s to 11.3 s the car climbs 20 m and
        // comes back down with a_y between -5 and +13 -- REACTOR FLIGHT, no
        // contact anywhere in it, and the user's own description of the defect
        // says so. A rule that only asks "is a_y above -g" calls that supported
        // and then condemns a CORRECT contact flag for reading zero there.
        // Ground support means the car is being held at a roughly constant
        // height: small vertical acceleration AND small vertical speed.
        let vy = (r[i + 1].pos[1] - r[i - 1].pos[1]) / (dt0 + dt1);
        let sh = {
            let dx = (r[i + 1].pos[0] - r[i - 1].pos[0]) / (dt0 + dt1);
            let dz = (r[i + 1].pos[2] - r[i - 1].pos[2]) / (dt0 + dt1);
            (dx * dx + dz * dz).sqrt()
        };
        if e < tol {
            raw[i] = Cls::Ballistic;
        } else if a[1].abs() < margin && vy.abs() < 2.0 && sh > 3.0 {
            raw[i] = Cls::Supported;
        }
    }
    // require a run of `run` consecutive samples of the same kind
    let mut i = 0usize;
    while i < n {
        let k = raw[i];
        let mut j = i;
        while j < n && raw[j] == k {
            j += 1;
        }
        if k != Cls::Unknown && j - i >= run {
            for x in i..j {
                cls[x] = k;
            }
        }
        i = j;
    }
    Classified { cls, aerr, ay }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
fn fnum(args: &[String], name: &str, d: f64) -> f64 {
    flag(args, name).and_then(|v| v.parse().ok()).unwrap_or(d)
}
fn inum(args: &[String], name: &str, d: i64) -> i64 {
    flag(args, name).and_then(|v| v.parse().ok()).unwrap_or(d)
}

const USAGE: &str = "\
tmtraj whl dump  GHOST [--csv OUT] [--race MS]      every surface/contact byte, per sample
tmtraj whl grav  GHOST [--race MS]                  fit g on the longest ballistic stretch
tmtraj whl air   GHOST [--race MS] [--g G] [--tol T] [--margin M] [--run N]
                                                    classify, and cross-tab against the flags
tmtraj whl gate  GHOST [--race MS] ...              the ACCEPTANCE gate: both directions
tmtraj whl cmp   A B [--race MS]                    per-sample surface-byte agreement
tmtraj whl calib GHOST...                           a_y grouped by the RECORDED contact flag
";

pub fn cmd(args: &[String]) {
    if args.is_empty() {
        print!("{}", USAGE);
        std::process::exit(2);
    }
    let sub = args[0].as_str();
    let rest: Vec<String> = args[1..].to_vec();
    match sub {
        "dump" => dump(&rest),
        "grav" => grav(&rest),
        "air" => air(&rest, false),
        "gate" => air(&rest, true),
        "cmp" => cmp(&rest),
        "roll" => roll(&rest),
        "twoway" => twoway(&rest),
        "calib" => calib(&rest),
        "surv" => surv(&rest),
        _ => {
            print!("{}", USAGE);
            std::process::exit(2);
        }
    }
}

fn load(args: &[String], idx: usize) -> (Vec<R>, i64) {
    let p = args.get(idx).cloned().unwrap_or_default();
    let r = match decode(&p) {
        Ok(v) => v,
        Err(e) => {
            println!("ABORT: {}: {}", p, e);
            std::process::exit(3)
        }
    };
    let race = inum(args, "--race", i64::MAX);
    (r, race)
}

fn dump(args: &[String]) {
    let (r, race) = load(args, 0);
    let out = flag(args, "--csv");
    let mut s = String::from(
        "time_ms,x,y,z,speed_ms,contact,b89,b76,ice81,ice82,ice83,ice84,\
dirt93,dirt95,dirt97,dirt99,wet101,rpm5,gear91,boost90,turbo21,\
w6,w7,w8,w9,w10,w11,w12,w13,d23,d25,d27,d29\n",
    );
    for x in r.iter().filter(|x| x.ms <= race) {
        s.push_str(&format!(
            "{},{:.4},{:.4},{:.4},{:.4},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            x.ms, x.pos[0], x.pos[1], x.pos[2], x.speed,
            u8::from(x.contact()), x.b(89), x.b(76),
            x.b(81), x.b(82), x.b(83), x.b(84),
            x.b(93), x.b(95), x.b(97), x.b(99),
            x.b(101), x.b(5), x.b(91), x.b(90), x.b(21),
            x.b(6), x.b(7), x.b(8), x.b(9), x.b(10), x.b(11), x.b(12), x.b(13),
            x.b(23), x.b(25), x.b(27), x.b(29)
        ));
    }
    match out {
        Some(f) => {
            std::fs::write(&f, s).unwrap();
            println!("wrote {}", f);
        }
        None => print!("{}", s),
    }
}

/// Fit g on the longest stretch where the horizontal acceleration is ~0.
fn grav(args: &[String]) {
    let (r, race) = load(args, 0);
    let v: Vec<&R> = r.iter().filter(|x| x.ms <= race && x.finite()).collect();
    let n = v.len();
    if n < 5 {
        println!("ABORT: too few samples");
        return;
    }
    // a sample is a free-fall candidate when the HORIZONTAL acceleration is
    // near zero -- that test does not mention g, so fitting g on it is not
    // circular.
    let mut cand = vec![false; n];
    let mut ay = vec![f64::NAN; n];
    for i in 1..n - 1 {
        let dt = (v[i].ms - v[i - 1].ms) as f64 / 1000.0;
        if dt <= 0.0 || ((v[i + 1].ms - v[i].ms) as f64 / 1000.0 - dt).abs() > 1e-6 {
            continue;
        }
        let ax = (v[i + 1].pos[0] - 2.0 * v[i].pos[0] + v[i - 1].pos[0]) / (dt * dt);
        let az = (v[i + 1].pos[2] - 2.0 * v[i].pos[2] + v[i - 1].pos[2]) / (dt * dt);
        ay[i] = (v[i + 1].pos[1] - 2.0 * v[i].pos[1] + v[i - 1].pos[1]) / (dt * dt);
        cand[i] = (ax * ax + az * az).sqrt() < 1.0 && ay[i] < -5.0;
    }
    let (mut bi, mut bl, mut i) = (0usize, 0usize, 0usize);
    while i < n {
        if cand[i] {
            let mut j = i;
            while j < n && cand[j] {
                j += 1;
            }
            if j - i > bl {
                bl = j - i;
                bi = i;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    if bl == 0 {
        println!("no free-fall stretch (no sample with horizontal |a| < 1.0 and a_y < -5)");
        return;
    }
    let mut s: Vec<f64> = (bi..bi + bl).map(|k| ay[k]).collect();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = s[s.len() / 2];
    let mean = s.iter().sum::<f64>() / s.len() as f64;
    println!(
        "longest free-fall stretch: {} samples, {:.3} .. {:.3} s",
        bl,
        v[bi].ms as f64 / 1000.0,
        v[bi + bl - 1].ms as f64 / 1000.0
    );
    println!("  a_y median {:.4} m/s^2   mean {:.4}   min {:.4}   max {:.4}", med, mean, s[0], s[s.len() - 1]);
    println!("  => g = {:.4}", -med);
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}

fn air(args: &[String], gate: bool) {
    let (r, race) = load(args, 0);
    let g = fnum(args, "--g", G_DEFAULT);
    let tol = fnum(args, "--tol", 2.0);
    let margin = fnum(args, "--margin", 5.0);
    let run = inum(args, "--run", 3) as usize;
    let v: Vec<R> = r.into_iter().filter(|x| x.ms <= race).collect();
    let c = classify(&v, g, tol, margin, run);
    let n = v.len();
    let nb = c.cls.iter().filter(|x| **x == Cls::Ballistic).count();
    let ns = c.cls.iter().filter(|x| **x == Cls::Supported).count();
    let nu = n - nb - ns;
    let dt = if n > 1 { (v[1].ms - v[0].ms) as f64 / 1000.0 } else { 0.05 };
    // longest ballistic run
    let mut longest = 0usize;
    let mut i = 0usize;
    while i < n {
        if c.cls[i] == Cls::Ballistic {
            let mut j = i;
            while j < n && c.cls[j] == Cls::Ballistic {
                j += 1;
            }
            longest = longest.max(j - i);
            i = j;
        } else {
            i += 1;
        }
    }
    println!(
        "samples {} ({:.3} .. {:.3} s, dt {:.3})   g={:.2} tol={:.2} margin={:.2} run={}",
        n,
        v.first().map(|x| x.ms as f64 / 1000.0).unwrap_or(0.0),
        v.last().map(|x| x.ms as f64 / 1000.0).unwrap_or(0.0),
        dt, g, tol, margin, run
    );
    println!(
        "BALLISTIC {:4} ({:5.1} %, {:.3} s, longest run {:.3} s)   SUPPORTED {:4} ({:5.1} %)   UNKNOWN {:4} ({:5.1} %)",
        nb, pct(nb, n), nb as f64 * dt, longest as f64 * dt, ns, pct(ns, n), nu, pct(nu, n)
    );

    // ---- the two directions, reported separately, always -------------------
    let ball: Vec<usize> = (0..n).filter(|i| c.cls[*i] == Cls::Ballistic).collect();
    let sup: Vec<usize> = (0..n).filter(|i| c.cls[*i] == Cls::Supported).collect();
    let con_b = ball.iter().filter(|i| v[**i].contact()).count();
    let con_s = sup.iter().filter(|i| v[**i].contact()).count();
    let dirt_b = ball.iter().filter(|i| v[**i].dirt_max() > 0).count();
    let dirt_s = sup.iter().filter(|i| v[**i].dirt_max() > 0).count();
    let ice_b = ball.iter().filter(|i| v[**i].ice_max() > 0).count();
    let ice_s = sup.iter().filter(|i| v[**i].ice_max() > 0).count();
    println!("                              on BALLISTIC        on SUPPORTED");
    println!(
        "  ground contact (b89&1)      {:4}/{:<4} {:5.1} %   {:4}/{:<4} {:5.1} %",
        con_b, ball.len(), pct(con_b, ball.len()), con_s, sup.len(), pct(con_s, sup.len())
    );
    println!(
        "  dirt  > 0 (b93/95/97/99)    {:4}/{:<4} {:5.1} %   {:4}/{:<4} {:5.1} %",
        dirt_b, ball.len(), pct(dirt_b, ball.len()), dirt_s, sup.len(), pct(dirt_s, sup.len())
    );
    println!(
        "  ice   > 0 (b81..84)         {:4}/{:<4} {:5.1} %   {:4}/{:<4} {:5.1} %",
        ice_b, ball.len(), pct(ice_b, ball.len()), ice_s, sup.len(), pct(ice_s, sup.len())
    );
    let allc = v.iter().filter(|x| x.contact()).count();
    let alld = v.iter().filter(|x| x.dirt_max() > 0).count();
    let alli = v.iter().filter(|x| x.ice_max() > 0).count();
    println!(
        "  whole file: contact {}/{} ({:.1} %)  dirt>0 {} ({:.1} %)  ice>0 {} ({:.1} %)  max dirt {}  max ice {}",
        allc, n, pct(allc, n), alld, pct(alld, n), alli, pct(alli, n),
        v.iter().map(|x| x.dirt_max()).max().unwrap_or(0),
        v.iter().map(|x| x.ice_max()).max().unwrap_or(0)
    );

    if args.iter().any(|a| a == "--tl") {
        println!("\n  t(s)      y      a_y   class      contact dirt ice");
        for i in 0..n {
            println!(
                "{:>7.3} {:>7.2} {:>8.2}   {:<10} {:^7} {:>4} {:>3}",
                v[i].ms as f64 / 1000.0, v[i].pos[1], c.ay[i],
                match c.cls[i] { Cls::Ballistic => "BALLISTIC", Cls::Supported => "supported", _ => "-" },
                u8::from(v[i].contact()), v[i].dirt_max(), v[i].ice_max()
            );
        }
    }
    if gate {
        // A: OFF in the air. B: ON on the ground. A zeroed field passes A and
        // fails B; the carrier's field fails A. BOTH must hold.
        //
        // C and D are NOT "zero in the air". Material already stuck to a wheel
        // stays stuck: on 267460's own downloaded recording the ice bytes read
        // 245-253 through an airborne stretch, decaying. A gate that demanded
        // zero there would FAIL A GENUINE HUMAN RECORDING, which is the
        // must-say-yes test every gate here has to survive. What contact-free
        // flight forbids is ACCUMULATION: the value may hold or decay, never
        // rise.
        let rise = |o: &dyn Fn(&R) -> u8| -> usize {
            ball.iter()
                .filter(|i| **i > 0 && c.cls[**i - 1] == Cls::Ballistic && o(&v[**i]) > o(&v[**i - 1]))
                .count()
        };
        let dirt_rise = rise(&|r: &R| r.dirt_max());
        let ice_rise = rise(&|r: &R| r.ice_max());
        let a_ok = ball.is_empty() || con_b == 0;
        // B has to be able to say "no evidence". On 145875 the car descends for
        // the whole run, so there is not one unambiguously ground-borne sample,
        // and a gate that reports FAIL there is condemning a downloaded human
        // recording for the shape of its map.
        let b_n = sup.len();
        let b_ok = b_n >= 5 && pct(con_s, b_n) >= 95.0;
        let b_na = b_n < 5;
        let d_ok = dirt_rise == 0;
        let i_ok = ice_rise == 0;
        println!();
        println!("{} A  contact OFF on every BALLISTIC sample ({} violations)", if a_ok { "PASS" } else { "FAIL" }, con_b);
        println!(
            "{} B  contact ON on >=95 % of SUPPORTED samples ({:.1} % of {}) -- THE TEST A ZEROED FIELD FAILS",
            if b_na { "N/A " } else if b_ok { "PASS" } else { "FAIL" },
            pct(con_s, b_n), b_n
        );
        println!("{} C  dirt never RISES during ballistic flight ({} violations)", if d_ok { "PASS" } else { "FAIL" }, dirt_rise);
        println!("{} D  ice  never RISES during ballistic flight ({} violations)", if i_ok { "PASS" } else { "FAIL" }, ice_rise);
        if !(a_ok && (b_ok || b_na) && d_ok && i_ok) {
            std::process::exit(1);
        }
    }
}

/// Per-sample agreement of the surface/contact bytes between two files, matched
/// on sample time. This is how a regenerated file is graded against a
/// downloaded recording's own bytes.
fn cmp(args: &[String]) {
    let a = match decode(&args[0]) {
        Ok(v) => v,
        Err(e) => {
            println!("ABORT: {}", e);
            std::process::exit(3)
        }
    };
    let b = match decode(&args[1]) {
        Ok(v) => v,
        Err(e) => {
            println!("ABORT: {}", e);
            std::process::exit(3)
        }
    };
    let race = inum(args, "--race", i64::MAX);
    let bm: std::collections::HashMap<i64, &R> = b.iter().map(|x| (x.ms, x)).collect();
    let cols: Vec<(&str, usize)> = vec![
        ("b89 ground_mode", 89), ("b76 vehicle_state", 76),
        ("b81 fl_ice", 81), ("b82 fr_ice", 82), ("b83 rr_ice", 83), ("b84 rl_ice", 84),
        ("b93 fl_dirt", 93), ("b95 fr_dirt", 95), ("b97 rr_dirt", 97), ("b99 rl_dirt", 99),
        ("b101 wetness", 101), ("b5 rpm", 5), ("b91 gear", 91), ("b90 booster", 90),
        ("b21 turbo", 21), ("b102 simcoef", 102),
        ("b6 fl_frac", 6), ("b7 fl_turn", 7), ("b8 fr_frac", 8), ("b9 fr_turn", 9),
        ("b10 rr_frac", 10), ("b11 rr_turn", 11), ("b12 rl_frac", 12), ("b13 rl_turn", 13),
        ("b23 fl_damp", 23), ("b25 fr_damp", 25), ("b27 rr_damp", 27), ("b29 rl_damp", 29),
    ];
    let mut m = 0usize;
    let mut exact = vec![0usize; cols.len()];
    let mut within1 = vec![0usize; cols.len()];
    let mut maxd = vec![0i64; cols.len()];
    let mut contact_agree = 0usize;
    for x in a.iter().filter(|x| x.ms <= race) {
        let Some(y) = bm.get(&x.ms) else { continue };
        m += 1;
        if x.contact() == y.contact() {
            contact_agree += 1;
        }
        for (k, (_, o)) in cols.iter().enumerate() {
            let d = (x.b(*o) as i64 - y.b(*o) as i64).abs();
            if d == 0 {
                exact[k] += 1;
            }
            if d <= 1 {
                within1[k] += 1;
            }
            maxd[k] = maxd[k].max(d);
        }
    }
    println!("matched {} samples", m);
    println!("{:<20} {:>8} {:>10} {:>8}", "byte", "exact %", "within1 %", "maxdiff");
    for (k, (nm, _)) in cols.iter().enumerate() {
        println!("{:<20} {:>8.2} {:>10.2} {:>8}", nm, pct(exact[k], m), pct(within1[k], m), maxd[k]);
    }
    println!("contact bit agreement: {:.2} %", pct(contact_agree, m));
}

/// The calibration that is NOT circular, run on GENUINE RECORDINGS ONLY.
///
/// A downloaded human ghost carries the game's own contact flag. Group its
/// interior samples by that flag and print the vertical acceleration measured
/// from its own positions. If bit 0 of byte 89 really is ground contact then
/// the flag-OFF group must pile up at a single value -- and that value is `g`.
/// Two facts fall out of one measurement, and neither is assumed:
///   * what `g` is in this engine, and
///   * that byte 89 bit 0 means what the decoder says it means.
fn calib(args: &[String]) {
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    println!(
        "{:<44} {:>6} {:>7} {:>9} {:>9} {:>9} {:>7} {:>9} {:>9}",
        "recording", "n", "off", "ay_med", "ay_p10", "ay_p90", "on", "ay_med", "|ah|med"
    );
    let mut all_off: Vec<f64> = Vec::new();
    for f in files {
        let r = match decode(f) {
            Ok(v) => v,
            Err(e) => {
                println!("{:<44} ABORT {}", f, e);
                continue;
            }
        };
        let n = r.len();
        let (mut off, mut on, mut ahon) = (Vec::new(), Vec::new(), Vec::new());
        for i in 1..n.saturating_sub(1) {
            let dt = (r[i].ms - r[i - 1].ms) as f64 / 1000.0;
            if dt <= 0.0 || ((r[i + 1].ms - r[i].ms) as f64 / 1000.0 - dt).abs() > 1e-6 {
                continue;
            }
            if !r[i - 1].finite() || !r[i].finite() || !r[i + 1].finite() {
                continue;
            }
            let a: Vec<f64> = (0..3)
                .map(|k| (r[i + 1].pos[k] - 2.0 * r[i].pos[k] + r[i - 1].pos[k]) / (dt * dt))
                .collect();
            let ah = (a[0] * a[0] + a[2] * a[2]).sqrt();
            if r[i].contact() {
                on.push(a[1]);
                ahon.push(ah);
            } else {
                // only samples whose HORIZONTAL acceleration is tiny: a car
                // clipping a wall in mid-air is airborne and not in free fall.
                if ah < 1.0 {
                    off.push(a[1]);
                    all_off.push(a[1]);
                }
            }
        }
        let q = |v: &mut Vec<f64>, p: f64| -> f64 {
            if v.is_empty() {
                return f64::NAN;
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[((v.len() - 1) as f64 * p) as usize]
        };
        let mut o2 = off.clone();
        let mut o3 = off.clone();
        let mut o4 = off.clone();
        let mut n2 = on.clone();
        let mut h2 = ahon.clone();
        println!(
            "{:<44} {:>6} {:>7} {:>9.3} {:>9.3} {:>9.3} {:>7} {:>9.3} {:>9.3}",
            f.rsplit('/').next().unwrap_or(f),
            n,
            off.len(),
            q(&mut o2, 0.5),
            q(&mut o3, 0.1),
            q(&mut o4, 0.9),
            on.len(),
            q(&mut n2, 0.5),
            q(&mut h2, 0.5)
        );
    }
    if !all_off.is_empty() {
        all_off.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let m = all_off[all_off.len() / 2];
        let p10 = all_off[all_off.len() / 10];
        let p90 = all_off[all_off.len() * 9 / 10];
        println!(
            "\nPOOLED contact-OFF, horizontal |a| < 1: {} samples   a_y median {:.4}  p10 {:.4}  p90 {:.4}   => g = {:.4}",
            all_off.len(), m, p10, p90, -m
        );
    }
}

/// Which files carry a VARYING surface field? A field that is constant on a
/// recording is invisible to a correlation sweep -- the answer key has no
/// column there -- so the choice of control map is not free.
fn surv(args: &[String]) {
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    println!(
        "{:<52} {:>5} {:>10} {:>10} {:>10} {:>10} {:>9} {:>9}",
        "file", "n", "dirt", "ice", "wet101", "b89", "b76", "b90"
    );
    for f in files {
        let r = match decode(f) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let rng = |os: &[usize]| -> String {
            let mut lo = 255u8;
            let mut hi = 0u8;
            let mut d = std::collections::BTreeSet::new();
            for x in &r {
                for o in os {
                    let b = x.b(*o);
                    lo = lo.min(b);
                    hi = hi.max(b);
                    d.insert(b);
                }
            }
            format!("{}-{}/{}", lo, hi, d.len())
        };
        println!(
            "{:<52} {:>5} {:>10} {:>10} {:>10} {:>10} {:>9} {:>9}",
            f.rsplit('/').next().unwrap_or(f),
            r.len(),
            rng(&[93, 95, 97, 99]),
            rng(&[81, 82, 83, 84]),
            rng(&[101]),
            rng(&[89]),
            rng(&[76]),
            rng(&[90])
        );
    }
}

/// `tmtraj whl roll` -- the wheel-to-car rate, per class.
///
/// A rolling wheel covers |v| dt per tick, so `turns * 2 pi * r / (|v| dt)` is
/// 1.0. This prints it split by the trajectory-derived class, because the
/// airborne answer is NOT the ground answer: a free wheel does whatever the
/// engine does with it, and what that is has to be read off real recordings
/// rather than assumed. The radius is fitted on the SUPPORTED samples of the
/// same file, so the ground ratio is 1.0 by construction and the number that
/// carries information is the AIRBORNE one and the SPREAD.
fn roll(args: &[String]) {
    let (r, race) = load(args, 0);
    let g = fnum(args, "--g", G_DEFAULT);
    let tol = fnum(args, "--tol", 2.0);
    let margin = fnum(args, "--margin", 5.0);
    let run = inum(args, "--run", 3) as usize;
    let v: Vec<R> = r.into_iter().filter(|x| x.ms <= race).collect();
    let c = classify(&v, g, tol, margin, run);
    let n = v.len();
    // turns of wheel 0, unwrapped across the byte pair's own 256-turn range
    let turns = |x: &R| -> f64 { x.b(7) as f64 + x.b(6) as f64 / 255.0 };
    let mut rows: Vec<(Cls, f64, f64)> = Vec::new(); // class, d(turns), distance
    for i in 1..n {
        let dt = (v[i].ms - v[i - 1].ms) as f64 / 1000.0;
        if dt <= 0.0 || dt > 0.2 {
            continue;
        }
        let mut dturn = turns(&v[i]) - turns(&v[i - 1]);
        while dturn < -128.0 {
            dturn += 256.0;
        }
        while dturn > 128.0 {
            dturn -= 256.0;
        }
        let mut d = 0.0;
        for k in 0..3 {
            let q = v[i].pos[k] - v[i - 1].pos[k];
            d += q * q;
        }
        rows.push((c.cls[i], dturn, d.sqrt()));
    }
    // radius from the SUPPORTED rows only
    let mut rr: Vec<f64> = rows
        .iter()
        .filter(|(k, dt, d)| *k == Cls::Supported && *dt > 1e-4 && *d > 0.05)
        .map(|(_, dt, d)| d / (dt * std::f64::consts::TAU))
        .collect();
    rr.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rad = if rr.is_empty() { f64::NAN } else { rr[rr.len() / 2] };
    println!("radius fitted on {} supported steps: {:.4} m", rr.len(), rad);
    for k in [Cls::Supported, Cls::Ballistic, Cls::Unknown] {
        let mut q: Vec<f64> = rows
            .iter()
            .filter(|(kk, _, d)| *kk == k && *d > 0.05)
            .map(|(_, dt, d)| dt * std::f64::consts::TAU * rad / d)
            .filter(|x| x.is_finite())
            .collect();
        if q.is_empty() {
            println!("{:<10} no steps", format!("{:?}", k));
            continue;
        }
        q.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{:<10} n {:>5}   wheel/car rate  p10 {:>7.3}  median {:>7.3}  p90 {:>7.3}",
            format!("{:?}", k), q.len(),
            q[q.len() / 10], q[q.len() / 2], q[q.len() * 9 / 10]
        );
    }
}

/// `tmtraj whl twoway` -- the OTHER arm's classification, implemented exactly
/// as described, and pointed at recordings whose flag is not in dispute.
///
/// Their rule: central-difference a_y; a sample is AIRBORNE when a_y is near
/// -g and GROUNDED otherwise; then count "contact ON while airborne" and
/// "contact OFF while grounded". It is a two-class partition of every sample.
///
/// Mine has THREE classes and asserts only on the outer two, because a car
/// held up by anything other than the ground -- a reactor, a boost, a wall it
/// is scraping -- is neither in free fall nor ground-borne, and a two-class
/// rule must call it one of them.
///
/// Which is right is not a matter of taste: run BOTH against a downloaded
/// recording, whose contact flag the game itself wrote. Whichever rule
/// disagrees with a real recording is the rule that is wrong.
fn twoway(args: &[String]) {
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let g = fnum(args, "--g", G_DEFAULT);
    let tol = fnum(args, "--tol", 2.0);
    println!(
        "{:<44} {:>6} {:>8} {:>8} {:>9} {:>9} {:>8}",
        "file", "n", "air", "ground", "ON@air", "OFF@grnd", "wrong%"
    );
    for f in files {
        let r = match decode(f) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let n = r.len();
        let (mut air, mut grd, mut on_air, mut off_grd) = (0usize, 0usize, 0usize, 0usize);
        for i in 1..n.saturating_sub(1) {
            let dt = (r[i].ms - r[i - 1].ms) as f64 / 1000.0;
            if dt <= 0.0 || ((r[i + 1].ms - r[i].ms) as f64 / 1000.0 - dt).abs() > 1e-6 {
                continue;
            }
            if !r[i - 1].finite() || !r[i].finite() || !r[i + 1].finite() {
                continue;
            }
            let ay = (r[i + 1].pos[1] - 2.0 * r[i].pos[1] + r[i - 1].pos[1]) / (dt * dt);
            if (ay + g).abs() < tol {
                air += 1;
                if r[i].contact() {
                    on_air += 1;
                }
            } else {
                grd += 1;
                if !r[i].contact() {
                    off_grd += 1;
                }
            }
        }
        println!(
            "{:<44} {:>6} {:>8} {:>8} {:>9} {:>9} {:>7.1}%",
            f.rsplit('/').next().unwrap_or(f),
            n, air, grd, on_air, off_grd,
            100.0 * (on_air + off_grd) as f64 / n.max(1) as f64
        );
    }
}
