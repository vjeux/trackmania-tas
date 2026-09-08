//! The engine's reflection tables, read off `Trackmania.exe`: for one class
//! id, its size, its parent, and every member the class registers — name,
//! object offset, and the type-descriptor function — in DECLARATION ORDER.
//!
//! How (found 2026-09-08 while opening the particle classes): each class has
//! a registration function that starts with `mov ecx, <class id>` + `mov
//! DWORD PTR [rsp+0x20], <parent class id>` + `mov r8d, <object size>` and a
//! call, then, per member, `lea rcx,[rbp-0x20]; call <type fn>; lea r8,
//! [rbp-0x40]; mov edx, <offset>; lea rcx, [rip+<name>]; …; call <register>`
//! (the name `lea` sometimes lands after the `movups/movaps` copy). MSVC
//! keeps that shape for every class, so a byte scan suffices: no decoder.
//!
//! The declaration order is what the chunk readers serialise (checked against
//! `Fogger16M.ParticleModel.Gbx`: `CPlugParticleEmitterSubModel` chunk 0x2D
//! walks the first members in this order), and the type functions are named
//! by the members they type across the whole exe (`--types` lists them: a
//! function that types `Gravity`, `SizeBirth` and `LifeVariation` is Real).

use crate::minidump::Pe;
use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Clone, Debug)]
pub struct Member {
    pub offset: u32,
    pub type_fn: u64,
    pub name: String,
    /// RVA of the `mov edx, offset` instruction.
    pub at: u32,
}

#[derive(Clone, Debug)]
pub struct ClassInfo {
    pub class_id: u32,
    pub parent_id: u32,
    pub size: u32,
    pub name: String,
    pub fn_rva: u32,
    pub members: Vec<Member>,
}

fn u32le(b: &[u8]) -> u32 {
    if b.len() < 4 {
        return 0;
    }
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}
fn i32le(b: &[u8]) -> i32 {
    u32le(b) as i32
}

/// A NUL-terminated ASCII string at an RVA (up to 200 bytes).
fn cstr(pe: &Pe, rva: u32) -> Option<String> {
    let b = pe.bytes(rva, 200).or_else(|| {
        let o = pe.off(rva)?;
        pe.d.get(o..)
    })?;
    let n = b.iter().position(|&c| c == 0)?;
    let s = &b[..n];
    if s.is_empty() || !s.iter().all(|&c| (0x20..0x7f).contains(&c)) {
        return None;
    }
    Some(String::from_utf8_lossy(s).into_owned())
}

/// Every class registration in the code sections. Anchor: the engine's
/// class-register call (found from any `mov ecx, <class id>` … `mov [rsp+0x20],
/// <parent>` … `call X` sequence — X is the same function for every class);
/// then every `call X` marks one registration, whose preceding bytes carry
/// `lea rdx,[rip+name]`, `mov r8d, size` (41 B8), the parent (`C7 44 24 20
/// imm32`, or `mov DWORD PTR [rsp+0x20],0` for a struct) and the class id
/// (`B9 imm32`; a struct's id is allocated at run time — `mov ecx,[rax]` — and
/// reads as 0 here).
pub fn find_classes(pe: &Pe) -> Vec<ClassInfo> {
    let mut out = Vec::new();
    let code: Vec<(usize, usize, u32)> = pe
        .secs
        .iter()
        .filter(|s| s.executable())
        .filter_map(|s| {
            let o0 = pe.off(s.va)?;
            let end = (o0 + s.rsize as usize).min(pe.d.len());
            Some((o0, end, s.va))
        })
        .collect();
    let call_target = |o: usize| -> Option<u32> {
        // the E8 at file offset o: its RVA target
        let rel = i32le(pe.d.get(o + 1..o + 5)?);
        let (o0, _, va) = code.iter().find(|(a, b, _)| o >= *a && o < *b)?;
        let next_rva = *va as i64 + (o as i64 - *o0 as i64) + 5;
        Some((next_rva + rel as i64) as u32)
    };
    // 1. the register function: the call that follows a class-id mov + parent store
    let mut register: Option<u32> = None;
    'find: for &(o0, end, _) in &code {
        let d = &pe.d[o0..end];
        let mut i = 0;
        while i + 40 < d.len() {
            if d[i] == 0xB9 {
                let cid = u32le(&d[i + 1..]);
                if cid & 0xFFF == 0 && cid >= 0x0100_0000 && cid < 0x4000_0000 {
                    let win = &d[i + 5..(i + 5 + 16).min(d.len())];
                    if win.windows(4).any(|w| w == [0xC7, 0x44, 0x24, 0x20]) {
                        let after = &d[i + 5..(i + 5 + 40).min(d.len())];
                        if let Some(p) = after.iter().position(|&b| b == 0xE8) {
                            if let Some(t) = call_target(o0 + i + 5 + p) {
                                register = Some(t);
                                break 'find;
                            }
                        }
                    }
                }
            }
            i += 1;
        }
    }
    let Some(register) = register else { return out };
    // 2. every call to it
    for &(o0, end, va) in &code {
        let d = &pe.d[o0..end];
        let mut i = 0;
        while i + 5 <= d.len() {
            if d[i] == 0xE8 && call_target(o0 + i) == Some(register) {
                let lo = i.saturating_sub(96);
                let back = &d[lo..i];
                let name = back.windows(3).rposition(|w| w == [0x48, 0x8D, 0x15]).and_then(|q| {
                    let rel = i32le(&back[q + 3..]);
                    let next_rva = va as i64 + (lo + q + 7) as i64;
                    cstr(pe, (next_rva + rel as i64) as u32)
                });
                let size = back.windows(2).rposition(|w| w == [0x41, 0xB8]).map(|q| u32le(&back[q + 2..])).unwrap_or(0);
                let parent = back.windows(4).rposition(|w| w == [0xC7, 0x44, 0x24, 0x20]).map(|q| u32le(&back[q + 4..])).unwrap_or(0);
                let class_id = back.windows(1).rposition(|w| w[0] == 0xB9).map(|q| u32le(&back[q + 1..])).filter(|c| c & 0xFFF == 0 && *c >= 0x0100_0000).unwrap_or(0);
                if let Some(name) = name {
                    out.push(ClassInfo { class_id, parent_id: parent, size, name, fn_rva: va + (i as u32), members: Vec::new() });
                }
                i += 5;
                continue;
            }
            i += 1;
        }
    }
    out
}

