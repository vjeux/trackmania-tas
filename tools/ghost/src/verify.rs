//! `ghost verify` -- the acceptance gate.
//!
//! Nothing here reads a number off the command line and compares it to itself:
//! every check reads both of its operands out of the FILE or out of the world.
//! A check that cannot get its second operand says NA and says why; it never
//! passes by default.

use gbx::container::{secs, Container};
use crate::ident::{self, Role};
use crate::oracle::{self, MapsMode};
use gbx::tape::Tape;
use crate::cli::{die, flag, has, num};

#[derive(PartialEq, Clone, Copy)]
pub enum Verdict {
    Pass,
    Fail,
    Na,
    Warn,
}

pub struct Check {
    pub id: &'static str,
    pub verdict: Verdict,
    pub msg: String,
}

pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    fn add(&mut self, id: &'static str, v: Verdict, m: impl Into<String>) {
        self.checks.push(Check { id, verdict: v, msg: m.into() });
    }
    pub fn failed(&self) -> bool {
        self.checks.iter().any(|c| c.verdict == Verdict::Fail)
    }
    pub fn print(&self) {
        for c in &self.checks {
            let tag = match c.verdict {
                Verdict::Pass => "PASS",
                Verdict::Fail => "FAIL",
                Verdict::Na => "  NA",
                Verdict::Warn => "WARN",
            };
            println!("{} {:<4} {}", tag, c.id, c.msg);
        }
    }
}

/// V6: does the RECORDING agree with the TAPE the same file carries?
///
/// One `.Ghost.Gbx` holds the driver's inputs twice: the input chunk at 10 ms,
/// and byte 14 of every 50 ms telemetry sample. A synthesised tape grafted onto
/// a donor carries the DONOR's telemetry, and no positional check can see that
/// -- but these two channels stop agreeing immediately, and comparing them
/// needs no reference file at all.
///
/// The statistic is COHEN'S KAPPA on the exact byte, not a raw hit rate, and
/// the difference matters: a bang-bang keyboard tape sits at three steer values
/// for most of a run, so two UNRELATED runs of it agree on 40 % of samples by
/// chance alone, and a raw rate cannot tell that from a real recording. Kappa
/// divides out that chance agreement -- `(p_obs - p_chance) / (1 - p_chance)`
/// with `p_chance` computed from each channel's own marginals -- so a genuine
/// recording scores near 1 and a foreign one scores near 0 whatever the tape
/// looks like.
///
/// Byte 14 is `floor((steer_i8 + 127) * 255 / 254)`, measured against the whole
/// corpus rather than assumed.
///
/// Returns (kappa, raw hit rate, best lag ms, samples compared).
pub fn tape_record_agreement(path: &str) -> Option<(f64, f64, i64, usize)> {
    let t = Tape::from_file(path).ok()?;
    let d = gbx::record::decode_ghost(path).ok()?;
    if d.samples.is_empty() {
        return None;
    }
    let (ss, raw) = crate::regen::raw_vehicle_samples(path).ok()?;
    let a0 = &t.archives[0];
    let so = a0.start_offset_ms as i64;
    let mut best: Option<(f64, f64, i64, usize)> = None;
    for lag in -10i64..=10 {
        let mut pairs: Vec<(u8, u8)> = Vec::new();
        for (i, s) in d.samples.iter().enumerate() {
            let idx = (s.time_ms as i64 + lag * 10 - so) / 10;
            if idx < 0 || idx >= a0.packets.len() as i64 || (i + 1) * ss > raw.len() {
                continue;
            }
            pairs.push((
                crate::regen::steer_byte(a0.packets[idx as usize].steer_i8()),
                raw[i * ss + 14],
            ));
        }
        let n = pairs.len();
        if n < 20 {
            continue;
        }
        let mut ma = [0u32; 256];
        let mut mb = [0u32; 256];
        let mut hit = 0usize;
        for (x, y) in &pairs {
            ma[*x as usize] += 1;
            mb[*y as usize] += 1;
            if x == y {
                hit += 1;
            }
        }
        let p_obs = hit as f64 / n as f64;
        let p_exp: f64 = (0..256)
            .map(|v| (ma[v] as f64 / n as f64) * (mb[v] as f64 / n as f64))
            .sum();
        let kappa = if (1.0 - p_exp).abs() < 1e-12 { 1.0 } else { (p_obs - p_exp) / (1.0 - p_exp) };
        if best.map_or(true, |b| kappa > b.0) {
            best = Some((kappa, p_obs, lag * 10, n));
        }
    }
    best
}

