//! `CPlugSurface` (0x0900C000): chunk 0x0900C003 with the `GmSurf` archive
//! (Mesh v6/7 typed; Sphere, Ellipsoid, Box, Compound per GBX.NET), the
//! material list, the u16 material-id list and the skeleton reference.

use super::{read_ref, write_ref, Id, Rd, Ref, Wr, R, FACADE};

/// A `GmSurfMeshTri`: three vertex indices, then the physics material id
/// (a byte), the gameplay id (a byte: 1 = Turbo on `Z_Mini_Pltf_Flat_Turbo1`),
/// and the index into `CPlugSurface::material_ids`, whose u16 entries are
/// `physics | gameplay << 8`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    pub indices: [u32; 3],
    /// The physics id (the surface material: Asphalt 16, Grass 76, …).
    pub material_id: u8,
    /// The gameplay id (Turbo 1, ReactorBoost 12, Reset 8, …; 0 none) — the
    /// high byte of the `material_ids` table entry this triangle indexes.
    pub gameplay: u8,
    pub surface_index: i16,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Surf {
    Sphere { size: f32, surface_index: Option<i16> },
    Ellipsoid { size: [f32; 3], surface_index: Option<i16> },
    Box { transform: [f32; 6], surface_index: Option<i16> },
    Mesh {
        version: u32,
        vertices: Vec<[f32; 3]>,
        triangles: Vec<Triangle>,
    },
    /// The convex hull the dyna objects move with (a pusher's `MoveShape` is
    /// one; a rotor's is a compound of nine): a bounding box (centre, half
    /// extents), the hull's vertices, a flat index list and the faces as
    /// (start, count) runs into it, then the surface index like the other
    /// primitives. Read off `ObstaclePusher8mPiston.MoveShape.Gbx` (20 vertices,
    /// 18 faces) and `ObstacleRotor16mHolesX4.MoveShape.Gbx`.
    ConvexPolyhedron {
        version: u32,
        u01: u32,
        center: [f32; 3],
        half: [f32; 3],
        vertices: Vec<[f32; 3]>,
        indices: Vec<u32>,
        faces: Vec<(u32, u32)>,
        surface_index: Option<i16>,
    },
    Compound { surfs: Vec<(Surf, Option<[f32; 3]>)>, locs: Vec<[f32; 12]>, joints: Option<Vec<i16>> },
}

impl Surf {
    pub fn type_id(&self) -> i32 {
        match self {
            Surf::Sphere { .. } => 0,
            Surf::Ellipsoid { .. } => 1,
            Surf::Box { .. } => 6,
            Surf::Mesh { .. } => 7,
            Surf::ConvexPolyhedron { .. } => 10,
            Surf::Compound { .. } => 13,
        }
    }

    /// Every length in the shape multiplied by `s` (a compound's child
    /// placements included).
    pub fn scale(&mut self, s: f32) {
        let sv = |v: &mut [f32; 3]| {
            for x in v.iter_mut() {
                *x *= s;
            }
        };
        match self {
            Surf::Sphere { size, .. } => *size *= s,
            Surf::Ellipsoid { size, .. } => sv(size),
            Surf::Box { transform, .. } => {
                for x in transform.iter_mut() {
                    *x *= s;
                }
            }
            Surf::Mesh { vertices, .. } => vertices.iter_mut().for_each(sv),
            Surf::ConvexPolyhedron { center, half, vertices, .. } => {
                sv(center);
                sv(half);
                vertices.iter_mut().for_each(sv);
            }
            Surf::Compound { surfs, locs, .. } => {
                for (c, _) in surfs.iter_mut() {
                    c.scale(s);
                }
                for l in locs.iter_mut() {
                    l[9] *= s;
                    l[10] *= s;
                    l[11] *= s;
                }
            }
        }
    }

