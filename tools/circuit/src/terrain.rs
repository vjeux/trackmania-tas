//! The ground: a heightfield read off the LIDAR DTM, classified by the
//! LIDAR intensity into asphalt (dark returns: run-off, service roads,
//! paddock) and grass (everything else), cut into tile items.
//!
//! Two nested resolutions with NO overlap: the grid is `coarse` m cells over
//! the whole map; a coarse cell within `near` m of the lap is replaced by
//! its `fine` m sub-cells (the run-off shapes matter there), any other cell
//! is drawn whole. Every heightfield corner within `conform` m beyond the
//! kerbs is pinned to the ROAD plane 6 cm below it instead of the raw DTM,
//! so no ground can stand above the tarmac: the first build let 24 m cells
//! straddle the 13 m road with all four corners past the sink margin and
//! their plane cut 2.3 m through Chapel Curve — the lap video showed the car
//! flying. `check.rs` now measures this on the placed triangles.

use crate::edges::Edges;
use crate::mapbuild::{Frame, Placement, AUTHOR};
use crate::mesh::{self, MeshBuilder};
use crate::tiff::{Mosaic, Raster};
use crate::track::Track;

/// Intensity below this is asphalt (measured: tarmac cores 45..130, grass
/// 300..700, painted concrete and roofs brighter still).
pub const ASPHALT_MAX: f64 = 190.0;

/// How far beyond the kerb's outer edge the ground may not stand above the
/// road plane: a 4 m cell touching the shoulder reaches 5.7 m further, so
/// every corner of every cell under the kerbs and shoulders is inside this
/// reach and the interpolated ground there IS the plane, 6 cm down. Ground
/// below the plane is kept (a ditch stays a ditch); ground above it is
/// shaved to the plane. Under the road and the shoulder the ground is
/// pinned to the plane whichever way the DTM went.
pub const CONFORM: f64 = 7.5;
/// Beyond the kerb, the strip that is pinned outright (the shoulder).
pub const PIN: f64 = crate::mapbuild::SHOULDER + 0.2;
/// The plane the conformed ground sits at, below the road surface.
pub const CONFORM_DROP: f64 = 0.06;

pub struct TerrainSpec {
    /// Venue box in BNG (e0, n0, e1, n1): intensity classification happens
    /// inside it (a 32 m cell outside is grass whatever it saw).
    pub bbox: (f64, f64, f64, f64),
    /// The whole grid's box (the map).
    pub coarse_bbox: (f64, f64, f64, f64),
    pub coarse: f64,
    pub fine: f64,
    /// Coarse cells with any sample point within this many metres of the
    /// kerbs are subdivided.
    pub near: f64,
    /// Tile size (m) per item; a multiple of `coarse`.
    pub tile: f64,
}

/// The roads' footprint on a 2 m grid: for every node, the distance beyond
/// the nearest road's kerb outer edge (negative under a road) and that
/// road's plane height there, from the nearest station's cross-section.
/// A union: `add` every road (the lap first, then the pit lanes and the
/// other tarmac); lookups interpolate bilinearly between nodes.
pub struct Ribbon {
    e0: f64,
    n1: f64,
    step: f64,
    w: usize,
    h: usize,
    d: Vec<f32>,
    plane: Vec<f32>,
    /// The lowest road plane among the roads within CONFORM of the node:
    /// what the ground must stay under (a pit lane 1.2 m above the lap
    /// beside it must not lift the ground over the lap's kerb).
    lid: Vec<f32>,
    /// Which `add` set the node (0 = none): two roads at different heights
    /// (the pit exit runs 1.5 m below Abbey on a bank) must never be
    /// interpolated across.
    road: Vec<u16>,
    roads: u16,
}

/// The kerb strip width the road items draw at station `i`, one side.
pub fn kerb_width(k0: f64, k1: f64) -> f64 {
    if k0.min(4.0) > 0.4 || k1.min(4.0) > 0.4 {
        k0.min(4.0).max(0.5)
    } else {
        0.0
    }
}

impl Ribbon {
    pub fn new(bbox: (f64, f64, f64, f64)) -> Ribbon {
        let step = 2.0;
        let (e0, n0, e1, n1) = bbox;
        let w = ((e1 - e0) / step).ceil() as usize + 1;
        let h = ((n1 - n0) / step).ceil() as usize + 1;
        Ribbon { e0, n1, step, w, h, d: vec![f32::INFINITY; w * h], plane: vec![f32::NAN; w * h], lid: vec![f32::INFINITY; w * h], road: vec![0; w * h], roads: 0 }
    }

