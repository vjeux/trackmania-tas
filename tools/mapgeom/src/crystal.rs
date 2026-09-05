//! Turn geometry into a Trackmania item the game will embed, render, and
//! SCALE: a mesh-modeler crystal (`CGameCommonItemEntityModelEdition` holding
//! a `CPlugCrystal`), the only item kind we found the game accepts from a map's
//! embedded ZIP with a placement scale honoured.
//!
//! The item is written around a TEMPLATE — a known-good crystal item — whose
//! bytes are kept for everything but the crystal itself: the header, the
//! collector chunks, the item-model chunks after the entity model. Inside the
//! entity model the material list (0x09003003), the geometry layer (0x09003005),
//! the lightmap coords (0x09003006) and the smoothing groups (0x09003007) are
//! re-emitted from `CrystalMesh`, at the template's own versions (crystal
//! version 37, geometry layer version 1), which were read off the template
//! field by field before this was written.
//!
//! Inline node numbering: the template's materials are consecutive inline
//! nodes; the emitted list is padded (by repeating the last material) to the
//! template's count so every later node index in the file stays valid.

use crate::tiny_assets::put_string;
use std::collections::HashMap;
use tmmaps::gbx::Gbx;

#[derive(Clone, Debug)]
pub struct Face {
    /// Corner indices into `positions`, 3 or more, counter-clockwise.
    pub verts: Vec<u32>,
    /// One texture coordinate per corner.
    pub uvs: Vec<[f32; 2]>,
    pub material: u32,
}

#[derive(Clone, Debug, Default)]
pub struct CrystalMesh {
    pub positions: Vec<[f32; 3]>,
    pub faces: Vec<Face>,
}

#[derive(Clone, Debug)]
pub struct MaterialSpec {
    /// The game material to link, e.g. `Stadium\Media\Material\RoadTech`.
    pub link: String,
    /// Surface physics id the car feels (Asphalt = 16).
    pub physics: u8,
}

impl CrystalMesh {
    /// Squeeze every UV into a tiny patch around the texture centre: a flat
    /// colour per material. NOT collapsed to one point -- a zero-area UV
    /// mapping gives NaN tangents, and materials with a normal map (Dirt,
    /// Wood, Rock, Stone) then crash the game once a map has enough faces.
    pub fn flatten_uvs(&mut self) {
        for f in &mut self.faces {
            for uv in &mut f.uvs {
                // Centre of ONE checker cell, not (0.5,0.5) where four meet.
                *uv = [0.25 + (uv[0] - 0.5) * 0.01, 0.25 + (uv[1] - 0.5) * 0.01];
            }
        }
    }

    /// Add triangles with their own vertex list; positions are de-duplicated
    /// exactly. UVs are BOX-mapped: each face projects onto the plane of its
    /// dominant normal axis, tiled every `uv_scale` metres, and v is squeezed
    /// into the texture's lit band (0.06..0.94 — Nadeo's RoadTech deck uses
    /// exactly that range; outside it the atlases are black).
    pub fn add_tris(&mut self, verts: &[[f32; 3]], tris: &[[u32; 3]], material: u32, uv_scale: f32) {
        let mut map: HashMap<[u32; 3], u32> = HashMap::new();
        for (i, p) in self.positions.iter().enumerate() {
            map.insert([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()], i as u32);
        }
        let mut index = |p: [f32; 3], positions: &mut Vec<[f32; 3]>| -> u32 {
            let k = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
            *map.entry(k).or_insert_with(|| {
                positions.push(p);
                (positions.len() - 1) as u32
            })
        };
        for t in tris {
            let ps = [verts[t[0] as usize], verts[t[1] as usize], verts[t[2] as usize]];
            if ps[0] == ps[1] || ps[1] == ps[2] || ps[0] == ps[2] {
                continue; // degenerate
            }
            let idx: Vec<u32> = ps.iter().map(|p| index(*p, &mut self.positions)).collect();
            let e1 = [ps[1][0] - ps[0][0], ps[1][1] - ps[0][1], ps[1][2] - ps[0][2]];
            let e2 = [ps[2][0] - ps[0][0], ps[2][1] - ps[0][1], ps[2][2] - ps[0][2]];
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
            let squeeze = |v: f32| 0.06 + 0.88 * v.rem_euclid(1.0);
            let uvs: Vec<[f32; 2]> = ps
                .iter()
                .map(|p| {
                    if ay >= ax && ay >= az {
                        [p[0] / uv_scale, squeeze(p[2] / uv_scale)]
                    } else if ax >= az {
                        [p[2] / uv_scale, squeeze(p[1] / uv_scale)]
                    } else {
                        [p[0] / uv_scale, squeeze(p[1] / uv_scale)]
                    }
                })
                .collect();
            self.faces.push(Face { verts: idx, uvs, material });
        }
    }
}

