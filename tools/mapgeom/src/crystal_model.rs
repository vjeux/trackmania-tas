//! A complete model of the Trackmania 2020 `CPlugCrystal` class (0x09003000):
//! every chunk, every layer type, every field, parsed into typed structs and
//! serialised back byte for byte.
//!
//! The definitions follow GBX.NET's `CPlugCrystal` (reader/writer + chunkl)
//! and `CPlugMaterialUserInst`. The model is exact by construction: the
//! round-trip example (`examples/crystal_roundtrip_all.rs`) re-emits every
//! Nadeo item's crystal and demands identical bytes.
//!
//! Two encodings are stateful and shared with the rest of the item body:
//!
//! * **Lookback strings** (`id`): `0x40000000` followed by a string defines
//!   the next table entry, `0x40000000 | n` refers to entry *n* (1-based),
//!   `0xFFFFFFFF` is null, and the first id in a body is preceded by the
//!   table version word `3`. The table is body-wide: the item's ident strings
//!   come before the crystal, and the crystal's layer ids (`Layer0`, ...) and
//!   material ids join the same table. `LookbackState` carries it; `locate`
//!   walks the item body up to the crystal to seed it.
//! * **Node references**: an index into the file's node table; the first
//!   time an index appears the node's class id and chunks follow inline.
//!   `NodeRef::inline` holds the inline body; `LookbackState::defined_nodes`
//!   knows which indices are already defined.
//!
//! **Optimized ints.** An index is 1, 2 or 4 bytes wide, the width chosen from
//! a count with `count < 0xFF -> u8, count < 0xFFFF -> u16, else u32`
//! (`opt_width`; GBX.NET's thresholds). Which count depends on the encoding,
//! measured on all 2222 Nadeo items + Sheep:
//!
//! * a lone index (a face's vertex indices, material index, group index) is
//!   sized from the number of things it indexes -- positions, materials,
//!   groups. `DecoWallTiltTransition1DownRight` has exactly 255 positions and
//!   writes its vertex indices as u16, which fixes the threshold at `< 0xFF`;
//! * a length-prefixed index array (the tex-coord indices, the lightmap
//!   indices, the v35+ edge pairs) is sized from its own LENGTH, as GBX.NET's
//!   `ReadArrayOptimizedInt()`: `OpenDirtHillsShortCurve1In`'s collision layer
//!   has 4536 indices into 3 tex coords, written as u16s. 1921 items have a
//!   tex-coord index array whose length falls in a different width band than
//!   its coord count, so the two rules are not interchangeable.

use std::collections::HashSet;

pub type R<T> = Result<T, String>;

pub const C_CRYSTAL: u32 = 0x09003000;
pub const C_MATERIAL_USER_INST: u32 = 0x090FD000;
pub const C_TREE_GENERATOR_CHUNK: u32 = 0x09051000;
pub const FACADE: u32 = 0xFACADE01;
const SKIP: &[u8; 4] = b"PIKS";

// ------------------------------------------------------------------ lookback

/// The body-wide lookback string table plus the set of node indices already
/// defined, at some point of a body walk.
#[derive(Clone, Debug, Default)]
pub struct LookbackState {
    /// Entry `i` is lookback index `i + 1`.
    pub table: Vec<String>,
    /// Whether the one-per-body table version word has been consumed.
    pub version_seen: bool,
    /// Node indices whose bodies have already been written/read.
    pub defined_nodes: HashSet<u32>,
}

/// A lookback string as the model keeps it.
#[derive(Clone, Debug, PartialEq)]
pub enum Id {
    /// `0xFFFFFFFF`.
    Null,
    /// A string: written as a new table entry the first time, a back
    /// reference afterwards.
    Str(String),
    /// A back reference to a table entry the walk never saw (a `parse`
    /// without a seeded table); kept as the raw word.
    Prior(u32),
    /// Any other encoding (a collection id, `Unassigned`), raw.
    Raw(u32),
}

impl Id {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Id::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Width of an optimized int chosen from `count` (see the module doc for
/// which count each encoding uses).
pub fn opt_width(count: usize) -> usize {
    if count < 0xFF {
        1
    } else if count < 0xFFFF {
        2
    } else {
        4
    }
}

// -------------------------------------------------------------------- reader

pub struct Rd<'a> {
    pub b: &'a [u8],
    pub o: usize,
    pub lb: LookbackState,
}

impl<'a> Rd<'a> {
    pub fn new(b: &'a [u8], o: usize, lb: LookbackState) -> Rd<'a> {
        Rd { b, o, lb }
    }
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        let end = self.o.checked_add(n).ok_or("length overflow")?;
        if end > self.b.len() {
            return Err(format!("read {} bytes at 0x{:x} past the end (0x{:x})", n, self.o, self.b.len()));
        }
        let s = &self.b[self.o..end];
        self.o = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> R<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> R<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> R<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn f32(&mut self) -> R<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn bool32(&mut self) -> R<bool> {
        Ok(self.u32()? != 0)
    }
    pub fn vec2(&mut self) -> R<[f32; 2]> {
        Ok([self.f32()?, self.f32()?])
    }
    pub fn vec3(&mut self) -> R<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    pub fn floats<const N: usize>(&mut self) -> R<[f32; N]> {
        let mut m = [0f32; N];
        for v in m.iter_mut() {
            *v = self.f32()?;
        }
        Ok(m)
    }
    pub fn peek_u32(&self) -> Option<u32> {
        self.b.get(self.o..self.o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn string(&mut self) -> R<String> {
        let n = self.u32()? as usize;
        if n > 1 << 24 {
            return Err(format!("absurd string length {} at 0x{:x}", n, self.o - 4));
        }
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    pub fn count(&mut self) -> R<usize> {
        let n = self.u32()? as usize;
        if n > 50_000_000 {
            return Err(format!("absurd count {} at 0x{:x}", n, self.o - 4));
        }
        Ok(n)
    }
    pub fn array<T>(&mut self, mut f: impl FnMut(&mut Self) -> R<T>) -> R<Vec<T>> {
        let n = self.count()?;
        let mut v = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            v.push(f(self)?);
        }
        Ok(v)
    }
    /// One optimized int indexing `count` things.
    pub fn opt(&mut self, count: usize) -> R<u32> {
        Ok(match opt_width(count) {
            1 => self.u8()? as u32,
            2 => self.u16()? as u32,
            _ => self.u32()?,
        })
    }
    /// A length-prefixed optimized int array: the element width is chosen from
    /// the array LENGTH (measured: a 4536-entry tex-coord index array over 3
    /// tex coords is written as u16s), as GBX.NET's `ReadArrayOptimizedInt()`.
    pub fn opt_array(&mut self) -> R<Vec<u32>> {
        let n = self.count()?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(self.opt(n)?);
        }
        Ok(v)
    }
    pub fn id(&mut self) -> R<Id> {
        if !self.lb.version_seen {
            let v = self.u32()?;
            if v != 3 {
                return Err(format!("lookback version {} (expected 3) at 0x{:x}", v, self.o - 4));
            }
            self.lb.version_seen = true;
        }
        let raw = self.u32()?;
        if raw == 0xFFFF_FFFF {
            return Ok(Id::Null);
        }
        let flags = raw >> 30;
        let idx = (raw & 0x3FFF_FFFF) as usize;
        if flags == 0 {
            return Ok(Id::Raw(raw));
        }
        if idx == 0 {
            let s = self.string()?;
            self.lb.table.push(s.clone());
            return Ok(Id::Str(s));
        }
        if idx == 0x3FFF_FFFF {
            return Ok(Id::Raw(raw));
        }
        match self.lb.table.get(idx - 1) {
            Some(s) => Ok(Id::Str(s.clone())),
            None => Ok(Id::Prior(raw)),
        }
    }
    /// A node reference: the index, and the inline body when this is the
    /// node's first appearance (`read` parses the body after its class id).
    pub fn noderef<T>(&mut self, read: impl FnOnce(&mut Self, u32) -> R<T>) -> R<NodeRef<T>> {
        let index = self.i32()?;
        if index < 0 {
            return Ok(NodeRef { index, inline: None });
        }
        if self.lb.defined_nodes.contains(&(index as u32)) {
            return Ok(NodeRef { index, inline: None });
        }
        self.lb.defined_nodes.insert(index as u32);
        let class_id = self.u32()?;
        let body = read(self, class_id).map_err(|e| format!("node {} (class 0x{:08X}): {}", index, class_id, e))?;
        Ok(NodeRef { index, inline: Some(Box::new(body)) })
    }
}

// -------------------------------------------------------------------- writer

pub struct Wr<'a> {
    pub w: &'a mut Vec<u8>,
    pub lb: &'a mut LookbackState,
}

impl<'a> Wr<'a> {
    pub fn u8(&mut self, v: u8) {
        self.w.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.w.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.w.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.w.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.u32(v as u32);
    }
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub fn bool32(&mut self, v: bool) {
        self.u32(v as u32);
    }
    pub fn floats(&mut self, v: &[f32]) {
        for x in v {
            self.f32(*x);
        }
    }
    pub fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.w.extend_from_slice(s.as_bytes());
    }
    pub fn opt(&mut self, v: u32, count: usize) {
        match opt_width(count) {
            1 => self.u8(v as u8),
            2 => self.u16(v as u16),
            _ => self.u32(v),
        }
    }
    pub fn opt_array(&mut self, v: &[u32]) {
        self.u32(v.len() as u32);
        for x in v {
            self.opt(*x, v.len());
        }
    }
    pub fn id(&mut self, id: &Id) {
        if !self.lb.version_seen {
            self.u32(3);
            self.lb.version_seen = true;
        }
        match id {
            Id::Null => self.u32(0xFFFF_FFFF),
            Id::Raw(r) | Id::Prior(r) => self.u32(*r),
            Id::Str(s) => match self.lb.table.iter().position(|x| x == s) {
                Some(p) => self.u32(0x4000_0000 | (p as u32 + 1)),
                None => {
                    self.lb.table.push(s.clone());
                    self.u32(0x4000_0000);
                    self.string(s);
                }
            },
        }
    }
    pub fn noderef<T>(&mut self, n: &NodeRef<T>, class_id: u32, write: impl FnOnce(&mut Self, &T)) {
        self.i32(n.index);
        if let Some(body) = &n.inline {
            self.lb.defined_nodes.insert(n.index as u32);
            self.u32(class_id);
            write(self, body);
        }
    }
}

/// A node reference; `inline` is the body when it is defined at this spot.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeRef<T> {
    pub index: i32,
    pub inline: Option<Box<T>>,
}

