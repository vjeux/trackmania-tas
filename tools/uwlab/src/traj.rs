//! The 29-column trajectory CSV, read once.
//!
//! Both `tmtraj export --csv` and `fk trace` emit the same header. Everything
//! in `uwlab` reads through this module so a column index is spelled out in
//! exactly one place.

use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub t: f64, // seconds
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub speed_ms: f64,
    pub ground: bool,
    pub gear: f64,
    pub steer: f64,
    pub gas: f64,
    pub brake: f64,
}

pub struct Traj {
    pub rows: Vec<Row>,
    pub path: String,
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim(), "True" | "true" | "1" | "TRUE")
}

impl Traj {
    pub fn load(path: &str) -> Result<Traj, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let mut lines = text.lines();
        let header = lines.next().ok_or_else(|| format!("{path}: empty"))?;
        let mut idx: HashMap<&str, usize> = HashMap::new();
        for (i, name) in header.trim_end_matches('\r').split(',').enumerate() {
            idx.insert(name.trim(), i);
        }
        let need = |n: &str| -> Result<usize, String> {
            idx.get(n)
                .copied()
                .ok_or_else(|| format!("{path}: no column `{n}`"))
        };
        let (ct, cx, cy, cz) = (need("time_ms")?, need("x")?, need("y")?, need("z")?);
        let (cvx, cvy, cvz) = (need("vx")?, need("vy")?, need("vz")?);
        let csp = need("speed_ms")?;
        let cg = need("is_ground_contact")?;
        let cgear = need("gear")?;
        let (cst, cgas, cbr) = (need("steer")?, need("gas")?, need("brake")?);
        let mut rows = Vec::new();
        for line in lines {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split(',').collect();
            if f.len() <= csp {
                continue;
            }
            let g = |i: usize| -> f64 { f.get(i).and_then(|s| s.trim().parse().ok()).unwrap_or(0.0) };
            rows.push(Row {
                t: g(ct) / 1000.0,
                x: g(cx),
                y: g(cy),
                z: g(cz),
                vx: g(cvx),
                vy: g(cvy),
                vz: g(cvz),
                speed_ms: g(csp),
                ground: parse_bool(f.get(cg).copied().unwrap_or("")),
                gear: g(cgear),
                steer: g(cst),
                gas: g(cgas),
                brake: g(cbr),
            });
        }
        Ok(Traj {
            rows,
            path: path.to_string(),
        })
    }

    /// Horizontal speed, which is the one the glide question is about.
    pub fn vh(r: &Row) -> f64 {
        (r.vx * r.vx + r.vz * r.vz).sqrt()
    }
}
