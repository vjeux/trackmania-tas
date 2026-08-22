//! Minimal GBX container split/rebuild. LZO1X via liblzo2 (decompress only --
//! candidates are written with an uncompressed body, which the dedicated
//! server accepts and which skips LZO entirely).
//!
//! PROVENANCE: copied verbatim from `/tmp/tmsearch/src/gbx.rs` (the input-search
//! crate) as instructed, with two local changes:
//!   * `lzo_init` is now idempotent and thread-safe (`OnceLock`) because
//!     `tmtraj decode-all` decompresses ghosts from many threads at once;
//!   * `all_skip_chunks` is unused here and kept only so the file stays a
//!     drop-in copy of the original.
//! It is the Rust equivalent of `tmtas/oracle/gbx.py` (`Gbx.__init__`,
//! `_parse_ref_table`, `header_bytes`).

use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::sync::OnceLock;

// liblzo2 is loaded at runtime with dlopen rather than linked, so the search
// binary runs on any box with the runtime library present -- no -dev package,
// no `liblzo2.so` development symlink.
const RTLD_NOW: c_int = 2;

extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type InitFn = unsafe extern "C" fn(
    c_uint,
    c_int,
    c_int,
    c_int,
    c_int,
    c_int,
    c_int,
    c_int,
    c_int,
    c_int,
) -> c_int;
pub type DecompFn =
    unsafe extern "C" fn(*const u8, usize, *mut u8, *mut usize, *mut c_void) -> c_int;

static DECOMPRESS: OnceLock<usize> = OnceLock::new();

/// Resolve `lzo1x_decompress_safe` once, on first use. Safe to call from any
/// number of threads; every later call is a plain atomic load.
pub fn lzo_init() -> DecompFn {
    let addr = *DECOMPRESS.get_or_init(|| unsafe {
        let mut h = std::ptr::null_mut();
        for name in [b"liblzo2.so.2\0".as_ref(), b"liblzo2.so\0".as_ref()] {
            h = dlopen(name.as_ptr() as *const c_char, RTLD_NOW);
            if !h.is_null() {
                break;
            }
        }
        assert!(!h.is_null(), "cannot dlopen liblzo2");
        let init = dlsym(h, b"__lzo_init_v2\0".as_ptr() as *const c_char);
        assert!(!init.is_null(), "no __lzo_init_v2 in liblzo2");
        let init: InitFn = std::mem::transmute(init);
        init(1, -1, -1, -1, -1, -1, -1, -1, -1, -1);
        let d = dlsym(h, b"lzo1x_decompress_safe\0".as_ptr() as *const c_char);
        assert!(!d.is_null(), "no lzo1x_decompress_safe in liblzo2");
        d as usize
    });
    unsafe { std::mem::transmute::<usize, DecompFn>(addr) }
}

pub fn lzo_decompress(src: &[u8], out_len: usize) -> Vec<u8> {
    let f = lzo_init();
    let mut dst = vec![0u8; out_len];
    let mut dl = out_len;
    let r = unsafe {
        f(
            src.as_ptr(),
            src.len(),
            dst.as_mut_ptr(),
            &mut dl,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(r, 0, "lzo1x_decompress_safe -> {}", r);
    dst.truncate(dl);
    dst
}

pub struct Reader<'a> {
    pub b: &'a [u8],
    pub o: usize,
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Reader { b, o: 0 }
    }
    pub fn u8(&mut self) -> u8 {
        let v = self.b[self.o];
        self.o += 1;
        v
    }
    pub fn u16(&mut self) -> u16 {
        let v = u16::from_le_bytes(self.b[self.o..self.o + 2].try_into().unwrap());
        self.o += 2;
        v
    }
    pub fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.b[self.o..self.o + 4].try_into().unwrap());
        self.o += 4;
        v
    }
    pub fn i32(&mut self) -> i32 {
        self.u32() as i32
    }
    pub fn skip(&mut self, n: usize) {
        self.o += n;
    }
    pub fn string(&mut self) {
        let n = self.u32() as usize;
        self.skip(n);
    }
}

