//! ROW 1 — the ATTRIBUTE PRE-PASS, transcribed from the capture (passcap/pwc-day, capture pwc2 frame 127447,
//! logs/draws-frame127447.json.gz, shaders-frame127447/): the LM-space raster of every lightmapped object's
//! MDiffuse (bounce albedo) into the 2048² RGBA16F atlas 17001, nine jittered runs (three per frame; frame
//! 127447 = runs 6, 7, 8 by their `LM01_Trans_RasterSS` and the 7/9 alpha of the sum after its first run)
//! summed into 16963 by PS 1109, resolved into the B8G8R8A8_SRGB MDiffuse 16969 by PS 17043 at the start of
//! the first compute frame (`ilightin::resolve_ps17043`).
//!
//! One run (eids 8 … 12482 of frame 127447):
//!
//! | draws | VS / PS | what |
//! |---|---|---|
//! | clear 17001 | | |
//! | 4096 × DrawIndexed 24 | 8400 / 8401 | the zone tiles, one draw each: `GbxVTexCoordToRasterLM` maps the tile mesh's TEXCOORD0 into the tile's chart (+ the run's jitter), `GbxVisualToWorld` = 0 (!), the PS is the terrain material (`TMapBaseColor` array slice `iPxz`, triplanar blend) — with the zero world matrix its normal is 0 and its world position 0, so the blend weights saturate to 0 and the colour is the Pxz sample at the constant uv of `g_WorldPosToTcPyPxz`'s translation |
//! | 2376 + 159 + 3216 | 17021 / 17022, 17023, 17022 | the wall item's three materials: TEXCOORD1 → chart, TEXCOORD0 → `TMapBaseColor` (BC3 128², BC1 64², BC3 1024²); 17022 discards where the sampled alpha < `GbxP_RasterFaceSign_Alpha01Ref.y` = 0.50196 |
//! | 12 | 17024 / 17025 | the 12-index item: a triplanar `PyBaseColor`/`PxzBaseColor` material (`TMapACosSmooth` LUTs), world matrix 0 again |
//! | 24 | 8400 / 8401 | the pad (slices 0) |
//! | clear 17004; DrawIndexedInstanced 24 × 4096 | 17011 / 17012 | the water-id map: every tile instance's `(type + 1, plane id)` into R8G8_UINT over the world XZ (`WorldToHPos`) |
//! | 4 × DrawIndexedInstanced | 17017 / 17018 | the water tint: per lightmapped instance, the texels whose world point lies under a water plane get `dst × transmittance + fog` (blend One / Src1Color, `ScaleOut` 1/9) |
//! | Draw 3 | 522 / 1109 | 17001 × `ScaleSrc` added into 16963 (blend One/One) |
//!
//! Every pixel shader writes `× GbxP_LmComputeScaleNoAcc` = 1/9 with alpha 1, so the alpha of the sum counts
//! the covering runs (and overlapping triangles) in ninths. The raster follows D3D11 (pixel centres, the
//! top-left rule, vertices snapped to 1/256 pixel with ties to even, `raster_tri`); the blend is the capture's
//! f16 rule (source truncated, sum rounded to nearest).
//!
//! What the capture says (`lmtool prepass-check passcap/pwc-day --all-runs`, 2026-09-25):
//!
//! * THE RASTER IS EXACT. Each of the three captured runs covers the same texel set as the capture with the
//!   same alpha on every texel (2 953 164 / 3 038 104 / 2 950 890 texels, 0 on one side only); the water-id
//!   map is 3 × 8 388 608 / 8 388 608 values bit-identical; the six uncaptured runs rebuilt from the instance
//!   STs (`raster_lm_for`: the CPU's f32 expression, 12 288 / 12 288 captured tile matrices bit-identical)
//!   give a nine-run sum whose alpha is bit-identical on 4 194 304 / 4 194 304 texels after runs 6, 7, 8 —
//!   once the 1/256 snapping rounds ties to EVEN (one trunk vertex of run 3 sits at exactly 258810.5 units).
//! * The two leaf draws (PS 17022, alpha-tested) leave nothing: their blend state has AlphaToCoverage ON
//!   (env/frame127447/state.json) and the PS alpha is 1/9 on a 1-sample target → coverage 0 for every fragment
//!   (`RunOpts::atc_threshold`). The same state kills them in the shadow-caster pass.
//! * The *_TYPELESS textures are sampled through _UNORM_SRGB views (5354, 14585, 14579, 14609, 14627, 15075,
//!   15078 decoded to linear before filtering); 5363 / 5367 / 5468 / 5457 / 5459 are linear. The samplers are
//!   anisotropic 16×, wrap (ClampEdge for the LUTs and the water 1D tables), no bias.
//! * Colours: with the 8-bit BC1 palette expansion and rounded thirds (`Bc1Decode::Expand8Round`) the tiles /
//!   wall / pad land within two f16 quanta (mean ours/captured 0.9994 / 0.9988 / 0.9993, one channel exact) —
//!   the rest is the texture unit's sRGB decode table (ROW 4's measurement: ≤ 0.3 % off the IEC curve) and
//!   the bilinear weights; the trunk (2 740 texels, a real anisotropic footprint) within 0.5 % mean / 5 % max —
//!   the reference tap placement of `texsample::sample`, not the vendor's. The nine-run sum resolved through
//!   PS 17043 into the sRGB UNORM8 MDiffuse 16969 of frame 127448: 16 774 305 / 16 777 216 bytes bit-identical,
//!   2 798 one step off, 113 (trunk) beyond.
//! * The per-run PS 1109 accumulation into 16963 (source truncated, sum RTNE): 2 × 16 777 216 / 16 777 216 on
//!   the banked snapshots; the water tint changes exactly the captured texel set (the seabed tiles, u = 2·3/3.5
//!   clamped → the deep end of the fog / transmittance tables).

use crate::gpufmt::{quantise_f16, Rounding};
use crate::passdiff::Buf;
use crate::texsample::{self, Sampler, Texture};

/// The atlas size of the pre-pass targets.
pub const W: u32 = 2048;
pub const H: u32 = 2048;

/// A vertex stream's layout: byte offsets inside a `stride`-byte vertex.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub stride: usize,
    pub pos: usize,
    /// NORMAL as R16G16B16A16_SNORM.
    pub nrm: Option<usize>,
    /// TEXCOORD0 as R32G32_FLOAT.
    pub uv0: Option<usize>,
    /// TEXCOORD1 as R32G32_FLOAT.
    pub uv1: Option<usize>,
}

impl Layout {
    /// POSITION, NORMAL, TEXCOORD0, TEXCOORD1, TANGENT, BINORMAL (stride 52): the wall's three meshes.
    pub const ITEM52: Layout = Layout { stride: 52, pos: 0, nrm: Some(12), uv0: Some(20), uv1: Some(28) };
    /// POSITION, NORMAL, COLOR, TEXCOORD0, TEXCOORD1, TANGENT, BINORMAL (stride 56): the 12-index item.
    pub const ITEM56: Layout = Layout { stride: 56, pos: 0, nrm: Some(12), uv0: Some(24), uv1: Some(32) };
    /// POSITION, BLENDINDICES, NORMAL, TEXCOORD0 (stride 32): the pad.
    pub const PAD32: Layout = Layout { stride: 32, pos: 0, nrm: Some(16), uv0: Some(24), uv1: None };
    /// POSITION, NORMAL, TEXCOORD0 (stride 28): the zone tile.
    pub const TILE28: Layout = Layout { stride: 28, pos: 0, nrm: Some(12), uv0: Some(20), uv1: None };
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    pub uv1: Vec<[f32; 2]>,
    pub idx: Vec<u32>,
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn snorm16_at(b: &[u8], o: usize) -> f32 {
    let v = i16::from_le_bytes([b[o], b[o + 1]]) as f32;
    (v / 32767.0).max(-1.0)
}

/// Load a mesh from a raw vertex buffer and a u16 index buffer (RenderDoc's `vb_<id>.bin` and
/// `e<eid>_vsout_indices.bin`).
pub fn load_mesh(vb: &std::path::Path, ib: &std::path::Path, l: Layout) -> Result<Mesh, String> {
    let vbb = read_maybe_gz(vb)?;
    let ibb = read_maybe_gz(ib)?;
    let n = vbb.len() / l.stride;
    let mut m = Mesh::default();
    for i in 0..n {
        let v = &vbb[i * l.stride..(i + 1) * l.stride];
        m.pos.push([f32_at(v, l.pos), f32_at(v, l.pos + 4), f32_at(v, l.pos + 8)]);
        m.nrm.push(match l.nrm { Some(o) => [snorm16_at(v, o), snorm16_at(v, o + 2), snorm16_at(v, o + 4)], None => [0.0; 3] });
        m.uv0.push(match l.uv0 { Some(o) => [f32_at(v, o), f32_at(v, o + 4)], None => [0.0; 2] });
        m.uv1.push(match l.uv1 { Some(o) => [f32_at(v, o), f32_at(v, o + 4)], None => [0.0; 2] });
    }
    m.idx = ibb.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as u32).collect();
    if m.idx.iter().any(|&i| i as usize >= n) {
        return Err(format!("{}: an index exceeds the {n} vertices", ib.display()));
    }
    Ok(m)
}

pub fn read_maybe_gz(p: &std::path::Path) -> Result<Vec<u8>, String> {
    let b = match std::fs::read(p) {
        Ok(b) => b,
        Err(e) => {
            let pg = std::path::PathBuf::from(format!("{}.gz", p.display()));
            std::fs::read(&pg).map_err(|_| format!("{}: {e}", p.display()))?
        }
    };
    if b.len() > 2 && b[0] == 0x1f && b[1] == 0x8b {
        return crate::passdiff::gunzip(&b);
    }
    Ok(b)
}

/// What a pre-pass draw needs from the log: the shader ids, the LM raster matrix (DXBC registers 0 and 1 of
/// `GbxVTexCoordToRasterLM`), the material slot indices, the alpha reference and the output scale.
#[derive(Clone, Debug)]
pub struct DrawRec {
    pub eid: u64,
    pub idx: u32,
    pub inst: u32,
    pub vs: String,
    pub ps: String,
    pub rlm: [[f32; 4]; 2],
    pub i_py: u32,
    pub i_pxz: u32,
    pub i_pyx2: u32,
    pub i_pyh2: u32,
    pub lm_scale: f32,
    pub alpha_ref: f32,
    /// The pixel-shader SRVs (slot, texture id).
    pub psrv: Vec<(u32, String)>,
    pub cleared: bool,
    /// The blend state's AlphaToCoverageEnable (env/frame<N>/state.json; the vegetation material's leaf draws).
    pub alpha_to_coverage: bool,
    /// The VS `ShaderV` and PS `ShaderP` cbuffers as the log prints them (the instanced passes read theirs here).
    pub shader_v: serde_json::Value,
    pub shader_p: serde_json::Value,
}

