//! A CAR SKIN mesh from arbitrary geometry — the skin-scenery study (2026-09-12).
//!
//! What a TM2020 3D skin is, read off the game's own files (the stock
//! `Skins\Models\CarSport\{Rally,Snow,Desert}` folders in
//! `Maniaplanet_ModelsSport.pak`): a zip whose `MainBody.Mesh.gbx` is a bare
//! `CPlugSolid2Model` (class 0x090BB000, v34) — the very class this crate
//! writes for every embedded item, and `solid2-roundtrip` re-emits the stock
//! Rally mesh byte-identically. Its materials are NAMES in `material_ids`
//! (`_<Shading>_<TexSet>`, e.g. `_DetailsDmgNormal_Details`), which the vehicle
//! vis model resolves against the zip's `<TexSet>_{B,R,N,I,AO,DirtMask}.dds`;
//! the shading names the game knows (strings in NadeoImporter.exe /
//! CPlugVehicleVisModel): `SkinDmg`, `SkinDmgDecal`, `SkinNoSkelDmg`,
//! `DetailsDmg`, `DetailsDmgNormal`, `DetailsDmgDecal`, `DetailsNoSkelDmg`,
//! `GlassDmg`, `GlassDmgCrack`, `GlassDmgDecal`, `GlassNoSkelDmg`. The stock
//! vertices carry a per-vertex joint index (element 4, Int32) against the
//! Solid2's `joints` list (30 joints on Rally, `u10` = joints per LOD); the
//! `NoSkel` shadings are the skeleton-free path.
//!
//! This module builds such a mesh from parts of OUR geometry: a probe cube, or
//! the visuals of embedded map items placed in the world and re-expressed in
//! the frame of a parked ghost (`ghost static`). The stock Rally mesh is the
//! TEMPLATE: its root and its first visual are cloned and only the data is
//! swapped (geoms, visuals, streams, bounds, material names, joints) — when a
//! working example exists, diff against it before building.
//!
//! THE SHAPE THAT WORKS (2026-09-12, attempt 2 — measured on the render box with
//! two community zip skins that import and render, `TM2_Stadium` (Nadeo skin
//! storage, v34) and ManiaPark's `Maserati MC20` (v32), then with our own
//! geometry in that shape: a 3 m probe cube, an axis probe, two hill items):
//!
//! * `Binding::Template` + a community mesh as `--template`: the template's
//!   INLINE `CPlugSkel` (v19 from NadeoImporter; 35 joints `Hips, Body, …`),
//!   its `joints` list, `u10`, `vis_cst_type 3`, `0x090BB002` (fake shadow) are
//!   kept verbatim; our visuals replace the geoms; every vertex carries the
//!   Int32 joint word = index of `Body` in that list. ONE LOD (`lod_max_dist`
//!   empty). Vertex layout Position f32x3 · Int32 · Normal dec3n · uv0 f32x2 ·
//!   TangentU dec3n, stream flags 0x13, visual chunk flags 0x39, `SkinData`
//!   present and empty. Our writer round-trips both community meshes
//!   byte-identically.
//! * THE ZIP MUST CARRY TEXTURES FOR THE Skin, Wheels AND Glass SETS TOO, not
//!   only the set our material uses: `Details_*` alone crashed the client on
//!   import (int3 through a destroyed object's vtable, `exe+0x124520` via
//!   `+0x12b79f`) with 7 material names AND with the one name we use; the same
//!   mesh with `Skin_*`/`Wheels_*`/`Glass_*` copies of the six Details files
//!   imported and rendered (hillB-min / X2 vs X1 / X3). `TM2_Stadium` ships
//!   Wheels + Glass_I and no Skin_* and works, so Skin_* may be optional.
//! * The first attempt's crashes were something else: the STOCK pak Rally
//!   mesh in a zip (5 LODs, no inline skel or the v20 54-joint one) died in a
//!   GPU-buffer create that returned NULL (`exe+0xa86da6`: count 0x47, then
//!   AddRef without a check); `NoSkel` materials without joints hit the
//!   destroyed-object path above. Neither shape is what the community ships.
//! * Skin-frame axes as drawn (axis probe `--cubes`): +z = car front, +x = the
//!   car's LEFT (screen-left when the chase camera looks along +z), y up; the
//!   geometry is drawn at ghost position + local, no scale.
//! * Materials: `_DetailsDmgNormal_<Set>` is plain PBR from `<Set>_{B,N,R,I,AO,
//!   DirtMask}.dds`; a `--flat RRGGBB` set is 64x64 BC3 + 4x4 flats (1.2 KB).

