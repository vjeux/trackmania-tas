//! `tinyctl floorfinish-draw MAP --pak F:KEY --out F.png [--px 40] [--zoom "cx0,cz0,cx1,cz1" --zoom-out F.png]`
//! — Hugo's Floor Finish v3 on a Stadium map, top-down (2026-10-23 14:50): the main grass plane's row strips in
//! green, every ground block's hidden cells light grey with the block outline, the VOID squares inside them
//! dark, the blocks' structure (walls, decks, plates) as dark outlines, every CUTOUT placement in its kind's
//! colour with a number at its centroid, the cell grid faint, the start marked, and a legend
//! (kind → colour → placements → m²). The picture is drawn from the generator's own data (`mapgeom::floor_finish`),
//! so what it shows is what the items carry.
use crate::flagdraw::{Canvas, INK};
use std::collections::BTreeMap;

const PLANE: [u8; 3] = [110, 190, 90];
const HIDDEN: [u8; 3] = [214, 214, 214];
const VOID: [u8; 3] = [70, 70, 80];
const STRUCT: [u8; 3] = [40, 40, 40];
const GRID: [u8; 3] = [200, 200, 200];
const KINDS: [[u8; 3]; 12] = [
    [230, 60, 60],
    [60, 100, 230],
    [240, 150, 30],
    [160, 60, 200],
    [20, 170, 170],
    [220, 40, 160],
    [120, 80, 30],
    [250, 210, 40],
    [30, 120, 60],
    [200, 120, 120],
    [90, 90, 220],
    [240, 100, 0],
];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

/// A block-frame (dir 0) point → world xz of a placement at `cell` (the file cell = the min corner of the turned
/// footprint) with `dir`, the footprint `size` (metres): the pivot sits at the min corner + (0,0) (SZ,0) (SX,SZ)
/// (0,SX) for dir 0..3 — `place::grid_block`'s rule.
fn turn_pt(cell: (i32, i32, i32), dir: u8, size: (f32, f32), x: f32, z: f32) -> (f32, f32) {
    let (bx, bz) = (32.0 * cell.0 as f32, 32.0 * cell.2 as f32);
    let (sx, sz) = size;
    match dir & 3 {
        0 => (bx + x, bz + z),
        1 => (bx + sz - z, bz + x),
        2 => (bx + sx - x, bz + sz - z),
        _ => (bx + z, bz + sx - x),
    }
}

struct Scene {
    px: f32,
    ox: f32,
    oz: f32,
}

