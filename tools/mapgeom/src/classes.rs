//! Chunk layouts, class by class, for everything on the road from a block or
//! an item to triangles.
//!
//! Only classes this project actually walks are here. The list is deliberately
//! short: each one was added because a real file in `dedicated_TMStadium.pak`
//! or a real `.Map.Gbx` needed it, and `mapgeom classes` prints what has been
//! walked so the next gap names itself.

use crate::node::*;
use crate::reader::R;

/// A ten-bit signed normal, as `Dec3N` packs three of them into a u32.
fn tenb(v: u32) -> f32 {
    let v = v & 0x3FF;
    // 0x200 is -1.0, 0x1FF is +1.0; the game's own inverse of float_to_tenb.
    let s = if v >= 0x200 { v as f32 - 1024.0 } else { v as f32 };
    (s / 511.0).clamp(-1.0, 1.0)
}

fn dec3n(v: u32) -> [f32; 3] {
    [tenb(v), tenb(v >> 10), tenb(v >> 20)]
}

/// The `CPlugVisual` chunk-flags word, as `0x0900600D..F` writes it.
#[derive(Clone, Copy, Default, Debug)]
pub struct VisualFlags {
    pub skin_index_count: u32,
    pub use_vertex_normal: bool,
    pub use_vertex_color: bool,
    pub compress_float3_local3d: bool,
    pub compress_float4_color: bool,
    pub bit22: bool,
}

impl VisualFlags {
    fn from_word(w: u32) -> VisualFlags {
        VisualFlags {
            skin_index_count: w & 7,
            use_vertex_normal: w & (1 << 5) != 0,
            use_vertex_color: w & (1 << 6) != 0,
            compress_float3_local3d: w & (1 << 7) != 0,
            compress_float4_color: w & (1 << 8) != 0,
            bit22: w & (1 << 9) != 0,
        }
    }
}

impl<'a> Graph<'a> {
    /// Is this a chunk we can parse? Decides whether a *skippable* chunk is
    /// walked or stepped over. Unknown non-skippable chunks are fatal wherever
    /// they appear, which is what `chunk` does with them.
    pub fn chunk_is_known(&self, class_id: u32, cid: u32) -> bool {
        known(class_id, cid)
    }

    /// A node body with no chunk framing: the class IS the struct.
    pub fn plain_body(&mut self, class_id: u32) -> R<Node> {
        match class_id {
            C_PREFAB => {
                let _version = self.r.u32()?;
                let _updated = self.r.take(8)?;
                let _url = self.r.string()?;
                let _u01 = self.r.i32()?;
                let n = self.r.u32()? as usize;
                if n > 1_000_000 {
                    return Err(format!("prefab claims {} entities", n));
                }
                let _u02 = self.r.i32()?;
                let mut ents = Vec::with_capacity(n);
                for i in 0..n {
                    // Breadcrumbs: a prefab that ends early is almost always
                    // one entity whose payload this reader does not know, and
                    // "entity 37 of 51" is the difference between a bug report
                    // and a guess. `MAPGEOM_TRACE=1` prints every step of the
                    // walk, which is how the reader for a new class gets
                    // written.
                    let ctx = |e: String| format!("entity {}/{}: {}", i, n, e);
                    crate::reader::trace(|| format!("  ent {}/{} at 0x{:x}", i, n, self.r.o));
                    let model = self.noderef().map_err(ctx)?;
                    crate::reader::trace(|| format!("    model {} at 0x{:x}", model, self.r.o));
                    let rot = self.r.quat().map_err(ctx)?;
                    let pos = self.r.vec3().map_err(ctx)?;
                    crate::reader::trace(|| format!("    params at 0x{:x} id 0x{:08X}", self.r.o, self.r.peek_u32().unwrap_or(0)));
                    self.prefab_ent_params().map_err(ctx)?;
                    let _u01 = self.r.bytes_pfx().map_err(ctx)?;
                    ents.push(PrefabEnt { model, rot, pos });
                }
                Ok(Node::Prefab(Prefab { ents }))
            }
            C_STATIC_OBJECT => {
                let _version = self.r.u32()?;
                let mesh = self.noderef()?;
                let mesh_collidable = self.r.bool8()?;
                let shape = if mesh_collidable { -1 } else { self.noderef()? };
                Ok(Node::StaticObject(StaticObject { mesh, mesh_collidable, shape }))
            }
            // An item may hand out a LIST of entity models, one per tag set
            // (a gate has left, right and centre variants). Nothing here knows
            // which variant a placement wants, so the first is taken and the
            // rest are ignored — see MAPGEOM.md, "what is still missing".
            C_VARIANT_LIST => {
                let version = self.r.u32()?;
                let n = self.r.u32()? as usize;
                let mut first = -1;
                for i in 0..n {
                    self.r.array(|r| Ok((r.string()?, r.string()?)))?; // Tags
                    let m = self.noderef()?;
                    if i == 0 {
                        first = m;
                    }
                    if version >= 1 {
                        self.r.bool32()?; // HiddenInManualCycle
                    }
                }
                Ok(Node::ItemModel(first))
            }
            // NPlugDyna_SConstraintModel: a spring, no geometry.
            0x2F074000 => {
                self.r.take(4 * 5)?;
                Ok(Node::Other(class_id))
            }
            // The remaining body-less classes are placement metadata we do not
            // read; reaching one means the walk went somewhere unexpected.
            c => Err(format!("class 0x{:08X} has no chunk framing and no reader", c)),
        }
    }

