// janasm — instruction-level audit of the January island.
//
// Everything here is a DIFFERENTIAL test: the island is disassembled twice,
// once as shipped and once with every declared old field value restored. A
// legitimate field remap must show up as a change in exactly one memory
// displacement of one instruction, with identical mnemonic, length and operand
// shape on both sides. Anything else means the patch landed on bytes that are
// not the ModRM displacement it claimed (the alias class), or corrupted the
// instruction stream.
//
// It also hunts the two omission classes that no count can reveal:
//   * a struct displacement left unpatched on a base register where the same
//     January offset was patched elsewhere;
//   * a January struct offset carried as an IMMEDIATE rather than a
//     displacement, which the generator's displacement-only rewriter cannot see.
//
// std only; objdump is driven as a subprocess disassembler oracle.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::{env, fs};

#[derive(Clone, Debug)]
struct Insn {
    offset: usize,
    bytes: Vec<u8>,
    text: String,
}

impl Insn {
    fn mnemonic(&self) -> &str {
        self.text.split_whitespace().next().unwrap_or("")
    }
    /// operand text with every hex literal blanked, so two instructions that
    /// differ only in a constant compare equal
    fn shape(&self) -> String {
        let mut out = String::new();
        let bytes = self.text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 2 < bytes.len() && bytes[i] == b'0' && bytes[i + 1] == b'x' {
                let mut j = i + 2;
                while j < bytes.len() && (bytes[j] as char).is_ascii_hexdigit() {
                    j += 1;
                }
                out.push_str("IMM");
                i = j;
                continue;
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out
    }
    /// every hex literal in the operand text, in order
    fn literals(&self) -> Vec<u64> {
        let mut out = Vec::new();
        let bytes = self.text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 2 < bytes.len() && bytes[i] == b'0' && bytes[i + 1] == b'x' {
                let mut j = i + 2;
                while j < bytes.len() && (bytes[j] as char).is_ascii_hexdigit() {
                    j += 1;
                }
                if let Ok(v) = u64::from_str_radix(&self.text[i + 2..j], 16) {
                    out.push(v);
                }
                i = j;
                continue;
            }
            i += 1;
        }
        out
    }
    /// memory operands of the form [base+0xdisp] / [base-0xdisp], as (base, signed disp)
    fn memory_displacements(&self) -> Vec<(String, i64)> {
        let mut out = Vec::new();
        let text = &self.text;
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
            // inner looks like "rcx+0x1378" or "rax+rdx*4+0x20" or "rip+0x1234"
            let (sign, split) = match (inner.rfind('+'), inner.rfind('-')) {
                (Some(p), Some(m)) if m > p => (-1i64, m),
                (Some(p), _) => (1i64, p),
                (None, Some(m)) => (-1i64, m),
                (None, None) => continue,
            };
            let base = inner[..split].to_string();
            let disp_text = inner[split + 1..].trim();
            let Some(hex) = disp_text.strip_prefix("0x") else { continue };
            let Ok(value) = u64::from_str_radix(hex, 16) else { continue };
            out.push((base, sign * value as i64));
        }
        out
    }
}

fn disassemble(path: &str, start: usize, stop: usize) -> Vec<Insn> {
    let output = Command::new("objdump")
        .args([
            "-D",
            "-b",
            "binary",
            "-m",
            "i386:x86-64",
            "-M",
            "intel",
            &format!("--start-address={start}"),
            &format!("--stop-address={stop}"),
            path,
        ])
        .output()
        .expect("run objdump");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut insns: Vec<Insn> = Vec::new();
    for line in text.lines() {
        let Some((left, rest)) = line.split_once(":\t") else { continue };
        let left = left.trim();
        if left.is_empty() || !left.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let offset = usize::from_str_radix(left, 16).expect("offset");
        let (byte_text, insn_text) = match rest.split_once('\t') {
            Some((b, t)) => (b, t),
            None => (rest, ""),
        };
        let bytes: Vec<u8> = byte_text
            .split_whitespace()
            .filter_map(|b| u8::from_str_radix(b, 16).ok())
            .collect();
        if insn_text.trim().is_empty() {
            // continuation of the previous instruction's byte column
            if let Some(last) = insns.last_mut() {
                last.bytes.extend_from_slice(&bytes);
                continue;
            }
        }
        insns.push(Insn {
            offset,
            bytes,
            text: insn_text.split('#').next().unwrap_or("").trim().to_string(),
        });
    }
    insns
}