use super::solid2::{CPlugSolid2Model, ShadedGeom};
use super::visual::{CPlugVisualIndexedTriangles, IndexBuffer, SkinData};
use super::vstream::{CPlugVertexStream, Decl, Elem};
use super::{Id, LookbackState, Node, NodeRef, Rd, Wr, R};

/// One draw of one texture set.
#[derive(Clone, Debug, Default)]
pub struct Part {
    pub texset: String,
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub idx: Vec<u32>,
    /// where the vertices came from, for the report
    pub source: String,
}

impl Part {
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in &self.pos {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    }
}

/// How the mesh binds to the car.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Binding {
    /// `_<Family>NoSkelDmg_<Set>` materials, no joints, no per-vertex joint element.
    NoSkel,
    /// `_<Family>Dmg_<Set>` / `_DetailsDmgNormal_<Set>` materials, joints = ["Body"],
    /// every vertex bound to joint 0 — "everything bound to the body".
    Body,
    /// THE WORKING SHAPE (2026-09-12, measured on the community `TM2_Stadium`
    /// zip skin that imports and renders): keep the template's skeleton
    /// (inline `CPlugSkel`), its `joints` list, `u10`, its `material_ids` and
    /// every other root field verbatim; bind every vertex to the template's
    /// `Body` joint by index; our geometry replaces only the geoms/visuals.
    Template,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub binding: Binding,
    /// Skin | Details | Glass — the shading family (Details = plain PBR with a
    /// normal map; Skin = the hue-shifted car paint).
    pub family: String,
    /// The visual's chunk flag word (the stock Rally mesh: 0x39).
    pub vis_flags: u32,
    /// Keep the template's `SkinData` block in each visual.
    pub skin_data: bool,
    /// `lod_max_dist` for the single level (empty = one level, no ladder).
    pub lod_max_dist: Vec<f32>,
    /// Largest vertex count per visual (delta index buffers are i16 steps).
    pub max_verts: usize,
    /// A skeleton to carry INLINE in the Solid2 `skel` slot (a zip skin needs one; the
    /// stock pak meshes leave it null and ship MainBody.Skel.Gbx instead).
    pub skel: Option<super::skel::CPlugSkel>,
    /// Template mode: drop the template's material names our geometry does not use
    /// (a name in `material_ids` makes the vehicle resolve THAT texture set too —
    /// the 2026-09-12 hillB-min crash: 7 names, textures for one set).
    pub prune_materials: bool,
    /// `SET=_FullMaterialName` overrides for `material_name` (e.g. the cards under
    /// `_GlassDmgCrack_Glass`, the one glass name both community skins carry).
    pub material_override: Vec<(String, String)>,
}

impl Default for Options {
    fn default() -> Options {
        Options { binding: Binding::NoSkel, family: "Details".into(), vis_flags: 0x39, skin_data: true, lod_max_dist: Vec::new(), max_verts: 32_000, skel: None, prune_materials: false, material_override: Vec::new() }
    }
}

/// The material name the vehicle vis model resolves.
pub fn material_name(o: &Options, texset: &str) -> String {
    if let Some((_, n)) = o.material_override.iter().find(|(s, _)| s == texset) {
        return n.clone();
    }
    match (o.binding, o.family.as_str()) {
        (Binding::NoSkel, f) => format!("_{f}NoSkelDmg_{texset}"),
        (Binding::Body | Binding::Template, "Details") => format!("_DetailsDmgNormal_{texset}"),
        (Binding::Body | Binding::Template, f) => format!("_{f}Dmg_{texset}"),
    }
}

