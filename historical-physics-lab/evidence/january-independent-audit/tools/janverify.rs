// janverify — independent static audit of the Historical Physics Lab
// January 2022 native island payload.
//
// Design rule: this tool trusts NOTHING the generator printed. Every count is
// re-derived from the payload text itself, and every structural claim is
// re-checked against the payload bytes and the current (build 128130) image.
//
// It deliberately does NOT need the January 2022 executable: everything here is
// derivable from the emitted island plus the current image. Checks that
// genuinely require the January bytes are reported as UNVERIFIABLE, never as
// pass.
//
// std only.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryInto;
use std::{env, fs};

// ---------------------------------------------------------------- PE loading

struct Pe {
    bytes: Vec<u8>,
    base: u64,
    // (virtual_address, virtual_size, raw_pointer, raw_size, characteristics, name)
    sections: Vec<(u64, u64, usize, usize, u32, String)>,
}

impl Pe {
    fn load(path: &str) -> Self {
        let bytes = fs::read(path).expect("read PE image");
        let pe = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
        assert_eq!(&bytes[pe..pe + 4], b"PE\0\0", "PE signature");
        let section_count = u16::from_le_bytes(bytes[pe + 6..pe + 8].try_into().unwrap()) as usize;
        let optional_size = u16::from_le_bytes(bytes[pe + 20..pe + 22].try_into().unwrap()) as usize;
        let optional = pe + 24;
        let base = u64::from_le_bytes(bytes[optional + 24..optional + 32].try_into().unwrap());
        let table = optional + optional_size;
        let mut sections = Vec::new();
        for index in 0..section_count {
            let off = table + index * 40;
            let name = String::from_utf8_lossy(&bytes[off..off + 8])
                .trim_end_matches('\0')
                .to_string();
            let virtual_size = u32::from_le_bytes(bytes[off + 8..off + 12].try_into().unwrap()) as u64;
            let virtual_address = u32::from_le_bytes(bytes[off + 12..off + 16].try_into().unwrap()) as u64;
            let raw_size = u32::from_le_bytes(bytes[off + 16..off + 20].try_into().unwrap()) as usize;
            let raw_pointer = u32::from_le_bytes(bytes[off + 20..off + 24].try_into().unwrap()) as usize;
            let characteristics = u32::from_le_bytes(bytes[off + 36..off + 40].try_into().unwrap());
            sections.push((
                virtual_address,
                virtual_size.max(raw_size as u64),
                raw_pointer,
                raw_size,
                characteristics,
                name,
            ));
        }
        Self { bytes, base, sections }
    }

    fn at(&self, address: u64, size: usize) -> Option<&[u8]> {
        let rva = address.checked_sub(self.base)?;
        for &(section_rva, virtual_size, raw_pointer, raw_size, _, _) in &self.sections {
            if rva >= section_rva && rva + size as u64 <= section_rva + virtual_size {
                let offset = raw_pointer + (rva - section_rva) as usize;
                if offset + size <= raw_pointer + raw_size {
                    return self.bytes.get(offset..offset + size);
                }
            }
        }
        None
    }

    fn section_of_rva(&self, rva: u64) -> Option<(&str, u32)> {
        for &(section_rva, virtual_size, _, _, characteristics, ref name) in &self.sections {
            if rva >= section_rva && rva < section_rva + virtual_size {
                return Some((name.as_str(), characteristics));
            }
        }
        None
    }

    fn is_executable_rva(&self, rva: u64) -> bool {
        // IMAGE_SCN_MEM_EXECUTE
        self.section_of_rva(rva).is_some_and(|(_, ch)| ch & 0x2000_0000 != 0)
    }

    fn is_writable_rva(&self, rva: u64) -> bool {
        // IMAGE_SCN_MEM_WRITE
        self.section_of_rva(rva).is_some_and(|(_, ch)| ch & 0x8000_0000 != 0)
    }

    fn count_occurrences(&self, needle: &[u8]) -> usize {
        self.bytes.windows(needle.len()).filter(|w| *w == needle).count()
    }
}

// ------------------------------------------------------- AngelScript parsing

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let begin = text.find(start)? + start.len();
    let finish = text[begin..].find(end)? + begin;
    Some(&text[begin..finish])
}

fn number(text: &str) -> u64 {
    let t = text.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).unwrap_or_else(|_| panic!("hex number {t:?}"))
    } else {
        t.parse().unwrap_or_else(|_| panic!("decimal number {t:?}"))
    }
}

fn scalar(text: &str, name: &str) -> Option<u64> {
    between(text, &format!("{name} = "), ";").map(|b| number(b.trim()))
}

fn numbers(text: &str, name: &str) -> Vec<u64> {
    let Some(body) = between(text, &format!("{name} = {{"), "};") else {
        return Vec::new();
    };
    if body.trim().is_empty() {
        return Vec::new();
    }
    body.split(',').map(|p| number(p.trim())).collect()
}

