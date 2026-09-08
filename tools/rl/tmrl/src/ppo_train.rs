//! PPO over the TM2020 environment — the a7aa56c trainer, ported by the ENV arm (`tmrl ppo-selftest`, `tmrl train`).
//! Lives beside the BC trainer (`crate::bc`) until L2 unifies the two on one network (`crate::bcnet`).
//!
//! # The standing rule this binary obeys
//!
//! Nothing it prints about a policy's performance is a result. A rollout's
//! reward is the environment's own reading; a *result* is a tape written to
//! disk that the **plain oracle** re-simulates. So every time the policy sets a
//! new best, the trainer writes the tape and validates it, and reports the
//! oracle's verdict beside its own — never instead of it.

use crate::net;
use crate::ppo;
use candle_core::{DType, Device, Tensor};
use candle_nn::{loss, Optimizer, ParamsAdamW};
use net::{sample, softmax, Trainable, Weights};
use ppo::{assemble, GaeCfg, PpoCfg, Step};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;
use tmenv::action::ActionSpace;
use tmenv::core::{CoreCfg, Done};
use tmenv::forkenv::{ForkEnv, Rig};
use tmenv::control;

fn flag(a: &[String], k: &str) -> Option<String> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned()
}
fn num<T: std::str::FromStr>(a: &[String], k: &str, d: T) -> T {
    flag(a, k).and_then(|v| v.parse().ok()).unwrap_or(d)
}
fn die(e: String) -> ! {
    eprintln!("tmrl: {e}");
    std::process::exit(1)
}
fn secs(ms: i64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

struct Rng(u64);
impl Rng {
    fn f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / ((1u32 << 24) as f32)
    }
}

pub fn main(a: &[String]) {
    match a.first().map(|s| s.as_str()) {
        Some("ppo-selftest") => selftest(),
        Some("train") => train(a),
        _ => {
            eprintln!(
                "tmrl -- PPO over the TM2020 environment\n\
                 \n\
                   tmrl selftest\n\
                        The network's controls: the rollout forward against the training\n\
                 forward, and PPO's advantage arithmetic. No engine needed.\n\
                 \n\
                   tmrl train --map M --ref R [--workers N] [--iters N] [--steps-per-worker N]\n\
                              [--k N] [--max-ticks N] [--lr F] [--ent F] [--out DIR]\n\
                 \n\
                 Common: --server DIR ($TM_SERVER)  --shim SO ($FK_SHIM)  --work DIR"
            );
            std::process::exit(2)
        }
    }
}

fn selftest() {
    let dev = Device::Cpu;
    println!("# tmrl selftest");
    println!();
    let t = Trainable::new(77, 20, &dev).unwrap_or_else(|e| die(e.to_string()));
    let w = t.snapshot().unwrap_or_else(|e| die(e.to_string()));
    println!("--- the two forwards must agree");
    println!("    Training uses candle (autograd); rollout uses a flat-weight hand-written");
    println!("    forward, so a hundred workers need no locks and no allocation per step.");
    println!("    Two implementations of one function is how this project got silent");
    println!("    corruption before, so they are checked rather than trusted.");
    match w.agrees_with(&t, &dev, 64, 1e-4) {
        Ok(worst) => println!("    PASS  worst |difference| over 64 random inputs: {worst:.3e}"),
        Err(e) => die(e),
    }
    // The negative half: a forward that has drifted must be CAUGHT. Without
    // this, "the two agree" is satisfied by a check that cannot fail.
    let mut bad = w.clone();
    bad.perturb_first_bias(0.01);
    match bad.agrees_with(&t, &dev, 64, 1e-4) {
        Ok(x) => die(format!(
            "the agreement check passed a policy whose weights were deliberately changed \
             (worst {x:.3e}); it cannot fail and certifies nothing"
        )),
        Err(_) => println!("    PASS  a deliberately perturbed copy is REFUSED (the check can fail)"),
    }
    println!();
    println!("--- PPO's advantage arithmetic");
    println!("    (cargo test -p tmrl runs these; the sharp one is that a TRUNCATED episode");
    println!("     bootstraps its value and a TERMINATED one does not -- conflating them");
    println!("     teaches the policy that running out of tape is death)");
    println!("    run `cargo test --release -p tmrl` for the three known-answer checks");
}

/// A worker's private world.
struct Worker {
    env: ForkEnv,
    rng: Rng,
}

