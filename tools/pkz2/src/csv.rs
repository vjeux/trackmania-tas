//! One reader for both trajectory dialects on this map.
//!
//! Two CSV shapes carry a car's path here and they are NOT the same file
//! format:
//!
//! * the **human recording**, decoded out of the `.Ghost.Gbx` — a header line
//!   naming 29 columns, `time_ms,x,y,z,speed_kmh,...`, sampled every 50 ms,
//!   carrying gas/brake/steer and the derived contact bit;
//! * the **simulator readout** (`fk rtraj`) — `time_ms,x,y,z,kmh[,gas,brake,
//!   steer]`, every 10 ms.
//!
//! Reading them with two parsers is how a project ends up quoting a column
//! that means something else, so this reads BOTH by header name where a header
//! exists and by position where it does not, and every consumer downstream
//! sees one `Sample`.

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    /// race time, seconds
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// km/h
    pub v: f64,
    pub vy: Option<f64>,
    pub gas: Option<f64>,
    pub brake: Option<f64>,
    pub steer: Option<f64>,
    pub ground: Option<bool>,
}

fn num(s: &str) -> Option<f64> {
    let s = s.trim();
    s.parse::<f64>().ok()
}

fn boolish(s: &str) -> Option<bool> {
    match s.trim() {
        "True" | "true" | "1" => Some(true),
        "False" | "false" | "0" => Some(false),
        _ => None,
    }
}

/// Read a trajectory CSV. `t0`/`t1` are race seconds, inclusive.
pub fn read(path: &str, t0: f64, t1: f64) -> Result<Vec<Sample>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut idx: Option<std::collections::HashMap<String, usize>> = None;
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = if line.contains(",") { line.split(",").collect() } else { line.split_whitespace().collect() };
        if f.len() < 5 {
            continue;
        }
        if num(f[0]).is_none() {
            // a header line
            let mut m = std::collections::HashMap::new();
            for (i, name) in f.iter().enumerate() {
                m.insert(name.trim().to_string(), i);
            }
            idx = Some(m);
            continue;
        }
        let get = |name: &str, fallback: Option<usize>| -> Option<f64> {
            if let Some(m) = &idx {
                if let Some(i) = m.get(name) {
                    return f.get(*i).and_then(|s| num(s));
                }
                if fallback.is_none() {
                    return None;
                }
            }
            fallback.and_then(|i| f.get(i)).and_then(|s| num(s))
        };
        let t = match get("race_s", None) {
            Some(v) => v,
            None => match get("time_ms", Some(0)) {
                Some(v) => v / 1000.0,
                None => continue,
            },
        };
        if t < t0 || t > t1 {
            continue;
        }
        let v = get("speed_kmh", None).or_else(|| get("v_kmh", None)).or_else(|| get("kmh", Some(4)));
        let (x, y, z) = (get("x", Some(1)), get("y", Some(2)), get("z", Some(3)));
        let (x, y, z, v) = match (x, y, z, v) {
            (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
            _ => continue,
        };
        let ground = idx
            .as_ref()
            .and_then(|m| m.get("is_ground_contact"))
            .and_then(|i| f.get(*i))
            .and_then(|s| boolish(s));
        out.push(Sample {
            t,
            x,
            y,
            z,
            v,
            vy: get("vy", None),
            gas: get("gas", Some(5)),
            brake: get("brake", Some(6)),
            steer: get("steer", Some(7)),
            ground,
        });
    }
    if out.is_empty() {
        return Err(format!("{}: no samples in [{}, {}]", path, t0, t1));
    }
    Ok(out)
}
