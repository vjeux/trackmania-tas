//! `uwlab` — read-only analysis for 173691 "Spring 2023-15 (Underwater)".
//!
//! The map is the Nadeo campaign map with the whole volume filled with water,
//! and every question it poses is about the WATER REGIME: how hard does the
//! water pull the car down, how fast does it take a glide apart, and can a
//! given launch state still be inside the finish gate's box when it gets
//! there. Those are trajectory questions, so this crate only ever reads a
//! trajectory CSV. It writes no ghost and edits no map.

mod blitz;
mod chain;
mod climb;
mod platladder;
mod skyline;
mod sweep;
mod traj;

use traj::{Row, Traj};

const USAGE: &str = "\
uwlab — trajectory analysis for the underwater map. Times print as seconds.

  rows   CSV [--t A:B] [--y A:B] [--z A:B] [--air] [--ground] [--every N]
  drag   CSV --t A:B            fit the water law over a free-flight window
  box    CSV --box x0,y0,z0:x1,y1,z1 [--t A:B]
                                did the run enter the box; if not, how far off
  glide  CSV [--min-air S]      every airborne stretch, with its reach and sink
  reach  --from x,y,z --vel vx,vy,vz --law g,k1,k2,h1,h2 [--secs S]
                                forward-integrate the fitted law from a launch
  probemap --map M --tape G --tmmaps P --fk P [--cx A:B] [--cz A:B] [--cy N] [--jobs N]
                                a plumb-probe LATTICE: one column per 32 m cell
  maxy   CSV [--after S]          the highest the car ever got, and where
  plumb  CSV [--after S]         where a dropped car first stops falling
  launch CENSUS.tsv --box x0,y0,z0:x1,y1,z1 [--v M/S] [--filter PAT]
                                every surface that could ballistically reach the box
  tape   --out F.gtape [--ticks N] [--start-offset MS] --seg t0:t1:steer:accel:brake ...
  cols   REGION.tsv [--filter PAT] [--x A:B] [--z A:B] [--ymin Y]
                                a `tmmaps region` dump as a COLUMN MAP: what is
                                stacked over each 32 m cell, and how high
  sweep  --map M --carrier G --template T.gtape --spawns cx,cy,cz,dir
         [--tape NAME=segs] [--plan F] [--box B] [--jobs N] [--dir D]
                                DIRECTED launches: spawn (with a heading) x
                                tape, traced and scored per axis
";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        print!("{USAGE}");
        std::process::exit(2);
    }
    let rest = &argv[1..];
    let code = match argv[0].as_str() {
        "rows" => cmd_rows(rest),
        "drag" => cmd_drag(rest),
        "box" => cmd_box(rest),
        "glide" => cmd_glide(rest),
        "reach" => cmd_reach(rest),
        "cols" => cmd_cols(rest),
        "tape" => cmd_tape(rest),
        "launch" => cmd_launch(rest),
        "plumb" => cmd_plumb(rest),
        "maxy" => cmd_maxy(rest),
        "probemap" => cmd_probemap(rest),
        "sweep" => sweep::cmd_sweep(rest),
        "skyline" => skyline::cmd_skyline(rest),
        "chain" => chain::cmd_chain(rest),
        "lattice" => skyline::cmd_lattice(rest),
        "blitz" => blitz::cmd_blitz(rest),
        "platladder" => platladder::cmd_platladder(rest),
        "climb" => climb::cmd_climb(rest),
        other => {
            eprintln!("uwlab: unknown command `{other}`");
            print!("{USAGE}");
            2
        }
    };
    std::process::exit(code);
}

// ---------------------------------------------------------------- arg helpers

fn flag_val(a: &[String], name: &str) -> Option<String> {
    a.iter().position(|s| s == name).and_then(|i| a.get(i + 1)).cloned()
}
fn has(a: &[String], name: &str) -> bool {
    a.iter().any(|s| s == name)
}
fn range(a: &[String], name: &str) -> Option<(f64, f64)> {
    let v = flag_val(a, name)?;
    let (lo, hi) = v.split_once(':')?;
    Some((lo.parse().ok()?, hi.parse().ok()?))
}
fn triple(s: &str) -> Option<(f64, f64, f64)> {
    let p: Vec<&str> = s.split(',').collect();
    if p.len() != 3 {
        return None;
    }
    Some((p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?))
}
fn positional(a: &[String]) -> Vec<String> {
    // Everything that is not a flag and not a flag's value.
    let mut out = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if a[i].starts_with("--") {
            // Boolean flags take no value; the value-taking ones all do.
            let boolean = matches!(a[i].as_str(), "--air" | "--ground");
            i += if boolean { 1 } else { 2 };
        } else {
            out.push(a[i].clone());
            i += 1;
        }
    }
    out
}

fn load_one(a: &[String]) -> Traj {
    let p = positional(a);
    let Some(path) = p.first() else {
        eprintln!("uwlab: need a trajectory CSV");
        std::process::exit(2);
    };
    match Traj::load(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("uwlab: {e}");
            std::process::exit(2);
        }
    }
}

// ------------------------------------------------------------------ rows

