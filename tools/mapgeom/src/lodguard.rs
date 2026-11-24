//! THE LOD GUARD — the size ladder's `--lod-pick N` keeps ONE detail level of a source part
//! at every distance while the item's collision stays the NEAREST level's hull, and Nadeo's
//! coarse levels are chords of that hull on every curved or sloped road: Fall 2026 25 x2 drew
//! its dirt tilt curves 0.15 m (mean) to 1.84 m (max) off the surface the car drove on (vjeux
//! 2026-10-02: "the collision and the texture are not matching … all the sloped blocks").
//!
//! The guard of 2026-10-02 lived on the `stadium2` branch only and was lost with its bundle;
//! this is its re-implementation (2026-11-24, Giant Fall 2026 x4, from the write-up in
//! projects/trackmania/fall-2026-fix-stadium-2-2026-10-02.md):
//!
//! 1. PER PART, PER MATERIAL LINK: every part the pick coarsened (`Merged::picked_parts`) is
//!    compared with the same part of the same item baked at its nearest level. Per material
//!    LINK, the DRIVING surfaces (physics not Metal 4 / Water 13 / ResonantMetal 22 /
//!    NotCollidable 28 / MetalTrans 32; no vegetation, no decal) are sampled on an xz lattice
//!    (`step`), every layer; each interior sample is matched with the closest driving surface
//!    of the other level above or below it, both ways. A link FAILS when more than 2 % of its
//!    interior samples stray more than `tol` (world metres) — a coarse level that draws none of
//!    it (a vanished kerb whose hull stays = an invisible wall) strays everywhere. The failed
//!    links of a part are held at their nearest level, its other links keep the pick — or the
//!    whole part when its look is collision-voted per triangle (terrain id materials) or its
//!    two levels do not draw the same links (one mesh replacing another).
//! 2. THE CLOSING CHECK on the whole item: the TOP-MOST visible surface of the result against
//!    the nearest-level bake's wherever that one is a driving surface (a part's coarse sheet
//!    over another part's deck), and the vis-coll census (the top visual against the collision
//!    hull) no worse than the nearest-level bake's; and per link-held part, no new coplanar
//!    overlap between a held link and a coarse one (z-fighting). A failure holds the coarsened
//!    parts that drew the straying cells whole and checks again; when nothing is left to hold,
//!    the whole item goes to its nearest level.
//!
//! `TINY_LOD_GUARD=0` turns it off; `TINY_LOD_GUARD_TOL` (default 0.05 m) and
//! `TINY_LOD_GUARD_STEP` (default 0.125 m × scale) tune it — `TINY_*` knobs, so the bake cache
//! keys them like every other bake setting.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::static_item::lod::Hold;
use crate::static_item::merged::{visual_triangles, Merged, VEGET_PART_BASE};

/// Physics ids that are no driving surface (the vis-coll gate's list of 2026-10-02).
pub const NON_DRIVING: [u8; 5] = [4, 13, 22, 28, 32];
/// The share of a surface's interior samples that may stray before it fails.
pub const FAIL_SHARE: f64 = 0.02;
/// A surface smaller than this many interior samples is not judged (under 1 m² at x4).
pub const MIN_SAMPLES: usize = 4;
/// Two surfaces closer than this are coplanar (they z-fight).
const COPLANAR: f32 = 0.02;

pub fn enabled() -> bool {
    std::env::var("TINY_LOD_GUARD").map(|v| v != "0").unwrap_or(true)
}

/// The deviation a driving surface may show, in world metres.
pub fn tolerance() -> f32 {
    std::env::var("TINY_LOD_GUARD_TOL").ok().and_then(|v| v.parse().ok()).filter(|v: &f32| *v > 0.0).unwrap_or(0.05)
}

/// The lattice step, in world metres: 0.125 m of the source (0.25 m at x2, 0.5 m at x4).
pub fn step(scale: f32) -> f32 {
    std::env::var("TINY_LOD_GUARD_STEP").ok().and_then(|v| v.parse().ok()).filter(|v: &f32| *v > 0.0).unwrap_or((0.125 * scale.abs().max(1.0)).clamp(0.1, 1.0))
}

fn skipped_link(link: &str) -> bool {
    let l = link.to_ascii_lowercase();
    l.contains("veget") || l.contains("foliage") || l.contains("leaves") || l.contains("decal")
}

/// The material slot's (link, driving), or None for a surface the guard never reads.
fn surface_kind(m: &Merged, slot: usize) -> Option<(String, bool)> {
    let mat = m.materials.get(slot)?;
    let link = mat.link().unwrap_or("").to_string();
    if skipped_link(&link) {
        return None;
    }
    Some((link, !NON_DRIVING.contains(&mat.physics())))
}

pub(crate) type Cell = (i32, i32);

