//! `tmrl ppo` — PPO fine-tuning of a BC policy (L2, v0): the `bcnet` categorical-chunk network warm-started from
//! `policy.tmw`, rolled out in a fleet of `tmenv` workers with `step_ticks`, updated with clipped PPO over the CHUNK
//! log-probability (sum over the k ticks of log p(steer bin) + log p(gas) + log p(brake)). GAE and the batch
//! assembly are `crate::ppo` (the truncated-vs-terminated known-answer tests apply unchanged); the chunk actions live
//! in a side table indexed by `ppo::Step::action`.
//!
//! v0 resets every episode to the root (reset-anywhere from a `StateArchive` is the next step). Reward is the
//! env's (`CoreCfg` default: 0.01/m new progress, −0.30/s, +10 finish, −1 crash). Every new best (by env gates,
//! then progress) is written as a tape and handed to the plain oracle — the env's reading is printed beside the
//! oracle's, never instead of it (a finish is certified only by the oracle; partial cps is UNREPORTED there).

use crate::bcnet::{bin_of, bin_centres, HeadKind, Trainable, Weights};
use crate::ppo::{assemble, GaeCfg, PpoCfg, Step};
use candle_core::{Device, Tensor, D};
use candle_nn::{Optimizer, ParamsAdamW};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;
use tmenv::action::ActionSpace;
use tmenv::core::{CoreCfg, Done};
use tmenv::forkenv::{ForkEnv, Rig};
use tmstate::Action;

pub struct PpoArgs {
    pub init: String,
    /// Multi-map: DATA maps dir + uids; workers are assigned round-robin; per map the template (fastest plain
    /// ghost) is built and the identity control run; maps failing it are dropped. Empty = single map (--geom/--map/--ref).
    pub maps_dir: Option<String>,
    pub maps: Vec<String>,
    pub tape_factor: f32,
    /// Cap on the template length (ms). MEASURED 2026-09-07: tapes past ~7,000 ticks make every step of that worker fail
    /// with `PROBE-EMPTY … n 7196` (ENV item); 60 s keeps every template under 6,000 ticks.
    pub tape_cap_ms: u32,
    pub geom: String,
    pub map: PathBuf,
    pub reference: PathBuf,
    pub server: PathBuf,
    pub shim: PathBuf,
    pub work: PathBuf,
    pub out: PathBuf,
    pub workers: usize,
    pub iters: usize,
    pub steps_per_worker: usize,
    pub max_ep_steps: usize,
    pub max_ticks: usize,
    pub lr: f64,
    pub ent: f32,
    pub clip: f32,
    pub epochs: usize,
    pub minibatch: usize,
    pub temp: f32,
    pub seed: u64,
    /// Reset-anywhere: fraction of episodes started from an archived state; snapshots every N chunks; bucket size.
    pub p_archive: f32,
    pub snap_every: usize,
    pub bucket_m: f32,
    /// Off-route margin (m) beyond the corridor half-width for training episodes (tmenv default 4).
    pub margin_m: f32,
    /// Tracking reward (BAR M2-3): per-map human references from `tmrl refs` (<dir>/<uid>.ref); per chunk
    /// r += w(iter) × exp(−(Δlat/2 m)² − (Δv/8 m/s)² − (Δyaw/0.35)²) at the car's s; w anneals linearly from
    /// `track_w0` to 0 at `track_anneal` × iters (0.1 ≈ a 10 m progress gain per chunk under tmenv's c_prog 0.01).
    pub refs_dir: Option<String>,
    pub track_w0: f32,
    pub track_anneal: f32,
    /// Seed each worker's StateArchive with the donor's own run (a snapshot every `snap_every` ticks along the human
    /// line); requires the donor-seed template. Episodes then start along the human line with p_archive.
    pub human_archive: bool,
    /// Rebuild the fleet on a fresh random map sample every N iterations (0 = never; only when the pool > workers).
    pub rotate_every: usize,
    /// Of the archive starts, the fraction that resume the FRONTIER state (furthest progress) instead of a uniform one.
    pub p_frontier: f32,
}

/// xorshift64: the sampler's RNG (the worker seeds it from the episode seed alone; see ROLLOUT-WORKER.md §3).
pub struct Rng(pub u64);
impl Rng {
    pub fn f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / ((1u32 << 24) as f32)
    }
}

/// One chunk as the update needs it: chosen steer bins and pedal bits per tick.
#[derive(Clone)]
pub struct Chunk {
    pub bins: Vec<u32>,
    pub gas: Vec<f32>,
    pub brake: Vec<f32>,
}

