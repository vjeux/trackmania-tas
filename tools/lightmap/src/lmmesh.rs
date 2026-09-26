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

/// THE LIGHTMAP UV SET PER MATERIAL (RE 7, 2026-09-26 03:10Z, hill4's tiny items against the editor's charts): the geoms of
/// the world-projected TERRAIN game materials — `BlueBay\Media\Material\{TransitionToSand, TransitionToLand,
/// TransitionToSeaFloor, Sand, Land, SeaFloor, HillPxz}` (the Pxz shaders texture from the world position, so their mesh
/// uv is free for the lightmap) — carry the lightmap in TEXCOORD0; stock models, Technics / LightSpot and the TD* user
/// material models carry it in TEXCOORD1 (the converter's assumption). Rasterising hill4's transition item through
/// TEXCOORD0 covers 67 % of its chart with 97 % of the covered texels lit, as the editor's file; through TEXCOORD1 the
/// charts are 31 % / 24 % covered and the lit texels sit elsewhere. The flag the CPU LM builder reads is not located; this
/// is the observed material table (LMTOOL_LM_UV_TC1_ALL=1 restores TEXCOORD1 for everything).
pub fn terrain_material_takes_tc0(link: &str) -> bool {
    if std::env::var_os("LMTOOL_LM_UV_TC1_ALL").is_some() { return false; }
    // THE GAME'S SELECTOR (RE 11, 2026-09-26 09:20Z, NHmsLightMap::NLocal::CreateVStreamTcLM 0x140a3a6f0 → FUN_140216b30): the
    // lightmap uv = TEXCOORD[TexCoordIndex] of the material's SHADER's "PreLightGen" bitmap binding (CPlugBitmapAddress chunk
    // 0x09047007 in the .Shader.Gbx); read from the pack when the link was prefetched (`prefetch_lm_uv_index`), else the observed table
    if let Some(Some(idx)) = lm_uv_index_cached(link) { return idx == 0; }
    let l = link.to_ascii_lowercase();
    let Some(pos) = l.rfind("\\media\\material\\") else { return false };
    let name = &l[pos + "\\media\\material\\".len()..];
    // the observed table (RE 7, 03:10Z) — RE 11's shader read moves TransitionTo* to index 1 (PyPxz_Ids_Tex); kept as the fallback only
    matches!(name, "transitiontosand" | "transitiontoland" | "transitiontoseafloor" | "sand" | "land" | "seafloor" | "hillpxz")
}

/// The per-link cache of the shader's PreLightGen TexCoordIndex: `None` = the shader has no PreLightGen binding or its pass word
/// lacks 0x1000 — the geom is in NO lightmap pass (RE 11: this ONE field + ONE bit is both of RE 7's observed tables).
static LM_UV_UNKNOWN: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));
static LM_UV_INDEX: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, Option<u32>>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// The pack store the selector resolves unknown links against (set once by the bake before the scene is built:
/// `set_lm_uv_store`); without it only prefetched links are known.
static LM_UV_STORE: std::sync::Mutex<Option<mapgeom::store::DataStore>> = std::sync::Mutex::new(None);

pub fn set_lm_uv_store(store: mapgeom::store::DataStore) {
    *LM_UV_STORE.lock().unwrap() = Some(store);
}

/// The cached selector for a link: `Some(Some(idx))` lightmapped with TEXCOORD[idx]; `Some(None)` not lightmapped; `None` unknown
/// (no store and not prefetched). A miss is resolved through the bake's store when one was set.
pub fn lm_uv_index_cached(link: &str) -> Option<Option<u32>> {
    if link.is_empty() { return None; }
    let key = link.to_ascii_lowercase();
    if let Some(v) = LM_UV_INDEX.lock().unwrap().get(&key).copied() { return Some(v); }
    if LM_UV_UNKNOWN.lock().unwrap().contains(&key) { return None; }
    let mut guard = LM_UV_STORE.lock().unwrap();
    let store = guard.as_mut()?;
    let (n, _) = prefetch_lm_uv_index(store, &[link.to_string()]);
    if n == 0 {
        // unresolvable (an embedded user material, a missing pack): remember the miss so the fallback table answers next time
        LM_UV_UNKNOWN.lock().unwrap().insert(key);
        return None;
    }
    LM_UV_INDEX.lock().unwrap().get(&link.to_ascii_lowercase()).copied()
}

