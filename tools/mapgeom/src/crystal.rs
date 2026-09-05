//! Turn geometry into a Trackmania item the game will embed, render, and
//! SCALE: a mesh-modeler crystal (`CGameCommonItemEntityModelEdition` holding
//! a `CPlugCrystal`), the only item kind we found the game accepts from a map's
//! embedded ZIP with a placement scale honoured.
//!
//! The item is written around a TEMPLATE -- a known-good crystal item -- parsed
//! into the complete `crystal_model::CPlugCrystal`: the header, collector
//! chunks and item-model chunks around the crystal node are kept, the crystal
//! itself is re-emitted from the model with the material list and the first
//! Geometry layer replaced from `CrystalMesh`, and every other layer (Trigger,
//! SpawnPosition, ...) written back as parsed.
//!
//! Inline node numbering: the materials are consecutive inline nodes starting
//! at the template's first material node; every node index after them moves
//! by the change in material count, rewritten at the node-reference sites the
//! full body walk (`node::Graph`) records, and the header's node count with it.

use crate::crystal_model::{locate, CPlugCrystal, CPlugMaterialUserInst, Crystal, Face as ModelFace, Id, LayerKind, Lightmap, Located, NodeRef, Part};
use std::collections::HashMap;
use tmmaps::gbx::Gbx;

#[derive(Clone, Debug)]
pub struct Face {
    /// Corner indices into `positions`, 3 or more, counter-clockwise.
    pub verts: Vec<u32>,
    /// One texture coordinate per corner.
    pub uvs: Vec<[f32; 2]>,
    pub material: u32,
}

#[derive(Clone, Debug, Default)]
pub struct CrystalMesh {
    pub positions: Vec<[f32; 3]>,
    pub faces: Vec<Face>,
}

#[derive(Clone, Debug)]
pub struct MaterialSpec {
    /// The game material to link, e.g. `Stadium\Media\Material\RoadTech`.
    pub link: String,
    /// Surface physics id the car feels (Asphalt = 16).
    pub physics: u8,
}

impl CrystalMesh {
    /// Squeeze every UV into a tiny patch around the texture centre: a flat
    /// colour per material. NOT collapsed to one point -- a zero-area UV
    /// mapping gives NaN tangents, and materials with a normal map (Dirt,
    /// Wood, Rock, Stone) then crash the game once a map has enough faces.
    pub fn flatten_uvs(&mut self) {
        for f in &mut self.faces {
            for uv in &mut f.uvs {
                // Centre of ONE checker cell, not (0.5,0.5) where four meet.
                *uv = [0.25 + (uv[0] - 0.5) * 0.01, 0.25 + (uv[1] - 0.5) * 0.01];
            }
        }
    }

    /// Add triangles with their own vertex list; positions are de-duplicated
    /// exactly. UVs are BOX-mapped: each face projects onto the plane of its
    /// dominant normal axis, tiled every `uv_scale` metres, and v is squeezed
    /// into the texture's lit band (0.06..0.94 — Nadeo's RoadTech deck uses
    /// exactly that range; outside it the atlases are black).
    pub fn add_tris(&mut self, verts: &[[f32; 3]], tris: &[[u32; 3]], material: u32, uv_scale: f32) {
        let mut map: HashMap<[u32; 3], u32> = HashMap::new();
        for (i, p) in self.positions.iter().enumerate() {
            map.insert([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()], i as u32);
        }
        let mut index = |p: [f32; 3], positions: &mut Vec<[f32; 3]>| -> u32 {
            let k = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
            *map.entry(k).or_insert_with(|| {
                positions.push(p);
                (positions.len() - 1) as u32
            })
        };
        for t in tris {
            let ps = [verts[t[0] as usize], verts[t[1] as usize], verts[t[2] as usize]];
            if ps[0] == ps[1] || ps[1] == ps[2] || ps[0] == ps[2] {
                continue; // degenerate
            }
            let idx: Vec<u32> = ps.iter().map(|p| index(*p, &mut self.positions)).collect();
            let e1 = [ps[1][0] - ps[0][0], ps[1][1] - ps[0][1], ps[1][2] - ps[0][2]];
            let e2 = [ps[2][0] - ps[0][0], ps[2][1] - ps[0][1], ps[2][2] - ps[0][2]];
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
            let squeeze = |v: f32| 0.06 + 0.88 * v.rem_euclid(1.0);
            let uvs: Vec<[f32; 2]> = ps
                .iter()
                .map(|p| {
                    if ay >= ax && ay >= az {
                        [p[0] / uv_scale, squeeze(p[2] / uv_scale)]
                    } else if ax >= az {
                        [p[2] / uv_scale, squeeze(p[1] / uv_scale)]
                    } else {
                        [p[0] / uv_scale, squeeze(p[1] / uv_scale)]
                    }
                })
                .collect();
            self.faces.push(Face { verts: idx, uvs, material });
        }
    }
}


