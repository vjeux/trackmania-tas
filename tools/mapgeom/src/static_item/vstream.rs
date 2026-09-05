//! `CPlugVertexStream` (0x09056000): the declarations and every element array.
//!
//! Chunk 0x09056000: version, count, flags, base-stream noderef, then (when
//! the stream owns its data) the declarations, a `compress Local3D` bool and
//! one tightly packed array per declaration, in declaration order.
//!
//! A declaration is two words: `flags1` = name (bits 0..9) | type (9..18) |
//! space (28..32); `flags2` carries an in-vertex byte offset in bits 2..12 —
//! when that field is non-zero a `u16` and the `u16` offset follow the pair
//! (GBX.NET `DataDecl`). A Float3 declared in Local3D space is stored packed
//! (Dec3N, one word) when the compress bool is set — the rule `classes.rs`
//! measured on the pack.

use super::{read_ref, write_ref, Rd, Ref, Wr, R};

pub const T_FLOAT1: u32 = 0;
pub const T_FLOAT2: u32 = 1;
pub const T_FLOAT3: u32 = 2;
pub const T_FLOAT4: u32 = 3;
pub const T_COLOR: u32 = 4;
pub const T_INT32: u32 = 5;
pub const T_DEC3N: u32 = 14;

pub const N_POSITION: u32 = 0;
pub const N_NORMAL: u32 = 5;
pub const N_COLOR0: u32 = 8;
pub const N_TEXCOORD0: u32 = 10;
pub const N_TANGENT_U: u32 = 18;
pub const N_TANGENT_V: u32 = 20;

pub const SPACE_GLOBAL3D: u32 = 0;
pub const SPACE_LOCAL3D: u32 = 1;
pub const SPACE_GLOBAL2D: u32 = 2;

