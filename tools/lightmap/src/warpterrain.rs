//! THE DECORATION'S WARP TERRAIN IN THE PEEL — VS 16748 / PS 16752 transcrib        let rd = |id: u32| -> Result<Vec<u8>, String> { let p = dir.join(format!("e001028_{id}.dds")); std::fs::read(&p).or_else(|_| { let g = std::fs::read(format!("{}.gz", p.display())).map_err(|e| format!("{}: {e}", p.display()))?; crate::passdiff::gunzip(&g) }) };d (pwc-day frame 127448, eids 1028/1033/1071/1076:
//! `Tech3 Warp PyPxzDiff` on the BlueBay decoration's four terrain patches; RE 15 NOTES 08:40Z, E2 2026-09-28).
//!
//! The game's environment render draws the terrain LIT: a triplanar albedo (the Py diffuse at the world xz through
//! `GbxWorldPosToTexCoord_MapPyDiffuse`, the Pxz diffuse at (±z·s, y·s − t) and (∓x·s, y·s − t) with `PxzScaleTrans` (s, s, t),
//! blended by two 1-D `ACosSmooth` LUTs of the normal's azimuth and elevation), a normal from the Pxz normal map through the
//! vertex's TBN, `colour = albedo × (LightAmbientLinear + max(n·(−sunDir), 0)·LDirSun)`, times the `CloudsX2` term (a cloud
//! coverage texture at a world→cloud projection along the sun, lerped between the mood's min/max cloud colours), then the fog
//! lerp `o = Fog + f·(2·colour − Fog)` with f the VS fog factor; back faces black (`and o0.xyz, …, isfrontface`); NO shadow
//! map, NO discard. The port drew every environment surface black (envcap::env_decor, albedo 0) — under the game's untiled
//! structure (g23) every receiver that sees the terrain lost its bounce (baker-5's TOTAL 0.828).
//!
//! Everything below follows the DXBC instruction by instruction in f32 (the permuted tangent frame included — the shader's
//! `r1 = (n.z, n.x, n.y)` bookkeeping is reproduced as written, not "as intended"). The samplers: anisotropic ×16 on every
//! slot (Wrap for the Pxz/Py textures and the clouds, ClampEdge for the LUTs); the port samples bilinear at level 0 — at the
//! peel's 0.5 m/px against 2 texels/m (Pxz/Py) and 0.4 texel/m (clouds) the footprint is ≤ 1 texel, LOD ≤ 0.
//!
//! The textures: from the capture (`WarpTextures::from_capture`: env/frame127448/textures/e001028_*.dds — the two R16 LUTs,
//! the 4096² BC1 Py diffuse, the 6-slice BC1 / BC5 arrays whose slice the check picks, the 512² R8 clouds) or from the packs
//! (`from_pak`: the Warp material's PyDiffuse / PxzDiffuse / PxzNormal slots, the collection's CloudsX2 map).

use crate::texsample::{self, Address, Bc1Decode, Sampler, Texture};

pub type V3 = [f32; 3];

/// The constants of the two shaders for one direction (the SceneV/SceneP/DrawV/ShaderV/ShaderP cbuffers of the draw).
#[derive(Clone, Debug)]
pub struct WarpConsts {
    /// ShaderP.PxzScaleTrans (s, s, t): 0.0005 / 0.5 on BlueBay; 0.001 on RedIsland / WhiteShore.
    pub pxz_scale_trans: [f32; 3],
    pub eye: V3,
    /// GbxP_LightDirDirInWorld0 (the direction the light travels) and GbxP_LightDirRgbLinear0.
    pub light_dir: V3,
    pub light_rgb: V3,
    pub ambient: V3,
    /// GbxP_CloudsX2minRGB_Half (min rgb, 0.5) and GbxP_CloudsX2maxRGB_FREE.
    pub clouds_min_half: [f32; 4],
    pub clouds_max: V3,
    pub fog_rgb: V3,
    /// GbxV_WorldToCloudsX2: the two float4 registers (dp4 with (pos, 1)).
    pub world_to_clouds: [[f32; 4]; 2],
    /// GbxWorldPosToTexCoord_MapPyDiffuse: two float4 registers.
    pub world_to_py: [[f32; 4]; 2],
    pub fog: FogConsts,
    /// GbxVisualToWorld (3 float4 rows; identity for the decoration).
    pub visual_to_world: [[f32; 4]; 3],
}

/// The VS fog block (GbxV_Fog_*), as the vertex shader reads it.
#[derive(Clone, Debug)]
pub struct FogConsts {
    pub enable: bool,
    pub depth_st_exp: [f32; 4],
    pub use_exp: bool,
    pub world_to_height: [f32; 4],
    pub muly_out_minmax: [f32; 4],
    pub use_range_y: bool,
    pub water_eq: [f32; 4],
    pub use_water_plane: bool,
    pub world_to_fog_tnl: [f32; 4],
}

/// The VS outputs the PS interpolates (o1..o6), per vertex.
#[derive(Clone, Copy, Debug, Default)]
pub struct VsOut {
    pub o1: [f32; 4],
    pub o2: [f32; 4],
    pub o3: [f32; 4],
    /// (cloud u, cloud v, fog factor)
    pub o4: [f32; 3],
    /// GbxWorldPosToTexCoord_MapPyDiffuse (u, v)
    pub o6: [f32; 2],
}

