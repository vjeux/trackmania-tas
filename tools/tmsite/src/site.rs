//! The full site: every sample of every run, as rounded JSON floats, inlined
//! into a self-contained page (port of `site/build_site.py`).

use crate::pyfmt::{json_str, pyformat, repr_f64, round_nd, Val};
use crate::traj::{load_dir, CHECKPOINTS};

const TMPL: &str = include_str!("../templates/site.html");

pub struct Opts {
    pub dir: String,
    pub out: String,
    pub stride: usize,
}

pub fn build(o: &Opts) -> Result<String, String> {
    let mut runs = load_dir(&o.dir, o.stride);
    runs.retain(|r| !r.samples.is_empty());
    if runs.is_empty() {
        return Err(format!("no paths in {}", o.dir));
    }
    // stable sort by time: files that tie keep filename order, as in CPython
    runs.sort_by_key(|r| r.time_ms);

    // round hard: this is what keeps the page small
    let rounded: Vec<Vec<[f64; 4]>> = runs
        .iter()
        .map(|r| {
            r.samples
                .iter()
                .map(|s| {
                    [
                        round_nd(s.x, 1),
                        round_nd(s.y, 1),
                        round_nd(s.z, 1),
                        round_nd(s.speed, 1),
                    ]
                })
                .collect()
        })
        .collect();

    let nsamp: usize = rounded.iter().map(|p| p.len()).sum();
    let vmax_raw = rounded
        .iter()
        .flat_map(|p| p.iter())
        .map(|p| p[3])
        .fold(f64::NEG_INFINITY, f64::max);
    let vmax = ((vmax_raw / 50.0).ceil() * 50.0) as i64;

    // json.dumps(runs, separators=(",", ":"))
    let mut js = String::with_capacity(nsamp * 26 + 4096);
    js.push('[');
    for (i, (r, p)) in runs.iter().zip(rounded.iter()).enumerate() {
        if i > 0 {
            js.push(',');
        }
        js.push_str("{\"name\":");
        js.push_str(&json_str(&r.name));
        js.push_str(",\"time\":");
        js.push_str(&r.time_ms.to_string());
        js.push_str(",\"cps\":[");
        for (k, c) in r.cps.iter().enumerate() {
            if k > 0 {
                js.push(',');
            }
            js.push_str(&c.to_string());
        }
        js.push_str("],\"p\":[");
        for (k, q) in p.iter().enumerate() {
            if k > 0 {
                js.push(',');
            }
            js.push('[');
            js.push_str(&repr_f64(q[0]));
            js.push(',');
            js.push_str(&repr_f64(q[1]));
            js.push(',');
            js.push_str(&repr_f64(q[2]));
            js.push(',');
            js.push_str(&repr_f64(q[3]));
            js.push(']');
        }
        js.push_str("]}");
    }
    js.push(']');

    // json.dumps(CHECKPOINTS) -- default separators, ", " / ": "
    let mut cps = String::from("{");
    for (i, (k, x, z)) in CHECKPOINTS.iter().enumerate() {
        if i > 0 {
            cps.push_str(", ");
        }
        cps.push_str(&format!(
            "{}: [{}, {}]",
            json_str(k),
            repr_f64(*x),
            repr_f64(*z)
        ));
    }
    cps.push('}');

    let html = pyformat(
        TMPL,
        &[
            ("n", Val::Int(runs.len() as i64)),
            ("runs", Val::Str(js)),
            ("cps", Val::Str(cps)),
            ("vmax", Val::Int(vmax)),
        ],
    );
    std::fs::write(&o.out, &html).map_err(|e| format!("write {}: {}", o.out, e))?;
    Ok(format!(
        "wrote {}  ({} runs, {} samples, {:.1} KB)",
        o.out,
        runs.len(),
        nsamp,
        html.len() as f64 / 1024.0
    ))
}
