//! `tmmaps cpify`: give every TREE in a map its own checkpoint, and link only
//! the checkpoints that would otherwise crash the game.
//!
//! Why (Altrd_Amill101, 2026-10-18): collecting several checkpoints whose
//! trigger volumes INTERSECT crashes Trackmania — measured on the render box
//! that day: 2 and 3 overlapping are fine, **4 or more crash** (heap
//! corruption, 0xc0000374), stock 8 m Nadeo gates exactly as much as the
//! half-size Tiny Items ones, while 50 checkpoints in a row whose volumes do
//! not touch are fine and 50 overlapping ones tagged `LinkedCheckpoint` all
//! validate. So: a checkpoint per tree is only playable if the piles are
//! linked — and Editor++'s "Set All Checkpoints" links the WHOLE map into one
//! group, which makes every tree the same checkpoint.
//!
//! This builds the middle ground: one checkpoint per tree, clustered by actual
//! volume intersection, each cluster its own link order, isolated checkpoints
//! left unlinked.
//!
//!   tmmaps cpify MAP --out F [--item ITEM] [--author A] [--width M] [--depth M]
//!        [--filter PAT[,PAT…]] [--skip PAT[,PAT…]] [--variants N[,N…]]
//!        [--dy M] [--link-min N] [--order-base N] [--all-items] [--report TSV]
//!        [--variant-table TSV] [--types A,B] [--no-clumps] [--box X0,Y0,Z0:X1,Y1,Z1]
//!        [--trees TSV from `mapgeom block-trees` — the trees baked into the terrain]
//!        [--item-collection N: the checkpoint item's own collection, default 26 Stadium]
//!
//!   --item     the checkpoint item to place; default is the game's own
//!              `GateCheckpointCenter8mv2` (a STOCK pack item: nothing to
//!              embed, nothing to install). A loose set item is named by its
//!              path under `Items`, e.g.
//!              `TinyItems\Technics\Checkpoint\CPCenter\GateCheckpointCenter8mv2.Item.Gbx`
//!              with `--author <the item's author login>`.
//!   --width    the checkpoint's catch width in metres (default 8: the stock
//!              gate; the Tiny Items half-size copy is 4 — measured with
//!              `mapgeom item-check --facts`, which prints the waypoint
//!              trigger's bounds). Two checkpoints are in one cluster when
//!              their footprints overlap in x/z AND in height.
//!   --filter   only placements whose model name contains one of these
//!              (default: the Stadium vegetation that is a TREE — see
//!              `TREE_NAMES`; bushes, grass tufts and rocks are not in it)
//!   --skip     drop placements whose model name contains one of these
//!   --variants keep only these variant indices of a variant-list item (the
//!              `Spring`/`Summer`/`Fall`/`Winter` items are one model with
//!              several tree clumps behind a variant byte)
//!   --dy       lift each checkpoint by this many metres (default 0)
//!   --link-min a cluster of at least this many overlapping checkpoints is
//!              linked (default 2 — the safe reading of the measurement:
//!              link every pile, since 4 is where it was first seen to crash)
//!   --all-items  no name filter at all: a checkpoint on EVERY item placement
use crate::cli;
use crate::map::MapFile;
use std::path::{Path, PathBuf};

/// The Stadium vegetation items that are TREES (the item browser's
/// `Vegetation/…` leaves, minus the ground cover). A placement is a tree when
/// its model name contains one of these, so both `FallTreeBig` and a map's
/// `Fall` clump item match.
pub const TREE_NAMES: &[&str] = &[
    "Tree", // SpringTree*, FallTree*, FrozenTree*, PalmTree*, CherryTree*, WinterFrozenTree…
    "Fir", "Cypress", "Palm", "Cactus",
    // the four season clump items (`Spring`, `Summer`, `Fall`, `Winter`): one
    // model, several tree prefabs behind the placement's variant byte
    "Spring", "Summer", "Fall", "Winter",
];

/// Ground cover that is NOT a tree, whatever else its name says.
pub const NOT_TREE: &[&str] = &["Bush", "Grass", "Flower", "Fern", "Shrub", "Rock", "Stone", "Log", "Stump", "Leaf", "Hedge"];

