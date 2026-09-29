//! `tmrl bc` — behaviour cloning on `.tmd` shards with chunked actions.
//!
//! Sample = (obs at tick t, the k actions t..t+k, the record's weight). Observation from `tmobs::observe` with the
//! action history threaded tick by tick per ghost, exactly as a live env would. Loss = weighted mean over
//! samples of [ steer MSE on ±1 (tanh) + gas BCE + brake BCE ] per chunk tick. Metrics on the held-out shard:
//! per-tick steer MAE in i8 units (after the 1/127 quantisation), gas / brake accuracy — beside two baselines
//! that need no network: HOLD (repeat the previous action for k ticks; the previous action is IN the
//! observation, so a net that does not beat HOLD has learned nothing) and CONST (steer 0, gas, no brake).

use crate::bcnet::{HeadKind, Shape, Trainable, Weights, HIDDEN};
use crate::shard::{read_shard, Record};
use candle_core::{Device, Tensor, D};
use candle_nn::{AdamW, Optimizer, ParamsAdamW};
use std::collections::BTreeMap;
use std::time::Instant;
use tmstate::{Action, TrackGeom};

pub struct Sample {
    pub obs: Vec<f32>,
    pub acts: Vec<Action>, // k
    pub weight: f32,
    pub prev: Option<Action>,
}

/// Records → chunked samples. One observation pass per ghost, in tick order.
/// `lag`: pair the observation at tick t with the actions t+lag .. t+lag+k. MEASURED 2026-09-06 (BAR CL-0): the env's
/// race label is 20 ms behind the ghost clock, so a policy that will run in the env sees a state labelled t and
/// should emit what the human did at t+2. Default 0 = the recorded pairing.
pub fn build_samples(recs: &[Record], geoms: &BTreeMap<String, TrackGeom>, k: usize, stride: usize, lag: usize, obs_version: u32) -> Result<Vec<Sample>, String> {
    let mut by_ghost: BTreeMap<(String, u32), Vec<&Record>> = BTreeMap::new();
    for r in recs {
        by_ghost.entry((r.map_uid.clone(), r.ghost_id)).or_default().push(r);
    }
    let mut out = Vec::new();
    let mut missing = std::collections::BTreeSet::new();
    for ((uid, gid), mut rs) in by_ghost {
        rs.sort_by_key(|r| r.tick);
        let Some(g) = geoms.get(&uid) else {
            missing.insert(uid.clone());
            let _ = gid;
            continue;
        };
        let mut obs_seq = Vec::with_capacity(rs.len());
        let mut hist: Vec<Action> = Vec::with_capacity(rs.len());
        for r in &rs {
            let o = tmobs::observe_version(obs_version, g, &r.state, &hist);
            obs_seq.push((o, hist.last().copied()));
            hist.push(r.action);
        }
        let n = rs.len();
        let mut t = 0;
        while t + lag + k <= n {
            out.push(Sample { obs: obs_seq[t].0.clone(), acts: rs[t + lag..t + lag + k].iter().map(|r| r.action).collect(), weight: rs[t].weight, prev: obs_seq[t].1 });
            t += stride.max(1);
        }
    }
    if !missing.is_empty() {
        eprintln!("WARNING {} map(s) without geometry skipped: {:?}", missing.len(), missing.iter().take(5).collect::<Vec<_>>());
    }
    Ok(out)
}

pub struct Tensors {
    pub x: Tensor,
    pub steer: Tensor, // [n,k] in ±1
    pub cls: Tensor,   // [n,k] u32 steer bin (categorical head; zeros for analog)
    pub gas: Tensor,   // [n,k] 0/1
    pub brake: Tensor, // [n,k]
    pub w: Tensor,     // [n]
    pub n: usize,
}

pub fn to_tensors(s: &[Sample], k: usize, obs_dim: usize, bins: usize, dev: &Device) -> candle_core::Result<Tensors> {
    let n = s.len();
    let mut x = Vec::with_capacity(n * obs_dim);
    let mut st = Vec::with_capacity(n * k);
    let mut cl: Vec<u32> = Vec::with_capacity(n * k);
    let centres = if bins >= 3 { crate::bcnet::bin_centres(bins) } else { vec![0i8] };
    let mut ga = Vec::with_capacity(n * k);
    let mut br = Vec::with_capacity(n * k);
    let mut w = Vec::with_capacity(n);
    for smp in s {
        x.extend_from_slice(&smp.obs);
        for a in &smp.acts {
            st.push(a.steer as f32 / 127.0);
            cl.push(if bins >= 3 { crate::bcnet::bin_of(a.steer, &centres) as u32 } else { 0 });
            ga.push(a.gas as u8 as f32);
            br.push(a.brake as u8 as f32);
        }
        w.push(smp.weight);
    }
    Ok(Tensors {
        x: Tensor::from_vec(x, (n, obs_dim), dev)?,
        steer: Tensor::from_vec(st, (n, k), dev)?,
        cls: Tensor::from_vec(cl, (n, k), dev)?,
        gas: Tensor::from_vec(ga, (n, k), dev)?,
        brake: Tensor::from_vec(br, (n, k), dev)?,
        w: Tensor::from_vec(w, n, dev)?,
        n,
    })
}