/// An inline node of a class this model has no reader for: its bytes up to
/// and including its FACADE are kept verbatim (the chunk walk is generic:
/// a body is chunks until `0xFACADE01`, and only skippable chunks can be
/// stepped over without a reader, so this is only ever reached for nodes
/// made of skippable chunks). Never hit on the Nadeo corpus.
#[derive(Clone, Debug, PartialEq)]
pub struct OpaqueNode {
    pub class_id: u32,
    pub raw: Vec<u8>,
}

fn read_opaque(r: &mut Rd, class_id: u32) -> R<OpaqueNode> {
    let start = r.o;
    loop {
        let cid = r.u32()?;
        if cid == FACADE {
            break;
        }
        if r.b.get(r.o..r.o + 4) == Some(SKIP) {
            r.u32()?;
            let n = r.count()?;
            r.take(n)?;
        } else {
            return Err(format!("class 0x{:08X} has no reader and chunk 0x{:08X} is not skippable", class_id, cid));
        }
    }
    Ok(OpaqueNode { class_id, raw: r.b[start..r.o].to_vec() })
}

fn write_opaque(w: &mut Wr, n: &OpaqueNode) {
    w.w.extend_from_slice(&n.raw);
}

// ----------------------------------------------------- CPlugMaterialUserInst

#[derive(Clone, Debug, PartialEq)]
pub struct Cst {
    pub u01: Id,
    pub u02: Id,
    pub u03: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UvAnim {
    pub u01: Id,
    pub u02: Id,
    pub u03: f32,
    pub u04: u64,
    /// v5+
    pub u05: Id,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserTexture {
    pub u01: i32,
    pub texture: String,
}

/// Chunk 0x090FD000.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialMain {
    pub version: u32,
    /// v11+ (a byte).
    pub is_using_game_material: bool,
    pub material_name: Id,
    pub model: Id,
    pub base_texture: String,
    pub surface_physic_id: u8,
    /// v10+
    pub surface_gameplay_id: u8,
    /// v1+. A plain string when `is_using_game_material` (or v9..10), an id
    /// otherwise; `Id::Str` either way when it is a string.
    pub link: Id,
    /// v2+
    pub csts: Vec<Cst>,
    pub color: Vec<i32>,
    /// v3+
    pub uv_anims: Vec<UvAnim>,
    /// v4+
    pub u01: Vec<Id>,
    /// v6+
    pub user_textures: Vec<UserTexture>,
    /// v7+
    pub hiding_group: Id,
}

impl MaterialMain {
    fn link_is_string(&self) -> bool {
        self.is_using_game_material || (9..=10).contains(&self.version)
    }
}

/// Chunk 0x090FD001.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialTiling {
    pub version: u32,
    pub atlas: NodeRef<OpaqueNode>,
    /// v3+
    pub tiling_u: i32,
    pub tiling_v: i32,
    pub texture_size_in_meters: f32,
    /// v4+
    pub u01: i32,
    /// v5+
    pub is_natural: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugMaterialUserInst {
    /// Chunk ids in file order.
    pub chunks: Vec<u32>,
    pub main: Option<MaterialMain>,
    pub tiling: Option<MaterialTiling>,
    /// Chunk 0x090FD002: version, int.
    pub chunk2: Option<(u32, i32)>,
}

impl CPlugMaterialUserInst {
    /// A game material: `link` with `physics`, everything else at the values
    /// every Nadeo item carries (v11 main, v5 tiling, v0 chunk 2).
    pub fn game_material(link: &str, physics: u8) -> CPlugMaterialUserInst {
        CPlugMaterialUserInst {
            chunks: vec![0x090FD000, 0x090FD001, 0x090FD002],
            main: Some(MaterialMain {
                version: 11,
                is_using_game_material: true,
                material_name: Id::Null,
                model: Id::Null,
                base_texture: String::new(),
                surface_physic_id: physics,
                surface_gameplay_id: 0,
                link: Id::Str(link.to_string()),
                csts: Vec::new(),
                color: Vec::new(),
                uv_anims: Vec::new(),
                u01: Vec::new(),
                user_textures: Vec::new(),
                hiding_group: Id::Null,
            }),
            tiling: Some(MaterialTiling {
                version: 5,
                atlas: NodeRef { index: -1, inline: None },
                tiling_u: 0,
                tiling_v: 0,
                texture_size_in_meters: 1.0,
                u01: 0,
                is_natural: false,
            }),
            chunk2: Some((0, 0)),
        }
    }

    pub fn link(&self) -> Option<&str> {
        self.main.as_ref().and_then(|m| m.link.as_str())
    }
    pub fn physics(&self) -> u8 {
        self.main.as_ref().map(|m| m.surface_physic_id).unwrap_or(0)
    }

