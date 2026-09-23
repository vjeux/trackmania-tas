//! Course geometry in Trackmania's frame: the display-list pieces turned into
//! world-space triangles with normalised UVs and per-texture materials, and
//! the `TrackSections` pieces turned into a collision soup with surfaces.
//!
//! Frames: MK64 is right-handed with y up and north = −z; TM2020 is x east,
//! y up, z north (left-handed). The map is (x, y, z) → (x, y, −z) · scale,
//! and every triangle's vertex order is reversed so that the TM convention
//! `normal = cross(b−a, c−a)` still points the way the N64 face did.

use crate::course::{Course, Piece, TexState};
use crate::texture::AssetIndex;
use std::collections::HashMap;

pub const G_CULL_BACK: u32 = 0x2000;
pub const G_CULL_FRONT: u32 = 0x1000;
pub const G_LIGHTING: u32 = 0x20000;

#[derive(Clone, Copy, Debug)]
pub struct Corner {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub rgba: [u8; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub c: [Corner; 3],
    /// Index into `Mesh::materials`; None = untextured (vertex colour only).
    pub mat: Option<usize>,
    pub two_sided: bool,
    /// Drawn with `G_LIGHTING`: the "colours" were normals, the piece is lit by the engine.
    pub lit: bool,
    /// The display list the triangle came from.
    pub piece: u32,
}

/// One texture as a TM material: the N64 texture symbol plus how it is
/// wrapped. Mirrored axes get a 2× mirrored copy of the image (so plain
/// repeat reproduces `G_TX_MIRROR`); clamped axes have their UVs clamped.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Material {
    pub sym: String,
    pub mirror_s: bool,
    pub mirror_t: bool,
    pub clamp_s: bool,
    pub clamp_t: bool,
    /// Texture size in texels (the asset's, or the tile's when the asset is unknown).
    pub w: u32,
    pub h: u32,
    pub fmt: u8,
}

impl Material {
    /// The file stem of this material's texture (`Road1`, `Road1_ms` for a mirrored-s copy).
    pub fn stem(&self) -> String {
        let base = self.sym.trim_start_matches("gTexture");
        let mut s = base.to_string();
        if self.mirror_s || self.mirror_t {
            s.push('_');
            s.push('m');
            if self.mirror_s {
                s.push('s');
            }
            if self.mirror_t {
                s.push('t');
            }
        }
        s
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub tris: Vec<Tri>,
    pub materials: Vec<Material>,
    pub piece_names: Vec<String>,
}

/// A collision triangle in TM space with its MK64 surface type.
#[derive(Clone, Copy, Debug)]
pub struct CollTri {
    pub p: [[f32; 3]; 3],
    pub surface: u8,
    pub section_id: u8,
}

/// MK64 units → metres. `mirror` flips x (the game's mirror mode).
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub scale: f32,
    pub mirror: bool,
    /// Added after scaling (metres): where the course origin lands in the map.
    pub offset: [f32; 3],
}

impl Frame {
    pub fn to_tm(&self, p: [i16; 3]) -> [f32; 3] {
        let x = if self.mirror { -(p[0] as f32) } else { p[0] as f32 };
        [x * self.scale + self.offset[0], p[1] as f32 * self.scale + self.offset[1], -(p[2] as f32) * self.scale + self.offset[2]]
    }
    pub fn to_tm_f(&self, p: [f32; 3]) -> [f32; 3] {
        let x = if self.mirror { -p[0] } else { p[0] };
        [x * self.scale + self.offset[0], p[1] * self.scale + self.offset[1], -p[2] * self.scale + self.offset[2]]
    }
}

