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
    let item = parse_body_with(&g.body, &ref_table_nodes(&g.ref_table))?;
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
    parse_body_with(body, &[])
}

/// Same, with the node indices the reference table defines (external files:
/// a reference to one carries no inline body).
pub fn parse_body_with(body: &[u8], externals: &[u32]) -> R<CGameItemModel> {
    let mut lb = LookbackState::default();
    lb.defined_nodes.extend(externals.iter().copied());
    let mut r = Rd::new(body, 0, lb);
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

/// A Gbx reference table naming external files: `(node index, path)`, the
/// paths as the packs spell them (ancestor level 0: relative to the item's own
/// folder, no steps up). Folders are numbered depth-first from 1 (0 = the
/// ancestor directory itself), each external carries flags 1, its file name,
/// node index, use-file 0 and its folder index — the layout the pack's own
/// files carry (`ItemLampSpot.Light.Gbx`: level 1, folders `Light`, `Texture`,
/// one external in folder 2).
pub fn ref_table(externals: &[(u32, String)]) -> Vec<u8> {
    let ancestor_level: u32 = 0;
    let use_file: u32 = 0;
    let mut out = Vec::new();
    out.extend_from_slice(&(externals.len() as u32).to_le_bytes());
    if externals.is_empty() {
        return out;
    }
    out.extend_from_slice(&ancestor_level.to_le_bytes());
    // one flat folder per distinct directory (no nesting: every directory
    // path becomes ONE root folder whose name carries the backslashes — the
    // game joins names with `\`, so `Media\Texture` as one name is the same
    // path as the nested pair)
    let mut dirs: Vec<String> = Vec::new();
    let mut entries: Vec<(u32, String, u32)> = Vec::new();
    for (node, path) in externals {
        let (dir, name) = match path.rfind('\\') {
            Some(i) => (path[..i].to_string(), path[i + 1..].to_string()),
            None => (String::new(), path.clone()),
        };
        let folder = if dir.is_empty() {
            0
        } else {
            match dirs.iter().position(|d| *d == dir) {
                Some(p) => p as u32 + 1,
                None => {
                    dirs.push(dir);
                    dirs.len() as u32
                }
            }
        };
        entries.push((*node, name, folder));
    }
    out.extend_from_slice(&(dirs.len() as u32).to_le_bytes());
    for d in &dirs {
        out.extend_from_slice(&(d.len() as u32).to_le_bytes());
        out.extend_from_slice(d.as_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // no subfolders
    }
    for (node, name, folder) in &entries {
        out.extend_from_slice(&1u32.to_le_bytes()); // flags: a file name follows
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&node.to_le_bytes());
        out.extend_from_slice(&use_file.to_le_bytes()); // use file
        out.extend_from_slice(&folder.to_le_bytes());
    }
    out
}

/// The node indices a raw reference table (as `ref_table` builds it, or as a
/// version-6 file carries it) defines as external files.
pub fn ref_table_nodes(raw: &[u8]) -> Vec<u32> {
    let mut r = crate::reader::Reader::new(raw);
    let mut out = Vec::new();
    let Ok(n) = r.u32() else { return out };
    if n == 0 {
        return out;
    }
    let _ancestor = r.u32();
    fn skip_folders(r: &mut crate::reader::Reader, count: u32) -> Option<()> {
        for _ in 0..count {
            r.string().ok()?;
            let sub = r.u32().ok()?;
            skip_folders(r, sub)?;
        }
        Some(())
    }
    let Ok(roots) = r.u32() else { return out };
    if skip_folders(&mut r, roots).is_none() {
        return out;
    }
    for _ in 0..n {
        let Ok(flags) = r.u32() else { break };
        if flags & 4 == 0 {
            if r.string().is_err() {
                break;
            }
        } else if r.u32().is_err() {
            break;
        }
        let Ok(node) = r.u32() else { break };
        out.push(node);
        let _use = r.u32();
        if flags & 4 == 0 {
            let _folder = r.u32();
        }
    }
    out
}

/// A plain node file (no header chunks): the `.Mesh.Gbx` / `.DynaObject.Gbx`
/// sidecars an item can name from its own archive folder. `body` is the
/// node's chunks as `write_node` emits them (the class id rides in the
/// header), `num_nodes` counts the file's root plus its inline nodes and
/// externals, `externals` = (node index, bare file name) for the reference
/// table (ancestor level 0 = the file's own folder).
pub fn write_node_file(class_id: u32, body: &[u8], num_nodes: u32, externals: &[(u32, String)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 64);
    out.extend_from_slice(b"GBX");
    out.extend_from_slice(&6u16.to_le_bytes());
    out.extend_from_slice(b"BUUR");
    out.extend_from_slice(&class_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // no header chunks
    out.extend_from_slice(&num_nodes.to_le_bytes());
    out.extend_from_slice(&ref_table(externals));
    out.extend_from_slice(body);
    out
}
