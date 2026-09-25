//! The TERRAIN MATERIAL chain of a collection pack, as the game builds it (RE child 8, 2026-09-25,
//! DISASSEMBLY of Trackmania.exe md5 4a28c004…; every rule below reproduces the pwc-day capture,
//! frame 127447, to the bit):
//!
//! ```text
//! <Coll>\Media\Material\<Name>.Material.Gbx           CPlugMaterial → inline CPlugMaterialCustom
//!   ├─ chunk 0x0903A013  texture slots: BaseColor, RoughMetal, Normal, PyX2, PyH2 → CPlugBitmap refs
//!   └─ chunk 0x0903A015  layer names (Pxz, Py, X2, H2) — "SeaFloor","SeaFloor","SeaFloor",""
//! <Coll>\Media\Texture Array\Terrain_D.TextureArray.Gbx   CPlugBitmap (the BaseColor slot)
//!   ├─ chunk 0x09011030  → inline CPlugFileGen kind 0x1d {4096, 4096, 6, 1, 3, 13, 1, 1, 0}: the
//!   │                      generated 4096² BC1 array, 6 slices, 13 mips
//!   └─ chunk 0x09011034  {ref ImageArray, suffix "_D", slice refs IN GPU SLICE ORDER}
//!        → the GPU array (captured 5354) = those .dds files, each VERTICALLY FLIPPED (`vflip_bc1`)
//! <Coll>\Media\Texture Array\Image Array\TerrainLayers_DNRM.ImageArray.Gbx   CPlugImageArray
//!   └─ chunk 0x0914C000  layers {name, Py size, offsets, Pxz size, offsets, rotation, blend angles, ids}
//!        → g_WorldPosToTcPyPxz (captured 5352) = `world_pos_to_tc(layers)`: 4 float4 per layer
//! ```
//!
//! The shader (`Tech3/Block_PyPxz_ids_p`, PS 8401 of the capture) reads its slices through
//! `g_CBufferP_Shader.{iPy, iPxz, iPyX2, iPyH2}` = the material's `SubIndexPyPxz` parameter
//! (0x1404424e0 → 0x1404429f0 / 0x140442d80 / 0x140442e00): iPy / iPxz = the index of the Py /
//! Pxz NAME among the BaseColor array's ImageArray layers (0x1404d4550: the first layer whose name
//! matches, else −1), iPyX2 = the X2 name in the PyX2 slot's ImageArray, iPyH2 = the H2 name in the
//! PyH2 slot's (an empty name = −1 → the shader's `ult 255` test skips the modulation). A data
//! quirk to know: BlueBay's X2 TEXTURE has 4 slices (Land, Dirt2, Dirt, SeaFloor) but its
//! ImageArray only 2 layers (Land, SeaFloor), so the Sea tile's iPyX2 = 1 samples Dirt2_X2 with
//! SeaFloor's mapping — the game does exactly that.
//!
//! The pre-pass constant (`GbxVisualToWorld = 0`): every interpolant is 0, the normal's
//! normalisation is NaN → both blend weights saturate to 0 → PS 8401 = TMapBaseColor slice iPxz
//! at uv (0, −buffer[iPxz][2].z) through SGbxWrap_Aniso with zero derivatives (mip 0, bilinear,
//! wrap) — `lightmap::paktables` samples it.
//!
//! WATER (chunk 0x03033038 of `Collections\<Coll>.Collection.Gbx`, reader 0x140d0c580 →
//! 0x14040c720): the collection's one water type {name, top, floor, FogMaxDepth, fog texture,
//! transmittance ImageGen, normal texture} → the lightmapper's `g_WaterTop_ByPlanes` (= top) and
//! `g_WaterDepth_FogMaxDepthInv_ByIds` (= (top − floor, 1/FogMaxDepth), 0x1402255a0 l.430–466)
//! and the fog LUT `TMapWaterFog` (= column 0 of the fog texture — a 32×256 TGA — read top-down,
//! `tga_column_topdown`).

use crate::node::{ImageArrayLayer, ImageArrayRaw, Node, Slot};
use crate::store::DataStore;

