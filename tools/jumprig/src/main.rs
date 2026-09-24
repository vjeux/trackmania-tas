//! `jumprig` — the event-driven test harness for the Jump Button plugin.
//!
//! Two rules this program exists to demonstrate, both learned the hard way on
//! 2026-09-23:
//!
//! 1. **`wait_for(event, timeout)` is the only synchronisation primitive**, and
//!    the single poll tick inside it is the only sleep in the system. Nothing
//!    waits a guessed duration or samples for a fixed window. Every wait is
//!    named, snapshots a baseline so a stale file cannot satisfy it, aborts the
//!    moment the game dies, and says exactly which condition was unmet.
//!
//! 2. **The game is driven only under the lock.** This harness once launched
//!    the game over a lock held by another session and spent an hour blaming
//!    the resulting chaos on its own hook. It now takes [`tmdrive`]'s guard
//!    like every other driver.

use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tmdrive::{ops, Error, GameLock, Host};

const POLL_TICK: Duration = Duration::from_millis(50);
const PROGRESS_EVERY: Duration = Duration::from_secs(5);

fn storage_dir() -> PathBuf {
    PathBuf::from(format!(
        "{}/Users/vjeux/OpenplanetNext/PluginStorage/JumpButton",
        tmdrive::drive_c()
    ))
}
fn state_path() -> PathBuf {
    storage_dir().join("state.json")
}
fn cmd_path() -> PathBuf {
    storage_dir().join("cmd.txt")
}

#[derive(Debug, Clone, Default)]
struct State {
    heartbeat: u64,
    build_supported: bool,
    hooked: bool,
    hook_ticks: u64,
    in_playground: bool,
    car_valid: bool,
    car: String,
    pos: [f64; 3],
    vel: [f64; 3],
    wheels_down: i64,
    jumps: u64,
    cmd_seq: i64,
    cmd_result: String,
    last_jump: String,
    status: String,
}

