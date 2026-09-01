//! recon -- grow an input tape forward for as long as it keeps matching a
//! speed trace read off a video.
//!
//! The problem this solves is the one the video leaves: the video says how
//! fast the car was going, at 60 Hz, and says nothing about which keys were
//! held. The oracle inverts that — any tape can be simulated exactly and its
//! own speed read back — so the reconstruction is a search whose objective is
//! "how long does this tape keep the video's speed".
//!
//! Two things shape it.
//!
//! **A reconstruction is right until it is wrong.** Mean error over the whole
//! run is meaningless: once the two cars are in different places, the
//! comparison is between two unrelated runs. The score is therefore the race
//! time at which the candidate stops tracking, and only the error BEFORE that
//! breaks ties.
//!
//! **The search is greedy in time, not global.** Each round mutates the
//! incumbent only around the point where it currently stops tracking, because
//! nothing earlier is in question — the incumbent already matches there — and
//! nothing later can be judged until this is fixed.
//!
//! Each candidate costs one `ghost tape script`, one `ghost tape inject` and
//! one `fk trace`; they are independent and run in parallel.

mod corridor;
mod onsurface;
mod wet;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Ev {
    pub ms: i64,
    pub press: bool,
    pub key: &'static str,
}

fn render(evs: &[Ev]) -> String {
    let mut s = String::new();
    for e in evs {
        s.push_str(&format!("{} {} {}\n", e.ms, if e.press { "press" } else { "release" }, e.key));
    }
    s
}

/// A tiny deterministic PRNG: the search must be repeatable.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[(self.next() % v.len() as u64) as usize]
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1).max(1) as u64) as i64
    }
}

pub struct Cfg {
    pub ghost: String,
    pub fk: String,
    pub base_gtape: String,
    pub donor: String,
    pub map: String,
    pub work: PathBuf,
    pub state_off: String,
    pub probe_tick: String,
    pub signature_ms: String,
    pub tol: f64,
    pub run: usize,
    pub match_ms: i64,
    pub keep_before: bool,
    pub corridor_m: f64,
    pub corridor_run: usize,
    /// The video's decoded wetness, and how far a candidate may stray from it.
    pub wet_video: Option<wet::Wet>,
    pub wet_tol: f64,
    pub wet_run: usize,
    /// Race time the score starts at. Zero for a reconstruction from the line;
    /// non-zero states a DIFFERENT claim — "given this driving up to T, how far
    /// past T can the run be recovered" — and the two must never be quoted as
    /// the same number.
    pub from_ms: i64,
}

/// (race ms at which tracking stops, mean |diff| before it)
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Score {
    pub until: i64,
    pub err: f64,
}

impl Score {
    fn better(&self, o: &Score) -> bool {
        if self.until != o.until {
            return self.until > o.until;
        }
        self.err < o.err
    }
}

fn load_video(path: &str) -> BTreeMap<i64, f64> {
    let txt = std::fs::read_to_string(path).expect("video trace");
    let mut m = BTreeMap::new();
    for l in txt.lines().skip(1) {
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 2 || f[1].is_empty() {
            continue;
        }
        m.insert((f[0].parse::<f64>().unwrap() * 1000.0).round() as i64, f[1].parse().unwrap());
    }
    m
}

fn load_positions(path: &Path) -> Option<BTreeMap<i64, (f64, f64, f64)>> {
    let txt = std::fs::read_to_string(path).ok()?;
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next()?.split(',').collect();
    let c = |n: &str| hdr.iter().position(|h| *h == n);
    let (ct, cx, cy, cz) = (c("time_ms")?, c("x")?, c("y")?, c("z")?);
    let mut m = BTreeMap::new();
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        if let (Some(Ok(t)), Some(Ok(x)), Some(Ok(y)), Some(Ok(z))) = (
            f.get(ct).map(|v| v.parse::<i64>()),
            f.get(cx).map(|v| v.parse::<f64>()),
            f.get(cy).map(|v| v.parse::<f64>()),
            f.get(cz).map(|v| v.parse::<f64>()),
        ) {
            m.insert(t, (x, y, z));
        }
    }
    Some(m)
}

