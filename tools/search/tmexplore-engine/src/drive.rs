//! `tmexplore-real drive` — a closed-loop driver for a real-world circuit map.
//!
//! The circuit maps (`tools/circuit`) come with a centreline TSV: one row per
//! metre of lap with position, heading, curvature and the tarmac edges, plus a
//! header naming the start, finish and checkpoint stations. On a wide, flat,
//! gently curved road that line IS drivable, which is the one case where a
//! feedback controller beats a tree search: a pure-pursuit steering law toward
//! a lookahead point and a curvature-limited speed plan produce a smooth lap
//! that looks like a car being driven, not a macro alphabet being searched.
//!
//! The engine in the loop is the fork server ([`ForkBranch`]): every decision
//! reads the car's real state (position, velocity, attitude) from the paused
//! simulation and every macro re-simulates the prefix, so nothing here is a
//! model of the car — the car is the car.
//!
//! Recovery is rewinding: when the car leaves the tarmac or stalls, the tape is
//! cut back a few seconds, the speed allowance for that stretch of road is
//! reduced, and driving resumes. The plain oracle is asked to confirm every
//! checkpoint crossing and the finish; only its verdict is reported as one.

use super::Args;
use tmexplore_engine::fork::{ForkBranch, ForkOpts};
use tmexplore_engine::EngineOracle;
use std::path::PathBuf;
use tmexplore::action::Input;
use tmexplore::branch::{Branch, CarState};
use tmexplore::outcome::Verdict;

/// One metre of the lap.
#[derive(Clone, Debug)]
pub struct Row {
    pub s: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// atan2(dx, dz) of travel: 0 = +z, increasing toward +x.
    pub heading: f64,
    /// > 0 turns left.
    pub curvature: f64,
    pub left_m: f64,
    pub right_m: f64,
    pub corner: String,
}

/// The centreline and the map's gates, as stations along it.
pub struct Line {
    pub rows: Vec<Row>,
    pub start: usize,
    pub finish: usize,
    pub checkpoints: Vec<usize>,
}

impl Line {
    pub fn load(p: &std::path::Path) -> Result<Line, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
        let mut rows = Vec::new();
        let (mut start, mut finish, mut cps) = (None, None, Vec::new());
        for line in text.lines() {
            if let Some(h) = line.strip_prefix('#') {
                let f: Vec<&str> = h.split('\t').map(|s| s.trim()).collect();
                let mut i = 0;
                while i + 1 < f.len() {
                    match f[i] {
                        "start_station" => start = f[i + 1].parse().ok(),
                        "finish_station" => finish = f[i + 1].parse().ok(),
                        "checkpoints" => {
                            cps = f[i + 1]
                                .split(',')
                                .filter_map(|s| s.trim().parse().ok())
                                .collect()
                        }
                        _ => {}
                    }
                    i += 2;
                }
                continue;
            }
            if line.starts_with("station") || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 9 {
                return Err(format!("{}: short row {:?}", p.display(), line));
            }
            let num = |i: usize| -> Result<f64, String> {
                f[i].trim()
                    .parse::<f64>()
                    .map_err(|_| format!("{}: field {} of {:?} is not a number", p.display(), i, line))
            };
            rows.push(Row {
                s: num(1)?,
                x: num(2)?,
                y: num(3)?,
                z: num(4)?,
                heading: num(5)?,
                curvature: num(6)?,
                left_m: num(7)?,
                right_m: num(8)?,
                corner: f.get(9).map(|s| s.trim().to_string()).unwrap_or_default(),
            });
        }
        let start = start.ok_or("line header: no start_station")?;
        let finish = finish.ok_or("line header: no finish_station")?;
        if cps.is_empty() {
            return Err("line header: no checkpoints".into());
        }
        if rows.len() < 100 {
            return Err(format!("line: only {} rows", rows.len()));
        }
        Ok(Line { rows, start, finish, checkpoints: cps })
    }

    pub fn n(&self) -> usize {
        self.rows.len()
    }

    /// Station index for a distance driven from the start (wraps).
    pub fn station_at(&self, d: f64) -> usize {
        let n = self.n() as i64;
        (((self.start as f64 + d).floor() as i64).rem_euclid(n)) as usize
    }

    /// Distance from the start (in metres of lap) of a station.
    pub fn dist_of(&self, station: usize) -> f64 {
        let n = self.n() as i64;
        ((station as i64 - self.start as i64).rem_euclid(n)) as f64
    }

    /// Unit forward and right vectors in the XZ plane at a station.
    pub fn frame(&self, station: usize) -> ([f64; 2], [f64; 2]) {
        let h = self.rows[station % self.n()].heading;
        let fwd = [h.sin(), h.cos()];
        // Facing +z (heading 0) the right hand points to -x: x east, z south.
        let right = [-h.cos(), h.sin()];
        (fwd, right)
    }

    /// Nearest station to `pos` within a window around a hint (the lap closes
    /// on itself, so a global search would jump to the other side of the
    /// start/finish straight). Returns (station, along, lateral).
    pub fn locate(&self, pos: [f64; 3], hint: usize, back: usize, ahead: usize) -> (usize, f64) {
        let n = self.n();
        let mut best = (hint, f64::INFINITY);
        for k in 0..(back + ahead) {
            let st = ((hint + n - back + k) % n) as usize;
            let r = &self.rows[st];
            let dx = pos[0] - r.x;
            let dz = pos[2] - r.z;
            let d2 = dx * dx + dz * dz;
            if d2 < best.1 {
                best = (st, d2);
            }
        }
        let st = best.0;
        let r = &self.rows[st];
        let (_, right) = self.frame(st);
        let lateral = (pos[0] - r.x) * right[0] + (pos[2] - r.z) * right[1];
        (st, lateral)
    }
}