struct W(Vec<u8>);
impl W {
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        put_string(&mut self.0, s);
    }
    /// An index sized by what it indexes, the game's "optimized int".
    fn opt(&mut self, v: u32, determine_from: usize) {
        if determine_from >= 65535 {
            self.u32(v);
        } else if determine_from >= 255 {
            self.u16(v as u16);
        } else {
            self.u8(v as u8);
        }
    }
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

const FACADE: [u8; 4] = [0x01, 0xDE, 0xCA, 0xFA];

/// Offset of the Link string (its length word) inside a CPlugMaterialUserInst
/// node that starts at `node_start`: class id, chunk id, version, one byte,
/// two ids, the BaseTexture string, the physics byte, the gameplay byte.
fn link_offset(b: &[u8], node_start: usize) -> usize {
    let rd = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    assert_eq!(rd(node_start), 0x090FD000, "material node class");
    assert_eq!(rd(node_start + 4), 0x090FD000, "material node first chunk");
    let mut o = node_start + 8 + 4 + 1 + 4 + 4;
    o += 4 + rd(o) as usize; // BaseTexture
    o + 2
}


/// The template's shape, read once: where the crystal content starts and
/// ends inside the body, the material node pattern, and the layer id.
struct Template {
    body: Vec<u8>,
    num_nodes: u32,
    /// Offset of chunk id 0x09003003 (start of the replaced span).
    start: usize,
    /// Offset just past chunk 0x09003007's content (the crystal's FACADE).
    end: usize,
    /// First material node index and how many materials the template has.
    first_node: u32,
    n_materials: usize,
    /// Material node bytes before the Link string (physics id at [-2]) and after it.
    mat_prefix: Vec<u8>,
    mat_suffix: Vec<u8>,
    /// Chunk 0x09003004 verbatim (id, PIKS, size, data).
    chunk4: Vec<u8>,
    /// The visual levels (int, float) and the smoothing group floats.
    visual_levels: Vec<(u32, f32)>,
    smoothing: Vec<f32>,
    /// Non-geometry layers of the template (Trigger = 14, SpawnPosition = 15),
    /// each as its verbatim bytes: a waypoint item is nothing without them.
    extra_layers: Vec<Vec<u8>>,
    /// Verbatim spans of the template's chunks, for bisecting the writer.
    raw_003: Vec<u8>,
    raw_005: Vec<u8>,
    raw_006: Vec<u8>,
    raw_007: Vec<u8>,
}