    pub fn parse(r: &mut Rd) -> R<CPlugMaterialUserInst> {
        let mut m = CPlugMaterialUserInst { chunks: Vec::new(), main: None, tiling: None, chunk2: None };
        loop {
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            m.chunks.push(cid);
            match cid {
                0x090FD000 => {
                    let version = r.u32()?;
                    let is_using_game_material = if version >= 11 { r.u8()? != 0 } else { false };
                    let material_name = r.id()?;
                    let model = r.id()?;
                    let base_texture = r.string()?;
                    let surface_physic_id = r.u8()?;
                    let surface_gameplay_id = if version >= 10 { r.u8()? } else { 0 };
                    let mut main = MaterialMain {
                        version,
                        is_using_game_material,
                        material_name,
                        model,
                        base_texture,
                        surface_physic_id,
                        surface_gameplay_id,
                        link: Id::Null,
                        csts: Vec::new(),
                        color: Vec::new(),
                        uv_anims: Vec::new(),
                        u01: Vec::new(),
                        user_textures: Vec::new(),
                        hiding_group: Id::Null,
                    };
                    if version >= 1 {
                        main.link = if main.link_is_string() { Id::Str(r.string()?) } else { r.id()? };
                    }
                    if version >= 2 {
                        main.csts = r.array(|r| Ok(Cst { u01: r.id()?, u02: r.id()?, u03: r.i32()? }))?;
                        main.color = r.array(|r| r.i32())?;
                    }
                    if version >= 3 {
                        main.uv_anims = r.array(|r| {
                            Ok(UvAnim {
                                u01: r.id()?,
                                u02: r.id()?,
                                u03: r.f32()?,
                                u04: r.u64()?,
                                u05: if version >= 5 { r.id()? } else { Id::Null },
                            })
                        })?;
                    }
                    if version >= 4 {
                        main.u01 = r.array(|r| r.id())?;
                    }
                    if version >= 6 {
                        main.user_textures = r.array(|r| Ok(UserTexture { u01: r.i32()?, texture: r.string()? }))?;
                    }
                    if version >= 7 {
                        main.hiding_group = r.id()?;
                    }
                    m.main = Some(main);
                }
                0x090FD001 => {
                    let version = r.u32()?;
                    if version == 2 {
                        return Err("CPlugMaterialUserInst chunk 001 version 2 (GBX.NET: throw)".into());
                    }
                    let atlas = r.noderef(read_opaque)?;
                    let mut t = MaterialTiling { version, atlas, tiling_u: 0, tiling_v: 0, texture_size_in_meters: 0.0, u01: 0, is_natural: false };
                    if version >= 3 {
                        t.tiling_u = r.i32()?;
                        t.tiling_v = r.i32()?;
                        t.texture_size_in_meters = r.f32()?;
                    }
                    if version >= 4 {
                        t.u01 = r.i32()?;
                    }
                    if version >= 5 {
                        t.is_natural = r.bool32()?;
                    }
                    m.tiling = Some(t);
                }
                0x090FD002 => {
                    m.chunk2 = Some((r.u32()?, r.i32()?));
                }
                c => return Err(format!("CPlugMaterialUserInst chunk 0x{:08X} at 0x{:x} has no reader", c, r.o - 4)),
            }
        }
        Ok(m)
    }

    pub fn write(&self, w: &mut Wr) {
        for cid in &self.chunks {
            w.u32(*cid);
            match *cid {
                0x090FD000 => {
                    let m = self.main.as_ref().expect("material chunk 000 listed but absent");
                    w.u32(m.version);
                    if m.version >= 11 {
                        w.u8(m.is_using_game_material as u8);
                    }
                    w.id(&m.material_name);
                    w.id(&m.model);
                    w.string(&m.base_texture);
                    w.u8(m.surface_physic_id);
                    if m.version >= 10 {
                        w.u8(m.surface_gameplay_id);
                    }
                    if m.version >= 1 {
                        if m.link_is_string() {
                            w.string(m.link.as_str().unwrap_or(""));
                        } else {
                            w.id(&m.link);
                        }
                    }
                    if m.version >= 2 {
                        w.u32(m.csts.len() as u32);
                        for c in &m.csts {
                            w.id(&c.u01);
                            w.id(&c.u02);
                            w.i32(c.u03);
                        }
                        w.u32(m.color.len() as u32);
                        for c in &m.color {
                            w.i32(*c);
                        }
                    }
                    if m.version >= 3 {
                        w.u32(m.uv_anims.len() as u32);
                        for a in &m.uv_anims {
                            w.id(&a.u01);
                            w.id(&a.u02);
                            w.f32(a.u03);
                            w.u64(a.u04);
                            if m.version >= 5 {
                                w.id(&a.u05);
                            }
                        }
                    }
                    if m.version >= 4 {
                        w.u32(m.u01.len() as u32);
                        for i in &m.u01 {
                            w.id(i);
                        }
                    }
                    if m.version >= 6 {
                        w.u32(m.user_textures.len() as u32);
                        for t in &m.user_textures {
                            w.i32(t.u01);
                            w.string(&t.texture);
                        }
                    }
                    if m.version >= 7 {
                        w.id(&m.hiding_group);
                    }
                }
                0x090FD001 => {
                    let t = self.tiling.as_ref().expect("material chunk 001 listed but absent");
                    w.u32(t.version);
                    w.noderef(&t.atlas, t.atlas.inline.as_ref().map(|n| n.class_id).unwrap_or(0), write_opaque);
                    if t.version >= 3 {
                        w.i32(t.tiling_u);
                        w.i32(t.tiling_v);
                        w.f32(t.texture_size_in_meters);
                    }
                    if t.version >= 4 {
                        w.i32(t.u01);
                    }
                    if t.version >= 5 {
                        w.bool32(t.is_natural);
                    }
                }
                0x090FD002 => {
                    let (v, x) = self.chunk2.expect("material chunk 002 listed but absent");
                    w.u32(v);
                    w.i32(x);
                }
                c => panic!("CPlugMaterialUserInst chunk 0x{c:08X} has no writer"),
            }
        }
        w.u32(FACADE);
    }
}

// -------------------------------------------------------------- the crystal

/// A crystal material slot: a literal name, or (when the name is empty) a
/// `CPlugMaterialUserInst` node.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub node: Option<NodeRef<CPlugMaterialUserInst>>,
}

impl Material {
    pub fn inst(&self) -> Option<&CPlugMaterialUserInst> {
        self.node.as_ref().and_then(|n| n.inline.as_deref())
    }
}

