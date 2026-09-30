//! `tmrl` — the LEARN arm's binary: BC trainer, policy artefact, self-tests (PPO / tracking / eval to follow).

use tmrl::{bc, bcnet, eval, policy, ppo_bc, ppo_train, refs, rollout, shard, synth};

use bcnet::{HeadKind, Shape, Trainable, HIDDEN};
use std::collections::HashMap;

const USAGE: &str = "tmrl -- the learned player (LEARN arm)

  tmrl synth --route R.json --pack P.json --ghosts DIR --out DIR [--heldout-mod 4] [--geom-source cartographer|wr]
        One-map BC shard from Nadeo ghosts: geom.json, train.tmd, heldout.tmd, manifest.tsv.
  tmrl bc --train A.tmd[,B.tmd] --heldout H.tmd --geom geom.json[,..] --out DIR
          [--k 10] [--stride 1] [--head analog|keyboard|cat] [--bins 21] [--epochs 30] [--batch 512] [--lr 1e-3] [--seed 1]
          [--init policy.tmw] [--threads 32] [--steer-loss mse|l1] [--action-lag 0] [--map-uid UID] [--exclude-ghosts 1,2] [--heldout-ghosts 3,4] [--geom-dir data/v0/maps] [--split-fnv]
        Behaviour cloning with chunked actions. Writes policy.tmw (best held-out), policy-last.tmw, curve.tsv.
  tmrl selftest [--k 10]
        Both forwards agree on random weights (analog AND keyboard heads); a perturbed copy is REFUSED;
        policy.tmw round-trips bit-exactly; a corrupted file is refused.
  tmrl rollout --policy P.tmw --geom geom.json --map M.Map.Gbx --ref REF.Ghost.Gbx --out DIR
          [--episodes 1] [--max-ticks 1900] [--max-steps 400] [--temp 0] [--seed 1] [--verbose] [--no-oracle] [--const] [--tape GHOST.Gbx (open-loop replay control) [--tape-shift-ms 0]]
          [--server $TM_SERVER] [--shim $FK_SHIM] [--work DIR]
        Zero-shot closed loop in the fork env; every episode's tape written and re-simulated by the plain oracle.
  tmrl ppo --init policy.tmw --geom geom.json --map M --ref REF --out DIR [--workers 32] [--iters 50]
          [--steps-per-worker 256] [--max-ep-steps 220] [--max-ticks 2100] [--lr 1e-4] [--ent 0.001] [--clip 0.1]
          [--ppo-epochs 4] [--minibatch 1024] [--temp 1.0] [--seed 1] [--p-archive 0.5 --snap-every 20 --bucket-m 100]
        PPO fine-tune of a categorical BC policy in a tmenv fleet (chunk log-probs); new bests to the plain oracle.
  tmrl eval --policy P.tmw --maps-dir data/v0/maps --out DIR [--maps uid,uid] [--max-maps 30] [--episodes 5] [--temp 1.0]
          [--seed 11] [--tape-factor 2.0] [--skip-const]
        L4 harness: held-out maps (fnv split) or listed uids; per map a template from the slowest ghost, CONST control,
        sampled episodes, plain oracle per episode; table.tsv + medals.
  tmrl template --map-dir data/v0/maps/<uid> --out DIR [--tape-ms 60000]
        The env reference for a map: slowest ghost → lengthened template (declared + walltime) → from-template.
  tmrl policy info policy.tmw
        Header and tensor shapes.
  tmrl train --map M --ref R [...]      PPO in the fork env (ENV arm's a7aa56c port; `tmrl train` with no args prints its help).
  tmrl ppo-selftest                      the PPO network + advantage controls alone (also run by `tmrl selftest`).

Threads: candle's CPU backend uses RAYON_NUM_THREADS; tmrl sets it to --threads (default 32) when unset,
so a training run never takes the whole box (COMMON-RULES 8: leave >= 8 cores free).";

fn parse(args: &[String]) -> (Vec<String>, HashMap<String, String>) {
    let mut pos = Vec::new();
    let mut kv = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        if let Some(k) = args[i].strip_prefix("--") {
            if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                kv.insert(k.to_string(), args[i + 1].clone());
                i += 2;
            } else {
                kv.insert(k.to_string(), "true".into());
                i += 1;
            }
        } else {
            pos.push(args[i].clone());
            i += 1;
        }
    }
    (pos, kv)
}

fn need(kv: &HashMap<String, String>, k: &str) -> Result<String, String> {
    kv.get(k).cloned().ok_or_else(|| format!("missing --{k}"))
}

