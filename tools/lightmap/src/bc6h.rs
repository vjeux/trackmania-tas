//! BC6H (DXGI_FORMAT_BC6H_UF16/SF16) block decoder — a Rust port of the BC6H part of
//! bcdec.h by Sergii "iOrange" Kudlai (MIT / Unlicense, third-party/bcdec), used to read
//! the packs' HDR sky and ambient cubes (`SkyColor.dds`, `EnvCubicHdr.dds`, `AmbCubeP.dds`).

struct Bits {
    low: u64,
    high: u64,
}

impl Bits {
    fn bits(&mut self, n: u32) -> i32 {
        let mask = if n >= 32 { u64::MAX } else { (1u64 << n) - 1 };
        let v = (self.low & mask) as i32;
        self.low >>= n;
        self.low |= (self.high & mask) << (64 - n);
        self.high >>= n;
        v
    }
    /// The same bits, reversed (BC6H stores a few endpoint fields MSB first).
    fn bits_r(&mut self, n: u32) -> i32 {
        let mut b = self.bits(n);
        let mut r = 0;
        for _ in 0..n {
            r = (r << 1) | (b & 1);
            b >>= 1;
        }
        r
    }
}

const ACTUAL_BITS: [[i32; 14]; 4] = [
    [10, 7, 11, 11, 11, 9, 8, 8, 8, 6, 10, 11, 12, 16],
    [5, 6, 5, 4, 4, 5, 6, 5, 5, 6, 10, 9, 8, 4],
    [5, 6, 4, 5, 4, 5, 5, 6, 5, 6, 10, 9, 8, 4],
    [5, 6, 4, 4, 5, 5, 5, 5, 6, 6, 10, 9, 8, 4],
];

const PARTITION_SETS: [[[u8; 4]; 4]; 32] = [
    [[128,0,1,1], [0,0,1,1], [0,0,1,1], [0,0,1,129]],
    [[128,0,0,1], [0,0,0,1], [0,0,0,1], [0,0,0,129]],
    [[128,1,1,1], [0,1,1,1], [0,1,1,1], [0,1,1,129]],
    [[128,0,0,1], [0,0,1,1], [0,0,1,1], [0,1,1,129]],
    [[128,0,0,0], [0,0,0,1], [0,0,0,1], [0,0,1,129]],
    [[128,0,1,1], [0,1,1,1], [0,1,1,1], [1,1,1,129]],
    [[128,0,0,1], [0,0,1,1], [0,1,1,1], [1,1,1,129]],
    [[128,0,0,0], [0,0,0,1], [0,0,1,1], [0,1,1,129]],
    [[128,0,0,0], [0,0,0,0], [0,0,0,1], [0,0,1,129]],
    [[128,0,1,1], [0,1,1,1], [1,1,1,1], [1,1,1,129]],
    [[128,0,0,0], [0,0,0,1], [0,1,1,1], [1,1,1,129]],
    [[128,0,0,0], [0,0,0,0], [0,0,0,1], [0,1,1,129]],
    [[128,0,0,1], [0,1,1,1], [1,1,1,1], [1,1,1,129]],
    [[128,0,0,0], [0,0,0,0], [1,1,1,1], [1,1,1,129]],
    [[128,0,0,0], [1,1,1,1], [1,1,1,1], [1,1,1,129]],
    [[128,0,0,0], [0,0,0,0], [0,0,0,0], [1,1,1,129]],
    [[128,0,0,0], [1,0,0,0], [1,1,1,0], [1,1,1,129]],
    [[128,1,129,1], [0,0,0,1], [0,0,0,0], [0,0,0,0]],
    [[128,0,0,0], [0,0,0,0], [129,0,0,0], [1,1,1,0]],
    [[128,1,129,1], [0,0,1,1], [0,0,0,1], [0,0,0,0]],
    [[128,0,129,1], [0,0,0,1], [0,0,0,0], [0,0,0,0]],
    [[128,0,0,0], [1,0,0,0], [129,1,0,0], [1,1,1,0]],
    [[128,0,0,0], [0,0,0,0], [129,0,0,0], [1,1,0,0]],
    [[128,1,1,1], [0,0,1,1], [0,0,1,1], [0,0,0,129]],
    [[128,0,129,1], [0,0,0,1], [0,0,0,1], [0,0,0,0]],
    [[128,0,0,0], [1,0,0,0], [129,0,0,0], [1,1,0,0]],
    [[128,1,129,0], [0,1,1,0], [0,1,1,0], [0,1,1,0]],
    [[128,0,129,1], [0,1,1,0], [0,1,1,0], [1,1,0,0]],
    [[128,0,0,1], [0,1,1,1], [129,1,1,0], [1,0,0,0]],
    [[128,0,0,0], [1,1,1,1], [129,1,1,1], [0,0,0,0]],
    [[128,1,129,1], [0,0,0,1], [1,0,0,0], [1,1,1,0]],
    [[128,0,129,1], [1,0,0,1], [1,0,0,1], [1,1,0,0]],
];