fn booleans(text: &str, name: &str) -> Vec<bool> {
    let Some(body) = between(text, &format!("{name} = {{"), "};") else {
        return Vec::new();
    };
    if body.trim().is_empty() {
        return Vec::new();
    }
    body.split(',')
        .map(|p| match p.trim() {
            "true" => true,
            "false" => false,
            other => panic!("bad bool {other:?}"),
        })
        .collect()
}

fn payload_bytes(text: &str, name: &str) -> Vec<u8> {
    between(text, &format!("{name} = \""), "\";")
        .expect("island byte string")
        .split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("payload byte"))
        .collect()
}

// ------------------------------------------------------------------ findings

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
enum Level {
    Pass,
    Note,
    Warn,
    Defect,
}

struct Report {
    rows: Vec<(Level, String, String)>,
}

impl Report {
    fn new() -> Self {
        Self { rows: Vec::new() }
    }
    fn add(&mut self, level: Level, check: &str, detail: String) {
        self.rows.push((level, check.to_string(), detail));
    }
    fn pass(&mut self, check: &str, detail: String) {
        self.add(Level::Pass, check, detail)
    }
    fn note(&mut self, check: &str, detail: String) {
        self.add(Level::Note, check, detail)
    }
    fn warn(&mut self, check: &str, detail: String) {
        self.add(Level::Warn, check, detail)
    }
    fn defect(&mut self, check: &str, detail: String) {
        self.add(Level::Defect, check, detail)
    }
    fn print(&self) {
        for (level, check, detail) in &self.rows {
            let tag = match level {
                Level::Pass => "PASS  ",
                Level::Note => "NOTE  ",
                Level::Warn => "WARN  ",
                Level::Defect => "DEFECT",
            };
            println!("{tag}\t{check}\t{detail}");
        }
        let defects = self.rows.iter().filter(|r| r.0 == Level::Defect).count();
        let warns = self.rows.iter().filter(|r| r.0 == Level::Warn).count();
        println!("\nSUMMARY defects={defects} warnings={warns} checks={}", self.rows.len());
    }
}

