//! Build a complete tiny-map item library from the game's client packs.
//!
//! Modern block placements become thin CGameItemModel wrappers around the exact
//! prefab selected by the source map. The three legacy, prefab-less models used
//! by Summer 01 come from the public Nadeo converted-item archive; their Ident
//! strings are rewritten here, in Rust. An intentionally empty pillar mobil is
//! represented by the bundled empty item template so it remains an explicit
//! placement rather than a silent omission.

use crate::{embedded, store::DataStore};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use tmmaps::{gbx::Gbx, map::MapFile};

pub const AUTHOR: &str = "KTaOsd-lTR2zkoskETSfPA";
pub const BLUEBAY_KEY: &str = "660C4C156B80337E296A1034B0AA05B8";
pub const STADIUM_KEY: &str = "B773D73047A4104857722366D78D28A6";

const LEGACY_PLATFORM: &str = "Walls/DecoWall/Platform/Platform/PlatformBase.Item.Gbx";
const LEGACY_SLOPE: &str = "Walls/DecoWall/Unnamed_1/SlopeStraight/DecoWallSlope2Straight.Item.Gbx";
const LEGACY_WALL: &str = "Walls/DecoWall/Platform/Platform/DecoWallBase.Item.Gbx";

fn put_string(v: &mut Vec<u8>, s: &str) {
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s.as_bytes());
}

/// One external file reference, relative to an Item in `Items/`.
fn one_ref(path: &str) -> Vec<u8> {
    let p = path.replace('/', "\\");
    let mut parts: Vec<&str> = p.split('\\').collect();
    let file = parts.pop().expect("prefab filename");
    let mut v = Vec::new();
    v.extend_from_slice(&1u32.to_le_bytes()); // external count
    v.extend_from_slice(&1u32.to_le_bytes()); // ancestor: Items -> archive root
    v.extend_from_slice(&1u32.to_le_bytes()); // one root folder
    for (i, part) in parts.iter().enumerate() {
        put_string(&mut v, part);
        v.extend_from_slice(&(if i + 1 < parts.len() { 1u32 } else { 0 }).to_le_bytes());
    }
    v.extend_from_slice(&0u32.to_le_bytes()); // flags
    put_string(&mut v, file);
    v.extend_from_slice(&1u32.to_le_bytes()); // node index
    v.extend_from_slice(&0u32.to_le_bytes()); // useFile=false
    v.extend_from_slice(&(parts.len() as u32).to_le_bytes());
    v
}

/// Replace every length-prefixed UTF-8 occurrence. GBX strings and new lookback
/// definitions both put their byte length immediately before their contents.
fn replace_lp(mut b: Vec<u8>, old: &str, new: &str) -> Vec<u8> {
    let oldb = old.as_bytes();
    let mut hits = Vec::new();
    for i in 4..=b.len().saturating_sub(oldb.len()) {
        if &b[i..i + oldb.len()] == oldb
            && u32::from_le_bytes(b[i - 4..i].try_into().unwrap()) as usize == oldb.len()
        {
            hits.push(i);
        }
    }
    assert!(!hits.is_empty(), "length-prefixed string {old:?} is absent");
    for i in hits.into_iter().rev() {
        b[i - 4..i].copy_from_slice(&(new.len() as u32).to_le_bytes());
        b.splice(i..i + oldb.len(), new.as_bytes().iter().copied());
    }
    b
}

fn rewrite_ident(bytes: &[u8], old_name: &str, alias: &str, old_author: &str) -> Vec<u8> {
    let mut g = Gbx::parse(bytes);
    g.user_data = replace_lp(g.user_data, old_name, alias);
    g.body = replace_lp(g.body, old_name, alias);
    g.user_data = replace_lp(g.user_data, old_author, AUTHOR);
    g.body = replace_lp(g.body, old_author, AUTHOR);
    let body = g.body.clone();
    g.write_body_recompressed(&body)
}

