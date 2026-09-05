//! Bake a uniform scale into COPIES of a prefab tree, in place.
//!
//! Why this exists: a `CGameItemModel` whose entity model is a `CPlugPrefab`
//! renders at the prefab's authored size whatever the placement's scale field
//! says (measured on Summer 2026 - 01: half-spaced, full-size terrain). The
//! archive items that do honour the field are mesh-modeler crystals, a format
//! this tool does not write. So instead of asking the game to scale, the
//! geometry is scaled before the game sees it: every position-like float the
//! walker meets — prefab entity positions, mesh vertex positions, collision
//! vertices, bounding boxes, compound transforms — is multiplied, and nothing
//! else in the file moves. Normals, UVs, materials, indices and every chunk
//! the walker does not understand are byte-identical to the original.
//!
//! The walker (`node::Graph`) records the offsets as it reads (`Reader::marks`),
//! so a layout it cannot follow is a hard refusal here rather than a mesh
//! quietly left at full size: the only recovery accepted is the tail of a
//! `CPlugVertexStream` after its positions, which is a leaf node whose
//! remaining content is normals and UVs.
//!
//! Files a prefab references that carry geometry of their own are rescaled
//! recursively and renamed in the copy's reference table, so a whole tree
//! resolves to scaled copies while textures and materials keep pointing at
//! the game's own files.

use crate::store::{DataStore, Model};
use std::collections::BTreeMap;

/// The recovery the walker reports for a vertex stream's post-position tail.
const VERTEX_TAIL: &str = "CPlugVertexStream layout after Position";

/// File kinds that carry geometry and therefore need a scaled copy of their
/// own when referenced.
const GEOMETRY_KINDS: &[&str] = &[
    ".Prefab.Gbx",
    ".StaticObject.Gbx",
    ".Mesh.Gbx",
    ".Shape.Gbx",
    ".Solid2Model.Gbx",
    ".DynaObject.Gbx",
    ".HitShape.Gbx",
];

pub fn is_geometry(name: &str) -> bool {
    let u = name.to_ascii_uppercase();
    GEOMETRY_KINDS.iter().any(|k| u.ends_with(&k.to_ascii_uppercase()))
}

/// `Zone\Land\Base.Prefab.Gbx` + `_half` -> `Zone\Land\Base_half.Prefab.Gbx`.
pub fn renamed(logical: &str, suffix: &str) -> String {
    let (dir, file) = match logical.rfind('\\') {
        Some(i) => (&logical[..=i], &logical[i + 1..]),
        None => ("", logical),
    };
    let stem_end = file.find('.').unwrap_or(file.len());
    format!("{dir}{}{suffix}{}", &file[..stem_end], &file[stem_end..])
}

fn file_name(logical: &str) -> &str {
    match logical.rfind('\\') {
        Some(i) => &logical[i + 1..],
        None => logical,
    }
}

pub struct Report {
    pub logical: String,
    pub out: String,
    pub floats: usize,
    pub marks: usize,
    pub nested: usize,
}

pub struct Rescale {
    pub factor: f32,
    pub suffix: String,
    /// Logical pack path of each scaled copy -> its complete GBX bytes.
    pub files: BTreeMap<String, Vec<u8>>,
    pub reports: Vec<Report>,
}

impl Rescale {
    pub fn new(factor: f32, suffix: &str) -> Rescale {
        assert!(factor.is_finite() && factor > 0.0, "scale factor must be positive");
        Rescale { factor, suffix: suffix.to_string(), files: BTreeMap::new(), reports: Vec::new() }
    }

    /// Scale one pack file (and, recursively, the geometry files it names).
    /// Returns the logical path of the scaled copy.
    pub fn file(&mut self, store: &mut DataStore, logical: &str) -> Result<String, String> {
        let out_name = renamed(logical, &self.suffix);
        if self.files.contains_key(&out_name) {
            return Ok(out_name);
        }
        let bytes = store.read(logical)?;
        let (body, marks, floats) = scaled_body(&bytes, logical, self.factor)?;

        let model = Model::parse(&bytes, logical)?;
        let mut renames: BTreeMap<String, String> = BTreeMap::new();
        for (_, ext) in &model.externals {
            if is_geometry(ext) {
                let new = self.file(store, ext)?;
                renames.insert(file_name(ext).to_string(), file_name(&new).to_string());
            }
        }

        let mut g = tmmaps::gbx::Gbx::parse(&bytes);
        if g.body.len() != body.len() {
            return Err(format!(
                "{logical}: container readers disagree on body length ({} vs {})",
                g.body.len(),
                body.len()
            ));
        }
        if !renames.is_empty() {
            g.ref_table = rename_refs(&g.ref_table, g.version, &renames)?;
        }
        let out = g.write_body_recompressed(&body);
        self.reports.push(Report {
            logical: logical.to_string(),
            out: out_name.clone(),
            floats,
            marks,
            nested: renames.len(),
        });
        self.files.insert(out_name.clone(), out);
        Ok(out_name)
    }
}

