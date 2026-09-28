//! THE ATTRIBUTE PRE-PASS'S MATERIAL INPUTS FROM THE PACK (RE child 8, 2026-09-25) — what the
//! `--env-from` capture used to freeze (`prepass_check::frozen_tables`): the terrain constants,
//! the projected-texture (TrackWall) constant and the water pass's tables and LUTs, every one
//! transcribed from the game (DISASSEMBLY of Trackmania.exe 4a28c004… + the pack files) and
//! bit-exact against pwc-day frame 127447 where the capture holds the object:
//!
//! * `terrain_constant` — PS 8401 (`Tech3/Block_PyPxz_ids_p`) at `GbxVisualToWorld = 0`: the
//!   world position and normal are 0, `normalize(0)` is NaN, both `mul_sat` blend weights become 0,
//!   and the colour is `TMapBaseColor` slice `iPxz` at uv (0, −g_WorldPosToTcPyPxz[iPxz·4+2].z)
//!   through `SGbxWrap_Aniso` with zero derivatives = mip 0, bilinear, wrap. The slice image, the
//!   slice index and the buffer come from the pack (`mapgeom::terrain`); the buffer 5352 and the
//!   fog LUT 15075 reproduce the capture byte for byte, the constants to A's 7 digits
//!   (SeaFloor (0.85107964, 0.7339805, 0.32585403), Land (0.11579…), TrackWall (0.43581…)).
//! * `projected_constant` — PS 17025 (`Tech3 Block PyPxzDiff_Spec_Norm_LM1` family) at the zero
//!   matrix: `TMapPxzBaseColor` at (0, 0) (the Pxz uv rows are `(±z, y)·GbxSamplerTcScaleTrans −
//!   trans`, all 0), and `TMapACosSmoothPy(|n.y| = 0)` = texel 0 of the material's `ACosSmoothPy`
//!   slot — the Techno3 `ACosSmoothDefaultPyPxz.Texture.Gbx` (Maniaplanet.pak), a GENERATED 1-D
//!   R16 LUT (`acos_smooth_lut`, CPlugFileGen kind 0x21: smoothstep of acos(u)/(π/2) between 0.45
//!   and 0.7; texel 0 = 65535, the captured 5459 1024/1024) — so the Py term's weight is `1 − 1 = 0`
//!   and the constant is the Pxz image's sample. The X2 slot's default `DisabledModX2` (a 4×4 TGA of
//!   0x7f7f7f = the captured 5468) multiplies nothing at the zero matrix.
//! * `water_tables` — `Collections\<Coll>.Collection.Gbx` chunk 0x03033038: `g_WaterTop_ByPlanes`
//!   = WaterTop, `g_WaterDepth_FogMaxDepthInv_ByIds` = (WaterTop − WaterFloor, 1/FogMaxDepth)
//!   (0x1402255a0 l.430–466), the fog LUT = column 0 of the descriptor's fog TGA read top-down
//!   (256/256 = 15075), and the transmittance LUT = `water_transmittance_lut` on the descriptor's
//!   `WaterTransmittance.ImageGen.Gbx` (CPlugFileGen kind 0x33, generator 0x140418310: 2048/2048 =
//!   15078).

use crate::texsample::{decode_bc1_block, Bc1Decode, Level, TexFmt, Texture};
use mapgeom::node::FileGenRaw;
use mapgeom::store::DataStore;
use mapgeom::terrain::{self, TerrainMaterial, WaterDesc};

/// THE COMPANION PACKS every bake needs beside the collection's (E, 2026-09-27): Stadium.pak holds the Modifier\StadiumOnTerrain
/// materials' textures (np-tk3's pillar / wall BaseColor) and the Stadium items' materials; Maniaplanet.pak the Techno3 parent
/// materials / shaders (the PyPxz family names, the LM uv-set selector) and the stock items. Their keys are fixed per file.
/// `lmtool bake` adds the ones found in the named --pak's directory (--no-pak-defaults opts out).
pub const COMPANION_PAKS: [(&str, &str); 2] = [("Stadium.pak", "B773D73047A4104857722366D78D28A6"), ("Maniaplanet.pak", "9A93723447347A8CE336CCFC49E65449")];
/// The key of a known pack by its file name (the collections' packs share one key), for tools that take a directory.
pub fn pak_key_of(file_name: &str) -> Option<&'static str> {
    let n = file_name.to_ascii_lowercase();
    match n.as_str() {
        "stadium.pak" => Some("B773D73047A4104857722366D78D28A6"),
        "maniaplanet.pak" | "maniaplanet_core.pak" => Some("9A93723447347A8CE336CCFC49E65449"),
        "bluebay.pak" | "greencoast.pak" | "whiteshore.pak" | "redisland.pak" => Some("660C4C156B80337E296A1034B0AA05B8"),
        _ => None,
    }
}

/// A world-projected material's pre-pass constant and how it was found.
#[derive(Clone, Debug)]
pub struct MaterialConstant {
    /// The linear rgb PS 8401 / 17025 output before `× GbxP_LmComputeScaleNoAcc`.
    pub rgb: [f32; 3],
    /// Which pixel shader family the material runs (`PyPxzIds` = the terrain array shader, PS 8401;
    /// `PyPxzProjected` = the single-texture Py/Pxz shader, PS 17025).
    pub family: Family,
    /// The image the constant was sampled from and the uv.
    pub image: String,
    pub uv: [f32; 2],
    /// The shader ids (iPy, iPxz, iPyX2, iPyH2) for the terrain family.
    pub ids: [i32; 4],
    /// Slots that had to be assumed (a Techno3 pack not in the store).
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    PyPxzIds,
    PyPxzProjected,
    /// `Tech3 Block PyPxz_Hue` (CustomPlastic …): the Py/Pxz BaseColor IS a hue mask, recoloured toward `RgbTargetColor`
    /// unconditionally (`hue_class_constant`).
    PyPxzHue,
}

/// Parse a DDS header: (data offset, width, height, mips, array size, fourcc/dxgi is BC1).
fn dds_head(dds: &[u8]) -> Result<(usize, u32, u32, u32, u32), String> {
    if dds.len() < 128 || &dds[0..4] != b"DDS " {
        return Err("not a DDS".into());
    }
    let rd = |o: usize| u32::from_le_bytes(dds[o..o + 4].try_into().unwrap());
    let h = rd(12);
    let w = rd(16);
    let mips = rd(28).max(1);
    let fourcc = &dds[84..88];
    let (mut off, mut arr) = (128usize, 1u32);
    let bc1 = if fourcc == b"DX10" {
        let fmt = rd(128);
        arr = rd(140).max(1);
        off = 148;
        matches!(fmt, 70 | 71 | 72)
    } else {
        fourcc == b"DXT1"
    };
    if !bc1 {
        return Err(format!("DDS is not BC1 (fourcc {:?})", String::from_utf8_lossy(fourcc)));
    }
    Ok((off, w, h, mips, arr))
}

