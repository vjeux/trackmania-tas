//! `mapgeom waterblocks` — ship17's free custom WATER BLOCKS (2026-09-10).
//!
//! The engine's water is a BLOCK volume: drag + the Water material under the
//! wheels + the tint; an item plate is nothing to the car (measured, TINY.md
//! "Water"). An embedded custom block carries its ARCHETYPE's water volume
//! (probe 21:30Z), full 32-m size. So the water bodies a tiny map carries as
//! item plates are TILED with 32-m free custom blocks (archetype = the source
//! block's own name; the volume's top = the tiny plane): a pool of N 16-m
//! cells gets the 32-m blocks that cover it, anchored at the pool's min corner,
//! each covering four tiny cells. A block whose covered cells are all water is
//! exact; a block that reaches beyond the pool SPILLS into the other cells —
//! allowed only where those cells hold no drivable surface within the volume's
//! depth band and would not show a water sheet in the air (coordinator's rule,
//! 21:40Z). Bodies that fail keep the 13-item and are disclosed.

use std::collections::{BTreeMap, BTreeSet};

pub struct PlateRow {
    pub item_index: usize,
    pub model: String,
    pub source_block: String,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub plane_y: f32,
    pub xmin: f32,
    pub xmax: f32,
    pub zmin: f32,
    pub zmax: f32,
}

pub fn read_plates(path: &str) -> Result<Vec<PlateRow>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if i == 0 || l.trim().is_empty() {
            continue;
        }
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 13 {
            continue;
        }
        let f = |s: &str| s.trim().parse::<f32>().unwrap_or(f32::NAN);
        out.push(PlateRow {
            item_index: c[0].parse().unwrap_or(0),
            model: c[1].to_string(),
            source_block: c[2].to_string(),
            pos: [f(c[3]), f(c[4]), f(c[5])],
            yaw: f(c[6]),
            plane_y: f(c[7]),
            xmin: f(c[8]),
            xmax: f(c[9]),
            zmin: f(c[10]),
            zmax: f(c[11]),
        });
    }
    Ok(out)
}

/// The archetype families this pass handles: full-cell water bodies whose
/// volume fills the cell (rotation-free placement), with the volume's depth
/// below its top in FULL-size metres.
/// MEASURED 2026-09-10 22:30Z on a tiny 05 with free custom WaterBase blocks: the
/// volume band sits at block-origin +3 .. +7 (a car at origin +3.5 reads Water
/// 13, at origin +0.5 reads nothing) — the same band the WaterIceCornerIn probe
/// gave at base 88 (91–95). So the block origin goes at plane − 7 and the band
/// reaches 4 m under the plane.
pub fn archetype_depth(block: &str) -> Option<f32> {
    let name = block.split_whitespace().next().unwrap_or(block);
    match name {
        "WaterBase" | "WaterBaseDirt" | "WaterBaseIce" | "WaterBaseGrass" => Some(4.0),
        _ => None,
    }
}

/// Where the volume TOP sits above the block origin (metres, full size).
pub fn archetype_top_offset(_block: &str) -> f32 {
    7.0
}

pub struct Decision {
    pub body: String,
    pub archetype: String,
    pub choice: &'static str,
    pub reason: String,
    pub origin: [f32; 3],
    pub spill: String,
}

/// One upward collision triangle of the map (world), with its physics name.
pub struct UpTri {
    pub c: [f32; 3],
    pub top: f32,
    pub phys: String,
}

