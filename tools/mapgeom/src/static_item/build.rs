//! Building static items (S3): every `CPlugStaticObjectModel` of a prefab
//! (recursively through its external prefabs), or a Nadeo item's own static
//! object / baked crystal, merged into ONE `CPlugSolid2Model` + ONE
//! `CPlugSurface` and written in the reference `.Item.Gbx` layout.
//!
//! * visuals are appended as they are (their own vertex layout, index
//!   buffer and flags), positions transformed by the entity iso and scaled,
//!   packed normals/tangents rotated, bounding boxes recomputed;
//! * materials are `CPlugMaterialUserInst` game materials deduplicated by
//!   (link, physics id);
//! * the collision mesh is the union of the entity surfaces (triangle
//!   physics id kept, the u16 id list deduplicated), or the visual triangles
//!   of a mesh-collidable object.

use super::solid2::{CPlugSolid2Model, Material, PreLightGen, ShadedGeom};
use super::surface::{CPlugSurface, Surf, Triangle};
use super::visual::CPlugVisualIndexedTriangles;
use super::vstream::{Elem, N_NORMAL, N_POSITION, N_TANGENT_U, N_TANGENT_V, T_DEC3N, T_FLOAT3};
use super::{Node, NodeRef, Ref, R};
use crate::crystal_model::CPlugMaterialUserInst;
use crate::geom::{apply, compose, Xform, IDENTITY};

/// What the built item is called and how big it is.
#[derive(Clone, Debug)]
pub struct BuildOpts {
    /// `Ident` path, e.g. `ZZZ_TinyBlocks\Tiny\Road\Tiny_Road_01.Item.Gbx`.
    pub ident: String,
    pub author: String,
    pub scale: f32,
    /// Collection id (26 = Stadium).
    pub collection: u32,
    /// Remap every material link onto the mesh-editor family (BlueBay
    /// embedded items: only `Editors\...` links are known to render there).
    pub editors: bool,
}

/// One source visual + the material slot it draws with.
#[derive(Clone, Debug)]
pub struct MergedVisual {
    pub visual: CPlugVisualIndexedTriangles,
    pub material: usize,
}

/// The accumulator.
#[derive(Clone, Debug, Default)]
pub struct Merged {
    pub visuals: Vec<MergedVisual>,
    /// Deduplicated by (link, physics).
    pub materials: Vec<CPlugMaterialUserInst>,
    pub surf_vertices: Vec<[f32; 3]>,
    pub surf_triangles: Vec<Triangle>,
    pub surf_ids: Vec<u16>,
    pub pre_light_gen: Option<PreLightGen>,
    pub file_write_time: u64,
    /// A trigger surface carried over from a source item (scaled), and its
    /// waypoint type (chunk 2E00201F; 3 = none).
    pub trigger: Option<CPlugSurface>,
    /// Explicit waypoint type (0 start, 1 finish, 2 checkpoint, 4 start+finish);
    /// None writes 3 (not a waypoint). A start has a type and NO trigger.
    pub waypoint_type: Option<i32>,
    /// Entity-model iso translation: the spawn point for waypoint items
    /// (Granady: block spawn_loc x scale, e.g. RoadTechStart [16,2,16] ->
    /// [8,1,8]). Zero = identity.
    pub spawn: [f32; 3],
    /// Things skipped, for the report.
    pub notes: Vec<String>,
    /// Remap every material link onto the mesh-editor family (BlueBay).
    pub editors: bool,
    /// The crystal bake built `surf_vertices`/`surf_triangles`/`surf_ids`
    /// itself (per-slot entries, trigger synthesis); skip the shared
    /// dedup-and-weld tail in `add_crystal`.
    pub surface_built: bool,
}

pub fn dec3n_unpack(v: u32) -> [f32; 3] {
    let c = |s: u32| -> f32 {
        let x = ((v >> s) & 0x3FF) as i32;
        let x = if x >= 512 { x - 1024 } else { x };
        x as f32 / 511.0
    };
    [c(0), c(10), c(20)]
}

pub fn dec3n_pack(n: [f32; 3]) -> u32 {
    let mut out = 0u32;
    for (k, x) in n.iter().enumerate() {
        // Truncation toward zero (C-style `(int)(x*511)`), NOT round:
        // fitted against Tiny_Road_17 (modefit: trunc beats round 612-298
        // on TSpecials, Sign 214/214 exact; axis-aligned components are
        // unaffected since their fractions are 0).
        let q = (x.clamp(-1.0, 1.0) * 511.0) as i32;
        out |= ((q & 0x3FF) as u32) << (10 * k);
    }
    out
}

fn rotate(m: &Xform, v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[3] * v[1] + m[6] * v[2],
        m[1] * v[0] + m[4] * v[1] + m[7] * v[2],
        m[2] * v[0] + m[5] * v[1] + m[8] * v[2],
    ]
}

fn is_identity_rotation(m: &Xform) -> bool {
    m[..9].iter().zip(IDENTITY[..9].iter()).all(|(a, b)| (a - b).abs() < 1e-6)
}

fn bbox(points: &[[f32; 3]]) -> [f32; 6] {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if points.is_empty() {
        return [0.0; 6];
    }
    [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, (hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0, (hi[2] - lo[2]) / 2.0]
}

/// Transform a visual's inline vertex stream in place: positions by `iso`
/// then `scale`, packed normals/tangents by the rotation; bounding box
/// recomputed.
pub fn transform_visual(v: &mut CPlugVisualIndexedTriangles, iso: &Xform, scale: f32) -> R<()> {
    let m = v.main.as_mut().ok_or("visual without chunk 0x0900600F")?;
    let stream = match m.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
        Some(Node::VertexStream(s)) => s,
        _ => return Err("visual without an inline vertex stream".into()),
    };
    let compress = stream.compress_local3d.unwrap_or(false);
    let rot = !is_identity_rotation(iso);
    let mut positions = Vec::new();
    for (d, e) in stream.decls.iter().zip(stream.elems.iter_mut()) {
        match (d.name(), d.stored_type(compress), e) {
            (N_POSITION, T_FLOAT3, Elem::Float3(p)) => {
                for q in p.iter_mut() {
                    let t = apply(iso, *q);
                    *q = [t[0] * scale, t[1] * scale, t[2] * scale];
                }
                positions = p.clone();
            }
            (N_POSITION, _, _) => return Err("packed (Dec3N) positions cannot be transformed".into()),
            (N_NORMAL | N_TANGENT_U | N_TANGENT_V, T_DEC3N, Elem::Word(w)) if rot => {
                for x in w.iter_mut() {
                    *x = dec3n_pack(rotate(iso, dec3n_unpack(*x)));
                }
            }
            (N_NORMAL | N_TANGENT_U | N_TANGENT_V, T_FLOAT3, Elem::Float3(p)) if rot => {
                for q in p.iter_mut() {
                    *q = rotate(iso, *q);
                }
            }
            _ => {}
        }
    }
    m.bounding_box = bbox(&positions);
    Ok(())
}

impl Merged {
    /// Slot of a game material, adding it when new.
    pub fn material_slot(&mut self, link: &str, physics: u8) -> usize {
        if let Some(i) = self.materials.iter().position(|m| m.link() == Some(link) && m.physics() == physics) {
            return i;
        }
        self.materials.push(CPlugMaterialUserInst::game_material(link, physics));
        self.materials.len() - 1
    }

    /// Slot of a link, remapped onto the editor family when asked.
    pub fn link_slot(&mut self, link: &str, physics: u8, editors: bool) -> usize {
        let link = if editors { crate::tiny_assets::editors_link_for_stadium_material(link.rsplit('\\').next().unwrap_or(link)).to_string() } else { link.to_string() };
        self.material_slot(&link, physics)
    }

    /// Slot of a material instance copied from a source (deduplicated by
    /// link + physics like the rest).
    pub fn material_inst_slot(&mut self, inst: &CPlugMaterialUserInst) -> usize {
        let link = inst.link().unwrap_or("").to_string();
        if let Some(i) = self.materials.iter().position(|m| m.link() == Some(link.as_str()) && m.physics() == inst.physics()) {
            return i;
        }
        self.materials.push(inst.clone());
        self.materials.len() - 1
    }

    /// Slot for a crystal material instance with the editor-resolved link
    /// ([`resolve_crystal_link`]; deduplicated by resolved link + physics).
    pub fn resolved_inst_slot(&mut self, inst: &CPlugMaterialUserInst) -> usize {
        let link = resolve_crystal_link(inst.link().unwrap_or("")).to_string();
        if let Some(i) = self.materials.iter().position(|m| m.link() == Some(link.as_str()) && m.physics() == inst.physics()) {
            return i;
        }
        let mut owned = inst.clone();
        if let Some(main) = owned.main.as_mut() {
            main.link = crate::crystal_model::Id::Str(link);
        }
        self.materials.push(owned);
        self.materials.len() - 1
    }

    pub fn surf_id_slot(&mut self, physics: u16) -> i16 {
        if let Some(i) = self.surf_ids.iter().position(|x| *x == physics) {
            return i as i16;
        }
        self.surf_ids.push(physics);
        (self.surf_ids.len() - 1) as i16
    }