impl Scene {
    fn p(&self, x: f32, z: f32) -> (f32, f32) {
        ((x - self.ox) * self.px / 32.0, (z - self.oz) * self.px / 32.0)
    }
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let map = rest.first().filter(|a| !a.starts_with("--")).cloned().ok_or("floorfinish-draw needs a MAP")?;
    let out = f(rest, "--out").unwrap_or_else(|| "floorfinish.png".into());
    let px: f32 = f(rest, "--px").and_then(|v| v.parse().ok()).unwrap_or(40.0);
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--pak" {
            if let Some((p, k)) = rest.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                store.add_pak(p, k)?;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&map))?;
    let collection = m.body_collections().map(|c| c[0].1).unwrap_or(26);
    if collection != 0x1a {
        return vista(&mut store, &m, collection, &out, px, rest);
    }
    let floor = mapgeom::floor_finish::stadium_floor(&mut store, &m)?;
    let plane = mapgeom::floor_finish::stadium_plane_mesh(&mut store, &m, 0.0, 1.0)?;
    let (mut nx, mut nz) = (0, 0);
    for &(x, z) in &floor.grass {
        nx = nx.max(x + 1);
        nz = nz.max(z + 1);
    }
    // the ground blocks and their cutouts
    let ground: Vec<&tmmaps::map::BlockRec> = m
        .blocks
        .iter()
        .filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none() && b.flags & mapgeom::blockmap::FLAG_GROUND != 0 && !b.name.eq_ignore_ascii_case("Grass"))
        .collect();
    // kind stem -> (name, flags, cutout)
    let mut cutouts: BTreeMap<String, (String, u32, Option<mapgeom::floor_finish::Cutout>)> = BTreeMap::new();
    for b in &ground {
        let kflags = b.flags & mapgeom::floor_finish::VARIANT_FLAG_MASK;
        let var = mapgeom::floor_finish::variant_byte(kflags);
        let stem = if var == 0 { format!("FloorFinish{}", b.name) } else { format!("FloorFinish{}V{var}", b.name) };
        if !cutouts.contains_key(&stem) {
            let c = mapgeom::floor_finish::cutout_for_block(&mut store, &b.name, kflags, 0.5, 0.0, 1.0).unwrap_or(None);
            cutouts.insert(stem, (b.name.clone(), kflags, c));
        }
    }
    // every ground block's base-row footprint cells (turned), for the outlines
    let mut footprints: Vec<Vec<(i32, i32)>> = Vec::new();
    let mut sizes: Vec<(f32, f32)> = Vec::new();
    for b in &ground {
        let units = mapgeom::floor_finish::block_units(&mut store, &b.name, b.flags);
        let size = (units.iter().map(|u| u[0]).max().unwrap_or(0) + 1, units.iter().map(|u| u[2]).max().unwrap_or(0) + 1);
        footprints.push(units.iter().filter(|u| u[1] == 0).map(|u| mapgeom::floor_finish::turn_unit(b.coords(), b.dir, *u, size)).collect());
        sizes.push((32.0 * size.0 as f32, 32.0 * size.1 as f32));
    }
    let kinds: Vec<String> = cutouts.iter().filter(|(_, (_, _, c))| c.is_some()).map(|(k, _)| k.clone()).collect();
    let colour_of = |stem: &str| -> [u8; 3] { KINDS[kinds.iter().position(|k| k == stem).unwrap_or(0) % KINDS.len()] };

    let draw = |sc: &Scene, cv: &mut Canvas, w: usize, h: usize, label_scale: usize| {
        // the kept grass: the plane's strips
        for c in plane.verts.chunks(8) {
            let (x0, x1, z0, z1) = (c[0][0], c[1][0], c[0][2], c[4][2]);
            let (a, b) = (sc.p(x0, z0), sc.p(x1, z1));
            cv.poly(&[a, (b.0, a.1), b, (a.0, b.1)], PLANE, 1.0);
        }
        // hidden cells: grey, void squares dark, structure squares a shade darker
        for &(cx, cz) in &floor.hidden {
            let (a, b) = (sc.p(32.0 * cx as f32, 32.0 * cz as f32), sc.p(32.0 * (cx + 1) as f32, 32.0 * (cz + 1) as f32));
            cv.poly(&[a, (b.0, a.1), b, (a.0, b.1)], HIDDEN, 1.0);
            let occ = floor.occupied.get(&(cx, cz)).copied().unwrap_or(0);
            let grn = floor.green.get(&(cx, cz)).copied().unwrap_or(0);
            for zi in 0..8 {
                for xi in 0..8 {
                    let bit = 1u64 << (zi * 8 + xi);
                    if occ & bit == 0 && grn & bit == 0 {
                        let (a, b) = (sc.p(32.0 * cx as f32 + 4.0 * xi as f32, 32.0 * cz as f32 + 4.0 * zi as f32), sc.p(32.0 * cx as f32 + 4.0 * (xi + 1) as f32, 32.0 * cz as f32 + 4.0 * (zi + 1) as f32));
                        cv.poly(&[a, (b.0, a.1), b, (a.0, b.1)], VOID, 1.0);
                    }
                }
            }
        }
        // the cell grid
        for x in 0..=nx {
            let (a, b) = (sc.p(32.0 * x as f32, 0.0), sc.p(32.0 * x as f32, 32.0 * nz as f32));
            cv.line(a.0, a.1, b.0, b.1, GRID, 1.0, 0.6);
        }
        for z in 0..=nz {
            let (a, b) = (sc.p(0.0, 32.0 * z as f32), sc.p(32.0 * nx as f32, 32.0 * z as f32));
            cv.line(a.0, a.1, b.0, b.1, GRID, 1.0, 0.6);
        }
        // block outlines (their footprint cells) + the structure's 4 m squares as dark outlines
        for cells in &footprints {
            for cell in cells {
                let (a, c) = (sc.p(32.0 * cell.0 as f32, 32.0 * cell.1 as f32), sc.p(32.0 * (cell.0 + 1) as f32, 32.0 * (cell.1 + 1) as f32));
                cv.outline(&[a, (c.0, a.1), c, (a.0, c.1)], [120, 120, 120], 1.5);
            }
        }
        for (&(cx, cz), &occ) in &floor.occupied {
            if !floor.hidden.contains(&(cx, cz)) {
                continue;
            }
            for zi in 0..8 {
                for xi in 0..8 {
                    if occ & (1u64 << (zi * 8 + xi)) != 0 {
                        let (a, b) = (sc.p(32.0 * cx as f32 + 4.0 * xi as f32, 32.0 * cz as f32 + 4.0 * zi as f32), sc.p(32.0 * cx as f32 + 4.0 * (xi + 1) as f32, 32.0 * cz as f32 + 4.0 * (zi + 1) as f32));
                        cv.poly(&[a, (b.0, a.1), b, (a.0, b.1)], STRUCT, 0.55);
                    }
                }
            }
        }
        // the cutout placements: the kind's boxes turned into the block's frame, filled in the kind's colour
        let mut n_placed = 0usize;
        let mut placements: Vec<(String, (f32, f32))> = Vec::new();
        for (bi, b) in ground.iter().enumerate() {
            let kflags = b.flags & mapgeom::floor_finish::VARIANT_FLAG_MASK;
            let var = mapgeom::floor_finish::variant_byte(kflags);
            let stem = if var == 0 { format!("FloorFinish{}", b.name) } else { format!("FloorFinish{}V{var}", b.name) };
            let Some((_, _, Some(c))) = cutouts.get(&stem) else { continue };
            let col = colour_of(&stem);
            let size = sizes[bi];
            let (mut sx, mut sz, mut n) = (0.0f32, 0.0f32, 0.0f32);
            for bx in c.verts.chunks(8) {
                let (x0, x1, z0, z1) = (bx[0][0], bx[1][0], bx[0][2], bx[4][2]);
                let corners = [turn_pt(b.coords(), b.dir, size, x0, z0), turn_pt(b.coords(), b.dir, size, x1, z0), turn_pt(b.coords(), b.dir, size, x1, z1), turn_pt(b.coords(), b.dir, size, x0, z1)];
                let pts: Vec<(f32, f32)> = corners.iter().map(|&(x, z)| sc.p(x, z)).collect();
                cv.poly(&pts, col, 0.95);
                let area = (x1 - x0) * (z1 - z0);
                let (cxm, czm) = ((corners[0].0 + corners[2].0) / 2.0, (corners[0].1 + corners[2].1) / 2.0);
                sx += cxm * area;
                sz += czm * area;
                n += area;
            }
            n_placed += 1;
            placements.push((stem.clone(), sc.p(sx / n, sz / n)));
        }
        // numbers at the centroids (white box, kind colour)
        for (k, (stem, (x, y))) in placements.iter().enumerate() {
            let s = format!("{}", k + 1);
            let (tw, th) = ((s.chars().count() * 6 + 1) as f32 * label_scale as f32, 9.0 * label_scale as f32);
            cv.text(x - tw / 2.0, y - th / 2.0, &s, label_scale, colour_of(stem));
        }
        // the start
        if let Some(st) = m.blocks.iter().find(|b| b.waypoint_tag.as_deref() == Some("Spawn") || b.name.contains("Start")) {
            let c = st.coords();
            let (a, b) = (sc.p(32.0 * c.0 as f32 + 4.0, 32.0 * c.2 as f32 + 4.0), sc.p(32.0 * c.0 as f32 + 28.0, 32.0 * c.2 as f32 + 28.0));
            cv.outline(&[a, (b.0, a.1), b, (a.0, b.1)], [0, 0, 0], 2.0);
            cv.text(a.0 + 2.0, a.1 + 2.0, "START", label_scale.max(1), INK);
        }
        let _ = (w, h);
        (n_placed, placements)
    };

    // the full map
    let (w, h) = ((nx as f32 * px) as usize, (nz as f32 * px) as usize);
    let legend_w = 900usize;
    let mut cv = Canvas::new(w + legend_w, h.max(900));
    let sc = Scene { px, ox: 0.0, oz: 0.0 };
    let (n_placed, placements) = draw(&sc, &mut cv, w, h, 2);
    // legend
    let lx = w as f32 + 20.0;
    let mut ly = 20.0;
    cv.text(lx, ly, "Floor Finish v3 - Fall 2026 - 05 (Stadium)", 2, INK);
    ly += 26.0;
    cv.text(lx, ly, &format!("main plane: {} kept grass cells, {} strips", plane.cells, plane.strips), 2, INK);
    ly += 24.0;
    cv.text(lx, ly, &format!("hidden cells (ground blocks): {}; cutouts: {} placements, {} kinds", floor.hidden.len(), n_placed, kinds.len()), 2, INK);
    ly += 34.0;
    for (name, col) in [("kept grass = the main plane", PLANE), ("hidden cell (grass tile gone)", HIDDEN), ("void inside it (no collision, no slab)", VOID), ("structure (walls / decks / plates)", STRUCT)] {
        cv.poly(&[(lx, ly), (lx + 24.0, ly), (lx + 24.0, ly + 18.0), (lx, ly + 18.0)], col, 1.0);
        cv.text(lx + 32.0, ly, name, 2, INK);
        ly += 26.0;
    }
    ly += 14.0;
    cv.text(lx, ly, "cutout kinds (number = placement):", 2, INK);
    ly += 28.0;
    for stem in &kinds {
        let (name, _, c) = &cutouts[stem];
        let c = c.as_ref().unwrap();
        let n = placements.iter().filter(|(s, _)| s == stem).count();
        let nums: Vec<String> = placements.iter().enumerate().filter(|(_, (s, _))| s == stem).map(|(k, _)| format!("{}", k + 1)).collect();
        cv.poly(&[(lx, ly), (lx + 24.0, ly), (lx + 24.0, ly + 18.0), (lx, ly + 18.0)], colour_of(stem), 1.0);
        cv.text(lx + 32.0, ly, &format!("{name} x{n}", ), 2, INK);
        ly += 22.0;
        cv.text(lx + 32.0, ly, &format!("placements {}  -  {:.0} m2 each", nums.join(", "), c.area), 2, [90, 90, 90]);
        ly += 30.0;
    }
    ly += 10.0;
    for line in [
        "slabs 8.0..9.0 (grass +1 m)",
        "cutout = the block FCBGround grass fill at 1 m,",
        "0.5 m inset from structure, in the block frame",
        "main plane = whole kept cells only; void never",
    ] {
        cv.text(lx, ly, line, 2, [90, 90, 90]);
        ly += 24.0;
    }
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {}x{} px, {} px/cell; plane {} strips; {} hidden cells; {} cutout placements of {} kinds", w + legend_w, h.max(900), px, plane.strips, floor.hidden.len(), n_placed, kinds.len());

    // the zoom crop
    if let (Some(z), Some(zo)) = (f(rest, "--zoom"), f(rest, "--zoom-out")) {
        let v: Vec<i32> = z.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if v.len() == 4 {
            let zpx = px * 2.0;
            let (cw, ch) = (((v[2] - v[0]) as f32 * zpx) as usize, ((v[3] - v[1]) as f32 * zpx) as usize);
            let mut cz = Canvas::new(cw, ch);
            let scz = Scene { px: zpx, ox: 32.0 * v[0] as f32, oz: 32.0 * v[1] as f32 };
            let (n2, _) = draw(&scz, &mut cz, cw, ch, 3);
            std::fs::write(&zo, crate::png::encode(&cz.img)).map_err(|e| format!("{zo}: {e}"))?;
            println!("wrote {zo}: {cw}x{ch} px (2x), cells {}..{} x {}..{}, {n2} cutouts in view or not", v[0], v[2], v[1], v[3]);
        }
    }
    Ok(())
}

