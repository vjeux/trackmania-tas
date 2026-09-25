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
//!   trans`, all 0), and `TMapACosSmoothPy(|n.y| = 0)` = texel 0 = 1.0 of the Techno3 default LUT
//!   `ACosSmoothDefaultPyPxz` (Maniaplanet.pak — not in a collection pack; the capture's 5459 has
//!   65535 there) so the Py term's weight `1 − 1 = 0` and the constant is the Pxz corner mean.
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    PyPxzIds,
    PyPxzProjected,
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
    Ok(MaterialConstant { rgb, family: Family::PyPxzIds, image, uv, ids: [tm.i_py, tm.i_pxz, tm.i_py_x2, tm.i_py_h2] })
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
    let rgb = mip0_bilinear_wrap_srgb(&dds, uv)?;
    Ok(MaterialConstant { rgb, family: Family::PyPxzProjected, image, uv, ids: [-1; 4] })
}

/// The constant of any world-projected material, by its parent shader family; an error names a
/// material whose pre-pass colour is not constant (a mesh-uv textured material: sample its own
/// textures per texel as `setupmap::attr_from_map` does).
pub fn material_constant(store: &mut DataStore, link: &str) -> Result<MaterialConstant, String> {
    let file = if link.to_ascii_lowercase().ends_with(".gbx") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let m = store.load_model(&file)?;
    let parent = m.externals.iter().map(|(_, p)| p.to_ascii_lowercase()).find(|p| p.ends_with(".material.gbx") && !p.eq_ignore_ascii_case(&file.to_ascii_lowercase())).unwrap_or_default();
    if parent.contains("pypxz_ids") {
        terrain_constant(store, link)
    } else if parent.contains("pypxz") {
        projected_constant(store, link)
    } else {
        Err(format!("{link}: parent {parent:?} is not a world-projected (PyPxz) material — its pre-pass colour is per texel"))
    }
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
