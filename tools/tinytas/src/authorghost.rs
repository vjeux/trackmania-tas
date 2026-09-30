//! The author-validation ghost embedded in a `.Map.Gbx` — reading it out
//! (the proof) and writing ours in (the deliverable, decision 1).
//!
//! Format facts, all measured (see `ACQUISITION_addendum_embedded_author_ghost.md`
//! in the unbeaten-AT bank, 2026-08-18, and this module's probes):
//!
//! * The map's `CGameCtnChallengeParameters` node carries skippable chunk
//!   `0x0305B00F` = `raceValidateGhost`, a node reference: `u32 nodeIndex |
//!   u32 classId(0x03092000) | <CGameCtnGhost chunk stream> | 0xFACADE01`.
//! * A ghost body is NOT all skippable chunks: `0x0309200C/0E/0F/10/1C` carry
//!   no `PIKS` and no length; the walker finds the next ASCENDING plausible
//!   chunk id, and every walk is verified by reassembling the input.
//! * Two four-byte differences separate the embedded blob from a standalone
//!   `.Ghost.Gbx`: (1) the `CPlugEntRecordData` reference inside `0x03092000`
//!   has NO node-index word in the blob (`u32 classId` directly) and has one
//!   (`u32 1 | u32 classId`) in a standalone file; (2) `0x03092010` holds the
//!   map uid AS OF THE VALIDATION SAVE, and the file we hold may carry a newer
//!   uid. Extraction adds (1) and rewrites (2); embedding does the reverse.
//! * The ghost's Id (lookback) strings share the MAP's Id table, which the
//!   blocks chunk downstream indexes by position. An embed must therefore
//!   define the same number of Id literals in the same order as the blob it
//!   replaces — content may change, count and order may not.

use tmmaps::gbx::Gbx;
#[allow(unused_imports)]
use gbx as gbxcrate;

#[derive(Clone, Debug)]
pub struct Chunk {
    pub id: u32,
    /// whole chunk: id + (PIKS + size) + payload
    pub bytes: Vec<u8>,
}

pub struct Body {
    pub chunks: Vec<Chunk>,
    /// whatever follows the last chunk -- normally the 0xFACADE01 terminator
    pub suffix: Vec<u8>,
}

pub const CLASS_GHOST: u32 = 0x0309_2000;
pub const CLASS_ENTRECORD: u32 = 0x0911_F000;
pub const FACADE: u32 = 0xFACA_DE01;
pub const CHUNK_RACE_VALIDATE_GHOST: u32 = 0x0305_B00F;

fn u32at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}

fn plausible_id(v: u32) -> bool {
    matches!(v & 0xFFFF_F000, 0x0303_F000 | 0x0309_2000 | 0x0911_F000)
}

pub fn is_skip(c: &Chunk) -> bool {
    c.bytes.len() >= 8 && &c.bytes[4..8] == b"PIKS"
}
pub fn payload_off(c: &Chunk) -> usize {
    if is_skip(c) {
        12
    } else {
        4
    }
}
pub fn payload(c: &Chunk) -> &[u8] {
    &c.bytes[payload_off(c)..]
}

/// Set a chunk's payload, fixing the PIKS size word when there is one.
pub fn set_payload(c: &mut Chunk, p: &[u8]) {
    let po = payload_off(c);
    let mut nb = c.bytes[..po].to_vec();
    nb.extend_from_slice(p);
    if po == 12 {
        nb[8..12].copy_from_slice(&(p.len() as u32).to_le_bytes());
    }
    c.bytes = nb;
}

fn split_body_unchecked(b: &[u8]) -> Body {
    let n = b.len();
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut i = 0usize;
    let mut last_id = 0u32;
    while i + 4 <= n {
        let id = u32at(b, i);
        if id == FACADE || !plausible_id(id) || id <= last_id {
            break;
        }
        if i + 12 <= n && &b[i + 4..i + 8] == b"PIKS" {
            let size = u32at(b, i + 8) as usize;
            if i + 12 + size > n {
                break;
            }
            chunks.push(Chunk {
                id,
                bytes: b[i..i + 12 + size].to_vec(),
            });
            i += 12 + size;
        } else {
            let mut j = i + 4;
            while j + 4 <= n {
                let v = u32at(b, j);
                if v == FACADE || (plausible_id(v) && v > id) {
                    break;
                }
                j += 1;
            }
            chunks.push(Chunk {
                id,
                bytes: b[i..j].to_vec(),
            });
            i = j;
        }
        last_id = id;
    }
    Body {
        chunks,
        suffix: b[i..].to_vec(),
    }
}

