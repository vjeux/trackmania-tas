//! A minimal VP8 key-frame encoder (RFC 6386), lossy WEBP output — written
//! because the game REFUSES a lossless (VP8L) probe-volume atlas (frame 0
//! image 2 re-encoded as VP8L crashes the client on load, 2026-09-22) while
//! it accepts VP8L for the two big atlases. Only what a lightmap atlas
//! needs: intra 16×16 DC prediction everywhere (no B_PRED, no V/H/TM),
//! chroma DC prediction, one quantizer, no segmentation, no loop filter, one
//! token partition, the default coefficient probabilities. The forward
//! transforms are libvpx's; the inverse transforms and the prediction rules
//! are the decoder's (image-webp), so the reconstruction the encoder predicts
//! from is exactly what the decoder rebuilds.
//!
//! `encode(rgb, w, h, q)` → a complete `RIFF/WEBP/VP8 ` file.

use crate::vp8_tables::{COEFF_PROBS, COEFF_UPDATE_PROBS};

// ---- tables (RFC 6386 / image-webp) ----

const KF_YMODE_TREE: [i8; 8] = [-B_PRED, 2, 4, 6, -DC_PRED, -V_PRED, -H_PRED, -TM_PRED];
const KF_YMODE_PROBS: [u8; 4] = [145, 156, 163, 128];
const KF_UV_MODE_TREE: [i8; 6] = [-DC_PRED, 2, -V_PRED, 4, -H_PRED, -TM_PRED];
const KF_UV_MODE_PROBS: [u8; 3] = [142, 114, 183];
const DC_PRED: i8 = 0;
const V_PRED: i8 = 1;
const H_PRED: i8 = 2;
const TM_PRED: i8 = 3;
const B_PRED: i8 = 4;

