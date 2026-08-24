//! `clip overlay` -- draw a run's own inputs onto a finished clip.
//!
//! **Which of the two steering channels this draws, and why it matters.** A
//! ghost holds the driver's steering twice: the input chunk `0x0309201D`, one
//! packet per **10 ms** tick, which is what the player actually pressed; and
//! byte 14 of every 116-byte telemetry sample at **50 ms**, which is what the
//! car had. An overlay is a picture of the player, so it draws the input
//! chunk. (The telemetry copy is a fifth of the resolution and, on a
//! synthesised tape, is whatever the carrier's driver did -- which is how a
//! previous overlay showed a stranger's hands.)
//!
//! **Steer is an i8 in a u8 field.** Read unsigned and every left input draws
//! as hard right; `Tape::steer_i8s` returns it correctly and this never touches
//! the raw byte.
//!
//! **Race zero is not tick zero.** Most of this project's tapes are
//! countdown-prefixed -- `start_offset_ms` around -1500, gas already down -- so
//! the overlay maps VIDEO time to RACE time via `Tape::race_ms`, never to a
//! tick index. `--offset-ms` shifts it if a particular render does not start at
//! race 0, and `clip alignment` measures what the shift should be instead of
//! leaving it to the eye.
//!
//! **No font dependency.** The split-screen path needs ffmpeg's `drawtext`,
//! which needs libfreetype, which the Mac's ffmpeg does not have. This draws
//! its own 5x7 digits, so it runs anywhere ffmpeg runs.
//!
//! The frames go to ffmpeg on a pipe as raw RGBA and are composited in one
//! pass, so nothing lands on disk and a 219-second clip costs one re-encode.
//!
//! # The control, and it was free
//!
//! 227969's published clip already has an overlay burned into it, drawn in
//! August 2026 by a since-deleted tool that read the file by its own reader.
//! Compositing this one onto that clip puts two independently written overlays
//! of the same run in one frame, and they agree: at race 3.000 both read full
//! right lock with the throttle down, and at 5.500 both read neutral steering
//! with the throttle AND the brake down together. Two readers, two codebases,
//! one file, same answer -- which is a stronger statement about the timing than
//! any number of frames judged by eye, and it cost nothing but choosing that
//! clip to test on.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::fmt::secs;
use crate::platform::Ff;

pub const PANEL_W: usize = 460;
pub const PANEL_H: usize = 116;

/// What the driver was doing at one instant.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Input {
    pub steer: i8,
    pub gas: bool,
    pub brake: bool,
    pub respawn: bool,
}

/// The last race instant the strip may draw: the run's declared finish, or the
/// end of the tape when the file declares nothing.
///
/// Split out of `inputs_by_race_ms` so it can be TESTED. It cannot be tested
/// through that function: the only fixtures in the repo are genuine
/// recordings whose tape ends at the finish, so a test through the file path
/// passes identically with the clamp and without it — which is what the first
/// version of this test did, and it is a guard that never fires.
///
/// A file declaring nothing keeps its whole tape. "This file declares no time"
/// is a different situation from "the run ended", and clamping to zero would be
/// worse than the defect.
fn keep_to(tape_end_ms: i64, declared_ms: Option<i64>) -> i64 {
    declared_ms.map(|e| e.min(tape_end_ms)).unwrap_or(tape_end_ms)
}