/// Chunk 0x09003004 (skippable), kept raw: version, data, `v1+` int.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk4 {
    pub version: u32,
    pub data: Vec<u8>,
    pub u01: Option<i32>,
    /// Bytes past what the reader consumes, if the game wrote any.
    pub trailing: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisualLevel {
    pub u01: i32,
    pub u02: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnchorInfo {
    pub u01: bool,
    pub u02: bool,
    pub u03: [f32; 12],
    pub u04: String,
    pub u05: i32,
}

/// A group ("part") of the crystal: folders and leaves of a tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// v31+
    pub u01: i32,
    /// A byte from v36, an int before.
    pub u02: i32,
    /// Parent group index (-1 for none).
    pub u03: i32,
    pub name: String,
    pub u04: i32,
    /// Child group indices.
    pub u05: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Face {
    /// Position indices, 3 or more.
    pub verts: Vec<u32>,
    /// v37+: one index into `Crystal::tex_coords` per corner.
    pub uv_index: Vec<u32>,
    /// v<37: one texture coordinate per corner.
    pub uvs: Vec<[f32; 2]>,
    /// v<27: a vec3 after the uvs (a normal?).
    pub u01: Option<[f32; 3]>,
    /// v25+; -1 when the crystal has no materials and none is set.
    pub material: i32,
    pub group: u32,
}

/// The `Crystal` mesh archive (`Version` 21..37 read by GBX.NET; 37 is what
/// TM2020 writes).
#[derive(Clone, Debug, PartialEq)]
pub struct Crystal {
    pub version: u32,
    /// v13+; always 4.
    pub u01: i32,
    pub visual_levels: Vec<VisualLevel>,
    /// v23+
    pub anchor_infos: Vec<AnchorInfo>,
    /// v22+
    pub groups: Vec<Part>,
    /// v25+. Two extra copies precede it below v29; a byte from v34.
    pub is_embedded: bool,
    pub is_embedded_extra: [bool; 2],
    /// v33+. U02 is the largest face material index, U03 the largest face
    /// group index (the game reads them as width selectors).
    pub u02: i32,
    pub u03: i32,
    pub positions: Vec<[f32; 3]>,
    /// v35+: the edge count word before the optimized edge array (the game
    /// writes the true edge count; GBX.NET writes 0).
    pub edge_count: u32,
    pub edges: Vec<[u32; 2]>,
    /// v37+
    pub tex_coords: Vec<[f32; 2]>,
    pub faces: Vec<Face>,
    /// Per face: the ints read after the faces (non-embedded / v<30 forms).
    pub face_extra: Vec<Vec<i32>>,
    /// v<29: one float per position.
    pub position_extra: Vec<f32>,
    pub u04: i32,
    /// v7..31: crystal link count (must be 0), then v10+: int, string, and
    /// v<30 a float array.
    pub u05: i32,
    pub u06: String,
    pub old_smoothing: Vec<f32>,
    /// v<36: three counted int blocks (kept raw) and U07.
    pub old_blocks: Option<(Vec<u8>, Vec<u8>, Vec<u8>)>,
    pub u07: i32,
}

impl Crystal {
    pub fn parse(r: &mut Rd, material_count: usize) -> R<Crystal> {
        let version = r.u32()?;
        if !(21..=37).contains(&version) {
            return Err(format!("crystal version {} at 0x{:x} is outside 21..37", version, r.o - 4));
        }
        let mut c = Crystal {
            version,
            u01: 0,
            visual_levels: Vec::new(),
            anchor_infos: Vec::new(),
            groups: Vec::new(),
            is_embedded: true,
            is_embedded_extra: [true, true],
            u02: 0,
            u03: 0,
            positions: Vec::new(),
            edge_count: 0,
            edges: Vec::new(),
            tex_coords: Vec::new(),
            faces: Vec::new(),
            face_extra: Vec::new(),
            position_extra: Vec::new(),
            u04: 0,
            u05: 0,
            u06: String::new(),
            old_smoothing: Vec::new(),
            old_blocks: None,
            u07: 0,
        };
        if version >= 13 {
            c.u01 = r.i32()?;
            c.visual_levels = r.array(|r| Ok(VisualLevel { u01: r.i32()?, u02: r.f32()? }))?;
        }
        if version >= 23 {
            c.anchor_infos = r.array(|r| Ok(AnchorInfo { u01: r.bool32()?, u02: r.bool32()?, u03: r.floats()?, u04: r.string()?, u05: r.i32()? }))?;
        }
        if version >= 22 {
            c.groups = r.array(|r| {
                Ok(Part {
                    u01: if version >= 31 { r.i32()? } else { 0 },
                    u02: if version >= 36 { r.u8()? as i32 } else { r.i32()? },
                    u03: r.i32()?,
                    name: r.string()?,
                    u04: r.i32()?,
                    u05: r.array(|r| r.i32())?,
                })
            })?;
        }
        if version >= 25 {
            if version < 29 {
                c.is_embedded_extra = [r.bool32()?, r.bool32()?];
            }
            c.is_embedded = if version >= 34 { r.u8()? != 0 } else { r.bool32()? };
            if version >= 33 {
                c.u02 = r.i32()?;
                c.u03 = r.i32()?;
            }
        }
        if !c.is_embedded {
            return Err("non-embedded crystal (Crystal.Gbx) at 0x{:x}: GBX.NET: not supported".into());
        }
        c.positions = r.array(|r| r.vec3())?;
        let npos = c.positions.len();
        let edge_count = r.count()?;
        if version >= 35 {
            c.edge_count = edge_count as u32;
            let n = r.count()?;
            for _ in 0..n {
                c.edges.push([r.opt(n)?, r.opt(n)?]);
            }
        } else {
            for _ in 0..edge_count {
                c.edges.push([r.u32()?, r.u32()?]);
            }
        }
        let face_count = r.count()?;
        let mut tex_indices: Vec<u32> = Vec::new();
        if version >= 37 {
            c.tex_coords = r.array(|r| r.vec2())?;
            tex_indices = r.opt_array()?;
        }
        let mut corner = 0usize;
        for _ in 0..face_count {
            let nv = if version >= 35 { r.u8()? as usize + 3 } else { r.count()? };
            let mut f = Face::default();
            for _ in 0..nv {
                f.verts.push(if version >= 34 { r.opt(npos)? } else { r.u32()? });
            }
            if version < 27 {
                let uv_count = r.count()?.min(nv);
                for _ in 0..uv_count {
                    f.uvs.push(r.vec2()?);
                }
                f.u01 = Some(r.vec3()?);
            } else if version < 37 {
                for _ in 0..nv {
                    f.uvs.push(r.vec2()?);
                }
            } else {
                for _ in 0..nv {
                    let i = *tex_indices.get(corner).ok_or_else(|| format!("tex coord index array too short at corner {corner}"))?;
                    f.uv_index.push(i);
                    corner += 1;
                }
            }
            f.material = -1;
            if version >= 25 {
                f.material = if version >= 33 && material_count > 0 { r.opt(material_count)? as i32 } else { r.i32()? };
            }
            f.group = if version >= 33 { r.opt(c.groups.len())? } else { r.u32()? };
            c.faces.push(f);
        }
        if corner != tex_indices.len() {
            return Err(format!("tex coord index array has {} entries, faces use {}", tex_indices.len(), corner));
        }
        for _ in 0..face_count {
            let mut extra = Vec::new();
            if version < 30 {
                extra.push(r.i32()?);
            }
            c.face_extra.push(extra);
        }
        if version < 29 {
            for _ in 0..npos {
                c.position_extra.push(r.f32()?);
            }
        }
        c.u04 = r.i32()?;
        if (7..32).contains(&version) {
            let n = r.count()?;
            if n > 0 {
                return Err(format!("CCrystalLink array length {} > 0: GBX.NET: not supported", n));
            }
            if version >= 10 {
                c.u05 = r.i32()?;
                c.u06 = r.string()?;
                if version < 30 {
                    c.old_smoothing = r.array(|r| r.f32())?;
                }
            }
        }
        if version < 36 {
            let nf = r.count()?;
            let ne = r.count()?;
            let nv = r.count()?;
            let a = r.take(nf * 4)?.to_vec();
            let b = r.take(ne * 4)?.to_vec();
            let d = r.take(nv * 4)?.to_vec();
            c.old_blocks = Some((a, b, d));
            c.u07 = r.i32()?;
        }
        Ok(c)
    }

    pub fn write(&self, w: &mut Wr, material_count: usize) {
        let version = self.version;
        w.u32(version);
        if version >= 13 {
            w.i32(self.u01);
            w.u32(self.visual_levels.len() as u32);
            for v in &self.visual_levels {
                w.i32(v.u01);
                w.f32(v.u02);
            }
        }
        if version >= 23 {
            w.u32(self.anchor_infos.len() as u32);
            for a in &self.anchor_infos {
                w.bool32(a.u01);
                w.bool32(a.u02);
                w.floats(&a.u03);
                w.string(&a.u04);
                w.i32(a.u05);
            }
        }
        if version >= 22 {
            w.u32(self.groups.len() as u32);
            for g in &self.groups {
                if version >= 31 {
                    w.i32(g.u01);
                }
                if version >= 36 {
                    w.u8(g.u02 as u8);
                } else {
                    w.i32(g.u02);
                }
                w.i32(g.u03);
                w.string(&g.name);
                w.i32(g.u04);
                w.u32(g.u05.len() as u32);
                for c in &g.u05 {
                    w.i32(*c);
                }
            }
        }
        if version >= 25 {
            if version < 29 {
                w.bool32(self.is_embedded_extra[0]);
                w.bool32(self.is_embedded_extra[1]);
            }
            if version >= 34 {
                w.u8(self.is_embedded as u8);
            } else {
                w.bool32(self.is_embedded);
            }
            if version >= 33 {
                w.i32(self.u02);
                w.i32(self.u03);
            }
        }
        let npos = self.positions.len();
        w.u32(npos as u32);
        for p in &self.positions {
            w.floats(p);
        }
        if version >= 35 {
            w.u32(self.edge_count);
            w.u32(self.edges.len() as u32);
            for e in &self.edges {
                w.opt(e[0], self.edges.len());
                w.opt(e[1], self.edges.len());
            }
        } else {
            w.u32(self.edges.len() as u32);
            for e in &self.edges {
                w.u32(e[0]);
                w.u32(e[1]);
            }
        }
        w.u32(self.faces.len() as u32);
        if version >= 37 {
            w.u32(self.tex_coords.len() as u32);
            for t in &self.tex_coords {
                w.floats(t);
            }
            let idx: Vec<u32> = self.faces.iter().flat_map(|f| f.uv_index.iter().copied()).collect();
            w.opt_array(&idx);
        }
        for f in &self.faces {
            if version >= 35 {
                w.u8((f.verts.len() - 3) as u8);
            } else {
                w.u32(f.verts.len() as u32);
            }
            for v in &f.verts {
                if version >= 34 {
                    w.opt(*v, npos);
                } else {
                    w.u32(*v);
                }
            }
            if version < 27 {
                w.u32(f.uvs.len() as u32);
                for uv in &f.uvs {
                    w.floats(uv);
                }
                w.floats(&f.u01.unwrap_or([0.0; 3]));
            } else if version < 37 {
                for uv in &f.uvs {
                    w.floats(uv);
                }
            }
            if version >= 25 {
                if version >= 33 && material_count > 0 {
                    w.opt(f.material as u32, material_count);
                } else {
                    w.i32(f.material);
                }
            }
            if version >= 33 {
                w.opt(f.group, self.groups.len());
            } else {
                w.u32(f.group);
            }
        }
        for (i, _) in self.faces.iter().enumerate() {
            if version < 30 {
                w.i32(self.face_extra.get(i).and_then(|e| e.first().copied()).unwrap_or(0));
            }
        }
        if version < 29 {
            for i in 0..npos {
                w.f32(self.position_extra.get(i).copied().unwrap_or(0.0));
            }
        }
        w.i32(self.u04);
        if (7..32).contains(&version) {
            w.u32(0);
            if version >= 10 {
                w.i32(self.u05);
                w.string(&self.u06);
                if version < 30 {
                    w.u32(self.old_smoothing.len() as u32);
                    w.floats(&self.old_smoothing);
                }
            }
        }
        if version < 36 {
            let (a, b, d) = self.old_blocks.clone().unwrap_or_default();
            w.u32((a.len() / 4) as u32);
            w.u32((b.len() / 4) as u32);
            w.u32((d.len() / 4) as u32);
            w.w.extend_from_slice(&a);
            w.w.extend_from_slice(&b);
            w.w.extend_from_slice(&d);
            w.i32(self.u07);
        }
    }

    /// Texture coordinates of a face's corners (v37 indices resolved).
    pub fn face_uvs(&self, f: &Face) -> Vec<[f32; 2]> {
        if self.version >= 37 {
            f.uv_index.iter().map(|i| self.tex_coords[*i as usize]).collect()
        } else {
            f.uvs.clone()
        }
    }
}

// --------------------------------------------------------------------- layers

pub const LAYER_GEOMETRY: u32 = 0;
pub const LAYER_SUBDIVIDE_SMOOTH: u32 = 1;
pub const LAYER_TRANSLATION: u32 = 2;
pub const LAYER_ROTATION: u32 = 3;
pub const LAYER_SCALE: u32 = 4;
pub const LAYER_MIRROR: u32 = 5;
pub const LAYER_MOVE_TO_GROUND: u32 = 6;
pub const LAYER_EXTRUDE: u32 = 7;
pub const LAYER_SUBDIVIDE: u32 = 8;
pub const LAYER_CHAOS: u32 = 9;
pub const LAYER_SMOOTH: u32 = 10;
pub const LAYER_BORDER_TRANSITION: u32 = 11;
pub const LAYER_DEFORMATION: u32 = 12;
pub const LAYER_CUBES: u32 = 13;
pub const LAYER_TRIGGER: u32 = 14;
pub const LAYER_SPAWN_POSITION: u32 = 15;
pub const LAYER_LIGHT: u32 = 18;

/// Fields every layer starts with (`archive Layer`).
#[derive(Clone, Debug, PartialEq)]
pub struct LayerBase {
    pub version: u32,
    pub crystal_enabled: bool,
    pub layer_id: Id,
    pub layer_name: String,
    /// v1+
    pub is_enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PartInLayer {
    pub group_index: i32,
    pub layer_id: Id,
}

/// `archive ModifierLayer`: what a modifier layer applies to.
#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    pub version: u32,
    pub mask: Vec<PartInLayer>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightPos {
    pub u01: i32,
    pub u02: [f32; 12],
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayerKind {
    Geometry {
        version: u32,
        crystal: Crystal,
        /// One int per group.
        u02: Vec<i32>,
        /// v1+
        is_visible: bool,
        collidable: bool,
    },
    SubdivideSmooth { modifier: Modifier, version: u32, subdivisions: i32 },
    Translation { modifier: Modifier, version: u32, translation: [f32; 3] },
    Rotation { modifier: Modifier, version: u32, rotation: f32, axis: i32, independently: bool },
    Scale { modifier: Modifier, version: u32, scale: [f32; 3], independently: bool },
    Mirror { modifier: Modifier, version: u32, axis: i32, distance: f32, independently: bool },
    MoveToGround { modifier: Modifier, version: u32, u01: bool },
    Extrude { modifier: Modifier, version: u32, size: [f32; 3] },
    Subdivide { modifier: Modifier, version: u32, subdivisions: i32 },
    Chaos {
        modifier: Modifier,
        version: u32,
        min_distance: f32,
        u01: i32,
        /// v1+
        max_distance: f32,
    },
    Smooth { modifier: Modifier, version: u32, factor: f32, independently: bool },
    BorderTransition { modifier: Modifier, version: u32, u01: f32, u02: f32, visuals: Vec<NodeRef<OpaqueNode>> },
    Deformation { modifier: Modifier, version: u32, box_aligned: [f32; 6], iso4: [f32; 12] },
    Trigger {
        version: u32,
        crystal: Crystal,
        /// v1+
        u01: Vec<i32>,
    },
    SpawnPosition {
        modifier: Modifier,
        version: u32,
        position: [f32; 3],
        horizontal_angle: f32,
        vertical_angle: f32,
        /// v1+
        roll_angle: f32,
    },
    Light { modifier: Modifier, version: u32, lights: Vec<NodeRef<OpaqueNode>>, positions: Vec<LightPos> },
}

impl LayerKind {
    pub fn type_id(&self) -> u32 {
        match self {
            LayerKind::Geometry { .. } => LAYER_GEOMETRY,
            LayerKind::SubdivideSmooth { .. } => LAYER_SUBDIVIDE_SMOOTH,
            LayerKind::Translation { .. } => LAYER_TRANSLATION,
            LayerKind::Rotation { .. } => LAYER_ROTATION,
            LayerKind::Scale { .. } => LAYER_SCALE,
            LayerKind::Mirror { .. } => LAYER_MIRROR,
            LayerKind::MoveToGround { .. } => LAYER_MOVE_TO_GROUND,
            LayerKind::Extrude { .. } => LAYER_EXTRUDE,
            LayerKind::Subdivide { .. } => LAYER_SUBDIVIDE,
            LayerKind::Chaos { .. } => LAYER_CHAOS,
            LayerKind::Smooth { .. } => LAYER_SMOOTH,
            LayerKind::BorderTransition { .. } => LAYER_BORDER_TRANSITION,
            LayerKind::Deformation { .. } => LAYER_DEFORMATION,
            LayerKind::Trigger { .. } => LAYER_TRIGGER,
            LayerKind::SpawnPosition { .. } => LAYER_SPAWN_POSITION,
            LayerKind::Light { .. } => LAYER_LIGHT,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            LayerKind::Geometry { .. } => "Geometry",
            LayerKind::SubdivideSmooth { .. } => "SubdivideSmooth",
            LayerKind::Translation { .. } => "Translation",
            LayerKind::Rotation { .. } => "Rotation",
            LayerKind::Scale { .. } => "Scale",
            LayerKind::Mirror { .. } => "Mirror",
            LayerKind::MoveToGround { .. } => "MoveToGround",
            LayerKind::Extrude { .. } => "Extrude",
            LayerKind::Subdivide { .. } => "Subdivide",
            LayerKind::Chaos { .. } => "Chaos",
            LayerKind::Smooth { .. } => "Smooth",
            LayerKind::BorderTransition { .. } => "BorderTransition",
            LayerKind::Deformation { .. } => "Deformation",
            LayerKind::Trigger { .. } => "Trigger",
            LayerKind::SpawnPosition { .. } => "SpawnPosition",
            LayerKind::Light { .. } => "Light",
        }
    }
    /// The mesh a Geometry or Trigger layer carries.
    pub fn crystal(&self) -> Option<&Crystal> {
        match self {
            LayerKind::Geometry { crystal, .. } | LayerKind::Trigger { crystal, .. } => Some(crystal),
            _ => None,
        }
    }
    pub fn crystal_mut(&mut self) -> Option<&mut Crystal> {
        match self {
            LayerKind::Geometry { crystal, .. } | LayerKind::Trigger { crystal, .. } => Some(crystal),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub base: LayerBase,
    pub kind: LayerKind,
}

fn read_modifier(r: &mut Rd) -> R<Modifier> {
    Ok(Modifier { version: r.u32()?, mask: r.array(|r| Ok(PartInLayer { group_index: r.i32()?, layer_id: r.id()? }))? })
}

fn write_modifier(w: &mut Wr, m: &Modifier) {
    w.u32(m.version);
    w.u32(m.mask.len() as u32);
    for p in &m.mask {
        w.i32(p.group_index);
        w.id(&p.layer_id);
    }
}

impl Layer {
    pub fn parse(r: &mut Rd, material_count: usize) -> R<Layer> {
        let ty = r.u32()?;
        let version = r.u32()?;
        let crystal_enabled = r.bool32()?;
        let layer_id = r.id()?;
        let layer_name = r.string()?;
        let is_enabled = if version >= 1 { r.bool32()? } else { true };
        let base = LayerBase { version, crystal_enabled, layer_id, layer_name, is_enabled };
        let kind = match ty {
            LAYER_GEOMETRY => {
                let version = r.u32()?;
                let crystal = Crystal::parse(r, material_count)?;
                let u02 = r.array(|r| r.i32())?;
                let (is_visible, collidable) = if version >= 1 { (r.bool32()?, r.bool32()?) } else { (true, true) };
                LayerKind::Geometry { version, crystal, u02, is_visible, collidable }
            }
            LAYER_SUBDIVIDE_SMOOTH => {
                let modifier = read_modifier(r)?;
                LayerKind::SubdivideSmooth { modifier, version: r.u32()?, subdivisions: r.i32()? }
            }
            LAYER_TRANSLATION => {
                let modifier = read_modifier(r)?;
                LayerKind::Translation { modifier, version: r.u32()?, translation: r.vec3()? }
            }
            LAYER_ROTATION => {
                let modifier = read_modifier(r)?;
                LayerKind::Rotation { modifier, version: r.u32()?, rotation: r.f32()?, axis: r.i32()?, independently: r.bool32()? }
            }
            LAYER_SCALE => {
                let modifier = read_modifier(r)?;
                LayerKind::Scale { modifier, version: r.u32()?, scale: r.vec3()?, independently: r.bool32()? }
            }
            LAYER_MIRROR => {
                let modifier = read_modifier(r)?;
                LayerKind::Mirror { modifier, version: r.u32()?, axis: r.i32()?, distance: r.f32()?, independently: r.bool32()? }
            }
            LAYER_MOVE_TO_GROUND => {
                let modifier = read_modifier(r)?;
                LayerKind::MoveToGround { modifier, version: r.u32()?, u01: r.bool32()? }
            }
            LAYER_EXTRUDE => {
                let modifier = read_modifier(r)?;
                LayerKind::Extrude { modifier, version: r.u32()?, size: r.vec3()? }
            }
            LAYER_SUBDIVIDE => {
                let modifier = read_modifier(r)?;
                LayerKind::Subdivide { modifier, version: r.u32()?, subdivisions: r.i32()? }
            }
            LAYER_CHAOS => {
                let modifier = read_modifier(r)?;
                let version = r.u32()?;
                let min_distance = r.f32()?;
                let u01 = r.i32()?;
                let max_distance = if version >= 1 { r.f32()? } else { 0.0 };
                LayerKind::Chaos { modifier, version, min_distance, u01, max_distance }
            }
            LAYER_SMOOTH => {
                let modifier = read_modifier(r)?;
                LayerKind::Smooth { modifier, version: r.u32()?, factor: r.f32()?, independently: r.bool32()? }
            }
            LAYER_BORDER_TRANSITION => {
                let modifier = read_modifier(r)?;
                let version = r.u32()?;
                let u01 = r.f32()?;
                let u02 = r.f32()?;
                let visuals = r.array(|r| r.noderef(read_opaque))?;
                LayerKind::BorderTransition { modifier, version, u01, u02, visuals }
            }
            LAYER_DEFORMATION => {
                let modifier = read_modifier(r)?;
                LayerKind::Deformation { modifier, version: r.u32()?, box_aligned: r.floats()?, iso4: r.floats()? }
            }
            LAYER_CUBES => return Err(format!("Cubes layer at 0x{:x}: VoxelSpace has no known layout (GBX.NET: throw)", r.o)),
            LAYER_TRIGGER => {
                let version = r.u32()?;
                let crystal = Crystal::parse(r, material_count)?;
                let u01 = if version >= 1 { r.array(|r| r.i32())? } else { Vec::new() };
                LayerKind::Trigger { version, crystal, u01 }
            }
            LAYER_SPAWN_POSITION => {
                let modifier = read_modifier(r)?;
                let version = r.u32()?;
                let position = r.vec3()?;
                let horizontal_angle = r.f32()?;
                let vertical_angle = r.f32()?;
                let roll_angle = if version >= 1 { r.f32()? } else { 0.0 };
                LayerKind::SpawnPosition { modifier, version, position, horizontal_angle, vertical_angle, roll_angle }
            }
            LAYER_LIGHT => {
                let modifier = read_modifier(r)?;
                let version = r.u32()?;
                let lights = r.array(|r| r.noderef(read_opaque))?;
                let positions = r.array(|r| Ok(LightPos { u01: r.i32()?, u02: r.floats()? }))?;
                LayerKind::Light { modifier, version, lights, positions }
            }
            t => return Err(format!("layer type {} at 0x{:x} is not a CPlugCrystal layer type", t, r.o)),
        };
        Ok(Layer { base, kind })
    }

    pub fn write(&self, w: &mut Wr, material_count: usize) {
        w.u32(self.kind.type_id());
        w.u32(self.base.version);
        w.bool32(self.base.crystal_enabled);
        w.id(&self.base.layer_id);
        w.string(&self.base.layer_name);
        if self.base.version >= 1 {
            w.bool32(self.base.is_enabled);
        }
        match &self.kind {
            LayerKind::Geometry { version, crystal, u02, is_visible, collidable } => {
                w.u32(*version);
                crystal.write(w, material_count);
                w.u32(u02.len() as u32);
                for x in u02 {
                    w.i32(*x);
                }
                if *version >= 1 {
                    w.bool32(*is_visible);
                    w.bool32(*collidable);
                }
            }
            LayerKind::SubdivideSmooth { modifier, version, subdivisions } | LayerKind::Subdivide { modifier, version, subdivisions } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.i32(*subdivisions);
            }
            LayerKind::Translation { modifier, version, translation } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.floats(translation);
            }
            LayerKind::Rotation { modifier, version, rotation, axis, independently } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.f32(*rotation);
                w.i32(*axis);
                w.bool32(*independently);
            }
            LayerKind::Scale { modifier, version, scale, independently } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.floats(scale);
                w.bool32(*independently);
            }
            LayerKind::Mirror { modifier, version, axis, distance, independently } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.i32(*axis);
                w.f32(*distance);
                w.bool32(*independently);
            }
            LayerKind::MoveToGround { modifier, version, u01 } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.bool32(*u01);
            }
            LayerKind::Extrude { modifier, version, size } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.floats(size);
            }
            LayerKind::Chaos { modifier, version, min_distance, u01, max_distance } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.f32(*min_distance);
                w.i32(*u01);
                if *version >= 1 {
                    w.f32(*max_distance);
                }
            }
            LayerKind::Smooth { modifier, version, factor, independently } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.f32(*factor);
                w.bool32(*independently);
            }
            LayerKind::BorderTransition { modifier, version, u01, u02, visuals } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.f32(*u01);
                w.f32(*u02);
                w.u32(visuals.len() as u32);
                for v in visuals {
                    w.noderef(v, v.inline.as_ref().map(|n| n.class_id).unwrap_or(0), write_opaque);
                }
            }
            LayerKind::Deformation { modifier, version, box_aligned, iso4 } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.floats(box_aligned);
                w.floats(iso4);
            }
            LayerKind::Trigger { version, crystal, u01 } => {
                w.u32(*version);
                crystal.write(w, material_count);
                if *version >= 1 {
                    w.u32(u01.len() as u32);
                    for x in u01 {
                        w.i32(*x);
                    }
                }
            }
            LayerKind::SpawnPosition { modifier, version, position, horizontal_angle, vertical_angle, roll_angle } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.floats(position);
                w.f32(*horizontal_angle);
                w.f32(*vertical_angle);
                if *version >= 1 {
                    w.f32(*roll_angle);
                }
            }
            LayerKind::Light { modifier, version, lights, positions } => {
                write_modifier(w, modifier);
                w.u32(*version);
                w.u32(lights.len() as u32);
                for l in lights {
                    w.noderef(l, l.inline.as_ref().map(|n| n.class_id).unwrap_or(0), write_opaque);
                }
                w.u32(positions.len() as u32);
                for p in positions {
                    w.i32(p.u01);
                    w.floats(&p.u02);
                }
            }
        }
    }
}