/// Heading of the car's forward axis (+z in the car frame), in the line's
/// convention: atan2(fx, fz).
pub fn yaw_of(q: [f32; 4]) -> f64 {
    let [w, x, y, z] = [q[0] as f64, q[1] as f64, q[2] as f64, q[3] as f64];
    let fx = 2.0 * (x * z + w * y);
    let fz = 1.0 - 2.0 * (x * x + y * y);
    fx.atan2(fz)
}

pub fn wrap(a: f64) -> f64 {
    let mut a = a;
    while a > std::f64::consts::PI {
        a -= 2.0 * std::f64::consts::PI;
    }
    while a < -std::f64::consts::PI {
        a += 2.0 * std::f64::consts::PI;
    }
    a
}

fn die<T, E: std::fmt::Display>(e: E) -> T {
    eprintln!("{}", e);
    std::process::exit(1)
}

fn secs(ms: i64) -> String {
    format!("{}.{:03}", ms / 1000, (ms % 1000).abs())
}

pub struct Rig {
    pub line: Line,
    pub branch: ForkBranch,
    pub oracle: EngineOracle,
    pub work: PathBuf,
}

fn rig(a: &Args) -> Rig {
    // `--want-boundary N`: the fork boundary varies by a tick between server
    // launches; a resume needs the server to stop no later than the tape frame.
    let want: Option<usize> = a.get("want-boundary").and_then(|v| v.parse().ok());
    for attempt in 1..=8 {
        let r = rig_once(a);
        match want {
            Some(w) if r.branch.from > w => {
                println!("boundary {} is past the wanted {}; relaunching the fork server (attempt {})", r.branch.from, w, attempt);
                drop(r);
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            _ => return r,
        }
    }
    die("could not get a fork boundary at or before the wanted tick in 8 launches")
}

fn rig_once(a: &Args) -> Rig {
    let line = Line::load(&PathBuf::from(a.req("line"))).unwrap_or_else(die);
    let map = PathBuf::from(a.req("map"));
    let template = PathBuf::from(a.req("template"));
    let server = PathBuf::from(a.get("server").unwrap_or("/tmp/tmoracle/server"));
    let shim = PathBuf::from(a.req("shim"));
    let work = PathBuf::from(a.get("work").unwrap_or("/tmp/tm-drive"));
    let tape = ghost::Tape::from_file(&template.to_string_lossy()).unwrap_or_else(die);
    let start_offset_ms = tape.archives.first().map(|x| x.start_offset_ms).unwrap_or(0);
    let oracle = EngineOracle::new(&template, &map, &server, &work.join("oracle")).unwrap_or_else(die);
    let reference = oracle.template_inputs();
    let forktick: i64 = a.num("forktick", 60i64);
    let checkpoint_clock: u64 = a.num("checkpoint-clock", tmsearch::forkeval::clock_for_tick(forktick, 0));
    let route_points: Vec<[f32; 3]> = line.rows.iter().map(|r| [r.x as f32, r.y as f32, r.z as f32]).collect();
    let opts = ForkOpts {
        work: work.join("fork"),
        server,
        map,
        reference_ghost: template,
        shim,
        checkpoint_clock,
        start_offset_ms,
        route_points,
        tail_margin: 200,
        common_from: None,
    };
    let branch = ForkBranch::start(&opts, reference).unwrap_or_else(die);
    println!(
        "fork server up: boundary tick {} (search tick 0 = file tick {}), container {} ticks, start_offset {} ms",
        branch.from,
        branch.from,
        branch.capacity,
        start_offset_ms
    );
    oracle.set_prefix_ticks(branch.from as u64);
    println!(
        "line: {} m, start station {}, finish station {} (d = {:.0} m), {} checkpoints at d = {}",
        line.n(),
        line.start,
        line.finish,
        line.dist_of(line.finish),
        line.checkpoints.len(),
        line.checkpoints.iter().map(|&c| format!("{:.0}", line.dist_of(c))).collect::<Vec<_>>().join(",")
    );
    Rig { line, branch, oracle, work }
}

/// One line of the per-tick log.
fn describe(line: &Line, s: &CarState, hint: usize) -> (usize, f64, String) {
    let pos = [s.pos[0] as f64, s.pos[1] as f64, s.pos[2] as f64];
    let (st, lat) = line.locate(pos, hint, 40, 120);
    let speed = s.speed() as f64;
    let yaw = yaw_of(s.quat);
    let vyaw = if speed > 0.5 { (s.vel[0] as f64).atan2(s.vel[2] as f64) } else { yaw };
    let r = &line.rows[st];
    let txt = format!(
        "{}\t{:.3}\t{:.3}\t{:.3}\t{:.2}\t{:.1}\t{:.4}\t{:.4}\t{:.4}\t{}\t{:.0}\t{:.2}\t{:.4}\t{}",
        s.tick,
        s.pos[0],
        s.pos[1],
        s.pos[2],
        speed,
        speed * 3.6,
        yaw,
        vyaw,
        wrap(yaw - r.heading),
        st,
        line.dist_of(st),
        lat,
        r.curvature,
        r.corner
    );
    (st, lat, txt)
}

const LOG_HEADER: &str = "tick\tx\ty\tz\tspeed_ms\tkmh\tyaw\tvel_yaw\tyaw_err\tstation\tdist\tlateral\tcurv\tcorner";

/// Replay a tape (or a constant input) through the fork engine and log the car.
pub fn trace(a: &Args) {
    let mut r = rig(a);
    let n: usize = a.num("ticks", 1500usize);
    let steer: i8 = a.num("steer", 0i8);
    let gas = !a.flag("no-gas");
    let brake = a.flag("brake");
    let tape: Vec<Input> = match a.get("tape") {
        Some(p) => read_tape(&PathBuf::from(p)).unwrap_or_else(die),
        None => vec![Input { steer, gas, brake }; n],
    };
    let every: usize = a.num("every", 10usize);
    let out = PathBuf::from(a.get("out").unwrap_or("/tmp/tm-drive/trace.tsv"));
    let init = r.branch.initial_state().unwrap_or_else(|e| die(format!("{e:?}")));
    let (st0, lat0, txt0) = describe(&r.line, &init, r.line.start);
    println!("initial (search tick 0): {}", txt0);
    println!("  spawn expected near station {} (d=0); located station {} lateral {:.2}", r.line.start, st0, lat0);
    let h = r.branch.open(&[], None).unwrap_or_else(|e| die(format!("{e:?}")));
    let t0 = std::time::Instant::now();
    let adv = r.branch.advance(h, 0, &tape).unwrap_or_else(|e| die(format!("{e:?}")));
    println!("advance of {} ticks: {:.2} s, {} states, ended {:?}", tape.len(), t0.elapsed().as_secs_f64(), adv.trace.len(), adv.ended);
    let mut txt = String::from(LOG_HEADER);
    txt.push('\n');
    let mut hint = st0;
    let mut max_speed = 0.0f64;
    let mut max_lat = 0.0f64;
    for (i, s) in adv.trace.iter().enumerate() {
        let (st, lat, t) = describe(&r.line, s, hint);
        hint = st;
        max_speed = max_speed.max(s.speed() as f64);
        max_lat = max_lat.max(lat.abs());
        if i % every == 0 || i + 1 == adv.trace.len() {
            txt.push_str(&t);
            txt.push('\n');
        }
    }
    std::fs::create_dir_all(out.parent().unwrap()).ok();
    std::fs::write(&out, &txt).unwrap_or_else(die);
    let last = adv.trace.last().copied().unwrap_or(init);
    let (st, lat, t) = describe(&r.line, &last, hint);
    println!("final: {}", t);
    println!(
        "reached d = {:.0} m (station {}), lateral {:.2} m, max speed {:.1} m/s ({:.0} km/h), max |lateral| {:.2} m; log {}",
        r.line.dist_of(st),
        st,
        lat,
        max_speed,
        max_speed * 3.6,
        max_lat,
        out.display()
    );
    if a.flag("confirm") {
        let t1 = std::time::Instant::now();
        match r.oracle.confirm_echo(&tape) {
            Ok((v, echo, uid, desc)) => println!(
                "plain oracle: {:?} desc {:?} uid {} echo {} ({:.1} s)",
                v,
                desc.trim(),
                uid,
                echo,
                t1.elapsed().as_secs_f64()
            ),
            Err(e) => println!("plain oracle REFUSED: {}", e),
        }
    }
}

pub fn read_tape(p: &std::path::Path) -> Result<Vec<Input>, String> {
    let txt = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
    let mut rows: Vec<(usize, Input)> = Vec::new();
    for line in txt.lines() {
        if line.starts_with('#') || line.starts_with("tick") || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        rows.push((
            f[0].parse().map_err(|_| format!("bad tick {:?}", f[0]))?,
            Input {
                steer: f[1].parse().map_err(|_| format!("bad steer {:?}", f[1]))?,
                gas: f[2] != "0",
                brake: f[3] != "0",
            },
        ));
    }
    rows.sort_by_key(|r| r.0);
    let n = rows.last().map(|r| r.0 + 1).unwrap_or(0);
    let mut tape = vec![Input { steer: 0, gas: false, brake: false }; n];
    let mut last = Input { steer: 0, gas: false, brake: false };
    let mut j = 0;
    for t in 0..n {
        while j < rows.len() && rows[j].0 <= t {
            last = rows[j].1;
            j += 1;
        }
        tape[t] = last;
    }
    Ok(tape)
}

pub fn write_tape(p: &std::path::Path, tape: &[Input], frame: usize) -> Result<(), String> {
    let mut txt = format!("# frame\t{}\ntick\tsteer\tgas\tbrake\n", frame);
    for (i, t) in tape.iter().enumerate() {
        txt.push_str(&format!("{}\t{}\t{}\t{}\n", i, t.steer, t.gas as u8, t.brake as u8));
    }
    std::fs::write(p, txt).map_err(|e| format!("{}: {}", p.display(), e))
}

/// The tunables of the driving law. Every one is a flag.
#[derive(Clone, Debug)]
pub struct Law {
    /// Lookahead distance = clamp(look_t * speed, look_min, look_max), metres.
    pub look_t: f64,
    pub look_min: f64,
    pub look_max: f64,
    /// Steer = -k_alpha * alpha (radians to the lookahead point) scaled by 1/speed factor.
    pub k_alpha: f64,
    /// Lateral-offset correction, per metre of offset, added to alpha (radians).
    pub k_lat: f64,
    /// Allowed lateral acceleration in a corner, m/s^2 -> v = sqrt(a_lat / |kappa|).
    pub a_lat: f64,
    /// Planning deceleration, m/s^2.
    pub a_brake: f64,
    /// Speed cap, m/s.
    pub v_max: f64,
    /// Planning horizon, metres.
    pub horizon: f64,
    /// Ticks per decision.
    pub k: usize,
    /// Speed margin: brake above target + hi, gas below target - lo.
    pub band_hi: f64,
    pub band_lo: f64,
    /// Full-lock path curvature model kappa_max(v) = k0 * v^-v_roll (see `kappa_max`).
    pub k0: f64,
    pub v_roll: f64,
    /// Lateral offset of the pursued point from the centreline, metres, positive = right.
    pub bias: f64,
}

impl Law {
    fn from_args(a: &Args) -> Law {
        Law {
            look_t: a.num("look-t", 1.0),
            look_min: a.num("look-min", 10.0),
            look_max: a.num("look-max", 70.0),
            k_alpha: a.num("k-alpha", 1.0),
            k_lat: a.num("k-lat", 0.0),
            a_lat: a.num("a-lat", 20.0),
            a_brake: a.num("a-brake", 20.0),
            v_max: a.num("v-max", 55.0),
            horizon: a.num("horizon", 400.0),
            k: a.num("k", 10usize),
            band_hi: a.num("band-hi", 2.0),
            band_lo: a.num("band-lo", 0.0),
            k0: a.num("k0", 12.5),
            v_roll: a.num("v-roll", 1.6),
            bias: a.num("bias", 0.0),
        }
    }

    /// Path curvature at full lock, 1/m. Measured on the Silverstone map
    /// (full gas, steer 127, the velocity-direction rate): 0.042 at 28 m/s,
    /// 0.024 at 50, 0.015 at 67, and linear in the steer input (steer 40 gave
    /// 0.31-0.32 of full lock at both speeds). `k0 * v^-1.6` fits those to
    /// within 10 %; below 10 m/s the steering saturates and the value is
    /// clamped there.
    pub fn kappa_max(&self, v: f64) -> f64 {
        self.k0 * v.max(10.0).powf(-self.v_roll)
    }
}

/// Per-stretch speed allowance, learned by failing: `scale[d]` multiplies the
/// planned speed for metre `d` of the lap.
pub struct Allow {
    pub scale: Vec<f64>,
}

/// Target speed at distance `d` given the line ahead and the allowance.
fn plan_speed(line: &Line, law: &Law, allow: &Allow, d: f64) -> f64 {
    let mut v = law.v_max;
    let n = line.n();
    let steps = law.horizon as usize;
    for i in 0..steps {
        let di = d + i as f64;
        let st = line.station_at(di);
        let k = line.rows[st].curvature.abs();
        let sc = allow.scale[(di.floor() as usize) % n];
        let mut v_allow = law.v_max;
        if k > 1e-5 {
            v_allow = (law.a_lat / k).sqrt().min(law.v_max);
        }
        v_allow *= sc;
        // What we may be doing now and still be at v_allow after i metres of braking.
        let here = (v_allow * v_allow + 2.0 * law.a_brake * i as f64).sqrt();
        if here < v {
            v = here;
        }
    }
    v
}

/// One decision: the input to hold for the next `k` ticks.
fn decide(line: &Line, law: &Law, allow: &Allow, s: &CarState, hint: usize) -> (Input, usize, f64, f64, f64) {
    let pos = [s.pos[0] as f64, s.pos[1] as f64, s.pos[2] as f64];
    let (st, lat) = line.locate(pos, hint, 40, 120);
    let speed = s.speed() as f64;
    let d = line.dist_of(st);
    let yaw = yaw_of(s.quat);
    // Lookahead point on the centreline.
    let look = (law.look_t * speed).clamp(law.look_min, law.look_max);
    let target_st = line.station_at(d + look);
    let t = &line.rows[target_st];
    // A constant lateral bias moves the pursued point off the centreline (positive
    // = right), clamped 1.5 m inside the tarmac edge at the target station.
    let (_, tr) = line.frame(target_st);
    let b = law.bias.clamp(-(t.left_m - 1.5).max(0.0), (t.right_m - 1.5).max(0.0));
    let (tx, tz) = (t.x + b * tr[0], t.z + b * tr[1]);
    let bearing = (tx - pos[0]).atan2(tz - pos[2]);
    // Angle to the target, in the line's convention (positive = toward +x from +z = LEFT).
    let alpha = wrap(bearing - yaw);
    // Pure pursuit: curvature to reach the target point.
    let dist = ((tx - pos[0]).powi(2) + (tz - pos[2]).powi(2)).sqrt().max(1.0);
    let kappa_des = 2.0 * alpha.sin() / dist + law.k_lat * (-lat) / (dist);
    let kmax = law.kappa_max(speed);
    // Positive kappa = left turn = negative steer value.
    let steer_f = (-(kappa_des / kmax) * law.k_alpha).clamp(-1.0, 1.0);
    let steer = (steer_f * 127.0).round() as i8;
    let v_target = plan_speed(line, law, allow, d);
    let (gas, brake) = if speed > v_target + law.band_hi {
        (false, true)
    } else if speed < v_target - law.band_lo {
        (true, false)
    } else {
        (false, false)
    };
    (Input { steer, gas, brake }, st, lat, v_target, alpha)
}

/// The closed loop.
pub fn drive(a: &Args) {
    let mut r = rig(a);
    let law = Law::from_args(a);
    println!("law: {:?}", law);
    let out_dir = r.work.clone();
    std::fs::create_dir_all(&out_dir).ok();
    let n = r.line.n();
    let mut allow = Allow { scale: vec![1.0; n] };
    let init = r.branch.initial_state().unwrap_or_else(|e| die(format!("{e:?}")));
    let (st0, lat0, txt0) = describe(&r.line, &init, r.line.start);
    println!("initial: {}", txt0);
    println!("  located station {} (d={:.0}) lateral {:.2}", st0, r.line.dist_of(st0), lat0);
    let d_finish = r.line.dist_of(r.line.finish);
    let cp_d: Vec<f64> = r.line.checkpoints.iter().map(|&c| r.line.dist_of(c)).collect();

    let max_ticks: usize = a.num("max-ticks", 24000usize).min(r.branch.writable_ticks());
    let max_rewinds: usize = a.num("max-rewinds", 60usize);
    let rewind_s: f64 = a.num("rewind-s", 3.0);
    let shrink: f64 = a.num("shrink", 0.85);
    let off_margin: f64 = a.num("off-margin", -0.5);
    let log_path = out_dir.join("drive.log.tsv");
    let mut log = String::from(LOG_HEADER);
    log.push_str("\tsteer\tgas\tbrake\tv_target\talpha\n");

    let mut tape: Vec<Input> = Vec::new();
    // Per-macro bookkeeping so a rewind can cut at a macro boundary.
    let mut hist: Vec<(usize, CarState, usize)> = Vec::new(); // (tape len before macro, state before macro, station hint)
    let mut state = init;
    let mut hint = st0;
    let mut best_d = 0.0f64;
    let mut rewinds = 0usize;
    let mut cps_confirmed = 0usize;
    let mut next_cp = 0usize;
    let t_start = std::time::Instant::now();
    let mut ended: Option<Verdict> = None;
    let mut sim_secs = 0.0f64;
    // `--start-from TAPE` resumes from a banked tape: it is replayed through the
    // engine in one advance (the physics is deterministic, so the car ends where
    // it ended when the tape was banked), the per-macro history is rebuilt from
    // the trace, and driving continues from its last tick. The tape's frame must
    // be this server's boundary, or it is a different run: refused.
    if let Some(p) = a.get("start-from") {
        let p = PathBuf::from(p);
        let txt = std::fs::read_to_string(&p).unwrap_or_else(die);
        let frame: Option<usize> = txt
            .lines()
            .find_map(|l| l.strip_prefix("# frame").and_then(|r| r.trim().parse().ok()));
        // The fork boundary moves by a tick between server launches (the
        // `lroundf` clock is load-sensitive). A server that stopped EARLIER
        // than the tape's frame can still replay it exactly: the ticks between
        // its boundary and the frame hold the container's own inputs in the
        // file the tape was confirmed in, so those are what is prepended. A
        // server that stopped LATER cannot -- the tape's first ticks would fall
        // below the boundary and be silently ignored -- so that is refused.
        let pad: Vec<Input> = match frame {
            Some(f) if f >= r.branch.from => {
                let tpl = r.oracle.template_inputs();
                (r.branch.from..f)
                    .map(|t| Input { steer: tpl.steer[t], gas: tpl.gas[t], brake: tpl.brake[t] })
                    .collect()
            }
            Some(f) => die(format!(
                "{} was driven at boundary {} but this server resumes at {}; its first {} ticks would be \
                 below the boundary and silently ignored. Relaunch (the boundary varies by a tick).",
                p.display(),
                f,
                r.branch.from,
                r.branch.from - f
            )),
            None => die(format!("{} carries no `# frame` line", p.display())),
        };
        if !pad.is_empty() {
            println!("  prepending {} ticks of the container's own inputs (boundary {} < frame {})", pad.len(), r.branch.from, frame.unwrap());
        }
        let mut start = pad;
        start.extend(read_tape(&p).unwrap_or_else(die));
        let h = r.branch.open(&[], None).unwrap_or_else(|e| die(format!("{e:?}")));
        let adv = r.branch.advance(h, 0, &start).unwrap_or_else(|e| die(format!("{e:?}")));
        if adv.trace.len() != start.len() {
            die::<(), _>(format!("replaying {} returned {} states for {} ticks", p.display(), adv.trace.len(), start.len()));
        }
        let mut hh = st0;
        for (i, s) in adv.trace.iter().enumerate() {
            if i % law.k == 0 {
                let prev = if i == 0 { init } else { adv.trace[i - 1] };
                hist.push((i, prev, hh));
            }
            let pos = [s.pos[0] as f64, s.pos[1] as f64, s.pos[2] as f64];
            hh = r.line.locate(pos, hh, 40, 120).0;
        }
        state = *adv.trace.last().unwrap();
        hint = hh;
        tape = start;
        let d0 = r.line.dist_of(hint);
        while next_cp < cp_d.len() && d0 > cp_d[next_cp] + 5.0 {
            next_cp += 1;
        }
        best_d = d0;
        println!(
            "resumed from {}: {} ticks, car at d = {:.0} m doing {:.1} m/s, {} checkpoints behind it",
            p.display(),
            tape.len(),
            d0,
            state.speed(),
            next_cp
        );
    }

    loop {
        if tape.len() + law.k > max_ticks {
            println!("tape reached {} ticks without finishing", tape.len());
            break;
        }
        let (input, st, lat, v_target, alpha) = decide(&r.line, &law, &allow, &state, hint);
        hist.push((tape.len(), state, hint));
        let macro_in = vec![input; law.k];
        let h = r.branch.open(&tape, None).unwrap_or_else(|e| die(format!("{e:?}")));
        let t0 = std::time::Instant::now();
        let adv = match r.branch.advance(h, tape.len() as u32, &macro_in) {
            Ok(x) => x,
            Err(e) => die(format!("advance failed at tick {}: {e:?}", tape.len())),
        };
        sim_secs += t0.elapsed().as_secs_f64();
        tape.extend_from_slice(&macro_in);
        // Log the decision against the state it was made on.
        let (_, _, t) = describe(&r.line, &state, hint);
        log.push_str(&format!(
            "{}\t{}\t{}\t{}\t{:.2}\t{:.4}\n",
            t, input.steer, input.gas as u8, input.brake as u8, v_target, alpha
        ));
        let _ = (st, lat);
        if let Some(last) = adv.trace.last() {
            state = *last;
        } else {
            println!("no states came back at tick {}; stopping", tape.len());
            break;
        }
        // Where are we now?
        let pos = [state.pos[0] as f64, state.pos[1] as f64, state.pos[2] as f64];
        let (st_now, lat_now) = r.line.locate(pos, hint, 40, 120);
        hint = st_now;
        let d_now = r.line.dist_of(st_now);
        let row = &r.line.rows[st_now];
        let off_track = lat_now > row.right_m + off_margin || lat_now < -(row.left_m + off_margin);
        let stalled = tape.len() > 600 && (state.speed() as f64) < 1.5;
        let too_far = pos[1] < row.y - 3.0 || pos[1] > row.y + 6.0;
        if let Some(v) = adv.ended {
            println!("fork says the run ENDED at tick {}: {:?} (d = {:.0} m)", tape.len(), v, d_now);
            ended = Some(v);
            break;
        }
        if d_now > best_d {
            best_d = d_now;
        }
        // Checkpoint bookkeeping: confirm each geometric crossing with the plain oracle.
        while next_cp < cp_d.len() && d_now > cp_d[next_cp] + 5.0 && d_now < cp_d[next_cp] + 400.0 {
            let t1 = std::time::Instant::now();
            match r.oracle.confirm_echo(&tape) {
                Ok((v, _echo, _uid, desc)) => {
                    let cps = match v {
                        Verdict::Dnf { cps } => cps as usize,
                        Verdict::Finish { .. } => cp_d.len(),
                    };
                    println!(
                        "[{:>6.1}s sim {:>5.1}s] tick {} d={:.0} m: passed CP{} geometrically; plain oracle says {:?} ({}) ({:.1} s)",
                        t_start.elapsed().as_secs_f64(),
                        sim_secs,
                        tape.len(),
                        d_now,
                        next_cp + 1,
                        v,
                        desc.trim(),
                        t1.elapsed().as_secs_f64()
                    );
                    if cps > cps_confirmed {
                        cps_confirmed = cps;
                        let p = out_dir.join(format!("cp{:02}.tape.tsv", cps));
                        write_tape(&p, &tape, r.branch.from).unwrap_or_else(die);
                        r.oracle.write_container(&tape, &out_dir.join(format!("cp{:02}.Ghost.Gbx", cps))).unwrap_or_else(die);
                        println!("  banked {}", p.display());
                    } else {
                        println!("  WARNING: the oracle credits {} checkpoints, geometry says {}", cps, next_cp + 1);
                    }
                }
                Err(e) => println!("  plain oracle refused: {}", e),
            }
            next_cp += 1;
        }
        if off_track || stalled || too_far {
            rewinds += 1;
            let why = if off_track { "off the tarmac" } else if stalled { "stalled" } else { "off the deck" };
            println!(
                "[{:>6.1}s] tick {} d={:.0} m ({}): {} (lateral {:.2}, edges -{:.1}/+{:.1}, speed {:.1} m/s, y {:.1} vs {:.1}); rewind #{}",
                t_start.elapsed().as_secs_f64(),
                tape.len(),
                d_now,
                row.corner,
                why,
                lat_now,
                row.left_m,
                row.right_m,
                state.speed(),
                pos[1],
                row.y,
                rewinds
            );
            if rewinds > max_rewinds {
                println!("too many rewinds; giving up");
                break;
            }
            // Shrink the allowance over the stretch that led here.
            let d_fail = d_now;
            let lo = (d_fail - 150.0).max(0.0) as usize;
            let hi = (d_fail + 30.0).min(n as f64 - 1.0) as usize;
            for d in lo..=hi {
                let st = r.line.station_at(d as f64);
                allow.scale[st] *= shrink;
            }
            // Cut the tape back rewind_s seconds (at least one macro), and further back if still off-track.
            let cut_ticks = ((rewind_s * 100.0) as usize).max(law.k);
            let target_len = tape.len().saturating_sub(cut_ticks);
            while hist.len() > 1 && hist.last().map(|h| h.0 > target_len).unwrap_or(false) {
                hist.pop();
            }
            let (len, st_state, st_hint) = hist.pop().unwrap_or((0, init, st0));
            tape.truncate(len);
            state = st_state;
            hint = st_hint;
            // Also cut anything the checkpoint bookkeeping has moved past.
            let d_after = r.line.dist_of(hint);
            while next_cp > 0 && cp_d[next_cp - 1] > d_after {
                next_cp -= 1;
            }
            println!("  resumed at tick {} (d = {:.0} m), allowance {:.2} at the failure", tape.len(), d_after, allow.scale[r.line.station_at(d_fail)]);
            continue;
        }
        if tape.len() % 500 == 0 {
            println!(
                "[{:>6.1}s sim {:>5.1}s] tick {} d={:.0} m {} speed {:.1} m/s ({:.0} km/h) lateral {:+.2} target {:.1} steer {} gas {} brake {}",
                t_start.elapsed().as_secs_f64(),
                sim_secs,
                tape.len(),
                d_now,
                row.corner,
                state.speed(),
                state.speed() * 3.6,
                lat_now,
                v_target,
                input.steer,
                input.gas as u8,
                input.brake as u8
            );
            std::fs::write(&log_path, &log).ok();
            write_tape(&out_dir.join("current.tape.tsv"), &tape, r.branch.from).ok();
        }
        if d_now > d_finish + 60.0 {
            println!("past the finish line by 60 m and the fork did not end the run; stopping");
            break;
        }
    }
    std::fs::write(&log_path, &log).ok();
    let final_tape = out_dir.join("final.tape.tsv");
    write_tape(&final_tape, &tape, r.branch.from).unwrap_or_else(die);
    let final_ghost = out_dir.join("final.Ghost.Gbx");
    r.oracle.write_container(&tape, &final_ghost).unwrap_or_else(die);
    println!("container {}", final_ghost.display());
    println!(
        "done: {} ticks ({} s of tape), best d {:.0} m of {:.0}, {} rewinds, {:.0} s wall ({:.0} s in the engine); tape {}",
        tape.len(),
        secs(tape.len() as i64 * 10),
        best_d,
        d_finish,
        rewinds,
        t_start.elapsed().as_secs_f64(),
        sim_secs,
        final_tape.display()
    );
    // The plain oracle is the verdict.
    match r.oracle.confirm_echo(&tape) {
        Ok((v, _echo, uid, desc)) => {
            println!("PLAIN ORACLE: {:?} desc {:?} map {}", v, desc.trim(), uid);
            if let Verdict::Finish { ms } = v {
                println!("VALIDATED FINISH {} s", secs(ms as i64));
            }
        }
        Err(e) => println!("plain oracle REFUSED: {}", e),
    }
    let _ = ended;
}

/// `tmexplore-real write --template T --tape TAPE.tsv --out F.Ghost.Gbx [--prefix N]`
///
/// Lay a driven tape into a container at its frame and KEEP the file. The tape
/// file's `# frame` line is the boundary it was driven at; `--prefix` overrides
/// it. The written bytes are exactly what `confirm` would hand the oracle, so
/// the same tape in two templates that share their inputs below the frame
/// (e.g. the telemetry-less search template and the 50 ms-grid one that a
/// filmable ghost needs) is the same run.
pub fn write(a: &Args) {
    let template = PathBuf::from(a.req("template"));
    let map = PathBuf::from(a.req("map"));
    let server = PathBuf::from(a.get("server").unwrap_or("/tmp/tmoracle/server"));
    let out = PathBuf::from(a.req("out"));
    let work = PathBuf::from(a.get("work").unwrap_or("/tmp/tm-drive/write"));
    let tape_path = PathBuf::from(a.req("tape"));
    let txt = std::fs::read_to_string(&tape_path).unwrap_or_else(die);
    let frame: Option<u64> = txt
        .lines()
        .find_map(|l| l.strip_prefix("# frame").and_then(|r| r.trim().parse().ok()));
    let prefix: u64 = match (a.get("prefix"), frame) {
        (Some(p), _) => p.parse().unwrap_or_else(|_| die("--prefix wants a tick")),
        (None, Some(f)) => f,
        (None, None) => die("the tape carries no `# frame` line; pass --prefix"),
    };
    let tape = read_tape(&tape_path).unwrap_or_else(die);
    let oracle = EngineOracle::new(&template, &map, &server, &work).unwrap_or_else(die);
    oracle.set_prefix_ticks(prefix);
    let bytes = oracle.write_container(&tape, &out).unwrap_or_else(die);
    println!(
        "wrote {} ({} bytes): {} ticks of tape at file tick {}, template {}",
        out.display(),
        bytes.len(),
        tape.len(),
        prefix,
        template.display()
    );
    if a.flag("confirm") {
        match oracle.confirm_echo(&tape) {
            Ok((v, _echo, uid, desc)) => println!("plain oracle: {:?} desc {:?} map {}", v, desc.trim(), uid),
            Err(e) => println!("plain oracle REFUSED: {}", e),
        }
    }
}