const WEIGHT3: [i32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const WEIGHT4: [i32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

fn extend_sign(v: i32, bits: i32) -> i32 {
    (v << (32 - bits)) >> (32 - bits)
}

fn transform_inverse(v: i32, a0: i32, bits: i32, signed: bool) -> i32 {
    let v = (v + a0) & ((1 << bits) - 1);
    if signed { extend_sign(v, bits) } else { v }
}

fn unquantize(v: i32, bits: i32, signed: bool) -> i32 {
    if !signed {
        if bits >= 15 { v } else if v == 0 { 0 } else if v == (1 << bits) - 1 { 0xFFFF } else { ((v << 16) + 0x8000) >> bits }
    } else if bits >= 16 {
        v
    } else {
        let (s, v) = if v < 0 { (true, -v) } else { (false, v) };
        let unq = if v == 0 { 0 } else if v >= (1 << (bits - 1)) - 1 { 0x7FFF } else { ((v << 15) + 0x4000) >> (bits - 1) };
        if s { -unq } else { unq }
    }
}

fn interpolate(a: i32, b: i32, w: i32) -> i32 {
    (a * (64 - w) + b * w + 32) >> 6
}

fn finish_unquantize(v: i32, signed: bool) -> u16 {
    if !signed {
        ((v * 31) >> 6) as u16
    } else {
        let v = if v < 0 { -(((-v) * 31) >> 5) } else { (v * 31) >> 5 };
        if v < 0 { 0x8000 | (-v) as u16 } else { v as u16 }
    }
}

pub fn half_to_f32(h: u16) -> f32 {
    let mut o = ((h & 0x7fff) as u32) << 13;
    let exp = 0x7c00u32 << 13 & o;
    o = o.wrapping_add((127 - 15) << 23);
    let mut f = f32::from_bits(o);
    if exp == 0x7c00 << 13 {
        f = f32::from_bits(o.wrapping_add((128 - 16) << 23));
    } else if exp == 0 {
        f = f32::from_bits(o.wrapping_add(1 << 23)) - f32::from_bits(113 << 23);
    }
    if h & 0x8000 != 0 { -f } else { f }
}

/// Decode one 16-byte BC6H block into 16 RGB half-floats (row-major 4×4).
pub fn decode_block_half(block: &[u8], signed: bool) -> [[u16; 3]; 16] {
    let mut out = [[0u16; 3]; 16];
    let mut bs = Bits { low: u64::from_le_bytes(block[0..8].try_into().unwrap()), high: u64::from_le_bytes(block[8..16].try_into().unwrap()) };
    let (mut r, mut g, mut b) = ([0i32; 4], [0i32; 4], [0i32; 4]);
    let mut mode = bs.bits(2) as usize;
    if mode > 1 {
        mode |= (bs.bits(3) as usize) << 2;
    }
    let mut partition = 0usize;
    match mode {
        0b00 => {
            g[2] |= (bs.bits(1) << 4);
            b[2] |= (bs.bits(1) << 4);
            b[3] |= (bs.bits(1) << 4);
            r[0] |= bs.bits(10);
            g[0] |= bs.bits(10);
            b[0] |= bs.bits(10);
            r[1] |= bs.bits(5);
            g[3] |= (bs.bits(1) << 4);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(5);
            b[3] |= bs.bits(1);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 2);
            r[3] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 3);
            partition = bs.bits(5) as usize;
            mode = 0;
        }
        0b01 => {
            g[2] |= (bs.bits(1) << 5);
            g[3] |= (bs.bits(1) << 4);
            g[3] |= (bs.bits(1) << 5);
            r[0] |= bs.bits(7);
            b[3] |= bs.bits(1);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= (bs.bits(1) << 4);
            g[0] |= bs.bits(7);
            b[2] |= (bs.bits(1) << 5);
            b[3] |= (bs.bits(1) << 2);
            g[2] |= (bs.bits(1) << 4);
            b[0] |= bs.bits(7);
            b[3] |= (bs.bits(1) << 3);
            b[3] |= (bs.bits(1) << 5);
            b[3] |= (bs.bits(1) << 4);
            r[1] |= bs.bits(6);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(6);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(6);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(6);
            r[3] |= bs.bits(6);
            partition = bs.bits(5) as usize;
            mode = 1;
        }
        0b00010 => {
            r[0] |= bs.bits(10);
            g[0] |= bs.bits(10);
            b[0] |= bs.bits(10);
            r[1] |= bs.bits(5);
            r[0] |= (bs.bits(1) << 10);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(4);
            g[0] |= (bs.bits(1) << 10);
            b[3] |= bs.bits(1);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(4);
            b[0] |= (bs.bits(1) << 10);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 2);
            r[3] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 3);
            partition = bs.bits(5) as usize;
            mode = 2;
        }
        0b00110 => {
            r[0] |= bs.bits(10);
            g[0] |= bs.bits(10);
            b[0] |= bs.bits(10);
            r[1] |= bs.bits(4);
            r[0] |= (bs.bits(1) << 10);
            g[3] |= (bs.bits(1) << 4);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(5);
            g[0] |= (bs.bits(1) << 10);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(4);
            b[0] |= (bs.bits(1) << 10);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(4);
            b[3] |= bs.bits(1);
            b[3] |= (bs.bits(1) << 2);
            r[3] |= bs.bits(4);
            g[2] |= (bs.bits(1) << 4);
            b[3] |= (bs.bits(1) << 3);
            partition = bs.bits(5) as usize;
            mode = 3;
        }
        0b01010 => {
            r[0] |= bs.bits(10);
            g[0] |= bs.bits(10);
            b[0] |= bs.bits(10);
            r[1] |= bs.bits(4);
            r[0] |= (bs.bits(1) << 10);
            b[2] |= (bs.bits(1) << 4);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(4);
            g[0] |= (bs.bits(1) << 10);
            b[3] |= bs.bits(1);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(5);
            b[0] |= (bs.bits(1) << 10);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(4);
            b[3] |= (bs.bits(1) << 1);
            b[3] |= (bs.bits(1) << 2);
            r[3] |= bs.bits(4);
            b[3] |= (bs.bits(1) << 4);
            b[3] |= (bs.bits(1) << 3);
            partition = bs.bits(5) as usize;
            mode = 4;
        }
        0b01110 => {
            r[0] |= bs.bits(9);
            b[2] |= (bs.bits(1) << 4);
            g[0] |= bs.bits(9);
            g[2] |= (bs.bits(1) << 4);
            b[0] |= bs.bits(9);
            b[3] |= (bs.bits(1) << 4);
            r[1] |= bs.bits(5);
            g[3] |= (bs.bits(1) << 4);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(5);
            b[3] |= bs.bits(1);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 2);
            r[3] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 3);
            partition = bs.bits(5) as usize;
            mode = 5;
        }
        0b10010 => {
            r[0] |= bs.bits(8);
            g[3] |= (bs.bits(1) << 4);
            b[2] |= (bs.bits(1) << 4);
            g[0] |= bs.bits(8);
            b[3] |= (bs.bits(1) << 2);
            g[2] |= (bs.bits(1) << 4);
            b[0] |= bs.bits(8);
            b[3] |= (bs.bits(1) << 3);
            b[3] |= (bs.bits(1) << 4);
            r[1] |= bs.bits(6);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(5);
            b[3] |= bs.bits(1);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(6);
            r[3] |= bs.bits(6);
            partition = bs.bits(5) as usize;
            mode = 6;
        }
        0b10110 => {
            r[0] |= bs.bits(8);
            b[3] |= bs.bits(1);
            b[2] |= (bs.bits(1) << 4);
            g[0] |= bs.bits(8);
            g[2] |= (bs.bits(1) << 5);
            g[2] |= (bs.bits(1) << 4);
            b[0] |= bs.bits(8);
            g[3] |= (bs.bits(1) << 5);
            b[3] |= (bs.bits(1) << 4);
            r[1] |= bs.bits(5);
            g[3] |= (bs.bits(1) << 4);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(6);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 2);
            r[3] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 3);
            partition = bs.bits(5) as usize;
            mode = 7;
        }
        0b11010 => {
            r[0] |= bs.bits(8);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= (bs.bits(1) << 4);
            g[0] |= bs.bits(8);
            b[2] |= (bs.bits(1) << 5);
            g[2] |= (bs.bits(1) << 4);
            b[0] |= bs.bits(8);
            b[3] |= (bs.bits(1) << 5);
            b[3] |= (bs.bits(1) << 4);
            r[1] |= bs.bits(5);
            g[3] |= (bs.bits(1) << 4);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(5);
            b[3] |= bs.bits(1);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(6);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 2);
            r[3] |= bs.bits(5);
            b[3] |= (bs.bits(1) << 3);
            partition = bs.bits(5) as usize;
            mode = 8;
        }
        0b11110 => {
            r[0] |= bs.bits(6);
            g[3] |= (bs.bits(1) << 4);
            b[3] |= bs.bits(1);
            b[3] |= (bs.bits(1) << 1);
            b[2] |= (bs.bits(1) << 4);
            g[0] |= bs.bits(6);
            g[2] |= (bs.bits(1) << 5);
            b[2] |= (bs.bits(1) << 5);
            b[3] |= (bs.bits(1) << 2);
            g[2] |= (bs.bits(1) << 4);
            b[0] |= bs.bits(6);
            g[3] |= (bs.bits(1) << 5);
            b[3] |= (bs.bits(1) << 3);
            b[3] |= (bs.bits(1) << 5);
            b[3] |= (bs.bits(1) << 4);
            r[1] |= bs.bits(6);
            g[2] |= bs.bits(4);
            g[1] |= bs.bits(6);
            g[3] |= bs.bits(4);
            b[1] |= bs.bits(6);
            b[2] |= bs.bits(4);
            r[2] |= bs.bits(6);
            r[3] |= bs.bits(6);
            partition = bs.bits(5) as usize;
            mode = 9;
        }
        0b00011 => {
            r[0] |= bs.bits(10);
            g[0] |= bs.bits(10);
            b[0] |= bs.bits(10);
            r[1] |= bs.bits(10);
            g[1] |= bs.bits(10);
            b[1] |= bs.bits(10);
            mode = 10;
        }
        0b00111 => {
            r[0] |= bs.bits(10);
            g[0] |= bs.bits(10);
            b[0] |= bs.bits(10);
            r[1] |= bs.bits(9);
            r[0] |= (bs.bits(1) << 10);
            g[1] |= bs.bits(9);
            g[0] |= (bs.bits(1) << 10);
            b[1] |= bs.bits(9);
            b[0] |= (bs.bits(1) << 10);
            mode = 11;
        }
        0b01011 => {
            r[0] |= bs.bits(10);
            g[0] |= bs.bits(10);
            b[0] |= bs.bits(10);
            r[1] |= bs.bits(8);
            r[0] |= (bs.bits_r(2) << 10);
            g[1] |= bs.bits(8);
            g[0] |= (bs.bits_r(2) << 10);
            b[1] |= bs.bits(8);
            b[0] |= (bs.bits_r(2) << 10);
            mode = 12;
        }
        0b01111 => {
            r[0] |= bs.bits(10);
            g[0] |= bs.bits(10);
            b[0] |= bs.bits(10);
            r[1] |= bs.bits(4);
            r[0] |= (bs.bits_r(6) << 10);
            g[1] |= bs.bits(4);
            g[0] |= (bs.bits_r(6) << 10);
            b[1] |= bs.bits(4);
            b[0] |= (bs.bits_r(6) << 10);
            mode = 13;
        }
        _ => return out, // reserved modes decode to zero
    }
    let two = mode < 10;
    let bits0 = ACTUAL_BITS[0][mode];
    if signed {
        r[0] = extend_sign(r[0], bits0);
        g[0] = extend_sign(g[0], bits0);
        b[0] = extend_sign(b[0], bits0);
    }
    let n_ep = if two { 4 } else { 2 };
    if (mode != 9 && mode != 10) || signed {
        for i in 1..n_ep {
            r[i] = extend_sign(r[i], ACTUAL_BITS[1][mode]);
            g[i] = extend_sign(g[i], ACTUAL_BITS[2][mode]);
            b[i] = extend_sign(b[i], ACTUAL_BITS[3][mode]);
        }
    }
    if mode != 9 && mode != 10 {
        for i in 1..n_ep {
            r[i] = transform_inverse(r[i], r[0], bits0, signed);
            g[i] = transform_inverse(g[i], g[0], bits0, signed);
            b[i] = transform_inverse(b[i], b[0], bits0, signed);
        }
    }
    for i in 0..n_ep {
        r[i] = unquantize(r[i], bits0, signed);
        g[i] = unquantize(g[i], bits0, signed);
        b[i] = unquantize(b[i], bits0, signed);
    }
    for i in 0..4 {
        for j in 0..4 {
            let mut pset: u32 = if !two { if i | j != 0 { 0 } else { 128 } } else { PARTITION_SETS[partition][i][j] as u32 };
            let mut index_bits = if !two { 4 } else { 3 };
            if pset & 0x80 != 0 {
                index_bits -= 1;
            }
            pset &= 1;
            let index = bs.bits(index_bits) as usize;
            let w = if !two { WEIGHT4[index] } else { WEIGHT3[index] };
            let e = (pset * 2) as usize;
            out[i * 4 + j] = [finish_unquantize(interpolate(r[e], r[e + 1], w), signed), finish_unquantize(interpolate(g[e], g[e + 1], w), signed), finish_unquantize(interpolate(b[e], b[e + 1], w), signed)];
        }
    }
    out
}

