//! Geometry -> a Nadeo-style static item (`CGameItemModel` with a
//! `CPlugStaticObjectModel`: a `CPlugSolid2Model` for the eye and a
//! `CPlugSurface` for the car), through mapgeom's writer. Everything is in
//! metres, in the item's own frame; the map places the item.

use mapgeom::static_item::bake::{self, Corner, VisualLayout};
use mapgeom::static_item::build::{assemble, BuildOpts, Merged, MergedVisual};
use mapgeom::static_item::surface::{CPlugSurface, Triangle};
use mapgeom::static_item::write_file;

/// A game material and the physics the car feels on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub link: &'static str,
    pub physics: u8,
    /// Texture repeat in metres (box-mapped UVs).
    pub uv_scale: f32,
}

pub const ASPHALT: Material = Material { link: "Stadium\\Media\\Material\\RoadTech", physics: 16, uv_scale: 32.0 };
pub const KERB: Material = Material { link: "Stadium\\Media\\Material\\TrackBorders", physics: 9, uv_scale: 4.0 };
pub const CONCRETE: Material = Material { link: "Stadium\\Media\\Material\\PlatformTech", physics: 0, uv_scale: 32.0 };
pub const GRASS: Material = Material { link: "Stadium\\Media\\Material\\Grass", physics: 76, uv_scale: 32.0 };
pub const GRAVEL: Material = Material { link: "Stadium\\Media\\Material\\RoadDirt", physics: 6, uv_scale: 32.0 };
pub const WALL: Material = Material { link: "Stadium\\Media\\Material\\TrackWall", physics: 0, uv_scale: 16.0 };
pub const METAL: Material = Material { link: "Stadium\\Media\\Material\\Technics", physics: 4, uv_scale: 8.0 };

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaypointKind {
    Start,
    Finish,
    Checkpoint,
}

pub struct Waypoint {
    pub kind: WaypointKind,
    /// Where the car appears (start) in item space.
    pub spawn: [f32; 3],
    /// Axis-aligned trigger box (min, max) in item space; unused for a start.
    pub trigger: Option<([f32; 3], [f32; 3])>,
}

#[derive(Default)]
pub struct MeshBuilder {
    materials: Vec<Material>,
    tris: Vec<Vec<[Corner; 3]>>,
    /// Collision triangles: positions + physics id.
    coll: Vec<([[f32; 3]; 3], u8)>,
    face_counter: u32,
}

fn key(p: &[[f32; 3]; 3]) -> [u32; 9] {
    let mut k = [0u32; 9];
    for i in 0..3 {
        for j in 0..3 {
            k[i * 3 + j] = p[i][j].to_bits();
        }
    }
    k
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 { [0.0, 1.0, 0.0] } else { [v[0] / l, v[1] / l, v[2] / l] }
}

impl MeshBuilder {
    pub fn new() -> MeshBuilder {
        MeshBuilder::default()
    }

    pub fn material(&mut self, m: &Material) -> usize {
        if let Some(i) = self.materials.iter().position(|x| x == m) {
            return i;
        }
        self.materials.push(m.clone());
        self.tris.push(Vec::new());
        self.materials.len() - 1
    }

    /// Box-mapped UV for a point, given the face normal's dominant axis. v is
    /// squeezed into the atlases' lit band (0.06..0.94): Nadeo's Stadium
    /// materials are atlases whose rows outside that band are black, which is
    /// how the buildings first came out black in play mode.
    fn box_uv(p: [f32; 3], n: [f32; 3], scale: f32) -> [f32; 2] {
        let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
        let squeeze = |v: f32| 0.06 + 0.88 * v.rem_euclid(1.0);
        if ay >= ax && ay >= az {
            [p[0] / scale, squeeze(p[2] / scale)]
        } else if ax >= az {
            [p[2] / scale, squeeze(p[1] / scale)]
        } else {
            [p[0] / scale, squeeze(p[1] / scale)]
        }
    }

    /// One triangle, counter-clockwise seen from the side it faces; drawn
    /// and (when `collide`) driven on.
    pub fn tri(&mut self, mat: usize, p: [[f32; 3]; 3], collide: bool) {
        let n = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
        if n == [0.0, 1.0, 0.0] && cross(sub(p[1], p[0]), sub(p[2], p[0])) == [0.0, 0.0, 0.0] {
            return; // degenerate
        }
        let scale = self.materials[mat].uv_scale;
        self.face_counter += 1;
        let mk = |q: [f32; 3]| Corner { pos: q, normal: n, uv: Self::box_uv(q, n, scale), uv1: Self::box_uv(q, n, scale), tan_u: [1.0, 0.0, 0.0], tan_v: [0.0, 0.0, 1.0], face: self.face_counter, group: 0 };
        let mut t = [mk(p[0]), mk(p[1]), mk(p[2])];
        let (tu, tv) = bake::tangent(&t);
        for c in &mut t {
            c.tan_u = tu;
            c.tan_v = tv;
        }
        self.tris[mat].push(t);
        if collide {
            self.coll.push((p, self.materials[mat].physics));
        }
    }