fn covering(insns: &[Insn], offset: usize) -> Option<&Insn> {
    insns
        .iter()
        .find(|i| offset >= i.offset && offset < i.offset + i.bytes.len().max(1))
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: janasm DIR patched.bin unpatched.bin");
        std::process::exit(2);
    }
    let dir = &args[0];
    let patched_path = &args[1];
    let unpatched_path = &args[2];
    let manifest = fs::read_to_string(format!("{dir}/manifest.tsv")).expect("manifest");

    struct Field {
        index: usize,
        offset: usize,
        width: usize,
        site: u64,
        old: i64,
        new: i64,
    }
    let mut fields = Vec::new();
    let mut regions = Vec::new();
    let mut reloc_offsets = BTreeSet::<usize>::new();
    for line in manifest.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 {
            continue;
        }
        let parse = |s: &str| -> u64 {
            let s = s.trim();
            if let Some(h) = s.strip_prefix("0x") {
                u64::from_str_radix(h, 16).unwrap()
            } else {
                s.parse().unwrap()
            }
        };
        match f[0] {
            "field" => fields.push(Field {
                index: f[1].parse().unwrap(),
                offset: parse(f[2]) as usize,
                width: parse(f[3]) as usize,
                site: parse(f[4]),
                old: parse(f[5]) as i64,
                new: parse(f[6]) as i64,
            }),
            "region" => regions.push((parse(f[2]) as usize, parse(f[3]) as usize, parse(f[4]), parse(f[5]))),
            "reloc" => {
                reloc_offsets.insert(parse(f[2]) as usize);
            }
            _ => {}
        }
    }

    let mut defects = 0usize;
    let mut warns = 0usize;

    // ---------------- differential field-patch validation ----------------
    // disassemble each copied region in both variants
    let mut patched_all: Vec<Insn> = Vec::new();
    let mut unpatched_all: Vec<Insn> = Vec::new();
    for (island_offset, island_length, _, _) in &regions {
        patched_all.extend(disassemble(patched_path, *island_offset, island_offset + island_length));
        unpatched_all.extend(disassemble(unpatched_path, *island_offset, island_offset + island_length));
    }
    patched_all.sort_by_key(|i| i.offset);
    patched_all.dedup_by_key(|i| i.offset);
    unpatched_all.sort_by_key(|i| i.offset);
    unpatched_all.dedup_by_key(|i| i.offset);

    println!("== differential field-patch validation ==");
    let mut alias_hits = Vec::new();
    let mut ok_patches = 0;
    for field in &fields {
        let Some(pi) = covering(&patched_all, field.offset) else {
            println!("DEFECT\tfield#{}\tno instruction covers island offset {}", field.index, field.offset);
            defects += 1;
            continue;
        };
        let Some(ui) = covering(&unpatched_all, field.offset) else {
            println!("DEFECT\tfield#{}\tno unpatched instruction covers offset {}", field.index, field.offset);
            defects += 1;
            continue;
        };
        // 1. instruction identity must survive the patch
        if pi.offset != ui.offset || pi.bytes.len() != ui.bytes.len() || pi.mnemonic() != ui.mnemonic() {
            println!(
                "DEFECT\tfield#{}\tpatch at {} changes the instruction itself: `{}` ({} B) vs `{}` ({} B)",
                field.index, field.offset, ui.text, ui.bytes.len(), pi.text, pi.bytes.len()
            );
            defects += 1;
            continue;
        }
        // 2. the only textual difference must be a memory displacement
        if pi.shape() != ui.shape() {
            println!(
                "DEFECT\tfield#{}\tpatch changes operand shape: `{}` -> `{}`",
                field.index, ui.text, pi.text
            );
            defects += 1;
            continue;
        }
        let old_disps = ui.memory_displacements();
        let new_disps = pi.memory_displacements();
        let changed_disp = old_disps
            .iter()
            .zip(&new_disps)
            .any(|((ob, ov), (nb, nv))| ob == nb && *ov == field.old && *nv == field.new);
        if changed_disp {
            ok_patches += 1;
            continue;
        }
        // 3. not a displacement: did an immediate move instead?
        let old_literals = ui.literals();
        let new_literals = pi.literals();
        let immediate_moved = old_literals
            .iter()
            .zip(&new_literals)
            .any(|(o, n)| *o == field.old as u64 && *n == field.new as u64)
            && !changed_disp;
        if immediate_moved {
            println!(
                "DEFECT\tfield#{}\tALIAS: patch rewrote a non-displacement literal. site {:#x}, {:#x}->{:#x}: `{}` -> `{}`",
                field.index, field.site, field.old, field.new, ui.text, pi.text
            );
            alias_hits.push(field.index);
            defects += 1;
        } else {
            println!(
                "DEFECT\tfield#{}\tpatch produced no expected literal change. site {:#x}, {:#x}->{:#x}: `{}` -> `{}`",
                field.index, field.site, field.old, field.new, ui.text, pi.text
            );
            defects += 1;
        }
    }
    println!("field patches validated as true displacement remaps: {ok_patches}/{}", fields.len());

    // ---------------- base-register aware field map ----------------------
    println!("\n== field map by base register ==");
    let mut patched_field_offsets = BTreeMap::<usize, &Field>::new();
    for f in &fields {
        patched_field_offsets.insert(f.offset, f);
    }
    // map each field patch to the base register of the instruction it sits in
    let mut by_base: BTreeMap<String, BTreeMap<i64, BTreeSet<i64>>> = BTreeMap::new();
    let mut field_base = BTreeMap::<usize, String>::new();
    for field in &fields {
        let Some(pi) = covering(&patched_all, field.offset) else { continue };
        let base = pi
            .memory_displacements()
            .into_iter()
            .find(|(_, v)| *v == field.new)
            .map(|(b, _)| b)
            .unwrap_or_else(|| "?".into());
        field_base.insert(field.index, base.clone());
        by_base.entry(base).or_default().entry(field.old).or_default().insert(field.new);
    }
    let mut contradictions = 0;
    for (base, map) in &by_base {
        for (old, news) in map {
            if news.len() > 1 {
                println!(
                    "DEFECT\tfieldmap\tbase `{base}`: January offset {old:#x} remapped to {:?} at different sites",
                    news.iter().map(|n| format!("{n:#x}")).collect::<Vec<_>>()
                );
                contradictions += 1;
                defects += 1;
            }
        }
    }
    if contradictions == 0 {
        println!("no base-register-local contradictions in the field map");
    }
    // report the 0x118 case explicitly with full context
    for field in &fields {
        if field.old == 0x118 {
            let ctx = covering(&patched_all, field.offset).map(|i| i.text.clone()).unwrap_or_default();
            println!(
                "  0x118 site: field#{} island {} site {:#x} -> {:#x}  base=`{}`  `{}`",
                field.index,
                field.offset,
                field.site,
                field.new,
                field_base.get(&field.index).cloned().unwrap_or_default(),
                ctx
            );
        }
    }

    // ---------------- omitted carriers -----------------------------------
    println!("\n== unpatched structure displacements (omitted-remap candidates) ==");
    // Every January offset that WAS remapped, per base register.
    let mut known_old: BTreeMap<String, BTreeMap<i64, i64>> = BTreeMap::new();
    for (base, map) in &by_base {
        for (old, news) in map {
            if news.len() == 1 {
                known_old
                    .entry(base.clone())
                    .or_default()
                    .insert(*old, *news.iter().next().unwrap());
            }
        }
    }
    // Any offset remapped anywhere, regardless of base (a January offset is a
    // January offset; the base register only tells us which object it rides).
    let any_old: BTreeMap<i64, BTreeSet<i64>> = {
        let mut m: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
        for f in &fields {
            m.entry(f.old).or_default().insert(f.new);
        }
        m
    };
    let patched_byte_ranges: BTreeSet<usize> = fields
        .iter()
        .flat_map(|f| f.offset..f.offset + f.width)
        .collect();

    let mut omitted = Vec::new();
    for insn in &patched_all {
        for (base, disp) in insn.memory_displacements() {
            if base.starts_with("rip") || base.starts_with("rsp") || base.starts_with("rbp") {
                continue;
            }
            if disp <= 0 {
                continue;
            }
            // is any byte of this instruction a declared field patch?
            let touched = (insn.offset..insn.offset + insn.bytes.len()).any(|b| patched_byte_ranges.contains(&b));
            if touched {
                continue;
            }
            // does this displacement equal a January offset that was remapped elsewhere?
            if let Some(news) = any_old.get(&disp) {
                let same_base = known_old.get(&base).and_then(|m| m.get(&disp)).copied();
                omitted.push((insn.offset, base.clone(), disp, news.clone(), same_base, insn.text.clone()));
            }
        }
    }
    if omitted.is_empty() {
        println!("none: every displacement equal to a remapped January offset was itself remapped");
    } else {
        for (offset, base, disp, news, same_base, text) in &omitted {
            let severity = if same_base.is_some() { "DEFECT" } else { "WARN" };
            if same_base.is_some() {
                defects += 1;
            } else {
                warns += 1;
            }
            println!(
                "{severity}\tomitted-remap\tisland {offset}: `{text}` keeps [{base}+{disp:#x}] while the same January offset is remapped to {:?} elsewhere{}",
                news.iter().map(|n| format!("{n:#x}")).collect::<Vec<_>>(),
                match same_base {
                    Some(n) => format!(" (SAME base register `{base}`, expected {n:#x})"),
                    None => String::new(),
                }
            );
        }
    }

    // ---------------- immediate carriers ---------------------------------
    println!("\n== immediate carriers of January structure offsets ==");
    let mut immediates = Vec::new();
    for insn in &patched_all {
        let displacements: BTreeSet<i64> = insn.memory_displacements().into_iter().map(|(_, v)| v).collect();
        for literal in insn.literals() {
            let value = literal as i64;
            if value < 0x40 {
                continue;
            }
            if displacements.contains(&value) {
                continue; // already accounted as a displacement
            }
            if !any_old.contains_key(&value) {
                continue;
            }
            let touched = (insn.offset..insn.offset + insn.bytes.len()).any(|b| patched_byte_ranges.contains(&b));
            let mnemonic = insn.mnemonic();
            // branch targets in a raw-binary disassembly are literals too; skip control flow
            if mnemonic.starts_with('j') || mnemonic == "call" || mnemonic == "loop" {
                continue;
            }
            immediates.push((insn.offset, value, touched, insn.text.clone(), any_old[&value].clone()));
        }
    }
    if immediates.is_empty() {
        println!("none: no instruction carries a remapped January offset as an immediate");
    } else {
        for (offset, value, touched, text, news) in &immediates {
            println!(
                "WARN\timmediate-carrier\tisland {offset}: `{text}` carries {value:#x}, a January offset remapped to {:?} elsewhere{}",
                news.iter().map(|n| format!("{n:#x}")).collect::<Vec<_>>(),
                if *touched { " (instruction also carries a declared patch)" } else { "" }
            );
            warns += 1;
        }
    }

    // ---------------- decode integrity -----------------------------------
    println!("\n== decode integrity of copied regions ==");
    let bad: Vec<&Insn> = patched_all.iter().filter(|i| i.text.starts_with("(bad)")).collect();
    if bad.is_empty() {
        println!("no undecodable bytes in the copied regions ({} instructions)", patched_all.len());
    } else {
        println!("DEFECT\tdecode\t{} undecodable instructions, first at island {}", bad.len(), bad[0].offset);
        defects += 1;
    }

    println!("\nJANASM SUMMARY defects={defects} warnings={warns}");
}