    /// Append collision triangles (already in source space).
    pub fn add_surface_mesh(&mut self, vertices: &[[f32; 3]], triangles: &[Triangle], iso: &Xform, scale: f32) {
        let base = self.surf_vertices.len() as u32;
        for v in vertices {
            let t = apply(iso, *v);
            self.surf_vertices.push([t[0] * scale, t[1] * scale, t[2] * scale]);
        }
        for t in triangles {
            // The u16 list entry is physics | gameplay << 8 (Z_Mini_Pltf_Flat_Turbo1_Grass:
            // Green (76) with Turbo (1) is listed as 332).
            let si = self.surf_id_slot(t.material_id as u16 | ((t.u03 as u16) << 8));
            self.surf_triangles.push(Triangle { indices: [t.indices[0] + base, t.indices[1] + base, t.indices[2] + base], material_id: t.material_id, u03: t.u03, surface_index: si });
        }
    }
}

/// Resolves an external node (by node index) to (path, link, physics).
pub type MaterialResolver<'a> = dyn FnMut(i32) -> Option<(String, String, u8)> + 'a;

impl Merged {
    /// Slot for a pre-UserInst material: the first non-shared
    /// `.Material.Gbx` it stands for gives the link (shared Techno3 id bases
    /// match first in file order); its SurfaceId, the library table, then the
    /// object's most common collision physics give the physics.
    pub fn old_material_slot(&mut self, om: &super::oldmat::OldMaterial, resolve: &mut MaterialResolver, common: Option<u8>) -> Option<usize> {
        let mut found = None;
        let mut shared = None;
        for ri in &om.refs {
            if let Some((path, _, _)) = resolve(*ri) {
                if path.to_ascii_lowercase().ends_with(".material.gbx") {
                    if path.to_ascii_lowercase().contains("techno3") {
                        if shared.is_none() {
                            shared = Some(material_link(&path));
                        }
                    } else {
                        found = Some(material_link(&path));
                        break;
                    }
                }
            }
        }
        match found.or(shared) {
            Some(link) => {
                let mut phys = if om.physics != 0 { om.physics } else { 0 };
                if phys == 0 {
                    phys = physics_for_link(&link).or(common).unwrap_or(0);
                }
                Some(self.link_slot(&link, phys, self.editors))
            }
            None => {
                self.notes.push("old material with no .Material.Gbx ref".into());
                None
            }
        }
    }

