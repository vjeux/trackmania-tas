//! Every other piece of tarmac at the venue: the pit lanes, the Stowe
//! circuit and its pits, the old Bridge and Priory loops, the Porsche
//! centre's handling roads, the link roads between layouts — every OSM
//! `highway=raceway` way the GP lap does not use, minus the unpaved ones
//! (those are ground, and the LIDAR ground already has them).
//!
//! Each way is an open road with its own intensity-read edges, built in
//! order of length and CLIPPED against everything built before it (the lap
//! first): where it runs onto an earlier road it stops at that road's kerb,
//! its heights blend onto that road's plane over the last 4 m, and neither
//! side gets a shoulder there — so a pit exit meets the lap flush, with no
//! lip and no second surface underneath. The lap builder's refusals were
//! only ever about ROUTING the lap; nothing in the world is refused.

use crate::edges::Edges;
use crate::geo::Bng;
use crate::mapbuild::{road_items, Frame, Placement, RoadStyle};
use crate::osm::Ways;
use crate::terrain::{kerb_width, Ribbon, CONFORM};
use crate::tiff::{Mosaic, Raster};
use crate::track::Track;

pub struct Road {
    pub name: String,
    /// Where this road's edges and shoulder surface were read: the LIDAR
    /// intensity (the venue) or aerial imagery (a track laid after the LIDAR).
    pub from_imagery: bool,
    pub track: Track,
    pub edges: Edges,
    /// Slices to leave out (the road runs on an earlier one there).
    pub skip: Vec<bool>,
    /// Per station: (left, right) shoulder wanted.
    pub shoulders: Vec<(bool, bool)>,
    /// Per station: (left, right) edge capped against an earlier road --
    /// the tarmac is contiguous there and the heights blend onto it.
    pub caps: Vec<(bool, bool)>,
    /// Per station, the heights the road was DRAWN at (left edge, centre,
    /// right edge) after blending -- what the check compares against.
    pub drawn: std::cell::RefCell<Vec<[f64; 3]>>,
}

impl Road {
    /// The drawn surface height at station `i`, lateral `off` (+ left).
    pub fn drawn_height(&self, i: usize, off: f64) -> f64 {
        let d = self.drawn.borrow();
        if d.is_empty() {
            let w = self.track.offset(i, off);
            return w[2];
        }
        let [zl, zc, zr] = d[i];
        if off >= 0.0 {
            let l = self.edges.left[i].max(1e-6);
            zc + (zl - zc) * (off / l).min(1.5)
        } else {
            let r = self.edges.right[i].max(1e-6);
            zc + (zr - zc) * (-off / r).min(1.5)
        }
    }
}

/// A way is part of the lap when most of its nodes lie on the lap's tarmac.
fn on_lap(nodes: &[Bng], lap: &Ribbon) -> bool {
    let on = nodes.iter().filter(|p| lap.beyond_kerb(p.e, p.n) < 1.0).count();
    on * 10 >= nodes.len() * 8
}

/// Blend factor onto the union plane: 1 at the kerb, 0 at CONFORM out.
fn blend(d: f64) -> f64 {
    ((CONFORM - d) / 4.0).clamp(0.0, 1.0)
}

pub fn extra_roads(ways: &Ways, lap: &Ribbon, union: &mut Ribbon, dtm: &Mosaic, inten: &Raster) -> Vec<Road> {
    extra_roads_with(ways, lap, union, dtm, inten, &|_| true, Vec::new(), None)
}

/// A road candidate: name, centreline, and whether its edges come from the
/// imagery (true) or the LIDAR intensity (false).
pub type Candidate = (String, Vec<Bng>, bool);

