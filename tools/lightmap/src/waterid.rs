//! THE WATER-ID MAP FROM THE MAP (RE child 11, 2026-09-26) — the `SetWaterId` pass of the attribute pre-pass
//! (pwc-day frame 127447 eid 12420: VS 17011 / PS 17012, one `DrawIndexedInstanced` of every WATER QUAD of the scene
//! into the R8G8_UINT id map, no depth, NoCull) rebuilt from the map's records, and the plane table it indexes.
//!
//! What the game draws (client-re/NOTES.md 2026-09-26 05:35Z): per record with the water-visual flag (rec+0x118 & 2) its
//! water visuals, placed by the record's Iso4; the vertex shader maps the world point to `WorldToHPos` = (x·2/W − 1,
//! z·2/H − 1) → the W × H viewport (one texel per metre over the map: pwc-day 2048², `World_To_i2WaterId` = (x, H − z));
//! the pixel shader writes `(BLENDINDICES.x + 1, TEXCOORD7.x)` = (the water TYPE of the quad's vertices + 1, the
//! PLANE index carried by the instance). `BlendWaterFog` (PS 17018) then reads `g_WaterTop_ByPlanes[plane]` for the
//! surface height and `g_WaterDepth_FogMaxDepthInv_ByIds[type]` for the depth / fog scale.
//!
//! A water quad = a shaded geom whose material's shader is the water shader (`Tech3_Water_MultiH`: `B & 0x40000`, the
//! never-casts bit, and "Water" in the shader path — Stadium\Media\Material\Water; NOT Waterground / Underwater /
//! WaterBorders / PoolBorders, whose shaders are ordinary block shaders); its vertex stream is POSITION + BLENDINDICES
//! (name 4, Int32: the type in the low byte) + NORMAL + COLOR0, no texcoord — which is why `lmmesh` drops it from the
//! LM mesh (no lightmap uvs) and why it is not peeled (RE 7 04:20Z). Stadium's `Water\Base_Air.Prefab.Gbx` entity 0
//! carries one: 9 vertices at local y 7, 8 triangles over the 32 × 32 m block → world y = the block's origin + 7 (stpad:
//! 23). The plane table = the distinct world heights of the quads (ascending; the game's own plane order is not read —
//! only the pairing of the id map's plane index with `g_WaterTop_ByPlanes` matters).

use crate::passdiff::Buf;
use crate::records::Rec;
use mapgeom::geom::Xform;
use mapgeom::static_item::solid2::CPlugSolid2Model;
use mapgeom::static_item::vstream::Elem;
use mapgeom::static_item::Node;
use mapgeom::store::DataStore;

/// The vertex-declaration name of BLENDINDICES in a `CPlugVertexStream` (POSITION 0, NORMAL 5, COLOR0 8, TEXCOORD0 10).
pub const N_BLENDINDICES: u32 = 4;

/// One water quad of the scene, in world space: its triangles and the water type its vertices carry.
#[derive(Clone, Debug)]
pub struct WaterQuad {
    pub tris: Vec<[[f32; 3]; 3]>,
    /// BLENDINDICES.x of the vertices (0 on stpad / pwc-day: the collection's first water type).
    pub water_type: u32,
    /// The record it came from (its index in the record list).
    pub record: usize,
    /// The prefab file and entity.
    pub source: String,
}

impl WaterQuad {
    /// The surface height: the mean vertex y (a water quad is horizontal).
    pub fn top(&self) -> f32 {
        let mut s = 0.0f64;
        let mut n = 0usize;
        for t in &self.tris {
            for v in t {
                s += v[1] as f64;
                n += 1;
            }
        }
        if n == 0 { 0.0 } else { (s / n as f64) as f32 }
    }
}

/// The id map and its plane table.
#[derive(Clone, Debug)]
pub struct WaterIdMap {
    /// W × H × 2 (channel 0 = type + 1, channel 1 = the plane index), one texel per metre, row 0 = z = H.
    pub ids: Buf,
    /// `g_WaterTop_ByPlanes[i].x`: the distinct quad heights, ascending.
    pub plane_tops: Vec<f32>,
    /// Texels that carry an id.
    pub texels: usize,
    pub quads: usize,
    pub notes: Vec<String>,
}

