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
    let s = if v >= 0x200 {
        v as f32 - 1024.0
    } else {
        v as f32
    };
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
                    self.r.mark(3);
                    let pos = self.r.vec3().map_err(ctx)?;
                    crate::reader::trace(|| {
                        format!(
                            "    params at 0x{:x} id 0x{:08X}",
                            self.r.o,
                            self.r.peek_u32().unwrap_or(0)
                        )
                    });
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
                Ok(Node::StaticObject(StaticObject {
                    mesh,
                    mesh_collidable,
                    shape,
                }))
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
            // NPlugTrigger_SWaypoint: the volume a checkpoint or finish gate
            // FIRES on. It carries a trigger shape and nothing the car can
            // rest on, so no geometry comes out of it — but it has to be
            // walked, because it sits in the middle of the gate prefabs and
            // everything after it in the file is unreachable until it is.
            // Read off the bytes of `Items\Gate\CheckpointRight32m.Prefab.Gbx`:
            // sixteen bytes, of which the second word is a reference to a
            // shape that already exists.
            0x09178000 => {
                let _version = self.r.u32()?;
                self.noderef()?; // TriggerShape
                self.r.take(8)?;
                Ok(Node::Other(class_id))
            }
            // Two more trigger-side classes in the gate prefabs, both eight
            // and sixteen bytes of metadata with no geometry. Same file, same
            // method: the entity after them has an identity quaternion and a
            // -1 parameter chunk, which pins where they end.
            0x0917B000 => {
                self.r.take(8)?;
                Ok(Node::Other(class_id))
            }
            // 0x09179000 (the special/expandable gate trigger): version 2,
            // a TriggerShape reference, one word; the entity quaternion follows.
            0x09179000 => {
                let _version = self.r.u32()?;
                self.noderef()?;
                self.r.take(4)?;
                Ok(Node::Other(class_id))
            }
            // `CPlugDynaObjectModel`: a block that MOVES. Eighty-three bytes
            // at version 13, identical in every one in the pack (rotor, tube,
            // turnstile, flag, light ray), read off `ObstacleTube6m` and
            // confirmed against the other three — the rotor is the file that
            // names its two shapes, `MoveShape` at the first reference and
            // `HitShape` at the second, which is what fixes their order.
            //
            // The member list is in the game's own class reference
            // (`next.openplanet.dev/Plug/CPlugDynaObjectModel`); the byte
            // layout is not, and this is it:
            //
            // ```
            //  0 version = 13      0x18 f32 BreakSpeedKmh     0x36 u32
            //  4 u32 IsStatic      0x1c f32 Mass              0x3a u32
            //  8 u32 DynamizeOnSpawn 0x20 f32 LightAlive_Min  0x3e u32
            //  c ref Mesh          0x24 f32 LightAlive_Max    0x42 u8
            // 10 ref DynaShape     0x28 u32                   0x43 ref LocAnim
            // 14 ref StaticShape   0x2c u32                   0x47 u32
            //                      0x30 u8, 0x31 u8           0x4b u32
            //                      0x32 u32                   0x4f ref WaterModel
            // ```
            C_DYNA_OBJECT => {
                let _version = self.r.u32()?;
                let _is_static = self.r.u32()?;
                let _dynamize_on_spawn = self.r.u32()?;
                let mesh = self.noderef()?;
                let dyna_shape = self.noderef()?;
                let static_shape = self.noderef()?;
                self.r.take(4 * 6)?; // break speed, mass, two light durations, two words
                self.r.take(2)?;
                self.r.take(4 * 4)?;
                self.r.take(1)?;
                self.noderef()?; // LocAnim
                self.r.take(8)?;
                self.noderef()?; // WaterModel
                Ok(Node::Dyna(DynaObject {
                    mesh,
                    dyna_shape,
                    static_shape,
                }))
            }
            // NPlugDyna_SConstraintModel: a spring, no geometry.
            0x2F074000 => {
                self.r.take(4 * 5)?;
                Ok(Node::Other(class_id))
            }
            // The remaining body-less classes are placement metadata we do not
            // read; reaching one means the walk went somewhere unexpected.
            c => Err(format!(
                "class 0x{:08X} has no chunk framing and no reader",
                c
            )),
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
                self.r
                    .array(|r| r.array(|r| Ok((r.string()?, r.string()?))))?;
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
                    r.mark(3);
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
            c => Err(format!(
                "prefab entity params chunk 0x{:08X} has no reader",
                c
            )),
        }
    }

    /// One chunk of one node.
    pub fn chunk(&mut self, class_id: u32, cid: u32, acc: &mut Acc) -> R<()> {
        // The block-info families live in blockinfo.rs; anything it knows it
        // reads, anything else falls through to the table below.
        if let Some(res) = self.bi_chunk(class_id, cid) {
            return res;
        }
        match cid {
            // ---------------------------------------------- CPlugSurface
            0x0900C003 => {
                acc.touched = true;
                let version = self.r.u32()?;
                let surf_version = if version < 2 { 0 } else { self.r.u32()? };
                self.surf(
                    surf_version,
                    &mut acc.surface,
                    &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                )?;
                let n_mats = self.r.u32()? as usize;
                for _ in 0..n_mats {
                    let has = self.r.bool32()?;
                    if has {
                        let m = self.noderef()?;
                        acc.surface.materials.push(m);
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

            // ------------------------------------------ CPlugSkel (0x090BA000)
            // A Solid2's skeleton (Stadium Grass\Base, DecoWallToGrass\*: four
            // "Fences_*" joints the fence pieces hang from). Version 20:
            // version, name Id, u16 joint count, per joint {name Id, i16
            // parent, Iso4}; the 33-byte tail after the joints (counts and
            // flags, all zero-length here) is not decoded — the walk
            // recovers to the node terminator, reported. A static item
            // carries no skeleton, so nothing of it is needed downstream.
            0x090BA000 => {
                let v = self.r.u32()?;
                if v != 20 {
                    return Err(format!("CPlugSkel version {v} (only 20 is read)"));
                }
                self.r.lookback()?; // name
                let n = self.r.u16()? as usize;
                for _ in 0..n {
                    self.r.lookback()?;
                    self.r.u16()?;
                    self.r.iso4()?;
                }
                self.recover_to_facade("CPlugSkel tail")
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
                    r.mark(6);
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
                    return Err(format!(
                        "visual morph_count {} (only 0 is understood)",
                        morph
                    ));
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
                            c => {
                                return Err(format!("index buffer chunk 0x{:08X} has no reader", c))
                            }
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

            // ------------------------------ CGameCtnBlockInfo
            // NoRespawn, one 32-bit boolean. Present on the current 2026
            // client/server block records; it carries no geometry itself.
            0x0304E00F | 0x0304E013 | 0x0304E017 => {
                self.r.bool32()?;
                Ok(())
            }
            // CGameCtnBlockInfo: waypoint/podium metadata. The current
            // MP4 chunk is version 8; all references are part of the node
            // graph but carry no geometry themselves.
            0x0304E020 => {
                let v = self.r.u32()?;
                self.noderef()?; // CharPhySpecialProperty
                if v < 6 {
                    self.noderef()?; // legacy WaypointSpecialProperty
                }
                if v >= 2 {
                    self.noderef()?; // PodiumInfo
                }
                if v >= 3 {
                    self.noderef()?; // IntroInfo
                }
                if v >= 4 {
                    self.r.bool32()?; // CharPhySpecialPropertyCustomizable
                }
                if v == 5 {
                    self.r.bool32()?;
                }
                if v >= 8 {
                    let has_modifier = self.r.bool32()?;
                    if has_modifier {
                        self.r.string()?;
                        self.r.string()?;
                    }
                }
                Ok(())
            }

            // ------------------------------ CPlugSolid (inline, as a block
            // variant's trigger solid) and its CPlugTree. Only what the block
            // info files carry: GateSpecialBoost has the whole set.
            0x09005000 => {
                self.r.i32()?; // TypeAndIndex
                Ok(())
            }
            0x09005010 => {
                self.noderef()?;
                Ok(())
            }
            0x09005011 => {
                self.r.bool32()?;
                if self.r.bool32()? {
                    self.r.bool32()?;
                }
                acc.touched = true;
                acc.entity_model = self.noderef()?; // Tree
                Ok(())
            }
            0x09005017 => {
                let v = self.r.u32()?;
                if v >= 3 {
                    if self.r.bool32()? {
                        // PreLightGen: version, int, float, bool, 8 floats,
                        // int2 sprite count, box[], v1+ uvgroup[].
                        let pv = self.r.u32()?;
                        self.r.take(4 + 4 + 4 + 8 * 4 + 8)?;
                        self.r.array(|r| r.take(24).map(|_| ()))?;
                        if pv >= 1 {
                            self.r.array(|r| r.take(20).map(|_| ()))?;
                        }
                    }
                } else {
                    self.r.take(1 + 4 + 4 + 16 + 16 + 8)?;
                    if v >= 1 {
                        self.r.array(|r| r.take(24).map(|_| ()))?;
                    }
                }
                if v >= 2 {
                    self.r.take(8)?; // FileWriteTime (GateSpecialBoost: present at v3)
                }
                Ok(())
            }
            0x09005019 => {
                let v = self.r.u32()?;
                for _ in 0..2 {
                    let lv = self.r.u32()?;
                    if lv != 10 {
                        return Err(format!("CPlugSolid 019 list version {} (expected 10)", lv));
                    }
                    let n = self.r.u32()? as usize;
                    for _ in 0..n {
                        self.noderef()?;
                    }
                }
                for _ in 0..2 {
                    self.r.array(|r| r.take(4 + 48).map(|_| ()))?; // LocatedInstance
                }
                if v >= 1 {
                    self.r.i32()?;
                }
                if v >= 2 {
                    self.r.array(|r| r.lookback())?;
                    self.r.array(|r| r.iso4())?;
                }
                if v >= 3 {
                    self.r.string()?;
                }
                // GBX.NET stops at v3. The BlueBay GateSpecial* files are v5
                // and carry eight more bytes here (0x00019312, 0xFFFFFFFF on
                // every one — looks like a word and a null node ref) right
                // before chunk 0x0900501A.
                if v >= 4 {
                    self.r.take(8)?;
                }
                Ok(())
            }
            0x0904F006 => {
                let lv = self.r.u32()?;
                if lv != 10 {
                    return Err(format!("CPlugTree children list version {} (expected 10)", lv));
                }
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    self.noderef()?;
                }
                Ok(())
            }
            0x0904F00D => {
                self.r.lookback()?;
                self.r.lookback()?;
                Ok(())
            }
            0x0904F011 | 0x0904F017 => {
                self.noderef()?;
                Ok(())
            }
            0x0904F016 => {
                self.noderef()?; // Visual
                self.noderef()?; // Shader
                self.noderef()?; // Surface
                self.noderef()?; // Generator
                Ok(())
            }
            0x0904F01A => {
                let flags = self.r.u32()?;
                if flags & 4 != 0 {
                    self.r.iso4()?;
                }
                Ok(())
            }
            // CPlugMediaClipList / CGamePodiumInfo: version, external clips.
            0x09189000 | 0x03168000 => {
                let _v = self.r.u32()?;
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    self.noderef()?;
                }
                Ok(())
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
            0x2E00100C => {
                let name = self.r.string()?;
                if self.bi_stack.len() <= 1 {
                    self.collector_name = name;
                }
                Ok(())
            }
            0x2E00100D => {
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
                    return Err(format!(
                        "item defaultPlacement chunk version {} (expected 5)",
                        v
                    ));
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
            // CGameCommonItemEntityModelEdition: the mesh-modeler item, whose
            // geometry is the MeshCrystal. Layout per GBX.NET's chunkl; the
            // inventory strings and the trailing nodes are read so the walk
            // (and its node-reference sites) stays exact to the FACADE.
            0x2E026000 => {
                acc.touched = true;
                let v = self.r.u32()?;
                let item_type = self.r.u32()?;
                acc.entity_model = self.noderef()?; // MeshCrystal
                self.r.string()?; // U01
                self.noderef()?; // U02 CPlugSolid
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    self.noderef()?; // U03 CPlugFileImg
                }
                let n = self.r.u32()? as usize;
                self.r.take(n * (12 + 4 + 4))?; // SpriteParams
                self.noderef()?; // U04 particle emitter
                self.noderef()?; // U05 anim loc
                let n = self.r.u32()? as usize;
                self.r.take(n * 32)?; // LightBallStateSimple: int, 7 floats
                self.r.take(7 * 4)?; // U07..U13
                self.r.iso4()?; // U14
                if v >= 3 && item_type == 2 {
                    self.r.f32()?; // Mass (PickUp)
                }
                let u15 = self.r.bool32()?;
                if !u15 {
                    self.noderef()?; // U16 CPlugCrystal
                }
                if item_type != 1 {
                    return Err(format!("item entity model edition type {} (only Ornament is read)", item_type));
                }
                if self.r.bool32()? {
                    self.r.i32()?; // U18
                    self.r.iso4()?; // U19
                }
                self.r.i32()?; // U20
                if v >= 1 {
                    self.r.string()?; // InventoryName
                    self.r.string()?; // InventoryDescription
                    self.r.i32()?; // InventoryItemClass
                    self.r.i32()?; // InventoryOccupation
                    if v >= 6 {
                        if v <= 7 {
                            self.noderef()?; // U21
                        }
                        if v >= 7 && item_type == 2 {
                            self.r.bool32()?; // U22
                        }
                    }
                }
                Ok(())
            }
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

            // CGameBlockItem: a custom BLOCK a map embeds. Its geometry hangs
            // off one node per editor variant; the first is taken, as with
            // NPlugItem_SVariantList.
            0x2E025000 => {
                acc.touched = true;
                let version = self.r.u32()?;
                self.r.lookback()?; // ArchetypeBlockInfoId
                self.r.lookback()?; // ArchetypeBlockInfoCollectionId
                let n = self.r.u32()? as usize;
                crate::reader::trace(|| {
                    format!(
                        "    blockitem v{} {} variants at 0x{:x}",
                        version, n, self.r.o
                    )
                });
                let mut first = -1;
                for i in 0..n {
                    crate::reader::trace(|| {
                        format!(
                            "      variant {} at 0x{:x} next u32 0x{:08X}",
                            i,
                            self.r.o,
                            self.r.peek_u32().unwrap_or(0)
                        )
                    });
                    self.r.u32()?; // variant key
                    let v = self.noderef()?;
                    crate::reader::trace(|| format!("      -> node {} at 0x{:x}", v, self.r.o));
                    if i == 0 {
                        first = v;
                    }
                }
                acc.entity_model = first;
                if version < 1 {
                    return Ok(());
                }
                // Version 1 adds a second table, one entry per variant. It
                // exists because a v1 block hands out a NULL node in the
                // variant list above and puts its geometry here instead --
                // 210218's whole track is 83 embedded wood platforms that all
                // look like this, and reading the entry as a 32-bit flag word
                // (there was no v1 file to check against when this was
                // written) walked straight off into a garbage node reference
                // and cost the map every one of them.
                //
                // Read off the bytes of `PlatformWoodBase.Block.Gbx`: a byte
                // saying the table is present, then per variant a byte of
                // flags, then whichever of mesh / collision / box / offset
                // that byte claims.
                if self.r.u8()? != 0 {
                    for _ in 0..n {
                        let flags = self.r.u8()?;
                        if flags & 1 != 0 {
                            let mesh = self.noderef()?;
                            // A v1 block's variant list hands out null; this
                            // is where its shape actually is.
                            if acc.entity_model < 0 {
                                acc.entity_model = mesh;
                            }
                        }
                        if flags & 2 != 0 {
                            let surf = self.noderef()?; // collision surface
                            if acc.entity_model < 0 {
                                acc.entity_model = surf;
                            }
                        }
                        if flags & 4 != 0 {
                            self.r.boxf()?;
                        }
                        if flags & 8 != 0 {
                            self.r.vec3()?;
                        }
                    }
                }
                Ok(())
            }

            // -------------------------------------------- CPlugCrystal
            // The editor's editable mesh. This is what a map's EMBEDDED custom
            // items and blocks are made of, and it is the only place in this
            // pipeline where a real authored mesh — vertices and n-gon faces
            // with a material per face — turns up rather than a pack asset.
            0x09003003 => {
                acc.touched = true;
                let _v = self.r.u32()?;
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    let name = self.r.string()?;
                    // An empty name means the material is a node: a
                    // CPlugMaterialUserInst, which carries the physics id.
                    let node = if name.is_empty() { self.noderef()? } else { -1 };
                    acc.crystal_materials.push((name, node));
                }
                Ok(())
            }
            0x09003004 => {
                let _v = self.r.u32()?;
                let n = self.r.u32()? as usize;
                self.r.take(n)?;
                self.r.take(4)?;
                Ok(())
            }
            0x09003005 => {
                acc.touched = true;
                // The complete layer model lives in `crystal_model`; this walk
                // hands it the reader's state (lookback table, defined nodes)
                // and takes the meshes of the drivable geometry layers back.
                let mut defined = std::collections::HashSet::new();
                for (i, s) in self.slots.iter().enumerate() {
                    if !matches!(s, Slot::Unset) {
                        defined.insert(i as u32);
                    }
                }
                let lb = crate::crystal_model::LookbackState {
                    table: self.r.lb.clone(),
                    version_seen: self.r.lb_version_seen(),
                    defined_nodes: defined,
                };
                let mut rd = crate::crystal_model::Rd::new(self.r.b, self.r.o, lb);
                let (_version, layers) = crate::crystal_model::read_layers_chunk(&mut rd, acc.crystal_materials.len())?;
                self.r.o = rd.o;
                self.r.lb = rd.lb.table;
                self.r.set_lb_version_seen(rd.lb.version_seen);
                for i in rd.lb.defined_nodes {
                    if let Some(s) = self.slots.get_mut(i as usize) {
                        if matches!(s, Slot::Unset) {
                            *s = Slot::Node(Node::Other(0));
                        }
                    }
                }
                for l in &layers {
                    if let crate::crystal_model::LayerKind::Geometry { crystal, is_visible, collidable, .. } = &l.kind {
                        // A trigger layer is a volume, not a surface; keep
                        // it out of the drivable geometry.
                        if *is_visible || *collidable {
                            acc.crystals.push(CrystalMesh {
                                verts: crystal.positions.clone(),
                                faces: crystal
                                    .faces
                                    .iter()
                                    .map(|f| (f.verts.iter().map(|v| *v as i32).collect(), f.material.max(0) as usize))
                                    .collect(),
                            });
                        }
                    }
                }
                Ok(())
            }
            0x09003006 => {
                let v = self.r.u32()?;
                if v == 0 {
                    self.r.array(|r| r.vec2())?;
                }
                if v < 1 {
                    return Ok(());
                }
                self.r.array(|r| r.take(4).map(|_| ()))?;
                if v < 2 {
                    return Ok(());
                }
                let n = self.r.u32()? as usize;
                self.r.take(opt_int_size(n) * n)?;
                Ok(())
            }
            0x09003007 => {
                let _v = self.r.u32()?;
                self.r.array(|r| r.f32())?;
                self.r.array(|r| r.i32())?;
                Ok(())
            }
            // CPlugTreeGenerator, inherited by CPlugCrystal.
            0x09051000 => {
                self.r.u32()?;
                Ok(())
            }

            // -------------------------------- CPlugMaterialUserInst
            // A custom item's material — and, crucially, its `surfacePhysicId`,
            // the same enum a stock block's collision triangles carry. So a
            // custom mesh can be coloured by what the car FEELS, exactly like a
            // stock one, instead of by an artist's material name.
            0x090FD000 => {
                acc.touched = true;
                let v = self.r.u32()?;
                let using_game_material = if v >= 11 { self.r.u8()? != 0 } else { false };
                let name = self.r.lookback()?;
                self.r.lookback()?; // model
                self.r.string()?; // baseTexture
                acc.physics_id = self.r.u8()?;
                if v >= 10 {
                    self.r.u8()?; // surfaceGameplayId
                }
                acc.material_name = name;
                if v < 1 {
                    return Ok(());
                }
                let link = if (9..11).contains(&v) || using_game_material {
                    self.r.string()?
                } else {
                    self.r.lookback()?
                };
                if !link.is_empty() {
                    acc.material_name = link;
                }
                if v < 2 {
                    return Ok(());
                }
                self.r.array(|r| {
                    r.lookback()?;
                    r.lookback()?;
                    r.i32()
                })?;
                self.r.array(|r| r.i32())?;
                if v < 3 {
                    return Ok(());
                }
                self.r.array(|r| {
                    r.lookback()?;
                    r.lookback()?;
                    r.f32()?;
                    r.take(8)?;
                    if v >= 5 {
                        r.lookback()?;
                    }
                    Ok(())
                })?;
                if v < 4 {
                    return Ok(());
                }
                self.r.array(|r| r.lookback())?;
                if v < 6 {
                    return Ok(());
                }
                self.r.array(|r| {
                    r.i32()?;
                    r.string()
                })?;
                if v < 7 {
                    return Ok(());
                }
                self.r.lookback()?; // hidingGroup
                Ok(())
            }
            0x090FD001 => {
                let v = self.r.u32()?;
                self.noderef()?;
                self.r.take(4 * 3)?; // tilingU, tilingV, textureSize
                if v < 4 {
                    return Ok(());
                }
                self.r.i32()?;
                if v < 5 {
                    return Ok(());
                }
                self.r.bool32()?;
                Ok(())
            }
            0x090FD002 => {
                self.r.take(8)?;
                Ok(())
            }

            // ------------------------------------ CPlugLight (0x0901D000)
            // The wrapper a Solid2 `lights` socket names (`.Light.Gbx`):
            // its GxLight, an optional animation, and (0x002) the NightOnly /
            // ReflectByGround flags. Layouts: GBX.NET CPlugLight.chunkl.
            // CPlugLightUserModel (0x090F9000): the item editor's light —
            // version, kind, colour, intensity, distance, point emission
            // radius/length, spot inner/outer angle, spot emission size x/y,
            // v1+ NightOnly (GBX.NET CPlugLightUserModel.chunkl).
            0x090F9000 => {
                let v = self.r.u32()?;
                let l = acc.light_mut();
                let _kind = self.r.i32()?;
                l.color = self.r.vec3()?;
                l.intensity = self.r.f32()?;
                l.radius = self.r.f32()?;
                l.emitting_radius = self.r.f32()?;
                l.emitting_cylinder_len_z = self.r.f32()?;
                l.angle_inner = self.r.f32()?;
                l.angle_outer = self.r.f32()?;
                self.r.f32()?;
                self.r.f32()?;
                if v >= 1 {
                    l.flags = if self.r.bool32()? { 1 } else { 0 };
                }
                Ok(())
            }
            0x0901D000 | 0x0901D002 => {
                let gx = self.noderef()?;
                let _func_light = self.noderef()?;
                let _bitmap_flare = self.noderef()?;
                let _bitmap_projector = self.noderef()?;
                let l = acc.light_mut();
                l.gx_node = gx;
                if cid == 0x0901D002 {
                    l.flags = self.r.u32()?;
                }
                Ok(())
            }
            0x0901D003 => {
                let v = self.r.u32()?;
                let image_anim = self.noderef()?;
                let a = self.r.f32()?;
                let b = self.r.f32()?;
                if v >= 1 {
                    self.r.lookback()?;
                }
                let l = acc.light_mut();
                l.image_anim = image_anim;
                l.anim_period = [a, b];
                Ok(())
            }
            0x0901D004 => {
                let _v = self.r.u32()?;
                let gx = self.noderef()?;
                let mut tail = [0i32; 5];
                for t in tail.iter_mut() {
                    *t = self.r.i32()?;
                }
                let l = acc.light_mut();
                l.gx_node = gx;
                l.tail = tail;
                Ok(())
            }
            // GxLight (0x04001000) and its subclasses, inline in a CPlugLight.
            0x04001008 => {
                let l = acc.light_mut();
                l.color = self.r.vec3()?;
                l.intensity = self.r.f32()?;
                l.gx_flags = self.r.u32()?;
                l.shadow_intensity = self.r.f32()?;
                l.flare_intensity = self.r.f32()?;
                l.shadow_rgb = self.r.vec3()?;
                Ok(())
            }
            0x04001009 => {
                let l = acc.light_mut();
                l.color = self.r.vec3()?;
                l.gx_flags = self.r.u32()?;
                l.intensity = self.r.f32()?;
                l.diffuse_intensity = self.r.f32()?;
                let _specular_intens = self.r.f32()?;
                let _specular_power = self.r.f32()?;
                l.shadow_intensity = self.r.f32()?;
                l.flare_intensity = self.r.f32()?;
                l.shadow_rgb = self.r.vec3()?;
                Ok(())
            }
            0x0400100A => {
                let _v = self.r.u32()?;
                let l = acc.light_mut();
                l.color = self.r.vec3()?;
                l.gx_flags = self.r.u32()?;
                l.intensity = self.r.f32()?;
                l.diffuse_intensity = self.r.f32()?;
                l.shadow_intensity = self.r.f32()?;
                l.flare_intensity = self.r.f32()?;
                l.shadow_rgb = self.r.vec3()?;
                Ok(())
            }
            // GxLightAmbient: ShadeMinY, ShadeMaxY
            0x04005000 => {
                acc.light_mut();
                self.r.take(8)?;
                Ok(())
            }
            // GxLightPoint: FlareSize [, FlareBiasZ]
            0x04003003 | 0x04003004 => {
                let l = acc.light_mut();
                l.flare_size = self.r.f32()?;
                if cid == 0x04003004 {
                    l.flare_bias_z = self.r.f32()?;
                }
                Ok(())
            }
            // GxLightBall
            0x04002002 => {
                let l = acc.light_mut();
                l.radius = self.r.f32()?;
                l.att_htnlr = [self.r.f32()?, self.r.f32()?];
                l.emitting_radius = self.r.f32()?;
                l.ambient_rgb = self.r.vec3()?;
                Ok(())
            }
            0x04002006 => {
                let l = acc.light_mut();
                l.ball_flags = self.r.u32()?;
                l.radius = self.r.f32()?;
                l.radius_specular = self.r.f32()?;
                l.radius_shadow = self.r.f32()?;
                l.radius_flare = self.r.f32()?;
                l.emitting_radius = self.r.f32()?;
                l.att_htnlr = [self.r.f32()?, self.r.f32()?];
                l.ambient_rgb = self.r.vec3()?;
                Ok(())
            }
            0x04002008 => {
                let l = acc.light_mut();
                l.ball_flags = self.r.u32()?;
                l.radius = self.r.f32()?;
                l.radius_specular = self.r.f32()?;
                l.radius_shadow = self.r.f32()?;
                l.radius_flare = self.r.f32()?;
                l.emitting_radius = self.r.f32()?;
                l.emitting_cylinder_len_z = self.r.f32()?;
                l.att_htnlr = [self.r.f32()?, self.r.f32()?];
                l.ambient_rgb = self.r.vec3()?;
                l.att_hyper2 = [self.r.f32()?, self.r.f32()?];
                Ok(())
            }
            0x04002009 => {
                acc.light_mut().ball_u09 = self.r.f32()?;
                Ok(())
            }
            0x0400200A => {
                acc.light_mut().ball_u0a = self.r.f32()?;
                Ok(())
            }
            // GxLightFrustum
            0x0400A004 => {
                acc.light_mut();
                self.r.take(32)?;
                Ok(())
            }
            0x0400A006 => {
                acc.light_mut();
                self.r.take(4 + 24 + 4)?;
                Ok(())
            }
            // GxLightSpot
            0x0400B001 => {
                let l = acc.light_mut();
                l.angle_inner = self.r.f32()?;
                l.angle_outer = self.r.f32()?;
                l.angle_flare = self.r.f32()?;
                l.falloff_exponent = self.r.f32()?;
                Ok(())
            }
            0x0400B002 | 0x0400B003 => {
                let v = if cid == 0x0400B003 { self.r.u32()? } else { 0 };
                let l = acc.light_mut();
                l.spot_flags = self.r.u32()?;
                l.angle_inner = self.r.f32()?;
                l.angle_outer = self.r.f32()?;
                l.angle_flare = self.r.f32()?;
                l.angle_inner_shadow = self.r.f32()?;
                l.angle_outer_shadow = self.r.f32()?;
                l.falloff_exponent = self.r.f32()?;
                if cid == 0x0400B003 {
                    if v >= 1 {
                        l.spot_bytes = [self.r.u8()?, self.r.u8()?];
                    } else {
                        self.r.i32()?;
                    }
                }
                Ok(())
            }
            // GxLightDirectional
            0x04007001 => {
                acc.light_mut();
                self.r.take(16)?;
                Ok(())
            }
            0x04007002 => {
                acc.light_mut();
                self.r.take(24)?;
                Ok(())
            }
            0x04007003 => {
                acc.light_mut();
                self.r.take(12)?;
                Ok(())
            }
            0x04007004 => {
                acc.light_mut();
                self.r.take(16)?;
                Ok(())
            }
            0x04007005 => {
                acc.light_mut();
                self.r.take(8)?;
                Ok(())
            }

            // CPlugSpawnModel: where the car appears on a start/checkpoint
            // gate. The location's translation is geometry; the gravity
            // vector is a direction.
            0x0917A000 => {
                let _v = self.r.u32()?;
                self.r.marks.push((self.r.o + 36, 3));
                self.r.iso4()?;
                self.r.take(4 + 4 + 12 + 4)?;
                Ok(())
            }

            // ------------------------------- CPlugPath / CPlugPolyLine3
            // A strip of points along a border (the turbo road's light line).
            // Positions are geometry and are marked; `Lefts` are directions.
            0x09119000 => {
                let v = self.r.u32()?;
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    self.noderef()?;
                }
                if v >= 2 {
                    self.r.bool32()?;
                    self.r.u8()?;
                    self.r.bytes_pfx()?;
                }
                Ok(())
            }
            0x09118000 => {
                let v = self.r.u32()?;
                self.r.array(|r| {
                    r.mark(3);
                    r.vec3()
                })?;
                if v >= 2 {
                    self.r.array(|r| r.vec3())?;
                }
                if v == 3 {
                    self.r.bool32()?;
                    self.r.i32()?;
                }
                if v >= 4 {
                    if v == 4 {
                        self.r.bool32()?;
                    }
                    self.r.bool32()?;
                    self.r.bool32()?;
                    if v >= 5 {
                        self.r.bool32()?;
                    }
                    if v >= 6 {
                        self.r.i32()?;
                    }
                    if v >= 7 {
                        self.r.u8()?;
                    }
                    if v >= 8 {
                        self.r.u8()?;
                        self.r.lookback()?;
                    }
                }
                Ok(())
            }

            // ------------------------------------------- CPlugMaterial
            // Met inline in the client BlueBay prefabs (`Zone\Land\Base`): a
            // material with its own CPlugMaterialCustom nested, everything
            // else external. Layouts follow GBX.NET's CPlugMaterial.chunkl and
            // were checked against the bytes of that file.
            0x09079001 | 0x09079007 => {
                let n = self.noderef()?;
                acc.touched = true;
                acc.material_refs.push(n);
                Ok(())
            }
            0x09079002 | 0x0907900A | 0x0907900F => {
                self.r.u32()?;
                Ok(())
            }
            0x09079004 => {
                let refs = self.device_materials(4)?;
                acc.material_refs.extend(refs);
                Ok(())
            }
            0x09079009 => {
                let shader = self.noderef()?;
                acc.material_refs.push(shader);
                if shader == -1 {
                    let refs = self.device_materials(9)?;
                    acc.material_refs.extend(refs);
                }
                Ok(())
            }
            0x0907900D => {
                let shader = self.noderef()?;
                acc.material_refs.push(shader);
                if shader == -1 {
                    let refs = self.device_materials(0xD)?;
                    acc.material_refs.extend(refs);
                    // Per-device material nodes: the external .Material.Gbx a
                    // BlueBay terrain material stands for is listed here.
                    let more = self.r.array(|r| r.i32())?;
                    acc.material_refs.extend(more);
                }
                Ok(())
            }
            0x0907900E => {
                // SurfaceId (the physics the car feels), U01: two shorts.
                acc.touched = true;
                acc.physics_id = self.r.u16()? as u8;
                self.r.u16()?;
                Ok(())
            }
            0x09079010 => {
                self.r.f32()?;
                Ok(())
            }
            0x09079011 => {
                self.r.array(|r| r.lookback())?;
                Ok(())
            }
            0x09079015 => {
                let v = self.r.u32()?;
                let shader = self.noderef()?;
                acc.touched = true;
                acc.material_refs.push(shader);
                if shader == -1 {
                    let refs = self.device_materials(0x15)?;
                    acc.material_refs.extend(refs);
                    self.r.array(|r| r.i32())?;
                    if v >= 3 {
                        self.r.i32()?;
                    }
                } else {
                    let n = self.r.u32()? as usize;
                    for _ in 0..n {
                        self.noderef()?; // CPlugMaterialColorTargetTable
                    }
                    if v >= 7 {
                        self.noderef()?;
                    }
                }
                Ok(())
            }
            0x09079016 => {
                self.r.take(8)?; // version, uint
                Ok(())
            }
            0x09079017 => {
                let v = self.r.u32()?;
                self.r.u32()?;
                if v >= 1 {
                    self.r.take(8)?;
                    self.r.string()?;
                }
                Ok(())
            }

            // ------------------------------------- CPlugMaterialCustom
            0x0903A004 => {
                self.r.array(|r| r.i32())?;
                Ok(())
            }
            0x0903A006 => {
                self.material_bitmaps(0)
            }
            0x0903A00A => {
                for _ in 0..2 {
                    let n = self.r.u32()? as usize;
                    for _ in 0..n {
                        // GpuFx: id, count1, count2, bool, count2 x count1 floats
                        self.r.lookback()?;
                        let c1 = self.r.u32()? as usize;
                        let c2 = self.r.u32()? as usize;
                        self.r.bool32()?;
                        self.r.take(4 * c1 * c2)?;
                    }
                }
                Ok(())
            }
            0x0903A00B => {
                let u01 = self.r.u32()?;
                self.r.take(8)?;
                if u01 & 1 != 0 {
                    self.r.take(4)?;
                }
                Ok(())
            }
            0x0903A00C => {
                self.r.array(|r| {
                    r.lookback()?;
                    r.bool32()
                })?;
                Ok(())
            }
            // Skippable, but it DEFINES lookback ids (`cIndexPerVertex`,
            // `VertexAlpha` on BlueBay's Land material) that the bitmap list
            // two chunks later refers to by index. Skipping it by size, as the
            // walk does with every other unknown skippable chunk, desyncs the
            // id table and the next material's names come out one too far.
            0x0903A00F => {
                let v = self.r.u32()?;
                self.r.take(8)?;
                if v >= 1 {
                    self.r.take(4)?;
                }
                if v >= 2 {
                    self.r.array(|r| {
                        r.lookback()?;
                        r.i32()
                    })?;
                }
                Ok(())
            }
            0x0903A00D | 0x0903A016 => {
                let v = if cid == 0x0903A016 { self.r.u32()? } else { 0 };
                let u01 = self.r.u32()?;
                self.r.take(4 + 8)?;
                if cid == 0x0903A016 && v >= 1 {
                    self.r.i32()?;
                }
                if u01 & 1 != 0 {
                    self.r.take(4)?;
                }
                Ok(())
            }
            0x0903A010 | 0x0903A012 => {
                self.noderef()?;
                Ok(())
            }
            0x0903A013 => {
                let _v = self.r.u32()?;
                self.material_bitmaps(1)
            }
            0x0903A014 => {
                let _v = self.r.u32()?;
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    self.r.i32()?;
                    self.r.bytes_pfx()?;
                }
                Ok(())
            }
            0x0903A015 => {
                let v = self.r.u32()?;
                let u01 = if v >= 1 { self.r.i32()? } else { 0 };
                if u01 == 0 {
                    self.r.string()?;
                    self.r.string()?;
                    if v >= 2 {
                        self.r.string()?;
                        self.r.string()?;
                    }
                }
                Ok(())
            }

            c => Err(format!(
                "class 0x{:08X}: chunk 0x{:08X} has no reader (add it to classes.rs)",
                class_id, c
            )),
        }
    }

    /// `CPlugMaterial::DeviceMat[]`, whose layout grew with the chunk version
    /// that carries it.
    fn device_materials(&mut self, version: u32) -> R<Vec<i32>> {
        let n = self.r.u32()? as usize;
        let mut refs = Vec::new();
        for _ in 0..n {
            self.r.take(4)?; // two shorts
            if version >= 4 {
                self.r.bool32()?;
            }
            refs.push(self.noderef()?); // Shader1
            if version >= 9 {
                refs.push(self.noderef()?); // Shader2
                refs.push(self.noderef()?); // Shader3
            }
        }
        Ok(refs)
    }

    /// `CPlugMaterialCustom::Bitmap[]`: id, int, texture ref, and two more
    /// ints from version 1 (the chunk passes 1 whatever its own version).
    fn material_bitmaps(&mut self, version: u32) -> R<()> {
        let n = self.r.u32()? as usize;
        for _ in 0..n {
            self.r.lookback()?;
            self.r.i32()?;
            self.noderef()?;
            if version >= 1 {
                self.r.take(8)?;
            }
        }
        Ok(())
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
                    return Err(format!(
                        "surface mesh version {} (only >= 6 is understood)",
                        v
                    ));
                }
                let verts = self.r.array(|r| {
                    r.mark(3);
                    r.vec3()
                })?;
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
                    // translation is the last three of the twelve floats
                    self.r.marks.push((self.r.o + 36, 3));
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
                self.r.mark(6);
                let _aabb = self.r.boxf()?;
                let verts = self.r.array(|r| {
                    r.mark(3);
                    r.vec3()
                })?;
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
                        tris.push((
                            [face_idx[off], face_idx[off + k], face_idx[off + k + 1]],
                            0u8,
                            0u8,
                        ));
                    }
                }
                let verts = verts.iter().map(|v| apply(xform, *v)).collect();
                out.meshes.push(SurfMesh { verts, tris });
            }
            // Primitives: sphere, ellipsoid, box, cylinder, capsule, ...
            0 => {
                self.r.mark(1);
                self.r.take(4 + 2)?;
                out.primitives.push(ty);
            }
            1 => {
                self.r.mark(3);
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
            let u01 = r.i32()?;
            let lod = if version >= 1 { r.i32()? } else { 0 };
            let u02 = if version >= 32 { r.i32()? } else { 0 };
            Ok(ShadedGeom {
                visual,
                material,
                lod,
                u01,
                u02,
            })
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
                let m = self.noderef()?;
                out.material_nodes.push(m);
            }
        }
        self.noderef()?; // skel
        if version < 1 {
            return Ok(());
        }
        out.lod_max_dist = self.r.array(|r| r.f32())?; // lodDistances
        if version < 2 {
            return Ok(());
        }
        out.vis_cst_type = self.r.u32()?;
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
            let name = self.r.lookback()?;
            let is_node = self.r.bool32()?;
            let node = if is_node {
                self.noderef()?
            } else {
                self.r.string()?;
                -1
            };
            self.r.marks.push((self.r.o + 36, 3));
            let iso = self.r.iso4()?;
            out.lights.push((name, node, iso));
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
            let node = self.noderef()?;
            out.light_user_models.push(node);
        }
        out.light_insts = self.r.array(|r| {
            let model = r.u32()?;
            Ok((model, r.u32()?))
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
        let n_custom = if version >= 29 {
            material_count
        } else {
            mat_count_lt29
        };
        for _ in 0..n_custom {
            let name = self.r.string()?;
            let node = if name.is_empty() { self.noderef()? } else { -1 };
            out.material_nodes.push(node);
            out.material_names.push(name);
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
            return Err(format!(
                "solid2 u18 array has {} elements (only 0 is understood)",
                n_u18
            ));
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
            let count = if version >= 3 {
                self.r.u32()?
            } else {
                vertex_count
            };
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
                self.r
                    .take(4 * vertex_count as usize * flags.skin_index_count as usize)?;
            }
            self.r.array(|r| r.lookback())?;
            self.r.array(|r| r.i32())?;
        }
        self.r.mark(6);
        self.r.boxf()?; // BoundingBox
        Ok(())
    }

    /// `CPlugVisual3D`'s inline vertex array.
    ///
    /// **Inline vertices only exist when the visual has no vertex STREAM.** The
    /// two are alternatives, and reading the inline form anyway consumes forty
    /// bytes per vertex of somebody else's data: on 210218's embedded ice
    /// blocks that walked 604 bytes past the chunk and then read a 4-billion
    /// element tangent array, and the file failed to open at all.
    fn visual_inline_vertices(&mut self, acc: &mut Acc) -> R<()> {
        let f = acc.visual_flags;
        let n = acc.visual.count as usize;
        if acc.visual.vertex_streams.is_empty() {
            if !f.bit22 && !f.compress_float4_color && f.use_vertex_color {
                for _ in 0..n {
                    self.r.mark(3);
                    let p = self.r.vec3()?;
                    let nl = self.r.vec3()?;
                    self.r.take(16)?;
                    acc.visual.inline_positions.push(p);
                    acc.visual.inline_normals.push(nl);
                }
            } else {
                for _ in 0..n {
                    self.r.mark(3);
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
        // The full declaration layout, shared with `static_item::vstream`:
        // a declaration is (flags1, flags2) and, when `flags2 & 0xFFC` is
        // set, a u16 pair carrying the element's byte offset in the vertex.
        // Every declared element has a fixed stored size, so the whole
        // stream is walked without a recovery scan.
        use crate::static_item::vstream::{type_size, Decl, N_NORMAL, N_POSITION, N_TEXCOORD0, SPACE_LOCAL3D, T_DEC3N, T_FLOAT2, T_FLOAT3};
        let version = self.r.u32()?;
        let num = self.r.i32()?;
        let _flags = self.r.u32()?;
        let base = self.noderef()?;
        if num == 0 || base != -1 {
            return Ok(());
        }
        let num = num as usize;
        let mut decls = Vec::new();
        let n_decl = self.r.u32()? as usize;
        for _ in 0..n_decl {
            let flags1 = self.r.u32()?;
            let flags2 = self.r.u32()?;
            let d = Decl { flags1, flags2, extra: None, v0_data: Vec::new() };
            if flags2 & 0xFFC == 0 {
                if version == 0 {
                    let per = ((flags1 >> 0x12) & 0x3FF) as usize;
                    self.r.take(per * num)?;
                }
            } else {
                self.r.take(4)?;
            }
            decls.push(d);
        }
        if version == 0 {
            return Ok(());
        }
        let compress_local3d = self.r.bool32()?;
        for d in &decls {
            let stored = d.stored_type(compress_local3d);
            match (d.name(), stored) {
                (N_POSITION, T_FLOAT3) => {
                    self.r.mark(3 * num);
                    for _ in 0..num {
                        out.positions.push(self.r.vec3()?);
                    }
                }
                (N_POSITION, T_DEC3N) => {
                    // Packed positions cannot be rescaled in place; say so
                    // rather than leave a mesh at full size.
                    self.r.marks.push((usize::MAX, num));
                    for _ in 0..num {
                        out.positions.push(dec3n(self.r.u32()?));
                    }
                }
                (N_NORMAL, T_DEC3N) => {
                    for _ in 0..num {
                        out.normals.push(dec3n(self.r.u32()?));
                    }
                }
                (N_NORMAL, T_FLOAT3) => {
                    for _ in 0..num {
                        out.normals.push(self.r.vec3()?);
                    }
                }
                (N_TEXCOORD0, T_FLOAT2) => {
                    for _ in 0..num {
                        out.uv0.push(self.r.vec2()?);
                    }
                }
                (_, t) => {
                    let size = type_size(t).ok_or_else(|| format!("vertex element type {t} has no size"))?;
                    self.r.take(size * num)?;
                }
            }
        }
        let _ = SPACE_LOCAL3D;
        Ok(())
    }

    /// Skip to this node's terminator after meeting a layout we do not know.
    ///
    /// A GBX node ends with `0xFACADE01`, and that word is not a plausible
    /// float or packed normal, so scanning for it recovers the walk instead of
    /// desynchronising it. This is a REPORTED recovery, not a silent one: the
    /// node's remaining content is lost and the caller counts it, because a
    /// mesh quietly missing from a model is the failure this whole tool exists
    /// to avoid.
    #[allow(dead_code)]
    fn recover_to_facade(&mut self, what: &str) -> R<()> {
        let mut i = self.r.o;
        while i + 4 <= self.r.b.len() {
            if u32::from_le_bytes(self.r.b[i..i + 4].try_into().unwrap()) == 0xFACADE01 {
                self.r.o = i;
                self.recovered.push(what.to_string());
                return Ok(());
            }
            i += 1;
        }
        Err(format!("{}: no node terminator after it", what))
    }
}

#[allow(dead_code)]
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
    crate::blockinfo::known(cid) || matches!(
        cid,
        0x090F9000
            | 0x0901D000
            | 0x0901D002
            | 0x0901D003
            | 0x0901D004
            | 0x04001008
            | 0x04001009
            | 0x0400100A
            | 0x04005000
            | 0x04003003
            | 0x04003004
            | 0x04002002
            | 0x04002006
            | 0x04002008
            | 0x04002009
            | 0x0400200A
            | 0x0400A004
            | 0x0400A006
            | 0x0400B001
            | 0x0400B002
            | 0x0400B003
            | 0x04007001
            | 0x04007002
            | 0x04007003
            | 0x04007004
            | 0x04007005
            | 0x09005000
            | 0x09005010
            | 0x09005011
            | 0x09005017
            | 0x09005019
            | 0x0904F006
            | 0x0904F00D
            | 0x0904F011
            | 0x0904F016
            | 0x0904F017
            | 0x0904F01A
            | 0x09189000
            | 0x03168000
            | 0x0900C003
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
            | 0x0304E00F
            | 0x0304E013
            | 0x0304E017
            | 0x0304E020
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
            | 0x0917A000
            | 0x2E026000
            | 0x090FD000
            | 0x090FD001
            | 0x090FD002
            | 0x09003003
            | 0x09003004
            | 0x09003005
            | 0x09003006
            | 0x09003007
            | 0x09051000
            | 0x2E025000
            | 0x2E027000
            | 0x09119000
            | 0x09118000
            | 0x09079001
            | 0x09079002
            | 0x09079004
            | 0x09079007
            | 0x09079009
            | 0x0907900A
            | 0x0907900D
            | 0x0907900E
            | 0x0907900F
            | 0x09079010
            | 0x09079011
            | 0x09079015
            | 0x09079016
            | 0x09079017
            | 0x0903A004
            | 0x0903A006
            | 0x0903A00A
            | 0x0903A00B
            | 0x0903A00C
            | 0x0903A00D
            | 0x0903A00F
            | 0x0903A010
            | 0x0903A012
            | 0x0903A013
            | 0x0903A014
            | 0x0903A015
            | 0x0903A016
    )
}

/// `GbxOptimizedInt`: the game writes an index in the narrowest type that
/// holds the collection's size.
fn opt_int_size(max: usize) -> usize {
    if max < 256 {
        1
    } else if max < 65536 {
        2
    } else {
        4
    }
}