/// Read a shader file's `PreLightGen*` binding: the Id string, then the following chunk 0x09047007 / 0x09047006 (u32 flags, i32
/// TexCoordIndex, u8) or 0x09047004 (i32 TexCoordIndex) — RE 11's re11_plgtc scan.
pub fn shader_prelightgen_tc(bytes: &[u8]) -> Option<u32> {
    for name in ["PreLightGenTx", "PreLightGen", "PreLightGenTy", "PreLightGenTz", "PreLightGenSH0", "PreLightGenSprite"] {
        let nb = name.as_bytes();
        let mut i = 4usize;
        while i + nb.len() <= bytes.len() {
            if &bytes[i..i + nb.len()] == nb && u32::from_le_bytes(bytes[i - 4..i].try_into().unwrap()) as usize == nb.len() {
                let p = i + nb.len();
                let mut q = p;
                while q + 12 <= bytes.len() && q < p + 64 {
                    let w = u32::from_le_bytes(bytes[q..q + 4].try_into().unwrap());
                    if w == 0x0904_7007 || w == 0x0904_7006 {
                        let tc = i32::from_le_bytes(bytes[q + 8..q + 12].try_into().unwrap());
                        return u32::try_from(tc).ok();
                    }
                    if w == 0x0904_7004 {
                        let tc = i32::from_le_bytes(bytes[q + 4..q + 8].try_into().unwrap());
                        return u32::try_from(tc).ok();
                    }
                    q += 1;
                }
                i = p;
            } else {
                i += 1;
            }
        }
    }
    None
}

/// Resolve and cache the LM uv selector of every link: the material chain → the shader file → its pass word (0x09002020: bit 0x1000 =
/// lightmapped) and its PreLightGen binding's TexCoordIndex. Returns (resolved, lightmapped) counts for the log.
pub fn prefetch_lm_uv_index(store: &mut mapgeom::store::DataStore, links: &[String]) -> (usize, usize) {
    let (mut resolved, mut lit) = (0usize, 0usize);
    for l in links {
        if l.is_empty() { continue; }
        let key = l.to_ascii_lowercase();
        if LM_UV_INDEX.lock().unwrap().contains_key(&key) { continue; }
        let mat = if key.ends_with(".material.gbx") { l.clone() } else { format!("{l}.Material.Gbx") };
        let chain = mapgeom::envblock::material_chain(store, &mat);
        let mut v: Option<Option<u32>> = None;
        if !chain.shader.is_empty() {
            let pass_ok = chain.flags.map(|f| f.pass_bits & 0x1000 != 0).unwrap_or(true);
            match store.read(&chain.shader) {
                Ok(bytes) => { let tc = shader_prelightgen_tc(&bytes); v = Some(if pass_ok { tc } else { None }); }
                Err(_) => {}
            }
        }
        if let Some(val) = v {
            resolved += 1;
            if val.is_some() { lit += 1; }
            LM_UV_INDEX.lock().unwrap().insert(key, val);
        }
    }
    (resolved, lit)
}

/// The game-material link of a shaded geom (the custom material's link, else its name, else the older material list).
pub fn geom_material_link(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, sg: &mapgeom::static_item::solid2::ShadedGeom) -> String {
    geom_material_link_ext(s2, sg, None)
}

/// `geom_material_link` with the model FILE's external references: a pack prefab's material is an external ref (RE 11's 0003), the
/// link = `materials::material_link(path)`.
pub fn geom_material_link_ext(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, sg: &mapgeom::static_item::solid2::ShadedGeom, externals: Option<&[(u32, String)]>) -> String {
    usize::try_from(sg.material_index).ok().and_then(|mi| {
        s2.custom_materials.get(mi).and_then(|cm| cm.inst().and_then(|m| m.link().map(|l| l.to_string())).or_else(|| if cm.name.is_empty() { None } else { Some(cm.name.clone()) }))
            .or_else(|| s2.materials.get(mi).and_then(|mr| match mr.inline.as_deref() { Some(mapgeom::static_item::Node::Material(m)) => m.link().map(|l| l.to_string()), _ => None }))
            .or_else(|| externals.and_then(|ex| s2.materials.get(mi).and_then(|r| if r.index >= 0 { ex.iter().find(|(i, _)| *i == r.index as u32).map(|(_, p)| mapgeom::static_item::materials::material_link(p)) } else { None })))
    }).unwrap_or_default()
}

