//! The COMMON data segment (0xD) of the ROM: the models every course shares —
//! the item box, its "?" model, the fake box — as real geometry, read from the
//! ROM rather than approximated (vjeux, 2026-09-25: "Please stop making
//! approximations").
//!
//! The segment is the MIO0 block at ROM 0x132B50 (decompressed 0x2D158 bytes);
//! a segment address `0x0Dxxxxxx` indexes straight into it. The display lists
//! are F3DEX 0.95 (`F3DEX_GBI=1 F3D_OLD=1` in the decomp's Makefile), so the
//! opcodes are the negative `G_IMMFIRST` ones: TRI1 0xBF, QUAD 0xB5, TRI2 0xB1,
//! ENDDL 0xB8, and G_VTX 0x04 carrying `n` at bit 10 and `(v0+n)*2` in byte 2.
//!
//! What the item box is, exactly (`src/actors/item_box/render.inc.c`):
//!   * `D_0D003090` — the cube, drawn with LIGHTING cleared and
//!     `G_CC_MODULATEIA` + `G_RM_ZB_CLD_SURF` (a translucent cloud surface), so
//!     its colour is the VERTEX colour of `common_vtx_itembox` and its alpha
//!     the vertex alpha: that is the rainbow, and it is per-vertex, not a tint;
//!   * `itemBoxQuestionMarkModel` — a separate quad pair wearing
//!     `common_texture_item_box_question_mark` (32×64 RGBA16), rotated at twice
//!     the cube's rate so the "?" always faces you.

use crate::texture::{mio0_decode, Rom};

/// The MIO0 block that holds segment 0xD in the US ROM.
pub const SEG_D_ROM: usize = 0x0013_2B50;

/// A vertex of an F3D display list: position, texture coordinates (S10.5) and
/// the colour/normal byte quad. With lighting off the quad IS the colour.
#[derive(Clone, Copy, Debug)]
pub struct CVert {
    pub pos: [i16; 3],
    pub uv: [i16; 2],
    pub rgba: [u8; 4],
}

/// A triangle, with the texture bound when it was drawn.
#[derive(Clone, Debug)]
pub struct CTri {
    pub c: [CVert; 3],
    /// (segment offset of the image, format, bit size, width, height)
    pub tex: Option<TexRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TexRef {
    pub off: usize,
    pub fmt: u8,
    pub siz: u8,
    pub w: u32,
    pub h: u32,
}

/// The decompressed common segment.
pub struct Seg {
    pub data: Vec<u8>,
}

impl Seg {
    pub fn load(rom: &Rom) -> Result<Seg, String> {
        if rom.bytes.len() < SEG_D_ROM + 16 || &rom.bytes[SEG_D_ROM..SEG_D_ROM + 4] != b"MIO0" {
            return Err(format!("no MIO0 block at ROM 0x{SEG_D_ROM:X}"));
        }
        Ok(Seg { data: mio0_decode(&rom.bytes[SEG_D_ROM..])? })
    }

    fn u32_at(&self, o: usize) -> u32 {
        u32::from_be_bytes(self.data[o..o + 4].try_into().unwrap())
    }

    /// One vertex of a `Vtx_t` array at a segment offset.
    pub fn vertex(&self, off: usize) -> CVert {
        let s16 = |o: usize| i16::from_be_bytes(self.data[o..o + 2].try_into().unwrap());
        CVert {
            pos: [s16(off), s16(off + 2), s16(off + 4)],
            uv: [s16(off + 8), s16(off + 10)],
            rgba: [self.data[off + 12], self.data[off + 13], self.data[off + 14], self.data[off + 15]],
        }
    }

    /// The triangles a display list draws, following nested lists.
    pub fn dl_tris(&self, off: usize) -> Vec<CTri> {
        let mut out = Vec::new();
        self.walk(off, &mut [None; 64], &mut None, &mut out, 0);
        out
    }

