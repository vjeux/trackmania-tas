//! `jumprig reactortest` — proves the reactor-contact field on the car
//! physics object against a live game, under the lock, and records the
//! evidence for REACTOR.md.
//!
//! The claim (tools/openplanet-plugin/ReactorProbe/REACTOR.md): the physics
//! step refreshes `car+0x13B0 = now` on EVERY tick the car touches a reactor
//! surface (gameplay materials 12/14/18/19) and leaves it alone otherwise; the
//! duration `car+0x13BC` reads 6000 after the first contact. The run drives a
//! purpose-built straight (two RoadTechSpecialBoost pads, a GateSpecialBoost
//! ring, a RoadTechSpecialBoost2 pad) with the Reactor Probe plugin logging
//! the block every frame, respawns, drives again, restarts the map (a new car
//! object) and drives once more. The log is then judged frame by frame:
//!
//!   * a contact EPISODE = consecutive frames whose `contact` value moved;
//!     inside one, `contact` must track the playground GameTime (same clock,
//!     within a tick or two) and the car must be over a reactor cell;
//!   * outside the reactor cells the value must not move at all;
//!   * `duration` must read 6000 from the first contact on.
//!
//! The car pointer comes from the Jump Button hook (the physics handler's own
//! argument), so the probe's Openplanet-side discovery — which CSmPlayer /
//! CSceneVehicleVis qwords lead to that object — is checked against ground
//! truth, and the signature scan (`scan`, no hook) must name the same object.

use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tmdrive::{ops, GameLock, Host};

use crate::{
    bool_after, enter_map, launch_and_hook, num_after, read_state, send_command, str_after,
    wait_for, POLL_TICK, PROGRESS_EVERY,
};

fn probe_dir() -> PathBuf {
    PathBuf::from(format!(
        "{}/Users/vjeux/OpenplanetNext/PluginStorage/ReactorProbe",
        tmdrive::drive_c()
    ))
}

/// The deliverable itself, the read-only Reactor Contact plugin: its state file
/// (written at 10 Hz when the automation marker is present) is checked against
/// the probe during every drive.
fn contact_dir() -> PathBuf {
    PathBuf::from(format!(
        "{}/Users/vjeux/OpenplanetNext/PluginStorage/ReactorContact",
        tmdrive::drive_c()
    ))
}

/// (resolved, car, touching, contact, game_time)
fn read_contact_state() -> Option<(bool, String, bool, u32, i64)> {
    let raw = fs::read_to_string(contact_dir().join("state.json")).ok()?;
    if !raw.trim_end().ends_with('}') {
        return None;
    }
    Some((
        bool_after(&raw, "resolved")?,
        str_after(&raw, "car").unwrap_or_default(),
        bool_after(&raw, "touching")?,
        num_after(&raw, "contact").unwrap_or(0.0) as u32,
        num_after(&raw, "game_time").unwrap_or(0.0) as i64,
    ))
}

/// The straight built by `tmmaps straight ... --specials` (see REACTOR.md):
/// x cell 24, start cell z=2, pads at z cells 6-7 (lvl 1), Boost2 pad at 10 (lvl 2),
/// PlatformTechBase from 13 to 30 with GateSpecialBoost hoops sunk 2 m as free blocks at 16
/// (corner position) and 20 (centre position), a grid one at 26; no checkpoint, finish 45/46.
pub const REACTOR_MAP: &str = "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/Probe/reactor.Map.Gbx";
const CELL: f64 = 32.0;
/// (z cell, expected level, label)
const REACTOR_CELLS: &[(i32, u32, &str)] = &[(6, 1, "pad Boost"), (7, 1, "pad Boost"), (10, 2, "pad Boost2"), (16, 1, "ring GateSpecialBoost, free block sunk 2 m (corner pos)"), (20, 1, "ring GateSpecialBoost, free block sunk 2 m (centre pos)"), (26, 1, "ring GateSpecialBoost on the grid")];

