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
use crate::tmr::{CarState, Record, OUTCOME_CRASH_STOP, OUTCOME_FINISHED, OUTCOME_OK};
use forkoracle::layout::Row;

pub const HUMAN_MACRO: u16 = 65535;
pub const RESPAWN_MACRO: u16 = 65534;

pub struct HumanOut {
    pub starts: Vec<crate::fanout::StartRow>,
    pub records: Vec<Record>,
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
    let first = det.credits(gates, &flat, &vec![false; ng], 5).gate_row;
    let mut events: Vec<(usize, usize)> = first.iter().enumerate().filter(|(_, t)| **t >= 0).map(|(gi, t)| (*t as usize, gi)).collect();
    events.sort();
    let respawns = respawn_ticks(&w.ghost);
    let mut out = HumanOut { starts: Vec::new(), records: Vec::new(), legs: 0, respawns: 0, log: Vec::new() };
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
        out.records.push(Record {
            start_id,
            macro_id: HUMAN_MACRO,
            horizon_ticks: (row_idx - leg_start).min(65535) as u16,
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
        "{}: {} legs, {} respawn negatives ({} respawn presses in the tape), gates in order {:?}",
        w.ghost.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        out.legs,
        out.respawns,
        respawns.len(),
        first.iter().enumerate().filter(|(_, t)| **t >= 0).map(|(gi, t)| (gates.gates[gi].waypoint, crate::secs(w.race_of(&flat[*t as usize])))).collect::<Vec<_>>()
    ));

    Ok(out)
}
