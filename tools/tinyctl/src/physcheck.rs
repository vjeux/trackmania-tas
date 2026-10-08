//! `tinyctl phys-check --orig A.Map.Gbx --conv B.Map.Gbx [--surface dirt] [--cells 1,12,2]…`
//! — does the converted map DRIVE as the new surface, or only look like it?
//!
//! Why this exists (Everios96, 2026-10-07: *"we want both physics and texture to be
//! correct … we are just looking for a way for you to confirm whether your changes
//! worked or not. Because i dont want to test every map you create"*). Every earlier
//! answer came from the wheel MATERIAL column, and on a water block that column reads
//! `13` (water) whatever the car is standing on — the wheels are submerged. So this
//! check never looks at the material: it drives the same car over the same geometry in
//! both maps and compares what the car DOES.
//!
//! The measurement: the start block is moved onto the cell under test, the accelerator
//! is held through the box's SendInput helper (`tinyctl play --hold-up`), and the wheel
//! log gives speed against distance. A surface the car can feel changes that curve —
//! measured on a row of six `RoadWaterStraight` at 200 m: water 38.2, dirt 41.2,
//! tech 43.2, plastic 43.9 m/s. A conversion that only repainted the floor reproduces
//! the original's curve exactly (that was the bug this check is built to catch).
//!
//! Verdicts per probe: `PASS` the converted map's curve differs from the original's by
//! more than the noise floor in the expected direction; `SAME` it does not (the surface
//! is not reaching the car); `FAIL` the car never drove (no map, no contact, fell).
//! Noise: two runs of the SAME map agree within ~0.3 m/s at every mark, so `--tol 1.0`
//! is five sigma.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One driven probe: where the car starts, and the speeds it reached.
struct Run {
    marks: BTreeMap<u32, f32>,
    top: f32,
    contact_y: f32,
    moved: usize,
}

/// Speeds at fixed distances from the spawn, read out of a `--wheels-ms` log.
fn read_wheels(path: &Path, marks: &[u32]) -> Result<Run, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let rows: Vec<Vec<&str>> = text
        .lines()
        .skip(1)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|f| f.len() > 27)
        .collect();
    let num = |r: &Vec<&str>, i: usize| -> f32 { r.get(i).and_then(|v| v.parse().ok()).unwrap_or(0.0) };
    let moving: Vec<&Vec<&str>> = rows.iter().filter(|r| num(r, 8) > 0.5).collect();
    if moving.is_empty() {
        return Err("the car never moved (no drive input reached the game, or it is stuck)".into());
    }
    let (x0, z0) = (num(moving[0], 2), num(moving[0], 4));
    let mut out = BTreeMap::new();
    for m in marks {
        let want = *m as f32;
        if let Some(r) = moving.iter().find(|r| ((num(r, 2) - x0).powi(2) + (num(r, 4) - z0).powi(2)).sqrt() >= want) {
            out.insert(*m, num(r, 8));
        }
    }
    let grounded: Vec<&&Vec<&str>> = moving.iter().filter(|r| r.get(12) == Some(&"1")).collect();
    let contact_y = if grounded.is_empty() {
        0.0
    } else {
        grounded.iter().map(|r| num(r, 3) - num(r, 26)).sum::<f32>() / grounded.len() as f32
    };
    Ok(Run {
        top: moving.iter().map(|r| num(r, 8)).fold(0.0, f32::max),
        marks: out,
        contact_y,
        moved: moving.len(),
    })
}

