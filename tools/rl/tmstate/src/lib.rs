//! `tmstate` — the plain data types shared by the tm-player arms.
//!
//! Owner: ENV arm. This file is the text of `coord/INTERFACES.md` §tmstate, verbatim, so that the DATA and
//! LEARN arms compile against it from day 0. std + serde only; NO engine dependencies, ever. Change only by
//! agreement recorded in INTERFACES.md, and bump `STATE_VERSION` when the wire layout of `CarState` changes.

use serde::{Deserialize, Serialize};

/// v2 (2026-09-06, ENV): `car: u8` joins the struct in what was the padding after `finished`, so the
/// 100-byte layout is unchanged and a v1 reader sees 0 (= Stadium) there; the wheel/gear/rpm/turbo fields
/// are now FILLED by the env (G3: the live `CSceneVehicleVisState`), with the PHASE convention below.
pub const STATE_VERSION: u32 = 3;

/// LABEL CONVENTION (2026-09-07, the three-way check on Summer 2026 - 01 WR and Summer 2026 - 02 rank-1): a record
/// labelled `race_ms = T` holds the PHYSICS state (pos, vel, quat, speed) at race time T -- the state before input
/// record `(T - start_offset) / 10` is read -- and **the ghost's own telemetry sample stamped T, `fk regen
/// --dump-truth` row T and the env's row T are the SAME tick**: pairwise |dpos| 0.1-0.5 mm at dt = 0, 0.8-1.0 m
/// (one tick) at +-10 ms. DATA's telemetry-derived labels therefore need NO lag (`--lag 0`), and the vis-derived
/// fields (gear, rpm, wheel_*, turbo, car) in row T are exactly what sample T carries (`tmenv wheels-control`
/// phase 0 ms on 4 maps). Measured per map by the controls, never assumed.
pub const VIS_PHASE_MS_DEFAULT: i32 = 0;

/// One 10 ms tick of ground-truth car state. Units: metres, m/s, radians, seconds. World frame = the map's
/// (x east, y UP, z). Fields the source cannot provide are NaN (floats) / u8::MAX (small ints) — never zero.
///
/// `repr(C)` because the dataset shards carry this struct in its fixed C layout (INTERFACES.md §Dataset):
/// 100 bytes, little-endian, padding zeroed — see `tmrl::shard` / `tmdata` for the byte-exact writer.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarState {
    pub race_ms: i32,            // engine race clock at the END of this tick (negative during countdown)
    pub pos: [f32; 3],
    pub vel: [f32; 3],           // world-frame velocity
    pub quat: [f32; 4],          // (w, x, y, z), car orientation, world frame
    pub ang_vel: [f32; 3],       // car frame, rad/s (NaN if not measured)
    pub speed: f32,              // |vel| unless the source has a better one
    pub gear: u8,                // 0..=6, u8::MAX unknown
    pub rpm: f32,                // NaN unknown
    pub wheel_contact: [u8; 4],  // FL, FR, RL, RR: 0 air, 1 ground, u8::MAX unknown
    pub wheel_material: [u8; 4], // engine surface id, u8::MAX unknown
    pub wheel_slip: [f32; 4],    // NaN unknown
    pub turbo: f32,              // NaN unknown
    pub cps: u8,                 // checkpoints credited so far (engine-authoritative when available)
    pub finished: bool,
    pub car: u8,                 // v2: the vehicle KIND 0 Stadium, 1 Snow, 2 Rally, 3 Desert (the model fingerprint at phy+0x1d14 since 2026-09-07; before that the participant slot, which is the kind only on Stadium maps); u8::MAX unknown
    // --- v3: CarState.effects (INPUT arm EFFECTS.md; all from the vis state the env already gathers) ---
    pub effects: u8,             // v3 bit flags: 0x01 turbo, 0x02 ground contact, 0x04 reactor ground mode, 0x08 reactor inputs-x, 0x80 KNOWN (0 = unknown; lives in v2's padding byte at offset 99)
    pub reactor_lvl: u8,         // v3: reactor boost level 0/1/2, u8::MAX unknown
    pub reactor_type: u8,        // v3: 1 down, 2 up, 0 none, u8::MAX unknown
    pub boost_enum: u8,          // v3: boost enum (u32(+0x19c) & 7), u8::MAX unknown
    pub car_slot: u8,            // v3 (was _pad3): the participant vehicle slot the car came from, 0..3; u8::MAX unknown -- NOT a kind
    pub reactor_air: [f32; 3],   // v3: reactor air control, NaN unknown
    pub sim_time_coef: f32,      // v3: simulation time coefficient (slow-motion), 1.0 normally, NaN unknown
}

/// One 10 ms tick of driver input.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Action {
    pub steer: i8, // -127..=127
    pub gas: bool,
    pub brake: bool,
}

/// The route the observation is computed against. Source-agnostic: cartographer route, or the field's median
/// line, or the WR trajectory. Arc length s is monotone along `pts`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackGeom {
    pub geom_version: u32,
    pub map_uid: String,
    pub pts: Vec<[f32; 3]>,   // centreline, resampled every ~2 m
    pub half_width: Vec<f32>, // per point
    pub s: Vec<f32>,          // cumulative arc length per point
    pub gates: Vec<Gate>,     // in race order: checkpoints then finish
    pub spawn: [f32; 3],
    pub spawn_yaw: f32,
    pub source: String, // "cartographer" | "field-median" | "wr-trajectory" | "router-*"
    /// ROUTE FINDER additions (INTERFACES.md "Routes as data"). None = a plain centreline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs: Option<Vec<Leg>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<RouteMeta>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateKind {
    Checkpoint,
    Finish,
    Multilap,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    pub kind: GateKind,
    pub centre: [f32; 3],
    pub normal: [f32; 3],
    pub half_width: f32,
    pub s: f32,
    /// Index of this gate in the .Map.Gbx per `tmmaps waypoints` (u32::MAX unknown) — what makes gate orders
    /// comparable across sources.
    #[serde(default = "u32_max")]
    pub map_waypoint: u32,
}

