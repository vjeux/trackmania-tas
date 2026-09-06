fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut st = mapgeom::store::DataStore::empty();
    st.add_pak(&a[1], &a[2]).unwrap();
    let data = st.read(&a[3]).unwrap();
    std::fs::write(&a[4], &data).unwrap();
    println!("wrote {} ({} bytes)", &a[4], data.len());
}