fn parse_template(bytes: &[u8]) -> Template {
    let g = Gbx::parse(bytes);
    let b = g.body.clone();
    let start = find(&b, &0x09003003u32.to_le_bytes(), 0).expect("template has no material chunk");
    let rd = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    // materials chunk: id, version, count, then per material: string(""), node index, inline node ... FACADE
    let n_materials = rd(start + 8) as usize;
    let mut o = start + 12;
    let mut first_node = 0;
    let mut mat_prefix = Vec::new();
    let mut mat_suffix = Vec::new();
    for i in 0..n_materials {
        let slen = rd(o) as usize;
        assert_eq!(slen, 0, "template material {i} has an inline name; only node materials are handled");
        o += 4;
        let node = rd(o);
        o += 4;
        if i == 0 {
            first_node = node;
        } else {
            assert_eq!(node, first_node + i as u32, "template material nodes are not consecutive");
        }
        let node_start = o; // class id follows
        let fac = find(&b, &FACADE, o).expect("material node end") + 4;
        if i == 0 {
            let link_at = link_offset(&b, node_start);
            let l = rd(link_at) as usize;
            mat_prefix = b[node_start..link_at].to_vec();
            mat_suffix = b[link_at + 4 + l..fac].to_vec();
        }
        o = fac;
    }
    // chunk 0x09003004 (skippable): id, PIKS, size, data
    assert_eq!(rd(o), 0x09003004, "expected chunk 0x09003004 after materials");
    let c4 = o;
    let sz = rd(o + 8) as usize;
    let chunk4 = b[o..o + 12 + sz].to_vec();
    o += 12 + sz;
    let c5 = o;
    assert_eq!(rd(o), 0x09003005, "expected layers chunk");
    // read visual levels from the geometry layer: id, ver, nlayers, type, lver, enabled, lookback id (NEW string), name string, isEnabled, geomver, crystal ver, U01, nvl
    let mut p = o + 4 + 4 + 4 + 4 + 4 + 4;
    let w = rd(p);
    p += 4;
    if (w & 0x3FFF_FFFF) == 0 {
        p += 4 + rd(p) as usize;
    }
    p += 4 + rd(p) as usize; // layer name
    p += 4; // isEnabled
    p += 4; // geometry version
    let cv = rd(p);
    assert_eq!(cv, 37, "template crystal version {cv}, writer knows 37");
    p += 4;
    p += 4; // U01
    let nvl = rd(p) as usize;
    p += 4;
    let mut visual_levels = Vec::new();
    for _ in 0..nvl {
        visual_levels.push((rd(p), f32::from_le_bytes(b[p + 4..p + 8].try_into().unwrap())));
        p += 8;
    }
    // smoothing groups from chunk 0x09003007
    let c7 = find(&b, &0x09003007u32.to_le_bytes(), o).expect("smoothing chunk");
    let nsg = rd(c7 + 8) as usize;
    let smoothing: Vec<f32> = (0..nsg).map(|i| f32::from_le_bytes(b[c7 + 12 + 4 * i..c7 + 16 + 4 * i].try_into().unwrap())).collect();
    let ni = rd(c7 + 12 + 4 * nsg) as usize;
    let end = c7 + 16 + 4 * nsg + 4 * ni;
    assert_eq!(&b[end..end + 4], &FACADE, "crystal node does not end after smoothing groups");
    let c6 = find(&b, &0x09003006u32.to_le_bytes(), o).expect("lightmap chunk");
    let raw_003 = b[start..c4].to_vec();
    let raw_005 = b[c5..c6].to_vec();
    // Layer headers: type u32, version u32, u01 u32, 0x40000000, len, "LayerN".
    // Layer k starts 20 bytes before its "LayerN" string; the last ends at c6.
    let mut layer_starts: Vec<(usize, u32)> = Vec::new();
    for k in 0..16u32 {
        let name = format!("Layer{k}");
        let mut probe = [0u8; 4];
        probe.copy_from_slice(&(name.len() as u32).to_le_bytes());
        let mut pat = vec![0u8, 0, 0, 0x40];
        pat.extend_from_slice(&probe);
        pat.extend_from_slice(name.as_bytes());
        if let Some(p) = find(&b, &pat, c5) {
            if p < c6 && p >= 12 {
                let ltype = rd(p - 12);
                layer_starts.push((p - 12, ltype));
            }
        }
    }
    layer_starts.sort();
    let mut extra_layers = Vec::new();
    for (i, (st, ltype)) in layer_starts.iter().enumerate() {
        let en = layer_starts.get(i + 1).map(|x| x.0).unwrap_or(c6);
        if matches!(*ltype, 14 | 15) {
            extra_layers.push(b[*st..en].to_vec());
        }
    }
    let raw_006 = b[c6..c7].to_vec();
    let raw_007 = b[c7..end].to_vec();
    Template { body: b, num_nodes: g.num_nodes, start, end, first_node, n_materials, mat_prefix, mat_suffix, chunk4, visual_levels, smoothing, extra_layers, raw_003, raw_005, raw_006, raw_007 }
}

/// Build the item: `template` bytes with the crystal replaced by `mesh` and
/// `materials`, and the Ident set to (`ident`, `author`).
pub fn build_item(template: &[u8], ident: &str, author: &str, materials: &[MaterialSpec], mesh: &CrystalMesh) -> Vec<u8> {
    build_item_with(template, ident, author, materials, mesh, 0)
}