fn load_engine(path: &Path) -> Option<BTreeMap<i64, f64>> {
    let txt = std::fs::read_to_string(path).ok()?;
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next()?.split(',').collect();
    let ct = hdr.iter().position(|h| *h == "time_ms")?;
    let cs = hdr.iter().position(|h| *h == "speed_kmh")?;
    let mut m = BTreeMap::new();
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        if let (Some(Ok(t)), Some(Ok(v))) =
            (f.get(ct).map(|x| x.parse::<i64>()), f.get(cs).map(|x| x.parse::<f64>()))
        {
            m.insert(t, v);
        }
    }
    Some(m)
}

fn score(
    video: &BTreeMap<i64, f64>,
    eng: &BTreeMap<i64, f64>,
    tol: f64,
    run: usize,
    match_ms: i64,
    from_ms: i64,
) -> Score {
    let mut bad = 0usize;
    let mut n = 0usize;
    let mut sum = 0.0;
    let mut last_ok = from_ms;
    for (t, v) in video.range(from_ms..) {
        // Closest VALUE inside the timing window, not closest instant: see the
        // note in vidread::enginecmp. On the launch ramp a ten millisecond
        // difference is four km/h and a nearest-instant rule scores two runs
        // of the same tape 8 seconds apart.
        let mut near: Option<f64> = None;
        for (_, e) in eng.range(t - match_ms..=t + match_ms) {
            if near.map_or(true, |b: f64| (e - v).abs() < (b - v).abs()) {
                near = Some(*e);
            }
        }
        let Some(e) = near else { continue };
        let d = (e - v).abs();
        n += 1;
        sum += d;
        if d > tol {
            bad += 1;
            if bad >= run {
                return Score { until: last_ok, err: if n > 0 { sum / n as f64 } else { 1e9 } };
            }
        } else {
            bad = 0;
            last_ok = *t;
        }
    }
    Score { until: last_ok, err: if n > 0 { sum / n as f64 } else { 1e9 } }
}

