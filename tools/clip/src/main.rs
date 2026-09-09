//! `clip` -- publish  rendered run, or put two of them side by side.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clip::{cut, frames, inventory, overlay, platform, ship, split, sync};

const USAGE: &str = "\
clip ship  <file.mp4> <map-dir> [release-asset-name] [--no-mirror] [--no-overlay]
    Publish one clip so a LOGGED-OUT visitor can watch it: settle and probe the
    file, REQUIRE THE CONTROLS-OVERLAY MARKER (every published clip carries the
    overlay; --no-overlay ships a bare clip and says so), upload it to the
    release, upload it to user-attachments, register the URL in the release
    body (this is what makes it public), then fetch it back with no credential
    and require 200 and playable bytes. Refuses at every step.

clip cut   <in.webm> <out.mp4> --ghost <run.Ghost.Gbx> [--to SECONDS] [--crf Q]
                                [--offset-ms N | --nominal-ms N --tolerance-ms N]
           <in.webm> <out.mp4> --no-overlay [--to SECONDS] [--crf Q]
    The game's VP8/WebM into the mp4 `ship` takes, cut to the length the run
    actually is, WITH THE RUN'S CONTROLS DRAWN ON IT (the default since
    2026-09-09: steering, throttle, brake, respawn, and the strip of them over
    time). The video<->tape offset is MEASURED against the picture (`clip sync`)
    and the cut refuses when the fit is unsound or off --nominal-ms (0) by more
    than --tolerance-ms (100); --offset-ms N forces it after a frame check. The
    file is stamped with the marker `ship` checks. --no-overlay is the old bare
    cut, spelled out. The output is probed, not assumed.

clip overlay <ghost.Gbx> <in.mp4> <out.mp4> [--to S] [--offset-ms N | --sync] [--fps F] [--crf Q]
                                           [--history-ms N] [--future-ms N]
    Draw a run's own inputs -- steering, throttle, brake, respawn, and a strip
    of them over time -- onto a finished clip. NOW IS IN THE MIDDLE of that
    strip: `--history-ms` behind the playhead and `--future-ms` ahead (3000 and
    3000 by default), so an input is visible arriving BEFORE the thing it
    causes, which is the only way the timing of a TAS is legible. The future
    half is drawn dimmer. Reads the 10 ms input chunk (what the driver
    pressed), never the 50 ms telemetry echo (what the car had, and on a
    synthesised tape whoever drove the carrier). Draws its own glyphs, so it
    needs no drawtext and no font. --sync measures the offset from the picture
    instead of taking --offset-ms (0); the output carries the overlay marker.

clip sync <ghost.Gbx> <video> [--span-ms N] [--nominal-ms N] [--tolerance-ms N]
    MEASURE the offset between a render and the tape it plays: the picture's
    frame-to-frame change against the telemetry's speed, rank-correlated over
    every lag in +-span (1500 ms). Prints the best lag, its correlation, and the
    peak's width; with --nominal-ms/--tolerance-ms also passes or refuses the
    way `cut` does.

clip panel <ghost.Gbx> <out.png> --at S [--history-ms N] [--future-ms N]
    One overlay panel at one race time, as a PNG, with no video render. Looking
    at a change to the panel used to mean re-encoding a whole clip first, so it
    got skipped -- and it is the only way to see the panel at a race time the
    clip does not reach.

clip alignment <ghost.Gbx> [--span-ms N]
    Fit the constant lag between a ghost's two steering channels. They describe
    one run, so they agree at exactly one shift -- which makes overlay timing a
    measurement rather than something to eyeball against a frame.

clip frames <in.mp4> <outdir> [--at T,T,...] [-n N] [--prefix P] [--stream] [--thumb W]
    Still frames out of a finished clip, because FILMING.md rule 6 says look at
    what you made and there was no tool for it. --at names the instants (the
    ones the telemetry says something should be happening); -n N spreads N
    stills across the whole clip. Each still is confirmed non-empty and its real
    timestamp read back, since a seek past the end writes nothing and exits 0.

clip split <left.mp4> <right.mp4> <left-label> <right-label> <out.mp4>
    Two runs side by side, for maps where a chase camera provably cannot hold
    both cars. The shorter run holds its final frame so the gap reads as time.

