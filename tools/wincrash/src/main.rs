// wincrash -- read a Windows minidump and a PE image without leaving Rust.
//
// The render box is the only machine that has the game, and when the client
// dies importing a ghost the only witnesses are a WER event, a .dmp and the
// .exe itself. This reads all three well enough to name the faulting
// instruction and the pointer it dereferenced.
//
//   wincrash dmp info   <dmp>                     exception, registers, modules
//   wincrash dmp mem    <dmp> <addr> <len> [out]  memory out of the dump
//   wincrash dmp stack  <dmp> <n>                 top of the faulting stack, as
//                                                 candidate return addresses
//   wincrash pe  info   <exe>                     image base and sections
//   wincrash pe  bytes  <exe> <rva> <len> [out]   bytes at an RVA
//
// Addresses are hex, with or without `0x`. With `out` the bytes are written
// raw to that path (feed to `objdump -b binary --adjust-vma=<va>`); without
// it, a hexdump goes to stdout.

use std::fs;

fn u16at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(d[o..o + 2].try_into().unwrap())
}
fn u32at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn u64at(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}
fn hexarg(s: &str) -> u64 {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(s, 16).unwrap_or_else(|_| panic!("not hex: {s}"))
}

fn hexdump(base: u64, b: &[u8]) {
    for (i, chunk) in b.chunks(16).enumerate() {
        let mut line = format!("{:016x}  ", base + (i * 16) as u64);
        for (j, x) in chunk.iter().enumerate() {
            line.push_str(&format!("{x:02x} "));
            if j == 7 {
                line.push(' ');
            }
        }
        println!("{line}");
    }
}

// ------------------------------------------------------------------ minidump

struct Dump {
    d: Vec<u8>,
    dirs: Vec<(u32, u32, u32)>, // (type, size, rva)
}

impl Dump {
    fn open(p: &str) -> Dump {
        let d = fs::read(p).expect("read dmp");
        assert_eq!(&d[0..4], b"MDMP", "not a minidump");
        let n = u32at(&d, 8) as usize;
        let dir = u32at(&d, 12) as usize;
        let mut dirs = Vec::new();
        for i in 0..n {
            let o = dir + i * 12;
            dirs.push((u32at(&d, o), u32at(&d, o + 4), u32at(&d, o + 8)));
        }
        Dump { d, dirs }
    }
    fn stream(&self, ty: u32) -> Option<(usize, usize)> {
        self.dirs
            .iter()
            .find(|e| e.0 == ty)
            .map(|e| (e.2 as usize, e.1 as usize))
    }
    /// every (start, len, file-offset) memory range in the dump
    fn ranges(&self) -> Vec<(u64, u64, usize)> {
        let mut v = Vec::new();
        if let Some((rva, _)) = self.stream(9) {
            let n = u64at(&self.d, rva) as usize;
            let base = u64at(&self.d, rva + 8) as usize;
            let mut off = base;
            for i in 0..n {
                let o = rva + 16 + i * 16;
                let start = u64at(&self.d, o);
                let size = u64at(&self.d, o + 8);
                v.push((start, size, off));
                off += size as usize;
            }
        }
        if let Some((rva, _)) = self.stream(5) {
            let n = u32at(&self.d, rva) as usize;
            for i in 0..n {
                let o = rva + 4 + i * 16;
                let start = u64at(&self.d, o);
                let size = u32at(&self.d, o + 8) as u64;
                let fo = u32at(&self.d, o + 12) as usize;
                v.push((start, size, fo));
            }
        }
        v
    }
    fn read(&self, addr: u64, len: usize) -> Option<Vec<u8>> {
        for (start, size, fo) in self.ranges() {
            if addr >= start && addr + len as u64 <= start + size {
                let o = fo + (addr - start) as usize;
                return Some(self.d[o..o + len].to_vec());
            }
        }
        None
    }
    fn modules(&self) -> Vec<(u64, u32, String)> {
        let mut v = Vec::new();
        let Some((rva, _)) = self.stream(4) else {
            return v;
        };
        let n = u32at(&self.d, rva) as usize;
        for i in 0..n {
            let o = rva + 4 + i * 108;
            let base = u64at(&self.d, o);
            let size = u32at(&self.d, o + 8);
            let nrva = u32at(&self.d, o + 20) as usize;
            let nlen = u32at(&self.d, nrva) as usize;
            let mut s = String::new();
            for k in 0..nlen / 2 {
                let c = u16at(&self.d, nrva + 4 + k * 2);
                s.push(char::from_u32(c as u32).unwrap_or('?'));
            }
            v.push((base, size, s));
        }
        v
    }
    fn module_of(&self, addr: u64) -> Option<(u64, String)> {
        self.modules()
            .into_iter()
            .find(|(b, s, _)| addr >= *b && addr < b + *s as u64)
            .map(|(b, _, n)| (b, n))
    }
    /// (thread id, context file offset)
    fn exception(&self) -> Option<(u32, u32, u64, Vec<u64>, usize)> {
        let (rva, _) = self.stream(6)?;
        let tid = u32at(&self.d, rva);
        let er = rva + 8;
        let code = u32at(&self.d, er);
        let addr = u64at(&self.d, er + 16);
        let nparam = u32at(&self.d, er + 24) as usize;
        let mut params = Vec::new();
        for i in 0..nparam.min(15) {
            params.push(u64at(&self.d, er + 32 + i * 8));
        }
        let ctx_rva = u32at(&self.d, er + 32 + 15 * 8 + 4) as usize;
        Some((tid, code, addr, params, ctx_rva))
    }
}