/// One texture array of a terrain material: the ImageArray layers and the slice images.
#[derive(Clone, Debug, Default)]
pub struct TerrainArray {
    /// The `.TextureArray.Gbx` logical path.
    pub bitmap_path: String,
    /// The `.ImageArray.Gbx` logical path and its layers.
    pub image_array_path: String,
    pub layers: Vec<ImageArrayLayer>,
    /// The slice images (`.dds` logical paths) in GPU slice order (chunk 0x09011034).
    pub slices: Vec<String>,
    /// The CPlugFileGen words of the generated array {w, h, slices, 1, 3, mips, 1, 1, 0} when present.
    pub gen: Vec<u32>,
}

/// A terrain material (`Tech3 Block PyPxz_Ids` family) resolved through the pack.
#[derive(Clone, Debug, Default)]
pub struct TerrainMaterial {
    pub link: String,
    /// The parent material (`Techno3\Media\Material\Tech3 Block PyPxz_Ids.Material.gbx`).
    pub parent: String,
    /// (Pxz, Py, X2, H2) names of chunk 0x0903A015.
    pub layer_names: [String; 4],
    pub base: Option<TerrainArray>,
    pub x2: Option<TerrainArray>,
    pub h2: Option<TerrainArray>,
    /// The `g_CBufferP_Shader` ids: iPy, iPxz, iPyX2, iPyH2 (−1 = none, what the shader tests as ≥ 255).
    pub i_py: i32,
    pub i_pxz: i32,
    pub i_py_x2: i32,
    pub i_py_h2: i32,
    /// Slots that could not be followed (a Techno3 default texture outside the collection pack).
    pub notes: Vec<String>,
}

impl TerrainMaterial {
    /// The three per-slice buffers as the game uploads them (`g_WorldPosToTcPyPxz`,
    /// `g_WorldPosToTcPyX2`, `g_WorldPosToTcPyH2`), 4 float4 per layer.
    pub fn buffers(&self) -> (Vec<[f32; 4]>, Vec<[f32; 4]>, Vec<[f32; 4]>) {
        let b = |a: &Option<TerrainArray>| a.as_ref().map(|a| world_pos_to_tc(&a.layers)).unwrap_or_default();
        (b(&self.base), b(&self.x2), b(&self.h2))
    }
}

fn graph_of<'a>(m: &'a crate::store::Model) -> Result<crate::node::Graph<'a>, String> {
    m.graph()
}

/// The `.ImageArray.Gbx` at `path`.
pub fn load_image_array(store: &mut DataStore, path: &str) -> Result<ImageArrayRaw, String> {
    let m = store.load_model(path)?;
    let g = graph_of(&m)?;
    match g.root {
        Some(Node::ImageArray(a)) => Ok(*a),
        other => Err(format!("{path}: not a CPlugImageArray ({:?})", other.map(|n| crate::node::node_kind_name(&n)))),
    }
}

/// A `.TextureArray.Gbx` (or any CPlugBitmap): its ImageArray, slices and generator words.
pub fn load_texture_array(store: &mut DataStore, path: &str) -> Result<TerrainArray, String> {
    let m = store.load_model(path)?;
    let g = graph_of(&m)?;
    let Some(Node::Bitmap(b)) = &g.root else { return Err(format!("{path}: not a CPlugBitmap")) };
    let ext = |i: i32| -> Result<String, String> { g.external(i).map(|s| s.to_string()).ok_or_else(|| format!("{path}: node {i} is not an external reference")) };
    let gen = match g.node(b.image) {
        Some(Node::FileGen(f)) => f.u32s.clone(),
        _ => Vec::new(),
    };
    let mut out = TerrainArray { bitmap_path: path.to_string(), gen, ..TerrainArray::default() };
    if b.array_image_array >= 0 {
        out.image_array_path = ext(b.array_image_array)?;
        let ia = load_image_array(store, &out.image_array_path)?;
        out.layers = ia.layers;
    } else if b.image >= 0 {
        // a plain texture: one "slice", no layer table
        if let Some(p) = g.external(b.image) {
            out.slices.push(p.to_string());
        }
    }
    for &s in &b.array_slices {
        out.slices.push(ext(s)?);
    }
    Ok(out)
}