/// Walk a file, multiply every marked float, return the new body. Refuses any
/// layout recovery other than the vertex-stream tail, and any packed position
/// stream, because both would leave geometry at full size.
pub fn scaled_body(bytes: &[u8], logical: &str, factor: f32) -> Result<(Vec<u8>, usize, usize), String> {
    let model = Model::parse(bytes, logical)?;
    let graph = model.graph()?;
    for r in &graph.recovered {
        if r != VERTEX_TAIL {
            return Err(format!("{logical}: walker recovered past an unknown layout ({r}); geometry may be missed"));
        }
    }
    let mut body = model.body.clone();
    let mut floats = 0usize;
    for (off, n) in &graph.r.marks {
        if *off == usize::MAX {
            return Err(format!("{logical}: a vertex stream stores packed positions, which cannot be rescaled in place"));
        }
        let end = off + 4 * n;
        if end > body.len() {
            return Err(format!("{logical}: mark {off}+{n} floats past the body"));
        }
        for k in 0..*n {
            let o = off + 4 * k;
            let v = f32::from_le_bytes(body[o..o + 4].try_into().unwrap());
            body[o..o + 4].copy_from_slice(&(v * factor).to_le_bytes());
        }
        floats += n;
    }
    Ok((body, graph.r.marks.len(), floats))
}

/// Re-check a scaled copy against its source: the same walk must find the
/// same marks at the same offsets, each float `factor` times the original.
pub fn verify(orig: &[u8], scaled: &[u8], logical: &str, factor: f32) -> Result<usize, String> {
    let a = Model::parse(orig, logical)?;
    let b = Model::parse(scaled, logical)?;
    let ga = a.graph()?;
    let gb = b.graph()?;
    if ga.r.marks != gb.r.marks {
        return Err(format!("{logical}: scaled copy walks differently ({} vs {} marks)", ga.r.marks.len(), gb.r.marks.len()));
    }
    if ga.seen != gb.seen {
        return Err(format!("{logical}: scaled copy has a different chunk census"));
    }
    let mut checked = 0;
    for (off, n) in &ga.r.marks {
        for k in 0..*n {
            let o = off + 4 * k;
            let x = f32::from_le_bytes(a.body[o..o + 4].try_into().unwrap());
            let y = f32::from_le_bytes(b.body[o..o + 4].try_into().unwrap());
            if (y - x * factor).abs() > 1e-4 * x.abs().max(1.0) {
                return Err(format!("{logical}: float at 0x{o:x} is {y}, expected {}", x * factor));
            }
            checked += 1;
        }
    }
    Ok(checked)
}

/// Rewrite the file names inside a GBX reference table. Folder names and
/// every other word are copied through unchanged.
pub fn rename_refs(table: &[u8], version: u16, renames: &BTreeMap<String, String>) -> Result<Vec<u8>, String> {
    let mut r = tmmaps::gbx::Reader::new(table);
    let mut out = Vec::new();
    let n = r.u32();
    out.extend_from_slice(&n.to_le_bytes());
    if n == 0 {
        return Ok(out);
    }
    out.extend_from_slice(&r.u32().to_le_bytes()); // ancestor level
    fn folders(r: &mut tmmaps::gbx::Reader, out: &mut Vec<u8>, count: u32) {
        for _ in 0..count {
            let name = r.string();
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            let sub = r.u32();
            out.extend_from_slice(&sub.to_le_bytes());
            folders(r, out, sub);
        }
    }
    let nf = r.u32();
    out.extend_from_slice(&nf.to_le_bytes());
    folders(&mut r, &mut out, nf);
    let mut hits = 0;
    for _ in 0..n {
        let flags = r.u32();
        out.extend_from_slice(&flags.to_le_bytes());
        if flags & 4 == 0 {
            let name = r.string();
            let new = renames.get(&name).cloned().unwrap_or_else(|| name.clone());
            if new != name {
                hits += 1;
            }
            out.extend_from_slice(&(new.len() as u32).to_le_bytes());
            out.extend_from_slice(new.as_bytes());
        } else {
            out.extend_from_slice(&r.u32().to_le_bytes());
        }
        out.extend_from_slice(&r.u32().to_le_bytes()); // node index
        if version >= 5 {
            out.extend_from_slice(&r.u32().to_le_bytes()); // use file
        }
        if flags & 4 == 0 {
            out.extend_from_slice(&r.u32().to_le_bytes()); // folder index
        }
    }
    if hits != renames.len() {
        return Err(format!("reference table renamed {hits} of {} geometry files", renames.len()));
    }
    Ok(out)
}
