//! `tmreach fanout` — every macro from every savestate of a human run (G3),
//! and the per-start identity control (macro 0 reproduces the human's own
//! trajectory).

use crate::gates::{Detector, GateKind, MapGates};
use crate::macros::{build, Built, Macro};
use crate::rig::{dist, pos, speed, Worker};
use crate::starts::{run_on_worker, StartsOpts};
use crate::tele::Telemetry;
use crate::tmr::{CarState, FromRow, Record, OUTCOME_ABORTED, OUTCOME_CRASH_STOP, OUTCOME_FINISHED, OUTCOME_OFFWORLD, OUTCOME_OK};
use branch::Handle;
use forkoracle::layout::Row;
use std::sync::Arc;

pub struct FanoutCfg {
    pub every_ms: i64,
    pub horizons: Vec<u16>,
    pub lib: Arc<Vec<Macro>>,
    pub det: Arc<Detector>,
    pub gates: Arc<MapGates>,
    /// World y below which the car is off the map.
    pub floor_y: f64,
    /// Keep the per-tick rows of every rollout (for endpoints/plots). Costly.
    pub keep_rows: bool,
}

/// One savestate, as the fan-out actually used it.
#[derive(Clone, Debug)]
pub struct StartRow {
    pub start_id: u32,
    pub ghost_md5: String,
    /// Tape tick of the first macro record.
    pub tick: usize,
    pub state: CarState,
    pub cps_before: u8,
}

#[derive(Default, Debug, Clone)]
pub struct Stats {
    pub rollouts: usize,
    pub noop: usize,
    pub out_of_tape: usize,
    pub errors: usize,
    pub identity_fail: usize,
    pub identity_max_m: f64,
    /// How far a macro's first record moved the start row (engine input blending).
    pub start_blend_max_m: f64,
    pub outcomes: [usize; 5],
    /// Rollouts that crossed the human's NEXT gate.
    pub reached_next: usize,
    /// Rollouts that crossed some OTHER uncredited gate first.
    pub reached_other: usize,
    /// Rollouts whose rows carried no engine counter (geometry-only credits).
    pub no_counter: usize,
    /// Engine counter steps no gate trigger explained.
    pub unattributed: usize,
    /// Geometric crossings the engine did not credit.
    pub geometric_only: usize,
    pub distinct_cells: Vec<usize>,
    pub switches: usize,
    pub rollout_secs: f64,
}

pub struct GhostFanout {
    pub starts: Vec<StartRow>,
    pub records: Vec<Record>,
    /// Parallel to `records`: the 4 sampled path points (TMP4 sidecar).
    pub paths: Vec<[crate::tmr::PathPoint; crate::tmr::TMP4_POINTS]>,
    pub stats: Stats,
    /// (start index, macro, horizon, end position) for endpoints.tsv
    pub endpoints: Vec<(u32, u16, u16, [f32; 3])>,
    pub other_connections: Vec<String>,
    pub log: Vec<String>,
    pub human_legs: usize,
    pub human_respawns: usize,
}

/// Engine label of the state after record `k`.
pub fn label_of_tick(w: &Worker, k: usize) -> i64 {
    k as i64 * 10 + w.tape.start_offset_ms as i64
}

/// Which gates the human had crossed (detector) before tape tick `k`.
pub fn credited_before(det: &Detector, gates: &MapGates, flat: &[Row], w: &Worker, k: usize) -> Vec<bool> {
    let lim = label_of_tick(w, k);
    let rows: Vec<Row> = flat.iter().filter(|r| r.time_ms < lim).cloned().collect();
    let first = det.credits(gates, &rows, &vec![false; gates.gates.len()], 5).gate_row;
    first.iter().map(|t| *t >= 0).collect()
}

pub fn classify(rows: &[Row], start_speed: f64, floor_y: f64, exited: bool, finished_gate: bool) -> (u8, f32, f32, f32) {
    let mut path = 0.0;
    let mut vmin = f64::INFINITY;
    let mut vmax = 0.0f64;
    let mut off = false;
    for (i, r) in rows.iter().enumerate() {
        let v = speed(r);
        vmin = vmin.min(v);
        vmax = vmax.max(v);
        if i > 0 {
            path += dist(pos(r), pos(&rows[i - 1]));
        }
        if r.y < floor_y || !(0.0..=2048.0).contains(&r.x) || !(0.0..=2048.0).contains(&r.z) {
            off = true;
        }
    }
    let end_v = rows.last().map(speed).unwrap_or(0.0);
    let outcome = if finished_gate || exited {
        OUTCOME_FINISHED
    } else if off {
        OUTCOME_OFFWORLD
    } else if rows.is_empty() {
        OUTCOME_ABORTED
    } else if start_speed > 8.0 && end_v < 3.0 {
        OUTCOME_CRASH_STOP
    } else {
        OUTCOME_OK
    };
    (outcome, path as f32, vmin as f32, vmax as f32)
}

