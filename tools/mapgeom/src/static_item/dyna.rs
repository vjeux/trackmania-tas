//! The moving part of a prefab: `CPlugDynaObjectModel` (0x09144000, the
//! object that moves — a pusher's piston, a rotor's disc, a flag's cloth) and
//! `NPlugDyna_SKinematicConstraint` (0x2F0CA000, how it moves). Neither class
//! has chunk framing: the class IS the struct, read and written field by
//! field, so an inline copy inside an item round-trips byte for byte.
//!
//! Layouts read off the Stadium pack (2026-09-07):
//!
//! * `ObstaclePusher8mPiston.DynaObject.Gbx` (version 13): version, IsStatic,
//!   DynamizeOnSpawn, Mesh ref, DynaShape ref (`MoveShape`, moves with the
//!   object), StaticShape ref (`HitShape`), then 43 bytes carried verbatim
//!   (break speed 100 km/h, mass 10, two light durations 5/7, two words 1/1,
//!   a u16 4, four words 1/10/0/0, a byte 0), LocAnim ref, 8 bytes, WaterModel
//!   ref. The rotor's file is identical past its refs.
//! * `AnimPusher8mLevel1.KinematicConstraint.Gbx` (127 bytes): version 0,
//!   sub-version 3, TransAnimFunc, RotAnimFunc (each: a word 1, then N sub
//!   functions of {u8 ease, u8 reverse, u32 duration ms}), ShaderTcType,
//!   ShaderTcVersion, N shader keyframes of {u32 duration ms, u32 sub
//!   texture} (the countdown light strip: 6 frames over the pusher's 4.8 s
//!   cycle), for type 1 (TransSub) the 4 words {NbSubTexture,
//!   NbSubTexturePerLine, NbSubTexturePerColumn, TopToBottom}, then the ranges:
//!   u8 TransAxis, f32 TransMin, f32 TransMax, u8 RotAxis, f32 AngleMinDeg,
//!   f32 AngleMaxDeg. The Level1 pusher translates along Z from -0 to 8 m in
//!   2 s linear out / 2 s back; the rotor turns about Z from 180 to -180 in 4 s.
//!   The prefab's own `KinematicConstraints\ObstaclePusher8m` has a ZERO range:
//!   the item's Level modifier (a game skin) swaps it for the `Modifier\
//!   ItemObstacle\Anim<X>Level<N>` file, like it swaps the materials.
//!
//! Scaling a constraint = scaling its translation range; angles and durations
//! stay (a half-size rotor still turns once per period).
//!
//! What the game accepts, measured on Summer 15 lineups (2026-09-07):
//!
//! * the prefab sits DIRECTLY under `CGameItemModel` (the pack's layout);
//!   wrapped in a `CGameCommonItemEntityModel` the item is dropped silently;
//! * everything inline — the game resolves no pack reference from an
//!   embedded item (the pack item re-embedded under a new ident is dropped);
//! * a dyna object with a NULL DynaShape crashes the client at load
//!   (Trackmania.exe+0xb7088c reading NULL+0x38); a mesh, a convex polyhedron
//!   or a compound all serve;
//! * `NPlugDyna_SPrefabConstraintParams.Ent2` is the RANK of the dyna object
//!   among the prefab's dyna entities (the pack's ObstacleRotor24mWing90X2
//!   lists the constraint first and still says Ent2 = 0), Ent1 = -1 the world;
//! * translations and rotations both animate in the editor, at scale 1 and
//!   0.5, with the translation range scaled and the periods kept; a pusher's
//!   piston pushed by the rotor's constraint swings about the item origin,
//!   so the pivot is the entity origin, not the hull's centre.

use super::{read_ref, write_ref, Rd, Ref, Wr, R};

pub const C_DYNA_OBJECT_MODEL: u32 = 0x09144000;
pub const C_KINEMATIC_CONSTRAINT: u32 = 0x2F0CA000;

