//! Baking a `CPlugCrystal` (a Nadeo crystal item's mesh) into static-item
//! geometry: the enabled, visible Geometry layers' faces are fan-triangulated
//! into one visual per material (positions, flat normals, UVs, tangents in
//! the reference vertex layout), the collidable layers' faces (or, failing
//! that, the visible faces) become the collision mesh.

use super::build::Merged;
use super::R;
use crate::crystal_model::{CPlugCrystal, Crystal, LayerKind};

/// One triangle corner with everything the vertex layout needs.
#[derive(Clone, Copy, Debug)]
pub struct Corner {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

/// Fan-triangulate a face; corners carry the face normal and their UV.
/// Quads (and n-gons) use the (v1,v3) diagonal: the editor's bake starts
/// faces at v1 (measured: every reference quad splits v1-v3, never v0-v2).
pub fn face_triangles(c: &Crystal, f: &crate::crystal_model::Face, scale: f32) -> Vec<[Corner; 3]> {
    let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| c.positions[*i as usize]).map(|p| [p[0] * scale, p[1] * scale, p[2] * scale]).collect();
    if pts.len() < 3 {
        return Vec::new();
    }
    let uvs = c.face_uvs(f);
    // Newell normal: right for concave polygons too.
    let mut n = [0f32; 3];
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let n = normalize(n);
    let corner = |i: usize| Corner { pos: pts[i], normal: n, uv: uvs.get(i).copied().unwrap_or([0.0, 0.0]) };
    if pts.len() == 3 {
        return vec![[corner(0), corner(1), corner(2)]];
    }
    // Fan from v1: (v1,v2,v3), (v1,v3,v4), ...
    (2..pts.len()).map(|i| [corner(1), corner(i), corner((i + 1) % pts.len())]).collect()
}

