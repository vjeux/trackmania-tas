//! `tinyctl video` — one map's driven lap as a video and a contact sheet, from
//! the devserver, through the render box.
//!
//! ```text
//! tinyctl video --map NN | --all [--ghost F] [--out /tmp/tinyvid] [--maps-dir /tmp/audit/ship9]
//!               [--ghosts-dir /tmp/ghosts] [--cam 2] [--load-timeout 120] [--no-guard]
//!               [--box-videos "…/Maps/Tiny/videos"] [--store host:dir | dir] [--pull-webm]
//!               [--box-shootctl P] [--wsx P] [-v]
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
//! 4. the clip is copied on the box to `Maps\Tiny\videos\NN-ghost-<time>.webm`
//!    (where vjeux watches them), the sheets are pulled into `--out`, and with
//!    `--store` the clip + sheet are copied to the shared store as well
//!    (`host:dir` goes through scp — an OD has no manifold mount).
//!
//! The last line printed is the REPORT.md skeleton for the map: time and
//! checkpoints from the ghost, the sheet to look at, the rest is yours.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::wsx::{to_win, Wsx};

const STAGE: &str = "/home/vjeux/shoot/_stage";
const VID: &str = "/mnt/c/Users/vjeux/tinyvid";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const BOX_VIDEOS: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/Tiny/videos";

/// One rendered lap, as the state file records it.
struct Done {
    nn: String,
    ghost_md5: String,
    time: String,
    cps: usize,
    clip: String,
    sheet: PathBuf,
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    if tmmaps::cli::has(args, "--all") {
        return all(args);
    }
    one(args).map(|_| ())
}

