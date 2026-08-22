//! The compact variant: the same viewer, trajectories packed 6 bytes per sample
//! and base64'd (port of `code/build_compact.py`).
//!
//!   x: uint16  decimetres from the bounding-box origin
//!   z: uint16  decimetres
//!   y: uint8   decimetres above the lowest point
//!   v: uint8   km/h / 2

use crate::pyfmt::{b64, json_str, pyformat, round_half_even, Val};
use crate::traj::{load_dir, CHECKPOINTS};

const TMPL: &str = include_str!("../templates/compact.html");

pub struct Opts {
    pub dir: String,
    pub out: String,
    pub stride: usize,
    pub pick: usize,
}

pub fn build(o: &Opts) -> Result<String, String> {
    let mut runs = load_dir(&o.dir, o.stride);
    runs.retain(|r| r.samples.len() >= 8);
    if runs.is_empty() {
        return Err(format!("no paths in {}", o.dir));
    }
    runs.sort_by_key(|r| r.time_ms);

    if o.pick > 0 && o.pick < runs.len() {
        if o.pick < 2 {
            return Err("--pick must be >= 2".into());
        }
        // keep the fastest, the slowest, and an even spread between
        let n = runs.len();
        let mut idx: Vec<usize> = (0..o.pick)
            .map(|i| round_half_even(i as f64 * (n - 1) as f64 / (o.pick - 1) as f64) as usize)
            .collect();
        idx.sort_unstable();
        idx.dedup();
        let mut kept = Vec::with_capacity(idx.len());
        for i in idx {
            kept.push(std::mem::replace(
                &mut runs[i],
                crate::traj::Traj {
                    name: String::new(),
                    time_ms: 0,
                    cps: vec![],
                    samples: vec![],
                },
            ));
        }
        runs = kept;
    }

    let x0 = runs.iter().flat_map(|r| r.samples.iter()).map(|s| s.x).fold(f64::INFINITY, f64::min);
    let y0 = runs.iter().flat_map(|r| r.samples.iter()).map(|s| s.y).fold(f64::INFINITY, f64::min);
    let z0 = runs.iter().flat_map(|r| r.samples.iter()).map(|s| s.z).fold(f64::INFINITY, f64::min);

    let mut blob: Vec<u8> = Vec::with_capacity(runs.iter().map(|r| r.samples.len() * 6).sum());
    let mut meta = String::from("[");
    // The packing is lossy by clamp as well as by rounding, and silently so in
    // the Python: y only reaches 25.5 m above the lowest point and speed only
    // 510 km/h. Count the saturations so a map that outgrows the format says so
    // instead of quietly flattening.
    let mut clamped = [0usize; 4];
    for (i, r) in runs.iter().enumerate() {
        for s in &r.samples {
            let xi = clamp_u(round_half_even((s.x - x0) * 10.0), 65535.0, &mut clamped[0]) as u16;
            let zi = clamp_u(round_half_even((s.z - z0) * 10.0), 65535.0, &mut clamped[1]) as u16;
            let yi = clamp_u(round_half_even((s.y - y0) * 10.0), 255.0, &mut clamped[2]) as u8;
            let vi = clamp_u(round_half_even(s.speed / 2.0), 255.0, &mut clamped[3]) as u8;
            blob.extend_from_slice(&xi.to_le_bytes());
            blob.extend_from_slice(&zi.to_le_bytes());
            blob.push(yi);
            blob.push(vi);
        }
        if i > 0 {
            meta.push(',');
        }
        meta.push('[');
        meta.push_str(&json_str(&r.name));
        meta.push(',');
        meta.push_str(&r.time_ms.to_string());
        meta.push(',');
        meta.push_str(&r.samples.len().to_string());
        meta.push_str(",[");
        for (k, c) in r.cps.iter().enumerate() {
            if k > 0 {
                meta.push(',');
            }
            meta.push_str(&c.to_string());
        }
        meta.push_str("]]");
    }
    meta.push(']');

    let nsamp: usize = runs.iter().map(|r| r.samples.len()).sum();
    if clamped.iter().any(|&c| c > 0) {
        eprintln!(
            "warning: packing saturated -- x {} / z {} / y {} / speed {} samples out of range \
             (x,z limit 6553.5 m from origin, y 25.5 m above the lowest point, speed 510 km/h)",
            clamped[0], clamped[1], clamped[2], clamped[3]
        );
    }
    let vmax_raw = runs
        .iter()
        .flat_map(|r| r.samples.iter())
        .map(|s| s.speed)
        .fold(f64::NEG_INFINITY, f64::max);
    let vmax = ((vmax_raw / 50.0).ceil() * 50.0) as i64;

    let mut cps = String::from("{");
    for (i, (k, x, z)) in CHECKPOINTS.iter().enumerate() {
        if i > 0 {
            cps.push_str(", ");
        }
        cps.push_str(&format!(
            "{}: [{}, {}]",
            json_str(k),
            crate::pyfmt::repr_f64(*x),
            crate::pyfmt::repr_f64(*z)
        ));
    }
    cps.push('}');

    let html = pyformat(
        TMPL,
        &[
            ("n", Val::Int(runs.len() as i64)),
            ("meta", Val::Str(meta)),
            ("b64", Val::Str(b64(&blob))),
            ("x0", Val::Float(x0)),
            ("y0", Val::Float(y0)),
            ("z0", Val::Float(z0)),
            ("vmax", Val::Int(vmax)),
            ("cps", Val::Str(cps)),
            ("nsamp", Val::Int(nsamp as i64)),
        ],
    );
    std::fs::write(&o.out, &html).map_err(|e| format!("write {}: {}", o.out, e))?;
    Ok(format!(
        "{}  {} runs  {} samples  {:.1} KB",
        o.out,
        runs.len(),
        nsamp,
        html.len() as f64 / 1024.0
    ))
}

fn clamp_u(v: f64, hi: f64, count: &mut usize) -> f64 {
    if v < 0.0 || v > hi {
        *count += 1;
    }
    v.max(0.0).min(hi)
}
