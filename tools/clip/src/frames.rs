//! `clip frames` -- still frames out of a finished clip, so somebody can look
//! at it.
//!
//! FILMING.md §6 says "a clip nobody has looked at is not a clip: pull frames
//! back out of the finished file and look at them", and until now there was no
//! tool for it -- every look was a hand-written ffmpeg line, which on this box
//! means remembering that the Windows binary cannot open a WSL path and that a
//! file in the Linux home has to be staged first. So it got skipped, and a
//! re-shoot shipped twice before anyone noticed the camera was under the track.
//!
//! Two things here are not obvious and both are measured:
//!
//! **`-ss` goes BEFORE `-i`, and is followed by an accuracy check.** After the
//! input, ffmpeg decodes every frame from zero to the seek point, which on a
//! 12 s clip is cheap but on a 240 s one is not. Before the input it seeks
//! first -- and modern ffmpeg then decodes forward from the preceding keyframe,
//! so the frame really is the one asked for. That is not something to take on
//! trust at 30 fps: the frame's own presentation timestamp is read back with
//! `-show_entries frame=pkt_pts_time` and a still that landed more than half a
//! frame away from its request is reported, with the time it actually is.
//!
//! **A time past the end silently writes nothing.** ffmpeg exits 0 having
//! produced no file, which reads as success to anything that only checks the
//! status. Every requested still is confirmed to exist and to be non-empty.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::fmt::secs;
use crate::platform::Ff;
use crate::proc::capture;

/// One still: `-ss T -i FILE -frames:v 1 OUT`.
///
/// `-q:v 2` is only read by the mjpeg encoder; a `.png` output ignores it and
/// is lossless, which is what a frame meant for inspection wants.
pub fn ffmpeg_argv(input: &str, at: f64, out: &str) -> Vec<String> {
    vec![
        "-v".into(),
        "error".into(),
        "-y".into(),
        "-ss".into(),
        format!("{at:.3}"),
        "-i".into(),
        input.into(),
        "-frames:v".into(),
        "1".into(),
        out.into(),
    ]
}

/// `1.0,4.5,8` -> the times, in order given. An empty or unparsable entry is a
/// refusal: a typo that silently grabs fewer frames than asked for is how a
/// comparison ends up missing the one instant that mattered.
pub fn parse_times(spec: &str) -> Result<Vec<f64>, String> {
    let mut v = Vec::new();
    for part in spec.split(',') {
        let t = part.trim();
        if t.is_empty() {
            return Err(format!("--at {spec:?} has an empty entry"));
        }
        let n: f64 = t.parse().map_err(|e| format!("--at {t:?}: {e}"))?;
        if n < 0.0 {
            return Err(format!("--at {t:?} is before the start"));
        }
        v.push(n);
    }
    if v.is_empty() {
        return Err("--at needs at least one time".into());
    }
    Ok(v)
}

/// Evenly spaced times across a clip, endpoints included, for `--every`/`-n`
/// style asks. `n = 1` is the midpoint rather than the first frame, because one
/// frame of a run is more useful from the middle than from the countdown.
pub fn spread(duration: f64, n: usize) -> Vec<f64> {
    match n {
        0 => Vec::new(),
        1 => vec![duration / 2.0],
        _ => (0..n)
            .map(|i| duration * i as f64 / (n - 1) as f64)
            // The very last frame of a file is a coin toss; step just inside it.
            .map(|t| if t >= duration { (duration - 0.05).max(0.0) } else { t })
            .collect(),
    }
}

/// A still's filename: `<prefix>t<seconds, ms, underscored>.png`, so a
/// directory of them sorts in time order and says what it is without a manifest.
pub fn still_name(prefix: &str, at: f64) -> String {
    format!("{prefix}t{:07.3}.png", at).replace('.', "_").replace("_png", ".png")
}

pub struct Opts {
    pub at: Vec<f64>,
    pub count: Option<usize>,
    pub prefix: String,
}

