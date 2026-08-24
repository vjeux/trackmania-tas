//! `tmexplore` — the CLI.
//!
//! ```text
//! tmexplore toy search   [flags]     the archive search on the toy track
//! tmexplore toy ablate   [flags]     the control battery: does the archive earn its keep?
//! tmexplore toy guard    [flags]     the guard's two-sided control
//! ```
//!
//! Everything under `toy` runs against the stub in `src/toy.rs` and says
//! nothing about a real map. The real subcommands appear when agent D's
//! `Branch` and agent A's `Verdict` land; the search itself does not change,
//! which is the point of building against traits.

use std::time::Instant;
use tmexplore::action::{Alphabet, Input};
use tmexplore::archive::{Bands, Policy};
use tmexplore::branch::{PlainOracle, Route};
use tmexplore::explore::{Cfg, Explorer};
use tmexplore::outcome::{Reached, Verdict};
use tmexplore::rng::Rng;
use tmexplore::toy::{ToyOracle, ToySim, ToyTrack};

fn secs(ms: i64) -> String {
    format!("{}.{:03}", ms / 1000, (ms % 1000).abs())
}

struct Args {
    map: Vec<(String, String)>,
    free: Vec<String>,
}

impl Args {
    fn parse() -> Args {
        let mut map = Vec::new();
        let mut free = Vec::new();
        let mut it = std::env::args().skip(1).peekable();
        while let Some(a) = it.next() {
            if let Some(k) = a.strip_prefix("--") {
                if let Some((k, v)) = k.split_once('=') {
                    map.push((k.to_string(), v.to_string()));
                } else {
                    let v = it.peek().filter(|s| !s.starts_with('-')).cloned();
                    if let Some(v) = v {
                        it.next();
                        map.push((k.to_string(), v));
                    } else {
                        map.push((k.to_string(), "true".into()));
                    }
                }
            } else if let Some(k) = a.strip_prefix('-') {
                // short flags are booleans; `-v` must not become a subcommand
                map.push((k.to_string(), "true".into()));
            } else {
                free.push(a);
            }
        }
        Args { map, free }
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.map.iter().rev().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }
    fn num<T: std::str::FromStr>(&self, k: &str, d: T) -> T {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    fn flag(&self, k: &str) -> bool {
        self.get(k).map(|v| v != "false").unwrap_or(false)
    }
}

fn cfg_from(a: &Args) -> Cfg {
    let mut c = Cfg {
        alphabet: Alphabet::parse(a.get("alphabet").unwrap_or("keyboard")).unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(2)
        }),
        k: a.num("k", 10u16),
        fanout: Some(a.num("fanout", 3usize)),
        max_rollout: a.num("rollout", 30u32),
        sticky: a.num("sticky", 0.7f64),
        policy: Policy {
            frontier_depth: a.num("frontier-depth", 3u32),
            p_frontier: a.num("p-frontier", 0.85f64),
            visit_decay: a.num("visit-decay", 0.5f64),
            time_halflife: a.num("time-halflife", 200.0f64),
        },
        bands: Bands {
            station_m: 20.0,
            lateral_m: a.num("band-lateral", 3.0f32),
            height_m: a.num("band-height", 4.0f32),
            speed_ms: a.num("band-speed", 5.0f32),
            yaw_deg: a.num("band-yaw", 20.0f32),
            state_blind: a.flag("state-blind"),
        },
        seed: a.num("seed", 1u64),
        tick_limit: a.num("tick-limit", 4000u32),
    };
    if c.k == 0 {
        c.k = 1;
    }
    c
}

/// One search run. Returns (best confirmed outcome, evals, furthest station,
/// wall seconds).
fn run_search(
    track: &ToyTrack,
    cfg: Cfg,
    budget_evals: u64,
    tree: bool,
    defect: usize,
    verbose: bool,
) -> (Option<Reached>, u64, u32, f64, u64, u64) {
    let mut sim = ToySim::new(track, tree, cfg.tick_limit);
    sim.inject_boundary_defect(defect);
    let oracle = ToyOracle::new(track);
    let mut ex = Explorer::new(track, cfg);
    ex.seed_root(&mut sim).expect("seed the root");

    let t0 = Instant::now();
    let mut last = 0u64;
    while ex.stats.evals < budget_evals {
        let rep = ex.step(&mut sim, &oracle);
        if verbose {
            for ms in &rep.confirmed {
                println!(
                    "  [{:>9} evals] *** CONFIRMED FINISH {}   (plain oracle re-simulated the tape)",
                    ex.stats.evals,
                    secs(*ms)
                );
            }
            for (claimed, cps) in &rep.phantoms {
                println!(
                    "  [{:>9} evals] *** PHANTOM: the fork claimed {} and the plain oracle says DNF cps {}",
                    ex.stats.evals,
                    secs(*claimed),
                    cps
                );
            }
            if ex.stats.evals - last > budget_evals / 10 {
                last = ex.stats.evals;
                println!(
                    "  [{:>9} evals] {} bins, furthest station {}, best seen {}",
                    ex.stats.evals,
                    ex.archive.len(),
                    ex.archive.max_station,
                    ex.best_seen.map(|b| b.to_string()).unwrap_or("-".into())
                );
            }
        }
        if rep.expanded == 0 && rep.errors.is_empty() && rep.hit_tick_limit {
            // every frontier bin is out of ticks; keep going, the policy will
            // draw a shallower one.
        }
    }
    let dt = t0.elapsed().as_secs_f64();
    (
        ex.best.as_ref().map(|(r, _)| *r),
        ex.stats.evals,
        ex.archive.max_station,
        dt,
        ex.stats.phantoms,
        ex.stats.confirmed,
    )
}

