// asmdig -- read a stripped C++ binary's call sites out of a flat `objdump -d`
// text, with every pointer argument resolved against the ELF (or PE32+: the
// image base and sections come from the COFF headers, function bounds from
// the `.pdata` exception table).
//
// Nadeo's engine registers each script-visible class with a run of calls of the
// form `declareClass(id, "CSceneVehicleVisState", size)` followed by one
// `addMember("Name", byteOffset)` per field. Those calls carry the whole struct
// layout, but a stripped objdump shows them as bare addresses. `asmdig calls`
// tracks the argument registers across a function and prints, for every call,
// the C string / f32 / immediate each register holds -- which turns that run
// into a member table.
//
//   asmdig fn     ASM ELF <hexaddr>          the function containing addr (.pdata bounds on a PE)
//   asmdig callers ASM ELF <hexaddr>...      call sites of the targets, each with its function
//   asmdig bytes      ELF <hexaddr> <n>       raw data at a VMA (hex, u32s, strings)
//   asmdig xref   ASM     <hexaddr>          rip-relative and call/jmp refs
//   asmdig calls  ASM ELF <hexaddr>          annotated call trace of a function
//   asmdig consts     ELF <f32>[,<f32>..]    where a float literal lives
//   asmdig ptrs       ELF <hexaddr>...       which qwords hold these addresses (vtable slots)
//   asmdig find       ELF <hexbytes>         a byte pattern (`??` wildcard) in every section
//   asmdig argsof ASM ELF <hexaddr>...       resolved arguments at every call site of the targets
//   asmdig cmptree ASM ELF <hexaddr> [reg] [--rust]  a compare-tree id→id function as a table
//   asmdig classtree ASM ELF <reg1> <reg2> [--rust]  the engine's class hierarchy off its registrations
//   asmdig vtables ASM ELF <base-archive> [--reg2 A] [--rust]  every node vtable: class, Archive kind
//
// Addresses everywhere are objdump/file addresses in hex, no `0x`.
use std::collections::HashMap;

// ---------------------------------------------------------------- ELF access

struct Elf {
    data: Vec<u8>,
    // (vaddr, file offset, size) per section that occupies memory
    secs: Vec<(u64, u64, u64)>,
    /// PE only: the `.pdata` RUNTIME_FUNCTION table as (begin, end) VMAs,
    /// sorted by begin. A stripped PE still carries it (x64 SEH needs it), so
    /// function bounds are exact instead of guessed from int3 padding.
    funcs: Vec<(u64, u64)>,
    /// PE only: per `funcs` entry, the begin address of the root function it
    /// is chained to (itself when not chained).
    roots: Vec<u64>,
}

impl Elf {
    fn open(path: &str) -> Elf {
        let data = std::fs::read(path).expect("read elf");
        if &data[0..2] == b"MZ" {
            return Elf::open_pe(data);
        }
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
        Elf { data, secs, funcs: Vec::new(), roots: Vec::new() }
    }

    /// A PE32+ image (Trackmania.exe): sections from the COFF header, the
    /// image base from the optional header, and the exception directory
    /// (`.pdata`) as the function table. Only the file-backed part of a
    /// section is mapped, so BSS reads come back as None like ELF NOBITS.
    fn open_pe(data: Vec<u8>) -> Elf {
        let u16at = |o: usize| u16::from_le_bytes(data[o..o + 2].try_into().unwrap());
        let u32at = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
        let u64at = |o: usize| u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
        let pe = u32at(0x3c) as usize;
        assert_eq!(&data[pe..pe + 4], b"PE\0\0", "not a PE");
        let nsec = u16at(pe + 6) as usize;
        let opt_size = u16at(pe + 20) as usize;
        let opt = pe + 24;
        assert_eq!(u16at(opt), 0x20b, "not PE32+");
        let image_base = u64at(opt + 24);
        // data directory 3 = exception table (RVA, size)
        let exc_rva = u32at(opt + 112 + 3 * 8) as u64;
        let exc_size = u32at(opt + 112 + 3 * 8 + 4) as u64;
        let mut secs = Vec::new();
        let sh = opt + opt_size;
        for i in 0..nsec {
            let s = sh + i * 40;
            let vsize = u32at(s + 8) as u64;
            let rva = u32at(s + 12) as u64;
            let rawsize = u32at(s + 16) as u64;
            let rawoff = u32at(s + 20) as u64;
            if rva != 0 && rawsize != 0 {
                secs.push((image_base + rva, rawoff, rawsize.min(vsize.max(rawsize))));
            }
        }
        let mut e = Elf { data, secs, funcs: Vec::new(), roots: Vec::new() };
        if exc_size > 0 {
            let mut funcs = Vec::with_capacity((exc_size / 12) as usize);
            let mut off = 0;
            while off + 12 <= exc_size {
                let Some(b) = e.at(image_base + exc_rva + off) else { break };
                let begin = u32::from_le_bytes(b[0..4].try_into().unwrap()) as u64;
                let end = u32::from_le_bytes(b[4..8].try_into().unwrap()) as u64;
                let unwind = u32::from_le_bytes(b[8..12].try_into().unwrap()) as u64;
                funcs.push((image_base + begin, image_base + end, image_base + unwind));
                off += 12;
            }
            funcs.sort();
            // Resolve chained unwind info (UNW_FLAG_CHAININFO): a chunk whose
            // UNWIND_INFO ends in the parent's RUNTIME_FUNCTION belongs to the
            // parent function. root = the begin of the outermost parent.
            let mut roots = Vec::with_capacity(funcs.len());
            for &(begin, _, unwind) in &funcs {
                let mut root = begin;
                let mut u = unwind;
                for _ in 0..8 {
                    let Some(b) = e.at(u) else { break };
                    if b.len() < 4 {
                        break;
                    }
                    let flags = b[0] >> 3;
                    if flags & 4 == 0 {
                        break;
                    }
                    let ncodes = b[2] as usize;
                    let off = 4 + ((ncodes + 1) & !1) * 2;
                    if b.len() < off + 12 {
                        break;
                    }
                    root = image_base
                        + u32::from_le_bytes(b[off..off + 4].try_into().unwrap()) as u64;
                    u = image_base
                        + u32::from_le_bytes(b[off + 8..off + 12].try_into().unwrap()) as u64;
                }
                roots.push(root);
            }
            e.funcs = funcs.iter().map(|&(b, en, _)| (b, en)).collect();
            e.roots = roots;
        }
        e
    }

