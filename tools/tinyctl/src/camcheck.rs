//! `tinyctl camcheck` — the MediaTracker transform, checked in numbers.
//!
//! ```text
//! tinyctl camcheck --orig cam-ORIG.tsv --tiny cam-TINY.tsv --anchor sx,sy,sz:tx,ty,tz
//!                  [--scale 0.5] [--step-ms 250] [--trigger x0,y0,z0:x1,y1,z1]
//! ```
//!
//! Both files are `shootctl playshots --camlog-ms` logs (a row per frame:
//! wall_ms, race t_ms, car px py pz, camera cx cy cz, fov) of the same map,
//! the original and the tiny build, each started when its playground opened.
//! The two clocks are aligned on the intro's FIRST CAMERA CUT — the intro is
//! three camera blocks, and the camera jumps tens of metres where one ends
//! and the next begins; that instant is the same clip time on both sides.
//! From there, every `--step-ms` the original's camera goes through the
//! items' transform and is compared with the tiny build's: the error column
//! is what the transform got wrong (0 = the tiny intro is the original's,
//! shrunk). The fov is compared as is (it must not change).
//!
//! `--trigger` (a world box in TINY coordinates) reports when the tiny car
//! entered it and when the tiny camera left the chase position — the in-game
//! trigger test: the camera should jump within a frame of the entry.

use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
struct Row {
    wall: f64,
    race: Option<f64>,
    car: Option<[f64; 3]>,
    cam: Option<[f64; 3]>,
    fov: Option<f64>,
}

fn parse_log(path: &PathBuf) -> Result<Vec<Row>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 9 {
            continue;
        }
        let num = |s: &str| s.trim().parse::<f64>().ok();
        let wall = num(f[0]).ok_or_else(|| format!("{}:{}: bad wall_ms", path.display(), i + 1))?;
        let race = num(f[1]);
        let car = match (num(f[2]), num(f[3]), num(f[4])) {
            (Some(x), Some(y), Some(z)) => Some([x, y, z]),
            _ => None,
        };
        let cam = match (num(f[5]), num(f[6]), num(f[7])) {
            (Some(x), Some(y), Some(z)) => Some([x, y, z]),
            _ => None,
        };
        rows.push(Row { wall, race, car, cam, fov: num(f[8]) });
    }
    if rows.is_empty() {
        return Err(format!("{}: no rows", path.display()));
    }
    Ok(rows)
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The wall time of the first camera cut: a jump of more than `min_jump` m
/// between consecutive frames, after the camera has moved at all (the first
/// frames of a load sit on a default camera).
fn first_cut(rows: &[Row], min_jump: f64) -> Option<f64> {
    let mut prev: Option<(f64, [f64; 3])> = None;
    let mut moved = false;
    for r in rows {
        let Some(c) = r.cam else { continue };
        if let Some((_, p)) = prev {
            let d = dist(p, c);
            if d > 0.01 {
                moved = true;
            }
            if moved && d > min_jump {
                return Some(r.wall);
            }
        }
        prev = Some((r.wall, c));
    }
    None
}