fn dp4(v: [f32; 4], r: [f32; 4]) -> f32 { v[0] * r[0] + v[1] * r[1] + v[2] * r[2] + v[3] * r[3] }
fn dp3(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

/// VS 16748 on one vertex: `pos` (visual space, w = 1), `nrm` (the authored normal). Instructions 0–73.
pub fn vs_16748(c: &WarpConsts, pos: V3, nrm: V3) -> VsOut {
    let v0 = [pos[0], pos[1], pos[2], 1.0];
    let vtw = &c.visual_to_world;
    // 0–2: the world position
    let r0 = [dp4(v0, vtw[0]), dp4(v0, vtw[1]), dp4(v0, vtw[2])];
    // 3–5: r1 = (n·row2, n·row0, n·row1) — the permuted world normal (r1.x = nz, r1.y = nx, r1.z = ny)
    let rot = |row: [f32; 4]| -> f32 { nrm[0] * row[0] + nrm[1] * row[1] + nrm[2] * row[2] };
    let r1 = [rot(vtw[2]), rot(vtw[0]), rot(vtw[1])];
    // 6–12: r3 = (0, −r1.y, r1.x) (instr 8: `mad r3, −r1, (0,1,0), r2.yzxy` with r2 = (r1.x, 0, 0)) scaled by rsq(r3.y² + r3.z²); r2.w = (0 < that)
    let r3a = [0.0f32, -r1[1], r1[0]];
    let l3 = r3a[1] * r3a[1] + r3a[2] * r3a[2];
    let flag = 0.0 < l3;
    let inv3 = 1.0 / l3.sqrt();
    let r3n = [r3a[0] * inv3, r3a[1] * inv3, r3a[2] * inv3];
    // 13–16: r4 = normalize over xy of (−r1.x, r1.z, 0)
    let r4a = [-r1[0], r1[2], 0.0f32];
    let l4 = r4a[0] * r4a[0] + r4a[1] * r4a[1];
    let inv4 = 1.0 / l4.sqrt();
    let r4n = [r4a[0] * inv4, r4a[1] * inv4, r4a[2] * inv4];
    // 17: T = flag ? r3n : r4n  (r2.yzw)
    let t = if flag { r3n } else { r4n };
    // 18–19: r3 = (r1.z·T.y − r1.x·T.x, r1.x·T.z − r1.y·T.y, r1.y·T.x − r1.z·T.z)  (as written)
    let m = [r1[0] * t[0], r1[1] * t[1], r1[2] * t[2]];
    let b = [r1[2] * t[1] - m[0], r1[0] * t[2] - m[1], r1[1] * t[0] - m[2]];
    // 20: r4 = world pos − eye
    let r4 = [r0[0] - c.eye[0], r0[1] - c.eye[1], r0[2] - c.eye[2]];
    let p4 = [r0[0], r0[1], r0[2], 1.0];
    // 29–30: the cloud projection
    let o4x = dp4(p4, c.world_to_clouds[0]);
    let o4y = dp4(p4, c.world_to_clouds[1]);
    // 31–59: the fog factor
    let o4z = if c.fog.enable {
        let dist = (r4[0] * r4[0] + r4[1] * r4[1] + r4[2] * r4[2]).sqrt();
        let lin = dist * c.fog.depth_st_exp[0] + c.fog.depth_st_exp[1];
        let e = lin.max(0.0).ln() * c.fog.depth_st_exp[2];
        let e = e.exp();
        let mut r1x = if c.fog.use_exp { e } else { lin };
        let h = dp4(p4, c.fog.world_to_height).max(c.fog.muly_out_minmax[0]).min(c.fog.muly_out_minmax[1]);
        let ranged = h * r1x;
        if c.fog.use_range_y { r1x = ranged; }
        let eye_w = dp4([c.eye[0], c.eye[1], c.eye[2], 1.0], c.fog.water_eq);
        let pos_w = dp4(p4, c.fog.water_eq);
        let both = eye_w < -0.001 && 0.001 < pos_w;
        let ratio = pos_w / (pos_w - eye_w) * r1x;
        let water = if both { ratio } else { r1x };
        // 54: `movc_sat r1.x, UseWaterPlane, r1.w, r1.x` — the SATURATE applies whichever branch is taken
        r1x = (if c.fog.use_water_plane { water } else { r1x }).clamp(0.0, 1.0);
        let f = r1x * c.fog.muly_out_minmax[2] + c.fog.muly_out_minmax[3];
        1.0 - f
    } else {
        dp4(p4, c.fog.world_to_fog_tnl)
    };
    // 60–61: the Py diffuse uv
    let o6 = [dp4(p4, c.world_to_py[0]), dp4(p4, c.world_to_py[1])];
    // 62–72: o1 = (T.z, B.x, r1.y, r4.x), o2 = (T.x, B.y, r1.z, r4.y), o3 = (T.y, B.z, r1.x, r4.z)
    VsOut {
        o1: [t[2], b[0], r1[1], r4[0]],
        o2: [t[0], b[1], r1[2], r4[1]],
        o3: [t[1], b[2], r1[0], r4[2]],
        o4: [o4x, o4y, o4z],
        o6,
    }
}

/// The interpolated VS outputs at barycentrics `bw` over a triangle's three `VsOut`s (linear, as the orthographic peel does).
pub fn interp(v: &[VsOut; 3], bw: [f32; 3]) -> VsOut {
    let mix4 = |k: fn(&VsOut) -> [f32; 4]| -> [f32; 4] { let (a, b, c) = (k(&v[0]), k(&v[1]), k(&v[2])); [a[0] * bw[0] + b[0] * bw[1] + c[0] * bw[2], a[1] * bw[0] + b[1] * bw[1] + c[1] * bw[2], a[2] * bw[0] + b[2] * bw[1] + c[2] * bw[2], a[3] * bw[0] + b[3] * bw[1] + c[3] * bw[2]] };
    let o4 = [v[0].o4[0] * bw[0] + v[1].o4[0] * bw[1] + v[2].o4[0] * bw[2], v[0].o4[1] * bw[0] + v[1].o4[1] * bw[1] + v[2].o4[1] * bw[2], v[0].o4[2] * bw[0] + v[1].o4[2] * bw[1] + v[2].o4[2] * bw[2]];
    let o6 = [v[0].o6[0] * bw[0] + v[1].o6[0] * bw[1] + v[2].o6[0] * bw[2], v[0].o6[1] * bw[0] + v[1].o6[1] * bw[1] + v[2].o6[1] * bw[2]];
    VsOut { o1: mix4(|o| o.o1), o2: mix4(|o| o.o2), o3: mix4(|o| o.o3), o4, o6 }
}

/// The textures PS 16752 samples.
pub struct WarpTextures {
    /// TMapACosSmooth / TMapACosSmoothPy: 1024 R16_UNORM values.
    pub acos: Vec<f32>,
    pub acos_py: Vec<f32>,
    pub py_diffuse: Texture,
    pub pxz_diffuse: Texture,
    /// The BC5 normal map as (x, y) in [0, 1] per texel (levels[0][0] holds (x, y, 0, 1)).
    pub pxz_normal: Texture,
    /// TBindedMapCloudsX2: R8_UNORM.
    pub clouds: Texture,
    /// Which slice of the captured arrays (the check's choice; 0 for the pak's plain textures).
    pub slice: u32,
}

fn lut_from_r16(dds: &[u8]) -> Result<Vec<f32>, String> {
    let t = texsample::parse_dds(dds, Bc1Decode::Ideal)?;
    let lv = &t.levels[0][0];
    Ok((0..lv.w).map(|x| lv.get(x, 0)[0]).collect())
}

/// A texture's level 0 of one slice decoded linearly (sRGB when `srgb`).
fn one_slice(t: &Texture, slice: u32) -> Texture {
    let mut out = t.clone();
    let s = (slice as usize).min(out.levels.len().saturating_sub(1));
    let keep = out.levels.remove(s);
    out.levels = vec![keep];
    out.slices = 1;
    out
}

impl WarpTextures {
    /// From the pwc-day capture's env textures at eid 1028 (`root/env/frame127448/textures/e001028_<id>.dds`): the LUTs 5457 (t0)
    /// and 16813 (t1), the Py diffuse 16796 (t2), the Pxz diffuse array 5354 (t3), the Pxz normal array 5359 (t4), the clouds 6140.
    pub fn from_capture(root: &std::path::Path, frame: u32, slice: u32, srgb: bool) -> Result<WarpTextures, String> {
        let dir = root.join(format!("env/frame{frame}/textures"));
        let rd = |id: u32| -> Result<Vec<u8>, String> { let p = dir.join(format!("e001028_{id}.dds")); std::fs::read(&p).or_else(|_| { let g = std::fs::read(format!("{}.gz", p.display())).map_err(|e| format!("{}: {e}", p.display()))?; crate::passdiff::gunzip(&g) }) };
        let acos = lut_from_r16(&rd(5457)?)?;
        let acos_py = lut_from_r16(&rd(16813)?)?;
        let mut py = texsample::parse_dds(&rd(16796)?, Bc1Decode::Expand8Round)?;
        if srgb { py.decode_srgb(); }
        let pxz_all = texsample::parse_dds(&rd(5354)?, Bc1Decode::Expand8Round)?;
        let mut pxz = one_slice(&pxz_all, slice);
        if srgb { pxz.decode_srgb(); }
        let nrm_all = parse_bc5_dds(&rd(5359)?)?;
        let nrm = one_slice(&nrm_all, slice);
        let clouds = texsample::parse_dds(&rd(6140)?, Bc1Decode::Ideal)?;
        Ok(WarpTextures { acos, acos_py, py_diffuse: py, pxz_diffuse: pxz, pxz_normal: nrm, clouds, slice })
    }
}

/// A BC5 (two BC4 channels) DDS → a Texture whose texels are (x, y, 0, 1) in [0, 1]; every slice, level 0 only.
pub fn parse_bc5_dds(b: &[u8]) -> Result<Texture, String> {
    if b.len() < 128 || &b[0..4] != b"DDS " { return Err("not a DDS".into()); }
    let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let h = u32_at(12) as usize;
    let w = u32_at(16) as usize;
    let mips = u32_at(28).max(1) as usize;
    let fourcc = &b[84..88];
    let (mut off, slices) = if fourcc == b"DX10" { (148usize, u32_at(140).max(1) as usize) } else { (128usize, 1usize) };
    if fourcc == b"DX10" && u32_at(128) != 83 && u32_at(128) != 82 { return Err(format!("DX10 format {} is not BC5", u32_at(128))); }
    let mut levels: Vec<Vec<texsample::Level>> = Vec::new();
    for _s in 0..slices {
        let (bw, bh) = ((w + 3) / 4, (h + 3) / 4);
        let mut px = vec![[0f32; 4]; w * h];
        for by in 0..bh {
            for bx in 0..bw {
                let o = off + (by * bw + bx) * 16;
                if o + 16 > b.len() { return Err("truncated BC5".into()); }
                let xs = crate::alphatex::bc3_alpha_block(&b[o..o + 8]);
                let ys = crate::alphatex::bc3_alpha_block(&b[o + 8..o + 16]);
                for i in 0..16 {
                    let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                    if x < w && y < h { px[y * w + x] = [xs[i] as f32 / 255.0, ys[i] as f32 / 255.0, 0.0, 1.0]; }
                }
            }
        }
        // skip the whole mip chain of this slice
        let (mut mw, mut mh) = (w, h);
        for _ in 0..mips { off += ((mw + 3) / 4) * ((mh + 3) / 4) * 16; mw = (mw / 2).max(1); mh = (mh / 2).max(1); }
        levels.push(vec![texsample::Level::from_f32(w as u32, h as u32, px)]);
    }
    Ok(Texture { fmt: texsample::TexFmt::Unknown(83), w: w as u32, h: h as u32, mips: 1, slices: slices as u32, levels, complete: false })
}

fn sample2(t: &Texture, uv: [f32; 2], wrap: bool) -> [f32; 4] {
    let s = Sampler::trilinear(if wrap { Address::Wrap } else { Address::Clamp });
    texsample::sample(t, 0, &s, uv, [0.0; 2], [0.0; 2])
}

/// A 1-D LUT sample (1024×1, ClampEdge, linear): D3D texel centres at (i + 0.5)/1024.
fn lut(l: &[f32], u: f32) -> f32 {
    let n = l.len() as f32;
    let x = (u.clamp(0.0, 1.0) * n - 0.5).clamp(0.0, n - 1.0);
    let i = x.floor() as usize;
    let f = x - i as f32;
    let j = (i + 1).min(l.len() - 1);
    l[i] * (1.0 - f) + l[j] * f
}

/// PS 16752 on one fragment: the interpolated VS outputs, the facing (`isfrontface`); returns o0.rgb.
pub fn ps_16752(c: &WarpConsts, tex: &WarpTextures, v: &VsOut, front: bool) -> V3 {
    ps_16752_cloud(c, tex, v, front, None)
}

/// `ps_16752` with the CloudsX2 coverage sample replaced by `cloud` when given (0.5 = the field's mean → the factor 0.5 exactly).
pub fn ps_16752_cloud(c: &WarpConsts, tex: &WarpTextures, v: &VsOut, front: bool, cloud: Option<f32>) -> V3 {
    ps_16752_k(c, tex, v, front, cloud, None)
}

/// `ps_16752_cloud` with the whole CloudsX2 FACTOR overridden by `k` when given (the field's mean factor through the shader's
/// arithmetic — `cloud_mean_factor`; RE 15 07:02Z: BlueBay Day's Clouds.tga has c ∈ [0.2, 0.5], mean k = (0.388, 0.398, 0.403)).
pub fn ps_16752_k(c: &WarpConsts, tex: &WarpTextures, v: &VsOut, front: bool, cloud: Option<f32>, k: Option<V3>) -> V3 {
    let (v1, v2, v3, v4, v6) = (v.o1, v.o2, v.o3, v.o4, v.o6);
    // 0–5: r0 = normalize(v1.z, v2.z, v3.z)
    let mut r0 = [v1[2], v2[2], v3[2]];
    let l = 1.0 / dp3(r0, r0).sqrt();
    r0 = [r0[0] * l, r0[1] * l, r0[2] * l];
    // 6–10: r0.w = ACosSmooth(|r0.x / √(r0.x² + r0.z²)|)
    let r0w = (r0[0] * (1.0 / (r0[0] * r0[0] + r0[2] * r0[2]).sqrt())).abs();
    let w_x = lut(&tex.acos, r0w);
    // 11: r0.z < 0
    let neg_z = r0[2] < 0.0;
    // 12–15: r1 = (v3.w, v2.w, v1.w, v2.w) + eye.zyxy = (pos.z, pos.y, pos.x, pos.y)
    let r1 = [v3[3] + c.eye[2], v2[3] + c.eye[1], v1[3] + c.eye[0], v2[3] + c.eye[1]];
    // 16–17: r1.xz *= s; r2.zw = r1.yw·s − t
    let s = c.pxz_scale_trans[0];
    let r1x = r1[0] * s;
    let r1z = r1[2] * s;
    let r2z = r1[1] * c.pxz_scale_trans[1] - c.pxz_scale_trans[2];
    let r2w = r1[3] * c.pxz_scale_trans[1] - c.pxz_scale_trans[2];
    // 18: r2.x = neg_z ? −r1.z : r1.z
    let r2x = if neg_z { -r1z } else { r1z };
    // 19–20: the x-facing projection at (r2.x, r2.w): normal.xy, diffuse
    let n_a = sample2(&tex.pxz_normal, [r2x, r2w], true);
    let d_a = sample2(&tex.pxz_diffuse, [r2x, r2w], true);
    // 21–23: r0.x = (0 < r0.x); r0.y = ACosSmoothPy(|r0.y|)
    let pos_x = 0.0 < r0[0];
    let w_y = lut(&tex.acos_py, r0[1].abs());
    // 24–26: r2.y = pos_x ? −r1.x : r1.x; the z-facing projection at (r2.y, r2.z)
    let r2y = if pos_x { -r1x } else { r1x };
    let n_b = sample2(&tex.pxz_normal, [r2y, r2z], true);
    let d_b = sample2(&tex.pxz_diffuse, [r2y, r2z], true);
    // 27–29: n = lerp(n_b, n_a, w_x) on xy, then ×255/128 − 127/128
    let nx = (n_b[0] + w_x * (n_a[0] - n_b[0])) * 1.992188 - 0.992188;
    let ny = (n_b[1] + w_x * (n_a[1] - n_b[1])) * 1.992188 - 0.992188;
    // 30–35: r1.x = nx²+ny²; r1.y = 0.999999 / max(r1.x, 1); r1.x = 1 − r1.y·r1.x; r4 = (nx, ny)·r1.y, √r1.x
    let l2 = nx * nx + ny * ny;
    let inv = 0.999999 / l2.max(1.0);
    let zz = 1.0 - inv * l2;
    let r4 = [nx * inv, ny * inv, zz.sqrt()];
    // 36–39: normalize, then lerp((0,0,1), n_ts, w_y)
    let il = 1.0 / dp3(r4, r4).sqrt();
    let mut r1n = [r4[0] * il, r4[1] * il, r4[2] * il - 1.0];
    r1n = [w_y * r1n[0], w_y * r1n[1], w_y * r1n[2] + 1.0];
    // 40–45: the world normal through (v1.xyz, v2.xyz, v3.xyz), normalized
    let r4w = [dp3(r1n, [v1[0], v1[1], v1[2]]), dp3(r1n, [v2[0], v2[1], v2[2]]), dp3(r1n, [v3[0], v3[1], v3[2]])];
    let il2 = 1.0 / dp3(r4w, r4w).sqrt();
    let nw = [r4w[0] * il2, r4w[1] * il2, r4w[2] * il2];
    // 46–48: ndl = max(n·(−L), 0); r1 = ndl · LightDirRgbLinear0
    let ndl = dp3(nw, [-c.light_dir[0], -c.light_dir[1], -c.light_dir[2]]).max(0.0);
    let sun = [ndl * c.light_rgb[0], ndl * c.light_rgb[1], ndl * c.light_rgb[2]];
    // 49–50: albedo_pxz = lerp(d_b, d_a, w_x)
    let apx = [d_b[0] + w_x * (d_a[0] - d_b[0]), d_b[1] + w_x * (d_a[1] - d_b[1]), d_b[2] + w_x * (d_a[2] - d_b[2])];
    // 51–53: the Py diffuse at v6; albedo = lerp(py, albedo_pxz, w_y)  — r0 = r0.y·(apx − py) + py
    let py = sample2(&tex.py_diffuse, v6, true);
    let alb = [py[0] + w_y * (apx[0] - py[0]), py[1] + w_y * (apx[1] - py[1]), py[2] + w_y * (apx[2] - py[2])];
    // 54–55: colour = albedo·ambient + albedo·sun
    let mut col = [alb[0] * c.ambient[0] + alb[0] * sun[0], alb[1] * c.ambient[1] + alb[1] * sun[1], alb[2] * c.ambient[2] + alb[2] * sun[2]];
    // 56–63: the CloudsX2 term
    let cl = match cloud { Some(k) => k, None => sample2(&tex.clouds, [v4[0], v4[1]], true)[0] };
    let t_hi = (cl * 2.0 - 1.0).clamp(0.0, 1.0);
    let t_lo = (cl + cl).clamp(0.0, 1.0);
    let mn = c.clouds_min_half;
    let low = [t_lo * (mn[3] - mn[0]) + mn[0], t_lo * (mn[3] - mn[1]) + mn[1], t_lo * (mn[3] - mn[2]) + mn[2]];
    let cloud = match k { Some(k) => k, None => [t_hi * (c.clouds_max[0] - low[0]) + low[0], t_hi * (c.clouds_max[1] - low[1]) + low[1], t_hi * (c.clouds_max[2] - low[2]) + low[2]] };
    // 64–67: × cloud, ×2 − fog, lerp by sat(v4.z)
    col = [col[0] * cloud[0], col[1] * cloud[1], col[2] * cloud[2]];
    let f = v4[2].clamp(0.0, 1.0);
    let out = [f * (col[0] * 2.0 - c.fog_rgb[0]) + c.fog_rgb[0], f * (col[1] * 2.0 - c.fog_rgb[1]) + c.fog_rgb[1], f * (col[2] * 2.0 - c.fog_rgb[2]) + c.fog_rgb[2]];
    // 68: back faces black
    if front { out } else { [0.0; 3] }
}

/// The constants from a passcap draws log entry (the terrain draw's record).
pub fn consts_from_draw(d: &serde_json::Value) -> Result<WarpConsts, String> {
    let g = |p: &str| -> Result<serde_json::Value, String> { d.pointer(p).cloned().ok_or_else(|| format!("{p} missing")) };
    let f3 = |v: &serde_json::Value| -> [f32; 3] { let a = v.as_array().expect("float3"); [a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32, a[2].as_f64().unwrap() as f32] };
    let f4 = |v: &serde_json::Value| -> [f32; 4] { let a = v.as_array().expect("float4"); let get = |i: usize| a.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32; [get(0), get(1), get(2), get(3)] };
    let flag = |v: &serde_json::Value| -> bool { v.as_f64().map(|x| x != 0.0).or_else(|| v.as_bool()).unwrap_or(false) };
    // a 4×N (printed) matrix = N float4 registers: register k = the printed column k
    let regs = |v: &serde_json::Value, n: usize| -> Vec<[f32; 4]> {
        let rows = v.as_array().expect("matrix");
        (0..n).map(|k| { let mut r = [0f32; 4]; for (i, row) in rows.iter().enumerate().take(4) { r[i] = row.as_array().and_then(|c| c.get(k)).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32; } r }).collect()
    };
    let sv = "/Vertex/cbuffers/SceneV";
    let sp = "/Pixel/cbuffers/SceneP";
    let vtw_v = g("/Vertex/cbuffers/DrawV/GbxVisualToWorld").ok();
    let vtw: [[f32; 4]; 3] = match vtw_v { Some(v) => { let rows = v.as_array().cloned().unwrap_or_default(); let mut m = [[0f32; 4]; 3]; for i in 0..3 { if let Some(r) = rows.get(i) { m[i] = f4(r); } } m }, None => [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] };
    let w2c = regs(&g(&format!("{sv}/GbxV_WorldToCloudsX2"))?, 2);
    let w2py = regs(&g("/Vertex/cbuffers/ShaderV/GbxWorldPosToTexCoord_MapPyDiffuse").or_else(|_| g("/Vertex/cbuffers/DrawV/GbxWorldPosToTexCoord_MapPyDiffuse"))?, 2);
    Ok(WarpConsts {
        pxz_scale_trans: f3(&g("/Pixel/cbuffers/ShaderP/PxzScaleTrans")?),
        eye: f3(&g(&format!("{sp}/GbxP_EyeInWorld"))?),
        light_dir: f3(&g(&format!("{sp}/GbxP_LightDirDirInWorld0"))?),
        light_rgb: f3(&g(&format!("{sp}/GbxP_LightDirRgbLinear0"))?),
        ambient: f3(&g(&format!("{sp}/GbxP_LightAmbientLinear"))?),
        clouds_min_half: f4(&g(&format!("{sp}/GbxP_CloudsX2minRGB_Half"))?),
        clouds_max: f3(&g(&format!("{sp}/GbxP_CloudsX2maxRGB_FREE"))?),
        fog_rgb: f3(&g(&format!("{sp}/GbxP_Fog_LinearRGB"))?),
        world_to_clouds: [w2c[0], w2c[1]],
        world_to_py: [w2py[0], w2py[1]],
        fog: FogConsts {
            enable: flag(&g(&format!("{sv}/GbxV_Fog_Enable"))?),
            depth_st_exp: f4(&g(&format!("{sv}/GbxV_Fog_DepthST_Exp"))?),
            use_exp: flag(&g(&format!("{sv}/GbxV_Fog_UseExp"))?),
            world_to_height: f4(&g(&format!("{sv}/GbxV_Fog_WorldToHeight"))?),
            muly_out_minmax: f4(&g(&format!("{sv}/GbxV_Fog_MulY_Out_MinMax"))?),
            use_range_y: flag(&g(&format!("{sv}/GbxV_Fog_UseRangeY"))?),
            water_eq: f4(&g(&format!("{sv}/GbxV_Fog_WaterEqInWorld"))?),
            use_water_plane: flag(&g(&format!("{sv}/GbxV_Fog_UseWaterPlane"))?),
            world_to_fog_tnl: f4(&g(&format!("{sv}/GbxV_Fog_WorldToFogTnL"))?),
        },
        visual_to_world: vtw,
    })
}

/// One captured terrain patch: world positions + authored normals (the VB at eid: POSITION f32×3 @0, NORMAL R16G16B16A16_SNORM @12,
/// stride 20) and its index list.
pub struct CapPatch {
    pub eid: u32,
    pub pos: Vec<V3>,
    pub nrm: Vec<V3>,
    pub indices: Vec<u16>,
}

/// The four terrain patches of the pwc-day capture (eids 1028, 1033, 1071, 1076) from `mesh/frame<F>/vb_<res>.bin` per mesh.json.
pub fn capture_patches(root: &std::path::Path, frame: u32) -> Result<Vec<CapPatch>, String> {
    let mesh_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root.join(format!("env/frame{frame}/mesh.json"))).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for eid in [1028u32, 1033, 1071, 1076] {
        let e = mesh_json.as_array().and_then(|a| a.iter().find(|e| e.get("eid").and_then(|v| v.as_u64()) == Some(eid as u64))).ok_or(format!("eid {eid} not in mesh.json"))?;
        let vb = e.get("vertex_buffers").and_then(|v| v.as_array()).and_then(|v| v.first()).ok_or("no VB")?;
        let file = vb.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let stride = vb.get("stride").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
        let bytes = std::fs::read(root.join(format!("mesh/frame{frame}/{file}"))).or_else(|_| std::fs::read(root.join(format!("env/frame{frame}/mesh/{file}")))).map_err(|e| format!("{file}: {e}"))?;
        let n = bytes.len() / stride;
        let f = |o: usize| f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let sn = |o: usize| i16::from_le_bytes([bytes[o], bytes[o + 1]]) as f32 / 32767.0;
        let mut pos = Vec::with_capacity(n);
        let mut nrm = Vec::with_capacity(n);
        for i in 0..n {
            let o = i * stride;
            pos.push([f(o), f(o + 4), f(o + 8)]);
            nrm.push([sn(o + 12).max(-1.0), sn(o + 14).max(-1.0), sn(o + 16).max(-1.0)]);
        }
        let idx = std::fs::read(root.join(format!("mesh/frame{frame}/e{eid:06}_vsout_indices.bin"))).or_else(|_| std::fs::read(root.join(format!("env/frame{frame}/mesh/e{eid:06}_vsout_indices.bin")))).map_err(|e| format!("indices {eid}: {e}"))?;
        let indices: Vec<u16> = idx.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        out.push(CapPatch { eid, pos, nrm, indices });
    }
    Ok(out)
}