fn cmd_rows(a: &[String]) -> i32 {
    let t = load_one(a);
    let tr = range(a, "--t");
    let yr = range(a, "--y");
    let zr = range(a, "--z");
    let every: usize = flag_val(a, "--every").and_then(|s| s.parse().ok()).unwrap_or(1);
    let air = has(a, "--air");
    let ground = has(a, "--ground");
    println!("t       x         y        z         vx      vy      vz      |vh|    |v|     grnd gear steer gas brk");
    let mut n = 0usize;
    for r in &t.rows {
        if let Some((lo, hi)) = tr {
            if r.t < lo || r.t > hi {
                continue;
            }
        }
        if let Some((lo, hi)) = yr {
            if r.y < lo || r.y > hi {
                continue;
            }
        }
        if let Some((lo, hi)) = zr {
            if r.z < lo || r.z > hi {
                continue;
            }
        }
        if air && r.ground {
            continue;
        }
        if ground && !r.ground {
            continue;
        }
        if n % every == 0 {
            println!(
                "{:7.3} {:9.3} {:8.3} {:9.3} {:7.2} {:7.2} {:7.2} {:7.2} {:7.2} {:>4} {:>4} {:5.2} {:3.0} {:3.0}",
                r.t, r.x, r.y, r.z, r.vx, r.vy, r.vz, Traj::vh(r), r.speed_ms,
                if r.ground { "G" } else { "-" }, r.gear, r.steer, r.gas, r.brake
            );
        }
        n += 1;
    }
    eprintln!("{} rows ({} printed) from {}", n, n.div_ceil(every.max(1)), t.path);
    0
}

// ------------------------------------------------------------------ drag

/// Least squares for `y = c0*b0 + c1*b1 + c2*b2` by normal equations.
fn solve3(rows: &[([f64; 3], f64)]) -> [f64; 3] {
    let mut ata = [[0.0f64; 3]; 3];
    let mut atb = [0.0f64; 3];
    for (b, y) in rows {
        for i in 0..3 {
            for j in 0..3 {
                ata[i][j] += b[i] * b[j];
            }
            atb[i] += b[i] * y;
        }
    }
    // Gaussian elimination with partial pivoting.
    let mut m = [[0.0f64; 4]; 3];
    for i in 0..3 {
        m[i][..3].copy_from_slice(&ata[i]);
        m[i][3] = atb[i];
    }
    for c in 0..3 {
        let mut piv = c;
        for r in c + 1..3 {
            if m[r][c].abs() > m[piv][c].abs() {
                piv = r;
            }
        }
        m.swap(c, piv);
        if m[c][c].abs() < 1e-12 {
            continue;
        }
        let d = m[c][c];
        for k in c..4 {
            m[c][k] /= d;
        }
        for r in 0..3 {
            if r != c {
                let f = m[r][c];
                for k in c..4 {
                    m[r][k] -= f * m[c][k];
                }
            }
        }
    }
    [m[0][3], m[1][3], m[2][3]]
}

fn rms(rows: &[([f64; 3], f64)], c: [f64; 3]) -> f64 {
    if rows.is_empty() {
        return f64::NAN;
    }
    let s: f64 = rows
        .iter()
        .map(|(b, y)| {
            let p = c[0] * b[0] + c[1] * b[1] + c[2] * b[2];
            (p - y) * (p - y)
        })
        .sum();
    (s / rows.len() as f64).sqrt()
}

fn cmd_drag(a: &[String]) -> i32 {
    let t = load_one(a);
    let (lo, hi) = match range(a, "--t") {
        Some(r) => r,
        None => {
            eprintln!("uwlab drag: --t A:B is required (a free-flight window)");
            return 2;
        }
    };
    let w: Vec<&Row> = t.rows.iter().filter(|r| r.t >= lo && r.t <= hi).collect();
    if w.len() < 6 {
        eprintln!("uwlab drag: only {} samples in the window", w.len());
        return 2;
    }
    let ng = w.iter().filter(|r| r.ground).count();
    println!("window {:.3}..{:.3}  {} samples, {} with ground contact", lo, hi, w.len(), ng);
    if ng > 0 {
        println!("  WARNING: the window is not pure free flight; the fit is about the road, not the water.");
    }

    // Central differences of the recorded velocity, which is what an
    // acceleration must be measured from -- differencing POSITION twice adds
    // the encoder's own noise to the answer.
    let mut vert: Vec<([f64; 3], f64)> = Vec::new();
    let mut horz: Vec<([f64; 3], f64)> = Vec::new();
    for i in 1..w.len() - 1 {
        let dt = w[i + 1].t - w[i - 1].t;
        if dt <= 0.0 {
            continue;
        }
        let ay = (w[i + 1].vy - w[i - 1].vy) / dt;
        let vy = w[i].vy;
        // a_y = -g - k1*vy - k2*vy*|vy|   (signs so that all three are positive
        // for a car being pulled down and slowed by water)
        vert.push(([-1.0, -vy, -vy * vy.abs()], ay));

        let vh0 = Traj::vh(w[i - 1]);
        let vh2 = Traj::vh(w[i + 1]);
        let ah = (vh2 - vh0) / dt;
        let vh = Traj::vh(w[i]);
        horz.push(([-1.0, -vh, -vh * vh], ah));
    }

    println!("\nVERTICAL   a_y = -g - k1*vy - k2*vy|vy|");
    report_fits(&vert, "g ", "k1", "k2");
    let vymin = w.iter().map(|r| r.vy).fold(f64::MAX, f64::min);
    let vymax = w.iter().map(|r| r.vy).fold(f64::MIN, f64::max);
    println!("  vy spans {vymin:.3} .. {vymax:.3} m/s  (a law fitted over a narrow span cannot be extrapolated)");

    println!("\nHORIZONTAL d|vh|/dt = -c0 - c1*vh - c2*vh^2");
    report_fits(&horz, "c0", "c1", "c2");
    let vhmin = w.iter().map(|r| Traj::vh(r)).fold(f64::MAX, f64::min);
    let vhmax = w.iter().map(|r| Traj::vh(r)).fold(f64::MIN, f64::max);
    println!("  |vh| spans {vhmin:.3} .. {vhmax:.3} m/s");
    0
}

