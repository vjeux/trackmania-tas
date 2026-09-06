//! Find Nadeo items with exactly a given material-stem set.
//! Usage: matset NADEO.zip "stem1,stem2,..."
use std::collections::BTreeSet;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let want: BTreeSet<String> = a[2].split(',').map(|s| s.to_string()).collect();
    let zip = std::fs::read(&a[1]).expect("nadeo zip");
    let files = mapgeom::embedded::unzip(&zip).expect("zip");
    for (name, bytes) in &files {
        if !name.ends_with(".Item.Gbx") {
            continue;
        }
        let (mats, _) = mapgeom::crystal::decode_template(bytes);
        let set: BTreeSet<String> = mats.iter().map(|m| m.link.rsplit('\\').next().unwrap_or(&m.link).to_string()).collect();
        if want.iter().all(|w| set.contains(w)) {
            let extra: Vec<_> = set.difference(&want).collect();
            println!("{name} extra={extra:?}");
        }
    }
}
