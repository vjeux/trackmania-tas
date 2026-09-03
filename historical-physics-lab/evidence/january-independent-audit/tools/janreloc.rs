// janreloc — two checks the shipped verifier cannot make.
//
// 1. INCOMPLETE CALLS / RIP REFS. The generator reports "unresolved=0", but it
//    counts only what its own aligner looked at. This walks the island as
//    shipped and asks the opposite question: does every instruction that
//    encodes a rel32 (direct call, direct jmp, or any RIP-relative operand)
//    have a relocation entry inside its own bytes? An instruction without one
//    keeps a displacement computed for the January image and will branch or
//    load at a wild address once the island is allocated somewhere else.
//
// 2. HELPER IDENTITY. Every external call leaves through an absolute thunk
//    whose target is a raw RVA in the current image. Nothing in the shipped
//    verifier checks that the RVA is the ENTRY of a function rather than a
//    point inside one. A target that is not preceded by padding or a return,
//    and is not aligned, is a call landing mid-function.
//
// std only; objdump as the decoder.

use std::collections::BTreeSet;
use std::process::Command;
use std::{env, fs};

struct Pe {
    bytes: Vec<u8>,
    base: u64,
    sections: Vec<(u64, u64, usize, usize)>,
}

impl Pe {
    fn load(path: &str) -> Self {
        let bytes = fs::read(path).expect("read PE");
        let pe = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
        let n = u16::from_le_bytes(bytes[pe + 6..pe + 8].try_into().unwrap()) as usize;
        let os = u16::from_le_bytes(bytes[pe + 20..pe + 22].try_into().unwrap()) as usize;
        let o = pe + 24;
        let base = u64::from_le_bytes(bytes[o + 24..o + 32].try_into().unwrap());
        let t = o + os;
        let mut sections = Vec::new();
        for i in 0..n {
            let q = t + i * 40;
            let vs = u32::from_le_bytes(bytes[q + 8..q + 12].try_into().unwrap()) as u64;
            let va = u32::from_le_bytes(bytes[q + 12..q + 16].try_into().unwrap()) as u64;
            let rs = u32::from_le_bytes(bytes[q + 16..q + 20].try_into().unwrap()) as usize;
            let rp = u32::from_le_bytes(bytes[q + 20..q + 24].try_into().unwrap()) as usize;
            sections.push((va, vs.max(rs as u64), rp, rs));
        }
        Self { bytes, base, sections }
    }
    fn at_rva(&self, rva: u64, size: usize) -> Option<&[u8]> {
        for &(va, vs, rp, rs) in &self.sections {
            if rva >= va && rva + size as u64 <= va + vs {
                let off = rp + (rva - va) as usize;
                if off + size <= rp + rs {
                    return self.bytes.get(off..off + size);
                }
            }
        }
        None
    }
}

