//! Geometry for the baker: every item of a tiny map as world-space triangles
//! carrying the model's lightmap UVs (TexCoord1), grouped by item so each item
//! gets its own chart.

use std::collections::BTreeMap;

use mapgeom::static_item::vstream::{Elem, N_NORMAL, N_POSITION, N_TEXCOORD0};
use mapgeom::static_item::Node;

pub type V3 = [f32; 3];

#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub p: [V3; 3],
    /// Per-vertex normals (face normal when the stream has none usable).
    pub n: [V3; 3],
    /// TexCoord1 of each vertex.
    pub uv: [[f32; 2]; 3],
    /// TexCoord0 of each vertex (the alpha-tested materials' cut-out lookup; zeros when absent).
    pub uv0: [[f32; 2]; 3],
    /// Index into the model's `mat_links` (u16::MAX = unknown material).
    pub mat: u16,
    /// Index into the model's `alpha_tex` (u16::MAX = opaque): the material's cut-out texture.
    pub alpha: u16,
    /// Index into the model's `diff_tex` (u16::MAX = none): a link-less material's diffuse texture (slot 0).
    pub diff: u16,
}

/// A light socket of a model (`CPlugSolid2Model.lights`): position and axis in
/// model space, the `GxLight` parameters. Spot angles in degrees (a pack spot
/// is 120–170°, nearly a hemisphere), `radius` = the ball radius (falloff range).
#[derive(Clone, Copy, Debug, Default)]
pub struct LightDef {
    pub pos: V3,
    /// The socket's forward axis (the spot direction).
    pub dir: V3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub radius: f32,
    /// (inner, outer) cone angles in degrees; (180, 180) for a ball light.
    pub cone: (f32, f32),
    pub animated: bool,
    /// CPlugLight NightOnly (flags bit 0): the lamp is lit — and baked into frame 1 — only while the mood blender's
    /// local-light switch is on (`moods::BlenderCurve::local_lights_on(word)`); a lamp without it is baked at every
    /// DayTime (`moods::lamp_is_baked`).
    pub night_only: bool,
    /// THE ATTENUATION FIELDS of the GxLightBall chunk 0x04002008 the local-light pass needs (engineer F, frame 1): `hyper2`
    /// (h1, h2) → moods::att_hn2 with the instance radius, `att_htnlr`, the ball flags (bit 9: a shadow radius of its own),
    /// the emitting radius / cylinder length (Emissive_Length_Left/Up), the GxLight flags and the specular / shadow / flare radii.
    pub hyper2: [f32; 2],
    pub att_htnlr: [f32; 2],
    pub ball_flags: u32,
    pub emitting: [f32; 2],
    pub gx_flags: u32,
    pub radii: [f32; 3],
    /// The socket's LEFT and UP axes (the light matrix's columns 0 and 1; column 2 = `dir`) — the emitter rectangle's axes for the
    /// area sampling (RE 13, 2026-09-26 21:20Z: pos' = pos + u·a·LEFT + v·b·UP). Zero when the source carries no frame.
    pub left: V3,
    pub up: V3,
}

