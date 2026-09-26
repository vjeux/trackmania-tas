//! The lightmapper's SUN SHADOW MAP, TRANSCRIBED from the capture (passcap/pwc-day, frame 127448, eids 319–410 —
//! the `sun_shadow` entry of MANIFEST.json): a depth-only render of every caster into the 4096² D16_UNORM target
//! 17089 (R16_TYPELESS), cleared to 0.0, through the ORTHOGRAPHIC light camera of the SceneV cbuffer
//! (`GbxV_WorldPrCamera`, the same block for the seven draws), viewport (1, 1, 4094, 4094, 0, 1), rasteriser
//! DepthBias −1 / SlopeScaledDepthBias −1.0 / DepthBiasClamp −0 / CullMode Back / FrontCounterClockwise /
//! DepthClip on, depth func GREATER with writes (reversed z: 1 = near the light, 0 = far), no colour target.
//!
//! The seven draws (logs/draws-frame127448.json.gz, `Drawcall` actions between the clear at eid 319 and 410):
//!
//! | eid | VS / PS | indices × instances | what |
//! |---|---|---|---|
//! | 347, 353, 359 | VS 5394 / PS 937 (`ret`) | 159 / 24 / 12 × 1 | the three items (pad, wall, vegetation card — the card's two quads are OPAQUE here) |
//! | 365 | VS 5394 / PS 937 | 24 × 4096 | the zone tiles (one 32 m quad mesh, `vb_5350`, per instance) |
//! | 377 | VS 1142 / PS 937 | 786 (DrawIndexed) | the sea box (`InstanceStart` = −1 → `VisualToWorld` = identity) |
//! | 394, 410 | VS 14613 / PS 1147 | 3216 / 2376 × 1 | alpha-tested meshes (`TMapAlpha01` BC3 1024² / 128², `GbxShadowAlphaThreshold` 128/255) |
//!
//! VS 5394 / 14613 (Vertex_5394.txt, Vertex_14613.txt) and the `InstanceStart ≥ 0` path of VS 1142: the instance's
//! static-mesh index is `g_Buf_DynaU32s[InstanceStart + SV_InstanceID]` (t115, buffer 2185 — REWRITTEN before every
//! draw: the capture banks it per eid), the transform `g_Buf_StaticMeshs[3·idx]` = the quaternion (x, y, z, w) and
//! `[3·idx + 1].xyz` = the translation (t116, buffer 17163, 4099 × 3 float4; the .w and the third float4 are not read
//! by these shaders — no scale), the rotation rows built with the DXBC's own operation order (instructions 8–24,
//! transcribed in `rotation_rows`), world = (v, 1) · rows (dp4), clip = world · GbxV_WorldPrCamera (dp4 against the
//! four registers = the COLUMNS of the HLSL matrix: the row-vector convention with the translation in the last row).
//! VS 14613 also passes TEXCOORD0 (o1.xy) to PS 1147: `alpha = TMapAlpha01.Sample(SMapAlpha01, uv).w;
//! if (alpha − GbxShadowAlphaThreshold < 0) discard;` — SMapAlpha01 = ClampEdge / anisotropic ×16 / no bias
//! (env/frame127448/samplers.json at eids 394 and 410).
//!
//! The rasteriser follows the Direct3D 11 rules: the viewport transform with the vertices snapped to 1/256 pixel
//! (8 sub-pixel bits, exact integer edge functions), pixel centres at +0.5, the top-left fill rule, the facing from
//! the screen-space winding (FrontCounterClockwise: counter-clockwise on the render target = front), the depth from
//! the triangle's plane through the SNAPPED vertices at the pixel centre, then the bias `DepthBias · r +
//! SlopeScaledDepthBias · MaxDepthSlope` with r = 1/65535 for D16_UNORM (the smallest representable step) and
//! MaxDepthSlope = max(|∂z/∂x|, |∂z/∂y|) of the plane, the biased depth TRUNCATED to a fixed point of 2^-20 (the
//! interpolator's precision — read off the capture, see below), quantised to UNORM16 (round to nearest), and
//! compared GREATER with the stored 16-bit value (D3D compares UNORM depth at the format's precision).
//!
//! What the capture pinned (`lmtool shadow-check` on pwc-day frame 127448, 11 430 201 texels written on either side):
//! * the vertex shader: `mad` = fused multiply-add and `dp4` = mul + three fmas in x, y, z, w order reproduce the
//!   captured post-VS positions of all seven draws BIT FOR BIT (153 316 of 153 316 clip components; separately
//!   rounded products: 126 597);
//! * coverage: 4 texels only ours, 5 only the game's, of 11.43 M (the rasteriser's residual);
//! * depth: 99.316 % of the written texels bit-identical, 0.684 % off by ONE 16-bit step (57 463 with ours =
//!   game + 1, 20 685 with ours = game − 1), none further; the 2^-20 truncation of z + bias was the decisive fact
//!   (round-to-nearest of the exact plane: 96.4 %; a plane through the unsnapped positions: 98.2 %; 2^-19 / 2^-21:
//!   96.6 / 98.0 %; the bias converted to the fixed point separately (floor/round/ceil): 98.7 / 98.5 / 94.6 %;
//!   f32 or fixed-point plane coefficients (floor / truncate / round at 32–40 bits, from the origin, vertex a or the
//!   bbox), truncated barycentrics (20–26 bits), a relative scale of the fixed conversion, the vertex z quantised to
//!   2^-20 / 2^-24 before the plane: no gain). The residual 0.68 % grows with the
//!   screen y / the depth (0.12 % at the top rows, 0.4 % at the bottom) and is not yet modelled;
//! * the chain: `lmtool sun-check --shadow OURS.dds` (the baker's transcribed direct-sun pass fed with OUR shadow
//!   map instead of the captured one) gives the SAME counts against the captured sun_direct target as with the
//!   captured map (16 775 687 exact / 1007 within 1 f16 ulp / 522 worse channel values, 3 087 931 texels) — the
//!   off-by-one texels never flip a PCF compare on this map.
//!
//! `lmtool shadow-check ROOT` runs it on the capture's own inputs and compares with the captured D16 texel by texel;
//! `--dump OUT.dds` writes ours (R16_UNORM, DX10 header) for `lmtool sun-check --shadow OUT.dds` (row 3 on our map).

use crate::passdiff::Buf;

/// The light camera: `GbxV_WorldPrCamera` as the log prints it (HLSL rows; DXBC register k = column k).
#[derive(Clone, Copy, Debug)]
pub struct LightCamera {
    pub world_pr_camera: [[f32; 4]; 4],
}

/// How `dp4` / `mad` evaluate: with fused multiply-adds (what the GPU emits for `mad` and for a dot product's
/// chain) or with separately rounded multiplies and adds. `shadow-check` reports which one reproduces the captured
/// post-VS positions bit for bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arith {
    Fma,
    Separate,
}

