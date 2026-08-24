//! Inventory semantic starts across a map corpus without reading any ghost.

use std::path::{Path, PathBuf};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn map_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.is_file() && p.to_string_lossy().ends_with(".Map.Gbx") {
            out.push(p);
        } else if p.is_dir() {
            for f in std::fs::read_dir(&p).map_err(|e| format!("{}: {e}", p.display()))? {
                let q = f.map_err(|e| e.to_string())?.path();
                if q.is_file() && q.to_string_lossy().ends_with(".Map.Gbx") {
                    out.push(q);
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

pub fn check(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let ghost = arg(args, "--recording").ok_or("--recording FILE is required")?;
    let expected = tmauto::synth::initial_state_for_map(&map)?;
    let recorded = gbx::record::decode_ghost(&ghost)?;
    let got = recorded
        .samples
        .first()
        .ok_or("recording has no vehicle sample")?;
    let dx = got.x - expected.pos[0] as f64;
    let dy = got.y - expected.pos[1] as f64;
    let dz = got.z - expected.pos[2] as f64;
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let dot = (got.qx * expected.quat[0]
        + got.qy * expected.quat[1]
        + got.qz * expected.quat[2]
        + got.qw * expected.quat[3])
        .abs()
        .clamp(-1.0, 1.0);
    let angle = 2.0 * dot.acos();
    println!(
        "map-derived\t{:.6}\t{:.6}\t{:.6}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\tdir={:?}",
        expected.pos[0],
        expected.pos[1],
        expected.pos[2],
        expected.quat[0],
        expected.quat[1],
        expected.quat[2],
        expected.quat[3],
        expected.roadtech_dir
    );
    println!(
        "recorded-t0\t{:.6}\t{:.6}\t{:.6}\t{:.9}\t{:.9}\t{:.9}\t{:.9}",
        got.x, got.y, got.z, got.qx, got.qy, got.qz, got.qw
    );
    println!("difference\tposition_m={dist:.6}\torientation_rad={angle:.9}");
    if dist > 0.02 || angle > 0.001 {
        return Err("map-derived start does not match the independent recording".into());
    }
    println!("PASS\tmap-derived RoadTechStart matches the independent tick-0 sample");
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), String> {
    let root = PathBuf::from(arg(args, "--root").ok_or("--root DIR is required")?);
    println!("path\tuid\tdecoration\twaypoint_order\tdir\tcx\tcy\tcz\tx\ty\tz\tstatus");
    for path in map_files(&root)? {
        let loaded = std::panic::catch_unwind(|| tmmaps::map::MapFile::load(&path));
        let m = match loaded {
            Ok(m) => m,
            Err(_) => {
                println!(
                    "{}\t-\t-\t-\t-\t-\t-\t-\t-\t-\t-\tparse-refused",
                    path.display()
                );
                continue;
            }
        };
        let uid = gbx::map_uid_of(&std::fs::read(&path).map_err(|e| e.to_string())?)
            .unwrap_or_else(|| "-".into());
        let waypoints = m.waypoints();
        let found = waypoints
            .iter()
            .enumerate()
            .find(|(_, w)| w.tag == "Spawn" && w.name == "RoadTechStart");
        let Some((order, w)) = found else {
            println!(
                "{}\t{}\t{}\t-\t-\t-\t-\t-\t-\t-\t-\tno-RoadTechStart",
                path.display(),
                uid,
                m.decoration_id
            );
            continue;
        };
        let state = tmauto::synth::initial_state_for_map(&path);
        match state {
            Ok(s) => println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\tok",
                path.display(),
                uid,
                m.decoration_id,
                order,
                w.dir.map(|x| x.to_string()).unwrap_or_else(|| "-".into()),
                w.coords.0,
                w.coords.1,
                w.coords.2,
                s.pos[0],
                s.pos[1],
                s.pos[2]
            ),
            Err(e) => println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t-\t-\t-\t{}",
                path.display(),
                uid,
                m.decoration_id,
                order,
                w.dir.map(|x| x.to_string()).unwrap_or_else(|| "-".into()),
                w.coords.0,
                w.coords.1,
                w.coords.2,
                e.replace(['\t', '\n', '\r'], " ")
            ),
        }
    }
    Ok(())
}
