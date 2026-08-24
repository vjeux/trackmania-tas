// asmdig -- read a stripped C++ binary's call sites out of a flat `objdump -d`
// text, with every pointer argument resolved against the ELF.
//
// Nadeo's engine registers each script-visible class with a run of calls of the
// form `declareClass(id, "CSceneVehicleVisState", size)` followed by one
// `addMember("Name", byteOffset)` per field. Those calls carry the whole struct
// layout, but a stripped objdump shows them as bare addresses. `asmdig calls`
// tracks the argument registers across a function and prints, for every call,
// the C string / f32 / immediate each register holds -- which turns that run
// into a member table.
//
//   asmdig fn     ASM ELF <hexaddr>          the function containing addr
//   asmdig xref   ASM     <hexaddr>          rip-relative and call/jmp refs
//   asmdig calls  ASM ELF <hexaddr>          annotated call trace of a function
//   asmdig consts     ELF <f32>[,<f32>..]    where a float literal lives
//
// Addresses everywhere are objdump/file addresses in hex, no `0x`.
use std::collections::HashMap;

// ---------------------------------------------------------------- ELF access

struct Elf {
    data: Vec<u8>,
    // (vaddr, file offset, size) per section that occupies memory
    secs: Vec<(u64, u64, u64)>,
}

impl Elf {
    fn open(path: &str) -> Elf {
        let data = std::fs::read(path).expect("read elf");
        assert_eq!(&data[0..4], b"\x7fELF", "not an ELF");
        let u16at = |o: usize| u16::from_le_bytes(data[o..o + 2].try_into().unwrap());
        let u64at = |o: usize| u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
        let shoff = u64at(0x28) as usize;
        let shentsize = u16at(0x3a) as usize;
        let shnum = u16at(0x3c) as usize;
        let mut secs = Vec::new();
        for i in 0..shnum {
            let s = shoff + i * shentsize;
            let sh_type = u32::from_le_bytes(data[s + 4..s + 8].try_into().unwrap());
            let addr = u64at(s + 0x10);
            let off = u64at(s + 0x18);
            let size = u64at(s + 0x20);
            if addr != 0 && sh_type != 8 {
                // SHT_NOBITS (.bss) has no file bytes
                secs.push((addr, off, size));
            }
        }
        Elf { data, secs }
    }

    fn at(&self, vaddr: u64) -> Option<&[u8]> {
        for &(a, o, s) in &self.secs {
            if vaddr >= a && vaddr < a + s {
                let start = (o + (vaddr - a)) as usize;
                return Some(&self.data[start..]);
            }
        }
        None
    }

    fn cstr(&self, vaddr: u64) -> Option<String> {
        let b = self.at(vaddr)?;
        let n = b.iter().take(200).position(|&c| c == 0)?;
        let s = &b[..n];
        if s.iter().all(|&c| (0x20..0x7f).contains(&c)) && n > 0 {
            Some(String::from_utf8_lossy(s).into_owned())
        } else {
            None
        }
    }

    fn f32at(&self, vaddr: u64) -> Option<f32> {
        let b = self.at(vaddr)?;
        Some(f32::from_le_bytes(b[..4].try_into().ok()?))
    }
}

// ------------------------------------------------------------- objdump text

#[derive(Clone)]
struct Insn {
    addr: u64,
    mnem: String,
    ops: String,
    /// the `# 30de16 <...>` target objdump prints for rip-relative operands
    riptgt: Option<u64>,
}

fn hex(s: &str) -> Option<u64> {
    u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
}

fn load_asm(path: &str) -> Vec<Insn> {
    let text = std::fs::read_to_string(path).expect("read asm");
    let mut out = Vec::with_capacity(6_000_000);
    for line in text.lines() {
        let Some((addrpart, rest)) = line.split_once(":\t") else {
            continue;
        };
        let Some(addr) = hex(addrpart) else { continue };
        let (body, riptgt) = match rest.split_once("        # ") {
            Some((b, c)) => (b, hex(c.split_whitespace().next().unwrap_or(""))),
            None => (rest, None),
        };
        let body = body.trim_end();
        let (mnem, ops) = match body.split_once(' ') {
            Some((m, o)) => (m, o.trim()),
            None => (body, ""),
        };
        out.push(Insn {
            addr,
            mnem: mnem.to_string(),
            ops: ops.to_string(),
            riptgt,
        });
    }
    out
}

/// The function containing `addr`: back to the previous `int3` padding run,
/// forward to the next one. The binary is stripped, so padding is the boundary.
fn func_range(insns: &[Insn], addr: u64) -> (usize, usize) {
    let i = insns
        .binary_search_by_key(&addr, |x| x.addr)
        .unwrap_or_else(|p| p.saturating_sub(1));
    let mut s = i;
    while s > 0 && insns[s - 1].mnem != "int3" {
        s -= 1;
    }
    let mut e = i;
    while e + 1 < insns.len() && insns[e + 1].mnem != "int3" {
        e += 1;
    }
    (s, e)
}