    /// Add every visual and the collision shape of one static object, placed
    /// by `iso` and scaled.
    pub fn add_static_object(&mut self, so: &super::item::CPlugStaticObjectModel, iso: &Xform, scale: f32, resolve: &mut MaterialResolver) -> R<()> {
        let s2 = so.solid2().ok_or("static object without an inline CPlugSolid2Model")?;
        if self.pre_light_gen.is_none() {
            self.pre_light_gen = s2.pre_light_gen.clone();
        }
        if self.file_write_time == 0 {
            self.file_write_time = s2.file_write_time;
        }
        // Material slot per source material index. Surface votes only ever
        // replace pre-UserInst (old) materials: shared Techno3 id bases
        // carry no look, so the coincident surface material wins over direct
        // resolution. UserInst links are never overridden (validated 1:1).
        let (votes, smap) = surface_votes(so, s2, resolve);
        let mut slots: Vec<Option<usize>> = Vec::new();
        if !s2.custom_materials.is_empty() {
            for (mi, m) in s2.custom_materials.iter().enumerate() {
                let is_old = matches!(m.node.as_ref().and_then(|r| r.inline.as_deref()), Some(super::Node::OldMaterial(_))) && m.inst().is_none() && m.name.is_empty();
                if is_old {
                    if let Some((link, phys)) = votes.get(mi).and_then(|v| v.clone()) {
                        slots.push(Some(self.link_slot(&link, phys, self.editors)));
                        continue;
                    }
                }
                slots.push(match m.inst() {
                    Some(inst) => {
                        if self.editors {
                            let link = inst.link().unwrap_or("").to_string();
                            let stem = link.rsplit('\\').next().unwrap_or(&link);
                            Some(self.material_slot(crate::tiny_assets::editors_link_for_stadium_material(stem), inst.physics()))
                        } else {
                            Some(self.material_inst_slot(inst))
                        }
                    }
                    None if !m.name.is_empty() => Some(self.link_slot(&m.name, 0, self.editors)),
                    None => match m.node.as_ref().and_then(|r| r.inline.as_deref()) {
                        Some(super::Node::OldMaterial(om)) => {
                            let om = om.clone();
                            let common = most_common_physics(so);
                            self.old_material_slot(&om, resolve, common)
                        }
                        _ => None,
                    },
                });
            }
        } else {
            for (mi, r) in s2.materials.iter().enumerate() {
                let is_old = matches!(r.inline.as_deref(), Some(super::Node::OldMaterial(_)));
                if is_old {
                    if let Some((link, phys)) = votes.get(mi).and_then(|v| v.clone()) {
                        slots.push(Some(self.link_slot(&link, phys, self.editors)));
                        continue;
                    }
                }
                // Inline pre-UserInst materials (BlueBay terrain) resolve
                // through their own `.Material.Gbx` refs, like above.
                let inline_old = match r.inline.as_deref() {
                    Some(super::Node::OldMaterial(om)) => {
                        let om = om.clone();
                        let common = most_common_physics(so);
                        self.old_material_slot(&om, resolve, common)
                    }
                    _ => None,
                };
                if inline_old.is_some() {
                    slots.push(inline_old);
                    continue;
                }
                slots.push(match resolve(r.index).map(|(_, l, p)| (l, p)) {
                    Some((link, phys)) => Some(self.link_slot(&link, phys, self.editors)),
                    None => {
                        self.notes.push(format!("material node {} unresolved", r.index));
                        None
                    }
                });
            }
        }
        // Per source material index: the slot came from a surface vote, i.e. a
        // shared Techno3 id material whose look the collision surface decides
        // per triangle (see the split below).
        let voted: Vec<bool> = (0..slots.len())
            .map(|mi| {
                let is_old = if !s2.custom_materials.is_empty() {
                    s2.custom_materials.get(mi).map(|m| matches!(m.node.as_ref().and_then(|r| r.inline.as_deref()), Some(super::Node::OldMaterial(_))) && m.inst().is_none() && m.name.is_empty()).unwrap_or(false)
                } else {
                    s2.materials.get(mi).map(|r| matches!(r.inline.as_deref(), Some(super::Node::OldMaterial(_)))).unwrap_or(false)
                };
                is_old && votes.get(mi).map(|v| v.is_some()).unwrap_or(false)
            })
            .collect();
        let mut visual_slots: Vec<(usize, usize)> = Vec::new();
        if std::env::var_os("TINY_DUMP_DECLS").is_some() {
            self.notes.push(format!(
                "solid2 v{} material_ids {:?} folder {:?} u03 {:?} u04 {:?} geoms {:?} material refs {:?}",
                s2.version,
                s2.material_ids,
                s2.materials_folder,
                s2.u03,
                s2.u04,
                s2.shaded_geoms.iter().map(|g| (g.visual_index, g.material_index, g.u01, g.lod_mask, g.u02)).collect::<Vec<_>>(), s2.materials.iter().map(|r| r.index).collect::<Vec<_>>()
            ));
        }
        // Prefab-wide layer table for the voted (shared Techno3 id) materials:
        // vertex layer id -> material slot, voted from the collision material
        // under the triangles whose three vertices carry that id (see the
        // split below). One table per static object: an id means the same
        // layer in every visual of the prefab (Beach: 0 Land, 1 SeaFloor,
        // 2 Sand; LandHill: 1 HillPxz; LandCliff: 1 CliffPxz).
        let mut layer_votes: std::collections::BTreeMap<u32, std::collections::BTreeMap<usize, usize>> = Default::default();
        if !smap.is_empty() && std::env::var_os("TINY_NO_SPLIT").is_none() {
            for g in &s2.shaded_geoms {
                if g.lod_mask != 0 && g.lod_mask & 1 == 0 && std::env::var_os("TINY_ALL_LODS").is_none() {
                    continue;
                }
                let mi = g.material_index.max(0) as usize;
                if !voted.get(mi).copied().unwrap_or(false) {
                    continue;
                }
                let Some(Node::Visual(vis)) = s2.visuals.get(g.visual_index as usize).and_then(|r| r.inline.as_deref()) else { continue };
                let Some(eff) = effective_layer_ids(vis) else { continue };
                let (pos, idx) = visual_triangles(vis);
                for t in idx.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    let (a, b, c) = (eff[t[0] as usize], eff[t[1] as usize], eff[t[2] as usize]);
                    if a != b || b != c {
                        continue;
                    }
                    let mut k = [mm(&pos[t[0] as usize]), mm(&pos[t[1] as usize]), mm(&pos[t[2] as usize])];
                    k.sort();
                    if let Some((link, phys)) = smap.get(&k) {
                        let slot = self.link_slot(link, *phys, self.editors);
                        *layer_votes.entry(a).or_default().entry(slot).or_default() += 1;
                    }
                }
            }
        }
        let layer_table: std::collections::BTreeMap<u32, usize> = layer_votes.into_iter().filter_map(|(id, votes)| votes.into_iter().max_by_key(|(_, n)| *n).map(|(m, _)| (id, m))).collect();
        if !layer_table.is_empty() {
            self.notes.push(format!("layer table {}", layer_table.iter().map(|(id, m)| format!("{id:x}->{}", self.materials[*m].link().unwrap_or("?").rsplit('\\').next().unwrap_or("?"))).collect::<Vec<_>>().join(" ")));
        }
        for g in &s2.shaded_geoms {
            // A geom's lod mask says which detail levels draw it (bit 0 =
            // nearest). The item is emitted with no lod distances, so every
            // geom it carries draws at every distance: taking every level made
            // the terrain tiles draw LOD0 and LOD1 coplanar (Beach: a coarse
            // 22-vertex LOD1 sheet voted SeaFloor over the LOD0 grass — the
            // "wide bright beaches with hard seams", 2026-09-06). Keep the
            // nearest level only; `TINY_ALL_LODS=1` restores the old behaviour.
            if g.lod_mask != 0 && g.lod_mask & 1 == 0 && std::env::var_os("TINY_ALL_LODS").is_none() {
                self.notes.push(format!("visual {} (lod mask {}) skipped: not the nearest level", g.visual_index, g.lod_mask));
                continue;
            }
            let vis = match s2.visuals.get(g.visual_index as usize).and_then(|r| r.inline.as_deref()) {
                Some(Node::Visual(v)) => v,
                _ => {
                    self.notes.push(format!("shaded geom visual {} is not an inline visual", g.visual_index));
                    continue;
                }
            };
            let mat = slots.get(g.material_index as usize).copied().flatten().unwrap_or_else(|| self.material_slot("Stadium\\Media\\Material\\PlatformTech", 0));
            // Techno3 "_Ids" materials are the terrain id/mask pass (Land Base
            // carries a 4-vertex `Tech3 Block PyPxz_Ids` quad over its Land
            // quad): no look of their own, and as an item material they draw
            // flat grey and z-fight the real surface into stripes (2026-09-06).
            if self.materials.get(mat).and_then(|m| m.link()).map(|l| l.contains("_Ids")).unwrap_or(false) {
                self.notes.push(format!("id-pass visual {} dropped", self.materials[mat].link().unwrap_or("")));
                continue;
            }
            // TINY_DROP_MATS=sub1,sub2: drop visuals whose material link contains
            // a substring (bisecting which material makes the game drop an item)
            if let Ok(drop) = std::env::var("TINY_DROP_MATS") {
                let link = self.materials.get(mat).and_then(|m| m.link()).unwrap_or("").to_string();
                if drop.split(',').any(|s| !s.is_empty() && link.contains(s)) {
                    self.notes.push(format!("visual {link} dropped (TINY_DROP_MATS)"));
                    continue;
                }
            }
            let mut v = vis.clone();
            // TINY_DUMP_DECLS=1: one note per visual with its vertex
            // declarations and the distinct values of every one-word element
            // (colour / int32 ids), for reading a shader's per-vertex inputs.
            if std::env::var_os("TINY_DUMP_DECLS").is_some() {
                if let Some(Node::VertexStream(s)) = v.main.as_ref().and_then(|m| m.vertex_streams.first()).and_then(|r| r.inline.as_deref()) {
                    let compress = s.compress_local3d.unwrap_or(false);
                    let mut parts = Vec::new();
                    for (d, e) in s.decls.iter().zip(s.elems.iter()) {
                        let mut p = format!("name{} type{} space{}", d.name(), d.stored_type(compress), d.space());
                        if d.name() == N_POSITION {
                            if let Elem::Float3(pts) = e {
                                // y histogram in 0.5 m bins (prefab space, before scaling)
                                let mut bins: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
                                for q in pts {
                                    *bins.entry((q[1] * 2.0).floor() as i32).or_default() += 1;
                                }
                                p.push_str(&format!(" y{{{}}}", bins.iter().map(|(b, n)| format!("{}:{}", *b as f32 / 2.0, n)).collect::<Vec<_>>().join(" ")));
                            }
                        }
                        if let Elem::Word(w) = e {
                            let mut u: Vec<u32> = w.clone();
                            u.sort_unstable();
                            u.dedup();
                            p.push_str(&format!(" values[{}]", u.iter().take(12).map(|x| format!("{x:08x}")).collect::<Vec<_>>().join(",")));
                            // per distinct word: the y histogram of the vertices carrying it
                            if let Some(Elem::Float3(pts)) = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == N_POSITION).map(|(_, e)| e) {
                                for val in u.iter().take(6) {
                                    let mut bins: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
                                    for (q, x) in pts.iter().zip(w.iter()) {
                                        if x == val {
                                            *bins.entry((q[1] * 2.0).floor() as i32).or_default() += 1;
                                        }
                                    }
                                    p.push_str(&format!(" {val:x}@y{{{}}}", bins.iter().map(|(b, n)| format!("{}:{}", *b as f32 / 2.0, n)).collect::<Vec<_>>().join(" ")));
                                }
                            }
                        }
                        if let Elem::Float4(f) = e {
                            p.push_str(&format!(" f4[{:?}..]", f.first()));
                        }
                        if let Elem::Float2(f) = e {
                            p.push_str(&format!(" f2[{:?}..]", f.first()));
                        }
                        parts.push(p);
                    }
                    self.notes.push(format!("visual {} mat#{} ({} verts) material {}: {} || chunks {:x?} uvg {} u02 {} u03 {} u04 {:x?} tcs {} subvis {} splits {} tangents {:?} idx {}", g.visual_index, g.material_index, s.elems.first().map(|e| e.len()).unwrap_or(0), self.materials[mat].link().unwrap_or("?"), parts.join(" | "), v.chunks, v.main.as_ref().map(|m| m.uv_groups.len()).unwrap_or(0), v.main.as_ref().map(|m| m.u02).unwrap_or(0), v.main.as_ref().map(|m| m.u03).unwrap_or(0), v.main.as_ref().map(|m| m.u04.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect::<Vec<u32>>()).unwrap_or_default(), v.main.as_ref().map(|m| m.tex_coord_sets.len()).unwrap_or(0), v.sub_visuals.len(), v.splits.len(), v.tangents.as_ref().map(|(a, b)| (a.len(), b.len())), v.index_buffer.as_ref().map(|b| b.indices.len()).unwrap_or(0)));
                }
            }
            // A voted (shared Techno3 id) material has no look of its own: the
            // prefab paints its layers per VERTEX (Beach: id 0 = Land on the
            // plateau, 2 = Sand on the slope, 1 = SeaFloor under water; hills
            // and cliffs likewise switch to HillPxz/CliffPxz on the steep part)
            // and the collision surface is partitioned the same way. A single
            // linkable pak material is uniform, so one material per visual
            // painted a whole tile with one layer (the 2026-09-06 beaches: sand
            // and sea floor where the original shows grass). Split the visual
            // by the collision material under each triangle instead — one
            // sub-visual per coincident surface material, unmatched triangles
            // staying with the voted majority. `TINY_NO_SPLIT=1` disables.
            let mi = g.material_index.max(0) as usize;
            if voted.get(mi).copied().unwrap_or(false) && !smap.is_empty() && std::env::var_os("TINY_NO_SPLIT").is_none() {
                let (pos, idx) = visual_triangles(&v);
                let ntri = idx.len() / 3;
                // Per-triangle collision material (None where no collision
                // triangle coincides).
                let surf_mat: Vec<Option<usize>> = idx
                    .chunks(3)
                    .take(ntri)
                    .map(|t| {
                        let mut k = [mm(&pos[t[0] as usize]), mm(&pos[t[1] as usize]), mm(&pos[t[2] as usize])];
                        k.sort();
                        smap.get(&k).map(|(link, phys)| self.link_slot(link, *phys, self.editors))
                    })
                    .collect();
                // The prefab's own painting: vertex element 4 (Int32) is the
                // layer id — one byte, or two bytes `hi:lo` on a blend visual
                // whose vertex colour's G byte picks lo (G < 128) or hi. The
                // id -> material table is voted from the collision material
                // under the single-id triangles, so the split lands where the
                // original changes layer (the blend's midpoint), not where the
                // physics zone happens to change.
                let eff_ids = effective_layer_ids(&v);
                let mut tri_mat: Vec<usize> = Vec::with_capacity(ntri);
                match eff_ids {
                    Some(eff) if !layer_table.is_empty() && std::env::var_os("TINY_SPLIT_SURFACE").is_none() => {
                        let table = &layer_table;
                        for (ti, t) in idx.chunks(3).take(ntri).enumerate() {
                            let (a, b, c) = (eff[t[0] as usize], eff[t[1] as usize], eff[t[2] as usize]);
                            let m = if a == b && b == c {
                                table.get(&a).copied().or(surf_mat[ti]).unwrap_or(mat)
                            } else {
                                // a triangle straddling two layers: the collision
                                // material under it, else the majority id
                                let maj = if a == b || a == c { a } else if b == c { b } else { a.max(b).max(c) };
                                surf_mat[ti].or_else(|| table.get(&maj).copied()).unwrap_or(mat)
                            };
                            tri_mat.push(m);
                        }
                    }
                    _ => {
                        for sm in &surf_mat {
                            tri_mat.push(sm.unwrap_or(mat));
                        }
                    }
                }
                let mut groups: Vec<usize> = tri_mat.clone();
                groups.sort_unstable();
                groups.dedup();
                if groups.len() > 1 {
                    for gm in groups {
                        let keep: Vec<bool> = tri_mat.iter().map(|m| *m == gm).collect();
                        let mut sv = sub_visual(&v, &keep)?;
                        transform_visual(&mut sv, iso, scale)?;
                        self.notes.push(format!("visual {} split: {} triangles -> {}", g.visual_index, keep.iter().filter(|k| **k).count(), self.materials[gm].link().unwrap_or("?")));
                        visual_slots.push((self.visuals.len(), gm));
                        self.visuals.push(MergedVisual { visual: sv, material: gm });
                    }
                    continue;
                }
                if let Some(&only) = tri_mat.first() {
                    if only != mat {
                        // every matched triangle disagrees with the majority
                        // vote (cannot happen — the vote is over these same
                        // triangles — but keep the per-triangle answer)
                        transform_visual(&mut v, iso, scale)?;
                        visual_slots.push((self.visuals.len(), only));
                        self.visuals.push(MergedVisual { visual: v, material: only });
                        continue;
                    }
                }
            }
            transform_visual(&mut v, iso, scale)?;
            visual_slots.push((self.visuals.len(), mat));
            self.visuals.push(MergedVisual { visual: v, material: mat });
        }
        if let Some(sf) = so.surface() {
            match &sf.surf {
                Surf::Mesh { vertices, triangles, .. } => {
                    // A triangle's u8 is its physics id (matches the u16 list
                    // it indexes on every Nadeo prefab measured).
                    self.add_surface_mesh(vertices, triangles, iso, scale);
                }
                other => self.notes.push(format!("collision surf type {} is not a mesh; skipped", other.type_id())),
            }
        } else if so.is_mesh_collidable {
            // Collide against the visuals themselves.
            for (vi, mat) in visual_slots {
                let v = &self.visuals[vi].visual;
                let phys = self.materials[mat].physics();
                let (pos, idx) = visual_triangles(v);
                let tris: Vec<Triangle> = idx.chunks(3).filter(|c| c.len() == 3).map(|c| Triangle { indices: [c[0], c[1], c[2]], material_id: phys, u03: 0, surface_index: 0 }).collect();
                self.add_surface_mesh(&pos, &tris, &IDENTITY, 1.0);
            }
        }
        Ok(())
    }
}

