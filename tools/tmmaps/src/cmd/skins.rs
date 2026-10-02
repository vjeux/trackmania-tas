//! Car-skin PackDescs (FileRefs) anywhere in a map's body, and the header's
//! `<dep>` list that mirrors them.
//!
//! Written for Fall 2026 map 22 "United Kingdom 2026" (2026-10-02): the only
//! one of the 25 sources whose header declares a CAR skin
//! (`Skins\Models\CarSport\Chraz21_by_SMSv6_<uuid>.zip`, locator
//! `core.trackmania.nadeo.live/storageObjects/<uuid>`), and the only
//! conversion the client never opens in play ("Updating data…" forever).
//!
//! The carrier is NOT the validation ghost: the source's ghost (0x0305B00F)
//! wears the stock `Stadium.zip` + `Stadium_FRA.zip`, and `tmmaps tiny` drops
//! that chunk anyway. It is the in-game MediaTracker clip "Trigger 1", track
//! "TheChraz": two `CGameCtnMediaBlockEntity` blocks (0x0329F000) whose skin
//! PackDesc carries an ALL-ZERO checksum plus the locator — the shape of the
//! 2026-09-12 "Updating data… (???)" loop (AutoUpdateFromLocator on a zip
//! whose hash never matches). The tiny/giant converter keeps MT entity blocks
//! verbatim, so the PackDesc — and the header `<dep>` the game derived from it
//! — ride into every tiny and giant 22.
//!
//!   tmmaps packdescs MAP [--filter SUBSTR]
//!       every FileRef v3 in the body whose path looks like a skin/asset path,
//!       with its body offset, checksum, path and locator. The evidence line:
//!       run it on the source and on the conversion and compare checksums.
//!   tmmaps packdescs MAP --out F --neutralise SUBSTR [--to stock|empty]
//!       every matching PackDesc rewritten to the stock car skin (the bytes the
//!       game itself wrote for the validation ghost's `Stadium.zip`: version 3,
//!       checksum 02 00…00, no locator) or to the empty FileRef. A
//!       variable-length splice: nothing else edited in the same write.
//!   tmmaps deps MAP [--out F --drop SUBSTR [--drop SUBSTR …]]
//!       the header XML's `<dep file= url=>` entries; with --out, the matching
//!       ones removed (header-only edit, the body is untouched byte for byte).

use std::path::Path;
use tmmaps::cli::{die, flag, flag_multi, has};
use tmmaps::header::FileRef;
use tmmaps::map;

/// The checksum the game writes for a skin that lives in its own pak
/// (`Skins\Models\CarSport\Stadium.zip` on every Fall 2026 validation ghost):
/// one 0x02 byte, then zeros. Not a SHA-256 of anything — a "stock" marker.
pub const STOCK_CHECKSUM: [u8; 32] = {
    let mut c = [0u8; 32];
    c[0] = 2;
    c
};
pub const STOCK_SKIN: &str = "Skins\\Models\\CarSport\\Stadium.zip";

/// A FileRef found in the body: where it starts and how long it is.
#[derive(Clone, Debug)]
pub struct Found {
    pub at: usize,
    pub len: usize,
    pub fr: FileRef,
}

