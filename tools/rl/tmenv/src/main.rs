//! `tmenv` — the environment, and the checks that say whether to believe it.
//!
//! ```text
//!   tmenv setup        write the reference container this box forks on
//!   tmenv track        what the map's geometry says, with its controls
//!   tmenv known-answer THE PROOF: a scripted policy, re-simulated two ways
//!   tmenv bench        rollout throughput, in env-steps/s, at parallelism
//!   tmenv rollout      drive one episode and say what happened
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;
use tmenv::action::ActionSpace;
use tmenv::core::{CoreCfg, Done};
use tmenv::forkenv::{ForkEnv, Rig};
use tmenv::{control, load_track};
use forkoracle as _fo;

fn flag(a: &[String], k: &str) -> Option<String> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned()
}

fn num<T: std::str::FromStr>(a: &[String], k: &str, d: T) -> T {
    flag(a, k).and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn has(a: &[String], k: &str) -> bool {
    a.iter().any(|x| x == k)
}

struct Paths {
    server: PathBuf,
    map: PathBuf,
    shim: PathBuf,
    work: PathBuf,
    reference: PathBuf,
    /// `--geom geom.json`: run on a TrackGeom instead of the cartographer.
    geom: Option<PathBuf>,
}

fn paths(a: &[String]) -> Paths {
    let server = PathBuf::from(
        flag(a, "--server")
            .or_else(|| std::env::var("TM_SERVER").ok())
            .unwrap_or_else(|| "/tmp/tmoracle/server".into()),
    );
    let shim = PathBuf::from(
        flag(a, "--shim")
            .or_else(|| std::env::var("FK_SHIM").ok())
            .unwrap_or_else(|| "/tmp/tmtas/tools/search/target/release/libforkshim.so".into()),
    );
    let work = PathBuf::from(
        flag(a, "--work").unwrap_or_else(|| format!("/tmp/tmenv/work-{}", std::process::id())),
    );
    Paths {
        server,
        map: PathBuf::from(flag(a, "--map").unwrap_or_default()),
        shim,
        work,
        reference: PathBuf::from(flag(a, "--ref").unwrap_or_default()),
        geom: flag(a, "--geom").map(PathBuf::from),
    }
}

fn die(e: String) -> ! {
    eprintln!("tmenv: {e}");
    std::process::exit(1)
}

fn secs(ms: i64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(|s| s.as_str()) {
        Some("setup") => setup(&a),
        Some("track") => track_cmd(&a),
        Some("known-answer") => known_answer(&a),
        Some("bench") => bench(&a),
        Some("rollout") => rollout(&a),
        Some("trace") => trace_cmd(&a),
        Some("checktraj") => checktraj(&a),
        Some("seek") => seek(&a),
        Some("verdict") => verdict_cmd(&a),
        Some("spawn") => spawn_cmd(&a),
        Some("calibrate") => calibrate(&a),
        Some("startmap") => startmap(&a),
        Some("fieldsweep") => fieldsweep(&a),
        Some("reset-control") => reset_control(&a),
        Some("template-control") => template_control(&a),
        Some("accept") => accept(&a),
        Some("from-template") => from_template(&a),
        Some("probe-scan") => probe_scan(&a),
        Some("geom-export") => geom_export(&a),
        _ => {
            eprintln!(
                "tmenv -- the RL environment over our own instrument\n\
                 \n\
                   tmenv setup --map M.Map.Gbx [--ticks N] [--out DIR]\n\
                         Write the reference container the fork server runs. Synthesized from\n\
                 nothing (no ghost), then given a VARIED steer channel, because a constant\n\
                 channel gives the shim's input-array search nothing to lock onto.\n\
                 \n\
                   tmenv track --map M.Map.Gbx\n\
                        The route, the gates and the station controls.\n\
                 \n\
                   tmenv known-answer --map M --ref R.Ghost.Gbx [--k N] [--steps N]\n\
                        THE PROOF that the env is not returning fiction. Drives a scripted\n\
                 policy, writes the tape, and re-simulates it two independent ways --- with\n\
                 both negative halves.\n\
                 \n\
                   tmenv bench --map M --ref R [--workers N] [--k N] [--steps N]\n\
                        Rollout throughput in env-steps/s at parallelism.\n\
                 \n\
                   tmenv rollout --map M --ref R [--k N] [--policy forward|random] [--seed N]\n\
                 \n\
                 Common: --server DIR ($TM_SERVER)  --shim SO ($FK_SHIM)  --work DIR"
            );
            std::process::exit(2)
        }
    }
}

// ------------------------------------------------------------------ setup

fn setup(a: &[String]) {
    let p = paths(a);
    let ticks: usize = num(a, "--ticks", 4000);
    let out = PathBuf::from(flag(a, "--out").unwrap_or_else(|| "/tmp/tmenv".into()));
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(e.to_string()));
    if !p.map.exists() {
        die(format!("no map at {}", p.map.display()));
    }

    // The declared time bounds the SIMULATION, not the tape length: a container
    // declaring 0 stops after ~2.500 s whatever its tape says, and the DNF that
    // produces is indistinguishable from a car that drove off the track. It is
    // the first of this project's paid-for traps and it costs an afternoon.
    let declared: u32 = num(a, "--declared", (ticks as u32) * 10);
    let flat: Vec<tmauto::Input> =
        (0..ticks).map(|_| tmauto::Input::new(0, true, false)).collect();
    let mut meta = tmauto::synth::meta_for_map(&p.map).unwrap_or_else(|e| die(e));
    // `set_declared`, NOT `meta.declared_ms = ...`. The declared time and the
    // walltime pair are one fact and the server checks them against each other;
    // moving one alone gets `wrong simu unexcepted walltime (0s)`, which is a
    // REFUSAL (`simulated() == false`, verdict `None`) and not a DNF. Those two
    // look identical if you collapse them and they mean opposite things.
    let cps: Vec<i32> = flag(a, "--cps")
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_default();
    meta.set_declared(declared, cps);
    let raw = out.join("rung0.Ghost.Gbx");
    let bytes = tmauto::synth::synthesize(
        &tmauto::synth::pad_to(&flat, ticks),
        &meta,
        &tmauto::synth::ChunkSet::ALL,
    );
    std::fs::write(&raw, &bytes).unwrap_or_else(|e| die(e.to_string()));

    // Set the declared time, then vary the steer channel.
    let t = fk::tape::Tape::load(&raw.to_string_lossy()).unwrap_or_else(|e| die(e));
    let varied = out.join("reference.Ghost.Gbx");
    fk::cmd::tree::tape_vary(&t, &varied, 20260824, 12).unwrap_or_else(|e| die(e));

    println!("map        {}", p.map.display());
    println!("ticks      {} ({} ms of tape, declared {})", ticks, ticks * 10, declared);
    println!("rung0      {}", raw.display());
    println!("reference  {}", varied.display());

    let vt = fk::tape::Tape::load(&varied.to_string_lossy()).unwrap_or_else(|e| die(e));
    vt.codec_is_lossless().unwrap_or_else(|e| die(e));
    println!("codec      lossless (the decode/encode round trip is the identity)");
    println!(
        "declared   {:?}   -- the validator simulates to THIS, not to the end of the tape",
        vt.declared_ms
    );

    // The control that this container is worth forking on at all: hand it to
    // the plain oracle and see the server simulate it.
    let batch = tmauto::oracle::validate_raw(
        &p.server,
        &[varied.clone()],
        tmauto::oracle::Maps::One(&p.map),
        "setup",
    )
    .unwrap_or_else(|e| die(e));
    for ans in &batch.answers {
        println!("oracle     {:?}", ans.verdict());
        println!("  simulated {}  is_valid {:?}  cps {:?}  time {:?}", ans.simulated(), ans.is_valid, ans.cps, ans.time_ms);
        println!("  desc     {}", ans.desc.trim());
    }
    if batch.answers.is_empty() {
        println!("oracle     NO ANSWER AT ALL -- the server printed nothing for this file");
    }
    if !batch.err.trim().is_empty() {
        println!("stderr     {}", batch.err.trim());
    }
}

// ------------------------------------------------------------------ track

fn track_cmd(a: &[String]) {
    let p = paths(a);
    let t = load_track(&p.server, &p.map).unwrap_or_else(|e| die(e));
    println!("map          {}", t.pack.name);
    println!("uid          {}", t.pack.uid);
    match t.author_s() {
        Some(s) => println!("author time  {:.3}", s),
        None => println!("author time  UNKNOWN (the map header carried no <times>)"),
    }
    println!("route        {:.1} m, {} stations", t.length(), t.route.stations.len());
    println!("gates        {} (finish last)", t.n_gates());
    for i in 0..t.n_gates() {
        let g = t.gate_pos[i];
        println!(
            "  gate {}  s = {:8.1} m   station {:3}   road point ({:8.1}, {:6.1}, {:8.1})",
            i,
            t.gate_s[i],
            (t.gate_s[i] / 20.0).round() as i32,
            g[0],
            g[1],
            g[2]
        );
    }
    if let Some(au) = t.author_s() {
        println!("implied      {:.1} m/s over the author time", t.length() as f64 / au);
    }
}

// ----------------------------------------------------------- known-answer

fn drive(env: &mut ForkEnv, actions: &dyn Fn(usize) -> usize, max_steps: usize) -> (usize, f32, Option<Done>) {
    env.reset().unwrap_or_else(|e| die(e));
    let mut total = 0.0f32;
    let mut n = 0usize;
    let mut done = None;
    while n < max_steps {
        let (_o, r, d, _i) = match env.step(actions(n)) {
            Ok(v) => v,
            Err(e) => die(e),
        };
        total += r;
        n += 1;
        if let Some(d) = d {
            done = Some(d);
            break;
        }
    }
    (n, total, done)
}

/// The track, with the tour solved from where the engine ACTUALLY starts the car.
///
/// Not the map's own Spawn waypoint: on Summer 2026 - 01 they are 389 m apart,
/// and a route from the wrong origin has the wrong arc length, the wrong leg
/// order and the wrong first gate -- all of it silently, because the geometry is
/// self-consistent either way and only the reward is nonsense.
fn measured_track(p: &Paths) -> Result<tmenv::Track, String> {
    // `--geom geom.json` (the DATA arm's TrackGeom: field-median line, WR line,
    // router output) takes precedence over the cartographer's pack/route, which
    // on Summer 2026 - 01 runs on another road for its first ~150 m
    // (RL-agentG §5.3). The geometry is the env's contract with the dataset:
    // BC and RL must see the same route.
    if let Some(g) = &p.geom {
        return tmenv::Track::load_geom_json(g);
    }
    tmenv::load_track_measured(&p.server, &p.map, &p.shim, &p.work.join("spawnfix"), &p.reference)
}

fn build_env(p: &Paths, a: &[String], work: &Path) -> (ForkEnv, Rig, fk::tape::Tape) {
    let track = std::sync::Arc::new(measured_track(p).unwrap_or_else(|e| die(e)));
    build_env_shared(p, a, work, track)
}

/// The env builder, with the track supplied.
///
/// Split out so a fleet builds the map's geometry ONCE. It is read-only, it is
/// identical for every worker, and building it forty times is forty pak stores
/// and ~4.7 s each — setup being measured as if it were stepping.
/// Is this reset state physically coherent, independent of whether it is the
/// answer we want?
///
/// A layout on the wrong address reads out a smooth-looking nothing. Measured,
/// once: position (1085.23, 0.69, 5.33) at race 1155135.336 s and 496.01 m/s.
/// These clauses are about internal consistency -- a unit quaternion, a speed a
/// car can reach, a race clock that matches the tick the engine says it
/// stopped at -- and deliberately NOT about agreeing with the measured start,
/// which is the acceptance control's job and would be circular here.