/// One texel of mip 0 of a pack BC1 image AS THE GPU HOLDS IT — the file is stored bottom-up and
/// uploaded flipped (`mapgeom::terrain::vflip_bc1`), so GPU row `y` is file row `h − 1 − y`.
/// `Bc1Decode::Expand8Round` (the 8-bit palette with rounded thirds the capture matched), sRGB
/// decoded through the IEC curve (the `_UNORM_SRGB` view).
fn gpu_texel_srgb(dds: &[u8], off: usize, w: u32, h: u32, x: u32, y: u32) -> [f32; 4] {
    let fy = h - 1 - y;
    let bw = w.div_ceil(4);
    let bi = ((fy / 4) * bw + x / 4) as usize * 8;
    let blk = decode_bc1_block(&dds[off + bi..off + bi + 8], Bc1Decode::Expand8Round, false);
    let mut p = blk[((fy % 4) * 4 + x % 4) as usize];
    for k in 0..3 {
        p[k] = crate::gpufmt::srgb_to_linear(p[k]);
    }
    p
}

/// Mip 0 of a pack BC1 sRGB image sampled at `uv` with zero derivatives through a WRAP bilinear
/// sampler — the pre-pass's constant sample. Texel space t = uv·size − 0.5, the four neighbours
/// wrapped, weights from the fractions, summed as the filter does (two lerps, f32).
pub fn mip0_bilinear_wrap_srgb(dds: &[u8], uv: [f32; 2]) -> Result<[f32; 3], String> {
    let (off, w, h, _mips, _arr) = dds_head(dds)?;
    let wrap = |t: f32, n: u32| -> (u32, u32, f32) {
        let s = t * n as f32 - 0.5;
        let f = s.floor();
        let i0 = (f as i64).rem_euclid(n as i64) as u32;
        let i1 = (i0 + 1) % n;
        (i0, i1, s - f)
    };
    let (x0, x1, fx) = wrap(uv[0], w);
    let (y0, y1, fy) = wrap(uv[1], h);
    let c00 = gpu_texel_srgb(dds, off, w, h, x0, y0);
    let c10 = gpu_texel_srgb(dds, off, w, h, x1, y0);
    let c01 = gpu_texel_srgb(dds, off, w, h, x0, y1);
    let c11 = gpu_texel_srgb(dds, off, w, h, x1, y1);
    let mut out = [0f32; 3];
    for k in 0..3 {
        let top = c00[k] * (1.0 - fx) + c10[k] * fx;
        let bot = c01[k] * (1.0 - fx) + c11[k] * fx;
        out[k] = top * (1.0 - fy) + bot * fy;
    }
    Ok(out)
}

/// PS 8401's pre-pass constant of a terrain material (`BlueBay\Media\Material\SeaFloor`): the
/// `iPxz` slice of the BaseColor array at uv (0, −buffer[iPxz][2].z).
pub fn terrain_constant(store: &mut DataStore, link: &str) -> Result<MaterialConstant, String> {
    let tm = terrain::load_terrain_material(store, link)?;
    terrain_constant_of(store, &tm)
}

pub fn terrain_constant_of(store: &mut DataStore, tm: &TerrainMaterial) -> Result<MaterialConstant, String> {
    let base = tm.base.as_ref().ok_or_else(|| format!("{}: no BaseColor texture array", tm.link))?;
    if tm.i_pxz < 0 {
        return Err(format!("{}: the Pxz layer {:?} is not in {}", tm.link, tm.layer_names[0], base.image_array_path));
    }
    let i = tm.i_pxz as usize;
    let image = base.slices.get(i).ok_or_else(|| format!("{}: slice {i} has no image (the array has {})", tm.link, base.slices.len()))?.clone();
    let buf = terrain::world_pos_to_tc(&base.layers);
    // the shader: r6.yz = r1.y · r5.y − r5.z with r1 = 0 → v = −[2].z; u = r1.z · r5.x = 0 (r0 signs: NaN compares false → +)
    let uv = [0.0f32, -buf[i * 4 + 2][2]];
    let dds = store.read(&image)?;
    let rgb = mip0_bilinear_wrap_srgb(&dds, uv)?;
    Ok(MaterialConstant { rgb, family: Family::PyPxzIds, image, uv, ids: [tm.i_py, tm.i_pxz, tm.i_py_x2, tm.i_py_h2], notes: tm.notes.clone() })
}