/// Read the frame's draw log (gzipped JSON) into the pre-pass draw records, in event order.
pub fn read_draws(path: &std::path::Path) -> Result<Vec<DrawRec>, String> {
    let bytes = read_maybe_gz(path)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let arr = v.as_array().ok_or("draw log is not an array")?;
    let mut out = Vec::new();
    for d in arr {
        let name = d["name"].as_str().unwrap_or("");
        let eid = d["eid"].as_u64().unwrap_or(0);
        if name.contains("Clear") {
            let target = d["outputs"][0]["id"].as_str().unwrap_or("").to_string();
            out.push(DrawRec { eid, idx: 0, inst: 0, vs: String::new(), ps: format!("clear:{target}"), rlm: [[0.0; 4]; 2], i_py: 0, i_pxz: 0, i_pyx2: 0, i_pyh2: 0, lm_scale: 0.0, alpha_ref: 0.0, psrv: vec![], cleared: true, alpha_to_coverage: false, shader_v: serde_json::Value::Null, shader_p: serde_json::Value::Null });
            continue;
        }
        if !name.contains("Draw") {
            continue;
        }
        let vs = d["Vertex"]["shader"].as_str().unwrap_or("").to_string();
        let ps = d["Pixel"]["shader"].as_str().unwrap_or("").to_string();
        let drawv = &d["Vertex"]["cbuffers"]["DrawV"];
        let mut rlm = [[0.0f32; 4]; 2];
        if let Some(rows) = drawv["GbxVTexCoordToRasterLM"].as_array() {
            // printed as HLSL rows of a column_major float4x2: register k = column k
            for (r, row) in rows.iter().enumerate().take(4) {
                if let Some(c) = row.as_array() {
                    for k in 0..2 {
                        rlm[k][r] = c.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                    }
                }
            }
        }
        let shp = &d["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP_Shader"];
        let gi = |k: &str| shp[k].as_u64().unwrap_or(0) as u32;
        let scenep = &d["Pixel"]["cbuffers"]["SceneP"];
        let lm_scale = scenep["GbxP_LmComputeScaleNoAcc"][0].as_f64().unwrap_or(0.0) as f32;
        let alpha_ref = scenep["GbxP_RasterFaceSign_Alpha01Ref"][1].as_f64().unwrap_or(0.0) as f32;
        let mut psrv = Vec::new();
        if let Some(s) = d["Pixel"]["srvs"].as_array() {
            for e in s {
                let id = e["tex"]["id"].as_str().or_else(|| e["buf"]["id"].as_str()).or_else(|| e["id"].as_str()).unwrap_or("").to_string();
                psrv.push((e["slot"].as_u64().unwrap_or(0) as u32, id));
            }
        }
        out.push(DrawRec { eid, idx: d["idx"].as_u64().unwrap_or(0) as u32, inst: d["inst"].as_u64().unwrap_or(0) as u32, vs, ps, rlm, i_py: gi("iPy"), i_pxz: gi("iPxz"), i_pyx2: gi("iPyX2"), i_pyh2: gi("iPyH2"), lm_scale, alpha_ref, psrv, cleared: false, alpha_to_coverage: false, shader_v: d["Vertex"]["cbuffers"]["ShaderV"].clone(), shader_p: d["Pixel"]["cbuffers"]["ShaderP"].clone() });
    }
    Ok(out)
}

/// The runs of a pre-pass frame: each begins with the clear of 17001 and holds the draws up to the next
/// clear of it (the item / tile draws, then the ids and water draws).
pub fn split_runs(draws: &[DrawRec]) -> Vec<Vec<DrawRec>> {
    let mut runs: Vec<Vec<DrawRec>> = Vec::new();
    // the attribute atlas = the target of the first clear (17001); its clear starts a run, the id map's clear
    // (17004) stays inside the run
    let atlas = draws.iter().find(|d| d.cleared).map(|d| d.ps.clone()).unwrap_or_default();
    for d in draws {
        if d.cleared {
            if d.ps == atlas {
                runs.push(Vec::new());
            }
            continue;
        }
        if let Some(r) = runs.last_mut() {
            r.push(d.clone());
        }
    }
    runs.retain(|r| r.iter().any(|d| d.ps == "8401"));
    runs
}

/// D3D `dp4` of (u, v, 0, 1) with a register — the products summed left to right.
#[inline]
pub fn dp4_uv(uv: [f32; 2], r: [f32; 4]) -> f32 {
    ((uv[0] * r[0] + uv[1] * r[1]) + 0.0 * r[2]) + 1.0 * r[3]
}

/// VS 8400 / 17021 / 17024: `o0.xy = dp4(uv, GbxVTexCoordToRasterLM[0..1])`, z = 0, w = 1 → NDC.
#[inline]
pub fn lm_ndc(uv: [f32; 2], rlm: &[[f32; 4]; 2]) -> [f32; 2] {
    [dp4_uv(uv, rlm[0]), dp4_uv(uv, rlm[1])]
}

/// The viewport transform of a (0, 0, W, H) viewport: x = (ndc.x + 1)·W/2, y = (1 − ndc.y)·H/2.
#[inline]
pub fn viewport(ndc: [f32; 2], w: u32, h: u32) -> [f32; 2] {
    [(ndc[0] + 1.0) * (w as f32 * 0.5), (1.0 - ndc[1]) * (h as f32 * 0.5)]
}

/// The sub-pixel grid of the D3D11 rasteriser: 1/256 pixel.
pub const SUBPIX: f32 = 256.0;

/// Rasterise one triangle (pixel coordinates, y down) with D3D11 rules: vertices snapped to the 1/256
/// grid, exact integer edge functions, the top-left fill rule, pixel centres at + 0.5. `f(x, y, bary)`
/// receives the barycentrics (of the vertices as given) evaluated from the snapped positions.
pub fn raster_tri<F: FnMut(u32, u32, [f32; 3])>(p: [[f32; 2]; 3], w: u32, h: u32, f: F) {
    raster_tri_rows(p, w, h, 0, h as i64, f)
}

/// `raster_tri` visiting only the pixel rows in [y_lo, y_hi) — the same pixels and barycentrics on those rows (the
/// band-parallel pre-pass: every band walks every triangle, each pixel decided by one band). Perf 8.
pub fn raster_tri_rows<F: FnMut(u32, u32, [f32; 3])>(p: [[f32; 2]; 3], w: u32, h: u32, y_lo: i64, y_hi: i64, mut f: F) {
    // the conversion to the 1/256 grid rounds ties to even: run 3's trunk triangle 29 has a vertex at exactly
    // 1010.978515625 px = 258810.5 units, and the capture covers texel (1012, 266) only with the even choice
    let snap = |v: f32| -> i64 { (v * SUBPIX).round_ties_even() as i64 };
    let q: [[i64; 2]; 3] = [[snap(p[0][0]), snap(p[0][1])], [snap(p[1][0]), snap(p[1][1])], [snap(p[2][0]), snap(p[2][1])]];
    let edge = |a: [i64; 2], b: [i64; 2], c: [i64; 2]| -> i64 { (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) };
    let area = edge(q[0], q[1], q[2]);
    if area == 0 {
        return;
    }
    // orient so that the inside is positive; remember the swap for the barycentric order
    let (a, b, c, swapped) = if area > 0 { (q[0], q[1], q[2], false) } else { (q[0], q[2], q[1], true) };
    let area = area.abs();
    let minx = q.iter().map(|v| v[0]).min().unwrap();
    let maxx = q.iter().map(|v| v[0]).max().unwrap();
    let miny = q.iter().map(|v| v[1]).min().unwrap();
    let maxy = q.iter().map(|v| v[1]).max().unwrap();
    let half = (SUBPIX as i64) / 2;
    // pixel centres c = 256·x + 128 inside [minx, maxx]
    let x0 = ((minx - half).div_euclid(256)).max(0);
    let x1 = ((maxx - half).div_euclid(256)).min(w as i64 - 1);
    let y0 = ((miny - half).div_euclid(256)).max(0).max(y_lo);
    let y1 = ((maxy - half).div_euclid(256)).min(h as i64 - 1).min(y_hi - 1);
    if x0 > x1 || y0 > y1 {
        return;
    }
    // top-left: with y down and the inside positive, a top edge is horizontal running right (dy = 0, dx > 0),
    // a left edge runs up the screen (dy < 0)
    let top_left = |a: [i64; 2], b: [i64; 2]| -> bool { (b[1] - a[1]) < 0 || ((b[1] - a[1]) == 0 && (b[0] - a[0]) > 0) };
    let tl = [top_left(a, b), top_left(b, c), top_left(c, a)];
    let inv = 1.0 / area as f64;
    for y in y0..=y1 {
        let py = y * 256 + half;
        for x in x0..=x1 {
            let px = [x * 256 + half, py];
            let e0 = edge(a, b, px);
            let e1 = edge(b, c, px);
            let e2 = edge(c, a, px);
            let inside = (e0 > 0 || (e0 == 0 && tl[0])) && (e1 > 0 || (e1 == 0 && tl[1])) && (e2 > 0 || (e2 == 0 && tl[2]));
            if !inside {
                continue;
            }
            let (wa, wb, wc) = ((e1 as f64 * inv) as f32, (e2 as f64 * inv) as f32, (e0 as f64 * inv) as f32);
            let bary = if swapped { [wa, wc, wb] } else { [wa, wb, wc] };
            f(x as u32, y as u32, bary);
        }
    }
}

/// The plane of an attribute over a triangle in pixel space: value = a + b·(x − x0) + c·(y − y0), with
/// (d/dx, d/dy) the screen-space derivatives every pixel of the triangle shares (an affine raster).
pub fn attr_gradient(p: [[f32; 2]; 3], v: [f32; 3]) -> (f32, f32) {
    let (x0, y0) = (p[0][0] as f64, p[0][1] as f64);
    let (x1, y1) = (p[1][0] as f64 - x0, p[1][1] as f64 - y0);
    let (x2, y2) = (p[2][0] as f64 - x0, p[2][1] as f64 - y0);
    let det = x1 * y2 - x2 * y1;
    if det == 0.0 {
        return (0.0, 0.0);
    }
    let (v1, v2) = ((v[1] - v[0]) as f64, (v[2] - v[0]) as f64);
    (((v1 * y2 - v2 * y1) / det) as f32, ((x1 * v2 - x2 * v1) / det) as f32)
}

/// The RGBA16F accumulation target with the capture's One/One blend rule: the source is truncated to f16,
/// the sum rounded to nearest even.
pub struct Target {
    pub buf: Buf,
    /// Per texel, the number of fragments blended in (the coverage count) — for the diagnostics.
    pub frags: Vec<u32>,
    /// Per texel, a class id of the last fragment (tiles 1, wall 2, item 3, pad 4).
    pub class: Vec<u8>,
}