/// `extra_roads` with a way filter (`keep`), extra candidates (polylines of
/// a layout OSM only knows as topology) and the imagery to read them with.
#[allow(clippy::too_many_arguments)]
pub fn extra_roads_with(ways: &Ways, lap: &Ribbon, union: &mut Ribbon, dtm: &Mosaic, inten: &Raster, keep: &dyn Fn(&crate::osm::Way) -> bool, extra: Vec<Candidate>, img: Option<&crate::imagery::Imagery>) -> Vec<Road> {
    let mut candidates: Vec<(f64, Candidate)> = Vec::new();
    for way in &ways.ways {
        if way.tags.get("highway").map(|s| s.as_str()) != Some("raceway") || !keep(way) {
            continue;
        }
        if matches!(way.tags.get("surface").map(|s| s.as_str()), Some("unpaved") | Some("dirt") | Some("gravel") | Some("grass")) {
            continue;
        }
        let pts: Vec<Bng> = way.nodes.iter().filter_map(|id| ways.nodes.get(id).copied()).collect();
        if pts.len() < 2 {
            continue;
        }
        if on_lap(&pts, lap) {
            continue;
        }
        let len: f64 = pts.windows(2).map(|w| ((w[1].e - w[0].e).powi(2) + (w[1].n - w[0].n).powi(2)).sqrt()).sum();
        if len < 15.0 {
            continue; // a stub at a junction
        }
        let name = if way.name.is_empty() { format!("way {}", way.id) } else { way.name.clone() };
        candidates.push((len, (name, pts, false)));
    }
    for (name, pts, from_img) in extra {
        let len: f64 = pts.windows(2).map(|w| ((w[1].e - w[0].e).powi(2) + (w[1].n - w[0].n).powi(2)).sqrt()).sum();
        if len < 12.0 || pts.len() < 2 {
            continue;
        }
        candidates.push((len, (name, pts, from_img)));
    }
    // longest first: the pit lanes and the Stowe circuit define the merges
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mut out = Vec::new();
    for (len, (name, pts, from_img)) in candidates {
        // pit lanes are narrow: fit the ground over +-4 m, not +-6
        let track = if from_img { Track::build_open(&pts, &name, dtm, 1.0, 2.0, 15.0, 3.5) } else { Track::build_open(&pts, &name, dtm, 1.0, 3.0, 8.0, 4.0) };
        let mut edges = match (from_img, img) {
            (true, Some(im)) => Edges::from_imagery(&track, im, crate::kart::MIN_HALF, crate::kart::MAX_HALF, (4.0, 30.0), 2.0).0,
            _ => Edges::from_intensity(&track, inten),
        };
        let n = track.len();
        let mut skip = vec![false; n];
        let mut shoulders = vec![(true, true); n];
        let mut caps = vec![(false, false); n];
        let mut clipped = 0usize;
        for i in 0..n {
            let st = &track.stations[i];
            // the road's own centre on an earlier road: leave the slice out
            // (its edges are still capped below, to 0.5 m each side: the slice
            // leading INTO this station is drawn, and its far end must not
            // carry the full width and the road's own height across the
            // earlier road -- a 1.4 m ramp onto the kart lap came from that)
            if union.beyond_kerb(st.e, st.n) < 0.0 {
                skip[i] = true;
            }
            for left in [true, false] {
                let (edge, kerb) = if left { (&mut edges.left[i], &mut edges.kerb_left[i]) } else { (&mut edges.right[i], &mut edges.kerb_right[i]) };
                let outer = *edge + kerb_width(*kerb, *kerb);
                // walk out from the centre; stop 0.3 m short of where the
                // union's road begins
                let mut off = 0.5;
                let mut cap: Option<(f64, f64, f64)> = None;
                while off <= outer + 0.01 {
                    let w = track.offset(i, if left { off } else { -off });
                    if union.beyond_kerb(w[0], w[1]) < 0.3 {
                        cap = Some((off - 0.1, w[2], union.plane(w[0], w[1]).unwrap_or(w[2])));
                        break;
                    }
                    off += 0.1;
                }
                if let Some((c, own_z, other_z)) = cap {
                    clipped += 1;
                    let c = c.max(0.5);
                    if c < *edge {
                        *edge = c;
                        *kerb = 0.0;
                    } else {
                        *kerb = (c - *edge).max(0.0);
                    }
                    // contiguous tarmac only when the two roads are at the
                    // same level there; a road running alongside on a bank
                    // (the pit exit, 1.5 m below Abbey) keeps its own
                    // heights and its own shoulder
                    // (a track laid after the LIDAR sits on re-graded ground: its
                    // links always meet the lap flush, whatever the old DTM says)
                    let flush = from_img || (own_z - other_z).abs() < 0.35;
                    if left {
                        shoulders[i].0 = false;
                        caps[i].0 = flush;
                    } else {
                        shoulders[i].1 = false;
                        caps[i].1 = flush;
                    }
                }
            }
        }
        // no shoulder either where the shoulder itself would reach an
        // earlier road (its full width plus the same 0.3 m margin: the pit
        // lane's 3.9 m painted apron plus a 1.5 m shoulder reached 0.7 m
        // over the lap's kerb at the Wing)
        for i in 0..n {
            for left in [true, false] {
                let (edge, kerb) = if left { (edges.left[i], edges.kerb_left[i]) } else { (edges.right[i], edges.kerb_right[i]) };
                let reach = edge + kerb_width(kerb, kerb) + crate::mapbuild::SHOULDER + 0.3;
                let mut hit = false;
                let mut off = edge + kerb_width(kerb, kerb);
                while off <= reach {
                    let w = track.offset(i, if left { off } else { -off });
                    if union.beyond_kerb(w[0], w[1]) < 0.3 {
                        hit = true;
                        break;
                    }
                    off += 0.25;
                }
                if hit {
                    if left {
                        shoulders[i].0 = false;
                    } else {
                        shoulders[i].1 = false;
                    }
                }
            }
        }
        let drawn = skip.iter().filter(|s| !**s).count();
        if drawn < 5 {
            continue;
        }
        if std::env::var_os("CIRCUIT_DEBUG_ROADS").is_some() {
            for i in 0..n {
                if skip[i] {
                    continue;
                }
                for left in [true, false] {
                    let (edge, kerb, sh) = if left { (edges.left[i], edges.kerb_left[i], shoulders[i].0) } else { (edges.right[i], edges.kerb_right[i], shoulders[i].1) };
                    let reach = edge + kerb_width(kerb, kerb) + if sh { crate::mapbuild::SHOULDER } else { 0.0 };
                    let w = track.offset(i, if left { reach } else { -reach });
                    let d = union.beyond_kerb(w[0], w[1]);
                    if d < 0.0 {
                        println!("  debug {name} station {i} {} reach {reach:.1} (edge {edge:.1} kerb {kerb:.1} shoulder {sh}) is {d:.2} m INSIDE an earlier road; cap {:?}", if left { "left" } else { "right" }, caps[i]);
                    }
                }
            }
        }
        println!("road {name:<28} {len:>5.0} m, {drawn} slices ({clipped} edge caps against earlier roads), width {:.1}..{:.1}", edges.left.iter().zip(&edges.right).map(|(l, r)| l + r).fold(f64::MAX, f64::min), edges.left.iter().zip(&edges.right).map(|(l, r)| l + r).fold(f64::MIN, f64::max));
        let road = Road { name, from_imagery: from_img, track, edges, skip, shoulders, caps, drawn: std::cell::RefCell::new(Vec::new()) };
        union.add(&road.track, &road.edges, 40.0);
        out.push(road);
    }
    out
}

