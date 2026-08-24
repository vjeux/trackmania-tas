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
