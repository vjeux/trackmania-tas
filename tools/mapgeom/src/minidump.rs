//! `mapgeom crash` -- read a Windows minidump of the game and say where it died.
//!
//! When the client crashes loading one of our generated items, Windows leaves
//! `Trackmania.exe.<pid>.dmp` in `%LOCALAPPDATA%\CrashDumps`. Together with the
//! game exe (no symbols; VMProtect scrambled the .pdata function table) that is
//! the whole evidence. This module reads both with nothing but the standard
//! library:
//!
//! * the minidump streams (ThreadList, ModuleList, MemoryList/Memory64List,
//!   Exception, SystemInfo, ThreadInfoList, MemoryInfoList);
//! * the exception record and the full AMD64 CONTEXT of the faulting thread;
//! * a heuristic stack walk -- no unwind info: every 8-byte value on the stack
//!   that lands inside a module's code and is preceded by a call instruction is
//!   a frame. For frames inside the exe the call's target is decoded too, so
//!   the function each frame lives in is named by its start RVA, and frame k's
//!   return address is checked to lie inside the function frame k+1 called
//!   ("chain" -- the frames that pass are the real stack, the rest is stale);
//! * `--read ADDR LEN` hex dumps, `--find PATTERN` byte/string search with a
//!   reverse-pointer scan (who points at the hit), `--disasm RVA [N]` through
//!   objdump on the exe.
//!
//! Addresses: the exe is loaded at a random base (ASLR); its on-disk ImageBase
//! is 0x140000000, which is what objdump prints. Every exe address is shown
//! three ways -- loaded VA, RVA (`Trackmania.exe+0x456c35`) and the objdump
//! address (`0x140456c35`). Hex arguments accept any of the three: a value
//! inside the on-disk image range that the dump does not map is treated as an
//! objdump address and slid to the loaded base.

use std::fmt::Write as _;
use std::fs;
use std::process::Command;

fn u16at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(d[o..o + 2].try_into().unwrap())
}
fn u32at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn u64at(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}

pub fn hexarg(s: &str) -> Result<u64, String> {
    let t = s.trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(t, 16).map_err(|_| format!("not a hex number: {s}"))
}

/// Minidump stream types (MINIDUMP_STREAM_TYPE).
fn stream_name(ty: u32) -> &'static str {
    match ty {
        0 => "Unused",
        3 => "ThreadList",
        4 => "ModuleList",
        5 => "MemoryList",
        6 => "Exception",
        7 => "SystemInfo",
        8 => "ThreadExList",
        9 => "Memory64List",
        10 => "CommentA",
        11 => "CommentW",
        12 => "HandleData",
        13 => "FunctionTable",
        14 => "UnloadedModuleList",
        15 => "MiscInfo",
        16 => "MemoryInfoList",
        17 => "ThreadInfoList",
        18 => "HandleOperationList",
        19 => "Token",
        20 => "JavaScriptData",
        21 => "SystemMemoryInfo",
        22 => "ProcessVmCounters",
        23 => "IptTrace",
        24 => "ThreadNames",
        0x8000 => "ceStreamNull",
        _ => "?",
    }
}

#[derive(Clone, Debug)]
pub struct Module {
    pub base: u64,
    pub size: u32,
    pub timestamp: u32,
    pub checksum: u32,
    pub name: String,
}

impl Module {
    pub fn short(&self) -> &str {
        self.name.rsplit(['\\', '/']).next().unwrap_or(&self.name)
    }
    pub fn contains(&self, va: u64) -> bool {
        va >= self.base && va < self.base + self.size as u64
    }
}

#[derive(Clone, Debug)]
pub struct Thread {
    pub tid: u32,
    pub suspend_count: u32,
    pub priority_class: u32,
    pub priority: u32,
    pub teb: u64,
    pub stack_start: u64,
    pub stack_size: u32,
    pub stack_rva: u32,
    pub ctx_size: u32,
    pub ctx_rva: u32,
}

#[derive(Clone, Debug, Default)]
pub struct ThreadInfo {
    pub tid: u32,
    pub dump_flags: u32,
    pub dump_error: u32,
    pub exit_status: u32,
    pub create_time: u64,
    pub exit_time: u64,
    pub kernel_time: u64,
    pub user_time: u64,
    pub start_address: u64,
    pub affinity: u64,
}

#[derive(Clone, Debug)]
pub struct MemInfo {
    pub base: u64,
    pub alloc_base: u64,
    pub size: u64,
    pub state: u32,
    pub protect: u32,
    pub kind: u32,
}

#[derive(Clone, Debug)]
pub struct Exception {
    pub tid: u32,
    pub code: u32,
    pub flags: u32,
    pub address: u64,
    pub params: Vec<u64>,
    pub ctx_rva: usize,
    pub ctx_size: usize,
}

#[derive(Clone, Debug, Default)]
pub struct SystemInfo {
    pub arch: u16,
    pub level: u16,
    pub revision: u16,
    pub cpus: u8,
    pub product_type: u8,
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub platform: u32,
    pub csd: String,
}

/// One memory range the dump carries: [va, va+len) lives at `off` in the file.
#[derive(Clone, Copy, Debug)]
pub struct Range {
    pub va: u64,
    pub len: u64,
    pub off: usize,
}

pub struct Dump {
    pub d: Vec<u8>,
    pub dirs: Vec<(u32, u32, u32)>, // (type, size, rva)
    pub ranges: Vec<Range>,
    pub modules: Vec<Module>,
    pub threads: Vec<Thread>,
    pub thread_info: Vec<ThreadInfo>,
    pub meminfo: Vec<MemInfo>,
    pub exception: Option<Exception>,
    pub sysinfo: Option<SystemInfo>,
}

fn read_wstring(d: &[u8], rva: usize) -> String {
    if rva == 0 || rva + 4 > d.len() {
        return String::new();
    }
    let nlen = u32at(d, rva) as usize;
    let mut s = String::new();
    for k in 0..nlen / 2 {
        let o = rva + 4 + k * 2;
        if o + 2 > d.len() {
            break;
        }
        let c = u16at(d, o);
        s.push(char::from_u32(c as u32).unwrap_or('?'));
    }
    s
}

impl Dump {
    pub fn open(path: &str) -> Result<Dump, String> {
        let d = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if d.len() < 32 || &d[0..4] != b"MDMP" {
            return Err(format!("{path}: not a minidump (no MDMP signature)"));
        }
        let n = u32at(&d, 8) as usize;
        let dir = u32at(&d, 12) as usize;
        let mut dirs = Vec::new();
        for i in 0..n {
            let o = dir + i * 12;
            if o + 12 > d.len() {
                break;
            }
            dirs.push((u32at(&d, o), u32at(&d, o + 4), u32at(&d, o + 8)));
        }
        let mut dp = Dump {
            d,
            dirs,
            ranges: Vec::new(),
            modules: Vec::new(),
            threads: Vec::new(),
            thread_info: Vec::new(),
            meminfo: Vec::new(),
            exception: None,
            sysinfo: None,
        };
        dp.parse_streams()?;
        Ok(dp)
    }

    fn stream(&self, ty: u32) -> Option<(usize, usize)> {
        self.dirs.iter().find(|e| e.0 == ty).map(|e| (e.2 as usize, e.1 as usize))
    }

