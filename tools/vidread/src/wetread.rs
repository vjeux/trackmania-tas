//! The tyre-wetness readout: a percentage whose digits move.
//!
//! Speed sits in a fixed three-cell field. This one does not: it is drawn
//! **left-aligned** after an icon as `4%`, `43%`, `100%`, so the digit count —
//! and therefore where the field ends — changes with the value.
//!
//! Two things have to be settled before a digit can be cut, and neither can be
//! assumed:
//!
//! * **Which line is in the box.** The same slot draws `! Slip`, and the `!`
//!   sits exactly where the droplet sits. A contrast test says the slot is
//!   *drawn*; it cannot say what is in it. On this run 1326 frames carry the
//!   bang and 688 carry a droplet with a readable number, so a reader that
//!   skips this step spends most of its frames decoding the word `Slip`.
//! * **Where the cells are.** Measured from the median ink profile of every
//!   clean droplet frame, per edge bucket (`vidread wetgeom`): the field starts
//!   at x 2136, the pitch is exactly 9 px, and the `%` is 11 px wide. So the
//!   right edge *counts the digits* — 2156, 2165, 2174 for one, two, three —
//!   and an integer pitch means every cell shares one sub-pixel phase.
//!
//! Three digits can only ever be `100`, which is what makes the alphabet
//! bootstrappable with no eye-labelling at all: those frames hand over a `1`
//! and two `0`s, and the rest of the alphabet follows from the dry-out law.

use crate::digits::{Field, Patch, Templates};
use crate::frame::{Frame, Rect};

/// The icon slot: a droplet when the readout is a percentage, an exclamation
/// mark when the line is `! Slip`. `icon_present` cannot tell them apart —
/// it measures contrast — so the shape has to be read.
pub const ICON: Rect = Rect::new(2119, 1229, 12, 15);
/// Where the field starts, the digit pitch, and the glyph box — all MEASURED,
/// from the median ink profile of every clean droplet frame in each edge
/// bucket (`vidread wetgeom`).
///
/// The field is **left-aligned** at `FIELD_X0` on a 9 px pitch, and the `%` is
/// 11 px wide and follows the last digit. That makes the right edge a direct
/// count of the digits: 1 digit ends at 2156, 2 at 2165, 3 at 2174. Cell `k`
/// of an `n`-digit reading sits at `FIELD_X0 + PITCH*k`, so the units cell of
/// any reading is at `edge - 20`.
pub const FIELD_X0: usize = 2136;
pub const PITCH: usize = 9;
pub const PCT_W: usize = 11;
pub const CELL_Y: usize = 1230;
pub const CELL_W: usize = 9;
pub const CELL_H: usize = 15;

/// The right edge an `n`-digit reading produces.
pub fn edge_for(n: usize) -> usize {
    FIELD_X0 + PITCH * n + PCT_W
}

/// How many digits a right edge implies, if it is one of the field's modes.
///
/// `tol` forgives a pixel or two. That is free rather than risky: the cells are
/// anchored at `FIELD_X0`, not at the edge, so a wrong edge moves no cell — it
/// can only change the digit COUNT, and a wrong count is refused downstream by
/// the leading-zero rule, the 0..100 range and the dry-out law.
pub fn digits_at(edge: usize, tol: usize) -> Option<usize> {
    (1..=3).find(|n| edge_for(*n).abs_diff(edge) <= tol)
}

/// The band of columns the readout occupies, measured from the icon so the
/// whole thing rides with the HUD rather than with absolute frame coordinates.
pub const BAND: (usize, usize) = (ICON.x + ICON.w + 2, ICON.x + ICON.w + 62);