/// The prefab's own terrain painting, per vertex: element 4 (Int32) is the
/// layer id — one byte, or two bytes `hi:lo` on a blend visual whose vertex
/// colour's G byte picks lo (G < 128) or hi (measured on Beach\Base1A: 0x200
/// vertices at the plateau edge read Land at the top ring and Sand below it,
/// 0x201 at the water line read SeaFloor below and Sand above). `None` when
/// the stream has no such element.
pub fn effective_layer_ids(v: &CPlugVisualIndexedTriangles) -> Option<Vec<u32>> {
    let s = v.stream()?;
    let compress = s.compress_local3d.unwrap_or(false);
    let ids = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == 4 && d.stored_type(compress) == super::vstream::T_INT32).and_then(|(_, e)| match e {
        Elem::Word(w) => Some(w),
        _ => None,
    })?;
    let colors = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == super::vstream::N_COLOR0).and_then(|(_, e)| match e {
        Elem::Word(w) => Some(w),
        _ => None,
    });
    Some(
        ids.iter()
            .enumerate()
            .map(|(i, id)| {
                if *id < 0x100 {
                    *id
                } else {
                    let g = colors.and_then(|c| c.get(i)).map(|c| (c >> 8) & 0xFF).unwrap_or(0);
                    if g < 128 { id & 0xFF } else { (id >> 8) & 0xFF }
                }
            })
            .collect(),
    )
}

/// The visual restricted to the triangles flagged in `keep` (one flag per
/// triangle): the index buffer is filtered, the vertices it no longer uses are
/// dropped from every stream element and both tangent arrays, and the counts
/// follow. Vertex order is preserved. The bounding box is left to
/// `transform_visual`, which recomputes it.
pub fn sub_visual(v: &CPlugVisualIndexedTriangles, keep: &[bool]) -> R<CPlugVisualIndexedTriangles> {
    let mut out = v.clone();
    let idx = v.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
    let nverts = v.main.as_ref().map(|m| m.count.max(0) as usize).unwrap_or(0);
    let mut used = vec![false; nverts];
    let mut new_idx: Vec<u32> = Vec::new();
    for (ti, t) in idx.chunks(3).enumerate() {
        if t.len() == 3 && keep.get(ti).copied().unwrap_or(false) {
            for &i in t {
                *used.get_mut(i as usize).ok_or("sub_visual: index past the vertex count")? = true;
            }
            new_idx.extend_from_slice(t);
        }
    }
    let mut remap = vec![u32::MAX; nverts];
    let mut n = 0u32;
    for (i, u) in used.iter().enumerate() {
        if *u {
            remap[i] = n;
            n += 1;
        }
    }
    for i in new_idx.iter_mut() {
        *i = remap[*i as usize];
    }
    let m = out.main.as_mut().ok_or("sub_visual: visual without chunk 0x0900600F")?;
    let per = (((!(m.flags() >> 17)) & 8) | 4) as usize;
    m.count = n as i32;
    let stream = match m.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
        Some(Node::VertexStream(s)) => s,
        _ => return Err("sub_visual: visual without an inline vertex stream".into()),
    };
    stream.count = n as i32;
    for e in stream.elems.iter_mut() {
        *e = match e {
            Elem::Float2(a) => Elem::Float2(a.iter().zip(&used).filter(|(_, u)| **u).map(|(x, _)| *x).collect()),
            Elem::Float3(a) => Elem::Float3(a.iter().zip(&used).filter(|(_, u)| **u).map(|(x, _)| *x).collect()),
            Elem::Float4(a) => Elem::Float4(a.iter().zip(&used).filter(|(_, u)| **u).map(|(x, _)| *x).collect()),
            Elem::Word(a) => Elem::Word(a.iter().zip(&used).filter(|(_, u)| **u).map(|(x, _)| *x).collect()),
            Elem::Raw { size, bytes } => {
                let size = *size;
                Elem::Raw { size, bytes: bytes.chunks(size.max(1)).zip(&used).filter(|(_, u)| **u).flat_map(|(c, _)| c.iter().copied()).collect() }
            }
        };
    }
    if let Some((a, b)) = out.tangents.as_mut() {
        for t in [a, b] {
            if !t.is_empty() {
                *t = t.chunks(per).zip(&used).filter(|(_, u)| **u).flat_map(|(c, _)| c.iter().copied()).collect();
            }
        }
    }
    if let Some(ib) = out.index_buffer.as_mut() {
        ib.indices = new_idx;
    }
    Ok(out)
}

/// A visual's (already transformed) positions and triangle indices.
pub fn visual_triangles(v: &CPlugVisualIndexedTriangles) -> (Vec<[f32; 3]>, Vec<u32>) {
    let pos = match v.stream().and_then(|s| s.elems.first()) {
        Some(Elem::Float3(p)) => p.clone(),
        _ => Vec::new(),
    };
    let idx = v.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
    (pos, idx)
}

fn mm(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0] * 1000.0).round() as i32, (p[1] * 1000.0).round() as i32, (p[2] * 1000.0).round() as i32)
}

/// Per source-material-index vote: visuals whose link came out shared-id
/// (Techno3) get the surface material most of their triangles coincide
/// with. Triangle lookup is by sorted mm vertex keys in prefab space.
/// Collision triangle (sorted mm vertex keys) -> (material link, physics).
pub type SurfMap = std::collections::HashMap<[(i32, i32, i32); 3], (String, u8)>;

fn surface_votes(
    so: &super::item::CPlugStaticObjectModel,
    s2: &super::solid2::CPlugSolid2Model,
    resolve: &mut MaterialResolver,
) -> (Vec<Option<(String, u8)>>, SurfMap) {
    use std::collections::BTreeMap;
    let sf = match so.surface() {
        Some(sf) => sf,
        None => return (Vec::new(), SurfMap::new()),
    };
    let (verts, tris) = match &sf.surf {
        super::surface::Surf::Mesh { vertices, triangles, .. } => (vertices, triangles),
        _ => return (Vec::new(), SurfMap::new()),
    };
    // surface tri -> (material path, physics)
    let mut smap: SurfMap = SurfMap::new();
    for t in tris {
        let mut k = [mm(&verts[t.indices[0] as usize]), mm(&verts[t.indices[1] as usize]), mm(&verts[t.indices[2] as usize])];
        k.sort();
        let si = t.surface_index.max(0) as usize;
        let (link, phys) = match sf.materials.get(si) {
            Some(super::surface::SurfMaterial::Node(r)) => match resolve(r.index) {
                Some((path, _, _)) => {
                    let phys = sf.material_ids.get(si).map(|id| (id & 0xFF) as u8).unwrap_or(t.material_id);
                    (material_link(&path), phys)
                }
                None => continue,
            },
            _ => continue,
        };
        smap.insert(k, (link, phys));
    }
    // votes per source material index
    let mut votes: Vec<BTreeMap<(String, u8), usize>> = vec![BTreeMap::new(); s2.shaded_geoms.iter().map(|g| g.material_index.max(0) as usize + 1).max().unwrap_or(0)];
    for g in &s2.shaded_geoms {
        let mi = g.material_index.max(0) as usize;
        let vis = match s2.visuals.get(g.visual_index as usize).and_then(|r| r.inline.as_deref()) {
            Some(super::Node::Visual(v)) => v,
            _ => continue,
        };
        let (pos, idx) = visual_triangles(vis);
        for t in idx.chunks(3) {
            if t.len() < 3 {
                continue;
            }
            let mut k = [mm(&pos[t[0] as usize]), mm(&pos[t[1] as usize]), mm(&pos[t[2] as usize])];
            k.sort();
            if let Some(vote) = smap.get(&k) {
                if mi < votes.len() {
                    *votes[mi].entry(vote.clone()).or_default() += 1;
                }
            }
        }
    }
    (votes.into_iter().map(|v| v.into_iter().max_by_key(|(_, n)| *n).map(|(lp, _)| lp)).collect(), smap)
}

