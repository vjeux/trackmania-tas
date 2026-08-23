//! Nadeo pak file-data reader: blowfish (optional) + the chunked LZ4 stream
//! with the 1006-byte built-in dictionary.
use crate::lz4dict::LZ4_DICT;

/// Decode one LZ4 block into `hist` (which already holds the dictionary and
/// everything decoded so far). Returns the number of bytes produced.
fn lz4_block(src: &[u8], hist: &mut Vec<u8>) -> Result<usize, String> {
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
        lz4_block(&raw[i..i + n], &mut hist)?;
        i += n;
    }
    Ok(hist[dict_len..dict_len + want].to_vec())
}