#[inline]
fn mad(a: f32, b: f32, c: f32, ar: Arith) -> f32 {
    match ar {
        Arith::Fma => a.mul_add(b, c),
        Arith::Separate => a * b + c,
    }
}

/// `dp4 a, b` as a mul followed by three mads in component order (x, then y, z, w).
#[inline]
pub fn dp4(a: [f32; 4], b: [f32; 4], ar: Arith) -> f32 {
    let t = a[0] * b[0];
    let t = mad(a[1], b[1], t, ar);
    let t = mad(a[2], b[2], t, ar);
    mad(a[3], b[3], t, ar)
}

/// VS 5394 instructions 8–24: the three rows (with the translation in .w) of the instance's transform from the
/// quaternion r1 = (x, y, z, w) and the translation t. Every intermediate is an f32 in the DXBC's own order.
pub fn rotation_rows(q: [f32; 4], t: [f32; 3], ar: Arith) -> [[f32; 4]; 3] {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    // 8: add r2, r1.yzxx, r1.yzxx
    let r2 = [y + y, z + z, x + x, x + x];
    // 9: mul r3.xyz, r1.wwww, r2.xywx
    let r3 = [w * r2[0], w * r2[1], w * r2[3]];
    // 10: mad r1.w, -r1.y, r2.x, 1
    let r1w = mad(-y, r2[0], 1.0, ar);
    // 11: mad r4.x, -r1.z, r2.y, r1.w
    let m00 = mad(-z, r2[1], r1w, ar);
    // 12: mad r1.w, -r1.x, r2.z, 1
    let r1w = mad(-x, r2[2], 1.0, ar);
    // 13: mad r5.y, -r1.z, r2.y, r1.w
    let m11 = mad(-z, r2[1], r1w, ar);
    // 14: mad r6.xyz, r1.xyxx, r2.xyyx, r3.yzxy
    let r6 = [mad(x, r2[0], r3[1], ar), mad(y, r2[1], r3[2], ar), mad(x, r2[1], r3[0], ar)];
    // 15: mad r2.yzw, r1.xxxy, r2.yyxy, -r3.xxyz  (all sources read before the write)
    let r2y = mad(x, r2[1], -r3[0], ar);
    let r2z = mad(x, r2[0], -r3[1], ar);
    let r2w = mad(y, r2[1], -r3[2], ar);
    // 16: mad r0.z, -r1.y, r2.x, r1.w  (r2.x still 2y; r1.w from 12)
    let m22 = mad(-y, r2[0], r1w, ar);
    // 17–24: the rows r4 = (m00, r2.z, r6.z, t.x), r5 = (r6.x, m11, r2.w, t.y), r0 = (r2.y, r6.y, m22, t.z)
    [[m00, r2z, r6[2], t[0]], [r6[0], m11, r2w, t[1]], [r2y, r6[1], m22, t[2]]]
}

/// The identity rows (the `InstanceStart & 0x80000000` branch of VS 5394 gives ZERO rows; VS 1142's
/// `InstanceStart == −1` branch takes `g_CBufferV_Draw.VisualToWorld`, identity for the sea box).
pub fn rows_from_iso(m: &[[f32; 3]; 4]) -> [[f32; 4]; 3] {
    // the DrawV log prints VisualToWorld as 4 rows of 3 (HLSL float3x4 column_major: register k = column k =
    // (m[0][k], m[1][k], m[2][k], m[3][k]) — the dp4 rows of the shader)
    [[m[0][0], m[1][0], m[2][0], m[3][0]], [m[0][1], m[1][1], m[2][1], m[3][1]], [m[0][2], m[1][2], m[2][2], m[3][2]]]
}

/// Instructions 30–39: world = (v, 1) · rows, clip = (world, 1) · GbxV_WorldPrCamera (register k = column k).
#[inline]
pub fn vs_static_mesh(v: [f32; 3], rows: &[[f32; 4]; 3], cam: &LightCamera, ar: Arith) -> [f32; 4] {
    let v4 = [v[0], v[1], v[2], 1.0];
    let world = [dp4(v4, rows[0], ar), dp4(v4, rows[1], ar), dp4(v4, rows[2], ar), 1.0];
    let m = &cam.world_pr_camera;
    let col = |k: usize| [m[0][k], m[1][k], m[2][k], m[3][k]];
    [dp4(world, col(0), ar), dp4(world, col(1), ar), dp4(world, col(2), ar), dp4(world, col(3), ar)]
}

/// The instance stream of a draw: the u32 remap (`g_Buf_DynaU32s`) and the static-mesh table (quaternion,
/// translation, third float4) as bound at that draw.
pub struct InstanceTables {
    pub dyna_u32: Vec<u32>,
    pub static_meshs: Vec<[f32; 4]>,
}

impl InstanceTables {
    pub fn parse(dyna: &[u8], sm: &[u8]) -> InstanceTables {
        let dyna_u32 = dyna.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let f = |c: &[u8]| f32::from_le_bytes(c.try_into().unwrap());
        let static_meshs = sm.chunks_exact(16).map(|c| [f(&c[0..4]), f(&c[4..8]), f(&c[8..12]), f(&c[12..16])]).collect();
        InstanceTables { dyna_u32, static_meshs }
    }
    /// The rows of instance `iid` of a draw with `InstanceStart` (VS 5394 lines 0–29): None = the zero rows.
    pub fn rows(&self, instance_start: u32, iid: u32, ar: Arith) -> Option<[[f32; 4]; 3]> {
        if instance_start & 0x8000_0000 != 0 {
            return None;
        }
        let idx = *self.dyna_u32.get((instance_start.wrapping_add(iid)) as usize)? as usize;
        let q = *self.static_meshs.get(3 * idx)?;
        let t = *self.static_meshs.get(3 * idx + 1)?;
        Some(rotation_rows(q, [t[0], t[1], t[2]], ar))
    }
    /// The static-mesh index an instance resolves to.
    pub fn index(&self, instance_start: u32, iid: u32) -> Option<u32> {
        self.dyna_u32.get((instance_start.wrapping_add(iid)) as usize).copied()
    }
}

/// A caster mesh as the input assembler sees it: positions (POSITION0 R32G32B32_FLOAT), TEXCOORD0 when the
/// layout has one, the u16 index list of the draw.
#[derive(Clone, Debug, Default)]
pub struct CasterMesh {
    pub pos: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
}

impl CasterMesh {
    /// Parse a vertex buffer with the given stride and attribute offsets (`uv_off` = None: no TEXCOORD0).
    pub fn parse(vb: &[u8], stride: usize, pos_off: usize, uv_off: Option<usize>, indices: &[u8]) -> CasterMesh {
        let n = vb.len() / stride;
        let f = |o: usize| f32::from_le_bytes(vb[o..o + 4].try_into().unwrap());
        let pos = (0..n).map(|i| [f(i * stride + pos_off), f(i * stride + pos_off + 4), f(i * stride + pos_off + 8)]).collect();
        let uv0 = match uv_off {
            Some(uo) => (0..n).map(|i| [f(i * stride + uo), f(i * stride + uo + 4)]).collect(),
            None => Vec::new(),
        };
        let indices = indices.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        CasterMesh { pos, uv0, indices }
    }
}

