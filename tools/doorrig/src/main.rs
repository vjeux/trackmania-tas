//! `doorrig` — the Yannex question, measured: can an Openplanet plugin move a
//! PHYSICAL obstacle at runtime, i.e. does the car's collision follow a
//! kinematic (moving) item whose pose the plugin forces?
//!
//! The test map (`YannexDoor.Map.Gbx`, `tmmaps lineup` over the straight
//! road) puts stock `ObstaclePusher8mLevel1` pistons across the road's centre
//! line a few blocks after the start. GhostShooter's `Kine.as` exposes the
//! playground's `NSceneDyna_SMgr` kinematic shared signals (the item model's
//! `NPlugDyna_SKinematicConstraint` + phase) for reading and writing. This
//! rig drives the car at the door under three regimes and logs the trajectory:
//!
//! ```text
//! doorrig load  --map C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/YannexDoor.Map.Gbx
//!               play the map (PlayMap), wait for the playground BY PROGRESS
//!               (ctx 3 + the map's name + a live car row), print /kine
//! doorrig kine  [--name Pusher]          the /kine dump (read)
//! doorrig hold  --i N --tmin X --tmax X [--phase P]   /kineset on shared signal N (write)
//! doorrig drive --hold MS [--log MS] [--respawn [--key DEL]] [--label L] [--out FILE]
//!               [restart from the start (DEL), then] hold the accelerator MS while
//!               logging /carlog for LOG ms; prints the trajectory summary
//!               (start, farthest z, final, top speed) and appends the raw TSV
//!               to FILE
//! doorrig move  --i MOBIL --dx X --dy Y --dz Z       /kinemove (the visual only)
//! doorrig menu                            back to the title screen (/back)
//! ```
//!
//! Every subcommand takes the tmdrive lock (one game, one driver) for its own
//! duration and releases it — slots stay short, other sessions get the box
//! between steps. Waits are on real conditions with a ceiling; no fixed sleeps
//! except the sampling interval of a poll.

use std::time::{Duration, Instant};
use tmdrive::{acquire, ops, Error, GameLock, Host};

const SHOOTCTL: &str = "/home/vjeux/trackmania-tas/tools/target/release/shootctl";

fn flag(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}
fn has(args: &[String], k: &str) -> bool {
    args.iter().any(|a| a == k)
}

fn usage() -> ! {
    eprintln!("doorrig load --map P | kine [--name S] | hold --i N --tmin X --tmax X [--phase P] | drive --hold MS [--log MS] [--respawn] [--label L] [--out F] | move --i M --dx X --dy Y --dz Z | menu");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let host = Host::detect();
    let purpose = format!("Yannex door test: {}", args[0]);
    let r: Result<(), String> = match acquire(host, &purpose) {
        Err(Error::Busy(h)) => {
            eprintln!("busy: {}", h.summary());
            std::process::exit(75);
        }
        Err(e) => Err(e.to_string()),
        Ok(lock) => match args[0].as_str() {
            "load" => load(&lock, &flag(&args, "--map").unwrap_or_else(|| usage())),
            "kine" => kine(&lock, &flag(&args, "--name").unwrap_or_else(|| "Pusher".into())).map(|s| println!("{s}")),
            "hold" => hold(&lock, &args),
            "drive" => drive(&lock, &args),
            "move" => mv(&lock, &args),
            "menu" => ops::plugin(&lock, "back", "").map(|s| println!("{}", s.trim())).map_err(|e| e.to_string()),
            _ => usage(),
        },
    };
    if let Err(e) = r {
        eprintln!("doorrig: {e}");
        std::process::exit(1);
    }
}

fn plugin(lock: &GameLock, ep: &str, q: &str) -> Result<String, String> {
    ops::plugin(lock, ep, q).map_err(|e| e.to_string())
}

/// A read route, no lock renewal needed (the guard's renewer runs anyway).
fn read(route: &str) -> String {
    tmdrive::plugin::get(route, 15).unwrap_or_else(|e| format!("err: {e}"))
}

fn kine(lock: &GameLock, name: &str) -> Result<String, String> {
    plugin(lock, "kine", &format!("name={name}"))
}