fn build_env_shared(
    p: &Paths,
    a: &[String],
    work: &Path,
    track: std::sync::Arc<tmenv::Track>,
) -> (ForkEnv, Rig, fk::tape::Tape) {
    let cfg = CoreCfg {
        k_ticks: num(a, "--k", 10),
        max_ticks: num(a, "--max-ticks", 3000),
        ..Default::default()
    };
    let mut root = tmenv::forkenv::RootCfg { verbose: !has(a, "--quiet"), ..Default::default() };
    // The env must reach the START, not merely a coherent state. Without this
    // the ladder is non-deterministic -- measured: tick 0 on one run and tick 66
    // on the next -- and a run rooted at tick 66 is a different environment
    // from the one the acceptance control certified.
    if !has(a, "--no-require-start") && flag(a, "--root-clock").is_none() {
        match tmenv::measured_spawn(&p.server, &p.map, &p.shim, &work.join("spawnfix"), &p.reference) {
            Ok(s) => root.require_start = Some((s, num(a, "--tol", 6.0f32), num(a, "--vmax", 4.0f64))),
            Err(e) => die(format!("the start is UNMEASURED, so the env cannot be certified: {e}")),
        }
    }
    if let Some(c) = flag(a, "--root-clock").and_then(|s| s.parse::<u64>().ok()) {
        root.clock = c;
    }
    root.max_root_floor_ms = num(a, "--max-root-floor-ms", root.max_root_floor_ms);
    root.max_root_tries = num(a, "--max-root-tries", root.max_root_tries);
    tmenv::forkenv::build_at_start(
        &p.server, &p.map, &p.shim, work, &p.reference, track,
        ActionSpace::default(), cfg, &root,
    )
    .unwrap_or_else(|e| die(e))
}

fn build_env_with(
    p: &Paths,
    a: &[String],
    work: &Path,
    track: std::sync::Arc<tmenv::Track>,
) -> ForkEnv {
    let (env, rig, tape) = build_env_shared(p, a, work, track);
    // The rig owns the Engine, whose Drop would delete the work directory, and
    // the Tape; both must outlive the env. Leaking them for the lifetime of a
    // benchmark worker is deliberate and bounded.
    std::mem::forget(rig);
    std::mem::forget(tape);
    env
}

fn known_answer(a: &[String]) {
    let p = paths(a);
    let max_steps: usize = num(a, "--steps", 300);
    let perturb_at: usize = num(a, "--perturb-at", 12);
    let tol: f64 = num(a, "--tol", 0.02);
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));

    println!("# tmenv known-answer");
    println!("#");
    println!("# The claim under test: the environment's step/observation/reward path reports");
    println!("# the run the engine actually performs. An env that silently returns garbage");
    println!("# trains a policy on fiction and nothing downstream can see it.");
    println!();

    let w1 = p.work.join("arm-a");
    let (mut env, _rig, tape) = build_env(&p, a, &w1);
    println!("obs dim      {}", env.obs_dim());
    println!("actions      {}", env.n_actions());
    let fwd = ActionSpace::default().forward();
    println!("scripted     action {} = full throttle, steer 0", fwd);
    println!();

    // ---- arm A: the scripted run.
    let t0 = Instant::now();
    let (steps, reward, done) = drive(&mut env, &|_| fwd, max_steps);
    let wall = t0.elapsed().as_secs_f64();
    let rec = env.rollout_record();
    println!("--- the scripted run");
    println!(
        "steps {}  ticks driven {}  done {:?}  return {:.3}  wall {:.3} s  ({:.0} env-steps/s, 1 worker)",
        steps,
        rec.spans.iter().map(|s| s.k).sum::<usize>(),
        done,
        reward,
        wall,
        steps as f64 / wall
    );
    println!(
        "gates {}/{}  best s {:.1} m of {:.1}  gap ticks {}  overlap ticks {}",
        env.core.gates_hit(),
        env.core.track.n_gates(),
        env.core.best_s(),
        env.core.track.length(),
        rec.gap_ticks,
        rec.overlap_ticks
    );

    let cand = p.work.join("scripted.Ghost.Gbx");
    let (s, g, b) = env.faithful_tape(&tape);
    tape.write_candidate(&s, &g, &b, &cand).unwrap_or_else(|e| die(e));
    let env_trace = rec.trace.clone();
    drop(env);

    // ---- CONTROL A: the plain oracle, which shares no code with the fork path.
    println!();
    println!("--- CONTROL A: the plain oracle on the written tape");
    println!("    (a separate dedicated server validating the file: no shim, no fork, no");
    println!("     memory readout, no `branch`. It cannot agree with the env by sharing a bug)");
    let batch = tmauto::oracle::validate_raw(
        &p.server,
        &[cand.clone()],
        tmauto::oracle::Maps::One(&p.map),
        "ka",
    )
    .unwrap_or_else(|e| die(e));
    let ans = batch.answers.first().unwrap_or_else(|| die("the oracle returned no answer".into()));
    let ov = ans.verdict();
    println!("oracle       {:?}", ov);
    let oracle_cps = match ov {
        Some(tmauto::Verdict::Dnf { cps }) => cps as usize,
        Some(tmauto::Verdict::Finish { .. }) => 0,
        None => usize::MAX,
    };
    let env_cps = env_gate_report(&ov, env_gates_from(&rec));
    println!("env said     {}", env_cps);
    // A1 is load-bearing: did the server SIMULATE our tape, or refuse it? A
    // refusal and a DNF look identical if you collapse them and they mean
    // opposite things.
    let a1 = ans.simulated();
    println!("A1 the server SIMULATED the tape (not a refusal)  : {}", if a1 { "PASS" } else { "FAIL" });
    let a_ok = match ov {
        Some(tmauto::Verdict::Finish { ms }) => {
            println!("oracle time  {}", secs(ms as i64));
            match rec.finish_ms {
                Some(e) => {
                    println!("env time     {} (at the tick the finish gate's 12 m sphere was entered)", secs(e));
                    (e - ms as i64).abs() < 400
                }
                None => false,
            }
        }
        Some(tmauto::Verdict::Dnf { .. }) => oracle_cps == env_gates_from(&rec),
        None => false,
    };
    // A2 is the CALIBRATION of the geometric gate detector, and it is reported
    // rather than gated -- see RL.md 5.1. The detector does not agree with the
    // oracle on this map in either direction, which is why CoreCfg::gate_cap is
    // off and the corridor does that job instead. Gating on it would say the
    // environment is broken when what is uncalibrated is a shaping heuristic
    // the reward is not using; hiding it would be worse.
    println!(
        "A2 the gate detector agrees with the oracle          : {}   [CALIBRATION, not a gate -- RL.md 5.1]",
        if a_ok { "agrees" } else { "DISAGREES" }
    );
    let a_ok = a1;

    // ---- CONTROL B: the trajectory, re-simulated in one piece.
    println!();
    println!("--- CONTROL B: the same tape re-simulated in ONE child, no per-step forking");
    println!("    (this shares the readout with the env, so it certifies the STEPPING, not");
    println!("     the readout. Saying which axis a check covers is why there are two)");
    let ticks: u64 = rec.spans.last().map(|s| (s.from + s.k) as u64).unwrap_or(0) + 300;
    let flat = control::flat_trace(
        &p.server,
        &p.map,
        &p.shim,
        &p.work.join("flat"),
        &cand,
        ticks,
    )
    .unwrap_or_else(|e| die(e));
    let cmp = control::compare(&env_trace, &flat, tol);
    println!("{}", cmp);
    let b_ok = cmp.same(tol);
    println!(
        "CONTROL B    {}",
        if b_ok {
            "PASS -- the stepped trajectory IS the flat simulation of the tape it wrote"
        } else {
            "FAIL"
        }
    );

    // ---- THE NEGATIVE HALVES.
    println!();
    println!("--- THE NEGATIVE HALVES");
    println!("    A comparison that always says 'same' passes CONTROL B on a broken rig, and");
    println!("    one verdict on one tape says nothing about discrimination. So: change one");
    println!("    macro, and require BOTH that the run differs from the first AND that it");
    println!("    still agrees with its own re-simulation.");
    let w2 = p.work.join("arm-b");
    let (mut env2, _r2, tape2) = build_env(&p, a, &w2);
    let hard_left = 0usize; // steer rung -127, full gas
    let (steps2, _r, done2) = drive(
        &mut env2,
        &|i| if i == perturb_at { hard_left } else { fwd },
        max_steps,
    );
    let rec2 = env2.rollout_record();
    println!(
        "perturbed at step {}: steps {} done {:?} gates {} best s {:.1} m",
        perturb_at,
        steps2,
        done2,
        env2.core.gates_hit(),
        env2.core.best_s()
    );
    let cand2 = p.work.join("perturbed.Ghost.Gbx");
    let (s2, g2, b2) = env2.faithful_tape(&tape2);
    tape2.write_candidate(&s2, &g2, &b2, &cand2).unwrap_or_else(|e| die(e));
    let trace2 = rec2.trace.clone();
    drop(env2);

    let diff = control::compare(&env_trace, &trace2, tol);
    println!("unperturbed vs perturbed, env side:  {}", diff);
    let neg_b1 = !diff.same(tol);
    println!(
        "  differ?    {}",
        if neg_b1 { "YES -- the instrument discriminates" } else { "NO -- the comparison is blind, so CONTROL B proved nothing" }
    );

    let flat2 = control::flat_trace(
        &p.server,
        &p.map,
        &p.shim,
        &p.work.join("flat2"),
        &cand2,
        rec2.spans.last().map(|s| (s.from + s.k) as u64).unwrap_or(0) + 300,
    )
    .unwrap_or_else(|e| die(e));
    let cmp2 = control::compare(&trace2, &flat2, tol);
    println!("perturbed env vs its own flat run:   {}", cmp2);
    let neg_b2 = cmp2.same(tol);
    println!(
        "  agree?     {}",
        if neg_b2 { "YES -- the identity holds for a DIFFERENT run too" } else { "NO" }
    );

    let batch2 = tmauto::oracle::validate_raw(
        &p.server,
        &[cand2.clone()],
        tmauto::oracle::Maps::One(&p.map),
        "ka2",
    )
    .unwrap_or_else(|e| die(e));
    let ov2 = batch2.answers.first().and_then(|x| x.verdict());
    println!("perturbed oracle verdict:            {:?}", ov2);
    let neg_a = ov2 != ov || env_gates_from(&rec2) != env_gates_from(&rec);
    println!(
        "  distinguishable? {}",
        if neg_a { "YES" } else { "NO -- one macro of hard left changed nothing the oracle can see" }
    );

    println!();
    let all = a_ok && b_ok && neg_b1 && neg_b2;
    println!(
        "VERDICT      {}",
        if all {
            "the environment reports the run the engine performs, on both axes, with both \
             negative halves"
        } else {
            "UNMEASURED or FAILED -- see the arms above. Nothing trained on this env counts."
        }
    );
    if !all {
        std::process::exit(1);
    }
}

fn env_gates_from(r: &tmenv::Rollout) -> usize {
    r.gates_hit
}

fn env_gate_report(_o: &Option<tmauto::Verdict>, g: usize) -> String {
    format!("{g} gate(s) collected")
}

// ------------------------------------------------------------------ bench

