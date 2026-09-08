//! Trees that overlap the track are DROPPED (vjeux, tiny Summer 20,
//! 2026-09-08: "the trees are still overlapping parts of the map" — "actually
//! can you just remove the trees that overlap with the road?").
//!
//! The game ignores the placement scale of a `VegetTreeModel`, so every tree
//! of a tiny map is a full-size stock species standing in a half-size world:
//! one size down and sunk to crown height (235ae1f8), it is still about twice
//! as big as it should be, and where the original had a palm beside a road
//! the tiny has its crown through the deck. The rule here is the simple one:
//!
//! * every tree placement (the `v@` rows a block's prefab carries, and the
//!   map's own vegetation items re-pointed at a stock species) is an upright
//!   cylinder at its final position: radius and height from the STAND-IN
//!   species' own model (`veget::tree_model_stats`; a trunk-only palm mesh
//!   gets the crown allowance the sink uses, and a crown radius of 0.4 × its
//!   height — the fronds are procedural, not in the file);
//! * every DRIVING surface — the up-facing collision triangles of the
//!   block-derived items whose block is a deck (Road*, Platform*,
//!   OpenTechRoad*, Gate*; not the DecoPlatform terraces) — is placed where
//!   `tmmaps tiny` places the item (the same origin and yaw functions);
//! * a tree whose cylinder, from half a metre above its foot to its top,
//!   meets any such triangle is dropped. The half metre keeps a tree that
//!   STANDS on a deck (its foot is the deck top). No margin beyond the
//!   crown radius, no per-map tuning.
//!
//! Everything is measured in the SOURCE frame times the scale — `tmmaps
//! tiny`'s anchor transform is affine and uniform, so distances there are
//! distances here. The verdicts travel in the mapping file (`xv@` rows for a
//! block placement's k-th prefab tree, `xvb@` for a baked block's, `xi@` for
//! an item placement) and `tmmaps tiny` leaves those trees out.

use std::collections::{BTreeMap, HashMap};

use crate::store::DataStore;

/// One up-facing collision triangle in the scaled source frame.
pub type Tri = [[f32; 3]; 3];

/// A tree placement to test: where its foot is, how big its stand-in is.
#[derive(Clone, Debug)]
pub struct Tree {
    /// The mapping row that drops it: `xv@N\tK`, `xvb@N\tK` or `xi@N`.
    pub row: String,
    pub species: String,
    pub pos: [f32; 3],
    pub radius: f32,
    pub height: f32,
    /// What the tree belongs to, for the report (block name, alias).
    pub owner: String,
    /// The placement the tree came out of (`@N` / `b@N` / `i@N`): a prefab's
    /// own vegetation is never tested against that prefab's own decks — it
    /// was authored standing on them (Summer 12: the 699 grass tufts and
    /// bushes of ONE RoadDirtStraightOnDirtHill2 stand on the hill that IS
    /// the model's collision; every one of them went, 726 of the map's 754).
    pub from: String,
}

/// A deck placement: the block's alias, its world origin and yaw, the block
/// name (report), and the triangles of its model (item space, scaled).
pub struct Deck<'a> {
    pub alias: String,
    pub name: String,
    /// The placement (`@N` / `b@N`), matched against `Tree::from`.
    pub key: String,
    pub origin: [f32; 3],
    pub yaw: f32,
    pub tris: &'a [Tri],
}

/// Is this block a driving surface a tree must clear? Roads, platforms and
/// gates — not the `DecoPlatform*` terraces and `Stand*`s, which are the
/// landscape the trees are planted in (Summer 20: with those counted, 510 of
/// 892 palms went — every palm at the foot of a terrace step).
pub fn is_deck_block(name: &str) -> bool {
    !name.starts_with("DecoPlatform") && ["Platform", "Road", "OpenTechRoad", "Gate"].iter().any(|p| name.starts_with(p))
}

/// The up-facing triangles of a baked model's collision (item space): a
/// normal with y above 0.5 — deck tops and ramps, not walls or undersides.
pub fn up_facing(vertices: &[[f32; 3]], triangles: &[crate::static_item::surface::Triangle]) -> Vec<Tri> {
    let mut out = Vec::new();
    for t in triangles {
        let [a, b, c] = [vertices[t.indices[0] as usize], vertices[t.indices[1] as usize], vertices[t.indices[2] as usize]];
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 1e-9 && n[1] / len > 0.5 {
            out.push([a, b, c]);
        }
    }
    out
}

/// A tree's cylinder from its stand-in species: (crown radius, height).
/// Trunk-only meshes (the Stadium palms: radius under a metre, procedural
/// fronds) get the sink's crown allowance (`CROWN_ALLOWANCE`, 3 m) on top
/// and a crown radius of 0.4 × the height; a species whose model does not
/// read borrows a sibling's (PalmTreeDirtSmall → PalmTreeSmall).
/// The crown a trunk-only tree model (the Stadium palms: radius under a
/// metre, procedural fronds) is given on top of its trunk, metres unscaled —
/// the fronds a half tree would carry are what the roads must clear.
pub const CROWN_ALLOWANCE: f32 = 3.0;

pub fn species_dims(store: &mut DataStore, name: &str, cache: &mut BTreeMap<String, Option<(f32, f32)>>) -> Option<(f32, f32)> {
    if let Some(d) = cache.get(name) {
        return *d;
    }
    fn measured(store: &mut DataStore, name: &str) -> Option<(f32, f32)> {
        let path = crate::tiny_library::find_item_file(store, name)?;
        let s = crate::veget::tree_model_stats(store, &path).ok()?;
        let height = if s.radius < 1.0 { s.top + CROWN_ALLOWANCE } else { s.top };
        let radius = s.radius.max(0.4 * height);
        Some((radius, height))
    }
    let mut d = measured(store, name);
    if d.is_none() {
        for sib in crate::tiny_library::species_siblings(name) {
            if let Some(x) = measured(store, &sib) {
                d = Some(x);
                break;
            }
        }
    }
    cache.insert(name.to_string(), d);
    d
}