/// The alpha-test texture of PS 1147: the alpha plane of a BC3 mip chain, UNORM8 → f32.
pub struct AlphaTexture {
    pub w: u32,
    pub h: u32,
    /// mip k: (w_k, h_k, alpha values row-major)
    pub mips: Vec<(u32, u32, Vec<f32>)>,
}

/// Decode the alpha plane of a BC3 (DXT5) block: two endpoints and 16 3-bit indices (D3D11 §19.3 — the 8-value
/// mode when a0 > a1, the 6-value mode with 0 and 255 otherwise).
pub fn bc3_alpha_block(b: &[u8]) -> [u8; 16] {
    let a0 = b[0] as u32;
    let a1 = b[1] as u32;
    let mut pal = [0u8; 8];
    pal[0] = a0 as u8;
    pal[1] = a1 as u8;
    if a0 > a1 {
        for i in 1..7u32 {
            pal[(i + 1) as usize] = (((7 - i) * a0 + i * a1) / 7) as u8;
        }
    } else {
        for i in 1..5u32 {
            pal[(i + 1) as usize] = (((5 - i) * a0 + i * a1) / 5) as u8;
        }
        pal[6] = 0;
        pal[7] = 255;
    }
    let mut bits = 0u64;
    for i in 0..6 {
        bits |= (b[2 + i] as u64) << (8 * i);
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = pal[((bits >> (3 * i)) & 7) as usize];
    }
    out
}

impl AlphaTexture {
    /// From a DDS (DX10 header, BC3_UNORM/TYPELESS 77/78/79 or the legacy DXT5 fourcc) with all its mips.
    pub fn from_dds(d: &[u8]) -> Result<AlphaTexture, String> {
        let dds = crate::bc6h::parse_dds(d)?;
        let is_bc3 = matches!(dds.format, 77 | 78 | 79) || (dds.format == 0 && &d[84..88] == b"DXT5");
        if !is_bc3 {
            return Err(format!("not a BC3 texture (dxgi {} fourcc {:?})", dds.format, std::str::from_utf8(&d[84..88]).unwrap_or("?")));
        }
        let mut mips = Vec::new();
        let mut off = 0usize;
        let (mut w, mut h) = (dds.w as u32, dds.h as u32);
        for _ in 0..dds.mips {
            let (bw, bh) = ((w + 3) / 4, (h + 3) / 4);
            let bytes = (bw * bh * 16) as usize;
            if off + bytes > dds.data.len() {
                return Err(format!("truncated BC3 mip chain at {w}×{h}"));
            }
            let mut a = vec![0f32; (w * h) as usize];
            for by in 0..bh {
                for bx in 0..bw {
                    let blk = &dds.data[off + ((by * bw + bx) * 16) as usize..];
                    let vals = bc3_alpha_block(&blk[..8]);
                    for py in 0..4 {
                        for px in 0..4 {
                            let (x, y) = (bx * 4 + px, by * 4 + py);
                            if x < w && y < h {
                                a[(y * w + x) as usize] = vals[(py * 4 + px) as usize] as f32 / 255.0;
                            }
                        }
                    }
                }
            }
            mips.push((w, h, a));
            off += bytes;
            if w == 1 && h == 1 {
                break;
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        Ok(AlphaTexture { w: dds.w as u32, h: dds.h as u32, mips })
    }

    #[inline]
    fn texel(&self, mip: usize, x: i64, y: i64) -> f32 {
        let (w, h, a) = &self.mips[mip];
        // ClampEdge addressing
        let xi = x.clamp(0, *w as i64 - 1) as usize;
        let yi = y.clamp(0, *h as i64 - 1) as usize;
        a[yi * *w as usize + xi]
    }

    /// One bilinear fetch at texture coordinates (u, v) in mip `mip` (D3D: the sample point in texel space is
    /// u·w − 0.5, the weights from its fractional part).
    pub fn bilinear(&self, mip: usize, u: f32, v: f32) -> f32 {
        let (w, h, _) = &self.mips[mip];
        let x = u * *w as f32 - 0.5;
        let y = v * *h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i64, y0 as i64);
        let top = self.texel(mip, xi, yi) * (1.0 - fx) + self.texel(mip, xi + 1, yi) * fx;
        let bot = self.texel(mip, xi, yi + 1) * (1.0 - fx) + self.texel(mip, xi + 1, yi + 1) * fx;
        top * (1.0 - fy) + bot * fy
    }

    /// `Sample` with an anisotropic sampler (max anisotropy 16, no LOD bias, ClampEdge), D3D11 §7.18.11 as the
    /// reference rasteriser does it: the screen-space derivatives of the texture coordinates scaled to texels give
    /// the two footprint axes; ratio = min(major/minor, 16); lod = log2(major / ratio); `ceil(ratio)` trilinear
    /// taps spread along the major axis, averaged.
    pub fn sample_aniso(&self, uv: [f32; 2], dudx: [f32; 2], dudy: [f32; 2], max_aniso: f32) -> f32 {
        let (w, h) = (self.w as f32, self.h as f32);
        let ax = [dudx[0] * w, dudx[1] * h];
        let ay = [dudy[0] * w, dudy[1] * h];
        let lx = (ax[0] * ax[0] + ax[1] * ax[1]).sqrt();
        let ly = (ay[0] * ay[0] + ay[1] * ay[1]).sqrt();
        let (major, minor, axis) = if lx >= ly { (lx, ly, ax) } else { (ly, lx, ay) };
        let ratio = if minor <= 0.0 { max_aniso } else { (major / minor).min(max_aniso).max(1.0) };
        let lod = if major > 0.0 { (major / ratio).log2() } else { 0.0 };
        let last = (self.mips.len() - 1) as f32;
        let lod = lod.clamp(0.0, last);
        let m0 = lod.floor() as usize;
        let m1 = (m0 + 1).min(self.mips.len() - 1);
        let frac = lod - m0 as f32;
        let n = ratio.ceil().max(1.0) as usize;
        // the taps along the major axis in texture-coordinate units: axis / (w, h) is the derivative itself
        let step = if lx >= ly { dudx } else { dudy };
        let mut sum = 0f32;
        for i in 0..n {
            let s = (i as f32 + 0.5) / n as f32 - 0.5;
            let (u, v) = (uv[0] + step[0] * s, uv[1] + step[1] * s);
            let a = self.bilinear(m0, u, v);
            let val = if frac > 0.0 && m1 != m0 { a * (1.0 - frac) + self.bilinear(m1, u, v) * frac } else { a };
            sum += val;
        }
        let _ = axis;
        sum / n as f32
    }
}

/// PS 1147's constant and texture.
pub struct AlphaTest {
    pub threshold: f32,
    pub texture: AlphaTexture,
    pub max_anisotropy: f32,
}

/// One caster draw of the pass.
pub struct CasterDraw {
    pub eid: u64,
    pub mesh: CasterMesh,
    pub instance_start: u32,
    pub instance_count: u32,
    /// VS 1142's `InstanceStart == −1` path: the DrawV VisualToWorld (4 rows of 3 as printed)
    pub visual_to_world: Option<[[f32; 3]; 4]>,
    pub tables: InstanceTables,
    pub alpha: Option<AlphaTest>,
    /// the captured post-VS positions (float4 per vertex, all instances), when banked
    pub vsout: Option<Vec<[f32; 4]>>,
}

/// The rasteriser state of the pass (from the draw's `raster` / `viewport` / `depthstate`).
#[derive(Clone, Copy, Debug)]
pub struct RasterState {
    /// (x, y, w, h, min z, max z)
    pub viewport: [f32; 6],
    pub depth_bias: i32,
    pub slope_scaled_depth_bias: f32,
    pub depth_bias_clamp: f32,
    pub cull_back: bool,
    pub front_ccw: bool,
    pub depth_clip: bool,
    /// how the depth plane is set up and evaluated (the coverage always uses the snapped positions)
    pub plane: PlaneEval,
    /// the fractional bits of the fixed-point increments for the FixedCoef* plane modes
    pub coef_bits: u32,
    /// quantise each vertex's window z to 2^-N (0 = keep the f32) before the plane setup — the depth converted at the
    /// vertex level (a hardware option the capture can rule in or out)
    pub vertex_z_bits: u32,
}

/// How the biased depth reaches the 16-bit target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnormRounding {
    Nearest,
    Truncate,
}

