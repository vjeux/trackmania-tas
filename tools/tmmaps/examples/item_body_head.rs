use tmmaps::gbx::Gbx;
fn main() {
    for f in std::env::args().skip(1) {
        let g = Gbx::parse(&std::fs::read(&f).unwrap());
        let b = &g.body;
        println!("### {f}: body {} bytes", b.len());
        for row in (0..160.min(b.len())).step_by(16) {
            let end = (row + 16).min(b.len());
            let hex: Vec<String> = b[row..end].iter().map(|x| format!("{x:02x}")).collect();
            let asc: String = b[row..end].iter().map(|&x| if (32..127).contains(&x) { x as char } else { '.' }).collect();
            println!("  {row:4}: {:<48} {asc}", hex.join(" "));
        }
    }
}
