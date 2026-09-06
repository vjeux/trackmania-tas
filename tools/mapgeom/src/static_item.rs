//! A complete, typed model of a Trackmania 2020 **static object item**: the
//! `.Item.Gbx` layout the game writes for a mesh-only item
//! (`CGameItemModel` -> `CGameCommonItemEntityModel` ->
//! `CPlugStaticObjectModel` -> `CPlugSolid2Model` + `CPlugSurface`), parsed
//! to structs and serialised back byte for byte.
//!
//! Definitions follow GBX.NET (`/tmp/gbxnet/*.chunkl|cs`); the reader is
//! exact by construction — no recovery scans, every element of every vertex
//! stream is read from its declaration — and `examples/static_roundtrip.rs`
//! demands identical bytes for the reference corpus.
//!
//! Shared encodings (lookback ids, node references, optimized ints) come from
//! `crystal_model`, whose `Rd`/`Wr`/`LookbackState` are reused as is.
//!
//! Layout of the module:
//!
//! * `vstream`  — `CPlugVertexStream` (0x09056000), every declared element
//! * `visual`   — `CPlugVisualIndexedTriangles` (0x0901E000) and the inline
//!                `CPlugIndexBuffer`
//! * `solid2`   — `CPlugSolid2Model` (0x090BB000)
//! * `surface`  — `CPlugSurface` (0x0900C000)
//! * `item`     — `CGameItemModel` chunks, `CGameCommonItemEntityModel`,
//!                `CPlugStaticObjectModel`, `CGameItemPlacementParam`
//! * `file`     — the container: header chunks, node count, body
//! * `prefab`   — `CPlugPrefab` read with the same model (S2/S3)
//! * `build`    — constructing an item from prefab / item geometry (S3)

pub mod oldmat;
pub mod vstream;
pub mod visual;
pub mod solid2;
pub mod surface;
pub mod item;
pub mod file;
pub mod prefab;
pub mod build;
pub mod bake;
pub mod cli;

pub use crate::crystal_model::{Id, LookbackState, NodeRef, OpaqueNode, Rd, Wr, R, FACADE};
pub use file::{parse_file, write_file, StaticItemFile};
pub use item::CGameItemModel;

pub const C_ITEM_MODEL: u32 = 0x2E002000;
pub const C_COMMON_ITEM_ENTITY_MODEL: u32 = 0x2E027000;
pub const C_STATIC_OBJECT_MODEL: u32 = 0x09159000;
pub const C_SOLID2_MODEL: u32 = 0x090BB000;
pub const C_VISUAL_INDEXED_TRIANGLES: u32 = 0x0901E000;
pub const C_VERTEX_STREAM: u32 = 0x09056000;
pub const C_INDEX_BUFFER: u32 = 0x09057000;
pub const C_SURFACE: u32 = 0x0900C000;
pub const C_ITEM_PLACEMENT_PARAM: u32 = 0x2E020000;
pub const C_MATERIAL_USER_INST: u32 = crate::crystal_model::C_MATERIAL_USER_INST;
pub const C_MATERIAL: u32 = 0x09079000;
pub const C_MATERIAL_CUSTOM: u32 = 0x0903A000;

const SKIP: &[u8; 4] = b"PIKS";

/// Any node this model can hold inline.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    EntityModel(item::CGameCommonItemEntityModel),
    StaticObject(item::CPlugStaticObjectModel),
    Solid2(solid2::CPlugSolid2Model),
    Visual(visual::CPlugVisualIndexedTriangles),
    VertexStream(vstream::CPlugVertexStream),
    Material(crate::crystal_model::CPlugMaterialUserInst),
    /// Pre-UserInst material (BlueBay terrain prefabs): read-only source.
    OldMaterial(oldmat::OldMaterial),
    /// Its nested custom node: read-only source.
    OldCustom(oldmat::OldCustom),
    Surface(surface::CPlugSurface),
    Placement(item::CGameItemPlacementParam),
    /// A class with no reader here, made only of skippable chunks.
    Opaque(OpaqueNode),
}