/// Whether a material link names the water plane's material: its shader chain is the water shader (`never_casts`
/// AND "water" in the shader path); with no chain loadable, a link whose last path component is exactly `Water`.
pub fn is_water_material(store: &mut DataStore, link: &str) -> bool {
    if link.is_empty() {
        return false;
    }
    let path = if link.to_ascii_lowercase().ends_with(".material.gbx") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let chain = mapgeom::envblock::material_chain(store, &path);
    match chain.flags {
        Some(f) => f.never_casts() && chain.shader.to_ascii_lowercase().contains("water"),
        None => link.rsplit(['\\', '/']).next().map(|s| s.eq_ignore_ascii_case("water")).unwrap_or(false),
    }
}

/// The material link of a shaded geom (the custom material's instance link, else the plain material ref's link; a pack
/// prefab references its materials as EXTERNAL files — the node index resolves through the file's reference table).
pub fn geom_material_link(s2: &CPlugSolid2Model, material_index: i32, externals: &[(u32, String)]) -> String {
    usize::try_from(material_index).ok().and_then(|mi| {
        s2.custom_materials.get(mi).and_then(|cm| cm.inst().and_then(|m| m.link().map(|l| l.to_string())).or_else(|| if cm.name.is_empty() { None } else { Some(cm.name.clone()) }))
            .or_else(|| s2.materials.get(mi).and_then(|mr| match mr.inline.as_deref() {
                Some(Node::Material(m)) => m.link().map(|l| l.to_string()),
                _ => externals.iter().find(|(i, _)| *i as i32 == mr.index).map(|(_, p)| mapgeom::static_item::materials::material_link(p)),
            }))
    }).unwrap_or_default()
}

/// The water quads of one Solid2Model placed by `xf`: every shaded geom whose material passes `is_water` (the LOD-0
/// visuals — a quad of lod mask with bit 0; the game's SetWaterId draw uses the record's water visuals, the LOD-0 pair on
/// Base_Air), as world triangles with the vertices' BLENDINDICES.x.
pub fn water_quads_of_solid(s2: &CPlugSolid2Model, externals: &[(u32, String)], xf: &Xform, record: usize, source: &str, is_water: &mut dyn FnMut(&str) -> bool) -> Vec<WaterQuad> {
    let mut out = Vec::new();
    for sg in &s2.shaded_geoms {
        if sg.lod_mask != 0 && sg.lod_mask & 1 == 0 {
            continue; // a LOD-1+ copy of the plane
        }
        let link = geom_material_link(s2, sg.material_index, externals);
        if !is_water(&link) {
            continue;
        }
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(Node::Visual(vis)) = vr.inline.as_deref() else { continue };
        let Some(st) = vis.stream() else { continue };
        let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let Some(Elem::Float3(pos)) = get(mapgeom::static_item::vstream::N_POSITION) else { continue };
        let water_type = match get(N_BLENDINDICES) {
            Some(Elem::Word(w)) => w.first().map(|x| x & 0xff).unwrap_or(0),
            Some(Elem::Raw { size, bytes }) if *size >= 1 => bytes.first().copied().unwrap_or(0) as u32,
            _ => 0,
        };
        let Some(ib) = vis.index_buffer.as_ref() else { continue };
        let world: Vec<[f32; 3]> = pos.iter().map(|p| mapgeom::geom::apply(xf, *p)).collect();
        let mut tris = Vec::new();
        for t in ib.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a < world.len() && b < world.len() && c < world.len() {
                tris.push([world[a], world[b], world[c]]);
            }
        }
        if !tris.is_empty() {
            out.push(WaterQuad { tris, water_type, record, source: format!("{source} geom v{}", sg.visual_index) });
        }
    }
    out
}

