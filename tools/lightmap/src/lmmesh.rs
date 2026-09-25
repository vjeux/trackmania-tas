//! THE LM MESH FROM THE MAP — the game's lightmap vertex stream built from a model's visuals, WITHOUT a capture
//! (the (b1) gap of the file chain: `--lm-from PASSCAP` takes the captured `vb_*.bin` / instance stream; a bake of any
//! map has to make them itself). The captured streams are the oracle (`lmtool lmmesh-check MAP PASSCAP`):
//!
//! * one LM mesh per model = its LOD-0 shaded geoms' visuals that carry the lightmap uv set (RE 7's
//!   `lm_geometry_nonempty`: the visual is skipped when it has no TEXCOORD1 — pwc-day's Sea tile keeps its SeaFloor
//!   plane and drops the Water plane), vertices and triangles in the visual's own order, geoms concatenated;
//! * the vertex format of the stream (stride 40, `sunpass::LmVertex`): POSITION f32×3 (the visual's positions),
//!   BLENDINDICES 0xffff (one chart), NORMAL snorm16 (round to nearest of n·32767, the visual's per-vertex normal),
//!   PSIZE = the tangent-frame mode (3 on the flat meshes, ±1 on the vegetation — the sign of the bitangent),
//!   TEXCOORD0 snorm16 = the lightmap uv (TexCoord1) quantised, TANGENT snorm16 (the visual's tangent; (1, 0, 0) when
//!   the visual has none);
//! * the instance = (the item's quaternion, translation, uniform scale, the chart ST of its layout rect).
//!
//! What this module does NOT yet transcribe is flagged by the check: the vertex positions of the captured pad / tile
//! carry a 1e-5 noise the stored model positions may not (a transform round trip in the game's builder).

use crate::sunpass::{LmInstance, LmMesh, LmVertex};

/// snorm16 quantisation as the game's LM-mesh builder does it: TRUNCATION toward zero of v·32767 (cvttss2si — the
/// captured pad / wall uvs: 16583.984 → 16583, 32.767 → 32; round-to-nearest is refuted on 5 of the pad's 9 vertices),
/// clamped, back to f32 as the shader reads it (v / 32767).
pub fn snorm16_roundtrip(v: f32) -> f32 {
    let q = (v * 32767.0).trunc().clamp(-32767.0, 32767.0);
    q / 32767.0
}

/// Whether a visual carries the lightmap uv set (TexCoord1 in the stream or the visual's own second set).
fn lightmap_uvs(vis: &mapgeom::static_item::visual::CPlugVisualIndexedTriangles) -> Option<Vec<[f32; 2]>> {
    use mapgeom::static_item::vstream::Elem;
    let st = vis.stream()?;
    let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
    if let Some(Elem::Float2(u)) = get(mapgeom::static_item::vstream::N_TEXCOORD0 + 1) {
        return Some(u.clone());
    }
    if let Some(s) = vis.main.as_ref().and_then(|m| m.tex_coord_sets.get(1)) {
        return Some(s.coords.iter().map(|c| c.0).collect());
    }
    // one uv set only: it is the lightmap set too (the tiny pad AC16236083 — its captured LM uv is its single set)
    if let Some(Elem::Float2(u)) = get(mapgeom::static_item::vstream::N_TEXCOORD0) {
        return Some(u.clone());
    }
    vis.main.as_ref().and_then(|m| m.tex_coord_sets.get(0)).map(|s| s.coords.iter().map(|c| c.0).collect())
}

