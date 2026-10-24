//! `tmmaps live-vs-build --live DIR --build DIR --out SUMMARY.tsv [--detail DIR]` — which
//! maps of a set a rebuild CHANGES against what is live (2026-10-17; was a shell loop
//! of `tmmaps census | sort | command diff` on 2026-10-09 — the shell-aliased `diff -u`
//! made 950 maps read as 0 differences once, and vjeux's rule is Rust only).
//!
//! Both directories hold `*.Map.Gbx` files anywhere below them; a file's MAP NUMBER
//! is read from its name (`U10S_56 …`, `06-U10S_56-By-…`, `U10S_056`). For every
//! number present on both sides the two maps are reduced to a CENSUS — one line per
//! unbaked block, baked block and item — and the two censuses compared as multisets.
//! Normalised away, because they differ between any two bakes of the same content:
//! the embedded item idents (`AC01700055`, `AI00400012` → `AC`, `AI`), and the DecoWall
//! terrain suffix the game appends (`DecoWallBase_Grass` → `DecoWallBase`). Positions
//! are kept to the millimetre (a moved deck is a change). The EMBEDDED ITEMS' CONTENT
//! counts too: a texture fix inside an item (the 2026-10-10 TrackWall dress) moves no
//! placement, so each `Items/*.Item.Gbx` of the zip is fingerprinted — FNV-64 of its
//! bytes with every occurrence of its own ident (`AC01700055`) blanked — and the two
//! fingerprint multisets compared; a map whose items differ is DIFF with `only_live`
//! / `only_build` counting those items as rows `Z <kind> <fingerprint>`.
//!
//! SUMMARY.tsv: `num  verdict  live_rows  build_rows  only_live  only_build  live_file
//! build_file`, verdict SAME | DIFF | LIVE-ONLY | BUILD-ONLY. `--detail DIR` writes
//! `DIR/<num>.diff` with the differing lines (`< live`, `> build`) for every DIFF map.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::map::MapFile;

fn map_number(name: &str) -> Option<u32> {
    // the first `U10S_<digits>` (case-insensitive), else the first `_<digits>` run
    let lower = name.to_ascii_lowercase();
    let i = lower.find("u10s_")?;
    let digits: String = lower[i + 5..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn collect(dir: &Path) -> BTreeMap<u32, PathBuf> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().and_then(|n| n.to_str()).map(|n| n.ends_with(".Map.Gbx")).unwrap_or(false) {
                if let Some(n) = p.file_name().and_then(|n| n.to_str()).and_then(map_number) {
                    out.entry(n).or_insert(p);
                }
            }
        }
    }
    out
}

fn norm_name(n: &str) -> String {
    // embedded item idents: two letters + 8 digits
    let b = n.as_bytes();
    if b.len() == 10 && b[0].is_ascii_uppercase() && b[1].is_ascii_uppercase() && b[2..].iter().all(|c| c.is_ascii_digit()) {
        return n[..2].to_string();
    }
    // the DecoWall terrain suffix
    if n.starts_with("DecoWall") {
        if let Some(i) = n.rfind('_') {
            return n[..i].to_string();
        }
    }
    n.to_string()
}

/// One census line per block / baked block / item, in a form two bakes agree on.
pub fn census_lines(m: &MapFile) -> Vec<String> {
    let mut out = Vec::new();
    // an embedded item's ALIAS is a numbering, not an identity: a library whose key set
    // grew renumbers every alias after the new keys (Magnet 3, 2026-10-24: 51 placements
    // moved from AC03700000… to AC03700008… with byte-identical items) — a placement is
    // compared by the FINGERPRINT of the item it places
    let alias_fp: std::collections::HashMap<String, String> = m
        .embedded_objects()
        .map(|(_, zip)| {
            crate::header::zip_entries(&zip)
                .into_iter()
                .filter_map(|(name, bytes)| {
                    let base = name.rsplit('/').next().unwrap_or(&name).to_string();
                    let stem = base.trim_end_matches(".Item.Gbx").trim_end_matches(".Block.Gbx").to_string();
                    base.ends_with(".Item.Gbx").then(|| (base.clone(), format!("fp:{:016x}", fingerprint(&bytes, stem.as_bytes()))))
                })
                .collect()
        })
        .unwrap_or_default();
    for b in &m.blocks {
        let (cx, cy, cz) = b.coords();
        let pos = b.free_pos.map(|p| format!("{:.3},{:.3},{:.3}", p[0], p[1], p[2])).unwrap_or_default();
        out.push(format!("U\t{}\t{cx},{cy},{cz}\t{}\t{:08X}\t{pos}", norm_name(&b.name), b.dir, b.flags));
    }
    for b in &m.baked {
        let (cx, cy, cz) = b.coords();
        let pos = b.free_pos.map(|p| format!("{:.3},{:.3},{:.3}", p[0], p[1], p[2])).unwrap_or_default();
        out.push(format!("B\t{}\t{cx},{cy},{cz}\t{}\t{:08X}\t{pos}", norm_name(&b.name), b.dir, b.flags));
    }
    for it in &m.items {
        let model = alias_fp.get(&it.model).cloned().unwrap_or_else(|| norm_name(&it.model));
        out.push(format!("I\t{}\t{:.3},{:.3},{:.3}\t{:.4},{:.4},{:.4}\t{:08X}", model, it.pos[0], it.pos[1], it.pos[2], it.yaw, it.pitch, it.roll, it.flags));
    }
    if let Some((_, zip)) = m.embedded_objects() {
        for (name, bytes) in crate::header::zip_entries(&zip) {
            let stem = name.rsplit('/').next().unwrap_or(&name).trim_end_matches(".Item.Gbx").trim_end_matches(".Block.Gbx");
            out.push(format!("Z\t{}\t{:016x}", norm_name(stem), fingerprint(&bytes, stem.as_bytes())));
        }
    }
    out
}

