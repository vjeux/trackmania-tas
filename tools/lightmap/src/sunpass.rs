//! The lightmapper's DIRECT SUN pass, TRANSCRIBED from the capture (passcap/pwc-day frame 127448, eids 476–765):
//! VS 15183 (Vertex_15183.txt) + PS 15187 (Pixel_15187.txt) rasterised into the 2048² RGBA16F target with the
//! D3D11 rules, 9 draws per object at the 9 sub-texel raster offsets (LM01_Trans_RasterSS), OutScale 1/9,
//! blend One/One; the shadow test is `sample_c_lz` through SMapShadow (ClampEdge, comparison LINEAR = 2×2 PCF,
//! GreaterEqual — logs/samplers-frame127448.json) against the D16 sun shadow map (reversed z, GREATER, cleared 0).
//!
//! Nothing here is inferred: the vertex formats come from the draws' input layouts (env/frame127448/mesh.json),
//! the per-instance data from the instance stream (v6 quaternion, v7 translation + uniform scale, v8 = the chart's
//! LM ST in 01 atlas space — BLENDINDICES 0xffff means "use v8", the g_TcLM_ST_LM01 path is transcribed too),
//! the constants from the draws' cbuffers (logs/draws-frame127448.json.gz).

use crate::gpufmt::{quantise_f16, Rounding};
use crate::passdiff::Buf;

/// One vertex of the LM mesh stream (stride 40): POSITION f32×3 @0, BLENDINDICES u8×4 @12, NORMAL snorm16×4 @16,
/// PSIZE f32 @24, TEXCOORD0 snorm16×2 @28 (the LM uv), TANGENT snorm16×4 @32.
/// (`psize` = the H-basis vertex shader's tangent-frame mode v4.x, `tangent` its v3 — lmaccum.rs.)
#[derive(Clone, Copy, Debug)]
pub struct LmVertex {
    pub pos: [f32; 3],
    pub chart_idx: u32,
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub psize: f32,
    pub tangent: [f32; 4],
}

/// One instance of the instance stream (stride 48): v6 = quaternion xyzw, v7 = translation xyz + uniform scale w,
/// v8 = LM ST (scale.xy, trans.zw) in 01 atlas space, or the chart base index in .x when the mesh indexes the table.
#[derive(Clone, Copy, Debug)]
pub struct LmInstance {
    pub q: [f32; 4],
    pub t: [f32; 3],
    pub scale: f32,
    pub st: [f32; 4],
    pub st_x_bits: u32,
}

/// snorm16 → float as D3D does it (v / 32767, clamped at −1).
#[inline]
pub fn snorm16(v: i16) -> f32 {
    (v as f32 / 32767.0).max(-1.0)
}

pub fn parse_lm_vertices(vb: &[u8]) -> Vec<LmVertex> {
    let n = vb.len() / 40;
    (0..n)
        .map(|i| {
            let b = &vb[i * 40..i * 40 + 40];
            let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            let h = |o: usize| i16::from_le_bytes(b[o..o + 2].try_into().unwrap());
            LmVertex {
                pos: [f(0), f(4), f(8)],
                chart_idx: (b[13] as u32) << 8 | b[12] as u32,
                normal: [snorm16(h(16)), snorm16(h(18)), snorm16(h(20))],
                uv: [snorm16(h(28)), snorm16(h(30))],
                psize: f(24),
                tangent: [snorm16(h(32)), snorm16(h(34)), snorm16(h(36)), snorm16(h(38))],
            }
        })
        .collect()
}

pub fn parse_instances(ib: &[u8]) -> Vec<LmInstance> {
    let n = ib.len() / 48;
    (0..n)
        .map(|i| {
            let b = &ib[i * 48..i * 48 + 48];
            let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            LmInstance { q: [f(0), f(4), f(8), f(12)], t: [f(16), f(20), f(24)], scale: f(28), st: [f(32), f(36), f(40), f(44)], st_x_bits: u32::from_le_bytes(b[32..36].try_into().unwrap()) }
        })
        .collect()
}

