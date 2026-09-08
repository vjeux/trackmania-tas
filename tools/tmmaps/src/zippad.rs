//! `tmmaps zippad SRC.Map.Gbx --out F [--prune-unplaced] [--drop-ext dds] [--pad-items N] [--pad-bytes M] [--keep-models K]`
//!
//! Variants of one tiny map that differ ONLY in the embedded archive — the
//! instrument of the load-failure bisect (2026-09-08): the "Missing Items:
//! AC00000000.Item.Gbx" dialog of the big archives correlates with the entry
//! count (≤ 697 fine, ≥ 757 flaky) but nobody has separated the entry count
//! from the archive bytes from the load time. So:
//!
//! * `--prune-unplaced` drops the item entries no placement names (the dead
//!   weight the library ships: 40 in 21, 114 in 25);
//! * `--pad-items N` ADDS N unplaced copies of the smallest placed item under
//!   fresh names (`ZZ00000000.Item.Gbx`…, the ident rewritten inside the file,
//!   same length so no offset moves) — more ENTRIES, the map unchanged;
//! * `--pad-bytes M` adds one incompressible `Items/zzpad.dds` of M bytes —
//!   more BYTES, one entry;
//! * `--keep-models K` re-points every placement of the models after the K-th
//!   (by name) at the K-th model and drops their files — FEWER entries, the
//!   map visibly wrong (stand-ins) but every placement still resolves.
//!
//! The manifest is rebuilt from the placements (every placed `.Item.Gbx`,
//! author = its name, as `tmmaps tiny` writes it), so the game sees exactly
//! the items the map uses, plus whatever padding was asked for.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::map::MapFile;

