//! `mapgeom checkpoint-catalogue SOURCE.Map.Gbx --out MAP --mapping MAPPING.tsv` — Hugo's test map (2026-10-24
//! 06:15): a Stadium map holding every checkpoint block once at North on a 4-cell grid, the whole grid again
//! turned East, the multi-cell ones a third time at South, a start and a finish; built from a Fall Stadium source
//! with every authored block and item stripped. `mapgeom checkpoint-clean MAP --out MAP --mapping MAPPING.tsv` —
//! the Rust twin of his CheckpointsCleaned: every checkpoint block → its plain block, Direction + offset, the
//! unmappable ones removed; the expected result he can diff against.
use std::collections::BTreeMap;

use tmmaps::map::{FreeBlockSpec, MapFile};

/// One MAPPING.tsv row (the summary written by `checkpoint-map`): block → (replacement, offset, verdict, size).
#[derive(Clone, Debug)]
pub struct Rule {
    pub block: String,
    pub replacement: String,
    pub offset: u8,
    pub verdict: String,
    pub size: (i32, i32),
}

pub fn offset_of(name: &str) -> u8 {
    match name {
        "Quarter" => 1,
        "Half" => 2,
        "ThreeQuarters" => 3,
        _ => 0,
    }
}

/// Read the SUMMARY-format MAPPING.tsv (block, replacement, offset, verdict, verified_by, mismatch, collections,
/// size, hugo); dead rows (no such block) are skipped.
pub fn read_mapping(path: &str) -> Result<Vec<Rule>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 8 || f[3] == "dead" {
            continue;
        }
        let size = f[7].split_once('x').map(|(a, b)| (a.parse().unwrap_or(1), b.parse().unwrap_or(1))).unwrap_or((1, 1));
        out.push(Rule { block: f[0].to_string(), replacement: f[1].to_string(), offset: offset_of(f[2]), verdict: f[3].to_string(), size });
    }
    if out.is_empty() {
        return Err(format!("{path}: no rules"));
    }
    Ok(out)
}

/// The flags of a fresh grid placement of `name`: the ground base variant when the block has one (bit 12), else
/// air; variant 0.
pub fn placement_flags(store: &mut crate::store::DataStore, name: &str) -> Result<(u32, bool), String> {
    let Some(bp) = crate::vista::find_block_info(store, name) else { return Err(format!("{name}: no block info")) };
    let bi = crate::blockinfo::load(store, &bp)?;
    Ok(if bi.variant_base_ground.is_some() { (crate::blockmap::FLAG_GROUND, true) } else { (0, false) })
}

/// A slot of the catalogue: where a block goes.
pub struct Slot {
    pub name: String,
    pub cell: [i32; 3],
    pub dir: u8,
    pub flags: u32,
    pub ground: bool,
    pub group: &'static str,
}

/// Lay the catalogue out in rows of 2-cell slots across the map width (a 1×1 block per slot, a cell of grass
/// between neighbours), the ten 3×2 multi-cell blocks in 4-cell slots on their own rows; the North copy (start +
/// 98), a blank row, the East copy (a straight marker + 98), a blank row, the multi-cell ones at South, the finish.
/// Returns the slots and the rows of cells used.
pub fn layout(store: &mut crate::store::DataStore, rules: &[Rule], grass_row: i32, width_cells: i32) -> Result<(Vec<Slot>, i32), String> {
    let mut slots = Vec::new();
    let mut x = 0i32;
    let mut z = 0i32;
    let mut row_h = 2i32;
    let mut place = |slots: &mut Vec<Slot>, name: &str, dir: u8, flags: u32, ground: bool, group: &'static str, size: (i32, i32), x: &mut i32, z: &mut i32, row_h: &mut i32| {
        // the turned footprint
        let (sx, sz) = if dir & 1 == 1 { (size.1, size.0) } else { size };
        let pitch_x = sx + 1;
        if *x + sx > width_cells {
            *x = 0;
            *z += *row_h;
            *row_h = 2;
        }
        *row_h = (*row_h).max(sz + 1);
        slots.push(Slot { name: name.to_string(), cell: [*x, if ground { grass_row } else { grass_row + 1 }, *z], dir, flags, ground, group });
        *x += pitch_x;
    };
    let newline = |x: &mut i32, z: &mut i32, row_h: &mut i32, gap: i32| {
        if *x > 0 {
            *z += *row_h;
        }
        *z += gap;
        *x = 0;
        *row_h = 2;
    };
    let singles: Vec<&Rule> = rules.iter().filter(|r| r.size == (1, 1)).collect();
    let multis: Vec<&Rule> = rules.iter().filter(|r| r.size != (1, 1)).collect();
    let (sf, sg) = placement_flags(store, "RoadTechStart")?;
    place(&mut slots, "RoadTechStart", 0, sf, sg, "start", (1, 1), &mut x, &mut z, &mut row_h);
    for r in &singles {
        let (flags, ground) = placement_flags(store, &r.block)?;
        place(&mut slots, &r.block, 0, flags, ground, "north", r.size, &mut x, &mut z, &mut row_h);
    }
    newline(&mut x, &mut z, &mut row_h, 0);
    for r in &multis {
        let (flags, ground) = placement_flags(store, &r.block)?;
        place(&mut slots, &r.block, 0, flags, ground, "north", r.size, &mut x, &mut z, &mut row_h);
    }
    newline(&mut x, &mut z, &mut row_h, 1);
    let (pf, pg) = placement_flags(store, "RoadTechStraight")?;
    place(&mut slots, "RoadTechStraight", 1, pf, pg, "marker-east", (1, 1), &mut x, &mut z, &mut row_h);
    for r in &singles {
        let (flags, ground) = placement_flags(store, &r.block)?;
        place(&mut slots, &r.block, 1, flags, ground, "east", r.size, &mut x, &mut z, &mut row_h);
    }
    newline(&mut x, &mut z, &mut row_h, 0);
    for r in &multis {
        let (flags, ground) = placement_flags(store, &r.block)?;
        place(&mut slots, &r.block, 1, flags, ground, "east", r.size, &mut x, &mut z, &mut row_h);
    }
    newline(&mut x, &mut z, &mut row_h, 1);
    for r in &multis {
        let (flags, ground) = placement_flags(store, &r.block)?;
        place(&mut slots, &r.block, 2, flags, ground, "south", r.size, &mut x, &mut z, &mut row_h);
    }
    let (ff, fg) = placement_flags(store, "RoadTechFinish")?;
    place(&mut slots, "RoadTechFinish", 0, ff, fg, "finish", (1, 1), &mut x, &mut z, &mut row_h);
    let rows = z + row_h;
    Ok((slots, rows))
}