/// The visual mesh of the course.
pub fn visual_mesh(course: &Course, pieces: &[Piece], assets: Option<&AssetIndex>, frame: &Frame) -> Mesh {
    let mut mesh = Mesh::default();
    let mut mat_index: HashMap<Material, usize> = HashMap::new();
    for (pi, piece) in pieces.iter().enumerate() {
        mesh.piece_names.push(piece.dl.clone());
        for t in &piece.tris {
            let st = t.tex.map(|i| course.tex_states[i as usize]);
            let mat = st.map(|st| {
                let sym = course.tex_syms[st.sym as usize].clone();
                let (w, h) = assets
                    .and_then(|a| a.locate(&sym))
                    .map(|l| (l.w, l.h))
                    .unwrap_or((st.w.max(1) as u32, st.h.max(1) as u32));
                let m = Material {
                    sym,
                    mirror_s: st.cms & 1 != 0,
                    mirror_t: st.cmt & 1 != 0,
                    clamp_s: st.cms & 2 != 0,
                    clamp_t: st.cmt & 2 != 0,
                    w,
                    h,
                    fmt: st.fmt,
                };
                *mat_index.entry(m.clone()).or_insert_with(|| {
                    mesh.materials.push(m);
                    mesh.materials.len() - 1
                })
            });
            let corner = |vi: u32| {
                let v = course.vertices[vi as usize];
                let uv = match (st, mat) {
                    (Some(st), Some(mi)) => uv_of(&st, &mesh.materials[mi], v.tc),
                    _ => [0.0, 0.0],
                };
                Corner { pos: frame.to_tm(v.pos), uv, rgba: [v.rgb[0], v.rgb[1], v.rgb[2], 255] }
            };
            // reversed order: see the module doc (the z flip mirrors the winding)
            let (a, b, c) = (corner(t.v[0]), corner(t.v[2]), corner(t.v[1]));
            // a mirrored course flips it back
            let c3 = if frame.mirror { [a, c, b] } else { [a, b, c] };
            mesh.tris.push(Tri {
                c: c3,
                mat,
                two_sided: t.geom & (G_CULL_BACK | G_CULL_FRONT) == 0,
                lit: t.geom & G_LIGHTING != 0,
                piece: pi as u32,
            });
        }
    }
    mesh
}

/// N64 texture coordinates (S10.5 texels, scaled by `gsSPTexture`, offset by
/// the tile origin) → normalised UV over the material's image (2× wide/high
/// when mirrored). `v` is NOT flipped here: the DDS writer keeps the image's
/// row order and the game samples v downwards from the top row, like the N64.
pub fn uv_of(st: &TexState, m: &Material, tc: [i16; 2]) -> [f32; 2] {
    let ss = if st.scale_s == 0 { 1.0 } else { st.scale_s as f32 / 65536.0 };
    let tt = if st.scale_t == 0 { 1.0 } else { st.scale_t as f32 / 65536.0 };
    let mut s = tc[0] as f32 / 32.0 * ss - st.uls as f32;
    let mut t = tc[1] as f32 / 32.0 * tt - st.ult as f32;
    if m.clamp_s {
        s = s.clamp(0.0, m.w as f32);
    }
    if m.clamp_t {
        t = t.clamp(0.0, m.h as f32);
    }
    let period_s = if m.mirror_s { 2.0 * m.w as f32 } else { m.w as f32 };
    let period_t = if m.mirror_t { 2.0 * m.h as f32 } else { m.h as f32 };
    [s / period_s, t / period_t]
}

/// The collision soup: every `TrackSections` piece, TM frame, reversed winding.
pub fn collision_mesh(course: &Course, coll: &[(crate::course::Section, Piece)], frame: &Frame) -> Vec<CollTri> {
    let mut out = Vec::new();
    for (sec, piece) in coll {
        for t in &piece.tris {
            let p = |vi: u32| frame.to_tm(course.vertices[vi as usize].pos);
            let (a, b, c) = (p(t.v[0]), p(t.v[2]), p(t.v[1]));
            let p3 = if frame.mirror { [a, c, b] } else { [a, b, c] };
            out.push(CollTri { p: p3, surface: sec.surface, section_id: sec.section_id });
        }
    }
    out
}

pub fn bbox(pts: impl Iterator<Item = [f32; 3]>) -> Option<([f32; 3], [f32; 3])> {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for p in pts {
        any = true;
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    any.then_some((lo, hi))
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}
pub fn face_normal(p: &[[f32; 3]; 3]) -> [f32; 3] {
    normalize(cross(sub(p[1], p[0]), sub(p[2], p[0])))
}