/// The captured post-VS outputs (stride 96 = o0..o6 packed) of a patch, for the VS check.
pub fn capture_vsout(root: &std::path::Path, frame: u32, eid: u32) -> Result<Vec<[f32; 24]>, String> {
    let b = std::fs::read(root.join(format!("mesh/frame{frame}/e{eid:06}_vsout.bin"))).or_else(|_| std::fs::read(root.join(format!("env/frame{frame}/mesh/e{eid:06}_vsout.bin")))).map_err(|e| e.to_string())?;
    Ok(b.chunks_exact(96).map(|c| { let mut v = [0f32; 24]; for i in 0..24 { v[i] = f32::from_le_bytes(c[i * 4..i * 4 + 4].try_into().unwrap()); } v }).collect())
}

/// The shading state the peel carries: the constants and the textures. `clouds_from_texture` = sample the capture's cloud field
/// (its translation is the wind's — only the capture's own frames have it); false = the coverage at its mean 0.5 → the factor
/// 0.5 exactly (`sat(2·0.5) = 1 → lerp(min, .w = 0.5, 1) = 0.5; sat(2·0.5 − 1) = 0`), RE 15's recommendation for every other map.
pub struct WarpShading {
    pub consts: WarpConsts,
    pub tex: WarpTextures,
    pub clouds_from_texture: bool,
    /// The CloudsX2 factor's MEAN over the mood's cloud field (`cloud_mean_factor` on the mood's Clouds.tga) when the field is not
    /// sampled; None = the coverage at 0.5 (k = 0.5 exactly — the pre-07:05Z stand-in).
    pub cloud_k: Option<V3>,
}

