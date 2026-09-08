//! `tmmaps` record surgery: placement swaps (`swapplace`, `swaprec`,
//! `swapcell`, `swapmodel`), waypoint edits (`untag`, `wpdump`, `waypoints`,
//! `recdump`), the uid / ghost / block deletions (`setuid`, `stripghost`,
//! `delblocks`) and the rename-machinery check (`renamecheck`).

use std::path::{Path, PathBuf};
use tmmaps::cli::{flag_multi, jobs_of, server_of};
use tmmaps::{gbx, map, oracle};


/// `tmmaps recdump`.
pub fn recdump(args: &[String]) {
        let m = map::MapFile::load(Path::new(&args[2]));
        for it in m.items.iter().filter(|it| it.waypoint_tag.is_some()) {
            let (s, e) = it.record_region;
            let hex: String = m.gbx.body[s..e].iter().map(|x| format!("{:02x}", x)).collect();
            println!("i{} {} {} pos_off_rel={} model_off_rel={} anchor? {}\n  {}", it.index, it.model, it.waypoint_tag.clone().unwrap_or_default(), it.pos_off - s, m.item_ids[it.model_field].off - s, it.author.clone().unwrap_or_default(), hex);
        }
}

/// `tmmaps untag`.
pub fn untag(args: &[String]) {
        // remove an item's waypoint node (it stops being a waypoint; record shrinks to a 4-byte null)
        let m = map::MapFile::load(Path::new(&args[2]));
        let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
        let pa: usize = args.iter().position(|a| a == "--item").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--item iN");
        let ia = m.items.iter().find(|it| it.index == pa).expect("not an item").clone();
        let mut m = m;
        m.raw_splices.push((ia.waypoint_region, vec![0xff, 0xff, 0xff, 0xff]));
        println!("untagged i{} {} {:?}", pa, ia.model, ia.waypoint_tag);
        m.write_to_reporting(Path::new(&out)).expect("write");
        println!("wrote {out}");
}