/// The candidates for the GPU's depth-plane arithmetic: the plane through the snapped (or unsnapped) vertex
/// positions evaluated in f64 at the pixel centre; or f32 coefficients (∂z/∂x, ∂z/∂y rounded to f32) with an f32
/// evaluation from vertex a, from the screen origin, or from the triangle's bounding-box corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaneEval {
    F64Snapped,
    F64Unsnapped,
    F32VertexA,
    F32Origin,
    F32Bbox,
    /// f32-rounded coefficients, the evaluation exact (f64) from the screen origin / vertex a / the bbox corner
    F32CoefOrigin,
    F32CoefVertexA,
    F32CoefBbox,
    /// the increments in a fixed point of 2^-N (N = `coef_bits`), truncated toward −∞ (Floor) or toward 0 (Trunc), the
    /// evaluation exact from the screen origin
    FixedCoefFloor,
    FixedCoefTrunc,
    /// the increments rounded to nearest at 2^-N, the evaluation exact from the screen origin
    FixedCoefRound,
    /// increments AND the origin value rounded to nearest at 2^-N
    FixedAllRound,
    /// barycentric weights truncated to 2^-N (N = `coef_bits`), z = Σ wᵢ·zᵢ exact
    BaryFixed,
}

/// The D16 target: the stored 16-bit depths, and per texel the draw that last wrote it (0 = clear).
pub struct ShadowTarget {
    pub w: u32,
    pub h: u32,
    pub depth: Vec<u16>,
    pub source: Vec<u8>,
    /// the biased depth before quantisation (f32) of the last write, for the rounding study
    pub raw: Vec<f32>,
    /// the slope term |SlopeScaledDepthBias · MaxDepthSlope| of the last write (z01 units), for the bias study
    pub slope: Vec<f32>,
    /// the fragment's offset from its triangle's reference vertex (debug)
    pub dxy: Vec<[f32; 2]>,
}

impl ShadowTarget {
    pub fn new(w: u32, h: u32) -> ShadowTarget {
        ShadowTarget { w, h, depth: vec![0; (w * h) as usize], source: vec![0; (w * h) as usize], raw: vec![0.0; (w * h) as usize], slope: vec![0.0; (w * h) as usize], dxy: vec![[0.0; 2]; (w * h) as usize] }
    }
    /// As a Buf of UNORM16 → f32 (what `passdiff` makes of the captured R16).
    pub fn to_buf(&self) -> Buf {
        let mut b = Buf::new(self.w, self.h, 1);
        for i in 0..self.depth.len() {
            b.data[i] = self.depth[i] as f32 / 65535.0;
        }
        b
    }
    /// A DDS (DX10 header, DXGI_FORMAT_R16_UNORM = 56) of the target.
    pub fn to_dds(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(148 + self.depth.len() * 2);
        out.extend_from_slice(b"DDS ");
        let mut hdr = [0u8; 124];
        let put = |h: &mut [u8; 124], o: usize, v: u32| h[o..o + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut hdr, 0, 124);
        put(&mut hdr, 4, 0x1 | 0x2 | 0x4 | 0x1000 | 0x8); // caps, height, width, pixelformat, pitch
        put(&mut hdr, 8, self.h);
        put(&mut hdr, 12, self.w);
        put(&mut hdr, 16, self.w * 2);
        put(&mut hdr, 24, 1); // mip count
        put(&mut hdr, 72, 32); // pixel format size
        put(&mut hdr, 76, 0x4); // DDPF_FOURCC
        hdr[80..84].copy_from_slice(b"DX10");
        put(&mut hdr, 104, 0x1000); // caps: texture
        out.extend_from_slice(&hdr);
        // DX10 header: format, dimension (3 = 2D), misc, array size, misc2
        for v in [56u32, 3, 0, 1, 0] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for d in &self.depth {
            out.extend_from_slice(&d.to_le_bytes());
        }
        out
    }
}

/// A vertex after the viewport transform: fixed-point x, y (1/256 pixel), f32 depth, the attributes.
#[derive(Clone, Copy, Debug)]
struct ScreenVertex {
    fx: i64,
    fy: i64,
    /// the unsnapped viewport position
    x: f32,
    y: f32,
    z: f32,
    uv: [f32; 2],
}

const SUBPIX: f64 = 256.0;

fn to_screen(clip: [f32; 4], uv: [f32; 2], vp: &[f32; 6]) -> ScreenVertex {
    // D3D11 viewport transform: X = (x/w + 1)/2 · W + TopLeftX, Y = (1 − y/w)/2 · H + TopLeftY, Z = z/w · (max − min) + min;
    // an orthographic w = 1. The x/y are snapped to the 8-bit sub-pixel grid (rounded to nearest).
    let inv_w = 1.0 / clip[3];
    let x = (clip[0] * inv_w * 0.5 + 0.5) * vp[2] + vp[0];
    let y = (0.5 - clip[1] * inv_w * 0.5) * vp[3] + vp[1];
    let z = clip[2] * inv_w * (vp[5] - vp[4]) + vp[4];
    ScreenVertex { fx: (x as f64 * SUBPIX).round() as i64, fy: (y as f64 * SUBPIX).round() as i64, x, y, z, uv }
}

