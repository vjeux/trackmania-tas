//! `tinyctl video` — one map's driven lap as a video and a contact sheet, from
//! the devserver, through the render box — **with the run's controls drawn on
//! it, by default, checked, and stamped.**
//!
//! ```text
//! tinyctl video --map NN | --all [--watch SECS [--idle-quit-min 30 (0 = never)]] [--min-gain-s 0.1] [--ghost-archive DIR|none] [--ghost F] [--out /tmp/tinyvid] [--maps-dir /tmp/audit/ship9]
//!               [--ghosts-dir /tmp/ghosts] [--ghosts-sync host:dir] [--build ship15] [--cam 2] [--load-timeout 120] [--no-guard]
//!               [--box-videos "…/Maps/Tiny/videos"] [--store host:dir | dir] [--pull-webm] [--suffix S]
//!               [--from-webm F] [--no-overlay] [--crf N] [--offset-ms N] [--ship [--readme tiny/README.md]]
//!               [--box-shootctl P] [--wsx P] [-v]
//! tinyctl shipwatch --out /tmp/tinyvid --readme tiny/README.md [--repo DIR] [--once] [--commit] [--min-gain-s 0.1] [--wsx P]
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
use std::time::{Duration, Instant};

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

/// The ghosts README's LAST row about map `nn` and lap `time` (the INPUT arm
/// appends rows in more than one shape; a row is one that names both), or
/// `None` while the README has no row for that lap yet.
pub fn readme_row(text: &str, nn: &str, time: &str) -> Option<String> {
    let key = format!("| {nn} |");
    let rows: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with(&key) || l.contains(&format!(" {key}")) || l.starts_with(&format!("|{nn}|")))
        .filter(|l| l.contains(time))
        .collect();
    // The README's second table (map | file md5 | time) names the map and the
    // lap too, since 2026-09-10 06:50Z — and says nothing about the build, so
    // taking it as "the row" skipped 25 119.115 as "not ship15". A row that
    // names the ghost FILE is the one that carries the build; it wins whenever
    // there is one.
    rows.iter()
        .rev()
        .find(|l| l.contains(".Ghost.Gbx"))
        .or_else(|| rows.last())
        .map(|l| l.to_string())
}

/// How the page names the driver of this lap: a TAS lap is `tiny ghost`; a
/// lap that is vjeux's own playtest run promoted as the map's best says so
/// (coordinator, 2026-09-09 15:43Z). Read off the README row for (nn, time);
/// no row → TAS.
pub fn lap_label(readme: &str, nn: &str, time: &str) -> String {
    match readme_row(readme, nn, time) {
        Some(row) if row.to_ascii_lowercase().contains("playtest") => "driven by vjeux (playtest)".to_string(),
        _ => "tiny ghost".to_string(),
    }
}

