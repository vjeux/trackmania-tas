//! Floor Finish v4.18 — the VISUAL split into per-tile items (RESEARCH-visual.md): the one waypoint item keeps the
//! trigger (and no visual but its speck); the red slab is drawn by separate NON-waypoint items, one per 128 m tile
//! of the map (+ the ring), each with its own six lightmap charts and PreLightGen. Every trigger body (box or
//! ramp — an 8-corner prism whose top and bottom are bilinear in (x, z)) is cut at the tile borders by bilinear
//! interpolation, so the union of the tiles' bodies is exactly the trigger volume.
use crate::static_item::build::Merged;

pub const TILE_M: f32 = 128.0;
/// The build tag in the tile idents (bump per delivered build: the game caches embedded items by ident).
pub const BUILD_TAG: &str = "v22";
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
static PURE_GLASS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
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
static PURE_LINK_OVERRIDE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// A game material link (`Stadium\Media\Material\X`) in place of the modeler's CustomGlass / CustomPlastic for the
/// crystal's slot 0 — no TargetColor constants then (a game material carries its own look). `None` restores.
pub fn set_pure_material_link(link: Option<&str>) {
    *PURE_LINK_OVERRIDE.lock().unwrap() = link.map(|s| s.to_string());
}

pub fn build_pure_item(ident: &str, collection: u32, mesh: &crate::crystal::CrystalMesh, trigger: Option<&crate::crystal::CrystalMesh>) -> Result<Vec<u8>, String> {
    let template = pure_template()?;
    let (mut link, rgb) = if pure_glass() { (PURE_GLASS_LINK.to_string(), PURE_GLASS_RGB) } else { (PURE_PLASTIC_LINK.to_string(), PURE_PLASTIC_RGB) };
    let override_link = PURE_LINK_OVERRIDE.lock().unwrap().clone();
    if let Some(l) = &override_link { link = l.clone(); }
    let link = link.as_str();
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
            if mi == 0 && override_link.is_none() {
                main.csts = vec![crate::crystal_model::Cst { u01: crate::crystal_model::Id::Str("TargetColor".into()), u02: crate::crystal_model::Id::Str("Real".into()), u03: 3 }];
                main.color = rgb.iter().map(|c| c.to_bits() as i32).collect();
            } else if mi == 0 {
                main.csts.clear();
                main.color.clear();
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

/// v4.21: a body's TOP face only (the friend's single up-facing quad) and its bottom footprint as a flat trigger
/// quad wound DOWN (the friend's trigger quad [3,0,1,2] has (p1−p0)×(p2−p0) = −y).
pub fn body_top_face(mesh: &mut crate::crystal::CrystalMesh, b: &Body, material: u32) {
    let top = [b[3], b[7], b[6], b[2]]; // (x0,z0)↑ (x0,z1)↑ (x1,z1)↑ (x1,z0)↑ — the friend's order, cross = +y
    let base = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&top);
    mesh.faces.push(crate::crystal::Face { verts: (0..4).map(|k| base + k).collect(), uvs: vec![[4.0, 4.0], [4.0, 0.0], [0.0, 0.0], [0.0, 4.0]], material });
}
pub fn body_bottom_face(mesh: &mut crate::crystal::CrystalMesh, b: &Body, material: u32) {
    let bottom = [b[0], b[1], b[5], b[4]]; // wound −y (seen from below)
    let base = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&bottom);
    mesh.faces.push(crate::crystal::Face { verts: (0..4).map(|k| base + k).collect(), uvs: vec![[4.0, 4.0], [4.0, 0.0], [0.0, 0.0], [0.0, 4.0]], material });
}
pub fn body_trigger_face(mesh: &mut crate::crystal::CrystalMesh, b: &Body, material: u32) {
    let bottom = [b[0], b[1], b[5], b[4]]; // (x0,z0) (x1,z0) (x1,z1) (x0,z1) at the floor — cross = −y like the friend's trigger
    let base = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&bottom);
    mesh.faces.push(crate::crystal::Face { verts: (0..4).map(|k| base + k).collect(), uvs: vec![[0.0, 0.0]; 4], material });
}