/// Build the catalogue map: strip the source's authored blocks and items, append the slots as grid records with
/// their waypoint tags (Checkpoint / Spawn / Goal). Returns (slots, map size z needed).
pub fn build_catalogue(store: &mut crate::store::DataStore, source: &str, out: &str, rules: &[Rule]) -> Result<Vec<Slot>, String> {
    let m0 = MapFile::try_load(std::path::Path::new(source))?;
    let grass_row = m0.blocks.iter().chain(m0.baked.iter()).filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none() && b.name.eq_ignore_ascii_case("Grass")).map(|b| b.coords().1).min().ok_or("source: no Grass tile")?;
    let width = m0.size[0];
    let (slots, rows_needed) = layout(store, rules, grass_row, width)?;
    if rows_needed > m0.size[2] {
        return Err(format!("the catalogue needs {rows_needed} rows in z, the source map has {}: use a wider pitch or a bigger map", m0.size[2]));
    }
    // 1. strip every authored block and every item (a fresh load per variable-length edit)
    let tmp = format!("{out}.stage1.Map.Gbx");
    {
        let mut m = MapFile::try_load(std::path::Path::new(source))?;
        let n = m.items.len();
        m.remove_items(|_| true);
        m.write_to(std::path::Path::new(&tmp)).map_err(|e| e.to_string())?;
        let _ = n;
    }
    let specs: Vec<FreeBlockSpec> = slots
        .iter()
        .map(|s| {
            let wp = match s.group {
                "start" => Some("Spawn".to_string()),
                "finish" => Some("Goal".to_string()),
                "marker-east" => None,
                _ => Some("Checkpoint".to_string()),
            };
            FreeBlockSpec { name: s.name.clone(), author: None, flags: s.flags, pos: [0.0; 3], rot: [0.0; 3], grid: Some(s.cell), dir: s.dir, waypoint: wp }
        })
        .collect();
    {
        let mut m = MapFile::try_load(std::path::Path::new(&tmp))?;
        m.remove_and_add_blocks(|b| b.flags != 0xFFFF_FFFF, |_| false, &specs);
        m.write_to(std::path::Path::new(out)).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_file(&tmp);
    Ok(slots)
}

/// Apply the mapping to a map: every checkpoint block → replacement at (dir + offset) % 4, the `none` ones removed.
/// Returns (replaced, removed, left: names that had no rule).
pub fn clean(store: &mut crate::store::DataStore, source: &str, out: &str, rules: &[Rule]) -> Result<(usize, usize, Vec<String>), String> {
    let by_name: BTreeMap<String, Rule> = rules.iter().map(|r| (r.block.clone(), r.clone())).collect();
    let m0 = MapFile::try_load(std::path::Path::new(source))?;
    let mut specs: Vec<FreeBlockSpec> = Vec::new();
    let mut drop: Vec<usize> = Vec::new();
    let (mut replaced, mut removed) = (0usize, 0usize);
    let mut left: Vec<String> = Vec::new();
    for b in &m0.blocks {
        if b.flags == 0xFFFF_FFFF || b.free_pos.is_some() {
            continue;
        }
        let is_cp = b.waypoint_tag.as_deref() == Some("Checkpoint") || b.name.contains("Checkpoint");
        if !is_cp {
            continue;
        }
        match by_name.get(&b.name) {
            Some(r) if r.verdict == "none" || r.verdict == "no-candidate" || r.replacement == "(remove)" => {
                drop.push(b.index);
                removed += 1;
            }
            Some(r) => {
                let (flags, _) = placement_flags(store, &r.replacement)?;
                let flags = flags | (b.flags & crate::blockmap::FLAG_GROUND);
                let c = b.coords();
                specs.push(FreeBlockSpec { name: r.replacement.clone(), author: None, flags, pos: [0.0; 3], rot: [0.0; 3], grid: Some([c.0, c.1, c.2]), dir: (b.dir + r.offset) & 3, waypoint: None });
                drop.push(b.index);
                replaced += 1;
            }
            None => left.push(b.name.clone()),
        }
    }
    let mut m = MapFile::try_load(std::path::Path::new(source))?;
    let drop_set: std::collections::HashSet<usize> = drop.into_iter().collect();
    m.remove_and_add_blocks(|b| drop_set.contains(&b.index), |_| false, &specs);
    m.write_to(std::path::Path::new(out)).map_err(|e| e.to_string())?;
    Ok((replaced, removed, left))
}
