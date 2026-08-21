//! `tmtraj anon` -- strip a donor's identity out of a ghost body.
//!
//! WHY THIS EXISTS (arm `r165`, 2026-08-20)
//! ----------------------------------------
//! Every ghost in the published tree carries the same identity: one skin pack
//! `Skins\Models\CarSport\TAS.zip`, login `TAS`, no locator URL. On most maps
//! that costs nothing to keep, because regeneration starts FROM the published
//! file, which was anonymised when it was published.
//!
//! 165922 cannot: its nine published files have no telemetry record at all, so
//! the record has to be imported from a human recording of that map, and the
//! import brings the human's body with it -- their login, their clan tag, their
//! zone, their account id, their personal skin and the storage URL that
//! contains their account's object id. `tmtraj body setlogin` replaces the
//! FIRST login-shaped string and its exact repeats, which leaves all of that
//! behind, and `tmtraj hdr setlogin` must not be used on these files at all:
//! their user-data size is zero -- everything is in the BODY -- so the header
//! path misreads the body as a chunk table and produces a longer, corrupt file
//! (measured: 5263 -> 10436 B, and it overwrites the map UID).
//!
//! So this is the body-side anonymiser:
//!   * any `Skins\Models\...` path  -> `Skins\Models\CarSport\TAS.zip`, and the
//!     32-byte checksum in front of it is zeroed;
//!   * any `http...` locator URL    -> the empty string;
//!   * any string named by --ident  -> --name (default `TAS`).
//! Nothing else is touched, so the map UID, the game-version strings and the
//! tape all survive byte for byte. Every enclosing skippable chunk's size is
//! corrected, which is what makes the result loadable.

use crate::gbx::{all_skip_chunks, Gbx};

struct Edit {
    /// offset of the length prefix in the OLD body
    at: usize,
    old_len: usize,
    new: Vec<u8>,
    /// zero the 32 bytes immediately before the length prefix (a skin checksum)
    zero_cksum: bool,
}

pub fn cmd(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let inp = flag("--in").expect("--in GHOST.Ghost.Gbx");
    let out = flag("--out").expect("--out OUT.Ghost.Gbx");
    let name = flag("--name").unwrap_or_else(|| "TAS".into());
    let idents: Vec<String> = flag("--ident")
        .map(|s| s.split(',').map(|v| v.to_string()).filter(|v| !v.is_empty()).collect())
        .unwrap_or_default();
    let keep: Vec<String> = flag("--keep")
        .map(|s| s.split(',').map(|v| v.to_string()).filter(|v| !v.is_empty()).collect())
        .unwrap_or_default();
    let verbose = !args.iter().any(|a| a == "--quiet");

    let data = std::fs::read(&inp).unwrap_or_else(|e| panic!("read {}: {}", inp, e));
    let g = Gbx::parse(&data);
    let body = g.body.clone();
    let g4 = |o: usize| u32::from_le_bytes(body[o..o + 4].try_into().unwrap()) as usize;

    let mut edits: Vec<Edit> = Vec::new();
    let mut p = 0usize;
    // WHERE THE IDENTITY CAN BE: anywhere in the body EXCEPT inside the
    // record's compressed payload, which is not text and must never be scanned
    // as text. This used to be a flat `min(len, 65536)` with the comment "the
    // identity lives in the ghost's own chunks at the front of the body". That
    // is true of the file it was written for and false of 227654, whose record
    // is 76 KB and whose login, trigram and zone all sit after it: the
    // anonymiser replaced the four strings it could see, reported success, and
    // left the account id in the file. A constant that was right once.
    let rec = crate::entrec::find_entrecord_span(&body);
    let lim = body.len();
    while p + 4 <= lim {
        if let Some((rs, re)) = rec {
            if p >= rs && p < re {
                p = re;
                continue;
            }
        }
        let n = g4(p);
        if n > 0 && n < 256 && p + 4 + n <= body.len() {
            if let Ok(s) = std::str::from_utf8(&body[p + 4..p + 4 + n]) {
                if s.chars().all(|c| c.is_ascii_graphic() || c == ' ') && !keep.contains(&s.to_string()) {
                    let mut ed: Option<Edit> = None;
                    if s.contains("Skins\\Models\\") {
                        ed = Some(Edit {
                            at: p,
                            old_len: n,
                            new: b"Skins\\Models\\CarSport\\TAS.zip".to_vec(),
                            zero_cksum: true,
                        });
                    } else if s.starts_with("http") {
                        ed = Some(Edit { at: p, old_len: n, new: Vec::new(), zero_cksum: false });
                    } else if idents.iter().any(|v| v == s) {
                        ed = Some(Edit {
                            at: p,
                            old_len: n,
                            new: name.as_bytes().to_vec(),
                            zero_cksum: false,
                        });
                    }
                    if let Some(e) = ed {
                        if verbose {
                            println!(
                                "  body@{:<6} {:?} -> {:?}{}",
                                p,
                                s,
                                String::from_utf8_lossy(&e.new),
                                if e.zero_cksum { "  (checksum zeroed)" } else { "" }
                            );
                        }
                        edits.push(e);
                    }
                    p += 4 + n;
                    continue;
                }
            }
        }
        p += 1;
    }
    if edits.is_empty() {
        println!("nothing to anonymise in {}", inp);
        return;
    }

    // rebuild the body
    let mut nb: Vec<u8> = Vec::with_capacity(body.len() + 64);
    let mut last = 0usize;
    for e in &edits {
        let mut upto = e.at;
        if e.zero_cksum && e.at >= 32 {
            upto = e.at - 32;
        }
        nb.extend_from_slice(&body[last..upto]);
        if e.zero_cksum && e.at >= 32 {
            nb.extend_from_slice(&[0u8; 32]);
        }
        nb.extend_from_slice(&(e.new.len() as u32).to_le_bytes());
        nb.extend_from_slice(&e.new);
        last = e.at + 4 + e.old_len;
    }
    nb.extend_from_slice(&body[last..]);

    // every skippable chunk that contained an edit must have its declared size
    // corrected, and its own header may itself have moved.
    let skips = all_skip_chunks(&body);
    for (_, coff, poff, sz) in &skips {
        let inside: i64 = edits
            .iter()
            .filter(|e| e.at >= *poff && e.at < *poff + *sz)
            .map(|e| e.new.len() as i64 - e.old_len as i64)
            .sum();
        if inside == 0 {
            continue;
        }
        let shift: i64 = edits
            .iter()
            .filter(|e| e.at < *coff)
            .map(|e| e.new.len() as i64 - e.old_len as i64)
            .sum();
        let at = (*coff as i64 + shift) as usize;
        let cur = u32::from_le_bytes(nb[at + 8..at + 12].try_into().unwrap()) as i64;
        nb[at + 8..at + 12].copy_from_slice(&((cur + inside) as u32).to_le_bytes());
    }

    let mut file = g.header_bytes_u();
    file.extend_from_slice(&nb);
    std::fs::write(&out, &file).unwrap_or_else(|e| panic!("write {}: {}", out, e));
    println!(
        "anon: wrote {} ({} -> {} bytes, body {} -> {} B, {} string(s) replaced)",
        out,
        data.len(),
        file.len(),
        body.len(),
        nb.len(),
        edits.len()
    );
}