// -------------------------------------------------- argument-register tracker

#[derive(Clone, Debug)]
enum Val {
    Imm(u64),
    Ptr(u64),
    Unknown,
}

fn reg_slot(r: &str) -> Option<&'static str> {
    Some(match r {
        "rdi" | "edi" | "di" | "dil" => "rdi",
        "rsi" | "esi" | "si" | "sil" => "rsi",
        "rdx" | "edx" | "dx" | "dl" => "rdx",
        "rcx" | "ecx" | "cx" | "cl" => "rcx",
        "r8" | "r8d" | "r8w" | "r8b" => "r8",
        "r9" | "r9d" | "r9w" | "r9b" => "r9",
        "rax" | "eax" => "rax",
        "rbx" | "ebx" => "rbx",
        "r12" | "r12d" => "r12",
        "r13" | "r13d" => "r13",
        "r14" | "r14d" => "r14",
        "r15" | "r15d" => "r15",
        "xmm0" => "xmm0",
        "xmm1" => "xmm1",
        "xmm2" => "xmm2",
        _ => return None,
    })
}

fn show(elf: &Elf, name: &str, v: &Val) -> Option<String> {
    match v {
        Val::Imm(n) => Some(format!("{}=0x{:x}({})", name, n, *n as i64)),
        Val::Ptr(a) => {
            if name.starts_with("xmm") {
                elf.f32at(*a)
                    .map(|f| format!("{}=f32:{} @{:x}", name, f, a))
            } else {
                Some(match elf.cstr(*a) {
                    Some(s) => format!("{}=\"{}\"", name, s),
                    None => format!("{}=&{:x}", name, a),
                })
            }
        }
        Val::Unknown => None,
    }
}

