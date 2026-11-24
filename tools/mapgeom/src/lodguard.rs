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
//! 1. PER PART: every part the pick coarsened (`Merged::picked_parts`) is compared with the
//!    same part of the same item baked at its nearest level. Per material LINK of the nearest
//!    level, the DRIVING surfaces (physics not Metal 4 / Water 13 / ResonantMetal 22 /
//!    NotCollidable 28 / MetalTrans 32; no vegetation, no decal) are sampled on an xz lattice
//!    (`step`), every layer; each interior sample is matched with the closest driving surface
//!    of the coarse level above or below it. A link FAILS when more than 2 % of its interior
//!    samples stray more than `tol` (world metres) — a coarse level that draws none of it (a
//!    vanished kerb whose hull stays = an invisible wall) strays everywhere. The reverse
//!    direction (a coarse sheet where the nearest level has no surface) is read the same way.
//!    A failed part keeps its nearest level; the others keep the pick.
//! 2. THE CLOSING CHECK on the whole item: the TOP-MOST visible surface of the result against
//!    the nearest-level bake's, wherever that one is a driving surface — a part's coarse sheet
//!    over another part's deck (RoadTechHole in the original). A failure rebakes the item with
//!    every part at its nearest level.
//!
//! `TINY_LOD_GUARD=0` turns it off; `TINY_LOD_GUARD_TOL` (default 0.05 m) and
//! `TINY_LOD_GUARD_STEP` (default 0.125 m × scale) tune it — `TINY_*` knobs, so the bake cache
//! keys them like every other bake setting.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::static_item::merged::{visual_triangles, Merged, VEGET_PART_BASE};

/// Physics ids that are no driving surface (the vis-coll gate's list of 2026-10-02).
pub const NON_DRIVING: [u8; 5] = [4, 13, 22, 28, 32];
/// The share of a surface's interior samples that may stray before it fails.
pub const FAIL_SHARE: f64 = 0.02;
/// A surface smaller than this many interior samples is not judged (under 1 m² at x4).
pub const MIN_SAMPLES: usize = 4;

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

type Cell = (i32, i32);