/// Per shaded geom of a solid: (geom index, visual index, lod mask, vertex count, triangle count, has lightmap uvs, has
/// a tangent stream, the psize modes) — the builder's raw material for the order study.
pub fn geom_summary(s2: &mapgeom::static_item::solid2::CPlugSolid2Model) -> Vec<String> {
    use mapgeom::static_item::vstream::Elem;
    let mut out = Vec::new();
    for (gi, sg) in s2.shaded_geoms.iter().enumerate() {
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
        let st = vis.stream();
        let names: Vec<u32> = st.map(|s| s.decls.iter().map(|d| d.name()).collect()).unwrap_or_default();
        let nv = st.and_then(|s| s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == mapgeom::static_item::vstream::N_POSITION).map(|(_, e)| match e { Elem::Float3(p) => p.len(), _ => 0 })).unwrap_or(0);
        let nt = vis.index_buffer.as_ref().map(|ib| ib.indices.len() / 3).unwrap_or(0);
        out.push(format!("geom {gi}: visual {} lod_mask {} material {} — {nv} verts, {nt} tris, stream decls {:?}, tex_coord_sets {}, lm uvs {}, visual tangent arrays {:?} (inline_tangents {}), main flags {:#x}", sg.visual_index, sg.lod_mask, sg.material_index, names, vis.main.as_ref().map(|m| m.tex_coord_sets.len()).unwrap_or(0), lightmap_uvs(vis).is_some(), vis.tangents.as_ref().map(|(a, b)| (a.len(), b.len())), vis.inline_tangents, vis.main.as_ref().map(|m| m.flags()).unwrap_or(0)));
    }
    out
}

/// The LM mesh of a model's Solid2: `None` when no visual carries lightmap uvs.
pub fn lm_mesh_of_solid(s2: &mapgeom::static_item::solid2::CPlugSolid2Model) -> Option<LmMesh> {
    lm_mesh_of_solid_ordered(s2, GeomOrder::default())
}

/// The order the builder concatenates a solid's LOD-0 geoms in (study; the captured vegetation mesh of pwc-day has its
/// three geoms as [2, 0, 1] = ascending vertex count = ascending triangle count).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeomOrder {
    /// The shaded-geom order of the file.
    File,
    /// Ascending vertex count (ties by file order).
    VertsAsc,
    /// Ascending triangle count.
    TrisAsc,
    /// Ascending material index.
    MaterialAsc,
}

impl Default for GeomOrder {
    fn default() -> Self {
        match std::env::var("LM_GEOM_ORDER").as_deref() {
            Ok("file") => GeomOrder::File,
            Ok("tris") => GeomOrder::TrisAsc,
            Ok("material") => GeomOrder::MaterialAsc,
            _ => GeomOrder::VertsAsc,
        }
    }
}