fn bench(a: &[String]) {
    let p = paths(a);
    let workers: usize = num(a, "--workers", 8);
    let steps: usize = num(a, "--steps", 120);
    let episodes: usize = num(a, "--episodes", 3);
    let k: usize = num(a, "--k", 10);
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));

    println!("# tmenv bench -- rollout throughput");
    println!(
        "# {} workers, k = {} ticks per action, {} episodes x up to {} steps",
        workers, k, episodes, steps
    );
    println!("# box: {} cores", std::thread::available_parallelism().map(|v| v.get()).unwrap_or(0));
    if let Ok(l) = std::fs::read_to_string("/proc/loadavg") {
        println!("# load at start: {}", l.trim());
    }
    println!();

    // The track is read-only and identical for every worker: building it 40
    // times costs 40 pak stores and ~4.7 s each, and that is setup being
    // measured as if it were stepping.
    let t_track = Instant::now();
    let track = std::sync::Arc::new(measured_track(&p).unwrap_or_else(|e| die(e)));
    println!("track built once in {:.2} s, shared by every worker", t_track.elapsed().as_secs_f64());

    let t_all = Instant::now();
    let done: Vec<(usize, f64, f64, usize)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..workers)
            .map(|w| {
                let p = &p;
                let a = a.to_vec();
                let track = track.clone();
                sc.spawn(move || {
                    let work = p.work.join(format!("w{w}"));
                    let t0 = Instant::now();
                    let mut env = build_env_with(p, &a, &work, track);
                    let setup = t0.elapsed().as_secs_f64();
                    let fwd = ActionSpace::default().forward();
                    let start = Instant::now();
                    let mut n = 0usize;
                    let mut eps = 0usize;
                    for _ in 0..episodes {
                        let (s, _r, _d) = drive(&mut env, &|_| fwd, steps);
                        n += s;
                        eps += 1;
                    }
                    (n, start.elapsed().as_secs_f64(), setup, eps)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("worker panicked")).collect()
    });
    let wall = t_all.elapsed().as_secs_f64();
    let total: usize = done.iter().map(|d| d.0).sum();
    let eps: usize = done.iter().map(|d| d.3).sum();
    let steady: f64 = done.iter().map(|d| d.1).fold(0.0, f64::max);
    let setup_med = {
        let mut v: Vec<f64> = done.iter().map(|d| d.2).collect();
        v.sort_by(|x, y| x.partial_cmp(y).unwrap());
        v[v.len() / 2]
    };

    println!();
    println!("workers            {}", workers);
    println!("env-steps          {}", total);
    println!("episodes           {}", eps);
    println!("median worker setup {:.2} s  (server launch + car locate; paid once per worker)", setup_med);
    println!("steady-state wall  {:.2} s  (slowest worker's stepping only, setup excluded)", steady);
    println!("end-to-end wall    {:.2} s  (setup included)", wall);
    println!();
    println!("STEADY-STATE       {:.0} env-steps/s", total as f64 / steady);
    println!("                   {:.0} game-ticks/s = {:.0}x wall-clock", total as f64 * k as f64 / steady, total as f64 * k as f64 * 0.010 / steady);
    println!("  per worker       {:.1} env-steps/s", total as f64 / steady / workers as f64);
    println!("  per step         {:.3} ms  (D measured 6.953 ms for a k=10 branch on an idle box)", steady * 1000.0 * workers as f64 / total as f64);
    println!("END-TO-END         {:.0} env-steps/s", total as f64 / wall);
    if let Ok(l) = std::fs::read_to_string("/proc/loadavg") {
        println!("load at end        {}", l.trim());
    }
}

// ---------------------------------------------------------------- rollout

fn rollout(a: &[String]) {
    let p = paths(a);
    let steps: usize = num(a, "--steps", 400);
    let seed: u64 = num(a, "--seed", 1);
    let policy = flag(a, "--policy").unwrap_or_else(|| "forward".into());
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));
    let (mut env, _rig, tape) = build_env(&p, a, &p.work);
    let fwd = ActionSpace::default().forward();
    let n = env.n_actions();
    let mut st = seed.wrapping_mul(6364136223846793005).wrapping_add(1) | 1;
    let mut pre: Vec<usize> = Vec::with_capacity(steps);
    for _ in 0..steps {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        pre.push((st % n as u64) as usize);
    }
    let (nst, ret, done) = match policy.as_str() {
        "random" => drive(&mut env, &|i| pre[i.min(pre.len() - 1)], steps),
        _ => drive(&mut env, &|_| fwd, steps),
    };
    let rec = env.rollout_record();
    println!("steps {nst}  return {ret:.3}  done {done:?}");
    println!(
        "gates {}/{}  best s {:.1} m  finish {:?}",
        env.core.gates_hit(),
        env.core.track.n_gates(),
        env.core.best_s(),
        rec.finish_ms.map(secs)
    );
    if has(a, "--write") {
        let out = p.work.join("rollout.Ghost.Gbx");
        let (s, g, b) = env.banked_tape(&tape);
        tape.write_candidate(&s, &g, &b, &out).unwrap_or_else(|e| die(e));
        println!("wrote {}", out.display());
    }
}

// ------------------------------------------------------------------ trace

/// Drive a scripted episode and print what the track model says about every
/// tick of it.
///
/// This exists because CONTROL A and CONTROL B certify that the trajectory is
/// REAL and say nothing about whether the *progress metric over it* is sane. A
/// route that doubles back on itself makes nearest-point arc length jump to the
/// far leg, and both controls pass while the reward is nonsense.
fn trace_cmd(a: &[String]) {
    let p = paths(a);
    let steps: usize = num(a, "--steps", 60);
    let every: usize = num(a, "--every", 10);
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));
    let (mut env, _rig, _t) = build_env(&p, a, &p.work);
    let fwd = ActionSpace::default().forward();
    env.reset().unwrap_or_else(|e| die(e));
    println!("tick  race_s      x        y        z     speed      s   lateral  height  gate  cap");
    let mut n = 0;
    loop {
        let (_o, _r, d, i) = match env.step(fwd) {
            Ok(v) => v,
            Err(e) => die(e),
        };
        if n % every == 0 || d.is_some() {
            let r = env.core.last_row();
            println!(
                "{:5} {:7.3} {:8.1} {:7.1} {:8.1} {:7.1} {:7.1} {:8.2} {:7.2} {:4} {:7.1}",
                i.tick, i.race_s, r.x, r.y, r.z, i.speed, i.s, i.lateral, i.height, i.gates, i.best_s
            );
        }
        n += 1;
        if d.is_some() || n >= steps {
            println!("done {:?} after {} steps", d, n);
            break;
        }
    }
    let t = &env.core.track;
    println!();
    println!("route vertex 0 at ({:.1}, {:.1}, {:.1})", t.route.at(0.0)[0], t.route.at(0.0)[1], t.route.at(0.0)[2]);
    println!("spawn         ({:.1}, {:.1}, {:.1})", t.pack.spawn[0], t.pack.spawn[1], t.pack.spawn[2]);
    for s in [0.0f32, 50.0, 100.0, 200.0, 300.0, 400.0, 511.6] {
        let q = t.route.at(s);
        println!("  route at {:6.1} m -> ({:8.1}, {:6.1}, {:8.1})", s, q[0], q[1], q[2]);
    }
}

// -------------------------------------------------------------- checktraj

/// Measure a recorded trajectory against the map's own geometry.
///
/// This is the control CONTROL A and CONTROL B do not provide. They certify
/// that the trajectory is real and that the stepped trajectory is the flat one.
/// Neither says a word about whether the ROUTE the reward is computed against
/// describes the drive at all — and a route whose start is in the wrong place
/// produces a real trajectory, a passing identity check, and a reward that is
/// nonsense.
///
/// Reads the 29-column CSV `fk trace` / `tmtraj decode --csv` write, so it can
/// be pointed at any trajectory this project produces.
fn checktraj(a: &[String]) {
    let p = paths(a);
    let csv = flag(a, "--csv").unwrap_or_else(|| die("--csv FILE is required".into()));
    let text = std::fs::read_to_string(&csv).unwrap_or_else(|e| die(format!("{csv}: {e}")));
    let mut pts: Vec<(f64, [f32; 3], f32)> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let c: Vec<&str> = line.trim().split(',').collect();
        if c.len() < 7 {
            continue;
        }
        let g = |k: usize| c[k].parse::<f64>().unwrap_or(f64::NAN);
        pts.push((g(0), [g(1) as f32, g(2) as f32, g(3) as f32], g(5) as f32));
    }
    if pts.is_empty() {
        die(format!("{csv} carried no rows"));
    }
    let t = load_track(&p.server, &p.map).unwrap_or_else(|e| die(e));

    println!("trajectory   {} rows, race {:.3} .. {:.3}", pts.len(), pts[0].0 / 1000.0, pts[pts.len() - 1].0 / 1000.0);
    println!("first point  ({:.1}, {:.1}, {:.1}) at {:.3}, speed {:.1} m/s", pts[0].1[0], pts[0].1[1], pts[0].1[2], pts[0].0 / 1000.0, pts[0].2);
    println!("spawn block  ({:.1}, {:.1}, {:.1})", t.pack.spawn[0], t.pack.spawn[1], t.pack.spawn[2]);
    let d0 = {
        let d = tmenv::geom::sub(pts[0].1, t.pack.spawn);
        tmenv::geom::norm(d)
    };
    println!("             the first sample is {:.1} m from the spawn block", d0);
    println!();
    println!("--- closest approach to each ordered gate (the tour B solved)");
    for i in 0..t.n_gates() {
        let g = t.gate_pos[i];
        let mut best = (f32::INFINITY, 0.0f64);
        for (ms, q, _) in &pts {
            let d = tmenv::geom::norm(tmenv::geom::sub(*q, g));
            if d < best.0 {
                best = (d, *ms);
            }
        }
        println!(
            "  gate {}  s = {:8.1} m  at ({:8.1},{:6.1},{:8.1})   closest {:8.2} m at race {:7.3}",
            i, t.gate_s[i], g[0], g[1], g[2], best.0, best.1 / 1000.0
        );
    }
    println!();
    println!("--- closest approach to each RAW waypoint the map declares");
    let mut raw: Vec<(String, [f32; 3])> = Vec::new();
    for (i, c) in t.pack.checkpoints.iter().enumerate() {
        raw.push((format!("Checkpoint[{i}]"), c.pos));
    }
    for (i, c) in t.pack.finish.iter().enumerate() {
        raw.push((format!("Goal[{i}]"), c.pos));
    }
    raw.push(("Spawn".into(), t.pack.spawn));
    for (name, g) in &raw {
        let mut best = (f32::INFINITY, 0.0f64);
        for (ms, q, _) in &pts {
            let d = tmenv::geom::norm(tmenv::geom::sub(*q, *g));
            if d < best.0 {
                best = (d, *ms);
            }
        }
        println!(
            "  {:<16} ({:8.1},{:6.1},{:8.1})   closest {:8.2} m at race {:7.3}",
            name, g[0], g[1], g[2], best.0, best.1 / 1000.0
        );
    }
    println!();
    println!("--- arc length along B's route, over the drive");
    println!("  race_s     x        y        z    speed       s   lateral");
    let step = (pts.len() / 24).max(1);
    for (ms, q, sp) in pts.iter().step_by(step) {
        let pr = t.probe(*q);
        println!(
            "  {:7.3} {:8.1} {:7.1} {:8.1} {:7.1} {:8.1} {:8.2}",
            ms / 1000.0, q[0], q[1], q[2], sp, pr.s, pr.lateral
        );
    }
    let mut plen = 0.0f32;
    for w in pts.windows(2) {
        plen += tmenv::geom::norm(tmenv::geom::sub(w[1].1, w[0].1));
    }
    println!();
    println!("path length driven {:.1} m; B's route is {:.1} m", plen, t.length());
}

// ----------------------------------------------------------------- locate


// ------------------------------------------------------------------- seek