/// v4.21: a polygon's top cap only (triangles at the cap's y, wound +y) / the same polygon at the floor wound −y.
pub fn polygon_faces(mesh: &mut crate::crystal::CrystalMesh, verts: &[[f32; 3]], tris: &[[u32; 3]], y: f32, up: bool, material: u32) {
    for t in tris {
        let mut p = [verts[t[0] as usize], verts[t[1] as usize], verts[t[2] as usize]];
        for q in p.iter_mut() { q[1] = y; }
        let (u, v) = ([p[1][0] - p[0][0], 0.0, p[1][2] - p[0][2]], [p[2][0] - p[0][0], 0.0, p[2][2] - p[0][2]]);
        let ny = u[2] * v[0] - u[0] * v[2];
        if ny.abs() < 1e-9 { continue; }
        if (ny > 0.0) != up { p.swap(1, 2); }
        let base = mesh.positions.len() as u32;
        mesh.positions.extend_from_slice(&p);
        mesh.faces.push(crate::crystal::Face { verts: (0..3).map(|k| base + k).collect(), uvs: p.iter().map(|q| [q[0] / 8.0, q[2] / 8.0]).collect(), material });
    }
}

/// The per-tile PureFloor-recipe visual item: closed slabs, no trigger. v4.20: the geometry is LOCAL to the tile
/// corner `origin` (x, 0, z) like the friend's plates, and the item is placed there (the v4.19 tiles, world
/// coordinates up to 2 km from an item at the origin, did not render; the cutouts, local to their block, did).
pub fn build_pure_tile(bodies: &[Body], ident: &str, collection: u32, origin: [f32; 3]) -> Result<(Vec<u8>, usize), String> {
    let mut mesh = crate::crystal::CrystalMesh::default();
    let mut trig = crate::crystal::CrystalMesh::default();
    for b in bodies {
        let mut lb = *b;
        for p in lb.iter_mut() { p[0] -= origin[0]; p[1] -= origin[1]; p[2] -= origin[2]; }
        if pure_top_only() {
            // v4.21: top face (up) + bottom face (down) per body, no walls (Hugo 06:26); the trigger layer KEPT as the
            // friend's flat quad at the floor, so the item is a Finish exactly like PureFloorN — the v4.19/4.20 tiles
            // (no trigger layer, WaypointType None) never rendered while the cutouts (trigger layer, Finish) did
            body_top_face(&mut mesh, &lb, 0);
            body_bottom_face(&mut mesh, &lb, 0);
            body_trigger_face(&mut trig, &lb, 0);
        } else {
            body_faces(&mut mesh, &lb, 0);
        }
    }
    let bytes = build_pure_item(ident, collection, &mesh, if pure_top_only() { Some(&trig) } else { None })?;
    Ok((bytes, mesh.faces.len()))
}

/// v4.21 default: the friend's single-sided form (top face + flat trigger); `--tiles-slab` = the v4.20 closed slabs.
static PURE_TOP_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub fn set_pure_top_only(on: bool) {
    PURE_TOP_ONLY.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub fn pure_top_only() -> bool {
    PURE_TOP_ONLY.load(std::sync::atomic::Ordering::Relaxed)
}

/// v4.22: the single-item plane — every body's top + bottom faces (world coordinates) and the closed prisms as the
/// trigger layer; returns (bytes, glass faces, trigger faces).
pub fn build_pure_plane(verts: &[[f32; 3]], ident: &str, collection: u32) -> Result<(Vec<u8>, usize, usize), String> {
    let mut mesh = crate::crystal::CrystalMesh::default();
    let mut trig = crate::crystal::CrystalMesh::default();
    for c in verts.chunks_exact(8) {
        let b: Body = [c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]];
        body_top_face(&mut mesh, &b, 0);
        body_bottom_face(&mut mesh, &b, 0);
        body_faces(&mut trig, &b, 0);
    }
    if mesh.faces.is_empty() {
        return Err("single crystal plane: no bodies".into());
    }
    let bytes = build_pure_item(ident, collection, &mesh, Some(&trig))?;
    Ok((bytes, mesh.faces.len(), trig.faces.len()))
}