    fn parse_streams(&mut self) -> Result<(), String> {
        let d = &self.d;
        // Memory64List: u64 count, u64 base rva, then {u64 start, u64 size}
        // laid out contiguously from base rva.
        if let Some((rva, _)) = self.stream(9) {
            let n = u64at(d, rva) as usize;
            let mut off = u64at(d, rva + 8) as usize;
            for i in 0..n {
                let o = rva + 16 + i * 16;
                let va = u64at(d, o);
                let len = u64at(d, o + 8);
                self.ranges.push(Range { va, len, off });
                off += len as usize;
            }
        }
        // MemoryList: u32 count, then {u64 start, u32 size, u32 rva}
        if let Some((rva, _)) = self.stream(5) {
            let n = u32at(d, rva) as usize;
            for i in 0..n {
                let o = rva + 4 + i * 16;
                self.ranges.push(Range {
                    va: u64at(d, o),
                    len: u32at(d, o + 8) as u64,
                    off: u32at(d, o + 12) as usize,
                });
            }
        }
        self.ranges.sort_by_key(|r| r.va);
        // ModuleList: u32 count, then MINIDUMP_MODULE (108 bytes each)
        if let Some((rva, _)) = self.stream(4) {
            let n = u32at(d, rva) as usize;
            for i in 0..n {
                let o = rva + 4 + i * 108;
                self.modules.push(Module {
                    base: u64at(d, o),
                    size: u32at(d, o + 8),
                    checksum: u32at(d, o + 12),
                    timestamp: u32at(d, o + 16),
                    name: read_wstring(d, u32at(d, o + 20) as usize),
                });
            }
        }
        // ThreadList: u32 count, then MINIDUMP_THREAD (48 bytes each)
        if let Some((rva, _)) = self.stream(3) {
            let n = u32at(d, rva) as usize;
            for i in 0..n {
                let o = rva + 4 + i * 48;
                self.threads.push(Thread {
                    tid: u32at(d, o),
                    suspend_count: u32at(d, o + 4),
                    priority_class: u32at(d, o + 8),
                    priority: u32at(d, o + 12),
                    teb: u64at(d, o + 16),
                    stack_start: u64at(d, o + 24),
                    stack_size: u32at(d, o + 32),
                    stack_rva: u32at(d, o + 36),
                    ctx_size: u32at(d, o + 40),
                    ctx_rva: u32at(d, o + 44),
                });
            }
        }
        // ThreadInfoList: {u32 hdr size, u32 entry size, u32 count} + entries
        if let Some((rva, _)) = self.stream(17) {
            let hdr = u32at(d, rva) as usize;
            let esz = u32at(d, rva + 4) as usize;
            let n = u32at(d, rva + 8) as usize;
            for i in 0..n {
                let o = rva + hdr + i * esz;
                if o + 64 > d.len() {
                    break;
                }
                self.thread_info.push(ThreadInfo {
                    tid: u32at(d, o),
                    dump_flags: u32at(d, o + 4),
                    dump_error: u32at(d, o + 8),
                    exit_status: u32at(d, o + 12),
                    create_time: u64at(d, o + 16),
                    exit_time: u64at(d, o + 24),
                    kernel_time: u64at(d, o + 32),
                    user_time: u64at(d, o + 40),
                    start_address: u64at(d, o + 48),
                    affinity: u64at(d, o + 56),
                });
            }
        }
        // MemoryInfoList: {u32 hdr size, u32 entry size, u64 count} + entries
        if let Some((rva, _)) = self.stream(16) {
            let hdr = u32at(d, rva) as usize;
            let esz = u32at(d, rva + 4) as usize;
            let n = u64at(d, rva + 8) as usize;
            for i in 0..n {
                let o = rva + hdr + i * esz;
                if o + 48 > d.len() {
                    break;
                }
                self.meminfo.push(MemInfo {
                    base: u64at(d, o),
                    alloc_base: u64at(d, o + 8),
                    size: u64at(d, o + 24),
                    state: u32at(d, o + 32),
                    protect: u32at(d, o + 36),
                    kind: u32at(d, o + 40),
                });
            }
        }
        // SystemInfo
        if let Some((rva, _)) = self.stream(7) {
            self.sysinfo = Some(SystemInfo {
                arch: u16at(d, rva),
                level: u16at(d, rva + 2),
                revision: u16at(d, rva + 4),
                cpus: d[rva + 6],
                product_type: d[rva + 7],
                major: u32at(d, rva + 8),
                minor: u32at(d, rva + 12),
                build: u32at(d, rva + 16),
                platform: u32at(d, rva + 20),
                csd: read_wstring(d, u32at(d, rva + 24) as usize),
            });
        }
        // Exception: u32 tid, u32 pad, MINIDUMP_EXCEPTION (152), ctx location
        if let Some((rva, _)) = self.stream(6) {
            let er = rva + 8;
            let nparam = (u32at(d, er + 24) as usize).min(15);
            let params = (0..nparam).map(|i| u64at(d, er + 32 + i * 8)).collect();
            self.exception = Some(Exception {
                tid: u32at(d, rva),
                code: u32at(d, er),
                flags: u32at(d, er + 4),
                address: u64at(d, er + 16),
                params,
                ctx_size: u32at(d, er + 152) as usize,
                ctx_rva: u32at(d, er + 156) as usize,
            });
        }
        Ok(())
    }

    /// File offset of `va`, if the dump carries `len` bytes from there.
    pub fn offset(&self, va: u64, len: usize) -> Option<usize> {
        self.ranges
            .iter()
            .find(|r| va >= r.va && va + len as u64 <= r.va + r.len)
            .map(|r| r.off + (va - r.va) as usize)
    }
    pub fn read(&self, va: u64, len: usize) -> Option<&[u8]> {
        let o = self.offset(va, len)?;
        self.d.get(o..o + len)
    }
    pub fn q(&self, va: u64) -> Option<u64> {
        self.read(va, 8).map(|b| u64at(b, 0))
    }
    pub fn module_of(&self, va: u64) -> Option<&Module> {
        self.modules.iter().find(|m| m.contains(va))
    }
    pub fn meminfo_of(&self, va: u64) -> Option<&MemInfo> {
        self.meminfo.iter().find(|m| va >= m.base && va < m.base + m.size)
    }
    pub fn thread(&self, tid: u32) -> Option<&Thread> {
        self.threads.iter().find(|t| t.tid == tid)
    }
    /// The main executable: the module whose name ends in `.exe`, else the
    /// lowest-based one.
    pub fn exe_module(&self) -> Option<&Module> {
        self.modules
            .iter()
            .find(|m| m.short().to_ascii_lowercase().ends_with(".exe"))
            .or_else(|| self.modules.iter().min_by_key(|m| m.base))
    }
    /// Region label for a pointer: which module / stack / heap-ish region.
    pub fn describe(&self, va: u64) -> String {
        if va == 0 {
            return "NULL".into();
        }
        if let Some(m) = self.module_of(va) {
            return format!("{}+0x{:x}", m.short(), va - m.base);
        }
        for t in &self.threads {
            if va >= t.stack_start && va < t.stack_start + t.stack_size as u64 {
                return format!("stack of thread 0x{:x} (+0x{:x})", t.tid, va - t.stack_start);
            }
        }
        if let Some(mi) = self.meminfo_of(va) {
            let kind = match mi.kind {
                0x1000000 => "image",
                0x40000 => "mapped",
                0x20000 => "private",
                _ => "?",
            };
            let state = match mi.state {
                0x1000 => "commit",
                0x2000 => "reserve",
                0x10000 => "free",
                _ => "?",
            };
            let prot = match mi.protect & 0xff {
                0x01 => "---",
                0x02 => "r--",
                0x04 => "rw-",
                0x08 => "rwc",
                0x10 => "--x",
                0x20 => "r-x",
                0x40 => "rwx",
                0x80 => "rwcx",
                _ => "?",
            };
            let dumped = if self.offset(va, 1).is_some() { "in dump" } else { "NOT in dump" };
            return format!(
                "{kind} {state} {prot} region 0x{:x}+0x{:x} ({dumped})",
                mi.alloc_base, mi.size
            );
        }
        if self.offset(va, 1).is_some() {
            "in dump (no region info)".into()
        } else {
            "not mapped in dump".into()
        }
    }
}