/// Drive the car at a list of world-space waypoints with pure pursuit, then ask
/// the PLAIN ORACLE what the resulting tape did.
///
/// # Why this exists
///
/// The map says the start is at its `RoadTechStart` block; the engine puts the
/// car 389 m away from it. Both readings cannot order the checkpoints, and I do
/// not get to pick which one to believe by argument. The oracle can settle it:
/// drive at a candidate first checkpoint, and see whether the server credits
/// one. That is an experiment with a known answer on each arm, and it needs no
/// theory about why the start moved.
fn seek(a: &[String]) {
    let p = paths(a);
    let steps: usize = num(a, "--steps", 400);
    let reach: f32 = num(a, "--reach", 22.0);
    let wps: Vec<[f32; 3]> = flag(a, "--via")
        .unwrap_or_default()
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() != 3 {
                die(format!("--via wants x,y,z;x,y,z;...  got {s}"));
            }
            [v[0], v[1], v[2]]
        })
        .collect();
    if wps.is_empty() {
        die("--via 'x,y,z;x,y,z' is required".into());
    }
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));
    let (mut env, _rig, tape) = build_env(&p, a, &p.work);
    let acts = ActionSpace::default();

    env.reset().unwrap_or_else(|e| die(e));
    let mut wi = 0usize;
    let mut act = acts.forward();
    let mut done = None;
    let mut n = 0usize;
    let mut closest: Vec<f32> = vec![f32::INFINITY; wps.len()];
    let mut samples: Vec<(f32, [f32; 3], f32)> = Vec::new();
    while n < steps {
        let (_o, _r, d, _i) = match env.step(act) {
            Ok(v) => v,
            Err(e) => die(e),
        };
        n += 1;
        let r = env.core.last_row();
        let pos = [r.x as f32, r.y as f32, r.z as f32];
        if n % 5 == 1 { samples.push((r.time_ms as f32 / 1000.0, pos, tmenv::geom::norm([r.vx as f32, r.vy as f32, r.vz as f32]))); }
        for (i, w) in wps.iter().enumerate() {
            let dd = tmenv::geom::norm(tmenv::geom::sub(pos, *w));
            if dd < closest[i] {
                closest[i] = dd;
            }
        }
        if let Some(d) = d {
            done = Some(d);
            break;
        }
        // Advance the target when it is reached.
        while wi + 1 < wps.len()
            && tmenv::geom::norm(tmenv::geom::sub(pos, wps[wi])) < reach
        {
            wi += 1;
        }
        // Pure pursuit: the bearing to the target in the car's own frame picks
        // the steer rung; the throttle eases off only for a very sharp turn.
        let q = tmenv::geom::Quat(r.qx as f32, r.qy as f32, r.qz as f32, r.qw as f32);
        let to = q.world_to_car(tmenv::geom::sub(wps[wi], pos));
        // The car's forward axis is -z in its own frame (yaw ~ pi facing -z in
        // world at the start, and the readout's quaternion takes car to world).
        // The car's local forward is +z: at the start the world velocity is -z and
        // the quaternion is a 180-degree turn about Y, so world_to_car puts the
        // velocity on +z. Measured, not assumed.
        let mut bearing = (to[0]).atan2(to[2]);
        if has(a, "--flip") { bearing = -bearing; }
        let want = (bearing / (35f32.to_radians())).clamp(-1.0, 1.0);
        let rung = ((want * 2.0).round() as i32 + 2).clamp(0, 4) as usize;
        let throttle = if bearing.abs() > 1.1 { 2 } else { 0 };
        act = rung * 4 + throttle;
    }

    println!("steps {n}  done {done:?}  waypoint reached index {wi} of {}", wps.len() - 1);
    println!("--- the drive");
    for (i, r) in samples.iter().enumerate() {
        println!("  {:3}  race {:6.3}  ({:8.1},{:6.1},{:8.1})  speed {:6.1}", i, r.0, r.1[0], r.1[1], r.1[2], r.2);
    }
    for (i, w) in wps.iter().enumerate() {
        println!("  waypoint {i} ({:.0},{:.0},{:.0})  closest {:.2} m", w[0], w[1], w[2], closest[i]);
    }
    let out = p.work.join("seek.Ghost.Gbx");
    let (s, g, b) = env.banked_tape(&tape);
    tape.write_candidate(&s, &g, &b, &out).unwrap_or_else(|e| die(e));
    let batch = tmauto::oracle::validate_raw(
        &p.server,
        &[out.clone()],
        tmauto::oracle::Maps::One(&p.map),
        "seek",
    )
    .unwrap_or_else(|e| die(e));
    match batch.answers.first() {
        Some(ans) => {
            println!("PLAIN ORACLE {:?}   (simulated {})", ans.verdict(), ans.simulated());
            println!("  desc       {}", ans.desc.trim());
        }
        None => println!("PLAIN ORACLE no answer"),
    }
    println!("tape         {}", out.display());
}

// ---------------------------------------------------------------- verdict

/// Hand files to the plain oracle and print the server's OWN transcript.
///
/// `Answer` is a parse of that transcript, and a parse is a place a wrong
/// reading can hide. When the parsed verdict is surprising, the thing to read is
/// what the server said.
fn verdict_cmd(a: &[String]) {
    let p = paths(a);
    let files: Vec<PathBuf> = a
        .iter()
        .skip(1)
        .filter(|x| x.ends_with(".Ghost.Gbx") || x.ends_with(".Replay.Gbx"))
        .map(PathBuf::from)
        .collect();
    if files.is_empty() {
        die("give one or more .Ghost.Gbx files".into());
    }
    let batch = tmauto::oracle::validate_raw(
        &p.server,
        &files,
        tmauto::oracle::Maps::One(&p.map),
        "verdict",
    )
    .unwrap_or_else(|e| die(e));
    println!("--- the server's own transcript");
    println!("{}", batch.raw);
    println!("--- parsed");
    for ans in &batch.answers {
        println!(
            "{:<28} verdict {:?}  simulated {}  cps {:?}  time {:?}  declared {:?}  desc {}",
            ans.file,
            ans.verdict(),
            ans.simulated(),
            ans.cps,
            ans.time_ms,
            ans.declared_ms,
            ans.desc.trim()
        );
    }
}

// ------------------------------------------------------------------ spawn

/// Measure where the engine puts the car, re-solve the route from there, and
/// print both routes side by side with the controls that discriminate them.
fn spawn_cmd(a: &[String]) {
    let p = paths(a);
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));
    let (fix, rows) =
        control::measure_spawn(&p.server, &p.map, &p.shim, &p.work, &p.reference)
            .unwrap_or_else(|e| die(e));
    println!("# tmenv spawn -- where the engine ACTUALLY starts the car");
    println!();
    println!(
        "measured     ({:.2}, {:.2}, {:.2})  at race {:.3}, speed {:.2} m/s, probe tick {}",
        fix.pos[0], fix.pos[1], fix.pos[2], fix.race_ms as f64 / 1000.0, fix.speed, fix.probe_tick
    );
    println!("  the car has been rolling for {:.3} s at the earliest sample, so the true", fix.race_ms as f64 / 1000.0);
    println!("  spawn is a metre or two behind this. Reported, not corrected.");
    for r in rows.iter().take(3) {
        println!(
            "  race {:.3}  ({:.2}, {:.2}, {:.2})  v ({:.2}, {:.2}, {:.2})",
            r.time_ms as f64 / 1000.0, r.x, r.y, r.z, r.vx, r.vy, r.vz
        );
    }

    for (label, sp) in [("map's own Spawn waypoint", None), ("the MEASURED spawn", Some(fix.pos))] {
        println!();
        println!("--- route solved from {label}");
        match tmenv::load_track_from(&p.server, &p.map, sp) {
            Err(e) => println!("  no route: {e}"),
            Ok(t) => {
                println!("  origin     ({:.1}, {:.1}, {:.1})", t.pack.spawn[0], t.pack.spawn[1], t.pack.spawn[2]);
                println!("  length     {:.1} m over {} stations", t.length(), t.route.stations.len());
                if let Some(au) = t.author_s() {
                    let v = t.length() as f64 / au;
                    println!(
                        "  implied    {:.1} m/s over the author time {:.3}{}",
                        v,
                        au,
                        if v > 95.0 { "   <-- OVER 95 m/s: the route goes somewhere the drive does not" } else { "" }
                    );
                }
                for i in 0..t.n_gates() {
                    println!(
                        "  gate {}     s = {:8.1} m  at ({:8.1},{:6.1},{:8.1})   {:7.1} m from the spawn",
                        i,
                        t.gate_s[i],
                        t.gate_pos[i][0],
                        t.gate_pos[i][1],
                        t.gate_pos[i][2],
                        tmenv::geom::norm(tmenv::geom::sub(t.gate_pos[i], t.pack.spawn))
                    );
                }
                let pr = t.probe(fix.pos);
                println!(
                    "  the measured spawn sits at s = {:.1} m, lateral {:.2} m on THIS route",
                    pr.s, pr.lateral
                );
            }
        }
    }
}

// -------------------------------------------------------------- calibrate

/// Calibrate the geometric gate detector against the PLAIN ORACLE.
///
/// # Why this is not optional
///
/// The reward's progress term saturates at the first gate the car still owes,
/// so a detector that under-reports pins the cap and the policy can never be
/// paid past it — silently, with a reward that looks fine. And a detector that
/// over-reports pays for shortcuts.
///
/// The detector's own reading cannot check itself, so the oracle does: drive a
/// spread of random policies, write each tape, ask the server how many
/// checkpoints it credits, and compare. The oracle is a separate dedicated
/// server validating a file — no shim, no fork, no readout — so agreement is
/// not two readings of one source.
///
/// The sweep over radii is a measurement, not a fit: the answer is reported
/// with the disagreements it still has, and a radius that agrees everywhere is
/// only believable if some radius in the sweep DISAGREES somewhere.
fn calibrate(a: &[String]) {
    let p = paths(a);
    let n: usize = num(a, "--tapes", 24);
    let steps: usize = num(a, "--steps", 200);
    let radii: Vec<f32> = flag(a, "--radii")
        .unwrap_or_else(|| "6,9,12,16,20,26,32".into())
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));

    println!("# tmenv calibrate -- the gate detector against the plain oracle");
    println!("# {n} tapes, up to {steps} actions each, radii {radii:?}");
    println!();

    let (mut env, _rig, tape) = build_env(&p, a, &p.work);
    let nact = env.n_actions();
    let ngates = env.core.track.n_gates();
    let mut files: Vec<PathBuf> = Vec::new();
    // Per tape: the position/arc-length trace, so every radius can be scored
    // from ONE set of drives rather than re-driving per radius (which would
    // make the arms differ by more than the radius).
    let mut traces: Vec<Vec<([f32; 3], f32)>> = Vec::new();

    let mut st: u64 = num(a, "--seed", 7u64) | 1;
    let mut rnd = move || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        st
    };
    for t in 0..n {
        // A spread: mostly-forward tapes with a per-tape steering bias, so the
        // set contains runs that go nowhere AND runs that get somewhere.
        let bias = (rnd() % nact as u64) as usize;
        let hold = 1 + (rnd() % 12) as usize;
        let mut plan: Vec<usize> = Vec::with_capacity(steps);
        let mut cur = bias;
        for i in 0..steps {
            if i % hold == 0 {
                cur = (rnd() % nact as u64) as usize;
            }
            plan.push(cur);
        }
        env.reset().unwrap_or_else(|e| die(e));
        let mut tr: Vec<([f32; 3], f32)> = Vec::new();
        for i in 0..steps {
            let (_o, _r, d, info) = match env.step(plan[i]) {
                Ok(v) => v,
                Err(_) => break,
            };
            let r = env.core.last_row();
            tr.push(([r.x as f32, r.y as f32, r.z as f32], info.s));
            if d.is_some() {
                break;
            }
        }
        let f = p.work.join(format!("cal{t:03}.Ghost.Gbx"));
        let (s, g, b) = env.banked_tape(&tape);
        tape.write_candidate(&s, &g, &b, &f).unwrap_or_else(|e| die(e));
        files.push(f);
        traces.push(tr);
    }

    let batch = tmauto::oracle::validate_raw(
        &p.server,
        &files,
        tmauto::oracle::Maps::One(&p.map),
        "cal",
    )
    .unwrap_or_else(|e| die(e));
    // The oracle renames on a filename collision, so answers are matched by
    // ORDER, which validate_raw preserves, and the count is asserted.
    if batch.answers.len() != files.len() {
        die(format!(
            "the oracle answered {} of {} files; matching by order would misalign them",
            batch.answers.len(),
            files.len()
        ));
    }
    let oracle: Vec<usize> = batch
        .answers
        .iter()
        .map(|x| match x.verdict() {
            Some(tmauto::Verdict::Dnf { cps }) => cps as usize,
            Some(tmauto::Verdict::Finish { .. }) => ngates,
            None => usize::MAX,
        })
        .collect();
    let refused = oracle.iter().filter(|x| **x == usize::MAX).count();
    println!("oracle answered {} tapes ({} refused, excluded)", oracle.len(), refused);
    let spread: std::collections::BTreeMap<usize, usize> =
        oracle.iter().filter(|x| **x != usize::MAX).fold(Default::default(), |mut m, c| {
            *m.entry(*c).or_default() += 1;
            m
        });
    println!("oracle checkpoint counts across the set: {spread:?}");
    if spread.len() < 2 {
        println!();
        println!("WARNING: every tape got the same answer, so this set cannot discriminate any");
        println!("radius from any other. Whatever agrees below agrees vacuously. Drive a wider");
        println!("spread (more tapes, more steps) before believing a radius.");
    }
    println!();
    let dirmode = tmenv::track::CrossDir::parse(&flag(a, "--dir").unwrap_or_else(|| "any".into()))
        .unwrap_or_else(|| die("--dir wants fwd, rev or any".into()));
    println!("crossing direction through the gate plane: {dirmode:?}");
    println!("radius   exact   over   under   mean |error|");
    let mut best = (f32::INFINITY, 0f32);
    for &rad in &radii {
        let mut exact = 0usize;
        let mut over = 0usize;
        let mut under = 0usize;
        let mut err = 0f32;
        let mut cnt = 0usize;
        for (i, tr) in traces.iter().enumerate() {
            if oracle[i] == usize::MAX {
                continue;
            }
            let mut gt = tmenv::track::GateTracker::new(rad, 1.0e9);
            gt.dir = dirmode;
            gt.reset();
            for (pos, s) in tr {
                gt.observe(&env.core.track, *pos, *s);
            }
            let mine = gt.hit();
            cnt += 1;
            err += (mine as f32 - oracle[i] as f32).abs();
            match mine.cmp(&oracle[i]) {
                std::cmp::Ordering::Equal => exact += 1,
                std::cmp::Ordering::Greater => over += 1,
                std::cmp::Ordering::Less => under += 1,
            }
        }
        let m = if cnt == 0 { f32::NAN } else { err / cnt as f32 };
        println!("{rad:6.0}  {exact:6}  {over:5}  {under:6}   {m:10.3}");
        if m < best.0 {
            best = (m, rad);
        }
    }
    println!();
    println!("best radius {:.0} m, mean |error| {:.3} checkpoints", best.1, best.0);
    if best.0 > 0.0 {
        println!("NOT exact. The detector is a reward-shaping device and the oracle remains the");
        println!("arbiter of every banked result, so a residual here costs signal quality, not");
        println!("correctness -- but it is a task, not a conclusion.");
    }
}

