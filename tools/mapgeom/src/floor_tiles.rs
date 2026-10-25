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