/// A probe cube of edge `size`, standing on y = 0 centred on x = z = 0, one
/// texture tile per face (so UV wrap vs clamp and orientation are readable in a
/// frame: +z is the car's front).
pub fn cube(size: f32, texset: &str) -> Part {
    let h = size / 2.0;
    let mut p = Part { texset: texset.into(), source: format!("cube {size} m"), ..Default::default() };
    // faces: (normal, four corners CCW seen from outside)
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0, 0.0, 1.0], [[-h, 0.0, h], [h, 0.0, h], [h, size, h], [-h, size, h]]),       // front (+z)
        ([0.0, 0.0, -1.0], [[h, 0.0, -h], [-h, 0.0, -h], [-h, size, -h], [h, size, -h]]),  // back
        ([1.0, 0.0, 0.0], [[h, 0.0, h], [h, 0.0, -h], [h, size, -h], [h, size, h]]),       // right (+x)
        ([-1.0, 0.0, 0.0], [[-h, 0.0, -h], [-h, 0.0, h], [-h, size, h], [-h, size, -h]]),  // left
        ([0.0, 1.0, 0.0], [[-h, size, h], [h, size, h], [h, size, -h], [-h, size, -h]]),   // top
        ([0.0, -1.0, 0.0], [[-h, 0.0, -h], [h, 0.0, -h], [h, 0.0, h], [-h, 0.0, h]]),      // bottom
    ];
    for (n, c) in faces.iter() {
        let b = p.pos.len() as u32;
        for (k, v) in c.iter().enumerate() {
            p.pos.push(*v);
            p.nrm.push(*n);
            p.uv.push(match k { 0 => [0.0, 1.0], 1 => [1.0, 1.0], 2 => [1.0, 0.0], _ => [0.0, 0.0] });
        }
        p.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    p
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 { [0.0, 1.0, 0.0] } else { [v[0] / l, v[1] / l, v[2] / l] }
}

/// A tangent per vertex from the normal alone (any unit vector orthogonal to
/// it): the stock Details visuals carry TangentU as Dec3N and the shader takes
/// the bitangent from N x T. Without authored UV derivatives this is the
/// honest choice for a probe; a real bake would derive it from the UVs.
fn tangent_for(n: [f32; 3]) -> [f32; 3] {
    let a = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 0.0, 1.0] };
    // t = a - (a.n) n
    let d = a[0] * n[0] + a[1] * n[1] + a[2] * n[2];
    normalize([a[0] - d * n[0], a[1] - d * n[1], a[2] - d * n[2]])
}

/// Split a part into pieces of at most `max` vertices (triangle-wise).
fn split_part(p: &Part, max: usize) -> Vec<Part> {
    if p.pos.len() <= max {
        return vec![p.clone()];
    }
    let mut out = Vec::new();
    let mut cur = Part { texset: p.texset.clone(), source: p.source.clone(), ..Default::default() };
    let mut remap: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for tri in p.idx.chunks(3) {
        if cur.pos.len() + 3 > max {
            out.push(std::mem::replace(&mut cur, Part { texset: p.texset.clone(), source: p.source.clone(), ..Default::default() }));
            remap.clear();
        }
        for &i in tri {
            let ni = *remap.entry(i).or_insert_with(|| {
                cur.pos.push(p.pos[i as usize]);
                cur.nrm.push(p.nrm[i as usize]);
                cur.uv.push(p.uv[i as usize]);
                (cur.pos.len() - 1) as u32
            });
            cur.idx.push(ni);
        }
    }
    if !cur.idx.is_empty() {
        out.push(cur);
    }
    out
}

/// Load the template mesh (the stock Rally `MainBody.Mesh.gbx` body bytes).
pub fn template_from_body(body: &[u8]) -> R<CPlugSolid2Model> {
    let mut r = Rd::new(body, 0, LookbackState::default());
    CPlugSolid2Model::parse(&mut r)
}

pub struct Built {
    pub file: Vec<u8>,
    pub visuals: usize,
    pub vertices: usize,
    pub triangles: usize,
    pub materials: Vec<String>,
    pub bounds: ([f32; 3], [f32; 3]),
}

