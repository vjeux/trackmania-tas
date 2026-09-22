//! `mapgeom giantwater` — the ENGINE'S WATER for a giant (×N, whole N ≥ 2)
//! build, from the source map's water blocks (2026-09-13; vjeux: "Giant 10 is
//! missing water blocks, can't finish the map").
//!
//! An item carries no water volume (TINY.md "Water collision"); a block does.
//! A giant build is cell-aligned (`tmmaps tiny --anchor fit` snaps the anchor
//! so every source cell lands on N×N×N target cells), so the pools can be
//! NATIVE grid blocks — the very block the source placed, N×N tiles per cell:
//!
//! * `DecoWallWaterBase` (volume 0..8 of its cell, no mesh of its own — walls,
//!   floor and the water top are clip fillers) fills every one of the N rows a
//!   source block covers: the bottom tile row keeps the source flags (ground /
//!   `InDecoWallPillar` as the editor left them), the rows above are the plain
//!   air variant — the stack the source itself is (U10S_10: a ground row under
//!   an air row). The top row's `DecoWallWaterFCT` is the water surface at the
//!   row top, where the ×N build's own rim tops are.
//! * `WaterBase` (volume 4..7, its prefab a 3 m concrete basin with the water
//!   quad at 7) goes into the TOP tile row only: the surface lands 1 m under
//!   the ×N rim like the source's, the basin floor 3 m under it; the ×N basin
//!   floor (at 4N) stays hidden below. Tiles in the lower rows would leave dry
//!   bands (4..7 of each row) and a second, deeper floor — a 3 m pool that
//!   floats the car is what the source has too.
//!
//! The clip fillers the engine generates on the tiles' OUTER faces (plank
//! walls, rims, floor caps — hidden inside the ×N wall items, flush with their
//! tops) are the same pieces the source generated around its pool.
//!
//! The water ROADS (`RoadWater*`: a 26 m channel, volume 0..2 under a deck AT
//! the volume's top plane) cannot be tiled natively — a 1× road mesh inside a
//! 2× one — so they get FREE CUSTOM blocks, the tiny's form (the wood-platform
//! template re-pointed at the archetype, its deck 200 m down): N×N invisible
//! volumes per source block whose top plane is the ×N deck, contiguous across
//! the channel (26 m apart, not on the cell lattice) and butted along it so the
//! `RoadWaterVFC` end clips match tile to tile. The dead ends keep their
//! clip-less archetype (`RoadWaterStart` south, `RoadWaterFinish` north); every
//! other tile is a `RoadWaterStraight` — no end wall (`TrackWallWater` VFC,
//! 8 m of planks) anywhere inside the section.
//!
//! The item bake drops its `Water` visuals for such a build
//! (`TINY_WATER_VISUAL=0`): the native blocks and the volumes' sheets draw the
//! water.

use std::collections::BTreeMap;
use std::path::Path;

use tmmaps::map::{BlockRec, FreeBlockSpec, MapFile, CELL_XZ, CELL_Y};

/// Block flag bits (see `blockmap`).
const FLAG_GROUND: u32 = 1 << 12;
const FLAG_REPLACEMENT: u32 = 1 << 16;

/// The pool blocks tiled natively.
pub const POOL_BLOCKS: &[&str] = &["WaterBase", "DecoWallWaterBase"];

/// The water-road blocks given free custom volumes, and the archetype each
/// tile takes (the dead-end archetypes only at the dead-end tile).
pub fn road_family(name: &str) -> bool {
    name.starts_with("RoadWater")
}

pub struct Plan {
    pub grid: Vec<FreeBlockSpec>,
    pub roads: Vec<FreeBlockSpec>,
    /// archetype name -> archive ident (`Water\<Archetype>.Block.Gbx`)
    pub archetypes: BTreeMap<String, String>,
    pub notes: Vec<String>,
    pub skipped: Vec<String>,
    /// Pool tiles whose cell falls outside the map grid (a wide map doubled
    /// past the 48-cell arena): left out — the engine keeps ITEMS outside
    /// the grid but a grid block needs a cell inside it (2026-09-13: 18 of
    /// the 975 club maps, x −20…53).
    pub clipped: usize,
}

fn transform(p: [f32; 3], s: [f32; 3], t: [f32; 3], scale: f32) -> [f32; 3] {
    [t[0] + (p[0] - s[0]) * scale, t[1] + (p[1] - s[1]) * scale, t[2] + (p[2] - s[2]) * scale]
}