pub struct Gbx {
    pub version: u16,
    pub format: u8,
    pub ref_comp: u8,
    pub unknown: Option<u8>,
    pub class_id: u32,
    pub user_data: Vec<u8>,
    pub num_nodes: u32,
    pub ref_table: Vec<u8>,
    pub body: Vec<u8>,
}

impl Gbx {
    pub fn parse(data: &[u8]) -> Gbx {
        let mut r = Reader::new(data);
        assert_eq!(&data[0..3], b"GBX", "not a GBX file");
        r.skip(3);
        let version = r.u16();
        let format = r.u8();
        let ref_comp = r.u8();
        let body_comp = r.u8();
        let unknown = if version >= 4 { Some(r.u8()) } else { None };
        let class_id = r.u32();
        let mut user_data = Vec::new();
        if version >= 6 {
            let n = r.u32() as usize;
            user_data = data[r.o..r.o + n].to_vec();
            r.skip(n);
        }
        let num_nodes = r.u32();
        let ref_start = r.o;
        parse_ref_table(&mut r, version);
        let ref_table = data[ref_start..r.o].to_vec();
        let body = if body_comp == b'C' {
            let uncomp = r.u32() as usize;
            let csize = r.u32() as usize;
            lzo_decompress(&data[r.o..r.o + csize], uncomp)
        } else {
            data[r.o..].to_vec()
        };
        Gbx {
            version,
            format,
            ref_comp,
            unknown,
            class_id,
            user_data,
            num_nodes,
            ref_table,
            body,
        }
    }

    /// Header for an uncompressed-body file (`'U'`).
    pub fn header_bytes_u(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + self.user_data.len() + self.ref_table.len());
        out.extend_from_slice(b"GBX");
        out.extend_from_slice(&self.version.to_le_bytes());
        out.push(self.format);
        out.push(self.ref_comp);
        out.push(b'U');
        if let Some(u) = self.unknown {
            out.push(u);
        }
        out.extend_from_slice(&self.class_id.to_le_bytes());
        if self.version >= 6 {
            out.extend_from_slice(&(self.user_data.len() as u32).to_le_bytes());
            out.extend_from_slice(&self.user_data);
        }
        out.extend_from_slice(&self.num_nodes.to_le_bytes());
        out.extend_from_slice(&self.ref_table);
        out
    }
}

fn parse_ref_table(r: &mut Reader, version: u16) {
    let n = r.u32();
    if n == 0 {
        return;
    }
    r.u32(); // ancestorLevel
    let nfolders = r.u32();
    fn folders(r: &mut Reader, cnt: u32) {
        for _ in 0..cnt {
            r.string();
            let sub = r.u32();
            folders(r, sub);
        }
    }
    folders(r, nfolders);
    for _ in 0..n {
        let flags = r.u32();
        if flags & 4 == 0 {
            r.string();
        } else {
            r.u32();
        }
        r.u32(); // nodeIndex
        if version >= 5 {
            r.u32(); // useFile
        }
        if flags & 4 == 0 {
            r.u32(); // folderIndex
        }
    }
}

pub const SKIP_MAGIC: &[u8; 4] = b"PIKS";

/// Every skippable chunk in a body: (chunk_id, offset, payload_offset, size).
pub fn all_skip_chunks(body: &[u8]) -> Vec<(u32, usize, usize, usize)> {
    let mut out = Vec::new();
    let n = body.len();
    let mut i = 0usize;
    while i + 12 <= n {
        if &body[i + 4..i + 8] == SKIP_MAGIC {
            let cid = u32::from_le_bytes(body[i..i + 4].try_into().unwrap());
            let size = u32::from_le_bytes(body[i + 8..i + 12].try_into().unwrap()) as usize;
            let top = cid >> 24;
            if matches!(top, 0x03 | 0x0B | 0x24 | 0x2E | 0x30) && i + 12 + size <= n {
                out.push((cid, i, i + 12, size));
                i += 12 + size;
                continue;
            }
        }
        i += 1;
    }
    out
}
// Container-level facts about a `.Ghost.Gbx` / `.Replay.Gbx`: which chunks it
// carries, which map it will actually run on, what time it declares, and which
// identity strings are in it.