impl Node {
    pub fn class_id(&self) -> u32 {
        match self {
            Node::EntityModel(_) => C_COMMON_ITEM_ENTITY_MODEL,
            Node::StaticObject(_) => C_STATIC_OBJECT_MODEL,
            Node::Solid2(_) => C_SOLID2_MODEL,
            Node::Visual(_) => C_VISUAL_INDEXED_TRIANGLES,
            Node::VertexStream(_) => C_VERTEX_STREAM,
            Node::Material(_) => C_MATERIAL_USER_INST,
            Node::OldMaterial(_) => C_MATERIAL,
            Node::OldCustom(_) => C_MATERIAL_CUSTOM,
            Node::Surface(_) => C_SURFACE,
            Node::Placement(_) => C_ITEM_PLACEMENT_PARAM,
            Node::Opaque(o) => o.class_id,
        }
    }
}

/// Read an inline node body (after its class id) by class.
pub fn read_node(r: &mut Rd, class_id: u32) -> R<Node> {
    Ok(match class_id {
        C_COMMON_ITEM_ENTITY_MODEL => Node::EntityModel(item::CGameCommonItemEntityModel::parse(r)?),
        C_STATIC_OBJECT_MODEL => Node::StaticObject(item::CPlugStaticObjectModel::parse(r)?),
        C_SOLID2_MODEL => Node::Solid2(solid2::CPlugSolid2Model::parse(r)?),
        C_VISUAL_INDEXED_TRIANGLES => Node::Visual(visual::CPlugVisualIndexedTriangles::parse(r)?),
        C_VERTEX_STREAM => Node::VertexStream(vstream::CPlugVertexStream::parse(r)?),
        C_MATERIAL_USER_INST => Node::Material(crate::crystal_model::CPlugMaterialUserInst::parse(r)?),
        C_MATERIAL => Node::OldMaterial(oldmat::OldMaterial::parse(r)?),
        C_MATERIAL_CUSTOM => Node::OldCustom(oldmat::OldCustom::parse(r)?),
        C_SURFACE => Node::Surface(surface::CPlugSurface::parse(r)?),
        C_ITEM_PLACEMENT_PARAM => Node::Placement(item::CGameItemPlacementParam::parse(r)?),
        // Trigger-side and path classes of the gate / special prefabs: no
        // geometry, unskippable bodies. Read as the generic walker
        // (`classes.rs`) does and kept raw so the entity list stays walkable.
        0x09178000 | 0x0917A000 | 0x0917B000 | 0x09119000 | 0x09118000 => Node::Opaque(read_fixed_opaque(r, class_id)?),
        other => Node::Opaque(read_opaque(r, other)?),
    })
}

/// Write an inline node body (after its class id).
pub fn write_node(w: &mut Wr, n: &Node) {
    match n {
        Node::EntityModel(x) => x.write(w),
        Node::StaticObject(x) => x.write(w),
        Node::Solid2(x) => x.write(w),
        Node::Visual(x) => x.write(w),
        Node::VertexStream(x) => x.write(w),
        Node::Material(x) => x.write(w),
        Node::OldMaterial(x) => x.write(w),
        Node::OldCustom(x) => x.write(w),
        Node::Surface(x) => x.write(w),
        Node::Placement(x) => x.write(w),
        Node::Opaque(o) => w.bytes(&o.raw),
    }
}

/// A node reference whose inline body (if any) is a `Node`.
pub type Ref = NodeRef<Node>;

pub fn read_ref(r: &mut Rd) -> R<Ref> {
    r.noderef(read_node)
}

pub fn write_ref(w: &mut Wr, n: &Ref) {
    let cid = n.inline.as_ref().map(|b| b.class_id()).unwrap_or(0);
    w.noderef(n, cid, write_node);
}

pub fn null_ref() -> Ref {
    NodeRef { index: -1, inline: None }
}

/// A raw chunk kept verbatim: `id`, then for a skippable chunk the payload
/// (written back as `PIKS`, size, payload).
#[derive(Clone, Debug, PartialEq)]
pub struct RawChunk {
    pub id: u32,
    pub payload: Vec<u8>,
}