/// The giant cell a source cell maps onto (its min corner), or an error when
/// the anchor does not put it on the lattice.
fn giant_cell(src: (i32, i32, i32), ground: f32, s: [f32; 3], t: [f32; 3], scale: f32) -> Result<[i32; 3], String> {
    let corner = [src.0 as f32 * CELL_XZ, src.1 as f32 * CELL_Y + ground, src.2 as f32 * CELL_XZ];
    let g = transform(corner, s, t, scale);
    let cx = g[0] / CELL_XZ;
    let cy = (g[1] - ground) / CELL_Y;
    let cz = g[2] / CELL_XZ;
    for (v, name) in [(cx, "x"), (cy, "y"), (cz, "z")] {
        if (v - v.round()).abs() > 1e-3 {
            return Err(format!("source cell {:?} maps to a non-integer {name} cell {v:.3} — the anchor is not lattice-aligned (tmmaps tiny --anchor fit snaps it)", src));
        }
    }
    Ok([cx.round() as i32, cy.round() as i32, cz.round() as i32])
}

/// Yaw and origin corner of a block frame of size `size` (metres) at the
/// min corner `(x, z)`, the `tmmaps::tiny::block_origin` / `block_yaw`
/// convention: dir 0 → (x, z) yaw 0; dir 1 → (x+size, z) yaw −π/2; dir 2 →
/// (x+size, z+size) yaw π; dir 3 → (x, z+size) yaw +π/2. A local point
/// (lx, lz) of the frame is at origin + R(yaw)·(lx, lz) with R(+π/2): (lx, lz)
/// → (lz, −lx) (TINY.md "Free block rotation, measured").
fn frame(dir: u8, x: f32, z: f32, size: f32) -> ([f32; 2], f32) {
    match dir & 3 {
        0 => ([x, z], 0.0),
        1 => ([x + size, z], -std::f32::consts::FRAC_PI_2),
        2 => ([x + size, z + size], std::f32::consts::PI),
        _ => ([x, z + size], std::f32::consts::FRAC_PI_2),
    }
}

fn local_to_world(origin: [f32; 2], yaw: f32, lx: f32, lz: f32) -> [f32; 2] {
    let (s, c) = yaw.sin_cos();
    // R(yaw) with R(+π/2)·(lx, lz) = (lz, −lx): x' = lx·cos + lz·sin, z' = −lx·sin + lz·cos
    [origin[0] + lx * c + lz * s, origin[1] - lx * s + lz * c]
}

/// The plan for `source` scaled by `scale` (whole, ≥ 2) about the anchor.
pub fn plan(source: &MapFile, ground: f32, s: [f32; 3], t: [f32; 3], scale: f32, roads: bool, author: &str) -> Result<Plan, String> {
    plan_opt(source, ground, s, t, scale, roads, author, false)
}

/// `legacy_stack`: every DecoWallWaterBase row above the bottom as the AIR variant (the
/// 2026-09-13 form; each air tile carries its own `DecoWallWaterFCT` water sheet, so a
/// stack showed a sheet at every row — Everios96: "the water blocks are not connected to
/// each other vertically, creating a roof above the player"). The default since
/// 2026-09-14: a tile with a water tile ABOVE it takes the GROUND variant (no top
/// sheet; the source itself is a ground row under an air row), only the top of each
/// column is the air variant with the surface.
pub fn plan_opt(source: &MapFile, ground: f32, s: [f32; 3], t: [f32; 3], scale: f32, roads: bool, author: &str, legacy_stack: bool) -> Result<Plan, String> {
    plan_stack(source, ground, s, t, scale, roads, author, legacy_stack, STACKED_BELOW, None)
}

/// The editor's own word for a DecoWallWaterBase with another water block ABOVE it:
/// additional variant 1 ("InDecoWallPillar") + bit 16. Measured 2026-09-14 on giant 10
/// (`/mapblocks2?list=baked`): a plain tile (ground or air) under a water tile still
/// emits its `DecoWallWaterFCT` water sheet (a "roof" inside the pool — Everios96);
/// this variant under a water tile emits nothing; under a plain GROUND tile it emits
/// `DecoWallWaterBaseFCT` — a ResonantMetal platform plate, a lid. Every deep source
/// pool of the club carries exactly this: 0x210000 / 0x211000 on the rows below the
/// top, 0 on the top row (U10S_91, _131, _221).
pub const STACKED_BELOW: u32 = 0x0021_0000;