/// `tmmaps swapplace`.
pub fn swapplace(args: &[String]) {
        // table-safe placement swap: exchange the two placements' MODEL names via the rename machinery,
        // and their pose (pos, yaw/pitch/roll, cell) and waypoint node (tag) via patches/splices.
        // Equivalent to swapping the records, without moving any lookback definition.
        let m = map::MapFile::load(Path::new(&args[2]));
        let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
        let pa: usize = args.iter().position(|a| a == "--a").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--a iN");
        let pb: usize = args.iter().position(|a| a == "--b").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--b iM");
        let ia = m.items.iter().position(|it| it.index == pa).expect("--a not an item");
        let ib = m.items.iter().position(|it| it.index == pb).expect("--b not an item");
        let (ra, rb) = (m.items[ia].clone(), m.items[ib].clone());
        let orig_colors = m.colors();
        let orig_body = m.gbx.body.clone();
        // the waypoint node AND the v8 tail (flags with the variant byte, pivot, scale,
        // skin FileRef, the two trailing Vec3) travel with the model: one span each
        let wa = m.gbx.body[ra.waypoint_region.0..ra.record_region.1].to_vec();
        let wb = m.gbx.body[rb.waypoint_region.0..rb.record_region.1].to_vec();
        let mut m = m;
        // the per-item side bytes (colour 0x62, anim phase 0x63, foreground 0x65,
        // lightmap quality 0x68: the items are the last ni bytes of each) swap too
        {
            let chunks = map::skip_chunks(&m.gbx.body);
            let ni = m.items.len();
            for cid in [0x0304_3062u32, 0x0304_3063, 0x0304_3065, 0x0304_3068] {
                let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == cid) else { continue };
                if size < 4 + ni {
                    continue;
                }
                let base = payload + size - ni;
                let (ba, bb) = (m.gbx.body[base + ia], m.gbx.body[base + ib]);
                if ba != bb {
                    m.raw_patches.push((base + ia, vec![bb]));
                    m.raw_patches.push((base + ib, vec![ba]));
                }
            }
        }
        m.set_item_model(ia, &rb.model);
        m.set_item_model(ib, &ra.model);
        // pose: yaw/pitch/roll + cell + pos
        let mut rot = Vec::new(); for v in [rb.yaw, rb.pitch, rb.roll] { rot.extend_from_slice(&v.to_le_bytes()); }
        m.raw_patches.push((ra.yaw_off, rot));
        let mut rot = Vec::new(); for v in [ra.yaw, ra.pitch, ra.roll] { rot.extend_from_slice(&v.to_le_bytes()); }
        m.raw_patches.push((rb.yaw_off, rot));
        m.raw_patches.push((ra.coord_off, rb.file_cell.to_vec()));
        m.raw_patches.push((rb.coord_off, ra.file_cell.to_vec()));
        let mut p = Vec::new(); for v in rb.pos { p.extend_from_slice(&v.to_le_bytes()); }
        m.raw_patches.push((ra.pos_off, p));
        let mut p = Vec::new(); for v in ra.pos { p.extend_from_slice(&v.to_le_bytes()); }
        m.raw_patches.push((rb.pos_off, p));
        // pass 1: renames + fixed-length patches; pass 2 (after reload): the variable-length tag swap
        let tmp = format!("{out}.pass1.tmp");
        m.write_to_reporting(Path::new(&tmp)).expect("write pass 1");
        let m2 = map::MapFile::load(Path::new(&tmp));
        let ja = m2.items.iter().position(|it| it.index == pa).unwrap();
        let jb = m2.items.iter().position(|it| it.index == pb).unwrap();
        let (qa, qb) = (m2.items[ja].clone(), m2.items[jb].clone());
        let mut m = m2;
        m.raw_splices.push(((qa.waypoint_region.0, qa.record_region.1), wb));
        m.raw_splices.push(((qb.waypoint_region.0, qb.record_region.1), wa));
        let _ = std::fs::remove_file(&tmp);
        println!("swapped placements i{} {} {:?} <-> i{} {} {:?} (models via rename, pose via patch, waypoint node + v8 tail + side bytes via splice)", pa, ra.model, ra.waypoint_tag, pb, rb.model, rb.waypoint_tag);
        m.write_to_reporting(Path::new(&out)).expect("write");
        println!("wrote {out}");
        // `--check`: re-read the WRITTEN map and assert that every
        // per-placement field arrived. A swap that silently drops one is
        // the dangerous kind — a start moved into the engine's slot must
        // not rescale, re-anchor, recolour or re-variant either item, and
        // the freeze pass depends on that (2026-09-07).
        if args.iter().any(|a| a == "--check") {
            let m3 = map::MapFile::load(Path::new(&out));
            let ga = m3.items.iter().find(|it| it.index == pa).expect("a");
            let gb = m3.items.iter().find(|it| it.index == pb).expect("b");
            let same = |x: [f32; 3], y: [f32; 3]| x.iter().zip(y).all(|(u, v)| (u - v).abs() < 1e-4);
            let mut bad = Vec::new();
            if ga.model != rb.model || gb.model != ra.model { bad.push("model"); }
            if !same(ga.pos, rb.pos) || !same(gb.pos, ra.pos) { bad.push("pos"); }
            if (ga.yaw - rb.yaw).abs() > 1e-6 || (gb.yaw - ra.yaw).abs() > 1e-6 { bad.push("yaw"); }
            if !same(ga.pivot, rb.pivot) || !same(gb.pivot, ra.pivot) { bad.push("pivot"); }
            if (ga.scale - rb.scale).abs() > 1e-6 || (gb.scale - ra.scale).abs() > 1e-6 { bad.push("scale"); }
            if ga.variant() != rb.variant() || gb.variant() != ra.variant() { bad.push("variant"); }
            if ga.waypoint_tag != rb.waypoint_tag || gb.waypoint_tag != ra.waypoint_tag { bad.push("waypoint tag"); }
            if ga.waypoint_order != rb.waypoint_order || gb.waypoint_order != ra.waypoint_order { bad.push("waypoint order"); }
            if ga.skin(&m3.gbx.body) != rb.skin(&orig_body) || gb.skin(&m3.gbx.body) != ra.skin(&orig_body) { bad.push("skin"); }
            if let (Some(c0), Some(c1)) = (orig_colors.as_ref(), m3.colors()) {
                if c1.item(pa) != c0.item(pb) || c1.item(pb) != c0.item(pa) { bad.push("colour"); }
            }
            if bad.is_empty() {
                println!("check: every per-placement field arrived (model, pose, pivot, scale, variant, tag, order, colour, skin)");
            } else {
                eprintln!("check FAILED: {} did not survive the swap", bad.join(", "));
                std::process::exit(1);
            }
        }
}

