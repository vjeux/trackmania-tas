use crate::blowfish::PakCipher;
use crate::md5;

pub const HEADER_KEY: [u8; 16] = [
    0x56, 0xee, 0xcb, 0xbb, 0xde, 0xb6, 0xbc, 0x90, 0xa1, 0x7d, 0xfc, 0xeb, 0x76, 0x1d, 0x59, 0xce,
];

/// A sequential reader over the blowfish-decrypted header stream.
pub struct CipherReader<'a> {
    data: &'a [u8],
    pos: usize,
    cipher: PakCipher,
}

impl<'a> CipherReader<'a> {
    pub fn new(data: &'a [u8], off: usize, key: &[u8; 16], version: i32) -> CipherReader<'a> {
        let iv = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        CipherReader {
            data,
            pos: off + 8,
            cipher: PakCipher::new(key, iv, version),
        }
    }
    pub fn take(&mut self, n: usize) -> Vec<u8> {
        let mut out = vec![0u8; n];
        let data = self.data;
        let pos = &mut self.pos;
        self.cipher.read(&mut out, |b| {
            let t = b.len().min(data.len() - *pos);
            b[..t].copy_from_slice(&data[*pos..*pos + t]);
            *pos += t;
            t
        });
        out
    }
    pub fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    pub fn i32(&mut self) -> i32 {
        self.u32() as i32
    }
    pub fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    pub fn u128(&mut self) -> u128 {
        u128::from_le_bytes(self.take(16).try_into().unwrap())
    }
    pub fn string(&mut self) -> String {
        let n = self.u32() as usize;
        assert!(n < 1 << 20, "absurd string length {}", n);
        String::from_utf8_lossy(&self.take(n)).to_string()
    }
    pub fn initialize(&mut self, data: &[u8], off: usize, count: usize) {
        self.cipher.initialize(data, off, count);
    }
}

#[derive(Clone, Debug)]
pub struct PakEntry {
    pub name: String,
    pub folder: String,
    pub class_id: u32,
    pub offset: u32,
    pub uncompressed_size: i32,
    pub compressed_size: i32,
    pub size: i32,
    pub flags: u64,
}

impl PakEntry {
    pub fn is_compressed(&self) -> bool {
        self.flags & 0x3C != 0
    }
    pub fn public_file(&self) -> bool {
        self.flags & 0x2000000000000 != 0
    }
    pub fn force_no_crypt(&self) -> bool {
        self.flags & 0x4000000000000 != 0
    }
    pub fn is_encrypted(&self) -> bool {
        !self.force_no_crypt() && !self.public_file()
    }
    pub fn path(&self) -> String {
        if self.folder.is_empty() {
            self.name.clone()
        } else {
            format!("{}\\{}", self.folder, self.name)
        }
    }
}

pub struct Pak {
    pub version: i32,
    pub header_max_size: i64,
    pub gbx_headers_start: u32,
    pub gbx_headers_size: i32,
    pub gbx_headers_compr_size: i32,
    pub size: u32,
    pub flags: u32,
    pub folders: Vec<(String, i32)>,
    pub entries: Vec<PakEntry>,
    pub key: [u8; 16],
}

/// Decrypt and parse the private header of a v6+ pak, given the encryption key.
pub fn read_pak(data: &[u8], enc_start: usize, version: i32, key: &[u8; 16]) -> Pak {
    let mut kh = *key;
    for i in 0..16 {
        kh[i] ^= HEADER_KEY[i];
    }
    let mut r = CipherReader::new(data, enc_start, &kh, version);
    let _md5 = r.take(16);
    let gbx_headers_start = r.u32();
    if version < 15 {
        let _hms = r.i32();
    }
    let gbx_headers_size = r.i32();
    let gbx_headers_compr_size = r.i32();
    let mut size = 0u32;
    if version >= 14 {
        let _unused = r.take(16);
        if version >= 16 {
            size = r.u32();
        }
    }
    let _unused2 = r.take(16);
    let flags = r.u32();

    let num_folders = r.i32();
    let mut folders: Vec<(String, i32)> = Vec::new();
    for _ in 0..num_folders {
        let parent = r.i32();
        let name = r.string();
        folders.push((name, parent));
    }
    // the third folder's name (UTF-16) perturbs the cipher
    if folders.len() > 2 && folders[2].0.len() > 4 {
        let utf16: Vec<u8> = folders[2]
            .0
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        r.initialize(&utf16, 4, 4);
    }

    let num_files = r.i32();
    let mut entries = Vec::new();
    for _ in 0..num_files {
        let folder_index = r.i32();
        let name = r.string();
        let _u01 = r.i32();
        let uncompressed_size = r.i32();
        let compressed_size = r.i32();
        let offset = r.u32();
        let class_id = r.u32();
        let size_f = if version >= 17 { r.i32() } else { 0 };
        if version >= 14 {
            let _checksum = r.u128();
        }
        let flags = r.u64();
        entries.push(PakEntry {
            name,
            folder: folder_path(folder_index, &folders),
            class_id,
            offset,
            uncompressed_size,
            compressed_size,
            size: size_f,
            flags,
        });
    }

    Pak {
        version,
        header_max_size: 0,
        gbx_headers_start,
        gbx_headers_size,
        gbx_headers_compr_size,
        size,
        flags,
        folders,
        entries,
        key: *key,
    }
}

fn folder_path(mut idx: i32, folders: &[(String, i32)]) -> String {
    let mut parts = Vec::new();
    while idx >= 0 {
        let f = &folders[idx as usize];
        parts.push(f.0.trim_end_matches(['\\', '/']).to_string());
        idx = f.1;
    }
    parts.reverse();
    parts.join("\\")
}

pub fn key_hex(k: &[u8; 16]) -> String {
    md5::hex_upper(k)
}
