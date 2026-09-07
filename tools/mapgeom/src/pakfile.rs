//! Nadeo pak file-data reader: blowfish (optional) + the chunked LZ4 stream
//! with the 1006-byte built-in dictionary.
use crate::lz4dict::LZ4_DICT;

/// Decode one LZ4 block into `hist` (which already holds the dictionary and
/// everything decoded so far). Returns the number of bytes produced.
pub fn lz4_block(src: &[u8], hist: &mut Vec<u8>) -> Result<usize, String> {
    lz4_block_at(src, hist).map_err(|(e, _)| e)
}

/// `lz4_block`, with the number of input bytes consumed when it fails.
pub fn lz4_block_at(src: &[u8], hist: &mut Vec<u8>) -> Result<usize, (String, usize)> {
    let start = hist.len();
    let mut i = 0usize;
    while i < src.len() {
        let token = src[i];
        i += 1;
        let mut lit = (token >> 4) as usize;
        if lit == 15 {
            loop {
                if i >= src.len() {
                    return Err(("literal length overrun".into(), i));
                }
                let b = src[i];
                i += 1;
                lit += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        if i + lit > src.len() {
            return Err(("literal overrun".into(), i));
        }
        hist.extend_from_slice(&src[i..i + lit]);
        i += lit;
        if i == src.len() {
            break; // last sequence has no match part
        }
        if i + 2 > src.len() {
            return Err(("offset overrun".into(), i));
        }
        let off = u16::from_le_bytes([src[i], src[i + 1]]) as usize;
        i += 2;
        if off == 0 || off > hist.len() {
            // A malformed final sequence: keep what decoded so far when the
            // caller asked for leniency (a partial body still yields strings).
            if std::env::var_os("MAPGEOM_LENIENT_LZ4").is_some() {
                return Ok(hist.len() - start);
            }
            return Err((format!("bad match offset {} (hist {})", off, hist.len()), i - 2));
        }
        let mut mlen = (token & 0xF) as usize;
        if mlen == 15 {
            loop {
                if i >= src.len() {
                    return Err(("match length overrun".into(), i));
                }
                let b = src[i];
                i += 1;
                mlen += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        mlen += 4;
        let mut p = hist.len() - off;
        for _ in 0..mlen {
            let b = hist[p];
            hist.push(b);
            p += 1;
        }
    }
    Ok(hist.len() - start)
}

/// Read a file's bytes out of the pak.
pub fn read_file(
    data: &[u8],
    header_max_size: usize,
    e: &crate::pak::PakEntry,
    key: &[u8; 16],
    version: i32,
) -> Result<Vec<u8>, String> {
    let base = header_max_size + e.offset as usize;
    if base >= data.len() {
        return Err("offset past EOF".into());
    }
    // raw (possibly still compressed) bytes
    let raw: Vec<u8> = if e.is_encrypted() {
        if !e.is_compressed() && !e.dont_use_dummy_write() {
            return decrypt_with_dummy_writes(data, base, e, key, version);
        }
        if e.is_compressed() && !e.dont_use_dummy_write() {
            return read_compressed_with_dummy_writes(data, base, e, key, version);
        }
        let mut r = crate::pak::CipherReader::new(data, base, key, version);
        r.take(e.compressed_size.max(0) as usize)
    } else {
        let n = e.compressed_size.max(0) as usize;
        if base + n > data.len() {
            return Err("compressed size past EOF".into());
        }
        data[base..base + n].to_vec()
    };
    if !e.is_compressed() {
        return Ok(raw);
    }
    let want = e.uncompressed_size.max(0) as usize;
    let mut hist: Vec<u8> = Vec::with_capacity(LZ4_DICT.len() + want + 4096);
    hist.extend_from_slice(LZ4_DICT);
    let dict_len = hist.len();
    let mut i = 0usize;
    while hist.len() - dict_len < want {
        if i + 2 > raw.len() {
            if std::env::var_os("MAPGEOM_LENIENT_LZ4").is_some() {
                break;
            }
            return Err(format!(
                "ran out of compressed data at {}/{} bytes",
                hist.len() - dict_len,
                want
            ));
        }
        let n = u16::from_le_bytes([raw[i], raw[i + 1]]) as usize;
        i += 2;
        if n > 4128 || i + n > raw.len() {
            return Err(format!("bad lz4 chunk size {}", n));
        }
        let before = hist.len();
        lz4_block(&raw[i..i + n], &mut hist)?;
        i += n;
        if std::env::var_os("MAPGEOM_LENIENT_LZ4").is_some() && hist.len() == before {
            break;
        }
    }
    if hist.len() < dict_len + want {
        return Ok(hist[dict_len..].to_vec());
    }
    Ok(hist[dict_len..dict_len + want].to_vec())
}

/// Decrypt `n` bytes at `base`, folding each scheduled `(offset, class)` into
/// the cipher's IV perturbation when the read reaches `offset` — the game's
/// "dummy write" of a node's parent class id at the start of its body
/// (`parents.rs`). `schedule` is sorted by offset.
fn decrypt_scheduled(data: &[u8], base: usize, key: &[u8; 16], version: i32, n: usize, schedule: &[(usize, u32)]) -> Vec<u8> {
    let mut r = crate::pak::CipherReader::new(data, base, key, version);
    let mut out = Vec::with_capacity(n);
    let mut pos = 0usize;
    for &(off, class) in schedule {
        let off = off.min(n);
        if off > pos {
            out.extend(r.take(off - pos));
            pos = off;
        }
        r.initialize(&class.to_le_bytes(), 0, 4);
    }
    if n > pos {
        out.extend(r.take(n - pos));
    }
    out
}

/// The node-body starts of a (partially) decrypted Gbx file, as
/// (file offset, parent class id folded there), in read order: the main node
/// at the start of the body, then every inline node right after its class id.
/// The walk stops at the first byte it cannot read; what it found before that
/// is right as long as the bytes were.
fn dummy_write_points(plain: &[u8], class_id: u32) -> Vec<(usize, u32)> {
    let Ok(g) = crate::container::Gbx::parse(plain) else { return Vec::new() };
    let body_start = plain.len() - g.body.len();
    let main_fold = crate::parents::dummy_write_class(class_id).or_else(|| {
        // MAPGEOM_FOLD_MAIN=hex: a fold class for a main node the table does
        // not know (the probe for a table-less class's parent)
        std::env::var("MAPGEOM_FOLD_MAIN").ok().and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
    });
    let mut out: Vec<(usize, u32)> = main_fold.map(|c| (body_start, c)).into_iter().collect();
    let externals: Vec<(u32, String)> = g.refs.iter().map(|e| (e.node_index, e.name.clone())).collect();
    let mut graph = crate::node::Graph::new(&g.body, g.num_nodes, &externals);
    let _ = graph.node_body(class_id);
    for (off, c) in &graph.node_starts {
        // the main node's own start is `body_start` (its class id sits in
        // the header, not in the body)
        if *off == 0 {
            continue;
        }
        if let Some(fold) = crate::parents::dummy_write_class(*c) {
            out.push((body_start + off, fold));
        }
    }
    out
}

/// An encrypted, uncompressed, dummy-written pak file: decrypt, find where its
/// node bodies begin, decrypt again with those perturbations, repeat until the
/// walk finds nothing new. Each round extends the correctly decrypted prefix
/// past at least one more 0x100 boundary, so it ends within `num_nodes` rounds.
fn decrypt_with_dummy_writes(data: &[u8], base: usize, e: &crate::pak::PakEntry, key: &[u8; 16], version: i32) -> Result<Vec<u8>, String> {
    let n = e.compressed_size.max(0) as usize;
    let mut schedule: Vec<(usize, u32)> = Vec::new();
    let mut plain = decrypt_scheduled(data, base, key, version, n, &schedule);
    for _round in 0..256 {
        let points = dummy_write_points(&plain, e.class_id);
        let Some(first_new) = points.iter().find(|p| !schedule.contains(p)) else { break };
        // a perturbation folded at `off` lands at the next 0x100 boundary
        // (a start exactly on the boundary still lands there: the cipher
        // re-keys when the byte AFTER it is asked for). Bytes before that
        // boundary were decrypted right, so every new start up to it is real.
        let boundary = (first_new.0 + 0xFF) & !0xFF;
        let before = schedule.len();
        let fresh: Vec<(usize, u32)> = points.iter().filter(|p| p.0 <= boundary && !schedule.contains(p)).copied().collect();
        schedule.extend(fresh);
        if schedule.len() == before {
            break;
        }
        schedule.sort();
        plain = decrypt_scheduled(data, base, key, version, n, &schedule);
    }
    Ok(plain)
}

/// The chunked LZ4 stream decoded as far as it goes: the plain bytes, the
/// compressed offset at which each chunk's u16 size word sits (index k = the
/// chunk holding plain bytes 4096k..4096(k+1); one extra entry = where the
/// chunk after the last decoded one would start), and the compressed offset
/// the decode reached (the whole stream when `Ok`). `Ok` when `want` bytes
/// came out, else the error that stopped the decode (the plain prefix is kept).
fn lz4_stream(raw: &[u8], want: usize) -> (Vec<u8>, Vec<usize>, Result<(), String>) {
    let (p, s, st, _) = lz4_stream_reach(raw, want);
    (p, s, st)
}

/// `lz4_stream` plus the compressed offset the decode reached. `starts` has
/// one entry per chunk that decoded whole plus one for where the next chunk
/// begins; a failing chunk is NOT pushed, so it is chunk `starts.len() - 1`,
/// beginning at `starts.last()`.
fn lz4_stream_reach(raw: &[u8], want: usize) -> (Vec<u8>, Vec<usize>, Result<(), String>, usize) {
    let mut hist: Vec<u8> = Vec::with_capacity(LZ4_DICT.len() + want + 4096);
    hist.extend_from_slice(LZ4_DICT);
    let dict_len = hist.len();
    let (starts, status, reach) = lz4_continue(raw, 0, &mut hist, dict_len, want);
    let end = hist.len().min(dict_len + want);
    (hist[dict_len..end].to_vec(), starts, status, reach)
}

/// Decode chunks from compressed offset `from` on into `hist` (which holds the
/// dictionary plus every plain byte before `from`'s chunk) until `want` plain
/// bytes exist past `dict_len`. Returns the chunk starts from `from` on (the
/// failing chunk not pushed), the status, and the compressed offset reached.
fn lz4_continue(raw: &[u8], from: usize, hist: &mut Vec<u8>, dict_len: usize, want: usize) -> (Vec<usize>, Result<(), String>, usize) {
    let mut starts: Vec<usize> = vec![from];
    let mut i = from;
    let mut status: Result<(), String> = Ok(());
    let mut reach = from;
    while hist.len() - dict_len < want {
        if i + 2 > raw.len() {
            status = Err(format!("ran out of compressed data at {}/{} bytes", hist.len() - dict_len, want));
            reach = raw.len();
            break;
        }
        let n = u16::from_le_bytes([raw[i], raw[i + 1]]) as usize;
        if n > 4128 || i + 2 + n > raw.len() {
            status = Err(format!("bad lz4 chunk size {n} at compressed offset {i}"));
            reach = i;
            break;
        }
        let before = hist.len();
        let r = lz4_block_at(&raw[i + 2..i + 2 + n], hist);
        match r {
            Err((e, consumed)) => {
                status = Err(format!("{e} in the chunk at compressed offset {i}"));
                reach = i + 2 + consumed;
                break;
            }
            Ok(_) if hist.len() == before => {
                status = Err(format!("empty lz4 chunk at compressed offset {i}"));
                reach = i;
                break;
            }
            // every chunk but the last holds exactly 4096 plain bytes (the block
            // compressor emits a chunk when its buffer fills): anything else
            // before the end is garbage that happened to parse
            Ok(_) if hist.len() - before != 4096 && hist.len() - dict_len < want => {
                status = Err(format!("short lz4 chunk ({} plain bytes) at compressed offset {i}", hist.len() - before));
                hist.truncate(before);
                reach = i;
                break;
            }
            Ok(_) => {
                i += 2 + n;
                starts.push(i);
                reach = i;
            }
        }
    }
    (starts, status, reach)
}

/// Where a dummy write at PLAIN offset `p` reaches the cipher of a compressed
/// file: the writer's node bodies go through the LZ4 block compressor (4096
/// plain bytes per chunk), so the four bytes of the parent class id fold into
/// the cipher at the compressed position the stream has reached when the
/// chunk holding `p` is emitted — the start of the next chunk. Measured on
/// `Stadium\Media\VegetTreeModel\PalmTreeSmall.VegetTreeModel.Gbx` (169 KB,
/// flags 0x5): the first chunk decoded right without any fold and the second
/// went bad past a 0x100 boundary, which a fold before the first chunk could
/// not produce (`MAPGEOM_LZ4_FOLD=before` keeps that model for A/B).
fn fold_position(p: usize, chunk_starts: &[usize]) -> Option<usize> {
    let k = p / 4096;
    let idx = if std::env::var_os("MAPGEOM_LZ4_FOLD").map(|v| v == "before").unwrap_or(false) { k } else { k + 1 };
    chunk_starts.get(idx).copied()
}

/// Decrypt `n` bytes at `base`, folding `(offset, class)` pairs in the given
/// order when the read reaches each offset (several may share one offset).
fn decrypt_scheduled_ordered(data: &[u8], base: usize, key: &[u8; 16], version: i32, n: usize, schedule: &[(usize, usize, u32)]) -> Vec<u8> {
    let mut r = crate::pak::CipherReader::new(data, base, key, version);
    let mut out = Vec::with_capacity(n);
    let mut pos = 0usize;
    // MAPGEOM_FOLD_BYTES=1..4 (default 4): how many bytes of each value fold
    // (the hunt's probe for a perturbation that is not a 4-byte class id)
    let width: usize = std::env::var("MAPGEOM_FOLD_BYTES").ok().and_then(|v| v.parse().ok()).filter(|w| (1..=4).contains(w)).unwrap_or(4);
    for &(off, _, class) in schedule {
        let off = off.min(n);
        if off > pos {
            out.extend(r.take(off - pos));
            pos = off;
        }
        r.initialize(&class.to_le_bytes(), 0, width);
    }
    if n > pos {
        out.extend(r.take(n - pos));
    }
    out
}

/// An encrypted, LZ4-compressed, dummy-written pak file (the tree models,
/// every big `.Gbx` of the packs): decrypt, decompress as far as the bytes are
/// right, find the node-body starts in the plain prefix, fold their parent
/// class ids at the compressed positions `fold_position` gives, decrypt again
/// — until the whole file decodes or a round finds nothing new. Schedule
/// entries are (compressed offset, plain offset, class), kept in plain order
/// within one compressed offset (the fold is order-dependent).
fn read_compressed_with_dummy_writes(data: &[u8], base: usize, e: &crate::pak::PakEntry, key: &[u8; 16], version: i32) -> Result<Vec<u8>, String> {
    let n = e.compressed_size.max(0) as usize;
    let want = e.uncompressed_size.max(0) as usize;
    // MAPGEOM_LZ4_RESYNC=skipK|newiv: at the first 0x100 boundary after each
    // chunk ends, the ciphertext carries K extra bytes (skipped) or a fresh
    // 8-byte IV (cipher restarted on it) — probes for the tree models
    if let Ok(mode) = std::env::var("MAPGEOM_LZ4_RESYNC") {
        let skip: usize = mode.strip_prefix("skip").and_then(|k| k.parse().ok()).unwrap_or(8);
        let mut hist: Vec<u8> = Vec::with_capacity(LZ4_DICT.len() + want + 4096);
        hist.extend_from_slice(LZ4_DICT);
        let dict_len = hist.len();
        // ciphertext position (absolute in `data`) and a running cipher
        let mut st = Resync { r: crate::pak::CipherReader::new_at(data, base, base + 8, key, version), cpos: base + 8, produced: 0, resync_at: None };
        let mut k = 0usize;
        while hist.len() - dict_len < want {
            let head = st.take(data, base, key, version, &mode, skip, 2);
            let cn = u16::from_le_bytes([head[0], head[1]]) as usize;
            if cn > 4128 {
                return Err(format!("resync {mode}: bad lz4 chunk size {cn} in chunk {k}"));
            }
            let block = st.take(data, base, key, version, &mode, skip, cn);
            lz4_block(&block, &mut hist).map_err(|err| format!("resync {mode}: {err} in chunk {k}"))?;
            k += 1;
            // the next resync: first 0x100 boundary at/after the current decrypted offset
            st.resync_at = Some((st.produced + 0xFF) & !0xFF);
        }
        let end = hist.len().min(dict_len + want);
        return Ok(hist[dict_len..end].to_vec());
    }
    // MAPGEOM_LZ4_CIPHER=restart: the cipher starts over (same IV) at every
    // whose second chunk never decodes under one continuous cipher stream
    if std::env::var("MAPGEOM_LZ4_CIPHER").ok().as_deref() == Some("restart") {
        let mut hist: Vec<u8> = Vec::with_capacity(LZ4_DICT.len() + want + 4096);
        hist.extend_from_slice(LZ4_DICT);
        let dict_len = hist.len();
        let mut pos = 0usize;
        let mut k = 0usize;
        while hist.len() - dict_len < want {
            if pos + 2 > n {
                return Err(format!("restart: ran out of compressed data at {}/{want} after {k} chunks", hist.len() - dict_len));
            }
            let mut r = crate::pak::CipherReader::new_at(data, base, base + 8 + pos, key, version);
            let head = r.take(2);
            let cn = u16::from_le_bytes([head[0], head[1]]) as usize;
            if cn > 4128 || pos + 2 + cn > n {
                return Err(format!("restart: bad lz4 chunk size {cn} at compressed offset {pos} (chunk {k})"));
            }
            let block = r.take(cn);
            lz4_block(&block, &mut hist).map_err(|err| format!("restart: {err} in chunk {k} at {pos}"))?;
            pos += 2 + cn;
            k += 1;
        }
        let end = hist.len().min(dict_len + want);
        return Ok(hist[dict_len..end].to_vec());
    }
    let mut schedule: Vec<(usize, usize, u32)> = Vec::new();
    let mut last_err = String::new();
    for _round in 0..512 {
        let raw = decrypt_scheduled_ordered(data, base, key, version, n, &schedule);
        let (plain, chunk_starts, status) = lz4_stream(&raw, want);
        if status.is_ok() {
            return Ok(plain);
        }
        last_err = status.err().unwrap_or_default();
        let points = dummy_write_points(&plain, e.class_id);
        // (compressed fold position, plain offset, class); a start whose chunk
        // has not been decoded yet cannot be placed — the next round will
        let mut mapped: Vec<(usize, usize, u32)> = points.iter().filter_map(|&(p, c)| fold_position(p, &chunk_starts).map(|off| (off, p, c))).collect();
        mapped.sort();
        let Some(first_new) = mapped.iter().find(|m| !schedule.contains(m)) else { break };
        // bytes before the first unscheduled fold's boundary were decrypted
        // right, so every start folding up to that boundary is real
        let boundary = (first_new.0 + 0xFF) & !0xFF;
        let before = schedule.len();
        let fresh: Vec<(usize, usize, u32)> = mapped.iter().filter(|m| m.0 <= boundary && !schedule.contains(m)).copied().collect();
        schedule.extend(fresh);
        if schedule.len() == before {
            break;
        }
        schedule.sort();
    }
    // The table knows no node of this file (a table-less class, e.g. the
    // VegetTreeModels): find the folds by search — the compressed stream
    // itself says which sequence decodes each chunk — and remember them in
    // a cache next to the user's home (one hunt per file, ~1 s to minutes).
    if let Some(folds) = cached_folds(e) {
        if folds.is_empty() {
            // a hunt that failed before: not repeated (MAPGEOM_REHUNT=1 retries)
            if std::env::var_os("MAPGEOM_REHUNT").is_none() {
                return Err(format!("{last_err} (a fold hunt failed earlier; MAPGEOM_REHUNT=1 retries)"));
            }
        } else {
            let sched: Vec<(usize, usize, u32)> = folds.iter().enumerate().map(|(i, &(o, c))| (o, i, c)).collect();
            let raw = decrypt_scheduled_ordered(data, base, key, version, n, &sched);
            let (plain, _, status) = lz4_stream(&raw, want);
            if status.is_ok() {
                return Ok(plain);
            }
        }
    }
    if std::env::var_os("MAPGEOM_NO_FOLD_HUNT").is_none() {
        match fold_hunt(data, base - e.offset as usize, e, key, version, 4) {
            Ok(folds) => {
                store_folds(e, &folds);
                let sched: Vec<(usize, usize, u32)> = folds.iter().enumerate().map(|(i, &(o, c))| (o, i, c)).collect();
                let raw = decrypt_scheduled_ordered(data, base, key, version, n, &sched);
                let (plain, _, status) = lz4_stream(&raw, want);
                if status.is_ok() {
                    return Ok(plain);
                }
                last_err = format!("hunt found {} folds but the stream still fails: {}", folds.len(), status.err().unwrap_or_default());
            }
            Err((msg, _)) => {
                // remembered as a failure so the next build does not pay again
                store_folds(e, &[]);
                last_err = format!("{last_err}; fold hunt: {msg}");
            }
        }
    }
    if std::env::var_os("MAPGEOM_LENIENT_LZ4").is_some() {
        let raw = decrypt_scheduled_ordered(data, base, key, version, n, &schedule);
        return Ok(lz4_stream(&raw, want).0);
    }
    Err(format!("{last_err} ({} dummy-write folds scheduled)", schedule.len()))
}

/// The fold cache: `$HOME/.cache/mapgeom/folds/<class>-<name>-<sizes>.tsv`,
/// one `offset<TAB>class` line per fold (hex).
fn fold_cache_path(e: &crate::pak::PakEntry) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dir = std::path::Path::new(&home).join(".cache").join("mapgeom").join("folds");
    let name: String = e.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '_' }).collect();
    Some(dir.join(format!("{:08X}-{name}-{}-{}.tsv", e.class_id, e.compressed_size, e.uncompressed_size)))
}

fn cached_folds(e: &crate::pak::PakEntry) -> Option<Vec<(usize, u32)>> {
    let path = fold_cache_path(e)?;
    let text = std::fs::read_to_string(path).ok()?;
    let mut out = Vec::new();
    for line in text.lines() {
        let (o, c) = line.split_once('\t')?;
        out.push((usize::from_str_radix(o.trim_start_matches("0x"), 16).ok()?, u32::from_str_radix(c, 16).ok()?));
    }
    Some(out)
}

fn store_folds(e: &crate::pak::PakEntry, folds: &[(usize, u32)]) {
    let Some(path) = fold_cache_path(e) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text: String = folds.iter().map(|(o, c)| format!("{o:#x}\t{c:08X}\n")).collect();
    let _ = std::fs::write(path, text);
}

/// The fold value a class id produces: `initialize` ORs 0xAA into every
/// byte, so only bits 0/2/4/6 of each byte reach the cipher.
pub fn fold_canonical(class: u32) -> u32 {
    class & 0x5555_5555
}

/// Which dummy-write folds make a compressed pak file decode, found chunk by
/// chunk: the folds of every node whose body starts inside plain chunk k land
/// on the cipher at the compressed position where chunk k ends (the block
/// compressor emits a chunk whole; the dummy writes of the bodies inside it
/// reach the crypt stream after it), i.e. at the first 0x100 boundary at or
/// after the next chunk's start. So: decrypt with the folds so far, find the
/// first chunk that fails, and at its start boundary try every sequence of up
/// to `max_len` parent-class fold values (the canonical forms of the classes
/// the parents table knows — 4 bytes fold as `byte | 0xAA`, so many ids share
/// one value); the sequence that decodes the whole failing chunk is kept.
/// Measured on `Stadium\Media\VegetTreeModel\PalmTreeSmall.VegetTreeModel.Gbx`:
/// chunk 1 needed [CPlugVisualIndexed-like, CMwNod-like] at 0xa00 and then
/// chunks 1–2 decoded whole. Returns the schedule as (compressed offset,
/// class) pairs; `Ok` when the whole file decodes.
pub fn fold_hunt(data: &[u8], header_max_size: usize, e: &crate::pak::PakEntry, key: &[u8; 16], version: i32, max_len: usize) -> Result<Vec<(usize, u32)>, (String, Vec<(usize, u32)>)> {
    let base = header_max_size + e.offset as usize;
    let n = e.compressed_size.max(0) as usize;
    let want = e.uncompressed_size.max(0) as usize;
    // the alphabet: canonical fold values of every known class (and the
    // roots), or the whole 512-value space with MAPGEOM_FOLD_ALPHABET
    let alphabet: Vec<u32> = if std::env::var_os("MAPGEOM_FOLD_ALPHABET").is_some() {
        let mut v = Vec::new();
        for b3 in [0x00u32, 0x01, 0x04, 0x05, 0x10, 0x11, 0x14, 0x15] {
            for b2 in 0..16u32 {
                let b2v = (b2 & 1) | ((b2 & 2) << 1) | ((b2 & 4) << 2) | ((b2 & 8) << 3);
                for b1 in [0x00u32, 0x10, 0x40, 0x50] {
                    v.push((b3 << 24) | (b2v << 16) | (b1 << 8));
                }
            }
        }
        v
    } else {
        let mut v: Vec<u32> = crate::parents::known_classes().into_iter().map(fold_canonical).collect();
        v.sort();
        v.dedup();
        v
    };
    let mut schedule: Vec<(usize, usize, u32)> = Vec::new();
    // MAPGEOM_FOLD_SEED=off:class,off:class,… (hex): folds already known
    if let Ok(seed) = std::env::var("MAPGEOM_FOLD_SEED") {
        for (i, entry) in seed.split(',').filter(|s| !s.is_empty()).enumerate() {
            if let Some((o, c)) = entry.split_once(':') {
                let off = usize::from_str_radix(o.trim_start_matches("0x"), 16).map_err(|e| (format!("seed offset {o}: {e}"), Vec::new()))?;
                let class = u32::from_str_radix(c.trim_start_matches("0x"), 16).map_err(|e| (format!("seed class {c}: {e}"), Vec::new()))?;
                schedule.push((off, i, class));
            }
        }
        schedule.sort();
    }
    let decode = |schedule: &[(usize, usize, u32)], upto: usize| -> (Vec<u8>, Vec<usize>, bool, usize) {
        let raw = decrypt_scheduled_ordered(data, base, key, version, upto.min(n), schedule);
        let (p, s, st, reach) = lz4_stream_reach(&raw, want);
        (p, s, st.is_ok(), reach)
    };
    let mut tries = 0usize;
    eprintln!("alphabet of {} fold values, sequences up to {max_len}", alphabet.len());
    // MAPGEOM_FOLD_BUDGET: tries per file before the hunt gives up (a species
    // whose folds the alphabet cannot express must not cost every build minutes)
    let budget: usize = std::env::var("MAPGEOM_FOLD_BUDGET").ok().and_then(|v| v.parse().ok()).unwrap_or(300_000);
    for _round in 0..256 {
        let (plain, starts, ok, reach0) = decode(&schedule, n);
        if ok {
            eprintln!("decodes: {} plain bytes, {} folds, {tries} tries", plain.len(), schedule.len());
            return Ok(schedule.iter().map(|&(o, _, c)| (o, c)).collect());
        }
        // the failing chunk (not pushed by the decoder) begins at the last start
        let f = starts.len() - 1;
        let chunk_start = *starts.last().unwrap_or(&0);
        let b = (chunk_start + 0xFF) & !0xFF;
        if b >= n {
            return Err((format!("chunk {f} starts at {chunk_start:#x}, past the stream"), schedule.iter().map(|&(o, _, c)| (o, c)).collect()));
        }
        let already = schedule.iter().filter(|s| s.0 == b).count();
        // the cipher state at `b` with the folds so far (all of them lie at
        // or before `b`), and the decrypted bytes from the failing chunk's
        // start up to `b`
        let mut r0 = crate::pak::CipherReader::new(data, base, key, version);
        let mut pos = 0usize;
        let mut head: Vec<u8> = Vec::with_capacity(b);
        for &(off, _, class) in &schedule {
            let off = off.min(b);
            if off > pos {
                head.extend(r0.take(off - pos));
                pos = off;
            }
            r0.initialize(&class.to_le_bytes(), 0, 4);
        }
        if b > pos {
            head.extend(r0.take(b - pos));
        }
        let seg_head: Vec<u8> = head[chunk_start..].to_vec();
        // plain history up to the failing chunk
        let mut hist0: Vec<u8> = Vec::with_capacity(LZ4_DICT.len() + plain.len() + 8200);
        hist0.extend_from_slice(LZ4_DICT);
        let dict_len = hist0.len();
        hist0.extend_from_slice(&plain[..(4096 * f).min(plain.len())]);
        let limit = (b + 2 * (4128 + 2)).min(n) - b;
        let mut found: Option<(Vec<u32>, usize, usize)> = None;
        // the values seen so far in this file (and the two every tree model
        // used: CPlugVisualIndexed-like 01040000 and CMwNod/CPlug-like
        // 01001000) first, in longer sequences; the whole alphabet after
        let mut pref: Vec<u32> = vec![0x0104_0000, 0x0100_1000];
        pref.extend(schedule.iter().map(|s| s.2));
        pref.sort();
        pref.dedup();
        let stages: [(&Vec<u32>, usize); 2] = [(&pref, max_len.max(6)), (&alphabet, max_len.max(1))];
        'stages: for (alpha, klimit) in stages {
        for k in 1..=klimit {
            let m = alpha.len();
            let mut idx = vec![0usize; k];
            loop {
                tries += 1;
                if tries > budget {
                    return Err((format!("fold hunt over budget ({budget} tries) at chunk {f}"), schedule.iter().map(|&(o, _, c)| (o, c)).collect()));
                }
                let mut r = r0.clone();
                for &ai in &idx {
                    r.initialize(&alpha[ai].to_le_bytes(), 0, 4);
                }
                let mut seg = seg_head.clone();
                seg.extend(r.take(limit));
                let mut hist = hist0.clone();
                let (s2, _, r2) = lz4_continue(&seg, 0, &mut hist, dict_len, (4096 * (f + 1)).min(want));
                // the whole failing chunk decodes (a second start recorded)
                // to exactly 4096 plain bytes (every chunk but the last is
                // full: garbage that parses to the end of a block never
                // lands on 4096), or the file completes
                let produced = hist.len() - dict_len - (4096 * f).min(plain.len());
                let whole = s2.len() == 2 && (produced == 4096 || hist.len() - dict_len >= want);
                if whole && chunk_start + r2 > reach0 {
                    found = Some((idx.iter().map(|&ai| alpha[ai]).collect(), hist.len() - dict_len, chunk_start + r2));
                    break 'stages;
                }
                // next tuple
                let mut pos = k;
                let mut wrapped = false;
                loop {
                    if pos == 0 {
                        wrapped = true;
                        break;
                    }
                    pos -= 1;
                    idx[pos] += 1;
                    if idx[pos] < m {
                        break;
                    }
                    idx[pos] = 0;
                }
                if wrapped {
                    break;
                }
            }
        }
        }
        match found {
            Some((folds, l2, r2)) => {
                eprintln!("  chunk {f} boundary {b:#x}: fold {} -> {l2} plain bytes, reach {r2:#x} (was {} / {reach0:#x}); {tries} tries so far", folds.iter().map(|c| format!("{c:08X}")).collect::<Vec<_>>().join(" "), plain.len());
                for (i, c) in folds.iter().enumerate() {
                    schedule.push((b, already + i, *c));
                }
                schedule.sort();
            }
            None => {
                let msg = format!("chunk {f} (compressed {chunk_start:#x}..): no sequence of up to {max_len} folds at {b:#x} decodes it ({} plain bytes, {tries} tries)", plain.len());
                return Err((msg, schedule.iter().map(|&(o, _, c)| (o, c)).collect()));
            }
        }
    }
    Err(("too many rounds".into(), schedule.iter().map(|&(o, _, c)| (o, c)).collect()))
}

