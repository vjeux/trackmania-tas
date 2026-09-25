//! THE CLOUD SPRITES OF THE PEEL'S ENVIRONMENT BLOCK (row 5, port engineer D): 177 draws per direction of
//! VS 14514 / PS 14515 (frame 127448 eids 1101 … 2685 step 9), camera-facing billboards blended
//! One/InvSrcAlpha over the environment layer with depth GreaterEqual, no depth write, NoCull.
//!
//! WHAT THE CAPTURE SAYS (pwc-day, frame 127448, direction (0.345, 0.117, 0.931)): of the 16 226 cloud
//! triangles' post-VS positions, 568 have z01 in [0, 1], 11 overlap the viewport in x/y, NONE both — the
//! sprites sit 1.9–4.8 km up, tiled every 16 km over ±64 km (VisualToWorld translations), while the peel
//! frustum is the world box's orthographic projection (y ≤ ~570 m) with DepthClip on: zero fragments in
//! every peel of this map. PS 14515 is therefore unexercised here (not transcribed); the VS is, so a map
//! whose peel reaches the cloud layer is covered and the clip test can be run for any direction
//! (`lmtool clouds-check PASSCAP [--frame F] [--dir-eid E]`).
//!
//! VS 14514, instruction by instruction (Vertex_14514.txt), inputs per the input layout (stride 28):
//! v0 = TEXCOORD0 float4 (the sprite centre in visual space, w = its size), v1 = TEXCOORD1 snorm16×4
//! (xy = the SIGNED atlas size — the signs pick the corner — zw = the atlas offset), v2 = TEXCOORD2
//! snorm16×2 (x = opacity, y = aspect: negative = 1/|aspect|). Constants: SceneV (WorldPrCamera,
//! WorldToCamera, EyeInWorld, FogClouds_DepthS_Exp_OutS), DrawV (GbxClouds3dInst0: VisualToWorld,
//! VisualMadBBox01, VisualToOpacity, VortexXZinW_Intens_Free1; GbxSpriteExpandA: AxeXinV_HalfNegPivotX,
//! AxeYinV_HalfNegPivotY, GlobalDir_Branch, CameraZCrossDir_IsRadial, EyeInVisual_Free; LightningPosW),
//! ShaderV (GbxLightDirAngle_m11Zx). Matrices are read as the shader reads them: storage float4 k =
//! column k of the JSON matrix (`dp4 o.x, p, M[0]` = Σ p_i·M[i][0]).
//!
//! Outputs: o0 = the clip position, o1 = (atlas uv, a height/elevation fade × the vertical sign, the
//! vertical sign), o2 = (world − LightningPosW, the fog factor), o3 = (world − EyeInWorld, the opacity),
//! o4 = (a unit vector: normalize(cross(WorldToCamera column 1 (.xyz), world − eye)), the folded sun
//! azimuth term |atan2(view.x, view.z)/π − LightDirAngle| mirrored past 1).

use serde_json::Value;

/// A 4×4 matrix as the JSON holds it (rows); the shader's storage float4 k is column k.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mat4(pub [[f32; 4]; 4]);

