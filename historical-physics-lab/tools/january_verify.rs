use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    env, fs,
};

#[derive(Clone, Debug)]
struct Instruction {
    address: u64,
    code: String,
    target: Option<u64>,
    length: usize,
}

#[derive(Clone, Debug)]
struct Region {
    start: u64,
    end: u64,
    island_offset: usize,
    island_length: usize,
}

struct Pe {
    bytes: Vec<u8>,
    base: u64,
    sections: Vec<(u64, u64, usize, usize)>,
}

impl Pe {
    fn load(path: &str) -> Self {
        let bytes = fs::read(path).expect("read PE image");
        let pe = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
        assert_eq!(&bytes[pe..pe + 4], b"PE\0\0", "PE signature");
        let section_count = u16::from_le_bytes(bytes[pe + 6..pe + 8].try_into().unwrap()) as usize;
        let optional_size =
            u16::from_le_bytes(bytes[pe + 20..pe + 22].try_into().unwrap()) as usize;
        let optional = pe + 24;
        let base = u64::from_le_bytes(bytes[optional + 24..optional + 32].try_into().unwrap());
        let section_table = optional + optional_size;
        let mut sections = Vec::new();
        for index in 0..section_count {
            let offset = section_table + index * 40;
            let virtual_size =
                u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as u64;
            let virtual_address =
                u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap()) as u64;
            let raw_size =
                u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap()) as usize;
            let raw_pointer =
                u32::from_le_bytes(bytes[offset + 20..offset + 24].try_into().unwrap()) as usize;
            sections.push((
                virtual_address,
                virtual_size.max(raw_size as u64),
                raw_pointer,
                raw_size,
            ));
        }
        Self {
            bytes,
            base,
            sections,
        }
    }

    fn at(&self, address: u64, size: usize) -> Option<&[u8]> {
        let rva = address.checked_sub(self.base)?;
        for &(section_rva, virtual_size, raw_pointer, raw_size) in &self.sections {
            if rva >= section_rva && rva + size as u64 <= section_rva + virtual_size {
                let offset = raw_pointer + (rva - section_rva) as usize;
                if offset + size <= raw_pointer + raw_size {
                    return self.bytes.get(offset..offset + size);
                }
            }
        }
        None
    }
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let begin = text
        .find(start)
        .unwrap_or_else(|| panic!("missing manifest token: {start}"))
        + start.len();
    let finish = text[begin..]
        .find(end)
        .unwrap_or_else(|| panic!("unterminated manifest token: {start}"))
        + begin;
    &text[begin..finish]
}

fn scalar(text: &str, name: &str) -> u64 {
    let body = between(text, &format!("{name} = "), ";").trim();
    number(body)
}

fn numbers(text: &str, name: &str) -> Vec<u64> {
    let body = between(text, &format!("{name} = {{"), "};");
    if body.trim().is_empty() {
        return Vec::new();
    }
    body.split(',').map(|part| number(part.trim())).collect()
}

fn booleans(text: &str, name: &str) -> Vec<bool> {
    let body = between(text, &format!("{name} = {{"), "};");
    if body.trim().is_empty() {
        return Vec::new();
    }
    body
        .split(',')
        .map(|part| match part.trim() {
            "true" => true,
            "false" => false,
            other => panic!("bad bool: {other}"),
        })
        .collect()
}

fn number(text: &str) -> u64 {
    if let Some(hex) = text.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).expect("hex number")
    } else {
        text.parse().expect("decimal number")
    }
}

fn parse_bytes(text: &str, name: &str) -> Vec<u8> {
    between(text, &format!("{name} = \""), "\";")
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("payload byte"))
        .collect()
}