// ------------------------------------------------------------- lightmap, node

/// Chunk 0x09003006, by version.
#[derive(Clone, Debug, PartialEq)]
pub enum Lightmap {
    /// v0: one float pair per face corner of the visible geometry layers.
    V0(Vec<[f32; 2]>),
    /// v1: one u16 pair per corner.
    V1(Vec<[u16; 2]>),
    /// v2: de-duplicated u16 pairs and one index per corner.
    V2 { coords: Vec<[u16; 2]>, indices: Vec<u32> },
}

impl Lightmap {
    pub fn version(&self) -> u32 {
        match self {
            Lightmap::V0(_) => 0,
            Lightmap::V1(_) => 1,
            Lightmap::V2 { .. } => 2,
        }
    }
}

/// The `CPlugCrystal` node: its chunks in file order.
#[derive(Clone, Debug, PartialEq)]
pub struct CPlugCrystal {
    /// Chunk ids in the order the file has them; `write` follows it.
    pub chunks: Vec<u32>,
    /// Chunk 0x09051000 (CPlugTreeGenerator, inherited): one int.
    pub tree_generator: Option<i32>,
    /// Chunk 0x09003000 (one layer, pre-layers files): version, crystal.
    pub single_layer: Option<(u32, Crystal)>,
    /// Chunk 0x09003003.
    pub materials_version: u32,
    pub materials: Vec<Material>,
    /// Chunk 0x09003004, raw.
    pub chunk4: Option<Chunk4>,
    /// Chunk 0x09003005.
    pub layers_version: u32,
    pub layers: Vec<Layer>,
    /// Chunk 0x09003006.
    pub lightmap: Option<Lightmap>,
    /// Chunk 0x09003007: version, one float per smoothing group, one int per
    /// face of the geometry layers.
    pub smoothing_version: u32,
    pub smoothing_groups: Vec<f32>,
    pub per_face_ints: Vec<i32>,
}

