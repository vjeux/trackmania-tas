//! `tmtraj setdecl` -- rewrite a ghost's DECLARED race time at every site.
//!
//! WHY (arm `r165`, 2026-08-20, following arm `hl`'s hdrfix of 2026-08-19)
//! ----------------------------------------------------------------------
//! The declared time is not one field. `hl` measured it in four chunks and five
//! or six sites:
//!
//!     0x03092005  the race time proper
//!     0x0309200B
//!     0x0309201B
//!     0x0309202B  the checkpoint-splits chunk (its LAST split is the finish)
//!
//! Rewriting only 0x03092005 leaves five copies of the old value and makes the
//! file disagree with itself in a new way.
//!
//! A file that imports its telemetry record from a human recording also imports
//! that recording's declared time: on 165922 the container declares 8790.769 s,
//! so every regenerated file would claim a 2.4-hour lap. This sets them all.
//!
//! HOW, and what it refuses to do. It replaces the exact 4-byte little-endian
//! value of the OLD declared time, and ONLY inside those four chunks. The
//! compressed telemetry payload (`CPlugEntRecordData`) is never scanned, which
//! is the trap a blind whole-file substitution walks into: four bytes of a zlib
//! stream that happen to match are not a time, and rewriting them corrupts the
//! record. Every site it changes is printed, with its chunk and offset.

use crate::gbx::{all_skip_chunks, Gbx};

const SITES: [u32; 4] = [0x0309_2005, 0x0309_200B, 0x0309_201B, 0x0309_202B];

pub fn cmd(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let inp = flag("--in").expect("--in GHOST.Ghost.Gbx");
    let out = flag("--out").expect("--out OUT.Ghost.Gbx");
    let new_ms: u32 = flag("--ms").expect("--ms N").parse().expect("--ms N");

    let data = std::fs::read(&inp).unwrap_or_else(|e| panic!("read {}: {}", inp, e));
    let g = Gbx::parse(&data);
    let mut body = g.body.clone();
    let skips = all_skip_chunks(&body);

    // the current declared time: chunk 0x03092005, first four bytes
    let old_ms: u32 = match flag("--from") {
        Some(v) => v.parse().expect("--from N"),
        None => match skips.iter().find(|c| c.0 == 0x0309_2005) {
            Some(c) => u32::from_le_bytes(body[c.2..c.2 + 4].try_into().unwrap()),
            None => {
                println!("ABORT: no 0x03092005 chunk in {} -- nothing declares a time here", inp);
                std::process::exit(3);
            }
        },
    };
    if old_ms == new_ms {
        println!("{}: already declares {} ms; nothing to do", inp, new_ms);
        std::fs::copy(&inp, &out).unwrap();
        return;
    }
    println!("declared time {} ms -> {} ms", old_ms, new_ms);

    let oldb = old_ms.to_le_bytes();
    let newb = new_ms.to_le_bytes();
    let mut hits = 0usize;
    for (cid, _coff, poff, sz) in &skips {
        if !SITES.contains(cid) {
            continue;
        }
        let end = (poff + sz).min(body.len());
        let mut p = *poff;
        while p + 4 <= end {
            if body[p..p + 4] == oldb {
                body[p..p + 4].copy_from_slice(&newb);
                println!("  chunk {:#010x} +{}", cid, p - poff);
                hits += 1;
                p += 4;
            } else {
                p += 1;
            }
        }
    }
    if hits == 0 {
        println!("ABORT: the old value appears at no site in the four declared-time chunks");
        std::process::exit(3);
    }
    let mut file = g.header_bytes_u();
    file.extend_from_slice(&body);
    std::fs::write(&out, &file).unwrap_or_else(|e| panic!("write {}: {}", out, e));
    println!("setdecl: wrote {} ({} bytes, {} site(s) rewritten)", out, file.len(), hits);
}