    /// A prefab entity's trailing parameter blob: a chunk id and its payload.
    fn prefab_ent_params(&mut self) -> R<()> {
        let chunk_id = self.r.i32()?;
        match chunk_id {
            -1 => Ok(()),
            // NPlugDynaObjectModel_SInstanceParams
            0x2F0B6000 => {
                let v = self.r.i32()?;
                self.r.take(4 * 3)?; // PeriodSc, TextureId, IsKinematic
                if v >= 1 {
                    self.r.take(4 * 3)?; // PeriodScMax, Phase01, Phase01Max
                }
                if v >= 2 {
                    self.r.take(4)?; // CastStaticShadow
                }
                Ok(())
            }
            // NPlugDyna_SPrefabConstraintParams
            0x2F0C8000 => {
                let _v = self.r.u32()?;
                self.r.take(4 * 2 + 12 * 2)?;
                Ok(())
            }
            // NPlugItemPlacement_SPlacement
            0x2F0A9000 => {
                let _v = self.r.u32()?;
                let _layout = self.r.i32()?;
                self.r.array(|r| r.array(|r| Ok((r.string()?, r.string()?))))?;
                Ok(())
            }
            // NPlugItemPlacement_SPlacementGroup
            0x2F0D8000 => {
                let _v = self.r.u32()?;
                self.r.array(|r| {
                    let _v = r.u32()?;
                    let _layout = r.i32()?;
                    r.array(|r| r.array(|r| Ok((r.string()?, r.string()?))))?;
                    Ok(())
                })?;
                self.r.array(|r| r.u16())?;
                // GbxLoc is a position and a quaternion — 28 bytes, not the
                // 48 of an Iso4. Reading it as an Iso4 swallows the rest of
                // the file, and the failure surfaces as "this prefab ends
                // early" one entity later.
                self.r.array(|r| {
                    r.vec3()?;
                    r.quat()?;
                    Ok(())
                })?;
                Ok(())
            }
            // NPlugStaticObjectModel_SInstanceParams
            0x2F0D9000 => {
                let _v = self.r.u32()?;
                let _phase = self.r.f32()?;
                Ok(())
            }
            c => Err(format!("prefab entity params chunk 0x{:08X} has no reader", c)),
        }
    }

