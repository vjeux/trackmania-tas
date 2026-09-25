//! The MK64 CPU racers as Trackmania ghosts (vjeux, 2026-09-25: "7 ghosts
//! that don't have collisions but different skins … generate the trajectory
//! of these ghosts from actual mario kart CPUs").
//!
//! What the game's CPU does (src/cpu_vehicles_camera_path.c, US ROM):
//!
//! * it follows one of the course's TRACK PATHS — `gCoursePathTable[course]`
//!   holds up to four (`d_course_<x>_track_path`, `_track_path_2..4`; Yoshi
//!   Valley's four branches, Koopa Beach's shortcut) and a CPU is re-homed to
//!   whichever path is nearest to it (`update_player_path_selection`);
//! * it steers to a lateral "track position factor" (−1..1 across the road),
//!   0 by default, pushed by the course's CPU BEHAVIOUR table
//!   (`cpu_BehaviourLUT`, segment 0xD): `DRIVE_LEFT` −0.6, `DRIVE_OUTER`
//!   +0.6, `DRIVE_CENTER` 0 over path-point ranges, and `BEHAVIOUR_1` ranges
//!   where it DRIFTS through the bend (`func_80011EC0`);
//! * its speed is regulated toward per-course targets (`cpu_NormalTargetSpeed`
//!   / `cpu_CurveTargetSpeed` / `cpu_OffTrackTargetSpeed`, [course][cc], units
//!   per frame — 150cc normal 6.167, curve 5.75 on most courses) with the
//!   rubber band `CPU_FAST_EFFECT` accelerating it toward the kart's physical
//!   cap (`gKartTopSpeedTable` 9.0 units/frame) whenever the human is ahead.
//!   Top speeds 150cc: 320 for Mario/Luigi/D.K./Wario/Bowser, 324 for the light
//!   Yoshi/Toad/Peach (`gTopSpeed150cc`).
//!
//! What a ghost can carry of that: the route, the lateral behaviours, the
//! drift zones, the grid start and a pace. The rubber band depends on the
//! human, so each CPU gets a fixed pace: the kart cap (9 units/frame = 148 km/h
//! in the map's scale) times the character's top-speed ratio, the field spread
//! 2 % per grid slot (the leader fastest), the bend limit doing the cornering.

use crate::course::Course;
use crate::ghost::{drive_route, Drive, Line, Trajectory};
use crate::mesh::{CollTri, Frame};

/// The eight drivers: display name, skin zip (in `Skins\Models\CarSport`),
/// 150cc top speed from `gTopSpeed150cc`.
pub const DRIVERS: [(&str, &str, f32); 8] = [
    ("Mario", "MK64 Mario.zip", 320.0),
    ("Luigi", "MK64 Luigi.zip", 320.0),
    ("Yoshi", "MK64 Yoshi.zip", 324.0),
    ("Toad", "MK64 Toad.zip", 324.0),
    ("Donkey Kong", "MK64 Donkey Kong.zip", 320.0),
    ("Wario", "MK64 Wario.zip", 320.0),
    ("Peach", "MK64 Peach.zip", 324.0),
    ("Bowser", "MK64 Bowser.zip", 320.0),
];

/// `gKartTopSpeedTable`: the physical speed cap, units per frame at 30 Hz.
pub const KART_CAP_UNITS_PER_FRAME: f32 = 9.0;

/// One CPU behaviour row (`CPUBehaviour`: path point start, end, type).
#[derive(Clone, Copy, Debug)]
pub struct Behaviour {
    pub start: i16,
    pub end: i16,
    pub kind: i32,
}

pub const BEHAVIOUR_DRIFT: i32 = 1;
pub const BEHAVIOUR_DRIVE_CENTER: i32 = 3;
pub const BEHAVIOUR_DRIVE_LEFT: i32 = 4;
pub const BEHAVIOUR_DRIVE_OUTER: i32 = 5;

/// The course order of the game's tables (`gCurrentCourseId`).
pub const COURSE_IDS: [&str; 15] = [
    "mario_raceway",
    "choco_mountain",
    "bowsers_castle",
    "banshee_boardwalk",
    "yoshi_valley",
    "frappe_snowland",
    "koopa_troopa_beach",
    "royal_raceway",
    "luigi_raceway",
    "moo_moo_farm",
    "toads_turnpike",
    "kalimari_desert",
    "sherbet_land",
    "rainbow_road",
    "wario_stadium",
];

/// `cpu_BehaviourLUT` in the US ROM: 21 segment-0xD pointers at RAM
/// 0x800DC720 (ROM = RAM − 0x7FFFF400; found by the `nullPath` marker
/// `{0x8000,0,0,0}` right after it, 2026-09-25); segment 0xD is the MIO0
/// block at ROM 0x132B50.
const LUT_ROM: usize = 0x800D_C720 - 0x7FFF_F400;
const SEG_D_ROM: usize = 0x132B50;

