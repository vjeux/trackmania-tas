//! A minimal PNG writer (8-bit RGB / grey, zlib through miniz_oxide) for the differential harness's
//! heat maps — no image crate in the tree carries a PNG encoder, and a heat map is one IDAT.

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        table[i as usize] = c;
    }
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ 0xffff_ffff
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let mut c = Vec::with_capacity(4 + body.len());
    c.extend_from_slice(kind);
    c.extend_from_slice(body);
    out.extend_from_slice(&c);
    out.extend_from_slice(&crc32(&c).to_be_bytes());
}

/// Encode `w`×`h` pixels of `channels` bytes each (1 = grey, 3 = RGB, 4 = RGBA), row-major, top row first.
pub fn encode(w: u32, h: u32, channels: u8, px: &[u8]) -> Vec<u8> {
    assert_eq!(px.len(), (w * h) as usize * channels as usize, "png: pixel buffer size");
    let colour_type = match channels { 1 => 0u8, 3 => 2, 4 => 6, _ => panic!("png: {channels} channels") };
    let mut raw = Vec::with_capacity(((w as usize * channels as usize) + 1) * h as usize);
    let stride = w as usize * channels as usize;
    for y in 0..h as usize {
        raw.push(0); // filter: none
        raw.extend_from_slice(&px[y * stride..(y + 1) * stride]);
    }
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, colour_type, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Write an RGB8 PNG.
pub fn write_rgb(path: &str, w: u32, h: u32, px: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, encode(w, h, 3, px))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_of_iend_is_the_well_known_constant() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
    }

    #[test]
    fn a_png_has_the_signature_and_the_three_chunks() {
        let png = encode(2, 1, 3, &[255, 0, 0, 0, 0, 255]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        // the IHDR body: width 2, height 1, depth 8, colour type 2
        assert_eq!(&png[16..29], &[0, 0, 0, 2, 0, 0, 0, 1, 8, 2, 0, 0, 0]);
    }
}