fn report_fits(rows: &[([f64; 3], f64)], n0: &str, n1: &str, n2: &str) {
    // Three nested laws, so that "which law" is answered by the residual
    // rather than assumed. A law with more freedom always fits better; what
    // matters is whether it fits BETTER THAN THE NOISE.
    let lin: Vec<([f64; 3], f64)> = rows.iter().map(|(b, y)| ([b[0], b[1], 0.0], *y)).collect();
    let quad: Vec<([f64; 3], f64)> = rows.iter().map(|(b, y)| ([b[0], 0.0, b[2]], *y)).collect();
    let cl = solve3(&lin);
    let cq = solve3(&quad);
    let cb = solve3(rows);
    println!(
        "  linear    {n0}={:8.4}  {n1}={:8.5}                 rms {:.4}",
        cl[0], cl[1], rms(&lin, cl)
    );
    println!(
        "  quadratic {n0}={:8.4}                  {n2}={:9.6}  rms {:.4}",
        cq[0], cq[2], rms(&quad, cq)
    );
    println!(
        "  both      {n0}={:8.4}  {n1}={:8.5}  {n2}={:9.6}  rms {:.4}",
        cb[0], cb[1], cb[2], rms(rows, cb)
    );
    if cl[1].abs() > 1e-9 {
        println!("  -> linear terminal {:.3} m/s, e-folding {:.3} s, asymptotic reach v0/{:.4}", cl[0] / cl[1], 1.0 / cl[1], cl[1]);
    }
    if cq[2].abs() > 1e-12 && cq[0] > 0.0 {
        println!("  -> quadratic terminal {:.3} m/s (no asymptotic reach: distance grows as ln t)", (cq[0] / cq[2]).sqrt());
    }
}

// ------------------------------------------------------------------ box

struct Bx {
    x0: f64,
    y0: f64,
    z0: f64,
    x1: f64,
    y1: f64,
    z1: f64,
}

