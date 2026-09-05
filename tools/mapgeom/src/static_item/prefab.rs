//! `CPlugPrefab` (0x09145000) read with the typed static-object model: every
//! entity's model is a `Node` (inline `CPlugStaticObjectModel` trees parse
//! fully; external nodes stay references), its placement is a quaternion and
//! a position, its instance params are kept as the raw words GBX.NET's
//! layouts size them to. `Model::externals` names the external nodes.

use super::{read_ref, write_ref, Rd, Ref, Wr, R};
use crate::store::Model;

#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    pub model: Ref,
    /// (x, y, z, w)
    pub rot: [f32; 4],
    pub pos: [f32; 3],
    /// Params chunk id (-1 for none) and its payload words.
    pub params_id: i32,
    pub params: Vec<u8>,
    /// The length-prefixed byte blob after the params.
    pub u01: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugPrefab {
    pub version: u32,
    pub file_write_time: u64,
    pub url: String,
    pub u01: i32,
    pub u02: i32,
    pub ents: Vec<Entity>,
}

fn read_params(r: &mut Rd, id: i32) -> R<Vec<u8>> {
    let start = r.o;
    match id {
        -1 => {}
        // NPlugDynaObjectModel_SInstanceParams
        0x2F0B6000 => {
            let v = r.i32()?;
            r.take(12)?;
            if v >= 1 {
                r.take(12)?;
            }
            if v >= 2 {
                r.take(4)?;
            }
        }
        // NPlugDyna_SPrefabConstraintParams
        0x2F0C8000 => {
            r.u32()?;
            r.take(8 + 24)?;
        }
        // NPlugItemPlacement_SPlacement
        0x2F0A9000 => {
            r.u32()?;
            r.i32()?;
            r.array(|r| r.array(|r| Ok((r.string()?, r.string()?))))?;
        }
        // NPlugItemPlacement_SPlacementGroup
        0x2F0D8000 => {
            r.u32()?;
            r.array(|r| {
                r.u32()?;
                r.i32()?;
                r.array(|r| r.array(|r| Ok((r.string()?, r.string()?))))?;
                Ok(())
            })?;
            r.array(|r| r.u16())?;
            r.array(|r| r.take(28))?;
        }
        // NPlugStaticObjectModel_SInstanceParams: version, phase
        0x2F0D9000 => {
            r.take(8)?;
        }
        c => return Err(format!("prefab entity params chunk 0x{c:08X} has no reader")),
    }
    Ok(r.b[start..r.o].to_vec())
}

impl CPlugPrefab {
    /// Parse a prefab body (`Model::body`); `externals` are the node indices
    /// the reference table defines, which never appear inline.
    pub fn parse(body: &[u8], externals: &[u32]) -> R<CPlugPrefab> {
        let mut lb = super::LookbackState::default();
        lb.defined_nodes.extend(externals.iter().copied());
        let mut r = Rd::new(body, 0, lb);
        let version = r.u32()?;
        let file_write_time = r.u64()?;
        let url = r.string()?;
        let u01 = r.i32()?;
        let n = r.count()?;
        let u02 = r.i32()?;
        let mut ents = Vec::with_capacity(n);
        for i in 0..n {
            let ctx = |e: String| format!("prefab entity {i}/{n}: {e}");
            let model = read_ref(&mut r).map_err(ctx)?;
            let rot = r.floats::<4>()?;
            let pos = r.vec3()?;
            let params_id = r.i32()?;
            let params = read_params(&mut r, params_id).map_err(ctx)?;
            let k = r.count()?;
            let u01 = r.take(k)?.to_vec();
            ents.push(Entity { model, rot, pos, params_id, params, u01 });
        }
        if r.o != body.len() {
            return Err(format!("prefab: {} trailing bytes after the entities at 0x{:x}", body.len() - r.o, r.o));
        }
        Ok(CPlugPrefab { version, file_write_time, url, u01, u02, ents })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut lb = super::LookbackState::default();
        let mut w = Wr { w: &mut out, lb: &mut lb };
        w.u32(self.version);
        w.u64(self.file_write_time);
        w.string(&self.url);
        w.i32(self.u01);
        w.u32(self.ents.len() as u32);
        w.i32(self.u02);
        for e in &self.ents {
            write_ref(&mut w, &e.model);
            w.floats(&e.rot);
            w.floats(&e.pos);
            w.i32(e.params_id);
            w.bytes(&e.params);
            w.u32(e.u01.len() as u32);
            w.bytes(&e.u01);
        }
        out
    }

    /// Parse a prefab loaded through the store.
    pub fn from_model(m: &Model) -> R<CPlugPrefab> {
        if m.class_id != 0x09145000 {
            return Err(format!("{}: class 0x{:08X} is not CPlugPrefab", m.path, m.class_id));
        }
        Self::parse(&m.body, &m.external_indices()).map_err(|e| format!("{}: {e}", m.path))
    }

    /// The entity's rotation + position as the game's Iso4 (three rotation
    /// columns, then the translation).
    pub fn entity_iso(e: &Entity) -> [f32; 12] {
        crate::geom::from_quat(e.rot, e.pos)
    }
}