/// Sample a chunk and return it with its log-probability under the policy (temperature applied to the logits).
pub fn sample_chunk(w: &Weights, raw: &[f32], temp: f32, rng: &mut Rng) -> (Vec<Action>, Chunk, f32) {
    let s = &w.shape;
    let (so, go, bo, _) = s.offsets();
    let b = s.bins;
    let centres = bin_centres(b);
    let t = temp.max(1e-3);
    let mut acts = Vec::with_capacity(s.k);
    let mut ch = Chunk { bins: Vec::with_capacity(s.k), gas: Vec::with_capacity(s.k), brake: Vec::with_capacity(s.k) };
    let mut logp = 0f32;
    for i in 0..s.k {
        let l = &raw[so + b * i..so + b * i + b];
        let m = l.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let p: Vec<f32> = l.iter().map(|x| ((x - m) / t).exp()).collect();
        let z: f32 = p.iter().sum();
        let r = rng.f32() * z;
        let mut acc = 0.0;
        let mut j = b - 1;
        for (idx, q) in p.iter().enumerate() {
            acc += q;
            if r < acc {
                j = idx;
                break;
            }
        }
        logp += (p[j] / z).max(1e-8).ln();
        let pg = 1.0 / (1.0 + (-raw[go + i] / t).exp());
        let pb = 1.0 / (1.0 + (-raw[bo + i] / t).exp());
        let gas = rng.f32() < pg;
        let brake = rng.f32() < pb;
        logp += if gas { pg } else { 1.0 - pg }.max(1e-8).ln();
        logp += if brake { pb } else { 1.0 - pb }.max(1e-8).ln();
        acts.push(Action { steer: centres[j], gas, brake });
        ch.bins.push(j as u32);
        ch.gas.push(gas as u8 as f32);
        ch.brake.push(brake as u8 as f32);
    }
    (acts, ch, logp)
}

struct Worker {
    env: ForkEnv,
    rng: Rng,
    /// Reset-anywhere (ENV G4): this worker's own archived states (snapshots are paused processes in THIS worker's
    /// forest, so they cannot be shared across workers). One kept state per `bucket_m` of progress.
    archive: tmenv::archive::StateArchive,
    /// Env step errors this worker has seen (an episode that ends on one is NOT a truncation: it is dropped).
    errors: usize,
    last_error: Option<String>,
    /// Which map this worker drives (index into the run's map set) and its episode cap.
    map_idx: usize,
    max_ep: usize,
    length_m: f32,
    reft: Option<std::sync::Arc<crate::refs::RefTrack>>,
}

struct Harvest {
    map_idx: usize,
    frac_sum: f32,
    track: (f32, usize),
    episodes: Vec<Vec<Step>>,
    chunks: Vec<Chunk>,
    steps: usize,
    best: Option<(f32, usize, Option<i64>, Vec<u8>, Vec<u8>, Vec<u8>)>,
    dones: [usize; 4],
    best_s_sum: f32,
    n_eps: usize,
    n_arch: usize,
    errors: usize,
    last_error: Option<String>,
}

/// `p_archive`: probability of starting from an archived state instead of the line (0 = always the line).
/// `snap_every`: take a snapshot every this many chunks when on the corridor and moving (0 = never).
fn run_episode(w: &mut Worker, weights: &Weights, temp: f32, max_steps: usize, chunks: &mut Vec<Chunk>, p_archive: f32, snap_every: usize, p_frontier: f32, w_track: f32, track_acc: &mut (f32, usize)) -> (Vec<Step>, Option<Done>, bool) {
    let s = &weights.shape;
    let (_, _, _, vo) = s.offsets();
    let mut out = Vec::with_capacity(max_steps);
    let from_archive = p_archive > 0.0 && !w.archive.is_empty() && w.rng.f32() < p_archive;
    let start = if from_archive {
        let r = (w.rng.f32() * 1e9) as u64;
        // Frontier bias: with probability `p_frontier` (of archive starts) resume the furthest archived state — the
        // backward-curriculum move for a map whose last section is the wall (Fall 2024 - 08: 6 of 7 gates).
        let pick = if w.rng.f32() < p_frontier { w.archive.frontier() } else { w.archive.sample(r) };
        match pick {
            Some(e) => w.env.reset_to(&e.id),
            None => w.env.reset(),
        }
    } else {
        w.env.reset()
    };
    let mut obs = match start {
        Ok(o) => o,
        Err(_) => return (out, None, from_archive),
    };
    let mut since_snap = 0usize;
    let errors_before = w.errors;
    let mut done = None;
    for _ in 0..max_steps {
        let raw = weights.forward(&obs);
        let value = raw[vo];
        let (acts, ch, logp) = sample_chunk(weights, &raw, temp, &mut w.rng);
        let (next_obs, mut reward, d, info) = match w.env.step_ticks(&acts) {
            Ok(v) => v,
            Err(e) => {
                w.errors += 1;
                if w.last_error.is_none() {
                    w.last_error = Some(e);
                }
                break;
            }
        };
        if let Some(r) = &w.reft {
            if let Some(t) = crate::refs::tracking_term(r, info.s, info.state.pos, info.speed, crate::refs::yaw_of(info.state.quat), (2.0, 8.0, 0.35)) {
                reward += w_track * t;
                track_acc.0 += t;
                track_acc.1 += 1;
            }
        }
        since_snap += 1;
        if snap_every > 0 && since_snap >= snap_every && d.is_none() && info.speed > 20.0 && info.lateral.abs() <= tmobs::half_width(&w.env.core.track.geom, info.s) {
            since_snap = 0;
            let id = w.env.snapshot();
            let race_ms = info.state.race_ms;
            let score = if race_ms > 0 { info.best_s / (race_ms as f32 / 1000.0) } else { 0.0 };
            if let Some(evicted) = w.archive.offer(tmenv::archive::Entry { id, progress_m: info.best_s, race_ms, score, origin: 1 }) {
                w.env.drop_snapshot(&evicted);
            }
        }
        let next_value = weights.forward(&next_obs)[vo];
        let terminal = matches!(d, Some(Done::Finished) | Some(Done::OffRoute) | Some(Done::NoProgress));
        let truncated = matches!(d, Some(Done::TickCap));
        chunks.push(ch);
        out.push(Step { obs: std::mem::replace(&mut obs, next_obs), action: chunks.len() - 1, logp, value, reward, terminal, truncated, next_value });
        if d.is_some() {
            done = d;
            break;
        }
    }
    // An episode cut by max_steps is truncated; one cut by an env ERROR is dropped entirely (its last transition
    // is not a real outcome and its value would teach the policy that the error state is worth something).
    if done.is_none() {
        if w.last_error.is_some() && w.errors > errors_before {
            out.clear();
        } else if let Some(last) = out.last_mut() {
            last.truncated = true;
        }
    }
    (out, done, from_archive)
}