/// PS 17025's pre-pass constant of a single-texture Py/Pxz material (`…\TrackWallInWorld`): the
/// `PxzBaseColor` slot's image at (0, 0) — `GbxWorldPosToTexCoord_MapPyBaseColor` has no
/// translation (chunk 0x09011025's trans = 0) so the Py term is also at (0, 0), and the
/// `ACosSmoothPy` default LUT's texel 0 = 1 removes it.
pub fn projected_constant(store: &mut DataStore, link: &str) -> Result<MaterialConstant, String> {
    let file = if link.to_ascii_lowercase().ends_with(".gbx") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let m = store.load_model(&file)?;
    let g = m.graph()?;
    let mut custom = None;
    for s in &g.slots {
        if let mapgeom::node::Slot::Node(mapgeom::node::Node::MaterialCustom(c)) = s {
            custom = Some(c);
        }
    }
    let custom = custom.ok_or_else(|| format!("{file}: no CPlugMaterialCustom"))?;
    let slot = |name: &str| custom.bitmaps.iter().find(|(n, _)| n == name).and_then(|(_, r)| g.external(*r)).map(|s| s.to_string());
    let tex = slot("PxzBaseColor").or_else(|| slot("PyBaseColor")).or_else(|| slot("BaseColor")).ok_or_else(|| format!("{file}: no PxzBaseColor / PyBaseColor / BaseColor slot"))?;
    // the .Texture.gbx → its image (chunk 0x09011030), and the translation of chunk 0x09011025
    let tm = store.load_model(&tex)?;
    let tg = tm.graph()?;
    let (image, trans) = match &tg.root {
        Some(mapgeom::node::Node::Bitmap(b)) => {
            let img = tg.external(b.image).ok_or_else(|| format!("{tex}: the image node {} is not external", b.image))?.to_string();
            let t = b.tc_scale_trans.map(|v| [f32::from_bits(v[2]), f32::from_bits(v[3])]).unwrap_or([0.0, 0.0]);
            (img, t)
        }
        _ => return Err(format!("{tex}: not a CPlugBitmap")),
    };
    let dds = store.read(&image)?;
    // v1 = 0: r1.x = ±(v1.z · s.x) = 0, r1.w = v1.y · s.y − s.w = −trans.y
    let uv = [0.0f32, -trans[1]];
    let pxz = mip0_bilinear_wrap_srgb(&dds, uv)?;
    // The Py term: weight = 1 − TMapACosSmoothPy(|n.y| = 0) = 1 − LUT[0]/65535. With the Techno3
    // default LUT that is 0 (texel 0 = 65535, the captured 5459); read the material's own slot when
    // the pack holding it is in the store, else assume the default (and say so in `notes`).
    let mut notes = Vec::new();
    let py_weight = match slot("ACosSmoothPy") {
        Some(t) => match acos_smooth_of_slot(store, &t) {
            Ok(lut) => 1.0 - lut.first().copied().unwrap_or(65535) as f32 / 65535.0,
            Err(e) => {
                notes.push(format!("ACosSmoothPy {t}: {e} — assuming the Techno3 default (texel 0 = 65535)"));
                0.0
            }
        },
        None => 0.0,
    };
    let rgb = if py_weight == 0.0 {
        pxz
    } else {
        // the Py image at (0, 0) blended in — never the case for the shipped defaults
        let py_tex = slot("PyBaseColor").ok_or_else(|| format!("{file}: ACosSmoothPy weight {py_weight} needs a PyBaseColor slot"))?;
        let pm = store.load_model(&py_tex)?;
        let pg = pm.graph()?;
        let py_img = match &pg.root { Some(mapgeom::node::Node::Bitmap(b)) => pg.external(b.image).map(|s| s.to_string()), _ => None }.ok_or_else(|| format!("{py_tex}: no image"))?;
        let py = mip0_bilinear_wrap_srgb(&store.read(&py_img)?, [0.0, 0.0])?;
        [pxz[0] * (1.0 - py_weight) + py[0] * py_weight, pxz[1] * (1.0 - py_weight) + py[1] * py_weight, pxz[2] * (1.0 - py_weight) + py[2] * py_weight]
    };
    Ok(MaterialConstant { rgb, family: Family::PyPxzProjected, image, uv, ids: [-1; 4], notes })
}

/// The constant of any world-projected material, by its parent shader family; an error names a
/// material whose pre-pass colour is not constant (a mesh-uv textured material: sample its own
/// textures per texel as `setupmap::attr_from_map` does).
pub fn material_constant(store: &mut DataStore, link: &str) -> Result<MaterialConstant, String> {
    material_constant_with(store, link, None)
}

/// The parent material's shader file name (lower case; the parent material's own name when the Techno3 pack is not in
/// the store — the same words) and the parent material path.
fn shader_of(store: &mut DataStore, file: &str) -> Result<(String, String), String> {
    let m = store.load_model(file)?;
    let parent = m.externals.iter().map(|(_, p)| p.to_ascii_lowercase()).find(|p| p.ends_with(".material.gbx") && !p.eq_ignore_ascii_case(&file.to_ascii_lowercase())).unwrap_or_default();
    // The parent material (Maniaplanet.pak `Techno3\Media\Material\Tech3 Block PyPxz_Ids.Material.gbx`)
    // names its shader (`…\Shader\Tech3 Block PyPxz_Ids.Shader.Gbx`): when the pack is in the store the
    // family comes from the SHADER name, else from the parent material's name (the same words).
    let shader = if parent.is_empty() {
        String::new()
    } else {
        match store.load_model(&parent) {
            Ok(pm) => pm.externals.iter().map(|(_, p)| p.to_ascii_lowercase()).find(|p| p.ends_with(".shader.gbx")).unwrap_or_else(|| parent.clone()),
            Err(_) => parent.clone(),
        }
    };
    Ok((shader, parent))
}

/// `material_constant` with the material INSTANCE's `TargetColor` override (`geometry::target_colour_override`: an item's
/// `CPlugMaterialUserInst` Csts) — it matters for the PyPxz_Hue family only.
pub fn material_constant_with(store: &mut DataStore, link: &str, target: Option<[f32; 3]>) -> Result<MaterialConstant, String> {
    let file = if link.to_ascii_lowercase().ends_with(".gbx") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let (shader, parent) = shader_of(store, &file)?;
    if shader.contains("pypxz_ids") {
        terrain_constant(store, link)
    } else if shader.contains("pypxz_hue") && std::env::var_os("LMTOOL_NO_PYPXZ_HUE").is_none() {
        // LMTOOL_NO_PYPXZ_HUE=1 (study / opt-out): the mask drawn as the albedo, the port's pre-read behaviour
        hue_class_constant(store, link, target)
    } else if shader.contains("pypxz") {
        projected_constant(store, link)
    } else {
        Err(format!("{link}: parent {parent:?} (shader {shader:?}) is not a world-projected (PyPxz) material — its pre-pass colour is per texel"))
    }
}

