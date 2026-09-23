//! Authoring MediaTracker nodes from scratch — the in-game clip group of
//! `Triangles2D` HUD drawings fired by trigger zones (the MK64 minimap: the
//! course outline with a dot that hops from zone to zone as the car drives).
//!
//! Byte layouts copied from a Summer 2026 map (05: its intro's "flag colour"
//! Triangles2D blocks, its end-race "Trigger N" clips — `tmmaps mediatracker
//! MAP --hex`), field names from map-mediatracker.md §2–3:
//!
//! ```text
//! group  idx, 0x0307A000, 0x0307A003 { 10, nClips, clips…, nTriggers, triggers… }, FACADE01
//! clip   idx, 0x03079000, 0x0307900D { 1, 10, nTracks, tracks…, name, StopWhenLeave,
//!        0, StopWhenRespawn, "", 0.2f, -1 }, skippable 0x0307900E PIKS 8 { 1, 0 }, FACADE01
//! track  idx, 0x03078000, 0x03078001 { name, 10, nBlocks, blocks…, -1 },
//!        0x03078005 { 1, IsKeepPlaying 1, 0, 0, -1f, -1f }, FACADE01
//! tri2d  idx, 0x0304B000, 0x03029001 { n, n×time, n, nv, n×nv×Vec3(x,y,0), nv, nv×RGBA f32,
//!        nt, nt×Int3, 1, 0, 0, 1.0f, 0, 0i64 }, skippable 0x03029002 PIKS 4 { -1 }, FACADE01
//! trigger { -1, -1, -1, 0, condition 0, 0.0f, nCells, cells… }
//! ```
//! Screen space: x, y in −1..1 (full width / full height; a square needs the
//! aspect ratio folded into x), z 0.

use crate::mediatracker::{MediaTracker, Slot};

pub const CLASS_GROUP: u32 = 0x0307_A000;
pub const CLASS_CLIP: u32 = 0x0307_9000;
pub const CLASS_TRACK: u32 = 0x0307_8000;
pub const CLASS_TRIANGLES_2D: u32 = 0x0304_B000;
pub const NODE_END: u32 = 0xFACA_DE01;
pub const NULL_REF: u32 = 0xFFFF_FFFF;

/// A 2D drawing: screen-space vertices with RGBA colours, triangles over them.
#[derive(Clone, Debug, Default)]
pub struct Drawing {
    pub verts: Vec<([f32; 2], [f32; 4])>,
    pub tris: Vec<[u32; 3]>,
}

impl Drawing {
    /// A filled convex polygon (fan), e.g. a dot.
    pub fn polygon(&mut self, pts: &[[f32; 2]], colour: [f32; 4]) {
        let base = self.verts.len() as u32;
        for p in pts {
            self.verts.push((*p, colour));
        }
        for i in 1..pts.len() as u32 - 1 {
            self.tris.push([base, base + i, base + i + 1]);
        }
    }
    /// A disc of `n` segments.
    pub fn disc(&mut self, c: [f32; 2], r: [f32; 2], n: usize, colour: [f32; 4]) {
        let pts: Vec<[f32; 2]> = (0..n).map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            [c[0] + r[0] * a.cos(), c[1] + r[1] * a.sin()]
        }).collect();
        self.polygon(&pts, colour);
    }
    /// A polyline of half-width `hw` (per axis, for the aspect) as quads;
    /// `closed` joins the last point to the first.
    pub fn polyline(&mut self, pts: &[[f32; 2]], hw: [f32; 2], closed: bool, colour: [f32; 4]) {
        let n = pts.len();
        if n < 2 {
            return;
        }
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            let d = [b[0] - a[0], b[1] - a[1]];
            let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
            if len < 1e-6 {
                continue;
            }
            // the normal, scaled per axis so the line is `hw` wide on screen
            let nrm = [-d[1] / len * hw[0], d[0] / len * hw[1]];
            let base = self.verts.len() as u32;
            self.verts.push(([a[0] + nrm[0], a[1] + nrm[1]], colour));
            self.verts.push(([a[0] - nrm[0], a[1] - nrm[1]], colour));
            self.verts.push(([b[0] - nrm[0], b[1] - nrm[1]], colour));
            self.verts.push(([b[0] + nrm[0], b[1] + nrm[1]], colour));
            self.tris.push([base, base + 1, base + 2]);
            self.tris.push([base, base + 2, base + 3]);
            // a round joint, light: a square (the outline repeats in every zone clip)
            self.disc(b, hw, 4, colour);
        }
    }
}