use crate::map_uid_of;
// all_skip_chunks / Gbx are defined above in this module.

/// `CGameCtnChallenge` embedded inside a replay's body.
pub const EMBEDDED_MAP_CHUNK: u32 = 0x03093002;
pub const RACE_TIME_CHUNK: u32 = 0x03092005;
pub const SPLITS_CHUNK: u32 = 0x0309202B;

pub struct Container {
    pub path: String,
    pub gbx: Gbx,
}

impl Container {
    pub fn load(path: &str) -> Result<Container, String> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
        if data.len() < 16 || &data[0..3] != b"GBX" {
            return Err(format!("{}: not a GBX file", path));
        }
        Ok(Container { path: path.into(), gbx: Gbx::parse(&data) })
    }

    pub fn body(&self) -> &[u8] {
        &self.gbx.body
    }

    pub fn chunks(&self) -> Vec<(u32, usize, usize, usize)> {
        all_skip_chunks(&self.gbx.body)
    }

    /// The `CGameCtnChallenge` the file carries, if any: (payload offset, size).
    ///
    /// THE MAP IS INSIDE THE REPLAY. When this returns `Some`, the dedicated
    /// server simulates THIS copy and every `--map` argument, every
    /// `UserData/Maps` entry and the uid in the header are decoration.
    ///
    /// The chunk is NOT skippable -- there is no `PIKS` marker and no chunk
    /// table entry: it is the id, a size word, and a whole nested GBX file. A
    /// scan that only knows skippable chunks reports "no embedded map" on every
    /// real replay, which is the most expensive way to get this wrong.
    pub fn embedded_map(&self) -> Option<(usize, usize)> {
        let b = &self.gbx.body;
        let pat = EMBEDDED_MAP_CHUNK.to_le_bytes();
        let mut i = 0usize;
        while i + 12 <= b.len() {
            if b[i..i + 4] == pat {
                let size = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
                if size > 1024 && i + 8 + size <= b.len() && &b[i + 8..i + 11] == b"GBX" {
                    return Some((i + 8, size));
                }
            }
            i += 1;
        }
        None
    }

    pub fn embedded_map_bytes(&self) -> Option<Vec<u8>> {
        self.embedded_map().map(|(o, n)| self.gbx.body[o..o + n].to_vec())
    }

    /// Every copy of the declared race time in the body: (offset, value ms).
    /// A container built on a borrowed carrier has been caught with the
    /// carrier's value in one of these and its own in the rest.
    pub fn declared_times(&self) -> Vec<(usize, u32)> {
        self.chunks()
            .into_iter()
            .filter(|c| c.0 == RACE_TIME_CHUNK)
            .map(|c| (c.2, u32::from_le_bytes(self.gbx.body[c.2..c.2 + 4].try_into().unwrap())))
            .collect()
    }

    /// The checkpoint split vector from `0x0309202B`.
    pub fn splits(&self) -> Vec<u32> {
        match self.chunks().into_iter().find(|c| c.0 == SPLITS_CHUNK) {
            None => Vec::new(),
            Some(c) => (0..c.3 / 4)
                .map(|k| {
                    u32::from_le_bytes(
                        self.gbx.body[c.2 + 4 * k..c.2 + 4 * k + 4].try_into().unwrap(),
                    )
                })
                .collect(),
        }
    }

    /// Every 27-character uid-shaped literal in the file, with its offset.
    pub fn uids(&self) -> Vec<(usize, String)> {
        let b = &self.gbx.body;
        let mut out = Vec::new();
        let mut i = 0usize;
        while i + 31 <= b.len() {
            if u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) == 27 {
                if let Ok(s) = std::str::from_utf8(&b[i + 4..i + 31]) {
                    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                        out.push((i, s.to_string()));
                    }
                }
            }
            i += 1;
        }
        out
    }
}