fn num_after(s: &str, key: &str) -> Option<f64> {
    let k = format!("\"{}\":", key);
    let i = s.find(&k)? + k.len();
    let rest = s[i..].trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.' || c == 'e' || c == '+'))
        .unwrap_or(rest.len());
    rest[..end].parse::<f64>().ok()
}
fn bool_after(s: &str, key: &str) -> Option<bool> {
    let k = format!("\"{}\":", key);
    let i = s.find(&k)? + k.len();
    let rest = s[i..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}
fn str_after(s: &str, key: &str) -> Option<String> {
    let k = format!("\"{}\":", key);
    let i = s.find(&k)? + k.len();
    let rest = s[i..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
fn vec3_after(s: &str, key: &str) -> Option<[f64; 3]> {
    let k = format!("\"{}\":", key);
    let i = s.find(&k)? + k.len();
    let rest = s[i..].trim_start();
    if !rest.starts_with('[') {
        return None;
    }
    let end = rest.find(']')?;
    let parts: Vec<f64> =
        rest[1..end].split(',').filter_map(|p| p.trim().parse::<f64>().ok()).collect();
    if parts.len() != 3 {
        return None;
    }
    Some([parts[0], parts[1], parts[2]])
}

/// A torn read is normal (the plugin rewrites this every frame) and is retried
/// by the poll loop, never reported as a failure.
fn read_state() -> Option<State> {
    let raw = fs::read_to_string(state_path()).ok()?;
    if !raw.trim_end().ends_with('}') {
        return None;
    }
    Some(State {
        heartbeat: num_after(&raw, "heartbeat")? as u64,
        build_supported: bool_after(&raw, "build_supported")?,
        hooked: bool_after(&raw, "hooked")?,
        hook_ticks: num_after(&raw, "hook_ticks")? as u64,
        in_playground: bool_after(&raw, "in_playground")?,
        car_valid: bool_after(&raw, "car_valid")?,
        car: str_after(&raw, "car").unwrap_or_default(),
        pos: vec3_after(&raw, "pos")?,
        vel: vec3_after(&raw, "vel")?,
        wheels_down: num_after(&raw, "wheels_down")? as i64,
        jumps: num_after(&raw, "jumps")? as u64,
        cmd_seq: num_after(&raw, "cmd_seq").unwrap_or(-1.0) as i64,
        cmd_result: str_after(&raw, "cmd_result").unwrap_or_default(),
        last_jump: str_after(&raw, "last_jump").unwrap_or_default(),
        status: str_after(&raw, "status").unwrap_or_default(),
    })
}

#[derive(Debug, Clone, Default)]
struct Outcome {
    state: State,
    max_y: f64,
    elapsed: Duration,
    samples: u64,
}

/// THE ONLY SYNCHRONISATION PRIMITIVE.
fn wait_for(host: &Host, event: &str, timeout: Duration) -> Result<Outcome, String> {
    let start = Instant::now();
    let base = read_state();
    let base_heartbeat = base.as_ref().map(|s| s.heartbeat).unwrap_or(0);
    let base_ticks = base.as_ref().map(|s| s.hook_ticks).unwrap_or(0);
    let wanted_seq: Option<i64> =
        event.strip_prefix("cmd:").and_then(|n| n.parse::<i64>().ok());
    let process_event = matches!(event, "process" | "exit" | "lock-free");

    let mut out = Outcome { max_y: f64::MIN, ..Default::default() };
    let mut last_progress = Instant::now();
    let mut last_seen: Option<State> = None;

    loop {
        let running = || tmdrive::game_pid(host).is_some();

        if event == "process" && running() {
            out.elapsed = start.elapsed();
            println!("  [ok] {:<9} {:.1}s", event, out.elapsed.as_secs_f64());
            return Ok(out);
        }
        if event == "exit" && !running() {
            out.elapsed = start.elapsed();
            println!("  [ok] {:<9} {:.1}s", event, out.elapsed.as_secs_f64());
            return Ok(out);
        }
        if event == "lock-free" {
            let me = std::env::var("TM_SESSION")
                .or_else(|_| std::env::var("AGENTCLOUD_SESSION_ID"))
                .unwrap_or_default();
            if tmdrive::is_free(host, &me) {
                out.elapsed = start.elapsed();
                println!("  [ok] {:<9} {:.1}s", event, out.elapsed.as_secs_f64());
                return Ok(out);
            }
        }

        if let Some(s) = read_state() {
            out.samples += 1;
            if s.pos[1] > out.max_y {
                out.max_y = s.pos[1];
            }
            let hit = match event {
                "process" | "exit" | "lock-free" => false,
                "alive" => s.heartbeat > base_heartbeat + 3,
                "hooked" => s.hooked,
                "in-map" => s.in_playground,
                "car" => s.car_valid,
                "ticking" => s.hook_ticks > base_ticks + 10,
                "grounded" => s.wheels_down > 0 && s.vel[1].abs() < 0.5,
                "airborne" => s.wheels_down == 0,
                "apex" => s.vel[1] <= 0.0,
                "landed" => s.wheels_down > 0,
                _ => match wanted_seq {
                    Some(n) => s.cmd_seq >= n,
                    None => return Err(format!("unknown event '{}'", event)),
                },
            };
            out.state = s.clone();
            last_seen = Some(s);
            if hit {
                out.elapsed = start.elapsed();
                println!("  [ok] {:<9} {:.1}s", event, out.elapsed.as_secs_f64());
                return Ok(out);
            }
        }

        // A dead game can never satisfy a plugin event: fail now, not at the
        // end of the timeout.
        if !process_event && out.samples % 20 == 0 && tmdrive::game_pid(host).is_none() {
            return Err(format!(
                "GAME EXITED while waiting for '{}' after {:.1}s",
                event,
                start.elapsed().as_secs_f64()
            ));
        }

        if start.elapsed() >= timeout {
            return Err(match last_seen {
                Some(s) => format!(
                    "TIMEOUT '{}' after {:.0}s - heartbeat={} hooked={} in_map={} car={} ticks={} wheels={} vy={:.2} status='{}'",
                    event, timeout.as_secs_f64(), s.heartbeat, s.hooked, s.in_playground,
                    s.car_valid, s.hook_ticks, s.wheels_down, s.vel[1], s.status
                ),
                None => format!(
                    "TIMEOUT '{}' after {:.0}s - the plugin never wrote {}",
                    event, timeout.as_secs_f64(), state_path().display()
                ),
            });
        }

        if last_progress.elapsed() >= PROGRESS_EVERY {
            last_progress = Instant::now();
            match &last_seen {
                Some(s) => println!(
                    "  ... {} {:.0}s (heartbeat={} hooked={} in_map={} car={} wheels={})",
                    event, start.elapsed().as_secs_f64(), s.heartbeat, s.hooked,
                    s.in_playground, s.car_valid, s.wheels_down
                ),
                None => println!(
                    "  ... {} {:.0}s (no state file yet)",
                    event,
                    start.elapsed().as_secs_f64()
                ),
            }
        }

        sleep(POLL_TICK); // the one poll tick in the system
    }
}

fn send_command(
    host: &Host,
    verb: &str,
    arg: &str,
    timeout: Duration,
) -> Result<Outcome, String> {
    let next_seq = read_state().map(|s| s.cmd_seq).unwrap_or(-1) + 1;
    let line = if arg.is_empty() {
        format!("{} {}", next_seq, verb)
    } else {
        format!("{} {} {}", next_seq, verb, arg)
    };
    fs::write(cmd_path(), line).map_err(|e| format!("cannot write cmd.txt: {}", e))?;
    let o = wait_for(host, &format!("cmd:{}", next_seq), timeout)?;
    println!("  -> {}", o.state.cmd_result);
    Ok(o)
}

fn print_state(s: &State) {
    println!("heartbeat      {}", s.heartbeat);
    println!("build ok       {}", s.build_supported);
    println!("hooked         {}", s.hooked);
    println!("physics ticks  {}", s.hook_ticks);
    println!("in map         {}", s.in_playground);
    println!("car            {} (valid={})", s.car, s.car_valid);
    println!("pos            {:.2} {:.2} {:.2}", s.pos[0], s.pos[1], s.pos[2]);
    println!("vel            {:.2} {:.2} {:.2}", s.vel[0], s.vel[1], s.vel[2]);
    println!("wheels down    {}", s.wheels_down);
    println!("jumps          {}", s.jumps);
    println!("last jump      {}", s.last_jump);
    println!("status         {}", s.status);
}

fn dur(v: Option<&String>, default: u64) -> Duration {
    Duration::from_secs(v.and_then(|s| s.parse().ok()).unwrap_or(default))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: jumprig <launch|state|waitfor|cmd|jumptest|kill> [args]");
        eprintln!("  waitfor <event> [timeout_s]  process exit lock-free alive hooked in-map");
        eprintln!("                               car ticking grounded airborne apex landed");
        eprintln!("Every game-driving subcommand takes the tmdrive lock first.");
        std::process::exit(2);
    }
    let host = Host::detect();

    let r: Result<(), String> = match args[0].as_str() {
        // Read-only: no lock needed, and deliberately so.
        "state" => match read_state() {
            Some(s) => {
                print_state(&s);
                Ok(())
            }
            None => Err(format!("no readable state at {}", state_path().display())),
        },
        "waitfor" => {
            let ev = args.get(1).cloned().unwrap_or_default();
            wait_for(&host, &ev, dur(args.get(2), 120)).map(|o| print_state(&o.state))
        }

        // Everything below drives the game, so everything below holds the lock.
        "launch" => with_lock(host.clone(), "jump-button launch", |lock| {
            let t = dur(args.get(1), 300);
            let pid = ops::launch(lock, t.as_secs()).map_err(|e| e.to_string())?;
            println!("  game pid {pid}");
            let o = wait_for(lock.host(), "alive", t)?;
            if !o.state.build_supported {
                return Err(format!("build NOT supported: {}", o.state.status));
            }
            let o = wait_for(lock.host(), "hooked", Duration::from_secs(30))?;
            println!("hook installed, ticks={}", o.state.hook_ticks);
            Ok(())
        }),

        "cmd" => with_lock(host.clone(), "jump-button command", |lock| {
            let verb = args.get(1).cloned().unwrap_or_default();
            let arg = args[2..].join(" ");
            send_command(lock.host(), &verb, &arg, Duration::from_secs(30)).map(|_| ())
        }),

        "kill" => with_lock(host.clone(), "jump-button kill", |lock| {
            ops::kill(lock).map_err(|e| e.to_string())?;
            println!("killed");
            Ok(())
        }),

        "jumptest" => with_lock(host.clone(), "jump-button test", |lock| {
            let h = lock.host();
            println!("== preconditions ==");
            wait_for(h, "alive", Duration::from_secs(60))?;
            wait_for(h, "hooked", Duration::from_secs(30))?;
            wait_for(h, "in-map", Duration::from_secs(240))?;
            wait_for(h, "car", Duration::from_secs(60))?;
            wait_for(h, "ticking", Duration::from_secs(30))?;
            let settled = wait_for(h, "grounded", Duration::from_secs(60))?;
            let base_y = settled.state.pos[1];
            println!("  settled at y={:.3} wheels={}", base_y, settled.state.wheels_down);

            println!("== jump ==");
            let j = send_command(h, "jump", "", Duration::from_secs(15))?;
            let res = j.state.cmd_result.clone();
            if res.contains("not driving") || res.contains("no car") || res.contains("hook not installed") {
                return Err(format!("jump refused: {}", res));
            }

            // The apex is an EVENT (upward motion stops), not a sampling window.
            let apex = wait_for(h, "apex", Duration::from_secs(10))?;
            let gain = apex.max_y - base_y;
            println!("  apex y={:.3} (gain {:.3} m) after {:.2}s", apex.max_y, gain, apex.elapsed.as_secs_f64());
            let land = wait_for(h, "landed", Duration::from_secs(15))?;
            println!("  landed y={:.3} after {:.2}s", land.state.pos[1], land.elapsed.as_secs_f64());

            if gain < 0.25 {
                return Err(format!("JUMP DID NOT LIFT THE CAR: gain {:.3} m ('{}')", gain, res));
            }
            println!("RESULT: jump works — car rose {:.2} m", gain);
            Ok(())
        }),

        other => Err(format!("unknown subcommand '{}'", other)),
    };

    if let Err(e) = r {
        eprintln!("FAIL: {}", e);
        std::process::exit(1);
    }
}

fn with_lock<F>(host: Host, purpose: &str, f: F) -> Result<(), String>
where
    F: FnOnce(&GameLock) -> Result<(), String>,
{
    match tmdrive::acquire(host, purpose) {
        Ok(lock) => f(&lock),
        Err(Error::Busy(h)) => Err(format!(
            "the box is busy.\n{}\n  ask for it: agentcloudctl send-message --to {} --body '...'\n  or wait:    jumprig waitfor lock-free 600",
            h.summary(),
            h.session_id
        )),
        Err(e) => Err(e.to_string()),
    }
}