/// One in-game clip: a drawing shown for `duration_s` once the car enters
/// any of `cells` (trigger-grid cells), stopping when it leaves them.
#[derive(Clone, Debug)]
pub struct HudClip {
    pub name: String,
    pub drawing: Drawing,
    pub duration_s: f32,
    pub cells: Vec<[i32; 3]>,
    pub stop_when_leave: bool,
}

struct W(Vec<u8>);
impl W {
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
}

fn write_triangles_2d(w: &mut W, idx: u32, d: &Drawing, duration_s: f32) {
    w.u32(idx);
    w.u32(CLASS_TRIANGLES_2D);
    w.u32(0x0302_9001);
    let times = [0.0f32, duration_s];
    w.u32(times.len() as u32);
    for t in times {
        w.f32(t);
    }
    w.u32(times.len() as u32);
    w.u32(d.verts.len() as u32);
    for _ in 0..times.len() {
        for (p, _) in &d.verts {
            w.f32(p[0]);
            w.f32(p[1]);
            w.f32(0.0);
        }
    }
    w.u32(d.verts.len() as u32);
    for (_, c) in &d.verts {
        for v in c {
            w.f32(*v);
        }
    }
    w.u32(d.tris.len() as u32);
    for t in &d.tris {
        for v in t {
            w.u32(*v);
        }
    }
    w.u32(1); // U01
    w.u32(0); // U02
    w.u32(0); // U03
    w.f32(1.0); // U04
    w.u32(0); // U05
    w.u32(0); // U06 (long)
    w.u32(0);
    // skippable 0x03029002: PIKS, 4 bytes, -1
    w.u32(0x0302_9002);
    w.0.extend_from_slice(b"PIKS");
    w.u32(4);
    w.u32(NULL_REF);
    w.u32(NODE_END);
}

fn write_track(w: &mut W, idx: u32, name: &str, blocks: &mut dyn FnMut(&mut W)) {
    w.u32(idx);
    w.u32(CLASS_TRACK);
    w.u32(0x0307_8001);
    w.string(name);
    w.u32(10);
    w.u32(1);
    blocks(w);
    w.u32(NULL_REF);
    w.u32(0x0307_8005);
    w.u32(1);
    w.u32(1); // IsKeepPlaying
    w.u32(0); // IsReadOnly
    w.u32(0); // IsCycling
    w.f32(-1.0);
    w.f32(-1.0);
    w.u32(NODE_END);
}

fn write_clip(w: &mut W, idx: u32, c: &HudClip, track_idx: u32, block_idx: u32) {
    w.u32(idx);
    w.u32(CLASS_CLIP);
    w.u32(0x0307_900D);
    w.u32(1); // version
    w.u32(10); // list version
    w.u32(1); // tracks
    write_track(w, track_idx, "hud", &mut |w| write_triangles_2d(w, block_idx, &c.drawing, c.duration_s));
    w.string(&c.name);
    w.u32(if c.stop_when_leave { 1 } else { 0 }); // StopWhenLeave
    w.u32(0); // U02
    w.u32(0); // StopWhenRespawn: keep the drawing through a respawn
    w.string("");
    w.f32(0.2);
    w.i32(-1);
    w.u32(0x0307_900E);
    w.0.extend_from_slice(b"PIKS");
    w.u32(8);
    w.u32(1);
    w.u32(0);
    w.u32(NODE_END);
}

/// The whole in-game group node as bytes: node indices from `first_idx`
/// (group, then per clip: clip, track, block). Returns (bytes, nodes used).
pub fn in_game_group(first_idx: u32, clips: &[HudClip]) -> (Vec<u8>, u32) {
    let mut w = W(Vec::new());
    let mut next = first_idx;
    let group_idx = next;
    next += 1;
    w.u32(group_idx);
    w.u32(CLASS_GROUP);
    w.u32(0x0307_A003);
    w.u32(10);
    w.u32(clips.len() as u32);
    for c in clips {
        let (ci, ti, bi) = (next, next + 1, next + 2);
        next += 3;
        write_clip(&mut w, ci, c, ti, bi);
    }
    w.u32(clips.len() as u32);
    for c in clips {
        for v in [-1i32, -1, -1, 0, 0] {
            w.i32(v);
        }
        w.f32(0.0);
        w.u32(c.cells.len() as u32);
        for cell in &c.cells {
            for v in cell {
                w.i32(*v);
            }
        }
    }
    w.u32(NODE_END);
    (w.0, next - first_idx)
}

