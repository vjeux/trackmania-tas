//! `mapgeom tileset SOURCE.Map.Gbx CONVERTED.Map.Gbx --out-dir DIR [--prefix Dirt]
//! [--author NAME] [--collection 26]` — a hand-assembly BLOCKSET from the game's own
//! connected geometry (Everios96 2026-10-27: "variations of the flat water blocks and
//! water road blocks without the water … like as a blockset … so later as a set I can put
//! them together by hand").
//!
//! The look of a water block depends on its neighbours: the block's own prefab is next
//! to nothing (a `PlatformWaterBase` is a water sheet and a collision plane), and every
//! wall, rim, floor and junction is a CLIP FILLER the editor generates per side from what
//! stands across it, picked among several variants by the wider neighbourhood. So the
//! set is HARVESTED, not designed:
//!
//! 1. `SOURCE` is a pattern map saved by the editor (`shootctl probe --how edit` +
//!    `mapsave`), so its baked records are the game's fillers for every pattern laid out
//!    in it (pools 1x1, 1x2, 1x3, 2x2, 3x3, L, plus; channels; curves with straights on
//!    their ends; T; cross).
//! 2. `CONVERTED` is `tinyctl surface … --surface S` of SOURCE with
//!    `TINY_WATER_FORCE_BAKE=PlatformWater,RoadWater`: every block and every filler is an
//!    S-dressed item (water sheets off) placed where the record stood.
//! 3. Here, every converted item is assigned to the authored block whose FOOTPRINT holds
//!    the item's geometry (its collision centroid; the FCB skirts below a block count for
//!    the block above): a junction piece lies in the cell it protrudes into, so every
//!    piece goes to exactly one tile and the tiles of a pattern reassemble it exactly.
//! 4. Per block, the assigned items are merged in the block's LOCAL frame (the dir-0
//!    cell corner is the pivot) into one static item, grid 32 × 8 m like a block. Blocks
//!    whose merged content is the same (same pieces at the same local places) share one
//!    tile; the tile is named by the block and its connectivity (which local sides have
//!    a same-family neighbour), with a letter when two tiles share the connectivity but
//!    differ in content (the diagonal-dependent junction variants).
//!
//! Output: `DIR/<prefix>_<Name>.Item.Gbx` per tile, `DIR/tiles.tsv` (tile, blocks,
//! pieces, sides), and `DIR/reassembly.txt` — one `tmmaps add-items` spec per source
//! block (`ITEM@x,y,z@dir@0`) that rebuilds the whole pattern map from the tiles, the
//! check that the set is complete.

use crate::geom::{apply, compose, Xform, IDENTITY};
use crate::static_item::build::{assemble, BuildOpts, Merged};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

type R<T> = Result<T, String>;

fn flag<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

/// The inverse of a rigid transform (rotation columns + translation).
fn invert(m: &Xform) -> Xform {
    // columns c0 = m[0..3], c1 = m[3..6], c2 = m[6..9] (apply: x' = m0 x + m3 y + m6 z + m9);
    // R⁻¹ = Rᵀ, whose columns are R's rows
    let r_inv = [m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]];
    let t = [m[9], m[10], m[11]];
    let ti = [
        -(r_inv[0] * t[0] + r_inv[3] * t[1] + r_inv[6] * t[2]),
        -(r_inv[1] * t[0] + r_inv[4] * t[1] + r_inv[7] * t[2]),
        -(r_inv[2] * t[0] + r_inv[5] * t[1] + r_inv[8] * t[2]),
    ];
    [r_inv[0], r_inv[1], r_inv[2], r_inv[3], r_inv[4], r_inv[5], r_inv[6], r_inv[7], r_inv[8], ti[0], ti[1], ti[2]]
}

/// Side index of a unit vector in the ground plane, by `tmmaps::fillers::SIDE_VEC`
/// (N +z, E −x, S −z, W +x).
fn side_of(v: [f32; 3]) -> Option<usize> {
    tmmaps::fillers::SIDE_VEC.iter().position(|(dx, dz)| (v[0] - *dx as f32).abs() < 0.01 && (v[2] - *dz as f32).abs() < 0.01)
}

struct Placed {
    stem: String,
    world: Xform,
    centroid: [f32; 3],
    yaw: f32,
}

struct Block<'a> {
    rec: &'a tmmaps::map::BlockRec,
    base: Xform,
    /// footprint size in cells (dir-0 frame)
    size: (i32, i32),
    /// extra quarter turns chosen for the canonical frame (single-cell blocks: the
    /// rotation that makes the content signature smallest, so one tile serves every
    /// orientation of the same shape)
    turn: u8,
    cells: Vec<(i32, i32)>,
    /// which LOCAL sides (N,E,S,W of the dir-0 frame) see a same-family block
    open: [bool; 4],
    /// same-family block at the local diagonals NE, SE, SW, NW
    diag: [bool; 4],
    items: Vec<usize>,
}

