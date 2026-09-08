//! `tmmaps` read-only inspectors: `colors` (chunk 0x03043062), `blockrefs`,
//! `chunks`, `genealogy` (chunk 0x03043043), `segments`.

use std::path::{Path, PathBuf};
use tmmaps::cli::{flag, has, jobs_of, server_of};
use tmmaps::{gbx, map, secs, segments};


/// `tmmaps segments`.
pub fn segment_table(args: &[String]) {
        let src = PathBuf::from(&args[2]);
        let out = PathBuf::from(flag(&args, "--out").unwrap_or("/tmp/segmaps"));
        let g = flag(&args, "--ref-ghost").expect("--ref-ghost is required (order is measured)");
        let ord: Option<Vec<String>> = flag(&args, "--order")
            .map(|s| s.split(',').map(|v| v.trim().to_string()).collect());
        let segs = match segments::make_all_ordered(
            &src,
            &out,
            Path::new(g),
            jobs_of(&args),
            &server_of(&args),
            true,
            ord.as_deref(),
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(2);
            }
        };
        for s in &segs {
            println!(
                "seg{} {} cut={} method={} exact={} time={} expect={} verified={}",
                s.segment,
                s.map.display(),
                s.cut,
                s.method,
                s.exact,
                secs::opt(s.time),
                secs::ms(s.expect),
                s.verified
            );
        }
}