/// The rotation rows VS 15183 builds from the quaternion (instructions 0–8, 20–28): o3 = rows · normal.
pub fn rotation_rows(q: [f32; 4]) -> [[f32; 3]; 3] {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    // 0: r0 = v6.yzxx + v6.yzxx
    let r0 = [y + y, z + z, x + x, x + x];
    // 1: r1 = r0.xyw * v6.w
    let r1 = [r0[0] * w, r0[1] * w, r0[3] * w];
    // 2: r0.w = -v6.y * r0.x + 1 ; 3: r2.x = -v6.z * r0.y + r0.w
    let r0w = -y * r0[0] + 1.0;
    let m00 = -z * r0[1] + r0w;
    // 4: r0.z = -v6.x * r0.z + 1 ; 5: r3.y = -v6.z * r0.y + r0.z
    let r0z = -x * r0[2] + 1.0;
    let m11 = -z * r0[1] + r0z;
    // 6: r4 = v6.xyx * r0.xyy + r1.yzx
    let r4 = [x * r0[0] + r1[1], y * r0[1] + r1[2], x * r0[1] + r1[0]];
    // 7: r1' = v6.xxy * r0.yxy - r1.xyz
    let r1b = [x * r0[1] - r1[0], x * r0[0] - r1[1], y * r0[1] - r1[2]];
    // 8: r0.z = -v6.y * r0.x + r0.z
    let m22 = -y * r0[0] + r0z;
    // 20-28: rows (r2.x, r1.y, r4.z), (r4.x, r3.y, r1.z), (r1.x, r4.y, r0.z)
    [[m00, r1b[1], r4[2]], [r4[0], m11, r1b[2]], [r1b[0], r4[1], m22]]
}

/// The per-draw constants (VS ShaderV + PS ShaderP cbuffers).
#[derive(Clone, Debug)]
pub struct SunDraw {
    pub eid: u64,
    pub mesh: usize,
    pub instance_first: usize,
    pub instance_count: usize,
    pub scale_ss: [f32; 2],
    pub trans_ss: [f32; 2],
    pub world_pw01_shadow: [[f32; 4]; 4],
    pub dir_in_world: [f32; 3],
    pub light_rgb: [f32; 3],
    pub out_scale: f32,
}

#[derive(Debug)]
pub struct LmMesh {
    pub verts: Vec<LmVertex>,
    pub indices: Vec<u16>,
}

/// A vertex after VS 15183: clip xy (w = 1, z = 0.5), world position o1, world normal o3.
#[derive(Clone, Copy, Debug)]
struct VsOut {
    clip: [f32; 2],
    pos: [f32; 3],
    nrm: [f32; 3],
}

fn vs_15183(v: &LmVertex, inst: &LmInstance, table: &[[f32; 4]], d: &SunDraw) -> VsOut {
    let rows = rotation_rows(inst.q);
    // 9-17: the chart ST: idx = (b1 << 8 | b0); idx < 0xffff → table[idx + v8.x (as int)], else v8
    let st = if v.chart_idx < 0xffff {
        let i = v.chart_idx.wrapping_add(inst.st_x_bits) as usize;
        table.get(i).copied().unwrap_or([0.0; 4])
    } else {
        inst.st
    };
    // 18: r5.xy = ST.xy * Scale ; 19: r5.zw = Scale.xy * ST.zw + Trans.xy ; 29: o0.xy = r5.xy * uv + r5.zw
    let sxy = [st[0] * d.scale_ss[0], st[1] * d.scale_ss[1]];
    let tzw = [d.scale_ss[0] * st[2] + d.trans_ss[0], d.scale_ss[1] * st[3] + d.trans_ss[1]];
    let clip = [sxy[0] * v.uv[0] + tzw[0], sxy[1] * v.uv[1] + tzw[1]];
    // 22-28: the normal through the rows
    let nrm = [
        v.normal[0] * rows[0][0] + v.normal[1] * rows[0][1] + v.normal[2] * rows[0][2],
        v.normal[0] * rows[1][0] + v.normal[1] * rows[1][1] + v.normal[2] * rows[1][2],
        v.normal[0] * rows[2][0] + v.normal[1] * rows[2][1] + v.normal[2] * rows[2][2],
    ];
    // 30-37: r1 = v0 * scale, 1; o1 = dp4(r1, (row, t))
    let p = [v.pos[0] * inst.scale, v.pos[1] * inst.scale, v.pos[2] * inst.scale];
    let pos = [
        p[0] * rows[0][0] + p[1] * rows[0][1] + p[2] * rows[0][2] + inst.t[0],
        p[0] * rows[1][0] + p[1] * rows[1][1] + p[2] * rows[1][2] + inst.t[1],
        p[0] * rows[2][0] + p[1] * rows[2][1] + p[2] * rows[2][2] + inst.t[2],
    ];
    VsOut { clip, pos, nrm }
}