// ------------------------------------------------------------------------ PE

pub struct Pe {
    pub path: String,
    pub d: Vec<u8>,
    pub base: u64,
    pub size_of_image: u32,
    pub timestamp: u32,
    pub secs: Vec<Section>,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    pub va: u32,
    pub vsize: u32,
    pub raw: u32,
    pub rsize: u32,
    pub chars: u32,
}

impl Section {
    pub fn executable(&self) -> bool {
        self.chars & 0x2000_0000 != 0
    }
    pub fn contains(&self, rva: u32) -> bool {
        rva >= self.va && rva < self.va + self.vsize.max(self.rsize)
    }
}

impl Pe {
    pub fn open(path: &str) -> Result<Pe, String> {
        let d = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if d.len() < 0x40 || &d[0..2] != b"MZ" {
            return Err(format!("{path}: not a PE (no MZ)"));
        }
        let pe = u32at(&d, 0x3c) as usize;
        if pe + 24 > d.len() || &d[pe..pe + 4] != b"PE\0\0" {
            return Err(format!("{path}: not a PE (no PE signature)"));
        }
        let nsec = u16at(&d, pe + 6) as usize;
        let timestamp = u32at(&d, pe + 8);
        let optsz = u16at(&d, pe + 20) as usize;
        let opt = pe + 24;
        let magic = u16at(&d, opt);
        let base = if magic == 0x20b { u64at(&d, opt + 24) } else { u32at(&d, opt + 28) as u64 };
        let size_of_image = u32at(&d, opt + 56);
        let mut secs = Vec::new();
        for i in 0..nsec {
            let o = opt + optsz + i * 40;
            secs.push(Section {
                name: String::from_utf8_lossy(&d[o..o + 8]).trim_end_matches('\0').to_string(),
                vsize: u32at(&d, o + 8),
                va: u32at(&d, o + 12),
                rsize: u32at(&d, o + 16),
                raw: u32at(&d, o + 20),
                chars: u32at(&d, o + 36),
            });
        }
        Ok(Pe { path: path.to_string(), d, base, size_of_image, timestamp, secs })
    }
    pub fn section_of(&self, rva: u32) -> Option<&Section> {
        self.secs.iter().find(|s| s.contains(rva))
    }
    /// File offset of an RVA, if it is backed by file bytes.
    pub fn off(&self, rva: u32) -> Option<usize> {
        let s = self.section_of(rva)?;
        let rel = rva - s.va;
        if rel >= s.rsize {
            return None;
        }
        let o = s.raw as usize + rel as usize;
        (o < self.d.len()).then_some(o)
    }
    pub fn bytes(&self, rva: u32, len: usize) -> Option<&[u8]> {
        let o = self.off(rva)?;
        self.d.get(o..o + len)
    }
    pub fn is_code(&self, rva: u32) -> bool {
        self.section_of(rva).map_or(false, |s| s.executable())
    }
}

// ------------------------------------------------------------- call decoding

/// Decode the `call` whose next instruction is at `ret` given the bytes just
/// before it. Returns (length, description, direct target relative to `ret`).
/// Recognises E8 rel32 and the FF /2 forms with REX prefixes, i.e. what MSVC
/// emits: `call rel32`, `call reg`, `call [reg+disp8/32]`, `call [rip+disp32]`,
/// `call [rsp+disp8]`.
pub fn decode_call(before: &[u8]) -> Option<(usize, String, Option<i64>)> {
    let n = before.len();
    let at = |k: usize| before.get(n.wrapping_sub(k)).copied();
    // E8 rel32
    if n >= 5 && at(5) == Some(0xE8) {
        let rel = i32::from_le_bytes(before[n - 4..].try_into().unwrap()) as i64;
        return Some((5, format!("call rel32 {}", shex(rel)), Some(rel)));
    }
    // FF /2 family. modrm reg field == 2.
    let is_call_modrm = |m: u8| (m >> 3) & 7 == 2;
    // len 2: FF D0..D7 (call reg)
    if n >= 2 && at(2) == Some(0xFF) && matches!(at(1), Some(m) if is_call_modrm(m) && m >> 6 == 3) {
        return Some((2, format!("call r{}", at(1).unwrap() & 7), None));
    }
    // len 3: 41 FF D0..D7 (call r8..r15) ; FF 10..17 (call [reg]) ; FF 14 24 (call [rsp])
    if n >= 3 && at(3) == Some(0x41) && at(2) == Some(0xFF) && matches!(at(1), Some(m) if is_call_modrm(m) && m >> 6 == 3) {
        return Some((3, format!("call r{}", 8 + (at(1).unwrap() & 7)), None));
    }
    if n >= 3 && at(3) == Some(0xFF) && matches!(at(2), Some(m) if is_call_modrm(m) && m >> 6 == 1 && m & 7 != 4) {
        return Some((3, format!("call [reg+{:#x}]", at(1).unwrap() as i8), None));
    }
    if n >= 2 && at(2) == Some(0xFF) && matches!(at(1), Some(m) if is_call_modrm(m) && m >> 6 == 0 && m & 7 != 4 && m & 7 != 5) {
        return Some((2, "call [reg]".into(), None));
    }
    // len 4: FF 54 24 xx (call [rsp+disp8]) ; 41 FF 50..57 xx ; 41 FF 10..17
    if n >= 4 && at(4) == Some(0xFF) && at(3) == Some(0x54) && at(2) == Some(0x24) {
        return Some((4, format!("call [rsp+{:#x}]", at(1).unwrap()), None));
    }
    if n >= 4 && at(4) == Some(0x41) && at(3) == Some(0xFF) && matches!(at(2), Some(m) if is_call_modrm(m) && m >> 6 == 1 && m & 7 != 4) {
        return Some((4, format!("call [r8-15+{:#x}]", at(1).unwrap() as i8), None));
    }
    if n >= 3 && at(3) == Some(0x41) && at(2) == Some(0xFF) && matches!(at(1), Some(m) if is_call_modrm(m) && m >> 6 == 0 && m & 7 != 4 && m & 7 != 5) {
        return Some((3, "call [r8-15]".into(), None));
    }
    // len 6: FF 15 disp32 (call [rip+disp32]) ; FF 90..97 disp32 (call [reg+disp32])
    if n >= 6 && at(6) == Some(0xFF) && at(5) == Some(0x15) {
        let rel = i32::from_le_bytes(before[n - 4..].try_into().unwrap()) as i64;
        return Some((6, format!("call [rip{rel:+#x}]"), None));
    }
    if n >= 6 && at(6) == Some(0xFF) && matches!(at(5), Some(m) if is_call_modrm(m) && m >> 6 == 2 && m & 7 != 4) {
        let disp = i32::from_le_bytes(before[n - 4..].try_into().unwrap());
        return Some((6, format!("call [reg{disp:+#x}]"), None));
    }
    // len 7: 41 FF 90..97 disp32 ; 48 FF 15 disp32 ; FF 94 24 disp32
    if n >= 7 && at(7) == Some(0x41) && at(6) == Some(0xFF) && matches!(at(5), Some(m) if is_call_modrm(m) && m >> 6 == 2 && m & 7 != 4) {
        let disp = i32::from_le_bytes(before[n - 4..].try_into().unwrap());
        return Some((7, format!("call [r8-15{disp:+#x}]"), None));
    }
    if n >= 7 && at(7) == Some(0xFF) && at(6) == Some(0x94) && at(5) == Some(0x24) {
        let disp = i32::from_le_bytes(before[n - 4..].try_into().unwrap());
        return Some((7, format!("call [rsp{disp:+#x}]"), None));
    }
    None
}

// ------------------------------------------------------------------- context

