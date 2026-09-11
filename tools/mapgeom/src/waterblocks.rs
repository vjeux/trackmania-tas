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
/// The archetype a plate's source block maps to (the DecoWallWater* clip plates are
/// the drawn top of a DecoWallWaterBase stack), with the band depth under the top.
/// DecoWallWaterBase measured 2026-09-10 23:00Z on 15's pool A (origin plane − 7):
/// Water at 1.5 m and at 5 m under the plane — the 8-m "Shallow" box, band origin −1..+7.
pub fn archetype_of(block: &str) -> Option<(&'static str, f32)> {
    let name = block.split_whitespace().next().unwrap_or(block);
    if name.starts_with("WaterBase") { return Some(("WaterBase", 4.0)); }
    if name.starts_with("DecoWallWater") { return Some(("DecoWallWaterBase", 8.0)); }
    None
}

pub fn archetype_depth(block: &str) -> Option<f32> {
    archetype_of(block).map(|(_, d)| d)
}

/// Where the volume TOP sits above the block origin (metres, full size) — from the
/// block info's water volume boxes (`mapgeom blockinfo NAME`): WaterBase boxes y 4..6
/// in 1-m units = 4..7 → top 7; DecoWallWaterBase one 32×8×32 unit = 0..8 → top 8
/// (the FCT plate plane sits at the block top); RoadWater* y 0..0 in 2-m units → top 2.
pub fn archetype_top_offset(block: &str) -> f32 {
    match archetype_of(block).map(|(a, _)| a) {
        Some("DecoWallWaterBase") => 8.0,
        _ => 7.0,
    }
}

