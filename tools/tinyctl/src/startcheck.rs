//! `tinyctl startcheck --map M [--tag T] [--wheels-ms 15000]` — where does the CLIENT put the car?
//!
//! The freeze pass needs this per map. A published tiny map is only sound if
//! the dedicated server (the oracle that certifies laps and the author ghost)
//! and the client (what a player drives) start the car in the SAME place: a
//! server-certified lap that starts 158 m from where the player starts is not
//! a lap anybody can drive, and a client-driven validation lap DNFs on the
//! server. The server side is read by the player project; this is the client
//! side, measured the only way that is honest — by opening the playground and
//! reading the car's resting position out of the game.
//!
//! The expected position comes from the map itself: the Spawn placement plus
//! the spawn offset its model declares (a block-derived start carries the
//! block info's `spawn_loc`, halved; a start GATE item carries the pack
//! prefab's `NPlugTrigger_SSpawn` point). Rather than re-deriving that here,
//! the check reports the measured car position, the Spawn placement position
//! and the distance between them, and PASSES when the car is within
//! `--tolerance` (default 12 m — a start block's own offset is (8, 1, 8),
//! i.e. 11.4 m, so anything under ~12 m is "on the start", and the failures
//! this catches are 100 m+).

use std::path::PathBuf;
use std::process::Command;

pub fn run(args: &[String]) -> Result<(), String> {
    let f = |k: &str| -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() };
    let map = PathBuf::from(f("--map").ok_or("startcheck needs --map MAP.Map.Gbx")?);
    let tag = f("--tag").unwrap_or_else(|| "startchk".into());
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/tmp/tiny3".into()));
    let tol: f32 = f("--tolerance").and_then(|v| v.parse().ok()).unwrap_or(12.0);

    // The map's own Spawn placement.
    let m = tmmaps::map::MapFile::load(&map);
    let spawn = m
        .items
        .iter()
        .find(|it| it.waypoint_tag.as_deref() == Some("Spawn"))
        .ok_or("this map has no placement tagged Spawn")?;
    let sp = spawn.pos;
    println!("{}: Spawn placement i{} {} at [{:.2}, {:.2}, {:.2}]", map.display(), spawn.index, spawn.model, sp[0], sp[1], sp[2]);

    // Open the playground and read the car at rest. The wheel log runs for
    // `--wheels-ms` (default 15 000): the vehicle state is null through the
    // MediaTracker intro and the first frames of a big map (tiny 24 with
    // 22 662 items, 2026-09-08: a 100 ms window right after the playground
    // opened held two "# no vehicle state" rows and the check said NO VEHICLE
    // while the screenshot 5 s later showed the car on the start line), so
    // the window is long and the FIRST row with a position is the answer —
    // without input the car does not move.
    let wheels_ms = f("--wheels-ms").unwrap_or_else(|| "15000".into());
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = Command::new(&exe)
        .args(["play", "--map"])
        .arg(&map)
        .args(["--tag", &tag, "--shots", "1", "--first-ms", "600", "--wheels-ms", &wheels_ms, "--outdir"])
        .arg(&outdir)
        .output()
        .map_err(|e| format!("tinyctl play: {e}"))?;
    let log = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!("tinyctl play failed:\n{}", log.lines().rev().take(6).collect::<Vec<_>>().join("\n")));
    }
    let wheels = outdir.join(format!("wheels-{tag}.tsv"));
    let text = std::fs::read_to_string(&wheels).map_err(|e| format!("{}: {e}", wheels.display()))?;

    // First data row with a position: the car before any input.
    let row = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("wall_ms"))
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .find(|c| c.len() > 4);
    let Some(c) = row else {
        return Err(format!("{}: no car row in {wheels_ms} ms — the playground opened with NO VEHICLE (this map cannot be driven), or the vehicle state never came up: look at the play sheet", wheels.display()));
    };
    let car = [c[2].parse::<f32>().unwrap_or(f32::NAN), c[3].parse::<f32>().unwrap_or(f32::NAN), c[4].parse::<f32>().unwrap_or(f32::NAN)];
    let d = ((car[0] - sp[0]).powi(2) + (car[1] - sp[1]).powi(2) + (car[2] - sp[2]).powi(2)).sqrt();
    println!("{}: client car at [{:.2}, {:.2}, {:.2}] — {:.2} m from the Spawn placement", map.display(), car[0], car[1], car[2], d);
    if d <= tol {
        println!("{}: PASS (within {tol} m: the client starts on the start line)", map.display());
        Ok(())
    } else {
        Err(format!("{}: FAIL — the client starts {d:.1} m from the Spawn placement (tolerance {tol} m). The car is on some other waypoint; do not publish this build.", map.display()))
    }
}

