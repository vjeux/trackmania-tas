//! `tinyctl startcheck --map M [--tag T]` — where does the CLIENT put the car?
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

    // Open the playground and read the car at rest.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = Command::new(&exe)
        .args(["play", "--map"])
        .arg(&map)
        .args(["--tag", &tag, "--shots", "1", "--first-ms", "600", "--wheels-ms", "100", "--outdir"])
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
        return Err(format!("{}: no car row — the playground opened with NO VEHICLE (this map cannot be driven)", wheels.display()));
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