fn bce_logp(logit: &Tensor, y: &Tensor) -> candle_core::Result<Tensor> {
    // log p(y) = -( relu(x) - x*y + log(1+exp(-|x|)) )
    let a = logit.relu()?;
    let b = (logit * y)?;
    let c = (logit.abs()?.neg()?.exp()? + 1.0)?.log()?;
    ((a - b)? + c)?.neg()
}

/// PPO update over chunk log-probabilities. Returns (total loss, policy loss, value loss, entropy, approx KL).
fn update(m: &Trainable, opt: &mut candle_nn::AdamW, obs: &[f32], chunks: &[Chunk], logp_old: &[f32], adv: &[f32], ret: &[f32], cfg: &PpoCfg, dev: &Device, seed: u64) -> Result<(f32, f32, f32, f32, f32), String> {
    let e = |x: candle_core::Error| x.to_string();
    let s = &m.shape;
    let n = chunks.len();
    let (k, b) = (s.k, s.bins);
    let (so, go, bo, vo) = s.offsets();
    let obs_t = Tensor::from_vec(obs.to_vec(), (n, s.obs_dim), dev).map_err(e)?;
    let bins_t = Tensor::from_vec(chunks.iter().flat_map(|c| c.bins.iter().cloned()).collect::<Vec<u32>>(), (n, k), dev).map_err(e)?;
    let gas_t = Tensor::from_vec(chunks.iter().flat_map(|c| c.gas.iter().cloned()).collect::<Vec<f32>>(), (n, k), dev).map_err(e)?;
    let brake_t = Tensor::from_vec(chunks.iter().flat_map(|c| c.brake.iter().cloned()).collect::<Vec<f32>>(), (n, k), dev).map_err(e)?;
    let lpo_t = Tensor::from_vec(logp_old.to_vec(), n, dev).map_err(e)?;
    let adv_t = Tensor::from_vec(adv.to_vec(), n, dev).map_err(e)?;
    let ret_t = Tensor::from_vec(ret.to_vec(), n, dev).map_err(e)?;
    let mut idx: Vec<u32> = (0..n as u32).collect();
    let mut rng = Rng(seed | 1);
    let (mut last, mut lpg, mut lvf, mut lent, mut lkl) = (0f32, 0f32, 0f32, 0f32, 0f32);
    for _ in 0..cfg.epochs {
        for i in (1..n).rev() {
            let j = (rng.f32() * (i + 1) as f32) as usize;
            idx.swap(i, j.min(i));
        }
        for chunk in idx.chunks(cfg.minibatch) {
            let nb = chunk.len();
            let sel = Tensor::from_vec(chunk.to_vec(), nb, dev).map_err(e)?;
            let o = obs_t.index_select(&sel, 0).map_err(e)?;
            let bi = bins_t.index_select(&sel, 0).map_err(e)?;
            let ga = gas_t.index_select(&sel, 0).map_err(e)?;
            let br = brake_t.index_select(&sel, 0).map_err(e)?;
            let lpo = lpo_t.index_select(&sel, 0).map_err(e)?;
            let ad = adv_t.index_select(&sel, 0).map_err(e)?;
            let rt = ret_t.index_select(&sel, 0).map_err(e)?;
            let raw = m.forward(&o).map_err(e)?;
            let logits = raw.narrow(1, so, b * k).map_err(e)?.reshape((nb, k, b)).map_err(e)?;
            let lsm = candle_nn::ops::log_softmax(&logits, D::Minus1).map_err(e)?;
            let lp_steer = lsm.gather(&bi.unsqueeze(D::Minus1).map_err(e)?, D::Minus1).map_err(e)?.squeeze(D::Minus1).map_err(e)?.sum(D::Minus1).map_err(e)?;
            let lp_gas = bce_logp(&raw.narrow(1, go, k).map_err(e)?, &ga).map_err(e)?.sum(D::Minus1).map_err(e)?;
            let lp_brake = bce_logp(&raw.narrow(1, bo, k).map_err(e)?, &br).map_err(e)?.sum(D::Minus1).map_err(e)?;
            let lp = ((lp_steer + lp_gas).map_err(e)? + lp_brake).map_err(e)?;
            let v = raw.narrow(1, vo, 1).map_err(e)?.squeeze(1).map_err(e)?;

            let ratio = (lp.clone() - &lpo).map_err(e)?.exp().map_err(e)?;
            let s1 = (ratio.clone() * &ad).map_err(e)?;
            let s2 = (ratio.clamp(1.0 - cfg.clip, 1.0 + cfg.clip).map_err(e)? * &ad).map_err(e)?;
            let pg = s1.minimum(&s2).map_err(e)?.mean_all().map_err(e)?.neg().map_err(e)?;
            let vf = candle_nn::loss::mse(&v, &rt).map_err(e)?;
            // Entropy: categorical per tick + Bernoulli per pedal tick (negated → minimised term = −H).
            let p = lsm.exp().map_err(e)?;
            let neg_h_steer = (p * &lsm).map_err(e)?.sum(D::Minus1).map_err(e)?.sum(D::Minus1).map_err(e)?.mean_all().map_err(e)?;
            let bern = |x: &Tensor| -> candle_core::Result<Tensor> {
                let pz = candle_nn::ops::sigmoid(x)?;
                let one_m = (pz.neg()? + 1.0)?;
                ((&pz * pz.clamp(1e-6, 1.0)?.log()?)? + (&one_m * one_m.clamp(1e-6, 1.0)?.log()?)?)?.sum(D::Minus1)?.mean_all()
            };
            let neg_h_ped = (bern(&raw.narrow(1, go, k).map_err(e)?).map_err(e)? + bern(&raw.narrow(1, bo, k).map_err(e)?).map_err(e)?).map_err(e)?;
            let neg_h = (neg_h_steer + neg_h_ped).map_err(e)?;
            let total = ((pg.clone() + (vf.clone() * cfg.vf_coef as f64).map_err(e)?).map_err(e)? + (neg_h.clone() * cfg.ent_coef as f64).map_err(e)?).map_err(e)?;
            opt.backward_step(&total).map_err(e)?;
            last = total.to_scalar::<f32>().map_err(e)?;
            lpg = pg.to_scalar::<f32>().map_err(e)?;
            lvf = vf.to_scalar::<f32>().map_err(e)?;
            lent = -neg_h.to_scalar::<f32>().map_err(e)?;
            lkl = (lpo - lp).map_err(e)?.mean_all().map_err(e)?.to_scalar::<f32>().map_err(e)?;
        }
    }
    Ok((last, lpg, lvf, lent, lkl))
}