/// `keep` bits copy the template's chunk verbatim instead of regenerating it:
/// 1 = materials, 2 = layers, 4 = lightmap, 8 = smoothing. Bisecting aid.
pub fn build_item_with(template: &[u8], ident: &str, author: &str, materials: &[MaterialSpec], mesh: &CrystalMesh, keep: u8) -> Vec<u8> {
    assert!(!materials.is_empty() && !mesh.faces.is_empty(), "empty crystal");
    // Two material slots with the same (link, physics) are FATAL in game:
    // NGameItemUtils::CreateSolid2Model (0x140F558D0) deduplicates equal
    // CPlugMaterialUserInsts when building the Solid2Model's CustomMaterials,
    // then writes the per-slot index map into that shorter array with no
    // bound check -- one garbage Release() per duplicate, so the crash is
    // random (whatever lies past the array). Nadeo items never carry
    // duplicates; merge ours and remap the faces.
    let (materials, mesh) = dedupe_materials(materials, mesh);
    let (materials, mesh) = (&materials[..], &mesh);
    let t = parse_template(template);
    let n_mat = materials.len();
    let delta = n_mat as i64 - t.n_materials as i64;
    for f in &mesh.faces {
        assert!((f.material as usize) < materials.len(), "face material out of range");
        assert!(f.verts.len() >= 3 && f.verts.len() == f.uvs.len());
    }
    let mut w = W(Vec::new());
    // ---- 0x09003003 materials, padded to the template's count
    if keep & 1 != 0 { w.0.extend_from_slice(&t.raw_003); } else {
    w.u32(0x09003003);
    w.u32(2);
    w.u32(n_mat as u32);
    for (i, m) in materials.iter().enumerate() {
        w.u32(0); // empty MaterialName -> node follows
        w.u32(t.first_node + i as u32);
        let mut prefix = t.mat_prefix.clone();
        let n = prefix.len();
        prefix[n - 2] = m.physics;
        w.0.extend_from_slice(&prefix);
        w.str(&m.link);
        // Experiment knob TINY_MAT_COLOR="r,g,b": fill the material node's
        // `int[] Color` (the suffix starts Csts=0, Color=0, UvAnims=0, ids=0,
        // UserTextures=0, HidingGroup=-1).
        if let Ok(c) = std::env::var("TINY_MAT_COLOR") {
            let vals: Vec<i32> = c.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            let mut suf = t.mat_suffix.clone();
            assert_eq!(&suf[4..8], &[0, 0, 0, 0], "template Color array not empty");
            let mut ins = (vals.len() as u32).to_le_bytes().to_vec();
            for v in &vals { ins.extend_from_slice(&v.to_le_bytes()); }
            suf.splice(4..8, ins);
            w.0.extend_from_slice(&suf);
        } else {
            w.0.extend_from_slice(&t.mat_suffix);
        }
    }
    }
    // ---- 0x09003004 verbatim
    w.0.extend_from_slice(&t.chunk4);
    // ---- 0x09003005 layers: one geometry layer
    if keep & 2 != 0 { w.0.extend_from_slice(&t.raw_005); } else {
    w.u32(0x09003005);
    w.u32(0); // chunk version
    w.u32(1 + t.extra_layers.len() as u32); // geometry + trigger/spawn layers
    w.u32(0); // Geometry
    w.u32(2); // layer version
    w.u32(0); // crystalEnabled
    w.u32(0x4000_0000); // LayerId: new lookback string
    w.str("Layer0");
    w.str("Geometry");
    w.u32(1); // IsEnabled
    w.u32(1); // GeometryVersion
    // Crystal
    w.u32(37);
    w.u32(4);
    w.u32(t.visual_levels.len() as u32);
    for (a, b) in &t.visual_levels {
        w.u32(*a);
        w.f32(*b);
    }
    w.u32(0); // anchor infos
    // Groups form a tree: folders (no name, parent -1, a list of children)
    // and leaves ("part", parent = folder index, no children); faces name
    // leaves only. A leaf whose parent is itself hangs the loader forever.
    w.u32(2);
    w.u32(0); // folder: U01
    w.u8(1); //   U02
    w.u32(0xFFFF_FFFF); //   parent: none
    w.str("");
    w.u32(0xFFFF_FFFF); //   U04
    w.u32(1); //   one child ...
    w.u32(1); //   ... the leaf
    w.u32(0); // leaf: U01
    w.u8(1); //   U02
    w.u32(0); //   parent: the folder
    w.str("part");
    w.u32(0xFFFF_FFFF); //   U04
    w.u32(0); //   no children
    w.u8(1); // embedded crystal
    // U02 = max face material index, U03 = max face group index (the game
    // reads them as width selectors for the per-face optimized ints;
    // mesh archive 0x1413D0964).
    let max_mat = mesh.faces.iter().map(|f| f.material).max().unwrap_or(0);
    w.u32(max_mat);
    w.u32(1);
    w.u32(mesh.positions.len() as u32);
    for p in &mesh.positions {
        w.f32(p[0]);
        w.f32(p[1]);
        w.f32(p[2]);
    }
    // edges: informational count, then an empty optimized array
    let mut edges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    for f in &mesh.faces {
        for k in 0..f.verts.len() {
            let a = f.verts[k];
            let b = f.verts[(k + 1) % f.verts.len()];
            edges.insert((a.min(b), a.max(b)));
        }
    }
    w.u32(edges.len() as u32);
    w.u32(0);
    w.u32(mesh.faces.len() as u32);
    // texcoords, de-duplicated, then one index per face corner
    let mut uv_index: HashMap<[u32; 2], u32> = HashMap::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut corner_uv: Vec<u32> = Vec::new();
    for f in &mesh.faces {
        for uv in &f.uvs {
            let k = [uv[0].to_bits(), uv[1].to_bits()];
            let i = *uv_index.entry(k).or_insert_with(|| {
                uvs.push(*uv);
                (uvs.len() - 1) as u32
            });
            corner_uv.push(i);
        }
    }
    w.u32(uvs.len() as u32);
    for uv in &uvs {
        w.f32(uv[0]);
        w.f32(uv[1]);
    }
    w.u32(corner_uv.len() as u32);
    for i in &corner_uv {
        w.opt(*i, corner_uv.len());
    }
    for f in &mesh.faces {
        w.u8((f.verts.len() - 3) as u8);
        for v in &f.verts {
            w.opt(*v, mesh.positions.len());
        }
        w.opt(f.material, n_mat);
        w.opt(1, 2); // group: the leaf
    }
    w.u32(0); // U04
    // GeometryLayer tail
    w.u32(2); // U02 ints, one per group
    w.u32(0);
    w.u32(1);
    w.u32(1); // IsVisible
    w.u32(1); // Collidable
    for l in &t.extra_layers {
        w.0.extend_from_slice(l); // Trigger / SpawnPosition layers verbatim
    }
    }
    // ---- 0x09003006 lightmap coords: a non-overlapping atlas, one grid cell
    // per face with the corners spread on a circle inside it. Overlapping
    // lightmap UVs (the texture UVs reused) made the editor draw the item as
    // a translucent bounding box instead of a mesh.
    if keep & 4 != 0 { w.0.extend_from_slice(&t.raw_006); } else {
    w.u32(0x09003006);
    w.u32(2);
    let grid = (mesh.faces.len() as f64).sqrt().ceil().max(1.0) as usize;
    // TINY_LM_SCALE shrinks the atlas into a corner (experiment knob; the
    // "lightmap budget" theory it served was the duplicate-material crash).
    let lm_scale: f64 = std::env::var("TINY_LM_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let cell = lm_scale / grid as f64;
    let mut lms: Vec<[u16; 2]> = Vec::new();
    let mut corner_lm: Vec<u32> = Vec::new();
    for (fi, f) in mesh.faces.iter().enumerate() {
        let (cx, cy) = ((fi % grid) as f64 * cell, (fi / grid) as f64 * cell);
        let n = f.verts.len();
        for k in 0..n {
            let ang = std::f64::consts::TAU * k as f64 / n as f64;
            let u = cx + cell * (0.5 + 0.4 * ang.cos());
            let v = cy + cell * (0.5 + 0.4 * ang.sin());
            lms.push([(u * 65535.0) as u16, (v * 65535.0) as u16]);
            corner_lm.push((lms.len() - 1) as u32);
        }
    }
    w.u32(lms.len() as u32);
    for k in &lms {
        w.u16(k[0]);
        w.u16(k[1]);
    }
    w.u32(corner_lm.len() as u32);
    for i in &corner_lm {
        w.opt(*i, corner_lm.len());
    }
    }
    // ---- 0x09003007 smoothing groups
    if keep & 8 != 0 { w.0.extend_from_slice(&t.raw_007); } else {
    w.u32(0x09003007);
    w.u32(0);
    w.u32(t.smoothing.len() as u32);
    for s in &t.smoothing {
        w.f32(*s);
    }
    // Every Nadeo/community crystal read marks each face with group 2 (the
    // third of the three floats 0,1,2); 0 smoothed a box into a black blob.
    w.u32(mesh.faces.len() as u32);
    for _ in &mesh.faces {
        w.u32(2);
    }
    }
    // ---- splice; inline nodes after the materials move by `delta`
    let mut suffix = t.body[t.end..].to_vec();
    if delta != 0 {
        let lo = t.first_node + t.n_materials as u32;
        let mut i = 0;
        while i + 8 <= suffix.len() {
            let v = u32::from_le_bytes(suffix[i..i + 4].try_into().unwrap());
            let c = u32::from_le_bytes(suffix[i + 4..i + 8].try_into().unwrap());
            if v >= lo && v < t.num_nodes && (c >> 24 == 0x2E || c >> 24 == 0x09) && (c & 0xFFF) == 0 {
                let nv = (v as i64 + delta) as u32;
                suffix[i..i + 4].copy_from_slice(&nv.to_le_bytes());
                i += 8;
            } else {
                i += 1;
            }
        }
    }
    let mut body = Vec::with_capacity(t.body.len() + w.0.len());
    body.extend_from_slice(&t.body[..t.start]);
    body.extend_from_slice(&w.0);
    body.extend_from_slice(&suffix);
    let mut g = Gbx::parse(template);
    g.num_nodes = (g.num_nodes as i64 + delta) as u32;
    g.body = body.clone();
    let out = g.write_body_recompressed(&body);
    let out = crate::tiny_assets::set_header_ident(&out, ident, author);
    crate::tiny_assets::set_body_ident_nameless(&out, ident)
}

/// The game material a collision surface's physics name stands for: what the
/// car feels is also what the eye should see. Unknown names get the road
/// material with their own physics id, so the surface still drives right.
pub fn material_for_physics_name(name: &str) -> MaterialSpec {
    material_for_physics_name_in(name, 26)
}

/// Same, for a given map collection. Item material links are WHITELISTED per
/// environment: Stadium (26) accepts `Stadium\Media\Material\*`; BlueBay (28)
/// and the other 2026 environments cull every face set that names one, and
/// only the mesh-editor family `Editors\MeshEditorMedia\Materials\*` draws
/// there (flat tinted surfaces, no textures -- the best this build allows).
pub fn material_for_physics_name_in(name: &str, collection: u32) -> MaterialSpec {
    let phys = crate::scene::physics_id(name).unwrap_or(16);
    if collection != 26 {
        // The mesh-editor family, one flat tint each (Asphalt slate grey,
        // Concrete white, Grass mint, Sand cream, Rock/Metal grey, Dirt
        // terracotta, Wood tan, Ice cyan, Snow white, Plastic yellow). The
        // physics id is the surface's own -- the game compares link AND
        // physics when deduplicating, so two slots may share a link.
        let link = match name {
            "Asphalt" | "WetAsphalt" => "Editors\\MeshEditorMedia\\Materials\\Asphalt",
            "Rubber" | "SlidingRubber" => "Editors\\MeshEditorMedia\\Materials\\Concrete",
            "Concrete" | "Pavement" | "WetPavement" => "Editors\\MeshEditorMedia\\Materials\\Concrete",
            "Metal" | "ResonantMetal" | "MetalTrans" => "Editors\\MeshEditorMedia\\Materials\\Metal",
            "Grass" | "WetGrass" => "Editors\\MeshEditorMedia\\Materials\\Grass",
            "Dirt" | "DirtRoad" | "WetDirtRoad" => "Editors\\MeshEditorMedia\\Materials\\Dirt",
            "Ice" => "Editors\\MeshEditorMedia\\Materials\\Ice",
            "Sand" => "Editors\\MeshEditorMedia\\Materials\\Sand",
            "Wood" => "Editors\\MeshEditorMedia\\Materials\\Wood",
            "Rock" => "Editors\\MeshEditorMedia\\Materials\\Rock",
            "Snow" => "Editors\\MeshEditorMedia\\Materials\\Snow",
            "RoadSynthetic" => "Editors\\MeshEditorMedia\\Materials\\Plastic",
            _ => "Editors\\MeshEditorMedia\\Materials\\Concrete",
        };
        let mut link = link.to_string();
        if let Ok(ov) = std::env::var("TINY_LINK_OVERRIDE") {
            for kv in ov.split(';') {
                if let Some((k, v)) = kv.split_once('=') {
                    if k == name {
                        link = format!("Editors\\MeshEditorMedia\\Materials\\{v}");
                    }
                }
            }
        }
        return MaterialSpec { link, physics: phys };
    }
    let link = match name {
        "Asphalt" | "WetAsphalt" => "Stadium\\Media\\Material\\RoadTech",
        "Rubber" | "SlidingRubber" => "Stadium\\Media\\Material\\TrackBorders",
        "Concrete" | "Pavement" | "WetPavement" => "Stadium\\Media\\Material\\PlatformTech",
        "Metal" | "ResonantMetal" => "Stadium\\Media\\Material\\Technics",
        "Grass" | "WetGrass" => "Stadium\\Media\\Material\\Grass",
        "Dirt" | "DirtRoad" | "WetDirtRoad" => "Stadium\\Media\\Material\\RoadDirt",
        "Ice" => "Stadium\\Media\\Material\\RoadIce",
        "Sand" => "Stadium\\Media\\Material\\Sand",
        "Wood" => "Stadium\\Media\\Material\\PlatformWood",
        "Rock" => "Stadium\\Media\\Material\\Rock",
        "Snow" => "Stadium\\Media\\Material\\Snow",
        _ => "Stadium\\Media\\Material\\RoadTech",
    };
    MaterialSpec { link: link.to_string(), physics: phys }
}

/// Merge material slots that are equal for the game (same link and physics)
/// and remap the faces onto the surviving slots, in first-seen order.
pub fn dedupe_materials(materials: &[MaterialSpec], mesh: &CrystalMesh) -> (Vec<MaterialSpec>, CrystalMesh) {
    // Keyed on the LINK alone: the map still crashed at F5633C with slots
    // that differed only in physics (Concrete for both rubber borders and
    // concrete decks), so the game's equality ignores the physics byte at
    // this point. The surviving slot takes the physics of whichever slot
    // covers the most faces.
    let mut faces_per: Vec<usize> = vec![0; materials.len()];
    for f in &mesh.faces {
        faces_per[f.material as usize] += 1;
    }
    let mut out: Vec<MaterialSpec> = Vec::new();
    let mut out_faces: Vec<usize> = Vec::new();
    let mut remap: Vec<u32> = Vec::with_capacity(materials.len());
    for (i, m) in materials.iter().enumerate() {
        let pos = out.iter().position(|o| o.link == m.link);
        remap.push(match pos {
            Some(p) => {
                if faces_per[i] > out_faces[p] {
                    out[p].physics = m.physics;
                    out_faces[p] = faces_per[i];
                }
                p as u32
            }
            None => {
                out.push(m.clone());
                out_faces.push(faces_per[i]);
                (out.len() - 1) as u32
            }
        });
    }
    let mut mesh = mesh.clone();
    for f in &mut mesh.faces {
        f.material = remap[f.material as usize];
    }
    (out, mesh)
}

/// A `LINK|PHYS` label from `Collector::link_labels` as a material spec. A
/// label with no link (an inline material with no file) falls back to a
/// neutral game material for its physics.
pub fn material_for_link_label(label: &str) -> MaterialSpec {
    let (link, phys) = label.rsplit_once('|').unwrap_or((label, "16"));
    let physics: u8 = phys.parse().unwrap_or(16);
    if link.is_empty() || !link.contains('\\') {
        return MaterialSpec { link: neutral_link_for(physics).to_string(), physics };
    }
    MaterialSpec { link: link.to_string(), physics }
}

/// The scene's material labels mapped onto game materials: a label that is a
/// physics name keeps its physics id on a neutral material; anything else
/// gets the collection's material of that name.
pub fn material_for_label(collection: &str, label: &str) -> MaterialSpec {
    let phys = crate::scene::physics_id(label);
    if let Some(p) = phys {
        return MaterialSpec { link: neutral_link_for(p).to_string(), physics: p };
    }
    MaterialSpec { link: format!("{collection}\\Media\\Material\\{label}"), physics: 16 }
}

fn neutral_link_for(phys: u8) -> &'static str {
    match crate::scene::physics_name(phys) {
        "Grass" => "Stadium\\Media\\Material\\Grass",
        "Dirt" | "DirtRoad" => "Stadium\\Media\\Material\\RoadDirt",
        "Ice" => "Stadium\\Media\\Material\\RoadIce",
        "Concrete" => "Stadium\\Media\\Material\\PlatformTech",
        _ => "Stadium\\Media\\Material\\RoadTech",
    }
}