    pub fn build(tr: &Track, ed: &Edges, bbox: (f64, f64, f64, f64), reach: f64) -> Ribbon {
        let mut r = Ribbon::new(bbox);
        r.add(tr, ed, reach);
        r
    }

    /// Fold a road in: nodes within `reach` of its kerbs take its distance
    /// where that is nearer than what they had.
    pub fn add(&mut self, tr: &Track, ed: &Edges, reach: f64) {
        let (e0, n1, step, w, h) = (self.e0, self.n1, self.step, self.w, self.h);
        self.roads += 1;
        let id = self.roads;
        let n = tr.len();
        for i in 0..n {
            let st = &tr.stations[i];
            let i1 = tr.next(i);
            let (kl, kr) = (kerb_width(ed.kerb_left[i], ed.kerb_left[i1]), kerb_width(ed.kerb_right[i], ed.kerb_right[i1]));
            let outer = (ed.left[i] + kl).max(ed.right[i] + kr);
            let rr = ((outer + reach) / step).ceil() as i64;
            let cx = ((st.e - e0) / step).round() as i64;
            let cz = ((n1 - st.n) / step).round() as i64;
            let (sh, ch) = st.heading.sin_cos();
            for dz in -rr..=rr {
                for dx in -rr..=rr {
                    let (x, z) = (cx + dx, cz + dz);
                    if x < 0 || z < 0 || x >= w as i64 || z >= h as i64 {
                        continue;
                    }
                    let ce = e0 + x as f64 * step;
                    let cn = n1 - z as f64 * step;
                    // signed lateral offset (+ left) and along-track offset
                    let (de, dn) = (ce - st.e, cn - st.n);
                    let lat = -de * sh + dn * ch;
                    let along = de * ch + dn * sh;
                    // an open road ends where it ends
                    let at_end = !tr.closed && ((i == 0 && along < -0.5) || (i == n - 1 && along > 0.5));
                    if along.abs() > 1.5 && !at_end {
                        continue; // another station is nearer along the road
                    }
                    let edge = if lat >= 0.0 { ed.left[i] + kl } else { ed.right[i] + kr };
                    let mut dist = lat.abs() - edge;
                    if at_end {
                        // past the end: distance grows with the overshoot
                        dist = dist.max(0.0).hypot(along.abs() - 0.5);
                    }
                    let k = z as usize * w + x as usize;
                    // the plane: the road's camber across its own width, then
                    // level beyond the kerb (a verge is flat, the camber is not
                    // a fact about the ground 15 m out)
                    let lat_c = lat.clamp(-(ed.right[i] + kr), ed.left[i] + kl);
                    let plane = (st.z + st.cross_slope * lat_c) as f32;
                    if (dist as f32) < self.d[k] {
                        self.d[k] = dist as f32;
                        self.plane[k] = plane;
                        self.road[k] = id;
                    }
                    if dist < CONFORM && plane < self.lid[k] {
                        self.lid[k] = plane;
                    }
                }
            }
        }
    }

    fn cell(&self, e: f64, n: f64) -> Option<(usize, usize, f64, f64)> {
        let fx = (e - self.e0) / self.step;
        let fz = (self.n1 - n) / self.step;
        if fx < 0.0 || fz < 0.0 || fx >= (self.w - 1) as f64 || fz >= (self.h - 1) as f64 {
            return None;
        }
        Some((fx.floor() as usize, fz.floor() as usize, fx.fract(), fz.fract()))
    }

    fn bilinear(&self, v: &[f32], e: f64, n: f64) -> f32 {
        let Some((x, z, tx, tz)) = self.cell(e, n) else { return f32::INFINITY };
        let g = |xx: usize, zz: usize| v[zz * self.w + xx];
        let (a, b, c, d) = (g(x, z), g(x + 1, z), g(x, z + 1), g(x + 1, z + 1));
        let r = |xx: usize, zz: usize| self.road[zz * self.w + xx];
        let same_road = r(x, z) == r(x + 1, z) && r(x, z) == r(x, z + 1) && r(x, z) == r(x + 1, z + 1);
        if !same_road || !a.is_finite() || !b.is_finite() || !c.is_finite() || !d.is_finite() {
            // the nearest node then (infinite/NaN neighbours are "no road")
            let (nx, nz) = ((x as f64 + tx).round() as usize, (z as f64 + tz).round() as usize);
            return g(nx.min(self.w - 1), nz.min(self.h - 1));
        }
        let top = a as f64 * (1.0 - tx) + b as f64 * tx;
        let bot = c as f64 * (1.0 - tx) + d as f64 * tx;
        (top * (1.0 - tz) + bot * tz) as f32
    }

