//! Minimal GBX container split/rebuild + LZO1X, ported from the project's
//! Python `gbx.py` (and `/tmp/tmsearch/src/gbx.rs`, decompress-only).
//!
//! A .Gbx file is:  "GBX" | version u16 | format u8 | refTableComp u8 |
//! bodyComp u8 | [unknown u8 if version>=4] | classId u32 |
//! [userData len u32 + bytes if version>=6] | numNodes u32 | refTable |
//! body (LZO1X-compressed when bodyComp == 'C': uncompSize u32, compSize u32,
//! payload).
//!
//! Maps MUST be written back compressed ('C'): unlike ghosts, the dedicated
//! server refuses a map with an uncompressed body ("Can't load map").
//! Measured, see `tests`.

use std::os::raw::{c_char, c_int, c_uint, c_void};

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
type DecompFn =
    unsafe extern "C" fn(*const u8, usize, *mut u8, *mut usize, *mut c_void) -> c_int;
type CompFn =
    unsafe extern "C" fn(*const u8, usize, *mut u8, *mut usize, *mut c_void) -> c_int;

static mut DECOMPRESS: Option<DecompFn> = None;
static mut COMPRESS: Option<CompFn> = None;
static INIT: std::sync::Once = std::sync::Once::new();

const LZO1X_1_MEM_COMPRESS: usize = 16384 * 8;

pub fn lzo_init() {
    INIT.call_once(|| unsafe {
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
        assert!(!d.is_null(), "no lzo1x_decompress_safe");
        DECOMPRESS = Some(std::mem::transmute::<*mut c_void, DecompFn>(d));
        let c = dlsym(h, b"lzo1x_1_compress\0".as_ptr() as *const c_char);
        assert!(!c.is_null(), "no lzo1x_1_compress");
        COMPRESS = Some(std::mem::transmute::<*mut c_void, CompFn>(c));
    });
}

pub fn lzo_decompress(src: &[u8], out_len: usize) -> Vec<u8> {
    lzo_init();
    let f = unsafe { DECOMPRESS }.expect("lzo_init");
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

pub fn lzo_compress(src: &[u8]) -> Vec<u8> {
    lzo_init();
    let f = unsafe { COMPRESS }.expect("lzo_init");
    let cap = src.len() + src.len() / 16 + 64 + 3;
    let mut dst = vec![0u8; cap];
    let mut dl = cap;
    let mut wrk = vec![0u8; LZO1X_1_MEM_COMPRESS];
    let r = unsafe {
        f(
            src.as_ptr(),
            src.len(),
            dst.as_mut_ptr(),
            &mut dl,
            wrk.as_mut_ptr() as *mut c_void,
        )
    };
    assert_eq!(r, 0, "lzo1x_1_compress -> {}", r);
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
    pub fn at(b: &'a [u8], o: usize) -> Self {
        Reader { b, o }
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
    #[allow(dead_code)]
    pub fn i32(&mut self) -> i32 {
        self.u32() as i32
    }
    pub fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    pub fn peek_u32(&self) -> u32 {
        u32::from_le_bytes(self.b[self.o..self.o + 4].try_into().unwrap())
    }
    pub fn skip(&mut self, n: usize) {
        self.o += n;
    }
    pub fn bytes(&mut self, n: usize) -> &'a [u8] {
        let v = &self.b[self.o..self.o + n];
        self.o += n;
        v
    }
    pub fn string(&mut self) -> String {
        let n = self.u32() as usize;
        String::from_utf8_lossy(self.bytes(n)).into_owned()
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
    /// The file's OWN compressed stream, exactly as it was read. Kept so an
    /// edit can be spliced into it rather than recompressed over it — see
    /// `splice.rs` and `build_spliced`. `None` for a body that was stored
    /// uncompressed.
    pub comp: Option<Vec<u8>>,
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
        let mut comp: Option<Vec<u8>> = None;
        let num_nodes = r.u32();
        let ref_start = r.o;
        parse_ref_table(&mut r, version);
        let ref_table = data[ref_start..r.o].to_vec();
        let body = if body_comp == b'C' {
            let uncomp = r.u32() as usize;
            let csize = r.u32() as usize;
            comp = Some(data[r.o..r.o + csize].to_vec());
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
            comp,
        }
    }

    pub fn load(path: &std::path::Path) -> std::io::Result<Gbx> {
        Ok(Gbx::parse(&std::fs::read(path)?))
    }

    fn header_bytes(&self, body_comp: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + self.user_data.len() + self.ref_table.len());
        out.extend_from_slice(b"GBX");
        out.extend_from_slice(&self.version.to_le_bytes());
        out.push(self.format);
        out.push(self.ref_comp);
        out.push(body_comp);
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

    /// Write this container back out with `new_body` as its body.
    ///
    /// The body is **spliced into the file's own compressed stream** wherever
    /// the edit allows, so an edited map stays byte-identical to the stock file
    /// everywhere the edit did not reach. See `splice.rs` for the methods and
    /// for the verification every one of them goes through. `numNodes` and the
    /// refTable are carried over untouched — a byte patcher never changes the
    /// node count, which is precisely the bug that cost the Python version a
    /// day.
    ///
    /// Maps are always written compressed: the dedicated server refuses a map
    /// with a `'U'` body ("Can't load map").
    pub fn write_body(&self, new_body: &[u8]) -> (Vec<u8>, crate::splice::Spliced) {
        let sp = match &self.comp {
            Some(stock) => crate::splice::splice(stock, &self.body, new_body),
            // A map read from an uncompressed container has no stock stream to
            // splice into; it gets one built for it.
            None => {
                let stream = lzo_compress(new_body);
                crate::splice::Spliced {
                    stream,
                    method: crate::splice::Method::Reemit,
                    shared_prefix: 0,
                    shared_suffix: 0,
                    stock_len: 0,
                    changed_bytes: 0,
                }
            }
        };
        let out = self.file_with_stream(new_body, &sp.stream);
        (out, sp)
    }

    /// The whole file: this container's header, then `stream` declared as the
    /// compressed form of a body `body_len` long.
    pub fn file_with_stream(&self, new_body: &[u8], stream: &[u8]) -> Vec<u8> {
        let mut out = self.header_bytes(b'C');
        out.extend_from_slice(&(new_body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
        out.extend_from_slice(stream);
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