fn evaluate(
    cfg: &Cfg,
    id: usize,
    evs: &[Ev],
    video: &BTreeMap<i64, f64>,
    corr: Option<&corridor::Corridor>,
) -> Option<Score> {
    let d = cfg.work.join(format!("c{id}"));
    std::fs::create_dir_all(&d).ok()?;
    let ev = d.join("ev.txt");
    let gt = d.join("t.gtape");
    let gb = d.join("c.Replay.Gbx");
    let tr = d.join("tr.csv");
    // A candidate directory is reused every round. If this candidate's trace
    // fails to be written, a STALE trace from an earlier round is still lying
    // there, and reading it scores one tape against another tape's trajectory --
    // silently, and in the direction that inflates the score. Remove it first
    // so a missing file is a failed candidate and nothing else.
    let _ = std::fs::remove_file(&tr);
    std::fs::write(&ev, render(evs)).ok()?;
    let ok = Command::new(&cfg.ghost)
        .args(["tape", "script", &cfg.base_gtape])
        .arg("--events")
        .arg(&ev)
        .arg("--signature-at")
        .arg(&cfg.signature_ms)
        .args(if cfg.keep_before { vec!["--keep-before"] } else { vec![] })
        .arg("--out")
        .arg(&gt)
        .output()
        .ok()?;
    if !ok.status.success() {
        // A candidate that cannot be BUILT is a configuration error, not a bad
        // tape, and swallowing it silently costs an hour: the search prints
        // "0 evaluated" and every explanation for that is wrong except this
        // one. The child's own words, once, are what tell them apart.
        eprintln!("recon: ghost tape script failed: {}", String::from_utf8_lossy(&ok.stderr).trim());
        return None;
    }
    let ok = Command::new(&cfg.ghost)
        .args(["tape", "inject", &cfg.donor])
        .arg(&gb)
        .arg("--tape")
        .arg(&gt)
        .output()
        .ok()?;
    if !ok.status.success() {
        return None;
    }
    // fk aborts on its own self-check for reasons that are about the check's
    // calibration, not the trajectory (see the map's write-up); the CSV it
    // wrote is still the engine's own state, so the exit status is not the
    // test — the presence and length of the file is.
    let _ = Command::new(&cfg.fk)
        .env("FK_STATE_OFF", &cfg.state_off)
        .env("FK_VERR_MAX", "2.0")
        .args(["trace", "--tape"])
        .arg(&gb)
        .arg("--map")
        .arg(&cfg.map)
        .arg("--at")
        .arg(format!("tick:{}", cfg.probe_tick))
        .arg("--out")
        .arg(&tr)
        .arg("--work")
        .arg(d.join("fk"))
        .output()
        .ok()?;
    let eng = load_engine(&tr)?;
    if eng.len() < 100 {
        return None;
    }
    let mut sc = score(video, &eng, cfg.tol, cfg.run, cfg.match_ms, cfg.from_ms);
    // A TRACE THAT RUNS OUT LOOKS EXACTLY LIKE ONE THAT TRACKS PERFECTLY.
    // `score` skips video instants with no engine sample near them, so a trace
    // that stops early records no disagreement past its own end and the
    // candidate keeps whatever it had earned. Cap the score at the last instant
    // the engine actually reported.
    let eng_end = *eng.keys().last()?;
    sc.until = sc.until.min(eng_end);
    // A candidate that has LEFT THE TRACK keeps the video's speed for a while
    // as it falls -- 16 m away and 4 m below, and still scoring. Where the
    // human corridor makes a claim, it is the earlier of the two that is true.
    //
    // AND A GATE THAT CANNOT BE EVALUATED IS NOT A GATE THAT PASSED. If the
    // caller asked for the corridor or the wetness and this trace cannot serve
    // it, the candidate is REFUSED. Scoring it on the remaining objectives is
    // how a search ends up keeping its luckiest measurement: on this map, under
    // heavy load, one such candidate was reported at 12.580 and re-measures at
    // 12.480 on a quiet box, because 12.580 is its SPEED-ONLY score.
    if let Some(c) = corr {
        let pos = load_positions(&tr)?;
        if let Some(left) = c.departs(&pos, cfg.corridor_m, cfg.corridor_run, 30, cfg.from_ms) {
            if left < sc.until {
                sc.until = left;
            }
        }
    }
    // And where the corridor is silent -- past the reroute, which is most of
    // this run -- the wetness readout is not. Same rule: the score is the
    // EARLIEST of the observables that make a claim, because a candidate is
    // right until it is wrong and any one of them can be the thing that is
    // wrong first.
    if let Some(v) = &cfg.wet_video {
        let e = wet::load_series(tr.to_str()?)?;
        if let Some(left) = wet::departs(v, &e, cfg.wet_tol, cfg.wet_run, cfg.match_ms, cfg.from_ms)
        {
            if left < sc.until {
                sc.until = left;
            }
        }
    }
    Some(sc)
}

/// Race windows the video OBSERVED directly, in which the search may not
/// invent anything. A recovered key record is not a hint to be traded against
/// a better score -- it is the only direct evidence about the tape that exists,
/// and a search allowed to overwrite it will, because a wrong input that buys
/// a tenth of tracking always outscores a right one that does not.
fn locked(windows: &[(i64, i64)], ms: i64) -> bool {
    windows.iter().any(|(a, b)| ms >= *a && ms <= *b)
}

