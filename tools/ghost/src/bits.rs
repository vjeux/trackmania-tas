//! LSB-first bit reader/writer, matching the TM2020 ghost input bitstream.

pub struct BitReader<'a> {
    pub d: &'a [u8],
    pub pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(d: &'a [u8]) -> Self {
        BitReader { d, pos: 0 }
    }

    pub fn bits(&mut self, n: usize) -> u64 {
        let mut v: u64 = 0;
        let mut shift = 0;
        let mut left = n;
        while left > 0 {
            let byi = self.pos >> 3;
            assert!(byi < self.d.len(), "bitstream ended early");
            let bit = self.pos & 7;
            let take = left.min(8 - bit);
            let chunk = ((self.d[byi] >> bit) as u64) & ((1u64 << take) - 1);
            v |= chunk << shift;
            self.pos += take;
            shift += take;
            left -= take;
        }
        v
    }

    pub fn bit(&mut self) -> u64 {
        self.bits(1)
    }
}

#[derive(Default)]
pub struct BitWriter {
    pub buf: Vec<u8>,
    pub pos: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bits(&mut self, v: u64, n: usize) {
        let mut v = if n >= 64 { v } else { v & ((1u64 << n) - 1) };
        let mut left = n;
        while left > 0 {
            let byi = self.pos >> 3;
            if byi >= self.buf.len() {
                self.buf.push(0);
            }
            let bit = self.pos & 7;
            let put = left.min(8 - bit);
            let mask = (1u64 << put) - 1;
            self.buf[byi] |= (((v & mask) << bit) & 0xFF) as u8;
            v >>= put;
            self.pos += put;
            left -= put;
        }
    }

    pub fn bit(&mut self, v: u64) {
        self.bits(v, 1);
    }
}

/// Overwrite `n` bits at absolute bit position `pos` in an existing buffer.
/// Clears the target bits first, unlike `BitWriter` which only ORs into fresh
/// zero bytes. This is what makes candidate generation a patch rather than a
/// re-encode.
#[inline]
pub fn patch_bits(buf: &mut [u8], pos: usize, v: u64, n: usize) {
    let mut v = if n >= 64 { v } else { v & ((1u64 << n) - 1) };
    let mut left = n;
    let mut p = pos;
    while left > 0 {
        let byi = p >> 3;
        let bit = p & 7;
        let put = left.min(8 - bit);
        let mask = ((1u32 << put) - 1) as u8;
        buf[byi] = (buf[byi] & !(mask << bit)) | (((v as u8) & mask) << bit);
        v >>= put;
        p += put;
        left -= put;
    }
}