/// The index of `name` among `layers` — 0x1404d4550: the first layer whose name matches
/// (length first, then bytes), −1 for no match or an empty name.
pub fn layer_index(layers: &[ImageArrayLayer], name: &str) -> i32 {
    if name.is_empty() {
        return -1;
    }
    layers.iter().position(|l| l.name == name).map(|i| i as i32).unwrap_or(-1)
}

/// Resolve a terrain material link (`BlueBay\Media\Material\SeaFloor`, with or without the
/// `.Material.Gbx` suffix) through the pack: the layer names, the three texture arrays and the ids.
pub fn load_terrain_material(store: &mut DataStore, link: &str) -> Result<TerrainMaterial, String> {
    let file = if link.to_ascii_lowercase().ends_with(".gbx") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let m = store.load_model(&file)?;
    let g = graph_of(&m)?;
    let mut out = TerrainMaterial { link: link.to_string(), i_py: -1, i_pxz: -1, i_py_x2: -1, i_py_h2: -1, ..TerrainMaterial::default() };
    let mut custom: Option<&crate::node::MaterialCustomRaw> = None;
    for s in &g.slots {
        if let Slot::Node(Node::MaterialCustom(c)) = s {
            custom = Some(c);
        }
        if let Slot::External(p) = s {
            if p.to_ascii_lowercase().ends_with(".material.gbx") && !p.eq_ignore_ascii_case(&file) {
                out.parent = p.clone();
            }
        }
    }
    let Some(custom) = custom else { return Err(format!("{file}: no CPlugMaterialCustom node")) };
    out.layer_names = custom.layer_names.clone();
    let slot = |name: &str| -> Option<String> {
        custom.bitmaps.iter().find(|(n, _)| n == name).and_then(|(_, r)| g.external(*r)).map(|s| s.to_string())
    };
    if let Some(p) = slot("BaseColor") {
        out.base = Some(load_texture_array(store, &p)?);
    }
    // The X2 / H2 slots may name a Techno3 default (`DisabledModX2.Texture.gbx`, Maniaplanet.pak —
    // not in a collection pack): then there is no ImageArray and the id stays −1, as in the game
    // (a plain texture has no layer to match).
    if let Some(p) = slot("PyX2") {
        match load_texture_array(store, &p) {
            Ok(a) => out.x2 = Some(a),
            Err(e) => out.notes.push(format!("PyX2 {p}: {e}")),
        }
    }
    if let Some(p) = slot("PyH2") {
        match load_texture_array(store, &p) {
            Ok(a) => out.h2 = Some(a),
            Err(e) => out.notes.push(format!("PyH2 {p}: {e}")),
        }
    }
    // 0x1404429f0: iPy / iPxz from the first texture slot that carries an ImageArray and is not
    // the PyX2 / PyH2 slot — the BaseColor array; both names must resolve or the pair stays −1.
    if let Some(b) = &out.base {
        let py = layer_index(&b.layers, &out.layer_names[1]);
        let pxz = layer_index(&b.layers, &out.layer_names[0]);
        out.i_py = py;
        out.i_pxz = pxz;
    }
    // 0x140442d80 / 0x140442e00: only when the name is non-empty (its length gates the call).
    if let Some(x) = &out.x2 {
        out.i_py_x2 = layer_index(&x.layers, &out.layer_names[2]);
    }
    if let Some(h) = &out.h2 {
        out.i_py_h2 = layer_index(&h.layers, &out.layer_names[3]);
    }
    Ok(out)
}

