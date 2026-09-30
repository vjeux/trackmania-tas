//! `tmrl synth` — a one-map BC shard from a directory of Nadeo ghosts, until the DATA arm's real shards exist.
//!
//! Inputs: the cartographer `route.json` + `pack.json` for the map (B-cartographer bank) and the ghosts
//! (`pNNNNN_MS.Ghost.Gbx`, rank in the name). Per ghost: the 10 ms input tape (`gbx::tape`) is the action label
//! for every tick; the 50 ms telemetry record (`gbx::record`) is interpolated to 10 ms — velocity-aware Hermite
//! for position, linear for velocity, nlerp for the quaternion, hold for discrete — which is INTERFACES.md's
//! `state_source = telemetry-interp`. `cps` comes from the ghost's own checkpoint times (the validator's
//! reading of THIS run), `finished` from its race time.
//!
//! What the shard is NOT: engine-resimulated. The manifest says `unverified` in the verdict column until the DATA
//! arm's resim runs; the LEARN pipeline is being stood up on it, not certified on it.

use crate::md5::md5_hex;
use crate::shard::{write_shard, Record};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use tmstate::{Action, CarState, Gate, GateKind, TrackGeom};

#[derive(Deserialize)]
struct RouteVert {
    p: [f32; 3],
    s: f32,
    w: f32,
}
#[derive(Deserialize)]
struct RouteJson {
    uid: String,
    length_m: f32,
    gate_s: Vec<f32>,
    verts: Vec<RouteVert>,
}
#[derive(Deserialize)]
struct PackJson {
    uid: String,
    name: String,
    author_ms: Option<i64>,
    spawn: [f32; 3],
    spawn_yaw: Option<f32>,
}

pub struct MapInfo {
    pub name: String,
    pub author_ms: Option<i64>,
}

/// TrackGeom from the cartographer route: verts resampled every `step` m, gates at `gate_s` with the route
/// tangent as normal (checkpoints then finish, finish = last).
pub fn geom_from_cartographer(route_path: &str, pack_path: &str, step: f32) -> Result<(TrackGeom, MapInfo), String> {
    let r: RouteJson = serde_json::from_str(&fs::read_to_string(route_path).map_err(|e| format!("{route_path}: {e}"))?)
        .map_err(|e| format!("{route_path}: {e}"))?;
    let p: PackJson = serde_json::from_str(&fs::read_to_string(pack_path).map_err(|e| format!("{pack_path}: {e}"))?)
        .map_err(|e| format!("{pack_path}: {e}"))?;
    if r.uid != p.uid {
        return Err(format!("route uid {} != pack uid {}", r.uid, p.uid));
    }
    if r.verts.len() < 2 {
        return Err("route has fewer than 2 verts".into());
    }
    // Resample by arc length.
    let mut pts = Vec::new();
    let mut hw = Vec::new();
    let mut ss = Vec::new();
    let total = r.verts.last().unwrap().s;
    let n = ((total / step).ceil() as usize).max(1);
    let mut j = 0usize;
    for i in 0..=n {
        let s = (i as f32 * step).min(total);
        while j + 2 < r.verts.len() && r.verts[j + 1].s < s {
            j += 1;
        }
        let (a, b) = (&r.verts[j], &r.verts[j + 1]);
        let t = if b.s > a.s { ((s - a.s) / (b.s - a.s)).clamp(0.0, 1.0) } else { 0.0 };
        pts.push(tmobs::add(a.p, tmobs::scale(tmobs::sub(b.p, a.p), t)));
        hw.push(if t < 0.5 { a.w } else { b.w }.max(2.0));
        ss.push(s);
    }
    let mut g = TrackGeom {
        geom_version: 1,
        map_uid: r.uid.clone(),
        pts,
        half_width: hw,
        s: ss,
        gates: Vec::new(),
        spawn: p.spawn,
        spawn_yaw: p.spawn_yaw.unwrap_or(0.0),
        source: "cartographer".into(),
        legs: None,
        route: None,
        speed_hint: None,
    };
    let ng = r.gate_s.len();
    for (i, &gs) in r.gate_s.iter().enumerate() {
        let kind = if i + 1 == ng { GateKind::Finish } else { GateKind::Checkpoint };
        g.gates.push(Gate { kind, centre: tmobs::at(&g, gs), normal: tmobs::tangent(&g, gs), half_width: tmobs::half_width(&g, gs), s: gs, map_waypoint: u32::MAX });
    }
    if (g.length() - r.length_m).abs() > 1.0 {
        return Err(format!("resampled length {} vs route length {}", g.length(), r.length_m));
    }
    Ok((g, MapInfo { name: p.name, author_ms: p.author_ms }))
}