const REGS: [(&str, usize); 17] = [
    ("rax", 0x78),
    ("rcx", 0x80),
    ("rdx", 0x88),
    ("rbx", 0x90),
    ("rsp", 0x98),
    ("rbp", 0xa0),
    ("rsi", 0xa8),
    ("rdi", 0xb0),
    ("r8", 0xb8),
    ("r9", 0xc0),
    ("r10", 0xc8),
    ("r11", 0xd0),
    ("r12", 0xd8),
    ("r13", 0xe0),
    ("r14", 0xe8),
    ("r15", 0xf0),
    ("rip", 0xf8),
];

fn dmp_info(path: &str) {
    let dp = Dump::open(path);
    println!("streams: {}", dp.dirs.len());
    let Some((tid, code, addr, params, ctx)) = dp.exception() else {
        println!("no exception stream");
        return;
    };
    println!("thread   0x{tid:x}");
    println!("code     0x{code:08x}");
    println!("address  0x{addr:016x}");
    if let Some((b, n)) = dp.module_of(addr) {
        println!("         in {n} +0x{:x}", addr - b);
    }
    for (i, p) in params.iter().enumerate() {
        let what = match i {
            0 => match p {
                0 => " (read)",
                1 => " (write)",
                8 => " (execute)",
                _ => "",
            },
            1 => " (faulting data address)",
            _ => "",
        };
        println!("param{i}   0x{p:016x}{what}");
    }
    println!("--- context ---");
    for (n, o) in REGS {
        let v = u64at(&dp.d, ctx + o);
        let mut line = format!("{n:<4} 0x{v:016x}", n = n, v = v);
        if let Some((b, m)) = dp.module_of(v) {
            line.push_str(&format!("   {m} +0x{:x}", v - b));
        } else if dp.read(v, 8).is_some() {
            let deref = u64at(&dp.read(v, 8).unwrap(), 0);
            line.push_str(&format!("   -> 0x{deref:016x}"));
        } else if v != 0 {
            line.push_str("   (not mapped in dump)");
        }
        println!("{line}");
    }
    let xmm0 = ctx + 0x1a0;
    for i in 0..4 {
        let lo = u64at(&dp.d, xmm0 + i * 16);
        let hi = u64at(&dp.d, xmm0 + i * 16 + 8);
        let f: Vec<String> = (0..4)
            .map(|k| {
                let bits = u32at(&dp.d, xmm0 + i * 16 + k * 4);
                format!("{:.6}", f32::from_bits(bits))
            })
            .collect();
        println!("xmm{i}  0x{hi:016x}{lo:016x}  f32[{}]", f.join(", "));
    }
    println!("--- modules ---");
    for (b, s, n) in dp.modules() {
        let short = n.rsplit('\\').next().unwrap_or(&n).to_string();
        println!("0x{b:016x} +0x{s:08x}  {short}");
    }
    let rip = u64at(&dp.d, ctx + 0xf8);
    if let Some(code) = dp.read(rip.saturating_sub(32), 96) {
        println!("--- code around rip (rip-32) ---");
        hexdump(rip - 32, &code);
    }
}

