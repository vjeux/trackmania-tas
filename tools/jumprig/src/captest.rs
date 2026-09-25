//! `jumprig captest` — proves the Speed Cap plugin against the game's own
//! physics state, under the lock, event by event.
//!
//! The probe is the Jump Button's `setspeed`: it rescales the car's velocity
//! to a given magnitude. `NSceneVehiclePhy::ComputeForces` then clamps it to
//! the model's MaxSpeed on the very next physics tick, so the magnitude read
//! back a few ticks later says whether the cap is in force:
//!
//!   stock      setspeed 300 m/s  ->  277.8 m/s (1000 km/h) within a tick
//!   unlimited  setspeed 300 m/s  ->  ~300 m/s, only drag eating at it
//!
//! Before either phase the harness asks the plugin to compare the params
//! pointer the physics reads through `car+0x88` with the model the plugin
//! writes: a DIFFERENT answer would mean the write never reaches the car and
//! the whole approach is wrong, so it fails loudly there rather than in a
//! confusing phase B.

use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tmdrive::{ops, GameLock, Host};

use crate::{
    bool_after, enter_map, launch_and_hook, num_after, read_state, send_command, str_after,
    wait_for, POLL_TICK, PROGRESS_EVERY,
};

fn speedcap_dir() -> PathBuf {
    PathBuf::from(format!(
        "{}/Users/vjeux/OpenplanetNext/PluginStorage/SpeedCap",
        tmdrive::drive_c()
    ))
}

#[derive(Debug, Clone, Default)]
pub struct CapState {
    pub heartbeat: u64,
    pub model_found: bool,
    pub signature_ok: bool,
    pub original_mps: f64,
    pub current_mps: f64,
    pub current_kmh: f64,
    pub desired_mps: f64,
    pub unlimited: bool,
    pub applied: u64,
    pub reapplied: u64,
    pub model_changes: u64,
    pub cmd_seq: i64,
    pub cmd_result: String,
    pub status: String,
}

/// A torn read (the plugin rewrites the file at 4 Hz) is retried by the poll
/// loop, never reported.
pub fn read_cap_state() -> Option<CapState> {
    let raw = fs::read_to_string(speedcap_dir().join("state.json")).ok()?;
    if !raw.trim_end().ends_with('}') {
        return None;
    }
    Some(CapState {
        heartbeat: num_after(&raw, "heartbeat")? as u64,
        model_found: bool_after(&raw, "model_found")?,
        signature_ok: bool_after(&raw, "signature_ok")?,
        original_mps: num_after(&raw, "original_mps")?,
        current_mps: num_after(&raw, "current_mps")?,
        current_kmh: num_after(&raw, "current_kmh")?,
        desired_mps: num_after(&raw, "desired_mps")?,
        unlimited: bool_after(&raw, "unlimited")?,
        applied: num_after(&raw, "applied")? as u64,
        reapplied: num_after(&raw, "reapplied")? as u64,
        model_changes: num_after(&raw, "model_changes")? as u64,
        cmd_seq: num_after(&raw, "cmd_seq").unwrap_or(-1.0) as i64,
        cmd_result: str_after(&raw, "cmd_result").unwrap_or_default(),
        status: str_after(&raw, "status").unwrap_or_default(),
    })
}

pub fn print_cap_state(s: &CapState) {
    println!("heartbeat      {}", s.heartbeat);
    println!("model found    {} (signature ok: {})", s.model_found, s.signature_ok);
    println!("original       {:.4} m/s ({:.1} km/h)", s.original_mps, s.original_mps * 3.6);
    println!("current        {:.4} m/s ({:.1} km/h)", s.current_mps, s.current_kmh);
    println!("desired        {:.4} m/s (unlimited={})", s.desired_mps, s.unlimited);
    println!("writes         {} asked, {} re-applied, model rebuilt {}x", s.applied, s.reapplied, s.model_changes);
    println!("last command   #{} -> {}", s.cmd_seq, s.cmd_result);
    println!("status         {}", s.status);
}