    /// The shape as triangles in its own frame, for merging into an item's one
    /// collision mesh: a mesh as is; a box (centre + half extents, axis
    /// aligned — the six floats carry no rotation) as 12 triangles; a sphere or
    /// ellipsoid as a 16×8 UV sphere (128 triangles, radius error < 2 %); a
    /// convex polyhedron fanned from its face runs; a compound as its children
    /// placed by their Iso4. `None` when nothing can be produced. Every
    /// triangle carries the shape's surface index as its physics id (the
    /// caller maps it through the surface's material table like a mesh
    /// triangle's u8).
    pub fn triangulate(&self) -> Option<(Vec<[f32; 3]>, Vec<Triangle>)> {
        let tri = |i: [u32; 3], si: Option<i16>| Triangle { indices: i, material_id: si.unwrap_or(0).max(0) as u8, gameplay: 0, surface_index: si.unwrap_or(0) };
        match self {
            Surf::Mesh { vertices, triangles, .. } => Some((vertices.clone(), triangles.clone())),
            Surf::Box { transform, surface_index } => {
                let c = [transform[0], transform[1], transform[2]];
                let h = [transform[3].abs(), transform[4].abs(), transform[5].abs()];
                let v: Vec<[f32; 3]> = (0..8).map(|k| [c[0] + if k & 1 == 0 { -h[0] } else { h[0] }, c[1] + if k & 2 == 0 { -h[1] } else { h[1] }, c[2] + if k & 4 == 0 { -h[2] } else { h[2] }]).collect();
                // outward-facing (counter-clockwise seen from outside)
                let faces: [[u32; 3]; 12] = [[0, 2, 3], [0, 3, 1], [4, 5, 7], [4, 7, 6], [0, 1, 5], [0, 5, 4], [2, 6, 7], [2, 7, 3], [0, 4, 6], [0, 6, 2], [1, 3, 7], [1, 7, 5]];
                Some((v, faces.iter().map(|f| tri(*f, *surface_index)).collect()))
            }
            Surf::Sphere { size, surface_index } => Some(uv_sphere([0.0; 3], [*size, *size, *size], *surface_index, &tri)),
            Surf::Ellipsoid { size, surface_index } => Some(uv_sphere([0.0; 3], *size, *surface_index, &tri)),
            Surf::ConvexPolyhedron { vertices, indices, faces, surface_index, .. } => {
                let mut out = Vec::new();
                for (start, count) in faces {
                    let (s, n) = (*start as usize, *count as usize);
                    if n < 3 || s + n > indices.len() {
                        continue;
                    }
                    let a = indices[s] as u32;
                    for k in 1..n - 1 {
                        out.push(tri([a, indices[s + k] as u32, indices[s + k + 1] as u32], *surface_index));
                    }
                }
                Some((vertices.clone(), out))
            }
            Surf::Compound { surfs, locs, .. } => {
                let mut verts: Vec<[f32; 3]> = Vec::new();
                let mut tris: Vec<Triangle> = Vec::new();
                for (k, (child, _)) in surfs.iter().enumerate() {
                    let Some((cv, ct)) = child.triangulate() else { continue };
                    let base = verts.len() as u32;
                    let loc = locs.get(k).copied().unwrap_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
                    for p in cv {
                        verts.push(crate::geom::apply(&loc, p));
                    }
                    for t in ct {
                        tris.push(Triangle { indices: [t.indices[0] + base, t.indices[1] + base, t.indices[2] + base], ..t });
                    }
                }
                if tris.is_empty() { None } else { Some((verts, tris)) }
            }
        }
    }

    /// Triangle / vertex counts for reports (a polyhedron counts its faces).
    pub fn counts(&self) -> (usize, usize) {
        match self {
            Surf::Mesh { vertices, triangles, .. } => (vertices.len(), triangles.len()),
            Surf::ConvexPolyhedron { vertices, faces, .. } => (vertices.len(), faces.len()),
            Surf::Compound { surfs, .. } => surfs.iter().map(|(s, _)| s.counts()).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1)),
            _ => (0, 0),
        }
    }
}

/// A UV sphere / ellipsoid (`radii` per axis) about `c`: 16 segments × 8
/// rings, outward-facing triangles.
fn uv_sphere(c: [f32; 3], radii: [f32; 3], si: Option<i16>, tri: &dyn Fn([u32; 3], Option<i16>) -> Triangle) -> (Vec<[f32; 3]>, Vec<Triangle>) {
    const SEG: u32 = 16;
    const RINGS: u32 = 8;
    let mut v: Vec<[f32; 3]> = Vec::new();
    for ring in 0..=RINGS {
        let phi = std::f32::consts::PI * ring as f32 / RINGS as f32; // 0 = top
        let (sp, cp) = phi.sin_cos();
        for seg in 0..SEG {
            let th = 2.0 * std::f32::consts::PI * seg as f32 / SEG as f32;
            let (st, ct) = th.sin_cos();
            v.push([c[0] + radii[0] * sp * ct, c[1] + radii[1] * cp, c[2] + radii[2] * sp * st]);
        }
    }
    let mut t: Vec<Triangle> = Vec::new();
    let at = |ring: u32, seg: u32| ring * SEG + (seg % SEG);
    for ring in 0..RINGS {
        for seg in 0..SEG {
            let (a, b, c2, d) = (at(ring, seg), at(ring, seg + 1), at(ring + 1, seg + 1), at(ring + 1, seg));
            if ring > 0 {
                t.push(tri([a, c2, b], si));
            }
            if ring < RINGS - 1 {
                t.push(tri([a, d, c2], si));
            }
        }
    }
    (v, t)
}

