//! The pak cipher's "dummy write": which value the engine folds into the
//! Blowfish stream at the start of every node body, read off the client
//! itself (`Trackmania.exe`, `CMwNod::Archive` at 0x1402d0720 — vtable slot
//! 14 of every node class; DISASSEMBLY 2026-09-23, `docs/formats/pak-nadeopak.md` §4).
//!
//! ```text
//! CMwNod::Archive(this, archive):
//!     info = this->GetClassInfo()                       // vtable slot 2
//!     if (info->parent != NULL) {                       // +0x20; only CMwNod and the primitive types have none
//!         archive->buffer->dummy = 1                    // top buffer +0x10: its Write folds instead of writing
//!         switch (info->classId) {                      // +0x18, the ENGINE id
//!             case 0x0A003000:              v = 0x0A001000   // CSceneLayout  → CScene       (its parent is CMwNod today)
//!             case 0x090BF000:              v = 0x0804B000   // CPlugMaterialFx-family → its Maniaplanet-era parent
//!             case 0x0917E000, 0x09184000:  v = 0x05010000
//!             case 0x09185000:              v = 0x05002000   // → CFuncShader
//!             default:                      v = REMAP(info->parent->classId)   // 0x1402f3570: 0x03xxxxxx → 0x24xxxxxx, else identity
//!                                           if (v == 0x07031000) v = 0x07001000  // a CControlText child folds CControlBase (the pre-CControlText layout)
//!         }
//!         if (v == 0) v = 0xFFFFFFFF                    // 0x1402d1d00 (never taken: a parent always has an id)
//!         archive->Write4(&v)                           // 0x14012bbe0 → crypt buffer Write 0x1413b0e00 in dummy mode:
//!                                                       //   per byte b: iv_xor = rotl64(iv_xor, 13) ^ (b | 0xAA)
//!         archive->buffer->dummy = 0
//!     }
//!     … the chunk loop …
//! ```
//!
//! The fold reaches the cipher at its next 0x100-byte batch (`blowfish.rs`);
//! `pakfile.rs` places it. Three more engine facts the placement needs
//! (same function and `ArchiveNodRef` 0x140905cb0):
//! * a node reference folds ONLY when it creates a new inline node (index
//!   not yet in the node table); `-1` refs, back-references and external
//!   (reference-table) nodes fold nothing;
//! * an inline node inside a SKIPPABLE chunk folds nothing: the reader
//!   pulls the whole chunk into a memory buffer first (`PIKS` + size), and
//!   the memory buffer's dummy Write is a no-op (0x140123c80);
//! * `DontUseDummyWrite` (pak entry flag bit 32 → crypt buffer +0x18) turns
//!   every dummy write into a no-op.
//!
//! Three explicit per-class dummy writes exist besides the generic one (GBX.NET
//! knows them too): `CPlugVehiclePhyTuning`/`CPlugVehicleCarPhyTuning` fold
//! the first four bytes of their name (0x1405fbcca, 0x1405da97d) and
//! `CPlugSurfaceGeom` chunk 0x0900F004 folds `f32(box.X − box.X2)` (0x14051ed4e).
//!
//! The fold lives in `CMwNod::Archive` — the chunk loop. A class whose own
//! `Archive` override never calls it (a plain-struct body: CPlugVegetTreeModel,
//! CPlugDynaObjectModel, CPlugPrefab, CPlugStaticObjectModel, the
//! `NPlug*::S*` structs — `NO_FOLD_CLASSES`, 92 of the 1857 vtables, read off
//! slot 14 of every vtable) folds nothing even though it has a parent: the
//! 172 `.VegetTreeModel.Gbx` and 24 `.DynaObject.Gbx` pak entries decode
//! without a main-node fold and garble with one (measured 2026-09-23). The
//! CPlugVisual* family, CPlugBitmap and CPlugMaterial override `Archive` but
//! tail-call the base before writing anything, so they fold at the body start
//! like everyone else.
//!
//! The class table is the engine's own (`engine_classes.rs`, 1905 classes
//! from the class registrations), not GBX.NET's `inherits:` lines: the two
//! differ where it matters (CGameCtnChallenge : CMwNod here, CGameCtnBlockInfo
//! : CGameCtnCollector 0x2E001000, CPlugDynaObjectModel : CMwNod), and the
//! special cases above are not derivable from any hierarchy. A class id read
//! from a file is first put through NORMALISE (0x1402f2610: the read-side
//! id aliasing, e.g. `0x24003000 → 0x03043000`), as the engine does before
//! its registry lookup.

use crate::engine_classes::{ENGINE_CLASSES, NORMALISE, NO_FOLD_CLASSES, REMAP};

pub const C_MWNOD: u32 = 0x01001000;
pub const C_PLUG: u32 = 0x0902B000;

fn lookup(table: &[(u32, u32)], key: u32) -> Option<u32> {
    // the generated tables are sorted by key
    table.binary_search_by_key(&key, |&(k, _)| k).ok().map(|i| table[i].1)
}

/// A class id as it appears in a file → the engine's class id (0x1402f2610).
pub fn normalise(file_id: u32) -> u32 {
    lookup(NORMALISE, file_id).unwrap_or(file_id)
}

/// An engine class id → the id the engine writes for it (0x1402f3570).
pub fn remap(engine_id: u32) -> u32 {
    lookup(REMAP, engine_id).unwrap_or(engine_id)
}