/// Fan out one ghost. `start_id_base` numbers this ghost's starts globally.
/// `shard = (k, n)`: only every n-th nominal start (index ≡ k mod n) is fanned
/// out by this call, so one ghost can be split over n workers; the human legs
/// are emitted by shard 0 only. Start ids: `start_id_base + grid index`.
pub fn fanout_ghost(w: &mut Worker, tel: &Telemetry, cfg: &FanoutCfg, start_id_base: u32, shard: (usize, usize)) -> Result<GhostFanout, String> {
    let o = StartsOpts { every_ms: cfg.every_ms, out: None, trace_out: None, verbose: false };
    let rep = run_on_worker(w, tel, &cfg.gates, &o)?;
    if !(rep.start_ctrl_pass && rep.identity.passes()) {
        return Err(format!("startup controls FAILED on {}: {}", w.ghost.display(), rep.identity));
    }
    let flat = rep.flat;
    let n = w.n_ticks();
    let off = w.tape.start_offset_ms as i64;
    let gates = &cfg.gates;
    let det = &cfg.det;
    let ng = gates.gates.len();
    let human_order: Vec<u32> = {
        let first = det.credits(gates, &flat, &vec![false; ng], 5).gate_row;
        let mut v: Vec<(i32, u32)> = first.iter().enumerate().filter(|(_, t)| **t >= 0).map(|(i, t)| (*t, gates.gates[i].waypoint)).collect();
        v.sort();
        v.into_iter().map(|(_, wp)| wp).collect()
    };
    let mut out = GhostFanout { starts: Vec::new(), records: Vec::new(), stats: Stats::default(), endpoints: Vec::new(), other_connections: Vec::new(), log: Vec::new(), human_legs: 0, human_respawns: 0, paths: Vec::new() };
    out.log.push(format!("{}: human gate order by detector {:?}", w.ghost.display(), human_order));
    let max_h = *cfg.horizons.iter().max().unwrap_or(&300) as usize;

    // The savestate chain: one live node walked along the reference.
    let mut node: Option<Handle> = None;
    let mut cursor = w.root_probe; // records consumed at the current node (its probe)
    let mut next_ms = 0i64;
    let mut grid_index: u32 = 0;
    let t_all = std::time::Instant::now();
    while next_ms + 10 * max_h as i64 <= label_of_tick(w, n) {
        // the nominal start: state after record k, label next_ms
        let k = ((next_ms - off) / 10).max(0) as usize;
        next_ms += cfg.every_ms;
        let this_index = grid_index;
        grid_index += 1;
        if (this_index as usize) % shard.1 != shard.0 {
            continue;
        }
        if k + 1 <= cursor {
            continue; // the chain is already past it
        }
        let want = k + 1 - cursor; // ticks to advance so the probe lands ~k+1
        let from = match node {
            None => 0,
            Some(h) => w.floor(h)?,
        };
        let h_from = node.unwrap_or(branch::ROOT);
        let (mut rows_walk, new_node) = match w.rollout_keep(h_from, &[], from, want as u64) {
            Ok(x) => x,
            Err(e) => {
                out.log.push(format!("  walk to tick {k} failed: {e}"));
                out.stats.errors += 1;
                break;
            }
        };
        crate::rig::drop_stale_tail(&mut rows_walk);
        if let Some(h) = node {
            w.release(h);
        }
        node = Some(new_node);
        let f = w.floor(new_node)?; // first writable record
        cursor = f - 1;
        if f + max_h > n {
            out.stats.out_of_tape += 1;
            break;
        }
        let start_label = label_of_tick(w, f - 1);
        let credited = credited_before(det, gates, &flat, w, f);
        let cps_before = credited.iter().filter(|c| **c).count() as u8;
        // the human's next gate from here
        let next_gate: Option<usize> = human_order.iter().find(|wp| !credited[gates.gates.iter().position(|g| g.waypoint == **wp).unwrap()]).map(|wp| gates.gates.iter().position(|g| g.waypoint == *wp).unwrap());
        let start_id = start_id_base + this_index;
        let mut start_state: Option<CarState> = None;
        let mut cells: std::collections::HashSet<(i64, i64, i64)> = Default::default();
        let flat_at = |label: i64| flat.iter().find(|r| r.time_ms == label);
        let airborne = flat_at(start_label).map(|r| r.vy.abs() > 2.0).unwrap_or(false);

        for &h in &cfg.horizons {
            let h = h as usize;
            let base: Vec<(u8, u8, u8)> = (f..f + h).map(|t| (w.tape.steer[t], w.tape.accel[t], w.tape.brake[t])).collect();
            for m in cfg.lib.iter() {
                let recs = match build(m, &base, airborne) {
                    Built::Recs(r) => r,
                    Built::NoOp => {
                        out.stats.noop += 1;
                        continue;
                    }
                };
                let t0 = std::time::Instant::now();
                let ask = (h as f64 * 1.08) as u64 + 12;
                let rolled = match w.rollout(new_node, &recs, f, ask) {
                    Ok(r) => r,
                    Err(e) => {
                        out.log.push(format!("  start {start_id} macro {} h {h}: {e}", m.id));
                        out.stats.errors += 1;
                        continue;
                    }
                };
                out.stats.rollout_secs += t0.elapsed().as_secs_f64();
                out.stats.rollouts += 1;
                let end_label = label_of_tick(w, f + h - 1);
                let first_label = label_of_tick(w, f);
                // The START state is the human's own state at the last
                // reference-driven tick (the flat trace; macro 0 reproduces it to
                // 0.0000 m). The rollout's own row at that label is NOT used:
                // the engine blends the next record into the current tick, so a
                // macro's first record moves that row by 2-13 mm (measured on
                // 2960 rollouts). A boundary violation would be metres.
                if start_state.is_none() {
                    start_state = flat_at(start_label).map(|r| CarState::from_row(r, w.race_of(r), cps_before, false));
                }
                if let Some(r) = rolled.rows.iter().find(|r| r.time_ms == start_label) {
                    if let Some(s0) = &start_state {
                        let d = dist([s0.pos[0] as f64, s0.pos[1] as f64, s0.pos[2] as f64], pos(r));
                        out.stats.start_blend_max_m = out.stats.start_blend_max_m.max(d);
                        if d > 0.5 {
                            out.log.push(format!("  start {start_id}: macro {} moved the START row by {d:.3} m -- a write below the boundary", m.id));
                            out.stats.errors += 1;
                        }
                    }
                }
                let win: Vec<Row> = rolled.rows.iter().filter(|r| r.time_ms >= first_label && r.time_ms <= end_label).cloned().collect();
                let complete = win.last().map(|r| r.time_ms == end_label).unwrap_or(false);
                // credits: the ENGINE counter says how many and when, the geometry says which
                let cr = det.credits(gates, &win, &credited, 5);
                let first = cr.gate_row.clone();
                if !cr.engine {
                    out.stats.no_counter += 1;
                }
                out.stats.unattributed += cr.unattributed.len();
                out.stats.geometric_only += cr.geometric_only.len();
                if !cr.unattributed.is_empty() {
                    out.log.push(format!("  start {start_id} macro {} h {h}: {} counter step(s) no gate explains at rows {:?}", m.id, cr.unattributed.len(), cr.unattributed));
                }

                let mut gate_tick = [-1i16; 32];
                let mut finished_gate = false;
                let mut n_new = 0;
                let mut first_new: Option<(i32, usize)> = None;
                for (gi, t) in first.iter().enumerate() {
                    if *t >= 0 {
                        let wp = gates.gates[gi].waypoint as usize;
                        if wp < 32 {
                            gate_tick[wp] = *t as i16;
                        }
                        n_new += 1;
                        if gates.gates[gi].kind == GateKind::Finish {
                            finished_gate = true;
                        }
                        if first_new.map(|(ft, _)| *t < ft).unwrap_or(true) {
                            first_new = Some((*t, gi));
                        }
                    }
                }
                if let Some((_, gi)) = first_new {
                    if Some(gi) == next_gate {
                        out.stats.reached_next += 1;
                    } else {
                        out.stats.reached_other += 1;
                        out.other_connections.push(format!(
                            "{}\t{}\t{}\t{}\twp{}\t{}\tnext_was_wp{}",
                            w.ghost.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                            start_id,
                            m.id,
                            h,
                            gates.gates[gi].waypoint,
                            m.description,
                            next_gate.map(|g| gates.gates[g].waypoint.to_string()).unwrap_or("-".into())
                        ));
                    }
                }
                let start_speed = start_state.map(|s| s.speed as f64).unwrap_or(0.0);
                let (mut outcome, path, vmin, vmax) = classify(&win, start_speed, cfg.floor_y, rolled.exited && !complete, finished_gate);
                if !complete && !rolled.exited {
                    outcome = OUTCOME_ABORTED;
                }
                out.stats.outcomes[outcome as usize] += 1;
                // a FINISHED rollout ends at its finish-crossing row (the rows past it
                // are however many the exiting child flushed: 0..3, run to run)
                let end_row = if finished_gate {
                    first.iter().enumerate().find(|(gi, t)| **t >= 0 && gates.gates[*gi].kind == GateKind::Finish).and_then(|(_, t)| win.get(*t as usize).cloned())
                } else {
                    win.last().cloned()
                };
                let Some(end_row) = end_row else {
                    out.stats.errors += 1;
                    out.log.push(format!("  start {start_id} macro {} h {h}: no rows in the window", m.id));
                    continue;
                };
                // identity control: macro 0 must reproduce the human's own trajectory
                if m.id == 0 {
                    if let Some(hr) = flat_at(end_row.time_ms) {
                        let d = dist(pos(hr), pos(&end_row));
                        out.stats.identity_max_m = out.stats.identity_max_m.max(d);
                        if d > 0.05 {
                            out.stats.identity_fail += 1;
                            out.log.push(format!("  start {start_id} h {h}: IDENTITY macro 0 end state {d:.4} m off the human's trajectory at {}", crate::secs(w.race_of(&end_row))));
                        }
                    }
                }
                let end = CarState::from_row(&end_row, w.race_of(&end_row), cps_before + n_new as u8, finished_gate || (rolled.exited && !complete));
                cells.insert(((end_row.x / 2.0).floor() as i64, (end_row.z / 2.0).floor() as i64, (speed(&end_row) / 5.0).floor() as i64));
                out.endpoints.push((start_id, m.id, h as u16, end.pos));
                out.records.push(Record { start_id, macro_id: m.id, horizon_ticks: h as u16, outcome, end, gate_tick, path_len_m: path, min_speed: vmin, max_speed: vmax });
                out.paths.push(crate::tmr::path4(&win, &|r| w.race_of(r)));
            }
        }
        let Some(st) = start_state else {
            out.log.push(format!("  start {start_id}: no rollout produced the start row; skipped"));
            continue;
        };
        out.starts.push(StartRow { start_id, ghost_md5: tel.md5.clone(), tick: f, state: st, cps_before });
        out.stats.distinct_cells.push(cells.len());
        if cells.len() <= 2 {
            out.stats.switches += 1;
            out.log.push(format!("  start {start_id} (tick {f}, {:.1} m/s): only {} distinct end cells over {} macros -- a switch, not a fan", st.speed, cells.len(), cfg.lib.len()));
        }
    }
    if let Some(h) = node {
        w.release(h);
    }
    // G4: the human's own legs (positives) and respawn negatives, ids after the
    // fan-out's starts.
    let hb = start_id_base + 500;
    if shard.0 == 0 {
    match crate::human::human_from_flat(w, tel, flat.clone(), gates, det, hb) {
        Ok(h) => {
            out.human_legs = h.legs;
            out.human_respawns = h.respawns;
            out.starts.extend(h.starts);
            out.records.extend(h.records);
            out.paths.extend(h.paths);
            out.log.extend(h.log);
        }
        Err(e) => out.log.push(format!("  human legs FAILED: {e}")),
    }
    }
    out.log.push(format!(
        "{}: {} starts, {} rollouts in {:.1} s wall ({:.1}/s), outcomes ok/crash/off/fin/abort {:?}, identity max {:.4} m ({} fails), start-row blend max {:.4} m, noop {}, errors {}; engine counter: {} rollouts without, {} unattributed steps, {} geometric-only crossings",
        w.ghost.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        out.starts.len(),
        out.stats.rollouts,
        t_all.elapsed().as_secs_f64(),
        out.stats.rollouts as f64 / t_all.elapsed().as_secs_f64().max(1e-9),
        out.stats.outcomes,
        out.stats.identity_max_m,
        out.stats.identity_fail,
        out.stats.start_blend_max_m,
        out.stats.noop,
        out.stats.errors,
        out.stats.no_counter,
        out.stats.unattributed,
        out.stats.geometric_only
    ));
    Ok(out)
}
