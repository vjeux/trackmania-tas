//! `mapgeom zipcheck MAP.Map.Gbx…` — the embedded-object chunk (0x03043054) and
//! its ZIP, verified field by field, the way a strict reader would see them.
//!
//! The "Missing Items: AC00000000.Item.Gbx" dialog of the big archives names
//! an item that IS in the archive, first of 757 entries, structurally sound.
//! Before blaming the game's loader for re-reading a large archive badly, the
//! archive itself has to be above suspicion: local headers vs the central
//! directory, CRCs, sizes, duplicate or oddly encoded names, the ordering, the
//! manifest's ident list against the entries, each item file's own header
//! ident against the manifest row that names it. This prints all of that, and
//! the same report of a GAME-WRITTEN archive (an original map with club items)
//! is the reference to diff against.

use std::collections::{BTreeMap, BTreeSet};

use tmmaps::map::MapFile;

pub const EMBEDDED_CHUNK: u32 = 0x03043054;

#[derive(Default, Debug)]
pub struct LocalEntry {
    pub offset: usize,
    pub version: u16,
    pub flags: u16,
    pub method: u16,
    pub time: u16,
    pub date: u16,
    pub crc: u32,
    pub csize: u32,
    pub usize_: u32,
    pub name: String,
    pub name_raw: Vec<u8>,
    pub extra_len: u16,
    pub data_start: usize,
}

#[derive(Default, Debug)]
pub struct CentralEntry {
    pub made_by: u16,
    pub version: u16,
    pub flags: u16,
    pub method: u16,
    pub time: u16,
    pub date: u16,
    pub crc: u32,
    pub csize: u32,
    pub usize_: u32,
    pub name: String,
    pub extra_len: u16,
    pub comment_len: u16,
    pub disk: u16,
    pub iattr: u16,
    pub eattr: u32,
    pub local_offset: u32,
}

fn u16at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// The chunk's fields.
pub struct Chunk054<'a> {
    pub version: u32,
    pub u01: i32,
    pub inner_len: usize,
    /// (name, collection, author) per manifest row
    pub manifest: Vec<(String, String, String)>,
    pub zip: &'a [u8],
    pub textures: Vec<String>,
    pub tail_bytes: usize,
}

pub fn read_chunk(m: &MapFile) -> Result<Chunk054<'_>, String> {
    let body = &m.gbx.body;
    let (_, _, payload, size) = tmmaps::map::skip_chunks(body).into_iter().find(|(id, ..)| *id == EMBEDDED_CHUNK).ok_or("no embedded-objects chunk 0x03043054")?;
    let chunk = &body[payload..payload + size];
    let mut r = crate::reader::Reader::new(chunk);
    let version = r.u32()?;
    let u01 = r.i32()?;
    let inner_len = r.u32()? as usize;
    if inner_len != chunk.len() - 12 {
        return Err(format!("inner length {inner_len} but the chunk has {} bytes after the 12-byte prefix", chunk.len() - 12));
    }
    let inner = r.take(inner_len)?;
    let mut ir = crate::reader::Reader::new(inner);
    let n_meta = ir.u32()? as usize;
    let mut manifest = Vec::with_capacity(n_meta);
    for _ in 0..n_meta {
        manifest.push(ir.meta()?);
    }
    let zip_len = ir.u32()? as usize;
    let zip_off = ir.o;
    let zip = &inner[zip_off..zip_off + zip_len];
    ir.take(zip_len)?;
    let mut textures = Vec::new();
    let mut tail_bytes = inner.len() - ir.o;
    if version >= 1 && ir.o + 4 <= inner.len() {
        let n_tex = ir.u32()? as usize;
        for _ in 0..n_tex {
            textures.push(ir.string()?);
        }
        tail_bytes = inner.len() - ir.o;
    }
    Ok(Chunk054 { version, u01, inner_len, manifest, zip, textures, tail_bytes })
}