/// Entity params chunk ids met next to these classes in a prefab.
pub const P_DYNA_INSTANCE: i32 = 0x2F0B6000;
pub const P_CONSTRAINT: i32 = 0x2F0C8000;

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugDynaObjectModel {
    pub version: u32,
    pub is_static: u32,
    pub dynamize_on_spawn: u32,
    pub mesh: Ref,
    /// The hull that moves with the object (`MoveShape`).
    pub dyna_shape: Ref,
    /// The hull that stays (`HitShape`).
    pub static_shape: Ref,
    /// 43 bytes: break speed, mass, light durations, flags — verbatim.
    pub tail_a: Vec<u8>,
    pub loc_anim: Ref,
    pub tail_b: [u8; 8],
    pub water_model: Ref,
}

impl CPlugDynaObjectModel {
    pub fn parse(r: &mut Rd) -> R<CPlugDynaObjectModel> {
        let version = r.u32()?;
        if version != 13 {
            return Err(format!("CPlugDynaObjectModel version {version} (only 13 is read)"));
        }
        let is_static = r.u32()?;
        let dynamize_on_spawn = r.u32()?;
        let mesh = read_ref(r)?;
        let dyna_shape = read_ref(r)?;
        let static_shape = read_ref(r)?;
        let tail_a = r.take(43)?.to_vec();
        let loc_anim = read_ref(r)?;
        let tail_b: [u8; 8] = r.take(8)?.try_into().unwrap();
        let water_model = read_ref(r)?;
        Ok(CPlugDynaObjectModel { version, is_static, dynamize_on_spawn, mesh, dyna_shape, static_shape, tail_a, loc_anim, tail_b, water_model })
    }