// ------------------------------------------------------------- the item

/// An item file opened around its crystal: the container, the decompressed
/// body, where the crystal node sits in it, and the crystal as a model.
pub struct ItemCrystal {
    pub gbx: Gbx,
    pub body: Vec<u8>,
    pub loc: Located,
    /// Offset just past the crystal node's FACADE.
    pub end: usize,
    pub model: CPlugCrystal,
}

impl ItemCrystal {
    pub fn open(bytes: &[u8]) -> Result<ItemCrystal, String> {
        let gbx = Gbx::parse(bytes);
        let body = gbx.body.clone();
        let loc = locate(&body)?;
        let (model, end, _) = CPlugCrystal::parse_with(&body, loc.at, loc.lookback.clone())?;
        Ok(ItemCrystal { gbx, body, loc, end, model })
    }

    /// Node indices of the crystal's inline material nodes: the first one and
    /// the count (they are consecutive in every item the game writes).
    pub fn material_node_range(&self) -> (u32, usize) {
        let idx = self.model.material_node_indices();
        let first = idx.first().copied().unwrap_or(self.loc.node_index as i32 + 1) as u32;
        for (i, n) in idx.iter().enumerate() {
            assert_eq!(*n as u32, first + i as u32, "material nodes are not consecutive");
        }
        (first, idx.len())
    }

    /// Write the file back with the (edited) model in place of the crystal.
    /// When the material node count changed by `delta`, every node index
    /// past the old material range moves: inside the model (`node_indices_mut`),
    /// and in the rest of the body at the node-reference words the full graph
    /// walk (`node::Graph`) recorded; the header's node count follows.
    pub fn close(mut self, old_first: u32, old_count: usize) -> Vec<u8> {
        let (new_first, new_count) = self.material_node_range();
        assert_eq!(new_first, old_first, "material nodes must start where the template's did");
        let delta = new_count as i64 - old_count as i64;
        let last_old = old_first as i64 + old_count as i64 - 1;
        if delta != 0 {
            for idx in self.model.node_indices_mut() {
                if (*idx as i64) > last_old && (*idx as i64) < new_first as i64 + new_count as i64 {
                    // a material node index: already renumbered by the caller
                    continue;
                }
                if (*idx as i64) > last_old {
                    *idx = (*idx as i64 + delta) as i32;
                }
            }
        }
        let mut prefix = self.body[..self.loc.at].to_vec();
        let mut suffix = self.body[self.end..].to_vec();
        if delta != 0 {
            let sites = node_sites(&self.gbx, &self.body);
            match sites {
                Some(sites) => {
                    for (o, v) in sites {
                        if (v as i64) <= last_old {
                            continue;
                        }
                        let nv = ((v as i64) + delta) as u32;
                        if o + 4 <= self.loc.at {
                            prefix[o..o + 4].copy_from_slice(&nv.to_le_bytes());
                        } else if o >= self.end {
                            suffix[o - self.end..o - self.end + 4].copy_from_slice(&nv.to_le_bytes());
                        }
                    }
                }
                None => {
                    // The graph walk failed: fall back to the byte scan for
                    // `index, class id` pairs (inline node definitions only).
                    eprintln!("warning: item body walk failed; renumbering nodes by pattern scan");
                    let lo = old_first as i64 + old_count as i64;
                    let mut i = 0;
                    while i + 8 <= suffix.len() {
                        let v = u32::from_le_bytes(suffix[i..i + 4].try_into().unwrap());
                        let c = u32::from_le_bytes(suffix[i + 4..i + 8].try_into().unwrap());
                        if (v as i64) >= lo && v < self.gbx.num_nodes && (c >> 24 == 0x2E || c >> 24 == 0x09) && (c & 0xFFF) == 0 {
                            suffix[i..i + 4].copy_from_slice(&((v as i64 + delta) as u32).to_le_bytes());
                            i += 8;
                        } else {
                            i += 1;
                        }
                    }
                }
            }
        }
        let mut lb = self.loc.lookback.clone();
        let mut out_body = prefix;
        self.model.write(&mut out_body, &mut lb);
        out_body.extend_from_slice(&suffix);
        let mut g = self.gbx;
        g.num_nodes = (g.num_nodes as i64 + delta) as u32;
        g.body = out_body.clone();
        g.write_body_recompressed(&out_body)
    }
}