/// The Speed Cap side of `wait_for`: `alive` (heartbeat moving), `model`
/// (found and signature-checked), `cmd:N` (command N acted on). Same rules:
/// a baseline is snapshotted, a dead game fails fast, the timeout names the
/// unmet condition.
pub fn wait_cap(host: &Host, event: &str, timeout: Duration) -> Result<CapState, String> {
    let start = Instant::now();
    let base_heartbeat = read_cap_state().map(|s| s.heartbeat).unwrap_or(0);
    let wanted_seq: Option<i64> = event.strip_prefix("cmd:").and_then(|n| n.parse::<i64>().ok());
    let mut last_progress = Instant::now();
    let mut last_seen: Option<CapState> = None;
    let mut samples: u64 = 0;
    let mut gone_for: u64 = 0;
    loop {
        if let Some(s) = read_cap_state() {
            samples += 1;
            // A model whose neighbours do not match is a verdict, not a wait.
            if event == "model" && s.model_found && !s.signature_ok && s.status.contains("signature") {
                return Err(format!("the plugin refused the model: {}", s.status));
            }
            let hit = match event {
                "alive" => s.heartbeat != base_heartbeat
                    && (s.heartbeat > base_heartbeat + 3 || s.heartbeat < base_heartbeat),
                "model" => s.model_found && s.signature_ok,
                _ => match wanted_seq {
                    Some(n) => s.cmd_seq >= n,
                    None => return Err(format!("unknown speedcap event '{}'", event)),
                },
            };
            last_seen = Some(s.clone());
            if hit {
                println!("  [ok] cap:{:<7} {:.1}s", event, start.elapsed().as_secs_f64());
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
                return Err(format!("GAME EXITED while waiting for speedcap '{}'", event));
            }
        }
        if start.elapsed() >= timeout {
            return Err(match last_seen {
                Some(s) => format!(
                    "TIMEOUT speedcap '{}' after {:.0}s - heartbeat={} model={} sig={} current={:.1} km/h status='{}'",
                    event, timeout.as_secs_f64(), s.heartbeat, s.model_found, s.signature_ok, s.current_kmh, s.status
                ),
                None => format!(
                    "TIMEOUT speedcap '{}' after {:.0}s - the plugin never wrote {}",
                    event, timeout.as_secs_f64(), speedcap_dir().join("state.json").display()
                ),
            });
        }
        if last_progress.elapsed() >= PROGRESS_EVERY {
            last_progress = Instant::now();
            match &last_seen {
                Some(s) => println!(
                    "  ... cap:{} {:.0}s (heartbeat={} model={} current={:.1} km/h)",
                    event, start.elapsed().as_secs_f64(), s.heartbeat, s.model_found, s.current_kmh
                ),
                None => println!("  ... cap:{} {:.0}s (no state file yet)", event, start.elapsed().as_secs_f64()),
            }
        }
        sleep(POLL_TICK);
    }
}

pub fn send_cap_command(host: &Host, verb: &str, arg: &str, timeout: Duration) -> Result<CapState, String> {
    let next_seq = read_cap_state().map(|s| s.cmd_seq).unwrap_or(-1) + 1;
    let line = if arg.is_empty() { format!("{} {}", next_seq, verb) } else { format!("{} {} {}", next_seq, verb, arg) };
    fs::write(speedcap_dir().join("cmd.txt"), line).map_err(|e| format!("cannot write speedcap cmd.txt: {}", e))?;
    let s = wait_cap(host, &format!("cmd:{}", next_seq), timeout)?;
    println!("  -> {}", s.cmd_result);
    Ok(s)
}