/// The water quads of a record list: every record with a prefab-entity mesh source (`Rec.mesh`) contributes its
/// entity's water geoms placed by the record's transform. Prefab entities are loaded once.
pub fn water_quads_of_records(store: &mut DataStore, recs: &[Rec]) -> Result<(Vec<WaterQuad>, Vec<String>), String> {
    let mut notes = Vec::new();
    let mut is_water_cache: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    // (prefab, entity) → the entity's Solid2 water geoms in LOCAL space, or None when it has none
    let mut local: std::collections::HashMap<(String, usize), Option<Vec<WaterQuad>>> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for (k, r) in recs.iter().enumerate() {
        let Some(m) = &r.mesh else { continue };
        let key = (m.prefab.clone(), m.entity);
        if !local.contains_key(&key) {
            let pm = store.load_model(&m.prefab)?;
            let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
            let quads = pf.ents.get(m.entity).and_then(|e| match e.model.inline.as_deref() { Some(Node::StaticObject(so)) => so.solid2(), _ => None }).map(|s2| {
                let mut is_water = |link: &str| -> bool {
                    if let Some(v) = is_water_cache.get(link) {
                        return *v;
                    }
                    let v = is_water_material(store, link);
                    is_water_cache.insert(link.to_string(), v);
                    v
                };
                water_quads_of_solid(s2, &pm.externals, &mapgeom::geom::IDENTITY, 0, &format!("{} ent {}", m.prefab, m.entity), &mut is_water)
            });
            let quads = match quads { Some(q) if !q.is_empty() => Some(q), _ => None };
            if let Some(q) = &quads {
                notes.push(format!("{} entity {}: {} water quad(s) ({} triangles, type {}) — record class {}", m.prefab, m.entity, q.len(), q.iter().map(|w| w.tris.len()).sum::<usize>(), q[0].water_type, r.class));
            }
            local.insert(key.clone(), quads);
        }
        if let Some(Some(qs)) = local.get(&key) {
            for q in qs {
                let tris = q.tris.iter().map(|t| [mapgeom::geom::apply(&m.xf, t[0]), mapgeom::geom::apply(&m.xf, t[1]), mapgeom::geom::apply(&m.xf, t[2])]).collect();
                out.push(WaterQuad { tris, water_type: q.water_type, record: k, source: q.source.clone() });
            }
        }
    }
    Ok((out, notes))
}

/// The SetWaterId raster: the quads' triangles top-down into a `size_m` (W × H) id map at one texel per metre —
/// viewport (x, H − z) as `WorldToHPos` (x·2/W − 1, z·2/H − 1) and the viewport transform give — with the D3D11
/// rasteriser (`prepass::raster_tri`), no depth test (the last quad drawn wins a texel, as the game's draw order does),
/// writing (type + 1, plane). The plane table = the quads' distinct heights (to the millimetre), ascending.
pub fn water_id_map(quads: &[WaterQuad], size_m: [f32; 2]) -> WaterIdMap {
    let (w, h) = (size_m[0].round().max(1.0) as u32, size_m[1].round().max(1.0) as u32);
    let mut ids = Buf::new(w, h, 2);
    let mut plane_tops: Vec<f32> = Vec::new();
    let key = |y: f32| (y * 1000.0).round() as i64;
    for q in quads {
        let t = q.top();
        if !plane_tops.iter().any(|p| key(*p) == key(t)) {
            plane_tops.push(t);
        }
    }
    plane_tops.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut texels = 0usize;
    for q in quads {
        let plane = plane_tops.iter().position(|p| key(*p) == key(q.top())).unwrap_or(0) as f32;
        let id1 = (q.water_type + 1) as f32;
        for t in &q.tris {
            let p = [[t[0][0], h as f32 - t[0][2]], [t[1][0], h as f32 - t[1][2]], [t[2][0], h as f32 - t[2][2]]];
            crate::prepass::raster_tri(p, w, h, |x, y, _| {
                if ids.get(x, y, 0) == 0.0 {
                    texels += 1;
                }
                ids.set(x, y, 0, id1);
                ids.set(x, y, 1, plane);
            });
        }
    }
    let notes = vec![format!("water-id map {w} × {h}: {} quads, {texels} texels carry an id, plane tops {:?}", quads.len(), plane_tops)];
    WaterIdMap { ids, plane_tops, texels, quads: quads.len(), notes }
}