fn parse_disassembly(path: &str, regions: &[Region]) -> BTreeMap<u64, Instruction> {
    let text = fs::read_to_string(path).expect("read disassembly");
    let mut raw = Vec::<(u64, String, Option<u64>)>::new();
    for line in text.lines() {
        let Some((left, right)) = line.split_once(":\t") else {
            continue;
        };
        let address_text = left.trim();
        if address_text.is_empty() || !address_text.chars().all(|ch| ch.is_ascii_hexdigit()) {
            continue;
        }
        let address = u64::from_str_radix(address_text, 16).expect("instruction address");
        if !regions
            .iter()
            .any(|region| address >= region.start && address < region.end)
        {
            continue;
        }
        let mnemonic = right.split_whitespace().next().unwrap_or("");
        if mnemonic == "int3" || mnemonic == "(bad)" {
            continue;
        }
        let comment_target = right
            .split("# 0x")
            .nth(1)
            .and_then(|tail| tail.split_whitespace().next())
            .and_then(|hex| u64::from_str_radix(hex, 16).ok());
        let direct_target = if mnemonic == "call" || mnemonic == "jmp" || mnemonic.starts_with('j') {
            right
                .split_whitespace()
                .nth(1)
                .map(|part| part.trim_end_matches(',').trim_start_matches("0x"))
                .and_then(|hex| u64::from_str_radix(hex, 16).ok())
        } else {
            None
        };
        raw.push((
            address,
            right.split('#').next().unwrap_or("").trim().to_owned(),
            comment_target.or(direct_target),
        ));
    }
    raw.sort_by_key(|entry| entry.0);
    let mut instructions = BTreeMap::new();
    for region in regions {
        let region_instructions: Vec<_> = raw
            .iter()
            .filter(|entry| entry.0 >= region.start && entry.0 < region.end)
            .collect();
        for (index, entry) in region_instructions.iter().enumerate() {
            let next = region_instructions
                .get(index + 1)
                .map_or(region.end, |next| next.0);
            instructions.entry(entry.0).or_insert_with(|| Instruction {
                address: entry.0,
                code: entry.1.clone(),
                target: entry.2,
                length: (next - entry.0) as usize,
            });
        }
    }
    instructions
}

fn find_displacement(bytes: &[u8], value: i64, width: usize) -> usize {
    let matches: Vec<usize> = if width == 4 {
        let needle = (value as i32).to_le_bytes();
        bytes
            .windows(4)
            .enumerate()
            .filter_map(|(index, candidate)| (candidate == needle).then_some(index))
            .collect()
    } else {
        assert_eq!(width, 1);
        let needle = value as i8 as u8;
        bytes
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| (*candidate == needle).then_some(index))
            .collect()
    };
    assert_eq!(matches.len(), 1, "non-unique displacement {value:#x} in {bytes:02x?}");
    matches[0]
}