/// `tmmaps swaprec`.
pub fn swaprec(args: &[String]) {
        // swap two whole item placement RECORDS (file order experiment); the item count is unchanged
        let m = map::MapFile::load(Path::new(&args[2]));
        let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
        let pa: usize = args.iter().position(|a| a == "--a").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--a iN");
        let pb: usize = args.iter().position(|a| a == "--b").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--b iM");
        let ia = m.items.iter().find(|it| it.index == pa).expect("--a not an item").clone();
        let ib = m.items.iter().find(|it| it.index == pb).expect("--b not an item").clone();
        let mut ra = m.gbx.body[ia.record_region.0..ia.record_region.1].to_vec();
        let mut rb = m.gbx.body[ib.record_region.0..ib.record_region.1].to_vec();
        // the author field is a lookback REFERENCE to the model-name slot this very record defines
        // (slot numbers follow file order), so each moved record must take over the slot word of
        // the position it lands in -- otherwise the file carries a forward reference and the game
        // refuses to load it
        let (aa, ab) = (&m.item_ids[ia.author_field], &m.item_ids[ib.author_field]);
        if aa.len == 4 && ab.len == 4 && m.item_ids[ia.model_field].is_def && m.item_ids[ib.model_field].is_def {
            let oa = aa.off - ia.record_region.0; // author word offset inside record a
            let ob = ab.off - ib.record_region.0;
            // rb goes to a's position: its model def takes a's slot; its author word must be a's word
            rb[ob..ob + 4].copy_from_slice(&aa.raw.to_le_bytes());
            ra[oa..oa + 4].copy_from_slice(&ab.raw.to_le_bytes());
            println!("author slot words re-homed: {:#x} <-> {:#x}", aa.raw, ab.raw);
        }
        let mut m = m;
        m.raw_splices.push((ia.record_region, rb));
        m.raw_splices.push((ib.record_region, ra));
        println!("swapped records: i{} ({} B) <-> i{} ({} B)", pa, ia.record_region.1 - ia.record_region.0, pb, ib.record_region.1 - ib.record_region.0);
        m.write_to_reporting(Path::new(&out)).expect("write");
        println!("wrote {out}");
}

/// `tmmaps swapcell`.
pub fn swapcell(args: &[String]) {
        // swap the CELL bytes (file_cell) of two item placements; positions untouched
        let m = map::MapFile::load(Path::new(&args[2]));
        let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
        let pa: usize = args.iter().position(|a| a == "--a").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--a iN");
        let pb: usize = args.iter().position(|a| a == "--b").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--b iM");
        let ia = m.items.iter().find(|it| it.index == pa).expect("--a not an item").clone();
        let ib = m.items.iter().find(|it| it.index == pb).expect("--b not an item").clone();
        let mut m = m;
        m.raw_patches.push((ia.coord_off, ib.file_cell.to_vec()));
        m.raw_patches.push((ib.coord_off, ia.file_cell.to_vec()));
        println!("swapped cells: i{} {:?} <-> i{} {:?}", pa, ia.file_cell, pb, ib.file_cell);
        m.write_to_reporting(Path::new(&out)).expect("write");
        println!("wrote {out}");
}

