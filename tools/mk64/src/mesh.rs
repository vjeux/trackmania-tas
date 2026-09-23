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
    /// The vertex-colour tint baked into this variant of the texture
    /// (255,255,255 = the texture as is). `Flat` materials are a tint alone.
    pub tint: [u8; 3],
}

/// The synthetic "texture" of untextured (vertex-coloured) faces.
pub const FLAT_SYM: &str = "Flat";

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
        if self.tint != [255, 255, 255] {
            s.push_str(&format!("_c{:02x}{:02x}{:02x}", self.tint[0], self.tint[1], self.tint[2]));
        }
        s
    }
    pub fn is_flat(&self) -> bool {
        self.sym == FLAT_SYM
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
                    tint: [255, 255, 255],
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
/// when mirrored). `v` IS flipped: the game samples v upwards from the DDS's
/// bottom row (vjeux, driving Luigi Raceway: "all the textures are upside
/// down" with the N64 top-down v; docs/formats/textures-dds-skins.md §1).
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
    [s / period_s, 1.0 - t / period_t]
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

/// Metres per MK64 unit: the official lap lengths over the centre-path
/// lengths agree to 0.3 % on all 15 measured courses (0.0565–0.0568;
/// Rainbow Road's rounded "2000 m" reads 0.0562). One constant for all.
pub const UNITS_TO_M: f32 = 0.05673;

/// The plateau problem: a course sits above the Stadium grass (its lowest
/// road point must clear the solid ground plane), so its outer terrain edges
/// hang in the air. This closes them: every upward-facing triangle edge with
/// no geometry beyond it (top view) grows a vertical quad down to `ground_y`,
/// textured like the triangle it hangs from. Returns the number of quads.
pub fn add_skirt(mesh: &mut Mesh, ground_y: f32) -> usize {
    let key = |p: [f32; 3]| [(p[0] * 100.0).round() as i64, (p[1] * 100.0).round() as i64, (p[2] * 100.0).round() as i64];
    // edge → (count, first triangle index)
    let mut edges: HashMap<([i64; 3], [i64; 3]), (u32, usize)> = HashMap::new();
    let mut up: Vec<usize> = Vec::new();
    for (ti, t) in mesh.tris.iter().enumerate() {
        let n = face_normal(&[t.c[0].pos, t.c[1].pos, t.c[2].pos]);
        if n[1] < 0.6 || t.two_sided {
            continue;
        }
        up.push(ti);
        for e in 0..3 {
            let (a, b) = (key(t.c[e].pos), key(t.c[(e + 1) % 3].pos));
            let k = if a < b { (a, b) } else { (b, a) };
            let ent = edges.entry(k).or_insert((0, ti));
            ent.0 += 1;
        }
    }
    // coverage: every non-vertical triangle projected to the ground
    let cover: Vec<[[f32; 3]; 3]> = mesh
        .tris
        .iter()
        .filter(|t| face_normal(&[t.c[0].pos, t.c[1].pos, t.c[2].pos])[1].abs() > 0.05)
        .map(|t| [t.c[0].pos, t.c[1].pos, t.c[2].pos])
        .collect();
    let covered = |x: f32, z: f32, y: f32| -> bool {
        cover.iter().any(|p| {
            let e = |a: [f32; 3], b: [f32; 3]| (b[0] - a[0]) * (z - a[2]) - (b[2] - a[2]) * (x - a[0]);
            let (w0, w1, w2) = (e(p[1], p[2]), e(p[2], p[0]), e(p[0], p[1]));
            let inside = (w0 >= -1e-3 && w1 >= -1e-3 && w2 >= -1e-3) || (w0 <= 1e-3 && w1 <= 1e-3 && w2 <= 1e-3);
            inside && p.iter().any(|q| (q[1] - y).abs() < 40.0)
        })
    };
    let mut added = 0;
    let mut new_tris: Vec<Tri> = Vec::new();
    for &ti in &up {
        let t = mesh.tris[ti];
        for e in 0..3 {
            let (pa, pb) = (t.c[e].pos, t.c[(e + 1) % 3].pos);
            let (a, b) = (key(pa), key(pb));
            let k = if a < b { (a, b) } else { (b, a) };
            let Some(&(count, first)) = edges.get(&k) else { continue };
            if count != 1 || first != ti {
                continue;
            }
            let pc = t.c[(e + 2) % 3].pos;
            let mid = [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0, (pa[2] + pb[2]) / 2.0];
            let out = normalize([mid[0] - pc[0], 0.0, mid[2] - pc[2]]);
            let probe = [mid[0] + out[0] * 1.5, mid[2] + out[2] * 1.5];
            if covered(probe[0], probe[1], mid[1]) {
                continue;
            }
            if pa[1] <= ground_y + 0.05 && pb[1] <= ground_y + 0.05 {
                continue;
            }
            // texture density along the source edge (texture repeats per metre)
            let (ua, ub) = (t.c[e].uv, t.c[(e + 1) % 3].uv);
            let len = ((pb[0] - pa[0]).powi(2) + (pb[2] - pa[2]).powi(2)).sqrt().max(0.01);
            let du = ((ub[0] - ua[0]).powi(2) + (ub[1] - ua[1]).powi(2)).sqrt();
            let density = if du > 1e-4 { du / len } else { 0.25 };
            let depth_a = pa[1] - ground_y;
            let depth_b = pb[1] - ground_y;
            let ga = [pa[0], ground_y, pa[2]];
            let gb = [pb[0], ground_y, pb[2]];
            let c = |p: [f32; 3], uv: [f32; 2], rgba: [u8; 4]| Corner { pos: p, uv, rgba };
            let ca = c(pa, [0.0, 0.0], t.c[e].rgba);
            let cb = c(pb, [len * density, 0.0], t.c[(e + 1) % 3].rgba);
            let cga = c(ga, [0.0, depth_a * density], t.c[e].rgba);
            let cgb = c(gb, [len * density, depth_b * density], t.c[(e + 1) % 3].rgba);
            // winding: normal = cross(b−a, c−a) must point along `out`
            let quad = [[ca, cb, cgb], [ca, cgb, cga]];
            for q in quad {
                let n = face_normal(&[q[0].pos, q[1].pos, q[2].pos]);
                let dot = n[0] * out[0] + n[2] * out[2];
                let cc = if dot >= 0.0 { q } else { [q[0], q[2], q[1]] };
                new_tris.push(Tri { c: cc, mat: t.mat, two_sided: false, lit: t.lit, piece: t.piece });
            }
            added += 1;
        }
    }
    mesh.tris.extend(new_tris);
    added
}

/// Vertex-colour census of a mesh: (triangles with one colour at all three
/// corners, triangles with a gradient, distinct corner colours, distinct
/// (material, colour) pairs after quantising to `levels` per channel).
pub fn colour_census(mesh: &Mesh, levels: u32) -> (usize, usize, usize, usize) {
    let q = |c: u8| ((c as u32 * (levels - 1) + 127) / 255) as u8;
    let mut colours = std::collections::HashSet::new();
    let mut pairs = std::collections::HashSet::new();
    let (mut flat, mut grad) = (0, 0);
    for t in &mesh.tris {
        let c: Vec<[u8; 3]> = t.c.iter().map(|k| [k.rgba[0], k.rgba[1], k.rgba[2]]).collect();
        if c[0] == c[1] && c[1] == c[2] {
            flat += 1;
        } else {
            grad += 1;
        }
        for k in &c {
            colours.insert(*k);
            pairs.insert((t.mat, [q(k[0]), q(k[1]), q(k[2])]));
        }
    }
    (flat, grad, colours.len(), pairs.len())
}

/// TM's item shaders ignore vertex colours (`TDSN`/`TDOSN` never read
/// colour0 — verified 2026-09), and MK64 shades everything with them: the
/// tunnel's darkness, the hill's greens, Bowser's Castle's gloom, the sand's
/// warm tint. So the colours go into the TEXTURES: every triangle takes the
/// (texture × tint) variant of its material, tints quantised to `levels` per
/// channel; a triangle whose corners disagree by more than `max_spread` is
/// split along its longest edge (colour, uv, position interpolated) until they
/// do, so gradients become steps finer than the eye picks out on a texture.
/// Untextured faces become `Flat` materials (a tint alone). Lit faces (their
/// "colours" were normals) count as white. Returns (triangles split, variants).
pub fn bake_vertex_colours(mesh: &mut Mesh, levels: u32, max_spread: u8, max_depth: u32) -> (usize, usize) {
    let q = |c: u8| -> u8 {
        let step = 255.0 / (levels - 1) as f32;
        ((c as f32 / step).round() * step).round().clamp(0.0, 255.0) as u8
    };
    let mut variants: HashMap<(Option<usize>, [u8; 3]), usize> = HashMap::new();
    let base_materials = mesh.materials.clone();
    let mut out: Vec<Tri> = Vec::with_capacity(mesh.tris.len() * 2);
    let mut splits = 0usize;
    let tris = std::mem::take(&mut mesh.tris);
    let mut stack: Vec<(Tri, u32)> = Vec::new();
    for t in tris {
        stack.push((t, 0));
        while let Some((mut t, depth)) = stack.pop() {
            if t.lit {
                for c in t.c.iter_mut() {
                    c.rgba = [255, 255, 255, 255];
                }
            }
            let spread = (0..3)
                .map(|ch| {
                    let v: Vec<u8> = t.c.iter().map(|c| c.rgba[ch]).collect();
                    v.iter().max().unwrap() - v.iter().min().unwrap()
                })
                .max()
                .unwrap_or(0);
            if spread > max_spread && depth < max_depth {
                // split the longest edge
                let len = |a: [f32; 3], b: [f32; 3]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
                let e = [len(t.c[0].pos, t.c[1].pos), len(t.c[1].pos, t.c[2].pos), len(t.c[2].pos, t.c[0].pos)];
                let k = if e[0] >= e[1] && e[0] >= e[2] { 0 } else if e[1] >= e[2] { 1 } else { 2 };
                let (a, b, c) = (t.c[k], t.c[(k + 1) % 3], t.c[(k + 2) % 3]);
                let m = Corner {
                    pos: [(a.pos[0] + b.pos[0]) / 2.0, (a.pos[1] + b.pos[1]) / 2.0, (a.pos[2] + b.pos[2]) / 2.0],
                    uv: [(a.uv[0] + b.uv[0]) / 2.0, (a.uv[1] + b.uv[1]) / 2.0],
                    rgba: [
                        ((a.rgba[0] as u16 + b.rgba[0] as u16) / 2) as u8,
                        ((a.rgba[1] as u16 + b.rgba[1] as u16) / 2) as u8,
                        ((a.rgba[2] as u16 + b.rgba[2] as u16) / 2) as u8,
                        255,
                    ],
                };
                splits += 1;
                stack.push((Tri { c: [a, m, c], ..t }, depth + 1));
                stack.push((Tri { c: [m, b, c], ..t }, depth + 1));
                continue;
            }
            let avg = |ch: usize| ((t.c[0].rgba[ch] as u16 + t.c[1].rgba[ch] as u16 + t.c[2].rgba[ch] as u16) / 3) as u8;
            let tint = [q(avg(0)), q(avg(1)), q(avg(2))];
            let key = (t.mat, tint);
            let mi = *variants.entry(key).or_insert_with(|| {
                let mut m = match t.mat {
                    Some(i) => base_materials[i].clone(),
                    None => Material { sym: FLAT_SYM.to_string(), mirror_s: false, mirror_t: false, clamp_s: false, clamp_t: false, w: 4, h: 4, fmt: 0, tint: [255, 255, 255] },
                };
                m.tint = tint;
                mesh.materials.push(m);
                mesh.materials.len() - 1
            });
            t.mat = Some(mi);
            out.push(t);
        }
    }
    // drop the untinted bases nobody uses any more: remap indices
    let used: std::collections::HashSet<usize> = out.iter().filter_map(|t| t.mat).collect();
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut kept: Vec<Material> = Vec::new();
    for (i, m) in mesh.materials.iter().enumerate() {
        if used.contains(&i) {
            remap.insert(i, kept.len());
            kept.push(m.clone());
        }
    }
    for t in out.iter_mut() {
        t.mat = t.mat.map(|i| remap[&i]);
    }
    let n_variants = kept.len();
    mesh.materials = kept;
    mesh.tris = out;
    (splits, n_variants)
}