/// THE PyPxz_Hue PRE-PASS CONSTANT (E3 2026-09-28; GpuCache `Tech3/Block_PyPxz_X2H2_PeelDiff_Hue_p` blob 0 =
/// DTwk_SkipMap_ILightInput, `× GbxP_LmComputeScaleNoAcc`): the material's Py/Pxz BaseColor is a HUE MASK
/// (`CustomPlastic_D` = (0, 0.815, 0) everywhere) recoloured UNCONDITIONALLY toward `ShaderP.RgbTargetColor` — no
/// `BaseColorTargetId` gate, no `m.a` blend: with m = the mask tap, k = max(m.g − ½(m.r + m.b), 0),
/// albedo = sat((m.g − k)·mean(T) + k·T); the Py path (`4·recol_py·X2·Hx2`, weight 1 − ACosSmoothPy(|n.y|)) vanishes at the
/// zero world matrix (RE 15's rule: |n.y| = 0 → LUT[0] = 1), so the constant is the Pxz tap's recolour, the tap at
/// (0, −trans.y) as `projected_constant` reads it. `T` = the ITEM's material-instance `TargetColor` Csts when the item
/// carries one (a "Real" float3 — no colour-space conversion: the palette path linearises BYTES, this is a float
/// parameter), else the material's authored `TargetColor` param (CustomPlastic (1.0, 0.0863, 0.0863)), else the
/// program's RDEF default (0, 1, 0) — which is exactly the green the port drew before this read. The vertex COLOR0
/// factor (`mul r0.xyz, r0.xyz, v4.xyz`) is taken as 1 (no colour stream on these items).
pub fn hue_class_constant(store: &mut DataStore, link: &str, target: Option<[f32; 3]>) -> Result<MaterialConstant, String> {
    let mut base = projected_constant(store, link)?;
    let file = if link.to_ascii_lowercase().ends_with(".gbx") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let m = store.load_model(&file)?;
    let g = m.graph()?;
    let mut authored: Option<[f32; 3]> = None;
    for s in &g.slots {
        if let mapgeom::node::Slot::Node(mapgeom::node::Node::MaterialCustom(c)) = s {
            if let Some((_, v)) = c.params.iter().find(|(n, v)| n.eq_ignore_ascii_case("TargetColor") && v.len() >= 3) { authored = Some([v[0], v[1], v[2]]); }
        }
    }
    let (t, src) = match (target, authored) {
        (Some(t), _) => (t, "instance Csts"),
        (None, Some(a)) => (a, "material param"),
        (None, None) => ([0.0, 1.0, 0.0], "shader default"),
    };
    let mask = base.rgb;
    let k = (mask[1] - 0.5 * (mask[0] + mask[2])).max(0.0);
    let mean_t = (t[0] + t[1] + t[2]) / 3.0;
    let mut recol = [((mask[1] - k) * mean_t + k * t[0]).clamp(0.0, 1.0), ((mask[1] - k) * mean_t + k * t[1]).clamp(0.0, 1.0), ((mask[1] - k) * mean_t + k * t[2]).clamp(0.0, 1.0)];
    // LMTOOL_HUE_STUDY=grey|mask (STUDY, E5 2026-09-28 21:30Z — E4's probe oracle on g23: the editor's 479 probes over the CustomPlastic
    // hills read GREY (1.09, 1.13, 1.12) where ours read ORANGE (1.95, 1.50, 0.29) → the game's hills EMIT a neutral colour in its LM,
    // not the TargetColor-recoloured mask): `grey` = the recolour's luminance as a neutral (k·mean(T)), `mask` = the mask itself (the
    // pre-E3 form). A direction test of the items' G/B deficit, not a rule — RE 16 reads which program / constant the LM pre-pass runs.
    let study = std::env::var("LMTOOL_HUE_STUDY").unwrap_or_default();
    if study == "grey" { let g = ((mask[1] - k) * mean_t + k * mean_t).clamp(0.0, 1.0); recol = [g, g, g]; }
    else if study == "mask" { recol = mask; }
    base.notes.push(format!("PyPxz_Hue: mask tap {:?} → k {k:.4}, RgbTargetColor {:?} ({src}) → albedo {:?}{}", mask, t, recol, if study.is_empty() { String::new() } else { format!(" [STUDY LMTOOL_HUE_STUDY={study}]") }));
    base.rgb = recol;
    base.family = Family::PyPxzHue;
    Ok(base)
}

/// The engine's linear → sRGB 8-bit table (0x141a64760, 4096 entries; used by 0x14018cdf0:
/// `byte = table[clamp(trunc(x · 4095 + 0.5), 0, 4095)]`): `round(255 · IEC_encode(i / 4095))`
/// — verified against the banked table (`client-re/linear_to_srgb_u8_4096.bin`) entry for entry.
pub fn linear_to_srgb_table() -> Vec<u8> {
    (0..4096)
        .map(|i| {
            let x = i as f64 / 4095.0;
            let e = if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
            (255.0 * e).round().clamp(0.0, 255.0) as u8
        })
        .collect()
}

/// 0x14018cdf0: a linear f32 to an sRGB byte through the 4096-entry table.
pub fn linear_to_srgb_u8(table: &[u8], x: f32) -> u8 {
    let v = x * 4095.0f32 + 0.5;
    // cvttss2si: truncation toward zero (NaN → i32::MIN → clamped to 0)
    let i = if v.is_nan() { 0 } else { v as i32 };
    let i = if i <= 0 { 0 } else { i.min(4095) };
    table[i as usize]
}

/// `WaterTransmittance.ImageGen.Gbx` (CPlugFileGen kind 0x33) as the game generates it
/// (0x140418530 case 0x33 → 0x140418310): `width = u32s[0]`, `curve = u32s.get(1) != 0`
/// (1 when the array has one word), colour `c = f32s[0..3]`, `depth = f32s[3]`; texel i:
/// `t = i/width` (f32 div), `t = (t·t + t) − (t·t)·t` when `curve`, `d = t·depth`,
/// rgb = (c.x^d, c.y^d, c.z^d) (CRT `powf` 0x1418faad0), each → an sRGB byte through the
/// 4096-entry table, alpha 0xff. The captured 15078 (BlueBay: c = (0.15, 0.18, 0.1), depth 4,
/// curve on) reproduces 2048/2048 texels (RGBA8 `_SRGB`).
pub fn water_transmittance_lut(gen: &FileGenRaw) -> Result<Vec<[u8; 4]>, String> {
    if gen.kind != 0x33 {
        return Err(format!("CPlugFileGen kind {} is not the water transmittance generator (0x33)", gen.kind));
    }
    let width = *gen.u32s.first().ok_or("kind 0x33 without a width")? as usize;
    let curve = gen.u32s.len() < 2 || gen.u32s[1] != 0;
    if gen.f32s.len() < 4 {
        return Err(format!("kind 0x33 with {} floats (colour + depth needed)", gen.f32s.len()));
    }
    let c = [gen.f32s[0], gen.f32s[1], gen.f32s[2]];
    let depth = gen.f32s[3];
    let table = linear_to_srgb_table();
    let mut out = Vec::with_capacity(width);
    for i in 0..width {
        let mut t = i as f32 / width as f32;
        if curve {
            let t2 = t * t;
            let t3 = t2 * t;
            t = (t2 + t) - t3;
        }
        let d = t * depth;
        let px = |x: f32| linear_to_srgb_u8(&table, crt_powf(x, d));
        out.push([px(c[0]), px(c[1]), px(c[2]), 0xff]);
    }
    Ok(out)
}