impl Bx {
    fn parse(s: &str) -> Option<Bx> {
        let (a, b) = s.split_once(':')?;
        let (x0, y0, z0) = triple(a)?;
        let (x1, y1, z1) = triple(b)?;
        Some(Bx {
            x0: x0.min(x1),
            y0: y0.min(y1),
            z0: z0.min(z1),
            x1: x0.max(x1),
            y1: y0.max(y1),
            z1: z0.max(z1),
        })
    }
    /// Distance from a point to the box; 0 inside.
    fn miss(&self, x: f64, y: f64, z: f64) -> f64 {
        let dx = (self.x0 - x).max(x - self.x1).max(0.0);
        let dy = (self.y0 - y).max(y - self.y1).max(0.0);
        let dz = (self.z0 - z).max(z - self.z1).max(0.0);
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
    /// How far INSIDE the box, for a point that is in it.
    fn margin(&self, x: f64, y: f64, z: f64) -> f64 {
        let dx = (x - self.x0).min(self.x1 - x);
        let dy = (y - self.y0).min(self.y1 - y);
        let dz = (z - self.z0).min(self.z1 - z);
        dx.min(dy).min(dz)
    }
}

fn cmd_box(a: &[String]) -> i32 {
    let t = load_one(a);
    let Some(bs) = flag_val(a, "--box") else {
        eprintln!("uwlab box: --box x0,y0,z0:x1,y1,z1 is required");
        return 2;
    };
    let Some(b) = Bx::parse(&bs) else {
        eprintln!("uwlab box: cannot parse `{bs}`");
        return 2;
    };
    let tr = range(a, "--t");
    let mut best = f64::MAX;
    let mut bestr: Option<Row> = None;
    let mut inside: Vec<&Row> = Vec::new();
    for r in &t.rows {
        if let Some((lo, hi)) = tr {
            if r.t < lo || r.t > hi {
                continue;
            }
        }
        let m = b.miss(r.x, r.y, r.z);
        if m < best {
            best = m;
            bestr = Some(r.clone());
        }
        if m == 0.0 {
            inside.push(r);
        }
    }
    let Some(r) = bestr else {
        eprintln!("uwlab box: no samples in the time window");
        return 2;
    };
    if !inside.is_empty() {
        let f = inside[0];
        let l = inside[inside.len() - 1];
        let mm = inside
            .iter()
            .map(|r| b.margin(r.x, r.y, r.z))
            .fold(f64::MIN, f64::max);
        println!(
            "INSIDE  {} samples, {:.3}..{:.3}  best margin {:.3} m",
            inside.len(),
            f.t,
            l.t,
            mm
        );
        println!(
            "  entry  t={:.3} ({:.2}, {:.2}, {:.2})  |v| {:.2} m/s",
            f.t, f.x, f.y, f.z, f.speed_ms
        );
        return 0;
    }
    println!(
        "MISS {:.3} m  at t={:.3} ({:.2}, {:.2}, {:.2})  |v| {:.2} vy {:.2}",
        best, r.t, r.x, r.y, r.z, r.speed_ms, r.vy
    );
    // Per-axis, because "16 m short" and "16 m low" want different fixes.
    let dx = (b.x0 - r.x).max(r.x - b.x1).max(0.0);
    let dy = (b.y0 - r.y).max(r.y - b.y1).max(0.0);
    let dz = (b.z0 - r.z).max(r.z - b.z1).max(0.0);
    println!("  short by  x {dx:.3}   y {dy:.3}   z {dz:.3}");
    1
}

// ------------------------------------------------------------------ glide

fn cmd_glide(a: &[String]) -> i32 {
    let t = load_one(a);
    let minair: f64 = flag_val(a, "--min-air")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.5);
    let mut i = 0usize;
    let n = t.rows.len();
    println!("start_t   end_t   dur    launch(x,y,z)                v0    vy0    reach_h   sink   apex_gain  end(x,y,z)");
    while i < n {
        if t.rows[i].ground {
            i += 1;
            continue;
        }
        let s = i;
        while i < n && !t.rows[i].ground {
            i += 1;
        }
        let e = i - 1;
        let dur = t.rows[e].t - t.rows[s].t;
        if dur < minair {
            continue;
        }
        let a0 = &t.rows[s];
        let a1 = &t.rows[e];
        let reach = ((a1.x - a0.x).powi(2) + (a1.z - a0.z).powi(2)).sqrt();
        let ymax = t.rows[s..=e].iter().map(|r| r.y).fold(f64::MIN, f64::max);
        println!(
            "{:7.3} {:7.3} {:6.3}  ({:8.2},{:7.2},{:8.2})  {:6.2} {:6.2}  {:7.2}  {:7.2}  {:7.2}   ({:8.2},{:7.2},{:8.2})",
            a0.t, a1.t, dur, a0.x, a0.y, a0.z, Traj::vh(a0), a0.vy, reach, a0.y - a1.y, ymax - a0.y, a1.x, a1.y, a1.z
        );
    }
    0
}

// ------------------------------------------------------------------ reach

fn cmd_reach(a: &[String]) -> i32 {
    let Some(from) = flag_val(a, "--from").and_then(|s| triple(&s)) else {
        eprintln!("uwlab reach: --from x,y,z");
        return 2;
    };
    let Some(vel) = flag_val(a, "--vel").and_then(|s| triple(&s)) else {
        eprintln!("uwlab reach: --vel vx,vy,vz");
        return 2;
    };
    let law: Vec<f64> = flag_val(a, "--law")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    if law.len() != 5 {
        eprintln!("uwlab reach: --law g,k1,k2,h1,h2 (vertical g/k1/k2, horizontal h1/h2)");
        return 2;
    }
    let (g, k1, k2, h1, h2) = (law[0], law[1], law[2], law[3], law[4]);
    let secs: f64 = flag_val(a, "--secs").and_then(|s| s.parse().ok()).unwrap_or(40.0);
    let dt = 0.01;
    let (mut x, mut y, mut z) = from;
    let (mut vx, mut vy, mut vz) = vel;
    let mut t = 0.0;
    println!("t      x         y        z         vh      vy");
    while t <= secs {
        if (t * 100.0).round() as i64 % 50 == 0 {
            println!(
                "{:6.2} {:9.2} {:8.2} {:9.2} {:7.2} {:7.2}",
                t,
                x,
                y,
                z,
                (vx * vx + vz * vz).sqrt(),
                vy
            );
        }
        let ay = -g - k1 * vy - k2 * vy * vy.abs();
        let vh = (vx * vx + vz * vz).sqrt();
        let ah = -h1 * vh - h2 * vh * vh;
        let (ux, uz) = if vh > 1e-9 { (vx / vh, vz / vh) } else { (0.0, 0.0) };
        vy += ay * dt;
        vx += ah * ux * dt;
        vz += ah * uz * dt;
        x += vx * dt;
        y += vy * dt;
        z += vz * dt;
        t += dt;
    }
    0
}

// ------------------------------------------------------------------ cols
//
// `tmmaps region` answers "what is in this box" as a flat list, which is the
// right answer to that question and the wrong shape for "can the car get up
// there". Height is what identifies a surface on this map -- `--map` is inert
// for the container and relocated gates do not fire, so geometry is the only
// ruler left. This turns the list into one row per 32 m column, so a stack of
// blocks reads as a stack.