/// The lightmap uvs of a shaded geom's visual: TEXCOORD0 for the terrain materials, else the TexCoord1 rule below.
pub fn lightmap_uvs_of_geom(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, sg: &mapgeom::static_item::solid2::ShadedGeom, vis: &mapgeom::static_item::visual::CPlugVisualIndexedTriangles) -> Option<Vec<[f32; 2]>> {
    lightmap_uvs_of_geom_ext(s2, sg, vis, None)
}

pub fn lightmap_uvs_of_geom_ext(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, sg: &mapgeom::static_item::solid2::ShadedGeom, vis: &mapgeom::static_item::visual::CPlugVisualIndexedTriangles, externals: Option<&[(u32, String)]>) -> Option<Vec<[f32; 2]>> {
    let link = geom_material_link_ext(s2, sg, externals);
    // a link whose shader is known NOT lightmapped (no PreLightGen binding / pass word without 0x1000): no LM geometry (RE 11)
    if let Some(None) = lm_uv_index_cached(&link) { return None; }
    if terrain_material_takes_tc0(&link) {
        use mapgeom::static_item::vstream::Elem;
        let st = vis.stream()?;
        let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        if let Some(Elem::Float2(u)) = get(mapgeom::static_item::vstream::N_TEXCOORD0) {
            return Some(u.clone());
        }
        if let Some(s) = vis.main.as_ref().and_then(|m| m.tex_coord_sets.get(0)) {
            return Some(s.coords.iter().map(|c| c.0).collect());
        }
    }
    lightmap_uvs(vis)
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
    lm_mesh_of_solid_ext(s2, order, None)
}

