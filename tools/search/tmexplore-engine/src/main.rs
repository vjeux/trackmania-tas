//! `tmexplore-real` — the archive search on a real campaign map.
//!
//! ```text
//! tmexplore-real run --pack B/<uid>.pack.json --route B/<uid>.route.json \
//!                    --map <uid>.Map.Gbx --template <synthesized>.Ghost.Gbx \
//!                    --server /tmp/tmoracle/server --shim libforkshim.so \
//!                    --work /tmp/cwork --threads 40 --budget 2000000
//! ```
//!
//! Every number this prints that is called a RESULT came from the dedicated
//! server re-simulating a file on disk. Everything else is labelled as what it
//! is.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tmexplore::action::Alphabet;
use tmexplore::action::Input;
use tmexplore::archive::{Bands, Policy};
use tmexplore::branch::{PlainOracle, Route};
use tmexplore::explore::Cfg;
use tmexplore::outcome::{Reached, Verdict};
use tmexplore::parallel::{self, Counters, Shared};
use tmexplore_engine::fork::{ForkBranch, ForkOpts};
use tmexplore_engine::{BRoute, EngineOracle, MapPack};

struct Args(Vec<(String, String)>, Vec<String>);
impl Args {
    fn parse() -> Args {
        let mut m = Vec::new();
        let mut free = Vec::new();
        let mut it = std::env::args().skip(1).peekable();
        while let Some(a) = it.next() {
            if let Some(k) = a.strip_prefix("--") {
                if let Some((k, v)) = k.split_once('=') {
                    m.push((k.into(), v.into()));
                } else if let Some(v) = it.peek().filter(|s| !s.starts_with('-')).cloned() {
                    it.next();
                    m.push((k.into(), v));
                } else {
                    m.push((k.into(), "true".into()));
                }
            } else if let Some(k) = a.strip_prefix('-') {
                m.push((k.into(), "true".into()));
            } else {
                free.push(a);
            }
        }
        Args(m, free)
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }
    fn req(&self, k: &str) -> String {
        match self.get(k) {
            Some(v) => v.into(),
            None => {
                eprintln!("--{} is required", k);
                std::process::exit(2)
            }
        }
    }
    fn num<T: std::str::FromStr>(&self, k: &str, d: T) -> T {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    fn flag(&self, k: &str) -> bool {
        self.get(k).map(|v| v != "false").unwrap_or(false)
    }
}

fn secs(ms: i64) -> String {
    format!("{}.{:03}", ms / 1000, (ms % 1000).abs())
}

fn main() {
    let a = Args::parse();
    if a.1.first().map(|s| s.as_str()) == Some("confirm") {
        confirm_tape(&a);
        return;
    }
    if a.1.first().map(|s| s.as_str()) == Some("template") {
        make_template(&a);
        return;
    }
    if a.1.first().map(|s| s.as_str()) != Some("run") {
        eprintln!(
            "usage: tmexplore-real run --pack P --route R --map M --template T --server S --shim SO\n           [--work DIR] [--threads N] [--budget N] [--alphabet three|keyboard|ladder5]\n           [--k N] [--rollout N] [--crude] [--forktick N] [--minutes N]"
        );
        std::process::exit(2);
    }

    let pack = MapPack::load(&PathBuf::from(a.req("pack"))).unwrap_or_else(die);
    let route = BRoute::load(
        &PathBuf::from(a.req("route")),
        a.num("default-half", 8.0f32),
    )
    .unwrap_or_else(die);
    let map = PathBuf::from(a.req("map"));
    let template = PathBuf::from(a.req("template"));
    let server = PathBuf::from(a.get("server").unwrap_or("/tmp/tmoracle/server"));
    let shim = PathBuf::from(a.req("shim"));
    let work = PathBuf::from(a.get("work").unwrap_or("/tmp/cwork"));
    let threads: usize = a.num("threads", 32usize);
    let budget: u64 = a.num("budget", 1_000_000u64);
    let minutes: u64 = a.num("minutes", 0u64);

    println!("MAP   {}  ({})", pack.name, pack.uid);
    println!(
        "  author time {}   (a number in the map file, not anybody's run)",
        secs(pack.author_ms)
    );
    println!(
        "  route {:.1} m, {} stations of {:.0} m, {} checkpoints; gates at s = {}",
        route.length(),
        route.n_stations(),
        route.spacing(),
        route.n_checkpoints(),
        route
            .gate_s
            .iter()
            .map(|s| format!("{:.0}", s))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if route.width_missing > 0 {
        println!(
            "  NOTE: {} of {} route vertices arrived with no measured corridor width; \
             {:.0} m substituted. The corridor is that much weaker a prune there.",
            route.width_missing,
            route.n_verts(),
            route.default_half()
        );
    }
    if let Some(&cp1) = route.gate_s.first() {
        println!(
            "  RUNG 1 is the gate at s = {:.0} m, which is station {} of {}.",
            cp1,
            route.station_of(cp1),
            route.n_stations()
        );
    }

    // ---- the plain oracle: the only thing that can produce a result ----
    let oracle =
        EngineOracle::new(&template, &map, &server, &work.join("oracle")).unwrap_or_else(die);
    println!(
        "\nORACLE  container holds {} ticks ({} s of race). A tape longer than that is refused,\n        not truncated.",
        oracle.capacity(),
        oracle.capacity() / 100
    );

    // ---- the identity control, before anything is searched ----
    //
    // Two-sided and cheap. The all-neutral tape must come back as a DNF the
    // server actually simulated, and the same tape must come back the same way
    // twice. A container that validated no matter what it held would pass a
    // one-sided check perfectly.
    let neutral = vec![tmexplore::action::Input::NEUTRAL; oracle.capacity()];
    let (v1, e1, uid1, d1) = oracle.confirm_echo(&neutral).unwrap_or_else(die);
    let full = vec![
        tmexplore::action::Input {
            steer: 0,
            gas: true,
            brake: false
        };
        oracle.capacity()
    ];
    let (v2, e2, _uid2, _d2) = oracle.confirm_echo(&full).unwrap_or_else(die);
    println!(
        "CONTROL do-nothing tape            -> {:?}  echo {:?}  desc {:?}",
        v1, e1, d1
    );
    println!(
        "CONTROL full-throttle-straight     -> {:?}  echo {:?}",
        v2, e2
    );
    // ASSERT IDENTITY, NOT CONTEXT. "the server answered" is not "the server
    // ran MY map": the first real run on this box answered about a file whose
    // container carried a different map's uid, and the echo check passed on it
    // because no car existed in either run.
    if uid1 != pack.uid {
        println!(
            "        FAILED: the server ran map uid {:?}, not {:?}. Every number below would be\n        about another map.",
            uid1, pack.uid
        );
        std::process::exit(1);
    }
    println!(
        "        the server confirms it ran map uid {} -- the one we asked for.",
        uid1
    );
    if e1.is_empty() && e2.is_empty() {
        println!(
            "        UNMEASURED: this server build echoes no input tape, so the writer cannot be\n        checked this way. Do not read the verdicts below as evidence that input reached the car."
        );
    } else if e1 == e2 {
        println!(
            "        FAILED: two tapes that differ in EVERY tick decoded to the same input echo.\n        The writer is not reaching the engine. Nothing below this line is worth reading."
        );
        std::process::exit(1);
    } else {
        println!(
            "        PASS, two-sided: the tapes differ, the engine's decoded echo differs, and the\n        map is ours. (A verdict comparison could not have said this: both are {:?}.)",
            v1
        );
    }

    let cfg = Cfg {
        alphabet: Alphabet::parse(a.get("alphabet").unwrap_or("three")).unwrap_or_else(die),
        k: a.num("k", 12u16),
        fanout: Some(a.num("fanout", 3usize)),
        max_rollout: a.num("rollout", 40u32),
        sticky: a.num("sticky", 0.75f64),
        policy: Policy {
            frontier_depth: a.num("frontier-depth", 4u32),
            p_frontier: a.num("p-frontier", 0.85f64),
            visit_decay: a.num("visit-decay", 0.5f64),
            time_halflife: a.num("time-halflife", 400.0f64),
        },
        bands: Bands {
            station_m: route.spacing(),
            lateral_m: a.num("band-lateral", 4.0f32),
            height_m: a.num("band-height", 6.0f32),
            speed_ms: a.num("band-speed", 6.0f32),
            yaw_deg: a.num("band-yaw", 25.0f32),
            state_blind: false,
            crude: !a.flag("full-bins"),
        },
        seed: a.num("seed", 1u64),
        tick_limit: (oracle.capacity() as u32).saturating_sub(400),
    };
    println!(
        "\nSEARCH  alphabet {:?} ({} actions), macro {} ticks, rollout <= {}, {} threads, budget {} evals",
        cfg.alphabet,
        cfg.alphabet.len(),
        cfg.k,
        cfg.max_rollout,
        threads,
        budget
    );
    println!(
        "        bins: {}",
        if cfg.bands.crude {
            "CRUDE (station, lateral, height, speed, checkpoints)"
        } else {
            "full (adds heading, wheel-contact, airtime)"
        }
    );

    let route_points: Vec<[f32; 3]> = (0..route.n_verts())
        .map(|i| {
            let s = i as f32 * route.spacing();
            let _ = s;
            [0.0, 0.0, 0.0]
        })
        .collect();
    let _ = route_points;

    let ladder = route
        .gate_ladder(&pack, a.num("gate-radius", 8.0f32))
        .unwrap_or_else(die);
    println!(
        "\nGATE LADDER  {} required gates in tour order, collected within {:.0} m:",
        ladder.gates.len(),
        ladder.radius
    );
    for (i, (s, p)) in ladder.gates.iter().enumerate() {
        println!(
            "  gate {} at s = {:>8.1} m (station {:>3})  ({:.0}, {:.0}, {:.0})",
            i,
            s,
            route.station_of(*s),
            p[0],
            p[1],
            p[2]
        );
    }
    println!(
        "  progress SATURATES at the station of the first uncollected gate: a car that cuts a\n  corner cannot score past the gate it skipped, however far it flies."
    );

    // `--start-from TAPE.tsv` restarts from one of OUR OWN banked, oracle-
    // confirmed tapes. Not a reference and not a demonstration: it is this
    // system's previous output, and building on it is regression-testing our
    // own work.
    let shared = match a.get("start-from") {
        None => Mutex::new(Shared::new(&route, &cfg)),
        Some(p) => {
            let txt = std::fs::read_to_string(p).unwrap_or_else(die);
            let mut seed: Vec<Input> = Vec::new();
            for (ln, line) in txt.lines().enumerate() {
                if line.starts_with('#') || line.starts_with("tick") {
                    continue;
                }
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() < 4 {
                    continue;
                }
                let g = |i: usize| -> i64 {
                    f[i].parse().unwrap_or_else(|_| {
                        die(format!("line {}: field {} is not a number", ln + 1, i))
                    })
                };
                seed.push(Input {
                    steer: g(1) as i8,
                    gas: g(2) != 0,
                    brake: g(3) != 0,
                });
            }
            println!(
                "\nSEEDED  from {} ({} ticks of our own confirmed output). The search extends it; it\n        does not re-explore it.",
                p,
                seed.len()
            );
            Mutex::new(Shared::seeded(&route, &cfg, seed))
        }
    };
    let counters = Counters::default();
    let started = std::time::Instant::now();
    let workdir = work.clone();

    // `--forktick` is a TICK; the shim wants an `lroundf` COUNT. The fitted line
    // is `clock = 36141 + 25.483 * race_ms` and it is per map (this one is map
    // 2's). It only has to place the checkpoint near the right instant — where
    // the server ACTUALLY stopped is probed and is what everything is labelled
    // from. Passing the tick straight through as a clock puts the fork at
    // lroundf call 60, which is during load, and the shim then reports
    // `bad handshake: ERR notfound` because the input array is not there yet.
    let forktick_t: i64 = a.num("forktick", 60i64);
    let forktick: u64 = tmsearch::forkeval::clock_for_tick(forktick_t, 0);
    println!(
        "\nFORK    checkpoint at tick {} -> lroundf clock {} (fitted line, per map; the boundary\n        the server actually stops at is PROBED and is what ticks are labelled from)",
        forktick_t, forktick
    );
    let route_pts = route_polyline(&route);
    let opts_for = |wi: usize| ForkOpts {
        work: work.join(format!("w{:03}", wi)),
        server: server.clone(),
        map: map.clone(),
        reference_ghost: template.clone(),
        shim: shim.clone(),
        checkpoint_clock: forktick,
        start_offset_ms: 0,
        route_points: route_pts.clone(),
        tail_margin: 200,
        common_from: None,
    };
    // THE REFERENCE IS THE CONTAINER'S OWN TAPE, read out of the file rather
    // than assumed. Assuming it was neutral is what produced
    // "TAPE MISMATCH: 12000 of 12000 ticks differ" on the first run with a
    // varied steer channel — the control doing exactly its job.
    let reference = oracle.template_inputs();
    {
        let d = {
            let mut s = reference.steer.clone();
            s.sort_unstable();
            s.dedup();
            s.len()
        };
        println!(
            "\nREFERENCE  the container's own tape: {} ticks, {} distinct steer values.",
            reference.len(),
            d
        );
        if d < 20 {
            println!(
                "           FEWER THAN 20 DISTINCT VALUES. The fork shim locates the input array by\n           searching for this sequence as f32 at stride 32; a near-constant channel matches\n           a stretch of zeroes that is not the array, and the locate succeeds on the WRONG\n           memory. Rebuild the container with `tmexplore-real template`."
            );
        }
    }

    // THE TICK FRAME. The search's tick 0 is the fork server's own probed
    // boundary, so the oracle must lay a candidate down from there. Probed from
    // a real worker rather than computed, because where a server stops is a
    // property of that server and not of the fitted clock line.
    // THE BOUNDARY IS NOT STABLE, SO IT IS MEASURED SEVERAL TIMES AND THE MAX
    // IS TAKEN.
    //
    // Measured on this map, same container, same code: three runs probed 153,
    // 153 and **152**. One tick. And a one-tick shift of the whole tape turned
    // a tape confirmed at `cps 3` into `cps 0` — every checkpoint lost — which
    // is not a rounding error, it is a different run.
    //
    // This is the same shape as the phantom the search layer already paid for:
    // where a server stops is a property of THAT SERVER, so anything derived
    // from one server's stop is per-server. The fix there was the same as the
    // fix here: publish the MAXIMUM over the fleet and have everybody use it.
    let mut boundary = 0usize;
    for i in 0..3 {
        let o = opts_for(9990 + i);
        match ForkBranch::start(&o, reference.clone()) {
            Ok(b) => boundary = boundary.max(b.from),
            Err(e) => println!("boundary probe {} failed: {}", i, e),
        }
    }
    {
        let o = opts_for(9998);
        match ForkBranch::start(&o, reference.clone()) {
            Ok(b) => {
                boundary = boundary.max(b.from);
                oracle.set_prefix_ticks(boundary as u64);
                println!(
                    "\nTICK FRAME  boundary probed 4x, MAX = tick {} (this probe said {}). The search's tick 0\n            is file tick {}. The maximum is used because where a server stops is a property\n            of that server: probes here differ by a tick between runs, and a one-tick shift\n            of a whole tape turned a confirmed cps 3 into cps 0.",
                    boundary, b.from, boundary
                );
            }
            Err(e) => {
                println!("could not probe the resume boundary: {}", e);
                std::process::exit(1);
            }
        }
    }

    let deadline = if minutes > 0 {
        Some(started + std::time::Duration::from_secs(minutes * 60))
    } else {
        None
    };
    let cnt = &counters;
    std::thread::scope(|sc| {
        sc.spawn(|| {
            // The progress reporter, and THE DEAD-INSTRUMENT STOP.
            //
            // My own stall rule — "report if the furthest station has not
            // improved across 2 M evals" — cannot fire at zero evals per
            // second. Three runs produced **0 evals in 440 s with 0 errors**
            // and the rule watched them go by, because it measures
            // progress-per-eval while the thing that broke was evals-per-
            // second. Nothing was failing, so nothing was being attempted, and
            // an instrument that fails toward silence produces nothing to be
            // suspicious of.
            //
            // So: zero evals for five consecutive minutes is a HARD STOP, not
            // a warning line. Both rules stay, because they catch different
            // failures — that is the point of having two.
            let mut zero_since: Option<std::time::Instant> = None;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(20));
                if cnt.stop.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(d) = deadline {
                    if std::time::Instant::now() >= d {
                        cnt.stop.store(true, Ordering::Relaxed);
                        return;
                    }
                }
                let s = shared.lock().unwrap();
                let e = cnt.evals.load(Ordering::Relaxed);
                println!(
                    "[{:>6.0}s] {:>9} evals ({:.0}/s)  {} bins  furthest station {} of {}  oracle calls {}  errors {}",
                    started.elapsed().as_secs_f64(),
                    e,
                    e as f64 / started.elapsed().as_secs_f64().max(0.001),
                    s.archive.len(),
                    s.archive.max_station,
                    route.n_stations(),
                    cnt.oracle_calls.load(Ordering::Relaxed),
                    cnt.errors.load(Ordering::Relaxed),
                );
                drop(s);
                // ---- THE PERIODIC CONFIRMATION ----
                //
                // Independent of the per-rollout trigger, and it exists
                // because that trigger reported `oracle calls 0` while the
                // fork claimed station 74 of 97. Whatever is wrong with the
                // trigger, an unconfirmed archive is scored on fiction: 0 of
                // 312 fork-reported finishes once survived re-simulation of
                // the byte-identical tape.
                //
                // So every 20 s the best entry in the archive is written out as
                // a tape and handed to the plain oracle, and what the oracle
                // says is what gets reported. This costs one server launch —
                // 2.68 s — against thousands of evaluations.
                let cand = {
                    let s = shared.lock().unwrap();
                    let mut best: Option<(u32, u32, u32, Vec<Input>)> = None;
                    for (k, e) in s.archive.iter() {
                        let key = (e.state.cps, k.station, u32::MAX - e.ticks);
                        let better = best
                            .as_ref()
                            .map(|(c, st, t, _)| key > (*c, *st, u32::MAX - *t))
                            .unwrap_or(true);
                        if better {
                            best = Some((e.state.cps, k.station, e.ticks, s.trunk.inputs_to(e.node, e.ticks)));
                        }
                    }
                    best
                };
                if let Some((fcps, station, t, tape)) = cand {
                    cnt.oracle_calls.fetch_add(1, Ordering::Relaxed);
                    match oracle.confirm(&tape) {
                        Ok(v) => {
                            let r = match v {
                                Verdict::Finish { ms } => Reached::Finished { ms },
                                Verdict::Dnf { cps } => Reached::Stopped { cps, station, ticks: t },
                            };
                            println!(
                                "        PLAIN ORACLE on the best tape ({} ticks, fork says station {} \
                                 with {} gates): {:?}",
                                t, station, fcps, v
                            );
                            let mut s = shared.lock().unwrap();
                            s.confirmations.push((station, t, v));
                            if s.best.as_ref().map(|(b, _)| r > *b).unwrap_or(true) {
                                let out = workdir.join(format!(
                                    "confirmed_st{}_cps{}_{}.tape.tsv",
                                    station,
                                    match v {
                                        Verdict::Dnf { cps } => cps,
                                        Verdict::Finish { .. } => 999,
                                    },
                                    t
                                ));
                                let mut txt = format!(
                                    "# frame\t{}\n# map\t{}\ntick\tsteer\tgas\tbrake\n",
                                    oracle.prefix_ticks(),
                                    pack.uid
                                );
                                for (i, x) in tape.iter().enumerate() {
                                    txt.push_str(&format!(
                                        "{}\t{}\t{}\t{}\n",
                                        i, x.steer, x.gas as u8, x.brake as u8
                                    ));
                                }
                                let _ = std::fs::write(&out, txt);
                                println!("        *** NEW BEST CONFIRMED: {}  -> {}", r, out.display());
                                s.best = Some((r, tape));
                            }
                        }
                        Err(e) => println!("        the oracle refused the best tape: {}", e),
                    }
                }
                if e == 0 {
                    let t0 = *zero_since.get_or_insert_with(std::time::Instant::now);
                    if t0.elapsed().as_secs() >= 300 {
                        println!(
                            "\nDEAD INSTRUMENT: zero evaluations for {:.0} s. Nothing is failing and\n\
                             nothing is being attempted, which is the shape of a worker that never\n\
                             started rather than a search that cannot find anything. Stopping.",
                            t0.elapsed().as_secs_f64()
                        );
                        cnt.stop.store(true, Ordering::Relaxed);
                        return;
                    }
                } else {
                    zero_since = None;
                }
                if e >= budget {
                    return;
                }
            }
        });

        parallel::run(
            &route,
            &cfg,
            threads,
            budget,
            |wi| {
                let mut o = opts_for(wi);
                o.common_from = Some(boundary);
                let mut b = ForkBranch::start(&o, reference.clone())?;
                // Every worker resolves its own validator-owned chain. A stale
                // callback offset or field hop fails closed; there is no shared
                // state offset and no scanner fallback.
                // WHERE DOES THE CAR START? Asked before anything else, because a
                // run that begins in the wrong place passes every other check.
                match b.start_position_control(pack.spawn, a.num("spawn-tol", 40.0f32)) {
                    Ok(m) => println!("worker {:>3} start OK: {}", wi, m),
                    Err(e) => return Err(format!("worker {}: {}", wi, e)),
                }
                match b.self_check() {
                    Ok(m) => println!("worker {:>3} ready: {}", wi, m),
                    Err(e) => return Err(format!("worker {} self-check FAILED: {}", wi, e)),
                }
                Ok(b)
            },
            &oracle,
            &ladder,
            &shared,
            &counters,
            &|m| println!("{}", m),
        );
        counters.stop.store(true, Ordering::Relaxed);
    });