/// Every lattice point inside the xz projection of an up- or down-facing triangle (|n.y| ≥ 0.3
/// of |n|: walls are not surfaces the car stands on), with the triangle's height there.
pub(crate) fn raster(a: [f32; 3], b: [f32; 3], c: [f32; 3], step: f32, out: &mut dyn FnMut(Cell, f32)) {
    let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if !(len > 1e-9) || (n[1] / len).abs() < 0.3 {
        return;
    }
    let d = e1[0] * e2[2] - e2[0] * e1[2];
    if d.abs() < 1e-12 {
        return;
    }
    let (minx, maxx) = (a[0].min(b[0]).min(c[0]), a[0].max(b[0]).max(c[0]));
    let (minz, maxz) = (a[2].min(b[2]).min(c[2]), a[2].max(b[2]).max(c[2]));
    let (i0, i1) = ((minx / step).ceil() as i64, (maxx / step).floor() as i64);
    let (k0, k1) = ((minz / step).ceil() as i64, (maxz / step).floor() as i64);
    if i1 < i0 || k1 < k0 || (i1 - i0 + 1) * (k1 - k0 + 1) > 4_000_000 {
        return;
    }
    for i in i0..=i1 {
        let x = i as f32 * step - a[0];
        for k in k0..=k1 {
            let z = k as f32 * step - a[2];
            let u = (x * e2[2] - e2[0] * z) / d;
            let v = (e1[0] * z - x * e1[2]) / d;
            if u < -1e-4 || v < -1e-4 || u + v > 1.0 + 1e-4 {
                continue;
            }
            out((i as i32, k as i32), a[1] + u * e1[1] + v * e2[1]);
        }
    }
}

/// The triangles of the visuals `keep` selects, with their material slot.
fn for_each_triangle(m: &Merged, keep: &dyn Fn(&crate::static_item::merged::MergedVisual) -> bool, f: &mut dyn FnMut(usize, [f32; 3], [f32; 3], [f32; 3])) {
    for v in m.visuals.iter().filter(|v| v.part < VEGET_PART_BASE && keep(v)) {
        let (pos, idx) = visual_triangles(&v.visual);
        for t in idx.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (pos.get(t[0] as usize), pos.get(t[1] as usize), pos.get(t[2] as usize)) else { continue };
            f(v.material, *a, *b, *c);
        }
    }
}

/// A part's surfaces, every layer: per link and all together.
#[derive(Default)]
struct Layers {
    by_link: BTreeMap<String, HashMap<Cell, Vec<f32>>>,
    all: HashMap<Cell, Vec<f32>>,
}

/// The part's driving surfaces (`driving_only`) or every surface the guard reads.
fn layers(m: &Merged, part: u32, step: f32, driving_only: bool) -> Layers {
    let mut out = Layers::default();
    for_each_triangle(m, &|v| v.part == part, &mut |slot, a, b, c| {
        let Some((link, driving)) = surface_kind(m, slot) else { return };
        if driving_only && !driving {
            return;
        }
        let mut push = |cell: Cell, y: f32| {
            out.by_link.entry(link.clone()).or_default().entry(cell).or_default().push(y);
            out.all.entry(cell).or_default().push(y);
        };
        raster(a, b, c, step, &mut push);
    });
    out
}

/// The links a part's visuals draw (every surface kind, vegetation and decals included: the
/// test is whether the two levels are made of the same meshes).
fn part_links(m: &Merged, part: u32) -> BTreeSet<String> {
    m.visuals.iter().filter(|v| v.part == part).filter_map(|v| m.materials.get(v.material).and_then(|x| x.link()).map(String::from)).collect()
}

const NEIGHBOURS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// A sample with a surface of the same set at every lattice neighbour, within a slope's reach
/// (an edge sample is where two levels' outlines differ by construction, not by deviation).
fn interior(set: &HashMap<Cell, Vec<f32>>, cell: Cell, y: f32, step: f32) -> bool {
    NEIGHBOURS.iter().all(|(di, dk)| set.get(&(cell.0 + di, cell.1 + dk)).map(|ys| ys.iter().any(|h| (h - y).abs() <= 3.0 * step)).unwrap_or(false))
}

/// How a surface reads against another: interior samples, how many stray more than `tol`,
/// the largest finite deviation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stray {
    pub samples: usize,
    pub over: usize,
    pub missing: usize,
    pub max: f32,
}

impl Stray {
    pub fn fails(&self) -> bool {
        self.samples >= MIN_SAMPLES && (self.over as f64) > FAIL_SHARE * self.samples as f64
    }
    /// More than 2 % of the samples off beyond what `base` (the same census of the nearest-level
    /// bake: the same hull, the same samples) already had.
    pub fn worse_than(&self, base: &Stray) -> bool {
        self.samples >= MIN_SAMPLES && (self.over.saturating_sub(base.over) as f64) > FAIL_SHARE * self.samples as f64
    }
    fn add(&mut self, d: Option<f32>, tol: f32) {
        self.samples += 1;
        match d {
            Some(d) => {
                if d > tol {
                    self.over += 1;
                }
                if d > self.max {
                    self.max = d;
                }
            }
            None => {
                self.over += 1;
                self.missing += 1;
            }
        }
    }
    pub fn describe(&self) -> String {
        format!("{:.1} % of {} samples off (max {:.2} m{})", 100.0 * self.over as f64 / self.samples.max(1) as f64, self.samples, self.max, if self.missing > 0 { format!(", {} with no surface", self.missing) } else { String::new() })
    }
}