    /// Distance beyond the nearest kerb's outer edge at (e, n): < 0 under
    /// a road.
    pub fn beyond_kerb(&self, e: f64, n: f64) -> f32 {
        self.bilinear(&self.d, e, n)
    }

    /// The road plane's height at (e, n), where the ribbon knows it.
    pub fn plane(&self, e: f64, n: f64) -> Option<f64> {
        let v = self.bilinear(&self.plane, e, n);
        if v.is_finite() { Some(v as f64) } else { None }
    }

    /// The lowest road plane within CONFORM of (e, n) -- nearest node.
    pub fn lid(&self, e: f64, n: f64) -> Option<f64> {
        let (x, z, tx, tz) = self.cell(e, n)?;
        let (nx, nz) = (((x as f64 + tx).round() as usize).min(self.w - 1), ((z as f64 + tz).round() as usize).min(self.h - 1));
        let v = self.lid[nz * self.w + nx];
        if v.is_finite() { Some(v as f64) } else { None }
    }
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

pub fn terrain_items(ribbon: &Ribbon, dtm: &Mosaic, inten: &Raster, fr: &Frame, spec: &TerrainSpec) -> Vec<Placement> {
    let (e0, n0, e1, n1) = spec.coarse_bbox;
    let (ve0, vn0, ve1, vn1) = spec.bbox;
    let sub = (spec.coarse / spec.fine).round() as usize;
    assert!((sub as f64 * spec.fine - spec.coarse).abs() < 1e-9, "fine must divide coarse");
    let per_tile = (spec.tile / spec.coarse).round() as usize;
    assert!((per_tile as f64 * spec.coarse - spec.tile).abs() < 1e-9, "coarse must divide tile");
    // the fine grid over the whole box; heights at its nodes, sampled once,
    // conformed to the road plane where the ribbon says so
    let fw = ((e1 - e0) / spec.fine).ceil() as usize;
    let fh = ((n1 - n0) / spec.fine).ceil() as usize;
    let mut hz = vec![f64::NAN; (fw + 1) * (fh + 1)];
    let mut conformed = 0usize;
    for j in 0..=fh {
        for i in 0..=fw {
            let e = e0 + i as f64 * spec.fine;
            let n = n1 - j as f64 * spec.fine;
            let mut z = dtm.sample(e, n).unwrap_or(f64::NAN);
            let d = ribbon.beyond_kerb(e, n) as f64;
            if d < CONFORM {
                if let Some(p) = ribbon.lid(e, n) {
                    let lid = p - CONFORM_DROP;
                    z = if d < PIN || z.is_nan() { lid } else { z.min(lid) };
                    conformed += 1;
                }
            }
            hz[j * (fw + 1) + i] = z;
        }
    }
    let cw = fw / sub + 1;
    let ch = fh / sub + 1;
    let tiles_x = (cw + per_tile - 1) / per_tile;
    let tiles_z = (ch + per_tile - 1) / per_tile;
    let mut out = Vec::new();
    let (mut n_coarse, mut n_fine) = (0usize, 0usize);
    for tz in 0..tiles_z {
        for tx in 0..tiles_x {
            let mut cb = MeshBuilder::new();
            let (cg, ca) = (cb.material(&mesh::GRASS), cb.material(&mesh::ASPHALT));
            let mut fb = MeshBuilder::new();
            let (fg, fa) = (fb.material(&mesh::GRASS), fb.material(&mesh::ASPHALT));
            let ae = e0 + (tx * per_tile) as f64 * spec.coarse;
            let an = n1 - (tz * per_tile) as f64 * spec.coarse;
            let az = hz[((tz * per_tile * sub).min(fh)) * (fw + 1) + (tx * per_tile * sub).min(fw)];
            if az.is_nan() {
                continue;
            }
            let anchor = fr.to_tm(ae, an, az);
            let (mut cc, mut fc) = (0usize, 0usize);
            for cj in tz * per_tile..((tz + 1) * per_tile).min(ch) {
                for ci in tx * per_tile..((tx + 1) * per_tile).min(cw) {
                    let (fi0, fj0) = (ci * sub, cj * sub);
                    if fi0 >= fw || fj0 >= fh {
                        continue;
                    }
                    let (fi1, fj1) = ((fi0 + sub).min(fw), (fj0 + sub).min(fh));
                    // near the lap? nine sample points of the coarse cell
                    let ce0 = e0 + fi0 as f64 * spec.fine;
                    let cn0 = n1 - fj0 as f64 * spec.fine;
                    let ce1 = e0 + fi1 as f64 * spec.fine;
                    let cn1 = n1 - fj1 as f64 * spec.fine;
                    let mut near = false;
                    for (u, v) in [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (0.0, 0.5), (0.5, 0.5), (1.0, 0.5), (0.0, 1.0), (0.5, 1.0), (1.0, 1.0)] {
                        let e = ce0 + u * (ce1 - ce0);
                        let n = cn0 + v * (cn1 - cn0);
                        if (ribbon.beyond_kerb(e, n) as f64) < spec.near {
                            near = true;
                            break;
                        }
                    }
                    let corner = |i: usize, j: usize| -> [f32; 3] {
                        let t = fr.to_tm(e0 + i as f64 * spec.fine, n1 - j as f64 * spec.fine, hz[j * (fw + 1) + i]);
                        [t[0] - anchor[0], t[1] - anchor[1], t[2] - anchor[2]]
                    };
                    let emit = |mb: &mut MeshBuilder, grass: usize, asphalt: usize, i0: usize, j0: usize, i1: usize, j1: usize, classify_here: bool| -> bool {
                        let c = [hz[j0 * (fw + 1) + i0], hz[j0 * (fw + 1) + i1], hz[j1 * (fw + 1) + i1], hz[j1 * (fw + 1) + i0]];
                        if c.iter().any(|v| v.is_nan()) {
                            return false;
                        }
                        let me = e0 + (i0 + i1) as f64 * 0.5 * spec.fine;
                        let mn = n1 - (j0 + j1) as f64 * 0.5 * spec.fine;
                        let in_venue = me >= ve0 && me <= ve1 && mn >= vn0 && mn <= vn1;
                        let step = (i1 - i0) as f64 * spec.fine;
                        let mat = if classify_here && in_venue && classify(inten, me, mn, step) { asphalt } else { grass };
                        let q = [corner(i0, j0), corner(i1, j0), corner(i1, j1), corner(i0, j1)];
                        if mat == asphalt {
                            // the plain middle of the RoadTech atlas, tiled every 32 m
                            let uv = |k: usize| -> [f32; 2] {
                                let x = q[k][0] + anchor[0];
                                let z = q[k][2] + anchor[2];
                                [x / 32.0, 0.35 + 0.30 * (z / 32.0).rem_euclid(1.0)]
                            };
                            mb.quad_uv_up(mat, q, [uv(0), uv(1), uv(2), uv(3)], true);
                        } else {
                            mb.quad_up(mat, q, true);
                        }
                        true
                    };
                    if near {
                        for j in fj0..fj1 {
                            for i in fi0..fi1 {
                                if emit(&mut fb, fg, fa, i, j, i + 1, j + 1, true) {
                                    fc += 1;
                                }
                            }
                        }
                    } else if emit(&mut cb, cg, ca, fi0, fj0, fi1, fj1, true) {
                        cc += 1;
                    }
                }
            }
            if cc > 0 {
                let ident = format!("Silverstone\\Ground{tx:02}_{tz:02}.Item.Gbx");
                let physics = cb.physics_hash(None);
                let coll = cb.coll_world(anchor, 0.0);
                let bytes = cb.build(&ident, AUTHOR, None);
                out.push(Placement { ident, bytes, pos: anchor, yaw: 0.0, tag: None, physics, coll });
                n_coarse += cc;
            }
            if fc > 0 {
                let ident = format!("Silverstone\\Verge{tx:02}_{tz:02}.Item.Gbx");
                let physics = fb.physics_hash(None);
                let coll = fb.coll_world(anchor, 0.0);
                let bytes = fb.build(&ident, AUTHOR, None);
                out.push(Placement { ident, bytes, pos: anchor, yaw: 0.0, tag: None, physics, coll });
                n_fine += fc;
            }
        }
    }
    println!("terrain: {n_coarse} coarse cells + {n_fine} fine cells in {} items; {conformed} nodes conformed to the road plane", out.len());
    out
}
