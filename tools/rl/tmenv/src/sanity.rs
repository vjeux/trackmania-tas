//! `tmenv start-sanity` -- does the env START right on this map?
//!
//! LEARN's first held-out eval (M2-0, 2026-09-07) had the CONST control (steer
//! 0, full gas) die at 14-60 m on 13 of 20 maps, within ~1.5 s: an
//! episode-START defect -- spawn vs the geometry's start, the start block's
//! kind, the template -- not a policy failure. This runs, per map, with the
//! rank-1 ghost's OWN container as the template (so the identity replay carries
//! the ghost's validation seed, INPUT's VALIDATION-SEED.md):
//!
//! 1. the IDENTITY REPLAY: the ghost's tape through the env at offset 0 must
//!    finish and the plain oracle must give the ghost's own time;
//! 2. CONST for `--seconds` (3): best progress, distance driven, the `Done`;
//! 3. the measured spawn vs the geometry's spawn, and the start block's name;
//!
//! and writes `env-sanity.json` into the map dir (DATA's dataset gate, LEARN's
//! eval filter) plus a copy under `--out-dir`. Errors are recorded per map,
//! never fatal for the run: 340 maps in one pass, `--threads` at a time.

use crate::core::{CoreCfg, Done};
use crate::forkenv::{build_at_start, RootCfg};
use crate::ActionSpace;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tmstate::Action;

#[derive(Default, Clone, Debug)]
pub struct MapSanity {
    pub uid: String,
    pub name: String,
    pub ghost: String,
    pub ghost_ms: i64,
    pub geom_source: String,
    pub spawn_measured: Option<[f32; 3]>,
    pub spawn_geom: Option<[f32; 3]>,
    pub spawn_vs_geom_m: Option<f32>,
    pub start_blocks: Vec<String>,
    pub car_switch_blocks: Vec<String>,
    pub identity_ok: Option<bool>,
    pub identity_declared_ms: Option<i64>,
    pub identity_oracle: String,
    pub identity_env_gates: Option<usize>,
    pub identity_n_gates: Option<usize>,
    pub identity_max_jump_m: Option<f64>,
    pub const_m: Option<f32>,
    pub const_best_s: Option<f32>,
    pub const_done: String,
    pub const_ticks: Option<usize>,
    pub const_speed: Option<f32>,
    pub error: String,
    pub seconds: f64,
}