fn mutate(
    rng: &mut Rng,
    base: &[Ev],
    around: i64,
    back: i64,
    fwd: i64,
    floor: i64,
    windows: &[(i64, i64)],
) -> Vec<Ev> {
    let mut e = base.to_vec();
    // Escape a local optimum by taking something back: a greedy search that
    // can only ADD events cements every mistake it has already made, and the
    // events that matter are the recent ones, near where it stopped tracking.
    if rng.next() % 4 == 0 && e.len() > 2 {
        let mut near: Vec<usize> =
            (0..e.len()).filter(|&i| (e[i].ms - around).abs() < 2500 && e[i].ms > floor && !locked(windows, e[i].ms)).collect();
        if !near.is_empty() {
            let victim = near.remove((rng.next() % near.len() as u64) as usize);
            e.remove(victim);
        }
    }
    let key = rng.pick(&["left", "right", "brake", "gas"]);
    let start = (rng.range(around - back, around + fwd) / 10 * 10).max(floor);
    if locked(windows, start) {
        return e;
    }
    let dur = rng.pick(&[30, 50, 80, 120, 180, 250, 350, 500, 800, 1200]);
    if key == "gas" {
        // gas is held by default; a mutation lifts it for a while
        e.push(Ev { ms: start.max(0), press: false, key: "gas" });
        e.push(Ev { ms: start.max(0) + dur, press: true, key: "gas" });
    } else {
        e.push(Ev { ms: start.max(0), press: true, key });
        e.push(Ev { ms: start.max(0) + dur, press: false, key });
    }
    e.sort();
    e.dedup();
    e
}

/// `recon onsurface` -- a CHECKER, deliberately not a search objective. It asks
/// the map whether the car had anything under it, beside a human run at the
/// same instants, because the model has holes and "no surface" only means
/// something where a real run has one.
fn cmd_onsurface(a: &[String]) {
    let get = |k: &str, d: &str| -> String {
        a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned().unwrap_or(d.into())
    };
    let map = get("--map", "map.Map.Gbx");
    let yoff = get("--yoff", "-64");
    let mg = get("--mapgeom", "mapgeom");
    let (from, to, every) = (
        get("--from-ms", "8000").parse().unwrap(),
        get("--to-ms", "16000").parse().unwrap(),
        get("--every-ms", "200").parse().unwrap(),
    );
    let drop_max: f64 = get("--drop-max", "3.0").parse().unwrap();
    let cand = onsurface::load_traj(&get("--candidate", ""), from, to, every).expect("candidate");
    let human = onsurface::load_traj(&get("--human", ""), from, to, every).expect("human");
    let hp = onsurface::plumb(&mg, &map, &yoff, &human, drop_max).expect("plumb human");
    let cp = onsurface::plumb(&mg, &map, &yoff, &cand, drop_max).expect("plumb candidate");
    let href: BTreeMap<i64, bool> =
        human.iter().zip(&hp).map(|(s, r)| (s.race_ms, r.is_some())).collect();
    let mut o = std::io::stdout();
    onsurface::report(&get("--label", "candidate"), &cand, &cp, &href, &mut o);
}

/// `recon wetcmp` -- hold the video's decoded wetness against any number of
/// simulated or recorded series, and say where each one stops reproducing it.
///
/// This is the objective run as a REPORT rather than as a gate, and its first
/// job is not scoring candidates at all: held against the human replays it
/// dates the point where this run stops driving the human route, which is the
/// number `--corridor-to` needs and which was previously a guess.
fn cmd_wetcmp(a: &[String]) {
    let get = |k: &str, d: &str| -> String {
        a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned().unwrap_or(d.into())
    };
    let vpath = get("--video", "wet_video.tsv");
    let video = load_ref(a, &vpath, get("--wet-shift-ms", "0").parse().unwrap());
    let tol: f64 = get("--wet-tol", "5").parse().unwrap();
    let run: usize = get("--wet-run", "6").parse().unwrap();
    let match_ms: i64 = get("--match-ms", "50").parse().unwrap();
    println!("video: {} readings, race {:.3}..{:.3} s", video.len(), *video.keys().next().unwrap() as f64 / 1000.0, *video.keys().last().unwrap() as f64 / 1000.0);
    for (i, k) in a.iter().enumerate() {
        if k != "--series" {
            continue;
        }
        let path = &a[i + 1];
        let Some(e) = wet::load_series(path) else {
            println!("{path}: NO WETNESS COLUMN");
            continue;
        };
        let r = wet::agreement(&video, &e, tol, run, match_ms);
        println!(
            "{path}\tshared {}\twithin {} pt: {} ({:.1} %)\tmean |diff| {:.2} pt\tlast agreed {}\tfirst break {}",
            r.shared,
            tol,
            r.within,
            100.0 * r.within as f64 / r.shared.max(1) as f64,
            r.mean_abs,
            r.last_agreed.map(|t| format!("{:.3} s", t as f64 / 1000.0)).unwrap_or_else(|| "-".into()),
            r.first_break.map(|t| format!("{:.3} s", t as f64 / 1000.0)).unwrap_or_else(|| "never".into())
        );
        if a.iter().any(|x| x == "--dump") {
            let every: usize = get("--dump-every", "10").parse().unwrap();
            for (i, (t, v, e)) in r.rows.iter().enumerate() {
                if i % every == 0 {
                    println!("  {:.3}\tvideo {:5.1}\tseries {:6.2}\td {:6.2}", *t as f64 / 1000.0, v, e, e - v);
                }
            }
        }
    }
}