/// `lm_mesh_of_solid_ordered` with the model file's external references (pack prefabs: the material links, hence the lightmap uv set
/// and the lightmapped test, come from them).
pub fn lm_mesh_of_solid_ext(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, order: GeomOrder, externals: Option<&[(u32, String)]>) -> Option<LmMesh> {
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
        let Some(uv1) = lightmap_uvs_of_geom_ext(s2, sg, vis, externals) else { continue };
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
        // A VISUAL WITHOUT A NORMAL STREAM IS NOT LIGHTMAP GEOMETRY (engineer F, the stpad Sunrise capture): the Stadium Grass
        // tile's Base prefab has a fourth LOD-0 geom — the 9 880-vertex GrassFence skirt on the "Tech3 GrassFence_VDepLight"
        // material, a vertex-colour-lit visual with Position / Int32 colour / TexCoord0 and no normal — and the game's LM
        // draws of the 9 216 tiles (frame 0's H-basis accumulate and the local-light pass alike) carry 9 vertices / 24
        // indices = geom 0 alone; its PreLightGen bounds [0.001, 0.999]² are geom 0's uv range too. LMTOOL_LM_NO_NORMAL_KEEP=1
        // restores the old inclusion (the (0, 1, 0) stand-in normal).
        if normals.is_none() && std::env::var_os("LMTOOL_LM_NO_NORMAL_KEEP").is_none() {
            continue;
        }
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
    // ypr_to_quat returns (w, x, y, z) = the game's stream quaternion NEGATED (the same rotation; the stream is (x, y, z, w))
    let q = mapgeom::veget_instance::ypr_to_quat(pose.yaw, pose.pitch, pose.roll);
    LmInstance { q: [-q[1], -q[2], -q[3], -q[0]], t: pose.pos, scale: if pose.scale > 0.0 { pose.scale } else { 1.0 }, st, st_x_bits: st[0].to_bits() }
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
        let Some(uv1) = lightmap_uvs_of_geom(s2, sg, vis) else { continue };
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
        if lightmap_uvs_of_geom(s2, sg, vis).is_none() { continue; }
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

/// THE LM SCENE FROM THE MAP: one LM mesh per distinct item model (instanced over its items) plus the zone tile mesh
/// instanced over the 4096 cells, every instance carrying its chart ST from the game's layout (`layout::for_map`) —
/// what `--lm-from PASSCAP` took from the capture's vertex/instance streams. The instance order is the object order
/// (items first — model by model in first-appearance order — then the tiles in chart-array order); only overlapping
/// charts could make the order matter, and charts never overlap.
pub fn lm_scene_from_map(scene: &crate::geometry::Scene, layout: &crate::layout::GameLayout, base: u32, item_bytes: &dyn Fn(&str) -> Option<Vec<u8>>, tile_mesh: Option<LmMesh>, tile_plg: crate::layout::TilePlg, atlas: f32) -> Result<crate::lmaccum::LmScene, String> {
    lm_scene_from_map_at(scene, layout, base, item_bytes, tile_mesh, tile_plg, atlas, 0.0)
}

/// `lm_scene_from_map` with the tiles' world y (the tile row · 8 + the collection's yoff; BlueBay's is 0).
pub fn lm_scene_from_map_at(scene: &crate::geometry::Scene, layout: &crate::layout::GameLayout, base: u32, item_bytes: &dyn Fn(&str) -> Option<Vec<u8>>, tile_mesh: Option<LmMesh>, tile_plg: crate::layout::TilePlg, atlas: f32, tile_world_y: f32) -> Result<crate::lmaccum::LmScene, String> {
    use crate::lmaccum::LmScene;
    let mut sc = LmScene { meshes: Vec::new(), inst_first: Vec::new(), inst_count: Vec::new(), instances: Vec::new(), table: Vec::new(), eids: Vec::new(), frag_lists: Default::default(), fitted_world_box: None, rec_of: Vec::new(), st_src: Vec::new() };
    // the item's rect: by its map item index when the layout carries its records (chart k ↔ record k), else by obj = base + item
    let rect_of: std::collections::HashMap<u32, [i32; 4]> = if !layout.records.is_empty() {
        layout.records.iter().enumerate().filter_map(|(k, r)| { let (ii, _) = r.item.as_ref()?; let c = &layout.charts[k]; (c.charted == crate::layout::Charted::Bound).then_some((base + *ii as u32, [c.x, c.y, c.w, c.h])) }).collect()
    } else {
        layout.charts.iter().filter(|c| c.charted == crate::layout::Charted::Bound).map(|c| (c.obj, [c.x, c.y, c.w, c.h])).collect()
    };
    // the record behind an item (by map item index) — `LmScene::rec_of` for the local-light cull
    let rec_of_item: std::collections::HashMap<usize, usize> = layout.records.iter().enumerate().filter_map(|(k, r)| r.item.as_ref().map(|(ii, _)| (*ii, k))).collect();
    // items, grouped by model in first-appearance order
    let mut by_model: Vec<(usize, Vec<usize>)> = Vec::new(); // (model index, instance indices)
    for (ii, inst) in scene.instances.iter().enumerate() {
        match by_model.iter_mut().find(|(m, _)| *m == inst.model) {
            Some((_, v)) => v.push(ii),
            None => by_model.push((inst.model, vec![ii])),
        }
    }
    for (mi, insts) in &by_model {
        let name = &scene.model_names[*mi];
        let Some(bytes) = item_bytes(name) else { continue };
        let Some(mesh) = lm_mesh_of_item(&bytes)? else { continue };
        let bounds = scene.models[*mi].plg_bounds.unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let first = sc.instances.len();
        let mut n = 0usize;
        for &ii in insts {
            let inst = &scene.instances[ii];
            let Some(r) = rect_of.get(&(base + inst.item as u32)) else { continue };
            let st = crate::peelcolor::chart_st(*r, bounds, atlas);
            if std::env::var_os("LM_ST_TRACE").is_some() { eprintln!("  item {} rect {:?} bounds bits [{:#x} {:#x} {:#x} {:#x}] st bits [{:#x} {:#x} {:#x} {:#x}]", inst.item, r, bounds[0].to_bits(), bounds[1].to_bits(), bounds[2].to_bits(), bounds[3].to_bits(), st[0].to_bits(), st[1].to_bits(), st[2].to_bits(), st[3].to_bits()); }
            sc.instances.push(lm_instance(&inst.pose, st));
            sc.rec_of.push(rec_of_item.get(&inst.item).copied().unwrap_or(usize::MAX));
            sc.st_src.push((*r, bounds));
            n += 1;
        }
        if n == 0 { continue; }
        sc.meshes.push(mesh);
        sc.inst_first.push(first);
        sc.inst_count.push(n);
        sc.eids.push(0);
    }
    if let Some(tm) = tile_mesh {
        let first = sc.instances.len();
        let mut n = 0usize;
        // the tile charts: the tile records in order (chart k ↔ record k; their cells = layout.cell_of in the same order), else
        // the charts below the item base
        let tile_charts: Vec<(usize, (i32, i32))> = if !layout.records.is_empty() {
            layout.records.iter().enumerate().filter(|(_, r)| r.class == "tile").enumerate().map(|(ti, (k, _))| (k, layout.cell_of[ti])).collect()
        } else {
            layout.charts.iter().enumerate().filter(|(_, c)| c.obj < base).map(|(k, c)| (k, layout.cell_of[c.obj as usize])).collect()
        };
        for (k, (cx, cz)) in tile_charts {
            let c = &layout.charts[k];
            let st = crate::peelcolor::chart_st([c.x, c.y, c.w, c.h], tile_plg.bounds, atlas);
            sc.instances.push(LmInstance { q: [0.0, 0.0, 0.0, 1.0], t: [cx as f32 * 32.0, tile_world_y, cz as f32 * 32.0], scale: 1.0, st, st_x_bits: st[0].to_bits() });
            sc.rec_of.push(if layout.records.is_empty() { usize::MAX } else { k });
            sc.st_src.push(([c.x, c.y, c.w, c.h], tile_plg.bounds));
            n += 1;
        }
        sc.meshes.push(tm);
        sc.inst_first.push(first);
        sc.inst_count.push(n);
        sc.eids.push(0);
    }
    Ok(sc)
}

/// The zone tile's VISUAL vertices — (position, f32 lightmap uv) as the peel / pre-pass vertex streams carry them (the
/// LM stream quantises the uv to snorm16; the peel colour's atlas lookup (PS 17131) and the pre-pass raster read the
/// visual stream's f32 TexCoord1 — a 1e-5 uv difference moves bilinear atlas taps).
pub fn visual_tile_verts_of_zone(store: &mut mapgeom::store::DataStore, collection: &str, zone: &str) -> Result<Vec<([f32; 3], [f32; 2])>, String> {
    use mapgeom::static_item::vstream::Elem;
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
            let mut out = Vec::new();
            for sg in &s2.shaded_geoms {
                let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
                let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
                let Some(uv1) = lightmap_uvs(vis) else { continue };
                let Some(st) = vis.stream() else { continue };
                let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
                let Some(Elem::Float3(pos)) = get(mapgeom::static_item::vstream::N_POSITION) else { continue };
                for (p, u) in pos.iter().zip(uv1.iter()) {
                    out.push((*p, *u));
                }
            }
            return Ok(out);
        }
    }
    Ok(Vec::new())
}

