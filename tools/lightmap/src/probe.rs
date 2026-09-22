//! Probing the mapping chunk 0x0602201A: parse the known head, then walk the
//! nested zlib blocks (u32 uncompressed, u32 compressed, data) and report.

use crate::{hexdump, zlib_inflate, Cur};

pub struct Block {
    pub at: usize,
    pub usize_: usize,
    pub csize: usize,
    pub data: Vec<u8>,
}

/// Find every `u32 usize, u32 csize, 78 xx` zlib block in `p` by scanning
/// (the layout in between is not yet known). Returns blocks in order.
pub fn scan_zlib_blocks(p: &[u8]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 10 <= p.len() {
        let usize_ = u32::from_le_bytes(p[i..i + 4].try_into().unwrap()) as usize;
        let csize = u32::from_le_bytes(p[i + 4..i + 8].try_into().unwrap()) as usize;
        let hdr = &p[i + 8..i + 10];
        let zhdr = hdr[0] == 0x78 && matches!(hdr[1], 0x01 | 0x5e | 0x9c | 0xda);
        if zhdr && csize >= 2 && csize <= p.len() - i - 8 && usize_ < 64 << 20 {
            if let Ok(d) = zlib_inflate(&p[i + 8..i + 8 + csize], usize_) {
                out.push(Block { at: i, usize_, csize, data: d });
                i += 8 + csize;
                continue;
            }
        }
        i += 1;
    }
    out
}

pub fn probe_mapping(p: &[u8]) -> String {
    let mut s = String::new();
    let blocks = scan_zlib_blocks(p);
    s.push_str(&format!("{} zlib blocks in {} bytes\n", blocks.len(), p.len()));
    let mut prev_end = 0usize;
    for (i, b) in blocks.iter().enumerate() {
        let gap = &p[prev_end..b.at];
        s.push_str(&format!(
            "-- gap before block {i}: {} bytes at {:#x}\n{}",
            gap.len(),
            prev_end,
            hexdump(gap, prev_end, 256)
        ));
        s.push_str(&format!(
            "-- block {i} at {:#x}: uncompressed {} compressed {} ratio {:.1}\n",
            b.at,
            b.usize_,
            b.csize,
            b.usize_ as f64 / b.csize.max(1) as f64
        ));
        s.push_str(&hexdump(&b.data, 0, 128));
        prev_end = b.at + 8 + b.csize;
    }
    let tail = &p[prev_end..];
    s.push_str(&format!("-- tail: {} bytes at {:#x}\n{}", tail.len(), prev_end, hexdump(tail, prev_end, 512)));
    s
}

pub fn words(p: &[u8], n: usize) -> String {
    let mut s = String::new();
    let mut c = Cur::new(p);
    for i in 0..n {
        let Ok(v) = c.u32() else { break };
        let f = f32::from_bits(v);
        s.push_str(&format!("{:04x}: {v:>10} {v:#010x} {f:>14.5}\n", i * 4));
    }
    s
}