/// The per-layer `g_WorldPosToTc*` buffer as 0x1409f2e50 + 0x1404d49d0 fill it — every operation
/// in the game's f32 order, the CRT `sinf` / `cosf` (double core, one rounding = correctly
/// rounded for these inputs) standing in for 0x14195d240 / 0x14195c010:
///
/// ```text
/// a  = (rotation° × 3.1415927f) / 180f              [mulss, divss]
/// s, c = sinf(a), cosf(a)
/// [0] = (c, 0, s, py_offset.x) × (1 / py_scale.x)   [mulss by the reciprocal]
/// [1] = (s, −0, −c, −py_offset.y) × (1 / py_scale.y)
/// [2] = (1 / pxz_scale.x, 1 / pxz_scale.y, pxz_offset.y / pxz_scale.y, bits(ids[1]))
/// [3] = (cos(blend_pxz.end × k), cos(blend_pxz.start × k), cos(blend_py.end × k), cos(blend_py.start × k)),
///        k = f32 0x3c8efa36 (0.017453294, the .rdata constant at 0x141d1ee4c), the products in f32
/// ```
/// (the shader: u_py = x·[0].x + z·[0].z + [0].w, v_py = x·[1].x + z·[1].z + [1].w; the Pxz sample
/// at (±z·[2].x or ±x·[2].x, y·[2].y − [2].z); the side blend smoothsteps |n.x|/√(n.x²+n.z²) between
/// [3].x and [3].y, the top blend |n.y| between [3].z and [3].w).
pub fn world_pos_to_tc(layers: &[ImageArrayLayer]) -> Vec<[f32; 4]> {
    const DEG: f32 = f32::from_bits(0x3c8e_fa36);
    const PI: f32 = f32::from_bits(0x4049_0fdb);
    let mut out = Vec::with_capacity(layers.len() * 4);
    for l in layers {
        let a = (l.rotation_deg * PI) / 180.0f32;
        let (s, c) = (crt_sinf(a), crt_cosf(a));
        let inv_x = 1.0f32 / l.py_scale[0];
        let inv_y = 1.0f32 / l.py_scale[1];
        out.push([inv_x * c, inv_x * 0.0, inv_x * s, inv_x * l.py_offset[0]]);
        out.push([inv_y * s, inv_y * -0.0, inv_y * -c, inv_y * -l.py_offset[1]]);
        out.push([1.0f32 / l.pxz_scale[0], 1.0f32 / l.pxz_scale[1], l.pxz_offset[1] / l.pxz_scale[1], f32::from_bits(l.ids[1])]);
        out.push([
            crt_cosf(l.blend_pxz_deg[1] * DEG),
            crt_cosf(l.blend_pxz_deg[0] * DEG),
            crt_cosf(l.blend_py_deg[1] * DEG),
            crt_cosf(l.blend_py_deg[0] * DEG),
        ]);
    }
    out
}

/// The CRT `sinf` (0x14195d240): a double-precision core with one final rounding — the
/// correctly rounded f32 sine for every angle this module feeds it (RE child 5 verified the
/// same routine's outputs against the game to the bit).
pub fn crt_sinf(x: f32) -> f32 {
    (x as f64).sin() as f32
}
/// The CRT `cosf` (0x14195c010), likewise.
pub fn crt_cosf(x: f32) -> f32 {
    (x as f64).cos() as f32
}

/// A GPU texture-array slice from a pack `.dds`: the game uploads the file's BC1 blocks
/// VERTICALLY FLIPPED (the DDS is stored bottom-up) — block rows reversed and, inside every
/// block, the four 2-bit index rows reversed (the two palette colours keep their order). Applies
/// to one mip level of `w`×`h` texels; `data` holds its blocks row-major. Verified: all six 4096²
/// slices of the captured 5354 and the four of 5363 are byte-identical to the flipped files.
pub fn vflip_bc1(data: &[u8], w: u32, h: u32) -> Vec<u8> {
    let bw = ((w + 3) / 4) as usize;
    let bh = ((h + 3) / 4) as usize;
    let rowb = bw * 8;
    let mut out = vec![0u8; data.len().min(rowb * bh)];
    for r in 0..bh {
        let src = &data[(bh - 1 - r) * rowb..(bh - r) * rowb];
        let dst = &mut out[r * rowb..(r + 1) * rowb];
        for i in 0..bw {
            let b = &src[i * 8..i * 8 + 8];
            let d = &mut dst[i * 8..i * 8 + 8];
            d[..4].copy_from_slice(&b[..4]);
            d[4] = b[7];
            d[5] = b[6];
            d[6] = b[5];
            d[7] = b[4];
        }
    }
    out
}