pub fn lm_mesh_of_solid_ordered(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, order: GeomOrder) -> Option<LmMesh> {
    use mapgeom::static_item::vstream::Elem;
    let mut verts: Vec<LmVertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let mut geoms: Vec<usize> = (0..s2.shaded_geoms.len()).collect();
    let stats = |gi: usize| -> (usize, usize, i32) {
        let sg = &s2.shaded_geoms[gi];
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { return (0, 0, sg.material_index) };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { return (0, 0, sg.material_index) };
        let nv = vis.stream().and_then(|st| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == mapgeom::static_item::vstream::N_POSITION).map(|(_, e)| match e { Elem::Float3(p) => p.len(), _ => 0 })).unwrap_or(0);
        (nv, vis.index_buffer.as_ref().map(|ib| ib.indices.len() / 3).unwrap_or(0), sg.material_index)
    };
    match order {
        GeomOrder::File => {}
        GeomOrder::VertsAsc => geoms.sort_by_key(|&g| stats(g).0),
        GeomOrder::TrisAsc => geoms.sort_by_key(|&g| stats(g).1),
        GeomOrder::MaterialAsc => geoms.sort_by_key(|&g| stats(g).2),
    }
    for &gi in &geoms {
        let sg = &s2.shaded_geoms[gi];
        if sg.lod_mask != 0 && sg.lod_mask & 1 == 0 {
            continue;
        }
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
        let Some(uv1) = lightmap_uvs(vis) else { continue };
        let Some(st) = vis.stream() else { continue };
        let Some(ib) = vis.index_buffer.as_ref() else { continue };
        let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let Some(Elem::Float3(pos)) = get(mapgeom::static_item::vstream::N_POSITION) else { continue };
        // the normal: a Dec3N word decoded as the 10-bit fields / 511 WITHOUT normalisation (the captured vegetation normals
        // are exactly that, snorm16-truncated: 0x0145956c → (0.7123288, 0.69863, 0.0391389) → (23340, 22892, 1282)/32767), or
        // the f32 stream as is
        let normals: Option<Vec<[f32; 3]>> = match get(mapgeom::static_item::vstream::N_NORMAL) {
            Some(Elem::Float3(n)) => Some(n.clone()),
            Some(Elem::Word(w)) => Some(w.iter().map(|&x| dec3n_raw(x, 511.0)).collect()),
            _ => None,
        };
        // the tangent: TANGENT_U the same way (w = 0); PSIZE = the tangent frame's handedness ±1 from TANGENT_V (the sign of
        // (n × tU) · tV), 3 for a mesh without tangents
        let dec_t = |e: Option<&Elem>| -> Option<Vec<[f32; 3]>> { match e { Some(Elem::Float4(t)) => Some(t.iter().map(|v| [v[0], v[1], v[2]]).collect()), Some(Elem::Float3(t)) => Some(t.clone()), Some(Elem::Word(w)) => Some(w.iter().map(|&x| dec3n_raw(x, 511.0)).collect()), _ => None } };
        // the tangent frame is emitted for a geom whose material is a CUSTOM user material with its own shader model (the
        // vegetation's TDOSN_/TDSN_ materials); a geom on a LINKED pack material (the tiny wall's TrackWallInWorld — its
        // stream carries tangent words too — and the pad's Land) gets mode 3 and the (1, 0, 0) tangent
        let linked = usize::try_from(sg.material_index).ok().map(|mi| {
            let custom_link = s2.custom_materials.get(mi).and_then(|cm| cm.inst().and_then(|m| m.link().map(|l| l.to_string())));
            let plain_link = s2.materials.get(mi).and_then(|mr| match mr.inline.as_deref() { Some(mapgeom::static_item::Node::Material(m)) => m.link().map(|l| l.to_string()), _ => None });
            custom_link.or(plain_link).map(|l| !l.is_empty()).unwrap_or(false)
        }).unwrap_or(false);
        let tan_u = if linked { None } else { dec_t(get(mapgeom::static_item::vstream::N_TANGENT_U)) };
        let tan_v = if linked { None } else { dec_t(get(mapgeom::static_item::vstream::N_TANGENT_V)) };
        let base = verts.len() as u16;
        for (i, p) in pos.iter().enumerate() {
            let n = normals.as_ref().and_then(|v| v.get(i)).copied().unwrap_or([0.0, 1.0, 0.0]);
            let tu = tan_u.as_ref().and_then(|v| v.get(i)).copied();
            let tv = tan_v.as_ref().and_then(|v| v.get(i)).copied();
            let uv = uv1.get(i).copied().unwrap_or([0.0, 0.0]);
            let (t, psize) = match (tu, tv) {
                (Some(tu), Some(tv)) => {
                    let c = [n[1] * tu[2] - n[2] * tu[1], n[2] * tu[0] - n[0] * tu[2], n[0] * tu[1] - n[1] * tu[0]];
                    let h = c[0] * tv[0] + c[1] * tv[1] + c[2] * tv[2];
                    (tu, if h < 0.0 { -1.0 } else { 1.0 })
                }
                (Some(tu), None) => (tu, 1.0),
                _ => ([1.0, 0.0, 0.0], 3.0),
            };
            verts.push(LmVertex {
                pos: *p,
                chart_idx: 0xffff,
                normal: [snorm16_roundtrip(n[0]), snorm16_roundtrip(n[1]), snorm16_roundtrip(n[2])],
                uv: [snorm16_roundtrip(uv[0]), snorm16_roundtrip(uv[1])],
                psize,
                tangent: [snorm16_roundtrip(t[0]), snorm16_roundtrip(t[1]), snorm16_roundtrip(t[2]), 0.0],
            });
        }
        for &i in &ib.indices {
            indices.push(base + i as u16);
        }
    }
    if verts.is_empty() {
        return None;
    }
    Some(LmMesh { verts, indices })
}

