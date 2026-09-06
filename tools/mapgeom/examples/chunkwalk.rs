fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    for c in &f.item.chunks {
        let s = format!("{c:?}");
        // trim huge nested mesh output: keep first 600 chars
        let t: String = s.chars().take(600).collect();
        println!("0x{:08X} {}", c.id(), t);
    }
}
