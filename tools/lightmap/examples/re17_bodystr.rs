// re17 bodystr: strings (len ≥ 4) in a map's DECOMPRESSED body matching a case-insensitive needle, with 24 bytes of hex context before/after
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).expect("map");
    let g = gbx::Gbx::parse(&data);
    let body = &g.body;
    let needle = a[2].to_ascii_lowercase().into_bytes();
    let mut n = 0;
    for i in 0..body.len().saturating_sub(needle.len()) {
        if body[i..i + needle.len()].iter().zip(needle.iter()).all(|(b, c)| b.to_ascii_lowercase() == *c) {
            n += 1; if n > 40 { break; }
            let lo = i.saturating_sub(24); let hi = (i + needle.len() + 40).min(body.len());
            let asc: String = body[lo..hi].iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
            let hex: Vec<String> = body[lo..i].iter().map(|b| format!("{b:02x}")).collect();
            println!("{i:08x}: {asc}   [before: {}]", hex.join(" "));
        }
    }
    println!("{n} hits for {:?} in {} body bytes", a[2], body.len());
}