/// Tangent along +u of a triangle's UV mapping (falls back to any vector
/// perpendicular to the normal).
pub fn tangent(t: &[Corner; 3]) -> ([f32; 3], [f32; 3]) {
    let e1 = sub(t[1].pos, t[0].pos);
    let e2 = sub(t[2].pos, t[0].pos);
    let du1 = t[1].uv[0] - t[0].uv[0];
    let dv1 = t[1].uv[1] - t[0].uv[1];
    let du2 = t[2].uv[0] - t[0].uv[0];
    let dv2 = t[2].uv[1] - t[0].uv[1];
    let det = du1 * dv2 - du2 * dv1;
    let n = t[0].normal;
    if det.abs() < 1e-12 {
        let up = if n[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let tu = normalize(cross(up, n));
        return (tu, normalize(cross(n, tu)));
    }
    let r = 1.0 / det;
    let tu = normalize([(e1[0] * dv2 - e2[0] * dv1) * r, (e1[1] * dv2 - e2[1] * dv1) * r, (e1[2] * dv2 - e2[2] * dv1) * r]);
    let tv = normalize([(e2[0] * du1 - e1[0] * du2) * r, (e2[1] * du1 - e1[1] * du2) * r, (e2[2] * du1 - e1[2] * du2) * r]);
    (tu, tv)
}

/// Layers whose faces are drawn / collided against.
pub fn geometry_layers(c: &CPlugCrystal) -> Vec<(&Crystal, bool, bool)> {
    let mut out = Vec::new();
    if let Some((_, cr)) = &c.single_layer {
        out.push((cr, true, true));
    }
    for l in &c.layers {
        if let LayerKind::Geometry { crystal, is_visible, collidable, .. } = &l.kind {
            if l.base.is_enabled {
                out.push((crystal, *is_visible, *collidable));
            }
        }
    }
    out
}

use super::build::{dec3n_pack, MergedVisual};
use super::surface::Triangle;
use super::visual::{CPlugVisualIndexedTriangles, IndexBuffer, VisualMain};
use super::vstream::*;
use super::{Node, NodeRef};
use std::collections::HashMap;

/// Reference vertex layout: Position Float3 | Normal Dec3N | TexCoord0 |
/// TexCoord1 | TangentU Dec3N | TangentV Dec3N (vertex = 40 bytes).
pub fn reference_decls() -> Vec<Decl> {
    vec![
        Decl::new(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0),
        Decl::new(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC),
        Decl::new(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x10),
        Decl::new(N_TEXCOORD0 + 1, T_FLOAT2, SPACE_GLOBAL2D, 0x18),
        Decl::new(N_TANGENT_U, T_DEC3N, SPACE_LOCAL3D, 0x20),
        Decl::new(N_TANGENT_V, T_DEC3N, SPACE_LOCAL3D, 0x24),
    ]
}

/// Materials whose game definition is built on a `PyPxz` triplanar base get a
/// flat white vertex-color layer and visual flags 0x78 from the editor's bake
/// (measured on all 26 reference items: exactly TrackWall/DecoHill/DecoHill2
/// carry it, and all three resolve to a `Tech3 Block PyPxz...` base while
/// every other material resolves to TDSN/PDiff).
pub fn material_has_vertex_color(link: &str) -> bool {
    let name = link.rsplit('\\').next().unwrap_or(link);
    matches!(name, "TrackWall" | "DecoHill" | "DecoHill2")
}

/// Color-layout decls: Color u32 slots between the normal and TexCoord0.
pub fn color_decls() -> Vec<Decl> {
    vec![
        Decl::new(N_POSITION, T_FLOAT3, SPACE_GLOBAL3D, 0),
        Decl::new(N_NORMAL, T_DEC3N, SPACE_LOCAL3D, 0xC),
        Decl::new(N_COLOR0, T_COLOR, SPACE_GLOBAL2D, 0x10),
        Decl::new(N_TEXCOORD0, T_FLOAT2, SPACE_GLOBAL2D, 0x14),
        Decl::new(N_TEXCOORD0 + 1, T_FLOAT2, SPACE_GLOBAL2D, 0x1C),
        Decl::new(N_TANGENT_U, T_DEC3N, SPACE_LOCAL3D, 0x24),
        Decl::new(N_TANGENT_V, T_DEC3N, SPACE_LOCAL3D, 0x28),
    ]
}

/// Visuals (at most 65000 vertices each) over triangles; identical corners
/// share a vertex. `white_color` adds the flat-white Color layer (and the
/// 0x78 flags) the editor's bake puts on Pxz-base materials.
pub fn make_visuals(tris: &[[Corner; 3]], white_color: bool) -> Vec<CPlugVisualIndexedTriangles> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < tris.len() {
        let mut pos: Vec<[f32; 3]> = Vec::new();
        let mut nrm: Vec<u32> = Vec::new();
        let mut uv: Vec<[f32; 2]> = Vec::new();
        let mut tu: Vec<u32> = Vec::new();
        let mut tv: Vec<u32> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        let mut seen: HashMap<[u32; 8], u32> = HashMap::new();
        let mut end = start;
        while end < tris.len() && pos.len() + 3 <= 65000 {
            let t = &tris[end];
            let (a, b) = tangent(t);
            for c in t {
                let key = [c.pos[0].to_bits(), c.pos[1].to_bits(), c.pos[2].to_bits(), c.normal[0].to_bits(), c.normal[1].to_bits(), c.normal[2].to_bits(), c.uv[0].to_bits(), c.uv[1].to_bits()];
                let i = *seen.entry(key).or_insert_with(|| {
                    pos.push(c.pos);
                    nrm.push(dec3n_pack(c.normal));
                    uv.push(c.uv);
                    tu.push(dec3n_pack(a));
                    tv.push(dec3n_pack(b));
                    (pos.len() - 1) as u32
                });
                idx.push(i);
            }
            end += 1;
        }
        start = end;
        let n = pos.len() as i32;
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in &pos {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let stream = CPlugVertexStream {
            version: 1,
            count: n,
            flags: 1,
            base: super::null_ref(),
            decls: if white_color { color_decls() } else { reference_decls() },
            compress_local3d: Some(true),
            elems: if white_color {
                vec![Elem::Float3(pos), Elem::Word(nrm), Elem::Word(vec![0xFFFF_FFFF; n as usize]), Elem::Float2(uv.clone()), Elem::Float2(uv), Elem::Word(tu), Elem::Word(tv)]
            } else {
                vec![Elem::Float3(pos), Elem::Word(nrm), Elem::Float2(uv.clone()), Elem::Float2(uv), Elem::Word(tu), Elem::Word(tv)]
            },
        };
        let main = VisualMain {
            version: 6,
            chunk_flags: if white_color { 0x78 } else { 0x38 },
            tex_coord_sets: Vec::new(),
            count: n,
            vertex_streams: vec![NodeRef { index: 0, inline: Some(Box::new(Node::VertexStream(stream))) }],
            skin: None,
            bounding_box: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, (hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0, (hi[2] - lo[2]) / 2.0],
            bitmap_elems: Vec::new(),
            uv_groups: Vec::new(),
            u02: 0,
            u03: 0,
            u04: Vec::new(),
        };
        out.push(CPlugVisualIndexedTriangles {
            chunks: vec![0x09006001, 0x09006005, 0x09006009, 0x0900600B, 0x0900600F, 0x09006010, 0x0902C002, 0x0902C004, 0x0906A001],
            id: super::Id::Null,
            u_node: super::null_ref(),
            sub_visuals: Vec::new(),
            u_float: 0.0,
            splits: Vec::new(),
            main: Some(main),
            morph: Some((0, 0)),
            v3d_node: super::null_ref(),
            tangents: Some((Vec::new(), Vec::new())),
            index_buffer: Some(IndexBuffer::delta(idx)),
        });
    }
    out
}

/// Bake a crystal into `m`: one visual per material over the visible faces,
/// collision from the collidable layers (else the visible faces).
pub fn add_crystal(c: &CPlugCrystal, scale: f32, m: &mut Merged) -> R<()> {
    let layers = geometry_layers(c);
    if layers.is_empty() {
        return Err("crystal has no geometry layer".into());
    }
    // material slot per crystal material index
    let slots: Vec<usize> = c
        .materials
        .iter()
        .map(|mat| match mat.inst() {
            Some(inst) => m.material_inst_slot(inst),
            None => m.material_slot(&mat.name, 0),
        })
        .collect();
    let mut per_material: Vec<Vec<[Corner; 3]>> = vec![Vec::new(); slots.len().max(1)];
    let mut surf_tris: Vec<(Triangle, [[f32; 3]; 3])> = Vec::new();
    let any_collidable = layers.iter().any(|(_, _, col)| *col);
    for (cr, visible, collidable) in &layers {
        for f in &cr.faces {
            let tris = face_triangles(cr, f, scale);
            let slot_i = if f.material >= 0 && (f.material as usize) < slots.len() { Some(f.material as usize) } else { None };
            if *visible {
                match slot_i {
                    Some(i) => per_material[i].extend(tris.iter().cloned()),
                    None => per_material[0].extend(tris.iter().cloned()),
                }
            }
            if *collidable || (!any_collidable && *visible) {
                let phys = slot_i.map(|i| m.materials[slots[i]].physics()).unwrap_or(0);
                for t in &tris {
                    surf_tris.push((Triangle { indices: [0; 3], material_id: phys, u03: 0, surface_index: 0 }, [t[0].pos, t[1].pos, t[2].pos]));
                }
            }
        }
    }
    // The editor's bake orders materials by triangle count, most first
    // (measured on every reference item).
    let mut order: Vec<usize> = (0..per_material.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(per_material[i].len()));
    for i in order {
        let tris = &per_material[i];
        if tris.is_empty() {
            continue;
        }
        let slot = slots.get(i).copied().unwrap_or_else(|| m.material_slot("Stadium\\Media\\Material\\PlatformTech", 0));
        let white = material_has_vertex_color(&m.materials[slot].link().unwrap_or("").to_string());
        for v in make_visuals(tris, white) {
            m.visuals.push(MergedVisual { visual: v, material: slot });
        }
    }
    // collision: shared vertices by exact position
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut seen: HashMap<[u32; 3], u32> = HashMap::new();
    let mut tris: Vec<Triangle> = Vec::new();
    for (t, ps) in &surf_tris {
        let mut ix = [0u32; 3];
        for (k, p) in ps.iter().enumerate() {
            ix[k] = *seen.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_insert_with(|| {
                verts.push(*p);
                (verts.len() - 1) as u32
            });
        }
        tris.push(Triangle { indices: ix, ..*t });
    }
    m.add_surface_mesh(&verts, &tris, &crate::geom::IDENTITY, 1.0);
    m.notes.push(format!("crystal baked: {} layers, {} triangles, {} collision triangles", layers.len(), per_material.iter().map(|v| v.len()).sum::<usize>(), tris.len()));
    Ok(())
}