fn dmp_mem(path: &str, addr: u64, len: usize, out: Option<&str>) {
    let dp = Dump::open(path);
    match dp.read(addr, len) {
        Some(b) => match out {
            Some(p) => {
                fs::write(p, &b).unwrap();
                println!("wrote {} bytes to {p}", b.len());
            }
            None => hexdump(addr, &b),
        },
        None => {
            println!("0x{addr:x}+{len} not in the dump; ranges near it:");
            let mut r = dp.ranges();
            r.sort();
            for (s, l, _) in r {
                if s + l > addr.saturating_sub(0x100000) && s < addr + 0x100000 {
                    println!("  0x{s:016x} .. 0x{:016x}", s + l);
                }
            }
        }
    }
}

fn dmp_stack(path: &str, n: usize) {
    let dp = Dump::open(path);
    let Some((_, _, _, _, ctx)) = dp.exception() else {
        return;
    };
    let rsp = u64at(&dp.d, ctx + 0x98);
    println!("rsp 0x{rsp:016x}");
    for i in 0..n {
        let a = rsp + (i * 8) as u64;
        let Some(b) = dp.read(a, 8) else { continue };
        let v = u64at(&b, 0);
        if let Some((base, m)) = dp.module_of(v) {
            println!("  +0x{:<4x} 0x{v:016x}  {m} +0x{:x}", i * 8, v - base);
        }
    }
}

// ------------------------------------------------------------------------ PE

struct Pe {
    d: Vec<u8>,
    base: u64,
    secs: Vec<(String, u32, u32, u32, u32)>, // name, va, vsize, raw ptr, raw size
}

