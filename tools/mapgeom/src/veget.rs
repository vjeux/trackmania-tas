//! The procedural vegetation models (`.VegetTreeModel.Gbx`, class
//! 0x2F086000 `CPlugVegetTreeModel`): a table-less struct the node walker
//! cannot read, decoded here field by field (layout read off the packs,
//! 2026-09-08 — 191 of the 193 species files of the five collections parse
//! to the byte). Its meshes are ordinary `CPlugVisualIndexedTriangles` nodes
//! written inline, grouped by detail level; its materials are NOT pack
//! `.Material.Gbx` files but inline (name, texture triples, a leaf flag) —
//! the vegetation renderer has its own shaders; and it carries the trunk's
//! collision hull as plain arrays.
//!
//! Body layout (version 21):
//! ```text
//! u32 version (21)      u32 h1 (3|4)      u32 lod_count      u32 lod_count-1
//! u32 material_count
//! material × n: [i32;3] `.Texture.gbx` refs (D,N,R; -1 = none),
//!                [i32;3] `.dds` refs (D,N,R), [i32;3] (unused, -1), u8 leaf
//! u32 material_count (again)   u32 3
//! material × n: f32 (2.0)   string name
//! lod group × lod_count: u32 visual_count;
//!     visual × count: u16 material index, u32 node index, u32 class
//!         0x0901E000, CPlugVisualIndexedTriangles body (to FACADE), u8 (0)
//! f32 × (lod_count-1) switch distances (metres, the model's own; 50, 100)
//! u8 (1)   f32 far distance (100 | 150)   u8 (3)   u8 (2)   u64 FILETIME
//! f32 (1.0)   f32 (0.1)   u32   u32 (1)   u32 (7)   u32 (7)
//! u32 hull vertex count, vec3 × n      u32 hull triangle count,
//!     (u32 a, u32 b, u32 c, u32 surface material id — 14 Wood) × n
//! … wind / impostor parameters (kept raw)
//! ```

use crate::static_item::visual::CPlugVisualIndexedTriangles;
use crate::static_item::{LookbackState, Rd, R};
use crate::store::DataStore;

pub const CLASS_VEGET_TREE_MODEL: u32 = 0x2F086000;
const CLASS_VISUAL_INDEXED_TRIANGLES: u32 = 0x0901E000;

#[derive(Clone, Debug)]
pub struct VisualStats {
    pub vertices: usize,
    /// Centre xyz, half extents xyz (the chunk's own words).
    pub bbox: [f32; 6],
}

#[derive(Clone, Debug)]
pub struct TreeStats {
    pub visuals: Vec<VisualStats>,
    /// Lowest / highest y over every visual's box (metres, model space).
    pub bottom: f32,
    pub top: f32,
    /// The widest horizontal half extent.
    pub radius: f32,
}

/// One inline material of a tree model.
#[derive(Clone, Debug)]
pub struct VegetMaterial {
    pub name: String,
    /// `.Texture.gbx` wrapper paths (D, N, R), when referenced.
    pub texture_nodes: [Option<String>; 3],
    /// `.dds` image paths (D, N, R), when referenced.
    pub images: [Option<String>; 3],
    /// The third reference triple (never seen set).
    pub extra: [i32; 3],
    /// The byte after the triples: 1 on every `*_Leaf` material, 0 on bark —
    /// the two-sided, alpha-tested foliage shader.
    pub leaf: bool,
    /// The float before the name (2.0 on every file read).
    pub f: f32,
}

#[derive(Clone, Debug)]
pub struct VegetLodEntry {
    pub material: u16,
    pub node_index: u32,
    pub visual: CPlugVisualIndexedTriangles,
    /// The byte after the visual (0 on every file read).
    pub flag: u8,
}

#[derive(Clone, Debug)]
pub struct VegetTreeModel {
    pub version: u32,
    pub h1: u32,
    pub materials: Vec<VegetMaterial>,
    /// Detail levels, nearest first.
    pub lods: Vec<Vec<VegetLodEntry>>,
    /// Level k draws while the camera is within `switch[k]`; one fewer than
    /// the levels.
    pub switch: Vec<f32>,
    /// Past this the mesh is culled (the game's impostor takes over).
    pub far: f32,
    pub file_write_time: u64,
    pub hull_vertices: Vec<[f32; 3]>,
    /// (indices, surface material id).
    pub hull_triangles: Vec<([u32; 3], u32)>,
    /// Body offset where the raw tail begins, and the tail itself.
    pub tail_at: usize,
    pub tail: Vec<u8>,
}

