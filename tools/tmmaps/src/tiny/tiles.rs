//! Which terrain tiles the tiny map leaves out: a tile is never drawn in a
//! cell one of a block's units occupies, authored or generated
//! (`hidden_tiles`, the game's own rule read off the block infos and fillers;
//! `stands_in_for_tile` is the older name rule that stands in when a mapping
//! carries no auto terrain), and `tmmaps shared-cells`, the census of cells a
//! block and a tile share.

use super::mapping::read_mapping;
use crate::map::{BlockRec, MapFile};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// World y the tiny transform keeps fixed by default, per collection: the
/// BlueBay sea surface (7: the land plane, block top y 10, halves to 8.5
/// above it — the verified Summer 01 mapping, spawn 16 -> 11.5); the Stadium
/// grass (10: the full-size floor the genealogy regenerates stays the floor).
/// A block that stands in for the terrain tile of its cell: the game draws
/// only the replacement. Two families: a block named after the zone it
/// replaces (`PlatformGrassOnLandHillSlopeBase`, `RoadTechStraightOnBeach` —
/// `On<zone>` with or without the zone's trailing digit, OnLandHill covers
/// LandHill1/2) and a GROUND-variant block (flags bit 12) whose family covers
/// its whole cell with a deck: platforms, roads, stands, deco platforms
/// (Summer 04's PlatformDirtCheckpoint shared its cell with a Grass tile whose
/// grass-blade shader poked tufts up through the dirt deck). Pillars,
/// DecoTerrainHD detail meshes and wall pillars do not cover the cell and keep
/// their tile.
pub fn stands_in_for_tile(name: &str, flags: u32, zones: &BTreeSet<String>) -> bool {
    if zones.contains(name) {
        return false;
    }
    let replaces = |zone: &str| {
        let stem = zone.trim_end_matches(|c: char| c.is_ascii_digit());
        name.contains(&format!("On{zone}")) || (!stem.is_empty() && name.contains(&format!("On{stem}")))
    };
    let covers_cell = flags & (1 << 12) != 0 && ["Platform", "Road", "OpenTechRoad", "DecoPlatform", "Stand"].iter().any(|p| name.starts_with(p));
    zones.iter().any(|z| replaces(z)) || covers_cell
}

/// The cells whose terrain tile is hidden by a stand-in block (see
/// `stands_in_for_tile`), keyed by the raw file cell: every unit cell of the
/// block's variant (`units`, in the block's frame, turned by its direction
/// the way `block_origin` turns footprints — dir 1 = yaw −π/2 maps local +x
/// onto world +z and local +z onto world −x), or the origin cell alone when
/// the mapping carries no units. The game replaces the terrain under EVERY
/// unit of a ground deck; hiding only the origin cell's tile left a Curve5's
/// twelve other Grass tiles at deck height — the wheels read Grass on the
/// road and the car was capped at grass speed (Summer 01, 2026-09-07).
pub fn replaced_cells(source: &MapFile, zones: &BTreeSet<String>, units_of: &dyn Fn(&BlockRec) -> Vec<[i32; 3]>) -> BTreeSet<[u8; 3]> {
    let mut out = BTreeSet::new();
    for b in source.blocks.iter().filter(|b| stands_in_for_tile(&b.name, b.flags, zones)) {
        out.insert(b.file_cell);
        if b.free_pos.is_some() {
            continue; // a free block has no cell footprint to turn
        }
        let units = units_of(b);
        if units.len() <= 1 {
            continue;
        }
        let fp = footprint_of(&units);
        for u in &units {
            if let Some(c) = turned_cell(b, fp, *u) {
                out.insert(c);
            }
        }
    }
    out
}

/// The footprint (sx, sz) a variant's unit cells span — the pivot the game
/// turns the block about.
fn footprint_of(units: &[[i32; 3]]) -> (i32, i32) {
    units.iter().fold((1i32, 1i32), |(sx, sz), u| (sx.max(u[0] + 1), sz.max(u[2] + 1)))
}

