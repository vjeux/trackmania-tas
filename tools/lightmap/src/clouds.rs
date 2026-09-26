//! THE CLOUD SPRITES OF THE PEEL'S ENVIRONMENT BLOCK (row 5, port engineer D): 177 draws per direction of
//! VS 14514 / PS 14515 (frame 127448 eids 1101 … 2685 step 9), camera-facing billboards blended
//! One/InvSrcAlpha over the environment layer with depth GreaterEqual, no depth write, NoCull.
//!
//! WHAT THE CAPTURE SAYS (pwc-day, frame 127448, direction (0.345, 0.117, 0.931)): of the 16 226 cloud
//! triangles' post-VS positions, 568 have z01 in [0, 1], 11 overlap the viewport in x/y, NONE both — the
//! instances tile every 16 km over ±64 km at y 2.1–3.0 km (VisualToWorld translations; the sprites' centres
//! 574–5 706 m, half extents up to 2.2 km), while the peel frustum is the world box's orthographic
//! projection (its highest point 1 588 m over every direction of the q4 sets on pwc-day's box, top 138 m)
//! with DepthClip on: zero fragments in every peel of this map. PS 14515 is therefore unexercised here (not
//! transcribed); the VS is, so a map whose peel reaches the cloud layer is covered and the clip test can be
//! run for any direction (`lmtool clouds-check PASSCAP [--frame F] [--dir-eid E]`). `lmtool clouds-reach`
//! (port engineer G) runs the frustum test for EVERY direction of the game's sets with the sprites expanded
//! for each peel camera: 0 fragments on pwc-day's box (least separation 245 m), the first fragment at a world
//! box top of ≈ 470–500 m (docs/formats/lightmapper-client.md §6e). Stadium's peel has no environment block
//! (stpad f4788), so no cloud draw at all.
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

/// One cloud sprite in WORLD space as the peel's orthographic camera expands it (port engineer G): VS 14514's
/// camera-facing branch with the capture's constants (GlobalDir_Branch.w = 0, IsRadial = 0, pivots −0, vortex
/// off, VisualToWorld = a pure translation — all 177 draws of frame 127448): the corner = centre + R·(a·size)·(±½)
/// + U·size·(±½) with R = AxeXinV = the camera's right axis, U = AxeYinV = its up axis (`GbxV_WorldToCamera`
/// columns 0 and 1) and a = aspect (v2.y ≥ 0) or 1/|aspect|. The quad is perpendicular to the peel direction, so
/// its depth along D is the centre's.
#[derive(Clone, Copy, Debug)]
pub struct WorldSprite {
    pub centre: [f32; 3],
    /// half-width along R and half-height along U (metres)
    pub half_w: f32,
    pub half_h: f32,
    pub opacity: f32,
    pub draw_eid: u64,
}