pub fn decide(plates: &[PlateRow], tris: &[UpTri]) -> Vec<Decision> {
    let mut out = Vec::new();
    // pools: plates of a handled archetype grouped by plane height (±0.05) and
    // by 16-m cell (the tiny cell of the source block)
    struct Cell {
        cx: i32,
        cz: i32,
        row: usize,
    }
    let mut pools: BTreeMap<(String, i32), Vec<Cell>> = BTreeMap::new();
    for (i, p) in plates.iter().enumerate() {
        let Some(_) = archetype_depth(&p.source_block) else { continue };
        let arche = p.source_block.split_whitespace().next().unwrap_or("").to_string();
        let key = (arche, (p.plane_y * 20.0).round() as i32);
        pools.entry(key).or_default().push(Cell { cx: (p.xmin / 16.0).floor() as i32, cz: (p.zmin / 16.0).floor() as i32, row: i });
    }
    // every water footprint at a plane: a surface inside one is wet floor, not a spill
    let wet: Vec<(f32, f32, f32, f32, f32)> = plates.iter().map(|p| (p.xmin, p.xmax, p.zmin, p.zmax, p.plane_y)).collect();
    let in_wet = |x: f32, z: f32, plane: f32| wet.iter().any(|(x0, x1, z0, z1, py)| (py - plane).abs() < 0.3 && x >= x0 - 0.01 && x <= x1 + 0.01 && z >= z0 - 0.01 && z <= z1 + 0.01);
    for ((arche, _), cells) in &pools {
        let depth_full = archetype_depth(arche).unwrap_or(1.0);
        let plane = plates[cells[0].row].plane_y;
        let occupied: BTreeSet<(i32, i32)> = cells.iter().map(|c| (c.cx, c.cz)).collect();
        let (minx, minz) = (cells.iter().map(|c| c.cx).min().unwrap(), cells.iter().map(|c| c.cz).min().unwrap());
        // 32-m blocks anchored at the pool's min corner: block (i, j) covers the
        // 16-m cells (minx + 2i .. +1, minz + 2j .. +1)
        let mut blocks: BTreeSet<(i32, i32)> = BTreeSet::new();
        for (cx, cz) in &occupied {
            blocks.insert(((cx - minx).div_euclid(2), (cz - minz).div_euclid(2)));
        }
        for (bi, bj) in blocks {
            let (c0x, c0z) = (minx + 2 * bi, minz + 2 * bj);
            let covered: Vec<(i32, i32)> = vec![(c0x, c0z), (c0x + 1, c0z), (c0x, c0z + 1), (c0x + 1, c0z + 1)];
            let water_cells: Vec<&(i32, i32)> = covered.iter().filter(|c| occupied.contains(c)).collect();
            let spill_cells: Vec<&(i32, i32)> = covered.iter().filter(|c| !occupied.contains(c)).collect();
            let (ox, oz) = (c0x as f32 * 16.0, c0z as f32 * 16.0);
            let band_lo = plane - depth_full;
            let band_hi = plane + 0.3;
            let in_cell = |x: f32, z: f32, c: &(i32, i32)| x >= c.0 as f32 * 16.0 && x < (c.0 + 1) as f32 * 16.0 && z >= c.1 as f32 * 16.0 && z < (c.1 + 1) as f32 * 16.0;
            // (1) drivable surfaces inside the spilled cells within the band (not wet floor)
            let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
            for t in tris {
                if t.phys == "Water" || t.phys == "NotCollidable" { continue; }
                if t.top < band_lo || t.top > band_hi { continue; }
                if spill_cells.iter().any(|c| in_cell(t.c[0], t.c[2], c)) && !in_wet(t.c[0], t.c[2], plane) {
                    *kinds.entry(t.phys.clone()).or_default() += 1;
                }
            }
            // (2) the sheet visible in the air over the spilled cells
            let (mut visible, mut samples) = (0usize, 0usize);
            for c in &spill_cells {
                for k in 0..4 {
                    for l in 0..4 {
                        let (x, z) = (c.0 as f32 * 16.0 + 2.0 + 4.0 * k as f32, c.1 as f32 * 16.0 + 2.0 + 4.0 * l as f32);
                        if in_wet(x, z, plane) { continue; }
                        samples += 1;
                        let mut best_below = f32::MIN;
                        let mut above = false;
                        for t in tris {
                            if (t.c[0] - x).abs() <= 2.0 && (t.c[2] - z).abs() <= 2.0 {
                                if t.top <= band_hi && t.top > best_below { best_below = t.top; }
                                if t.top > band_hi && t.top < plane + 8.0 { above = true; }
                            }
                        }
                        if !above && (best_below == f32::MIN || best_below < plane - 0.6) { visible += 1; }
                    }
                }
            }
            let origin = [ox, plane - archetype_top_offset(arche), oz];
            let body = format!("{arche} pool plane {plane:.2} block cells ({c0x},{c0z})..({},{}) [{} water, {} spill] items {}", c0x + 1, c0z + 1, water_cells.len(), spill_cells.len(), cells.iter().filter(|c| covered.contains(&(c.cx, c.cz))).map(|c| format!("i{}", plates[c.row].item_index)).collect::<Vec<_>>().join(","));
            let spill = format!("x {:.0}..{:.0} z {:.0}..{:.0} band {:.2}..{:.2}", ox, ox + 32.0, oz, oz + 32.0, band_lo, plane);
            if !kinds.is_empty() {
                out.push(Decision { body, archetype: arche.clone(), choice: "item", reason: format!("drivable surface in the spilled cells: {}", kinds.iter().map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(" ")), origin, spill });
            } else if samples > 0 && visible * 4 > samples {
                out.push(Decision { body, archetype: arche.clone(), choice: "item", reason: format!("sheet visible in the air over {visible}/{samples} spill samples"), origin, spill });
            } else {
                out.push(Decision { body, archetype: arche.clone(), choice: "block", reason: if spill_cells.is_empty() { "exact: all four cells are water".to_string() } else { format!("spill hidden: {visible}/{samples} samples open") }, origin, spill });
            }
        }
    }
    out
}