/// v4.25: a polygon set (outer loops + holes, the cutout outline) rasterised at `step` and decomposed into
/// axis-aligned rectangles (row runs, consecutive rows with identical runs merged) — the glass caps of a cutout as
/// well-shaped quads instead of a fan of sliver triangles (StructurePillar: 70 of 228 triangles under 5 cm tall,
/// 90 with edges over 20 m → Hugo's "4 thin lines sticking out of the pillar foot"). Returns (x0, x1, z0, z1) rects
/// and the covered area.
pub fn rect_fill(polygons: &[(Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>)], size: (f32, f32), step: f32) -> (Vec<[f32; 4]>, f32) {
    let inside = |x: f32, z: f32| -> bool {
        // even-odd over every loop (outer and holes alike)
        let mut hits = 0usize;
        for (outer, holes) in polygons {
            for lp in std::iter::once(outer).chain(holes.iter()) {
                let n = lp.len();
                for i in 0..n {
                    let (a, b) = (lp[i], lp[(i + 1) % n]);
                    if (a[1] > z) != (b[1] > z) {
                        let xi = a[0] + (z - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                        if xi > x {
                            hits += 1;
                        }
                    }
                }
            }
        }
        hits % 2 == 1
    };
    let (nx, nz) = ((size.0 / step).round() as usize, (size.1 / step).round() as usize);
    let mut rows: Vec<Vec<(usize, usize)>> = Vec::with_capacity(nz);
    for j in 0..nz {
        let z = (j as f32 + 0.5) * step;
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut i = 0usize;
        while i < nx {
            if inside((i as f32 + 0.5) * step, z) {
                let start = i;
                while i + 1 < nx && inside((i as f32 + 1.5) * step, z) {
                    i += 1;
                }
                runs.push((start, i + 1));
            }
            i += 1;
        }
        rows.push(runs);
    }
    // merge vertically: a run (a, b) in row j extends a rect ending at row j with the same (a, b)
    let mut open: Vec<(usize, usize, usize)> = Vec::new(); // (a, b, z0 row)
    let mut rects: Vec<[f32; 4]> = Vec::new();
    for j in 0..=nz {
        let runs: &[(usize, usize)] = if j < nz { &rows[j] } else { &[] };
        let mut next_open: Vec<(usize, usize, usize)> = Vec::new();
        for &(a, b) in runs {
            if let Some(pos) = open.iter().position(|&(oa, ob, _)| oa == a && ob == b) {
                next_open.push(open.remove(pos));
            } else {
                next_open.push((a, b, j));
            }
        }
        for (a, b, z0) in open.drain(..) {
            rects.push([a as f32 * step, b as f32 * step, z0 as f32 * step, j as f32 * step]);
        }
        open = next_open;
    }
    let area = rects.iter().map(|r| (r[1] - r[0]) * (r[3] - r[2])).sum();
    (rects, area)
}

/// The rectangles as crystal faces: top quads at `y1` (up) and bottom quads at `y0` (down).
pub fn rect_faces(mesh: &mut crate::crystal::CrystalMesh, rects: &[[f32; 4]], y0: f32, y1: f32, material: u32) {
    for r in rects {
        let b: Body = [[r[0], y0, r[2]], [r[1], y0, r[2]], [r[1], y1, r[2]], [r[0], y1, r[2]], [r[0], y0, r[3]], [r[1], y0, r[3]], [r[1], y1, r[3]], [r[0], y1, r[3]]];
        body_top_face(mesh, &b, material);
        body_bottom_face(mesh, &b, material);
    }
}

/// The square-with-square-hole caps as four trapezoids (one per side), top (up) and bottom (down).
pub fn frame_faces(mesh: &mut crate::crystal::CrystalMesh, g: f32, s0: f32, s1: f32, y0: f32, y1: f32, material: u32) {
    // the four trapezoids in xz, CCW seen from above (+y): bottom side, right side, top side, left side
    let quads: [[[f32; 2]; 4]; 4] = [
        [[0.0, 0.0], [g, 0.0], [s1, s0], [s0, s0]],
        [[g, 0.0], [g, g], [s1, s1], [s1, s0]],
        [[g, g], [0.0, g], [s0, s1], [s1, s1]],
        [[0.0, g], [0.0, 0.0], [s0, s0], [s0, s1]],
    ];
    for q in quads {
        // CCW in (x, z) seen from +y means the cross product (p1−p0)×(p2−p0) has NEGATIVE y in a right-handed xz
        // frame; the friend's up-facing quad (−N,0,0)(−N,0,N)(0,0,N)(0,0,0) is CW in (x,z) → +y. So the top face is
        // the reversed order, the bottom the given order.
        let top: Vec<[f32; 3]> = q.iter().rev().map(|p| [p[0], y1, p[1]]).collect();
        let bottom: Vec<[f32; 3]> = q.iter().map(|p| [p[0], y0, p[1]]).collect();
        for face in [top, bottom] {
            let base = mesh.positions.len() as u32;
            mesh.positions.extend_from_slice(&face);
            mesh.faces.push(crate::crystal::Face { verts: (0..4).map(|k| base + k).collect(), uvs: face.iter().map(|p| [p[0] / 8.0, p[2] / 8.0]).collect(), material });
        }
    }
}

/// v4.29 (Hugo 04:16 "finishable assets for StructurePillar"; 04:18 "no glass, just the trigger"): a crystal item whose
/// Trigger layer is a SLEEVE prism around the pillar column — the 3 × 3 m shaft (14.5…17.5 on both axes in the
/// block frame) dilated by `pad` → a 4 × 4 m box, from `y0` (the plate / grass level) to `y1` (0.3 m under the row
/// top so a car on the deck above never overlaps) — and whose Geometry layer is a 0.1 mm speck hidden inside the
/// column (a crystal needs at least one face; the friend's items always carry one). Finish, friend's material slot.
pub fn build_pillar_sleeve(ident: &str, collection: u32, shaft: [f32; 4], pad: f32, y0: f32, y1: f32) -> Result<(Vec<u8>, usize), String> {
    let (x0, x1, z0, z1) = (shaft[0] - pad, shaft[1] + pad, shaft[2] - pad, shaft[3] + pad);
    let body: Body = [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]];
    let mut trig = crate::crystal::CrystalMesh::default();
    body_faces(&mut trig, &body, 0);
    // the speck: a 0.1 mm quad at the column's centre, 1 m up (inside the shaft, never visible)
    let (cx, cz) = ((shaft[0] + shaft[1]) / 2.0, (shaft[2] + shaft[3]) / 2.0);
    let e = 0.00005;
    let speck: Body = [[cx - e, y0 + 1.0, cz - e], [cx + e, y0 + 1.0, cz - e], [cx + e, y0 + 1.0 + e, cz - e], [cx - e, y0 + 1.0 + e, cz - e], [cx - e, y0 + 1.0, cz + e], [cx + e, y0 + 1.0, cz + e], [cx + e, y0 + 1.0 + e, cz + e], [cx - e, y0 + 1.0 + e, cz + e]];
    let mut geo = crate::crystal::CrystalMesh::default();
    body_top_face(&mut geo, &speck, 0);
    let bytes = build_pure_item(ident, collection, &geo, Some(&trig))?;
    Ok((bytes, trig.faces.len()))
}