fn nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut b = b;
    let d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    if d < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
    }
    let mut q = [0f32; 4];
    for k in 0..4 {
        q[k] = a[k] * (1.0 - t) + b[k] * t;
    }
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n < 1e-9 { a } else { [q[0] / n, q[1] / n, q[2] / n, q[3] / n] }
}

/// Per-ghost synth report line.
pub struct GhostReport {
    pub rank: u32,
    pub declared_ms: i64,
    pub ticks: usize,
    pub on_route_frac: f32,
    pub max_lateral: f32,
    pub final_s: f32,
    pub cps_end: u8,
    pub tape_ticks: usize,
    pub samples: usize,
    pub weight: f32,
    pub md5: String,
    pub split: &'static str,
}

pub fn rank_of(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix('p')?;
    let end = rest.find('_')?;
    rest[..end].parse().ok()
}

/// Sample weight by rank: rank 1 → 1.0, rank 10 → 0.30, rank 1000 → 0.13, rank 10000 → 0.098. Faster runs
/// weigh more; the slowest still count (they teach recovery).
pub fn weight_of_rank(rank: u32) -> f32 {
    1.0 / (1.0 + (rank.max(1) as f32).ln())
}

/// Decode one ghost into per-tick records. Returns the records and the report.
pub fn ghost_records(path: &Path, g: &TrackGeom, rank: u32, split: &'static str) -> Result<(Vec<Record>, GhostReport), String> {
    let ps = path.to_str().ok_or("non-utf8 path")?;
    let dec = gbx::record::decode_ghost(ps).map_err(|e| format!("{ps}: {e:?}"))?;
    let tape = gbx::tape::Tape::from_file(ps).map_err(|e| format!("{ps}: tape: {e}"))?;
    // The state word is an INPUT the engine acts on (measured 2026-09-06: stripping the WR's two 200 ms flags=0x404
    // windows from its own container turns 19.538 into a DNF). Our action is {steer, gas, brake} only, so a ghost
    // whose in-race state word is anything but the plain literal (word0 2, flags 0) carries inputs the action cannot
    // express: its labels are not what the physics saw. Such ghosts are refused here and listed in the report.
    let steer = tape.steer_i8s();
    let accel = tape.accels();
    let brake = tape.brakes();
    let t0 = tape.race_ms(0);
    if t0 % 10 != 0 {
        return Err(format!("{ps}: tape start offset {t0} ms is not on the 10 ms grid"));
    }
    let smp = &dec.samples;
    if smp.len() < 2 {
        return Err(format!("{ps}: {} telemetry samples", smp.len()));
    }
    let race_ms_end = dec.race_time_ms.map(|x| x as i64).unwrap_or(smp.last().unwrap().time_ms as i64);
    let mut cps_ms: Vec<i64> = dec.checkpoints_ms.iter().map(|&x| x as i64).collect();
    // The finish is the last checkpoint entry when it equals the race time; drop it from the cp count.
    if let Some(&last) = cps_ms.last() {
        if (last - race_ms_end).abs() <= 1 {
            cps_ms.pop();
        }
    }
    let last_sample_ms = smp.last().unwrap().time_ms as i64;
    let tick_end = (last_sample_ms.min(race_ms_end) / 10) as usize; // inclusive
    let mut recs = Vec::with_capacity(tick_end + 1);
    let mut si = 0usize;
    let mut prev_q: Option<[f32; 4]> = None;
        let mut on_route = 0usize;
    let mut max_lat = 0f32;
    let mut final_s = 0f32;
    let weight = weight_of_rank(rank);
    let mut cps_end = 0u8;
    for tick in 0..=tick_end {
        let ms = 10 * tick as i64;
        // tape index
        let ti = (ms - t0) / 10;
        if ti < 0 || ti as usize >= steer.len() {
            return Err(format!("{ps}: tick {tick} (race {ms} ms) is outside the tape ({} ticks from {t0} ms)", steer.len()));
        }
        let ti = ti as usize;
        let action = Action { steer: steer[ti], gas: accel[ti] != 0, brake: brake[ti] != 0 };
        // bracketing samples
        while si + 2 < smp.len() && (smp[si + 1].time_ms as i64) <= ms {
            si += 1;
        }
        let (a, b) = (&smp[si], &smp[si + 1]);
        let (ta, tb) = (a.time_ms as f32 / 1000.0, b.time_ms as f32 / 1000.0);
        let h = (tb - ta).max(1e-6);
        let u = (((ms as f32 / 1000.0) - ta) / h).clamp(0.0, 1.0);
        let (u2, u3) = (u * u, u * u * u);
        let (h00, h10, h01, h11) = (2.0 * u3 - 3.0 * u2 + 1.0, u3 - 2.0 * u2 + u, -2.0 * u3 + 3.0 * u2, u3 - u2);
        let pa = [a.x, a.y, a.z];
        let pb = [b.x, b.y, b.z];
        let va = [a.vx, a.vy, a.vz];
        let vb = [b.vx, b.vy, b.vz];
        let mut pos = [0f32; 3];
        let mut vel = [0f32; 3];
        for k in 0..3 {
            pos[k] = h00 * pa[k] + h10 * h * va[k] + h01 * pb[k] + h11 * h * vb[k];
            vel[k] = va[k] * (1.0 - u) + vb[k] * u;
        }
        // CarState stores (w, x, y, z); the record decodes (qx, qy, qz, qw).
        let qa = [a.qw, a.qx, a.qy, a.qz];
        let qb = [b.qw, b.qx, b.qy, b.qz];
        let quat = nlerp(qa, qb, u);
        let ang_vel = match prev_q {
            Some(pq) => tmobs::ang_vel_from_quats(pq, quat, 0.010),
            None => [0.0; 3],
        };
        prev_q = Some(quat);
        let held = if u < 1.0 { a } else { b };
        let contact = if held.is_ground_contact { 1u8 } else { 0u8 };
        let cps = cps_ms.iter().filter(|&&c| c <= ms).count() as u8;
        cps_end = cps;
        let st = CarState {
            race_ms: ms as i32,
            pos,
            vel,
            quat,
            ang_vel,
            speed: tmobs::norm(vel),
            gear: (held.gear.round().max(0.0) as u8).min(6),
            rpm: held.rpm_raw as f32, // the record's raw byte; unit unknown, kept as-is
            wheel_contact: [contact; 4], // the record has ONE contact flag; broadcast (manifest header says so)
            wheel_material: [u8::MAX; 4],
            wheel_slip: [f32::NAN; 4],
            turbo: if held.is_turbo { held.turbo_time } else { 0.0 },
            cps,
            finished: ms >= race_ms_end,
            car: u8::MAX,
            ..CarState::unknown()
        };
        // Track progress along the route for the report (the trainer recomputes it; this is the control that
        // the ghosts drive the cartographer route at all).
        let pr = tmobs::probe(g, &st);
        if pr.lateral.abs() <= pr.half_width {
            on_route += 1;
        }
        max_lat = max_lat.max(pr.lateral.abs());
        final_s = pr.s;
        recs.push(Record { map_uid: g.map_uid.clone(), ghost_id: rank, tick: tick as u32, state: st, action, weight });
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let rep = GhostReport {
        rank,
        declared_ms: race_ms_end,
        ticks: recs.len(),
        on_route_frac: on_route as f32 / recs.len().max(1) as f32,
        max_lateral: max_lat,
        final_s,
        cps_end,
        tape_ticks: steer.len(),
        samples: smp.len(),
        weight,
        md5: md5_hex(&bytes),
        split,
    };
    Ok((recs, rep))
}

pub struct SynthArgs {
    /// "cartographer" (route.json + pack.json) or "wr" (the field: fastest ghost line + field spread).
    pub geom_source: String,
    pub route: String,
    pub pack: String,
    pub ghosts_dir: String,
    pub out: String,
    pub heldout_mod: u32, // ghost index (sorted by rank) % heldout_mod == heldout_mod-1 → heldout
}

pub fn run(a: &SynthArgs) -> Result<(), String> {
    let (g_cart, info) = geom_from_cartographer(&a.route, &a.pack, 2.0)?;
    fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    let mut files: Vec<(u32, std::path::PathBuf)> = fs::read_dir(&a.ghosts_dir)
        .map_err(|e| format!("{}: {e}", a.ghosts_dir))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_str().map(|s| s.ends_with(".Ghost.Gbx")).unwrap_or(false))
        .filter_map(|p| rank_of(&p).map(|r| (r, p)))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("no pNNNNN_*.Ghost.Gbx in {}", a.ghosts_dir));
    }
    let g = match a.geom_source.as_str() {
        "cartographer" => g_cart,
        "wr" => geom_from_field(&files, &g_cart.map_uid, 2.0, 2.0, 4.0)?,
        other => return Err(format!("--geom-source {other}: cartographer|wr")),
    };
    fs::write(format!("{}/geom.json", a.out), serde_json::to_string_pretty(&g).unwrap()).map_err(|e| e.to_string())?;
    let mut train = Vec::new();
    let mut held = Vec::new();
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    // Only plain-literal ghosts carry BC labels; the split index runs over THOSE so the held-out fraction stays 1/4.
    let mut plain: Vec<(u32, std::path::PathBuf)> = Vec::new();
    for (rank, path) in &files {
        match nonplain_reason(path)? {
            Some(r) => failures.push(r),
            None => plain.push((*rank, path.clone())),
        }
    }
    for (i, (rank, path)) in plain.iter().enumerate() {
        let split = if a.heldout_mod > 0 && (i as u32) % a.heldout_mod == a.heldout_mod - 1 { "heldout" } else { "train" };
        match ghost_records(path, &g, *rank, split) {
            Ok((recs, rep)) => {
                if split == "heldout" { held.extend(recs) } else { train.extend(recs) }
                reports.push(rep);
            }
            Err(e) => failures.push(e),
        }
    }
    write_shard(&format!("{}/train.tmd", a.out), &train)?;
    write_shard(&format!("{}/heldout.tmd", a.out), &held)?;
    let mut man = String::from(
        "# map_uid\tmap_name\tsource\trank\tdeclared_ms\tresim_ms\tresim_verdict\tghost_md5\tticks\tsplit\tweight\ton_route_frac\tmax_lateral_m\tfinal_s_m\tcps_end\n",
    );
    man.push_str(&format!(
        "# state_source=telemetry-interp (Hermite pos / linear vel / nlerp quat / hold discrete, 50→10 ms); wheel_contact = the record's single contact flag broadcast to 4 wheels; wheel_material/slip unknown; rpm = raw record byte; geom source=cartographer route resampled 2 m, length {:.1} m, {} gates; author_ms={:?}\n",
        g.length(), g.gates.len(), info.author_ms
    ));
    for r in &reports {
        man.push_str(&format!(
            "{}\t{}\tcampaign\t{}\t{}\t-\tunverified\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.2}\t{:.1}\t{}\n",
            g.map_uid, info.name, r.rank, r.declared_ms, r.md5, r.ticks, r.split, r.weight, r.on_route_frac, r.max_lateral, r.final_s, r.cps_end
        ));
    }
    fs::write(format!("{}/manifest.tsv", a.out), &man).map_err(|e| e.to_string())?;
    println!("map {} ({}), geom {} {:.1} m, {} gates at {:?}", info.name, g.map_uid, g.source, g.length(), g.gates.len(), g.gates.iter().map(|x| format!("{:.1}", x.s)).collect::<Vec<_>>());
    println!("{:>6} {:>9} {:>6} {:>6} {:>8} {:>8} {:>9} {:>4} {:>7} split", "rank", "declared", "ticks", "tape", "onroute", "maxlat", "final_s", "cps", "weight");
    for r in &reports {
        println!(
            "{:>6} {:>9.3} {:>6} {:>6} {:>8.3} {:>8.2} {:>9.1} {:>4} {:>7.3} {}",
            r.rank, r.declared_ms as f64 / 1000.0, r.ticks, r.tape_ticks, r.on_route_frac, r.max_lateral, r.final_s, r.cps_end, r.weight, r.split
        );
    }
    println!("train {} records ({} ghosts), heldout {} records ({} ghosts), {} failures", train.len(), reports.iter().filter(|r| r.split == "train").count(), held.len(), reports.iter().filter(|r| r.split == "heldout").count(), failures.len());
    for f in &failures {
        println!("{}", if f.contains("NON-PLAIN") { format!("EXCLUDED {f}") } else { format!("FAIL {f}") });
    }
    let hard: Vec<&String> = failures.iter().filter(|f| !f.contains("NON-PLAIN")).collect();
    if !hard.is_empty() {
        return Err(format!("{} ghosts failed to decode", hard.len()));
    }
    Ok(())
}