/// `tmmaps swapmodel`.
pub fn swapmodel(args: &[String]) {
        // swap the model Id words of two item placements (both must be 4-byte table references)
        let m = map::MapFile::load(Path::new(&args[2]));
        let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
        let pa: usize = args.iter().position(|a| a == "--a").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--a iN");
        let pb: usize = args.iter().position(|a| a == "--b").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--b iM");
        let ia = m.items.iter().find(|it| it.index == pa).expect("--a not an item").clone();
        let ib = m.items.iter().find(|it| it.index == pb).expect("--b not an item").clone();
        let fa = m.item_ids[ia.model_field].clone();
        let fb = m.item_ids[ib.model_field].clone();
        let mut m = m;
        if fa.len == 4 && fb.len == 4 {
            let (oa, ob, ra, rb) = (fa.off, fb.off, fa.raw, fb.raw);
            m.raw_patches.push((oa, rb.to_le_bytes().to_vec()));
            m.raw_patches.push((ob, ra.to_le_bytes().to_vec()));
        } else if fa.is_def && fb.is_def && fa.len == fb.len {
            // both inline definitions of equal length: swap the name strings (every later
            // reference to either table slot follows the swap: the two MODELS trade places)
            let na = fa.name.clone().unwrap_or_default().into_bytes();
            let nb = fb.name.clone().unwrap_or_default().into_bytes();
            m.raw_patches.push((fa.off + 8, nb));
            m.raw_patches.push((fb.off + 8, na));
            println!("(inline definitions: the two model NAMES were swapped at their definition sites -- every placement of either model trades models)");
        } else {
            panic!("model ids not swappable: a len {} def {} / b len {} def {}", fa.len, fa.is_def, fb.len, fb.is_def);
        }
        println!("swapped model ids: i{} {} <-> i{} {} (words {:#x} <-> {:#x})", pa, ia.model, pb, ib.model, fa.raw, fb.raw);
        m.write_to_reporting(Path::new(&out)).expect("write swapped map");
        println!("wrote {out}");
}

/// `tmmaps wpdump`.
pub fn wpdump(args: &[String]) {
        // every waypoint ITEM's raw placement record fields, for diffing maps
        let m = map::MapFile::load(Path::new(&args[2]));
        println!("idx\tmodel\tauthor\tcoll\ttag\torder\tyaw\tpitch\troll\tcoords\tpos\tflags\tpivot\tscale\ttail24\twpnode_bytes\trecord_bytes");
        for it in m.items.iter().filter(|it| it.waypoint_tag.is_some()) {
            let b = &m.gbx.body;
            let (ws, we) = it.waypoint_region;
            // order = the u32 after the tag string inside the waypoint node (v2)
            let order = if we - ws >= 16 { u32::from_le_bytes(b[we - 8..we - 4].try_into().unwrap()) } else { 0 };
            let flags = u16::from_le_bytes(b[we..we + 2].try_into().unwrap());
            let tail_start = it.scale_off + 4 + if flags & 4 != 0 { 0 } else { 0 };
            let tail: Vec<f32> = (0..6).map(|k| f32::from_le_bytes(b[tail_start + 4 * k..tail_start + 4 * k + 4].try_into().unwrap())).collect();
            let wp_hex: String = b[ws..we].iter().map(|x| format!("{:02x}", x)).collect();
            println!("{}\t{}\t{}\t{:#x}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:?}\t({:.3},{:.3},{:.3})\t{:#06x}\t({:.3},{:.3},{:.3})\t{:.3}\t{:?}\t{}\t{}", it.index, it.model, it.author.clone().unwrap_or_default(), it.collection_raw, it.waypoint_tag.clone().unwrap_or_default(), order, it.yaw, it.pitch, it.roll, it.file_cell, it.pos[0], it.pos[1], it.pos[2], flags, it.pivot[0], it.pivot[1], it.pivot[2], it.scale, tail, wp_hex, it.record_region.1 - it.record_region.0);
        }
}