/// The raw file cell an offset in a block's own frame lands on, turned by the
/// block's direction about its footprint (the way `block_origin` turns it);
/// `None` off the 256³ grid.
fn turned_cell(b: &BlockRec, (sx, sz): (i32, i32), u: [i32; 3]) -> Option<[u8; 3]> {
    let [cx, cy, cz] = b.file_cell.map(|c| c as i32);
    let (x, z) = match b.dir & 3 {
        0 => (cx + u[0], cz + u[2]),
        1 => (cx + sz - 1 - u[2], cz + u[0]),
        2 => (cx + sx - 1 - u[0], cz + sz - 1 - u[2]),
        _ => (cx + u[2], cz + sx - 1 - u[0]),
    };
    let y = cy + u[1];
    ((0..=255).contains(&x) && (0..=255).contains(&y) && (0..=255).contains(&z)).then(|| [x as u8, y as u8, z as u8])
}

/// A zone or tile name without its trailing digit(s): `LandHill2` -> `LandHill`
/// (a block info's auto terrain names the zone, the file's tile record the
/// zone's block — `PlatformGrassOnLandHillSlopeBase` brings `LandHill`, the
/// cell holds `LandHill1`); `Land` stays `Land`, a transition
/// `Grass0_Grass1_Grass2` stays distinct from the flat `Grass`.
fn zone_stem(name: &str) -> &str {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// Which terrain tiles — authored or GENERATED — the tiny map leaves out
/// because a block stands in their cell.
///
/// The game's rule, read off Summer 04's file and block infos (2026-09-08):
/// a terrain tile is never drawn in a cell one of a block's UNITS occupies.
/// The editor still records the tile — every ground variant declares an
/// AUTO TERRAIN (`[0,0,0] Grass` on a RoadTechStart, a PlatformTechBase, a
/// StructurePillar on the flat; a Curve5 lists its 14 units) and places it,
/// so Summer 04's 633 flat Grass tiles are BAKED and 38 of them sit under
/// the start road, the finish plaza's platforms and its gates — but what the
/// game draws there is the block: its variant's prefab, made for the terrain
/// it stands on (`StructurePillarOnLake`, `…OnLakeShoreDeadend`,
/// `PlatformTechBaseOnGrassCliff2`, a DecoLakeCurve that IS the shore over
/// its 2×2 footprint), plus its `…FCBGround` fillers, full-footprint planes
/// at exactly the tile's height (`TrackToDeco…FCBGround` the apron round a
/// road, `StructurePillarFCBGround` the plate under a pillar,
/// `PlatformToDecoDiag1FCBGround` the half beside a diagonal deck). Drawn
/// together with the tile they would z-fight in Nadeo's own maps.
///
/// The tiny map drew both: a half-size Grass tile and a half-size deck at
/// the same height — vjeux, playing 04: "the road and grass are mixed
/// together and flash like crazy". Before this the tile was hidden only
/// under a short list of deck families (`stands_in_for_tile`), and only when
/// authored.
///
/// A block that maps to no geometry (`-`: an empty pillar variant, a wall
/// pillar whose faces are generated fillers) hides nothing — with no block
/// drawn there, the tile is the only ground the cell has. A free block sits
/// in no cell. The variant's declared auto terrain (the mapping's 7th
/// column) is kept as a cross-check: a tile hidden under a block that did
/// not declare its zone is counted (`undeclared`) — the place to look if a
/// hole ever shows.
pub struct HiddenTiles {
    /// the cells a block with geometry occupies, with the block's index
    occupied: BTreeMap<[u8; 3], usize>,
    /// the cells only GHOST-mode blocks (record flag bit 28) occupy — they hide an
    /// authored tile like any unit (`TINY_GHOST_TILES=keep` turns that off) but
    /// never a Sea record: the game draws the sea pond under Summer 11's ghost
    /// StructureBase foot at (38, 5, 26)
    ghost_only: BTreeSet<[u8; 3]>,
    ghost_hides_tiles: bool,
    /// (cell, zone stem) some block declares as its own auto terrain
    declared: BTreeSet<([u8; 3], String)>,
    /// the file cells of blocks with geometry (a rotated footprint's min corner may
    /// not be a unit — see `kept_at_file_cell`)
    file_cells: BTreeSet<[u8; 3]>,
    /// blocks whose footprint hides tiles / blocks that declare auto terrain
    pub blocks: usize,
    pub declaring: usize,
}

impl HiddenTiles {
    /// The game's rule, CORRECTED 2026-09-11 (Summer 01, vjeux's "hole in the mountain"):
    /// a tile is hidden only where the occupying block DECLARES its auto terrain for
    /// that cell. A unit cell a block occupies without declaring (the empty corner of
    /// a RoadTechCurve4's 4×4, the Land under a DecoTreeBeach) keeps its tile — the
    /// original draws the LandHill3 at (30, 6, 32) under the curve's corner (palms
    /// on it); hiding it left a cell-sized cutout with the sea showing through.
    pub fn hides(&self, tile: &BlockRec) -> bool {
        // RULE (corrected twice on 2026-09-11): hidden iff a block UNIT covers the cell.
        // Not "declared auto terrain only" — Summer 12's Dirt tile at the TOP cell of a
        // RoadDirtSlope2BaseCurve2 (unit (0,2,1), undeclared) must stay hidden: drawn, its
        // 8.75 top buried the slope's road at 7.98 and stopped the 18e lap at 4.74 s.
        if self.occupied.contains_key(&tile.file_cell) {
            return true;
        }
        // A cell covered only by ghost-mode units: the original draws the SEA there
        // (2026-09-12, the pond); whether it draws an authored tile too is unverified,
        // and restoring those tiles DNF'd the certified laps of 18 and 21 (the tile met
        // the deck the car drives on), so they stay hidden unless asked.
        self.ghost_only.contains(&tile.file_cell) && tile.name != "Sea" && self.ghost_hides_tiles
    }
    /// A tile kept in a cell that is some block's FILE cell without being one of its
    /// units (the min corner of a rotated footprint the block does not cover): the
    /// class the 2026-09-08 rule hid wrongly (Summer 01's hole). Listed for the log.
    pub fn kept_at_file_cell(&self, tile: &BlockRec) -> bool {
        !self.hides(tile) && self.file_cells.contains(&tile.file_cell)
    }
    /// Index of the block occupying the tile's cell.
    pub fn occupant(&self, tile: &BlockRec) -> Option<usize> {
        self.occupied.get(&tile.file_cell).copied()
    }
}

/// `info_of` gives a block's mapping data: its model (`-` = no geometry), its
/// variant's unit cells and, when the mapping carries the column, its auto
/// terrain (offsets, zone) + place type.
pub fn hidden_tiles(source: &MapFile, zones: &BTreeSet<String>, info_of: &dyn Fn(&BlockRec) -> Option<(String, Vec<[i32; 3]>, Option<(Vec<([i32; 3], String)>, i32)>)>) -> HiddenTiles {
    // `TINY_GHOST_TILES=keep`: a ghost-mode block's units hide no tile at all (the
    // editor's ghost mode leaves the terrain alone). Not the default: it restores
    // ~100 tiles campaign-wide and two of them meet the deck the certified laps of
    // 18 and 21 drive on (the laps DNF in the oracle) — to be verified against the
    // original frame by frame before it can be the rule. See `HiddenTiles::hides`.
    let ghost_hides_tiles = std::env::var("TINY_GHOST_TILES").map(|v| v != "keep").unwrap_or(true);
    let mut out = HiddenTiles { occupied: BTreeMap::new(), ghost_only: BTreeSet::new(), ghost_hides_tiles, declared: BTreeSet::new(), file_cells: BTreeSet::new(), blocks: 0, declaring: 0 };
    for b in source.blocks.iter().filter(|b| !zones.contains(&b.name) && b.free_pos.is_none()) {
        let Some((model, units, auto)) = info_of(b) else { continue };
        if let Some((list, _place)) = &auto {
            out.declaring += 1;
            let fp = footprint_of(&units);
            for (off, zone) in list {
                if let Some(c) = turned_cell(b, fp, *off) {
                    out.declared.insert((c, zone_stem(zone).to_string()));
                }
            }
        }
        if model == "-" {
            continue;
        }
        out.blocks += 1;
        out.file_cells.insert(b.file_cell);
        // The block's UNITS occupy cells — NOT the raw file cell: for a rotated multi-cell
        // block the file cell is the footprint's min corner, which for a RoadTechCurve4
        // turned 90° is one of the curve's EMPTY corners (local (0,3)/(3,0), not a unit):
        // inserting it hid the LandHill3 tile of Summer 01 at (30, 6, 32) — vjeux's "hole
        // in the mountain" (2026-09-11). The game draws the tile in a cell no unit covers.
        // A GHOST-mode block's units (record flag bit 28) are kept apart: the game draws
        // the sea pond under Summer 11's ghost StructureBase foot at (38, 5, 26).
        let ghost = b.flags & crate::fillers::FLAG_GHOST != 0;
        let fp = footprint_of(&units);
        for u in &units {
            if let Some(c) = turned_cell(b, fp, *u) {
                if ghost {
                    out.ghost_only.insert(c);
                } else {
                    out.occupied.entry(c).or_insert(b.index);
                }
            }
        }
    }
    out
}

/// The map's PONDS: the cells of `Sea` records (baked or authored) that are not
/// part of the open sea — not connected, cell edge to cell edge through other Sea
/// cells of the same row, to the outer ring of the sea's own extent. The game
/// draws every Sea cell from the `Zone\Sea\Base.Prefab`: a Water quad at +7 over a
/// Sand `SeaFloor` at +4, so the original's water is 3 m deep everywhere. The tiny
/// map keeps the open sea as the collection's regenerated decoration (BlueBay's
/// genealogy is cleared: plain sea under the island) and never placed a floor —
/// fine on the open water, but a one-cell pond enclosed by Beach tiles became
/// bottomless: its water showed the sky/fog where the original shows 1.5 m of
/// water over sand (Summer 11 (38, 5, 26), under the ghost pillar foot). These are
/// the cells `tmmaps tiny` gives a half-size Sea item (the floor; the Water visual
/// is dropped as on every shore tile).
pub fn pond_cells(source: &MapFile) -> BTreeSet<[u8; 3]> {
    let sea: BTreeSet<[u8; 3]> = source.blocks.iter().chain(source.baked.iter()).filter(|b| b.name == "Sea" && b.free_pos.is_none()).map(|b| b.file_cell).collect();
    if sea.is_empty() {
        return BTreeSet::new();
    }
    let (mut lo, mut hi) = ([u8::MAX; 3], [u8::MIN; 3]);
    for c in &sea {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k]);
            hi[k] = hi[k].max(c[k]);
        }
    }
    let mut open: BTreeSet<[u8; 3]> = BTreeSet::new();
    let mut queue: Vec<[u8; 3]> = sea.iter().filter(|c| c[0] == lo[0] || c[0] == hi[0] || c[2] == lo[2] || c[2] == hi[2]).copied().collect();
    open.extend(queue.iter().copied());
    while let Some([x, y, z]) = queue.pop() {
        for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let (nx, nz) = (x as i32 + dx, z as i32 + dz);
            if !(0..=255).contains(&nx) || !(0..=255).contains(&nz) {
                continue;
            }
            let n = [nx as u8, y, nz as u8];
            if sea.contains(&n) && open.insert(n) {
                queue.push(n);
            }
        }
    }
    sea.difference(&open).copied().collect()
}