/// One ghost's interpolated 10 ms trajectory, for geometry building: positions per tick, checkpoint ticks, end.
fn decode_traj(path: &Path) -> Result<(Vec<CarState>, Vec<usize>, f32), String> {
    // Reuse ghost_records against a dummy straight geometry: only pos / cps are read here.
    let dummy = TrackGeom {
        geom_version: 1,
        map_uid: "dummy".into(),
        pts: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        half_width: vec![1.0, 1.0],
        s: vec![0.0, 1.0],
        gates: vec![],
        spawn: [0.0; 3],
        spawn_yaw: 0.0,
        source: "dummy".into(),
        legs: None,
        route: None,
        speed_hint: None,
    };
    let (recs, _) = ghost_records(path, &dummy, 1, "train")?;
    let pos: Vec<CarState> = recs.iter().map(|r| r.state).collect();
    let mut cp_ticks = Vec::new();
    let mut last = 0u8;
    for r in &recs {
        if r.state.cps > last {
            cp_ticks.push(r.tick as usize);
            last = r.state.cps;
        }
    }
    let yaw = {
        let a = pos[0].pos;
        let b = pos[pos.len().min(50) - 1].pos;
        (b[0] - a[0]).atan2(b[2] - a[2])
    };
    Ok((pos, cp_ticks, yaw))
}