fn inline(index: i32, node: Node) -> Ref {
    NodeRef { index, inline: Some(Box::new(node)) }
}

/// The reference items' PreLightGen (every one of the 26 carries these).
fn default_prelight() -> PreLightGen {
    PreLightGen {
        version: 1,
        u01: 1,
        u02: 32.14457,
        u03: true,
        u04: [0.001, 0.001, 0.99712694, 0.999, f32::MAX, f32::MAX, f32::MIN, f32::MIN],
        sprite_count: [0, 0],
        boxes: Vec::new(),
        uv_groups: Vec::new(),
    }
}

/// `CGameItemPlacementParam` as the reference items carry it: chunks
/// 2E020000 (grid snap 1 m, fly step 1 m, pivot snap -1), 001 (no pivots), 004,
/// 005 (an inline `NPlugItemPlacement_SClass` node at `sclass_index`).
pub fn placement_param(sclass_index: i32) -> super::item::CGameItemPlacementParam {
    let mut p0 = Vec::new();
    p0.extend_from_slice(&0u32.to_le_bytes()); // version
    p0.extend_from_slice(&1u16.to_le_bytes()); // flags
    // cube center, cube size 0, grid snap h 1 m / v 0, offsets 0, fly v step
    // 1 m, fly v offset 0, pivot snap distance -1
    for f in [0.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0] {
        p0.extend_from_slice(&f.to_le_bytes());
    }
    let mut p5 = Vec::new();
    p5.extend_from_slice(&sclass_index.to_le_bytes());
    p5.extend_from_slice(&0x09187000u32.to_le_bytes());
    for w in [10u32, 0xFFFF_FFFF, 0, 0, 1, 0, 0, 0, 0x3F80_0000, 0, 0] {
        p5.extend_from_slice(&w.to_le_bytes());
    }
    super::item::CGameItemPlacementParam {
        chunks: vec![
            super::RawChunk { id: 0x2E020000, payload: p0 },
            super::RawChunk { id: 0x2E020001, payload: vec![0; 8] },
            super::RawChunk { id: 0x2E020004, payload: vec![0; 8] },
            super::RawChunk { id: 0x2E020005, payload: p5 },
        ],
    }
}

/// Build the whole item tree from the merged geometry.
pub fn assemble(m: &Merged, opts: &BuildOpts) -> R<super::StaticItemFile> {
    use super::item::*;
    use super::Id;
    if m.visuals.is_empty() {
        return Err("no visuals: nothing to build".into());
    }
    let mut next = 4i32;
    let mut s2 = CPlugSolid2Model::new_v34();
    let visuals = if std::env::var_os("TINY_NO_COALESCE").is_some() { m.visuals.clone() } else { coalesce(&m.visuals) };
    // Only the materials some visual draws with, in first-use order (the
    // reference items list exactly one material per visual).
    let mut used: Vec<usize> = Vec::new();
    for mv in &visuals {
        if !used.contains(&mv.material) {
            used.push(mv.material);
        }
    }
    // Geoms in material order: the game reads the shaded-geom list as
    // material-sorted runs (every Nadeo item and every item that ever loaded
    // here is non-decreasing in material index; the first split terrain items
    // with (0,1,2,0,1,2,3,4) crashed the client at 0x140456507 reading a
    // garbage material index, 2026-09-06). Stable, so same-material visuals
    // keep their relative order.
    let mut visuals = visuals;
    visuals.sort_by_key(|mv| used.iter().position(|u| *u == mv.material).unwrap_or(usize::MAX));
    for mv in &visuals {
        let mut v = mv.visual.clone();
        let main = v.main.as_mut().unwrap();
        // the stream sits right after its visual
        for r in main.vertex_streams.iter_mut() {
            if r.inline.is_some() {
                r.index = next + 1;
            }
        }
        let material_index = used.iter().position(|u| *u == mv.material).unwrap() as i32;
        s2.shaded_geoms.push(ShadedGeom { visual_index: s2.visuals.len() as i32, material_index, u01: -1, lod_mask: 1, u02: 0 });
        s2.visuals.push(inline(next, Node::Visual(v)));
        next += 2;
    }
    for inst in used.iter().map(|u| &m.materials[*u]) {
        let inst = skinned_material(inst, opts.collection);
        s2.custom_materials.push(Material { name: String::new(), node: Some(inline(next, Node::Material(inst))) });
        next += 1;
    }
    s2.pre_light_gen = Some(m.pre_light_gen.clone().unwrap_or_else(default_prelight));
    s2.file_write_time = m.file_write_time;
    let surface = CPlugSurface::mesh(m.surf_vertices.clone(), m.surf_triangles.clone(), m.surf_ids.clone(), [0.0, 0.0, 1.0]);
    let surface_index = next;
    next += 1;
    let no_wp = std::env::var_os("TINY_NO_WAYPOINT").is_some();
    let trigger = match m.trigger.as_ref().filter(|_| !no_wp) {
        Some(t) => {
            next += 1;
            inline(next - 1, Node::Surface(t.clone()))
        }
        None => super::null_ref(),
    };
    let so = CPlugStaticObjectModel { version: 3, mesh: inline(3, Node::Solid2(s2)), is_mesh_collidable: false, shape: inline(surface_index, Node::Surface(surface)) };
    let ent = CGameCommonItemEntityModel {
        version: 6,
        v0_models: None,
        v3_strings: None,
        static_object: inline(2, Node::StaticObject(so)),
        trigger_shape: trigger,
        iso: { let mut i = IDENTITY; i[9] = m.spawn[0]; i[10] = m.spawn[1]; i[11] = m.spawn[2]; i },
        particle_emitter: super::null_ref(),
        actions: Vec::new(),
        u_node: super::null_ref(),
        strings: Default::default(),
        iso2: IDENTITY,
        expr_validator: 0,
        u_byte: 1,
    };
    let placement_index = next;
    let sclass_index = next + 1;
    next += 2;
    let ident = || ItemChunk::Ident { path: Id::Str(opts.ident.clone()), collection: Id::Raw(opts.collection), author: Id::Str(opts.author.clone()) };
    let model = ModelChunk {
        version: 15,
        old_models: None,
        default_weapon_name: Id::Null,
        phy_model_custom: super::null_ref(),
        vis_model_custom: super::null_ref(),
        actions: Vec::new(),
        default_cam: 0,
        entity_model_edition: super::null_ref(),
        entity_model: inline(1, Node::EntityModel(ent)),
        vfx: super::null_ref(),
        material_modifier: super::null_ref(),
    };
    let chunks = vec![
        ItemChunk::Collector1009 { page_name: "Items".into(), icon: None, u01: Id::Null },
        ident(),
        ItemChunk::Name("New Item".into()),
        ItemChunk::Description("No Description".into()),
        ItemChunk::Skin { version: 4, default_skin: super::null_ref(), skin_directory: String::new(), extra: Some(super::null_ref()) },
        ItemChunk::Catalog { version: 1, is_internal: false, is_advanced: false, position: 1, prod_state: Some(3) },
        ItemChunk::Ints1012([0, 1, 0, 0]),
        ItemChunk::NadeoSkinFids(vec![super::null_ref(); 7]),
        ItemChunk::Cameras(10, Vec::new()),
        ItemChunk::RaceInterface(super::null_ref()),
        ItemChunk::Ground { ground_point: [0.0; 3], floats: [0.0, 0.0, -1.0, 0.15] },
        ItemChunk::ItemType(1),
        ItemChunk::Model(model),
        ItemChunk::Node201A(super::null_ref()),
        ItemChunk::DefaultPlacement { version: 5, placement: inline(placement_index, Node::Placement(placement_param(sclass_index))) },
        ItemChunk::Archetype { version: 7, archetype_ref: String::new(), archetype_fid: Some(super::null_ref()), skin_dir: Some(String::new()), u01: Some(-1) },
        ItemChunk::Waypoint { version: 12, waypoint_type: if no_wp { 3 } else { m.waypoint_type.unwrap_or(3) }, disable_lightmap: false, u_node: Some(super::null_ref()), u_byte: Some(0), u_ints: Some((-1, -1)) },
        ItemChunk::Icon { version: 3, icon_fid: String::new(), u_byte: Some(1) },
        ItemChunk::Skippable(super::RawChunk { id: 0x2E002025, payload: vec![0; 8] }),
        ItemChunk::Skippable(super::RawChunk { id: 0x2E002026, payload: vec![0; 8] }),
        ItemChunk::Skippable(super::RawChunk { id: 0x2E002027, payload: vec![0; 8] }),
    ];
    Ok(super::StaticItemFile {
        version: 6,
        format: b'B',
        ref_comp: b'U',
        body_comp: b'U',
        unknown: Some(b'R'),
        class_id: super::C_ITEM_MODEL,
        header_chunks: header_chunks(opts),
        num_nodes: next as u32,
        ref_table: 0u32.to_le_bytes().to_vec(),
        item: CGameItemModel { chunks },
    })
}

