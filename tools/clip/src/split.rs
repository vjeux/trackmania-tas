//! `clip split` -- two runs side by side.
//!
//! WHY THIS EXISTS
//! A two-car MediaTracker clip only works while both cars stay near the camera.
//! On 276877 the human record is 6.061 s slower and 61.5 m away, and on 228607
//! it is 4.605 s slower and 356.68 m away: the opponent is behind the camera
//! for the entire run, so the "comparison" is one car and a caption that lies.
//! Rendering each run on its own and putting them side by side shows what the
//! other driver actually does. Distance is the test, and it is measured, not
//! judged (FILMING.md §2) -- this command is only for maps that fail it.
//!
//! Both inputs start at the race start (t=0), so they align with no offset.
//! Every flag below is here for a reason:
//!
//!   * `tpad=stop_mode=clone` + `trim=duration=<longer>` -- THE SHORTER RUN IS
//!     HELD ON ITS FINAL FRAME until the longer one finishes. The fast car
//!     parks at the flag while the slow car is still out on the track, so the
//!     gap reads as TIME. A half-screen going black would read as a broken
//!     video instead.
//!   * `drawtext` on each half -- the labels are not decoration. This command
//!     exists precisely because the two cars could not share a frame, so
//!     nothing in the picture says which driver is which. An unlabelled split
//!     screen is the same lie in a different shape, which is why a box with no
//!     usable font REFUSES here instead of encoding without them. (The Mac's
//!     ffmpeg is built without libfreetype, so `drawtext` is unavailable there
//!     at any font path -- that is what the render box is for.)

use std::path::Path;
use std::process::Command;

use crate::fmt::secs;
use crate::platform::Ff;
use crate::proc::capture;

/// One half of the picture.
pub const HALF_WIDTH: u32 = 960;
/// How long a final frame may be held. Longer than any gap this project films.
pub const HOLD_SECONDS: u32 = 60;

/// `drawtext`'s own escaping: it splits options on `:`, ends the text at an
/// unescaped `'`, and reads `%` as a strftime expansion.
///
/// The shell version escaped nothing and simply produced a broken filtergraph
/// for any label with a colon in it -- driver nicknames are arbitrary strings
/// off a leaderboard, so that was a matter of time.
pub fn escape_drawtext(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            ':' => out.push_str(r"\:"),
            '\'' => out.push_str(r"\'"),
            '%' => out.push_str(r"\%"),
            _ => out.push(c),
        }
    }
    out
}

/// The whole filtergraph: scale both halves, hold each on its last frame to the
/// length of the longer run, label them, stack them.
pub fn filtergraph(longest: f64, left_label: &str, right_label: &str, font: &str) -> String {
    let d = secs(longest);
    let half = |input: usize, label: &str, out: &str| {
        format!(
            "[{input}:v]scale={HALF_WIDTH}:-2,tpad=stop_mode=clone:stop_duration={HOLD_SECONDS},\
trim=duration={d},setpts=PTS-STARTPTS,\
drawtext=fontfile='{font}':text='{}':x=18:y=14:fontsize=28:fontcolor=white:\
box=1:boxcolor=black@0.6:boxborderw=9[{out}]",
            escape_drawtext(label)
        )
    };
    format!(
        "{};{};[l][r]hstack=inputs=2[v]",
        half(0, left_label, "l"),
        half(1, right_label, "r")
    )
}