// --------------------------------------------------------------- startmap

/// Where does the dedicated server put the car, and which of the map's own
/// waypoints is that?
///
/// # The design, and why it can come out wrong
///
/// Two hypotheses about the start, which disagree:
///
/// * **H_spawn** — the car starts at the waypoint tagged `Spawn`
///   (`RoadTechStart` and friends). This is what the map file means and what
///   `mapgeom` assumes.
/// * **H_first** — the car starts at the map's FIRST waypoint in file order,
///   whatever it is tagged.
///
/// Across the 25 Summer 2026 maps, 7 have `waypoint[0] == Spawn` and 18 do not,
/// so the campaign is a natural experiment rather than a single case:
///
/// * the 7 are the **positive control** — both hypotheses predict the Spawn
///   there, so if the car is somewhere else BOTH are wrong and the instrument
///   is what is broken;
/// * the 18 **discriminate**, and three of them (Summer 2026 - 12, 15, 18) have
///   a `Goal` first, which is about as sharp as a prediction gets.
///
/// A result on one map is a coincidence. This runs several and prints the
/// per-map verdict plus the tally, and it is written so that "H_first" can lose.
fn startmap(a: &[String]) {
    let p = paths(a);
    let maps: Vec<PathBuf> = a
        .iter()
        .filter(|x| x.ends_with(".Map.Gbx"))
        .map(PathBuf::from)
        .collect();
    if maps.is_empty() {
        die("give one or more .Map.Gbx paths".into());
    }
    let bank = PathBuf::from(
        flag(a, "--bank").unwrap_or_else(|| "/tmp/tmenv/startmap".into()),
    );
    std::fs::create_dir_all(&bank).unwrap_or_else(|e| die(e.to_string()));
    let ticks: usize = num(a, "--ticks", 3000);

    println!("# tmenv startmap -- which waypoint does the engine start the car at?");
    println!("#");
    println!("# H_spawn : the waypoint tagged Spawn (what the map file means)");
    println!("# H_first : the map's FIRST waypoint in file order, whatever its tag");
    println!("# banking transcripts and trajectories under {}", bank.display());
    println!();

    let mut tally = (0usize, 0usize, 0usize, 0usize); // spawn, first, both, neither
    for m in &maps {
        let uid = m.file_name().map(|s| s.to_string_lossy().replace(".Map.Gbx", "")).unwrap_or_default();
        let hdr = tmmaps::header::read(&m.to_string_lossy()).ok();
        let name = hdr.map(|h| h.name).unwrap_or_else(|| uid.clone());
        println!("=== {name}  [{uid}]");

        // The map's own waypoints, in file order, with no MapPack in the way.
        let mf = tmmaps::map::MapFile::load(m);
        let yoff = {
            let mut store = match open_store(&p.server) {
                Ok(s) => s,
                Err(e) => {
                    println!("  no pak store ({e}); skipping");
                    continue;
                }
            };
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            let _ = asm.with_embedded(&mf);
            let (gs, _) = asm.map_split(&mf);
            match mapgeom::yoff::measure(&mf, &gs).value() {
                Some(y) => y,
                None => {
                    println!("  map height UNMEASURED; skipping");
                    continue;
                }
            }
        };
        let wps = mapgeom::pack::gates(&mf, yoff);
        if wps.is_empty() {
            println!("  no waypoints; skipping");
            continue;
        }
        for (i, g) in wps.iter().enumerate() {
            println!(
                "  waypoint[{i}] {:<12} {:<34} ({:8.1},{:6.1},{:8.1})",
                g.tag, g.name, g.pos[0], g.pos[1], g.pos[2]
            );
        }
        let first = wps[0].pos;
        let spawn = match wps.iter().find(|g| g.tag == "Spawn") {
            Some(g) => g.pos,
            None => {
                println!("  no Spawn waypoint at all; skipping");
                continue;
            }
        };

        // A reference container for this map, ghost-free.
        let dir = bank.join(&uid);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| die(e.to_string()));
        let refp = dir.join("reference.Ghost.Gbx");
        if let Err(e) = write_reference(&p.map_or(m), m, ticks, &dir, &refp) {
            println!("  could not build a container: {e}");
            continue;
        }
        let work = dir.join("work");
        let _ = std::fs::remove_dir_all(&work);
        let (fix, rows) = match control::measure_spawn(&p.server, m, &p.shim, &work, &refp) {
            Ok(v) => v,
            Err(e) => {
                println!("  START UNMEASURED: {e}");
                continue;
            }
        };
        // Bank the trajectory. Every acceptance run leaves its raw evidence.
        let csv = dir.join("start-trajectory.csv");
        let mut s = String::from("time_ms,x,y,z,vx,vy,vz,qx,qy,qz,qw\n");
        for r in &rows {
            s.push_str(&format!(
                "{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.6},{:.6},{:.6},{:.6}\n",
                r.time_ms, r.x, r.y, r.z, r.vx, r.vy, r.vz, r.qx, r.qy, r.qz, r.qw
            ));
        }
        let _ = std::fs::write(&csv, s);

        let d_first = tmenv::geom::norm(tmenv::geom::sub(fix.pos, first));
        let d_spawn = tmenv::geom::norm(tmenv::geom::sub(fix.pos, spawn));
        println!(
            "  MEASURED start ({:.2}, {:.2}, {:.2}) at race {:.3}, speed {:.2} m/s",
            fix.pos[0], fix.pos[1], fix.pos[2], fix.race_ms as f64 / 1000.0, fix.speed
        );
        println!("    distance to waypoint[0] (H_first) : {d_first:8.2} m");
        println!("    distance to the Spawn   (H_spawn) : {d_spawn:8.2} m");
        // The car rolls for ~0.1 s before the earliest sample, so a few metres
        // is agreement and tens of metres is not.
        let tol: f32 = num(a, "--tol", 15.0);
        let vf = d_first <= tol;
        let vs = d_spawn <= tol;
        let verdict = match (vs, vf) {
            (true, true) => {
                tally.2 += 1;
                "BOTH agree here (this map cannot discriminate)"
            }
            (true, false) => {
                tally.0 += 1;
                "H_spawn"
            }
            (false, true) => {
                tally.1 += 1;
                "H_first"
            }
            (false, false) => {
                tally.3 += 1;
                "NEITHER -- both hypotheses are wrong on this map"
            }
        };
        println!("    verdict: {verdict}");
        println!("    banked: {}", dir.display());
        println!();
    }
    println!("tally  H_spawn {}   H_first {}   both(no discrimination) {}   neither {}",
        tally.0, tally.1, tally.2, tally.3);
    if tally.2 == 0 {
        println!();
        println!("WARNING: no map in this set had waypoint[0] == Spawn, so there is no positive");
        println!("control. A run with only discriminating maps cannot tell 'H_first is right'");
        println!("from 'the measurement is broken in a way that always points at waypoint[0]'.");
    }
}

fn open_store(server: &Path) -> Result<mapgeom::store::DataStore, String> {
    let packs = server.join("Packs");
    let mut paths: Vec<String> = Vec::new();
    for n in ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"] {
        let q = packs.join(n);
        if q.exists() {
            paths.push(q.to_string_lossy().into_owned());
        }
    }
    if paths.is_empty() {
        return Err(format!("no .pak under {}", packs.display()));
    }
    mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY)
}

impl Paths {
    fn map_or<'b>(&'b self, m: &'b Path) -> &'b Path {
        if self.map.as_os_str().is_empty() {
            m
        } else {
            &self.map
        }
    }
}

/// Synthesize a reference container for a map: ghost-free, declared correctly,
/// varied steer so the shim can find the input array.
fn write_reference(
    _unused: &Path,
    map: &Path,
    ticks: usize,
    dir: &Path,
    out: &Path,
) -> Result<(), String> {
    let declared = (ticks as u32) * 10;
    let flat: Vec<tmauto::Input> = (0..ticks).map(|_| tmauto::Input::new(0, true, false)).collect();
    let mut meta = tmauto::synth::meta_for_map(map)?;
    meta.set_declared(declared, Vec::new());
    let raw = dir.join("rung0.Ghost.Gbx");
    let bytes = tmauto::synth::synthesize(
        &tmauto::synth::pad_to(&flat, ticks),
        &meta,
        &tmauto::synth::ChunkSet::ALL,
    );
    std::fs::write(&raw, &bytes).map_err(|e| e.to_string())?;
    let t = fk::tape::Tape::load(&raw.to_string_lossy())?;
    fk::cmd::tree::tape_vary(&t, out, 20260824, 12)
}

// -------------------------------------------------------------- fieldsweep

