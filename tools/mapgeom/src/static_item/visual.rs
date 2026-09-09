//! `CPlugVisualIndexedTriangles` (0x0901E000): the `CPlugVisual` chunks
//! (0x09006001/004/005/009/00B/00F/010), `CPlugVisual3D` (0x0902C002/004),
//! `CPlugVisualIndexed` (0x0906A001) with its inline `CPlugIndexBuffer`
//! (written with `WriteNode`: chunk ids directly, no class id, no index).

use super::{read_ref, write_ref, Id, Rd, Ref, Wr, R, FACADE};

/// Chunk 0x0900600F (version 5 or 6 in TM2020).
#[derive(Clone, Debug, PartialEq)]
pub struct VisualMain {
    pub version: u32,
    /// The chunk's packed flag word (GBX.NET `ConvertChunkFlagsToFlags`
    /// undone). Bit 3 = geometry static, 5 = indexation static, 7 = vertex
    /// normals (in the unpacked form).
    pub chunk_flags: u32,
    pub tex_coord_sets: Vec<TexCoordSet>,
    pub count: i32,
    pub vertex_streams: Vec<Ref>,
    /// `SSkinData`, present when `flags & 7 != 0` (skinned).
    pub skin: Option<SkinData>,
    pub bounding_box: [f32; 6],
    /// BitmapElemToPack: five ints each.
    pub bitmap_elems: Vec<[i32; 5]>,
    /// v5+
    pub uv_groups: Vec<u16>,
    /// v6+
    pub u02: i32,
    pub u03: i32,
    pub u04: Vec<u8>,
}

impl VisualMain {
    /// GBX.NET's unpacked `Flags`.
    pub fn flags(&self) -> u32 {
        let c = self.chunk_flags;
        (c & 15) | ((c << 1) & 0x20) | ((c << 2) & 0x80) | ((c << 2) & 0x100) | ((c << 13) & 0x100000) | ((c << 13) & 0x200000) | ((c << 13) & 0x400000)
    }
}

/// `SSkinData` as chunk 0x0900600F reads it (archive version `2 + chunk
/// version`, so 7 or 8 here: the two extra bools are present).
#[derive(Clone, Debug, PartialEq)]
pub struct SkinData {
    pub u01: bool,
    pub u02: i32,
    pub u03: bool,
    pub u04: bool,
    pub bones: Vec<Id>,
    pub u07: Vec<i32>,
}

impl SkinData {
    fn read(r: &mut Rd, version: u32) -> R<SkinData> {
        let u01 = r.bool32()?;
        let u02 = r.i32()?;
        let (mut u03, mut u04) = (false, false);
        if version >= 5 {
            u03 = r.bool32()?;
            u04 = r.bool32()?;
        }
        if u03 {
            return Err("SkinData U03 is true (GBX.NET: throw)".into());
        }
        let n = r.count()?;
        if version == 2 {
            return Err("SkinData version 2 (with iso4s) is not modelled".into());
        }
        let bones = (0..n).map(|_| r.id()).collect::<R<_>>()?;
        let u07 = if version != 3 { r.array(|r| r.i32())? } else { Vec::new() };
        Ok(SkinData { u01, u02, u03, u04, bones, u07 })
    }
    fn write(&self, w: &mut Wr, version: u32) {
        w.bool32(self.u01);
        w.i32(self.u02);
        if version >= 5 {
            w.bool32(self.u03);
            w.bool32(self.u04);
        }
        w.u32(self.bones.len() as u32);
        self.bones.iter().for_each(|b| w.id(b));
        if version != 3 {
            w.u32(self.u07.len() as u32);
            self.u07.iter().for_each(|x| w.i32(*x));
        }
    }
}

/// A `TexCoordSet` (only version 3 is written by TM2020; older forms carry
/// per-coordinate ints).
#[derive(Clone, Debug, PartialEq)]
pub struct TexCoordSet {
    pub version: u32,
    pub flags: Option<i32>,
    pub coords: Vec<([f32; 2], Option<i32>, Option<i32>)>,
    pub u01: Vec<f32>,
}

