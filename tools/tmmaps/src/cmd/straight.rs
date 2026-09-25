//! `tmmaps straight SOURCE.Map.Gbx --out F [--name N] [--blocks 24] [--cps 12,18,24]
//!     [--x-cell 24] [--z0 6] [--tree-step 12] [--no-hills] [--no-trees]
//!     [--specials CZ[-CZ2]:BLOCK[@DIR][^DY],…]`
//!
//! A purpose-built showcase map (vjeux, 2026-09-14): ONE straight RoadTech road along +z
//! at ground level, start at one end, finish at the other, checkpoints at regular
//! intervals, and BOTH sides lined with scenery — two tiers of `DecoHillSlope2Straight`
//! rising away from the road and three rows of mixed stock trees standing on the slopes.
//! The scenery exists to be stripped again and carried by parked ghosts wearing season
//! skins, so the driven surface itself stays plain.
//!
//! The waypoint blocks are the SOURCE map's own start / checkpoint / finish records, moved
//! to the line and renamed to their RoadTech forms: our writer cannot mint the
//! CGameWaypointSpecialProperty node a waypoint record carries, so we keep the ones a
//! Nadeo map already has (Summer 2025 - 02 has one start, three checkpoints and two
//! finishes). Everything else of the source goes: every other authored block, every
//! generated block but the `Grass` floor, and every item (sunk to y −900, the way `tiny`
//! parks a dropped item).
//!
//! Cells: x = cx·32, y = 8·(cy − 8) + 2 at the surface (ground cy 9 → y 10), z = cz·32.
//! `DecoHillSlope2Straight` rises 16 m toward its +z at dir 0 (measured on the Summer
//! 2025 - 02 bake); dir 1 turns it toward −x, dir 3 toward +x.

use tmmaps::cli;
use tmmaps::map::{self, FreeBlockSpec, MapFile};
use std::path::PathBuf;

const GROUND_CY_DEFAULT: i32 = 9;