/// Per-column ink over the glyph rows only, as a fraction of the rows.
///
/// Rows, not the whole box: the `! Slip` line shares this y range, and a
/// profile taken over the full box height mixes it in. Ink is measured against
/// the column band's own dark level rather than an absolute threshold, because
/// this box sits over everything from a dark tunnel to a white wall.
pub fn ink_profile(f: &Frame) -> Vec<f32> {
    let mut lvl: Vec<f32> = Vec::new();
    for x in BAND.0..BAND.1 {
        for y in CELL_Y..CELL_Y + CELL_H {
            lvl.push(f.minc(x, y));
        }
    }
    lvl.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let dark = lvl[(lvl.len() - 1) * 20 / 100];
    let bright = lvl[(lvl.len() - 1) * 98 / 100];
    let cut = dark + (bright - dark) * 0.55;
    (BAND.0..BAND.1)
        .map(|x| {
            (CELL_Y..CELL_Y + CELL_H).filter(|&y| f.minc(x, y) >= cut).count() as f32
                / CELL_H as f32
        })
        .collect()
}

/// The field's right edge: the rightmost run of inked columns in the band.
///
/// Returns the x of the column just past the last ink, which is where a cell
/// grid anchored on the `%` starts. No template is involved, which is the
/// point: the cells cannot be cut until the right edge is known, and a
/// template for the `%` cannot be trained until the cells can be cut.
pub fn right_edge(f: &Frame, min_ink: f32, max_gap: usize) -> Option<usize> {
    let p = ink_profile(f);
    let last = (0..p.len()).rev().find(|&i| p[i] >= min_ink)?;
    // Walk left through the glyph, tolerating the gaps inside a `%`.
    let mut i = last;
    let mut gap = 0usize;
    while i > 0 {
        i -= 1;
        if p[i] >= min_ink {
            gap = 0;
        } else {
            gap += 1;
            if gap > max_gap {
                break;
            }
        }
    }
    Some(BAND.0 + last + 1)
}