/// The D16 shadow map as stored (UNORM16 → f32, the value the comparison sampler sees).
pub struct ShadowMap<'a> {
    pub depth: &'a Buf,
}

impl<'a> ShadowMap<'a> {
    #[inline]
    fn texel(&self, x: i64, y: i64) -> f32 {
        let xi = x.clamp(0, self.depth.w as i64 - 1) as u32;
        let yi = y.clamp(0, self.depth.h as i64 - 1) as u32;
        self.depth.get(xi, yi, 0)
    }
    /// `sample_c_lz` with a LINEAR comparison filter, ClampEdge, GreaterEqual: the four texels around the sample
    /// point compared individually, the results bilinearly weighted (D3D11 §7.18.16 — the weights come from the
    /// fractional position after the half-texel offset).
    pub fn sample_cmp_linear_ge(&self, u: f32, v: f32, z: f32) -> f32 {
        let (w, h) = (self.depth.w as f32, self.depth.h as f32);
        let x = u * w - 0.5;
        let y = v * h - 0.5;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let (xi, yi) = (x0 as i64, y0 as i64);
        let c = |tx: i64, ty: i64| -> f32 { if z >= self.texel(tx, ty) { 1.0 } else { 0.0 } };
        let c00 = c(xi, yi);
        let c10 = c(xi + 1, yi);
        let c01 = c(xi, yi + 1);
        let c11 = c(xi + 1, yi + 1);
        let top = c00 + (c10 - c00) * fx;
        let bot = c01 + (c11 - c01) * fx;
        top + (bot - top) * fy
    }
}

/// PS 15187 for one pixel: world position p (v1), world normal n (v3, NOT normalised).
#[inline]
pub fn ps_15187(p: [f32; 3], n: [f32; 3], d: &SunDraw, sm: &ShadowMap) -> [f32; 4] {
    let m = &d.world_pw01_shadow;
    // 2-4: dp4(p, register k) for k = 0..2; the cbuffer's float4x4 is column_major (HLSL default), so register k
    // is COLUMN k of the matrix as the log prints it (rows = HLSL rows): uvz_k = Σ_i p_i · M[i][k] + M[3][k] — the
    // row-vector convention with the translation in the last row (1 · M[3][k])
    let u = p[0] * m[0][0] + p[1] * m[1][0] + p[2] * m[2][0] + m[3][0];
    let v = p[0] * m[0][1] + p[1] * m[1][1] + p[2] * m[2][1] + m[3][1];
    let z = p[0] * m[0][2] + p[1] * m[1][2] + p[2] * m[2][2] + m[3][2];
    // 5: the comparison sample
    let s = sm.sample_cmp_linear_ge(u, v, z);
    // 6-7: ndl = max(dot(n, -Dir), 0)
    let ndl = (n[0] * -d.dir_in_world[0] + n[1] * -d.dir_in_world[1] + n[2] * -d.dir_in_world[2]).max(0.0);
    // 8-11
    let s = s * ndl;
    [s * d.light_rgb[0] * d.out_scale, s * d.light_rgb[1] * d.out_scale, s * d.light_rgb[2] * d.out_scale, 1.0 * d.out_scale]
}

/// How the f32 blend result reaches the f16 target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlendModel {
    /// acc = f16(acc + src) — the source blended at full precision, the sum rounded on store
    RoundSum,
    /// acc = f16(acc + f16(src)) — the source rounded to the target format first
    RoundSrcAndSum,
    /// acc = f16_rtz(acc + src) — the sum truncated on store
    TruncSum,
    /// acc = f16_rtz(acc + f16_rtz(src))
    TruncSrcAndSum,
    /// acc = f16_rtne(acc + f16_rtz(src)) — the shader output truncated to the target format, the blend rounded
    TruncSrcRoundSum,
}

/// The 2048² RGBA16F accumulation target.
pub struct Target {
    pub w: u32,
    pub h: u32,
    pub px: Vec<[f32; 4]>,
}