/// The LM mesh of an item file's model.
pub fn lm_mesh_of_item(bytes: &[u8]) -> Result<Option<LmMesh>, String> {
    let f = mapgeom::static_item::file::parse_file(bytes)?;
    let Some(so) = f.item.static_object() else { return Ok(None) };
    let Some(s2) = so.solid2() else { return Ok(None) };
    Ok(lm_mesh_of_solid(s2))
}

/// The instance of a placed item: RE 4's quaternion, the translation, the uniform scale, the chart ST of its layout
/// rect (`peelcolor::chart_st` with the model's PreLightGen bounds).
pub fn lm_instance(pose: &crate::geometry::ItemPose, st: [f32; 4]) -> LmInstance {
    let q = mapgeom::veget_instance::ypr_to_quat(pose.yaw, pose.pitch, pose.roll);
    LmInstance { q: [q[0], q[1], q[2], q[3]], t: pose.pos, scale: if pose.scale > 0.0 { pose.scale } else { 1.0 }, st, st_x_bits: st[0].to_bits() }
}

/// A vertex-by-vertex comparison of two LM meshes (ours vs the captured): counts of exact positions / normals / uvs /
/// tangents / psize, the triangle lists, the worst position error.
pub struct MeshDiff {
    pub n_ours: usize,
    pub n_theirs: usize,
    pub pos_exact: usize,
    pub pos_within_1e4: usize,
    pub pos_worst: f32,
    pub nrm_exact: usize,
    pub uv_exact: usize,
    pub tan_exact: usize,
    pub psize_exact: usize,
    pub tris_equal: bool,
}

