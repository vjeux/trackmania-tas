fn main() {
    let a: Vec<String> = std::env::args().collect();
    let b = std::fs::read(&a[1]).unwrap();
    println!("{:?}", tmmaps::header::item_ident_author(&b));
}