/// The skin mesh file bytes.
pub fn build(template: &CPlugSolid2Model, parts: &[Part], o: &Options) -> R<Built> {
    if parts.is_empty() {
        return Err("no parts".into());
    }
    let tv = template
        .visuals
        .iter()
        .find_map(|v| match v.inline.as_deref() {
            Some(Node::Visual(vis)) => Some(vis.clone()),
            _ => None,
        })
        .ok_or("the template has no inline visual")?;
    let mut s2 = template.clone();
    let tpl_mode = o.binding == Binding::Template;
    // Template mode: the skeleton and joint list are the working skin's own;
    // the vertex joint word is the index of `Body` in THAT list (1 on the
    // NadeoImporter shape: Hips, Body, …).
    let body_joint: u32 = if tpl_mode {
        template
            .joints
            .iter()
            .position(|j| j.as_str() == Some("Body"))
            .ok_or("template mode: the template's joints list has no `Body`")? as u32
    } else {
        0
    };
    let tpl_skel: Option<Node> = if tpl_mode { template.skel.inline.as_deref().cloned() } else { None };
    if tpl_mode && tpl_skel.is_none() {
        return Err("template mode: the template carries no inline skeleton".into());
    }
    s2.shaded_geoms.clear();
    s2.visuals.clear();
    if !tpl_mode || o.prune_materials {
        s2.material_ids.clear();
    }
    s2.custom_materials.clear();
    s2.materials.clear();
    s2.lod_max_dist = o.lod_max_dist.clone();
    s2.lights.clear();
    s2.light_user_models.clear();
    s2.light_insts.clear();
    s2.pre_light_gen = None;
    if !tpl_mode {
        s2.joints = match o.binding {
            Binding::NoSkel => Vec::new(),
            _ => vec![Id::Str("Body".into())],
        };
        s2.u10 = match o.binding {
            Binding::NoSkel => Vec::new(),
            _ => vec![1],
        };
    }
    // texture sets in first-use order -> material ids
    let mut sets: Vec<String> = Vec::new();
    for p in parts {
        if !sets.contains(&p.texset) {
            sets.push(p.texset.clone());
        }
    }
    let mut set_material: Vec<i32> = Vec::new();
    for s in &sets {
        let name = material_name(o, s);
        let pos = s2.material_ids.iter().position(|i| i.as_str() == Some(name.as_str()));
        let mi = match pos {
            Some(i) => i as i32,
            None => {
                s2.material_ids.push(Id::Str(name));
                (s2.material_ids.len() - 1) as i32
            }
        };
        set_material.push(mi);
    }
    let mut next: i32 = 1;
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    let (mut nverts, mut ntris, mut nvis) = (0usize, 0usize, 0usize);
    for p in parts {
        for piece in split_part(p, o.max_verts.max(3)) {
            let n = piece.pos.len();
            if n == 0 || piece.idx.is_empty() {
                continue;
            }
            let (plo, phi) = piece.bounds();
            for k in 0..3 {
                lo[k] = lo[k].min(plo[k]);
                hi[k] = hi[k].max(phi[k]);
            }
            // ---- the vertex stream
            let bone = o.binding != Binding::NoSkel;
            let stride_words: u32 = if bone { 8 } else { 7 };
            let mut decls = Vec::new();
            let mut elems = Vec::new();
            let mut off = 0u32;
            decls.push(Decl::with_stride(super::vstream::N_POSITION, super::vstream::T_FLOAT3, super::vstream::SPACE_GLOBAL3D, off, stride_words));
            elems.push(Elem::Float3(piece.pos.clone()));
            off += 12;
            if bone {
                decls.push(Decl::with_stride(4, super::vstream::T_INT32, super::vstream::SPACE_GLOBAL2D, off, stride_words));
                elems.push(Elem::Word(vec![body_joint; n]));
                off += 4;
            }
            decls.push(Decl::with_stride(super::vstream::N_NORMAL, super::vstream::T_DEC3N, super::vstream::SPACE_LOCAL3D, off, stride_words));
            elems.push(Elem::Word(piece.nrm.iter().map(|nn| super::merged::dec3n_pack(normalize(*nn))).collect()));
            off += 4;
            decls.push(Decl::with_stride(super::vstream::N_TEXCOORD0, super::vstream::T_FLOAT2, super::vstream::SPACE_GLOBAL2D, off, stride_words));
            elems.push(Elem::Float2(piece.uv.clone()));
            off += 8;
            decls.push(Decl::with_stride(super::vstream::N_TANGENT_U, super::vstream::T_DEC3N, super::vstream::SPACE_LOCAL3D, off, stride_words));
            elems.push(Elem::Word(piece.nrm.iter().map(|nn| super::merged::dec3n_pack(tangent_for(normalize(*nn)))).collect()));
            off += 4;
            debug_assert_eq!(off, stride_words * 4);
            let stream = CPlugVertexStream { version: 1, count: n as i32, flags: 0x13, base: super::null_ref(), decls, compress_local3d: Some(true), elems };
            // ---- the visual, cloned from the template
            let mut v: CPlugVisualIndexedTriangles = tv.clone();
            let main = v.main.as_mut().ok_or("template visual has no main chunk")?;
            main.count = n as i32;
            main.chunk_flags = o.vis_flags;
            main.vertex_streams = vec![NodeRef { index: next + 1, inline: Some(Box::new(Node::VertexStream(stream))) }];
            main.skin = if o.skin_data { Some(SkinData { u01: false, u02: 0, u03: false, u04: false, bones: Vec::new(), u07: Vec::new() }) } else { None };
            main.tex_coord_sets.clear();
            main.uv_groups.clear();
            main.bitmap_elems.clear();
            let c = [(plo[0] + phi[0]) / 2.0, (plo[1] + phi[1]) / 2.0, (plo[2] + phi[2]) / 2.0];
            let e = [(phi[0] - plo[0]) / 2.0, (phi[1] - plo[1]) / 2.0, (phi[2] - plo[2]) / 2.0];
            main.bounding_box = [c[0], c[1], c[2], e[0], e[1], e[2]];
            v.index_buffer = Some(IndexBuffer { chunk: 0x09057001, flags: 2, indices: piece.idx.clone() });
            v.sub_visuals.clear();
            v.splits.clear();
            let mi = set_material[sets.iter().position(|s| *s == piece.texset).unwrap()];
            s2.shaded_geoms.push(ShadedGeom { visual_index: s2.visuals.len() as i32, material_index: mi, u01: -1, lod_mask: 1, u02: 0 });
            s2.visuals.push(NodeRef { index: next, inline: Some(Box::new(Node::Visual(v))) });
            next += 2;
            nverts += n;
            ntris += piece.idx.len() / 3;
            nvis += 1;
        }
    }
    if nvis == 0 {
        return Err("every part was empty".into());
    }
    if let Some(sk) = tpl_skel {
        s2.skel = NodeRef { index: next, inline: Some(Box::new(sk)) };
        next += 1;
    } else if let Some(sk) = &o.skel {
        s2.skel = NodeRef { index: next, inline: Some(Box::new(Node::Skel(sk.clone()))) };
        next += 1;
    } else {
        s2.skel = super::null_ref();
    }
    let mut body = Vec::new();
    {
        let mut lb = LookbackState::default();
        let mut w = Wr { w: &mut body, lb: &mut lb };
        s2.write(&mut w);
    }
    let file = super::file::write_node_file(0x090BB000, &body, next as u32, &[]);
    Ok(Built { file, visuals: nvis, vertices: nverts, triangles: ntris, materials: s2.material_ids.iter().map(|i| i.as_str().unwrap_or("?").to_string()).collect(), bounds: (lo, hi) })
}