/// Every FileRef v3 whose path starts with `Skins\` or `Items\` (the asset
/// roots the game uses) — found by the path string, then validated by reading
/// the structure backwards (version byte 3, 32 checksum bytes, the length
/// word) and forwards (the locator string). Structural, not a string grep:
/// the `strings` listing finds the same paths, but cannot say whether a
/// checksum is zero.
pub fn scan(body: &[u8]) -> Vec<Found> {
    let mut out = Vec::new();
    let roots: [&[u8]; 2] = [b"Skins\\", b"Items\\"];
    let mut i = 37usize; // a v3 FileRef needs 1 + 32 + 4 bytes before the path
    while i + 6 <= body.len() {
        let hit = roots.iter().any(|r| body[i..].starts_with(r));
        if !hit {
            i += 1;
            continue;
        }
        let len = u32::from_le_bytes(body[i - 4..i].try_into().unwrap()) as usize;
        let start = i - 37;
        if len == 0 || len > 512 || i + len > body.len() || body[start] != 3 {
            i += 1;
            continue;
        }
        if !body[i..i + len].iter().all(|c| *c >= 0x20 && *c != 0x7f) {
            i += 1;
            continue;
        }
        match FileRef::decode(&body[start..]) {
            Some((fr, n)) if fr.path.len() == len && fr.url.len() <= 1024 && fr.url.chars().all(|c| !c.is_control()) => {
                out.push(Found { at: start, len: n, fr });
                i = start + n;
            }
            _ => i += 1,
        }
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn checksum_word(c: &[u8; 32]) -> String {
    if c.iter().all(|b| *b == 0) {
        "ZERO".to_string()
    } else if *c == STOCK_CHECKSUM {
        "stock(02 00…)".to_string()
    } else {
        hex(c)
    }
}

/// `tmmaps packdescs MAP [--filter SUBSTR] [--out F --neutralise SUBSTR [--to stock|empty]]`
pub fn packdescs(args: &[String]) {
    let path = Path::new(&args[2]);
    let filter = flag(args, "--filter").map(String::from);
    let neutralise = flag(args, "--neutralise").or_else(|| flag(args, "--neutralize")).map(String::from);
    let to = flag(args, "--to").unwrap_or("stock");
    if !matches!(to, "stock" | "empty") {
        die("--to wants `stock` (the game's own Stadium.zip FileRef) or `empty` (no path, zero checksum)");
    }
    let mut m = map::MapFile::load(path);
    let found = scan(&m.gbx.body);
    let shown: Vec<&Found> = found.iter().filter(|f| filter.as_deref().map(|w| f.fr.path.contains(w) || f.fr.url.contains(w)).unwrap_or(true)).collect();
    println!("=== {}  {} FileRef(s) in the body{}", path.display(), found.len(), filter.as_deref().map(|w| format!(", {} matching {w:?}", shown.len())).unwrap_or_default());
    println!("offset\tbytes\tversion\tchecksum\tpath\tlocator");
    for f in &shown {
        println!("{}\t{}\t{}\t{}\t{}\t{}", f.at, f.len, f.fr.version, checksum_word(&f.fr.checksum), f.fr.path, f.fr.url);
    }
    let Some(pat) = neutralise else { return };
    let out = flag(args, "--out").unwrap_or_else(|| die("--neutralise wants --out F"));
    let targets: Vec<&Found> = found.iter().filter(|f| f.fr.path.contains(&pat) || f.fr.url.contains(&pat)).collect();
    if targets.is_empty() {
        die(&format!("no FileRef in {} matches {pat:?} — nothing to neutralise", path.display()));
    }
    let replacement = match to {
        "stock" => FileRef { version: 3, checksum: STOCK_CHECKSUM, path: STOCK_SKIN.to_string(), url: String::new() },
        _ => FileRef { version: 3, checksum: [0u8; 32], path: String::new(), url: String::new() },
    };
    let bytes = replacement.encode();
    for f in &targets {
        println!("  neutralise @{}: {} bytes {:?} ({}) -> {} bytes {:?} ({})", f.at, f.len, f.fr.path, checksum_word(&f.fr.checksum), bytes.len(), replacement.path, checksum_word(&replacement.checksum));
        m.raw_splices.push(((f.at, f.at + f.len), bytes.clone()));
    }
    let outp = Path::new(out);
    m.write_to(outp).unwrap_or_else(|e| die(&format!("{out}: {e}")));
    // read back: the pattern must be gone, the replacement must decode
    let check = map::MapFile::load(outp);
    let after = scan(&check.gbx.body);
    let left = after.iter().filter(|f| f.fr.path.contains(&pat) || f.fr.url.contains(&pat)).count();
    if left != 0 {
        die(&format!("{out}: {left} FileRef(s) still match {pat:?} after the write"));
    }
    let strings_left = tmmaps_strings_containing(&check.gbx.body, &pat);
    println!("wrote {out}: {} FileRef(s) neutralised to {to}; {} FileRef(s) read back; {} body string(s) still contain {pat:?}", targets.len(), after.len(), strings_left);
    if strings_left != 0 {
        die(&format!("{out}: the pattern survives in {strings_left} body string(s) outside a FileRef (a ghost nickname or a track name is not a skin — use `tmmaps strings --grep` to see them)"));
    }
}

/// Length-prefixed body strings containing `pat` (the `strings` scan).
fn tmmaps_strings_containing(b: &[u8], pat: &str) -> usize {
    let mut n = 0usize;
    let mut i = 0usize;
    while i + 4 <= b.len() {
        let len = u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
        if len >= 4 && len <= 512 && i + 4 + len <= b.len() {
            let s = &b[i + 4..i + 4 + len];
            if s.iter().all(|c| (*c >= 0x20 && *c != 0x7f) || *c == b'\t') {
                if let Ok(t) = std::str::from_utf8(s) {
                    if t.contains(pat) {
                        n += 1;
                    }
                    i += 4 + len;
                    continue;
                }
            }
        }
        i += 1;
    }
    n
}

/// The header XML's `<dep …/>` elements, whole.
pub fn dep_elements(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(p) = xml[i..].find("<dep ") {
        let s = i + p;
        let Some(e) = xml[s..].find("/>") else { break };
        out.push(xml[s..s + e + 2].to_string());
        i = s + e + 2;
    }
    out
}

fn dep_attr(el: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    el.find(&key).and_then(|a| {
        let a = a + key.len();
        el[a..].find('"').map(|b| el[a..a + b].to_string())
    }).unwrap_or_default()
}

/// `tmmaps deps MAP [--out F --drop SUBSTR [--drop SUBSTR …]]`
pub fn deps(args: &[String]) {
    let path = Path::new(&args[2]);
    let drops = flag_multi(args, "--drop");
    let mut m = map::MapFile::load(path);
    let chunks = tmmaps::header::user_chunks(&m.gbx.user_data).unwrap_or_default();
    let xml = tmmaps::header::header_xml(&chunks).unwrap_or_default();
    let els = dep_elements(&xml);
    println!("=== {}  {} <dep> in the header XML", path.display(), els.len());
    println!("file\turl");
    for el in &els {
        println!("{}\t{}", dep_attr(el, "file"), dep_attr(el, "url"));
    }
    if drops.is_empty() {
        if has(args, "--out") {
            die("--out wants at least one --drop SUBSTR");
        }
        return;
    }
    let out = flag(args, "--out").unwrap_or_else(|| die("--drop wants --out F"));
    let matching: Vec<&String> = els.iter().filter(|el| drops.iter().any(|d| el.contains(d.as_str()))).collect();
    if matching.is_empty() {
        die(&format!("no <dep> in {} matches {:?} — nothing to drop", path.display(), drops));
    }
    for el in &matching {
        println!("  drop {}", dep_attr(el, "file"));
    }
    let drops2 = drops.clone();
    let changed = m.edit_header_xml(&|s: &str| {
        let mut s = s.to_string();
        for el in dep_elements(&s) {
            if drops2.iter().any(|d| el.contains(d.as_str())) {
                s = s.replacen(&el, "", 1);
            }
        }
        Some(s)
    });
    if !changed {
        die("the header XML did not change");
    }
    let outp = Path::new(out);
    m.write_to(outp).unwrap_or_else(|e| die(&format!("{out}: {e}")));
    let check = map::MapFile::load(outp);
    let xml2 = tmmaps::header::header_xml(&tmmaps::header::user_chunks(&check.gbx.user_data).unwrap_or_default()).unwrap_or_default();
    let left = dep_elements(&xml2).into_iter().filter(|el| drops.iter().any(|d| el.contains(d.as_str()))).count();
    if left != 0 {
        die(&format!("{out}: {left} <dep> still match after the write"));
    }
    let body_same = check.gbx.body == m.gbx.body;
    println!("wrote {out}: {} <dep> dropped, {} left; body {}", matching.len(), dep_elements(&xml2).len(), if body_same { "byte-identical" } else { "CHANGED (unexpected for a header-only edit)" });
    if !body_same {
        std::process::exit(1);
    }
}