/// The ghosts README's CURRENT lap for map `nn`, as (time, build): the first
/// table row `| nn | nn.Ghost.Gbx | time | credits | build | …`. `None` when
/// the README has no such row (or a row of another shape) — then nothing is
/// superseded on its word.
pub fn readme_current_lap(readme: &str, nn: &str) -> Option<(String, String)> {
    let key = format!("| {nn} |");
    readme.lines().filter(|l| l.starts_with(&key)).find_map(|l| {
        let c: Vec<&str> = l.split('|').map(str::trim).collect();
        // ["", nn, file, time, credits, build, …]
        if c.len() < 6 || c[2] != format!("{nn}.Ghost.Gbx") {
            return None;
        }
        let time = c[3];
        if time.is_empty() || !time.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
            return None;
        }
        Some((time.to_string(), c[5].to_string()))
    })
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
    // THE GAME IS A PROCESS ON A PHYSICAL BOX SOMEBODY SITS NEXT TO. A render
    // leaves it up (the next lap is usually minutes away and a launch costs a
    // minute), but a night with no new lap would leave it burning for hours
    // (vjeux, 2026-08-24: the fans). After `--idle-quit-min` minutes (default
    // 30) without a render, one taskkill closes it; the next render relaunches.
    let idle_quit = Duration::from_secs(f("--idle-quit-min").and_then(|s| s.parse::<u64>().ok()).unwrap_or(30) * 60);
    // armed from the start: a loop restarted while the game is up (a tool
    // upgrade between renders, 2026-09-10) must close it too, not only a loop
    // that rendered something itself
    let mut last_render: Option<Instant> = Some(Instant::now());
    let mut game_up = true;
    loop {
        let before = std::fs::metadata(PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/tinyvid".into())).join("videos.tsv")).and_then(|m| m.modified()).ok();
        let r = all_once(args);
        let after = std::fs::metadata(PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/tinyvid".into())).join("videos.tsv")).and_then(|m| m.modified()).ok();
        if before != after {
            last_render = Some(Instant::now());
            game_up = true;
        }
        match (&r, watch) {
            (_, None) => return r,
            (Err(e), Some(_)) => eprintln!("scan: {e}"),
            (Ok(()), Some(_)) => {}
        }
        // `--idle-quit-min 0` keeps the game warm (the burst: a launch costs 1–2 min per clip)
        if !idle_quit.is_zero() && game_up && last_render.map(|t| t.elapsed() >= idle_quit).unwrap_or(false) {
            // UNDER THE RENDER LOCK. Other threads drive the same game through
            // `shootctl` (a shootset and a render were running at 12:02Z on
            // 2026-09-10 when a bare taskkill went out); the lock is what says
            // whether anyone is. `--wait 0`: if it is held, the game is in use
            // and nothing happens; if it is free, we hold it for the second
            // the kill takes, so nobody can start a load underneath it.
            let wsx = Wsx::new(args);
            let shootctl = f("--box-shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
            let cmd = format!(
                "sh -c 'if {shootctl} lock acquire --owner tinyctl-idle-quit --wait 0 2>/dev/null; then \
                   timeout 30 /mnt/c/Windows/System32/taskkill.exe /IM Trackmania.exe /F 2>&1 | tr -d \"\\r\"; \
                   {shootctl} lock release --owner tinyctl-idle-quit 2>/dev/null; echo CLOSED; \
                 else echo BUSY; fi'"
            );
            match wsx.sh(&cmd) {
                Ok(o) if o.contains("BUSY") => eprintln!("[idle {} min] the render lock is held — the game is in use by another thread; not closed", idle_quit.as_secs() / 60),
                Ok(o) => {
                    eprintln!("[idle {} min] closed the game on the box: {}", idle_quit.as_secs() / 60, o.replace("CLOSED", "").trim());
                    game_up = false;
                }
                Err(e) => eprintln!("[idle] could not close the game: {e}"),
            }
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
        rsync_dir(&src, &ghosts_dir, &[])?;
    }
    // --webm-sync host:dir: the store's renders (webm + sheet) into --from-webm-dir
    // before the scan, so a lap another thread rendered is cut, not rendered twice
    if let (Some(src), Some(dir)) = (f("--webm-sync"), f("--from-webm-dir")) {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        rsync_dir(&src, &dir, &["--include=*.webm", "--include=*-sheet.png", "--exclude=*"])?;
    }
    let state = out.join("videos.tsv");
    // (map, trajectory id, clip name): a ghost counts as rendered only on the
    // build its clip was made for — the clip name ends in `-<build>.webm`, so a
    // per-map build change (builds.tsv) renders the same ghost again
    let seen: Vec<(String, String, String)> = std::fs::read_to_string(&state)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            Some((c.first()?.to_string(), c.get(1)?.to_string(), c.get(4).copied().unwrap_or("").to_string()))
        })
        .collect();
    let mut names: Vec<String> = std::fs::read_dir(&ghosts_dir)
        .map_err(|e| format!("{}: {e}", ghosts_dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .filter(|n| n.len() == 12 && n.ends_with(".Ghost.Gbx") && n[..2].chars().all(|c| c.is_ascii_digit()))
        .collect();
    names.sort();
    let global_build = f("--build");
    // PER-MAP BUILDS (the burst, coordinator 2026-09-10 22:10Z): `<out>/builds.tsv`
    // (`nn<TAB>build<TAB>map_path`) names the build a map is rendered on and the
    // map file of that build; every other map keeps --build / --maps-dir. The
    // README-row filter, the gate's build comparison, the archive sidecar and
    // the per-map call (`--build`, `--map-file`, `--suffix`) all follow it, so
    // 05 and 15 can render on ship16/ship17 while the rest stays ship15.
    let builds = read_builds(&out);
    let readme = std::fs::read_to_string(ghosts_dir.join("README.md")).unwrap_or_default();
    // THE RE-RENDER THRESHOLD (a tool default — vjeux's "not a rule, a
    // default"; coordinator 2026-09-10 11:34Z). The player project shaves
    // 1-ms slivers off a lap every ten minutes once it converges (15: 48.753 →
    // 48.748 → 48.747 → 48.738 in an hour), and each sliver cost a 5–15 min
    // render and an upload slot on the one GitHub session. A map is rendered
    // again only when its newest certified lap improves on the PUBLISHED clip's
    // lap by `--min-gain-s` (default 0.1), or it is the map's first lap, or the
    // build changed. A skipped sliver is not lost: `tinyctl page-status` keeps
    // the "latest lap X — video pending" note under the row, and the gain is
    // measured against the published clip, so it accumulates and the render
    // happens when it crosses the threshold. `--min-gain-s 0` is the opt-out
    // (every new ghost renders, as before).
    let min_gain: f64 = f("--min-gain-s").map(|s| s.parse().map_err(|_| "--min-gain-s wants seconds")).transpose()?.unwrap_or(0.1);
    let published = published_laps(&std::fs::read_to_string(out.join("ships.tsv")).unwrap_or_default());
    // PUBLISH HOLDS (coordinator, 2026-09-10 17:55Z: "the opening is bad; stop
    // optimizing the end" — no new Argentina lap goes up until lifted). A map
    // listed in `<out>/holds.tsv` (`nn<TAB>reason`) is not rendered and not
    // shipped, whatever its gain; its new ghost IS archived (the bytes are the
    // record), and page-status writes "held (<reason>)" under the row. Lift =
    // delete the line. Read every scan, so a hold takes effect on the next tick.
    let holds = read_holds(&out);
    // a `render`-mode hold renders and stages (the ship step records `held`) —
    // only a `none`-mode hold stops at the archive
    let render_holds = read_render_holds(&out);
    let skips_path = out.join("skips.tsv");
    let mut skips = std::fs::read_to_string(&skips_path).unwrap_or_default();
    let mut todo = Vec::new();
    for n in &names {
        let nn = n[..2].to_string();
        let path = ghosts_dir.join(n);
        let g = gbx::record::decode_ghost(path.to_str().ok_or("ghost path is not utf-8")?)?;
        let md5 = trajectory_id(&g);
        let build = builds.get(&nn).map(|(b, _)| b.clone()).or_else(|| global_build.clone());
        let rendered_on_this_build = |clip: &str| match &build {
            Some(b) => clip.ends_with(&format!("-{b}.webm")) || clip.is_empty(),
            None => true,
        };
        if seen.iter().any(|(a, b, clip)| *a == nn && *b == md5 && rendered_on_this_build(clip)) {
            continue;
        }
        let race_ms = g.race_time_ms.or_else(|| g.samples.last().map(|s| s.time_ms)).unwrap_or(0);
        let time = format!("{}.{:03}", race_ms / 1000, race_ms % 1000);
        // THE FILE AND THE README MUST AGREE (coordinator, 2026-09-10 16:40Z).
        // The label of every clip comes from the FILE's declared race time (so
        // a caption can never carry another lap's time), but the README is what
        // says which build a lap was certified on — and the alias
        // `NN.Ghost.Gbx` is rewritten by the input arm in two steps, README
        // first (24: row 99.529 while the alias still held the 100.116 bytes).
        // A file whose lap the README does not name — neither in the map's
        // current row nor, by this file's md5, in the md5 table — is in
        // transition: skipped this scan, picked up when the two agree.
        let file_md5 = md5_of(&path)?;
        if !readme.is_empty() && !readme_names_lap(&readme, &nn, &time, &file_md5) {
            let current = readme_current_lap(&readme, &nn).map(|(t, _)| t).unwrap_or_else(|| "?".into());
            println!("{nn} {time}: the ghost FILE ({}) holds lap {time} but the README names {current} for this map and no row for {time} — in transition, skipped this scan", &file_md5[..8]);
            continue;
        }
        // --build B: only a lap whose README row names B (the FILE says which
        // lap: its race time; the README says which build it was regenerated on)
        if let Some(b) = &build {
            match readme_row(&readme, &nn, &time) {
                Some(row) if row.contains(b.as_str()) => {}
                Some(row) => {
                    println!("{nn} {time}: README row is not {b} — skipped ({})", row.chars().take(120).collect::<String>());
                    continue;
                }
                None => {
                    // the md5 table names the lap (above) but no row carries the
                    // build: the pass runs on the installed build, so render and
                    // say the row was missing
                    println!("{nn} {time}: no README row with a build for this lap yet — rendering on the installed build's word ({b}); the label is read at swap time");
                }
            }
        }
        match render_gate(race_ms as f64 / 1000.0, published.get(&nn).map(|(t, name)| (*t, name.as_str())), build.as_deref(), min_gain) {
            Gate::Render(_) | Gate::Skip { .. } if holds.contains_key(&nn) && !render_holds.contains(&nn) => {
                // HELD: archive the bytes (write-once) so the lap is kept, render
                // nothing. Said once per ghost, like a skip.
                let key = format!("{nn}\t{md5}\t");
                if !skips.contains(&key) {
                    let reason = &holds[&nn];
                    println!("{nn} {time}: HELD — {reason} (holds.tsv); archived, not rendered");
                    if let Some(dir) = f("--ghost-archive").filter(|d| d != "none").map(PathBuf::from).or_else(|| Path::new(GHOST_ARCHIVE_DEFAULT).is_dir().then(|| PathBuf::from(GHOST_ARCHIVE_DEFAULT))) {
                        let map = PathBuf::from(f("--maps-dir").unwrap_or_else(|| "/tmp/audit/ship9".into())).join(format!("Tiny Summer 2026 - {nn}.Map.Gbx"));
                        let map_md5 = md5_of(&map).unwrap_or_default();
                        let row = readme_row(&readme, &nn, &time).unwrap_or_default();
                        if let Err(e) = archive_ghost(&dir, &path, &md5_of(&path)?, &nn, &time, race_ms, &md5, &map_md5, build.as_deref(), &row) {
                            eprintln!("{nn} {time}: could not archive the held ghost: {e}");
                        }
                    }
                    let cps = g.checkpoints_ms.iter().filter(|c| **c < race_ms - 50).count();
                    append_report_row(&out, &format!("| {nn} | {} | {time} | {cps} cps | HELD ({reason}); archived, not rendered | — | — | — |", map_title(&nn)))?;
                    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    if skips.is_empty() {
                        skips.push_str("# nn\ttrajectory_id\ttime\tgain\tpublished\tunix\n");
                    }
                    skips.push_str(&format!("{key}{time}\theld\t-\t{when}\n"));
                    std::fs::write(&skips_path, &skips).map_err(|e| format!("{}: {e}", skips_path.display()))?;
                }
                continue;
            }
            Gate::Render(why) => {
                if published.contains_key(&nn) {
                    println!("{nn} {time}: rendering — {why}");
                }
            }
            Gate::Skip { gain, published: p } => {
                // recorded ONCE per ghost (the scan runs every two minutes): a
                // REPORT row and a skips.tsv line; re-evaluated every scan, so the
                // opt-out or a changed reference renders it after all
                let key = format!("{nn}\t{md5}\t");
                if !skips.contains(&key) {
                    let cps = g.checkpoints_ms.iter().filter(|c| **c < race_ms - 50).count();
                    let row = format!("| {nn} | {} | {time} | {cps} cps | skipped (gain {gain:.3} < {min_gain:.3} over the published {p:.3}; page-status keeps the pending note) | — | — | — |", map_title(&nn));
                    println!("{nn} {time}: skipped — gain {gain:.3} s over the published {p:.3} is under {min_gain:.3} (--min-gain-s); the page keeps the pending note and the render happens when the gain reaches the threshold");
                    append_report_row(&out, &row)?;
                    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    if skips.is_empty() {
                        skips.push_str("# nn\ttrajectory_id\ttime\tgain\tpublished\tunix\n");
                    }
                    skips.push_str(&format!("{key}{time}\t{gain:.3}\t{p:.3}\t{when}\n"));
                    std::fs::write(&skips_path, &skips).map_err(|e| format!("{}: {e}", skips_path.display()))?;
                }
                continue;
            }
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
                "--map" | "--ghost" | "--map-file" | "--watch" | "--ghosts-sync" | "--webm-sync" | "--build" | "--suffix" | "--from-webm" => skip = true,
                _ => v.push(a.clone()),
            }
        }
        // the per-map call learns where the FRESH ghosts live, so a clip whose
        // lap was replaced during its own render is not uploaded (see `one`)
        if let Some(src) = f("--ghosts-sync") {
            v.push("--ghosts-src".into());
            v.push(src);
        }
        v
    };
    let mut failed = Vec::new();
    for nn in &todo {
        let mut a = base.clone();
        a.push("--map".into());
        a.push(nn.clone());
        // the map's build: builds.tsv (build + map file) or the global --build;
        // the clip suffix follows the build so ship15 and ship16 clips of one lap
        // have different names
        let (b, map_file) = match builds.get(nn) {
            Some((b, p)) => (Some(b.clone()), Some(p.clone())),
            None => (global_build.clone(), None),
        };
        if let Some(b) = &b {
            a.push("--build".into());
            a.push(b.clone());
            a.push("--suffix".into());
            a.push(f("--suffix").filter(|_| builds.get(nn).is_none()).unwrap_or_else(|| b.clone()));
        } else if let Some(s) = f("--suffix") {
            a.push("--suffix".into());
            a.push(s);
        }
        if let Some(p) = map_file {
            a.push("--map-file".into());
            a.push(p);
        }
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
    // --from-webm F: this render; --from-webm-dir D: the render of this lap if D
    // holds it (`<name>.webm`), else a fresh render — how a day loop re-cuts
    // the clips that exist and renders the ones that do not
    let mut from_webm = f("--from-webm").map(PathBuf::from);
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
    // the cut (overlay or bare) needs ffmpeg HERE; find it before the box does any work
    let ff = Some(clip::platform::from_env().map_err(|e| format!("the cut needs ffmpeg on this side: {e}"))?);

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
    if from_webm.is_none() {
        if let Some(d) = f("--from-webm-dir") {
            let p = PathBuf::from(d).join(format!("{name}.webm"));
            if p.is_file() {
                from_webm = Some(p);
            } else {
                println!("from-webm-dir: no {} — rendering", p.display());
            }
        }
    }
    let wsx = Wsx::new(args);
    let traj_id = trajectory_id(&g);
    let cps = g.checkpoints_ms.iter().filter(|c| **c < race_ms - 50).count();
    let sheet = out.join(format!("{name}-sheet.png"));
    let local_webm = out.join(format!("{name}.webm"));
    let mut bytes: u64;

    // --- THE GHOST ARCHIVE, before anything is rendered (coordinator, 2026-09-10
    // 16:17Z, for the parent project). `ghosts-for-video/NN.Ghost.Gbx` is a
    // MUTABLE alias the input arm overwrites several times a day, so the bytes a
    // clip was rendered from were gone within the hour and nothing could say
    // which tape a published video shows. Now the input ghost is copied,
    // write-once, to `<archive>/<md5>.Ghost.Gbx` with a sidecar `<md5>.json`
    // (map, build, lap, README row, FNV, map md5, when), and the REPORT row
    // names the archive file. `--ghost-archive DIR` (default: the store's
    // tm-player/tiny/ghost-archive when that mount exists; `--ghost-archive
    // none` turns it off). A failure to archive is a failure to render: the
    // archive is what makes the render accountable.
    let ghost_md5 = md5_of(&ghost)?;
    let archive_name = match f("--ghost-archive").as_deref() {
        Some("none") => None,
        given => {
            let dir = given.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(GHOST_ARCHIVE_DEFAULT));
            if dir.is_dir() || given.is_some() {
                let readme_row = readme_row(&std::fs::read_to_string(ghosts_dir.join("README.md")).unwrap_or_default(), &nn, &time).unwrap_or_default();
                let map_md5 = md5_of(&map)?;
                archive_ghost(&dir, &ghost, &ghost_md5, &nn, &time, race_ms, &traj_id, &map_md5, f("--build").as_deref(), &readme_row)?;
                Some(format!("{ghost_md5}.Ghost.Gbx"))
            } else {
                eprintln!("ghost archive: {} is not there (store not mounted?) — NOT archiving; pass --ghost-archive DIR", dir.display());
                None
            }
        }
    };
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
    // the archive name rides the overlay column into the REPORT row and the
    // mp4's sidecar, so a clip says which archived tape it shows
    let overlay_col = match &archive_name {
        Some(a) => format!("{overlay_col}; ghost archive {a}"),
        None => overlay_col,
    };
    if let Some(a) = &archive_name {
        let side = mp4.with_extension("mp4.json");
        let json = format!("{{\n  \"mp4\": \"{}\",\n  \"map\": \"{nn}\",\n  \"lap\": \"{time}\",\n  \"ghost_md5\": \"{ghost_md5}\",\n  \"ghost_fnv\": \"{}\",\n  \"trajectory_id\": \"{traj_id}\",\n  \"ghost_archive\": \"{a}\",\n  \"overlay\": \"{}\"\n}}\n", mp4.file_name().unwrap().to_string_lossy(), clip::overlay::file_id(&ghost).unwrap_or_default(), overlay_col.replace('"', "\\\""));
        std::fs::write(&side, json).map_err(|e| format!("{}: {e}", side.display()))?;
    }

    // --- the mp4 to the box: staged OUTSIDE the OneDrive tree, then copied in
    // beside the raw clip (where vjeux watches them). A chunked push straight
    // into `Maps\Tiny\videos` fails with "Permission denied" moving its temp
    // chunk — OneDrive holds a lock on files in the synced tree while it
    // uploads, and a 40 MB push is 115 renames (06 failed that way twice).
    // The ship runs from the staged copy, which is never under OneDrive.
    let r_stage_dir = format!("{VID}/mp4");
    let r_mp4 = format!("{r_stage_dir}/{name}.mp4");
    let r_watch = format!("{box_videos}/{name}.mp4");
    let mp4_md5 = md5_of(&mp4)?;
    let have = wsx.sh(&format!("md5sum '{r_mp4}' 2>/dev/null | cut -c1-32")).unwrap_or_default().trim().to_string();
    if have == mp4_md5 {
        eprintln!("mp4 already on the box ({mp4_md5}) — not pushing");
    } else {
        eprintln!("pushing the mp4 ({} MB) to the box …", std::fs::metadata(&mp4).map(|m| m.len() / 1_000_000).unwrap_or(0));
        wsx.sh(&format!("mkdir -p '{r_stage_dir}'"))?;
        wsx.push(&mp4, &r_mp4)?;
        let now = wsx.sh(&format!("md5sum '{r_mp4}' | cut -c1-32")).unwrap_or_default().trim().to_string();
        if now != mp4_md5 {
            return Err(format!("{r_mp4} on the box reads md5 {now}, pushed {mp4_md5}"));
        }
    }
    println!("box: {}", to_win(&r_mp4));
    // the watch copy is a convenience, not the pipeline: OneDrive may refuse it
    match wsx.sh(&format!("mkdir -p '{box_videos}' && cp -f '{r_mp4}' '{r_watch}' && echo ok")) {
        Ok(_) => println!("box (watch copy): {}", to_win(&r_watch)),
        Err(e) => eprintln!("watch copy into the OneDrive videos folder failed (the ship uses the staged copy): {e}"),
    }

    // --- the store
    if let Some(dest) = f("--store") {
        let mut sent = Vec::new();
        for p in [&local_webm, &mp4, &sheet, &mp4.with_extension("mp4.json")] {
            if !p.is_file() {
                continue;
            }
            // an existing render came FROM a store: only what is new here goes back
            if from_webm.is_some() && *p == local_webm {
                continue;
            }
            copy_to_store(p, &dest)?;
            sent.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
        println!("store: {dest}/ ← {}", sent.join(" "));
    }

    // --- the ship, detached on the box; `tinyctl shipwatch` collects the URL
    if tmmaps::cli::has(args, "--ship") {
        // A HELD MAP (holds.tsv) ships nothing: the clip is rendered and banked,
        // the row is recorded `held` so the watcher never launches it.
        if let Some(reason) = read_holds(&out).get(nn.as_str()) {
            println!("HELD ({reason}): {name} is rendered and banked but NOT shipped (holds.tsv)");
            record_ship_row(&out, &nn, &time, &name, &format!("{VID}/ship/{name}.done"), "held")?;
            return finish_row(&out, &nn, &time, cps, &overlay_col, &name, &sheet, traj_id);
        }
        // UPLOADS WAIT FOR AN OPENING-CHECK RECEIPT (parent project via the
        // coordinator, 2026-09-10 18:45Z, after Argentina's bad opening went
        // out). The render loop no longer launches the ship: the clip is
        // recorded `staged` and `tinyctl shipwatch` launches it only when
        // `<out>/approvals.tsv` carries a receipt for this map AND lap, or the
        // map is listed in `<out>/prechecked.tsv` (clips that may go out
        // unchecked — none at first). Until then the page note reads
        // "staged — awaiting opening check". `--ship-unchecked` is the old
        // behaviour (launch now), by name.
        if !tmmaps::cli::has(args, "--ship-unchecked") {
            let approved = approval_for(&out, &nn, &time).is_some() || read_prechecked(&out).contains(nn.as_str());
            if !approved {
                println!("STAGED — awaiting the opening check: {name} is rendered and banked; shipwatch launches it once out/approvals.tsv has a receipt for {nn} {time} (or {nn} is in out/prechecked.tsv)");
                record_ship_row(&out, &nn, &time, &name, &format!("{VID}/ship/{name}.done"), "staged")?;
                return finish_row(&out, &nn, &time, cps, &overlay_col, &name, &sheet, traj_id);
            }
            println!("opening check: receipt on file for {nn} {time} — shipping");
        }
        // SUPERSEDED DURING ITS OWN RENDER? The player project replaced 21's
        // ghost twice in 40 minutes on 2026-09-10 (122.318 → 122.311 → 122.294):
        // a 15-minute render finished, its clip went up, and the watcher
        // retired it before it reached the page — an upload slot on the one
        // session for nothing. So, right before the ship, the FRESH ghost
        // folder (`--ghosts-src`, the sync source) is asked what the map's lap
        // is now; a lap that will pass the re-render gate against THIS clip
        // (better by `--min-gain-s`, default 0.1) means the next scan renders
        // that one, and this clip is recorded as superseded without touching
        // GitHub. A sliver under the threshold would be skipped by the gate
        // anyway, so this clip ships. The render, mp4 and store copy stand.
        if let Some(src) = f("--ghosts-src") {
            let min_gain: f64 = f("--min-gain-s").and_then(|s| s.parse().ok()).unwrap_or(0.1);
            let fresh = PathBuf::from(&src).join(format!("{nn}.Ghost.Gbx"));
            if let Ok(g2) = gbx::record::decode_ghost(fresh.to_str().unwrap_or("")) {
                if let Some(ms2) = g2.race_time_ms.or_else(|| g2.samples.last().map(|s| s.time_ms)) {
                    let fresh_time = format!("{}.{:03}", ms2 / 1000, ms2 % 1000);
                    let will_render = matches!(render_gate(ms2 as f64 / 1000.0, Some((race_ms as f64 / 1000.0, &name)), f("--build").as_deref(), min_gain), Gate::Render(_));
                    if fresh_time != time && will_render {
                        println!("SUPERSEDED DURING THE RENDER: {} now holds {fresh_time}, this clip is {time} — not shipped (the next scan renders {fresh_time})", fresh.display());
                        record_ship_row(&out, &nn, &time, &name, &format!("{VID}/ship/{name}.done"), "superseded")?;
                        return finish_row(&out, &nn, &time, cps, &overlay_col, &name, &sheet, traj_id);
                    }
                }
            }
        }
        let script = script_path()?;
        wsx.push(&script, BOX_SHIP_SH)?;
        let slug = map_slug(&nn);
        let done_file = format!("{VID}/ship/{name}.done");
        let _ = wsx.sh(&format!("mkdir -p {VID}/ship && rm -f '{done_file}' && chmod +x {BOX_SHIP_SH} && nohup sh {BOX_SHIP_SH} '{r_mp4}' '{slug}' '{VID}/ship/{name}' > /dev/null 2>&1 < /dev/null &"))?;
        record_ship_row(&out, &nn, &time, &name, &done_file, "pending")?;
        println!("ship: started on the box as {slug} — done file {done_file}; `tinyctl shipwatch --out {} --readme tiny/README.md` collects it", out.display());
    }

    finish_row(&out, &nn, &time, cps, &overlay_col, &name, &sheet, traj_id)
}

/// One row APPENDED to `<out>/ships.tsv` — never read-modify-written: the
/// watcher rewrites this file to mark statuses, and a read-modify-write from
/// this side raced it (map 03's newest row was clobbered out of the queue).
fn record_ship_row(out: &Path, nn: &str, time: &str, name: &str, done_file: &str, status: &str) -> Result<(), String> {
    use std::io::Write;
    let ships = out.join("ships.tsv");
    let fresh = !ships.exists() || std::fs::metadata(&ships).map(|m| m.len() == 0).unwrap_or(true);
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&ships).map_err(|e| format!("{}: {e}", ships.display()))?;
    if fresh {
        f.write_all(b"# nn\ttime\tname\tdone_file\tstatus\n").map_err(|e| e.to_string())?;
    }
    f.write_all(format!("{nn}\t{time}\t{name}\t{done_file}\t{status}\n").as_bytes()).map_err(|e| e.to_string())
}

/// The REPORT.md row per lap (stdout and `<out>/REPORT.md`) and the `Done`
/// record the state file keeps.
#[allow(clippy::too_many_arguments)]
fn finish_row(out: &Path, nn: &str, time: &str, cps: usize, overlay_col: &str, name: &str, sheet: &Path, traj_id: String) -> Result<Done, String> {
    println!();
    // the finish is the last "checkpoint" the decoder lists
    let row = format!("| {nn} | {} | {time} | {cps} cps | {overlay_col} | {name}.webm | {name}.mp4 | look at {} |", map_title(nn), sheet.display());
    println!("{row}");
    append_report_row(out, &row)?;
    Ok(Done { nn: nn.to_string(), ghost_md5: traj_id, time: time.to_string(), cps, clip: format!("{name}.webm"), sheet: sheet.to_path_buf() })
}

fn append_report_row(out: &Path, row: &str) -> Result<(), String> {
    let report = out.join("REPORT.md");
    let mut text = std::fs::read_to_string(&report).unwrap_or_default();
    if text.is_empty() {
        text.push_str("| map | title | time | cps | overlay | clip | mp4 | sheet |\n|---|---|---|---|---|---|---|---|\n");
    }
    text.push_str(row);
    text.push('\n');
    std::fs::write(&report, text).map_err(|e| format!("{}: {e}", report.display()))
}

/// Whether a map's newest certified lap is worth a render and an upload.
#[derive(Debug, PartialEq)]
pub enum Gate {
    /// Render, and why (the first lap, a build change, or a gain over the threshold).
    Render(String),
    /// Under the threshold: the gain in seconds over the published lap, and that lap.
    Skip { gain: f64, published: f64 },
}

/// THE RE-RENDER GATE. `newest` is the certified lap (seconds); `published` the
/// lap the map's published clip shows, with the clip's name (its suffix names
/// the build it was rendered on); `build` the build the loop renders on.
/// Renders on the first lap, on a build change, or when the gain reaches
/// `min_gain`; `min_gain <= 0` is the opt-out (every new ghost renders).
pub fn render_gate(newest: f64, published: Option<(f64, &str)>, build: Option<&str>, min_gain: f64) -> Gate {
    let Some((p_time, p_name)) = published else {
        return Gate::Render("the map's first lap".into());
    };
    if min_gain <= 0.0 {
        return Gate::Render("threshold off (--min-gain-s 0)".into());
    }
    if let Some(b) = build {
        if !p_name.ends_with(&format!("-{b}")) {
            return Gate::Render(format!("the published clip is not a {b} render ({p_name})"));
        }
    }
    let gain = p_time - newest;
    if gain + 1e-9 >= min_gain {
        Gate::Render(format!("{gain:.3} s better than the published {p_time:.3}"))
    } else {
        Gate::Skip { gain, published: p_time }
    }
}

/// The lap each map's PUBLISHED clip shows, from `ships.tsv`: the last row per
/// map whose status is a URL (published) or `pending` (uploading, about to be)
/// — never a `superseded` or `FAILED` one. Returns map → (seconds, clip name).
pub fn published_laps(ships: &str) -> std::collections::HashMap<String, (f64, String)> {
    let mut out = std::collections::HashMap::new();
    for l in ships.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 5 {
            continue;
        }
        let status = c[4].trim();
        if !(status.starts_with("https://") || status == "pending") {
            continue;
        }
        if let Ok(t) = c[1].trim().parse::<f64>() {
            out.insert(c[0].trim().to_string(), (t, c[2].trim().to_string()));
        }
    }
    out
}

fn secs(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(f64::NAN)
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
pub fn page_swap(text: &str, nn: &str, time: &str, label: &str, build_note: &str, url: &str) -> Result<String, String> {
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
    let new_line = format!("**{title}** — original author time `{orig}` · {label} **{time}** ({build_note})");
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    out[i] = new_line;
    // THE ROW'S WHOLE BLOCK, not just the next line. The block runs to the next
    // row; it may hold a `*latest lap … — video pending*` line (page-status puts
    // one right under the row) and the previous video's URL below that. Looking
    // only at the next non-empty line (2026-09-10, 05:27–05:51Z) inserted the
    // new URL above the pending line and LEFT THE OLD URL, so 15, 18 and 19
    // showed two videos and a stale pending line each. Now: the first asset
    // line in the block becomes the new URL, every other asset line goes, and
    // so does every pending line — the video is current the moment it is
    // swapped in; page-status re-adds a line if the README is ahead again.
    let end = block_end(&out, i);
    let mut asset_at: Option<usize> = None;
    let mut drop: Vec<usize> = Vec::new();
    for j in i + 1..end {
        if out[j].starts_with(ASSET_PREFIX) {
            if asset_at.is_none() {
                asset_at = Some(j);
            } else {
                drop.push(j);
            }
        } else if crate::pagestatus::is_status_note(&out[j]) {
            drop.push(j);
        }
    }
    match asset_at {
        Some(j) => out[j] = url.to_string(),
        None => {
            out.insert(i + 1, String::new());
            out.insert(i + 2, url.to_string());
            drop.iter_mut().for_each(|d| *d += 2);
        }
    }
    for j in drop.into_iter().rev() {
        out.remove(j);
        // and the blank that held it, when that leaves two blanks in a row
        if j > 0 && j < out.len() && out[j - 1].trim().is_empty() && out[j].trim().is_empty() {
            out.remove(j);
        }
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') {
        s.push('\n');
    }
    Ok(s)
}

pub const ASSET_PREFIX: &str = "https://github.com/user-attachments/assets/";
pub const PENDING_MARK: &str = "— video pending*";

/// The index one past a row's block: the next row line (`**Tiny …`) or a
/// heading, or the end of the page.
pub fn block_end(lines: &[String], row: usize) -> usize {
    (row + 1..lines.len()).find(|&j| lines[j].starts_with("**Tiny ") || lines[j].starts_with('#')).unwrap_or(lines.len())
}

/// `tinyctl shipwatch`: the ships `tinyctl video --ship` started, collected —
/// each done file read off the box; a `URL …` swaps the map's page row and,
/// with `--commit`, commits and pushes the page; a `FAILED …` is printed and
/// left pending so it is seen again. `--once` scans once; otherwise every 60 s.
pub fn shipwatch_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let out = PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/tinyvid".into()));
    let readme = f("--readme").map(PathBuf::from);
    let ghosts_dir = f("--ghosts-dir").map(PathBuf::from);
    let repo = f("--repo").map(PathBuf::from).or_else(|| readme.as_ref().and_then(|r| r.parent().and_then(|p| p.parent()).map(Path::to_path_buf)));
    let build_note = f("--build-note").unwrap_or_else(|| "build ship15, controls overlay".into());
    let commit = tmmaps::cli::has(args, "--commit");
    let wsx = Wsx::new(args);
    let ships = out.join("ships.tsv");
    let retry_after = Duration::from_secs(f("--retry-min").and_then(|s| s.parse::<u64>().ok()).unwrap_or(6) * 60);
    let min_gain: f64 = f("--min-gain-s").and_then(|s| s.parse().ok()).unwrap_or(0.1);
    let mut last_probe: Option<std::time::Instant> = None;
    let mut attitude_said: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    loop {
        let text = std::fs::read_to_string(&ships).unwrap_or_default();
        let mut rows: Vec<String> = text.lines().map(String::from).collect();
        // WHICH ROW IS STILL THE MAP'S LAP. The player project replaces a map's
        // ghost several times a day, so ships.tsv holds more than one row per
        // map — and a stale one that ships LATER would swap the page back to the
        // slower lap. Only the LAST row of each map is shipped; earlier ones are
        // marked superseded, and their ships are not retried.
        let last_of: std::collections::HashMap<String, usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.starts_with('#'))
            .filter_map(|(i, r)| r.split('\t').next().map(|nn| (nn.to_string(), i)))
            .collect();
        let mut dead_cookie: Vec<(String, String, String, String)> = Vec::new();
        let mut changed = false;
        let mut pending = 0;
        // ALL THE DONE FILES IN ONE BRIDGE CALL. One `wsx cat` per pending row
        // was ~15 calls a minute, and every call authorizes: after a few hours
        // the WhiteStick bridge answered "Too many requests … more than 6000
        // (api-connex-oauth-authorize-client)" and the box went unreachable for
        // everyone. One call per tick reads them all.
        let pending_files: Vec<String> = rows
            .iter()
            .filter(|r| !r.starts_with('#'))
            .filter_map(|r| {
                let c: Vec<&str> = r.split('\t').collect();
                (c.len() >= 5 && c[4] == "pending").then(|| c[3].to_string())
            })
            .collect();
        let mut done_of: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        if !pending_files.is_empty() {
            let list = pending_files.iter().map(|f| format!("'{f}'")).collect::<Vec<_>>().join(" ");
            if let Ok(out) = wsx.sh(&format!("for f in {list}; do [ -f \"$f\" ] && printf '%s\\t%s\\n' \"$f\" \"$(tr '\\n' ' ' < \"$f\" | cut -c1-200)\"; done; true")) {
                for l in out.lines() {
                    if let Some((f, c)) = l.split_once('\t') {
                        done_of.insert(f.trim().to_string(), c.trim().to_string());
                    }
                }
            }
        }
        for (i, row) in rows.iter_mut().enumerate() {
            if row.starts_with('#') {
                continue;
            }
            let cells: Vec<String> = row.split('\t').map(String::from).collect();
            if cells.len() < 5 || (cells[4] != "pending" && cells[4] != "staged") {
                continue;
            }
            // STAGED = rendered, banked, waiting for the opening-check receipt
            // (approvals.tsv: `nn<TAB>time`, or the map in prechecked.tsv). With
            // a receipt the row becomes `pending` and joins the launch queue on
            // this tick; without one it sits, and page-status says so.
            if cells[4] == "staged" {
                // THE ATTITUDE GATE comes first and a receipt cannot override
                // it (parent project via the coordinator, 2026-09-10 19:20Z): a
                // clip ships only when INPUT's README says its lap is CLEAN —
                // no inverted interval and no |roll|/|pitch| > 60° sustained
                // > 0.3 s. No table for the lap = not clean (fail closed).
                let readme = ghosts_dir.as_ref().map(|d| std::fs::read_to_string(d.join("README.md")).unwrap_or_default()).unwrap_or_default();
                match attitude_verdict(&readme, &cells[0], &cells[1]) {
                    Attitude::Clean => {}
                    verdict => {
                        if !attitude_said.contains(&(cells[0].clone(), cells[1].clone())) {
                            println!("{} {} {}: ATTITUDE GATE — {} — not shipped, whatever approvals.tsv says", chrono_now(), cells[0], cells[1], verdict.describe());
                            attitude_said.insert((cells[0].clone(), cells[1].clone()));
                        }
                        continue;
                    }
                }
                let approved = approval_for(&out, &cells[0], &cells[1]).is_some() || read_prechecked(&out).contains(cells[0].as_str());
                if !approved {
                    continue;
                }
                println!("{} {} {}: attitude clean + opening check receipt on file — queued for upload", chrono_now(), cells[0], cells[1]);
                *row = format!("{}\t{}\t{}\t{}\tpending", cells[0], cells[1], cells[2], cells[3]);
                changed = true;
            }
            let cells: Vec<String> = row.split('\t').map(String::from).collect();
            if last_of.get(&cells[0]) != Some(&i) {
                println!("{} {} {}: superseded by a newer lap — not shipped", chrono_now(), cells[0], cells[1]);
                *row = format!("{}\t{}\t{}\t{}\tsuperseded", cells[0], cells[1], cells[2], cells[3]);
                changed = true;
                continue;
            }
            // THE GHOSTS README OUTRANKS THIS FILE — once its ghost has caught
            // up. A lap that landed after the clip was cut (the input arm
            // replaces a map's ghost several times a day) makes the staged clip
            // stale before it ever shipped; the page converges on the newest
            // lap per map, so the row is superseded here and the newer lap's
            // render ships instead. Two guards: only a README row on the same
            // build counts (a map that fell back to an older build keeps its
            // ship15 clip), and the README's lap must be the one the ghost FILE
            // in --ghosts-dir actually holds — when the README is ahead of its
            // file (09 on 2026-09-10: row 28.292, file still 28.572) nothing
            // newer can render, so the staged clip is still the best there is
            // and ships.
            if let Some(d) = &ghosts_dir {
                let readme = std::fs::read_to_string(d.join("README.md")).unwrap_or_default();
                if let Some((newest, build)) = readme_current_lap(&readme, &cells[0]) {
                    let file_time = gbx::record::decode_ghost(d.join(format!("{}.Ghost.Gbx", cells[0])).to_str().unwrap_or(""))
                        .ok()
                        .and_then(|g| g.race_time_ms.or_else(|| g.samples.last().map(|s| s.time_ms)))
                        .map(|ms| format!("{}.{:03}", ms / 1000, ms % 1000));
                    // and the newer lap must be one the render loop WILL render:
                    // better than this staged clip by the re-render threshold
                    // (--min-gain-s, default 0.1); a sliver under it is skipped
                    // by the loop, so the staged clip is the best that will exist
                    let will_render = matches!(render_gate(secs(&newest), Some((secs(&cells[1]), &cells[2])), Some(build.as_str()), min_gain), Gate::Render(_));
                    if newest != cells[1] && build_note.contains(&build) && file_time.as_deref() == Some(newest.as_str()) && will_render {
                        println!("{} {} {}: the ghosts README now says {newest} ({build}) and its ghost file agrees — superseded, not shipped", chrono_now(), cells[0], cells[1]);
                        *row = format!("{}\t{}\t{}\t{}\tsuperseded", cells[0], cells[1], cells[2], cells[3]);
                        changed = true;
                        continue;
                    }
                }
            }
            pending += 1;
            let (nn, time, name, done_file) = (&cells[0], &cells[1], &cells[2], &cells[3]);
            // A HELD MAP'S PENDING CLIP IS NEVER LAUNCHED (holds.tsv, read every
            // tick). A ship already running on the box for it finishes (the lock
            // is the box's), but its URL is not swapped into the page while the
            // hold stands: the row is left pending and picked up when lifted.
            if let Some(reason) = read_holds(&out).get(nn.as_str()) {
                println!("{} {nn} {time}: HELD ({reason}) — not launched, not swapped", chrono_now());
                continue;
            }
            let Some(done) = done_of.get(done_file.as_str()).cloned() else {
                // NO VERDICT ON THE BOX: the ship never ran, or died before
                // writing its done file (a box reboot, a killed shell, a session
                // that expired before this row's turn came — 2026-09-09 left
                // rows like that pending forever, and the drain of a fresh cookie
                // skipped them). Queue it for a launch; the launch below happens
                // only when nothing is running on the box, so a ship that IS
                // running (its done file removed at launch) is never doubled.
                dead_cookie.push((nn.clone(), time.clone(), name.clone(), done_file.clone()));
                continue;
            };
            let done = done.trim().to_string();
            // PENDING <url>: uploaded and registered, the gate not yet 200 when the
            // box gave up (a big asset can take an hour) — probe it from here,
            // anonymously, and never re-upload
            let published: Option<String> = if let Some(url) = done.strip_prefix("URL ") {
                Some(url.trim().to_string())
            } else if let Some(url) = done.strip_prefix("PENDING ") {
                let url = url.trim().to_string();
                match anon_probe(&url) {
                    Ok((200, bytes)) if bytes > 1_000_000 => {
                        println!("{} {nn} {time}: the gate turned 200 ({bytes} bytes) for {url}", chrono_now());
                        Some(url)
                    }
                    Ok((code, bytes)) => {
                        println!("{} {nn} {time}: still pending — anonymous fetch http {code} ({bytes} bytes) for {url}", chrono_now());
                        None
                    }
                    Err(e) => {
                        println!("{} {nn} {time}: gate probe failed to run: {e}", chrono_now());
                        None
                    }
                }
            } else {
                None
            };
            if let Some(url) = published {
                let url = url.as_str();
                println!("{} {nn} {time}: PUBLISHED {url}", chrono_now());
                if let Some(readme) = &readme {
                    // THE PAGE IS BEST-EFFORT, THE QUEUE IS NOT. A git failure
                    // here — nothing to commit because a duplicate watcher got
                    // there first, a rejected push, a rebase conflict — used to
                    // propagate and KILL the watcher, so a drain stopped dead
                    // with clips still staged. It is logged and the queue goes on;
                    // the next published clip re-swaps and re-pushes anyway.
                    let swap = (|| -> Result<(), String> {
                        let page = std::fs::read_to_string(readme).map_err(|e| format!("{}: {e}", readme.display()))?;
                        // the driver label, read NOW from the ghosts README (a row can
                        // land after the cut): TAS = tiny ghost; vjeux's own run says so
                        let label = match &ghosts_dir {
                            Some(d) => lap_label(&std::fs::read_to_string(d.join("README.md")).unwrap_or_default(), nn, time),
                            None => "tiny ghost".to_string(),
                        };
                        let new = page_swap(&page, nn, time, &label, &build_note, url)?;
                        let unchanged = new == page;
                        std::fs::write(readme, &new).map_err(|e| format!("{}: {e}", readme.display()))?;
                        println!("  page: row {} swapped in {} ({label})", map_title(nn), readme.display());
                        if commit && !unchanged {
                            let repo = repo.clone().ok_or("--commit needs --repo (or a --readme inside the repo)")?;
                            let msg = format!("tiny page: {} = {time} ({label}, build ship15) with the controls overlay ({name}.mp4)", map_title(nn));
                            git(&repo, &["add", &readme.strip_prefix(&repo).unwrap_or(readme).display().to_string()])?;
                            git(&repo, &["commit", "-q", "-m", &msg])?;
                            git(&repo, &["pull", "-q", "--rebase"])?;
                            git(&repo, &["push", "-q"])?;
                            println!("  pushed: {msg}");
                        } else if commit {
                            println!("  page already carries this row — nothing to commit");
                        }
                        Ok(())
                    })();
                    if let Err(e) = swap {
                        println!("  PAGE NOT UPDATED for {nn} {time} (the clip IS published at {url}): {e}");
                    }
                }
                *row = format!("{nn}\t{time}\t{name}\t{done_file}\t{url}");
                changed = true;
                pending -= 1;
                // A ship that WORKED means the session is good: let the next one
                // start on this tick instead of waiting out the retry interval,
                // which exists for a session being renewed by hand.
                last_probe = None;
            } else if !done.starts_with("PENDING ") {
                println!("{} {nn} {time}: {done}", chrono_now());
                // A dead browser cookie fails a ship in two places — at the probe
                // (302 → /login) and inside the uploader (ghvid exit 3, "no upload
                // CSRF token", which is the same session being gone). Both are
                // cured by a fresh cookie and by nothing else here, so both are
                // re-launched every `retry_after`; nothing that got past the
                // upload is ever re-uploaded.
                let cookie_dead = done.contains("cookie probe") || done.contains("no upload CSRF") || done.contains("attachment upload failed");
                if cookie_dead {
                    dead_cookie.push((nn.clone(), time.clone(), name.clone(), done_file.clone()));
                }
            }
        }
        // ONE SHIP AT A TIME, AND NO PROBE FROM HERE. The watcher used to probe
        // github.com itself and then release the whole queue at once: every
        // launched script probed again in the same second, on the same session,
        // writing one shared cookie jar — and GitHub answers a burst of parallel
        // replays of a rotating session by logging it out. Five copied sessions
        // died that way on 2026-09-09. Now the box holds the only client: the
        // ship script probes, uploads and gates inside its lock, and this side
        // launches the NEXT one only when nothing is running there. A dead
        // session therefore costs one 302 per tick, by one client, and the queue
        // resumes by itself when a fresh cookie lands.
        if !dead_cookie.is_empty() {
            let due = last_probe.map(|t: std::time::Instant| t.elapsed() >= retry_after).unwrap_or(true);
            let busy = wsx
                .sh("ps aux | grep -c '[t]inyship.sh'")
                .map(|s| s.trim().parse::<u32>().unwrap_or(0))
                .unwrap_or(0);
            if busy > 0 {
                println!("{} {} clip(s) waiting; a ship is running on the box", chrono_now(), dead_cookie.len());
            } else if due {
                last_probe = Some(std::time::Instant::now());
                let (nn, time, name, done_file) = &dead_cookie[0];
                let r_mp4 = format!("{VID}/mp4/{name}.mp4");
                let r_watch = format!("{}/{name}.mp4", BOX_VIDEOS);
                let outbase = done_file.trim_end_matches(".done").to_string();
                let slug = map_slug(nn);
                // CHECK AND LAUNCH IN ONE BOX-SIDE COMMAND. The batched done-file
                // read above and the busy check are two calls seconds apart; a
                // ship that wrote its verdict and exited BETWEEN them (23 at
                // 09:00:31Z on 2026-09-10) looked like "no verdict, nothing
                // running" and was relaunched — and the relaunch's `rm -f DONE`
                // destroyed the fresh URL and re-uploaded the clip. So the box
                // itself decides at the last instant: a verdict carrying a URL
                // (URL or PENDING) blocks the launch, and so does a ship still
                // running; only then is the stale verdict removed and the ship
                // started.
                match wsx.sh(&format!(
                    "mkdir -p {VID}/mp4 && [ -f '{r_mp4}' ] || cp -f '{r_watch}' '{r_mp4}'; \
                     if grep -qs '^URL \\|^PENDING ' '{done_file}'; then echo VERDICT-EXISTS; \
                     elif pgrep -f '[t]inyship.sh' > /dev/null; then echo BUSY; \
                     else rm -f '{done_file}' && nohup sh {BOX_SHIP_SH} '{r_mp4}' '{slug}' '{outbase}' > /dev/null 2>&1 < /dev/null & echo LAUNCHED; fi"
                )) {
                    Ok(out) if out.contains("LAUNCHED") => println!("{} launching {nn} {time} ({} clip(s) held; one at a time — the box's lock covers the probe, the upload and the gate)", chrono_now(), dead_cookie.len()),
                    Ok(out) if out.contains("VERDICT-EXISTS") => {
                        println!("{} {nn} {time}: a verdict with a URL appeared since the read — not relaunched (collected next tick)", chrono_now());
                        last_probe = None;
                    }
                    Ok(_) => println!("{} {nn} {time}: a ship started on the box meanwhile — not relaunched", chrono_now()),
                    Err(e) => println!("  could not launch {name}: {e}"),
                }
            }
        }
        if changed {
            // MERGE, don't overwrite: the render loop appends new rows to this
            // file while we work, and writing our stale copy back dropped one
            // map's newest lap out of the queue. Re-read, apply our status
            // changes by (map, time), keep every row we have not seen.
            let mut want: std::collections::HashMap<(String, String), String> = std::collections::HashMap::new();
            for r in &rows {
                let c: Vec<&str> = r.split('\t').collect();
                if c.len() >= 5 && !r.starts_with('#') {
                    want.insert((c[0].to_string(), c[1].to_string()), r.clone());
                }
            }
            let fresh = std::fs::read_to_string(&ships).unwrap_or_default();
            let merged: Vec<String> = fresh
                .lines()
                .map(|l| {
                    let c: Vec<&str> = l.split('\t').collect();
                    if c.len() >= 5 && !l.starts_with('#') {
                        if let Some(updated) = want.get(&(c[0].to_string(), c[1].to_string())) {
                            return updated.clone();
                        }
                    }
                    l.to_string()
                })
                .collect();
            std::fs::write(&ships, merged.join("\n") + "\n").map_err(|e| format!("{}: {e}", ships.display()))?;
        }
        if tmmaps::cli::has(args, "--once") {
            println!("{pending} pending");
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(f("--tick-s").and_then(|s| s.parse().ok()).unwrap_or(120)));
    }
}