/// The geometry of one embedded item file (LOD0 visuals of its static
/// object's Solid2), one Part per (visual, material link), in MODEL space.
/// `material_of` maps a material link to a texture set (None drops the visual).
pub fn item_parts(bytes: &[u8], name: &str, material_of: &dyn Fn(&str) -> Option<String>) -> R<Vec<(String, Part)>> {
    let f = super::file::parse_file(bytes).map_err(|e| format!("{name}: {e}"))?;
    let model = crate::store::Model::parse(bytes, name).map_err(|e| format!("{name}: {e}"))?;
    let so = f.item.static_object().ok_or_else(|| format!("{name}: no static object (a moving/prefab item)"))?;
    let s2 = so.solid2().ok_or_else(|| format!("{name}: static object without an inline Solid2"))?;
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        if g.lod_mask != 0 && g.lod_mask & 1 == 0 {
            continue;
        }
        // the material link
        let link: String = if !s2.custom_materials.is_empty() {
            match s2.custom_materials.get(g.material_index as usize) {
                Some(m) if !m.name.is_empty() => m.name.clone(),
                Some(m) => m
                    .inst()
                    .and_then(|i| {
                        i.link().map(|s| s.to_string()).or_else(|| {
                            let mm = i.main.as_ref()?;
                            mm.material_name.as_str().map(|s| s.to_string()).or_else(|| if mm.base_texture.is_empty() { None } else { Some(mm.base_texture.clone()) })
                        })
                    })
                    .unwrap_or_else(|| format!("inline#{}", g.material_index)),
                None => format!("mat#{}", g.material_index),
            }
        } else if let Some(r) = s2.materials.get(g.material_index as usize) {
            model.externals.iter().find(|(i, _)| *i as i32 == r.index).map(|(_, p)| p.trim_end_matches(".Material.Gbx").to_string()).unwrap_or_else(|| format!("ext#{}", r.index))
        } else if let Some(i) = s2.material_ids.get(g.material_index as usize) {
            i.as_str().unwrap_or("?").to_string()
        } else {
            format!("mat#{}", g.material_index)
        };
        let Some(texset) = material_of(&link) else { continue };
        let Some(vr) = s2.visuals.get(g.visual_index as usize) else { continue };
        let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
        let Some(main) = v.main.as_ref() else { continue };
        let n = main.count.max(0) as usize;
        let mut part = Part { texset, source: format!("{name} geom v{} {link}", g.visual_index), ..Default::default() };
        let mut got_pos = false;
        for sr in &main.vertex_streams {
            let Some(Node::VertexStream(s)) = sr.inline.as_deref() else { continue };
            let comp = s.compress_local3d.unwrap_or(false);
            for (di, d) in s.decls.iter().enumerate() {
                let Some(e) = s.elems.get(di) else { continue };
                match (d.name(), e) {
                    (super::vstream::N_POSITION, Elem::Float3(v)) => {
                        part.pos = v.clone();
                        got_pos = true;
                    }
                    (super::vstream::N_NORMAL, Elem::Float3(v)) => part.nrm = v.clone(),
                    (super::vstream::N_NORMAL, Elem::Word(w)) if d.stored_type(comp) == super::vstream::T_DEC3N => {
                        part.nrm = w.iter().map(|x| super::merged::dec3n_unpack(*x)).collect();
                    }
                    (super::vstream::N_TEXCOORD0, Elem::Float2(v)) => part.uv = v.clone(),
                    _ => {}
                }
            }
        }
        if !got_pos {
            continue;
        }
        if part.uv.len() != n {
            if let Some(tc) = main.tex_coord_sets.first() {
                part.uv = tc.coords.iter().map(|(uv, _, _)| *uv).collect();
            }
        }
        if part.uv.len() != n {
            part.uv = vec![[0.0, 0.0]; n];
        }
        if part.nrm.len() != n {
            part.nrm = vec![[0.0, 1.0, 0.0]; n];
        }
        part.idx = v.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        if part.idx.is_empty() || part.pos.len() != n {
            continue;
        }
        out.push((link, part));
    }
    Ok(out)
}

