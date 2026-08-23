//! A positional constraint that needs neither camera pose nor map geometry.
//!
//! The video does not say where the car was. But every human replay of the map
//! does say where a car CAN be, and on the stretches where the run being
//! reconstructed follows the normal route, a candidate that leaves the
//! envelope of the human lines has left the track — whatever its speed says.
//!
//! That matters because it is the exact failure this reconstruction hits: a
//! candidate that drives off the outside of a corner and falls keeps the
//! video's speed for another 0.7 s while it is already 16 m away and 4 m below,
//! so a speed-only objective PAYS for leaving the track. Distance to the
//! nearest human line at the same instant costs one lookup per sample and
//! catches it immediately.
//!
//! Its limit is written into it and must be published with any number it
//! produces: the corridor is only evidence where the run follows the route the
//! humans drive. Where it reroutes, the corridor is silent, and `--corridor-to`
//! is where the caller says so.

use std::collections::BTreeMap;

pub struct Corridor {
    /// race_ms -> the human positions recorded at that instant.
    pub lines: Vec<BTreeMap<i64, (f64, f64, f64)>>,
    /// Past this race time the corridor makes no claim.
    pub until_ms: i64,
}

impl Corridor {
    pub fn load(paths: &[String], until_ms: i64) -> Result<Corridor, String> {
        let mut lines = Vec::new();
        for p in paths {
            let txt = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
            let mut it = txt.lines();
            let hdr: Vec<&str> = it.next().ok_or("empty")?.split(',').collect();
            let c = |n: &str| hdr.iter().position(|h| *h == n).ok_or(format!("{p}: no {n}"));
            let (ct, cx, cy, cz) = (c("time_ms")?, c("x")?, c("y")?, c("z")?);
            let mut m = BTreeMap::new();
            for l in it {
                let f: Vec<&str> = l.split(',').collect();
                if let (Some(Ok(t)), Some(Ok(x)), Some(Ok(y)), Some(Ok(z))) = (
                    f.get(ct).map(|v| v.parse::<i64>()),
                    f.get(cx).map(|v| v.parse::<f64>()),
                    f.get(cy).map(|v| v.parse::<f64>()),
                    f.get(cz).map(|v| v.parse::<f64>()),
                ) {
                    m.insert(t, (x, y, z));
                }
            }
            if m.len() < 50 {
                return Err(format!("{p}: only {} samples", m.len()));
            }
            lines.push(m);
        }
        Ok(Corridor { lines, until_ms })
    }

    /// Distance from `p` to the nearest human position within `tol_ms` of `ms`.
    /// `None` where no human line has a sample there.
    pub fn distance(&self, ms: i64, p: (f64, f64, f64), tol_ms: i64) -> Option<f64> {
        let mut best: Option<f64> = None;
        for l in &self.lines {
            for (_, q) in l.range(ms - tol_ms..=ms + tol_ms) {
                let d = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2) + (p.2 - q.2).powi(2)).sqrt();
                if best.map_or(true, |b| d < b) {
                    best = Some(d);
                }
            }
        }
        best
    }

    /// The first race time at which a trajectory is further than `max_m` from
    /// every human line for `run` consecutive samples. `None` = never, inside
    /// the corridor's own window.
    pub fn departs(
        &self,
        traj: &BTreeMap<i64, (f64, f64, f64)>,
        max_m: f64,
        run: usize,
        tol_ms: i64,
    ) -> Option<i64> {
        let mut bad = 0usize;
        let mut first_bad = 0i64;
        for (ms, p) in traj {
            if *ms > self.until_ms {
                break;
            }
            let Some(d) = self.distance(*ms, *p, tol_ms) else { continue };
            if d > max_m {
                if bad == 0 {
                    first_bad = *ms;
                }
                bad += 1;
                if bad >= run {
                    return Some(first_bad);
                }
            } else {
                bad = 0;
            }
        }
        None
    }
}