fn build_worker(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    reference: &Path,
    track: std::sync::Arc<tmenv::Track>,
    cfg: CoreCfg,
    seed: u64,
    start: Option<[f32; 3]>,
) -> Result<(Worker, Rig, fk::tape::Tape), String> {
    // The SHARED root selection: ladder, wide locate, coherence gate. One
    // implementation, in the library, because a trainer whose root differs from
    // the one the acceptance control certified is training on a different
    // environment than the one that passed.
    let mut root = tmenv::forkenv::RootCfg { verbose: false, ..Default::default() };
    root.require_start = start.map(|s| (s, 6.0f32, 4.0f64));
    let (env, rig, tape) = tmenv::forkenv::build_at_start(
        server, map, shim, work, reference, track, ActionSpace::default(), cfg, &root,
    )?;
    Ok((Worker { env, rng: Rng(seed | 1) }, rig, tape))
}

/// What one worker sends back per iteration.
struct Harvest {
    episodes: Vec<Vec<Step>>,
    steps: usize,
    /// The best episode this worker saw, by furthest saturated progress, with
    /// the tape it drove.
    best: Option<(f32, usize, Option<i64>, Vec<u8>, Vec<u8>, Vec<u8>)>,
    dones: [usize; 4],
}

fn run_episode(w: &mut Worker, weights: &Weights, max_steps: usize) -> (Vec<Step>, Option<Done>) {
    let mut out = Vec::with_capacity(max_steps);
    let mut obs = match w.env.reset() {
        Ok(o) => o,
        Err(_) => return (out, None),
    };
    let mut done = None;
    for _ in 0..max_steps {
        let (mut logits, value) = weights.forward(&obs);
        softmax(&mut logits);
        let a = sample(&logits, w.rng.f32());
        let logp = logits[a].max(1e-8).ln();
        let (next_obs, reward, d, _info) = match w.env.step(a) {
            Ok(v) => v,
            Err(_) => break,
        };
        let (_, next_value) = weights.forward(&next_obs);
        // TERMINAL vs TRUNCATED, kept apart. A crash and a finish are the world
        // ending; the tick cap is the harness stopping, and its value must be
        // bootstrapped or the policy learns that running out of tape is death.
        let terminal = matches!(d, Some(Done::Finished) | Some(Done::OffRoute) | Some(Done::NoProgress));
        let truncated = matches!(d, Some(Done::TickCap));
        out.push(Step {
            obs: std::mem::replace(&mut obs, next_obs),
            action: a,
            logp,
            value,
            reward,
            terminal,
            truncated,
            next_value,
        });
        if let Some(d) = d {
            done = Some(d);
            break;
        }
    }
    if done.is_none() {
        // Cut by the step budget rather than by the world: bootstrap it.
        if let Some(l) = out.last_mut() {
            l.truncated = true;
        }
    }
    (out, done)
}

