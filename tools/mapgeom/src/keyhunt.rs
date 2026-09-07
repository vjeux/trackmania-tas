//! `mapgeom pak-keyhunt DUMP PAK...` — recover a client pack's Blowfish key
//! from a full memory dump of the running game.
//!
//! The key is not in the pak and not in the exe: the client derives it at load
//! and keeps it in memory next to the pack's bookkeeping. The pack's own
//! 32-byte SHA-256 (public header, bytes 12..44) is a string the client also
//! keeps, so every occurrence of it in the dump anchors a search window; each
//! 4-byte-aligned 16-byte window nearby is tried as the key (and as the
//! header-XORed form) against the pack's private header, whose first fields
//! are only sane under the right key. A hit is confirmed by a full header
//! parse. Falls back to a whole-dump scan of the 16-byte-aligned windows when
//! no anchored window works (slow: minutes per GB).
use crate::pak::{read_pak, CipherReader, HEADER_KEY};

struct Target {
    name: String,
    data: Vec<u8>,
    version: i32,
    enc_start: usize,
    sha: [u8; 32],
}

fn plausible(t: &Target, key: &[u8; 16]) -> bool {
    let mut kh = *key;
    for i in 0..16 {
        kh[i] ^= HEADER_KEY[i];
    }
    let mut r = CipherReader::new(&t.data, t.enc_start, &kh, t.version);
    let _md5 = r.take(16);
    let gbx_headers_start = r.u32() as usize;
    if t.version < 15 {
        let _ = r.i32();
    }
    let gbx_headers_size = r.i32();
    let gbx_headers_compr_size = r.i32();
    if gbx_headers_start > t.data.len() || gbx_headers_size <= 0 || gbx_headers_compr_size <= 0 || gbx_headers_compr_size > gbx_headers_size + 0x1000 {
        return false;
    }
    if t.version >= 14 {
        let _ = r.take(16);
        if t.version >= 16 {
            let _ = r.u32();
        }
    }
    let _ = r.take(16);
    let flags = r.u32();
    let num_folders = r.i32();
    flags < 0x100 && (1..100_000).contains(&num_folders)
}

fn confirm(t: &Target, key: &[u8; 16]) -> Option<usize> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(|| read_pak(&t.data, t.enc_start, t.version, key).entries.len());
    std::panic::set_hook(prev);
    r.ok()
}

fn hex(k: &[u8]) -> String {
    k.iter().map(|b| format!("{b:02X}")).collect()
}

fn find_all(hay: &[u8], needle: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    if needle.is_empty() || hay.len() < needle.len() {
        return out;
    }
    let first = needle[0];
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        match hay[i..].iter().position(|b| *b == first) {
            Some(p) => {
                let at = i + p;
                if hay[at..].starts_with(needle) {
                    out.push(at);
                }
                i = at + 1;
            }
            None => break,
        }
    }
    out
}