/// Squared distance from (px, pz) to a triangle's xz projection (0 inside).
fn dist2_xz(px: f32, pz: f32, t: &Tri) -> f32 {
    let p = [px, pz];
    let v: [[f32; 2]; 3] = [[t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]];
    // inside test by edge signs
    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let s0 = cross(v[0], v[1], p);
    let s1 = cross(v[1], v[2], p);
    let s2 = cross(v[2], v[0], p);
    if (s0 >= 0.0 && s1 >= 0.0 && s2 >= 0.0) || (s0 <= 0.0 && s1 <= 0.0 && s2 <= 0.0) {
        return 0.0;
    }
    let seg = |a: [f32; 2], b: [f32; 2]| -> f32 {
        let d = [b[0] - a[0], b[1] - a[1]];
        let l2 = d[0] * d[0] + d[1] * d[1];
        let t = if l2 <= 1e-12 { 0.0 } else { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) };
        let q = [a[0] + t * d[0], a[1] + t * d[1]];
        (p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1])
    };
    seg(v[0], v[1]).min(seg(v[1], v[2])).min(seg(v[2], v[0]))
}

/// A world triangle set in 8 m xz buckets.
pub struct Grid {
    cell: f32,
    /// (triangle, display owner, placement key)
    tris: Vec<(Tri, String, String)>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl Grid {
    pub fn new() -> Grid {
        Grid { cell: 8.0, tris: Vec::new(), buckets: HashMap::new() }
    }
    pub fn add(&mut self, t: Tri, owner: &str, key: &str) {
        let i = self.tris.len();
        self.tris.push((t, owner.to_string(), key.to_string()));
        let (mut x0, mut x1, mut z0, mut z1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for v in &t {
            x0 = x0.min(v[0]);
            x1 = x1.max(v[0]);
            z0 = z0.min(v[2]);
            z1 = z1.max(v[2]);
        }
        let (bx0, bx1) = ((x0 / self.cell).floor() as i32, (x1 / self.cell).floor() as i32);
        let (bz0, bz1) = ((z0 / self.cell).floor() as i32, (z1 / self.cell).floor() as i32);
        for bx in bx0..=bx1 {
            for bz in bz0..=bz1 {
                self.buckets.entry((bx, bz)).or_default().push(i);
            }
        }
    }
    pub fn len(&self) -> usize {
        self.tris.len()
    }
    /// The nearest deck triangle the cylinder meets, ignoring the placement
    /// `from` (the tree's own): (owner, triangle y).
    pub fn hit(&self, pos: [f32; 3], radius: f32, ylo: f32, yhi: f32, from: &str) -> Option<(String, f32)> {
        let r2 = radius * radius;
        let (bx0, bx1) = (((pos[0] - radius) / self.cell).floor() as i32, ((pos[0] + radius) / self.cell).floor() as i32);
        let (bz0, bz1) = (((pos[2] - radius) / self.cell).floor() as i32, ((pos[2] + radius) / self.cell).floor() as i32);
        let mut best: Option<(String, f32, f32)> = None;
        for bx in bx0..=bx1 {
            for bz in bz0..=bz1 {
                let Some(list) = self.buckets.get(&(bx, bz)) else { continue };
                for &i in list {
                    let (t, owner, key) = &self.tris[i];
                    if key == from {
                        continue;
                    }
                    let tlo = t[0][1].min(t[1][1]).min(t[2][1]);
                    let thi = t[0][1].max(t[1][1]).max(t[2][1]);
                    if thi < ylo || tlo > yhi {
                        continue;
                    }
                    let d2 = dist2_xz(pos[0], pos[2], t);
                    if d2 <= r2 && best.as_ref().map(|b| d2 < b.2).unwrap_or(true) {
                        best = Some((owner.clone(), tlo, d2));
                    }
                }
            }
        }
        best.map(|(o, y, _)| (o, y))
    }
}

/// Place a deck's triangles into the grid: item space → world by the
/// block's yaw about y and its origin (what `tmmaps tiny` does with the
/// item, `push_veget`'s convention: dir 1 = yaw −π/2 maps local +x onto
/// world +z).
pub fn add_deck(grid: &mut Grid, d: &Deck) {
    let (s, c) = d.yaw.sin_cos();
    let owner = format!("{} ({})", d.name, d.alias);
    for t in d.tris {
        let mut w: Tri = [[0.0; 3]; 3];
        for (k, v) in t.iter().enumerate() {
            w[k] = [d.origin[0] + v[0] * c + v[2] * s, d.origin[1] + v[1], d.origin[2] - v[0] * s + v[2] * c];
        }
        grid.add(w, &owner, &d.key);
    }
}

/// The verdicts: which trees to drop, with what they hit.
pub struct Verdict {
    pub dropped: Vec<(Tree, String, f32)>,
    pub kept: usize,
}

pub fn judge(grid: &Grid, trees: &[Tree]) -> Verdict {
    let mut dropped = Vec::new();
    let mut kept = 0usize;
    for t in trees {
        let ylo = t.pos[1] + 0.5;
        let yhi = t.pos[1] + t.height;
        match grid.hit(t.pos, t.radius, ylo, yhi, &t.from) {
            Some((owner, y)) => dropped.push((t.clone(), owner, y)),
            None => kept += 1,
        }
    }
    Verdict { dropped, kept }
}
