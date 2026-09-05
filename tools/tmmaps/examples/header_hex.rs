use tmmaps::gbx::Gbx;
fn main() {
    for f in std::env::args().skip(1) {
        let g = Gbx::parse(&std::fs::read(&f).unwrap());
        let ud = &g.user_data;
        let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
        let mut off = 4 + n * 8;
        println!("### {f} header {} bytes, body {} bytes, version {}", ud.len(), g.body.len(), g.version);
        for i in 0..n {
            let id = u32::from_le_bytes(ud[4 + i * 8..8 + i * 8].try_into().unwrap());
            let raw = u32::from_le_bytes(ud[8 + i * 8..12 + i * 8].try_into().unwrap());
            let size = (raw & 0x7FFF_FFFF) as usize;
            println!("  chunk {id:08X} size {size} heavy={}", raw >> 31);
            if id == 0x2E001003 {
                let d = &ud[off..off + size];
                let hex: Vec<String> = d.iter().map(|x| format!("{x:02x}")).collect();
                println!("    {}", hex.join(" "));
                println!("    {}", d.iter().map(|&x| if (32..127).contains(&x) { x as char } else { '.' }).collect::<String>());
            }
            off += size;
        }
    }
}
