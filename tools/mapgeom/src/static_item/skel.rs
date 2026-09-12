//! `CPlugSkel` (0x090BA000) — the skeleton a car skin mesh binds to.
//!
//! Layout from gbx-py (`body_chunks[0x090BA000]`, the library the community's
//! `skinfix.py` is built on), read off the stock `MainBody.Skel.Gbx` (v20):
//!
//! ```text
//! u32 version (>= 12)
//! Id  name
//! u16 nJoints × { Id name, i16 parentIndex, [v<15: quat + vec3], v>=1: Iso4 globalLoc (12 f32) }
//! v>=2:  bool hasU03, [ { array{i16,i16,i16}, array{i32×4}, array i32, i16, i16 } ]
//! v>=6:  array sockets { Id name, i16 linkedJoint, Iso4 loc }
//! v>=9:  bool hasU04, [ { array Id, array i32, array i32, array quat } ]
//! v>=10: v<=15: array u32 | v>15: array u8 jointsLods
//! v>13:  array u8 rotationOrder (enum byte)
//! v==14: i32, i32
//! v>=19: array u8 u10
//! v>=17: u8 cLod, array f32 lodMaxDists
//! ```
//!
//! Why this exists (2026-09-12): a ZIP 3D skin must carry its skeleton INLINE
//! in the Solid2's `skel` slot — the stock pak meshes leave the slot null and
//! ship `MainBody.Skel.Gbx` beside them, resolved by the pak's model kit, and a
//! zip has no model kit: every zip mesh with a null skel crashed the client on
//! import (a null object's virtual call). `skinfix.py` keeps the importer's
//! inline skel; this module lets us put the stock skeleton (or a one-joint
//! "Body" one) inline. Kept generic and byte-faithful: the stock file round-trips.

use super::{Id, Rd, Wr, R, FACADE};