/// The collection's water type (`Collections\<Coll>.Collection.Gbx` chunk 0x03033038 v8).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterDesc {
    /// The type's Id (`Sea`, `Deep`, `Shallow`).
    pub name: String,
    /// The water surface height in the zone prefab's frame (= `g_WaterTop_ByPlanes`).
    pub top: f32,
    /// The floor height; `top − floor` = the depth the lightmapper's tint stops at (`− 0.1`).
    pub floor: f32,
    /// The fog table's depth scale: the shader's u = 2·(top − y) / fog_max_depth.
    pub fog_max_depth: f32,
    /// The fog table image (`<Coll>\Media\Texture\Image\Water<Type>_Fog.dds` — a 32×256 TGA).
    pub fog_image: String,
    /// The transmittance generator (`…\WaterTransmittance.ImageGen.Gbx`, CPlugFileGen kind 51).
    pub transmittance: String,
    /// The water normal texture (`…\WaterSxSySzPC3.Texture.Gbx`).
    pub normal: String,
    /// The four floats after the references and the trailing {u32, f32, u32, f32}.
    pub params: [f32; 4],
    pub tail: [u32; 4],
}

impl WaterDesc {
    /// The `g_WaterDepth_FogMaxDepthInv_ByIds` entry of this type: (depth, 1/FogMaxDepth).
    pub fn depth_and_inv(&self) -> [f32; 2] {
        [self.top - self.floor, 1.0f32 / self.fog_max_depth]
    }
}

/// The water descriptor of `Collections\<collection>.Collection.Gbx`. The collection file's
/// walk has no reader for most of its chunks, so the chunk is located by its id in the
/// decompressed body and parsed in place (version 8 only; the older layouts embed up to four
/// {Id, 3 floats, ref} records inline).
pub fn collection_water(store: &mut DataStore, collection: &str) -> Result<WaterDesc, String> {
    let path = format!("Collections\\{collection}.Collection.Gbx");
    let m = store.load_model(&path)?;
    let body = &m.body;
    let ext = |i: u32| -> String { m.externals.iter().find(|(n, _)| *n == i).map(|(_, p)| p.clone()).unwrap_or_default() };
    let id = 0x0303_3038u32.to_le_bytes();
    let mut o = 0usize;
    loop {
        let Some(p) = body[o..].windows(4).position(|w| w == id) else { return Err(format!("{path}: no chunk 0x03033038")) };
        let at = o + p + 4;
        o = at;
        let rd = |q: usize| -> Option<u32> { body.get(q..q + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) };
        let Some(version) = rd(at) else { continue };
        if version != 8 {
            continue;
        }
        // i32 −1 (no water-types node), u32, then the descriptor: Id (0x40000000 + a string, or an index)
        if rd(at + 4) != Some(0xffff_ffff) {
            continue;
        }
        let mut q = at + 12;
        let Some(idw) = rd(q) else { continue };
        q += 4;
        let name = if idw & 0x4000_0000 != 0 {
            let Some(n) = rd(q) else { continue };
            q += 4;
            let n = n as usize;
            if n > 64 || q + n > body.len() {
                continue;
            }
            let s = String::from_utf8_lossy(&body[q..q + n]).to_string();
            q += n;
            s
        } else {
            format!("#{}", idw & 0x3fff_ffff)
        };
        let f = |q: usize| rd(q).map(f32::from_bits);
        let (Some(top), Some(floor), Some(fog_max_depth)) = (f(q), f(q + 4), f(q + 8)) else { continue };
        let (Some(r0), Some(r1), Some(r2)) = (rd(q + 12), rd(q + 16), rd(q + 20)) else { continue };
        let params = [f(q + 24).unwrap_or(0.0), f(q + 28).unwrap_or(0.0), f(q + 32).unwrap_or(0.0), f(q + 36).unwrap_or(0.0)];
        let tail = [rd(q + 40).unwrap_or(0), rd(q + 44).unwrap_or(0), rd(q + 48).unwrap_or(0), rd(q + 52).unwrap_or(0)];
        return Ok(WaterDesc { name, top, floor, fog_max_depth, fog_image: ext(r0), transmittance: ext(r1), normal: ext(r2), params, tail });
    }
}