fn bce_with_logits(x: &Tensor, y: &Tensor) -> candle_core::Result<Tensor> {
    // relu(x) - x*y + log(1 + exp(-|x|))
    let a = x.relu()?;
    let b = (x * y)?;
    let c = (x.abs()?.neg()?.exp()? + 1.0)?.log()?;
    (a - b)? + c
}

/// Per-sample loss [n] and the weighted mean.
pub fn loss(net: &Trainable, t: &Tensors, idx: &Tensor, shape: &Shape, steer_l1: bool) -> candle_core::Result<(Tensor, Tensor)> {
    let x = t.x.index_select(idx, 0)?;
    let ys = t.steer.index_select(idx, 0)?;
    let yc = t.cls.index_select(idx, 0)?;
    let yg = t.gas.index_select(idx, 0)?;
    let yb = t.brake.index_select(idx, 0)?;
    let w = t.w.index_select(idx, 0)?;
    let raw = net.forward(&x)?;
    let (so, go, bo, _) = shape.offsets();
    let k = shape.k;
    let steer_l = match shape.head {
        HeadKind::AnalogPedals => {
            let ps = raw.narrow(1, so, k)?.tanh()?;
            let d = (ps - &ys)?;
            if steer_l1 { d.abs()?.sum(D::Minus1)? } else { d.sqr()?.sum(D::Minus1)? }
        }
        HeadKind::Categorical => {
            // B-way CE per tick over the bin centres.
            let b = shape.bins;
            let logits = raw.narrow(1, so, b * k)?.reshape((idx.dim(0)?, k, b))?;
            let lsm = candle_nn::ops::log_softmax(&logits, D::Minus1)?;
            let cls = yc.unsqueeze(D::Minus1)?;
            lsm.gather(&cls, D::Minus1)?.squeeze(D::Minus1)?.neg()?.sum(D::Minus1)?
        }
    };
    let gas_l = bce_with_logits(&raw.narrow(1, go, k)?, &yg)?.sum(D::Minus1)?;
    let brake_l = bce_with_logits(&raw.narrow(1, bo, k)?, &yb)?.sum(D::Minus1)?;
    let per = ((steer_l + gas_l)? + brake_l)?.affine(1.0 / k as f64, 0.0)?; // mean over chunk ticks
    let wsum = w.sum_all()?;
    let mean = ((&per * &w)?.sum_all()? / wsum)?;
    Ok((per, mean))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics {
    pub n: usize,
    pub steer_mae_i8: f64,
    pub gas_acc: f64,
    pub brake_acc: f64,
    pub steer_sign_acc: f64, // fraction of ticks where sign(pred)==sign(target) among |target|>0
}

impl Metrics {
    pub fn row(&self) -> String {
        format!("steer MAE {:.2} i8 | steer-sign acc {:.3} | gas acc {:.4} | brake acc {:.4} (n={})", self.steer_mae_i8, self.steer_sign_acc, self.gas_acc, self.brake_acc, self.n)
    }
}

/// Metrics from predicted action chunks vs samples.
pub fn metrics_from(pred: &[Vec<Action>], s: &[Sample]) -> Metrics {
    let mut m = Metrics { n: s.len(), ..Default::default() };
    let (mut mae, mut ga, mut ba, mut sg, mut nsg, mut cnt) = (0f64, 0usize, 0usize, 0usize, 0usize, 0usize);
    for (p, smp) in pred.iter().zip(s) {
        for (a, b) in p.iter().zip(&smp.acts) {
            mae += (a.steer as f64 - b.steer as f64).abs();
            ga += (a.gas == b.gas) as usize;
            ba += (a.brake == b.brake) as usize;
            if b.steer != 0 {
                nsg += 1;
                sg += (a.steer.signum() == b.steer.signum()) as usize;
            }
            cnt += 1;
        }
    }
    let c = cnt.max(1) as f64;
    m.steer_mae_i8 = mae / c;
    m.gas_acc = ga as f64 / c;
    m.brake_acc = ba as f64 / c;
    m.steer_sign_acc = sg as f64 / nsg.max(1) as f64;
    m
}

pub fn baseline_hold(s: &[Sample], k: usize) -> Vec<Vec<Action>> {
    s.iter().map(|x| vec![x.prev.unwrap_or(Action { steer: 0, gas: true, brake: false }); k]).collect()
}
pub fn baseline_const(s: &[Sample], k: usize) -> Vec<Vec<Action>> {
    s.iter().map(|_| vec![Action { steer: 0, gas: true, brake: false }; k]).collect()
}

/// Predictions with the hand-written forward (what the env will run).
pub fn predict_hand(w: &Weights, s: &[Sample]) -> Vec<Vec<Action>> {
    s.iter().map(|x| w.act(&x.obs)).collect()
}

/// Predictions with candle, batched.
pub fn predict_candle(net: &Trainable, t: &Tensors, w: &Weights, bs: usize) -> candle_core::Result<Vec<Vec<Action>>> {
    let mut out = Vec::with_capacity(t.n);
    let mut i = 0;
    while i < t.n {
        let n = bs.min(t.n - i);
        let x = t.x.narrow(0, i, n)?;
        let raw = net.forward(&x)?.to_vec2::<f32>()?;
        for r in raw {
            out.push(w.decode(&r).0);
        }
        i += n;
    }
    Ok(out)
}

pub struct BcArgs {
    /// Observation version (tmobs v1 = 80, v2 = 100 with the vehicle block, v3 = 108 with effects).
    pub obs_version: u32,
    /// Network width and depth (hidden units per layer, number of hidden layers).
    pub hidden: usize,
    pub layers: usize,
    /// Load every `<dir>/*/geom.json` (DATA's maps/ layout) in addition to --geom.
    pub geom_dir: Option<String>,
    /// INTERFACES.md split rule: held-out = fnv1a64(map_uid) % 10 == 0. Those maps' records go to the held-out
    /// set for metrics and are NEVER trained on.
    pub split_fnv: bool,
    /// Keep only these maps (uids); empty = all.
    pub include_maps: Vec<String>,
    /// Keep only records of this map (a DATA shard mixes maps).
    pub map_uid: Option<String>,
    /// Drop these ghost ids (e.g. keyboard/Action-Key ghosts until DATA relabels them).
    pub exclude_ghosts: Vec<u32>,
    /// Hold out these ghost ids from the training shards (when no separate held-out shard exists).
    pub heldout_ghosts: Vec<u32>,
    pub lag: usize,
    pub bins: usize,
    pub steer_l1: bool,
    pub train: Vec<String>,
    pub heldout: Vec<String>,
    pub geom: Vec<String>,
    pub out: String,
    pub k: usize,
    pub stride: usize,
    pub head: HeadKind,
    pub epochs: usize,
    pub batch: usize,
    pub lr: f64,
    pub seed: u64,
    pub init: Option<String>,
}

fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// FNV-1a 64 of the map uid, the split hash of INTERFACES.md.
pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn is_heldout_map(uid: &str) -> bool {
    fnv1a64(uid) % 10 == 0
}

pub fn load_geoms(paths: &[String]) -> Result<BTreeMap<String, TrackGeom>, String> {
    let mut m = BTreeMap::new();
    for p in paths {
        let g: TrackGeom = serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?).map_err(|e| format!("{p}: {e}"))?;
        m.insert(g.map_uid.clone(), g);
    }
    Ok(m)
}