/// The anonymous gate, from this side: `/usr/bin/curl` with a CLEARED
/// environment (no cookie jar, no token, no netrc — a gate with credentials is
/// not a gate), through `CLIP_PROXY` (default `http://fwdproxy:8080`: a
/// devserver or OD reaches github.com only that way; a route, not a
/// credential). Returns the status and the bytes fetched.
fn anon_probe(url: &str) -> Result<(u16, u64), String> {
    let proxy = std::env::var("CLIP_PROXY").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "http://fwdproxy:8080".into());
    let dir = std::env::temp_dir().join(format!("tinyctl-anon-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let body = dir.join("anon.mp4");
    let out = Command::new("/usr/bin/curl")
        .env_clear()
        .args(["-s", "-L", "--max-time", "300", "-o"])
        .arg(&body)
        .args(["-w", "%{http_code}", "-x", &proxy, url])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    let code: u16 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0);
    let bytes = std::fs::metadata(&body).map(|m| m.len()).unwrap_or(0);
    let _ = std::fs::remove_dir_all(&dir);
    Ok((code, bytes))
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
/// The store's content-addressed ghost folder, the default `--ghost-archive`.
const GHOST_ARCHIVE_DEFAULT: &str = "/home/vjeux/persistent/private-30d/tm-player/tiny/ghost-archive";

/// Copy `ghost` to `<dir>/<md5>.Ghost.Gbx` (write-once: an existing file with
/// that name IS these bytes, by construction — it is verified, not overwritten)
/// and write the sidecar `<dir>/<md5>.json`. The sidecar is rewritten only when
/// absent, so the first render's facts stand.
#[allow(clippy::too_many_arguments)]
pub fn archive_ghost(dir: &Path, ghost: &Path, md5: &str, nn: &str, time: &str, race_ms: i32, fnv: &str, map_md5: &str, build: Option<&str>, readme_row: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let dst = dir.join(format!("{md5}.Ghost.Gbx"));
    if dst.is_file() {
        let have = md5_of(&dst)?;
        if have != md5 {
            return Err(format!("{}: holds md5 {have}, not the {md5} its name claims — the archive is corrupt, refusing to render on top of it", dst.display()));
        }
    } else {
        let tmp = dir.join(format!(".{md5}.Ghost.Gbx.{}", std::process::id()));
        std::fs::copy(ghost, &tmp).map_err(|e| format!("{} → {}: {e}", ghost.display(), tmp.display()))?;
        std::fs::rename(&tmp, &dst).map_err(|e| format!("{} → {}: {e}", tmp.display(), dst.display()))?;
        let back = md5_of(&dst)?;
        if back != md5 {
            let _ = std::fs::remove_file(&dst);
            return Err(format!("{}: read back as md5 {back}, wrote {md5}", dst.display()));
        }
        println!("ghost archive: {} ← {}", dst.display(), ghost.display());
    }
    let side = dir.join(format!("{md5}.json"));
    if !side.is_file() {
        let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let json = format!(
            "{{\n  \"ghost_md5\": \"{md5}\",\n  \"ghost_fnv\": \"{}\",\n  \"trajectory_id\": \"{fnv}\",\n  \"map\": \"{nn}\",\n  \"title\": \"{}\",\n  \"lap\": \"{time}\",\n  \"lap_ms\": {race_ms},\n  \"map_md5\": \"{map_md5}\",\n  \"build\": {},\n  \"readme_row\": \"{}\",\n  \"source\": \"{}\",\n  \"archived_unix\": {when}\n}}\n",
            clip::overlay::file_id(ghost).unwrap_or_default(),
            esc(&map_title(nn)),
            build.map(|b| format!("\"{}\"", esc(b))).unwrap_or_else(|| "null".into()),
            esc(readme_row),
            esc(&ghost.display().to_string())
        );
        let tmp = dir.join(format!(".{md5}.json.{}", std::process::id()));
        std::fs::write(&tmp, json).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &side).map_err(|e| format!("{} → {}: {e}", tmp.display(), side.display()))?;
    }
    Ok(())
}