#[derive(Debug, Clone, Default)]
pub struct ProbeState {
    pub heartbeat: u64,
    pub in_map: bool,
    pub car: String,
    pub car_source: String,
    pub game_time: i64,
    pub kind: u32,
    pub lvl: u32,
    pub contact: u32,
    pub activation: u32,
    pub start: u32,
    pub duration: u32,
    pub vis_lvl: i64,
    pub vis_type: i64,
    pub final_timer: f64,
    pub log_count: u64,
    pub contact_frames: u64,
    pub cmd_seq: i64,
    pub cmd_result: String,
    pub status: String,
}

pub fn read_probe_state() -> Option<ProbeState> {
    let raw = fs::read_to_string(probe_dir().join("state.json")).ok()?;
    if !raw.trim_end().ends_with('}') {
        return None;
    }
    Some(ProbeState {
        heartbeat: num_after(&raw, "heartbeat")? as u64,
        in_map: bool_after(&raw, "in_map")?,
        car: str_after(&raw, "car").unwrap_or_default(),
        car_source: str_after(&raw, "car_source").unwrap_or_default(),
        game_time: num_after(&raw, "game_time").unwrap_or(0.0) as i64,
        kind: num_after(&raw, "type").unwrap_or(0.0) as u32,
        lvl: num_after(&raw, "lvl").unwrap_or(0.0) as u32,
        contact: num_after(&raw, "contact").unwrap_or(0.0) as u32,
        activation: num_after(&raw, "activation").unwrap_or(0.0) as u32,
        start: num_after(&raw, "start").unwrap_or(0.0) as u32,
        duration: num_after(&raw, "duration").unwrap_or(0.0) as u32,
        vis_lvl: num_after(&raw, "vis_lvl").unwrap_or(-1.0) as i64,
        vis_type: num_after(&raw, "vis_type").unwrap_or(-1.0) as i64,
        final_timer: num_after(&raw, "final_timer").unwrap_or(0.0),
        log_count: num_after(&raw, "log_count").unwrap_or(0.0) as u64,
        contact_frames: num_after(&raw, "contact_frames").unwrap_or(0.0) as u64,
        cmd_seq: num_after(&raw, "cmd_seq").unwrap_or(-1.0) as i64,
        cmd_result: str_after(&raw, "cmd_result").unwrap_or_default(),
        status: str_after(&raw, "status").unwrap_or_default(),
    })
}

pub fn print_probe_state(s: &ProbeState) {
    println!("heartbeat      {}   in map {}", s.heartbeat, s.in_map);
    println!("car            {} ({})", s.car, s.car_source);
    println!("game time      {}", s.game_time);
    println!("reactor        type {} lvl {} contact {} activation {} start {} duration {}", s.kind, s.lvl, s.contact, s.activation, s.start, s.duration);
    println!("vis            lvl {} type {} final timer {:.3}", s.vis_lvl, s.vis_type, s.final_timer);
    println!("log            {} lines, {} contact-refresh frames", s.log_count, s.contact_frames);
    println!("last command   #{} -> {}", s.cmd_seq, s.cmd_result);
    println!("status         {}", s.status);
}

/// `alive` (heartbeat moving), `car` (a car accepted), `cmd:N`.
pub fn wait_probe(host: &Host, event: &str, timeout: Duration) -> Result<ProbeState, String> {
    let start = Instant::now();
    let base_heartbeat = read_probe_state().map(|s| s.heartbeat).unwrap_or(0);
    let wanted_seq: Option<i64> = event.strip_prefix("cmd:").and_then(|n| n.parse::<i64>().ok());
    let mut last_progress = Instant::now();
    let mut last_seen: Option<ProbeState> = None;
    let mut samples: u64 = 0;
    let mut gone_for: u64 = 0;
    loop {
        if let Some(s) = read_probe_state() {
            samples += 1;
            let hit = match event {
                "alive" => s.heartbeat != base_heartbeat && (s.heartbeat > base_heartbeat + 3 || s.heartbeat < base_heartbeat),
                "car" => s.car != "0x0" && !s.car.is_empty() && s.car != "0x0000000000000000",
                _ => match wanted_seq {
                    Some(n) => s.cmd_seq >= n,
                    None => return Err(format!("unknown probe event '{}'", event)),
                },
            };
            last_seen = Some(s.clone());
            if hit {
                println!("  [ok] probe:{:<7} {:.1}s", event, start.elapsed().as_secs_f64());
                return Ok(s);
            }
        }
        if samples % 20 == 0 {
            if tmdrive::game_pid(host).is_none() {
                gone_for += 1;
            } else {
                gone_for = 0;
            }
            if gone_for >= crate::GONE_CHECKS_BEFORE_DEAD {
                return Err(format!("GAME EXITED while waiting for probe '{}'", event));
            }
        }
        if start.elapsed() >= timeout {
            return Err(match last_seen {
                Some(s) => format!(
                    "TIMEOUT probe '{}' after {:.0}s - heartbeat={} car={} status='{}'",
                    event, timeout.as_secs_f64(), s.heartbeat, s.car, s.status
                ),
                None => format!(
                    "TIMEOUT probe '{}' after {:.0}s - the plugin never wrote {}",
                    event, timeout.as_secs_f64(), probe_dir().join("state.json").display()
                ),
            });
        }
        if last_progress.elapsed() >= PROGRESS_EVERY {
            last_progress = Instant::now();
            match &last_seen {
                Some(s) => println!("  ... probe:{} {:.0}s (heartbeat={} car={} status='{}')", event, start.elapsed().as_secs_f64(), s.heartbeat, s.car, s.status),
                None => println!("  ... probe:{} {:.0}s (no state file yet)", event, start.elapsed().as_secs_f64()),
            }
        }
        sleep(POLL_TICK);
    }
}

