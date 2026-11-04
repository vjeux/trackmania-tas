//! `mapgeom mapsize MAP.Map.Gbx [--top N] [--tsv OUT.tsv]` — where the bytes of a
//! map go. The upload cap (25 MiB) is on the FILE, and the file is the map body
//! LZO-compressed, inside which the embedded-objects archive (chunk
//! `0x03043054`) is a ZIP of deflated entries; so the number that matters per
//! entry is its DEFLATED size, not its raw size. This prints:
//!
//! - the archive's entries grouped by kind (items, blocks, textures by folder),
//!   raw and deflated;
//! - a DDS census: format and dimensions × count, raw and deflated, so a
//!   resolution lever can be priced;
//! - the texture references found in the item and block bodies (their
//!   `CPlugMaterialUserInst` records: material model, slot id, `Skins\…` path),
//!   joined with the entries: per-slot bytes, textures no body names, names no
//!   entry serves, textures shared by several materials;
//! - byte-identical duplicates (by content hash);
//! - the top-N entries by deflated size.
//!
//! Written 2026-11-04 for AR's `Egypt 2026 (Ice)` (161 MiB): the question was
//! whether the LOD lever applies, and the census says 85 % of the file is
//! per-block baked texture atlases, which no mesh lever reaches.

use std::collections::{BTreeMap, HashMap};
use tmmaps::gbx::Gbx;

#[derive(Default, Clone)]
struct Entry {
    raw: u64,
    deflated: u64,
}

#[derive(Default)]
struct Tally {
    n: u64,
    raw: u64,
    deflated: u64,
}

impl Tally {
    fn add(&mut self, e: &Entry) {
        self.n += 1;
        self.raw += e.raw;
        self.deflated += e.deflated;
    }
}

fn mib(b: u64) -> String {
    format!("{:.2} MiB", b as f64 / 1048576.0)
}

/// Local-header walk of the archive: (name, sizes, offset of the data, method).
fn zip_sizes(zip: &[u8]) -> Vec<(String, Entry, usize, u16)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 30 <= zip.len() && &zip[i..i + 4] == b"PK\x03\x04" {
        let method = u16::from_le_bytes(zip[i + 8..i + 10].try_into().unwrap());
        let csize = u32::from_le_bytes(zip[i + 18..i + 22].try_into().unwrap()) as usize;
        let usize_ = u32::from_le_bytes(zip[i + 22..i + 26].try_into().unwrap()) as u64;
        let nlen = u16::from_le_bytes(zip[i + 26..i + 28].try_into().unwrap()) as usize;
        let xlen = u16::from_le_bytes(zip[i + 28..i + 30].try_into().unwrap()) as usize;
        let name = String::from_utf8_lossy(&zip[i + 30..i + 30 + nlen]).to_string();
        let start = i + 30 + nlen + xlen;
        if start + csize > zip.len() {
            break;
        }
        out.push((name, Entry { raw: usize_, deflated: csize as u64 }, start, method));
        i = start + csize;
    }
    out
}