clip inventory [--root D] [--tsv] [--probe] [--probe-all] [--verify [--store D] [--markdown]]
    What is published, per map, read off the pages: the map's NAME, its headline
    caption, how many videos it carries, and WHICH TREATMENT its clip used --
    two-car, single-car or split. A map with no video plans two-car. Nothing is
    estimated: a page that does not say what its scene contained reads UNKNOWN,
    which is a page to read rather than a default to apply.
    --probe measures the clips the page is silent about; --probe-all measures
    every published clip, including the ones the prose answers, and shouts a
    DISAGREES where the two do not match (prose about a withdrawn clip reads as
    the surviving one's treatment).

Environment:
    CLIP_PLATFORM   native | wsl          (default: native if ffmpeg is on PATH)
    CLIP_FFMPEG CLIP_FFPROBE CLIP_WINFF_BIN CLIP_STAGE_DIR CLIP_FONT
    REPO RELEASE GHVID CLIP_GH CLIP_CURL CLIP_PROXY (the gate's forward proxy, e.g. http://fwdproxy:8080 on a devserver)
";

fn main() -> ExitCode {
    // --version / -V. Compile-time only: CARGO_PKG_* come from the crate's
    // Cargo.toml (which inherits the one workspace version), and TAS_BUILD is
    // the git hash the release build sets. option_env! means an ordinary
    // `cargo build` still works and simply reports "dev". No dependency.
    if std::env::args().any(|x| x == "--version" || x == "-V") {
        println!(
            "{} {} ({})",
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            env!("CARGO_PKG_VERSION"),
            option_env!("TAS_BUILD").unwrap_or("dev")
        );
        std::process::exit(0);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    match go(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("clip: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `--key value`, as a string.
fn val<'a>(args: &'a [String], k: &str) -> Option<&'a String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1))
}

/// `--key value`, parsed, with the flag's name in the complaint.
fn num<T: std::str::FromStr>(args: &[String], k: &str) -> Result<Option<T>, String>
where
    T::Err: std::fmt::Display,
{
    val(args, k).map(|v| v.parse::<T>().map_err(|e| format!("{k}: {e}"))).transpose()
}

fn go(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first().map(String::as_str) else {
        return Err(format!("no subcommand\n\n{USAGE}"));
    };
    match cmd {
        "ship" => {
            let pos: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
            let (file, mapdir) = match pos.len() {
                2 | 3 => (pos[0], pos[1]),
                _ => return Err(format!("usage:\n{USAGE}")),
            };
            let ff = platform::from_env()?;
            let mut cfg = ship::Cfg::from_env();
            if args.iter().any(|a| a == "--no-mirror") {
                cfg.mirror = false;
            }
            if args.iter().any(|a| a == "--no-overlay") {
                cfg.allow_bare = true;
            }
            ship::run(
                &ff,
                &cfg,
                Path::new(file),
                Path::new(mapdir),
                pos.get(2).map(|s| s.as_str()),
            )
        }
        "cut" => {
            // positionals: in, out -- the flags carry values, so skip those too
            let mut pos: Vec<&String> = Vec::new();
            let mut i = 1;
            while i < args.len() {
                let a = &args[i];
                if a == "--no-overlay" {
                    i += 1;
                } else if a.starts_with("--") {
                    i += 2;
                } else {
                    pos.push(a);
                    i += 1;
                }
            }
            if pos.len() != 2 {
                return Err(format!("usage:\n{USAGE}"));
            }
            let d = cut::CutOpts::default();
            let o = cut::CutOpts {
                to: num::<f64>(args, "--to")?,
                crf: num::<u32>(args, "--crf")?.unwrap_or(d.crf),
                ghost: val(args, "--ghost").map(PathBuf::from),
                offset_ms: num::<i64>(args, "--offset-ms")?,
                nominal_ms: num::<i64>(args, "--nominal-ms")?.unwrap_or(d.nominal_ms),
                bare: args.iter().any(|a| a == "--no-overlay"),
            };
            let ff = platform::from_env()?;
            cut::run_opts(&ff, Path::new(pos[0]), Path::new(pos[1]), &o).map(|_| ())
        }
        "frames" => {
            if args.len() < 3 {
                return Err(format!("usage:\n{USAGE}"));
            }
            let at = match val(args, "--at") {
                Some(s) => frames::parse_times(s)?,
                None => Vec::new(),
            };
            let count = match val(args, "-n").or_else(|| val(args, "--count")) {
                Some(s) => Some(s.parse::<usize>().map_err(|e| format!("-n: {e}"))?),
                None => None,
            };
            if at.is_empty() && count.is_none() {
                return Err("frames: pass --at T,T,... or -n N".into());
            }
            if !at.is_empty() && count.is_some() {
                return Err("frames: --at and -n both name the instants; pass one".into());
            }
            let o = frames::Opts {
                at,
                count,
                prefix: val(args, "--prefix").cloned().unwrap_or_default(),
                stream: args.iter().any(|a| a == "--stream"),
                thumb: num::<u32>(args, "--thumb")?,
            };
            let ff = platform::from_env()?;
            frames::run(&ff, Path::new(&args[1]), Path::new(&args[2]), &o)
        }
        "panel" => {
            if args.len() < 3 {
                return Err(format!("usage:\n{USAGE}"));
            }
            let mut o = overlay::Opts::default();
            if let Some(v) = num::<i64>(args, "--history-ms")? {
                o.history_ms = v;
            }
            if let Some(v) = num::<i64>(args, "--future-ms")? {
                o.future_ms = v;
            }
            let at: f64 = num::<f64>(args, "--at")?.ok_or("clip panel needs --at S (the race time to draw)")?;
            let ff = platform::from_env()?;
            overlay::panel_png(
                &ff,
                Path::new(&args[1]),
                (at * 1000.0).round() as i64,
                Path::new(&args[2]),
                &o,
            )
        }
        "overlay" => {
            if args.len() < 4 {
                return Err(format!("usage:\n{USAGE}"));
            }
            let mut o = overlay::Opts::default();
            if let Some(v) = num::<f64>(args, "--to")? {
                o.to = Some(v);
            }
            if let Some(v) = num::<f64>(args, "--fps")? {
                o.fps = v;
            }
            if let Some(v) = num::<i64>(args, "--history-ms")? {
                o.history_ms = v;
            }
            if let Some(v) = num::<i64>(args, "--future-ms")? {
                o.future_ms = v;
            }
            if let Some(v) = num::<u32>(args, "--crf")? {
                o.crf = v;
            }
            let offset = num::<i64>(args, "--offset-ms")?;
            let timing = match (offset, args.iter().any(|a| a == "--sync")) {
                (Some(_), true) => return Err("overlay: --offset-ms and --sync together — decide which".into()),
                (Some(ms), false) => overlay::Timing::Given(ms),
                (None, true) => overlay::Timing::Checked(num::<i64>(args, "--nominal-ms")?.unwrap_or(0)),
                (None, false) => overlay::Timing::Given(0),
            };
            let ff = platform::from_env()?;
            overlay::run(&ff, Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3]), &o, &timing).map(|_| ())
        }
        "sync" => {
            if args.len() < 3 {
                return Err(format!("usage:\n{USAGE}"));
            }
            let span = num::<i64>(args, "--span-ms")?.unwrap_or(1500);
            let ff = platform::from_env()?;
            if args.iter().any(|a| a == "--all") {
                return sync::run_all(&ff, Path::new(&args[1]), Path::new(&args[2]), span);
            }
            let f = sync::run(&ff, Path::new(&args[1]), Path::new(&args[2]), span)?;
            if let Some(nominal) = num::<i64>(args, "--nominal-ms")? {
                f.check(nominal)?;
                println!("sync: PASS -- the picture follows the tape where a clip at offset {nominal:+} ms should");
            }
            Ok(())
        }
        "alignment" => {
            if args.len() < 2 {
                return Err(format!("usage:\n{USAGE}"));
            }
            let span: i64 = num::<i64>(args, "--span-ms")?.unwrap_or(200);
            let (lag, at_best, at_zero) = overlay::alignment(&args[1], span)?;
            println!("alignment: best lag {lag:+} ms (disagreement {at_best:.2}), lag 0 {at_zero:.2}");
            if lag == 0 {
                println!("  the two channels agree at lag 0 -- an overlay drawn at race time is in time.");
            } else {
                println!(
                    "  the input chunk leads the telemetry echo by {lag} ms on this file. That is a \
                     property of the RUN, not of the overlay; pass --offset-ms {lag} if a frame \
                     check disagrees."
                );
            }
            Ok(())
        }
        "split" => {
            if args.len() != 6 {
                return Err(format!("usage:\n{USAGE}"));
            }
            let ff = platform::from_env()?;
            split::run(
                &ff,
                Path::new(&args[1]),
                Path::new(&args[2]),
                &args[3],
                &args[4],
                Path::new(&args[5]),
            )
        }
        "inventory" => inventory::main(&args[1..]),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown subcommand {other:?}\n\n{USAGE}")),
    }
}