fn mio0(d: &[u8]) -> Vec<u8> {
    let be = |o: usize| u32::from_be_bytes(d[o..o + 4].try_into().unwrap());
    let (n, lo, ro) = (be(4) as usize, be(8) as usize, be(12) as usize);
    let (mut out, mut bp, mut lp, mut rp) = (Vec::with_capacity(n), 16usize, lo, ro);
    let (mut bits, mut nb) = (0u32, 0u32);
    while out.len() < n {
        if nb == 0 {
            bits = be(bp);
            bp += 4;
            nb = 32;
        }
        if bits & 0x8000_0000 != 0 {
            out.push(d[rp]);
            rp += 1;
        } else {
            let v = u16::from_be_bytes([d[lp], d[lp + 1]]);
            lp += 2;
            let (ln, off) = ((v >> 12) as usize + 3, (v & 0xfff) as usize + 1);
            for _ in 0..ln {
                let b = out[out.len() - off];
                out.push(b);
            }
        }
        bits <<= 1;
        nb -= 1;
    }
    out.truncate(n);
    out
}

/// The CPU behaviour table of `dir` from the ROM.
pub fn behaviours(rom: &[u8], dir: &str) -> Vec<Behaviour> {
    let Some(ci) = COURSE_IDS.iter().position(|d| *d == dir) else { return Vec::new() };
    if rom.len() < SEG_D_ROM + 16 || rom.len() < LUT_ROM + 4 * 21 {
        return Vec::new();
    }
    let seg = mio0(&rom[SEG_D_ROM..]);
    let ptr = u32::from_be_bytes(rom[LUT_ROM + 4 * ci..LUT_ROM + 4 * ci + 4].try_into().unwrap());
    if ptr >> 24 != 0x0D {
        return Vec::new();
    }
    let mut o = (ptr & 0x00FF_FFFF) as usize;
    let mut out = Vec::new();
    while o + 8 <= seg.len() && out.len() < 100 {
        let start = i16::from_be_bytes([seg[o], seg[o + 1]]);
        let end = i16::from_be_bytes([seg[o + 2], seg[o + 3]]);
        let kind = i32::from_be_bytes(seg[o + 4..o + 8].try_into().unwrap());
        o += 8;
        if start == -1 && end == -1 {
            break;
        }
        out.push(Behaviour { start, end, kind });
    }
    out
}

/// The CPU routes of a course in TM space: the main path first, then the
/// `_track_path_N` alternates in order.
pub fn routes(c: &Course, frame: &Frame) -> Vec<Vec<[f32; 3]>> {
    let mut r: Vec<Vec<[f32; 3]>> = vec![c.path.iter().map(|p| frame.to_tm(p.pos)).collect()];
    let mut alts: Vec<(&String, &Vec<[i16; 3]>)> = c.other_paths.iter().filter(|(nm, _)| nm.contains("_track_path_")).collect();
    alts.sort();
    for (_, alt) in alts {
        r.push(alt.iter().map(|p| frame.to_tm(*p)).collect());
    }
    r
}

pub struct Cpu {
    pub name: &'static str,
    pub skin: &'static str,
    pub route: usize,
    pub line: Line,
    pub drive: Drive,
}

/// The seven CPUs for a race where the human drives `player` (a DRIVERS
/// index): grid slots 1..7 (the human is 8th, MK64's first GP race), routes
/// round-robin, the course's behaviours as lateral targets and drift zones.
/// `half_width_m` scales the ±0.6 track-position factor.
pub fn field(c: &Course, frame: &Frame, rom: &[u8], player: usize, base: &Drive, half_width_m: f32) -> Vec<Cpu> {
    let n_routes = routes(c, frame).len().max(1);
    let n_path = c.path.len().max(1) as f32;
    let beh = behaviours(rom, &c.dir);
    let mut lateral = Vec::new();
    let mut drift = Vec::new();
    for b in &beh {
        let (a, e) = (b.start as f32 / n_path, b.end as f32 / n_path);
        match b.kind {
            BEHAVIOUR_DRIVE_LEFT => lateral.push((a, e, -0.6 * half_width_m)),
            BEHAVIOUR_DRIVE_OUTER => lateral.push((a, e, 0.6 * half_width_m)),
            BEHAVIOUR_DRIVE_CENTER => lateral.push((a, e, 0.0)),
            BEHAVIOUR_DRIFT => drift.push((a, e)),
            _ => {}
        }
    }
    // the kart cap in the map's scale (units/frame × 30 Hz × m/unit)
    let cap_mps = KART_CAP_UNITS_PER_FRAME * 30.0 * frame.scale;
    let mut out = Vec::new();
    let mut slot = 0usize;
    for (k, (name, skin, top)) in DRIVERS.iter().enumerate() {
        if k == player {
            continue;
        }
        // grid: slot 1 = front left … 7 = back left; rows 5 m apart, columns ±1.6 m
        let (row, col) = (slot / 2, slot % 2);
        let start_ahead_m = (3 - row.min(3)) as f32 * 5.0;
        let base_lateral = if col == 0 { -1.6 } else { 1.6 };
        let vmax = cap_mps.min(base.vmax) * (top / 320.0) * (1.0 - 0.02 * slot as f32);
        out.push(Cpu {
            name,
            skin,
            route: slot % n_routes,
            line: Line { lateral: lateral.clone(), base_lateral, start_ahead_m, drift: drift.clone(), drift_deg: 16.0 },
            drive: Drive { vmax, ..*base },
        });
        slot += 1;
    }
    out
}

/// Drive one CPU: its route with its line.
pub fn trajectory(c: &Course, frame: &Frame, soup: &[CollTri], cpu: &Cpu, laps: u32, dt_ms: i64) -> Trajectory {
    let rs = routes(c, frame);
    let route = &rs[cpu.route.min(rs.len() - 1)];
    drive_route(c, frame, soup, laps, &cpu.drive, dt_ms, route, &cpu.line)
}