/// TrackGeom from the field itself: the fastest ghost's line, resampled every `step` m, is the centreline
/// ("wr-trajectory"); the corridor half-width at each station is the field's max |lateral| there + `margin`
/// (floor `floor`); checkpoints are where the fastest ghost's cps counter flipped, the finish is the end of its
/// line. No map file, no pak: on a map with no cartographer route the ghosts ARE the route (PLAYER-PLAN §5.5).
pub fn geom_from_field(files: &[(u32, std::path::PathBuf)], map_uid: &str, step: f32, margin: f32, floor: f32) -> Result<TrackGeom, String> {
    let (wr_rank, wr_path) = files.iter().min_by_key(|(r, _)| *r).ok_or("no ghosts")?;
    let (states, cp_ticks, yaw) = decode_traj(wr_path)?;
    let pos: Vec<[f32; 3]> = states.iter().map(|s| s.pos).collect();
    // Arc length along the WR line, then resample.
    let mut s_raw = vec![0f32];
    for i in 1..pos.len() {
        s_raw.push(s_raw[i - 1] + tmobs::norm(tmobs::sub(pos[i], pos[i - 1])));
    }
    let total = *s_raw.last().unwrap();
    let n = (total / step).ceil() as usize;
    let mut pts = Vec::with_capacity(n + 1);
    let mut ss = Vec::with_capacity(n + 1);
    let mut j = 0usize;
    for i in 0..=n {
        let s = (i as f32 * step).min(total);
        while j + 2 < pos.len() && s_raw[j + 1] < s {
            j += 1;
        }
        let t = if s_raw[j + 1] > s_raw[j] { ((s - s_raw[j]) / (s_raw[j + 1] - s_raw[j])).clamp(0.0, 1.0) } else { 0.0 };
        pts.push(tmobs::add(pos[j], tmobs::scale(tmobs::sub(pos[j + 1], pos[j]), t)));
        ss.push(s);
    }
    let mut g = TrackGeom {
        geom_version: 1,
        map_uid: map_uid.to_string(),
        half_width: vec![floor; pts.len()],
        pts,
        s: ss,
        gates: Vec::new(),
        spawn: pos[0],
        spawn_yaw: yaw,
        source: format!("wr-trajectory(rank {wr_rank})"),
        legs: None,
        route: None,
        speed_hint: None,
    };
    // Gates: cps flips of the WR, finish at the end.
    let mut gate_s: Vec<f32> = cp_ticks.iter().map(|&t| s_raw[t.min(s_raw.len() - 1)]).collect();
    gate_s.push(total);
    let ng = gate_s.len();
    for (i, &gs) in gate_s.iter().enumerate() {
        let kind = if i + 1 == ng { GateKind::Finish } else { GateKind::Checkpoint };
        g.gates.push(Gate { kind, centre: tmobs::at(&g, gs), normal: tmobs::tangent(&g, gs), half_width: floor, s: gs, map_waypoint: u32::MAX });
    }
    // Corridor: the field's lateral spread per station.
    let mut maxlat = vec![0f32; g.pts.len()];
    for (_, p) in files {
        let (pp, _, _) = decode_traj(p)?;
        for q in &pp {
            let pr = tmobs::probe(&g, q);
            let i = ((pr.s / step).round() as usize).min(maxlat.len() - 1);
            maxlat[i] = maxlat[i].max(pr.lateral.abs());
        }
    }
    for i in 0..g.pts.len() {
        g.half_width[i] = (maxlat[i] + margin).max(floor);
    }
    let hws: Vec<f32> = g.gates.iter().map(|gt| tmobs::half_width(&g, gt.s)).collect();
    for (gt, hw) in g.gates.iter_mut().zip(hws) {
        gt.half_width = hw;
    }
    Ok(g)
}