pub fn diff_meshes(ours: &LmMesh, theirs: &LmMesh) -> MeshDiff {
    let n = ours.verts.len().min(theirs.verts.len());
    let mut d = MeshDiff { n_ours: ours.verts.len(), n_theirs: theirs.verts.len(), pos_exact: 0, pos_within_1e4: 0, pos_worst: 0.0, nrm_exact: 0, uv_exact: 0, tan_exact: 0, psize_exact: 0, tris_equal: ours.indices == theirs.indices };
    for i in 0..n {
        let (a, b) = (&ours.verts[i], &theirs.verts[i]);
        let e = (0..3).map(|k| (a.pos[k] - b.pos[k]).abs()).fold(0f32, f32::max);
        if e == 0.0 { d.pos_exact += 1; }
        if e <= 1e-4 { d.pos_within_1e4 += 1; }
        d.pos_worst = d.pos_worst.max(e);
        if a.normal == b.normal { d.nrm_exact += 1; }
        if a.uv == b.uv { d.uv_exact += 1; }
        if a.tangent == b.tangent { d.tan_exact += 1; }
        if a.psize == b.psize { d.psize_exact += 1; }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snorm16_quantises_the_plg_bound_like_the_capture() {
        // the Sea tile's uv bound 0.047988176 → 1572/32767 = 0.0479751 in the captured stream; the pad's v 0.506118476
        // (16583.984) → 16583 (truncation, not rounding)
        assert_eq!(snorm16_roundtrip(0.047988176), 1572.0 / 32767.0);
        assert_eq!(snorm16_roundtrip(0.506118476), 16583.0 / 32767.0);
        assert_eq!(snorm16_roundtrip(0.001), 32.0 / 32767.0);
        assert_eq!(snorm16_roundtrip(1.0), 1.0);
        assert_eq!(snorm16_roundtrip(-1.0), -1.0);
    }
}

/// The raw (unquantised) lightmap uvs of a model's LM-mesh vertices, in the builder's vertex order — the quantisation study.
pub fn raw_lm_uvs(bytes: &[u8]) -> Result<Vec<[f32; 2]>, String> {
    let f = mapgeom::static_item::file::parse_file(bytes)?;
    let Some(s2) = f.item.static_object().and_then(|so| so.solid2()) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for sg in &s2.shaded_geoms {
        if sg.lod_mask != 0 && sg.lod_mask & 1 == 0 { continue; }
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
        let Some(uv1) = lightmap_uvs(vis) else { continue };
        out.extend(uv1);
    }
    Ok(out)
}

/// The raw NORMAL / TANGENT_U / TANGENT_V stream words (Dec3N) of a model's LM-mesh vertices in the builder's order — the
/// decode study (`lmmesh-check --perm` prints them for the first matched vertices).
pub fn raw_normal_words(bytes: &[u8]) -> Result<Vec<(Option<u32>, Option<u32>, Option<u32>, Option<[f32; 3]>)>, String> {
    use mapgeom::static_item::vstream::Elem;
    let f = mapgeom::static_item::file::parse_file(bytes)?;
    let Some(s2) = f.item.static_object().and_then(|so| so.solid2()) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for sg in &s2.shaded_geoms {
        if sg.lod_mask != 0 && sg.lod_mask & 1 == 0 { continue; }
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
        if lightmap_uvs(vis).is_none() { continue; }
        let Some(st) = vis.stream() else { continue };
        let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let n = match get(mapgeom::static_item::vstream::N_POSITION) { Some(Elem::Float3(p)) => p.len(), _ => 0 };
        let word = |e: Option<&Elem>, i: usize| -> Option<u32> { match e { Some(Elem::Word(w)) => w.get(i).copied(), _ => None } };
        let f3 = |e: Option<&Elem>, i: usize| -> Option<[f32; 3]> { match e { Some(Elem::Float3(v)) => v.get(i).copied(), _ => None } };
        for i in 0..n {
            out.push((word(get(mapgeom::static_item::vstream::N_NORMAL), i), word(get(mapgeom::static_item::vstream::N_TANGENT_U), i), word(get(mapgeom::static_item::vstream::N_TANGENT_V), i), f3(get(mapgeom::static_item::vstream::N_NORMAL), i)));
        }
    }
    Ok(out)
}

/// Dec3N decoded WITHOUT normalisation: the 10-bit signed fields / `den`.
pub fn dec3n_raw(w: u32, den: f32) -> [f32; 3] {
    let f = |s: u32| { let v = ((w >> s) & 0x3ff) as i32; let v = if v >= 512 { v - 1024 } else { v }; v as f32 / den };
    [f(0), f(10), f(20)]
}

/// The zone tile's LM mesh from the pak: the zone block info's ground variant → its mobil's prefab → entity 0's static
/// object → the lightmapped visuals (the Sea prefab's SeaFloor plane; the Water plane has no lightmap uvs) — the same
/// builder as the items'.
pub fn lm_mesh_of_zone(store: &mut mapgeom::store::DataStore, collection: &str, zone: &str) -> Result<Option<LmMesh>, String> {
    for (fam, ext) in [("GameCtnBlockInfoFlat", "EDFlat"), ("GameCtnBlockInfoFrontier", "EDFrontier"), ("GameCtnBlockInfoTransition", "EDTransition"), ("GameCtnBlockInfoClassic", "EDClassic")] {
        let path = format!("{collection}\\GameCtnBlockInfo\\{fam}\\{zone}.{ext}.Gbx");
        let Ok(bi) = mapgeom::blockinfo::load(store, &path) else { continue };
        let Some(v) = bi.variant_base_ground.as_ref() else { continue };
        let Some(pp) = v.mobils.iter().flatten().find_map(|m| m.prefab.clone()) else { continue };
        let pm = store.load_model(&pp)?;
        let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
        for e in &pf.ents {
            let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
            let Some(s2) = so.solid2() else { continue };
            return Ok(lm_mesh_of_solid(s2));
        }
    }
    Ok(None)
}
