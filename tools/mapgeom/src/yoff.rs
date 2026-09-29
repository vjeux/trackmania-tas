//! The map's vertical offset, **without a recording**.
//!
//! `world_y = 8*cy + yoff` for every grid-placed block, and `yoff` is a
//! property of the map's decoration. Until now this project fitted it by
//! dropping a human run onto the model and asking which height made the car
//! rest on something. That is a ghost, and the ghost is gone.
//!
//! It does not need one. A map file contains two kinds of placement, and only
//! one of them moves with `yoff`:
//!
//! | placement | height |
//! |---|---|
//! | grid block | `8*cy + yoff` — **moves** |
//! | free block | an absolute f32 in the file — **fixed** |
//! | item | an absolute f32 in the file — **fixed** |
//!
//! So `yoff` is the number that makes the two halves of the same map agree.
//! The author placed the items ON the blocks; the offset that puts them there
//! is the map's own, and nothing about it was driven.
//!
//! Two estimators, and they are **deliberately independent** — a control that
//! shares a source with the thing it checks is decoration:
//!
//! * **`cellmode`** reads the map file only. Every item record carries an
//!   absolute position *and* a cell, and `pos.y - 8*cell.y` has a dominant
//!   mode. It never opens the pack, so it cannot be wrong for any reason the
//!   geometry reader could be wrong for.
//! * **`rest`** reads the pack. It drops a plumb line from every item's anchor
//!   onto the **grid blocks only** and counts how many land on a surface just
//!   below them. It never looks at an item's cell, so it cannot be wrong for
//!   any reason `cellmode` could be wrong for.
//!
//! They can disagree. When they do, this says so and reports **UNMEASURED**
//! rather than picking the prettier one.
//!
//! ## The trap this is built around
//!
//! **An item's own cell y is not linear in its world y.** The earlier arm
//! recorded that, and it is why `cellmode` reports a *mode* over a whole map
//! rather than reading one item: most items sit at their cell's base, a
//! minority do not, and a single sample cannot tell you which kind it drew.
//! The spread is printed with the answer, because a mode holding 4 % of the
//! items is not the same finding as a mode holding 60 %.

use crate::probe::Index;
use crate::scene::Scene;
use std::collections::BTreeMap;
use tmmaps::map::MapFile;

/// The range of `yoff` the sweep considers, in whole 8 m cell rows.
///
/// `place::Yoff::coarse()` stops at −320. Three maps in the earlier corpus
/// settled on exactly that, which is the sweep's own floor, and **a fit that
/// lands on the boundary has not been shown to contain the optimum.** This
/// range goes past it deliberately so that landing on the edge means
/// something.
pub const ROW_LO: i32 = -48;
pub const ROW_HI: i32 = 8;

/// How far below an item's anchor a supporting surface may be and still count
/// as "this item is standing on that block".
///
/// Items are placed by a pivot that is usually at or near the model's base,
/// but a sign on a post and a gate arch both put the anchor above the road.
/// 4 m is generous enough for those and far tighter than the 8 m cell it has
/// to discriminate against.
pub const REST_BELOW: f32 = 4.0;
/// How far ABOVE the anchor the surface may be — numerical slack only.
pub const REST_ABOVE: f32 = 0.30;

#[derive(Clone, Debug)]
pub struct Estimate {
    pub yoff: f32,
    /// How many observations backed the winner.
    pub support: usize,
    /// How many observations there were in total.
    pub total: usize,
    /// The runner-up's support: a winner with a close second is not a finding.
    pub runner_up: usize,
    pub runner_up_yoff: f32,
}

impl Estimate {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.support as f32 / self.total as f32
        }
    }
    /// The winner's margin over the runner-up, as a fraction of the total.
    pub fn margin(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.support as f32 - self.runner_up as f32) / self.total as f32
        }
    }
}

/// **Estimator 1 — the map file alone.**
///
/// The mode of `item.pos.y - 8 * item.cell.y`, binned to the metre. Opens no
/// pack, reads no geometry, and needs nothing but the map.
pub fn cellmode(m: &MapFile) -> Estimate {
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    let mut total = 0usize;
    for it in &m.items {
        let (_, cy, _) = it.coords();
        let d = it.pos[1] - 8.0 * cy as f32;
        // Only offsets in a plausible decoration range are counted. Most of a
        // map's items are OFF the grid — their cell bytes are 0xFF filler —
        // and `pos.y - 8*255` is about −2000, nowhere near a decoration
        // height. Those are not evidence and are dropped; `total` counts what
        // survived, so a map whose items are all off-grid reports a small
        // sample rather than a confident answer from nothing.
        if d < (ROW_LO * 8) as f32 - 8.0 || d > (ROW_HI * 8) as f32 + 8.0 {
            continue;
        }
        *hist.entry(d.round() as i32).or_insert(0) += 1;
        total += 1;
    }
    top2(&hist, total)
}