/// `water_quads_of_records` + `water_id_map` for a record list.
pub fn water_id_map_of_records(store: &mut DataStore, recs: &[Rec], size_m: [f32; 2]) -> Result<WaterIdMap, String> {
    let (quads, notes) = water_quads_of_records(store, recs)?;
    let mut m = water_id_map(&quads, size_m);
    m.notes.splice(0..0, notes);
    Ok(m)
}

/// The id map as an RGB image (type in R as 64·id, plane in G as 64·(plane + 1), B = 255 where an id is set).
pub fn id_map_rgb(m: &WaterIdMap) -> Vec<u8> {
    let (w, h) = (m.ids.w, m.ids.h);
    let mut px = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let id = m.ids.get(x, y, 0);
            if id > 0.0 {
                let i = ((y * w + x) * 3) as usize;
                px[i] = (id * 64.0).min(255.0) as u8;
                px[i + 1] = ((m.ids.get(x, y, 1) + 1.0) * 64.0).min(255.0) as u8;
                px[i + 2] = 255;
            }
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(x0: f32, z0: f32, side: f32, y: f32, ty: u32) -> WaterQuad {
        let (x1, z1) = (x0 + side, z0 + side);
        WaterQuad { tris: vec![[[x0, y, z0], [x1, y, z0], [x1, y, z1]], [[x0, y, z0], [x1, y, z1], [x0, y, z1]]], water_type: ty, record: 0, source: "test".into() }
    }

    #[test]
    fn a_pool_block_covers_its_32_by_32_footprint_top_down() {
        // one WaterBase block at cell (2, 3) of a 1536 m map, water at world 23
        let m = water_id_map(&[quad(64.0, 96.0, 32.0, 23.0, 0)], [1536.0, 1536.0]);
        assert_eq!(m.texels, 1024);
        assert_eq!(m.plane_tops, vec![23.0]);
        // texel (x, H − z): world x 64..96 → columns 64..95; world z 96..128 → rows 1536 − 128 .. 1536 − 96
        assert_eq!(m.ids.get(64, 1536 - 128, 0), 1.0);
        assert_eq!(m.ids.get(95, 1536 - 97, 0), 1.0);
        assert_eq!(m.ids.get(63, 1536 - 100, 0), 0.0);
        assert_eq!(m.ids.get(96, 1536 - 100, 0), 0.0);
        assert_eq!(m.ids.get(80, 1536 - 96, 0), 0.0, "the row at z = 96 is the quad's lower edge (top-left rule)");
        assert_eq!(m.ids.get(80, 1536 - 129, 0), 0.0);
        assert_eq!(m.ids.get(80, 1536 - 110, 1), 0.0, "plane 0");
    }

    #[test]
    fn two_heights_make_two_planes_and_the_type_is_plus_one() {
        let m = water_id_map(&[quad(0.0, 0.0, 32.0, 23.0, 0), quad(32.0, 0.0, 32.0, 31.0, 1), quad(64.0, 0.0, 32.0, 23.0004, 0)], [96.0, 32.0]);
        assert_eq!(m.plane_tops.len(), 2);
        assert_eq!(m.plane_tops[0], 23.0);
        assert_eq!(m.plane_tops[1], 31.0);
        assert_eq!(m.texels, 3 * 1024);
        assert_eq!((m.ids.get(10, 10, 0), m.ids.get(10, 10, 1)), (1.0, 0.0));
        assert_eq!((m.ids.get(40, 10, 0), m.ids.get(40, 10, 1)), (2.0, 1.0));
        assert_eq!((m.ids.get(70, 10, 0), m.ids.get(70, 10, 1)), (1.0, 0.0), "a height within a millimetre is the same plane");
    }

    #[test]
    fn no_quads_is_an_empty_map() {
        let m = water_id_map(&[], [64.0, 64.0]);
        assert_eq!(m.texels, 0);
        assert!(m.plane_tops.is_empty());
    }

    /// Stadium's `Water\Base_Air.Prefab.Gbx` entity 0 (the WaterBase block's solid): one water quad (visual 1, 9 vertices
    /// at local y 7, 8 triangles over 32 × 32), material Stadium\Media\Material\Water (Tech3_Water_MultiH, never_casts);
    /// placed at a block origin (64, 16, 96) it is the plane at world 23 over 1024 texels. Skipped without the paks.
    #[test]
    fn stadium_base_air_has_one_water_quad_at_local_7() {
        let dir = std::env::var("TM_PAKS").unwrap_or_else(|_| format!("{}/persistent/private-30d/tm-paks", std::env::var("HOME").unwrap_or_default()));
        let (st, mp) = (format!("{dir}/Stadium.pak"), format!("{dir}/Maniaplanet.pak"));
        if !std::path::Path::new(&st).is_file() || !std::path::Path::new(&mp).is_file() {
            eprintln!("skipped: {st} / {mp} not on this box");
            return;
        }
        let mut store = DataStore::empty();
        store.add_pak(&st, "B773D73047A4104857722366D78D28A6").unwrap();
        store.add_pak(&mp, "9A93723447347A8CE336CCFC49E65449").unwrap();
        assert!(is_water_material(&mut store, "Stadium\\Media\\Material\\Water"));
        assert!(!is_water_material(&mut store, "Stadium\\Media\\Material\\Waterground"));
        assert!(!is_water_material(&mut store, "Stadium\\Media\\Material\\Underwater"));
        assert!(!is_water_material(&mut store, "Stadium\\Media\\Material\\PoolBorders"));
        let prefab = "Stadium\\Media\\Prefab\\Water\\Base_Air.Prefab.Gbx";
        let mut xf = mapgeom::geom::IDENTITY;
        xf[9] = 64.0;
        xf[10] = 16.0;
        xf[11] = 96.0;
        let rec = Rec { class: "block", obj: 0, sub: 0, meter_by_uv: 1.0, uv: [0.0, 0.0, 1.0, 1.0], quality: 1.0, centre: [0.0; 3], half: [0.0; 3], group: 0, key_centre: None, pos_rank: None, wall: None, item: None, scale: 1.0, mesh: Some(crate::records::MeshRef { prefab: prefab.into(), entity: 0, xf }) };
        let (quads, notes) = water_quads_of_records(&mut store, &[rec.clone(), rec]).unwrap();
        for n in &notes {
            eprintln!("{n}");
        }
        assert_eq!(quads.len(), 2, "one quad per record instance");
        assert_eq!(quads[0].tris.len(), 8);
        assert_eq!(quads[0].water_type, 0);
        assert!((quads[0].top() - 23.0).abs() < 1e-4, "top {}", quads[0].top());
        for t in &quads[0].tris {
            for v in t {
                assert!((v[1] - 23.0).abs() < 1e-4);
                assert!(v[0] >= 64.0 - 1e-3 && v[0] <= 96.0 + 1e-3 && v[2] >= 96.0 - 1e-3 && v[2] <= 128.0 + 1e-3, "{v:?}");
            }
        }
        let m = water_id_map(&quads, [1536.0, 1536.0]);
        assert_eq!(m.texels, 1024);
        assert_eq!(m.plane_tops, vec![quads[0].top()]);
    }
}

