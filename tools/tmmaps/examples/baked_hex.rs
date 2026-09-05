use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = MapFile::load(std::path::Path::new(&a[1]));
    let body = &m.gbx.body;
    let (_, off, payload, size) = *tmmaps::gbx::all_skip_chunks(body).iter().find(|(c, ..)| *c == 0x03043048).unwrap();
    println!("chunk off {off} payload {payload} size {size}; baked {} records", m.baked.len());
    println!("head: {}", body[payload..payload + 48].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
    for b in m.baked.iter().take(4).chain(m.baked.iter().skip(1890).take(2)) {
        let s = b.coord_off - 5;
        println!("rec {:>5} {:<28} coord_off {} bytes {}", b.index, b.name, b.coord_off, body[s..s + 14].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "));
    }
    let last = m.baked.last().unwrap();
    println!("last rec {} {} coord_off {} ; payload end {}", last.index, last.name, last.coord_off, payload + size);
    println!("tail: {}", body[payload + size - 24..payload + size].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
}