    println!("\n================ REPORT ================");
    let s = shared.lock().unwrap();
    print!("{}", s.report(&route));
    println!(
        "{} evals in {:.0} s, {} plain-oracle answers, {} errors",
        counters.evals.load(Ordering::Relaxed),
        started.elapsed().as_secs_f64(),
        counters.oracle_calls.load(Ordering::Relaxed),
        counters.errors.load(Ordering::Relaxed)
    );
    if let Some((r, tape)) = &s.best {
        let out = work.join("best.tape.tsv");
        let mut txt = String::from("tick\tsteer\tgas\tbrake\n");
        for (i, t) in tape.iter().enumerate() {
            txt.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                i, t.steer, t.gas as u8, t.brake as u8
            ));
        }
        let _ = std::fs::write(&out, txt);
        println!(
            "best confirmed outcome {} — its tape is at {}",
            r,
            out.display()
        );
    }
}

fn route_polyline(r: &BRoute) -> Vec<[f32; 3]> {
    r.points().to_vec()
}

fn die<T, E: std::fmt::Display>(e: E) -> T {
    eprintln!("{}", e);
    std::process::exit(1)
}

/// Build a container this search can actually use.
///
/// Two properties that A's rung-0 container does not have, both measured by
/// agent D, and both of which fail in ways that read as something else:
///
/// 1. **The validator simulates until the DECLARED TIME, not until the tape
///    ends.** Same map, same generator, one field changed: declared 0 stops at
///    race 2.500; declared 30.000 runs to race 29.500. A 600-tick and a
///    3000-tick tape at declared 0 give identical totals, so it is the field
///    and not the archive length. A container declaring 0 therefore stops
///    2.5 s in and returns a DNF that reads as bad driving.
///
/// 2. **A constant steer channel makes the fork shim lock onto the wrong
///    memory.** The shim finds the input array by searching the address space
///    for the steer sequence as f32 at stride 32; an all-zero channel matches a
///    stretch of zeroes that is not the array. D's identity control caught it
///    as `TAPE MISMATCH: 3000 of 3000 ticks differ`, which is the right
///    outcome — a locate that matches the wrong thing is far worse than one
///    that fails.
///
/// So the reference tape carries 25 distinct steer values and the throttle
/// stays off: the car does not move, the run's length is the declared time's
/// business, and the shim has something to lock onto.
fn make_template(a: &Args) {
    let map = PathBuf::from(a.req("map"));
    let out = PathBuf::from(a.req("out"));
    let ticks: usize = a.num("ticks", 12000usize);
    let declare_ms: u32 = a.num("declare", 100_000u32);
    let ncp: usize = a.num("cps", 3usize);

    let mut inputs = Vec::with_capacity(ticks);
    for t in 0..ticks {
        // 25 distinct values in [-12, 12], in an order with no short period.
        let v = ((t as u64 * 7919 + 13) % 25) as i8 - 12;
        // GAS ON. The car must MOVE: `locate_blind` finds the vehicle state by
        // its velocity consistency, and a parked car is indistinguishable from
        // any other constant region of memory. With the throttle off the
        // locator simply never returns — measured here as two workers sitting
        // in locate for 440 s at a load average of 1.8, which reads as a hang
        // and is actually a car that never moved.
        //
        // The steer pattern cycles all 25 values evenly, so its mean is zero
        // and the car goes essentially straight.
        inputs.push(tmauto::tape::Input {
            steer: v,
            gas: true,
            brake: false,
            respawn: false,
        });
    }
    let distinct = {
        let mut s: Vec<i8> = inputs.iter().map(|i| i.steer).collect();
        s.sort_unstable();
        s.dedup();
        s.len()
    };

    let mut meta = tmauto::synth::meta_for_map(&map).unwrap_or_else(die);
    let cps: Vec<i32> = (1..=ncp)
        .map(|i| (declare_ms as i32 / (ncp as i32 + 1)) * i as i32)
        .chain(std::iter::once(declare_ms as i32))
        .collect();
    meta.set_declared(declare_ms, cps.clone());
    let bytes = tmauto::synth::synthesize(&inputs, &meta, &tmauto::synth::ChunkSet::ALL);
    std::fs::write(&out, &bytes).unwrap_or_else(die);
    println!(
        "wrote {} ({} bytes)\n  map uid      {}\n  ticks        {}\n  declared     {} ({} s) -- the validator simulates until THIS, not until the tape ends\n  declared cps {:?}\n  steer channel {} distinct values -- the fork shim locks onto the array by this sequence,\n                and a constant channel matches a stretch of zeroes that is not the array",
        out.display(),
        bytes.len(),
        meta.map_uid,
        ticks,
        secs(declare_ms as i64),
        declare_ms / 1000,
        cps,
        distinct
    );
    if distinct < 20 {
        println!("  WARNING: fewer than 20 distinct steer values; the locate may not lock on.");
    }
}

