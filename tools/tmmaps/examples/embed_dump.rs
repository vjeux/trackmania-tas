use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = MapFile::load(std::path::Path::new(&a[1]));
    let body = &m.gbx.body;
    if let Some((off, names)) = tmmaps::header::embedded_zip(body) {
        println!("zip at {off}, {} declared items:", names.len());
        for n in &names { println!("  {n}"); }
    } else { println!("no embedded zip"); }
    // raw head of the chunk
    let (_, _, payload, size) = *tmmaps::gbx::all_skip_chunks(body).iter().find(|(c, ..)| *c == 0x03043054).unwrap();
    println!("chunk size {size}; head {}", body[payload..payload + 96].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
    let s = String::from_utf8_lossy(&body[payload..payload + 400]).replace(|c: char| !c.is_ascii_graphic() && c != ' ', ".");
    println!("{s}");
}