/// `tmmaps chunks`.
pub fn chunks(args: &[String]) {
        // Every skippable chunk in the body, with its size. Needed to
        // reason about FREE blocks (0x0304305F) and to tell at a glance
        // whether a map even has a baked-blocks chunk (0x03043048).
        // `--only 0x0304305D --hex N` dumps the head of one chunk's payload.
        let g = gbx::Gbx::load(Path::new(&args[2])).unwrap();
        let only: Option<u32> = flag(&args, "--only").map(|s| u32::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16).expect("--only CHUNKID (hex)"));
        let hex: usize = flag(&args, "--hex").and_then(|s| s.parse().ok()).unwrap_or(0);
        // `--at OFF --hex N`: the decompressed body at an absolute offset
        // (the non-skippable chunks -- the MediaTracker 0x03043049 -- have
        // no PIKS header and never appear in the table below).
        if let Some(at) = flag(&args, "--at").and_then(|s| s.parse::<usize>().ok()) {
            let end = (at + hex.max(256)).min(g.body.len());
            println!("body {} bytes; {at}..{end}:", g.body.len());
            for (i, row) in g.body[at..end].chunks(16).enumerate() {
                println!("  {:08x}: {:<48} {}", at + i * 16, row.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "), row.iter().map(|x| if x.is_ascii_graphic() { *x as char } else { '.' }).collect::<String>());
            }
            return;
        }
        println!("chunk\toff\tpayload\tsize");
        for (cid, off, payload, size) in map::skip_chunks(&g.body) {
            if only.map(|c| c != cid).unwrap_or(false) {
                continue;
            }
            println!("0x{:08X}\t{}\t{}\t{}", cid, off, payload, size);
            if has(&args, "--words") {
                // the payload as little-endian i32 words, one per line (for a histogram)
                for w in g.body[payload..payload + size - size % 4].chunks(4) {
                    println!("{}", i32::from_le_bytes(w.try_into().unwrap()));
                }
            }
            if hex > 0 {
                let b = &g.body[payload..payload + size.min(hex)];
                for (i, row) in b.chunks(16).enumerate() {
                    println!("  {:06x}: {:<48} {}", i * 16, row.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "), row.iter().map(|x| if x.is_ascii_graphic() { *x as char } else { '.' }).collect::<String>());
                }
            }
        }
}

/// `tmmaps blockrefs`.
pub fn blockrefs(args: &[String]) {
        // Everything in the file that lists blocks by INDEX or per block,
        // against the parsed block and item counts — the audit behind
        // `MapFile::remove_blocks` (2026-09-07): the snapped-on tables of
        // 0x03043040, the free-block entries of 0x0304305F, the per-block
        // bytes of 0x03043062/0x03043068, the macroblock refs of 0x03043069.
        let m = map::MapFile::load(Path::new(&args[2]));
        let nb = m.blocks.len();
        let nk = m.baked.len();
        let ni = m.items.len();
        let placeholders = m.blocks.iter().filter(|b| b.flags == 0xFFFF_FFFF).count();
        println!("blocks {nb} (placeholders {placeholders})  baked {nk}  items {ni}  lookback slots {}", m.body_ids.iter().filter(|f| f.is_def).count());
        println!("block records {:?} ({} bytes), baked records {:?}", m.blocks_records, m.blocks_records.1 - m.blocks_records.0, m.baked_records);
        let free_b = m.blocks.iter().filter(|b| b.free_off.is_some()).count();
        let free_k = m.baked.iter().filter(|b| b.free_off.is_some()).count();
        println!("free blocks {free_b} + free baked {free_k} (0x0304305F entries)");
        let chunks = map::skip_chunks(&m.gbx.body);
        for cid in [0x0304_3062u32, 0x0304_3068] {
            match chunks.iter().find(|(c, ..)| *c == cid) {
                Some(&(_, _, _, size)) => println!("chunk {cid:#010x}: {size} bytes = 4 + {nb} + {nk} + {ni} -> {}", if size == 4 + nb + nk + ni { "ok" } else { "MISMATCH" }),
                None => println!("chunk {cid:#010x}: absent"),
            }
        }
        match m.macroblock_refs() {
            Some(mb) => println!("chunk 0x03043069: {} bytes; blocks with a macroblock ref {}, items {}, tail {} bytes", mb.size, mb.blocks.iter().filter(|v| **v != -1).count(), mb.items.iter().filter(|v| **v != -1).count(), mb.tail.len()),
            None => println!("chunk 0x03043069: absent"),
        }
        // the removal re-serialises both block chunks: with nothing dropped it must reproduce the body
        {
            let mut same = map::MapFile::load(Path::new(&args[2]));
            same.remove_blocks(|_| false, |_| false);
            let body = same.patched_body();
            match body.iter().zip(&m.gbx.body).position(|(a, b)| a != b) {
                None if body.len() == m.gbx.body.len() => println!("no-op block rewrite: byte-identical"),
                first => println!("no-op block rewrite: DIFFERS ({} -> {} bytes, first at {:?})", m.gbx.body.len(), body.len(), first),
            }
        }
        match chunks.iter().find(|(c, ..)| *c == 0x0304_305D) {
            Some(&(_, _, payload, size)) => match map::octree_chunk_summary(&m.gbx.body[payload..payload + size]) {
                Ok(None) => println!("chunk 0x0304305D: empty (no tree)"),
                Ok(Some((grid, origin, n, hist))) => println!("chunk 0x0304305D: octree grid {grid} origin {origin:?}, {n} nodes, leaf flags {hist:?} — node indices only, no block refs"),
                Err(e) => println!("chunk 0x0304305D: {size} bytes, NOT the octree layout: {e}"),
            },
            None => println!("chunk 0x0304305D: absent"),
        }
        match m.snap_tables() {
            Some(st) => {
                let bg = st.block_indexes.iter().filter(|v| **v != -1).count();
                let tagged = st.block_indexes.iter().filter(|v| **v != -1 && (**v as u32) >> 24 != 0).count();
                let ig = st.item_indexes.iter().filter(|v| **v >= 0).count();
                let both = st.block_indexes.iter().zip(&st.item_indexes).filter(|(b, i)| **b != -1 && **i >= 0).count();
                let snapped = st.snapped.iter().filter(|v| **v >= 0).count();
                println!("chunk 0x03043040 snapped-on tables: {} groups ({bg} name a block — {tagged} with a tag byte —, {ig} an item, {both} both), snap_groups {:?}.., u07 all -1: {}, {snapped} of {} items snapped", st.block_indexes.len(), st.snap_groups.iter().take(6).collect::<Vec<_>>(), st.u07.iter().all(|v| *v == -1), st.snapped.len());
                if has(&args, "--groups") {
                    for k in 0..st.block_indexes.len() {
                        let users: Vec<usize> = st.snapped.iter().enumerate().filter(|(_, s)| **s == k as i32).map(|(i, _)| i).collect();
                        let target = if st.block_indexes[k] != -1 { let w = st.block_indexes[k] as u32; let idx = (w & 0x00FF_FFFF) as usize; format!("block {idx}{} {}", if w >> 24 != 0 { format!(" (tag {:#04x})", w >> 24) } else { String::new() }, m.blocks.get(idx).map(|b| format!("{} {:?}", b.name, b.coords())).unwrap_or("?".into())) } else { format!("item {} {}", st.item_indexes[k], m.items.get(st.item_indexes[k] as usize).map(|i| i.model.as_str()).unwrap_or("?")) };
                        println!("  group {k}: {target} group {} <- items {:?}", st.snap_groups[k], users);
                    }
                }
            }
            None => println!("chunk 0x03043040 snapped-on tables: none"),
        }
}

/// `tmmaps genealogy`.
pub fn genealogy(args: &[String]) {
        // Chunk 0x03043043 (terrain zone genealogies): version, inner
        // buffer length, record count, then the records — a hex dump of
        // the head to read the record layout off.
        let g = gbx::Gbx::load(Path::new(&args[2])).unwrap();
        let &(_, _, payload, size) = map::skip_chunks(&g.body).iter().find(|(c, ..)| *c == 0x0304_3043).expect("no genealogy chunk");
        let b = &g.body[payload..payload + size];
        println!("version {} buffer {} count {}", u32::from_le_bytes(b[0..4].try_into().unwrap()), u32::from_le_bytes(b[4..8].try_into().unwrap()), u32::from_le_bytes(b[8..12].try_into().unwrap()));
        match map::genealogy_full(b) {
            Ok(recs) => {
                let mut hist = std::collections::BTreeMap::new();
                for r in &recs { *hist.entry(r.current.clone()).or_insert(0) += 1; }
                println!("zones: {hist:?}");
                // --chains: the distinct full records (zone chain, current
                // index, direction) with their counts — what the game
                // regenerates per cell, not just the current zone's name
                if tmmaps::cli::has(&args, "--chains") {
                    let mut chains: std::collections::BTreeMap<String, usize> = Default::default();
                    for r in &recs { *chains.entry(r.describe()).or_insert(0) += 1; }
                    let mut rows: Vec<_> = chains.into_iter().collect();
                    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                    println!("{} distinct records:", rows.len());
                    for (chain, n) in rows { println!("  {n:5}  {chain}"); }
                }
                if tmmaps::cli::has(&args, "--grid") {
                    // 64 rows of 64 first letters, record order
                    for row in recs.chunks(64) {
                        println!("{}", row.iter().map(|r| r.current.chars().next().unwrap_or('.')).collect::<String>());
                    }
                }
            }
            Err(e) => println!("records: {e}"),
        }
        let n: usize = tmmaps::cli::flag(&args, "--bytes").and_then(|s| s.parse().ok()).unwrap_or(256);
        for (i, row) in b[12..(12 + n).min(b.len())].chunks(16).enumerate() {
            println!("{:06x}: {}  {}", 12 + i * 16, row.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "), row.iter().map(|x| if x.is_ascii_graphic() { *x as char } else { '.' }).collect::<String>());
        }
}

/// `tmmaps colors`.
pub fn colors(args: &[String]) {
        // Chunk 0x03043062: version, then one colour byte per block
        // (unbaked, then baked) and per item — 0 Default, 1 White, 2 Green,
        // 3 Blue, 4 Red, 5 Black. Prints a histogram and, with --filter,
        // the colour of every matching block.
        let m = map::MapFile::load(Path::new(&args[2]));
        let chunks = map::skip_chunks(&m.gbx.body);
        let &(_, _, payload, size) = chunks.iter().find(|(c, ..)| *c == 0x0304_3062).expect("no colour chunk");
        let bytes = &m.gbx.body[payload + 4..payload + size];
        let nb = m.blocks.len();
        let nbaked = m.baked.len();
        let ni = m.items.len();
        eprintln!("{} colour bytes for {} blocks + {} baked + {} items", bytes.len(), nb, nbaked, ni);
        let filter = tmmaps::cli::flag(&args, "--filter");
        let mut hist = std::collections::BTreeMap::new();
        for (i, b) in m.blocks.iter().enumerate() {
            let c = bytes.get(i).copied().unwrap_or(255);
            *hist.entry(("block", c)).or_insert(0) += 1;
            if filter.as_deref().map(|f| b.name.contains(f)).unwrap_or(false) {
                println!("block\t{}\t{}\tcolor {}", i, b.name, c);
            }
        }
        for (i, it) in m.items.iter().enumerate() {
            let c = bytes.get(nb + nbaked + i).copied().unwrap_or(255);
            *hist.entry(("item", c)).or_insert(0) += 1;
            if filter.as_deref().map(|f| it.model.contains(f)).unwrap_or(false) {
                println!("item\t{}\t{}\tcolor {}", i, it.model, c);
            }
        }
        for (i, b) in m.baked.iter().enumerate() {
            let c = bytes.get(nb + i).copied().unwrap_or(255);
            *hist.entry(("baked", c)).or_insert(0) += 1;
            if filter.as_deref().map(|f| b.name.contains(f)).unwrap_or(false) {
                println!("baked\tb{}\t{}\tcolor {}", i, b.name, c);
            }
        }
        for ((k, c), n) in hist {
            println!("{k}\tcolor {c}\t{n}");
        }
}