fn speed_of(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// One phase: get the car moving, rescale its velocity to `poke_mps`, let
/// ten physics ticks run (the clamp, if any, is in the first one), and read
/// the magnitude back. Returns (speed after ten ticks, peak speed seen).
fn poke(lock: &GameLock, poke_mps: f64) -> Result<(f64, f64), String> {
    let h = lock.host();
    // The accelerator gives the car a direction to scale; it runs on its own
    // thread under a nested hold so the poke keeps its timing.
    let drive_lock = tmdrive::acquire(h.clone(), "captest: accelerator").map_err(|e| e.to_string())?;
    // Six seconds: long enough to outlast the start countdown after a give-up
    // and still be pressed when the car is released.
    let driver = std::thread::spawn(move || ops::accelerate(&drive_lock, 6000));
    wait_for(h, "moving", Duration::from_secs(15))?;
    let r = send_command(h, "setspeed", &format!("{poke_mps}"), Duration::from_secs(10))?;
    if !r.state.cmd_result.starts_with("setspeed: |v|") {
        let _ = driver.join();
        return Err(format!("setspeed refused: {}", r.state.cmd_result));
    }
    // Ten more physics ticks (100 ms) after the write: whatever ComputeForces
    // does to an over-limit velocity, it has done by then.
    let o = wait_for(h, "ticking", Duration::from_secs(5))?;
    let after = speed_of(o.state.vel);
    let _ = driver.join().map_err(|_| "driver thread panicked".to_string())?;
    Ok((after, o.max_speed))
}

/// Restart the run (the game's own give-up, through the playground API) and
/// wait for the car to be standing on its wheels at the start again. A poke
/// leaves the car somewhere off the track; the next phase needs a fresh one.
fn back_to_start(lock: &GameLock) -> Result<(), String> {
    let h = lock.host();
    let r = send_command(h, "restartmap", "", Duration::from_secs(10))?;
    if !r.state.cmd_result.contains("requested") {
        return Err(format!("restart refused: {}", r.state.cmd_result));
    }
    // The playground is rebuilt: a new car object, then physics ticking,
    // then the car standing still at the start; the start countdown is
    // waited out by the next phase's accelerator hold.
    wait_for(h, "in-map", Duration::from_secs(60))?;
    wait_for(h, "car", Duration::from_secs(60))?;
    wait_for(h, "ticking", Duration::from_secs(30))?;
    match wait_for(h, "stopped", Duration::from_secs(30)) {
        Ok(_) => Ok(()),
        // Not fatal: the next phase only needs a car with a direction, which
        // a moving or falling car also has.
        Err(e) => {
            println!("  (respawn did not settle the car: {e})");
            Ok(())
        }
    }
}

pub fn cap_test(lock: &GameLock, map: &str) -> Result<(), String> {
    let h = lock.host();
    println!("=== 1. a fresh game, both plugins alive ===");
    // A game left running by the previous holder loaded whatever SpeedCap
    // source it found at ITS start, and a plugin that failed to compile then
    // is never reloaded (a file written from outside did not trigger the
    // developer-mode reload either, 2026-09-24). A verification run must
    // know which source it is testing, so it always starts its own game.
    if tmdrive::game_pid(h).is_some() {
        println!("  a game is running from a previous hold - stopping it");
        ops::kill(lock).map_err(|e| e.to_string())?;
        wait_for(h, "exit", Duration::from_secs(60))?;
    }
    launch_and_hook(lock, Duration::from_secs(300))?;
    let a = wait_cap(h, "alive", Duration::from_secs(60))?;
    println!("  speed cap plugin alive: {}", a.status);

    println!("=== 2. into a map ===");
    enter_map(lock, map)?;
    wait_for(h, "grounded", Duration::from_secs(60))?;
    let car = read_state().map(|s| s.car).unwrap_or_default();
    println!("  car {car}");
    // What the physics reads, before anything else: the params object behind
    // car+0x88, its dwords around the MaxSpeed slot, and whether the catalog
    // model is that object or merely points at it.
    let p = send_cap_command(h, "probe", &car, Duration::from_secs(10))?;
    println!("  {}", p.cmd_result);
    // The catalog article is looked up from inside the map: the plugin keeps
    // retrying at 4 Hz, and a menu-time miss is reported by its status.
    let m = wait_cap(h, "model", Duration::from_secs(120))?;
    println!(
        "  physics model found: original {:.4} m/s ({:.1} km/h), currently {:.1} km/h - {}",
        m.original_mps, m.original_mps * 3.6, m.current_kmh, m.status
    );
    if (m.original_mps - 277.7778).abs() > 0.001 {
        return Err(format!("the original MaxSpeed is not 1000 km/h but {:.4} m/s", m.original_mps));
    }

    println!("=== 3. does the car read the model we write? ===");
    let c = send_cap_command(h, "carcheck", &car, Duration::from_secs(10))?;
    if !c.cmd_result.contains(" SAME") {
        return Err(format!("the car's params are not the catalog model: {}", c.cmd_result));
    }

    // Phases 4 and 5 run inside a closure so that, whatever the verdict, the
    // shared box is left with stock physics afterwards. Whoever wants the
    // cap gone enables the plugin's setting.
    let verdict = (|| -> Result<(f64, f64), String> {
        println!("=== 4. stock limit: the poke must be clamped ===");
        let s = send_cap_command(h, "stock", "", Duration::from_secs(10))?;
        if (s.current_kmh - 1000.0).abs() > 0.5 {
            return Err(format!("could not set the stock limit: current {:.1} km/h ({})", s.current_kmh, s.status));
        }
        let (after_stock, peak_stock) = poke(lock, 300.0)?;
        println!(
            "  stock: 300 m/s poke -> {:.2} m/s ({:.0} km/h) after 10 ticks, peak seen {:.2} m/s",
            after_stock, after_stock * 3.6, peak_stock
        );
        if after_stock > 278.5 {
            return Err(format!("THE STOCK CAP DID NOT CLAMP: {:.2} m/s after the poke (probe broken?)", after_stock));
        }
        back_to_start(lock)?;

        println!("=== 5. unlimited: the poke must survive ===");
        let u = send_cap_command(h, "unlimited", "", Duration::from_secs(10))?;
        if u.current_mps < 1.0e5 {
            return Err(format!("could not remove the limit: current {:.1} m/s ({})", u.current_mps, u.status));
        }
        let (after_free, peak_free) = poke(lock, 300.0)?;
        println!(
            "  unlimited: 300 m/s poke -> {:.2} m/s ({:.0} km/h) after 10 ticks, peak seen {:.2} m/s",
            after_free, after_free * 3.6, peak_free
        );
        back_to_start(lock)?;
        if after_free < 285.0 {
            return Err(format!("THE CAP IS STILL THERE: {:.2} m/s after the poke with the limit removed", after_free));
        }
        Ok((after_stock, after_free))
    })();

    match send_cap_command(h, "stock", "", Duration::from_secs(10)) {
        Ok(back) => println!("  left the limit at {:.1} km/h", back.current_kmh),
        Err(e) => println!("  WARNING: could not put the limit back to stock: {e}"),
    }
    let (after_stock, after_free) = verdict?;

    println!(
        "RESULT: speed cap removed - stock clamps 300 -> {:.1} m/s, unlimited keeps {:.1} m/s ({:.0} km/h)",
        after_stock, after_free, after_free * 3.6
    );
    Ok(())
}