impl CPlugCrystal {
    /// Parse the node body that starts at `at` (the word after the class id)
    /// with a seeded lookback/node state; returns the node and the offset
    /// just past its FACADE.
    pub fn parse_with(body: &[u8], at: usize, lb: LookbackState) -> R<(CPlugCrystal, usize, LookbackState)> {
        let mut r = Rd::new(body, at, lb);
        let c = Self::read(&mut r)?;
        Ok((c, r.o, r.lb))
    }

    /// Parse the crystal at `at`, seeding the lookback table by walking the
    /// item body from its start (`locate`); when that walk fails, the table
    /// starts empty and pre-crystal back references come out as `Id::Prior`.
    pub fn parse(body: &[u8], at: usize) -> R<(CPlugCrystal, usize)> {
        let lb = match locate(body) {
            Ok(l) if l.at == at => l.lookback,
            _ => LookbackState::default(),
        };
        let (c, end, _) = Self::parse_with(body, at, lb)?;
        Ok((c, end))
    }

    pub fn read(r: &mut Rd) -> R<CPlugCrystal> {
        let mut c = CPlugCrystal {
            chunks: Vec::new(),
            tree_generator: None,
            single_layer: None,
            materials_version: 2,
            materials: Vec::new(),
            chunk4: None,
            layers_version: 0,
            layers: Vec::new(),
            lightmap: None,
            smoothing_version: 0,
            smoothing_groups: Vec::new(),
            per_face_ints: Vec::new(),
        };
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            c.chunks.push(cid);
            match cid {
                C_TREE_GENERATOR_CHUNK => c.tree_generator = Some(r.i32()?),
                0x09003000 => {
                    let v = r.u32()?;
                    let crystal = Crystal::parse(r, c.materials.len())?;
                    c.single_layer = Some((v, crystal));
                }
                0x09003003 => {
                    c.materials_version = r.u32()?;
                    c.materials = r.array(|r| {
                        let name = r.string()?;
                        let node = if name.is_empty() {
                            Some(r.noderef(|r, class_id| {
                                if class_id != C_MATERIAL_USER_INST {
                                    return Err(format!("material node class 0x{:08X} is not CPlugMaterialUserInst", class_id));
                                }
                                CPlugMaterialUserInst::parse(r)
                            })?)
                        } else {
                            None
                        };
                        Ok(Material { name, node })
                    })?;
                }
                0x09003004 => {
                    if r.take(4)? != SKIP {
                        return Err(format!("chunk 0x09003004 at 0x{:x} is not skippable", at));
                    }
                    let size = r.count()?;
                    let end = r.o + size;
                    let version = r.u32()?;
                    let n = r.count()?;
                    let data = r.take(n)?.to_vec();
                    let u01 = if version >= 1 && r.o + 4 <= end { Some(r.i32()?) } else { None };
                    if r.o > end {
                        return Err("chunk 0x09003004 overran its size".into());
                    }
                    let trailing = r.take(end - r.o)?.to_vec();
                    c.chunk4 = Some(Chunk4 { version, data, u01, trailing });
                }
                0x09003005 => {
                    let (v, layers) = read_layers_chunk(r, c.materials.len())?;
                    c.layers_version = v;
                    c.layers = layers;
                }
                0x09003006 => {
                    let v = r.u32()?;
                    let n = r.count()?;
                    c.lightmap = Some(match v {
                        0 => Lightmap::V0((0..n).map(|_| r.vec2()).collect::<R<_>>()?),
                        1 => Lightmap::V1((0..n).map(|_| Ok([r.u16()?, r.u16()?])).collect::<R<_>>()?),
                        _ => {
                            let coords: Vec<[u16; 2]> = (0..n).map(|_| Ok([r.u16()?, r.u16()?])).collect::<R<_>>()?;
                            let indices = r.opt_array()?;
                            Lightmap::V2 { coords, indices }
                        }
                    });
                }
                0x09003007 => {
                    c.smoothing_version = r.u32()?;
                    c.smoothing_groups = r.array(|r| r.f32())?;
                    c.per_face_ints = r.array(|r| r.i32())?;
                }
                other => return Err(format!("CPlugCrystal chunk 0x{:08X} at 0x{:x} has no reader", other, at)),
            }
        }
        Ok(c)
    }

    /// Serialise the node body (chunks then FACADE) after its class id.
    pub fn write(&self, w: &mut Vec<u8>, lookback: &mut LookbackState) {
        let mut w = Wr { w, lb: lookback };
        for cid in &self.chunks {
            w.u32(*cid);
            match *cid {
                C_TREE_GENERATOR_CHUNK => w.i32(self.tree_generator.unwrap_or(1)),
                0x09003000 => {
                    let (v, crystal) = self.single_layer.as_ref().expect("chunk 0x09003000 listed but absent");
                    w.u32(*v);
                    crystal.write(&mut w, self.materials.len());
                }
                0x09003003 => {
                    w.u32(self.materials_version);
                    w.u32(self.materials.len() as u32);
                    for m in &self.materials {
                        w.string(&m.name);
                        if m.name.is_empty() {
                            let n = m.node.as_ref().expect("a nameless material needs a node");
                            w.noderef(n, C_MATERIAL_USER_INST, |w, inst| inst.write(w));
                        }
                    }
                }
                0x09003004 => {
                    let c4 = self.chunk4.as_ref().expect("chunk 0x09003004 listed but absent");
                    w.w.extend_from_slice(SKIP);
                    let mut payload = Vec::new();
                    payload.extend_from_slice(&c4.version.to_le_bytes());
                    payload.extend_from_slice(&(c4.data.len() as u32).to_le_bytes());
                    payload.extend_from_slice(&c4.data);
                    if let Some(u) = c4.u01 {
                        payload.extend_from_slice(&u.to_le_bytes());
                    }
                    payload.extend_from_slice(&c4.trailing);
                    w.u32(payload.len() as u32);
                    w.w.extend_from_slice(&payload);
                }
                0x09003005 => {
                    w.u32(self.layers_version);
                    w.u32(self.layers.len() as u32);
                    for l in &self.layers {
                        l.write(&mut w, self.materials.len());
                    }
                }
                0x09003006 => {
                    let lm = self.lightmap.as_ref().expect("chunk 0x09003006 listed but absent");
                    w.u32(lm.version());
                    match lm {
                        Lightmap::V0(v) => {
                            w.u32(v.len() as u32);
                            for c in v {
                                w.floats(c);
                            }
                        }
                        Lightmap::V1(v) => {
                            w.u32(v.len() as u32);
                            for c in v {
                                w.u16(c[0]);
                                w.u16(c[1]);
                            }
                        }
                        Lightmap::V2 { coords, indices } => {
                            w.u32(coords.len() as u32);
                            for c in coords {
                                w.u16(c[0]);
                                w.u16(c[1]);
                            }
                            w.opt_array(indices);
                        }
                    }
                }
                0x09003007 => {
                    w.u32(self.smoothing_version);
                    w.u32(self.smoothing_groups.len() as u32);
                    w.floats(&self.smoothing_groups);
                    w.u32(self.per_face_ints.len() as u32);
                    for x in &self.per_face_ints {
                        w.i32(*x);
                    }
                }
                c => panic!("CPlugCrystal chunk 0x{c:08X} has no writer"),
            }
        }
        w.u32(FACADE);
    }

    /// Node indices of the inline material nodes, in slot order.
    pub fn material_node_indices(&self) -> Vec<i32> {
        self.materials.iter().filter_map(|m| m.node.as_ref().map(|n| n.index)).collect()
    }

    /// Every node index referenced inside the node, with a mutable handle,
    /// so a renumbering can move them all.
    pub fn node_indices_mut(&mut self) -> Vec<&mut i32> {
        let mut out: Vec<&mut i32> = Vec::new();
        for m in &mut self.materials {
            if let Some(n) = &mut m.node {
                out.push(&mut n.index);
                if let Some(inst) = &mut n.inline {
                    if let Some(t) = &mut inst.tiling {
                        out.push(&mut t.atlas.index);
                    }
                }
            }
        }
        for l in &mut self.layers {
            match &mut l.kind {
                LayerKind::BorderTransition { visuals, .. } => out.extend(visuals.iter_mut().map(|v| &mut v.index)),
                LayerKind::Light { lights, .. } => out.extend(lights.iter_mut().map(|v| &mut v.index)),
                _ => {}
            }
        }
        out
    }

    /// The first Geometry layer.
    pub fn first_geometry(&self) -> Option<&Layer> {
        self.layers.iter().find(|l| matches!(l.kind, LayerKind::Geometry { .. }))
    }
    pub fn first_geometry_mut(&mut self) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| matches!(l.kind, LayerKind::Geometry { .. }))
    }
    /// Face count of the geometry layers the lightmap and smoothing chunks
    /// cover (enabled and visible ones, per GBX.NET's chunk 006 reader).
    pub fn lit_face_count(&self) -> usize {
        self.layers
            .iter()
            .filter_map(|l| match &l.kind {
                LayerKind::Geometry { crystal, is_visible, .. } if l.base.is_enabled && *is_visible => Some(crystal.faces.len()),
                _ => None,
            })
            .sum()
    }
}