/// The running state of the `MAPGEOM_LZ4_RESYNC` probe: a cipher that is
/// re-synchronised at the first 0x100 boundary after every chunk.
struct Resync<'a> {
    r: crate::pak::CipherReader<'a>,
    cpos: usize,
    produced: usize,
    resync_at: Option<usize>,
}

impl<'a> Resync<'a> {
    /// `m` decrypted bytes, re-synchronising on the way when the point falls inside.
    fn take(&mut self, data: &'a [u8], base: usize, key: &[u8; 16], version: i32, mode: &str, skip: usize, m: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(m);
        let mut left = m;
        while left > 0 {
            let upto = self.resync_at.map(|s| s.saturating_sub(self.produced)).unwrap_or(usize::MAX);
            if upto == 0 {
                if mode == "newiv" {
                    self.r = crate::pak::CipherReader::new_at(data, self.cpos, self.cpos + 8, key, version);
                    self.cpos += 8;
                } else if mode == "restart" {
                    self.r = crate::pak::CipherReader::new_at(data, base, self.cpos, key, version);
                } else {
                    // K bytes decrypted and dropped, the chain kept
                    let _ = self.r.take(skip);
                    self.cpos += skip;
                    self.produced += skip;
                }
                self.resync_at = None;
                continue;
            }
            let t = left.min(upto);
            out.extend(self.r.take(t));
            self.produced += t;
            self.cpos += t;
            left -= t;
        }
        out
    }
}