/// Rasteriser variants under test (the D3D11 rules leave two things to the implementation): whether the vertex
/// positions snap to the 1/256 grid by rounding (default) or by truncation, and whether the attributes are
/// interpolated from the snapped positions (default) or the unsnapped ones.
pub static RASTER_SNAP_FLOOR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The rasteriser's 1/256-pixel snapping TIE rule: 2 = round half to EVEN (the default — the GPU's: on the captured
/// H-basis pass the vegetation's varying normals go from 99.69 % to 99.99 % exact single-fragment values with it, the
/// flat meshes and the coverage unchanged; `hbasis-check --snap-tie N`), 0 = round half away from zero (the old
/// default), 3 = round half down (toward −∞).
pub static RASTER_SNAP_TIE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);
/// Snap a screen coordinate to the 1/256-pixel grid under the study switches.
pub fn snap256(c: f32) -> f32 {
    if RASTER_SNAP_FLOOR.load(std::sync::atomic::Ordering::Relaxed) {
        return (c * 256.0).floor() / 256.0;
    }
    let s = c * 256.0;
    let r = match RASTER_SNAP_TIE.load(std::sync::atomic::Ordering::Relaxed) {
        2 => { let f = s.floor(); let d = s - f; if d > 0.5 { f + 1.0 } else if d < 0.5 { f } else if (f as i64) % 2 == 0 { f } else { f + 1.0 } }
        3 => (s - 0.5).ceil(),
        _ => s.round(),
    };
    r / 256.0
}
pub static RASTER_INTERP_UNSNAPPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// D3D11 rasterisation of one triangle at 8 sub-pixel bits with the top-left rule; calls `f(x, y, b0, b1, b2)`
/// for every covered pixel centre with the barycentric weights of the three vertices (w ≡ 1 → linear).
/// The rasteriser's snapped screen position of a clip-space vertex (the viewport transform, 1/256-pixel snapping) — for
/// the attribute-plane study (`lmaccum::interp_mode`).
pub fn screen_snapped(clip: [f32; 2], w: u32, h: u32) -> [f32; 2] {
    let floor_snap = RASTER_SNAP_FLOOR.load(std::sync::atomic::Ordering::Relaxed);
    let snap = |c: f32| { let _ = floor_snap; snap256(c) };
    [snap((clip[0] * 0.5 + 0.5) * w as f32), snap((0.5 - clip[1] * 0.5) * h as f32)]
}

