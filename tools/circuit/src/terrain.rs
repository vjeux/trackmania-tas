//! The ground: a heightfield read off the LIDAR DTM, classified by the
//! LIDAR intensity into asphalt (dark returns: run-off, service roads,
//! paddock) and grass (everything else), cut into tile items. Two layers:
//! a fine one (2 m cells) in a band around the lap where the run-off shapes
//! matter, and a coarse one (8 m) over the whole venue underneath it.

use crate::edges::Edges;
use crate::mapbuild::{Frame, Placement, AUTHOR};
use crate::mesh::{self, MeshBuilder};
use crate::tiff::{Mosaic, Raster};
use crate::track::Track;

/// Intensity below this is asphalt (measured: tarmac cores 45..130, grass
/// 300..700, painted concrete and roofs brighter still).
pub const ASPHALT_MAX: f64 = 190.0;

pub struct TerrainSpec {
    /// Venue box in BNG (e0, n0, e1, n1).
    pub bbox: (f64, f64, f64, f64),
    pub coarse: f64,
    pub fine: f64,
    /// Half-width of the fine band around the lap's edges (m).
    pub band: f64,
    /// Tile size (m) per item.
    pub tile: f64,
}

struct Cell {
    e: f64,
    n: f64,
    asphalt: bool,
}

/// Which 2 m-cell centres lie within `band` of the lap (edge-to-edge).
fn band_mask(tr: &Track, ed: &Edges, bbox: (f64, f64, f64, f64), step: f64, band: f64) -> (Vec<bool>, usize, usize) {
    let (e0, n0, e1, n1) = bbox;
    let w = ((e1 - e0) / step).ceil() as usize;
    let h = ((n1 - n0) / step).ceil() as usize;
    let mut mask = vec![false; w * h];
    let r = (band / step).ceil() as i64;
    for i in 0..tr.len() {
        let st = &tr.stations[i];
        let half = ed.left[i].max(ed.right[i]);
        let rr = ((half + band) / step).ceil() as i64;
        let cx = ((st.e - e0) / step) as i64;
        let cz = ((n1 - st.n) / step) as i64;
        for dz in -rr..=rr {
            for dx in -rr..=rr {
                let (x, z) = (cx + dx, cz + dz);
                if x < 0 || z < 0 || x >= w as i64 || z >= h as i64 {
                    continue;
                }
                let ce = e0 + (x as f64 + 0.5) * step;
                let cn = n1 - (z as f64 + 0.5) * step;
                let d = ((ce - st.e).powi(2) + (cn - st.n).powi(2)).sqrt();
                if d <= half + band {
                    mask[z as usize * w + x as usize] = true;
                }
            }
        }
        let _ = r;
    }
    (mask, w, h)
}

fn classify(inten: &Raster, e: f64, n: f64, step: f64) -> bool {
    // asphalt when most of four sub-samples are dark
    let mut dark = 0;
    let mut seen = 0;
    for (dx, dn) in [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)] {
        if let Some(v) = inten.at(e + dx * step, n + dn * step) {
            seen += 1;
            if (v as f64) < ASPHALT_MAX {
                dark += 1;
            }
        }
    }
    seen > 0 && dark * 2 > seen
}