/// Load the reference wetness series the way both the gate and the report see
/// it: the decoded file, plus any control shift, plus any asserted band.
/// One function so a report can never be shown a different series from the one
/// the search scored against.
fn load_ref(a: &[String], path: &str, shift_ms: i64) -> wet::Wet {
    let mut w = wet::load_video(path)
        .or_else(|| wet::load_series(path))
        .expect("reference wetness series");
    if shift_ms != 0 {
        eprintln!("wetness: series shifted {shift_ms} ms -- this is the CONTROL, not a measurement");
        w = wet::shift(&w, shift_ms);
    }
    // --wet-zero FROM:TO asserts a DRY window the reader could not read,
    // because the HUD draws nothing when there is nothing to draw. On this run
    // the supports for race 10100..20600 are four and independent: the reset
    // MEASURED at 10.038; the frames, which show the car on dry blue surfaces
    // throughout; the run's author saying "flat lining into the water pool" at
    // race 20.3; and the soak rate, which puts the entry behind the 100 % at
    // 22.355 no earlier than race 21.1. It is still an ASSERTION and the log
    // says so on every run.
    for (i, k) in a.iter().enumerate() {
        if k != "--wet-zero" {
            continue;
        }
        let (f, t) = a[i + 1].split_once(':').expect("--wet-zero FROM_MS:TO_MS");
        let (f, t) = (f.parse().unwrap(), t.parse().unwrap());
        eprintln!(
            "wetness: ASSERTING 0 % over race {:.3}..{:.3} s -- not read, inferred",
            f as f64 / 1000.0,
            t as f64 / 1000.0
        );
        w = wet::assert_band(&w, f, t, 0.0, 60.0);
    }
    w
}