/// `<out>/approvals.tsv`: `nn<TAB>time<TAB>by<TAB>note` — the opening-check
/// receipts. A row approves ONE lap of ONE map; `time` may be `*` to approve
/// whatever lap of that map is staged (a standing approval). Returns the
/// approving row.
pub fn approval_for(out: &Path, nn: &str, time: &str) -> Option<String> {
    let text = std::fs::read_to_string(out.join("approvals.tsv")).unwrap_or_default();
    find_approval(&text, nn, time)
}

pub fn find_approval(text: &str, nn: &str, time: &str) -> Option<String> {
    text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).find(|l| {
        let c: Vec<&str> = l.split('\t').map(str::trim).collect();
        c.len() >= 2 && c[0] == nn && (c[1] == time || c[1] == "*")
    }).map(String::from)
}

/// `<out>/prechecked.tsv`: one map number per line — clips of these maps may
/// go out without a receipt.
pub fn read_prechecked(out: &Path) -> std::collections::HashSet<String> {
    parse_prechecked(&std::fs::read_to_string(out.join("prechecked.tsv")).unwrap_or_default())
}

pub fn parse_prechecked(text: &str) -> std::collections::HashSet<String> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').next())
        .map(str::trim)
        .filter(|nn| nn.len() == 2 && nn.chars().all(|c| c.is_ascii_digit()))
        .map(String::from)
        .collect()
}

