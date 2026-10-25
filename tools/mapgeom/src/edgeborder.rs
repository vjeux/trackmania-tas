//! `mapgeom edge-border MAP.Map.Gbx --items A,B,… --records records.tsv --out-dir D
//! [--width 0.5] [--lift 0.015] [--probe 0.6] [--material LINK] [--ident-base ACBORDER]`
//!
//! A BORDER DECAL along the open edges of converted blocks (Wouter via Everios96,
//! 2026-10-24, Icy 78: "create a border at the edge of the black blocks, so it's easier
//! to see when you jump … similar to how platform border textures work, in this case
//! red"). The game draws a platform's red rim from a generated edge piece
//! (`PlatformFCSmall`, material `Modifier\Platform<S>\DecalPlatform`) that a black block
//! never gets; a converted black block's cap is a bare sheet.
//!
//! Method: the listed items' COLLISION triangles (the drivable caps) are taken in world
//! space from every placement; an edge belonging to exactly one triangle is a rim of the
//! union; a rim whose outward probe point lands in a cell another block of the map still
//! occupies (the vanilla platform next to the slope: its own rim is there already) is
//! dropped; every other rim gets a visual-only quad `width` metres wide, inside the edge,
//! `lift` above the surface, dressed with the border decal (texture 128×4096: U across the
//! strip, V along it, one repeat per 32 widths). One item per source placement, geometry
//! relative to that placement's position, so `tmmaps add-items` places it at the same
//! spot with yaw 0.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use crate::static_item::build::{assemble, BuildOpts};
use crate::static_item::merged::Merged;

/// A welding key: 1 mm in x/z, 2 cm in y — the assembler hands a cap's hull back twice, the
/// second copy 4 mm up (the deck shell), and both must read as one surface.
fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0] * 100.0).round() as i32, (p[1] * 50.0).round() as i32, (p[2] * 100.0).round() as i32)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The map cell of a world point (Stadium: 32 × 8 × 32 m; cell y 8 is the ground).
fn cell_of(p: [f32; 3]) -> [i32; 3] {
    [(p[0] / 32.0).floor() as i32, (p[1] / 8.0).floor() as i32 + 8, (p[2] / 32.0).floor() as i32]
}