/// The fog LUT of a collection's water fog image: BlueBay's `WaterSea_Fog.dds` is 32 wide × 256
/// tall and the captured 256-texel LUT is its COLUMN 0 read top-down (`tga_column_topdown`, texel
/// for texel). RedIsland / GreenCoast / WhiteShore ship the transposed image (256 × 32, the colour
/// running along x): for those this takes the TOP row left → right — the same "first line along
/// the long axis from the top-left corner" rule, INFERRED (no capture of those collections' water
/// exists yet; one pwc-style frame of a Deep-water map settles it).
pub fn water_fog_lut(tga: &[u8]) -> Result<Vec<[u8; 4]>, String> {
    if tga.len() < 18 {
        return Err("TGA shorter than its header".into());
    }
    let w = u16::from_le_bytes([tga[12], tga[13]]) as usize;
    let h = u16::from_le_bytes([tga[14], tga[15]]) as usize;
    if h >= w {
        return tga_column_topdown(tga, 0);
    }
    tga_row_from_top(tga, 0)
}

/// Row `y` (counted from the TOP of the image) of a 32-bit uncompressed TGA, left → right, as
/// (r, g, b, a) bytes.
pub fn tga_row_from_top(tga: &[u8], y: usize) -> Result<Vec<[u8; 4]>, String> {
    if tga.len() < 18 {
        return Err("TGA shorter than its header".into());
    }
    let idlen = tga[0] as usize;
    let kind = tga[2];
    let w = u16::from_le_bytes([tga[12], tga[13]]) as usize;
    let h = u16::from_le_bytes([tga[14], tga[15]]) as usize;
    let bpp = tga[16];
    let desc = tga[17];
    if kind != 2 || bpp != 32 {
        return Err(format!("TGA type {kind} at {bpp} bpp: only uncompressed 32-bit is read"));
    }
    if y >= h {
        return Err(format!("row {y} of a {h}-tall TGA"));
    }
    let px = &tga[18 + idlen..];
    if px.len() < w * h * 4 {
        return Err("TGA pixel data truncated".into());
    }
    let row = if desc & 0x20 != 0 { y } else { h - 1 - y };
    Ok((0..w).map(|x| { let o = (row * w + x) * 4; [px[o + 2], px[o + 1], px[o], px[o + 3]] }).collect())
}