pub fn run(a: &BcArgs) -> Result<(), String> {
    let t_start = Instant::now();
    let dev = Device::Cpu;
    let mut geoms = load_geoms(&a.geom)?;
    if let Some(dir) = &a.geom_dir {
        let mut n = 0;
        for e in std::fs::read_dir(dir).map_err(|e| format!("{dir}: {e}"))? {
            let p = e.map_err(|e| e.to_string())?.path().join("geom.json");
            if p.exists() {
                match load_geoms(&[p.to_string_lossy().into_owned()]) {
                    Ok(g) => {
                        n += g.len();
                        geoms.extend(g);
                    }
                    Err(err) => eprintln!("WARNING skipping {}: {err}", p.display()),
                }
            }
        }
        println!("loaded {n} geometries from {dir}");
    }
    let keep = |r: &Record| a.map_uid.as_ref().map(|u| &r.map_uid == u).unwrap_or(true) && !a.exclude_ghosts.contains(&r.ghost_id);
    let mut train_recs = Vec::new();
    let mut held_recs = Vec::new();
    for p in &a.train {
        let n0 = train_recs.len() + held_recs.len();
        for r in read_shard(p)?.into_iter().filter(|r| keep(r)) {
            if !a.include_maps.is_empty() && !a.include_maps.contains(&r.map_uid) {
                continue;
            }
            if a.heldout_ghosts.contains(&r.ghost_id) || (a.split_fnv && is_heldout_map(&r.map_uid)) { held_recs.push(r) } else { train_recs.push(r) }
        }
        println!("{p}: kept {} records", train_recs.len() + held_recs.len() - n0);
    }
    for p in &a.heldout {
        held_recs.extend(read_shard(p)?.into_iter().filter(|r| keep(r)));
    }
    {
        let mut ids: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
        for r in &train_recs {
            *ids.entry(r.ghost_id).or_default() += 1;
        }
        let mut hids: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
        for r in &held_recs {
            *hids.entry(r.ghost_id).or_default() += 1;
        }
        println!("train ghosts {:?}", ids);
        println!("heldout ghosts {:?}", hids);
    }
    let t0 = Instant::now();
    let train_s = build_samples(&train_recs, &geoms, a.k, a.stride, a.lag, a.obs_version)?;
    let held_s = build_samples(&held_recs, &geoms, a.k, 1, a.lag, a.obs_version)?;
    if !(1..=4).contains(&a.obs_version) {
        return Err(format!("obs version {} unknown (1..=4)", a.obs_version));
    }
    let obs_dim = tmobs::obs_dim(a.obs_version);
    println!(
        "samples: train {} (from {} records), heldout {} (from {} records), k={}, stride={}, built in {:.1}s",
        train_s.len(), train_recs.len(), held_s.len(), held_recs.len(), a.k, a.stride, t0.elapsed().as_secs_f64()
    );
    if train_s.is_empty() {
        return Err("no training samples".into());
    }
    let shape = Shape { obs_dim, hidden: a.hidden, layers: a.layers, k: a.k, head: a.head, bins: if a.head == HeadKind::Categorical { a.bins } else { 0 } };
    let tr = to_tensors(&train_s, a.k, obs_dim, shape.bins, &dev).map_err(|e| e.to_string())?;
    let he = to_tensors(&held_s, a.k, obs_dim, shape.bins, &dev).map_err(|e| e.to_string())?;

    // Baselines first: they set the bar the network has to clear.
    if !held_s.is_empty() {
        println!("baseline HOLD (heldout): {}", metrics_from(&baseline_hold(&held_s, a.k), &held_s).row());
        println!("baseline CONST      (heldout): {}", metrics_from(&baseline_const(&held_s, a.k), &held_s).row());
    }
    println!("baseline HOLD (train):   {}", metrics_from(&baseline_hold(&train_s, a.k), &train_s).row());

    let net = match &a.init {
        Some(p) => {
            let w = crate::policy::read_checked(p, a.obs_version, obs_dim)?;
            if w.shape != shape {
                return Err(format!("--init {p} has shape {:?}, this run wants {:?}", w.shape, shape));
            }
            Trainable::from_weights(&w, &dev).map_err(|e| e.to_string())?
        }
        None => Trainable::new(shape, &dev).map_err(|e| e.to_string())?,
    };
    println!("net {:?}: {} params, n_out {}", shape, net.n_params(), shape.n_out());
    let mut opt = AdamW::new(net.varmap.all_vars(), ParamsAdamW { lr: a.lr, weight_decay: 0.01, ..Default::default() }).map_err(|e| e.to_string())?;

    std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    let mut curve = String::from("epoch\ttrain_loss\theldout_loss\theld_steer_mae_i8\theld_steer_sign_acc\theld_gas_acc\theld_brake_acc\tepoch_s\tsamples_per_s\n");
    let mut rng = a.seed.max(1);
    let mut idx: Vec<u32> = (0..tr.n as u32).collect();
    let all_held = Tensor::arange(0u32, he.n as u32, &dev).map_err(|e| e.to_string())?;
    let mut best = (f64::INFINITY, None::<Weights>);
    for ep in 1..=a.epochs {
        let te = Instant::now();
        // Fisher-Yates
        for i in (1..idx.len()).rev() {
            let j = (xorshift(&mut rng) % (i as u64 + 1)) as usize;
            idx.swap(i, j);
        }
        let mut tot = 0f64;
        let mut nb = 0usize;
        for chunk in idx.chunks(a.batch) {
            let bi = Tensor::from_vec(chunk.to_vec(), chunk.len(), &dev).map_err(|e| e.to_string())?;
            let (_, l) = loss(&net, &tr, &bi, &shape, a.steer_l1).map_err(|e| e.to_string())?;
            opt.backward_step(&l).map_err(|e| e.to_string())?;
            tot += l.to_scalar::<f32>().map_err(|e| e.to_string())? as f64;
            nb += 1;
        }
        let train_loss = tot / nb.max(1) as f64;
        let w = net.snapshot().map_err(|e| e.to_string())?;
        let (held_loss, m) = if he.n > 0 {
            let (_, l) = loss(&net, &he, &all_held, &shape, a.steer_l1).map_err(|e| e.to_string())?;
            let hl = l.to_scalar::<f32>().map_err(|e| e.to_string())? as f64;
            let pred = predict_candle(&net, &he, &w, 8192).map_err(|e| e.to_string())?;
            (hl, metrics_from(&pred, &held_s))
        } else {
            (f64::NAN, Metrics::default())
        };
        let ntr = tr.n.min(20000);
        let trm = metrics_from(&predict_candle(&net, &Tensors { x: tr.x.narrow(0, 0, ntr).map_err(|e| e.to_string())?, steer: tr.steer.clone(), cls: tr.cls.clone(), gas: tr.gas.clone(), brake: tr.brake.clone(), w: tr.w.clone(), n: ntr }, &w, 8192).map_err(|e| e.to_string())?, &train_s[..ntr]);
        let dt = te.elapsed().as_secs_f64();
        println!(
            "epoch {ep:>3}  train {train_loss:.4}  heldout {held_loss:.4}  | held {}  | train steer MAE {:.2} sign {:.3} | {:.1}s, {:.0} samples/s",
            m.row(), trm.steer_mae_i8, trm.steer_sign_acc, dt, tr.n as f64 / dt
        );
        curve.push_str(&format!("{ep}\t{train_loss:.6}\t{held_loss:.6}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{dt:.2}\t{:.0}\n", m.steer_mae_i8, m.steer_sign_acc, m.gas_acc, m.brake_acc, tr.n as f64 / dt));
        std::fs::write(format!("{}/curve.tsv", a.out), &curve).map_err(|e| e.to_string())?;
        let score = if held_loss.is_nan() { train_loss } else { held_loss };
        if score < best.0 {
            best = (score, Some(w.clone()));
            crate::policy::write(&format!("{}/policy.tmw", a.out), &w, a.obs_version)?;
        }
        crate::policy::write(&format!("{}/policy-last.tmw", a.out), &w, a.obs_version)?;
    }
    let best_w = best.1.ok_or("no epochs ran")?;

    // The artefact round trip, and the two forwards on REAL data: reload policy.tmw, predict the held-out set
    // with the hand-written forward, and require the same actions candle produced for the same weights.
    let loaded = crate::policy::read_checked(&format!("{}/policy.tmw", a.out), a.obs_version, obs_dim)?;
    let agree = loaded.agrees_with(&Trainable::from_weights(&best_w, &dev).map_err(|e| e.to_string())?, &dev, 64, 1e-4)?;
    let eval_set = if held_s.is_empty() { &train_s } else { &held_s };
    let hand = predict_hand(&loaded, eval_set);
    let cand_net = Trainable::from_weights(&loaded, &dev).map_err(|e| e.to_string())?;
    let cand = predict_candle(&cand_net, if held_s.is_empty() { &tr } else { &he }, &loaded, 8192).map_err(|e| e.to_string())?;
    let mut diff_ticks = 0usize;
    let mut tot_ticks = 0usize;
    for (h, c) in hand.iter().zip(&cand) {
        for (x, y) in h.iter().zip(c) {
            tot_ticks += 1;
            if x != y {
                diff_ticks += 1;
            }
        }
    }
    let mh = metrics_from(&hand, eval_set);
    println!("policy.tmw reloaded: hand-written forward vs candle on {} random inputs: worst |Δ| {agree:.2e}", 64);
    println!("policy.tmw reloaded: hand-written forward on the eval set: {}", mh.row());
    println!("policy.tmw reloaded: decoded actions hand vs candle differ on {diff_ticks} of {tot_ticks} ticks (quantisation boundary hits only)");
    if diff_ticks as f64 > 0.001 * tot_ticks as f64 {
        return Err(format!("hand-written and candle forwards decode to different actions on {diff_ticks}/{tot_ticks} ticks — more than float noise"));
    }
    println!("done in {:.1}s; wrote {}/policy.tmw (best heldout), policy-last.tmw, curve.tsv", t_start.elapsed().as_secs_f64(), a.out);
    Ok(())
}