fn stray(reference: &HashMap<Cell, Vec<f32>>, other: &HashMap<Cell, Vec<f32>>, step: f32, tol: f32) -> Stray {
    let mut s = Stray::default();
    for (cell, ys) in reference {
        for &y in ys {
            if !interior(reference, *cell, y, step) {
                continue;
            }
            let d = other.get(cell).and_then(|hs| hs.iter().map(|h| (h - y).abs()).fold(None, |m: Option<f32>, d| Some(m.map_or(d, |m| m.min(d)))));
            s.add(d, tol);
        }
    }
    s
}

fn short(link: &str) -> &str {
    link.rsplit('\\').next().unwrap_or(link)
}

/// One cell of a TOP-MOST field: the height, whether that surface is a driving one, the part
/// that drew it and its material slot.
#[derive(Clone, Copy)]
struct Top {
    y: f32,
    driving: bool,
    part: u32,
    slot: usize,
}

/// The TOP-MOST read surface per lattice cell (any material but vegetation and decals).
fn top(m: &Merged, step: f32) -> HashMap<Cell, Top> {
    let mut out: HashMap<Cell, Top> = HashMap::new();
    for v in m.visuals.iter().filter(|v| v.part < VEGET_PART_BASE) {
        let Some((_, driving)) = surface_kind(m, v.material) else { continue };
        let (pos, idx) = visual_triangles(&v.visual);
        for t in idx.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (pos.get(t[0] as usize), pos.get(t[1] as usize), pos.get(t[2] as usize)) else { continue };
            raster(*a, *b, *c, step, &mut |cell, y| {
                let e = out.entry(cell).or_insert(Top { y: f32::NEG_INFINITY, driving: false, part: 0, slot: 0 });
                if y > e.y {
                    *e = Top { y, driving, part: v.part, slot: v.material };
                }
            });
        }
    }
    out
}

/// Step 1, AGAINST THE HULL (the guard's reading of 2026-10-02: a coarse level is judged where it
/// is the top surface, against the collision): at every interior cell whose collision top is a
/// driving surface, the top-most visual of the ladder's bake and of the nearest-level bake are
/// read against the hull top. A cell the nearest level has ON the hull (within `tol`) and the
/// ladder's bake has OFF it (or not at all) is blamed on the coarsened (part, link) that draws
/// the ladder's top there and on the one that drew the nearest level's top there (the surface
/// that should be there: a vanished kerb). A (part, link) is held when its blame passes 2 % of
/// the cells it tops in either bake (and MIN_SAMPLES). Cells where Nadeo's own nearest level is
/// off its hull (a Canopy roof) are nobody's fault.
pub fn hull_holds(picked: &Merged, nearest: &Merged, step: f32, tol: f32) -> BTreeMap<u32, Found> {
    let coarsened: BTreeSet<u32> = picked.picked_parts.keys().copied().collect();
    hull_holds_for(picked, nearest, step, tol, &coarsened)
}

/// `hull_holds` over an explicit set of candidate parts (the decimated ones).
pub fn hull_holds_for(picked: &Merged, nearest: &Merged, step: f32, tol: f32, coarsened: &BTreeSet<u32>) -> BTreeMap<u32, Found> {
    let hull = hull_top(nearest, step);
    let hull_layers: HashMap<Cell, Vec<f32>> = hull.iter().map(|(c, (y, _))| (*c, vec![*y])).collect();
    let (tp, tn) = (top(picked, step), top(nearest, step));
    let link_of = |m: &Merged, slot: usize| m.materials.get(slot).and_then(|x| x.link()).unwrap_or("").to_string();
    let mut tops: BTreeMap<(u32, String), usize> = BTreeMap::new();
    let mut blame: BTreeMap<(u32, String), (usize, f32)> = BTreeMap::new();
    for (cell, (y, driving)) in &hull {
        if !*driving || !interior(&hull_layers, *cell, *y, step) {
            continue;
        }
        let (p, n) = (tp.get(cell), tn.get(cell));
        let keys: BTreeSet<(u32, String)> = [p.filter(|t| coarsened.contains(&t.part)).map(|t| (t.part, link_of(picked, t.slot))), n.filter(|t| coarsened.contains(&t.part)).map(|t| (t.part, link_of(nearest, t.slot)))].into_iter().flatten().collect();
        for k in &keys {
            *tops.entry(k.clone()).or_default() += 1;
        }
        let (dp, dn) = (p.map(|t| (t.y - y).abs()), n.map(|t| (t.y - y).abs()));
        if dn.map_or(false, |d| d <= tol) && dp.map_or(true, |d| d > tol) {
            let dev = dp.unwrap_or(f32::INFINITY);
            for k in keys {
                let e = blame.entry(k).or_insert((0, 0.0));
                e.0 += 1;
                if dev.is_finite() && dev > e.1 {
                    e.1 = dev;
                }
            }
        }
    }
    let mut failing: BTreeMap<u32, Vec<(String, usize, usize, f32)>> = BTreeMap::new();
    for ((part, link), (bad, max)) in &blame {
        let tot = tops.get(&(*part, link.clone())).copied().unwrap_or(*bad).max(*bad);
        if *bad >= MIN_SAMPLES && (*bad as f64) > FAIL_SHARE * tot as f64 {
            failing.entry(*part).or_default().push((link.clone(), *bad, tot, *max));
        }
    }
    let mut holds = BTreeMap::new();
    for (part, links) in failing {
        let level = picked.picked_parts.get(&part).copied().unwrap_or(0);
        let whole = nearest.voted_parts.contains(&part) || part_links(nearest, part) != part_links(picked, part);
        let set: BTreeSet<String> = links.iter().map(|l| l.0.clone()).collect();
        let what = if whole { "whole".to_string() } else { format!("links {}", set.iter().map(|l| short(l)).collect::<Vec<_>>().join("+")) };
        let why: Vec<String> = links.iter().map(|(l, bad, tot, max)| format!("{} {:.1} % of {tot} cells off the hull (max {:.2} m)", short(l), 100.0 * *bad as f64 / (*tot).max(1) as f64, max)).collect();
        holds.insert(part, Found { whole, links: set, level, why: format!("part {part} (level {level}, {what}): {}", why.join("; ")) });
    }
    holds
}

