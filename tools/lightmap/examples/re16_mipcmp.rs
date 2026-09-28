//! `re16_mipcmp CAPTURED.dds FILE.dds [--no-flip]` — the mip chain the GAME uploaded (a RenderDoc texture dump
//! with all levels, DX10 header) against the pack/embedded DDS it was loaded from, level by level, with the
//! engine's vertical flip applied to the file's blocks (`mapgeom::terrain::vflip_bc1`'s rule; BC3 = the alpha
//! block flipped row-wise too). SAME on every level > 0 means the stored chain is honoured (compressed blocks
//! cannot be regenerated bit-identically); DIFF on the coarse levels only = regenerated.
//!
//! RE 16, 2026-09-28 19:15Z: pwc-day env/frame127447/textures/e012346_14609.dds.gz (the palm trunk, BC1 64×256,
//! 9 levels) vs the embedded VegetPalmTreeSugarTrunk_D.dds → levels 0–6 and 8 SAME (0 blocks differ), level 7
//! (1×2) differs by 2 bytes = the flipper's sub-4-row edge case → the game uploads the file's levels as stored
//! (CPlugBitmap::ComputeMipMapLevels FUN_14046b1c0 runs only for single-level, CPU-filterable images).
use std::fs;

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

struct Dds {
    w: u32,
    h: u32,
    mips: u32,
    block: usize,
    off: usize,
    fmt: String,
}

fn parse(b: &[u8]) -> Dds {
    assert_eq!(&b[0..4], b"DDS ");
    let h = u32le(b, 12);
    let w = u32le(b, 16);
    let mips = u32le(b, 28).max(1);
    let fourcc = &b[84..88];
    let (block, off, fmt) = if fourcc == b"DX10" {
        let dxgi = u32le(b, 128);
        let block = match dxgi {
            70..=72 => 8,            // BC1
            73..=78 => 16,           // BC2, BC3
            79..=81 => 8,            // BC4
            82..=84 | 94..=99 => 16, // BC5, BC6H, BC7
            _ => panic!("dxgi {dxgi} not a block format"),
        };
        (block, 148usize, format!("DXGI {dxgi}"))
    } else {
        let s = String::from_utf8_lossy(fourcc).to_string();
        let block = match s.as_str() {
            "DXT1" | "ATI1" | "BC4U" => 8,
            "DXT3" | "DXT5" | "ATI2" | "BC5U" => 16,
            _ => panic!("fourcc {s} not a block format"),
        };
        (block, 128usize, s)
    };
    Dds { w, h, mips, block, off, fmt }
}

/// The engine's vertical flip of one compressed level: block rows reversed, the 4 index rows of every block
/// reversed (BC1: bytes 4..8; BC3: the alpha block's 16 3-bit indices re-packed row-wise, the colour block as
/// BC1). Levels under 4 rows keep the generic 4-row reversal (the game's exact rule there is not read).
fn vflip(data: &[u8], w: u32, h: u32, block: usize) -> Vec<u8> {
    let bw = ((w + 3) / 4) as usize;
    let bh = ((h + 3) / 4) as usize;
    let rowb = bw * block;
    let mut out = vec![0u8; rowb * bh];
    for r in 0..bh {
        let src = &data[(bh - 1 - r) * rowb..(bh - r) * rowb];
        let dst = &mut out[r * rowb..(r + 1) * rowb];
        for i in 0..bw {
            let b = &src[i * block..(i + 1) * block];
            let d = &mut dst[i * block..(i + 1) * block];
            if block == 8 {
                d[..4].copy_from_slice(&b[..4]);
                d[4] = b[7];
                d[5] = b[6];
                d[6] = b[5];
                d[7] = b[4];
            } else {
                d[0] = b[0];
                d[1] = b[1];
                let mut bits: u64 = 0;
                for k in 0..6 {
                    bits |= (b[2 + k] as u64) << (8 * k);
                }
                let mut idx = [0u8; 16];
                for k in 0..16 {
                    idx[k] = ((bits >> (3 * k)) & 7) as u8;
                }
                let mut flipped = [0u8; 16];
                for row in 0..4 {
                    for c in 0..4 {
                        flipped[row * 4 + c] = idx[(3 - row) * 4 + c];
                    }
                }
                let mut nb: u64 = 0;
                for k in 0..16 {
                    nb |= (flipped[k] as u64) << (3 * k);
                }
                for k in 0..6 {
                    d[2 + k] = ((nb >> (8 * k)) & 0xff) as u8;
                }
                d[8..12].copy_from_slice(&b[8..12]);
                d[12] = b[15];
                d[13] = b[14];
                d[14] = b[13];
                d[15] = b[12];
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: re16_mipcmp CAPTURED.dds FILE.dds [--no-flip]");
        std::process::exit(2);
    }
    let cap = fs::read(&a[1]).expect("captured dds");
    let fil = fs::read(&a[2]).expect("file dds");
    let flip = !a.iter().any(|x| x == "--no-flip");
    let c = parse(&cap);
    let f = parse(&fil);
    println!("captured {}x{} mips {} {} block {}; file {}x{} mips {} {} block {}; flip {}", c.w, c.h, c.mips, c.fmt, c.block, f.w, f.h, f.mips, f.fmt, f.block, flip);
    assert_eq!((c.w, c.h, c.block), (f.w, f.h, f.block), "dimensions/format differ");
    let (mut oc, mut of) = (c.off, f.off);
    let mut w = c.w;
    let mut h = c.h;
    let levels = c.mips.min(f.mips);
    let mut same_all = true;
    for lv in 0..levels {
        let bw = ((w + 3) / 4) as usize;
        let bh = ((h + 3) / 4) as usize;
        let sz = bw * bh * c.block;
        if oc + sz > cap.len() || of + sz > fil.len() {
            println!("level {lv}: data runs out (captured {} left, file {} left, need {sz})", cap.len() - oc, fil.len() - of);
            break;
        }
        let cl = &cap[oc..oc + sz];
        let fl_raw = &fil[of..of + sz];
        let fl = if flip { vflip(fl_raw, w, h, c.block) } else { fl_raw.to_vec() };
        let ndiff = cl.iter().zip(fl.iter()).filter(|(x, y)| x != y).count();
        let nblk = cl.chunks(c.block).zip(fl.chunks(c.block)).filter(|(x, y)| x != y).count();
        let verdict = if ndiff == 0 { "SAME" } else { "DIFF" };
        if ndiff != 0 {
            same_all = false;
        }
        println!("level {lv:2} {w:5}x{h:<5} {sz:7} B: {verdict}  bytes differing {ndiff} / blocks differing {nblk} of {}", sz / c.block);
        oc += sz;
        of += sz;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    println!("{}", if same_all { "VERDICT: every compared level is the file's bytes (flipped) — the stored chain is uploaded as is" } else { "VERDICT: levels differ — see above" });
}
