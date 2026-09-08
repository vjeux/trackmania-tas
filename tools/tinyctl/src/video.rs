//! `tinyctl video` — one map's driven lap as a video and a contact sheet, from
//! the devserver, through the render box.
//!
//! ```text
//! tinyctl video --map NN [--ghost F] [--out /tmp/tinyvid] [--maps-dir /tmp/audit/ship9]
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
//!    centre, (8, ·, 8) turned by the placement's yaw — within a metre on the
//!    ground and inside a metre and a half in height. `--no-guard` renders
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

pub fn cmd(args: &[String]) -> Result<(), String> {
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
    let dh = ((s0.x - want[0]).powi(2) + (s0.z - want[2]).powi(2)).sqrt();
    let dy = s0.y - want[1];
    println!(
        "guard: sample 0 at [{:.2}, {:.2}, {:.2}]; Spawn placement [{:.2}, {:.2}, {:.2}] yaw {:.4} → start centre [{:.2}, ·, {:.2}]: {dh:.2} m off on the ground, {dy:+.2} m in height",
        s0.x, s0.y, s0.z, spawn.pos[0], spawn.pos[1], spawn.pos[2], spawn.yaw, want[0], want[2]
    );
    let on_start = dh <= 1.0 && (-0.5..=1.5).contains(&dy);
    if !on_start {
        if tmmaps::cli::has(args, "--no-guard") {
            println!("GUARD OVERRIDDEN (--no-guard): THIS GHOST'S SAMPLES DO NOT START ON THIS MAP'S START LINE — the clip shows the samples, not this map's physics");
        } else {
            return Err(format!(
                "REFUSED: the ghost's first sample is {dh:.1} m / {dy:+.1} m from where this map starts the car. A ghost = inputs + samples, and a render plays the SAMPLES; \
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
    let rg = wsx.sh(&format!("md5sum '{r_ghost}' | cut -c1-32")).unwrap_or_default().trim().to_string();
    if rg != ghost_md5 {
        return Err(format!("{r_ghost} on the box reads md5 {rg}, pushed {ghost_md5}"));
    }

    // --- render there, poll here
    let r_dir = format!("{VID}/{tag}");
    let cmd = format!("{shootctl} render --detach --map {r_map} --name {tag} --outdir {r_dir} --cam {cam} --load-timeout {load_timeout} {r_ghost}");
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

    // --- the clip where vjeux watches them, on the box
    let name = format!("{nn}-ghost-{time}");
    let r_keep = format!("{box_videos}/{name}.webm");
    let cp = wsx.sh(&format!("mkdir -p '{box_videos}' && cp -f '{webm}' '{r_keep}' && stat -c %s '{r_keep}'"))?;
    let kept: u64 = cp.trim().parse().unwrap_or(0);
    if kept != bytes {
        return Err(format!("{r_keep}: {kept} bytes after the copy, the clip is {bytes}"));
    }
    println!("box: {} ({kept} bytes)", to_win(&r_keep));

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

fn md5_of(p: &Path) -> Result<String, String> {
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
