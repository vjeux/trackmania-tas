//! `tmreach human` — G4: every human run as positive connection samples (one
//! record per leg: start = the state at the previous gate crossing, end = the
//! state at the next), and every respawn as a negative (start = the last gate
//! crossing, end = the state at the respawn press, no gate crossed).
//!
//! Records carry `macro_id = HUMAN_MACRO` and `horizon_ticks` = the leg's
//! length in ticks; their starts are appended to starts.tsv with
//! `source = human`.

use crate::fanout::classify;
use crate::gates::{Detector, GateKind, MapGates};
use crate::rig::{speed, Worker};
use crate::starts::{run_on_worker, StartsOpts};
use crate::tele::Telemetry;
use crate::tmr::{CarState, FromRow, Record, OUTCOME_CRASH_STOP, OUTCOME_FINISHED, OUTCOME_OK};
use forkoracle::layout::Row;

pub const HUMAN_MACRO: u16 = 65535;
pub const RESPAWN_MACRO: u16 = 65534;

pub struct HumanOut {
    pub starts: Vec<crate::fanout::StartRow>,
    pub records: Vec<Record>,
    pub paths: Vec<[crate::tmr::PathPoint; crate::tmr::TMP4_POINTS]>,
    pub legs: usize,
    pub respawns: usize,
    pub log: Vec<String>,
}

