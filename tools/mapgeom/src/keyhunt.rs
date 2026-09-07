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
    /// (slot offset, expected first plaintext bytes) of the first encrypted
    /// entry, when the key is known (`FILE:KEYHEX` argument): a control for
    /// the schedule scan.
    first_encrypted: Option<(usize, Vec<u8>)>,
}

fn plausible(t: &Target, key: &[u8; 16]) -> bool {
    let mut kh = *key;
    for i in 0..16 {
        kh[i] ^= HEADER_KEY[i];
    }
    let bf = crate::blowfish::Blowfish::new(&kh, crate::blowfish::PakCipher::trick_for(t.version));
    plausible_bf(t, bf)
}

/// `plausible` over an already-scheduled header cipher (key XOR HEADER_KEY).
fn plausible_bf(t: &Target, bf: crate::blowfish::Blowfish) -> bool {
    let mut r = CipherReader::with_blowfish(&t.data, t.enc_start, bf, t.version);
    let _md5 = r.take(16);
    let gbx_headers_start = r.u32() as usize;
    if t.version < 15 {
        let _ = r.i32();
    }
    let gbx_headers_size = r.i32();
    let gbx_headers_compr_size = r.i32();
    if gbx_headers_start > t.data.len().max(1 << 31) || gbx_headers_size <= 0 || gbx_headers_compr_size <= 0 || gbx_headers_compr_size > gbx_headers_size + 0x1000 {
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
    let dump_path = rest.get(1).ok_or("pak-keyhunt DUMP PAK... [--window BYTES] [--full [--align N] [--threads N]]")?;
    let window: usize = rest.iter().position(|a| a == "--window").and_then(|i| rest.get(i + 1)).and_then(|w| w.parse().ok()).unwrap_or(65536);
    let full = rest.iter().any(|a| a == "--full");
    let mut targets = Vec::new();
    let mut known: Vec<Option<[u8; 16]>> = Vec::new();
    for arg in rest.iter().skip(2).filter(|a| !a.starts_with("--") && (a.ends_with(".pak") || a.contains(".pak:"))) {
        let (p, key) = match arg.split_once(".pak:") { Some((a, k)) => (format!("{a}.pak"), Some(k.to_string())), None => (arg.clone(), None) };
        let p = &p;
        let data = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
        if data.len() < 0x95 || &data[0..8] != b"NadeoPak" {
            return Err(format!("{p}: not a NadeoPak"));
        }
        let version = i32::from_le_bytes(data[8..12].try_into().unwrap());
        let enc_start = crate::store::pak_encrypted_header_start(&data, version)?;
        let mut sha = [0u8; 32];
        sha.copy_from_slice(&data[12..44]);
        println!("{p}: version {version}, private header at {enc_start:#x}, sha256 {}", hex(&sha));
        let mut t = Target { name: p.clone(), data, version, enc_start, sha, first_encrypted: None };
        let mut kb: Option<[u8; 16]> = None;
        if let Some(k) = key {
            let bytes: Vec<u8> = (0..k.len()).step_by(2).filter_map(|i| u8::from_str_radix(&k[i..i + 2], 16).ok()).collect();
            if bytes.len() == 16 {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&bytes);
                let pak = read_pak(&t.data, t.enc_start, t.version, &arr);
                let hmax = u32::from_le_bytes(t.data[0x30..0x34].try_into().unwrap()) as usize;
                if let Some(e) = pak.entries.iter().find(|e| e.is_encrypted()) {
                    let base = hmax + e.offset as usize;
                    let mut r = CipherReader::new(&t.data, base, &arr, t.version);
                    let plain = r.take(32);
                    println!("  {p}: control entry {} at {base:#x}, plaintext {:02x?}", e.path(), &plain[..8]);
                    t.first_encrypted = Some((base, plain));
                }
                kb = Some(arr);
            }
        }
        known.push(kb);
        targets.push(t);
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
            println!("  (no anchored hit)");
        }
        if found[ti].is_none() {
            println!("  no key found for {}", t.name);
        }
    }
    if rest.iter().any(|a| a == "--schedule") {
        let threads = rest.iter().position(|a| a == "--threads").and_then(|i| rest.get(i + 1)).and_then(|w| w.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
        println!("schedule scan with {threads} threads...");
        schedule_scan(&mut f, total, &targets, &known, threads);
    }
    if full && found.iter().any(|f| f.is_none()) {
        let threads = rest.iter().position(|a| a == "--threads").and_then(|i| rest.get(i + 1)).and_then(|w| w.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
        let align: usize = rest.iter().position(|a| a == "--align").and_then(|i| rest.get(i + 1)).and_then(|w| w.parse().ok()).unwrap_or(16);
        println!("full scan of the dump with {threads} threads at {align}-byte alignment...");
        full_scan(&mut f, total, &targets, &mut found, threads, align);
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

/// Whole-dump scan, every 16-byte-aligned window, all targets at once: one
/// Blowfish schedule per candidate (the header cipher is keyed by
/// key XOR HEADER_KEY, the same for every pack), cloned per target. A cheap
/// entropy filter first: an MD5-shaped key has no byte value four times and
/// at most two zero bytes; pointers, floats and text fail that.
fn full_scan(f: &mut std::fs::File, total: usize, targets: &[Target], found: &mut [Option<[u8; 16]>], threads: usize, align: usize) {
    use std::io::{Read, Seek, SeekFrom};
    const CHUNK: usize = 256 << 20;
    let trick = crate::blowfish::PakCipher::trick_for(targets[0].version);
    let mut pos = 0usize;
    let mut buf = vec![0u8; CHUNK];
    let t0 = std::time::Instant::now();
    while pos < total && found.iter().any(|f| f.is_none()) {
        let want = CHUNK.min(total - pos);
        if f.seek(SeekFrom::Start(pos as u64)).is_err() { break; }
        let mut got = 0;
        while got < want {
            match f.read(&mut buf[got..want]) { Ok(0) | Err(_) => break, Ok(n) => got += n }
        }
        let chunk = &buf[..got];
        let per = (got / threads + align - 1) / align * align;
        let hits: Vec<(usize, [u8; 16], usize)> = std::thread::scope(|s| {
            let mut hs = Vec::new();
            for ti in 0..threads {
                let lo = ti * per;
                let hi = ((ti + 1) * per).min(got);
                if lo >= hi { continue; }
                let found_now: Vec<bool> = found.iter().map(|f| f.is_some()).collect();
                hs.push(s.spawn(move || {
                    let mut out = Vec::new();
                    let mut off = lo;
                    while off + 16 <= hi {
                        let k: [u8; 16] = chunk[off..off + 16].try_into().unwrap();
                        off += align;
                        let mut counts = [0u8; 256];
                        let mut bad = false;
                        for b in k { counts[b as usize] += 1; if counts[b as usize] >= 4 { bad = true; break; } }
                        if bad || counts[0] > 2 { continue; }
                        // candidate as the pack key (schedule on k ^ HEADER_KEY) and as the header key itself
                        for form in 0..2 {
                            let mut kh = k;
                            if form == 0 { for i in 0..16 { kh[i] ^= HEADER_KEY[i]; } }
                            let bf = crate::blowfish::Blowfish::new(&kh, trick);
                            for (i, t) in targets.iter().enumerate() {
                                if found_now[i] { continue; }
                                if plausible_bf(t, bf.clone()) {
                                    let key = if form == 0 { k } else { let mut x = k; for j in 0..16 { x[j] ^= HEADER_KEY[j]; } x };
                                    out.push((i, key, off - align));
                                }
                            }
                        }
                    }
                    out
                }));
            }
            hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        for (i, key, off) in hits {
            if found[i].is_none() {
                if let Some(n) = confirm(&targets[i], &key) {
                    println!("  KEY {} for {} ({} entries) at dump offset {:#x}", hex(&key), targets[i].name, n, pos + off);
                    found[i] = Some(key);
                }
            }
        }
        pos += got;
        println!("  scanned {} MiB in {:.0} s", pos >> 20, t0.elapsed().as_secs_f32());
        if got == 0 { break; }
    }
}

/// Whole-dump scan for a live Blowfish SCHEDULE (the game keeps one per
/// pack for the files; maybe one for the header). Every 4-byte-aligned
/// offset is read as P words followed by the four S boxes (and S boxes
/// followed by P), P as stored and reversed, with 10 or 18 P words. A
/// candidate is tested by decrypting each pack's private header with it
/// (`plausible_bf`) and, for a target with a known key, by decrypting the
/// first bytes of its first encrypted entry. Cheap per candidate (no key
/// schedule to run), so the S-box randomness filter is the only gate.
fn schedule_scan(f: &mut std::fs::File, total: usize, targets: &[Target], known: &[Option<[u8; 16]>], threads: usize) {
    use std::io::{Read, Seek, SeekFrom};
    const CHUNK: usize = 256 << 20;
    const OVERLAP: usize = 4200;
    let trick = crate::blowfish::PakCipher::trick_for(targets[0].version);
    let n = if trick == crate::blowfish::Trick::LittleEndianPak18 { 8 } else { 16 };
    // reference schedules for the known keys: what the layouts should look like
    for (t, k) in targets.iter().zip(known.iter()) {
        if let Some(k) = k {
            let bf = crate::blowfish::Blowfish::new(k, trick);
            let mut kh = *k;
            for i in 0..16 {
                kh[i] ^= HEADER_KEY[i];
            }
            let bh = crate::blowfish::Blowfish::new(&kh, trick);
            if let Ok(path) = std::env::var("KEYHUNT_EMIT_SCHEDULE") {
                // self-test material: both schedules as [P18][S], as-stored order
                let mut out = Vec::new();
                for b in [&bf, &bh] {
                    for w in b.p() { out.extend_from_slice(&w.to_le_bytes()); }
                    for box_ in b.s() { for w in box_ { out.extend_from_slice(&w.to_le_bytes()); } }
                    out.extend_from_slice(&[0u8; 24]);
                }
                std::fs::write(&path, out).ok();
                println!("  wrote {path}");
            }
            println!("  {}: file schedule P[0..4] {:08x} {:08x} {:08x} {:08x}, S0[0..2] {:08x} {:08x}; header schedule P[0..2] {:08x} {:08x}", t.name, bf.p()[0], bf.p()[1], bf.p()[2], bf.p()[3], bf.s()[0][0], bf.s()[0][1], bh.p()[0], bh.p()[1]);
        }
    }
    let mut pos = 0usize;
    let mut buf = vec![0u8; CHUNK + OVERLAP];
    let t0 = std::time::Instant::now();
    let mut hits_total = 0usize;
    while pos < total {
        let want = (CHUNK + OVERLAP).min(total - pos);
        if f.seek(SeekFrom::Start(pos as u64)).is_err() { break; }
        let mut got = 0;
        while got < want {
            match f.read(&mut buf[got..want]) { Ok(0) | Err(_) => break, Ok(n) => got += n }
        }
        let chunk = &buf[..got];
        let limit = if pos + CHUNK >= total { got } else { CHUNK.min(got) };
        let per = (limit / threads + 3) & !3;
        let hits: Vec<String> = std::thread::scope(|s| {
            let mut hs = Vec::new();
            for ti in 0..threads {
                let lo = ti * per;
                let hi = ((ti + 1) * per).min(limit);
                if lo >= hi { continue; }
                hs.push(s.spawn(move || {
                    let mut out = Vec::new();
                    let word = |o: usize| u32::from_le_bytes(chunk[o..o + 4].try_into().unwrap());
                    let mut off = lo;
                    while off + 4200 <= chunk.len() && off < hi {
                        // S-box randomness gate on the words that would be S[0][0..8] in the P-first layouts
                        let mut distinct = [false; 256];
                        let mut nd = 0;
                        for b in &chunk[off + 40..off + 40 + 64] {
                            if !distinct[*b as usize] { distinct[*b as usize] = true; nd += 1; }
                        }
                        if nd < 44 {
                            off += 4;
                            continue;
                        }
                        // layouts: P (18 or 10 words) then S — the struct order of every
                        // Blowfish implementation we know; S-then-P is not tried (cost)
                        for (plen, s_first) in [(18usize, false), (10, false)] {
                            let (p_off, s_off) = if s_first { (off + 4096, off) } else { (off, off + plen * 4) };
                            if s_off + 4096 > chunk.len() || p_off + plen * 4 > chunk.len() { continue; }
                            let mut p = [0u32; 18];
                            for i in 0..plen { p[i] = word(p_off + i * 4); }
                            // cheap pre-test before copying 4 KB of S boxes: with the
                            // real schedule the header's first plaintext word is the
                            // MD5's first word — no constraint; but the FIRST SLOT's
                            // first two bytes are an LZ4 length <= 4128, and the header
                            // test needs the boxes anyway: so copy once per s_off.
                            let mut sb = [[0u32; 256]; 4];
                            for i in 0..4 { for j in 0..256 { sb[i][j] = word(s_off + (i * 256 + j) * 4); } }
                            for rev in [false, true] {
                                let mut pp = p;
                                if rev { pp[0..n + 2].reverse(); }
                                let bf = crate::blowfish::Blowfish::from_raw(pp, sb, n);
                                for (i, t) in targets.iter().enumerate() {
                                    if plausible_bf(t, bf.clone()) {
                                        out.push(format!("HEADER schedule for {} at {:#x} (P{} {} rev {})", t.name, off, plen, if s_first { "after S" } else { "first" }, rev));
                                    }
                                    if let Some(exp) = &t.first_encrypted {
                                        let mut r = CipherReader::with_blowfish(&t.data, exp.0, bf.clone(), t.version);
                                        let got = r.take(exp.1.len());
                                        if got == exp.1 {
                                            out.push(format!("FILE schedule for {} at {:#x} (P{} {} rev {})", t.name, off, plen, if s_first { "after S" } else { "first" }, rev));
                                        }
                                    } else if first_slot_decodes(t, bf.clone()) {
                                        out.push(format!("FILE schedule (first slot decodes as an LZ4 block) for {} at {:#x} (P{} {} rev {})", t.name, off, plen, if s_first { "after S" } else { "first" }, rev));
                                    }
                                    let _ = i;
                                }
                            }
                        }
                        off += 4;
                    }
                    out
                }));
            }
            hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        for h in hits {
            hits_total += 1;
            println!("  {h} (dump offset {:#x} + chunk)", pos);
        }
        pos += limit;
        println!("  schedule scan: {} MiB in {:.0} s, {hits_total} hit(s)", pos >> 20, t0.elapsed().as_secs_f32());
        if got == 0 || limit == 0 { break; }
    }
}

/// Without the header, the one slot whose position is known is the first
/// one (offset 0 = header_max): decrypt its first bytes with the candidate
/// and require a well-formed first LZ4 block — `[u16 len ≤ 4128]` then a
/// block that decodes against the Nadeo dictionary to exactly 4096 bytes (a
/// compressed entry's blocks all do, bar the last). An encrypted+compressed
/// first entry is the common case (BlueBay: Collections\BlueBay.Collection.Gbx).
fn first_slot_decodes(t: &Target, bf: crate::blowfish::Blowfish) -> bool {
    let hmax = u32::from_le_bytes(t.data[0x30..0x34].try_into().unwrap()) as usize;
    if hmax + 8 + 4200 > t.data.len() {
        return false;
    }
    let mut r = CipherReader::with_blowfish(&t.data, hmax, bf, t.version);
    let head = r.take(2);
    let n = u16::from_le_bytes([head[0], head[1]]) as usize;
    if n == 0 || n > 4128 {
        return false;
    }
    let block = r.take(n);
    let mut hist: Vec<u8> = Vec::with_capacity(crate::lz4dict::LZ4_DICT.len() + 4200);
    hist.extend_from_slice(crate::lz4dict::LZ4_DICT);
    let before = hist.len();
    match crate::pakfile::lz4_block(&block, &mut hist) {
        Ok(_) => hist.len() - before == 4096,
        Err(_) => false,
    }
}

/// `mapgeom pak-basekey FILE PAK:KEYHEX...` — a pack key is
/// `MD5(hex_upper(base16) + "NadeoPak")` (GBX.NET `Pak.ComputeKey`; the
/// ModelsSport key checked out that way). Where the 16-byte BASE keys live
/// is the question: scan FILE (the exe, the title pack, a dump) for any
/// 16-byte window whose computed key is one of the known pack keys. A hit
/// names the table; the other packs' bases are its neighbours.
pub fn basekey_scan(rest: &[String]) -> Result<(), String> {
    let path = rest.get(1).ok_or("pak-basekey FILE PAK:KEYHEX...")?;
    let mut wants: Vec<(String, [u8; 16])> = Vec::new();
    for arg in rest.iter().skip(2) {
        if let Some((p, k)) = arg.rsplit_once(':') {
            let bytes: Vec<u8> = (0..k.len()).step_by(2).filter_map(|i| u8::from_str_radix(&k[i..i + 2], 16).ok()).collect();
            if bytes.len() == 16 {
                let mut a = [0u8; 16];
                a.copy_from_slice(&bytes);
                wants.push((p.to_string(), a));
            }
        }
    }
    if wants.is_empty() {
        return Err("no PAK:KEYHEX given".into());
    }
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    println!("{path}: {} bytes; {} known keys", data.len(), wants.len());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let per = data.len() / threads + 1;
    let hits: Vec<String> = std::thread::scope(|s| {
        let mut hs = Vec::new();
        for ti in 0..threads {
            let lo = ti * per;
            let hi = ((ti + 1) * per + 16).min(data.len());
            let data = &data;
            let wants = &wants;
            hs.push(s.spawn(move || {
                let mut out = Vec::new();
                let mut off = lo;
                while off + 16 <= hi {
                    let w = &data[off..off + 16];
                    // an all-zero or text window is not a key
                    if w.iter().all(|b| *b == 0) || w.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
                        off += 1;
                        continue;
                    }
                    let k = crate::md5::compute_key(w);
                    for (name, want) in wants.iter() {
                        if k == *want {
                            out.push(format!("BASE for {name} at {off:#x}: {}", crate::md5::hex_upper(w)));
                        }
                    }
                    off += 1;
                }
                out
            }));
        }
        hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    for h in &hits {
        println!("  {h}");
    }
    if hits.is_empty() {
        println!("  no base key found in {path}");
    }
    Ok(())
}

/// `mapgeom pak-trykeys PAK... --keys FILE` — try every 32-hex BASE key of
/// FILE (one per line: the exe's table, `strings -n 32 Trackmania.exe |
/// grep -E '^[0-9A-F]{32}$'`) as `compute_key(base)` against each pack's
/// private header (the first 4 KB of the pack is enough for the test).
pub fn try_keys(rest: &[String]) -> Result<(), String> {
    let kf = rest.iter().position(|a| a == "--keys").and_then(|i| rest.get(i + 1)).ok_or("pak-trykeys PAK... --keys FILE")?;
    let bases: Vec<[u8; 16]> = std::fs::read_to_string(kf)
        .map_err(|e| format!("{kf}: {e}"))?
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.len() != 32 { return None; }
            let b: Vec<u8> = (0..32).step_by(2).filter_map(|i| u8::from_str_radix(&l[i..i + 2], 16).ok()).collect();
            (b.len() == 16).then(|| { let mut a = [0u8; 16]; a.copy_from_slice(&b); a })
        })
        .collect();
    println!("{} base keys from {kf}", bases.len());
    for p in rest.iter().skip(1).take_while(|a| *a != "--keys") {
        let data = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
        if data.len() < 0x200 || &data[0..8] != b"NadeoPak" {
            println!("{p}: not a NadeoPak");
            continue;
        }
        let version = i32::from_le_bytes(data[8..12].try_into().unwrap());
        let enc_start = crate::store::pak_encrypted_header_start(&data, version)?;
        let mut sha = [0u8; 32];
        sha.copy_from_slice(&data[12..44]);
        let t = Target { name: p.clone(), data, version, enc_start, sha, first_encrypted: None };
        let mut hit = false;
        for b in &bases {
            let k = crate::md5::compute_key(b);
            if plausible(&t, &k) {
                println!("{p}: base {} -> KEY {}", crate::md5::hex_upper(b), crate::md5::hex_upper(&k));
                hit = true;
            }
        }
        if !hit {
            println!("{p}: no base key of the table opens it");
        }
    }
    Ok(())
}
