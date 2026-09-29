//! `tmrl rollout` — zero-shot closed-loop episodes of a `policy.tmw` in the fork env, every tape re-simulated by the
//! plain oracle. THE number that matters for BC (BRIEF-LEARN L1): finish rate and time, oracle-confirmed, beside the
//! env's own reading — never instead of it.
//!
//! Per episode: `reset`, then `step_actions(policy chunk)` until the env says done or `--max-steps` chunks. The
//! banked tape (driven prefix + stop tail, `ForkEnv::banked_tape`) is written and handed to `tmauto::oracle`.
//! `--temp 0` (default) drives the head's mode; `--temp T > 0` samples the categorical steer / Bernoulli pedals
//! (seeded), which is how a BC policy shows its spread.

use crate::bcnet::Weights;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tmenv::action::ActionSpace;
use tmenv::core::{CoreCfg, Done};
use tmstate::Action;

pub struct RolloutArgs {
    pub policy: String,
    pub geom: String,
    pub map: PathBuf,
    pub reference: PathBuf,
    pub server: PathBuf,
    pub shim: PathBuf,
    pub work: PathBuf,
    pub out: PathBuf,
    pub episodes: usize,
    pub max_ticks: usize,
    pub max_steps: usize,
    pub temp: f32,
    pub seed: u64,
    pub verbose: bool,
    pub no_oracle: bool,
    /// CONTROL: ignore the policy, drive steer 0 + gas every tick (the policy must beat this or it did nothing).
    pub const_ctrl: bool,
    /// POSITIVE CONTROL of the instrument: replay a recorded ghost's own 10 ms inputs open-loop (by race tick), so
    /// the env's gate reading and the oracle's verdict can be compared on a drive KNOWN to collect checkpoints.
    pub tape: Option<String>,
    /// Tape race-ms offset for `--tape`. On the lroundf-clock env (≤ 92d92fd) the replay of p00004 needed +20 ms to
    /// reproduce (oracle 19.558); on the tick-hook env (a8b41b2+) it reproduces at 0 to 0.06 m over the whole lap
    /// (oracle 19.556, env Finished). Default 0; the flag stays as the measuring instrument.
    pub tape_shift_ms: i64,
    /// Disable the env's off-route / no-progress cuts (identity replays: a human line must not be cut by a tight
    /// field-median corridor; the oracle judges the tape). Also the corridor margin used when cuts are on.
    pub no_cut: bool,
    /// A `tmrl refs` file: print the mean tracking term per episode (the piece-2 control of BAR M2-3).
    pub refs: Option<String>,
    pub margin_m: f32,
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

pub struct EpisodeResult {
    pub steps: usize,
    pub ticks: usize,
    pub race_s: f32,
    pub best_s: f32,
    pub gates_env: usize,
    pub done: Option<Done>,
    pub finish_env_s: Option<f64>,
    pub tape: PathBuf,
    pub oracle: String,
    pub oracle_cps: Option<u32>,
    pub oracle_finish_s: Option<f64>,
}

fn secs(ms: i64) -> f64 {
    ms as f64 / 1000.0
}

pub fn run(a: &RolloutArgs) -> Result<Vec<EpisodeResult>, String> {
    let (w, obs_version) = crate::policy::read_any(&a.policy)?;
    println!("policy {}: {:?}", a.policy, w.shape);
    let track = std::sync::Arc::new(tmenv::Track::load_geom_json(Path::new(&a.geom))?);
    println!("geom {}: {} {:.1} m, {} gates", a.geom, track.geom.source, track.geom.length(), track.geom.gates.len());
    std::fs::create_dir_all(&a.work).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let spawn = tmenv::measured_spawn(&a.server, &a.map, &a.shim, &a.work.join("spawnfix"), &a.reference)?;
    println!("measured start {:?} ({:.1}s)", spawn, t0.elapsed().as_secs_f64());
    let mut cfg = CoreCfg { k_ticks: w.shape.k, max_ticks: a.max_ticks, offroute_margin: a.margin_m, obs_version, ..Default::default() };
    if a.no_cut {
        cfg.offroute_ticks = usize::MAX;
        cfg.noprog_ticks = usize::MAX;
    }
    let mut root = tmenv::forkenv::RootCfg { verbose: a.verbose, ..Default::default() };
    root.require_start = Some((spawn, 6.0, 4.0));
    let t1 = Instant::now();
    let (mut env, _rig, tape) = tmenv::forkenv::build_at_start(&a.server, &a.map, &a.shim, &a.work.join("env"), &a.reference, track.clone(), ActionSpace::default(), cfg, &root)?;
    println!("env up in {:.1}s; tape {} ticks, max_ticks {}", t1.elapsed().as_secs_f64(), tape.n(), a.max_ticks);
    let mut rng = Rng(a.seed | 1);
    // Open-loop tape: actions indexed by race tick; the env root sits at race -0.020 s (root row), so env tick i is
    // race 10*i - 20 ms; we read the root race from the first Info.
    let tape_acts: Option<(Vec<Action>, i64)> = match &a.tape {
        None => None,
        Some(p) => {
            let t = gbx::tape::Tape::from_file(p).map_err(|e| format!("{p}: {e}"))?;
            let (s, g, b) = (t.steer_i8s(), t.accels(), t.brakes());
            let acts: Vec<Action> = (0..s.len()).map(|i| Action { steer: s[i], gas: g[i] != 0, brake: b[i] != 0 }).collect();
            let mut hist = std::collections::BTreeMap::new();
            for a in &acts {
                *hist.entry(a.steer).or_insert(0usize) += 1;
            }
            // Index base (INPUT, 2026-09-07): the env's tape index is the forest's write floor (`env.next_tick()`), and
            // the external tape maps onto it with shift = (template.start_offset − tape.start_offset)/10. The engine's
            // race LABEL is lroundf-granular and repeats/skips a record now and then — harmless on a human tape, fatal on
            // a frame-perfect TAS tape (the 31.769 tiny-map control failed at every constant shift until this change).
            let tpl_t0 = gbx::tape::Tape::from_file(a.reference.to_str().unwrap_or("")).map(|tt| tt.race_ms(0)).unwrap_or(0);
            let shift_ticks = (tpl_t0 - t.race_ms(0)) / 10 + a.tape_shift_ms / 10;
            println!("open-loop tape {p}: {} ticks from race {} ms (template starts at {tpl_t0} ms → index shift {shift_ticks} ticks); steer histogram {:?}", acts.len(), t.race_ms(0), hist);
            Some((acts, shift_ticks))
        }
    };
    let reft = a.refs.as_ref().map(|p| crate::refs::RefTrack::read(std::path::Path::new(p))).transpose()?;
    let mut results = Vec::new();
    println!("{:>3} {:>5} {:>6} {:>7} {:>8} {:>5} {:>10} {:>9} | oracle", "ep", "steps", "ticks", "race_s", "best_s", "gates", "done", "fin_env");
    for ep in 0..a.episodes {
        let te = Instant::now();
        let mut env_err: Option<String> = None;
        let mut obs = match env.reset() {
            Ok(o) => o,
            Err(e) => {
                println!("{:>3} env reset failed: {e}", ep);
                continue;
            }
        };
        let mut steps = 0usize;
        let mut done = None;
        let mut last_info = tmenv::core::Info::default();
        // The race clock the NEXT written tick will carry: the engine's own reading, not a tick count (a k-tick step
        // advances ~0.97 k ticks on lroundf granularity, so a counter drifts).
        let mut track_acc = (0f32, 0usize);
        while steps < a.max_steps {
            let raw = w.forward(&obs);
            let acts: Vec<Action> = if let Some((ta, shift)) = &tape_acts {
                let k = w.shape.k;
                let base = env.next_tick().map_err(|e| format!("next_tick: {e}"))? as i64;
                (0..k)
                    .map(|j| {
                        let idx = base + j as i64 + shift;
                        if idx >= 0 && (idx as usize) < ta.len() { ta[idx as usize] } else { Action { steer: 0, gas: false, brake: true } }
                    })
                    .collect()
            } else if a.const_ctrl {
                vec![Action { steer: 0, gas: true, brake: false }; w.shape.k]
            } else if a.temp > 0.0 {
                w.sample(&raw, a.temp, &mut || rng.f32())
            } else {
                w.decode(&raw).0
            };
            let (o, _r, d, info) = match env.step_ticks(&acts) {
                Ok(v) => v,
                Err(e) => {
                    env_err = Some(e);
                    break;
                }
            };
            obs = o;
            steps += 1;
            last_info = info;
            if let Some(r) = &reft {
                if let Some(t) = crate::refs::tracking_term(r, info.s, info.state.pos, info.speed, crate::refs::yaw_of(info.state.quat), (2.0, 8.0, 0.35)) {
                    track_acc.0 += t;
                    track_acc.1 += 1;
                }
            }
            if a.verbose && steps % 20 == 0 {
                println!("    step {steps:4} tick {:5} race {:7.3} s {:8.1} lat {:6.2} gates {} speed {:5.1}", info.tick, info.race_s, info.s, info.lateral, info.gates, info.speed);
            }
            if d.is_some() {
                done = d;
                break;
            }
        }
        let rec = env.rollout_record();
        let path = a.out.join(format!("ep{ep:03}.Ghost.Gbx"));
        {
            // The env's own trajectory reading, per tick, beside the tape: what the policy saw, for diffing against a
            // ghost decode of the same tape (tmtraj export) or the human line.
            let mut csv = String::from("race_ms,x,y,z,vx,vy,vz,qw,qx,qy,qz\n");
            for r in &rec.trace {
                csv.push_str(&format!("{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.6},{:.6},{:.6},{:.6}\n", r.time_ms, r.x, r.y, r.z, r.vx, r.vy, r.vz, r.qw, r.qx, r.qy, r.qz));
            }
            std::fs::write(a.out.join(format!("ep{ep:03}.trace.csv")), csv).map_err(|e| e.to_string())?;
            let mut acsv = String::from("from_tick,k,steer_i8,gas,brake\n");
            for sp in &rec.spans {
                acsv.push_str(&format!("{},{},{},{},{}\n", sp.from, sp.k, sp.act.steer as i8, sp.act.gas, sp.act.brake));
            }
            std::fs::write(a.out.join(format!("ep{ep:03}.spans.csv")), acsv).map_err(|e| e.to_string())?;
        }
        let (s, g, b) = env.banked_tape(&tape);
        tape.write_candidate(&s, &g, &b, &path)?;
        let (oracle, ocps, ofin) = if a.no_oracle {
            ("skipped".to_string(), None, None)
        } else {
            match tmauto::oracle::validate_raw(&a.server, &[path.clone()], tmauto::oracle::Maps::One(&a.map), "tmrl-rollout") {
                Err(e) => (format!("UNREACHABLE: {e}"), None, None),
                Ok(bt) => match bt.answers.first() {
                    None => ("no answer".into(), None, None),
                    Some(ans) => match ans.verdict() {
                        None => (format!("REFUSED ({})", ans.desc.trim()), None, None),
                        Some(tmauto::verdict::Verdict::Dnf { cps }) => match ans.cps {
                            // The server reports a checkpoint count only in some conditions; a bare "wrong simu"
                            // carries NONE (measured: p00004's exact tape braked at 10 s, CP1 passed at 7.6 s, reads
                            // bare "wrong simu"). tmauto collapses that to cps 0; we do not.
                            None => ("DNF (cps unreported)".to_string(), None, None),
                            Some(c) => (format!("DNF cps={c}"), Some(cps), None),
                        },
                        Some(tmauto::verdict::Verdict::Finish { ms }) => (format!("FINISH {:.3}", ms as f64 / 1000.0), ans.cps, Some(ms as f64 / 1000.0)),
                    },
                },
            }
        };
        let r = EpisodeResult {
            steps,
            ticks: last_info.tick,
            race_s: last_info.race_s,
            best_s: env.core.best_s(),
            gates_env: env.core.gates_hit(),
            done,
            finish_env_s: rec.finish_ms.map(secs),
            tape: path,
            oracle,
            oracle_cps: ocps,
            oracle_finish_s: ofin,
        };
        println!(
            "{:>3} {:>5} {:>6} {:>7.3} {:>8.1} {:>5} {:>10} {:>9} | {}   ({:.1}s)",
            ep, r.steps, r.ticks, r.race_s, r.best_s, r.gates_env, format!("{:?}", r.done), r.finish_env_s.map(|f| format!("{f:.3}")).unwrap_or("-".into()), r.oracle, te.elapsed().as_secs_f64()
        );
        if reft.is_some() {
            println!("      tracking term: mean {:.3} over {} chunks", track_acc.0 / track_acc.1.max(1) as f32, track_acc.1);
        }
        if let Some(e) = &env_err {
            println!("      ENV ERROR during this episode (counted as env-error, tape is the prefix): {e}");
        }
        results.push(r);
    }
    // Summary
    let n = results.len().max(1);
    let fin: Vec<f64> = results.iter().filter_map(|r| r.oracle_finish_s).collect();
    let mut dones = std::collections::BTreeMap::new();
    for r in &results {
        *dones.entry(format!("{:?}", r.done)).or_insert(0usize) += 1;
    }
    let cps_hist = {
        let mut h = std::collections::BTreeMap::new();
        for r in &results {
            *h.entry(r.oracle_cps.map(|c| c.to_string()).unwrap_or("-".into())).or_insert(0usize) += 1;
        }
        h
    };
    println!(
        "SUMMARY {} episodes: oracle finishes {}/{} (median {}), oracle cps histogram {:?}, env dones {:?}, best_s median {:.1} m",
        results.len(),
        fin.len(),
        n,
        if fin.is_empty() { "-".to_string() } else { let mut f = fin.clone(); f.sort_by(|a, b| a.partial_cmp(b).unwrap()); format!("{:.3}", f[f.len() / 2]) },
        cps_hist,
        dones,
        { let mut b: Vec<f32> = results.iter().map(|r| r.best_s).collect(); b.sort_by(|a, c| a.partial_cmp(c).unwrap()); b.get(b.len() / 2).copied().unwrap_or(0.0) }
    );
    Ok(results)
}

/// `tmrl tape-into-template --template REF.Ghost.Gbx --tape GHOST.Ghost.Gbx --out OUT.Ghost.Gbx`
///
/// The instrument's POSITIVE CONTROL, offline: a recorded ghost's full 10 ms tape, aligned by race clock, written
/// into the env's template wrapper (the same wrapper every rollout tape gets). If the plain oracle reproduces the
/// ghost's own time on the result, the wrapper + tape path is faithful and any env-vs-oracle disagreement is the
/// env's; if it does not, the wrapper changes the physics and no rollout verdict means what it says.
/// Ticks of the template outside the ghost's tape get steer 0, gas on (countdown) / brake (after the end).
pub fn tape_into_template(template: &str, tape: &str, out: &str, digital_to: Option<i8>, effective: bool) -> Result<(), String> {
    let tpl_tape = gbx::tape::Tape::from_file(template).map_err(|e| format!("{template}: {e}"))?;
    let (n, t0) = (tpl_tape.steer_i8s().len(), tpl_tape.race_ms(0));
    let src = gbx::tape::Tape::from_file(tape).map_err(|e| format!("{tape}: {e}"))?;
    let (ss, sg, sb, s0) = (src.steer_i8s(), src.accels(), src.brakes(), src.race_ms(0));
    let mut s = vec![0u8; n];
    let mut g = vec![0u8; n];
    let mut b = vec![0u8; n];
    // --effective: steer from the telemetry record's effective steer (50 ms, held), not the tape byte: what the
    // physics saw, including whatever the state-word flags did to the press.
    let eff: Option<Vec<(i64, i8)>> = if effective {
        let d = gbx::record::decode_ghost(tape).map_err(|e| format!("{tape}: {e:?}"))?;
        Some(d.samples.iter().map(|sm| (sm.time_ms as i64, (sm.steer * 127.0).round().clamp(-127.0, 127.0) as i8)).collect())
    } else {
        None
    };
    let mut inside = 0usize;
    for i in 0..n {
        let race = t0 + 10 * i as i64;
        let j = (race - s0) / 10;
        if j >= 0 && (j as usize) < ss.len() {
            let j = j as usize;
            s[i] = match digital_to {
                Some(v) if ss[j] == 127 => v as u8,
                Some(v) if ss[j] == -127 => (-v) as u8,
                _ => ss[j] as u8,
            };
            if let Some(e) = &eff {
                // the latest sample at or before this race time
                let shift: i64 = std::env::var("TMRL_EFF_SHIFT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
                let k = e.partition_point(|(t, _)| *t <= race + shift);
                if k > 0 {
                    s[i] = e[k - 1].1 as u8;
                }
            }
            g[i] = sg[j];
            b[i] = sb[j];
            inside += 1;
        } else if race < s0 {
            g[i] = 1;
        } else {
            b[i] = 1;
        }
    }
    let tpl = tmenv::template::Template::load(Path::new(template))?;
    tpl.write_with_inputs(&s, &g, &b, Path::new(out))?;
    println!("template {template}: {n} ticks from race {t0} ms; tape {tape}: {} ticks from race {s0} ms; {inside} ticks carried, wrote {out}", ss.len());
    Ok(())
}

/// `tmrl walltime IN OUT --race-ms N` — set the validation block's walltime END so that (end − start) matches a
/// race of N ms, as `tmauto::synth::GhostMeta::set_declared` does ((ms + 500) / 1000 s). The server requires the
/// walltime span to sit within race_ms ± (10 s + 10 %); `ghost trim --declare` / `ghost declare` lengthen the
/// declared time but leave this pair, so a template lengthened from 23 s to 45 s answers "unexcepted walltime (23s)"
/// (REFUSAL, not DNF). Layout of chunk 0x0309202D read off tmauto's writer: u32 flag, string exe_version, u32
/// checksum, u32 os, u32 cpu, i32 walltime_start, i32 walltime_end, … (this belongs in `ghost declare`; filed).
pub fn set_walltime(input: &str, out: &str, race_ms: u32) -> Result<(), String> {
    let c = gbx::container::Container::load(input)?;
    let mut body = c.body().to_vec();
    let (_, _, payload, size) = c.chunks().into_iter().find(|(id, ..)| *id == 0x0309202D).ok_or("no 0x0309202D validation chunk in this file")?;
    let rd = |o: usize| -> Result<u32, String> {
        body.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| "validation chunk truncated".to_string())
    };
    let len = rd(payload + 4)? as usize;
    let ws_off = payload + 4 + 4 + len + 12;
    if ws_off + 8 > payload + size {
        return Err(format!("walltime offset {ws_off} outside the chunk ({} bytes)", size));
    }
    let start = rd(ws_off)? as i32;
    let end_old = rd(ws_off + 4)? as i32;
    let end_new = start + ((race_ms + 500) / 1000) as i32;
    body[ws_off + 4..ws_off + 8].copy_from_slice(&end_new.to_le_bytes());
    gbx::container::write_gbx(&c.gbx, body, out)?;
    println!("{input}: walltime start {start}, end {end_old} → {end_new} ({} s span for race {:.3} s); wrote {out}", end_new - start, race_ms as f64 / 1000.0);
    Ok(())
}
