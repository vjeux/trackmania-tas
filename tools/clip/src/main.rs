//! `clip` -- publish a rendered run, or put two of them side by side.

use std::path::Path;
use std::process::ExitCode;

use clip::{cut, platform, ship, split};

const USAGE: &str = "\
clip ship  <file.mp4> <map-dir> [release-asset-name]
    Publish one clip so a LOGGED-OUT visitor can watch it: settle and probe the
    file, upload it to the release, upload it to user-attachments, register the
    URL in the release body (this is what makes it public), then fetch it back
    with no credential and require 200 and playable bytes. Refuses at every step.

clip cut   <in.webm> <out.mp4> [--to SECONDS]
    The game's VP8/WebM into the mp4 `ship` takes, cut to the length the run
    actually is. The MediaTracker renders the LONGEST ghost in the scene, so a
    218.812 run filmed against a 441.002 human record comes out 441 s long.
    Trimming the opponent ghost before staging is the cheaper fix; this is for
    the ones already rendered. The output is probed, not assumed.

clip split <left.mp4> <right.mp4> <left-label> <right-label> <out.mp4>
    Two runs side by side, for maps where a chase camera provably cannot hold
    both cars. The shorter run holds its final frame so the gap reads as time.

Environment:
    CLIP_PLATFORM   native | wsl          (default: native if ffmpeg is on PATH)
    CLIP_FFMPEG CLIP_FFPROBE CLIP_WINFF_BIN CLIP_STAGE_DIR CLIP_FONT
    REPO RELEASE GHVID CLIP_GH CLIP_CURL
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match go(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("clip: {e}");
            ExitCode::FAILURE
        }
    }
}

fn go(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first().map(String::as_str) else {
        return Err(format!("no subcommand\n\n{USAGE}"));
    };
    match cmd {
        "ship" => {
            let (file, mapdir) = match args.len() {
                3 | 4 => (&args[1], &args[2]),
                _ => return Err(format!("usage:\n{USAGE}")),
            };
            let ff = platform::from_env()?;
            let cfg = ship::Cfg::from_env();
            ship::run(
                &ff,
                &cfg,
                Path::new(file),
                Path::new(mapdir),
                args.get(3).map(String::as_str),
            )
        }
        "cut" => {
            if args.len() < 3 {
                return Err(format!("usage:\n{USAGE}"));
            }
            let to = args
                .iter()
                .position(|a| a == "--to")
                .and_then(|i| args.get(i + 1))
                .map(|v| v.parse::<f64>().map_err(|e| format!("--to: {e}")))
                .transpose()?;
            let ff = platform::from_env()?;
            cut::run(&ff, Path::new(&args[1]), Path::new(&args[2]), to)
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
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown subcommand {other:?}\n\n{USAGE}")),
    }
}