pub fn locals(zip: &[u8]) -> Result<Vec<LocalEntry>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 4 <= zip.len() && &zip[i..i + 4] == b"PK\x03\x04" {
        if i + 30 > zip.len() {
            return Err(format!("local header at {i:#x} truncated"));
        }
        let mut e = LocalEntry { offset: i, ..Default::default() };
        e.version = u16at(zip, i + 4);
        e.flags = u16at(zip, i + 6);
        e.method = u16at(zip, i + 8);
        e.time = u16at(zip, i + 10);
        e.date = u16at(zip, i + 12);
        e.crc = u32at(zip, i + 14);
        e.csize = u32at(zip, i + 18);
        e.usize_ = u32at(zip, i + 22);
        let nlen = u16at(zip, i + 26) as usize;
        e.extra_len = u16at(zip, i + 28);
        e.name_raw = zip[i + 30..i + 30 + nlen].to_vec();
        e.name = String::from_utf8_lossy(&e.name_raw).into_owned();
        e.data_start = i + 30 + nlen + e.extra_len as usize;
        if e.flags & 0x8 != 0 {
            return Err(format!("entry {} at {i:#x} uses a data descriptor (flag 3) — the sizes are after the data; this checker does not follow those", e.name));
        }
        i = e.data_start + e.csize as usize;
        if i > zip.len() {
            return Err(format!("entry {} at {:#x}: data runs {} bytes past the end of the archive", e.name, e.offset, i - zip.len()));
        }
        out.push(e);
    }
    Ok(out)
}

pub struct Central {
    pub entries: Vec<CentralEntry>,
    pub cd_offset: u32,
    pub cd_size: u32,
    pub eocd_offset: usize,
    pub n_this_disk: u16,
    pub n_total: u16,
    pub comment_len: u16,
}

pub fn central(zip: &[u8]) -> Result<Central, String> {
    // the end-of-central-directory record: last 22 bytes when there is no comment
    if zip.len() < 22 {
        return Err("archive shorter than an EOCD record".into());
    }
    let mut eocd = None;
    let lo = zip.len().saturating_sub(22 + 65535);
    let mut k = zip.len() - 22;
    loop {
        if &zip[k..k + 4] == b"PK\x05\x06" {
            eocd = Some(k);
            break;
        }
        if k == lo {
            break;
        }
        k -= 1;
    }
    let e = eocd.ok_or("no end-of-central-directory record")?;
    let n_this_disk = u16at(zip, e + 8);
    let n_total = u16at(zip, e + 10);
    let cd_size = u32at(zip, e + 12);
    let cd_offset = u32at(zip, e + 16);
    let comment_len = u16at(zip, e + 20);
    if e + 22 + comment_len as usize != zip.len() {
        return Err(format!("EOCD at {e:#x} + comment {comment_len} does not end the archive ({} bytes)", zip.len()));
    }
    let mut entries = Vec::new();
    let mut i = cd_offset as usize;
    let cd_end = i + cd_size as usize;
    if cd_end > e {
        return Err(format!("central directory {i:#x}..{cd_end:#x} overlaps the EOCD at {e:#x}"));
    }
    while i < cd_end {
        if &zip[i..i + 4] != b"PK\x01\x02" {
            return Err(format!("central directory entry at {i:#x}: bad signature"));
        }
        let mut c = CentralEntry { made_by: u16at(zip, i + 4), version: u16at(zip, i + 6), flags: u16at(zip, i + 8), method: u16at(zip, i + 10), time: u16at(zip, i + 12), date: u16at(zip, i + 14), crc: u32at(zip, i + 16), csize: u32at(zip, i + 20), usize_: u32at(zip, i + 24), ..Default::default() };
        let nlen = u16at(zip, i + 28) as usize;
        c.extra_len = u16at(zip, i + 30);
        c.comment_len = u16at(zip, i + 32);
        c.disk = u16at(zip, i + 34);
        c.iattr = u16at(zip, i + 36);
        c.eattr = u32at(zip, i + 38);
        c.local_offset = u32at(zip, i + 42);
        c.name = String::from_utf8_lossy(&zip[i + 46..i + 46 + nlen]).into_owned();
        i += 46 + nlen + c.extra_len as usize + c.comment_len as usize;
        entries.push(c);
    }
    if i != cd_end {
        return Err(format!("central directory entries end at {i:#x}, the record says {cd_end:#x}"));
    }
    Ok(Central { entries, cd_offset, cd_size, eocd_offset: e, n_this_disk, n_total, comment_len })
}

fn dos_date(d: u16, t: u16) -> String {
    let (y, mo, da) = (1980 + (d >> 9) as u32, ((d >> 5) & 0xF) as u32, (d & 0x1F) as u32);
    let (h, mi, s) = ((t >> 11) as u32, ((t >> 5) & 0x3F) as u32, ((t & 0x1F) * 2) as u32);
    format!("{y:04}-{mo:02}-{da:02} {h:02}:{mi:02}:{s:02}")
}

