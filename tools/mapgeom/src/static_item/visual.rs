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
                        return Err("CPlugVisual3D with inline vertices (no vertex stream) is not supported".into());
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
        w.u32(m.tex_coord_sets.len() as u32);
        w.i32(m.count);
        w.u32(m.vertex_streams.len() as u32);
        for s in &m.vertex_streams {
            write_ref(w, s);
        }
        for t in &m.tex_coord_sets {
            t.write(w);
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
}