/// A part `hull_holds` blames: whole or by links, the level it was read at, the reason.
pub struct Found {
    pub whole: bool,
    pub links: BTreeSet<String>,
    pub level: u32,
    pub why: String,
}

/// Step 2: the result's top-most surface against the nearest-level bake's, where that one is a
/// driving surface; with the COARSENED parts that drew a straying cell in either bake (the parts
/// holding which can mend it).
pub fn closing(result: &Merged, nearest: &Merged, step: f32, tol: f32, coarsened: &BTreeSet<u32>) -> (Stray, BTreeSet<u32>) {
    let near = top(nearest, step);
    let got = top(result, step);
    let near_layers: HashMap<Cell, Vec<f32>> = near.iter().map(|(c, t)| (*c, vec![t.y])).collect();
    let mut s = Stray::default();
    let mut culprits = BTreeSet::new();
    for (cell, t) in &near {
        if !t.driving || !interior(&near_layers, *cell, t.y, step) {
            continue;
        }
        let g = got.get(cell);
        let d = g.map(|g| (g.y - t.y).abs());
        s.add(d, tol);
        if d.map_or(true, |d| d > tol) {
            if let Some(g) = g.filter(|g| coarsened.contains(&g.part)) {
                culprits.insert(g.part);
            }
            if coarsened.contains(&t.part) {
                culprits.insert(t.part);
            }
        }
    }
    (s, culprits)
}

/// The collision hull's TOP-MOST surface per cell (every physics but NotCollidable 28, which the
/// car never touches) and whether it is a driving one.
fn hull_top(m: &Merged, step: f32) -> HashMap<Cell, (f32, bool)> {
    let mut out: HashMap<Cell, (f32, bool)> = HashMap::new();
    for t in &m.surf_triangles {
        let phys = t.material_id as u8;
        if phys == 28 {
            continue;
        }
        let (Some(a), Some(b), Some(c)) = (m.surf_vertices.get(t.indices[0] as usize), m.surf_vertices.get(t.indices[1] as usize), m.surf_vertices.get(t.indices[2] as usize)) else { continue };
        let driving = !NON_DRIVING.contains(&phys);
        raster(*a, *b, *c, step, &mut |cell, y| {
            let e = out.entry(cell).or_insert((f32::NEG_INFINITY, false));
            if y > e.0 {
                *e = (y, driving);
            }
        });
    }
    out
}

/// THE VIS-COLL CENSUS (the gate's reading of 2026-10-02): at every interior cell whose hull top
/// is a driving surface, the top-most visual surface's distance from it — for the result and for
/// the nearest-level bake of the same item (the same hull): Nadeo's own nearest levels are not
/// all on their hulls (a Canopy roof, a banked chicane's border), so the guard reads the result
/// RELATIVE to it (`Stray::worse_than`). The culprits: coarsened parts drawing a cell the result
/// has off the hull where the nearest level has it on.
pub fn vis_coll_rel(result: &Merged, nearest: &Merged, step: f32, tol: f32, coarsened: &BTreeSet<u32>) -> (Stray, Stray, BTreeSet<u32>) {
    let hull = hull_top(result, step);
    let vr = top(result, step);
    let vn = top(nearest, step);
    let hull_layers: HashMap<Cell, Vec<f32>> = hull.iter().map(|(c, (y, _))| (*c, vec![*y])).collect();
    let (mut sr, mut sn) = (Stray::default(), Stray::default());
    let mut culprits = BTreeSet::new();
    for (cell, (y, driving)) in &hull {
        if !*driving || !interior(&hull_layers, *cell, *y, step) {
            continue;
        }
        let (r, n) = (vr.get(cell), vn.get(cell));
        let (dr, dn) = (r.map(|t| (t.y - y).abs()), n.map(|t| (t.y - y).abs()));
        sr.add(dr, tol);
        sn.add(dn, tol);
        if dr.map_or(true, |d| d > tol) && dn.map_or(false, |d| d <= tol) {
            for t in [r, n].into_iter().flatten() {
                if coarsened.contains(&t.part) {
                    culprits.insert(t.part);
                }
            }
        }
    }
    (sr, sn, culprits)
}