/// MINIMAL PAIRS: two containers differing in exactly one field, the same tape,
/// and the question of which field moves the car's start state.
///
/// # Why a sweep and not an argument
///
/// The engine starts the validated car somewhere other than the map's `Spawn`
/// waypoint. That is either something the ENGINE does with this map, or
/// something OUR CONTAINER asks for. Those have completely different fixes and
/// no amount of reading tells them apart.
///
/// Each arm changes exactly one field from the baseline, so a difference names
/// a field. The baseline is re-measured in the same batch — a paired difference
/// only transfers if its baseline was taken on the same box in the same
/// conditions — and every arm banks its container, its trajectory and the
/// server's own transcript.
///
/// **The sweep can come out empty, and that is a result**: if no single field
/// moves the start, the container is not the cause and the engine is, which is
/// a different investigation and a much better thing to know than a guess.
fn fieldsweep(a: &[String]) {
    let p = paths(a);
    let ticks: usize = num(a, "--ticks", 3000);
    let bank = PathBuf::from(flag(a, "--bank").unwrap_or_else(|| "/tmp/tmenv/fieldsweep".into()));
    std::fs::create_dir_all(&bank).unwrap_or_else(|e| die(e.to_string()));
    if !p.map.exists() {
        die(format!("no map at {}", p.map.display()));
    }

    // The two candidate positions, straight from the map file.
    let mf = tmmaps::map::MapFile::load(&p.map);
    let mut store = open_store(&p.server).unwrap_or_else(|e| die(e));
    let yoff = {
        let mut asm = mapgeom::assemble::Assembler::new(&mut store);
        let _ = asm.with_embedded(&mf);
        let (gs, _) = asm.map_split(&mf);
        mapgeom::yoff::measure(&mf, &gs).value().unwrap_or_else(|| die("map height UNMEASURED".into()))
    };
    let wps = mapgeom::pack::gates(&mf, yoff);
    let spawn_wp = wps.iter().find(|g| g.tag == "Spawn").map(|g| g.pos)
        .unwrap_or_else(|| die("no Spawn waypoint".into()));
    let first_non_spawn = wps.iter().find(|g| g.tag != "Spawn").map(|g| g.pos)
        .unwrap_or_else(|| die("no non-Spawn waypoint".into()));

    println!("# tmenv fieldsweep -- minimal pairs on the container");
    println!("# map {}", p.map.display());
    println!("# the map's Spawn waypoint        ({:.1}, {:.1}, {:.1})", spawn_wp[0], spawn_wp[1], spawn_wp[2]);
    println!("# the first NON-Spawn waypoint    ({:.1}, {:.1}, {:.1})", first_non_spawn[0], first_non_spawn[1], first_non_spawn[2]);
    println!("# every arm changes ONE field from the baseline; the baseline is in the batch");
    println!();

    type Arm = (&'static str, fn(&mut tmauto::synth::GhostMeta, &mut tmauto::synth::ChunkSet, u32));
    let arms: Vec<Arm> = vec![
        ("baseline", |m, _c, d| m.set_declared(d, Vec::new())),
        ("declared_ms = 0", |m, _c, _d| m.set_declared(0, Vec::new())),
        ("declared_ms = 2500", |m, _c, _d| m.set_declared(2500, Vec::new())),
        ("declared_cps = [1 entry]", |m, _c, d| m.set_declared(d, vec![d as i32])),
        ("declared_cps = [4 entries]", |m, _c, d| {
            let q = d as i32 / 4;
            m.set_declared(d, vec![q, 2 * q, 3 * q, d as i32])
        }),
        ("walltime pair left at 0 length", |m, _c, d| {
            m.declared_ms = d;
            m.declared_cps = Vec::new();
        }),
        ("start_offset_ms = -3000", |m, _c, d| {
            m.set_declared(d, Vec::new());
            m.start_offset_ms = -3000;
        }),
        ("validation_seed = 987654321", |m, _c, d| {
            m.set_declared(d, Vec::new());
            m.validation_seed = 987_654_321;
        }),
        ("no result chunk", |m, c, d| {
            m.set_declared(d, Vec::new());
            c.result = false;
        }),
        ("no racetime chunk", |m, c, d| {
            m.set_declared(d, Vec::new());
            c.racetime = false;
        }),
        ("no validation chunk", |m, c, d| {
            m.set_declared(d, Vec::new());
            c.validation = false;
        }),
        ("no login", |m, c, d| {
            m.set_declared(d, Vec::new());
            c.login = false;
        }),
        ("class = CGameCtnReplayRecord", |m, c, d| {
            m.set_declared(d, Vec::new());
            c.class_id = tmauto::synth::CLASS_CGAMECTNREPLAYRECORD;
        }),
        ("uid_enc = PlainString", |m, c, d| {
            m.set_declared(d, Vec::new());
            c.uid_enc = tmauto::synth::UidEnc::PlainString;
        }),
    ];

    let declared = (ticks as u32) * 10;
    println!("{:<32} {:>10} {:>12} {:>12}  {}", "arm", "race_s", "d(Spawn)", "d(first)", "verdict / note");
    let mut baseline: Option<[f32; 3]> = None;
    for (name, f) in &arms {
        let dir = bank.join(name.replace([' ', '=', '[', ']', ','], "_"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| die(e.to_string()));
        let mut meta = match tmauto::synth::meta_for_map(&p.map) {
            Ok(m) => m,
            Err(e) => {
                println!("{name:<32}  meta failed: {e}");
                continue;
            }
        };
        let mut cs = tmauto::synth::ChunkSet::ALL;
        f(&mut meta, &mut cs, declared);
        let flat: Vec<tmauto::Input> = (0..ticks).map(|_| tmauto::Input::new(0, true, false)).collect();
        let raw = dir.join("rung0.Ghost.Gbx");
        let bytes = tmauto::synth::synthesize(&tmauto::synth::pad_to(&flat, ticks), &meta, &cs);
        if std::fs::write(&raw, &bytes).is_err() {
            println!("{name:<32}  could not write");
            continue;
        }
        let refp = dir.join("reference.Ghost.Gbx");
        let t = match fk::tape::Tape::load(&raw.to_string_lossy()) {
            Ok(t) => t,
            Err(e) => {
                println!("{name:<32}  tape load failed: {e}");
                continue;
            }
        };
        if let Err(e) = fk::cmd::tree::tape_vary(&t, &refp, 20260824, 12) {
            println!("{name:<32}  tape vary failed: {e}");
            continue;
        }
        // Bank the server's own transcript for this arm, before anything forks.
        if let Ok(b) = tmauto::oracle::validate_raw(&p.server, &[refp.clone()], tmauto::oracle::Maps::One(&p.map), "fsw") {
            let _ = std::fs::write(dir.join("oracle-transcript.json"), &b.raw);
            let _ = std::fs::write(dir.join("oracle-stderr.txt"), &b.err);
        }
        let work = dir.join("work");
        match control::measure_spawn(&p.server, &p.map, &p.shim, &work, &refp) {
            Err(e) => println!("{name:<32} {:>10} {:>12} {:>12}  UNMEASURED: {}", "-", "-", "-", e.lines().next().unwrap_or("")),
            Ok((fix, rows)) => {
                let mut s = String::from("time_ms,x,y,z,vx,vy,vz,qx,qy,qz,qw\n");
                for r in &rows {
                    s.push_str(&format!(
                        "{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.6},{:.6},{:.6},{:.6}\n",
                        r.time_ms, r.x, r.y, r.z, r.vx, r.vy, r.vz, r.qx, r.qy, r.qz, r.qw
                    ));
                }
                let _ = std::fs::write(dir.join("start-trajectory.csv"), s);
                let ds = tmenv::geom::norm(tmenv::geom::sub(fix.pos, spawn_wp));
                let df = tmenv::geom::norm(tmenv::geom::sub(fix.pos, first_non_spawn));
                let moved = match baseline {
                    None => {
                        baseline = Some(fix.pos);
                        "(this is the baseline)".to_string()
                    }
                    Some(b) => {
                        let d = tmenv::geom::norm(tmenv::geom::sub(fix.pos, b));
                        if d > 5.0 {
                            format!("*** MOVED {d:.1} m FROM THE BASELINE ***")
                        } else {
                            format!("same as baseline ({d:.2} m)")
                        }
                    }
                };
                println!(
                    "{name:<32} {:>10.3} {:>12.2} {:>12.2}  {}",
                    fix.race_ms as f64 / 1000.0, ds, df, moved
                );
            }
        }
    }
    println!();
    println!("Every arm's container, trajectory and raw server transcript are under");
    println!("{}", bank.display());
}

// ----------------------------------------------------------- reset-control

/// THE ENV RESET CONTROL. Run it before any training run; a failure means the
/// policy would be learning from a state that is not the start of the race.
///
/// # What it asserts, and why each clause is there
///
/// 1. **Position** — the env's reset state is within tolerance of the start
///    established INDEPENDENTLY of `mapgeom`'s MapPack, by measuring the engine
///    itself (`tmenv startmap` / `control::measure_spawn`).
/// 2. **Speed near zero** — the car is at the line, not already driving. A late
///    root reads 15 m/s and five metres downroad, and that stretch is run by
///    the REFERENCE tape, not by the policy.
/// 3. **No checkpoint already collected** — a late root can start the episode
///    past a gate, so the agent is scored on a race it did not begin.
/// 4. **The plain oracle reconstructs the same trajectory** — a fresh process,
///    the written tape, no shim and no fork.
///
/// # It must be able to fail
///
/// `--also-check-old-root` re-runs clauses 1-3 against the root this env used
/// before (`lroundf` 36000, tick ~95) and requires them to **FAIL**. A control
/// that passes on the broken setup certifies nothing, and this one was written
/// against a setup that really was broken.
fn reset_control(a: &[String]) {
    let p = paths(a);
    let tol: f32 = num(a, "--tol", 6.0);
    let vmax: f64 = num(a, "--vmax", 4.0);
    let bank = PathBuf::from(flag(a, "--bank").unwrap_or_else(|| p.work.join("acceptance").to_string_lossy().into_owned()));
    std::fs::create_dir_all(&bank).unwrap_or_else(|e| die(e.to_string()));

    println!("# tmenv reset-control -- does the environment start the episode at the START?");
    println!("# banking every artefact under {}", bank.display());
    println!();

    // ---- the independently established start.
    let (fix, rows0) =
        control::measure_spawn(&p.server, &p.map, &p.shim, &bank.join("spawnfix"), &p.reference)
            .unwrap_or_else(|e| die(format!("the start itself is UNMEASURED: {e}")));
    println!("independently measured start (engine memory, no MapPack involved)");
    println!(
        "  ({:.2}, {:.2}, {:.2})  race {:.3}  speed {:.2} m/s  probe tick {}",
        fix.pos[0], fix.pos[1], fix.pos[2], fix.race_ms as f64 / 1000.0, fix.speed, fix.probe_tick
    );
    bank_traj(&bank.join("independent-start.csv"), &rows0);

    let mut arms: Vec<(&str, Option<u64>)> = vec![("the env's root (ladder)", None)];
    if has(a, "--also-check-old-root") {
        arms.push(("the OLD root (must FAIL)", Some(36_000)));
    }

    let mut ok_new = false;
    for (label, clock) in arms {
        println!();
        println!("--- {label}");
        let mut aa: Vec<String> = a.iter().filter(|x| x.as_str() != "--root-clock").cloned().collect();
        if let Some(c) = clock {
            aa.push("--root-clock".into());
            aa.push(c.to_string());
            // The old root is INSIDE the race, so its probe is a real tick and
            // the read-ahead restart rule would refuse it -- that is not the
            // failure this arm is here to show.
            aa.push("--max-root-floor-ms".into());
            aa.push("1000000".into());
        }
        let work = bank.join(format!("root{}", clock.map(|c| c.to_string()).unwrap_or_else(|| "ladder".into())));
        let _ = std::fs::remove_dir_all(&work);
        let (mut env, _rig, tape) = build_env(&p, &aa, &work);
        env.reset().unwrap_or_else(|e| die(e));
        let r = env.core.last_row();
        let pos = [r.x as f32, r.y as f32, r.z as f32];
        let speed = (r.vx * r.vx + r.vy * r.vy + r.vz * r.vz).sqrt();
        let d = tmenv::geom::norm(tmenv::geom::sub(pos, fix.pos));
        let gates = env.core.gates_hit();
        let c1 = d <= tol;
        let c2 = speed <= vmax;
        let c3 = gates == 0;
        println!(
            "  reset at ({:.2}, {:.2}, {:.2})  race {:.3}  speed {:.2} m/s  gates {}",
            pos[0], pos[1], pos[2], r.time_ms as f64 / 1000.0, speed, gates
        );
        println!("  1 position within {tol:.1} m of the measured start : {:.2} m   {}", d, yn(c1));
        println!("  2 speed at or below {vmax:.1} m/s                    : {:.2}    {}", speed, yn(c2));
        println!("  3 no checkpoint already collected                 : {}       {}", gates, yn(c3));

        // ---- clause 4, on the env's root only: drive, write, reconstruct.
        let mut c4 = false;
        if clock.is_none() {
            let fwd = ActionSpace::default().forward();
            let (steps, _ret, done) = drive(&mut env, &|_| fwd, num(a, "--steps", 200));
            let rec = env.rollout_record();
            let cand = bank.join("scripted-from-the-start.Ghost.Gbx");
            let (s, g, b) = env.faithful_tape(&tape);
            tape.write_candidate(&s, &g, &b, &cand).unwrap_or_else(|e| die(e));
            bank_traj(&bank.join("env-trajectory.csv"), &rec.trace);
            println!();
            println!("  scripted full-throttle run: {steps} steps, done {done:?}, best s {:.1} m", env.core.best_s());
            println!("  tape {}", cand.display());

            let batch = tmauto::oracle::validate_raw(
                &p.server, &[cand.clone()], tmauto::oracle::Maps::One(&p.map), "acc",
            )
            .unwrap_or_else(|e| die(e));
            let _ = std::fs::write(bank.join("oracle-transcript.json"), &batch.raw);
            let _ = std::fs::write(bank.join("oracle-stderr.txt"), &batch.err);
            let ans = batch.answers.first();
            println!(
                "  PLAIN ORACLE on the written tape: {:?}  ({})",
                ans.and_then(|x| x.verdict()),
                ans.map(|x| x.desc.trim().to_string()).unwrap_or_else(|| "no answer".into())
            );

            let ticks: u64 = rec.spans.last().map(|s| (s.from + s.k) as u64).unwrap_or(0) + 300;
            let flat = control::flat_trace(&p.server, &p.map, &p.shim, &bank.join("flat"), &cand, ticks)
                .unwrap_or_else(|e| die(e));
            bank_traj(&bank.join("oracle-reconstruction.csv"), &flat);
            // THE POSITIVE CONTROL ON THE RECONSTRUCTION ITSELF. Before reading a
            // disagreement as "the stepping is wrong", ask what else could
            // produce it: run the SAME tape through the SAME flat path twice.
            // If two flat runs of one file already differ, the instrument has
            // that much noise in it and clause 4 cannot resolve anything finer.
            let flat2 = control::flat_trace(&p.server, &p.map, &p.shim, &bank.join("flat-again"), &cand, ticks)
                .unwrap_or_else(|e| die(e));
            let selfcmp = control::compare(&flat, &flat2, num(a, "--tol-traj", 0.02f64));
            println!("  4a the SAME tape, reconstructed TWICE                : {selfcmp}");
            println!("     (the instrument's own noise floor; a disagreement below this cannot");
            println!("      be read as a fault in the stepping)");
            bank_traj(&bank.join("oracle-reconstruction-2.csv"), &flat2);
            let cmp = control::compare(&rec.trace, &flat, num(a, "--tol-traj", 0.02f64));
            println!("  4 fresh-process reconstruction agrees             : {cmp}");
            // Judged against the instrument's own floor, not against zero.
            c4 = cmp.same(num(a, "--tol-traj", 0.02f64))
                || (selfcmp.over_tol > 0
                    && cmp.over_tol <= selfcmp.over_tol
                    && cmp.max_pos_err <= selfcmp.max_pos_err * 1.5);
            if !cmp.same(num(a, "--tol-traj", 0.02f64)) && c4 {
                println!("     PASS AT THE NOISE FLOOR: the stepped run differs from a flat one by");
                println!("     no more than two flat runs differ from each other. That is a");
                println!("     statement about the ENGINE, not about the environment.");
            }
            println!("                                                      {}", yn(c4));
            ok_new = c1 && c2 && c3 && c4;
        } else {
            let broken = !(c1 && c2 && c3);
            println!();
            println!(
                "  the old root {}  {}",
                if broken { "FAILS clauses 1-3, as it must" } else { "PASSES -- so this control cannot fail and certifies nothing" },
                yn(broken)
            );
            if !broken {
                die("the reset control passed on the setup it was written to catch".into());
            }
        }
    }

    println!();
    println!("ACCEPTANCE  {}", if ok_new { "PASS" } else { "FAIL -- do not train on this env" });
    if !ok_new {
        std::process::exit(1);
    }
}

fn yn(b: bool) -> &'static str {
    if b {
        "PASS"
    } else {
        "FAIL"
    }
}

fn bank_traj(path: &Path, rows: &[forkoracle::layout::Row]) {
    let mut s = String::from("time_ms,x,y,z,vx,vy,vz,qx,qy,qz,qw,wetness\n");
    for r in rows {
        s.push_str(&format!(
            "{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.6},{:.6},{:.6},{:.6},{:.4}\n",
            r.time_ms, r.x, r.y, r.z, r.vx, r.vy, r.vz, r.qx, r.qy, r.qz, r.qw, r.wetness
        ));
    }
    let _ = std::fs::write(path, s);
}

// ------------------------------------------------------- template-control

/// Prove the container-template boundary: RL code cannot reach the donor's
/// driving.
///
/// The ruling allows a game-recorded ghost as an OPAQUE wrapper and
/// startup-state template, and forbids learning from or inspecting its inputs
/// or trajectory. This is the runtime half of that boundary; the compile-time
/// half is `tmenv::template::Template`, whose donor bytes are private and which
/// exposes no accessor for input channels, samples or raw bytes.
///
/// Note what this does NOT do: it never reads the donor's inputs. A test that
/// had to read them to prove they had not leaked would be the leak. Instead it
/// proves the written archive IS ours, tick for tick, over its whole length --
/// and an archive fully determined by its ticks that equals ours contains none
/// of anybody else's.
fn template_control(a: &[String]) {
    let p = paths(a);
    let tpl = match flag(a, "--template") {
        Some(t) => PathBuf::from(t),
        None => {
            println!("# tmenv template-control");
            println!();
            println!("UNMEASURED: no --template given.");
            println!();
            println!("This control needs a game-recorded container for the map. Without one it");
            println!("cannot run, and it says so rather than passing vacuously -- a boundary test");
            println!("that reports PASS when it examined nothing is the worst kind of green.");
            std::process::exit(2);
        }
    };
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));

    println!("# tmenv template-control -- the donor is a WRAPPER, never a driver");
    println!();
    let t = tmenv::template::Template::load(&tpl).unwrap_or_else(|e| die(e));
    let f = t.facts();
    println!("template     {}", f.origin);
    println!("ticks        {}  (a property of the file's length, not of how it was driven)", f.ticks);
    println!();

    // Two input sets of our own, differing everywhere.
    let n = f.ticks;
    let mine_s: Vec<u8> = (0..n).map(|i| ((i as i32 * 7 % 61) - 30) as i8 as u8).collect();
    let mine_g: Vec<u8> = (0..n).map(|i| (i % 3 != 0) as u8).collect();
    let mine_b: Vec<u8> = (0..n).map(|i| (i % 17 == 0) as u8).collect();
    let other_s: Vec<u8> = (0..n).map(|i| ((i as i32 * 11 % 41) - 20) as i8 as u8).collect();
    let other_g: Vec<u8> = (0..n).map(|i| (i % 5 != 0) as u8).collect();
    let other_b: Vec<u8> = (0..n).map(|i| (i % 23 == 0) as u8).collect();

    let a_path = p.work.join("template-ours.Ghost.Gbx");
    let b_path = p.work.join("template-other.Ghost.Gbx");
    t.write_with_inputs(&mine_s, &mine_g, &mine_b, &a_path).unwrap_or_else(|e| die(e));
    t.write_with_inputs(&other_s, &other_g, &other_b, &b_path).unwrap_or_else(|e| die(e));

    // Decode what was written. Reading OUR OWN output is not a ghost read.
    let ra = fk::tape::Tape::load(&a_path.to_string_lossy()).unwrap_or_else(|e| die(e));
    let rb = fk::tape::Tape::load(&b_path.to_string_lossy()).unwrap_or_else(|e| die(e));

    let len_ok = ra.n() == n;
    println!("1 the written archive is exactly our tape's length   : {} vs {}   {}", ra.n(), n, yn(len_ok));

    let same = ra.steer == mine_s && ra.accel == mine_g && ra.brake == mine_b;
    let first_bad = (0..ra.n().min(n))
        .find(|&i| ra.steer[i] != mine_s[i] || ra.accel[i] != mine_g[i] || ra.brake[i] != mine_b[i]);
    println!(
        "2 it decodes to OUR inputs, tick for tick               : {}   {}",
        match first_bad {
            None => "all ticks".to_string(),
            Some(i) => format!("first difference at tick {i}"),
        },
        yn(same)
    );
    println!("   (an archive is fully determined by its ticks, so an output that equals ours");
    println!("    carries none of the donor's -- proven without ever reading the donor's)");

    // THE NEGATIVE HALF: the comparison must be able to fail.
    let differs = rb.steer != ra.steer || rb.accel != ra.accel || rb.brake != ra.brake;
    println!(
        "3 a DIFFERENT input set decodes differently             : {}",
        yn(differs)
    );
    println!("   (without this, clause 2 is satisfied by a comparison that cannot fail)");

    // A short tape must be REFUSED, not padded from the donor.
    let short = t.write_with_inputs(&mine_s[..n / 2], &mine_g[..n / 2], &mine_b[..n / 2], &p.work.join("short.Ghost.Gbx"));
    let refused = short.is_err();
    println!(
        "4 a SHORT tape is refused, never padded from the donor  : {}",
        yn(refused)
    );
    if let Err(e) = &short {
        println!("   {}", e.lines().next().unwrap_or(""));
    }

    println!();
    let all = len_ok && same && differs && refused;
    println!("BOUNDARY     {}", if all { "PASS -- the donor contributes the wrapper and no driving" } else { "FAIL" });
    if !all {
        std::process::exit(1);
    }
}