    /// One chunk of one node.
    pub fn chunk(&mut self, class_id: u32, cid: u32, acc: &mut Acc) -> R<()> {
        match cid {
            // ---------------------------------------------- CPlugSurface
            0x0900C003 => {
                acc.touched = true;
                let version = self.r.u32()?;
                let surf_version = if version < 2 { 0 } else { self.r.u32()? };
                self.surf(surf_version, &mut acc.surface, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0])?;
                let n_mats = self.r.u32()? as usize;
                for _ in 0..n_mats {
                    let has = self.r.bool32()?;
                    if has {
                        self.noderef()?;
                    } else {
                        self.r.take(2)?;
                    }
                }
                let surface_ids_len = if (version == 3 && n_mats == 0) || version >= 4 {
                    let n = self.r.u32()? as usize;
                    self.r.take(2 * n)?;
                    n
                } else {
                    0
                };
                if version < 3 {
                    let n = self.r.u32()? as usize;
                    self.r.take(n)?;
                }
                if version >= 3 && surface_ids_len == 0 {
                    let n = self.r.u32()? as usize;
                    self.r.take(2 * n)?;
                }
                if version >= 1 {
                    self.noderef()?; // CPlugSkel
                }
                Ok(())
            }

            // ------------------------------------------ CPlugSolid2Model
            0x090BB000 => {
                acc.touched = true;
                self.solid2(&mut acc.solid2)
            }

            // ------------------------------- CPlugVisual / CPlugVisual3D
            0x09006001 => {
                self.r.lookback()?;
                Ok(())
            }
            0x09006004 => {
                self.noderef()?;
                Ok(())
            }
            0x09006005 => {
                self.r.array(|r| Ok([r.i32()?, r.i32()?, r.i32()?]))?;
                Ok(())
            }
            0x09006009 => {
                self.r.bool32()?;
                Ok(())
            }
            0x0900600B => {
                self.r.array(|r| {
                    r.i32()?;
                    r.i32()?;
                    r.boxf()
                })?;
                Ok(())
            }
            0x0900600D | 0x0900600E | 0x0900600F => {
                acc.touched = true;
                let version = if cid == 0x0900600F { self.r.u32()? } else { 0 };
                self.visual_common(acc)?;
                if cid != 0x0900600D {
                    self.r.array(|r| r.take(20).map(|_| ()))?; // bitmapElemToPacks
                }
                if cid == 0x0900600F {
                    if version >= 5 {
                        self.r.array(|r| r.u16())?;
                    }
                    if version >= 6 {
                        self.r.u32()?;
                        let n = self.r.u32()? as usize;
                        if n > 0 {
                            self.r.take(n - 4)?;
                        }
                    }
                }
                Ok(())
            }
            0x09006010 => {
                let _v = self.r.u32()?;
                let morph = self.r.u32()?;
                if morph != 0 {
                    return Err(format!("visual morph_count {} (only 0 is understood)", morph));
                }
                Ok(())
            }
            0x0902C002 => {
                self.noderef()?;
                Ok(())
            }
            0x0902C004 => {
                acc.touched = true;
                self.visual_inline_vertices(acc)
            }
            0x0906A000 => {
                acc.touched = true;
                let idx = self.r.array(|r| r.u16())?;
                acc.visual.indices = idx.into_iter().map(|v| v as u32).collect();
                acc.visual.index_is_absolute = true;
                Ok(())
            }
            0x0906A001 => {
                acc.touched = true;
                let has = self.r.bool32()?;
                if has {
                    // An inline CPlugIndexBuffer, as a nested chunk list.
                    loop {
                        let sub = self.r.u32()?;
                        if sub == 0xFACADE01 {
                            break;
                        }
                        match sub {
                            0x09057000 | 0x09057001 => {
                                let flags = self.r.u32()?;
                                let idx = self.r.array(|r| r.u16())?;
                                acc.visual.indices = idx.into_iter().map(|v| v as u32).collect();
                                // flags bit 1 marks an absolute index list; the
                                // relative form is a delta chain from 0.
                                acc.visual.index_is_absolute = flags & 2 != 0;
                            }
                            c => return Err(format!("index buffer chunk 0x{:08X} has no reader", c)),
                        }
                    }
                }
                Ok(())
            }

            // ----------------------------------------- CPlugVertexStream
            0x09056000 => {
                acc.touched = true;
                self.vertex_stream(&mut acc.vstream)
            }

            // ------------------------------ CGameCtnCollector / item model
            // An item's geometry hangs off `0x2E002019`'s EntityModel; the
            // rest of these exist only so the chunk walk reaches it.
            0x2E001009 => {
                self.r.string()?; // pagePath
                if self.r.bool32()? {
                    self.noderef()?;
                }
                self.r.lookback()?;
                Ok(())
            }
            0x2E00100B => {
                self.r.meta()?;
                Ok(())
            }
            0x2E00100C | 0x2E00100D => {
                self.r.string()?;
                Ok(())
            }
            0x2E00100E => {
                self.r.bool32()?;
                self.r.i32()?;
                Ok(())
            }
            0x2E001010 => {
                let v = self.r.u32()?;
                self.noderef()?;
                let skin = self.r.string()?;
                if v >= 2 && skin.is_empty() {
                    self.noderef()?;
                }
                Ok(())
            }
            0x2E001011 => {
                let v = self.r.u32()?;
                self.r.bool32()?;
                self.r.bool32()?;
                self.r.i32()?;
                if v >= 1 {
                    self.r.u8()?; // EProdState is a byte
                }
                Ok(())
            }
            0x2E001012 => {
                self.r.take(16)?;
                Ok(())
            }
            0x2E002008 => {
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    self.noderef()?;
                }
                Ok(())
            }
            0x2E00200C | 0x2E002013 | 0x2E00201A => {
                self.noderef()?;
                Ok(())
            }
            0x2E002009 => {
                self.r.u32()?;
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    self.noderef()?;
                }
                Ok(())
            }
            0x2E002012 => {
                self.r.take(12 + 4 * 4)?;
                Ok(())
            }
            0x2E00201C => {
                let v = self.r.u32()?;
                if v != 5 {
                    return Err(format!("item defaultPlacement chunk version {} (expected 5)", v));
                }
                self.noderef()?;
                Ok(())
            }
            0x2E00201E => {
                let v = self.r.u32()?;
                let arch = self.r.string()?;
                if v >= 5 && arch.is_empty() {
                    self.noderef()?;
                }
                if v < 6 {
                    return Ok(());
                }
                self.r.string()?;
                if v < 7 {
                    return Ok(());
                }
                self.noderef()?;
                Ok(())
            }
            // CGameItemModel waypoint properties. gbx-py documents up to
            // version 12; the Stadium pack ships 13, which drops the
            // scriptWithSettings node reference. Read off the bytes of
            // ShowScreen.Item.Gbx and checked against every item in the pack.
            0x2E00201F => {
                let v = self.r.u32()?;
                self.r.u32()?; // waypointType
                if v < 6 {
                    return Ok(());
                }
                self.r.bool32()?; // DisableLightmap
                if (10..13).contains(&v) {
                    self.noderef()?; // scriptWithSettings
                }
                if v < 11 {
                    return Ok(());
                }
                self.r.u8()?; // flags
                if v < 12 {
                    return Ok(());
                }
                self.noderef()?; // PodiumClipList
                self.noderef()?; // IntroClipList
                Ok(())
            }
            0x2E002020 => {
                let _v = self.r.u32()?;
                self.r.string()?; // iconFid
                self.r.u8()?;
                Ok(())
            }
            0x2E002021 => {
                self.r.take(8)?;
                Ok(())
            }
            0x2E002023 => {
                self.r.take(9)?;
                Ok(())
            }
            0x2E002015 => {
                self.r.u32()?; // EItemType
                Ok(())
            }
            0x2E002019 => {
                acc.touched = true;
                let v = self.r.u32()?;
                if v < 3 {
                    return Ok(());
                }
                self.r.lookback()?; // defaultWeaponName
                if v < 4 {
                    return Ok(());
                }
                self.noderef()?; // PhyModelCustom
                if v < 5 {
                    return Ok(());
                }
                self.noderef()?; // VisModelCustom
                if v < 6 {
                    return Ok(());
                }
                self.r.u32()?;
                if v < 7 {
                    return Ok(());
                }
                self.r.u32()?; // defaultCam
                if v < 8 {
                    return Ok(());
                }
                let edition = self.noderef()?;
                let model = if edition == -1 { self.noderef()? } else { -1 };
                acc.entity_model = if edition >= 0 { edition } else { model };
                if v < 13 {
                    return Ok(());
                }
                self.noderef()?; // vfxFile
                if v < 15 {
                    return Ok(());
                }
                self.noderef()?; // MaterialModifier
                Ok(())
            }
            // The bridge from an item to a static object or a prefab.
            0x2E027000 => {
                acc.touched = true;
                let v = self.r.u32()?;
                acc.entity_model = self.noderef()?;
                if v < 2 {
                    return Ok(());
                }
                self.noderef()?; // triggerShape
                self.r.iso4()?; // spawnLoc
                self.noderef()?; // emitter
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    self.noderef()?; // actions
                }
                if v < 6 {
                    self.noderef()?;
                }
                for _ in 0..5 {
                    self.r.string()?;
                }
                self.r.iso4()?;
                let z = self.r.i32()?;
                if z != 0 {
                    return Err(format!("item entity model u06 = {} (expected 0)", z));
                }
                if v < 5 {
                    return Ok(());
                }
                self.r.u8()?;
                Ok(())
            }

            c => Err(format!(
                "class 0x{:08X}: chunk 0x{:08X} has no reader (add it to classes.rs)",
                class_id, c
            )),
        }
    }

    // ------------------------------------------------------------ surface

    /// One `GbxSurf`, with `xform` (a 3x3 + translation, row-major) already
    /// composed from any enclosing `Compound`.
    fn surf(&mut self, surf_version: u32, out: &mut Surface, xform: &[f32; 12]) -> R<()> {
        let ty = self.r.i32()?;
        match ty {
            // Mesh
            7 => {
                let v = self.r.u32()?;
                if v < 6 {
                    return Err(format!("surface mesh version {} (only >= 6 is understood)", v));
                }
                let verts = self.r.array(|r| r.vec3())?;
                let tris = self.r.array(|r| {
                    let f = [r.i32()?, r.i32()?, r.i32()?];
                    let phys = r.u8()?;
                    let gameplay = r.u8()?;
                    let _mat_index = r.u16()?;
                    Ok((f, phys, gameplay))
                })?;
                let verts = verts.iter().map(|v| apply(xform, *v)).collect();
                out.meshes.push(SurfMesh { verts, tris });
            }
            // Compound: children, then one Iso4 each.
            13 => {
                let n = self.r.u32()? as usize;
                // The children come first and the locs after, so the children
                // must be collected before they can be placed.
                let mut kids: Vec<Surface> = Vec::with_capacity(n);
                for _ in 0..n {
                    let mut k = Surface::default();
                    self.surf(surf_version, &mut k, &IDENTITY)?;
                    kids.push(k);
                }
                let mut locs = Vec::with_capacity(n);
                for _ in 0..n {
                    locs.push(self.r.iso4()?);
                }
                let _bones = self.r.array(|r| r.u16())?;
                for (k, loc) in kids.into_iter().zip(locs) {
                    let m = compose(xform, &loc);
                    for mut mesh in k.meshes {
                        for v in mesh.verts.iter_mut() {
                            *v = apply(&m, *v);
                        }
                        out.meshes.push(mesh);
                    }
                    out.primitives.extend(k.primitives);
                }
            }
            // ConvexPolyhedron: a hull. Kept as its vertex cloud plus faces.
            10 => {
                let _v = self.r.u32()?;
                let odd = self.r.bool32()?;
                if odd {
                    return Err("convex polyhedron with u01 = true has no reader".into());
                }
                let _aabb = self.r.boxf()?;
                let verts = self.r.array(|r| r.vec3())?;
                let face_idx = self.r.array(|r| r.i32())?;
                let faces = self.r.array(|r| Ok([r.i32()?, r.i32()?]))?;
                let _u03 = self.r.u16()?;
                // Each face is (offset, count) into face_idx; fan-triangulate.
                let mut tris = Vec::new();
                for f in &faces {
                    let (off, cnt) = (f[0] as usize, f[1] as usize);
                    if off + cnt > face_idx.len() || cnt < 3 {
                        continue;
                    }
                    for k in 1..cnt - 1 {
                        tris.push(([face_idx[off], face_idx[off + k], face_idx[off + k + 1]], 0u8, 0u8));
                    }
                }
                let verts = verts.iter().map(|v| apply(xform, *v)).collect();
                out.meshes.push(SurfMesh { verts, tris });
            }
            // Primitives: sphere, ellipsoid, box, cylinder, capsule, ...
            0 => {
                self.r.take(4 + 2)?;
                out.primitives.push(ty);
            }
            1 => {
                self.r.take(12 + 2)?;
                out.primitives.push(ty);
            }
            c => return Err(format!("surface shape type {} has no reader", c)),
        }
        if surf_version >= 2 {
            self.r.vec3()?; // GameplayMainDir
        }
        Ok(())
    }

    // -------------------------------------------------------- solid2model

    fn solid2(&mut self, out: &mut Solid2) -> R<()> {
        let version = self.r.u32()?;
        let _u01 = self.r.lookback()?;
        out.geoms = self.r.array(|r| {
            let visual = r.i32()?;
            let material = r.i32()?;
            let _u01 = r.i32()?;
            let lod = if version >= 1 { r.i32()? } else { 0 };
            if version >= 32 {
                r.i32()?;
            }
            Ok(ShadedGeom { visual, material, lod })
        })?;
        if version >= 6 {
            let lv = self.r.u32()?;
            if lv != 10 {
                return Err(format!("solid2 listVersion01 = {} (expected 10)", lv));
            }
            let n = self.r.u32()? as usize;
            for _ in 0..n {
                let v = self.noderef()?;
                out.visuals.push(v);
            }
        }
        out.material_names = self.r.array(|r| r.lookback())?;
        let material_count = if version >= 29 { self.r.u32()? } else { 0 };
        if material_count == 0 {
            let lv = self.r.u32()?;
            if lv != 10 {
                return Err(format!("solid2 listVersion02 = {} (expected 10)", lv));
            }
            let n = self.r.u32()? as usize;
            for _ in 0..n {
                self.noderef()?;
            }
        }
        self.noderef()?; // skel
        if version < 1 {
            return Ok(());
        }
        self.r.array(|r| r.f32())?; // lodDistances
        if version < 2 {
            return Ok(());
        }
        let _vis_cst = self.r.u32()?;
        if version < 3 {
            return Ok(());
        }
        let has_prelight = self.r.bool32()?;
        if has_prelight {
            let pv = self.r.u32()?;
            self.r.take(4)?; // u01
            self.r.take(4)?; // MeterByUv
            self.r.take(4)?; // u03
            self.r.take(16 * 2)?; // two GbxRect
            self.r.take(4 * 2)?; // spriteCount, u10
            self.r.array(|r| r.boxf())?;
            if pv >= 1 {
                self.r.array(|r| {
                    r.take(20)?;
                    Ok(())
                })?;
            }
        }
        if version < 4 {
            return Ok(());
        }
        self.r.take(8)?; // updatedTime
        if version < 5 {
            return Ok(());
        }
        self.r.string()?; // ImportString
        if version < 7 {
            return Ok(());
        }
        self.r.string()?; // materialFolderName
        if version >= 19 {
            self.r.string()?;
        }
        if version < 8 {
            return Ok(());
        }
        // The lights array interleaves node refs, so it cannot go through
        // `Reader::array` (which takes a closure over the reader alone).
        let n_lights = self.r.u32()? as usize;
        for _ in 0..n_lights {
            let _name = self.r.lookback()?;
            let is_node = self.r.bool32()?;
            if is_node {
                self.noderef()?;
            } else {
                self.r.string()?;
            }
            self.r.iso4()?;
            self.r.take(12)?;
            if version >= 26 {
                self.r.take(12)?;
            }
            if self.r.bool32()? {
                self.r.take(12)?;
            }
        }
        if version < 16 {
            let n = self.r.u32()? as usize;
            for _ in 0..n {
                self.noderef()?;
            }
        }
        if version < 10 {
            return Ok(());
        }
        let n = self.r.u32()? as usize;
        for _ in 0..n {
            self.noderef()?;
        }
        self.r.array(|r| {
            r.u32()?;
            r.u32()
        })?;
        if version < 11 {
            return Ok(());
        }
        self.r.i32()?; // damageZone
        if version < 12 {
            return Ok(());
        }
        self.r.u32()?; // flags
        if version < 13 {
            return Ok(());
        }
        self.r.i32()?;
        if version < 14 {
            return Ok(());
        }
        self.r.string()?; // creationCmd
        if version < 15 {
            return Ok(());
        }
        let mat_count_lt29 = if version < 29 { self.r.u32()? } else { 0 };
        if version >= 30 {
            self.r.i32()?;
        }
        let n_custom = if version >= 29 { material_count } else { mat_count_lt29 };
        for _ in 0..n_custom {
            let name = self.r.string()?;
            if name.is_empty() {
                self.noderef()?;
            }
        }
        if version < 17 {
            return Ok(());
        }
        if version < 21 {
            self.r.array(|r| r.boxf())?;
        }
        if version < 20 {
            return Ok(());
        }
        self.r.array(|r| r.lookback())?; // bonesNames
        if version < 22 {
            return Ok(());
        }
        self.r.array(|r| r.i32())?;
        if version < 23 {
            return Ok(());
        }
        let n_u18 = self.r.u32()?;
        if n_u18 != 0 {
            return Err(format!("solid2 u18 array has {} elements (only 0 is understood)", n_u18));
        }
        self.r.array(|r| r.i32())?;
        if version < 24 {
            return Ok(());
        }
        self.r.i32()?;
        if version < 25 {
            return Ok(());
        }
        self.noderef()?; // icon
        self.r.vec2()?;
        if version < 27 {
            return Ok(());
        }
        self.r.lookback()?;
        if version < 31 {
            return Ok(());
        }
        self.r.array(|r| r.take(8).map(|_| ()))?;
        if version < 33 {
            return Ok(());
        }
        if version == 33 {
            let z = self.r.u32()?;
            if z != 0 {
                return Err(format!("solid2 cst_0 = {} (expected 0)", z));
            }
        }
        self.r.array(|r| {
            r.take(20)?;
            Ok(())
        })?;
        Ok(())
    }

    // -------------------------------------------------------------- visual

    fn visual_common(&mut self, acc: &mut Acc) -> R<()> {
        let flags = VisualFlags::from_word(self.r.u32()?);
        acc.visual_flags = flags;
        let tex_coord_count = self.r.u32()?;
        if tex_coord_count >= 16 {
            return Err(format!("visual TexCoordCount = {}", tex_coord_count));
        }
        let vertex_count = self.r.u32()?;
        acc.visual.count = vertex_count;
        let n_streams = self.r.u32()? as usize;
        for _ in 0..n_streams {
            let v = self.noderef()?;
            acc.visual.vertex_streams.push(v);
        }
        for t in 0..tex_coord_count {
            let version = self.r.u32()?;
            let count = if version >= 3 { self.r.u32()? } else { vertex_count };
            let tflags = if version >= 3 { self.r.u32()? } else { 0 };
            for _ in 0..count {
                let uv = self.r.vec2()?;
                if (1..3).contains(&version) {
                    self.r.i32()?;
                }
                if version == 2 {
                    self.r.i32()?;
                }
                if t == 0 {
                    acc.visual.uv0.push(uv);
                }
            }
            if tflags != 0 {
                self.r.take(4 * count as usize * (tflags & 0xFF) as usize)?;
            }
        }
        if flags.skin_index_count != 0 {
            self.r.bool32()?;
            self.r.i32()?;
            let has_weight = self.r.bool32()?;
            self.r.bool32()?;
            if has_weight {
                self.r.take(4 * vertex_count as usize * flags.skin_index_count as usize)?;
            }
            self.r.array(|r| r.lookback())?;
            self.r.array(|r| r.i32())?;
        }
        self.r.boxf()?; // BoundingBox
        Ok(())
    }

    fn visual_inline_vertices(&mut self, acc: &mut Acc) -> R<()> {
        let f = acc.visual_flags;
        let n = acc.visual.count as usize;
        if !f.bit22 && !f.compress_float4_color && f.use_vertex_color {
            for _ in 0..n {
                let p = self.r.vec3()?;
                let nl = self.r.vec3()?;
                self.r.take(16)?;
                acc.visual.inline_positions.push(p);
                acc.visual.inline_normals.push(nl);
            }
        } else if acc.visual.vertex_streams.is_empty() {
            for _ in 0..n {
                let p = self.r.vec3()?;
                let nl = if !f.bit22 || f.use_vertex_normal {
                    if f.compress_float3_local3d {
                        dec3n(self.r.u32()?)
                    } else {
                        self.r.vec3()?
                    }
                } else {
                    [0.0, 0.0, 0.0]
                };
                if !f.bit22 || f.use_vertex_color {
                    if f.compress_float4_color {
                        self.r.u32()?;
                    } else {
                        self.r.take(16)?;
                    }
                }
                acc.visual.inline_positions.push(p);
                acc.visual.inline_normals.push(nl);
            }
        }
        let per = if f.compress_float3_local3d { 4 } else { 12 };
        let nu = self.r.u32()? as usize;
        self.r.take(per * nu)?;
        let nv = self.r.u32()? as usize;
        self.r.take(per * nv)?;
        Ok(())
    }

    // ------------------------------------------------------ vertex stream

    fn vertex_stream(&mut self, out: &mut VertexStream) -> R<()> {
        let _version = self.r.u32()?;
        let num = self.r.i32()?;
        let _flags = self.r.u32()?;
        let base = self.noderef()?;
        if num == 0 || base != -1 {
            return Ok(());
        }
        let num = num as usize;
        struct Decl {
            name: u32,
            ty: u32,
        }
        let mut decls = Vec::new();
        let n_decl = self.r.u32()? as usize;
        for _ in 0..n_decl {
            let lo = self.r.u32()? as u64;
            let hi = self.r.u32()? as u64;
            let w = lo | (hi << 32);
            let name = (w & 0x1FF) as u32;
            let ty = ((w >> 9) & 0x1FF) as u32;
            let ptr_offset = ((w >> 34) & 0x3FF) as u32;
            if ptr_offset != 0 {
                self.r.take(4)?;
            }
            decls.push(Decl { name, ty });
        }
        let compress_local3d = self.r.bool32()?;
        for d in &decls {
            // Float3 in Local3D space is stored packed when the stream says so.
            let effective = if d.ty == 2 && compress_local3d { 14 } else { d.ty };
            match (d.name, effective) {
                // Position
                (0, 2) => {
                    for _ in 0..num {
                        out.positions.push(self.r.vec3()?);
                    }
                }
                (0, 14) => {
                    for _ in 0..num {
                        out.positions.push(dec3n(self.r.u32()?));
                    }
                }
                // Normal
                (5, 2) => {
                    for _ in 0..num {
                        out.normals.push(self.r.vec3()?);
                    }
                }
                (5, 14) => {
                    for _ in 0..num {
                        out.normals.push(dec3n(self.r.u32()?));
                    }
                }
                // TexCoord0
                (10, 1) => {
                    for _ in 0..num {
                        out.uv0.push(self.r.vec2()?);
                    }
                }
                (_, t) => {
                    let sz = type_size(t)
                        .ok_or_else(|| format!("vertex declaration type {} has no size", t))?;
                    self.r.take(sz * num)?;
                }
            }
        }
        Ok(())
    }
}