/// Is the readout on screen? Contrast, not level: the box sits over everything
/// from a dark tunnel to a white wall, so an absolute threshold measures the
/// scenery rather than the icon.
///
/// This says the slot is **drawn**, not what is in it. The same slot carries
/// the `!` of `! Slip`, so a percentage reader has to go on to `icon_cut`.
pub fn icon_present(f: &Frame, span_min: f32) -> bool {
    let mut v: Vec<f32> = Vec::with_capacity(ICON.w * ICON.h);
    for y in ICON.y..ICON.y + ICON.h {
        for x in ICON.x..ICON.x + ICON.w {
            v.push(f.minc(x, y));
        }
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[(v.len() - 1) * 95 / 100] - v[(v.len() - 1) * 5 / 100] >= span_min
}

/// The icon slot as a patch at a given shift, contrast-normalised like every
/// other glyph in this crate.
pub fn icon_at(f: &Frame, dx: i32, dy: i32) -> Patch {
    Patch::cut(f, &Field::parse(&format!("{};{};{};{}", ICON.x, ICON.y, ICON.w, ICON.h)), 0, dx, dy)
}

/// The icon slot as a patch.
pub fn icon_cut(f: &Frame) -> Patch {
    icon_at(f, 0, 0)
}

/// The droplet's name in an icon template bank. `!` names the `! Slip` bang.
pub const DROPLET: char = 'D';

/// Which icon is in the slot, and how well it matched. The bank is trained
/// from the clusters `vidread weticon` finds, so the two shapes are measured
/// rather than assumed. The HUD box drifts by a pixel, so each template is
/// matched over a small shift window, exactly as the digit cells are.
pub fn icon_kind(f: &Frame, t: &Templates) -> (char, f32) {
    let shifts: Vec<Patch> =
        (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| icon_at(f, dx, dy)).collect();
    let mut best = ('?', -2.0f32);
    for ((_, c), q) in t.g.iter() {
        let s = shifts.iter().map(|p| p.dot(q)).fold(-2.0f32, f32::max);
        if s > best.1 {
            best = (*c, s);
        }
    }
    best
}

/// Which frames carry a readable percentage, and where its cells are.
///
/// Three conditions, each measured rather than assumed: the slot is drawn
/// (contrast), the icon in it is the **droplet** and not the `!` of `! Slip`
/// (template), and the ink profile's right edge is one of the field's own
/// sharp modes rather than the band end the detector pins to when the
/// background saturates.
pub struct Select {
    pub span_min: f32,
    pub min_ink: f32,
    pub max_gap: usize,
    pub icon_min: f32,
    /// How far the measured right edge may sit from one of the field's modes.
    pub edge_tol: usize,
}

impl Select {
    /// The frame's right edge if it carries a readable percentage.
    pub fn edge_of(&self, f: &Frame, icons: &Templates) -> Option<usize> {
        if !icon_present(f, self.span_min) {
            return None;
        }
        let (c, s) = icon_kind(f, icons);
        if c != DROPLET || s < self.icon_min {
            return None;
        }
        let e = right_edge(f, self.min_ink, self.max_gap)?;
        digits_at(e, self.edge_tol).map(|_| e)
    }
}

/// One cell of the field: cell `k` from the left, on the measured grid.
pub fn cell_at(k: usize) -> Field {
    Field::parse(&format!("{};{CELL_Y};{CELL_W};{CELL_H}", FIELD_X0 + PITCH * k))
}

/// The best-matching digit for cell `k`, over a one-pixel shift window.
///
/// The whole field is one sprite drawn on an integer pitch, so a shift is a
/// property of the field and not of a cell; the window is here because the HUD
/// box itself drifts by a pixel between clips.
pub fn match_cell(f: &Frame, t: &Templates, k: usize) -> (char, f32, f32) {
    let fd = cell_at(k);
    let cuts: Vec<Patch> = (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| Patch::cut(f, &fd, 0, dx, dy))
        .collect();
    let (mut best, mut second) = (('?', -2.0f32), -2.0f32);
    for ((_, c), q) in t.g.iter() {
        let s = cuts.iter().map(|p| p.dot(q)).fold(-2.0f32, f32::max);
        if s > best.1 {
            second = best.1;
            best = (*c, s);
        } else if s > second {
            second = s;
        }
    }
    (best.0, best.1, second)
}

pub struct Reading {
    pub edge: usize,
    pub text: String,
    pub value: Option<u32>,
    /// The weakest cell's correlation, and its margin over the runner-up.
    pub worst: f32,
    pub margin: f32,
}

/// Read the field on one frame, or refuse it.
///
/// Refusals are not failures: the line may be `! Slip`, the background may
/// have saturated the ink profile, or a cell may be too weak to call. Every
/// one of those is a frame this reader must not put a number on.
pub fn read(
    f: &Frame,
    icons: &Templates,
    t: &Templates,
    sel: &Select,
    digit_min: f32,
    margin_min: f32,
) -> Option<Reading> {
    let edge = sel.edge_of(f, icons)?;
    let n = digits_at(edge, sel.edge_tol)?;
    let mut text = String::new();
    let mut worst = 2.0f32;
    let mut margin = 2.0f32;
    for k in 0..n {
        let (c, s, second) = match_cell(f, t, k);
        // Two bars, and the second is the one that matters. A cell can
        // correlate at 0.95 with `9` and 0.92 with `0`, which is not a
        // reading of `9`: it is the reader saying it cannot tell. On this run
        // every wrong digit the law caught sat under a margin of 0.05 while
        // the bulk of the readings sit at 0.18.
        if s < digit_min || s - second < margin_min {
            return None;
        }
        text.push(c);
        worst = worst.min(s);
        margin = margin.min(s - second);
    }
    // A percentage is printed without leading zeros and cannot pass 100, so a
    // decode whose digits do not spell their own value is a misread. Both are
    // free checks; both are enforced.
    let value = text.parse::<u32>().ok().filter(|v| *v <= 100 && v.to_string() == text);
    Some(Reading { edge, text, value, worst, margin })
}