/// `tmmaps waypoints`.
pub fn waypoints(args: &[String]) {
        let m = map::MapFile::load(Path::new(&args[2]));
        eprintln!(
            "size={:?} decoration={} collection={:#x} blocks={} items={} body_regions={:?} items_region={:?}",
            m.size,
            m.decoration_id,
            m.items.first().map(|it| it.collection_raw).unwrap_or(0),
            m.blocks.len(),
            m.items.len(),
            m.body_regions.clone(),
            m.items_region
        );
        for (i, w) in m.waypoints().iter().enumerate() {
            println!("{} {}", i, w);
        }
}

/// `tmmaps renamecheck`.
pub fn renamecheck(args: &[String]) {
        // `prs`: the RENAMING round-trip. The identity round-trip
        // (`tmmaps roundtrip`) is blind to a whole class of surgery bug,
        // because it never adds or removes a lookback-table slot -- and
        // the table is exactly where a rename can go wrong. The blocks
        // chunk and the baked chunk share one table, and parts of the file
        // downstream of both hold raw indices into it, so a rename that
        // changes the table's LENGTH can silently renumber somebody else's
        // name.
        //
        // So: rename one waypoint, write, re-read, and require that EVERY
        // OTHER block name, item model, waypoint tag and waypoint
        // placement is unchanged. Three renames are tried, because they
        // stress the table in different directions:
        //
        //   same-length  -- content changes, no slot moves
        //   fresh        -- a name the table has never seen (may add a slot)
        //   existing     -- another block's name (may drop a slot)
        //
        // `reemit_regions` already warns "downstream indices may not
        // resolve" when neither encoder preserves the length. This turns
        // that warning into a pass/fail on the actual names.
        let src = PathBuf::from(&args[2]);
        let m0 = map::MapFile::load(&src);
        let wp: Vec<usize> = m0
            .blocks
            .iter()
            .filter(|b| b.waypoint_tag.is_some())
            .map(|b| b.index)
            .collect();
        if wp.is_empty() {
            println!("{}: no waypoint blocks to rename", src.display());
            return;
        }
        let target = wp[0];
        let orig = m0.blocks[target].name.clone();
        // a name the table has never seen, one the same length, and one
        // that another block already owns
        let same: String = {
            let mut s = orig.clone();
            let n = s.len();
            s.replace_range(n - 1.., "Z");
            s
        };
        let fresh = format!("{}_prsRenameCheck", orig);
        let other = m0
            .blocks
            .iter()
            .map(|b| b.name.clone())
            .find(|n| *n != orig && !n.is_empty())
            .unwrap_or_else(|| "RoadTechStraight".into());
        let mut fails = 0;
        // test 0 -- rename to ITSELF. This forces the whole two-region Id
        // stream through the rename re-encoder (`Mode::SlotPreserving` /
        // `Fresh`) instead of the identity memcpy path, and requires the
        // result to be byte-identical. `tmmaps roundtrip` never exercises
        // that code at all.
        {
            let mut m = map::MapFile::load(&src);
            m.set_block_name(target, &orig);
            let built = gbx::Gbx::parse(&m.build()).body;
            let ok = built == m0.gbx.body;
            println!(
                "  {:<12} {}   (re-encoder exercised, output must be byte-identical)",
                "self",
                if ok { "OK  " } else { "FAIL" }
            );
            if !ok {
                fails += 1;
            }
        }
        for (label, newname) in
            [("same-length", &same), ("fresh", &fresh), ("existing", &other)]
        {
            let mut m = map::MapFile::load(&src);
            m.set_block_name(target, newname);
            let tmp = std::env::temp_dir()
                .join(format!("prs-renamecheck-{}.Map.Gbx", std::process::id()));
            if m.write_to(&tmp).is_err() {
                println!("  {:<12} WRITE FAILED", label);
                fails += 1;
                continue;
            }
            let m2 = match std::panic::catch_unwind(|| map::MapFile::load(&tmp)) {
                Ok(v) => v,
                Err(_) => {
                    println!("  {:<12} RE-READ PANICKED", label);
                    fails += 1;
                    continue;
                }
            };
            let mut bad: Vec<String> = Vec::new();
            if m2.blocks.len() != m0.blocks.len() {
                bad.push(format!("block count {} -> {}", m0.blocks.len(), m2.blocks.len()));
            }
            if m2.items.len() != m0.items.len() {
                bad.push(format!("item count {} -> {}", m0.items.len(), m2.items.len()));
            }
            for (a, b) in m0.blocks.iter().zip(m2.blocks.iter()) {
                if a.index == target {
                    if b.name != *newname {
                        bad.push(format!(
                            "target block#{} name {:?} != {:?}",
                            a.index, b.name, newname
                        ));
                    }
                    continue;
                }
                if a.name != b.name {
                    bad.push(format!(
                        "block#{} name {:?} -> {:?}",
                        a.index, a.name, b.name
                    ));
                }
                if a.waypoint_tag != b.waypoint_tag
                    || a.file_cell != b.file_cell
                    || a.dir != b.dir
                    || a.free_pos != b.free_pos
                {
                    bad.push(format!("block#{} placement/tag changed", a.index));
                }
                if bad.len() > 6 {
                    break;
                }
            }
            for (a, b) in m0.items.iter().zip(m2.items.iter()) {
                if a.model != b.model || a.waypoint_tag != b.waypoint_tag || a.pos != b.pos {
                    bad.push(format!("item#{} {:?} -> {:?}", a.index, a.model, b.model));
                }
                if bad.len() > 6 {
                    break;
                }
            }
            let _ = std::fs::remove_file(&tmp);
            if bad.is_empty() {
                println!("  {:<12} OK   (renamed block#{})", label, target);
            } else {
                fails += 1;
                println!("  {:<12} FAIL {} problem(s):", label, bad.len());
                for b in bad.iter().take(6) {
                    println!("      {}", b);
                }
            }
        }
        println!("{}: renamecheck {} failure(s)", src.display(), fails);
        // With --ghosts, add the check the parser cannot make: a
        // mutually-consistent reader/writer error is invisible to a
        // re-read, so ask the GAME. Rename a block that is far from every
        // waypoint -- decoration, not track -- to a fresh name, and
        // require the control ghost's time to be unchanged. A table that
        // renumbered somebody else's name shows up as "Can't load map"
        // (no row at all) or as a different time.
        let ghosts: Vec<PathBuf> =
            flag_multi(&args, "--ghosts").into_iter().map(PathBuf::from).collect();
        if !ghosts.is_empty() {
            let wpos: Vec<(i32, i32, i32)> =
                wp.iter().map(|i| m0.blocks[*i].coords()).collect();
            let far = m0
                .blocks
                .iter()
                .filter(|b| b.waypoint_tag.is_none() && !b.name.is_empty())
                .max_by_key(|b| {
                    let (x, y, z) = b.coords();
                    wpos.iter()
                        .map(|(a, c, d)| {
                            (x - a).pow(2) + (y - c).pow(2) + (z - d).pow(2)
                        })
                        .min()
                        .unwrap_or(0)
                })
                .map(|b| b.index);
            if let Some(fi) = far {
                let mut m = map::MapFile::load(&src);
                let fname = m0.blocks[fi].name.clone();
                m.set_block_name(fi, &format!("{}_prsRenameCheck", fname));
                let tmp = std::env::temp_dir()
                    .join(format!("prs-rc-game-{}.Map.Gbx", std::process::id()));
                m.write_to(&tmp).expect("write renamed map");
                let base = oracle::run_maps(
                    &[(src.clone(), ghosts.clone())],
                    jobs_of(&args),
                    &server_of(&args),
                );
                let got = oracle::run_maps(
                    &[(tmp.clone(), ghosts.clone())],
                    jobs_of(&args),
                    &server_of(&args),
                );
                let want = oracle::times(&base[0]);
                let have = oracle::times(&got[0]);
                let mut bad = 0;
                for (k, w) in &want {
                    let g = have.get(k).cloned().flatten();
                    if g != *w {
                        println!("      GAME {}: untouched {:?} vs renamed {:?}", k, w, g);
                        bad += 1;
                    }
                }
                if have.is_empty() {
                    println!("      GAME: the renamed map produced NO rows -- it did not load");
                    bad += 1;
                }
                println!(
                    "  {:<12} {}   (renamed off-route block#{} {:?}, {} control ghost(s))",
                    "game",
                    if bad == 0 { "OK  " } else { "FAIL" },
                    fi,
                    fname.chars().take(40).collect::<String>(),
                    want.len()
                );
                let _ = std::fs::remove_file(&tmp);
                if bad > 0 {
                    fails += 1;
                }
                println!("{}: renamecheck (with game check) {} failure(s)", src.display(), fails);
            }
        }
        if fails > 0 {
            std::process::exit(1);
        }
}