impl VegetTreeModel {
    /// Lowest / highest y and widest horizontal half extent over the visuals
    /// of the nearest level (model metres).
    pub fn stats(&self) -> TreeStats {
        let mut visuals = Vec::new();
        for lod in &self.lods {
            for e in lod {
                if let Some(m) = &e.visual.main {
                    visuals.push(VisualStats { vertices: m.count.max(0) as usize, bbox: m.bounding_box });
                }
            }
        }
        stats_of(visuals)
    }
}

fn stats_of(visuals: Vec<VisualStats>) -> TreeStats {
    let mut bottom = f32::MAX;
    let mut top = f32::MIN;
    let mut radius = 0.0f32;
    for v in &visuals {
        let [cx, cy, cz, hx, hy, hz] = v.bbox;
        bottom = bottom.min(cy - hy);
        top = top.max(cy + hy);
        radius = radius.max((cx.abs() + hx).max(cz.abs() + hz));
    }
    if visuals.is_empty() {
        bottom = 0.0;
        top = 0.0;
    }
    TreeStats { visuals, bottom, top, radius }
}

/// The VegetTreeModel behind a path: the path itself, or the model an
/// `.Item.Gbx` references (`Stadium\Items\PalmTreeSmall.Item.Gbx`; a
/// variant-list item such as `PalmForest` names many — the first is taken,
/// `tree_model_paths` lists them all).
pub fn tree_model_path(store: &mut DataStore, path: &str) -> Result<String, String> {
    tree_model_paths(store, path)?.into_iter().next().ok_or_else(|| format!("{path}: no VegetTreeModel reference"))
}

/// Every VegetTreeModel an item references, in reference order (the order
/// the placement's variant byte indexes).
pub fn tree_model_paths(store: &mut DataStore, path: &str) -> Result<Vec<String>, String> {
    if path.to_ascii_lowercase().ends_with(".vegettreemodel.gbx") {
        return Ok(vec![path.to_string()]);
    }
    let m = store.load_model(path)?;
    Ok(m.externals.iter().map(|(_, p)| p.clone()).filter(|n| n.to_ascii_lowercase().ends_with(".vegettreemodel.gbx")).collect())
}

/// The model file's decoded body and its externals, whole or — when the
/// decode stops short — as far as it goes (`MAPGEOM_LENIENT_LZ4`).
fn load_body(store: &mut DataStore, model_path: &str) -> Result<(Vec<u8>, Vec<(u32, String)>, bool), String> {
    match store.load_model(model_path) {
        Ok(m) => Ok((m.body, m.externals, true)),
        Err(e) => {
            std::env::set_var("MAPGEOM_LENIENT_LZ4", "1");
            let r = store.load_model(model_path);
            std::env::remove_var("MAPGEOM_LENIENT_LZ4");
            let m = r.map_err(|e2| format!("{e}; partial read: {e2}"))?;
            Ok((m.body, m.externals, false))
        }
    }
}

/// The whole tree model, typed. Errors name the field that failed.
pub fn parse_tree_model(store: &mut DataStore, path: &str) -> Result<VegetTreeModel, String> {
    let model_path = tree_model_path(store, path)?;
    let (body, externals, whole) = load_body(store, &model_path)?;
    parse_body(&body, &externals).map_err(|e| format!("{model_path}: {e}{}", if whole { "" } else { " (the file decoded only partly)" }))
}

