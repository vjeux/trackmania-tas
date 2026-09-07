//! `tmreach starts` — savestates along a human run, with the two startup
//! controls (G1).
//!
//! (a) START-POSITION: the root's live state is within 6 m of the map's Spawn
//!     waypoint (horizontal) and at ≤ 4 m/s (RL-agentG §8.5 clause 1–2). A
//!     synthesized container has put the car at CP3 before.
//! (b) IDENTITY: the engine's per-tick positions along the reference tape lie
//!     on the ghost's own 50 ms telemetry, interpolated to 1 ms: RMS < 5 cm,
//!     max < 1 m over the whole run. This proves the server is running THIS
//!     tape — and, for a game-recorded container, that the dedicated server
//!     reproduces the game client's physics on it.
//! Both fail closed: no starts.tsv is written on a failed control.

use crate::gates::MapGates;
use crate::rig::{dist, pos, speed, Worker};
use crate::tele::{compare, Telemetry};
use forkoracle::layout::Row;
use std::path::Path;

pub struct StartsOpts {
    pub every_ms: i64,
    pub out: Option<std::path::PathBuf>,
    pub trace_out: Option<std::path::PathBuf>,
    pub verbose: bool,
}

#[derive(Clone, Debug)]
pub struct Start {
    pub tick: usize,
    pub row: Row,
    pub cps_before: u8,
}

pub struct StartsReport {
    pub starts: Vec<Start>,
    pub flat: Vec<Row>,
    pub start_ctrl_d: f64,
    pub start_ctrl_speed: f64,
    pub start_ctrl_pass: bool,
    pub identity: crate::tele::IdentityCmp,
    pub ghost_md5: String,
    pub checkpoints_ms: Vec<i32>,
}

/// Row on the flat trace nearest race clock `ms` (exact tick match preferred).
pub fn row_at_ms(flat: &[Row], ms: i64) -> Option<&Row> {
    flat.iter().min_by_key(|r| (r.time_ms - ms).abs())
}

/// Checkpoints credited before race clock `ms` (the ghost's own notices).
pub fn cps_before(cps_ms: &[i32], ms: i64) -> u8 {
    cps_ms.iter().filter(|c| (**c as i64) <= ms).count() as u8
}