fn disassemble(path: &str, start: usize, stop: usize) -> Vec<(usize, Vec<u8>, String)> {
    let out = Command::new("objdump")
        .args([
            "-D", "-b", "binary", "-m", "i386:x86-64", "-M", "intel",
            &format!("--start-address={start}"),
            &format!("--stop-address={stop}"),
            path,
        ])
        .output()
        .expect("objdump");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut v: Vec<(usize, Vec<u8>, String)> = Vec::new();
    for line in text.lines() {
        let Some((left, rest)) = line.split_once(":\t") else { continue };
        let left = left.trim();
        if left.is_empty() || !left.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let offset = usize::from_str_radix(left, 16).unwrap();
        let (bt, it) = match rest.split_once('\t') {
            Some((b, t)) => (b, t),
            None => (rest, ""),
        };
        let bytes: Vec<u8> = bt.split_whitespace().filter_map(|b| u8::from_str_radix(b, 16).ok()).collect();
        if it.trim().is_empty() {
            if let Some(last) = v.last_mut() {
                last.1.extend_from_slice(&bytes);
                continue;
            }
        }
        v.push((offset, bytes, it.trim().to_string()));
    }
    v
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: janreloc island.bin DIR CURRENT.exe");
        std::process::exit(2);
    }
    let island_path = &args[0];
    let dir = &args[1];
    let current = Pe::load(&args[2]);
    let manifest = fs::read_to_string(format!("{dir}/manifest.tsv")).expect("manifest");
    let island = fs::read(island_path).expect("island");

    let parse = |s: &str| -> u64 {
        let s = s.trim();
        if let Some(h) = s.strip_prefix("0x") { u64::from_str_radix(h, 16).unwrap() } else { s.parse().unwrap() }
    };
    let mut regions = Vec::new();
    let mut reloc_bytes = BTreeSet::<usize>::new();
    let mut reloc_sites = BTreeSet::<usize>::new();
    for line in manifest.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 { continue }
        if f[0] == "region" {
            regions.push((parse(f[2]) as usize, parse(f[3]) as usize, parse(f[4]), parse(f[5])));
        }
        if f[0] == "reloc" {
            let o = parse(f[2]) as usize;
            reloc_sites.insert(o);
            for b in o..o + 4 { reloc_bytes.insert(b); }
        }
    }

    let mut defects = 0usize;

    // ---- 1. unrelocated rel32 / RIP references --------------------------
    println!("== unrelocated relative references in copied regions ==");
    let mut examined = 0usize;
    let mut missing = Vec::new();
    for (island_offset, island_length, source_start, _) in &regions {
        for (offset, bytes, text) in disassemble(island_path, *island_offset, island_offset + island_length) {
            if bytes.is_empty() { continue }
            let mnemonic = text.split_whitespace().next().unwrap_or("");
            let operand = text[mnemonic.len()..].trim();
            // direct rel32 branch: operand is a bare hex address
            let first = operand.split(',').next().unwrap_or("").trim();
            let direct_branch = (mnemonic == "call" || mnemonic == "jmp")
                && !first.is_empty()
                && first.chars().all(|c| c.is_ascii_hexdigit())
                && bytes.first().is_some_and(|b| *b == 0xE8 || *b == 0xE9);
            let rip_relative = text.contains("[rip+") || text.contains("[rip-");
            if !direct_branch && !rip_relative { continue }
            examined += 1;
            let covered = (offset..offset + bytes.len()).any(|b| reloc_bytes.contains(&b));
            if !covered {
                let source_va = source_start + (offset - island_offset) as u64;
                missing.push((offset, source_va, text.clone()));
            }
        }
    }
    if missing.is_empty() {
        println!("PASS: all {examined} relative references inside copied regions carry a relocation");
    } else {
        for (offset, va, text) in &missing {
            println!("DEFECT\tunrelocated\tisland {offset} (January {va:#x}): `{text}` has no relocation entry");
            defects += 1;
        }
        println!("{} of {examined} relative references are unrelocated", missing.len());
    }

    // ---- 1b. the same question over the WHOLE island, including the
    //          generated adapter and thunk area ---------------------------
    println!("\n== relative references outside the copied regions ==");
    let covered_region = |x: usize| regions.iter().any(|(o, l, _, _)| x >= *o && x < o + l);
    let mut outside = Vec::new();
    for (offset, bytes, text) in disassemble(island_path, 0, island.len()) {
        if bytes.is_empty() || covered_region(offset) { continue }
        let mnemonic = text.split_whitespace().next().unwrap_or("");
        let first = text[mnemonic.len()..].trim().split(',').next().unwrap_or("").trim().to_string();
        let direct_branch = (mnemonic == "call" || mnemonic == "jmp")
            && !first.is_empty()
            && first.chars().all(|c| c.is_ascii_hexdigit())
            && bytes.first().is_some_and(|b| *b == 0xE8 || *b == 0xE9);
        if !direct_branch { continue }
        let covered = (offset..offset + bytes.len()).any(|b| reloc_bytes.contains(&b));
        if !covered {
            outside.push((offset, text.clone()));
        }
    }
    if outside.is_empty() {
        println!("PASS: no unrelocated direct branch in the generated adapter/pool/thunk area");
    } else {
        for (offset, text) in &outside {
            println!("DEFECT\tunrelocated-generated\tisland {offset}: `{text}`");
            defects += 1;
        }
    }

    // ---- 2. absolute thunk target identity ------------------------------
    println!("\n== absolute thunk targets: function entry or mid-function? ==");
    let profile_dir = dir;
    let _ = profile_dir;
    // The imm64 fields are ZERO placeholders in the shipped payload; the loader
    // writes imageBase+RVA at install time. Read the declared targets from the
    // profile, and check the placeholder shape separately.
    let profile_text = fs::read_to_string(&args[3]).expect("read Profile_Jan2022.as");
    let list = |name: &str| -> Vec<u64> {
        let start = format!("{name} = {{");
        let b = profile_text.find(&start).map(|p| p + start.len()).expect("array");
        let e = profile_text[b..].find("};").unwrap() + b;
        profile_text[b..e]
            .split(',')
            .map(|p| {
                let p = p.trim();
                if let Some(h) = p.strip_prefix("0x") { u64::from_str_radix(h, 16).unwrap() } else { p.parse().unwrap() }
            })
            .collect()
    };
    let abs_offsets = list("PROFILE_JAN2022_ABS64_OFFSETS");
    let abs_targets = list("PROFILE_JAN2022_ABS64_TARGET_RVAS");
    let mut targets: Vec<(usize, u64)> = Vec::new();
    let mut placeholder_bad = 0;
    for (o, t) in abs_offsets.iter().zip(&abs_targets) {
        let o = *o as usize;
        if island[o..o + 8] != [0u8; 8] {
            placeholder_bad += 1;
        }
        targets.push((o - 2, *t));
    }
    println!(
        "{} absolute thunks declared; {} carry a non-zero imm64 placeholder (expected 0, filled at install)",
        targets.len(), placeholder_bad
    );
    let mut mid_function = 0;
    for (offset, rva) in &targets {
        let Some(head) = current.at_rva(*rva, 8) else {
            println!("DEFECT\tthunk-target\tthunk at {offset}: rva {rva:#x} is not mapped in the current image");
            defects += 1;
            continue;
        };
        let previous = current.at_rva(rva.wrapping_sub(1), 1).map(|b| b[0]);
        let aligned = rva % 16 == 0;
        let padded = matches!(previous, Some(0xCC) | Some(0x90) | Some(0xC3) | Some(0xE9) | Some(0xEB));
        // common Win64 entry openings
        let entry_like = head.starts_with(&[0x48, 0x89, 0x5C])      // mov [rsp+x],rbx
            || head.starts_with(&[0x48, 0x83, 0xEC])                 // sub rsp,imm8
            || head.starts_with(&[0x48, 0x81, 0xEC])                 // sub rsp,imm32
            || head.starts_with(&[0x40, 0x53])                       // push rbx
            || head.starts_with(&[0x55])                             // push rbp
            || head.starts_with(&[0x53])
            || head.starts_with(&[0x56])
            || head.starts_with(&[0x57])
            || head.starts_with(&[0x48, 0x8B, 0xC4])                 // mov rax,rsp
            || head.starts_with(&[0xF3, 0x0F, 0x10])                 // movss xmm,...
            || head.starts_with(&[0x0F, 0x57])                       // xorps
            || head.starts_with(&[0xC3]);
        if !(padded || aligned || entry_like) {
            mid_function += 1;
            println!(
                "WARN\tthunk-target\tthunk at {offset}: rva {rva:#x} is unaligned, unpadded and has no entry-like prologue (bytes {:02x?})",
                head
            );
        }
    }
    if mid_function == 0 {
        println!("PASS: every absolute thunk target is aligned, padded, or opens with a recognised prologue");
    } else {
        println!("{mid_function} thunk target(s) do not look like function entries");
    }

    // ---- 3. entry patch compatibility -----------------------------------
    println!("\n== island entry vs current handler ==");
    let handler_rva = 0x851f00u64;
    if let Some(handler) = current.at_rva(handler_rva, 64) {
        let common = island.iter().zip(handler).take_while(|(a, b)| a == b).count();
        println!("island and current handler share their first {common} bytes");
        if common < 12 {
            println!("DEFECT\tentry\tisland entry diverges before the 12-byte patch window");
            defects += 1;
        }
    }

    println!("\nJANRELOC SUMMARY defects={defects}");
}