/// A length-prefixed ASCII string found in a body, with the offset of its
/// length word.
#[derive(Clone, Debug)]
pub struct BodyStr {
    pub at: usize,
    pub len: usize,
    pub s: String,
}

/// Walk the length-prefixed strings at the front of a ghost body.
///
/// The identity lives in the ghost's own chunks before the compressed record;
/// scanning past that finds bytes that only look like strings, so the walk is
/// bounded and it never crosses into the record blob.
/// Walk the length-prefixed strings in a byte range.
///
/// Strings here are UTF-8, not ASCII: a display name routinely carries a BOM,
/// `$`-colour codes and circled letters, and an ASCII-only walk silently misses
/// exactly the field a rename is supposed to change.
pub fn body_strings_in(body: &[u8], from: usize, to: usize) -> Vec<BodyStr> {
    let lim = body.len().min(to);
    let mut out = Vec::new();
    let mut p = from;
    while p + 4 <= lim {
        let n = u32::from_le_bytes(body[p..p + 4].try_into().unwrap()) as usize;
        if n > 0 && n < 512 && p + 4 + n <= body.len() {
            if let Ok(s) = std::str::from_utf8(&body[p + 4..p + 4 + n]) {
                if !s.is_empty()
                    && s.chars().all(|c| !c.is_control())
                    && s.chars().any(|c| c.is_alphanumeric() || c == '\\' || c == '|')
                {
                    out.push(BodyStr { at: p, len: n, s: s.to_string() });
                    p += 4 + n;
                    continue;
                }
            }
        }
        p += 1;
    }
    out
}

/// Replace a set of length-prefixed strings in a body, fixing the size of every
/// skippable chunk that encloses each edit. Edits must be sorted by offset and
/// must not overlap.
pub fn replace_strings(
    body: &[u8],
    edits: &[(usize, usize, Vec<u8>)],
    protect: Option<(usize, usize)>,
) -> Result<Vec<u8>, String> {
    // A replay carries a whole MAP inside its body, and that map has chunk
    // headers of its own. One of them (`0x0304305F`, the free-block table)
    // declares a size that runs PAST the end of the carried map, so a naive
    // chunk walk reports it as framing the replay's own strings -- and
    // "correcting" its size writes into the map. Measured: that alone turned a
    // replay that validates at 7.241 into one that validates at nothing, while
    // every string still read back correctly. So the carried map is protected:
    // no chunk inside it frames anything, and nothing inside it is edited.
    let (plo, phi) = protect.unwrap_or((usize::MAX, usize::MAX));
    let chunks: Vec<_> = all_skip_chunks(body)
        .into_iter()
        .filter(|(_, coff, poff, sz)| *coff >= phi || poff + sz <= plo)
        .collect();
    // A LENGTH CHANGE MOVES EVERY BYTE AFTER IT. That is only safe when some
    // skippable chunk frames the edit and can have its size corrected; a string
    // written inline -- which is where a REPLAY keeps its driver's name -- has
    // no size word anywhere to fix.
    let mut unframed: Vec<String> = Vec::new();
    for (at, old_len, new) in edits {
        if *at >= plo && *at < phi {
            return Err(format!("edit at {} is inside the carried map; refusing", at));
        }
        if new.len() == *old_len {
            continue;
        }
        let framed = chunks.iter().any(|(_, _, poff, sz)| *at >= *poff && at + 4 + old_len <= poff + sz);
        if !framed {
            unframed.push(format!("offset {} ({} B -> {} B)", at, old_len, new.len()));
        }
    }
    // Not fatal by itself: a body is parsed serially, so a top-level inline
    // chunk's string can shrink safely. It is fatal when nothing can prove it,
    // so the caller is told and must let the plain oracle decide.
    UNFRAMED.with(|u| *u.borrow_mut() = unframed);
    let mut out: Vec<u8> = Vec::with_capacity(body.len() + 256);
    let mut last = 0usize;
    for (at, old_len, new) in edits {
        if *at < last {
            return Err("overlapping or unsorted string edits".into());
        }
        out.extend_from_slice(&body[last..*at]);
        out.extend_from_slice(&(new.len() as u32).to_le_bytes());
        out.extend_from_slice(new);
        last = at + 4 + old_len;
    }
    out.extend_from_slice(&body[last..]);
    // fix every enclosing chunk size
    for (_cid, coff, poff, sz) in chunks {
        let mut delta = 0i64;
        let mut shift = 0i64;
        for (at, old_len, new) in edits {
            let d = new.len() as i64 - *old_len as i64;
            if *at >= poff && *at < poff + sz {
                delta += d;
            }
            if *at < coff {
                shift += d;
            }
        }
        if delta != 0 {
            let nsz = (sz as i64 + delta) as u32;
            let o = (coff as i64 + shift) as usize;
            out[o + 8..o + 12].copy_from_slice(&nsz.to_le_bytes());
        }
    }
    Ok(out)
}

