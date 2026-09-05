use mapgeom::tiny_assets;
// ident_rename IN OUT NAME AUTHOR [body]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let b = std::fs::read(&a[1]).unwrap();
    let mut out = tiny_assets::set_header_ident(&b, &a[3], &a[4]);
    match a.get(5).map(|s| s.as_str()) {
        Some("body") => out = tiny_assets::set_body_ident_nameless(&out, &a[3]),
        Some("insert") => out = tiny_assets::set_body_ident_insert(&out, &a[3]),
        _ => {}
    }
    if let Some(c) = a.get(6) { out = tiny_assets::set_ident_collection(&out, u32::from_str_radix(c, 16).unwrap()); }
    std::fs::write(&a[2], out).unwrap();
}
