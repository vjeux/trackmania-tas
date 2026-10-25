//! Floor Finish v4.18 — the VISUAL split into per-tile items (RESEARCH-visual.md): the one waypoint item keeps the
//! trigger (and no visual but its speck); the red slab is drawn by separate NON-waypoint items, one per 128 m tile
//! of the map (+ the ring), each with its own six lightmap charts and PreLightGen. Every trigger body (box or
//! ramp — an 8-corner prism whose top and bottom are bilinear in (x, z)) is cut at the tile borders by bilinear
//! interpolation, so the union of the tiles' bodies is exactly the trigger volume.
use crate::static_item::build::Merged;

pub const TILE_M: f32 = 128.0;
pub type Body = [[f32; 3]; 8];

/// Corner order of a body: 0 (x0,z0) 1 (x1,z0) 2 (x1,z0)↑ 3 (x0,z0)↑ 4 (x0,z1) 5 (x1,z1) 6 (x1,z1)↑ 7 (x0,z1)↑
/// (↑ = top). The y of a corner at (x, z) is bilinear in the four bottom (or top) corners.
fn y_bilinear(b: &Body, top: bool, x: f32, z: f32) -> f32 {
    let (x0, x1, z0, z1) = (b[0][0], b[1][0], b[0][2], b[4][2]);
    let tx = if (x1 - x0).abs() < 1e-9 { 0.0 } else { ((x - x0) / (x1 - x0)).clamp(0.0, 1.0) };
    let tz = if (z1 - z0).abs() < 1e-9 { 0.0 } else { ((z - z0) / (z1 - z0)).clamp(0.0, 1.0) };
    let (a, bb, c, d) = if top { (b[3][1], b[2][1], b[7][1], b[6][1]) } else { (b[0][1], b[1][1], b[4][1], b[5][1]) };
    let y_z0 = a + (bb - a) * tx;
    let y_z1 = c + (d - c) * tx;
    y_z0 + (y_z1 - y_z0) * tz
}

fn sub_body(b: &Body, x0: f32, x1: f32, z0: f32, z1: f32) -> Body {
    let c = |x: f32, z: f32, top: bool| [x, y_bilinear(b, top, x, z), z];
    [c(x0, z0, false), c(x1, z0, false), c(x1, z0, true), c(x0, z0, true), c(x0, z1, false), c(x1, z1, false), c(x1, z1, true), c(x0, z1, true)]
}

/// Cut a body at every tile border it crosses; returns (tile x, tile z, piece).
pub fn tile_body(b: &Body, tile: f32) -> Vec<(i32, i32, Body)> {
    let (x0, x1, z0, z1) = (b[0][0], b[1][0], b[0][2], b[4][2]);
    let mut out = Vec::new();
    let (tx0, tx1) = ((x0 / tile).floor() as i32, ((x1 - 1e-4) / tile).floor() as i32);
    let (tz0, tz1) = ((z0 / tile).floor() as i32, ((z1 - 1e-4) / tile).floor() as i32);
    for tz in tz0..=tz1 {
        for tx in tx0..=tx1 {
            let (cx0, cx1) = (x0.max(tx as f32 * tile), x1.min((tx + 1) as f32 * tile));
            let (cz0, cz1) = (z0.max(tz as f32 * tile), z1.min((tz + 1) as f32 * tile));
            if cx1 - cx0 < 1e-4 || cz1 - cz0 < 1e-4 {
                continue;
            }
            out.push((tx, tz, sub_body(b, cx0, cx1, cz0, cz1)));
        }
    }
    out
}

/// The tiles of a trigger mesh (8-vertex bodies in `verts`): tile → its pieces, sorted by (tz, tx).
pub fn tiles_of(verts: &[[f32; 3]], tile: f32) -> Vec<((i32, i32), Vec<Body>)> {
    let mut map: std::collections::BTreeMap<(i32, i32), Vec<Body>> = Default::default();
    for c in verts.chunks_exact(8) {
        let b: Body = [c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]];
        for (tx, tz, piece) in tile_body(&b, tile) {
            map.entry((tz, tx)).or_default().push(piece);
        }
    }
    map.into_iter().map(|((tz, tx), v)| ((tx, tz), v)).collect()
}