/// `ReadSurf`: type id, the surf, and (surf version 2+) the gameplay main
/// direction.
pub fn read_surf(r: &mut Rd, sv: u32) -> R<(Surf, Option<[f32; 3]>)> {
    let ty = r.i32()?;
    let si = |r: &mut Rd| -> R<Option<i16>> { Ok(if sv >= 1 { Some(r.i16()?) } else { None }) };
    let s = match ty {
        0 => {
            let size = r.f32()?;
            Surf::Sphere { size, surface_index: si(r)? }
        }
        1 => {
            let size = r.vec3()?;
            Surf::Ellipsoid { size, surface_index: si(r)? }
        }
        6 => {
            let transform = r.floats::<6>()?;
            Surf::Box { transform, surface_index: si(r)? }
        }
        7 => {
            let version = r.u32()?;
            if !(6..=7).contains(&version) {
                return Err(format!("GmSurf Mesh version {version} (only 6/7 are modelled)"));
            }
            let vertices = r.array(|r| r.vec3())?;
            let triangles = r.array(|r| {
                Ok(Triangle { indices: [r.u32()?, r.u32()?, r.u32()?], material_id: r.u8()?, gameplay: r.u8()?, surface_index: r.i16()? })
            })?;
            Surf::Mesh { version, vertices, triangles }
        }
        10 => {
            let version = r.u32()?;
            if version != 0 {
                return Err(format!("GmSurf ConvexPolyhedron version {version} (only 0 is modelled)"));
            }
            let u01 = r.u32()?;
            let center = r.vec3()?;
            let half = r.vec3()?;
            let vertices = r.array(|r| r.vec3())?;
            let indices = r.array(|r| r.u32())?;
            let faces = r.array(|r| Ok((r.u32()?, r.u32()?)))?;
            Surf::ConvexPolyhedron { version, u01, center, half, vertices, indices, faces, surface_index: si(r)? }
        }
        13 => {
            let n = r.count()?;
            let mut surfs = Vec::with_capacity(n);
            for _ in 0..n {
                surfs.push(read_surf(r, sv)?);
            }
            let locs = (0..n).map(|_| r.floats::<12>()).collect::<R<_>>()?;
            let joints = if sv >= 1 { Some(r.array(|r| r.i16())?) } else { None };
            Surf::Compound { surfs, locs, joints }
        }
        t => return Err(format!("GmSurf type {t} has no reader")),
    };
    let dir = if sv >= 2 { Some(r.vec3()?) } else { None };
    Ok((s, dir))
}

pub fn write_surf(w: &mut Wr, s: &Surf, dir: &Option<[f32; 3]>, sv: u32) {
    w.i32(s.type_id());
    let si = |w: &mut Wr, i: &Option<i16>| {
        if sv >= 1 {
            w.i16(i.unwrap_or(0));
        }
    };
    match s {
        Surf::Sphere { size, surface_index } => {
            w.f32(*size);
            si(w, surface_index);
        }
        Surf::Ellipsoid { size, surface_index } => {
            w.floats(size);
            si(w, surface_index);
        }
        Surf::Box { transform, surface_index } => {
            w.floats(transform);
            si(w, surface_index);
        }
        Surf::Mesh { version, vertices, triangles } => {
            w.u32(*version);
            w.u32(vertices.len() as u32);
            vertices.iter().for_each(|v| w.floats(v));
            w.u32(triangles.len() as u32);
            for t in triangles {
                t.indices.iter().for_each(|i| w.u32(*i));
                w.u8(t.material_id);
                w.u8(t.gameplay);
                w.i16(t.surface_index);
            }
        }
        Surf::ConvexPolyhedron { version, u01, center, half, vertices, indices, faces, surface_index } => {
            w.u32(*version);
            w.u32(*u01);
            w.floats(center);
            w.floats(half);
            w.u32(vertices.len() as u32);
            vertices.iter().for_each(|v| w.floats(v));
            w.u32(indices.len() as u32);
            indices.iter().for_each(|i| w.u32(*i));
            w.u32(faces.len() as u32);
            for (a, b) in faces {
                w.u32(*a);
                w.u32(*b);
            }
            si(w, surface_index);
        }
        Surf::Compound { surfs, locs, joints } => {
            w.u32(surfs.len() as u32);
            for (s, d) in surfs {
                write_surf(w, s, d, sv);
            }
            locs.iter().for_each(|l| w.floats(l));
            if let Some(j) = joints {
                w.u32(j.len() as u32);
                j.iter().for_each(|x| w.i16(*x));
            }
        }
    }
    if sv >= 2 {
        w.floats(&dir.unwrap_or([0.0; 3]));
    }
}