/// `--all`: every `NN.Ghost.Gbx` in `--ghosts-dir` whose md5 is not yet in
/// `<out>/videos.tsv` (nn, ghost md5, time, cps, clip, sheet, when), rendered in
/// turn — the loop of a day when laps land every half hour. A ghost REPLACED
/// under the same name (a better lap, a regenerated file) has a new md5 and is
/// rendered again; a failure is printed and the loop goes on to the next map.
fn all(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let ghosts_dir = PathBuf::from(f("--ghosts-dir").unwrap_or_else(|| "/tmp/ghosts".into()));
    let out = PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/tinyvid".into()));
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
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
    let mut todo = Vec::new();
    for n in &names {
        let nn = n[..2].to_string();
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
                "--map" | "--ghost" | "--map-file" => skip = true,
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
            Ok(d) => {
                let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let row = format!("{}\t{}\t{}\t{}\t{}\t{}\t{when}\n", d.nn, d.ghost_md5, d.time, d.cps, d.clip, d.sheet.display());
                let header = if state.exists() { String::new() } else { "# nn\ttrajectory_id\ttime\tcps\tclip\tsheet\tunix\n".to_string() };
                let mut text = std::fs::read_to_string(&state).unwrap_or_default();
                text.push_str(&header);
                text.push_str(&row);
                std::fs::write(&state, text).map_err(|e| format!("{}: {e}", state.display()))?;
            }
            Err(e) => {
                eprintln!("{nn}: FAILED — {e}");
                failed.push(nn.clone());
            }
        }
    }
    println!("\n{} rendered, {} failed{}", todo.len() - failed.len(), failed.len(), if failed.is_empty() { String::new() } else { format!(" ({})", failed.join(" ")) });
    if failed.is_empty() { Ok(()) } else { Err("some renders failed (above)".into()) }
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
    for p in [&map, &ghost] {
        if !p.is_file() {
            return Err(format!("{}: no such file", p.display()));
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;

    // --- the ghost: time, checkpoints, and THE GUARD
    let g = gbx::record::decode_ghost(ghost.to_str().ok_or("ghost path is not utf-8")?)?;
    let race_ms = g.race_time_ms.or_else(|| g.samples.last().map(|s| s.time_ms)).ok_or("the ghost has no race time and no samples")?;
    let time = format!("{}.{:03}", race_ms / 1000, race_ms % 1000);
    let s0 = g.samples.first().ok_or("the ghost has no samples — nothing to render")?;
    println!("{}: {} samples, race time {time}, checkpoints at {}", ghost.display(), g.samples.len(), g.checkpoints_ms.iter().map(|c| format!("{:.3}", *c as f64 / 1000.0)).collect::<Vec<_>>().join(" "));
    let m = tmmaps::map::MapFile::load(&map);
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

    // --- push: the ghost always, the map only when the box does not have these bytes
    let wsx = Wsx::new(args);
    let tag = format!("vid{nn}");
    let r_map = format!("{STAGE}/{tag}.Map.Gbx");
    let r_ghost = format!("{STAGE}/{tag}.Ghost.Gbx");
    let want_md5 = md5_of(&map)?;
    let have = wsx.sh(&format!("md5sum '{r_map}' 2>/dev/null | cut -c1-32")).unwrap_or_default().trim().to_string();
    if have == want_md5 {
        eprintln!("map already on the box ({want_md5}) — not pushing");
    } else {
        eprintln!("pushing {} ({} MB) …", map.display(), std::fs::metadata(&map).map(|m| m.len() / 1_000_000).unwrap_or(0));
        wsx.push(&map, &r_map)?;
        let now = wsx.sh(&format!("md5sum '{r_map}' | cut -c1-32")).unwrap_or_default().trim().to_string();
        if now != want_md5 {
            return Err(format!("{r_map} on the box reads md5 {now}, pushed {want_md5}"));
        }
    }
    wsx.push(&ghost, &r_ghost)?;
    let ghost_md5 = md5_of(&ghost)?;
    let traj_id = trajectory_id(&g);
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
    let bytes: u64 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let secs: f64 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    println!("clip: {webm} ({bytes} bytes, {secs:.3} s; the lap is {time})");
    if (secs - race_ms as f64 / 1000.0).abs() > 1.0 {
        println!("NOTE: the clip is {secs:.3} s for a {time} lap — the render did not cover the lap (a camera that never attached plays the whole clip static)");
    }

    // --- the clip where vjeux watches them, on the box (MOVED, not copied:
    // one 30 MB copy per lap on a nearly full C:)
    // --suffix S: `NN-ghost-<time>-S.webm` — the set the clip was rendered on,
    // when one videos folder holds more than one set (ship9 and ship10 clips of
    // the same lap side by side)
    let name = match f("--suffix") {
        Some(s) => format!("{nn}-ghost-{time}-{s}"),
        None => format!("{nn}-ghost-{time}"),
    };
    let r_keep = format!("{box_videos}/{name}.webm");
    let mv = wsx.sh(&format!("mkdir -p '{box_videos}' && mv -f '{webm}' '{r_keep}' && stat -c %s '{r_keep}'"))?;
    let kept: u64 = mv.trim().parse().unwrap_or(0);
    if kept != bytes {
        return Err(format!("{r_keep}: {kept} bytes after the move, the clip was {bytes}"));
    }
    println!("box: {} ({kept} bytes)", to_win(&r_keep));
    let webm = r_keep;

    // --- pull: sheets always, the clip on request or for the store
    let sheet = out.join(format!("{name}-sheet.png"));
    let dense = out.join(format!("{name}-dense.png"));
    let n = wsx.pull(&format!("{r_dir}/{tag}-sheet.png"), &sheet)?;
    println!("sheet: {} ({n} B)", sheet.display());
    match wsx.pull(&format!("{r_dir}/{tag}-dense.png"), &dense) {
        Ok(n) => println!("dense: {} ({n} B)", dense.display()),
        Err(e) => eprintln!("dense sheet: {e}"),
    }
    let store = f("--store");
    let local_webm = out.join(format!("{name}.webm"));
    if store.is_some() || tmmaps::cli::has(args, "--pull-webm") {
        eprintln!("pulling the clip ({} MB) …", bytes / 1_000_000);
        let n = wsx.pull(&webm, &local_webm)?;
        if n != bytes {
            return Err(format!("{}: pulled {n} bytes, the clip is {bytes}", local_webm.display()));
        }
        println!("clip: {} ({n} B)", local_webm.display());
    }
    if let Some(dest) = store {
        for p in [&local_webm, &sheet] {
            copy_to_store(p, &dest)?;
        }
        println!("store: {dest}/{name}.webm + {name}-sheet.png");
    }
    println!();
    // the finish is the last "checkpoint" the decoder lists
    let cps = g.checkpoints_ms.iter().filter(|c| **c < race_ms - 50).count();
    println!("| {nn} | {time} | {cps} cps | on-road: ? | look at {} |", sheet.display());
    Ok(Done { nn, ghost_md5: traj_id, time, cps, clip: format!("{name}.webm"), sheet })
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
    use super::start_centre;

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