fn train(a: &[String]) {
    let server = PathBuf::from(
        flag(a, "--server").or_else(|| std::env::var("TM_SERVER").ok()).unwrap_or_else(|| "/tmp/tmoracle/server".into()),
    );
    let shim = PathBuf::from(
        flag(a, "--shim").or_else(|| std::env::var("FK_SHIM").ok())
            .unwrap_or_else(|| "/tmp/tmtas/tools/search/target/release/libforkshim.so".into()),
    );
    let map = PathBuf::from(flag(a, "--map").unwrap_or_else(|| die("--map is required".into())));
    let reference = PathBuf::from(flag(a, "--ref").unwrap_or_else(|| die("--ref is required".into())));
    let work = PathBuf::from(flag(a, "--work").unwrap_or_else(|| format!("/dev/shm/tmrl/{}", std::process::id())));
    tmenv::warn_if_not_tmpfs(&work);
    let out = PathBuf::from(flag(a, "--out").unwrap_or_else(|| "/tmp/tmrl/out".into()));
    let workers: usize = num(a, "--workers", 32);
    let iters: usize = num(a, "--iters", 200);
    let steps_per_worker: usize = num(a, "--steps-per-worker", 256);
    let max_ep: usize = num(a, "--max-episode-steps", 220);
    let k: usize = num(a, "--k", 10);
    let max_ticks: usize = num(a, "--max-ticks", 2400);
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(e.to_string()));
    std::fs::create_dir_all(&work).unwrap_or_else(|e| die(e.to_string()));

    let mut pcfg = PpoCfg { lr: num(a, "--lr", 3e-4), ent_coef: num(a, "--ent", 0.01f32), ..Default::default() };
    let gcfg = GaeCfg::default();

    println!("# tmrl train -- PPO over the TM2020 environment");
    println!("# map {}", map.display());
    println!("# {workers} workers, {steps_per_worker} steps each per iteration, {iters} iterations");
    println!("# k = {k} ticks per action, episode cap {max_ep} actions / {max_ticks} ticks");
    println!();

    // The track, once, with the tour solved from where the engine ACTUALLY
    // starts the car (see tmenv::load_track_measured).
    let t0 = Instant::now();
    let track = std::sync::Arc::new(
        tmenv::load_track_measured(&server, &map, &shim, &work.join("spawnfix"), &reference)
            .unwrap_or_else(|e| die(e)),
    );
    println!("track        {:.1} m, {} gates, built in {:.1} s", track.length(), track.n_gates(), t0.elapsed().as_secs_f64());
    if let Some(au) = track.author_s() {
        println!("author time  {au:.3}   (the target, and never a reference line)");
    }

    let ccfg = CoreCfg { k_ticks: k, max_ticks, ..Default::default() };
    let start = tmenv::measured_spawn(&server, &map, &shim, &work.join("spawnfix"), &reference)
        .unwrap_or_else(|e| die(format!("the start is UNMEASURED, so no env can be certified: {e}")));
    let start = Some(start);
    println!("start        ({:.2}, {:.2}, {:.2})  -- every worker must root here", start.unwrap()[0], start.unwrap()[1], start.unwrap()[2]);
    let obs_dim = {
        let c = tmenv::Core::new(ccfg.clone(), track.clone(), ActionSpace::default());
        c.obs_dim()
    };
    let n_actions = ActionSpace::default().n();
    println!("obs dim      {obs_dim}");
    println!("actions      {n_actions}");

    let dev = Device::Cpu;
    let model = Trainable::new(obs_dim, n_actions, &dev).unwrap_or_else(|e| die(e.to_string()));
    // The control, before a single sample is collected: the forward the workers
    // will run must be the forward the update differentiates.
    match model.snapshot().and_then(|w| Ok(w.agrees_with(&model, &dev, 32, 1e-4))) {
        Ok(Ok(worst)) => println!("forward      rollout == training to {worst:.2e} over 32 random inputs"),
        Ok(Err(e)) => die(e),
        Err(e) => die(e.to_string()),
    }
    let mut opt = candle_nn::AdamW::new(
        model.varmap.all_vars(),
        ParamsAdamW { lr: pcfg.lr, ..Default::default() },
    )
    .unwrap_or_else(|e| die(e.to_string()));

    // ---- stand the fleet up. IN PARALLEL, and that is not a micro-optimisation:
    // a worker costs a server launch plus a car locate, and the locate now
    // sweeps up to 600 windows so the root can sit at tick 0. Measured at 71-96 s
    // per worker; serially that is an hour and a half of startup for a 56-worker
    // fleet before a single gradient step.
    println!();
    println!("standing up {workers} workers in parallel (server launch + car locate each)...");
    let t0 = Instant::now();
    let mut fleet: Vec<(Worker, Rig, fk::tape::Tape)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..workers)
            .map(|w| {
                let (server, map, shim, reference, work) =
                    (&server, &map, &shim, &reference, work.join(format!("w{w}")));
                let start = start;
                let track = track.clone();
                let ccfg = ccfg.clone();
                sc.spawn(move || {
                    let seed = 0x9E3779B97F4A7C15u64.wrapping_mul(w as u64 + 1);
                    match build_worker(server, map, shim, &work, reference, track, ccfg, seed, start) {
                        Ok(x) => Some(x),
                        Err(e) => {
                            eprintln!("  worker {w} did not come up ({e}); continuing with fewer");
                            None
                        }
                    }
                })
            })
            .collect();
        hs.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    });
    if fleet.is_empty() {
        die("no worker came up".into());
    }
    println!("fleet        {} workers up in {:.1} s", fleet.len(), t0.elapsed().as_secs_f64());
    println!();
    println!("iter    steps   eps    ret     len   best_s   gates  fin  off  nop  cap   steps/s   loss");

    let mut best_s_ever = 0f32;
    let mut best_gates_ever = 0usize;

    for it in 0..iters {
        let weights = model.snapshot().unwrap_or_else(|e| die(e.to_string()));
        let t_roll = Instant::now();
        let (tx, rx) = mpsc::channel::<Harvest>();
        std::thread::scope(|sc| {
            for (wi, wk) in fleet.iter_mut().enumerate() {
                let tx = tx.clone();
                let weights = &weights;
                let _ = wi;
                sc.spawn(move || {
                    let (w, _rig, tape) = wk;
                    let mut episodes = Vec::new();
                    let mut steps = 0usize;
                    let mut best: Option<(f32, usize, Option<i64>, Vec<u8>, Vec<u8>, Vec<u8>)> = None;
                    let mut dones = [0usize; 4];
                    while steps < steps_per_worker {
                        let (ep, d) = run_episode(w, weights, max_ep.min(steps_per_worker - steps + 1));
                        if ep.is_empty() {
                            break;
                        }
                        steps += ep.len();
                        match d {
                            Some(Done::Finished) => dones[0] += 1,
                            Some(Done::OffRoute) => dones[1] += 1,
                            Some(Done::NoProgress) => dones[2] += 1,
                            _ => dones[3] += 1,
                        }
                        let bs = w.env.core.best_s();
                        let g = w.env.core.gates_hit();
                        let better = match &best {
                            None => true,
                            Some((s, gg, ..)) => g > *gg || (g == *gg && bs > *s),
                        };
                        if better {
                            let (s, ga, b) = w.env.banked_tape(tape);
                            best = Some((bs, g, w.env.rollout_record().finish_ms, s, ga, b));
                        }
                        episodes.push(ep);
                    }
                    let _ = tx.send(Harvest { episodes, steps, best, dones });
                });
            }
            drop(tx);
        });
        let harvests: Vec<Harvest> = rx.into_iter().collect();
        let roll_s = t_roll.elapsed().as_secs_f64();

        let mut episodes: Vec<Vec<Step>> = Vec::new();
        let mut total = 0usize;
        let mut dones = [0usize; 4];
        let mut best: Option<(f32, usize, Option<i64>, Vec<u8>, Vec<u8>, Vec<u8>)> = None;
        for h in harvests {
            total += h.steps;
            for i in 0..4 {
                dones[i] += h.dones[i];
            }
            if let Some(b) = h.best {
                let better = match &best {
                    None => true,
                    Some((s, g, ..)) => b.1 > *g || (b.1 == *g && b.0 > *s),
                };
                if better {
                    best = Some(b);
                }
            }
            episodes.extend(h.episodes);
        }
        if episodes.is_empty() {
            die("no episodes collected: every worker failed".into());
        }
        let n_eps = episodes.len();
        let mean_ret: f32 = episodes.iter().map(|e| e.iter().map(|s| s.reward).sum::<f32>()).sum::<f32>() / n_eps as f32;
        let mean_len: f32 = total as f32 / n_eps as f32;

        // ---- the update
        let batch = assemble(&episodes, &gcfg, obs_dim);
        let loss_val = update(&model, &mut opt, &batch, &pcfg, &dev).unwrap_or_else(|e| die(e));

        let (bs, bg) = best.as_ref().map(|b| (b.0, b.1)).unwrap_or((0.0, 0));
        println!(
            "{:4} {:8} {:5} {:7.2} {:7.1} {:8.1} {:6}  {:3}  {:3}  {:3}  {:3} {:9.0} {:7.4}",
            it, total, n_eps, mean_ret, mean_len, bs, bg, dones[0], dones[1], dones[2], dones[3],
            total as f64 / roll_s, loss_val
        );

        // ---- THE STANDING RULE: a new best is not a result until the plain
        // oracle re-simulates the tape it wrote.
        if let Some((s, g, fin, st, ga, br)) = best {
            if g > best_gates_ever || (g == best_gates_ever && s > best_s_ever + 20.0) {
                best_gates_ever = g.max(best_gates_ever);
                best_s_ever = s.max(best_s_ever);
                let (_, _, tape) = &fleet[0];
                let path = out.join(format!("iter{it:05}-g{g}-s{:.0}.Ghost.Gbx", s));
                match tape.write_candidate(&st, &ga, &br, &path) {
                    Err(e) => eprintln!("      could not write the tape: {e}"),
                    Ok(()) => {
                        let v = tmauto::oracle::validate_raw(
                            &server, &[path.clone()], tmauto::oracle::Maps::One(&map), "tmrl",
                        );
                        match v {
                            Err(e) => eprintln!("      the oracle could not be reached: {e}"),
                            Ok(b) => {
                                let ans = b.answers.first();
                                println!(
                                    "      NEW BEST  env says {g} gate(s), s = {s:.1} m{}",
                                    fin.map(|f| format!(", finish {}", secs(f))).unwrap_or_default()
                                );
                                println!(
                                    "                PLAIN ORACLE on the written tape: {:?}  ({})",
                                    ans.and_then(|x| x.verdict()),
                                    ans.map(|x| x.desc.trim().to_string()).unwrap_or_else(|| "no answer".into())
                                );
                                println!("                {}", path.display());
                            }
                        }
                    }
                }
            }
        }

        // Anneal exploration: high early because twenty actions and a long map
        // need it, low later because it is a tax on a policy that has found the
        // road.
        pcfg.ent_coef = (0.01f32 * (1.0 - it as f32 / iters as f32)).max(0.001);
    }
}