// ---------------------------------------------------------------------- main

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: janverify Profile_Jan2022.as CURRENT.exe [--emit-dir DIR]");
        std::process::exit(2);
    }
    let profile = fs::read_to_string(&args[0]).expect("read profile");
    let current = Pe::load(&args[1]);
    let emit_dir = args
        .iter()
        .position(|a| a == "--emit-dir")
        .and_then(|i| args.get(i + 1))
        .cloned();

    let mut report = Report::new();

    // ---- payload and declared scalars -----------------------------------
    let payload = payload_bytes(&profile, "PROFILE_JAN2022_ISLAND_BYTES");
    let declared_size = scalar(&profile, "PROFILE_JAN2022_ISLAND_SIZE").expect("island size");
    if payload.len() as u64 == declared_size {
        report.pass("payload.size", format!("{} bytes, matches declaration", payload.len()));
    } else {
        report.defect(
            "payload.size",
            format!("declared {declared_size}, decoded {}", payload.len()),
        );
    }

    let field_offsets = numbers(&profile, "PROFILE_JAN2022_FIELD_OFFSETS");
    let field_sites = numbers(&profile, "PROFILE_JAN2022_FIELD_SOURCE_VAS");
    let field_widths = numbers(&profile, "PROFILE_JAN2022_FIELD_WIDTHS");
    let field_old = numbers(&profile, "PROFILE_JAN2022_FIELD_OLD_VALUES");
    let field_new = numbers(&profile, "PROFILE_JAN2022_FIELD_NEW_VALUES");
    let call_offsets = numbers(&profile, "PROFILE_JAN2022_CALL_RELOC_OFFSETS");
    let call_targets = numbers(&profile, "PROFILE_JAN2022_CALL_TARGET_ISLAND_OFFSETS");
    let rip_offsets = numbers(&profile, "PROFILE_JAN2022_RIP_RELOC_OFFSETS");
    let rip_targets = numbers(&profile, "PROFILE_JAN2022_RIP_TARGET_RVAS");
    let rip_is_island = booleans(&profile, "PROFILE_JAN2022_RIP_TARGET_IS_ISLAND");
    let reloc_offsets = numbers(&profile, "PROFILE_JAN2022_RELOC_OFFSETS");
    let reloc_targets = numbers(&profile, "PROFILE_JAN2022_RELOC_TARGET_RVAS");
    let reloc_is_island = booleans(&profile, "PROFILE_JAN2022_RELOC_TARGET_IS_ISLAND");
    let abs64_offsets = numbers(&profile, "PROFILE_JAN2022_ABS64_OFFSETS");
    let abs64_targets = numbers(&profile, "PROFILE_JAN2022_ABS64_TARGET_RVAS");
    let region_starts = numbers(&profile, "PROFILE_JAN2022_SOURCE_REGION_START_VAS");
    let region_ends = numbers(&profile, "PROFILE_JAN2022_SOURCE_REGION_END_VAS");
    let region_offsets = numbers(&profile, "PROFILE_JAN2022_ISLAND_REGION_OFFSETS");
    let region_lengths = numbers(&profile, "PROFILE_JAN2022_ISLAND_REGION_LENGTHS");
    let init_sources = numbers(&profile, "PROFILE_JAN2022_INIT_SOURCE_VAS");
    let init_shadow_offsets = numbers(&profile, "PROFILE_JAN2022_INIT_SHADOW_OFFSETS");
    let init_values = numbers(&profile, "PROFILE_JAN2022_INIT_VALUES");
    let adapter = scalar(&profile, "PROFILE_JAN2022_INTERPOLATION_ADAPTER_OFFSET").expect("adapter");
    let shadow = scalar(&profile, "PROFILE_JAN2022_INITIALIZER_SHADOW_OFFSET").expect("shadow");

    // ---- claim: counts ---------------------------------------------------
    let claims: [(&str, u64, usize); 6] = [
        ("field", scalar(&profile, "PROFILE_JAN2022_FIELD_RELOCATION_COUNT").unwrap(), field_offsets.len()),
        ("call", scalar(&profile, "PROFILE_JAN2022_CALL_RELOCATION_COUNT").unwrap(), call_offsets.len()),
        ("rip", scalar(&profile, "PROFILE_JAN2022_RIP_RELOCATION_COUNT").unwrap(), rip_offsets.len()),
        ("unresolved_calls", scalar(&profile, "PROFILE_JAN2022_UNRESOLVED_CALLS").unwrap(), 0),
        ("unresolved_rip", scalar(&profile, "PROFILE_JAN2022_UNRESOLVED_RIP").unwrap(), 0),
        ("regions", 14, region_starts.len()),
    ];
    for (name, declared, derived) in claims {
        if name.starts_with("unresolved") {
            continue;
        }
        if declared as usize == derived {
            report.pass("counts", format!("{name}: declared {declared} == array length {derived}"));
        } else {
            report.defect("counts", format!("{name}: declared {declared} != array length {derived}"));
        }
    }
    for (name, arrays) in [
        ("field", vec![field_offsets.len(), field_sites.len(), field_widths.len(), field_old.len(), field_new.len()]),
        ("call", vec![call_offsets.len(), call_targets.len()]),
        ("rip", vec![rip_offsets.len(), rip_targets.len(), rip_is_island.len()]),
        ("reloc", vec![reloc_offsets.len(), reloc_targets.len(), reloc_is_island.len()]),
        ("abs64", vec![abs64_offsets.len(), abs64_targets.len()]),
        ("region", vec![region_starts.len(), region_ends.len(), region_offsets.len(), region_lengths.len()]),
        ("init", vec![init_shadow_offsets.len(), init_values.len()]),
    ] {
        if arrays.windows(2).all(|w| w[0] == w[1]) {
            report.pass("manifest.parallel", format!("{name} arrays all {} entries", arrays[0]));
        } else {
            report.defect("manifest.parallel", format!("{name} arrays ragged: {arrays:?}"));
        }
    }
    if reloc_offsets.len() == call_offsets.len() + rip_offsets.len() {
        report.pass(
            "manifest.union",
            format!("combined {} == calls {} + rips {}", reloc_offsets.len(), call_offsets.len(), rip_offsets.len()),
        );
    } else {
        report.defect(
            "manifest.union",
            format!("combined {} != calls {} + rips {}", reloc_offsets.len(), call_offsets.len(), rip_offsets.len()),
        );
    }
    if init_sources.len() == 4 && init_values.len() == 4 {
        report.pass("init.count", "4 copied initialization values".into());
    } else {
        report.defect("init.count", format!("{} sources / {} values", init_sources.len(), init_values.len()));
    }

    // ---- duplicate and overlap analysis ---------------------------------
    let mut seen = BTreeMap::<usize, Vec<String>>::new();
    for (i, &o) in field_offsets.iter().enumerate() {
        seen.entry(o as usize).or_default().push(format!("field#{i}"));
    }
    for (i, &o) in reloc_offsets.iter().enumerate() {
        seen.entry(o as usize).or_default().push(format!("reloc#{i}"));
    }
    let mut dup = 0usize;
    for (o, owners) in &seen {
        if owners.len() > 1 {
            dup += 1;
            report.defect("manifest.duplicate_offset", format!("offset {o} claimed by {owners:?}"));
        }
    }
    if dup == 0 {
        report.pass("manifest.duplicate_offset", "no offset claimed twice".into());
    }

    // byte-range overlap: a field patch (1 or 4 bytes) must not overlap a rel32 slot
    let mut painted = BTreeMap::<usize, String>::new();
    let mut overlaps = 0usize;
    for (i, (&o, &w)) in field_offsets.iter().zip(&field_widths).enumerate() {
        for b in o as usize..o as usize + w as usize {
            if let Some(prev) = painted.insert(b, format!("field#{i}")) {
                overlaps += 1;
                report.defect("manifest.byte_overlap", format!("byte {b} in field#{i} and {prev}"));
            }
        }
    }
    for (i, &o) in reloc_offsets.iter().enumerate() {
        for b in o as usize..o as usize + 4 {
            if let Some(prev) = painted.insert(b, format!("reloc#{i}")) {
                overlaps += 1;
                report.defect(
                    "manifest.byte_overlap",
                    format!("byte {b} written by reloc#{i} and {prev}"),
                );
            }
        }
    }
    for (i, &o) in abs64_offsets.iter().enumerate() {
        for b in o as usize..o as usize + 8 {
            if let Some(prev) = painted.insert(b, format!("abs64#{i}")) {
                overlaps += 1;
                report.defect("manifest.byte_overlap", format!("byte {b} written by abs64#{i} and {prev}"));
            }
        }
    }
    if overlaps == 0 {
        report.pass("manifest.byte_overlap", "no patch/relocation byte ranges overlap".into());
    }

    // ---- bounds ----------------------------------------------------------
    let mut oob = 0;
    for (i, (&o, &w)) in field_offsets.iter().zip(&field_widths).enumerate() {
        if o as usize + w as usize > payload.len() {
            oob += 1;
            report.defect("bounds.field", format!("field#{i} at {o} width {w} exceeds payload"));
        }
        if w != 1 && w != 4 {
            report.defect("bounds.field", format!("field#{i} unsupported width {w}"));
        }
    }
    for (i, &o) in reloc_offsets.iter().enumerate() {
        if o as usize + 4 > payload.len() {
            oob += 1;
            report.defect("bounds.reloc", format!("reloc#{i} at {o} exceeds payload"));
        }
    }
    for (i, &o) in abs64_offsets.iter().enumerate() {
        if o as usize + 8 > payload.len() {
            oob += 1;
            report.defect("bounds.abs64", format!("abs64#{i} at {o} exceeds payload"));
        }
    }
    if oob == 0 {
        report.pass("bounds", "all patch sites inside the payload".into());
    }

    // ---- regions ---------------------------------------------------------
    let mut region_rows = Vec::new();
    for i in 0..region_starts.len() {
        let src_len = (region_ends[i] - region_starts[i]) as usize;
        let shift = region_lengths[i] as usize as i64 - src_len as i64;
        region_rows.push((region_starts[i], region_ends[i], region_offsets[i] as usize, region_lengths[i] as usize, shift));
        if region_offsets[i] as usize + region_lengths[i] as usize > payload.len() {
            report.defect(
                "region.bounds",
                format!("region {:#x} island span exceeds payload", region_starts[i]),
            );
        }
        if shift != 0 && !(shift == 3 && region_starts[i] == 0x1405edcf0) {
            report.defect(
                "region.shift",
                format!("region {:#x} unexpected shift {shift}", region_starts[i]),
            );
        }
    }
    // island-space overlap between regions
    let mut region_overlap = 0;
    for i in 0..region_rows.len() {
        for j in i + 1..region_rows.len() {
            let (_, _, ao, al, _) = region_rows[i];
            let (_, _, bo, bl, _) = region_rows[j];
            if ao < bo + bl && bo < ao + al {
                region_overlap += 1;
                report.defect(
                    "region.overlap",
                    format!("island ranges {ao}..{} and {bo}..{} overlap", ao + al, bo + bl),
                );
            }
        }
    }
    if region_overlap == 0 {
        report.pass("region.overlap", format!("{} copied regions, none overlapping in island space", region_rows.len()));
    }
    // duplicate source regions (expected: one wrapper copied twice)
    let mut source_seen = BTreeMap::<(u64, u64), usize>::new();
    for (s, e, _, _, _) in &region_rows {
        *source_seen.entry((*s, *e)).or_insert(0) += 1;
    }
    let duplicated: Vec<_> = source_seen.iter().filter(|(_, &c)| c > 1).collect();
    report.note(
        "region.duplicate_source",
        format!("{} source region(s) copied more than once: {:?}", duplicated.len(),
            duplicated.iter().map(|((s, e), c)| format!("{s:#x}..{e:#x}x{c}")).collect::<Vec<_>>()),
    );

    // ---- build gate against the current image ---------------------------
    let gate_pattern: Vec<u8> = "48 8B C4 F3 0F 11 48 10 48 89 48 08 55 56 57 41 54 48 8D A8 48 FE FF FF 48 81 EC 98 02 00 00 49 8B 78 08 41 BA 04 00 00 00"
        .split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect();
    let handler_rva = 0x851f00u64;
    match current.at(current.base + handler_rva, gate_pattern.len()) {
        Some(actual) if actual == gate_pattern.as_slice() => {
            let occurrences = current.count_occurrences(&gate_pattern);
            if occurrences == 1 {
                report.pass("gate.handler", "41-byte handler signature present and unique".into());
            } else {
                report.defect("gate.handler", format!("handler signature occurs {occurrences} times"));
            }
        }
        Some(_) => report.defect("gate.handler", "handler bytes at 0x851f00 do not match the gate pattern".into()),
        None => report.defect("gate.handler", "handler RVA not mapped in the current image".into()),
    }
    let banner = b"date=2026-01-28_13_00 git=128130-6dda3728e91 GameVersion=3.3.0";
    let banner_hits = current.count_occurrences(banner);
    if banner_hits == 1 {
        report.pass("gate.banner", "build banner present and unique".into());
    } else {
        report.defect("gate.banner", format!("build banner occurs {banner_hits} times"));
    }
    if payload.len() >= 12 && payload[..12] == gate_pattern[..12] {
        report.pass("entry.prologue", "island entry prologue matches the current handler's first 12 bytes".into());
    } else {
        report.defect("entry.prologue", "island entry prologue differs from the current handler".into());
    }

    // ---- absolute thunks -------------------------------------------------
    let mut thunk_entries = BTreeSet::<usize>::new();
    let mut bad_thunks = 0;
    for (i, (&off, &rva)) in abs64_offsets.iter().zip(&abs64_targets).enumerate() {
        let off = off as usize;
        if off < 2 || off + 10 > payload.len() {
            bad_thunks += 1;
            report.defect("thunk.shape", format!("abs64#{i} offset {off} cannot hold a thunk"));
            continue;
        }
        if &payload[off - 2..off] != [0x48, 0xb8] || &payload[off + 8..off + 10] != [0xff, 0xe0] {
            bad_thunks += 1;
            report.defect(
                "thunk.shape",
                format!("abs64#{i} at {off} is not `mov rax,imm64; jmp rax`"),
            );
            continue;
        }
        thunk_entries.insert(off - 2);
        if !current.is_executable_rva(rva) {
            bad_thunks += 1;
            report.defect(
                "thunk.target",
                format!("abs64#{i} target rva {rva:#x} is not in an executable section of the current image"),
            );
        }
    }
    if bad_thunks == 0 {
        report.pass(
            "thunk",
            format!("{} absolute thunks well formed, all targets in executable sections", abs64_offsets.len()),
        );
    }
    // thunk entries must be 16-byte aligned per the generator's own padding rule
    let misaligned: Vec<_> = thunk_entries.iter().filter(|o| *o % 16 != 0).collect();
    if misaligned.is_empty() {
        report.pass("thunk.align", "every thunk entry is 16-byte aligned".into());
    } else {
        report.warn("thunk.align", format!("{} thunk entries not 16-byte aligned", misaligned.len()));
    }

    // ---- relocation classification --------------------------------------
    let mut island_targets_out_of_range = 0;
    let mut image_relative = Vec::<(usize, u64)>::new();
    for i in 0..reloc_offsets.len() {
        let off = reloc_offsets[i] as usize;
        let target = reloc_targets[i];
        if reloc_is_island[i] {
            if target as usize >= payload.len() {
                island_targets_out_of_range += 1;
                report.defect(
                    "reloc.island_target",
                    format!("reloc#{i} at {off} targets island offset {target} beyond payload"),
                );
            }
        } else {
            if current.at(current.base + target, 1).is_none() {
                report.defect(
                    "reloc.image_target",
                    format!("reloc#{i} at {off} targets rva {target:#x} outside the current image"),
                );
            }
            image_relative.push((off, target));
        }
    }
    if island_targets_out_of_range == 0 {
        report.pass("reloc.island_target", "all in-island relocation targets are inside the payload".into());
    }

    // THE ±2 GiB QUESTION: any relocation that is NOT island-relative encodes a
    // rel32 from the island to the current image. Those only work if the island
    // is allocated within ±2 GiB of Trackmania.exe. The absolute-thunk design
    // exists precisely to avoid that constraint, so any survivor is a real
    // placement constraint that the loader must honour.
    if image_relative.is_empty() {
        report.pass(
            "reloc.placement",
            "no rel32 references from island to image survive; island is position independent".into(),
        );
    } else {
        report.defect(
            "reloc.placement",
            format!(
                "{} relocations encode rel32 island->image (offsets {:?}...): island MUST be allocated within +/-2 GiB of the image, which the absolute-thunk design claims not to require",
                image_relative.len(),
                image_relative.iter().take(6).map(|(o, _)| *o).collect::<Vec<_>>()
            ),
        );
        // classify their targets
        let mut exec = 0;
        let mut write = 0;
        let mut other = 0;
        for (_, rva) in &image_relative {
            if current.is_executable_rva(*rva) {
                exec += 1;
            } else if current.is_writable_rva(*rva) {
                write += 1;
            } else {
                other += 1;
            }
        }
        report.note(
            "reloc.placement.targets",
            format!("image-relative targets: {exec} executable, {write} writable-data, {other} read-only-data"),
        );
    }

    // ---- call relocations must land on island code ----------------------
    let call_set: BTreeSet<usize> = call_offsets.iter().map(|o| *o as usize).collect();
    let rip_set: BTreeSet<usize> = rip_offsets.iter().map(|o| *o as usize).collect();
    if call_set.len() == call_offsets.len() {
        report.pass("call.unique", "no duplicated call relocation offsets".into());
    } else {
        report.defect("call.unique", "duplicated call relocation offsets".into());
    }
    if call_set.is_disjoint(&rip_set) {
        report.pass("reloc.disjoint", "call and RIP relocation sets are disjoint".into());
    } else {
        report.defect("reloc.disjoint", "a relocation offset is classified as both call and RIP".into());
    }

    let region_span: Vec<(usize, usize)> = region_rows.iter().map(|(_, _, o, l, _)| (*o, *o + *l)).collect();
    let in_region = |x: usize| region_span.iter().any(|(a, b)| x >= *a && x < *b);
    let mut cave_defects = 0;
    for (i, (&off, &target)) in call_offsets.iter().zip(&call_targets).enumerate() {
        let t = target as usize;
        if t >= payload.len() {
            cave_defects += 1;
            report.defect("cave.call", format!("call#{i} targets {t} beyond the payload"));
            continue;
        }
        let is_thunk = thunk_entries.contains(&t);
        let is_region_entry = region_rows.iter().any(|(_, _, o, _, _)| *o == t);
        let is_adapter = t == adapter as usize;
        if !(is_thunk || is_region_entry || is_adapter) {
            cave_defects += 1;
            let landing = if in_region(t) { "inside a copied region but not at its entry" } else { "in island filler" };
            report.defect(
                "cave.call",
                format!("call#{i} at island offset {off} jumps to {t}, which is neither a thunk, a copied-region entry, nor the adapter ({landing})"),
            );
        }
    }
    if cave_defects == 0 {
        report.pass(
            "cave.call",
            format!("all {} in-island call targets land on a thunk, a region entry, or the adapter", call_offsets.len()),
        );
    }

    // ---- adapter -----------------------------------------------------------
    let adapter_expected: [u8; 28] = [
        0x48, 0x83, 0xEC, 0x38, // sub rsp,0x38
        0x4C, 0x89, 0x44, 0x24, 0x20, // mov [rsp+0x20],r8
        0xE8, 0, 0, 0, 0, // call rel32 (relocated)
        0x48, 0x8B, 0x44, 0x24, 0x20, // mov rax,[rsp+0x20]
        0xF3, 0x0F, 0x11, 0x00, // movss [rax],xmm0
        0x48, 0x83, 0xC4, 0x38, // add rsp,0x38
        0xC3, // ret
    ];
    let a = adapter as usize;
    if a + adapter_expected.len() <= payload.len() {
        let got = &payload[a..a + adapter_expected.len()];
        let same_shape = got[..9] == adapter_expected[..9]
            && got[9] == 0xE8
            && got[14..] == adapter_expected[14..];
        if same_shape {
            report.pass("adapter.shape", "interpolation adapter matches the documented 28-byte thunk".into());
            // Win64 stack alignment: entry rsp%16==8, sub 0x38 -> 0 mod 16, call pushes 8 -> callee sees 8. Correct.
            report.pass("adapter.abi.alignment", "sub rsp,0x38 keeps the Win64 16-byte alignment contract at the inner call".into());
            report.warn(
                "adapter.abi.r8",
                "adapter forwards the caller's r8 (the old output pointer) unchanged into the current scalar helper; if that helper reads a third integer argument it receives an output pointer. Unverifiable without the January binary and the current helper's signature".into(),
            );
            report.warn(
                "adapter.abi.xmm_spill",
                "adapter allocates 0x38 bytes but spills only r8; it saves no xmm register and preserves no argument beyond r8, so any current-helper clobber of xmm1-xmm5 that the old wrapper relied on is lost".into(),
            );
        } else {
            report.defect("adapter.shape", format!("adapter bytes differ from the documented sequence: {:02x?}", got));
        }
        let adapter_call_reloc = call_set.contains(&(a + 10)) || rip_set.contains(&(a + 10));
        if adapter_call_reloc {
            report.pass("adapter.reloc", "adapter's inner call is relocated".into());
        } else {
            report.defect("adapter.reloc", "adapter's inner call has no relocation entry".into());
        }
    } else {
        report.defect("adapter.shape", "adapter offset outside payload".into());
    }

    // ---- initializer shadow ---------------------------------------------
    let expected_init: [(u64, u64); 4] = [
        (0x788, 0x40A00000),
        (0x78c, 0x40A00000),
        (0x790, 0x41C80000),
        (0x7a0, 0x00000000),
    ];
    let mut init_bad = 0;
    for (i, (&off, &val)) in init_shadow_offsets.iter().zip(&init_values).enumerate() {
        let off = off as usize;
        if off + 4 > payload.len() {
            init_bad += 1;
            report.defect("init.bounds", format!("init#{i} offset {off} outside payload"));
            continue;
        }
        let actual = u32::from_le_bytes(payload[off..off + 4].try_into().unwrap()) as u64;
        if actual != val {
            init_bad += 1;
            report.defect("init.value", format!("init#{i} declares {val:#x} but payload holds {actual:#x}"));
        }
        let rel = off as u64 - shadow;
        if rel != expected_init[i].0 || val != expected_init[i].1 {
            report.warn(
                "init.layout",
                format!("init#{i} at shadow+{rel:#x}={val:#x}, documented shadow+{:#x}={:#x}", expected_init[i].0, expected_init[i].1),
            );
        }
    }
    if init_bad == 0 {
        report.pass("init", "all four January initialization values present in the shadow block".into());
    }
    // the shadow block must be zero everywhere else it is read from; report its extent
    let shadow_end = (shadow as usize + 0x7a4).min(payload.len());
    let nonzero: Vec<usize> = (shadow as usize..shadow_end)
        .filter(|&b| payload[b] != 0)
        .collect();
    let expected_nonzero: BTreeSet<usize> = init_shadow_offsets
        .iter()
        .flat_map(|&o| (o as usize..o as usize + 4))
        .collect();
    let unexpected: Vec<usize> = nonzero
        .iter()
        .copied()
        .filter(|b| !expected_nonzero.contains(b))
        .collect();
    if unexpected.is_empty() {
        report.pass(
            "init.shadow_zeroed",
            format!("shadow block of {:#x} bytes is zero except the four copied values", 0x7a4),
        );
        report.warn(
            "init.shadow_semantics",
            "the shadow is a 0x7a4-byte ZERO block with four January values written into it; every other field the removed helper reads from that model block therefore reads 0.0, not the January value. Only four of the block's fields were shown to be initialized by January code".into(),
        );
    } else {
        report.note("init.shadow_zeroed", format!("{} unexpected non-zero bytes in shadow", unexpected.len()));
    }

    // ---- field map algebra ----------------------------------------------
    // A January struct offset must map to exactly one current offset. Two
    // different January offsets must not collapse onto one current offset.
    let mut forward = BTreeMap::<u64, BTreeSet<u64>>::new();
    let mut backward = BTreeMap::<u64, BTreeSet<u64>>::new();
    for i in 0..field_old.len() {
        forward.entry(field_old[i]).or_default().insert(field_new[i]);
        backward.entry(field_new[i]).or_default().insert(field_old[i]);
    }
    let contradictions: Vec<_> = forward.iter().filter(|(_, v)| v.len() > 1).collect();
    if contradictions.is_empty() {
        report.pass(
            "fieldmap.function",
            format!("{} distinct January offsets each map to exactly one current offset", forward.len()),
        );
    } else {
        for (old, news) in &contradictions {
            report.defect(
                "fieldmap.function",
                format!("January offset {old:#x} maps to multiple current offsets {:?}", news.iter().map(|n| format!("{n:#x}")).collect::<Vec<_>>()),
            );
        }
    }
    let collisions: Vec<_> = backward.iter().filter(|(_, v)| v.len() > 1).collect();
    if collisions.is_empty() {
        report.pass("fieldmap.injective", "no two January offsets collapse onto one current offset".into());
    } else {
        for (new, olds) in &collisions {
            report.warn(
                "fieldmap.injective",
                format!("current offset {new:#x} is the image of several January offsets {:?}", olds.iter().map(|o| format!("{o:#x}")).collect::<Vec<_>>()),
            );
        }
    }
    // identity remaps carry no information but are legal
    let identity = field_old.iter().zip(&field_new).filter(|(o, n)| o == n).count();
    report.note("fieldmap.identity", format!("{identity} of {} relocations map an offset onto itself", field_old.len()));

    // patched values must actually be in the payload
    let mut value_bad = 0;
    for i in 0..field_offsets.len() {
        let off = field_offsets[i] as usize;
        let w = field_widths[i] as usize;
        if off + w > payload.len() {
            continue;
        }
        let actual = if w == 1 {
            payload[off] as i8 as i64
        } else {
            i32::from_le_bytes(payload[off..off + 4].try_into().unwrap()) as i64
        };
        if actual != field_new[i] as i32 as i64 && actual != field_new[i] as i64 {
            value_bad += 1;
            report.defect(
                "field.value",
                format!("field#{i} at {off} declares new {:#x} but payload holds {actual:#x}", field_new[i]),
            );
        }
    }
    if value_bad == 0 {
        report.pass("field.value", format!("all {} field patches present in the payload bytes", field_offsets.len()));
    }

    // ---- field sites must fall inside a copied region -------------------
    let mut site_outside = 0;
    for i in 0..field_sites.len() {
        let site = field_sites[i];
        if !region_rows.iter().any(|(s, e, _, _, _)| site >= *s && site < *e) {
            site_outside += 1;
            report.defect(
                "field.site_region",
                format!("field#{i} source VA {site:#x} lies outside every copied region"),
            );
        }
    }
    if site_outside == 0 {
        report.pass("field.site_region", "every field source VA lies inside a copied region".into());
    }

    // island offset must be consistent with (region, site) + in-instruction position
    let mut site_map_bad = 0;
    for i in 0..field_sites.len() {
        let site = field_sites[i];
        let off = field_offsets[i] as usize;
        let mut ok = false;
        for (s, e, io, il, shift) in &region_rows {
            if site < *s || site >= *e {
                continue;
            }
            let base = *io + (site - *s) as usize + if *shift == 3 && site > *s { 3 } else { 0 };
            // the patch must be within 15 bytes of the instruction start (max x86 length)
            if off >= base && off < base + 15 && off + field_widths[i] as usize <= *io + *il {
                ok = true;
                break;
            }
        }
        if !ok {
            site_map_bad += 1;
            report.defect(
                "field.site_offset",
                format!("field#{i} site {site:#x} cannot reach island offset {off} within one instruction of any copied region"),
            );
        }
    }
    if site_map_bad == 0 {
        report.pass("field.site_offset", "every field patch lies within one instruction of its declared source site".into());
    }

    // ---- emit artifacts for disassembly ---------------------------------
    if let Some(dir) = emit_dir {
        fs::create_dir_all(&dir).expect("create emit dir");
        fs::write(format!("{dir}/island-patched.bin"), &payload).expect("write island");
        // un-patched variant: restore every declared OLD field value
        let mut unpatched = payload.clone();
        for i in 0..field_offsets.len() {
            let off = field_offsets[i] as usize;
            let w = field_widths[i] as usize;
            if off + w > unpatched.len() {
                continue;
            }
            if w == 1 {
                unpatched[off] = field_old[i] as i8 as u8;
            } else {
                unpatched[off..off + 4].copy_from_slice(&(field_old[i] as i32).to_le_bytes());
            }
        }
        fs::write(format!("{dir}/island-unpatched.bin"), &unpatched).expect("write unpatched");
        // machine-readable manifests for the disassembly pass
        let mut manifest = String::from("kind\tindex\toffset\twidth\tsite\told\tnew\n");
        for i in 0..field_offsets.len() {
            manifest.push_str(&format!(
                "field\t{i}\t{}\t{}\t{:#x}\t{:#x}\t{:#x}\n",
                field_offsets[i], field_widths[i], field_sites[i], field_old[i], field_new[i]
            ));
        }
        for i in 0..reloc_offsets.len() {
            manifest.push_str(&format!(
                "reloc\t{i}\t{}\t4\t0\t{}\t{:#x}\n",
                reloc_offsets[i], reloc_is_island[i], reloc_targets[i]
            ));
        }
        for (i, (s, e, o, l, _)) in region_rows.iter().enumerate() {
            manifest.push_str(&format!("region\t{i}\t{o}\t{l}\t{s:#x}\t{e:#x}\t0\n"));
        }
        fs::write(format!("{dir}/manifest.tsv"), manifest).expect("write manifest");
        report.note("emit", format!("wrote island-patched.bin, island-unpatched.bin, manifest.tsv to {dir}"));
    }

    // ---- what cannot be checked without the January bytes ---------------
    report.warn(
        "unverifiable.january_binary",
        "Trackmania-2022-01-21.exe (sha256 e2255c415f0f7fc2d0a66512fa7609256c42cf639a5380b7a5bcdbb4486ab75b) is absent: island bytes cannot be compared against their January source, so copied-region fidelity, the field TSV's own correctness, and every hand-tabled old->new pair remain unverified".into(),
    );
    report.warn(
        "unverifiable.field_evidence",
        "january-vs-current-fields.tsv and january-via-spring-fields.tsv are absent from the bundle: field-map COMPLETENESS (the omitted-carrier class) cannot be checked against the evidence that produced it".into(),
    );

    report.print();
}