fn wrapper(template: &[u8], alias: &str, prefab: &str) -> Vec<u8> {
    let mut g = Gbx::parse(template);
    g.user_data = replace_lp(g.user_data, "GateSupport", alias);
    g.body = replace_lp(g.body, "GateSupport", alias);
    g.user_data = replace_lp(g.user_data, "Nadeo", AUTHOR);
    g.body = replace_lp(g.body, "Nadeo", AUTHOR);
    g.ref_table = one_ref(prefab);
    let body = g.body.clone();
    g.write_body_recompressed(&body)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for &x in bytes {
        c ^= x as u32;
        for _ in 0..8 {
            c = (c >> 1) ^ ((0u32.wrapping_sub(c & 1)) & 0xEDB_88320);
        }
    }
    !c
}

/// Minimal stored ZIP with explicit directory rows for Trackmania's browser.
fn zip(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    // Trackmania's embedded-file browser walks explicit ZIP directory rows; a
    // standards-valid flat archive is readable by ordinary unzip tools but its
    // files are invisible to the game.
    let mut entries = BTreeMap::new();
    for (name, data) in files {
        for (i, _) in name.match_indices('/') {
            entries
                .entry(name[..=i].to_string())
                .or_insert_with(Vec::new);
        }
        entries.insert(name.clone(), data.clone());
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in &entries {
        let off = out.len() as u32;
        let n = name.as_bytes();
        let crc = crc32(data);
        out.extend_from_slice(b"PK\x03\x04");
        for x in [10u16, 0, 0, 0, 0] {
            out.extend_from_slice(&x.to_le_bytes());
        }
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(data);

        central.extend_from_slice(b"PK\x01\x02");
        for x in [10u16, 10, 0, 0, 0, 0] {
            central.extend_from_slice(&x.to_le_bytes());
        }
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(n.len() as u16).to_le_bytes());
        for x in [0u16, 0, 0, 0] {
            central.extend_from_slice(&x.to_le_bytes());
        }
        central.extend_from_slice(&(if name.ends_with('/') { 0x10u32 } else { 0 }).to_le_bytes());
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(n);
    }
    let central_off = out.len() as u32;
    let central_len = central.len() as u32;
    out.extend(central);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&central_len.to_le_bytes());
    out.extend_from_slice(&central_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn fallback_alias(name: &str, flags: u32) -> Option<&'static str> {
    match name {
        "PlatformBase" => Some("AC00000101"),
        "DecoWallSlope2Straight" => Some("AC00000102"),
        "DecoWallBasePillar" => Some("AC00000103"),
        "StructurePillar" if flags == 0x0000_4001 => Some("AC00000104"),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build(
    map: &Path,
    catalog: &Path,
    footprints: &Path,
    nadeo_zip: &Path,
    empty_template: &Path,
    blue_pak: &Path,
    stadium_pak: &Path,
    out_zip: &Path,
    out_map: &Path,
) {
    let source = MapFile::load(map);
    let mut catalog_map = BTreeMap::new();
    for (line_no, line) in fs::read_to_string(catalog).unwrap().lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 3 {
            panic!(
                "{}:{}: expected NAME<TAB>FLAGS<TAB>PREFAB",
                catalog.display(),
                line_no + 1
            );
        }
        let flags = u32::from_str_radix(f[1], 16).expect("hex block flags");
        assert!(catalog_map
            .insert((f[0].to_string(), flags), f[2].to_string())
            .is_none());
    }

    let mut footprint_map: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for (line_no, line) in fs::read_to_string(footprints).unwrap().lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 3 {
            panic!(
                "{}:{}: expected NAME<TAB>SX<TAB>SZ",
                footprints.display(),
                line_no + 1
            );
        }
        let sx: u32 = f[1].parse().expect("SX cells");
        let sz: u32 = f[2].parse().expect("SZ cells");
        assert!(footprint_map
            .insert(f[0].to_string(), (sx, sz))
            .is_none());
    }
    for b in &source.blocks {
        assert!(
            footprint_map.contains_key(&b.name),
            "no footprint for block model {}",
            b.name
        );
    }

    let mut paths = BTreeSet::new();
    for b in &source.blocks {
        if fallback_alias(&b.name, b.flags).is_none() {
            let p = catalog_map
                .get(&(b.name.clone(), b.flags))
                .unwrap_or_else(|| {
                    panic!(
                        "no resolved prefab for block#{} {} flags {:08X}",
                        b.index, b.name, b.flags
                    )
                });
            paths.insert(p.clone());
        }
    }
    let aliases: BTreeMap<String, String> = paths
        .into_iter()
        .enumerate()
        .map(|(i, p)| (p, format!("AC{i:08}")))
        .collect();
    assert_eq!(aliases.len(), 101, "Summer 01 direct-prefab count changed");

    let mut blue = DataStore::open(&[blue_pak.display().to_string()], BLUEBAY_KEY).unwrap();
    let mut stadium = DataStore::open(&[stadium_pak.display().to_string()], STADIUM_KEY).unwrap();
    let template = stadium
        .read("Stadium\\Items\\GateSupport.Item.Gbx")
        .unwrap();
    let mut files = BTreeMap::new();
    for (prefab, alias) in &aliases {
        files.insert(
            format!("C:/Users/vjeux/Documents/Trackmania/Items/{alias}.Item.Gbx"),
            wrapper(&template, &format!("{alias}.Item.Gbx"), prefab),
        );
        let bytes = if prefab.starts_with("BlueBay\\") {
            blue.read(prefab)
        } else {
            stadium.read(prefab)
        }
        .unwrap_or_else(|e| panic!("{prefab}: {e}"));
        files.insert(
            format!(
                "C:/Users/vjeux/Documents/Trackmania/{}",
                prefab.replace('\\', "/")
            ),
            bytes,
        );
    }

    let legacy = embedded::unzip(&fs::read(nadeo_zip).unwrap()).expect("read Nadeo item archive");
    for (src, alias) in [
        (LEGACY_PLATFORM, "AC00000101"),
        (LEGACY_SLOPE, "AC00000102"),
        (LEGACY_WALL, "AC00000103"),
    ] {
        let bytes = legacy
            .get(src)
            .unwrap_or_else(|| panic!("{src} absent from Nadeo archive"));
        files.insert(
            format!("C:/Users/vjeux/Documents/Trackmania/Items/{alias}.Item.Gbx"),
            rewrite_ident(
                bytes,
                "New Item",
                &format!("{alias}.Item.Gbx"),
                "UVkE4dP3TEmNl3yiI40gVg",
            ),
        );
    }
    let empty = fs::read(empty_template).unwrap();
    files.insert(
        "C:/Users/vjeux/Documents/Trackmania/Items/AC00000104.Item.Gbx".into(),
        rewrite_ident(&empty, "AC000000104", "AC00000104.Item.Gbx", AUTHOR),
    );

    let archive = zip(&files);
    fs::write(out_zip, &archive).unwrap();
    let mut mapping = String::new();
    for b in &source.blocks {
        let alias = fallback_alias(&b.name, b.flags)
            .unwrap_or_else(|| aliases[&catalog_map[&(b.name.clone(), b.flags)]].as_str());
        let (sx, sz) = footprint_map[&b.name];
        mapping.push_str(&format!(
            "@{}\t{}.Item.Gbx\t1\t{}\t{}\n",
            b.index, alias, sx, sz
        ));
    }
    let mapping_path = out_zip.with_extension("placements.tsv");
    fs::write(&mapping_path, mapping).unwrap();
    let args = vec![
        "tmmaps".into(),
        "tiny".into(),
        map.display().to_string(),
        "--out".into(),
        out_map.display().to_string(),
        "--mapping".into(),
        mapping_path.display().to_string(),
        "--library".into(),
        out_zip.display().to_string(),
        "--scale".into(),
        "0.5".into(),
    ];
    tmmaps::tiny::cmd(&args);
    println!(
        "  generated {} item wrappers + {} prefab/support files",
        105,
        files.len() - 105
    );
    println!("  library: {} ({} bytes)", out_zip.display(), archive.len());
}
