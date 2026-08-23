//! `clip cut` -- the game's `.webm` into the published `.mp4`, at the length
//! the run actually is.
//!
//! Two facts about what comes out of the MediaTracker, both measured
//! 2026-08-22 on 286279:
//!
//! **The clip is as long as the LONGEST ghost in the scene, not as long as our
//! run.** A 218.812 s TAS filmed against a 441.002 s human record renders
//! **441 s** of video — 1059 s of wall clock, more than half of it a camera
//! bolted to a car that has already finished. Trimming the opponent ghost
//! before staging (`ghost trim OPP --to <finish+1s>`) is the cheap fix and it
//! is the one to reach for; this is what fixes the ones already rendered.
//!
//! **And the output is VP8 in WebM**, which `clip ship` does not take: the
//! publish path wants an mp4, because that is what the inline player on the
//! release page will play.
//!
//! So: one pass, `-t` for the length and x264 for the codec, at the same
//! crf 19 / yuv420p the split-screen path uses, so a cut clip and a split clip
//! are the same encode. The output is probed afterwards rather than assumed —
//! FILMING.md §6 — and a duration that does not match what was asked for is an
//! error, not a note.

use std::path::Path;
use std::process::Command;

use crate::fmt::secs;
use crate::platform::Ff;
use crate::proc::capture;

/// x264 at crf 19, cut to `to` seconds. `-an` for the same reason as `split`:
/// a rendered run's audio track is nothing, and a silent track upsets the
/// inline player.
///
/// `-t` after `-i` cuts the OUTPUT, so the frames are decoded and re-encoded
/// and the cut lands exactly on the second asked for. `-t` before `-i` would
/// seek the input and land on the nearest keyframe instead, which on a 441 s
/// VP8 is up to several seconds early.
pub fn ffmpeg_argv(input: &str, to: f64, out: &str) -> Vec<String> {
    vec![
        "-v".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        input.into(),
        "-t".into(),
        format!("{to:.3}"),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        "19".into(),
        "-preset".into(),
        "medium".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-an".into(),
        out.into(),
    ]
}

pub fn run(ff: &Ff, input: &Path, out: &Path, to: Option<f64>) -> Result<(), String> {
    let din = ff.probe_duration(input)?;
    let to = to.unwrap_or(din);
    if to <= 0.0 {
        return Err(format!("--to {to} is not a length"));
    }
    if to > din + 0.5 {
        return Err(format!(
            "asked to cut to {}s from a {}s file -- `cut` only shortens, and a clip that \
             is shorter than the run means the RENDER was short, which is a defect in the \
             recording rather than something to paper over here",
            secs(to),
            secs(din)
        ));
    }
    println!("cut: {}s -> {}s", secs(din), secs(to));

    let args = ffmpeg_argv(&ff.arg_path(input)?, to, &ff.arg_path(out)?);
    let mut c = Command::new(&ff.ffmpeg);
    c.args(&args);
    let r = capture(&mut c)?;
    if !r.ok() {
        return Err(format!("ffmpeg failed: {}", r.why()));
    }

    // Look at what you made. A re-encode that silently produced 0.04 s of
    // video passes "the file exists" and passes "it is an mp4".
    let dout = ff.probe_duration(out)?;
    if (dout - to).abs() > 1.0 {
        return Err(format!(
            "asked for {}s and the output is {}s",
            secs(to),
            secs(dout)
        ));
    }
    let bytes = crate::proc::filesize(out)?;
    println!("cut: {}s {bytes} bytes -> {}", secs(dout), out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_length_is_an_output_option_not_an_input_seek() {
        let a = ffmpeg_argv("in.webm", 219.812, "out.mp4");
        let i = a.iter().position(|x| x == "-i").unwrap();
        let t = a.iter().position(|x| x == "-t").unwrap();
        assert!(t > i, "-t must come after -i or the cut snaps to a keyframe");
        assert_eq!(a[t + 1], "219.812");
    }

    #[test]
    fn the_encode_matches_the_split_screen_path() {
        let a = ffmpeg_argv("in.webm", 10.0, "out.mp4");
        for want in ["libx264", "19", "yuv420p", "-an"] {
            assert!(a.iter().any(|x| x == want), "missing {want}");
        }
    }
}
