//! `e7_pakls --pak FILE:KEY … [--grep SUBSTR …] [--dump LOGICAL OUT]` — the packs' entry list (folder\name, class id, sizes) filtered by
//! case-insensitive substrings; --dump writes one entry's bytes (E7, 2026-09-30: where WhiteShore keeps its colour palette / tables).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    let (mut greps, mut dump): (Vec<String>, Option<(String, String)>) = (Vec::new(), None);
    let mut i = 1;
    while i < a.len() {
        match a[i].as_str() {
            "--pak" => { if let Some((p, k)) = a.get(i + 1).and_then(|s| s.rsplit_once(':')) { store.add_pak(p, k).unwrap_or_else(|e| panic!("--pak {p}: {e}")); } i += 2; }
            "--grep" => { greps.push(a[i + 1].to_ascii_lowercase()); i += 2; }
            "--dump" => { dump = Some((a[i + 1].clone(), a[i + 2].clone())); i += 3; }
            "--load" => {
                // parse one entry as a model (MAPGEOM_TRACE=1 prints the chunk walk) and print its graph summary
                let l = a[i + 1].clone();
                match store.load_model(&l) { Ok(m) => { println!("{l}: {} externals: {:?}", m.externals.len(), m.externals); match m.graph() { Ok(g) => println!("  root {:?}", g.root.as_ref().map(|n| format!("{n:?}").chars().take(600).collect::<String>())), Err(e) => println!("  graph: {e}") } } Err(e) => println!("{l}: {e}") }
                return;
            }
            "--cat-class" => {
                // print every entry of the given class id (hex) as text (the colour-table JSONs are class 0915E000)
                let cid = u32::from_str_radix(&a[i + 1], 16).expect("--cat-class HEX");
                let names: Vec<String> = store.entries().filter(|e| e.class_id == cid).map(|e| e.path()).collect();
                for n in names { match store.read_entry(&n) { Ok(b) => println!("== {n} ({} B)\n{}", b.len(), String::from_utf8_lossy(&b)), Err(e) => println!("== {n}: {e}") } }
                return;
            }
            _ => i += 1,
        }
    }
    if let Some((logical, out)) = dump {
        let b = store.read(&logical).unwrap_or_else(|e| panic!("{logical}: {e}"));
        std::fs::write(&out, &b[..]).expect("write");
        eprintln!("{logical}: {} B → {out}", b.len());
        return;
    }
    let mut n = 0;
    for e in store.entries() {
        let full = format!("{}{}", e.folder, e.name);
        let l = full.to_ascii_lowercase();
        if !greps.is_empty() && !greps.iter().any(|g| l.contains(g.as_str())) { continue; }
        println!("{}\tclass {:08x}\t{} B", e.path(), e.class_id, e.uncompressed_size);
        n += 1;
    }
    eprintln!("{n} entries");
}