fn cmd_cols(a: &[String]) -> i32 {
    let p = positional(a);
    let Some(path) = p.first() else {
        eprintln!("uwlab cols: need a `tmmaps region` TSV");
        return 2;
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("uwlab cols: {path}: {e}");
            return 2;
        }
    };
    let filt = flag_val(a, "--filter");
    let xr = range(a, "--x");
    let zr = range(a, "--z");
    let ymin: f64 = flag_val(a, "--ymin").and_then(|s| s.parse().ok()).unwrap_or(f64::MIN);

    // (cell x, cell z) -> list of (y, name)
    let mut cols: std::collections::BTreeMap<(i64, i64), Vec<(i64, String)>> =
        std::collections::BTreeMap::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 6 {
            continue;
        }
        let name = f[1].to_string();
        let (Ok(x), Ok(y), Ok(z)) = (
            f[3].parse::<f64>(),
            f[4].parse::<f64>(),
            f[5].parse::<f64>(),
        ) else {
            continue;
        };
        if let Some(pat) = &filt {
            if !name.contains(pat.as_str()) {
                continue;
            }
        }
        if let Some((lo, hi)) = xr {
            if x < lo || x > hi {
                continue;
            }
        }
        if let Some((lo, hi)) = zr {
            if z < lo || z > hi {
                continue;
            }
        }
        if y < ymin {
            continue;
        }
        let cx = ((x - 16.0) / 32.0).round() as i64;
        let cz = ((z - 16.0) / 32.0).round() as i64;
        cols.entry((cx, cz)).or_default().push((y.round() as i64, name));
    }

    println!("cell        x       z     | stack (y: name xN), lowest first  -- a block occupies [y, y+8)");
    for ((cx, cz), mut v) in cols {
        v.sort();
        let mut parts: Vec<String> = Vec::new();
        let mut i = 0;
        while i < v.len() {
            let mut j = i;
            while j < v.len() && v[j] == v[i] {
                j += 1;
            }
            let n = j - i;
            if n > 1 {
                parts.push(format!("{}:{}x{}", v[i].0, v[i].1, n));
            } else {
                parts.push(format!("{}:{}", v[i].0, v[i].1));
            }
            i = j;
        }
        let top = v.iter().map(|(y, _)| *y).max().unwrap_or(0);
        println!(
            "({:2},{:2}) {:7.0} {:7.0} | top surface {:>4}  {}",
            cx,
            cz,
            32 * cx + 16,
            32 * cz + 16,
            top + 8,
            parts.join("  ")
        );
    }
    0
}

// ------------------------------------------------------------------ tape
//
// A `.gtape` writer. `ghost tape extract | inject` is the round-trip that owns
// the ghost's input chunk; what it does not have is a way to say "hold this
// steer for two seconds, then lift". Every probe in this arm is a piecewise
// constant control, so that is what this emits -- as the same text `inject`
// reads, so there is still exactly one codec.