fn update(
    m: &Trainable,
    opt: &mut candle_nn::AdamW,
    b: &ppo::Batch,
    cfg: &PpoCfg,
    dev: &Device,
) -> Result<f32, String> {
    let e = |x: candle_core::Error| x.to_string();
    let n = b.n;
    let obs = Tensor::from_vec(b.obs.clone(), (n, b.obs_dim), dev).map_err(e)?;
    let act = Tensor::from_vec(b.actions.clone(), n, dev).map_err(e)?;
    let logp_old = Tensor::from_vec(b.logp_old.clone(), n, dev).map_err(e)?;
    let adv = Tensor::from_vec(b.adv.clone(), n, dev).map_err(e)?;
    let ret = Tensor::from_vec(b.ret.clone(), n, dev).map_err(e)?;

    let mut idx: Vec<u32> = (0..n as u32).collect();
    let mut rng = Rng(0xC0FFEE);
    let mut last = 0f32;
    for _ in 0..cfg.epochs {
        // Fisher-Yates over the batch.
        for i in (1..n).rev() {
            let j = (rng.f32() * (i + 1) as f32) as usize;
            idx.swap(i, j.min(i));
        }
        for chunk in idx.chunks(cfg.minibatch) {
            let sel = Tensor::from_vec(chunk.to_vec(), chunk.len(), dev).map_err(e)?;
            let o = obs.index_select(&sel, 0).map_err(e)?;
            let ac = act.index_select(&sel, 0).map_err(e)?;
            let lpo = logp_old.index_select(&sel, 0).map_err(e)?;
            let ad = adv.index_select(&sel, 0).map_err(e)?;
            let rt = ret.index_select(&sel, 0).map_err(e)?;

            let (logits, v) = m.forward(&o).map_err(e)?;
            let logp_all = candle_nn::ops::log_softmax(&logits, 1).map_err(e)?;
            let lp = logp_all
                .gather(&ac.unsqueeze(1).map_err(e)?, 1)
                .map_err(e)?
                .squeeze(1)
                .map_err(e)?;

            let ratio = (lp.clone() - lpo).map_err(e)?.exp().map_err(e)?;
            let s1 = (ratio.clone() * ad.clone()).map_err(e)?;
            let s2 = (ratio.clamp(1.0 - cfg.clip, 1.0 + cfg.clip).map_err(e)? * ad).map_err(e)?;
            let pg = s1.minimum(&s2).map_err(e)?.mean_all().map_err(e)?.neg().map_err(e)?;

            let vf = loss::mse(&v, &rt).map_err(e)?;
            // Entropy of the categorical policy: -sum p log p.
            let p = logp_all.exp().map_err(e)?;
            let ent = (p * logp_all).map_err(e)?.sum(1).map_err(e)?.mean_all().map_err(e)?;

            let total = ((pg + (vf * cfg.vf_coef as f64).map_err(e)?).map_err(e)?
                + (ent * cfg.ent_coef as f64).map_err(e)?)
            .map_err(e)?;
            opt.backward_step(&total).map_err(e)?;
            last = total.to_scalar::<f32>().map_err(e)?;
        }
    }
    let _ = DType::F32;
    Ok(last)
}