/// The sprites of every cloud draw with a banked vertex buffer (each draw = one VisualToWorld translation).
pub fn world_sprites(root: &std::path::Path, frame: u32) -> Result<(Vec<WorldSprite>, usize), String> {
    let draws = crate::lmaccum::load_draws(root, frame)?;
    let env = root.join(format!("env/frame{frame}"));
    let mesh: Value = serde_json::from_str(&std::fs::read_to_string(env.join("mesh.json")).map_err(|e| format!("mesh.json: {e}"))?).map_err(|e| format!("mesh.json: {e}"))?;
    let mut out = Vec::new();
    let mut n_draws = 0usize;
    for d in draws.iter().filter(|d| d["Vertex"]["shader"].as_str() == Some("14514")) {
        let eid = d["eid"].as_u64().unwrap_or(0);
        let Some(rec) = mesh.as_array().and_then(|a| a.iter().find(|r| r["eid"].as_u64() == Some(eid))) else { continue };
        let Some(vb_file) = rec["vertex_buffers"].as_array().and_then(|a| a.first()).and_then(|v| v["file"].as_str()) else { continue };
        let vb = std::fs::read(env.join("mesh").join(vb_file)).map_err(|e| format!("{vb_file}: {e}"))?;
        let k = constants_of(d);
        // the branch the transcription takes for these constants — anything else is a different expansion
        if !(k.global_dir_branch[3].abs() <= 0.01 && k.camera_z_cross_dir_is_radial[3] < 0.5 && k.vortex[2] <= 0.0 && k.axe_x_half_neg_pivot_x[3] == 0.0 && k.axe_y_half_neg_pivot_y[3] == 0.0) {
            return Err(format!("eid {eid}: cloud constants outside the camera-facing/no-pivot/no-vortex case (branch {}, radial {}, vortex {}, pivots {} {})", k.global_dir_branch[3], k.camera_z_cross_dir_is_radial[3], k.vortex[2], k.axe_x_half_neg_pivot_x[3], k.axe_y_half_neg_pivot_y[3]));
        }
        let m = k.visual_to_world.0;
        if m[0] != [1.0, 0.0, 0.0, 0.0] || m[1] != [0.0, 1.0, 0.0, 0.0] || m[2] != [0.0, 0.0, 1.0, 0.0] {
            return Err(format!("eid {eid}: VisualToWorld is not a pure translation: {:?}", m));
        }
        let t = [m[3][0], m[3][1], m[3][2]];
        // the four corners of a sprite share v0/v2; one record per distinct centre
        let mut seen: Vec<[f32; 4]> = Vec::new();
        for v in parse_vertices(&vb) {
            if seen.iter().any(|s| *s == v.v0) {
                continue;
            }
            seen.push(v.v0);
            let a = if v.v2[1] >= 0.0 { v.v2[1] } else { -1.0 / v.v2[1] };
            out.push(WorldSprite { centre: add3(xyz(v.v0), t), half_w: 0.5 * a * v.v0[3], half_h: 0.5 * v.v0[3], opacity: v.v2[0], draw_eid: eid });
        }
        n_draws += 1;
    }
    Ok((out, n_draws))
}

/// A sprite against one peel camera: the separation (metres) on each light-space axis — 0 on every axis ⇔ the
/// quad meets the frustum (a fragment is possible). R/U: the quad's interval vs the frustum's; D: the centre's
/// depth vs [near, far] (DepthClip on; the quad has one depth).
pub fn sprite_separation(s: &WorldSprite, cam: &crate::lightcam::OrthoCamera) -> [f32; 3] {
    let rel = sub3(s.centre, cam.eye);
    let r = dot3(rel, cam.right);
    let u = dot3(rel, cam.up);
    let d = dot3(rel, cam.forward);
    let sep = |x: f32, c: f32, h: f32, half: f32| ((x - c).abs() - h - half).max(0.0);
    [sep(r, cam.c[0], cam.h[0], s.half_w), sep(u, cam.c[1], cam.h[1], s.half_h), sep(d, cam.c[2], cam.h[2], 0.0)]
}

/// The highest world y any point of the frustum reaches: eye.y + Σ_axis h_axis·|axis.y| (+ the depth centre's shift).
pub fn frustum_top_y(cam: &crate::lightcam::OrthoCamera) -> f32 {
    cam.eye[1] + cam.c[0] * cam.right[1] + cam.c[1] * cam.up[1] + cam.c[2] * cam.forward[1] + cam.h[0] * cam.right[1].abs() + cam.h[1] * cam.up[1].abs() + cam.h[2] * cam.forward[1].abs()
}

/// THE BAKE-TIME GUARD (coordinator, 2026-09-26): the cloud sprites of the BlueBay-family environment block are
/// UNTRANSCRIBED (PS 14515 has no captured fragment to check against) and the proof that none reaches a peel is
/// geometric — on pwc-day's cloud layout the least separation is 16.5 m once the world peel frustum's highest
/// point reaches 1 900 m and a sprite ENTERS at 1 950 m (`clouds-reach --scan-ymax`: box tops 450 / 500 m on the
/// 2048² footprint). A bake whose world box lets any direction's frustum top this line is therefore not covered
/// by the transcription; the guard returns that top so the caller can warn loudly. Stadium has no environment
/// block at all (stpad f4788) — the caller skips the check there.
pub const CLOUD_REACH_WARN_Y: f32 = 1850.0;