pub fn rasterise_triangle(v: [[f32; 2]; 3], w: u32, h: u32, mut f: impl FnMut(u32, u32, f32, f32, f32)) {
    // viewport: x = (ndc.x + 1)/2 · W, y = (1 − ndc.y)/2 · H, snapped to 1/256 pixel
    let floor_snap = RASTER_SNAP_FLOOR.load(std::sync::atomic::Ordering::Relaxed);
    let snap = |c: f32| { let _ = floor_snap; snap256(c) };
    let ux: Vec<f32> = v.iter().map(|p| (p[0] * 0.5 + 0.5) * w as f32).collect();
    let uy: Vec<f32> = v.iter().map(|p| (0.5 - p[1] * 0.5) * h as f32).collect();
    let sx: Vec<f32> = ux.iter().map(|&c| snap(c)).collect();
    let sy: Vec<f32> = uy.iter().map(|&c| snap(c)).collect();
    let area = (sx[1] - sx[0]) * (sy[2] - sy[0]) - (sx[2] - sx[0]) * (sy[1] - sy[0]);
    if area == 0.0 {
        return;
    }
    // make the winding consistent (NoCull: both windings draw)
    let (ia, ib, ic) = if area > 0.0 { (0, 1, 2) } else { (0, 2, 1) };
    let ax = [sx[ia], sx[ib], sx[ic]];
    let ay = [sy[ia], sy[ib], sy[ic]];
    let area = area.abs();
    // the unsnapped positions in the same order, for the attribute barycentrics when asked
    let unsnapped = RASTER_INTERP_UNSNAPPED.load(std::sync::atomic::Ordering::Relaxed);
    let bx = [ux[ia], ux[ib], ux[ic]];
    let by = [uy[ia], uy[ib], uy[ic]];
    let uarea = ((bx[1] - bx[0]) * (by[2] - by[0]) - (bx[2] - bx[0]) * (by[1] - by[0])).abs();
    let minx = ax.iter().cloned().fold(f32::INFINITY, f32::min).floor().max(0.0) as i64;
    let maxx = ax.iter().cloned().fold(f32::NEG_INFINITY, f32::max).ceil().min(w as f32) as i64;
    let miny = ay.iter().cloned().fold(f32::INFINITY, f32::min).floor().max(0.0) as i64;
    let maxy = ay.iter().cloned().fold(f32::NEG_INFINITY, f32::max).ceil().min(h as f32) as i64;
    // edge functions e_i(p) = (b − a) × (p − a) for edges a→b: (v0→v1), (v1→v2), (v2→v0); with the positive area the
    // inside is e ≥ 0; a top or left edge includes the boundary (top-left rule)
    let edge = |x0: f32, y0: f32, x1: f32, y1: f32, px: f32, py: f32| (x1 - x0) * (py - y0) - (y1 - y0) * (px - x0);
    let is_top_left = |x0: f32, y0: f32, x1: f32, y1: f32| -> bool {
        // in this (x right, y down) orientation with positive area the edges run clockwise on screen: an edge is
        // "top" if horizontal and the interior is below it (dx > 0), "left" if it goes up (dy < 0)
        let (dx, dy) = (x1 - x0, y1 - y0);
        (dy == 0.0 && dx > 0.0) || dy < 0.0
    };
    let tl = [is_top_left(ax[0], ay[0], ax[1], ay[1]), is_top_left(ax[1], ay[1], ax[2], ay[2]), is_top_left(ax[2], ay[2], ax[0], ay[0])];
    for py in miny..maxy {
        let cy = py as f32 + 0.5;
        for px in minx..maxx {
            let cx = px as f32 + 0.5;
            let e0 = edge(ax[0], ay[0], ax[1], ay[1], cx, cy);
            let e1 = edge(ax[1], ay[1], ax[2], ay[2], cx, cy);
            let e2 = edge(ax[2], ay[2], ax[0], ay[0], cx, cy);
            let inside = (e0 > 0.0 || (e0 == 0.0 && tl[0])) && (e1 > 0.0 || (e1 == 0.0 && tl[1])) && (e2 > 0.0 || (e2 == 0.0 && tl[2]));
            if !inside {
                continue;
            }
            // barycentrics: weight of vertex k = the edge opposite to it / area
            let (b0, b1, b2) = if unsnapped && uarea > 0.0 {
                let f0 = edge(bx[0], by[0], bx[1], by[1], cx, cy);
                let f1 = edge(bx[1], by[1], bx[2], by[2], cx, cy);
                let f2 = edge(bx[2], by[2], bx[0], by[0], cx, cy);
                (f1 / uarea, f2 / uarea, f0 / uarea)
            } else {
                (e1 / area, e2 / area, e0 / area)
            };
            // map back to the original vertex order
            let mut b = [0f32; 3];
            b[ia] = b0;
            b[ib] = b1;
            b[ic] = b2;
            f(px as u32, py as u32, b[0], b[1], b[2]);
        }
    }
}

/// Run the 36 draws in issue order into a fresh target.
pub fn run_sun_pass(meshes: &[LmMesh], instances: &[LmInstance], table: &[[f32; 4]], draws: &[SunDraw], sm: &ShadowMap, w: u32, h: u32, blend: BlendModel) -> Target {
    let mut tgt = Target { w, h, px: vec![[0.0; 4]; (w * h) as usize] };
    for d in draws {
        let mesh = &meshes[d.mesh];
        for ii in d.instance_first..d.instance_first + d.instance_count {
            let inst = &instances[ii];
            let vs: Vec<VsOut> = mesh.verts.iter().map(|v| vs_15183(v, inst, table, d)).collect();
            for tri in mesh.indices.chunks_exact(3) {
                let (a, b, c) = (&vs[tri[0] as usize], &vs[tri[1] as usize], &vs[tri[2] as usize]);
                rasterise_triangle([a.clip, b.clip, c.clip], w, h, |x, y, b0, b1, b2| {
                    let p = [a.pos[0] * b0 + b.pos[0] * b1 + c.pos[0] * b2, a.pos[1] * b0 + b.pos[1] * b1 + c.pos[1] * b2, a.pos[2] * b0 + b.pos[2] * b1 + c.pos[2] * b2];
                    let n = [a.nrm[0] * b0 + b.nrm[0] * b1 + c.nrm[0] * b2, a.nrm[1] * b0 + b.nrm[1] * b1 + c.nrm[1] * b2, a.nrm[2] * b0 + b.nrm[2] * b1 + c.nrm[2] * b2];
                    let o = ps_15187(p, n, d, sm);
                    let px = &mut tgt.px[(y * w + x) as usize];
                    for k in 0..4 {
                        let (src, r) = match blend {
                            BlendModel::RoundSum => (o[k], Rounding::NearestEven),
                            BlendModel::RoundSrcAndSum => (quantise_f16(o[k], Rounding::NearestEven), Rounding::NearestEven),
                            BlendModel::TruncSum => (o[k], Rounding::Truncate),
                            BlendModel::TruncSrcAndSum => (quantise_f16(o[k], Rounding::Truncate), Rounding::Truncate),
                            BlendModel::TruncSrcRoundSum => (quantise_f16(o[k], Rounding::Truncate), Rounding::NearestEven),
                        };
                        px[k] = quantise_f16(px[k] + src, r);
                    }
                });
            }
        }
    }
    tgt
}