#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub name: Id,
    pub parent: i16,
    /// v<15 only
    pub old: Option<([f32; 4], [f32; 3])>,
    pub global_loc: [f32; 12],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Socket {
    pub name: Id,
    pub linked_joint: i16,
    pub loc: [f32; 12],
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct U03 {
    pub a: Vec<[i16; 3]>,
    pub b: Vec<[i32; 4]>,
    pub c: Vec<i32>,
    pub d: i16,
    pub e: i16,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct U04 {
    pub names: Vec<Id>,
    pub a: Vec<i32>,
    pub b: Vec<i32>,
    pub quats: Vec<[f32; 4]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugSkel {
    pub version: u32,
    pub name: Id,
    pub joints: Vec<Joint>,
    pub u03: Option<U03>,
    pub sockets: Vec<Socket>,
    pub u04: Option<U04>,
    /// v10..=15
    pub u05: Vec<u32>,
    /// v>15
    pub joints_lods: Vec<u8>,
    /// v>13
    pub rotation_order: Vec<u8>,
    /// v==14
    pub v14: Option<(i32, i32)>,
    /// v>=19
    pub u10: Vec<u8>,
    /// v>=20: one word before cLod (0 on the stock skel; not in gbx-py, read off the bytes)
    pub v20_word: u32,
    /// v>=17
    pub c_lod: u8,
    pub lod_max_dists: Vec<f32>,
    /// Skippable chunks after 000, raw.
    pub raw: Vec<super::RawChunk>,
}

pub const CLASS: u32 = 0x090BA000;

impl CPlugSkel {
    /// A one-joint skeleton: `Body` at the origin, the minimum a rigid skin
    /// can bind to. Version 20 like the stock file, no sockets.
    pub fn body_only() -> CPlugSkel {
        CPlugSkel {
            version: 20,
            name: Id::Null,
            joints: vec![Joint { name: Id::Str("Body".into()), parent: -1, old: None, global_loc: IDENTITY }],
            u03: None,
            sockets: Vec::new(),
            u04: None,
            u05: Vec::new(),
            joints_lods: vec![0],
            rotation_order: vec![0],
            v14: None,
            u10: vec![0],
            v20_word: 0,
            c_lod: 1,
            lod_max_dists: Vec::new(),
            raw: Vec::new(),
        }
    }

    /// Parse a node body: chunk id, payload, [skippable chunks], FACADE.
    pub fn parse(r: &mut Rd) -> R<CPlugSkel> {
        let cid = r.u32()?;
        if cid != CLASS {
            return Err(format!("CPlugSkel: first chunk 0x{cid:08X} is not 0x090BA000"));
        }
        let mut s = Self::parse_main(r)?;
        loop {
            let at = r.o;
            let c = r.u32()?;
            if c == FACADE {
                break;
            }
            if super::is_skippable_here(r) {
                let payload = super::read_skippable_payload(r, c)?;
                s.raw.push(super::RawChunk { id: c, payload });
            } else {
                return Err(format!("CPlugSkel: chunk 0x{c:08X} at 0x{at:x} has no reader"));
            }
        }
        Ok(s)
    }

    fn parse_main(r: &mut Rd) -> R<CPlugSkel> {
        let v = r.u32()?;
        if v < 12 {
            return Err(format!("CPlugSkel v{v}: only v12+ is modelled"));
        }
        let name = r.id()?;
        let n = r.u16()? as usize;
        let mut joints = Vec::with_capacity(n);
        for _ in 0..n {
            let jn = r.id()?;
            let parent = r.i16()?;
            let old = if v < 15 { Some((r.floats::<4>()?, r.floats::<3>()?)) } else { None };
            let global_loc = r.floats::<12>()?;
            joints.push(Joint { name: jn, parent, old, global_loc });
        }
        let mut s = CPlugSkel {
            version: v,
            name,
            joints,
            u03: None,
            sockets: Vec::new(),
            u04: None,
            u05: Vec::new(),
            joints_lods: Vec::new(),
            rotation_order: Vec::new(),
            v14: None,
            u10: Vec::new(),
            v20_word: 0,
            c_lod: 0,
            lod_max_dists: Vec::new(),
            raw: Vec::new(),
        };
        if r.bool32()? {
            let a = r.array(|r| Ok([r.i16()?, r.i16()?, r.i16()?]))?;
            let b = r.array(|r| Ok([r.i32()?, r.i32()?, r.i32()?, r.i32()?]))?;
            let c = r.array(|r| r.i32())?;
            let d = r.i16()?;
            let e = r.i16()?;
            s.u03 = Some(U03 { a, b, c, d, e });
        }
        s.sockets = r.array(|r| Ok(Socket { name: r.id()?, linked_joint: r.i16()?, loc: r.floats::<12>()? }))?;
        if r.bool32()? {
            let names = r.array(|r| r.id())?;
            let a = r.array(|r| r.i32())?;
            let b = r.array(|r| r.i32())?;
            let quats = r.array(|r| r.floats::<4>())?;
            s.u04 = Some(U04 { names, a, b, quats });
        }
        if v <= 15 {
            s.u05 = r.array(|r| r.u32())?;
        } else {
            s.joints_lods = r.array(|r| r.u8())?;
        }
        s.rotation_order = r.array(|r| r.u8())?;
        if v == 14 {
            s.v14 = Some((r.i32()?, r.i32()?));
        }
        if v >= 19 {
            s.u10 = r.array(|r| r.u8())?;
        }
        if v >= 20 {
            s.v20_word = r.u32()?;
        }
        if v >= 17 {
            s.c_lod = r.u8()?;
            s.lod_max_dists = r.array(|r| r.f32())?;
        }
        Ok(s)
    }

    /// Write the node body (chunk id + payload + raw chunks + FACADE).
    pub fn write(&self, w: &mut Wr) {
        w.u32(CLASS);
        let v = self.version;
        w.u32(v);
        w.id(&self.name);
        w.u16(self.joints.len() as u16);
        for j in &self.joints {
            w.id(&j.name);
            w.i16(j.parent);
            if v < 15 {
                let (q, p) = j.old.unwrap_or(([0.0, 0.0, 0.0, 1.0], [0.0; 3]));
                w.floats(&q);
                w.floats(&p);
            }
            w.floats(&j.global_loc);
        }
        w.bool32(self.u03.is_some());
        if let Some(u) = &self.u03 {
            w.u32(u.a.len() as u32);
            for x in &u.a {
                x.iter().for_each(|y| w.i16(*y));
            }
            w.u32(u.b.len() as u32);
            for x in &u.b {
                x.iter().for_each(|y| w.i32(*y));
            }
            w.u32(u.c.len() as u32);
            u.c.iter().for_each(|y| w.i32(*y));
            w.i16(u.d);
            w.i16(u.e);
        }
        w.u32(self.sockets.len() as u32);
        for sk in &self.sockets {
            w.id(&sk.name);
            w.i16(sk.linked_joint);
            w.floats(&sk.loc);
        }
        w.bool32(self.u04.is_some());
        if let Some(u) = &self.u04 {
            w.u32(u.names.len() as u32);
            u.names.iter().for_each(|x| w.id(x));
            w.u32(u.a.len() as u32);
            u.a.iter().for_each(|x| w.i32(*x));
            w.u32(u.b.len() as u32);
            u.b.iter().for_each(|x| w.i32(*x));
            w.u32(u.quats.len() as u32);
            u.quats.iter().for_each(|x| w.floats(x));
        }
        if v <= 15 {
            w.u32(self.u05.len() as u32);
            self.u05.iter().for_each(|x| w.u32(*x));
        } else {
            w.u32(self.joints_lods.len() as u32);
            self.joints_lods.iter().for_each(|x| w.u8(*x));
        }
        w.u32(self.rotation_order.len() as u32);
        self.rotation_order.iter().for_each(|x| w.u8(*x));
        if v == 14 {
            let (a, b) = self.v14.unwrap_or((0, 0));
            w.i32(a);
            w.i32(b);
        }
        if v >= 19 {
            w.u32(self.u10.len() as u32);
            self.u10.iter().for_each(|x| w.u8(*x));
        }
        if v >= 20 {
            w.u32(self.v20_word);
        }
        if v >= 17 {
            w.u8(self.c_lod);
            w.u32(self.lod_max_dists.len() as u32);
            w.floats(&self.lod_max_dists);
        }
        for rc in &self.raw {
            super::write_skippable(w, rc.id, &rc.payload);
        }
        w.u32(FACADE);
    }

    pub fn joint_names(&self) -> Vec<String> {
        self.joints.iter().map(|j| j.name.as_str().unwrap_or("?").to_string()).collect()
    }
}

pub const IDENTITY: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