/// `tinyctl startcheck --maps A.Map.Gbx,B.Map.Gbx,… [--tag-prefix sc] [--tolerance T] [--outdir D] [--report R.tsv]`
/// — the check above over a list, one play load each under the render lock,
/// a TSV row per map (map, name, verdict, car, spawn, distance, seconds); a map
/// already in the report is skipped (a batch restarts where it stopped). The
/// giant campaigns (2026-09-22): 75 maps, the block start's own spawn offset is
/// (8k, 1, 8k) at scale k, so the tolerance is the caller's (26 / 37 / 48 m).
pub fn run_batch(args: &[String]) -> Result<(), String> {
    let f = |k: &str| -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() };
    let maps: Vec<PathBuf> = f("--maps").ok_or("startcheck --maps A,B,…")?.split(',').filter(|s| !s.trim().is_empty()).map(|s| PathBuf::from(s.trim())).collect();
    let prefix = f("--tag-prefix").unwrap_or_else(|| "sc".into());
    let outdir = f("--outdir").unwrap_or_else(|| "/tmp/tiny3".into());
    let report = PathBuf::from(f("--report").unwrap_or_else(|| format!("{outdir}/startcheck.tsv")));
    let tol = f("--tolerance").unwrap_or_else(|| "12".into());
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{outdir}: {e}"))?;
    let done: std::collections::HashSet<String> = std::fs::read_to_string(&report).map(|t| t.lines().skip(1).filter_map(|l| l.split('\t').next().map(String::from)).collect()).unwrap_or_default();
    if !report.exists() {
        std::fs::write(&report, "map\tname\tverdict\tcar\tspawn\tdistance_m\tseconds\n").map_err(|e| format!("{}: {e}", report.display()))?;
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut failed = 0usize;
    for (i, map) in maps.iter().enumerate() {
        let key = map.display().to_string();
        if done.contains(&key) {
            println!("{key}: already in the report, skipped");
            continue;
        }
        let name = tmmaps::header::read(&key).map(|h| h.name).unwrap_or_default();
        let tag = format!("{prefix}{i:02}");
        let t0 = std::time::Instant::now();
        let out = Command::new(&exe).args(["startcheck", "--map", &key, "--tag", &tag, "--tolerance", &tol, "--outdir", &outdir]).output().map_err(|e| format!("tinyctl startcheck: {e}"))?;
        let log = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        let secs = t0.elapsed().as_secs();
        let car_line = log.lines().find(|l| l.contains("client car at")).unwrap_or("");
        let car = car_line.split("client car at ").nth(1).and_then(|s| s.split(']').next()).map(|s| format!("{s}]")).unwrap_or_else(|| "-".into());
        let dist = car_line.split("] — ").nth(1).and_then(|s| s.split(' ').next()).unwrap_or("-").to_string();
        let spawn = log.lines().find(|l| l.contains("Spawn placement")).and_then(|l| l.split(" at ").nth(1)).unwrap_or("-").to_string();
        let verdict = if out.status.success() && log.contains(": PASS") {
            "PASS".to_string()
        } else {
            failed += 1;
            let why = log.lines().rev().find(|l| l.contains("FAIL") || l.contains("failed") || l.contains("NO VEHICLE") || l.contains("Error") || l.contains("error")).unwrap_or("FAIL (see the play log)");
            format!("FAIL: {}", why.replace('\t', " ").chars().take(160).collect::<String>())
        };
        println!("{key}: {verdict} ({secs} s)");
        let row = format!("{key}\t{name}\t{verdict}\t{car}\t{spawn}\t{dist}\t{secs}\n");
        let mut fh = std::fs::OpenOptions::new().append(true).open(&report).map_err(|e| format!("{}: {e}", report.display()))?;
        std::io::Write::write_all(&mut fh, row.as_bytes()).map_err(|e| e.to_string())?;
    }
    if failed > 0 {
        return Err(format!("{failed} of {} maps failed the start check", maps.len()));
    }
    Ok(())
}