/// Hugo's There-and-Back fallback for `GateExpandableFinish` (10-27): the segment's pane as a crystal CHECKPOINT item in
/// the BLOCK frame — placed with offset 0 / yaw 0 on the segment's own pose, upright or pitched. Pane x 0..32,
/// y 0..7.18, z 15.579..16.416 (the pak's ExpandableCheckpoint_Trigger slab): glass caps on the two big faces only,
/// trigger prism = the full slab, WaypointType Checkpoint.
pub fn build_expandable_checkpoint(ident: &str, collection: u32) -> Result<(Vec<u8>, usize), String> {
    build_expandable_checkpoint_ext(ident, collection, false)
}

/// `all_faces`: the full 6-face box (the pane's exact 24-vertex form) instead of the two caps.
pub fn build_expandable_checkpoint_ext(ident: &str, collection: u32, all_faces: bool) -> Result<(Vec<u8>, usize), String> {
    let (x0, x1, y0, y1, z0, z1) = (0.0f32, 32.0f32, 0.0f32, 7.18f32, 15.579172f32, 16.416002f32);
    let b: Body = [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]];
    let mut trig = crate::crystal::CrystalMesh::default();
    body_faces(&mut trig, &b, 0);
    let mut geo = crate::crystal::CrystalMesh::default();
    if all_faces {
        body_faces(&mut geo, &b, 0);
    } else {
        // (p1−p0)×(p2−p0) = the outward normal: −z for the z0 cap, +z for the z1 cap
        for (corners, _n) in [([b[0], b[3], b[2], b[1]], -1.0f32), ([b[4], b[5], b[6], b[7]], 1.0f32)] {
            let base = geo.positions.len() as u32;
            geo.positions.extend_from_slice(&corners);
            geo.faces.push(crate::crystal::Face { verts: (0..4).map(|k| base + k).collect(), uvs: vec![[0.0, 0.0], [0.0, 2.0], [8.0, 2.0], [8.0, 0.0]], material: 0 });
        }
    }
    let was_glass = pure_glass();
    set_pure_glass(true);
    let built = build_pure_item(ident, collection, &geo, Some(&trig));
    set_pure_glass(was_glass);
    let bytes = built?;
    // the item chunk 0x2E00201F: WaypointType 1 (Finish, the template's) → 2 (Checkpoint); the chunk sits after the
    // crystal region of the decompressed body, so the search cannot hit crystal data
    let mut it = crate::crystal::ItemCrystal::open(&bytes)?;
    let (first_node, n_old) = it.material_node_range();
    let pat = 0x2E00201Fu32.to_le_bytes();
    let end = it.end;
    let pos = it.body[end..].windows(4).position(|w| w == pat).ok_or("no 0x2E00201F chunk after the crystal")? + end;
    it.body[pos + 8..pos + 12].copy_from_slice(&2i32.to_le_bytes());
    Ok((it.close(first_node, n_old), trig.faces.len()))
}