/// Cells where a surface of a link in `a` lies within COPLANAR of a surface of a link in `b`.
fn overlap(l: &Layers, a: &BTreeSet<String>, b: &BTreeSet<String>) -> usize {
    let mut n = 0usize;
    for la in a {
        let Some(fa) = l.by_link.get(la) else { continue };
        for (cell, ys) in fa {
            let hit = b.iter().filter(|lb| *lb != la).filter_map(|lb| l.by_link.get(lb)).filter_map(|fb| fb.get(cell)).any(|hs| ys.iter().any(|y| hs.iter().any(|h| (h - y).abs() < COPLANAR)));
            if hit {
                n += 1;
            }
        }
    }
    n
}

/// The link-held parts of `result` whose held links now lie COPLANAR with one of the part's
/// coarse links more than in the nearest-level bake (one mesh drawn twice = z-fighting).
fn coplanar_parts(result: &Merged, nearest: &Merged, holds: &BTreeMap<u32, Hold>, step: f32) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for (part, hold) in holds {
        let Hold::Links(held_levels) = hold else { continue };
        let held: BTreeSet<String> = held_levels.keys().cloned().collect();
        let lr = layers(result, *part, step, false);
        let ln = layers(nearest, *part, step, false);
        let others: BTreeSet<String> = lr.by_link.keys().filter(|l| !held.contains(*l)).cloned().collect();
        let (a, b) = (overlap(&lr, &held, &others), overlap(&ln, &held, &others));
        let base: usize = others.iter().filter_map(|l| lr.by_link.get(l)).map(|f| f.len()).sum();
        if a > b && (a - b) as f64 > (FAIL_SHARE * base as f64).max(16.0) {
            out.insert(*part);
        }
    }
    out
}

/// How many closing rounds may add culprit parts before the whole item goes to its nearest level.
const CLOSING_ROUNDS: usize = 3;

/// THE DECIMATION FLOW: bake again with the decimation in force (the held nearest-level visuals,
/// or with `all` every nearest-level visual), read the decimated parts against the SAME bake
/// undecimated (`out`: a decimated part answers for the cells its decimation moved off the hull,
/// not for its coarse neighbours') and the item's vis-coll census; one more round without the
/// parts off the hull; else everything back. Returns the LAST bake run and the note.
#[allow(clippy::too_many_arguments)]
fn decimate_flow<T>(bake: &mut impl FnMut(bool, &BTreeMap<u32, Hold>) -> Result<T, String>, merged: &impl Fn(&mut T) -> &mut Merged, mut out: T, holds: &BTreeMap<u32, Hold>, step: f32, tol: f32, dtol: f32, all: bool) -> Result<(T, String), String> {
    let mut targets = holds.clone();
    let mut exclude: BTreeSet<u32> = BTreeSet::new();
    for round in 0..2 {
        let mut dec = with_decimation_all(Some((dtol, targets.clone())), if all { Some(exclude.clone()) } else { None }, || bake(false, holds))?;
        let parts = DECIMATED.with(|d| d.borrow().clone());
        if parts.is_empty() {
            return Ok((dec, "lod guard: nothing to decimate".into()));
        }
        let blamed_why = hull_holds_for(merged(&mut dec), merged(&mut out), step, tol, &parts);
        let why_s: Vec<String> = blamed_why.values().map(|f| f.why.clone()).collect();
        let blamed: BTreeSet<u32> = blamed_why.into_keys().collect();
        let (dvc, dvc0, _) = vis_coll_rel(merged(&mut dec), merged(&mut out), step, tol, &parts);
        let what = if all { "nearest-level" } else { "held" };
        if !dvc.worse_than(&dvc0) && blamed.is_empty() {
            let (b, a) = DECIMATED_VERTS.with(|d| d.get());
            return Ok((dec, format!("lod guard: {} {what} part(s) decimated within {dtol} m ({b} -> {a} vertices; vis-coll {} vs {}){}", parts.len(), dvc.describe(), dvc0.describe(), if round > 0 { " after putting back the parts off the hull" } else { "" })));
        }
        if round == 1 || (dvc.worse_than(&dvc0) && blamed.is_empty()) {
            let back = bake(false, holds)?;
            return Ok((back, format!("lod guard: decimation put back (vis-coll {} vs {}; {} part(s) off the hull: {})", dvc.describe(), dvc0.describe(), blamed.len(), why_s.join(" | "))));
        }
        targets.retain(|p, _| !blamed.contains(p));
        exclude.extend(blamed.iter().copied());
        if targets.is_empty() && !all {
            let back = bake(false, holds)?;
            return Ok((back, format!("lod guard: decimation put back ({} part(s) off the hull: {})", blamed.len(), why_s.join(" | "))));
        }
        // `dec` is not the reference: `out` stays the undecimated bake
        let _ = &mut out;
    }
    let back = bake(false, holds)?;
    Ok((back, "lod guard: decimation put back".into()))
}