/// Every node-reference word of the item body, as (offset, index), from the
/// full graph walk; `None` when the walk does not understand the file.
fn node_sites(gbx: &Gbx, body: &[u8]) -> Option<Vec<(usize, i32)>> {
    // Rebuild the container bytes the store's Model parser expects.
    let file = gbx.write_body_recompressed(body);
    let m = crate::store::Model::parse(&file, "item").ok()?;
    let g = m.graph().ok()?;
    Some(g.noderef_sites.clone())
}

/// A material slot's spec (link, physics) as the model holds it.
fn spec_of(m: &crate::crystal_model::Material) -> MaterialSpec {
    match m.inst() {
        Some(i) => MaterialSpec { link: i.link().unwrap_or("").to_string(), physics: i.physics() },
        None => MaterialSpec { link: m.name.clone(), physics: 16 },
    }
}

/// A material slot for `spec`, shaped like `proto` (the template's own
/// material node: same chunk versions and unknown fields) with the link and
/// physics replaced; a game material from scratch when there is no prototype.
fn material_slot(proto: Option<&CPlugMaterialUserInst>, spec: &MaterialSpec, node_index: u32) -> crate::crystal_model::Material {
    let mut inst = match proto {
        Some(p) => p.clone(),
        None => CPlugMaterialUserInst::game_material(&spec.link, spec.physics),
    };
    if let Some(main) = &mut inst.main {
        main.is_using_game_material = true;
        main.link = Id::Str(spec.link.clone());
        main.surface_physic_id = spec.physics;
    }
    crate::crystal_model::Material { name: String::new(), node: Some(NodeRef { index: node_index as i32, inline: Some(Box::new(inst)) }) }
}

/// Build the item: `template` bytes with the crystal replaced by `mesh` and
/// `materials`, and the Ident set to (`ident`, `author`).
pub fn build_item(template: &[u8], ident: &str, author: &str, materials: &[MaterialSpec], mesh: &CrystalMesh) -> Vec<u8> {
    build_item_with(template, ident, author, materials, mesh, 0)
}

/// Keep bit: retain the template's Geometry layers other than the first
/// (its collision mesh, which describes the TEMPLATE's shape, not `mesh`).
pub const KEEP_EXTRA_GEOMETRY: u8 = 16;

