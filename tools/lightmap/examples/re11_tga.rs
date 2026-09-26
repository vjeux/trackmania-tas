//! `re11_tga FILE.tga` — a 32-bpp uncompressed TGA (the water fog images): per ROW (file order, bottom-up when descriptor bit 5 is 0)
//! the first / last pixel (RGBA) and the alpha min / max; then per COLUMN the same over the rows (the transposed reading). RE 11.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let b = std::fs::read(&a[1]).unwrap();
    let (idlen, cmap, ty) = (b[0] as usize, b[1], b[2]);
    let w = u16::from_le_bytes([b[12], b[13]]) as usize;
    let h = u16::from_le_bytes([b[14], b[15]]) as usize;
    let bpp = b[16] as usize;
    let desc = b[17];
    println!("{}: type {ty} cmap {cmap} {w}×{h} {bpp} bpp descriptor {desc:#x} ({}, alpha bits {})", a[1], if desc & 0x20 != 0 { "top-down" } else { "bottom-up" }, desc & 0xf);
    assert!(ty == 2 && bpp == 32);
    let px = &b[18 + idlen..];
    let get = |x: usize, y: usize| -> [u8; 4] { let i = (y * w + x) * 4; [px[i + 2], px[i + 1], px[i], px[i + 3]] }; // BGRA → RGBA
    for y in 0..h {
        let (mut lo, mut hi) = (255u8, 0u8);
        for x in 0..w { let p = get(x, y); lo = lo.min(p[3]); hi = hi.max(p[3]); }
        if y < 3 || y + 3 >= h || y % 8 == 0 { println!("  file row {y:3}: first {:?} last {:?} alpha {lo}..{hi}", get(0, y), get(w - 1, y)); }
    }
    println!("  transposed (per column over the rows):");
    for x in 0..w {
        let (mut lo, mut hi) = (255u8, 0u8);
        for y in 0..h { let p = get(x, y); lo = lo.min(p[3]); hi = hi.max(p[3]); }
        if x < 3 || x + 3 >= w || x % 32 == 0 { println!("  column {x:3}: bottom {:?} top {:?} alpha {lo}..{hi}", get(x, 0), get(x, h - 1)); }
    }
}