pub struct Decision {
    pub water_cells: usize,
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

pub fn decide(plates: &[PlateRow], tris: &[UpTri], source: Option<&SourceUnder>) -> Vec<Decision> {
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
        let arche = archetype_of(&p.source_block).map(|(a, _)| a.to_string()).unwrap_or_default();
        let key = (arche, (p.plane_y * 20.0).round() as i32);
        // cell indices are RELATIVE to the pool later (the tiny grid is offset by the
        // anchor, not aligned to world multiples of 16); keep the raw corner here
        pools.entry(key).or_default().push(Cell { cx: p.xmin.round() as i32, cz: p.zmin.round() as i32, row: i });
    }
    // every water footprint at a plane: a surface inside one is wet floor, not a spill
    let wet: Vec<(f32, f32, f32, f32, f32)> = plates.iter().map(|p| (p.xmin, p.xmax, p.zmin, p.zmax, p.plane_y)).collect();
    let in_wet = |x: f32, z: f32, plane: f32| wet.iter().any(|(x0, x1, z0, z1, py)| (py - plane).abs() < 0.3 && x >= x0 - 0.01 && x <= x1 + 0.01 && z >= z0 - 0.01 && z <= z1 + 0.01);
    for ((arche, _), cells) in &pools {
        let depth_full = archetype_depth(arche).unwrap_or(1.0);
        let plane = plates[cells[0].row].plane_y;
        let (wx0, wz0) = (cells.iter().map(|c| c.cx).min().unwrap(), cells.iter().map(|c| c.cz).min().unwrap());
        // 16-m cell indices relative to the pool's min corner
        let occupied: BTreeSet<(i32, i32)> = cells.iter().map(|c| ((c.cx - wx0).div_euclid(16), (c.cz - wz0).div_euclid(16))).collect();
        let cell_world = |c: &(i32, i32)| -> (f32, f32) { ((wx0 + c.0 * 16) as f32, (wz0 + c.1 * 16) as f32) };
        // 32-m blocks on a lattice anchored at the pool's min corner, or shifted one
        // 16-m cell in x and/or z: the four lattices are tried and the one whose
        // ACCEPTED blocks cover the most water cells wins (an edge block that spills
        // onto a road under one lattice may be a full-water block under another)
        let mut best: Option<(usize, Vec<Decision>)> = None;
        for (offx, offz) in [(0i32, 0i32), (1, 0), (0, 1), (1, 1)] {
        let (minx, minz) = (-offx, -offz);
        let mut local: Vec<Decision> = Vec::new();
        let mut blocks: BTreeSet<(i32, i32)> = BTreeSet::new();
        for (cx, cz) in &occupied {
            blocks.insert(((cx - minx).div_euclid(2), (cz - minz).div_euclid(2)));
        }
        for (bi, bj) in blocks {
            let (c0x, c0z) = (minx + 2 * bi, minz + 2 * bj);
            let covered: Vec<(i32, i32)> = vec![(c0x, c0z), (c0x + 1, c0z), (c0x, c0z + 1), (c0x + 1, c0z + 1)];
            let water_cells: Vec<&(i32, i32)> = covered.iter().filter(|c| occupied.contains(c)).collect();
            let spill_cells: Vec<&(i32, i32)> = covered.iter().filter(|c| !occupied.contains(c)).collect();
            let (ox, oz) = cell_world(&(c0x, c0z));
            let band_lo = plane - depth_full;
            let band_hi = plane + 0.3;
            let in_cell = |x: f32, z: f32, c: &(i32, i32)| { let (cx, cz) = cell_world(c); x >= cx && x < cx + 16.0 && z >= cz && z < cz + 16.0 };
            // (1) drivable surfaces inside the spilled cells within the band (not wet floor)
            let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
            for t in tris {
                if t.phys == "Water" || t.phys == "NotCollidable" { continue; }
                if t.top < band_lo || t.top > band_hi { continue; }
                if spill_cells.iter().any(|c| in_cell(t.c[0], t.c[2], c)) && !in_wet(t.c[0], t.c[2], plane) {
                    *kinds.entry(t.phys.clone()).or_default() += 1;
                }
            }
            // (1b) under the pool: the band reaches depth_full under the plane; the pool's own
            // floor is the highest non-water surface under the plane in its cells — any
            // drivable surface between the band bottom and 0.3 m under that floor is a
            // road under the pool that would get water
            // a surface in the water cells within the band that has ANOTHER surface above
            // it (same 4-m bin, at least 0.5 m higher, still under the plane) is under the
            // pool's floor slab — a room or road the volume would flood. The pool's own
            // floor and ramps have nothing between them and the water.
            let in_water_cells = |x: f32, z: f32| water_cells.iter().any(|c| in_cell(x, z, c));
            let mut bins: BTreeMap<(i32, i32), Vec<f32>> = BTreeMap::new();
            for t in tris {
                if t.phys == "Water" || t.phys == "NotCollidable" { continue; }
                if t.top < plane - 0.4 && t.top >= band_lo - 0.5 && in_water_cells(t.c[0], t.c[2]) {
                    bins.entry(((t.c[0] / 4.0).floor() as i32, (t.c[2] / 4.0).floor() as i32)).or_default().push(t.top);
                }
            }
            // the SOURCE decides what is under the pool (the tiny geometry inside a pool —
            // ramps over the floor, pillar tops — defeats a height heuristic): drivable
            // source blocks in the cells under the water body within the band
            let _ = &bins;
            let floor = plane - depth_full / 2.0;
            let under: Vec<String> = match source {
                Some(s) => s.drivable_under(&water_cells.iter().map(|c| cell_world(c)).collect::<Vec<_>>(), plane, depth_full / 2.0),
                None => Vec::new(),
            };
            // (2) the sheet visible in the air over the spilled cells
            let (mut visible, mut samples) = (0usize, 0usize);
            for c in &spill_cells {
                for k in 0..4 {
                    for l in 0..4 {
                        let (cxw, czw) = cell_world(c);
                        let (x, z) = (cxw + 2.0 + 4.0 * k as f32, czw + 2.0 + 4.0 * l as f32);
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
            let body = format!("{arche} pool plane {plane:.2} block cells ({c0x},{c0z})..({},{}) [{} water, {} spill] items {}", c0x + 1, c0z + 1, water_cells.len(), spill_cells.len(), cells.iter().filter(|c| covered.contains(&((c.cx - wx0).div_euclid(16), (c.cz - wz0).div_euclid(16)))).map(|c| format!("i{}", plates[c.row].item_index)).collect::<Vec<_>>().join(","));
            let spill = format!("x {:.0}..{:.0} z {:.0}..{:.0} band {:.2}..{:.2}", ox, ox + 32.0, oz, oz + 32.0, band_lo, plane);
            if !under.is_empty() {
                local.push(Decision { water_cells: water_cells.len(), body, archetype: arche.clone(), choice: "item", reason: format!("drivable source block under the pool (band to ~{floor:.1}): {}", under.join("; ")), origin, spill });
            } else if !kinds.is_empty() {
                local.push(Decision { water_cells: water_cells.len(), body, archetype: arche.clone(), choice: "item", reason: format!("drivable surface in the spilled cells: {}", kinds.iter().map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(" ")), origin, spill });
            } else if samples > 0 && visible * 4 > samples {
                local.push(Decision { water_cells: water_cells.len(), body, archetype: arche.clone(), choice: "item", reason: format!("sheet visible in the air over {visible}/{samples} spill samples"), origin, spill });
            } else {
                // EMITTED archetype is always WaterBase (volume 4..7 of the block, no collision of
                // its own). A DecoWallWaterBase custom block puts a COLLIDABLE clip cap (ResonantMetal)
                // on its top face at origin + 8 — a metal lid at the pool plane (measured 2026-09-11
                // 01:55Z: a car dropped on a free DecoWallWaterBase block in open air lands on 22 at
                // origin + 8). Deep (DecoWallWater) pools get TWO WaterBase layers: bands
                // plane−3..plane and plane−6..plane−3.
                let reason = if spill_cells.is_empty() { "exact: all four cells are water".to_string() } else { format!("spill hidden: {visible}/{samples} samples open") };
                let layers: &[f32] = if arche == "DecoWallWaterBase" { &[7.0, 10.0] } else { &[7.0] };
                for (li, off) in layers.iter().enumerate() {
                    let body_l = if layers.len() > 1 { format!("{body} layer {}/{}", li + 1, layers.len()) } else { body.clone() };
                    local.push(Decision { water_cells: if li == 0 { water_cells.len() } else { 0 }, body: body_l, archetype: "WaterBase".to_string(), choice: "block", reason: reason.clone(), origin: [ox, plane - off, oz], spill: spill.clone() });
                }
            }
        }
        let score: usize = local.iter().filter(|d| d.choice == "block").map(|d| d.water_cells).sum();
        if best.as_ref().map(|b| score > b.0).unwrap_or(true) {
            best = Some((score, local));
        }
        }
        if let Some((_, v)) = best {
            out.extend(v);
        }
    }
    out
}

/// What the SOURCE map has under a water body: the blocks of the source cells a
/// tiny block covers, in the cells below the water body's own, down to where the
/// archetype's band reaches. A drivable block there (a road, a platform deck, an
/// open-tech piece) means the volume would flood a place the original keeps dry.
/// Cells: source x = sx + (tiny x − tx) × 2, cell = floor(x / 32); the file cell
/// is the game cell + (1, 0, 1).
pub struct SourceUnder<'a> {
    pub map: &'a tmmaps::map::MapFile,
    pub anchor_s: [f32; 3],
    pub anchor_t: [f32; 3],
}

impl SourceUnder<'_> {
    fn is_water_name(n: &str) -> bool {
        n.starts_with("Water") || n.starts_with("DecoWallWater") || n.starts_with("PlatformWater") || n.starts_with("RoadWater")
    }
    fn is_drivable_name(n: &str) -> bool {
        if n.contains("FC") || n.contains("Pillar") || Self::is_water_name(n) {
            return false;
        }
        n.starts_with("Road") || n.starts_with("Platform") || n.starts_with("OpenTech") || n.starts_with("DecoPlatform") || n.starts_with("Stand") || n.starts_with("Track")
    }
    /// The drivable source blocks under the water body in the given tiny cells
    /// (16-m corners) whose top would be inside a band reaching `under_m` tiny
    /// metres below the pool floor.
    pub fn drivable_under(&self, cells: &[(f32, f32)], plane_tiny: f32, under_m: f32) -> Vec<String> {
        let mut out = Vec::new();
        let plane_s = self.anchor_s[1] + (plane_tiny - self.anchor_t[1]) * 2.0;
        for (ox, oz) in cells {
            let sx = self.anchor_s[0] + (ox - self.anchor_t[0]) * 2.0;
            let sz = self.anchor_s[2] + (oz - self.anchor_t[2]) * 2.0;
            let (cx, cz) = ((sx / 32.0).floor() as i32, (sz / 32.0).floor() as i32);
            // the water body's own cell layer: the water blocks in this column whose
            // 8-m cell contains the plane
            // the water body's own cell layer: the water surface sits at local +7 of its
            // block (Stadium: cell y → base = cy*8 − 64; the plumb calibration cell 19 → 88)
            let wcy = ((plane_s - 7.0 + 64.0) / 8.0).round() as i32;
            let cells_under = ((under_m * 2.0) / 8.0).ceil() as i32;
            for b in self.map.blocks.iter().chain(self.map.baked.iter()) {
                if b.flags == 0xFFFF_FFFF { continue; }
                let (bx, by, bz) = (b.file_cell[0] as i32 - 1, b.file_cell[1] as i32, b.file_cell[2] as i32 - 1);
                if bx != cx || bz != cz { continue; }
                if by < wcy && by >= wcy - cells_under && Self::is_drivable_name(&b.name) {
                    out.push(format!("{} at ({cx},{by},{cz})", b.name));
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}
