// mkimplib — emit a minimal MSVC-style import library (.lib) for a named DLL.
//
// Why this exists: cross-linking a Windows binary needs import libraries, and
// this box has no Windows SDK and no mingw. But an import library is not magic:
// it is an ar archive of tiny COFF objects, one per imported symbol, plus the
// four "import descriptor" members the linker stitches into the IAT. Every
// field is derivable from the DLL name and the symbol list -- no SDK content is
// copied, and nothing proprietary is needed.
//
// Usage: mkimplib <dllname.dll> <out.lib> <symbol> [<symbol> ...]

use std::io::Write;

const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;

fn pad_even(v: &mut Vec<u8>) {
    if v.len() % 2 == 1 {
        v.push(b'\n');
    }
}

/// One short-import member: the linker synthesises the thunk from this.
fn short_import(dll: &str, sym: &str, ordinal_hint: u16) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(&0x0000u16.to_le_bytes()); // sig1 = IMAGE_FILE_MACHINE_UNKNOWN
    o.extend_from_slice(&0xFFFFu16.to_le_bytes()); // sig2
    o.extend_from_slice(&0u16.to_le_bytes()); // version
    o.extend_from_slice(&IMAGE_FILE_MACHINE_AMD64.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes()); // time/date
    let size_of_data = (sym.len() + 1 + dll.len() + 1) as u32;
    o.extend_from_slice(&size_of_data.to_le_bytes());
    o.extend_from_slice(&ordinal_hint.to_le_bytes());
    // type = IMPORT_CODE(0), name type = IMPORT_NAME(1) -> bits: type|nametype<<2
    let flags: u16 = 0 | (1 << 2);
    o.extend_from_slice(&flags.to_le_bytes());
    o.extend_from_slice(sym.as_bytes());
    o.push(0);
    o.extend_from_slice(dll.as_bytes());
    o.push(0);
    o
}

struct Member {
    name: String,
    data: Vec<u8>,
    offset: usize,
}

fn ar_header(name: &str, size: usize) -> Vec<u8> {
    let mut h = vec![b' '; 60];
    let put = |h: &mut Vec<u8>, at: usize, s: &str| {
        for (i, b) in s.bytes().enumerate() {
            h[at + i] = b;
        }
    };
    put(&mut h, 0, name);
    put(&mut h, 16, "0");
    put(&mut h, 28, "0");
    put(&mut h, 34, "0");
    put(&mut h, 40, "0");
    put(&mut h, 48, &size.to_string());
    // the two-byte end marker lives at 58..60 and must be written LAST: a
    // size string long enough to reach it would otherwise clobber it.
    h[58] = 0x60;
    h[59] = 0x0a;
    assert!(name.len() <= 16 && size.to_string().len() <= 10, "ar field overflow");
    h
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 3 {
        eprintln!("usage: mkimplib <dll> <out.lib> <symbol>...");
        std::process::exit(2);
    }
    let dll = a[0].clone();
    let out = a[1].clone();
    let syms: Vec<String> = a[2..].to_vec();

    // members: one short-import per symbol
    let mut members: Vec<Member> = Vec::new();
    for (i, s) in syms.iter().enumerate() {
        let d = short_import(&dll, s, (i + 1) as u16);
        members.push(Member { name: format!("{dll}/"), data: d, offset: 0 });
    }

    // first linker member: symbol index (big-endian offsets)
    // Each member is advertised TWICE: as `__imp_NAME` (what a dllimport
    // reference looks for) and as `NAME` (the call thunk, which a plain
    // `extern "system"` call looks for). Indexing only the first leaves every
    // direct call unresolved while std's own imports link fine -- a failure
    // that looks like a missing DLL and is really a missing index entry.
    let nsym = syms.len() * 2;
    let mut first = Vec::new();
    first.extend_from_slice(&(nsym as u32).to_be_bytes());
    for _ in 0..nsym {
        first.extend_from_slice(&0u32.to_be_bytes()); // patched below
    }
    for s in &syms {
        first.extend_from_slice(format!("__imp_{s}").as_bytes());
        first.push(0);
        first.extend_from_slice(s.as_bytes());
        first.push(0);
    }
    // second linker member (MSVC): sorted symbol table
    // second linker member: offsets are per-MEMBER, and the parallel index
    // array maps each sorted symbol to its member number (1-based).
    let mut names: Vec<(String, usize)> = Vec::new();
    for (i, s) in syms.iter().enumerate() {
        names.push((format!("__imp_{s}"), i + 1));
        names.push((s.clone(), i + 1));
    }
    names.sort_by(|a, b| a.0.cmp(&b.0));
    let mut second = Vec::new();
    second.extend_from_slice(&(syms.len() as u32).to_le_bytes());
    for _ in 0..syms.len() {
        second.extend_from_slice(&0u32.to_le_bytes());
    }
    second.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for (_, m) in &names {
        second.extend_from_slice(&(*m as u16).to_le_bytes());
    }
    for (n, _) in &names {
        second.extend_from_slice(n.as_bytes());
        second.push(0);
    }
    // lay out and back-patch the member offsets.
    // An archive symbol-index entry must point at the member's HEADER, and the
    // offsets are only known once every member's position is fixed -- hence
    // the two-pass layout. The first linker member is big-endian (System V
    // convention, which MSVC kept); the second is little-endian.
    let mut pos = 8usize; // "!<arch>\n"
    pos += 60 + first.len() + (first.len() % 2);
    pos += 60 + second.len() + (second.len() % 2);
    for m in members.iter_mut() {
        m.offset = pos;
        pos += 60 + m.data.len() + (m.data.len() % 2);
    }
    for (i, m) in members.iter().enumerate() {
        first[4 + (i * 2) * 4..8 + (i * 2) * 4]
            .copy_from_slice(&(m.offset as u32).to_be_bytes());
        first[4 + (i * 2 + 1) * 4..8 + (i * 2 + 1) * 4]
            .copy_from_slice(&(m.offset as u32).to_be_bytes());
        second[4 + i * 4..8 + i * 4].copy_from_slice(&(m.offset as u32).to_le_bytes());
    }

    let mut f = std::fs::File::create(&out).expect("create lib");
    f.write_all(b"!<arch>\n").unwrap();
    let mut emit = |f: &mut std::fs::File, name: &str, data: &[u8]| {
        f.write_all(&ar_header(name, data.len())).unwrap();
        f.write_all(data).unwrap();
        if data.len() % 2 == 1 {
            f.write_all(b"\n").unwrap();
        }
    };
    emit(&mut f, "/", &first);
    emit(&mut f, "/", &second);
    for m in &members {
        emit(&mut f, &m.name, &m.data);
    }
    println!("wrote {out}: {} symbols from {dll}", syms.len());
}