/// Every lattice point inside the xz projection of an up- or down-facing triangle (|n.y| ≥ 0.3
/// of |n|: walls are not surfaces the car stands on), with the triangle's height there.
fn raster(a: [f32; 3], b: [f32; 3], c: [f32; 3], step: f32, out: &mut dyn FnMut(Cell, f32)) {
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

/// A part's driving surfaces, every layer: per link and all together.
#[derive(Default)]
struct Layers {
    by_link: BTreeMap<String, HashMap<Cell, Vec<f32>>>,
    all: HashMap<Cell, Vec<f32>>,
}

fn layers(m: &Merged, part: u32, step: f32) -> Layers {
    let mut out = Layers::default();
    for_each_triangle(m, &|v| v.part == part, &mut |slot, a, b, c| {
        let Some((link, true)) = surface_kind(m, slot) else { return };
        let mut push = |cell: Cell, y: f32| {
            out.by_link.entry(link.clone()).or_default().entry(cell).or_default().push(y);
            out.all.entry(cell).or_default().push(y);
        };
        raster(a, b, c, step, &mut push);
    });
    out
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

/// Step 1: the parts of `picked` (the ladder's bake) whose coarse level strays from the same
/// part in `nearest` (every part at its nearest level), with the reason per part.
pub fn parts_to_keep(picked: &Merged, nearest: &Merged, step: f32, tol: f32) -> BTreeMap<u32, String> {
    let mut keep = BTreeMap::new();
    for (&part, &level) in &picked.picked_parts {
        let near = layers(nearest, part, step);
        if near.all.is_empty() {
            continue; // no driving surface in the part at all
        }
        let coarse = layers(picked, part, step);
        let mut why: Vec<String> = Vec::new();
        for (link, field) in &near.by_link {
            let s = stray(field, &coarse.all, step, tol);
            if s.fails() {
                why.push(format!("{} {}", link.rsplit('\\').next().unwrap_or(link), s.describe()));
            }
        }
        for (link, field) in &coarse.by_link {
            let s = stray(field, &near.all, step, tol);
            if s.fails() {
                why.push(format!("level-{level} {} sheet {}", link.rsplit('\\').next().unwrap_or(link), s.describe()));
            }
        }
        if !why.is_empty() {
            keep.insert(part, format!("part {part} (level {level}): {}", why.join("; ")));
        }
    }
    keep
}

/// The TOP-MOST read surface per lattice cell (any material but vegetation and decals) and
/// whether it is a driving one.
fn top(m: &Merged, step: f32) -> HashMap<Cell, (f32, bool)> {
    let mut out: HashMap<Cell, (f32, bool)> = HashMap::new();
    for_each_triangle(m, &|_| true, &mut |slot, a, b, c| {
        let Some((_, driving)) = surface_kind(m, slot) else { return };
        let mut push = |cell: Cell, y: f32| {
            let e = out.entry(cell).or_insert((f32::NEG_INFINITY, false));
            if y > e.0 {
                *e = (y, driving);
            }
        };
        raster(a, b, c, step, &mut push);
    });
    out
}

/// Step 2: the result's top-most surface against the nearest-level bake's, where that one is a
/// driving surface.
pub fn closing(result: &Merged, nearest: &Merged, step: f32, tol: f32) -> Stray {
    let near = top(nearest, step);
    let got = top(result, step);
    let near_layers: HashMap<Cell, Vec<f32>> = near.iter().map(|(c, (y, _))| (*c, vec![*y])).collect();
    let mut s = Stray::default();
    for (cell, (y, driving)) in &near {
        if !*driving || !interior(&near_layers, *cell, *y, step) {
            continue;
        }
        s.add(got.get(cell).map(|(h, _)| (h - y).abs()), tol);
    }
    s
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
/// is a driving surface, the top-most visual surface's distance from it. Nadeo's own nearest
/// levels are not all on their hulls (a Canopy roof, a banked chicane's border), so the guard
/// reads it RELATIVE to the nearest-level bake of the same item: `Stray::worse_than`.
pub fn vis_coll(m: &Merged, step: f32, tol: f32) -> Stray {
    let hull = hull_top(m, step);
    let vis = top(m, step);
    let hull_layers: HashMap<Cell, Vec<f32>> = hull.iter().map(|(c, (y, _))| (*c, vec![*y])).collect();
    let mut s = Stray::default();
    for (cell, (y, driving)) in &hull {
        if !*driving || !interior(&hull_layers, *cell, *y, step) {
            continue;
        }
        s.add(vis.get(cell).map(|(h, _)| (h - y).abs()), tol);
    }
    s
}

impl Stray {
    /// More than 2 % of the samples off beyond what `base` (the same census of the nearest-level
    /// bake: the same hull, the same samples) already had.
    pub fn worse_than(&self, base: &Stray) -> bool {
        self.samples >= MIN_SAMPLES && (self.over.saturating_sub(base.over) as f64) > FAIL_SHARE * self.samples as f64
    }
}

/// The whole decision for one item. `bake(all, parts)` bakes the item with the override in force
/// (every part, or the parts listed, at their nearest level) and hands back its output; `merged`
/// reaches the output's `Merged`. The ladder's bake stands when no part strays; else the
/// straying parts are held at their nearest level; the closing check (the top surface against
/// the nearest-level bake, and the vis-coll census no worse than the nearest-level bake's)
/// decides between that and the whole item at its nearest level. The output handed back is
/// always the LAST bake run, so per-bake state the caller keeps (externals, sidecars) is its own.
pub fn guarded<T>(scale: f32, mut bake: impl FnMut(bool, &BTreeSet<u32>) -> Result<T, String>, merged: impl Fn(&mut T) -> &mut Merged) -> Result<T, String> {
    let none = BTreeSet::new();
    let mut first = bake(false, &none)?;
    if !enabled() || merged(&mut first).picked_parts.is_empty() {
        return Ok(first);
    }
    let (step, tol) = (step(scale), tolerance());
    let mut nearest = bake(true, &none)?;
    let keep_why = parts_to_keep(merged(&mut first), merged(&mut nearest), step, tol);
    let keep: BTreeSet<u32> = keep_why.keys().copied().collect();
    let why: Vec<String> = keep_why.values().cloned().collect();
    let mut candidate = if keep.is_empty() { first } else { bake(false, &keep)? };
    let top_check = closing(merged(&mut candidate), merged(&mut nearest), step, tol);
    let vc = vis_coll(merged(&mut candidate), step, tol);
    let vc0 = vis_coll(merged(&mut nearest), step, tol);
    let coarsened = merged(&mut candidate).picked_parts.len() + keep.len();
    if top_check.fails() || vc.worse_than(&vc0) {
        let mut out = bake(true, &none)?;
        merged(&mut out).notes.push(format!(
            "lod guard: the whole item at its nearest level — the closing check failed (top surface {}; vis-coll {} vs the nearest level's {}){}",
            top_check.describe(),
            vc.describe(),
            vc0.describe(),
            if why.is_empty() { String::new() } else { format!("; {}", why.join(" | ")) }
        ));
        return Ok(out);
    }
    let mut out = if keep.is_empty() { bake(false, &none)? } else { candidate };
    let note = if keep.is_empty() {
        format!("lod guard: {coarsened} coarsened part(s) on the hull within {tol} m (top surface {}; vis-coll {} vs the nearest level's {})", top_check.describe(), vc.describe(), vc0.describe())
    } else {
        format!("lod guard: {} of {coarsened} coarsened part(s) held at the nearest level — {} (top surface {}; vis-coll {} vs the nearest level's {})", keep.len(), why.join(" | "), top_check.describe(), vc.describe(), vc0.describe())
    };
    merged(&mut out).notes.push(note);
    Ok(out)
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
}
