//! The surroundings, simplified: OSM building footprints extruded to the
//! height the LIDAR first-return surface measures over them (pit building,
//! grandstands, garages, hospitality, the village), OSM bridges as decks
//! over the track, fences and hedges as low walls. Reference points, not
//! architecture — enough to know where you are on the lap.

use crate::geo::Bng;
use crate::mapbuild::{Frame, Placement, AUTHOR};
use crate::mesh::{self, Material, MeshBuilder};
use crate::osm::Ways;
use crate::tiff::Mosaic;

/// Signed area (shoelace); positive = counter-clockwise in (e, n).
fn area(p: &[Bng]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i].e * p[(i + 1) % n].n - p[(i + 1) % n].e * p[i].n).sum::<f64>() / 2.0
}

fn inside(p: &[Bng], e: f64, n: f64) -> bool {
    let mut c = false;
    let m = p.len();
    for i in 0..m {
        let (a, b) = (p[i], p[(i + 1) % m]);
        if (a.n > n) != (b.n > n) && e < (b.e - a.e) * (n - a.n) / (b.n - a.n) + a.e {
            c = !c;
        }
    }
    c
}

/// Ear-clipping triangulation of a simple polygon (indices into `p`).
fn triangulate(p: &[Bng]) -> Vec<[usize; 3]> {
    let n = p.len();
    if n < 3 {
        return Vec::new();
    }
    let ccw = area(p) > 0.0;
    let mut idx: Vec<usize> = if ccw { (0..n).collect() } else { (0..n).rev().collect() };
    let mut out = Vec::new();
    let cross = |a: Bng, b: Bng, c: Bng| (b.e - a.e) * (c.n - a.n) - (b.n - a.n) * (c.e - a.e);
    let mut guard = 0;
    while idx.len() > 3 && guard < 10 * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for k in 0..m {
            let (ia, ib, ic) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (p[ia], p[ib], p[ic]);
            if cross(a, b, c) <= 1e-9 {
                continue; // reflex or degenerate
            }
            // no other vertex inside the ear
            let mut ok = true;
            for &j in &idx {
                if j == ia || j == ib || j == ic {
                    continue;
                }
                let q = p[j];
                if cross(a, b, q) >= 0.0 && cross(b, c, q) >= 0.0 && cross(c, a, q) >= 0.0 {
                    ok = false;
                    break;
                }
            }
            if ok {
                out.push([ia, ib, ic]);
                idx.remove(k);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break; // give up on a self-intersecting outline: fan the rest
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    } else if idx.len() > 3 {
        for k in 1..idx.len() - 1 {
            out.push([idx[0], idx[k], idx[k + 1]]);
        }
    }
    out
}

pub struct Structure {
    pub name: String,
    pub kind: &'static str,
    pub footprint: Vec<Bng>,
    pub base: f64,
    pub height: f64,
    /// Where the walls start: the ground, or 5 m up for a structure that
    /// spans the tarmac (OSM `level`/`layer`/`min_level` >= 1, `bridge`, or a
    /// footprint the lap runs through — the Woodcote footbridge is
    /// `building=yes level=1`, and extruded from the ground it was a wall
    /// across the track that stopped the first drive at station 2724).
    pub bottom: f64,
}

/// Every OSM building inside `bbox`, with its LIDAR height. `tr`/`ed` tell
/// which footprints the lap runs through (those are lifted off the ground).
pub fn structures(w: &Ways, dtm: &Mosaic, dsm: &Mosaic, bbox: (f64, f64, f64, f64), tr: &crate::track::Track, ed: &crate::edges::Edges) -> Vec<Structure> {
    let (e0, n0, e1, n1) = bbox;
    let mut out = Vec::new();
    let nearest = |e: f64, n: f64| -> (usize, f64) {
        let mut best = (0usize, f64::MAX);
        for (i, s) in tr.stations.iter().enumerate().step_by(2) {
            let d = (s.e - e).powi(2) + (s.n - n).powi(2);
            if d < best.1 {
                best = (i, d);
            }
        }
        (best.0, best.1.sqrt())
    };
    let spans_tarmac = |pts: &[Bng]| -> bool {
        let m = pts.len();
        for i in 0..m {
            let (a, b) = (pts[i], pts[(i + 1) % m]);
            let len = ((b.e - a.e).powi(2) + (b.n - a.n).powi(2)).sqrt();
            let steps = (len.ceil() as usize).max(1);
            for k in 0..=steps {
                let t = k as f64 / steps as f64;
                let (e, n) = (a.e + t * (b.e - a.e), a.n + t * (b.n - a.n));
                let (i, d) = nearest(e, n);
                if d < ed.left[i].max(ed.right[i]) + 1.0 {
                    return true;
                }
            }
        }
        // or the lap passes wholly inside the footprint
        tr.stations.iter().step_by(4).any(|s| inside(pts, s.e, s.n))
    };
    let mut lifted = 0usize;
    for way in &w.ways {
        let Some(b) = way.tags.get("building") else { continue };
        if way.nodes.len() < 4 || way.nodes.first() != way.nodes.last() {
            continue;
        }
        let pts: Vec<Bng> = way.nodes[..way.nodes.len() - 1].iter().filter_map(|id| w.nodes.get(id).copied()).collect();
        if pts.len() < 3 {
            continue;
        }
        let ce = pts.iter().map(|p| p.e).sum::<f64>() / pts.len() as f64;
        let cn = pts.iter().map(|p| p.n).sum::<f64>() / pts.len() as f64;
        if ce < e0 || ce > e1 || cn < n0 || cn > n1 {
            continue;
        }
        // ground: median DTM at the vertices; top: 90th percentile of the
        // first-return DSM sampled on a 1 m grid inside the footprint
        let mut g: Vec<f64> = pts.iter().filter_map(|p| dtm.sample(p.e, p.n)).collect();
        if g.is_empty() {
            continue;
        }
        g.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let base = g[g.len() / 2];
        let (mut le, mut ln, mut he, mut hn) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in &pts {
            le = le.min(p.e);
            ln = ln.min(p.n);
            he = he.max(p.e);
            hn = hn.max(p.n);
        }
        let mut tops: Vec<f64> = Vec::new();
        let mut e = le + 0.5;
        while e < he {
            let mut n = ln + 0.5;
            while n < hn {
                if inside(&pts, e, n) {
                    if let Some(v) = dsm.sample(e, n) {
                        tops.push(v);
                    }
                }
                n += 1.0;
            }
            e += 1.0;
        }
        if tops.is_empty() {
            continue;
        }
        tops.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let top = tops[tops.len() * 9 / 10];
        let height = (top - base).clamp(3.0, 60.0);
        let kind = match b.as_str() {
            "grandstand" => "grandstand",
            "house" | "detached" | "semidetached_house" | "terrace" | "residential" | "bungalow" => "house",
            "garage" | "garages" | "shed" | "barn" => "shed",
            _ => "building",
        };
        let kind = if way.name.contains("Stand") || way.name.starts_with("Stowe") || way.name == "Vale" || way.name.starts_with("Club") || way.name.starts_with("Village") || way.name == "The View" { "grandstand" } else { kind };
        let lvl = |k: &str| way.tags.get(k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        let elevated = lvl("level") >= 1.0 || lvl("layer") >= 1.0 || lvl("min_level") >= 1.0 || lvl("min_height") > 0.0 || way.tags.contains_key("bridge") || spans_tarmac(&pts);
        let (bottom, height) = if elevated {
            lifted += 1;
            let bottom = base + lvl("min_height").max(5.0);
            (bottom, (base + height - bottom).max(3.0) + (bottom - base))
        } else {
            (base - 0.5, height)
        };
        out.push(Structure { name: way.name.clone(), kind, footprint: pts, base, height, bottom });
    }
    if lifted > 0 {
        println!("buildings: {lifted} span the tarmac or sit on an upper level; their walls start 5 m up");
    }
    out
}

/// Technics (metal) is near-black in play mode; the light materials are the
/// stadium wall panels and the platform concrete.
fn material_for(kind: &str) -> Material {
    match kind {
        "grandstand" => mesh::WALL,
        _ => mesh::CONCRETE,
    }
}

/// Extruded footprints, grouped into items by 256 m tile of their centroid.
pub fn building_items(structs: &[Structure], fr: &Frame) -> Vec<Placement> {
    let mut tiles: std::collections::BTreeMap<(i64, i64), Vec<&Structure>> = Default::default();
    for s in structs {
        let ce = s.footprint.iter().map(|p| p.e).sum::<f64>() / s.footprint.len() as f64;
        let cn = s.footprint.iter().map(|p| p.n).sum::<f64>() / s.footprint.len() as f64;
        tiles.entry(((ce / 256.0).floor() as i64, (cn / 256.0).floor() as i64)).or_default().push(s);
    }
    let mut out = Vec::new();
    for ((te, tn), list) in tiles {
        let anchor_bng = (te as f64 * 256.0, (tn + 1) as f64 * 256.0);
        let base_z = list.iter().map(|s| s.base).fold(f64::MAX, f64::min);
        let anchor = fr.to_tm(anchor_bng.0, anchor_bng.1, base_z);
        let mut mb = MeshBuilder::new();
        for s in &list {
            let mat = mb.material(&material_for(s.kind));
            let roof_mat = mb.material(&mesh::CONCRETE);
            let n = s.footprint.len();
            let z0 = s.bottom;
            let z1 = s.base + s.height;
            let p = |q: Bng, z: f64| {
                let t = fr.to_tm(q.e, q.n, z);
                [t[0] - anchor[0], t[1] - anchor[1], t[2] - anchor[2]]
            };
            // walls facing away from the footprint's centroid
            let ce = s.footprint.iter().map(|q| q.e).sum::<f64>() / n as f64;
            let cn = s.footprint.iter().map(|q| q.n).sum::<f64>() / n as f64;
            let inside = p(Bng { e: ce, n: cn }, (z0 + z1) / 2.0);
            for i in 0..n {
                let (a, b) = (s.footprint[i], s.footprint[(i + 1) % n]);
                let (a0, a1, b0, b1) = (p(a, z0), p(a, z1), p(b, z0), p(b, z1));
                mb.quad_away(mat, [a0, a1, b1, b0], inside, true);
            }
            // roof (and the underside of a lifted structure), facing out
            let tris = triangulate(&s.footprint);
            let lifted = s.bottom > s.base;
            for t in &tris {
                let (a, b, c) = (p(s.footprint[t[0]], z1), p(s.footprint[t[1]], z1), p(s.footprint[t[2]], z1));
                let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let ny = ab[2] * ac[0] - ab[0] * ac[2];
                if ny >= 0.0 {
                    mb.tri(roof_mat, [a, b, c], true);
                } else {
                    mb.tri(roof_mat, [a, c, b], true);
                }
                if lifted {
                    let (a, b, c) = (p(s.footprint[t[0]], z0), p(s.footprint[t[1]], z0), p(s.footprint[t[2]], z0));
                    if ny >= 0.0 {
                        mb.tri(roof_mat, [a, c, b], true);
                    } else {
                        mb.tri(roof_mat, [a, b, c], true);
                    }
                }
            }
        }
        if mb.is_empty() {
            continue;
        }
        let ident = format!("Silverstone\\Bldg{}_{}.Item.Gbx", te.rem_euclid(1000), tn.rem_euclid(1000));
        let physics = mb.physics_hash(None);
            let bytes = mb.build(&ident, AUTHOR, None);
        out.push(Placement { ident, bytes, pos: anchor, yaw: 0.0, tag: None, physics });
    }
    out
}

/// OSM barriers (fences, hedges, walls) as low walls, and bridges as decks:
/// one item per 256 m tile.
pub fn linear_items(w: &Ways, dtm: &Mosaic, dsm: &Mosaic, ed: &crate::edges::Edges, fr: &Frame, bbox: (f64, f64, f64, f64), tr: &crate::track::Track, near: f64) -> Vec<Placement> {
    let (e0, n0, e1, n1) = bbox;
    // barriers only matter close to the lap (the pit wall, track-side fences);
    // bridges anywhere in the venue
    let near_lap = |e: f64, n: f64| tr.stations.iter().step_by(5).any(|s| (s.e - e).powi(2) + (s.n - n).powi(2) < near * near);
    // A barrier that OSM draws across the tarmac (a fence line through a
    // spectator crossing, a gate) would be an invisible wall across the lap
    // — the first drive stopped dead at station 1473 on the Wellington
    // Straight. Anything within 3 m of the tarmac edge is dropped.
    let nearest = |e: f64, n: f64| -> (usize, f64) {
        let mut best = (0usize, f64::MAX);
        for (i, s) in tr.stations.iter().enumerate().step_by(2) {
            let d = (s.e - e).powi(2) + (s.n - n).powi(2);
            if d < best.1 {
                best = (i, d);
            }
        }
        (best.0, best.1.sqrt())
    };
    let on_tarmac = |a: Bng, b: Bng| -> Option<usize> {
        let len = ((b.e - a.e).powi(2) + (b.n - a.n).powi(2)).sqrt();
        let steps = (len.ceil() as usize).max(1);
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            let (e, n) = (a.e + t * (b.e - a.e), a.n + t * (b.n - a.n));
            let (i, d) = nearest(e, n);
            if d < ed.left[i].max(ed.right[i]) + 3.0 {
                return Some(i);
            }
        }
        None
    };
    let mut dropped: Vec<usize> = Vec::new();
    struct Seg {
        a: Bng,
        b: Bng,
        h: f64,
        thick: f64,
        deck: Option<f64>, // bridge deck height above ground
        mat: Material,
    }
    let mut segs: Vec<Seg> = Vec::new();
    for way in &w.ways {
        let is_bridge = way.tags.get("bridge").map(|v| v == "yes").unwrap_or(false) || way.tags.get("man_made").map(|v| v == "bridge").unwrap_or(false);
        let barrier = way.tags.get("barrier").cloned();
        if !is_bridge && barrier.is_none() {
            continue;
        }
        let pts: Vec<Bng> = way.nodes.iter().filter_map(|id| w.nodes.get(id).copied()).collect();
        for k in 1..pts.len() {
            let (a, b) = (pts[k - 1], pts[k]);
            let (me, mn) = ((a.e + b.e) / 2.0, (a.n + b.n) / 2.0);
            if me < e0 || me > e1 || mn < n0 || mn > n1 {
                continue;
            }
            if !is_bridge && !near_lap(me, mn) {
                continue;
            }
            if !is_bridge {
                if let Some(i) = on_tarmac(a, b) {
                    dropped.push(i);
                    continue;
                }
            }
            if is_bridge {
                // deck height from the first-return surface over the span
                let g = dtm.sample(me, mn).unwrap_or(0.0);
                let top = dsm.sample(me, mn).unwrap_or(g + 6.0);
                let clearance = (top - g).clamp(4.5, 12.0);
                segs.push(Seg { a, b, h: 1.0, thick: 4.0, deck: Some(clearance), mat: mesh::CONCRETE });
            } else {
                let (h, thick, mat) = match barrier.as_deref() {
                    Some("hedge") => (1.6, 1.0, mesh::GRAVEL), // Grass on a vertical face renders as a translucent sheet
                    Some("wall") | Some("retaining_wall") => (1.2, 0.4, mesh::WALL),
                    Some("fence") | Some("wood_fence") => (1.2, 0.15, mesh::WALL),
                    _ => continue,
                };
                segs.push(Seg { a, b, h, thick, deck: None, mat });
            }
        }
    }
    if !dropped.is_empty() {
        dropped.sort_unstable();
        println!("barriers: {} segments crossed the tarmac and were dropped (stations {:?})", dropped.len(), dropped);
    }
    let mut tiles: std::collections::BTreeMap<(i64, i64), Vec<&Seg>> = Default::default();
    for s in &segs {
        let (me, mn) = ((s.a.e + s.b.e) / 2.0, (s.a.n + s.b.n) / 2.0);
        tiles.entry(((me / 256.0).floor() as i64, (mn / 256.0).floor() as i64)).or_default().push(s);
    }
    let mut out = Vec::new();
    for ((te, tn), list) in tiles {
        let (ae, an) = (te as f64 * 256.0, (tn + 1) as f64 * 256.0);
        let az = dtm.sample(ae + 128.0, an - 128.0).unwrap_or(150.0);
        let anchor = fr.to_tm(ae, an, az);
        let mut mb = MeshBuilder::new();
        for s in &list {
            let mat = mb.material(&s.mat);
            let ga = dtm.sample(s.a.e, s.a.n).unwrap_or(az);
            let gb = dtm.sample(s.b.e, s.b.n).unwrap_or(az);
            let dx = s.b.e - s.a.e;
            let dn = s.b.n - s.a.n;
            let len = (dx * dx + dn * dn).sqrt();
            if len < 0.5 {
                continue;
            }
            // unit normal in (e, n)
            let (ue, un) = (-dn / len * s.thick / 2.0, dx / len * s.thick / 2.0);
            let p = |e: f64, n: f64, z: f64| {
                let t = fr.to_tm(e, n, z);
                [t[0] - anchor[0], t[1] - anchor[1], t[2] - anchor[2]]
            };
            let (za0, zb0) = match s.deck {
                Some(c) => (ga + c, gb + c),
                None => (ga - 0.3, gb - 0.3),
            };
            let (za1, zb1) = (za0 + s.h, zb0 + s.h);
            // eight corners of the slab
            let c = [
                p(s.a.e + ue, s.a.n + un, za0), p(s.b.e + ue, s.b.n + un, zb0), p(s.b.e - ue, s.b.n - un, zb0), p(s.a.e - ue, s.a.n - un, za0),
                p(s.a.e + ue, s.a.n + un, za1), p(s.b.e + ue, s.b.n + un, zb1), p(s.b.e - ue, s.b.n - un, zb1), p(s.a.e - ue, s.a.n - un, za1),
            ];
            mb.slab(mat, [c[0], c[1], c[2], c[3]], [c[4], c[5], c[6], c[7]], s.deck.is_some(), true);
        }
        if mb.is_empty() {
            continue;
        }
        let ident = format!("Silverstone\\Line{}_{}.Item.Gbx", te.rem_euclid(1000), tn.rem_euclid(1000));
        let physics = mb.physics_hash(None);
            let bytes = mb.build(&ident, AUTHOR, None);
        out.push(Placement { ident, bytes, pos: anchor, yaw: 0.0, tag: None, physics });
    }
    out
}