/// The whole histogram an estimator saw, biggest first — for when a mode needs
/// arguing with rather than reading.
pub fn cellmode_hist(m: &MapFile) -> Vec<(i32, usize)> {
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    for it in &m.items {
        let (_, cy, _) = it.coords();
        let d = it.pos[1] - 8.0 * cy as f32;
        if d < (ROW_LO * 8) as f32 - 8.0 || d > (ROW_HI * 8) as f32 + 8.0 {
            continue;
        }
        *hist.entry(d.round() as i32).or_insert(0) += 1;
    }
    let mut v: Vec<(i32, usize)> = hist.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

pub fn rest_hist(m: &MapFile, grid: &Scene) -> Vec<(i32, usize)> {
    let idx = Index::build(grid, 32.0);
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    for it in &m.items {
        let col = idx.column(it.pos[0], it.pos[2]);
        let mut voted: Vec<i32> = Vec::new();
        for (y, _) in &col {
            let k = (it.pos[1] - y).round() as i32;
            if k < ROW_LO * 8 || k > ROW_HI * 8 || voted.contains(&k) {
                continue;
            }
            voted.push(k);
        }
        for k in voted {
            *hist.entry(k).or_insert(0) += 1;
        }
    }
    let mut v: Vec<(i32, usize)> = hist.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

fn top2(hist: &BTreeMap<i32, usize>, total: usize) -> Estimate {
    let mut v: Vec<(i32, usize)> = hist.iter().map(|(k, n)| (*k, *n)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let (y0, n0) = v.first().copied().unwrap_or((0, 0));
    let (y1, n1) = v.get(1).copied().unwrap_or((0, 0));
    Estimate {
        yoff: y0 as f32,
        support: n0,
        total,
        runner_up: n1,
        runner_up_yoff: y1 as f32,
    }
}

/// **Estimator 2 — the pack's collision geometry.**
///
/// For each item anchor (an absolute position, so it does not move with
/// `yoff`), find every grid-block surface in its column and record the offset
/// that would put that surface just under the anchor. The mode of those
/// offsets is the height at which the map's blocks hold up the map's items.
///
/// Reads the item's **position** and never its cell, so it shares no input
/// with `cellmode` beyond the map file existing.
///
/// `grid` must be the grid-block scene built at `yoff = 0`.
pub fn rest(m: &MapFile, grid: &Scene) -> Estimate {
    let idx = Index::build(grid, 32.0);
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    let mut total = 0usize;
    for it in &m.items {
        let col = idx.column(it.pos[0], it.pos[2]);
        if col.is_empty() {
            continue;
        }
        total += 1;
        // ONE vote per surface, and the vote is sharp: the offset that would
        // put that surface exactly under this anchor. A range vote instead —
        // "any offset that puts a surface within reach below" — produces a
        // plateau several metres wide in which every candidate ties, and a
        // tie broken by sort order is not a measurement. That was the first
        // version of this function and it read 1704/1704 for four different
        // offsets at once.
        let mut voted: Vec<i32> = Vec::new();
        for (y, _mat) in &col {
            let k = (it.pos[1] - y).round() as i32;
            if k < ROW_LO * 8 || k > ROW_HI * 8 {
                continue;
            }
            if !voted.contains(&k) {
                voted.push(k);
            }
        }
        for k in voted {
            *hist.entry(k).or_insert(0) += 1;
        }
    }
    top2(&hist, total)
}

/// What the estimators together are allowed to conclude.
#[derive(Clone, Debug)]
pub enum Verdict {
    /// `rest` names an offset and every control beside it passed.
    Measured(f32),
    /// A control failed. **No number is reported.** A fit whose control fails
    /// licenses nothing.
    Unmeasured(String),
}

/// How much of the item population the winner must hold over the runner-up
/// before the mode is called an answer rather than a preference.
pub const MIN_MARGIN: f32 = 0.10;

/// The largest residual `cellmode - rest` that is explained by grid-placed
/// items sitting above their cell's base plane rather than by the two
/// estimators disagreeing.
pub const MAX_PIVOT: i32 = 4;

pub struct Report {
    pub cellmode: Estimate,
    pub rest: Estimate,
    pub verdict: Verdict,
    /// `cellmode - rest`, the systematic offset between the two.
    pub residual: i32,
    pub multiple_of_8: bool,
}

/// Estimate the map height and say what may be concluded.
///
/// **`rest` is the estimate; the other three lines are its controls, and each
/// one can fail.**
///
/// | control | what it would catch | shares a source with `rest`? |
/// |---|---|---|
/// | the winner is a whole 8 m cell row | a fit landing between rows, i.e. a placement or reader error | **no** — nothing in the sweep prefers multiples of 8, it steps by the metre over 449 candidates |
/// | the margin over the runner-up | a mode that is a preference rather than an answer | yes — it is `rest`'s own histogram |
/// | `cellmode` agrees to within a pivot height | a wrong cell convention, a wrong footprint shift, a pak read at the wrong height | **no** — `cellmode` never opens the pack and `rest` never reads a cell |
///
/// The residual is expected to be **+2** and not 0: the grid-placed items
/// whose cell is meaningful are mostly gates, and a gate's anchor sits about
/// two metres above the road it straddles. Measured across the Summer 2026
/// campaign the residual is +2 on 21 of 25 maps, 0 on two and +1/+3 on one
/// each. That is a systematic property of the item class, not two answers —
/// and reading it as two answers is exactly the mistake this table is here to
/// stop.
pub fn measure(m: &MapFile, grid: &Scene) -> Report {
    let c = cellmode(m);
    let r = rest(m, grid);
    let residual = (c.yoff - r.yoff).round() as i32;
    let multiple_of_8 = (r.yoff / 8.0).fract().abs() < 1e-3;
    let mut fail: Vec<String> = Vec::new();
    if r.total == 0 {
        fail.push("no item anchor has any grid-block surface in its column".into());
    }
    if !multiple_of_8 {
        fail.push(format!("{} is not a whole 8 m cell row", r.yoff));
    }
    if r.margin() < MIN_MARGIN {
        fail.push(format!(
            "margin over the runner-up is {:.1} % (< {:.0} %)",
            100.0 * r.margin(),
            100.0 * MIN_MARGIN
        ));
    }
    if c.total == 0 {
        fail.push("cellmode had no grid-placed item to work with".into());
    } else if !(0..=MAX_PIVOT).contains(&residual) {
        fail.push(format!(
            "cellmode says {} against rest's {}: residual {} is outside 0..{}",
            c.yoff, r.yoff, residual, MAX_PIVOT
        ));
    }
    let verdict = if fail.is_empty() {
        Verdict::Measured(r.yoff)
    } else {
        Verdict::Unmeasured(fail.join("; "))
    };
    Report { cellmode: c, rest: r, verdict, residual, multiple_of_8 }
}

impl Report {
    /// The offset, or `None` when a control failed.
    pub fn value(&self) -> Option<f32> {
        match self.verdict {
            Verdict::Measured(y) => Some(y),
            Verdict::Unmeasured(_) => None,
        }
    }

    pub fn line(&self) -> String {
        let v = match &self.verdict {
            Verdict::Measured(y) => format!("MEASURED yoff {}", y),
            Verdict::Unmeasured(why) => format!("UNMEASURED -- {}", why),
        };
        format!(
            "{}\n  rest      yoff {:>6}  {}/{} item anchors ({:.1} %), runner-up {} at {}, margin {:.1} %\n  control   whole 8 m cell row: {}\n  control   cellmode (map file only, no pack) {:>6}, residual {:+} (a gate anchor sits ~2 m over its road)\n            from {} grid-placed items, {:.1} % of them",
            v,
            self.rest.yoff,
            self.rest.support,
            self.rest.total,
            100.0 * self.rest.fraction(),
            self.rest.runner_up,
            self.rest.runner_up_yoff,
            100.0 * self.rest.margin(),
            if self.multiple_of_8 { "yes" } else { "NO" },
            self.cellmode.yoff,
            self.residual,
            self.cellmode.total,
            100.0 * self.cellmode.fraction(),
        )
    }
}