fn root_expected_fields(path: &str, composed: bool) -> Vec<(u64, u64, u64)> {
    fs::read_to_string(path)
        .expect("read field evidence")
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let (context_index, old_index, new_index) = if composed {
                (3, 4, 6)
            } else {
                (2, 3, 4)
            };
            if fields.len() <= new_index {
                return None;
            }
            let context = fields[context_index];
            if context.starts_with("rbp")
                || context.starts_with("rsp")
                || context == "rax-OFF"
            {
                return None;
            }
            let old_value = number(fields[old_index]);
            let new_value = number(fields[new_index]);
            (old_value != new_value).then_some((number(fields[0]), old_value, new_value))
        })
        .collect()
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if !(5..=6).contains(&args.len()) {
        eprintln!(
            "usage: january_verify Profile_Jan2022.as JAN.exe CURRENT.exe JAN.objdump DIRECT_FIELDS.tsv [COMPOSED_FIELDS.tsv]"
        );
        std::process::exit(2);
    }
    let profile = fs::read_to_string(&args[0]).expect("read profile");
    let source_pe = Pe::load(&args[1]);
    let current_pe = Pe::load(&args[2]);
    let payload = parse_bytes(&profile, "PROFILE_JAN2022_ISLAND_BYTES");
    let target_handler_pattern: Vec<u8> = "48 8B C4 F3 0F 11 48 10 48 89 48 08 55 56 57 41 54 48 8D A8 48 FE FF FF 48 81 EC 98 02 00 00 49 8B 78 08 41 BA 04 00 00 00"
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    assert_eq!(
        current_pe.at(current_pe.base + 0x851f00, target_handler_pattern.len()),
        Some(target_handler_pattern.as_slice()),
        "build-128130 handler gate bytes",
    );
    assert_eq!(
        current_pe
            .bytes
            .windows(target_handler_pattern.len())
            .filter(|candidate| *candidate == target_handler_pattern)
            .count(),
        1,
        "target handler signature must be unique",
    );
    let target_banner = b"date=2026-01-28_13_00 git=128130-6dda3728e91 GameVersion=3.3.0";
    assert_eq!(
        current_pe
            .bytes
            .windows(target_banner.len())
            .filter(|candidate| *candidate == target_banner)
            .count(),
        1,
        "build banner must be unique",
    );
    assert_eq!(&payload[..12], &target_handler_pattern[..12], "entry prologue compatibility");
    assert_eq!(
        payload.len(),
        scalar(&profile, "PROFILE_JAN2022_ISLAND_SIZE") as usize
    );
    assert_eq!(scalar(&profile, "PROFILE_JAN2022_UNRESOLVED_CALLS"), 0);
    assert_eq!(scalar(&profile, "PROFILE_JAN2022_UNRESOLVED_RIP"), 0);

    let starts = numbers(&profile, "PROFILE_JAN2022_SOURCE_REGION_START_VAS");
    let ends = numbers(&profile, "PROFILE_JAN2022_SOURCE_REGION_END_VAS");
    let region_offsets = numbers(&profile, "PROFILE_JAN2022_ISLAND_REGION_OFFSETS");
    let region_lengths = numbers(&profile, "PROFILE_JAN2022_ISLAND_REGION_LENGTHS");
    assert_eq!(starts.len(), ends.len());
    assert_eq!(starts.len(), region_offsets.len());
    assert_eq!(starts.len(), region_lengths.len());
    let regions: Vec<Region> = (0..starts.len())
        .map(|index| Region {
            start: starts[index],
            end: ends[index],
            island_offset: region_offsets[index] as usize,
            island_length: region_lengths[index] as usize,
        })
        .collect();
    let instructions = parse_disassembly(&args[3], &regions);

    let field_offsets = numbers(&profile, "PROFILE_JAN2022_FIELD_OFFSETS");
    let field_sites = numbers(&profile, "PROFILE_JAN2022_FIELD_SOURCE_VAS");
    let field_widths = numbers(&profile, "PROFILE_JAN2022_FIELD_WIDTHS");
    let field_old = numbers(&profile, "PROFILE_JAN2022_FIELD_OLD_VALUES");
    let field_new = numbers(&profile, "PROFILE_JAN2022_FIELD_NEW_VALUES");
    let field_count = scalar(&profile, "PROFILE_JAN2022_FIELD_RELOCATION_COUNT") as usize;
    for length in [
        field_offsets.len(),
        field_sites.len(),
        field_widths.len(),
        field_old.len(),
        field_new.len(),
    ] {
        assert_eq!(length, field_count, "field manifest length");
    }
    let mut field_records = BTreeSet::new();
    for index in 0..field_count {
        let offset = field_offsets[index] as usize;
        let site = field_sites[index];
        let width = field_widths[index] as usize;
        let old_value = field_old[index] as i64;
        let new_value = field_new[index] as i64;
        assert!(field_records.insert((offset, site, old_value, new_value)));
        let instruction = instructions.get(&site).expect("field source instruction");
        let source_bytes = source_pe
            .at(site, instruction.length)
            .expect("field source bytes");
        let in_instruction = find_displacement(source_bytes, old_value, width);
        let mut matching_region = false;
        for region in &regions {
            if site < region.start || site >= region.end {
                continue;
            }
            let source_length = (region.end - region.start) as usize;
            let shift = region.island_length - source_length;
            let expected = region.island_offset
                + (site - region.start) as usize
                + if shift == 3 && site > region.start { 3 } else { 0 }
                + in_instruction;
            if expected == offset {
                matching_region = true;
                break;
            }
        }
        assert!(matching_region, "field offset does not map to source site {site:#x}");
        match width {
            1 => assert_eq!(payload[offset], new_value as i8 as u8),
            4 => assert_eq!(
                i32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap()),
                new_value as i32
            ),
            other => panic!("unsupported field width {other}"),
        }
    }

    let manifested_fields: BTreeSet<(u64, u64, u64)> = field_sites
        .iter()
        .zip(&field_old)
        .zip(&field_new)
        .map(|((site, old), new)| (*site, *old, *new))
        .collect();
    let root_start = regions[0].start;
    let root_end = regions[0].end;
    let mut missing_fields = Vec::new();
    for path_and_kind in [(&args[4], false)] {
        for row in root_expected_fields(path_and_kind.0, path_and_kind.1) {
            if row.0 >= root_start && row.0 < root_end && !manifested_fields.contains(&row) {
                missing_fields.push(row);
            }
        }
    }
    // A composed January->Spring->current file is accepted as an optional sixth
    // positional argument in older invocations by placing it after the direct map.
    if let Some(composed_path) = args.get(5) {
        for row in root_expected_fields(composed_path, true) {
            if row.0 >= root_start && row.0 < root_end && !manifested_fields.contains(&row) {
                missing_fields.push(row);
            }
        }
    }
    missing_fields.sort_unstable();
    missing_fields.dedup();
    assert!(missing_fields.is_empty(), "unmanifested field mappings: {missing_fields:#x?}");

    let call_offsets: BTreeSet<usize> = numbers(&profile, "PROFILE_JAN2022_CALL_RELOC_OFFSETS")
        .into_iter()
        .map(|value| value as usize)
        .collect();
    let call_targets = numbers(&profile, "PROFILE_JAN2022_CALL_TARGET_ISLAND_OFFSETS");
    let rip_offsets: BTreeSet<usize> = numbers(&profile, "PROFILE_JAN2022_RIP_RELOC_OFFSETS")
        .into_iter()
        .map(|value| value as usize)
        .collect();
    let rip_targets = numbers(&profile, "PROFILE_JAN2022_RIP_TARGET_RVAS");
    let rip_is_island = booleans(&profile, "PROFILE_JAN2022_RIP_TARGET_IS_ISLAND");
    assert_eq!(call_offsets.len(), call_targets.len());
    assert_eq!(rip_offsets.len(), rip_targets.len());
    assert_eq!(rip_offsets.len(), rip_is_island.len());
    assert_eq!(
        call_offsets.len(),
        scalar(&profile, "PROFILE_JAN2022_CALL_RELOCATION_COUNT") as usize
    );
    assert_eq!(
        rip_offsets.len(),
        scalar(&profile, "PROFILE_JAN2022_RIP_RELOCATION_COUNT") as usize
    );

    let mut expected_calls = BTreeSet::new();
    let mut expected_rips = BTreeSet::new();
    for region in &regions {
        let source_length = (region.end - region.start) as usize;
        let shift = region.island_length - source_length;
        assert!(shift == 0 || (shift == 3 && region.start == 0x1405edcf0));
        for instruction in instructions
            .values()
            .filter(|instruction| instruction.address >= region.start && instruction.address < region.end)
        {
            if shift == 3 && instruction.address == region.start {
                continue;
            }
            let mnemonic = instruction.code.split_whitespace().next().unwrap_or("");
            let is_call = mnemonic == "call"
                && instruction
                    .code
                    .split_whitespace()
                    .nth(1)
                    .is_some_and(|operand| {
                        let value = operand.trim_end_matches(',').trim_start_matches("0x");
                        !value.is_empty() && value.chars().all(|ch| ch.is_ascii_hexdigit())
                    });
            let is_rip = instruction.code.contains("rip+0x") || instruction.code.contains("rip-0x");
            if !is_call && !is_rip {
                continue;
            }
            let target = instruction.target.expect("relative target");
            let source_bytes = source_pe
                .at(instruction.address, instruction.length)
                .expect("relative source bytes");
            let delta = target as i64 - (instruction.address + instruction.length as u64) as i64;
            let in_instruction = find_displacement(source_bytes, delta, 4);
            let operand = region.island_offset
                + (instruction.address - region.start) as usize
                + shift
                + in_instruction;
            if is_call {
                expected_calls.insert(operand);
            } else {
                expected_rips.insert(operand);
            }
        }
    }
    let adapter = scalar(&profile, "PROFILE_JAN2022_INTERPOLATION_ADAPTER_OFFSET") as usize;
    expected_calls.insert(adapter + 10);
    let helper_region = regions
        .iter()
        .find(|region| region.start == 0x1405edcf0)
        .expect("historical helper region");
    expected_rips.insert(helper_region.island_offset + 3);
    assert_eq!(expected_calls, call_offsets, "call relocation coverage");
    assert_eq!(expected_rips, rip_offsets, "RIP relocation coverage");

    let reloc_offsets = numbers(&profile, "PROFILE_JAN2022_RELOC_OFFSETS");
    let reloc_targets = numbers(&profile, "PROFILE_JAN2022_RELOC_TARGET_RVAS");
    let reloc_is_island = booleans(&profile, "PROFILE_JAN2022_RELOC_TARGET_IS_ISLAND");
    assert_eq!(reloc_offsets.len(), reloc_targets.len());
    assert_eq!(reloc_offsets.len(), reloc_is_island.len());
    let union: BTreeSet<usize> = reloc_offsets.iter().map(|value| *value as usize).collect();
    let expected_union: BTreeSet<usize> = call_offsets.union(&rip_offsets).copied().collect();
    assert_eq!(union, expected_union, "combined relocation manifest");

    let absolute_offsets = numbers(&profile, "PROFILE_JAN2022_ABS64_OFFSETS");
    let absolute_targets = numbers(&profile, "PROFILE_JAN2022_ABS64_TARGET_RVAS");
    assert_eq!(absolute_offsets.len(), absolute_targets.len());
    for (&offset, &target_rva) in absolute_offsets.iter().zip(&absolute_targets) {
        let offset = offset as usize;
        assert_eq!(&payload[offset - 2..offset], &[0x48, 0xb8]);
        assert_eq!(&payload[offset + 8..offset + 10], &[0xff, 0xe0]);
        assert!(
            current_pe.at(current_pe.base + target_rva, 1).is_some(),
            "absolute target outside current image: {target_rva:#x}"
        );
    }

    let init_sources = numbers(&profile, "PROFILE_JAN2022_INIT_SOURCE_VAS");
    let init_offsets = numbers(&profile, "PROFILE_JAN2022_INIT_SHADOW_OFFSETS");
    let init_values = numbers(&profile, "PROFILE_JAN2022_INIT_VALUES");
    assert_eq!(init_sources.len(), 4);
    assert_eq!(init_offsets.len(), init_values.len());
    for (&offset, &value) in init_offsets.iter().zip(&init_values) {
        let offset = offset as usize;
        assert_eq!(
            u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap()),
            value as u32
        );
    }

    let mock_island = 0x0000_0180_0000_0000_u64;
    let mock_image = 0x0000_0001_4000_0000_u64;
    let mut relocated = payload.clone();
    for (&offset, &target_rva) in absolute_offsets.iter().zip(&absolute_targets) {
        let offset = offset as usize;
        relocated[offset..offset + 8].copy_from_slice(&(mock_image + target_rva).to_le_bytes());
    }
    for index in 0..reloc_offsets.len() {
        let offset = reloc_offsets[index] as usize;
        let target = if reloc_is_island[index] {
            assert!((reloc_targets[index] as usize) < relocated.len());
            mock_island + reloc_targets[index]
        } else {
            assert!(
                current_pe
                    .at(mock_image + reloc_targets[index], 1)
                    .is_some(),
                "image relocation target outside image"
            );
            mock_image + reloc_targets[index]
        };
        let delta = target as i128 - (mock_island + offset as u64 + 4) as i128;
        assert!((i32::MIN as i128..=i32::MAX as i128).contains(&delta));
        relocated[offset..offset + 4].copy_from_slice(&(delta as i32).to_le_bytes());
        let encoded = i32::from_le_bytes(relocated[offset..offset + 4].try_into().unwrap());
        assert_eq!(
            (mock_island + offset as u64 + 4).wrapping_add_signed(encoded as i64),
            target
        );
    }

    let output = format!("{}.verified.bin", &args[0]);
    fs::write(&output, relocated).expect("write relocated payload");
    println!(
        "January payload statically verified: bytes={} regions={} fields={} calls={} RIP={} abs64={} init_defaults={} output={}",
        payload.len(),
        regions.len(),
        field_count,
        call_offsets.len(),
        rip_offsets.len(),
        absolute_offsets.len(),
        init_values.len(),
        output,
    );
}