/// Header chunks 2E001003 (desc, v8), 2E001006 (lightmap time 0),
/// 2E002000 (item type Ornament), 2E002001 (file version 0).
pub fn header_chunks(opts: &BuildOpts) -> Vec<super::file::HeaderChunk> {
    use super::file::HeaderChunk;
    let mut d = Vec::new();
    let mut lb = super::LookbackState::default();
    {
        let mut w = super::Wr { w: &mut d, lb: &mut lb };
        w.id(&super::Id::Str(opts.ident.clone()));
        w.id(&super::Id::Raw(opts.collection));
        w.id(&super::Id::Str(opts.author.clone()));
        w.u32(8);
        w.string("Items");
        w.id(&super::Id::Null);
        w.i32(8);
        w.i16(1);
        w.string("New Item");
        w.u8(3);
    }
    vec![
        HeaderChunk { id: 0x2E001003, heavy: false, payload: d },
        HeaderChunk { id: 0x2E001006, heavy: false, payload: vec![0; 8] },
        HeaderChunk { id: 0x2E002000, heavy: false, payload: 1u32.to_le_bytes().to_vec() },
        HeaderChunk { id: 0x2E002001, heavy: false, payload: vec![0; 4] },
    ]
}

/// Physics id of an external `.Material.Gbx` (its `CPlugMaterial` surface
/// id), through the store; `None` when it cannot be read.
pub fn material_physics(store: &mut crate::store::DataStore, path: &str) -> Option<u8> {
    let m = store.load_model(path).ok()?;
    let g = m.graph().ok()?;
    match g.root.as_ref()? {
        crate::node::Node::Material(_, phys) => Some(*phys),
        _ => None,
    }
}

/// `Stadium\Media\Material\RoadTech.Material.Gbx` -> `Stadium\Media\Material\RoadTech`.
pub fn material_link(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    match lower.rfind(".material.gbx") {
        Some(i) => path[..i].to_string(),
        None => path.to_string(),
    }
}

/// Walk a prefab (and its external prefabs, recursively) adding every static
/// object placed by `at`.
pub fn add_prefab(store: &mut crate::store::DataStore, path: &str, at: &Xform, scale: f32, m: &mut Merged, depth: usize) -> R<()> {
    if depth > 8 {
        return Err(format!("{path}: prefab nesting deeper than 8"));
    }
    let model = store.load_model(path)?;
    let prefab = super::prefab::CPlugPrefab::from_model(&model)?;
    let externals = model.externals.clone();
    let ext_name = |i: i32| externals.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone());
    for (i, e) in prefab.ents.iter().enumerate() {
        let iso = compose(at, &super::prefab::CPlugPrefab::entity_iso(e));
        match e.model.inline.as_deref() {
            Some(Node::StaticObject(so)) => {
                // Physics: the game's library table first, the .Material.Gbx's
                // own surface id (usually 0) next, else the object's most
                // common collision physics.
                let common = most_common_physics(so);
                let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
                    let p = ext_name(idx)?;
                    let link = material_link(&p);
                    let phys = physics_for_link(&link).or_else(|| material_physics(store, &p).filter(|x| *x != 0)).or(common).unwrap_or(0);
                    Some((p, link, phys))
                };
                m.add_static_object(so, &iso, scale, &mut resolve).map_err(|err| format!("{path} entity {i}: {err}"))?;
            }
            // NPlugTrigger_SWaypoint: { version, waypoint type, trigger shape ref,
            // u32 } (read off Items\Gate\CheckpointLeft32m: 01 00 00 00 | 02 00 00
            // 00 = checkpoint | 1d 00 00 00 = node 29, the external
            // *_Trigger.Shape.Gbx | 00 00 00 00). The item gets that type and the
            // shape, transformed like the geometry.
            Some(Node::Opaque(o)) if o.class_id == 0x09178000 && o.raw.len() >= 16 => {
                let wtype = i32::from_le_bytes(o.raw[4..8].try_into().unwrap());
                let shape_idx = i32::from_le_bytes(o.raw[8..12].try_into().unwrap());
                match ext_name(shape_idx) {
                    Some(sp) if sp.to_ascii_lowercase().ends_with(".shape.gbx") => match store.load_model(&sp) {
                        Ok(sm) => {
                            let mut lb = super::LookbackState::default();
                            lb.defined_nodes.extend(sm.external_indices().iter().copied());
                            let mut r = super::Rd::new(&sm.body, 0, lb);
                            match super::surface::CPlugSurface::parse(&mut r) {
                                Ok(sf) => {
                                    // re-emitted in the canonical form the box
                                    // triggers use: the pack shape kept verbatim
                                    // (its materials/ids) made the game drop the
                                    // whole item (GateCheckpointLeft32m, 2026-09-06)
                                    let super::surface::Surf::Mesh { vertices, triangles, .. } = &sf.surf else {
                                        m.notes.push(format!("{path} entity {i}: trigger shape {sp} is not a mesh surface; skipped"));
                                        continue;
                                    };
                                    let verts: Vec<[f32; 3]> = vertices.iter().map(|v| { let t = apply(&iso, *v); [t[0] * scale, t[1] * scale, t[2] * scale] }).collect();
                                    let tris: Vec<super::surface::Triangle> = triangles.iter().map(|t| super::surface::Triangle { indices: t.indices, material_id: 0, u03: 0, surface_index: 0 }).collect();
                                    m.trigger = Some(super::surface::CPlugSurface::mesh(verts, tris, vec![0], [0.0, 0.0, 1.0]));
                                    m.waypoint_type = Some(wtype);
                                    m.notes.push(format!("{path} entity {i}: waypoint trigger type {wtype} from {sp}"));
                                }
                                Err(e) => m.notes.push(format!("{path} entity {i}: trigger shape {sp} failed: {e}")),
                            }
                        }
                        Err(e) => m.notes.push(format!("{path} entity {i}: trigger shape {sp} failed: {e}")),
                    },
                    _ => m.notes.push(format!("{path} entity {i}: waypoint trigger type {wtype} with shape node {shape_idx} (not an external shape) skipped")),
                }
            }
            Some(other) => m.notes.push(format!("{path} entity {i}: model class 0x{:08X} skipped", other.class_id())),
            None if e.model.index < 0 => {}
            None => match ext_name(e.model.index) {
                Some(p) if p.to_ascii_lowercase().ends_with(".prefab.gbx") => add_prefab(store, &p, &iso, scale, m, depth + 1)?,
                // an external static object (gate speedometer lights, road
                // signs): its own file, mesh and shape external again
                Some(p) if p.to_ascii_lowercase().ends_with(".staticobject.gbx") => {
                    if let Err(e) = add_static_object_file(store, &p, &iso, scale, m) {
                        m.notes.push(format!("{path} entity {i}: external {p} failed: {e}"));
                    }
                }
                Some(p) => m.notes.push(format!("{path} entity {i}: external {p} skipped")),
                None => m.notes.push(format!("{path} entity {i}: external node {} unnamed", e.model.index)),
            },
        }
    }
    Ok(())
}

/// Every `CPlugStaticObjectModel` entity of `prefab` (recursively), merged
/// into one static item.
pub fn static_item_from_prefab(store: &mut crate::store::DataStore, prefab: &str, ident: &str, author: &str, scale: f32, collection: u32) -> R<Vec<u8>> {
    let (bytes, _) = static_item_from_prefab_report(store, prefab, ident, author, scale, collection)?;
    Ok(bytes)
}

/// Same, also returning the merge notes.
pub fn static_item_from_prefab_report(store: &mut crate::store::DataStore, prefab: &str, ident: &str, author: &str, scale: f32, collection: u32) -> R<(Vec<u8>, Merged)> {
    let mut m = Merged::default();
    m.editors = std::env::var_os("TINY_EDITORS").is_some();
    add_prefab(store, prefab, &IDENTITY, scale, &mut m, 0)?;
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, editors: m.editors };
    let f = assemble(&m, &opts)?;
    Ok((super::write_file(&f), m))
}

/// A Nadeo (or any) item that is already a static object, or a crystal item
/// (`CGameCommonItemEntityModelEdition` -> `CPlugCrystal`) baked to visuals
/// + collision, rewritten as a static item.
pub fn static_item_from_item(item_bytes: &[u8], ident: &str, author: &str, scale: f32) -> R<Vec<u8>> {
    let (bytes, _) = static_item_from_item_report(item_bytes, ident, author, scale, 26)?;
    Ok(bytes)
}

