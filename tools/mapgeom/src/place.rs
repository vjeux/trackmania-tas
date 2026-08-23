//! Placing a map's blocks and items into world space.
//!
//! `tmmaps` reads the map; this decides where each record's geometry goes.
//!
//! **The grid.** A grid-placed block names a cell `(cx, cy, cz)` and a
//! quarter-turn `dir`. Cells are 32 m in X and Z and 8 m in Y, so
//!
//! ```text
//!   world = (32*cx + lx,  8*cy + yoff + y,  32*cz + lz)
//! ```
//!
//! with `(lx, lz)` the block-local point turned by `dir` **clockwise looking
//! down** and shifted by the block's own footprint, because a rotation about
//! the cell corner moves the block off its cells:
//!
//! ```text
//!   dir=0: (x,        z)
//!   dir=1: (SZ - z,   x)
//!   dir=2: (SX - x,   SZ - z)
//!   dir=3: (z,        SX - x)
//! ```
//!
//! The opposite handedness scores measurably worse and produces an incoherent
//! route (72.4 % of a driven path inside the road, against 64.2 %), which is
//! why the convention is written down rather than guessed at each call site.
//!
//! **`yoff` is per map** and is the one number here that is not derivable from
//! the block record. See `Yoff` below.

use crate::geom::{compose, yaw, yaw_quarter, Xform};

pub const CELL_XZ: f32 = 32.0;
pub const CELL_Y: f32 = 8.0;

/// A block model's footprint in metres, as whole 32 m cells.
///
/// Read off the model's own geometry rather than its unit list, with a 15 %
/// overhang tolerance: a kerb or a sign that pokes past the last cell must not
/// buy the block another one. Where the unit list is available this should be
/// replaced by it — see MAPGEOM.md, "what is still missing".
pub fn footprint(max_x: f32, max_z: f32) -> (f32, f32) {
    fn cells(m: f32) -> f32 {
        (((m / CELL_XZ) - 0.15).ceil()).max(1.0) * CELL_XZ
    }
    (cells(max_x), cells(max_z))
}

/// The transform for a grid-placed block.
///
/// A rotation about the cell CORNER moves the block off its own cells, so it
/// has to be shifted back — and the shift is not free to choose: it is
/// determined by the quarter turn it is paired with. Pair them wrongly and
/// `dir = 0` and `dir = 2` blocks stay exactly right while every `dir = 1` and
/// `dir = 3` block moves by a whole footprint, which is invisible in the
/// height fit (the misplaced blocks are at the correct HEIGHT) and takes about
/// a third of a run off the model.
///
/// MEASURED, and the measurement is the reason this is written down: on
/// 134672 the pairing below gives 87.1 % of samples a surface against 55.8 %
/// for the mismatched one and 76.9 % for the other handedness, with the median
/// ride height unmoved at 0.029 m — and 252289, which was already at 100 %,
/// is bit-for-bit unchanged by all three.
pub fn grid_block(cell: (i32, i32, i32), dir: u8, size: (f32, f32), yoff: f32) -> Xform {
    let (sx, sz) = size;
    // dir=1 -> (lx, lz) = (SZ - z, x);  dir=3 -> (z, SX - x).
    let (steps, t) = match dir & 3 {
        0 => (0u8, [0.0, 0.0, 0.0]),
        1 => (3u8, [sz, 0.0, 0.0]),
        2 => (2u8, [sx, 0.0, sz]),
        _ => (1u8, [0.0, 0.0, sx]),
    };
    let local = yaw_quarter(steps, t);
    let origin = [
        CELL_XZ * cell.0 as f32,
        CELL_Y * cell.1 as f32 + yoff,
        CELL_XZ * cell.2 as f32,
    ];
    compose(&yaw_quarter(0, origin), &local)
}

/// The transform for a free-placed block or an item: an absolute position in
/// metres and a yaw in radians. Pitch and roll are carried through when the
/// record has them.
pub fn free(pos: [f32; 3], rot: [f32; 3]) -> Xform {
    // The map stores free rotation as (yaw, pitch, roll) in radians. Yaw
    // dominates on every map this project has looked at; pitch and roll are
    // composed after it, in that order, about the already-turned axes.
    let m = yaw(rot[0], pos);
    if rot[1] == 0.0 && rot[2] == 0.0 {
        return m;
    }
    let (sp, cp) = rot[1].sin_cos();
    let pitch = [1.0, 0.0, 0.0, 0.0, cp, sp, 0.0, -sp, cp, 0.0, 0.0, 0.0];
    let (sr, cr) = rot[2].sin_cos();
    let roll = [cr, sr, 0.0, -sr, cr, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    compose(&compose(&m, &pitch), &roll)
}

/// The map-wide vertical offset between cell rows and world metres.
///
/// `world_y = 8*cy + yoff`. It is a property of the map's DECORATION, not of
/// the block, and it is **not a whole number of cells**: fitting it to
/// multiples of 8 m leaves a residual that is a small whole number of metres,
/// and that residual is exactly what made this project's early measurements
/// look like a per-map mystery. Across 30 maps the fitted 8 m offset left the
/// car sitting 0.03 m, 2.01 m, 2.05 m, 3.09 m, 4.02 m or 5.12 m above the
/// model — the same ride height plus an integer.
///
/// So the fit is two passes: whole cells over the range TM2020 decorations
/// use, then metre by metre around the winner.
#[derive(Clone, Copy, Debug)]
pub struct Yoff(pub f32);

impl Yoff {
    /// Whole cell rows over the range TM2020 decorations use.
    pub fn coarse() -> impl Iterator<Item = f32> {
        (-40..=0).map(|k| k as f32 * CELL_Y)
    }
    /// Metre by metre around a coarse winner.
    pub fn refine(around: f32) -> impl Iterator<Item = f32> {
        (-7..=7).map(move |k| around + k as f32)
    }
}