const DCT_0: i8 = 0;
const DCT_1: i8 = 1;
const DCT_2: i8 = 2;
const DCT_3: i8 = 3;
const DCT_4: i8 = 4;
const DCT_CAT1: i8 = 5;
const DCT_CAT2: i8 = 6;
const DCT_CAT3: i8 = 7;
const DCT_CAT4: i8 = 8;
const DCT_CAT5: i8 = 9;
const DCT_CAT6: i8 = 10;
const DCT_EOB: i8 = 11;
const DCT_TOKEN_TREE: [i8; 22] = [
    -DCT_EOB, 2, -DCT_0, 4, -DCT_1, 6, 8, 12, -DCT_2, 10, -DCT_3, -DCT_4, 14, 16, -DCT_CAT1, -DCT_CAT2, 18, 20, -DCT_CAT3, -DCT_CAT4, -DCT_CAT5,
    -DCT_CAT6,
];
const PROB_DCT_CAT: [[u8; 12]; 6] = [
    [159, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [165, 145, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [173, 148, 140, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [176, 155, 140, 135, 0, 0, 0, 0, 0, 0, 0, 0],
    [180, 157, 141, 134, 130, 0, 0, 0, 0, 0, 0, 0],
    [254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129, 0],
];
const DCT_CAT_BASE: [i32; 6] = [5, 7, 11, 19, 35, 67];
const COEFF_BANDS: [usize; 16] = [0, 1, 2, 3, 6, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7];
const ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

#[rustfmt::skip]
const DC_QUANT: [i32; 128] = [
      4,   5,   6,   7,   8,   9,  10,  10,  11,  12,  13,  14,  15,  16,  17,  17,
     18,  19,  20,  20,  21,  21,  22,  22,  23,  23,  24,  25,  25,  26,  27,  28,
     29,  30,  31,  32,  33,  34,  35,  36,  37,  37,  38,  39,  40,  41,  42,  43,
     44,  45,  46,  46,  47,  48,  49,  50,  51,  52,  53,  54,  55,  56,  57,  58,
     59,  60,  61,  62,  63,  64,  65,  66,  67,  68,  69,  70,  71,  72,  73,  74,
     75,  76,  76,  77,  78,  79,  80,  81,  82,  83,  84,  85,  86,  87,  88,  89,
     91,  93,  95,  96,  98, 100, 101, 102, 104, 106, 108, 110, 112, 114, 116, 118,
    122, 124, 126, 128, 130, 132, 134, 136, 138, 140, 143, 145, 148, 151, 154, 157,
];
#[rustfmt::skip]
const AC_QUANT: [i32; 128] = [
      4,   5,   6,   7,   8,    9,  10,  11,  12,  13,  14,  15,  16,  17,  18,  19,
      20,  21,  22,  23,  24,  25,  26,  27,  28,  29,  30,  31,  32,  33,  34,  35,
      36,  37,  38,  39,  40,  41,  42,  43,  44,  45,  46,  47,  48,  49,  50,  51,
      52,  53,  54,  55,  56,  57,  58,  60,  62,  64,  66,  68,  70,  72,  74,  76,
      78,  80,  82,  84,  86,  88,  90,  92,  94,  96,  98, 100, 102, 104, 106, 108,
     110, 112, 114, 116, 119, 122, 125, 128, 131, 134, 137, 140, 143, 146, 149, 152,
     155, 158, 161, 164, 167, 170, 173, 177, 181, 185, 189, 193, 197, 201, 205, 209,
     213, 217, 221, 225, 229, 234, 239, 245, 249, 254, 259, 264, 269, 274, 279, 284,
];

// ---- the boolean entropy encoder (RFC 6386 §7.3) ----

struct BoolEnc {
    out: Vec<u8>,
    range: u32,
    bottom: u32,
    bit_count: i32,
}

impl BoolEnc {
    fn new() -> Self {
        BoolEnc { out: Vec::new(), range: 255, bottom: 0, bit_count: 24 }
    }
    fn add_one_to_output(&mut self) {
        let mut i = self.out.len();
        while i > 0 {
            i -= 1;
            if self.out[i] == 255 {
                self.out[i] = 0;
            } else {
                self.out[i] += 1;
                break;
            }
        }
    }
    fn put(&mut self, prob: u8, bit: bool) {
        let split = 1 + (((self.range - 1) * prob as u32) >> 8);
        if bit {
            self.bottom = self.bottom.wrapping_add(split);
            self.range -= split;
        } else {
            self.range = split;
        }
        while self.range < 128 {
            self.range <<= 1;
            if self.bottom & (1 << 31) != 0 {
                self.add_one_to_output();
            }
            self.bottom <<= 1;
            self.bit_count -= 1;
            if self.bit_count == 0 {
                self.out.push((self.bottom >> 24) as u8);
                self.bottom &= (1 << 24) - 1;
                self.bit_count = 8;
            }
        }
    }
    fn literal(&mut self, v: u32, bits: u32) {
        for i in (0..bits).rev() {
            self.put(128, (v >> i) & 1 != 0);
        }
    }
    /// Write `value` (a leaf of `tree`) starting at node `start`.
    fn tree(&mut self, tree: &[i8], probs: &[u8], value: i8, start: usize) {
        // find the path: DFS from start to the leaf holding -value
        fn path(tree: &[i8], node: usize, value: i8, acc: &mut Vec<(usize, bool)>) -> bool {
            for (bit, child) in [(false, tree[node]), (true, tree[node + 1])] {
                acc.push((node, bit));
                if child <= 0 {
                    if -child == value {
                        return true;
                    }
                } else if path(tree, child as usize, value, acc) {
                    return true;
                }
                acc.pop();
            }
            false
        }
        let mut acc = Vec::new();
        assert!(path(tree, start, value, &mut acc), "value {value} not in tree");
        for (node, bit) in acc {
            self.put(probs[node >> 1], bit);
        }
    }
    fn flush(mut self) -> Vec<u8> {
        // libvpx pads every partition with 32 zero bits first (`vp8_stop_encode`):
        // libwebp's reader loads bytes ahead and reports "premature end of
        // partition" — a decode failure, and the game then crashes — when a
        // partition ends exactly on its last symbol.
        for _ in 0..32 {
            self.put(128, false);
        }
        // RFC 6386 §7.3 flush: push out the remaining bits of `bottom`
        let mut c = self.bit_count;
        let mut v = self.bottom;
        if v & (1u32 << (32 - c)) != 0 {
            self.add_one_to_output();
        }
        v <<= c & 7;
        c >>= 3;
        while c > 0 {
            c -= 1;
            v <<= 8;
        }
        for _ in 0..4 {
            self.out.push((v >> 24) as u8);
            v <<= 8;
        }
        self.out
    }
}

// ---- transforms ----

/// libvpx `vp8_short_fdct4x4_c` (input: 16 residuals row-major).
fn fdct4x4(inp: &[i32; 16]) -> [i32; 16] {
    let mut tmp = [0i32; 16];
    for i in 0..4 {
        let ip = &inp[i * 4..i * 4 + 4];
        let a1 = (ip[0] + ip[3]) * 8;
        let b1 = (ip[1] + ip[2]) * 8;
        let c1 = (ip[1] - ip[2]) * 8;
        let d1 = (ip[0] - ip[3]) * 8;
        tmp[i * 4] = a1 + b1;
        tmp[i * 4 + 2] = a1 - b1;
        tmp[i * 4 + 1] = (c1 * 2217 + d1 * 5352 + 14500) >> 12;
        tmp[i * 4 + 3] = (d1 * 2217 - c1 * 5352 + 7500) >> 12;
    }
    let mut out = [0i32; 16];
    for i in 0..4 {
        let a1 = tmp[i] + tmp[12 + i];
        let b1 = tmp[4 + i] + tmp[8 + i];
        let c1 = tmp[4 + i] - tmp[8 + i];
        let d1 = tmp[i] - tmp[12 + i];
        out[i] = (a1 + b1 + 7) >> 4;
        out[8 + i] = (a1 - b1 + 7) >> 4;
        out[4 + i] = ((c1 * 2217 + d1 * 5352 + 12000) >> 16) + (d1 != 0) as i32;
        out[12 + i] = (d1 * 2217 - c1 * 5352 + 51000) >> 16;
    }
    out
}

/// libvpx `vp8_short_walsh4x4_c` (input: the 16 luma DCs, row-major).
fn fwht4x4(inp: &[i32; 16]) -> [i32; 16] {
    let mut tmp = [0i32; 16];
    for i in 0..4 {
        let ip = &inp[i * 4..i * 4 + 4];
        let a1 = (ip[0] + ip[2]) * 4;
        let d1 = (ip[1] + ip[3]) * 4;
        let c1 = (ip[1] - ip[3]) * 4;
        let b1 = (ip[0] - ip[2]) * 4;
        tmp[i * 4] = a1 + d1 + (a1 != 0) as i32;
        tmp[i * 4 + 1] = b1 + c1;
        tmp[i * 4 + 2] = b1 - c1;
        tmp[i * 4 + 3] = a1 - d1;
    }
    let mut out = [0i32; 16];
    for i in 0..4 {
        let a1 = tmp[i] + tmp[8 + i];
        let d1 = tmp[4 + i] + tmp[12 + i];
        let c1 = tmp[4 + i] - tmp[12 + i];
        let b1 = tmp[i] - tmp[8 + i];
        let mut a2 = a1 + d1;
        let mut b2 = b1 + c1;
        let mut c2 = b1 - c1;
        let mut d2 = a1 - d1;
        a2 += (a2 < 0) as i32;
        b2 += (b2 < 0) as i32;
        c2 += (c2 < 0) as i32;
        d2 += (d2 < 0) as i32;
        out[i] = (a2 + 3) >> 3;
        out[4 + i] = (b2 + 3) >> 3;
        out[8 + i] = (c2 + 3) >> 3;
        out[12 + i] = (d2 + 3) >> 3;
    }
    out
}

/// The decoder's inverse DCT (image-webp `transform::idct4x4`).
fn idct4x4(block: &mut [i32; 16]) {
    const C1: i64 = 20091;
    const C2: i64 = 35468;
    let f = |b: &[i32; 16], i: usize| b[i] as i64;
    for i in 0..4 {
        let a1 = f(block, i) + f(block, 8 + i);
        let b1 = f(block, i) - f(block, 8 + i);
        let t1 = (f(block, 4 + i) * C2) >> 16;
        let t2 = f(block, 12 + i) + ((f(block, 12 + i) * C1) >> 16);
        let c1 = t1 - t2;
        let t1 = f(block, 4 + i) + ((f(block, 4 + i) * C1) >> 16);
        let t2 = (f(block, 12 + i) * C2) >> 16;
        let d1 = t1 + t2;
        block[i] = (a1 + d1) as i32;
        block[4 + i] = (b1 + c1) as i32;
        block[12 + i] = (a1 - d1) as i32;
        block[8 + i] = (b1 - c1) as i32;
    }
    for i in 0..4 {
        let a1 = f(block, 4 * i) + f(block, 4 * i + 2);
        let b1 = f(block, 4 * i) - f(block, 4 * i + 2);
        let t1 = (f(block, 4 * i + 1) * C2) >> 16;
        let t2 = f(block, 4 * i + 3) + ((f(block, 4 * i + 3) * C1) >> 16);
        let c1 = t1 - t2;
        let t1 = f(block, 4 * i + 1) + ((f(block, 4 * i + 1) * C1) >> 16);
        let t2 = (f(block, 4 * i + 3) * C2) >> 16;
        let d1 = t1 + t2;
        block[4 * i] = ((a1 + d1 + 4) >> 3) as i32;
        block[4 * i + 3] = ((a1 - d1 + 4) >> 3) as i32;
        block[4 * i + 1] = ((b1 + c1 + 4) >> 3) as i32;
        block[4 * i + 2] = ((b1 - c1 + 4) >> 3) as i32;
    }
}

/// The decoder's inverse WHT (image-webp `transform::iwht4x4`).
fn iwht4x4(block: &mut [i32; 16]) {
    for i in 0..4 {
        let a1 = block[i] + block[12 + i];
        let b1 = block[4 + i] + block[8 + i];
        let c1 = block[4 + i] - block[8 + i];
        let d1 = block[i] - block[12 + i];
        block[i] = a1 + b1;
        block[4 + i] = c1 + d1;
        block[8 + i] = a1 - b1;
        block[12 + i] = d1 - c1;
    }
    for r in 0..4 {
        let b = &mut block[r * 4..r * 4 + 4];
        let a1 = b[0] + b[3];
        let b1 = b[1] + b[2];
        let c1 = b[1] - b[2];
        let d1 = b[0] - b[3];
        let (a2, b2, c2, d2) = (a1 + b1, c1 + d1, a1 - b1, d1 - c1);
        b[0] = (a2 + 3) >> 3;
        b[1] = (b2 + 3) >> 3;
        b[2] = (c2 + 3) >> 3;
        b[3] = (d2 + 3) >> 3;
    }
}

fn quantize(v: i32, q: i32) -> i32 {
    let a = v.abs();
    let r = (a + q / 2) / q;
    let r = r.min(2047 + 67);
    if v < 0 {
        -r
    } else {
        r
    }
}

// ---- token writing ----

struct TokenCtx {
    /// complexity flags as the decoder keeps them: [0] = Y2, [1..5] = Y columns/rows,
    /// [5..7] = U, [7..9] = V
    top: Vec<[u8; 9]>,
    left: [u8; 9],
}

/// Write one block's quantized coefficients (`coef[k]` indexed by raster position k,
/// scanned in zigzag order from `first`). Returns the "has coefficients" flag.
fn write_block(enc: &mut BoolEnc, plane: usize, coef: &[i32; 16], first: usize, ctx0: usize) -> bool {
    let probs = &COEFF_PROBS[plane];
    // last nonzero position in scan order
    let mut last: Option<usize> = None;
    for i in first..16 {
        if coef[ZIGZAG[i]] != 0 {
            last = Some(i);
        }
    }
    let mut ctx = ctx0;
    let mut prev_zero = false;
    let Some(last) = last else {
        enc.tree(&DCT_TOKEN_TREE, &probs[COEFF_BANDS[first]][ctx], DCT_EOB, 0);
        return false;
    };
    for i in first..=last {
        let v = coef[ZIGZAG[i]];
        let band = COEFF_BANDS[i];
        let p = &probs[band][ctx];
        let start = if prev_zero { 2 } else { 0 };
        let a = v.abs();
        if a == 0 {
            enc.tree(&DCT_TOKEN_TREE, p, DCT_0, start);
            prev_zero = true;
            ctx = 0;
            continue;
        }
        prev_zero = false;
        if a <= 4 {
            enc.tree(&DCT_TOKEN_TREE, p, a as i8, start);
        } else {
            let cat = (0..6).rev().find(|&c| a >= DCT_CAT_BASE[c]).unwrap();
            enc.tree(&DCT_TOKEN_TREE, p, DCT_CAT1 + cat as i8, start);
            let extra = a - DCT_CAT_BASE[cat];
            let nbits = [1, 2, 3, 4, 5, 11][cat];
            for t in 0..nbits {
                let bit = (extra >> (nbits - 1 - t)) & 1 != 0;
                enc.put(PROB_DCT_CAT[cat][t], bit);
            }
        }
        enc.put(128, v < 0);
        ctx = if a == 1 { 1 } else { 2 };
    }
    if last < 15 {
        let band = COEFF_BANDS[last + 1];
        enc.tree(&DCT_TOKEN_TREE, &probs[band][ctx], DCT_EOB, 0);
    }
    true
}

/// Encode an RGB image as a lossy VP8 key frame inside a WEBP container.
/// `q` = the quantizer index 0..127 (≈ 10 = very high quality, 40 = ordinary).
pub fn encode(rgb: &[u8], w: u32, h: u32, q: u8) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    assert_eq!(rgb.len(), w * h * 3);
    let (mbw, mbh) = ((w + 15) / 16, (h + 15) / 16);
    let (pw, ph) = (mbw * 16, mbh * 16);
    // planes, edge-replicated to the macroblock grid
    let mut yp = vec![0u8; pw * ph];
    let mut up = vec![0u8; pw / 2 * ph / 2];
    let mut vp = vec![0u8; pw / 2 * ph / 2];
    let mut uacc = vec![0i32; pw / 2 * ph / 2];
    let mut vacc = vec![0i32; pw / 2 * ph / 2];
    for y in 0..ph {
        let sy = y.min(h - 1);
        for x in 0..pw {
            let sx = x.min(w - 1);
            let i = (sy * w + sx) * 3;
            let (r, g, b) = (rgb[i] as i32, rgb[i + 1] as i32, rgb[i + 2] as i32);
            let yy = 16 + ((65738 * r + 129057 * g + 25064 * b + 128000) >> 18);
            let uu = 128 * 1024 + ((-37945 * r - 74494 * g + 112439 * b) >> 8);
            let vv = 128 * 1024 + ((112439 * r - 94154 * g - 18285 * b) >> 8);
            yp[y * pw + x] = yy.clamp(0, 255) as u8;
            let ci = (y / 2) * (pw / 2) + x / 2;
            uacc[ci] += uu;
            vacc[ci] += vv;
        }
    }
    for i in 0..up.len() {
        up[i] = ((uacc[i] + 2048) >> 12).clamp(0, 255) as u8;
        vp[i] = ((vacc[i] + 2048) >> 12).clamp(0, 255) as u8;
    }
    // quantizers (one segment, no deltas)
    let qi = q.min(127) as usize;
    let yac = AC_QUANT[qi];
    let y2dc = DC_QUANT[qi] * 2;
    let y2ac = (AC_QUANT[qi] * 155 / 100).max(8);
    let uvdc = DC_QUANT[qi].min(132);
    let uvac = AC_QUANT[qi];

    // reconstruction buffers (what the decoder will hold)
    let mut ry = vec![0u8; pw * ph];
    let mut ru = vec![0u8; pw / 2 * ph / 2];
    let mut rv = vec![0u8; pw / 2 * ph / 2];

    let mut hdr = BoolEnc::new();
    // frame header
    hdr.literal(0, 1); // color space
    hdr.literal(0, 1); // clamping type
    hdr.put(128, false); // segmentation enabled
    hdr.put(128, false); // filter type
    hdr.literal(0, 6); // loop filter level
    hdr.literal(0, 3); // sharpness
    hdr.put(128, false); // loop filter adjustments
    hdr.literal(0, 2); // log2 partitions
    hdr.literal(qi as u32, 7); // y ac qi
    for _ in 0..5 {
        hdr.put(128, false); // no quantizer deltas
    }
    hdr.literal(1, 1); // refresh entropy probs
    for i in 0..4 {
        for j in 0..8 {
            for k in 0..3 {
                for t in 0..11 {
                    hdr.put(COEFF_UPDATE_PROBS[i][j][k][t], false);
                }
            }
        }
    }
    hdr.literal(0, 1); // mb_no_coeff_skip = 0

    let mut tok = BoolEnc::new();
    let mut ctx = TokenCtx { top: vec![[0u8; 9]; mbw], left: [0u8; 9] };

    for mby in 0..mbh {
        ctx.left = [0u8; 9];
        for mbx in 0..mbw {
            // modes: DC everywhere
            hdr.tree(&KF_YMODE_TREE, &KF_YMODE_PROBS, DC_PRED, 0);
            hdr.tree(&KF_UV_MODE_TREE, &KF_UV_MODE_PROBS, DC_PRED, 0);

            // ---- luma ----
            let pred_y = dc_predict(&ry, pw, mbx * 16, mby * 16, 16, mby != 0, mbx != 0);
            let mut ycoef = [[0i32; 16]; 16];
            let mut dcs = [0i32; 16];
            for by in 0..4 {
                for bx in 0..4 {
                    let mut res = [0i32; 16];
                    for y in 0..4 {
                        for x in 0..4 {
                            let px = yp[(mby * 16 + by * 4 + y) * pw + mbx * 16 + bx * 4 + x] as i32;
                            res[y * 4 + x] = px - pred_y as i32;
                        }
                    }
                    let d = fdct4x4(&res);
                    let bi = by * 4 + bx;
                    dcs[bi] = d[0];
                    for k in 1..16 {
                        ycoef[bi][k] = quantize(d[k], yac);
                    }
                }
            }
            // Y2: WHT of the DCs, quantized
            let wht = fwht4x4(&dcs);
            let mut y2 = [0i32; 16];
            for k in 0..16 {
                y2[k] = quantize(wht[k], if k == 0 { y2dc } else { y2ac });
            }
            let c0 = (ctx.top[mbx][0] + ctx.left[0]) as usize;
            let nz = write_block(&mut tok, 1, &y2, 0, c0);
            ctx.top[mbx][0] = nz as u8;
            ctx.left[0] = nz as u8;
            // the DCs the decoder will reconstruct: dequant + iWHT
            let mut y2d = [0i32; 16];
            for k in 0..16 {
                y2d[k] = y2[k] * if k == 0 { y2dc } else { y2ac };
            }
            iwht4x4(&mut y2d);
            for by in 0..4 {
                let mut left = ctx.left[by + 1];
                for bx in 0..4 {
                    let bi = by * 4 + bx;
                    let c = (ctx.top[mbx][bx + 1] + left) as usize;
                    let nz = write_block(&mut tok, 0, &ycoef[bi], 1, c);
                    left = nz as u8;
                    ctx.top[mbx][bx + 1] = nz as u8;
                    // reconstruct this subblock
                    let mut blk = [0i32; 16];
                    blk[0] = y2d[bi];
                    for k in 1..16 {
                        blk[k] = ycoef[bi][k] * yac;
                    }
                    idct4x4(&mut blk);
                    for y in 0..4 {
                        for x in 0..4 {
                            let v = (pred_y as i32 + blk[y * 4 + x]).clamp(0, 255) as u8;
                            ry[(mby * 16 + by * 4 + y) * pw + mbx * 16 + bx * 4 + x] = v;
                        }
                    }
                }
                ctx.left[by + 1] = left;
            }
            // ---- chroma ----
            let cw = pw / 2;
            for (plane_src, plane_rec, j) in [(&up, &mut ru, 5usize), (&vp, &mut rv, 7usize)] {
                let pred = dc_predict(plane_rec, cw, mbx * 8, mby * 8, 8, mby != 0, mbx != 0);
                for by in 0..2 {
                    let mut left = ctx.left[by + j];
                    for bx in 0..2 {
                        let mut res = [0i32; 16];
                        for y in 0..4 {
                            for x in 0..4 {
                                let px = plane_src[(mby * 8 + by * 4 + y) * cw + mbx * 8 + bx * 4 + x] as i32;
                                res[y * 4 + x] = px - pred as i32;
                            }
                        }
                        let d = fdct4x4(&res);
                        let mut qc = [0i32; 16];
                        for k in 0..16 {
                            qc[k] = quantize(d[k], if k == 0 { uvdc } else { uvac });
                        }
                        let c = (ctx.top[mbx][bx + j] + left) as usize;
                        let nz = write_block(&mut tok, 2, &qc, 0, c);
                        left = nz as u8;
                        ctx.top[mbx][bx + j] = nz as u8;
                        let mut blk = [0i32; 16];
                        for k in 0..16 {
                            blk[k] = qc[k] * if k == 0 { uvdc } else { uvac };
                        }
                        idct4x4(&mut blk);
                        for y in 0..4 {
                            for x in 0..4 {
                                let v = (pred as i32 + blk[y * 4 + x]).clamp(0, 255) as u8;
                                plane_rec[(mby * 8 + by * 4 + y) * cw + mbx * 8 + bx * 4 + x] = v;
                            }
                        }
                    }
                    ctx.left[by + j] = left;
                }
            }
        }
    }

    let first = hdr.flush();
    let second = tok.flush();
    // frame tag: key frame (0), version 0, show_frame 1, first_part_size
    let tag: u32 = 0 | (0 << 1) | (1 << 4) | ((first.len() as u32) << 5);
    let mut vp8 = Vec::with_capacity(first.len() + second.len() + 10);
    vp8.extend_from_slice(&tag.to_le_bytes()[..3]);
    vp8.extend_from_slice(&[0x9d, 0x01, 0x2a]);
    vp8.extend_from_slice(&(w as u16).to_le_bytes());
    vp8.extend_from_slice(&(h as u16).to_le_bytes());
    vp8.extend_from_slice(&first);
    vp8.extend_from_slice(&second);
    // RIFF container
    let mut out = Vec::with_capacity(vp8.len() + 20);
    let padded = vp8.len() + (vp8.len() & 1);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((4 + 8 + padded) as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(b"VP8 ");
    out.extend_from_slice(&(vp8.len() as u32).to_le_bytes());
    out.extend_from_slice(&vp8);
    if vp8.len() & 1 == 1 {
        out.push(0);
    }
    out
}

/// The decoder's DC prediction for a `size`×`size` block at (x0, y0) of a
/// reconstructed plane with row pitch `pitch`.
fn dc_predict(plane: &[u8], pitch: usize, x0: usize, y0: usize, size: usize, above: bool, left: bool) -> u8 {
    let mut sum = 0u32;
    let mut shf = if size == 8 { 2 } else { 3 };
    if left {
        for y in 0..size {
            sum += plane[(y0 + y) * pitch + x0 - 1] as u32;
        }
        shf += 1;
    }
    if above {
        for x in 0..size {
            sum += plane[(y0 - 1) * pitch + x0 + x] as u32;
        }
        shf += 1;
    }
    if !left && !above {
        128
    } else {
        ((sum + (1 << (shf - 1))) >> shf) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_decodes_close() {
        // a smooth gradient with a hard-edged square, odd size (198×194 like a probe atlas)
        let (w, h) = (198u32, 194u32);
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 3) as usize;
                let inside = (40..90).contains(&x) && (50..120).contains(&y);
                rgb[i] = if inside { 10 } else { (x * 255 / w) as u8 };
                rgb[i + 1] = if inside { 10 } else { (y * 255 / h) as u8 };
                rgb[i + 2] = if inside { 10 } else { 200 };
            }
        }
        let bytes = encode(&rgb, w, h, 10);
        let dec = crate::img::decode_webp(&bytes).expect("our VP8 must decode");
        assert_eq!((dec.w, dec.h), (w, h));
        let mut se = 0f64;
        for (a, b) in dec.px.iter().zip(&rgb) {
            let d = *a as f64 - *b as f64;
            se += d * d;
        }
        let mse = se / rgb.len() as f64;
        let psnr = 10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10();
        eprintln!("vp8 roundtrip: {} bytes, psnr {psnr:.1} dB", bytes.len());
        assert!(psnr > 35.0, "psnr {psnr}");
    }
}