impl Mat4 {
    /// The shader's `M[k]` (storage float4 k = column k).
    #[inline]
    pub fn col(&self, k: usize) -> [f32; 4] {
        [self.0[0][k], self.0[1][k], self.0[2][k], self.0[3][k]]
    }
    /// `dp4(p, M[k])` for k = 0..3 (p a float4) — evaluated as the GPU does: a multiply then three FUSED
    /// multiply-adds in component order (the capture's o0 is bit-identical with this chain, see `check`).
    #[inline]
    pub fn mul4(&self, p: [f32; 4]) -> [f32; 4] {
        let mut o = [0f32; 4];
        for k in 0..4 {
            let c = self.col(k);
            o[k] = dp4(p, c);
        }
        o
    }
    /// A float4x3 (4 JSON rows of 3): the shader's `M[k]` for k = 0..2 as float4 columns.
    pub fn from_rows3(rows: &[[f32; 3]; 4]) -> Mat4 {
        let mut m = [[0f32; 4]; 4];
        for i in 0..4 {
            for k in 0..3 {
                m[i][k] = rows[i][k];
            }
        }
        m[3][3] = 1.0;
        Mat4(m)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CloudConstants {
    pub world_pr_camera: Mat4,
    /// float4x3: columns 0..2 used (`WorldToCamera[1].xyz` = the camera's Y axis in world).
    pub world_to_camera: Mat4,
    pub eye_in_world: [f32; 3],
    pub fog_clouds: [f32; 4],
    pub visual_to_world: Mat4,
    pub visual_mad_bbox: [[f32; 3]; 2],
    pub visual_to_opacity: [f32; 4],
    pub vortex: [f32; 4],
    pub axe_x_half_neg_pivot_x: [f32; 4],
    pub axe_y_half_neg_pivot_y: [f32; 4],
    pub global_dir_branch: [f32; 4],
    pub camera_z_cross_dir_is_radial: [f32; 4],
    pub eye_in_visual: [f32; 3],
    pub lightning_pos: [f32; 3],
    pub light_dir_angle: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CloudVertex {
    /// TEXCOORD0: the centre in visual space and the size.
    pub v0: [f32; 4],
    /// TEXCOORD1 (snorm16 decoded): the signed atlas size and the offset.
    pub v1: [f32; 4],
    /// TEXCOORD2 (snorm16 decoded): opacity, aspect.
    pub v2: [f32; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CloudVsOut {
    pub o0: [f32; 4],
    pub o1: [f32; 4],
    pub o2: [f32; 4],
    pub o3: [f32; 4],
    pub o4: [f32; 4],
}

/// The GPU's dp4: `x·a`, then fused multiply-adds for y, z, w (one rounding each).
#[inline]
pub fn dp4(p: [f32; 4], c: [f32; 4]) -> f32 {
    let mut r = p[0] * c[0];
    r = p[1].mul_add(c[1], r);
    r = p[2].mul_add(c[2], r);
    p[3].mul_add(c[3], r)
}
/// The GPU's dp3 (the same chain, three components).
#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let mut r = a[0] * b[0];
    r = a[1].mul_add(b[1], r);
    a[2].mul_add(b[2], r)
}
/// The GPU's `mad`: fused.
#[inline]
fn mad(a: f32, b: f32, c: f32) -> f32 {
    a.mul_add(b, c)
}
#[inline]
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline]
fn scale3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
#[inline]
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
#[inline]
fn xyz(v: [f32; 4]) -> [f32; 3] {
    [v[0], v[1], v[2]]
}
/// `mul a.yzx, b.zxy; mad a.zxy, b.yzx, -that` = cross(a, b) in the DXBC's spelling.
#[inline]
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
#[inline]
fn rsq(x: f32) -> f32 {
    1.0 / x.sqrt()
}
#[inline]
fn sat(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// VS 14514 on one vertex.
pub fn vs_14514(v: &CloudVertex, k: &CloudConstants) -> CloudVsOut {
    let (v0, v1, v2) = (v.v0, v.v1, v.v2);
    let axe_x = xyz(k.axe_x_half_neg_pivot_x);
    let axe_y = xyz(k.axe_y_half_neg_pivot_y);
    let gdir = xyz(k.global_dir_branch);
    let branch = k.global_dir_branch[3];
    let czc = xyz(k.camera_z_cross_dir_is_radial);
    let is_radial = k.camera_z_cross_dir_is_radial[3];
    // 0–3: the half-width factor: aspect (or 1/|aspect| when negative) × size
    let r0x = (if v2[1] >= 0.0 { v2[1] } else { -1.0 / v2[1] }) * v0[3];
    // 4: the radial sprites (IsRadial) or the camera-facing ones
    let (r2, r3): ([f32; 3], [f32; 3]) = if 0.5 < is_radial {
        // 6–9: r1 = normalize((v0 − EyeInVisual).zxy) — the DXBC keeps the cyclic permutation through the
        // branch; the results below are cross products spelled in the same permuted swizzles, so the
        // permutation cancels: written here in plain xyz
        let r1 = sub3(xyz(v0), k.eye_in_visual);
        let r1 = scale3(r1, rsq(dot3(r1, r1)));
        if 0.5 < branch {
            // 12–17: r2 = normalize(cross(GlobalDir, r1)), r3 = GlobalDir
            let c = cross3(gdir, r1);
            (scale3(c, rsq(dot3(c, c))), gdir)
        } else if branch < -0.01 {
            // 21–43: the bent axis
            let c = cross3(gdir, r1);
            let r2 = scale3(c, rsq(dot3(c, c)));
            let r4 = sub3(k.eye_in_visual, xyz(v0));
            let r5 = scale3(r4, rsq(dot3(r4, r4)));
            let r0z = dot3(r5, gdir);
            let r0w = v0[3] + v0[3];
            let r1w = dot3(r4, gdir);
            let r2w = r1w < r0w;
            let t = sat(r1w / r0w) * r0z.abs();
            let r0z = (if r2w { t } else { r0z.abs() }) * branch;
            let r0w = (1.0 - r0z * r0z).sqrt();
            // 40–41: r4 = −cross(r2, GlobalDir)  (mul −r2.zxy, g.yzx; mad −r2.yzx, g.zxy, −that)
            let r4 = scale3(cross3(r2, gdir), -1.0);
            let r4 = scale3(r4, r0z);
            (r2, add3(scale3(gdir, r0w), r4))
        } else {
            // 45–62: an axis pair from the view direction's dominant component
            let r0z = r1[1].abs() < r1[2].abs();
            let r4 = [r1[0], 0.0, 0.0];
            // 47: r5 = −r1 × (0,0,1) + r4.zxy = (−0·r1.x + r4.z, −0·r1.y + r4.x, −1·r1.z + r4.y) = (0, r1.x, −r1.z)
            let r5 = [0.0 + r4[2], 0.0 + r4[0], -r1[2] + r4[1]];
            let r5 = scale3(r5, rsq(r5[1] * r5[1] + r5[2] * r5[2]));
            let r5 = if r5[1] < 0.0 { scale3(r5, -1.0) } else { r5 };
            // 53–54: r6 = cross(r5, r1)
            let r6 = cross3(r5, r1);
            // 55: r4 = −r1.zxy × (0,0,1) + r4 = (r4.x, r4.y, −r1.y)
            let r4 = [r4[0], r4[1], -r1[1]];
            let r4 = scale3(r4, rsq(r4[0] * r4[0] + r4[2] * r4[2]));
            // 59–60: r1' = cross(r4, r1)
            let r1b = cross3(r4, r1);
            (if r0z { r6 } else { r4 }, if r0z { r5 } else { r1b })
        }
    } else {
        // 66–89: the camera-facing sprites
        let r0z = 0.5 < branch;
        let r0w = branch < -0.01;
        let r1 = sub3(k.eye_in_visual, xyz(v0));
        let r4 = scale3(r1, rsq(dot3(r1, r1)));
        let r1w = dot3(r4, gdir);
        let r2w = v0[3] + v0[3];
        let r1x = dot3(r1, gdir);
        let r1y = r1x < r2w;
        let t = sat(r1x / r2w) * r1w.abs();
        let r1x = (if r1y { t } else { r1w.abs() }) * branch;
        let r1y = (1.0 - r1x * r1x).sqrt();
        // 82–83: r4 = cross(CameraZCrossDir, GlobalDir)
        let r4 = cross3(czc, gdir);
        let r1v = add3(scale3(gdir, r1y), scale3(r4, r1x));
        let r1v = if r0w { r1v } else { axe_y };
        let either = r0z || r0w;
        let r2 = if either { scale3(czc, -1.0) } else { axe_x };
        let r3 = if r0z { gdir } else { r1v };
        (r2, r3)
    };
    // 91–92
    let r1 = scale3(r3, v0[3]);
    let r0 = scale3(r2, r0x);
    // 93–98: the corner from the atlas size's signs
    let sgn = |x: f32| -> f32 { (if 0.0 < x { 1.0 } else { 0.0 }) - (if x < 0.0 { 1.0 } else { 0.0 }) };
    let (sx, sy) = (sgn(v1[0]), sgn(v1[1]));
    let (r2z, r2w) = (sx * 0.5, sy * 0.5);
    let (r3x, r3y) = (sx * 0.5 + 0.5, sy * 0.5 + 0.5);
    // 99–102: the corner in visual space
    let mad3 = |a: [f32; 3], s: f32, c: [f32; 3]| [mad(a[0], s, c[0]), mad(a[1], s, c[1]), mad(a[2], s, c[2])];
    let r4 = mad3(r0, k.axe_x_half_neg_pivot_x[3], xyz(v0));
    let r4 = mad3(r1, k.axe_y_half_neg_pivot_y[3], r4);
    let r0 = mad3(r0, r2z, r4);
    let corner = mad3(r1, r2w, r0);
    // 103–104: the atlas uv
    let r3z = 1.0 - r3x;
    let o1xy = [mad(r3z, v1[0].abs(), v1[2]), mad(r3y, v1[1].abs(), v1[3])];
    // 105–113: world and clip
    let cw = [corner[0], corner[1], corner[2], 1.0];
    let w4 = k.visual_to_world.mul4(cw);
    let world = [w4[0], w4[1], w4[2]];
    let clip = k.world_pr_camera.mul4([world[0], world[1], world[2], 1.0]);
    // 114–130: the elevation fade (o1.z)
    let r5 = sub3(world, k.eye_in_world);
    let r0x_inv = rsq(dot3(r5, r5));
    let c1 = k.world_pr_camera.col(1);
    let r0z = (v0[3] * c1[1] + 1.0 * c1[3]) / clip[3];
    let r0w = r3x * 2.0 - 1.0;
    let o1w = r3y * 2.0 - 1.0;
    let r0z = sat((r0z - 0.15) * 2.857143) * 0.8 + 0.2;
    let r0xf = sat((r5[1] * r0x_inv - 0.2) * 2.666667);
    let mx = r0xf.max(r0z);
    let o1z = (if 0.5 < is_radial { mx } else { r0z }) * r0w;
    // 131–139: the bbox fade × opacity plane
    let b = k.visual_mad_bbox;
    let q = [corner[0] * b[0][0] + b[1][0], corner[1] * b[0][1] + b[1][1], corner[2] * b[0][2] + b[1][2]];
    let q = [q[0] * 2.0 - 1.0, q[1] * 2.0 - 1.0, q[2] * 2.0 - 1.0];
    let r0x = dot3(q, q).sqrt();
    let vo = k.visual_to_opacity;
    let r0y = sat(corner[0] * vo[0] + corner[1] * vo[1] + corner[2] * vo[2] + vo[3]) - 1.0;
    let fade = (r0y * (r0x + 1.0) + 1.0).max(0.0);
    // 140–168: the azimuth of the view vector (an atan2 polynomial) → o4.w
    let r0y_len = (r5[0] * r5[0] + r5[2] * r5[2]).sqrt();
    let mn = r5[2].abs().min(r5[0].abs());
    let mxa = r5[2].abs().max(r5[0].abs());
    let inv = 1.0 / mxa;
    let r0z = inv * mn;
    let r0w = r0z * r0z;
    let mut p = r0w * 0.020835 + -0.085133;
    p = r0w * p + 0.180141;
    p = r0w * p + -0.330299;
    let r0w = r0w * p + 0.999866;
    let r1x = r0w * r0z;
    let r1y = r5[2].abs() < r5[0].abs();
    let r1x = if r1y { r1x * -2.0 + 1.570796 } else { 0.0 };
    let r0z = r0z * r0w + r1x;
    let r0w = if r5[2] < -r5[2] { -3.141593 } else { 0.0 };
    let r0z = r0w + r0z;
    let r0w = r5[2].min(r5[0]);
    let r1x_b = r5[2].max(r5[0]);
    let neg = (r0w < -r0w) && (r1x_b >= -r1x_b);
    let r0z = if neg { -r0z } else { r0z };
    let r0z = r0z * 0.318354 - k.light_dir_angle;
    let o4w = if 1.0 < r0z.abs() { 2.0 - r0z.abs() } else { r0z.abs() };
    // 169–176: o2
    let o2xyz = sub3(world, k.lightning_pos);
    let fc = k.fog_clouds;
    let r0y = sat(r0y_len * fc[0]);
    let r0y = (r0y.ln() * fc[1]).exp();
    let r0y = -r0y * fc[2] + 1.0;
    let o2w = (1.0 - fc[3]) * r0y;
    // 177: the opacity
    let r5w = fade * v2[0];
    // 178–183: o4.xyz = normalize(cross(WorldToCamera[1].xyz, r5) + WorldToCamera[2].z × 0)
    let cy = xyz(k.world_to_camera.col(1));
    let c = cross3(cy, r5);
    let o4xyz = scale3(c, rsq(dot3(c, c)));
    // 184–202: the vortex (off when Intens ≤ 0): the sprite collapses to (0, 0, −1, 1) below 0.001 opacity
    let (o0, o3w) = if 0.0 < k.vortex[2] {
        let vw = k.visual_to_world.mul4([v0[0], v0[1], v0[2], 1.0]);
        let dx = vw[0] - k.vortex[0];
        let dz = vw[2] - k.vortex[1];
        let r = (dx * dx + dz * dz).sqrt();
        let f = sat((r - 4000.0) * 0.01) - 1.0;
        let f = k.vortex[2] * f + 1.0;
        let w = f * r5w;
        (if w < 0.001 { [0.0, 0.0, -1.0, 1.0] } else { clip }, w)
    } else {
        (clip, r5w)
    };
    CloudVsOut { o0, o1: [o1xy[0], o1xy[1], o1z, o1w], o2: [o2xyz[0], o2xyz[1], o2xyz[2], o2w], o3: [r5[0], r5[1], r5[2], o3w], o4: [o4xyz[0], o4xyz[1], o4xyz[2], o4w] }
}

/// snorm16 → float as D3D decodes it (v / 32767, clamped at −1).
#[inline]
pub fn snorm16(v: i16) -> f32 {
    (v as f32 / 32767.0).max(-1.0)
}

/// The vertices of a cloud vertex buffer (stride 28: float4, snorm16×4, snorm16×2).
pub fn parse_vertices(vb: &[u8]) -> Vec<CloudVertex> {
    vb.chunks_exact(28)
        .map(|c| {
            let f = |o: usize| f32::from_le_bytes(c[o..o + 4].try_into().unwrap());
            let s = |o: usize| snorm16(i16::from_le_bytes([c[o], c[o + 1]]));
            CloudVertex { v0: [f(0), f(4), f(8), f(12)], v1: [s(16), s(18), s(20), s(22)], v2: [s(24), s(26)] }
        })
        .collect()
}

fn f4(v: &Value) -> [f32; 4] {
    let a = v.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect::<Vec<_>>()).unwrap_or_default();
    [a.first().copied().unwrap_or(0.0), a.get(1).copied().unwrap_or(0.0), a.get(2).copied().unwrap_or(0.0), a.get(3).copied().unwrap_or(0.0)]
}
fn f3(v: &Value) -> [f32; 3] {
    let a = f4(v);
    [a[0], a[1], a[2]]
}
fn mat_rows(v: &Value) -> Mat4 {
    let rows: Vec<Vec<f32>> = v.as_array().map(|r| r.iter().map(|row| row.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default()).collect()).unwrap_or_default();
    let mut m = [[0f32; 4]; 4];
    for (i, r) in rows.iter().enumerate().take(4) {
        for (k, x) in r.iter().enumerate().take(4) {
            m[i][k] = *x;
        }
    }
    if rows.first().map(|r| r.len()).unwrap_or(4) == 3 {
        m[3][3] = 1.0;
    }
    Mat4(m)
}

/// The constants of one cloud draw from the capture's draw log entry.
pub fn constants_of(draw: &Value) -> CloudConstants {
    let cb = &draw["Vertex"]["cbuffers"];
    let sv = &cb["SceneV"];
    let dv = &cb["DrawV"];
    let inst = &dv["GbxClouds3dInst0"];
    let sp = &dv["GbxSpriteExpandA"];
    let mm = |v: &Value| -> [[f32; 3]; 2] {
        // "VisualMadBBox01": {"VisualMadBBox01[0]": [...], "VisualMadBBox01[1]": [...]}
        let a = f3(&v["VisualMadBBox01[0]"]);
        let b = f3(&v["VisualMadBBox01[1]"]);
        [a, b]
    };
    CloudConstants {
        world_pr_camera: mat_rows(&sv["GbxV_WorldPrCamera"]),
        world_to_camera: mat_rows(&sv["GbxV_WorldToCamera"]),
        eye_in_world: f3(&sv["GbxV_EyeInWorld"]),
        fog_clouds: f4(&sv["GbxV_FogClouds_DepthS_Exp_OutS"]),
        visual_to_world: mat_rows(&inst["VisualToWorld"]),
        visual_mad_bbox: mm(&inst["VisualMadBBox01"]),
        visual_to_opacity: f4(&inst["VisualToOpacity"]),
        vortex: f4(&inst["VortexXZinW_Intens_Free1"]),
        axe_x_half_neg_pivot_x: f4(&sp["AxeXinV_HalfNegPivotX"]),
        axe_y_half_neg_pivot_y: f4(&sp["AxeYinV_HalfNegPivotY"]),
        global_dir_branch: f4(&sp["GlobalDir_Branch"]),
        camera_z_cross_dir_is_radial: f4(&sp["CameraZCrossDir_IsRadial"]),
        eye_in_visual: f3(&sp["EyeInVisual_Free"]),
        lightning_pos: f3(&dv["LightningPosW"]),
        light_dir_angle: cb["ShaderV"]["GbxLightDirAngle_m11Zx"].as_f64().unwrap_or(0.0) as f32,
    }
}

/// `lmtool clouds-check PASSCAP [--frame F] [--max-draws N] [--verbose]`: every cloud draw of the frame's
/// log — the VS on the banked vertex buffer against the banked post-VS vertices (max |Δ| per output, the
/// count bit-identical), and the clip test of its triangles against the peel viewport (z01 in [0, 1] and
/// x/y inside (1, 1, 4094, 4094)).
pub fn check(args: &[String]) -> Result<(), String> {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let root = std::path::PathBuf::from(&args[1]);
    let frame: u32 = f("--frame").map(|v| v.parse().unwrap()).unwrap_or(127448);
    let max_draws: usize = f("--max-draws").map(|v| v.parse().unwrap()).unwrap_or(usize::MAX);
    let verbose = args.iter().any(|x| x == "--verbose");
    let draws = crate::lmaccum::load_draws(&root, frame)?;
    let env = root.join(format!("env/frame{frame}"));
    let mesh: Value = serde_json::from_str(&std::fs::read_to_string(env.join("mesh.json")).map_err(|e| format!("mesh.json: {e}"))?).map_err(|e| format!("mesh.json: {e}"))?;
    let cloud_draws: Vec<&Value> = draws.iter().filter(|d| d["Vertex"]["shader"].as_str() == Some("14514")).collect();
    println!("frame {frame}: {} cloud draws (VS 14514)", cloud_draws.len());
    let mut n_draws = 0usize;
    let (mut n_verts, mut n_exact, mut n_tris, mut n_z, mut n_xy, mut n_both) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut worst = [0f32; 5];
    let mut worst_rel = [0f32; 5];
    let mut exact_comp = [[0usize; 4]; 5];
    let mut missing = 0usize;
    for d in cloud_draws.iter().take(max_draws) {
        let eid = d["eid"].as_u64().unwrap_or(0);
        let Some(rec) = mesh.as_array().and_then(|a| a.iter().find(|r| r["eid"].as_u64() == Some(eid))) else { missing += 1; continue };
        let Some(vb_file) = rec["vertex_buffers"].as_array().and_then(|a| a.first()).and_then(|v| v["file"].as_str()) else { missing += 1; continue };
        let vb = std::fs::read(env.join("mesh").join(vb_file)).map_err(|e| format!("{vb_file}: {e}"))?;
        let Some(vsout_file) = rec["vsout"]["file"].as_str() else { missing += 1; continue };
        let vsout = std::fs::read(env.join("mesh").join(vsout_file)).map_err(|e| format!("{vsout_file}: {e}"))?;
        let Some(idx_file) = rec["vsout"]["index_file"].as_str() else { missing += 1; continue };
        let ib = std::fs::read(env.join("mesh").join(idx_file)).map_err(|e| format!("{idx_file}: {e}"))?;
        let indices: Vec<u16> = ib.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let k = constants_of(d);
        let verts = parse_vertices(&vb);
        let game: Vec<[f32; 20]> = vsout.chunks_exact(80).map(|c| { let mut o = [0f32; 20]; for i in 0..20 { o[i] = f32::from_le_bytes(c[i * 4..i * 4 + 4].try_into().unwrap()); } o }).collect();
        let n = verts.len().min(game.len());
        let mut ours: Vec<CloudVsOut> = Vec::with_capacity(n);
        for i in 0..n {
            let o = vs_14514(&verts[i], &k);
            ours.push(o);
            let flat = [o.o0, o.o1, o.o2, o.o3, o.o4];
            let mut exact = true;
            for (j, out) in flat.iter().enumerate() {
                for c in 0..4 {
                    let g = game[i][j * 4 + c];
                    let dlt = (out[c] - g).abs();
                    if out[c].to_bits() != g.to_bits() { exact = false; } else { exact_comp[j][c] += 1; }
                    if dlt > worst[j] { worst[j] = dlt; }
                    let rel = dlt / g.abs().max(1e-3);
                    if rel > worst_rel[j] { worst_rel[j] = rel; }
                    if verbose && dlt > 1e-3 * g.abs().max(1.0) { println!("  eid {eid} vertex {i} o{j}.{c}: ours {} game {g}", out[c]); }
                }
            }
            n_verts += 1;
            if exact { n_exact += 1; }
        }
        // the clip test on the game's own post-VS positions (o0): z01 = z/w in [0, 1], the viewport in x/y
        for t in indices.chunks_exact(3) {
            let p: Vec<[f32; 4]> = t.iter().map(|&i| { let g = &game[i as usize]; [g[0], g[1], g[2], g[3]] }).collect();
            let z01: Vec<f32> = p.iter().map(|q| q[2] / q[3]).collect();
            let zin = z01.iter().any(|z| *z >= 0.0 && *z <= 1.0) || (z01.iter().any(|z| *z < 0.0) && z01.iter().any(|z| *z > 1.0));
            let xs: Vec<f32> = p.iter().map(|q| q[0] / q[3]).collect();
            let ys: Vec<f32> = p.iter().map(|q| q[1] / q[3]).collect();
            let xy_in = xs.iter().cloned().fold(f32::MAX, f32::min) <= 1.0 && xs.iter().cloned().fold(f32::MIN, f32::max) >= -1.0 && ys.iter().cloned().fold(f32::MAX, f32::min) <= 1.0 && ys.iter().cloned().fold(f32::MIN, f32::max) >= -1.0;
            n_tris += 1;
            if zin { n_z += 1; }
            if xy_in { n_xy += 1; }
            if zin && xy_in { n_both += 1; }
        }
        n_draws += 1;
    }
    println!("{n_draws} draws checked ({missing} without banked buffers): {n_verts} vertices, {n_exact} bit-identical on all 20 outputs ({:.3} %)", 100.0 * n_exact as f64 / n_verts.max(1) as f64);
    for (j, name) in ["o0 clip", "o1 uv/fade", "o2 lightning/fog", "o3 view/opacity", "o4 axis/sun-angle"].iter().enumerate() {
        println!("  {name}: max |Δ| {:.3e} (relative {:.3e}); bit-identical per component {}", worst[j], worst_rel[j], (0..4).map(|c| format!("{:.1} %", 100.0 * exact_comp[j][c] as f64 / n_verts.max(1) as f64)).collect::<Vec<_>>().join(" / "));
    }
    println!("clip test on the captured positions: {n_tris} triangles, {n_z} with depth in the frustum, {n_xy} overlapping the viewport in x/y, {n_both} both → {} fragments", if n_both == 0 { "ZERO" } else { "some" });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camera_facing_sprite_expands_to_its_four_corners() {
        // AxeX = +x, AxeY = +y, no pivot, camera looking down −z: a sprite at the origin of size 2 with aspect 1
        let mut k = CloudConstants::default();
        k.axe_x_half_neg_pivot_x = [1.0, 0.0, 0.0, 0.0];
        k.axe_y_half_neg_pivot_y = [0.0, 1.0, 0.0, 0.0];
        k.global_dir_branch = [0.0, 1.0, 0.0, 0.0];
        k.eye_in_visual = [0.0, 0.0, 10.0];
        k.visual_to_world = Mat4([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [100.0, 0.0, 0.0, 1.0]]);
        k.world_pr_camera = Mat4([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        k.world_to_camera = Mat4([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        k.eye_in_world = [100.0, 0.0, 10.0];
        let mk = |sx: f32, sy: f32| CloudVertex { v0: [0.0, 0.0, 0.0, 2.0], v1: [sx * 0.25, sy * 0.25, 0.5, 0.5], v2: [1.0, 1.0] };
        let a = vs_14514(&mk(-1.0, -1.0), &k);
        let b = vs_14514(&mk(1.0, 1.0), &k);
        // the corners: centre ± 0.5·size along each axis, then + the visual→world translation
        assert!((a.o0[0] - 99.0).abs() < 1e-5 && (a.o0[1] + 1.0).abs() < 1e-5, "{:?}", a.o0);
        assert!((b.o0[0] - 101.0).abs() < 1e-5 && (b.o0[1] - 1.0).abs() < 1e-5, "{:?}", b.o0);
        // the uv: the negative-sign corner reads (1 − 0)·|size| + offset = 0.75, the positive one 0.5 + 0 … : (1−1)·0.25 + 0.5
        assert!((a.o1[0] - 0.75).abs() < 1e-6 && (a.o1[1] - 0.5).abs() < 1e-6, "{:?}", a.o1);
        assert!((b.o1[0] - 0.5).abs() < 1e-6 && (b.o1[1] - 0.75).abs() < 1e-6, "{:?}", b.o1);
        // the aspect: a negative aspect −2 → width factor 1/2
        let c = vs_14514(&CloudVertex { v0: [0.0, 0.0, 0.0, 2.0], v1: [0.25, 0.25, 0.0, 0.0], v2: [1.0, -2.0] }, &k);
        assert!((c.o0[0] - 100.5).abs() < 1e-5, "{:?}", c.o0);
    }

    #[test]
    fn the_matrix_reads_columns_like_the_shader() {
        let m = Mat4([[1.0, 2.0, 3.0, 0.0], [4.0, 5.0, 6.0, 0.0], [7.0, 8.0, 9.0, 0.0], [10.0, 20.0, 30.0, 1.0]]);
        assert_eq!(m.col(0), [1.0, 4.0, 7.0, 10.0]);
        let p = m.mul4([1.0, 1.0, 1.0, 1.0]);
        assert_eq!(p, [22.0, 35.0, 48.0, 1.0]);
    }
}
