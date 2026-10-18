//! `mapgeom block-trees MAP --out TSV [--collection BlueBay] [--yoff N]`:
//! where the trees that are BAKED INTO THE TERRAIN stand, in world metres.
//!
//! Why (Altrd_Amill101, 2026-10-18): a checkpoint per tree is easy for the
//! trees a map PLACES as items, and impossible from the map file alone for
//! the ones that come with the ground — BlueBay's jungle hills carry their
//! palms inside the zone prefab (`Zone\LandHill6\Base1A.Prefab.Gbx`: 232
//! entities, most of them `VegetTreeModel` references). Those trees are in no
//! item list, so `tmmaps cpify` cannot see them.
//!
//! They ARE addressable, though: the block info names the mobil prefab per
//! variant, the prefab's entities carry local positions, and the block's grid
//! placement turns local into world. This walks that chain for every authored
//! block of a map and prints one row per baked tree — which `tmmaps cpify
//! --trees TSV` then turns into checkpoints.
use crate::blockmap;
use crate::place;
use crate::store::DataStore;

/// One tree the terrain carries, in world metres.
pub struct BakedTree {
    pub block: String,
    pub cell: [i32; 3],
    pub dir: u8,
    pub prefab: String,
    pub model: String,
    pub pos: [f32; 3],
}

/// Does this external reference name a tree model?
pub fn is_tree_ref(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    // the species live in VegetTreeModel (a swaying tree) or as static
    // vegetation objects; bushes and grass tufts carry it in the name
    let veget = p.contains("vegettreemodel") || p.contains("\\vegetation\\");
    if !veget {
        return false;
    }
    let stem = p.rsplit('\\').next().unwrap_or(&p);
    !(stem.contains("bush") || stem.contains("grass") || stem.contains("flower") || stem.contains("fern") || stem.contains("shrub"))
}

pub fn run(store: &mut DataStore, rest: &[String]) -> Result<(), String> {
    let flag = |n: &str| rest.iter().position(|a| a == n).and_then(|i| rest.get(i + 1)).cloned();
    let path = rest.get(1).filter(|a| !a.starts_with("--")).cloned().ok_or("block-trees MAP --out TSV")?;
    // the map's own collection word names its environment when --collection
    // is not given (0xF GreenCoast, 0x10 RedIsland, 0x1A Stadium, 0x1C
    // BlueBay, 0x1D WhiteShore — measured on the Fall 2026 campaign)
    let collection = flag("--collection").unwrap_or_default();
    let yoff: f32 = flag("--yoff").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let m = tmmaps::map::MapFile::load(std::path::Path::new(&path));
    let collection = if collection.is_empty() {
        let c = m.items.first().map(|it| it.collection_raw).unwrap_or(28);
        match c {
            15 => "GreenCoast",
            16 => "RedIsland",
            25 | 26 => "Stadium",
            29 => "WhiteShore",
            _ => "BlueBay",
        }
        .to_string()
    } else {
        collection
    };
    // authored blocks only: a baked record is a generated clip filler and
    // carries no vegetation of its own
    let (placements, _) = blockmap::walk(store, &m, false, &collection);

    // prefab -> its tree entities (local position, model path), read once
    let mut cache: std::collections::HashMap<String, Vec<([f32; 3], String)>> = Default::default();
    let mut trees: Vec<BakedTree> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for p in placements.iter().filter(|p| !p.baked) {
        if p.free {
            // a free-placed terrain block is not a thing; skip rather than
            // place its trees at the wrong spot
            continue;
        }
        for prefab in &p.prefabs {
            if !cache.contains_key(prefab) {
                let found = match prefab_trees(store, prefab) {
                    Ok(v) => v,
                    Err(e) => {
                        missing.push(format!("{prefab}: {e}"));
                        Vec::new()
                    }
                };
                cache.insert(prefab.clone(), found);
            }
            let Some(list) = cache.get(prefab) else { continue };
            if list.is_empty() {
                continue;
            }
            // the block's grid transform: its own cell, its direction, and the
            // footprint the rotation turns about (place::grid_block)
            let size = (32.0 * p.cells.iter().map(|(_, c)| c[0]).max().map(|v| v as f32 + 1.0).unwrap_or(1.0), 32.0 * p.cells.iter().map(|(_, c)| c[2]).max().map(|v| v as f32 + 1.0).unwrap_or(1.0));
            let xf = place::grid_block((p.cell[0], p.cell[1], p.cell[2]), p.dir, size, yoff);
            for (local, model) in list {
                let w = crate::geom::apply(&xf, *local);
                trees.push(BakedTree { block: p.name.clone(), cell: p.cell, dir: p.dir, prefab: prefab.clone(), model: model.clone(), pos: w });
            }
        }
    }

    let mut t = String::from("block\tcell_x\tcell_y\tcell_z\tdir\tprefab\tmodel\tx\ty\tz\n");
    for b in &trees {
        t.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\n",
            b.block,
            b.cell[0],
            b.cell[1],
            b.cell[2],
            b.dir,
            b.prefab.rsplit('\\').next().unwrap_or(&b.prefab),
            b.model.rsplit('\\').next().unwrap_or(&b.model).trim_end_matches(".VegetTreeModel.Gbx"),
            b.pos[0],
            b.pos[1],
            b.pos[2]
        ));
    }
    if let Some(out) = flag("--out") {
        std::fs::write(&out, &t).map_err(|e| format!("{out}: {e}"))?;
        println!("wrote {out}");
    } else {
        print!("{t}");
    }
    let blocks: std::collections::BTreeMap<&str, usize> = trees.iter().fold(Default::default(), |mut a, b| {
        *a.entry(b.block.as_str()).or_insert(0) += 1;
        a
    });
    println!("{}: {} baked trees in {} block kind(s) ({} authored blocks walked)", path, trees.len(), blocks.len(), placements.iter().filter(|p| !p.baked).count());
    for (k, n) in blocks.iter().take(12) {
        println!("  {k}: {n}");
    }
    for e in missing.iter().take(5) {
        println!("  unread prefab {e}");
    }
    Ok(())
}

/// The tree entities of one zone prefab: (local position, model path).
fn prefab_trees(store: &mut DataStore, path: &str) -> Result<Vec<([f32; 3], String)>, String> {
    let model = store.load_model(path)?;
    let externals: std::collections::HashMap<u32, String> = model.externals.iter().map(|(i, e)| (*i, e.clone())).collect();
    // `load_model` hands back the body WITHOUT the class id: a prefab body
    // opens with its own version word (11 on the BlueBay zone prefabs)
    if model.body.len() < 4 {
        return Err("empty body".into());
    }
    let p = crate::static_item::prefab::CPlugPrefab::parse(&model.body, &model.external_indices())?;
    let mut out = Vec::new();
    for e in &p.ents {
        if e.model.index < 0 {
            continue;
        }
        let Some(name) = externals.get(&(e.model.index as u32)) else { continue };
        if is_tree_ref(name) {
            out.push((e.pos, name.clone()));
        }
    }
    Ok(out)
}