/// sides: N=0 (+z), E=1 (−x), S=2 (−z), W=3 (+x); diagonals: NE=0, SE=1, SW=2, NW=3
fn canonical_open(open: &[bool; 4]) -> bool {
    matches!(
        open,
        [false, false, false, false]        // island
            | [false, false, false, true]   // end: open W
            | [true, false, false, true]    // corner: open N + W
            | [true, false, true, false]    // channel: open N + S
            | [true, false, true, true]     // edge: open N + S + W (closed E)
            | [true, true, true, true]      // middle
    )
}

/// The diagonals whose water changes a pool cell's pieces: for every CLOSED side, the
/// diagonal at each of its ends that lies behind an OPEN side (the rim piece's end is
/// `Str` without that water, `Out` with it; an end at another closed side is `In`
/// regardless). Indices into `diag` (NE=0 between N and E, SE=1, SW=2, NW=3).
fn relevant_diagonals(open: &[bool; 4]) -> Vec<usize> {
    // diagonal d lies between side d and side (d+1)%4
    let mut v = Vec::new();
    for d in 0..4 {
        let (a, b) = (open[d], open[(d + 1) % 4]);
        if a != b {
            v.push(d);
        }
    }
    v
}

/// `RoadWater*VFCInside*` style patterns: `*` matches any run of characters
fn wild_match(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let mut rest = name;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match rest.find(part) {
            Some(pos) if i > 0 || pos == 0 => rest = &rest[pos + part.len()..],
            _ => return false,
        }
    }
    parts.last().map(|l| l.is_empty() || rest.is_empty()).unwrap_or(true)
}