/// Into the map, by progress: the game is asked, not a clock.
fn load(lock: &GameLock, map: &str) -> Result<(), String> {
    let r = ops::play_map(lock, map).map_err(|e| e.to_string())?;
    println!("{}", r.trim());
    let t0 = Instant::now();
    let ceiling = Duration::from_secs(600);
    let mut answered = false;
    let mut stable_since: Option<Instant> = None;
    loop {
        if t0.elapsed() > ceiling {
            return Err(format!("no playground within {}s; last ctx {}", ceiling.as_secs(), read("/ctx").trim()));
        }
        if tmdrive::game_pid(lock.host()).is_none() {
            return Err("the game exited while loading".into());
        }
        let ctx = read("/ctx");
        if ctx.contains("\"dialog\":\"") && !ctx.contains("\"dialog\":null") {
            if answered {
                return Err(format!("a dialog blocks the load: {}", ctx.trim()));
            }
            println!("  dialog {} -> /yes", ctx.trim());
            let _ = plugin(lock, "yes", "");
            answered = true;
        }
        // the transient ctx 3 / map:null right after /playmap does not count;
        // the playground is open when ctx 3 holds with the map's name for 2 s
        // AND the car answers
        if ctx.contains("\"ctx\":3") && ctx.contains("\"playground\":true") && !ctx.contains("\"map\":null") {
            let since = *stable_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= Duration::from_secs(2) {
                let car = read("/car");
                if !car.starts_with("err") && car.split('\t').count() >= 9 {
                    println!("  playground open after {:.1}s: ctx {}", t0.elapsed().as_secs_f64(), ctx.trim());
                    println!("  car: {}", car.trim());
                    break;
                }
            }
        } else {
            stable_since = None;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    println!("{}", kine(lock, "Pusher")?);
    Ok(())
}

fn hold(lock: &GameLock, args: &[String]) -> Result<(), String> {
    let i = flag(args, "--i").unwrap_or_else(|| "0".into());
    let mut q = format!("i={i}");
    for k in ["tmin", "tmax", "amin", "amax", "phase"] {
        if let Some(v) = flag(args, &format!("--{k}")) {
            q.push_str(&format!("&{k}={v}"));
        }
    }
    let r = plugin(lock, "kineset", &q)?;
    println!("{}", r.trim());
    if r.contains("token-refused") || r.starts_with("usage") || r.contains("no dyna") {
        return Err("kineset did not apply".into());
    }
    Ok(())
}

fn mv(lock: &GameLock, args: &[String]) -> Result<(), String> {
    let q = format!(
        "i={}&dx={}&dy={}&dz={}",
        flag(args, "--i").unwrap_or_else(|| usage()),
        flag(args, "--dx").unwrap_or_else(|| "0".into()),
        flag(args, "--dy").unwrap_or_else(|| "0".into()),
        flag(args, "--dz").unwrap_or_else(|| "0".into())
    );
    println!("{}", plugin(lock, "kinemove", &q)?.trim());
    Ok(())
}

/// A key to the game window through shootctl (PowerShell keybd_event, the
/// game brought to the foreground first). Runs on the box; the lock is held
/// by the caller, so this is a game input under the lock.
fn key(name: &str, hold_ms: u64) -> Result<String, String> {
    let out = std::process::Command::new(SHOOTCTL)
        .args(["key", name, "--hold-ms", &hold_ms.to_string()])
        .output()
        .map_err(|e| format!("shootctl key: {e}"))?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        return Err(format!("shootctl key {name}: {s} {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(s)
}

#[derive(Default, Clone, Copy)]
struct Row {
    wall: f64,
    x: f64,
    y: f64,
    z: f64,
    speed: f64,
}

fn parse_rows(tsv: &str) -> Vec<Row> {
    tsv.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("wall_ms"))
        .filter_map(|l| {
            let f: Vec<f64> = l.split('\t').filter_map(|c| c.trim().parse().ok()).collect();
            if f.len() >= 9 {
                Some(Row { wall: f[0], x: f[2], y: f[3], z: f[4], speed: f[8] })
            } else {
                None
            }
        })
        .collect()
}

/// Respawn (optional), then accelerate for `--hold` ms while `/carlog`
/// samples the car; the summary says how far along the road (z) the car got.
fn drive(_lock: &GameLock, args: &[String]) -> Result<(), String> {
    let hold_ms: u64 = flag(args, "--hold").and_then(|v| v.parse().ok()).unwrap_or(5000);
    let log_ms: u64 = flag(args, "--log").and_then(|v| v.parse().ok()).unwrap_or(hold_ms + 2000);
    let label = flag(args, "--label").unwrap_or_else(|| "drive".into());
    let out = flag(args, "--out");
    let ctx = read("/ctx");
    if !ctx.contains("\"ctx\":3") {
        return Err(format!("not in a playground: {}", ctx.trim()));
    }
    let before = read("/car");
    println!("[{label}] car before: {}", before.trim());
    if has(args, "--respawn") {
        // DEL = give up / restart from the START (Backspace only goes back to
        // the last checkpoint, and a run that passed the doors has one);
        // --key BACKSPACE for a checkpoint respawn
        let k = flag(args, "--key").unwrap_or_else(|| "DEL".into());
        println!("[{label}] respawn ({k}): {}", key(&k, 80)?);
        // the car is back when its z is within the start block and it stands
        // still; give the respawn up to 8 s
        let t0 = Instant::now();
        loop {
            let rows = parse_rows(&read("/carlog?ms=300"));
            if let Some(r) = rows.last() {
                if r.speed.abs() < 0.5 && r.z < 260.0 {
                    println!("[{label}] respawned at ({:.2},{:.2},{:.2})", r.x, r.y, r.z);
                    break;
                }
            }
            if t0.elapsed() > Duration::from_secs(8) {
                println!("[{label}] respawn not confirmed within 8 s; driving anyway");
                break;
            }
        }
    }
    // the accelerator on its own thread; the log on this one, in 5 s slices
    // (a longer /carlog request never returns)
    let driver = std::thread::spawn(move || key("UP", hold_ms));
    let t0 = Instant::now();
    let mut tsv = String::new();
    let mut left = log_ms;
    while left > 0 {
        let chunk = left.min(5000);
        let body = read(&format!("/carlog?ms={chunk}"));
        for l in body.lines() {
            if !l.starts_with("wall_ms") || tsv.is_empty() {
                tsv.push_str(l);
                tsv.push('\n');
            }
        }
        left = left.saturating_sub(chunk);
    }
    let held = driver.join().map_err(|_| "driver thread panicked".to_string())??;
    let rows = parse_rows(&tsv);
    if rows.is_empty() {
        return Err(format!("no car rows logged ({held}); body head: {}", tsv.lines().take(3).collect::<Vec<_>>().join(" | ")));
    }
    let first = rows[0];
    let last = rows[rows.len() - 1];
    let far = rows.iter().fold(rows[0], |m, r| if r.z > m.z { *r } else { m });
    let top = rows.iter().map(|r| r.speed).fold(0.0, f64::max);
    // where the car stopped advancing: the first row after which z never grows by > 0.2 m
    let mut stall = last;
    for (k, r) in rows.iter().enumerate() {
        if rows[k..].iter().all(|q| q.z <= r.z + 0.2) {
            stall = *r;
            break;
        }
    }
    println!(
        "[{label}] {} rows over {:.1}s ({held}); start ({:.2},{:.2},{:.2}) -> farthest z {:.2} (x {:.2}) -> final ({:.2},{:.2},{:.2}) speed {:.1}; top speed {:.1}; stopped advancing at z {:.2} after {:.1}s",
        rows.len(),
        t0.elapsed().as_secs_f64(),
        first.x, first.y, first.z,
        far.z, far.x,
        last.x, last.y, last.z, last.speed,
        top,
        stall.z,
        (stall.wall - first.wall) / 1000.0
    );
    if let Some(p) = out {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p).map_err(|e| format!("{p}: {e}"))?;
        use std::io::Write;
        writeln!(f, "# {label}").map_err(|e| e.to_string())?;
        f.write_all(tsv.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(())
}
