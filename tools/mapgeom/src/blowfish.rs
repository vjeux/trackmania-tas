// Blowfish as Nadeo uses it in NadeoPak files, transcribed from
// GBX.NET.Crypto/Blowfish.cs + GBX.NET.PAK/BlowfishStream.cs (MIT).
use crate::tables::{P_INIT, S_INIT};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trick {
    LittleEndian,
    LittleEndianPak18,
}

#[derive(Clone)]
pub struct Blowfish {
    p: [u32; 18],
    s: [[u32; 256]; 4],
    n: usize,
}

impl Blowfish {
    pub fn new(key: &[u8], trick: Trick) -> Blowfish {
        let n = if trick == Trick::LittleEndianPak18 { 8 } else { 16 };
        let mut bf = Blowfish { p: P_INIT, s: S_INIT, n };
        let mut j = 0usize;
        for i in 0..(n + 2) {
            let mut data: u32 = 0;
            for k in 0..4u32 {
                data |= (key[j] as u32) << (k * 8);
                j += 1;
                if j >= key.len() {
                    j = 0;
                }
            }
            bf.p[i] ^= data;
        }
        let (mut l, mut r) = (0u32, 0u32);
        let mut i = 0;
        while i < n + 2 {
            bf.encrypt_pair(&mut l, &mut r);
            bf.p[i] = l;
            bf.p[i + 1] = r;
            i += 2;
        }
        for i in 0..4 {
            let mut j = 0;
            while j < 256 {
                bf.encrypt_pair(&mut l, &mut r);
                bf.s[i][j] = l;
                bf.s[i][j + 1] = r;
                j += 2;
            }
        }
        if trick == Trick::LittleEndianPak18 {
            bf.p[0..10].reverse();
        }
        bf
    }

    #[inline(always)]
    fn f(&self, x: u32) -> u32 {
        let a = (x >> 24) as usize;
        let b = ((x >> 16) & 0xff) as usize;
        let c = ((x >> 8) & 0xff) as usize;
        let d = (x & 0xff) as usize;
        ((self.s[0][a].wrapping_add(self.s[1][b])) ^ self.s[2][c]).wrapping_add(self.s[3][d])
    }

    #[inline(always)]
    pub fn encrypt_pair(&self, l: &mut u32, r: &mut u32) {
        for i in 0..self.n {
            *l ^= self.p[i];
            *r ^= self.f(*l);
            std::mem::swap(l, r);
        }
        std::mem::swap(l, r);
        *r ^= self.p[self.n];
        *l ^= self.p[self.n + 1];
    }

    #[inline(always)]
    pub fn decrypt_pair(&self, l: &mut u32, r: &mut u32) {
        let mut i = self.n + 1;
        while i > 1 {
            *l ^= self.p[i];
            *r ^= self.f(*l);
            std::mem::swap(l, r);
            i -= 1;
        }
        std::mem::swap(l, r);
        *l ^= self.p[0];
        *r ^= self.p[1];
    }

    #[inline(always)]
    pub fn encrypt_block_le(&self, b: &mut [u8; 8]) {
        let mut l = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let mut r = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        self.encrypt_pair(&mut l, &mut r);
        b[0..4].copy_from_slice(&l.to_le_bytes());
        b[4..8].copy_from_slice(&r.to_le_bytes());
    }

    #[inline(always)]
    pub fn decrypt_block_le(&self, b: &mut [u8; 8]) {
        let mut l = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let mut r = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        self.decrypt_pair(&mut l, &mut r);
        b[0..4].copy_from_slice(&l.to_le_bytes());
        b[4..8].copy_from_slice(&r.to_le_bytes());
    }
}

/// The Nadeo pak cipher stream: CBC-ish with a custom IV update, a 0x100-byte
/// re-key point, and (v18) encrypt-used-as-decrypt.
pub struct PakCipher {
    bf: Blowfish,
    iv: u64,
    iv_xor: u64,
    buf: [u8; 8],
    buf_index: usize,
    total_index: usize,
    version: i32,
}

impl PakCipher {
    pub fn new(key: &[u8], iv: u64, version: i32) -> PakCipher {
        let trick = if version >= 18 { Trick::LittleEndianPak18 } else { Trick::LittleEndian };
        PakCipher {
            bf: Blowfish::new(key, trick),
            iv,
            iv_xor: 0,
            buf: [0; 8],
            buf_index: 0,
            total_index: 0,
            version,
        }
    }

    /// The "encryption initializer": folder-name derived IV perturbation.
    pub fn initialize(&mut self, data: &[u8], offset: usize, count: usize) {
        for i in 0..count {
            let mut lopart = (self.iv_xor & 0xFFFF_FFFF) as u32;
            let hipart_in = (self.iv_xor >> 32) as u32;
            lopart = ((data[offset + i] as u32) | 0xAA) ^ ((lopart << 13) | (hipart_in >> 19));
            let hipart = ((self.iv_xor << 13) >> 32) as u32;
            self.iv_xor = ((hipart as u64) << 32) | (lopart as u64);
        }
    }

    /// Decrypt `out.len()` bytes, pulling ciphertext from `src` (a reader closure).
    pub fn read<F: FnMut(&mut [u8]) -> usize>(&mut self, out: &mut [u8], mut src: F) -> usize {
        if self.total_index == 0 {
            self.iv ^= self.iv_xor;
            self.iv_xor = 0;
        }
        for i in 0..out.len() {
            if self.buf_index % 8 == 0 {
                if self.buf_index == 0x100 {
                    self.iv ^= self.iv_xor;
                    self.iv_xor = 0;
                    self.buf_index = 0;
                }
                let mut read = 0;
                while read < 8 {
                    let r = src(&mut self.buf[read..8]);
                    if r == 0 {
                        break;
                    }
                    read += r;
                }
                if read < 8 {
                    return i;
                }
                let next_iv = u64::from_le_bytes(self.buf);
                if self.version >= 18 {
                    self.bf.encrypt_block_le(&mut self.buf);
                } else {
                    self.bf.decrypt_block_le(&mut self.buf);
                }
                let block = u64::from_le_bytes(self.buf) ^ self.iv;
                self.buf = block.to_le_bytes();
                if self.version >= 12 {
                    self.iv = (self.iv >> 0x2f) ^ self.iv.wrapping_mul(9) ^ next_iv;
                } else {
                    self.iv = next_iv;
                }
            }
            out[i] = self.buf[self.buf_index & 7];
            self.buf_index += 1;
            self.total_index += 1;
        }
        out.len()
    }
}