pub fn straight(args: &[String]) {
    let src = PathBuf::from(args.get(2).expect("straight SOURCE.Map.Gbx --out F"));
    let out = PathBuf::from(cli::flag(args, "--out").expect("--out F"));
    let name = cli::flag(args, "--name").unwrap_or("Straight Seasons").to_string();
    let n_blocks: i32 = cli::flag(args, "--blocks").unwrap_or("24").parse().expect("--blocks N");
    let x_cell: i32 = cli::flag(args, "--x-cell").unwrap_or("24").parse().expect("--x-cell N");
    let z0: i32 = cli::flag(args, "--z0").unwrap_or("6").parse().expect("--z0 N");
    let cps: Vec<i32> = cli::flag(args, "--cps").unwrap_or("12,18,24").split(',').filter(|s| !s.is_empty()).map(|s| s.trim().parse().expect("--cps a,b,c (cells)")).collect();
    let tree_step: f32 = cli::flag(args, "--tree-step").unwrap_or("12").parse().expect("--tree-step METRES");
    let hills = !cli::has(args, "--no-hills");
    let trees = !cli::has(args, "--no-trees");
    let rename = !cli::has(args, "--no-rename");
    // --stop-after N: write stage N's result as the output and stop (bisecting a broken map)
    let stop_after: u32 = cli::flag(args, "--stop-after").unwrap_or("99").parse().expect("--stop-after N");
    let stop = |n: u32, tmp: &PathBuf, out: &PathBuf| { if stop_after == n { std::fs::copy(tmp, out).expect("copy"); println!("stopped after stage {n}: {}", out.display()); std::process::exit(0); } };
    #[allow(non_snake_case)]
    let GROUND_CY: i32 = cli::flag(args, "--line-y").unwrap_or("9").parse().expect("--line-y N");
    let _ = GROUND_CY_DEFAULT;
    let z_start = z0;
    let z_finish = z0 + n_blocks - 1;
    assert!(cps.iter().all(|c| *c > z_start && *c < z_finish), "every checkpoint cell must lie strictly between start {z_start} and finish {z_finish}");

    // ---- stage 1: keep one start, the checkpoints we need and one finish; drop the rest
    let source = MapFile::load(&src);
    let mut keep: Vec<usize> = Vec::new();
    let pick = |tag: &str, n: usize, keep: &mut Vec<usize>| {
        let mut found = 0;
        for b in &source.blocks {
            if b.waypoint_tag.as_deref() == Some(tag) && found < n {
                keep.push(b.index);
                found += 1;
            }
        }
        assert_eq!(found, n, "the source has only {found} {tag} block(s); {n} needed");
    };
    pick("Spawn", 1, &mut keep);
    pick("Checkpoint", cps.len(), &mut keep);
    // BOTH finish records: Summer 2025 - 02's finish is a linked pair (two records, one
    // waypoint group) and keeping only one of them leaves the map with NO working
    // waypoints at all — the engine then spawns the car at the world origin (measured
    // 2026-09-14 with the headless oracle: k-PlatformDirtFinish spawns, k2 falls forever).
    let n_goal = source.blocks.iter().filter(|b| b.waypoint_tag.as_deref() == Some("Goal")).count();
    pick("Goal", n_goal, &mut keep);
    println!("keeping source records {keep:?} (start, {} checkpoint(s), finish)", cps.len());
    let tmp1 = out.with_extension("s1.Map.Gbx");
    {
        let mut m = MapFile::load(&src);
        let keep2 = keep.clone();
        // --keep-baked all|grass (default grass): which generated blocks survive stage 1
        let keep_baked_all = cli::flag(args, "--keep-baked").map(|v| v == "all").unwrap_or(false);
        // --also-keep PAT,PAT: authored blocks whose name contains a pattern survive too (bisection)
        let also: Vec<String> = cli::flag(args, "--also-keep").unwrap_or("").split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
        let r = m.remove_blocks(move |b| !keep2.contains(&b.index) && !also.iter().any(|p| b.name.contains(p.as_str())), move |b| !keep_baked_all && b.name != "Grass");
        println!("stage 1: removed {} authored + {} generated blocks", r.blocks, r.baked);
        m.write_to(&tmp1).expect("write stage 1");
    }
    stop(1, &tmp1, &out);
    // ---- stage 2: renames (their own write): the RoadTech forms, a fresh uid, the name
    let tmp2 = out.with_extension("s2.Map.Gbx");
    {
        let mut m = MapFile::load(&tmp1);
        for b in m.blocks.clone() {
            if !rename { break; }
            match b.waypoint_tag.as_deref() {
                Some("Spawn") => m.set_block_name(b.index, "RoadTechStart"),
                Some("Checkpoint") => m.set_block_name(b.index, "RoadTechCheckpoint"),
                Some("Goal") => m.set_block_name(b.index, "RoadTechFinish"),
                _ => {}
            }
        }
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        m.set_map_uid(&format!("Str8{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000));
        m.write_to(&tmp2).expect("write stage 2");
    }
    // ---- stage 2b: the map name is a variable-length splice — its own write
    {
        let mut m = MapFile::load(&tmp2);
        // --source-name: what the source declares (Summer 2025 - 02 by default)
        let old = cli::flag(args, "--source-name").unwrap_or("Summer 2025 - 02").to_string();
        let (a, b) = m.set_map_name(&old, &name);
        println!("stage 2: map name {old:?} -> {name:?} ({a} body, {b} header)");
        m.write_to(&tmp2).expect("write stage 2b");
    }
    stop(2, &tmp2, &out);
    // ---- stage 3: fixed-size edits — move the waypoints onto the line, normalise their
    // flags, sink every source item
    let tmp3 = out.with_extension("s3.Map.Gbx");
    {
        let mut m = MapFile::load(&tmp2);
        let mut cp_i = 0usize;
        let mut goal_i = 0i32;
        for b in m.blocks.clone() {
            let cz = match b.waypoint_tag.as_deref() {
                Some("Spawn") => z_start,
                // the linked finish pair stands in a row at the end of the line
                Some("Goal") => { let c = z_finish + goal_i; goal_i += 1; c }
                Some("Checkpoint") => { let c = cps[cp_i]; cp_i += 1; c }
                _ => continue,
            };
            m.move_block_cell(b.index, (x_cell, GROUND_CY, cz));
            m.set_block_dir(b.index, 0);
            // 0x100000 = carries a waypoint node (structural — keep); bits 21/22 belonged
            // to the dirt slope variants — clear them for the flat RoadTech forms
            let flags = (b.flags & !0x0060_0000) | 0x0010_0000;
            m.set_block_flags(b.index, flags);
            println!("  {} -> cell ({x_cell}, {GROUND_CY}, {cz}) dir 0 flags {flags:08X}", b.name);
        }
        let n_items = m.items.len();
        for i in 0..n_items {
            m.move_item(i, [8.0, -900.0, 8.0], 0.0, (0, 0, 0));
        }
        println!("stage 3: {n_items} source items sunk");
        // the header's advertisement <dep> list goes too (same-length blanking)
        let nd = tmmaps::header::strip_deps_in(&mut m);
        m.write_to(&tmp3).expect("write stage 3");
        println!("stage 3: {nd} bytes of header <deps> blanked");
    }
    // ---- stage 3b: the sunk items' skin FileRefs (advertisement screens carry skins with
    // a zero checksum and a locator — the client sits in "Updating data…" forever on them)
    {
        let mut m = MapFile::load(&tmp3);
        let mut n = 0;
        for it in m.items.clone() {
            if it.skin_region.is_some() || it.flags & 4 != 0 {
                m.set_item_skin(it.index, None);
                n += 1;
            }
        }
        m.write_to(&tmp3).expect("write stage 3b");
        println!("stage 3b: {n} item skin reference(s) removed");
    }
    stop(3, &tmp3, &out);
    // ---- stage 4: the road and the hills (grid blocks appended)
    let tmp4 = out.with_extension("s4.Map.Gbx");
    {
        let mut m = MapFile::load(&tmp3);
        let mut specs: Vec<FreeBlockSpec> = Vec::new();
        let grid = |name: &str, cx: i32, cy: i32, cz: i32, dir: u8| FreeBlockSpec { name: name.to_string(), author: None, flags: 0, pos: [0.0; 3], rot: [0.0; 3], grid: Some([cx, cy, cz]), dir };
        let n_goal = m.blocks.iter().filter(|b| b.waypoint_tag.as_deref() == Some("Goal")).count() as i32;
        let waypoint_cells: Vec<i32> = std::iter::once(z_start).chain(cps.iter().cloned()).chain((0..n_goal).map(|k| z_finish + k)).collect();
        // --specials CZ[-CZ2]:BLOCK[@DIR][^DY][,…]: the road cell(s) at CZ (a range CZ-CZ2
        // inclusive) carry BLOCK instead of the plain RoadTechStraight (a RoadTechSpecialBoost
        // reactor pad, a PlatformTechBase run…). A `Gate*` block is ADDED to its cell on top of
        // whatever the cell's ground block is (another special, else the plain road), the way
        // the editor stacks a ring gate over a surface (the reactor-contact probe map,
        // 2026-09-25). @DIR rotates it (0..3, default 0), ^DY lifts it DY cells. A ring gate
        // over a RoadTech piece is a WALL at either dir 0 or dir 1 (the 13:44 and 13:56 runs
        // stopped dead in the gate's cell): the road surface stands 2 m above the cell floor the
        // gate is built for, so rings go on a platform run.
        // ~YM (or ~~YM): a FREE block instead of a grid one, YM metres above the cell floor,
        // its position at the cell's min corner (~) or centre (~~) -- the stock ring gates
        // stand on the cell floor with their hoop's hole starting ~2.4 m up, above a car
        // on the 2 m road/platform surface, so a hoop a car can drive through is sunk.
        let mut specials: Vec<(i32, String, u8, i32)> = Vec::new();
        let mut free_specials: Vec<(i32, String, u8, f32, bool)> = Vec::new();
        for s in cli::flag(args, "--specials").unwrap_or("").split(',').filter(|s| !s.is_empty()) {
            let (cells, rest) = s.split_once(':').expect("--specials CZ[-CZ2]:BLOCK[@DIR][^DY][~YM],…");
            let (rest, free_y) = match rest.split_once('~') {
                Some((a, b)) => { let centre = b.starts_with('~'); (a, Some((b.trim_start_matches('~').trim().parse::<f32>().expect("--specials: ~YM is metres"), centre))) }
                None => (rest, None),
            };
            let (rest, dy) = match rest.split_once('^') {
                Some((a, b)) => (a, b.trim().parse::<i32>().expect("--specials: ^DY is a cell count")),
                None => (rest, 0),
            };
            let (name, dir) = match rest.split_once('@') {
                Some((a, b)) => (a, b.trim().parse::<u8>().expect("--specials: @DIR is 0..3")),
                None => (rest, 0u8),
            };
            let (a, b) = match cells.split_once('-') {
                Some((a, b)) => (a.trim().parse::<i32>().expect("--specials: CZ"), b.trim().parse::<i32>().expect("--specials: CZ2")),
                None => { let c = cells.trim().parse::<i32>().expect("--specials: CZ is a cell number"); (c, c) }
            };
            for cz in a..=b {
                match free_y {
                    Some((ym, centre)) => free_specials.push((cz, name.trim().to_string(), dir, ym, centre)),
                    None => specials.push((cz, name.trim().to_string(), dir, dy)),
                }
            }
        }
        for (cz, name, _, _) in &specials {
            assert!(*cz > z_start && *cz < z_finish && !waypoint_cells.contains(cz), "--specials cell {cz} ({name}) must be a plain road cell strictly between start {z_start} and finish {z_finish}");
        }
        let mut n_road = 0;
        let mut n_special = 0;
        for cz in z_start..=z_finish {
            if waypoint_cells.contains(&cz) {
                continue;
            }
            let here: Vec<&(i32, String, u8, i32)> = specials.iter().filter(|(c, _, _, _)| *c == cz).collect();
            if !here.iter().any(|(_, name, _, _)| !name.starts_with("Gate")) {
                specs.push(grid("RoadTechStraight", x_cell, GROUND_CY, cz, 0));
            }
            for (_, name, dir, dy) in &here {
                specs.push(grid(name, x_cell, GROUND_CY + dy, cz, *dir));
                n_special += 1;
            }
            for (_, name, dir, ym, centre) in free_specials.iter().filter(|(c, _, _, _, _)| *c == cz) {
                let half = if *centre { 16.0 } else { 0.0 };
                let floor_y = 8.0 * (GROUND_CY as f32 - 8.0);
                specs.push(FreeBlockSpec { name: name.to_string(), author: None, flags: 0, pos: [x_cell as f32 * 32.0 + half, floor_y + ym, cz as f32 * 32.0 + half], rot: [(*dir as f32) * std::f32::consts::FRAC_PI_2, 0.0, 0.0], grid: None, dir: *dir });
                println!("  free {name} at ({:.1}, {:.1}, {:.1}) yaw {} quarter turn(s)", x_cell as f32 * 32.0 + half, floor_y + ym, cz as f32 * 32.0 + half, dir);
                n_special += 1;
            }
            n_road += 1;
        }
        if n_special > 0 {
            println!("stage 4: {n_special} special cell(s): {}", specials.iter().map(|(c, n, d, dy)| format!("{c}:{n}@{d}^{dy}")).collect::<Vec<_>>().join(" "));
        }
        let mut n_hill = 0;
        if hills {
            for cz in (z_start - 1)..=(z_finish + n_goal) {
                // west (−x): tier 1 beside the road rising toward −x, tier 2 behind it
                specs.push(grid("DecoHillSlope2Straight", x_cell - 1, GROUND_CY, cz, 1));
                specs.push(grid("DecoHillSlope2Straight", x_cell - 2, GROUND_CY + 2, cz, 1));
                // east (+x)
                specs.push(grid("DecoHillSlope2Straight", x_cell + 1, GROUND_CY, cz, 3));
                specs.push(grid("DecoHillSlope2Straight", x_cell + 2, GROUND_CY + 2, cz, 3));
                n_hill += 4;
            }
        }
        // the generated `Grass` floor: the editor only keeps it where no ground-level
        // block stands, so the cells our road and tier-1 hills occupy lose theirs
        // (--grass keep|drop|carve; carve is the editor's behaviour)
        let grass_mode = cli::flag(args, "--grass").unwrap_or("carve").to_string();
        let occupied: std::collections::BTreeSet<(i32, i32, i32)> = specs.iter().filter_map(|s| s.grid.map(|g| (g[0], g[1], g[2])))
            .chain(m.blocks.iter().filter(|b| b.waypoint_tag.is_some()).map(|b| b.coords()))
            .collect();
        let drop_baked = move |b: &map::BlockRec| -> bool {
            match grass_mode.as_str() {
                "drop" => true,
                "carve" => b.name == "Grass" && occupied.contains(&b.coords()),
                _ => false,
            }
        };
        let r = m.remove_and_add_blocks(|_| false, drop_baked, &specs);
        println!("stage 4: added {n_road} road blocks + {n_hill} hill blocks, {} generated floor block(s) removed (table {} -> {})", r.baked, r.table_before, r.table_after);
        m.write_to(&tmp4).expect("write stage 4");
    }
    stop(4, &tmp4, &out);
    // ---- stage 5: the trees — item slots first (variable-length), then the fields
    let tmp5 = out.with_extension("s5.Map.Gbx");
    let mut placements: Vec<(String, [f32; 3], f32)> = Vec::new();
    if trees {
        let species = ["SummerPalmTree", "Summer", "FirTall", "PalmTreeDirtMedium", "Summer", "SummerPalmTree", "FirMedium", "PalmTreeMedium"];
        let road_x = (x_cell as f32 + 0.5) * 32.0;
        let z_from = z_start as f32 * 32.0 + 8.0;
        let z_to = (z_finish + 1) as f32 * 32.0 - 8.0;
        // rows: metres off the road EDGE (16 m from the centre), on the slopes
        let rows: [f32; 3] = [4.0, 20.0, 40.0];
        let surface_y = |d: f32| -> f32 {
            // d = metres from the road edge; tier 1 spans 0..32 (10 -> 26), tier 2 32..64 (26 -> 42)
            if d < 32.0 { 10.0 + 16.0 * d / 32.0 } else { 26.0 + 16.0 * (d - 32.0) / 32.0 }
        };
        let mut k = 0usize;
        let mut z = z_from;
        while z <= z_to {
            for (ri, d) in rows.iter().enumerate() {
                let zz = z + ri as f32 * tree_step / 3.0; // stagger the rows
                if zz > z_to { continue; }
                for side in [-1.0f32, 1.0] {
                    let sp = species[k % species.len()];
                    k += 1;
                    let x = road_x + side * (16.0 + d);
                    let y = surface_y(*d) - 0.3;
                    let yaw = ((k * 37) % 360) as f32 * std::f32::consts::PI / 180.0;
                    placements.push((sp.to_string(), [x, y, zz], yaw));
                }
            }
            z += tree_step;
        }
    }
    {
        let mut m = MapFile::load(&tmp4);
        let n = m.items.len();
        m.append_item_clones(n + placements.len());
        m.write_to(&tmp5).expect("write stage 5 slots");
        let mut m = MapFile::load(&tmp5);
        tmmaps::tiny::set_ground(m.items.first().map(|it| it.collection_raw).unwrap_or(26));
        for (k, (sp, pos, yaw)) in placements.iter().enumerate() {
            let i = n + k;
            m.set_item_model(i, sp);
            m.set_item_author(i, "Nadeo");
            m.move_item(i, *pos, *yaw, tmmaps::tiny::cell_for_pub(*pos));
            m.set_item_frame(i, [*yaw, 0.0, 0.0], [0.0; 3]);
            m.set_item_scale(i, 1.0);
            m.set_item_variant(i, 0);
            m.set_item_color(i, 0);
        }
        m.write_to(&out).expect("write output");
        println!("stage 5: {} trees placed", placements.len());
    }
    for t in [&tmp1, &tmp2, &tmp3, &tmp4, &tmp5] {
        let _ = std::fs::remove_file(t);
    }
    // ---- control: read the result back
    let m = MapFile::load(&out);
    let mut by_name: std::collections::BTreeMap<String, usize> = Default::default();
    for b in &m.blocks {
        *by_name.entry(b.name.clone()).or_insert(0) += 1;
    }
    println!("wrote {}: {} blocks, {} items ({} of them ours)", out.display(), m.blocks.len(), m.items.len(), placements.len());
    for (n, c) in by_name {
        println!("  {c} x {n}");
    }
    let wps: Vec<String> = m.blocks.iter().filter(|b| b.waypoint_tag.is_some()).map(|b| format!("{}@{:?}", b.name, b.coords())).collect();
    println!("  waypoints: {}", wps.join(", "));
    let _ = map::FREE_BLOCK_FLAG;
}

/// `tmmaps wpprobe SOURCE --out F --what flags|rename|move` — one waypoint-record edit
/// at a time on an otherwise untouched map, so the oracle can say which edit breaks the
/// waypoint (the straight map's checkpoints registered nothing, 2026-09-14).
pub fn wpprobe(args: &[String]) {
    let src = PathBuf::from(args.get(2).expect("wpprobe SOURCE --out F --what flags|rename|move"));
    let out = PathBuf::from(cli::flag(args, "--out").expect("--out F"));
    let what = cli::flag(args, "--what").expect("--what flags|rename|move|dir").to_string();
    let mut m = MapFile::load(&src);
    for b in m.blocks.clone() {
        if b.waypoint_tag.is_none() {
            continue;
        }
        match what.as_str() {
            "flags" => { let f = (b.flags & !0x0060_0000) | 0x0010_0000; m.set_block_flags(b.index, f); println!("  {} flags {:08X} -> {f:08X}", b.name, b.flags); }
            "rename" => { m.set_block_name(b.index, &b.name); println!("  {} renamed to itself", b.name); }
            "move" => { m.move_block_cell(b.index, b.coords()); println!("  {} moved to its own cell {:?}", b.name, b.coords()); }
            "dir" => { m.set_block_dir(b.index, b.dir); println!("  {} dir rewritten {}", b.name, b.dir); }
            _ => panic!("--what flags|rename|move|dir"),
        }
    }
    m.write_to(&out).expect("write");
    println!("wrote {}", out.display());
}
