//! `HAULPACK` — a self-describing archive of the state tree, and base64.
//!
//! Why not `tar`: this file has to be reconstructable by a fresh box that has
//! nothing but the repo and an x509 cert, and it travels as the body of a
//! Phabricator paste. A format whose header a human can read in the paste UI,
//! with a per-file md5 already in it, is worth more here than a standard one.
//!
//! ```text
//! HAULPACK 1
//! generated <iso8601> node=<node> files=<n>
//! FILE <path> <bytes> <md5>
//! <base64 of the file, one line>
//! ...
//! END <md5-of-the-whole-manifest>
//! ```
//!
//! Every file carries its md5 *inside the archive*, so "did this arrive
//! intact" is answerable from the archive alone — the check does not depend on
//! the box that wrote it still existing.

use crate::md5::md5_hex;
use std::path::{Path, PathBuf};

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut rev = [255u8; 256];
    for (i, c) in B64.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let clean: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        if chunk.len() < 2 {
            return Err("truncated base64".into());
        }
        let mut n: u32 = 0;
        let mut pad = 0;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                pad += 1;
                n <<= 6;
                continue;
            }
            let v = rev[c as usize];
            if v == 255 {
                return Err(format!("bad base64 byte {:?} at group {i}", c as char));
            }
            n = (n << 6) | v as u32;
        }
        for _ in chunk.len()..4 {
            n <<= 6;
            pad += 1;
        }
        let b = n.to_be_bytes();
        out.push(b[1]);
        if pad < 2 {
            out.push(b[2]);
        }
        if pad < 1 {
            out.push(b[3]);
        }
    }
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, out)?;
        } else if p.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

/// Pack every file under `dir`, with paths relative to `rel_root`.
pub fn pack(dir: &Path, rel_root: &Path, node: &str) -> std::io::Result<String> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    let mut body = String::new();
    let mut manifest = String::new();
    for f in &files {
        let bytes = std::fs::read(f)?;
        let rel = f.strip_prefix(rel_root).unwrap_or(f).to_string_lossy().to_string();
        let line = format!("FILE {rel} {} {}\n", bytes.len(), md5_hex(&bytes));
        manifest.push_str(&line);
        body.push_str(&line);
        body.push_str(&b64_encode(&bytes));
        body.push('\n');
    }
    Ok(format!(
        "HAULPACK 1\ngenerated {} node={node} files={}\n{body}END {}\n",
        crate::time::iso(crate::time::now()),
        files.len(),
        md5_hex(manifest.as_bytes())
    ))
}

#[derive(Debug, Clone)]
pub struct Unpacked {
    pub files: Vec<(String, Vec<u8>)>,
    pub generated: String,
    pub node: String,
}

/// Unpack, verifying every file's md5 and the manifest digest.
///
/// A mismatch is an error. It is never a warning and never a skipped file: an
/// archive that half-restores is how a recovery quietly produces a state that
/// never existed.
pub fn unpack(text: &str) -> Result<Unpacked, String> {
    let mut lines = text.lines();
    let header = lines.next().ok_or("empty pack")?;
    if header.trim() != "HAULPACK 1" {
        return Err(format!("not a HAULPACK 1 archive: {header:?}"));
    }
    let meta = lines.next().ok_or("pack has no metadata line")?;
    let generated = meta
        .split_whitespace()
        .nth(1)
        .unwrap_or("unknown")
        .to_string();
    let node = meta
        .split_whitespace()
        .find_map(|t| t.strip_prefix("node="))
        .unwrap_or("unknown")
        .to_string();

    let mut files = Vec::new();
    let mut manifest = String::new();
    let mut end_digest = None;
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("END ") {
            end_digest = Some(rest.trim().to_string());
            break;
        }
        let Some(rest) = line.strip_prefix("FILE ") else {
            return Err(format!("unexpected line in pack: {line:?}"));
        };
        let mut parts = rest.rsplitn(3, ' ');
        let md5 = parts.next().ok_or("FILE line has no md5")?.to_string();
        let len: usize = parts
            .next()
            .ok_or("FILE line has no length")?
            .parse()
            .map_err(|_| "FILE length is not a number")?;
        let path = parts.next().ok_or("FILE line has no path")?.to_string();
        if path.starts_with('/') || path.split('/').any(|c| c == "..") {
            return Err(format!("pack contains an escaping path: {path:?}"));
        }
        let data = b64_decode(lines.next().ok_or("truncated in transit: a FILE header with no body")?)?;
        if data.len() != len {
            return Err(format!("{path}: {} bytes, the pack declares {len}", data.len()));
        }
        let got = md5_hex(&data);
        if got != md5 {
            return Err(format!("{path}: md5 {got} but the pack declares {md5}"));
        }
        manifest.push_str(&format!("FILE {path} {len} {md5}\n"));
        files.push((path, data));
    }
    let end = end_digest.ok_or("pack has no END line — it was truncated in transit")?;
    let got = md5_hex(manifest.as_bytes());
    if got != end {
        return Err(format!("manifest digest {got} but the pack declares {end}"));
    }
    Ok(Unpacked { files, generated, node })
}