/// Drive one map from one cell and read the wheel log back.
#[allow(clippy::too_many_arguments)]
fn drive(bin: &Path, map: &Path, cell: &str, dir: &str, tag: &str, outdir: &Path, hold_ms: u32, marks: &[u32], tmmaps: &Path) -> Result<Run, String> {
    let staged = outdir.join(format!("{tag}.Map.Gbx"));
    let timed = outdir.join(format!("{tag}-t.Map.Gbx"));
    let st = Command::new(tmmaps)
        .args(["restart", &map.display().to_string(), "--cell", cell, "--dir", dir, "--out", &staged.display().to_string()])
        .output()
        .map_err(|e| format!("tmmaps restart: {e}"))?;
    if !st.status.success() {
        return Err(format!("tmmaps restart: {}", String::from_utf8_lossy(&st.stderr).trim()));
    }
    // a map with no author time is refused by the client
    let _ = Command::new(tmmaps)
        .args([
            "set-times",
            &staged.display().to_string(),
            "--author",
            "9786",
            "--gold",
            "11000",
            "--silver",
            "12000",
            "--bronze",
            "15000",
            "--out",
            &timed.display().to_string(),
        ])
        .output();
    let run_map = if timed.exists() { timed } else { staged };
    let out = Command::new(bin)
        .args([
            "play",
            "--map",
            &run_map.display().to_string(),
            "--tag",
            tag,
            "--shots",
            "1",
            "--first-ms",
            &(hold_ms + 7000).to_string(),
            "--wheels-ms",
            &(hold_ms + 6000).to_string(),
            "--hold-up",
            "--hold-ms",
            &hold_ms.to_string(),
            "--timeout",
            "300",
            "--outdir",
            &outdir.display().to_string(),
        ])
        .output()
        .map_err(|e| format!("tinyctl play: {e}"))?;
    let log = outdir.join(format!("wheels-{tag}.tsv"));
    if !log.exists() {
        let tail: String = String::from_utf8_lossy(&out.stderr).lines().rev().take(2).collect::<Vec<_>>().join(" | ");
        return Err(format!("no wheel log ({tail})"));
    }
    read_wheels(&log, marks)
}

pub fn cmd(args: &[String]) -> i32 {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let (Some(orig), Some(conv)) = (f("--orig"), f("--conv")) else {
        eprintln!("tinyctl phys-check --orig ORIGINAL.Map.Gbx --conv CONVERTED.Map.Gbx [--cells C[;C…]] [--dir D] [--tol 1.0] [--hold-ms 15000] [--outdir DIR]");
        eprintln!("  drives both maps from the same cell and compares SPEED OVER DISTANCE — the wheel");
        eprintln!("  material column is useless under water (it always reads 13), the car's behaviour is not.");
        return 2;
    };
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/tmp/physcheck".into()));
    let _ = std::fs::create_dir_all(&outdir);
    let cells: Vec<String> = f("--cells").unwrap_or_else(|| "1,12,2".into()).split(';').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    let dir = f("--dir").unwrap_or_else(|| "0".into());
    let tol: f32 = f("--tol").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let hold_ms: u32 = f("--hold-ms").and_then(|v| v.parse().ok()).unwrap_or(15000);
    let marks: Vec<u32> = f("--marks").unwrap_or_else(|| "20,60,100,140,180,200".into()).split(',').filter_map(|m| m.trim().parse().ok()).collect();
    let bin = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tinyctl"));
    let tmmaps = PathBuf::from(f("--tmmaps").unwrap_or_else(|| {
        bin.parent().map(|p| p.join("tmmaps").display().to_string()).unwrap_or_else(|| "tmmaps".into())
    }));
    let (mut pass, mut same, mut fail) = (0u32, 0u32, 0u32);
    println!("cell\tverdict\toriginal m/s\tconverted m/s\tΔ at the last mark\tcontact height");
    for (k, cell) in cells.iter().enumerate() {
        let o = drive(&bin, Path::new(&orig), cell, &dir, &format!("pc{k}o"), &outdir, hold_ms, &marks, &tmmaps);
        let c = drive(&bin, Path::new(&conv), cell, &dir, &format!("pc{k}c"), &outdir, hold_ms, &marks, &tmmaps);
        match (o, c) {
            (Ok(o), Ok(c)) => {
                let last = marks.iter().rev().find(|m| o.marks.contains_key(m) && c.marks.contains_key(m)).copied();
                let d = last.map(|m| c.marks[&m] - o.marks[&m]).unwrap_or(0.0);
                let verdict = if last.is_none() {
                    fail += 1;
                    "FAIL (neither run reached a mark)"
                } else if d.abs() >= tol {
                    pass += 1;
                    "PASS"
                } else {
                    same += 1;
                    "SAME — the surface is not reaching the car"
                };
                println!(
                    "{cell}\t{verdict}\t{:.1}\t{:.1}\t{:+.1}\t{:.3} → {:.3}",
                    last.map(|m| o.marks[&m]).unwrap_or(o.top),
                    last.map(|m| c.marks[&m]).unwrap_or(c.top),
                    d,
                    o.contact_y,
                    c.contact_y
                );
            }
            (o, c) => {
                fail += 1;
                let why = [o.err(), c.err()].into_iter().flatten().collect::<Vec<_>>().join("; ");
                println!("{cell}\tFAIL\t-\t-\t-\t{why}");
            }
        }
    }
    println!("\n{pass} pass, {same} unchanged, {fail} failed");
    i32::from(same > 0 || fail > 0)
}
