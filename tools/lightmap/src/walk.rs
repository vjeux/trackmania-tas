//! Structure walker: prints what it can make of the chunk, stops at the first
//! thing it does not understand and dumps the bytes there.

use crate::{hexdump, zlib_inflate, Cur};

pub struct Walk {
    pub log: String,
    /// The inflated CHmsLightMapCache node bytes.
    pub cache: Vec<u8>,
    /// The WEBP blobs in file order (empty = absent).
    pub blobs: Vec<Vec<u8>>,
}

fn is_webp(b: &[u8]) -> bool {
    b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP"
}

pub fn walk_chunk(p: &[u8]) -> Result<Walk, String> {
    let mut log = String::new();
    let mut blobs = Vec::new();
    let mut c = Cur::new(p);
    let version = c.u32()?;
    let has = c.u32()?;
    let u01 = c.u32()?;
    let u02 = c.u32()?;
    log.push_str(&format!("head: version={version} hasLightmaps={has} u01={u01} u02={u02}\n"));
    if has == 0 {
        log.push_str(&format!("no lightmaps; {} bytes left\n", c.left()));
        return Ok(Walk { log, cache: vec![], blobs });
    }
    let lmver = c.u32()?;
    let nframes = c.u32()?;
    log.push_str(&format!("lightmapVersion={lmver} frameCount={nframes}\n"));
    // Frames: a run of (u32 size, bytes) where the bytes are WEBP (or empty).
    let mut fi = 0;
    loop {
        let at = c.o;
        let n = c.u32()? as usize;
        if n == 0 {
            log.push_str(&format!("  blob {fi} at {at:#x}: EMPTY\n"));
            blobs.push(vec![]);
            fi += 1;
            if fi > 16 {
                break;
            }
            continue;
        }
        if n > c.left() {
            // not a blob length: rewind, this is the end of the frames
            c.o = at;
            break;
        }
        let b = &p[c.o..c.o + n];
        if is_webp(b) {
            let (w, h, kind) = webp_dims(b);
            log.push_str(&format!("  blob {fi} at {at:#x}: WEBP {n} B  {kind} {w}x{h}\n"));
            blobs.push(b.to_vec());
            c.o += n;
            fi += 1;
        } else {
            c.o = at;
            break;
        }
    }
    log.push_str(&format!("after frames at {:#x}, {} bytes left\n", c.o, c.left()));
    let usize_ = c.u32()? as usize;
    let csize = c.u32()? as usize;
    log.push_str(&format!("cache: uncompressed {usize_} compressed {csize}\n"));
    let z = c.take(csize)?;
    let raw = zlib_inflate(z, usize_)?;
    log.push_str(&format!("inflated {} bytes; {} bytes left after\n", raw.len(), c.left()));
    // the node: PIKS-framed chunks
    let mut r = Cur::new(&raw);
    while r.left() >= 4 {
        let id = r.u32()?;
        if id == 0xFACA_DE01 {
            log.push_str(&format!("  end marker FACADE01 at {:#x}; {} bytes left\n", r.o - 4, r.left()));
            break;
        }
        let magic = r.u32()?;
        if magic != 0x534B_4950 {
            log.push_str(&format!("  chunk {id:#010x} at {:#x}: NOT SKIPPABLE (magic {magic:#x})\n", r.o - 8));
            log.push_str(&hexdump(&raw[r.o - 8..], r.o - 8, 256));
            break;
        }
        let size = r.u32()? as usize;
        let payload = r.take(size)?;
        log.push_str(&format!("  chunk {id:#010x} size {size}\n"));
        let show = if size > 2048 { 1024 } else { size };
        log.push_str(&hexdump(&payload[..show], 0, show));
        if size > 2048 {
            log.push_str("   ...\n");
            log.push_str(&hexdump(&payload[size - 256..], size - 256, 256));
        }
    }
    Ok(Walk { log, cache: raw, blobs })
}

/// WEBP: (width, height, kind) from the VP8/VP8L/VP8X header.
pub fn webp_dims(b: &[u8]) -> (u32, u32, &'static str) {
    if b.len() < 30 {
        return (0, 0, "short");
    }
    match &b[12..16] {
        b"VP8 " => {
            // keyframe: 3 bytes frame tag, 3 bytes start code 9d 01 2a, then w/h u16 (14 bits)
            let o = 20;
            if &b[o + 3..o + 6] == b"\x9d\x01\x2a" {
                let w = u16::from_le_bytes([b[o + 6], b[o + 7]]) & 0x3fff;
                let h = u16::from_le_bytes([b[o + 8], b[o + 9]]) & 0x3fff;
                (w as u32, h as u32, "VP8")
            } else {
                (0, 0, "VP8?")
            }
        }
        b"VP8L" => {
            let o = 20;
            if b[o] == 0x2f {
                let bits = u32::from_le_bytes([b[o + 1], b[o + 2], b[o + 3], b[o + 4]]);
                let w = (bits & 0x3fff) + 1;
                let h = ((bits >> 14) & 0x3fff) + 1;
                (w, h, "VP8L")
            } else {
                (0, 0, "VP8L?")
            }
        }
        b"VP8X" => {
            let o = 20;
            let w = (b[o + 4] as u32 | (b[o + 5] as u32) << 8 | (b[o + 6] as u32) << 16) + 1;
            let h = (b[o + 7] as u32 | (b[o + 8] as u32) << 8 | (b[o + 9] as u32) << 16) + 1;
            (w, h, "VP8X")
        }
        _ => (0, 0, "?"),
    }
}