/// Read the template's first geometry layer back into a `CrystalMesh` plus its
/// material links: the round-trip oracle for the writer (an item rebuilt from
/// its own decoded mesh must load like the original).
pub fn decode_template(bytes: &[u8]) -> (Vec<MaterialSpec>, CrystalMesh) {
    let g = Gbx::parse(bytes);
    let b = &g.body;
    let rd = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let rf = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let mut o = find(b, &0x09003003u32.to_le_bytes(), 0).unwrap() + 8;
    let nm = rd(o) as usize;
    o += 4;
    let mut materials = Vec::new();
    for _ in 0..nm {
        o += 8; // empty name, node index
        let node_start = o;
        let fac = find(b, &FACADE, o).unwrap() + 4;
        let link_at = link_offset(b, node_start);
        let l = rd(link_at) as usize;
        materials.push(MaterialSpec { link: String::from_utf8_lossy(&b[link_at + 4..link_at + 4 + l]).to_string(), physics: b[link_at - 2] });
        o = fac;
    }
    let mut o = find(b, &0x09003005u32.to_le_bytes(), o).unwrap() + 4;
    let mut u32r = |o: &mut usize| { let v = rd(*o); *o += 4; v };
    u32r(&mut o); // chunk version
    u32r(&mut o); // layer count
    u32r(&mut o); // type
    u32r(&mut o); // layer version
    u32r(&mut o); // enabled
    let w = u32r(&mut o);
    if (w & 0x3FFF_FFFF) == 0 { let l = u32r(&mut o) as usize; o += l; }
    let l = u32r(&mut o) as usize; o += l; // name
    u32r(&mut o); // isEnabled
    u32r(&mut o); // geometry version
    assert_eq!(u32r(&mut o), 37);
    u32r(&mut o); // U01
    let nvl = u32r(&mut o) as usize; o += 8 * nvl;
    assert_eq!(u32r(&mut o), 0, "anchor infos");
    let ng = u32r(&mut o) as usize;
    for _ in 0..ng {
        o += 4 + 1 + 4; // U01, U02 byte, U03
        let l = u32r(&mut o) as usize; o += l; // name
        o += 4; // U04
        let n = u32r(&mut o) as usize; o += 4 * n;
    }
    o += 1 + 8; // embedded, U02, U03
    let npos = u32r(&mut o) as usize;
    let mut mesh = CrystalMesh::default();
    for i in 0..npos { mesh.positions.push([rf(o + 12 * i), rf(o + 12 * i + 4), rf(o + 12 * i + 8)]); }
    o += 12 * npos;
    u32r(&mut o); assert_eq!(u32r(&mut o), 0, "edge array");
    let nf = u32r(&mut o) as usize;
    let nt = u32r(&mut o) as usize;
    let mut uvs = Vec::new();
    for i in 0..nt { uvs.push([rf(o + 8 * i), rf(o + 8 * i + 4)]); }
    o += 8 * nt;
    let nti = u32r(&mut o) as usize;
    let wt = if nti >= 65535 { 4 } else if nti >= 255 { 2 } else { 1 };
    let mut tci = Vec::with_capacity(nti);
    for i in 0..nti {
        let p = o + i * wt;
        tci.push(match wt { 4 => rd(p) as usize, 2 => u16::from_le_bytes(b[p..p + 2].try_into().unwrap()) as usize, _ => b[p] as usize });
    }
    o += nti * wt;
    let wp = if npos >= 65535 { 4 } else if npos >= 255 { 2 } else { 1 };
    let mut corner = 0usize;
    for _ in 0..nf {
        let vc = b[o] as usize + 3; o += 1;
        let mut verts = Vec::with_capacity(vc);
        for k in 0..vc {
            let p = o + k * wp;
            verts.push(match wp { 4 => rd(p), 2 => u16::from_le_bytes(b[p..p + 2].try_into().unwrap()) as u32, _ => b[p] as u32 });
        }
        o += vc * wp;
        let material = if nm >= 65535 { let v = rd(o); o += 4; v } else if nm >= 255 { let v = u16::from_le_bytes(b[o..o + 2].try_into().unwrap()) as u32; o += 2; v } else { let v = b[o] as u32; o += 1; v };
        // group index
        if ng >= 65535 { o += 4 } else if ng >= 255 { o += 2 } else { o += 1 }
        let fuv: Vec<[f32; 2]> = (0..vc).map(|k| uvs[tci[corner + k]]).collect();
        corner += vc;
        mesh.faces.push(Face { verts, uvs: fuv, material });
    }
    (materials, mesh)
}