impl Target {
    pub fn new() -> Target {
        Target { buf: Buf::new(W, H, 4), frags: vec![0; (W * H) as usize], class: vec![0; (W * H) as usize] }
    }
    #[inline]
    pub fn blend(&mut self, x: u32, y: u32, src: [f32; 4], class: u8) {
        for k in 0..4 {
            let s = quantise_f16(src[k], Rounding::Truncate);
            let d = self.buf.get(x, y, k as u32);
            self.buf.set(x, y, k as u32, quantise_f16(d + s, Rounding::NearestEven));
        }
        let i = (y * W + x) as usize;
        self.frags[i] += 1;
        self.class[i] = class;
    }
}

/// The textures a run's pixel shaders sample, by RenderDoc id.
pub struct Textures {
    pub by_id: std::collections::HashMap<String, Texture>,
}

impl Textures {
    pub fn get(&self, id: &str) -> Option<&Texture> {
        self.by_id.get(id)
    }
}

/// PS 17022 / 17023 (`TMapBaseColor` at TEXCOORD0, `SMapBaseColor`): `sample → r0`; 17022 discards where
/// `r0.w − Alpha01Ref.y < 0`; `r0.w = 1`; `o0 = r0 × LmComputeScaleNoAcc`. Returns None when discarded.
pub fn ps_basecolor(tex: &Texture, s: &Sampler, uv: [f32; 2], ddx: [f32; 2], ddy: [f32; 2], alpha_test: Option<f32>, lm_scale: f32) -> Option<[f32; 4]> {
    let c = texsample::sample(tex, 0, s, uv, ddx, ddy);
    if let Some(a_ref) = alpha_test {
        if c[3] - a_ref < 0.0 {
            return None;
        }
    }
    Some([c[0] * lm_scale, c[1] * lm_scale, c[2] * lm_scale, 1.0 * lm_scale])
}

/// The object classes of a run's draws (by shader / index count).
pub fn class_of(d: &DrawRec) -> u8 {
    match (d.ps.as_str(), d.idx) {
        ("8401", 24) if d.i_pxz == 0 && d.i_py == 0 => 4,
        ("8401", _) => 1,
        ("17022", 2376) => 5,
        ("17023", _) => 6,
        ("17022", _) => 7,
        ("17025", _) => 3,
        _ => 0,
    }
}

/// The meshes of the pre-pass draws, keyed by (PS, index count) — the same vertex buffers the frame 127448
/// shadow pass draws (env/frame127448/mesh).
pub struct Meshes {
    pub tile: Mesh,
    pub pad: Mesh,
    pub wall_2376: Mesh,
    pub wall_159: Mesh,
    pub wall_3216: Mesh,
    pub item_12: Mesh,
}

impl Meshes {
    pub fn load(env: &std::path::Path) -> Result<Meshes, String> {
        let m = env.join("mesh");
        Ok(Meshes {
            tile: load_mesh(&m.join("vb_5350.bin"), &m.join("e000365_vsout_indices.bin"), Layout::TILE28)?,
            pad: load_mesh(&m.join("vb_14616.bin"), &m.join("e000353_vsout_indices.bin"), Layout::PAD32)?,
            wall_2376: load_mesh(&m.join("vb_14589.bin"), &m.join("e000410_vsout_indices.bin"), Layout::ITEM52)?,
            wall_159: load_mesh(&m.join("vb_14577.bin"), &m.join("e000347_vsout_indices.bin"), Layout::ITEM52)?,
            wall_3216: load_mesh(&m.join("vb_14583.bin"), &m.join("e000394_vsout_indices.bin"), Layout::ITEM52)?,
            item_12: load_mesh(&m.join("vb_14623.bin"), &m.join("e000359_vsout_indices.bin"), Layout::ITEM56)?,
        })
    }
    pub fn for_draw(&self, d: &DrawRec) -> Option<&Mesh> {
        match (d.ps.as_str(), d.idx) {
            ("8401", 24) if d.i_pxz == 0 && d.i_py == 0 => Some(&self.pad),
            ("8401", 24) => Some(&self.tile),
            ("17022", 2376) => Some(&self.wall_2376),
            ("17023", 159) => Some(&self.wall_159),
            ("17022", 3216) => Some(&self.wall_3216),
            ("17025", 12) => Some(&self.item_12),
            _ => None,
        }
    }
}

/// Options of the run emulation that the capture decides.
pub struct RunOpts<'a> {
    pub sampler: Sampler,
    /// Rasterise only (alpha and coverage): the colours are 0.
    pub coverage_only: bool,
    /// The colour PS 8401 produces for a tile / the wall (a function of the draw's slot indices — constant over
    /// the draw in the pre-pass, whose world matrix is 0); None → 0.
    pub tile_rgb: Option<Box<dyn Fn(&DrawRec) -> [f32; 3] + 'a>>,
    /// The colour PS 17025 produces for the pad (constant over the draw for the same reason); None → 0.
    pub pad_rgb: Option<Box<dyn Fn(&DrawRec) -> [f32; 3] + 'a>>,
    /// Alpha-to-coverage on a 1-sample target: a fragment whose output alpha is below 0.5 gets no coverage
    /// (the leaf draws, `DrawRec::alpha_to_coverage`).
    pub atc_threshold: f32,
}