    /// One triangle with explicit texture coordinates (box mapping skipped).
    pub fn tri_uv(&mut self, mat: usize, p: [[f32; 3]; 3], uv: [[f32; 2]; 3], collide: bool) {
        let n = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
        if cross(sub(p[1], p[0]), sub(p[2], p[0])) == [0.0, 0.0, 0.0] {
            return;
        }
        self.face_counter += 1;
        let mk = |q: [f32; 3], t: [f32; 2]| Corner { pos: q, normal: n, uv: t, uv1: t, tan_u: [1.0, 0.0, 0.0], tan_v: [0.0, 0.0, 1.0], face: self.face_counter, group: 0 };
        let mut t = [mk(p[0], uv[0]), mk(p[1], uv[1]), mk(p[2], uv[2])];
        let (tu, tv) = bake::tangent(&t);
        for c in &mut t {
            c.tan_u = tu;
            c.tan_v = tv;
        }
        self.tris[mat].push(t);
        if collide {
            self.coll.push((p, self.materials[mat].physics));
        }
    }

    /// A quad with explicit UVs whose face must point up (+y).
    pub fn quad_uv_up(&mut self, mat: usize, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], collide: bool) {
        let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        if n[1] >= 0.0 {
            self.tri_uv(mat, [p[0], p[1], p[2]], [uv[0], uv[1], uv[2]], collide);
            self.tri_uv(mat, [p[0], p[2], p[3]], [uv[0], uv[2], uv[3]], collide);
        } else {
            self.tri_uv(mat, [p[0], p[3], p[2]], [uv[0], uv[3], uv[2]], collide);
            self.tri_uv(mat, [p[0], p[2], p[1]], [uv[0], uv[2], uv[1]], collide);
        }
    }

    /// A quad whose face must point AWAY from `inside` (a point behind it):
    /// the winding is flipped when the normal points towards that point. In
    /// play mode a face lit from behind renders black, so every wall of a
    /// building faces outwards and every side of a slab faces out.
    pub fn quad_away(&mut self, mat: usize, p: [[f32; 3]; 4], inside: [f32; 3], collide: bool) {
        let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        let c = [(p[0][0] + p[2][0]) / 2.0, (p[0][1] + p[2][1]) / 2.0, (p[0][2] + p[2][2]) / 2.0];
        let d = sub(c, inside);
        if n[0] * d[0] + n[1] * d[1] + n[2] * d[2] >= 0.0 {
            self.quad(mat, p, collide);
        } else {
            self.quad(mat, [p[0], p[3], p[2], p[1]], collide);
        }
    }

    /// A hexahedron from its bottom ring `b` and top ring `t` (same order),
    /// every face facing out; `underside` draws the bottom too.
    pub fn slab(&mut self, mat: usize, b: [[f32; 3]; 4], t: [[f32; 3]; 4], underside: bool, collide: bool) {
        let mut c = [0.0f32; 3];
        for q in b.iter().chain(t.iter()) {
            for k in 0..3 {
                c[k] += q[k] / 8.0;
            }
        }
        self.quad_away(mat, t, c, collide);
        if underside {
            self.quad_away(mat, b, c, collide);
        }
        for i in 0..4 {
            let j = (i + 1) % 4;
            self.quad_away(mat, [b[i], b[j], t[j], t[i]], c, collide);
        }
    }

    /// A quad p0 p1 p2 p3 (counter-clockwise), as two triangles.
    pub fn quad(&mut self, mat: usize, p: [[f32; 3]; 4], collide: bool) {
        self.tri(mat, [p[0], p[1], p[2]], collide);
        self.tri(mat, [p[0], p[2], p[3]], collide);
    }

    /// A quad whose face must point UP (+y): the winding is fixed so the
    /// normal has a positive y, whatever order the corners came in.
    pub fn quad_up(&mut self, mat: usize, p: [[f32; 3]; 4], collide: bool) {
        let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        if n[1] >= 0.0 {
            self.quad(mat, p, collide);
        } else {
            self.quad(mat, [p[0], p[3], p[2], p[1]], collide);
        }
    }

    /// Re-emit every triangle of `self` into `dst` with a point transform and
    /// a material map (src slot -> dst slot). Collision follows the visual.
    pub fn replay_into(&self, dst: &mut MeshBuilder, mats: &[(usize, usize)], f: &dyn Fn([f32; 3]) -> [f32; 3]) {
        let coll: std::collections::HashSet<[u32; 9]> = self.coll.iter().map(|(p, _)| key(p)).collect();
        for (si, tris) in self.tris.iter().enumerate() {
            let di = mats.iter().find(|(s, _)| *s == si).map(|(_, d)| *d).unwrap_or_else(|| panic!("material {si} not mapped"));
            for t in tris {
                let p = [t[0].pos, t[1].pos, t[2].pos];
                let collide = coll.contains(&key(&p));
                dst.tri_uv(di, [f(p[0]), f(p[1]), f(p[2])], [t[0].uv, t[1].uv, t[2].uv], collide);
            }
        }
    }

    /// An axis-aligned box (all six faces, outward), collidable.
    pub fn abox(&mut self, mat: usize, lo: [f32; 3], hi: [f32; 3], collide: bool) {
        let c = |x: usize, y: usize, z: usize| [if x == 0 { lo[0] } else { hi[0] }, if y == 0 { lo[1] } else { hi[1] }, if z == 0 { lo[2] } else { hi[2] }];
        // top (+y)
        self.quad(mat, [c(0, 1, 0), c(0, 1, 1), c(1, 1, 1), c(1, 1, 0)], collide);
        // bottom (-y)
        self.quad(mat, [c(0, 0, 0), c(1, 0, 0), c(1, 0, 1), c(0, 0, 1)], collide);
        // -x
        self.quad(mat, [c(0, 0, 0), c(0, 0, 1), c(0, 1, 1), c(0, 1, 0)], collide);
        // +x
        self.quad(mat, [c(1, 0, 0), c(1, 1, 0), c(1, 1, 1), c(1, 0, 1)], collide);
        // -z
        self.quad(mat, [c(0, 0, 0), c(0, 1, 0), c(1, 1, 0), c(1, 0, 0)], collide);
        // +z
        self.quad(mat, [c(0, 0, 1), c(1, 0, 1), c(1, 1, 1), c(0, 1, 1)], collide);
    }

    pub fn triangle_count(&self) -> usize {
        self.tris.iter().map(|t| t.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.triangle_count() == 0
    }

    /// The item file bytes.
    /// FNV-1a over the collision triangles (mm-rounded) and their physics,
    /// plus the waypoint kind/spawn/trigger: everything the car can feel.
    pub fn physics_hash(&self, waypoint: Option<&Waypoint>) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |v: i64| {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        for (p, phys) in &self.coll {
            for q in p {
                for c in q {
                    feed((c * 1000.0).round() as i64);
                }
            }
            feed(*phys as i64);
        }
        if let Some(w) = waypoint {
            feed(match w.kind {
                WaypointKind::Start => 1,
                WaypointKind::Finish => 2,
                WaypointKind::Checkpoint => 3,
            });
            for c in w.spawn {
                feed((c * 1000.0).round() as i64);
            }
            if let Some((lo, hi)) = w.trigger {
                for c in lo.iter().chain(hi.iter()) {
                    feed((c * 1000.0).round() as i64);
                }
            }
        }
        h
    }

    /// The collision triangles as the game will see them: placed at `pos`,
    /// turned by `yaw` (local +z onto (sin yaw, cos yaw), the inverse of
    /// `mapbuild::to_local`).
    pub fn coll_world(&self, pos: [f32; 3], yaw: f32) -> Vec<[[f32; 3]; 3]> {
        let (s, c) = yaw.sin_cos();
        let f = |l: [f32; 3]| -> [f32; 3] { [pos[0] + l[0] * c + l[2] * s, pos[1] + l[1], pos[2] - l[0] * s + l[2] * c] };
        self.coll.iter().map(|(p, _)| [f(p[0]), f(p[1]), f(p[2])]).collect()
    }

    pub fn build(mut self, ident: &str, author: &str, waypoint: Option<&Waypoint>) -> Vec<u8> {
        assert!(!self.is_empty(), "{ident}: empty mesh");
        let mut m = Merged::default();
        let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        m.file_write_time = unix * 10_000_000 + 116444736000000000;
        // smooth normals per material, then one lightmap atlas over all
        for tris in &mut self.tris {
            if !tris.is_empty() {
                bake::smooth_normals(tris);
            }
        }
        let has_uv1: Vec<bool> = self.materials.iter().map(|_| true).collect();
        bake::assign_lightmap_atlas(&mut self.tris, &has_uv1);
        let (mut mnx, mut mxx, mut mny, mut mxy) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let (mut aworld, mut auv1) = (0.0f64, 0.0f64);
        for (i, mat) in self.materials.iter().enumerate() {
            let tris = &self.tris[i];
            if tris.is_empty() {
                continue;
            }
            let slot = m.material_slot(mat.link, mat.physics);
            for t in tris {
                for c in t {
                    mnx = mnx.min(c.uv1[0]);
                    mxx = mxx.max(c.uv1[0]);
                    mny = mny.min(c.uv1[1]);
                    mxy = mxy.max(c.uv1[1]);
                }
                let au = ((t[1].uv1[0] - t[0].uv1[0]) * (t[2].uv1[1] - t[0].uv1[1]) - (t[2].uv1[0] - t[0].uv1[0]) * (t[1].uv1[1] - t[0].uv1[1])).abs() as f64 / 2.0;
                let cr = cross(sub(t[1].pos, t[0].pos), sub(t[2].pos, t[0].pos));
                aworld += ((cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]) as f64).sqrt() / 2.0;
                auv1 += au;
            }
            let layout = bake::visual_layout(mat.link);
            assert!(layout == VisualLayout::Full || layout == VisualLayout::White, "{}: unexpected layout", mat.link);
            for v in bake::make_visuals(tris, layout, "") {
                m.visuals.push(MergedVisual { visual: v, material: slot });
            }
        }
        // collision: welded by exact position
        let mut verts: Vec<[f32; 3]> = Vec::new();
        let mut seen: std::collections::HashMap<[u32; 3], u32> = std::collections::HashMap::new();
        let mut tris: Vec<Triangle> = Vec::new();
        for (p, phys) in &self.coll {
            let mut ix = [0u32; 3];
            for (k, q) in p.iter().enumerate() {
                ix[k] = *seen.entry([q[0].to_bits(), q[1].to_bits(), q[2].to_bits()]).or_insert_with(|| {
                    verts.push(*q);
                    (verts.len() - 1) as u32
                });
            }
            tris.push(Triangle { indices: ix, material_id: *phys, u03: 0, surface_index: 0 });
        }
        m.add_surface_mesh(&verts, &tris, &mapgeom::geom::IDENTITY, 1.0);
        if let Some(w) = waypoint {
            m.waypoint_type = Some(match w.kind {
                WaypointKind::Start => 0,
                WaypointKind::Finish => 1,
                WaypointKind::Checkpoint => 2,
            });
            m.spawn = w.spawn;
            if let Some((lo, hi)) = w.trigger {
                m.trigger = Some(trigger_box(lo, hi));
            }
        }
        let u02 = if auv1 > 1e-12 { (aworld / auv1).sqrt() as f32 } else { 32.14457 };
        m.pre_light_gen = Some(mapgeom::static_item::solid2::PreLightGen {
            version: 1,
            u01: 1,
            u02,
            u03: true,
            u04: [mnx, mny, mxx, mxy, f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            sprite_count: [0, 0],
            boxes: Vec::new(),
            uv_groups: Vec::new(),
        });
        let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale: 1.0, collection: 26, editors: false, skin: None };
        let f = assemble(&m, &opts).unwrap_or_else(|e| panic!("{ident}: {e}"));
        write_file(&f)
    }
}