impl TexCoordSet {
    fn read(r: &mut Rd, n: usize) -> R<TexCoordSet> {
        let version = r.u32()?;
        let mut flags = None;
        if version >= 3 {
            let actual = r.count()?;
            if actual != n {
                return Err(format!("TexCoordSet count {actual} != vertex count {n}"));
            }
            flags = Some(r.i32()?);
        }
        let mut coords = Vec::with_capacity(n);
        for _ in 0..n {
            let uv = r.vec2()?;
            let (mut a, mut b) = (None, None);
            if (1..3).contains(&version) {
                a = Some(r.i32()?);
                if version >= 2 {
                    b = Some(r.i32()?);
                }
            }
            coords.push((uv, a, b));
        }
        let mut u01 = Vec::new();
        if let Some(f) = flags {
            let k = (n * (f as usize)) & 0xFF;
            u01 = (0..k).map(|_| r.f32()).collect::<R<_>>()?;
        }
        Ok(TexCoordSet { version, flags, coords, u01 })
    }
    fn write(&self, w: &mut Wr) {
        w.u32(self.version);
        if self.version >= 3 {
            w.u32(self.coords.len() as u32);
            w.i32(self.flags.unwrap_or(256));
        }
        for (uv, a, b) in &self.coords {
            w.floats(uv);
            if (1..3).contains(&self.version) {
                w.i32(a.unwrap_or(0));
                if self.version >= 2 {
                    w.i32(b.unwrap_or(0));
                }
            }
        }
        if self.flags.is_some() {
            w.floats(&self.u01);
        }
    }
}

/// The inline `CPlugIndexBuffer` (chunk 0x09057000 plain u16s, or
/// 0x09057001 delta-coded i16s); `indices` is always the resolved list.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexBuffer {
    pub chunk: u32,
    pub flags: u32,
    pub indices: Vec<u32>,
}

