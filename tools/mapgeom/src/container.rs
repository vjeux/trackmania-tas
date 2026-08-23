//! The GBX container: header, the external-reference table (with names — the
//! part every other reader in this project throws away), and the body.
//!
//! `gbx::Container` already owns the container for ghosts and replays, and it
//! is the right place for LZO. What it does not keep is the reference table's
//! *contents*: for a ghost that is dead weight, but for a block model it is the
//! entire point — the ref table is where a `.EDClassic.Gbx` names the prefab
//! files that hold its mesh. So this reads the same header and keeps the names.

use crate::reader::{Reader, R};

#[derive(Clone, Debug)]
pub struct ExtRef {
    /// The file this node stands for, as the file itself spells it —
    /// `TiltCurve3_Air.Prefab.Gbx`, relative to the folder tree below.
    pub name: String,
    /// The node index this external file occupies in the body's node graph.
    pub node_index: u32,
    /// Index into `folders`, or `None` when the ref names a resource index.
    pub folder: Option<usize>,
    pub use_file: bool,
}

#[derive(Clone, Debug)]
pub struct Gbx {
    pub version: u16,
    pub class_id: u32,
    pub user_data: Vec<u8>,
    pub num_nodes: u32,
    /// How many folders up from the file's own folder the ref paths start.
    pub ancestor_level: u32,
    /// Flattened folder tree; each entry is a path relative to the ancestor.
    pub folders: Vec<String>,
    pub refs: Vec<ExtRef>,
    pub body: Vec<u8>,
}

impl Gbx {
    pub fn parse(data: &[u8]) -> R<Gbx> {
        if data.len() < 16 || &data[0..3] != b"GBX" {
            return Err("not a GBX file".into());
        }
        let mut r = Reader::new(data);
        r.take(3)?;
        let version = r.u16()?;
        let _format = r.u8()?;
        let _ref_comp = r.u8()?;
        let body_comp = r.u8()?;
        if version >= 4 {
            r.u8()?;
        }
        let class_id = r.u32()?;
        let mut user_data = Vec::new();
        if version >= 6 {
            let n = r.u32()? as usize;
            user_data = r.take(n)?.to_vec();
        }
        let num_nodes = r.u32()?;

        let mut folders: Vec<String> = Vec::new();
        let mut refs: Vec<ExtRef> = Vec::new();
        let mut ancestor_level = 0u32;
        let n_ext = r.u32()?;
        if n_ext > 0 {
            ancestor_level = r.u32()?;
            let n_root = r.u32()?;
            fn walk(r: &mut Reader, cnt: u32, prefix: &str, out: &mut Vec<String>) -> R<()> {
                for _ in 0..cnt {
                    let name = r.string()?;
                    let path =
                        if prefix.is_empty() { name.clone() } else { format!("{}\\{}", prefix, name) };
                    out.push(path.clone());
                    let sub = r.u32()?;
                    walk(r, sub, &path, out)?;
                }
                Ok(())
            }
            // Index 0 is the ancestor directory itself; the tree below it is
            // numbered depth-first from 1. Getting this off by one puts every
            // referenced file in a sibling folder — which resolves to nothing,
            // and reads as "the pack does not have it".
            folders.push(String::new());
            walk(&mut r, n_root, "", &mut folders)?;
            for _ in 0..n_ext {
                let flags = if version >= 5 { r.u32()? } else { 0 };
                let name = if flags & 4 == 0 { r.string()? } else { format!("#resource{}", r.u32()?) };
                let node_index = r.u32()?;
                let use_file = if version >= 5 { r.u32()? != 0 } else { false };
                let folder = if flags & 4 == 0 { Some(r.u32()? as usize) } else { None };
                refs.push(ExtRef { name, node_index, folder, use_file });
            }
        }

        let body = if body_comp == b'C' {
            let uncomp = r.u32()? as usize;
            let csize = r.u32()? as usize;
            gbx::container::lzo_decompress(r.take(csize)?, uncomp)
        } else {
            r.take(r.left())?.to_vec()
        };

        Ok(Gbx { version, class_id, user_data, num_nodes, ancestor_level, folders, refs, body })
    }

    /// The full path an external reference names, as the pack spells paths:
    /// `<folder>\<name>`, with `..\` for each ancestor level. Resolution
    /// against a pack's file table is `names::resolve`'s job, not this one's.
    pub fn ref_path(&self, e: &ExtRef) -> String {
        let up = "..\\".repeat(self.ancestor_level as usize);
        match e.folder.and_then(|i| self.folders.get(i)) {
            Some(f) if !f.is_empty() => format!("{}{}\\{}", up, f, e.name),
            _ => format!("{}{}", up, e.name),
        }
    }
}
