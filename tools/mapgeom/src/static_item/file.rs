//! The `.Item.Gbx` container around a `CGameItemModel` body: the header
//! (with its header chunks), node count, reference table and body. The
//! reference items are written uncompressed (`'U'`), so a parse -> write
//! round trip can be byte-identical; a compressed (`'C'`) body is inflated
//! with the LZO from `tmmaps` and written back through it.

use super::{CGameItemModel, LookbackState, Rd, Wr, R};

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderChunk {
    pub id: u32,
    pub heavy: bool,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StaticItemFile {
    pub version: u16,
    pub format: u8,
    pub ref_comp: u8,
    pub body_comp: u8,
    pub unknown: Option<u8>,
    pub class_id: u32,
    pub header_chunks: Vec<HeaderChunk>,
    pub num_nodes: u32,
    /// The reference table bytes verbatim (`0` for none).
    pub ref_table: Vec<u8>,
    pub item: CGameItemModel,
}

fn u32_at(b: &[u8], o: usize) -> R<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| format!("header truncated at 0x{o:x}"))
}

/// Split the user-data block into header chunks.
pub fn parse_header_chunks(user_data: &[u8]) -> R<Vec<HeaderChunk>> {
    if user_data.is_empty() {
        return Ok(Vec::new());
    }
    let n = u32_at(user_data, 0)? as usize;
    let mut out = Vec::with_capacity(n);
    let mut o = 4;
    for _ in 0..n {
        let id = u32_at(user_data, o)?;
        let sz = u32_at(user_data, o + 4)?;
        out.push(HeaderChunk { id, heavy: sz & 0x8000_0000 != 0, payload: vec![0; (sz & 0x7FFF_FFFF) as usize] });
        o += 8;
    }
    for c in &mut out {
        let n = c.payload.len();
        c.payload = user_data.get(o..o + n).ok_or("header chunk payload truncated")?.to_vec();
        o += n;
    }
    if o != user_data.len() {
        return Err(format!("header chunks use {o} of {} user-data bytes", user_data.len()));
    }
    Ok(out)
}

pub fn write_header_chunks(chunks: &[HeaderChunk]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    for c in chunks {
        out.extend_from_slice(&c.id.to_le_bytes());
        out.extend_from_slice(&((c.payload.len() as u32) | if c.heavy { 0x8000_0000 } else { 0 }).to_le_bytes());
    }
    for c in chunks {
        out.extend_from_slice(&c.payload);
    }
    out
}

/// Parse a whole `.Item.Gbx`.
pub fn parse_file(data: &[u8]) -> R<StaticItemFile> {
    if data.len() < 12 || &data[0..3] != b"GBX" {
        return Err("not a GBX file".into());
    }
    let g = tmmaps::gbx::Gbx::parse(data);
    let header_chunks = parse_header_chunks(&g.user_data)?;
    let item = parse_body(&g.body)?;
    Ok(StaticItemFile {
        version: g.version,
        format: g.format,
        ref_comp: g.ref_comp,
        body_comp: if g.comp.is_some() { b'C' } else { b'U' },
        unknown: g.unknown,
        class_id: g.class_id,
        header_chunks,
        num_nodes: g.num_nodes,
        ref_table: g.ref_table,
        item,
    })
}

/// Parse an item body (the decompressed bytes after the reference table).
pub fn parse_body(body: &[u8]) -> R<CGameItemModel> {
    let mut r = Rd::new(body, 0, LookbackState::default());
    let item = CGameItemModel::parse(&mut r)?;
    if r.o != body.len() {
        return Err(format!("item body: {} trailing bytes after FACADE at 0x{:x}", body.len() - r.o, r.o));
    }
    Ok(item)
}

pub fn write_body(item: &CGameItemModel) -> Vec<u8> {
    let mut out = Vec::new();
    let mut lb = LookbackState::default();
    let mut w = Wr { w: &mut out, lb: &mut lb };
    item.write(&mut w);
    out
}

/// Serialise a whole file.
pub fn write_file(f: &StaticItemFile) -> Vec<u8> {
    let body = write_body(&f.item);
    let mut out = Vec::with_capacity(body.len() + 512);
    out.extend_from_slice(b"GBX");
    out.extend_from_slice(&f.version.to_le_bytes());
    out.push(f.format);
    out.push(f.ref_comp);
    out.push(f.body_comp);
    if let Some(u) = f.unknown {
        out.push(u);
    }
    out.extend_from_slice(&f.class_id.to_le_bytes());
    if f.version >= 6 {
        let ud = write_header_chunks(&f.header_chunks);
        out.extend_from_slice(&(ud.len() as u32).to_le_bytes());
        out.extend_from_slice(&ud);
    }
    out.extend_from_slice(&f.num_nodes.to_le_bytes());
    out.extend_from_slice(&f.ref_table);
    if f.body_comp == b'C' {
        let stream = tmmaps::gbx::lzo_compress(&body);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
        out.extend_from_slice(&stream);
    } else {
        out.extend_from_slice(&body);
    }
    out
}