/// Read the `PIKS` + size + payload of a skippable chunk whose id was read.
pub fn read_skippable_payload(r: &mut Rd, id: u32) -> R<Vec<u8>> {
    if r.take(4)? != SKIP {
        return Err(format!("chunk 0x{:08X} at 0x{:x} is not skippable", id, r.o - 8));
    }
    let n = r.count()?;
    Ok(r.take(n)?.to_vec())
}

pub fn write_skippable(w: &mut Wr, id: u32, payload: &[u8]) {
    w.u32(id);
    w.bytes(SKIP);
    w.u32(payload.len() as u32);
    w.bytes(payload);
}

pub fn is_skippable_here(r: &Rd) -> bool {
    r.b.get(r.o..r.o + 4) == Some(&SKIP[..])
}

fn read_opaque(r: &mut Rd, class_id: u32) -> R<OpaqueNode> {
    let start = r.o;
    loop {
        let cid = r.u32()?;
        if cid == FACADE {
            break;
        }
        if is_skippable_here(r) {
            read_skippable_payload(r, cid)?;
        } else {
            return Err(format!("class 0x{:08X} has no reader and chunk 0x{:08X} is not skippable", class_id, cid));
        }
    }
    Ok(OpaqueNode { class_id, raw: r.b[start..r.o].to_vec() })
}

/// Bodies of the trigger/path classes met inside prefabs (read off the gate
/// and turbo files, same layouts as `classes.rs`):
/// * `NPlugTrigger_SWaypoint` 0x09178000: version, TriggerShape ref, 8 bytes;
/// * `NPlugTrigger_SSpecial` 0x0917A000: CHUNKED -- chunk 0x0917A000 = version,
///   Iso4 (48 bytes), 24 bytes; then FACADE;
/// * 0x0917B000: 8 bytes;
/// * `CPlugPath` 0x09119000: version, N refs (polylines), v>=2: bool32, u8,
///   length-prefixed bytes.
/// The plain bodies (0x09178000, 0x0917B000) have NO chunk framing and no
/// FACADE: the class IS the struct (as `classes.rs::plain_body`).
fn read_fixed_opaque(r: &mut Rd, class_id: u32) -> R<OpaqueNode> {
    let start = r.o;
    match class_id {
        // plain bodies: the class IS the struct, nothing follows it
        0x09178000 => {
            r.u32()?;
            r.noderef(read_node)?;
            r.take(8)?;
        }
        0x0917B000 => {
            r.take(8)?;
        }
        // chunked bodies: chunk ids up to FACADE
        0x09119000 | 0x09118000 | 0x0917A000 => loop {
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            match cid {
                // NPlugTrigger_SSpecial: version, Iso4, 24 bytes
                0x0917A000 => {
                    r.u32()?;
                    r.take(48 + 4 + 4 + 12 + 4)?;
                }
                0x09119000 => {
                    let v = r.u32()?;
                    let n = r.u32()? as usize;
                    for _ in 0..n {
                        r.noderef(read_node)?;
                    }
                    if v >= 2 {
                        r.bool32()?;
                        r.u8()?;
                        let k = r.u32()? as usize;
                        r.take(k)?;
                    }
                }
                0x09118000 => {
                    let v = r.u32()?;
                    let n = r.u32()? as usize;
                    r.take(n * 12)?;
                    if v >= 2 {
                        let n2 = r.u32()? as usize;
                        r.take(n2 * 12)?;
                    }
                    if v == 3 {
                        r.bool32()?;
                        r.i32()?;
                    }
                    if v >= 4 {
                        if v == 4 {
                            r.bool32()?;
                        }
                        r.bool32()?;
                        r.bool32()?;
                        if v >= 5 {
                            r.bool32()?;
                        }
                        if v >= 6 {
                            r.i32()?;
                        }
                        if v >= 7 {
                            r.u8()?;
                        }
                        if v >= 8 {
                            r.u8()?;
                            r.id()?;
                        }
                    }
                }
                _ => {
                    if is_skippable_here(r) {
                        read_skippable_payload(r, cid)?;
                    } else {
                        return Err(format!("class 0x{class_id:08X}: chunk 0x{cid:08X} has no reader and is not skippable"));
                    }
                }
            }
        },
        _ => unreachable!(),
    }
    Ok(OpaqueNode { class_id, raw: r.b[start..r.o].to_vec() })
}
