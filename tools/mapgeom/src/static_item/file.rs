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
/// paths as the packs spell them. Folders are a NESTED tree numbered
/// depth-first from 1 (0 = the ancestor directory itself), each external
/// carries flags 1, its file name, node index, use-file 0 and its folder index
/// — the layout the pack's own files carry (`FlagSmall.Mesh.Gbx`: ancestor
/// level 4 up to GameData, folders `Stadium` > `Media` > `Material`, two
/// externals in the last one).
///
/// The ancestor level (how many directories the resolver climbs from the
/// file's own folder before descending the tree) is 0 by default — a sidecar
/// next to the item — and `TINY_REF_ANCESTOR=N` sets it. What it reaches from
/// an embedded item (2026-09-09, `/fids`): the item lives at
/// `<fake>\MemoryTemp\CurrentMap_EmbeddedFiles\ContentLoaded\Items\`, so level 1
/// is `ContentLoaded` — a `Stadium\Media\Material\X.Material.Gbx` carried in the
/// map archive at that path is one level up and three folders down.
pub fn ref_table(externals: &[(u32, String)]) -> Vec<u8> {
    let ancestor_level: u32 = std::env::var("TINY_REF_ANCESTOR").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let use_file: u32 = 0;
    let mut out = Vec::new();
    out.extend_from_slice(&(externals.len() as u32).to_le_bytes());
    if externals.is_empty() {
        return out;
    }
    out.extend_from_slice(&ancestor_level.to_le_bytes());
    #[derive(Default)]
    struct Folder {
        name: String,
        subs: Vec<Folder>,
    }
    fn insert(f: &mut Folder, parts: &[&str]) {
        if parts.is_empty() {
            return;
        }
        let pos = match f.subs.iter().position(|s| s.name == parts[0]) {
            Some(p) => p,
            None => {
                f.subs.push(Folder { name: parts[0].to_string(), subs: Vec::new() });
                f.subs.len() - 1
            }
        };
        insert(&mut f.subs[pos], &parts[1..]);
    }
    fn number(f: &Folder, prefix: &str, next: &mut u32, out: &mut std::collections::BTreeMap<String, u32>) {
        for sub in &f.subs {
            let path = if prefix.is_empty() { sub.name.clone() } else { format!("{prefix}\\{}", sub.name) };
            out.insert(path.clone(), *next);
            *next += 1;
            number(sub, &path, next, out);
        }
    }
    fn write(f: &Folder, v: &mut Vec<u8>) {
        v.extend_from_slice(&(f.subs.len() as u32).to_le_bytes());
        for sub in &f.subs {
            v.extend_from_slice(&(sub.name.len() as u32).to_le_bytes());
            v.extend_from_slice(sub.name.as_bytes());
            write(sub, v);
        }
    }
    let mut root = Folder::default();
    let mut entries: Vec<(u32, String, String)> = Vec::new();
    for (node, path) in externals {
        let (dir, name) = match path.rfind('\\') {
            Some(i) => (path[..i].to_string(), path[i + 1..].to_string()),
            None => (String::new(), path.clone()),
        };
        if !dir.is_empty() {
            insert(&mut root, &dir.split('\\').collect::<Vec<_>>());
        }
        entries.push((*node, dir, name));
    }
    let mut index = std::collections::BTreeMap::new();
    let mut next = 1u32;
    number(&root, "", &mut next, &mut index);
    write(&root, &mut out);
    for (node, dir, name) in &entries {
        out.extend_from_slice(&1u32.to_le_bytes()); // flags: a file name follows
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&node.to_le_bytes());
        out.extend_from_slice(&use_file.to_le_bytes()); // use file
        let folder = if dir.is_empty() { 0 } else { index[dir] };
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

/// The bounding boxes of a parsed item's geometry: every solid visual's `bounding_box` (lo xyz, hi xyz) plus the
/// collision / trigger meshes' vertex extents — for the catalogs' `Size` (csgen).
pub fn item_bboxes(item: &CGameItemModel) -> Vec<[f32; 6]> {
    let mut out = Vec::new();
    if let Some(s2) = item.static_object().and_then(|so| so.solid2()) {
        for r in &s2.visuals {
            if let Some(super::Node::Visual(v)) = r.inline.as_deref() {
                // the visual's box is centre + half extents
                if let Some(main) = v.main.as_ref() { let b = main.bounding_box; out.push([b[0] - b[3], b[1] - b[4], b[2] - b[5], b[0] + b[3], b[1] + b[4], b[2] + b[5]]); }
            }
        }
    }
    let mut push_surf = |sf: &super::surface::CPlugSurface| {
        if let super::surface::Surf::Mesh { vertices, .. } = &sf.surf {
            let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
            for v in vertices { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
            if lo[0].is_finite() { out.push([lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]]); }
        }
    };
    if let Some(sf) = item.static_object().and_then(|so| so.surface()) { push_surf(sf); }
    // the waypoint trigger shape of the entity model (our carriers' finish volume)
    if let Some(em) = item.model().and_then(|m| m.entity_model()) {
        if let Some(super::Node::Surface(sf)) = em.trigger_shape.inline.as_deref() { push_surf(sf); }
    }
    out
}
