//! `tinyctl video` — one map's driven lap as a video and a contact sheet, from
//! the devserver, through the render box — **with the run's controls drawn on
//! it, by default, checked, and stamped.**
//!
//! ```text
//! tinyctl video --map NN | --all [--watch SECS] [--ghost F] [--out /tmp/tinyvid] [--maps-dir /tmp/audit/ship9]
//!               [--ghosts-dir /tmp/ghosts] [--ghosts-sync host:dir] [--build ship15] [--cam 2] [--load-timeout 120] [--no-guard]
//!               [--box-videos "…/Maps/Tiny/videos"] [--store host:dir | dir] [--pull-webm] [--suffix S]
//!               [--from-webm F] [--no-overlay] [--crf N] [--offset-ms N] [--ship [--readme tiny/README.md]]
//!               [--box-shootctl P] [--wsx P] [-v]
//! tinyctl shipwatch --out /tmp/tinyvid --readme tiny/README.md [--repo DIR] [--once] [--commit] [--wsx P]
//! ```
//!
//! What happens, and where:
//!
//! 1. THE GUARD. A `.Ghost.Gbx` is inputs + samples, and a client render PLAYS
//!    SAMPLES: a validation container whose samples are a full-size donor's
//!    renders as a car flying off a half-size map, and the night of 2026-09-08
//!    was spent believing that. So sample 0 must sit where the client puts the
//!    car at rest on THIS map — the Spawn placement plus the start block's
//!    centre, (8, ·, 8) turned by the placement's yaw — within three metres (a
//!    start GATE item spawns a metre or two off its pivot) — AND its last sample
//!    within 20 m of a Goal placement, which a donor line (twice as far from the
//!    anchor) never is. `--no-guard` renders
//!    anyway and says so in capitals.
//! 2. the map and the ghost are pushed to the box's staging dir (the map only
//!    when its md5 is not already there — 45 MB over a bridge eight sessions
//!    share is worth a `md5sum`);
//! 3. `shootctl render --detach` runs there: the render lock for the game part
//!    only, the game left up, contact sheets after the lock; this side polls
//!    its done file;
//! 4. the clip is moved on the box to `Maps\Tiny\videos\NN-ghost-<time>.webm`
//!    (the raw render), the sheets are pulled into `--out`, and the clip is
//!    pulled too;
//! 5. **THE OVERLAY (default).** `clip cut --ghost` here, natively: the webm
//!    becomes the publishable mp4 in one pass — trimmed to the lap, x264 at a
//!    crf chosen for the lap's length (under GitHub's 100 MB), the run's own
//!    inputs drawn on it (steer, gas, brake, respawn, the strip), the video↔tape
//!    timing CHECKED against the picture (`clip sync`: the world's sideways
//!    motion must follow the tape's yaw where a correct clip does; a render
//!    that started early, a wrong ghost, a static frame all refuse), and the
//!    file stamped with the marker `clip ship` requires. vjeux, 2026-09-09:
//!    "add the controls overlay on the videos you generate" — "do not make it a
//!    rule, change the renderer to do it by default". `--no-overlay` makes a
//!    bare mp4 and says so in capitals; `clip ship` will refuse that file.
//! 6. the mp4 is pushed to the box beside the webm (where vjeux watches them,
//!    and where the ship step runs); with `--store` webm, mp4 and sheet go to
//!    the shared store as well (`host:dir` goes through scp — an OD has no
//!    manifold mount);
//! 7. `--ship`: the box-side publish (`tools/tinyctl/box/tinyship.sh`: cookie
//!    probe, `clip ship --no-mirror`, the anonymous gate) is started detached
//!    and recorded in `<out>/ships.tsv`; `tinyctl shipwatch` polls those done
//!    files, swaps the map's row in the page (`--readme`) when the URL is in,
//!    and with `--commit` commits and pushes it. The gate can take an hour on a
//!    big file, which is why the render loop does not wait for it.
//!
//! `--from-webm F` skips steps 2–4 and runs 5–7 on an existing render (the store
//! copy of a clip published before the overlay existed): the default path is
//! what runs, whatever the clip's origin.
//!
//! The REPORT.md row per lap goes to `<out>/REPORT.md` and stdout: time,
//! checkpoints, the overlay column (offset, how it was checked), the sheet.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::wsx::{to_win, Wsx};

const STAGE: &str = "/home/vjeux/shoot/_stage";
const VID: &str = "/mnt/c/Users/vjeux/tinyvid";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const BOX_VIDEOS: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/Tiny/videos";
const BOX_SHIP_SH: &str = "/home/vjeux/shoot/tinyship.sh";
/// GitHub's user-attachments cap is 100 MB; leave room for the container.
const MP4_MAX_BYTES: u64 = 99_000_000;

/// One rendered lap, as the state file records it.
struct Done {
    nn: String,
    ghost_md5: String,
    time: String,
    cps: usize,
    clip: String,
    sheet: PathBuf,
}

/// The page name of a map. 01–20 are the Summer maps by number; 21–25 are the
/// country maps and go by their source headers' names (vjeux, 2026-09-09).
pub fn map_title(nn: &str) -> String {
    match nn {
        "21" => "Tiny Argentina 2026".into(),
        "22" => "Tiny Saudi Arabia 2026".into(),
        "23" => "Tiny Norway 2026".into(),
        "24" => "Tiny Poland 2026".into(),
        "25" => "Tiny Japan 2026".into(),
        _ => format!("Tiny Summer 2026 - {nn}"),
    }
}

/// The title as `clip ship` registers it in the release body:
/// `tiny-summer-2026-07`, `tiny-norway-2026`.
pub fn map_slug(nn: &str) -> String {
    map_title(nn)
        .to_ascii_lowercase()
        .replace(" - ", "-")
        .replace(' ', "-")
}

/// x264 crf for a lap of this length, so the mp4 stays under GitHub's 100 MB:
/// measured on the ship15 renders (1080p30; the water map 15 at 51 s was
/// 100.2 MB at crf 19 and 56.7 MB at 24; 24 at 150 s needed 30).
pub fn crf_for(lap_s: f64) -> u32 {
    if lap_s <= 25.0 {
        20
    } else if lap_s <= 40.0 {
        22
    } else if lap_s <= 60.0 {
        24
    } else if lap_s <= 120.0 {
        28
    } else {
        30
    }
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    if tmmaps::cli::has(args, "--all") {
        return all(args);
    }
    one(args).map(|_| ())
}