/// Byte size of a declared element type (`GbxPlugVDclTypeBytes`).
pub fn type_size(t: u32) -> Option<usize> {
    const B: [usize; 17] = [4, 8, 0xC, 0x10, 4, 4, 4, 8, 4, 4, 8, 4, 8, 4, 4, 4, 8];
    B.get(t as usize).copied()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decl {
    pub flags1: u32,
    pub flags2: u32,
    /// Present when `flags2 & 0xFFC != 0`: (u02, offset).
    pub extra: Option<(u16, u16)>,
    /// Version-0 streams carry the data right after the declaration.
    pub v0_data: Vec<u8>,
}

impl Decl {
    pub fn name(&self) -> u32 {
        self.flags1 & 0x1FF
    }
    pub fn ty(&self) -> u32 {
        (self.flags1 >> 9) & 0x1FF
    }
    pub fn space(&self) -> u32 {
        (self.flags1 >> 28) & 0xF
    }
    pub fn offset(&self) -> u32 {
        (self.flags2 >> 2) & 0x3FF
    }
    /// The type the bytes are actually stored as.
    pub fn stored_type(&self, compress_local3d: bool) -> u32 {
        if self.ty() == T_FLOAT3 && self.space() == SPACE_LOCAL3D && compress_local3d {
            T_DEC3N
        } else {
            self.ty()
        }
    }
    /// A declaration as the game writes it: `offset` is the byte offset of the
    /// element inside a vertex.
    pub fn new(name: u32, ty: u32, space: u32, offset: u32) -> Decl {
        let flags1 = name | (ty << 9) | (space << 28) | 0x00A0_0000;
        let flags2 = offset << 2;
        let extra = if flags2 & 0xFFC != 0 { Some((0, offset as u16)) } else { None };
        Decl { flags1, flags2, extra, v0_data: Vec::new() }
    }
}

/// One declared element's data for every vertex.
#[derive(Clone, Debug, PartialEq)]
pub enum Elem {
    Float2(Vec<[f32; 2]>),
    Float3(Vec<[f32; 3]>),
    Float4(Vec<[f32; 4]>),
    /// Colour (RGBA bytes) or Int32 or Dec3N or any other one-word type.
    Word(Vec<u32>),
    /// Any other fixed-size type, raw.
    Raw { size: usize, bytes: Vec<u8> },
}

impl Elem {
    pub fn read(r: &mut Rd, stored_type: u32, n: usize) -> R<Elem> {
        let size = type_size(stored_type).ok_or_else(|| format!("vertex element type {stored_type} has no size"))?;
        Ok(match (stored_type, size) {
            (T_FLOAT2, _) => Elem::Float2((0..n).map(|_| r.vec2()).collect::<R<_>>()?),
            (T_FLOAT3, _) => Elem::Float3((0..n).map(|_| r.vec3()).collect::<R<_>>()?),
            (T_FLOAT4, _) => Elem::Float4((0..n).map(|_| r.floats::<4>()).collect::<R<_>>()?),
            (_, 4) => Elem::Word((0..n).map(|_| r.u32()).collect::<R<_>>()?),
            (_, size) => Elem::Raw { size, bytes: r.take(size * n)?.to_vec() },
        })
    }
    pub fn write(&self, w: &mut Wr) {
        match self {
            Elem::Float2(v) => v.iter().for_each(|x| w.floats(x)),
            Elem::Float3(v) => v.iter().for_each(|x| w.floats(x)),
            Elem::Float4(v) => v.iter().for_each(|x| w.floats(x)),
            Elem::Word(v) => v.iter().for_each(|x| w.u32(*x)),
            Elem::Raw { bytes, .. } => w.bytes(bytes),
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Elem::Float2(v) => v.len(),
            Elem::Float3(v) => v.len(),
            Elem::Float4(v) => v.len(),
            Elem::Word(v) => v.len(),
            Elem::Raw { size, bytes } => bytes.len() / size.max(&1),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugVertexStream {
    pub version: u32,
    pub count: i32,
    pub flags: u32,
    pub base: Ref,
    pub decls: Vec<Decl>,
    /// Absent for version 0 streams.
    pub compress_local3d: Option<bool>,
    /// One per declaration, in declaration order.
    pub elems: Vec<Elem>,
}

impl CPlugVertexStream {
    /// Parse the node body after its class id (chunk 0x09056000 + FACADE).
    pub fn parse(r: &mut Rd) -> R<CPlugVertexStream> {
        let cid = r.u32()?;
        if cid != 0x09056000 {
            return Err(format!("CPlugVertexStream starts with chunk 0x{cid:08X}"));
        }
        let s = Self::parse_chunk(r)?;
        let f = r.u32()?;
        if f != super::FACADE {
            return Err(format!("CPlugVertexStream: 0x{f:08X} after chunk 000 is not FACADE (at 0x{:x})", r.o - 4));
        }
        Ok(s)
    }

    pub fn write(&self, w: &mut Wr) {
        w.u32(0x09056000);
        self.write_chunk(w);
        w.u32(super::FACADE);
    }

    fn parse_chunk(r: &mut Rd) -> R<CPlugVertexStream> {
        let version = r.u32()?;
        let count = r.i32()?;
        let flags = r.u32()?;
        let base = read_ref(r)?;
        let mut s = CPlugVertexStream { version, count, flags, base, decls: Vec::new(), compress_local3d: None, elems: Vec::new() };
        if count == 0 || s.base.index != -1 {
            return Ok(s);
        }
        let n = count as usize;
        let n_decl = r.count()?;
        for _ in 0..n_decl {
            let flags1 = r.u32()?;
            let flags2 = r.u32()?;
            let mut d = Decl { flags1, flags2, extra: None, v0_data: Vec::new() };
            if flags2 & 0xFFC == 0 {
                if version == 0 {
                    let per = ((flags1 >> 0x12) & 0x3FF) as usize;
                    d.v0_data = r.take(per * n)?.to_vec();
                }
            } else {
                let u02 = r.u16()?;
                let off = r.u16()?;
                if off as u32 != d.offset() {
                    return Err(format!("vertex decl offset mismatch ({off} vs {})", d.offset()));
                }
                d.extra = Some((u02, off));
            }
            s.decls.push(d);
        }
        if version == 0 {
            return Ok(s);
        }
        let compress = r.bool32()?;
        s.compress_local3d = Some(compress);
        for d in &s.decls {
            s.elems.push(Elem::read(r, d.stored_type(compress), n)?);
        }
        Ok(s)
    }

    fn write_chunk(&self, w: &mut Wr) {
        w.u32(self.version);
        w.i32(self.count);
        w.u32(self.flags);
        write_ref(w, &self.base);
        if self.count == 0 || self.base.index != -1 {
            return;
        }
        w.u32(self.decls.len() as u32);
        for d in &self.decls {
            w.u32(d.flags1);
            w.u32(d.flags2);
            if d.flags2 & 0xFFC == 0 {
                if self.version == 0 {
                    w.bytes(&d.v0_data);
                }
            } else if let Some((u02, off)) = d.extra {
                w.u16(u02);
                w.u16(off);
            }
        }
        if self.version == 0 {
            return;
        }
        w.bool32(self.compress_local3d.unwrap_or(false));
        for e in &self.elems {
            e.write(w);
        }
    }
}
