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
    pub material_id: u8,
    pub u03: u8,
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
    Compound { surfs: Vec<(Surf, Option<[f32; 3]>)>, locs: Vec<[f32; 12]>, joints: Option<Vec<i16>> },
}

impl Surf {
    pub fn type_id(&self) -> i32 {
        match self {
            Surf::Sphere { .. } => 0,
            Surf::Ellipsoid { .. } => 1,
            Surf::Box { .. } => 6,
            Surf::Mesh { .. } => 7,
            Surf::Compound { .. } => 13,
        }
    }
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
                Ok(Triangle { indices: [r.u32()?, r.u32()?, r.u32()?], material_id: r.u8()?, u03: r.u8()?, surface_index: r.i16()? })
            })?;
            Surf::Mesh { version, vertices, triangles }
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
                w.u8(t.u03);
                w.i16(t.surface_index);
            }
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
