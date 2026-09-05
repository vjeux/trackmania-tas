use mapgeom::store::DataStore;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut st = DataStore::empty();
    st.add_pak(&a[1], &a[2]).unwrap();
    for e in st.entries() {
        let p = e.path();
        if a[3..].iter().any(|q| p.to_uppercase().contains(&q.to_uppercase())) {
            println!("{p}: class {:08X} off {} usize {} csize {} size {} flags {:#018x} compressed {} public {}", e.class_id, e.offset, e.uncompressed_size, e.compressed_size, e.size, e.flags, e.is_compressed(), e.public_file());
        }
    }
}