/// Walk a ghost chunk stream; refuses unless the walk reassembles its input.
pub fn split_body(b: &[u8]) -> Result<Body, String> {
    let bd = split_body_unchecked(b);
    if bd.assemble() != b {
        return Err("chunk walk did not round-trip".into());
    }
    Ok(bd)
}

impl Body {
    pub fn assemble(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for c in &self.chunks {
            out.extend_from_slice(&c.bytes);
        }
        out.extend_from_slice(&self.suffix);
        out
    }
    pub fn get(&self, id: u32) -> Option<&Chunk> {
        self.chunks.iter().find(|c| c.id == id)
    }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Chunk> {
        self.chunks.iter_mut().find(|c| c.id == id)
    }
}

/// Every lookback ("Id") string literal in a byte range, found by the
/// 0x40000000 flag word followed by a plausible length + printable bytes.
/// Returns (offset of the flag word, the string).
pub fn id_literals(p: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= p.len() {
        let f = u32at(p, i);
        if f == 0x4000_0000 {
            let n = u32at(p, i + 4) as usize;
            if n > 0
                && n < 64
                && i + 8 + n <= p.len()
                && p[i + 8..i + 8 + n].iter().all(|&c| (0x20..0x7f).contains(&c))
            {
                out.push((i, String::from_utf8_lossy(&p[i + 8..i + 8 + n]).to_string()));
                i += 8 + n;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Every Id WORD in a payload (literal flag words AND back-references), so two
/// blobs can be compared for Id-table effect. A back-reference is a u32 whose
/// top two bits are 0b00/0b10 with a small index... which is indistinguishable
/// from data, so only literals are counted: a literal DEFINES a table slot,
/// a reference does not. Slot count and order are what the map downstream
/// depends on.
pub fn id_literal_count(p: &[u8]) -> usize {
    id_literals(p).len()
}

/// The embedded author-validation ghost: the byte range of the chunk stream
/// (after the class id) up to and including the 0xFACADE01 terminator, plus
/// the offset of the node-index word that precedes the class id.
#[derive(Clone, Copy, Debug)]
pub struct EmbeddedGhost {
    /// body offset of the `u32 nodeIndex` word
    pub index_off: usize,
    pub node_index: u32,
    /// [start of the chunk stream, end past 0xFACADE01)
    pub stream: (usize, usize),
    /// the enclosing skippable chunk 0x0305B00F: (payload start, payload end)
    pub chunk_payload: Option<(usize, usize)>,
}

pub fn find_embedded_ghost(body: &[u8]) -> Vec<EmbeddedGhost> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= body.len() {
        if u32at(body, i) == CLASS_GHOST && u32at(body, i + 4) == 0x0303_F006 {
            let bd = split_body_unchecked(&body[i + 4..]);
            let used: usize = bd.chunks.iter().map(|c| c.bytes.len()).sum();
            let end = i + 4 + used;
            if end + 4 <= body.len() && u32at(body, end) == FACADE && i >= 4 {
                // the enclosing skippable chunk, if the class id sits right
                // after `0x0305B00F PIKS size nodeIndex`
                // layout: id | PIKS | size | u32 0 | u32 len | classId(i) | stream
                let chunk_payload = if i >= 20
                    && u32at(body, i - 20) == CHUNK_RACE_VALIDATE_GHOST
                    && &body[i - 16..i - 12] == b"PIKS"
                {
                    let sz = u32at(body, i - 12) as usize;
                    Some((i - 8, i - 8 + sz))
                } else {
                    None
                };
                out.push(EmbeddedGhost {
                    index_off: i - 4,
                    node_index: u32at(body, i - 4),
                    stream: (i + 4, end + 4),
                    chunk_payload,
                });
            }
        }
        i += 1;
    }
    out
}

/// The CPlugEntRecordData node in a byte range: `u32 classId(0x0911F000) |
/// u32 chunkId(0x0911F000) | u32 version | u32 uncompressedSize |
/// u32 compressedSize | data`. Returns (start of the class id, end past the
/// data) -- the 0xFACADE01 that closes the node follows.
pub fn find_record_node(b: &[u8]) -> Option<(usize, usize)> {
    for i in 0..b.len().saturating_sub(24) {
        if u32at(b, i) == CLASS_ENTRECORD && u32at(b, i + 4) == CLASS_ENTRECORD {
            let csize = u32at(b, i + 16) as usize;
            let end = i + 20 + csize;
            if end + 4 <= b.len() {
                return Some((i, end));
            }
        }
    }
    None
}

pub struct Probe {
    pub uid: Option<String>,
    pub ghosts: Vec<EmbeddedGhost>,
    pub report: String,
}

/// What a map embeds, as text: the 0x0305B00F chunk, the ghost's chunk list
/// with sizes, its Id literals, its record node, its declared times.
pub fn probe(map: &Gbx) -> Probe {
    let uid = gbx::map_uid_of(&map_header_bytes(map));
    let ghosts = find_embedded_ghost(&map.body);
    let mut r = String::new();
    r.push_str(&format!("body {} bytes, header uid {:?}, num_nodes {}\n", map.body.len(), uid, map.num_nodes));
    for (k, g) in ghosts.iter().enumerate() {
        r.push_str(&format!(
            "embedded ghost {k}: node index {} at body {}, stream [{}, {}) = {} bytes, chunk 0x0305B00F payload {:?}\n",
            g.node_index, g.index_off, g.stream.0, g.stream.1, g.stream.1 - g.stream.0, g.chunk_payload
        ));
        let bd = split_body_unchecked(&map.body[g.stream.0..g.stream.1 - 4]);
        for c in &bd.chunks {
            let ids = id_literals(payload(c));
            r.push_str(&format!(
                "  0x{:08X} {:>5} bytes {}  ids {:?}\n",
                c.id,
                payload(c).len(),
                if is_skip(c) { "skip" } else { "    " },
                ids.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>()
            ));
            if c.id == CLASS_GHOST {
                if let Some((a, b)) = find_record_node(payload(c)) {
                    let p = payload(c);
                    r.push_str(&format!(
                        "      record node at payload [{a}, {b}) version {} uncompressed {} compressed {}; word before class id = 0x{:08X}\n",
                        u32at(p, a + 8),
                        u32at(p, a + 12),
                        u32at(p, a + 16),
                        if a >= 4 { u32at(p, a - 4) } else { 0 }
                    ));
                }
            }
            if c.id == 0x0309_2005 {
                r.push_str(&format!("      declared race time {} ms\n", u32at(payload(c), 0)));
            }
        }
        r.push_str(&format!("  suffix {:02X?}\n", &bd.suffix[..bd.suffix.len().min(8)]));
    }
    Probe { uid, ghosts, report: r }
}

fn map_header_bytes(g: &Gbx) -> Vec<u8> {
    // gbx::map_uid_of wants the whole file's leading bytes; the header chunks
    // live in user_data. Rebuild just enough of a file for it.
    let mut out = Vec::new();
    out.extend_from_slice(b"GBX");
    out.extend_from_slice(&g.version.to_le_bytes());
    out.push(g.format);
    out.push(g.ref_comp);
    out.push(b'U');
    if let Some(u) = g.unknown {
        out.push(u);
    }
    out.extend_from_slice(&g.class_id.to_le_bytes());
    out.extend_from_slice(&(g.user_data.len() as u32).to_le_bytes());
    out.extend_from_slice(&g.user_data);
    out
}

/// `ct mapghost`, ported: the embedded ghost as a standalone `.Ghost.Gbx`
/// (node-index word inserted before the record-data class id, map uid in
/// 0x03092010 rewritten to `uid`, synthesised header).
pub fn extract(map: &Gbx, g: &EmbeddedGhost, uid: Option<&str>) -> Result<(Vec<u8>, String), String> {
    let mut log = String::new();
    let mut bd = split_body(&map.body[g.stream.0..g.stream.1])?;
    let noderef: u32 = 1;
    let mut fixed = false;
    for c in bd.chunks.iter_mut() {
        let po = payload_off(c);
        if let Some(at) = c.bytes[po..]
            .windows(4)
            .position(|w| u32::from_le_bytes(w.try_into().unwrap()) == CLASS_ENTRECORD)
        {
            let abs = po + at;
            let mut nb = c.bytes[..abs].to_vec();
            nb.extend_from_slice(&noderef.to_le_bytes());
            nb.extend_from_slice(&c.bytes[abs..]);
            if is_skip(c) {
                let sz = nb.len() - 12;
                nb[8..12].copy_from_slice(&(sz as u32).to_le_bytes());
            }
            c.bytes = nb;
            log.push_str(&format!("node index {} inserted before CPlugEntRecordData in 0x{:08X}\n", noderef, c.id));
            fixed = true;
            break;
        }
    }
    if !fixed {
        log.push_str("WARNING: no CPlugEntRecordData node found -- nothing to fix\n");
    }
    if let (Some(uid), Some(c)) = (uid, bd.get_mut(0x0309_2010)) {
        let po = payload_off(c);
        if let Some((at, olds)) = id_literals(&c.bytes[po..]).first().cloned() {
            log.push_str(&format!("map uid {:?} -> {:?}\n", olds, uid));
            let mut nb = c.bytes[..po + at + 4].to_vec();
            nb.extend_from_slice(&(uid.len() as u32).to_le_bytes());
            nb.extend_from_slice(uid.as_bytes());
            nb.extend_from_slice(&c.bytes[po + at + 8 + olds.len()..]);
            if is_skip(c) {
                let sz = nb.len() - 12;
                nb[8..12].copy_from_slice(&(sz as u32).to_le_bytes());
            }
            c.bytes = nb;
        }
    }
    // A synthesised .Ghost.Gbx header: the ghost class, no header chunks, an
    // empty reference table, two nodes. Byte-identical to a Nadeo ghost's.
    let mut out = Vec::new();
    out.extend_from_slice(b"GBX");
    out.extend_from_slice(&6u16.to_le_bytes());
    out.push(b'B');
    out.push(b'U');
    out.push(b'U');
    out.push(b'R');
    out.extend_from_slice(&CLASS_GHOST.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // user data length
    out.extend_from_slice(&(noderef + 1).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // ref table: none
    // No Id-version word is inserted: `ct mapghost` (2026-08-18) produced
    // files that validated to the millisecond without one, on two maps.
    let body = bd.assemble();
    out.extend_from_slice(&body);
    Ok((out, log))
}


// ---------------------------------------------------------------- embedding

/// Medal times from the author time, Nadeo's way: gold = AT x 1.08, silver =
/// AT x 1.20, bronze = AT x 1.50, each rounded UP to the whole second.
/// Calibrated on Summer 2026 - 01 (AT 23.144 -> 25.000 / 28.000 / 35.000).
pub fn medals(author_ms: u32) -> (u32, u32, u32) {
    let up = |x: f64| ((x / 1000.0).ceil() as u32) * 1000;
    (
        up(author_ms as f64 * 1.08),
        up(author_ms as f64 * 1.20),
        up(author_ms as f64 * 1.50),
    )
}

fn replace_all_u32x4(buf: &mut [u8], old: [u32; 4], new: [u32; 4]) -> usize {
    let mut pat = Vec::new();
    for v in old {
        pat.extend_from_slice(&v.to_le_bytes());
    }
    let mut rep = Vec::new();
    for v in new {
        rep.extend_from_slice(&v.to_le_bytes());
    }
    let mut hits = 0;
    let mut i = 0;
    while i + 16 <= buf.len() {
        if &buf[i..i + 16] == &pat[..] {
            buf[i..i + 16].copy_from_slice(&rep);
            hits += 1;
            i += 16;
        } else {
            i += 1;
        }
    }
    hits
}

fn set_xml_attr(xml: &str, tag: &str, name: &str, value: &str) -> Result<String, String> {
    let tstart = xml.find(&format!("<{tag} ")).ok_or_else(|| format!("no <{tag}> in header xml"))?;
    let tend = tstart + xml[tstart..].find('>').ok_or("unterminated tag")?;
    let seg = &xml[tstart..tend];
    let key = format!("{name}=\"");
    let a = seg.find(&key).ok_or_else(|| format!("no {name}= in <{tag}>"))? + key.len();
    let b = a + seg[a..].find('"').ok_or("unterminated attribute")?;
    Ok(format!("{}{}{}{}", &xml[..tstart], &seg[..a], value, &xml[tstart + b..]))
}

pub struct Embedded {
    pub bytes: Vec<u8>,
    pub log: String,
    pub author_ms: u32,
    pub medals: (u32, u32, u32),
}

/// Build the validated map: `ghost` (a standalone `.Ghost.Gbx` this project
/// wrote, already declared from the oracle) becomes the map's author ghost,
/// AT = its declared millisecond, medals from it, in every copy of the times.
///
/// The blob keeps the map's existing embedded ghost as its SKELETON: chunk by
/// chunk in the original order, a chunk is taken from ours where ours has it
/// (the tape `0x0309201D`, the record `0x03092000`, the times, the uid, the
/// validation block) and kept from the skeleton otherwise, with the skeleton's
/// declared-time chunks (`0x0309201B`) rewritten. The Id literal sequence must
/// come out identical in count and order — the map's Id table depends on it —
/// and the function refuses otherwise.
pub fn embed(map: &Gbx, ghost_file: &[u8], login: Option<&str>) -> Result<Embedded, String> {
    let mut log = String::new();
    let p = probe(map);
    let skel = *p.ghosts.first().ok_or("the map embeds no author ghost to use as a skeleton")?;
    let map_uid = p.uid.clone().ok_or("map header has no uid")?;
    let (pay_a, pay_b) = skel.chunk_payload.ok_or("the embedded ghost is not inside a 0x0305B00F chunk")?;
    let skel_bd = split_body(&map.body[skel.stream.0..skel.stream.1])?;

    let g = Gbx::parse(ghost_file);
    if g.class_id != CLASS_GHOST {
        return Err(format!("ghost file class 0x{:08X} is not CGameCtnGhost", g.class_id));
    }
    let ours = split_body(&g.body)?;
    if u32at(&ours.suffix, 0) != FACADE {
        return Err("our ghost body does not end in 0xFACADE01".into());
    }
    let declared = ours
        .get(0x0309_2005)
        .map(|c| u32at(payload(c), 0))
        .ok_or("our ghost declares no time (0x03092005)")?;
    let splits: Vec<u32> = ours
        .get(0x0309_202B)
        .map(|c| {
            let p = payload(c);
            let n = u32at(p, 20) as usize;
            (0..n).filter(|i| 24 + i * 8 + 4 <= p.len()).map(|i| u32at(p, 24 + i * 8)).collect()
        })
        .unwrap_or_default();
    log.push_str(&format!("our ghost declares {} ms, splits {:?}\n", declared, splits));

    // The new stream, in the skeleton's chunk order.
    let mut chunks: Vec<Chunk> = Vec::new();
    for sc in &skel_bd.chunks {
        match ours.get(sc.id) {
            Some(oc) => {
                let mut c = oc.clone();
                if c.id == CLASS_GHOST {
                    // Reverse defect 1: the blob carries NO node-index word
                    // before the CPlugEntRecordData class id.
                    let po = payload_off(&c);
                    let (a, _) = find_record_node(payload(&c)).ok_or("our 0x03092000 has no record node")?;
                    let idx_at = po + a - 4;
                    let idx = u32at(&c.bytes, idx_at);
                    if idx != 1 && idx != 2 {
                        return Err(format!("expected a node-index word (1|2) before the record class id, found 0x{idx:08X}"));
                    }
                    let mut nb = c.bytes[..idx_at].to_vec();
                    nb.extend_from_slice(&c.bytes[idx_at + 4..]);
                    let newp = nb[po..].to_vec();
                    set_payload(&mut c, &newp);
                    log.push_str(&format!("0x03092000: dropped the record node-index word ({idx})\n"));
                }
                if c.id == 0x0309_2010 {
                    let po = payload_off(&c);
                    if let Some((at, olds)) = id_literals(&c.bytes[po..]).first().cloned() {
                        if olds != map_uid {
                            return Err(format!(
                                "our ghost declares map uid {olds:?} but the map is {map_uid:?}; declare it for THIS map first"
                            ));
                        }
                        let _ = at;
                    }
                }
                if c.id == 0x0309_200F {
                    if let Some(l) = login {
                        let mut np = (l.len() as u32).to_le_bytes().to_vec();
                        np.extend_from_slice(l.as_bytes());
                        set_payload(&mut c, &np);
                    }
                }
                log.push_str(&format!("0x{:08X}: ours ({} bytes; skeleton had {})\n", c.id, payload(&c).len(), payload(sc).len()));
                chunks.push(c);
            }
            None => {
                let mut c = sc.clone();
                if c.id == 0x0309_201B {
                    // the skeleton's summary: time + per-checkpoint deltas
                    let po = payload_off(&c);
                    let p = &mut c.bytes[po..];
                    let ncp = u32at(p, 4) as usize;
                    p[10..14].copy_from_slice(&declared.to_le_bytes());
                    let mut marks = splits.clone();
                    if marks.last() != Some(&declared) {
                        marks.push(declared);
                    }
                    let mut prev = 0u32;
                    for (i, m) in marks.iter().enumerate() {
                        if i >= ncp {
                            break;
                        }
                        let off = 14 + i * 4;
                        if off + 4 <= p.len() {
                            p[off..off + 4].copy_from_slice(&(m - prev).to_le_bytes());
                        }
                        prev = *m;
                    }
                    log.push_str(&format!("0x0309201B: skeleton, redeclared to {} ms\n", declared));
                } else {
                    log.push_str(&format!("0x{:08X}: skeleton ({} bytes)\n", c.id, payload(&c).len()));
                }
                chunks.push(c);
            }
        }
    }
    for oc in &ours.chunks {
        if skel_bd.get(oc.id).is_none() {
            return Err(format!(
                "our ghost has chunk 0x{:08X} which the skeleton lacks; refusing to guess its place",
                oc.id
            ));
        }
    }
    let new_bd = Body { chunks, suffix: FACADE.to_le_bytes().to_vec() };
    let new_stream = new_bd.assemble();

    // THE ID-TABLE INVARIANT: same literal count, same order, same content
    // except the uid slot.
    let old_lits: Vec<String> = id_literals(&map.body[skel.stream.0..skel.stream.1]).into_iter().map(|(_, s)| s).collect();
    let new_lits: Vec<String> = id_literals(&new_stream).into_iter().map(|(_, s)| s).collect();
    if old_lits.len() != new_lits.len() {
        return Err(format!("Id literal count changed {:?} -> {:?}; the map's Id table would shift", old_lits, new_lits));
    }
    for (o, n) in old_lits.iter().zip(&new_lits) {
        if o != n && n != &map_uid {
            return Err(format!("Id literal {o:?} -> {n:?}: only the map-uid slot may change"));
        }
    }
    log.push_str(&format!("Id literals {:?} -> {:?}\n", old_lits, new_lits));

    // Payload of 0x0305B00F: u32 0 | u32 len(classId..FACADE) | classId | stream
    let mut payload_new = Vec::new();
    payload_new.extend_from_slice(&u32at(&map.body, pay_a).to_le_bytes());
    payload_new.extend_from_slice(&((4 + new_stream.len()) as u32).to_le_bytes());
    payload_new.extend_from_slice(&CLASS_GHOST.to_le_bytes());
    payload_new.extend_from_slice(&new_stream);
    let old_len_word = u32at(&map.body, pay_a + 4) as usize;
    if old_len_word != pay_b - pay_a - 8 {
        return Err(format!("0x0305B00F length word {} does not span its payload ({})", old_len_word, pay_b - pay_a - 8));
    }

    let mut body = Vec::with_capacity(map.body.len() + payload_new.len());
    body.extend_from_slice(&map.body[..pay_a - 4]);
    body.extend_from_slice(&(payload_new.len() as u32).to_le_bytes());
    body.extend_from_slice(&payload_new);
    body.extend_from_slice(&map.body[pay_b..]);
    log.push_str(&format!("0x0305B00F payload {} -> {} bytes; body {} -> {} bytes\n", pay_b - pay_a, payload_new.len(), map.body.len(), body.len()));

    // Times: every copy. Header chunk 0x03043002 (binary), header XML
    // 0x03043005, body 0x0305B004 and 0x0305B00A, all of which hold the
    // sequence bronze|silver|gold|author.
    let hchunks = gbx::header::parse_user_data(&map.user_data).ok_or("header chunk table unreadable")?;
    let xml_old = {
        let c = hchunks.iter().find(|c| c.id == 0x0304_3005).ok_or("no header xml")?;
        let n = u32at(&c.data, 0) as usize;
        String::from_utf8_lossy(&c.data[4..4 + n]).to_string()
    };
    let attr = |name: &str| -> Result<u32, String> {
        tmmaps::header::attr_pub(&xml_old, "times", name)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| format!("header xml has no times {name}"))
    };
    let old = [attr("bronze")?, attr("silver")?, attr("gold")?, attr("authortime")?];
    let (gold, silver, bronze) = medals(declared);
    let new = [bronze, silver, gold, declared];
    let mut user_data_chunks = hchunks.clone();
    let mut header_hits = 0;
    for c in user_data_chunks.iter_mut() {
        if c.id == 0x0304_3002 {
            header_hits += replace_all_u32x4(&mut c.data, old, new);
        }
        if c.id == 0x0304_3005 {
            let mut xml = xml_old.clone();
            for (k, v) in [("bronze", bronze), ("silver", silver), ("gold", gold), ("authortime", declared)] {
                xml = set_xml_attr(&xml, "times", k, &v.to_string())?;
            }
            xml = set_xml_attr(&xml, "desc", "validated", "1")?;
            let mut d = (xml.len() as u32).to_le_bytes().to_vec();
            d.extend_from_slice(xml.as_bytes());
            c.data = d;
        }
    }
    if header_hits != 1 {
        return Err(format!("header chunk 0x03043002: expected one times quadruple {:?}, replaced {}", old, header_hits));
    }
    let head_len = pay_a.min(body.len());
    let body_hits = replace_all_u32x4(&mut body[..head_len], old, new);
    if body_hits < 2 {
        return Err(format!("body challenge parameters: expected >=2 times quadruples {:?} before the ghost, replaced {}", old, body_hits));
    }
    log.push_str(&format!(
        "times {:?} -> bronze {} silver {} gold {} author {} (header x{}, body x{})\n",
        old, bronze, silver, gold, declared, header_hits, body_hits
    ));

    let out_gbx = Gbx {
        version: map.version,
        format: map.format,
        ref_comp: map.ref_comp,
        unknown: map.unknown,
        class_id: map.class_id,
        user_data: gbx::header::build_user_data(&user_data_chunks),
        num_nodes: map.num_nodes,
        ref_table: map.ref_table.clone(),
        body: Vec::new(),
        comp: None,
    };
    // Maps must be written LZO-compressed; a fresh stream, verified by the
    // writer against its own decompression.
    let bytes = out_gbx.write_body_recompressed(&body);
    Ok(Embedded { bytes, log, author_ms: declared, medals: (gold, silver, bronze) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medals_match_nadeo_on_summer_01() {
        assert_eq!(medals(23144), (25000, 28000, 35000));
    }

    #[test]
    fn xml_attr_replacement_is_local() {
        let x = r#"<header><times bronze="35000" silver="28000" gold="25000" authortime="23144" authorscore="0"/><desc validated="1"/></header>"#;
        let y = set_xml_attr(x, "times", "authortime", "31769").unwrap();
        assert!(y.contains(r#"authortime="31769""#));
        assert!(y.contains(r#"bronze="35000""#));
        assert_eq!(x.len() + 0, y.len());
    }
}