/// The members a class registration function (at `fn_rva`) registers, in
/// order, scanning until the function's `int3` padding.
pub fn members(pe: &Pe, fn_rva: u32) -> Vec<Member> {
    let mut out = Vec::new();
    let Some(o0) = pe.off(fn_rva) else { return out };
    let d = &pe.d;
    let mut i = o0;
    let limit = (o0 + 0x8000).min(d.len());
    while i + 12 < limit {
        // function end: int3 padding (two or more CC) after a ret/jmp
        if d[i] == 0xCC && d[i + 1] == 0xCC && i > o0 + 16 {
            break;
        }
        // BA imm32: mov edx, <offset>
        if d[i] == 0xBA {
            let offset = u32le(&d[i + 1..]);
            // the name lea within the next 40 bytes: 48 8D 0D rel32
            let win = &d[i + 5..(i + 5 + 40).min(d.len())];
            let name = win.windows(3).position(|w| w == [0x48, 0x8D, 0x0D]).and_then(|p| {
                let rel = i32le(&win[p + 3..]);
                let next_off = i + 5 + p + 7;
                let next_rva = fn_rva as i64 + (next_off as i64 - o0 as i64);
                let target = (next_rva + rel as i64) as u32;
                cstr(pe, target)
            });
            // the type fn: the nearest E8 call before (within 16 bytes)
            let lo = i.saturating_sub(16);
            let back = &d[lo..i];
            let type_fn = back.windows(1).rposition(|w| w[0] == 0xE8).map(|q| {
                let rel = i32le(&back[q + 1..]);
                let next_off = lo + q + 5;
                let next_rva = fn_rva as i64 + (next_off as i64 - o0 as i64);
                (pe.base as i64 + next_rva + rel as i64) as u64
            });
            if let (Some(name), Some(type_fn)) = (name, type_fn) {
                if offset < 0x10000 {
                    out.push(Member { offset, type_fn, name, at: fn_rva + (i - o0) as u32 });
                }
            }
            i += 5;
            continue;
        }
        i += 1;
    }
    out
}

/// `mapgeom exe-class --exe EXE [--types] [CLASSID|NAME]…`
pub fn run(args: &[String]) -> Result<(), String> {
    let mut exe = String::new();
    let mut want: Vec<String> = Vec::new();
    let mut types = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--exe" => {
                exe = args.get(i + 1).cloned().ok_or("--exe EXE")?;
                i += 2;
            }
            "--types" => {
                types = true;
                i += 1;
            }
            a => {
                want.push(a.to_string());
                i += 1;
            }
        }
    }
    if exe.is_empty() {
        return Err("exe-class --exe EXE [--types] CLASSID|NAME…".into());
    }
    let pe = Pe::open(&exe)?;
    let mut classes = find_classes(&pe);
    for c in classes.iter_mut() {
        c.members = members(&pe, c.fn_rva);
    }
    let mut out = String::new();
    if types {
        // name every type function by what it types, across all classes
        let mut by_fn: BTreeMap<u64, Vec<String>> = BTreeMap::new();
        for c in &classes {
            for m in &c.members {
                by_fn.entry(m.type_fn).or_default().push(format!("{}.{}", c.name, m.name));
            }
        }
        for (f, names) in &by_fn {
            let _ = writeln!(out, "type fn 0x{f:x}: {} members, e.g. {}", names.len(), names.iter().take(6).cloned().collect::<Vec<_>>().join(", "));
        }
    }
    if want.is_empty() {
        let _ = writeln!(out, "{} classes", classes.len());
        for c in &classes {
            let _ = writeln!(out, "0x{:08X} {:<40} parent 0x{:08X} size 0x{:x} members {} (fn exe+0x{:x})", c.class_id, c.name, c.parent_id, c.size, c.members.len(), c.fn_rva);
        }
    }
    for w in &want {
        let cid = u32::from_str_radix(w.trim_start_matches("0x"), 16).ok();
        let hits: Vec<&ClassInfo> = classes.iter().filter(|c| Some(c.class_id) == cid || c.name.eq_ignore_ascii_case(w)).collect();
        if hits.is_empty() {
            let _ = writeln!(out, "{w}: no class registration found");
        }
        for c in hits {
            let _ = writeln!(out, "class 0x{:08X} {} : parent 0x{:08X}, object size 0x{:x}, registration exe+0x{:x}, {} members", c.class_id, c.name, c.parent_id, c.size, c.fn_rva, c.members.len());
            for m in &c.members {
                let _ = writeln!(out, "  +0x{:<4x} {:<40} type fn 0x{:x}", m.offset, m.name, m.type_fn);
            }
        }
    }
    print!("{out}");
    Ok(())
}
