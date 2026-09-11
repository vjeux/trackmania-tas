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
        // `--find 0x03043025 [--hex N] [--floats N]`: every occurrence of a chunk
        // id's four bytes in the body (a NON-skippable chunk is found by its id,
        // nothing else marks it), with the bytes that follow — as hex, or as N
        // little-endian f32 (the coordinate chunks: MapCoordOrigin/Target
        // 0x03043025 = 4 f32, the thumbnail camera 0x03043036 = 10 f32).
        if let Some(find) = flag(&args, "--find").map(|s| u32::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16).expect("--find CHUNKID (hex)")) {
            let floats: usize = flag(&args, "--floats").and_then(|s| s.parse().ok()).unwrap_or(0);
            let id = find.to_le_bytes();
            let mut n = 0;
            for at in 0..g.body.len().saturating_sub(4) {
                if g.body[at..at + 4] != id {
                    continue;
                }
                n += 1;
                let p = at + 4;
                if floats > 0 {
                    let vals: Vec<String> = g.body[p..(p + 4 * floats).min(g.body.len())].chunks_exact(4).map(|w| format!("{:.3}", f32::from_le_bytes(w.try_into().unwrap()))).collect();
                    println!("0x{find:08X} at body {at}: {}", vals.join(" "));
                } else {
                    let end = (p + hex.max(32)).min(g.body.len());
                    println!("0x{find:08X} at body {at}:");
                    for (i, row) in g.body[p..end].chunks(16).enumerate() {
                        println!("  {:08x}: {:<48} {}", p + i * 16, row.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "), row.iter().map(|x| if x.is_ascii_graphic() { *x as char } else { '.' }).collect::<String>());
                    }
                }
            }
            if n == 0 {
                println!("0x{find:08X}: not in the body");
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

/// `tmmaps phases MAP [--filter PAT] [--all]` — chunk 0x03043063: the map's
/// per-item ANIMATION PHASE OFFSET (`CGameCtnAnchoredObject::AnimPhaseOffset`,
/// EPhaseOffset in eighths of the period: 0 None, 1 One8th, 2 One4th,
/// 3 Three8th, 4 Half, 5 Five8th, 6 Three4th, 7 Seven8th), one byte per
/// anchored object after the version word. This is where the map editor
/// stores the phase the author gives a kinematic item (pushers, rotors,
/// tubes, turnstiles — Summer 15's two facing channel pistons carry 0 and 4,
/// which is why the original's never meet). Prints every item with a
/// non-zero phase (all items with --all, or those matching --filter), and a
/// histogram.
pub fn phases(args: &[String]) {
    let m = map::MapFile::load(Path::new(&args[2]));
    let chunks = map::skip_chunks(&m.gbx.body);
    let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == 0x0304_3063) else {
        println!("no phase chunk 0x03043063 ({} items)", m.items.len());
        return;
    };
    let bytes = &m.gbx.body[payload + 4..payload + size];
    let ni = m.items.len();
    eprintln!("{} phase bytes for {} items (chunk version {})", bytes.len(), ni, u32::from_le_bytes(m.gbx.body[payload..payload + 4].try_into().unwrap()));
    let filter = tmmaps::cli::flag(&args, "--filter");
    let all = args.iter().any(|a| a == "--all");
    let mut hist = std::collections::BTreeMap::new();
    println!("item\tmodel\tphase8\tx\ty\tz\tyaw");
    for (i, it) in m.items.iter().enumerate() {
        let p = bytes.get(i).copied().unwrap_or(255);
        *hist.entry(p).or_insert(0usize) += 1;
        let show = match filter.as_deref() {
            Some(f) => it.model.contains(f),
            None => all || p != 0,
        };
        if show {
            println!("i{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.4}", it.index, it.model, p, it.pos[0], it.pos[1], it.pos[2], it.yaw);
        }
    }
    for (p, n) in hist {
        println!("phase {p}\t{n}");
    }
}

/// `tmmaps genealogy-cells MAP --cells x0:x1,z0:z1` — the genealogy record of
/// each cell in the range (record order = x*64 + z, the block grid), as chain,
/// index, dir, current zone. What the game regenerates there.
pub fn genealogy_cells(args: &[String]) {
    let g = gbx::Gbx::load(Path::new(&args[2])).unwrap();
    let &(_, _, payload, size) = map::skip_chunks(&g.body).iter().find(|(c, ..)| *c == 0x0304_3043).expect("no genealogy chunk");
    let b = &g.body[payload..payload + size];
    let recs = map::genealogy_full(b).expect("genealogy records");
    let side = (recs.len() as f64).sqrt() as usize;
    let spec = tmmaps::cli::flag(&args, "--cells").expect("--cells x0:x1,z0:z1");
    let (xs, zs) = spec.split_once(',').expect("--cells x0:x1,z0:z1");
    let rng = |s: &str| -> (usize, usize) { let (a, b) = s.split_once(':').unwrap_or((s, s)); (a.parse().unwrap(), b.parse().unwrap()) };
    let ((x0, x1), (z0, z1)) = (rng(xs), rng(zs));
    let order = tmmaps::cli::flag(&args, "--order").unwrap_or_else(|| "xz".into());
    println!("cell\tchain\tindex\tdir\tcurrent");
    for x in x0..=x1 {
        for z in z0..=z1 {
            let i = if order == "xz" { x * side + z } else { z * side + x };
            if let Some(r) = recs.get(i) {
                println!("({x},{z})\t{}\t{}\t{}\t{}", r.ids.join(">"), r.current_index, r.dir, r.current);
            }
        }
    }
}