pub fn run(path: &str, a: &[String]) -> Report {
    let mut r = Report { checks: Vec::new() };
    let c = match Container::load(path) {
        Ok(c) => c,
        Err(e) => {
            r.add("V0", Verdict::Fail, e);
            return r;
        }
    };

    // ---- V1 codec identity ------------------------------------------------
    match Tape::from_file(path) {
        Err(e) => r.add("V1", Verdict::Na, format!("no input tape in this file ({})", e)),
        Ok(t) => match t.verbatim_is_identity() {
            Ok(()) => r.add(
                "V1",
                Verdict::Pass,
                format!(
                    "codec identity: a verbatim re-encode of all {} ticks reproduces the file's own bitstream",
                    t.n()
                ),
            ),
            Err(e) => r.add("V1", Verdict::Fail, format!("codec identity: {}", e)),
        },
    }

    // ---- V2 declared-time census -----------------------------------------
    let dts = c.declared_times();
    let mut vals: Vec<u32> = dts.iter().map(|x| x.1).collect();
    vals.sort();
    vals.dedup();
    if dts.is_empty() {
        r.add("V2", Verdict::Na, "this container declares no race time");
    } else if vals.len() > 1 {
        r.add(
            "V2",
            Verdict::Fail,
            format!(
                "declared-time census: {} copies carrying {} DIFFERENT times ({:?}) -- \
                 the file declares one time and another",
                dts.len(),
                vals.len(),
                vals.iter().map(|v| secs(*v as i64)).collect::<Vec<_>>()
            ),
        );
    } else {
        r.add(
            "V2",
            Verdict::Pass,
            format!(
                "declared-time census: {} copies, all {}",
                dts.len(),
                secs(vals[0] as i64)
            ),
        );
    }

    // ---- V3 container identity -------------------------------------------
    let fields = ident::scan(&c);
    let foreign: Vec<String> = fields
        .iter()
        .filter(|f| matches!(f.role, Role::AccountId | Role::Locator) && !f.s.is_empty())
        .map(|f| format!("{} {:?}", f.role.label(), f.s))
        .collect();
    if foreign.is_empty() {
        r.add("V3", Verdict::Pass, "container identity: no account id and no locator URL");
    } else {
        r.add(
            "V3",
            Verdict::Warn,
            format!(
                "container identity: this file still carries {} -- run `ghost identity set IN OUT --anonymise` before publishing it as ours",
                foreign.join(", ")
            ),
        );
    }

    // ---- V4 which map does this actually run on --------------------------
    match c.embedded_map() {
        Some((_, n)) => {
            let mut m = format!("embedded map: {} B -- the server simulates THIS copy, --map is decoration", n);
            if let Some(b) = c.embedded_map_bytes() {
                if let Some(u) = crate::map_uid_of(&b) {
                    m.push_str(&format!(" (uid {})", u));
                }
            }
            r.add("V4", Verdict::Warn, m);
        }
        None => r.add("V4", Verdict::Pass, "no embedded map: --map is real for this file"),
    }

    // ---- V5 telemetry span -------------------------------------------------
    match gbx::record::decode_ghost(path) {
        Err(e) => r.add("V5", Verdict::Na, format!("no telemetry record ({})", e)),
        Ok(d) => {
            let last = d.samples.last().map(|s| s.time_ms).unwrap_or(0) as i64;
            let decl = vals.first().copied().unwrap_or(0) as i64;
            if decl > 0 && last > decl + 2000 {
                r.add(
                    "V5",
                    Verdict::Fail,
                    format!(
                        "telemetry: the last sample is at {} but the file declares {} -- \
                         the record outlives the run it claims to be",
                        secs(last),
                        secs(decl)
                    ),
                );
            } else if d.samples.is_empty() {
                r.add("V5", Verdict::Fail, "telemetry: the record has no vehicle samples at all");
            } else {
                r.add(
                    "V5",
                    Verdict::Pass,
                    format!(
                        "telemetry: {} samples, {} .. {}, span {} .. {}",
                        d.samples.len(),
                        secs(d.samples[0].time_ms as i64),
                        secs(last),
                        secs(d.start_ms as i64),
                        secs(d.end_ms as i64)
                    ),
                );
            }
        }
    }

    // ---- V6 does the recording agree with the tape? -----------------------
    match tape_record_agreement(path) {
        None => r.add("V6", Verdict::Na, "tape/record agreement: this file has no telemetry to compare"),
        Some((kappa, rate, lag, n)) => {
            let thr: f64 = flag(a, "--agree-thr").and_then(|v| v.parse().ok()).unwrap_or(0.60);
            if kappa >= thr {
                r.add(
                    "V6",
                    Verdict::Pass,
                    format!(
                        "tape/record agreement: kappa {:.3} ({:.1}% of {} samples exact, best lag {} ms)",
                        kappa, 100.0 * rate, n, lag
                    ),
                );
            } else {
                r.add(
                    "V6",
                    Verdict::Fail,
                    format!(
                        "tape/record agreement: kappa {:.3} ({:.1}% of {} samples exact, best of 21 lags at {} ms). \
                         Chance-corrected agreement this low means the recording in this file is NOT this \
                         tape's run -- it is carrying another run. Rebuild it with `ghost regen`, or name \
                         the file so that nobody renders it.",
                        kappa, 100.0 * rate, n, lag
                    ),
                );
            }
        }
    }

    // ---- V7 the plain oracle re-simulating THE WRITTEN FILE ---------------
    let sd = flag(a, "--server").map(|s| s.to_string()).or_else(|| std::env::var("TM_SERVER").ok());
    let server = oracle::server_dir(sd.as_deref());
    if !server.join("TrackmaniaServer").exists() {
        r.add("V7", Verdict::Na, format!("no dedicated server at {} (set TM_SERVER or --server)", server.display()));
    } else if has(a, "--no-oracle") {
        r.add("V7", Verdict::Na, "--no-oracle");
    } else {
        let mode = match flag(a, "--map") {
            Some(m) => MapsMode::One(std::path::Path::new(m)),
            None => MapsMode::Empty,
        };
        match oracle::validate(&server, std::path::Path::new(path), mode, "verify") {
            Err(e) => r.add("V7", Verdict::Na, format!("oracle: {}", e)),
            Ok(res) => {
                let expect_dnf = flag(a, "--expect") == Some("dnf");
                let want = num(a, "--expect-ms").or(vals.first().map(|v| *v as i64));
                match (res.time_ms, expect_dnf) {
                    (None, true) => r.add(
                        "V7",
                        Verdict::Pass,
                        format!("oracle on the written file: DNF at cps {:?}, as expected for a partial run", res.cps),
                    ),
                    (Some(t), true) => r.add(
                        "V7",
                        Verdict::Fail,
                        format!("oracle returned {} but --expect dnf was asked for", secs(t)),
                    ),
                    _ => match (res.time_ms, want) {
                        (Some(t), Some(w)) if t == w => r.add(
                            "V7",
                            Verdict::Pass,
                            format!("oracle re-simulated the written file: {} == the declared time", secs(t)),
                        ),
                        (Some(t), Some(w)) => r.add(
                            "V7",
                            Verdict::Fail,
                            format!(
                                "oracle re-simulated the written file to {} but it declares {}",
                                secs(t),
                                secs(w)
                            ),
                        ),
                        (Some(t), None) => {
                            r.add("V7", Verdict::Pass, format!("oracle re-simulated the written file: {}", secs(t)))
                        }
                        (None, _) => r.add(
                            "V7",
                            Verdict::Fail,
                            format!(
                                "oracle: DNF (cps {:?}) on this file. If this is a deliberately \
                                 partial run -- a trimmed clip, a search tape -- say so with \
                                 `--expect dnf`; a file that declares a finish and does not \
                                 finish is not publishable.",
                                res.cps
                            ),
                        ),
                    },
                }
            }
        }
    }

    // ---- V8 the empty-Maps control ----------------------------------------
    if has(a, "--empty-maps") {
        let server = oracle::server_dir(sd.as_deref());
        match oracle::validate(&server, std::path::Path::new(path), MapsMode::Empty, "empty") {
            Err(e) => r.add("V8", Verdict::Na, format!("empty-Maps control: {}", e)),
            Ok(res) => {
                let carried = c.embedded_map().is_some();
                match (res.time_ms, carried) {
                    (Some(t), true) => r.add(
                        "V8",
                        Verdict::Pass,
                        format!(
                            "empty-Maps control: {} with ZERO maps on disk, uid {:?} -- confirmed, the map \
                             this file runs on comes out of the file",
                            secs(t),
                            res.map_uid
                        ),
                    ),
                    (None, false) => r.add(
                        "V8",
                        Verdict::Pass,
                        "empty-Maps control: no result with zero maps on disk -- confirmed, this file needs a map from disk",
                    ),
                    (Some(t), false) => r.add(
                        "V8",
                        Verdict::Fail,
                        format!("empty-Maps control: returned {} with no map on disk, but no embedded map was found", secs(t)),
                    ),
                    (None, true) => r.add(
                        "V8",
                        Verdict::Warn,
                        "empty-Maps control: this file carries a map but did not validate against it",
                    ),
                }
            }
        }
    }
    // ---- V9 the ENGINE re-simulating this tape ---------------------------
    //
    // V6 has a floor it cannot see past, and the floor is measured: a search
    // tape that differs from its template by a few per cent of ticks still
    // agrees with the template's recording on the other 97 %, and three of
    // this project's own files labelled `SEARCHTAPE_..._DO_NOT_PUBLISH` score
    // kappa 0.83 -- the same as a human recording. The only thing that settles
    // it is running the tape through the real engine and comparing the
    // trajectory the engine produces with the trajectory the file claims.
    if has(a, "--engine") {
        match flag(a, "--map") {
            None => r.add("V9", Verdict::Na, "--engine needs --map"),
            Some(m) => match crate::regen::engine_trajectory_agreement(path, m) {
                Err(e) => r.add("V9", Verdict::Na, format!("engine trajectory: {}", e)),
                Ok((mean, worst, n, shift)) => {
                    let thr: f64 = flag(a, "--traj-thr").and_then(|v| v.parse().ok()).unwrap_or(0.05);
                    if shift {
                        r.add(
                            "V9",
                            Verdict::Fail,
                            format!(
                                "engine trajectory: the recording is a WHOLE SAMPLE out of phase with the \
                                 engine's own run of this tape (mean {:.4} m over {} samples). A solo clip \
                                 cannot look wrong from this; every frame-synchronous comparison is.",
                                mean, n
                            ),
                        );
                    } else if mean <= thr {
                        r.add(
                            "V9",
                            Verdict::Pass,
                            format!(
                                "engine trajectory: the engine's own run of this tape matches the recording \
                                 to {:.4} m mean / {:.4} m worst over {} samples",
                                mean, worst, n
                            ),
                        );
                    } else {
                        r.add(
                            "V9",
                            Verdict::Fail,
                            format!(
                                "engine trajectory: running THIS tape through the real engine puts the car \
                                 {:.3} m from where this file says it was (worst {:.3} m over {} samples). \
                                 The recording is not this tape's run.",
                                mean, worst, n
                            ),
                        );
                    }
                }
            },
        }
    }
    r
}

pub fn cmd(a: &[String]) {
    let path = a.first().unwrap_or_else(|| die("ghost verify FILE [--map M] [--server DIR]"));
    println!("verifying {}", path);
    let r = run(path, a);
    r.print();
    let n_fail = r.checks.iter().filter(|c| c.verdict == Verdict::Fail).count();
    if n_fail > 0 {
        println!("\nREFUSED: {} check(s) failed", n_fail);
        std::process::exit(2);
    }
    println!("\nOK");
}