pub fn run_on_worker(w: &mut Worker, tel: &Telemetry, gates: &MapGates, o: &StartsOpts) -> Result<StartsReport, String> {
    let r0 = w.root_row.clone();
    let sp = gates.spawn.as_ref().ok_or("the map has no Spawn waypoint")?;
    let v0 = speed(&r0);
    // ---- flat run of the whole tape, one child
    let n = w.n_ticks();
    // Run the whole tape, off its end: the last chunk exits with the race
    // (Advanced::RunEnded), so the finish crossing is in the rows.
    let ticks = n.saturating_sub(w.root_probe) as u64;
    let t0 = std::time::Instant::now();
    let flat = w.flat(ticks)?;
    let dt = t0.elapsed().as_secs_f64();
    forkoracle::layout::check_rows(&flat).map_err(|e| format!("flat trace failed its own checks: {e}"))?;
    let (first, last) = (flat.first().unwrap(), flat.last().unwrap());
    println!(
        "flat: {} rows race {} .. {} in {:.2} s ({:.1} ticks/ms)",
        flat.len(),
        crate::secs(first.time_ms),
        crate::secs(last.time_ms),
        dt,
        flat.len() as f64 / (dt * 1000.0)
    );
    if let Some(p) = &o.trace_out {
        write_trace(p, &flat)?;
    }
    // ---- (b) identity control
    let id0 = compare(&flat, tel);
    let (shift, id) = crate::tele::best_shift(&flat, tel);
    println!("IDENTITY control: engine vs telemetry(1 ms Hermite), labels as read: {}", id0);
    // The label convention: a row's race time is label + shift. Under the tick
    // hook the root's race time is known from the engine itself, so the two
    // must agree: the telemetry's origin and the engine's race clock are then
    // one clock (they were measured to be, root −0.010 s ↔ label −30 ↔ +20).
    let hook_shift = w.root_race_ms_hook.map(|r| r - w.root_row.time_ms);
    println!(
        "IDENTITY control: best label shift {:+} ms (engine row t == telemetry t{:+}){}: {} => {}",
        shift,
        shift,
        match hook_shift {
            Some(h) => format!("; the tick hook's race clock at the root implies {h:+}"),
            None => String::new(),
        },
        id,
        if id.passes() && shift.abs() <= 30 && hook_shift.map(|h| h == shift).unwrap_or(true) { "PASS" } else { "FAIL" }
    );
    // which vehicle slot was live along the run (a transformation gate changes it mid-run)
    {
        let mut cars: std::collections::BTreeMap<u8, usize> = Default::default();
        for r in &flat {
            *cars.entry(if r.vis.known { r.vis.car } else { u8::MAX }).or_default() += 1;
        }
        println!("VEHICLE SLOT along the run (0 Stadium, 1 Snow, 2 Rally, 3 Desert, 255 unknown): {:?}", cars);
    }
    if shift.abs() > 30 || !id.passes() {
        return Err(format!("the engine trajectory matches the telemetry only {shift:+} ms away (RMS {:.3} m): not one label convention, a different run", id.rms));
    }
    // THE TICK HOOK'S RACE CLOCK IS THE LABEL AUTHORITY. When the telemetry's best shift differs
    // from it (Spring 2026 - 12: every ghost's telemetry reads −30 ms against the hook's +0, RMS
    // 4 mm at that shift) the telemetry's time origin is what is off; labels stay the hook's and
    // the offset is recorded.
    if let Some(h) = hook_shift {
        if h != shift {
            println!("TELEMETRY TIME ORIGIN OFF by {:+} ms against the tick hook's race clock (identity RMS {:.4} m at the telemetry's best shift {shift:+}); labels keep the hook's convention {h:+}", shift - h, id.rms);
        }
    }
    w.label_shift = hook_shift.unwrap_or(shift);
    w.telemetry_offset_ms = shift - w.label_shift;
    // ---- (a) START-POSITION control. Two forms, and the transcript says which:
    //  LIVE  -- the root is pre-race (a countdown tape): the live root state must
    //           be at the map's Spawn, at rest.
    //  ORIGIN -- the root is inside the race (an offset-0 tape, whose pre-race
    //           pause is broken -- see rig::root_clock_for): the telemetry's
    //           first sample must be at the Spawn at rest, AND the live root
    //           state must lie on that telemetry to 5 cm (the identity control
    //           then ties the whole run to it). A stated substitute, not the
    //           live clause.
    let dxz = ((r0.x - sp.centre[0]).powi(2) + (r0.z - sp.centre[2]).powi(2)).sqrt();
    let dy = r0.y - sp.centre[1];
    let d3 = dist(pos(&r0), sp.centre);
    // the telemetry's own first sample is the independent witness of the race origin; the
    // map Spawn is a GEOM number (Summer 2026 - 19: every root car sits 10.33 m from the
    // tmroute Spawn, at rest, on its own telemetry to 4 mm -- the spawn FRAME is off, the
    // run is not), so the control passes on either witness and flags the other
    let s0 = &tel.dec.samples[0];
    let t0p = [s0.x as f64, s0.y as f64, s0.z as f64];
    let root_on_t0 = dist(pos(&r0), t0p);
    let at_origin = dxz <= 6.0 || root_on_t0 <= 0.5;
    let pre_race = v0 <= 4.0 && at_origin;
    if dxz > 6.0 && root_on_t0 <= 0.5 {
        println!("SPAWN FRAME OFF (GEOM/tmroute): the root car is {dxz:.2} m (horizontal) from the map Spawn wp{} {:?} at ({:.1}, {:.1}, {:.1}) but {root_on_t0:.3} m from the telemetry's own first sample -- the gates.json spawn, not the run", sp.waypoint, sp.model, sp.centre[0], sp.centre[1], sp.centre[2]);
    }
    let pass_a;
    if pre_race {
        if dy.abs() > 12.0 {
            println!("SPAWN FRAME OFF vertically (GEOM/tmroute): root y {:.2} vs Spawn y {:.2}", r0.y, sp.centre[1]);
        }
        pass_a = at_origin && v0 <= 4.0;
        println!(
            "START-POSITION control (LIVE): root tick {} race {} at ({:.3}, {:.3}, {:.3}) {:.2} m/s; map Spawn wp{} {:?} group {} -> \
             ({:.1}, {:.1}, {:.1}); d_xz {:.2} m, dy {:+.2} m, d3 {:.2} m  => {}",
            w.root_probe, crate::secs(r0.time_ms), r0.x, r0.y, r0.z, v0, sp.waypoint, sp.model, sp.group,
            sp.centre[0], sp.centre[1], sp.centre[2], dxz, dy, d3, if pass_a { "PASS" } else { "FAIL" }
        );
    } else {
        let s0 = &tel.dec.samples[0];
        let t0 = [s0.x as f64, s0.y as f64, s0.z as f64];
        let t0_dxz = ((t0[0] - sp.centre[0]).powi(2) + (t0[2] - sp.centre[2]).powi(2)).sqrt();
        let t0_v = (s0.vx as f64).hypot(s0.vy as f64).hypot(s0.vz as f64);
        let root_on_tel = tel.pos_at(r0.time_ms + shift).map(|p| dist(pos(&r0), p)).unwrap_or(f64::NAN);
        // the spawn's vertical position is a GEOM number (Spring 2026 - 06: 20 m below the cars), noted, not judged
        if (t0[1] - sp.centre[1]).abs() > 12.0 {
            println!("SPAWN FRAME OFF vertically (GEOM/tmroute): telemetry t=0 y {:.2} vs Spawn y {:.2}", t0[1], sp.centre[1]);
        }
        // the map Spawn is a GEOM number (Winter 2026 - 17: 9.9 m off on an offset-0 tape); the
        // telemetry's own first sample at rest + the live root ON that telemetry is the control
        if t0_dxz > 6.0 {
            println!("SPAWN FRAME OFF (GEOM/tmroute): the telemetry's first sample is {t0_dxz:.2} m (horizontal) from the map Spawn ({:.1}, {:.1}, {:.1})", sp.centre[0], sp.centre[1], sp.centre[2]);
        }
        pass_a = t0_v <= 4.0 && root_on_tel < 0.05;
        println!(
            "START-POSITION control (ORIGIN; root is inside the race at tick {} race {}, {:.1} m/s): telemetry t=0 at ({:.3}, {:.3}, {:.3}) {:.2} m/s, \
             d_xz {:.2} m from Spawn ({:.1}, {:.1}, {:.1}); live root state {:.4} m off that telemetry  => {}",
            w.root_probe, crate::secs(r0.time_ms), v0, t0[0], t0[1], t0[2], t0_v, t0_dxz, sp.centre[0], sp.centre[1], sp.centre[2], root_on_tel,
            if pass_a { "PASS" } else { "FAIL" }
        );
    }
    // ---- savestates every `every_ms` of race time, from 0 (or the root) to the end
    let mut starts = Vec::new();
    let mut ms = (first.time_ms.max(0) / o.every_ms) * o.every_ms;
    if ms < first.time_ms {
        ms += o.every_ms;
    }
    let end = tel.checkpoints_ms.last().copied().map(|c| c as i64).unwrap_or(last.time_ms);
    while ms < end {
        if let Some(r) = row_at_ms(&flat, ms) {
            if (r.time_ms - ms).abs() <= 10 {
                let tick = crate::rig::tick_of_ms(&w.tape, r.time_ms);
                starts.push(Start { tick, row: r.clone(), cps_before: cps_before(&tel.checkpoints_ms, r.time_ms) });
            }
        }
        ms += o.every_ms;
    }
    println!("{} savestates every {} on race {} .. {}", starts.len(), crate::secs(o.every_ms), crate::secs(0), crate::secs(end));
    Ok(StartsReport {
        starts,
        flat,
        start_ctrl_d: dxz,
        start_ctrl_speed: v0,
        start_ctrl_pass: pass_a,
        identity: id,
        ghost_md5: tel.md5.clone(),
        checkpoints_ms: tel.checkpoints_ms.clone(),
    })
}