/// The run's inputs on a 10 ms race-time grid, indexed by `(race_ms / 10)`.
///
/// Ticks before race 0 (the countdown) are dropped: they are real inputs, but
/// no frame of the video shows them, and keeping them shifts every index by an
/// amount that varies per tape.
///
/// **AND TICKS AFTER THE FINISH ARE DROPPED TOO, WHICH IS NOT COSMETIC.** A
/// transplanted ghost inherits its CARRIER's input array, so a 12.759 run sits
/// in a 48.480 tape and everything past the finish is a stranger's driving —
/// the exact defect this module's header warns about, arriving through the back
/// door. It did not matter while the strip only drew the past, because the
/// video stops at the finish and the past never reached beyond it. Centring the
/// strip made the future half draw up to `future_ms` AHEAD of the playhead, so
/// the last three seconds of every clip showed someone else's inputs, presented
/// as this run's and reading as more of the same driving.
///
/// `end_ms` is the run's declared time. Past it there are no inputs, so the
/// strip visibly ENDS — which is also the honest picture: the run is over.
pub fn inputs_by_race_ms(path: &str) -> Result<Vec<Input>, String> {
    let t = gbx::tape::Tape::from_file(path)?;
    let (st, ac, br, rs) = (t.steer_i8s(), t.accels(), t.brakes(), t.respawns());
    if st.is_empty() {
        return Err(format!("{path} carries no input packets"));
    }
    let last = t.race_ms(st.len() - 1);
    if last < 0 {
        return Err(format!("{path}'s tape ends at race {last} ms -- it is all countdown"));
    }
    // The declared time, when the file states one. A file with no declared time
    // keeps the whole tape: dropping to zero would be worse than a stranger's
    // inputs, and "this file declares nothing" is a different situation.
    let end = gbx::record::decode_ghost(path)
        .ok()
        .and_then(|d| d.race_time_ms)
        .map(|v| v as i64)
        .filter(|v| *v > 0);
    let keep_to = keep_to(last, end);
    let mut out = vec![Input::default(); (keep_to / 10) as usize + 1];
    for i in 0..st.len() {
        let ms = t.race_ms(i);
        if ms < 0 || ms > keep_to {
            continue;
        }
        out[(ms / 10) as usize] = Input {
            steer: st[i],
            gas: ac.get(i).copied().unwrap_or(0) != 0,
            brake: br.get(i).copied().unwrap_or(0) != 0,
            respawn: rs.get(i).copied().unwrap_or(false),
        };
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// A very small raster, because a dependency is not worth a bar chart
// ---------------------------------------------------------------------------

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>, // RGBA
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Canvas { w, h, px: vec![0; w * h * 4] }
    }
    fn clear(&mut self) {
        self.px.iter_mut().for_each(|b| *b = 0);
    }
    fn set(&mut self, x: i64, y: i64, c: [u8; 4]) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        self.px[i..i + 4].copy_from_slice(&c);
    }
    fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 4]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }
    /// `x0` may be greater than `x1`; the bar is drawn between them either way,
    /// which is what a centre-zero steering bar needs.
    fn span(&mut self, x0: i64, x1: i64, y: i64, h: i64, c: [u8; 4]) {
        let (a, b) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        self.rect(a, y, (b - a).max(1), h, c);
    }
}

/// 5x7 glyphs. One `u8` per row, bit 4 is the leftmost pixel.
///
/// **THE WHOLE ALPHABET, AND A VISIBLE BOX FOR ANYTHING ELSE.** This used to
/// hold only the letters the labels of the day needed, with everything else
/// falling through to seven zero rows — a blank. So a label containing a letter
/// nobody had added rendered as a GAP, silently and correctly-looking: `NOW`
/// came out as `N W` and `INPUTS` as `NP TS`, burned into published clips,
/// because O, I and U were never in the table. A missing glyph now draws a
/// filled box, which is impossible to mistake for a space.
fn glyph(c: char) -> [u8; 7] {
    match c {
        ' ' => [0; 7],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        _ => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F],
    }
}

fn text(cv: &mut Canvas, x: i64, y: i64, s: &str, scale: i64, c: [u8; 4]) {
    let mut cx = x;
    for ch in s.chars() {
        let g = glyph(ch.to_ascii_uppercase());
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    cv.rect(cx + col * scale, y + row as i64 * scale, scale, scale, c);
                }
            }
        }
        cx += 6 * scale;
    }
}

const WHITE: [u8; 4] = [255, 255, 255, 235];
const DIM: [u8; 4] = [150, 150, 150, 200];
const GREEN: [u8; 4] = [60, 220, 90, 240];
const RED: [u8; 4] = [235, 70, 60, 240];
const BLUE: [u8; 4] = [90, 170, 255, 240];
const AMBER: [u8; 4] = [255, 190, 40, 245];
/// THE FUTURE HALF OF THE STRIP, in the same hues at lower intensity.
///
/// Dimmer rather than a different colour on purpose: the eye reads "same thing,
/// not yet" from brightness and "different thing" from hue, and these ARE the
/// same channels. A distinct colour would say the future inputs are a
/// prediction or a plan; they are the tape, already known and about to happen.
const BLUE_DIM: [u8; 4] = [70, 120, 180, 170];
const GREEN_DIM: [u8; 4] = [45, 150, 65, 170];
const RED_DIM: [u8; 4] = [160, 55, 48, 170];
const AMBER_DIM: [u8; 4] = [175, 130, 30, 175];
const BG: [u8; 4] = [0, 0, 0, 150];