    /// The `.pdata` entry containing `addr`, when the image has a table.
    /// A chunked function (hot/cold split) has one entry per chunk; the
    /// caller gets the chunk, which is what the unwinder means by a function.
    #[allow(dead_code)]
    fn func_bounds(&self, addr: u64) -> Option<(u64, u64)> {
        if self.funcs.is_empty() {
            return None;
        }
        let i = self.funcs.partition_point(|&(b, _)| b <= addr);
        if i == 0 {
            return None;
        }
        let (b, en) = self.funcs[i - 1];
        (addr < en).then_some((b, en))
    }

    /// Every chunk of the function containing `addr` — the chunk itself plus
    /// all chunks chained to the same root — in address order.
    fn func_chunks(&self, addr: u64) -> Vec<(u64, u64)> {
        if self.funcs.is_empty() {
            return Vec::new();
        }
        let i = self.funcs.partition_point(|&(b, _)| b <= addr);
        if i == 0 || addr >= self.funcs[i - 1].1 {
            return Vec::new();
        }
        let root = self.roots[i - 1];
        self.funcs
            .iter()
            .zip(&self.roots)
            .filter(|(_, &r)| r == root)
            .map(|(&f, _)| f)
            .collect()
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
        // `objdump -d` without --no-show-raw-insn puts the raw bytes between
        // the address and the mnemonic, tab-separated: skip that column when
        // it is nothing but hex pairs.
        let rest = match rest.split_once('\t') {
            Some((raw, insn))
                if !raw.is_empty()
                    && raw
                        .split_whitespace()
                        .all(|b| b.len() == 2 && b.chars().all(|c| c.is_ascii_hexdigit())) =>
            {
                insn
            }
            _ => rest,
        };
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

/// The function containing `addr`: the `.pdata` entry when the image has a
/// function table (PE), else back to the previous `int3` padding run and
/// forward to the next one (a stripped ELF: padding is the boundary).
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

#[allow(dead_code)]
fn func_range_in(insns: &[Insn], elf: &Elf, addr: u64) -> (usize, usize) {
    match elf.func_bounds(addr) {
        Some((b, en)) => {
            let s = insns.partition_point(|x| x.addr < b);
            let e = insns.partition_point(|x| x.addr < en);
            (s, e.saturating_sub(1).max(s))
        }
        None => func_range(insns, addr),
    }
}

/// Instruction index ranges of every chunk of the function containing
/// `addr` (one range on ELF, where padding is the only boundary).
fn func_chunks_in(insns: &[Insn], elf: &Elf, addr: u64) -> Vec<(usize, usize)> {
    let chunks = elf.func_chunks(addr);
    if chunks.is_empty() {
        return vec![func_range(insns, addr)];
    }
    chunks
        .iter()
        .map(|&(b, en)| {
            let s = insns.partition_point(|x| x.addr < b);
            let e = insns.partition_point(|x| x.addr < en);
            (s, e.saturating_sub(1).max(s))
        })
        .collect()
}

// -------------------------------------------------- argument-register tracker

#[derive(Clone, Debug)]
enum Val {
    Imm(u64),
    Ptr(u64),
    /// A pointer to a stack slot known to hold this immediate: what the
    /// engine's tiny `GetClassId(&out) { *out = ID; return out; }` helpers
    /// return (the class registrations of ~100 classes fetch both ids that way).
    Boxed(u64),
    Unknown,
}

fn reg_slot(r: &str) -> Option<&'static str> {
    // the x64 stack-argument slots (the 5th argument on) — a class
    // registration passes the parent class id in `[rsp+0x20]`
    if let Some(rest) = r.strip_prefix("DWORD PTR [rsp+0x").or_else(|| r.strip_prefix("QWORD PTR [rsp+0x")) {
        return Some(match rest {
            "20]" => "sp20",
            "28]" => "sp28",
            "30]" => "sp30",
            "38]" => "sp38",
            _ => return None,
        });
    }
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
        Val::Boxed(n) => Some(format!("{}=&0x{:x}", name, n)),
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
    trace_calls_filtered(insns, elf, s, e, &[]);
}

/// `trace_calls`, printing only the calls to `only` (every call when empty).
fn trace_calls_filtered(insns: &[Insn], elf: &Elf, s: usize, e: usize, only: &[u64]) {
    for (tgt, addr, regs) in trace_call_states(insns, s, e, only) {
        let mut parts = Vec::new();
        for r in ["rdi", "rsi", "rdx", "rcx", "r8", "r9", "sp20", "sp28", "sp30", "sp38", "xmm0", "xmm1"] {
            if let Some(v) = regs.get(r) {
                if let Some(t) = show(elf, r, v) {
                    parts.push(t);
                }
            }
        }
        println!("{:x}  call {:>8}  {}", addr, tgt.map(|t| format!("{:x}", t)).unwrap_or("?".into()), parts.join("  "));
    }
}

/// The argument-register state at every call in `insns[s..=e]` to one of
/// `only` (every call when empty), as (target, call address, registers).
fn trace_call_args(insns: &[Insn], _elf: &Elf, s: usize, e: usize, only: &[u64]) -> Vec<(u64, HashMap<&'static str, Val>)> {
    trace_call_states(insns, s, e, only).into_iter().filter_map(|(t, _, r)| t.map(|t| (t, r))).collect()
}

fn trace_call_states(insns: &[Insn], s: usize, e: usize, only: &[u64]) -> Vec<(Option<u64>, u64, HashMap<&'static str, Val>)> {
    let mut out = Vec::new();
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
                    } else if let Some(boxed) = src.strip_prefix("DWORD PTR [").and_then(|s| s.strip_suffix("]")).and_then(reg_slot) {
                        // `mov edx, DWORD PTR [rax]` off a Boxed pointer = the immediate
                        let v = match regs.get(boxed) { Some(Val::Boxed(n)) => Val::Imm(*n), _ => Val::Unknown };
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
                if only.is_empty() || tgt.map(|t| only.contains(&t)).unwrap_or(false) {
                    out.push((tgt, ins.addr, regs.clone()));
                }
                // the callee clobbers the argument registers
                // a class-id getter touches only rcx and rax (LTCG keeps the
                // other argument registers live across it); any other callee
                // clobbers them all
                if let Some(n) = tgt.and_then(|t| id_getter(insns, t)) {
                    regs.insert("rax", Val::Boxed(n));
                    regs.insert("rcx", Val::Unknown);
                } else {
                    for r in ["rdi", "rsi", "rdx", "rcx", "r8", "r9", "rax", "xmm0", "xmm1"] {
                        regs.remove(r);
                    }
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
    out
}

// ------------------------------------------------------------------ commands

fn main() {
    // --version / -V. Compile-time only: CARGO_PKG_* come from the crate's
    // Cargo.toml (which inherits the one workspace version), and TAS_BUILD is
    // the git hash the release build sets. option_env! means an ordinary
    // `cargo build` still works and simply reports "dev". No dependency.
    if std::env::args().any(|x| x == "--version" || x == "-V") {
        println!(
            "{} {} ({})",
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            env!("CARGO_PKG_VERSION"),
            option_env!("TAS_BUILD").unwrap_or("dev")
        );
        std::process::exit(0);
    }
    let a: Vec<String> = std::env::args().skip(1).collect();
    let usage = "asmdig fn|calls ASM ELF ADDR | asmdig near ASM NEEDLE1 NEEDLE2 N [LO HI] | asmdig callers ASM ELF ADDR... | asmdig bytes ELF ADDR N | asmdig xref ASM ADDR | asmdig xrefs ASM ADDR... | asmdig jumptable ELF ADDR COUNT [TARGET] | asmdig ptrs ELF ADDR... | asmdig find ELF HEXBYTES | asmdig argsof ASM ELF ADDR... | asmdig cmptree ASM ELF ADDR [REG] [--rust] | asmdig classtree ASM ELF REG1 REG2 [--rust] | asmdig vtables ASM ELF BASE_ARCHIVE [--reg2 ADDR] [--rust] | asmdig consts ELF V,V,..";
    match a.first().map(|s| s.as_str()) {
        Some("fn") => {
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            let chunks = func_chunks_in(&insns, &elf, hex(&a[3]).expect("addr"));
            for (ci, &(s, e)) in chunks.iter().enumerate() {
                if chunks.len() > 1 {
                    println!("### chunk {} of {}: {:x}..{:x}", ci + 1, chunks.len(), insns[s].addr, insns[e].addr);
                }
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
        }
        Some("calls") => {
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            for (s, e) in func_chunks_in(&insns, &elf, hex(&a[3]).expect("addr")) {
                println!("### function {:x}..{:x}", insns[s].addr, insns[e].addr);
                trace_calls(&insns, &elf, s, e);
            }
        }
        Some("callers") => {
            // Every call/jmp to the targets, each with the bounds of the
            // function it sits in — the "who calls this, and from which
            // function" question in one pass over the text. A chunked
            // function is named by its root chunk.
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            let targets: Vec<u64> = a[3..].iter().map(|s| hex(s).expect("addr")).collect();
            for ins in &insns {
                if !(ins.mnem == "call" || ins.mnem.starts_with('j')) {
                    continue;
                }
                let Some(t) = ins.ops.split_whitespace().next().and_then(hex) else { continue };
                if !targets.contains(&t) {
                    continue;
                }
                let chunks = func_chunks_in(&insns, &elf, ins.addr);
                let (s, _) = chunks[0];
                let (_, e) = chunks[chunks.len() - 1];
                println!(
                    "{:x}\t{:x}\tin {:x}..{:x}\t{}",
                    t, ins.addr, insns[s].addr, insns[e].addr, ins.mnem
                );
            }
        }
        Some("near") => {
            // Windowed co-occurrence: every instruction whose text contains
            // NEEDLE1 and is followed within N instructions by one containing
            // NEEDLE2. Shell loops of sed over a 7M-line dump take minutes
            // per query; this is one pass.
            //   asmdig near ASM NEEDLE1 NEEDLE2 N [LO HI]
            let insns = load_asm(&a[1]);
            let n1 = &a[2];
            let n2 = &a[3];
            let win: usize = a[4].parse().expect("window");
            let lo = a.get(5).and_then(|s| hex(s)).unwrap_or(0);
            let hi = a.get(6).and_then(|s| hex(s)).unwrap_or(u64::MAX);
            let text = |i: &Insn| format!("{} {}", i.mnem, i.ops);
            for (i, ins) in insns.iter().enumerate() {
                if ins.addr < lo || ins.addr > hi {
                    continue;
                }
                if !text(ins).contains(n1.as_str()) {
                    continue;
                }
                let end = (i + win + 1).min(insns.len());
                if let Some(j) = insns[i + 1..end].iter().find(|x| text(x).contains(n2.as_str())) {
                    println!("{:x}\t{}\t->\t{:x}\t{}", ins.addr, text(ins), j.addr, text(j));
                }
            }
        }
        Some("bytes") => {
            // Raw data at a VMA: hex, then the u32 and u64 readings per row,
            // with any pointer-looking qword resolved to a C string.
            let elf = Elf::open(&a[1]);
            let base = hex(&a[2]).expect("addr");
            let n: usize = a[3].parse().expect("count");
            let Some(b) = elf.at(base) else {
                println!("{:x}: not file-backed (BSS?)", base);
                return;
            };
            let b = &b[..n.min(b.len())];
            for (i, row) in b.chunks(16).enumerate() {
                let hexs: Vec<String> = row.iter().map(|x| format!("{:02x}", x)).collect();
                let mut words = Vec::new();
                for w in row.chunks(4) {
                    if w.len() == 4 {
                        words.push(format!("{:08x}", u32::from_le_bytes(w.try_into().unwrap())));
                    }
                }
                let mut notes = Vec::new();
                for q in row.chunks(8) {
                    if q.len() == 8 {
                        let v = u64::from_le_bytes(q.try_into().unwrap());
                        if let Some(s) = elf.cstr(v) {
                            notes.push(format!("\"{}\"", s));
                        }
                    }
                }
                println!(
                    "{:x}  {:<48} {:<36} {}",
                    base + (i * 16) as u64,
                    hexs.join(" "),
                    words.join(" "),
                    notes.join(" ")
                );
            }
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
        Some("ptrs") => {
            // Where 8-byte little-endian pointers to the given VMAs live in
            // the image's file-backed sections — a virtual method's vtable
            // slots (a stripped C++ binary calls it only through `call
            // [rax+0x10]`, so `callers` sees nothing), a function pointer
            // table, a static initialiser list. The row prints the holding
            // VMA and, when the neighbouring qwords are code pointers too,
            // the start of that run (the vtable's first slot) and the slot
            // index — which is what a `call [reg+off]` site needs.
            //   asmdig ptrs ELF ADDR...
            let elf = Elf::open(&a[1]);
            let targets: Vec<u64> = a[2..].iter().map(|s| hex(s).expect("addr")).collect();
            let text = elf.secs.first().map(|&(v, _, s)| (v, v + s)).unwrap_or((0, 0));
            let is_code = |v: u64| v >= text.0 && v < text.1;
            for &(vaddr, off, size) in &elf.secs {
                let bytes = &elf.data[off as usize..(off + size) as usize];
                for i in (0..bytes.len().saturating_sub(8)).step_by(8) {
                    let v = u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
                    if !targets.contains(&v) {
                        continue;
                    }
                    // walk back over code pointers to the run's first slot
                    let mut j = i;
                    while j >= 8 {
                        let p = u64::from_le_bytes(bytes[j - 8..j].try_into().unwrap());
                        if !is_code(p) {
                            break;
                        }
                        j -= 8;
                    }
                    let mut k = i;
                    while k + 16 <= bytes.len() {
                        let p = u64::from_le_bytes(bytes[k + 8..k + 16].try_into().unwrap());
                        if !is_code(p) {
                            break;
                        }
                        k += 8;
                    }
                    println!(
                        "{:x}\tat {:x}\trun {:x}..{:x} ({} slots)\tslot {} (+0x{:x})",
                        v,
                        vaddr + i as u64,
                        vaddr + j as u64,
                        vaddr + k as u64 + 8,
                        (k - j) / 8 + 1,
                        (i - j) / 8,
                        i - j
                    );
                }
            }
        }
        Some("find") => {
            // Every VMA where a byte pattern (hex, spaces optional, `??` a
            // wildcard byte) occurs in a file-backed section — a table
            // located by its first bytes (the LZ4 preset dictionary's `DDS |`
            // head, a Blowfish P-array's first π words), a magic string.
            //   asmdig find ELF HEXBYTES
            let elf = Elf::open(&a[1]);
            let pat: Vec<Option<u8>> = a[2]
                .split_whitespace()
                .flat_map(|w| {
                    let w = w.to_string();
                    (0..w.len() / 2).map(move |i| {
                        let p = &w[2 * i..2 * i + 2];
                        if p == "??" { None } else { Some(u8::from_str_radix(p, 16).expect("hex byte")) }
                    })
                })
                .collect();
            for &(vaddr, off, size) in &elf.secs {
                let bytes = &elf.data[off as usize..(off + size) as usize];
                let mut i = 0usize;
                while i + pat.len() <= bytes.len() {
                    if pat.iter().zip(&bytes[i..]).all(|(p, b)| p.map_or(true, |p| p == *b)) {
                        println!("{:x}\t(file {:x})", vaddr + i as u64, off as usize + i);
                    }
                    i += 1;
                }
            }
        }
        Some("argsof") => {
            // The resolved argument registers at EVERY call site of the
            // targets, in one pass: the class-registration call
            // (`Register(&info, classId, &parentInfo, "Name")`, ~3000 sites)
            // read as a table instead of one `calls` run per caller — each
            // run re-parses a 400 MB listing.
            //   asmdig argsof ASM ELF ADDR...
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            let targets: Vec<u64> = a[3..].iter().map(|s| hex(s).expect("addr")).collect();
            let mut done_funcs: Vec<(usize, usize)> = Vec::new();
            for (i, ins) in insns.iter().enumerate() {
                if ins.mnem != "call" {
                    continue;
                }
                let Some(t) = ins.ops.split_whitespace().next().and_then(hex) else { continue };
                if !targets.contains(&t) {
                    continue;
                }
                let chunks = func_chunks_in(&insns, &elf, ins.addr);
                let Some(&(s, e)) = chunks.iter().find(|&&(s, e)| s <= i && i <= e) else { continue };
                if done_funcs.contains(&(s, e)) {
                    continue;
                }
                done_funcs.push((s, e));
                trace_calls_filtered(&insns, &elf, s, e, &targets);
            }
        }
        Some("cmptree") => {
            // Evaluate a pure compare-tree function of one 32-bit register
            // (`cmp edx,IMM` / conditional jumps / `mov eax,IMM` /
            // `mov [rcx],eax|edx` / ret — the shape MSVC gives a big
            // `switch` that maps ids to ids) at every immediate it compares
            // against, and print the input → output table. The engine's
            // class-id remap (0x1402f3570: 161 CGame ids → their 0x24xxxxxx
            // archive ids) read as a table instead of 1000 lines of asm.
            //   asmdig cmptree ASM ELF ADDR [REG] [--rust]
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            let start = hex(&a[3]).expect("addr");
            let rust = a.iter().any(|s| s == "--rust");
            let reg = a.get(4).filter(|s| s.as_str() != "--rust").map(|s| s.as_str()).unwrap_or("edx");
            let chunks = func_chunks_in(&insns, &elf, start);
            let (s, e) = (chunks[0].0, chunks[chunks.len() - 1].1);
            let body = &insns[s..=e];
            let idx_of = |addr: u64| body.binary_search_by_key(&addr, |i| i.addr).ok();
            let mut inputs: Vec<u64> = body
                .iter()
                .filter(|i| i.mnem == "cmp" && i.ops.starts_with(reg))
                .filter_map(|i| i.ops.split_once(',').and_then(|(_, v)| hex(v.trim())))
                .collect();
            inputs.sort();
            inputs.dedup();
            for &input in &inputs {
                let mut pc = idx_of(start).expect("start in function");
                let (mut eax, mut out): (Option<u64>, Option<u64>) = (None, None);
                let mut flags: Option<(u64, u64)> = None; // (lhs, rhs) of the last cmp
                let mut steps = 0;
                let result = loop {
                    steps += 1;
                    if steps > 10_000 || pc >= body.len() {
                        break Err("runaway");
                    }
                    let ins = &body[pc];
                    let (dst, src) = ins.ops.split_once(',').map(|(d, s)| (d.trim(), s.trim())).unwrap_or((ins.ops.trim(), ""));
                    let tgt = || ins.ops.split_whitespace().next().and_then(hex).and_then(idx_of);
                    match ins.mnem.as_str() {
                        "cmp" if dst == reg => {
                            flags = Some((input & 0xffff_ffff, hex(src).unwrap_or(0)));
                            pc += 1;
                        }
                        "jmp" => match tgt() { Some(t) => pc = t, None => break Err("jmp out") },
                        "ja" | "je" | "jne" | "jb" | "jbe" | "jae" => {
                            let Some((l, r)) = flags else { break Err("jump without cmp") };
                            let take = match ins.mnem.as_str() {
                                "ja" => l > r,
                                "jae" => l >= r,
                                "je" => l == r,
                                "jne" => l != r,
                                "jb" => l < r,
                                _ => l <= r,
                            };
                            if take {
                                match tgt() { Some(t) => pc = t, None => break Err("jcc out") }
                            } else {
                                pc += 1;
                            }
                        }
                        "mov" if dst == "eax" => {
                            eax = if src == reg { Some(input) } else { hex(src) };
                            pc += 1;
                        }
                        "mov" if dst == "DWORD PTR [rcx]" => {
                            out = if src == "eax" { eax } else if src == reg { Some(input) } else { hex(src) };
                            pc += 1;
                        }
                        "mov" | "sub" | "add" | "lea" | "nop" => pc += 1,
                        "ret" => break Ok(out),
                        _ => break Err("unsupported insn"),
                    }
                };
                match result {
                    Ok(Some(o)) if o != input && rust => println!("    (0x{:08X}, 0x{:08X}),", input, o),
                    Ok(Some(o)) if o != input => println!("{:08x}\t{:08x}", input, o),
                    Ok(Some(_)) if rust => {}
                    Ok(Some(_)) => println!("{:08x}\t=", input),
                    Ok(None) => println!("{:08x}	(no store)", input),
                    Err(e) => println!("{:08x}	! {}", input, e),
                }
            }
        }
        Some("classtree") => {
            // The engine's class hierarchy, read off the class registrations
            // (Trackmania.exe: 0x1402d52e0 `Register(&info, classId,
            // &parentInfo, "Name", …)` — 1631 sites — and 0x1402ea9e0
            // `Register2(classId, "Name", size, isNod, parentClassId, …)`).
            // Prints `classId TAB name TAB parentClassId TAB parentName` per
            // class, sorted; `--rust` prints it as a Rust slice literal
            // (mapgeom's `engine_classes.rs`). The pak cipher's dummy write
            // folds the PARENT class id of every node, so this table is what
            // the pak reader needs, straight from the binary that wrote the
            // files rather than from a third party's chunk grammar.
            //   asmdig classtree ASM ELF REG1 REG2 [--rust]
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            let reg1 = hex(&a[3]).expect("reg1");
            let reg2 = hex(&a[4]).expect("reg2");
            let rust = a.get(5).map(|s| s == "--rust").unwrap_or(false);
            // (info address, class id, name, parent info address) / (class id, name, parent id)
            let mut by_info: HashMap<u64, (u64, String, u64)> = HashMap::new();
            let mut direct: Vec<(u64, String, u64)> = Vec::new();
            let mut done_funcs: Vec<(usize, usize)> = Vec::new();
            for (i, ins) in insns.iter().enumerate() {
                if ins.mnem != "call" {
                    continue;
                }
                let Some(t) = ins.ops.split_whitespace().next().and_then(hex) else { continue };
                if t != reg1 && t != reg2 {
                    continue;
                }
                let chunks = func_chunks_in(&insns, &elf, ins.addr);
                let Some(&(s, e)) = chunks.iter().find(|&&(s, e)| s <= i && i <= e) else { continue };
                if done_funcs.contains(&(s, e)) {
                    continue;
                }
                done_funcs.push((s, e));
                for (tgt, regs) in trace_call_args(&insns, &elf, s, e, &[reg1, reg2]) {
                    let imm = |r: &str| match regs.get(r) { Some(Val::Imm(n)) => Some(*n), _ => None };
                    let ptr = |r: &str| match regs.get(r) { Some(Val::Ptr(p)) => Some(*p), _ => None };
                    let name = |r: &str| ptr(r).and_then(|p| elf.cstr(p));
                    if tgt == reg1 {
                        if let (Some(info), Some(id), Some(n)) = (ptr("rcx"), imm("rdx"), name("r9")) {
                            let parent = ptr("r8").unwrap_or(0);
                            by_info.insert(info, (id, n, parent));
                        }
                    } else if let (Some(id), Some(n)) = (imm("rcx"), name("rdx")) {
                        // r9d = 1 marks a CMwNod-derived class; structs pass 0
                        if imm("r9") == Some(1) || imm("sp20").map(|p| p != 0).unwrap_or(false) {
                            direct.push((id, n, imm("sp20").unwrap_or(0)));
                        }
                    }
                }
            }
            let mut rows: Vec<(u64, String, u64, String)> = Vec::new();
            for (_, (id, n, parent)) in &by_info {
                let (pid, pname) = match by_info.get(parent) {
                    Some((pid, pn, _)) => (*pid, pn.clone()),
                    None => (0, String::new()),
                };
                rows.push((*id, n.clone(), pid, pname));
            }
            let name_of: HashMap<u64, String> = rows.iter().map(|(id, n, _, _)| (*id, n.clone())).collect();
            for (id, n, pid) in &direct {
                rows.push((*id, n.clone(), *pid, name_of.get(pid).cloned().unwrap_or_default()));
            }
            rows.sort();
            rows.dedup_by_key(|r| r.0);
            for (id, n, pid, pn) in &rows {
                if rust {
                    println!("    (0x{:08X}, 0x{:08X}), // {} : {}", id, pid, n, if pn.is_empty() { "-" } else { pn });
                } else {
                    println!("{:08X}	{}	{:08X}	{}", id, n, pid, pn);
                }
            }
        }
        Some("vtables") => {
            // Every CMwNod-class vtable in the image, found by its shape: slot
            // 3 is `GetClassId(&out) { *out = ID; }` (`mov DWORD PTR [rdx],ID`)
            // and slot 4 `IsClassId(id)` compares against the same ID. Prints
            // `classId TAB vtable TAB slot14 TAB kind` where slot 14 is the
            // node's virtual `Archive(CClassicArchive&)` and kind says whether
            // it is CMwNod::Archive itself (`base`), an override that calls
            // it (`chains`), or one that never does (`custom`) — the pak
            // cipher's dummy write lives in CMwNod::Archive, so a `custom`
            // class (a plain-struct body: CPlugVegetTreeModel,
            // CPlugDynaObjectModel, CPlugPrefab, …) folds nothing.
            //   asmdig vtables ASM ELF BASE_ARCHIVE [--reg2 ADDR] [--rust]
            let insns = load_asm(&a[1]);
            let elf = Elf::open(&a[2]);
            let base = hex(&a[3]).expect("CMwNod::Archive address");
            let rust = a.iter().any(|s| s == "--rust");
            let text = elf.secs.first().map(|&(v, _, s)| (v, v + s)).unwrap_or((0, 0));
            let is_code = |v: u64| v >= text.0 && v < text.1;
            let insn_at = |addr: u64| insns.binary_search_by_key(&addr, |x| x.addr).ok().map(|i| &insns[i]);
            // does the function at `f` reach `base` (a call or a tail jmp, in any of its chunks)?
            let reaches = |f: u64| -> bool {
                for (s, e) in func_chunks_in(&insns, &elf, f) {
                    for ins in &insns[s..=e] {
                        if (ins.mnem == "call" || ins.mnem == "jmp") && ins.ops.split_whitespace().next().and_then(hex) == Some(base) {
                            return true;
                        }
                    }
                }
                false
            };
            let mut rows: Vec<(u64, u64, u64, &str)> = Vec::new();
            for &(vaddr, off, size) in &elf.secs {
                if is_code(vaddr) {
                    continue;
                }
                let bytes = &elf.data[off as usize..(off + size) as usize];
                let q = |i: usize| u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
                let mut i = 0usize;
                while i + 15 * 8 <= bytes.len() {
                    let s3 = q(i + 3 * 8);
                    let s4 = q(i + 4 * 8);
                    if is_code(s3) && is_code(s4) {
                        let id3 = insn_at(s3).filter(|x| x.mnem == "mov" && x.ops.starts_with("DWORD PTR [rdx],")).and_then(|x| hex(x.ops.split_once(',').unwrap().1.trim()));
                        // IsClassId: `cmp edx,ID` first, or after a `xor eax,eax` (CMwNod itself)
                        let id4 = insns.binary_search_by_key(&s4, |x| x.addr).ok().and_then(|i| {
                            insns[i..(i + 2).min(insns.len())].iter().find(|x| x.mnem == "cmp" && x.ops.starts_with("edx,")).and_then(|x| hex(x.ops.split_once(',').unwrap().1.trim()))
                        });
                        if let (Some(id), Some(id_b)) = (id3, id4) {
                            if id == id_b && id & 0xFFF == 0 {
                                let s14 = q(i + 14 * 8);
                                let kind = if s14 == base { "base" } else if is_code(s14) && reaches(s14) { "chains" } else { "custom" };
                                rows.push((id, vaddr + i as u64, s14, kind));
                            }
                        }
                    }
                    i += 8;
                }
            }
            // Second anchor, for the classes registered through `Register2`
            // (0x1402ea9e0), whose vtables carry no GetClassId/IsClassId pair:
            // slot 2 (GetClassInfo) is `mov rax,[rip+INFO]; ret`, and the
            // class's registration function ends with `call …; mov [rip+INFO],rax`
            // right after its `Register2(classId, …)` call — so INFO names the
            // registering function, whose Register2 arguments name the class.
            if let Some(reg2) = a.iter().position(|s| s == "--reg2").and_then(|i| a.get(i + 1)).and_then(|s| hex(s)) {
                // INFO pointer → the function storing it
                let mut store_fn: HashMap<u64, (usize, usize)> = HashMap::new();
                for (i, ins) in insns.iter().enumerate() {
                    if ins.mnem == "mov" && ins.ops.starts_with("QWORD PTR [rip+") && ins.ops.ends_with(",rax") {
                        if let Some(t) = ins.riptgt {
                            if let Some(&(s, e)) = func_chunks_in(&insns, &elf, ins.addr).iter().find(|&&(s, e)| s <= i && i <= e) {
                                store_fn.insert(t, (s, e));
                            }
                        }
                    }
                }
                let known: std::collections::HashSet<u64> = rows.iter().map(|r| r.1).collect();
                let mut extra: Vec<(u64, u64, u64, &str)> = Vec::new();
                for &(vaddr, off, size) in &elf.secs {
                    if is_code(vaddr) {
                        continue;
                    }
                    let bytes = &elf.data[off as usize..(off + size) as usize];
                    let q = |i: usize| u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
                    let mut i = 0usize;
                    while i + 15 * 8 <= bytes.len() {
                        let vt = vaddr + i as u64;
                        let s2 = q(i + 2 * 8);
                        if !known.contains(&vt) && is_code(s2) && is_code(q(i)) && is_code(q(i + 8)) {
                            if let Some(x) = insn_at(s2).filter(|x| x.mnem == "mov" && x.ops.starts_with("rax,QWORD PTR [rip+")).and_then(|x| x.riptgt) {
                                let next_is_ret = insns.binary_search_by_key(&s2, |x| x.addr).ok().and_then(|k| insns.get(k + 1)).map(|x| x.mnem == "ret").unwrap_or(false);
                                if next_is_ret {
                                    if let Some(&(s, e)) = store_fn.get(&x) {
                                        for (tgt, regs) in trace_call_args(&insns, &elf, s, e, &[reg2]) {
                                            if tgt != reg2 {
                                                continue;
                                            }
                                            if let Some(Val::Imm(id)) = regs.get("rcx") {
                                                let s14 = q(i + 14 * 8);
                                                let kind = if s14 == base { "base" } else if is_code(s14) && reaches(s14) { "chains" } else { "custom" };
                                                extra.push((*id, vt, s14, kind));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        i += 8;
                    }
                }
                rows.extend(extra);
                rows.sort();
                rows.dedup();
            }
            rows.sort();
            for (id, vt, s14, kind) in &rows {
                if rust {
                    if *kind == "custom" {
                        println!("    0x{:08X}, // vtable {:x}, Archive {:x}", id, vt, s14);
                    }
                } else {
                    println!("{:08X}	{:x}	{:x}	{}", id, vt, s14, kind);
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

/// The immediate a 3-instruction `mov DWORD PTR [rcx],IMM; mov rax,rcx; ret`
/// helper stores — the engine's per-class `GetClassId(&out)` getters.
fn id_getter(insns: &[Insn], addr: u64) -> Option<u64> {
    let i = insns.binary_search_by_key(&addr, |x| x.addr).ok()?;
    let a = insns.get(i)?;
    let b = insns.get(i + 1)?;
    let c = insns.get(i + 2)?;
    if a.mnem == "mov" && a.ops.starts_with("DWORD PTR [rcx],") && b.mnem == "mov" && b.ops == "rax,rcx" && c.mnem == "ret" {
        return hex(a.ops.split_once(',')?.1.trim());
    }
    None
}