/// `tmmaps ponds MAP [--trace LAP.csv --anchor sx,sy,sz:tx,ty,tz [--scale 0.5]]`: the
/// enclosed sea cells (`pond_cells`) with what else stands in each — the census
/// behind the pond floors of 2026-09-12. With `--trace` (a `tmtraj export --csv`
/// of a tiny-map lap) every pond cell the car passes over is listed with the
/// sample time and the car's height over the new floor, for the lap project: a
/// lap that used to fall through a bottomless pond now lands on sand.
pub fn ponds_cmd(args: &[String]) {
    let source = MapFile::load(Path::new(&args[2]));
    let ponds = pond_cells(&source);
    let sea_total = source.blocks.iter().chain(source.baked.iter()).filter(|b| b.name == "Sea").count();
    println!("cell\tother blocks in the cell (flags)");
    for c in &ponds {
        let others: Vec<String> = source
            .blocks
            .iter()
            .chain(source.baked.iter())
            .filter(|b| b.file_cell == *c && b.name != "Sea" && b.free_pos.is_none())
            .map(|b| format!("{} ({:08X}{})", b.name, b.flags, if b.flags & crate::fillers::FLAG_GHOST != 0 { " ghost" } else { "" }))
            .collect();
        println!("{},{},{}\t{}", c[0], c[1], c[2], if others.is_empty() { "-".to_string() } else { others.join(", ") });
    }
    eprintln!("{} pond cells of {} Sea cells", ponds.len(), sea_total);
    print_trace(args, &ponds, "pond");
}