/// Re-simulate a banked tape, from outside the search that produced it.
///
/// The search confirms its own results as it goes, but a claim checked only by
/// the process that made it is checked by an instrument that shares every
/// assumption with the thing under test. This is the same tape, a separate
/// invocation, a fresh server, and the frame stated on the command line rather
/// than inherited.
fn confirm_tape(a: &Args) {
    let map = PathBuf::from(a.req("map"));
    let template = PathBuf::from(a.req("template"));
    let server = PathBuf::from(a.get("server").unwrap_or("/tmp/tmoracle/server"));
    let work = PathBuf::from(a.get("work").unwrap_or("/tmp/cconfirm"));
    let prefix: u64 = a.num("prefix", 153u64);
    let reps: usize = a.num("reps", 2usize);

    let txt = std::fs::read_to_string(a.req("tape")).unwrap_or_else(die);
    let mut tape: Vec<Input> = Vec::new();
    // A TAPE IS ONLY MEANINGFUL WITH ITS FRAME. The file carries the boundary
    // it was written from; `--prefix` overrides it, and a file with neither is
    // refused rather than replayed against a guess.
    let mut file_frame: Option<u64> = None;
    for line in txt.lines() {
        if let Some(r) = line.strip_prefix("# frame") {
            file_frame = r.trim().parse().ok();
        }
        if line.starts_with('#') || line.starts_with("tick") {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        tape.push(Input {
            steer: f[1].parse().unwrap_or(0),
            gas: f[2] != "0",
            brake: f[3] != "0",
        });
    }
    let oracle = EngineOracle::new(&template, &map, &server, &work).unwrap_or_else(die);
    let frame = match (a.get("prefix"), file_frame) {
        (Some(_), _) => prefix,
        (None, Some(f)) => {
            println!("frame {} taken from the tape file", f);
            f
        }
        (None, None) => {
            eprintln!(
                "this tape carries no frame and none was given. A tape replayed at the wrong\nboundary is a different run: one tick of shift turned a confirmed cps 3 into cps 0.\nPass --prefix N."
            );
            std::process::exit(2);
        }
    };
    oracle.set_prefix_ticks(frame);
    println!(
        "tape {} ticks, written into a {}-tick container from file tick {}",
        tape.len(),
        oracle.capacity(),
        prefix
    );
    // A CONTROL IN THE SAME BATCH, because a verdict with nothing beside it
    // says nothing about the instrument: the container's own tape must come
    // back as something, and it must not come back as this tape's answer.
    let bare: Vec<Input> = Vec::new();
    match oracle.confirm_echo(&bare) {
        Ok((v, e, uid, d)) => println!(
            "control (container's own tape): {:?}  desc {:?}  uid {}  echo {:?}",
            v, d, uid, e
        ),
        Err(e) => println!("control UNMEASURED: {}", e),
    }
    for i in 0..reps {
        match oracle.confirm_echo(&tape) {
            Ok((v, e, uid, d)) => println!(
                "run {}: {:?}   desc {:?}   map {}   echo {:?}",
                i + 1,
                v,
                d,
                uid,
                e
            ),
            Err(e) => println!("run {}: REFUSED -- {}", i + 1, e),
        }
    }
}