/// `below_flags`: the flag word of a DecoWallWaterBase tile that has another water tile
/// above it (the variant the engine matches against the tile above; measured 2026-09-14).
#[allow(clippy::too_many_arguments)]
pub fn plan_stack(source: &MapFile, ground: f32, s: [f32; 3], t: [f32; 3], scale: f32, roads: bool, author: &str, legacy_stack: bool, below_flags: u32, bounds: Option<[i32; 3]>) -> Result<Plan, String> {
    plan_free(source, ground, s, t, scale, roads, author, legacy_stack, below_flags, bounds, FreePools::None)
}

/// Which pool tiles become FREE custom blocks (the tiny water-block template
/// re-pointed at the pool archetype, positioned by metres, no cell) instead of
/// native grid tiles: none (the u10s form), every tile, or the tiles of the
/// pools that reach past the map grid (a grid tile there is dropped by the
/// engine at load — measured 2026-09-22 on giant Summer 15: 362 of 1056 tiles
/// gone, the map with its size words raised to 72 crashed the client).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FreePools {
    None,
    All,
    Outside,
}

#[allow(clippy::too_many_arguments)]
pub fn plan_free(source: &MapFile, ground: f32, s: [f32; 3], t: [f32; 3], scale: f32, roads: bool, author: &str, legacy_stack: bool, below_flags: u32, bounds: Option<[i32; 3]>, free: FreePools) -> Result<Plan, String> {
    // the cells kept: the map grid (the default) or, for the out-of-grid probe,
    // everything a cell byte can hold (`--no-clip`: x/z 0..254, y 0..255)
    let size = bounds.unwrap_or(source.size);
    // every DecoWallWaterBase tile cell first: the variant of a tile depends on
    // whether another water tile sits directly above it
    let mut water_cells: std::collections::HashSet<[i32; 3]> = std::collections::HashSet::new();
    if !legacy_stack {
        let n = scale.round() as i32;
        for b in source.blocks.iter().filter(|b| b.free_pos.is_none() && b.name == "DecoWallWaterBase") {
            let c = giant_cell(b.coords(), ground, s, t, scale)?;
            for j in 0..n {
                for i in 0..n {
                    for k in 0..n {
                        water_cells.insert([c[0] + i, c[1] + j, c[2] + k]);
                    }
                }
            }
        }
    }
    let n = scale.round() as i32;
    if n < 2 || (scale - n as f32).abs() > 1e-6 {
        return Err(format!("giantwater wants a whole scale of 2 or more, got {scale}"));
    }
    let mut p = Plan { grid: Vec::new(), roads: Vec::new(), archetypes: BTreeMap::new(), notes: Vec::new(), skipped: Vec::new(), clipped: 0 };
    let mut by_name: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    // The pools that become FREE tiles: per connected water body (pool blocks
    // 6-adjacent in the source grid), all or nothing — a native tile next to a
    // free one would not match clips with it (a rim wall through the pool).
    let free_blocks: std::collections::HashSet<usize> = match free {
        FreePools::None => Default::default(),
        FreePools::All => source.blocks.iter().filter(|b| b.free_pos.is_none() && POOL_BLOCKS.contains(&b.name.as_str())).map(|b| b.index).collect(),
        FreePools::Outside => {
            let pool: Vec<&BlockRec> = source.blocks.iter().filter(|b| b.free_pos.is_none() && POOL_BLOCKS.contains(&b.name.as_str())).collect();
            let cell_of: std::collections::HashMap<(i32, i32, i32), usize> = pool.iter().enumerate().map(|(i, b)| (b.coords(), i)).collect();
            // union-find over the pool blocks
            let mut parent: Vec<usize> = (0..pool.len()).collect();
            fn find(p: &mut [usize], i: usize) -> usize {
                let mut r = i;
                while p[r] != r {
                    r = p[r];
                }
                let mut j = i;
                while p[j] != r {
                    let n = p[j];
                    p[j] = r;
                    j = n;
                }
                r
            }
            for (i, b) in pool.iter().enumerate() {
                let (x, y, z) = b.coords();
                for d in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
                    if let Some(&j) = cell_of.get(&(x + d.0, y + d.1, z + d.2)) {
                        let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                        if ri != rj {
                            parent[ri] = rj;
                        }
                    }
                }
            }
            // a body reaches outside when any tile of any of its blocks lies past the grid
            let grid = size;
            let mut out_roots: std::collections::HashSet<usize> = Default::default();
            for (i, b) in pool.iter().enumerate() {
                let c = giant_cell(b.coords(), ground, s, t, scale)?;
                if c[0] < 0 || c[2] < 0 || c[0] + n > grid[0] || c[2] + n > grid[2] {
                    let r = find(&mut parent, i);
                    out_roots.insert(r);
                }
            }
            (0..pool.len()).filter(|&i| out_roots.contains(&find(&mut parent, i))).map(|i| pool[i].index).collect()
        }
    };
    let mut free_bodies_note = 0usize;
    for b in &source.blocks {
        if b.free_pos.is_some() {
            if POOL_BLOCKS.contains(&b.name.as_str()) || road_family(&b.name) {
                p.skipped.push(format!("{} #{} is a FREE block (no lattice tiling)", b.name, b.index));
            }
            continue;
        }
        if POOL_BLOCKS.contains(&b.name.as_str()) {
            let c = giant_cell(b.coords(), ground, s, t, scale)?;
            let e = by_name.entry(b.name.clone()).or_insert((0, 0));
            e.0 += 1;
            let rows: Vec<i32> = if b.name == "DecoWallWaterBase" { (0..n).collect() } else { vec![n - 1] };
            let rows_first = rows[0];
            for j in rows {
                let flags_of = |cell: [i32; 3]| -> u32 {
                    if legacy_stack {
                        return if j == 0 { b.flags } else { 0 };
                    }
                    let above = water_cells.contains(&[cell[0], cell[1] + 1, cell[2]]);
                    if above {
                        // a tile under another water tile: the editor's stacked variant; the
                        // ground bit only on the terrain tile (the bottom of a ground block)
                        let w = (b.flags & !FLAG_GROUND) | below_flags;
                        if j == 0 { w | (b.flags & FLAG_GROUND) } else { w }
                    } else {
                        // the column top: the plain air variant carries the surface (the
                        // source's other bits — ghost mode — kept)
                        b.flags & !(FLAG_GROUND | STACKED_BELOW)
                    }
                };
                let flags = if b.name == "DecoWallWaterBase" {
                    0 // per cell below
                } else {
                    // the top row is never the terrain row: an air tile, the rest of the word kept
                    b.flags & !(FLAG_GROUND | FLAG_REPLACEMENT)
                };
                let as_free = free_blocks.contains(&b.index);
                if as_free && j == rows_first {
                    free_bodies_note += 1;
                }
                for i in 0..n {
                    for k in 0..n {
                        let cell = [c[0] + i, c[1] + j, c[2] + k];
                        let flags = if b.name == "DecoWallWaterBase" { flags_of(cell) } else { flags };
                        if as_free {
                            // a FREE custom tile: the template re-pointed at the pool block,
                            // 32 m square at the cell's corner, turned like the source block;
                            // the variant bits ride in the same flag word (0x1000_8000 is the
                            // water-block convention of the tiny builds and the road tiles)
                            let ident = format!("Water\\{}.Block.Gbx", b.name);
                            p.archetypes.entry(b.name.clone()).or_insert(ident.clone());
                            let (origin, yaw) = frame(b.dir, cell[0] as f32 * CELL_XZ, cell[2] as f32 * CELL_XZ, CELL_XZ);
                            let y = cell[1] as f32 * CELL_Y + ground;
                            p.roads.push(FreeBlockSpec { name: format!("{ident}_CustomBlock"), author: Some(author.to_string()), flags: flags | 0x1000_8000, pos: [origin[0], y, origin[1]], rot: [yaw, 0.0, 0.0], grid: None, dir: 0 });
                            e.1 += 1;
                            continue;
                        }
                        if cell[0] < 0 || cell[1] < 0 || cell[2] < 0 || cell[0] >= size[0] || cell[1] >= size[1] || cell[2] >= size[2] {
                            p.clipped += 1;
                            continue;
                        }
                        p.grid.push(FreeBlockSpec { name: b.name.clone(), author: None, flags, pos: [0.0; 3], rot: [0.0; 3], grid: Some(cell), dir: b.dir });
                        e.1 += 1;
                    }
                }
            }
            continue;
        }
        if road_family(&b.name) {
            if !roads {
                p.skipped.push(format!("{} #{} (roads off)", b.name, b.index));
                continue;
            }
            let (kind, dead_end_k): (&str, Option<i32>) = match b.name.as_str() {
                "RoadWaterStraight" => ("RoadWaterStraight", None),
                "RoadWaterStart" => ("RoadWaterStart", Some(0)),
                "RoadWaterFinish" => ("RoadWaterFinish", Some(n - 1)),
                other if other.starts_with("RoadWaterSpecial") => ("RoadWaterStraight", None),
                other => {
                    p.skipped.push(format!("{other} #{}: not a straight channel (curve/branch/slope: no volume emitter)", b.index));
                    continue;
                }
            };
            let c = giant_cell(b.coords(), ground, s, t, scale)?;
            let size = CELL_XZ * n as f32;
            let (origin, yaw) = frame(b.dir, c[0] as f32 * CELL_XZ, c[2] as f32 * CELL_XZ, size);
            // the deck is AT the volume's top plane (+2 of the tile); the ×N deck is at 2N
            let y = c[1] as f32 * CELL_Y + ground + 2.0 * n as f32 - 2.0;
            let e = by_name.entry(b.name.clone()).or_insert((0, 0));
            e.0 += 1;
            for i in 0..n {
                // the tile's volume (local x 3..29) must cover channel x 3N + 26i .. 3N + 26(i+1)
                let lx = 3.0 * n as f32 + 26.0 * i as f32 - 3.0;
                for k in 0..n {
                    let lz = CELL_XZ * k as f32;
                    let arch = match dead_end_k {
                        Some(d) if d == k => kind,
                        _ => "RoadWaterStraight",
                    };
                    let ident = format!("Water\\{arch}.Block.Gbx");
                    p.archetypes.entry(arch.to_string()).or_insert(ident.clone());
                    let w = local_to_world(origin, yaw, lx, lz);
                    p.roads.push(FreeBlockSpec { name: format!("{ident}_CustomBlock"), author: Some(author.to_string()), flags: 0x1000_8000, pos: [w[0], y, w[1]], rot: [yaw, 0.0, 0.0], grid: None, dir: 0 });
                    e.1 += 1;
                }
            }
        }
    }
    if free != FreePools::None {
        p.notes.push(format!("{free_bodies_note} pool blocks as FREE custom tiles ({free:?}), {} free tiles in all", p.roads.len()));
    }
    for (name, (blocks, tiles)) in &by_name {
        p.notes.push(format!("{name}: {blocks} source blocks -> {tiles} tiles"));
    }
    if p.clipped > 0 {
        p.notes.push(format!("{} pool tiles outside the {}x{}x{} grid CLIPPED (no water volume there; the items stay)", p.clipped, size[0], size[1], size[2]));
    }
    Ok(p)
}