fn trace_calls(insns: &[Insn], elf: &Elf, s: usize, e: usize) {
    let mut regs: HashMap<&'static str, Val> = HashMap::new();
    for ins in &insns[s..=e] {
        let (dst, src) = match ins.ops.split_once(',') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (ins.ops.trim(), ""),
        };
        match ins.mnem.as_str() {
            "lea" => {
                if let (Some(r), Some(t)) = (reg_slot(dst), ins.riptgt) {
                    regs.insert(r, Val::Ptr(t));
                }
            }
            "movss" | "movsd" => {
                if let (Some(r), Some(t)) = (reg_slot(dst), ins.riptgt) {
                    regs.insert(r, Val::Ptr(t));
                }
            }
            "mov" | "movabs" => {
                if let Some(r) = reg_slot(dst) {
                    if let Some(n) = hex(src) {
                        regs.insert(r, Val::Imm(n));
                    } else if let Some(sr) = reg_slot(src) {
                        let v = regs.get(sr).cloned().unwrap_or(Val::Unknown);
                        regs.insert(r, v);
                    } else {
                        regs.insert(r, Val::Unknown);
                    }
                }
            }
            "xor" => {
                if let (Some(r), Some(sr)) = (reg_slot(dst), reg_slot(src)) {
                    if r == sr {
                        regs.insert(r, Val::Imm(0));
                    }
                }
            }
            "xorps" => {
                if let (Some(r), Some(sr)) = (reg_slot(dst), reg_slot(src)) {
                    if r == sr {
                        regs.insert(r, Val::Imm(0));
                    }
                }
            }
            "call" => {
                let tgt = ins.ops.split_whitespace().next().and_then(hex);
                let mut parts = Vec::new();
                for r in ["rdi", "rsi", "rdx", "rcx", "r8", "r9", "xmm0", "xmm1"] {
                    if let Some(v) = regs.get(r) {
                        if let Some(t) = show(elf, r, v) {
                            parts.push(t);
                        }
                    }
                }
                println!(
                    "{:x}  call {:>8}  {}",
                    ins.addr,
                    tgt.map(|t| format!("{:x}", t)).unwrap_or("?".into()),
                    parts.join("  ")
                );
                // the callee clobbers the argument registers
                for r in [
                    "rdi", "rsi", "rdx", "rcx", "r8", "r9", "rax", "xmm0", "xmm1",
                ] {
                    regs.remove(r);
                }
            }
            _ => {
                if let Some(r) = reg_slot(dst) {
                    if !ins.ops.is_empty() {
                        regs.insert(r, Val::Unknown);
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------ commands

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let usage = "asmdig fn|calls ASM ELF ADDR | asmdig xref ASM ADDR | asmdig xrefs ASM ADDR... | asmdig jumptable ELF ADDR COUNT [TARGET] | asmdig consts ELF V,V,..";
    match a.first().map(|s| s.as_str()) {
        Some("fn") => {
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            let (s, e) = func_range(&insns, hex(&a[3]).expect("addr"));
            for ins in &insns[s..=e] {
                // A stripped float-heavy function is unreadable while every
                // coefficient is a rip-relative address; resolve each one to
                // the literal it loads (string first, else the f32).
                let note = match ins.riptgt {
                    Some(t) => match elf.cstr(t) {
                        Some(c) => format!("   \"{}\"", c),
                        None => match elf.f32at(t) {
                            Some(f) => format!("   = {}", f),
                            None => String::new(),
                        },
                    },
                    None => String::new(),
                };
                println!("{:x}\t{} {}{}", ins.addr, ins.mnem, ins.ops, note);
            }
        }
        Some("calls") => {
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            let (s, e) = func_range(&insns, hex(&a[3]).expect("addr"));
            println!("### function {:x}..{:x}", insns[s].addr, insns[e].addr);
            trace_calls(&insns, &elf, s, e);
        }
        Some("jumptable") => {
            // A stripped switch compiles to a table of 32-bit offsets RELATIVE
            // TO THE TABLE'S OWN ADDRESS, indexed by the switch value. Reading
            // it by hand is a pile of shell arithmetic over `od` output, which
            // is exactly the kind of one-liner that gets an index wrong by one
            // and sends someone down the wrong handler for an hour.
            //
            //   asmdig jumptable ELF <table-vaddr> <count> [<target-vaddr>]
            //
            // With a target, it prints only the indices that dispatch there —
            // the "which case reaches this code?" question.
            let elf = Elf::open(&a[1]);
            let base = hex(&a[2]).expect("table vaddr");
            let n: u64 = a[3].parse().expect("count");
            let want = a.get(4).and_then(|s| hex(s));
            for i in 0..n {
                let b = match elf.at(base + 4 * i) {
                    Some(b) if b.len() >= 4 => b,
                    _ => {
                        println!("{:3}  <outside any mapped section>", i);
                        continue;
                    }
                };
                let off = i32::from_le_bytes(b[..4].try_into().unwrap());
                let tgt = (base as i64 + off as i64) as u64;
                match want {
                    Some(w) if w != tgt => continue,
                    _ => println!("{:3}  0x{:x}  ->  {:x}", i, i, tgt),
                }
            }
        }
        Some("xrefs") => {
            // Resolve several xrefs in one pass. A full server disassembly is
            // hundreds of MB; re-parsing it once per validator string hid the
            // call graph behind minutes of avoidable I/O.
            let insns = load_asm(&a[1]);
            let targets: Vec<u64> = a[2..].iter().map(|s| hex(s).expect("addr")).collect();
            for ins in &insns {
                let branch = if ins.mnem == "call" || ins.mnem.starts_with('j') {
                    ins.ops.split_whitespace().next().and_then(hex)
                } else {
                    None
                };
                for &t in &targets {
                    if ins.riptgt == Some(t) || branch == Some(t) {
                        println!("{:x}\t{:x}\t{} {}", t, ins.addr, ins.mnem, ins.ops);
                    }
                }
            }
        }
        Some("xref") => {
            let insns = load_asm(&a[1]);
            let t = hex(&a[2]).expect("addr");
            for ins in &insns {
                let is_ref = ins.riptgt == Some(t)
                    || ((ins.mnem == "call" || ins.mnem.starts_with('j'))
                        && ins.ops.split_whitespace().next().and_then(hex) == Some(t));
                if is_ref {
                    println!("{:x}\t{} {}", ins.addr, ins.mnem, ins.ops);
                }
            }
        }
        Some("consts") => {
            let elf = Elf::open(&a[1]);
            let wanted: Vec<f32> = a[2].split(',').map(|s| s.parse().expect("f32")).collect();
            for &(vaddr, off, size) in &elf.secs {
                let bytes = &elf.data[off as usize..(off + size) as usize];
                for i in (0..bytes.len().saturating_sub(8)).step_by(4) {
                    let f = f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
                    let d = f64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
                    for &w in &wanted {
                        if f != 0.0 && (f - w).abs() <= w.abs() * 1e-6 {
                            println!("{:x}\tf32\t{}\t(want {})", vaddr + i as u64, f, w);
                        }
                        if d != 0.0 && (d - w as f64).abs() <= (w as f64).abs() * 1e-9 {
                            println!("{:x}\tf64\t{}\t(want {})", vaddr + i as u64, d, w);
                        }
                    }
                }
            }
        }
        _ => eprintln!("{}", usage),
    }
}