/// Compare with the captured target (RGBA16F decoded to f32): exact / within 1 f16 ulp / worse, per channel rgb.
pub fn compare_f16(ours: &Target, theirs: &Buf) -> (usize, usize, usize, usize, f32) {
    let mut exact = 0;
    let mut ulp1 = 0;
    let mut worse = 0;
    let mut covered = 0;
    let mut maxd = 0f32;
    for i in 0..ours.px.len() {
        let (x, y) = ((i as u32) % ours.w, (i as u32) / ours.w);
        let t = [theirs.get(x, y, 0), theirs.get(x, y, 1), theirs.get(x, y, 2), theirs.get(x, y, 3)];
        if t[3] != 0.0 || ours.px[i][3] != 0.0 {
            covered += 1;
        }
        for k in 0..4 {
            let o = ours.px[i][k];
            let d = (o - t[k]).abs();
            if d == 0.0 {
                exact += 1;
            } else {
                let ulp = crate::gpufmt::decode_f16(crate::gpufmt::encode_f16(t[k], Rounding::NearestEven).wrapping_add(1)) - t[k];
                if d <= ulp.abs() * 1.001 { ulp1 += 1; } else { worse += 1; }
                if d > maxd { maxd = d; }
            }
        }
    }
    (covered, exact, ulp1, worse, maxd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_rows_of_identity_and_y180() {
        let r = rotation_rows([0.0, 0.0, 0.0, 1.0]);
        assert_eq!(r, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        let r = rotation_rows([0.0, 1.0, 0.0, 0.0]); // 180° about y
        assert_eq!(r[0], [-1.0, 0.0, 0.0]);
        assert_eq!(r[1], [0.0, 1.0, 0.0]);
        assert_eq!(r[2], [0.0, 0.0, -1.0]);
    }

    #[test]
    fn top_left_rule_covers_each_pixel_once() {
        // two triangles sharing the diagonal of a 4×4 pixel square: every centre exactly once
        let mut hits = vec![0u32; 16];
        let quad = [[-1.0f32, 1.0], [1.0, 1.0], [1.0, -1.0], [-1.0, -1.0]];
        for tri in [[0usize, 1, 2], [0, 2, 3]] {
            rasterise_triangle([quad[tri[0]], quad[tri[1]], quad[tri[2]]], 4, 4, |x, y, _, _, _| hits[(y * 4 + x) as usize] += 1);
        }
        assert!(hits.iter().all(|&h| h == 1), "{hits:?}");
    }

    #[test]
    fn pcf_is_bilinear_in_the_compare_results() {
        let mut d = Buf::new(4, 4, 1);
        for y in 0..4 { for x in 0..4 { d.set(x, y, 0, if x >= 2 { 0.9 } else { 0.1 }); } }
        let sm = ShadowMap { depth: &d };
        // receiver z = 0.5: lit (z ≥ 0.1) on the left half, shadowed on the right; the sample point between texels 1 and 2
        let s = sm.sample_cmp_linear_ge(2.0 / 4.0, 0.5, 0.5); // x = 1.5 → fx = 0.5 between texel 1 (lit) and 2 (shadow)
        assert!((s - 0.5).abs() < 1e-6, "{s}");
        assert_eq!(sm.sample_cmp_linear_ge(0.1, 0.5, 0.5), 1.0);
    }
}