pub fn write_gbx(g: &Gbx, body: Vec<u8>, out: &str) -> Result<(), String> {
    // Always write an UNCOMPRESSED body. The dedicated server accepts it, the
    // game accepts it, and it keeps every write path free of an LZO compressor
    // whose output is not bit-reproducible -- which matters, because half the
    // controls here are byte comparisons.
    let mut file = g.header_bytes_u();
    file.extend_from_slice(&body);
    std::fs::write(out, file).map_err(|e| format!("{}: {}", out, e))
}

/// Seconds, with a decimal, from milliseconds. Times are reported this way
/// everywhere in this project.
pub fn secs(ms: i64) -> String {
    let neg = ms < 0;
    let v = ms.abs();
    format!("{}{}.{:03}", if neg { "-" } else { "" }, v / 1000, v % 1000)
}

thread_local! {
    /// Length-changing edits the last `replace_strings` could not frame. A body
    /// is parsed serially, so a top-level inline chunk's string CAN shrink
    /// safely -- measured: clearing a ghost's 22-byte account id in the inline
    /// chunk `0x0309200F` leaves a file that still validates to the same
    /// millisecond. But nothing in the file proves it, so the caller has to let
    /// the plain oracle decide rather than assume either way.
    pub static UNFRAMED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn unframed_edits() -> Vec<String> {
    UNFRAMED.with(|u| u.borrow().clone())
}
pub fn set_embedded_map(c: &Container, newmap: &[u8], newuid: &str) -> Result<Vec<u8>, String> {
    let (off, size) = c.embedded_map().ok_or(
        "this file carries no embedded map. It is a pure ghost: it is bound to a map by the uid \
         it declares, so use `ghost map rebind` and put the target map in the server's \
         UserData/Maps.",
    )?;
    let body = c.body();
    let olduid = map_uid_of(&body[off..off + size]);
    let mut out = Vec::with_capacity(body.len() + newmap.len());
    out.extend_from_slice(&body[..off]);
    out.extend_from_slice(newmap);
    out.extend_from_slice(&body[off + size..]);
    // The size word sits immediately in front of the nested file.
    out[off - 4..off].copy_from_slice(&(newmap.len() as u32).to_le_bytes());
    // rewrite the uid literals that named the old map, OUTSIDE the map we just
    // pasted in (which carries its own). TM2020 uids are 27 characters, so this
    // is length preserving.
    if let Some(old) = olduid {
        if old.len() == newuid.len() {
            let nb = newuid.as_bytes();
            let ob = old.as_bytes();
            let newmap_end = off + newmap.len();
            let mut i = 0usize;
            while i + 4 + ob.len() <= out.len() {
                if i >= off && i < newmap_end {
                    i = newmap_end;
                    continue;
                }
                if u32::from_le_bytes(out[i..i + 4].try_into().unwrap()) as usize == ob.len()
                    && &out[i + 4..i + 4 + ob.len()] == ob
                {
                    out[i + 4..i + 4 + ob.len()].copy_from_slice(nb);
                    i += 4 + ob.len();
                    continue;
                }
                i += 1;
            }
        }
    }
    Ok(out)
}