fn entry_bytes(zip: &[u8], start: usize, e: &Entry, method: u16) -> Vec<u8> {
    let c = &zip[start..start + e.deflated as usize];
    match method {
        0 => c.to_vec(),
        8 => miniz_oxide::inflate::decompress_to_vec(c).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// `DDS ` header: (fourcc or "RGB", width, height, mip count).
fn dds_info(b: &[u8]) -> Option<(String, u32, u32, u32)> {
    if b.len() < 128 || &b[0..4] != b"DDS " {
        return None;
    }
    let h = u32::from_le_bytes(b[12..16].try_into().unwrap());
    let w = u32::from_le_bytes(b[16..20].try_into().unwrap());
    let mips = u32::from_le_bytes(b[28..32].try_into().unwrap());
    let flags = u32::from_le_bytes(b[80..84].try_into().unwrap());
    let fourcc = if flags & 4 != 0 { String::from_utf8_lossy(&b[84..88]).trim_end_matches('\0').to_string() } else { "RGB".to_string() };
    Some((fourcc, w, h, mips))
}

/// The kind a path falls in: its first three folders (`Skins/ManiaKit/OnTerrain`).
fn kind_of(name: &str) -> String {
    let parts: Vec<&str> = name.split('/').collect();
    match parts.len() {
        0 | 1 => "(root)".to_string(),
        2 => parts[0].to_string(),
        3 => format!("{}/{}", parts[0], parts[1]),
        _ => format!("{}/{}/{}", parts[0], parts[1], parts[2]),
    }
}

/// Every `(slot id, Skins\… path)` a decompressed body names: a
/// `CPlugMaterialUserInst` texture record is `u32 slot, u32 len, "Skins\…"`.
fn texture_refs(body: &[u8]) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let needle = b"Skins\\";
    let mut i = 0usize;
    while i + 8 + needle.len() <= body.len() {
        if &body[i + 8..i + 8 + needle.len()] == needle {
            let len = u32::from_le_bytes(body[i + 4..i + 8].try_into().unwrap()) as usize;
            if len >= needle.len() && len < 300 && i + 8 + len <= body.len() {
                if let Ok(s) = std::str::from_utf8(&body[i + 8..i + 8 + len]) {
                    let slot = u32::from_le_bytes(body[i..i + 4].try_into().unwrap());
                    out.push((slot, s.to_string()));
                    i += 8 + len;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// Material model names (`TDSN`, `TDSNI_Night`, …) as length-prefixed strings.
fn material_models(body: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    for m in ["TDSN_Night", "TDSNI_Night", "TDSNI", "TDSN", "TDOSN", "TDOBSN", "TIAdd", "TAdd"] {
        let nb = m.as_bytes();
        let mut i = 0;
        while i + nb.len() <= body.len() {
            match body[i..].windows(nb.len()).position(|w| w == nb) {
                None => break,
                Some(p) => {
                    let at = i + p;
                    if at >= 4 && u32::from_le_bytes(body[at - 4..at].try_into().unwrap()) as usize == nb.len() {
                        out.push(m.to_string());
                    }
                    i = at + nb.len();
                }
            }
        }
    }
    out
}

fn line(t: &Tally, total: u64, k: &str) -> String {
    format!("  {:>5} files  {:>12} raw  {:>12} deflated  {:>5.1} %  {}", t.n, mib(t.raw), mib(t.deflated), 100.0 * t.deflated as f64 / total.max(1) as f64, k)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let path = args.first().ok_or("mapsize MAP.Map.Gbx [--top N] [--tsv OUT.tsv]")?;
    let top: usize = args.iter().position(|a| a == "--top").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(15);
    let tsv = args.iter().position(|a| a == "--tsv").and_then(|i| args.get(i + 1)).cloned();
    let file = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let g = Gbx::parse(&file);
    println!("{path}: file {} ({} B); body {} decompressed", mib(file.len() as u64), file.len(), mib(g.body.len() as u64));
    let (zip, _) = tmmaps::header::embedded_zip_bytes(&g.body).ok_or("no embedded-objects archive (chunk 0x03043054)")?;
    let entries = zip_sizes(&zip);
    let zip_total: u64 = entries.iter().map(|e| e.1.deflated + 76 + 2 * e.0.len() as u64).sum();
    println!("embedded archive: {} entries, {} = {:.0} % of the file (the rest: blocks, items, lightmap, thumbnail)", entries.len(), mib(zip_total), 100.0 * zip_total as f64 / file.len() as f64);

    // 1. by type and folder
    let mut kinds: BTreeMap<String, Tally> = BTreeMap::new();
    let mut exts: BTreeMap<String, Tally> = BTreeMap::new();
    for (name, e, _, _) in &entries {
        kinds.entry(kind_of(name)).or_default().add(e);
        let lower = name.to_ascii_lowercase();
        let ext = if lower.ends_with(".item.gbx") {
            "Item.Gbx".to_string()
        } else if lower.ends_with(".block.gbx") {
            "Block.Gbx".to_string()
        } else {
            lower.rsplit('.').next().unwrap_or("").to_string()
        };
        exts.entry(ext).or_default().add(e);
    }
    println!("\n== by file type (deflated is what the file pays) ==");
    let mut ev: Vec<_> = exts.iter().collect();
    ev.sort_by(|a, b| b.1.deflated.cmp(&a.1.deflated));
    for (k, t) in ev {
        println!("{}", line(t, zip_total, k));
    }
    println!("\n== by folder ==");
    let mut kv: Vec<_> = kinds.iter().collect();
    kv.sort_by(|a, b| b.1.deflated.cmp(&a.1.deflated));
    for (k, t) in kv {
        println!("{}", line(t, zip_total, k));
    }

    // 2. DDS census, content hashes, bodies
    let mut dds: BTreeMap<String, Tally> = BTreeMap::new();
    let mut by_hash: HashMap<u64, Vec<String>> = HashMap::new();
    let mut dup_bytes = 0u64;
    let mut tex_entries: HashMap<String, Entry> = HashMap::new();
    let mut refs: Vec<(String, u32, String)> = Vec::new();
    let mut models: BTreeMap<String, u64> = BTreeMap::new();
    let mut gbx_bodies = 0usize;
    for (name, e, start, method) in &entries {
        let lower = name.to_ascii_lowercase();
        let bytes = entry_bytes(&zip, *start, e, *method);
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in &bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        let v = by_hash.entry(h).or_default();
        if !v.is_empty() {
            dup_bytes += e.deflated;
        }
        v.push(name.clone());
        if lower.ends_with(".dds") {
            let key = match dds_info(&bytes) {
                Some((fourcc, w, h, mips)) => format!("{fourcc} {w}x{h} ({mips} mips)"),
                None => "(not a DDS header)".into(),
            };
            dds.entry(key).or_default().add(e);
            let k = name[..name.len() - 4].replace('/', "\\");
            tex_entries.insert(k, e.clone());
        } else if lower.ends_with(".gbx") {
            let ig = Gbx::parse(&bytes);
            gbx_bodies += 1;
            for (slot, p) in texture_refs(&ig.body) {
                let k = p.strip_suffix(".dds").or_else(|| p.strip_suffix(".DDS")).unwrap_or(&p).to_string();
                refs.push((name.clone(), slot, k));
            }
            for m in material_models(&ig.body) {
                *models.entry(m).or_default() += 1;
            }
        }
    }
    println!("\n== textures (DDS) by format and size ==");
    let mut dv: Vec<_> = dds.iter().collect();
    dv.sort_by(|a, b| b.1.deflated.cmp(&a.1.deflated));
    for (k, t) in dv {
        println!("{}", line(t, zip_total, k));
    }
    let dups: Vec<_> = by_hash.values().filter(|v| v.len() > 1).collect();
    println!("\n== byte-identical duplicates: {} groups, {} saved if shared ==", dups.len(), mib(dup_bytes));
    for v in dups.iter().take(top) {
        println!("  x{}  {}", v.len(), v[0]);
    }

    // 3. references
    println!("\n== material model strings in the {gbx_bodies} item/block bodies (first spellings; later uses are lookback refs) ==");
    for (m, n) in &models {
        println!("  {:>5}  {}", n, m);
    }
    // a texture counts once per slot it is used under (the same file referenced by
    // 200 items is 200 refs but one entry)
    let mut per_slot: BTreeMap<u32, (u64, Tally)> = BTreeMap::new();
    let mut ref_count: HashMap<String, u32> = HashMap::new();
    let mut missing: BTreeMap<String, u32> = BTreeMap::new();
    let mut seen_slot: std::collections::HashSet<(u32, String)> = std::collections::HashSet::new();
    for (_, slot, k) in &refs {
        *ref_count.entry(k.clone()).or_default() += 1;
        match tex_entries.get(k) {
            Some(e) => {
                let ps = per_slot.entry(*slot).or_default();
                ps.0 += 1;
                if seen_slot.insert((*slot, k.clone())) {
                    ps.1.add(e);
                }
            }
            None => *missing.entry(k.clone()).or_default() += 1,
        }
    }
    let referenced: u64 = tex_entries.iter().filter(|(k, _)| ref_count.contains_key(*k)).map(|(_, e)| e.deflated).sum();
    let unreferenced: Vec<(&String, &Entry)> = tex_entries.iter().filter(|(k, _)| !ref_count.contains_key(*k)).collect();
    let unref_bytes: u64 = unreferenced.iter().map(|(_, e)| e.deflated).sum();
    println!("\n== texture references from the bodies: {} refs to {} distinct textures ==", refs.len(), ref_count.len());
    for (slot, (nrefs, t)) in &per_slot {
        println!("  slot {:>2}: {:>5} refs to {:>5} distinct textures  {:>12} deflated", slot, nrefs, t.n, mib(t.deflated));
    }
    let shared = ref_count.values().filter(|n| **n > 1).count();
    println!("  textures referenced by more than one material: {shared}");
    println!("  embedded textures some body names: {}", mib(referenced));
    println!("  embedded textures NO body names: {} files, {}", unreferenced.len(), mib(unref_bytes));
    let mut ul: Vec<_> = unreferenced.iter().collect();
    ul.sort_by(|a, b| b.1.deflated.cmp(&a.1.deflated));
    for (k, e) in ul.iter().take(top.min(8)) {
        println!("    {:>12} deflated  {}", mib(e.deflated), k);
    }
    if !missing.is_empty() {
        println!("  names with no entry in the archive (pack / skin-folder textures?): {}", missing.len());
        for (k, n) in missing.iter().take(top) {
            println!("    x{n}  {k}");
        }
    }

    // 4. top entries
    println!("\n== top {top} entries by deflated size ==");
    let mut sorted: Vec<_> = entries.iter().collect();
    sorted.sort_by(|a, b| b.1.deflated.cmp(&a.1.deflated));
    for (name, e, _, _) in sorted.iter().take(top) {
        println!("  {:>12} deflated  {:>12} raw  {}", mib(e.deflated), mib(e.raw), name);
    }

    if let Some(t) = tsv {
        let mut s = String::from("entry\traw\tdeflated\tkind\tdds\trefs\n");
        for (name, e, start, method) in &entries {
            let lower = name.to_ascii_lowercase();
            let d = if lower.ends_with(".dds") {
                dds_info(&entry_bytes(&zip, *start, e, *method)).map(|(f, w, h, m)| format!("{f} {w}x{h} {m}")).unwrap_or_default()
            } else {
                String::new()
            };
            let k = if lower.ends_with(".dds") { name[..name.len() - 4].replace('/', "\\") } else { String::new() };
            s += &format!("{}\t{}\t{}\t{}\t{}\t{}\n", name, e.raw, e.deflated, kind_of(name), d, ref_count.get(&k).copied().unwrap_or(0));
        }
        std::fs::write(&t, s).map_err(|e| format!("{t}: {e}"))?;
        println!("\nwrote {t}");
    }
    Ok(())
}