// ------------------------------------------------------------------ accept

/// THE ACCEPTANCE BATTERY, on the validator-owned resolver and an opaque
/// game-recorded container.
///
/// Four controls, run live, every artefact banked:
///
/// * **START** — the car the validator owns begins at the map's own
///   `RoadTechStart`, not 389 m away at CP3.
/// * **MIRROR** — hard-left and hard-right tapes produce opposite signed
///   lateral responses. This is what says the resolved object *consumes our
///   input*; a start check alone is satisfied by an object that sits still.
/// * **MOVED START** — move the map's `RoadTechStart` 64 m and the car must
///   move with it. This is the one that discriminates, and it is the reason the
///   other two are not enough: a recorded container OF THIS MAP starts at this
///   map's start, so "the container decides" and "the map decides" predict the
///   same thing until the map is changed underneath it.
/// * **ORACLE** — the plain oracle simulates the written tape.
fn accept(a: &[String]) {
    let p = paths(a);
    let tplp = PathBuf::from(
        flag(a, "--template").unwrap_or_else(|| die("--template FILE.Ghost.Gbx is required".into())),
    );
    let bank = PathBuf::from(flag(a, "--bank").unwrap_or_else(|| "/tmp/tmenv/accept".into()));
    std::fs::create_dir_all(&bank).unwrap_or_else(|e| die(e.to_string()));
    let tol: f32 = num(a, "--tol", 8.0);
    let vmax: f64 = num(a, "--vmax", 4.0);

    println!("# tmenv accept -- validator-owned car, opaque recorded container");
    println!("# template  {}", tplp.display());
    println!("# banking   {}", bank.display());
    println!();

    let tpl = tmenv::template::Template::load(&tplp).unwrap_or_else(|e| die(e));
    let n = tpl.facts().ticks;
    println!("template ticks {n}  ({:.3} s of tape)", n as f64 * 0.01);

    // The map's own start, read from the map file. Not from a MapPack.
    let mf = tmmaps::map::MapFile::load(&p.map);
    let mut store = open_store(&p.server).unwrap_or_else(|e| die(e));
    let yoff = {
        let mut asm = mapgeom::assemble::Assembler::new(&mut store);
        let _ = asm.with_embedded(&mf);
        let (gs, _) = asm.map_split(&mf);
        mapgeom::yoff::measure(&mf, &gs).value().unwrap_or_else(|| die("map height UNMEASURED".into()))
    };
    let wps = mapgeom::pack::gates(&mf, yoff);
    let spawn = wps.iter().find(|g| g.tag == "Spawn").map(|g| g.pos)
        .unwrap_or_else(|| die("no Spawn waypoint in the map".into()));
    println!("map RoadTechStart ({:.1}, {:.1}, {:.1})", spawn[0], spawn[1], spawn[2]);
    println!();

    // One tape writer: full input replacement through the boundary type.
    // THE CONSTANT-STEER TRAP. The shim finds the engine's decoded input array
    // by searching for the reference's steer sequence as f32 at stride 32. A
    // constant channel of ZERO gives it nothing to lock onto and it matches the
    // first stretch of zeroes it meets -- the identity control then reports
    // "2109 of 2109 ticks differ", which reads like a container fault and is
    // really a locate that matched the wrong memory. A constant of +-127 is
    // distinctive enough to survive it; zero is not. So the straight arm gets
    // the project's jitter pattern, which is varied enough to lock onto and
    // gentle enough that the car goes essentially straight.
    let write = |steer: Option<i8>, tag: &str| -> PathBuf {
        let s: Vec<u8> = match steer {
            Some(v) => vec![v as u8; n],
            None => (0..n).map(|t| ((((t as i64 * 7919 + 13) % 25) - 12) as i8) as u8).collect(),
        };
        let g = vec![1u8; n];
        let b = vec![0u8; n];
        let out = bank.join(format!("{tag}.Ghost.Gbx"));
        tpl.write_with_inputs(&s, &g, &b, &out).unwrap_or_else(|e| die(e));
        out
    };
    // Resolve the car on a map+container pair and report its opening state.
    let start_of = |mapp: &Path, cand: &Path, tag: &str| -> Option<(f32, [f32; 3], f64)> {
        let work = bank.join(format!("work-{tag}"));
        let _ = std::fs::remove_dir_all(&work);
        match control::measure_spawn(&p.server, mapp, &p.shim, &work, cand) {
            Err(e) => {
                println!("  {tag}: UNMEASURED -- {}", e.lines().next().unwrap_or(""));
                None
            }
            Ok((fix, rows)) => {
                bank_traj(&bank.join(format!("{tag}-trajectory.csv")), &rows);
                Some((fix.race_ms as f32 / 1000.0, fix.pos, fix.speed))
            }
        }
    };

    // ---- 1. START
    println!("--- CONTROL 1: START (the car begins at the map's RoadTechStart)");
    let straight = write(None, "straight");
    let s1 = start_of(&p.map, &straight, "straight");
    let mut c1 = false;
    if let Some((race, pos, sp)) = s1 {
        let d = tmenv::geom::norm(tmenv::geom::sub(pos, spawn));
        println!("  resolved start ({:.2}, {:.2}, {:.2})  race {race:.3}  speed {sp:.2} m/s", pos[0], pos[1], pos[2]);
        println!("  distance to RoadTechStart  {d:.2} m   (want <= {tol:.1})");
        println!("  speed                      {sp:.2} m/s (want <= {vmax:.1})");
        c1 = d <= tol && sp <= vmax;
    }
    println!("  {}", yn(c1));

    // ---- 2. MIRROR
    println!();
    println!("--- CONTROL 2: MIRROR (the resolved object consumes OUR input)");
    println!("    a start check alone is satisfied by an object that sits still");
    let left = write(Some(-127), "hardleft");
    let right = write(Some(127), "hardright");
    let sl = start_of(&p.map, &left, "hardleft");
    let sr = start_of(&p.map, &right, "hardright");
    let mut c2 = false;
    if let (Some(l), Some(r)) = (sl, sr) {
        // Compare after the run has developed, not at tick 0 where both are the
        // same standing car. The trajectories are banked; read them back.
        let lt = read_traj(&bank.join("hardleft-trajectory.csv"));
        let rt = read_traj(&bank.join("hardright-trajectory.csv"));
        let k = lt.len().min(rt.len()).saturating_sub(1);
        if k > 20 {
            let (lx, rx) = (lt[k][0], rt[k][0]);
            let (lz, rz) = (lt[k][2], rt[k][2]);
            println!("  after {k} ticks:  hard-left ({lx:.3}, {lz:.3})   hard-right ({rx:.3}, {rz:.3})");
            let sep = ((lx - rx).powi(2) + (lz - rz).powi(2)).sqrt();
            println!("  separation {sep:.3} m   (want > 0.5, and they must lie on OPPOSITE sides)");
            // Opposite sides of the straight-ahead run.
            let st = read_traj(&bank.join("straight-trajectory.csv"));
            if k < st.len() {
                let dl = lx - st[k][0];
                let dr = rx - st[k][0];
                println!("  lateral vs straight:  left {dl:+.3} m   right {dr:+.3} m");
                c2 = sep > 0.5 && dl * dr < 0.0;
            }
        }
        let _ = (l, r);
    }
    println!("  {}", yn(c2));

    // ---- 3. MOVED START -- the discriminating one
    println!();
    println!("--- CONTROL 3: MOVED START (move the map's start; the car must follow)");
    println!("    THE one that discriminates. A recorded container OF THIS MAP starts at");
    println!("    this map's start, so 'the container decides' and 'the map decides' agree");
    println!("    until the map is changed underneath it.");
    let bi = mf.blocks.iter().position(|b| b.name.contains("Start"));
    let mut c3 = false;
    match bi {
        None => println!("  no Start BLOCK in the map (item-placed?); UNMEASURED"),
        Some(bi) => {
            let c = mf.blocks[bi].coords();
            let moved_cell = (c.0 - 2, c.1, c.2); // 2 cells = 64 m west
            let movedmap = bank.join("moved-start.Map.Gbx");
            let st = std::process::Command::new(
                "/tmp/tmtas/tools/target/release/tmmaps",
            )
            .arg("move")
            .arg(&p.map)
            .arg("--out")
            .arg(&movedmap)
            .arg("--move")
            .arg(format!("{bi}:{},{},{}", moved_cell.0, moved_cell.1, moved_cell.2))
            .output();
            match st {
                Err(e) => println!("  could not run tmmaps move: {e}"),
                Ok(o) if !o.status.success() => {
                    println!("  tmmaps move failed: {}", String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or(""))
                }
                Ok(o) => {
                    println!("  {}", String::from_utf8_lossy(&o.stdout).lines().find(|l| l.contains("->")).unwrap_or("moved").trim());
                    let s3 = start_of(&movedmap, &straight, "movedstart");
                    if let (Some((_, p0, _)), Some((_, p1, _))) = (s1, s3) {
                        let shift = tmenv::geom::norm(tmenv::geom::sub(p1, p0));
                        println!("  start on the ORIGINAL map ({:.2}, {:.2}, {:.2})", p0[0], p0[1], p0[2]);
                        println!("  start on the MOVED map    ({:.2}, {:.2}, {:.2})", p1[0], p1[1], p1[2]);
                        println!("  the car moved {shift:.2} m   (want ~64; the synthetic container gave 0.00)");
                        c3 = shift > 32.0;
                    }
                }
            }
        }
    }
    println!("  {}", yn(c3));

    // ---- 4. ORACLE
    println!();
    println!("--- CONTROL 4: the plain oracle simulates the written tape");
    let batch = tmauto::oracle::validate_raw(
        &p.server, &[straight.clone()], tmauto::oracle::Maps::One(&p.map), "acc",
    )
    .unwrap_or_else(|e| die(e));
    let _ = std::fs::write(bank.join("oracle-transcript.json"), &batch.raw);
    let _ = std::fs::write(bank.join("oracle-stderr.txt"), &batch.err);
    let ans = batch.answers.first();
    let c4 = ans.map(|x| x.simulated()).unwrap_or(false);
    println!("  verdict {:?}   simulated {}", ans.and_then(|x| x.verdict()), c4);
    println!("  desc    {}", ans.map(|x| x.desc.trim().to_string()).unwrap_or_default());
    println!("  {}", yn(c4));

    println!();
    println!("ACCEPTANCE  START {}   MIRROR {}   MOVED-START {}   ORACLE {}",
        yn(c1), yn(c2), yn(c3), yn(c4));
    let all = c1 && c2 && c3 && c4;
    println!("            {}", if all { "PASS" } else { "FAIL -- do not train" });
    if !all {
        std::process::exit(1);
    }
}

