//! Decompress a GBX file's body to a file, printing where the crystal chunks sit.
use tmmaps::gbx::Gbx;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&a[1]).unwrap();
    let g = Gbx::parse(&bytes);
    std::fs::write(&a[2], &g.body).unwrap();
    println!("class 0x{:08X} num_nodes {} body {} bytes, user_data {} bytes", g.class_id, g.num_nodes, g.body.len(), g.user_data.len());
    for id in [0x2E00100Bu32, 0x2E002019, 0x2E026000, 0x09003003, 0x09003004, 0x09003005, 0x09003006, 0x09003007, 0x090FD000, 0x090FD001, 0x090FD002] {
        let pat = id.to_le_bytes();
        let hits: Vec<usize> = g.body.windows(4).enumerate().filter(|(_, w)| *w == pat).map(|(i, _)| i).collect();
        println!("0x{id:08X}: {:x?}", hits);
    }
}
