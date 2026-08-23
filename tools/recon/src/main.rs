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

fn score(video: &BTreeMap<i64, f64>, eng: &BTreeMap<i64, f64>, tol: f64, run: usize, match_ms: i64) -> Score {
    let mut bad = 0usize;
    let mut n = 0usize;
    let mut sum = 0.0;
    let mut last_ok = 0i64;
    for (t, v) in video {
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

fn evaluate(cfg: &Cfg, id: usize, evs: &[Ev], video: &BTreeMap<i64, f64>) -> Option<Score> {
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
    Some(score(video, &eng, cfg.tol, cfg.run, cfg.match_ms))
}

fn mutate(rng: &mut Rng, base: &[Ev], around: i64, back: i64, fwd: i64, floor: i64) -> Vec<Ev> {
    let mut e = base.to_vec();
    // Escape a local optimum by taking something back: a greedy search that
    // can only ADD events cements every mistake it has already made, and the
    // events that matter are the recent ones, near where it stopped tracking.
    if rng.next() % 4 == 0 && e.len() > 2 {
        let mut near: Vec<usize> =
            (0..e.len()).filter(|&i| (e[i].ms - around).abs() < 2500 && e[i].ms > floor).collect();
        if !near.is_empty() {
            let victim = near.remove((rng.next() % near.len() as u64) as usize);
            e.remove(victim);
        }
    }
    let key = rng.pick(&["left", "right", "brake", "gas"]);
    let start = (rng.range(around - back, around + fwd) / 10 * 10).max(floor);
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

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
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
    };
    let video = load_video(&get("--video", "video.tsv"));
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
        let sc = evaluate(&cfg, 999, &evs, &video).expect("evaluate");
        println!("{} events: tracks to {:.3} s, mean |diff| {:.2}", evs.len(), sc.until as f64 / 1000.0, sc.err);
        return;
    }
    let mut best_score = evaluate(&cfg, 0, &best, &video).expect("the seed tape must evaluate");
    println!("seed: tracks to {:.3} s, mean |diff| {:.2} km/h", best_score.until as f64 / 1000.0, best_score.err);

    let mut rng = Rng(seed);
    for round in 1..=rounds {
        let cands: Vec<Vec<Ev>> =
            (0..batch)
            .map(|_| mutate(&mut rng, &best, best_score.until.max(seed_ms), back, fwd, seed_ms))
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
                s.spawn(move || loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= cands.len() {
                        break;
                    }
                    if let Some(sc) = evaluate(cfg, i + 1, &cands[i], video) {
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