/// `<out>/holds.tsv`: `nn<TAB>reason[<TAB>mode]` per held map (comments with
/// `#`). `mode` = `none` (archive the ghost only — no render, no upload; the
/// default) or `render` (render, cut and stage the clip, but never upload it —
/// the reviewer wants to see the opening first). A held map is never shipped
/// and never swapped into the page; page-status notes the hold.
pub fn read_holds(out: &Path) -> std::collections::HashMap<String, String> {
    parse_holds(&std::fs::read_to_string(out.join("holds.tsv")).unwrap_or_default())
}

/// The held maps whose mode is `render`.
pub fn read_render_holds(out: &Path) -> std::collections::HashSet<String> {
    parse_hold_modes(&std::fs::read_to_string(out.join("holds.tsv")).unwrap_or_default())
        .into_iter()
        .filter(|(_, mode)| mode == "render")
        .map(|(nn, _)| nn)
        .collect()
}

pub fn parse_holds(text: &str) -> std::collections::HashMap<String, String> {
    hold_rows(text).into_iter().map(|(nn, reason, _)| (nn, reason)).collect()
}

pub fn parse_hold_modes(text: &str) -> std::collections::HashMap<String, String> {
    hold_rows(text).into_iter().map(|(nn, _, mode)| (nn, mode)).collect()
}

