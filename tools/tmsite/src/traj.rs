//! Loading the decoded trajectories that `entrec` writes.
//!
//! One JSON file per ghost:
//!   {"name","time_ms","checkpoints_ms","sample_period_ms","samples":[{t,x,y,z,speed,gear,yaw}]}

use crate::json::{self, Value};

pub struct Sample {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub speed: f64,
}

pub struct Traj {
    pub name: String,
    pub time_ms: i64,
    pub cps: Vec<i64>,
    pub samples: Vec<Sample>,
}

/// Every `*.json` in `dir`, in filename order, sub-sampled by `stride`.
/// A file that does not parse is skipped, matching the Python (which swallowed
/// the exception so a half-written file from a concurrent decode run could not
/// break the build).
pub fn load_dir(dir: &str, stride: usize) -> Vec<Traj> {
    assert!(stride >= 1, "stride must be >= 1");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {}", dir, e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    files.sort();

    let mut out = Vec::new();
    for f in files {
        let text = match std::fs::read_to_string(&f) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let v = match json::parse(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let time_ms = v.get("time_ms").and_then(|x| x.as_i64()).unwrap_or(0);
        let cps: Vec<i64> = v
            .get("checkpoints_ms")
            .and_then(|x| x.as_arr())
            .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
            .unwrap_or_default();
        let all = match v.get("samples").and_then(|x| x.as_arr()) {
            Some(a) => a,
            None => continue,
        };
        let samples: Vec<Sample> = all
            .iter()
            .step_by(stride)
            .map(|s| Sample {
                x: num(s, "x"),
                y: num(s, "y"),
                z: num(s, "z"),
                speed: num(s, "speed"),
            })
            .collect();
        out.push(Traj { name, time_ms, cps, samples });
    }
    out
}

fn num(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

/// The map's checkpoint markers, in the Python's dict order.
pub const CHECKPOINTS: [(&str, f64, f64); 5] = [
    ("start", 1584.0, 784.0),
    ("CP1", 1232.0, 976.0),
    ("CP2", 1154.0, 1328.0),
    ("CP3", 1360.0, 1104.0),
    ("finish", 1360.0, 688.0),
];