/// One frame of the panel at race time `ms`.
///
/// The strip is the point of the thing: a single bar says what the driver is
/// doing now, and a TAS is only legible as a shape over time.
///
/// **NOW IS IN THE MIDDLE.** The strip used to end at the playhead, so it was a
/// picture of the past alone — you watched a car take a corner and only
/// afterwards saw the input that took it. Reading a TAS means seeing the input
/// arrive: the flick is placed BEFORE the thing it causes, and a viewer can
/// only judge the timing if both sides of the instant are on screen. So `now`
/// sits at a playhead in the middle, with `history_ms` behind it and
/// `future_ms` ahead, and the future half is drawn dimmer so the two are never
/// confused.
///
/// **Each pixel is an INTERVAL, not a sample.** With the window twice as wide
/// one pixel can span more than one 10 ms tick, and the old point-sampling
/// would then step straight over a single-tick flick — exactly the input most
/// worth seeing on these tapes. Every pixel now takes the extreme steering and
/// the OR of the pedals over the whole interval it covers, so a one-tick input
/// is visible at any zoom.
pub fn draw(cv: &mut Canvas, ins: &[Input], ms: i64, history_ms: i64, future_ms: i64) {
    cv.clear();
    cv.rect(0, 0, PANEL_W as i64, PANEL_H as i64, BG);

    let at = |t: i64| -> Input {
        if t < 0 {
            return Input::default();
        }
        ins.get((t / 10) as usize).copied().unwrap_or_default()
    };
    let now = at(ms);

    // --- the clock, and the two pedals
    text(cv, 10, 10, &format!("{:.3}", ms as f64 / 1000.0), 2, WHITE);
    let lamp = |cv: &mut Canvas, x: i64, on: bool, c: [u8; 4], label: &str| {
        cv.rect(x, 8, 74, 20, if on { c } else { [40, 40, 40, 200] });
        text(cv, x + 8, 14, label, 1, if on { [0, 0, 0, 255] } else { DIM });
    };
    lamp(cv, 150, now.gas, GREEN, "GAS");
    lamp(cv, 232, now.brake, RED, "BRAKE");
    if now.respawn {
        cv.rect(314, 8, 100, 20, AMBER);
        text(cv, 322, 14, "RESPAWN", 1, [0, 0, 0, 255]);
    }

    // --- the steering bar: centre zero, full lock is the full half-width
    let cx = PANEL_W as i64 / 2;
    let half = PANEL_W as i64 / 2 - 20;
    cv.rect(20, 44, PANEL_W as i64 - 40, 1, DIM);
    cv.rect(cx, 40, 1, 9, DIM);
    let sx = cx + (now.steer as i64 * half) / 127;
    cv.span(cx, sx, 38, 13, BLUE);
    text(cv, 20, 38, if now.steer < 0 { "L" } else { " " }, 1, DIM);
    text(cv, PANEL_W as i64 - 26, 38, if now.steer > 0 { "R" } else { " " }, 1, DIM);

    // --- the strip: steering across the window, with the throttle and brake as
    //     a band underneath it, and NOW at the playhead.
    let (sy, sh) = (58i64, 40i64);
    cv.rect(20, sy + sh / 2, PANEL_W as i64 - 40, 1, [90, 90, 90, 180]);
    let w = PANEL_W as i64 - 40;
    let span_ms = (history_ms + future_ms).max(1);
    // Where the playhead lands. Integer arithmetic throughout so the pixel the
    // playhead is drawn on is the same pixel the loop calls "now" -- a rounding
    // disagreement here puts the marker one pixel off the input it marks, which
    // is a lie about the timing at exactly the moment being judged.
    let ph = (history_ms * w) / span_ms;
    for px in 0..w {
        // The interval THIS pixel covers, [t0, t1).
        let t0 = ms - history_ms + (px * span_ms) / w;
        let t1 = ms - history_ms + ((px + 1) * span_ms) / w;
        let (mut steer, mut gas, mut brake, mut respawn) = (0i64, false, false, false);
        let mut t = t0;
        loop {
            let i = at(t);
            if (i.steer as i64).abs() > steer.abs() {
                steer = i.steer as i64;
            }
            gas |= i.gas;
            brake |= i.brake;
            respawn |= i.respawn;
            t += 10;
            if t >= t1 {
                break;
            }
        }
        let future = px > ph;
        let y = sy + sh / 2 - (steer * (sh / 2)) / 127;
        let c = if px == ph {
            WHITE
        } else if future {
            BLUE_DIM
        } else {
            BLUE
        };
        cv.span(20 + px, 20 + px, y.min(sy + sh / 2), (y - (sy + sh / 2)).abs().max(1), c);
        if gas {
            cv.rect(20 + px, sy + sh + 2, 1, 4, if future { GREEN_DIM } else { GREEN });
        }
        if brake {
            cv.rect(20 + px, sy + sh + 8, 1, 4, if future { RED_DIM } else { RED });
        }
        if respawn {
            cv.rect(20 + px, sy, 1, sh, if future { AMBER_DIM } else { AMBER });
        }
    }
    // The playhead, the full height of the strip and over the pedal bands, so
    // the instant is readable even where the steering is at zero. It STOPS
    // above the label row: run through the text and it cuts "NOW" in half,
    // which is what the first version of this did.
    cv.rect(20 + ph, sy - 2, 1, sh + 6, WHITE);
    text(cv, 20, PANEL_H as i64 - 12, &format!("-{:.1}S", history_ms as f64 / 1000.0), 1, DIM);
    text(cv, 20 + ph - 9, PANEL_H as i64 - 12, "NOW", 1, WHITE);
    let right = format!("+{:.1}S", future_ms as f64 / 1000.0);
    text(cv, PANEL_W as i64 - 20 - right.len() as i64 * 6, PANEL_H as i64 - 12, &right, 1, DIM);
}

