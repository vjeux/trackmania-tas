//! `tmreach explore` — G6: a Go-Explore / MAP-Elites seed on top of the fan-out.
//!
//! Archive keyed on the state CELL (pos 2 m × 2 m, speed 5 m/s, yaw 30°, and
//! the checkpoint count); an entry is the cheapest way there we know: a human
//! savestate tick `f` plus a CHAIN of macros (each `h` ticks) from it. Return
//! to a cell = fork the human's node at `f`, replay the chain, then fan the
//! whole macro library out again from there; every end state is a new or
//! better cell. Selection favours under-visited cells with high progress
//! (progress = credited checkpoints, then arclength along the human's own
//! path). THE TARGET is a connection between gates no human in the corpus
//! drove: any rollout that credits a gate other than the human's next one is
//! listed with its tape (human prefix + chain + macro + brake to the end) so
//! the VERIFY arm can try to drive a lap through it.
//!
//! Everything a rollout credits comes from the engine counter (Row::cps);
//! the geometry attributes the gate (see `Detector::credits`).

use crate::fanout::{classify, label_of_tick};
use crate::gates::{Detector, MapGates};
use crate::macros::{build, library_v0, Built, Macro};
use crate::rig::{pos, speed, Worker};
use crate::starts::{run_on_worker, StartsOpts};
use crate::tele::Telemetry;
use forkoracle::forksrv::{rec_of, Rec};
use forkoracle::layout::Row;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellKey {
    pub cx: i32,
    pub cz: i32,
    pub cv: i32,
    pub cyaw: i32,
    pub cps: u8,
}

pub fn cell_of(r: &Row, cps: u8) -> CellKey {
    let v = speed(r);
    // yaw from the velocity when moving, else from the quaternion's forward
    let (fx, fz) = if v > 1.0 { (r.vx, r.vz) } else { let f = crate::gatecal::rotate(r, [0.0, 0.0, 1.0]); (f[0], f[2]) };
    let yaw = fx.atan2(fz).to_degrees();
    CellKey { cx: (r.x / 2.0).floor() as i32, cz: (r.z / 2.0).floor() as i32, cv: (v / 5.0).floor() as i32, cyaw: ((yaw + 180.0) / 30.0).floor() as i32 % 12, cps }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub key: CellKey,
    /// human savestate tick the chain starts from
    pub f: usize,
    /// (macro id, horizon ticks) applied in order from `f`
    pub chain: Vec<(u16, usize)>,
    /// checkpoints credited at the end (engine), and the progress index along the human path
    pub cps: u8,
    pub progress: f64,
    pub visits: u32,
    pub end: Row,
    pub gates_seen: Vec<u32>,
}

pub struct ExploreCfg {
    pub gates: MapGates,
    pub det: Detector,
    pub macros: Vec<Macro>,
    pub h: usize,
    pub every_ms: i64,
    pub budget: usize,
    pub seed: u64,
    pub out: std::path::PathBuf,
}