/// PS 16752 through the shading state at a fragment: `vs` = the triangle's three VS outputs, `bw` its barycentrics, `front` the facing.
pub fn shade(sh: &WarpShading, vs: &[VsOut; 3], bw: [f32; 3], front: bool) -> V3 {
    let v = interp(vs, bw);
    if sh.clouds_from_texture { return ps_16752(&sh.consts, &sh.tex, &v, front); }
    match sh.cloud_k {
        Some(k) => ps_16752_k(&sh.consts, &sh.tex, &v, front, Some(0.5), Some(k)),
        None => ps_16752_cloud(&sh.consts, &sh.tex, &v, front, Some(0.5)),
    }
}

/// The CloudsX2 factor averaged over a mood's cloud field (TBindedMapCloudsX2 = the mood's Clouds.tga: 512² 8-bit grey, TGA type 3;
/// RE 15 07:02Z) through the shader's own arithmetic per texel: `t_lo = sat(2c); low = lerp(min, .w, t_lo); t_hi = sat(2c − 1);
/// k = lerp(low, max, t_hi)`. The field's translation in the peel is the wind's — unreproducible per position — so its mean stands
/// for it (BlueBay Day: c ∈ [0.2, 0.5], mean c 0.317 → mean k (0.388, 0.398, 0.403), 2k ≈ 0.79). Returns (mean k, mean c, texels).
pub fn cloud_mean_factor(tga: &[u8], c: &WarpConsts) -> Result<(V3, f32, usize), String> {
    if tga.len() < 18 { return Err("TGA too short".into()); }
    let id_len = tga[0] as usize;
    let cmap = tga[1];
    let kind = tga[2];
    let w = u16::from_le_bytes([tga[12], tga[13]]) as usize;
    let h = u16::from_le_bytes([tga[14], tga[15]]) as usize;
    let bpp = tga[16] as usize;
    if cmap != 0 || (kind != 3 && kind != 2) { return Err(format!("TGA type {kind} (colour map {cmap}) is not an uncompressed grey/RGB image")); }
    let bytes = bpp / 8;
    let off = 18 + id_len;
    if tga.len() < off + w * h * bytes { return Err("TGA truncated".into()); }
    let mn = c.clouds_min_half;
    let mut sum = [0f64; 3];
    let mut sum_c = 0f64;
    for i in 0..w * h {
        // grey: the byte; RGB/BGR: the first channel (the shader reads .x of an R8 view)
        let v = tga[off + i * bytes] as f32 / 255.0;
        let t_lo = (v + v).clamp(0.0, 1.0);
        let t_hi = (v * 2.0 - 1.0).clamp(0.0, 1.0);
        let low = [t_lo * (mn[3] - mn[0]) + mn[0], t_lo * (mn[3] - mn[1]) + mn[1], t_lo * (mn[3] - mn[2]) + mn[2]];
        for ch in 0..3 { sum[ch] += (t_hi * (c.clouds_max[ch] - low[ch]) + low[ch]) as f64; }
        sum_c += v as f64;
    }
    let n = (w * h).max(1) as f64;
    Ok(([(sum[0] / n) as f32, (sum[1] / n) as f32, (sum[2] / n) as f32], (sum_c / n) as f32, w * h))
}

