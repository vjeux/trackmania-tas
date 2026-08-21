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