pub const REGS: [(&str, usize); 17] = [
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

pub struct Context<'a> {
    b: &'a [u8],
}

impl<'a> Context<'a> {
    pub fn reg(&self, name: &str) -> Option<u64> {
        REGS.iter().find(|(n, _)| *n == name).map(|(_, o)| u64at(self.b, *o))
    }
    pub fn rip(&self) -> u64 {
        u64at(self.b, 0xf8)
    }
    pub fn rsp(&self) -> u64 {
        u64at(self.b, 0x98)
    }
    pub fn eflags(&self) -> u32 {
        u32at(self.b, 0x44)
    }
    pub fn mxcsr(&self) -> u32 {
        u32at(self.b, 0x34)
    }
    pub fn xmm(&self, i: usize) -> &[u8] {
        &self.b[0x1a0 + i * 16..0x1a0 + i * 16 + 16]
    }
}

// ---------------------------------------------------------------- the report

/// Everything `mapgeom crash` knows, in one place, so the sub-reports share the
/// address translation.
pub struct Session {
    pub dump: Dump,
    pub pe: Option<Pe>,
    /// loaded base of the exe module in the dump
    pub exe_base: u64,
    pub exe_size: u32,
    pub exe_name: String,
}

impl Session {
    pub fn open(dump_path: &str, exe_path: Option<&str>) -> Result<Session, String> {
        let dump = Dump::open(dump_path)?;
        let pe = match exe_path {
            Some(p) => Some(Pe::open(p)?),
            None => None,
        };
        let (exe_base, exe_size, exe_name) = match dump.exe_module() {
            Some(m) => (m.base, m.size, m.short().to_string()),
            None => (0, 0, String::from("?")),
        };
        if let (Some(pe), Some(m)) = (&pe, dump.exe_module()) {
            if pe.timestamp != m.timestamp || pe.size_of_image != m.size {
                eprintln!(
                    "warning: {} on disk (timestamp 0x{:x}, size 0x{:x}) does not match the module in the dump (timestamp 0x{:x}, size 0x{:x}) -- RVAs may not line up",
                    pe.path, pe.timestamp, pe.size_of_image, m.timestamp, m.size
                );
            }
        }
        Ok(Session { dump, pe, exe_base, exe_size, exe_name })
    }

    pub fn disk_base(&self) -> u64 {
        self.pe.as_ref().map_or(0x1_4000_0000, |p| p.base)
    }
    pub fn in_exe(&self, va: u64) -> bool {
        self.exe_size > 0 && va >= self.exe_base && va < self.exe_base + self.exe_size as u64
    }
    pub fn rva(&self, va: u64) -> Option<u32> {
        self.in_exe(va).then(|| (va - self.exe_base) as u32)
    }
    /// A user-supplied address: loaded VA, or an objdump address in the
    /// on-disk image range that the dump itself does not map -> slide it.
    pub fn resolve_addr(&self, a: u64) -> u64 {
        let disk = self.disk_base();
        let sz = self.pe.as_ref().map_or(self.exe_size, |p| p.size_of_image) as u64;
        if self.exe_size > 0 && a >= disk && a < disk + sz && self.dump.offset(a, 1).is_none() && !self.dump.module_of(a).is_some() {
            return a - disk + self.exe_base;
        }
        a
    }
    /// Three-way rendering of an exe address (plus the section when it is not
    /// .text -- the VMProtect sections `.A2U` / `.D."` hold virtualised code).
    pub fn fmt_exe(&self, va: u64) -> String {
        match self.rva(va) {
            Some(r) => {
                let sec = match self.pe.as_ref().and_then(|p| p.section_of(r)) {
                    Some(s) if s.name != ".text" => format!(" [{}]", s.name),
                    _ => String::new(),
                };
                format!("{}+0x{:x} (objdump 0x{:x}){sec}", self.exe_name, r, self.disk_base() + r as u64)
            }
            None => match self.dump.module_of(va) {
                Some(m) => format!("{}+0x{:x}", m.short(), va - m.base),
                None => format!("0x{va:x}"),
            },
        }
    }
    /// Code bytes just before `va` (a return address): from the exe on disk
    /// for the exe module, else from the dump memory.
    fn code_before(&self, va: u64, n: usize) -> Option<Vec<u8>> {
        if let (Some(pe), Some(rva)) = (&self.pe, self.rva(va)) {
            if rva as usize >= n {
                if let Some(b) = pe.bytes(rva - n as u32, n) {
                    return Some(b.to_vec());
                }
            }
        }
        self.dump.read(va - n as u64, n).map(|b| b.to_vec())
    }
    fn is_code_va(&self, va: u64) -> bool {
        if let (Some(pe), Some(rva)) = (&self.pe, self.rva(va)) {
            return pe.is_code(rva);
        }
        match self.dump.module_of(va) {
            // no PE for other modules: accept anything past the headers
            Some(m) => va >= m.base + 0x1000,
            None => false,
        }
    }