/// Everything a pixel shader / depth stage needs at one covered pixel.
#[derive(Clone, Copy, Debug)]
pub struct Fragment {
    pub x: u32,
    pub y: u32,
    /// the interpolated (unbiased) depth at the pixel centre
    pub z: f32,
    /// max(|∂z/∂x|, |∂z/∂y|) of the triangle's plane, per pixel
    pub max_depth_slope: f32,
    pub uv: [f32; 2],
    pub duv_dx: [f32; 2],
    pub duv_dy: [f32; 2],
    /// the pixel centre's offset from the reference vertex (a) in pixels (debug: where does an interpolation error grow?)
    pub dxy: [f32; 2],
}

/// Rasterise one triangle (clip-space vertices, per-vertex TEXCOORD0) with the D3D11 rules; `f` gets every
/// covered pixel centre. Returns false when the triangle was culled or degenerate.
pub fn rasterise(clip: [[f32; 4]; 3], uv: [[f32; 2]; 3], st: &RasterState, w: u32, h: u32, mut f: impl FnMut(Fragment)) -> bool {
    let v: Vec<ScreenVertex> = (0..3).map(|i| { let mut s = to_screen(clip[i], uv[i], &st.viewport); if st.vertex_z_bits > 0 { let q = (1u64 << st.vertex_z_bits) as f64; s.z = ((s.z as f64 * q).floor() / q) as f32; } s }).collect();
    // twice the signed area in sub-pixel units; with x right and y down a positive value is a CLOCKWISE triangle
    // on the render target
    let area2 = (v[1].fx - v[0].fx) * (v[2].fy - v[0].fy) - (v[2].fx - v[0].fx) * (v[1].fy - v[0].fy);
    if area2 == 0 {
        return false;
    }
    let ccw = area2 < 0;
    let front = if st.front_ccw { ccw } else { !ccw };
    if st.cull_back && !front {
        return false;
    }
    // orient counter-clockwise (area2 < 0 in this frame) → inside = e ≤ 0 … simpler: reorder so area2 > 0 and use
    // e ≥ 0 with the top-left rule on the boundary
    let (a, b, c) = if area2 > 0 { (v[0], v[1], v[2]) } else { (v[0], v[2], v[1]) };
    let area2 = area2.abs();
    // pixel bounds (centres at fx = 256·x + 128), within the viewport rectangle (D3D rasterises only the pixels
    // inside the viewport: TopLeft..TopLeft + size − 1) and the target
    let vx0 = (st.viewport[0].floor() as i64).max(0);
    let vy0 = (st.viewport[1].floor() as i64).max(0);
    let vx1 = ((st.viewport[0] + st.viewport[2]).ceil() as i64 - 1).min(w as i64 - 1);
    let vy1 = ((st.viewport[1] + st.viewport[3]).ceil() as i64 - 1).min(h as i64 - 1);
    let minx = [a.fx, b.fx, c.fx].iter().min().copied().unwrap();
    let maxx = [a.fx, b.fx, c.fx].iter().max().copied().unwrap();
    let miny = [a.fy, b.fy, c.fy].iter().min().copied().unwrap();
    let maxy = [a.fy, b.fy, c.fy].iter().max().copied().unwrap();
    let px0 = ((minx - 128).div_euclid(256)).max(vx0);
    let px1 = ((maxx - 128).div_euclid(256) + 1).min(vx1);
    let py0 = ((miny - 128).div_euclid(256)).max(vy0);
    let py1 = ((maxy - 128).div_euclid(256) + 1).min(vy1);
    if px0 > px1 || py0 > py1 {
        return true;
    }
    // edge function e(p, q, s) = (q − p) × (s − p); inside (with area2 > 0) is e ≥ 0; a top or left edge owns its
    // boundary pixels: in this frame (positive area = clockwise on screen) an edge is "top" when it is horizontal
    // and the interior lies below it (dx > 0), "left" when it goes up the screen (dy < 0)
    let tl = |p: &ScreenVertex, q: &ScreenVertex| -> bool {
        let (dx, dy) = (q.fx - p.fx, q.fy - p.fy);
        (dy == 0 && dx > 0) || dy < 0
    };
    let tls = [tl(&a, &b), tl(&b, &c), tl(&c, &a)];
    // the plane of z and uv over the screen in f64 (pixels): gradients from the sub-pixel positions (or the unsnapped ones)
    let inv_area = 1.0 / area2 as f64;
    let unsnapped = st.plane == PlaneEval::F64Unsnapped;
    let pos = |v: &ScreenVertex| -> (f64, f64) { if unsnapped { (v.x as f64, v.y as f64) } else { (v.fx as f64 / SUBPIX, v.fy as f64 / SUBPIX) } };
    let (ax, ay) = pos(&a);
    let (bx, by) = pos(&b);
    let (cx, cy) = pos(&c);
    // 1 / (2·area in pixel²) of the plane's own triangle
    let inv_area_px = if unsnapped { 1.0 / ((bx - ax) * (cy - ay) - (cx - ax) * (by - ay)) } else { inv_area * SUBPIX * SUBPIX };
    let grad = |va: f64, vb: f64, vc: f64| -> (f64, f64) {
        // d/dx, d/dy of the linear function through (a, va), (b, vb), (c, vc)
        let dx = ((vb - va) * (cy - ay) - (vc - va) * (by - ay)) * inv_area_px;
        let dy = ((vc - va) * (bx - ax) - (vb - va) * (cx - ax)) * inv_area_px;
        (dx, dy)
    };
    let (dzdx, dzdy) = grad(a.z as f64, b.z as f64, c.z as f64);
    let (dudx, dudy) = grad(a.uv[0] as f64, b.uv[0] as f64, c.uv[0] as f64);
    let (dvdx, dvdy) = grad(a.uv[1] as f64, b.uv[1] as f64, c.uv[1] as f64);
    let max_slope = dzdx.abs().max(dzdy.abs()) as f32;
    let duv_dx = [dudx as f32, dvdx as f32];
    let duv_dy = [dudy as f32, dvdy as f32];
    let edge = |p: &ScreenVertex, q: &ScreenVertex, sx: i64, sy: i64| -> i64 { (q.fx - p.fx) * (sy - p.fy) - (q.fy - p.fy) * (sx - p.fx) };
    for py in py0..=py1 {
        let sy = py * 256 + 128;
        for px in px0..=px1 {
            let sx = px * 256 + 128;
            let e0 = edge(&a, &b, sx, sy);
            let e1 = edge(&b, &c, sx, sy);
            let e2 = edge(&c, &a, sx, sy);
            let inside = (e0 > 0 || (e0 == 0 && tls[0])) && (e1 > 0 || (e1 == 0 && tls[1])) && (e2 > 0 || (e2 == 0 && tls[2]));
            if !inside {
                continue;
            }
            // the plane evaluated at the pixel centre: z = z_ref + ∂z/∂x·(x − x_ref) + ∂z/∂y·(y − y_ref)
            let (dx, dy) = (px as f64 + 0.5 - ax, py as f64 + 0.5 - ay);
            let _ = inv_area;
            let z = match st.plane {
                PlaneEval::F64Snapped | PlaneEval::F64Unsnapped => (a.z as f64 + dzdx * dx + dzdy * dy) as f32,
                PlaneEval::F32VertexA => (dzdx as f32).mul_add(dx as f32, (dzdy as f32).mul_add(dy as f32, a.z)),
                PlaneEval::F32Origin => { let z0 = (a.z as f64 - dzdx * ax - dzdy * ay) as f32; (dzdx as f32).mul_add(px as f32 + 0.5, (dzdy as f32).mul_add(py as f32 + 0.5, z0)) }
                PlaneEval::F32Bbox => { let (bxp, byp) = (px0 as f64 + 0.5, py0 as f64 + 0.5); let z0 = (a.z as f64 + dzdx * (bxp - ax) + dzdy * (byp - ay)) as f32; (dzdx as f32).mul_add((px - px0) as f32, (dzdy as f32).mul_add((py - py0) as f32, z0)) }
                PlaneEval::F32CoefOrigin => { let z0 = a.z as f64 - dzdx * ax - dzdy * ay; (z0 + (dzdx as f32) as f64 * (px as f64 + 0.5) + (dzdy as f32) as f64 * (py as f64 + 0.5)) as f32 }
                PlaneEval::F32CoefVertexA => (a.z as f64 + (dzdx as f32) as f64 * dx + (dzdy as f32) as f64 * dy) as f32,
                PlaneEval::BaryFixed => {
                    let s = (1u64 << st.coef_bits) as f64;
                    let (wa, wb, wc) = ((e1 as f64 * inv_area * s).floor() / s, (e2 as f64 * inv_area * s).floor() / s, (e0 as f64 * inv_area * s).floor() / s);
                    (a.z as f64 * wa + b.z as f64 * wb + c.z as f64 * wc) as f32
                }
                PlaneEval::F32CoefBbox => { let (bxp, byp) = (px0 as f64 + 0.5, py0 as f64 + 0.5); let z0 = a.z as f64 + dzdx * (bxp - ax) + dzdy * (byp - ay); (z0 + (dzdx as f32) as f64 * (px - px0) as f64 + (dzdy as f32) as f64 * (py - py0) as f64) as f32 }
                PlaneEval::FixedCoefFloor | PlaneEval::FixedCoefTrunc | PlaneEval::FixedCoefRound | PlaneEval::FixedAllRound => {
                    let s = (1u64 << st.coef_bits) as f64;
                    let q = |v: f64| -> f64 { match st.plane { PlaneEval::FixedCoefFloor => (v * s).floor() / s, PlaneEval::FixedCoefTrunc => (v * s).trunc() / s, _ => (v * s).round() / s } };
                    let (aq, bq) = (q(dzdx), q(dzdy));
                    let z0 = a.z as f64 - dzdx * ax - dzdy * ay;
                    let z0 = if st.plane == PlaneEval::FixedAllRound { q(z0) } else { z0 };
                    (z0 + aq * (px as f64 + 0.5) + bq * (py as f64 + 0.5)) as f32
                }
            };
            let u = (a.uv[0] as f64 + dudx * dx + dudy * dy) as f32;
            let vv = (a.uv[1] as f64 + dvdx * dx + dvdy * dy) as f32;
            f(Fragment { x: px as u32, y: py as u32, z, max_depth_slope: max_slope, uv: [u, vv], duv_dx, duv_dy, dxy: [dx as f32, dy as f32] });
        }
    }
    true
}

