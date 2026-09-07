//! `tmenv` — a Gym-shaped environment for TM2020, over our own instrument.
//!
//! # What this is, and what it is not
//!
//! It is an environment: `reset`, `step(action) -> (obs, reward, done, info)`,
//! over the real engine, with the car's true physics state as the observation
//! rather than a screenshot.
//!
//! It is **not** an oracle. Nothing this crate returns is a result. A finish
//! the env reports is the env's own reading of a trajectory read out of a
//! forked engine, and the fork server's own regime warning applies to it: 0 of
//! 312 fork-reported finishes once survived re-simulation. A result is a tape
//! written to disk that the **plain oracle** re-simulates, and
//! [`control::oracle_agrees`] is the check that says so.
//!
//! # The layering, and why the core is separate
//!
//! ```text
//!   ForkEnv  ──rows──►  Core  ──►  obs / reward / done
//!      │                  ▲
//!   branch::Forest        │
//!      │              (same code)
//!   fork server           │
//!                    flat trajectory  ◄── the control
//! ```
//!
//! [`core::Core`] consumes per-tick car state and knows nothing about where it
//! came from. That is what makes it possible to run the identical reward and
//! termination logic over a trajectory obtained by a completely different path,
//! which is the only way to catch an environment that returns plausible
//! fiction. An env that silently lies trains a policy on fiction and nothing
//! downstream can see it.
//!
//! # No ghost, anywhere
//!
//! The route, the stations, the corridor and the gate positions come from
//! `mapgeom`, whose inputs are the `.Map.Gbx` and the game's own pak. The
//! author time comes from the map file's own header. No `.Ghost.Gbx` a human
//! drove is read by anything here, for any purpose — not as a reference line,
//! not for tuning, not as a yardstick.

pub mod action;
pub mod archive;
pub mod control;
pub mod cpfind;
pub mod core;
pub mod forkenv;
pub mod geom;
pub mod template;
pub mod sanity;
pub mod track;

pub use action::{Act, ActionSpace};
pub use core::{Core, CoreCfg, Done, Info};
pub use forkenv::{ForkEnv, Rig, Rollout, Span};
pub use track::Track;

use std::path::Path;

/// Build the track with the tour's origin MEASURED from the engine, caching the
/// measurement beside the reference container.
///
/// The measurement costs two server starts and a memory sweep — about a minute
/// — and the answer is a property of the map and the build, not of the run, so
/// it is paid once and read thereafter. The cache records the position it
/// measured *and* the self-check that licensed it; a cache file that does not
/// parse is ignored rather than guessed at.
pub fn load_track_measured(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    reference: &Path,
) -> Result<Track, String> {
    let uid = map
        .file_name()
        .map(|s| s.to_string_lossy().replace(".Map.Gbx", ""))
        .unwrap_or_default();
    let cache = reference
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("spawn-{uid}.txt"));
    let spawn = match std::fs::read_to_string(&cache).ok().and_then(|s| parse_spawn(&s)) {
        Some(p) => p,
        None => {
            let (fix, _rows) = control::measure_spawn(server, map, shim, work, reference)?;
            let _ = std::fs::write(
                &cache,
                format!(
                    "# where the dedicated server ACTUALLY starts the car on this map, measured\n\
                     # from the engine. Not the map's Spawn waypoint; see mapgeom\n\
                     # packrun::Opts::spawn_override. Delete this file to re-measure.\n\
                     spawn {} {} {}\nrace_ms {}\nspeed {:.4}\nprobe_tick {}\n",
                    fix.pos[0], fix.pos[1], fix.pos[2], fix.race_ms, fix.speed, fix.probe_tick
                ),
            );
            fix.pos
        }
    };
    load_track_from(server, map, Some(spawn))
}