    pub fn context(&self) -> Option<Context<'_>> {
        let e = self.dump.exception.as_ref()?;
        let b = self.dump.d.get(e.ctx_rva..e.ctx_rva + e.ctx_size.max(0x4d0))?;
        Some(Context { b })
    }

    // ---- reports

    pub fn report_header(&self, out: &mut String, dump_path: &str) {
        let d = &self.dump;
        let _ = writeln!(out, "dump     {dump_path} ({} bytes)", d.d.len());
        let _ = write!(out, "streams  ");
        for (i, (ty, sz, _)) in d.dirs.iter().enumerate() {
            if i > 0 {
                let _ = write!(out, ", ");
            }
            let _ = write!(out, "{}({ty}) {sz}B", stream_name(*ty));
        }
        let _ = writeln!(out);
        if let Some(s) = &d.sysinfo {
            let arch = match s.arch {
                0 => "x86",
                5 => "ARM",
                9 => "x64",
                12 => "ARM64",
                _ => "?",
            };
            let _ = writeln!(
                out,
                "system   {arch} cpu level {} rev 0x{:x}, {} cpus; Windows {}.{} build {} {}",
                s.level, s.revision, s.cpus, s.major, s.minor, s.build, s.csd
            );
        }
        let total: u64 = d.ranges.iter().map(|r| r.len).sum();
        let _ = writeln!(out, "memory   {} ranges, {} bytes ({} MiB) captured; {} region records", d.ranges.len(), total, total >> 20, d.meminfo.len());
        let _ = writeln!(out, "modules  {}", d.modules.len());
        for m in &d.modules {
            let mark = if m.base == self.exe_base { "  <== exe" } else { "" };
            let _ = writeln!(out, "  0x{:016x} +0x{:08x}  ts 0x{:08x}  {}{mark}", m.base, m.size, m.timestamp, m.short());
        }
        if self.exe_size > 0 {
            let _ = writeln!(
                out,
                "exe      {} loaded at 0x{:x}; ImageBase on disk 0x{:x}; slide {:+#x}  (RVA = VA - 0x{:x}; objdump address = 0x{:x} + RVA)",
                self.exe_name,
                self.exe_base,
                self.disk_base(),
                self.exe_base as i128 - self.disk_base() as i128,
                self.exe_base,
                self.disk_base()
            );
        }
        if let Some(pe) = &self.pe {
            let _ = writeln!(out, "pe       {} ImageBase 0x{:x} SizeOfImage 0x{:x} ts 0x{:x}", pe.path, pe.base, pe.size_of_image, pe.timestamp);
            for s in &pe.secs {
                let _ = writeln!(
                    out,
                    "  {:<8} rva 0x{:08x} vsz 0x{:08x} raw 0x{:08x} rsz 0x{:08x}{}",
                    s.name,
                    s.va,
                    s.vsize,
                    s.raw,
                    s.rsize,
                    if s.executable() { "  X" } else { "" }
                );
            }
        }
        let _ = writeln!(out, "threads  {}", d.threads.len());
        let crash_tid = d.exception.as_ref().map(|e| e.tid);
        for t in &d.threads {
            let ti = d.thread_info.iter().find(|i| i.tid == t.tid);
            let mark = if Some(t.tid) == crash_tid { "  <== faulting" } else { "" };
            let start = ti.map(|i| format!("  start {}", self.fmt_exe(i.start_address))).unwrap_or_default();
            let _ = writeln!(
                out,
                "  tid 0x{:<6x} stack 0x{:016x}+0x{:<7x} teb 0x{:x} prio {}/{}{start}{mark}",
                t.tid, t.stack_start, t.stack_size, t.teb, t.priority_class, t.priority
            );
        }
    }

    pub fn report_exception(&self, out: &mut String) {
        let d = &self.dump;
        let Some(e) = &d.exception else {
            let _ = writeln!(out, "no exception stream");
            return;
        };
        let name = match e.code {
            0xC0000005 => "ACCESS_VIOLATION",
            0xC000001D => "ILLEGAL_INSTRUCTION",
            0xC0000094 => "INTEGER_DIVIDE_BY_ZERO",
            0xC00000FD => "STACK_OVERFLOW",
            0xC0000409 => "STACK_BUFFER_OVERRUN / fastfail",
            0xC0000374 => "HEAP_CORRUPTION",
            0x80000003 => "BREAKPOINT",
            0xE06D7363 => "C++ exception",
            0xC0000006 => "IN_PAGE_ERROR",
            0xC000008C => "ARRAY_BOUNDS_EXCEEDED",
            0xC000008E => "FLT_DIVIDE_BY_ZERO",
            0xC0000090 => "FLT_INVALID_OPERATION",
            0xC0000096 => "PRIV_INSTRUCTION",
            _ => "",
        };
        let _ = writeln!(out, "\n=== exception ===");
        let _ = writeln!(out, "code     0x{:08x} {name}  flags 0x{:x}  thread 0x{:x}", e.code, e.flags, e.tid);
        let _ = writeln!(out, "address  0x{:016x}  {}", e.address, self.fmt_exe(e.address));
        if e.code == 0xC0000005 && e.params.len() >= 2 {
            let what = match e.params[0] {
                0 => "read from",
                1 => "write to",
                8 => "execute at",
                _ => "access",
            };
            let _ = writeln!(out, "fault    {what} 0x{:x}  ({})", e.params[1], d.describe(e.params[1]));
        } else {
            for (i, p) in e.params.iter().enumerate() {
                let _ = writeln!(out, "param{i}   0x{p:016x}");
            }
        }
        let Some(ctx) = self.context() else {
            let _ = writeln!(out, "(no context)");
            return;
        };
        let _ = writeln!(out, "--- registers ---");
        for (n, o) in REGS {
            let v = u64at(ctx.b, o);
            let mut line = format!("{n:<4} 0x{v:016x}");
            if self.in_exe(v) || d.module_of(v).is_some() {
                line.push_str(&format!("   {}", self.fmt_exe(v)));
            } else if let Some(q) = d.q(v) {
                line.push_str(&format!("   -> 0x{q:016x}  [{}]", d.describe(v)));
            } else if v != 0 && v < 0x10000 {
                line.push_str(&format!("   ({v} decimal)"));
            } else if v != 0 {
                line.push_str(&format!("   [{}]", d.describe(v)));
            }
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out, "eflags 0x{:08x}  mxcsr 0x{:08x}", ctx.eflags(), ctx.mxcsr());
        for i in 0..16 {
            let x = ctx.xmm(i);
            let f: Vec<String> = (0..4).map(|k| format!("{}", f32::from_bits(u32at(x, k * 4)))).collect();
            let _ = writeln!(out, "xmm{i:<2} 0x{:016x}{:016x}  f32[{}]", u64at(x, 8), u64at(x, 0), f.join(", "));
        }
        // the faulting instruction, from the exe if we have it
        let rip = ctx.rip();
        let _ = writeln!(out, "--- code at rip ---");
        match self.disasm(rip, 8) {
            Ok(s) => out.push_str(&s),
            Err(e) => {
                let _ = writeln!(out, "(no disassembly: {e})");
                if let Some(b) = d.read(rip.saturating_sub(16), 48) {
                    hexdump_into(out, rip - 16, b, None);
                }
            }
        }
    }

    /// Frames of the faulting thread, heuristically.
    ///
    /// 1. every 8-byte stack slot from rsp to the top of the thread's stack
    ///    whose value lands in code and is preceded by a call is a candidate;
    /// 2. the chain: starting from rip, the first candidate whose (direct)
    ///    callee contains the current address is the real caller frame; then
    ///    from its return address, and so on. Candidates that never match are
    ///    stale return addresses left in dead stack (a 0x1700-byte frame holds
    ///    many). When no direct call matches, the first indirect call (`call
    ///    [reg+x]`, a virtual call -- target unknowable without the registers)
    ///    is taken as an unverified link and the search goes on from there.
    pub fn stack_walk(&self) -> Vec<Frame> {
        let d = &self.dump;
        let mut frames = Vec::new();
        let Some(ctx) = self.context() else { return frames };
        let rip = ctx.rip();
        let rsp = ctx.rsp();
        frames.push(Frame { sp: rsp, ret: rip, call: None, callee: None, depth: 0, link: Link::Rip, func: None });
        // bound the scan by the thread's stack range if we know it
        let end = d
            .exception
            .as_ref()
            .and_then(|e| d.thread(e.tid))
            .map(|t| t.stack_start + t.stack_size as u64)
            .filter(|&e| e > rsp)
            .unwrap_or(rsp + 0x20000);
        let mut sp = rsp & !7;
        while sp < end {
            let Some(v) = d.q(sp) else { break };
            if v >= 0x10000 && self.is_code_va(v) {
                let before = self.code_before(v, 7);
                let call = before.as_deref().and_then(decode_call);
                if let Some((len, desc, rel)) = call {
                    let callee = rel.map(|r| (v as i64 + r) as u64);
                    frames.push(Frame { sp, ret: v, call: Some((len, desc)), callee, depth: 0, link: Link::Stale, func: None });
                }
            }
            sp += 8;
        }
        // the chain
        const FN_MAX: u64 = 0x40000;
        let mut cur = rip;
        let mut idx = 1;
        let mut depth = 1;
        while idx < frames.len() {
            let direct = (idx..frames.len()).find(|&j| matches!(frames[j].callee, Some(c) if self.same_function(c, cur, FN_MAX)));
            let pick = match direct {
                Some(j) => {
                    frames[j].link = Link::Verified;
                    j
                }
                None => {
                    // No direct call owns `cur`: the caller used a virtual /
                    // indirect call. Prefer the indirect candidate whose own
                    // return address a later direct frame vouches for; the
                    // indirect candidates skipped on the way stay "possible".
                    let indirect: Vec<usize> = (idx..frames.len()).filter(|&j| frames[j].callee.is_none()).collect();
                    let confirmed = indirect.iter().copied().find(|&j| {
                        (j + 1..frames.len()).any(|k| matches!(frames[k].callee, Some(c) if self.same_function(c, frames[j].ret, FN_MAX)))
                    });
                    let Some(j) = confirmed.or(indirect.first().copied()) else { break };
                    for &p in indirect.iter().take_while(|&&p| p < j) {
                        frames[p].link = Link::Possible;
                    }
                    frames[j].link = if confirmed.is_some() { Link::Indirect } else { Link::Unverified };
                    j
                }
            };
            // the frame below the picked one executes inside the picked callee
            let callee = frames[pick].callee;
            let below = (0..pick).rev().find(|&k| !matches!(frames[k].link, Link::Stale | Link::Possible)).unwrap_or(0);
            if frames[below].func.is_none() {
                frames[below].func = callee;
            }
            frames[pick].depth = depth;
            depth += 1;
            cur = frames[pick].ret;
            idx = pick + 1;
        }
        frames
    }

    pub fn report_stack(&self, out: &mut String, all: bool) {
        let frames = self.stack_walk();
        let _ = writeln!(out, "\n=== stack walk (faulting thread; heuristic: return addresses preceded by a call, chained through the callees) ===");
        let rsp0 = frames.first().map_or(0, |f| f.sp);
        let mut stale = 0;
        for f in &frames {
            if f.link == Link::Stale && !all {
                stale += 1;
                continue;
            }
            let mut line = if f.link == Link::Rip {
                format!("#{:<3} rip          0x{:016x}  {}", f.depth, f.ret, self.fmt_exe(f.ret))
            } else {
                let tag = match f.link {
                    Link::Verified => " ",
                    Link::Indirect => "v",
                    Link::Unverified => "?",
                    Link::Possible => "?",
                    _ => "x",
                };
                let num = if matches!(f.link, Link::Possible | Link::Stale) { "  -".to_string() } else { format!("{:<3}", f.depth) };
                format!("#{num}{tag}[rsp+0x{:<5x}] 0x{:016x}  {}", f.sp - rsp0, f.ret, self.fmt_exe(f.ret))
            };
            if let Some((_, desc)) = &f.call {
                line.push_str(&format!("   after {desc}"));
            }
            if let Some(c) = f.callee {
                line.push_str(&format!(" -> {}", self.fmt_exe(c)));
            }
            if let Some(func) = f.func {
                line.push_str(&format!("   [in fn {} +0x{:x}]", self.fmt_exe(func), f.ret - func));
            }
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(
            out,
            "  ({} candidate return address(es) not on the chain{})\n  legend: ' ' direct call, callee contains the frame below; 'v' indirect call, its return address vouched for by the next direct frame;\n          '?' indirect call, nothing vouches for it (#- = skipped in favour of a vouched one, may still be real)",
            stale,
            if all { "" } else { " hidden; --all-frames shows them" }
        );
        // The faulting function's prologue: does the context agree with it?
        if let (Some(ctx), Some(func)) = (self.context(), frames.first().and_then(|f| f.func)) {
            match self.prologue(func) {
                Ok(p) => {
                    let _ = writeln!(out, "\n--- faulting function {} ---", self.fmt_exe(func));
                    let _ = writeln!(out, "prologue: {} push(es), sub rsp 0x{:x}{}{}", p.pushes, p.sub, if p.chkstk { " (via __chkstk)" } else { "" }, match p.rbp { Some(o) => format!(", rbp = rsp_after_pushes {}", shex(o)), None => String::new() });
                    let rsp = ctx.rsp();
                    let ret_slot = rsp + p.sub + 8 * p.pushes as u64;
                    let chained = frames.iter().find(|f| f.depth == 1).map(|f| f.sp);
                    let _ = writeln!(
                        out,
                        "return slot from prologue: rsp+0x{:x} = 0x{ret_slot:x}  {}",
                        p.sub + 8 * p.pushes as u64,
                        match chained {
                            Some(s) if s == ret_slot => "== frame #1 (frame size confirmed)".to_string(),
                            Some(s) => format!("!= frame #1 at 0x{s:x} (rsp moved after the prologue, or the wrong function)"),
                            None => "(no frame #1)".to_string(),
                        }
                    );
                    if let Some(o) = p.rbp {
                        let want = (rsp + p.sub) as i64 + o;
                        let have = ctx.reg("rbp").unwrap_or(0) as i64;
                        if want == have {
                            let _ = writeln!(out, "rbp from prologue: 0x{want:x} == context rbp");
                        } else {
                            let _ = writeln!(
                                out,
                                "rbp from prologue: 0x{want:x}  BUT context rbp = 0x{have:x} ({}) -- read [rbp+X] locals at 0x{want:x}+X, the context value is not the frame pointer this code used",
                                shex(have - want)
                            );
                        }
                    }
                }
                Err(e) => {
                    let _ = writeln!(out, "(prologue of {}: {e})", self.fmt_exe(func));
                }
            }
        }
    }

    /// Is `addr` inside the function that starts at `func`? Without unwind
    /// info (VMProtect scrambled .pdata) the test is: `addr` lies after `func`,
    /// within `max` bytes, and no `int3 int3` padding -- which MSVC puts only
    /// between functions -- sits between the two in the exe's bytes.
    pub fn same_function(&self, func: u64, addr: u64, max: u64) -> bool {
        if addr < func || addr - func >= max {
            return false;
        }
        if let (Some(pe), Some(f), Some(a)) = (&self.pe, self.rva(func), self.rva(addr)) {
            if let Some(b) = pe.bytes(f, (a - f) as usize) {
                return !b.windows(2).any(|w| w == [0xCC, 0xCC]);
            }
        }
        // no bytes to look at: fall back to a tighter distance bound
        addr - func < 0x10000
    }

    /// Parse a function prologue through objdump: pushes, `lea rbp,[rsp±N]` /
    /// `mov rbp,rsp`, `sub rsp,N` (or `mov eax,N; call __chkstk; sub rsp,rax`).
    pub fn prologue(&self, func: u64) -> Result<Prologue, String> {
        let pe = self.pe.as_ref().ok_or("no --exe")?;
        let rva = self.rva(func).ok_or("not in exe")?;
        let text = self.objdump_range(pe.base + rva as u64, pe.base + rva as u64 + 0x60)?;
        let mut p = Prologue::default();
        let mut pending_eax: Option<u64> = None;
        let mut saw_call = false;
        for l in text.lines() {
            let Some((_, ins)) = l.split_once(":\t") else { continue };
            let ins = ins.split('\t').nth(1).unwrap_or("").trim();
            let ins = ins.trim_start_matches("rex ").trim();
            let (mn, ops) = ins.split_once(char::is_whitespace).unwrap_or((ins, ""));
            let ops = ops.trim();
            match mn {
                "push" => p.pushes += 1,
                "lea" if ops.starts_with("rbp,[rsp") => {
                    let inner = ops.trim_start_matches("rbp,[rsp").trim_end_matches(']');
                    let v = inner.trim_start_matches(['+', '-']).trim_start_matches("0x");
                    let n = i64::from_str_radix(v, 16).map_err(|_| format!("lea operand {ops}"))?;
                    p.rbp = Some(if inner.starts_with('-') { -n } else { n });
                }
                "mov" if ops == "rbp,rsp" => p.rbp = Some(0),
                "mov" if ops.starts_with("eax,0x") => pending_eax = u64::from_str_radix(&ops[6..], 16).ok(),
                "call" if pending_eax.is_some() => saw_call = true,
                "sub" if ops == "rsp,rax" && saw_call => {
                    p.sub = pending_eax.unwrap_or(0);
                    p.chkstk = true;
                    break;
                }
                "sub" if ops.starts_with("rsp,0x") => {
                    p.sub = u64::from_str_radix(&ops[6..], 16).map_err(|_| format!("sub operand {ops}"))?;
                    break;
                }
                "mov" if ops.contains("[rsp+") || ops.contains("[rsp]") => {} // arg homing
                "int3" | "nop" => {}
                _ => {
                    // anything else: prologue over without a stack adjust
                    // (a leaf), unless nothing was seen yet.
                    if p.pushes > 0 || p.rbp.is_some() {
                        break;
                    }
                    return Err(format!("no prologue at {} (first insn: {ins})", self.fmt_exe(func)));
                }
            }
        }
        Ok(p)
    }

    fn objdump_range(&self, start: u64, stop: u64) -> Result<String, String> {
        let pe = self.pe.as_ref().ok_or("no --exe")?;
        let outp = Command::new("objdump")
            .args([
                "-d",
                "-b",
                "pei-x86-64",
                "-m",
                "i386:x86-64",
                "-M",
                "intel",
                "-z",
                "--wide",
                &format!("--start-address=0x{start:x}"),
                &format!("--stop-address=0x{stop:x}"),
                &pe.path,
            ])
            .output()
            .map_err(|e| format!("objdump: {e}"))?;
        if !outp.status.success() {
            return Err(format!("objdump failed: {}", String::from_utf8_lossy(&outp.stderr)));
        }
        Ok(String::from_utf8_lossy(&outp.stdout).into_owned())
    }

    // ---- memory tools

    pub fn read_report(&self, out: &mut String, addr: u64, len: usize) {
        let va = self.resolve_addr(addr);
        match self.dump.read(va, len) {
            Some(b) => hexdump_into(out, va, b, Some(self)),
            None => {
                let _ = writeln!(out, "0x{va:x}+0x{len:x} not in the dump ({})", self.dump.describe(va));
                if let (Some(pe), Some(rva)) = (&self.pe, self.rva(va)) {
                    if let Some(b) = pe.bytes(rva, len) {
                        let _ = writeln!(out, "from the exe on disk (RVA 0x{rva:x}):");
                        hexdump_into(out, va, b, Some(self));
                        return;
                    }
                }
                let _ = writeln!(out, "ranges near it:");
                for r in &self.dump.ranges {
                    if r.va + r.len > va.saturating_sub(0x100000) && r.va < va + 0x100000 {
                        let _ = writeln!(out, "  0x{:016x} .. 0x{:016x}  ({} B)", r.va, r.va + r.len, r.len);
                    }
                }
            }
        }
    }

    /// Search every captured range for `pat`; returns hit VAs.
    pub fn find_bytes(&self, pat: &[u8], limit: usize) -> Vec<u64> {
        let mut hits = Vec::new();
        if pat.is_empty() {
            return hits;
        }
        for r in &self.dump.ranges {
            let end = (r.off + r.len as usize).min(self.dump.d.len());
            if r.off >= end {
                continue;
            }
            let hay = &self.dump.d[r.off..end];
            let mut i = 0;
            while i + pat.len() <= hay.len() {
                if hay[i] == pat[0] && &hay[i..i + pat.len()] == pat {
                    hits.push(r.va + i as u64);
                    if hits.len() >= limit {
                        return hits;
                    }
                    i += pat.len();
                } else {
                    i += 1;
                }
            }
        }
        hits
    }

    /// 8-byte aligned pointers anywhere in the dump whose value is in [lo, hi).
    pub fn find_pointers(&self, lo: u64, hi: u64, limit: usize) -> Vec<(u64, u64)> {
        let mut hits = Vec::new();
        for r in &self.dump.ranges {
            let end = (r.off + r.len as usize).min(self.dump.d.len());
            if r.off + 8 > end {
                continue;
            }
            let hay = &self.dump.d[r.off..end];
            // pointers are 8-aligned in practice; align the scan to the VA
            let start = ((r.va + 7) & !7) - r.va;
            let mut i = start as usize;
            while i + 8 <= hay.len() {
                let v = u64at(hay, i);
                if v >= lo && v < hi {
                    hits.push((r.va + i as u64, v));
                    if hits.len() >= limit {
                        return hits;
                    }
                }
                i += 8;
            }
        }
        hits
    }

    pub fn find_report(&self, out: &mut String, spec: &str, limit: usize) {
        // spec: hex:DEADBEEF | wide:text | text (ASCII; also searched as UTF-16LE)
        let mut pats: Vec<(String, Vec<u8>)> = Vec::new();
        if let Some(h) = spec.strip_prefix("hex:") {
            let h: String = h.chars().filter(|c| !c.is_whitespace()).collect();
            let mut b = Vec::new();
            let mut i = 0;
            while i + 2 <= h.len() {
                match u8::from_str_radix(&h[i..i + 2], 16) {
                    Ok(x) => b.push(x),
                    Err(_) => {
                        let _ = writeln!(out, "bad hex pattern {spec}");
                        return;
                    }
                }
                i += 2;
            }
            pats.push(("bytes".into(), b));
        } else if let Some(w) = spec.strip_prefix("wide:") {
            pats.push(("utf16".into(), w.encode_utf16().flat_map(|c| c.to_le_bytes()).collect()));
        } else {
            pats.push(("ascii".into(), spec.as_bytes().to_vec()));
            pats.push(("utf16".into(), spec.encode_utf16().flat_map(|c| c.to_le_bytes()).collect()));
        }
        let _ = writeln!(out, "\n=== find {spec:?} ===");
        for (kind, pat) in pats {
            let hits = self.find_bytes(&pat, limit);
            let _ = writeln!(out, "{kind} ({} bytes): {} hit(s){}", pat.len(), hits.len(), if hits.len() >= limit { " (limit)" } else { "" });
            for h in hits {
                let _ = writeln!(out, "  0x{h:016x}  [{}]", self.dump.describe(h));
                // context: the containing bytes, a little before and after
                let lo = h.saturating_sub(16);
                if let Some(b) = self.dump.read(lo, 16 + pat.len() + 16) {
                    let _ = writeln!(out, "    {}", ascii_preview(b));
                }
                // who points here? exact start, and into the first 16 bytes
                let ptrs = self.find_pointers(h, h + 1, 32);
                for (at, v) in &ptrs {
                    let _ = writeln!(out, "    <- pointer at 0x{at:016x} [{}] = 0x{v:x}", self.dump.describe(*at));
                    // what does the pointer's neighbourhood look like? (a
                    // string object usually has the length nearby)
                    if let Some(b) = self.dump.read(at.saturating_sub(16), 48) {
                        let _ = writeln!(out, "       {}", qword_preview(at.saturating_sub(16), b, self));
                    }
                }
                if ptrs.is_empty() {
                    let near = self.find_pointers(h.saturating_sub(64), h, 8);
                    for (at, v) in near {
                        let _ = writeln!(out, "    <- pointer at 0x{at:016x} [{}] = 0x{v:x} (into the {} bytes before)", self.dump.describe(at), h - v);
                    }
                }
            }
        }
    }

    // ---- disassembly through objdump

    /// `n` instructions around a loaded VA (or RVA / objdump address).
    pub fn disasm(&self, addr: u64, n: usize) -> Result<String, String> {
        let pe = self.pe.as_ref().ok_or("no --exe given")?;
        // accept loaded VA, objdump VA, or a bare RVA
        let rva: u64 = if self.in_exe(addr) {
            addr - self.exe_base
        } else if addr >= pe.base && addr < pe.base + pe.size_of_image as u64 {
            addr - pe.base
        } else if addr < pe.size_of_image as u64 {
            addr
        } else {
            return Err(format!("0x{addr:x} is not inside {}", self.exe_name));
        };
        let target = pe.base + rva;
        // x86 decoding resynchronises within a few instructions, so start a
        // window before the target and print the lines around it.
        let before = (n as u64 * 6).min(rva);
        let after = n as u64 * 6 + 16;
        let text = self.objdump_range(target - before, target + after)?;
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                t.len() > 10 && t.as_bytes()[0].is_ascii_hexdigit() && t.contains(":\t")
            })
            .collect();
        let idx = lines.iter().position(|l| {
            l.trim_start().split(':').next().and_then(|h| u64::from_str_radix(h.trim(), 16).ok()) == Some(target)
        });
        let mut out = String::new();
        let _ = writeln!(
            out,
            "objdump {} @ 0x{:x} = {}+0x{:x} = loaded 0x{:x}",
            pe.path, target, self.exe_name, rva, self.exe_base + rva
        );
        match idx {
            Some(i) => {
                let lo = i.saturating_sub(n / 2);
                let hi = (i + n / 2 + 1).min(lines.len());
                for (k, l) in lines[lo..hi].iter().enumerate() {
                    let mark = if lo + k == i { "=>" } else { "  " };
                    let _ = writeln!(out, "{mark} {}", l.trim_end());
                }
            }
            None => {
                let _ = writeln!(out, "(target not on an instruction boundary in this window; raw window follows)");
                for l in lines.iter().take(n) {
                    let _ = writeln!(out, "   {}", l.trim_end());
                }
            }
        }
        Ok(out)
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    /// stack slot holding the return address (rsp itself for the rip frame)
    pub sp: u64,
    /// the return address (rip for frame 0)
    pub ret: u64,
    /// decoded call instruction before `ret`: (length, description)
    pub call: Option<(usize, String)>,
    /// direct call target, when the call was `call rel32`
    pub callee: Option<u64>,
    /// position on the chain (0 = rip); stale candidates keep 0
    pub depth: usize,
    pub link: Link,
    /// the function this frame executes in (the callee of the frame above)
    pub func: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Rip,
    /// the callee of this frame's call contains the previous frame's address
    Verified,
    /// an indirect call; the next verified frame's callee contains its return address
    Indirect,
    /// an indirect call taken because nothing direct matched and nothing vouches for it
    Unverified,
    /// an indirect call skipped in favour of a later, vouched-for one; may still be real
    Possible,
    /// a return address on the stack that is not on the chain
    Stale,
}