/// The game's water-id map is always `ID_MAP_SIZE` × `ID_MAP_SIZE` texels (0x1402255a0: FUN_1403fc8f0(tex, {0x800, 0x800, 1}, R8G8_UINT)).
pub const ID_MAP_SIZE: u32 = 2048;

/// One tile of the water-id grid: the world XZ range it covers and its id map (the SetWaterId raster of the quads clipped to it).
#[derive(Clone, Debug)]
pub struct WaterTile {
    pub ix: u32,
    pub iz: u32,
    pub world_min: [f32; 2],
    pub world_max: [f32; 2],
    pub ids: Buf,
}

impl WaterTile {
    /// `World_To_i2WaterId` as the two DXBC registers VS 17017 reads: o1.x = dp4((x, y, z, 1), r0), o1.y = dp4(…, r1) — texel
    /// (x − minX)·sx, (maxZ − z)·sz (captured stpad tile 0: sx 0.65699, sz −1.31799, translation (21.0236, 2018.26)).
    pub fn world_to_id(&self) -> [[f32; 4]; 2] {
        let sx = ID_MAP_SIZE as f32 / (self.world_max[0] - self.world_min[0]);
        let sz = ID_MAP_SIZE as f32 / (self.world_max[1] - self.world_min[1]);
        [[sx, 0.0, 0.0, -self.world_min[0] * sx], [0.0, 0.0, -sz, self.world_max[1] * sz]]
    }
}