fn cmd_tape(a: &[String]) -> i32 {
    // `--from` copies a real tape's header and per-tick coding and overwrites
    // only steer/accel/brake. Emitting a header from scratch is what a
    // from-scratch writer wants to do and it is WRONG: a tape whose decoded
    // ticks are byte-identical to a working one, but whose `bits_used` header
    // says 0, re-simulates to DNF. The control that caught it is the round
    // trip -- extract, inject, require the oracle to reproduce the time.
    let Some(out) = flag_val(a, "--out") else {
        eprintln!("uwlab tape: --out FILE.gtape");
        return 2;
    };
    let ticks: usize = flag_val(a, "--ticks").and_then(|s| s.parse().ok()).unwrap_or(4000);
    let off: i64 = flag_val(a, "--start-offset").and_then(|s| s.parse().ok()).unwrap_or(-1560);
    // --seg t0:t1:steer:accel:brake, repeated; ticks not covered are coasting.
    let mut steer = vec![0i32; ticks];
    let mut accel = vec![0i32; ticks];
    let mut brake = vec![0i32; ticks];
    let mut i = 0;
    let mut nseg = 0;
    while i < a.len() {
        if a[i] == "--seg" {
            let Some(spec) = a.get(i + 1) else { break };
            let p: Vec<&str> = spec.split(':').collect();
            if p.len() != 5 {
                eprintln!("uwlab tape: --seg wants t0:t1:steer:accel:brake, got `{spec}`");
                return 2;
            }
            let t0: usize = p[0].parse().unwrap_or(0);
            let t1: usize = p[1].parse().unwrap_or(0);
            let s: i32 = p[2].parse().unwrap_or(0);
            let g: i32 = p[3].parse().unwrap_or(0);
            let b: i32 = p[4].parse().unwrap_or(0);
            for t in t0..t1.min(ticks) {
                steer[t] = s.clamp(-127, 127);
                accel[t] = g.clamp(0, 1);
                brake[t] = b.clamp(0, 1);
            }
            nseg += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    let mut s = String::new();
    s.push_str("#gtape 1\n#source uwlab tape\n#chunk_version 4\n");
    s.push_str(&format!(
        "@archive 0 format_version=12 field0=24940 start_offset_ms={off} packets={ticks} bitstream_bytes=0 bits_used=0\n"
    ));
    // Tick 0 is the container's own mode-14 opener; every later tick is an
    // explicit mode-2 vehicle packet, so no tick inherits the previous one's
    // inputs (`ghost tape inject`'s default, spelled out here).
    s.push_str("t=0 mode=14 w=lit:0x00000000E respawn=0 mouse=34443,5522 vsame=0 tri=0,0,0,0 flags=0x000000\n");
    for t in 1..ticks {
        // The state word is 0x2 on every vehicle tick; spelling it as a 32-bit
        // literal instead of `prev` inflates the bitstream from 6.5 KB to 24 KB
        // and leaves a 17-byte tail -- and the oracle then DNFs a tape whose
        // DECODED ticks are identical, which cost this arm twenty minutes and
        // is only visible because the round-trip control was run first.
        let w = if t == 1 { "lit:0x000000002" } else { "prev" };
        s.push_str(&format!(
            "t={t} mode=2 w={w} respawn=0 mouse=none vsame=0 steer={} accel={} brake={} flags=0x000000\n",
            steer[t], accel[t], brake[t]
        ));
    }
    // With --from, keep the template's own header and per-tick coding and
    // overwrite only steer/accel/brake: a header written from scratch says
    // bits_used=0 and the oracle DNFs a tape whose decoded ticks are identical
    // to a working one.
    let s = if let Some(tpl) = flag_val(a, "--from") {
        let Ok(text) = std::fs::read_to_string(&tpl) else {
            eprintln!("uwlab tape: cannot read --from {tpl}");
            return 2;
        };
        let mut out_s = String::new();
        for line in text.lines() {
            if !line.starts_with("t=") {
                out_s.push_str(line);
                out_s.push('\n');
                continue;
            }
            let tick: usize = line[2..].split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
            if tick >= ticks || !line.contains(" steer=") {
                out_s.push_str(line);
                out_s.push('\n');
                continue;
            }
            let mut fields: Vec<String> = Vec::new();
            for f in line.split_whitespace() {
                if let Some(r) = f.strip_prefix("steer=") { let _ = r; fields.push(format!("steer={}", steer[tick])); }
                else if f.starts_with("accel=") { fields.push(format!("accel={}", accel[tick])); }
                else if f.starts_with("brake=") { fields.push(format!("brake={}", brake[tick])); }
                else { fields.push(f.to_string()); }
            }
            out_s.push_str(&fields.join(" "));
            out_s.push('\n');
        }
        out_s
    } else { s };
    if let Err(e) = std::fs::write(&out, s) {
        eprintln!("uwlab tape: {out}: {e}");
        return 2;
    }
    eprintln!("wrote {out}: {ticks} ticks, {nseg} segments, start_offset {off} ms");
    0
}

// ------------------------------------------------------------------ launch
//
// THE REACHABILITY QUESTION, ASKED OF THE WHOLE MAP AT ONCE.
//
// Underwater the car cannot translate in free water at all -- it only sinks
// (measured: a car dropped in mid-water holds |vh| < 0.02 m/s for its whole
// descent). So every metre of horizontal travel is either ON a surface or
// ballistic from one, and a ballistic arc's reach is bounded by the water's
// own linear drag: `reach(t) = (v0/k)(1 - exp(-k t))`, asymptote `v0/k`. With
// k = 0.489 /s and the flat-water speed cap of 28.6 m/s that asymptote is
// 58.5 m, whatever the launch height.
//
// So: enumerate every surface the census knows about, and for each ask whether
// a car leaving it at `--v` could still be inside a target box when it gets
// there. The answer is a DEFICIT in metres, which is a much more useful null
// than "we searched and found nothing".
fn cmd_launch(a: &[String]) -> i32 {
    let p = positional(a);
    let Some(path) = p.first() else {
        eprintln!("uwlab launch: need a `tmmaps census` or `region` TSV");
        return 2;
    };
    let Some(bs) = flag_val(a, "--box") else {
        eprintln!("uwlab launch: --box x0,y0,z0:x1,y1,z1 (the target)");
        return 2;
    };
    let Some(b) = Bx::parse(&bs) else {
        eprintln!("uwlab launch: cannot parse `{bs}`");
        return 2;
    };
    let v0: f64 = flag_val(a, "--v").and_then(|s| s.parse().ok()).unwrap_or(28.6);
    let kh: f64 = flag_val(a, "--kh").and_then(|s| s.parse().ok()).unwrap_or(0.489);
    let vt: f64 = flag_val(a, "--sink").and_then(|s| s.parse().ok()).unwrap_or(2.65);
    let kv: f64 = flag_val(a, "--kv").and_then(|s| s.parse().ok()).unwrap_or(0.77);
    let filt = flag_val(a, "--filter");
    let top = flag_val(a, "--top").is_some();
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("uwlab launch: {path}: {e}");
            return 2;
        }
    };
    // A block's own top surface: the census reports the cell base, and a
    // platform's drivable face sits 0.16 m above it (measured on this map by 35
    // plumb probes: every column rests at 9.16 / 114.16 / 170.16).
    struct Cand {
        name: String,
        x: f64,
        y: f64,
        z: f64,
        dist: f64,
        budget: f64,
        reach: f64,
    }
    let mut out: Vec<Cand> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 6 {
            continue;
        }
        // census: src id name cx cy cz flags placement x y z wp
        // region:     id name placement x y z wp
        let (name, x, y, z) = if f.len() >= 11 {
            (f[2], f[8], f[9], f[10])
        } else {
            (f[1], f[3], f[4], f[5])
        };
        let (Ok(x), Ok(y), Ok(z)) = (x.parse::<f64>(), y.parse::<f64>(), z.parse::<f64>()) else {
            continue;
        };
        if let Some(pat) = &filt {
            if !name.contains(pat.as_str()) {
                continue;
            }
        }
        let sy = y + 0.16;
        if sy <= b.y0 {
            continue; // below the box: a sinking car can never come back up
        }
        // Nearest point of the box in the horizontal plane.
        let dx = (b.x0 - x).max(x - b.x1).max(0.0);
        let dz = (b.z0 - z).max(z - b.z1).max(0.0);
        let dist = (dx * dx + dz * dz).sqrt();
        // How long the car may stay above the box floor.
        // y(t) = y0 - [vt*t - (vt/kv)(1-exp(-kv t))]; solve for y = b.y0.
        let drop = sy - b.y0;
        let mut t = 0.0;
        let mut lo = 0.0;
        let mut hi = 600.0;
        for _ in 0..60 {
            t = 0.5 * (lo + hi);
            let d = vt * t - (vt / kv) * (1.0 - (-kv * t).exp());
            if d < drop {
                lo = t;
            } else {
                hi = t;
            }
        }
        let reach = (v0 / kh) * (1.0 - (-kh * t).exp());
        out.push(Cand { name: name.to_string(), x, y: sy, z, dist, budget: t, reach });
    }
    out.sort_by(|p, q| (p.dist - p.reach).partial_cmp(&(q.dist - q.reach)).unwrap());
    println!(
        "target box x {:.0}..{:.0}  y {:.0}..{:.0}  z {:.0}..{:.0}",
        b.x0, b.x1, b.y0, b.y1, b.z0, b.z1
    );
    println!("launch speed {v0:.2} m/s, drag k {kh}, sink {vt} m/s (kv {kv})  ->  asymptotic reach {:.1} m", v0 / kh);
    println!("\nsurface                                   x        y        z    dist   air_s   reach  DEFICIT");
    let n = if top { 40 } else { out.len().min(40) };
    for c in out.iter().take(n) {
        println!(
            "{:38} {:8.0} {:8.2} {:8.0} {:7.1} {:7.1} {:7.1}  {:+8.1}",
            c.name, c.x, c.y, c.z, c.dist, c.budget, c.reach, c.dist - c.reach
        );
    }
    let best = out.first().map(|c| c.dist - c.reach).unwrap_or(f64::NAN);
    println!("\n{} candidate surfaces above the box floor; best deficit {:+.1} m", out.len(), best);
    if best <= 0.0 { 0 } else { 1 }
}