fn main() {
    // --version / -V. Compile-time only: CARGO_PKG_* come from the crate's
    // Cargo.toml (which inherits the one workspace version), and TAS_BUILD is
    // the git hash the release build sets. option_env! means an ordinary
    // `cargo build` still works and simply reports "dev". No dependency.
    if std::env::args().any(|x| x == "--version" || x == "-V") {
        println!(
            "{} {} ({})",
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            env!("CARGO_PKG_VERSION"),
            option_env!("TAS_BUILD").unwrap_or("dev")
        );
        std::process::exit(0);
    }
    if std::env::args().any(|x| x == "--help" || x == "-h") {
        // Usage on STDOUT, exit 0 -- see gbx/tests/cli_contract.rs.
        print!("{}", r#"
recon -- grow an input tape forward for as long as it keeps matching

  recon gas|left|right TRACE.csv [flags]
        Extend a tape one input at a time and stop at the first tick whose
        simulated state leaves the recorded trace.
"#);
        std::process::exit(0);
    }
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.first().map(|x| x.as_str()) == Some("onsurface") {
        cmd_onsurface(&a[1..]);
        return;
    }
    if a.first().map(|x| x.as_str()) == Some("wetcmp") {
        cmd_wetcmp(&a[1..]);
        return;
    }
    let get = |k: &str, d: &str| -> String {
        a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned().unwrap_or(d.into())
    };
    let cfg = Cfg {
        ghost: get("--ghost", "ghost"),
        fk: get("--fk", "fk"),
        base_gtape: get("--base", "base.gtape"),
        donor: get("--donor", "donor.Replay.Gbx"),
        map: get("--map", "map.Map.Gbx"),
        work: PathBuf::from(get("--work", "/tmp/recon")),
        state_off: get("--state-off", "8183260"),
        probe_tick: get("--probe-tick", "60"),
        signature_ms: get("--signature-at", "95000"),
        tol: get("--tol", "8").parse().unwrap(),
        run: get("--run", "6").parse().unwrap(),
        match_ms: get("--match-ms", "50").parse().unwrap(),
        keep_before: a.iter().any(|x| x == "--keep-before"),
        corridor_m: get("--corridor-m", "12").parse().unwrap(),
        corridor_run: get("--corridor-run", "5").parse().unwrap(),
        // --wet FILE turns the decoded wetness on as an objective. Off by
        // default: a gate that is on when nobody asked for it is a gate whose
        // effect on a number cannot be measured.
        // --wet FILE turns the decoded wetness on as an objective. Off by
        // default: a gate that is on when nobody asked for it is a gate whose
        // effect on a number cannot be measured.
        wet_video: a
            .iter()
            .position(|x| x == "--wet")
            .map(|i| load_ref(&a, &a[i + 1], get("--wet-shift-ms", "0").parse().unwrap())),
        wet_tol: get("--wet-tol", "5").parse().unwrap(),
        wet_run: get("--wet-run", "6").parse().unwrap(),
        from_ms: get("--from-ms", "0").parse().unwrap(),
    };
    let video = load_video(&get("--video", "video.tsv"));
    // --corridor FILE.csv (repeatable): human trajectories that bound where a
    // car on this route can be. --corridor-to is the race time past which the
    // run being reconstructed stops following that route.
    let corridor_files: Vec<String> = a
        .iter()
        .enumerate()
        .filter(|(_, x)| *x == "--corridor")
        .map(|(i, _)| a[i + 1].clone())
        .collect();
    let corr = if corridor_files.is_empty() {
        None
    } else {
        let until: i64 = get("--corridor-to", "30000").parse().unwrap();
        let c = corridor::Corridor::load(&corridor_files, until).expect("corridor");
        eprintln!(
            "corridor: {} human lines, authoritative to race {:.3} s, tube {} m",
            c.lines.len(),
            until as f64 / 1000.0,
            get("--corridor-m", "12")
        );
        Some(c)
    };
    let rounds: usize = get("--rounds", "12").parse().unwrap();
    let batch: usize = get("--batch", "48").parse().unwrap();
    let seed: u64 = get("--seed", "20260822").parse().unwrap();
    // How far back from the divergence a mutation may reach. The fix for a car
    // that leaves the line at T is usually not at T: it is wherever the aim was
    // set, which can be seconds earlier.
    let back: i64 = get("--back", "800").parse().unwrap();
    let fwd: i64 = get("--fwd", "200").parse().unwrap();
    std::fs::create_dir_all(&cfg.work).expect("work dir");

    // Seeding at a race time other than 0, with --keep-before, makes the search
    // an EDIT of the base tape from that instant on: the opening of this map is
    // forced and every real run drives it identically, so there is nothing there
    // for a search to find.
    let seed_ms: i64 = get("--seed-ms", "0").parse().unwrap();
    let mut best: Vec<Ev> = vec![Ev { ms: seed_ms, press: true, key: "gas" }];
    // --events FILE scores one list and stops: the way to ask this binary what
    // it thinks of a list some other tool produced, which is the only way two
    // implementations of the same statistic can be held against each other.
    if let Some(p) = a.iter().position(|x| x == "--events") {
        let txt = std::fs::read_to_string(&a[p + 1]).expect("events");
        let mut evs: Vec<Ev> = Vec::new();
        for l in txt.lines() {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 3 { continue; }
            let key = match f[2] { "gas" => "gas", "brake" => "brake", "left" => "left", "right" => "right", _ => continue };
            evs.push(Ev { ms: f[0].parse().unwrap(), press: f[1] == "press", key });
        }
        let sc = evaluate(&cfg, 999, &evs, &video, corr.as_ref()).expect("evaluate");
        println!("{} events: tracks to {:.3} s, mean |diff| {:.2}", evs.len(), sc.until as f64 / 1000.0, sc.err);
        return;
    }
    let mut best_score: Score = evaluate(&cfg, 0, &best, &video, corr.as_ref()).expect("the seed tape must evaluate");
    println!("seed: tracks to {:.3} s, mean |diff| {:.2} km/h", best_score.until as f64 / 1000.0, best_score.err);

    // --anchor FILE.events, repeatable: an observed record, spliced in and then
    // locked. Its window comes from the record itself.
    let mut windows: Vec<(i64, i64)> = Vec::new();
    for (i, k) in a.iter().enumerate() {
        if k != "--anchor" {
            continue;
        }
        let txt = std::fs::read_to_string(&a[i + 1]).expect("anchor events");
        let (mut lo, mut hi) = (i64::MAX, i64::MIN);
        for l in txt.lines() {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 3 {
                continue;
            }
            let key = match f[2] {
                "gas" => "gas",
                "brake" | "brake2" => "brake",
                "left" => "left",
                "right" => "right",
                _ => continue,
            };
            let ms: i64 = match f[0].parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            lo = lo.min(ms);
            hi = hi.max(ms);
            best.push(Ev { ms, press: f[1] == "press", key });
        }
        if lo <= hi {
            windows.push((lo, hi));
            eprintln!("anchored: race {lo}..{hi} ms is locked");
        }
    }
    best.sort();
    best.dedup();
    if !windows.is_empty() {
        best_score = evaluate(&cfg, 0, &best, &video, corr.as_ref()).expect("the anchored seed must evaluate");
        println!(
            "with anchors: tracks to {:.3} s, mean |diff| {:.2} km/h",
            best_score.until as f64 / 1000.0,
            best_score.err
        );
    }

    let mut rng = Rng(seed);
    for round in 1..=rounds {
        let cands: Vec<Vec<Ev>> =
            (0..batch)
            .map(|_| {
                mutate(&mut rng, &best, best_score.until.max(seed_ms), back, fwd, seed_ms, &windows)
            })
            .collect();
        let next = Arc::new(AtomicUsize::new(0));
        let out: Arc<Mutex<Vec<(usize, Score)>>> = Arc::new(Mutex::new(Vec::new()));
        let jobs: usize = get("--jobs", "24").parse().unwrap();
        std::thread::scope(|s| {
            for _ in 0..jobs {
                let next = next.clone();
                let out = out.clone();
                let cands = &cands;
                let cfg = &cfg;
                let video = &video;
                let corr = corr.as_ref();
                s.spawn(move || loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= cands.len() {
                        break;
                    }
                    if let Some(sc) = evaluate(cfg, i + 1, &cands[i], video, corr) {
                        out.lock().unwrap().push((i, sc));
                    }
                });
            }
        });
        let res = out.lock().unwrap();
        let mut improved = false;
        for (i, sc) in res.iter() {
            if sc.better(&best_score) {
                best_score = *sc;
                best = cands[*i].clone();
                improved = true;
            }
        }
        println!(
            "round {round}: {} evaluated, best tracks to {:.3} s (mean |diff| {:.2}){}",
            res.len(),
            best_score.until as f64 / 1000.0,
            best_score.err,
            if improved { " <- improved" } else { "" }
        );
        let mut f = std::fs::File::create(cfg.work.join("best.events")).expect("best");
        write!(f, "{}", render(&best)).unwrap();
    }
    println!("\nbest event list:\n{}", render(&best));
}