pub struct Opts {
    pub offset_ms: i64,
    pub fps: f64,
    pub to: Option<f64>,
    pub history_ms: i64,
    /// How far AHEAD of the playhead the strip reaches. The strip is centred on
    /// `now`, so this is the half a viewer uses to see an input arrive before
    /// the thing it causes.
    pub future_ms: i64,
    pub margin: i64,
    /// x264 quality. 19 is the published clips' encode and is the default;
    /// **a long clip needs a higher number to fit.** GitHub refuses an asset
    /// over 100 MB, and 220 s of 1280x720 at crf 19 is 171 MB -- so on this
    /// project's long maps the choice is a slightly softer picture or no video
    /// at all. `clip ship` cannot tell you this in advance, which is why it is
    /// a flag rather than something to discover at the upload.
    pub crf: u32,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { offset_ms: 0, fps: 30.0, to: None, history_ms: 3000, future_ms: 3000, margin: 24, crf: 19 }
    }
}

/// The ffmpeg invocation: the clip, our RGBA frames on a pipe, one overlay.
///
/// `-t` goes on the OUTPUT so the cut is exact; on the input it would seek to a
/// keyframe. The overlay is anchored bottom-left with a margin, in `main_h`
/// terms, so it sits correctly whatever the clip's height is.
pub fn ffmpeg_argv(video: &str, out: &str, o: &Opts) -> Vec<String> {
    let mut v: Vec<String> = vec!["-v".into(), "error".into(), "-y".into(), "-i".into(), video.into()];
    v.extend([
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "-s".into(),
        format!("{PANEL_W}x{PANEL_H}"),
        "-r".into(),
        format!("{}", o.fps),
        "-i".into(),
        "-".into(),
        "-filter_complex".into(),
        format!("[0:v][1:v]overlay={m}:main_h-overlay_h-{m}:format=auto", m = o.margin),
    ]);
    if let Some(t) = o.to {
        v.extend(["-t".into(), format!("{t:.3}")]);
    }
    v.extend([
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        o.crf.to_string(),
        "-preset".into(),
        "medium".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-an".into(),
        out.into(),
    ]);
    v
}

