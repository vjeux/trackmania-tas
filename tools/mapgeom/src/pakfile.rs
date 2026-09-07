//! Nadeo pak file-data reader: blowfish (optional) + the chunked LZ4 stream
//! with the 1006-byte built-in dictionary.
use crate::lz4dict::LZ4_DICT;

/// Decode one LZ4 block into `hist` (which already holds the dictionary and
/// everything decoded so far). Returns the number of bytes produced.
pub fn lz4_block(src: &[u8], hist: &mut Vec<u8>) -> Result<usize, String> {
    let start = hist.len();
    let mut i = 0usize;
    while i < src.len() {
        let token = src[i];
        i += 1;
        let mut lit = (token >> 4) as usize;
        if lit == 15 {
            loop {
                if i >= src.len() {
                    return Err("literal length overrun".into());
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
            return Err("literal overrun".into());
        }
        hist.extend_from_slice(&src[i..i + lit]);
        i += lit;
        if i == src.len() {
            break; // last sequence has no match part
        }
        if i + 2 > src.len() {
            return Err("offset overrun".into());
        }
        let off = u16::from_le_bytes([src[i], src[i + 1]]) as usize;
        i += 2;
        if off == 0 || off > hist.len() {
            // A malformed final sequence: keep what decoded so far when the
            // caller asked for leniency (a partial body still yields strings).
            if std::env::var_os("MAPGEOM_LENIENT_LZ4").is_some() {
                return Ok(hist.len() - start);
            }
            return Err(format!("bad match offset {} (hist {})", off, hist.len()));
        }
        let mut mlen = (token & 0xF) as usize;
        if mlen == 15 {
            loop {
                if i >= src.len() {
                    return Err("match length overrun".into());
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
    let mut out = vec![(body_start, crate::parents::dummy_write_class(class_id))];
    let externals: Vec<(u32, String)> = g.refs.iter().map(|e| (e.node_index, e.name.clone())).collect();
    let mut graph = crate::node::Graph::new(&g.body, g.num_nodes, &externals);
    let _ = graph.node_body(class_id);
    for (off, c) in &graph.node_starts {
        // the main node's own start is `body_start` (its class id sits in
        // the header, not in the body)
        if *off == 0 {
            continue;
        }
        out.push((body_start + off, crate::parents::dummy_write_class(*c)));
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