/// The CRT `powf` (0x1418faad0): a double-precision core with one rounding — the correctly rounded
/// f32 power for these inputs (every captured texel agrees).
pub fn crt_powf(x: f32, y: f32) -> f32 {
    (x as f64).powf(y as f64) as f32
}

/// `ACosSmooth*.Texture.Gbx` (CPlugFileGen kind 0x21) as the game generates it (0x14041b290,
/// reached from the kind switch 0x140418530 case 0x21 with `width = u32s[0]`, `smooth = u32s[1]`,
/// `degrees = u32s.get(2)`, `a0 = f32s[0]`, `a1 = f32s[1]`): a 1-D R16_UNORM LUT over u = i/width,
/// `t = acos(u)` (CRT acosf), `/ (π/2)` (or `/π·180` when `degrees`), `(t − a0)/(a1 − a0)` clamped
/// to [0, 1], smoothstep `3t² − 2t³` when `smooth`, `× 65535` truncated and clamped. `a0`/`a1` are
/// clamped to [0, 1] (or [0, 90]) and ordered first. Mip 0 only (the sampler reads mip 0 at a
/// constant uv). The Techno3 default (0.45, 0.7, smooth) reproduces the captured 5459 1024/1024.
pub fn acos_smooth_lut(gen: &FileGenRaw) -> Result<Vec<u16>, String> {
    if gen.kind != 0x21 {
        return Err(format!("CPlugFileGen kind {} is not the ACosSmooth generator (0x21)", gen.kind));
    }
    let width = *gen.u32s.first().ok_or("kind 0x21 without a width")? as usize;
    let smooth = gen.u32s.get(1).copied().unwrap_or(0) != 0;
    let degrees = gen.u32s.get(2).copied().unwrap_or(0) != 0;
    if gen.f32s.len() < 2 {
        return Err(format!("kind 0x21 with {} floats (two angles needed)", gen.f32s.len()));
    }
    let max = if degrees { 90.0f32 } else { 1.0f32 };
    let a0 = gen.f32s[0].clamp(0.0, max);
    let a1 = if gen.f32s[0] <= gen.f32s[1] { gen.f32s[1].min(max) } else { a0 };
    let a1 = if gen.f32s[0] <= gen.f32s[1] && gen.f32s[1] <= max { gen.f32s[1] } else { a1 };
    let mut out = Vec::with_capacity(width);
    for i in 0..width {
        let mut t = ((i as f32 / width as f32) as f64).acos() as f32;
        t = if degrees { (t / 3.1415927f32) * 180.0f32 } else { t / 1.5707964f32 };
        t = (t - a0) / (a1 - a0);
        t = if t <= 0.0 { 0.0 } else if t >= 1.0 { 1.0 } else { t };
        let v: i32 = if smooth {
            if t <= 0.0 {
                0
            } else if t < 1.0 {
                let t = t * 3.0 * t - (t + t) * t * t;
                (t * 65535.0) as i32
            } else {
                0xffff
            }
        } else {
            (t * 65535.0) as i32
        };
        out.push(if v <= 0 { 0 } else { v.min(0xffff) as u16 });
    }
    Ok(out)
}

/// The material slot's `.Texture.Gbx` resolved to its generated LUT (kind 0x21), or an error naming
/// what the slot is.
pub fn acos_smooth_of_slot(store: &mut DataStore, texture: &str) -> Result<Vec<u16>, String> {
    let m = store.load_model(texture)?;
    let g = m.graph()?;
    let Some(mapgeom::node::Node::Bitmap(b)) = &g.root else { return Err(format!("{texture}: not a CPlugBitmap")) };
    match g.node(b.image) {
        Some(mapgeom::node::Node::FileGen(f)) => acos_smooth_lut(f),
        _ => Err(format!("{texture}: the image is not a generated LUT (node {})", b.image)),
    }
}

/// The single colour of a constant TGA such as `Techno3\Media\Texture\Image\DisabledModX2.tga`
/// (4×4, 24-bit, RLE): (r, g, b, a) with a = 255 — the X2 modulation's neutral 0x7f (the captured
/// 5468). Errors when the image is not one colour.
pub fn constant_tga_rgba(tga: &[u8]) -> Result<[u8; 4], String> {
    if tga.len() < 18 {
        return Err("TGA shorter than its header".into());
    }
    let idlen = tga[0] as usize;
    let kind = tga[2];
    let w = u16::from_le_bytes([tga[12], tga[13]]) as usize;
    let h = u16::from_le_bytes([tga[14], tga[15]]) as usize;
    let bpp = tga[16] as usize / 8;
    if !(bpp == 3 || bpp == 4) {
        return Err(format!("TGA at {} bpp", bpp * 8));
    }
    let mut px: Vec<[u8; 4]> = Vec::with_capacity(w * h);
    let mut o = 18 + idlen;
    let take = |o: usize| -> Result<[u8; 4], String> {
        let p = tga.get(o..o + bpp).ok_or("TGA truncated")?;
        Ok([p[2], p[1], p[0], if bpp == 4 { p[3] } else { 255 }])
    };
    match kind {
        2 => {
            for i in 0..w * h {
                px.push(take(o + i * bpp)?);
            }
        }
        10 => {
            while px.len() < w * h {
                let hdr = *tga.get(o).ok_or("TGA truncated")?;
                o += 1;
                let n = (hdr & 0x7f) as usize + 1;
                if hdr & 0x80 != 0 {
                    let p = take(o)?;
                    o += bpp;
                    px.extend(std::iter::repeat(p).take(n));
                } else {
                    for _ in 0..n {
                        px.push(take(o)?);
                        o += bpp;
                    }
                }
            }
        }
        k => return Err(format!("TGA type {k}: only uncompressed / RLE truecolor is read")),
    }
    let first = *px.first().ok_or("empty TGA")?;
    if px.iter().any(|p| *p != first) {
        return Err("TGA is not a single colour".into());
    }
    Ok(first)
}

/// A 1-D RGBA8 LUT as a sampleable `Texture` (one level, `Rgba8`), sRGB-decoded when `srgb`
/// (the `_UNORM_SRGB` views the shader reads 15075 / 15078 through).
pub fn lut_texture(px: &[[u8; 4]], srgb: bool) -> Texture {
    let w = px.len() as u32;
    let mut t = Texture { fmt: TexFmt::Rgba8, w, h: 1, mips: 1, slices: 1, levels: vec![vec![Level::from_f32(w, 1, px.iter().map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0]).collect())]], complete: true };
    if srgb {
        t.decode_srgb();
    }
    t
}