/// The no-archive control: sample whole macro sequences at random from the
/// start, same alphabet, same macro length, same evaluation budget.
///
/// This is the ablation that says whether the archive is doing anything. If
/// random rollouts finish the track too, the archive is decoration.
fn run_random(
    track: &ToyTrack,
    cfg: &Cfg,
    budget_evals: u64,
    seed: u64,
) -> (Option<Reached>, u32, u64) {
    let oracle = ToyOracle::new(track);
    let mut rng = Rng::new(seed);
    let acts = cfg.alphabet.actions();
    let n_macros = (cfg.tick_limit / cfg.k as u32) as usize;
    let mut best: Option<Reached> = None;
    let mut furthest = 0u32;
    let mut evals = 0u64;
    while evals < budget_evals {
        let mut tape = Vec::with_capacity(cfg.tick_limit as usize);
        for _ in 0..n_macros {
            let a = acts[rng.below(acts.len() as u64) as usize];
            for _ in 0..cfg.k {
                tape.push(a);
            }
        }
        // one rollout costs as many macro advances as it has macros, so the
        // budget is comparable to the archive search's.
        evals += n_macros as u64;
        let c = tmexplore::toy::simulate(track, &tape);
        let st = track.station_of(c.max_s);
        furthest = furthest.max(st);
        let r = match c.finished {
            Some(ms) => match oracle.confirm(&tape) {
                Ok(Verdict::Finish { ms: real }) => Reached::Finished { ms: real },
                _ => Reached::Stopped { cps: c.cps, station: st, ticks: c.tick },
            },
            None => Reached::Stopped { cps: c.cps, station: st, ticks: c.tick },
        };
        let _ = ms_unused(&r);
        if best.map(|b| r > b).unwrap_or(true) {
            best = Some(r);
        }
    }
    (best, furthest, evals)
}

fn ms_unused(_r: &Reached) {}

fn main() {
    let a = Args::parse();
    let cmd: Vec<&str> = a.free.iter().map(|s| s.as_str()).collect();
    match cmd.as_slice() {
        ["toy", "search"] => toy_search(&a),
        ["toy", "ablate"] => toy_ablate(&a),
        ["toy", "guard"] => toy_guard(&a),
        ["toy", "sweep"] => toy_sweep(&a),
        _ => {
            eprintln!(
                "usage:\n  tmexplore toy search [--budget N] [--seed N] [--k N] [--alphabet kb|ladder5|ladder9]\n                       [--tree true|false] [--state-blind] [--tick-limit N] [-v]\n  tmexplore toy ablate [--budget N] [--seeds N]\n  tmexplore toy guard  [--budget N] [--seed N]"
            );
            std::process::exit(2);
        }
    }
}

