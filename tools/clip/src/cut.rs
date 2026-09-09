//! `clip cut` -- the game's `.webm` into the published `.mp4`, at the length
//! the run actually is, **with the run's own controls drawn on it.**
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
//!
//! **And since 2026-09-09 the cut IS the overlay.** Every published clip
//! carries the controls overlay (vjeux: "add the controls overlay on the videos
//! you generate" — "do not make it a rule, change the renderer to do it by
//! default"). So `cut` takes the run's ghost and draws its inputs in the same
//! ffmpeg pass that trims and re-encodes (`clip overlay` does the drawing; this
//! is its front door for a fresh render), settles the video↔tape offset by
//! MEASURING it against the picture ([`crate::sync`]) and refuses when the
//! measurement is unsound or off the expected value, and stamps the file with
//! the marker `clip ship` checks. A bare cut needs `--no-overlay`, spelled
//! out, and says in capitals what it is making.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::fmt::secs;
use crate::overlay::{self, Marker, Timing};
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
/// `crf` 19 unless the caller says otherwise (`--crf`): a 105 s lap at 19 is
/// over the 100 MB the inline player takes, and +5 is roughly -40 % of the bytes.
pub fn ffmpeg_argv(input: &str, to: f64, out: &str) -> Vec<String> {
    ffmpeg_argv_crf(input, to, out, 19)
}

pub fn ffmpeg_argv_crf(input: &str, to: f64, out: &str, crf: u32) -> Vec<String> {
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
        crf.to_string(),
        "-preset".into(),
        "medium".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-an".into(),
        out.into(),
    ]
}

/// What a cut is asked for.
pub struct CutOpts {
    pub to: Option<f64>,
    pub crf: u32,
    /// The run's ghost: the overlay is drawn from its input chunk. `None` is
    /// only legal with `bare`.
    pub ghost: Option<PathBuf>,
    /// `--offset-ms N`: force the video↔tape offset instead of measuring it.
    pub offset_ms: Option<i64>,
    /// `--nominal-ms N`: the offset the pipeline expects and draws at (its clips
    /// start at race 0, so 0); the picture check is made against it.
    pub nominal_ms: i64,
    /// `--no-overlay`: the old bare cut. Says so in capitals.
    pub bare: bool,
}

impl Default for CutOpts {
    fn default() -> Self {
        CutOpts { to: None, crf: 19, ghost: None, offset_ms: None, nominal_ms: 0, bare: false }
    }
}

/// The decision a cut makes before it touches ffmpeg: overlay from this ghost,
/// a bare cut because the caller said so, or a refusal. Split out so it can be
/// tested without a video.
pub fn decide(o: &CutOpts) -> Result<Option<(PathBuf, Timing)>, String> {
    match (&o.ghost, o.bare) {
        (Some(g), false) => {
            let timing = match o.offset_ms {
                Some(ms) => Timing::Given(ms),
                None => Timing::Checked(o.nominal_ms),
            };
            Ok(Some((g.clone(), timing)))
        }
        (Some(_), true) => Err("cut: --ghost and --no-overlay together — decide which".into()),
        (None, true) => Ok(None),
        (None, false) => Err(
            "cut: no --ghost. EVERY PUBLISHED CLIP CARRIES THE CONTROLS OVERLAY (vjeux, 2026-09-09), so a cut \
             draws the run's inputs from its ghost: `clip cut <in.webm> <out.mp4> --ghost <run.Ghost.Gbx>`. \
             A bare cut is `--no-overlay`, and `clip ship` will refuse the result."
                .into(),
        ),
    }
}

pub fn run(ff: &Ff, input: &Path, out: &Path, to: Option<f64>) -> Result<(), String> {
    run_crf(ff, input, out, to, 19)
}

/// The bare cut. Kept for the callers that know what they are doing
/// (`--no-overlay`); the pipeline goes through [`run_opts`].
pub fn run_crf(ff: &Ff, input: &Path, out: &Path, to: Option<f64>, crf: u32) -> Result<(), String> {
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

    let args = ffmpeg_argv_crf(&ff.arg_path(input)?, to, &ff.arg_path(out)?, crf);
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

/// The cut the pipeline makes: overlaid, timing-checked, marked — or, with
/// `--no-overlay`, bare and loud about it. Returns the marker written (`None`
/// for a bare cut).
pub fn run_opts(ff: &Ff, input: &Path, out: &Path, o: &CutOpts) -> Result<Option<Marker>, String> {
    match decide(o)? {
        None => {
            println!("CUTTING A BARE CLIP (--no-overlay): NO CONTROLS OVERLAY — `clip ship` will refuse it without --no-overlay");
            run_crf(ff, input, out, o.to, o.crf).map(|_| None)
        }
        Some((ghost, timing)) => {
            let din = ff.probe_duration(input)?;
            let to = o.to.unwrap_or(din);
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
            println!("cut: {}s -> {}s, with the controls overlay from {}", secs(din), secs(to), ghost.display());
            let oo = overlay::Opts { to: Some(to), crf: o.crf, ..overlay::Opts::default() };
            overlay::run(ff, &ghost, input, out, &oo, &timing).map(Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_length_is_an_output_option_not_an_input_seek() {
        let a = ffmpeg_argv("in.webm", 219.812, "out.mp4");
        let i = a.iter().position(|x| x == "-i").unwrap();
        let t = a.iter().position(|x| x == "-t").unwrap();
        assert!(
            t > i,
            "-t must come after -i or the cut snaps to a keyframe"
        );
        assert_eq!(a[t + 1], "219.812");
    }

    #[test]
    fn the_encode_matches_the_split_screen_path() {
        let a = ffmpeg_argv("in.webm", 10.0, "out.mp4");
        for want in ["libx264", "19", "yuv420p", "-an"] {
            assert!(a.iter().any(|x| x == want), "missing {want}");
        }
    }

    /// THE DEFAULT IS THE OVERLAY, AND A BARE CUT MUST BE ASKED FOR BY NAME.
    /// This is the whole point of the 2026-09-09 change: a clip cannot come
    /// out bare because someone forgot a step.
    #[test]
    fn a_cut_without_a_ghost_is_refused_unless_bare_is_spelled_out() {
        let e = decide(&CutOpts::default()).unwrap_err();
        assert!(e.contains("CONTROLS OVERLAY"), "{e}");
        assert!(e.contains("--no-overlay"), "{e}");
        assert!(decide(&CutOpts { bare: true, ..CutOpts::default() }).unwrap().is_none());
        let both = CutOpts { bare: true, ghost: Some(PathBuf::from("g.Ghost.Gbx")), ..CutOpts::default() };
        assert!(decide(&both).is_err(), "ghost + bare is a contradiction");
    }

    /// With a ghost, the timing is CHECKED against the picture unless an offset is forced.
    #[test]
    fn the_offset_is_measured_by_default_and_forced_only_on_request() {
        let g = Some(PathBuf::from("g.Ghost.Gbx"));
        let (_, t) = decide(&CutOpts { ghost: g.clone(), ..CutOpts::default() }).unwrap().unwrap();
        assert!(matches!(t, Timing::Checked(0)));
        let (_, t) = decide(&CutOpts { ghost: g, offset_ms: Some(-40), ..CutOpts::default() }).unwrap().unwrap();
        assert!(matches!(t, Timing::Given(-40)));
    }
}