pub fn send_probe_command(host: &Host, verb: &str, arg: &str, timeout: Duration) -> Result<ProbeState, String> {
    let next_seq = read_probe_state().map(|s| s.cmd_seq).unwrap_or(-1) + 1;
    let line = if arg.is_empty() { format!("{} {}", next_seq, verb) } else { format!("{} {} {}", next_seq, verb, arg) };
    fs::write(probe_dir().join("cmd.txt"), line).map_err(|e| format!("cannot write probe cmd.txt: {}", e))?;
    let s = wait_probe(host, &format!("cmd:{}", next_seq), timeout)?;
    let shown: String = s.cmd_result.chars().take(600).collect();
    println!("  -> {}", shown);
    Ok(s)
}

/// Copy the plugin sources from the repo into the game's Plugins folder.
fn install_plugins(lock: &GameLock) -> Result<(), String> {
    let _ = lock;
    let repo = std::env::current_exe()
        .ok()
        .and_then(|p| p.ancestors().nth(3).map(|a| a.to_path_buf()))
        .ok_or("cannot locate the repo from the jumprig binary")?;
    let plugins = PathBuf::from(format!("{}/Users/vjeux/OpenplanetNext/Plugins", tmdrive::drive_c()));
    for (dir, files) in [("ReactorProbe", &["Main.as", "info.toml"][..]), ("ReactorContact", &["Main.as", "Export.as", "info.toml"][..]), ("JumpButton", &["Main.as", "info.toml"][..])] {
        let src = repo.join("openplanet-plugin").join(dir);
        let dst = plugins.join(dir);
        fs::create_dir_all(&dst).map_err(|e| format!("cannot create {}: {e}", dst.display()))?;
        for f in files {
            fs::copy(src.join(f), dst.join(f)).map_err(|e| format!("cannot install {}/{f}: {e}", dir))?;
        }
        println!("  installed {dir} from {}", src.display());
    }
    Ok(())
}

/// Hold the accelerator for `ms` while printing every change of the probe's
/// reactor block (type/lvl/duration, and whether `contact` is moving).
/// What the Reactor Contact plugin said during a drive, sampled at 10 Hz next
/// to the probe: (samples, samples resolved to the hook's car, samples
/// touching, samples where its contact word == the probe's, samples touching
/// while the probe's contact word was moving).
#[derive(Debug, Clone, Default)]
struct ContactTally {
    /// (GameTime, touching) of every sample, judged later against the probe's per-frame log
    seen: Vec<(i64, bool)>,
    samples: u64,
    resolved_to_car: u64,
    touching: u64,
    same_contact: u64,
    touching_while_moving: u64,
    touching_while_still: u64,
}