/// The lap samples of a `tmtraj export --csv` trace (a TINY-map lap) over the
/// given file cells: cell -> (first sample ms, lowest car y, highest car y,
/// samples). `anchor` is `sx,sy,sz:tx,ty,tz` (ANCHORS.tsv), the tiny transform
/// the map was built with; a sample is mapped back to the source world and its
/// game cell (file cell = game cell + (1, 0, 1)).
pub fn trace_over_cells(trace: &str, anchor: &str, scale: f32, cells: &BTreeSet<[u8; 3]>) -> (usize, BTreeMap<[u8; 3], (u32, f32, f32, usize)>) {
    let v3 = |s: &str| -> [f32; 3] {
        let f: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap_or_else(|_| panic!("bad number in {s}"))).collect();
        assert_eq!(f.len(), 3, "{s}: three numbers");
        [f[0], f[1], f[2]]
    };
    let (src, dst) = anchor.split_once(':').unwrap_or_else(|| panic!("--anchor sx,sy,sz:tx,ty,tz"));
    let (src, dst) = (v3(src), v3(dst));
    let text = std::fs::read_to_string(trace).unwrap_or_else(|e| panic!("{trace}: {e}"));
    let mut hits: BTreeMap<[u8; 3], (u32, f32, f32, usize)> = BTreeMap::new();
    let mut samples = 0usize;
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 4 {
            continue;
        }
        let (Ok(t), Ok(x), Ok(y), Ok(z)) = (f[0].parse::<u32>(), f[1].parse::<f32>(), f[2].parse::<f32>(), f[3].parse::<f32>()) else { continue };
        samples += 1;
        let sx = src[0] + (x - dst[0]) / scale;
        let sz = src[2] + (z - dst[2]) / scale;
        let (cx, cz) = ((sx / 32.0).floor() as i32 + 1, (sz / 32.0).floor() as i32 + 1);
        for c in cells {
            if c[0] as i32 == cx && c[2] as i32 == cz {
                let e = hits.entry(*c).or_insert((t, y, y, 0));
                e.1 = e.1.min(y);
                e.2 = e.2.max(y);
                e.3 += 1;
            }
        }
    }
    (samples, hits)
}

