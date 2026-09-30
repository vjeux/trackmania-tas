//! `re17_cct MAP.Gbx` — the map's decompressed body around the `CCT_CustomColorTables` script-metadata key: hex + ASCII, and any
//! `#rrggbb` / JSON-looking text nearby (the 2026 Nations maps' custom item-colour tables; RE 17 2026-09-30 17:30Z).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).expect("map");
    let g = gbx::Gbx::parse(&data);
    let body = &g.body;
    let key = b"CCT_CustomColorTables";
    let mut found = 0;
    for i in 0..body.len().saturating_sub(key.len()) {
        if &body[i..i + key.len()] == key {
            found += 1;
            let lo = i.saturating_sub(48); let hi = (i + 1200).min(body.len());
            println!("== hit {found} at body offset {i} (body {} bytes)", body.len());
            for row in (lo..hi).step_by(32) {
                let end = (row + 32).min(hi);
                let hex: Vec<String> = body[row..end].iter().map(|b| format!("{b:02x}")).collect();
                let asc: String = body[row..end].iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                println!("{row:08x}  {:<96} {asc}", hex.join(" "));
            }
        }
    }
    if found == 0 { println!("no CCT_CustomColorTables in the decompressed body ({} bytes)", body.len()); }
}