fn drive_and_watch(lock: &GameLock, label: &str, ms: u64, car: &str) -> Result<ContactTally, String> {
    let h = lock.host();
    println!("--- {label}: accelerator for {:.1} s ---", ms as f64 / 1000.0);
    let drive_lock = tmdrive::acquire(h.clone(), "reactortest: accelerator").map_err(|e| e.to_string())?;
    let driver = std::thread::spawn(move || ops::accelerate(&drive_lock, ms));
    let start = Instant::now();
    let mut last: Option<ProbeState> = None;
    let mut moving_since: Option<Instant> = None;
    let mut tally = ContactTally::default();
    let mut last_touching: Option<bool> = None;
    while start.elapsed() < Duration::from_millis(ms + 1500) {
        if let Some(s) = read_probe_state() {
            let z = read_state().map(|j| j.pos[2]).unwrap_or(f64::NAN);
            let moved_now = last.as_ref().map(|l| l.contact != s.contact).unwrap_or(false);
            let changed = match &last {
                Some(l) => l.kind != s.kind || l.lvl != s.lvl || l.duration != s.duration || moved_now != moving_since.is_some(),
                None => true,
            };
            if moved_now {
                if moving_since.is_none() {
                    moving_since = Some(Instant::now());
                }
            } else if moving_since.map(|t| t.elapsed() > Duration::from_millis(600)).unwrap_or(false) {
                moving_since = None;
            }
            // the deliverable's own verdict on the same frame
            let mut plugin_note = String::new();
            if let Some((resolved, pcar, touching, contact, gt)) = read_contact_state() {
                tally.samples += 1;
                tally.seen.push((gt, touching));
                if resolved && pcar.eq_ignore_ascii_case(car) {
                    tally.resolved_to_car += 1;
                }
                if touching {
                    tally.touching += 1;
                    if moved_now { tally.touching_while_moving += 1 } else { tally.touching_while_still += 1 }
                }
                if contact == s.contact {
                    tally.same_contact += 1;
                }
                if last_touching != Some(touching) {
                    plugin_note = format!("  | ReactorContact: {}", if touching { "TOUCHING" } else { "not touching" });
                    last_touching = Some(touching);
                }
            }
            if changed || !plugin_note.is_empty() {
                println!(
                    "  {:5.1}s z={:7.1} game={} type={} lvl={} contact={} act={} start={} dur={} vis(lvl {} type {} final {:.2}) {}{}",
                    start.elapsed().as_secs_f64(), z, s.game_time, s.kind, s.lvl, s.contact, s.activation, s.start, s.duration,
                    s.vis_lvl, s.vis_type, s.final_timer,
                    if moving_since.is_some() { "CONTACT (moving)" } else { "" }, plugin_note
                );
            }
            last = Some(s);
        }
        sleep(Duration::from_millis(100));
    }
    driver.join().map_err(|_| "driver thread panicked".to_string())?.map_err(|e| e.to_string())?;
    println!(
        "  ReactorContact during {label}: {} samples, {} resolved to the hook's car, {} touching ({} while the probe's word moved, {} while it stood still), contact word equal on {}",
        tally.samples, tally.resolved_to_car, tally.touching, tally.touching_while_moving, tally.touching_while_still, tally.same_contact
    );
    Ok(tally)
}

#[derive(Debug, Clone)]
struct Row {
    game_time: i64,
    race_time: i64,
    kind: u32,
    lvl: u32,
    contact: u32,
    activation: u32,
    start: u32,
    duration: u32,
    vis_lvl: i64,
    vis_type: i64,
    final_timer: f64,
    z: f64,
    y: f64,
}

fn parse_log(text: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.starts_with("now_ms") || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 17 {
            continue;
        }
        let p = |i: usize| f[i].trim().parse::<f64>().unwrap_or(f64::NAN);
        rows.push(Row {
            game_time: p(1) as i64,
            race_time: p(2) as i64,
            kind: p(4) as u32,
            lvl: p(5) as u32,
            contact: p(6) as u32,
            activation: p(7) as u32,
            start: p(8) as u32,
            duration: p(9) as u32,
            vis_lvl: p(10) as i64,
            vis_type: p(11) as i64,
            final_timer: p(12),
            y: p(14),
            z: p(15),
        });
    }
    rows
}

fn reactor_cell_at(z: f64) -> Option<(i32, u32, &'static str)> {
    let cz = (z / CELL).floor() as i32;
    REACTOR_CELLS.iter().copied().find(|(c, _, _)| *c == cz)
}