/// Build one visual-only slab item from a tile's bodies.
pub fn build_tile_item(store: &mut crate::store::DataStore, bodies: &[Body], ident: &str, collection: u32, disable_lightmap: bool) -> Result<(Vec<u8>, usize, f32), String> {
    let donor = "Stadium\\Items\\LightCube2m.Item.Gbx";
    let mut mg: Merged = crate::static_item::build::pack_item_merged(store, donor, 1.0, collection, 0, None).map_err(|e| format!("donor {donor}: {e}"))?;
    crate::static_item::build::make_visual_only(&mut mg);
    mg.disable_lightmap = disable_lightmap;
    crate::static_item::build::add_finish_slab_visual(&mut mg, bodies)?;
    let mpu = mg.pre_light_gen.as_ref().map(|p| p.u02).unwrap_or(0.0);
    let (bytes, _) = crate::static_item::build::finish_item(mg, ident, ident, 1.0, collection)?;
    Ok((bytes, 12 * bodies.len(), mpu))
}

/// Volume check: Σ(top − bottom) × area over the pieces equals the same over the bodies (to 1e-3 relative).
pub fn volume(bodies: &[Body]) -> f64 {
    bodies
        .iter()
        .map(|b| {
            let area = ((b[1][0] - b[0][0]) * (b[4][2] - b[0][2])).abs() as f64;
            let h = ((b[3][1] - b[0][1]) + (b[2][1] - b[1][1]) + (b[7][1] - b[4][1]) + (b[6][1] - b[5][1])) as f64 / 4.0;
            area * h
        })
        .sum()
}

// ------------------------------------------------------------------ v4.19: the PureFloor (mesh-modeler crystal) recipe