/// FNV-1a 64 over the bytes, with every occurrence of `ident` read as zeros — two bakes
/// of one item differ only in the ident the library numbered it with.
fn fingerprint(bytes: &[u8], ident: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0usize;
    let n = ident.len();
    while i < bytes.len() {
        let b = if n > 0 && i + n <= bytes.len() && &bytes[i..i + n] == ident {
            i += n;
            // one zero byte stands for the whole ident
            0u8
        } else {
            let b = bytes[i];
            i += 1;
            b
        };
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn multiset(lines: &[String]) -> BTreeMap<&str, i64> {
    let mut m: BTreeMap<&str, i64> = BTreeMap::new();
    for l in lines {
        *m.entry(l.as_str()).or_default() += 1;
    }
    m
}

pub fn cmd(args: &[String]) -> i32 {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let (Some(live), Some(build), Some(out)) = (f("--live"), f("--build"), f("--out")) else {
        eprintln!("usage: tmmaps live-vs-build --live DIR --build DIR --out SUMMARY.tsv [--detail DIR]");
        return 2;
    };
    let detail = f("--detail").map(PathBuf::from);
    // --dump-items NUM,NUM: the differing embedded items of those maps, written as
    // `DETAIL/<num>/{live,build}/<entry>` for a look with mapgeom
    let dump: Vec<u32> = f("--dump-items").map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    if let Some(d) = &detail {
        if let Err(e) = std::fs::create_dir_all(d) {
            eprintln!("{}: {e}", d.display());
            return 1;
        }
    }
    let live_maps = collect(Path::new(&live));
    let build_maps = collect(Path::new(&build));
    let mut nums: Vec<u32> = live_maps.keys().chain(build_maps.keys()).copied().collect();
    nums.sort();
    nums.dedup();
    let mut rows = String::from("num\tverdict\tlive_rows\tbuild_rows\tonly_live\tonly_build\tlive_file\tbuild_file\n");
    let (mut same, mut diff, mut lo, mut bo) = (0usize, 0usize, 0usize, 0usize);
    for n in nums {
        match (live_maps.get(&n), build_maps.get(&n)) {
            (Some(l), None) => {
                lo += 1;
                rows.push_str(&format!("{n}\tLIVE-ONLY\t-\t-\t-\t-\t{}\t-\n", l.display()));
            }
            (None, Some(b)) => {
                bo += 1;
                rows.push_str(&format!("{n}\tBUILD-ONLY\t-\t-\t-\t-\t-\t{}\n", b.display()));
            }
            (Some(l), Some(b)) => {
                let lm = MapFile::load(l);
                let bm = MapFile::load(b);
                let ll = census_lines(&lm);
                let bl = census_lines(&bm);
                let (ls, bs) = (multiset(&ll), multiset(&bl));
                let mut only_live: Vec<String> = Vec::new();
                let mut only_build: Vec<String> = Vec::new();
                for (k, c) in &ls {
                    let d = c - bs.get(k).copied().unwrap_or(0);
                    for _ in 0..d.max(0) {
                        only_live.push(k.to_string());
                    }
                }
                for (k, c) in &bs {
                    let d = c - ls.get(k).copied().unwrap_or(0);
                    for _ in 0..d.max(0) {
                        only_build.push(k.to_string());
                    }
                }
                let verdict = if only_live.is_empty() && only_build.is_empty() { same += 1; "SAME" } else { diff += 1; "DIFF" };
                if dump.contains(&n) {
                    if let Some(d) = &detail {
                        let fps: std::collections::BTreeSet<&str> = only_live.iter().chain(only_build.iter()).filter(|x| x.starts_with("Z\t")).map(|x| x.as_str()).collect();
                        for (side, mm) in [("live", &lm), ("build", &bm)] {
                            if let Some((_, zip)) = mm.embedded_objects() {
                                for (name, bytes) in crate::header::zip_entries(&zip) {
                                    let stem = name.rsplit('/').next().unwrap_or(&name).trim_end_matches(".Item.Gbx").trim_end_matches(".Block.Gbx");
                                    let row = format!("Z\t{}\t{:016x}", norm_name(stem), fingerprint(&bytes, stem.as_bytes()));
                                    if fps.contains(row.as_str()) {
                                        let dir = d.join(n.to_string()).join(side);
                                        let _ = std::fs::create_dir_all(&dir);
                                        let _ = std::fs::write(dir.join(name.rsplit('/').next().unwrap_or(&name)), &bytes);
                                    }
                                }
                            }
                        }
                    }
                }
                rows.push_str(&format!("{n}\t{verdict}\t{}\t{}\t{}\t{}\t{}\t{}\n", ll.len(), bl.len(), only_live.len(), only_build.len(), l.display(), b.display()));
                if verdict == "DIFF" {
                    if let Some(d) = &detail {
                        let mut txt = String::new();
                        for x in &only_live {
                            txt.push_str("< ");
                            txt.push_str(x);
                            txt.push('\n');
                        }
                        for x in &only_build {
                            txt.push_str("> ");
                            txt.push_str(x);
                            txt.push('\n');
                        }
                        let _ = std::fs::write(d.join(format!("{n}.diff")), txt);
                    }
                }
            }
            (None, None) => {}
        }
    }
    if let Err(e) = std::fs::write(&out, &rows) {
        eprintln!("{out}: {e}");
        return 1;
    }
    println!("live-vs-build: {same} SAME, {diff} DIFF, {lo} live-only, {bo} build-only → {out}");
    0
}