pub fn run(args: &[String], open_store: &mut dyn FnMut() -> crate::store::DataStore) -> R<()> {
    let src_path = args.get(1).ok_or("tileset needs SOURCE.Map.Gbx CONVERTED.Map.Gbx")?;
    let conv_path = args.get(2).ok_or("tileset needs SOURCE.Map.Gbx CONVERTED.Map.Gbx")?;
    let out_dir = flag(args, "--out-dir").ok_or("--out-dir DIR")?;
    let prefix = flag(args, "--prefix").unwrap_or("Dirt").to_string();
    let simple = flag(args, "--layout").unwrap_or("simple") != "flat";
    let author = flag(args, "--author").unwrap_or("Everios96").to_string();
    let collection: u32 = flag(args, "--collection").and_then(|v| v.parse().ok()).unwrap_or(26);
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{out_dir}: {e}"))?;

    let src = tmmaps::map::MapFile::load(Path::new(src_path));
    let conv = tmmaps::map::MapFile::load(Path::new(conv_path));
    let yoff = tmmaps::map::ground_y(collection);
    // `--drop-block-own PlatformWaterBase[,…]`: the converted items that stand for the
    // BLOCK itself (not its fillers) of these kinds are left out — a water base block is
    // nothing but its water sheet, and the library's stand-in for it is a flat deck quad
    // at the water line, which has no place in a "without the water" tile (the pool
    // floor is the drive surface). The alias → source mapping comes from the library
    // report beside the converted map (`report.tsv`).
    let drop_own: Vec<String> = flag(args, "--drop-block-own").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    // `--drop-pieces RoadWaterVFCInside[,…]`: filler kinds left out of every tile. The water
    // roads' junction piece re-draws 16 m of channel bed that the straight's own prefab
    // already carries (coplanar at y 1: a lighter patch and a z-fighting line in the dirt
    // bed, 2026-10-28 02:00 box shot); under water the game never shows the duplicate.
    let drop_pieces: Vec<String> = flag(args, "--drop-pieces").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let mut source_of: HashMap<String, String> = HashMap::new();
    if let Some(dir) = Path::new(conv_path).parent() {
        if let Ok(text) = std::fs::read_to_string(dir.join("report.tsv")) {
            for l in text.lines().skip(1) {
                let f: Vec<&str> = l.split('\t').collect();
                if f.len() >= 5 && f[0] == "block" && f[1] != "-" {
                    let name = f[4].split_whitespace().next().unwrap_or("").to_string();
                    source_of.insert(f[1].to_string(), name);
                }
            }
        }
    }

    // the converted map's item files, by stem
    let (_, zip) = conv.embedded_objects().ok_or("the converted map embeds no items")?;
    let files: HashMap<String, Vec<u8>> = tmmaps::header::zip_entries(&zip)
        .into_iter()
        .filter(|(n, _)| n.to_ascii_lowercase().ends_with(".item.gbx"))
        .map(|(n, b)| (n.rsplit('/').next().unwrap_or(&n).trim_end_matches(".Item.Gbx").trim_end_matches(".item.gbx").to_string(), b))
        .collect();
    let mut parsed: HashMap<String, crate::static_item::StaticItemFile> = HashMap::new();
    for (stem, bytes) in &files {
        match crate::static_item::parse_file(bytes) {
            Ok(f) => {
                parsed.insert(stem.clone(), f);
            }
            Err(e) => println!("  {stem}: not a static item ({e}) — skipped"),
        }
    }

    // every placement: world transform + centroid of its collision (or visual) vertices
    let mut placed: Vec<Placed> = Vec::new();
    for it in &conv.items {
        let stem = it.model.trim_end_matches(".Item.Gbx").to_string();
        let Some(f) = parsed.get(&stem) else {
            println!("  item {} ({stem}): no embedded file — skipped", it.index);
            continue;
        };
        let Some(so) = f.item.static_object() else { continue };
        if let Some(srcname) = source_of.get(&stem) {
            if drop_own.iter().any(|d| srcname == d) || drop_pieces.iter().any(|d| wild_match(d, srcname)) {
                continue;
            }
        }
        let world = crate::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, 1.0);
        let mut tmp = Merged::default();
        let mut resolve = |_: i32| -> Option<(String, String, u8)> { None };
        tmp.add_static_object(so, &world, 1.0, &mut resolve)?;
        let pts: Vec<[f32; 3]> = if !tmp.surf_vertices.is_empty() { tmp.surf_vertices.clone() } else { visual_positions(&tmp) };
        if pts.is_empty() {
            continue;
        }
        let n = pts.len() as f32;
        let c = pts.iter().fold([0.0f32; 3], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n, a[2] + p[2] / n]);
        if std::env::var("TILESET_DEBUG_PLACED").is_ok() {
            println!("  placed {stem} ({}) at ({:.1},{:.1},{:.1}) pos ({:.1},{:.1},{:.1}) yaw {:.2} pivot {:?}", source_of.get(&stem).cloned().unwrap_or_default(), c[0], c[1], c[2], it.pos[0], it.pos[1], it.pos[2], it.yaw, it.pivot);
        }
        placed.push(Placed { stem, world, centroid: c, yaw: it.yaw });
    }
    println!("{} converted placements with geometry", placed.len());

    // the authored blocks' footprints (block infos through the index)
    let mut store = open_store();
    let idx = crate::blockmap::BlockInfoIndex::build(&store, "Stadium");
    let mut infos: HashMap<String, crate::blockinfo::BlockInfo> = HashMap::new();
    let mut blocks: Vec<Block> = Vec::new();
    for rec in &src.blocks {
        if !infos.contains_key(&rec.name) {
            let path = idx.path_for(&rec.name).ok_or_else(|| format!("{}: no block info", rec.name))?;
            let bi = crate::blockinfo::load(&mut store, &path)?;
            infos.insert(rec.name.clone(), bi);
        }
        let bi = &infos[&rec.name];
        let ground = rec.flags & tmmaps::fillers::FLAG_GROUND != 0;
        let (variant, _) = bi.pick(ground, 0).or_else(|| bi.pick(!ground, 0)).ok_or_else(|| format!("{}: no variant", rec.name))?;
        let units = variant.cells();
        let (sx, sz) = units.iter().fold((1, 1), |(a, b), u| (a.max(u[0] + 1), b.max(u[2] + 1)));
        // the file keeps a one-cell margin in x and z (raw = editor + 1); item positions
        // and the game's world are in editor cells
        let cell = (rec.file_cell[0] as i32 - 1, rec.file_cell[1] as i32, rec.file_cell[2] as i32 - 1);
        let base = crate::place::grid_block(cell, rec.dir, (sx as f32 * 32.0, sz as f32 * 32.0), yoff);
        let cells: Vec<(i32, i32)> = units
            .iter()
            .map(|u| {
                let w = apply(&base, [u[0] as f32 * 32.0 + 16.0, 0.0, u[2] as f32 * 32.0 + 16.0]);
                ((w[0] / 32.0).floor() as i32, (w[2] / 32.0).floor() as i32)
            })
            .collect();
        if std::env::var("TILESET_DEBUG").is_ok() {
            println!("  block {} cell {:?} dir {} units {:?} → cells {:?} base t ({:.0},{:.0},{:.0})", rec.name, rec.file_cell, rec.dir, units, cells, base[9], base[10], base[11]);
        }
        blocks.push(Block { rec, base, size: (sx, sz), turn: 0, cells, open: [false; 4], diag: [false; 4], items: Vec::new() });
    }
    // connectivity: a same-FAMILY block (same name prefix family: PlatformWater / RoadWater)
    // in the cell across each LOCAL side of each footprint cell, and at the diagonals
    fn family(n: &str) -> String {
        if n.starts_with("PlatformWater") {
            "PlatformWater".to_string()
        } else if n.starts_with("RoadWater") {
            "RoadWater".to_string()
        } else {
            n.to_string()
        }
    }
    let occupied: HashMap<(i32, i32, i32), String> = blocks
        .iter()
        .flat_map(|b| b.cells.iter().map(move |c| ((c.0, b.rec.file_cell[1] as i32, c.1), b.rec.name.clone())))
        .collect();
    fn connectivity(b: &mut Block, occupied: &HashMap<(i32, i32, i32), String>) {
        let fam = family(&b.rec.name);
        let y = b.rec.file_cell[1] as i32;
        for side in 0..4 {
            // local side vector → world
            let lv = tmmaps::fillers::SIDE_VEC[side];
            let r = &b.base;
            let wv = [r[0] * lv.0 as f32 + r[6] * lv.1 as f32, 0.0, r[2] * lv.0 as f32 + r[8] * lv.1 as f32];
            let Some(ws) = side_of(wv) else { continue };
            let (dx, dz) = tmmaps::fillers::SIDE_VEC[ws];
            // open when ANY footprint cell's neighbour across that world side is a same-family
            // block that is not this block itself
            b.open[side] = b.cells.iter().any(|c| {
                let n = (c.0 + dx, y, c.1 + dz);
                !b.cells.contains(&(n.0, n.2)) && occupied.get(&n).map(|m| family(m) == fam).unwrap_or(false)
            });
        }
        // diagonals NE, SE, SW, NW in the local frame (single-cell blocks only)
        if b.cells.len() == 1 {
            let c = b.cells[0];
            let corners = [(1usize, 0usize), (1, 2), (3, 2), (3, 0)]; // (E,N), (E,S), (W,S), (W,N) as side pairs
            for (k, (sa, sb)) in corners.iter().enumerate() {
                let r = &b.base;
                let la = tmmaps::fillers::SIDE_VEC[*sa];
                let lb = tmmaps::fillers::SIDE_VEC[*sb];
                let lv = (la.0 + lb.0, la.1 + lb.1);
                let wv = [r[0] * lv.0 as f32 + r[6] * lv.1 as f32, 0.0, r[2] * lv.0 as f32 + r[8] * lv.1 as f32];
                let n = (c.0 + wv[0].round() as i32, y, c.1 + wv[2].round() as i32);
                b.diag[k] = occupied.get(&n).map(|m| family(m) == fam).unwrap_or(false);
            }
        }
    }
    for b in blocks.iter_mut() {
        connectivity(b, &occupied);
    }

    // assign every placement to the block whose footprint holds its centroid (x, z)
    let mut unassigned = 0usize;
    for (i, p) in placed.iter().enumerate() {
        let cx = (p.centroid[0] / 32.0).floor() as i32;
        let cz = (p.centroid[2] / 32.0).floor() as i32;
        let cy = ((p.centroid[1] - yoff) / 8.0).floor() as i32;
        let mut best: Option<(usize, i32)> = None;
        for (bi, b) in blocks.iter().enumerate() {
            if b.cells.contains(&(cx, cz)) {
                let dy = (b.rec.file_cell[1] as i32 - cy).abs();
                if best.map(|(_, d)| dy < d).unwrap_or(true) {
                    best = Some((bi, dy));
                }
            }
        }
        match best {
            Some((bi, dy)) if dy <= 2 => blocks[bi].items.push(i),
            _ => {
                unassigned += 1;
                println!("  unassigned: {} at ({:.1}, {:.1}, {:.1})", p.stem, p.centroid[0], p.centroid[1], p.centroid[2]);
            }
        }
    }
    println!("{} placements assigned, {unassigned} unassigned", placed.len() - unassigned);

    // signature per block: pieces in the local frame. A single-cell block is judged in
    // all four quarter-turned frames and keeps the one with the smallest signature, so
    // the four orientations of one shape share a tile (Everios96: "I can rotate items by
    // 90 degrees in the editor myself").
    let signature = |b: &Block, base: &Xform, dir: u8| -> String {
        let inv = invert(base);
        let mut parts: Vec<String> = b
            .items
            .iter()
            .map(|&i| {
                let p = &placed[i];
                let lc = apply(&inv, p.centroid);
                // the grid's quarter turn d is a yaw of −d·π/2 (grid_block / geom::yaw):
                // a +d·π/2 here put the dir-1 straights' end walls two quarters off and
                // split the one-ended straight into two tiles (2026-10-28)
                let byaw = -(dir as f32) * std::f32::consts::FRAC_PI_2;
                let ly = ((p.yaw - byaw) / std::f32::consts::FRAC_PI_2).round().rem_euclid(4.0) as i32;
                // a piece centred on the cell (the square pool floors) reads the same at
                // every quarter turn: its yaw is left out, or the four orientations of one
                // dead-end tile would never match
                let centred = b.size == (1, 1) && (lc[0] - 16.0).abs() < 1.0 && (lc[2] - 16.0).abs() < 1.0;
                if centred {
                    format!("{}@{:.0},{:.0},{:.0}", p.stem, lc[0] * 2.0, lc[1] * 2.0, lc[2] * 2.0)
                } else {
                    format!("{}@{:.0},{:.0},{:.0}r{}", p.stem, lc[0] * 2.0, lc[1] * 2.0, lc[2] * 2.0, ly)
                }
            })
            .collect();
        parts.sort();
        format!("{}|{}", b.rec.name, parts.join(";"))
    };
    let mut sigs: Vec<String> = Vec::new();
    for bi in 0..blocks.len() {
        let (cell, size) = {
            let b = &blocks[bi];
            ((b.rec.file_cell[0] as i32 - 1, b.rec.file_cell[1] as i32, b.rec.file_cell[2] as i32 - 1), b.size)
        };
        // Pool tiles only are judged in all four frames (a water road's own prefab has a
        // direction of its own; its tiles stay in the block's frame). Among the frames the
        // one whose open sides match the class's canonical form wins — End open to W,
        // Corner open to N+W, Channel N+S, Edge N+S+W — so every tile of a class faces
        // the same way and its name's diagonals mean the same thing; the signature
        // breaks the remaining ties (a channel's half turn, an island, a middle).
        // …and the two symmetric road pieces: a straight reads the same after a half
        // turn (one-ended straights would otherwise come out twice, OpenN and OpenS), a
        // cross after any quarter turn (16 end combinations → 6 tiles). Their own item
        // is centred, so its yaw is already left out of the signature.
        let is_pool = blocks[bi].rec.name.starts_with("PlatformWater");
        let mut best: Option<((u8, String), u8, Xform)> = None;
        let turns: Vec<u8> = match (size == (1, 1), is_pool, blocks[bi].rec.name.as_str()) {
            (true, true, _) | (true, _, "RoadWaterBranchCross") => vec![0, 1, 2, 3],
            (true, _, "RoadWaterStraight") => vec![0, 2],
            _ => vec![0],
        };
        for k in turns {
            let dir = (blocks[bi].rec.dir + k) % 4;
            let base = crate::place::grid_block(cell, dir, (size.0 as f32 * 32.0, size.1 as f32 * 32.0), yoff);
            let sig = signature(&blocks[bi], &base, dir);
            blocks[bi].base = base;
            connectivity(&mut blocks[bi], &occupied);
            let rank = if canonical_open(&blocks[bi].open) { 0u8 } else { 1u8 };
            let key = (rank, sig);
            if best.as_ref().map(|(b, _, _)| key < *b).unwrap_or(true) {
                best = Some((key, k, base));
            }
        }
        let ((_, sig), k, base) = best.unwrap();
        blocks[bi].turn = k;
        blocks[bi].base = base;
        connectivity(&mut blocks[bi], &occupied);
        if std::env::var("TILESET_DEBUG").is_ok() {
            println!("  sig {} {:?} turn {k}: {}", blocks[bi].rec.name, blocks[bi].rec.file_cell, sig);
        }
        sigs.push(sig);
    }
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (bi, sig) in sigs.iter().enumerate() {
        groups.entry(sig.clone()).or_default().push(bi);
    }

    // names: block + connectivity; a letter suffix separates same-connectivity tiles
    let side_names = ["N", "E", "S", "W"];
    let mut name_of: HashMap<String, String> = HashMap::new();
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut rows: Vec<String> = vec!["tile\tblock\tsides_open\tdiagonals_that_matter\tblocks\tpieces".to_string()];
    for (sig, members) in &groups {
        let b = &blocks[members[0]];
        let open: String = (0..4).filter(|&s| b.open[s]).map(|s| side_names[s]).collect();
        let n_open = b.open.iter().filter(|x| **x).count();
        let role = if b.rec.name == "PlatformWaterBase" {
            match (n_open, b.open) {
                (0, _) => "Island".to_string(),
                (1, _) => "End".to_string(),
                (2, [true, false, true, false]) | (2, [false, true, false, true]) => "Channel".to_string(),
                (2, _) => "Corner".to_string(),
                (3, _) => "Edge".to_string(),
                _ => "Middle".to_string(),
            }
        } else if open.is_empty() {
            "Closed".to_string()
        } else {
            format!("Open{open}")
        };
        let short = b.rec.name.replace("PlatformWaterBase", "Pool").replace("RoadWater", "Channel");
        // a pool tile's name carries the water diagonals that shape it (in its canonical
        // frame): `Dirt_Pool_End_NW_SW` = a dead end whose two rim pieces turn outward
        let diag_names = ["NE", "SE", "SW", "NW"];
        let relevant: Vec<usize> = if b.rec.name == "PlatformWaterBase" { relevant_diagonals(&b.open) } else { Vec::new() };
        let wet: Vec<&str> = relevant.iter().filter(|&&d| b.diag[d]).map(|&d| diag_names[d]).collect();
        // `--layout simple` (the default; Everios96 2026-10-28 "a simple naming scheme …
        // that they appear in a simple way in the folders"): the surface is the folder,
        // so the names drop it — `Pool/End_NW_SW`, `Pool/Island`, `Channel/Curve3_NE`,
        // `Channel/Cross` (closed = no suffix). `--layout flat` keeps the one-folder
        // `<prefix>_Pool_End_NW_SW` form.
        let base_name = if simple {
            if b.rec.name == "PlatformWaterBase" {
                if wet.is_empty() { format!("Pool/{role}") } else { format!("Pool/{role}_{}", wet.join("_")) }
            } else {
                let kind = b.rec.name.trim_start_matches("RoadWater").replace("BranchTShaped", "T").replace("BranchCross", "Cross");
                if open.is_empty() { format!("Channel/{kind}") } else { format!("Channel/{kind}_{open}") }
            }
        } else if wet.is_empty() {
            format!("{prefix}_{short}_{role}")
        } else {
            format!("{prefix}_{short}_{role}_{}", wet.join("_"))
        };
        let k = used.entry(base_name.clone()).or_insert(0);
        let name = if *k == 0 { base_name.clone() } else { format!("{base_name}{}", (b'A' + *k as u8) as char) };
        *k += 1;
        name_of.insert(sig.clone(), name.clone());
        let diag: String = relevant.iter().map(|&d| format!("{}{}", diag_names[d], if b.diag[d] { "=water" } else { "=dry" })).collect::<Vec<_>>().join(",");
        rows.push(format!("{name}\t{}\t{open}\t{diag}\t{}\t{}", b.rec.name, members.len(), b.items.len()));
    }

    // one item per group, in the local frame of its first member
    let mut reassembly: Vec<String> = Vec::new();
    for (sig, members) in &groups {
        let b = &blocks[members[0]];
        let name = &name_of[sig];
        let inv = invert(&b.base);
        let mut m = Merged::default();
        for &i in &b.items {
            let p = &placed[i];
            let f = &parsed[&p.stem];
            let so = f.item.static_object().ok_or("static object")?;
            let local = compose(&inv, &p.world);
            let mut resolve = |_: i32| -> Option<(String, String, u8)> { None };
            m.add_static_object(so, &local, 1.0, &mut resolve)?;
        }
        if m.visuals.is_empty() {
            println!("  {name}: nothing visible — skipped");
            continue;
        }
        // the ident is the FILE name; the folder is the browser's (and a map's) business
        let ident = format!("{}.Item.Gbx", name.rsplit('/').next().unwrap_or(name));
        let opts = BuildOpts { ident: ident.clone(), author: author.clone(), scale: 1.0, collection, skin: None };
        crate::static_item::assemble::GRID_SNAP.with(|g| g.set(Some((32.0, 8.0))));
        let mut f = assemble(&m, &opts)?;
        crate::static_item::assemble::GRID_SNAP.with(|g| g.set(None));
        f.body_comp = b'C';
        let bytes = crate::static_item::write_file(&f);
        let out = format!("{out_dir}/{name}.Item.Gbx");
        if let Some(parent) = Path::new(&out).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&out, &bytes).map_err(|e| format!("{out}: {e}"))?;
        let tris: usize = m.visuals.iter().filter_map(|v| v.visual.index_buffer.as_ref()).map(|ib| ib.indices.len() / 3).sum();
        println!("wrote {out}: {} bytes, {} pieces, {tris} tris, collision {} tris, {} block(s)", bytes.len(), b.items.len(), m.surf_triangles.len(), members.len());
        for &mi in members {
            let mb = &blocks[mi];
            let o = apply(&mb.base, [0.0, 0.0, 0.0]);
            // `tmmaps add-items` takes the yaw in RADIANS about the item's pivot (the cell
            // corner); a quarter turn d of the block grid is a yaw of −d·π/2 (geom::yaw maps
            // the x axis to (cos θ, 0, −sin θ); grid_block's dir 1 maps it to +z)
            let d = ((mb.rec.dir + mb.turn) % 4) as f32;
            let yaw = -d * std::f32::consts::FRAC_PI_2;
            reassembly.push(format!("{out}@{:.0},{:.0},{:.0}@{yaw:.6}@0", o[0], o[1], o[2]));
        }
    }
    std::fs::write(format!("{out_dir}/tiles.tsv"), rows.join("\n") + "\n").map_err(|e| e.to_string())?;
    std::fs::write(format!("{out_dir}/reassembly.txt"), reassembly.join(";") + "\n").map_err(|e| e.to_string())?;
    println!("{} tiles from {} blocks → {out_dir}/tiles.tsv, reassembly.txt", groups.len(), blocks.len());
    let _ = IDENTITY;
    Ok(())
}