// ------------------------------------------------------------ the item body

/// Where an item body's crystal is, and the walk state on arrival.
#[derive(Clone, Debug)]
pub struct Located {
    /// Offset of the crystal node's first chunk id (just after its class id).
    pub at: usize,
    /// The crystal's own node index.
    pub node_index: u32,
    /// Node index of the CGameCommonItemEntityModelEdition holding it.
    pub edition_index: u32,
    pub lookback: LookbackState,
}

/// Walk a `CGameItemModel` body from its start to the `CPlugCrystal` node
/// inside its `CGameCommonItemEntityModelEdition` (chunk 0x2E002019 ->
/// chunk 0x2E026000 -> MeshCrystal). Reads exactly the fields the chunks
/// before it carry, so the lookback table and the defined-node set on
/// arrival are the game's own.
pub fn locate(body: &[u8]) -> R<Located> {
    let mut r = Rd::new(body, 0, LookbackState::default());
    let null_or_defined = |r: &mut Rd, what: &str| -> R<()> {
        let n = r.noderef(|r, class_id| Err::<(), _>(format!("inline node of class 0x{:08X} at 0x{:x}", class_id, r.o)))
            .map_err(|e| format!("{what}: {e}"))?;
        let _ = n;
        Ok(())
    };
    loop {
        let at = r.o;
        let cid = r.u32()?;
        if cid == FACADE {
            return Err("item body ended before chunk 0x2E002019".into());
        }
        if r.b.get(r.o..r.o + 4) == Some(SKIP) {
            r.u32()?;
            let n = r.count()?;
            r.take(n)?;
            continue;
        }
        match cid {
            0x2E001009 => {
                r.string()?;
                if r.bool32()? {
                    null_or_defined(&mut r, "collector icon")?;
                }
                r.id()?;
            }
            0x2E00100B => {
                r.id()?;
                r.id()?;
                r.id()?;
            }
            0x2E00100C | 0x2E00100D => {
                r.string()?;
            }
            0x2E00100E => {
                r.take(8)?;
            }
            0x2E001010 => {
                let v = r.u32()?;
                null_or_defined(&mut r, "collector 0x2E001010")?;
                let skin = r.string()?;
                if v >= 2 && skin.is_empty() {
                    null_or_defined(&mut r, "collector 0x2E001010 skin")?;
                }
            }
            0x2E001011 => {
                let v = r.u32()?;
                r.take(12)?;
                if v >= 1 {
                    r.u8()?;
                }
            }
            0x2E001012 => {
                r.take(16)?;
            }
            0x2E002008 => {
                let n = r.count()?;
                for _ in 0..n {
                    null_or_defined(&mut r, "item 0x2E002008")?;
                }
            }
            0x2E002009 => {
                r.u32()?;
                let n = r.count()?;
                for _ in 0..n {
                    null_or_defined(&mut r, "item 0x2E002009")?;
                }
            }
            0x2E00200C | 0x2E002013 | 0x2E00201A => {
                null_or_defined(&mut r, "item 0x2E00200C")?;
            }
            0x2E002012 => {
                r.take(12 + 16)?;
            }
            0x2E002015 => {
                r.u32()?;
            }
            0x2E002019 => {
                let v = r.u32()?;
                if v < 8 {
                    return Err(format!("chunk 0x2E002019 version {v} has no entity model edition"));
                }
                r.id()?; // defaultWeaponName
                null_or_defined(&mut r, "PhyModelCustom")?;
                null_or_defined(&mut r, "VisModelCustom")?;
                r.u32()?;
                r.u32()?; // defaultCam
                let edition_index = r.i32()?;
                if edition_index <= 0 || r.lb.defined_nodes.contains(&(edition_index as u32)) {
                    return Err("item has no inline CGameCommonItemEntityModelEdition (not a crystal item)".into());
                }
                r.lb.defined_nodes.insert(edition_index as u32);
                let class_id = r.u32()?;
                if class_id != 0x2E026000 {
                    return Err(format!("entity model class 0x{:08X} is not CGameCommonItemEntityModelEdition", class_id));
                }
                loop {
                    let cid = r.u32()?;
                    if cid == FACADE {
                        return Err("entity model edition has no chunk 0x2E026000".into());
                    }
                    if r.b.get(r.o..r.o + 4) == Some(SKIP) {
                        r.u32()?;
                        let n = r.count()?;
                        r.take(n)?;
                        continue;
                    }
                    if cid != 0x2E026000 {
                        return Err(format!("entity model edition chunk 0x{:08X} before 0x2E026000 has no reader", cid));
                    }
                    r.u32()?; // version
                    r.u32()?; // item type
                    let node_index = r.i32()?;
                    if node_index <= 0 || r.lb.defined_nodes.contains(&(node_index as u32)) {
                        return Err("entity model edition has no inline MeshCrystal".into());
                    }
                    r.lb.defined_nodes.insert(node_index as u32);
                    let class_id = r.u32()?;
                    if class_id != C_CRYSTAL {
                        return Err(format!("MeshCrystal class 0x{:08X} is not CPlugCrystal", class_id));
                    }
                    return Ok(Located { at: r.o, node_index: node_index as u32, edition_index: edition_index as u32, lookback: r.lb });
                }
            }
            other => return Err(format!("item chunk 0x{:08X} at 0x{:x} before the entity model has no reader", other, at)),
        }
    }
}

/// The content of chunk 0x09003005 after its id: (chunk version, layers).
/// `material_count` is the crystal's material slot count, which sizes the
/// per-face material index.
pub fn read_layers_chunk(r: &mut Rd, material_count: usize) -> R<(u32, Vec<Layer>)> {
    let version = r.u32()?;
    let n = r.count()?;
    let mut layers = Vec::with_capacity(n);
    for i in 0..n {
        let l = Layer::parse(r, material_count).map_err(|e| format!("layer {} at 0x{:x}: {}", i, r.o, e))?;
        layers.push(l);
    }
    Ok((version, layers))
}