/// The engine's parent of a class (engine ids); `None` for a class the
/// table does not know, `Some(0)` for a root (CMwNod, the primitive types).
pub fn engine_parent(engine_id: u32) -> Option<u32> {
    lookup(ENGINE_CLASSES, engine_id)
}

/// The value the engine folds into the cipher at the start of this class's
/// node body (`class_id` as read from the file), or `None` when the class is
/// unknown to the table — the caller falls back on the fold hunt then. A
/// known root class folds nothing: `Some(NO_FOLD)`... expressed as `None`
/// would hide the difference, so roots return `Some(0)` and callers skip a
/// zero.
pub fn dummy_write_class(class_id: u32) -> Option<u32> {
    let e = normalise(class_id);
    let parent = engine_parent(e)?;
    if parent == 0 || NO_FOLD_CLASSES.binary_search(&e).is_ok() {
        return Some(0);
    }
    Some(fold_value(e, parent))
}

/// `CMwNod::Archive`'s switch: the fold value for an engine class with the
/// given engine parent.
pub fn fold_value(engine_id: u32, engine_parent: u32) -> u32 {
    match engine_id {
        0x0A003000 => 0x0A001000,
        0x090BF000 => 0x0804B000,
        0x0917E000 | 0x09184000 => 0x05010000,
        0x09185000 => 0x05002000,
        _ => {
            let v = remap(engine_parent);
            if v == 0x07031000 { 0x07001000 } else { v }
        }
    }
}

/// Every fold value a node of a known class can produce — the alphabet of a
/// fold hunt (canonicalised by the caller: only bits 0/2/4/6 of each byte
/// reach the cipher).
pub fn known_classes() -> Vec<u32> {
    let mut v: Vec<u32> = ENGINE_CLASSES
        .iter()
        .filter(|&&(_, p)| p != 0)
        .map(|&(c, p)| fold_value(c, p))
        .collect();
    v.extend([0xFFFF_FFFF]);
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene3d_folds() {
        // BlueBay\GameCtnDecoration\Scene3d\Base64x64.Scene3d.Gbx, 43 folds
        // measured 2026-09-23: the main node's is the engine special case
        assert_eq!(dummy_write_class(0x0A003000), Some(0x0A001000));
        assert_eq!(dummy_write_class(0x04005000), Some(0x04001000)); // GxLightAmbient : GxLight
        assert_eq!(dummy_write_class(0x04007000), Some(0x04006000)); // GxLightDirectional : GxLightNotAmbient
        assert_eq!(dummy_write_class(0x09005000), Some(C_PLUG)); // CPlugSolid
        assert_eq!(dummy_write_class(0x0904F000), Some(C_PLUG)); // CPlugTree
        assert_eq!(dummy_write_class(0x09056000), Some(C_PLUG)); // CPlugVertexStream
        assert_eq!(dummy_write_class(0x0901E000), Some(0x0906A000)); // CPlugVisualIndexedTriangles : CPlugVisualIndexed
        assert_eq!(dummy_write_class(0x09057000), Some(C_PLUG)); // CPlugIndexBuffer
        assert_eq!(dummy_write_class(C_MWNOD), Some(0)); // the root folds nothing
        assert_eq!(dummy_write_class(0x0901D000), Some(C_PLUG)); // CPlugLight (the .Light.Gbx files)
    }

    #[test]
    fn plain_struct_bodies_fold_nothing() {
        // a parent, but an `Archive` that never reaches CMwNod::Archive
        assert_eq!(dummy_write_class(0x2F086000), Some(0)); // CPlugVegetTreeModel
        assert_eq!(dummy_write_class(0x09144000), Some(0)); // CPlugDynaObjectModel
        assert_eq!(dummy_write_class(0x09145000), Some(0)); // CPlugPrefab
        assert_eq!(dummy_write_class(0x09159000), Some(0)); // CPlugStaticObjectModel
        // CMwNod-parented classes with the chunk loop DO fold (CMwNod's id)
        assert_eq!(dummy_write_class(0x0303A000), Some(C_MWNOD)); // CGameCtnDecorationMood
        assert_eq!(dummy_write_class(0x2E020000), Some(C_MWNOD)); // CGameItemPlacementParam
        assert_eq!(dummy_write_class(0x0915C000), Some(C_MWNOD)); // CPlugFxSystem
        // an override that tail-calls the base folds at the body start
        assert_eq!(dummy_write_class(0x09011000), Some(C_PLUG)); // CPlugBitmap
    }

    #[test]
    fn remapped_ids() {
        // a CGameCtnBlockInfoClassic (file id 0x03051000) folds its parent
        // CGameCtnBlockInfo as written in files: 0x24005000
        assert_eq!(dummy_write_class(0x03051000), Some(0x24005000));
        // the file spelling of a remapped class normalises back
        assert_eq!(normalise(0x24003000), 0x03043000);
        assert_eq!(remap(0x03043000), 0x24003000);
        assert_eq!(dummy_write_class(0x24003000), Some(C_MWNOD)); // CGameCtnChallenge : CMwNod
        // a CControlText (0x07031000) child folds CControlBase, the class's
        // pre-CControlText parent; CControlText itself folds its real parent
        assert_eq!(dummy_write_class(0x07006000), Some(0x07001000)); // CControlLabel
        assert_eq!(dummy_write_class(0x07031000), Some(0x07001000)); // CControlText : CControlBase
    }
}