/// The friend's PureFloor material: CustomGlass, physics 76, TargetColor (0.9434, 0.3585, 0.3585).
pub const PURE_PHYSICS: u8 = 76;
pub const PURE_GLASS_LINK: &str = "Stadium\\Media\\Material_BlockCustom\\CustomGlass";
pub const PURE_GLASS_RGB: [f32; 3] = [0.9434388, 0.3585064, 0.3585064];
pub const PURE_PLASTIC_LINK: &str = "Stadium\\Media\\Material_BlockCustom\\CustomPlastic";
pub const PURE_PLASTIC_RGB: [f32; 3] = [1.0, 0.1, 0.1];
static PURE_GLASS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_pure_glass(on: bool) {
    PURE_GLASS.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub fn pure_glass() -> bool {
    PURE_GLASS.load(std::sync::atomic::Ordering::Relaxed)
}
static PURE_TEMPLATE: std::sync::OnceLock<std::sync::Mutex<Option<String>>> = std::sync::OnceLock::new();
pub fn set_pure_template(path: &str) {
    *PURE_TEMPLATE.get_or_init(|| std::sync::Mutex::new(None)).lock().unwrap() = Some(path.to_string());
}
pub fn pure_template() -> Result<Vec<u8>, String> {
    let p = PURE_TEMPLATE.get_or_init(|| std::sync::Mutex::new(None)).lock().unwrap().clone().unwrap_or_else(|| {
        format!("{}/tm-bank/hugo-floorfinish/v4-indicator/data/items/PureFloor4.Item.Gbx", std::env::var("HOME").unwrap_or_default())
    });
    std::fs::read(&p).map_err(|e| format!("pure template {p}: {e}"))
}

/// A closed slab body as crystal faces (6 quads, wound so (p1−p0)×(p2−p0) points OUT of the body — the friend's
/// up-facing quad (−N,0,0)(−N,0,N)(0,0,N)(0,0,0) has that cross product +y).
pub fn body_faces(mesh: &mut crate::crystal::CrystalMesh, b: &Body, material: u32) {
    let centre = [b.iter().map(|p| p[0]).sum::<f32>() / 8.0, b.iter().map(|p| p[1]).sum::<f32>() / 8.0, b.iter().map(|p| p[2]).sum::<f32>() / 8.0];
    let faces: [[u32; 4]; 6] = [[0, 1, 2, 3], [5, 4, 7, 6], [4, 0, 3, 7], [1, 5, 6, 2], [4, 5, 1, 0], [3, 2, 6, 7]];
    for f in faces {
        let mut idx = f;
        let p: Vec<[f32; 3]> = idx.iter().map(|i| b[*i as usize]).collect();
        let (u, v) = ([p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]], [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let fc = [(p[0][0] + p[2][0]) / 2.0 - centre[0], (p[0][1] + p[2][1]) / 2.0 - centre[1], (p[0][2] + p[2][2]) / 2.0 - centre[2]];
        if n[0] * fc[0] + n[1] * fc[1] + n[2] * fc[2] < 0.0 {
            idx.reverse();
        }
        let uv = |q: [f32; 3]| -> [f32; 2] {
            if n[1].abs() >= n[0].abs() && n[1].abs() >= n[2].abs() { [q[0] / 8.0, q[2] / 8.0] } else if n[0].abs() >= n[2].abs() { [q[2] / 8.0, q[1] / 8.0] } else { [q[0] / 8.0, q[1] / 8.0] }
        };
        // v4.20: every face owns its corners — nothing shared between the caps and the walls, so the game's
        // vertex-normal smoothing cannot tilt a cap's rim (the "waver")
        let base = mesh.positions.len() as u32;
        mesh.positions.extend(idx.iter().map(|i| b[*i as usize]));
        mesh.faces.push(crate::crystal::Face { verts: (0..4).map(|k| base + k).collect(), uvs: idx.iter().map(|i| uv(b[*i as usize])).collect(), material });
    }
}

/// Triangles (a cutout prism) as crystal faces, wound outward by the centroid test.
pub fn tri_faces(mesh: &mut crate::crystal::CrystalMesh, verts: &[[f32; 3]], tris: &[[u32; 3]], material: u32) {
    let n_v = verts.len().max(1) as f32;
    let centre = [verts.iter().map(|p| p[0]).sum::<f32>() / n_v, verts.iter().map(|p| p[1]).sum::<f32>() / n_v, verts.iter().map(|p| p[2]).sum::<f32>() / n_v];
    for t in tris {
        let mut idx = *t;
        let p = [verts[t[0] as usize], verts[t[1] as usize], verts[t[2] as usize]];
        let (u, v) = ([p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]], [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len < 1e-9 {
            continue;
        }
        let fc = [(p[0][0] + p[1][0] + p[2][0]) / 3.0, (p[0][1] + p[1][1] + p[2][1]) / 3.0, (p[0][2] + p[1][2] + p[2][2]) / 3.0];
        // caps by their y side (a concave outline makes the centroid test unreliable for walls: the extrude winds
        // its walls outward already)
        let horizontal = n[1].abs() / len > 0.5;
        if horizontal && (fc[1] - centre[1]) * n[1] < 0.0 {
            idx.swap(1, 2);
        }
        let uv = |q: [f32; 3]| -> [f32; 2] {
            if horizontal { [q[0] / 8.0, q[2] / 8.0] } else if n[0].abs() >= n[2].abs() { [q[2] / 8.0, q[1] / 8.0] } else { [q[0] / 8.0, q[1] / 8.0] }
        };
        let base = mesh.positions.len() as u32;
        mesh.positions.extend(idx.iter().map(|i| verts[*i as usize]));
        mesh.faces.push(crate::crystal::Face { verts: (0..3).map(|k| base + k).collect(), uvs: idx.iter().map(|i| uv(verts[*i as usize])).collect(), material });
    }
}

/// v4.20: the crystal's lightmap table with ONE island per planar group — every up-facing face in one chart
/// (the xz projection, square texels) in [0, 0.5)², every down-facing face in one chart in [0.5, 1) × [0, 0.5),
/// and the walls each their own rectangle (real proportions) in a grid over [0, 1) × [0.5, 1). Our generic
/// `lightmap_atlas` gives EVERY face its own small chart: on a fan-triangulated cap the game bakes each triangle
/// separately and the cap shows the triangles (Hugo's "waver", w1.png); the mesh modeler charts a connected
/// planar region as one island (PureFloor1QuarterCircle: 32 fan faces, one island over the uv square).
pub fn grouped_lightmap(model: &crate::crystal_model::CPlugCrystal) -> crate::crystal_model::Lightmap {
    let mut coords: Vec<[u16; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut faces: Vec<(usize, Vec<[f32; 3]>)> = Vec::new(); // (class, corners)
    for l in &model.layers {
        if let crate::crystal_model::LayerKind::Geometry { crystal, is_visible, .. } = &l.kind {
            if !l.base.is_enabled || !*is_visible {
                continue;
            }
            for f in &crystal.faces {
                let p: Vec<[f32; 3]> = f.verts.iter().map(|i| crystal.positions[*i as usize]).collect();
                let (u, v) = ([p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]], [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]]);
                let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
                let class = if n[1] / len > 0.5 { 0 } else if n[1] / len < -0.5 { 1 } else { 2 };
                faces.push((class, p));
            }
        }
    }
    let q = |x: f64| -> u16 { (x.clamp(0.0, 1.0) * 65535.0) as u16 };
    // caps: one island each
    for class in 0..2 {
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for (c, p) in &faces {
            if *c == class {
                for v in p { lo[0] = lo[0].min(v[0]); lo[1] = lo[1].min(v[2]); hi[0] = hi[0].max(v[0]); hi[1] = hi[1].max(v[2]); }
            }
        }
        let ext = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-3) as f64;
        let s = 0.48 / ext;
        let (ox, oy) = if class == 0 { (0.01, 0.01) } else { (0.51, 0.01) };
        for (c, p) in &faces {
            if *c != class { continue; }
            for v in p {
                coords.push([q(ox + (v[0] - lo[0]) as f64 * s), q(oy + (v[2] - lo[1]) as f64 * s)]);
            }
        }
    }
    // walls: a grid of cells over the upper half, each face its own rectangle (its horizontal extent × height)
    let n_walls = faces.iter().filter(|(c, _)| *c == 2).count().max(1);
    let grid = (n_walls as f64).sqrt().ceil().max(1.0) as usize;
    let cell = 1.0 / grid as f64;
    let mut wi = 0usize;
    for (c, p) in &faces {
        if *c != 2 { continue; }
        let (cx, cy) = ((wi % grid) as f64 * cell, 0.5 + (wi / grid) as f64 * cell * 0.5);
        // in-plane axes: horizontal along the face's longest xz direction, vertical = y
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        let ax = |v: &[f32; 3]| -> f32 { if (p[1][0] - p[0][0]).abs() + (p[2][0] - p[0][0]).abs() >= (p[1][2] - p[0][2]).abs() + (p[2][2] - p[0][2]).abs() { v[0] } else { v[2] } };
        for v in p { lo[0] = lo[0].min(ax(v)); lo[1] = lo[1].min(v[1]); hi[0] = hi[0].max(ax(v)); hi[1] = hi[1].max(v[1]); }
        let ext = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-3) as f64;
        let s = cell * 0.8 / ext;
        for v in p {
            coords.push([q(cx + cell * 0.1 + (ax(v) - lo[0]) as f64 * s), q(cy + cell * 0.05 + (v[1] - lo[1]) as f64 * s * 0.5)]);
        }
        wi += 1;
    }
    // the coords were pushed class by class; the indices must follow FACE order
    let mut per_class_cursor = [0usize; 3];
    let mut class_starts = [0usize; 3];
    let counts: Vec<usize> = (0..3).map(|k| faces.iter().filter(|(c, _)| *c == k).map(|(_, p)| p.len()).sum()).collect();
    class_starts[1] = counts[0];
    class_starts[2] = counts[0] + counts[1];
    for (c, p) in &faces {
        for _ in p {
            indices.push((class_starts[*c] + per_class_cursor[*c]) as u32);
            per_class_cursor[*c] += 1;
        }
    }
    crate::crystal_model::Lightmap::V2 { coords, indices }
}