fn u32_max() -> u32 {
    u32::MAX
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionClass {
    Road,
    Jump,
    Drop,
    Cut,
    Wallride,
    Launcher,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LegEvidence {
    Human { runs: u32, best_ms: i32 },
    Rollout { reached: u32, tried: u32 },
    Predicted,
    Driven { ms: i32, ghost_md5: String },
}

/// One leg of a route: ends at `gates[gate_idx]`; leg 0 starts at `spawn`. `legs.len() == gates.len()`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leg {
    pub gate_idx: u32,
    pub map_waypoint: u32,
    pub s_start: f32,
    pub s_end: f32,
    pub connection: ConnectionClass,
    #[serde(with = "nanf::arr2")]
    pub arrival_speed: [f32; 2],
    #[serde(with = "nanf::arr3")]
    pub arrival_heading: [f32; 3],
    #[serde(with = "nanf::scalar")]
    pub arrival_heading_tol: f32,
    #[serde(with = "nanf::arr2")]
    pub arrival_height: [f32; 2],
    #[serde(with = "nanf::scalar")]
    pub p_reach: f32, // NaN if not measured
    pub expected_ms: i32, // -1 unknown
    pub evidence: LegEvidence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RouteStatus {
    Hypothesis,
    Certified { ms: i32, ghost_md5: String, oracle_box: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteMeta {
    pub route_version: u32,
    pub source: String,
    pub rank: u32,
    pub predicted_ms: i32,
    pub status: RouteStatus,
    pub gate_order: Vec<u32>, // map waypoint ids, finish last
    pub produced_by: String,
}

impl CarState {
    /// Every field "unknown", per the NaN / u8::MAX convention. Start from this, never from zeroes.
    pub fn unknown() -> CarState {
        CarState {
            race_ms: 0,
            pos: [f32::NAN; 3],
            vel: [f32::NAN; 3],
            quat: [f32::NAN; 4],
            ang_vel: [f32::NAN; 3],
            speed: f32::NAN,
            gear: u8::MAX,
            rpm: f32::NAN,
            wheel_contact: [u8::MAX; 4],
            wheel_material: [u8::MAX; 4],
            wheel_slip: [f32::NAN; 4],
            turbo: f32::NAN,
            cps: 0,
            finished: false,
            car: u8::MAX,
            effects: 0,
            reactor_lvl: u8::MAX,
            reactor_type: u8::MAX,
            boost_enum: u8::MAX,
            car_slot: u8::MAX,
            reactor_air: [f32::NAN; 3],
            sim_time_coef: f32::NAN,
        }
    }
}

impl Action {
    pub const COAST: Action = Action { steer: 0, gas: false, brake: false };
}

impl TrackGeom {
    pub fn length(&self) -> f32 {
        self.s.last().copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dataset writer depends on this exact size; a field added without a version bump must fail here.
    #[test]
    fn car_state_is_120_bytes_repr_c() {
        // v3 = the 100-byte v2 record as an identical prefix (car at 98, the
        // effects flags in v2's padding byte 99) + 20 bytes of effects.
        assert_eq!(std::mem::size_of::<CarState>(), 120);
        assert_eq!(std::mem::size_of::<Action>(), 3);
        assert_eq!(STATE_VERSION, 3);
        let s = CarState::unknown();
        let base = &s as *const CarState as usize;
        assert_eq!(&s.finished as *const bool as usize - base, 97);
        assert_eq!(&s.car as *const u8 as usize - base, 98);
        assert_eq!(&s.effects as *const u8 as usize - base, 99);
        assert_eq!(&s.reactor_lvl as *const u8 as usize - base, 100);
        assert_eq!(&s.reactor_air as *const [f32; 3] as usize - base, 104);
        assert_eq!(&s.sim_time_coef as *const f32 as usize - base, 116);
    }
}

/// JSON has no NaN: an "unknown" f32 serializes as `null` and reads back as NaN, so route/geom files interchange
/// with the ROUTE FINDER's `tmroute::types::nanf`. Use `#[serde(with = "nanf::scalar")]` / `arr2` / `arr3`.
pub mod nanf {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    fn to_opt(x: f32) -> Option<f32> {
        if x.is_nan() {
            None
        } else {
            Some(x)
        }
    }

    pub mod scalar {
        use super::*;
        pub fn serialize<S: Serializer>(x: &f32, s: S) -> Result<S::Ok, S::Error> {
            to_opt(*x).serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
            Ok(Option::<f32>::deserialize(d)?.unwrap_or(f32::NAN))
        }
    }

    macro_rules! arr {
        ($name:ident, $n:expr) => {
            pub mod $name {
                use super::*;
                pub fn serialize<S: Serializer>(x: &[f32; $n], s: S) -> Result<S::Ok, S::Error> {
                    let v: Vec<Option<f32>> = x.iter().map(|&f| to_opt(f)).collect();
                    v.serialize(s)
                }
                pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; $n], D::Error> {
                    let v = Vec::<Option<f32>>::deserialize(d)?;
                    if v.len() != $n {
                        return Err(serde::de::Error::custom(format!("expected {} floats, got {}", $n, v.len())));
                    }
                    let mut out = [f32::NAN; $n];
                    for (o, x) in out.iter_mut().zip(v) {
                        *o = x.unwrap_or(f32::NAN);
                    }
                    Ok(out)
                }
            }
        };
    }
    arr!(arr2, 2);
    arr!(arr3, 3);
}