/// The constants for a map WITHOUT a capture: the mood XML's <Fog> (+ <Height>) and <CloudsX2> blocks (the words the game
/// derives from them — checked against pwc-day 127448's SceneV/SceneP at eid 1028: Fog_DepthST_Exp = (1/(DepthMax − DepthMin),
/// −DepthMin/(DepthMax − DepthMin), Exponant, 1); Fog_WorldToHeight = (0, (MulTop − MulBottom)/(YTop − YBottom), 0, MulBottom −
/// YBottom·that); Fog_MulY_Out_MinMax = (min(MulTop, MulBottom), max(…), IntensMax, IntensMin); Fog_LinearRGB = sRGB→linear(Color);
/// CloudsX2minRGB_Half = (linear(MinRgb)·MinRgbX, 0.5), maxRGB = linear(MaxRgb)·MaxRgbX — all to the printed digits), the mood's
/// LDirSun and the sun direction (GbxP_LightDirDirInWorld0 = the direction the light travels), LightAmbientLinear = 0 (the LM peel's
/// value), the material's PxzScaleTrans and its PyDiffuse texcoord transform (scale s, rotation θ: the Py uv registers
/// (s·cos θ, 0, s·sin θ, 0) / (s·sin θ, 0, −s·cos θ, 0) — pwc-day's WarpSand: 0.0005 at 15°). The cloud projection is the capture's
/// wind-dependent field and is not reproduced: `WarpShading::clouds_from_texture = false` (k = 0.5).
pub fn consts_from_mood(xml: &str, light_dir_in_world: V3, l_dir_sun: V3, eye: V3, pxz_scale_trans: [f32; 3], py_scale: f32, py_rot_deg: f32) -> WarpConsts {
    let attr = |tag: &str, name: &str| -> Option<f32> {
        let p = xml.find(&format!("<{tag} "))?;
        let seg = &xml[p..xml[p..].find('>').map(|e| p + e).unwrap_or(xml.len())];
        let k = format!(" {name}=\"");
        let s = seg.find(&k)? + k.len();
        let e = seg[s..].find('"')? + s;
        seg[s..e].parse().ok()
    };
    let attr_s = |tag: &str, name: &str| -> Option<String> {
        let p = xml.find(&format!("<{tag} "))?;
        let seg = &xml[p..xml[p..].find('>').map(|e| p + e).unwrap_or(xml.len())];
        let k = format!(" {name}=\"");
        let s = seg.find(&k)? + k.len();
        let e = seg[s..].find('"')? + s;
        Some(seg[s..e].to_string())
    };
    let srgb = |c: u32| -> f32 { let v = c as f32 / 255.0; if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } };
    let hex3 = |s: &str| -> V3 { let h = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0xffffff); [srgb((h >> 16) & 255), srgb((h >> 8) & 255), srgb(h & 255)] };
    let dmin = attr("Fog", "DepthMin").unwrap_or(256.0);
    let dmax = attr("Fog", "DepthMax").unwrap_or(25000.0);
    let ex = attr("Fog", "Exponant").unwrap_or(0.7);
    let imin = attr("Fog", "IntensMin").unwrap_or(0.0);
    let imax = attr("Fog", "IntensMax").unwrap_or(0.976);
    let enabled = attr("Fog", "Enabled").map(|v| v != 0.0).unwrap_or(true);
    let (yb, yt, mb, mt) = (attr("Height", "YBottom"), attr("Height", "YTop"), attr("Height", "MulBottom"), attr("Height", "MulTop"));
    let (use_range_y, w2h, mm) = match (yb, yt, mb, mt) {
        (Some(yb), Some(yt), Some(mb), Some(mt)) if (yt - yb).abs() > 1e-6 => { let a = (mt - mb) / (yt - yb); (true, [0.0, a, 0.0, mb - yb * a], [mt.min(mb), mt.max(mb), imax, imin]) }
        _ => (false, [0.0; 4], [0.0, 1.0, imax, imin]),
    };
    let fog_rgb = attr_s("Fog", "Color").map(|s| hex3(&s)).unwrap_or([0.479, 0.680, 0.913]);
    let cmin = attr_s("CloudsX2", "MinRgb").map(|s| hex3(&s)).unwrap_or([1.0; 3]);
    let cminx = attr("CloudsX2", "MinRgbX").unwrap_or(0.25);
    let cmax = attr_s("CloudsX2", "MaxRgb").map(|s| hex3(&s)).unwrap_or([1.0; 3]);
    let cmaxx = attr("CloudsX2", "MaxRgbX").unwrap_or(0.25);
    let (sn, cs) = py_rot_deg.to_radians().sin_cos();
    WarpConsts {
        pxz_scale_trans,
        eye,
        light_dir: light_dir_in_world,
        light_rgb: l_dir_sun,
        ambient: [0.0; 3],
        clouds_min_half: [cmin[0] * cminx, cmin[1] * cminx, cmin[2] * cminx, 0.5],
        clouds_max: [cmax[0] * cmaxx, cmax[1] * cmaxx, cmax[2] * cmaxx],
        fog_rgb,
        world_to_clouds: [[0.0; 4], [0.0; 4]],
        world_to_py: [[py_scale * cs, 0.0, py_scale * sn, 0.0], [py_scale * sn, 0.0, -py_scale * cs, 0.0]],
        fog: FogConsts { enable: enabled, depth_st_exp: [1.0 / (dmax - dmin), -dmin / (dmax - dmin), ex, 1.0], use_exp: true, world_to_height: w2h, muly_out_minmax: mm, use_range_y, water_eq: [0.0; 4], use_water_plane: false, world_to_fog_tnl: [0.0, 0.0, -1.0, 1.0] },
        visual_to_world: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
    }
}