/// Column `x` of a 32-bit uncompressed TGA read TOP-DOWN, as (r, g, b, a) bytes per row: how the
/// lightmapper's `TMapWaterFog` LUT (256 texels, BGRA8 sRGB) relates to the water fog image — the
/// captured 15075 is column 0 of `WaterSea_Fog.dds` (a bottom-up TGA of 32×256) texel for texel.
pub fn tga_column_topdown(tga: &[u8], x: usize) -> Result<Vec<[u8; 4]>, String> {
    if tga.len() < 18 {
        return Err("TGA shorter than its header".into());
    }
    let idlen = tga[0] as usize;
    let kind = tga[2];
    let w = u16::from_le_bytes([tga[12], tga[13]]) as usize;
    let h = u16::from_le_bytes([tga[14], tga[15]]) as usize;
    let bpp = tga[16];
    let desc = tga[17];
    if kind != 2 || bpp != 32 {
        return Err(format!("TGA type {kind} at {bpp} bpp: only uncompressed 32-bit is read"));
    }
    if x >= w {
        return Err(format!("column {x} of a {w}-wide TGA"));
    }
    let px = &tga[18 + idlen..];
    if px.len() < w * h * 4 {
        return Err("TGA pixel data truncated".into());
    }
    let top_down = desc & 0x20 != 0;
    let mut out = Vec::with_capacity(h);
    for i in 0..h {
        let row = if top_down { i } else { h - 1 - i };
        let o = (row * w + x) * 4;
        // stored BGRA
        out.push([px[o + 2], px[o + 1], px[o], px[o + 3]]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(name: &str, py: f32, pxz: f32, rot: f32, bpxz: [f32; 2], bpy: [f32; 2], id: u32) -> ImageArrayLayer {
        ImageArrayLayer { name: name.into(), py_scale: [py, py], py_offset: [0.0, 0.0], pxz_scale: [pxz, pxz], pxz_offset: [0.0, 0.0], rotation_deg: rot, blend_pxz_deg: bpxz, blend_py_deg: bpy, ids: [id, id] }
    }

    /// The BlueBay TerrainLayers_DNRM layers → the captured g_WorldPosToTcPyPxz (buffer 5352 of
    /// pwc-day frame 127447, 384 bytes), bit for bit.
    #[test]
    fn bluebay_dnrm_buffer_is_the_captured_5352() {
        let layers = vec![
            layer("Land", 52.0, 40.0, 74.0, [40.0, 50.0], [40.0, 60.0], 100),
            layer("CliffPxz", 128.0, 128.0, 0.0, [40.0, 50.0], [40.0, 50.0], 110),
            layer("HillPxz", 64.0, 64.0, 0.0, [40.0, 50.0], [40.0, 50.0], 120),
            layer("Sand", 32.0, 32.0, 0.0, [40.0, 50.0], [40.0, 50.0], 30),
            layer("SeaFloor", 64.0, 64.0, 0.0, [40.0, 50.0], [40.0, 50.0], 54),
            layer("RocksTop", 40.0, 40.0, 0.0, [40.0, 50.0], [40.0, 50.0], 115),
        ];
        let b = world_pos_to_tc(&layers);
        let want: [[u32; 4]; 24] = [
            [0x3badb1a3, 0x00000000, 0x3c976f8a, 0x00000000],
            [0x3c976f8a, 0x80000000, 0xbbadb1a3, 0x80000000],
            [0x3ccccccd, 0x3ccccccd, 0x00000000, 0x00000064],
            [0x3f248dba, 0x3f441b7c, 0x3efffffc, 0x3f441b7c],
            [0x3c000000, 0, 0, 0],
            [0, 0x80000000, 0xbc000000, 0x80000000],
            [0x3c000000, 0x3c000000, 0, 0x6e],
            [0x3f248dba, 0x3f441b7c, 0x3f248dba, 0x3f441b7c],
            [0x3c800000, 0, 0, 0],
            [0, 0x80000000, 0xbc800000, 0x80000000],
            [0x3c800000, 0x3c800000, 0, 0x78],
            [0x3f248dba, 0x3f441b7c, 0x3f248dba, 0x3f441b7c],
            [0x3d000000, 0, 0, 0],
            [0, 0x80000000, 0xbd000000, 0x80000000],
            [0x3d000000, 0x3d000000, 0, 0x1e],
            [0x3f248dba, 0x3f441b7c, 0x3f248dba, 0x3f441b7c],
            [0x3c800000, 0, 0, 0],
            [0, 0x80000000, 0xbc800000, 0x80000000],
            [0x3c800000, 0x3c800000, 0, 0x36],
            [0x3f248dba, 0x3f441b7c, 0x3f248dba, 0x3f441b7c],
            [0x3ccccccd, 0, 0, 0],
            [0, 0x80000000, 0xbccccccd, 0x80000000],
            [0x3ccccccd, 0x3ccccccd, 0, 0x73],
            [0x3f248dba, 0x3f441b7c, 0x3f248dba, 0x3f441b7c],
        ];
        assert_eq!(b.len(), 24);
        for (i, (got, w)) in b.iter().zip(want.iter()).enumerate() {
            let g: [u32; 4] = [got[0].to_bits(), got[1].to_bits(), got[2].to_bits(), got[3].to_bits()];
            assert_eq!(g, *w, "float4 {i}: {:x?} vs {:x?}", g, w);
        }
    }

    /// The X2 array's two layers (Land: Py (256, −256) rotated 42°, SeaFloor 600) → buffer 5361.
    #[test]
    fn bluebay_x2_buffer_is_the_captured_5361() {
        let mut land = layer("Land", 256.0, 256.0, 42.0, [40.0, 50.0], [40.0, 50.0], 100);
        land.py_scale = [256.0, -256.0];
        let sea = layer("SeaFloor", 600.0, 600.0, 0.0, [40.0, 50.0], [40.0, 50.0], 54);
        let b = world_pos_to_tc(&[land, sea]);
        let want: [[u32; 4]; 8] = [
            [0x3b3e3ebd, 0, 0x3b2b4c25, 0],
            [0xbb2b4c25, 0, 0x3b3e3ebd, 0],
            [0x3b800000, 0x3b800000, 0, 0x64],
            [0x3f248dba, 0x3f441b7c, 0x3f248dba, 0x3f441b7c],
            [0x3ada740e, 0, 0, 0],
            [0, 0x80000000, 0xbada740e, 0x80000000],
            [0x3ada740e, 0x3ada740e, 0, 0x36],
            [0x3f248dba, 0x3f441b7c, 0x3f248dba, 0x3f441b7c],
        ];
        for (i, (got, w)) in b.iter().zip(want.iter()).enumerate() {
            let g: [u32; 4] = [got[0].to_bits(), got[1].to_bits(), got[2].to_bits(), got[3].to_bits()];
            assert_eq!(g, *w, "float4 {i}: {:x?} vs {:x?}", g, w);
        }
    }

    #[test]
    fn layer_lookup_is_by_exact_name_and_empty_is_none() {
        let layers = vec![layer("Land", 1.0, 1.0, 0.0, [0.0; 2], [0.0; 2], 0), layer("SeaFloor", 1.0, 1.0, 0.0, [0.0; 2], [0.0; 2], 0)];
        assert_eq!(layer_index(&layers, "SeaFloor"), 1);
        assert_eq!(layer_index(&layers, "seafloor"), -1);
        assert_eq!(layer_index(&layers, ""), -1);
    }

    #[test]
    fn vflip_reverses_block_rows_and_index_bytes() {
        // a 4×8 texel image = 1×2 blocks
        let top: [u8; 8] = [1, 2, 3, 4, 0xa, 0xb, 0xc, 0xd];
        let bot: [u8; 8] = [5, 6, 7, 8, 0x1a, 0x1b, 0x1c, 0x1d];
        let mut data = Vec::new();
        data.extend_from_slice(&top);
        data.extend_from_slice(&bot);
        let f = vflip_bc1(&data, 4, 8);
        assert_eq!(&f[..8], &[5, 6, 7, 8, 0x1d, 0x1c, 0x1b, 0x1a]);
        assert_eq!(&f[8..], &[1, 2, 3, 4, 0xd, 0xc, 0xb, 0xa]);
    }

    #[test]
    fn tga_column_is_read_top_down_from_a_bottom_up_file() {
        // 2×3 BGRA bottom-up: file row 0 is the image's bottom
        let mut t = vec![0u8; 18];
        t[2] = 2;
        t[12] = 2;
        t[14] = 3;
        t[16] = 32;
        t[17] = 8;
        for row in 0..3u8 {
            for x in 0..2u8 {
                t.extend_from_slice(&[10 + row, 20 + row, 30 + row, x]); // B, G, R, A
            }
        }
        let c = tga_column_topdown(&t, 1).unwrap();
        assert_eq!(c, vec![[32, 22, 12, 1], [31, 21, 11, 1], [30, 20, 10, 1]]);
    }

    #[test]
    fn water_depth_and_inverse_are_top_minus_floor_and_one_over_fog_max() {
        let w = WaterDesc { top: 7.0, floor: 4.0, fog_max_depth: 3.5, ..WaterDesc::default() };
        let d = w.depth_and_inv();
        assert_eq!(d[0].to_bits(), 3.0f32.to_bits());
        assert_eq!(d[1].to_bits(), 0x3e924925); // the captured 17009 (0.2857143)
    }
}