/// `tmmaps stripghost`.
pub fn stripghost(args: &[String]) {
        // `tmmaps stripghost MAP --out F`: drop the author's validation ghost
        // (the ORIGINAL map's full-size run, replayed over the tiny map as a
        // car driving in the air) and mark the map unvalidated. See
        // `MapFile::strip_validation_ghost`.
        let path = std::path::Path::new(&args[2]);
        let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
        // Default: the chunk stays as the game's own empty 12-byte skeleton (a
        // later `authorghost embed` can replace it byte-safely; inserting a
        // missing chunk shifts the body's Id table); `--remove-chunk` drops it.
        let skeleton = !args.iter().any(|a| a == "--remove-chunk");
        let mut m = map::MapFile::load(path);
        let removed = m.strip_validation_ghost_to(skeleton);
        if removed == 0 {
            println!("{}: no validation ghost to strip (no chunk, or already the empty skeleton)", path.display());
            std::process::exit(1);
        }
        m.write_to(std::path::Path::new(&out)).expect("write");
        println!("{}: validation ghost dropped ({removed} bytes; chunk {}), header validated=\"0\" -> {out}", path.display(), if skeleton { "kept as the empty skeleton" } else { "removed" });
}

/// `tmmaps setuid`.
pub fn setuid(args: &[String]) {
        let src = std::path::PathBuf::from(&args[2]);
        let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("setuid needs --out MAP"));
        let uid = tmmaps::cli::flag(&args, "--uid").map(String::from).unwrap_or_else(|| {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
            format!("Tst1{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000)
        });
        let mut m = tmmaps::map::MapFile::load(&src);
        m.set_map_uid(&uid);
        m.write_to(&out).expect("write output");
        println!("wrote {} with uid {uid}", out.display());
}

