//! Is the car ON THE TRACK at all?
//!
//! This is a general gate, not a Cobalt Cove one. A search whose objective is
//! any per-instant scalar — speed, height, distance to a goal — will happily
//! buy score with a candidate that has left the map, because a car in free
//! fall passes through plausible values on the way down. On this map that cost
//! half a second of an apparently-good reconstruction: the best speed-only
//! candidate left the pipe at race 12.2 and fell eight metres onto concrete,
//! and the objective kept paying it for another 0.7 s.
//!
//! The test is the one that caught it: sample the trajectory, and ask the map
//! whether there is a surface under the car. `mapgeom plumb` answers exactly
//! that, and it takes many points per invocation — which matters, because
//! building the triangle index costs minutes and the plumbing itself costs
//! nothing.
//!
//! What it CANNOT be used for, and the reason this is a checker rather than a
//! search objective: the model has holes. On this map 12–15 % of a real human
//! run has no triangle beneath it. So "no surface" is evidence about a
//! CANDIDATE only where a human run at the same place has one — which is why
//! this reports the two together and never fails a candidate on its own.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::Command;

pub struct Sample {
    pub race_ms: i64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub fn load_traj(path: &str, from_ms: i64, to_ms: i64, every_ms: i64) -> Result<Vec<Sample>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut it = txt.lines();
    let hdr: Vec<&str> = it.next().ok_or("empty")?.split(',').collect();
    let c = |n: &str| hdr.iter().position(|h| *h == n).ok_or(format!("no {n}"));
    let (ct, cx, cy, cz) = (c("time_ms")?, c("x")?, c("y")?, c("z")?);
    let mut v = Vec::new();
    for l in it {
        let f: Vec<&str> = l.split(',').collect();
        let g = |i: usize| f.get(i).and_then(|x| x.parse::<f64>().ok());
        let (Some(t), Some(x), Some(y), Some(z)) =
            (f.get(ct).and_then(|x| x.parse::<i64>().ok()), g(cx), g(cy), g(cz))
        else {
            continue;
        };
        if t >= from_ms && t <= to_ms && t % every_ms == 0 {
            v.push(Sample { race_ms: t, x, y, z });
        }
    }
    Ok(v)
}

/// Run one `mapgeom plumb` over every point and return, per point, the highest
/// surface at or below the car and its material.
pub fn plumb(
    mapgeom: &str,
    map: &str,
    yoff: &str,
    pts: &[Sample],
    drop_max: f64,
) -> Result<Vec<Option<(f64, String)>>, String> {
    let mut cmd = Command::new(mapgeom);
    cmd.arg("plumb").arg(map).arg("--yoff").arg(yoff);
    for p in pts {
        cmd.arg("--at").arg(format!("{:.3},{:.3}", p.x, p.z));
    }
    let out = cmd.output().map_err(|e| format!("{mapgeom}: {e}"))?;
    let txt = String::from_utf8_lossy(&out.stdout);
    // The output is one block per point, in order; a block starts with a line
    // naming the point and continues with "  y <height>   <material>" rows.
    let mut per_point: Vec<Vec<(f64, String)>> = Vec::new();
    for line in txt.lines() {
        let t = line.trim();
        if t.starts_with("y ") {
            let mut w = t[2..].split_whitespace();
            if let (Some(Ok(h)), Some(m)) = (w.next().map(|v| v.parse::<f64>()), w.next()) {
                if let Some(last) = per_point.last_mut() {
                    last.push((h, m.to_string()));
                }
            }
        } else if t.starts_with("column at ") {
            per_point.push(Vec::new());
        }
    }
    if per_point.len() != pts.len() {
        return Err(format!(
            "plumb returned {} columns for {} points -- the output format moved",
            per_point.len(),
            pts.len()
        ));
    }
    Ok(pts
        .iter()
        .zip(per_point)
        .map(|(p, col)| {
            col.into_iter()
                .filter(|(h, _)| *h <= p.y + 0.5 && p.y - h <= drop_max)
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        })
        .collect())
}

/// Report a candidate's on-surface record beside a reference run's, at the
/// same race instants. Neither column means anything without the other.
pub fn report(
    label: &str,
    pts: &[Sample],
    cand: &[Option<(f64, String)>],
    reference: &BTreeMap<i64, bool>,
    o: &mut impl Write,
) {
    writeln!(o, "race_ms\tcandidate\tmaterial\thuman_here").unwrap();
    let (mut both, mut cand_off_human_on) = (0usize, 0usize);
    let mut first_off: Option<i64> = None;
    for (p, c) in pts.iter().zip(cand) {
        let human = reference.get(&p.race_ms).copied();
        let on = c.is_some();
        writeln!(
            o,
            "{}\t{}\t{}\t{}",
            p.race_ms,
            if on { "on" } else { "OFF" },
            c.as_ref().map(|x| x.1.as_str()).unwrap_or("-"),
            match human {
                Some(true) => "on",
                Some(false) => "off",
                None => "?",
            }
        )
        .unwrap();
        if human == Some(true) {
            both += 1;
            if !on {
                cand_off_human_on += 1;
                first_off.get_or_insert(p.race_ms);
            }
        }
    }
    writeln!(
        o,
        "# {label}: at {both} instants a human IS on a surface; the candidate is not at {cand_off_human_on} of them"
    )
    .unwrap();
    match first_off {
        Some(t) => writeln!(o, "# first such instant: race {:.3} s", t as f64 / 1000.0).unwrap(),
        None => writeln!(o, "# the candidate is on a surface wherever a human is").unwrap(),
    }
}