pub fn run(rest: &[String]) -> Result<(), String> {
    let dump_path = rest.get(1).ok_or("pak-keyhunt DUMP PAK... [--window BYTES] [--full]")?;
    let window: usize = rest.iter().position(|a| a == "--window").and_then(|i| rest.get(i + 1)).and_then(|w| w.parse().ok()).unwrap_or(65536);
    let full = rest.iter().any(|a| a == "--full");
    let mut targets = Vec::new();
    for p in rest.iter().skip(2).filter(|a| !a.starts_with("--") && a.ends_with(".pak")) {
        let data = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
        if data.len() < 0x95 || &data[0..8] != b"NadeoPak" {
            return Err(format!("{p}: not a NadeoPak"));
        }
        let version = i32::from_le_bytes(data[8..12].try_into().unwrap());
        let enc_start = crate::store::pak_encrypted_header_start(&data, version)?;
        let mut sha = [0u8; 32];
        sha.copy_from_slice(&data[12..44]);
        println!("{p}: version {version}, private header at {enc_start:#x}, sha256 {}", hex(&sha));
        targets.push(Target { name: p.clone(), data, version, enc_start, sha });
    }
    if targets.is_empty() {
        return Err("no .pak given".into());
    }
    // The dump is streamed (a full client dump is 14 GB): checksum hits are
    // found chunk by chunk, then each window around a hit is re-read.
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(dump_path).map_err(|e| format!("{dump_path}: {e}"))?;
    let total = f.metadata().map_err(|e| e.to_string())?.len() as usize;
    println!("{dump_path}: {total} bytes");
    const CHUNK: usize = 64 << 20;
    let mut all_hits: Vec<Vec<usize>> = vec![Vec::new(); targets.len()];
    let mut buf = vec![0u8; CHUNK + 64];
    let mut pos = 0usize;
    while pos < total {
        f.seek(SeekFrom::Start(pos as u64)).map_err(|e| e.to_string())?;
        let want = (CHUNK + 64).min(total - pos);
        let mut got = 0;
        while got < want {
            let n = f.read(&mut buf[got..want]).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            got += n;
        }
        for (ti, t) in targets.iter().enumerate() {
            for h in find_all(&buf[..got], &t.sha) {
                if h < CHUNK || pos + CHUNK >= total {
                    all_hits[ti].push(pos + h);
                }
            }
        }
        pos += CHUNK;
    }
    let read_at = |f: &mut std::fs::File, off: usize, len: usize| -> Vec<u8> {
        let mut v = vec![0u8; len];
        if f.seek(SeekFrom::Start(off as u64)).is_err() { return Vec::new(); }
        let mut got = 0;
        while got < len {
            match f.read(&mut v[got..]) { Ok(0) | Err(_) => break, Ok(n) => got += n }
        }
        v.truncate(got);
        v
    };
    let mut found: Vec<Option<[u8; 16]>> = vec![None; targets.len()];
    for (ti, t) in targets.iter().enumerate() {
        let hits = all_hits[ti].clone();
        println!("{}: {} occurrence(s) of the pack checksum in the dump", t.name, hits.len());
        let mut tried = std::collections::HashSet::new();
        'outer: for h in &hits {
            let lo = h.saturating_sub(window) & !3;
            let region = read_at(&mut f, lo, 2 * window + 64);
            let hi = lo + region.len().saturating_sub(16);
            let mut off = lo;
            while off <= hi {
                let mut k = [0u8; 16];
                k.copy_from_slice(&region[off - lo..off - lo + 16]);
                if tried.insert(k) {
                    for cand in [k, { let mut x = k; for i in 0..16 { x[i] ^= HEADER_KEY[i]; } x }] {
                        if plausible(t, &cand) {
                            if let Some(n) = confirm(t, &cand) {
                                println!("  KEY {} ({} entries) at dump offset {off:#x}, {} bytes from the checksum at {h:#x}", hex(&cand), n, off as i64 - *h as i64);
                                found[ti] = Some(cand);
                                break 'outer;
                            }
                        }
                    }
                }
                off += 4;
            }
        }
        if found[ti].is_none() && full {
            println!("  no anchored hit; scanning the whole dump at 16-byte alignment...");
            let mut pos = 0usize;
            'full: while pos < total {
                let chunk = read_at(&mut f, pos, CHUNK);
                let mut off = 0usize;
                while off + 16 <= chunk.len() {
                    let mut k = [0u8; 16];
                    k.copy_from_slice(&chunk[off..off + 16]);
                    if k != [0u8; 16] && plausible(t, &k) {
                        if let Some(n) = confirm(t, &k) {
                            println!("  KEY {} ({} entries) at dump offset {:#x}", hex(&k), n, pos + off);
                            found[ti] = Some(k);
                            break 'full;
                        }
                    }
                    off += 16;
                }
                pos += CHUNK;
            }
        }
        if found[ti].is_none() {
            println!("  no key found for {}", t.name);
        }
    }
    println!();
    for (t, k) in targets.iter().zip(found.iter()) {
        match k {
            Some(k) => println!("--pak {}:{}", t.name, hex(k)),
            None => println!("# {}: NOT FOUND", t.name),
        }
    }
    Ok(())
}
