//! `edit_materials` over a corpus: the identity edit must reproduce the item
//! body byte for byte; a merging edit (every link collapsed onto two) must
//! give an item that re-parses, whose full graph walks, whose node count
//! dropped by the merged slots, and whose faces all index the new list.
use mapgeom::crystal::{edit_materials, ItemCrystal, MaterialSpec};
use std::path::{Path, PathBuf};
use tmmaps::gbx::Gbx;

fn collect(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_dir() {
        let mut ents: Vec<_> = std::fs::read_dir(p).unwrap().map(|e| e.unwrap().path()).collect();
        ents.sort();
        for e in ents {
            collect(&e, out);
        }
    } else if p.to_string_lossy().ends_with(".Item.Gbx") {
        out.push(p.to_path_buf());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut files = Vec::new();
    for a in &args {
        collect(Path::new(a), &mut files);
    }
    let (mut id_pass, mut merge_pass) = (0usize, 0usize);
    let mut fails = Vec::new();
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        let orig = Gbx::parse(&bytes);
        let same = edit_materials(&bytes, |m| m.clone());
        let g = Gbx::parse(&same);
        if g.body == orig.body && g.num_nodes == orig.num_nodes {
            id_pass += 1;
        } else {
            let first = g.body.iter().zip(orig.body.iter()).position(|(a, b)| a != b);
            fails.push(format!("{}: identity edit changed the body (first diff {:?}, len {} vs {})", f.display(), first, g.body.len(), orig.body.len()));
        }
        let merged = edit_materials(&bytes, |m| MaterialSpec {
            link: if m.physics % 2 == 0 { "Stadium\\Media\\Material\\RoadTech".into() } else { "Stadium\\Media\\Material\\PlatformTech".into() },
            physics: m.physics,
        });
        let it = match ItemCrystal::open(&merged) {
            Ok(it) => it,
            Err(e) => {
                fails.push(format!("{}: merged item does not parse: {e}", f.display()));
                continue;
            }
        };
        let old = ItemCrystal::open(&bytes).unwrap();
        let n_old = old.model.materials.len();
        let n_new = it.model.materials.len();
        let mut ok = true;
        if it.gbx.num_nodes as i64 != old.gbx.num_nodes as i64 - (n_old as i64 - n_new as i64) {
            fails.push(format!("{}: node count {} -> {} with {} -> {} materials", f.display(), old.gbx.num_nodes, it.gbx.num_nodes, n_old, n_new));
            ok = false;
        }
        if n_new > 2 || n_new == 0 {
            fails.push(format!("{}: merged to {n_new} materials", f.display()));
            ok = false;
        }
        for (li, l) in it.model.layers.iter().enumerate() {
            if let Some(c) = l.kind.crystal() {
                if c.faces.iter().any(|fc| fc.material < 0 || fc.material as usize >= n_new) {
                    fails.push(format!("{}: layer {li} has a face material out of range", f.display()));
                    ok = false;
                }
                if c.u02 != c.faces.iter().map(|f| f.material).max().unwrap_or(0) {
                    fails.push(format!("{}: layer {li} u02 stale", f.display()));
                    ok = false;
                }
            }
        }
        if it.model.layers.len() != old.model.layers.len() {
            fails.push(format!("{}: layer count changed", f.display()));
            ok = false;
        }
        match mapgeom::store::Model::parse(&merged, "item").and_then(|m| m.graph().map(|g| g.slots.len())) {
            Ok(_) => {}
            Err(e) => {
                fails.push(format!("{}: merged item graph walk failed: {e}", f.display()));
                ok = false;
            }
        }
        if ok {
            merge_pass += 1;
        }
    }
    println!("{} items: identity edit exact {}, merge edit ok {}, {} problems", files.len(), id_pass, merge_pass, fails.len());
    for f in fails.iter().take(40) {
        println!("FAIL {f}");
    }
}