#[derive(Clone, Debug, Default)]
pub struct Prologue {
    pub pushes: usize,
    /// rbp = rsp_after_pushes + this
    pub rbp: Option<i64>,
    pub sub: u64,
    pub chkstk: bool,
}

fn ascii_preview(b: &[u8]) -> String {
    b.iter().map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { '.' }).collect()
}

fn qword_preview(base: u64, b: &[u8], s: &Session) -> String {
    let mut out = String::new();
    for (i, ch) in b.chunks(8).enumerate() {
        if ch.len() < 8 {
            break;
        }
        let v = u64at(ch, 0);
        let _ = write!(out, "[0x{:x}]={:#x}", base + i as u64 * 8, v);
        if s.in_exe(v) {
            let _ = write!(out, "({})", s.fmt_exe(v));
        }
        out.push(' ');
    }
    out
}

pub fn hexdump_into(out: &mut String, base: u64, b: &[u8], s: Option<&Session>) {
    for (i, chunk) in b.chunks(16).enumerate() {
        let mut line = format!("{:016x}  ", base + (i * 16) as u64);
        for (j, x) in chunk.iter().enumerate() {
            let _ = write!(line, "{x:02x} ");
            if j == 7 {
                line.push(' ');
            }
        }
        for _ in chunk.len()..16 {
            line.push_str("   ");
        }
        line.push_str(" |");
        line.push_str(&ascii_preview(chunk));
        line.push('|');
        if let Some(s) = s {
            // annotate qwords that point into the exe
            for k in (0..chunk.len()).step_by(8) {
                if k + 8 <= chunk.len() {
                    let v = u64at(chunk, k);
                    if s.in_exe(v) {
                        let _ = write!(line, "  q{}: {}", k / 8, s.fmt_exe(v));
                    }
                }
            }
        }
        let _ = writeln!(out, "{line}");
    }
}