fn toy_search(a: &Args) {
    let track = ToyTrack::demo();
    let cfg = cfg_from(a);
    let budget: u64 = a.num("budget", 200_000u64);
    let tree = a.get("tree").map(|v| v != "false").unwrap_or(true);

    println!("tmexplore — archive search over savestates, TOY TRACK");
    println!(
        "  THE TOY PROVES THE SEARCH, NOT THE MAP. Nothing here is a claim about Trackmania."
    );
    println!(
        "  track {:.1} m, {} stations of {:.0} m, {} checkpoints, one 38 m gap",
        track.length(),
        track.n_stations(),
        track.spacing(),
        track.n_checkpoints()
    );
    println!(
        "  alphabet {:?} ({} actions), macro {} ticks, budget {} evals, seed {}, backend {}",
        cfg.alphabet,
        cfg.alphabet.len(),
        cfg.k,
        budget,
        cfg.seed,
        if tree { "savestate tree" } else { "prefix re-simulation" }
    );

    // THE DECOY TEST, printed before the first candidate.
    {
        let oracle = ToyOracle::new(&track);
        let mut sim = ToySim::new(&track, tree, cfg.tick_limit);
        let ex = Explorer::new(&track, cfg.clone());
        let _ = &mut sim;
        match ex.do_nothing_outcome(&oracle) {
            Ok(Verdict::Finish { ms }) => {
                println!(
                    "  DECOY TEST: the do-nothing tape FINISHES in {} — this objective can be maximised\n  without driving. Nothing was searched.",
                    secs(ms)
                );
                std::process::exit(1);
            }
            Ok(Verdict::Dnf { cps }) => println!(
                "  decoy test: the do-nothing tape (hands off from tick 0) gets cps {} and does not finish. OK.",
                cps
            ),
            Err(e) => println!("  decoy test UNMEASURED: {}", e),
        }
        // and the laziest DRIVEN tape, which is the more informative one
        let full = vec![Input { steer: 0, gas: true, brake: false }; cfg.tick_limit as usize];
        if let Ok(v) = oracle.confirm(&full) {
            println!("  full throttle, no steering: {:?}", v);
        }
    }

    let (best, evals, station, dt, phantoms, confirmed) =
        run_search(&track, cfg.clone(), budget, tree, 0, a.flag("v") || a.flag("verbose"));

    println!("\n--- result ---");
    println!("  {} evals in {:.1} s ({:.0} evals/s)", evals, dt, evals as f64 / dt);
    match best {
        Some(r @ Reached::Finished { .. }) => println!("  BEST, CONFIRMED BY THE PLAIN ORACLE: {}", r),
        _ => println!(
            "  no confirmed finish. Furthest station reached on our own route: {} of {}.",
            station,
            track.n_stations()
        ),
    }
    println!("  {} confirmed, {} phantoms", confirmed, phantoms);
}

fn toy_ablate(a: &Args) {
    let track = ToyTrack::demo();
    let budget: u64 = a.num("budget", 200_000u64);
    let seeds: u64 = a.num("seeds", 5u64);
    let base = cfg_from(a);

    println!("tmexplore — THE CONTROL BATTERY (toy track)");
    println!("  Question: does the archive earn its keep, or would anything have finished?");
    println!("  Every arm gets the same evaluation budget ({} macro advances) and the same", budget);
    println!("  alphabet, macro length and tick limit. {} seeds each.\n", seeds);

    struct Arm {
        name: &'static str,
        what: &'static str,
    }
    let arms = [
        Arm { name: "archive", what: "the whole-state archive (the thing being tested)" },
        Arm { name: "arc-only", what: "NEGATIVE: archive keyed on arc length alone" },
        Arm { name: "no-frontier", what: "NEGATIVE: uniform bin choice, no frontier preference" },
        Arm { name: "random", what: "NEGATIVE: no archive at all, random macro rollouts" },
    ];

    let mut rows: Vec<(String, u64, u64, u32, Option<i64>)> = Vec::new();
    for arm in arms.iter() {
        let mut fin = 0u64;
        let mut best_ms: Option<i64> = None;
        let mut furthest = 0u32;
        for s in 0..seeds {
            let mut cfg = base.clone();
            cfg.seed = 1000 + s;
            let (best, _e, station, _dt, _ph, _c) = match arm.name {
                "archive" => run_search(&track, cfg, budget, true, 0, false),
                "arc-only" => {
                    cfg.bands.state_blind = true;
                    run_search(&track, cfg, budget, true, 0, false)
                }
                "no-frontier" => {
                    cfg.policy.p_frontier = 0.0;
                    cfg.policy.visit_decay = 0.0;
                    run_search(&track, cfg, budget, true, 0, false)
                }
                "random" => {
                    let (b, st, _e) = run_random(&track, &cfg, budget, 1000 + s);
                    (b, budget, st, 0.0, 0, 0)
                }
                _ => unreachable!(),
            };
            furthest = furthest.max(station.max(best.map(|b| b.station_or(track.n_stations())).unwrap_or(0)));
            if let Some(Reached::Finished { ms }) = best {
                fin += 1;
                if best_ms.map(|b| ms < b).unwrap_or(true) {
                    best_ms = Some(ms);
                }
            }
        }
        rows.push((arm.name.into(), fin, seeds, furthest, best_ms));
        println!(
            "  {:<12} {:<52} {}/{} seeds finished, furthest station {}{}",
            arm.name,
            arm.what,
            fin,
            seeds,
            furthest,
            best_ms.map(|m| format!(", best {}", secs(m))).unwrap_or_default()
        );
    }

    println!("\n  Read it this way: the positive arm finishing is only a result if at least one");
    println!("  negative arm does not. An ablation battery where every arm finishes says the");
    println!("  track is too easy and measures nothing.");
}

