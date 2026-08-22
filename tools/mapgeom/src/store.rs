//! Where the game's files come from: one or more NadeoPaks, opened once and
//! indexed, with hashed names resolved on demand.
//!
//! A block model reaches its geometry by *file name*, and most of the files it
//! names are stored under a hash of some suffix of their path (`names.rs`). So
//! every load goes through `resolve`, and a load that fails says which
//! candidates it tried — a missing prefab is then a fact about the pack, not a
//! silent empty mesh.

use crate::container::Gbx;
use crate::names;
use crate::node::Graph;
use crate::pak::{read_pak, Pak, PakEntry};
use crate::pakfile::read_file;
use std::collections::HashMap;

/// The Stadium pack's encryption key. It is a property of the pack file, not
/// of a session or a machine: the same key opens the same bytes anywhere, and
/// it only changes if Nadeo re-packs the data. Recovered from a running
/// server's memory (see `MAPGEOM.md`); it is NOT in the shipped binary.
pub const STADIUM_KEY: &str = "870FBE770EE4909C714B18B04D914C17";

pub struct OpenPak {
    data: Vec<u8>,
    pak: Pak,
    header_max_size: usize,
    key: [u8; 16],
}

pub struct DataStore {
    paks: Vec<OpenPak>,
    /// UPPERCASE path -> (pak index, entry index).
    index: HashMap<String, (usize, usize)>,
    cache: HashMap<String, Option<Vec<u8>>>,
}

fn parse_key(hex: &str) -> Result<[u8; 16], String> {
    if hex.len() != 32 {
        return Err(format!("key must be 32 hex characters, got {}", hex.len()));
    }
    let mut k = [0u8; 16];
    for i in 0..16 {
        k[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(k)
}

impl DataStore {
    pub fn open(paths: &[String], key_hex: &str) -> Result<DataStore, String> {
        let key = parse_key(key_hex)?;
        let mut store = DataStore { paks: Vec::new(), index: HashMap::new(), cache: HashMap::new() };
        for p in paths {
            let data = std::fs::read(p).map_err(|e| format!("{}: {}", p, e))?;
            if data.len() < 0x95 || &data[0..8] != b"NadeoPak" {
                return Err(format!("{}: not a NadeoPak", p));
            }
            let version = i32::from_le_bytes(data[8..12].try_into().unwrap());
            let header_max_size =
                u32::from_le_bytes(data[0x30..0x34].try_into().unwrap()) as usize;
            let pak = read_pak(&data, 0x8D, version, &key);
            let pi = store.paks.len();
            for (ei, e) in pak.entries.iter().enumerate() {
                store.index.entry(e.path().to_uppercase()).or_insert((pi, ei));
            }
            store.paks.push(OpenPak { data, pak, header_max_size, key });
        }
        Ok(store)
    }

    pub fn entries(&self) -> impl Iterator<Item = &PakEntry> {
        self.paks.iter().flat_map(|p| p.pak.entries.iter())
    }

    /// Which pack path a logical path is stored under, if any.
    pub fn resolve(&self, logical: &str) -> Option<String> {
        for cand in names::candidates(logical) {
            if self.index.contains_key(&cand.to_uppercase()) {
                return Some(cand);
            }
        }
        None
    }

    /// Read a file by logical path, resolving the hash if needed.
    pub fn read(&mut self, logical: &str) -> Result<Vec<u8>, String> {
        let key = logical.to_uppercase();
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone().ok_or_else(|| format!("{}: not in any pack", logical));
        }
        let resolved = self.resolve(logical);
        let out = match resolved {
            None => None,
            Some(path) => {
                let (pi, ei) = self.index[&path.to_uppercase()];
                let p = &self.paks[pi];
                Some(read_file(&p.data, p.header_max_size, &p.pak.entries[ei], &p.key, p.pak.version)?)
            }
        };
        self.cache.insert(key, out.clone());
        out.ok_or_else(|| {
            format!(
                "{}: not in any pack (tried {})",
                logical,
                names::candidates(logical).join(", ")
            )
        })
    }

    /// Load a GBX file and walk its body into a node graph, with the file's
    /// own external references already registered so a ref to another file
    /// comes back as `Slot::External(<resolved logical path>)`.
    pub fn load_model(&mut self, logical: &str) -> Result<Model, String> {
        let bytes = self.read(logical)?;
        Model::parse(&bytes, logical)
    }
}

/// One parsed GBX file: its container, its node graph, and the logical paths
/// of the files it references.
pub struct Model {
    pub path: String,
    pub class_id: u32,
    pub body: Vec<u8>,
    pub num_nodes: u32,
    /// node index -> logical path of the referenced file
    pub externals: Vec<(u32, String)>,
}

impl Model {
    pub fn parse(bytes: &[u8], logical: &str) -> Result<Model, String> {
        let g = Gbx::parse(bytes).map_err(|e| format!("{}: {}", logical, e))?;
        let folder = match logical.rfind('\\') {
            Some(i) => &logical[..i],
            None => "",
        };
        let externals = g
            .refs
            .iter()
            .map(|e| (e.node_index, names::join(folder, &g.ref_path(e))))
            .collect();
        Ok(Model {
            path: logical.to_string(),
            class_id: g.class_id,
            body: g.body,
            num_nodes: g.num_nodes,
            externals,
        })
    }

    pub fn graph(&self) -> Result<Graph<'_>, String> {
        Graph::parse(&self.body, self.class_id, self.num_nodes, &self.externals)
            .map_err(|e| format!("{}: {}", self.path, e))
    }

    /// The external references whose name ends in `suffix` (case-insensitive).
    pub fn refs_ending(&self, suffix: &str) -> Vec<String> {
        let s = suffix.to_uppercase();
        self.externals
            .iter()
            .filter(|(_, p)| p.to_uppercase().ends_with(&s))
            .map(|(_, p)| p.clone())
            .collect()
    }
}