/// The embedded archive's bytes inside chunk 0x03043054: the first local
/// header signature after the 12-byte prefix and the manifest, its length in
/// the u32 just before it.
pub fn embedded_zip(body: &[u8]) -> Option<(usize, usize)> {
    let (_, _, payload, size) = crate::gbx::all_skip_chunks(body).into_iter().find(|(cid, ..)| *cid == 0x0304_3054)?;
    let chunk = &body[payload..payload + size];
    let at = chunk.windows(4).position(|w| w == b"PK\x03\x04")?;
    if at < 4 {
        return None;
    }
    let len = u32::from_le_bytes(chunk[at - 4..at].try_into().unwrap()) as usize;
    if at + len > chunk.len() {
        return None;
    }
    Some((payload + at, payload + at + len))
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

pub fn cmd(args: &[String]) {
    let src = PathBuf::from(args.get(2).unwrap_or_else(|| crate::cli::die("zippad SRC.Map.Gbx --out F [--prune-unplaced] [--pad-items N] [--pad-bytes M] [--keep-models K]".into())));
    let out = PathBuf::from(crate::cli::flag(args, "--out").unwrap_or_else(|| crate::cli::die("zippad needs --out MAP".into())));
    let pad_items: usize = crate::cli::flag(args, "--pad-items").and_then(|v| v.parse().ok()).unwrap_or(0);
    let pad_bytes: usize = crate::cli::flag(args, "--pad-bytes").and_then(|v| v.parse().ok()).unwrap_or(0);
    let keep_models: usize = crate::cli::flag(args, "--keep-models").and_then(|v| v.parse().ok()).unwrap_or(0);
    let prune = crate::cli::has(args, "--prune-unplaced");
    let mut m = MapFile::load(&src);
    let (zs, ze) = embedded_zip(&m.gbx.body).unwrap_or_else(|| crate::cli::die("no embedded archive in the map".into()));
    let zip = m.gbx.body[zs..ze].to_vec();
    let mut entries: BTreeMap<String, Vec<u8>> = crate::header::zip_entries(&zip).into_iter().collect();
    let n0 = entries.len();
    let bytes0 = zip.len();
    // the placed models
    let mut placed: BTreeSet<String> = m.items.iter().filter(|it| it.model.ends_with(".Item.Gbx")).map(|it| it.model.clone()).collect();
    // --keep-models K: the models after the K-th collapse onto the K-th
    if keep_models > 0 && placed.len() > keep_models {
        let names: Vec<String> = placed.iter().cloned().collect();
        let stand_in = names[keep_models - 1].clone();
        let gone: BTreeSet<String> = names[keep_models..].iter().cloned().collect();
        let mut repointed = 0usize;
        for i in 0..m.items.len() {
            if gone.contains(&m.items[i].model) {
                m.set_item_model(i, &stand_in);
                m.set_item_author(i, &stand_in);
                repointed += 1;
            }
        }
        for g in &gone {
            entries.remove(&format!("Items/{g}"));
            placed.remove(g);
        }
        println!("  --keep-models {keep_models}: {repointed} placements re-pointed at {stand_in}; {} model files dropped", gone.len());
        // renames are fixed-size patches, the archive splice is not: write
        // the renamed map and reload it before touching the archive
        let tmp = out.with_extension("keep0.Map.Gbx");
        m.write_to(&tmp).expect("write the renamed map");
        m = MapFile::load(&tmp);
        let _ = std::fs::remove_file(&tmp);
    }
    if prune {
        let before = entries.len();
        entries.retain(|name, _| !name.ends_with(".Item.Gbx") || placed.contains(name.trim_start_matches("Items/")));
        println!("  --prune-unplaced: {} unplaced item entries dropped", before - entries.len());
    }
    // --drop-ext dds: every entry with that extension goes (the sign-logo and
    // light-colour pictures the items' custom-texture materials name — the
    // one kind of entry the game reads on ANOTHER thread, the texture
    // streamer's, while the main thread walks the items through the same
    // archive: the race suspect of 2026-09-08)
    if let Some(ext) = crate::cli::flag(args, "--drop-ext") {
        let suffix = format!(".{}", ext.trim_start_matches('.').to_lowercase());
        let before = entries.len();
        entries.retain(|name, _| !name.to_lowercase().ends_with(&suffix));
        println!("  --drop-ext {ext}: {} entries dropped", before - entries.len());
    }
    if pad_items > 0 {
        // the smallest placed item is the template
        let (tname, tbytes) = entries
            .iter()
            .filter(|(n, _)| n.ends_with(".Item.Gbx") && placed.contains(n.trim_start_matches("Items/")))
            .min_by_key(|(_, b)| b.len())
            .map(|(n, b)| (n.trim_start_matches("Items/").to_string(), b.clone()))
            .unwrap_or_else(|| crate::cli::die("no placed item to copy".into()));
        for k in 0..pad_items {
            let name = format!("ZZ{k:08}.Item.Gbx");
            assert_eq!(name.len(), tname.len(), "the template name {tname} is not 19 chars");
            // same-length rewrite of every occurrence of the template's ident
            let mut b = Vec::with_capacity(tbytes.len());
            let (pat, rep) = (tname.as_bytes(), name.as_bytes());
            let mut i = 0;
            while i < tbytes.len() {
                if tbytes.len() - i >= pat.len() && &tbytes[i..i + pat.len()] == pat {
                    b.extend_from_slice(rep);
                    i += pat.len();
                } else {
                    b.push(tbytes[i]);
                    i += 1;
                }
            }
            entries.insert(format!("Items/{name}"), b);
        }
        println!("  --pad-items {pad_items}: copies of {tname} ({} B) added as ZZ00000000… (unplaced, unlisted)", tbytes.len());
    }
    if pad_bytes > 0 {
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        let mut b = Vec::with_capacity(pad_bytes);
        while b.len() < pad_bytes {
            b.extend_from_slice(&xorshift(&mut s).to_le_bytes());
        }
        b.truncate(pad_bytes);
        entries.insert("Items/zzpad.dds".to_string(), b);
        println!("  --pad-bytes {pad_bytes}: one incompressible Items/zzpad.dds added");
    }
    let new_zip = crate::header::deflated_zip(&entries);
    let manifest_names: Vec<String> = placed.iter().cloned().collect();
    let manifest: Vec<(&str, &str)> = manifest_names.iter().map(|n| (n.as_str(), n.as_str())).collect();
    m.replace_embedded_objects(&manifest, &new_zip);
    m.write_to(&out).expect("write output");
    let items_in_zip = entries.keys().filter(|n| n.ends_with(".Item.Gbx")).count();
    println!(
        "{}: archive {} -> {} entries ({} .Item.Gbx, {} in the manifest), {} -> {} zip bytes; file {} B",
        out.display(),
        n0,
        entries.len(),
        items_in_zip,
        manifest.len(),
        bytes0,
        new_zip.len(),
        std::fs::metadata(&out).map(|md| md.len()).unwrap_or(0)
    );
}