/// THE PREFAB ENTITY RECORDS' LM MESHES (Stadium: the authored blocks' Base_Air, the engine's clips, the VFC walls): every
/// record of `layout.records` with a `MeshRef` adds an instance of its entity's Solid2Model LM mesh (one mesh per (prefab,
/// entity), first appearance) placed by the entity's world transform, with the chart ST of its own rect (chart k ↔ record k)
/// and the entity's PreLightGen uv-0 bounds. The instances of one mesh follow record order.
pub fn lm_scene_add_entities(store: &mut mapgeom::store::DataStore, layout: &crate::layout::GameLayout, sc: &mut crate::lmaccum::LmScene, atlas: f32) -> Result<usize, String> {
    let mut by_mesh: Vec<((String, usize), Vec<usize>)> = Vec::new();
    for (k, r) in layout.records.iter().enumerate() {
        let Some(m) = &r.mesh else { continue };
        let key = (m.prefab.clone(), m.entity);
        match by_mesh.iter_mut().find(|(kk, _)| *kk == key) { Some((_, v)) => v.push(k), None => by_mesh.push((key, vec![k])) }
    }
    let mut added = 0usize;
    for ((prefab, entity), recs) in &by_mesh {
        let pm = store.load_model(prefab)?;
        let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
        let Some(e) = pf.ents.get(*entity) else { continue };
        let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
        let Some(s2) = so.solid2() else { continue };
        let Some(mesh) = lm_mesh_of_solid_ext(s2, GeomOrder::default(), Some(&pm.externals)) else { continue };
        // LMTOOL_LM_ENTITY_TRACE_TSV=FILE: one line per record — record index (= chart index), prefab#entity, class, world centre y —
        // for per-prefab / per-height statistics over the charts table
        if let Ok(path) = std::env::var("LMTOOL_LM_ENTITY_TRACE_TSV") {
            use std::io::Write;
            if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                for &k in recs { let r = &layout.records[k]; let _ = writeln!(fh, "{k}\t{prefab}#{entity}\t{}\t{:.3}", r.class, r.centre[1]); }
            }
        }
        // LMTOOL_LM_ENTITY_TRACE=1: per entity the geoms' materials, the chosen uv set, the LM uv bounds and the record's PLG uv box
        if std::env::var_os("LMTOOL_LM_ENTITY_TRACE").is_some() {
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for v in &mesh.verts { for a in 0..2 { lo[a] = lo[a].min(v.uv[a]); hi[a] = hi[a].max(v.uv[a]); } }
            let mats: Vec<String> = s2.shaded_geoms.iter().map(|sg| { let l = geom_material_link_ext(s2, sg, Some(&pm.externals)); format!("{}{}", l.rsplit('\\').next().unwrap_or(&l), match lm_uv_index_cached(&l) { Some(Some(i)) => format!("[tc{i}]"), Some(None) => "[not lightmapped]".to_string(), None => (if terrain_material_takes_tc0(&l) { "[tc0?]" } else { "[tc1?]" }).to_string() }) }).collect();
            let r0 = &layout.records[recs[0]];
            // the first triangle's winding normal vs its vertex normal (the peel's front-face test is the winding)
            let wind = if mesh.indices.len() >= 3 {
                let (a, b, c) = (mesh.verts[mesh.indices[0] as usize].pos, mesh.verts[mesh.indices[1] as usize].pos, mesh.verts[mesh.indices[2] as usize].pos);
                let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]]; let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                format!("tri0 winding n ({:.2}, {:.2}, {:.2}) vertex n {:?} pos y {:.3}", n[0], n[1], n[2], mesh.verts[mesh.indices[0] as usize].normal, a[1])
            } else { String::new() };
            let (nmin, nmax) = mesh.verts.iter().map(|v| (v.normal[0] * v.normal[0] + v.normal[1] * v.normal[1] + v.normal[2] * v.normal[2]).sqrt()).fold((f32::MAX, f32::MIN), |acc, l| (acc.0.min(l), acc.1.max(l)));
            eprintln!("lm entity {prefab}#{entity} ({} records, class {}): {} v / {} t, LM uv [{:.3}, {:.3}]..[{:.3}, {:.3}], record uv box {:?}, geoms {:?}; {wind}; psize {:?}; |normal| {:.3}..{:.3}", recs.len(), r0.class, mesh.verts.len(), mesh.indices.len() / 3, lo[0], lo[1], hi[0], hi[1], r0.uv, mats, mesh.verts.iter().map(|v| v.psize).fold((f32::MAX, f32::MIN), |acc, p| (acc.0.min(p), acc.1.max(p))), nmin, nmax);
            if std::env::var_os("LMTOOL_LM_ENTITY_TRACE_GEOMS").is_some() {
                for line in geom_summary(s2) { eprintln!("    {line}"); }
                for (gi, sg) in s2.shaded_geoms.iter().enumerate() {
                    if let Some(vr) = s2.visuals.get(sg.visual_index as usize) { if let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() { if let Some(st) = vis.stream() {
                        if let Some((_, mapgeom::static_item::vstream::Elem::Float3(pos))) = st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == mapgeom::static_item::vstream::N_POSITION) {
                            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                            for p in pos { for a in 0..3 { lo[a] = lo[a].min(p[a]); hi[a] = hi[a].max(p[a]); } }
                            eprintln!("    geom {gi}: {} verts, pos [{:.2}, {:.2}, {:.2}]..[{:.2}, {:.2}, {:.2}]; custom material {:?}", pos.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], s2.custom_materials.get(sg.material_index as usize).map(|cm| cm.name.clone()));
                        }
                    } } }
                }
            }
        }
        let first = sc.instances.len();
        let mut n = 0usize;
        for &k in recs {
            let r = &layout.records[k];
            let c = &layout.charts[k];
            let m = r.mesh.as_ref().unwrap();
            let iso = crate::lmtiles::from_xform(&m.xf);
            let m9: [f32; 9] = [iso[0], iso[1], iso[2], iso[3], iso[4], iso[5], iso[6], iso[7], iso[8]];
            let q = mapgeom::veget_instance::mat_to_quat(&m9);
            // the stream quaternion is (x, y, z, w) of the rotation; mat_to_quat gives (w, x, y, z)
            let st = crate::peelcolor::chart_st([c.x, c.y, c.w, c.h], r.uv, atlas);
            sc.instances.push(crate::sunpass::LmInstance { q: [q[1], q[2], q[3], q[0]], t: [iso[9], iso[10], iso[11]], scale: 1.0, st, st_x_bits: st[0].to_bits() });
            sc.rec_of.push(k);
            sc.st_src.push(([c.x, c.y, c.w, c.h], r.uv));
            n += 1;
        }
        if n == 0 { continue; }
        sc.meshes.push(mesh);
        sc.inst_first.push(first);
        sc.inst_count.push(n);
        sc.eids.push(0);
        added += n;
    }
    Ok(added)
}