/// The highest world point any of `dirs`' world-peel frusta on `world` reaches, when it is over the line.
pub fn world_box_reaches_clouds(world: &crate::lightcam::Aabb, dirs: &[[f32; 3]], rules: &crate::lightcam::FitRules) -> Option<(f32, [f32; 3])> {
    let mut top = (f32::MIN, [0f32; 3]);
    for d in dirs {
        let t = frustum_top_y(&crate::lightcam::fit_camera(world, *d, rules));
        if t > top.0 {
            top = (t, *d);
        }
    }
    if top.0 >= CLOUD_REACH_WARN_Y { Some(top) } else { None }
}

/// `lmtool clouds-reach PASSCAP [--frame F] [--box xmin,ymin,zmin,xmax,ymax,zmax] [--quality Q] [--scan-ymax Y1,Y2,…]
/// [--verbose]`: can any cloud sprite of the capture's environment block produce a fragment in ANY peel of the bake?
/// For every direction of the game's sweep sets at quality Q (the rotated table sets in issue order, `dome::
/// sweep_directions`; q3 = 256 + 128, q4 = 1024 + 512 + 256 + 128) the WORLD peel camera is fitted to `--box`
/// (default: pwc-day's world box, `PeelBoxes::pwc_day`) by the transcribed `lightcam::fit_camera`, and every sprite
/// (the 177 draws' vertex buffers, expanded for THAT camera as VS 14514 does) is tested against the frustum in light
/// space. Prints per set: sprites inside, the least separation and its direction, the frustum's highest world y vs
/// the sprites' lowest corner. `--scan-ymax` repeats the count with the box's y max replaced by each value — the
/// height a scene would need before a cloud reaches a peel. The fitted peel's box is inside the world box, so a
/// world-peel miss covers it.
pub fn reach(args: &[String]) -> Result<(), String> {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let root = std::path::PathBuf::from(args.get(1).ok_or("usage: lmtool clouds-reach PASSCAP_DIR [--frame F] [--box xmin,ymin,zmin,xmax,ymax,zmax] [--quality Q] [--scan-ymax Y1,Y2,…] [--verbose] — PASSCAP_DIR = a capture directory (passcap/pwc-day: env/frame<F>/mesh.json + logs/draws-frame<F>.json.gz), not a .Map.Gbx; the world box is --box (default pwc-day's)")?);
    if !root.is_dir() {
        return Err(format!("{}: not a directory — clouds-reach wants the CAPTURE directory (passcap/pwc-day with env/frame<F>/ and logs/), not a map file; the map's world box goes in --box xmin,ymin,zmin,xmax,ymax,zmax (the bake prints it as \"world peel box\")", root.display()));
    }
    let frame: u32 = f("--frame").map(|v| v.parse().unwrap()).unwrap_or(127448);
    let quality: u32 = f("--quality").map(|v| v.parse().unwrap()).unwrap_or(4);
    let verbose = args.iter().any(|x| x == "--verbose");
    let mut boxw = crate::lightcam::PeelBoxes::pwc_day().world;
    if let Some(b) = f("--box") {
        let v: Vec<f32> = b.split(',').map(|x| x.trim().parse::<f32>().map_err(|e| format!("--box: {e}"))).collect::<Result<_, _>>()?;
        if v.len() != 6 {
            return Err("--box wants xmin,ymin,zmin,xmax,ymax,zmax".into());
        }
        boxw = crate::lightcam::Aabb { min: [v[0], v[1], v[2]], max: [v[3], v[4], v[5]] };
    }
    let scan: Vec<f32> = f("--scan-ymax").map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
    let (sprites, n_draws) = world_sprites(&root, frame)?;
    let sets = crate::dome::PointSets::load(&crate::dome::default_path())?;
    let rules = crate::lightcam::FitRules::default();
    let (mut cy_min, mut cy_max, mut sz_max) = (f32::MAX, f32::MIN, 0f32);
    for s in &sprites {
        cy_min = cy_min.min(s.centre[1]);
        cy_max = cy_max.max(s.centre[1]);
        sz_max = sz_max.max(s.half_h.max(s.half_w));
    }
    println!("frame {frame}: {n_draws} cloud draws with banked buffers, {} sprites; centres y {cy_min:.1} … {cy_max:.1} m, largest half extent {sz_max:.1} m", sprites.len());
    if verbose {
        // per draw: the instance translation's y, its sprites' centre-y range and largest extent, opacities
        let mut eids: Vec<u64> = sprites.iter().map(|s| s.draw_eid).collect();
        eids.dedup();
        for e in eids {
            let ss: Vec<&WorldSprite> = sprites.iter().filter(|s| s.draw_eid == e).collect();
            let (mut y0, mut y1, mut hmax, mut o0, mut o1, mut x0, mut x1, mut z0, mut z1) = (f32::MAX, f32::MIN, 0f32, f32::MAX, f32::MIN, f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for s in &ss {
                y0 = y0.min(s.centre[1]); y1 = y1.max(s.centre[1]); hmax = hmax.max(s.half_h.max(s.half_w)); o0 = o0.min(s.opacity); o1 = o1.max(s.opacity);
                x0 = x0.min(s.centre[0]); x1 = x1.max(s.centre[0]); z0 = z0.min(s.centre[2]); z1 = z1.max(s.centre[2]);
            }
            println!("  eid {e}: {} sprites, centres x {x0:.0}…{x1:.0} y {y0:.0}…{y1:.0} z {z0:.0}…{z1:.0}, largest half extent {hmax:.0} m, opacity {o0:.3}…{o1:.3}", ss.len());
        }
    }
    println!("world box {:?} … {:?} (top {} m), quality {quality} → sets {:?}", boxw.min, boxw.max, boxw.max[1], crate::dome::sweep_counts(quality));
    let run = |b: &crate::lightcam::Aabb, verbose: bool| -> Result<usize, String> {
        let mut total_inside = 0usize;
        for sweep in 0..crate::dome::sweep_counts(quality).len() {
            let dirs = crate::dome::sweep_directions(&sets, quality, sweep, false).ok_or("no direction set")?;
            let (mut n_inside, mut n_dirs_hit) = (0usize, 0usize);
            let mut least = (f32::MAX, [0f32; 3], 0usize, [0f32; 3]);
            let (mut top_y, mut top_dir) = (f32::MIN, [0f32; 3]);
            let mut bottom_y = f32::MAX;
            for d in &dirs {
                let cam = crate::lightcam::fit_camera(b, *d, &rules);
                let ty = frustum_top_y(&cam);
                if ty > top_y {
                    top_y = ty;
                    top_dir = *d;
                }
                let mut hit_here = 0usize;
                for (i, s) in sprites.iter().enumerate() {
                    // the quad's lowest corner for this camera
                    let low = s.centre[1] - s.half_h * cam.up[1].abs() - s.half_w * cam.right[1].abs();
                    bottom_y = bottom_y.min(low);
                    let sep = sprite_separation(s, &cam);
                    let m = sep[0].max(sep[1]).max(sep[2]);
                    if m < least.0 {
                        least = (m, *d, i, sep);
                    }
                    if m == 0.0 {
                        hit_here += 1;
                        if verbose {
                            println!("  HIT dir ({:.4},{:.4},{:.4}) sprite {i} (eid {}) centre ({:.0},{:.0},{:.0}) half {:.0}×{:.0} opacity {:.3}", d[0], d[1], d[2], s.draw_eid, s.centre[0], s.centre[1], s.centre[2], s.half_w, s.half_h, s.opacity);
                        }
                    }
                }
                if hit_here > 0 {
                    n_dirs_hit += 1;
                }
                n_inside += hit_here;
            }
            let s = &sprites[least.2];
            println!("  sweep {sweep} ({} dirs): sprites inside {n_inside} in {n_dirs_hit} directions; least separation {:.1} m (R {:.1} / U {:.1} / D {:.1}) at dir ({:.4},{:.4},{:.4}) for sprite {} (eid {}, centre ({:.0},{:.0},{:.0}), half {:.0}×{:.0}); frustum top y {top_y:.1} m at dir ({:.4},{:.4},{:.4}); lowest sprite corner {bottom_y:.1} m", dirs.len(), least.0, least.3[0], least.3[1], least.3[2], least.1[0], least.1[1], least.1[2], least.2, s.draw_eid, s.centre[0], s.centre[1], s.centre[2], s.half_w, s.half_h, top_dir[0], top_dir[1], top_dir[2]);
            total_inside += n_inside;
        }
        Ok(total_inside)
    };
    let total = run(&boxw, verbose)?;
    println!("→ {}", if total == 0 { "ZERO cloud fragments in every peel of every direction".to_string() } else { format!("{total} sprite–frustum overlaps") });
    for y in scan {
        let mut b = boxw;
        b.max[1] = y;
        println!("box y max {y}:");
        run(&b, false)?;
    }
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

    /// The frustum test of `clouds-reach` against the transcribed VS: a sprite's quad meets the peel frustum
    /// exactly when the VS 14514 corners (camera-facing branch, the peel camera's R/U as AxeX/AxeY) land in the
    /// clip volume — checked on a box looked at straight up and at 45°, with sprites just inside and just
    /// outside on each light-space axis.
    #[test]
    fn sprite_separation_agrees_with_the_vertex_shader_corners() {
        let b = crate::lightcam::Aabb { min: [0.0, 0.0, 0.0], max: [2048.0, 138.0, 2048.0] };
        let rules = crate::lightcam::FitRules::default();
        for d in [[0.0f32, 1.0, 0.0], [0.5102, 0.5858, -0.6297], [0.6185, -0.5848, 0.5249]] {
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let d = [d[0] / l, d[1] / l, d[2] / l];
            let cam = crate::lightcam::fit_camera(&b, d, &rules);
            // the VS constants for this camera: AxeX/AxeY = right/up, a pure translation VisualToWorld (zero here)
            let mut k = CloudConstants::default();
            k.axe_x_half_neg_pivot_x = [cam.right[0], cam.right[1], cam.right[2], 0.0];
            k.axe_y_half_neg_pivot_y = [cam.up[0], cam.up[1], cam.up[2], 0.0];
            k.global_dir_branch = [0.0, 1.0, 0.0, 0.0];
            k.visual_to_world = Mat4([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
            k.eye_in_visual = cam.eye;
            k.eye_in_world = cam.eye;
            k.world_pr_camera = Mat4(cam.world_pr_camera());
            k.world_to_camera = Mat4::from_rows3(&cam.world_to_camera());
            // sprites placed in light space: centre = eye + r·R + u·U + z·D, size s (aspect 1)
            let place = |r: f32, u: f32, z: f32, s: f32| -> WorldSprite {
                let c = [cam.eye[0] + r * cam.right[0] + u * cam.up[0] + z * cam.forward[0], cam.eye[1] + r * cam.right[1] + u * cam.up[1] + z * cam.forward[1], cam.eye[2] + r * cam.right[2] + u * cam.up[2] + z * cam.forward[2]];
                WorldSprite { centre: c, half_w: 0.5 * s, half_h: 0.5 * s, opacity: 1.0, draw_eid: 0 }
            };
            let (hr, hu, cz, hz) = (cam.h[0], cam.h[1], cam.c[2], cam.h[2]);
            let cases = [
                (place(0.0, 0.0, cz, 10.0), true),                 // dead centre
                (place(hr + 6.0, 0.0, cz, 10.0), false),           // 1 m past the right edge (half 5)
                (place(hr + 4.0, 0.0, cz, 10.0), true),            // 1 m inside it
                (place(0.0, -(hu + 6.0), cz, 10.0), false),        // below the bottom edge
                (place(0.0, 0.0, cz + hz + 1.0, 10.0), false),     // behind the far plane
                (place(0.0, 0.0, cz - hz - 1.0, 10.0), false),     // before the near plane
                (place(0.0, 0.0, cz + hz - 1.0, 10.0), true),      // just inside the far plane
            ];
            for (s, inside) in cases {
                let sep = sprite_separation(&s, &cam);
                let ours = sep.iter().all(|x| *x == 0.0);
                assert_eq!(ours, inside, "dir {d:?} sprite {s:?} sep {sep:?}");
                // the VS corners: the four (sx, sy) sign corners through vs_14514, then the clip test
                let mut any_in = false;
                let mut all_z = true;
                let (mut xs, mut ys) = (Vec::new(), Vec::new());
                for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    let v = CloudVertex { v0: [s.centre[0], s.centre[1], s.centre[2], 2.0 * s.half_h], v1: [sx * 0.25, sy * 0.25, 0.5, 0.5], v2: [1.0, 1.0] };
                    let o = vs_14514(&v, &k);
                    let (x, y, z) = (o.o0[0] / o.o0[3], o.o0[1] / o.o0[3], o.o0[2] / o.o0[3]);
                    xs.push(x);
                    ys.push(y);
                    if !(0.0..=1.0).contains(&z) {
                        all_z = false;
                    }
                    if (-1.0..=1.0).contains(&x) && (-1.0..=1.0).contains(&y) {
                        any_in = true;
                    }
                }
                // the quad is axis-aligned in NDC: it meets the viewport iff its x and y intervals overlap [−1, 1]
                let ov = |v: &Vec<f32>| v.iter().cloned().fold(f32::MAX, f32::min) <= 1.0 && v.iter().cloned().fold(f32::MIN, f32::max) >= -1.0;
                let vs_in = all_z && ov(&xs) && ov(&ys);
                let _ = any_in;
                assert_eq!(vs_in, inside, "VS corners disagree: dir {d:?} sprite {s:?} xs {xs:?} ys {ys:?}");
            }
        }
    }

    /// The bake-time guard on pwc-day's world box (top 138 m → the worst frustum tops out at 1 588 m, under the line)
    /// and on the same footprint raised to 500 m (→ 1 950 m, over it — where `clouds-reach` sees the first sprite).
    #[test]
    fn the_cloud_guard_trips_on_a_tall_world_box_only() {
        let rules = crate::lightcam::FitRules::default();
        let dirs = [[-0.5258f32, 0.6964, -0.4883], [0.5102, 0.5858, -0.6297], [0.0, 1.0, 0.0], [0.3455, 0.1171, 0.9311]];
        let dirs: Vec<[f32; 3]> = dirs.iter().map(|d| { let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(); [d[0] / l, d[1] / l, d[2] / l] }).collect();
        let low = crate::lightcam::PeelBoxes::pwc_day().world;
        assert!(world_box_reaches_clouds(&low, &dirs, &rules).is_none());
        let top = frustum_top_y(&crate::lightcam::fit_camera(&low, dirs[0], &rules));
        assert!((top - 1588.2).abs() < 1.0, "{top}");
        let mut tall = low;
        tall.max[1] = 500.0;
        let (t, d) = world_box_reaches_clouds(&tall, &dirs, &rules).expect("over the line");
        assert!((t - 1950.2).abs() < 1.0 && d == dirs[0], "{t} {d:?}");
    }
}