pub fn static_item_from_item_report(item_bytes: &[u8], ident: &str, author: &str, scale: f32, collection: u32) -> R<(Vec<u8>, Merged)> {
    let mut m = Merged::default();
    m.editors = std::env::var_os("TINY_EDITORS").is_some();
    match super::parse_file(item_bytes) {
        Ok(f) => {
            let so = f.item.static_object().ok_or("item has no CPlugStaticObjectModel (and is not a crystal item)")?;
            let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
                m.notes.push(format!("external material node {idx} in a standalone item"));
                None
            };
            let mut m2 = Merged::default();
            m2.add_static_object(so, &IDENTITY, scale, &mut resolve)?;
            // waypoint type from the item chunk (3 = none), trigger + spawn iso
            // from the entity model, all scaled
            let wt = f.item.chunks.iter().find_map(|c| match c {
                super::item::ItemChunk::Waypoint { waypoint_type, .. } => Some(*waypoint_type),
                _ => None,
            });
            if let Some(t) = wt.filter(|t| *t != 3) {
                m2.waypoint_type = Some(t);
            }
            if let Some(ent) = f.item.model().and_then(|mc| mc.entity_model()) {
                if let Some(Node::Surface(t)) = ent.trigger_shape.inline.as_deref() {
                    let mut t = t.clone();
                    if let super::surface::Surf::Mesh { vertices, .. } = &mut t.surf {
                        for v in vertices.iter_mut() {
                            *v = [v[0] * scale, v[1] * scale, v[2] * scale];
                        }
                    }
                    m2.trigger = Some(t);
                }
                m2.spawn = [ent.iso[9] * scale, ent.iso[10] * scale, ent.iso[11] * scale];
            }
            m2.editors = std::env::var_os("TINY_EDITORS").is_some();
            m2.notes.extend(m.notes.drain(..));
            m = m2;
        }
        Err(e) => {
            // Not a static item: try the crystal path.
            let g = tmmaps::gbx::Gbx::parse(item_bytes);
            let loc = crate::crystal_model::locate(&g.body).map_err(|e2| format!("not a static item ({e}); not a crystal item ({e2})"))?;
            let (crystal, _, _) = crate::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone())?;
            super::bake::add_crystal(&crystal, scale, &mut m)?;
        }
    }
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, editors: m.editors };
    let f = assemble(&m, &opts)?;
    Ok((super::write_file(&f), m))
}

/// Crystal virtual links resolved to the real game-material paths the
/// editor's bake writes (harvested against Granady's items + pak
/// existence). A crystal `Material\Special<Kind><Mod>` link has no
/// `.Material.Gbx` of its own; the baked item carries the existing
/// `Modifier\<Mod>\<Kind>` file instead (both the 2025.7.4 and the current
/// pak contain `Modifier\Turbo\{Sign,SignOff,Decal}.Material.Gbx`; tri-count
/// correspondence on Road_17 is exact: Sign 654idx/218tris, SignOff 54/18,
/// Decal 384/128). `SpecialFXTurbo` has no `Modifier\Turbo\SpecialFX` file,
/// so it stays virtual -- matching all 26 references.
pub const MATERIAL_LINK_RESOLVE: &[(&str, &str)] = &[
    ("Stadium\\Media\\Material\\SpecialSignTurbo", "Stadium\\Media\\Modifier\\Turbo\\Sign"),
    ("Stadium\\Media\\Material\\SpecialSignOff", "Stadium\\Media\\Modifier\\Turbo\\SignOff"),
    ("Stadium\\Media\\Material\\DecalSpecialTurbo", "Stadium\\Media\\Modifier\\Turbo\\Decal"),
];

/// The editor-resolved link for a crystal material link.
pub fn resolve_crystal_link(link: &str) -> &str {
    MATERIAL_LINK_RESOLVE.iter().find(|(v, _)| *v == link).map(|(_, r)| *r).unwrap_or(link)
}

/// Physics id the game gives a library material (harvested from the 26
/// reference items: every link there carries exactly one id). The
/// `.Material.Gbx` files themselves carry none (classes.rs reads their
/// surface id as 0), so this table is what makes a prefab-built item's
/// `CPlugMaterialUserInst.surface_physic_id` match the game's own items.
pub const MATERIAL_PHYSICS: &[(&str, u8)] = &[
    ("Stadium\\Media\\Material\\ChronoFinish", 32),
    ("Stadium\\Media\\Material\\DecoHill", 2),
    ("Stadium\\Media\\Material\\DecoHill2", 2),
    ("Stadium\\Media\\Material\\LightSpot", 32),
    ("Stadium\\Media\\Material\\LightSpot2", 32),
    ("Stadium\\Media\\Material\\PlatformTech", 16),
    ("Stadium\\Media\\Material\\RaceAd6x1", 32),
    ("Stadium\\Media\\Material\\RaceArchFinish", 4),
    ("Stadium\\Media\\Material\\RaceScreenStart", 32),
    ("Stadium\\Media\\Material\\RaceScreenStartSmall", 32),
    ("Stadium\\Media\\Material\\RoadTech", 16),
    ("Stadium\\Media\\Material\\Speedometer", 4),
    ("Stadium\\Media\\Material\\SpeedometerLight", 4),
    ("Stadium\\Media\\Material\\Technics", 4),
    ("Stadium\\Media\\Material\\TechnicsSpecials", 4),
    ("Stadium\\Media\\Material\\TechnicsTrims", 4),
    ("Stadium\\Media\\Material\\TrackBorders", 9),
    ("Stadium\\Media\\Material\\TrackBordersOff", 9),
    ("Stadium\\Media\\Material\\TrackWall", 14),
    ("Stadium\\Media\\Material\\TrackWallClips", 22),
    ("Stadium\\Media\\Modifier\\PlatformGrass\\OpenTechBorders", 76),
    ("Stadium\\Media\\Modifier\\PlatformGrass\\PlatformTech", 76),
    ("Stadium\\Media\\Modifier\\PlatformIce\\DecoHill", 21),
    ("Stadium\\Media\\Modifier\\PlatformIce\\PlatformTech", 74),
    ("Stadium\\Media\\Modifier\\Turbo\\Sign", 32),
    ("Stadium\\Media\\Modifier\\Turbo\\SignOff", 32),
];

/// Physics for a material link: the table, then the rules the table shows
/// (`Decal*`/`SpecialFX*`/`Turbo\Decal` -> NotCollidable 28, `ChronoFinish-*`
/// -> 32), else `None`.
pub fn physics_for_link(link: &str) -> Option<u8> {
    let l = link.to_ascii_lowercase();
    if let Some((_, p)) = MATERIAL_PHYSICS.iter().find(|(k, _)| k.to_ascii_lowercase() == l) {
        return Some(*p);
    }
    let base = l.rsplit('\\').next().unwrap_or(&l);
    if base.starts_with("decal") || base.starts_with("specialfx") || base.starts_with("racetriggerfx") {
        return Some(28);
    }
    if base.starts_with("chronofinish") {
        return Some(32);
    }
    None
}

/// The physics id most of a static object's collision triangles carry.
pub fn most_common_physics(so: &super::item::CPlugStaticObjectModel) -> Option<u8> {
    let sf = so.surface()?;
    let Surf::Mesh { triangles, .. } = &sf.surf else { return None };
    let mut counts = [0usize; 256];
    for t in triangles {
        counts[t.material_id as usize] += 1;
    }
    let (best, n) = counts.iter().enumerate().max_by_key(|(_, n)| **n)?;
    (*n > 0).then_some(best as u8)
}

/// Merge visuals that share a material slot AND a vertex layout (declaration
/// list, flags, no skin / tex-coord sets) into one, as the reference items
/// have exactly one visual per material. Vertex data is concatenated, indices
/// offset; a group is split when it would pass 65000 vertices (u16 indices).
pub fn coalesce(visuals: &[MergedVisual]) -> Vec<MergedVisual> {
    let mut out: Vec<MergedVisual> = Vec::new();
    for mv in visuals {
        let (Some(main), Some(stream)) = (mv.visual.main.as_ref(), mv.visual.stream()) else {
            out.push(mv.clone());
            continue;
        };
        let mergeable = main.tex_coord_sets.is_empty() && main.skin.is_none() && stream.base.index == -1 && main.vertex_streams.len() == 1;
        if !mergeable {
            out.push(mv.clone());
            continue;
        }
        let key = |v: &CPlugVisualIndexedTriangles| -> Option<(Vec<(u32, u32)>, u32, Option<bool>, Vec<u32>)> {
            let m = v.main.as_ref()?;
            let s = v.stream()?;
            Some((s.decls.iter().map(|d| (d.flags1, d.flags2)).collect(), m.chunk_flags, s.compress_local3d, v.chunks.clone()))
        };
        let k = key(&mv.visual);
        let target = out.iter_mut().find(|o| o.material == mv.material && key(&o.visual) == k && k.is_some() && o.visual.main.as_ref().map(|m| m.count as usize).unwrap_or(0) + main.count as usize <= 65000);
        match target {
            None => out.push(mv.clone()),
            Some(o) => append_visual(&mut o.visual, &mv.visual),
        }
    }
    out
}

fn append_visual(dst: &mut CPlugVisualIndexedTriangles, src: &CPlugVisualIndexedTriangles) {
    let src_main = src.main.as_ref().unwrap();
    let src_stream = src.stream().unwrap().clone();
    let src_idx = src.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
    let dm = dst.main.as_mut().unwrap();
    let base = dm.count as u32;
    dm.count += src_main.count;
    let ds = match dm.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
        Some(Node::VertexStream(s)) => s,
        _ => unreachable!("coalesce checked the stream"),
    };
    ds.count += src_stream.count;
    for (d, s) in ds.elems.iter_mut().zip(src_stream.elems.iter()) {
        match (d, s) {
            (Elem::Float2(a), Elem::Float2(b)) => a.extend_from_slice(b),
            (Elem::Float3(a), Elem::Float3(b)) => a.extend_from_slice(b),
            (Elem::Float4(a), Elem::Float4(b)) => a.extend_from_slice(b),
            (Elem::Word(a), Elem::Word(b)) => a.extend_from_slice(b),
            (Elem::Raw { bytes: a, .. }, Elem::Raw { bytes: b, .. }) => a.extend_from_slice(b),
            _ => unreachable!("coalesce matched the layouts"),
        }
    }
    if let Some(Elem::Float3(p)) = ds.elems.first() {
        dm.bounding_box = bbox(p);
    }
    match &mut dst.index_buffer {
        Some(ib) => ib.indices.extend(src_idx.iter().map(|i| i + base)),
        None => dst.index_buffer = Some(super::visual::IndexBuffer::delta(src_idx.iter().map(|i| i + base).collect())),
    }
}