/// `tmmaps delblocks`.
pub fn delblocks(args: &[String]) {
        let src = std::path::PathBuf::from(&args[2]);
        let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("delblocks needs --out MAP"));
        let keep: std::collections::BTreeSet<String> = tmmaps::cli::flag(&args, "--keep-baked").unwrap_or("").split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
        // --keep-first N: the first N authored blocks stay (the "does ONE block suffice" probe)
        let keep_first: usize = tmmaps::cli::flag(&args, "--keep-first").and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut m = tmmaps::map::MapFile::load(&src);
        let (nb, nk) = (m.blocks.len(), m.baked.len());
        let r = m.remove_blocks(|b| b.index >= keep_first, |b| !keep.contains(&b.name));
        println!("deleted {} of {nb} authored and {} of {nk} generated blocks; {} free entries, {} snap groups ({} items un-snapped)", r.blocks, r.baked, r.free_entries, r.snap_groups, r.snapped_items_cleared);
        let tmp = out.with_extension("del0.Map.Gbx");
        m.write_to(&tmp).expect("write");
        let mut m = tmmaps::map::MapFile::load(&tmp);
        if tmmaps::cli::has(&args, "--strip-lightmap") {
            println!("lightmap stripped ({} bytes)", m.strip_lightmap());
        }
        m.write_to(&out).expect("write output");
        let _ = std::fs::remove_file(&tmp);
        println!("wrote {} ({} blocks, {} items)", out.display(), m.blocks.len(), m.items.len());
}
