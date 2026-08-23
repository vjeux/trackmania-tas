//! The GBX node graph, and the classes on the path from a block or an item to
//! its geometry.
//!
//! A GBX body is a graph, not a stream of records: a *node reference* is an
//! index, and the first reference to an index carries the node's whole body
//! inline. So the only way to reach node 40 is to have parsed nodes 1..39
//! byte-exactly. There is no seeking and no skipping — which is why a reader
//! for one class is really a reader for every class that class can reach.
//!
//! That has one consequence worth stating plainly, because it decides what
//! this file must contain: **an unknown non-skippable chunk is fatal, not
//! skippable.** A chunk whose length is not written down cannot be stepped
//! over, and guessing its length desynchronises the walk into somebody else's
//! floats. When this reader meets one it says which class and which chunk, and
//! that sentence is a task, not a verdict about the data.
//!
//! Chunk layouts follow `gbx-py`'s `src/gbx_structs.py`
//! (github.com/schadocalex/gbx-py), the community's transcription of the
//! format; where this file departs from it the comment says why.

use crate::reader::{Reader, R};
use std::collections::HashMap;

pub const C_SURFACE: u32 = 0x0900C000;
pub const C_VISUAL_INDEXED_TRIANGLES: u32 = 0x0901E000;
pub const C_VERTEX_STREAM: u32 = 0x09056000;
pub const C_INDEX_BUFFER: u32 = 0x09057000;
pub const C_SOLID2MODEL: u32 = 0x090BB000;
pub const C_PREFAB: u32 = 0x09145000;
pub const C_STATIC_OBJECT: u32 = 0x09159000;
pub const C_MATERIAL_USER_INST: u32 = 0x090FD000;
pub const C_ITEM_MODEL: u32 = 0x2E002000;
pub const C_VARIANT_LIST: u32 = 0x2F0BC000;
pub const C_BLOCK_ITEM: u32 = 0x2E025000;
pub const C_CRYSTAL: u32 = 0x09003000;
pub const C_COMMON_ITEM_ENTITY_MODEL: u32 = 0x2E027000;

/// Classes whose node body is a single struct with no chunk framing.
fn no_body_chunks(class_id: u32) -> bool {
    matches!(
        class_id,
        0x0912F000
            | 0x09144000
            | C_STATIC_OBJECT
            | 0x09178000
            | 0x0917B000
            | 0x09187000
            | C_PREFAB
            | 0x2F074000
            | 0x2F0BC000
            | 0x2F086000
            | 0x2F0CA000
    )
}

// ---------------------------------------------------------------- node kinds