/// The bias of a fragment: `DepthBias · r + SlopeScaledDepthBias · MaxDepthSlope`, r = 1/65535 (D16_UNORM: the
/// smallest representable value > 0), clamped by DepthBiasClamp when that is not 0.
#[inline]
pub fn depth_bias_d16(st: &RasterState, max_depth_slope: f32) -> f32 {
    let r = 1.0f32 / 65535.0;
    let mut bias = st.depth_bias as f32 * r + st.slope_scaled_depth_bias * max_depth_slope;
    if st.depth_bias_clamp > 0.0 {
        bias = bias.min(st.depth_bias_clamp);
    } else if st.depth_bias_clamp < 0.0 {
        bias = bias.max(st.depth_bias_clamp);
    }
    bias
}

/// f32 in [0, 1] → UNORM16.
#[inline]
pub fn to_unorm16(z: f32, r: UnormRounding) -> u16 {
    let v = z.clamp(0.0, 1.0) * 65535.0;
    match r {
        UnormRounding::Nearest => (v + 0.5).floor().min(65535.0) as u16,
        UnormRounding::Truncate => v.min(65535.0) as u16,
    }
}

/// Statistics of one draw's rasterisation.
#[derive(Clone, Debug, Default)]
pub struct DrawStats {
    pub triangles: usize,
    pub culled: usize,
    pub fragments: usize,
    pub alpha_discarded: usize,
    pub depth_passed: usize,
    pub depth_clipped: usize,
}

/// Options of the run.
#[derive(Clone, Copy, Debug)]
pub struct RunOpts {
    pub arith: Arith,
    pub unorm: UnormRounding,
    pub alpha_test: bool,
    /// the depth is truncated to a fixed point with this many fractional bits before the UNORM16 conversion (0 = none):
    /// the interpolator's internal precision
    pub depth_fixed_bits: u32,
    /// where the truncation applies: false = to z + bias, true = to z only (the bias added after)
    pub fixed_before_bias: bool,
    /// the depth pipeline in units of 1/2^k of a D16 step: z → floor(z·65535·2^k), the bias → the same units by
    /// `bias_round` (0 floor, 1 round, 2 ceil, 3 = added in float before the conversion), the sum → (sum + 2^(k−1)) >> k
    pub step_fixed_k: u32,
    pub bias_round: u32,
    /// the fixed conversion scale is 2^k · (1 − scale_ulps · 2^-24) (a relative shrink of the depth before the truncation)
    pub scale_ulps: f64,
}