fn num<T: std::str::FromStr>(kv: &HashMap<String, String>, k: &str, d: T) -> Result<T, String> {
    match kv.get(k) {
        None => Ok(d),
        Some(v) => v.parse().map_err(|_| format!("--{k}: cannot parse '{v}'")),
    }
}

fn list(s: &str) -> Vec<String> {
    s.split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect()
}

fn selftest(k: usize) -> Result<(), String> {
    let dev = candle_core::Device::Cpu;
    for (head, bins) in [(HeadKind::AnalogPedals, 0usize), (HeadKind::Categorical, 3), (HeadKind::Categorical, 21)] {
        let shape = Shape { obs_dim: tmobs::OBS_DIM, hidden: HIDDEN, layers: 2, k, head, bins };
        let t = Trainable::new(shape, &dev).map_err(|e| e.to_string())?;
        let w = t.snapshot().map_err(|e| e.to_string())?;
        let worst = w.agrees_with(&t, &dev, 64, 1e-4)?;
        println!("[{}/{bins}] agreement: candle vs hand-written forward, 64 random inputs, worst |Δ| {worst:.2e}  PASS", head.name());
        // Negative half: a perturbed copy must be refused.
        let mut bad = w.clone();
        bad.perturb_first_bias(1e-2);
        match bad.agrees_with(&t, &dev, 64, 1e-4) {
            Ok(x) => return Err(format!("[{}] NEGATIVE HALF FAILED: a perturbed copy was accepted (worst {x:.2e}); the check cannot fail and certifies nothing", head.name())),
            Err(_) => println!("[{}/{bins}] negative half: perturbed copy REFUSED  PASS", head.name()),
        }
        // Artefact round trip, bit-exact.
        let p = format!("/tmp/tmrl-selftest-{}-{bins}.tmw", head.name());
        policy::write(&p, &w, tmobs::OBS_VERSION)?;
        let back = policy::read_checked(&p, tmobs::OBS_VERSION, tmobs::OBS_DIM)?;
        let same = w.tensors().iter().zip(back.tensors()).all(|((a, _), (b, _))| a.iter().zip(b.iter()).all(|(x, y)| x.to_bits() == y.to_bits()));
        if !same || back.shape != w.shape {
            return Err(format!("[{}] policy.tmw round trip is not bit-exact", head.name()));
        }
        println!("[{}] policy.tmw round trip: {} bytes, bit-exact  PASS", head.name(), std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0));
        // A corrupted file is refused (truncate one byte; flip the obs version).
        let mut bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
        bytes.pop();
        std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
        if policy::read(&p).is_ok() {
            return Err("truncated policy.tmw was accepted".into());
        }
        bytes.push(0);
        bytes[8] ^= 1; // OBS_VERSION
        std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
        if policy::read_checked(&p, tmobs::OBS_VERSION, tmobs::OBS_DIM).is_ok() {
            return Err("policy.tmw with a foreign OBS_VERSION was accepted".into());
        }
        println!("[{}] truncated file refused, foreign OBS_VERSION refused  PASS", head.name());
        // Decode sanity: the quantiser hits the grid ends.
        assert_eq!(bcnet::quantise_steer(1.0), 127);
        assert_eq!(bcnet::quantise_steer(-1.0), -127);
        assert_eq!(bcnet::quantise_steer(0.003), 0);
        let acts = w.act(&vec![0.1f32; tmobs::OBS_DIM]);
        if acts.len() != k {
            return Err(format!("decode gave {} actions for k={k}", acts.len()));
        }
        let _ = std::fs::remove_file(&p);
    }
    println!("SELFTEST PASS");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (pos, kv) = parse(&args);
    if std::env::var_os("RAYON_NUM_THREADS").is_none() {
        let th = kv.get("threads").cloned().unwrap_or_else(|| "32".into());
        std::env::set_var("RAYON_NUM_THREADS", th);
    }
    let r = match pos.first().map(|s| s.as_str()) {
        Some("synth") => (|| {
            synth::run(&synth::SynthArgs { route: need(&kv, "route")?, pack: need(&kv, "pack")?, ghosts_dir: need(&kv, "ghosts")?, out: need(&kv, "out")?, heldout_mod: num(&kv, "heldout-mod", 4u32)?, geom_source: kv.get("geom-source").cloned().unwrap_or_else(|| "cartographer".into()) })
        })(),
        Some("bc") => (|| {
            let head_s = kv.get("head").cloned().unwrap_or_else(|| "analog".into());
            let head = HeadKind::parse(&head_s).ok_or_else(|| format!("--head {head_s}: analog|keyboard|cat"))?;
            let bins = if head_s == "keyboard" { 3 } else { num(&kv, "bins", 21usize)? };
            bc::run(&bc::BcArgs {
                train: list(&need(&kv, "train")?),
                heldout: kv.get("heldout").map(|s| list(s)).unwrap_or_default(),
                geom: kv.get("geom").map(|s| list(s)).unwrap_or_default(),
                out: need(&kv, "out")?,
                k: num(&kv, "k", 10usize)?,
                stride: num(&kv, "stride", 1usize)?,
                head,
                epochs: num(&kv, "epochs", 30usize)?,
                batch: num(&kv, "batch", 512usize)?,
                lr: num(&kv, "lr", 1e-3f64)?,
                seed: num(&kv, "seed", 1u64)?,
                init: kv.get("init").cloned(),
                bins,
                lag: num(&kv, "action-lag", 0usize)?,
                map_uid: kv.get("map-uid").cloned(),
                geom_dir: kv.get("geom-dir").cloned(),
                hidden: num(&kv, "hidden", bcnet::HIDDEN)?,
                obs_version: num(&kv, "obs-version", 1u32)?,
                layers: num(&kv, "layers", 2usize)?,
                split_fnv: kv.contains_key("split-fnv"),
                include_maps: kv.get("include-maps").map(|s| list(s)).unwrap_or_default(),
                exclude_ghosts: kv.get("exclude-ghosts").map(|s| list(s).iter().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default(),
                heldout_ghosts: kv.get("heldout-ghosts").map(|s| list(s).iter().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default(),
                steer_l1: kv.get("steer-loss").map(|s| s == "l1").unwrap_or(false),
            })
        })(),
        Some("trace") => (|| trace(&need(&kv, "shard")?, &need(&kv, "geom")?, num(&kv, "ghost", 1u32)?, num(&kv, "every", 25usize)?))(),
        Some("obsstats") => (|| obsstats(&need(&kv, "shard")?, &need(&kv, "geom")?))(),
        Some("rollout") => (|| {
            let env_or = |k: &str, e: &str, d: &str| kv.get(k).cloned().or_else(|| std::env::var(e).ok()).unwrap_or_else(|| d.to_string());
            rollout::run(&rollout::RolloutArgs {
                policy: need(&kv, "policy")?,
                geom: need(&kv, "geom")?,
                map: std::path::PathBuf::from(need(&kv, "map")?),
                reference: std::path::PathBuf::from(need(&kv, "ref")?),
                server: std::path::PathBuf::from(env_or("server", "TM_SERVER", "/tmp/tmoracle/server")),
                shim: std::path::PathBuf::from(env_or("shim", "FK_SHIM", "/tmp/tmtas/tools/search/target/release/libforkshim.so")),
                work: std::path::PathBuf::from(kv.get("work").cloned().unwrap_or_else(|| format!("/tmp/tmrl-rollout-{}", std::process::id()))),
                out: std::path::PathBuf::from(need(&kv, "out")?),
                episodes: num(&kv, "episodes", 1usize)?,
                max_ticks: num(&kv, "max-ticks", 1900usize)?,
                max_steps: num(&kv, "max-steps", 400usize)?,
                temp: num(&kv, "temp", 0.0f32)?,
                seed: num(&kv, "seed", 1u64)?,
                verbose: kv.contains_key("verbose"),
                no_oracle: kv.contains_key("no-oracle"),
                const_ctrl: kv.contains_key("const"),
                tape: kv.get("tape").cloned(),
                tape_shift_ms: num(&kv, "tape-shift-ms", 0i64)?,
                no_cut: kv.contains_key("no-cut"),
                margin_m: num(&kv, "margin-m", 4.0f32)?,
                refs: kv.get("refs").cloned(),
            })
            .map(|_| ())
        })(),
        Some("tape-into-template") => (|| rollout::tape_into_template(&need(&kv, "template")?, &need(&kv, "tape")?, &need(&kv, "out")?, kv.get("digital-to").map(|v| v.parse::<i8>().map_err(|_| "--digital-to i8".to_string())).transpose()?, kv.contains_key("effective")))(),
        Some("ppo") => (|| {
            let env_or = |k: &str, e: &str, d: &str| kv.get(k).cloned().or_else(|| std::env::var(e).ok()).unwrap_or_else(|| d.to_string());
            ppo_bc::run(&ppo_bc::PpoArgs {
                init: need(&kv, "init")?,
                geom: kv.get("geom").cloned().unwrap_or_default(),
                map: std::path::PathBuf::from(kv.get("map").cloned().unwrap_or_default()),
                reference: std::path::PathBuf::from(kv.get("ref").cloned().unwrap_or_default()),
                server: std::path::PathBuf::from(env_or("server", "TM_SERVER", "/tmp/tmoracle/server")),
                shim: std::path::PathBuf::from(env_or("shim", "FK_SHIM", "/tmp/tmtas/tools/search/target/release/libforkshim.so")),
                work: std::path::PathBuf::from(kv.get("work").cloned().unwrap_or_else(|| format!("/tmp/tmrl-ppo-{}", std::process::id()))),
                out: std::path::PathBuf::from(need(&kv, "out")?),
                workers: num(&kv, "workers", 32usize)?,
                iters: num(&kv, "iters", 50usize)?,
                steps_per_worker: num(&kv, "steps-per-worker", 256usize)?,
                max_ep_steps: num(&kv, "max-ep-steps", 220usize)?,
                max_ticks: num(&kv, "max-ticks", 2100usize)?,
                lr: num(&kv, "lr", 1e-4f64)?,
                ent: num(&kv, "ent", 0.001f32)?,
                clip: num(&kv, "clip", 0.1f32)?,
                epochs: num(&kv, "ppo-epochs", 4usize)?,
                minibatch: num(&kv, "minibatch", 1024usize)?,
                temp: num(&kv, "temp", 1.0f32)?,
                seed: num(&kv, "seed", 1u64)?,
                maps_dir: kv.get("maps-dir").cloned(),
                maps: kv.get("maps").map(|s| list(s)).unwrap_or_default(),
                tape_factor: num(&kv, "tape-factor", 2.0f32)?,
                tape_cap_ms: num(&kv, "tape-cap-ms", 60_000u32)?,
                p_archive: num(&kv, "p-archive", 0.0f32)?,
                snap_every: num(&kv, "snap-every", 20usize)?,
                bucket_m: num(&kv, "bucket-m", 100.0f32)?,
                margin_m: num(&kv, "margin-m", 4.0f32)?,
                p_frontier: num(&kv, "p-frontier", 0.0f32)?,
                refs_dir: kv.get("refs-dir").cloned(),
                track_w0: num(&kv, "track-w0", 0.0f32)?,
                track_anneal: num(&kv, "track-anneal", 0.6f32)?,
                human_archive: kv.contains_key("human-archive"),
                rotate_every: num(&kv, "rotate-every", 0usize)?,
            })
        })(),
        Some("walltime") => (|| rollout::set_walltime(pos.get(1).ok_or("tmrl walltime IN OUT --race-ms N")?, pos.get(2).ok_or("tmrl walltime IN OUT --race-ms N")?, num(&kv, "race-ms", 0u32)?))(),
        Some("eval") => (|| {
            let env_or = |k: &str, e: &str, d: &str| kv.get(k).cloned().or_else(|| std::env::var(e).ok()).unwrap_or_else(|| d.to_string());
            eval::run(&eval::EvalArgs {
                policy: need(&kv, "policy")?,
                maps_dir: need(&kv, "maps-dir")?,
                maps: kv.get("maps").map(|s| list(s)).unwrap_or_default(),
                max_maps: num(&kv, "max-maps", 30usize)?,
                episodes: num(&kv, "episodes", 5usize)?,
                temp: num(&kv, "temp", 1.0f32)?,
                seed: num(&kv, "seed", 11u64)?,
                tape_factor: num(&kv, "tape-factor", 2.0f32)?,
                tape_cap_ms: num(&kv, "tape-cap-ms", 60_000u32)?,
                out: std::path::PathBuf::from(need(&kv, "out")?),
                work: std::path::PathBuf::from(kv.get("work").cloned().unwrap_or_else(|| format!("/tmp/tmrl-eval-{}", std::process::id()))),
                server: std::path::PathBuf::from(env_or("server", "TM_SERVER", "/tmp/tmoracle/server")),
                shim: std::path::PathBuf::from(env_or("shim", "FK_SHIM", "/tmp/tmtas/tools/search/target/release/libforkshim.so")),
                threads_note: 0,
                skip_const: kv.contains_key("skip-const"),
                margin_m: num(&kv, "margin-m", 4.0f32)?,
                sanity_dir: kv.get("sanity-dir").cloned(),
            })
            .map(|_| ())
        })(),
        Some("template") => (|| {
            let r = eval::build_reference(std::path::Path::new(&need(&kv, "map-dir")?), std::path::Path::new(&need(&kv, "out")?), num(&kv, "tape-ms", 60000u32)?)?;
            println!("reference {}", r.display());
            Ok(())
        })(),
        Some("probe") => (|| probe_trace(&need(&kv, "geom")?, &need(&kv, "trace")?, num(&kv, "last", 5usize)?))(),
        Some("refs") => (|| refs::build_all(&need(&kv, "manifest")?, &need(&kv, "shards")?, &need(&kv, "geom-dir")?, &need(&kv, "out")?, &kv.get("maps").map(|s| list(s)).unwrap_or_default()))(),
        Some("fields") => (|| fields(&need(&kv, "shard")?, kv.get("map-uid").map(|s| s.as_str())))(),
        Some("deathprobe") => (|| deathprobe(&need(&kv, "geom")?, &need(&kv, "trace")?, kv.get("at").and_then(|s| s.parse().ok())))(),
        Some("deathtable") => (|| deathtable(&need(&kv, "eval-dir")?, &need(&kv, "maps-dir")?))(),
        Some("selftest") => (|| { ppo_train::main(&["ppo-selftest".to_string()]); selftest(num(&kv, "k", 10usize)?) })(),
        Some("ppo-selftest") | Some("train") => { ppo_train::main(&args); Ok(()) }
        Some("policy") if pos.get(1).map(|s| s.as_str()) == Some("info") => (|| {
            let p = pos.get(2).ok_or("tmrl policy info FILE")?;
            let l = policy::read(p)?;
            println!("{p}: OBS_VERSION {} shape {:?} n_out {}", l.obs_version, l.weights.shape, l.weights.shape.n_out());
            for (name, (v, dims)) in bcnet::tensor_names(l.weights.shape.layers).iter().zip(l.weights.tensors()) {
                let rms = (v.iter().map(|x| (x * x) as f64).sum::<f64>() / v.len().max(1) as f64).sqrt();
                println!("  {name:<12} {dims:?}  rms {rms:.4}");
            }
            Ok(())
        })(),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(e) = r {
        eprintln!("tmrl: {e}");
        std::process::exit(1);
    }
}

/// `tmrl trace --shard S.tmd --geom geom.json --ghost RANK [--every 25]` — the route probe along one ghost:
/// tick, race s, pos, arc length, lateral, corridor half-width, height, cps. The quickest way to see whether the
/// geometry and the humans agree.
pub fn trace(shard: &str, geom: &str, ghost: u32, every: usize) -> Result<(), String> {
    let geoms = bc::load_geoms(&[geom.to_string()])?;
    let recs = shard::read_shard(shard)?;
    let mut rs: Vec<&shard::Record> = recs.iter().filter(|r| r.ghost_id == ghost).collect();
    if rs.is_empty() {
        return Err(format!("ghost {ghost} not in {shard}"));
    }
    rs.sort_by_key(|r| r.tick);
    let g = geoms.get(&rs[0].map_uid).ok_or("no geometry for this map")?;
    println!("{:>5} {:>7} {:>8} {:>7} {:>8} {:>8} {:>7} {:>5} {:>6} {:>3} {:>5} {:>3} {:>3}", "tick", "race_s", "x", "y", "z", "s", "lat", "hw", "height", "cps", "steer", "gas", "brk");
    for (i, r) in rs.iter().enumerate() {
        let pr = tmobs::probe(g, &r.state);
        if i % every.max(1) == 0 || i + 1 == rs.len() {
            println!(
                "{:>5} {:>7.3} {:>8.2} {:>7.2} {:>8.2} {:>8.1} {:>7.2} {:>5.1} {:>6.2} {:>3} {:>5} {:>3} {:>3}",
                r.tick, r.state.race_ms as f64 / 1000.0, r.state.pos[0], r.state.pos[1], r.state.pos[2], pr.s, pr.lateral, pr.half_width, pr.height, r.state.cps, r.action.steer, r.action.gas as u8, r.action.brake as u8
            );
        }
    }
    Ok(())
}

/// `tmrl obsstats --shard S.tmd --geom geom.json` — per-feature mean / sd / min / max of the observation over a
/// shard, so a feature that dwarfs the others (or is constant) is visible before it wastes a training run.
pub fn obsstats(shard: &str, geom: &str) -> Result<(), String> {
    let geoms = bc::load_geoms(&[geom.to_string()])?;
    let recs = shard::read_shard(shard)?;
    let s = bc::build_samples(&recs, &geoms, 1, 1, 0, 1)?;
    let d = tmobs::OBS_DIM;
    let (mut mean, mut m2, mut mn, mut mx) = (vec![0f64; d], vec![0f64; d], vec![f64::INFINITY; d], vec![f64::NEG_INFINITY; d]);
    for (n, smp) in s.iter().enumerate() {
        for i in 0..d {
            let x = smp.obs[i] as f64;
            let delta = x - mean[i];
            mean[i] += delta / (n + 1) as f64;
            m2[i] += delta * (x - mean[i]);
            mn[i] = mn[i].min(x);
            mx[i] = mx[i].max(x);
        }
    }
    println!("{} samples; {:>3} {:>10} {:>10} {:>10} {:>10}", s.len(), "i", "mean", "sd", "min", "max");
    for i in 0..d {
        println!("{:>3} {:>10.4} {:>10.4} {:>10.4} {:>10.4}", i, mean[i], (m2[i] / s.len().max(1) as f64).sqrt(), mn[i], mx[i]);
    }
    Ok(())
}

/// `tmrl probe --geom geom.json --trace ep000.trace.csv [--last 5]` — where a trace ended relative to the geometry:
/// arc length, lateral offset, corridor half-width for the last rows (the report DATA needs to fix a half-width).
pub fn probe_trace(geom: &str, trace: &str, last: usize) -> Result<(), String> {
    let geoms = bc::load_geoms(&[geom.to_string()])?;
    let g = geoms.values().next().ok_or("no geometry")?;
    let text = std::fs::read_to_string(trace).map_err(|e| format!("{trace}: {e}"))?;
    let rows: Vec<Vec<f32>> = text.lines().skip(1).map(|l| l.split(',').map(|x| x.parse().unwrap_or(0.0)).collect()).collect();
    let n = rows.len();
    println!("{:>8} {:>8} {:>8} {:>8} {:>7} {:>6} {:>6}", "race_ms", "s", "frac", "x", "z", "lat", "hw");
    for r in rows.iter().skip(n.saturating_sub(last)) {
        let mut st = tmstate::CarState::unknown();
        st.pos = [r[1], r[2], r[3]];
        st.cps = 0;
        // cps unknown in the CSV: probe over the whole route by trying each leg and taking the nearest.
        let mut best: Option<tmobs::Probe> = None;
        for c in 0..=g.gates.len() as u8 {
            st.cps = c;
            let p = tmobs::probe(g, &st);
            let d = p.lateral.abs() + p.height.abs();
            if best.as_ref().map(|b| d < b.lateral.abs() + b.height.abs()).unwrap_or(true) {
                best = Some(p);
            }
        }
        let p = best.unwrap();
        println!("{:>8.0} {:>8.1} {:>8.3} {:>8.1} {:>7.1} {:>6.1} {:>6.1}", r[0], p.s, p.s / g.length().max(1.0), r[1], r[3], p.lateral, p.half_width);
    }
    Ok(())
}

/// `tmrl fields --shard S.tmd [--map-uid U]` — which CarState fields are KNOWN (finite / not u8::MAX) in a shard:
/// the question "can a v2/v3 observation be trained from this data?" answered by counting.
pub fn fields(shard: &str, map_uid: Option<&str>) -> Result<(), String> {
    let recs = shard::read_shard(shard)?;
    let recs: Vec<_> = recs.iter().filter(|r| map_uid.map(|u| r.map_uid == u).unwrap_or(true)).collect();
    let n = recs.len().max(1) as f32;
    let pct = |k: usize| 100.0 * k as f32 / n;
    println!("{} records{}", recs.len(), map_uid.map(|u| format!(" for {u}")).unwrap_or_default());
    println!("  gear known        {:6.2} %", pct(recs.iter().filter(|r| r.state.gear != u8::MAX).count()));
    println!("  rpm finite        {:6.2} %", pct(recs.iter().filter(|r| r.state.rpm.is_finite()).count()));
    println!("  wheel_contact     {:6.2} %", pct(recs.iter().filter(|r| r.state.wheel_contact[0] != u8::MAX).count()));
    println!("  wheel_material    {:6.2} %", pct(recs.iter().filter(|r| r.state.wheel_material[0] != u8::MAX).count()));
    println!("  wheel_slip finite {:6.2} %", pct(recs.iter().filter(|r| r.state.wheel_slip[0].is_finite()).count()));
    println!("  turbo finite      {:6.2} %", pct(recs.iter().filter(|r| r.state.turbo.is_finite()).count()));
    println!("  ang_vel finite    {:6.2} %", pct(recs.iter().filter(|r| r.state.ang_vel[0].is_finite()).count()));
    println!("  car known         {:6.2} %", pct(recs.iter().filter(|r| r.state.car != u8::MAX).count()));
    println!("  effects known     {:6.2} %", pct(recs.iter().filter(|r| r.state.effects & 0x80 != 0).count()));
    Ok(())
}

/// `tmrl deathprobe --geom geom.json --trace ep000.trace.csv [--at N]` — the coordinator's diagnostic (BAR M2-3): at
/// the trace's death point (its last row, or row N), rebuild the observation from the traced state and print the
/// 9-point lookahead block (route points in the car frame, metres) beside the geometry's true heading change ahead —
/// does the observation resolve the turn the car failed? Also the car's speed, lateral offset and half-width there.
pub fn deathprobe(geom: &str, trace: &str, at: Option<usize>) -> Result<(), String> {
    let geoms = bc::load_geoms(&[geom.to_string()])?;
    let g = geoms.values().next().ok_or("no geometry")?;
    let text = std::fs::read_to_string(trace).map_err(|e| format!("{trace}: {e}"))?;
    let rows: Vec<Vec<f32>> = text.lines().skip(1).map(|l| l.split(',').map(|x| x.parse().unwrap_or(0.0)).collect()).collect();
    if rows.is_empty() {
        return Err("empty trace".into());
    }
    // The death point: the last row where the car still moved (a stopped car repeats its position).
    let mut i = at.unwrap_or(rows.len() - 1).min(rows.len() - 1);
    if at.is_none() {
        while i > 1 && (rows[i][1] - rows[i - 1][1]).abs() + (rows[i][3] - rows[i - 1][3]).abs() < 0.01 {
            i -= 1;
        }
    }
    let r = &rows[i];
    let mut st = tmstate::CarState::unknown();
    st.race_ms = r[0] as i32;
    st.pos = [r[1], r[2], r[3]];
    st.vel = [r[4], r[5], r[6]];
    st.speed = (r[4] * r[4] + r[5] * r[5] + r[6] * r[6]).sqrt();
    st.quat = [r[7], r[8], r[9], r[10]];
    // Leg: the gate count is not in the CSV; take the leg whose projection is nearest.
    let mut best = (f32::MAX, 0u8);
    for c in 0..=g.gates.len() as u8 {
        st.cps = c;
        let p = tmobs::probe(g, &st);
        let d = p.lateral.abs() + p.height.abs();
        if d < best.0 {
            best = (d, c);
        }
    }
    st.cps = best.1;
    let p = tmobs::probe(g, &st);
    let o = tmobs::observe(g, &st, &[]);
    println!("death point row {i}: race {:.2} s, s {:.1} m ({:.3} of the route), speed {:.1} m/s, lateral {:.1} m, half-width {:.1} m, leg {}", r[0] / 1000.0, p.s, p.s / g.length().max(1.0), st.speed, p.lateral, p.half_width, st.cps);
    println!("{:>6} {:>9} {:>9} {:>9} {:>6} | {:>10} {:>10}", "ahead", "right_m", "up_m", "fwd_m", "hw_m", "route_yaw°", "turn°");
    let yaw0 = tmrl::refs::yaw_of(st.quat);
    let dir_at = |s: f32| -> f32 {
        let a = tmobs::at(g, s);
        let b = tmobs::at(g, s + 2.0);
        (b[0] - a[0]).atan2(b[2] - a[2])
    };
    let base_yaw = dir_at(p.s);
    for (j, la) in tmobs::LOOKAHEAD.iter().enumerate() {
        let k = 24 + 4 * j;
        let (x, y, z, hw) = (o[k] * tmobs::D_SCALE, o[k + 1] * tmobs::D_SCALE, o[k + 2] * tmobs::D_SCALE, o[k + 3] * 20.0);
        let ry = dir_at(p.s + la);
        let mut turn = (ry - base_yaw).to_degrees();
        while turn > 180.0 { turn -= 360.0; }
        while turn < -180.0 { turn += 360.0; }
        let mut rel = (ry - yaw0).to_degrees();
        while rel > 180.0 { rel -= 360.0; }
        while rel < -180.0 { rel += 360.0; }
        println!("{:>6.0} {:>9.1} {:>9.1} {:>9.1} {:>6.1} | {:>10.1} {:>10.1}", la, x, y, z, hw, rel, turn);
    }
    println!("(right/up/fwd = the route point in the car frame as the policy sees it; route_yaw° = route direction there relative to the car's heading; turn° = route direction change from the death point)");
    Ok(())
}

/// `tmrl deathtable --eval-dir runs/m2c/X --maps-dir DIR` — the deathprobe over every non-finishing episode trace of an
/// eval: per death, speed, |lateral| vs half-width, the route's turn within 55 m and within 120 m, and whether the car
/// had STOPPED (a hit) or was CUT (off-route). Then the aggregate: how many deaths sit on a turn the lookahead shows.
pub fn deathtable(eval_dir: &str, maps_dir: &str) -> Result<(), String> {
    let mut rows: Vec<(String, f32, f32, f32, f32, f32, bool)> = Vec::new(); // map, speed, |lat|, hw, turn55, turn120, stopped
    for e in std::fs::read_dir(eval_dir).map_err(|e| format!("{eval_dir}: {e}"))? {
        let p = e.map_err(|e| e.to_string())?.path();
        if !p.is_dir() {
            continue;
        }
        let uid = p.file_name().unwrap().to_string_lossy().into_owned();
        let gp = std::path::Path::new(maps_dir).join(&uid).join("geom.json");
        let Ok(geoms) = bc::load_geoms(&[gp.to_string_lossy().into_owned()]) else { continue };
        let Some(g) = geoms.values().next() else { continue };
        let name = std::fs::read_to_string(std::path::Path::new(maps_dir).join(&uid).join("map.json")).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v.get("name").and_then(|n| n.as_str()).map(|s| s.to_string())).unwrap_or(uid.clone());
        for ep in 0..20 {
            let tp = p.join(format!("ep{ep:03}.trace.csv"));
            let Ok(text) = std::fs::read_to_string(&tp) else { continue };
            let rows_t: Vec<Vec<f32>> = text.lines().skip(1).map(|l| l.split(',').map(|x| x.parse().unwrap_or(0.0)).collect()).collect();
            if rows_t.len() < 5 {
                continue;
            }
            // finished episodes reach the route end; skip them (progress ≥ 0.98 of the length)
            let mut i = rows_t.len() - 1;
            let stopped = (rows_t[i][1] - rows_t[i - 1][1]).abs() + (rows_t[i][3] - rows_t[i - 1][3]).abs() < 0.01;
            while i > 1 && (rows_t[i][1] - rows_t[i - 1][1]).abs() + (rows_t[i][3] - rows_t[i - 1][3]).abs() < 0.01 {
                i -= 1;
            }
            let r = &rows_t[i];
            let mut st = tmstate::CarState::unknown();
            st.pos = [r[1], r[2], r[3]];
            st.vel = [r[4], r[5], r[6]];
            st.speed = (r[4] * r[4] + r[5] * r[5] + r[6] * r[6]).sqrt();
            st.quat = [r[7], r[8], r[9], r[10]];
            let mut best = (f32::MAX, 0u8);
            for c in 0..=g.gates.len() as u8 {
                st.cps = c;
                let pr = tmobs::probe(g, &st);
                let d = pr.lateral.abs() + pr.height.abs();
                if d < best.0 {
                    best = (d, c);
                }
            }
            st.cps = best.1;
            let pr = tmobs::probe(g, &st);
            if pr.s / g.length().max(1.0) > 0.98 {
                continue;
            }
            let dir_at = |s: f32| -> f32 {
                let a = tmobs::at(g, s);
                let b = tmobs::at(g, s + 2.0);
                (b[0] - a[0]).atan2(b[2] - a[2])
            };
            let base = dir_at(pr.s);
            let wrap = |x: f32| {
                let mut d = x.to_degrees();
                while d > 180.0 { d -= 360.0; }
                while d < -180.0 { d += 360.0; }
                d
            };
            let t55 = wrap(dir_at(pr.s + 55.0) - base).abs();
            let t120 = wrap(dir_at(pr.s + 120.0) - base).abs();
            rows.push((name.clone(), st.speed, pr.lateral.abs(), pr.half_width, t55, t120, stopped));
        }
    }
    println!("{} deaths", rows.len());
    println!("{:<22} {:>6} {:>6} {:>5} {:>7} {:>7} {:>7}", "map", "speed", "|lat|", "hw", "turn55", "turn120", "how");
    for r in &rows {
        println!("{:<22} {:>6.1} {:>6.1} {:>5.1} {:>7.1} {:>7.1} {:>7}", r.0.chars().take(22).collect::<String>(), r.1, r.2, r.3, r.4, r.5, if r.6 { "STOPPED" } else { "cut" });
    }
    let n = rows.len().max(1) as f32;
    let stopped = rows.iter().filter(|r| r.6).count();
    let on_turn = rows.iter().filter(|r| r.4 > 20.0 || r.5 > 45.0).count();
    let straight = rows.iter().filter(|r| r.4 <= 10.0 && r.5 <= 20.0).count();
    let fast = rows.iter().filter(|r| r.1 > 60.0).count();
    println!("\nAGGREGATE: stopped (hit something) {} ({:.0} %), cut off-route {} ({:.0} %); at a turn ahead (>20° in 55 m or >45° in 120 m) {} ({:.0} %); on a straight (≤10° / ≤20°) {} ({:.0} %); above 60 m/s {} ({:.0} %)", stopped, 100.0 * stopped as f32 / n, rows.len() - stopped, 100.0 * (rows.len() - stopped) as f32 / n, on_turn, 100.0 * on_turn as f32 / n, straight, 100.0 * straight as f32 / n, fast, 100.0 * fast as f32 / n);
    Ok(())
}