#[derive(Clone, Debug, Default)]
pub struct ModelGeom {
    pub tris: Vec<Tri>,
    pub lights: Vec<LightDef>,
    /// LOD-0 shaded geoms that carry a lightmap uv set (TexCoord1, or the visual's own set 1) — the game's record filter
    /// needs one (itemrule::lm_geometry_nonempty: a 1-uv-set item gets no chart record).
    pub lm_uv_geoms: usize,
    /// PreLightGen: u02 (the metres-per-uv the game sizes the chart with) and the uv1 bounds u04[0..4].
    pub plg_u02: f32,
    pub plg_bounds: Option<[f32; 4]>,
    /// √(world area / uv area): metres per uv unit — the chart's world size.
    pub metres_per_uv: f32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    /// The game-material links of the model's shaded geoms (`CPlugMaterialUserInst.link`, e.g.
    /// `Stadium\Media\Material\RoadTech`), indexed by `Tri.mat`.
    pub mat_links: Vec<String>,
    /// The cut-out textures (file names inside the map's item zip) of the alpha-tested materials: a
    /// material whose `CPlugMaterialUserInst` fills the DiffuseO slot (1) — the baked vegetation cards'
    /// TDOSN model reads its colour AND its alpha cut from it (mapgeom: `model_color_slot`).
    pub alpha_tex: Vec<String>,
    /// The diffuse textures (slot 0) of the link-less user materials: the bounce albedo of a custom-texture
    /// item (the tiny Stadium blocks) is that texture's mean colour.
    pub diff_tex: Vec<String>,
    /// Per material link: the diffuse albedo the bounce uses (`crate::albedo`).
    pub mat_albedo: Vec<[f32; 3]>,
    /// The union of the visuals' STORED bounding boxes (CPlugVisual `bounding_box` = centre xyz, half xyz — the tiny
    /// library writes each half at least 0.02), in model space, as (min, max): the box the game's block record
    /// carries (`|M|·h + T`), which the light-camera fit (lightcam) works from — the wall's ±0.02 thickness is here,
    /// not in its triangles.
    pub stored_bbox: Option<([f32; 3], [f32; 3])>,
    /// The visuals' stored boxes themselves, as written: (centre, half) per visual (the record arithmetic works on this form).
    pub stored_boxes: Vec<([f32; 3], [f32; 3])>,
    /// EVERY shaded geom's stored visual box in shaded-geom order, LOD 0 or not: (lod mask, centre, half) — the
    /// game's CPlugTree bounding box (the block record's box, `lmtiles::model_box`) is the union over the solid's
    /// visuals in this order; `stored_boxes` keeps only the LOD-0 ones.
    pub stored_boxes_all: Vec<(i32, [f32; 3], [f32; 3])>,
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn mul(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-12 {
        mul(a, 1.0 / l)
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Decode a Dec3N packed normal (10:10:10 signed).
pub fn dec3n(w: u32) -> V3 {
    let f = |s: u32| {
        let v = ((w >> s) & 0x3ff) as i32;
        let v = if v >= 512 { v - 1024 } else { v };
        v as f32 / 511.0
    };
    norm([f(0), f(10), f(20)])
}

/// The lightmappable triangles of one item file (static objects; prefabs
/// contribute nothing here — moving parts have no stable chart).
pub fn load_model(bytes: &[u8]) -> Result<ModelGeom, String> {
    let f = mapgeom::static_item::file::parse_file(bytes)?;
    let mut g = ModelGeom::default();
    // the item's static object: the entity model's own, or — a PREFAB item (several entities: a static object plus a
    // waypoint trigger, the stock Screen items, …) — the first entity whose model is a static object (the record the
    // game's static-item path makes: FUN_1404382c0 folds that entity's visual boxes); its pose is applied to the triangles
    let mut ent_pose: Option<([f32; 4], [f32; 3])> = None;
    let so: &mapgeom::static_item::item::CPlugStaticObjectModel = match f.item.static_object() {
        Some(so) => so,
        None => {
            let Some(pf) = f.item.prefab() else { return Ok(g) };
            let Some((e, so)) = pf.ents.iter().find_map(|e| match e.model.inline.as_deref() { Some(Node::StaticObject(so)) => Some((e, so)), _ => None }) else { return Ok(g) };
            if e.rot != [0.0, 0.0, 0.0, 1.0] || e.pos != [0.0, 0.0, 0.0] { ent_pose = Some((e.rot, e.pos)); }
            so
        }
    };
    let Some(s2) = so.solid2() else {
        return Ok(g);
    };
    Ok(geom_from_solid2(s2, ent_pose))
}

/// THE STOCK ITEMS (a placed item whose model the map does not embed — `Screen2x1Small`, …): the pack's
/// `<Collection>\Items\<name>.Item.Gbx` references its model as an EXTERNAL prefab (`…\2x1Small.Prefab.Gbx`) whose
/// entities carry the static object inline; resolved through the store.
pub fn load_model_from_store(store: &mut mapgeom::store::DataStore, logical: &str) -> Result<ModelGeom, String> {
    let m = store.load_model(logical)?;
    let trace = std::env::var_os("LMTOOL_STOCK_TRACE").is_some();
    let item = mapgeom::static_item::file::parse_body_with(&m.body, &m.external_indices()).map_err(|e| format!("{logical}: {e}"))?;
    // the entity model's reference (the edition slot first, as the game writes it)
    let Some(mc) = item.model() else { if trace { eprintln!("{logical}: no model chunk"); } return Ok(ModelGeom::default()) };
    if trace { eprintln!("{logical}: externals {:?}; edition index {} entity_model index {}", m.externals, mc.entity_model_edition.index, mc.entity_model.index); }
    let mut prefab_path: Option<String> = None;
    for slot in [&mc.entity_model_edition, &mc.entity_model] {
        if slot.index >= 0 {
            if let Some((_, path)) = m.externals.iter().find(|(i, _)| *i == slot.index as u32) { prefab_path = Some(path.clone()); break; }
        }
    }
    if prefab_path.is_none() {
        // any prefab among the externals (the entity model slot may be a variant list whose members are the prefabs)
        prefab_path = m.externals.iter().map(|(_, p)| p.clone()).find(|p| p.ends_with(".Prefab.Gbx"));
    }
    if trace { eprintln!("{logical}: prefab {prefab_path:?}"); }
    let Some(pp) = prefab_path else {
        // the model is inline after all
        return load_model(&std::fs::read(logical).unwrap_or_default()).or_else(|_| Ok(ModelGeom::default()));
    };
    let pm = store.load_model(&pp)?;
    let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
    for e in &pf.ents {
        let Some(Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
        let Some(s2) = so.solid2() else { continue };
        let pose = if e.rot != [0.0, 0.0, 0.0, 1.0] || e.pos != [0.0, 0.0, 0.0] { Some((e.rot, e.pos)) } else { None };
        return Ok(geom_from_solid2(s2, pose));
    }
    Ok(ModelGeom::default())
}

/// Rotate `v` by the quaternion `q` (x, y, z, w).
pub fn quat_rot(q: [f32; 4], v: V3) -> V3 {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let (xx, yy, zz, xy, xz, yz, wx, wy, wz) = (x * x, y * y, z * z, x * y, x * z, y * z, w * x, w * y, w * z);
    [
        (1.0 - 2.0 * (yy + zz)) * v[0] + 2.0 * (xy - wz) * v[1] + 2.0 * (xz + wy) * v[2],
        2.0 * (xy + wz) * v[0] + (1.0 - 2.0 * (xx + zz)) * v[1] + 2.0 * (yz - wx) * v[2],
        2.0 * (xz - wy) * v[0] + 2.0 * (yz + wx) * v[1] + (1.0 - 2.0 * (xx + yy)) * v[2],
    ]
}

/// Hamilton product a·b of two (x, y, z, w) quaternions (apply b, then a).
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// The lights of a solid, in the frame given by `pose` (a (quaternion, position) applied to the light's model-space
/// position and direction).
pub fn solid2_lights(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, pose: Option<([f32; 4], [f32; 3])>) -> Vec<LightDef> {
    solid2_lights_ext(s2, pose, None)
}

/// `solid2_lights` that also follows EXTERNAL light sockets (`u02`: the socket names a `.Light.Gbx` of the packs by external
/// index — the block prefabs' lamps, `TreeGen\RoadBorderSpot`'s `RoadBorderSpot.Light.Gbx`) through `(store, externals)`.
pub fn solid2_lights_ext(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, pose: Option<([f32; 4], [f32; 3])>, ext: Option<(&mut mapgeom::store::DataStore, &[(u32, String)])>) -> Vec<LightDef> {
    let mut out = Vec::new();
    let mut ext = ext;
    for l in &s2.lights {
        let loaded: Option<mapgeom::static_item::light::CPlugLight> = match l.node.inline.as_deref() {
            Some(Node::Light(_)) => None,
            _ => match &mut ext {
                Some((store, externals)) if l.node.index >= 0 => externals.iter().find(|(i, _)| *i == l.node.index as u32).and_then(|(_, path)| mapgeom::static_item::merged::load_light(store, path).ok()),
                _ => None,
            },
        };
        let pl: &mapgeom::static_item::light::CPlugLight = match (l.node.inline.as_deref(), &loaded) {
            (Some(Node::Light(pl)), _) => pl,
            (_, Some(pl)) => pl,
            _ => continue,
        };
        let Some(gx) = pl.gx_light() else { continue };
        let (color, intensity, radius) = gx.summary();
        let mut cone = (180.0f32, 180.0f32);
        let (mut hyper2, mut att_htnlr, mut ball_flags, mut emitting, mut gx_flags, mut radii) = ([0.0f32; 2], [0.0f32; 2], 0u32, [0.0f32; 2], 0u32, [radius; 3]);
        for ch in &gx.chunks {
            use mapgeom::static_item::light::GxChunk as G;
            match ch {
                G::Spot { angle_inner, angle_outer, .. } => cone = (*angle_inner, *angle_outer),
                G::Spot01 { angle_inner, angle_outer, .. } => cone = (*angle_inner, *angle_outer),
                G::Ball08 { flags, radius_specular, radius_shadow, radius_flare, emitting_radius, emitting_cylinder_len_z, att_htnlr: a, att_hyper2, .. } => { hyper2 = *att_hyper2; att_htnlr = *a; ball_flags = *flags; emitting = [*emitting_radius, *emitting_cylinder_len_z]; radii = [*radius_specular, *radius_shadow, *radius_flare]; }
                G::Ball06 { flags, radius_specular, radius_shadow, radius_flare, emitting_radius, attenuation, .. } => { att_htnlr = *attenuation; ball_flags = *flags; emitting = [*emitting_radius, 0.0]; radii = [*radius_specular, *radius_shadow, *radius_flare]; }
                G::Light08 { flags, .. } | G::Light09 { flags, .. } | G::Light0A { flags, .. } => gx_flags = *flags,
                _ => {}
            }
        }
        let t = &l.u05;
        let (mut pos, mut dir) = ([t[9], t[10], t[11]], norm([t[6], t[7], t[8]]));
        let (mut left, mut up) = (norm([t[0], t[1], t[2]]), norm([t[3], t[4], t[5]]));
        if let Some((q, p)) = pose {
            let r = quat_rot(q, pos);
            pos = [r[0] + p[0], r[1] + p[1], r[2] + p[2]];
            dir = quat_rot(q, dir);
            left = quat_rot(q, left);
            up = quat_rot(q, up);
        }
        out.push(LightDef { pos, dir, color, intensity, radius, cone, animated: pl.is_animated(), night_only: pl.night_only(), hyper2, att_htnlr, ball_flags, emitting, gx_flags, radii, left, up });
    }
    out
}

/// THE LIGHTS OF A PREFAB, RECURSIVELY (RE 7, 2026-09-25 — stpad's frame 1): every entity's static object contributes its
/// solid's lights in the entity's pose, and an entity whose model is an EXTERNAL prefab (a block clip's
/// `TreeGen\RoadBorderSpot.Prefab.Gbx` lamp post inside `Water\FCCenter_Air.Prefab.Gbx`) is loaded through the store and
/// walked the same way with the composed pose — the game instantiates nested prefabs as such, so their CPlugLights are
/// local lights of the map like an item's own. `pose` = the outer (quaternion, position); `depth` bounds cycles.
pub fn prefab_lights(store: &mut mapgeom::store::DataStore, pf: &mapgeom::static_item::prefab::CPlugPrefab, externals: &[(u32, String)], pose: Option<([f32; 4], [f32; 3])>, depth: u32) -> Vec<LightDef> {
    let mut out = Vec::new();
    for e in &pf.ents {
        let ent_pose = compose_pose(pose, (e.rot, e.pos));
        match e.model.inline.as_deref() {
            Some(Node::StaticObject(so)) => {
                if let Some(s2) = so.solid2() { out.extend(solid2_lights(s2, Some(ent_pose))); }
            }
            Some(Node::Prefab(inner)) => {
                if depth < 8 { out.extend(prefab_lights(store, inner, externals, Some(ent_pose), depth + 1)); }
            }
            None if e.model.index >= 0 => {
                if let Some((_, path)) = externals.iter().find(|(i, _)| *i == e.model.index as u32) {
                    if path.to_ascii_lowercase().ends_with(".prefab.gbx") && depth < 8 {
                        if let Ok(pm) = store.load_model(path) {
                            if let Ok(inner) = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm) {
                                out.extend(prefab_lights(store, &inner, &pm.externals, Some(ent_pose), depth + 1));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// outer ∘ inner: the inner (q, p) expressed in the outer frame.
pub fn compose_pose(outer: Option<([f32; 4], [f32; 3])>, inner: ([f32; 4], [f32; 3])) -> ([f32; 4], [f32; 3]) {
    match outer {
        None => inner,
        Some((oq, op)) => {
            let r = quat_rot(oq, inner.1);
            (quat_mul(oq, inner.0), [r[0] + op[0], r[1] + op[1], r[2] + op[2]])
        }
    }
}

/// A `ModelGeom` from a solid: the PreLightGen, lights, the LOD-0 shaded geoms' triangles (TexCoord1 = the lightmap uv),
/// materials / cut-out textures, the uv range and the metres-per-uv; `ent_pose` = a prefab entity's (quaternion, position).
/// A `ModelGeom` from a solid: the PreLightGen, lights, the LOD-0 shaded geoms' triangles (TexCoord1 = the lightmap uv),
/// materials / cut-out textures, the uv range and the metres-per-uv; `ent_pose` = a prefab entity's (quaternion, position).
pub fn geom_from_solid2(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, ent_pose: Option<([f32; 4], [f32; 3])>) -> ModelGeom {
    geom_from_solid2_opts(s2, ent_pose, false)
}

/// `geom_from_solid2` with `skip_no_lm_uv`: a visual WITHOUT lightmap uvs (no TexCoord1 / set 1, no tc0 under the terrain
/// rule, no uv set at all) is left out — the record scene's blocks and clips (stpad's WaterBase `Base_Air`: geom 1 is the
/// WATER SURFACE at local y 7 — POSITION + BLENDINDICES + NORMAL, no uv, no LM chart — 3 m above the pool FLOOR (geom 0,
/// the record's LM mesh at y 4); drawn as peel geometry it blacks out every floor texel's sky, while the editor's floor
/// charts are the brightest objects of the map: the game does not peel the water, as it leaves the Scene3d's Water mobil
/// out of the environment block). The port's items keep the fallback (a single-set pad IS lightmap geometry).
pub fn geom_from_solid2_opts(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, ent_pose: Option<([f32; 4], [f32; 3])>, skip_no_lm_uv: bool) -> ModelGeom {
    geom_from_solid2_ext(s2, ent_pose, skip_no_lm_uv, None)
}

/// THE LIGHTMAPPER'S DRAW RULE PER MATERIAL (RE 7 04:20Z / RE 9 §2, the stpad capture): a geom whose material's SHADER
/// carries `B & 0x40000` (never a shadow caster — Water's 0x004c0020) or fails the lightmapper's pass mask, or whose shader
/// is of the VDepLight class (Tech3 GrassFence_VDepLight), is drawn in NO lightmapper pass — not peeled, not a sun-map
/// caster, not lit; Waterground and the single-uv terrain quads stay. `ext` = (store, the model file's external refs) to
/// resolve the geom's material file and its shader (mapgeom::envblock::material_chain); without it the uv rule alone applies.
pub fn geom_from_solid2_ext(s2: &mapgeom::static_item::solid2::CPlugSolid2Model, ent_pose: Option<([f32; 4], [f32; 3])>, skip_no_lm_uv: bool, mut ext: Option<(&mut mapgeom::store::DataStore, &[(u32, String)])>) -> ModelGeom {
    // per material index: excluded by its shader (resolved once per material)
    let mut excluded: std::collections::HashMap<usize, bool> = std::collections::HashMap::new();
    let mut material_excluded = |mi: usize, ext: &mut Option<(&mut mapgeom::store::DataStore, &[(u32, String)])>| -> bool {
        if let Some(v) = excluded.get(&mi) { return *v; }
        let mut v = false;
        if let Some((store, externals)) = ext.as_mut() {
            let path: Option<String> = s2.materials.get(mi).and_then(|r| if r.index >= 0 { externals.iter().find(|(i, _)| *i == r.index as u32).map(|(_, p)| p.clone()) } else { None })
                .or_else(|| s2.custom_materials.get(mi).and_then(|cm| cm.inst().and_then(|m| m.link().map(|l| l.to_string()))));
            if let Some(p) = path {
                let up = p.to_ascii_uppercase();
                let mat = if up.ends_with(".MATERIAL.GBX") { p.clone() } else { format!("{p}.Material.Gbx") };
                let chain = mapgeom::envblock::material_chain(store, &mat);
                let vdep = chain.shader.to_ascii_uppercase().contains("VDEPLIGHT");
                v = vdep || chain.flags.map(|f| f.never_casts() || !f.passes_default_mask()).unwrap_or(false);
                // THE GAME'S OWN TEST (RE 11, 09:20Z): a shader without a PreLightGen binding, or whose pass word lacks 0x1000, gets no
                // TcLM stream — the geom is in NO lightmap pass (Water_MultiH, Warp PyPxzDiff, GrassFence_VDepLight, DecalGeom, TAddModCV,
                // ShadowCaster); this supersedes the never_casts / VDepLight reading above (they agree on every pack material read so far)
                let link_for_lm = mapgeom::static_item::materials::material_link(&mat);
                if let Some(None) = crate::lmmesh::lm_uv_index_cached(&link_for_lm) { v = true; }
                if std::env::var_os("LMTOOL_MATERIAL_TRACE").is_some() { eprintln!("material {mat}: shader {} flags {:?} → {}", chain.shader, chain.flags, if v { "EXCLUDED from the lightmapper" } else { "drawn" }); }
            }
        }
        excluded.insert(mi, v);
        v
    };
    let mut g = ModelGeom::default();
    if let Some(plg) = &s2.pre_light_gen {
        g.plg_u02 = plg.u02;
        if plg.u04[2] > plg.u04[0] && plg.u04[3] > plg.u04[1] && plg.u04[2].is_finite() {
            g.plg_bounds = Some([plg.u04[0], plg.u04[1], plg.u04[2], plg.u04[3]]);
        }
    }
    // the lights follow the entity's pose like the triangles (they were left in the solid's frame before RE7/0015)
    g.lights = solid2_lights(s2, ent_pose);
    if std::env::var_os("LMTOOL_MAT_DEBUG").is_some() {
        eprintln!("solid2: {} materials (deprec {}), {} custom materials, folder {:?}, {} shaded geoms, {} custom material ids", s2.materials.len(), s2.materials_deprec, s2.custom_materials.len(), s2.materials_folder, s2.shaded_geoms.len(), s2.material_ids.len());
        for (i, cm) in s2.custom_materials.iter().enumerate() { eprintln!("  custom[{i}] {:?} → link {:?}", cm.name, cm.inst().and_then(|m| m.link())); }
        for (i, mr) in s2.materials.iter().enumerate() {
            let kind = match mr.inline.as_deref() { Some(Node::Material(m)) => format!("Material link {:?} name {:?}", m.link(), m.main.as_ref().map(|x| format!("{:?}", x.material_name))), Some(Node::OldMaterial(_)) => "OldMaterial".into(), Some(_) => "other node".into(), None => "by index".into() };
            eprintln!("  material[{i}] index {} → {kind}", mr.index);
        }
        for sg in s2.shaded_geoms.iter().take(6) { eprintln!("  shaded geom: visual {} material_index {} lod {}", sg.visual_index, sg.material_index, sg.lod_mask); }
    }
    let (mut aw, mut au) = (0f64, 0f64);
    let (mut umin, mut umax) = ([f32::MAX; 2], [f32::MIN; 2]);
    // every shaded geom's stored visual box, LOD 0 or not, in shaded-geom order (the CPlugTree bbox source)
    for sg in &s2.shaded_geoms {
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
        if let Some(mm) = v.main.as_ref() {
            let b = mm.bounding_box;
            g.stored_boxes_all.push((sg.lod_mask, [b[0], b[1], b[2]], [b[3], b[4], b[5]]));
        }
    }
    for sg in &s2.shaded_geoms {
        // lod 0 only: the lightmap is computed on the highest detail
        if sg.lod_mask > 0 && sg.lod_mask & 1 == 0 {
            continue;
        }
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
        if let Some(mm) = v.main.as_ref() {
            let b = mm.bounding_box;
            if b[3] >= 0.0 && b[3].is_finite() {
                let (lo, hi) = ([b[0] - b[3], b[1] - b[4], b[2] - b[5]], [b[0] + b[3], b[1] + b[4], b[2] + b[5]]);
                g.stored_bbox = Some(match g.stored_bbox { None => (lo, hi), Some((a, c)) => ([a[0].min(lo[0]), a[1].min(lo[1]), a[2].min(lo[2])], [c[0].max(hi[0]), c[1].max(hi[1]), c[2].max(hi[2])]) });
                g.stored_boxes.push(([b[0], b[1], b[2]], [b[3], b[4], b[5]]));
            }
        }
        // the shaded geom's material link → an index into mat_links (deduplicated)
        // (the tiny items carry their materials as `custom_materials` — name + CPlugMaterialUserInst whose
        // link is the game material; `materials` is the older list)
        let link: String = usize::try_from(sg.material_index).ok().and_then(|mi| {
            s2.custom_materials.get(mi).and_then(|cm| cm.inst().and_then(|m| m.link().map(|l| l.to_string())).or_else(|| if cm.name.is_empty() { None } else { Some(cm.name.clone()) }))
                .or_else(|| s2.materials.get(mi).and_then(|mr| match mr.inline.as_deref() { Some(Node::Material(m)) => m.link().map(|l| l.to_string()), _ => None }))
                // a pack prefab's material is an EXTERNAL reference of the file (RE 11's 0003): the link from the reference path
                // (`Stadium\Media\Material\Waterground.Material.Gbx` → `Stadium\Media\Material\Waterground`), so the record scene's
                // models carry real links for the per-link pre-pass constants / textures
                .or_else(|| ext.as_ref().and_then(|(_, externals)| s2.materials.get(mi).and_then(|r| if r.index >= 0 { externals.iter().find(|(i, _)| *i == r.index as u32).map(|(_, p)| mapgeom::static_item::materials::material_link(p)) } else { None })))
        }).unwrap_or_default();
        // the material's cut-out texture: the DiffuseO (slot 1) user texture
        let alpha: u16 = usize::try_from(sg.material_index).ok().and_then(|mi| s2.custom_materials.get(mi)).and_then(|cm| cm.inst()).and_then(|m| m.main.as_ref())
            .and_then(|mm| mm.user_textures.iter().find(|t| t.u01 == 1).map(|t| t.texture.clone()))
            .map(|file| {
                let base = file.rsplit(['/', '\\']).next().unwrap_or(&file).to_string();
                match g.alpha_tex.iter().position(|f| *f == base) {
                    Some(i) => i as u16,
                    None => { g.alpha_tex.push(base); (g.alpha_tex.len() - 1) as u16 }
                }
            })
            .unwrap_or(u16::MAX);
        let diff: u16 = if !link.is_empty() { u16::MAX } else {
            usize::try_from(sg.material_index).ok().and_then(|mi| s2.custom_materials.get(mi)).and_then(|cm| cm.inst()).and_then(|m| m.main.as_ref())
                .and_then(|mm| mm.user_textures.iter().find(|t| t.u01 == 0).map(|t| t.texture.clone()))
                .map(|file| {
                    let base = file.rsplit(['/', '\\']).next().unwrap_or(&file).to_string();
                    match g.diff_tex.iter().position(|f| *f == base) {
                        Some(i) => i as u16,
                        None => { g.diff_tex.push(base); (g.diff_tex.len() - 1) as u16 }
                    }
                })
                .unwrap_or(u16::MAX)
        };
        let mat: u16 = if link.is_empty() {
            u16::MAX
        } else {
            match g.mat_links.iter().position(|l| *l == link) {
                Some(i) => i as u16,
                None => {
                    g.mat_links.push(link.clone());
                    g.mat_albedo.push(crate::albedo::for_link(&link).unwrap_or([f32::NAN; 3]));
                    (g.mat_links.len() - 1) as u16
                }
            }
        };
        let Some(ib) = v.index_buffer.as_ref() else { continue };
        if usize::try_from(sg.material_index).ok().map(|mi| material_excluded(mi, &mut ext)).unwrap_or(false) && std::env::var_os("LMTOOL_KEEP_EXCLUDED_MATERIALS").is_none() {
            continue;
        }
        let Some(st) = v.stream() else { continue };
        let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let Some(Elem::Float3(pos)) = get(N_POSITION) else { continue };
        // the lightmap uv set per material (lmmesh::terrain_material_takes_tc0 — RE 7: the terrain game materials' geoms carry
        // it in TEXCOORD0, everything else in TEXCOORD1)
        let tc0_is_lm = crate::lmmesh::terrain_material_takes_tc0(&link);
        let uv1: Option<&Vec<[f32; 2]>> = match get(if tc0_is_lm { N_TEXCOORD0 } else { N_TEXCOORD0 + 1 }) {
            Some(Elem::Float2(u)) => Some(u),
            _ => None,
        };
        // TexCoord1 may also live in the visual's own tex_coord_sets (set 1)
        let uv1_alt: Option<Vec<[f32; 2]>> = if uv1.is_none() && tc0_is_lm {
            v.main.as_ref().and_then(|m| m.tex_coord_sets.get(0)).map(|s| s.coords.iter().map(|c| c.0).collect())
        } else if uv1.is_none() {
            v.main.as_ref().and_then(|m| m.tex_coord_sets.get(1)).map(|s| s.coords.iter().map(|c| c.0).collect())
        } else {
            None
        };
        if uv1.is_some() || uv1_alt.is_some() {
            g.lm_uv_geoms += 1;
        } else if skip_no_lm_uv {
            continue;
        }
        // no lightmap uvs at all (terrain tiles: positions + normals only; the editor still charts
        // them at the default density): TexCoord0 when there is one, else a planar map over the
        // visual's own footprint — the triangles must exist in any case, they occlude
        let uv_fallback: Option<Vec<[f32; 2]>> = if uv1.is_none() && uv1_alt.is_none() {
            match get(N_TEXCOORD0) {
                Some(Elem::Float2(u)) => Some(u.clone()),
                _ => {
                    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for p in pos.iter() { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
                    let ex = (hi[0] - lo[0]).max(1e-3);
                    let ez = (hi[2] - lo[2]).max(1e-3);
                    Some(pos.iter().map(|p| [(p[0] - lo[0]) / ex, (p[2] - lo[2]) / ez]).collect())
                }
            }
        } else {
            None
        };
        let uv1: Option<&Vec<[f32; 2]>> = uv1.or(uv1_alt.as_ref()).or(uv_fallback.as_ref());
        let Some(uv1) = uv1 else { continue };
        let normals: Option<Vec<V3>> = match get(N_NORMAL) {
            Some(Elem::Float3(n)) => Some(n.clone()),
            Some(Elem::Word(w)) => Some(w.iter().map(|&x| dec3n(x)).collect()),
            _ => None,
        };
        // A visual WITHOUT a normal stream is drawn in no lightmapper pass (RE 7, 03:20Z, the stpad capture: the Stadium Grass
        // tile's 9 880-vertex GrassFence skirt — POSITION + BLENDINDICES + TEXCOORD0, Tech3 GrassFence_VDepLight — is neither
        // lit, nor peeled, nor a sun-map caster; only the 24-index quad is). Skipped here too, so the record scene's peel
        // geometry and the shadow casters carry what the game draws (LMTOOL_KEEP_NO_NORMAL=1 keeps them).
        if normals.is_none() && std::env::var_os("LMTOOL_KEEP_NO_NORMAL").is_none() {
            continue;
        }
        let uv0s: Option<&Vec<[f32; 2]>> = match get(N_TEXCOORD0) { Some(Elem::Float2(u)) => Some(u), _ => None };
        for t in ib.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a >= pos.len() || b >= pos.len() || c >= pos.len() || a >= uv1.len() || b >= uv1.len() || c >= uv1.len() {
                continue;
            }
            let p = [pos[a], pos[b], pos[c]];
            let fnrm = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
            let n = match &normals {
                Some(nv) if a < nv.len() && b < nv.len() && c < nv.len() => [nv[a], nv[b], nv[c]],
                _ => [fnrm, fnrm, fnrm],
            };
            let uv = [uv1[a], uv1[b], uv1[c]];
            for u in &uv {
                umin[0] = umin[0].min(u[0]);
                umin[1] = umin[1].min(u[1]);
                umax[0] = umax[0].max(u[0]);
                umax[1] = umax[1].max(u[1]);
            }
            au += (((uv[1][0] - uv[0][0]) as f64) * ((uv[2][1] - uv[0][1]) as f64) - ((uv[2][0] - uv[0][0]) as f64) * ((uv[1][1] - uv[0][1]) as f64)).abs() / 2.0;
            let cr = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            aw += (dot(cr, cr) as f64).sqrt() / 2.0;
            let uv0 = match uv0s { Some(u) if a < u.len() && b < u.len() && c < u.len() => [u[a], u[b], u[c]], _ => [[0.0; 2]; 3] };
            g.tris.push(Tri { p, n, uv, uv0, mat, alpha: if uv0s.is_some() { alpha } else { u16::MAX }, diff });
        }
    }
    g.metres_per_uv = if au > 1e-9 && aw > 1e-9 { (aw / au).sqrt() as f32 } else { 0.0 };
    g.uv_min = umin;
    g.uv_max = umax;
    // a prefab entity's pose: rotate (quaternion x, y, z, w) and translate the triangles (the stored boxes stay in the
    // entity's frame — the record boxes come from lmtiles::item_records)
    if let Some((q, t)) = ent_pose {
        let rot = |v: V3| -> V3 {
            let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
            let (xx, yy, zz, xy, xz, yz, wx, wy, wz) = (x * x, y * y, z * z, x * y, x * z, y * z, w * x, w * y, w * z);
            [
                (1.0 - 2.0 * (yy + zz)) * v[0] + 2.0 * (xy - wz) * v[1] + 2.0 * (xz + wy) * v[2],
                2.0 * (xy + wz) * v[0] + (1.0 - 2.0 * (xx + zz)) * v[1] + 2.0 * (yz - wx) * v[2],
                2.0 * (xz - wy) * v[0] + 2.0 * (yz + wx) * v[1] + (1.0 - 2.0 * (xx + yy)) * v[2],
            ]
        };
        for tri in g.tris.iter_mut() {
            for k in 0..3 {
                let r = rot(tri.p[k]);
                tri.p[k] = [r[0] + t[0], r[1] + t[1], r[2] + t[2]];
                tri.n[k] = rot(tri.n[k]);
            }
        }
    }
    g
}

/// A map item's placement fields as the file carries them (CGameCtnAnchoredObject): the game builds the mobil's
/// Iso4 from these (RE 4's chain, `lmtiles::item_iso4`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemPose {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub pos: [f32; 3],
    pub pivot: [f32; 3],
    pub scale: f32,
}

#[derive(Clone, Debug)]
pub struct Instance {
    pub item: usize,
    pub model: usize,
    pub xf: mapgeom::geom::Xform,
    pub model_name: String,
    /// The placement fields (zeros / scale 1 for a synthetic instance).
    pub pose: ItemPose,
    /// The item's MapElemLightmapQuality byte (chunk 0x03043068: Normal 0, High 1, VeryHigh 2, Highest 3, Lowest 4,
    /// VeryLow 5, Low 6); 0 when the map has no such chunk.
    pub lm_quality: u8,
}

pub struct Scene {
    pub models: Vec<ModelGeom>,
    pub model_names: Vec<String>,
    pub instances: Vec<Instance>,
    pub item_count: usize,
    /// The decoration's surroundings (the collection's Scene3d: island, sea, invisible shadow casters —
    /// `--decoration FILE.obj`): world-space triangles that occlude and bounce but get no chart. The
    /// BVH carries them with `inst == DECOR_INST` and `tri` indexing this list.
    pub decor: Vec<DecorTri>,
    /// The cut-out masks by texture file name (the map zip's `Items/*.dds` decoded at ≤ 256 px, alpha ≥ 0.5).
    pub alpha_masks: BTreeMap<String, AlphaMask>,
    /// The cut-out textures' mean opaque colour by file (the cards' albedo; sRGB-encoded 0..1).
    pub card_albedo: BTreeMap<String, [f32; 3]>,
    /// The link-less materials' diffuse textures' mean colour by file (their bounce albedo).
    pub tex_albedo: BTreeMap<String, [f32; 3]>,
}

/// LMTOOL_MASK_NOFLIP=1: the cut-out masks in the DDS file's row order (the game flips them: see `opaque`).
pub static MASK_NOFLIP: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_MASK_NOFLIP").map(|v| v == "1").unwrap_or(false));

/// A binary cut-out mask (alpha ≥ threshold) sampled with wrapping uv, nearest texel.
#[derive(Clone, Debug)]
pub struct AlphaMask {
    pub w: usize,
    pub h: usize,
    pub bits: Vec<u8>,
    /// The texture's mean colour over its opaque texels (sRGB-encoded 0..1): the card's albedo.
    pub albedo: [f32; 3],
    /// The full alpha mip chain as the GPU samples it (`alphatex`): the peel's alpha test filters it at
    /// the fragment's level of detail; None = the point-sampled mask above.
    pub tex: Option<std::sync::Arc<crate::alphatex::AlphaTex>>,
}

impl AlphaMask {
    pub fn from_rgba(w: usize, h: usize, rgba: &[u8], threshold: u8) -> AlphaMask {
        let mut bits = vec![0u8; (w * h + 7) / 8];
        let (mut sum, mut n) = ([0f64; 3], 0usize);
        for i in 0..w * h {
            if rgba.get(i * 4 + 3).copied().unwrap_or(255) >= threshold {
                bits[i >> 3] |= 1 << (i & 7);
                for k in 0..3 { sum[k] += rgba[i * 4 + k] as f64 / 255.0; }
                n += 1;
            }
        }
        let albedo = if n > 0 { [(sum[0] / n as f64) as f32, (sum[1] / n as f64) as f32, (sum[2] / n as f64) as f32] } else { [0.3; 3] };
        AlphaMask { w, h, bits, albedo, tex: None }
    }
    /// Fraction of opaque texels.
    pub fn coverage(&self) -> f32 {
        let n: u32 = self.bits.iter().map(|b| b.count_ones()).sum();
        n as f32 / (self.w * self.h).max(1) as f32
    }
    #[inline]
    pub fn opaque(&self, u: f32, v: f32) -> bool {
        let x = ((u.rem_euclid(1.0) * self.w as f32) as usize).min(self.w - 1);
        // THE GAME'S TEXTURE ROWS RUN BOTTOM-UP RELATIVE TO THE DDS FILE (the capture, pwc-day frame 127448:
        // the GPU textures 14585 / 14579 the card shaders sample equal the item zip's VegetPalmTreeSugar_D_in0 /
        // _D DDS with their rows reversed — 100 % of the 128² / 256² texels agree flipped, 64 % unflipped — the
        // engine's DDS loader reads the files bottom-up, the classic ManiaPlanet skin convention): texture v
        // addresses file row (1 − v)·h. LMTOOL_MASK_NOFLIP=1 restores the file order.
        let vv = if *MASK_NOFLIP { v.rem_euclid(1.0) } else { (1.0 - v).rem_euclid(1.0) };
        let y = ((vv * self.h as f32) as usize).min(self.h - 1);
        let i = y * self.w + x;
        self.bits[i >> 3] & (1 << (i & 7)) != 0
    }
}

/// The BVH instance id of the decoration triangles.
pub const DECOR_INST: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub struct DecorTri {
    pub p: [V3; 3],
    /// Bounce albedo (0 = an invisible shadow caster: occludes, gives nothing back).
    pub albedo: [f32; 3],
    /// A water surface: it also mirrors the sky (the peel reads the sky along the reflected direction).
    pub water: bool,
    /// Part of the game's ENVIRONMENT render (sea box, terrain): drawn once with the sky dome as the
    /// peel's first layer (nearest env surface wins, black), never as a geometry layer.
    pub env: bool,
    /// The sea box: only its FAR faces are drawn (the PS discards front faces) — the triangle's winding
    /// is outward, and a face whose outward normal points along the view direction is a far face.
    pub env_far_only: bool,
    /// In the SUN shadow map (RE 9's rule: a decoration leaf casts there only through a caster-flagged shader program or an
    /// alpha slot — ShadowCaster64 yes, the WarpSand terrain patches no; every leaf of the block is in the peels).
    pub sun_caster: bool,
}

/// Load a Wavefront OBJ (v / f lines, polygons fanned; `usemtl NAME` selects the albedo by the
/// material name through `crate::albedo::for_link`, `InvisibleShadowCaster` → 0) into world-space
/// decoration triangles. `scale` and `offset` place it (the Scene3d is authored in metres, origin at
/// the decoration's corner).
pub fn load_obj_decor(path: &str, scale: f32, offset: V3) -> Result<Vec<DecorTri>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut verts: Vec<V3> = Vec::new();
    let mut out = Vec::new();
    let mut albedo = [0.3f32; 3];
    let mut water = false;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let c: Vec<f32> = it.take(3).map(|x| x.parse().unwrap_or(0.0)).collect();
                if c.len() == 3 {
                    verts.push([c[0] * scale + offset[0], c[1] * scale + offset[1], c[2] * scale + offset[2]]);
                }
            }
            Some("usemtl") => {
                let name = it.next().unwrap_or("");
                albedo = if name.to_ascii_lowercase().contains("invisible") { [0.0; 3] } else { crate::albedo::for_link(name).unwrap_or([0.3; 3]) };
                water = name.to_ascii_lowercase().contains("water");
            }
            Some("f") => {
                let idx: Vec<usize> = it.map(|x| x.split('/').next().unwrap_or("0").parse::<i64>().unwrap_or(0)).map(|i| if i < 0 { (verts.len() as i64 + i) as usize } else { (i - 1).max(0) as usize }).collect();
                for k in 1..idx.len().saturating_sub(1) {
                    let (a, b, c) = (idx[0], idx[k], idx[k + 1]);
                    if a < verts.len() && b < verts.len() && c < verts.len() {
                        out.push(DecorTri { p: [verts[a], verts[b], verts[c]], albedo, water, env: false, env_far_only: false, sun_caster: true });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

impl Scene {
    pub fn from_map(path: &str) -> Result<Scene, String> {
        let m = tmmaps::map::MapFile::load(std::path::Path::new(path));
        let files = mapgeom::embedded::files(&m)?;
        // model name -> bytes (the zip keys carry a folder prefix)
        let mut by_name: BTreeMap<String, &Vec<u8>> = BTreeMap::new();
        for (k, v) in &files {
            let base = k.rsplit(['/', '\\']).next().unwrap_or(k).to_string();
            by_name.insert(base, v);
        }
        let mut models = Vec::new();
        let mut model_names = Vec::new();
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        let mut instances = Vec::new();
        let mut missing: BTreeMap<String, usize> = BTreeMap::new();
        // the per-item lightmap quality bytes (chunk 0x03043068: one byte per block, baked block and item, in that order)
        let lm_quality: Vec<u8> = tmmaps::gbx::all_skip_chunks(&m.gbx.body).iter().find(|(c, ..)| *c == 0x0304_3068).map(|&(_, _, payload, size)| {
            let start = payload + 4 + m.blocks.len() + m.baked.len();
            m.gbx.body[start.min(payload + size)..(payload + size).min(start + m.items.len())].to_vec()
        }).unwrap_or_default();
        // the game's STOCK items (a placed item whose model the map does not embed: `Screen2x1Small`, …) live in the packs —
        // LMTOOL_STOCK_PAKS=FILE:KEY[,FILE:KEY…] names the packs searched under `<Collection>\Items\<name>.Item.Gbx` (the map's
        // collection first, then Stadium, the shared library)
        let mut stock_store: Option<mapgeom::store::DataStore> = std::env::var("LMTOOL_STOCK_PAKS").ok().map(|v| {
            let mut st = mapgeom::store::DataStore::empty();
            for spec in v.split(',').filter(|s| !s.is_empty()) {
                if let Some((f, k)) = spec.rsplit_once(':') { if let Err(e) = st.add_pak(f, k) { eprintln!("LMTOOL_STOCK_PAKS {f}: {e}"); } }
            }
            st
        });
        let collection_name: String = { let c = m.items.first().map(|it| it.collection_raw).unwrap_or(0x1a); match c { 0x1a => "Stadium", 0x1c => "BlueBay", 0x10 => "RedIsland", 0x1d => "WhiteShore", 0xf => "GreenCoast", _ => "Stadium" }.to_string() };
        let mut stock_bytes: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for (i, it) in m.items.iter().enumerate() {
            let mi = match index.get(&it.model) {
                Some(&k) => k,
                None => {
                    // embedded: the zip's bytes; else a stock item through the packs (its model is an external prefab)
                    let geom: Option<ModelGeom> = match by_name.get(&it.model) {
                        Some(bytes) => Some(load_model(bytes).map_err(|e| format!("{}: {e}", it.model))?),
                        None => {
                            let mut found = None;
                            if let Some(st) = stock_store.as_mut() {
                                let name = it.model.trim_end_matches(".Item.Gbx");
                                for coll in [collection_name.as_str(), "Stadium"] {
                                    let logical = format!("{coll}\\Items\\{name}.Item.Gbx");
                                    match load_model_from_store(st, &logical) {
                                        Ok(g) => { stock_bytes.insert(it.model.clone(), Vec::new()); found = Some(g); break; }
                                        Err(_) => continue,
                                    }
                                }
                            }
                            found
                        }
                    };
                    match geom {
                        Some(g) => {
                            models.push(g);
                            model_names.push(it.model.clone());
                            index.insert(it.model.clone(), models.len() - 1);
                            models.len() - 1
                        }
                        None => {
                            *missing.entry(it.model.clone()).or_insert(0) += 1;
                            continue;
                        }
                    }
                }
            };
            let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
            let pose = ItemPose { yaw: it.yaw, pitch: it.pitch, roll: it.roll, pos: it.pos, pivot: it.pivot, scale: it.scale };
            instances.push(Instance { item: i, model: mi, xf, model_name: it.model.clone(), pose, lm_quality: lm_quality.get(i).copied().unwrap_or(0) });
        }
        // the cut-out masks of the alpha-tested materials (the zip's Items/*.dds by base name)
        let mut alpha_masks: BTreeMap<String, AlphaMask> = BTreeMap::new();
        let mut card_albedo: BTreeMap<String, [f32; 3]> = BTreeMap::new();
        // the link-less materials' diffuse textures → their mean colour (sRGB-encoded 0..1)
        let mut tex_albedo: BTreeMap<String, [f32; 3]> = BTreeMap::new();
        for g in &models {
            for file in &g.diff_tex {
                if tex_albedo.contains_key(file) { continue; }
                let Some(bytes) = by_name.get(file) else { continue };
                if let Ok((w, h, rgba)) = mapgeom::static_item::texture::decode_capped_rgba(bytes, 64) {
                    let m = AlphaMask::from_rgba(w as usize, h as usize, &rgba, 128);
                    tex_albedo.insert(file.clone(), m.albedo);
                }
            }
        }
        if !tex_albedo.is_empty() {
            let mut v: Vec<(&String, &[f32; 3])> = tex_albedo.iter().collect();
            v.sort_by(|a, b| a.0.cmp(b.0));
            eprintln!("  {} diffuse textures of link-less materials → bounce albedo: {}…", tex_albedo.len(), v.iter().take(4).map(|(k, c)| format!("{k} ({:.2},{:.2},{:.2})", c[0], c[1], c[2])).collect::<Vec<_>>().join(", "));
        }
        for g in &models {
            for file in &g.alpha_tex {
                if alpha_masks.contains_key(file) { continue; }
                let Some(bytes) = by_name.get(file) else { continue };
                // LMTOOL_MASK_RES=N: the cut-out decided on the texture reduced to N×N (its alpha box-averaged
                // — the game's peel samples the leaf texture at the peel's pixel footprint, i.e. a coarse mip,
                // and a sparse leaf texture passes the alpha test far less often there); default 256 = full
                let mask_res: usize = std::env::var("LMTOOL_MASK_RES").ok().and_then(|v| v.parse().ok()).unwrap_or(256);
                match mapgeom::static_item::texture::decode_capped_rgba(bytes, 256) {
                    Ok((w, h, rgba)) => {
                        let (w, h, rgba) = if mask_res < (w as usize).min(h as usize) {
                            // box-average down to mask_res × mask_res (alpha and colour)
                            let (nw, nh) = (mask_res, mask_res);
                            let mut out = vec![0u8; nw * nh * 4];
                            for y in 0..nh { for x in 0..nw {
                                let (x0, x1) = (x * w as usize / nw, ((x + 1) * w as usize / nw).max(x * w as usize / nw + 1));
                                let (y0, y1) = (y * h as usize / nh, ((y + 1) * h as usize / nh).max(y * h as usize / nh + 1));
                                let mut acc = [0u64; 4]; let mut n = 0u64;
                                for yy in y0..y1 { for xx in x0..x1 { let i = (yy * w as usize + xx) * 4; for k in 0..4 { acc[k] += rgba[i + k] as u64; } n += 1; } }
                                let o = (y * nw + x) * 4;
                                for k in 0..4 { out[o + k] = (acc[k] / n.max(1)) as u8; }
                            } }
                            (nw as u32, nh as u32, out)
                        } else { (w, h, rgba) };
                        // LMTOOL_MASK_THRESHOLD=T (0..255, default 128): the alpha a texel needs to count as leaf
                        let thr: u8 = std::env::var("LMTOOL_MASK_THRESHOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(128);
                        let mut m = AlphaMask::from_rgba(w as usize, h as usize, &rgba, thr);
                        // the whole mip chain for the filtered alpha test (the game's rows run bottom-up: `opaque`)
                        m.tex = match crate::alphatex::AlphaTex::from_dds(bytes, !*MASK_NOFLIP) { Ok(t) => Some(std::sync::Arc::new(t)), Err(e) => { eprintln!("  alpha texture {file}: {e} (point-sampled mask only)"); None } };
                        card_albedo.insert(file.clone(), m.albedo);
                        // a mask that cuts nothing is not worth the lookups
                        if m.coverage() < 0.999 { alpha_masks.insert(file.clone(), m); }
                    }
                    Err(e) => eprintln!("  alpha mask {file}: {e}"),
                }
            }
        }
        if !alpha_masks.is_empty() {
            let cov: Vec<String> = alpha_masks.iter().take(4).map(|(k, m)| format!("{k} {:.0} %", m.coverage() * 100.0)).collect();
            eprintln!("  {} cut-out masks (alpha-tested materials): {}…", alpha_masks.len(), cov.join(", "));
        }
        if !missing.is_empty() {
            eprintln!("  {} item models are not embedded (stock items?): {:?}", missing.len(), missing.iter().take(8).collect::<Vec<_>>());
        }
        Ok(Scene { models, model_names, instances, item_count: m.items.len(), decor: Vec::new(), alpha_masks, card_albedo, tex_albedo })
    }

    pub fn tri_count(&self) -> usize {
        self.instances.iter().map(|i| self.models[i.model].tris.len()).sum()
    }

    /// Every light of every instance in world space (position, direction, and
    /// the radius scaled like the instance).
    pub fn world_lights(&self) -> Vec<(usize, LightDef)> {
        let mut out = Vec::new();
        for (ii, inst) in self.instances.iter().enumerate() {
            let m = &self.models[inst.model];
            let scale = {
                let c0 = [inst.xf[0], inst.xf[1], inst.xf[2]];
                dot(c0, c0).sqrt()
            };
            for l in &m.lights {
                let mut w = *l;
                w.pos = xf_point(&inst.xf, l.pos);
                w.dir = xf_normal(&inst.xf, l.dir);
                w.left = xf_normal(&inst.xf, l.left);
                w.up = xf_normal(&inst.xf, l.up);
                w.radius = l.radius * scale;
                out.push((ii, w));
            }
        }
        out
    }
}

/// Transform a point and a normal (rotation part only; assumes uniform scale).
pub fn identity_xf() -> mapgeom::geom::Xform {
    mapgeom::geom::IDENTITY
}

pub fn xf_point(m: &mapgeom::geom::Xform, v: V3) -> V3 {
    mapgeom::geom::apply(m, v)
}
pub fn xf_normal(m: &mapgeom::geom::Xform, n: V3) -> V3 {
    norm([
        m[0] * n[0] + m[3] * n[1] + m[6] * n[2],
        m[1] * n[0] + m[4] * n[1] + m[7] * n[2],
        m[2] * n[0] + m[5] * n[1] + m[8] * n[2],
    ])
}