/// `SurfMaterial`: an external material node, or a physics id.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfMaterial {
    Node(Ref),
    Id(i16),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugSurface {
    pub version: u32,
    /// v2+
    pub surf_version: u32,
    pub surf: Surf,
    pub gameplay_main_dir: Option<[f32; 3]>,
    pub materials: Vec<SurfMaterial>,
    /// v4+, when `materials` is non-empty and none of them is a null node.
    /// GBX.NET reads it for any non-empty list ("somehow doesn't exist in the
    /// code, but works for almost every TM2020 surface"); the 4 Stadium.pak
    /// surfaces it does not work for are exactly the ones whose material list
    /// holds a `-1` node ref (`Items\E193E35D...`: [21, 22, -1] then straight
    /// to the 3 u16 ids). Measured on all 9718 prefabs.
    pub u05: Option<i32>,
    /// v<3: raw data; v3+: the u16 physics ids the triangles index (one per
    /// `materials` entry when there are any).
    pub u01: Vec<u8>,
    pub material_ids: Vec<u16>,
    /// v1+
    pub skel: Ref,
    /// v5+
    pub u06: Vec<Id>,
}

impl CPlugSurface {
    fn has_u05(version: u32, materials: &[SurfMaterial]) -> bool {
        version >= 4 && !materials.is_empty() && !materials.iter().any(|m| matches!(m, SurfMaterial::Node(n) if n.index == -1))
    }

    /// Parse the node body after its class id: chunk 0x0900C003 then FACADE.
    pub fn parse(r: &mut Rd) -> R<CPlugSurface> {
        let cid = r.u32()?;
        if cid != 0x0900C003 {
            return Err(format!("CPlugSurface starts with chunk 0x{cid:08X} (only 0x0900C003 is modelled)"));
        }
        let version = r.u32()?;
        let surf_version = if version >= 2 { r.u32()? } else { 0 };
        let sv = if version == 1 { 1 } else { surf_version };
        let (surf, gameplay_main_dir) = read_surf(r, sv)?;
        let materials = r.array(|r| Ok(if r.bool32()? { SurfMaterial::Node(read_ref(r)?) } else { SurfMaterial::Id(r.i16()?) }))?;
        let u05 = if Self::has_u05(version, &materials) { Some(r.i32()?) } else { None };
        let (mut u01, mut material_ids) = (Vec::new(), Vec::new());
        if version < 3 {
            let n = r.count()?;
            u01 = r.take(n)?.to_vec();
        } else {
            material_ids = r.array(|r| r.u16())?;
        }
        let skel = if version >= 1 { read_ref(r)? } else { super::null_ref() };
        let u06 = if version >= 5 { r.array(|r| r.id())? } else { Vec::new() };
        let f = r.u32()?;
        if f != FACADE {
            return Err(format!("CPlugSurface: 0x{f:08X} after chunk 003 is not FACADE (at 0x{:x})", r.o - 4));
        }
        Ok(CPlugSurface { version, surf_version, surf, gameplay_main_dir, materials, u05, u01, material_ids, skel, u06 })
    }

    pub fn write(&self, w: &mut Wr) {
        w.u32(0x0900C003);
        w.u32(self.version);
        if self.version >= 2 {
            w.u32(self.surf_version);
        }
        let sv = if self.version == 1 { 1 } else { self.surf_version };
        write_surf(w, &self.surf, &self.gameplay_main_dir, sv);
        w.u32(self.materials.len() as u32);
        for m in &self.materials {
            match m {
                SurfMaterial::Node(n) => {
                    w.bool32(true);
                    write_ref(w, n);
                }
                SurfMaterial::Id(i) => {
                    w.bool32(false);
                    w.i16(*i);
                }
            }
        }
        if Self::has_u05(self.version, &self.materials) {
            w.i32(self.u05.unwrap_or(0));
        }
        if self.version < 3 {
            w.u32(self.u01.len() as u32);
            w.bytes(&self.u01);
        } else {
            w.u32(self.material_ids.len() as u32);
            self.material_ids.iter().for_each(|x| w.u16(*x));
        }
        if self.version >= 1 {
            write_ref(w, &self.skel);
        }
        if self.version >= 5 {
            w.u32(self.u06.len() as u32);
            self.u06.iter().for_each(|i| w.id(i));
        }
        w.u32(FACADE);
    }

    /// A TM2020 mesh surface (chunk v4, surf v2, mesh v7) over triangles whose
    /// `surface_index` indexes `material_ids`.
    pub fn mesh(vertices: Vec<[f32; 3]>, triangles: Vec<Triangle>, material_ids: Vec<u16>, dir: [f32; 3]) -> CPlugSurface {
        CPlugSurface {
            version: 4,
            surf_version: 2,
            surf: Surf::Mesh { version: 7, vertices, triangles },
            gameplay_main_dir: Some(dir),
            materials: Vec::new(),
            u05: None,
            u01: Vec::new(),
            material_ids,
            skel: super::null_ref(),
            u06: Vec::new(),
        }
    }
}