// ------------------------------------------------------------------ plumb
//
// A PLUMB PROBE, read off a trajectory. Drop the car in one 32 m column and
// report where it stops falling: on this map height is the only ruler that
// works (`--map` is inert for the replay container and relocated gates do not
// fire), so "what is the top surface of this column" has to be answered by
// dropping a car down it. Reports the FIRST contact -- the car drives away
// afterwards and its final resting place is a different question.
fn cmd_plumb(a: &[String]) -> i32 {
    let t = load_one(a);
    let after: f64 = flag_val(a, "--after").and_then(|s| s.parse().ok()).unwrap_or(0.5);
    let mut sinking = false;
    for w in t.rows.windows(2) {
        let (r, n) = (&w[0], &w[1]);
        if r.t < after {
            continue;
        }
        if r.vy < -2.0 {
            sinking = true;
        }
        // Contact: the sink stops. A car that is merely slowing has |vy|
        // decreasing over several samples; a contact is one sample.
        if sinking && n.vy > -1.0 && r.vy < -2.0 {
            println!(
                "CONTACT y {:.3} at ({:.2}, {:.2}) t {:.3}",
                n.y, n.x, n.z, n.t
            );
            return 0;
        }
    }
    let last = t.rows.last();
    match last {
        Some(r) => println!("NO CONTACT; last y {:.3} at ({:.2}, {:.2}) t {:.3}", r.y, r.x, r.z, r.t),
        None => println!("NO ROWS"),
    }
    1
}