impl IndexBuffer {
    pub fn delta(indices: Vec<u32>) -> IndexBuffer {
        IndexBuffer { chunk: 0x09057001, flags: 0, indices }
    }
    fn read(r: &mut Rd) -> R<IndexBuffer> {
        let chunk = r.u32()?;
        let flags = r.u32()?;
        let n = r.count()?;
        let mut indices = Vec::with_capacity(n);
        match chunk {
            0x09057000 => {
                for _ in 0..n {
                    indices.push(r.u16()? as u32);
                }
            }
            0x09057001 => {
                let mut cur: i32 = 0;
                for _ in 0..n {
                    cur += r.i16()? as i32;
                    indices.push(cur as u32);
                }
            }
            c => return Err(format!("CPlugIndexBuffer chunk 0x{c:08X} has no reader")),
        }
        let f = r.u32()?;
        if f != FACADE {
            return Err(format!("CPlugIndexBuffer: 0x{f:08X} is not FACADE"));
        }
        Ok(IndexBuffer { chunk, flags, indices })
    }
    fn write(&self, w: &mut Wr) {
        w.u32(self.chunk);
        w.u32(self.flags);
        w.u32(self.indices.len() as u32);
        match self.chunk {
            0x09057000 => self.indices.iter().for_each(|i| w.u16(*i as u16)),
            _ => {
                let mut cur: i32 = 0;
                for i in &self.indices {
                    w.i16((*i as i32 - cur) as i16);
                    cur = *i as i32;
                }
            }
        }
        w.u32(FACADE);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugVisualIndexedTriangles {
    /// Chunk ids in file order.
    pub chunks: Vec<u32>,
    /// 0x09006001
    pub id: Id,
    /// 0x09006004
    pub u_node: Ref,
    /// 0x09006005: int3 sub-visuals.
    pub sub_visuals: Vec<[i32; 3]>,
    /// 0x09006009
    pub u_float: f32,
    /// 0x0900600B: Split = int, int, box(6 floats).
    pub splits: Vec<(i32, i32, [f32; 6])>,
    pub main: Option<VisualMain>,
    /// 0x09006010: (version, morph count).
    pub morph: Option<(u32, i32)>,
    /// 0x0902C002
    pub v3d_node: Ref,
    /// 0x0902C004: raw per-vertex bytes of the two tangent arrays (with their
    /// counts); the vertex data itself lives in the streams.
    pub tangents: Option<(Vec<u8>, Vec<u8>)>,
    /// 0x0906A001
    pub index_buffer: Option<IndexBuffer>,
    /// Write the vertices INLINE in chunk 0x0902C004 (no `CPlugVertexStream`
    /// node), the form the pack's Dyna meshes use — `Dyna\Flag\Flag.Mesh.Gbx`
    /// keeps its 86 cloth frames that way and the vertex-tween draw path
    /// reads them from there (a stream-form copy with the frame table crashed
    /// the client in SCBufferDraw@NCharAnimSkelV, 2026-09-06). The stream
    /// stays the in-memory form; only the writer changes. `inline_uv_sets`
    /// texcoord sets are emitted (the pack flag has one).
    pub inline_form: bool,
    pub inline_uv_sets: usize,
    /// The texcoord sets' flags word (version-3 sets), as the source had it.
    pub inline_uv_flags: i32,
    /// Whether the source carried its two tangent arrays (the pack flag has
    /// none: two zero counts; the in-memory stream always has a frame).
    pub inline_tangents: bool,
}

impl CPlugVisualIndexedTriangles {
    pub fn parse(r: &mut Rd) -> R<CPlugVisualIndexedTriangles> {
        let mut v = CPlugVisualIndexedTriangles {
            chunks: Vec::new(),
            id: Id::Null,
            u_node: super::null_ref(),
            sub_visuals: Vec::new(),
            u_float: 0.0,
            splits: Vec::new(),
            main: None,
            morph: None,
            v3d_node: super::null_ref(),
            tangents: None,
            index_buffer: None,
            inline_form: false,
            inline_uv_sets: 1,
            inline_uv_flags: 256,
            inline_tangents: false,
        };
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            v.chunks.push(cid);
            match cid {
                0x09006001 => v.id = r.id()?,
                0x09006004 => v.u_node = read_ref(r)?,
                0x09006005 => v.sub_visuals = r.array(|r| Ok([r.i32()?, r.i32()?, r.i32()?]))?,
                0x09006009 => v.u_float = r.f32()?,
                0x0900600B => v.splits = r.array(|r| Ok((r.i32()?, r.i32()?, r.floats::<6>()?)))?,
                0x0900600F => v.main = Some(Self::parse_main(r)?),
                0x09006010 => {
                    let ver = r.u32()?;
                    let morph = r.i32()?;
                    if morph > 0 {
                        return Err("CPlugVisual morphs are not supported".into());
                    }
                    v.morph = Some((ver, morph));
                }
                0x0902C002 => v.v3d_node = read_ref(r)?,
                0x0902C004 => {
                    let m = v.main.as_ref().ok_or("chunk 0x0902C004 before 0x0900600F")?;
                    if m.vertex_streams.is_empty() {
                        // Inline `CPlugVisual3D` vertices (the Dyna meshes:
                        // `Dyna\Flag\Flag.Mesh.Gbx` keeps 43 animation frames of
                        // its cloth this way, 12384 = 43 x 288 vertices under a
                        // 726-index list). Read them as `classes.rs` does and
                        // rebuild the visual as a vertex-stream one, the only
                        // form the item side knows how to carry.
                        Self::parse_inline_into_stream(r, &mut v)?;
                        continue;
                    }
                    let per = ((!(m.flags() >> 17)) & 8) | 4;
                    let one = |r: &mut Rd| -> R<Vec<u8>> {
                        let n = r.count()?;
                        if n != 0 && n as i32 != m.count {
                            return Err(format!("tangent count {n} != vertex count {}", m.count));
                        }
                        Ok(r.take(n * per as usize)?.to_vec())
                    };
                    let a = one(r)?;
                    let b = one(r)?;
                    v.tangents = Some((a, b));
                }
                0x0906A001 => {
                    if r.bool32()? {
                        v.index_buffer = Some(IndexBuffer::read(r)?);
                    }
                }
                c => return Err(format!("CPlugVisual chunk 0x{c:08X} at 0x{at:x} has no reader")),
            }
        }
        Ok(v)
    }

    /// Chunk 0x0902C004 of a visual WITHOUT a vertex stream: the vertices are
    /// inline (position, normal, colour per the chunk-flags word, then the two
    /// tangent arrays). They are re-emitted as one `CPlugVertexStream` in the
    /// pack's 40-byte layout (position, normal, uv0, uv1, tangent U/V — plus a
    /// colour word when the inline form carried one), the texcoord sets moving
    /// into the stream, and the chunk-flags word set to the stream form's
    /// 0x38, so downstream code (scaling, merging, the game) sees an ordinary
    /// pack visual.
    fn parse_inline_into_stream(r: &mut Rd, v: &mut CPlugVisualIndexedTriangles) -> R<()> {
        use super::build::{dec3n_pack, dec3n_unpack};
        use super::vstream::*;
        use super::{Node, NodeRef};
        let m = v.main.as_mut().ok_or("chunk 0x0902C004 before 0x0900600F")?;
        let w = m.chunk_flags;
        let (use_normal, use_color, compress3, compress4, bit22) = (w & (1 << 5) != 0, w & (1 << 6) != 0, w & (1 << 7) != 0, w & (1 << 8) != 0, w & (1 << 9) != 0);
        let n = m.count.max(0) as usize;
        let mut pos = Vec::with_capacity(n);
        let mut nrm = Vec::with_capacity(n);
        let mut col: Vec<u32> = Vec::new();
        let has_color = !bit22 || use_color;
        for _ in 0..n {
            pos.push(r.vec3()?);
            if !bit22 && !compress4 && use_color {
                nrm.push(dec3n_pack(r.vec3()?));
                let c = r.floats::<4>()?;
                col.push(pack_color(c));
            } else {
                let nl = if !bit22 || use_normal {
                    if compress3 {
                        r.u32()?
                    } else {
                        dec3n_pack(r.vec3()?)
                    }
                } else {
                    dec3n_pack([0.0, 1.0, 0.0])
                };
                nrm.push(nl);
                if has_color {
                    if compress4 {
                        col.push(r.u32()?);
                    } else {
                        col.push(pack_color(r.floats::<4>()?));
                    }
                }
            }
        }
        let per = if compress3 { 4 } else { 12 };
        let read_tangents = |r: &mut Rd| -> R<Vec<u32>> {
            let k = r.count()?;
            let raw = r.take(k * per)?.to_vec();
            Ok(if per == 4 {
                raw.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
            } else {
                raw.chunks(12).map(|c| dec3n_pack([f32::from_le_bytes([c[0], c[1], c[2], c[3]]), f32::from_le_bytes([c[4], c[5], c[6], c[7]]), f32::from_le_bytes([c[8], c[9], c[10], c[11]])])).collect()
            })
        };
        let mut tu = read_tangents(r)?;
        let mut tv = read_tangents(r)?;
        v.inline_tangents = tu.len() == n && tv.len() == n;
        if tu.len() != n || tv.len() != n {
            // no stored frame: any orthonormal frame around the normal
            tu.clear();
            tv.clear();
            for nw in &nrm {
                let nn = dec3n_unpack(*nw);
                let a = if nn[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
                let u = [a[1] * nn[2] - a[2] * nn[1], a[2] * nn[0] - a[0] * nn[2], a[0] * nn[1] - a[1] * nn[0]];
                let l = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt().max(1e-9);
                let u = [u[0] / l, u[1] / l, u[2] / l];
                let vv = [nn[1] * u[2] - nn[2] * u[1], nn[2] * u[0] - nn[0] * u[2], nn[0] * u[1] - nn[1] * u[0]];
                tu.push(dec3n_pack(u));
                tv.push(dec3n_pack(vv));
            }
        }
        let uv0: Vec<[f32; 2]> = match m.tex_coord_sets.first() {
            Some(t) => t.coords.iter().map(|(uv, _, _)| *uv).collect(),
            None => vec![[0.0, 0.0]; n],
        };
        let uv1: Vec<[f32; 2]> = match m.tex_coord_sets.get(1) {
            Some(t) => t.coords.iter().map(|(uv, _, _)| *uv).collect(),
            None => uv0.clone(),
        };
        let color = has_color && col.len() == n;
        let stride = if color { 11 } else { 10 };
        let d = Decl::with_stride;
        let mut decls = vec![d(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0, stride), d(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC, stride)];
        let mut elems = vec![Elem::Float3(pos), Elem::Word(nrm)];
        let mut off = 0x10;
        if color {
            decls.push(d(N_COLOR0, T_COLOR, SPACE_GLOBAL2D, off, stride));
            elems.push(Elem::Word(col));
            off += 4;
        }
        decls.push(d(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, off, stride));
        elems.push(Elem::Float2(uv0));
        decls.push(d(N_TEXCOORD0 + 1, T_FLOAT2, SPACE_GLOBAL2D, off + 8, stride));
        elems.push(Elem::Float2(uv1));
        decls.push(d(N_TANGENT_U, T_DEC3N, SPACE_LOCAL3D, off + 16, stride));
        elems.push(Elem::Word(tu));
        decls.push(d(N_TANGENT_V, T_DEC3N, SPACE_LOCAL3D, off + 20, stride));
        elems.push(Elem::Word(tv));
        let stream = CPlugVertexStream { version: 1, count: n as i32, flags: 3, base: super::null_ref(), decls, compress_local3d: Some(true), elems };
        m.vertex_streams = vec![NodeRef { index: 0, inline: Some(Box::new(Node::VertexStream(stream))) }];
        v.inline_form = true;
        v.inline_uv_sets = m.tex_coord_sets.len().max(1);
        v.inline_uv_flags = m.tex_coord_sets.first().and_then(|t| t.flags).unwrap_or(256);
        m.tex_coord_sets.clear();
        m.chunk_flags = 0x38;
        v.tangents = Some((Vec::new(), Vec::new()));
        Ok(())
    }

    /// The inline-vertex body of chunk 0x0902C004 (the reverse of
    /// `parse_inline_into_stream`), from the first stream: per vertex the
    /// position, the normal (a Dec3N word when the chunk-flags word says
    /// compressed, else three floats), the colour (a word or four floats) when
    /// the form carries one — opaque white when the stream has none — then
    /// the two tangent arrays. The branch structure mirrors the reader's.
    fn write_inline_vertices(&self, w: &mut Wr) {
        use super::build::dec3n_unpack;
        use super::vstream::*;
        let m = self.main.as_ref().expect("chunk 0x0902C004 needs 0x0900600F");
        let s = self.stream().expect("inline form needs a vertex stream to write from");
        let f = m.chunk_flags;
        let (use_normal, use_color, compress3, compress4, bit22) = (f & (1 << 5) != 0, f & (1 << 6) != 0, f & (1 << 7) != 0, f & (1 << 8) != 0, f & (1 << 9) != 0);
        let has_color = !bit22 || use_color;
        let n = m.count.max(0) as usize;
        let find = |name: u32| s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let pos = match find(N_POSITION) {
            Some(Elem::Float3(p)) => p.clone(),
            _ => panic!("inline form: no float3 positions"),
        };
        let words = |name: u32| -> Vec<u32> {
            match find(name) {
                Some(Elem::Word(v)) => v.clone(),
                Some(Elem::Float3(p)) => p.iter().map(|q| super::build::dec3n_pack(*q)).collect(),
                _ => Vec::new(),
            }
        };
        let nrm = words(N_NORMAL);
        let col = words(N_COLOR0);
        let (tu, tv) = (words(N_TANGENT_U), words(N_TANGENT_V));
        let color_floats = |c: u32| -> [f32; 4] { [(c & 0xFF) as f32 / 255.0, ((c >> 8) & 0xFF) as f32 / 255.0, ((c >> 16) & 0xFF) as f32 / 255.0, ((c >> 24) & 0xFF) as f32 / 255.0] };
        for i in 0..n {
            w.floats(&pos[i]);
            let nw = nrm.get(i).copied().unwrap_or_else(|| super::build::dec3n_pack([0.0, 1.0, 0.0]));
            let cw = col.get(i).copied().unwrap_or(0xFFFF_FFFF);
            if !bit22 && !compress4 && use_color {
                w.floats(&dec3n_unpack(nw));
                w.floats(&color_floats(cw));
            } else {
                if !bit22 || use_normal {
                    if compress3 {
                        w.u32(nw);
                    } else {
                        w.floats(&dec3n_unpack(nw));
                    }
                }
                if has_color {
                    if compress4 {
                        w.u32(cw);
                    } else {
                        w.floats(&color_floats(cw));
                    }
                }
            }
        }
        for t in [&tu, &tv] {
            let k = if self.inline_tangents && t.len() == n { n } else { 0 };
            w.u32(k as u32);
            for x in t.iter().take(k) {
                if compress3 {
                    w.u32(*x);
                } else {
                    w.floats(&dec3n_unpack(*x));
                }
            }
        }
    }

    /// Keep the first `n` vertices of every stream element (a vertex-animated
    /// mesh's first frame).
    pub fn truncate_vertices(&mut self, n: usize) {
        if let Some(m) = self.main.as_mut() {
            if let Some(super::Node::VertexStream(s)) = m.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
                for e in s.elems.iter_mut() {
                    match e {
                        super::vstream::Elem::Float2(v) => v.truncate(n),
                        super::vstream::Elem::Float3(v) => v.truncate(n),
                        super::vstream::Elem::Float4(v) => v.truncate(n),
                        super::vstream::Elem::Word(v) => v.truncate(n),
                        super::vstream::Elem::Raw { size, bytes } => bytes.truncate(n * *size),
                    }
                }
                s.count = n as i32;
            }
            m.count = n as i32;
        }
    }

    fn parse_main(r: &mut Rd) -> R<VisualMain> {
        let version = r.u32()?;
        let chunk_flags = r.u32()?;
        let n_sets = r.count()?;
        let count = r.i32()?;
        let vertex_streams = r.array(read_ref)?;
        let mut m = VisualMain {
            version,
            chunk_flags,
            tex_coord_sets: Vec::new(),
            count,
            vertex_streams,
            skin: None,
            bounding_box: [0.0; 6],
            bitmap_elems: Vec::new(),
            uv_groups: Vec::new(),
            u02: 0,
            u03: 0,
            u04: Vec::new(),
        };
        for _ in 0..n_sets {
            m.tex_coord_sets.push(TexCoordSet::read(r, count.max(0) as usize)?);
        }
        if m.flags() & 7 != 0 {
            m.skin = Some(SkinData::read(r, 2 + version)?);
        }
        m.bounding_box = r.floats::<6>()?;
        m.bitmap_elems = r.array(|r| Ok([r.i32()?, r.i32()?, r.i32()?, r.i32()?, r.i32()?]))?;
        if version >= 5 {
            m.uv_groups = r.array(|r| r.u16())?;
            if version >= 6 {
                m.u02 = r.i32()?;
                m.u03 = r.i32()?;
                if m.u03 > 0 {
                    m.u04 = r.take(m.u03 as usize - 4)?.to_vec();
                }
            }
        }
        Ok(m)
    }
}

/// A float4 colour as the RGBA byte word the stream form stores.
fn pack_color(c: [f32; 4]) -> u32 {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    b(c[0]) | (b(c[1]) << 8) | (b(c[2]) << 16) | (b(c[3]) << 24)
}

impl CPlugVisualIndexedTriangles {
    pub fn write(&self, w: &mut Wr) {
        for cid in &self.chunks {
            w.u32(*cid);
            match *cid {
                0x09006001 => w.id(&self.id),
                0x09006004 => write_ref(w, &self.u_node),
                0x09006005 => {
                    w.u32(self.sub_visuals.len() as u32);
                    for s in &self.sub_visuals {
                        s.iter().for_each(|x| w.i32(*x));
                    }
                }
                0x09006009 => w.f32(self.u_float),
                0x0900600B => {
                    w.u32(self.splits.len() as u32);
                    for (a, b, bx) in &self.splits {
                        w.i32(*a);
                        w.i32(*b);
                        w.floats(bx);
                    }
                }
                0x0900600F => self.write_main(w),
                0x09006010 => {
                    let (v, m) = self.morph.expect("chunk 0x09006010 listed but absent");
                    w.u32(v);
                    w.i32(m);
                }
                0x0902C002 => write_ref(w, &self.v3d_node),
                0x0902C004 => {
                    if self.inline_form {
                        self.write_inline_vertices(w);
                        continue;
                    }
                    let m = self.main.as_ref().expect("chunk 0x0902C004 needs 0x0900600F");
                    let per = (((!(m.flags() >> 17)) & 8) | 4) as usize;
                    let (a, b) = self.tangents.as_ref().expect("chunk 0x0902C004 listed but absent");
                    for t in [a, b] {
                        w.u32((t.len() / per) as u32);
                        w.bytes(t);
                    }
                }
                0x0906A001 => {
                    w.bool32(self.index_buffer.is_some());
                    if let Some(ib) = &self.index_buffer {
                        ib.write(w);
                    }
                }
                c => panic!("CPlugVisual chunk 0x{c:08X} has no writer"),
            }
        }
        w.u32(FACADE);
    }