pub fn restore(u: &Unpacked, into: &Path) -> std::io::Result<usize> {
    for (rel, data) in &u.files {
        let dest = into.join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(dest, data)?;
    }
    Ok(u.files.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(b64_encode(b""), "");
        assert_eq!(b64_encode(b"f"), "Zg==");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
        assert_eq!(b64_encode(b"foo"), "Zm9v");
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_round_trips_every_byte_value() {
        let all: Vec<u8> = (0..=255u8).collect();
        for n in 0..=64usize {
            let slice = &all[..n.min(all.len())];
            assert_eq!(b64_decode(&b64_encode(slice)).unwrap(), slice, "n={n}");
        }
        assert_eq!(b64_decode(&b64_encode(&all)).unwrap(), all);
    }

    fn tmpdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("haul-pack-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn a_tree_round_trips_through_a_pack() {
        let src = tmpdir("src");
        std::fs::create_dir_all(src.join("state/journal")).unwrap();
        std::fs::write(src.join("state/journal/a.rec"), "2026-08-24T00:00:00Z\tstart\n").unwrap();
        std::fs::write(src.join("state/frontier/best.bin"), [0u8, 255, 1, 254]).unwrap_or_else(|_| {
            std::fs::create_dir_all(src.join("state/frontier")).unwrap();
            std::fs::write(src.join("state/frontier/best.bin"), [0u8, 255, 1, 254]).unwrap()
        });
        let text = pack(&src, &src, "boxA").unwrap();
        let u = unpack(&text).unwrap();
        assert_eq!(u.files.len(), 2);
        let dst = tmpdir("dst");
        restore(&u, &dst).unwrap();
        assert_eq!(
            std::fs::read(dst.join("state/frontier/best.bin")).unwrap(),
            vec![0u8, 255, 1, 254],
            "binary content must survive — a tape is not text"
        );
    }

    #[test]
    fn a_corrupted_body_is_refused_rather_than_half_restored() {
        let src = tmpdir("corrupt");
        std::fs::write(src.join("x.rec"), "hello").unwrap();
        let text = pack(&src, &src, "boxA").unwrap();
        let broken = text.replace(&b64_encode(b"hello"), &b64_encode(b"hellO"));
        assert_ne!(broken, text);
        let e = unpack(&broken).unwrap_err();
        assert!(e.contains("md5"), "{e}");
    }

    #[test]
    fn a_truncated_pack_is_refused() {
        // The realistic transport failure: a paste that got cut off.
        let src = tmpdir("trunc");
        std::fs::write(src.join("x.rec"), "hello").unwrap();
        let text = pack(&src, &src, "boxA").unwrap();
        let cut = &text[..text.len() * 3 / 5];
        let e = unpack(cut).unwrap_err();
        assert!(e.contains("truncated") || e.contains("END"), "{e}");
    }

    #[test]
    fn a_pack_cannot_write_outside_its_destination() {
        let evil = "HAULPACK 1\ngenerated x node=y files=1\nFILE ../../escape 1 x\nAA==\nEND x\n";
        assert!(unpack(evil).unwrap_err().contains("escaping"));
    }
}