fn type_size(t: u32) -> Option<usize> {
    // GbxPlugVDclTypeBytes
    const B: [usize; 17] = [4, 8, 0xC, 0x10, 4, 4, 4, 8, 4, 4, 8, 4, 8, 4, 4, 4, 8];
    B.get(t as usize).copied()
}

const IDENTITY: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

/// An `Iso4` is nine rotation floats then three translation floats, in the
/// order the game writes them: three columns of the rotation, then the offset.
fn apply(m: &[f32; 12], v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[3] * v[1] + m[6] * v[2] + m[9],
        m[1] * v[0] + m[4] * v[1] + m[7] * v[2] + m[10],
        m[2] * v[0] + m[5] * v[1] + m[8] * v[2] + m[11],
    ]
}

fn compose(outer: &[f32; 12], inner: &[f32; 12]) -> [f32; 12] {
    let mut out = [0f32; 12];
    for c in 0..3 {
        let col = [inner[c * 3], inner[c * 3 + 1], inner[c * 3 + 2]];
        let r = [
            outer[0] * col[0] + outer[3] * col[1] + outer[6] * col[2],
            outer[1] * col[0] + outer[4] * col[1] + outer[7] * col[2],
            outer[2] * col[0] + outer[5] * col[1] + outer[8] * col[2],
        ];
        out[c * 3] = r[0];
        out[c * 3 + 1] = r[1];
        out[c * 3 + 2] = r[2];
    }
    let t = apply(outer, [inner[9], inner[10], inner[11]]);
    out[9] = t[0];
    out[10] = t[1];
    out[11] = t[2];
    out
}

fn known(_class_id: u32, cid: u32) -> bool {
    matches!(
        cid,
        0x0900C003
            | 0x090BB000
            | 0x09006001
            | 0x09006004
            | 0x09006005
            | 0x09006009
            | 0x0900600B
            | 0x0900600D
            | 0x0900600E
            | 0x0900600F
            | 0x09006010
            | 0x0902C002
            | 0x0902C004
            | 0x0906A000
            | 0x0906A001
            | 0x09056000
            | 0x2E001009
            | 0x2E00100B
            | 0x2E00100C
            | 0x2E00100D
            | 0x2E00100E
            | 0x2E001010
            | 0x2E001011
            | 0x2E001012
            | 0x2E002008
            | 0x2E002009
            | 0x2E00200C
            | 0x2E002012
            | 0x2E002013
            | 0x2E002015
            | 0x2E002019
            | 0x2E00201A
            | 0x2E00201C
            | 0x2E00201E
            | 0x2E00201F
            | 0x2E002020
            | 0x2E002021
            | 0x2E002023
            | 0x2E027000
    )
}