fn visual_positions(m: &Merged) -> Vec<[f32; 3]> {
    use crate::static_item::vstream::{Elem, N_POSITION};
    use crate::static_item::Node;
    let mut out = Vec::new();
    for mv in &m.visuals {
        let Some(main) = mv.visual.main.as_ref() else { continue };
        let Some(Node::VertexStream(stream)) = main.vertex_streams.first().and_then(|r| r.inline.as_deref()) else { continue };
        for (d, e) in stream.decls.iter().zip(stream.elems.iter()) {
            if d.name() == N_POSITION {
                if let Elem::Float3(p) = e {
                    out.extend_from_slice(p);
                }
            }
        }
    }
    out
}

/// `mapgeom tileset-patterns [--origin X,Z] [--y Y]`: the `tmmaps add-blocks` spec of a
/// pattern map that shows the game EVERY local configuration a water tile can be in,
/// so one editor save yields the complete set of generated pieces (Everios96
/// 2026-10-28: "look for all prefabs you can find that are unique").
///
/// What decides a pool cell's pieces (read off the harvested signatures): its open
/// sides, and for every CLOSED side the two ends of its rim piece — `In` where the
/// adjacent side is closed too (the pool's own corner), `Str` where the adjacent side
/// is open and the diagonal cell behind it is empty (the wall runs straight on into
/// the neighbour), `Out` where that diagonal cell is filled (the water turns the
/// corner). So the diagonals next to an open side matter, the rest do not:
///   island (0 open)            1 tile
///   end (1 open, 2 diagonals)  4
///   corner (2 adjacent open)   4 (the diagonals at the far ends of the two closed sides)
///   channel (2 opposite open) 16 shapes (10 up to a half turn)
///   edge (3 open)              4
///   middle (4 open)            1
/// A water ROAD's pieces are its own prefab + its bed + its underside + an end wall per
/// closed end, so every end open/closed combination is laid out: a lone straight and a
/// run of three, each curve alone / with one straight / with both, the T's 8 and the
/// cross's 16 combinations.
pub fn patterns(args: &[String]) -> R<()> {
    let origin: (i32, i32) = flag(args, "--origin")
        .map(|v| {
            let p: Vec<i32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (p.first().copied().unwrap_or(1), p.get(1).copied().unwrap_or(1))
        })
        .unwrap_or((1, 1));
    let y: i32 = flag(args, "--y").and_then(|v| v.parse().ok()).unwrap_or(10);
    let mut specs: Vec<String> = Vec::new();
    let mut emit = |name: &str, x: i32, z: i32, dir: u8| specs.push(format!("{name}@{x},{y},{z}@{dir}"));
    // ---- pools: slots of pitch 4 in a row of 11, shapes within −1..1 around the slot centre
    let pool = "PlatformWaterBase";
    let mut shapes: Vec<Vec<(i32, i32)>> = Vec::new();
    shapes.push(vec![(0, 0)]); // island
    for d in 0..4u8 {
        // end: open +x; the diagonals (1,1) and (1,−1)
        let mut s = vec![(0, 0), (1, 0)];
        if d & 1 != 0 { s.push((1, 1)); }
        if d & 2 != 0 { s.push((1, -1)); }
        shapes.push(s);
    }
    for d in 0..4u8 {
        // corner: open +x and +z; the far diagonals (1,−1) and (−1,1)
        let mut s = vec![(0, 0), (1, 0), (0, 1)];
        if d & 1 != 0 { s.push((1, -1)); }
        if d & 2 != 0 { s.push((-1, 1)); }
        shapes.push(s);
    }
    for d in 0..16u8 {
        // channel: open ±z; the four diagonals
        let mut s = vec![(0, 0), (0, 1), (0, -1)];
        if d & 1 != 0 { s.push((1, 1)); }
        if d & 2 != 0 { s.push((1, -1)); }
        if d & 4 != 0 { s.push((-1, 1)); }
        if d & 8 != 0 { s.push((-1, -1)); }
        shapes.push(s);
    }
    for d in 0..4u8 {
        // edge: open ±z and +x, closed −x; the diagonals (−1,1) and (−1,−1)
        let mut s = vec![(0, 0), (0, 1), (0, -1), (1, 0)];
        if d & 1 != 0 { s.push((-1, 1)); }
        if d & 2 != 0 { s.push((-1, -1)); }
        shapes.push(s);
    }
    shapes.push((-1..=1).flat_map(|x| (-1..=1).map(move |z| (x, z))).collect()); // middle (a 3×3)
    let per_row = 11;
    let pitch = 4;
    let mut row_base_z = origin.1 + 1;
    for (i, s) in shapes.iter().enumerate() {
        let (cx, cz) = (origin.0 + 1 + (i as i32 % per_row) * pitch, row_base_z + (i as i32 / per_row) * pitch);
        for (dx, dz) in s {
            emit(pool, cx + dx, cz + dz, 0);
        }
    }
    row_base_z += ((shapes.len() as i32 + per_row - 1) / per_row) * pitch + 1;
    // ---- roads, 1×1 kinds: a straight runs along z (dir 0: ends at ±z; dir 1: along x)
    let mut road_shapes: Vec<Vec<(&str, i32, i32, u8)>> = Vec::new();
    road_shapes.push(vec![("RoadWaterStraight", 0, 0, 0)]);
    road_shapes.push(vec![("RoadWaterStraight", 0, -1, 0), ("RoadWaterStraight", 0, 0, 0), ("RoadWaterStraight", 0, 1, 0)]);
    // T (dir 0): ends N (+z), E (−x), S (−z); its back is W
    for d in 0..8u8 {
        let mut s = vec![("RoadWaterBranchTShaped", 0, 0, 0)];
        if d & 1 != 0 { s.push(("RoadWaterStraight", 0, 1, 0)); }
        if d & 2 != 0 { s.push(("RoadWaterStraight", -1, 0, 1)); }
        if d & 4 != 0 { s.push(("RoadWaterStraight", 0, -1, 0)); }
        road_shapes.push(s);
    }
    for d in 0..16u8 {
        let mut s = vec![("RoadWaterBranchCross", 0, 0, 0)];
        if d & 1 != 0 { s.push(("RoadWaterStraight", 0, 1, 0)); }
        if d & 2 != 0 { s.push(("RoadWaterStraight", -1, 0, 1)); }
        if d & 4 != 0 { s.push(("RoadWaterStraight", 0, -1, 0)); }
        if d & 8 != 0 { s.push(("RoadWaterStraight", 1, 0, 1)); }
        road_shapes.push(s);
    }
    for (i, s) in road_shapes.iter().enumerate() {
        let (cx, cz) = (origin.0 + 1 + (i as i32 % per_row) * pitch, row_base_z + (i as i32 / per_row) * pitch);
        for (name, dx, dz, dir) in s {
            emit(name, cx + dx, cz + dz, *dir);
        }
    }
    row_base_z += ((road_shapes.len() as i32 + per_row - 1) / per_row) * pitch + 1;
    // ---- curves: Curve n is n×n at dir 0 with its ends E (a straight at (−1,0) dir 1)
    // and N (a straight at (n−1, n) dir 0); alone / E / N / both
    let mut x = origin.0 + 1;
    let mut z = row_base_z;
    let mut row_h = 0;
    for n in 1..=5i32 {
        let name = format!("RoadWaterCurve{n}");
        let w = n + 2; // −1..n
        for d in 0..4u8 {
            if x + w > 47 {
                x = origin.0 + 1;
                z += row_h + 1;
                row_h = 0;
            }
            // the curve's cell (0,0) sits at (x+1, z)
            let (bx, bz) = (x + 1, z);
            emit(&name, bx, bz, 0);
            if d & 1 != 0 { emit("RoadWaterStraight", bx - 1, bz, 1); }
            if d & 2 != 0 { emit("RoadWaterStraight", bx + n - 1, bz + n, 0); }
            x += w + 1;
            row_h = row_h.max(n + 1);
        }
    }
    let _ = (x, z, row_h);
    println!("{}", specs.join(","));
    eprintln!("{} blocks; pools {} shapes, roads {} shapes + 20 curve shapes", specs.len(), shapes.len(), road_shapes.len());
    Ok(())
}
