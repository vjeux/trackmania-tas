//! Savestate INJECTION hand-off (MODEL arm, coordinator 2026-09-09 23:27Z): the state file the fork engine's
//! writer (ENV arm) consumes, its converter from vjeux's LaunchedCP caches (`ghost lcp` JSON), and the seam
//! `tmreach lap --inject-state FILE` uses to start a search from an injected state with the human's approach
//! inputs as the first chain. Schema: tm-route/model/INJECT-STATE.md (`tm-inject-state/1`).
//!
//! ENV owns the memory writer (`inject_state` below is the seam it fills in); nothing here re-derives an
//! address. Until the writer lands, `inject_state` returns an error naming what is missing.

use forkoracle::forksrv::Rec;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "tm-inject-state/1";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source {
    pub file: String,
    pub entry: usize,
    pub landmark: u32,
    /// "approach-start" (the first approach sample; `inputs` replay the approach) or "crossing" (the state
    /// at the checkpoint; `inputs` empty).
    pub kind: String,
    pub time_ms: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Wheel {
    pub rotation: f32,
    pub steer: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    pub pos: [f32; 3],
    /// (w, x, y, z) — the engine / tmstate order. The LCP decoder emits (x, y, z, w); `from_lcp` converts.
    pub quat_wxyz: [f32; 4],
    pub vel: [f32; 3],
    #[serde(default)]
    pub ang_vel: Option<[f32; 3]>,
    #[serde(default)]
    pub speed_fwd: Option<f32>,
    #[serde(default)]
    pub wheels: Option<Vec<Wheel>>,
    #[serde(default)]
    pub gear: Option<u8>,
    #[serde(default)]
    pub rpm_raw: Option<u32>,
    #[serde(default)]
    pub ground: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InputTick {
    pub tick: u32,
    pub steer: i8,
    pub gas: bool,
    pub brake: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Expect {
    pub landmark: u32,
    pub credit_within_ticks: u32,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub quat_wxyz: [f32; 4],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InjectState {
    pub schema: String,
    pub map_uid: String,
    pub map_name: String,
    pub source: Source,
    pub state: State,
    /// 10 ms ticks to replay after the injection (the LCP approach samples, ~53 ms apart, held between samples).
    pub inputs: Vec<InputTick>,
    #[serde(default)]
    pub expect: Option<Expect>,
}

fn f3(v: &serde_json::Value) -> Option<[f32; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}
fn q_xyzw_to_wxyz(v: &serde_json::Value) -> Option<[f32; 4]> {
    let a = v.as_array()?;
    let (x, y, z, w) = (a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32, a.get(3)?.as_f64()? as f32);
    Some([w, x, y, z])
}

/// Angular velocity from two orientations `dt` seconds apart (world frame, rad/s): 2·(q1·q0⁻¹).xyz / dt.
fn ang_vel_from_quats(q0: [f32; 4], q1: [f32; 4], dt: f32) -> [f32; 3] {
    let inv = [q0[0], -q0[1], -q0[2], -q0[3]];
    let (aw, ax, ay, az) = (q1[0], q1[1], q1[2], q1[3]);
    let (bw, bx, by, bz) = (inv[0], inv[1], inv[2], inv[3]);
    let dw = aw * bw - ax * bx - ay * by - az * bz;
    let dx = aw * bx + ax * bw + ay * bz - az * by;
    let dy = aw * by - ax * bz + ay * bw + az * bx;
    let dz = aw * bz + ax * by - ay * bx + az * bw;
    let s = if dw < 0.0 { -1.0 } else { 1.0 };
    let k = 2.0 * s / dt.max(1e-3);
    [dx * k, dy * k, dz * k]
}

/// Convert one entry of `ghost lcp --json` output. `kind`: "approach-start" (state = the first approach
/// sample, inputs = the whole approach) or "crossing" (state = the crossing, no inputs).
pub fn from_lcp(lcp: &serde_json::Value, entry: usize, kind: &str, map_uid: &str, map_name: &str) -> Result<InjectState, String> {
    let e = lcp.get("entries").and_then(|v| v.as_array()).and_then(|a| a.get(entry)).ok_or_else(|| format!("entry {entry} not in the LCP file"))?;
    let landmark = e.get("landmark").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let time_ms = e.get("time_ms").and_then(|v| v.as_i64()).unwrap_or(0);
    let file = lcp.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string();
    // the crossing state (always the expectation)
    let cross_pos = f3(e.get("pos").ok_or("entry.pos")?).ok_or("entry.pos")?;
    let cross_quat = q_xyzw_to_wxyz(e.get("quat").ok_or("entry.quat")?).ok_or("entry.quat")?;
    let cross_vel = f3(e.get("vel").ok_or("entry.vel")?).ok_or("entry.vel")?;
    let cross_angvel = e.get("angvel").and_then(f3);
    let wheels: Option<Vec<Wheel>> = e.get("wheels").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|w| Some(Wheel { rotation: w.get("rotation")?.as_f64()? as f32, steer: w.get("steer")?.as_f64()? as f32 })).collect());
    let speed_fwd = e.get("speed_fwd").and_then(|v| v.as_f64()).map(|x| x as f32);
    let samples: Vec<&serde_json::Value> = e.get("samples").and_then(|v| v.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
    let (state, inputs, expect) = match kind {
        "crossing" => (
            State { pos: cross_pos, quat_wxyz: cross_quat, vel: cross_vel, ang_vel: cross_angvel, speed_fwd, wheels, gear: None, rpm_raw: None, ground: None },
            Vec::new(),
            None,
        ),
        "approach-start" => {
            let s0 = samples.first().ok_or("entry has no approach samples")?;
            let pos = f3(s0.get("pos").ok_or("sample.pos")?).ok_or("sample.pos")?;
            let quat = q_xyzw_to_wxyz(s0.get("quat").ok_or("sample.quat")?).ok_or("sample.quat")?;
            let vel = f3(s0.get("vel").ok_or("sample.vel")?).ok_or("sample.vel")?;
            let ang_vel = samples.get(1).and_then(|s1| {
                let q1 = q_xyzw_to_wxyz(s1.get("quat")?)?;
                let dt = (s1.get("t_ms")?.as_f64()? - s0.get("t_ms")?.as_f64()?) as f32 / 1000.0;
                Some(ang_vel_from_quats(quat, q1, dt))
            });
            let gear = s0.get("gear").and_then(|v| v.as_u64()).map(|g| g as u8);
            let rpm_raw = s0.get("rpm_raw").and_then(|v| v.as_u64()).map(|g| g as u32);
            let ground = s0.get("ground").and_then(|v| v.as_u64()).map(|g| g != 0);
            // inputs: each sample's steer/gas/brake held from its t_ms to the next sample's, on 10 ms ticks;
            // the last sample runs to the crossing (t_window end = the entry's approach span)
            let t0 = s0.get("t_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
            // hold the last sample's inputs 20 ticks past the recorded crossing: the injected car settles slower than the
            // human (wheel/engine caches are not written), so the crossing comes a little later (ENV: 170 ticks vs 149)
            let t_end = samples.last().and_then(|s| s.get("t_ms")).and_then(|v| v.as_f64()).unwrap_or(t0) + 40.0 + 200.0;
            let mut inputs = Vec::new();
            let n_ticks = (((t_end - t0) / 10.0).round() as u32).max(1);
            for tick in 0..n_ticks {
                let t = t0 + tick as f64 * 10.0;
                let s = samples.iter().rev().find(|s| s.get("t_ms").and_then(|v| v.as_f64()).map_or(false, |ts| ts <= t + 1e-6)).unwrap_or(s0);
                let steer_held = s.get("steer").and_then(|v| v.as_f64()).unwrap_or(0.0);
                // STEER_INTERP: linear steer between consecutive samples (the human's analog stick moved continuously; a
                // 53 ms hold is a staircase the engine pays for in speed — ENV 00:07Z)
                let steer_v = if std::env::var("TMREACH_LCP_HOLD").is_ok() {
                    steer_held
                } else {
                    let ts = s.get("t_ms").and_then(|v| v.as_f64()).unwrap_or(t);
                    match samples.iter().find(|n| n.get("t_ms").and_then(|v| v.as_f64()).map_or(false, |tn| tn > ts + 1e-6)) {
                        Some(nx) => {
                            let tn = nx.get("t_ms").and_then(|v| v.as_f64()).unwrap_or(ts + 1.0);
                            let sn = nx.get("steer").and_then(|v| v.as_f64()).unwrap_or(steer_held);
                            let a = ((t - ts) / (tn - ts).max(1.0)).clamp(0.0, 1.0);
                            steer_held + (sn - steer_held) * a
                        }
                        None => steer_held,
                    }
                };
                let steer = (steer_v * 127.0).round().clamp(-127.0, 127.0) as i8;
                let gas = s.get("gas").and_then(|v| v.as_f64()).unwrap_or(0.0) > 0.5;
                let brake = s.get("brake").and_then(|v| v.as_f64()).unwrap_or(0.0) > 0.5;
                inputs.push(InputTick { tick, steer, gas, brake });
            }
            let expect = Expect { landmark, credit_within_ticks: n_ticks, pos: cross_pos, vel: cross_vel, quat_wxyz: cross_quat };
            (State { pos, quat_wxyz: quat, vel, ang_vel, speed_fwd: s0.get("speed").and_then(|v| v.as_f64()).map(|x| x as f32), wheels: None, gear, rpm_raw, ground }, inputs, Some(expect))
        }
        other => return Err(format!("kind {other:?}: approach-start | crossing")),
    };
    Ok(InjectState { schema: SCHEMA.into(), map_uid: map_uid.into(), map_name: map_name.into(), source: Source { file, entry, landmark, kind: kind.into(), time_ms }, state, inputs, expect })
}

pub fn inputs_to_recs(inputs: &[InputTick]) -> Vec<Rec> {
    inputs.iter().map(|i| Rec { steer: i.steer as f32 / 127.0, gas: i.gas as u8 as f32, brake: i.brake as u8 as f32 }).collect()
}

pub fn load(path: &std::path::Path) -> Result<InjectState, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let s: InjectState = serde_json::from_str(&txt).map_err(|e| format!("{}: {e}", path.display()))?;
    if s.schema != SCHEMA {
        return Err(format!("{}: schema {:?}, this binary reads {SCHEMA}", path.display(), s.schema));
    }
    Ok(s)
}

/// THE SEAM: write `st.state` into the fork's live body at `h`, step one tick, read back. ENV's writer fills
/// this in (their locate map: controller+0x1a70 → sim → playground → scene → vehmgr → dyna body; copy-out
/// block phy+0x12e0/f0/fc). Returns the handle to search from and the read-back row.
/// Wired to ENV's writer (player-env 7d0a0a6c): fork a child from `h`, resolve the live car in it (the same
/// derivation the worker uses for car-switch maps), write the dyna body AND the phy copies
/// (`write_body_and_phy` — the body alone did not steer the next step), read back. The caller steps it.
/// Inject after race 0: during the countdown the engine holds the car at the spawn (ENV).
pub fn inject_state(w: &mut crate::rig::Worker, h: branch::Handle, st: &InjectState) -> Result<(branch::Handle, forkoracle::layout::Row), String> {
    let nh = w.forest.fork(h)?;
    let pid = w.forest.pid_of(nh)?;
    let (sim_ms, race_start) = w.forest.clock_of(nh)?;
    let car = forkoracle::car::resolve_with(w.car.controller, w.car.sim, w.module_base, sim_ms, race_start, |a, n| forkoracle::procmem::read_at(pid, a, n)).map_err(|e| format!("inject_state: no live body to write (inside a respawn window or before the spawn?): {e}"))?;
    let body = forkoracle::inject::BodyState { pos: st.state.pos, quat_wxyz: st.state.quat_wxyz, vel: st.state.vel, ang_vel: st.state.ang_vel.unwrap_or([0.0; 3]) };
    let back = forkoracle::inject::write_body_and_phy(pid, &car, &body)?;
    let d = back.dist(&body);
    if d > 0.05 {
        return Err(format!("inject_state: read-back differs from the written state by {d:.3} m"));
    }
    // the row the search sees: the written state on the worker's current row layout
    let mut row = w.root_row.clone();
    row.x = st.state.pos[0] as f64;
    row.y = st.state.pos[1] as f64;
    row.z = st.state.pos[2] as f64;
    row.vx = st.state.vel[0] as f64;
    row.vy = st.state.vel[1] as f64;
    row.vz = st.state.vel[2] as f64;
    Ok((nh, row))
}