fn parse_body(body: &[u8], externals: &[(u32, String)]) -> R<VegetTreeModel> {
    let ext_indices: Vec<u32> = externals.iter().map(|(i, _)| *i).collect();
    let name_of = |i: i32| -> Option<String> { if i < 0 { None } else { externals.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone()) } };
    let mut lb = LookbackState::default();
    let mut r = Rd::new(body, 0, lb.clone());
    let version = r.u32()?;
    if version != 21 {
        return Err(format!("VegetTreeModel version {version} (21 expected)"));
    }
    let h1 = r.u32()?;
    let lod_count = r.u32()? as usize;
    let switch_count = r.u32()? as usize;
    if lod_count == 0 || lod_count > 8 || switch_count + 1 != lod_count {
        return Err(format!("{lod_count} detail levels with {switch_count} switch distances"));
    }
    let material_count = r.u32()? as usize;
    if material_count == 0 || material_count > 64 {
        return Err(format!("{material_count} materials"));
    }
    let mut mats: Vec<([i32; 3], [i32; 3], [i32; 3], bool)> = Vec::with_capacity(material_count);
    for _ in 0..material_count {
        let mut t = [[0i32; 3]; 3];
        for slot in t.iter_mut() {
            for x in slot.iter_mut() {
                *x = r.i32()?;
            }
        }
        let leaf = r.u8()?;
        mats.push((t[0], t[1], t[2], leaf != 0));
    }
    let n2 = r.u32()? as usize;
    if n2 != material_count {
        return Err(format!("second material count {n2} != {material_count}"));
    }
    let _three = r.u32()?;
    let mut materials = Vec::with_capacity(material_count);
    for (t0, t1, t2, leaf) in mats {
        let f = r.f32()?;
        let name = r.string()?;
        let tex = |t: [i32; 3]| -> [Option<String>; 3] { [name_of(t[0]), name_of(t[1]), name_of(t[2])] };
        materials.push(VegetMaterial { name, texture_nodes: tex(t0), images: tex(t1), extra: t2, leaf, f });
    }
    // the body's lookback version word went by with the struct's own strings
    lb.version_seen = true;
    lb.defined_nodes.extend(ext_indices.iter().copied());
    let mut lods = Vec::with_capacity(lod_count);
    for l in 0..lod_count {
        let count = r.u32()? as usize;
        if count > 256 {
            return Err(format!("level {l}: {count} visuals"));
        }
        let mut entries = Vec::with_capacity(count);
        for k in 0..count {
            let material = r.u16()?;
            if material as usize >= material_count {
                return Err(format!("level {l} visual {k}: material {material} of {material_count}"));
            }
            let node_index = r.u32()?;
            let class = r.u32()?;
            if class != CLASS_VISUAL_INDEXED_TRIANGLES {
                return Err(format!("level {l} visual {k} at {:#x}: class {class:#010x} is not CPlugVisualIndexedTriangles", r.o - 8));
            }
            let mut vr = Rd::new(body, r.o, lb.clone());
            let visual = CPlugVisualIndexedTriangles::parse(&mut vr).map_err(|e| format!("level {l} visual {k} (node {node_index}): {e}"))?;
            lb = vr.lb.clone();
            r.o = vr.o;
            lb.defined_nodes.insert(node_index);
            let flag = r.u8()?;
            entries.push(VegetLodEntry { material, node_index, visual, flag });
        }
        lods.push(entries);
    }
    if std::env::var_os("MAPGEOM_VEGET_LAYOUT").is_some() {
        let end = (r.o + 240).min(body.len());
        eprintln!("  after the visuals at {:#x} ({} bytes left): {}", r.o, body.len() - r.o, body[r.o..end].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
    }
    let mut switch = Vec::with_capacity(switch_count);
    for _ in 0..switch_count {
        switch.push(r.f32()?);
    }
    let _one = r.u8()?;
    let far = r.f32()?;
    let _three_b = r.u8()?;
    let _two = r.u8()?;
    let file_write_time = r.u64()?;
    let _f1 = r.f32()?;
    let _f01 = r.f32()?;
    let _a = r.u32()?;
    let _b = r.u32()?;
    // the hull: kind 7 = a mesh (then a second 7, the vertices, the triangles);
    // -1 = none (grass, flowers, the small cacti and some bushes)
    let hull_kind = r.i32()?;
    let mut hull_vertices = Vec::new();
    let mut hull_triangles = Vec::new();
    let (nv, nt) = if hull_kind < 0 {
        (0usize, 0usize)
    } else {
        if hull_kind != 7 {
            return Err(format!("hull kind {hull_kind}"));
        }
        let _d = r.u32()?;
        let nv = r.u32()? as usize;
        if nv > 100_000 {
            return Err(format!("hull: {nv} vertices"));
        }
        for _ in 0..nv {
            hull_vertices.push(r.vec3()?);
        }
        let nt = r.u32()? as usize;
        if nt > 200_000 {
            return Err(format!("hull: {nt} triangles"));
        }
        (nv, nt)
    };
    for _ in 0..nt {
        let a = r.u32()?;
        let b = r.u32()?;
        let c = r.u32()?;
        let mat = r.u32()?;
        if a as usize >= nv || b as usize >= nv || c as usize >= nv {
            return Err(format!("hull triangle ({a}, {b}, {c}) past {nv} vertices"));
        }
        hull_triangles.push(([a, b, c], mat));
    }
    let tail_at = r.o;
    let tail = body[tail_at.min(body.len())..].to_vec();
    Ok(VegetTreeModel { version, h1, materials, lods, switch, far, file_write_time, hull_vertices, hull_triangles, tail_at, tail })
}