/// Draw one caster into the target. `tag` marks the texels it writes.
pub fn draw_caster(d: &CasterDraw, cam: &LightCamera, st: &RasterState, tgt: &mut ShadowTarget, tag: u8, o: &RunOpts) -> DrawStats {
    let mut stats = DrawStats::default();
    let (w, h) = (tgt.w, tgt.h);
    let count = d.instance_count.max(1);
    for iid in 0..count {
        let rows = match (&d.visual_to_world, d.instance_start) {
            (Some(m), 0xffff_ffff) => rows_from_iso(m),
            _ => match d.tables.rows(d.instance_start, iid, o.arith) {
                Some(r) => r,
                None => [[0.0; 4]; 3], // the shader's zero rows: everything collapses onto the translation-free origin
            },
        };
        let clip: Vec<[f32; 4]> = d.mesh.pos.iter().map(|p| vs_static_mesh(*p, &rows, cam, o.arith)).collect();
        for tri in d.mesh.indices.chunks_exact(3) {
            stats.triangles += 1;
            let ids = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            let c = [clip[ids[0]], clip[ids[1]], clip[ids[2]]];
            let uv = if d.mesh.uv0.is_empty() { [[0.0; 2]; 3] } else { [d.mesh.uv0[ids[0]], d.mesh.uv0[ids[1]], d.mesh.uv0[ids[2]]] };
            let drawn = rasterise(c, uv, st, w, h, |fr| {
                stats.fragments += 1;
                // DepthClip: a fragment beyond the near/far planes is clipped (before the bias)
                if st.depth_clip && !(fr.z >= 0.0 && fr.z <= 1.0) {
                    stats.depth_clipped += 1;
                    return;
                }
                if let (Some(at), true) = (&d.alpha, o.alpha_test) {
                    let a = at.texture.sample_aniso(fr.uv, fr.duv_dx, fr.duv_dy, at.max_anisotropy);
                    // 1: add r0.x, alpha, -threshold ; 2: lt r0.x, r0.x, 0 ; 3: discard_nz
                    if a - at.threshold < 0.0 {
                        stats.alpha_discarded += 1;
                        return;
                    }
                }
                let bias = depth_bias_d16(st, fr.max_depth_slope);
                let (zb, q) = if o.step_fixed_k > 0 {
                    // the depth pipeline in a binary fixed point of the [0, 1] range: units of 2^-k (`--fixed-k`), the
                    // interpolated z truncated to it, the bias converted by `bias_round` (0 floor, 1 round, 2 ceil,
                    // 3 = added in float before the truncation), the 16-bit value = round(sum · 65535)
                    let s = (1u64 << o.step_fixed_k) as f64 * (1.0 - o.scale_ulps * 2f64.powi(-24));
                    let (zq, bq) = match o.bias_round {
                        0 => ((fr.z as f64 * s).floor(), (bias as f64 * s).floor()),
                        1 => ((fr.z as f64 * s).floor(), (bias as f64 * s).round()),
                        2 => ((fr.z as f64 * s).floor(), (bias as f64 * s).ceil()),
                        _ => (((fr.z as f64 + bias as f64) * s).floor(), 0.0),
                    };
                    let sum = (zq + bq).max(0.0) / s;
                    (sum as f32, (sum * 65535.0 + 0.5).floor().min(65535.0) as u16)
                } else {
                    let zb = if o.depth_fixed_bits == 0 {
                        fr.z + bias
                    } else {
                        let s = (1u64 << o.depth_fixed_bits) as f64;
                        if o.fixed_before_bias { ((fr.z as f64 * s).floor() / s + bias as f64) as f32 } else { (((fr.z as f64 + bias as f64) * s).floor() / s) as f32 }
                    };
                    (zb, to_unorm16(zb, o.unorm))
                };
                let i = (fr.y * w + fr.x) as usize;
                if q > tgt.depth[i] {
                    tgt.depth[i] = q;
                    tgt.source[i] = tag;
                    tgt.raw[i] = zb;
                    tgt.slope[i] = (st.slope_scaled_depth_bias * fr.max_depth_slope).abs();
                    tgt.dxy[i] = fr.dxy;
                    stats.depth_passed += 1;
                }
            });
            if !drawn {
                stats.culled += 1;
            }
        }
    }
    stats
}

/// Compare our post-VS positions with the captured `vsout` of a draw (float4 per vertex, instance-major):
/// (compared, bit-identical, within 1 ulp, worse, max |Δ|).
pub fn check_vsout(d: &CasterDraw, cam: &LightCamera, ar: Arith) -> Option<(usize, usize, usize, usize, f32)> {
    let vs = d.vsout.as_ref()?;
    let count = d.instance_count.max(1) as usize;
    let nv = d.mesh.pos.len();
    let mut same = 0;
    let mut ulp1 = 0;
    let mut worse = 0;
    let mut compared = 0;
    let mut maxd = 0f32;
    for iid in 0..count {
        let rows = match (&d.visual_to_world, d.instance_start) {
            (Some(m), 0xffff_ffff) => rows_from_iso(m),
            _ => d.tables.rows(d.instance_start, iid as u32, ar).unwrap_or([[0.0; 4]; 3]),
        };
        for (vi, p) in d.mesh.pos.iter().enumerate() {
            let Some(cap) = vs.get(iid * nv + vi) else { continue };
            let ours = vs_static_mesh(*p, &rows, cam, ar);
            for k in 0..4 {
                compared += 1;
                if ours[k].to_bits() == cap[k].to_bits() {
                    same += 1;
                } else {
                    let d = (ours[k] - cap[k]).abs();
                    let ulp = f32::from_bits(cap[k].to_bits().wrapping_add(1)) - cap[k];
                    if d <= ulp.abs() { ulp1 += 1; } else { worse += 1; }
                    if d > maxd { maxd = d; }
                }
            }
        }
    }
    Some((compared, same, ulp1, worse, maxd))
}

/// Texel-by-texel comparison with the captured D16 (as UNORM16 → f32 in a Buf): per source tag the counts of
/// bit-identical / off by one 16-bit step / further, the coverage disagreements, and the worst texel.
#[derive(Clone, Debug, Default)]
pub struct CompareStats {
    pub texels: usize,
    pub both_clear: usize,
    pub only_ours: usize,
    pub only_game: usize,
    pub identical: usize,
    pub off_by_one: usize,
    /// of the off-by-one texels, how many have ours = game + 1 (the rest game − 1)
    pub off_plus: usize,
    pub worse: usize,
    pub worst: Option<(u32, u32, u16, u16)>,
    /// per source tag (1..): (texels written by that draw, identical, off by one, worse, only ours)
    pub per_tag: Vec<(usize, usize, usize, usize, usize)>,
}