/// The ghosts whose README row (`| map | file | time | credits | build | …`)
/// names `build`; every map when the README is absent or names no build.
fn ghosts_on_build(ghosts_dir: &Path, build: &str) -> Option<std::collections::BTreeSet<String>> {
    let text = std::fs::read_to_string(ghosts_dir.join("README.md")).ok()?;
    let mut set = std::collections::BTreeSet::new();
    let mut any = false;
    for l in text.lines() {
        let cells: Vec<&str> = l.split('|').map(|c| c.trim()).collect();
        // | map | file | time | credits | build | ...
        if cells.len() >= 6 && cells[1].len() == 2 && cells[1].chars().all(|c| c.is_ascii_digit()) && cells[2].ends_with(".Ghost.Gbx") {
            any = true;
            if cells[5] == build {
                set.insert(cells[1].to_string());
            }
        }
    }
    if any { Some(set) } else { None }
}

/// `--all`: every `NN.Ghost.Gbx` in `--ghosts-dir` whose md5 is not yet in
/// `<out>/videos.tsv` (nn, ghost md5, time, cps, clip, sheet, when), rendered in
/// turn — the loop of a day when laps land every half hour. A ghost REPLACED
/// under the same name (a better lap, a regenerated file) has a new md5 and is
/// rendered again; a failure is printed and the loop goes on to the next map.
/// `--watch SECS` repeats the scan forever; `--ghosts-sync host:dir` rsyncs the
/// ghosts dir from there before each scan; `--build ship15` takes only the
/// ghosts whose README row names that build.
fn all(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let watch: Option<u64> = f("--watch").map(|s| s.parse().map_err(|_| "--watch wants seconds")).transpose()?;
    loop {
        let r = all_once(args);
        match (&r, watch) {
            (_, None) => return r,
            (Err(e), Some(_)) => eprintln!("scan: {e}"),
            (Ok(()), Some(_)) => {}
        }
        let secs = watch.unwrap();
        eprintln!("[watch] next scan in {secs}s ({})", chrono_now());
        std::thread::sleep(Duration::from_secs(secs));
    }
}

fn chrono_now() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{:02}:{:02}:{:02}Z", (s / 3600) % 24, (s / 60) % 60, s % 60)
}