/// Rasterise one run's item and tile draws into a fresh target (before the water tint).
pub fn run_attr_draws(run: &[DrawRec], meshes: &Meshes, tex: &Textures, o: &RunOpts<'_>, tgt: &mut Target) -> Result<(), String> {
    for d in run {
        let Some(mesh) = meshes.for_draw(d) else { continue };
        let class = class_of(d);
        let uses_uv1 = d.ps == "17022" || d.ps == "17023" || d.ps == "17025";
        let basecolor = if d.ps == "17022" || d.ps == "17023" { d.psrv.iter().find(|(s, _)| *s == 0).and_then(|(_, id)| tex.get(id)) } else { None };
        if (d.ps == "17022" || d.ps == "17023") && basecolor.is_none() && !o.coverage_only {
            // without the texture the draw is rasterised for coverage only (an alpha-tested one over-covers)
            eprintln!("eid {}: texture {:?} not loaded — coverage only", d.eid, d.psrv);
        }
        let alpha_test = if d.ps == "17022" { Some(d.alpha_ref) } else { None };
        let n_tri = (d.idx as usize / 3).min(mesh.idx.len() / 3);
        for t in 0..n_tri {
            let i = [mesh.idx[3 * t] as usize, mesh.idx[3 * t + 1] as usize, mesh.idx[3 * t + 2] as usize];
            let lmuv = |k: usize| if uses_uv1 { mesh.uv1[i[k]] } else { mesh.uv0[i[k]] };
            let p = [viewport(lm_ndc(lmuv(0), &d.rlm), W, H), viewport(lm_ndc(lmuv(1), &d.rlm), W, H), viewport(lm_ndc(lmuv(2), &d.rlm), W, H)];
            let uv0 = [mesh.uv0[i[0]], mesh.uv0[i[1]], mesh.uv0[i[2]]];
            let (dudx, dudy) = attr_gradient(p, [uv0[0][0], uv0[1][0], uv0[2][0]]);
            let (dvdx, dvdy) = attr_gradient(p, [uv0[0][1], uv0[1][1], uv0[2][1]]);
            let tile_rgb = match d.ps.as_str() {
                "8401" => o.tile_rgb.as_ref().map(|f| f(d)).unwrap_or([0.0; 3]),
                "17025" => o.pad_rgb.as_ref().map(|f| f(d)).unwrap_or([0.0; 3]),
                _ => [0.0; 3],
            };
            raster_tri(p, W, H, |x, y, b| {
                let src = match d.ps.as_str() {
                    "17022" | "17023" => {
                        if o.coverage_only || basecolor.is_none() {
                            // the alpha test cannot run without the texture: count the fragment
                            Some([0.0, 0.0, 0.0, d.lm_scale])
                        } else {
                            let uv = [b[0] * uv0[0][0] + b[1] * uv0[1][0] + b[2] * uv0[2][0], b[0] * uv0[0][1] + b[1] * uv0[1][1] + b[2] * uv0[2][1]];
                            ps_basecolor(basecolor.unwrap(), &o.sampler, uv, [dudx, dvdx], [dudy, dvdy], alpha_test, d.lm_scale)
                        }
                    }
                    "8401" | "17025" => Some([tile_rgb[0] * d.lm_scale, tile_rgb[1] * d.lm_scale, tile_rgb[2] * d.lm_scale, 1.0 * d.lm_scale]),
                    _ => Some([0.0, 0.0, 0.0, d.lm_scale]),
                };
                if let Some(s) = src {
                    // alpha-to-coverage on the 1-sample target: the output alpha decides the coverage
                    if d.alpha_to_coverage && s[3] < o.atc_threshold {
                        return;
                    }
                    tgt.blend(x, y, s, class);
                }
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lm_raster_matrix_maps_uv_to_the_chart() {
        // the first tile draw of frame 127447 (eid 32): registers = the columns of the printed float4x2
        let rlm = [[0.021452128887176514, 0.0, 0.0, 0.599461555480957], [-0.0, -0.02127263695001602, -0.0, 0.336395800113678]];
        let p0 = viewport(lm_ndc([0.0, 0.0], &rlm), W, H);
        let p1 = viewport(lm_ndc([1.0, 1.0], &rlm), W, H);
        assert!((p0[0] - 1637.85).abs() < 0.01 && (p0[1] - 679.53).abs() < 0.01, "{p0:?}");
        assert!((p1[0] - p0[0] - 21.967).abs() < 0.01 && (p1[1] - p0[1] - 21.783).abs() < 0.01, "{p1:?}");
    }

    #[test]
    fn raster_follows_the_top_left_rule_and_snaps() {
        // two triangles sharing the diagonal of the pixel square [0,2]²: every pixel centre covered exactly once
        let mut count = vec![0u32; 4];
        raster_tri([[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]], 2, 2, |x, y, _| count[(y * 2 + x) as usize] += 1);
        raster_tri([[2.0, 0.0], [2.0, 2.0], [0.0, 2.0]], 2, 2, |x, y, _| count[(y * 2 + x) as usize] += 1);
        assert_eq!(count, vec![1, 1, 1, 1]);
        // a pixel centre exactly on a shared vertical edge belongs to the triangle on its right (left edge rule)
        let mut hits = Vec::new();
        raster_tri([[0.5, 0.0], [0.5, 4.0], [4.0, 2.0]], 4, 4, |x, y, _| hits.push((x, y)));
        assert!(hits.contains(&(0, 1)) && hits.contains(&(0, 2)));
        let mut hits2 = Vec::new();
        raster_tri([[0.5, 0.0], [-3.0, 2.0], [0.5, 4.0]], 4, 4, |x, y, _| hits2.push((x, y)));
        assert!(!hits2.contains(&(0, 1)) && !hits2.contains(&(0, 2)));
        // barycentrics sum to 1 and interpolate a plane
        raster_tri([[0.0, 0.0], [8.0, 0.0], [0.0, 8.0]], 8, 8, |x, y, b| {
            assert!((b[0] + b[1] + b[2] - 1.0).abs() < 1e-5);
            let px = b[1] * 8.0;
            assert!((px - (x as f32 + 0.5)).abs() < 1e-3, "x {x} → {px}");
            let py = b[2] * 8.0;
            assert!((py - (y as f32 + 0.5)).abs() < 1e-3);
        });
    }

    #[test]
    fn blend_follows_the_f16_rule() {
        let mut t = Target::new();
        for _ in 0..9 {
            t.blend(5, 5, [0.0, 0.0, 0.0, 1.0 / 9.0], 1);
        }
        // nine 1/9 (truncated to f16 0.11108398) summed with RTNE: the capture's fully covered alpha 1.0010
        assert_eq!(t.buf.get(5, 5, 3), 1.0009766);
        assert_eq!(t.frags[(5 * W + 5) as usize], 9);
    }
}

/// Diagnostics of one alpha-tested draw: per triangle the LOD its footprint selects and how many of its
/// fragments the alpha test keeps (the leaf cards of the vegetation item).
pub fn alpha_test_report(d: &DrawRec, mesh: &Mesh, tex: &Texture, s: &Sampler) -> String {
    let n_tri = (d.idx as usize / 3).min(mesh.idx.len() / 3);
    let mut lods: Vec<f32> = Vec::new();
    let (mut kept, mut total) = (0usize, 0usize);
    let mut alpha_hist = [0usize; 10];
    let mut tri_px = 0usize;
    for t in 0..n_tri {
        let i = [mesh.idx[3 * t] as usize, mesh.idx[3 * t + 1] as usize, mesh.idx[3 * t + 2] as usize];
        let p = [viewport(lm_ndc(mesh.uv1[i[0]], &d.rlm), W, H), viewport(lm_ndc(mesh.uv1[i[1]], &d.rlm), W, H), viewport(lm_ndc(mesh.uv1[i[2]], &d.rlm), W, H)];
        let uv0 = [mesh.uv0[i[0]], mesh.uv0[i[1]], mesh.uv0[i[2]]];
        let (dudx, dudy) = attr_gradient(p, [uv0[0][0], uv0[1][0], uv0[2][0]]);
        let (dvdx, dvdy) = attr_gradient(p, [uv0[0][1], uv0[1][1], uv0[2][1]]);
        let (lod, _, _) = texsample::lod_and_ratio([dudx * tex.w as f32, dvdx * tex.h as f32], [dudy * tex.w as f32, dvdy * tex.h as f32], s.max_aniso);
        let mut n = 0;
        raster_tri(p, W, H, |_, _, b| {
            n += 1;
            let uv = [b[0] * uv0[0][0] + b[1] * uv0[1][0] + b[2] * uv0[2][0], b[0] * uv0[0][1] + b[1] * uv0[1][1] + b[2] * uv0[2][1]];
            let c = texsample::sample(tex, 0, s, uv, [dudx, dvdx], [dudy, dvdy]);
            alpha_hist[((c[3] * 10.0) as usize).min(9)] += 1;
            total += 1;
            if c[3] - d.alpha_ref >= 0.0 {
                kept += 1;
            }
        });
        if n > 0 {
            lods.push(lod);
            tri_px += n;
        }
    }
    lods.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = lods.get(lods.len() / 2).copied().unwrap_or(0.0);
    format!("eid {} PS {}: {} triangles, {} with pixels ({} fragments), LOD min {:.2} median {:.2} max {:.2}; alpha test keeps {kept} of {total}; sampled alpha deciles {:?}", d.eid, d.ps, n_tri, lods.len(), tri_px, lods.first().copied().unwrap_or(0.0), med, lods.last().copied().unwrap_or(0.0), alpha_hist)
}

/// The uv0 / LM-space extents of a draw's triangles: min/max uv0, the pixel sizes of the triangles.
pub fn uv_report(d: &DrawRec, mesh: &Mesh) -> String {
    let n_tri = (d.idx as usize / 3).min(mesh.idx.len() / 3);
    let (mut umin, mut umax, mut vmin, mut vmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    let mut spans: Vec<(f32, f32)> = Vec::new(); // (uv0 span in texture units, pixel span)
    for t in 0..n_tri {
        let i = [mesh.idx[3 * t] as usize, mesh.idx[3 * t + 1] as usize, mesh.idx[3 * t + 2] as usize];
        let p = [viewport(lm_ndc(mesh.uv1[i[0]], &d.rlm), W, H), viewport(lm_ndc(mesh.uv1[i[1]], &d.rlm), W, H), viewport(lm_ndc(mesh.uv1[i[2]], &d.rlm), W, H)];
        let (mut u0, mut u1, mut v0, mut v1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for k in 0..3 {
            let uv = mesh.uv0[i[k]];
            umin = umin.min(uv[0]); umax = umax.max(uv[0]); vmin = vmin.min(uv[1]); vmax = vmax.max(uv[1]);
            u0 = u0.min(uv[0]); u1 = u1.max(uv[0]); v0 = v0.min(uv[1]); v1 = v1.max(uv[1]);
            x0 = x0.min(p[k][0]); x1 = x1.max(p[k][0]); y0 = y0.min(p[k][1]); y1 = y1.max(p[k][1]);
        }
        spans.push(((u1 - u0).max(v1 - v0), (x1 - x0).max(y1 - y0)));
    }
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let med = spans[spans.len() / 2];
    let first: Vec<String> = (0..6.min(mesh.pos.len())).map(|k| format!("uv0 ({:.4}, {:.4}) uv1 ({:.5}, {:.5}) pos ({:.2}, {:.2}, {:.2})", mesh.uv0[k][0], mesh.uv0[k][1], mesh.uv1[k][0], mesh.uv1[k][1], mesh.pos[k][0], mesh.pos[k][1], mesh.pos[k][2])).collect();
    format!("eid {}: uv0 ∈ [{umin:.3}, {umax:.3}] × [{vmin:.3}, {vmax:.3}]; per triangle the max uv0 span / pixel span: min ({:.4}, {:.2}) median ({:.4}, {:.2}) max ({:.4}, {:.2}); first vertices: {}", d.eid, spans[0].0, spans[0].1, med.0, med.1, spans[spans.len() - 1].0, spans[spans.len() - 1].1, first.join(" | "))
}

// ---------------------------------------------------------------------------------------------------------
// The water tint (eids 12448–12457 of every run): VS 17017 / PS 17018 over the four LM meshes (the same instance
// stream as the direct-sun and accumulate draws, `sunpass::LmVertex` / `LmInstance`), blend One / Src1Color into
// the run's atlas before PS 1109 adds it into 16963.

/// `mad` as the captured GPU evaluates it (fused).
#[inline]
fn mad(a: f32, b: f32, c: f32) -> f32 {
    a.mul_add(b, c)
}

/// The constants of a water-tint draw (VS `g_CBufferV`, PS `g_CBufferP`).
#[derive(Clone, Debug)]
pub struct WaterDraw {
    pub eid: u64,
    pub mesh: usize,
    pub instance_first: usize,
    pub instance_count: usize,
    pub scale_ss: [f32; 2],
    pub trans_ss: [f32; 2],
    /// `World_To_i2WaterId`: DXBC registers 0 and 1 (the columns of the printed float4x2).
    pub world_to_id: [[f32; 4]; 2],
    pub world_min_xz: [f32; 2],
    pub world_max_xz: [f32; 2],
    pub scale_out: f32,
}

/// A vertex after VS 17017: clip xy, the water-id map coordinates (o1.xy), the world height (o1.z), the four
/// clip distances (o2).
#[derive(Clone, Copy, Debug)]
pub struct WaterVsOut {
    pub clip: [f32; 2],
    pub id_uv: [f32; 2],
    pub world_y: f32,
    pub clipdist: [f32; 4],
}

/// VS 17017 (Vertex_17017.txt), instruction by instruction. `table` = `g_TcLM_ST_LM01`.
pub fn vs_17017(v: &crate::sunpass::LmVertex, inst: &crate::sunpass::LmInstance, table: &[[f32; 4]], d: &WaterDraw) -> WaterVsOut {
    // 0-8: the chart ST
    let st = if v.chart_idx < 0xffff { table.get(v.chart_idx.wrapping_add(inst.st_x_bits) as usize).copied().unwrap_or([0.0; 4]) } else { inst.st };
    // 9-10
    let r0xy = [st[0] * d.scale_ss[0], st[1] * d.scale_ss[1]];
    let r0zw = [mad(d.scale_ss[0], st[2], d.trans_ss[0]), mad(d.scale_ss[1], st[3], d.trans_ss[1])];
    // 11: r1 = v0.zyyx * v4.w
    let s = inst.scale;
    let (q, t) = (inst.q, inst.t);
    let r1 = [v.pos[2] * s, v.pos[1] * s, v.pos[1] * s, v.pos[0] * s];
    // 12: r2.xyz = r1.zxw * v3.zxy
    let mut r2 = [r1[2] * q[2], r1[0] * q[0], r1[3] * q[1], 0.0];
    // 13: r2.x = v3.y * r1.x - r2.x ; 14: r2.x = v3.w * r1.w + r2.x
    r2[0] = mad(q[1], r1[0], -r2[0]);
    r2[0] = mad(q[3], r1[3], r2[0]);
    // 15: r2.y = v3.z * r1.w - r2.y ; 16: r1.y = v3.w * r1.y + r2.y
    r2[1] = mad(q[2], r1[3], -r2[1]);
    let r1y = mad(q[3], r1[1], r2[1]);
    // 17: r2.y = v3.x * r1.z - r2.z ; 18: r2.y = v3.w * r1.x + r2.y
    r2[1] = mad(q[0], r1[2], -r2[2]);
    r2[1] = mad(q[3], r1[0], r2[1]);
    // 19: r2.z = r2.y * v3.x ; 20: r2.w = r1.y * v3.z ; 21: r2.y = r2.y * v3.y - r2.w ; 22: r3.x = r2.y * 2 + r1.w
    r2[2] = r2[1] * q[0];
    r2[3] = r1y * q[2];
    r2[1] = mad(r2[1], q[1], -r2[3]);
    let r3x = mad(r2[1], 2.0, r1[3]);
    // 23: r1.w = r2.x * v3.y ; 24: r2.x = r2.x * v3.z - r2.z ; 25: r3.y = r2.x * 2 + r1.z
    let r1w = r2[0] * q[1];
    r2[0] = mad(r2[0], q[2], -r2[2]);
    let r3y = mad(r2[0], 2.0, r1[2]);
    // 26: r1.y = r1.y * v3.x - r1.w ; 27: r3.z = r1.y * 2 + r1.x
    let r1y2 = mad(r1y, q[0], -r1w);
    let r3z = mad(r1y2, 2.0, r1[0]);
    // 28: world = r3 + v4.xyz
    let world = [r3x + t[0], r3y + t[1], r3z + t[2]];
    // 29: o0.xy = r0.xy * v2.xy + r0.zw
    let clip = [mad(r0xy[0], v.uv[0], r0zw[0]), mad(r0xy[1], v.uv[1], r0zw[1])];
    // 31-32: o1.xy = dp4((world, 1), World_To_i2WaterId[k])
    let dp4 = |r: [f32; 4]| ((world[0] * r[0] + world[1] * r[1]) + world[2] * r[2]) + 1.0 * r[3];
    let id_uv = [dp4(d.world_to_id[0]), dp4(d.world_to_id[1])];
    // 33-34: the clip distances
    let clipdist = [world[0] - d.world_min_xz[0], world[2] - d.world_min_xz[1], -world[0] + d.world_max_xz[0], -world[2] + d.world_max_xz[1]];
    WaterVsOut { clip, id_uv, world_y: world[1], clipdist }
}

/// The water data PS 17018 reads: the id map (R8G8_UINT: x = type + 1, y = plane), `g_WaterTop_ByPlanes`
/// (float4 per plane, .x = the surface height), `g_WaterDepth_FogMaxDepthInv_ByIds` (float4 per type, .x =
/// the depth, .y = 1 / the fog's max depth), the fog and transmittance 1D arrays.
pub struct WaterData<'a> {
    pub ids: &'a Buf,
    pub top_by_plane: Vec<[f32; 4]>,
    pub depth_by_id: Vec<[f32; 4]>,
    pub fog: &'a Texture,
    pub transmittance: &'a Texture,
    pub sampler: Sampler,
}

/// PS 17018 for one pixel (Pixel_17018.txt): (o0, o1) or None when discarded.
pub fn ps_17018(v: &WaterVsOut, w: &WaterData, scale_out: f32) -> Option<([f32; 4], [f32; 4])> {
    // 0: ftoi (toward zero); 2: ld TMapWaterId
    let (ix, iy) = (v.id_uv[0].trunc() as i64, v.id_uv[1].trunc() as i64);
    if ix < 0 || iy < 0 || ix >= w.ids.w as i64 || iy >= w.ids.h as i64 {
        return None; // `ld` out of range reads 0 → discard_z
    }
    let id1 = w.ids.get(ix as u32, iy as u32, 0) as u32;
    let plane = w.ids.get(ix as u32, iy as u32, 1) as u32;
    // 3: discard_z r0.x
    if id1 == 0 {
        return None;
    }
    // 4: top = g_WaterTop_ByPlanes[plane].x ; 5-6: discard if top < world.y
    let top = w.top_by_plane.get(plane as usize).map(|p| p[0]).unwrap_or(0.0);
    if top < v.world_y {
        return None;
    }
    // 7-12: id = id1 − 1; (depth, inv) = ByIds[id].xy; discard if world.y < top − depth − 0.1
    let id = id1 - 1;
    let dd = w.depth_by_id.get(id as usize).copied().unwrap_or([0.0; 4]);
    let (depth, inv) = (dd[0], dd[1]);
    let floor = (-depth + top) + -0.1;
    if v.world_y < floor {
        return None;
    }
    // 13-15: r0.y = top − world.y ; r1.x = dp2(r0.yy, r0.ww) ; r1.y = float(id)
    let dy = top + -v.world_y;
    let u = dy * inv + dy * inv;
    let slice = id;
    // 16-17: the two 1D-array samples through SGbxClamp_Bilinear
    let fog = texsample::sample(w.fog, slice, &w.sampler, [u, 0.5], [0.0, 0.0], [0.0, 0.0]);
    let tr = texsample::sample(w.transmittance, slice, &w.sampler, [u, 0.5], [0.0, 0.0], [0.0, 0.0]);
    // 18-23
    let o0 = [fog[3] * fog[0] * scale_out, fog[3] * fog[1] * scale_out, fog[3] * fog[2] * scale_out, 0.0];
    let one_minus = -fog[3] + 1.0;
    let o1 = [tr[0] * one_minus, tr[1] * one_minus, tr[2] * one_minus, 1.0];
    Some((o0, o1))
}

/// The four water-tint draws of a run applied to `tgt` (the atlas after the attribute draws): blend One /
/// Src1Color = `dst × o1 + o0` per channel, the f16 rule (source truncated, sum rounded to nearest).
pub fn run_water_draws(draws: &[WaterDraw], meshes: &[crate::sunpass::LmMesh], instances: &[crate::sunpass::LmInstance], table: &[[f32; 4]], water: &WaterData, tgt: &mut Target) {
    for d in draws {
        let mesh = &meshes[d.mesh];
        for ii in 0..d.instance_count {
            let Some(inst) = instances.get(d.instance_first + ii) else { continue };
            let vs: Vec<WaterVsOut> = mesh.verts.iter().map(|v| vs_17017(v, inst, table, d)).collect();
            for t in mesh.indices.chunks_exact(3) {
                let a = [vs[t[0] as usize], vs[t[1] as usize], vs[t[2] as usize]];
                // the clip distances: a triangle entirely outside a plane is dropped; a partial one would need
                // clipping — the scene's instances sit inside the world box, so none is
                if (0..4).any(|k| a.iter().all(|v| v.clipdist[k] < 0.0)) {
                    continue;
                }
                let p = [viewport(a[0].clip, W, H), viewport(a[1].clip, W, H), viewport(a[2].clip, W, H)];
                raster_tri(p, W, H, |x, y, b| {
                    let lerp = |f: &dyn Fn(&WaterVsOut) -> f32| b[0] * f(&a[0]) + b[1] * f(&a[1]) + b[2] * f(&a[2]);
                    let v = WaterVsOut { clip: [0.0; 2], id_uv: [lerp(&|o| o.id_uv[0]), lerp(&|o| o.id_uv[1])], world_y: lerp(&|o| o.world_y), clipdist: [0.0; 4] };
                    if let Some((o0, o1)) = ps_17018(&v, water, d.scale_out) {
                        for k in 0..4u32 {
                            let dst = tgt.buf.get(x, y, k);
                            let s0 = quantise_f16(o0[k as usize], Rounding::Truncate);
                            let s1 = o1[k as usize];
                            tgt.buf.set(x, y, k, quantise_f16(s0 + dst * s1, Rounding::NearestEven));
                        }
                    }
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The materials' pixel shaders, instruction by instruction.

/// D3D `saturate`: NaN → 0.
#[inline]
fn sat(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

/// D3D `rsq`: 1/sqrt(x) (+inf at 0).
#[inline]
fn rsq(v: f32) -> f32 {
    1.0 / v.sqrt()
}

/// D3D `lt` a < b (false on NaN).
#[inline]
fn lt(a: f32, b: f32) -> bool {
    a < b
}

/// The terrain material's buffers (PS 8401's t3 / t4 / t5: `g_WorldPosToTcPyPxz`, `g_WorldPosToTcPyX2`,
/// `g_WorldPosToTcPyH2`, float4 elements) and texture arrays (t0 `TMapBaseColor`, t1 `TMapPyX2`, t2 `TMapPyH2`).
pub struct TerrainMaterial<'a> {
    pub py_pxz: Vec<[f32; 4]>,
    pub py_x2: Vec<[f32; 4]>,
    pub py_h2: Vec<[f32; 4]>,
    pub base: &'a Texture,
    pub x2: &'a Texture,
    pub h2: &'a Texture,
    pub sampler: Sampler,
}

pub fn read_float4s(p: &std::path::Path) -> Result<Vec<[f32; 4]>, String> {
    let b = read_maybe_gz(p)?;
    Ok(b.chunks_exact(16).map(|c| [f32_at(c, 0), f32_at(c, 4), f32_at(c, 8), f32_at(c, 12)]).collect())
}

/// PS 8401 (Pixel_8401.txt) for one pixel: `v1` = the world normal (VS o1), `v2` = the eye-relative position
/// (VS o2 = world − EyeInWorld), `eye` = `GbxP_EyeInWorld.xyz`, the material slot indices from `g_CBufferP_Shader`,
/// the derivatives of `v2` per pixel for the samplers' LOD (zero in the pre-pass: the world matrix is 0).
/// Returns rgb before the `× LmComputeScaleNoAcc`.
pub fn ps_8401(mat: &TerrainMaterial, v1: [f32; 3], v2: [f32; 3], eye: [f32; 3], i_py: u32, i_pxz: u32, i_pyx2: u32, i_pyh2: u32, dv2dx: [f32; 3], dv2dy: [f32; 3]) -> [f32; 3] {
    let ld = |buf: &Vec<[f32; 4]>, i: u32| -> [f32; 4] { buf.get(i as usize).copied().unwrap_or([0.0; 4]) };
    // 0-2: r0.xyz = normalize(v1)
    let l2 = (v1[0] * v1[0] + v1[1] * v1[1]) + v1[2] * v1[2];
    let inv = rsq(l2);
    let r0 = [inv * v1[0], inv * v1[1], inv * v1[2]];
    // 3: r1.xyz = v2 + eye (the world position)
    let r1 = [v2[0] + eye[0], v2[1] + eye[1], v2[2] + eye[2]];
    // 4-5: r2.xyz = PyPxz[iPy·4].xzw
    let b = ld(&mat.py_pxz, i_py << 2);
    let r2 = [b[0], b[2], b[3]];
    // 6: r3 = bfi(30, 2, (iPy, iPy, iPxz, iPxz), (1, 3, 2, 3)) = (iPy·4 | 1, iPy·4 | 3, iPxz·4 | 2, iPxz·4 | 3)
    let r3i = [(i_py << 2) | 1, (i_py << 2) | 3, (i_pxz << 2) | 2, (i_pxz << 2) | 3];
    // 7: r4.xyz = PyPxz[r3.x].xzw ; 8: r3.xy = PyPxz[r3.y].zw ; 9: r5.xyz = PyPxz[r3.z].xyz ; 10: r3.zw = PyPxz[r3.w].xy
    let b1 = ld(&mat.py_pxz, r3i[0]);
    let r4 = [b1[0], b1[2], b1[3]];
    let b3 = ld(&mat.py_pxz, r3i[1]);
    let r3xy = [b3[2], b3[3]];
    let r5 = ld(&mat.py_pxz, r3i[2]);
    let b3b = ld(&mat.py_pxz, r3i[3]);
    let r3zw = [b3b[0], b3b[1]];
    // 11-13: r0.w = r0.x · rsq(r0.x² + r0.z²)
    let d2 = r0[0] * r0[0] + r0[2] * r0[2];
    let mut r0w = rsq(d2) * r0[0];
    // 14: r2.w = r3.w − r3.z ; 15: r0.y = |r0.y| − r3.x, r0.w = |r0.w| − r3.z ; 16: r2.w = 1 / r2.w ; 17: r0.w = sat(r0.w · r2.w)
    let mut r2w = -r3zw[0] + r3zw[1];
    let r0y = r0[1].abs() + -r3xy[0];
    r0w = r0w.abs() + -r3zw[0];
    r2w = 1.0 / r2w;
    r0w = sat(r0w * r2w);
    // 18-20: the smoothstep 1 − (3 − 2t)·t²
    r2w = mad(r0w, -2.0, 3.0);
    r0w = r0w * r0w;
    r0w = mad(-r2w, r0w, 1.0);
    // 21: r3.zw = (r1.z, r1.x) · r5.x
    let r3zw2 = [r1[2] * r5[0], r1[0] * r5[0]];
    // 22-25: the signs of r0.x / r0.z pick the mirrored coordinate
    let r6x = if lt(0.0, r0[0]) { -r3zw2[0] } else { r3zw2[0] };
    let r7x = if lt(r0[2], 0.0) { -r3zw2[1] } else { r3zw2[1] };
    // 26: r6.yz = r1.y · r5.y − r5.z
    let r6yz = mad(r1[1], r5[1], -r5[2]);
    // 27-32: r0.x = smoothstep of r0.y over (r3.x, r3.y): t = sat((r3.y − r3.x)⁻¹ · r0.y); (3 − 2t)·t²
    let mut r0x = -r3xy[0] + r3xy[1];
    r0x = 1.0 / r0x;
    r0x = sat(r0x * r0y);
    let r0y2 = mad(r0x, -2.0, 3.0);
    r0x = r0x * r0x;
    r0x = r0x * r0y2;
    // 33-35: the Py texture coordinates: dp3((r1.x, r1.z, 1), r2 / r4)
    let uv_py = [(r1[0] * r2[0] + r1[2] * r2[1]) + 1.0 * r2[2], (r1[0] * r4[0] + r1[2] * r4[1]) + 1.0 * r4[2]];
    // the derivatives of the coordinates (an affine function of the world position)
    let d_py = |dv: [f32; 3]| [dv[0] * r2[0] + dv[2] * r2[1], dv[0] * r4[0] + dv[2] * r4[1]];
    let d_pxz_a = |dv: [f32; 3]| [dv[2] * r5[0], dv[1] * r5[1]];
    let d_pxz_b = |dv: [f32; 3]| [dv[0] * r5[0], dv[1] * r5[1]];
    // 36-37: the Py base colour, slice iPy
    let mut r2c = texsample::sample(mat.base, i_py, &mat.sampler, uv_py, d_py(dv2dx), d_py(dv2dy));
    // 39: r3 = base at (r6.x, r6.y), slice iPxz ; 41: r4 = base at (r7.x, r6.z), slice iPxz
    let r3c = texsample::sample(mat.base, i_pxz, &mat.sampler, [r6x, r6yz], d_pxz_a(dv2dx), d_pxz_a(dv2dy));
    let r4c = texsample::sample(mat.base, i_pxz, &mat.sampler, [r7x, r6yz], d_pxz_b(dv2dx), d_pxz_b(dv2dy));
    // 42-43: r0.yzw = r0.w · (r4 − r3) + r3
    let mut pxz = [0.0f32; 3];
    for k in 0..3 {
        pxz[k] = mad(r0w, r4c[k] + -r3c[k], r3c[k]);
    }
    // 44-56: the X2 modulation of the Py colour when iPyX2 < 255
    if i_pyx2 < 255 {
        let bx = ld(&mat.py_x2, i_pyx2 << 2);
        let r3x = [bx[0], bx[2], bx[3]];
        let by = ld(&mat.py_x2, (i_pyx2 << 2) | 1);
        let r4x = [by[0], by[2], by[3]];
        let uv = [(r1[0] * r3x[0] + r1[2] * r3x[1]) + 1.0 * r3x[2], (r1[0] * r4x[0] + r1[2] * r4x[1]) + 1.0 * r4x[2]];
        let d = |dv: [f32; 3]| [dv[0] * r3x[0] + dv[2] * r3x[1], dv[0] * r4x[0] + dv[2] * r4x[1]];
        let x2 = texsample::sample(mat.x2, i_pyx2, &mat.sampler, uv, d(dv2dx), d(dv2dy));
        for k in 0..3 {
            let two = x2[k] + x2[k];
            r2c[k] = two * r2c[k];
        }
    }
    // 57-65: the H2 modulation (a 1D lookup by height) when iPyH2 < 255
    if i_pyh2 < 255 {
        let bh = ld(&mat.py_h2, (i_pyh2 << 2) | 2);
        let (r1x, r1z) = (bh[0], bh[2]);
        let u = mad(r1[1], r1x, r1z);
        let h2 = texsample::sample(mat.h2, i_pyh2, &mat.sampler, [u, u], [dv2dx[1] * r1x, dv2dx[1] * r1x], [dv2dy[1] * r1x, dv2dy[1] * r1x]);
        for k in 0..3 {
            let two = h2[k] + h2[k];
            r2c[k] = two * r2c[k];
        }
    }
    // 66-67: the top/side blend by r0.x
    let mut out = [0.0f32; 3];
    for k in 0..3 {
        let r1k = -pxz[k] + r2c[k];
        out[k] = mad(r0x, r1k, pxz[k]);
    }
    out
}

/// The pad's material data (PS 17025: `TMapACosSmooth` t0, `TMapACosSmoothPy` t1, `TMapPyBaseColor` t2,
/// `TMapPyX2` t3, `TMapPxzBaseColor` t4; samplers s0–s3; cb `GbxSamplerTcScaleTrans_PxzBaseColor`).
pub struct PadMaterial<'a> {
    pub acos: &'a Texture,
    pub acos_py: &'a Texture,
    pub py_base: &'a Texture,
    pub py_x2: &'a Texture,
    pub pxz_base: &'a Texture,
    pub s_acos: Sampler,
    pub s_acos_py: Sampler,
    pub s_py: Sampler,
    pub s_pxz: Sampler,
    pub tc_scale_trans_pxz: [f32; 4],
}

/// PS 17025 (Pixel_17025.txt) for one pixel: `v1` world position, `v2` world normal, `v3` = (Py base uv, PyX2 uv)
/// from VS 17024; derivatives zero (the world matrix is 0 in the pre-pass). Returns rgb before the scale.
pub fn ps_17025(m: &PadMaterial, v1: [f32; 3], v2: [f32; 3], v3: [f32; 4]) -> [f32; 3] {
    let z = [0.0f32, 0.0];
    // 0-3: r0.x = |v2.x · rsq(v2.x² + v2.z²)|
    let d2 = v2[0] * v2[0] + v2[2] * v2[2];
    let r0x = (rsq(d2) * v2[0]).abs();
    // 4: r0.x = TMapACosSmooth(r0.x).x
    let r0x = texsample::sample(m.acos, 0, &m.s_acos, [r0x, r0x], z, z)[0];
    // 5: r0.y = v2.z < 0 ; 6: r0.zw = (v1.z, v1.x) · ScaleTrans.x
    let neg_z = lt(v2[2], 0.0);
    let r0zw = [v1[2] * m.tc_scale_trans_pxz[0], v1[0] * m.tc_scale_trans_pxz[0]];
    // 7: r1.x = neg_z ? −r0.w : r0.w ; 8: r1.zw = v1.y · ScaleTrans.y − ScaleTrans.w
    let r1x = if neg_z { -r0zw[1] } else { r0zw[1] };
    let r1zw = mad(v1[1], m.tc_scale_trans_pxz[1], -m.tc_scale_trans_pxz[3]);
    // 9: r2.xyz = Pxz(r1.x, r1.w)
    let r2 = texsample::sample(m.pxz_base, 0, &m.s_pxz, [r1x, r1zw], z, z);
    // 10-11: r0.y = 0 < v2.x ; r1.y = r0.y ? −r0.z : r0.z ; 12: r0.yzw = Pxz(r1.y, r1.z)
    let pos_x = lt(0.0, v2[0]);
    let r1y = if pos_x { -r0zw[0] } else { r0zw[0] };
    let r0b = texsample::sample(m.pxz_base, 0, &m.s_pxz, [r1y, r1zw], z, z);
    // 13-14: r0.xyz = r0.x · (r2 − r0.yzw) + r0.yzw
    let mut r0c = [0.0f32; 3];
    for k in 0..3 {
        r0c[k] = mad(r0x, -r0b[k] + r2[k], r0b[k]);
    }
    // 15-16: r1 = PyBase(v3.xy) ; r2 = PyX2(v3.zw) through SMapPyBaseColor
    let r1 = texsample::sample(m.py_base, 0, &m.s_py, [v3[0], v3[1]], z, z);
    let r2b = texsample::sample(m.py_x2, 0, &m.s_py, [v3[2], v3[3]], z, z);
    // 17-18: r1 = 2 · (r1 · r2) − r0
    let mut r1c = [0.0f32; 3];
    for k in 0..3 {
        let p = r1[k] * r2b[k];
        r1c[k] = mad(p, 2.0, -r0c[k]);
    }
    // 19-21: r0.w = 1 − TMapACosSmoothPy(|v2.y|).x
    let r0w = texsample::sample(m.acos_py, 0, &m.s_acos_py, [v2[1].abs(), v2[1].abs()], z, z)[0];
    let r0w = -r0w + 1.0;
    // 22: r0.xyz = r0.w · r1 + r0
    let mut out = [0.0f32; 3];
    for k in 0..3 {
        out[k] = mad(r0w, r1c[k], r0c[k]);
    }
    out
}

/// Apply the blend states of `state.json` (RenderDoc's pipeline state at the sampled eids: `blendState.alphaToCoverage`)
/// to every draw of the same (pixel shader, index count) — the material's state is the same in every run.
pub fn apply_state(draws: &mut [DrawRec], state_json: &std::path::Path) -> Result<usize, String> {
    let bytes = read_maybe_gz(state_json)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", state_json.display()))?;
    let mut atc: Vec<(String, u32, bool)> = Vec::new();
    for e in v.as_array().ok_or("state.json is not an array")? {
        let eid = e["eid"].as_u64().unwrap_or(0);
        let flag = e["blendState"]["alphaToCoverage"].as_bool().unwrap_or(false);
        if let Some(d) = draws.iter().find(|d| d.eid == eid) {
            atc.push((d.ps.clone(), d.idx, flag));
        }
    }
    let mut n = 0;
    for d in draws.iter_mut() {
        if let Some((_, _, f)) = atc.iter().find(|(ps, idx, _)| *ps == d.ps && *idx == d.idx) {
            d.alpha_to_coverage = *f;
            if *f { n += 1; }
        }
    }
    Ok(n)
}

// ---------------------------------------------------------------------------------------------------------
// The water-id map (eid 12420 of every run): VS 17011 / PS 17012, 4096 tile instances into 17004 (R8G8_UINT).

/// A vertex of the id mesh 5392 (stride 28): POSITION f32×3 @0, BLENDINDICES u8×4 @12 (x = the water type),
/// NORMAL @16, COLOR @24.
#[derive(Clone, Copy, Debug)]
pub struct IdVertex {
    pub pos: [f32; 3],
    pub kind: u32,
}

/// An instance of the id stream 17032 (stride 64): TEXCOORD4/5/6 = the world matrix rows (float4 each, the
/// translation in .w), TEXCOORD7.x = the plane id.
#[derive(Clone, Copy, Debug)]
pub struct IdInstance {
    pub rows: [[f32; 4]; 3],
    pub plane: f32,
}

pub fn parse_id_vertices(vb: &[u8]) -> Vec<IdVertex> {
    vb.chunks_exact(28).map(|b| IdVertex { pos: [f32_at(b, 0), f32_at(b, 4), f32_at(b, 8)], kind: b[12] as u32 }).collect()
}

pub fn parse_id_instances(ib: &[u8]) -> Vec<IdInstance> {
    ib.chunks_exact(64).map(|b| IdInstance { rows: [[f32_at(b, 0), f32_at(b, 4), f32_at(b, 8), f32_at(b, 12)], [f32_at(b, 16), f32_at(b, 20), f32_at(b, 24), f32_at(b, 28)], [f32_at(b, 32), f32_at(b, 36), f32_at(b, 40), f32_at(b, 44)]], plane: f32_at(b, 48) }).collect()
}

/// VS 17011: world = (pos, 1) · rows; `o0.xy = dp4((world, 1), WorldToHPos[k])`; the ids ride as flat attributes.
pub fn vs_17011(v: &IdVertex, inst: &IdInstance, world_to_hpos: &[[f32; 4]; 2]) -> ([f32; 2], u32, f32) {
    let p = [v.pos[0], v.pos[1], v.pos[2], 1.0];
    let dp4 = |r: [f32; 4]| ((p[0] * r[0] + p[1] * r[1]) + p[2] * r[2]) + p[3] * r[3];
    let world = [dp4(inst.rows[0]), dp4(inst.rows[1]), dp4(inst.rows[2]), 1.0];
    let dp4w = |r: [f32; 4]| ((world[0] * r[0] + world[1] * r[1]) + world[2] * r[2]) + world[3] * r[3];
    ([dp4w(world_to_hpos[0]), dp4w(world_to_hpos[1])], v.kind, inst.plane)
}

/// The id pass over a cleared 2048² map: PS 17012 writes (kind + 1, plane) per covered pixel (no blend; the
/// flat attributes are the provoking vertex's — D3D's first vertex of the triangle).
pub fn run_id_pass(verts: &[IdVertex], indices: &[u32], instances: &[IdInstance], world_to_hpos: &[[f32; 4]; 2], w: u32, h: u32) -> Buf {
    let mut out = Buf::new(w, h, 2);
    for inst in instances {
        let vs: Vec<([f32; 2], u32, f32)> = verts.iter().map(|v| vs_17011(v, inst, world_to_hpos)).collect();
        for t in indices.chunks_exact(3) {
            let a = [vs[t[0] as usize], vs[t[1] as usize], vs[t[2] as usize]];
            let p = [viewport(a[0].0, w, h), viewport(a[1].0, w, h), viewport(a[2].0, w, h)];
            // nointerpolation attributes: the first vertex provides them
            let (kind, plane) = (a[0].1, a[0].2);
            raster_tri(p, w, h, |x, y, _| {
                out.set(x, y, 0, (kind + 1) as f32);
                out.set(x, y, 1, plane);
            });
        }
    }
    out
}

/// The nine LM raster offsets, in the game's issue order (engineer 2's RE of `LM01_Trans_RasterSS`), in
/// texels: run k shifts the raster by `OFFSETS[k] / 9` texels.
pub const OFFSETS: [(i32, i32); 9] = [(-4, 2), (-1, 3), (2, 4), (-3, -1), (0, 0), (3, 1), (-2, -4), (1, -3), (4, -2)];

/// The draws of run `k` derived from a captured run `from` (the same frame's runs differ only by their raster
/// offset): every `GbxVTexCoordToRasterLM` translation moves by (Δox / 9, −Δoy / 9) texels in NDC units.
pub fn shift_run(run: &[DrawRec], from: usize, k: usize) -> Vec<DrawRec> {
    let (dx, dy) = (OFFSETS[k].0 - OFFSETS[from].0, OFFSETS[k].1 - OFFSETS[from].1);
    let ndc = |t: i32| t as f32 / 9.0 * (2.0 / W as f32);
    run.iter()
        .map(|d| {
            let mut d = d.clone();
            if d.rlm[0] != [0.0; 4] {
                d.rlm[0][3] += ndc(dx);
                d.rlm[1][3] -= ndc(dy);
            }
            d
        })
        .collect()
}

/// A float4x2 printed as HLSL rows (column_major) → the two DXBC registers (the columns).
pub fn regs2(v: &serde_json::Value) -> [[f32; 4]; 2] {
    let mut r = [[0.0f32; 4]; 2];
    if let Some(rows) = v.as_array() {
        for (i, row) in rows.iter().enumerate().take(4) {
            for k in 0..2 {
                r[k][i] = row[k].as_f64().unwrap_or(0.0) as f32;
            }
        }
    }
    r
}

pub fn vec2(v: &serde_json::Value) -> [f32; 2] {
    [v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32]
}

/// The water-tint draws of a run (PS 17018) as `WaterDraw`s: the four LM meshes in draw order, the instance
/// stream offsets (0, 1, 2, 3 — from the 127447 mesh export: instance buffer offsets 0 / 48 / 96 / 144 bytes).
pub fn water_draws(run: &[DrawRec]) -> Vec<WaterDraw> {
    run.iter()
        .filter(|d| d.ps == "17018")
        .enumerate()
        .map(|(k, d)| {
            let cb = &d.shader_v["g_CBufferV"];
            WaterDraw {
                eid: d.eid,
                mesh: k,
                instance_first: k,
                instance_count: d.inst.max(1) as usize,
                scale_ss: vec2(&cb["LM01_Scale_RasterSS"]),
                trans_ss: vec2(&cb["LM01_Trans_RasterSS"]),
                world_to_id: regs2(&cb["World_To_i2WaterId"]),
                world_min_xz: vec2(&cb["WorldMinXZ"]),
                world_max_xz: vec2(&cb["WorldMaxXZ"]),
                scale_out: d.shader_p["g_CBufferP"]["ScaleOut"].as_f64().unwrap_or(0.0) as f32,
            }
        })
        .collect()
}

/// The chart of a pre-pass draw in the LM instance stream (the water pass draws the same objects: mesh 0 =
/// the 24-index wall = instance 0, mesh 1 = the 12-index pad = instance 1, mesh 2 = the 5751-index tree =
/// instance 2, the tiles = instances 3…): the instance index, or None for a draw without one.
pub fn instance_of(d: &DrawRec, tile_ordinal: usize) -> Option<usize> {
    match (d.ps.as_str(), d.idx) {
        ("8401", 24) if d.i_pxz == 0 && d.i_py == 0 => Some(0),
        ("17025", 12) => Some(1),
        ("17022", _) | ("17023", _) => Some(2),
        ("8401", 24) => Some(3 + tile_ordinal),
        _ => None,
    }
}

/// `GbxVTexCoordToRasterLM` as the game's CPU builds it for run `k` from the chart's ST (the instance stream's
/// TEXCOORD7) and the run's raster offset — the f32 expression that reproduces every captured tile draw bit for
/// bit (`prepass_check::trans_fit`): scale = (2·st.x, −2·st.y), translation = 2·st.zw·(1, −1) + LM01_Trans,
/// LM01_Trans = (ox/9·q − 1, 1 − oy/9·q), q = 2/2048.
pub fn raster_lm_for(st: [f32; 4], k: usize) -> [[f32; 4]; 2] {
    let (ox, oy) = OFFSETS[k];
    let q = 2.0f32 / W as f32;
    let tx = 2.0 * st[2] + ((ox as f32 / 9.0) * q - 1.0);
    let ty = -(2.0 * st[3]) + ((-oy as f32 / 9.0) * q + 1.0);
    [[2.0 * st[0], 0.0, 0.0, tx], [-0.0, -2.0 * st[1], -0.0, ty]]
}

/// `LM01_Trans_RasterSS` of run `k` (the water pass's VS 17017 constant).
pub fn lm01_trans_for(k: usize) -> [f32; 2] {
    let (ox, oy) = OFFSETS[k];
    let q = 2.0f32 / W as f32;
    [(ox as f32 / 9.0) * q - 1.0, (-oy as f32 / 9.0) * q + 1.0]
}

/// The draws of run `k` rebuilt from a captured run: every draw with a chart gets its `GbxVTexCoordToRasterLM`
/// from its instance's ST and the run's offset, the water draws their `LM01_Trans_RasterSS`.
pub fn rebuild_run(run: &[DrawRec], k: usize, instances: &[crate::sunpass::LmInstance]) -> Vec<DrawRec> {
    let mut tile = 0usize;
    let trans = lm01_trans_for(k);
    run.iter()
        .map(|d| {
            let mut d = d.clone();
            if let Some(i) = instance_of(&d, tile) {
                if d.ps == "8401" && d.i_pxz != 0 {
                    tile += 1;
                }
                if let Some(inst) = instances.get(i) {
                    d.rlm = raster_lm_for(inst.st, k);
                }
            }
            if d.ps == "17018" {
                d.shader_v["g_CBufferV"]["LM01_Trans_RasterSS"] = serde_json::json!([trans[0], trans[1]]);
            }
            d
        })
        .collect()
}

/// For one texel: every triangle of the run's material draws whose snapped edges come within `near` sub-pixel
/// units (1/256 px) of the texel centre — the raster's tie cases — with the edge values (positive = inside).
pub fn edge_report(run: &[DrawRec], meshes: &Meshes, x: u32, y: u32, near: i64) -> Vec<String> {
    let mut out = Vec::new();
    let px = [x as i64 * 256 + 128, y as i64 * 256 + 128];
    for d in run {
        let Some(mesh) = meshes.for_draw(d) else { continue };
        let uses_uv1 = d.ps == "17022" || d.ps == "17023" || d.ps == "17025";
        let n_tri = (d.idx as usize / 3).min(mesh.idx.len() / 3);
        for t in 0..n_tri {
            let i = [mesh.idx[3 * t] as usize, mesh.idx[3 * t + 1] as usize, mesh.idx[3 * t + 2] as usize];
            let lmuv = |k: usize| if uses_uv1 { mesh.uv1[i[k]] } else { mesh.uv0[i[k]] };
            let p = [viewport(lm_ndc(lmuv(0), &d.rlm), W, H), viewport(lm_ndc(lmuv(1), &d.rlm), W, H), viewport(lm_ndc(lmuv(2), &d.rlm), W, H)];
            let q: Vec<[i64; 2]> = p.iter().map(|v| [(v[0] * SUBPIX).round_ties_even() as i64, (v[1] * SUBPIX).round_ties_even() as i64]).collect();
            let edge = |a: [i64; 2], b: [i64; 2], c: [i64; 2]| -> i64 { (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) };
            let area = edge(q[0], q[1], q[2]);
            if area == 0 { continue; }
            let s = if area > 0 { 1 } else { -1 };
            let e = [edge(q[0], q[1], px) * s, edge(q[1], q[2], px) * s, edge(q[2], q[0], px) * s];
            // the distance of the centre to each edge in sub-pixel units = e / |edge length|
            let len = |a: [i64; 2], b: [i64; 2]| (((b[0] - a[0]).pow(2) + (b[1] - a[1]).pow(2)) as f64).sqrt();
            let dist = [e[0] as f64 / len(q[0], q[1]), e[1] as f64 / len(q[1], q[2]), e[2] as f64 / len(q[2], q[0])];
            let inside_or_near = dist.iter().all(|&v| v > -(near as f64));
            if inside_or_near {
                out.push(format!("    eid {} PS {} tri {t}: snapped {:?}; edge values {:?} (distances in 1/256 px {:.2?}); exact p {:?}", d.eid, d.ps, q, e, dist, p));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests_capture_facts {
    use super::*;
    use crate::texsample::{Address, Level, TexFmt};

    #[test]
    fn raster_lm_matrix_is_built_from_the_chart_st_and_the_run_offset() {
        // tile instance 3 of the capture (vb_17033): ST (0.010726064, 0.0106363185, 0.79983926, 0.33201912); run 6 = offset (−2, −4):
        // the captured GbxVTexCoordToRasterLM of eid 32 (draws-frame127447)
        let st = [0.010726064f32, 0.0106363185, 0.79983926, 0.33201912];
        let r = raster_lm_for(st, 6);
        assert_eq!(r[0][0].to_bits(), 0.021452129f32.to_bits());
        assert_eq!(r[1][1].to_bits(), (-0.021272637f32).to_bits());
        assert_eq!(r[0][3].to_bits(), 0.59946156f32.to_bits());
        assert_eq!(r[1][3].to_bits(), 0.3363958f32.to_bits());
        // run 7 (offset (1, −3)) = eid 12509's translation
        let r7 = raster_lm_for(st, 7);
        assert_eq!(r7[0][3].to_bits(), 0.5997869968414307f32.to_bits());
        assert_eq!(r7[1][3].to_bits(), 0.33628731966018677f32.to_bits());
        // the water pass's LM01_Trans_RasterSS of run 6: (−1.0002169609069824, 1.0004340410232544)
        let t = lm01_trans_for(6);
        assert_eq!(t[0].to_bits(), (-1.0002169609069824f32).to_bits());
        assert_eq!(t[1].to_bits(), 1.0004340410232544f32.to_bits());
    }

    #[test]
    fn snapping_rounds_ties_to_even() {
        // a vertex at exactly 258810.5 / 256 px snaps to the even 258810 (the capture's run-3 tie at texel (1012, 266))
        let x = 1010.978515625f32;
        assert_eq!((x * SUBPIX).round_ties_even() as i64, 258810);
        assert_eq!((x * SUBPIX).round() as i64, 258811);
        // the same triangle through the rasteriser: the pixel centre 0.23/256 px outside the unsnapped edge is covered
        let mut hit = false;
        raster_tri([[1012.44214, 188.64032], [1013.7827, 316.64545], [1010.9785156, 206.95361]], 2048, 2048, |x, y, _| if (x, y) == (1012, 266) { hit = true });
        assert!(hit);
    }

    #[test]
    fn the_id_pass_maps_world_xz_to_the_2048_map() {
        // WorldToHPos of eid 12420 as registers: x = world.x/1024 − 1, y = world.z/1024 − 1 → the viewport puts world (x, z) at texel (x, 2048 − z)
        let w2h = [[0.0009765625f32, 0.0, 0.0, -1.0], [0.0, 0.0, 0.0009765625, -1.0]];
        let inst = IdInstance { rows: [[1.0, 0.0, 0.0, 100.0], [0.0, 1.0, 0.0, 4.0], [0.0, 0.0, 1.0, 300.0]], plane: 0.0 };
        let v = IdVertex { pos: [16.0, 0.0, 16.0], kind: 2 };
        let (ndc, kind, plane) = vs_17011(&v, &inst, &w2h);
        let p = viewport(ndc, 2048, 2048);
        assert!((p[0] - 116.0).abs() < 1e-3 && (p[1] - (2048.0 - 316.0)).abs() < 1e-3, "{p:?}");
        assert_eq!((kind, plane), (2, 0.0));
    }

    #[test]
    fn water_tint_discards_above_the_surface_and_below_the_floor() {
        let fog = Texture { fmt: TexFmt::Bgra8, w: 2, h: 1, mips: 1, slices: 1, levels: vec![vec![Level::from_f32(2, 1, vec![[0.0, 0.5, 1.0, 0.5], [0.0, 0.5, 1.0, 1.0]])]], complete: true };
        let tr = Texture { fmt: TexFmt::Rgba8, w: 2, h: 1, mips: 1, slices: 1, levels: vec![vec![Level::from_f32(2, 1, vec![[1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]])]], complete: true };
        let mut ids = Buf::new(4, 4, 2);
        ids.set(1, 1, 0, 1.0); // type 0 + 1, plane 0
        let w = WaterData { ids: &ids, top_by_plane: vec![[7.0, 0.0, 0.0, 1.0]], depth_by_id: vec![[3.0, 1.0 / 3.5, 0.0, 1.0]], fog: &fog, transmittance: &tr, sampler: Sampler::bilinear_no_mip(Address::Clamp) };
        let at = |y: f32, x: f32| WaterVsOut { clip: [0.0; 2], id_uv: [x, 1.5], world_y: y, clipdist: [1.0; 4] };
        assert!(ps_17018(&at(8.0, 1.5), &w, 1.0 / 9.0).is_none(), "above the surface");
        assert!(ps_17018(&at(3.0, 1.5), &w, 1.0 / 9.0).is_none(), "below top − depth − 0.1");
        assert!(ps_17018(&at(4.0, 2.5), &w, 1.0 / 9.0).is_none(), "no water id");
        let (o0, o1) = ps_17018(&at(4.0, 1.5), &w, 1.0 / 9.0).unwrap();
        // depth 3 → u = 2·3/3.5 → clamped: the last fog texel (alpha 1) → o0 = fog·1/9, o1 = transmittance·0
        assert!((o0[2] - 1.0 / 9.0).abs() < 1e-6 && o0[3] == 0.0, "{o0:?}");
        assert_eq!(o1, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn ps_8401_with_a_zero_world_matrix_is_the_pxz_sample_at_the_map_translation() {
        // the terrain material with a zero normal / position: the blend weights saturate to 0 and the colour is the
        // Pxz base texture at (0, −r5.z) of slice iPxz — here a 1×1 texture per slice tells which slice was read
        let mk = |c: [f32; 4]| Texture { fmt: TexFmt::Rgba8, w: 1, h: 1, mips: 1, slices: 2, levels: vec![vec![Level::from_f32(1, 1, vec![[9.0; 4]])], vec![Level::from_f32(1, 1, vec![c])]], complete: true };
        let base = mk([0.25, 0.5, 0.75, 1.0]);
        let x2 = mk([0.5; 4]);
        let h2 = mk([1.0; 4]);
        let mat = TerrainMaterial { py_pxz: vec![[0.0; 4]; 8], py_x2: vec![[0.0; 4]; 8], py_h2: vec![[0.0; 4]; 4], base: &base, x2: &x2, h2: &h2, sampler: Sampler::trilinear(Address::Wrap) };
        let c = ps_8401(&mat, [0.0; 3], [0.0; 3], [0.0; 3], 1, 1, 1, 0xffff_ffff, [0.0; 3], [0.0; 3]);
        assert_eq!(c, [0.25, 0.5, 0.75]);
    }
}

/// PS 9539 (the textured HueMask class — RE 13's DXBC read, RE 15's table): the BaseColor sample recoloured toward the placement
/// colour's target through the HueMask sampled at the same footprint: m = HueMask(uv); k = max(m.g − ½(m.r + m.b), 0);
/// recol = sat((m.g − k)·mean(T) + k·T); o.rgb = lerp(BaseColor.rgb, recol, m.a) — the MASK's alpha (PS 9544's form), not BaseColor.a:
/// RE 13's reading of 9539's lerp factor as BaseColor.a takes every BC1 (alpha 1) Technics / RoadTech surface to the target colour and
/// REGRESSES the corpus (tiny16 Sunset items 0.978/0.973/1.022 → 0.944/0.957/1.006 vs the editor) where the mask's alpha leaves them at
/// 0.978/0.973/1.022 (= the base) and matches stpad's red poles either way (the mask is ~1 there). LMTOOL_HUE_TEXTURED_BLEND=base =
/// the BaseColor.a form as a study. Alpha := 1, ×lm_scale, as the plain class.
pub fn ps_basecolor_hue(tex: &Texture, mask: &Texture, target: [f32; 3], s: &Sampler, uv: [f32; 2], ddx: [f32; 2], ddy: [f32; 2], alpha_test: Option<f32>, lm_scale: f32) -> Option<[f32; 4]> {
    let c = texsample::sample(tex, 0, s, uv, ddx, ddy);
    if let Some(a_ref) = alpha_test { if c[3] - a_ref < 0.0 { return None; } }
    let m = texsample::sample(mask, 0, s, uv, ddx, ddy);
    let k = (m[1] - 0.5 * (m[0] + m[2])).max(0.0);
    let mean_t = (target[0] + target[1] + target[2]) / 3.0;
    let recol = [((m[1] - k) * mean_t + k * target[0]).clamp(0.0, 1.0), ((m[1] - k) * mean_t + k * target[1]).clamp(0.0, 1.0), ((m[1] - k) * mean_t + k * target[2]).clamp(0.0, 1.0)];
    static BLEND_BASE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_HUE_TEXTURED_BLEND").as_deref() == Ok("base"));
    let a = if *BLEND_BASE { c[3] } else { m[3] };
    let o = [c[0] + a * (recol[0] - c[0]), c[1] + a * (recol[1] - c[1]), c[2] + a * (recol[2] - c[2])];
    Some([o[0] * lm_scale, o[1] * lm_scale, o[2] * lm_scale, 1.0 * lm_scale])
}
