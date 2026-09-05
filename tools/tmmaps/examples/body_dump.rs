use tmmaps::gbx::Gbx;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g = Gbx::parse(&std::fs::read(&a[1]).unwrap());
    std::fs::write(&a[2], &g.body).unwrap();
    println!("body {} bytes, ref table {} bytes, num_nodes {}", g.body.len(), g.ref_table.len(), g.num_nodes);
}