/// The state word is an INPUT the engine acts on (measured 2026-09-06: stripping the WR's two 200 ms `flags=0x404`
/// windows from its own container turns 19.538 into a DNF). Our action is {steer, gas, brake} only, so a ghost
/// whose in-race state word is anything but the plain literal (word0 2, flags 0) carries inputs the action cannot
/// express: its labels are not what the physics saw. Returns the reason such a ghost must be excluded from BC
/// labels (its trajectory is still a fine geometry source).
pub fn nonplain_reason(path: &Path) -> Result<Option<String>, String> {
    let ps = path.to_str().ok_or("non-utf8 path")?;
    let tape = gbx::tape::Tape::from_file(ps).map_err(|e| format!("{ps}: tape: {e}"))?;
    let Some(arch) = tape.archives.first() else { return Ok(None) };
    // Only the RACE counts: a literal after the finish (word0 0xf on several ghosts) drives nothing.
    let end_ms = gbx::record::decode_ghost(ps).ok().and_then(|d| d.race_time_ms).map(|x| x as i64).unwrap_or(i64::MAX);
    let mut bad = std::collections::BTreeMap::new();
    for (i, p) in arch.packets.iter().enumerate() {
        let race = arch.start_offset_ms as i64 + 10 * i as i64;
        if race >= 0 && race < end_ms && (p.word0 != 2 || p.flags != 0) {
            *bad.entry((p.word0, p.flags)).or_insert(0usize) += 1;
        }
    }
    if bad.is_empty() {
        return Ok(None);
    }
    let desc: Vec<String> = bad.iter().map(|((w, f), n)| format!("word0=0x{w:x} flags=0x{f:x} x{n}")).collect();
    Ok(Some(format!("{ps}: NON-PLAIN state word during the race ({}): inputs the action space cannot express; excluded", desc.join(", "))))
}