/// The camera at wall time `t` (the nearest frame within 40 ms).
fn cam_at(rows: &[Row], t: f64) -> Option<(f64, [f64; 3], Option<f64>)> {
    let mut best: Option<(f64, [f64; 3], Option<f64>)> = None;
    for r in rows {
        let Some(c) = r.cam else { continue };
        let d = (r.wall - t).abs();
        if d <= 40.0 && best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
            best = Some((d, c, r.fov));
        }
    }
    best
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let orig = parse_log(&PathBuf::from(f("--orig").ok_or("camcheck needs --orig cam-ORIG.tsv")?))?;
    let tiny = parse_log(&PathBuf::from(f("--tiny").ok_or("camcheck needs --tiny cam-TINY.tsv")?))?;
    let anchor = f("--anchor").ok_or("camcheck needs --anchor sx,sy,sz:tx,ty,tz (tmmaps tiny prints it)")?;
    let (sa, ta) = anchor.split_once(':').ok_or("--anchor wants sx,sy,sz:tx,ty,tz")?;
    let v3 = |s: &str| -> Result<[f64; 3], String> {
        let v: Vec<f64> = s.split(',').map(|x| x.trim().parse::<f64>().map_err(|_| format!("bad number in {s}"))).collect::<Result<_, _>>()?;
        if v.len() != 3 {
            return Err(format!("{s}: three numbers wanted"));
        }
        Ok([v[0], v[1], v[2]])
    };
    let (sa, ta) = (v3(sa)?, v3(ta)?);
    let scale: f64 = f("--scale").map(|s| s.parse().map_err(|_| "--scale wants a number")).transpose()?.unwrap_or(0.5);
    let step: f64 = f("--step-ms").map(|s| s.parse().map_err(|_| "--step-ms wants a number")).transpose()?.unwrap_or(250.0);
    let transform = |p: [f64; 3]| [ta[0] + (p[0] - sa[0]) * scale, ta[1] + (p[1] - sa[1]) * scale, ta[2] + (p[2] - sa[2]) * scale];

    let cut_o = first_cut(&orig, 20.0).ok_or("original log: no camera cut found (is the intro in the log?)")?;
    let cut_t = first_cut(&tiny, 10.0).ok_or("tiny log: no camera cut found (is the intro in the log?)")?;
    println!("first camera cut: original at {:.3} s, tiny at {:.3} s into the logs (clocks aligned there)", (cut_o - orig[0].wall) / 1000.0, (cut_t - tiny[0].wall) / 1000.0);
    println!("{:>7}  {:>28}  {:>28}  {:>28}  {:>7}  {:>5}", "t(s)", "original camera", "expected tiny camera", "tiny camera", "err(m)", "fov");
    let mut errs: Vec<f64> = Vec::new();
    let mut fov_bad = 0usize;
    let mut k = -12i64; // 3 s before the cut .. 8 s after
    while (k as f64) * step <= 8000.0 {
        let dt = k as f64 * step;
        k += 1;
        let (Some((_, co, fo)), Some((_, ct, ft))) = (cam_at(&orig, cut_o + dt), cam_at(&tiny, cut_t + dt)) else { continue };
        let e = transform(co);
        let err = dist(e, ct);
        errs.push(err);
        let fov_note = match (fo, ft) {
            (Some(a), Some(b)) if (a - b).abs() > 0.5 => {
                fov_bad += 1;
                format!("{a:.1}≠{b:.1}")
            }
            (Some(a), _) => format!("{a:.1}"),
            _ => String::new(),
        };
        println!("{:>7.2}  {:>8.2} {:>8.2} {:>8.2}   {:>8.2} {:>8.2} {:>8.2}   {:>8.2} {:>8.2} {:>8.2}  {:>7.2}  {:>5}", dt / 1000.0, co[0], co[1], co[2], e[0], e[1], e[2], ct[0], ct[1], ct[2], err, fov_note);
    }
    if errs.is_empty() {
        return Err("no comparable samples".into());
    }
    let mut sorted = errs.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let max = sorted[sorted.len() - 1];
    println!("{} samples: median error {median:.2} m, max {max:.2} m (the cut instants themselves may straddle a frame); fov differs on {fov_bad}", errs.len());

    if let Some(tb) = f("--trigger") {
        let (lo, hi) = tb.split_once(':').ok_or("--trigger wants x0,y0,z0:x1,y1,z1")?;
        let (lo, hi) = (v3(lo)?, v3(hi)?);
        let inside = |p: [f64; 3]| (0..3).all(|k| p[k] >= lo[k].min(hi[k]) && p[k] <= lo[k].max(hi[k]));
        let entered = tiny.iter().find(|r| r.car.map(inside).unwrap_or(false));
        // the chase camera sits behind the car: a camera-to-car distance under ~25 m;
        // a MediaTracker camera is wherever the clip put it
        let racing: Vec<&Row> = tiny.iter().filter(|r| r.race.map(|t| t > 0.0).unwrap_or(false) && r.car.is_some() && r.cam.is_some()).collect();
        let jump = racing.windows(2).find(|w| dist(w[0].cam.unwrap(), w[1].cam.unwrap()) > 8.0);
        match entered {
            Some(r) => println!("tiny car entered the trigger box at race {:.3} s (car {:.1},{:.1},{:.1})", r.race.unwrap_or(0.0) / 1000.0, r.car.unwrap()[0], r.car.unwrap()[1], r.car.unwrap()[2]),
            None => println!("tiny car never entered the trigger box {lo:?}..{hi:?}"),
        }
        match jump {
            Some(w) => println!("tiny camera jumped at race {:.3} s: {:.1},{:.1},{:.1} -> {:.1},{:.1},{:.1}", w[1].race.unwrap_or(0.0) / 1000.0, w[0].cam.unwrap()[0], w[0].cam.unwrap()[1], w[0].cam.unwrap()[2], w[1].cam.unwrap()[0], w[1].cam.unwrap()[1], w[1].cam.unwrap()[2]),
            None => println!("tiny camera never jumped during the race (no in-game clip fired)"),
        }
    }
    Ok(())
}