fn hold_rows(text: &str) -> Vec<(String, String, String)> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').map(str::trim).collect();
            let nn = c[0];
            if nn.len() != 2 || !nn.chars().all(|ch| ch.is_ascii_digit()) {
                return None;
            }
            let reason = c.get(1).filter(|r| !r.is_empty()).unwrap_or(&"held").to_string();
            let mode = match c.get(2).map(|m| m.to_ascii_lowercase()) {
                Some(m) if m == "render" => "render".to_string(),
                _ => "none".to_string(),
            };
            Some((nn.to_string(), reason, mode))
        })
        .collect()
}

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
        let s = page_swap(PAGE, "01", "19.100", "tiny ghost", "build ship15, controls overlay", "https://github.com/user-attachments/assets/bbbb").unwrap();
        assert!(s.contains("**Tiny Summer 2026 - 01** — original author time `23.144` · tiny ghost **19.100** (build ship15, controls overlay)\n\nhttps://github.com/user-attachments/assets/bbbb\n"), "{s}");
        assert!(!s.contains("aaaa"));
        assert!(s.contains("cccc"), "the other rows stay");
        let s = page_swap(&s, "23", "110.000", "tiny ghost", "build ship15, controls overlay", "https://github.com/user-attachments/assets/dddd").unwrap();
        assert!(s.contains("**Tiny Norway 2026** — original author time `75.112` · tiny ghost **110.000** (build ship15, controls overlay)\n\nhttps://github.com/user-attachments/assets/dddd\n"), "{s}");
        assert!(!s.contains("Tiny Summer 2026 - 23"));
        // and again by the new name
        let s2 = page_swap(&s, "23", "109.000", "tiny ghost", "build ship15, controls overlay", "https://github.com/user-attachments/assets/eeee").unwrap();
        assert!(s2.contains("**110.000**") == false && s2.contains("eeee"));
        let s = page_swap(&s, "18", "40.000", "driven by vjeux (playtest)", "build ship15, controls overlay", "https://github.com/user-attachments/assets/ffff").unwrap();
        assert!(s.contains("**Tiny Summer 2026 - 18** — original author time `51.352` · driven by vjeux (playtest) **40.000** (build ship15, controls overlay)\n\nhttps://github.com/user-attachments/assets/ffff\n\n**Tiny Norway 2026**"), "{s}");
        assert!(s.ends_with("\n\n"), "the trailing newlines are kept");
        assert!(page_swap(PAGE, "09", "1.000", "tiny ghost", "x", "u").is_err(), "a map the page lacks is an error");
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

/// `rsync -a src/ dst/` (ssh, batch mode) with extra filters; `src` may be
/// `host:dir` or a local dir. A PARTIAL transfer (rsync 23/24: a file the far
/// side was rewriting as we read it — the input arm replaces a ghost every
/// twenty minutes, and manifoldfs answers "No data available" for the seconds
/// that takes) is a warning, not a failed scan: rsync lands each file through
/// a temporary and a rename, so the files that did not make it keep their
/// previous copy here and come over on the next tick.
fn rsync_dir(src: &str, dst: &Path, filters: &[&str]) -> Result<(), String> {
    let src = if src.ends_with('/') { src.to_string() } else { format!("{src}/") };
    let st = Command::new("rsync")
        .args(["-a", "-e", "ssh -o BatchMode=yes"])
        .args(filters)
        .arg(&src)
        .arg(format!("{}/", dst.display()))
        .status()
        .map_err(|e| format!("rsync: {e}"))?;
    match st.code() {
        Some(0) => Ok(()),
        Some(23) | Some(24) => {
            eprintln!("rsync {src} → {}: partial transfer ({st}) — a file was being rewritten; its previous copy stands until the next scan", dst.display());
            Ok(())
        }
        _ => Err(format!("rsync {src} → {}: {st}", dst.display())),
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;

    const README: &str = "| map | file | time | credits | build | build md5 | found by | validated map |\n\
| 01 | 01.Ghost.Gbx | 19.381 | 4 | ship15 | 9ae890a9 | PPO | x |\n\
| 03 | 03.Ghost.Gbx | 28.989 | 5 | ship15 | 4d759865 | GEN savestate search | x |\n\
| 21 | 21.Ghost.Gbx | 116.384 | 17 | ship14 | 6858b37b | GEN savestate search | x |\n\
| 03 | 03.Ghost.Gbx | 20.993 | regenerated on ship15 4d759865 (first lap ever on 03; vjeux playtest) | 20.993 s | x |\n";

    /// The README is appended in more than one row shape; the LAST row naming
    /// the map and the lap wins, the build is whatever that row says, and a
    /// playtest lap is labelled as vjeux's, not as a TAS ghost.
    #[test]
    fn readme_rows_are_found_by_map_and_lap_whatever_their_shape() {
        assert!(readme_row(README, "01", "19.381").unwrap().contains("ship15"));
        assert!(readme_row(README, "21", "116.384").unwrap().contains("ship14"));
        assert!(readme_row(README, "03", "20.993").unwrap().contains("ship15"));
        assert_eq!(readme_row(README, "03", "20.340"), None, "a lap the README has not written yet");
        assert_eq!(lap_label(README, "03", "28.989"), "tiny ghost");
        assert_eq!(lap_label(README, "03", "20.993"), "driven by vjeux (playtest)");
        assert_eq!(lap_label(README, "09", "1.000"), "tiny ghost", "no row: a TAS lap");
    }

    /// The README's FIRST table row per map is the map's current lap; a row of
    /// another shape (the appended playtest note) is not read as one, a map
    /// without a row supersedes nothing, and the build comes back with the time
    /// so a fallback to an older build does not retire a ship15 clip.
    #[test]
    fn the_readme_names_the_current_lap_and_its_build() {
        assert_eq!(readme_current_lap(README, "01"), Some(("19.381".into(), "ship15".into())));
        assert_eq!(readme_current_lap(README, "21"), Some(("116.384".into(), "ship14".into())));
        assert_eq!(readme_current_lap(README, "03"), Some(("28.989".into(), "ship15".into())), "the first table row, not the appended note");
        assert_eq!(readme_current_lap(README, "09"), None);
        assert_eq!(readme_current_lap("| 04 | 04.Ghost.Gbx | — | 0 | none |", "04"), None, "a placeholder is not a lap");
    }
}

#[cfg(test)]
mod ships_tests {
    /// The rule shipwatch applies to ships.tsv: only a map's LAST row is still
    /// its lap. The player project replaces a ghost several times a day, so a
    /// pending row from two laps ago must never ship — it would swap the page
    /// back to the slower time, and after the newer one had already landed.
    fn last_row_per_map(rows: &[&str]) -> Vec<bool> {
        let last: std::collections::HashMap<&str, usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.starts_with('#'))
            .filter_map(|(i, r)| r.split('\t').next().map(|nn| (nn, i)))
            .collect();
        rows.iter()
            .enumerate()
            .map(|(i, r)| !r.starts_with('#') && last.get(r.split('\t').next().unwrap_or("")) == Some(&i))
            .collect()
    }

    #[test]
    fn only_the_last_row_of_a_map_is_still_its_lap() {
        let rows = vec![
            "# nn\ttime\tname\tdone\tstatus",
            "03\t28.989\ta\td\tpending",
            "04\t29.474\tb\td\tpending",
            "03\t20.993\tc\td\tpending",
            "25\t121.235\te\td\thttps://x",
        ];
        assert_eq!(last_row_per_map(&rows), vec![false, false, true, true, true]);
    }
}