/// Place a model-space part in the world (`xf`, the item's placement) and
/// re-express it in the skin frame (world minus the ghost anchor).
pub fn place(part: &Part, xf: &crate::geom::Xform, anchor: [f32; 3]) -> Part {
    let mut p = part.clone();
    let rot: crate::geom::Xform = [xf[0], xf[1], xf[2], xf[3], xf[4], xf[5], xf[6], xf[7], xf[8], 0.0, 0.0, 0.0];
    for v in p.pos.iter_mut() {
        let w = crate::geom::apply(xf, *v);
        *v = [w[0] - anchor[0], w[1] - anchor[1], w[2] - anchor[2]];
    }
    for nn in p.nrm.iter_mut() {
        *nn = normalize(crate::geom::apply(&rot, *nn));
    }
    p
}

/// Planar UVs from world XZ (metres per texture tile).
pub fn planar_uv(part: &mut Part, anchor: [f32; 3], metres_per_tile: f32) {
    for (i, p) in part.pos.iter().enumerate() {
        part.uv[i] = [(p[0] + anchor[0]) / metres_per_tile, (p[2] + anchor[2]) / metres_per_tile];
    }
}

// ---------------------------------------------------------------- textures

/// An RGBA8 checker (`cells` per side) with a distinct colour per cell row,
/// an arrow marking +u, and a corner marker at (0,0): every frame of it says
/// which way the texture lies and whether it repeats.
pub fn checker_rgba(size: u32, cells: u32) -> Vec<u8> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    let cell = (size / cells).max(1);
    for y in 0..size {
        for x in 0..size {
            let (cx, cy) = (x / cell, y / cell);
            let dark = (cx + cy) % 2 == 0;
            let (mut r, mut g, mut b) = if dark { (40u8, 40u8, 40u8) } else { (220u8, 220u8, 220u8) };
            // row tint: top rows red, bottom rows blue (v = 0 is the top of a DDS)
            let t = y as f32 / size as f32;
            if !dark {
                r = (220.0 * (1.0 - t) + 60.0 * t) as u8;
                g = 200;
                b = (60.0 * (1.0 - t) + 220.0 * t) as u8;
            }
            // corner marker: a green square at (u,v) = (0,0)
            if x < cell / 2 && y < cell / 2 {
                (r, g, b) = (0, 255, 0);
            }
            // +u arrow along the middle row: a magenta band brightening toward +u
            if y >= size / 2 - cell / 8 && y < size / 2 + cell / 8 {
                let k = x as f32 / size as f32;
                (r, g, b) = ((255.0 * k) as u8, 0, (255.0 * k) as u8);
            }
            let o = ((y * size + x) * 4) as usize;
            px[o] = r;
            px[o + 1] = g;
            px[o + 2] = b;
            px[o + 3] = 255;
        }
    }
    px
}

