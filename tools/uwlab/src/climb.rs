//! `uwlab climb` — a hill-climb on HEIGHT, which is the only objective this
//! map has left.
//!
//! One blind run in two thousand climbed 7.8 m up the face of a stadium pillar
//! at the deck's south-east corner — 121.25 m against a deck at 114.16 — by
//! wedging itself against the pillar and being extruded upward. That is the
//! only mechanism found on this map that gains real height, and the finish's
//! live floor is 16.8 m above the deck. Whether the mechanism reaches that far
//! is a search question with a continuous objective, so: mutate a tape, keep
//! what climbs.
//!
//! The score is read off a written tape re-simulated by the engine. It is a
//! MEASUREMENT, not a result: a candidate that reaches the gate has to be
//! re-run through the plain oracle before anyone says the word finish.

use crate::traj::Traj;
use std::sync::atomic::{AtomicUsize, Ordering};

fn flag<'a>(a: &'a [String], n: &str) -> Option<&'a str> {
    a.iter().position(|s| s == n).and_then(|i| a.get(i + 1)).map(|s| s.as_str())
}

pub struct Rng(u64);
impl Rng {
    pub fn new(s: u64) -> Rng {
        Rng(s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn upto(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// The per-tick inputs of a tape, with the template's own text kept so the
/// header and the bit coding are never rewritten from scratch (a from-scratch
/// header says bits_used = 0 and the oracle DNFs the file).
struct Tape {
    lines: Vec<String>,
    /// index into `lines` for each tick that carries inputs
    idx: Vec<usize>,
    steer: Vec<i32>,
    accel: Vec<i32>,
    brake: Vec<i32>,
}

fn getnum(line: &str, key: &str) -> Option<i32> {
    let p = line.find(key)?;
    let rest = &line[p + key.len()..];
    let end = rest.find(' ').unwrap_or(rest.len());
    rest[..end].parse().ok()
}

impl Tape {
    fn load(path: &str) -> Result<Tape, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let mut t = Tape { lines: Vec::new(), idx: Vec::new(), steer: Vec::new(), accel: Vec::new(), brake: Vec::new() };
        for line in text.lines() {
            let i = t.lines.len();
            t.lines.push(line.to_string());
            if line.starts_with("t=") && line.contains("steer=") {
                t.idx.push(i);
                t.steer.push(getnum(line, "steer=").unwrap_or(0));
                t.accel.push(getnum(line, "accel=").unwrap_or(0));
                t.brake.push(getnum(line, "brake=").unwrap_or(0));
            }
        }
        if t.idx.is_empty() {
            return Err(format!("{path}: no input ticks"));
        }
        Ok(t)
    }
    fn write(&self, path: &str) -> std::io::Result<()> {
        let mut out = self.lines.clone();
        for (k, &li) in self.idx.iter().enumerate() {
            let l = &self.lines[li];
            let head = &l[..l.find("steer=").unwrap()];
            let tail = match l.find("flags=") {
                Some(p) => &l[p..],
                None => "",
            };
            out[li] = format!("{head}steer={} accel={} brake={} {tail}", self.steer[k], self.accel[k], self.brake[k]);
        }
        std::fs::write(path, out.join("\n") + "\n")
    }
    fn mutate(&mut self, r: &mut Rng, strength: usize, lo: usize, hi: usize) {
        let n = self.idx.len().min(hi);
        let lo = lo.min(n.saturating_sub(1));
        let steers = [-127i32, -96, -64, -32, -16, -4, 1, 4, 16, 32, 64, 96, 127];
        for _ in 0..strength {
            let t0 = lo + r.upto((n - lo).max(1));
            let len = 3 + r.upto(150);
            let s = steers[r.upto(steers.len())];
            let g = if r.upto(10) < 7 { 1 } else { 0 };
            let b = if r.upto(10) < 3 { 1 } else { 0 };
            let what = r.upto(3);
            for k in t0..(t0 + len).min(n) {
                if what != 1 {
                    self.steer[k] = s;
                }
                if what != 0 {
                    self.accel[k] = g;
                    self.brake[k] = b;
                }
            }
        }
    }
}

pub fn cmd_climb(a: &[String]) -> i32 {
    let need = |n: &str| -> String {
        match flag(a, n) {
            Some(v) => v.to_string(),
            None => {
                eprintln!("uwlab climb: {n} is required");
                std::process::exit(2);
            }
        }
    };
    let map = need("--map");
    let carrier = need("--carrier");
    let basetape = need("--tape");
    let fk = flag(a, "--fk").unwrap_or("fk").to_string();
    let ghostb = flag(a, "--ghost").unwrap_or("ghost").to_string();
    let dir = flag(a, "--dir").unwrap_or("climb").to_string();
    let iters: usize = flag(a, "--iters").and_then(|s| s.parse().ok()).unwrap_or(20);
    let pop: usize = flag(a, "--pop").and_then(|s| s.parse().ok()).unwrap_or(32);
    let jobs: usize = flag(a, "--jobs").and_then(|s| s.parse().ok()).unwrap_or(32);
    let seed: u64 = flag(a, "--seed").and_then(|s| s.parse().ok()).unwrap_or(1);
    // score only inside this xz window, so the climber cannot win by finding
    // some unrelated high place on the other side of the map
    let win: Vec<f64> = flag(a, "--window")
        .unwrap_or("0,0,4000,4000")
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    let (wx0, wz0, wx1, wz1) = (win[0], win[1], win[2], win[3]);
    let (mlo, mhi) = match flag(a, "--mut-range").and_then(|s| s.split_once(':').map(|(x, y)| (x.parse().unwrap_or(0), y.parse().unwrap_or(usize::MAX)))) {
        Some(v) => v,
        None => (0usize, usize::MAX),
    };
    let goal: Option<Vec<f64>> = flag(a, "--goalbox").and_then(|s| {
        let (p, q) = s.split_once(':')?;
        let v: Vec<f64> = p.split(',').chain(q.split(',')).filter_map(|x| x.parse().ok()).collect();
        if v.len() == 6 { Some(v) } else { None }
    });
    let endw: f64 = flag(a, "--endw").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let (bonus_y, bonus_w) = match flag(a, "--bonus").and_then(|s| s.split_once(':').map(|(y, w)| (y.parse().unwrap_or(0.0), w.parse().unwrap_or(0.0)))) {
        Some(v) => v,
        None => (0.0f64, 0.0f64),
    };
    let _ = std::fs::create_dir_all(&dir);

    let base = match Tape::load(&basetape) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("uwlab climb: {e}");
            return 2;
        }
    };
    let mut best = base;
    let mut bestscore = f64::MIN;
    let mut bestwhere = String::new();
    let mut rng = Rng::new(seed);

    for it in 0..=iters {
        // iteration 0 scores the incumbent alone
        let cands: Vec<Tape> = if it == 0 {
            vec![Tape { lines: best.lines.clone(), idx: best.idx.clone(), steer: best.steer.clone(), accel: best.accel.clone(), brake: best.brake.clone() }]
        } else {
            (0..pop)
                .map(|_| {
                    let mut c = Tape { lines: best.lines.clone(), idx: best.idx.clone(), steer: best.steer.clone(), accel: best.accel.clone(), brake: best.brake.clone() };
                    let strength = 1 + rng.upto(3);
                    c.mutate(&mut Rng::new(rng.next()), strength, mlo, mhi);
                    c
                })
                .collect()
        };
        let scores: Vec<(f64, String)> = vec![(f64::MIN, String::new()); cands.len()];
        let scores = std::sync::Mutex::new(scores);
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..jobs.min(cands.len()) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(c) = cands.get(i) else { return };
                    let gt = format!("{dir}/c{i}.gtape");
                    let gh = format!("{dir}/c{i}.Ghost.Gbx");
                    let csv = format!("{dir}/c{i}.csv");
                    let work = format!("/tmp/uwclimb-{}-{i}", dir.replace('/', "_"));
                    if c.write(&gt).is_err() {
                        return;
                    }
                    let o = std::process::Command::new(&ghostb).args(["tape", "inject", &carrier, &gh, "--tape", &gt]).output();
                    if o.map(|o| !o.status.success()).unwrap_or(true) {
                        return;
                    }
                    let _ = std::fs::remove_dir_all(&work);
                    let _ = std::process::Command::new(&fk)
                        .args(["trace", "--tape", &gh, "--map", &map, "--work", &work, "--at", "tick:160", "--out", &csv])
                        .env("FK_VERR_MAX", "3.0")
                        .output();
                    let _ = std::fs::remove_dir_all(&work);
                    if let Ok(t) = Traj::load(&csv) {
                        // ---- lexicographic goal score, non-overlapping bands.
                        // Height first, then distance, then inside, then finished:
                        // a car on the deck is 17 m from the gate box in Y and a car
                        // on the stands is 31 m from it in XZ, so a plain miss
                        // distance rewards giving the height back. The bands make
                        // that impossible, and each stays continuous inside itself.
                        if let Some(g) = &goal {
                            let mut s = f64::MIN;
                            let mut wh = String::new();
                            for r in &t.rows {
                                let inside = r.x >= g[0] && r.x <= g[3] && r.y >= g[1] && r.y <= g[4] && r.z >= g[2] && r.z <= g[5];
                                let v = if inside {
                                    100_000.0
                                } else if r.y >= g[1] {
                                    let dx = (g[0] - r.x).max(r.x - g[3]).max(0.0);
                                    let dz = (g[2] - r.z).max(r.z - g[5]).max(0.0);
                                    1000.0 - (dx * dx + dz * dz).sqrt()
                                } else {
                                    r.y
                                };
                                if v > s {
                                    s = v;
                                    wh = format!("({:.2}, {:.3}, {:.2}) t {:.2}", r.x, r.y, r.z, r.t);
                                }
                            }
                            let ended = t.rows.last().map(|r| r.t).unwrap_or(99.0);
                            if ended < 46.0 {
                                s += 1_000_000.0;
                                wh = format!("RUN ENDED at {ended:.3} — {wh}");
                            }
                            scores.lock().unwrap()[i] = (s, wh);
                            let _ = std::fs::remove_file(&csv);
                            continue;
                        }
                        let mut my = f64::MIN;
                        let mut wh = String::new();
                        for r in &t.rows {
                            if r.x >= wx0 && r.x <= wx1 && r.z >= wz0 && r.z <= wz1 && r.y > my {
                                my = r.y;
                                wh = format!("({:.2}, {:.3}, {:.2}) t {:.2}", r.x, r.y, r.z, r.t);
                            }
                        }
                        // Time above a threshold, added to the score: a car that LANDS
                        // on something high is worth far more than one that touches the
                        // same height at a ballistic apex and falls back, and maxy alone
                        // cannot tell them apart.
                        if bonus_w > 0.0 {
                            let mut secs = 0.0;
                            for w in t.rows.windows(2) {
                                if w[0].y > bonus_y && w[0].x >= wx0 && w[0].x <= wx1 && w[0].z >= wz0 && w[0].z <= wz1 {
                                    secs += w[1].t - w[0].t;
                                }
                            }
                            my += bonus_w * secs;
                        }
                        // Landing beats an apex: in water a ballistic peak decays at
                        // 2.7 m/s, so hang time alone rewards a slow sink. Where the
                        // car ENDS says whether it found something to stand on.
                        if endw > 0.0 {
                            if let Some(l) = t.rows.last() {
                                if l.x >= wx0 && l.x <= wx1 && l.z >= wz0 && l.z <= wz1 {
                                    my += endw * (l.y - 120.0).max(0.0);
                                }
                            }
                        }
                        // a run that ends early has finished: score it enormous
                        let ended = t.rows.last().map(|r| r.t).unwrap_or(99.0);
                        if ended < 46.0 {
                            my += 1000.0;
                            wh = format!("RUN ENDED at {ended:.3} — {wh}");
                        }
                        scores.lock().unwrap()[i] = (my, wh);
                    }
                    let _ = std::fs::remove_file(&csv);
                });
            }
        });
        let sc = scores.into_inner().unwrap();
        let (mut bi, mut bs) = (usize::MAX, f64::MIN);
        for (i, (s, _)) in sc.iter().enumerate() {
            if *s > bs {
                bs = *s;
                bi = i;
            }
        }
        if bi != usize::MAX && bs > bestscore {
            bestscore = bs;
            bestwhere = sc[bi].1.clone();
            best = Tape { lines: cands[bi].lines.clone(), idx: cands[bi].idx.clone(), steer: cands[bi].steer.clone(), accel: cands[bi].accel.clone(), brake: cands[bi].brake.clone() };
            let _ = best.write(&format!("{dir}/best.gtape"));
            println!("iter {it:3}  NEW BEST maxy {bestscore:9.3}  {bestwhere}");
        } else {
            println!("iter {it:3}  best {bestscore:9.3}  (no improvement)");
        }
    }
    println!("final: maxy {bestscore:.3} at {bestwhere}; tape {dir}/best.gtape");
    0
}