#[cfg(test)]
mod readme_shape_tests {
    use super::*;

    /// Since 2026-09-10 the ghosts README has a second table, `| map | file md5 |
    /// time |`, whose rows name the map and the lap but not the build. The row
    /// that names the ghost FILE is the one that carries the build and must win.
    #[test]
    fn the_md5_table_does_not_hide_the_build() {
        let readme = "| 25 | 25.Ghost.Gbx | 119.115 | 15 | ship15 | bd1a146f | PPO | x |\n\
\n\
| map | file md5 | time |\n\
| 25 | ac27c2fd | 119.115 |\n";
        assert!(readme_row(readme, "25", "119.115").unwrap().contains("ship15"));
        assert_eq!(readme_current_lap(readme, "25"), Some(("119.115".into(), "ship15".into())));
        // a lap that appears ONLY in the md5 table is still found (no build word)
        assert!(readme_row("| 25 | ac27c2fd | 119.115 |\n", "25", "119.115").is_some());
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    /// The re-render gate: a first lap, a build change and a gain at or over the
    /// threshold render; a sliver under it is skipped with its gain; the gain is
    /// measured against the PUBLISHED lap, so slivers accumulate and the render
    /// happens when they cross the line; `--min-gain-s 0` renders everything.
    #[test]
    fn the_gate_renders_first_laps_build_changes_and_real_gains_only() {
        assert_eq!(render_gate(84.954, None, Some("ship15"), 0.1), Gate::Render("the map's first lap".into()));
        assert!(matches!(render_gate(48.747, Some((48.753, "15-ghost-48.753-ship15")), Some("ship15"), 0.1), Gate::Skip { gain, published } if (gain - 0.006).abs() < 1e-9 && published == 48.753));
        assert!(matches!(render_gate(48.653, Some((48.753, "15-ghost-48.753-ship15")), Some("ship15"), 0.1), Gate::Render(_)), "exactly the threshold renders");
        assert!(matches!(render_gate(48.654, Some((48.753, "15-ghost-48.753-ship15")), Some("ship15"), 0.1), Gate::Skip { .. }));
        assert!(matches!(render_gate(121.235, Some((121.235, "25-ghost-121.235-ship14")), Some("ship15"), 0.1), Gate::Render(w) if w.contains("not a ship15 render")));
        assert!(matches!(render_gate(48.753, Some((48.753, "15-ghost-48.753-ship15")), Some("ship15"), 0.0), Gate::Render(w) if w.contains("threshold off")));
        assert!(matches!(render_gate(48.760, Some((48.753, "15-ghost-48.753-ship15")), Some("ship15"), 0.1), Gate::Skip { gain, .. } if gain < 0.0), "a slower lap is not a gain");
        // accumulation: two slivers of 0.087 and then 0.027 more
        assert!(matches!(render_gate(48.747, Some((48.834, "15-ghost-48.834-ship15")), Some("ship15"), 0.1), Gate::Skip { .. }));
        assert!(matches!(render_gate(48.720, Some((48.834, "15-ghost-48.834-ship15")), Some("ship15"), 0.1), Gate::Render(_)));
    }

    /// The published reference comes from ships.tsv: URL rows and pending rows
    /// count, superseded and FAILED ones do not, the last one per map wins.
    #[test]
    fn the_published_lap_is_read_off_the_ship_rows() {
        let ships = "# nn\ttime\tname\tdone_file\tstatus\n\
15\t50.336\t15-ghost-50.336-ship15\t/x\thttps://github.com/user-attachments/assets/a\n\
15\t49.097\t15-ghost-49.097-ship15\t/x\thttps://github.com/user-attachments/assets/b\n\
15\t48.747\t15-ghost-48.747-ship15\t/x\tsuperseded\n\
21\t122.311\t21-ghost-122.311-ship15\t/x\tFAILED cookie probe HTTP 302\n\
22\t96.298\t22-ghost-96.298-ship15\t/x\tpending\n";
        let p = published_laps(ships);
        assert_eq!(p.get("15").map(|(t, n)| (*t, n.as_str())), Some((49.097, "15-ghost-49.097-ship15")));
        assert_eq!(p.get("21"), None);
        assert_eq!(p.get("22").map(|(t, _)| *t), Some(96.298));
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;

    /// The archive is write-once and content-addressed: the first call copies
    /// the ghost and writes the sidecar, a second call with the same bytes is a
    /// no-op that verifies, and a file whose bytes do not match its name is a
    /// corrupt archive the render refuses to build on.
    #[test]
    fn the_ghost_archive_is_write_once_and_verified() {
        let dir = std::env::temp_dir().join(format!("tinyctl-archive-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let ghost = src.join("24.Ghost.Gbx");
        std::fs::write(&ghost, b"GBX-not-really-but-bytes-are-bytes").unwrap();
        let md5 = md5_of(&ghost).unwrap();
        let arch = dir.join("ghost-archive");
        archive_ghost(&arch, &ghost, &md5, "24", "100.116", 100_116, "c3589722871e4283", "a7ca005a", Some("ship15"), "| 24 | 24.Ghost.Gbx | 100.116 | 14 | ship15 | a7ca005a | PPO | x |").unwrap();
        let dst = arch.join(format!("{md5}.Ghost.Gbx"));
        assert_eq!(std::fs::read(&dst).unwrap(), std::fs::read(&ghost).unwrap());
        let side = std::fs::read_to_string(arch.join(format!("{md5}.json"))).unwrap();
        assert!(side.contains("\"ghost_md5\": \"") && side.contains("\"lap_ms\": 100116") && side.contains("\"trajectory_id\": \"c3589722871e4283\"") && side.contains("\"ghost_fnv\": \"") && side.contains("\"build\": \"ship15\"") && side.contains("Tiny Poland 2026"), "{side}");
        // second call: no change, no error
        let before = std::fs::metadata(&dst).unwrap().modified().unwrap();
        archive_ghost(&arch, &ghost, &md5, "24", "100.116", 100_116, "c3589722871e4283", "a7ca005a", Some("ship15"), "").unwrap();
        assert_eq!(std::fs::metadata(&dst).unwrap().modified().unwrap(), before);
        // a corrupt archive entry is refused
        std::fs::write(&dst, b"tampered").unwrap();
        let e = archive_ghost(&arch, &ghost, &md5, "24", "100.116", 100_116, "c3589722871e4283", "a7ca005a", Some("ship15"), "").unwrap_err();
        assert!(e.contains("corrupt"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Does the ghosts README name this (map, lap) — in a row that carries the lap
/// time, or in the md5 table by the FILE's md5 prefix beside that time? A file
/// the README does not name is in transition between two of the input arm's
/// writes and must not be rendered or labelled yet.
pub fn readme_names_lap(readme: &str, nn: &str, time: &str, file_md5: &str) -> bool {
    readme_row(readme, nn, time).is_some() || readme.lines().any(|l| l.starts_with(&format!("| {nn} | {}", &file_md5[..8.min(file_md5.len())])) && l.contains(time))
}

#[cfg(test)]
mod transition_tests {
    use super::*;

    /// 24 on 2026-09-10 16:40Z: the README's row moved to 99.529 while the alias
    /// file still held 100.116 (md5 53e390f3…). The file's lap is named by the md5
    /// table → still fine to render; a file the README names nowhere is skipped;
    /// once the alias holds 99.529 the row names it.
    #[test]
    fn a_file_is_rendered_only_when_the_readme_names_its_lap() {
        let readme = "| 24 | 24.Ghost.Gbx | 99.529 | 14 | ship15 | a7ca005a | PPO | x |\n\
\n\
| map | file md5 | time |\n\
| 24 | 53e390f3 | 100.116 |\n";
        assert!(readme_names_lap(readme, "24", "100.116", "53e390f3cc9858e1099b00950d6eebf9"), "the md5 table names the file's lap");
        assert!(readme_names_lap(readme, "24", "99.529", "0123456789abcdef0123456789abcdef"), "the row names the new lap");
        assert!(!readme_names_lap(readme, "24", "100.187", "ffffffffffffffffffffffffffffffff"), "a lap the README does not know");
        assert!(readme_names_lap(readme, "24", "100.116", "ffffffffffffffffffffffffffffffff"), "a row that names the map and the lap counts whatever md5 it shows — the caption comes from the file, the README only has to KNOW the lap");
        assert!(!readme_names_lap(readme, "23", "100.116", "53e390f3cc9858e1099b00950d6eebf9"), "another map");
    }
}

#[cfg(test)]
mod hold_mode_tests {
    use super::*;

    /// holds.tsv's third column: `none` (default) = archive only, `render` =
    /// render + stage, never upload. Both are holds for the shipwatch and the
    /// page; only the render loop tells them apart.
    #[test]
    fn hold_modes_parse_with_none_as_the_default() {
        let t = "# nn\treason\tmode\n21\topening rework\tnone\n22\tawaiting the opening check\trender\n20\topening rework\tRENDER\n07\tjust held\n";
        let modes = parse_hold_modes(t);
        assert_eq!(modes.get("21").map(String::as_str), Some("none"));
        assert_eq!(modes.get("22").map(String::as_str), Some("render"));
        assert_eq!(modes.get("20").map(String::as_str), Some("render"), "case-insensitive");
        assert_eq!(modes.get("07").map(String::as_str), Some("none"), "absent = none");
        let holds = parse_holds(t);
        assert_eq!(holds.get("22").map(String::as_str), Some("awaiting the opening check"));
        assert_eq!(holds.get("07").map(String::as_str), Some("just held"));
        assert_eq!(holds.len(), 4);
    }
}

/// What INPUT's README says about a lap's attitude (the STOP / ATTITUDE
/// summary: `- NN time: below 8 m/s: X s, respawns: N, inverted: Y s, S slow +
/// A attitude intervals`, one line per current certified lap, from README-1211
/// on). `Clean` = no inverted time and no attitude interval (an attitude
/// interval is > 0.3 s of |roll| or |pitch| > 60°). No line for the lap = not
/// clean: the gate fails closed.
#[derive(Debug, PartialEq)]
pub enum Attitude {
    Clean,
    /// Inverted seconds, attitude intervals.
    Dirty { inverted_s: f64, attitude_intervals: u32 },
    /// The lap touches water (the summary line's `water contact: X s`, once
    /// INPUT writes it; the ship15 water lid lets a car ride where the
    /// original's water would stop it). Seconds of contact.
    Water { contact_s: f64 },
    NoTable,
}

impl Attitude {
    pub fn describe(&self) -> String {
        match self {
            Attitude::Clean => "clean".into(),
            Attitude::Dirty { inverted_s, attitude_intervals } => format!("not clean: inverted {inverted_s:.2} s, {attitude_intervals} attitude interval(s) (> 0.3 s of |roll|/|pitch| > 60°)"),
            Attitude::Water { contact_s } => format!("not clean: water contact {contact_s:.2} s (rides the water lid)"),
            Attitude::NoTable => "no attitude table for this lap in the ghosts README (fail closed)".into(),
        }
    }
}

pub fn attitude_verdict(readme: &str, nn: &str, time: &str) -> Attitude {
    let key = format!("- {nn} {time}:");
    let Some(line) = readme.lines().find(|l| l.trim_start().starts_with(&key)) else {
        return Attitude::NoTable;
    };
    let field = |name: &str| -> Option<f64> {
        let i = line.find(name)? + name.len();
        line[i..].trim_start().split(|c: char| c == ' ' || c == ',').next()?.parse().ok()
    };
    let inverted_s = field("inverted:").unwrap_or(f64::NAN);
    // "… N slow + M attitude intervals"
    let attitude_intervals: Option<u32> = line.find("attitude interval").and_then(|i| line[..i].trim_end().rsplit(' ').next()).and_then(|s| s.parse().ok());
    // WATER CONTACT (parent project via the coordinator, 2026-09-10 20:45Z): the
    // moment INPUT's line carries `water contact: X s`, any X > 0 is not clean.
    // A line without the field is judged on attitude alone (today's rows are
    // not failed retroactively); INPUT adds the field to laps certified from
    // then on, and those are held to it.
    // Two water fields once INPUT's census writes them (WATER-CENSUS-1342,
    // 2026-09-10 21:00Z): A = seconds over a deep pool ridden as a lid, B =
    // seconds on a road through water. Either > 0 is not clean; BOTH are
    // required 0.00 for new clips when the line carries them. The field names
    // are matched loosely (`water A:`/`water lid:`/`pool:`, `water B:`/`road
    // water:`), and the older single `water contact:` still counts.
    let water_a = field("water A:").or_else(|| field("water lid:")).or_else(|| field("pool contact:"));
    let water_b = field("water B:").or_else(|| field("road water:")).or_else(|| field("road contact:"));
    let water_total = field("water contact:");
    let contact_s = [water_a, water_b, water_total].into_iter().flatten().fold(0.0_f64, f64::max);
    // AUTHOR-RELATIVE PASS (parent, 2026-09-10 22:20Z): the tilt criterion is
    // judged against the author's own line per section (a wall the author rides
    // at that tilt is legal, up to his duration + 0.3 s); inversions and airborne
    // rotation stay illegal everywhere. INPUT's line carries the verdict as
    // `author-relative: pass` / `author-relative: fail` — when present it decides
    // the ATTITUDE part (the absolute interval count is then informational);
    // inverted time and water are still read. Absent → today's absolute reading.
    let author_relative: Option<bool> = line.find("author-relative:").map(|i| line[i + "author-relative:".len()..].trim_start().to_ascii_lowercase()).and_then(|v| {
        if v.starts_with("pass") { Some(true) } else if v.starts_with("fail") { Some(false) } else { None }
    });
    let attitude_intervals = match author_relative {
        Some(true) => attitude_intervals.map(|_| 0),
        Some(false) => attitude_intervals.map(|a| a.max(1)),
        None => attitude_intervals,
    };
    match (inverted_s.is_nan(), attitude_intervals) {
        (true, _) | (_, None) => Attitude::NoTable,
        (false, Some(a)) if inverted_s <= 0.0 && a == 0 => {
            if contact_s > 0.0 {
                Attitude::Water { contact_s }
            } else {
                Attitude::Clean
            }
        }
        (false, Some(a)) => Attitude::Dirty { inverted_s, attitude_intervals: a },
    }
}

#[cfg(test)]
mod attitude_tests {
    use super::*;

    const README: &str = "STOP / ATTITUDE TABLES (…): summary for the current certified laps:\n\
- 15 48.738: below 8 m/s: 4.09 s, respawns: 0, inverted: 6.38 s, 6 slow + 4 attitude intervals\n\
- 19 46.362: below 8 m/s: 0.07 s, respawns: 0, inverted: 0.00 s, 2 slow + 0 attitude intervals\n\
- 20 75.595: below 8 m/s: 11.03 s, respawns: 0, inverted: 0.23 s, 10 slow + 1 attitude intervals\n\
- 23 102.148: below 8 m/s: 6.32 s, respawns: 2, inverted: 0.00 s, 9 slow + 1 attitude intervals\n\
- 24 99.529: below 8 m/s: 9.07 s, respawns: 0, inverted: 0.00 s, 10 slow + 1 attitude intervals\n";

    /// Clean = no inverted time AND no attitude interval; slow intervals and
    /// respawns do not count; a lap without a line fails closed.
    #[test]
    fn the_attitude_gate_reads_the_readme_summary() {
        assert_eq!(attitude_verdict(README, "19", "46.362"), Attitude::Clean);
        assert_eq!(attitude_verdict(README, "15", "48.738"), Attitude::Dirty { inverted_s: 6.38, attitude_intervals: 4 });
        assert_eq!(attitude_verdict(README, "20", "75.595"), Attitude::Dirty { inverted_s: 0.23, attitude_intervals: 1 });
        assert_eq!(attitude_verdict(README, "24", "99.529"), Attitude::Dirty { inverted_s: 0.0, attitude_intervals: 1 }, "an attitude interval alone is dirty");
        assert_eq!(attitude_verdict(README, "23", "102.148"), Attitude::Dirty { inverted_s: 0.0, attitude_intervals: 1 }, "respawns do not matter; the interval does");
        assert_eq!(attitude_verdict(README, "22", "82.652"), Attitude::NoTable);
        assert_eq!(attitude_verdict(README, "19", "46.445"), Attitude::NoTable, "another lap of the same map");
        assert!(Attitude::NoTable.describe().contains("fail closed"));
        // water contact: a clean-attitude lap with contact is not clean; without the field it is judged on attitude alone
        let with_water = "- 19 46.362: below 8 m/s: 0.07 s, respawns: 0, inverted: 0.00 s, 2 slow + 0 attitude intervals, water contact: 1.20 s\n\
- 05 18.298: below 8 m/s: 0.00 s, respawns: 0, inverted: 0.00 s, 0 slow + 0 attitude intervals, water contact: 0.00 s\n";
        assert_eq!(attitude_verdict(with_water, "19", "46.362"), Attitude::Water { contact_s: 1.2 });
        assert_eq!(attitude_verdict(with_water, "05", "18.298"), Attitude::Clean, "0.00 s of contact is clean");
        assert!(Attitude::Water { contact_s: 1.2 }.describe().contains("water contact 1.20 s"));
        // the census's two fields: A (pool as a lid) and B (road through water) — either > 0 is not clean, both 0.00 is clean
        let ab = "- 20 75.595: below 8 m/s: 11.03 s, respawns: 0, inverted: 0.00 s, 10 slow + 0 attitude intervals, water A: 0.00 s, water B: 0.78 s\n\
- 19 46.362: below 8 m/s: 0.07 s, respawns: 0, inverted: 0.00 s, 2 slow + 0 attitude intervals, water A: 0.00 s, water B: 0.00 s\n\
- 15 48.738: below 8 m/s: 4.09 s, respawns: 0, inverted: 0.00 s, 6 slow + 0 attitude intervals, water A: 12.60 s, water B: 2.60 s\n";
        assert_eq!(attitude_verdict(ab, "20", "75.595"), Attitude::Water { contact_s: 0.78 }, "B alone fails");
        assert_eq!(attitude_verdict(ab, "19", "46.362"), Attitude::Clean, "A = B = 0.00 is clean");
        assert_eq!(attitude_verdict(ab, "15", "48.738"), Attitude::Water { contact_s: 12.6 }, "the larger of A and B is reported");
        // author-relative verdict: pass makes the tilt intervals legal; fail makes them illegal even at 0; inversion stays illegal; absent = absolute reading
        let ar = "- 21 115.478: below 8 m/s: 10.44 s, respawns: 0, inverted: 0.00 s, 14 slow + 11 attitude intervals, author-relative: pass\n\
- 23 102.148: below 8 m/s: 6.32 s, respawns: 2, inverted: 0.00 s, 9 slow + 0 attitude intervals, author-relative: FAIL (wall at 41 s not in the author's line)\n\
- 25 83.772: below 8 m/s: 3.22 s, respawns: 0, inverted: 2.72 s, 8 slow + 2 attitude intervals, author-relative: pass\n";
        assert_eq!(attitude_verdict(ar, "21", "115.478"), Attitude::Clean, "11 wall intervals the author also rides");
        assert_eq!(attitude_verdict(ar, "23", "102.148"), Attitude::Dirty { inverted_s: 0.0, attitude_intervals: 1 }, "an author-relative fail is dirty");
        assert_eq!(attitude_verdict(ar, "25", "83.772"), Attitude::Dirty { inverted_s: 2.72, attitude_intervals: 0 }, "inversion is illegal whatever the author did");
    }
}

/// `<out>/builds.tsv`: `nn<TAB>build<TAB>map_path` — the build a map is
/// rendered on and the map file of that build. Maps not listed use `--build` /
/// `--maps-dir`.
pub fn read_builds(out: &Path) -> std::collections::HashMap<String, (String, String)> {
    parse_builds(&std::fs::read_to_string(out.join("builds.tsv")).unwrap_or_default())
}

pub fn parse_builds(text: &str) -> std::collections::HashMap<String, (String, String)> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').map(str::trim).collect();
            let nn = c[0];
            if nn.len() != 2 || !nn.chars().all(|ch| ch.is_ascii_digit()) || c.len() < 3 || c[1].is_empty() || c[2].is_empty() {
                return None;
            }
            Some((nn.to_string(), (c[1].to_string(), c[2].to_string())))
        })
        .collect()
}

#[cfg(test)]
mod builds_tests {
    use super::*;

    #[test]
    fn builds_tsv_names_a_build_and_a_map_file_per_map() {
        let b = parse_builds("# nn\tbuild\tmap_path\n05\tship16\t/store/incoming/ship16-a3d59cb4/Tiny Summer 2026 - 05.Map.Gbx\n15\tship17\t/store/ship17/Tiny Summer 2026 - 15.Map.Gbx\n07\tship16\n");
        assert_eq!(b.get("05").map(|(b, p)| (b.as_str(), p.as_str())), Some(("ship16", "/store/incoming/ship16-a3d59cb4/Tiny Summer 2026 - 05.Map.Gbx")));
        assert_eq!(b.get("15").map(|(b, _)| b.as_str()), Some("ship17"));
        assert!(b.get("07").is_none(), "a row without a map path is ignored");
    }
}