    fn walk(&self, start: usize, slots: &mut [Option<CVert>; 64], tex: &mut Option<TexRef>, out: &mut Vec<CTri>, depth: u32) {
        if depth > 8 {
            return;
        }
        let mut o = start;
        // the tile the last G_SETTIMG named, completed by G_SETTILESIZE
        let mut timg: Option<(usize, u8, u8)> = None;
        while o + 8 <= self.data.len() {
            let (w0, w1) = (self.u32_at(o), self.u32_at(o + 4));
            let op = (w0 >> 24) as u8;
            o += 8;
            match op {
                0x04 => {
                    // G_VTX: n at bit 10, (v0 + n) * 2 in byte 2, address in w1
                    // gDma1p(G_VTX, v, (n << 10) | (16n - 1), v0 * 2): byte 2
                    // is v0*2 — NOT (v0+n)*2, which is the F3DEX2 form
                    let n = ((w0 >> 10) & 0x3F) as usize;
                    let v0 = ((w0 >> 16) & 0xFF) as usize / 2;
                    let base = (w1 & 0x00FF_FFFF) as usize;
                    for k in 0..n {
                        let at = base + 16 * k;
                        if at + 16 <= self.data.len() && v0 + k < slots.len() {
                            slots[v0 + k] = Some(self.vertex(at));
                        }
                    }
                }
                // F3DEX 0.95 (F3D_OLD): TRI1's indices ride in w1, w0 is the
                // bare opcode; TRI2/QUAD put the first triangle in w0's low
                // bytes and the second in w1
                0xBF => self.tri(slots, tex, out, [(w1 >> 16) as u8, (w1 >> 8) as u8, w1 as u8]),
                0xB1 | 0xB5 => {
                    self.tri(slots, tex, out, [(w0 >> 16) as u8, (w0 >> 8) as u8, w0 as u8]);
                    self.tri(slots, tex, out, [(w1 >> 16) as u8, (w1 >> 8) as u8, w1 as u8]);
                }
                0x06 => {
                    // G_DL: push (byte 2 == 0) or branch
                    let to = (w1 & 0x00FF_FFFF) as usize;
                    if (w1 >> 24) as u8 == 0x0D {
                        if ((w0 >> 16) & 0xFF) == 0 {
                            self.walk(to, slots, tex, out, depth + 1);
                        } else {
                            o = to;
                        }
                    }
                }
                0xFD => {
                    // G_SETTIMG: format, size, address
                    timg = Some(((w1 & 0x00FF_FFFF) as usize, ((w0 >> 21) & 0x7) as u8, ((w0 >> 19) & 0x3) as u8));
                }
                0xF2 => {
                    // G_SETTILESIZE: lrs/lrt are (w-1)*4, (h-1)*4 in 10.2
                    let lrs = (w1 >> 12) & 0xFFF;
                    let lrt = w1 & 0xFFF;
                    if let Some((off, fmt, siz)) = timg {
                        *tex = Some(TexRef { off, fmt, siz, w: lrs / 4 + 1, h: lrt / 4 + 1 });
                    }
                }
                0xB8 => return, // G_ENDDL
                _ => {}
            }
        }
    }

    fn tri(&self, slots: &[Option<CVert>; 64], tex: &Option<TexRef>, out: &mut Vec<CTri>, idx: [u8; 3]) {
        let mut c = [CVert { pos: [0; 3], uv: [0; 2], rgba: [255; 4] }; 3];
        for (k, i) in idx.iter().enumerate() {
            match slots.get((*i / 2) as usize).copied().flatten() {
                Some(v) => c[k] = v,
                None => return,
            }
        }
        out.push(CTri { c, tex: *tex });
    }

    /// An RGBA16 image out of the segment.
    pub fn rgba16(&self, off: usize, w: u32, h: u32) -> crate::texture::Image {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for i in 0..(w * h) as usize {
            let at = off + 2 * i;
            let p = if at + 2 <= self.data.len() { u16::from_be_bytes([self.data[at], self.data[at + 1]]) } else { 0 };
            let (r, g, b, a) = (((p >> 11) & 0x1F) as u8, ((p >> 6) & 0x1F) as u8, ((p >> 1) & 0x1F) as u8, (p & 1) as u8);
            let x5 = |v: u8| (v << 3) | (v >> 2);
            rgba.extend_from_slice(&[x5(r), x5(g), x5(b), if a == 1 { 255 } else { 0 }]);
        }
        crate::texture::Image { w, h, rgba }
    }
}

/// Segment offsets of the item box's parts (`yamls/us/common_data.yml`).
pub const VTX_ITEMBOX: usize = 0x1CE8;
pub const DL_ITEMBOX_BODY: usize = 0x3090;
pub const DL_ITEMBOX_Q: usize = 0x3008;
pub const TEX_ITEMBOX_Q: usize = 0x1EE8;