// ------------------------------------------------------------------ probemap
//
// The plumb-probe LATTICE, as a command rather than a shell loop. It builds one
// spawn-moved map per 32 m column (`tmmaps move` on the map's start block),
// drops a car down each, and prints the height of the first surface it meets.
//
// Why a whole command: on this map height is the only ruler. `--map` is inert
// for the replay container, relocated gates do not fire, and the block census
// gives a cell and a name but NOT whether the thing under that name is solid,
// how much of the cell it fills, or where its drivable face is. Thirty-five
// hand-run probes were the previous state of the art here and they missed the
// two surfaces this arm needed.
fn cmd_probemap(a: &[String]) -> i32 {
    let need = |n: &str| -> String {
        match flag_val(a, n) {
            Some(v) => v,
            None => {
                eprintln!("uwlab probemap: {n} is required");
                std::process::exit(2);
            }
        }
    };
    let base = need("--map");
    let tape = need("--tape");
    let tmmaps = flag_val(a, "--tmmaps").unwrap_or_else(|| "tmmaps".into());
    let fk = flag_val(a, "--fk").unwrap_or_else(|| "fk".into());
    let block: String = flag_val(a, "--block").unwrap_or_else(|| "4633".into());
    let cy: i64 = flag_val(a, "--cy").and_then(|s| s.parse().ok()).unwrap_or(32);
    let jobs: usize = flag_val(a, "--jobs").and_then(|s| s.parse().ok()).unwrap_or(24);
    let dir = flag_val(a, "--dir").unwrap_or_else(|| "probemap".into());
    let (x0, x1) = range(a, "--cx").map(|(a, b)| (a as i64, b as i64)).unwrap_or((38, 48));
    let (z0, z1) = range(a, "--cz").map(|(a, b)| (a as i64, b as i64)).unwrap_or((11, 19));
    let _ = std::fs::create_dir_all(&dir);

    let cells: Vec<(i64, i64)> = (x0..=x1).flat_map(|x| (z0..=z1).map(move |z| (x, z))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::<String>::new());
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(&(cx, cz)) = cells.get(i) else { return };
                let tag = format!("{cx}_{cz}");
                let map = format!("{dir}/p_{tag}.Map.Gbx");
                let csv = format!("{dir}/t_{tag}.csv");
                let work = format!("/tmp/uwlab-probe-{tag}");
                let _ = std::fs::remove_dir_all(&work);
                let mv = std::process::Command::new(&tmmaps)
                    .args(["move", &base, "--out", &map, "--move", &format!("{block}:{cx},{cy},{cz}")])
                    .output();
                if mv.map(|o| !o.status.success()).unwrap_or(true) {
                    out.lock().unwrap().push(format!("({cx:2},{cz:2}) MAP BUILD FAILED"));
                    continue;
                }
                let tr = std::process::Command::new(&fk)
                    .args(["trace", "--tape", &tape, "--map", &map, "--work", &work, "--at", "tick:1200", "--out", &csv])
                    .env("FK_VERR_MAX", "3.0")
                    .output();
                if tr.map(|o| !o.status.success()).unwrap_or(true) {
                    out.lock().unwrap().push(format!("({cx:2},{cz:2}) x {:5} z {:5}  TRACE FAILED", 32 * cx, 32 * cz));
                    continue;
                }
                let line = match Traj::load(&csv) {
                    Ok(t) => contact_line(&t),
                    Err(e) => format!("read failed: {e}"),
                };
                out.lock().unwrap().push(format!("({cx:2},{cz:2}) x {:5} z {:5}  {line}", 32 * cx, 32 * cz));
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort();
    for l in v {
        println!("{l}");
    }
    0
}

/// First contact, shared by `plumb` and `probemap`.
///
/// "Contact" is the first sample after which the car stops sinking for a
/// whole second. A one-sample test reads every suspension wobble as a
/// landing and every slow-down as none: the first version of this returned
/// NO CONTACT on 68 of 68 columns, including the ones that provably land.
fn contact_line(t: &Traj) -> String {
    let n = t.rows.len();
    let mut i = 0;
    while i < n {
        let r = &t.rows[i];
        if r.t < 1.0 || r.vy < -2.0 {
            i += 1;
            continue;
        }
        // candidate: does it stay un-sunk for 1 s?
        let t0 = r.t;
        let mut j = i;
        let mut ok = true;
        while j < n && t.rows[j].t - t0 < 1.0 {
            if t.rows[j].vy < -2.0 {
                ok = false;
                break;
            }
            j += 1;
        }
        if ok && j < n {
            let e = t.rows.last().unwrap();
            return format!(
                "CONTACT y {:8.3} at ({:8.2},{:8.2}) t {:6.3}   end y {:8.3} at ({:8.2},{:8.2}) t {:6.3}",
                r.y, r.x, r.z, r.t, e.y, e.x, e.z, e.t
            );
        }
        i = j.max(i + 1);
    }
    match t.rows.last() {
        Some(r) => format!("never stops sinking; last y {:8.3} at ({:8.2},{:8.2}) t {:6.3}", r.y, r.x, r.z, r.t),
        None => "no rows".into(),
    }
}

// ------------------------------------------------------------------ maxy
//
// The height a run ever reached, and where. On 173691 that is the whole
// question from the lower canopy: the finish plane's live band starts about
// 16 m above the deck and nothing in the census connects the two, so "did the
// car ever leave 114.16" is the measurement that would reopen the map.
fn cmd_maxy(a: &[String]) -> i32 {
    let t = load_one(a);
    let after: f64 = flag_val(a, "--after").and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let mut best: Option<Row> = None;
    for r in &t.rows {
        if r.t < after {
            continue;
        }
        if best.as_ref().map(|b| r.y > b.y).unwrap_or(true) {
            best = Some(r.clone());
        }
    }
    match best {
        Some(r) => {
            println!("MAXY {:8.3} at ({:8.2},{:8.2}) t {:6.3}  |v| {:6.2}", r.y, r.x, r.z, r.t, r.speed_ms);
            0
        }
        None => {
            println!("no rows");
            1
        }
    }
}
