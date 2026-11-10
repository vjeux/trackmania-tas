//! `mapgeom print-stl MAP --out DIR [--mm 210] [--tiles AxB] [--wall-mm 0.8]
//! [--voxel-m N] [--margin-cells 2] [--base-mm 3] [--no-crop] [--no-items]
//! [--no-decoration] [--yoff N] [--render]`: a map as a 3D-PRINTABLE solid.
//!
//! Why (Altrd_Amill101, 2026-11-09): he wants Fall 2026 - 01 as an STL to
//! print and paint. The game's geometry is nothing a slicer can use — the
//! vista terrain is a one-sided skin with nothing under it, many blocks are
//! open shells, and a tree is a cloud of leaf cards. So the map is turned into
//! VOXELS and back into a closed surface:
//!
//! 1. the assembled scene (every block prefab, item and decoration triangle,
//!    `assemble::Assembler`) is rasterised conservatively into a bit grid
//!    (`Grid`), terrain materials into a second grid;
//! 2. every column is filled from the base plate up to its highest terrain or
//!    water voxel ("fill the space to the base below the vista terrain");
//! 3. air the outside cannot reach is filled (closed shells become solid,
//!    "fill blocks that only have one-way geometry");
//! 4. one-voxel sheets are thickened, and the critical configurations that
//!    make a voxel boundary non-manifold (two solids meeting at an edge or a
//!    corner only, and their air duals) are repaired by adding voxels;
//! 5. the boundary is emitted as shared-vertex triangles — watertight,
//!    2-manifold, consistently outward — and written as binary STL in mm,
//!    with the game's x/z footprint on the print's X/Y and height on Z.
//!
//! Trees have no visuals in the dedicated-server packs, but every tree model
//! carries its collision hull — the pines of RedIsland are 400-triangle cones,
//! which is exactly the "approximate visual hitbox" asked for — so a vegetation
//! placement is rasterised from its hull. Bushes (a hull a few cm tall) are
//! skipped: at print scale they are smaller than a voxel.
//!
//! Orientation: TM's world is x, y up, z with the map editor's North = +z and
//! East = -x (the start of Fall 2026 - 01 faces dir 2 = South and its road
//! leaves toward -z; dir 1 = East turns a block's forward from +z to -x, the
//! rotation the car-position fits validated). A right-handed print with
//! X = x, Y = z is therefore the MIRROR of the game's top view, which Altrd
//! saw at once ("we still need to fix the mirroring"). Default: X = -x, so a
//! print seen from above with +Y (north) up has east on the right like the
//! editor; `--mirror` keeps the other chirality.
//!
//! `check_mesh` is the proof: every edge in exactly two triangles with
//! opposite orientation, every vertex a single fan, positive signed volume.
//! Self-intersection is impossible by construction — every face lies on a
//! grid plane and distinct faces share at most an edge.
use std::collections::HashMap;

use crate::assemble::Assembler;
use crate::geom::{apply, Xform};
use crate::place;
use crate::store::DataStore;

// ---------------------------------------------------------------- bit grid

/// A dense bit grid over (i, j, k) = (x, y-up, z) voxels.
#[derive(Clone)]
pub struct Grid {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    bits: Vec<u64>,
}

impl Grid {
    pub fn new(nx: usize, ny: usize, nz: usize) -> Grid {
        let n = nx * ny * nz;
        Grid { nx, ny, nz, bits: vec![0u64; n.div_ceil(64)] }
    }
    #[inline]
    pub fn index(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.ny + j) * self.nx + i
    }
    #[inline]
    pub fn get(&self, i: usize, j: usize, k: usize) -> bool {
        let n = self.index(i, j, k);
        self.bits[n >> 6] >> (n & 63) & 1 == 1
    }
    /// `false` outside the grid.
    #[inline]
    pub fn get_i(&self, i: i64, j: i64, k: i64) -> bool {
        if i < 0 || j < 0 || k < 0 || i >= self.nx as i64 || j >= self.ny as i64 || k >= self.nz as i64 {
            return false;
        }
        self.get(i as usize, j as usize, k as usize)
    }
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, k: usize) {
        let n = self.index(i, j, k);
        self.bits[n >> 6] |= 1u64 << (n & 63);
    }
    #[inline]
    pub fn clear_lin(&mut self, n: usize) {
        self.bits[n >> 6] &= !(1u64 << (n & 63));
    }
    #[inline]
    pub fn clear(&mut self, i: usize, j: usize, k: usize) {
        let n = self.index(i, j, k);
        self.bits[n >> 6] &= !(1u64 << (n & 63));
    }
    #[inline]
    pub fn get_lin(&self, n: usize) -> bool {
        self.bits[n >> 6] >> (n & 63) & 1 == 1
    }
    #[inline]
    pub fn set_lin(&mut self, n: usize) {
        self.bits[n >> 6] |= 1u64 << (n & 63);
    }
    pub fn len(&self) -> usize {
        self.nx * self.ny * self.nz
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn count(&self) -> usize {
        self.bits.iter().map(|w| w.count_ones() as usize).sum()
    }
    pub fn or_with(&mut self, other: &Grid) {
        for (a, b) in self.bits.iter_mut().zip(other.bits.iter()) {
            *a |= *b;
        }
    }
    fn unlin(&self, n: usize) -> (usize, usize, usize) {
        let i = n % self.nx;
        let r = n / self.nx;
        (i, r % self.ny, r / self.ny)
    }
}

// ----------------------------------------------------------- voxelisation

/// Akenine-Möller triangle / axis-aligned box overlap (the 13-axis SAT).
pub fn tri_box_overlap(center: [f32; 3], half: [f32; 3], tri: [[f32; 3]; 3]) -> bool {
    let v: Vec<[f32; 3]> = tri.iter().map(|p| [p[0] - center[0], p[1] - center[1], p[2] - center[2]]).collect();
    // the box's own axes
    for a in 0..3 {
        let lo = v[0][a].min(v[1][a]).min(v[2][a]);
        let hi = v[0][a].max(v[1][a]).max(v[2][a]);
        if lo > half[a] || hi < -half[a] {
            return false;
        }
    }
    let e = [
        [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]],
        [v[2][0] - v[1][0], v[2][1] - v[1][1], v[2][2] - v[1][2]],
        [v[0][0] - v[2][0], v[0][1] - v[2][1], v[0][2] - v[2][2]],
    ];
    // the nine edge cross products
    for ed in &e {
        for a in 0..3 {
            let b = (a + 1) % 3;
            let c = (a + 2) % 3;
            // axis = unit(a) x edge
            let mut axis = [0.0f32; 3];
            axis[b] = -ed[c];
            axis[c] = ed[b];
            let p: Vec<f32> = v.iter().map(|q| q[0] * axis[0] + q[1] * axis[1] + q[2] * axis[2]).collect();
            let r = half[0] * axis[0].abs() + half[1] * axis[1].abs() + half[2] * axis[2].abs();
            let lo = p[0].min(p[1]).min(p[2]);
            let hi = p[0].max(p[1]).max(p[2]);
            if lo > r || hi < -r {
                return false;
            }
        }
    }
    // the triangle's plane
    let n = [e[0][1] * e[1][2] - e[0][2] * e[1][1], e[0][2] * e[1][0] - e[0][0] * e[1][2], e[0][0] * e[1][1] - e[0][1] * e[1][0]];
    let d = -(n[0] * v[0][0] + n[1] * v[0][1] + n[2] * v[0][2]);
    let r = half[0] * n[0].abs() + half[1] * n[1].abs() + half[2] * n[2].abs();
    (-r..=r).contains(&d)
}