/// Every inline `CPlugVisualIndexedTriangles` of a tree model file and the
/// union of their boxes. The typed parse first; a file that only decodes
/// partly falls back to scanning for the visuals' class id (the LOD boxes of
/// a species agree to centimetres, so a prefix is enough for a height).
pub fn tree_model_stats(store: &mut DataStore, path: &str) -> Result<TreeStats, String> {
    let model_path = tree_model_path(store, path)?;
    let (body, externals, _) = load_body(store, &model_path)?;
    if let Ok(m) = parse_body(&body, &externals) {
        return Ok(m.stats());
    }
    let ext_indices: Vec<u32> = externals.iter().map(|(i, _)| *i).collect();
    let mut visuals = Vec::new();
    let mut i = 0usize;
    while i + 4 <= body.len() {
        let cid = u32::from_le_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]);
        if cid == CLASS_VISUAL_INDEXED_TRIANGLES {
            let mut lb = LookbackState::default();
            lb.version_seen = true;
            lb.defined_nodes.extend(ext_indices.iter().copied());
            let mut r = Rd::new(&body, i + 4, lb);
            if let Ok(v) = CPlugVisualIndexedTriangles::parse(&mut r) {
                if let Some(m) = &v.main {
                    visuals.push(VisualStats { vertices: m.count.max(0) as usize, bbox: m.bounding_box });
                    i = r.o;
                    continue;
                }
            }
        }
        i += 1;
    }
    if visuals.is_empty() {
        return Err(format!("{model_path}: no visual parsed ({} body bytes)", body.len()));
    }
    Ok(stats_of(visuals))
}