/// The water pass's inputs of a collection, from its pack.
#[derive(Clone, Debug)]
pub struct WaterTables {
    pub desc: WaterDesc,
    /// `g_WaterTop_ByPlanes[0]`.
    pub top: f32,
    /// `g_WaterDepth_FogMaxDepthInv_ByIds[0]` = (top − floor, 1/FogMaxDepth).
    pub depth_inv: [f32; 2],
    /// `TMapWaterFog` slice 0 (256 RGBA8 sRGB texels).
    pub fog: Vec<[u8; 4]>,
    /// `TMapWaterTransmittance` slice 0 (2048 RGBA8 sRGB texels).
    pub transmittance: Vec<[u8; 4]>,
}

pub fn water_tables(store: &mut DataStore, collection: &str) -> Result<WaterTables, String> {
    let desc = terrain::collection_water(store, collection)?;
    let fog_bytes = store.read(&desc.fog_image)?;
    let fog = terrain::water_fog_lut(&fog_bytes)?;
    let gen_model = store.load_model(&desc.transmittance)?;
    let g = gen_model.graph()?;
    let transmittance = match &g.root {
        Some(mapgeom::node::Node::FileGen(f)) => water_transmittance_lut(f)?,
        _ => return Err(format!("{}: not a CPlugFileGen", desc.transmittance)),
    };
    Ok(WaterTables { top: desc.top, depth_inv: desc.depth_and_inv(), fog, transmittance, desc })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pak_store(coll: &str) -> Option<DataStore> {
        let dir = std::env::var("TM_PAKS").unwrap_or_else(|_| format!("{}/persistent/private-30d/tm-paks", std::env::var("HOME").unwrap_or_default()));
        let path = format!("{dir}/{coll}.pak");
        if !std::path::Path::new(&path).is_file() {
            eprintln!("skipped: {path} is not on this box");
            return None;
        }
        let key = if coll == "Stadium" { "B773D73047A4104857722366D78D28A6" } else { "660C4C156B80337E296A1034B0AA05B8" };
        Some(DataStore::open(&[path], key).unwrap())
    }

    #[test]
    fn the_srgb_table_is_the_banked_engine_table() {
        let t = linear_to_srgb_table();
        assert_eq!(&t[..16], &[0x00, 0x01, 0x02, 0x02, 0x03, 0x04, 0x05, 0x06, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0a, 0x0b, 0x0c]);
        assert_eq!(t[4095], 0xff);
        assert_eq!(linear_to_srgb_u8(&t, 0.0), 0);
        assert_eq!(linear_to_srgb_u8(&t, 1.0), 255);
        assert_eq!(linear_to_srgb_u8(&t, -1.0), 0);
        assert_eq!(linear_to_srgb_u8(&t, 5.0), 255);
    }

    /// BlueBay's generator parameters → the captured 15078 (pwc-day frame 127447) at the texels
    /// the notes list: 0, 2, 16, 256, 512, 1024, 1536, 2047.
    #[test]
    fn bluebay_transmittance_lut_matches_the_captured_15078() {
        let gen = FileGenRaw { version: 6, kind: 0x33, u32s: vec![2048, 1], float4s: vec![[0.25080317, 0.36408398, 0.075976014, 1.0]], f32s: vec![0.15, 0.18, 0.1, 4.0, -1.0, -1.0], name: String::new() };
        let lut = water_transmittance_lut(&gen).unwrap();
        assert_eq!(lut.len(), 2048);
        for (i, want) in [(0usize, [0xff, 0xff, 0xff, 0xff]), (2, [0xfe, 0xfe, 0xfe, 0xff]), (3, [0xfe, 0xfe, 0xfd, 0xff]), (16, [0xf8, 0xf9, 0xf7, 0xff]), (256, [0xa0, 0xa7, 0x90, 0xff]), (512, [0x5b, 0x65, 0x48, 0xff]), (768, [0x30, 0x3a, 0x20, 0xff]), (1024, [0x17, 0x1f, 0x0a, 0xff]), (1280, [0x0a, 0x10, 0x02, 0xff]), (1536, [0x04, 0x07, 0x01, 0xff]), (1792, [0x02, 0x04, 0x01, 0xff]), (2047, [0x02, 0x03, 0x00, 0xff])] {
            assert_eq!(lut[i], want, "texel {i}");
        }
    }

    /// The Techno3 default (0.45, 0.7, smooth) → the captured 5459 (R16, 1024): 65535 up to texel 464,
    /// 65534 at 465, 62606 at 510, 0 from 779 (every texel checked against the capture bytes).
    #[test]
    fn acos_smooth_default_matches_the_captured_5459() {
        let gen = FileGenRaw { version: 6, kind: 0x21, u32s: vec![1024, 1, 0], float4s: vec![], f32s: vec![0.45, 0.7], name: String::new() };
        let lut = acos_smooth_lut(&gen).unwrap();
        assert_eq!(lut.len(), 1024);
        assert_eq!(lut[0], 65535);
        assert_eq!(lut[464], 65535);
        assert_eq!(&lut[465..470], &[65534, 65533, 65528, 65520, 65509]);
        assert_eq!(&lut[510..514], &[62606, 62479, 62350, 62218]);
        assert_eq!(&lut[776..780], &[20, 7, 1, 0]);
        assert_eq!(lut[1023], 0);
    }

    /// Every texel against the capture file itself (pwc-day env/frame127447/textures/e012380_5459.dds.gz,
    /// a plain-header DDS of R16 1024×1) when the passcap is on this box.
    #[test]
    fn acos_smooth_default_matches_the_whole_captured_5459() {
        let path = format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/passcap/pwc-day/env/frame127447/textures/e012380_5459.dds.gz", std::env::var("HOME").unwrap_or_default());
        let Ok(gz) = std::fs::read(&path) else { eprintln!("skipped: {path} is not on this box"); return };
        let d = crate::passdiff::gunzip(&gz).unwrap();
        let gen = FileGenRaw { version: 6, kind: 0x21, u32s: vec![1024, 1, 0], float4s: vec![], f32s: vec![0.45, 0.7], name: String::new() };
        let lut = acos_smooth_lut(&gen).unwrap();
        let cap: Vec<u16> = (0..1024).map(|i| u16::from_le_bytes([d[128 + i * 2], d[129 + i * 2]])).collect();
        assert_eq!(lut, cap);
    }

    #[test]
    fn disabled_mod_x2_is_the_neutral_grey() {
        // the pack file: 4×4, 24-bit RLE, one packet per row of 0x7f7f7f
        let mut t = vec![0u8; 18];
        t[2] = 10;
        t[12] = 4;
        t[14] = 4;
        t[16] = 24;
        for _ in 0..4 {
            t.extend_from_slice(&[0x83, 0x7f, 0x7f, 0x7f]);
        }
        assert_eq!(constant_tga_rgba(&t).unwrap(), [0x7f, 0x7f, 0x7f, 0xff]);
    }

    /// With Maniaplanet.pak in the store nothing is assumed: the ACosSmoothPy slot's LUT is read and
    /// the X2 default is the 0x7f grey.
    #[test]
    fn techno3_defaults_from_maniaplanet_pak() {
        let dir = std::env::var("TM_PAKS").unwrap_or_else(|_| format!("{}/persistent/private-30d/tm-paks", std::env::var("HOME").unwrap_or_default()));
        let mp = format!("{dir}/Maniaplanet.pak");
        if !std::path::Path::new(&mp).is_file() {
            eprintln!("skipped: {mp} is not on this box");
            return;
        }
        let mut store = DataStore::empty();
        store.add_pak(&mp, "9A93723447347A8CE336CCFC49E65449").unwrap();
        let lut = acos_smooth_of_slot(&mut store, "Techno3\\Media\\Texture\\ACosSmoothDefaultPyPxz.Texture.Gbx").unwrap();
        assert_eq!(lut[0], 65535);
        assert_eq!(lut[510], 62606);
        let tga = store.read("Techno3\\Media\\Texture\\Image\\DisabledModX2.tga").unwrap();
        assert_eq!(constant_tga_rgba(&tga).unwrap(), [0x7f, 0x7f, 0x7f, 0xff]);
        let Some(bb) = pak_store("BlueBay") else { return };
        let mut both = bb;
        both.add_pak(&mp, "9A93723447347A8CE336CCFC49E65449").unwrap();
        both.add_pak(&format!("{dir}/Stadium.pak"), "B773D73047A4104857722366D78D28A6").unwrap();
        let c = material_constant(&mut both, "BlueBay\\Media\\Modifier\\StadiumOnTerrain\\TrackWallInWorld").unwrap();
        assert!(c.notes.is_empty(), "{:?}", c.notes);
        assert_eq!(c.family, Family::PyPxzProjected);
        let sea = material_constant(&mut both, "BlueBay\\Media\\Material\\SeaFloor").unwrap();
        assert_eq!(sea.family, Family::PyPxzIds);
    }

    #[test]
    fn bluebay_terrain_constants_from_the_pak() {
        let Some(mut store) = pak_store("BlueBay") else { return };
        let sea = terrain_constant(&mut store, "BlueBay\\Media\\Material\\SeaFloor").unwrap();
        assert_eq!(sea.ids, [4, 4, 1, -1]);
        assert!(sea.image.ends_with("SeaFloor_D.dds"), "{}", sea.image);
        assert_eq!(sea.uv, [0.0, -0.0]);
        // A's corner_mean on the same file: (0.85107964, 0.7339805, 0.32585403); the capture 0.8510797, 0.7339804, 0.325854
        for (k, w) in [0.85107964f32, 0.7339805, 0.32585403].iter().enumerate() {
            assert!((sea.rgb[k] - w).abs() <= 2e-7, "SeaFloor {k}: {} vs {w}", sea.rgb[k]);
        }
        let land = material_constant(&mut store, "BlueBay\\Media\\Material\\Land").unwrap();
        assert_eq!(land.family, Family::PyPxzIds);
        assert_eq!(land.ids, [0, 0, 0, -1]);
        for (k, w) in [0.1158f32, 0.1753, 0.0428].iter().enumerate() {
            assert!((land.rgb[k] - w).abs() < 1e-3, "Land {k}: {} vs {w}", land.rgb[k]);
        }
    }

    #[test]
    fn bluebay_water_tables_from_the_pak() {
        let Some(mut store) = pak_store("BlueBay") else { return };
        let w = water_tables(&mut store, "BlueBay").unwrap();
        assert_eq!(w.desc.name, "Sea");
        assert_eq!(w.top, 7.0);
        assert_eq!(w.depth_inv[0].to_bits(), 3.0f32.to_bits());
        assert_eq!(w.depth_inv[1].to_bits(), 0x3e924925);
        assert_eq!(w.fog.len(), 256);
        // 15075 texel 0 = BGRA 91 93 55 02 → RGBA (0x55, 0x93, 0x91, 0x02); texel 255 = 36 21 00 f5 → (0, 0x21, 0x36, 0xf5)
        assert_eq!(w.fog[0], [0x55, 0x93, 0x91, 0x02]);
        assert_eq!(w.fog[1], [0x53, 0x93, 0x91, 0x09]);
        assert_eq!(w.fog[255], [0x00, 0x21, 0x36, 0xf5]);
        assert_eq!(w.transmittance.len(), 2048);
        assert_eq!(w.transmittance[1024], [0x17, 0x1f, 0x0a, 0xff]);
    }

    #[test]
    fn stadium_trackwall_constant_from_the_pak() {
        let Some(mut store) = pak_store("Stadium") else { return };
        let Some(bb) = pak_store("BlueBay") else { return };
        // the material lives in BlueBay.pak, its texture in Stadium.pak
        let mut both = bb;
        both.add_pak(&format!("{}/persistent/private-30d/tm-paks/Stadium.pak", std::env::var("HOME").unwrap_or_default()), "B773D73047A4104857722366D78D28A6").unwrap();
        drop(store);
        let c = material_constant(&mut both, "BlueBay\\Media\\Modifier\\StadiumOnTerrain\\TrackWallInWorld").unwrap();
        assert_eq!(c.family, Family::PyPxzProjected);
        assert!(c.image.ends_with("TrackWallPxzInWorld_D.dds"), "{}", c.image);
        for (k, w) in [0.4358f32, 0.4036, 0.3483].iter().enumerate() {
            assert!((c.rgb[k] - w).abs() < 1e-3, "TrackWall {k}: {} vs {w}", c.rgb[k]);
        }
    }
}