/// `keep` bits keep the template's own data instead of regenerating it:
/// 1 = materials, 2 = the first geometry layer's crystal, 4 = lightmap,
/// 8 = smoothing, 16 = the template's extra Geometry (collision) layers.
/// Bisecting aid; 0 for a generated item.
///
/// The template's crystal is parsed into the complete model
/// (`crystal_model`): its material list becomes `materials`, the FIRST
/// Geometry layer's crystal becomes `mesh`, and every other layer -- Trigger,
/// SpawnPosition, modifiers, lights -- is written back as parsed. A second
/// Geometry layer ("Geometry (Collisions)" in most Nadeo items) is the
/// template's collision shape; it is dropped unless `KEEP_EXTRA_GEOMETRY`,
/// because the generated mesh is the item's shape (the first layer is written
/// visible AND collidable, so it collides). The lightmap atlas and smoothing
/// groups are regenerated over every lit face, in layer order, so kept layers
/// stay covered. Node indices after the material nodes are renumbered
/// through the body's node-reference sites, and the header's node count moves
/// with them.
pub fn build_item_with(template: &[u8], ident: &str, author: &str, materials: &[MaterialSpec], mesh: &CrystalMesh, keep: u8) -> Vec<u8> {
    assert!(!materials.is_empty() && !mesh.faces.is_empty(), "empty crystal");
    // Two material slots with the same (link, physics) are FATAL in game:
    // NGameItemUtils::CreateSolid2Model (0x140F558D0) deduplicates equal
    // CPlugMaterialUserInsts when building the Solid2Model's CustomMaterials,
    // then writes the per-slot index map into that shorter array with no
    // bound check -- one garbage Release() per duplicate, so the crash is
    // random (whatever lies past the array). Nadeo items never carry
    // duplicates; merge ours and remap the faces.
    let (materials, mesh) = dedupe_materials(materials, mesh);
    let (materials, mesh) = (&materials[..], &mesh);
    for f in &mesh.faces {
        assert!((f.material as usize) < materials.len(), "face material out of range");
        assert!(f.verts.len() >= 3 && f.verts.len() == f.uvs.len());
    }
    let mut item = ItemCrystal::open(template).unwrap_or_else(|e| panic!("template: {e}"));
    let (first_node, n_old) = item.material_node_range();
    let proto = item.model.materials.first().and_then(|m| m.inst()).cloned();

    // ---- materials
    if keep & 1 == 0 {
        item.model.materials = materials.iter().enumerate().map(|(i, m)| material_slot(proto.as_ref(), m, first_node + i as u32)).collect();
    }
    let n_mat = item.model.materials.len();

    // ---- layers: the first Geometry layer takes the mesh
    let first_geom = item.model.layers.iter().position(|l| matches!(l.kind, LayerKind::Geometry { .. })).expect("template has no Geometry layer");
    if keep & KEEP_EXTRA_GEOMETRY == 0 {
        let mut i = 0;
        item.model.layers.retain(|l| {
            let drop = matches!(l.kind, LayerKind::Geometry { .. }) && i != first_geom;
            i += 1;
            !drop
        });
    } else {
        // Kept geometry layers index the NEW material list: clamp what no
        // longer exists to the last slot.
        for (i, l) in item.model.layers.iter_mut().enumerate() {
            if i == first_geom {
                continue;
            }
            if let Some(c) = l.kind.crystal_mut() {
                for f in &mut c.faces {
                    if f.material as usize >= n_mat {
                        f.material = n_mat as i32 - 1;
                    }
                }
                c.u02 = c.faces.iter().map(|f| f.material).max().unwrap_or(0);
            }
        }
    }
    let first_geom = item.model.layers.iter().position(|l| matches!(l.kind, LayerKind::Geometry { .. })).unwrap();
    if keep & 2 == 0 {
        if let LayerKind::Geometry { crystal, u02, is_visible, collidable, .. } = &mut item.model.layers[first_geom].kind {
            fill_crystal(crystal, mesh, n_mat);
            // GeometryLayer tail: one int per group.
            *u02 = (0..crystal.groups.len() as i32).collect();
            *is_visible = true;
            *collidable = true;
        }
    }
    // Trigger layers keep their crystal but index the new material list.
    for l in &mut item.model.layers {
        if let LayerKind::Trigger { crystal, .. } = &mut l.kind {
            for f in &mut crystal.faces {
                if f.material as usize >= n_mat {
                    f.material = n_mat as i32 - 1;
                }
            }
            crystal.u02 = crystal.faces.iter().map(|f| f.material).max().unwrap_or(0);
        }
    }

    // ---- lightmap and smoothing over every lit face, in layer order
    if keep & 4 == 0 {
        item.model.lightmap = Some(lightmap_atlas(&item.model));
    }
    if keep & 8 == 0 {
        // Every Nadeo/community crystal read marks each face with group 2 (the
        // third of the three floats 0,1,2); 0 smoothed a box into a black blob.
        item.model.per_face_ints = vec![2; item.model.lit_face_count()];
    }

    let out = item.close(first_node, n_old);
    let out = crate::tiny_assets::set_header_ident(&out, ident, author);
    crate::tiny_assets::set_body_ident_nameless(&out, ident)
}