/// The variant `Type` tags that are a TREE (`mapgeom item-variants` prints the
/// tag table of a pack item: the four season items are ONE model whose
/// placements pick a species through the variant byte — `Summer` 0..4 are
/// cacti, 5..7 firs and a cypress). A variant with no tags is a PREFAB clump
/// of several plants and counts as a tree.
pub const TREE_TYPES: &[&str] = &["Tree", "Fir", "Cypress", "Palm", "Cherry"];

/// (environment, item stem, variant) -> the variant's Type tag, from
/// `mapgeom item-variants`. The environment matters: `Forest` is 42 variants
/// in GreenCoast (14 Bush, 10 TreeThin, 7 Tree…) and 19 in WhiteShore (3 Bush,
/// 12 TreeFir, 4 TreePineDead), so variant 4 is a bush in one and a fir in the
/// other — reading the wrong table put checkpoints on the bushes of four Fall
/// 2026 maps (2026-10-18).
fn variant_table(path: &str) -> std::collections::HashMap<(String, String, u8), String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("--variant-table {path}: {e}"));
    let mut out = std::collections::HashMap::new();
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 4 {
            continue;
        }
        let Ok(v) = c[2].trim().parse::<u8>() else { continue };
        out.insert((c[0].trim().to_string(), c[1].trim().to_string(), v), c[3].trim().to_string());
    }
    out
}

/// A map's collection word -> the pack folder its items come from. Measured
/// from the Fall 2026 campaign (2026-10-18): the season's maps are spread over
/// four environments, and the species tables differ between them.
pub fn env_of_collection(c: u32) -> &'static str {
    match c {
        15 => "GreenCoast",
        16 => "RedIsland",
        26 | 25 => "Stadium",
        28 => "BlueBay",
        29 => "WhiteShore",
        _ => "-",
    }
}

struct Cp {
    /// index of the source placement in the map
    src: usize,
    model: String,
    variant: u8,
    species: String,
    pos: [f32; 3],
    yaw: f32,
}

pub fn cmd(args: &[String]) {
    // --batch DIR --out-dir DIR: every .Map.Gbx in DIR, same flags, each
    // written to OUT/<name> (a whole campaign in one call — Altrd's 25 Fall
    // 2026 maps, 2026-10-18)
    if let Some(dir) = cli::flag(args, "--batch") {
        let outdir = PathBuf::from(cli::flag(args, "--out-dir").expect("--batch needs --out-dir DIR"));
        std::fs::create_dir_all(&outdir).unwrap_or_else(|e| panic!("{}: {e}", outdir.display()));
        let mut maps: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.to_string_lossy().ends_with(".Map.Gbx"))
            .collect();
        maps.sort();
        assert!(!maps.is_empty(), "--batch {dir}: no .Map.Gbx in it");
        for m in &maps {
            let name = m.file_name().unwrap().to_string_lossy().to_string();
            let out = outdir.join(&name);
            let mut sub: Vec<String> = args.to_vec();
            sub[2] = m.to_string_lossy().to_string();
            // replace --out (and drop --batch/--out-dir) for this map
            let mut i = 0;
            while i < sub.len() {
                match sub[i].as_str() {
                    "--batch" | "--out-dir" | "--out" | "--report" => {
                        sub.drain(i..(i + 2).min(sub.len()));
                    }
                    _ => i += 1,
                }
            }
            sub.push("--out".into());
            sub.push(out.to_string_lossy().to_string());
            if let Some(rep) = cli::flag(args, "--report") {
                sub.push("--report".into());
                sub.push(format!("{rep}.{}.tsv", name.trim_end_matches(".Map.Gbx")));
            }
            one(&sub);
        }
        println!("batch: {} map(s) -> {}", maps.len(), outdir.display());
        return;
    }
    one(args);
}