impl Pe {
    fn open(p: &str) -> Pe {
        let d = fs::read(p).expect("read exe");
        let pe = u32at(&d, 0x3c) as usize;
        assert_eq!(&d[pe..pe + 4], b"PE\0\0", "not a PE");
        let nsec = u16at(&d, pe + 6) as usize;
        let optsz = u16at(&d, pe + 20) as usize;
        let opt = pe + 24;
        let magic = u16at(&d, opt);
        let base = if magic == 0x20b {
            u64at(&d, opt + 24)
        } else {
            u32at(&d, opt + 28) as u64
        };
        let mut secs = Vec::new();
        for i in 0..nsec {
            let o = opt + optsz + i * 40;
            let name = String::from_utf8_lossy(&d[o..o + 8])
                .trim_end_matches('\0')
                .to_string();
            secs.push((
                name,
                u32at(&d, o + 12),
                u32at(&d, o + 8),
                u32at(&d, o + 20),
                u32at(&d, o + 16),
            ));
        }
        Pe { d, base, secs }
    }
    fn off(&self, rva: u32) -> Option<usize> {
        for (_, va, vsz, ptr, rsz) in &self.secs {
            if rva >= *va && rva < va + vsz.max(rsz) {
                let o = (rva - va) as usize + *ptr as usize;
                if o < self.d.len() {
                    return Some(o);
                }
            }
        }
        None
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let usage = "wincrash dmp info|mem|stack ... | wincrash pe info|bytes ...";
    match (a.get(1).map(|s| s.as_str()), a.get(2).map(|s| s.as_str())) {
        (Some("dmp"), Some("info")) => dmp_info(&a[3]),
        (Some("dmp"), Some("mem")) => dmp_mem(
            &a[3],
            hexarg(&a[4]),
            hexarg(&a[5]) as usize,
            a.get(6).map(|s| s.as_str()),
        ),
        (Some("dmp"), Some("ranges")) => {
            let dp = Dump::open(&a[3]);
            let mut r = dp.ranges();
            r.sort();
            let total: u64 = r.iter().map(|x| x.1).sum();
            println!("{} range(s), {} bytes", r.len(), total);
            for (s, l, _) in r.iter().take(a.get(4).map_or(40, |s| s.parse().unwrap())) {
                println!("  0x{s:016x} .. 0x{:016x}  {l} B", s + l);
            }
            println!("streams present: {:?}", dp.dirs.iter().map(|d| d.0).collect::<Vec<_>>());
        }
        (Some("dmp"), Some("stack")) => dmp_stack(&a[3], a.get(4).map_or(64, |s| s.parse().unwrap())),
        (Some("pe"), Some("info")) => {
            let pe = Pe::open(&a[3]);
            println!("image base 0x{:x}", pe.base);
            for (n, va, vsz, ptr, rsz) in &pe.secs {
                println!("{n:<9} rva 0x{va:08x} vsz 0x{vsz:08x} raw 0x{ptr:08x} rsz 0x{rsz:08x}");
            }
        }
        (Some("pe"), Some("disp")) => {
            // Every site in .text that touches [reg + disp32], classified by
            // whether it loads or stores. On a stripped binary this is how you
            // find who FILLS the field a crash read as null.
            let pe = Pe::open(&a[3]);
            let want = (hexarg(&a[4]) as u32).to_le_bytes();
            let (_, va, _, ptr, rsz) = pe
                .secs
                .iter()
                .find(|s| s.0 == ".text")
                .expect("no .text")
                .clone();
            let d = &pe.d[ptr as usize..(ptr + rsz) as usize];
            let mut n = 0;
            for i in 2..d.len() - 4 {
                if d[i..i + 4] != want {
                    continue;
                }
                let modrm = d[i - 1];
                if modrm & 0xc0 != 0x80 {
                    continue; // not [reg+disp32]
                }
                let op = d[i - 2];
                let kind = match op {
                    0x89 => "store32/64",
                    0x8b => "load32/64",
                    0x8d => "lea",
                    0x38 | 0x39 | 0x3b => "cmp",
                    0x80..=0x83 => "alu-imm",
                    _ => "other",
                };
                // the REX prefix, if any, sits before the opcode
                let rex = if i >= 3 && d[i - 3] & 0xf0 == 0x40 {
                    d[i - 3]
                } else {
                    0
                };
                let start = i - if rex != 0 { 3 } else { 2 };
                println!(
                    "0x{:x}  {kind:<10} op {op:02x} modrm {modrm:02x} rex {rex:02x}",
                    va as usize + start
                );
                n += 1;
            }
            println!("{n} site(s)");
        }
        (Some("pe"), Some("fn")) => {
            // the x64 exception directory carries one RUNTIME_FUNCTION per
            // function: begin, end, unwind info. It is the function table of a
            // stripped binary.
            let pe = Pe::open(&a[3]);
            let (_, va, vsz, ptr, _) = pe
                .secs
                .iter()
                .find(|s| s.0 == ".pdata")
                .expect("no .pdata")
                .clone();
            let _ = va;
            for rva in a[4..].iter().map(|s| hexarg(s) as u32) {
                let n = (vsz / 12) as usize;
                let mut hit = None;
                for i in 0..n {
                    let o = ptr as usize + i * 12;
                    let b = u32at(&pe.d, o);
                    let e = u32at(&pe.d, o + 4);
                    if rva >= b && rva < e {
                        hit = Some((b, e));
                        break;
                    }
                }
                match hit {
                    Some((b, e)) => println!(
                        "0x{rva:x}  fn 0x{b:x} .. 0x{e:x}  (size 0x{:x}, +0x{:x} in)",
                        e - b,
                        rva - b
                    ),
                    None => println!("0x{rva:x}  no RUNTIME_FUNCTION"),
                }
            }
        }
        (Some("pe"), Some("bytes")) => {
            let pe = Pe::open(&a[3]);
            let rva = hexarg(&a[4]) as u32;
            let len = hexarg(&a[5]) as usize;
            let o = pe.off(rva).expect("rva not in any section");
            let b = &pe.d[o..(o + len).min(pe.d.len())];
            match a.get(6) {
                Some(p) => {
                    fs::write(p, b).unwrap();
                    println!("wrote {} bytes at rva 0x{rva:x} (va 0x{:x}, file 0x{o:x}) to {p}", b.len(), pe.base + rva as u64);
                }
                None => hexdump(pe.base + rva as u64, b),
            }
        }
        _ => println!("{usage}"),
    }
}
