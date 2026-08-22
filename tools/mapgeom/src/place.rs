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
pub fn grid_block(cell: (i32, i32, i32), dir: u8, size: (f32, f32), yoff: f32) -> Xform {
    let (sx, sz) = size;
    // The shift that keeps the turned block on its own cells.
    let t = match dir & 3 {
        0 => [0.0, 0.0, 0.0],
        1 => [sz, 0.0, 0.0],
        2 => [sx, 0.0, sz],
        _ => [0.0, 0.0, sx],
    };
    let local = yaw_quarter(dir, t);
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
/// the block: two maps in this project measure −120 m and −40 m. Deriving it
/// from the decoration id is unfinished, so it is a flag with a calibration
/// path — `mapgeom yoff` fits it from a ghost that is known to have driven the
/// map, by the height that puts the most ground-contact samples on a surface.
#[derive(Clone, Copy, Debug)]
pub struct Yoff(pub f32);

impl Yoff {
    /// The candidate offsets a fit tries: whole 8 m cell rows over the range
    /// TM2020 decorations use.
    pub fn candidates() -> impl Iterator<Item = f32> {
        (-40..=0).map(|k| k as f32 * CELL_Y)
    }
}