// ------------------------------------------------------------------- the CLI

pub const USAGE: &str = "\
mapgeom crash <DUMP.dmp> [--exe Trackmania.exe] [options]
    default: dump summary, exception + registers, faulting instruction, stack walk
    --read ADDR LEN      hex dump LEN (hex) bytes at ADDR (loaded VA or objdump address)
    --find PATTERN       search all captured memory for an ASCII string (also as UTF-16),
                         `hex:DEADBEEF` bytes or `wide:text`; lists who points at each hit
    --disasm ADDR [N]    N instructions (default 16) around ADDR via objdump on --exe
                         (ADDR: loaded VA, objdump 0x14xxxxxxx address, or bare RVA)
    --limit N            cap --find hits (default 20)
    --no-stack           skip the stack walk
    --all-frames         also list the stale return addresses the chain skipped
    --quiet              skip the dump summary (modules, threads)
";

pub fn run(args: &[String]) -> Result<(), String> {
    // args[0] == "crash"
    let dump_path = args.get(1).filter(|a| !a.starts_with("--")).ok_or_else(|| USAGE.to_string())?;
    let mut exe = None;
    let mut reads: Vec<(u64, usize)> = Vec::new();
    let mut finds: Vec<String> = Vec::new();
    let mut disasms: Vec<(u64, usize)> = Vec::new();
    let mut limit = 20usize;
    let mut no_stack = false;
    let mut all_frames = false;
    let mut quiet = false;
    let mut i = 2;
    let next = |i: &mut usize, what: &str| -> Result<String, String> {
        *i += 1;
        args.get(*i).cloned().ok_or_else(|| format!("{what} needs a value\n{USAGE}"))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--exe" => exe = Some(next(&mut i, "--exe")?),
            "--read" => {
                let a = hexarg(&next(&mut i, "--read ADDR")?)?;
                let l = hexarg(&next(&mut i, "--read ADDR LEN")?)? as usize;
                reads.push((a, l));
            }
            "--find" => finds.push(next(&mut i, "--find")?),
            "--disasm" => {
                let a = hexarg(&next(&mut i, "--disasm ADDR")?)?;
                let mut n = 16;
                if let Some(v) = args.get(i + 1) {
                    if !v.starts_with("--") {
                        if let Ok(k) = v.parse::<usize>() {
                            n = k;
                            i += 1;
                        }
                    }
                }
                disasms.push((a, n));
            }
            "--limit" => limit = next(&mut i, "--limit")?.parse().map_err(|_| "--limit N".to_string())?,
            "--no-stack" => no_stack = true,
            "--all-frames" => all_frames = true,
            "--quiet" => quiet = true,
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
        i += 1;
    }
    // Default exe: the sibling of the dump, or /tmp/tmexe/Trackmania.exe if present.
    if exe.is_none() {
        for cand in ["Trackmania.exe", "/tmp/tmexe/Trackmania.exe"] {
            let p = std::path::Path::new(dump_path).parent().unwrap_or(std::path::Path::new(".")).join(cand);
            if p.exists() {
                exe = Some(p.to_string_lossy().into_owned());
                break;
            }
            if std::path::Path::new(cand).exists() {
                exe = Some(cand.to_string());
                break;
            }
        }
    }
    let s = Session::open(dump_path, exe.as_deref())?;
    let targeted = !reads.is_empty() || !finds.is_empty() || !disasms.is_empty();
    let mut out = String::new();
    if !targeted {
        if !quiet {
            s.report_header(&mut out, dump_path);
        }
        s.report_exception(&mut out);
        if !no_stack {
            s.report_stack(&mut out, all_frames);
        }
    }
    for (a, l) in reads {
        let _ = writeln!(out, "\n=== read 0x{a:x} +0x{l:x} ===");
        s.read_report(&mut out, a, l);
    }
    for f in finds {
        s.find_report(&mut out, &f, limit);
    }
    for (a, n) in disasms {
        let _ = writeln!(out, "\n=== disasm 0x{a:x} ===");
        match s.disasm(a, n) {
            Ok(t) => out.push_str(&t),
            Err(e) => {
                let _ = writeln!(out, "{e}");
            }
        }
    }
    print!("{out}");
    Ok(())
}

/// Signed hex: `+0x10`, `-0x1600`.
pub fn shex(v: i64) -> String {
    if v < 0 {
        format!("-0x{:x}", -v)
    } else {
        format!("+0x{v:x}")
    }
}