/// Near a reactor cell: inside it or within 6 m of its edges (the car is 4 m
/// long and the vis position is the body's centre).
fn near_reactor(z: f64) -> bool {
    REACTOR_CELLS.iter().any(|(c, _, _)| {
        let lo = *c as f64 * CELL - 6.0;
        let hi = (*c as f64 + 1.0) * CELL + 6.0;
        z >= lo && z <= hi
    })
}

#[derive(Debug, Clone)]
struct Episode {
    first: usize,
    last: usize,
    frames: usize,
}

/// Runs of frames whose `contact` moved; a single quiet frame inside a run
/// (a render frame that saw no new physics tick) does not end it.
fn episodes(rows: &[Row]) -> Vec<Episode> {
    let mut out: Vec<Episode> = Vec::new();
    let mut cur: Option<Episode> = None;
    let mut quiet = 0usize;
    for i in 1..rows.len() {
        let moved = rows[i].contact != rows[i - 1].contact;
        match (&mut cur, moved) {
            (None, true) => {
                cur = Some(Episode { first: i, last: i, frames: 1 });
                quiet = 0;
            }
            (Some(e), true) => {
                e.last = i;
                e.frames += 1;
                quiet = 0;
            }
            (Some(_), false) => {
                quiet += 1;
                if quiet > 1 {
                    out.push(cur.take().unwrap());
                }
            }
            (None, false) => {}
        }
    }
    if let Some(e) = cur {
        out.push(e);
    }
    out
}