    fn write_main(&self, w: &mut Wr) {
        let m = self.main.as_ref().expect("chunk 0x0900600F listed but absent");
        w.u32(m.version);
        w.u32(m.chunk_flags);
        if self.inline_form {
            // no stream node: the texcoord sets ride here (version 3, the form
            // TM2020 writes), the vertices in chunk 0x0902C004
            use super::vstream::{Elem, N_TEXCOORD0};
            let s = self.stream().expect("inline form needs a vertex stream to write from");
            let n = m.count.max(0) as usize;
            let uv = |k: u32| -> Vec<[f32; 2]> {
                match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == N_TEXCOORD0 + k).map(|(_, e)| e) {
                    Some(Elem::Float2(v)) => v.clone(),
                    _ => vec![[0.0, 0.0]; n],
                }
            };
            let sets: Vec<TexCoordSet> = (0..self.inline_uv_sets as u32).map(|k| TexCoordSet { version: 3, flags: Some(self.inline_uv_flags), coords: uv(k).into_iter().map(|c| (c, None, None)).collect(), u01: Vec::new() }).collect();
            w.u32(sets.len() as u32);
            w.i32(m.count);
            w.u32(0);
            for t in &sets {
                t.write(w);
            }
        } else {
            w.u32(m.tex_coord_sets.len() as u32);
            w.i32(m.count);
            w.u32(m.vertex_streams.len() as u32);
            for s in &m.vertex_streams {
                write_ref(w, s);
            }
            for t in &m.tex_coord_sets {
                t.write(w);
            }
        }
        if let Some(s) = &m.skin {
            s.write(w, 2 + m.version);
        }
        w.floats(&m.bounding_box);
        w.u32(m.bitmap_elems.len() as u32);
        for b in &m.bitmap_elems {
            b.iter().for_each(|x| w.i32(*x));
        }
        if m.version >= 5 {
            w.u32(m.uv_groups.len() as u32);
            m.uv_groups.iter().for_each(|x| w.u16(*x));
            if m.version >= 6 {
                w.i32(m.u02);
                w.i32(m.u03);
                if m.u03 > 0 {
                    w.bytes(&m.u04);
                }
            }
        }
    }

    /// The inline vertex stream of the first stream slot, if defined here.
    pub fn stream(&self) -> Option<&super::vstream::CPlugVertexStream> {
        let m = self.main.as_ref()?;
        match m.vertex_streams.first()?.inline.as_deref()? {
            super::Node::VertexStream(s) => Some(s),
            _ => None,
        }
    }

    /// Same, mutable.
    pub fn stream_mut(&mut self) -> Option<&mut super::vstream::CPlugVertexStream> {
        let m = self.main.as_mut()?;
        match m.vertex_streams.first_mut()?.inline.as_deref_mut()? {
            super::Node::VertexStream(s) => Some(s),
            _ => None,
        }
    }
}