/// What the leaf cards' vertex data encodes — the questions of the 2026-09-09
/// "trees look horrible" thread, answered from the model rather than guessed:
/// are the normals the cards' own face normals (flat paper) or a shell round
/// the crown (the sphere the vegetation shader lights), and does the one
/// varying byte of the vertex colour follow the radius (ambient occlusion:
/// bright outside, dark inside) or the height (a wind weight)?
/// One line per visual: material, vertex count, mean cos(normal, face normal),
/// mean cos(normal, vertex − crown centre), and the correlation of the colour
/// byte with the radial distance and with the height.
pub fn shape_report(m: &VegetTreeModel) -> Vec<String> {
    use crate::static_item::vstream::{Elem, T_DEC3N, T_FLOAT3};
    let mut out = Vec::new();
    for (l, lod) in m.lods.iter().enumerate() {
        for e in lod {
            let Some(main) = e.visual.main.as_ref() else { continue };
            let Some(s) = e.visual.stream() else { continue };
            let compress = s.compress_local3d.unwrap_or(false);
            let mut pos: Vec<[f32; 3]> = Vec::new();
            let mut nrm: Vec<[f32; 3]> = Vec::new();
            let mut col: Vec<u32> = Vec::new();
            for (d, el) in s.decls.iter().zip(s.elems.iter()) {
                match (d.name(), d.stored_type(compress), el) {
                    (0, T_FLOAT3, Elem::Float3(p)) => pos = p.clone(),
                    (5, T_DEC3N, Elem::Word(w)) => nrm = w.iter().map(|x| crate::static_item::build::dec3n_unpack(*x)).collect(),
                    (5, T_FLOAT3, Elem::Float3(p)) => nrm = p.clone(),
                    (8, _, Elem::Word(w)) => col = w.clone(),
                    _ => {}
                }
            }
            let n = pos.len();
            if n == 0 || nrm.len() != n {
                continue;
            }
            let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let norm = |a: [f32; 3]| {
                let l = dot(a, a).sqrt();
                if l > 1e-6 { [a[0] / l, a[1] / l, a[2] / l] } else { [0.0; 3] }
            };
            // the crown centre: the box centre of this visual
            let c = [main.bounding_box[0], main.bounding_box[1], main.bounding_box[2]];
            // per-vertex face normal: the area-weighted mean of the triangles that use it
            let mut face = vec![[0.0f32; 3]; n];
            if let Some(ib) = e.visual.index_buffer.as_ref() {
                for t in ib.indices.chunks(3) {
                    if t.len() < 3 {
                        break;
                    }
                    let (a, b, cc) = (t[0] as usize, t[1] as usize, t[2] as usize);
                    if a >= n || b >= n || cc >= n {
                        continue;
                    }
                    let u = [pos[b][0] - pos[a][0], pos[b][1] - pos[a][1], pos[b][2] - pos[a][2]];
                    let v = [pos[cc][0] - pos[a][0], pos[cc][1] - pos[a][1], pos[cc][2] - pos[a][2]];
                    let fnrm = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                    for k in [a, b, cc] {
                        for i in 0..3 {
                            face[k][i] += fnrm[i];
                        }
                    }
                }
            }
            let mut cos_face = 0.0f32;
            let mut abs_cos_face = 0.0f32;
            let mut cos_shell = 0.0f32;
            let mut cnt = 0usize;
            for i in 0..n {
                let fn_ = norm(face[i]);
                if dot(fn_, fn_) < 0.5 {
                    continue;
                }
                let nn = norm(nrm[i]);
                let r = norm([pos[i][0] - c[0], pos[i][1] - c[1], pos[i][2] - c[2]]);
                cos_face += dot(nn, fn_);
                abs_cos_face += dot(nn, fn_).abs();
                cos_shell += dot(nn, r);
                cnt += 1;
            }
            let k = cnt.max(1) as f32;
            // the colour byte that varies (the others read 0xff on every file): each of
            // the four against the radial distance and the height
            let mut colinfo = String::new();
            if col.len() == n {
                for byte in 0..4 {
                    let vals: Vec<f32> = col.iter().map(|w| ((w >> (8 * byte)) & 0xff) as f32).collect();
                    let mean = vals.iter().sum::<f32>() / n as f32;
                    let var = vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / n as f32;
                    if var < 1.0 {
                        continue;
                    }
                    let corr = |xs: &[f32]| {
                        let mx = xs.iter().sum::<f32>() / n as f32;
                        let vx = xs.iter().map(|v| (v - mx) * (v - mx)).sum::<f32>() / n as f32;
                        let cov = xs.iter().zip(vals.iter()).map(|(x, v)| (x - mx) * (v - mean)).sum::<f32>() / n as f32;
                        if vx > 1e-6 { cov / (vx.sqrt() * var.sqrt()) } else { 0.0 }
                    };
                    let radial: Vec<f32> = pos.iter().map(|p| ((p[0] - c[0]).powi(2) + (p[2] - c[2]).powi(2)).sqrt()).collect();
                    let dist3: Vec<f32> = pos.iter().map(|p| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt()).collect();
                    let height: Vec<f32> = pos.iter().map(|p| p[1]).collect();
                    let lo = vals.iter().cloned().fold(255.0f32, f32::min);
                    colinfo.push_str(&format!("; colour byte {byte}: mean {mean:.0} min {lo:.0} sd {:.0}, corr radial {:+.2} dist3 {:+.2} height {:+.2}", var.sqrt(), corr(&radial), corr(&dist3), corr(&height)));
                }
            }
            let mat = m.materials.get(e.material as usize).map(|x| x.name.as_str()).unwrap_or("?");
            out.push(format!("  level {l} {mat}{}: {n} v; normal·face {:+.2} (|·| {:.2}), normal·shell {:+.2}{colinfo}", if m.materials.get(e.material as usize).map(|x| x.leaf).unwrap_or(false) { " (leaf)" } else { "" }, cos_face / k, abs_cos_face / k, cos_shell / k));
        }
    }
    out
}