/// THE GRID (0x140225b85–0x140225c3c, transcribed): over the scene box {c, h} (computeParams+0xb0, the records' fold) with the full
/// extents ex = 2·hx, ez = 2·hz: `r(e) = (ceil(e) + 255) & ~255` (the extent in metres rounded up to a multiple of 256 = the texel
/// count a 1 m/texel map would need); nx = min(8, ceil(ex / min(r(ex), 4096))), nz = min(8, ceil(ez / min(r(ez), 2048))) — the two
/// caps differ (X 4096, Z 2048); each tile spans (ex/nx) × (ez/nz) metres on the 2048² map. pwc-day (2048 × 2048): 1 × 1 at
/// 1 texel/m; stpad (3117.25 × 3107.76): 1 × 2 at (0.657, 1.318) texel/m — the captured `World_To_i2WaterId` / `WorldMinXZ`.
pub fn water_grid(hx: f32, hz: f32) -> (u32, u32) {
    let r = |e: f32| ((e.ceil() as i64 + 255) & !255) as f32;
    let (ex, ez) = (hx + hx, hz + hz);
    let nx = (ex / r(ex).min(4096.0)).ceil() as u32;
    let nz = (ez / r(ez).min(2048.0)).ceil() as u32;
    (nx.clamp(1, 8), nz.clamp(1, 8))
}

/// The tiles of a scene box: tile (ix, iz) is centred on ((2·ix + 1)·hx/nx − hx + cx, (2·iz + 1)·hz/nz − hz + cz) with half extents
/// (hx/nx, hz/nz) (0x140226260 l.157–160), in the game's draw order (x fastest).
pub fn water_tiles_of_box(c: [f32; 3], h: [f32; 3]) -> Vec<WaterTile> {
    let (nx, nz) = water_grid(h[0], h[2]);
    let (tx, tz) = (h[0] / nx as f32, h[2] / nz as f32);
    let mut out = Vec::new();
    for iz in 0..nz {
        for ix in 0..nx {
            let cx = ((ix + ix + 1) as f32) * tx - h[0] + c[0];
            let cz = ((iz + iz + 1) as f32) * tz - h[2] + c[2];
            out.push(WaterTile { ix, iz, world_min: [cx - tx, cz - tz], world_max: [cx + tx, cz + tz], ids: Buf::new(ID_MAP_SIZE, ID_MAP_SIZE, 2) });
        }
    }
    out
}

/// The water-id grid of a scene: the plane table + per tile the SetWaterId raster of every quad (`WorldToHPos` = the tile's box →
/// NDC, texel (x − minX)·sx, (maxZ − z)·sz; the last quad wins; quads outside the tile fall off the viewport).
pub struct WaterIdTiles {
    pub tiles: Vec<WaterTile>,
    pub plane_tops: Vec<f32>,
    pub texels: usize,
    pub quads: usize,
    pub notes: Vec<String>,
}