pub fn flat_rgba(size: u32, rgba: [u8; 4]) -> Vec<u8> {
    (0..size * size).flat_map(|_| rgba).collect()
}

/// The six textures of one set from a base-colour RGBA image: `_B` the colour,
/// `_N` a flat normal, `_R` mid roughness, `_I` no emission, `_AO` none,
/// `_DirtMask` clean. BC3 with mips for the colour; small flats uncompressed.
pub fn texture_set(set: &str, w: u32, h: u32, base_rgba: &[u8], roughness: u8) -> Vec<(String, Vec<u8>)> {
    let levels = super::texture::mip_chain(super::texture::Level { w, h, rgba: base_rgba.to_vec() }, 128, false, 1.0);
    let b = super::texture::write_dds_dxt5_mips(&levels);
    let flat = |rgba: [u8; 4]| super::texture::write_dds_rgba(4, 4, &flat_rgba(4, rgba));
    let mut out = Vec::new();
    if set == "Glass" {
        // the community Glass sets are `_D` (MC20) or `_I` (TM2) + `_T`: write the
        // base under both names and the alpha channel as `_T` (a guess at
        // "transparency": white = opaque) — the foliage-cards probe of 2026-09-12
        let alpha: Vec<u8> = base_rgba.chunks(4).flat_map(|p| [p[3], p[3], p[3], 255u8]).collect();
        let t_levels = super::texture::mip_chain(super::texture::Level { w, h, rgba: alpha }, 128, false, 1.0);
        out.push((format!("{set}_D.dds"), b.clone()));
        out.push((format!("{set}_T.dds"), super::texture::write_dds_dxt5_mips(&t_levels)));
    }
    out.extend(vec![
        (format!("{set}_B.dds"), b),
        (format!("{set}_N.dds"), flat([128, 128, 255, 255])),
        (format!("{set}_R.dds"), flat([roughness, roughness, roughness, 255])),
        (format!("{set}_I.dds"), flat([0, 0, 0, 255])),
        (format!("{set}_AO.dds"), flat([255, 255, 255, 255])),
        (format!("{set}_DirtMask.dds"), flat([0, 0, 0, 255])),
    ]);
    out
}