fn all_once(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let ghosts_dir = PathBuf::from(f("--ghosts-dir").unwrap_or_else(|| "/tmp/ghosts".into()));
    let out = PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/tinyvid".into()));
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    if let Some(src) = f("--ghosts-sync") {
        std::fs::create_dir_all(&ghosts_dir).map_err(|e| format!("{}: {e}", ghosts_dir.display()))?;
        let src = if src.ends_with('/') { src } else { format!("{src}/") };
        let st = Command::new("rsync")
            .args(["-a", "-e", "ssh -o BatchMode=yes"])
            .arg(&src)
            .arg(format!("{}/", ghosts_dir.display()))
            .status()
            .map_err(|e| format!("rsync: {e}"))?;
        if !st.success() {
            return Err(format!("rsync {src} → {}: {st}", ghosts_dir.display()));
        }
    }
    let state = out.join("videos.tsv");
    let seen: Vec<(String, String)> = std::fs::read_to_string(&state)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let mut c = l.split('\t');
            Some((c.next()?.to_string(), c.next()?.to_string()))
        })
        .collect();
    let mut names: Vec<String> = std::fs::read_dir(&ghosts_dir)
        .map_err(|e| format!("{}: {e}", ghosts_dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .filter(|n| n.len() == 12 && n.ends_with(".Ghost.Gbx") && n[..2].chars().all(|c| c.is_ascii_digit()))
        .collect();
    names.sort();
    let on_build = match f("--build") {
        Some(b) => ghosts_on_build(&ghosts_dir, &b),
        None => None,
    };
    let mut todo = Vec::new();
    for n in &names {
        let nn = n[..2].to_string();
        if let Some(set) = &on_build {
            if !set.contains(&nn) {
                continue;
            }
        }
        let path = ghosts_dir.join(n);
        let md5 = trajectory_id(&gbx::record::decode_ghost(path.to_str().ok_or("ghost path is not utf-8")?)?);
        if seen.iter().any(|(a, b)| *a == nn && *b == md5) {
            continue;
        }
        todo.push(nn);
    }
    if todo.is_empty() {
        println!("nothing new in {} ({} ghosts, all rendered)", ghosts_dir.display(), names.len());
        return Ok(());
    }
    // --adopt: the clips of these ghosts already exist (rendered before this
    // tool, or before the state keyed on trajectories) — record them as done
    // instead of rendering.
    if tmmaps::cli::has(args, "--adopt") {
        let mut text = std::fs::read_to_string(&state).unwrap_or_default();
        if !state.exists() {
            text.push_str("# nn\ttrajectory_id\ttime\tcps\tclip\tsheet\tunix\n");
        }
        for nn in &todo {
            let g = gbx::record::decode_ghost(ghosts_dir.join(format!("{nn}.Ghost.Gbx")).to_str().ok_or("ghost path is not utf-8")?)?;
            let race_ms = g.race_time_ms.or_else(|| g.samples.last().map(|s| s.time_ms)).unwrap_or(0);
            let time = format!("{}.{:03}", race_ms / 1000, race_ms % 1000);
            let cps = g.checkpoints_ms.iter().filter(|c| **c < race_ms - 50).count();
            let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            text.push_str(&format!("{nn}\t{}\t{time}\t{cps}\t{nn}-ghost-{time}.webm\t(adopted)\t{when}\n", trajectory_id(&g)));
            println!("{nn}: adopted ({time}, {cps} cps)");
        }
        std::fs::write(&state, text).map_err(|e| format!("{}: {e}", state.display()))?;
        return Ok(());
    }
    println!("{} to render: {}", todo.len(), todo.join(" "));
    let base: Vec<String> = {
        // the per-map call gets the same flags minus --all and any --map/--ghost
        let mut v = Vec::new();
        let mut skip = false;
        for a in args {
            if skip {
                skip = false;
                continue;
            }
            match a.as_str() {
                "--all" => {}
                "--map" | "--ghost" | "--map-file" | "--watch" | "--ghosts-sync" | "--build" | "--from-webm" => skip = true,
                _ => v.push(a.clone()),
            }
        }
        v
    };
    let mut failed = Vec::new();
    for nn in &todo {
        let mut a = base.clone();
        a.push("--map".into());
        a.push(nn.clone());
        println!("\n=== {nn} ===");
        match one(&a) {
            Ok(d) => record_done(&state, &d)?,
            Err(e) => {
                eprintln!("{nn}: FAILED — {e}");
                failed.push(nn.clone());
            }
        }
    }
    println!("\n{} rendered, {} failed{}", todo.len() - failed.len(), failed.len(), if failed.is_empty() { String::new() } else { format!(" ({})", failed.join(" ")) });
    if failed.is_empty() { Ok(()) } else { Err("some renders failed (above)".into()) }
}

fn record_done(state: &Path, d: &Done) -> Result<(), String> {
    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let row = format!("{}\t{}\t{}\t{}\t{}\t{}\t{when}\n", d.nn, d.ghost_md5, d.time, d.cps, d.clip, d.sheet.display());
    let header = if state.exists() { String::new() } else { "# nn\ttrajectory_id\ttime\tcps\tclip\tsheet\tunix\n".to_string() };
    let mut text = std::fs::read_to_string(state).unwrap_or_default();
    text.push_str(&header);
    text.push_str(&row);
    std::fs::write(state, text).map_err(|e| format!("{}: {e}", state.display()))
}

fn one(args: &[String]) -> Result<Done, String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let nn = f("--map").ok_or("video needs --map NN (01..25)")?;
    if nn.len() != 2 || !nn.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("--map {nn}: two digits, 01..25"));
    }
    let maps_dir = PathBuf::from(f("--maps-dir").unwrap_or_else(|| "/tmp/audit/ship9".into()));
    let ghosts_dir = PathBuf::from(f("--ghosts-dir").unwrap_or_else(|| "/tmp/ghosts".into()));
    let map = f("--map-file").map(PathBuf::from).unwrap_or_else(|| maps_dir.join(format!("Tiny Summer 2026 - {nn}.Map.Gbx")));
    let ghost = f("--ghost").map(PathBuf::from).unwrap_or_else(|| ghosts_dir.join(format!("{nn}.Ghost.Gbx")));
    let out = PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/tinyvid".into()));
    let cam = f("--cam").unwrap_or_else(|| "2".into());
    let load_timeout: u64 = f("--load-timeout").map(|s| s.parse().map_err(|_| "--load-timeout wants seconds")).transpose()?.unwrap_or(120);
    let box_videos = f("--box-videos").unwrap_or_else(|| BOX_VIDEOS.into());
    let shootctl = f("--box-shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let from_webm = f("--from-webm").map(PathBuf::from);
    for p in [&map, &ghost] {
        if !p.is_file() {
            return Err(format!("{}: no such file", p.display()));
        }
    }
    if let Some(w) = &from_webm {
        if !w.is_file() {
            return Err(format!("--from-webm {}: no such file", w.display()));
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    // the overlay needs ffmpeg HERE; find it before the box does any work
    let ff = if tmmaps::cli::has(args, "--no-overlay") && from_webm.is_none() && f("--store").is_none() {
        None
    } else {
        Some(clip::platform::from_env().map_err(|e| format!("the overlay/cut needs ffmpeg on this side: {e}"))?)
    };

    // --- the ghost: time, checkpoints, and THE GUARD
    let g = gbx::record::decode_ghost(ghost.to_str().ok_or("ghost path is not utf-8")?)?;
    let race_ms = g.race_time_ms.or_else(|| g.samples.last().map(|s| s.time_ms)).ok_or("the ghost has no race time and no samples")?;
    let time = format!("{}.{:03}", race_ms / 1000, race_ms % 1000);
    let s0 = g.samples.first().ok_or("the ghost has no samples — nothing to render")?;
    println!("{}: {} samples, race time {time}, checkpoints at {}", ghost.display(), g.samples.len(), g.checkpoints_ms.iter().map(|c| format!("{:.3}", *c as f64 / 1000.0)).collect::<Vec<_>>().join(" "));
    let mut m = tmmaps::map::MapFile::load(&map);
    let spawn = m.items.iter().find(|it| it.waypoint_tag.as_deref() == Some("Spawn")).ok_or_else(|| format!("{}: no placement tagged Spawn", map.display()))?;
    let (dx, dz) = start_centre(spawn.yaw);
    let want = [spawn.pos[0] + dx, spawn.pos[1], spawn.pos[2] + dz];
    // the block-centre model (a block-derived start) or the placement itself (a
    // start GATE item spawns a few metres from its pivot: Summer 19, 5.0 m) —
    // whichever is nearer
    let d_centre = ((s0.x - want[0]).powi(2) + (s0.z - want[2]).powi(2)).sqrt();
    let d_pivot = ((s0.x - spawn.pos[0]).powi(2) + (s0.z - spawn.pos[2]).powi(2)).sqrt();
    let dh = d_centre.min(d_pivot);
    let dy = s0.y - want[1];
    println!(
        "guard: sample 0 at [{:.2}, {:.2}, {:.2}]; Spawn placement [{:.2}, {:.2}, {:.2}] yaw {:.4} → start centre [{:.2}, ·, {:.2}]: {d_centre:.2} m from the centre, {d_pivot:.2} m from the pivot, {dy:+.2} m in height",
        s0.x, s0.y, s0.z, spawn.pos[0], spawn.pos[1], spawn.pos[2], spawn.yaw, want[0], want[2]
    );
    // A block-derived start puts the car exactly at the centre; a start GATE
    // item (Summer 14: a free-positioned Spawn) carries the pack prefab's own
    // spawn point, a metre or two off the placement's pivot — so the start
    // check is loose, and the FINISH check below is what a donor line cannot
    // pass: its last sample sits at the full-size finish, twice as far from the
    // anchor as this map's Goal.
    let on_start = dh <= 8.0 && (-3.0..=3.0).contains(&dy);
    let last = g.samples.last().unwrap();
    let goals: Vec<&tmmaps::map::ItemRec> = m.items.iter().filter(|it| matches!(it.waypoint_tag.as_deref(), Some("Goal") | Some("StartFinish") | Some("Finish"))).collect();
    let goal_d = goals
        .iter()
        .map(|it| ((last.x - it.pos[0]).powi(2) + (last.y - it.pos[1]).powi(2) + (last.z - it.pos[2]).powi(2)).sqrt())
        .fold(f32::INFINITY, f32::min);
    if goals.is_empty() {
        println!("guard: this map has no Goal placement — finish not checked");
    } else {
        println!("guard: last sample at [{:.2}, {:.2}, {:.2}], {goal_d:.1} m from the nearest of {} Goal placement(s)", last.x, last.y, last.z, goals.len());
    }
    let on_finish = goals.is_empty() || goal_d <= 20.0;
    if !on_start || !on_finish {
        if tmmaps::cli::has(args, "--no-guard") {
            println!("GUARD OVERRIDDEN (--no-guard): THIS GHOST'S SAMPLES DO NOT RUN FROM THIS MAP'S START TO ITS FINISH — the clip shows the samples, not this map's physics");
        } else {
            return Err(format!(
                "REFUSED: the ghost's first sample is {dh:.1} m / {dy:+.1} m from where this map starts the car and its last sample {goal_d:.1} m from the nearest Goal. A ghost = inputs + samples, and a render plays the SAMPLES; \
                 this looks like a validation container carrying a donor's line (or a ghost for another build). Ask the player project for the REGENERATED ghost; --no-guard renders it anyway."
            ));
        }
    }

    // --suffix S: `NN-ghost-<time>-S.webm` — the set the clip was rendered on,
    // when one videos folder holds more than one set (ship9 and ship10 clips of
    // the same lap side by side)
    let name = match f("--suffix") {
        Some(s) => format!("{nn}-ghost-{time}-{s}"),
        None => format!("{nn}-ghost-{time}"),
    };
    let wsx = Wsx::new(args);
    let traj_id = trajectory_id(&g);
    let cps = g.checkpoints_ms.iter().filter(|c| **c < race_ms - 50).count();
    let sheet = out.join(format!("{name}-sheet.png"));
    let local_webm = out.join(format!("{name}.webm"));
    let mut bytes: u64;

    if let Some(w) = &from_webm {
        // --- an existing render: the post-render path on it, nothing on the box
        println!("from-webm: {} — no render; cut + overlay + ship on the file as it is", w.display());
        if w.canonicalize().ok() != local_webm.canonicalize().ok() {
            std::fs::copy(w, &local_webm).map_err(|e| format!("{} → {}: {e}", w.display(), local_webm.display()))?;
        }
        bytes = std::fs::metadata(&local_webm).map(|m| m.len()).map_err(|e| e.to_string())?;
        let src_sheet = w.with_file_name(format!("{name}-sheet.png"));
        if src_sheet.is_file() && !sheet.is_file() {
            let _ = std::fs::copy(&src_sheet, &sheet);
        }
        let dur = ff.as_ref().unwrap().probe_duration(&local_webm)?;
        println!("clip: {} ({bytes} bytes, {dur:.3} s; the lap is {time})", local_webm.display());
        if (dur - race_ms as f64 / 1000.0).abs() > 1.0 {
            return Err(format!("the clip is {dur:.3} s for a {time} lap — not this lap's render (or a render that did not cover it)"));
        }
    } else {
        // --- THE RENDER COPY: a renamed, re-uided map and a ghost whose uid literal
        // matches. 2026-09-09: seven maps rendered a STATIC frame (the map's thumbnail
        // camera, no car) although live playback followed the car — the in-game
        // MediaTracker had attached a "Ref. Ghost: Author ghost" (vjeux's finished
        // playtest run, kept by the client and looked up by MAP NAME: a re-uided
        // copy still had it, a renamed one did not — the file is ProgramData\
        // Trackmania\MediaTrackerCache\MTAuthorGhost<map name>.Ghost.gbx, a replay
        // with the map inside; `ghost unwrap` turns it into a plain ghost), and the
        // shoot follows that entity instead of ours. So the file the box renders is
        // never the map under its own name. `--no-rename` renders the map as it is.
        let (rmap, rghost) = if tmmaps::cli::has(args, "--no-rename") {
            (map.clone(), ghost.clone())
        } else {
            let dir = out.join("render");
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let hdr = tmmaps::header::read(map.to_str().ok_or("map path is not utf-8")?)?;
            let render_name = format!("Render {nn} {}", hdr.name);
            let render_uid = format!("Tst1VIDEO2RENDER{nn}{:0>9}", &hdr.uid[hdr.uid.len().saturating_sub(9)..]);
            let render_uid: String = render_uid.chars().take(27).collect();
            let rmap = dir.join(format!("{nn}.Map.Gbx"));
            let (h, b) = m.set_map_name(&hdr.name, &render_name);
            if h + b == 0 {
                return Err(format!("{}: the map does not declare the name {:?}", map.display(), hdr.name));
            }
            // a rename is a variable-length rewrite: write and reload before the uid patch
            m.write_to(&rmap).map_err(|e| format!("{}: {e}", rmap.display()))?;
            let mut m2 = tmmaps::map::MapFile::load(&rmap);
            m2.set_map_uid(&render_uid);
            m2.write_to(&rmap).map_err(|e| format!("{}: {e}", rmap.display()))?;
            let rghost = dir.join(format!("{nn}.Ghost.Gbx"));
            let n = write_ghost_with_uid(&ghost, &rghost, &render_uid)?;
            println!("render copy: {:?} uid {render_uid} -> {} ; ghost uid literal(s) rewritten: {n} -> {}", render_name, rmap.display(), rghost.display());
            (rmap, rghost)
        };

        // --- push: the ghost always, the map only when the box does not have these bytes
        let tag = format!("vid{nn}");
        let r_map = format!("{STAGE}/{tag}.Map.Gbx");
        let r_ghost = format!("{STAGE}/{tag}.Ghost.Gbx");
        let want_md5 = md5_of(&rmap)?;
        let have = wsx.sh(&format!("md5sum '{r_map}' 2>/dev/null | cut -c1-32")).unwrap_or_default().trim().to_string();
        if have == want_md5 {
            eprintln!("map already on the box ({want_md5}) — not pushing");
        } else {
            eprintln!("pushing {} ({} MB) …", rmap.display(), std::fs::metadata(&rmap).map(|m| m.len() / 1_000_000).unwrap_or(0));
            wsx.push(&rmap, &r_map)?;
            let now = wsx.sh(&format!("md5sum '{r_map}' | cut -c1-32")).unwrap_or_default().trim().to_string();
            if now != want_md5 {
                return Err(format!("{r_map} on the box reads md5 {now}, pushed {want_md5}"));
            }
        }
        wsx.push(&rghost, &r_ghost)?;
        let ghost_md5 = md5_of(&rghost)?;
        let rg = wsx.sh(&format!("md5sum '{r_ghost}' | cut -c1-32")).unwrap_or_default().trim().to_string();
        if rg != ghost_md5 {
            return Err(format!("{r_ghost} on the box reads md5 {rg}, pushed {ghost_md5}"));
        }

        // --- render there, poll here
        let r_dir = format!("{VID}/{tag}");
        let cmd = format!("{shootctl} render --detach --map {r_map} --name {tag} --outdir {r_dir} --cam {cam} --load-timeout {load_timeout} --footage {:.1} {r_ghost}", race_ms as f64 / 1000.0);
        eprintln!("rendering {tag} on the box — waits for the render lock if another thread holds the game …");
        let started = wsx.sh(&cmd)?;
        if wsx.verbose {
            eprintln!("{}", started.trim());
        }
        // lock wait (1500 s) + launch + load + the render itself (~2-3 min a lap)
        let done = wsx.wait_done(&format!("{r_dir}/done-render.txt"), &format!("{r_dir}/render.log"), Duration::from_secs(1500 + 900), &format!("render {tag}"))?;
        // OK <webm> <bytes> <seconds>
        let mut it = done.split_whitespace().skip(1);
        let webm = it.next().ok_or_else(|| format!("done file without a clip path: {done}"))?.to_string();
        bytes = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let secs: f64 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        println!("clip: {webm} ({bytes} bytes, {secs:.3} s; the lap is {time})");
        if (secs - race_ms as f64 / 1000.0).abs() > 1.0 {
            println!("NOTE: the clip is {secs:.3} s for a {time} lap — the render did not cover the lap (a camera that never attached plays the whole clip static)");
        }

        // --- the raw clip where vjeux watches them, on the box (MOVED, not copied:
        // one 30 MB copy per lap on a nearly full C:)
        let r_keep = format!("{box_videos}/{name}.webm");
        let mv = wsx.sh(&format!("mkdir -p '{box_videos}' && mv -f '{webm}' '{r_keep}' && stat -c %s '{r_keep}'"))?;
        let kept: u64 = mv.trim().parse().unwrap_or(0);
        if kept != bytes {
            return Err(format!("{r_keep}: {kept} bytes after the move, the clip was {bytes}"));
        }
        println!("box: {} ({kept} bytes)", to_win(&r_keep));

        // --- pull: sheets always, the clip always (the overlay is drawn here)
        let dense = out.join(format!("{name}-dense.png"));
        let n = wsx.pull(&format!("{r_dir}/{tag}-sheet.png"), &sheet)?;
        println!("sheet: {} ({n} B)", sheet.display());
        match wsx.pull(&format!("{r_dir}/{tag}-dense.png"), &dense) {
            Ok(n) => println!("dense: {} ({n} B)", dense.display()),
            Err(e) => eprintln!("dense sheet: {e}"),
        }
        eprintln!("pulling the clip ({} MB) …", bytes / 1_000_000);
        let n = wsx.pull(&r_keep, &local_webm)?;
        if n != bytes {
            return Err(format!("{}: pulled {n} bytes, the clip is {bytes}", local_webm.display()));
        }
        println!("clip: {} ({n} B)", local_webm.display());
    }

    // --- THE OVERLAY: the publishable mp4, by default with the run's controls
    // drawn on it and the timing checked against the picture (see the module
    // doc, step 5). `--no-overlay` is the bare cut, in capitals.
    let mp4 = out.join(format!("{name}.mp4"));
    let lap_s = race_ms as f64 / 1000.0;
    let mut crf: u32 = f("--crf").map(|s| s.parse().map_err(|_| "--crf wants a number")).transpose()?.unwrap_or_else(|| crf_for(lap_s));
    let overlay_col: String;
    {
        let ff = ff.as_ref().unwrap();
        let bare = tmmaps::cli::has(args, "--no-overlay");
        let offset_ms: Option<i64> = f("--offset-ms").map(|s| s.parse().map_err(|_| "--offset-ms wants ms")).transpose()?;
        let mut tries = 0;
        loop {
            tries += 1;
            let o = clip::cut::CutOpts {
                to: Some(lap_s),
                crf,
                ghost: if bare { None } else { Some(ghost.clone()) },
                offset_ms,
                nominal_ms: 0,
                bare,
            };
            let marker = clip::cut::run_opts(ff, &local_webm, &mp4, &o)?;
            let mp4_bytes = std::fs::metadata(&mp4).map(|m| m.len()).map_err(|e| e.to_string())?;
            if mp4_bytes > MP4_MAX_BYTES && tries < 4 {
                println!("mp4 is {:.1} MB at crf {crf} — over the 99 MB the inline player takes; re-encoding at crf {}", mp4_bytes as f64 / 1e6, crf + 3);
                crf += 3;
                continue;
            }
            if mp4_bytes > MP4_MAX_BYTES {
                return Err(format!("{}: {:.1} MB at crf {crf}, still over 99 MB", mp4.display(), mp4_bytes as f64 / 1e6));
            }
            overlay_col = match marker {
                Some(mk) => format!("controls overlay: {} (nominal 0, verified by frames on 02: the wheels turn at 1.950); crf {crf}, {:.1} MB", mk.summary(), mp4_bytes as f64 / 1e6),
                None => format!("NO OVERLAY (--no-overlay); crf {crf}, {:.1} MB", mp4_bytes as f64 / 1e6),
            };
            break;
        }
        if bare {
            println!("NO CONTROLS OVERLAY ON THIS CLIP (--no-overlay) — `clip ship` will refuse it");
        }
    }
    println!("mp4: {} [{overlay_col}]", mp4.display());

    // --- the mp4 to the box: beside the raw clip (where vjeux watches them), and
    // where the ship step runs
    let r_mp4 = format!("{box_videos}/{name}.mp4");
    let mp4_md5 = md5_of(&mp4)?;
    let have = wsx.sh(&format!("md5sum '{r_mp4}' 2>/dev/null | cut -c1-32")).unwrap_or_default().trim().to_string();
    if have == mp4_md5 {
        eprintln!("mp4 already on the box ({mp4_md5}) — not pushing");
    } else {
        eprintln!("pushing the mp4 ({} MB) to the box …", std::fs::metadata(&mp4).map(|m| m.len() / 1_000_000).unwrap_or(0));
        wsx.sh(&format!("mkdir -p '{box_videos}'"))?;
        wsx.push(&mp4, &r_mp4)?;
        let now = wsx.sh(&format!("md5sum '{r_mp4}' | cut -c1-32")).unwrap_or_default().trim().to_string();
        if now != mp4_md5 {
            return Err(format!("{r_mp4} on the box reads md5 {now}, pushed {mp4_md5}"));
        }
    }
    println!("box: {}", to_win(&r_mp4));

    // --- the store
    if let Some(dest) = f("--store") {
        let mut sent = Vec::new();
        let dest_dir = dest.split_once(':').map(|(_, d)| d.to_string()).unwrap_or_else(|| dest.clone());
        for p in [&local_webm, &mp4, &sheet] {
            if !p.is_file() {
                continue;
            }
            if let Some(w) = &from_webm {
                // the store copy IS the source: do not copy the webm onto itself
                let src_dir = w.parent().map(|d| d.display().to_string()).unwrap_or_default();
                if *p == local_webm && src_dir == dest_dir {
                    continue;
                }
            }
            copy_to_store(p, &dest)?;
            sent.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
        println!("store: {dest}/ ← {}", sent.join(" "));
    }

    // --- the ship, detached on the box; `tinyctl shipwatch` collects the URL
    if tmmaps::cli::has(args, "--ship") {
        let script = script_path()?;
        wsx.push(&script, BOX_SHIP_SH)?;
        let slug = map_slug(&nn);
        let done_file = format!("{VID}/ship/{name}.done");
        let _ = wsx.sh(&format!("mkdir -p {VID}/ship && rm -f '{done_file}' && chmod +x {BOX_SHIP_SH} && nohup sh {BOX_SHIP_SH} '{r_mp4}' '{slug}' '{VID}/ship/{name}' > /dev/null 2>&1 < /dev/null &"))?;
        let ships = out.join("ships.tsv");
        let mut text = std::fs::read_to_string(&ships).unwrap_or_default();
        if text.is_empty() {
            text.push_str("# nn\ttime\tname\tdone_file\tstatus\n");
        }
        text.push_str(&format!("{nn}\t{time}\t{name}\t{done_file}\tpending\n"));
        std::fs::write(&ships, text).map_err(|e| format!("{}: {e}", ships.display()))?;
        println!("ship: started on the box as {slug} — done file {done_file}; `tinyctl shipwatch --out {} --readme tiny/README.md` collects it", out.display());
    }

    println!();
    // the finish is the last "checkpoint" the decoder lists
    let row = format!("| {nn} | {} | {time} | {cps} cps | {overlay_col} | {name}.webm | {name}.mp4 | look at {} |", map_title(&nn), sheet.display());
    println!("{row}");
    let report = out.join("REPORT.md");
    let mut text = std::fs::read_to_string(&report).unwrap_or_default();
    if text.is_empty() {
        text.push_str("| map | title | time | cps | overlay | clip | mp4 | sheet |\n|---|---|---|---|---|---|---|---|\n");
    }
    text.push_str(&row);
    text.push('\n');
    std::fs::write(&report, text).map_err(|e| format!("{}: {e}", report.display()))?;
    Ok(Done { nn, ghost_md5: traj_id, time, cps, clip: format!("{name}.webm"), sheet })
}

/// `tools/tinyctl/box/tinyship.sh`, found from the binary's checkout (the
/// workspace layout) or `TINYSHIP_SH`.
fn script_path() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("TINYSHIP_SH") {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // tools/target/release/tinyctl → tools/tinyctl/box/tinyship.sh
    let mut d = exe.clone();
    for _ in 0..3 {
        d = d.parent().map(Path::to_path_buf).ok_or("no parent")?;
    }
    let p = d.join("tinyctl/box/tinyship.sh");
    if p.is_file() {
        return Ok(p);
    }
    Err(format!("{}: not found (set TINYSHIP_SH)", p.display()))
}

/// The page row of one map, rewritten: `**<title>** — original author time
/// `X` · tiny ghost **<time>** (<build note>)` and the asset URL on the line
/// after the blank; a *no lap yet* row gets its URL line inserted. The old row
/// is found by the map's title or, for 21–25, its former `Tiny Summer 2026 -
/// NN` name. The original author time is kept from the old line.
pub fn page_swap(text: &str, nn: &str, time: &str, build_note: &str, url: &str) -> Result<String, String> {
    let title = map_title(nn);
    let legacy = format!("**Tiny Summer 2026 - {nn}**");
    let lines: Vec<&str> = text.lines().collect();
    let i = lines
        .iter()
        .position(|l| l.starts_with(&format!("**{title}**")) || l.starts_with(&legacy))
        .ok_or_else(|| format!("the page has no row for {title} ({legacy})"))?;
    let old = lines[i];
    let orig = old
        .split_once("original author time `")
        .and_then(|(_, r)| r.split_once('`'))
        .map(|(t, _)| t.to_string())
        .ok_or_else(|| format!("{old:?}: no `original author time` on the row"))?;
    let new_line = format!("**{title}** — original author time `{orig}` · tiny ghost **{time}** ({build_note})");
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    out[i] = new_line;
    // the URL line: the next non-empty line if it is an asset, else insert
    let mut j = i + 1;
    while j < out.len() && out[j].trim().is_empty() {
        j += 1;
    }
    if j < out.len() && out[j].starts_with("https://github.com/user-attachments/assets/") {
        out[j] = url.to_string();
    } else {
        out.insert(i + 1, String::new());
        out.insert(i + 2, url.to_string());
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') {
        s.push('\n');
    }
    Ok(s)
}

/// `tinyctl shipwatch`: the ships `tinyctl video --ship` started, collected —
/// each done file read off the box; a `URL …` swaps the map's page row and,
/// with `--commit`, commits and pushes the page; a `FAILED …` is printed and
/// left pending so it is seen again. `--once` scans once; otherwise every 60 s.
pub fn shipwatch_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let out = PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/tinyvid".into()));
    let readme = f("--readme").map(PathBuf::from);
    let repo = f("--repo").map(PathBuf::from).or_else(|| readme.as_ref().and_then(|r| r.parent().and_then(|p| p.parent()).map(Path::to_path_buf)));
    let build_note = f("--build-note").unwrap_or_else(|| "build ship15, controls overlay".into());
    let commit = tmmaps::cli::has(args, "--commit");
    let wsx = Wsx::new(args);
    let ships = out.join("ships.tsv");
    loop {
        let text = std::fs::read_to_string(&ships).unwrap_or_default();
        let mut rows: Vec<String> = text.lines().map(String::from).collect();
        let mut changed = false;
        let mut pending = 0;
        for row in rows.iter_mut() {
            if row.starts_with('#') {
                continue;
            }
            let cells: Vec<String> = row.split('\t').map(String::from).collect();
            if cells.len() < 5 || cells[4] != "pending" {
                continue;
            }
            pending += 1;
            let (nn, time, name, done_file) = (&cells[0], &cells[1], &cells[2], &cells[3]);
            let Some(done) = wsx.cat(done_file) else { continue };
            let done = done.trim().to_string();
            if let Some(url) = done.strip_prefix("URL ") {
                let url = url.trim();
                println!("{} {nn} {time}: PUBLISHED {url}", chrono_now());
                if let Some(readme) = &readme {
                    let page = std::fs::read_to_string(readme).map_err(|e| format!("{}: {e}", readme.display()))?;
                    let new = page_swap(&page, nn, time, &build_note, url)?;
                    std::fs::write(readme, &new).map_err(|e| format!("{}: {e}", readme.display()))?;
                    println!("  page: row {} swapped in {}", map_title(nn), readme.display());
                    if commit {
                        let repo = repo.clone().ok_or("--commit needs --repo (or a --readme inside the repo)")?;
                        let msg = format!("tiny page: {} = {time} (build ship15) with the controls overlay ({name}.mp4)", map_title(nn));
                        git(&repo, &["add", &readme.strip_prefix(&repo).unwrap_or(readme).display().to_string()])?;
                        git(&repo, &["commit", "-q", "-m", &msg])?;
                        git(&repo, &["pull", "-q", "--rebase"])?;
                        git(&repo, &["push", "-q"])?;
                        println!("  pushed: {msg}");
                    }
                }
                *row = format!("{nn}\t{time}\t{name}\t{done_file}\t{url}");
                changed = true;
                pending -= 1;
            } else {
                println!("{} {nn} {time}: {done}", chrono_now());
            }
        }
        if changed {
            std::fs::write(&ships, rows.join("\n") + "\n").map_err(|e| format!("{}: {e}", ships.display()))?;
        }
        if tmmaps::cli::has(args, "--once") {
            println!("{pending} pending");
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

/// Where the start block puts the car relative to its Spawn placement: the
/// half-size block is 16 m square and the placement names a corner, so the
/// centre is (8, ·, 8) turned by the placement's yaw (measured on the 2026-09-08
/// ghosts: yaw 0 → (+8, +8), π/2 → (+8, −8), −π/2 → (−8, +8), π → (−8, −8)).
fn start_centre(yaw: f32) -> (f32, f32) {
    let (s, c) = yaw.sin_cos();
    (8.0 * c + 8.0 * s, -8.0 * s + 8.0 * c)
}

pub fn md5_of(p: &Path) -> Result<String, String> {
    let out = Command::new("md5sum").arg(p).output().map_err(|e| format!("md5sum: {e}"))?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.get(..32).map(String::from).ok_or_else(|| format!("md5sum {}: {}", p.display(), String::from_utf8_lossy(&out.stderr).trim()))
}

/// `host:dir` → scp; a plain directory → a local copy (the devserver has the
/// manifold mount, an OD does not).
fn copy_to_store(p: &Path, dest: &str) -> Result<(), String> {
    if dest.contains(':') {
        let (host, dir) = dest.split_once(':').unwrap();
        let st = Command::new("ssh").args(["-o", "BatchMode=yes", host, &format!("mkdir -p '{dir}'")]).status().map_err(|e| format!("ssh {host}: {e}"))?;
        if !st.success() {
            return Err(format!("ssh {host} mkdir -p {dir}: {st}"));
        }
        let st = Command::new("scp").args(["-q", "-o", "BatchMode=yes"]).arg(p).arg(format!("{host}:{dir}/")).status().map_err(|e| format!("scp: {e}"))?;
        if !st.success() {
            return Err(format!("scp {} {dest}: {st}", p.display()));
        }
    } else {
        std::fs::create_dir_all(dest).map_err(|e| format!("{dest}: {e}"))?;
        let to = Path::new(dest).join(p.file_name().ok_or("no file name")?);
        std::fs::copy(p, &to).map_err(|e| format!("{} → {}: {e}", p.display(), to.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four yaws the campaign's start blocks use, against the regenerated
    /// ghosts' sample 0 (2026-09-08): 01 yaw 0 (1576,·,776)→(1584,·,784);
    /// 06 yaw π/2 (1032,·,792)→(1040,·,784); 07 yaw −π/2 (1368,·,1032)→(1360,·,1040);
    /// 12 yaw π (1496,·,1336)→(1488,·,1328).
    #[test]
    fn the_start_centre_follows_the_placement_yaw() {
        let close = |(a, b): (f32, f32), (x, z): (f32, f32)| (a - x).abs() < 0.01 && (b - z).abs() < 0.01;
        assert!(close(start_centre(0.0), (8.0, 8.0)));
        assert!(close(start_centre(std::f32::consts::FRAC_PI_2), (8.0, -8.0)));
        assert!(close(start_centre(-std::f32::consts::FRAC_PI_2), (-8.0, 8.0)));
        assert!(close(start_centre(std::f32::consts::PI), (-8.0, -8.0)));
    }

    #[test]
    fn country_maps_go_by_their_names() {
        assert_eq!(map_title("07"), "Tiny Summer 2026 - 07");
        assert_eq!(map_title("23"), "Tiny Norway 2026");
        assert_eq!(map_slug("07"), "tiny-summer-2026-07");
        assert_eq!(map_slug("22"), "tiny-saudi-arabia-2026");
    }

    #[test]
    fn the_crf_ladder_keeps_a_lap_under_the_cap() {
        assert_eq!(crf_for(16.7), 20);
        assert_eq!(crf_for(39.3), 22);
        assert_eq!(crf_for(50.9), 24);
        assert_eq!(crf_for(115.2), 28);
        assert_eq!(crf_for(149.6), 30);
    }

    const PAGE: &str = "# Tiny\n\nintro\n\n**Tiny Summer 2026 - 01** — original author time `23.144` · tiny ghost **19.381** (build ship15)\n\nhttps://github.com/user-attachments/assets/aaaa\n\n**Tiny Summer 2026 - 18** — original author time `51.352` · *no lap yet*\n\n**Tiny Summer 2026 - 23** — original author time `75.112` · tiny ghost **115.244** (build ship15)\n\nhttps://github.com/user-attachments/assets/cccc\n\n";

    /// A row with a clip gets its time, note and URL swapped and keeps its
    /// original author time; a *no lap yet* row grows a URL line; 21–25 are
    /// renamed to their countries; nothing else moves.
    #[test]
    fn the_page_row_swap_keeps_the_author_time_and_renames_the_countries() {
        let s = page_swap(PAGE, "01", "19.100", "build ship15, controls overlay", "https://github.com/user-attachments/assets/bbbb").unwrap();
        assert!(s.contains("**Tiny Summer 2026 - 01** — original author time `23.144` · tiny ghost **19.100** (build ship15, controls overlay)\n\nhttps://github.com/user-attachments/assets/bbbb\n"), "{s}");
        assert!(!s.contains("aaaa"));
        assert!(s.contains("cccc"), "the other rows stay");
        let s = page_swap(&s, "23", "110.000", "build ship15, controls overlay", "https://github.com/user-attachments/assets/dddd").unwrap();
        assert!(s.contains("**Tiny Norway 2026** — original author time `75.112` · tiny ghost **110.000** (build ship15, controls overlay)\n\nhttps://github.com/user-attachments/assets/dddd\n"), "{s}");
        assert!(!s.contains("Tiny Summer 2026 - 23"));
        // and again by the new name
        let s2 = page_swap(&s, "23", "109.000", "build ship15, controls overlay", "https://github.com/user-attachments/assets/eeee").unwrap();
        assert!(s2.contains("**110.000**") == false && s2.contains("eeee"));
        let s = page_swap(&s, "18", "40.000", "build ship15, controls overlay", "https://github.com/user-attachments/assets/ffff").unwrap();
        assert!(s.contains("**Tiny Summer 2026 - 18** — original author time `51.352` · tiny ghost **40.000** (build ship15, controls overlay)\n\nhttps://github.com/user-attachments/assets/ffff\n\n**Tiny Norway 2026**"), "{s}");
        assert!(s.ends_with("\n\n"), "the trailing newlines are kept");
        assert!(page_swap(PAGE, "09", "1.000", "x", "u").is_err(), "a map the page lacks is an error");
    }
}

/// What a clip depends on: the samples the render PLAYS and the race time.
/// A ghost re-written with new metadata (the INPUT arm refreshed all eleven
/// files at 18:55Z with a different zone string and identical trajectories)
/// must not cost eleven re-renders, so the state file keys on this, not on the
/// file's md5. FNV-1a over the little-endian bytes of (race time, every
/// sample's time and position), as 16 hex digits.
pub fn trajectory_id(g: &gbx::record::Decoded) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(&g.race_time_ms.unwrap_or(0).to_le_bytes());
    for s in &g.samples {
        feed(&s.time_ms.to_le_bytes());
        feed(&s.x.to_le_bytes());
        feed(&s.y.to_le_bytes());
        feed(&s.z.to_le_bytes());
    }
    format!("{h:016x}")
}

/// The ghost with every 27-character uid literal in its body replaced by `uid`
/// (same length, nothing else moves), written with an uncompressed body. The
/// number of literals rewritten is returned; zero is an error — a ghost always
/// declares its map.
fn write_ghost_with_uid(src: &Path, out: &Path, uid: &str) -> Result<usize, String> {
    if uid.len() != 27 || !uid.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(format!("render uid {uid:?}: a map uid is 27 ASCII chars of [A-Za-z0-9_-]"));
    }
    let data = std::fs::read(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let g = gbx::Gbx::parse(&data);
    let mut body = g.body.clone();
    let mut n = 0usize;
    let mut i = 0usize;
    while i + 31 <= body.len() {
        if u32::from_le_bytes(body[i..i + 4].try_into().unwrap()) == 27 {
            if let Ok(s) = std::str::from_utf8(&body[i + 4..i + 31]) {
                if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                    body[i + 4..i + 31].copy_from_slice(uid.as_bytes());
                    n += 1;
                    i += 31;
                    continue;
                }
            }
        }
        i += 1;
    }
    if n == 0 {
        return Err(format!("{}: no map uid literal in the ghost body", src.display()));
    }
    let mut file = g.header_bytes_u();
    file.extend_from_slice(&body);
    std::fs::write(out, &file).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(n)
}