pub fn water_id_tiles(quads: &[WaterQuad], c: [f32; 3], h: [f32; 3]) -> WaterIdTiles {
    let mut tiles = water_tiles_of_box(c, h);
    let mut plane_tops: Vec<f32> = Vec::new();
    let key = |y: f32| (y * 1000.0).round() as i64;
    for q in quads {
        let t = q.top();
        if !plane_tops.iter().any(|p| key(*p) == key(t)) {
            plane_tops.push(t);
        }
    }
    plane_tops.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut texels = 0usize;
    let n = ID_MAP_SIZE as f32;
    for tile in tiles.iter_mut() {
        let sx = n / (tile.world_max[0] - tile.world_min[0]);
        let sz = n / (tile.world_max[1] - tile.world_min[1]);
        for q in quads {
            let plane = plane_tops.iter().position(|p| key(*p) == key(q.top())).unwrap_or(0) as f32;
            let id1 = (q.water_type + 1) as f32;
            for t in &q.tris {
                let p = [
                    [(t[0][0] - tile.world_min[0]) * sx, (tile.world_max[1] - t[0][2]) * sz],
                    [(t[1][0] - tile.world_min[0]) * sx, (tile.world_max[1] - t[1][2]) * sz],
                    [(t[2][0] - tile.world_min[0]) * sx, (tile.world_max[1] - t[2][2]) * sz],
                ];
                crate::prepass::raster_tri(p, ID_MAP_SIZE, ID_MAP_SIZE, |x, y, _| {
                    if tile.ids.get(x, y, 0) == 0.0 {
                        texels += 1;
                    }
                    tile.ids.set(x, y, 0, id1);
                    tile.ids.set(x, y, 1, plane);
                });
            }
        }
    }
    let (nx, nz) = water_grid(h[0], h[2]);
    let notes = vec![format!("water-id grid {nx} × {nz} tiles of {ID_MAP_SIZE}² over the scene box c {:?} h {:?} ({:.4} × {:.4} texel/m): {} quads, {texels} texels carry an id, plane tops {:?}", c, h, n / (2.0 * h[0] / nx as f32), n / (2.0 * h[2] / nz as f32), quads.len(), plane_tops)];
    WaterIdTiles { tiles, plane_tops, texels, quads: quads.len(), notes }
}

/// The records' fold {c, h} (FUN_140184fa0 over every record's rec+0x38 box = computeParams+0xb0, the scene box S).
pub fn records_box(recs: &[Rec]) -> Option<([f32; 3], [f32; 3])> {
    let mut mn = [f32::MAX; 3];
    let mut mx = [f32::MIN; 3];
    for r in recs {
        for k in 0..3 {
            mn[k] = mn[k].min(r.centre[k] - r.half[k]);
            mx[k] = mx[k].max(r.centre[k] + r.half[k]);
        }
    }
    if mn[0] > mx[0] { return None; }
    Some(([(mn[0] + mx[0]) * 0.5, (mn[1] + mx[1]) * 0.5, (mn[2] + mx[2]) * 0.5], [(mx[0] - mn[0]) * 0.5, (mx[1] - mn[1]) * 0.5, (mx[2] - mn[2]) * 0.5]))
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    #[test]
    fn the_grid_matches_the_two_captures() {
        // pwc-day: a 2048 × 2048 scene box → one tile at 1 texel/m
        assert_eq!(water_grid(1024.0, 1024.0), (1, 1));
        // stpad (f4468 cbuffers): box x [−32, 3085.2549], z [−22.5649, 1531.3137 ∪ 3085.1924] → 1 × 2, the Z extent split in two
        let (hx, hz) = ((3085.2549f32 + 32.0) * 0.5, (3085.1924f32 + 22.5649) * 0.5);
        assert_eq!(water_grid(hx, hz), (1, 2));
        let tiles = water_tiles_of_box([hx - 32.0, 0.0, hz - 22.5649], [hx, 0.0, hz]);
        assert_eq!(tiles.len(), 2);
        assert!((tiles[0].world_min[0] + 32.0).abs() < 1e-3 && (tiles[0].world_max[1] - 1531.3137).abs() < 1e-2, "{:?}", tiles[0]);
        assert!((tiles[1].world_min[1] - 1531.3137).abs() < 1e-2 && (tiles[1].world_max[1] - 3085.1924).abs() < 1e-2, "{:?}", tiles[1]);
        let m = tiles[0].world_to_id();
        assert!((m[0][0] - 0.6569883).abs() < 1e-5 && (m[0][3] - 21.0236).abs() < 2e-3 && (m[1][2] + 1.3179922).abs() < 1e-5 && (m[1][3] - 2018.2595).abs() < 5e-2, "{:?}", m);
        // an extent of exactly 4096 in x stays one tile; 4097 → two; 2049 in z → two, 16385 → the cap 8
        assert_eq!(water_grid(2048.0, 1024.0), (1, 1));
        assert_eq!(water_grid(2048.5, 1024.5), (2, 2));
        assert_eq!(water_grid(8192.5, 8192.5), (5, 8));
        assert_eq!(water_grid(6144.0, 6144.0), (3, 6));
    }
}