/// Ticks (tape indices) at which the respawn input is pressed.
pub fn respawn_ticks(ghost: &std::path::Path) -> Vec<usize> {
    match gbx::tape::Tape::from_file(&ghost.to_string_lossy()) {
        Ok(t) => match t.archives.first() {
            Some(a) => a.packets.iter().enumerate().filter(|(_, p)| p.respawn()).map(|(i, _)| i).collect(),
            None => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

/// Standalone: controls + flat run, then the legs.
pub fn human_ghost(w: &mut Worker, tel: &Telemetry, gates: &MapGates, det: &Detector, start_id_base: u32) -> Result<HumanOut, String> {
    let o = StartsOpts { every_ms: 500, out: None, trace_out: None, verbose: false };
    let rep = run_on_worker(w, tel, gates, &o)?;
    if !(rep.start_ctrl_pass && rep.identity.passes()) {
        return Err("startup controls FAILED".into());
    }
    human_from_flat(w, tel, rep.flat, gates, det, start_id_base)
}

/// The legs of an already-run flat trajectory (the fan-out calls this).
pub fn human_from_flat(w: &Worker, tel: &Telemetry, mut flat: Vec<Row>, gates: &MapGates, det: &Detector, start_id_base: u32) -> Result<HumanOut, String> {
    // the flat run exits at the finish; the crossing row may be missing
    crate::rig::extrapolate_exit(&mut flat);
    let ng = gates.gates.len();
    let mut first = det.credits(gates, &flat, &vec![false; ng], 5).gate_row;
    let mut synth_finish = false;
    let mut out = HumanOut { starts: Vec::new(), records: Vec::new(), legs: 0, respawns: 0, log: Vec::new(), paths: Vec::new() };
    // A finishing ghost whose finish step is among the samples the exiting
    // child lost (0..5, run to run): the declared time is the finish, so the
    // finish leg ends at the row of race floor10(declared) — the row whose tick
    // the counter credits — synthesised when it is within 0.5 s past the trace.
    if let Some(decl) = w.tape.declared_ms {
        for (gi, g) in gates.gates.iter().enumerate() {
            // DETERMINISM: whether the finish step is captured varies run to run, so the
            // finish leg's end row is ALWAYS the declared-time row with the ghost's own
            // telemetry position/velocity (cm-level vs the engine row), never the flushed rows
            if g.kind == GateKind::Finish {
                let want = decl as i64 - (decl as i64 % 10); // the row whose tick the counter credits (p00001: notice 19538 → step row race 19.530)
                if let Some(i) = flat.iter().position(|r| w.race_of(r) == want) {
                    // only when the counter shows every checkpoint credited (or the finish itself)
                    if flat.iter().rev().find(|r| r.cps != u32::MAX).map(|r| r.cps as usize + 1 >= ng).unwrap_or(false) {
                        // DETERMINISM: the rows near the exit are however many the child flushed
                        // (0..5) plus extrapolation; anchor the finish row on the row 200 ms
                        // before the finish, extrapolated at constant velocity, run to run
                        // DETERMINISM: the engine rows near the exit are however many the child flushed;
                        // the ghost's OWN telemetry (50 ms samples, Hermite to 1 ms) gives the finish
                        // position deterministically; velocity from the telemetry over ±10 ms
                        // (the telemetry ends AT the declared time: velocity by a backward difference)
                        if let (Some(p), Some(pm)) = (tel.pos_at(want), tel.pos_at(want - 20)) {
                            let mut r = flat[i].clone();
                            r.x = p[0];
                            r.y = p[1];
                            r.z = p[2];
                            r.vx = (p[0] - pm[0]) / 0.02;
                            r.vy = (p[1] - pm[1]) / 0.02;
                            r.vz = (p[2] - pm[2]) / 0.02;
                            flat[i] = r;
                        } else {
                            out.log.push(format!("  finish leg: no telemetry at race {} -- engine row kept (may differ run to run)", crate::secs(want)));
                        }
                        synth_finish = first[gi] < 0;
                        first[gi] = i as i32;
                    }
                }
            }
        }
    }
    let mut events: Vec<(usize, usize)> = first.iter().enumerate().filter(|(_, t)| **t >= 0).map(|(gi, t)| (*t as usize, gi)).collect();
    events.sort();
    let respawns = respawn_ticks(&w.ghost);
    // race-0 row index
    let race0 = flat.iter().position(|r| w.race_of(r) >= 0).unwrap_or(0);
    let mut leg_start = race0;
    let mut cps: u8 = 0;
    let mut start_id = start_id_base;
    let mk_start = |row: &Row, cps: u8, id: u32, w: &Worker, tel: &Telemetry| crate::fanout::StartRow {
        start_id: id,
        ghost_md5: tel.md5.clone(),
        tick: crate::rig::tick_of_ms(&w.tape, row.time_ms) + 1,
        state: CarState::from_row(row, w.race_of(row), cps, false),
        cps_before: cps,
    };
    // respawns inside a leg are negatives from the leg's start
    let respawn_rows: Vec<usize> = respawns
        .iter()
        .filter_map(|t| {
            let label = crate::fanout::label_of_tick(w, *t);
            flat.iter().position(|r| r.time_ms == label)
        })
        .collect();
    for (row_idx, gi) in events {
        // negatives: respawn presses between leg_start and this crossing
        for &rr in respawn_rows.iter().filter(|rr| **rr > leg_start && **rr < row_idx) {
            let win = &flat[leg_start + 1..=rr];
            let st = &flat[leg_start];
            let (outcome0, path, vmin, vmax) = classify(win, speed(st), -1.0e9, false, false);
            let _ = outcome0;
            let end = CarState::from_row(&flat[rr], w.race_of(&flat[rr]), cps, false);
            out.starts.push(mk_start(st, cps, start_id, w, tel));
            out.paths.push(crate::tmr::path4(win, &|r| w.race_of(r)));
            out.records.push(Record {
                start_id,
                macro_id: RESPAWN_MACRO,
                horizon_ticks: (rr - leg_start).min(65535) as u16,
                outcome: OUTCOME_CRASH_STOP,
                end,
                gate_tick: [-1; 32],
                path_len_m: path,
                min_speed: vmin,
                max_speed: vmax,
            });
            start_id += 1;
            out.respawns += 1;
        }
        let g = &gates.gates[gi];
        let win = &flat[leg_start + 1..=row_idx];
        let st = &flat[leg_start];
        let (_, path, vmin, vmax) = classify(win, speed(st), -1.0e9, false, false);
        let finished = g.kind == GateKind::Finish;
        let mut gate_tick = [-1i16; 32];
        if (g.waypoint as usize) < 32 {
            gate_tick[g.waypoint as usize] = (row_idx - leg_start).min(32767) as i16;
        }
        let end = CarState::from_row(&flat[row_idx], w.race_of(&flat[row_idx]), cps + 1, finished);
        out.starts.push(mk_start(st, cps, start_id, w, tel));
        out.paths.push(crate::tmr::path4(win, &|r| w.race_of(r)));
        out.records.push(Record {
            start_id,
            macro_id: HUMAN_MACRO,
            horizon_ticks: (row_idx - leg_start + 1).min(65535) as u16,
            outcome: if finished { OUTCOME_FINISHED } else { OUTCOME_OK },
            end,
            gate_tick,
            path_len_m: path,
            min_speed: vmin,
            max_speed: vmax,
        });
        start_id += 1;
        out.legs += 1;
        cps += 1;
        leg_start = row_idx;
    }
    out.log.push(format!(
        "{}: {} legs, {} respawn negatives ({} respawn presses in the tape), gates in order {:?}{}",
        w.ghost.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        out.legs,
        out.respawns,
        respawns.len(),
        first.iter().enumerate().filter(|(_, t)| **t >= 0).map(|(gi, t)| (gates.gates[gi].waypoint, crate::secs(w.race_of(&flat[*t as usize])))).collect::<Vec<_>>(),
        if synth_finish { " (finish leg from the declared time: its counter step was among the exiting child's lost samples)" } else { "" }
    ));

    Ok(out)
}