/// A `.StaticObject.Gbx` pack file (a bare `CPlugStaticObjectModel` body):
/// parse it with the typed reader, pull its EXTERNAL mesh (`.Mesh.Gbx`, a
/// `CPlugSolid2Model` file) and shape (`.HitShape.Gbx`/`.Shape.Gbx`, a
/// `CPlugSurface` file) inline, and merge it placed by `at`. Material refs
/// of the three files live in three index spaces; they are offset here
/// (mesh +100000, shape +200000) so one resolver serves all of them.
pub fn add_static_object_file(store: &mut crate::store::DataStore, path: &str, at: &Xform, scale: f32, m: &mut Merged) -> R<()> {
    const MESH_OFF: i32 = 100_000;
    const SHAPE_OFF: i32 = 200_000;
    let model = store.load_model(path)?;
    if model.class_id != super::C_STATIC_OBJECT_MODEL {
        return Err(format!("{path}: class 0x{:08X} is not CPlugStaticObjectModel", model.class_id));
    }
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(model.external_indices().iter().copied());
    let mut r = super::Rd::new(&model.body, 0, lb);
    let mut so = super::item::CPlugStaticObjectModel::parse(&mut r).map_err(|e| format!("{path}: {e}"))?;
    let so_ext = model.externals.clone();
    let mut mesh_ext: Vec<(u32, String)> = Vec::new();
    let mut shape_ext: Vec<(u32, String)> = Vec::new();
    let name_in = |tbl: &[(u32, String)], i: i32| tbl.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone());
    if so.mesh.inline.is_none() && so.mesh.index >= 0 {
        let mp = name_in(&so_ext, so.mesh.index).ok_or_else(|| format!("{path}: mesh node {} is neither inline nor external", so.mesh.index))?;
        let mm = store.load_model(&mp)?;
        let mut lb = super::LookbackState::default();
        lb.defined_nodes.extend(mm.external_indices().iter().copied());
        let mut r = super::Rd::new(&mm.body, 0, lb);
        let mut s2 = super::solid2::CPlugSolid2Model::parse(&mut r).map_err(|e| format!("{mp}: {e}"))?;
        for mr in s2.materials.iter_mut() {
            if mr.inline.is_none() && mr.index >= 0 {
                mr.index += MESH_OFF;
            }
        }
        for cm in s2.custom_materials.iter_mut() {
            if let Some(nr) = cm.node.as_mut() {
                if nr.inline.is_none() && nr.index >= 0 {
                    nr.index += MESH_OFF;
                }
            }
        }
        mesh_ext = mm.externals.clone();
        so.mesh.inline = Some(Box::new(Node::Solid2(s2)));
    }
    if !so.is_mesh_collidable && so.shape.inline.is_none() && so.shape.index >= 0 {
        let sp = name_in(&so_ext, so.shape.index).ok_or_else(|| format!("{path}: shape node {} is neither inline nor external", so.shape.index))?;
        let sm = store.load_model(&sp)?;
        let mut lb = super::LookbackState::default();
        lb.defined_nodes.extend(sm.external_indices().iter().copied());
        let mut r = super::Rd::new(&sm.body, 0, lb);
        let mut sf = super::surface::CPlugSurface::parse(&mut r).map_err(|e| format!("{sp}: {e}"))?;
        for sm_ in sf.materials.iter_mut() {
            if let super::surface::SurfMaterial::Node(nr) = sm_ {
                if nr.inline.is_none() && nr.index >= 0 {
                    nr.index += SHAPE_OFF;
                }
            }
        }
        shape_ext = sm.externals.clone();
        so.shape.inline = Some(Box::new(Node::Surface(sf)));
    }
    let common = most_common_physics(&so);
    let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
        let p = if idx >= SHAPE_OFF {
            name_in(&shape_ext, idx - SHAPE_OFF)?
        } else if idx >= MESH_OFF {
            name_in(&mesh_ext, idx - MESH_OFF)?
        } else {
            name_in(&so_ext, idx)?
        };
        let link = material_link(&p);
        let phys = physics_for_link(&link).or_else(|| material_physics(store, &p).filter(|x| *x != 0)).or(common).unwrap_or(0);
        Some((p, link, phys))
    };
    m.add_static_object(&so, at, scale, &mut resolve).map_err(|err| format!("{path}: {err}"))
}

/// A pack ITEM (`CGameItemModel` wrapper whose entity model -- a static
/// object, a prefab, or a variant list of them -- lives in EXTERNAL files):
/// bake the geometry those externals point at. Variant lists (RoadSignC,
/// Flag16m...) contribute their FIRST prefab/static-object external only
/// (the placement's variant index is not consulted). Vegetation
/// (`.VegetTreeModel.Gbx`) has no mesh and is reported, not baked.
pub fn static_item_from_pack_item_report(store: &mut crate::store::DataStore, item_path: &str, ident: &str, author: &str, scale: f32, collection: u32) -> R<(Vec<u8>, Merged)> {
    let model = store.load_model(item_path)?;
    let mut m = Merged::default();
    m.editors = std::env::var_os("TINY_EDITORS").is_some();
    let mut geometry_externals: Vec<String> = Vec::new();
    let mut veget = 0usize;
    for (_, p) in &model.externals {
        let low = p.to_ascii_lowercase();
        if low.ends_with(".prefab.gbx") || low.ends_with(".staticobject.gbx") {
            geometry_externals.push(p.clone());
        } else if low.ends_with(".vegettreemodel.gbx") {
            veget += 1;
        }
    }
    if geometry_externals.is_empty() {
        if veget > 0 {
            return Err(format!("procedural vegetation ({veget} VegetTreeModel refs, no mesh)"));
        }
        return Err(format!("no prefab/static-object external (externals: {})", model.externals.iter().map(|(_, p)| p.rsplit('\\').next().unwrap_or(p).to_string()).collect::<Vec<_>>().join(", ")));
    }
    // first geometry external = variant 0 (others are alternative variants,
    // usually the same block at other sizes/angles)
    let first = geometry_externals[0].clone();
    if first.to_ascii_lowercase().ends_with(".prefab.gbx") {
        add_prefab(store, &first, &IDENTITY, scale, &mut m, 0)?;
    } else {
        add_static_object_file(store, &first, &IDENTITY, scale, &mut m)?;
    }
    if geometry_externals.len() > 1 {
        m.notes.push(format!("{} geometry variants; baked the first ({})", geometry_externals.len(), first.rsplit('\\').next().unwrap_or(&first)));
    }
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, editors: m.editors };
    let f = assemble(&m, &opts)?;
    Ok((super::write_file(&f), m))
}

/// The "StadiumOnTerrain" game skin: in a BlueBay map every Stadium-family
/// block draws some of its materials through
/// `BlueBay\Media\Modifier\StadiumOnTerrain\<slot>.Material.Gbx` instead of
/// `Stadium\Media\Material\<name>` (the slot table is
/// `Stadium\GameSkin\StadiumOnTerrain.GameSkin.gbx`). Items know nothing of
/// skins, so the link is remapped here: without it the wall faces under the
/// stands drew Stadium's wooden `TrackWallClips` where the original shows
/// BlueBay's concrete `TrackWallClipsInWorld` (2026-09-06).
/// `TINY_NO_SKIN=1` disables the remap.
pub fn skinned_material(inst: &CPlugMaterialUserInst, collection: u32) -> CPlugMaterialUserInst {
    const SKIN: &[(&str, &str)] = &[
        ("TrackWallClips", "TrackWallClipsInWorld"),
        ("TrackWall", "TrackWallInWorld"),
        ("TrackBorders", "TrackBordersInWorld"),
        ("TrackBordersOff", "TrackBordersOffInWorld"),
        ("Structure", "StructureInWorld"),
        ("Deco", "Deco"),
        ("DecoHill", "DecoHill"),
        ("DecoHill2", "DecoHill2"),
        ("DecalPaintSponsor4x1D", "DecalPaintSponsor4x1D"),
        ("DecalPaint2Sponsor4x1D", "DecalPaint2Sponsor4x1D"),
        ("DecalPaint2Sponsor4x1NoColorizeD", "DecalPaint2Sponsor4x1NoColorizeD"),
        ("DecalPaintSponsor4x1NoColorizeD", "DecalPaintSponsor4x1NoColorizeD"),
    ];
    if collection != 28 || std::env::var_os("TINY_NO_SKIN").is_some() {
        return inst.clone();
    }
    let Some(link) = inst.link() else { return inst.clone() };
    let Some(stem) = link.strip_prefix("Stadium\\Media\\Material\\") else { return inst.clone() };
    let Some((_, slot)) = SKIN.iter().find(|(name, _)| *name == stem) else { return inst.clone() };
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.link = crate::crystal_model::Id::Str(format!("BlueBay\\Media\\Modifier\\StadiumOnTerrain\\{slot}"));
    }
    owned
}