// ───────────────── THE HUE-MASK RECOLOUR (PS 9539 / 9544 — RE 13's transcription, RE 15's palette read, G2 2026-09-28) ─────────────────

/// A HueMask material's recolour toward the placement colour, at the pre-pass tap: the game's `RgbBaseColorTarget` = the material's
/// own colour table (`<Coll>\Media\ColorTargetTables\<Name>.ColorTable.gbx.json`, class CPlugMaterialColorTargetTable, list
/// "Classic" for the editor bake) at MapElemColor − 1 (1 White, 2 Green, 3 Blue, 4 Red, 5 Black), sRGB bytes → linear; the mask
/// `m` = the material's `PxzBaseColorHueMask` (else Py / plain) image sampled like the base (WRAP bilinear at (0, −trans.y), rgb
/// through the sRGB view, alpha linear — RE 15 07:30Z: the captured red items sit on the sRGB-decoded prediction); then
/// k = max(m.g − ½(m.r + m.b), 0), recol = sat((m.g − k)·mean(T) + k·T), c' = c + m.a·(recol − c). Colour 0 (Default) = no recolour
/// (PS 9544's BaseColorTargetId gate). `base` = the material's un-recoloured constant.
#[derive(Clone, Debug)]
pub struct HueRecolour {
    pub rgb: [f32; 3],
    pub target: [f32; 3],
    pub mask: [f32; 4],
    pub table: String,
    pub mask_image: String,
}