/// A closed box as a trigger surface (12 triangles, outward winding).
fn trigger_box(lo: [f32; 3], hi: [f32; 3]) -> CPlugSurface {
    let c = |x: usize, y: usize, z: usize| [if x == 0 { lo[0] } else { hi[0] }, if y == 0 { lo[1] } else { hi[1] }, if z == 0 { lo[2] } else { hi[2] }];
    let verts: Vec<[f32; 3]> = (0..8).map(|i| c(i & 1, (i >> 1) & 1, (i >> 2) & 1)).collect();
    // vertex index = x | y<<1 | z<<2
    let quads: [[u32; 4]; 6] = [
        [2, 6, 7, 3], // +y
        [0, 1, 5, 4], // -y
        [0, 4, 6, 2], // -x
        [1, 3, 7, 5], // +x
        [0, 2, 3, 1], // -z
        [4, 5, 7, 6], // +z
    ];
    let mut tris = Vec::new();
    for q in quads {
        tris.push(Triangle { indices: [q[0], q[1], q[2]], material_id: 0, u03: 0, surface_index: 0 });
        tris.push(Triangle { indices: [q[0], q[2], q[3]], material_id: 0, u03: 0, surface_index: 0 });
    }
    CPlugSurface::mesh(verts, tris, vec![0], [0.0, 0.0, 1.0])
}