/// `TINY_LOD_DECIMATE=M`: the held nearest-level visuals get OUR coarse level, decimated within
/// M metres (`crate::decimate`), then read against the hull like any other level (unset: off).
pub fn decimate_tolerance() -> Option<f32> {
    std::env::var("TINY_LOD_DECIMATE").ok().and_then(|v| v.parse().ok()).filter(|v: &f32| *v > 0.0)
}

thread_local! {
    /// The decimation in force for ONE bake on this thread: the tolerance and the holds whose
    /// nearest-level visuals get decimated (`apply_decimation`, called before every assembly).
    static DECIMATE: std::cell::RefCell<Option<(f32, BTreeMap<u32, Hold>)>> = std::cell::RefCell::new(None);
    /// TINY_LOD_DECIMATE_ALL: every nearest-level visual of the bake (held, or of a part the pick
    /// left alone) is decimated but these parts' (put back after a round off the hull)
    static DECIMATE_ALL: std::cell::RefCell<Option<BTreeSet<u32>>> = std::cell::RefCell::new(None);
    /// What the last `apply_decimation` on this thread decimated: the parts, the vertex counts.
    static DECIMATED: std::cell::RefCell<BTreeSet<u32>> = std::cell::RefCell::new(BTreeSet::new());
    static DECIMATED_VERTS: std::cell::Cell<(usize, usize)> = std::cell::Cell::new((0, 0));
}

/// Run `f` (a bake) with the decimation `spec` in force (restored afterwards).
pub fn with_decimation<T>(spec: Option<(f32, BTreeMap<u32, Hold>)>, f: impl FnOnce() -> T) -> T {
    with_decimation_all(spec, None, f)
}

/// `with_decimation` with the ALL mode in force when `all` is Some (its parts excluded).
pub fn with_decimation_all<T>(spec: Option<(f32, BTreeMap<u32, Hold>)>, all: Option<BTreeSet<u32>>, f: impl FnOnce() -> T) -> T {
    let before = DECIMATE.with(|d| std::mem::replace(&mut *d.borrow_mut(), spec));
    let before_all = DECIMATE_ALL.with(|d| std::mem::replace(&mut *d.borrow_mut(), all));
    DECIMATED.with(|d| d.borrow_mut().clear());
    DECIMATED_VERTS.with(|d| d.set((0, 0)));
    let out = f();
    DECIMATE.with(|d| *d.borrow_mut() = before);
    DECIMATE_ALL.with(|d| *d.borrow_mut() = before_all);
    out
}

/// `TINY_LOD_DECIMATE_ALL=1`: decimate every nearest-level visual, not only the held ones.
pub fn decimate_all() -> bool {
    std::env::var("TINY_LOD_DECIMATE_ALL").map(|v| v == "1").unwrap_or(false)
}

/// THE HOOK every item assembly calls: with a decimation in force (`with_decimation`), the
/// visuals of the parts it names that are at their nearest level (a whole-held part, or a link
/// held at level 0) are replaced by their decimation (`crate::decimate`) in place.
pub fn apply_decimation(m: &mut Merged) {
    // once per bake: a second assembly in the same bake must not decimate the decimation
    let Some((tol, holds)) = DECIMATE.with(|d| d.borrow_mut().take()) else { return };
    let all = DECIMATE_ALL.with(|d| d.borrow_mut().take());
    let mut parts = BTreeSet::new();
    // the held nearest-level visuals go in ONE decimation (their shared edges move together);
    // every other visual's positions stay put (`decimate_many`'s lock)
    let mut held_idx: Vec<usize> = Vec::new();
    let mut locked: std::collections::HashSet<[u32; 3]> = std::collections::HashSet::new();
    for i in 0..m.visuals.len() {
        let part = m.visuals[i].part;
        let link = m.materials.get(m.visuals[i].material).and_then(|x| x.link()).unwrap_or("").to_string();
        let held = part < VEGET_PART_BASE
            && (match holds.get(&part) {
                Some(Hold::Whole) => true,
                Some(Hold::Links(l)) => l.get(&link) == Some(&0),
                None => false,
            } || all.as_ref().map(|ex| !ex.contains(&part) && !m.picked_parts.contains_key(&part) && !holds.contains_key(&part)).unwrap_or(false));
        if held {
            held_idx.push(i);
        } else {
            let (p, _) = visual_triangles(&m.visuals[i].visual);
            locked.extend(p.iter().map(|x| x.map(f32::to_bits)));
        }
    }
    let refs: Vec<&crate::static_item::visual::CPlugVisualIndexedTriangles> = held_idx.iter().map(|i| &m.visuals[*i].visual).collect();
    let (outs, before, after) = crate::decimate::decimate_many(&refs, &locked, tol);
    for (i, o) in held_idx.iter().zip(outs) {
        if let Some(o) = o {
            m.visuals[*i].visual = o;
            parts.insert(m.visuals[*i].part);
        }
    }
    DECIMATED.with(|d| *d.borrow_mut() = parts);
    DECIMATED_VERTS.with(|d| d.set((before, after)));
}