pub fn run(rest: &[String], open: &mut dyn FnMut() -> crate::store::DataStore) -> Result<(), String> {
    let flag = |k: &str| tmmaps::cli::flag(rest, k).map(String::from);
    let map_path = rest.get(1).filter(|a| !a.starts_with("--")).cloned().ok_or("edge-border MAP.Map.Gbx --items A,B --records R.tsv --out-dir D")?;
    let items: HashSet<String> = flag("--items").ok_or("--items A,B,… (item file names, with or without .Item.Gbx)")?.split(',').map(|s| { let s = s.trim(); if s.ends_with(".Item.Gbx") { s.to_string() } else { format!("{s}.Item.Gbx") } }).collect();
    let records = flag("--records").ok_or("--records records.tsv (the build's authored-block table: its `cells` column is the occupancy)")?;
    let out_dir = PathBuf::from(flag("--out-dir").ok_or("--out-dir D")?);
    let width: f32 = flag("--width").and_then(|v| v.parse().ok()).unwrap_or(0.5);
    let lift: f32 = flag("--lift").and_then(|v| v.parse().ok()).unwrap_or(0.015);
    let probe: f32 = flag("--probe").and_then(|v| v.parse().ok()).unwrap_or(0.6);
    let link = flag("--material").unwrap_or_else(|| "Stadium\\Media\\Modifier\\PlatformIce\\DecalPlatform".into());
    let ident_base = flag("--ident-base").unwrap_or_else(|| "ACBORDER".into());
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;

    // occupancy: every cell an authored block of the ORIGINAL still owns in the live map
    // (the converted ones — baked, deleted — are the items themselves and do not count)
    let mut occupied: HashSet<[i32; 3]> = HashSet::new();
    let text = std::fs::read_to_string(&records).map_err(|e| format!("{records}: {e}"))?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |n: &str| header.iter().position(|h| *h == n);
    let (ci_action, ci_cells, ci_name) = (col("action").ok_or("records.tsv: no `action` column")?, col("cells").ok_or("records.tsv: no `cells` column")?, col("name").ok_or("records.tsv: no `name` column")?);
    let mut kept = 0usize;
    for l in lines {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() <= ci_cells.max(ci_action) {
            continue;
        }
        // a baked block is gone from the live map; a pillar stays but is below the caps
        if c[ci_action] == "bake" || c[ci_name].ends_with("Pillar") {
            continue;
        }
        for cell in c[ci_cells].split(';') {
            let v: Vec<i32> = cell.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() == 3 {
                occupied.insert([v[0], v[1], v[2]]);
                kept += 1;
            }
        }
    }
    println!("occupancy: {} cell(s) from {} block rows that stay", occupied.len(), kept);

    let m = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
    let mut store = open();
    let mut asm = crate::assemble::Assembler::new(&mut store);
    asm.with_embedded(&m)?;
    // the whole map's drivable geometry (blocks at the Stadium grid height, items at their
    // positions), bucketed by 32 m cell in x/z: a rim's outward probe asks whether a surface
    // continues within a metre of the lip's height — the cell test above said "a block is
    // there" for a hill whose top sits 10 m below a lip (Icy 78, Everios96 2026-10-24: "there
    // is a jump down where the arrows are")
    let world = asm.map(&m, -64.0, true);
    let mut buckets: HashMap<(i32, i32), Vec<[[f32; 3]; 3]>> = HashMap::new();
    let mut n_world = 0usize;
    for g in world.groups.values() {
        for t in &g.tris {
            let w = [g.verts[t[0] as usize], g.verts[t[1] as usize], g.verts[t[2] as usize]];
            let (x0, x1) = (w.iter().map(|p| p[0]).fold(f32::MAX, f32::min), w.iter().map(|p| p[0]).fold(f32::MIN, f32::max));
            let (z0, z1) = (w.iter().map(|p| p[2]).fold(f32::MAX, f32::min), w.iter().map(|p| p[2]).fold(f32::MIN, f32::max));
            for cx in (x0 / 32.0).floor() as i32..=(x1 / 32.0).floor() as i32 {
                for cz in (z0 / 32.0).floor() as i32..=(z1 / 32.0).floor() as i32 {
                    buckets.entry((cx, cz)).or_default().push(w);
                }
            }
            n_world += 1;
        }
    }
    println!("world: {n_world} triangle(s) in {} cells", buckets.len());
    // the highest surface under (x, z) within [y_lo, y_hi]
    let surface_near = |x: f32, z: f32, y_lo: f32, y_hi: f32| -> Option<f32> {
        let mut best: Option<f32> = None;
        if let Some(list) = buckets.get(&((x / 32.0).floor() as i32, (z / 32.0).floor() as i32)) {
            for t in list {
                // 2D point-in-triangle (x, z)
                let (ax, az, bx, bz, cx, cz) = (t[0][0], t[0][2], t[1][0], t[1][2], t[2][0], t[2][2]);
                let d1 = (x - bx) * (az - bz) - (ax - bx) * (z - bz);
                let d2 = (x - cx) * (bz - cz) - (bx - cx) * (z - cz);
                let d3 = (x - ax) * (cz - az) - (cx - ax) * (z - az);
                let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                if has_neg && has_pos {
                    continue;
                }
                // barycentric height
                let det = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz);
                if det.abs() < 1e-6 {
                    continue;
                }
                let l1 = ((bz - cz) * (x - cx) + (cx - bx) * (z - cz)) / det;
                let l2 = ((cz - az) * (x - cx) + (ax - cx) * (z - cz)) / det;
                let l3 = 1.0 - l1 - l2;
                let y = l1 * t[0][1] + l2 * t[1][1] + l3 * t[2][1];
                if y >= y_lo && y <= y_hi && best.map(|b| y > b).unwrap_or(true) {
                    best = Some(y);
                }
            }
        }
        best
    };

    // world triangles of every selected placement's collision hull, tagged by placement
    struct Cap {
        placement: usize,
        pos: [f32; 3],
        color: u8,
        tris: Vec<[[f32; 3]; 3]>,
    }
    // the placement colour byte: the ice platform's own rim decal is red on this map because
    // Everios96 painted the blocks red (the `DecalPlatform` hue follows the placement colour)
    let colors = m.colors();
    let mut caps: Vec<Cap> = Vec::new();
    for (i, it) in m.items.iter().enumerate() {
        if !items.contains(&it.model) {
            continue;
        }
        let Some(lm) = asm.item_model(&it.model) else {
            println!("placement {i} {}: no model", it.model);
            continue;
        };
        let lm = lm.clone();
        let xf = crate::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
        let mut tris = Vec::new();
        if std::env::var("MAPGEOM_BORDER_DEBUG").is_ok() {
            println!("  placement {i} {}: groups {:?}", it.model, lm.scene.groups.iter().map(|(k, g)| format!("{k}:{}", g.tris.len())).collect::<Vec<_>>());
        }
        // one copy of each triangle (a hull drawn twice — both windings — would hide every rim)
        let mut seen: HashSet<[(i32, i32, i32); 3]> = HashSet::new();
        for (mat, g) in &lm.scene.groups {
            if mat == "Visual" {
                continue;
            }
            for t in &g.tris {
                let w = [crate::geom::apply(&xf, g.verts[t[0] as usize]), crate::geom::apply(&xf, g.verts[t[1] as usize]), crate::geom::apply(&xf, g.verts[t[2] as usize])];
                let mut k = [key(&w[0]), key(&w[1]), key(&w[2])];
                k.sort();
                if seen.insert(k) {
                    tris.push(w);
                }
            }
        }
        if std::env::var("MAPGEOM_BORDER_VERTS").is_ok() {
            let mut vs: Vec<(i32, i32, i32)> = tris.iter().flat_map(|t| t.iter().map(|p| ((p[0] * 10.0).round() as i32, (p[1] * 10.0).round() as i32, (p[2] * 10.0).round() as i32))).collect();
            vs.sort();
            vs.dedup();
            println!("    hull vertices (dm): {:?}", vs);
        }
        if std::env::var("MAPGEOM_BORDER_DEBUG").is_ok() {
            let mut ys: Vec<i32> = tris.iter().flat_map(|t| t.iter().map(|p| (p[1] * 1000.0).round() as i32)).collect();
            ys.sort();
            ys.dedup();
            println!("    {} unique triangle(s); y levels (mm): {:?}", tris.len(), &ys[..ys.len().min(12)]);
        }
        if !tris.is_empty() {
            caps.push(Cap { placement: i, pos: it.pos, color: colors.as_ref().map(|c| c.item(i)).unwrap_or(0), tris });
        }
    }
    println!("{} placement(s) of {} model(s) with hull triangles", caps.len(), items.len());
    if caps.is_empty() {
        return Err("nothing to border".into());
    }

    // edge → (count, the one triangle's third vertex + normal, the cap index)
    let mut edges: HashMap<((i32, i32, i32), (i32, i32, i32)), (usize, [f32; 3], [f32; 3], [f32; 3], [f32; 3], usize)> = HashMap::new();
    for (ci, cap) in caps.iter().enumerate() {
        for t in &cap.tris {
            let mut n = norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
            // a hull's winding is whatever the baker left (the ground slopes' caps face down):
            // the face is a cap when it is near-horizontal either way, and the normal we keep
            // points up
            if n[1].abs() < 0.3 {
                continue; // a wall or a steep side is not a driving cap
            }
            if n[1] < 0.0 {
                n = [-n[0], -n[1], -n[2]];
            }
            for k in 0..3 {
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                let (ka, kb) = (key(&a), key(&b));
                let e = if ka <= kb { (ka, kb) } else { (kb, ka) };
                let entry = edges.entry(e).or_insert((0, a, b, c, n, ci));
                entry.0 += 1;
            }
        }
    }
    let mut rims: BTreeMap<usize, Vec<([f32; 3], [f32; 3], [f32; 3], [f32; 3])>> = BTreeMap::new();
    let mut seen_rims: HashSet<((i32, i32, i32), (i32, i32, i32))> = HashSet::new();
    let (mut total, mut against_block, mut kept_rims) = (0usize, 0usize, 0usize);
    if std::env::var("MAPGEOM_BORDER_DEBUG").is_ok() {
        for (ci, cap) in caps.iter().enumerate() {
            println!("  cap {ci} = placement {} at ({:.0},{:.0},{:.0}) colour {}, {} tris", cap.placement, cap.pos[0], cap.pos[1], cap.pos[2], cap.color, cap.tris.len());
        }
    }
    for (_, (count, a, b, c, n, ci)) in edges {
        if count != 1 {
            if std::env::var("MAPGEOM_BORDER_DEBUG").is_ok() && count > 2 {
                println!("  edge ×{count} cap {ci} ({:.0},{:.1},{:.0})-({:.0},{:.1},{:.0})", a[0], a[1], a[2], b[0], b[1], b[2]);
            }
            continue;
        }
        total += 1;
        // outward: perpendicular to the edge within the surface plane, away from the third vertex
        let e = norm(sub(b, a));
        let mut out = norm(cross(e, n));
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5];
        if dot(out, sub(c, mid)) > 0.0 {
            out = [-out[0], -out[1], -out[2]];
        }
        // a rim where the ground goes on at the lip's height (another block's deck, a hill
        // flush with it): that neighbour draws its own edge, or there is no edge to see.
        // A surface more than a metre below is a jump.
        let q = [mid[0] + out[0] * probe, mid[1], mid[2] + out[2] * probe];
        let cont = surface_near(q[0], q[2], mid[1] - 1.0, mid[1] + 1.5);
        if let Some(y) = cont {
            against_block += 1;
            if std::env::var("MAPGEOM_BORDER_DEBUG").is_ok() {
                println!("  continues cap {ci} ({:.0},{:.1},{:.0})-({:.0},{:.1},{:.0}) surface at {y:.2} (cell {:?})", a[0], a[1], a[2], b[0], b[1], b[2], cell_of(q));
            }
            continue;
        }
        let _ = &occupied;
        // one strip per place: a finely tessellated cap hands the same rim twice (two
        // triangles whose shared edge did not weld), and two coplanar strips flicker
        let ek = { let (ka, kb) = (key(&a), key(&b)); if ka <= kb { (ka, kb) } else { (kb, ka) } };
        if !seen_rims.insert(ek) {
            continue;
        }
        kept_rims += 1;
        if std::env::var("MAPGEOM_BORDER_DEBUG").is_ok() {
            println!("  rim cap {ci} ({:.0},{:.1},{:.0})-({:.0},{:.1},{:.0}) out ({:.2},{:.2},{:.2}) probe cell {:?}", a[0], a[1], a[2], b[0], b[1], b[2], out[0], out[1], out[2], cell_of(q));
        }
        rims.entry(ci).or_default().push((a, b, out, n));
    }
    println!("rim edges: {total} on the union, {against_block} against another block, {kept_rims} bordered");

    let mut placements = String::new();
    let mut n_items = 0usize;
    for (ci, list) in rims {
        let cap = &caps[ci];
        let mut merged = Merged::default();
        let mut length = 0.0f32;
        for (a, b, out, n) in &list {
            // the strip inside the edge: a, b on the rim; a', b' `width` inward, all lifted
            let inward = [-out[0], -out[1], -out[2]];
            let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            length += l;
            let lift_v = [n[0] * lift, n[1] * lift, n[2] * lift];
            let rel = |p: [f32; 3]| [p[0] - cap.pos[0] + lift_v[0], p[1] - cap.pos[1] + lift_v[1], p[2] - cap.pos[2] + lift_v[2]];
            let (pa, pb) = (rel(*a), rel(*b));
            let (pa2, pb2) = (rel([a[0] + inward[0] * width, a[1] + inward[1] * width, a[2] + inward[2] * width]), rel([b[0] + inward[0] * width, b[1] + inward[1] * width, b[2] + inward[2] * width]));
            // texture: 128 px across (U 0 at the outer edge .. 1 inside), 4096 along (one repeat per 32 widths)
            let v1 = l / (width * 32.0);
            merged.add_decal_quad4([pa, pb, pb2, pa2], [[0.0, 0.0], [0.0, v1], [1.0, v1], [1.0, 0.0]], *n, &link);
            if std::env::var("MAPGEOM_BORDER_HULL").is_ok() {
                merged.add_quad4_hull([pa, pb, pb2, pa2], 74);
            }
        }
        if std::env::var("MAPGEOM_BORDER_FLATQUAD").is_ok() {
            // the proven helper instead of the strips: a flat 2 × 32 m quad at the cap's origin
            merged = Merged::default();
            merged.add_flat_quad(-32.0, 0.0, -30.0, 32.0, 0.05, &link, 74);
        }
        n_items += 1;
        let ident = format!("{ident_base}{:03}.Item.Gbx", n_items);
        let opts = BuildOpts { ident: ident.clone(), author: crate::tiny_library::item_author(&ident), scale: 1.0, collection: 26, skin: None };
        let mut f = assemble(&merged, &opts)?;
        // an embedded item's body must be LZO-compressed ("BUCR"): the library's items are,
        // and an uncompressed one ("BUUR") crashed the client at map load on every variant
        // tried (2026-10-24, six test maps)
        f.body_comp = b'C';
        let path = out_dir.join(&ident);
        std::fs::write(&path, crate::static_item::file::write_file(&f)).map_err(|e| format!("{}: {e}", path.display()))?;
        placements.push_str(&format!("{}\t{:.3},{:.3},{:.3}\t0\t{}\t{}\t{:.1}\n", path.display(), cap.pos[0], cap.pos[1], cap.pos[2], cap.color, list.len(), length));
        println!("placement {} ({}, colour {}): {} rim edge(s), {:.1} m → {ident}", cap.placement, m.items[cap.placement].model, cap.color, list.len(), length);
    }
    let pl = out_dir.join("placements.tsv");
    std::fs::write(&pl, format!("item\tpos\tyaw\tcolor\tedges\tlength_m\n{placements}")).map_err(|e| format!("{}: {e}", pl.display()))?;
    // the add-items spec, ready to paste (position, yaw 0, the cap's colour byte)
    let spec: Vec<String> = placements.lines().map(|l| { let c: Vec<&str> = l.split('\t').collect(); format!("{}@{}@0@{}", c[0], c[1], c[3]) }).collect();
    std::fs::write(out_dir.join("add-items.txt"), spec.join(";")).ok();
    println!("{n_items} border item(s) → {} (+ placements.tsv, add-items.txt)", out_dir.display());
    Ok(())
}