pub fn run(ff: &Ff, ghost: &Path, video: &Path, out: &Path, o: &Opts) -> Result<(), String> {
    let ins = inputs_by_race_ms(&ghost.to_string_lossy())?;
    let din = ff.probe_duration(video)?;
    let dur = o.to.unwrap_or(din);
    if dur > din + 0.5 {
        return Err(format!(
            "asked for {}s of overlay on a {}s clip",
            secs(dur),
            secs(din)
        ));
    }
    let tape_end = (ins.len() as i64 - 1) * 10;
    println!(
        "overlay: {} inputs to race {}s onto {}s of video (writing {}s), offset {:+} ms",
        ins.len(),
        secs(tape_end as f64 / 1000.0),
        secs(din),
        secs(dur),
        o.offset_ms
    );
    // THE RUN MUST FIT THE CLIP. An overlay that runs out of inputs half way
    // draws a car sitting at neutral for the rest of the video, which reads as
    // "the driver let go" rather than "the tool ran out".
    if tape_end + o.offset_ms < (dur * 1000.0) as i64 - 1500 {
        return Err(format!(
            "the tape ends at race {}s but the clip is {}s: the overlay would show neutral input \
             for the last {}s. Trim the clip with --to, or check --offset-ms.",
            secs(tape_end as f64 / 1000.0),
            secs(dur),
            secs((dur * 1000.0 - (tape_end + o.offset_ms) as f64) / 1000.0)
        ));
    }

    let args = ffmpeg_argv(&ff.arg_path(video)?, &ff.arg_path(out)?, o);
    let mut child = Command::new(&ff.ffmpeg)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", ff.ffmpeg.display()))?;
    {
        let mut w = std::io::BufWriter::with_capacity(
            PANEL_W * PANEL_H * 4 * 8,
            child.stdin.take().ok_or("no stdin on ffmpeg")?,
        );
        let mut cv = Canvas::new(PANEL_W, PANEL_H);
        let n = (dur * o.fps).round() as i64;
        for f in 0..n {
            let ms = ((f as f64 / o.fps) * 1000.0).round() as i64 + o.offset_ms;
            draw(&mut cv, &ins, ms, o.history_ms, o.future_ms);
            if w.write_all(&cv.px).is_err() {
                // ffmpeg died; its stderr below says why, and that is a better
                // message than a broken pipe.
                break;
            }
        }
        let _ = w.flush();
    }
    let outp = child.wait_with_output().map_err(|e| e.to_string())?;
    if !outp.status.success() {
        return Err(format!(
            "ffmpeg failed ({}): {}",
            outp.status,
            String::from_utf8_lossy(&outp.stderr).trim()
        ));
    }

    // Look at what you made (FILMING.md section 6).
    let dout = ff.probe_duration(out)?;
    if (dout - dur).abs() > 1.0 {
        return Err(format!("asked for {}s and the output is {}s", secs(dur), secs(dout)));
    }
    let bytes = crate::proc::filesize(out)?;
    println!("overlay: {}s {bytes} bytes -> {}", secs(dout), out.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Is the overlay in time?
// ---------------------------------------------------------------------------

/// One panel, at one race time, as a PNG — WITHOUT rendering a video.
///
/// FILMING.md §6 says look at what you made, and for the overlay itself that
/// used to mean a full re-encode of a whole clip before you could see whether a
/// change to the panel was right. `clip frames` closed that gap for the video;
/// this closes it for the panel. It is also the only way to see the panel at a
/// race time the clip does not reach, which is what checking the FUTURE half of
/// the strip needs.
///
/// Through ffmpeg rather than a PNG encoder, because this crate deliberately
/// has no third-party dependencies (it builds on the render box's WSL side with
/// no network) and it already drives ffmpeg for everything else.
pub fn panel_png(ff: &Ff, ghost: &Path, at_ms: i64, out: &Path, o: &Opts) -> Result<(), String> {
    let ins = inputs_by_race_ms(&ghost.to_string_lossy())?;
    let mut cv = Canvas::new(PANEL_W, PANEL_H);
    draw(&mut cv, &ins, at_ms, o.history_ms, o.future_ms);
    let args: Vec<String> = vec![
        "-v".into(),
        "error".into(),
        "-y".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "-s".into(),
        format!("{PANEL_W}x{PANEL_H}"),
        "-i".into(),
        "-".into(),
        "-frames:v".into(),
        "1".into(),
        ff.arg_path(out)?,
    ];
    let mut child = Command::new(&ff.ffmpeg)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", ff.ffmpeg.display()))?;
    child
        .stdin
        .take()
        .ok_or("no stdin on ffmpeg")?
        .write_all(&cv.px)
        .map_err(|e| e.to_string())?;
    let outp = child.wait_with_output().map_err(|e| e.to_string())?;
    if !outp.status.success() {
        return Err(format!(
            "ffmpeg failed ({}): {}",
            outp.status,
            String::from_utf8_lossy(&outp.stderr).trim()
        ));
    }
    // ffmpeg exits 0 having written nothing when its input is short; the file
    // is the only evidence that it did the work.
    let bytes = crate::proc::filesize(out)?;
    let tape_end = (ins.len() as i64 - 1) * 10;
    println!(
        "panel at race {} (tape runs to {}), window -{:.1}s/+{:.1}s: {bytes} bytes -> {}",
        secs(at_ms as f64 / 1000.0),
        secs(tape_end as f64 / 1000.0),
        o.history_ms as f64 / 1000.0,
        o.future_ms as f64 / 1000.0,
        out.display()
    );
    Ok(())
}

/// Fit the constant lag between the two steering channels a ghost carries.
///
/// The input chunk (10 ms, what the overlay draws) and the telemetry's byte-14
/// echo (50 ms, what the car had) describe ONE run, so they agree at exactly
/// one shift and nowhere else. That makes the alignment a measurement rather
/// than something to eyeball against a frame -- and it is the same instrument
/// that caught a 50 ms error three eyeballed frames had passed.
///
/// Returns `(best_lag_ms, disagreement_at_best, disagreement_at_zero)`.
pub fn alignment(ghost: &str, span_ms: i64) -> Result<(i64, f64, f64), String> {
    let ins = inputs_by_race_ms(ghost)?;
    let d = gbx::record::decode_ghost(ghost)?;
    let mut at_lag: Vec<(i64, f64)> = Vec::new();
    for lag in (-span_ms / 10..=span_ms / 10).map(|k| k * 10) {
        let mut n = 0usize;
        let mut err = 0.0f64;
        for s in &d.samples {
            if s.time_ms < 0 {
                continue;
            }
            let t = s.time_ms as i64 + lag;
            if t < 0 {
                continue;
            }
            let Some(i) = ins.get((t / 10) as usize) else { continue };
            // The record stores steer as `floor((steer_i8 + 127) * 255 / 254)`;
            // comparing in i8 space avoids re-deriving that here.
            err += (s.steer * 127.0 - i.steer as f64).abs();
            n += 1;
        }
        if n > 20 {
            at_lag.push((lag, err / n as f64));
        }
    }
    if at_lag.is_empty() {
        return Err("no shared instants between the tape and the record".into());
    }
    let zero = at_lag.iter().find(|(l, _)| *l == 0).map(|(_, e)| *e).unwrap_or(f64::NAN);
    let best = at_lag.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
    Ok((best.0, best.1, zero))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_length_is_an_output_option_not_an_input_seek() {
        let o = Opts { to: Some(222.0), ..Default::default() };
        let a = ffmpeg_argv("in.webm", "out.mp4", &o);
        let last_i = a.iter().rposition(|x| x == "-i").unwrap();
        let t = a.iter().position(|x| x == "-t").unwrap();
        assert!(t > last_i, "-t must follow every -i or the cut snaps to a keyframe");
        assert_eq!(a[t + 1], "222.000");
    }

    #[test]
    fn the_panel_is_a_second_input_not_a_filter() {
        let a = ffmpeg_argv("in.webm", "out.mp4", &Opts::default());
        assert_eq!(a.iter().filter(|x| *x == "-i").count(), 2);
        assert!(a.iter().any(|x| x == "-"), "the frames arrive on stdin");
        assert!(a.iter().any(|x| x.starts_with("[0:v][1:v]overlay=")));
    }

    /// How far the steering bar reaches from centre, signed: negative left.
    /// Counted rather than sampled at one pixel, because the L/R labels sit at
    /// the ends of the bar and overwrite it -- a single-pixel assertion there
    /// tests the label, which is how the first version of this test failed on
    /// correct output.
    fn bar_extent(steer: i8) -> i64 {
        let ins = vec![Input { steer, gas: true, brake: false, respawn: false }];
        let mut cv = Canvas::new(PANEL_W, PANEL_H);
        draw(&mut cv, &ins, 0, 3000, 3000);
        let cx = (PANEL_W / 2) as i64;
        let row = 44 * PANEL_W * 4;
        let is_bar = |x: i64| {
            let i = row + x as usize * 4;
            cv.px[i..i + 4] == BLUE
        };
        let mut left = 0i64;
        while cx - left - 1 > 0 && is_bar(cx - left - 1) {
            left += 1;
        }
        let mut right = 0i64;
        while cx + right + 1 < PANEL_W as i64 && is_bar(cx + right + 1) {
            right += 1;
        }
        if left > right { -left } else { right }
    }

    #[test]
    fn full_lock_reaches_the_end_of_the_bar_and_the_sides_are_not_swapped() {
        let half = (PANEL_W / 2 - 20) as i64;
        // Full left reaches the left end -- allowing for the label glyph, which
        // paints over the last few pixels of the bar.
        let l = bar_extent(-127);
        assert!(l < -(half - 12), "full left reached only {l}, expected about {}", -half);
        let r = bar_extent(127);
        assert!(r > half - 12, "full right reached only {r}, expected about {half}");
        // A NEGATIVE STEER MUST NOT DRAW RIGHT. Steer is an i8 in a u8 field,
        // and a naive unsigned read makes every left input full right -- the
        // exact defect this test exists for.
        assert!(bar_extent(-64) < 0, "left steering drew to the right");
        assert_eq!(bar_extent(0).abs() <= 1, true, "neutral must draw nothing");
    }

    #[test]
    fn the_countdown_is_dropped_so_index_zero_is_race_zero() {
        // A tape starting at -1500 ms must not shift the overlay by 150 ticks:
        // `inputs_by_race_ms` indexes by RACE time, so a value at race 0 lands
        // at index 0 and the bar reads it at ms 0.
        assert!(bar_extent(100) > 100, "race 0 must read the first input");
    }

    /// Build a tape whose only input is one tick of full right lock at
    /// `at_ms`, then report which strip pixels are painted.
    fn strip_pixels(at_ms: i64, now_ms: i64, past: i64, future: i64) -> Vec<i64> {
        let mut ins = vec![Input::default(); (at_ms / 10) as usize + 64];
        ins[(at_ms / 10) as usize] = Input { steer: 127, gas: false, brake: false, respawn: false };
        let mut cv = Canvas::new(PANEL_W, PANEL_H);
        draw(&mut cv, &ins, now_ms, past, future);
        // Only the STEERING TRACE counts, matched by its own two colours. A
        // "not the background" test also catches the playhead, which is painted
        // on every column of its own -- and then a test asking "did the future
        // input draw" passes on the marker instead of on the input.
        let row = (58 + 40 / 2 - 8) as usize * PANEL_W * 4;
        (0..(PANEL_W as i64 - 40))
            .filter(|px| {
                let i = row + (20 + *px) as usize * 4;
                cv.px[i..i + 4] == BLUE || cv.px[i..i + 4] == BLUE_DIM
            })
            .collect()
    }

    /// THE FUTURE HALF IS ACTUALLY DRAWN, AND ON THE RIGHT SIDE OF NOW.
    ///
    /// The strip used to end at the playhead: an input was invisible until
    /// after the corner it caused. This is the whole point of centring it, so
    /// it is asserted rather than eyeballed on a frame -- an input 1.5 s in
    /// the FUTURE must paint pixels, and they must be right of the playhead.
    #[test]
    fn an_input_that_has_not_happened_yet_is_drawn_ahead_of_the_playhead() {
        let w = PANEL_W as i64 - 40;
        let ph = (3000 * w) / 6000;
        let ahead = strip_pixels(4500, 3000, 3000, 3000);
        assert!(!ahead.is_empty(), "an input 1.5 s ahead drew nothing -- the future is not rendered");
        assert!(
            ahead.iter().all(|px| *px > ph),
            "a future input painted at or behind the playhead ({ph}): {ahead:?}"
        );
        // And the past still works, on the other side.
        let behind = strip_pixels(1500, 3000, 3000, 3000);
        assert!(!behind.is_empty(), "an input 1.5 s ago drew nothing");
        assert!(
            behind.iter().all(|px| *px < ph),
            "a past input painted at or ahead of the playhead ({ph}): {behind:?}"
        );
    }

    /// A ONE-TICK FLICK SURVIVES ANY ZOOM.
    ///
    /// Widening the window means a pixel can span more than one 10 ms tick,
    /// and the old per-pixel POINT SAMPLE would step straight over a
    /// single-tick input -- the input most worth seeing on these tapes, and a
    /// loss that looks exactly like the driver never touching the wheel. The
    /// strip aggregates over each pixel's interval instead, so this holds even
    /// at a zoom where one pixel is several ticks.
    #[test]
    fn a_single_tick_input_is_visible_even_when_a_pixel_spans_many_ticks() {
        for (past, future) in [(3000, 3000), (15000, 15000), (60000, 60000)] {
            let span = past + future;
            let ms_per_px = span / (PANEL_W as i64 - 40);
            // 730 ms ahead of now: an odd offset, and deliberately NOT the
            // playhead column, which is painted white by design.
            let hit = strip_pixels(past + 730, past, past, future);
            assert!(
                !hit.is_empty(),
                "a one-tick input vanished at {ms_per_px} ms/px (window -{past}/+{future})"
            );
        }
    }

    /// The playhead is where the arithmetic says it is. Drawn from one
    /// expression and used by another; if they disagree the marker sits beside
    /// the input it marks, which is a lie about timing at the one instant
    /// being judged.
    #[test]
    fn the_playhead_sits_where_now_is_and_moves_with_an_asymmetric_window() {
        let w = PANEL_W as i64 - 40;
        for (past, future) in [(3000i64, 3000i64), (5000, 1000), (1000, 5000)] {
            let ins = vec![Input::default(); 2000];
            let mut cv = Canvas::new(PANEL_W, PANEL_H);
            draw(&mut cv, &ins, 5000, past, future);
            let ph = (past * w) / (past + future);
            let row = (58 - 2) as usize * PANEL_W * 4;
            let i = row + (20 + ph) as usize * 4;
            assert_eq!(
                cv.px[i..i + 4],
                WHITE,
                "no playhead at px {ph} for window -{past}/+{future}"
            );
        }
    }

    /// EVERY LETTER AND DIGIT DRAWS SOMETHING, AND AN UNKNOWN ONE IS VISIBLE.
    ///
    /// The table used to hold only the letters the labels of the day needed and
    /// fall through to seven zero rows for the rest, so a label with an unlisted
    /// letter rendered as a GAP and looked deliberate. `NOW` shipped as `N W`;
    /// `INPUTS` shipped as `NP TS`. Both were burned into published clips and
    /// read as a font quirk rather than as a missing glyph.
    #[test]
    fn no_letter_renders_as_a_silent_blank() {
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars() {
            assert_ne!(glyph(c), [0u8; 7], "{c} draws nothing -- it would render as a space");
        }
        // A space is the ONE thing allowed to be blank.
        assert_eq!(glyph(' '), [0u8; 7]);
        // And anything unknown draws a box, so it cannot be mistaken for one.
        assert_ne!(glyph('@'), [0u8; 7], "an unknown glyph must be visible");
    }

    /// The labels the panel actually draws, spelled out. A rendering test on the
    /// real strings is what would have caught `NOW` -- the glyph table was
    /// self-consistent, and only the LABEL was unrenderable.
    #[test]
    fn the_panels_own_labels_all_render() {
        for s in ["NOW", "GAS", "BRAKE", "RESPAWN", "-3.0S", "+3.0S", "L", "R"] {
            for c in s.chars() {
                assert_ne!(
                    glyph(c),
                    [0u8; 7],
                    "the label {s:?} contains {c:?}, which draws nothing"
                );
            }
        }
    }

    /// THE STRIP MUST NOT DRAW INPUT FROM AFTER THE FINISH.
    ///
    /// A transplanted ghost inherits its CARRIER's input array, so a 12.759 run
    /// sits in a 48.480 tape and every tick past the finish is a stranger's
    /// driving. While the strip only drew the past this was invisible -- the
    /// video stops at the finish, so the past never reached beyond it. Centring
    /// the strip made the future half draw ahead of the playhead, and the last
    /// three seconds of both published clips then showed someone else's inputs,
    /// presented as this run's and reading as more of the same driving.
    ///
    /// Tested on `keep_to` rather than through a file, and that is the point:
    /// every ghost in `testdata/` is a genuine recording whose tape ends AT its
    /// finish (human_22730 declares 22.730 and its tape's last tick is race
    /// 22730), so a test through `inputs_by_race_ms` passes identically with
    /// the clamp and without it. I wrote that test first, removed the clamp to
    /// check it, and it stayed green.
    #[test]
    fn a_carriers_tape_is_cut_at_the_runs_own_finish() {
        // The real case: untitled 01, a 12.759 run in a 48.480 carrier tape.
        assert_eq!(keep_to(48_480, Some(12_759)), 12_759);
        // A genuine recording, where the two already agree: unchanged.
        assert_eq!(keep_to(22_730, Some(22_730)), 22_730);
        // A file that declares NOTHING keeps its tape. Clamping to zero would
        // be worse than the defect -- an empty strip reads as "the driver never
        // touched anything".
        assert_eq!(keep_to(48_480, None), 48_480);
        // And a declared time LONGER than the tape cannot invent input.
        assert_eq!(keep_to(9_000, Some(12_759)), 9_000);
    }
}