/// The items for one extra road; `union` is the ribbon of everything built
/// before it (its heights blend onto that near a merge).
pub fn road_items_for(road: &Road, idx: usize, fr: &Frame, before: &Ribbon, inten: &Raster, dtm: &Mosaic, img: Option<&crate::imagery::Imagery>) -> Vec<Placement> {
    let tr = &road.track;
    let ed = &road.edges;
    let shoulder_asphalt = |k: usize, left: bool| -> bool {
        let (edge, kerb) = if left { (ed.left[k], ed.kerb_left[k]) } else { (ed.right[k], ed.kerb_right[k]) };
        let off = edge + kerb_width(kerb, kerb) + 0.75;
        let w = tr.offset(k, if left { off } else { -off });
        match (road.from_imagery, img) {
            (true, Some(im)) => im.asphalt_majority(w[0], w[1], 1.0).unwrap_or(false),
            _ => inten.at(w[0], w[1]).map(|v| (v as f64) < crate::terrain::ASPHALT_MAX).unwrap_or(false),
        }
    };
    let shoulder_sides = |k: usize| road.shoulders[k];
    let worst = std::cell::Cell::new((0.0f64, 0usize, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64));
    let height = |k: usize, off: f64, own: f64| -> f64 {
        // only where this road's tarmac runs into the earlier road (a cap
        // on that side, here or within 3 stations): a road merely NEAR
        // another one keeps its own ground (the pit exit is 1.5 m below
        // Abbey on a bank)
        let lo = k.saturating_sub(3);
        let hi = (k + 3).min(road.caps.len() - 1);
        let capped = (lo..=hi).any(|j| if off >= 0.0 { road.caps[j].0 } else { road.caps[j].1 });
        if !capped {
            return own;
        }
        let w = tr.offset(k, off);
        let d = before.beyond_kerb(w[0], w[1]) as f64;
        match before.plane(w[0], w[1]) {
            Some(p) if d < CONFORM => {
                let b = blend(d);
                // the ribbon's plane is a 2 m lattice of the earlier road's
                // camber, a few cm off the exact surface: a link read off the
                // imagery meets it 4 cm LOW (a step down, never a lip)
                let p = if road.from_imagery { p - 0.04 } else { p };
                let z = b * p + (1.0 - b) * own;
                if (z - own).abs() > worst.get().0 {
                    worst.set(((z - own).abs(), k, off, own, p, w[0], w[1]));
                }
                z
            }
            _ => own,
        }
    };
    let style = RoadStyle { shoulder_asphalt: &shoulder_asphalt, shoulder_sides: &shoulder_sides, height: &height };
    let items = road_items(tr, ed, fr, 100.0, &road.skip, &style, &format!("Tarmac{idx:02}_"));
    {
        let mut drawn = road.drawn.borrow_mut();
        drawn.clear();
        for i in 0..tr.len() {
            let z = |off: f64| height(i, off, tr.offset(i, off)[2]);
            drawn.push([z(ed.left[i]), z(0.0), z(-ed.right[i])]);
        }
    }
    let (dz, k, off, own, p, e, n) = worst.get();
    if dz > 0.15 {
        println!("road {}: largest blend onto an earlier road's plane {dz:.2} m at station {k} off {off:+.1} E{e:.0} N{n:.0} (own {own:.2}, plane {p:.2}, DTM there {:.2})", road.name, dtm.sample(e, n).unwrap_or(f64::NAN));
    }
    items
}
