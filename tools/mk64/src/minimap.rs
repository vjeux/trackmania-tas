//! The MK64 minimap in Trackmania: the course outline with a dot for the
//! player, drawn by the MediaTracker. The MediaTracker cannot read the car's
//! position, but its in-game clips fire on TRIGGER ZONES — so the lap is cut
//! into zones along the centre path, each with its own clip drawing the
//! outline and the dot at that zone, stopping when the car leaves the zone.
//! The dot hops zone to zone as you drive (vjeux, 2026-09-22: "a topdown
//! view of the course with a dot for where you are").
//!
//! Everything is 2D triangles (`Triangles2D`), no images to host.

use tmmaps::mtauthor::{trigger_cell, Drawing, HudClip};

/// Screen-space layout: the box the course is fitted into (centre, max
/// half-extents in screen units where y −1..1 is the full height), and the
/// x scale that keeps world squares square on a 16:9 screen.
pub const CENTRE: [f32; 2] = [0.80, 0.40];
pub const HALF_MAX: [f32; 2] = [0.16, 0.32];
pub const ASPECT_X: f32 = 9.0 / 16.0;
pub const LINE_HW: f32 = 0.006;
pub const DOT_R: f32 = 0.018;
pub const WHITE: [f32; 4] = [0.95, 0.95, 0.95, 0.9];
pub const SHADOW: [f32; 4] = [0.05, 0.05, 0.05, 0.6];
pub const DOT: [f32; 4] = [1.0, 0.15, 0.1, 1.0];
/// A zone's clip runs at most this long (an hour: never mid-lap).
pub const DURATION_S: f32 = 3600.0;

/// World (TM frame) → screen: north (−z) up.
pub struct Fit {
    pub centre_w: [f32; 2],
    pub scale: f32,
}

impl Fit {
    pub fn new(path: &[[f32; 3]]) -> Fit {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in path {
            lo[0] = lo[0].min(p[0]);
            hi[0] = hi[0].max(p[0]);
            lo[1] = lo[1].min(p[2]);
            hi[1] = hi[1].max(p[2]);
        }
        let ext = [hi[0] - lo[0], hi[1] - lo[1]];
        // screen half-extent = ext/2/scale (× ASPECT_X for x) ≤ HALF_MAX
        let scale = (ext[0] * ASPECT_X / 2.0 / HALF_MAX[0]).max(ext[1] / 2.0 / HALF_MAX[1]).max(1e-3);
        Fit { centre_w: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0], scale }
    }
    pub fn screen(&self, p: [f32; 3]) -> [f32; 2] {
        [CENTRE[0] + (p[0] - self.centre_w[0]) / self.scale * ASPECT_X, CENTRE[1] - (p[2] - self.centre_w[1]) / self.scale]
    }
}

/// The clips: `zones` zones over the path (TM frame, a closed lap).
pub fn clips(path: &[[f32; 3]], zones: usize, ground: f32, ts: [i32; 3]) -> Vec<HudClip> {
    let n = path.len();
    if n < 4 || zones == 0 {
        return Vec::new();
    }
    let fit = Fit::new(path);
    // the outline, subsampled to ~200 points
    let step = (n / 90).max(1);
    let outline: Vec<[f32; 2]> = (0..n).step_by(step).map(|i| fit.screen(path[i])).collect();
    let hw = [LINE_HW * ASPECT_X, LINE_HW];
    let mut claimed: std::collections::HashSet<[i32; 3]> = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(zones);
    // (a separate always-on outline clip sharing the zones' cells was tried:
    // a cell in two triggers fires only the FIRST clip, so the dots never
    // showed — the outline rides in every zone clip instead, kept light)
    for z in 0..zones {
        let i0 = z * n / zones;
        let i1 = ((z + 1) * n / zones).max(i0 + 1).min(n);
        // the zone's cells: a 3×4×3 block of trigger cells around every point
        let mut cells: Vec<[i32; 3]> = Vec::new();
        for i in i0..i1 {
            let c = trigger_cell(path[i], ts, ground);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    for dy in -1..=2 {
                        let cell = [c[0] + dx, c[1] + dy, c[2] + dz];
                        if claimed.insert(cell) {
                            cells.push(cell);
                        }
                    }
                }
            }
        }
        if cells.is_empty() {
            continue;
        }
        let mid = fit.screen(path[(i0 + i1) / 2]);
        let mut d = Drawing::default();
        let shadow: Vec<[f32; 2]> = outline.iter().map(|p| [p[0] + 0.003 * ASPECT_X, p[1] - 0.003]).collect();
        d.polyline(&shadow, hw, true, SHADOW);
        d.polyline(&outline, hw, true, WHITE);
        d.disc([mid[0] + 0.002 * ASPECT_X, mid[1] - 0.002], [DOT_R * ASPECT_X, DOT_R], 12, SHADOW);
        d.disc(mid, [DOT_R * ASPECT_X, DOT_R], 12, DOT);
        // the game's screen x runs RIGHT-to-left in Triangles2D (Luigi's outline
        // drew mirrored on the left at x = +0.8): mirror at the end
        for (p, _) in d.verts.iter_mut() {
            p[0] = -p[0];
        }
        out.push(HudClip { name: format!("minimap {z}"), drawing: d, duration_s: DURATION_S, cells, stop_when_leave: true });
    }
    out
}