/// Trigger-grid cell of a world point: the grid splits a 32×8×32 block cell
/// into `ts` boxes, row 0 at `ground`.
pub fn trigger_cell(p: [f32; 3], ts: [i32; 3], ground: f32) -> [i32; 3] {
    let unit = [32.0 / ts[0].max(1) as f32, 8.0 / ts[1].max(1) as f32, 32.0 / ts[2].max(1) as f32];
    [(p[0] / unit[0]).floor() as i32, ((p[1] - ground) / unit[1]).floor() as i32, (p[2] / unit[2]).floor() as i32]
}

impl MediaTracker {
    /// Put an authored in-game group in place of the current one. `first_idx`
    /// is the first free node index of the file (the header's node count,
    /// which the caller bumps by the returned node count).
    pub fn set_in_game_authored(&mut self, first_idx: u32, clips: &[HudClip]) -> u32 {
        let (bytes, used) = in_game_group(first_idx, clips);
        self.in_game = Slot::Authored(bytes);
        used
    }
}

/// A CameraCustom key (chunk 0x030A2006 v4; the tangents are the game's
/// hermite handles, zero = the game's own smoothing).
#[derive(Clone, Debug)]
pub struct CamKey {
    pub time: f32,
    pub position: [f32; 3],
    /// pitch (positive looks down), yaw (0 = +z, positive toward +x:
    /// atan2(dx, dz) — the Summer 05 end-race camera), roll
    pub pitch_yaw_roll: [f32; 3],
    pub fov: f32,
}

fn write_camera_state(w: &mut W, position: [f32; 3], pyr: [f32; 3], fov: f32) {
    for v in position {
        w.f32(v);
    }
    for v in pyr {
        w.f32(v);
    }
    w.f32(fov);
    for _ in 0..3 {
        w.f32(0.0); // target position
    }
    w.f32(0.05);
    w.f32(1.0);
}

fn write_camera_custom(w: &mut W, idx: u32, keys: &[CamKey]) {
    w.u32(idx);
    w.u32(0x030A_2000);
    w.u32(0x030A_2006);
    w.u32(4);
    w.u32(keys.len() as u32);
    for k in keys {
        w.f32(k.time);
        w.i32(1); // interpolation
        w.i32(0); // anchor rot
        w.i32(-1); // anchor
        w.i32(1); // anchor vis
        w.i32(-1); // target
        write_camera_state(w, k.position, k.pitch_yaw_roll, k.fov);
        write_camera_state(w, [0.0; 3], [0.0; 3], 0.0); // left tangent
        write_camera_state(w, [0.0; 3], [0.0; 3], 0.0); // right tangent
    }
    w.u32(NODE_END);
}

/// An intro clip: one track of CameraCustom blocks (one per shot, cuts
/// between them). Node indices from `first_idx`; returns (bytes, nodes used).
pub fn intro_clip(first_idx: u32, shots: &[Vec<CamKey>]) -> (Vec<u8>, u32) {
    let mut w = W(Vec::new());
    let mut next = first_idx;
    let clip_idx = next;
    next += 1;
    let track_idx = next;
    next += 1;
    w.u32(clip_idx);
    w.u32(CLASS_CLIP);
    w.u32(0x0307_900D);
    w.u32(1);
    w.u32(10);
    w.u32(1);
    // the track, by hand (several blocks)
    w.u32(track_idx);
    w.u32(CLASS_TRACK);
    w.u32(0x0307_8001);
    w.string("Custom camera");
    w.u32(10);
    w.u32(shots.len() as u32);
    for keys in shots {
        let bi = next;
        next += 1;
        write_camera_custom(&mut w, bi, keys);
    }
    w.u32(NULL_REF);
    w.u32(0x0307_8005);
    w.u32(1);
    w.u32(1);
    w.u32(0);
    w.u32(0);
    w.f32(-1.0);
    w.f32(-1.0);
    w.u32(NODE_END);
    w.string("");
    w.u32(0); // StopWhenLeave
    w.u32(0);
    w.u32(1); // StopWhenRespawn
    w.string("");
    w.f32(0.2);
    w.i32(-1);
    w.u32(0x0307_900E);
    w.0.extend_from_slice(b"PIKS");
    w.u32(8);
    w.u32(1);
    w.u32(0);
    w.u32(NODE_END);
    (w.0, next - first_idx)
}

impl MediaTracker {
    /// Put an authored intro clip in place of the current one.
    pub fn set_intro_authored(&mut self, first_idx: u32, shots: &[Vec<CamKey>]) -> u32 {
        let (bytes, used) = intro_clip(first_idx, shots);
        self.intro = Slot::Authored(bytes);
        used
    }
}