/// The Warp material's textures from the packs: the PyDiffuse / PxzDiffuse (sRGB-decoded) and PxzNormal (BC5, raw) slots; the LUTs
/// are the shader-global ones (warp_luts.rs); no cloud field (the mean is used).
pub fn textures_from_pak(store: &mut mapgeom::store::DataStore, link: &str) -> Result<WarpTextures, String> {
    let py = crate::setupmap::slot_texture(store, link, "PyDiffuse", true)?.ok_or(format!("{link}: no PyDiffuse slot"))?.1;
    let pxz = crate::setupmap::slot_texture(store, link, "PxzDiffuse", true)?.ok_or(format!("{link}: no PxzDiffuse slot"))?.1;
    // the normal map: BC5 — through the slot's DDS bytes
    let mat = if link.to_ascii_uppercase().ends_with(".MATERIAL.GBX") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let chain = mapgeom::envblock::material_chain(store, &mat);
    let slot = chain.bitmaps.iter().find(|(n, p)| n.eq_ignore_ascii_case("PxzNormal") && !p.is_empty()).map(|(_, p)| p.clone()).ok_or(format!("{link}: no PxzNormal slot"))?;
    let dds = slot_dds_path(store, &slot);
    let bytes = store.read(&dds).map_err(|e| format!("{dds}: {e}"))?;
    let nrm = parse_bc5_dds(&bytes).or_else(|e| { eprintln!("warp terrain: {dds}: {e} — the normal map read as a colour texture"); texsample::parse_dds(&bytes, Bc1Decode::Expand8Round) })?;
    Ok(WarpTextures { acos: crate::warp_luts::ACOS_SMOOTH.to_vec(), acos_py: crate::warp_luts::ACOS_SMOOTH_PY.to_vec(), py_diffuse: py, pxz_diffuse: pxz, pxz_normal: nrm, clouds: half_texture(), slice: 0 })
}