const OPEN_WATER: [u8; 3] = [70, 130, 220];
const SHORE_DEEP: [u8; 3] = [40, 200, 230];
const SHORE_SHALLOW: [u8; 3] = [190, 215, 235];
const LAND: [u8; 3] = [236, 232, 220];
const FINISH: [u8; 3] = [220, 40, 40];

/// The vista branch (Hugo 2026-10-24): open-water cells blue, the shore tiles' water squares pale blue with
/// the DEEP ones (finish) bright cyan, every finish box outlined red, the grid, the start, a legend; `--zoom`
/// as for Stadium.
fn vista(store: &mut mapgeom::store::DataStore, m: &tmmaps::map::MapFile, collection: u32, out: &str, px: f32, rest: &[String]) -> Result<(), String> {
    let (tiles, bed) = mapgeom::floor_finish::water_tiles(collection);
    let (below, above) = mapgeom::floor_finish::vista_slab(collection);
    let mesh = mapgeom::floor_finish::floor_mesh(store, m, collection, below, above, true)?;
    let shore = mapgeom::floor_finish::shore_tiles(collection);
    let (mut nx, mut nz) = (0, 0);
    for b in m.blocks.iter().chain(m.baked.iter()).filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
        let c = b.coords();
        nx = nx.max(c.0 + 1);
        nz = nz.max(c.2 + 1);
    }
    // open-water cells (authored + baked), shore placements with their water / deep masks (turned)
    let mut open: Vec<(i32, i32)> = Vec::new();
    let mut shore_sq: Vec<((i32, i32), u64, u64, u8)> = Vec::new(); // cell, water mask, deep mask, dir
    let mut cache: BTreeMap<(String, u32), Option<(u64, u64)>> = BTreeMap::new();
    let mut kinds: BTreeMap<String, (usize, u32, u32)> = BTreeMap::new();
    for b in m.blocks.iter().chain(m.baked.iter()).filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
        let c = b.coords();
        if tiles.iter().any(|t| t.eq_ignore_ascii_case(&b.name)) {
            open.push((c.0, c.2));
        } else if shore.iter().any(|t| t.eq_ignore_ascii_case(&b.name)) {
            let key = (b.name.clone(), b.flags);
            let e = cache.entry(key).or_insert_with(|| {
                mapgeom::floor_finish::water_rect(store, collection, &b.name, b.flags).ok().flatten().map(|w| (mapgeom::floor_finish::footprint_mask(&w), mapgeom::floor_finish::deep_mask(&w, bed)))
            });
            if let Some((wm, dm)) = *e {
                shore_sq.push(((c.0, c.2), wm, dm, b.dir));
                let k = kinds.entry(b.name.clone()).or_default();
                k.0 += 1;
                k.1 += wm.count_ones();
                k.2 += dm.count_ones();
            }
        }
    }
    let legend_w = 820usize;
    let (w, h) = ((nx as f32 * px) as usize, (nz as f32 * px) as usize);
    let draw = |sc: &Scene, cv: &mut Canvas, label_scale: usize| {
        let (a, b) = (sc.p(0.0, 0.0), sc.p(32.0 * nx as f32, 32.0 * nz as f32));
        cv.poly(&[a, (b.0, a.1), b, (a.0, b.1)], LAND, 1.0);
        for &(cx, cz) in &open {
            let (a, b) = (sc.p(32.0 * cx as f32, 32.0 * cz as f32), sc.p(32.0 * (cx + 1) as f32, 32.0 * (cz + 1) as f32));
            cv.poly(&[a, (b.0, a.1), b, (a.0, b.1)], OPEN_WATER, 1.0);
        }
        for &((cx, cz), wm, dm, dir) in &shore_sq {
            let wm_t = mapgeom::floor_finish::turn_mask(wm, dir);
            let dm_t = mapgeom::floor_finish::turn_mask(dm, dir);
            for zi in 0..8 {
                for xi in 0..8 {
                    let bit = 1u64 << (zi * 8 + xi);
                    if wm_t & bit == 0 { continue; }
                    let col = if dm_t & bit != 0 { SHORE_DEEP } else { SHORE_SHALLOW };
                    let (a, b) = (sc.p(32.0 * cx as f32 + 4.0 * xi as f32, 32.0 * cz as f32 + 4.0 * zi as f32), sc.p(32.0 * cx as f32 + 4.0 * (xi + 1) as f32, 32.0 * cz as f32 + 4.0 * (zi + 1) as f32));
                    cv.poly(&[a, (b.0, a.1), b, (a.0, b.1)], col, 1.0);
                }
            }
        }
        for x in 0..=nx {
            let (a, b) = (sc.p(32.0 * x as f32, 0.0), sc.p(32.0 * x as f32, 32.0 * nz as f32));
            cv.line(a.0, a.1, b.0, b.1, GRID, 1.0, 0.5);
        }
        for z in 0..=nz {
            let (a, b) = (sc.p(0.0, 32.0 * z as f32), sc.p(32.0 * nx as f32, 32.0 * z as f32));
            cv.line(a.0, a.1, b.0, b.1, GRID, 1.0, 0.5);
        }
        for &(x0, x1, z0, z1, _) in &mesh.quads {
            let (a, b) = (sc.p(x0, z0), sc.p(x1, z1));
            cv.outline(&[a, (b.0, a.1), b, (a.0, b.1)], FINISH, 1.5);
        }
        if let Some(st) = m.blocks.iter().find(|b| b.waypoint_tag.as_deref() == Some("Spawn") || b.name.contains("Start")) {
            let c = st.coords();
            let (a, b) = (sc.p(32.0 * c.0 as f32 + 4.0, 32.0 * c.2 as f32 + 4.0), sc.p(32.0 * c.0 as f32 + 28.0, 32.0 * c.2 as f32 + 28.0));
            cv.outline(&[a, (b.0, a.1), b, (a.0, b.1)], [0, 0, 0], 2.0);
            cv.text(a.0 + 2.0, a.1 + 2.0, "START", label_scale.max(1), INK);
        }
    };
    let mut cv = Canvas::new(w + legend_w, h.max(900));
    let sc = Scene { px, ox: 0.0, oz: 0.0 };
    draw(&sc, &mut cv, 2);
    let lx = w as f32 + 20.0;
    let mut ly = 20.0;
    let coll_name = match collection { 0x10 => "RedIsland", 0x1d => "WhiteShore", 0x1c => "BlueBay", 0xf => "GreenCoast", _ => "?" };
    cv.text(lx, ly, &format!("Floor Finish v4 - {coll_name}"), 2, INK);
    ly += 30.0;
    cv.text(lx, ly, &format!("open-water cells {} -> {} strips; shore deep boxes {}", mesh.cells, mesh.strips, mesh.shore_prisms), 2, INK);
    ly += 24.0;
    cv.text(lx, ly, &format!("slab: bed -{below:.1} .. bed +{above:.1} (top = surface - 2.5)", ), 2, INK);
    ly += 34.0;
    for (name, col) in [("open water (full cell, bed depth)", OPEN_WATER), ("shore tile water at seabed depth = finish", SHORE_DEEP), ("shore tile water, shallow / sloping (no finish)", SHORE_SHALLOW), ("land", LAND)] {
        cv.poly(&[(lx, ly), (lx + 24.0, ly), (lx + 24.0, ly + 18.0), (lx, ly + 18.0)], col, 1.0);
        cv.text(lx + 32.0, ly, name, 2, INK);
        ly += 26.0;
    }
    cv.outline(&[(lx, ly), (lx + 24.0, ly), (lx + 24.0, ly + 18.0), (lx, ly + 18.0)], FINISH, 1.5);
    cv.text(lx + 32.0, ly, "finish box (one item, all boxes)", 2, INK);
    ly += 40.0;
    cv.text(lx, ly, "shore kinds: placements / water sq / deep sq", 2, INK);
    ly += 26.0;
    for (name, (n, wsq, dsq)) in &kinds {
        cv.text(lx, ly, &format!("{name}: {n} / {wsq} / {dsq}"), 2, [90, 90, 90]);
        ly += 22.0;
    }
    std::fs::write(out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {}x{} px; open {} cells / {} strips; shore deep boxes {}; shore kinds {}", w + legend_w, h.max(900), mesh.cells, mesh.strips, mesh.shore_prisms, kinds.len());
    if let (Some(z), Some(zo)) = (f(rest, "--zoom"), f(rest, "--zoom-out")) {
        let v: Vec<i32> = z.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if v.len() == 4 {
            let zpx = px * 2.0;
            let (cw, ch) = (((v[2] - v[0]) as f32 * zpx) as usize, ((v[3] - v[1]) as f32 * zpx) as usize);
            let mut cz = Canvas::new(cw, ch);
            let scz = Scene { px: zpx, ox: 32.0 * v[0] as f32, oz: 32.0 * v[1] as f32 };
            draw(&scz, &mut cz, 3);
            std::fs::write(&zo, crate::png::encode(&cz.img)).map_err(|e| format!("{zo}: {e}"))?;
            println!("wrote {zo}: {cw}x{ch} px (2x), cells {}..{} x {}..{}", v[0], v[2], v[1], v[3]);
        }
    }
    Ok(())
}