/// Fill a Geometry layer's crystal from `mesh`: the template's version,
/// visual levels and anchors stay; groups, positions, tex coords and faces
/// are the mesh's.
fn fill_crystal(c: &mut Crystal, mesh: &CrystalMesh, n_mat: usize) {
    assert_eq!(c.version, 37, "template crystal version {}, writer knows 37", c.version);
    // Groups form a tree: folders (no name, parent -1, a list of children)
    // and leaves ("part", parent = folder index, no children); faces name
    // leaves only. A leaf whose parent is itself hangs the loader forever.
    c.groups = vec![
        Part { u01: 0, u02: 1, u03: -1, name: String::new(), u04: -1, u05: vec![1] },
        Part { u01: 0, u02: 1, u03: 0, name: "part".into(), u04: -1, u05: Vec::new() },
    ];
    c.anchor_infos.clear();
    c.is_embedded = true;
    // U02 = max face material index, U03 = max face group index (the game
    // reads them as width selectors for the per-face optimized ints;
    // mesh archive 0x1413D0964).
    c.u02 = mesh.faces.iter().map(|f| f.material as i32).max().unwrap_or(0);
    c.u03 = 1;
    c.positions = mesh.positions.clone();
    // edges: informational count, then an empty optimized array
    let mut edges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    for f in &mesh.faces {
        for k in 0..f.verts.len() {
            let a = f.verts[k];
            let b = f.verts[(k + 1) % f.verts.len()];
            edges.insert((a.min(b), a.max(b)));
        }
    }
    c.edge_count = edges.len() as u32;
    c.edges.clear();
    // texcoords, de-duplicated, then one index per face corner
    let mut uv_index: HashMap<[u32; 2], u32> = HashMap::new();
    c.tex_coords.clear();
    c.faces.clear();
    for f in &mesh.faces {
        let mut face = ModelFace { verts: f.verts.clone(), ..ModelFace::default() };
        for uv in &f.uvs {
            let k = [uv[0].to_bits(), uv[1].to_bits()];
            let i = *uv_index.entry(k).or_insert_with(|| {
                c.tex_coords.push(*uv);
                (c.tex_coords.len() - 1) as u32
            });
            face.uv_index.push(i);
        }
        assert!((f.material as usize) < n_mat);
        face.material = f.material as i32;
        face.group = 1; // the leaf
        c.faces.push(face);
    }
    c.face_extra = vec![Vec::new(); c.faces.len()];
    c.u04 = 0;
}

