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

/// The run's inputs on a 10 ms race-time grid, indexed by `(race_ms / 10)`.
///
/// Ticks before race 0 (the countdown) are dropped: they are real inputs, but
/// no frame of the video shows them, and keeping them shifts every index by an
/// amount that varies per tape.
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
    let mut out = vec![Input::default(); (last / 10) as usize + 1];
    for i in 0..st.len() {
        let ms = t.race_ms(i);
        if ms < 0 {
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

/// 5x7 glyphs for `0123456789.:-+ ` and the few letters the labels need.
/// One `u8` per row, bit 4 is the leftmost pixel.
fn glyph(c: char) -> [u8; 7] {
    match c {
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
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        _ => [0; 7],
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
const BG: [u8; 4] = [0, 0, 0, 150];

/// One frame of the panel at race time `ms`.
///
/// The history strip is the point of the thing: a single bar says what the
/// driver is doing now, and a TAS is only legible as a shape over time.
pub fn draw(cv: &mut Canvas, ins: &[Input], ms: i64, history_ms: i64) {
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

    // --- the history strip: steering over the last `history_ms`, with the
    //     throttle and brake as a band underneath it.
    let (sy, sh) = (58i64, 40i64);
    cv.rect(20, sy + sh / 2, PANEL_W as i64 - 40, 1, [90, 90, 90, 180]);
    let w = PANEL_W as i64 - 40;
    for px in 0..w {
        let t = ms - history_ms + (px * history_ms) / w.max(1);
        let i = at(t);
        let y = sy + sh / 2 - (i.steer as i64 * (sh / 2)) / 127;
        let c = if px == w - 1 { WHITE } else { BLUE };
        cv.span(20 + px, 20 + px, y.min(sy + sh / 2), (y - (sy + sh / 2)).abs().max(1), c);
        if i.gas {
            cv.rect(20 + px, sy + sh + 2, 1, 4, GREEN);
        }
        if i.brake {
            cv.rect(20 + px, sy + sh + 8, 1, 4, RED);
        }
    }
    text(cv, 20, PANEL_H as i64 - 12, &format!("-{:.1}S", history_ms as f64 / 1000.0), 1, DIM);
    text(cv, PANEL_W as i64 - 64, PANEL_H as i64 - 12, "INPUTS", 1, DIM);
}

pub struct Opts {
    pub offset_ms: i64,
    pub fps: f64,
    pub to: Option<f64>,
    pub history_ms: i64,
    pub margin: i64,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { offset_ms: 0, fps: 30.0, to: None, history_ms: 3000, margin: 24 }
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
        "19".into(),
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
            draw(&mut cv, &ins, ms, o.history_ms);
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

    #[test]
    fn full_lock_reaches_the_end_of_the_bar_and_the_sides_are_not_swapped() {
        let ins = vec![Input { steer: -127, gas: true, brake: false, respawn: false }];
        let mut cv = Canvas::new(PANEL_W, PANEL_H);
        draw(&mut cv, &ins, 0, 3000);
        let cx = PANEL_W / 2;
        let row = 44 * PANEL_W * 4;
        let lit = |x: usize| cv.px[row + x * 4 + 3] > 200;
        // full left fills to the left edge of the bar and nothing to the right
        assert!(lit(cx - (PANEL_W / 2 - 20) + 1), "full left must reach the left end");
        assert!(!lit(cx + 40), "full left must not draw on the right");
    }

    #[test]
    fn the_countdown_is_dropped_so_index_zero_is_race_zero() {
        // A tape that starts at -1500 ms must not shift the overlay by 150
        // ticks; `inputs_by_race_ms` indexes by race time, and the check that
        // matters is that a value written at race 0 lands at index 0.
        let ins = vec![Input { steer: 100, ..Default::default() }];
        let mut cv = Canvas::new(PANEL_W, PANEL_H);
        draw(&mut cv, &ins, 0, 3000);
        let cx = PANEL_W / 2;
        let row = 44 * PANEL_W * 4;
        assert!(cv.px[row + (cx + 40) * 4 + 3] > 200, "race 0 must read the first input");
    }
}