/// Parse `sx,sy,sz:tx,ty,tz`.
pub fn parse_anchor(a: &str) -> Result<([f32; 3], [f32; 3]), String> {
    let (l, r) = a.split_once(':').ok_or("--anchor sx,sy,sz:tx,ty,tz")?;
    let v = |t: &str| -> Result<[f32; 3], String> {
        let f: Vec<f32> = t.split(',').map(|x| x.trim().parse::<f32>().map_err(|_| format!("--anchor: bad number `{x}`"))).collect::<Result<_, _>>()?;
        if f.len() != 3 {
            return Err("--anchor: three numbers a side".into());
        }
        Ok([f[0], f[1], f[2]])
    };
    Ok((v(l)?, v(r)?))
}

/// Write the plan into `giant` → `out`: archive entries + manifest rows for the
/// road archetypes (from `template`, re-pointed and renamed like `waterblocks`),
/// then the block records. Returns (grid tiles, road tiles).
pub fn apply(giant: &Path, out: &Path, plan: &Plan, template: Option<&Path>, author: &str) -> Result<(usize, usize), String> {
    apply_opt(giant, out, plan, template, author, false)
}

/// `rewater`: the map's existing pool tiles (grid records of `POOL_BLOCKS`) are
/// dropped before the plan's are added — a water-only rebuild of a finished giant
/// map, everything else byte-identical (2026-09-14).
pub fn apply_opt(giant: &Path, out: &Path, plan: &Plan, template: Option<&Path>, author: &str, rewater: bool) -> Result<(usize, usize), String> {
    if plan.grid.is_empty() && plan.roads.is_empty() && !rewater {
        std::fs::copy(giant, out).map_err(|e| format!("{}: {e}", out.display()))?;
        return Ok((0, 0));
    }
    let mut stage = giant.to_path_buf();
    let tmp_map = out.with_extension("gw1.Map.Gbx");
    if !plan.roads.is_empty() {
        let tpl = template.ok_or("road tiles need --template T.Block.Gbx (tinyctl/assets/water-template.Block.Gbx)")?;
        let tpl = std::fs::read(tpl).map_err(|e| format!("{}: {e}", tpl.display()))?;
        let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for (arch, ident) in &plan.archetypes {
            let zip_path = format!("Blocks/Water/{arch}.Block.Gbx");
            let tmp_in = std::env::temp_dir().join(format!("gw-{arch}.in.Block.Gbx"));
            let tmp_out = std::env::temp_dir().join(format!("gw-{arch}.Block.Gbx"));
            std::fs::write(&tmp_in, &tpl).map_err(|e| e.to_string())?;
            let status = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .args(["blockitem-archetype", tmp_in.to_str().unwrap(), "--out", tmp_out.to_str().unwrap(), "--archetype", arch])
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("blockitem-archetype failed for {arch}"));
            }
            let raw = std::fs::read(&tmp_out).map_err(|e| e.to_string())?;
            let (old_ident, _) = tmmaps::header::item_ident_author(&raw).ok_or("template block: no ident in the header")?;
            files.insert(zip_path, crate::crystal::rename_ident(&raw, &old_ident, ident));
            let _ = std::fs::remove_file(&tmp_in);
            let _ = std::fs::remove_file(&tmp_out);
        }
        let mut m2 = MapFile::load(giant);
        let (mut zip, existing) = tmmaps::header::embedded_zip_bytes(&m2.gbx.body).unwrap_or_default();
        for (path, bytes) in &files {
            zip = tmmaps::header::zip_add(&zip, path, bytes);
        }
        let kept: Vec<String> = existing.iter().filter(|n| n.to_ascii_lowercase().ends_with(".item.gbx")).map(|n| n.rsplit(['/', '\\']).next().unwrap_or(n).to_string()).collect();
        let block_rows: Vec<(String, String)> = files.keys().map(|zp| (zp.trim_start_matches("Blocks/").replace('/', "\\"), author.to_string())).collect();
        let mut manifest: Vec<(&str, &str)> = kept.iter().map(|n| (n.as_str(), n.as_str())).collect();
        manifest.extend(block_rows.iter().map(|(i, au)| (i.as_str(), au.as_str())));
        m2.replace_embedded_objects(&manifest, &zip);
        m2.write_to(&tmp_map).map_err(|e| e.to_string())?;
        stage = tmp_map.clone();
    }
    let mut m3 = MapFile::load(&stage);
    let mut specs: Vec<FreeBlockSpec> = plan.grid.clone();
    specs.extend(plan.roads.iter().cloned());
    let r = m3.remove_and_add_blocks(|b| rewater && b.free_pos.is_none() && POOL_BLOCKS.contains(&b.name.as_str()), |_| false, &specs);
    if rewater {
        println!("  giantwater: {} existing pool tiles dropped", r.blocks);
    }
    m3.write_to(out).map_err(|e| e.to_string())?;
    if stage == tmp_map {
        let _ = std::fs::remove_file(&tmp_map);
    }
    let check = MapFile::load(out);
    let grid_n = check.blocks.iter().filter(|b| b.free_pos.is_none() && POOL_BLOCKS.contains(&b.name.as_str())).count();
    let free_n = check.blocks.iter().filter(|b| b.free_pos.is_some()).count();
    println!("  giantwater: {} pool tiles + {} road tiles written (Id table {} -> {}); the map now holds {} blocks", plan.grid.len(), plan.roads.len(), r.table_before, r.table_after, check.blocks.len());
    if grid_n != plan.grid.len() || free_n < plan.roads.len() {
        return Err(format!("giantwater: readback holds {grid_n} pool tiles / {free_n} free blocks, wanted {} / {}", plan.grid.len(), plan.roads.len()));
    }
    Ok((plan.grid.len(), plan.roads.len()))
}

/// One BlockRec's game cell for callers that only have the record.
pub fn cell_of(b: &BlockRec) -> (i32, i32, i32) {
    b.coords()
}