fn read_traj(p: &Path) -> Vec<[f64; 3]> {
    let mut v = Vec::new();
    if let Ok(t) = std::fs::read_to_string(p) {
        for (i, l) in t.lines().enumerate() {
            if i == 0 {
                continue;
            }
            let c: Vec<&str> = l.split(',').collect();
            if c.len() > 3 {
                v.push([
                    c[1].parse().unwrap_or(f64::NAN),
                    c[2].parse().unwrap_or(f64::NAN),
                    c[3].parse().unwrap_or(f64::NAN),
                ]);
            }
        }
    }
    v
}

// -------------------------------------------------------------- from-template

/// Write the env's reference container from an opaque recorded template.
///
/// The env forks on a reference; that reference must be OURS. This replaces the
/// donor's archive in full with the project's jitter pattern — varied enough for
/// the shim to lock onto, gentle enough that the car goes essentially straight,
/// so the run lasts long enough to branch through.
fn from_template(a: &[String]) {
    let p = paths(a);
    let tplp = PathBuf::from(
        flag(a, "--template").unwrap_or_else(|| die("--template FILE.Ghost.Gbx is required".into())),
    );
    let out = PathBuf::from(flag(a, "--out").unwrap_or_else(|| "/tmp/tmenv/reference.Ghost.Gbx".into()));
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d).unwrap_or_else(|e| die(e.to_string()));
    }
    let tpl = tmenv::template::Template::load(&tplp).unwrap_or_else(|e| die(e));
    let n = tpl.facts().ticks;
    let s: Vec<u8> = (0..n).map(|t| ((((t as i64 * 7919 + 13) % 25) - 12) as i8) as u8).collect();
    let g = vec![1u8; n];
    let b = vec![0u8; n];
    tpl.write_with_inputs(&s, &g, &b, &out).unwrap_or_else(|e| die(e));
    println!("template  {}  ({n} ticks, {:.3} s)", tplp.display(), n as f64 * 0.01);
    println!("reference {}", out.display());
    println!("the archive is OURS in full; the donor contributes the wrapper and the startup state");
}

// ------------------------------------------------------------- probe-scan

/// How stable is the boundary probe at a given `lroundf` stop?
///
/// Starts `--repeat` fresh servers per clock, probes each one `--reprobe`
/// times, and prints beside every probe the RAW clock word of the first
/// sampled row (the clock the layout reads, before any bias). The engine is
/// deterministic, so the raw clock at a given stop must be constant; a probe
/// that varies while the raw clock does not is the probe mislabelling the
/// tick, and everything labelled from it is off by the same amount.
fn probe_scan(a: &[String]) {
    let p = paths(a);
    let clocks: Vec<u64> = flag(a, "--clocks")
        .unwrap_or_else(|| "11000".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let repeat: usize = num(a, "--repeat", 3);
    let reprobe: usize = num(a, "--reprobe", 3);
    std::fs::create_dir_all(&p.work).unwrap_or_else(|e| die(e.to_string()));
    println!("# tmenv probe-scan  clocks {:?}  repeat {}  reprobe {}", clocks, repeat, reprobe);
    println!("clock    run  probes                 raw_clock  race_label  pos                          speed");
    for c in &clocks {
        for r in 0..repeat {
            let work = p.work.join(format!("ps-{c}-{r}"));
            let rig = Rig::new(&p.server, &p.map, &p.shim, &work, &p.reference).unwrap_or_else(|e| die(e));
            let mut s = rig.session_clock(*c).unwrap_or_else(|e| die(e));
            let mut probes = Vec::new();
            for _ in 0..reprobe {
                probes.push(s.probe_tick().unwrap_or_else(|e| die(e)));
            }
            let probe = probes[0];
            let recs = s.tape.tail_records(0);
            let car = control::resolve_car(&mut s.srv, probe, &recs, s.tape.start_offset_ms, false)
                .unwrap_or_else(|e| die(e));
            let bias = car.layout().clock_bias;
            let start_offset = s.tape.start_offset_ms;
            let dir = work.join("traces");
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| die(e.to_string()));
            let cfg = branch::TraceCfg { layout: car.layout().clone(), dir, stride: 1, max: 4000 };
            let fk::session::Session { srv, .. } = s;
            let mut f = branch::Forest::new(srv, &work, recs, Some(cfg)).unwrap_or_else(|e| die(e));
            f.probe_root().unwrap_or_else(|e| die(e));
            let cal = f.calibrate_clock(tmenv::forkenv::CLOCK_WARM_TICKS, start_offset).unwrap_or_else(|e| die(e));
            let (rows, h) = f.advance(branch::ROOT, &[], 0, 2).unwrap_or_else(|e| die(e));
            f.release(h);
            match rows.first() {
                Some(row) => {
                    let v = (row.vx * row.vx + row.vy * row.vy + row.vz * row.vz).sqrt();
                    println!(
                        "{:6}  {:3}  {:<22} {:9}  {:10}  ({:8.2}, {:6.2}, {:8.2})  {:6.2}   cal: probe {} raw {} bias {} (root-implied {})",
                        c, r, format!("{:?}", probes), row.time_ms + cal.bias, row.time_ms, row.x, row.y, row.z, v, cal.probe, cal.raw_clock, cal.bias, bias
                    );
                }
                None => println!("{:6}  {:3}  {:<22} (no rows)", c, r, format!("{:?}", probes)),
            }
        }
    }
}

// ------------------------------------------------------------- geom-export

/// Write the track the env would run on as `geom.json` (`tmstate::TrackGeom`),
/// so the DATA arm's geometry and the cartographer's can be compared on equal
/// terms, and so a box can run without the pak store.
fn geom_export(a: &[String]) {
    let p = paths(a);
    let out = PathBuf::from(flag(a, "--out").unwrap_or_else(|| "geom.json".into()));
    let t = if has(a, "--measured") {
        measured_track(&p).unwrap_or_else(|e| die(e))
    } else if let Some(g) = &p.geom {
        tmenv::Track::load_geom_json(g).unwrap_or_else(|e| die(e))
    } else {
        load_track(&p.server, &p.map).unwrap_or_else(|e| die(e))
    };
    t.save_geom_json(&out).unwrap_or_else(|e| die(e));
    let g = &t.geom;
    println!(
        "wrote {}  ({} points, {:.1} m, {} gates, source {}, spawn ({:.1}, {:.1}, {:.1}))",
        out.display(),
        g.pts.len(),
        g.length(),
        g.gates.len(),
        g.source,
        g.spawn[0],
        g.spawn[1],
        g.spawn[2]
    );
}