fn jstr(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn jopt<T: std::fmt::Display>(v: &Option<T>) -> String {
    match v {
        Some(x) => x.to_string(),
        None => "null".into(),
    }
}

fn jvec3(v: &Option<[f32; 3]>) -> String {
    match v {
        Some(p) => format!("[{:.3}, {:.3}, {:.3}]", p[0], p[1], p[2]),
        None => "null".into(),
    }
}

impl MapSanity {
    pub fn to_json(&self) -> String {
        format!(
            "{{\n  \"uid\": {},\n  \"name\": {},\n  \"ghost\": {},\n  \"ghost_ms\": {},\n  \"geom_source\": {},\n  \
             \"spawn_measured\": {},\n  \"spawn_geom\": {},\n  \"spawn_vs_geom_m\": {},\n  \"start_blocks\": [{}],\n  \
             \"car_switch_blocks\": [{}],\n  \"identity_ok\": {},\n  \"identity_declared_ms\": {},\n  \"identity_oracle\": {},\n  \
             \"identity_env_gates\": {},\n  \"identity_n_gates\": {},\n  \"identity_max_jump_m\": {},\n  \
             \"const_m\": {},\n  \"const_best_s\": {},\n  \"const_done\": {},\n  \"const_ticks\": {},\n  \"const_speed\": {},\n  \
             \"error\": {},\n  \"seconds\": {:.1}\n}}\n",
            jstr(&self.uid),
            jstr(&self.name),
            jstr(&self.ghost),
            self.ghost_ms,
            jstr(&self.geom_source),
            jvec3(&self.spawn_measured),
            jvec3(&self.spawn_geom),
            jopt(&self.spawn_vs_geom_m.map(|v| format!("{v:.3}"))),
            self.start_blocks.iter().map(|s| jstr(s)).collect::<Vec<_>>().join(", "),
            self.car_switch_blocks.iter().map(|s| jstr(s)).collect::<Vec<_>>().join(", "),
            jopt(&self.identity_ok),
            jopt(&self.identity_declared_ms),
            jstr(&self.identity_oracle),
            jopt(&self.identity_env_gates),
            jopt(&self.identity_n_gates),
            jopt(&self.identity_max_jump_m.map(|v| format!("{v:.3}"))),
            jopt(&self.const_m.map(|v| format!("{v:.2}"))),
            jopt(&self.const_best_s.map(|v| format!("{v:.2}"))),
            jstr(&self.const_done),
            jopt(&self.const_ticks),
            jopt(&self.const_speed.map(|v| format!("{v:.2}"))),
            jstr(&self.error),
            self.seconds
        )
    }
}

/// The fastest (lowest ms) ghost in `<map>/ghosts/`, by the file name
/// `<rank>-<ms>.Ghost.Gbx`.
pub fn fastest_ghost(map_dir: &Path) -> Result<(PathBuf, i64), String> {
    fastest_ghost_in(&map_dir.join("ghosts"))
}

/// The map's ghosts directory: `<map>/ghosts/` when extracted, else `ghosts.tar`
/// unpacked into `work/ghosts/` (the pool keeps 356 of 393 maps tarred).
pub fn ghosts_dir(map_dir: &Path, work: &Path) -> Result<PathBuf, String> {
    let d = map_dir.join("ghosts");
    if d.is_dir() && fastest_ghost_in(&d).is_ok() {
        return Ok(d);
    }
    let tar = map_dir.join("ghosts.tar");
    if !tar.exists() {
        return Err(format!("{}: neither ghosts/ nor ghosts.tar", map_dir.display()));
    }
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let st = std::process::Command::new("tar").args(["xf", &tar.to_string_lossy(), "-C", &work.to_string_lossy()]).status().map_err(|e| format!("tar: {e}"))?;
    if !st.success() {
        return Err(format!("tar xf {} failed", tar.display()));
    }
    let out = work.join("ghosts");
    if out.is_dir() {
        Ok(out)
    } else {
        Err(format!("{}: the tar holds no ghosts/ directory", tar.display()))
    }
}

pub fn fastest_ghost_in(dir: &Path) -> Result<(PathBuf, i64), String> {
    let mut best: Option<(PathBuf, i64)> = None;
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let p = e.map_err(|e| e.to_string())?.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.ends_with(".Ghost.Gbx") {
            continue;
        }
        let ms: i64 = name.trim_end_matches(".Ghost.Gbx").split('-').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        if ms > 0 && best.as_ref().map(|(_, m)| ms < *m).unwrap_or(true) {
            best = Some((p, ms));
        }
    }
    best.ok_or_else(|| format!("{}: no ghosts", dir.display()))
}

/// The map's start block(s) / item(s), by name.
pub fn start_blocks(map: &Path) -> Vec<String> {
    let Ok(mf) = tmmaps::map::MapFile::try_load(map) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for b in mf.blocks.iter().chain(mf.baked.iter()) {
        let l = b.name.to_ascii_lowercase();
        if l.contains("start") && !out.contains(&b.name) {
            out.push(b.name.clone());
        }
    }
    for it in &mf.items {
        let l = it.model.to_ascii_lowercase();
        if l.contains("start") && !out.contains(&it.model) {
            out.push(it.model.clone());
        }
    }
    out
}

pub struct SanityCfg {
    pub server: PathBuf,
    pub shim: PathBuf,
    pub work: PathBuf,
    pub seconds: f32,
    pub write_into_map_dir: bool,
    pub out_dir: PathBuf,
}