/// The whole decision for one item. `bake(all, holds)` bakes the item with the override in force
/// (every part at its nearest level, or the parts held as listed) and hands back its output;
/// `merged` reaches the output's `Merged`. The ladder's bake stands when no part strays; else
/// the straying links (or parts) are held at their nearest level; then the closing check: a
/// failure holds the coarsened parts that drew the straying cells whole and checks again (up to
/// CLOSING_ROUNDS times); when nothing is left to hold, the whole item goes to its nearest level.
/// The output handed back is always the LAST bake run, so per-bake state the caller keeps
/// (externals, sidecars) is its own.
pub fn guarded<T>(scale: f32, mut bake: impl FnMut(bool, &BTreeMap<u32, Hold>) -> Result<T, String>, merged: impl Fn(&mut T) -> &mut Merged) -> Result<T, String> {
    let none: BTreeMap<u32, Hold> = BTreeMap::new();
    let mut first = bake(false, &none)?;
    if !enabled() || merged(&mut first).picked_parts.is_empty() {
        // nothing coarsened: with TINY_LOD_DECIMATE_ALL the item's nearest-level visuals still
        // get our coarse level
        if let (true, Some(dtol), true) = (enabled(), decimate_tolerance(), decimate_all()) {
            let (step, tol) = (step(scale), tolerance());
            let (mut o, n) = decimate_flow(&mut bake, &merged, first, &none, step, tol, dtol, true)?;
            merged(&mut o).notes.push(n);
            return Ok(o);
        }
        return Ok(first);
    }
    let (step, tol) = (step(scale), tolerance());
    let coarsened: BTreeSet<u32> = merged(&mut first).picked_parts.keys().copied().collect();
    let mut nearest = bake(true, &none)?;
    let found = hull_holds(merged(&mut first), merged(&mut nearest), step, tol);
    // a blamed link takes the next finer level first (THE STEP-DOWN: the coarsest level of
    // Nadeo's own ladder still on the hull), the nearest level when that one strays too
    let mut holds: BTreeMap<u32, Hold> = found
        .iter()
        .map(|(p, f)| (*p, if f.whole { Hold::Whole } else { Hold::Links(f.links.iter().map(|l| (l.clone(), f.level.saturating_sub(1))).collect()) }))
        .collect();
    let mut why: Vec<String> = found.into_values().map(|f| f.why).collect();
    let mut candidate = if holds.is_empty() { first } else { bake(false, &holds)? };
    for _ in 0..3 {
        let finer: bool = holds.values().any(|h| matches!(h, Hold::Links(m) if m.values().any(|l| *l > 0)));
        if !finer {
            break;
        }
        let still = hull_holds(merged(&mut candidate), merged(&mut nearest), step, tol);
        let mut changed = false;
        for (part, f) in &still {
            match holds.get_mut(part) {
                Some(Hold::Whole) => {}
                Some(Hold::Links(m)) if !f.whole => {
                    for l in &f.links {
                        match m.get_mut(l) {
                            Some(e) if *e > 0 => {
                                *e -= 1;
                                changed = true;
                            }
                            Some(_) => {}
                            None => {
                                m.insert(l.clone(), f.level.saturating_sub(1));
                                changed = true;
                            }
                        }
                    }
                }
                _ => {
                    holds.insert(*part, Hold::Whole);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
        why.push(format!("step-down: {} part(s) still off the hull at their finer level", still.len()));
        candidate = bake(false, &holds)?;
    }
    let mut rounds = 0usize;
    loop {
        // the top surface against the nearest-level bake is REPORTED; the decision is the gate's
        // own census (vis-coll against the hull, relative to the nearest level) + no z-fighting
        let (top_check, _) = closing(merged(&mut candidate), merged(&mut nearest), step, tol, &coarsened);
        let (vc, vc0, c2) = vis_coll_rel(merged(&mut candidate), merged(&mut nearest), step, tol, &coarsened);
        let fights = coplanar_parts(merged(&mut candidate), merged(&mut nearest), &holds, step);
        let tail = format!("vis-coll {} vs the nearest level's {}; top surface vs the nearest level {}", vc.describe(), vc0.describe(), top_check.describe());
        if !vc.worse_than(&vc0) && fights.is_empty() {
            // the last bake run is `candidate` unless it is still `first` (baked before `nearest`)
            let mut out = if holds.is_empty() && rounds == 0 { bake(false, &none)? } else { candidate };
            let note = if holds.is_empty() {
                format!("lod guard: {} coarsened part(s) on the hull within {tol} m ({tail})", coarsened.len())
            } else {
                let whole = holds.values().filter(|h| **h == Hold::Whole).count();
                format!("lod guard: {} of {} coarsened part(s) held ({whole} whole) — {} ({tail})", holds.len(), coarsened.len(), why.join(" | "))
            };
            // OUR coarse level for what the guard holds at the nearest level (TINY_LOD_DECIMATE),
            // and with TINY_LOD_DECIMATE_ALL for every nearest-level visual
            let mut dnote: Option<String> = None;
            if let Some(dtol) = decimate_tolerance() {
                if !holds.is_empty() || decimate_all() {
                    let (o, n) = decimate_flow(&mut bake, &merged, out, &holds, step, tol, dtol, decimate_all())?;
                    out = o;
                    dnote = Some(n);
                }
            }
            merged(&mut out).notes.push(note);
            if let Some(n) = dnote {
                merged(&mut out).notes.push(n);
            }
            return Ok(out);
        }
        let mut add: BTreeSet<u32> = fights.clone();
        if vc.worse_than(&vc0) {
            add.extend(c2.iter().copied());
        }
        add.retain(|p| holds.get(p) != Some(&Hold::Whole));
        if add.is_empty() || rounds >= CLOSING_ROUNDS {
            let mut out = bake(true, &none)?;
            merged(&mut out).notes.push(format!(
                "lod guard: the whole item at its nearest level — the closing check failed ({tail}){}",
                if why.is_empty() { String::new() } else { format!("; {}", why.join(" | ")) }
            ));
            return Ok(out);
        }
        why.push(format!("closing round {}: parts {:?} held whole{} ({tail})", rounds + 1, add, if fights.is_empty() { String::new() } else { format!(" (coplanar {:?})", fights) }));
        for p in add {
            holds.insert(p, Hold::Whole);
        }
        candidate = bake(false, &holds)?;
        rounds += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(a: [f32; 3], b: [f32; 3], c: [f32; 3], step: f32) -> HashMap<Cell, Vec<f32>> {
        let mut m: HashMap<Cell, Vec<f32>> = HashMap::new();
        raster(a, b, c, step, &mut |cell, y| m.entry(cell).or_default().push(y));
        m
    }

    #[test]
    fn raster_covers_the_triangle_with_its_plane_height() {
        // a 4 m right triangle tilted 1 m over x: lattice 1 m
        let m = collect([0.0, 0.0, 0.0], [4.0, 1.0, 0.0], [0.0, 0.0, 4.0], 1.0);
        assert!(m.contains_key(&(0, 0)) && m.contains_key(&(4, 0)) && m.contains_key(&(0, 4)) && m.contains_key(&(2, 2)));
        assert!(!m.contains_key(&(3, 3)));
        assert!((m[&(2, 1)][0] - 0.5).abs() < 1e-5);
        // a wall is no surface
        assert!(collect([0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0], 1.0).is_empty());
    }

    #[test]
    fn a_chord_strays_and_a_copy_does_not() {
        let near = collect([0.0, 0.0, 0.0], [8.0, 0.0, 0.0], [0.0, 0.0, 8.0], 0.5);
        let same = collect([0.0, 0.0, 0.0], [8.0, 0.0, 0.0], [0.0, 0.0, 8.0], 0.5);
        let s = stray(&near, &same, 0.5, 0.05);
        assert!(s.samples > 50 && s.over == 0 && !s.fails());
        // the coarse level 0.3 m lower everywhere
        let low = collect([0.0, -0.3, 0.0], [8.0, -0.3, 0.0], [0.0, -0.3, 8.0], 0.5);
        assert!(stray(&near, &low, 0.5, 0.05).fails());
        // a coarse level that draws nothing there: every sample missing
        let none: HashMap<Cell, Vec<f32>> = HashMap::new();
        let s = stray(&near, &none, 0.5, 0.05);
        assert!(s.fails() && s.missing == s.samples);
    }

    #[test]
    fn coplanar_links_count_as_overlap() {
        let mut l = Layers::default();
        l.by_link.insert("A".into(), collect([0.0, 0.0, 0.0], [8.0, 0.0, 0.0], [0.0, 0.0, 8.0], 1.0));
        l.by_link.insert("B".into(), collect([0.0, 0.01, 0.0], [8.0, 0.01, 0.0], [0.0, 0.01, 8.0], 1.0));
        l.by_link.insert("C".into(), collect([0.0, 1.0, 0.0], [8.0, 1.0, 0.0], [0.0, 1.0, 8.0], 1.0));
        let a: BTreeSet<String> = ["A".to_string()].into();
        assert!(overlap(&l, &a, &["B".to_string()].into()) > 20);
        assert_eq!(overlap(&l, &a, &["C".to_string()].into()), 0);
    }
}