/// `uwlab shift` — put a solved manoeuvre later in a longer tape.
///
/// The endgame that finishes from the deck was solved from a spawn at the
/// stadium's east end; the published jump lands 216 m west of it. Rather than
/// search the whole thing again from the landing point, prepend a run-up:
/// write the base tape's inputs starting at tick N of a longer template, with
/// a constant input before that. The manoeuvre then happens at the same
/// tick-offset from the wedge, which is what a hill-climb needs as a seed.
pub fn cmd_shift(a: &[String]) -> i32 {
    let need = |n: &str| -> String {
        match flag(a, n) {
            Some(v) => v.to_string(),
            None => {
                eprintln!("uwlab shift: {n} is required");
                std::process::exit(2);
            }
        }
    };
    let base = need("--base");
    let template = need("--template");
    let out = need("--out");
    let n: usize = flag(a, "--shift").and_then(|s| s.parse().ok()).unwrap_or(0);
    let pre: Vec<i32> = flag(a, "--pre")
        .unwrap_or("1,1,0")
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    let (b, mut t) = match (Tape::load(&base), Tape::load(&template)) {
        (Ok(b), Ok(t)) => (b, t),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("uwlab shift: {e}");
            return 2;
        }
    };
    for k in 0..t.idx.len() {
        if k < n {
            t.steer[k] = *pre.first().unwrap_or(&1);
            t.accel[k] = *pre.get(1).unwrap_or(&1);
            t.brake[k] = *pre.get(2).unwrap_or(&0);
        } else if k - n < b.idx.len() {
            t.steer[k] = b.steer[k - n];
            t.accel[k] = b.accel[k - n];
            t.brake[k] = b.brake[k - n];
        } else {
            t.steer[k] = 1;
            t.accel[k] = 0;
            t.brake[k] = 0;
        }
    }
    if let Err(e) = t.write(&out) {
        eprintln!("uwlab shift: {e}");
        return 2;
    }
    println!("wrote {out}: {} ticks, base shifted by {n}", t.idx.len());
    0
}