pub struct ExploreOut {
    pub cells: usize,
    pub rollouts: usize,
    pub steps: usize,
    pub connections: Vec<String>,
    pub max_cps: u8,
    pub log: Vec<String>,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Progress along the human's flat path: the index of the nearest human row
/// (monotone in race time), as metres travelled by the human to that row.
fn progress_along(flat: &[Row], cum: &[f64], p: [f64; 3]) -> f64 {
    let mut best = (f64::INFINITY, 0usize);
    for (i, r) in flat.iter().enumerate().step_by(3) {
        let d = crate::rig::dist(pos(r), p);
        if d < best.0 {
            best = (d, i);
        }
    }
    // a far-off point earns nothing beyond the nearest human point, discounted by its distance
    cum[best.1] - best.0.min(50.0)
}

pub fn explore_ghost(w: &mut Worker, tel: &Telemetry, cfg: &ExploreCfg) -> Result<ExploreOut, String> {
    let gates = &cfg.gates;
    let det = &cfg.det;
    let o = StartsOpts { every_ms: 500, out: None, trace_out: None, verbose: false };
    let rep = run_on_worker(w, tel, gates, &o)?;
    if !(rep.start_ctrl_pass && rep.identity.passes()) {
        return Err("startup controls FAILED".into());
    }
    let flat = rep.flat;
    let mut cum = vec![0.0; flat.len()];
    for i in 1..flat.len() {
        cum[i] = cum[i - 1] + crate::rig::dist(pos(&flat[i - 1]), pos(&flat[i]));
    }
    let n = w.n_ticks();
    let off = w.tape.start_offset_ms as i64;
    let ng = gates.gates.len();
    let human_first = det.credits(gates, &flat, &vec![false; ng], 5).gate_row;
    let human_order: Vec<usize> = {
        let mut v: Vec<(i32, usize)> = human_first.iter().enumerate().filter(|(_, t)| **t >= 0).map(|(gi, t)| (*t, gi)).collect();
        v.sort();
        v.iter().map(|(_, gi)| *gi).collect()
    };
    let name = w.ghost.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut rng = Rng(cfg.seed ^ 0x9E3779B97F4A7C15 ^ (name.len() as u64));
    let mut archive: std::collections::HashMap<CellKey, Entry> = Default::default();
    let mut out = ExploreOut { cells: 0, rollouts: 0, steps: 0, connections: Vec::new(), max_cps: 0, log: Vec::new() };
    let h = cfg.h;
    let flat_at = |label: i64| flat.iter().find(|r| r.time_ms == label);
    // credited gates before tick f (human prefix), from the engine counter on the flat run
    let label0 = label_of_tick(w, 0);
    let prefix_of = |f: usize| -> (Vec<bool>, u8) {
        let label = (f as i64 - 1) * 10 + off;
        let cps = flat_at(label).map(|r| if r.cps == u32::MAX { 0 } else { r.cps as u8 }).unwrap_or(0);
        // human_first are row indices into flat; flat rows are 10 ms apart from label0? no: from the first flat row
        let first_label = flat.first().map(|r| r.time_ms).unwrap_or(label0);
        let _ = first_label;
        let credited: Vec<bool> = (0..ng).map(|gi| human_first[gi] >= 0 && flat[human_first[gi] as usize].time_ms <= label).collect();
        (credited, cps)
    };
    // one exploration step: from node `node` (state after tick `from`-1, i.e. floor = from), fan out all macros
    // returns the number of rollouts
    let mut fan = |w: &mut Worker, node: branch::Handle, from: usize, base_entry: Option<&Entry>, f: usize, chain: &[(u16, usize)], archive: &mut std::collections::HashMap<CellKey, Entry>, out: &mut ExploreOut| -> Result<usize, String> {
        if from + h > n {
            return Ok(0);
        }
        let (credited, cps_before) = prefix_of(f);
        // the credits along the chain so far are in the base entry
        let mut credited = credited;
        if let Some(e) = base_entry {
            for g in &e.gates_seen {
                if let Some(gi) = gates.gates.iter().position(|x| x.waypoint == *g) {
                    credited[gi] = true;
                }
            }
        }
        let cps0 = base_entry.map(|e| e.cps).unwrap_or(cps_before);
        let base: Vec<(u8, u8, u8)> = (from..from + h).map(|t| (w.tape.steer[t.min(n - 1)], w.tape.accel[t.min(n - 1)], w.tape.brake[t.min(n - 1)])).collect();
        let mut count = 0;
        for m in &cfg.macros {
            let recs = match build(m, &base, false) {
                Built::Recs(r) => r,
                Built::NoOp => continue,
            };
            let rolled = match w.rollout(node, &recs, from, h as u64) {
                Ok(r) => r,
                Err(e) => {
                    out.log.push(format!("  rollout failed at f {f} chain {:?} macro {}: {e}", chain, m.id));
                    continue;
                }
            };
            count += 1;
            if rolled.rows.is_empty() {
                continue;
            }
            let cr = det.credits(gates, &rolled.rows, &credited, 5);
            let mut seen: Vec<u32> = base_entry.map(|e| e.gates_seen.clone()).unwrap_or_default();
            let mut new_gates = Vec::new();
            for (gi, t) in cr.gate_row.iter().enumerate() {
                if *t >= 0 {
                    seen.push(gates.gates[gi].waypoint);
                    new_gates.push(gi);
                }
            }
            let end = rolled.rows.last().unwrap().clone();
            let cps = if end.cps != u32::MAX { end.cps as u8 } else { cps0 + new_gates.len() as u8 };
            // THE TARGET: a gate credited that is not the human's next gate after the ones already credited
            for gi in &new_gates {
                let already: Vec<usize> = (0..ng).filter(|i| credited[*i]).collect();
                let human_next = human_order.iter().find(|g| !already.contains(g)).copied();
                if Some(*gi) != human_next {
                    let mut full_chain = chain.to_vec();
                    full_chain.push((m.id, h));
                    // write the tape: human prefix to f, then the chain, then brake to the end
                    let (mut st, mut gs, mut br) = (w.tape.steer.clone(), w.tape.accel.clone(), w.tape.brake.clone());
                    let mut t = f;
                    for (mid, hh) in &full_chain {
                        let mm = cfg.macros.iter().find(|x| x.id == *mid).unwrap();
                        let b: Vec<(u8, u8, u8)> = (t..t + hh).map(|k| (w.tape.steer[k.min(n - 1)], w.tape.accel[k.min(n - 1)], w.tape.brake[k.min(n - 1)])).collect();
                        if let Built::Recs(r) = build(mm, &b, false) {
                            for (i, rr) in r.iter().enumerate() {
                                if t + i < n {
                                    st[t + i] = (rr.steer * 127.0).round() as i8 as u8;
                                    gs[t + i] = (rr.gas > 0.5) as u8;
                                    br[t + i] = (rr.brake > 0.5) as u8;
                                }
                            }
                        }
                        t += hh;
                    }
                    for k in t..n {
                        st[k] = 0;
                        gs[k] = 0;
                        br[k] = 1;
                    }
                    let tape = cfg.out.join("connections").join(format!("{}-f{}-{}.Ghost.Gbx", name.trim_end_matches(".Ghost.Gbx"), f, full_chain.iter().map(|(a, b)| format!("m{a}x{b}")).collect::<Vec<_>>().join("_")));
                    let _ = std::fs::create_dir_all(tape.parent().unwrap());
                    let _ = w.tape.write_candidate(&st, &gs, &br, &tape);
                    let (s, lat, up) = gates.gates[*gi].local(pos(&rolled.rows[cr.gate_row[*gi] as usize]));
                    out.connections.push(format!(
                        "{}\t{}\t{}\t{}\twp{}\t{}\t{:?}\t{}\t{:+.2}\t{:+.2}\t{:+.2}\t{:.1}\t{}\t{}",
                        name, f, crate::secs(w.race_of(&rolled.rows[0])), already.iter().map(|i| format!("wp{}", gates.gates[*i].waypoint)).collect::<Vec<_>>().join(","), gates.gates[*gi].waypoint,
                        human_next.map(|g| format!("wp{}", gates.gates[g].waypoint)).unwrap_or("-".into()), full_chain, crate::secs(w.race_of(&rolled.rows[cr.gate_row[*gi] as usize])), s, lat, up, speed(&rolled.rows[cr.gate_row[*gi] as usize]), tape.display(),
                        if w.race_of(&rolled.rows[cr.gate_row[*gi] as usize]) > w.race_of_tick_end() { "AFTER-TAPE (batch-dependent oracle)" } else { "in-tape" }
                    ));
                }
            }
            let (outcome, _, _, _) = classify(&rolled.rows, speed(&rolled.rows[0]), -1.0e9, rolled.exited, false);
            if outcome == crate::tmr::OUTCOME_OFFWORLD || outcome == crate::tmr::OUTCOME_ABORTED {
                continue;
            }
            let key = cell_of(&end, cps);
            let progress = cps as f64 * 10_000.0 + progress_along(&flat, &cum, pos(&end));
            let mut full_chain = chain.to_vec();
            full_chain.push((m.id, h));
            let e = archive.entry(key.clone());
            match e {
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(Entry { key, f, chain: full_chain, cps, progress, visits: 0, end, gates_seen: seen });
                }
                std::collections::hash_map::Entry::Occupied(mut oc) => {
                    let cur = oc.get_mut();
                    // cheaper chain (fewer macros) or more progress replaces
                    if full_chain.len() < cur.chain.len() || (full_chain.len() == cur.chain.len() && progress > cur.progress) {
                        cur.f = f;
                        cur.chain = full_chain;
                        cur.progress = progress;
                        cur.end = end;
                        cur.gates_seen = seen;
                    }
                }
            }
            out.max_cps = out.max_cps.max(cps);
        }
        Ok(count)
    };