/// One layer of terrain tiles. `only` restricts cells (fine band); `lift`
/// raises the layer so it sits on the coarse one without z-fighting.
#[allow(clippy::too_many_arguments)]
fn layer(tr: &Track, dtm: &Mosaic, inten: &Raster, fr: &Frame, spec: &TerrainSpec, step: f64, only: Option<&(Vec<bool>, usize, usize)>, lift: f32, prefix: &str) -> Vec<Placement> {
    let (e0, n0, e1, n1) = spec.bbox;
    let w = ((e1 - e0) / step).ceil() as usize;
    let h = ((n1 - n0) / step).ceil() as usize;
    // heights at cell corners (w+1 x h+1), sampled once
    let mut hz = vec![f64::NAN; (w + 1) * (h + 1)];
    for j in 0..=h {
        for i in 0..=w {
            let e = e0 + i as f64 * step;
            let n = n1 - j as f64 * step;
            hz[j * (w + 1) + i] = dtm.sample(e, n).unwrap_or(f64::NAN);
        }
    }
    let per_tile = (spec.tile / step).round() as usize;
    let tiles_x = (w + per_tile - 1) / per_tile;
    let tiles_z = (h + per_tile - 1) / per_tile;
    let mut out = Vec::new();
    let _ = tr;
    for tz in 0..tiles_z {
        for tx in 0..tiles_x {
            let mut mb = MeshBuilder::new();
            let grass = mb.material(&mesh::GRASS);
            let asphalt = mb.material(&mesh::ASPHALT);
            // anchor: tile's north-west corner at ground level there
            let ae = e0 + (tx * per_tile) as f64 * step;
            let an = n1 - (tz * per_tile) as f64 * step;
            let az = hz[(tz * per_tile).min(h) * (w + 1) + (tx * per_tile).min(w)];
            if az.is_nan() {
                continue;
            }
            let anchor = fr.to_tm(ae, an, az);
            let mut cells = 0usize;
            for j in tz * per_tile..((tz + 1) * per_tile).min(h) {
                for i in tx * per_tile..((tx + 1) * per_tile).min(w) {
                    if let Some((mask, mw, _)) = only {
                        // fine layer: only band cells (its grid is the same step)
                        if !mask[j * mw + i] {
                            continue;
                        }
                    }
                    let c = [hz[j * (w + 1) + i], hz[j * (w + 1) + i + 1], hz[(j + 1) * (w + 1) + i + 1], hz[(j + 1) * (w + 1) + i]];
                    if c.iter().any(|v| v.is_nan()) {
                        continue;
                    }
                    let ce = e0 + (i as f64 + 0.5) * step;
                    let cn = n1 - (j as f64 + 0.5) * step;
                    let mat = if classify(inten, ce, cn, step) { asphalt } else { grass };
                    let p = |di: usize, dj: usize, z: f64| {
                        let t = fr.to_tm(e0 + (i + di) as f64 * step, n1 - (j + dj) as f64 * step, z);
                        [t[0] - anchor[0], t[1] - anchor[1] + lift, t[2] - anchor[2]]
                    };
                    // corners: (i,j) NW, (i+1,j) NE, (i+1,j+1) SE, (i,j+1) SW.
                    // Asphalt takes the plain middle of the RoadTech atlas
                    // (v 0.35..0.65, no edge lines), tiled every 32 m; grass
                    // is box-mapped.
                    let q = [p(0, 0, c[0]), p(1, 0, c[1]), p(1, 1, c[2]), p(0, 1, c[3])];
                    if mat == asphalt {
                        let uv = |k: usize| -> [f32; 2] {
                            let x = q[k][0] + anchor[0];
                            let z = q[k][2] + anchor[2];
                            [x / 32.0, 0.35 + 0.30 * (z / 32.0).rem_euclid(1.0)]
                        };
                        mb.quad_uv_up(mat, q, [uv(0), uv(1), uv(2), uv(3)], true);
                    } else {
                        mb.quad_up(mat, q, true);
                    }
                    cells += 1;
                }
            }
            if cells == 0 {
                continue;
            }
            let ident = format!("Silverstone\\{prefix}{tx:02}_{tz:02}.Item.Gbx");
            let bytes = mb.build(&ident, AUTHOR, None);
            out.push(Placement { ident, bytes, pos: anchor, yaw: 0.0, tag: None });
        }
    }
    out
}

pub fn terrain_items(tr: &Track, ed: &Edges, dtm: &Mosaic, inten: &Raster, fr: &Frame, spec: &TerrainSpec) -> Vec<Placement> {
    let mut out = layer(tr, dtm, inten, fr, spec, spec.coarse, None, -0.10, "Ground");
    let mask = band_mask(tr, ed, spec.bbox, spec.fine, spec.band);
    let fine = layer(tr, dtm, inten, fr, spec, spec.fine, Some(&mask), -0.04, "Verge");
    let n_fine = fine.len();
    out.extend(fine);
    println!("terrain: {} coarse tiles + {} fine tiles", out.len() - n_fine, n_fine);
    out
}