/// Build a PureFloor-recipe crystal item: the template's bytes with its Geometry layer = `mesh` (visible, NOT
/// collidable), its Trigger layer = `trigger` (or removed, and the item's WaypointType set to None), material 0 =
/// plastic (1, 0.1, 0.1) or the friend's glass, the ident/author = `ident`, the ident collection = `collection`.
pub fn build_pure_item(ident: &str, collection: u32, mesh: &crate::crystal::CrystalMesh, trigger: Option<&crate::crystal::CrystalMesh>) -> Result<Vec<u8>, String> {
    let template = pure_template()?;
    let (link, rgb) = if pure_glass() { (PURE_GLASS_LINK, PURE_GLASS_RGB) } else { (PURE_PLASTIC_LINK, PURE_PLASTIC_RGB) };
    // the friend's material table: slot 0 the plate, slot 1 `ERROR_MAT` (physics 0) for the trigger layer
    let mut specs = vec![crate::crystal::MaterialSpec { link: link.to_string(), physics: PURE_PHYSICS }];
    if trigger.is_some() {
        specs.push(crate::crystal::MaterialSpec { link: "ERROR_MAT".to_string(), physics: 0 });
    }
    let bytes = crate::crystal::build_item(&template, ident, ident, &specs, mesh);
    let bytes = if collection != 26 { crate::tiny_assets::set_ident_collection(&bytes, collection) } else { bytes };
    let mut it = crate::crystal::ItemCrystal::open(&bytes)?;
    let (first_node, n_old) = it.material_node_range();
    let n_mat = it.model.materials.len();
    for (mi, m) in it.model.materials.iter_mut().enumerate() {
        if let Some(main) = m.node.as_mut().and_then(|n| n.inline.as_mut()).and_then(|inst| inst.main.as_mut()) {
            if mi == 0 {
                main.csts = vec![crate::crystal_model::Cst { u01: crate::crystal_model::Id::Str("TargetColor".into()), u02: crate::crystal_model::Id::Str("Real".into()), u03: 3 }];
                main.color = rgb.iter().map(|c| c.to_bits() as i32).collect();
            } else {
                // the friend's ERROR_MAT slot carries no constants
                main.csts.clear();
                main.color.clear();
            }
        }
    }
    let mut kept = Vec::new();
    for mut l in std::mem::take(&mut it.model.layers) {
        match &mut l.kind {
            crate::crystal_model::LayerKind::Geometry { collidable, .. } => {
                *collidable = false;
                kept.push(l);
            }
            crate::crystal_model::LayerKind::Trigger { crystal, .. } => {
                if let Some(t) = trigger {
                    crate::crystal::fill_crystal(crystal, t, n_mat);
                    for f in &mut crystal.faces {
                        f.material = (n_mat as i32 - 1).max(0);
                    }
                    crystal.u02 = crystal.faces.iter().map(|f| f.material).max().unwrap_or(0);
                    kept.push(l);
                }
            }
            _ => kept.push(l),
        }
    }
    it.model.layers = kept;
    it.model.lightmap = Some(grouped_lightmap(&it.model));
    if trigger.is_none() {
        // the item chunk 0x2E00201F: WaypointType 1 (Finish) → 3 (None)
        let pat = 0x2E00201Fu32.to_le_bytes();
        let pos = it.body.windows(4).position(|w| w == pat).ok_or("no 0x2E00201F chunk in the template")?;
        if pos < it.end {
            return Err("waypoint chunk inside the crystal region".into());
        }
        it.body[pos + 8..pos + 12].copy_from_slice(&3i32.to_le_bytes());
    }
    Ok(it.close(first_node, n_old))
}

/// The per-tile PureFloor-recipe visual item: closed slabs, no trigger. v4.20: the geometry is LOCAL to the tile
/// corner `origin` (x, 0, z) like the friend's plates, and the item is placed there (the v4.19 tiles, world
/// coordinates up to 2 km from an item at the origin, did not render; the cutouts, local to their block, did).
pub fn build_pure_tile(bodies: &[Body], ident: &str, collection: u32, origin: [f32; 3]) -> Result<(Vec<u8>, usize), String> {
    let mut mesh = crate::crystal::CrystalMesh::default();
    for b in bodies {
        let mut lb = *b;
        for p in lb.iter_mut() { p[0] -= origin[0]; p[1] -= origin[1]; p[2] -= origin[2]; }
        body_faces(&mut mesh, &lb, 0);
    }
    let bytes = build_pure_item(ident, collection, &mesh, None)?;
    Ok((bytes, mesh.faces.len()))
}