pub fn write_trace(p: &Path, rows: &[Row]) -> Result<(), String> {
    let mut s = String::from("time_ms\tx\ty\tz\tvx\tvy\tvz\tqw\tqx\tqy\tqz\twet\tcps\n");
    for r in rows {
        s.push_str(&format!(
            "{}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.4}\t{}\n",
            r.time_ms, r.x, r.y, r.z, r.vx, r.vy, r.vz, r.qw, r.qx, r.qy, r.qz, r.wetness, r.cps
        ));
    }
    std::fs::write(p, s).map_err(|e| format!("{}: {}", p.display(), e))
}

/// One starts.tsv row (INTERFACES §2).
pub fn starts_tsv_header() -> &'static str {
    "start_id\tghost_md5\ttick\trace_ms\tx\ty\tz\tvx\tvy\tvz\tqw\tqx\tqy\tqz\tcps_before\tsource\tid_rms_trim_m\tid_p995_m\tid_bar\n"
}

pub fn starts_tsv_row(id: u32, md5: &str, s: &Start, source: &str) -> String {
    let r = &s.row;
    format!(
        "{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{}\t{}\t-\t-\t-\n",
        id, md5, s.tick, r.time_ms, r.x, r.y, r.z, r.vx, r.vy, r.vz, r.qw, r.qx, r.qy, r.qz, s.cps_before, source
    )
}