/// Lightmap coords for every lit face of `model`: a non-overlapping atlas,
/// one grid cell per face with the corners spread on a circle inside it.
/// Overlapping lightmap UVs (the texture UVs reused) made the editor draw the
/// item as a translucent bounding box instead of a mesh.
fn lightmap_atlas(model: &CPlugCrystal) -> Lightmap {
    let lit: Vec<&Crystal> = model
        .layers
        .iter()
        .filter_map(|l| match &l.kind {
            LayerKind::Geometry { crystal, is_visible, .. } if l.base.is_enabled && *is_visible => Some(crystal),
            _ => None,
        })
        .collect();
    let n_faces: usize = lit.iter().map(|c| c.faces.len()).sum();
    let grid = (n_faces as f64).sqrt().ceil().max(1.0) as usize;
    // TINY_LM_SCALE shrinks the atlas into a corner (experiment knob; the
    // "lightmap budget" theory it served was the duplicate-material crash).
    let lm_scale: f64 = std::env::var("TINY_LM_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let cell = lm_scale / grid as f64;
    let mut coords: Vec<[u16; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut fi = 0usize;
    for c in lit {
        for f in &c.faces {
            let (cx, cy) = ((fi % grid) as f64 * cell, (fi / grid) as f64 * cell);
            let n = f.verts.len();
            for k in 0..n {
                let ang = std::f64::consts::TAU * k as f64 / n as f64;
                let u = cx + cell * (0.5 + 0.4 * ang.cos());
                let v = cy + cell * (0.5 + 0.4 * ang.sin());
                coords.push([(u * 65535.0) as u16, (v * 65535.0) as u16]);
                indices.push((coords.len() - 1) as u32);
            }
            fi += 1;
        }
    }
    Lightmap::V2 { coords, indices }
}

/// Rewrite an existing item's materials: `f` maps every slot's (link,
/// physics) to a new one; slots that become equal by link are merged (the
/// surviving slot takes the physics of the one covering the most faces) and
/// the faces of EVERY layer with a crystal -- visible geometry, collision
/// geometry, triggers -- are remapped. Every layer, chunk and unknown field is
/// preserved; node indices and the header's node count follow the new count.
pub fn edit_materials(item: &[u8], f: impl Fn(&MaterialSpec) -> MaterialSpec) -> Vec<u8> {
    let mut it = ItemCrystal::open(item).unwrap_or_else(|e| panic!("item: {e}"));
    let (first_node, n_old) = it.material_node_range();
    let proto = it.model.materials.first().and_then(|m| m.inst()).cloned();
    let specs: Vec<MaterialSpec> = it.model.materials.iter().map(|m| f(&spec_of(m))).collect();
    // faces per slot, over every crystal
    let mut faces_per = vec![0usize; specs.len()];
    for l in &it.model.layers {
        if let Some(c) = l.kind.crystal() {
            for face in &c.faces {
                if face.material >= 0 && (face.material as usize) < faces_per.len() {
                    faces_per[face.material as usize] += 1;
                }
            }
        }
    }
    // Surviving slots keep their OWN node (gameplay id, tiling, every unknown
    // field) with the link and physics replaced; a merged-away slot's faces
    // move onto the first slot with its link.
    let mut out: Vec<MaterialSpec> = Vec::new();
    let mut out_src: Vec<usize> = Vec::new();
    let mut out_faces: Vec<usize> = Vec::new();
    let mut remap: Vec<u32> = Vec::with_capacity(specs.len());
    for (i, m) in specs.iter().enumerate() {
        match out.iter().position(|o| o.link == m.link) {
            Some(p) => {
                if faces_per[i] > out_faces[p] {
                    out[p].physics = m.physics;
                    out_faces[p] = faces_per[i];
                }
                remap.push(p as u32);
            }
            None => {
                out.push(m.clone());
                out_src.push(i);
                out_faces.push(faces_per[i]);
                remap.push((out.len() - 1) as u32);
            }
        }
    }
    let old_slots = std::mem::take(&mut it.model.materials);
    it.model.materials = out
        .iter()
        .enumerate()
        .map(|(i, m)| material_slot(old_slots[out_src[i]].inst().or(proto.as_ref()), m, first_node + i as u32))
        .collect();
    for l in &mut it.model.layers {
        if let Some(c) = l.kind.crystal_mut() {
            for face in &mut c.faces {
                if face.material >= 0 && (face.material as usize) < remap.len() {
                    face.material = remap[face.material as usize] as i32;
                }
            }
            c.u02 = c.faces.iter().map(|f| f.material).max().unwrap_or(0);
        }
    }
    it.close(first_node, n_old)
}

/// The game material a collision surface's physics name stands for: what the
/// car feels is also what the eye should see. Unknown names get the road
/// material with their own physics id, so the surface still drives right.
pub fn material_for_physics_name(name: &str) -> MaterialSpec {
    material_for_physics_name_in(name, 26)
}

/// Same, for a given map collection. Item material links are WHITELISTED per
/// environment: Stadium (26) accepts `Stadium\Media\Material\*`; BlueBay (28)
/// and the other 2026 environments cull every face set that names one, and
/// only the mesh-editor family `Editors\MeshEditorMedia\Materials\*` draws
/// there (flat tinted surfaces, no textures -- the best this build allows).
pub fn material_for_physics_name_in(name: &str, collection: u32) -> MaterialSpec {
    let phys = crate::scene::physics_id(name).unwrap_or(16);
    if collection != 26 {
        // The mesh-editor family, one flat tint each (Asphalt slate grey,
        // Concrete white, Grass mint, Sand cream, Rock/Metal grey, Dirt
        // terracotta, Wood tan, Ice cyan, Snow white, Plastic yellow). The
        // physics id is the surface's own -- the game compares link AND
        // physics when deduplicating, so two slots may share a link.
        let link = match name {
            "Asphalt" | "WetAsphalt" => "Editors\\MeshEditorMedia\\Materials\\Asphalt",
            "Rubber" | "SlidingRubber" => "Editors\\MeshEditorMedia\\Materials\\Concrete",
            "Concrete" | "Pavement" | "WetPavement" => "Editors\\MeshEditorMedia\\Materials\\Concrete",
            "Metal" | "ResonantMetal" | "MetalTrans" => "Editors\\MeshEditorMedia\\Materials\\Metal",
            "Grass" | "WetGrass" => "Editors\\MeshEditorMedia\\Materials\\Grass",
            "Dirt" | "DirtRoad" | "WetDirtRoad" => "Editors\\MeshEditorMedia\\Materials\\Dirt",
            "Ice" => "Editors\\MeshEditorMedia\\Materials\\Ice",
            "Sand" => "Editors\\MeshEditorMedia\\Materials\\Sand",
            "Wood" => "Editors\\MeshEditorMedia\\Materials\\Wood",
            "Rock" => "Editors\\MeshEditorMedia\\Materials\\Rock",
            "Snow" => "Editors\\MeshEditorMedia\\Materials\\Snow",
            "RoadSynthetic" => "Editors\\MeshEditorMedia\\Materials\\Plastic",
            _ => "Editors\\MeshEditorMedia\\Materials\\Concrete",
        };
        let mut link = link.to_string();
        if let Ok(ov) = std::env::var("TINY_LINK_OVERRIDE") {
            for kv in ov.split(';') {
                if let Some((k, v)) = kv.split_once('=') {
                    if k == name {
                        link = format!("Editors\\MeshEditorMedia\\Materials\\{v}");
                    }
                }
            }
        }
        return MaterialSpec { link, physics: phys };
    }
    let link = match name {
        "Asphalt" | "WetAsphalt" => "Stadium\\Media\\Material\\RoadTech",
        "Rubber" | "SlidingRubber" => "Stadium\\Media\\Material\\TrackBorders",
        "Concrete" | "Pavement" | "WetPavement" => "Stadium\\Media\\Material\\PlatformTech",
        "Metal" | "ResonantMetal" => "Stadium\\Media\\Material\\Technics",
        "Grass" | "WetGrass" => "Stadium\\Media\\Material\\Grass",
        "Dirt" | "DirtRoad" | "WetDirtRoad" => "Stadium\\Media\\Material\\RoadDirt",
        "Ice" => "Stadium\\Media\\Material\\RoadIce",
        // Only names that resolve in the Stadium pack (mapgeom resolve):
        // RoadTech TrackBorders PlatformTech Technics TechnicsTrims Grass
        // RoadDirt RoadIce RoadBump DecoHill TrackWall. No Sand/Rock/Wood.
        "Sand" => "Stadium\\Media\\Material\\RoadDirt",
        "Wood" => "Stadium\\Media\\Material\\PlatformTech",
        "Rock" => "Stadium\\Media\\Material\\TrackWall",
        "Snow" => "Stadium\\Media\\Material\\RoadIce",
        _ => "Stadium\\Media\\Material\\RoadTech",
    };
    MaterialSpec { link: link.to_string(), physics: phys }
}

/// Merge material slots that are equal for the game (same link and physics)
/// and remap the faces onto the surviving slots, in first-seen order.
pub fn dedupe_materials(materials: &[MaterialSpec], mesh: &CrystalMesh) -> (Vec<MaterialSpec>, CrystalMesh) {
    // Keyed on the LINK alone: the map still crashed at F5633C with slots
    // that differed only in physics (Concrete for both rubber borders and
    // concrete decks), so the game's equality ignores the physics byte at
    // this point. The surviving slot takes the physics of whichever slot
    // covers the most faces.
    let mut faces_per: Vec<usize> = vec![0; materials.len()];
    for f in &mesh.faces {
        faces_per[f.material as usize] += 1;
    }
    let mut out: Vec<MaterialSpec> = Vec::new();
    let mut out_faces: Vec<usize> = Vec::new();
    let mut remap: Vec<u32> = Vec::with_capacity(materials.len());
    for (i, m) in materials.iter().enumerate() {
        let pos = out.iter().position(|o| o.link == m.link);
        remap.push(match pos {
            Some(p) => {
                if faces_per[i] > out_faces[p] {
                    out[p].physics = m.physics;
                    out_faces[p] = faces_per[i];
                }
                p as u32
            }
            None => {
                out.push(m.clone());
                out_faces.push(faces_per[i]);
                (out.len() - 1) as u32
            }
        });
    }
    let mut mesh = mesh.clone();
    for f in &mut mesh.faces {
        f.material = remap[f.material as usize];
    }
    (out, mesh)
}

/// A `LINK|PHYS` label from `Collector::link_labels` as a material spec. A
/// label with no link (an inline material with no file) falls back to a
/// neutral game material for its physics.
pub fn material_for_link_label(label: &str) -> MaterialSpec {
    let (link, phys) = label.rsplit_once('|').unwrap_or((label, "16"));
    let physics: u8 = phys.parse().unwrap_or(16);
    if link.is_empty() || !link.contains('\\') {
        return MaterialSpec { link: neutral_link_for(physics).to_string(), physics };
    }
    MaterialSpec { link: link.to_string(), physics }
}

/// The scene's material labels mapped onto game materials: a label that is a
/// physics name keeps its physics id on a neutral material; anything else
/// gets the collection's material of that name.
pub fn material_for_label(collection: &str, label: &str) -> MaterialSpec {
    let phys = crate::scene::physics_id(label);
    if let Some(p) = phys {
        return MaterialSpec { link: neutral_link_for(p).to_string(), physics: p };
    }
    MaterialSpec { link: format!("{collection}\\Media\\Material\\{label}"), physics: 16 }
}

fn neutral_link_for(phys: u8) -> &'static str {
    match crate::scene::physics_name(phys) {
        "Grass" => "Stadium\\Media\\Material\\Grass",
        "Dirt" | "DirtRoad" => "Stadium\\Media\\Material\\RoadDirt",
        "Ice" => "Stadium\\Media\\Material\\RoadIce",
        "Concrete" => "Stadium\\Media\\Material\\PlatformTech",
        _ => "Stadium\\Media\\Material\\RoadTech",
    }
}


/// Read the item's first geometry layer back into a `CrystalMesh` plus its
/// material links: the round-trip oracle for the writer (an item rebuilt from
/// its own decoded mesh must load like the original).
pub fn decode_template(bytes: &[u8]) -> (Vec<MaterialSpec>, CrystalMesh) {
    let it = ItemCrystal::open(bytes).unwrap_or_else(|e| panic!("template: {e}"));
    let materials: Vec<MaterialSpec> = it.model.materials.iter().map(spec_of).collect();
    let layer = it.model.first_geometry().expect("template has no Geometry layer");
    let c = layer.kind.crystal().unwrap();
    let mut mesh = CrystalMesh { positions: c.positions.clone(), faces: Vec::new() };
    for f in &c.faces {
        mesh.faces.push(Face { verts: f.verts.clone(), uvs: c.face_uvs(f), material: f.material.max(0) as u32 });
    }
    (materials, mesh)
}
