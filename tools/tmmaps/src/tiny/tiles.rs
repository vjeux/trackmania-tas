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
        self.occupied.contains_key(&tile.file_cell)
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
    let mut out = HiddenTiles { occupied: BTreeMap::new(), declared: BTreeSet::new(), file_cells: BTreeSet::new(), blocks: 0, declaring: 0 };
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
        let fp = footprint_of(&units);
        for u in &units {
            if let Some(c) = turned_cell(b, fp, *u) {
                out.occupied.entry(c).or_insert(b.index);
            }
        }
    }
    out
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
}