/// x264 at crf 19: the published clips' encode. `-an` because a rendered run
/// carries no audio worth keeping and a silent track upsets the inline player.
pub fn ffmpeg_argv(left: &str, right: &str, graph: &str, out: &str) -> Vec<String> {
    vec![
        "-v".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        left.into(),
        "-i".into(),
        right.into(),
        "-filter_complex".into(),
        graph.into(),
        "-map".into(),
        "[v]".into(),
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

pub fn run(
    ff: &Ff,
    left: &Path,
    right: &Path,
    left_label: &str,
    right_label: &str,
    out: &Path,
) -> Result<(), String> {
    let font = ff.font.clone().ok_or_else(|| {
        "no drawtext font on this box: the halves would be unlabelled, and an unlabelled \
         split screen says nothing about which driver is which. Set CLIP_FONT to a TTF \
         (and note the Mac's ffmpeg has no libfreetype, so drawtext is unavailable there)."
            .to_string()
    })?;

    if !ff.has_drawtext() {
        return Err(format!(
            "{} is built without libfreetype, so it has no drawtext filter and the halves \
             could not be labelled. That is what the render box is for.",
            ff.ffmpeg.display()
        ));
    }

    let dl = ff.probe_duration(left)?;
    let dr = ff.probe_duration(right)?;
    let longest = if dl > dr { dl } else { dr };
    println!(
        "split: left {}s [{left_label}] | right {}s [{right_label}] | output {}s",
        secs(dl),
        secs(dr),
        secs(longest)
    );

    let graph = filtergraph(longest, left_label, right_label, &font);
    let args = ffmpeg_argv(
        &ff.arg_path(left)?,
        &ff.arg_path(right)?,
        &graph,
        &ff.arg_path(out)?,
    );
    let mut c = Command::new(&ff.ffmpeg);
    c.args(&args);
    let r = capture(&mut c)?;
    if !r.ok() {
        return Err(format!("ffmpeg failed: {}", r.why()));
    }

    // Look at what you made (FILMING.md §6): the output is probed, not assumed.
    let dout = ff.probe_duration(out)?;
    let bytes = crate::proc::filesize(out)?;
    println!("split: {}s {bytes} bytes -> {}", secs(dout), out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &str = r"C\:/Windows/Fonts/arialbd.ttf";

    #[test]
    fn the_shorter_run_is_held_not_blacked_out() {
        let g = filtergraph(36.049, "TAS 29.988", "ShcrTM 36.049", FONT);
        // both halves hold their final frame ...
        assert_eq!(g.matches("tpad=stop_mode=clone:stop_duration=60").count(), 2);
        // ... and both are cut to the LONGER run, so the gap reads as time
        assert_eq!(g.matches("trim=duration=36.049").count(), 2);
        // tpad's DEFAULT stop_mode is `add`, which pads with black frames --
        // the "broken video" reading this filter exists to avoid.
        assert!(!g.contains("stop_mode=add"));
    }

    #[test]
    fn both_halves_are_labelled_and_stacked_left_then_right() {
        let g = filtergraph(10.0, "LEFT", "RIGHT", FONT);
        assert!(g.contains("text='LEFT'"));
        assert!(g.contains("text='RIGHT'"));
        assert!(g.ends_with("[l][r]hstack=inputs=2[v]"));
        assert!(g.find("[0:v]").unwrap() < g.find("[1:v]").unwrap());
        assert_eq!(g.matches("scale=960:-2").count(), 2);
        assert_eq!(g.matches(&format!("fontfile='{FONT}'")).count(), 2);
    }

    #[test]
    fn durations_in_the_graph_are_seconds_with_a_decimal() {
        let g = filtergraph(6.36, "a", "b", FONT);
        assert!(g.contains("trim=duration=6.360"), "{g}");
        assert!(!g.contains("6360"));
    }

    #[test]
    fn labels_that_would_break_the_graph_are_escaped() {
        assert_eq!(escape_drawtext("xeap-.-"), "xeap-.-");
        assert_eq!(escape_drawtext("TAS: 6.323"), r"TAS\: 6.323");
        assert_eq!(escape_drawtext("d'Artagnan"), r"d\'Artagnan");
        assert_eq!(escape_drawtext("100%"), r"100\%");
        assert_eq!(escape_drawtext(r"a\b"), r"a\\b");
        let g = filtergraph(1.0, "TAS: x", "b", FONT);
        assert!(g.contains(r"text='TAS\: x'"), "{g}");
    }

    #[test]
    fn encoder_flags_are_the_published_encode() {
        let a = ffmpeg_argv("l.mp4", "r.mp4", "G", "o.mp4");
        for pair in [("-c:v", "libx264"), ("-crf", "19"), ("-preset", "medium"), ("-pix_fmt", "yuv420p")] {
            assert!(
                a.windows(2).any(|w| w[0] == pair.0 && w[1] == pair.1),
                "{pair:?} missing from {a:?}"
            );
        }
        assert!(a.contains(&"-an".to_string()));
        assert!(a.contains(&"-y".to_string()));
        assert_eq!(a[a.len() - 1], "o.mp4");
        // inputs in order: left is input 0, which is what the graph indexes
        let il = a.iter().position(|s| s == "l.mp4").unwrap();
        let ir = a.iter().position(|s| s == "r.mp4").unwrap();
        assert!(il < ir);
    }
}