/// Slash-insensitive: the game writes entries with forward slashes and
/// manifest idents with backslashes (original Summer 21: `Items/TME/Nations/…`
/// vs `TME\Nations\…`).
fn slashed(s: &str) -> String {
    s.replace('\\', "/")
}

pub fn run(args: &[String]) -> Result<(), String> {
    let maps: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with('-')).collect();
    if maps.is_empty() {
        return Err("zipcheck MAP.Map.Gbx… [--verbose]".into());
    }
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    for p in maps {
        println!("== {p}");
        let m = MapFile::load(std::path::Path::new(p));
        check_map(&m, verbose)?;
    }
    Ok(())
}

pub fn check_map(m: &MapFile, verbose: bool) -> Result<(), String> {
    let c = read_chunk(m)?;
    println!("chunk 0x03043054: version {} u01 {} inner {} B; manifest {} idents; zip {} B; textures {:?}; {} trailing bytes", c.version, c.u01, c.inner_len, c.manifest.len(), c.zip.len(), c.textures, c.tail_bytes);
    let mut problems: Vec<String> = Vec::new();
    // manifest
    let mut colls: BTreeSet<&str> = BTreeSet::new();
    let mut man_names: BTreeSet<&str> = BTreeSet::new();
    let mut author_eq_name = 0;
    for (name, coll, author) in &c.manifest {
        colls.insert(coll);
        if !man_names.insert(name) {
            problems.push(format!("manifest names {name} twice"));
        }
        if author == name {
            author_eq_name += 1;
        }
    }
    let authors: BTreeSet<&str> = c.manifest.iter().map(|(_, _, a)| a.as_str()).collect();
    println!("manifest: collections {:?}; {} rows with author == name; {} distinct authors{}", colls, author_eq_name, authors.len(), if authors.len() <= 5 { format!(" {:?}", authors) } else { String::new() });
    if let Some((n, co, a)) = c.manifest.first() {
        println!("  first row: ({n}, {co}, {a})");
    }
    if let Some((n, co, a)) = c.manifest.last() {
        println!("  last row:  ({n}, {co}, {a})");
    }
    // zip
    let zip = c.zip;
    let locs = locals(zip)?;
    let cen = central(zip)?;
    let local_end = locs.last().map(|e| e.data_start + e.csize as usize).unwrap_or(0);
    println!("zip: {} local entries ending at {:#x}; central directory at {:#x} ({} B, {} entries; EOCD says {}/{} entries, comment {} B) at {:#x}", locs.len(), local_end, cen.cd_offset, cen.cd_size, cen.entries.len(), cen.n_this_disk, cen.n_total, cen.comment_len, cen.eocd_offset);
    if local_end != cen.cd_offset as usize {
        problems.push(format!("{} bytes between the last local entry ({local_end:#x}) and the central directory ({:#x})", cen.cd_offset as i64 - local_end as i64, cen.cd_offset));
    }
    if cen.entries.len() != locs.len() || cen.n_total as usize != locs.len() {
        problems.push(format!("entry counts disagree: {} local, {} central, EOCD {}", locs.len(), cen.entries.len(), cen.n_total));
    }
    let mut methods: BTreeMap<u16, usize> = BTreeMap::new();
    let mut versions: BTreeSet<(u16, u16, u16)> = BTreeSet::new();
    let mut dates: BTreeSet<String> = BTreeSet::new();
    let mut extra: usize = 0;
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    let mut lower: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut backslash = 0;
    let mut non_ascii = 0;
    let (mut total_c, mut total_u) = (0u64, 0u64);
    let mut biggest: (usize, String) = (0, String::new());
    let mut crc_bad = 0;
    let mut inflate_bad = 0;
    let mut ext: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    let mut sorted = true;
    let mut ent_by_off: BTreeMap<u32, &CentralEntry> = BTreeMap::new();
    for ce in &cen.entries {
        ent_by_off.insert(ce.local_offset, ce);
    }
    for (k, e) in locs.iter().enumerate() {
        *methods.entry(e.method).or_default() += 1;
        dates.insert(dos_date(e.date, e.time));
        extra += e.extra_len as usize;
        *names.entry(e.name.clone()).or_default() += 1;
        lower.entry(e.name.to_lowercase()).or_default().push(e.name.clone());
        if e.name.contains('\\') {
            backslash += 1;
        }
        if !e.name.is_ascii() {
            non_ascii += 1;
        }
        total_c += e.csize as u64;
        total_u += e.usize_ as u64;
        if e.usize_ as usize > biggest.0 {
            biggest = (e.usize_ as usize, e.name.clone());
        }
        if k > 0 && locs[k - 1].name > e.name {
            sorted = false;
        }
        let x = e.name.rsplit('.').next().unwrap_or("").to_lowercase();
        let slot = ext.entry(x).or_default();
        slot.0 += 1;
        slot.1 += e.usize_ as u64;
        let data = &zip[e.data_start..e.data_start + e.csize as usize];
        let raw = match e.method {
            0 => Some(data.to_vec()),
            8 => miniz_oxide::inflate::decompress_to_vec(data).ok(),
            _ => None,
        };
        match raw {
            Some(r) => {
                if r.len() != e.usize_ as usize {
                    inflate_bad += 1;
                    if verbose {
                        println!("  {}: inflates to {} B, header says {}", e.name, r.len(), e.usize_);
                    }
                }
                if crc32(&r) != e.crc {
                    crc_bad += 1;
                    if verbose {
                        println!("  {}: CRC {:08x} vs stored {:08x}", e.name, crc32(&r), e.crc);
                    }
                }
            }
            None => {
                inflate_bad += 1;
                if verbose {
                    println!("  {}: method {} data does not inflate", e.name, e.method);
                }
            }
        }
        // the central directory row for this local header
        match ent_by_off.get(&(e.offset as u32)) {
            None => problems.push(format!("{}: no central directory row points at its local header {:#x}", e.name, e.offset)),
            Some(ce) => {
                versions.insert((ce.made_by, ce.version, e.version));
                let mut diffs = Vec::new();
                if ce.name != e.name {
                    diffs.push(format!("name {:?}", ce.name));
                }
                if ce.crc != e.crc {
                    diffs.push("crc".into());
                }
                if ce.csize != e.csize {
                    diffs.push("csize".into());
                }
                if ce.usize_ != e.usize_ {
                    diffs.push("usize".into());
                }
                if ce.method != e.method {
                    diffs.push("method".into());
                }
                if ce.flags != e.flags {
                    diffs.push("flags".into());
                }
                if ce.time != e.time || ce.date != e.date {
                    diffs.push("time/date".into());
                }
                if !diffs.is_empty() {
                    problems.push(format!("{}: central directory differs in {}", e.name, diffs.join(", ")));
                }
                if verbose && (ce.extra_len != 0 || ce.comment_len != 0 || ce.disk != 0) {
                    println!("  {}: central extra {} comment {} disk {}", e.name, ce.extra_len, ce.comment_len, ce.disk);
                }
            }
        }
    }
    let dups: Vec<&String> = names.iter().filter(|(_, &n)| n > 1).map(|(n, _)| n).collect();
    let case_dups: Vec<&Vec<String>> = lower.values().filter(|v| v.len() > 1).collect();
    println!("  methods {:?}; versions (made_by, needed, local) {:?}; dates {:?}; extra fields {} B; flags {:?}", methods, versions, dates, extra, locs.iter().map(|e| e.flags).collect::<BTreeSet<_>>());
    println!("  names: {} distinct, {} duplicate, {} case-insensitive collisions, {} with backslashes, {} non-ascii; sorted by name: {}", names.len(), dups.len(), case_dups.len(), backslash, non_ascii, sorted);
    println!("  bytes: {} compressed / {} uncompressed ({:.1} %); biggest entry {} B {}", total_c, total_u, 100.0 * total_c as f64 / total_u.max(1) as f64, biggest.0, biggest.1);
    println!("  CRC mismatches {crc_bad}; inflate failures/size mismatches {inflate_bad}");
    for (x, (n, b)) in &ext {
        println!("    .{x:<10} {n:>5} entries {b:>11} B");
    }
    if !dups.is_empty() {
        problems.push(format!("duplicate names: {:?}", dups));
    }
    if !case_dups.is_empty() {
        problems.push(format!("case-insensitive collisions: {:?}", case_dups));
    }
    let eattr: BTreeSet<u32> = cen.entries.iter().map(|c| c.eattr).collect();
    let iattr: BTreeSet<u16> = cen.entries.iter().map(|c| c.iattr).collect();
    println!("  central: external attrs {:?}, internal attrs {:?}", eattr, iattr);
    if verbose {
        for e in locs.iter().take(3).chain(locs.iter().rev().take(2)) {
            println!("  entry @{:#x}: v{} flags {:#x} method {} {} crc {:08x} {}->{} B name {:?} extra {}", e.offset, e.version, e.flags, e.method, dos_date(e.date, e.time), e.crc, e.usize_, e.csize, e.name, e.extra_len);
        }
    }
    // manifest vs entries
    let item_entries: BTreeSet<String> = locs.iter().filter(|e| e.name.to_lowercase().ends_with(".item.gbx")).map(|e| e.name.clone()).collect();
    let mut man_missing = Vec::new();
    let mut man_hdr_bad = Vec::new();
    for (name, coll, author) in &c.manifest {
        let key = format!("Items/{}", slashed(name));
        let Some(e) = locs.iter().find(|e| slashed(&e.name) == key) else {
            man_missing.push(name.clone());
            continue;
        };
        // the item file's own header ident
        let data = &zip[e.data_start..e.data_start + e.csize as usize];
        let raw = match e.method {
            0 => data.to_vec(),
            _ => miniz_oxide::inflate::decompress_to_vec(data).unwrap_or_default(),
        };
        if let Some((hn, ha)) = tmmaps::header::item_ident_author(&raw) {
            if hn != *name || ha != *author {
                man_hdr_bad.push(format!("{name}: file header ident ({hn}, {ha}) vs manifest ({name}, {coll}, {author})"));
            }
        } else {
            man_hdr_bad.push(format!("{name}: no header ident chunk 0x2E001003 readable"));
        }
    }
    let man_slashed: BTreeSet<String> = man_names.iter().map(|n| slashed(n)).collect();
    let unlisted: Vec<&String> = item_entries
        .iter()
        .filter(|n| {
            let base = slashed(n);
            let base = base.trim_start_matches("Items/");
            !man_slashed.contains(base)
        })
        .collect();
    let unlisted_bytes: u64 = locs.iter().filter(|e| unlisted.iter().any(|u| **u == e.name)).map(|e| e.csize as u64).sum();
    println!(
        "manifest vs archive: {} manifest rows, {} .Item.Gbx entries; {} rows without an entry; {} item entries not in the manifest ({} compressed bytes); {} header-ident mismatches",
        c.manifest.len(),
        item_entries.len(),
        man_missing.len(),
        unlisted.len(),
        unlisted_bytes,
        man_hdr_bad.len()
    );
    if !man_missing.is_empty() {
        problems.push(format!("manifest rows without a zip entry: {:?}", &man_missing[..man_missing.len().min(10)]));
    }
    if !unlisted.is_empty() {
        problems.push(format!("item entries not in the manifest: {:?}", &unlisted[..unlisted.len().min(10)]));
    }
    for l in man_hdr_bad.iter().take(if verbose { 1000 } else { 5 }) {
        problems.push(l.clone());
    }
    // placements vs manifest
    let mut placed: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unresolved: BTreeSet<String> = BTreeSet::new();
    let mut author_mismatch = 0;
    for it in &m.items {
        *placed.entry(it.model.as_str()).or_default() += 1;
        if it.model.ends_with(".Item.Gbx") {
            match c.manifest.iter().find(|(n, ..)| *n == it.model) {
                None => {
                    unresolved.insert(it.model.clone());
                }
                Some((_, _, a)) => {
                    if it.author.as_deref() != Some(a.as_str()) {
                        author_mismatch += 1;
                    }
                }
            }
        }
    }
    let placed_embedded = placed.iter().filter(|(n, _)| n.ends_with(".Item.Gbx")).count();
    let unplaced: Vec<&str> = man_names.iter().filter(|n| !placed.contains_key(**n)).copied().collect();
    println!(
        "placements: {} items, {} distinct embedded models placed, {} manifest rows never placed, {} placed models missing from the manifest, {} placements whose author differs from the manifest's",
        m.items.len(),
        placed_embedded,
        unplaced.len(),
        unresolved.len(),
        author_mismatch
    );
    if !unresolved.is_empty() {
        problems.push(format!("placed models missing from the manifest: {:?}", unresolved.iter().take(10).collect::<Vec<_>>()));
    }
    if problems.is_empty() {
        println!("VERDICT: archive and manifest are consistent (no problems found)");
    } else {
        println!("VERDICT: {} problem(s)", problems.len());
        for p in &problems {
            println!("  ! {p}");
        }
    }
    Ok(())
}