    // SEED: every human savestate, all macros once (h ticks)
    let mut next_ms = 0i64;
    let mut node: Option<branch::Handle> = None;
    let mut cursor = w.root_probe;
    while next_ms + 10 * h as i64 <= label_of_tick(w, n) {
        let k = ((next_ms - off) / 10).max(0) as usize;
        next_ms += cfg.every_ms;
        if k + 1 <= cursor {
            continue;
        }
        let want = k + 1 - cursor;
        let from = node.unwrap_or(branch::ROOT);
        let recs = w.reference_recs(cursor, want);
        let (_, nn) = w.rollout_keep(from, &recs, cursor, want as u64)?;
        if let Some(old) = node {
            w.release(old);
        }
        node = Some(nn);
        cursor = w.floor(nn)?;
        let f = cursor;
        out.rollouts += fan(w, nn, f, None, f, &[], &mut archive, &mut out)?;
        if out.rollouts >= cfg.budget {
            break;
        }
    }
    if let Some(hh) = node {
        w.release(hh);
    }
    out.log.push(format!("{name}: seed {} rollouts -> {} cells, max cps {}", out.rollouts, archive.len(), out.max_cps));

    // EXPLORE: pick a cell, return, fan out
    while out.rollouts < cfg.budget {
        if archive.is_empty() {
            break;
        }
        // selection weight: (1 + progress rank fraction)^2 / sqrt(1 + visits); chains capped at 6 macros
        let mut keys: Vec<(CellKey, f64)> = archive.values().filter(|e| e.chain.len() < 6).map(|e| (e.key.clone(), e.progress)).collect();
        if keys.is_empty() {
            break;
        }
        keys.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let m = keys.len();
        let weights: Vec<f64> = keys.iter().enumerate().map(|(i, (k, _))| {
            let rank = (i + 1) as f64 / m as f64;
            let v = archive[k].visits as f64;
            (0.2 + rank).powi(3) / (1.0 + v).sqrt()
        }).collect();
        let total: f64 = weights.iter().sum();
        let mut pick = rng.unit() * total;
        let mut chosen = keys.len() - 1;
        for (i, wgt) in weights.iter().enumerate() {
            if pick < *wgt {
                chosen = i;
                break;
            }
            pick -= wgt;
        }
        let key = keys[chosen].0.clone();
        let entry = archive.get(&key).unwrap().clone();
        archive.get_mut(&key).unwrap().visits += 1;
        // return: node at f from the root, then the chain
        let f = entry.f;
        let recs = w.reference_recs(w.root_probe, f - w.root_probe);
        let (_, nf) = match w.rollout_keep(branch::ROOT, &recs, w.root_probe, (f - w.root_probe) as u64) {
            Ok(x) => x,
            Err(e) => {
                out.log.push(format!("  return to f {f} failed: {e}"));
                continue;
            }
        };
        let mut t = f;
        let mut chain_recs: Vec<Rec> = Vec::new();
        for (mid, hh) in &entry.chain {
            let mm = cfg.macros.iter().find(|x| x.id == *mid).unwrap();
            let b: Vec<(u8, u8, u8)> = (t..t + hh).map(|k| (w.tape.steer[k.min(n - 1)], w.tape.accel[k.min(n - 1)], w.tape.brake[k.min(n - 1)])).collect();
            match build(mm, &b, false) {
                Built::Recs(r) => chain_recs.extend(r),
                Built::NoOp => chain_recs.extend(b.iter().map(|&(s, g, br)| rec_of(s, g, br))),
            }
            t += hh;
        }
        let nc = match w.rollout_keep(nf, &chain_recs, f, chain_recs.len() as u64) {
            Ok((rows, nc)) => {
                // the return must land where the archive says (identity of the replay)
                if let (Some(a), b) = (rows.last(), &entry.end) {
                    let d = crate::rig::dist(pos(a), pos(b));
                    if d > 0.05 {
                        out.log.push(format!("  RETURN MISMATCH at f {f} chain {:?}: replay end {:.3} m off the archived end", entry.chain, d));
                    }
                }
                nc
            }
            Err(e) => {
                out.log.push(format!("  chain replay failed at f {f}: {e}"));
                w.release(nf);
                continue;
            }
        };
        w.release(nf);
        let from = w.floor(nc)?;
        out.rollouts += fan(w, nc, from, Some(&entry), f, &entry.chain, &mut archive, &mut out)?;
        w.release(nc);
        out.steps += 1;
    }
    out.cells = archive.len();
    // archive dump
    let mut s = String::from("ghost\tcx\tcz\tcv\tcyaw\tcps\tvisits\tprogress\tf\tchain\tend_x\tend_y\tend_z\tend_speed\tgates_seen\n");
    let mut entries: Vec<&Entry> = archive.values().collect();
    entries.sort_by(|a, b| b.progress.partial_cmp(&a.progress).unwrap());
    for e in entries {
        s.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.1}\t{}\n", name, e.key.cx, e.key.cz, e.key.cv, e.key.cyaw, e.key.cps, e.visits, e.progress, e.f, e.chain.iter().map(|(a, b)| format!("m{a}x{b}")).collect::<Vec<_>>().join(","), e.end.x, e.end.y, e.end.z, speed(&e.end), e.gates_seen.iter().map(|g| format!("wp{g}")).collect::<Vec<_>>().join(",")));
    }
    std::fs::create_dir_all(&cfg.out).map_err(|e| e.to_string())?;
    std::fs::write(cfg.out.join(format!("archive-{}.tsv", name.trim_end_matches(".Ghost.Gbx"))), s).map_err(|e| e.to_string())?;
    out.log.push(format!("{name}: {} rollouts, {} explore steps, {} cells, max cps {}, {} other-gate connections", out.rollouts, out.steps, out.cells, out.max_cps, out.connections.len()));
    Ok(out)
}

pub fn connections_header() -> &'static str {
    "ghost\tf\tstart_race\tcredited_before\tgate\thuman_next\tchain\tcross_race\ts\tlat\tup\tspeed\ttape\twindow\n"
}

pub fn default_macros() -> Vec<Macro> {
    library_v0()
}
