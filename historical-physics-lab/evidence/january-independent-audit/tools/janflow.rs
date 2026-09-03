// janflow — is the base register the same object at both sites?
//
// The omitted-remap finding only stands if a displacement like [rdi+0x1758]
// names the same structure at the site that WAS remapped and the site that was
// not. This tool answers that the only way that is safe without the January
// binary: by finding every instruction in the copied region that WRITES the
// base register. A register written exactly once, in the prologue, holds one
// object for the whole region, and then two sites with the same displacement
// are the same field by construction.
//
// std only; objdump as the decoder.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::{env, fs};

fn disassemble(path: &str, start: usize, stop: usize) -> Vec<(usize, Vec<u8>, String)> {
    let output = Command::new("objdump")
        .args([
            "-D", "-b", "binary", "-m", "i386:x86-64", "-M", "intel",
            &format!("--start-address={start}"),
            &format!("--stop-address={stop}"),
            path,
        ])
        .output()
        .expect("objdump");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut out: Vec<(usize, Vec<u8>, String)> = Vec::new();
    for line in text.lines() {
        let Some((left, rest)) = line.split_once(":\t") else { continue };
        let left = left.trim();
        if left.is_empty() || !left.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let offset = usize::from_str_radix(left, 16).unwrap();
        let (byte_text, insn_text) = match rest.split_once('\t') {
            Some((b, t)) => (b, t),
            None => (rest, ""),
        };
        let bytes: Vec<u8> = byte_text
            .split_whitespace()
            .filter_map(|b| u8::from_str_radix(b, 16).ok())
            .collect();
        if insn_text.trim().is_empty() {
            if let Some(last) = out.last_mut() {
                last.1.extend_from_slice(&bytes);
                continue;
            }
        }
        out.push((offset, bytes, insn_text.split('#').next().unwrap_or("").trim().to_string()));
    }
    out
}

/// does this instruction write `reg` as a full 64-bit destination?
fn writes(text: &str, reg: &str) -> bool {
    let mnemonic = text.split_whitespace().next().unwrap_or("");
    let rest = text[mnemonic.len()..].trim();
    // first operand is the destination for the x86 forms that matter here
    let first = rest.split(',').next().unwrap_or("").trim();
    let destination_is_reg = first == reg;
    match mnemonic {
        // pure reads
        "cmp" | "test" | "push" | "jmp" | "call" | "ret" | "nop" | "comiss" | "ucomiss" => false,
        // writes to first operand
        "mov" | "lea" | "add" | "sub" | "or" | "and" | "xor" | "movzx" | "movsx" | "movsxd"
        | "imul" | "shl" | "shr" | "sar" | "inc" | "dec" | "neg" | "not" | "adc" | "sbb"
        | "cmov" | "xchg" | "bt" | "movabs" => destination_is_reg,
        "pop" => first == reg,
        other => {
            // cmovCC, setCC and the rest: treat any first-operand match as a write
            if other.starts_with("cmov") || other.starts_with("set") {
                destination_is_reg
            } else {
                destination_is_reg
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: janflow island.bin DIR");
        std::process::exit(2);
    }
    let island = &args[0];
    let dir = &args[1];
    let manifest = fs::read_to_string(format!("{dir}/manifest.tsv")).expect("manifest");

    // root region is the first region row
    let mut regions = Vec::new();
    let mut fields = Vec::new();
    for line in manifest.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 {
            continue;
        }
        let parse = |s: &str| -> u64 {
            let s = s.trim();
            if let Some(h) = s.strip_prefix("0x") { u64::from_str_radix(h, 16).unwrap() } else { s.parse().unwrap() }
        };
        if f[0] == "region" {
            regions.push((parse(f[2]) as usize, parse(f[3]) as usize, parse(f[4]), parse(f[5])));
        }
        if f[0] == "field" {
            fields.push((parse(f[2]) as usize, parse(f[3]) as usize, parse(f[4]), parse(f[5]) as i64, parse(f[6]) as i64));
        }
    }
    let (root_offset, root_length, root_start_va, root_end_va) = regions[0];
    println!(
        "root region: island {root_offset}..{}, source {root_start_va:#x}..{root_end_va:#x}",
        root_offset + root_length
    );

    let insns = disassemble(island, root_offset, root_offset + root_length);
    println!("decoded {} instructions in the root region\n", insns.len());

    // ---- who writes the base registers of interest? ---------------------
    let interesting = ["rdi", "rdx", "rax", "rcx", "rsi", "r15", "r13", "rbx", "r14", "r12"];
    let mut writers: BTreeMap<&str, Vec<(usize, String)>> = BTreeMap::new();
    for reg in interesting {
        for (offset, _, text) in &insns {
            if writes(text, reg) {
                writers.entry(reg).or_default().push((*offset, text.clone()));
            }
        }
    }
    println!("== definitions of each base register inside the root region ==");
    for reg in interesting {
        let list = writers.get(reg).cloned().unwrap_or_default();
        println!("{reg}: {} definition(s)", list.len());
        for (offset, text) in list.iter().take(6) {
            println!("    island {offset}: {text}");
        }
        if list.len() > 6 {
            println!("    ... {} more", list.len() - 6);
        }
    }

    // ---- for each displacement, list every site, patched or not ---------
    println!("\n== every site of the contested displacements ==");
    let patched_bytes: BTreeSet<usize> = fields.iter().flat_map(|f| f.0..f.0 + f.1).collect();
    let patched_by_offset: BTreeMap<usize, (i64, i64)> =
        fields.iter().map(|f| (f.0, (f.3, f.4))).collect();

    // contested = a (base, january offset) pair that appears both patched and unpatched
    let mut sites: BTreeMap<(String, i64), Vec<(usize, bool, String)>> = BTreeMap::new();
    for (offset, bytes, text) in &insns {
        // pull [base+0xdisp]
        let raw = text.as_bytes();
        let mut i = 0;
        while i < raw.len() {
            if raw[i] != b'[' {
                i += 1;
                continue;
            }
            let Some(close) = text[i..].find(']') else { break };
            let inner = &text[i + 1..i + close];
            i += close + 1;
            let Some(plus) = inner.rfind('+') else { continue };
            let base = inner[..plus].to_string();
            let Some(hex) = inner[plus + 1..].trim().strip_prefix("0x") else { continue };
            let Ok(value) = i64::from_str_radix(hex, 16) else { continue };
            let touched = (*offset..offset + bytes.len()).any(|b| patched_bytes.contains(&b));
            let january_value = if touched {
                // recover the january offset from the manifest entry inside this instruction
                (*offset..offset + bytes.len())
                    .find_map(|b| patched_by_offset.get(&b).map(|(old, _)| *old))
                    .unwrap_or(value)
            } else {
                value
            };
            sites.entry((base, january_value)).or_default().push((*offset, touched, text.clone()));
        }
    }
    let mut contested = 0;
    for ((base, january), entries) in &sites {
        let patched = entries.iter().filter(|e| e.1).count();
        let unpatched = entries.len() - patched;
        if patched > 0 && unpatched > 0 {
            contested += 1;
            println!(
                "\nCONTESTED [{base}+{january:#x}]: {patched} remapped, {unpatched} left at the January offset"
            );
            for (offset, touched, text) in entries {
                println!("    island {offset:5} {} {text}", if *touched { "REMAPPED " } else { "UNTOUCHED" });
            }
        }
    }
    println!("\ncontested (base, offset) pairs: {contested}");
}