/// The verdict on one log (one car object, one or more drives).
fn judge(label: &str, rows: &[Row]) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();
    if rows.len() < 50 {
        return Err(format!("{label}: only {} rows in the log", rows.len()));
    }
    let eps = episodes(rows);
    // the drive itself, one line per second: where the car was and what the block said
    let mut next_t = rows[0].game_time;
    for r in rows {
        if r.game_time >= next_t {
            println!(
                "    t={:6} z={:7.1} y={:6.2} type={} lvl={} contact={} dur={} vis lvl {} final {:.2}",
                r.game_time, r.z, r.y, r.kind, r.lvl, r.contact, r.duration, r.vis_lvl, r.final_timer
            );
            next_t += 1000;
        }
    }
    println!("  {label}: {} rows, game time {}..{}, {} contact episode(s)", rows.len(), rows[0].game_time, rows[rows.len() - 1].game_time, eps.len());
    let mut good = 0;
    for e in &eps {
        let a = &rows[e.first];
        let b = &rows[e.last];
        // the reactor cell most of the episode's frames sit in (the first and
        // last frames straddle the pad's edges: the 13:44 run's episode ran
        // z 189.8..256.8 over the 192..256 pads)
        let cell = {
            let mut counts: Vec<((i32, u32, &'static str), usize)> = Vec::new();
            for r in &rows[e.first..=e.last] {
                if let Some(c) = reactor_cell_at(r.z) {
                    match counts.iter_mut().find(|(k, _)| k.0 == c.0) {
                        Some((_, n)) => *n += 1,
                        None => counts.push((c, 1)),
                    }
                }
            }
            counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            counts.first().filter(|(_, n)| *n * 2 >= e.frames).map(|(c, _)| *c)
        };
        // contact tracks the playground clock, frame by frame
        let mut max_lag: i64 = 0;
        let mut ahead = 0;
        for r in &rows[e.first..=e.last] {
            let lag = r.game_time - r.contact as i64;
            if lag < 0 {
                ahead += 1;
            }
            max_lag = max_lag.max(lag.abs());
        }
        let ok = cell.is_some() && max_lag <= 40 && ahead == 0 && b.duration == 6000;
        if ok {
            good += 1;
        }
        println!(
            "    episode game {}..{} ({} ms, {} frames) z {:.1}..{:.1} -> {} | type {} lvl {} dur {} | contact-vs-GameTime lag max {} ms{} | vis lvl {} type {} final {:.2}{}",
            a.game_time, b.game_time, b.game_time - a.game_time, e.frames, a.z, b.z,
            cell.map(|c| format!("{} (cell {})", c.2, c.0)).unwrap_or_else(|| "NO REACTOR CELL".into()),
            b.kind, b.lvl, b.duration, max_lag, if ahead > 0 { format!(" ({ahead} frames with contact > GameTime)") } else { String::new() },
            b.vis_lvl, b.vis_type, b.final_timer, if ok { "  OK" } else { "  ??" }
        );
        if let Some(c) = cell {
            if b.lvl != c.1 {
                problems.push(format!("{label}: {} gave level {} instead of {}", c.2, b.lvl, c.1));
            }
        }
        if e.frames >= 3 && cell.is_none() {
            problems.push(format!("{label}: contact moved for {} frames at z {:.1} with no reactor cell there", e.frames, a.z));
        }
    }
    // a moving `contact` away from every reactor cell = a refresh with no contact
    let stray: usize = (1..rows.len()).filter(|&i| rows[i].contact != rows[i - 1].contact && !near_reactor(rows[i].z) && !near_reactor(rows[i - 1].z)).count();
    println!("    frames where contact moved away from every reactor cell: {stray}");
    if stray > 2 {
        problems.push(format!("{label}: contact moved on {stray} frames with the car away from every reactor cell"));
    }
    // once touched, 6000 stays until the object is reset
    if let Some(first_touch) = rows.iter().position(|r| r.contact != 0) {
        let dur_ok = rows[first_touch..].iter().all(|r| r.duration == 6000 || r.duration == 0);
        let zeros = rows[first_touch..].iter().filter(|r| r.duration == 0).count();
        println!("    duration after the first contact: 6000 everywhere{}", if zeros > 0 { format!(" except {zeros} frames at 0 (a reset)") } else { String::new() });
        if !dur_ok {
            problems.push(format!("{label}: duration took a value other than 6000/0 after the first contact"));
        }
    } else {
        problems.push(format!("{label}: the car never touched a reactor"));
    }
    if good == 0 {
        problems.push(format!("{label}: no contact episode over a reactor cell with contact tracking GameTime"));
    }
    let _ = rows.iter().map(|r| (r.race_time, r.activation, r.start, r.y)).count();
    if problems.is_empty() { Ok(()) } else { Err(problems.join("\n")) }
}

pub fn reactor_test(lock: &GameLock, map: &str, install: bool) -> Result<(), String> {
    let h = lock.host();
    let run_dir = PathBuf::from(format!("/home/vjeux/reactor/run-{}", chrono_stamp()));
    fs::create_dir_all(&run_dir).map_err(|e| format!("cannot create {}: {e}", run_dir.display()))?;
    if install {
        println!("=== 0. install the plugins from the repo ===");
        install_plugins(lock)?;
    }
    println!("=== 1. a fresh game, probe + hook alive ===");
    if tmdrive::game_pid(h).is_some() {
        println!("  a game is running from a previous hold - stopping it");
        ops::kill(lock).map_err(|e| e.to_string())?;
        wait_for(h, "exit", Duration::from_secs(60))?;
    }
    let _ = fs::remove_file(probe_dir().join("reactor.log"));
    let _ = fs::remove_file(probe_dir().join("discovery.txt"));
    fs::create_dir_all(contact_dir()).ok();
    fs::write(contact_dir().join("automation.on"), "reactortest\n").ok();
    launch_and_hook(lock, Duration::from_secs(300))?;
    let a = match wait_probe(h, "alive", Duration::from_secs(60)) {
        Ok(a) => a,
        Err(e) => {
            let log = fs::read_to_string(format!("{}/Users/vjeux/OpenplanetNext/Openplanet.log", tmdrive::drive_c())).unwrap_or_default();
            let errs: Vec<&str> = log.lines().filter(|l| l.contains("ReactorProbe") && (l.contains("ERR") || l.contains("error"))).collect();
            if !errs.is_empty() {
                return Err(format!("ReactorProbe did not compile:\n{}", errs.join("\n")));
            }
            return Err(e);
        }
    };
    println!("  probe alive: {}", a.status);
    // the deliverable must have compiled too, or the run proves nothing about it
    {
        let log = fs::read_to_string(format!("{}/Users/vjeux/OpenplanetNext/Openplanet.log", tmdrive::drive_c())).unwrap_or_default();
        let errs: Vec<&str> = log.lines().filter(|l| l.contains("ReactorContact") && l.contains(" ERR ")).collect();
        if !errs.is_empty() {
            return Err(format!("ReactorContact did not compile:\n{}", errs.join("\n")));
        }
        println!("  ReactorContact compiled ({} log lines mention it)", log.lines().filter(|l| l.contains("ReactorContact")).count());
    }

    println!("=== 2. into the reactor map ===");
    enter_map(lock, map)?;
    wait_for(h, "grounded", Duration::from_secs(60))?;
    let car = read_state().map(|s| s.car).unwrap_or_default();
    println!("  hook's car pointer {car}");

    println!("=== 3. the hook-free way first: the probe picks the car from the CSmPlayer alone ===");
    send_probe_command(h, "reset", "", Duration::from_secs(10))?;
    let s = send_probe_command(h, "scan", "", Duration::from_secs(30))?;
    let scan_ok = s.car.eq_ignore_ascii_case(&car);
    println!("  probe's own pick {} == hook's car: {scan_ok}", s.car);
    fs::write(run_dir.join("scan-1.txt"), &s.cmd_result).ok();

    println!("=== 4. hand the hook's car to the probe; where do CSmPlayer / vis point at it? ===");
    let r = send_probe_command(h, "car", &car, Duration::from_secs(20))?;
    if !r.car.eq_ignore_ascii_case(&car) {
        return Err(format!("the probe refused the car: {}", r.status));
    }
    let disc = fs::read_to_string(probe_dir().join("discovery.txt")).unwrap_or_default();
    for l in disc.lines().filter(|l| l.contains("->") || l.contains("= vis Position") || l.starts_with("car") || l.starts_with("CSm") || l.contains("hit(s)")) {
        println!("  {l}");
    }
    fs::write(run_dir.join("discovery-1.txt"), &disc).ok();

    println!("=== 5. drive A: pads, Boost2 pad, the ring gates ===");
    let tally_a = drive_and_watch(lock, "drive A", 14000, &car)?;
    let after_a = read_probe_state().ok_or("no probe state after drive A")?;
    print_probe_state(&after_a);
    send_probe_command(h, "flush", "", Duration::from_secs(10))?;
    let log1 = fs::read_to_string(probe_dir().join("reactor.log")).unwrap_or_default();
    fs::write(run_dir.join("reactor-1.log"), &log1).ok();

    println!("=== 6. restart the map: a new car object, found again without the hook ===");
    let r = send_command(h, "restartmap", "", Duration::from_secs(10))?;
    if !r.state.cmd_result.contains("requested") {
        return Err(format!("restart refused: {}", r.state.cmd_result));
    }
    wait_for(h, "in-map", Duration::from_secs(60))?;
    wait_for(h, "car", Duration::from_secs(60))?;
    wait_for(h, "ticking", Duration::from_secs(30))?;
    let _ = wait_for(h, "stopped", Duration::from_secs(30));
    let car2 = read_state().map(|s| s.car).unwrap_or_default();
    println!("  hook's car pointer now {car2} (was {car})");
    // the probe drops a car whose object went away only when the signature
    // breaks; a rebuilt playground keeps the old bytes readable, so say so
    send_probe_command(h, "drop", "", Duration::from_secs(10))?;
    send_probe_command(h, "reset", "", Duration::from_secs(10))?;
    let s = send_probe_command(h, "scan", "", Duration::from_secs(30))?;
    let scan2_ok = s.car.eq_ignore_ascii_case(&car2);
    println!("  probe's own pick {} == hook's new car: {scan2_ok}", s.car);
    fs::write(run_dir.join("scan-2.txt"), &s.cmd_result).ok();
    let r = send_probe_command(h, "car", &car2, Duration::from_secs(20))?;
    if !r.car.eq_ignore_ascii_case(&car2) {
        return Err(format!("the probe refused the new car: {}", r.status));
    }
    let disc2 = fs::read_to_string(probe_dir().join("discovery.txt")).unwrap_or_default();
    fs::write(run_dir.join("discovery-2.txt"), &disc2).ok();
    for l in disc2.lines().filter(|l| l.contains("-> car") || l.contains("-> CSceneVehicleVis") || l.contains("-> CSmPlayer")) {
        println!("  {l}");
    }
    let tally_c = drive_and_watch(lock, "drive C", 14000, &car2)?;
    send_probe_command(h, "flush", "", Duration::from_secs(10))?;
    let log2 = fs::read_to_string(probe_dir().join("reactor.log")).unwrap_or_default();
    fs::write(run_dir.join("reactor-2.log"), &log2).ok();

    println!("=== 7. verdict ===");
    let rows1 = parse_log(&log1);
    let rows2 = parse_log(&log2);
    let mut problems: Vec<String> = Vec::new();
    if let Err(e) = judge("drive A", &rows1) {
        problems.push(e);
    }
    if let Err(e) = judge("drive C (new car)", &rows2) {
        problems.push(e);
    }
    // the same pointer offsets in both discoveries = a stable path, not luck
    let links = |d: &str| -> Vec<String> {
        d.lines().filter(|l| l.contains("-> car") && (l.trim_start().starts_with("CSmPlayer+") || l.trim_start().starts_with("CSceneVehicleVis+") || l.trim_start().starts_with("CSmScriptPlayer+"))).map(|l| l.trim().to_string()).collect()
    };
    let l1 = links(&disc);
    let l2 = links(&disc2);
    println!("  pointers to the car, first object:  {}", if l1.is_empty() { "none".to_string() } else { l1.join(" | ") });
    println!("  pointers to the car, second object: {}", if l2.is_empty() { "none".to_string() } else { l2.join(" | ") });
    if l1.is_empty() {
        problems.push("no Openplanet-reachable object holds a pointer to the car".into());
    } else if l1 != l2 {
        problems.push("the pointer offsets differ between the two car objects".into());
    }
    if !scan_ok || !scan2_ok {
        problems.push(format!("the signature scan did not name the hook's car (first {scan_ok}, second {scan2_ok})"));
    }
    // the deliverable: resolved to the right car the whole time, touching only
    // while the probe saw the word move (a 10 Hz sample may straddle the edge)
    for (label, t, rows) in [("drive A", &tally_a, &rows1), ("drive C", &tally_c, &rows2)] {
        if t.samples < 50 {
            problems.push(format!("ReactorContact wrote only {} state samples during {label}", t.samples));
            continue;
        }
        if t.resolved_to_car + 2 < t.samples {
            problems.push(format!("ReactorContact resolved to the hook's car on only {} of {} samples during {label}", t.resolved_to_car, t.samples));
        }
        if t.touching < 5 {
            problems.push(format!("ReactorContact reported touching on only {} samples during {label}", t.touching));
        }
        // Each 10 Hz sample against the probe's per-frame log: a "touching" sample
        // must fall inside a contact episode (30 ms slack at the edges), and a
        // sample well inside an episode must say touching. (The probe's own
        // state file is 4 Hz, too stale to judge with — the 14:06 run's false FAIL.)
        let eps = episodes(rows);
        let spans: Vec<(i64, i64)> = eps.iter().map(|e| (rows[e.first].game_time, rows[e.last].game_time)).collect();
        let mut false_touch = 0;
        let mut missed = 0;
        for &(gt, touching) in &t.seen {
            let inside = spans.iter().any(|(a, b)| gt >= a - 30 && gt <= b + 30);
            let deep = spans.iter().any(|(a, b)| gt >= a + 60 && gt <= b - 60);
            if touching && !inside {
                false_touch += 1;
            }
            if !touching && deep {
                missed += 1;
            }
        }
        println!(
            "  ReactorContact vs the probe log, {label}: {} touching samples, {} outside every contact episode, {} not-touching samples deep inside one",
            t.touching, false_touch, missed
        );
        if false_touch > 1 {
            problems.push(format!("ReactorContact said touching on {false_touch} samples outside every contact episode ({label})"));
        }
        if missed > 1 {
            problems.push(format!("ReactorContact missed {missed} samples deep inside a contact episode ({label})"));
        }
    }
    println!("  evidence in {}", run_dir.display());
    if problems.is_empty() {
        println!("PASS: car+0x13B0 moves every tick over a reactor surface and only there; 6000 at +0x13BC; reachable from the CSmPlayer; the Reactor Contact plugin agrees");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn chrono_stamp() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{t}")
}