pub fn colour_table_target(store: &mut DataStore, table_path: &str, colour: u8, list: &str) -> Result<[f32; 3], String> {
    let txt = store.read(table_path)?;
    // Nadeo's JSON carries trailing commas before `}` / `]` — dropped before the strict parse
    let s = String::from_utf8_lossy(&txt);
    let mut cleaned = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c == ',' { let next = chars[i + 1..].iter().find(|x| !x.is_whitespace()); if matches!(next, Some('}') | Some(']')) { continue; } }
        cleaned.push(*c);
    }
    let v: serde_json::Value = serde_json::from_str(&cleaned).map_err(|e| format!("{table_path}: {e}"))?;
    let arr = v.get(list).and_then(|a| a.as_array()).ok_or_else(|| format!("{table_path}: no list {list:?}"))?;
    if colour == 0 || colour as usize > arr.len() { return Err(format!("{table_path}: colour {colour} outside the {}-entry list", arr.len())); }
    let hex = arr[colour as usize - 1].as_str().ok_or("colour entry is not a string")?;
    let h = hex.trim_start_matches('#');
    if h.len() < 6 { return Err(format!("{table_path}: colour {hex:?}")); }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|e| e.to_string());
    let (r, g, b) = (byte(0)?, byte(2)?, byte(4)?);
    Ok([crate::gpufmt::srgb_to_linear(r as f32 / 255.0), crate::gpufmt::srgb_to_linear(g as f32 / 255.0), crate::gpufmt::srgb_to_linear(b as f32 / 255.0)])
}

/// The WRAP bilinear mip-0 tap of any pack DDS (BC1 / BC3 / 8-bit) at GPU uv — rgb through the sRGB view, alpha linear.
pub fn mip0_bilinear_wrap_rgba(dds: &[u8], uv: [f32; 2]) -> Result<[f32; 4], String> {
    let mut tex = crate::texsample::parse_dds(dds, Bc1Decode::Expand8Round)?;
    tex.decode_srgb();
    let lv = tex.levels.first().and_then(|s| s.first()).ok_or("no mip 0")?;
    let s = crate::texsample::Sampler::bilinear_no_mip(crate::texsample::Address::Wrap);
    // the GPU texture is the file flipped: GPU v = 1 − file v
    Ok(crate::texsample::fetch_level(lv, &s, uv[0], 1.0 - uv[1]))
}

pub fn hue_recolour(store: &mut DataStore, link: &str, colour: u8, base: [f32; 3]) -> Result<HueRecolour, String> {
    if colour == 0 { return Err("colour 0 (Default): no recolour".into()); }
    let file = if link.to_ascii_lowercase().ends_with(".gbx") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let m = store.load_model(&file)?;
    let g = m.graph()?;
    let mut custom = None;
    for s in &g.slots { if let mapgeom::node::Slot::Node(mapgeom::node::Node::MaterialCustom(c)) = s { custom = Some(c); } }
    let custom = custom.ok_or_else(|| format!("{file}: no CPlugMaterialCustom"))?;
    let slot = |name: &str| custom.bitmaps.iter().find(|(n, _)| n == name).and_then(|(_, r)| g.external(*r)).map(|s| s.to_string());
    let mask_tex = slot("PxzBaseColorHueMask").or_else(|| slot("PyBaseColorHueMask")).or_else(|| slot("BaseColorHueMask")).ok_or_else(|| format!("{file}: no HueMask slot"))?;
    let table = m.externals.iter().map(|(_, p)| p.clone()).find(|p| p.to_ascii_lowercase().ends_with(".colortable.gbx.json")).ok_or_else(|| format!("{file}: no ColorTable external"))?;
    let target = colour_table_target(store, &table, colour, "Classic")?;
    let tm = store.load_model(&mask_tex)?;
    let tg = tm.graph()?;
    let (image, trans) = match &tg.root {
        Some(mapgeom::node::Node::Bitmap(b)) => (tg.external(b.image).ok_or_else(|| format!("{mask_tex}: image not external"))?.to_string(), b.tc_scale_trans.map(|v| [f32::from_bits(v[2]), f32::from_bits(v[3])]).unwrap_or([0.0, 0.0])),
        _ => return Err(format!("{mask_tex}: not a CPlugBitmap")),
    };
    let dds = store.read(&image)?;
    let mask = mip0_bilinear_wrap_rgba(&dds, [0.0, -trans[1]])?;
    let k = (mask[1] - 0.5 * (mask[0] + mask[2])).max(0.0);
    let mean_t = (target[0] + target[1] + target[2]) / 3.0;
    let recol = [((mask[1] - k) * mean_t + k * target[0]).clamp(0.0, 1.0), ((mask[1] - k) * mean_t + k * target[1]).clamp(0.0, 1.0), ((mask[1] - k) * mean_t + k * target[2]).clamp(0.0, 1.0)];
    let rgb = [base[0] + mask[3] * (recol[0] - base[0]), base[1] + mask[3] * (recol[1] - base[1]), base[2] + mask[3] * (recol[2] - base[2])];
    Ok(HueRecolour { rgb, target, mask, table, mask_image: image })
}