/// `--trace LAP.csv --anchor …` on a census command: print the cells of `cells`
/// the lap crosses, with the car's height range there.
fn print_trace(args: &[String], cells: &BTreeSet<[u8; 3]>, what: &str) {
    let Some(trace) = crate::cli::flag(args, "--trace") else { return };
    let anchor = crate::cli::flag(args, "--anchor").unwrap_or_else(|| panic!("--trace needs --anchor sx,sy,sz:tx,ty,tz (ANCHORS.tsv)"));
    let scale: f32 = crate::cli::flag(args, "--scale").and_then(|s| s.parse().ok()).unwrap_or(0.5);
    let (samples, hits) = trace_over_cells(trace, anchor, scale, cells);
    if hits.is_empty() {
        eprintln!("trace {trace}: {samples} samples, none over a {what} cell");
    } else {
        println!("\n{what} cells the lap crosses (cell, first sample ms, lowest car y, highest car y, samples):");
        for (c, (t, lo, hi, n)) in &hits {
            println!("{},{},{}\t{}\t{:.2}\t{:.2}\t{}", c[0], c[1], c[2], t, lo, hi, n);
        }
    }
}

/// `tmmaps shared-cells MAP [--all]`: every cell where a terrain tile shares
/// its cell with another authored block, and whether the tile survives the
/// tiny transform. The tiles that survive beside a block are the coplanar
/// pairs the terrain drop was invented for (2026-09-06: Land plane and road
/// deck both at +2 z-fought as two items); once every deck hides its tile the
/// remaining pairs decide whether the drop is still needed at all. Without
/// `--all` only the surviving pairs are listed.
pub fn shared_cells_cmd(args: &[String]) {
    let source = MapFile::load(Path::new(&args[2]));
    let all = args.iter().any(|a| a == "--all");
    let zones: BTreeSet<String> = source.genealogy_zones().into_iter().collect();
    // `--mapping placements.tsv`: the model, units and auto terrain per block —
    // every block with geometry hides the tiles of the cells its units occupy.
    // Without a mapping every non-tile block counts as one cell of geometry.
    let mapping = crate::cli::flag(args, "--mapping").map(|p| read_mapping(Path::new(p)));
    let info_of = |b: &BlockRec| -> Option<(String, Vec<[i32; 3]>, Option<(Vec<([i32; 3], String)>, i32)>)> {
        match mapping.as_ref() {
            Some(m) => m.by_index.get(&b.index).or_else(|| m.by_name.get(&b.name)).map(|m| (m.model.clone(), m.units.clone(), m.auto_terrain.clone())),
            None => Some((b.name.clone(), Vec::new(), None)),
        }
    };
    let hidden = hidden_tiles(&source, &zones, &info_of);
    eprintln!("{} blocks with geometry occupy tile cells; {} declare their auto terrain", hidden.blocks, hidden.declaring);
    let mut by_cell: BTreeMap<[u8; 3], (Vec<&BlockRec>, Vec<&BlockRec>)> = BTreeMap::new();
    for b in &source.blocks {
        let e = by_cell.entry(b.file_cell).or_default();
        if zones.contains(&b.name) {
            e.0.push(b);
        } else {
            e.1.push(b);
        }
    }
    // The GENERATED tiles count too: GreenCoast's flat base Grass is baked, not
    // authored (Summer 04: 633 baked Grass, 38 of them under the start road, the
    // finish plaza's platforms and gates — emitted as items they were coplanar
    // with the decks, the Z-fight of 2026-09-08). Listed with a `b` prefix.
    for b in source.baked.iter().filter(|b| zones.contains(&b.name)) {
        by_cell.entry(b.file_cell).or_default().0.push(b);
    }
    let mut pairs: BTreeMap<(String, String, &str), usize> = BTreeMap::new();
    let mut listed = 0usize;
    // the KEPT tiles' cells, for `--trace` (a lap over a kept tile is the class the 18d/18e rule broke: a lid on a driven road)
    let mut kept_cells: BTreeSet<[u8; 3]> = BTreeSet::new();
    // a generated tile prints as `b:<zone>` (the same tile, but from the baked chunk)
    let is_baked = |t: &BlockRec| source.baked.iter().any(|x| std::ptr::eq(x, t));
    let tile_label = |t: &BlockRec| if is_baked(t) { format!("b:{}", t.name) } else { t.name.clone() };
    println!("cell\ttile\tstatus\tother blocks (flags)");
    for (_cell, (tiles, others)) in &by_cell {
        if tiles.is_empty() || others.is_empty() {
            continue;
        }
        // hidden = a block with geometry occupies the cell; `hidden?` = the same,
        // but no block declared this zone as its auto terrain (look there first
        // if a hole ever shows)
        let status = if tiles.iter().all(|t| hidden.hides(t)) {
            if tiles.iter().any(|t| hidden.kept_at_file_cell(t)) { "kept (file cell, no unit)" } else { "hidden" }
        } else {
            "kept"
        };
        if !all && status == "hidden" {
            for t in tiles {
                for o in others {
                    *pairs.entry((tile_label(t), o.name.clone(), status)).or_default() += 1;
                }
            }
            continue;
        }
        listed += 1;
        if status.starts_with("kept") {
            kept_cells.insert(tiles[0].file_cell);
        }
        let (x, y, z) = tiles[0].coords();
        let tile_names: Vec<String> = tiles.iter().map(|t| tile_label(t)).collect();
        let other_names: Vec<String> = others.iter().map(|o| format!("{} ({:08X}{})", o.name, o.flags, if o.flags & (1 << 12) != 0 { " ground" } else { "" })).collect();
        println!("{x},{y},{z}\t{}\t{status}\t{}", tile_names.join("+"), other_names.join(", "));
        for t in tiles {
            for o in others {
                *pairs.entry((tile_label(t), o.name.clone(), status)).or_default() += 1;
            }
        }
    }
    eprintln!("{listed} cells listed; tile/block pairs:");
    for ((t, o, status), n) in &pairs {
        eprintln!("  {n:4} × {t} + {o}  [{status}]");
    }
    print_trace(args, &kept_cells, "kept tile");
}