pub fn compare_d16(ours: &ShadowTarget, game: &Buf, ntags: usize) -> CompareStats {
    let mut s = CompareStats { per_tag: vec![(0, 0, 0, 0, 0); ntags + 1], ..Default::default() };
    let mut worst_d = 0i32;
    for y in 0..ours.h {
        for x in 0..ours.w {
            let i = (y * ours.w + x) as usize;
            let o = ours.depth[i];
            let g = (game.get(x, y, 0) * 65535.0 + 0.5).floor() as i32;
            let g = g.clamp(0, 65535) as u16;
            s.texels += 1;
            let tag = ours.source[i] as usize;
            if o == 0 && g == 0 {
                s.both_clear += 1;
                continue;
            }
            let pt = &mut s.per_tag[tag.min(ntags)];
            pt.0 += 1;
            if o != 0 && g == 0 {
                s.only_ours += 1;
                pt.4 += 1;
            } else if o == 0 {
                s.only_game += 1;
            }
            let d = (o as i32 - g as i32).abs();
            if d == 0 {
                s.identical += 1;
                pt.1 += 1;
            } else if d == 1 {
                s.off_by_one += 1;
                if o > g { s.off_plus += 1; }
                pt.2 += 1;
            } else {
                s.worse += 1;
                pt.3 += 1;
            }
            if d > worst_d {
                worst_d = d;
                s.worst = Some((x, y, g, o));
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_rows_identity_and_axis_quarter_turns() {
        let r = rotation_rows([0.0, 0.0, 0.0, 1.0], [1.0, 2.0, 3.0], Arith::Separate);
        assert_eq!(r, [[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 2.0], [0.0, 0.0, 1.0, 3.0]]);
        // 90° about y: (x, y, z) → (z, y, −x); q = (0, sin45, 0, cos45)
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let r = rotation_rows([0.0, s, 0.0, s], [0.0; 3], Arith::Fma);
        let v = [1.0f32, 0.0, 0.0, 1.0];
        let w = [dp4(v, r[0], Arith::Fma), dp4(v, r[1], Arith::Fma), dp4(v, r[2], Arith::Fma)];
        assert!((w[0]).abs() < 1e-6 && (w[1]).abs() < 1e-6 && (w[2] + 1.0).abs() < 1e-6, "{w:?}");
    }

    #[test]
    fn dp4_order_is_x_then_y_z_w() {
        // with separate rounding the sum is ((x·a + y·b) + z·c) + w·d
        let a = [1.0f32, 2.0, 3.0, 4.0];
        let b = [0.5f32, 0.25, 0.125, 1.0];
        assert_eq!(dp4(a, b, Arith::Separate), ((0.5 + 0.5) + 0.375) + 4.0);
    }

    #[test]
    fn bc3_alpha_block_both_modes() {
        // 8-value mode: a0 = 255 > a1 = 0, indices all 0 → 255
        let mut b = [0u8; 8];
        b[0] = 255;
        b[1] = 0;
        assert_eq!(bc3_alpha_block(&b), [255u8; 16]);
        // index 1 everywhere → a1: bits 001 repeated
        let mut bits = 0u64;
        for i in 0..16 { bits |= 1u64 << (3 * i); }
        for i in 0..6 { b[2 + i] = (bits >> (8 * i)) as u8; }
        assert_eq!(bc3_alpha_block(&b), [0u8; 16]);
        // 6-value mode: a0 = 0 ≤ a1 = 255, index 7 → 255, index 6 → 0
        let mut c = [0u8; 8];
        c[0] = 0;
        c[1] = 255;
        let mut bits = 0u64;
        for i in 0..16 { bits |= 7u64 << (3 * i); }
        for i in 0..6 { c[2 + i] = (bits >> (8 * i)) as u8; }
        assert_eq!(bc3_alpha_block(&c), [255u8; 16]);
    }

    #[test]
    fn unorm16_rounding_and_bias_unit() {
        assert_eq!(to_unorm16(1.0, UnormRounding::Nearest), 65535);
        assert_eq!(to_unorm16(0.0, UnormRounding::Nearest), 0);
        assert_eq!(to_unorm16(0.5, UnormRounding::Nearest), 32768); // 32767.5 rounds up
        assert_eq!(to_unorm16(0.5, UnormRounding::Truncate), 32767);
        let st = RasterState { viewport: [1.0, 1.0, 4094.0, 4094.0, 0.0, 1.0], depth_bias: -1, slope_scaled_depth_bias: -1.0, depth_bias_clamp: -0.0, cull_back: true, front_ccw: true, depth_clip: true, plane: PlaneEval::F64Snapped, coef_bits: 0, vertex_z_bits: 0 };
        // a flat triangle: only the constant term, one D16 step toward the far plane
        let b = depth_bias_d16(&st, 0.0);
        assert!((b + 1.0 / 65535.0).abs() < 1e-9);
        // a slope of 0.001 per pixel adds −0.001
        assert!((depth_bias_d16(&st, 0.001) - (-1.0 / 65535.0 - 0.001)).abs() < 1e-9);
    }

    fn state() -> RasterState {
        RasterState { viewport: [0.0, 0.0, 8.0, 8.0, 0.0, 1.0], depth_bias: 0, slope_scaled_depth_bias: 0.0, depth_bias_clamp: 0.0, cull_back: false, front_ccw: true, depth_clip: true, plane: PlaneEval::F64Snapped, coef_bits: 0, vertex_z_bits: 0 }
    }

    #[test]
    fn rasteriser_covers_a_quad_once_and_interpolates_depth() {
        // two triangles over the whole 8×8 viewport with z = 0.25 at the left edge and 0.75 at the right
        let st = state();
        let q = [[-1.0f32, 1.0, 0.25, 1.0], [1.0, 1.0, 0.75, 1.0], [1.0, -1.0, 0.75, 1.0], [-1.0, -1.0, 0.25, 1.0]];
        let mut hits = vec![0u32; 64];
        let mut zs = vec![0f32; 64];
        for tri in [[0usize, 1, 2], [0, 2, 3]] {
            rasterise([q[tri[0]], q[tri[1]], q[tri[2]]], [[0.0; 2]; 3], &st, 8, 8, |fr| { hits[(fr.y * 8 + fr.x) as usize] += 1; zs[(fr.y * 8 + fr.x) as usize] = fr.z; });
        }
        assert!(hits.iter().all(|&h| h == 1), "{hits:?}");
        // pixel 0's centre is at x = 0.5 of 8 → z = 0.25 + 0.5·0.5/8
        assert!((zs[0] - (0.25 + 0.5 * 0.0625)).abs() < 1e-6, "{}", zs[0]);
        assert!((zs[7] - (0.25 + 0.5 * 7.5 / 8.0)).abs() < 1e-6);
    }

    #[test]
    fn back_faces_are_culled_by_screen_winding() {
        let mut st = state();
        st.cull_back = true;
        // on the render target (y down) the triangle (−1,1) → (1,1) → (1,−1) runs top-left → top-right → bottom-right:
        // clockwise on screen → a back face under FrontCounterClockwise
        let cw = [[-1.0f32, 1.0, 0.5, 1.0], [1.0, 1.0, 0.5, 1.0], [1.0, -1.0, 0.5, 1.0]];
        let mut n = 0;
        assert!(!rasterise(cw, [[0.0; 2]; 3], &st, 8, 8, |_| n += 1));
        assert_eq!(n, 0);
        let ccw = [cw[0], cw[2], cw[1]];
        assert!(rasterise(ccw, [[0.0; 2]; 3], &st, 8, 8, |_| n += 1));
        assert!(n > 0);
        st.front_ccw = false;
        assert!(rasterise(cw, [[0.0; 2]; 3], &st, 8, 8, |_| {}));
    }

    #[test]
    fn a_d16_dds_round_trips_through_the_loader() {
        let mut t = ShadowTarget::new(4, 2);
        t.depth[5] = 12345;
        let dds = t.to_dds();
        let b = crate::passdiff::load_dds_bytes(&dds, "R16_UNORM", 0, 0).unwrap();
        assert_eq!(b.w, 4);
        assert!((b.get(1, 1, 0) - 12345.0 / 65535.0).abs() < 1e-7);
    }
}