/// One map, start to finish; never panics on the map's account.
pub fn sanity_one(cfg: &SanityCfg, map_dir: &Path) -> MapSanity {
    let t0 = std::time::Instant::now();
    let uid = map_dir.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut r = MapSanity { uid: uid.clone(), ..Default::default() };
    let map = map_dir.join("map.Map.Gbx");
    if let Ok(h) = tmmaps::header::read(&map.to_string_lossy()) {
        r.name = h.name;
    }
    r.start_blocks = start_blocks(&map);
    r.car_switch_blocks = crate::track::car_switch_blocks(&map);
    let work = cfg.work.join(&uid);
    let _ = std::fs::remove_dir_all(&work);
    if let Err(e) = std::fs::create_dir_all(&work) {
        r.error = format!("work dir: {e}");
        r.seconds = t0.elapsed().as_secs_f64();
        return r;
    }
    let res = (|| -> Result<(), String> {
        // 1. the rank-1 ghost's own container as the template
        let gdir = ghosts_dir(map_dir, &work)?;
        let (ghost, ms) = fastest_ghost_in(&gdir)?;
        r.ghost = ghost.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        r.ghost_ms = ms;
        let tpl = crate::template::Template::load(&ghost)?;
        let n = tpl.facts().ticks;
        let s: Vec<u8> = (0..n).map(|t| ((((t as i64 * 7919 + 13) % 25) - 12) as i8) as u8).collect();
        let reference = work.join("reference.Ghost.Gbx");
        tpl.write_with_inputs(&s, &vec![1u8; n], &vec![0u8; n], &reference)?;

        // 2. the geometry: DATA's geom.json when the map dir has one, else measured
        let gj = map_dir.join("geom.json");
        let track = if gj.exists() {
            r.geom_source = "geom.json".into();
            crate::Track::load_geom_json(&gj)?
        } else {
            r.geom_source = "cartographer".into();
            crate::load_track_measured(&cfg.server, &map, &cfg.shim, &work.join("spawnfix"), &reference)?
        };
        r.spawn_geom = Some(track.geom.spawn);
        let track = std::sync::Arc::new(track);

        // 3. the measured spawn (engine memory) vs the geometry's
        let spawn = crate::measured_spawn(&cfg.server, &map, &cfg.shim, &work.join("spawnfix"), &reference)?;
        r.spawn_measured = Some(spawn);
        let g = track.geom.spawn;
        r.spawn_vs_geom_m = Some(((spawn[0] - g[0]).powi(2) + (spawn[1] - g[1]).powi(2) + (spawn[2] - g[2]).powi(2)).sqrt());

        // 4. the env, rooted at the start
        let core_cfg = CoreCfg { k_ticks: 10, max_ticks: n.saturating_sub(1).max(100), ..Default::default() };
        let root = RootCfg { require_start: Some((spawn, 6.0, 4.0)), verbose: false, ..Default::default() };
        let (mut env, _rig, tape) =
            build_at_start(&cfg.server, &map, &cfg.shim, &work, &reference, track.clone(), ActionSpace::default(), core_cfg, &root)?;
        env.allow_after_done = true;
        r.identity_n_gates = Some(track.n_gates());

        // 5. the identity replay (own container: donor index = env tick)
        let d = fk::tape::Tape::load(&ghost.to_string_lossy())?;
        env.reset()?;
        let mut t = env.next_tick()?;
        let mut last_cps = 0usize;
        loop {
            if t >= tape.n() {
                break;
            }
            let k = 10.min(tape.n() - t);
            let chunk: Vec<Action> = (t..t + k)
                .map(|j| {
                    let j = j.min(d.n() - 1);
                    Action { steer: d.steer[j] as i8, gas: d.accel[j] != 0, brake: d.brake[j] != 0 }
                })
                .collect();
            let (_o, _rw, dn, _info) = env.step_ticks(&chunk)?;
            t = env.next_tick().unwrap_or(t + k);
            if matches!(dn, Some(Done::Finished) | Some(Done::TickCap) | Some(Done::RunEnded)) {
                break;
            }
        }
        {
            let rec = env.rollout_record();
            let mut max_jump = 0.0f64;
            for w in rec.trace.windows(2) {
                let dd = ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2) + (w[1].z - w[0].z).powi(2)).sqrt();
                if dd > max_jump {
                    max_jump = dd;
                }
                if w[1].cps != u32::MAX {
                    last_cps = last_cps.max(w[1].cps as usize);
                }
            }
            r.identity_max_jump_m = Some(max_jump);
            r.identity_env_gates = Some(last_cps);
        }
        let banked = work.join("identity.Ghost.Gbx");
        let (bs, bg, bb) = env.banked_tape(&tape);
        tape.write_candidate(&bs, &bg, &bb, &banked)?;
        r.identity_declared_ms = d.declared_ms.map(|x| x as i64);
        let batch = tmauto::oracle::validate_raw(&cfg.server, &[banked.clone()], tmauto::oracle::Maps::One(&map), "sanity")?;
        match batch.answers.first() {
            Some(ans) => {
                let v = ans.verdict();
                r.identity_oracle = format!("{:?}", v);
                r.identity_ok = Some(matches!(v, Some(tmauto::Verdict::Finish { ms }) if Some(ms as i64) == r.identity_declared_ms));
            }
            None => {
                r.identity_oracle = "no answer".into();
                r.identity_ok = Some(false);
            }
        }

        // 6. CONST: steer 0, full gas, `seconds`
        env.reset()?;
        let steps = ((cfg.seconds * 100.0) as usize / 10).max(1);
        let gas = Action { steer: 0, gas: true, brake: false };
        let mut done: Option<Done> = None;
        let mut ticks = 0usize;
        for _ in 0..steps {
            let (_o, _rw, dn, _info) = env.step_ticks(&[gas; 10])?;
            ticks += 10;
            if dn.is_some() {
                done = dn;
                break;
            }
        }
        let st = env.core.state();
        r.const_ticks = Some(ticks);
        r.const_best_s = Some(env.core.best_s());
        r.const_m = Some(((st.pos[0] - spawn[0]).powi(2) + (st.pos[1] - spawn[1]).powi(2) + (st.pos[2] - spawn[2]).powi(2)).sqrt());
        r.const_speed = Some(st.speed);
        r.const_done = match done {
            Some(d) => format!("{:?}", d),
            None => "running".into(),
        };
        Ok(())
    })();
    if let Err(e) = res {
        r.error = e;
    }
    r.seconds = t0.elapsed().as_secs_f64();
    let json = r.to_json();
    let _ = std::fs::create_dir_all(&cfg.out_dir);
    let _ = std::fs::write(cfg.out_dir.join(format!("{uid}.json")), &json);
    if cfg.write_into_map_dir {
        let _ = std::fs::write(map_dir.join("env-sanity.json"), &json);
    }
    // the env's servers die with the work dir's rig; keep the dir small
    let _ = std::fs::remove_dir_all(work.join("traces"));
    r
}

/// Every map dir under `maps_dir`, `threads` at a time.
pub fn sanity_all(cfg: &SanityCfg, maps: &[PathBuf], threads: usize) -> Vec<MapSanity> {
    let next = AtomicUsize::new(0);
    let out: std::sync::Mutex<Vec<MapSanity>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1).min(maps.len().max(1)) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= maps.len() {
                    break;
                }
                let r = sanity_one(cfg, &maps[i]);
                println!(
                    "[{}/{}] {:<26} identity {:<5} ({}) const {:>6} m {:<12} spawn/geom {} err {}",
                    i + 1,
                    maps.len(),
                    r.name.chars().take(26).collect::<String>(),
                    r.identity_ok.map(|b| if b { "OK" } else { "FAIL" }).unwrap_or("-"),
                    r.identity_oracle,
                    r.const_m.map(|v| format!("{v:.1}")).unwrap_or("-".into()),
                    r.const_done,
                    r.spawn_vs_geom_m.map(|v| format!("{v:.1}")).unwrap_or("-".into()),
                    if r.error.is_empty() { "-".to_string() } else { r.error.chars().take(80).collect() }
                );
                out.lock().unwrap().push(r);
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by(|a, b| a.uid.cmp(&b.uid));
    v
}