pub fn run(ff: &Ff, input: &Path, outdir: &Path, o: &Opts) -> Result<(), String> {
    let dur = ff.probe_duration(input)?;
    let times = if let Some(n) = o.count { spread(dur, n) } else { o.at.clone() };
    if times.is_empty() {
        return Err("nothing to grab: pass --at T,T,... or -n N".into());
    }
    std::fs::create_dir_all(outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;

    println!("frames: {} of {}s", times.len(), secs(dur));
    let mut made: Vec<PathBuf> = Vec::new();
    for &at in &times {
        if at > dur {
            return Err(format!(
                "asked for a frame at {}s of a {}s clip",
                secs(at),
                secs(dur)
            ));
        }
        let out = outdir.join(still_name(&o.prefix, at));
        let args = ffmpeg_argv(&ff.arg_path(input)?, at, &ff.arg_path(&out)?);
        let r = capture(Command::new(&ff.ffmpeg).args(&args))?;
        if !r.ok() {
            return Err(format!("ffmpeg failed at {}s: {}", secs(at), r.why()));
        }
        // A seek past the end exits 0 and writes nothing.
        let bytes = crate::proc::filesize(&out).map_err(|e| {
            format!("no still came out at {}s ({e}) -- ffmpeg reported no error", secs(at))
        })?;
        if bytes == 0 {
            return Err(format!("the still at {}s is empty", secs(at)));
        }
        // WHERE THE FRAME ACTUALLY IS. A seek is not a promise.
        let landed = frame_time(ff, input, at);
        let note = match landed {
            Some(t) if (t - at).abs() > 0.017 => format!("  (the frame is at {}s)", secs(t)),
            _ => String::new(),
        };
        println!("  {}s  {bytes} B  {}{note}", secs(at), out.display());
        made.push(out);
    }
    println!("frames: {} still(s) in {}", made.len(), outdir.display());
    Ok(())
}

/// The presentation timestamp of the frame a seek to `at` actually lands on.
/// `None` when this ffprobe does not report one -- the check is a courtesy, not
/// a gate, and an old build should not fail a good grab.
fn frame_time(ff: &Ff, input: &Path, at: f64) -> Option<f64> {
    let path = ff.arg_path(input).ok()?;
    let out = capture(Command::new(&ff.ffprobe).args([
        "-v",
        "error",
        "-ss",
        &format!("{at:.3}"),
        "-i",
        &path,
        "-select_streams",
        "v:0",
        "-frames:v",
        "1",
        "-show_entries",
        "frame=best_effort_timestamp_time",
        "-of",
        "csv=p=0",
    ]))
    .ok()?;
    out.stdout.trim().lines().next()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seek_is_an_input_option() {
        let a = ffmpeg_argv("in.mp4", 4.25, "out.png");
        let i = a.iter().position(|x| x == "-i").unwrap();
        let s = a.iter().position(|x| x == "-ss").unwrap();
        assert!(s < i, "-ss before -i, or a long clip decodes from zero");
        assert_eq!(a[s + 1], "4.250");
        assert_eq!(a[a.iter().position(|x| x == "-frames:v").unwrap() + 1], "1");
    }

    #[test]
    fn times_parse_or_refuse() {
        assert_eq!(parse_times("1,4.5,8").unwrap(), vec![1.0, 4.5, 8.0]);
        assert_eq!(parse_times(" 2.5 ").unwrap(), vec![2.5]);
        assert!(parse_times("1,,2").is_err());
        assert!(parse_times("1,x").is_err());
        assert!(parse_times("-1").is_err());
    }

    #[test]
    fn a_spread_covers_both_ends() {
        let v = spread(10.0, 5);
        assert_eq!(v.len(), 5);
        assert_eq!(v[0], 0.0);
        assert!((v[2] - 5.0).abs() < 1e-9);
        // Just inside the end, never on it.
        assert!(v[4] < 10.0 && v[4] > 9.9, "{v:?}");
        assert_eq!(spread(10.0, 1), vec![5.0]);
        assert!(spread(10.0, 0).is_empty());
    }

    #[test]
    fn a_stills_name_sorts_in_time_order() {
        assert_eq!(still_name("u01_", 4.25), "u01_t004_250.png");
        assert_eq!(still_name("", 12.0), "t012_000.png");
        let mut v = [still_name("", 10.0), still_name("", 2.0), still_name("", 1.5)];
        v.sort();
        assert_eq!(v, [still_name("", 1.5), still_name("", 2.0), still_name("", 10.0)]);
    }
}