fn toy_guard(a: &Args) {
    let track = ToyTrack::demo();
    let budget: u64 = a.num("budget", 120_000u64);
    let mut cfg = cfg_from(a);
    cfg.seed = a.num("seed", 1u64);

    println!("tmexplore — THE GUARD'S TWO-SIDED CONTROL (toy track)");
    println!("  A guard that never refuses is decoration; a guard that refuses everything is a");
    println!("  broken search. Both halves, same budget, same seed.\n");

    let (best_a, _, _, _, ph_a, conf_a) = run_search(&track, cfg.clone(), budget, true, 0, false);
    println!(
        "  defect OFF: {} confirmed, {} phantoms, best {}",
        conf_a,
        ph_a,
        best_a.map(|b| b.to_string()).unwrap_or("none".into())
    );

    // The negative half: reproduce the real fork defect inside the toy fork.
    // "A record already consumed cannot be un-consumed: rewriting it is a
    // silent no-op." The fork then answers honestly about a run nobody asked
    // for, and the written tape must be refused.
    let (best_b, _, _, _, ph_b, conf_b) = run_search(&track, cfg.clone(), budget, true, 4, false);
    println!(
        "  defect ON (4 ticks of every advance silently dropped): {} confirmed, {} phantoms, best {}",
        conf_b,
        ph_b,
        best_b.map(|b| b.to_string()).unwrap_or("none".into())
    );

    println!();
    if ph_a == 0 && ph_b > 0 {
        println!("  BOTH HALVES PASS: clean run banks with no refusals, defective run is caught.");
    } else if ph_a > 0 {
        println!("  FAILED, first half: the guard refused a tape in a clean run. UNMEASURED above.");
    } else {
        println!(
            "  FAILED, second half: the injected defect produced no refusal. Either the defect\n  never changed an outcome at this budget/seed (raise --budget), or the guard is blind."
        );
    }
}

/// A configuration sweep and a seed fan-out, run across the box's cores.
///
/// One process, `std::thread::scope`, one shared immutable track. Each cell is
/// an independent search with its own seed, so the only thing shared is the
/// map — exactly the shape the real fleet will have.
fn toy_sweep(a: &Args) {
    let track = ToyTrack::demo();
    let budget: u64 = a.num("budget", 1_000_000u64);
    let seeds: u64 = a.num("seeds", 8u64);
    let base = cfg_from(a);

    // (label, mutation)
    type Mut = fn(&mut Cfg);
    let grid: Vec<(&str, Mut)> = vec![
        ("default", |_c: &mut Cfg| {}),
        ("rollout60", |c: &mut Cfg| c.max_rollout = 60),
        ("sticky.85", |c: &mut Cfg| c.sticky = 0.85),
        ("k20", |c: &mut Cfg| c.k = 20),
        ("halflife600", |c: &mut Cfg| c.policy.time_halflife = 600.0),
        ("frontier8", |c: &mut Cfg| c.policy.frontier_depth = 8),
    ];

    println!("tmexplore — TOY SWEEP: {} configs x {} seeds, {} evals each", grid.len(), seeds, budget);
    println!("  THE TOY PROVES THE SEARCH, NOT THE MAP.\n");

    let mut jobs = Vec::new();
    for (gi, (name, f)) in grid.iter().enumerate() {
        for s in 0..seeds {
            let mut cfg = base.clone();
            f(&mut cfg);
            cfg.seed = 7000 + s;
            jobs.push((gi, *name, cfg));
        }
    }

    let results: Vec<(usize, Option<Reached>, u32)> = std::thread::scope(|sc| {
        let hs: Vec<_> = jobs
            .iter()
            .map(|(gi, _n, cfg)| {
                let t = &track;
                let cfg = cfg.clone();
                let gi = *gi;
                sc.spawn(move || {
                    let (best, _e, st, _dt, _p, _c) = run_search(t, cfg, budget, true, 0, false);
                    (gi, best, st)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });

    for (gi, (name, _)) in grid.iter().enumerate() {
        let mine: Vec<_> = results.iter().filter(|(g, _, _)| *g == gi).collect();
        let fin = mine.iter().filter(|(_, b, _)| matches!(b, Some(Reached::Finished { .. }))).count();
        let best_ms = mine
            .iter()
            .filter_map(|(_, b, _)| match b {
                Some(Reached::Finished { ms }) => Some(*ms),
                _ => None,
            })
            .min();
        let far = mine.iter().map(|(_, _, s)| *s).max().unwrap_or(0);
        println!(
            "  {:<14} {}/{} seeds finished   furthest station {:>3} of {}{}",
            name,
            fin,
            seeds,
            far,
            track.n_stations(),
            best_ms.map(|m| format!("   best {}", secs(m))).unwrap_or_default()
        );
    }
}