    pub fn write(&self, w: &mut Wr) {
        w.u32(self.version);
        w.u32(self.is_static);
        w.u32(self.dynamize_on_spawn);
        write_ref(w, &self.mesh);
        write_ref(w, &self.dyna_shape);
        write_ref(w, &self.static_shape);
        w.bytes(&self.tail_a);
        write_ref(w, &self.loc_anim);
        w.bytes(&self.tail_b);
        write_ref(w, &self.water_model);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimSubFunc {
    /// EAnimEase: 0 None, 1 Linear, 2 QuadIn, 3 QuadOut, 4 QuadInOut, 5 CubicIn …
    pub ease: u8,
    pub reverse: u8,
    pub duration_ms: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimFunc {
    /// 1 in every pack file.
    pub u01: u32,
    pub subs: Vec<AnimSubFunc>,
}

impl AnimFunc {
    fn parse(r: &mut Rd) -> R<AnimFunc> {
        let u01 = r.u32()?;
        let n = r.count()?;
        if n > 64 {
            return Err(format!("AnimFunc with {n} sub functions"));
        }
        let mut subs = Vec::with_capacity(n);
        for _ in 0..n {
            let ease = r.u8()?;
            let reverse = r.u8()?;
            let duration_ms = r.u32()?;
            subs.push(AnimSubFunc { ease, reverse, duration_ms });
        }
        Ok(AnimFunc { u01, subs })
    }
    fn write(&self, w: &mut Wr) {
        w.u32(self.u01);
        w.u32(self.subs.len() as u32);
        for s in &self.subs {
            w.u8(s.ease);
            w.u8(s.reverse);
            w.u32(s.duration_ms);
        }
    }
    /// The whole cycle, in ms.
    pub fn period_ms(&self) -> u32 {
        self.subs.iter().map(|s| s.duration_ms).sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KinematicConstraint {
    pub version: u32,
    pub sub_version: u32,
    pub trans: AnimFunc,
    pub rot: AnimFunc,
    /// 0 none, 1 TransSub (sub-texture keyframes).
    pub shader_tc_type: u32,
    pub shader_tc_version: u32,
    /// (duration ms, sub texture index) keyframes.
    pub shader_tc_anim: Vec<(u32, u32)>,
    /// Type 1: NbSubTexture, NbSubTexturePerLine, NbSubTexturePerColumn, TopToBottom.
    pub shader_tc_trans_sub: Option<[u32; 4]>,
    /// 0 X, 1 Y, 2 Z
    pub trans_axis: u8,
    pub trans_min: f32,
    pub trans_max: f32,
    pub rot_axis: u8,
    pub angle_min_deg: f32,
    pub angle_max_deg: f32,
}

impl KinematicConstraint {
    pub fn parse(r: &mut Rd) -> R<KinematicConstraint> {
        let version = r.u32()?;
        let sub_version = r.u32()?;
        if version != 0 || sub_version != 3 {
            return Err(format!("NPlugDyna_SKinematicConstraint version {version}.{sub_version} (only 0.3 is read)"));
        }
        let trans = AnimFunc::parse(r)?;
        let rot = AnimFunc::parse(r)?;
        let shader_tc_type = r.u32()?;
        let shader_tc_version = r.u32()?;
        let n = r.count()?;
        if n > 256 {
            return Err(format!("{n} shader keyframes"));
        }
        let mut shader_tc_anim = Vec::with_capacity(n);
        for _ in 0..n {
            let d = r.u32()?;
            let t = r.u32()?;
            shader_tc_anim.push((d, t));
        }
        let shader_tc_trans_sub = match shader_tc_type {
            0 => None,
            1 => Some([r.u32()?, r.u32()?, r.u32()?, r.u32()?]),
            t => return Err(format!("ShaderTcType {t} has no reader")),
        };
        let trans_axis = r.u8()?;
        let trans_min = r.f32()?;
        let trans_max = r.f32()?;
        let rot_axis = r.u8()?;
        let angle_min_deg = r.f32()?;
        let angle_max_deg = r.f32()?;
        Ok(KinematicConstraint { version, sub_version, trans, rot, shader_tc_type, shader_tc_version, shader_tc_anim, shader_tc_trans_sub, trans_axis, trans_min, trans_max, rot_axis, angle_min_deg, angle_max_deg })
    }

    pub fn write(&self, w: &mut Wr) {
        w.u32(self.version);
        w.u32(self.sub_version);
        self.trans.write(w);
        self.rot.write(w);
        w.u32(self.shader_tc_type);
        w.u32(self.shader_tc_version);
        w.u32(self.shader_tc_anim.len() as u32);
        for (d, t) in &self.shader_tc_anim {
            w.u32(*d);
            w.u32(*t);
        }
        if let Some(ts) = &self.shader_tc_trans_sub {
            for x in ts {
                w.u32(*x);
            }
        }
        w.u8(self.trans_axis);
        w.f32(self.trans_min);
        w.f32(self.trans_max);
        w.u8(self.rot_axis);
        w.f32(self.angle_min_deg);
        w.f32(self.angle_max_deg);
    }

    /// Parse a whole `.KinematicConstraint.Gbx` body (no externals).
    pub fn parse_body(body: &[u8]) -> R<KinematicConstraint> {
        let mut r = Rd::new(body, 0, super::LookbackState::default());
        let k = Self::parse(&mut r)?;
        if r.o != body.len() {
            return Err(format!("{} trailing bytes after the constraint", body.len() - r.o));
        }
        Ok(k)
    }

    /// The translation range follows the geometry; angles and timing stay.
    pub fn scale(&mut self, s: f32) {
        self.trans_min *= s;
        self.trans_max *= s;
    }

    /// One line for reports: axes, ranges, periods.
    pub fn summary(&self) -> String {
        let axis = |a: u8| match a {
            0 => "X",
            1 => "Y",
            2 => "Z",
            _ => "?",
        };
        format!(
            "trans {} {}..{} m over {} ms ({} steps), rot {} {}..{} deg over {} ms ({} steps), shader type {} ({} keyframes)",
            axis(self.trans_axis),
            self.trans_min,
            self.trans_max,
            self.trans.period_ms(),
            self.trans.subs.len(),
            axis(self.rot_axis),
            self.angle_min_deg,
            self.angle_max_deg,
            self.rot.period_ms(),
            self.rot.subs.len(),
            self.shader_tc_type,
            self.shader_tc_anim.len()
        )
    }
}

/// `NPlugDyna_SPrefabConstraintParams` (entity params 0x2F0C8000): version,
/// the two entity indices the constraint binds (-1 = the world), two
/// positions. The pack's obstacle prefabs bind Ent1 = -1 to Ent2 = the dyna
/// entity's index.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintParams {
    pub version: u32,
    pub ent1: i32,
    pub ent2: i32,
    pub pos1: [f32; 3],
    pub pos2: [f32; 3],
}

impl ConstraintParams {
    pub fn parse(b: &[u8]) -> Option<ConstraintParams> {
        if b.len() != 36 {
            return None;
        }
        let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        Some(ConstraintParams { version: u(0), ent1: u(4) as i32, ent2: u(8) as i32, pos1: [f(12), f(16), f(20)], pos2: [f(24), f(28), f(32)] })
    }
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(36);
        b.extend_from_slice(&self.version.to_le_bytes());
        b.extend_from_slice(&self.ent1.to_le_bytes());
        b.extend_from_slice(&self.ent2.to_le_bytes());
        for p in [self.pos1, self.pos2] {
            for x in p {
                b.extend_from_slice(&x.to_le_bytes());
            }
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Modifier\ItemObstacle\AnimRotorLevel1.KinematicConstraint.Gbx`, body
    /// (after the 25-byte GBX header): rot Z 180..-180 over 4 s.
    const ROTOR_L1: &[u8] = &[
        0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x01, 0x40, 0x1f, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x01, 0xa0, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x34, 0x43, 0x00, 0x00, 0x34, 0xc3,
    ];
    /// `Modifier\ItemObstacle\AnimPusher8mLevel1.KinematicConstraint.Gbx`:
    /// trans Z -0..8 m, 2 s out / 2 s back, one shader keyframe.
    const PUSHER8M_L1: &[u8] = &[
        0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0xd0, 0x07, 0x00, 0x00, 0x01, 0x01, 0xd0, 0x07, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04, 0x00, 0xe8, 0x03, 0x00, 0x00, 0x04, 0x01, 0xe8, 0x03, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xa0, 0x0f, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x41, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    fn roundtrip(body: &[u8]) -> KinematicConstraint {
        let k = KinematicConstraint::parse_body(body).expect("parse");
        let mut out = Vec::new();
        let mut lb = super::super::LookbackState::default();
        let mut w = Wr { w: &mut out, lb: &mut lb };
        k.write(&mut w);
        assert_eq!(out, body, "constraint does not round-trip");
        k
    }

    #[test]
    fn rotor_constraint_roundtrips() {
        let k = roundtrip(ROTOR_L1);
        assert_eq!((k.rot_axis, k.angle_min_deg, k.angle_max_deg), (2, 180.0, -180.0));
        assert_eq!(k.rot.period_ms(), 4000);
        assert_eq!(k.trans.period_ms(), 8000);
        assert_eq!(k.shader_tc_type, 0);
    }

    #[test]
    fn pusher_constraint_roundtrips_and_scales() {
        let mut k = roundtrip(PUSHER8M_L1);
        assert_eq!((k.trans_axis, k.trans_min, k.trans_max), (2, -0.0, 8.0));
        assert_eq!(k.trans.subs.len(), 2);
        assert_eq!(k.shader_tc_anim, vec![(4000, 4)]);
        assert_eq!(k.shader_tc_trans_sub, Some([5, 1, 8, 0]));
        k.scale(0.5);
        assert_eq!(k.trans_max, 4.0);
        assert_eq!(k.rot.period_ms(), 2000);
    }

    #[test]
    fn constraint_params_roundtrip() {
        let p = ConstraintParams { version: 0, ent1: -1, ent2: 0, pos1: [0.0; 3], pos2: [0.0; 3] };
        assert_eq!(ConstraintParams::parse(&p.bytes()), Some(p));
    }
}