/// Decode a whole BC6H image (`w` × `h` texels, blocks row-major) to RGB f32.
pub fn decode_image(data: &[u8], w: usize, h: usize, signed: bool) -> Vec<[f32; 3]> {
    let (bw, bh) = ((w + 3) / 4, (h + 3) / 4);
    let mut out = vec![[0f32; 3]; w * h];
    for by in 0..bh {
        for bx in 0..bw {
            let o = (by * bw + bx) * 16;
            if o + 16 > data.len() {
                return out;
            }
            let blk = decode_block_half(&data[o..o + 16], signed);
            for i in 0..4 {
                for j in 0..4 {
                    let (x, y) = (bx * 4 + j, by * 4 + i);
                    if x < w && y < h {
                        let p = blk[i * 4 + j];
                        out[y * w + x] = [half_to_f32(p[0]), half_to_f32(p[1]), half_to_f32(p[2])];
                    }
                }
            }
        }
    }
    out
}

/// A DDS file with a DX10 header: (width, height, dxgi format, mip count, cubemap, array size, data).
pub struct Dds<'a> {
    pub w: usize,
    pub h: usize,
    pub format: u32,
    pub mips: usize,
    pub cubemap: bool,
    pub data: &'a [u8],
}

pub fn parse_dds(d: &[u8]) -> Result<Dds<'_>, String> {
    if d.len() < 128 || &d[..4] != b"DDS " {
        return Err("not a DDS".into());
    }
    let u = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let (h, w, mips) = (u(12) as usize, u(16) as usize, u(28).max(1) as usize);
    let fourcc = &d[84..88];
    let caps2 = u(112);
    let (format, off) = if fourcc == b"DX10" { (u(128), 148) } else { (0, 128) };
    Ok(Dds { w, h, format, mips, cubemap: caps2 & 0x200 != 0, data: &d[off..] })
}

/// Byte size of one BC6H/BC7 mip level.
pub fn bc_mip_bytes(w: usize, h: usize) -> usize {
    ((w + 3) / 4) * ((h + 3) / 4) * 16
}