#[derive(Clone, Debug)]
pub struct PrefabEnt {
    pub model: i32,
    /// Rotation as (x, y, z, w).
    pub rot: [f32; 4],
    pub pos: [f32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct Prefab {
    pub ents: Vec<PrefabEnt>,
}

#[derive(Clone, Debug)]
pub struct StaticObject {
    pub mesh: i32,
    pub mesh_collidable: bool,
    pub shape: i32,
}

/// One `CPlugSurface` leaf: a triangle soup with a physics material per face.
#[derive(Clone, Debug, Default)]
pub struct SurfMesh {
    pub verts: Vec<[f32; 3]>,
    /// (a, b, c, physics id, gameplay id)
    pub tris: Vec<([i32; 3], u8, u8)>,
}

/// A `CPlugSurface`'s shape tree, flattened to meshes in the surface's own
/// frame. A `Compound` places its children by an Iso4, which is applied here
/// rather than carried, because nothing downstream wants the tree.
#[derive(Clone, Debug, Default)]
pub struct Surface {
    pub meshes: Vec<SurfMesh>,
    /// Shape types met that are not triangle meshes — spheres, boxes,
    /// cylinders. Reported rather than dropped: a block whose collision is a
    /// primitive is a real answer, not a failure.
    pub primitives: Vec<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct VertexStream {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
}

#[derive(Clone, Debug, Default)]
pub struct Visual {
    pub vertex_streams: Vec<i32>,
    /// Positions written inline in `0x0902C004` (older visuals with no stream).
    pub inline_positions: Vec<[f32; 3]>,
    pub inline_normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub index_is_absolute: bool,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct ShadedGeom {
    pub visual: i32,
    pub material: i32,
    pub lod: i32,
}

/// One `CPlugCrystal` layer: an editable mesh of n-gon faces, each with a
/// material index into the crystal's own material-name list.
#[derive(Clone, Debug, Default)]
pub struct CrystalMesh {
    pub verts: Vec<[f32; 3]>,
    pub faces: Vec<(Vec<i32>, usize)>,
}

#[derive(Clone, Debug, Default)]
pub struct Crystal {
    /// Per slot: the literal material name, and the node index of the
    /// CPlugMaterialUserInst when the name is empty.
    pub materials: Vec<(String, i32)>,
    pub meshes: Vec<CrystalMesh>,
}

#[derive(Clone, Debug, Default)]
pub struct Solid2 {
    pub geoms: Vec<ShadedGeom>,
    /// Node index per visual slot, as referenced by `ShadedGeom::visual`.
    pub visuals: Vec<i32>,
    pub material_names: Vec<String>,
    /// Material NODES, in the same index space as `material_names`: a
    /// CPlugMaterialUserInst per slot when the names are empty.
    pub material_nodes: Vec<i32>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Prefab(Prefab),
    StaticObject(StaticObject),
    Surface(Surface),
    Solid2(Solid2),
    Visual(Visual),
    VertexStream(VertexStream),
    /// A class this reader walks but keeps nothing from.
    /// A `CGameItemModel` or `CGameCommonItemEntityModel`: a redirection to
    /// the node that actually holds the shape.
    Crystal(Crystal),
    /// A material: its name, and the physics id the car feels through it.
    Material(String, u8),
    ItemModel(i32),
    Other(u32),
}

impl Node {
    pub fn class_id(&self) -> u32 {
        match self {
            Node::Prefab(_) => C_PREFAB,
            Node::StaticObject(_) => C_STATIC_OBJECT,
            Node::Surface(_) => C_SURFACE,
            Node::Solid2(_) => C_SOLID2MODEL,
            Node::Visual(_) => C_VISUAL_INDEXED_TRIANGLES,
            Node::VertexStream(_) => C_VERTEX_STREAM,
            Node::Crystal(_) => C_CRYSTAL,
            Node::Material(..) => C_MATERIAL_USER_INST,
            Node::ItemModel(_) => C_ITEM_MODEL,
            Node::Other(c) => *c,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Slot {
    Unset,
    /// Reserved while its body is being read; a reference back to it here
    /// would be a cycle, which this format does not have.
    Reading,
    /// An entry of the reference table: another file, by name.
    External(String),
    Node(Node),
}

// ------------------------------------------------------------------ the walk

pub struct Graph<'a> {
    pub r: Reader<'a>,
    pub slots: Vec<Slot>,
    pub root: Option<Node>,
    /// Counts of chunks walked, by (class, chunk). Diagnostics only.
    pub seen: HashMap<(u32, u32), u32>,
    /// Places where a layout this reader does not know forced a scan to the
    /// node terminator. Never silent: whatever was in the node is missing.
    pub recovered: Vec<String>,
}

const FACADE: u32 = 0xFACADE01;
const SKIP: &[u8; 4] = b"PIKS";

impl<'a> Graph<'a> {
    pub fn new(body: &'a [u8], num_nodes: u32, externals: &[(u32, String)]) -> Graph<'a> {
        let mut slots = vec![Slot::Unset; (num_nodes as usize).max(1) + 1];
        for (i, name) in externals {
            let i = *i as usize;
            if i < slots.len() {
                slots[i] = Slot::External(name.clone());
            }
        }
        Graph { r: Reader::new(body), slots, root: None, seen: HashMap::new(), recovered: Vec::new() }
    }

    /// Parse a whole file body, rooted at `class_id`.
    pub fn parse(body: &'a [u8], class_id: u32, num_nodes: u32, externals: &[(u32, String)]) -> R<Graph<'a>> {
        let mut g = Graph::new(body, num_nodes, externals);
        let root = g.node_body(class_id)?;
        g.root = Some(root);
        Ok(g)
    }

    pub fn node(&self, idx: i32) -> Option<&Node> {
        match self.slots.get(idx.max(0) as usize) {
            Some(Slot::Node(n)) => Some(n),
            _ => None,
        }
    }
    pub fn external(&self, idx: i32) -> Option<&str> {
        match self.slots.get(idx.max(0) as usize) {
            Some(Slot::External(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Read a node reference. Returns the node index, or -1 for null.
    pub fn noderef(&mut self) -> R<i32> {
        let idx = self.r.i32()?;
        if idx <= 0 {
            return Ok(-1);
        }
        let u = idx as usize;
        if u >= self.slots.len() {
            // The node table is sized from the header. An index past it means
            // the walk is off the rails, not that the file has more nodes.
            return Err(format!("node ref {} past the {} declared nodes", idx, self.slots.len() - 1));
        }
        if matches!(self.slots[u], Slot::Unset) {
            let class_id = self.r.u32()?;
            if class_id == 0xFFFF_FFFF {
                self.slots[u] = Slot::Node(Node::Other(class_id));
                return Ok(idx);
            }
            self.slots[u] = Slot::Reading;
            let n = self
                .node_body(class_id)
                .map_err(|e| format!("node {} (class 0x{:08X}): {}", idx, class_id, e))?;
            self.slots[u] = Slot::Node(n);
        }
        Ok(idx)
    }

    /// A node's body: either one struct (`no_body_chunks`) or a chunk loop.
    pub fn node_body(&mut self, class_id: u32) -> R<Node> {
        crate::reader::trace(|| format!("node class 0x{:08X} at 0x{:x}", class_id, self.r.o));
        if no_body_chunks(class_id) {
            return self.plain_body(class_id);
        }
        let mut acc = Acc::new(class_id);
        loop {
            if self.r.eof() {
                // A body that ends without FACADE is legal for the outermost
                // node of some files; inside a node ref it is not, but we
                // cannot tell from here, so accept and let the caller's own
                // structure fail if it was wrong.
                break;
            }
            let cid = self.r.u32()?;
            crate::reader::trace(|| format!("  chunk 0x{:08X} of class 0x{:08X} at 0x{:x}", cid, class_id, self.r.o));
            if cid == FACADE {
                break;
            }
            let skippable = self.r.b.get(self.r.o..self.r.o + 4) == Some(SKIP);
            if skippable {
                self.r.u32()?;
                let size = self.r.u32()? as usize;
                let known = self.chunk_is_known(class_id, cid);
                if known {
                    let end = self.r.o + size;
                    if end > self.r.b.len() {
                        return Err(format!(
                            "skippable chunk 0x{:08X} of {} bytes past end of body",
                            cid, size
                        ));
                    }
                    self.chunk(class_id, cid, &mut acc)?;
                    // Trailing bytes inside a skippable chunk are normal (the
                    // game writes more than any one reader consumes); jump to
                    // the declared end rather than trusting our own cursor.
                    self.r.o = end;
                } else {
                    self.r.take(size)?;
                }
            } else {
                self.chunk(class_id, cid, &mut acc).map_err(|e| {
                    format!("class 0x{:08X} chunk 0x{:08X} at 0x{:x}: {}", class_id, cid, self.r.o, e)
                })?;
            }
            *self.seen.entry((class_id, cid)).or_insert(0) += 1;
        }
        Ok(acc.finish(class_id))
    }

}

/// Accumulates a node's chunks into one value.
pub struct Acc {
    pub class_id: u32,
    pub prefab: Prefab,
    pub statobj: Option<StaticObject>,
    pub surface: Surface,
    pub solid2: Solid2,
    pub visual: Visual,
    pub vstream: VertexStream,
    pub visual_flags: crate::classes::VisualFlags,
    /// The node an item model hands its geometry to (`0x2E002019` or
    /// `0x2E027000`). `-1` when the class carries none.
    pub entity_model: i32,
    pub crystal_materials: Vec<(String, i32)>,
    pub crystals: Vec<CrystalMesh>,
    pub material_name: String,
    pub physics_id: u8,
    pub touched: bool,
}

impl Acc {
    fn new(class_id: u32) -> Acc {
        Acc {
            class_id,
            prefab: Prefab::default(),
            statobj: None,
            surface: Surface::default(),
            solid2: Solid2::default(),
            visual: Visual::default(),
            vstream: VertexStream::default(),
            visual_flags: crate::classes::VisualFlags::default(),
            entity_model: -1,
            crystal_materials: Vec::new(),
            crystals: Vec::new(),
            material_name: String::new(),
            physics_id: 0,
            touched: false,
        }
    }
    fn finish(self, class_id: u32) -> Node {
        if !self.touched {
            return Node::Other(class_id);
        }
        match class_id {
            C_SURFACE => Node::Surface(self.surface),
            C_SOLID2MODEL => Node::Solid2(self.solid2),
            C_MATERIAL_USER_INST => Node::Material(self.material_name, self.physics_id),
            C_CRYSTAL => Node::Crystal(Crystal {
                materials: self.crystal_materials,
                meshes: self.crystals,
            }),
            C_VERTEX_STREAM => Node::VertexStream(self.vstream),
            C_ITEM_MODEL | C_COMMON_ITEM_ENTITY_MODEL | C_BLOCK_ITEM => {
                Node::ItemModel(self.entity_model)
            }
            c if is_visual(c) => Node::Visual(self.visual),
            c => Node::Other(c),
        }
    }
}

pub fn is_visual(class_id: u32) -> bool {
    matches!(
        class_id,
        C_VISUAL_INDEXED_TRIANGLES | 0x0906A000 | 0x0902C000 | 0x09006000 | 0x0900F000
    )
}