pub fn run(a: &PpoArgs) -> Result<(), String> {
    let dev = Device::Cpu;
    let (w0, obs_version) = crate::policy::read_any(&a.init)?;
    if w0.shape.head != HeadKind::Categorical {
        return Err("tmrl ppo v0 needs a categorical-steer policy (--head cat/keyboard)".into());
    }
    let k = w0.shape.k;
    let model = Trainable::from_weights(&w0, &dev).map_err(|e| e.to_string())?;
    let worst = w0.agrees_with(&model, &dev, 32, 1e-4)?;
    println!("# tmrl ppo -- PPO fine-tune from {} ({:?}); forward agreement {worst:.2e}", a.init, w0.shape);
    std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&a.work).map_err(|e| e.to_string())?;
    // The map set: one entry per map with its track, map file, reference and measured spawn.
    struct MapEnv {
        uid: String,
        reft: Option<std::sync::Arc<crate::refs::RefTrack>>,
        /// The donor ghost (its tape seeds the human-line archive) and its index shift against the template.
        donor: Option<(std::sync::Arc<Vec<Action>>, i64)>,
        track: std::sync::Arc<tmenv::Track>,
        map: PathBuf,
        reference: PathBuf,
        spawn: [f32; 3],
        max_ticks: usize,
        max_ep: usize,
    }
    let mut menvs: Vec<MapEnv> = Vec::new();
    if let Some(dir) = &a.maps_dir {
        let setup_map = |uid: &String| -> Option<MapEnv> {
            let md = Path::new(dir).join(uid);
            let mj: serde_json::Value = std::fs::read_to_string(md.join("map.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(serde_json::Value::Null);
            let at = mj.get("author_ms").and_then(|v| v.as_i64()).unwrap_or(60_000).max(10_000) as f32;
            let tape_ms = ((at * a.tape_factor) as u32).min(a.tape_cap_ms);
            let work = a.work.join("tpl").join(uid);
            let (reference, donor) = match crate::eval::build_reference_from(&md, &work, tape_ms, true) {
                Ok(x) => x,
                Err(e) => {
                    println!("# map {uid}: template failed ({e}) — dropped");
                    return None;
                }
            };
            let track = match tmenv::Track::load_geom_json(&md.join("geom.json")) {
                Ok(t) => std::sync::Arc::new(t),
                Err(e) => {
                    println!("# map {uid}: geom failed ({e}) — dropped");
                    return None;
                }
            };
            // Identity control: the donor's tape through the env must reproduce its time.
            let idc = crate::rollout::RolloutArgs {
                policy: a.init.clone(),
                geom: md.join("geom.json").to_string_lossy().into_owned(),
                map: md.join("map.Map.Gbx"),
                reference: crate::eval::identity_reference(&reference),
                server: a.server.clone(),
                shim: a.shim.clone(),
                work: work.join("identity"),
                out: a.out.join("identity").join(uid),
                episodes: 1,
                max_ticks: (tape_ms / 10) as usize - 60,
                max_steps: (tape_ms / 100) as usize,
                temp: 0.0,
                seed: 1,
                verbose: false,
                no_oracle: false,
                const_ctrl: false,
                tape: Some(donor.to_string_lossy().into_owned()),
                tape_shift_ms: 0,
                no_cut: true,
                margin_m: a.margin_m,
                refs: None,
            };
            let donor_ms: i64 = donor.file_name().and_then(|s| s.to_str()).and_then(|s| s.trim_end_matches(".Ghost.Gbx").split('-').nth(1)).and_then(|s| s.parse().ok()).unwrap_or(0);
            let ok = match crate::rollout::run(&idc) {
                Ok(r) => r.first().and_then(|x| x.oracle_finish_s).map(|t| ((t * 1000.0).round() as i64 - donor_ms).abs() <= 20).unwrap_or(false),
                Err(_) => false,
            };
            if !ok {
                println!("# map {uid}: identity control FAILED — dropped");
                return None;
            }
            let spawn = match tmenv::measured_spawn(&a.server, &md.join("map.Map.Gbx"), &a.shim, &work.join("spawnfix"), &reference) {
                Ok(s) => s,
                Err(e) => {
                    println!("# map {uid}: spawn unmeasured ({e}) — dropped");
                    return None;
                }
            };
            let reft = a.refs_dir.as_ref().and_then(|d| crate::refs::RefTrack::read(&Path::new(d).join(format!("{uid}.ref"))).ok()).map(std::sync::Arc::new);
            // Human-line archive (BAR M2-3 piece 3): the workers drive the DONOR-SEED template so the donor's tape
            // reproduces in their env; its snapshots become the episode starts.
            let reference = if a.human_archive { crate::eval::identity_reference(&reference) } else { reference };
            let donor_acts = if a.human_archive {
                gbx::tape::Tape::from_file(donor.to_str().unwrap()).ok().map(|t| {
                    let (s, g, b) = (t.steer_i8s(), t.accels(), t.brakes());
                    let acts: Vec<Action> = (0..s.len()).map(|i| Action { steer: s[i], gas: g[i] != 0, brake: b[i] != 0 }).collect();
                    let tpl_t0 = gbx::tape::Tape::from_file(reference.to_str().unwrap()).map(|tt| tt.race_ms(0)).unwrap_or(0);
                    (std::sync::Arc::new(acts), (tpl_t0 - t.race_ms(0)) / 10)
                })
            } else {
                None
            };
            println!("# map {uid}: {:.1} m, {} gates, tape {tape_ms} ms, identity OK, reference {}", track.geom.length(), track.geom.gates.len(), reft.as_ref().map(|r| format!("{} samples", r.s.len())).unwrap_or("NONE".into()));
            Some(MapEnv { uid: uid.clone(), reft, donor: donor_acts, track, map: md.join("map.Map.Gbx"), reference, spawn, max_ticks: (tape_ms / 10) as usize - 60, max_ep: (tape_ms / 100) as usize })
        };
        // 24 maps at a time: each setup spins a server for the identity replay (~5 s).
        let t_setup = Instant::now();
        for chunk in a.maps.chunks(24) {
            let got: Vec<Option<MapEnv>> = std::thread::scope(|sc| {
                let hs: Vec<_> = chunk.iter().map(|uid| sc.spawn(|| setup_map(uid))).collect();
                hs.into_iter().map(|h| h.join().ok().flatten()).collect()
            });
            menvs.extend(got.into_iter().flatten());
        }
        println!("# map setup: {} of {} maps usable in {:.0}s", menvs.len(), a.maps.len(), t_setup.elapsed().as_secs_f64());
        if menvs.is_empty() {
            return Err("no map passed the template + identity gate".into());
        }
        println!("# multi-map: {} maps → {} workers round-robin", menvs.len(), a.workers);
    } else {
        let track = std::sync::Arc::new(tmenv::Track::load_geom_json(Path::new(&a.geom))?);
        println!("# geom {} {:.1} m {} gates; map {}", track.geom.source, track.geom.length(), track.geom.gates.len(), a.map.display());
        let spawn = tmenv::measured_spawn(&a.server, &a.map, &a.shim, &a.work.join("spawnfix"), &a.reference)?;
        let reft = a.refs_dir.as_ref().and_then(|d| crate::refs::RefTrack::read(Path::new(d)).ok()).map(std::sync::Arc::new);
        menvs.push(MapEnv { uid: "single".into(), reft, donor: None, track, map: a.map.clone(), reference: a.reference.clone(), spawn, max_ticks: a.max_ticks, max_ep: a.max_ep_steps });
    }
    let track = menvs[0].track.clone();
    let _ = &track;
    let ccfg = CoreCfg { k_ticks: k, max_ticks: a.max_ticks, ..Default::default() };
    let pcfg = PpoCfg { lr: a.lr, ent_coef: a.ent, clip: a.clip, epochs: a.epochs, minibatch: a.minibatch, ..Default::default() };
    let gcfg = GaeCfg::default();
    let mut opt = candle_nn::AdamW::new(model.varmap.all_vars(), ParamsAdamW { lr: pcfg.lr, ..Default::default() }).map_err(|e| e.to_string())?;

    println!("# standing up {} workers...", a.workers);
    let t0 = Instant::now();
    // Fleet builder: `assign[wi]` = the map index worker wi drives (rotation rebuilds the fleet on a new assignment;
    // dropping a ForkEnv kills its server tree).
    let build_fleet = |assign: &[usize], gen: usize| -> Vec<(Worker, Rig, fk::tape::Tape)> {
        std::thread::scope(|sc| {
        let hs: Vec<_> = (0..a.workers)
            .map(|wi| {
                let work = a.work.join(format!("g{gen}-w{wi}"));
                let me = &menvs[assign[wi]];
                let track = me.track.clone();
                let ccfg = CoreCfg { k_ticks: k, max_ticks: me.max_ticks, offroute_margin: a.margin_m, obs_version, ..Default::default() };
                let spawn = me.spawn;
                let (map_idx, max_ep, length_m) = (assign[wi], me.max_ep, me.track.geom.length());
                let reft = me.reft.clone();
                let donor = me.donor.clone();
                let (server, map, shim, reference) = (&a.server, &me.map, &a.shim, &me.reference);
                let seed = a.seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(wi as u64 + 1);
                let (bucket_m, snap_every) = (a.bucket_m, a.snap_every.max(1));
                sc.spawn(move || {
                    let mut root = tmenv::forkenv::RootCfg { verbose: false, ..Default::default() };
                    root.require_start = Some((spawn, 6.0, 4.0));
                    match tmenv::forkenv::build_at_start(server, map, shim, &work, reference, track, ActionSpace::default(), ccfg, &root) {
                        Ok((env, rig, tape)) => {
                            let mut w = Worker { env, rng: Rng(seed | 1), archive: tmenv::archive::StateArchive::new(bucket_m, 1), errors: 0, last_error: None, map_idx, max_ep, length_m, reft };
                            if let Some((acts, shift)) = donor {
                                if let Err(e) = seed_human_archive(&mut w, &acts, shift, snap_every) {
                                    eprintln!("  worker {wi}: human archive failed ({e})");
                                }
                            }
                            Some((w, rig, tape))
                        }
                        Err(e) => {
                            eprintln!("  worker {wi} did not come up ({e})");
                            None
                        }
                    }
                })
            })
            .collect();
        hs.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
        })
    };
    // Map assignment: round-robin when the pool fits; otherwise a random sample per rotation.
    let mut rot_rng = Rng(a.seed.wrapping_mul(0xD1B54A32D192ED03) | 1);
    let mut pick_assign = |rng: &mut Rng| -> Vec<usize> {
        if menvs.len() <= a.workers {
            (0..a.workers).map(|wi| wi % menvs.len()).collect()
        } else {
            let mut idx: Vec<usize> = (0..menvs.len()).collect();
            for i in (1..idx.len()).rev() {
                let j = (rng.f32() * (i + 1) as f32) as usize;
                idx.swap(i, j.min(i));
            }
            idx.truncate(a.workers);
            idx
        }
    };
    let mut gen = 0usize;
    let mut fleet = build_fleet(&pick_assign(&mut rot_rng), gen);
    if fleet.is_empty() {
        return Err("no worker came up".into());
    }
    println!("# fleet {} workers up in {:.1}s{}", fleet.len(), t0.elapsed().as_secs_f64(), if a.human_archive { format!("; human-line archive: {} snapshots over the fleet ({:.1} per worker)", fleet.iter().map(|(w, _, _)| w.archive.len()).sum::<usize>(), fleet.iter().map(|(w, _, _)| w.archive.len()).sum::<usize>() as f32 / fleet.len().max(1) as f32) } else { String::new() });
    let mut curve = String::from("iter\tsteps\teps\tmean_ret\tmean_len\tmean_best_s\tbest_s\tbest_gates\tfin\toff\tnop\tcap\tsteps_per_s\tloss\tpg\tvf\tent\tkl\n");
    println!("{:>4} {:>7} {:>4} {:>8} {:>6} {:>8} {:>8} {:>3} {:>3} {:>3} {:>3} {:>3} {:>8} {:>8} {:>7} {:>7} {:>6}", "iter", "steps", "eps", "ret", "len", "mean_s", "best_s", "g", "fin", "off", "nop", "cap", "steps/s", "loss", "vf", "ent", "kl");
    let mut best_ever = (0usize, 0f32);
    let mut best_fin: Option<i64> = None;
    let mut best_mean_s = 0f32;
    for it in 0..a.iters {
        if a.rotate_every > 0 && it > 0 && it % a.rotate_every == 0 && menvs.len() > a.workers {
            let t_rot = Instant::now();
            drop(std::mem::take(&mut fleet));
            gen += 1;
            fleet = build_fleet(&pick_assign(&mut rot_rng), gen);
            println!("# rotation {gen}: {} workers on a fresh map sample in {:.1}s", fleet.len(), t_rot.elapsed().as_secs_f64());
            if fleet.is_empty() {
                return Err("no worker came up after rotation".into());
            }
        }
        let weights = model.snapshot().map_err(|e| e.to_string())?;
        let t_roll = Instant::now();
        let (tx, rx) = mpsc::channel::<Harvest>();
        std::thread::scope(|sc| {
            for wk in fleet.iter_mut() {
                let tx = tx.clone();
                let weights = &weights;
                let (spw, temp, p_arch, snap_every, p_frontier) = (a.steps_per_worker, a.temp, a.p_archive, a.snap_every, a.p_frontier);
                let human_archive = a.human_archive;
                let w_track = if a.track_anneal > 0.0 { a.track_w0 * (1.0 - it as f32 / (a.track_anneal * a.iters as f32)).max(0.0) } else { a.track_w0 };
                sc.spawn(move || {
                    let (w, _rig, tape) = wk;
                    let mut h = Harvest { map_idx: w.map_idx, frac_sum: 0.0, track: (0.0, 0), episodes: Vec::new(), chunks: Vec::new(), steps: 0, best: None, dones: [0; 4], best_s_sum: 0.0, n_eps: 0, n_arch: 0, errors: 0, last_error: None };
                    let max_ep = w.max_ep;
                    while h.steps < spw {
                        // The per-worker budget is a soft floor: an episode runs to its own end (max_ep), never cut by the budget —
                        // a slow-but-steady policy was losing every long episode to the cut (mean_len 216 vs a 256 budget).
                        let (ep, d, from_arch) = run_episode(w, weights, temp, max_ep, &mut h.chunks, p_arch, snap_every, p_frontier, w_track, &mut h.track);
                        if from_arch {
                            h.n_arch += 1;
                        }
                        if ep.is_empty() {
                            break;
                        }
                        h.steps += ep.len();
                        match d {
                            Some(Done::Finished) => h.dones[0] += 1,
                            Some(Done::OffRoute) => h.dones[1] += 1,
                            Some(Done::NoProgress) => h.dones[2] += 1,
                            _ => h.dones[3] += 1,
                        }
                        let bs = w.env.core.best_s();
                        let g = w.env.core.gates_hit();
                        h.best_s_sum += bs;
                        h.frac_sum += if w.length_m > 0.0 { (bs / w.length_m).min(1.0) } else { 0.0 };
                        h.n_eps += 1;
                        let fin = w.env.rollout_record().finish_ms;
                        let better = match &h.best {
                            None => true,
                            // A finish beats any non-finish; two finishes compare by time; otherwise gates then progress.
                            Some((s, gg, f, ..)) => match (fin, f) {
                                (Some(a), Some(b)) => a < *b,
                                (Some(_), None) => true,
                                (None, Some(_)) => false,
                                (None, None) => g > *gg || (g == *gg && bs > *s),
                            },
                        };
                        // AUTHORSHIP: an episode resumed from a HUMAN-line snapshot carries the human's inputs as its tape
                        // prefix — such a tape is not the policy's run and is never banked or validated as a best.
                        // (Policy-archive starts are the policy's own earlier inputs and count.)
                        if better && !(human_archive && from_arch) {
                            let (s, ga, b) = w.env.banked_tape(tape);
                            h.best = Some((bs, g, fin, s, ga, b));
                        }
                        h.episodes.push(ep);
                    }
                    h.errors = w.errors;
                    h.last_error = w.last_error.take();
                    let _ = tx.send(h);
                });
            }
            drop(tx);
        });
        let harvests: Vec<Harvest> = rx.into_iter().collect();
        let roll_s = t_roll.elapsed().as_secs_f64();
        // Merge: chunk indices are per-harvest; re-base them.
        let mut episodes: Vec<Vec<Step>> = Vec::new();
        let mut chunks: Vec<Chunk> = Vec::new();
        let (mut errs_total, mut err_sample) = (0usize, None::<String>);
        let (mut total, mut dones, mut best, mut s_sum, mut n_eps, mut n_arch) = (0usize, [0usize; 4], None::<(f32, usize, Option<i64>, Vec<u8>, Vec<u8>, Vec<u8>)>, 0f32, 0usize, 0usize);
        let mut frac_sum = 0f32;
        let mut best_map = 0usize;
        let mut track_acc = (0f32, 0usize);
        for h in harvests {
            let base = chunks.len();
            chunks.extend(h.chunks);
            for mut ep in h.episodes {
                for s in ep.iter_mut() {
                    s.action += base;
                }
                episodes.push(ep);
            }
            total += h.steps;
            for i in 0..4 {
                dones[i] += h.dones[i];
            }
            s_sum += h.best_s_sum;
            n_eps += h.n_eps;
            n_arch += h.n_arch;
            errs_total += h.errors;
            if let Some(e) = h.last_error {
                err_sample = Some(e);
            }
            frac_sum += h.frac_sum;
            track_acc.0 += h.track.0;
            track_acc.1 += h.track.1;
            if let Some(b) = h.best {
                // Across maps: a finish beats a non-finish; two finishes by time / AT-free proxy (the earlier one);
                // two non-finishes by progress FRACTION of their own map.
                let len_of = |i: usize| menvs[i].track.geom.length().max(1.0);
                let better = match &best {
                    None => true,
                    Some((s, _g, f, ..)) => match (b.2, f) {
                        (Some(x), Some(y)) => x < *y,
                        (Some(_), None) => true,
                        (None, Some(_)) => false,
                        (None, None) => b.0 / len_of(h.map_idx) > *s / len_of(best_map),
                    },
                };
                if better {
                    best = Some(b);
                    best_map = h.map_idx;
                }
            }
        }
        if episodes.is_empty() {
            return Err("no episodes collected".into());
        }
        let mean_ret: f32 = episodes.iter().map(|e| e.iter().map(|s| s.reward).sum::<f32>()).sum::<f32>() / episodes.len() as f32;
        let mean_len = total as f32 / episodes.len() as f32;
        let mean_s = s_sum / n_eps.max(1) as f32;
        let mean_frac = frac_sum / n_eps.max(1) as f32;
        let batch = assemble(&episodes, &gcfg, tmobs::obs_dim(obs_version));
        let ordered: Vec<Chunk> = batch.actions.iter().map(|&i| chunks[i as usize].clone()).collect();
        let (loss, pg, vf, ent, kl) = update(&model, &mut opt, &batch.obs, &ordered, &batch.logp_old, &batch.adv, &batch.ret, &pcfg, &dev, a.seed + it as u64)?;
        let (bs, bg) = best.as_ref().map(|b| (b.0, b.1)).unwrap_or((0.0, 0));
        println!(
            "{it:>4} {total:>7} {:>4} {mean_ret:>8.3} {mean_len:>6.1} {mean_s:>8.1} {bs:>8.1} {bg:>3} {:>3} {:>3} {:>3} {:>3} {:>8.0} {loss:>8.4} {vf:>7.3} {ent:>7.3} {kl:>6.3} arch {n_arch}/{n_eps} snaps {} env-errs {errs_total} frac {mean_frac:.3} track {:.3}",
            episodes.len(), dones[0], dones[1], dones[2], dones[3], total as f64 / roll_s, fleet.iter().map(|(w, _, _)| w.env.snapshots()).sum::<usize>(), track_acc.0 / track_acc.1.max(1) as f32
        );
        curve.push_str(&format!("{it}\t{total}\t{}\t{mean_ret:.4}\t{mean_len:.2}\t{mean_s:.2}\t{bs:.2}\t{bg}\t{}\t{}\t{}\t{}\t{:.0}\t{loss:.5}\t{pg:.5}\t{vf:.5}\t{ent:.4}\t{kl:.5}\n", episodes.len(), dones[0], dones[1], dones[2], dones[3], total as f64 / roll_s));
        if let Some(e) = err_sample {
            println!("      env error sample this iteration ({errs_total} total): {e}");
        }
        std::fs::write(a.out.join("curve.tsv"), &curve).map_err(|e| e.to_string())?;
        let w_now = model.snapshot().map_err(|e| e.to_string())?;
        crate::policy::write(a.out.join("policy-last.tmw").to_str().unwrap(), &w_now, obs_version)?;
        if mean_frac > best_mean_s {
            best_mean_s = mean_frac;
            crate::policy::write(a.out.join("policy.tmw").to_str().unwrap(), &w_now, obs_version)?;
        }
        // THE STANDING RULE: a new best is a tape the plain oracle re-simulates.
        if let Some((s, g, fin, st, ga, br)) = best {
            let is_new = match (fin, best_fin) {
                (Some(a), Some(b)) => a < b,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => g > best_ever.0 || (g == best_ever.0 && s > best_ever.1 + 20.0),
            };
            if is_new {
                best_ever = (g.max(best_ever.0), s.max(best_ever.1));
                if fin.is_some() {
                    best_fin = fin;
                }
                let wi = fleet.iter().position(|(w, _, _)| w.map_idx == best_map).unwrap_or(0);
                let (_, _, tape) = &fleet[wi];
                let best_map_file = menvs[best_map].map.clone();
                let path = a.out.join(format!("iter{it:04}-{}-g{g}-s{:.0}.Ghost.Gbx", menvs[best_map].uid, s));
                if let Err(e) = tape.write_candidate(&st, &ga, &br, &path) {
                    eprintln!("      could not write the tape: {e}");
                } else {
                    let v = tmauto::oracle::validate_raw(&a.server, &[path.clone()], tmauto::oracle::Maps::One(&best_map_file), "tmrl-ppo");
                    let verdict = match v {
                        Err(e) => format!("oracle unreachable: {e}"),
                        Ok(bt) => match bt.answers.first() {
                            None => "no answer".into(),
                            Some(ans) => match (ans.time_ms, ans.simulated()) {
                                (Some(t), _) if t >= 0 => format!("FINISH {:.3}", t as f64 / 1000.0),
                                (_, true) => match ans.cps { Some(c) => format!("DNF cps={c}"), None => "DNF (cps unreported)".into() },
                                (_, false) => format!("REFUSED ({})", ans.desc.trim()),
                            },
                        },
                    };
                    println!("      NEW BEST  env: {g} gate(s), s = {s:.1} m{} | PLAIN ORACLE: {verdict} | {}", fin.map(|f| format!(", env finish {:.3}", f as f64 / 1000.0)).unwrap_or_default(), path.display());
                }
            }
        }
    }
    println!("# done; policy.tmw = best mean progress ({best_mean_s:.1} m), policy-last.tmw, curve.tsv in {}", a.out.display());
    Ok(())
}

/// BAR M2-3 piece 3: replay the donor's tape through this worker's env (donor-seed template, so it reproduces) and
/// snapshot every `snap_every` ticks along the human line into the worker's archive. Returns the number of snapshots.
fn seed_human_archive(w: &mut Worker, acts: &[Action], shift: i64, snap_every: usize) -> Result<usize, String> {
    let k = w.env.core.cfg.k_ticks.max(1);
    w.env.reset()?;
    let mut n = 0usize;
    let mut since = 0usize;
    let mut steps = 0usize;
    loop {
        let base = w.env.next_tick()? as i64;
        let chunk: Vec<Action> = (0..k)
            .map(|j| {
                let idx = base + j as i64 + shift;
                if idx >= 0 && (idx as usize) < acts.len() { acts[idx as usize] } else { Action { steer: 0, gas: false, brake: true } }
            })
            .collect();
        let (_, _, d, info) = w.env.step_ticks(&chunk)?;
        steps += 1;
        since += k;
        if d.is_some() || steps > w.max_ep + 50 {
            break;
        }
        if since >= snap_every && info.speed > 5.0 {
            since = 0;
            let id = w.env.snapshot();
            let race_ms = info.state.race_ms;
            let score = if race_ms > 0 { info.best_s / (race_ms as f32 / 1000.0) } else { 0.0 };
            if let Some(ev) = w.archive.offer(tmenv::archive::Entry { id, progress_m: info.best_s, race_ms, score, origin: 0 }) {
                w.env.drop_snapshot(&ev);
            }
            n += 1;
        }
    }
    w.env.reset()?;
    Ok(n)
}