fn one(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(cli::flag(args, "--out").expect("cpify needs --out MAP"));
    let item = cli::flag(args, "--item").unwrap_or("GateCheckpointCenter8mv2").to_string();
    // A placement resolves its model by the FULL identity: path + COLLECTION +
    // author. The appended records are CLONES of one of the map's own items, so
    // they inherit the MAP's collection — on a GreenCoast map (15) that made
    // every checkpoint "Missing Items: TinyItems\…" even with the file in
    // place, because our item declares Stadium (26). Always write the item's
    // own collection (2026-10-18, Altrd's load error).
    let item_collection: u32 = cli::flag(args, "--item-collection").unwrap_or("26").parse().expect("--item-collection N");
    let author = cli::flag(args, "--author").unwrap_or("Nadeo").to_string();
    let width: f32 = cli::flag(args, "--width").unwrap_or("8").parse().expect("--width metres");
    // the gate's catch slab is thin along its own axis; keep a little margin so
    // two slabs that merely touch count as one cluster
    let depth: f32 = cli::flag(args, "--depth").unwrap_or("1").parse().expect("--depth metres");
    let height: f32 = cli::flag(args, "--height").unwrap_or("4").parse().expect("--height metres");
    let dy: f32 = cli::flag(args, "--dy").unwrap_or("0").parse().expect("--dy metres");
    let link_min: usize = cli::flag(args, "--link-min").unwrap_or("2").parse().expect("--link-min N");
    let order_base: u32 = cli::flag(args, "--order-base").unwrap_or("1").parse().expect("--order-base N");
    let all_items = args.iter().any(|a| a == "--all-items");
    let list: Vec<String> = cli::flag(args, "--filter").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_else(|| TREE_NAMES.iter().map(|s| s.to_string()).collect());
    let skip: Vec<String> = cli::flag(args, "--skip").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_else(|| NOT_TREE.iter().map(|s| s.to_string()).collect());
    let variants: Option<Vec<u8>> = cli::flag(args, "--variants").map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    // --trees TSV: the trees BAKED INTO THE TERRAIN, from `mapgeom
    // block-trees` (block, cell x/y/z, dir, prefab, model, x, y, z). They are
    // in no item list — BlueBay's jungle hills carry their palms inside the
    // zone prefab — so without this they get no checkpoint (Altrd, 2026-10-18;
    // 13 801 of them on Fall 2026 - 04, each verified to sit on the grass
    // surface under it).
    let baked: Vec<Cp> = match cli::flag(args, "--trees") {
        Some(f) => {
            let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("--trees {f}: {e}"));
            text.lines()
                .skip(1)
                .filter_map(|l| {
                    let c: Vec<&str> = l.split('\t').collect();
                    if c.len() < 10 {
                        return None;
                    }
                    let (x, y, z) = (c[7].trim().parse().ok()?, c[8].trim().parse().ok()?, c[9].trim().parse().ok()?);
                    Some(Cp { src: usize::MAX, model: format!("{}:{}", c[0].trim(), c[6].trim()), variant: 0, species: "baked".into(), pos: [x, y, z], yaw: 0.0 })
                })
                .collect()
        }
        None => Vec::new(),
    };
    // the species table from `mapgeom item-variants`: with it, a season item's
    // placements are judged by their own variant's Type tag (tree vs cactus vs
    // ground cover) instead of by the model name alone
    let table = cli::flag(args, "--variant-table").map(variant_table);
    let types: Vec<String> = cli::flag(args, "--types").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_else(|| TREE_TYPES.iter().map(|s| s.to_string()).collect());
    let clumps = !args.iter().any(|a| a == "--no-clumps");
    // --box X0,Y0,Z0:X1,Y1,Z1 — only placements inside this box get a
    // checkpoint (a corner of the map, or the one clump you are testing)
    let bbox: Option<([f32; 3], [f32; 3])> = cli::flag(args, "--box").map(|s| {
        let (a, b) = s.split_once(':').expect("--box X0,Y0,Z0:X1,Y1,Z1");
        let p = |t: &str| -> [f32; 3] {
            let v: Vec<f32> = t.split(',').map(|x| x.trim().parse().expect("--box wants numbers")).collect();
            assert_eq!(v.len(), 3, "--box wants x,y,z on each side");
            [v[0], v[1], v[2]]
        };
        let (lo, hi) = (p(a), p(b));
        ([lo[0].min(hi[0]), lo[1].min(hi[1]), lo[2].min(hi[2])], [lo[0].max(hi[0]), lo[1].max(hi[1]), lo[2].max(hi[2])])
    });

    let base = MapFile::load(&src);
    let n0 = base.items.len();

    // A link order is shared by the whole linked group, so ours must not
    // collide with a group the map already has (its own linked checkpoints
    // keep their order) — start above the highest one in the file unless the
    // caller names a base.
    let used_max = base
        .items
        .iter()
        .filter(|it| it.waypoint_tag.as_deref() == Some("LinkedCheckpoint"))
        .map(|it| it.waypoint_order)
        .max();
    let mut next_order = match cli::flag(args, "--order-base") {
        Some(_) => order_base,
        None => used_max.map(|m| m + 1).unwrap_or(order_base),
    };
    let mut picked: Vec<Cp> = Vec::new();
    for it in &base.items {
        let name = it.model.rsplit('\\').next().unwrap_or(&it.model).trim_end_matches(".Item.Gbx").to_string();
        let keep = all_items || (list.iter().any(|p| name.contains(p.as_str())) && !skip.iter().any(|p| name.contains(p.as_str())));
        if !keep {
            continue;
        }
        if let Some((lo, hi)) = &bbox {
            let p = it.pos;
            if p[0] < lo[0] || p[0] > hi[0] || p[1] < lo[1] || p[1] > hi[1] || p[2] < lo[2] || p[2] > hi[2] {
                continue;
            }
        }
        if let Some(v) = &variants {
            if !v.contains(&it.variant()) {
                continue;
            }
        }
        // the species table decides for a variant-list item: a `Summer`
        // placement is a fir or a cactus depending on its variant byte
        let mut species = String::from("-");
        if let Some(t) = &table {
            // the map's own environment first; a stem only one environment has
            // (the Stadium season items) resolves whatever the map's collection
            // each PLACEMENT names its own item's collection — a GreenCoast
            // map holds Stadium items too, so the map-wide collection is the
            // wrong key (2026-10-18: reading item 0's collection put the
            // whole of Fall 02 on the wrong species table)
            let env = env_of_collection(it.collection_raw);
            let hit = t.get(&(env.to_string(), name.clone(), it.variant())).or_else(|| {
                let mut found = None;
                for ((_, stem, v), ty) in t.iter() {
                    if stem == &name && *v == it.variant() {
                        if found.is_some() {
                            return None; // ambiguous across environments
                        }
                        found = Some(ty);
                    }
                }
                found
            });
            if let Some(ty) = hit {
                species = ty.clone();
                let is_clump = ty == "-" || ty.is_empty();
                let wanted = if is_clump { clumps } else { types.iter().any(|k| ty.contains(k.as_str())) };
                if !wanted {
                    continue;
                }
            }
        }
        picked.push(Cp { src: it.index, model: name, variant: it.variant(), species, pos: [it.pos[0], it.pos[1] + dy, it.pos[2]], yaw: it.yaw });
    }
    let baked: Vec<Cp> = match &bbox {
        Some((lo, hi)) => baked.into_iter().filter(|c| c.pos[0] >= lo[0] && c.pos[0] <= hi[0] && c.pos[1] >= lo[1] && c.pos[1] <= hi[1] && c.pos[2] >= lo[2] && c.pos[2] <= hi[2]).collect(),
        None => baked,
    };
    let n_baked = baked.len();
    picked.extend(baked);
    if picked.is_empty() {
        eprintln!("cpify: no placement of {} matches (filter {:?}, skip {:?})", src.display(), list, skip);
        std::process::exit(1);
    }

    // --- clusters: checkpoints whose catch volumes intersect --------------
    // The volume is the gate's slab: `width` across, `depth` through, `height`
    // tall, turned by the placement's yaw. Two are taken to intersect when
    // their yaw-aware footprints overlap — the slab is approximated by the
    // circle through its corners, which is what makes this conservative: it
    // never calls an overlapping pair separate.
    let r = ((width * width + depth * depth).sqrt()) / 2.0;
    let mut group: Vec<usize> = (0..picked.len()).collect();
    fn find(group: &mut Vec<usize>, i: usize) -> usize {
        let mut i = i;
        while group[i] != i {
            group[i] = group[group[i]];
            i = group[i];
        }
        i
    }
    for i in 0..picked.len() {
        for j in (i + 1)..picked.len() {
            let (a, b) = (&picked[i], &picked[j]);
            let dx = a.pos[0] - b.pos[0];
            let dz = a.pos[2] - b.pos[2];
            let dyy = (a.pos[1] - b.pos[1]).abs();
            if dx * dx + dz * dz <= (2.0 * r) * (2.0 * r) && dyy < height {
                let (ra, rb) = (find(&mut group, i), find(&mut group, j));
                if ra != rb {
                    group[ra] = rb;
                }
            }
        }
    }
    let mut sizes: std::collections::HashMap<usize, usize> = Default::default();
    for i in 0..picked.len() {
        let root = find(&mut group, i);
        *sizes.entry(root).or_insert(0) += 1;
    }
    // one link order per cluster that needs linking, in map order
    let mut order_of: std::collections::HashMap<usize, u32> = Default::default();
    for i in 0..picked.len() {
        let root = find(&mut group, i);
        if sizes[&root] >= link_min && !order_of.contains_key(&root) {
            order_of.insert(root, next_order);
            next_order += 1;
        }
    }

    // --- write: the whole map, plus one checkpoint record per pick --------
    let grown = out.with_extension("grown.Map.Gbx");
    let mut mg = MapFile::load(&src);
    mg.append_item_clones(n0 + picked.len());
    mg.write_to(&grown).expect("write slots");
    let mut m = MapFile::load(&grown);
    assert!(m.items.len() >= n0 + picked.len(), "map grew to {} item records, {} needed", m.items.len(), n0 + picked.len());
    let cell = |p: [f32; 3]| ((p[0] / 32.0) as i32, ((p[1] + 64.0) / 8.0) as i32, (p[2] / 32.0) as i32);
    let mut tags: Vec<(usize, String, u32)> = Vec::new();
    for (k, cp) in picked.iter().enumerate() {
        let i = n0 + k;
        m.move_item(i, cp.pos, cp.yaw, cell(cp.pos));
        m.set_item_frame(i, [cp.yaw, 0.0, 0.0], [0.0; 3]);
        m.set_item_scale(i, 1.0);
        m.set_item_model(i, &item);
        m.set_item_author(i, &author);
        m.set_item_collection(i, item_collection);
        // the donor record's variant byte means nothing on the gate and breaks
        // the placement (2026-09-23: no collision, a hung Validate)
        m.clear_item_variant(i);
        let root = find(&mut group, k);
        match order_of.get(&root) {
            Some(o) => tags.push((i, "LinkedCheckpoint".into(), *o)),
            None => tags.push((i, "Checkpoint".into(), 0)),
        }
    }
    let stage = out.with_extension("stage.Map.Gbx");
    m.write_to(&stage).expect("write models");
    // the waypoint properties are variable-length splices: a second pass over
    // the written file, after the model renames
    let mut m2 = MapFile::load(&stage);
    for (i, tag, order) in &tags {
        m2.set_item_waypoint(*i, Some(tag), *order);
    }
    m2.remove_password();
    m2.write_to(&out).expect("write map");
    let _ = std::fs::remove_file(&stage);
    let _ = std::fs::remove_file(&grown);

    let linked = tags.iter().filter(|(_, t, _)| t == "LinkedCheckpoint").count();
    let groups = order_of.len();
    let biggest = sizes.values().copied().max().unwrap_or(0);
    println!(
        "{}: {} item placements{} -> {} checkpoints ({} item), {} linked in {} group(s), {} standalone; biggest pile {}",
        src.display(),
        n0,
        if n_baked > 0 { format!(" + {n_baked} baked trees") } else { String::new() },
        picked.len(),
        item,
        linked,
        groups,
        picked.len() - linked,
        biggest
    );
    if let Some(rep) = cli::flag(args, "--report") {
        let mut t = String::from("cp_index\tsource_item\tmodel\tvariant\tspecies\tx\ty\tz\tyaw\ttag\torder\tcluster_size\n");
        for (k, cp) in picked.iter().enumerate() {
            let root = find(&mut group, k);
            let (tag, order) = match order_of.get(&root) {
                Some(o) => ("LinkedCheckpoint", *o),
                None => ("Checkpoint", 0),
            };
            t.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{tag}\t{order}\t{}\n",
                n0 + k,
                cp.src,
                cp.model,
                cp.variant,
                cp.species,
                cp.pos[0],
                cp.pos[1],
                cp.pos[2],
                cp.yaw,
                sizes[&root]
            ));
        }
        std::fs::write(Path::new(&rep), t).unwrap_or_else(|e| panic!("{rep}: {e}"));
        println!("  report {rep}");
    }
    println!("  wrote {}", out.display());
}