/// The measured start for a map: from the cache beside the reference, else
/// measured from the engine and cached.
pub fn measured_spawn(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    reference: &Path,
) -> Result<[f32; 3], String> {
    let uid = map
        .file_name()
        .map(|s| s.to_string_lossy().replace(".Map.Gbx", ""))
        .unwrap_or_default();
    // Keyed on the CONTAINER'S CONTENT, not on the map.
    //
    // Where the engine starts the car depends on the container -- the
    // synthesized record/sample state seeds the validator's vehicle -- so a
    // cache keyed on the map alone goes stale the moment the container writer
    // changes, and the env would then be certified against a start that is no
    // longer where the car is. That is the worst kind of stale: everything
    // downstream keeps passing.
    let tag = short_hash(reference);
    let cache = reference
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("spawn-{uid}-{tag}.txt"));
    if let Some(p) = std::fs::read_to_string(&cache).ok().and_then(|s| parse_spawn(&s)) {
        return Ok(p);
    }
    let (fix, _) = control::measure_spawn(server, map, shim, work, reference)?;
    let _ = std::fs::write(
        &cache,
        format!(
            "# where the dedicated server ACTUALLY starts the car on this map, measured\n\
             # from the engine. Not the map's Spawn waypoint; see mapgeom\n\
             # packrun::Opts::spawn_override. Delete this file to re-measure.\n\
             spawn {} {} {}\nrace_ms {}\nspeed {:.4}\nprobe_tick {}\n",
            fix.pos[0], fix.pos[1], fix.pos[2], fix.race_ms, fix.speed, fix.probe_tick
        ),
    );
    Ok(fix.pos)
}

fn parse_spawn(s: &str) -> Option<[f32; 3]> {
    for l in s.lines() {
        if let Some(rest) = l.strip_prefix("spawn ") {
            let v: Vec<f32> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if v.len() == 3 {
                return Some([v[0], v[1], v[2]]);
            }
        }
    }
    None
}

/// Build the track for a map, from the map file and the game's own pak.
///
/// This goes through `mapgeom`'s library rather than reading the `pack.json` /
/// `route.json` a previous run left behind. The JSON is a convenience; the
/// library is the contract, and a route parsed back out of a file is a route
/// that can be stale, hand-edited, or from another map without anything saying
/// so. Building it costs a few seconds once per process and every env in that
/// process shares it.
pub fn load_track(server: &Path, map: &Path) -> Result<Track, String> {
    load_track_from(server, map, None)
}

/// [`load_track`], with the tour's origin supplied.
///
/// Pass the position the engine actually spawns the car at, when it is known.
/// See `mapgeom::packrun::Opts::spawn_override` for why that is not always the
/// map's own `Spawn` waypoint, and [`measure_spawn`] for how to get it.
pub fn load_track_from(
    server: &Path,
    map: &Path,
    spawn: Option<[f32; 3]>,
) -> Result<Track, String> {
    let packs = server.join("Packs");
    let mut paths: Vec<String> = Vec::new();
    for name in ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"] {
        let p = packs.join(name);
        if p.exists() {
            paths.push(p.to_string_lossy().into_owned());
        }
    }
    if paths.is_empty() {
        return Err(format!("no .pak files under {}", packs.display()));
    }
    let key = std::env::var("TM_PAK_KEY")
        .unwrap_or_else(|_| mapgeom::store::STADIUM_KEY.to_string());
    let mut store = mapgeom::store::DataStore::open(&paths, &key)?;

    let p = map.to_string_lossy().into_owned();
    let uid = map
        .file_name()
        .map(|s| s.to_string_lossy().replace(".Map.Gbx", ""))
        .unwrap_or_default();
    let hdr = tmmaps::header::read(&p).ok();
    let name = hdr.as_ref().map(|h| h.name.clone()).unwrap_or_else(|| uid.clone());
    let author_ms = tmmaps::header::times(&p).ok().and_then(|t| t.author_ms);

    let b = mapgeom::packrun::build(
        &mut store,
        &p,
        &uid,
        &name,
        author_ms,
        &mapgeom::packrun::Opts {
            with_deco: true,
            verbose: false,
            step: mapgeom::surf::STEP,
            spawn_override: spawn,
        },
    )?;
    Ok(Track::new(b.pack, b.route))
}

/// A short content hash of a file, for cache keys.
///
/// FNV-1a over the bytes. Not a security hash and does not need to be: the job
/// is to notice that the container changed, and a container that changed and
/// hashed the same would have to be an adversary rather than an accident.
fn short_hash(p: &Path) -> String {
    match std::fs::read(p) {
        Err(_) => "nofile".into(),
        Ok(b) => {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for x in &b {
                h ^= *x as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
            format!("{h:016x}")
        }
    }
}