/// Mark every voxel the triangle touches. `tri` is in GRID units: voxel
/// (i, j, k) spans `[i, i+1)` on each axis. A sliver of tolerance keeps a
/// surface lying exactly on a voxel face from falling through the gap.
pub fn voxelize_tri(g: &mut Grid, tri: [[f32; 3]; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &tri {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let dims = [g.nx as i64, g.ny as i64, g.nz as i64];
    let mut i0 = [0i64; 3];
    let mut i1 = [0i64; 3];
    for a in 0..3 {
        i0[a] = (lo[a].floor() as i64).max(0);
        i1[a] = (hi[a].floor() as i64).min(dims[a] - 1);
        if i0[a] > i1[a] {
            return;
        }
    }
    let half = [0.5 + 1e-4; 3];
    for k in i0[2]..=i1[2] {
        for j in i0[1]..=i1[1] {
            for i in i0[0]..=i1[0] {
                let c = [i as f32 + 0.5, j as f32 + 0.5, k as f32 + 0.5];
                if tri_box_overlap(c, half, tri) {
                    g.set(i as usize, j as usize, k as usize);
                }
            }
        }
    }
}

// ------------------------------------------------------------------ fills

/// Every column solid from `j = 0` up to its highest voxel set in `top`
/// (the terrain and water skins), plus a base slab of `base` layers
/// everywhere.
pub fn fill_below(solid: &mut Grid, top: &Grid, base: usize) {
    for k in 0..solid.nz {
        for i in 0..solid.nx {
            let mut jmax: Option<usize> = None;
            for j in (0..top.ny).rev() {
                if top.get(i, j, k) {
                    jmax = Some(j);
                    break;
                }
            }
            let upto = jmax.map(|j| j + 1).unwrap_or(0).max(base.min(solid.ny));
            for j in 0..upto {
                solid.set(i, j, k);
            }
        }
    }
}

/// Air the outside cannot reach (6-connected, from every face of the grid)
/// becomes solid: a closed shell fills, a one-sided skin over an enclosed
/// volume fills. Returns how many voxels it added.
pub fn fill_enclosed(solid: &mut Grid) -> usize {
    let n = solid.len();
    let mut outside = Grid::new(solid.nx, solid.ny, solid.nz);
    let mut stack: Vec<u32> = Vec::new();
    let (nx, ny, nz) = (solid.nx, solid.ny, solid.nz);
    let mut push = |i: usize, j: usize, k: usize, outside: &mut Grid, stack: &mut Vec<u32>| {
        let lin = solid.index(i, j, k);
        if !solid.get_lin(lin) && !outside.get_lin(lin) {
            outside.set_lin(lin);
            stack.push(lin as u32);
        }
    };
    for k in 0..nz {
        for j in 0..ny {
            push(0, j, k, &mut outside, &mut stack);
            push(nx - 1, j, k, &mut outside, &mut stack);
        }
        for i in 0..nx {
            push(i, 0, k, &mut outside, &mut stack);
            push(i, ny - 1, k, &mut outside, &mut stack);
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            push(i, j, 0, &mut outside, &mut stack);
            push(i, j, nz - 1, &mut outside, &mut stack);
        }
    }
    while let Some(lin) = stack.pop() {
        let (i, j, k) = solid.unlin(lin as usize);
        let nb = [
            (i as i64 - 1, j as i64, k as i64),
            (i as i64 + 1, j as i64, k as i64),
            (i as i64, j as i64 - 1, k as i64),
            (i as i64, j as i64 + 1, k as i64),
            (i as i64, j as i64, k as i64 - 1),
            (i as i64, j as i64, k as i64 + 1),
        ];
        for (a, b, c) in nb {
            if a < 0 || b < 0 || c < 0 || a >= nx as i64 || b >= ny as i64 || c >= nz as i64 {
                continue;
            }
            let l = solid.index(a as usize, b as usize, c as usize);
            if !solid.get_lin(l) && !outside.get_lin(l) {
                outside.set_lin(l);
                stack.push(l as u32);
            }
        }
    }
    let mut added = 0;
    for lin in 0..n {
        if !solid.get_lin(lin) && !outside.get_lin(lin) {
            solid.set_lin(lin);
            added += 1;
        }
    }
    added
}

/// A solid voxel with air on both sides along TWO axes is a one-voxel stick
/// (a flag pole, a railing post, a tree tip): REMOVE it. Altrd on the 0.4 mm
/// run (2026-11-09): "just cut these, they would likely fail; stacking dots
/// is a bad idea" — and, after a first cut that also took sheets, "only
/// things that are .4 x .4, because they are thin in 2 directions": a
/// one-voxel wall or floor prints. One pass from a snapshot.
pub fn cut_thin(solid: &mut Grid) -> usize {
    let snap = solid.clone();
    let mut removed = 0;
    for k in 0..snap.nz {
        for j in 0..snap.ny {
            for i in 0..snap.nx {
                if !snap.get(i, j, k) {
                    continue;
                }
                let (ii, jj, kk) = (i as i64, j as i64, k as i64);
                let thin_x = !snap.get_i(ii - 1, jj, kk) && !snap.get_i(ii + 1, jj, kk);
                let thin_y = !snap.get_i(ii, jj - 1, kk) && !snap.get_i(ii, jj + 1, kk);
                let thin_z = !snap.get_i(ii, jj, kk - 1) && !snap.get_i(ii, jj, kk + 1);
                // Altrd: "only things that are .4 x .4, because they are thin
                // in 2 directions" — a one-voxel STICK goes, a one-voxel sheet
                // (a wall, a floor, a flag's cloth) prints and stays
                if (thin_x as u8 + thin_y as u8 + thin_z as u8) >= 2 {
                    solid.clear(i, j, k);
                    removed += 1;
                }
            }
        }
    }
    removed
}

/// A solid voxel with air on both sides along an axis is a one-voxel sheet:
/// give it a second layer on the negative side. One pass, read from a
/// snapshot so the pass cannot cascade.
pub fn thicken_sheets(solid: &mut Grid) -> usize {
    let snap = solid.clone();
    let mut added = 0;
    for k in 0..snap.nz {
        for j in 0..snap.ny {
            for i in 0..snap.nx {
                if !snap.get(i, j, k) {
                    continue;
                }
                let (ii, jj, kk) = (i as i64, j as i64, k as i64);
                let pairs = [((ii - 1, jj, kk), (ii + 1, jj, kk)), ((ii, jj - 1, kk), (ii, jj + 1, kk)), ((ii, jj, kk - 1), (ii, jj, kk + 1))];
                for (lo, hi) in pairs {
                    if !snap.get_i(lo.0, lo.1, lo.2) && !snap.get_i(hi.0, hi.1, hi.2) && lo.0 >= 0 && lo.1 >= 0 && lo.2 >= 0 && !solid.get(lo.0 as usize, lo.1 as usize, lo.2 as usize) {
                        solid.set(lo.0 as usize, lo.1 as usize, lo.2 as usize);
                        added += 1;
                    }
                }
            }
        }
    }
    added
}

/// Repair the critical configurations that make a voxel set's boundary
/// non-manifold, by adding voxels until none is left:
/// - in a 2x2 square of any axis plane, solid on one diagonal and air on the
///   other (two solids touching along an edge only — and equally two air
///   cells touching along an edge only);
/// - in a 2x2x2 cube, exactly two solids at opposite corners, or exactly two
///   air cells at opposite corners.
/// Returns (passes, voxels added).
pub fn make_well_composed(solid: &mut Grid) -> (usize, usize) {
    let mut passes = 0;
    let mut total = 0;
    loop {
        passes += 1;
        let mut added = 0;
        // 2x2 squares in the three axis planes
        for k in 0..solid.nz {
            for j in 0..solid.ny {
                for i in 0..solid.nx {
                    // plane xy (k fixed): cells (i,j),(i+1,j),(i,j+1),(i+1,j+1)
                    if i + 1 < solid.nx && j + 1 < solid.ny {
                        let a = solid.get(i, j, k);
                        let b = solid.get(i + 1, j, k);
                        let c = solid.get(i, j + 1, k);
                        let d = solid.get(i + 1, j + 1, k);
                        if a && d && !b && !c {
                            solid.set(i + 1, j, k);
                            added += 1;
                        } else if b && c && !a && !d {
                            solid.set(i, j, k);
                            added += 1;
                        }
                    }
                    // plane xz (j fixed)
                    if i + 1 < solid.nx && k + 1 < solid.nz {
                        let a = solid.get(i, j, k);
                        let b = solid.get(i + 1, j, k);
                        let c = solid.get(i, j, k + 1);
                        let d = solid.get(i + 1, j, k + 1);
                        if a && d && !b && !c {
                            solid.set(i + 1, j, k);
                            added += 1;
                        } else if b && c && !a && !d {
                            solid.set(i, j, k);
                            added += 1;
                        }
                    }
                    // plane yz (i fixed)
                    if j + 1 < solid.ny && k + 1 < solid.nz {
                        let a = solid.get(i, j, k);
                        let b = solid.get(i, j + 1, k);
                        let c = solid.get(i, j, k + 1);
                        let d = solid.get(i, j + 1, k + 1);
                        if a && d && !b && !c {
                            solid.set(i, j + 1, k);
                            added += 1;
                        } else if b && c && !a && !d {
                            solid.set(i, j, k);
                            added += 1;
                        }
                    }
                    // 2x2x2 corner contacts
                    if i + 1 < solid.nx && j + 1 < solid.ny && k + 1 < solid.nz {
                        let cells = [
                            (i, j, k),
                            (i + 1, j, k),
                            (i, j + 1, k),
                            (i + 1, j + 1, k),
                            (i, j, k + 1),
                            (i + 1, j, k + 1),
                            (i, j + 1, k + 1),
                            (i + 1, j + 1, k + 1),
                        ];
                        let s: Vec<bool> = cells.iter().map(|&(a, b, c)| solid.get(a, b, c)).collect();
                        let n = s.iter().filter(|x| **x).count();
                        if n == 2 || n == 6 {
                            // opposite corners: index pairs (0,7),(1,6),(2,5),(3,4)
                            for (p, q) in [(0usize, 7usize), (1, 6), (2, 5), (3, 4)] {
                                let want = n == 2; // the two solids (n==2) or the two air cells (n==6)
                                if s[p] == want && s[q] == want && s.iter().enumerate().all(|(x, v)| x == p || x == q || *v != want) {
                                    // two solids at opposite corners: fill a cell
                                    // face-adjacent to p (bridging them); two AIR cells
                                    // at opposite corners: fill one of the two
                                    let (a, b, c) = if want { cells[p ^ 1] } else { cells[p] };
                                    if !solid.get(a, b, c) {
                                        solid.set(a, b, c);
                                        added += 1;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        total += added;
        if added == 0 || passes > 64 {
            break;
        }
    }
    (passes, total)
}

// ---------------------------------------------------------------- surface

/// Shared-vertex triangles in print millimetres (X = game x, Y = game z,
/// Z = game y).
#[derive(Default, Clone)]
pub struct Mesh {
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    /// The grid corner (i, j, k) each vertex came from — what the puzzle-tab
    /// clearance shrink keys on.
    pub corners: Vec<[u32; 3]>,
}

/// The boundary of the solid voxels inside `[i0, i1) x [k0, k1)` (every
/// column of j), as outward-facing triangles. `mm` is the voxel edge in
/// millimetres; the sub-range's own lower corner is the print origin, so a
/// tile stands on its own plate.
pub fn extract_surface(solid: &Grid, i0: usize, i1: usize, k0: usize, k1: usize, mm: f32) -> Mesh {
    extract_surface_oriented(solid, i0, i1, k0, k1, mm, mm, false)
}

/// `extract_surface` with the chirality choice: `mirror_x` = print X runs
/// along -i (the game's east on the right when north is up).
pub fn extract_surface_oriented(solid: &Grid, i0: usize, i1: usize, k0: usize, k1: usize, mm: f32, mm_y: f32, mirror_x: bool) -> Mesh {
    let mut owned = Grid::new(solid.nx, solid.ny, solid.nz);
    for k in k0..k1 {
        for j in 0..solid.ny {
            for i in i0..i1 {
                if solid.get(i, j, k) {
                    owned.set(i, j, k);
                }
            }
        }
    }
    extract_owned(&owned, i0, i1, k0, mm, mm_y, mirror_x)
}

/// The boundary of the cells set in `owned` (a tile's cells: its range minus
/// the pockets other tiles' tabs cut into it, plus its own tabs), with the
/// print origin at grid corner `(oi, 0, ok)` — or, mirrored, at `(oi1, 0, ok)`
/// with X growing toward smaller i.
pub fn extract_owned(owned: &Grid, oi: usize, oi1: usize, ok: usize, mm: f32, mm_y: f32, mirror_x: bool) -> Mesh {
    let mut mesh = Mesh::default();
    let mut index: HashMap<(u32, u32, u32), u32> = HashMap::new();
    let mut vertex = |i: usize, j: usize, k: usize, mesh: &mut Mesh| -> u32 {
        let key = (i as u32, j as u32, k as u32);
        *index.entry(key).or_insert_with(|| {
            let x = if mirror_x { (oi1 as f32 - i as f32) * mm } else { (i as f32 - oi as f32) * mm };
            // voxels may be flatter than they are wide (Altrd: 0.4 mm terraces
            // on 0.8 mm walls, two print layers per step)
            mesh.verts.push([x, (k as f32 - ok as f32) * mm, j as f32 * mm_y]);
            mesh.corners.push([i as u32, j as u32, k as u32]);
            (mesh.verts.len() - 1) as u32
        })
    };
    let inside = |i: i64, j: i64, k: i64| -> bool { owned.get_i(i, j, k) };
    // For each face: the four corners in an order that is counter-clockwise
    // seen from outside in the GAME's left-handed frame; `quad` reverses the
    // winding for right-handed print space. The single-voxel test's positive
    // volume is the proof the faces point outward.
    for k in 0..owned.nz {
        for j in 0..owned.ny {
            for i in 0..owned.nx {
                if !owned.get(i, j, k) {
                    continue;
                }
                let (ii, jj, kk) = (i as i64, j as i64, k as i64);
                if !inside(ii - 1, jj, kk) {
                    quad(&mut mesh, &mut vertex, [(i, j, k), (i, j, k + 1), (i, j + 1, k + 1), (i, j + 1, k)]);
                }
                if !inside(ii + 1, jj, kk) {
                    quad(&mut mesh, &mut vertex, [(i + 1, j, k), (i + 1, j + 1, k), (i + 1, j + 1, k + 1), (i + 1, j, k + 1)]);
                }
                if !inside(ii, jj, kk - 1) {
                    quad(&mut mesh, &mut vertex, [(i, j, k), (i, j + 1, k), (i + 1, j + 1, k), (i + 1, j, k)]);
                }
                if !inside(ii, jj, kk + 1) {
                    quad(&mut mesh, &mut vertex, [(i, j, k + 1), (i + 1, j, k + 1), (i + 1, j + 1, k + 1), (i, j + 1, k + 1)]);
                }
                if !inside(ii, jj - 1, kk) {
                    quad(&mut mesh, &mut vertex, [(i, j, k), (i + 1, j, k), (i + 1, j, k + 1), (i, j, k + 1)]);
                }
                if !inside(ii, jj + 1, kk) {
                    quad(&mut mesh, &mut vertex, [(i, j + 1, k), (i, j + 1, k + 1), (i + 1, j + 1, k + 1), (i + 1, j + 1, k)]);
                }
            }
        }
    }
    if mirror_x {
        // a mirror flips handedness: turn every triangle back outward
        for t in mesh.tris.iter_mut() {
            t.swap(1, 2);
        }
    }
    mesh
}

// ----------------------------------------------------------- puzzle tabs

/// One jigsaw tab on a seam between two tiles: it belongs to the tile on the
/// `-dir` side of the cut plane and reaches `neck_len + head_len` cells past
/// it; the other tile gets the matching pocket. `axis` 0 = a plane of
/// constant i (the tab runs along i), 2 = constant k. `across` is the
/// centre on the other horizontal axis; the tab occupies layers
/// `0..height` — the bottom of the base, under the roof.
#[derive(Clone, Debug)]
pub struct Tab {
    pub axis: usize,
    pub cut: usize,
    pub across: usize,
    pub neck_w: usize,
    pub head_w: usize,
    pub neck_len: usize,
    pub head_len: usize,
    pub height: usize,
}

impl Tab {
    /// Every cell of the tab, in grid coordinates.
    pub fn cells(&self) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        for d in 0..self.neck_len + self.head_len {
            let w = if d < self.neck_len { self.neck_w } else { self.head_w };
            let lo = self.across.saturating_sub(w / 2);
            for a in lo..lo + w {
                for j in 0..self.height {
                    let along = self.cut + d;
                    out.push(if self.axis == 0 { (along, j, a) } else { (a, j, along) });
                }
            }
        }
        out
    }
    /// The tab's cells minus the socket-side ones: none — the socket IS the
    /// tab's cells, removed from the other tile.
    fn contains_corner(&self, c: [u32; 3]) -> bool {
        let (along, a, j) = if self.axis == 0 { (c[0] as usize, c[2] as usize, c[1] as usize) } else { (c[2] as usize, c[0] as usize, c[1] as usize) };
        if j > self.height || along <= self.cut || along > self.cut + self.neck_len + self.head_len {
            return false;
        }
        let w = self.head_w.max(self.neck_w);
        let lo = self.across.saturating_sub(w / 2);
        a >= lo && a <= lo + w
    }
}

/// Shrink a tile's own tabs by `clearance` mm on every side but the one
/// glued to the tile (the cut plane), so they press-fit the voxel-exact
/// pockets: the tip comes back, the shoulders move out (away from the plane),
/// the flanks move toward the tab's centreline, the top comes down.
pub fn shrink_tabs(mesh: &mut Mesh, tabs: &[Tab], clearance: f32, mm: f32, mm_y: f32, mirror_x: bool) {
    let c = clearance.min(mm * 0.45).min(mm_y * 0.45);
    // every adjustment along print X changes sign when X runs along -i
    let sx = if mirror_x { -1.0f32 } else { 1.0 };
    for (v, corner) in mesh.verts.iter_mut().zip(mesh.corners.iter()) {
        for t in tabs {
            if !t.contains_corner(*corner) {
                continue;
            }
            let (along_idx, across_idx) = if t.axis == 0 { (0usize, 1usize) } else { (1usize, 0usize) };
            // print axes: X = i, Y = k, Z = j
            let along = if t.axis == 0 { corner[0] } else { corner[2] } as usize;
            let across = if t.axis == 0 { corner[2] } else { corner[0] } as usize;
            let j = corner[1] as usize;
            let tip = t.cut + t.neck_len + t.head_len;
            let shoulder = t.cut + t.neck_len;
            let neck_lo = t.across.saturating_sub(t.neck_w / 2);
            let neck_hi = neck_lo + t.neck_w;
            let s_along = if along_idx == 0 { sx } else { 1.0 };
            let s_across = if across_idx == 0 { sx } else { 1.0 };
            if along == tip {
                v[along_idx] -= c * s_along;
            } else if along == shoulder && (across < neck_lo || across > neck_hi) {
                v[along_idx] += c * s_along;
            }
            // flanks: toward the centreline
            let centre = t.across as f32 + if t.head_w % 2 == 0 { 0.0 } else { 0.5 };
            if (across as f32) > centre + 0.25 {
                v[across_idx] -= c * s_across;
            } else if (across as f32) < centre - 0.25 {
                v[across_idx] += c * s_across;
            }
            if j == t.height {
                v[2] -= c;
            }
            break;
        }
    }
}

/// The tabs for a `tx` x `tz` tiling of the grid: two per seam segment, at
/// 30 % and 70 % of the segment, owned by the lower-index tile.
pub fn plan_tabs(nx: usize, nz: usize, tx: usize, tz: usize, voxel_mm: f32, height: usize) -> Vec<Tab> {
    let cells = |mm: f32| ((mm / voxel_mm).round() as usize).max(1);
    let neck_w = cells(6.0).max(3);
    let head_w = (cells(10.0)).max(neck_w + 4);
    let neck_len = cells(2.4).max(2);
    let head_len = cells(5.6).max(3);
    let mut out = Vec::new();
    for a in 1..tx {
        let cut = a * nx / tx;
        for b in 0..tz {
            let k0 = b * nz / tz;
            let k1 = (b + 1) * nz / tz;
            for f in [0.3f32, 0.7] {
                let across = k0 + ((k1 - k0) as f32 * f) as usize;
                if across < k0 + head_w / 2 + 2 || across + head_w / 2 + 2 > k1 {
                    continue;
                }
                out.push(Tab { axis: 0, cut, across, neck_w, head_w, neck_len, head_len, height });
            }
        }
    }
    for b in 1..tz {
        let cut = b * nz / tz;
        for a in 0..tx {
            let i0 = a * nx / tx;
            let i1 = (a + 1) * nx / tx;
            for f in [0.3f32, 0.7] {
                let across = i0 + ((i1 - i0) as f32 * f) as usize;
                if across < i0 + head_w / 2 + 2 || across + head_w / 2 + 2 > i1 {
                    continue;
                }
                out.push(Tab { axis: 2, cut, across, neck_w, head_w, neck_len, head_len, height });
            }
        }
    }
    out
}

/// A tile's owned cells: its range, minus the pockets of tabs that reach
/// into it from neighbours, plus its own tabs. `tile` is (a, b).
pub fn tile_owned(solid: &Grid, i0: usize, i1: usize, k0: usize, k1: usize, tabs: &[Tab]) -> (Grid, Vec<Tab>) {
    let mut owned = Grid::new(solid.nx, solid.ny, solid.nz);
    for k in k0..k1 {
        for j in 0..solid.ny {
            for i in i0..i1 {
                if solid.get(i, j, k) {
                    owned.set(i, j, k);
                }
            }
        }
    }
    let mut mine = Vec::new();
    for t in tabs {
        // the tab's root cell: just on the -dir side of the cut, at `across`
        let root = if t.axis == 0 { (t.cut - 1, t.across) } else { (t.across, t.cut - 1) };
        let owner_is_me = root.0 >= i0 && root.0 < i1 && root.1 >= k0 && root.1 < k1;
        let cells = t.cells();
        let reaches_me = cells.iter().any(|&(i, _, k)| i >= i0 && i < i1 && k >= k0 && k < k1);
        if owner_is_me {
            for (i, j, k) in &cells {
                if solid.get(*i, *j, *k) {
                    owned.set(*i, *j, *k);
                }
            }
            mine.push(t.clone());
        } else if reaches_me {
            // the pocket: clear those cells from this tile
            let mut cleared = Grid::new(1, 1, 1);
            let _ = &mut cleared;
            for (i, j, k) in &cells {
                if i >= &i0 && i < &i1 && k >= &k0 && k < &k1 {
                    owned.clear(*i, *j, *k);
                }
            }
        }
    }
    (owned, mine)
}

fn quad(mesh: &mut Mesh, vertex: &mut impl FnMut(usize, usize, usize, &mut Mesh) -> u32, c: [(usize, usize, usize); 4]) {
    let v: Vec<u32> = c.iter().map(|&(i, j, k)| vertex(i, j, k, mesh)).collect();
    // the corner lists above are counter-clockwise in the GAME's left-handed
    // frame; print space (X, Y, Z) = (x, z, y) is right-handed, so the
    // winding is reversed here — the single-voxel test's positive volume is
    // the proof the faces point outward
    mesh.tris.push([v[0], v[2], v[1]]);
    mesh.tris.push([v[0], v[3], v[2]]);
}

// ------------------------------------------------------------------ check

#[derive(Debug, Default, Clone)]
pub struct MeshReport {
    pub vertices: usize,
    pub triangles: usize,
    pub edges: usize,
    /// Edges used by one triangle only (holes).
    pub boundary_edges: usize,
    /// Edges used by three or more triangles.
    pub overused_edges: usize,
    /// Edges whose two triangles run the same way (an inside-out neighbour).
    pub inconsistent_edges: usize,
    /// Vertices whose incident triangles do not form one fan.
    pub nonmanifold_vertices: usize,
    pub volume_mm3: f64,
    pub area_mm2: f64,
    pub euler: i64,
    pub bounds_mm: ([f32; 3], [f32; 3]),
    /// Up to 16 of the non-manifold vertices (indices), for a debug print.
    pub bad_vertices: Vec<u32>,
}

impl MeshReport {
    pub fn watertight(&self) -> bool {
        self.boundary_edges == 0
    }
    pub fn manifold(&self) -> bool {
        self.overused_edges == 0 && self.nonmanifold_vertices == 0 && self.boundary_edges == 0
    }
    pub fn ok(&self) -> bool {
        self.manifold() && self.inconsistent_edges == 0 && self.volume_mm3 > 0.0
    }
}

pub fn check_mesh(m: &Mesh) -> MeshReport {
    let mut r = MeshReport { vertices: m.verts.len(), triangles: m.tris.len(), ..Default::default() };
    let mut edges: HashMap<(u32, u32), (u32, i32)> = HashMap::with_capacity(m.tris.len() * 3 / 2);
    for t in &m.tris {
        for e in 0..3 {
            let a = t[e];
            let b = t[(e + 1) % 3];
            let key = (a.min(b), a.max(b));
            let dir = if a < b { 1 } else { -1 };
            let ent = edges.entry(key).or_insert((0, 0));
            ent.0 += 1;
            ent.1 += dir;
        }
    }
    r.edges = edges.len();
    for (count, dir) in edges.values() {
        match count {
            1 => r.boundary_edges += 1,
            2 => {
                if *dir != 0 {
                    r.inconsistent_edges += 1;
                }
            }
            _ => r.overused_edges += 1,
        }
    }
    // vertex fans: the triangles around a vertex, linked by shared edges at
    // that vertex, must form ONE component. CSR lists (one flat array, no
    // per-vertex heap boxes): at 0.4 mm a tile has ~3 M vertices and the
    // Vec<Vec<u32>> form took the build box down (2026-11-09).
    let nv = m.verts.len();
    let mut deg = vec![0u32; nv + 1];
    for t in &m.tris {
        for v in t {
            deg[*v as usize + 1] += 1;
        }
    }
    for i in 0..nv {
        deg[i + 1] += deg[i];
    }
    let mut fill = deg.clone();
    let mut incident = vec![0u32; m.tris.len() * 3];
    for (ti, t) in m.tris.iter().enumerate() {
        for v in t {
            let slot = &mut fill[*v as usize];
            incident[*slot as usize] = ti as u32;
            *slot += 1;
        }
    }
    let mut parent: Vec<usize> = Vec::new();
    let mut others: Vec<(u32, usize)> = Vec::new();
    fn find(p: &mut [usize], x: usize) -> usize {
        let mut x = x;
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for v in 0..nv {
        let faces = &incident[deg[v] as usize..deg[v + 1] as usize];
        if faces.len() < 3 {
            if !faces.is_empty() {
                r.nonmanifold_vertices += 1;
            }
            continue;
        }
        parent.clear();
        parent.extend(0..faces.len());
        others.clear();
        for (fi, f) in faces.iter().enumerate() {
            let t = m.tris[*f as usize];
            for w in t {
                if w as usize == v {
                    continue;
                }
                if let Some(&(_, other)) = others.iter().find(|(x, _)| *x == w) {
                    let a = find(&mut parent, other);
                    let b = find(&mut parent, fi);
                    if a != b {
                        parent[a] = b;
                    }
                } else {
                    others.push((w, fi));
                }
            }
        }
        let root0 = find(&mut parent, 0);
        if (1..faces.len()).any(|x| find(&mut parent, x) != root0) {
            r.nonmanifold_vertices += 1;
            if r.bad_vertices.len() < 16 {
                r.bad_vertices.push(v as u32);
            }
        }
    }
    let mut vol = 0.0f64;
    let mut area = 0.0f64;
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &m.verts {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    for t in &m.tris {
        let a = m.verts[t[0] as usize].map(|x| x as f64);
        let b = m.verts[t[1] as usize].map(|x| x as f64);
        let c = m.verts[t[2] as usize].map(|x| x as f64);
        let cr = [b[1] * c[2] - b[2] * c[1], b[2] * c[0] - b[0] * c[2], b[0] * c[1] - b[1] * c[0]];
        vol += (a[0] * cr[0] + a[1] * cr[1] + a[2] * cr[2]) / 6.0;
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        area += (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0;
    }
    r.volume_mm3 = vol;
    r.area_mm2 = area;
    r.euler = m.verts.len() as i64 - r.edges as i64 + m.tris.len() as i64;
    r.bounds_mm = (lo, hi);
    r
}

// -------------------------------------------------------------------- STL

pub fn write_stl(path: &str, m: &Mesh, header: &str) -> Result<(), String> {
    use std::io::Write;
    let f = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    let mut h = header.as_bytes().to_vec();
    h.resize(80, b' ');
    w.write_all(&h).map_err(|e| e.to_string())?;
    w.write_all(&(m.tris.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
    for t in &m.tris {
        let a = m.verts[t[0] as usize];
        let b = m.verts[t[1] as usize];
        let c = m.verts[t[2] as usize];
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let mut n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l > 0.0 {
            n = [n[0] / l, n[1] / l, n[2] / l];
        }
        let mut rec = [0u8; 50];
        let mut o = 0;
        for v in [n, a, b, c] {
            for x in v {
                rec[o..o + 4].copy_from_slice(&x.to_le_bytes());
                o += 4;
            }
        }
        w.write_all(&rec).map_err(|e| e.to_string())?;
    }
    w.flush().map_err(|e| e.to_string())
}

/// Read a binary STL back (triangle soup; vertices NOT merged).
pub fn read_stl(path: &str) -> Result<(usize, Vec<[[f32; 3]; 3]>), String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if b.len() < 84 {
        return Err("too short".into());
    }
    let n = u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize;
    if b.len() != 84 + 50 * n {
        return Err(format!("{} bytes for {n} triangles", b.len()));
    }
    let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let mut tris = Vec::with_capacity(n);
    for t in 0..n {
        let o = 84 + 50 * t + 12;
        tris.push([[f(o), f(o + 4), f(o + 8)], [f(o + 12), f(o + 16), f(o + 20)], [f(o + 24), f(o + 28), f(o + 32)]]);
    }
    Ok((n, tris))
}

// ----------------------------------------------------------------- render

/// A flat-shaded depth-buffered picture of the mesh: `yaw`/`pitch` in degrees
/// about the mesh centre (pitch 90 = straight down), orthographic.
pub fn render_png(m: &Mesh, path: &str, w: usize, h: usize, yaw_deg: f32, pitch_deg: f32) -> Result<(), String> {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &m.verts {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let (sy, cy) = yaw_deg.to_radians().sin_cos();
    let (sp, cp) = pitch_deg.to_radians().sin_cos();
    // camera basis: right, up, forward(into the screen); Z is up in print space
    let right = [cy, sy, 0.0f32];
    let fwd = [-sy * cp, cy * cp, -sp];
    // up = right x fwd: for pitch 35 that is (0, 0.57, 0.82), mostly +Z, so
    // higher things sit higher on the page (the first cut negated it and
    // rendered the map hanging from its base plate)
    let up = [right[1] * fwd[2] - right[2] * fwd[1], right[2] * fwd[0] - right[0] * fwd[2], right[0] * fwd[1] - right[1] * fwd[0]];
    let proj = |p: [f32; 3]| -> [f32; 3] {
        let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
        [d[0] * right[0] + d[1] * right[1] + d[2] * right[2], d[0] * up[0] + d[1] * up[1] + d[2] * up[2], d[0] * fwd[0] + d[1] * fwd[1] + d[2] * fwd[2]]
    };
    let pv: Vec<[f32; 3]> = m.verts.iter().map(|p| proj(*p)).collect();
    let mut plo = [f32::MAX; 2];
    let mut phi = [f32::MIN; 2];
    for p in &pv {
        plo[0] = plo[0].min(p[0]);
        plo[1] = plo[1].min(p[1]);
        phi[0] = phi[0].max(p[0]);
        phi[1] = phi[1].max(p[1]);
    }
    let scale = ((w as f32 - 20.0) / (phi[0] - plo[0]).max(1e-3)).min((h as f32 - 20.0) / (phi[1] - plo[1]).max(1e-3));
    let ox = (w as f32 - (phi[0] - plo[0]) * scale) / 2.0;
    let oy = (h as f32 - (phi[1] - plo[1]) * scale) / 2.0;
    let to_px = |p: [f32; 3]| -> [f32; 3] { [(p[0] - plo[0]) * scale + ox, h as f32 - ((p[1] - plo[1]) * scale + oy), p[2]] };
    let mut img = vec![[235u8, 238, 242]; w * h];
    let mut depth = vec![f32::MAX; w * h];
    let light = {
        // light from the camera's side of the horizon, so an underside view is lit
        let l = if pitch_deg < 0.0 { [-0.4f32, 0.3, -0.86] } else { [-0.4f32, 0.3, 0.86] };
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };
    for t in &m.tris {
        let a = m.verts[t[0] as usize];
        let b = m.verts[t[1] as usize];
        let cc = m.verts[t[2] as usize];
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]];
        let mut n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l == 0.0 {
            continue;
        }
        n = [n[0] / l, n[1] / l, n[2] / l];
        // back-face cull: facing away from the camera
        if n[0] * fwd[0] + n[1] * fwd[1] + n[2] * fwd[2] > 0.0 {
            continue;
        }
        let shade = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0) * 0.75 + 0.25;
        // colour by face direction: tops warm, graded by height so a plan
        // view reads; sides cool; undersides dark
        let zmid = (a[2] + b[2] + cc[2]) / 3.0;
        let hgt = ((zmid - lo[2]) / (hi[2] - lo[2]).max(1e-3)).clamp(0.0, 1.0);
        let base = if n[2] > 0.5 {
            [120.0 + 100.0 * hgt, 150.0 + 60.0 * hgt, 110.0 + 50.0 * hgt]
        } else if n[2] < -0.5 {
            if pitch_deg < 0.0 { [200.0, 190.0, 170.0] } else { [90.0, 90.0, 95.0] }
        } else {
            [150.0, 170.0, 190.0]
        };
        let col = [(base[0] * shade).min(255.0) as u8, (base[1] * shade).min(255.0) as u8, (base[2] * shade).min(255.0) as u8];
        let p0 = to_px(pv[t[0] as usize]);
        let p1 = to_px(pv[t[1] as usize]);
        let p2 = to_px(pv[t[2] as usize]);
        raster(&mut img, &mut depth, w, h, p0, p1, p2, col);
    }
    let mut out = image::RgbImage::new(w as u32, h as u32);
    for (n, px) in img.iter().enumerate() {
        out.put_pixel((n % w) as u32, (n / w) as u32, image::Rgb(*px));
    }
    out.save(path).map_err(|e| format!("{path}: {e}"))
}

#[allow(clippy::too_many_arguments)]
fn raster(img: &mut [[u8; 3]], depth: &mut [f32], w: usize, h: usize, p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], col: [u8; 3]) {
    let minx = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as i64;
    let maxx = p0[0].max(p1[0]).max(p2[0]).ceil().min(w as f32 - 1.0) as i64;
    let miny = p0[1].min(p1[1]).min(p2[1]).floor().max(0.0) as i64;
    let maxy = p0[1].max(p1[1]).max(p2[1]).ceil().min(h as f32 - 1.0) as i64;
    if minx > maxx || miny > maxy {
        return;
    }
    let area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
    if area.abs() < 1e-12 {
        return;
    }
    for y in miny..=maxy {
        for x in minx..=maxx {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = ((p1[0] - px) * (p2[1] - py) - (p2[0] - px) * (p1[1] - py)) / area;
            let w1 = ((p2[0] - px) * (p0[1] - py) - (p0[0] - px) * (p2[1] - py)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < -1e-5 || w1 < -1e-5 || w2 < -1e-5 {
                continue;
            }
            let z = w0 * p0[2] + w1 * p1[2] + w2 * p2[2];
            let n = y as usize * w + x as usize;
            if z < depth[n] {
                depth[n] = z;
                img[n] = col;
            }
        }
    }
}

// ----------------------------------------------------------- the command


/// Is this block part of the landscape (terrain, water, cliffs) rather than
/// something the author built? Decides the crop rectangle only.
/// Items that cannot print at any sane scale: flags (a pole and a sheet of
/// cloth) and the like.
pub fn is_unprintable_item(name: &str) -> bool {
    let stem = name.rsplit('\\').next().unwrap_or(name).trim_end_matches(".Item.Gbx").to_ascii_lowercase();
    stem.starts_with("flag") || stem.contains("sparkler")
}

/// Vegetation items and the Stadium season clumps: scenery, never the track.
pub fn is_veget_name(name: &str) -> bool {
    const P: &[&str] = &["Tree", "Bush", "Palm", "Grove", "Forest", "Ecotone", "Flower", "Grass", "Fern", "Plant", "Cactus", "Spring", "Summer", "Fall", "Winter", "Fir", "Cypress"];
    let stem = name.rsplit('\\').next().unwrap_or(name).trim_end_matches(".Item.Gbx");
    P.iter().any(|p| stem.starts_with(p))
}

pub fn is_terrain_block(name: &str) -> bool {
    const P: &[&str] = &["Water", "Sea", "Land", "Dirt", "Grass", "Sand", "Rock", "Beach", "Lake", "Hill", "Cliff", "Snow", "Shore", "Ocean", "River", "Zone", "DecoTerrain", "Terrain"];
    P.iter().any(|p| name.starts_with(p))
}

pub struct Options {
    pub out_dir: String,
    /// Islands of track cells farther apart than this (in 32 m cells) are
    /// separate; only the start's island sets the crop.
    pub gap_cells: i32,
    /// Debug: write the assembled scene as OBJ here.
    pub obj: Option<String>,
    pub mm: f32,
    pub tiles: (usize, usize),
    pub wall_mm: f32,
    pub voxel_m: Option<f32>,
    pub margin_cells: i32,
    pub base_mm: f32,
    pub crop: bool,
    /// `--box x0,z0,x1,z1` in world metres: print exactly this footprint
    /// (overrides the crop), for a close-up or a single piece.
    pub bbox: Option<[f32; 4]>,
    pub items: bool,
    pub decoration: bool,
    pub yoff: Option<f32>,
    pub render: bool,
    /// Jigsaw tabs in the bottom of the base along every tile seam.
    pub puzzle: bool,
    /// Press-fit clearance per side, mm.
    pub puzzle_clearance: f32,
    /// Keep print X = game x (the mirror of the editor's top view).
    pub mirror: bool,
    /// Vertical voxels are this many times FLATTER than they are wide:
    /// 2 = 0.4 mm terraces on 0.8 mm walls (two 0.2 mm print layers per step).
    pub vertical_factor: f32,
    /// Double one-voxel sheets (a 0.4 mm wall becomes 0.8). Altrd: a single
    /// nozzle width prints, and the doubled flags looked chunky — off by default.
    pub thicken: bool,
    /// Remove one-voxel-thin features instead (default on; `--keep-thin`).
    pub cut_thin: bool,
}

pub fn run(store: &mut DataStore, map_path: &str, o: &Options) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let m = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let yoff = o.yoff.unwrap_or_else(|| tmmaps::map::ground_y(collection));
    std::fs::create_dir_all(&o.out_dir).map_err(|e| format!("{}: {e}", o.out_dir))?;

    // ---- the scene: blocks by the PICKED variant of each placement
    // (`blockmap::walk`), not the assembler's union of every variant a block
    // info names — `StructurePillar` names Pillar_Air2..Air32, and the union
    // drew a 256 m spike on every pillar cell (first Fall 01 run, 2026-11-09).
    // The walk also places the map's BAKED clip fillers. Items and the
    // decoration still come from the assembler.
    let collection_name = match collection {
        15 => "GreenCoast",
        16 => "RedIsland",
        25 | 26 => "Stadium",
        29 => "WhiteShore",
        _ => "BlueBay",
    };
    let (placements, _) = crate::blockmap::walk(store, &m, true, collection_name);
    let mut scene = crate::scene::Scene::default();
    // The DecoWall family is SOLID CONCRETE drawn as skins: the block has no
    // mesh of its own, its sides are the VFC filler faces the game bakes,
    // its top the FCT cover, and a wall in the air stands on
    // DecoWallBasePillar cells that reach the ground. Those zero-thickness
    // faces voxelise to one-voxel shells that the thin-feature cut removes
    // (the first 0.4 mm run carved the stadium's two towers into poles), so
    // they are collected here as well and column-filled like terrain.
    let mut wall_scene = crate::scene::Scene::default();
    // The ground: geometry that came from a TERRAIN BLOCK (Dirt, Water,
    // the cliffs and hills, DecoTerrainHD…), the only thing that fills down
    // to the base plate. The first cut keyed on material instead and a road
    // block's Sand shoulder (OpenTechZone) became a column to the ground —
    // "a column extending down under the techroad block that has a zone of
    // sand on it" (Altrd, 2026-11-10).
    let mut ground_scene = crate::scene::Scene::default();
    // A road variant that carries its own ground — `RoadTechStraightOnDirtCliff4`
    // on Fall 2026 - 01 is a bridge on pillars with a rock outcrop under its
    // start end (Rock + Sand in the prefab, seen in game 2026-11-10) — sits on
    // a plain terrain record at the SAME cell (DirtCliff4 DeadendA, a full
    // 32 m cliff end). The game draws the variant's own cliff; drawing both
    // and column-filling the plain cliff put a solid block under the bridge
    // (Altrd: "a column under the techroad block that has a zone of sand on
    // it"). So: the plain terrain record at an `On<Terrain>` block's cell is
    // skipped, and the variant's terrain-material groups are the ground there.
    let consumed: std::collections::HashSet<[i32; 3]> = placements
        .iter()
        .filter(|p| !p.baked && !p.free && p.name.contains("On") && ["OnDirt", "OnWater", "OnGrass", "OnSand", "OnRock", "OnLand", "OnBeach"].iter().any(|t| p.name.contains(t)))
        .map(|p| p.cell)
        .collect();
    let mut n_consumed = 0usize;
    // the cells the DecoWall family occupies: (cell, is_pillar)
    let mut wall_cells: Vec<([i32; 3], bool)> = Vec::new();
    let mut prefab_cache: HashMap<String, Option<crate::scene::Scene>> = HashMap::new();
    let (mut n_picked, mut n_placeholder, mut n_fallback, mut n_nothing) = (0usize, 0usize, 0usize, 0usize);
    let mut asm = Assembler::new(store);
    asm.with_embedded(&m).ok();
    for p in &placements {
        let (w, d) = {
            let mut mx = 0i32;
            let mut mz = 0i32;
            for (_, c) in &p.cells {
                mx = mx.max(c[0]);
                mz = mz.max(c[2]);
            }
            // the cells are rotated into world; dir 1 and 3 swap the axes
            if p.dir & 1 == 1 {
                ((mz + 1) as f32 * 32.0, (mx + 1) as f32 * 32.0)
            } else {
                ((mx + 1) as f32 * 32.0, (mz + 1) as f32 * 32.0)
            }
        };
        let xf: Xform = if p.free {
            let rec = if p.baked { m.baked.get(p.index) } else { m.blocks.get(p.index) };
            match rec.and_then(|b| b.free_pos.map(|q| (q, b.free_rot.unwrap_or([0.0; 3])))) {
                Some((q, r)) => place::free(q, r),
                None => continue,
            }
        } else {
            place::grid_block((p.cell[0], p.cell[1], p.cell[2]), p.dir, (w, d), yoff)
        };
        if p.name.starts_with("DecoWall") && !p.is_clip && !p.free {
            let pillar = p.name.contains("Pillar");
            for (_, rel) in &p.cells {
                wall_cells.push(([p.cell[0] + rel[0], p.cell[1] + rel[1], p.cell[2] + rel[2]], pillar));
            }
            if p.cells.is_empty() {
                wall_cells.push((p.cell, pillar));
            }
        }
        if let Ok(spec) = std::env::var("MAPGEOM_PRINT_WHO") {
            // MAPGEOM_PRINT_WHO=x,y0,y1,z: every placement whose transformed
            // geometry has a vertex within 1 m of x and z and y in y0..y1
            let v: Vec<f32> = spec.split(',').filter_map(|t| t.trim().parse().ok()).collect();
            if v.len() == 4 {
                let names: Vec<String> = if p.error.is_none() && !p.prefabs.is_empty() { p.prefabs.clone() } else { vec![format!("fallback:{}", p.name)] };
                for pf in &names {
                    let mut c = crate::geom::Collector::new(asm.store);
                    if !pf.starts_with("fallback:") {
                        c.file(pf, &crate::geom::IDENTITY, 0);
                    }
                    let hit = c.scene.groups.values().any(|g| g.verts.iter().any(|q| {
                        let w = crate::geom::apply(&xf, *q);
                        (w[0] - v[0]).abs() < 1.0 && (w[2] - v[3]).abs() < 1.0 && w[1] >= v[1] && w[1] <= v[2]
                    }));
                    if hit {
                        println!("  WHO: {} {} cell {:?} dir {} {} -> {pf}", if p.baked { "baked" } else { "grid" }, p.name, p.cell, p.dir, p.variant_label);
                    }
                }
            }
        }
        if is_terrain_block(&p.name) && consumed.contains(&p.cell) {
            n_consumed += 1;
            continue;
        }
        let carries_ground = ["OnDirt", "OnWater", "OnGrass", "OnSand", "OnRock", "OnLand", "OnBeach"].iter().any(|t| p.name.contains(t));
        if p.error.is_none() && !p.prefabs.is_empty() {
            for pf in &p.prefabs {
                let sc = prefab_cache.entry(pf.clone()).or_insert_with(|| {
                    let mut c = crate::geom::Collector::new(asm.store);
                    c.file(pf, &crate::geom::IDENTITY, 0);
                    if c.scene.tri_count() == 0 {
                        None
                    } else {
                        Some(std::mem::take(&mut c.scene))
                    }
                });
                if let Some(sc) = sc {
                    scene.append(sc, &xf);
                    if pf.contains("DecoWall") {
                        wall_scene.append(sc, &xf);
                    }
                    if is_terrain_block(&p.name) {
                        ground_scene.append(sc, &xf);
                    } else if carries_ground {
                        // only the variant's ground materials fill down
                        let mut g = crate::scene::Scene::default();
                        for (mat, grp) in &sc.groups {
                            if ["Rock", "Sand", "Dirt", "Grass", "Water", "Snow", "Gravel"].iter().any(|t| mat.starts_with(t)) {
                                g.groups.insert(mat.clone(), crate::scene::Group { verts: grp.verts.clone(), tris: grp.tris.clone() });
                            }
                        }
                        ground_scene.append(&g, &xf);
                    }
                }
            }
            n_picked += 1;
        } else if p.path.as_deref().map(|q| q.contains("GameCtnBlockInfoPillar")).unwrap_or(false) && p.prefabs.is_empty() {
            // a pillar cell covered by the taller segment above it
            n_placeholder += 1;
        } else if p.name.starts_with("DecoWall") && p.prefabs.is_empty() {
            // a deco wall block has no mesh of its own: the game bakes its
            // skins as separate records (the FCT top at the block's top face,
            // the VFC sides in the neighbour cells) and those are placed
            // above. The clip fallback drew a SECOND copy of the sloped top
            // at the block's base cell, 16 m too low — "the sloped decowalls
            // are way off" (Altrd, v8). The cell interior is filled by
            // `wall_cells` below.
            n_placeholder += 1;
        } else if p.error.is_none() && p.is_clip && p.prefabs.is_empty() {
            // a CLIP whose picked mobil is the EMPTY one (a VFC record under
            // the 32 m `Middle_Air4` piece that already covers it: lists 20-22
            // of the stack carry variant 4 = "nothing here") draws nothing.
            // Sending these to the clip fallback handed them Air4/Air8
            // templates — the phantom thin walls 40 m above the pillars that
            // Altrd saw on v8 (2026-11-09).
            n_placeholder += 1;
        } else {
            // the walker had nothing: a clip-built block (DecoWallSlope2Straight
            // carries its shape in its clip pieces). Gather the CLIP prefabs
            // only — the block info's own EDClassic refs reach the pillar
            // family, whose Pillar_Air32 is a 256 m template (the Wood spikes
            // of the second Fall 01 run)
            let key = format!("clips:{}", p.name);
            let sc = prefab_cache.entry(key).or_insert_with(|| {
                let mut c = crate::geom::Collector::new(asm.store);
                let path = p.path.clone()?;
                let model = c.store.load_model(&path).ok()?;
                // horizontal clips only: the VERTICAL clips are the fillers
                // between a wall and the ground, and their prefab sets hold
                // every length up to 256 m (the walker places the right one
                // as a baked record already)
                let mut prefabs: Vec<String> = Vec::new();
                for kind in [".EDClip.Gbx", ".EDHorizontalClip.Gbx"] {
                    for clip in model.refs_ending(kind) {
                        if let Ok(cm) = c.store.load_model(&clip) {
                            prefabs.extend(cm.refs_ending(".Prefab.Gbx"));
                        }
                    }
                }
                prefabs.extend(model.refs_ending(".Prefab.Gbx"));
                prefabs.sort();
                prefabs.dedup();
                prefabs.retain(|q| !q.contains("Pillar_Air") && !q.contains("ToDecoPillar"));
                for q in &prefabs {
                    // one prefab at a time, so a length template can be refused
                    let mut one = crate::geom::Collector::new(c.store);
                    one.file(q, &crate::geom::IDENTITY, 0);
                    let tall = one.scene.bounds().map(|(lo, hi)| hi[1] - lo[1] > 64.0).unwrap_or(false);
                    if tall {
                        continue;
                    }
                    c.scene.append(&one.scene, &crate::geom::IDENTITY);
                }
                if c.scene.tri_count() == 0 {
                    None
                } else {
                    Some(std::mem::take(&mut c.scene))
                }
            });
            match sc {
                Some(sc) => {
                    if let Ok(spec) = std::env::var("MAPGEOM_PRINT_WHO") {
                        let v: Vec<f32> = spec.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                        if v.len() == 4 {
                            let hit = sc.groups.values().any(|g| g.verts.iter().any(|q| {
                                let w = crate::geom::apply(&xf, *q);
                                (w[0] - v[0]).abs() < 1.0 && (w[2] - v[3]).abs() < 1.0 && w[1] >= v[1] && w[1] <= v[2]
                            }));
                            if hit {
                                println!("  WHO: FALLBACK {} cell {:?} dir {} ({} triangles)", p.name, p.cell, p.dir, sc.tri_count());
                            }
                        }
                    }
                    if p.name.contains("DecoWall") {
                        wall_scene.append(sc, &xf);
                    }
                    if std::env::var_os("MAPGEOM_PRINT_DEBUG").is_some() {
                        if let Some((lo, hi)) = sc.bounds() {
                            println!("  fallback {} cell {:?}: {} triangles, local y {:.1}..{:.1}", p.name, p.cell, sc.tri_count(), lo[1], hi[1]);
                        }
                    }
                    scene.append(sc, &xf);
                    n_fallback += 1;
                }
                None => n_nothing += 1,
            }
        }
    }
    println!("blocks: {n_picked} placements by their picked variant, {n_placeholder} covered pillar cells, {n_fallback} by the assembler's union, {n_nothing} with no geometry, {n_consumed} terrain records under an On<Terrain> road variant skipped");
    let mut n_skipped_items = 0usize;
    if o.items {
        for it in &m.items {
            // flags print as stacks of gap fill and only some survive the
            // stick cut (Altrd, 2026-11-10) — out, with anything else that
            // is cloth or a pole
            if is_unprintable_item(&it.model) {
                n_skipped_items += 1;
                continue;
            }
            let Some(lm) = asm.item_model(&it.model) else { continue };
            let xf = place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
            scene.append(&lm.scene, &xf);
        }
        if n_skipped_items > 0 {
            println!("items: {n_skipped_items} flag / pole placements skipped as unprintable");
        }
    }
    if o.decoration {
        if let Some((_, d)) = asm.decoration(&m, yoff) {
            scene.append(&d, &crate::geom::IDENTITY);
            ground_scene.append(&d, &crate::geom::IDENTITY);
        }
    }
    // --obj F: the assembled scene before voxelisation (debugging a spike:
    // the material and height of every vertex over the crop)
    if let Some(objp) = rest_flag_obj(o) {
        let (ob, _) = scene.obj(&format!("{}.mtl", objp.trim_end_matches(".obj")));
        std::fs::write(&objp, ob).map_err(|e| format!("{objp}: {e}"))?;
        println!("wrote {objp}");
    }
    let tris_in = scene.tri_count();
    let Some((slo, shi)) = scene.bounds() else { return Err("the map assembled to no geometry".into()) };
    println!("scene: {tris_in} triangles, bounds x {:.0}..{:.0} y {:.0}..{:.0} z {:.0}..{:.0} (yoff {yoff})", slo[0], shi[0], slo[1], shi[1], slo[2], shi[2]);

    // ---- the trees: hulls of the vegetation items the scene has nothing for
    let mut tree_tris: Vec<[[f32; 3]; 3]> = Vec::new();
    let mut tree_count = 0usize;
    let mut skipped_low = 0usize;
    if o.items {
        let mut hull_cache: HashMap<String, Option<(Vec<[f32; 3]>, Vec<[u32; 3]>, f32)>> = HashMap::new();
        let mut item_cache: HashMap<String, Option<Vec<String>>> = HashMap::new();
        for it in &m.items {
            if asm.item_model(&it.model).is_some() {
                continue; // has geometry already
            }
            let models = item_cache.entry(it.model.clone()).or_insert_with(|| veget_models(asm.store, &it.model)).clone();
            let Some(models) = models else { continue };
            if models.is_empty() {
                continue;
            }
            let v = (it.variant() as usize).min(models.len() - 1);
            let path = &models[v];
            let hull = hull_cache
                .entry(path.clone())
                .or_insert_with(|| match crate::veget::parse_tree_model(asm.store, path) {
                    Ok(t) => {
                        let top = t.hull_vertices.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
                        let bottom = t.hull_vertices.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
                        Some((t.hull_vertices.clone(), t.hull_triangles.iter().map(|(t, _)| *t).collect(), top - bottom))
                    }
                    Err(_) => None,
                })
                .clone();
            let Some((hv, _ht, height)) = hull else { continue };
            if height < 1.0 {
                skipped_low += 1;
                continue;
            }
            let xf: Xform = place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
            // The hull is the TRUNK (RedIsland's TreePineBigA1: 792 vertices,
            // 0.5 m wide) — rasterised it printed as a stack of single
            // voxels (Altrd: "stacking dots is a bad idea"). What he asked for
            // is the tree's visual volume: a cone for a conifer, a ball on a
            // trunk for anything else, sized from the hull's height and
            // footprint (closed meshes, so the enclosed fill makes them solid).
            let mut lo = [f32::MAX; 3];
            let mut hi = [f32::MIN; 3];
            for v in &hv {
                for a in 0..3 {
                    lo[a] = lo[a].min(v[a]);
                    hi[a] = hi[a].max(v[a]);
                }
            }
            let radius = (hi[0] - lo[0]).max(hi[2] - lo[2]) / 2.0;
            let conifer = {
                let n = path.to_ascii_lowercase();
                n.contains("pine") || n.contains("fir") || n.contains("cypress") || n.contains("spruce")
            };
            let cx = (lo[0] + hi[0]) / 2.0;
            let cz = (lo[2] + hi[2]) / 2.0;
            for t in tree_volume(conifer, [cx, lo[1].max(0.0), cz], radius.max(0.8), hi[1]) {
                tree_tris.push([apply(&xf, t[0]), apply(&xf, t[1]), apply(&xf, t[2])]);
            }
            tree_count += 1;
        }
        println!("trees: {tree_count} vegetation placements rasterised from their collision hulls ({} triangles); {skipped_low} ground-cover placements under 1 m skipped", tree_tris.len());
    }

    // ---- the crop rectangle: the TRACK's blocks, plus a margin.
    // Terrain, deco-terrain and vegetation never count; the remaining blocks
    // and items are grouped into islands on the 32 m cell grid, and an island
    // farther than `gap` cells from the island holding the start is an
    // outlier (a stand or a screen the author parked far off to one side) and
    // is dropped — Altrd's crop v2 (2026-11-09): "greater than a 2 block gap
    // in between, it's not included", which bought a big step in resolution.
    let (mut x0, mut x1, mut z0, mut z1) = (slo[0], shi[0], slo[2], shi[2]);
    if o.crop {
        let gap = o.gap_cells.max(1);
        let mut cells: std::collections::HashSet<(i32, i32)> = Default::default();
        let mut start: Option<(i32, i32)> = None;
        for b in &m.blocks {
            if is_terrain_block(&b.name) || is_veget_name(&b.name) {
                continue;
            }
            let (cx, cz) = if b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0 {
                match b.free_pos {
                    Some(p) => ((p[0] / 32.0).floor() as i32, (p[2] / 32.0).floor() as i32),
                    None => continue,
                }
            } else {
                let c = b.coords();
                (c.0, c.2)
            };
            cells.insert((cx, cz));
            if b.name.contains("Start") && start.is_none() {
                start = Some((cx, cz));
            }
        }
        for it in &m.items {
            if is_veget_name(&it.model) {
                continue;
            }
            cells.insert(((it.pos[0] / 32.0).floor() as i32, (it.pos[2] / 32.0).floor() as i32));
        }
        // islands: cells within `gap` (Chebyshev) of each other are one island
        let list: Vec<(i32, i32)> = cells.iter().copied().collect();
        let mut island = vec![usize::MAX; list.len()];
        let mut n_islands = 0;
        for i in 0..list.len() {
            if island[i] != usize::MAX {
                continue;
            }
            let id = n_islands;
            n_islands += 1;
            let mut stack = vec![i];
            island[i] = id;
            while let Some(a) = stack.pop() {
                for b in 0..list.len() {
                    if island[b] == usize::MAX && (list[a].0 - list[b].0).abs() <= gap && (list[a].1 - list[b].1).abs() <= gap {
                        island[b] = id;
                        stack.push(b);
                    }
                }
            }
        }
        // the main island: the start's, else the biggest
        let mut sizes = vec![0usize; n_islands];
        for id in &island {
            sizes[*id] += 1;
        }
        let main = start
            .and_then(|s| list.iter().position(|c| *c == s).map(|i| island[i]))
            .unwrap_or_else(|| (0..n_islands).max_by_key(|id| sizes[*id]).unwrap_or(0));
        let mut lo = [f32::MAX; 2];
        let mut hi = [f32::MIN; 2];
        let mut n = 0;
        let mut dropped = 0;
        for (i, c) in list.iter().enumerate() {
            if island[i] != main {
                dropped += 1;
                continue;
            }
            lo[0] = lo[0].min(c.0 as f32 * 32.0);
            lo[1] = lo[1].min(c.1 as f32 * 32.0);
            hi[0] = hi[0].max(c.0 as f32 * 32.0 + 32.0);
            hi[1] = hi[1].max(c.1 as f32 * 32.0 + 32.0);
            n += 1;
        }
        println!("crop: {} track cells in {n_islands} island(s) (gap {gap} cells); main island {n} cells, {dropped} outlier cells dropped", list.len());
        if n > 0 {
            let margin = o.margin_cells as f32 * 32.0;
            // snap to whole cells so the cut runs along block seams
            x0 = ((lo[0] - margin) / 32.0).floor().max(0.0) * 32.0;
            z0 = ((lo[1] - margin) / 32.0).floor().max(0.0) * 32.0;
            x1 = (((hi[0] + margin) / 32.0).ceil() * 32.0).min(shi[0]);
            z1 = (((hi[1] + margin) / 32.0).ceil() * 32.0).min(shi[2]);
            println!("crop: the track spans x {:.0}..{:.0} z {:.0}..{:.0}; with {} cells of margin the print covers x {x0:.0}..{x1:.0} z {z0:.0}..{z1:.0} ({:.0} x {:.0} m)", lo[0], hi[0], lo[1], hi[1], o.margin_cells, x1 - x0, z1 - z0);
        }
    }

    if let Some(b) = o.bbox {
        x0 = b[0];
        z0 = b[1];
        x1 = b[2];
        z1 = b[3];
        println!("box: x {x0:.0}..{x1:.0} z {z0:.0}..{z1:.0} ({:.0} x {:.0} m)", x1 - x0, z1 - z0);
    }

    // ---- scale and voxel size
    let (tx, tz) = o.tiles;
    let width_m = x1 - x0;
    let depth_m = z1 - z0;
    let mm_per_m = (o.mm * tx as f32 / width_m).min(o.mm * tz as f32 / depth_m);
    let voxel_m = o.voxel_m.unwrap_or(o.wall_mm / mm_per_m);
    let voxel_mm = voxel_m * mm_per_m;
    let vf = o.vertical_factor.max(1.0);
    let voxel_y_m = voxel_m / vf;
    let voxel_y_mm = voxel_mm / vf;
    // puzzle tabs live in the lower half of the base: 4.8 mm of base gives a
    // 2.4 mm tab under a 2.4 mm roof
    let base_mm = if o.puzzle && tx * tz > 1 { o.base_mm.max(4.8) } else { o.base_mm };
    let base_vox = (base_mm / voxel_y_mm).ceil().max(1.0) as usize;
    let y_lo = slo[1] - base_vox as f32 * voxel_y_m;
    let nx = ((x1 - x0) / voxel_m).ceil() as usize;
    let nz = ((z1 - z0) / voxel_m).ceil() as usize;
    let ny = ((shi[1] - y_lo) / voxel_y_m).ceil() as usize + 1;
    println!("scale: {mm_per_m:.4} mm per metre ({:.1} mm per block); voxel {voxel_m:.3} x {voxel_y_m:.3} m = {voxel_mm:.3} mm wide x {voxel_y_mm:.3} mm tall; grid {nx} x {ny} x {nz} = {:.1} M voxels; base {base_vox} layers", 32.0 * mm_per_m, (nx * ny * nz) as f64 / 1e6);
    if nx * ny * nz > u32::MAX as usize {
        return Err("grid too large for the 32-bit fill stack: raise --voxel-m or --wall-mm".into());
    }

    let stem = std::path::Path::new(map_path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default().replace(".Map.Gbx", "");
    // --render-scene: the assembled TRIANGLES over the crop, drawn with the
    // same camera as the voxel renders — what the voxel pass should look
    // like; a difference between the two is a fill or thickness artefact
    if o.render && std::env::var_os("MAPGEOM_PRINT_SCENE").is_some() {
        let mirror_x = !o.mirror;
        let mut sm = Mesh::default();
        let (wx, wz) = ((x1 - x0) * mm_per_m, (z1 - z0) * mm_per_m);
        let conv = |v: [f32; 3]| -> [f32; 3] {
            let px = if mirror_x { (x1 - v[0]) * mm_per_m } else { (v[0] - x0) * mm_per_m };
            [px, (v[2] - z0) * mm_per_m, (v[1] - y_lo) * mm_per_m]
        };
        for (_, g) in &scene.groups {
            for t in &g.tris {
                let ps = [conv(g.verts[t[0] as usize]), conv(g.verts[t[1] as usize]), conv(g.verts[t[2] as usize])];
                if !ps.iter().all(|p| p[0] >= 0.0 && p[0] <= wx && p[1] >= 0.0 && p[1] <= wz) {
                    continue;
                }
                let base = sm.verts.len() as u32;
                sm.verts.extend_from_slice(&ps);
                // both windings: the scene's triangles are not consistently oriented
                sm.tris.push([base, base + 1, base + 2]);
                sm.tris.push([base, base + 2, base + 1]);
            }
        }
        for t in &tree_tris {
            let ps = [conv(t[0]), conv(t[1]), conv(t[2])];
            if !ps.iter().all(|p| p[0] >= 0.0 && p[0] <= wx && p[1] >= 0.0 && p[1] <= wz) {
                continue;
            }
            let base = sm.verts.len() as u32;
            sm.verts.extend_from_slice(&ps);
            sm.tris.push([base, base + 1, base + 2]);
            sm.tris.push([base, base + 2, base + 1]);
        }
        for (name, yaw, pitch) in [("scene-iso-sw", 35.0f32, 35.0f32), ("scene-iso-ne", 215.0, 30.0)] {
            let p = format!("{}/{stem}-{name}.png", o.out_dir);
            render_png(&sm, &p, 1600, 1200, yaw, pitch)?;
            println!("rendered {p}");
        }
    }

    // ---- voxelise
    let mut solid = Grid::new(nx, ny, nz);
    let mut terrain = Grid::new(nx, ny, nz);
    let to_grid = |p: [f32; 3]| -> [f32; 3] { [(p[0] - x0) / voxel_m, (p[1] - y_lo) / voxel_y_m, (p[2] - z0) / voxel_m] };
    let mut rastered = 0usize;
    for (_, g) in &scene.groups {
        for t in &g.tris {
            let tri = [to_grid(g.verts[t[0] as usize]), to_grid(g.verts[t[1] as usize]), to_grid(g.verts[t[2] as usize])];
            // skip triangles wholly outside the crop
            if tri.iter().all(|p| p[0] < 0.0) || tri.iter().all(|p| p[0] >= nx as f32) || tri.iter().all(|p| p[2] < 0.0) || tri.iter().all(|p| p[2] >= nz as f32) {
                continue;
            }
            voxelize_tri(&mut solid, tri);
            rastered += 1;
        }
    }
    for (_, g) in &ground_scene.groups {
        for t in &g.tris {
            let tri = [to_grid(g.verts[t[0] as usize]), to_grid(g.verts[t[1] as usize]), to_grid(g.verts[t[2] as usize])];
            if tri.iter().all(|p| p[0] < 0.0) || tri.iter().all(|p| p[0] >= nx as f32) || tri.iter().all(|p| p[2] < 0.0) || tri.iter().all(|p| p[2] >= nz as f32) {
                continue;
            }
            voxelize_tri(&mut terrain, tri);
        }
    }
    // Deco walls are solid INSIDE THEIR OWN CELLS: a pillar cell is a full
    // 32 x 8 x 32 m block (its skins sit in the neighbouring cells), a wall
    // block fills each column of its cell up to its top face (the sloped
    // FCT). Not "to the ground": a pillar segment between two stacked
    // platforms ends at the platform below it — the first cut filled a
    // column from 80 m straight down through two air gaps.
    let mut wall = Grid::new(nx, ny, nz);
    let mut wall_tris = 0usize;
    for (_, g) in &wall_scene.groups {
        for t in &g.tris {
            let tri = [to_grid(g.verts[t[0] as usize]), to_grid(g.verts[t[1] as usize]), to_grid(g.verts[t[2] as usize])];
            voxelize_tri(&mut wall, tri);
            wall_tris += 1;
        }
    }
    let mut wall_filled = 0usize;
    for (c, pillar) in &wall_cells {
        let gi0 = (((c[0] as f32) * 32.0 - x0) / voxel_m).floor().max(0.0) as usize;
        let gi1 = ((((c[0] + 1) as f32) * 32.0 - x0) / voxel_m).ceil().clamp(0.0, nx as f32) as usize;
        let gk0 = (((c[2] as f32) * 32.0 - z0) / voxel_m).floor().max(0.0) as usize;
        let gk1 = ((((c[2] + 1) as f32) * 32.0 - z0) / voxel_m).ceil().clamp(0.0, nz as f32) as usize;
        let gj0 = (((c[1] as f32) * 8.0 + yoff - y_lo) / voxel_y_m).floor().max(0.0) as usize;
        let gj1 = ((((c[1] + 1) as f32) * 8.0 + yoff - y_lo) / voxel_y_m).ceil().clamp(0.0, ny as f32) as usize;
        if gi0 >= gi1 || gk0 >= gk1 || gj0 >= gj1 {
            continue;
        }
        for k in gk0..gk1 {
            for i in gi0..gi1 {
                let top = (gj0..gj1).rev().find(|j| wall.get(i, *j, k));
                let upto = match (top, pillar) {
                    (Some(j), _) => j + 1,
                    (None, true) => gj1,
                    (None, false) => continue,
                };
                for j in gj0..upto {
                    if !solid.get(i, j, k) {
                        solid.set(i, j, k);
                        wall_filled += 1;
                    }
                }
            }
        }
    }
    if !wall_cells.is_empty() {
        println!("deco walls: {} cells ({} pillar), {wall_tris} skin triangles, {wall_filled} voxels filled inside them", wall_cells.len(), wall_cells.iter().filter(|(_, p)| *p).count());
    }
    for t in &tree_tris {
        let tri = [to_grid(t[0]), to_grid(t[1]), to_grid(t[2])];
        voxelize_tri(&mut solid, tri);
        // a tree stands on the ground: its hull goes into the column-fill
        // grid too, so the cone comes out SOLID (the hulls are open at the
        // base, so the enclosed-air fill cannot close them, and a one-voxel
        // shell is exactly what the thin-feature cut removes — the 0.4 mm
        // run lost its pines that way)
        voxelize_tri(&mut terrain, tri);
    }
    println!("voxelised {rastered} scene triangles + {} tree triangles: {} surface voxels ({} terrain) in {:.1}s", tree_tris.len(), solid.count(), terrain.count(), t0.elapsed().as_secs_f32());

    // ---- fills
    fill_below(&mut solid, &terrain, base_vox);
    let after_columns = solid.count();
    let enclosed = fill_enclosed(&mut solid);
    let cut = if o.cut_thin && !o.thicken { cut_thin(&mut solid) } else { 0 };
    let sheets = if o.thicken { thicken_sheets(&mut solid) } else { 0 };
    let (passes, repaired) = make_well_composed(&mut solid);
    println!("filled: {after_columns} after the column fill, +{enclosed} enclosed, -{cut} one-voxel-thin cut, +{sheets} sheet thickening, +{repaired} manifold repairs in {passes} pass(es) -> {} solid voxels ({:.1}s)", solid.count(), t0.elapsed().as_secs_f32());

    // MAPGEOM_PRINT_COLUMN=x,z[;x,z…]: the solid runs of those world columns,
    // in metres — what the model has at a spot, for comparing against the
    // block records there
    if let Ok(spec) = std::env::var("MAPGEOM_PRINT_COLUMN") {
        for xz in spec.split(';') {
            let v: Vec<f32> = xz.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if v.len() != 2 {
                continue;
            }
            let i = ((v[0] - x0) / voxel_m).floor() as i64;
            let k = ((v[1] - z0) / voxel_m).floor() as i64;
            let mut runs: Vec<(f32, f32)> = Vec::new();
            let mut start: Option<usize> = None;
            for j in 0..=ny {
                let on = j < ny && solid.get_i(i, j as i64, k);
                match (on, start) {
                    (true, None) => start = Some(j),
                    (false, Some(s0)) => {
                        runs.push((y_lo + s0 as f32 * voxel_y_m, y_lo + j as f32 * voxel_y_m));
                        start = None;
                    }
                    _ => {}
                }
            }
            println!("  column x {:.0} z {:.0} (voxel {i},{k}): solid y {}", v[0], v[1], runs.iter().map(|(a, b)| format!("{a:.1}..{b:.1}")).collect::<Vec<_>>().join(", "));
        }
    }

    // ---- printability: overhangs, bridges, floating islands, thin walls —
    // what a slicer will want supports for (Altrd: "double check that
    // everything is reasonably printable with sufficient support")
    {
        let mut unsupported = 0usize; // solid with air below (an overhang face)
        let mut bridged = 0usize; // … whose nearest solid below is > 3 mm down
        let mut max_drop_mm = 0.0f32;
        let mut n_islands = 0usize;
        let mut island_cells = 0usize;
        // 26-connected islands not touching the base layer
        let mut seen = Grid::new(nx, ny, nz);
        let mut stack: Vec<u32> = Vec::new();
        for lin in 0..solid.len() {
            if !solid.get_lin(lin) || seen.get_lin(lin) {
                continue;
            }
            seen.set_lin(lin);
            stack.push(lin as u32);
            let mut cells = 0usize;
            let mut grounded = false;
            while let Some(l) = stack.pop() {
                cells += 1;
                let (i, j, k) = solid.unlin(l as usize);
                if j == 0 {
                    grounded = true;
                }
                for dk in -1i64..=1 {
                    for dj in -1i64..=1 {
                        for di in -1i64..=1 {
                            let (a, b, c) = (i as i64 + di, j as i64 + dj, k as i64 + dk);
                            if a < 0 || b < 0 || c < 0 || a >= nx as i64 || b >= ny as i64 || c >= nz as i64 {
                                continue;
                            }
                            let m = solid.index(a as usize, b as usize, c as usize);
                            if solid.get_lin(m) && !seen.get_lin(m) {
                                seen.set_lin(m);
                                stack.push(m as u32);
                            }
                        }
                    }
                }
            }
            if !grounded {
                n_islands += 1;
                island_cells += cells;
                // a floating piece prints as a blob on the bed or falls off
                // (Fall 01: the ShowSpeakers hanging under the stage truss,
                // 12-34 voxels each) — REMOVE it, the slicer cannot save it
                let (i, j, k) = solid.unlin(lin);
                println!("  floating island removed: {cells} voxels near x {:.0} y {:.0} z {:.0} m", x0 + i as f32 * voxel_m, y_lo + j as f32 * voxel_y_m, z0 + k as f32 * voxel_m);
                // second pass over the same island to clear it
                let mut clear_stack = vec![lin as u32];
                let mut cleared = Grid::new(nx, ny, nz);
                cleared.set_lin(lin);
                while let Some(l) = clear_stack.pop() {
                    solid.clear_lin(l as usize);
                    let (i, j, k) = solid.unlin(l as usize);
                    for dk in -1i64..=1 {
                        for dj in -1i64..=1 {
                            for di in -1i64..=1 {
                                let (a, b, c) = (i as i64 + di, j as i64 + dj, k as i64 + dk);
                                if a < 0 || b < 0 || c < 0 || a >= nx as i64 || b >= ny as i64 || c >= nz as i64 {
                                    continue;
                                }
                                let m = solid.index(a as usize, b as usize, c as usize);
                                if solid.get_lin(m) && !cleared.get_lin(m) {
                                    cleared.set_lin(m);
                                    clear_stack.push(m as u32);
                                }
                            }
                        }
                    }
                }
            }
        }
        for k in 0..nz {
            for i in 0..nx {
                for j in 1..ny {
                    if solid.get(i, j, k) && !solid.get(i, j - 1, k) {
                        unsupported += 1;
                        let mut d = 1;
                        while j >= d + 1 && !solid.get(i, j - 1 - d, k) {
                            d += 1;
                        }
                        let drop = d as f32 * voxel_y_mm;
                        if drop > 3.0 {
                            bridged += 1;
                        }
                        max_drop_mm = max_drop_mm.max(drop);
                    }
                }
            }
        }
        // where are the big overhangs: the >3 mm faces by 16 m height band
        let mut bands: std::collections::BTreeMap<i32, usize> = Default::default();
        for k in 0..nz {
            for i in 0..nx {
                for j in 1..ny {
                    if solid.get(i, j, k) && !solid.get(i, j - 1, k) {
                        let mut d = 1;
                        while j >= d + 1 && !solid.get(i, j - 1 - d, k) {
                            d += 1;
                        }
                        if d as f32 * voxel_y_mm > 3.0 {
                            *bands.entry(((y_lo + j as f32 * voxel_y_m) / 16.0).floor() as i32).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
        let face_mm2 = voxel_mm * voxel_mm;
        for (b, n) in &bands {
            println!("  overhang faces >3 mm at y {}..{} m: {:.1} cm2", b * 16, b * 16 + 16, *n as f32 * face_mm2 / 100.0);
        }
        println!(
            "printability: {} floating island(s) ({} voxels) not connected to the base; {:.0} cm2 of downward faces over air, {:.0} cm2 of them more than 3 mm above the next solid (bridges/overhangs needing support); tallest drop {:.1} mm",
            n_islands,
            island_cells,
            unsupported as f32 * face_mm2 / 100.0,
            bridged as f32 * face_mm2 / 100.0,
            max_drop_mm
        );
    }

    // MAPGEOM_PRINT_COLCENSUS=1: per 32 m cell, the share of columns that
    // are solid without a break from the base to more than 6 m above the
    // TERRAIN top of that column — pillars are ~1 % of a cell, a deco-wall
    // cell 100 %; anything in between is a column that should not be there
    // (Altrd's "column under the techroad block", 2026-11-10)
    if std::env::var_os("MAPGEOM_PRINT_COLCENSUS").is_some() {
        let cells_x = (nx as f32 * voxel_m / 32.0).ceil() as usize;
        let cells_z = (nz as f32 * voxel_m / 32.0).ceil() as usize;
        let mut rows: Vec<(f32, usize, usize, usize, usize, f32)> = Vec::new();
        for cz in 0..cells_z {
            for cx in 0..cells_x {
                let i0 = (cx as f32 * 32.0 / voxel_m) as usize;
                let i1 = (((cx + 1) as f32 * 32.0 / voxel_m) as usize).min(nx);
                let k0 = (cz as f32 * 32.0 / voxel_m) as usize;
                let k1 = (((cz + 1) as f32 * 32.0 / voxel_m) as usize).min(nz);
                let (mut n, mut tall) = (0usize, 0usize);
                let mut top_max = 0.0f32;
                for k in k0..k1 {
                    for i in i0..i1 {
                        n += 1;
                        // continuous run from the base
                        let mut j = 0;
                        while j < ny && solid.get(i, j, k) {
                            j += 1;
                        }
                        // terrain top
                        let mut tj = 0;
                        for jj in (0..ny).rev() {
                            if terrain.get(i, jj, k) {
                                tj = jj + 1;
                                break;
                            }
                        }
                        if (j as f32 - tj as f32) * voxel_y_m > 6.0 {
                            tall += 1;
                            top_max = top_max.max(y_lo + j as f32 * voxel_y_m);
                        }
                    }
                }
                if n > 0 && tall > 0 {
                    let share = tall as f32 / n as f32;
                    rows.push((share, cx, cz, tall, n, top_max));
                }
            }
        }
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        println!("column census: {} cells have columns rising >6 m above their terrain from the base", rows.len());
        for (share, cx, cz, tall, n, top) in rows.iter().filter(|r| r.0 > 0.03 && r.0 < 0.9) {
            let wx = x0 + *cx as f32 * 32.0;
            let wz = z0 + *cz as f32 * 32.0;
            println!("  cell x {:.0}..{:.0} z {:.0}..{:.0} (grid cell {},{}): {:.0}% of columns ({}/{}), top {:.0} m", wx, wx + 32.0, wz, wz + 32.0, (wx / 32.0) as i32, (wz / 32.0) as i32, share * 100.0, tall, n, top);
        }
    }

    // MAPGEOM_PRINT_TALL=N: the N tallest columns, in world metres — what the
    // spikes in a render are (then `mapgeom where MAP --at x,z` names them)
    if let Some(n) = std::env::var("MAPGEOM_PRINT_TALL").ok().and_then(|v| v.parse::<usize>().ok()) {
        let mut tops: Vec<(usize, usize, usize)> = Vec::new();
        for k in 0..nz {
            for i in 0..nx {
                for j in (0..ny).rev() {
                    if solid.get(i, j, k) {
                        tops.push((j, i, k));
                        break;
                    }
                }
            }
        }
        tops.sort_by(|a, b| b.cmp(a));
        for (j, i, k) in tops.iter().take(n) {
            println!("  tall column: top y {:.0} m at x {:.0} z {:.0} (voxel {i},{j},{k})", y_lo + (*j as f32 + 1.0) * voxel_y_m, x0 + (*i as f32 + 0.5) * voxel_m, z0 + (*k as f32 + 0.5) * voxel_m);
        }
    }

    // ---- surfaces: the whole, then the tiles

    let header = format!("mapgeom print-stl {stem} units=mm voxel={voxel_mm:.3}x{voxel_y_mm:.3}mm scale={mm_per_m:.4}mm/m");
    // one mesh at a time: write, check, read back, render, DROP — the 0.4 mm
    // 2x2 run (210 M voxels, 9 M triangles for the whole alone) ran the build
    // box out of memory when every mesh stayed alive until the end
    let mut all_ok = true;
    let mut finish = |path: &str, mesh: Mesh, renders: &[(&str, f32, f32)]| -> Result<(), String> {
        let rep = check_mesh(&mesh);
        write_stl(path, &mesh, &header)?;
        let (lo, hi) = rep.bounds_mm;
        println!(
            "{path}: {} triangles, {} vertices, {:.1} x {:.1} x {:.1} mm, volume {:.1} cm3; watertight {} manifold {} oriented {} (boundary {}, overused {}, inconsistent {}, nonmanifold vertices {}, euler {})",
            rep.triangles,
            rep.vertices,
            hi[0] - lo[0],
            hi[1] - lo[1],
            hi[2] - lo[2],
            rep.volume_mm3 / 1000.0,
            rep.watertight(),
            rep.manifold(),
            rep.inconsistent_edges == 0 && rep.volume_mm3 > 0.0,
            rep.boundary_edges,
            rep.overused_edges,
            rep.inconsistent_edges,
            rep.nonmanifold_vertices,
            rep.euler
        );
        all_ok &= rep.ok();
        if !rep.bad_vertices.is_empty() && std::env::var_os("MAPGEOM_PRINT_DEBUG").is_some() {
            for v in &rep.bad_vertices {
                let p = mesh.verts[*v as usize];
                let (ci, ck, cj) = ((p[0] / voxel_mm).round() as i64, (p[1] / voxel_mm).round() as i64, (p[2] / voxel_y_mm).round() as i64);
                let mut pat = String::new();
                for dk in -1..=0 {
                    for dj in -1..=0 {
                        for di in -1..=0 {
                            pat.push(if solid.get_i(ci + di, cj + dj, ck + dk) { '#' } else { '.' });
                        }
                        pat.push(' ');
                    }
                    pat.push('|');
                }
                println!("  non-manifold vertex at corner ({ci},{cj},{ck}) mm {:?}: cells {pat}", p);
            }
        }
        for (name, yaw, pitch) in renders {
            let p = format!("{}-{name}.png", path.trim_end_matches(".stl"));
            render_png(&mesh, &p, 1600, 1200, *yaw, *pitch)?;
            println!("rendered {p}");
        }
        let n_tris = mesh.tris.len();
        drop(mesh);
        // the FILE, not the memory: read the STL back as a triangle soup,
        // merge vertices on exact coordinates, check again
        let (n_file, soup) = read_stl(path)?;
        let mut re = Mesh::default();
        let mut idx: HashMap<[u32; 3], u32> = HashMap::with_capacity(n_file / 2);
        for t in &soup {
            let mut ids = [0u32; 3];
            for (k, v) in t.iter().enumerate() {
                let key = [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()];
                ids[k] = *idx.entry(key).or_insert_with(|| {
                    re.verts.push(*v);
                    (re.verts.len() - 1) as u32
                });
            }
            re.tris.push(ids);
        }
        drop(soup);
        drop(idx);
        let rep2 = check_mesh(&re);
        println!("  read back from the file: {n_file} triangles, watertight {} manifold {} volume {:.1} cm3", rep2.watertight(), rep2.manifold(), rep2.volume_mm3 / 1000.0);
        if n_file != n_tris || !rep2.ok() || (rep2.volume_mm3 - rep.volume_mm3).abs() > 1e-3 * rep.volume_mm3.abs().max(1.0) {
            println!("{path}: READ-BACK FAILED");
            all_ok = false;
        }
        Ok(())
    };
    let whole_renders: &[(&str, f32, f32)] = if o.render { &[("top", 0.0, 89.9), ("iso-sw", 35.0, 35.0), ("iso-ne", 215.0, 30.0), ("side", 90.0, 12.0)] } else { &[] };
    let tile_renders: &[(&str, f32, f32)] = if o.render && o.puzzle { &[("bottom", 35.0, -40.0)] } else { &[] };
    // default chirality: X = -x (east on the right with north up, as in the editor)
    let mirror_x = !o.mirror;
    println!("orientation: print X = {} game x, print Y = game z (north), Z = height{}", if mirror_x { "-" } else { "+" }, if mirror_x { " — the editor's top view" } else { " — MIRRORED versus the editor (--mirror)" });
    let whole = extract_surface_oriented(&solid, 0, nx, 0, nz, voxel_mm, voxel_y_mm, mirror_x);
    finish(&format!("{}/{stem}.stl", o.out_dir), whole, whole_renders)?;
    if tx * tz > 1 {
        let tabs = if o.puzzle { plan_tabs(nx, nz, tx, tz, voxel_mm, (base_vox / 2).max(2)) } else { Vec::new() };
        if o.puzzle {
            let t0 = tabs.first();
            println!(
                "puzzle: {} tabs on the seams, each {} x {} mm neck / {} mm head, {:.1} mm tall under a {:.1} mm roof, {:.2} mm clearance per side",
                tabs.len(),
                t0.map(|t| t.neck_w as f32 * voxel_mm).unwrap_or(0.0),
                t0.map(|t| (t.neck_len + t.head_len) as f32 * voxel_mm).unwrap_or(0.0),
                t0.map(|t| t.head_w as f32 * voxel_mm).unwrap_or(0.0),
                t0.map(|t| t.height as f32 * voxel_y_mm).unwrap_or(0.0),
                (base_vox - (base_vox / 2).max(2)) as f32 * voxel_y_mm,
                o.puzzle_clearance
            );
        }
        for a in 0..tx {
            for b in 0..tz {
                let i0 = a * nx / tx;
                let i1 = (a + 1) * nx / tx;
                let k0 = b * nz / tz;
                let k1 = (b + 1) * nz / tz;
                let (owned, mine) = tile_owned(&solid, i0, i1, k0, k1, &tabs);
                let mut tile = extract_owned(&owned, i0, i1, k0, voxel_mm, voxel_y_mm, mirror_x);
                drop(owned);
                if !mine.is_empty() {
                    shrink_tabs(&mut tile, &mine, o.puzzle_clearance, voxel_mm, voxel_y_mm, mirror_x);
                }
                // tile names follow the PRINT layout: x0 is the leftmost tile
                // seen from above with north up
                let pa = if mirror_x { tx - 1 - a } else { a };
                finish(&format!("{}/{stem}-tile-x{pa}-y{b}.stl", o.out_dir), tile, tile_renders)?;
            }
        }
        // close-up of the first seam's tab + pocket: the two tiles' cells within
        // a box around it, extracted on their own (cheap; no tile mesh kept)
        if o.render && o.puzzle {
            if let Some(t) = tabs.first() {
                let r = ((15.0 / voxel_mm).ceil() as usize).max(4);
                let rj = ((12.0 / voxel_y_mm).ceil() as usize).min(ny);
                let (ci, ck) = if t.axis == 0 { (t.cut, t.across) } else { (t.across, t.cut) };
                let mut close = Mesh::default();
                for (a, b) in [(0usize, 0usize), (1, 0), (0, 1), (1, 1)] {
                    if a >= tx || b >= tz {
                        continue;
                    }
                    let i0 = a * nx / tx;
                    let i1 = (a + 1) * nx / tx;
                    let k0 = b * nz / tz;
                    let k1 = (b + 1) * nz / tz;
                    let (owned, mine) = tile_owned(&solid, i0, i1, k0, k1, &tabs);
                    // keep only the cells in the box
                    let mut part = Grid::new(owned.nx, owned.ny, owned.nz);
                    let (bi0, bi1) = (ci.saturating_sub(r), (ci + r).min(nx));
                    let (bk0, bk1) = (ck.saturating_sub(r), (ck + r).min(nz));
                    let mut any = false;
                    for k in bk0..bk1 {
                        for j in 0..rj {
                            for i in bi0..bi1 {
                                if owned.get(i, j, k) {
                                    part.set(i, j, k);
                                    any = true;
                                }
                            }
                        }
                    }
                    if !any {
                        continue;
                    }
                    let mut pm = extract_owned(&part, i0, i1, k0, voxel_mm, voxel_y_mm, mirror_x);
                    if !mine.is_empty() {
                        shrink_tabs(&mut pm, &mine, o.puzzle_clearance, voxel_mm, voxel_y_mm, mirror_x);
                    }
                    // place the tile in whole-grid print space, pulled 1.5 mm off the seam
                    let pa = if mirror_x { tx - 1 - a } else { a };
                    let ox = if mirror_x { (nx - i1) as f32 * voxel_mm } else { i0 as f32 * voxel_mm };
                    let oy = k0 as f32 * voxel_mm;
                    let sep = if t.axis == 0 { if pa == 0 { -1.5 } else { 1.5 } } else { 0.0 };
                    let sepy = if t.axis == 2 { if b == 0 { -1.5 } else { 1.5 } } else { 0.0 };
                    let base = close.verts.len() as u32;
                    close.verts.extend(pm.verts.iter().map(|p| [p[0] + ox + sep, p[1] + oy + sepy, p[2]]));
                    close.tris.extend(pm.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
                }
                if !close.tris.is_empty() {
                    let p = format!("{}/{stem}-puzzle-closeup.png", o.out_dir);
                    render_png(&close, &p, 1600, 1200, 25.0, -35.0)?;
                    println!("rendered {p}");
                }
            }
        }
    }
    println!("done in {:.1}s; every mesh passed: {all_ok}", t0.elapsed().as_secs_f32());
    if !all_ok {
        return Err("a mesh failed the watertight/manifold check".into());
    }
    Ok(())
}

fn rest_flag_obj(o: &Options) -> Option<String> {
    o.obj.clone()
}

/// A closed tree solid in the item's own frame: a cone (base radius `r` at
/// 15 % of the height, apex at `top`) over a stub, or a ball of radius `r`
/// on a trunk 0.4 m wide, 16 segments.
pub fn tree_volume(conifer: bool, base: [f32; 3], r: f32, top: f32) -> Vec<[[f32; 3]; 3]> {
    let n = 16usize;
    let ring = |y: f32, rad: f32| -> Vec<[f32; 3]> { (0..n).map(|i| {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        [base[0] + rad * a.cos(), y, base[2] + rad * a.sin()]
    }).collect() };
    let mut out = Vec::new();
    // a closed band between two rings
    let band = |out: &mut Vec<[[f32; 3]; 3]>, a: &[[f32; 3]], b: &[[f32; 3]]| {
        for i in 0..n {
            let j = (i + 1) % n;
            out.push([a[i], a[j], b[j]]);
            out.push([a[i], b[j], b[i]]);
        }
    };
    // a cap: a fan to the centre
    let cap = |out: &mut Vec<[[f32; 3]; 3]>, ring: &[[f32; 3]], centre: [f32; 3]| {
        for i in 0..n {
            let j = (i + 1) % n;
            out.push([ring[i], ring[j], centre]);
        }
    };
    let h = top - base[1];
    if conifer {
        let r_trunk = (0.35 * r).max(0.3);
        let y_crown = base[1] + 0.15 * h;
        let trunk_lo = ring(base[1], r_trunk);
        let trunk_hi = ring(y_crown, r_trunk);
        let crown_lo = ring(y_crown, r);
        let apex = [base[0], top, base[2]];
        cap(&mut out, &trunk_lo, base);
        band(&mut out, &trunk_lo, &trunk_hi);
        band(&mut out, &trunk_hi, &crown_lo);
        cap(&mut out, &crown_lo, apex);
    } else {
        let r_trunk = 0.2f32.max(0.15 * r);
        let yc = base[1] + 0.62 * h;
        let ry = 0.38 * h;
        let trunk_lo = ring(base[1], r_trunk);
        let trunk_hi = ring(yc - ry * 0.7, r_trunk);
        cap(&mut out, &trunk_lo, base);
        band(&mut out, &trunk_lo, &trunk_hi);
        // the ball as stacked rings
        let steps = 8;
        let mut prev = trunk_hi.clone();
        for k in 0..=steps {
            let t = -1.0 + 2.0 * k as f32 / steps as f32; // -1..1
            let y = yc + ry * t;
            let rad = (r * (1.0 - t * t).max(0.0).sqrt()).max(0.05);
            let cur = ring(y, rad);
            band(&mut out, &prev, &cur);
            prev = cur;
        }
        cap(&mut out, &prev, [base[0], yc + ry, base[2]]);
    }
    out
}

/// `mapgeom stl-diff A.stl B.stl [--cell 2]`: where two prints of the same
/// map differ. Both are read as triangle soups, a height map (max Z per
/// `cell` mm square) is built for each, and the cells whose heights differ
/// by more than 1 mm are clustered and listed with their print coordinates
/// and the game-world position behind them when `--origin x0,z0 --mm-per-m
/// S [--mirror-width W]` is given. The tool that answered "where was the
/// column you removed" on Altrd's v10 -> v11 (2026-11-10).
pub fn stl_diff(rest: &[String]) -> Result<(), String> {
    let flag = |n: &str| rest.iter().position(|a| a == n).and_then(|i| rest.get(i + 1)).cloned();
    let files: Vec<&String> = rest.iter().skip(1).filter(|a| !a.starts_with("--")).take(2).collect();
    if files.len() != 2 {
        return Err("stl-diff A.stl B.stl".into());
    }
    let cell: f32 = flag("--cell").and_then(|v| v.parse().ok()).unwrap_or(2.0);
    let (_, a) = read_stl(files[0])?;
    let (_, b) = read_stl(files[1])?;
    let mut lo = [f32::MAX; 2];
    let mut hi = [f32::MIN; 2];
    for t in a.iter().chain(b.iter()) {
        for v in t {
            lo[0] = lo[0].min(v[0]);
            lo[1] = lo[1].min(v[1]);
            hi[0] = hi[0].max(v[0]);
            hi[1] = hi[1].max(v[1]);
        }
    }
    let nx = ((hi[0] - lo[0]) / cell).ceil() as usize + 1;
    let ny = ((hi[1] - lo[1]) / cell).ceil() as usize + 1;
    let heights = |tris: &[[[f32; 3]; 3]]| -> Vec<f32> {
        let mut h = vec![f32::MIN; nx * ny];
        for t in tris {
            // the triangle's footprint cells get its max Z (coarse, enough for columns)
            let zmax = t[0][2].max(t[1][2]).max(t[2][2]);
            let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for v in t {
                x0 = x0.min(v[0]);
                x1 = x1.max(v[0]);
                y0 = y0.min(v[1]);
                y1 = y1.max(v[1]);
            }
            // only small triangles (a voxel face) count: a big face spanning cells would smear
            if x1 - x0 > cell * 2.0 || y1 - y0 > cell * 2.0 {
                continue;
            }
            let cx = ((x0 + x1) / 2.0 - lo[0]) / cell;
            let cy = ((y0 + y1) / 2.0 - lo[1]) / cell;
            let (i, j) = (cx.floor() as usize, cy.floor() as usize);
            if i < nx && j < ny {
                let k = j * nx + i;
                h[k] = h[k].max(zmax);
            }
        }
        h
    };
    let ha = heights(&a);
    let hb = heights(&b);
    let origin: Option<Vec<f32>> = flag("--origin").map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect());
    let mm_per_m: Option<f32> = flag("--mm-per-m").and_then(|v| v.parse().ok());
    let mirror_w: Option<f32> = flag("--mirror-width").and_then(|v| v.parse().ok());
    let mut diffs: Vec<(usize, usize, f32, f32)> = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let (x, y) = (ha[j * nx + i], hb[j * nx + i]);
            if x == f32::MIN || y == f32::MIN {
                continue;
            }
            if (x - y).abs() > 1.0 {
                diffs.push((i, j, x, y));
            }
        }
    }
    println!("{} of {} cells differ by more than 1 mm in height ({} mm cells)", diffs.len(), nx * ny, cell);
    // cluster by 8-neighbourhood
    let mut seen = vec![false; diffs.len()];
    let idx: HashMap<(usize, usize), usize> = diffs.iter().enumerate().map(|(n, d)| ((d.0, d.1), n)).collect();
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for n in 0..diffs.len() {
        if seen[n] {
            continue;
        }
        let mut stack = vec![n];
        seen[n] = true;
        let mut members = Vec::new();
        while let Some(m) = stack.pop() {
            members.push(m);
            let (i, j, _, _) = diffs[m];
            for di in -1i64..=1 {
                for dj in -1i64..=1 {
                    let key = ((i as i64 + di) as usize, (j as i64 + dj) as usize);
                    if let Some(&o) = idx.get(&key) {
                        if !seen[o] {
                            seen[o] = true;
                            stack.push(o);
                        }
                    }
                }
            }
        }
        clusters.push(members);
    }
    clusters.sort_by_key(|c| std::cmp::Reverse(c.len()));
    for c in clusters.iter().take(12) {
        let (mut sx, mut sy, mut da, mut db) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for &m in c {
            let (i, j, x, y) = diffs[m];
            sx += lo[0] + (i as f32 + 0.5) * cell;
            sy += lo[1] + (j as f32 + 0.5) * cell;
            da += x;
            db += y;
        }
        let n = c.len() as f32;
        let (px, py) = (sx / n, sy / n);
        let world = match (&origin, mm_per_m) {
            (Some(o), Some(s)) if o.len() == 2 => {
                let gx = match mirror_w {
                    Some(w) => o[0] + (w - px) / s,
                    None => o[0] + px / s,
                };
                format!(" = game x {:.0} z {:.0}", gx, o[1] + py / s)
            }
            _ => String::new(),
        };
        println!("  cluster of {} cells at print ({:.1}, {:.1}) mm{world}: A top {:.1} mm, B top {:.1} mm", c.len(), px, py, da / n, db / n);
    }
    Ok(())
}

/// The VegetTreeModel paths of a vegetation item, in VARIANT order (the
/// placement's variant byte indexes them). `None` when the item is not a
/// variant-list item.
fn veget_models(store: &mut DataStore, name: &str) -> Option<Vec<String>> {
    let base = name.trim_end_matches(".Item.Gbx");
    let mut candidates = vec![format!("Stadium\\Items\\{base}.Item.Gbx"), format!("{base}.Item.Gbx")];
    for env in ["RedIsland", "GreenCoast", "BlueBay", "WhiteShore", "Stadium"] {
        candidates.push(format!("{env}\\Items\\Vegetation\\{base}.Item.Gbx"));
        candidates.push(format!("{env}\\Items\\{base}.Item.Gbx"));
    }
    let path = candidates.into_iter().find(|p| store.resolve(p).is_some())?;
    let model = store.load_model(&path).ok()?;
    let mut lb = crate::static_item::LookbackState::default();
    lb.defined_nodes.extend(model.external_indices().iter().copied());
    let mut r = crate::static_item::Rd::new(&model.body, 0, lb);
    let item = crate::static_item::item::CGameItemModel::parse(&mut r).ok()?;
    let externals: HashMap<u32, String> = model.externals.iter().map(|(i, e)| (*i, e.clone())).collect();
    for c in &item.chunks {
        let crate::static_item::item::ItemChunk::Model(mc) = c else { continue };
        let Some(crate::static_item::Node::VariantList(vl)) = mc.entity_model.inline.as_deref() else { continue };
        let mut out = Vec::new();
        for v in &vl.variants {
            if v.model.index >= 0 {
                if let Some(p) = externals.get(&(v.model.index as u32)) {
                    out.push(p.clone());
                    continue;
                }
            }
            out.push(String::new());
        }
        return Some(out.into_iter().filter(|p| p.to_ascii_lowercase().ends_with(".vegettreemodel.gbx")).collect());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_tris(lo: [f32; 3], hi: [f32; 3]) -> Vec<[[f32; 3]; 3]> {
        let p = |x: usize, y: usize, z: usize| [if x == 0 { lo[0] } else { hi[0] }, if y == 0 { lo[1] } else { hi[1] }, if z == 0 { lo[2] } else { hi[2] }];
        let faces: [[(usize, usize, usize); 4]; 6] = [
            [(0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0)],
            [(1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)],
            [(0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)],
            [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)],
            [(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)],
            [(0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0)],
        ];
        let mut out = Vec::new();
        for f in faces {
            let v: Vec<[f32; 3]> = f.iter().map(|&(x, y, z)| p(x, y, z)).collect();
            out.push([v[0], v[1], v[2]]);
            out.push([v[0], v[2], v[3]]);
        }
        out
    }

    #[test]
    fn single_voxel_surface_is_a_closed_oriented_cube() {
        let mut g = Grid::new(3, 3, 3);
        g.set(1, 1, 1);
        let m = extract_surface(&g, 0, 3, 0, 3, 2.0);
        assert_eq!(m.tris.len(), 12);
        assert_eq!(m.verts.len(), 8);
        let r = check_mesh(&m);
        assert!(r.ok(), "{r:?}");
        assert!((r.volume_mm3 - 8.0).abs() < 1e-6, "volume {}", r.volume_mm3);
        assert!((r.area_mm2 - 24.0).abs() < 1e-6);
        assert_eq!(r.euler, 2);
    }

    #[test]
    fn voxelize_marks_a_shell_and_the_fill_closes_it() {
        // a 6-voxel cube shell in a 10^3 grid: conservative voxelisation of a
        // closed box leaves its interior hollow; fill_enclosed fills it
        let mut g = Grid::new(10, 10, 10);
        for t in cube_tris([2.0, 2.0, 2.0], [8.0, 8.0, 8.0]) {
            voxelize_tri(&mut g, t);
        }
        let shell = g.count();
        // a face lying exactly on a cell boundary marks the cell whose range
        // contains it (the triangle's floor-based range never reaches the
        // neighbour), so the shell is one cell thick over cells 2..=8:
        // 7^3 - 5^3
        assert_eq!(shell, 7 * 7 * 7 - 5 * 5 * 5, "shell {shell}");
        let added = fill_enclosed(&mut g);
        assert_eq!(added, 5 * 5 * 5);
        let m = extract_surface(&g, 0, 10, 0, 10, 1.0);
        let r = check_mesh(&m);
        assert!(r.ok(), "{r:?}");
        assert!((r.volume_mm3 - 343.0).abs() < 1e-3);
    }

    #[test]
    fn column_fill_reaches_the_base_and_lays_a_slab() {
        let mut solid = Grid::new(4, 6, 4);
        let mut terrain = Grid::new(4, 6, 4);
        terrain.set(1, 3, 1); // one terrain voxel at height 3
        fill_below(&mut solid, &terrain, 2);
        // column (1,1): layers 0..=3 ; every other column: the 2-layer slab
        assert!(solid.get(1, 0, 1) && solid.get(1, 3, 1) && !solid.get(1, 4, 1));
        assert!(solid.get(0, 1, 0) && !solid.get(0, 2, 0));
        assert_eq!(solid.count(), 16 * 2 + 2);
    }

    #[test]
    fn sheet_thickening_and_manifold_repair() {
        // an edge-only contact: (0,0,0) and (1,1,0) solid in a 2x2x1 slab
        let mut g = Grid::new(3, 3, 3);
        g.set(0, 0, 0);
        g.set(1, 1, 0);
        let m = extract_surface(&g, 0, 3, 0, 3, 1.0);
        let r = check_mesh(&m);
        assert!(r.overused_edges > 0, "the edge contact must read as non-manifold: {r:?}");
        let (_, added) = make_well_composed(&mut g);
        assert!(added >= 1);
        let m = extract_surface(&g, 0, 3, 0, 3, 1.0);
        let r = check_mesh(&m);
        assert!(r.ok(), "{r:?}");
        // a lone sheet gets a second layer below it
        let mut s = Grid::new(3, 3, 3);
        s.set(1, 1, 1);
        let n = thicken_sheets(&mut s);
        assert!(n >= 1 && s.get(0, 1, 1) && s.get(1, 0, 1) && s.get(1, 1, 0));
    }

    #[test]
    fn corner_contact_is_repaired() {
        let mut g = Grid::new(2, 2, 2);
        g.set(0, 0, 0);
        g.set(1, 1, 1);
        let (_, added) = make_well_composed(&mut g);
        assert!(added >= 1);
        let m = extract_surface(&g, 0, 2, 0, 2, 1.0);
        assert!(check_mesh(&m).ok());
    }

    #[test]
    fn air_corner_contact_is_repaired() {
        // six solids, air at two opposite corners of a 2x2x2 block (the pattern
        // the first Fall 01 run left 25 of)
        let mut g = Grid::new(2, 2, 2);
        for (i, j, k) in [(1, 0, 0), (0, 1, 0), (1, 1, 0), (0, 0, 1), (1, 0, 1), (0, 1, 1)] {
            g.set(i, j, k);
        }
        let m = extract_surface(&g, 0, 2, 0, 2, 1.0);
        assert!(check_mesh(&m).nonmanifold_vertices > 0);
        let (_, added) = make_well_composed(&mut g);
        assert!(added >= 1);
        let m = extract_surface(&g, 0, 2, 0, 2, 1.0);
        let r = check_mesh(&m);
        assert!(r.ok(), "{r:?}");
    }

    #[test]
    fn tiles_are_closed_on_their_cut_faces() {
        let mut g = Grid::new(8, 3, 8);
        for i in 0..8 {
            for k in 0..8 {
                g.set(i, 0, k);
                g.set(i, 1, k);
            }
        }
        let left = extract_surface(&g, 0, 4, 0, 8, 1.0);
        let right = extract_surface(&g, 4, 8, 0, 8, 1.0);
        let (rl, rr) = (check_mesh(&left), check_mesh(&right));
        assert!(rl.ok() && rr.ok(), "{rl:?} {rr:?}");
        assert!((rl.volume_mm3 + rr.volume_mm3 - 128.0).abs() < 1e-3);
        // a tile stands on its own origin
        assert!(left.verts.iter().all(|v| v[0] >= 0.0 && v[0] <= 4.0) && right.verts.iter().all(|v| v[0] >= 0.0 && v[0] <= 4.0));
    }

    #[test]
    fn puzzle_tab_and_pocket_fit() {
        // a 2-tile slab, 6 layers thick; one tab on the seam
        let (nx, ny, nz) = (40usize, 8usize, 60usize);
        let mut g = Grid::new(nx, ny, nz);
        for k in 0..nz {
            for i in 0..nx {
                for j in 0..6 {
                    g.set(i, j, k);
                }
            }
        }
        let tabs = plan_tabs(nx, nz, 2, 1, 1.0, 3);
        assert!(!tabs.is_empty(), "no tab fitted");
        let (owned_a, mine_a) = tile_owned(&g, 0, 20, 0, nz, &tabs);
        let (owned_b, mine_b) = tile_owned(&g, 20, 40, 0, nz, &tabs);
        assert_eq!(mine_a.len(), tabs.len());
        assert!(mine_b.is_empty());
        let tab_cells: usize = tabs.iter().map(|t| t.cells().len()).sum();
        assert_eq!(owned_a.count(), 20 * nz * 6 + tab_cells);
        assert_eq!(owned_b.count(), 20 * nz * 6 - tab_cells);
        let mut a = extract_owned(&owned_a, 0, 20, 0, 1.0, 1.0, false);
        let b = extract_owned(&owned_b, 20, 40, 0, 1.0, 1.0, false);
        let (ra, rb) = (check_mesh(&a), check_mesh(&b));
        assert!(ra.ok() && rb.ok(), "{ra:?} {rb:?}");
        assert!((ra.volume_mm3 + rb.volume_mm3 - (nx * nz * 6) as f64).abs() < 1e-3);
        // the tab reaches past the tile's own width
        assert!(a.verts.iter().any(|v| v[0] > 20.0));
        let vol_before = ra.volume_mm3;
        shrink_tabs(&mut a, &mine_a, 0.2, 1.0, 1.0, false);
        let ra2 = check_mesh(&a);
        assert!(ra2.ok(), "{ra2:?}");
        assert!(ra2.volume_mm3 < vol_before && ra2.volume_mm3 > vol_before - tab_cells as f64);
        // nothing of the tile body moved
        assert!(a.verts.iter().zip(a.corners.iter()).all(|(v, c)| c[0] > 20 || (v[0] - c[0] as f32).abs() < 1e-6));
    }

    #[test]
    fn mirrored_extraction_is_outward_and_flipped() {
        // two voxels at i=0 and i=2 in a 3-wide grid; mirrored, the i=0 one
        // sits at the HIGH X end and the mesh still has positive volume
        let mut g = Grid::new(3, 1, 1);
        g.set(0, 0, 0);
        g.set(2, 0, 0);
        let plain = extract_surface_oriented(&g, 0, 3, 0, 1, 1.0, 1.0, false);
        let mirrored = extract_surface_oriented(&g, 0, 3, 0, 1, 1.0, 1.0, true);
        let (rp, rm) = (check_mesh(&plain), check_mesh(&mirrored));
        assert!(rp.ok() && rm.ok(), "{rp:?} {rm:?}");
        assert!((rp.volume_mm3 - 2.0).abs() < 1e-6 && (rm.volume_mm3 - 2.0).abs() < 1e-6);
        // the i=0 voxel's vertices: X in 0..1 plain, 2..3 mirrored
        let xs_plain: Vec<f32> = plain.verts.iter().zip(plain.corners.iter()).filter(|(_, c)| c[0] <= 1).map(|(v, _)| v[0]).collect();
        let xs_mirr: Vec<f32> = mirrored.verts.iter().zip(mirrored.corners.iter()).filter(|(_, c)| c[0] <= 1).map(|(v, _)| v[0]).collect();
        assert!(xs_plain.iter().all(|x| *x >= 0.0 && *x <= 1.0));
        assert!(xs_mirr.iter().all(|x| *x >= 2.0 && *x <= 3.0), "{xs_mirr:?}");
        // a mirrored tile with a tab still shrinks toward its own body
        let (nx, nz) = (40usize, 60usize);
        let mut g = Grid::new(nx, 8, nz);
        for k in 0..nz {
            for i in 0..nx {
                for j in 0..6 {
                    g.set(i, j, k);
                }
            }
        }
        let tabs = plan_tabs(nx, nz, 2, 1, 1.0, 3);
        let (owned_a, mine_a) = tile_owned(&g, 0, 20, 0, nz, &tabs);
        let mut a = extract_owned(&owned_a, 0, 20, 0, 1.0, 1.0, true);
        let before = check_mesh(&a).volume_mm3;
        shrink_tabs(&mut a, &mine_a, 0.2, 1.0, 1.0, true);
        let after = check_mesh(&a);
        assert!(after.ok() && after.volume_mm3 < before, "{after:?}");
        // mirrored: the tab (i > 20) lies at X < 0, beyond the tile's own 0..20
        assert!(a.verts.iter().zip(a.corners.iter()).filter(|(_, c)| c[0] > 20).all(|(v, _)| v[0] < 0.0));
    }

    #[test]
    fn flat_voxels_keep_volume_and_manifoldness() {
        // 0.8 mm wide, 0.4 mm tall: a 3-layer stack of one column is 0.8 x 0.8 x 1.2 mm
        let mut g = Grid::new(3, 5, 3);
        for j in 0..3 {
            g.set(1, j, 1);
        }
        let m = extract_surface_oriented(&g, 0, 3, 0, 3, 0.8, 0.4, false);
        let r = check_mesh(&m);
        assert!(r.ok(), "{r:?}");
        assert!((r.volume_mm3 - 0.8 * 0.8 * 1.2).abs() < 1e-5, "volume {}", r.volume_mm3);
        let (lo, hi) = r.bounds_mm;
        assert!((hi[2] - lo[2] - 1.2).abs() < 1e-5 && (hi[0] - lo[0] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn cut_thin_removes_sticks_but_not_sheets_or_slabs() {
        let mut g = Grid::new(6, 6, 6);
        // a 2-thick slab survives
        for i in 0..6 {
            for k in 0..6 {
                g.set(i, 0, k);
                g.set(i, 1, k);
            }
        }
        // a pole and a one-voxel sheet on top of it go
        for j in 2..6 {
            g.set(1, j, 1);
        }
        for i in 3..6 {
            for j in 2..5 {
                g.set(i, j, 4);
            }
        }
        // a one-layer floor floating above the slab stays
        for i in 0..3 {
            for k in 0..3 {
                g.set(i, 5, k);
            }
        }
        let before = g.count();
        let removed = cut_thin(&mut g);
        // the pole's top cell is part of the floor now, so 3 pole cells go;
        // the one-voxel SHEET (thin in z only) and the floor (thin in y only) stay
        assert_eq!(removed, 3, "removed {removed}");
        assert_eq!(g.count(), before - 3);
        assert!(g.get(1, 5, 1), "the floor must survive");
        assert!(g.get(4, 3, 4), "the sheet must survive");
    }

    #[test]
    fn stl_round_trips() {
        let mut g = Grid::new(2, 2, 2);
        g.set(0, 0, 0);
        let m = extract_surface(&g, 0, 2, 0, 2, 1.5);
        let p = std::env::temp_dir().join(format!("mapgeom-stl-test-{}.stl", std::process::id()));
        write_stl(p.to_str().unwrap(), &m, "test").unwrap();
        let (n, tris) = read_stl(p.to_str().unwrap()).unwrap();
        assert_eq!(n, 12);
        let hi = tris.iter().flatten().map(|v| v[0].max(v[1]).max(v[2])).fold(0.0f32, f32::max);
        assert!((hi - 1.5).abs() < 1e-6);
        std::fs::remove_file(p).ok();
    }

    #[test]
    fn tri_box_cases() {
        let tri = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert!(tri_box_overlap([0.5, 0.5, 0.0], [0.5; 3], tri));
        assert!(!tri_box_overlap([3.0, 3.0, 0.0], [0.5; 3], tri));
        // a plane just above the box
        assert!(!tri_box_overlap([0.5, 0.5, 0.0], [0.5; 3], [[0.0, 0.0, 0.6], [1.0, 0.0, 0.6], [0.0, 1.0, 0.6]]));
    }

    #[test]
    fn veget_names() {
        assert!(is_veget_name("TreePineBig") && is_veget_name("Bush") && is_veget_name("Forest") && is_veget_name("RedIsland\\Items\\Vegetation\\PalmTreeBigA0.Item.Gbx") && is_veget_name("Summer"));
        assert!(!is_veget_name("RoadTechStart") && !is_veget_name("Flag16m") && !is_veget_name("StructurePillar"));
    }

    #[test]
    fn unprintable_items() {
        assert!(is_unprintable_item("Flag16m") && is_unprintable_item("Flag8m") && is_unprintable_item("Stadium\\Items\\Flag16m.Item.Gbx") && is_unprintable_item("Sparkler8m"));
        assert!(!is_unprintable_item("RoadSignC") && !is_unprintable_item("ShowSpeakers") && !is_unprintable_item("TreePineBig"));
    }

    #[test]
    fn terrain_names() {
        assert!(is_terrain_block("Water") && is_terrain_block("DirtCliff4") && is_terrain_block("Dirt0_Dirt2_Dirt4") && is_terrain_block("LandHill6"));
        assert!(!is_terrain_block("OpenTechRoadFC") && !is_terrain_block("DecoWallBaseVFC") && !is_terrain_block("PlatformTechBase"));
    }
}