fn half_texture() -> Texture {
    Texture { fmt: texsample::TexFmt::R8Unorm, w: 1, h: 1, mips: 1, slices: 1, levels: vec![vec![texsample::Level::from_f32(1, 1, vec![[0.5, 0.5, 0.5, 1.0]])]], complete: true }
}

/// A `.Texture.gbx` slot's DDS path (the bitmap node's external image, else the `…\Image\<name>.dds` convention — setupmap's rule).
fn slot_dds_path(store: &mut mapgeom::store::DataStore, slot: &str) -> String {
    let mut out: Option<String> = None;
    if let Ok(m) = store.load_model(slot) { if let Ok(g) = m.graph() { if let Some(mapgeom::node::Node::Bitmap(b)) = &g.root {
        if b.image >= 0 { if let Some(mapgeom::node::Slot::External(dp)) = g.slots.get(b.image as usize) { out = Some(dp.clone()); } }
    } } }
    out.unwrap_or_else(|| {
        if slot.to_ascii_uppercase().ends_with(".TEXTURE.GBX") {
            let stem = &slot[..slot.len() - ".Texture.gbx".len()];
            match stem.rsplit_once('\\') { Some((dir, name)) => format!("{dir}\\Image\\{name}.dds"), None => format!("{stem}.dds") }
        } else { slot.to_string() }
    })
}
