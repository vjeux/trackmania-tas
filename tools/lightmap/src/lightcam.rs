//! The lightmapper's ORTHOGRAPHIC LIGHT CAMERAS on the CPU — the sun shadow camera and the two peel frusta per dome
//! direction — TRANSCRIBED against the capture's SceneV cbuffers (`GbxV_EyeInWorld`, `GbxV_WorldToCamera`,
//! `GbxV_CameraProjection`, `GbxV_Camera_MinZ_MaxZ_InvRange_HasDeferredZ`, `GbxV_WorldPrCamera`; passcap/pwc-day
//! frame 127448 eid 907/347 = the sun camera, eids ≥ 1000 = the peel cameras) and the per-direction
//! `WorldPw01Shadow` of the peel entries.
//!
//! THE FIT (CHmsVolumeShadow::UpdateFrustum 0x140a4b1f0 — the decompile read by RE child 5; the same machine for the
//! sun camera and for both peel phases, only the focus box vs+0x80 differs):
//!
//! * the light basis (FUN_140187d30): `right = normalize(cross(Y, D))` — the cross product in the component order
//!   (Y.y·D.z − Y.z·D.y, Y.z·D.x − Y.x·D.z, Y.x·D.y − Y.y·D.x), which leaves right.y = −0.0 as the cbuffer prints it,
//!   the normalisation as v · (1/sqrt(len²)) — `up = cross(D, right)`, `forward = D`; `GbxV_WorldToCamera`'s columns
//!   are (right, up, forward), its last row −(eye·right, eye·up, eye·forward);
//! * the eye = the focus box's centre (min + (max − min)·0.5 — the {centre, half} form; the box positions the light's
//!   Iso4); the light-space frustum {c = 0, h = |R|·h_box} (row-abs times the half extents, FUN_140185f70);
//! * SetFar(far + 5.0): near kept, cz = (near + far)·0.5, hz = (far − near)·0.5 (step (e): flag 0x10 of vs+0x2d0 with
//!   light+0x8c == 0 — every camera of the capture has far = −near + 5.0005);
//! * the expansion FUN_140a52ad0 / FUN_140a52a70 on the light-space corners: min_i −= |min_i|·1e-4, max_i += |max_i|·1e-4,
//!   then c = (max + min)·0.5, h = (max − min)·0.5 (the "×1.0001" of the extents);
//! * the matrices (FUN_140a4c9a0): `GbxV_Camera_MinZ_MaxZ_InvRange` = (cz − hz, cz + hz, 1/(hz + hz)); the projection
//!   FUN_140195960 mode 2 (reversed z): sx = 2/(hx + hx), sy = 2/(hy + hy), sz = −1/(hz + hz), oz = −sz·cz + 0.5, then
//!   the device's x mirror (FUN_1409dc650 flag bit 0) negates sx and P30; `GbxV_WorldPrCamera` = WorldToCamera ·
//!   Projection; the lookup matrix `WorldPw01Shadow` (the accumulate's cbuffer) = WorldToCamera · Projection
//!   (UNMIRRORED) · Bias with sx_b = −0.5·(w − 2)/w, sy_b = −0.5·(h − 2)/h, translation (0.5, 0.5, 0).
//!
//! The focus boxes (RenderLightIndirectDome / Peel, RE 5): the SUN camera's = computeParams+0xb0 = the scene box S
//! (the block records' union: on pwc-day the 4096 seabed tiles at y = 3.9999785 — the tile mesh's own vertex y — and
//! the three items, the wall's top at 95.5: S = [−1.5e-5, 2048] × [3.9999785, 95.5] × [7.6e-6, 2048]); the WORLD
//! peel's = S ∪ the zone / probe-grid box (on pwc-day y up to 138.0, x/z unchanged); the FITTED peel's = the tile
//! record of the tiling rule (FUN_140230080: n×n cells over the box's x/z, each cell the union of the item records
//! with quality > 0.51 clipped to the cell, y = the box's y range) — on pwc-day one cell: x ∈ [861.01, 880.0], z ∈
//! [336.98, 369.0] (the RECORDS' boxes = the models' own, the wall's ±0.02 thickness) with S's y.
//!
//! Verified (`lmtool frustum-check`, frame 127448, 55 cbuffer values per camera): the sun camera from the capture's
//! own caster geometry — ALL 55 BIT-IDENTICAL; the world peel of D (0.445, 0.293, 0.846) (eid 7272) with y max 138 —
//! ALL 55 BIT-IDENTICAL; the world peel of direction 0 (eid 1028) — 44 identical, 11 within 2 ulps (the basis of
//! that one direction rounds differently from every normalisation tried: the game's 1/√ is probably the SSE
//! `rsqrtss` + Newton approximation, CPU-specific in its last bit), its accumulate's WorldPw01Shadow 13 of 16
//! bit-identical, 3 within 1 ulp; the fitted peel of direction 0 with the records' box — within 3 ulps (the box's
//! last bits are back-solved: the models' stored bounding boxes are the missing input, not in the GPU capture).
//!
//! `lmtool frustum-check ROOT [--manifest FROZEN.json] [--f-box xmin,zmin,xmax,zmax] [--world-ymax Y] [--norm …]`
//! recomputes every distinct camera of the draws log from these rules and prints each cbuffer value against the
//! captured one in f32 ulps; `peel_frusta` hands the two peels of any direction to the port (passdiff's
//! `peel_frustums_for` for the directions the capture lacks).

/// An axis-aligned world box as min/max corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub fn empty() -> Aabb {
        Aabb { min: [f32::MAX; 3], max: [f32::MIN; 3] }
    }
    pub fn is_empty(&self) -> bool {
        self.min[0] > self.max[0]
    }
    pub fn add_point(&mut self, p: [f32; 3]) {
        for k in 0..3 {
            self.min[k] = self.min[k].min(p[k]);
            self.max[k] = self.max[k].max(p[k]);
        }
    }
    pub fn add_box(&mut self, b: &Aabb) {
        if b.is_empty() {
            return;
        }
        self.add_point(b.min);
        self.add_point(b.max);
    }
    /// (min + max) · 0.5 per axis, in f32.
    pub fn centre(&self) -> [f32; 3] {
        [(self.min[0] + self.max[0]) * 0.5, (self.min[1] + self.max[1]) * 0.5, (self.min[2] + self.max[2]) * 0.5]
    }
    /// min + (max − min)·0.5 per axis, in f32 — the {centre, half} form of a box built from its corners.
    pub fn centre_from_half(&self) -> [f32; 3] {
        let h = self.half();
        [self.min[0] + h[0], self.min[1] + h[1], self.min[2] + h[2]]
    }
    /// (max − min) · 0.5 per axis, in f32.
    pub fn half(&self) -> [f32; 3] {
        [(self.max[0] - self.min[0]) * 0.5, (self.max[1] - self.min[1]) * 0.5, (self.max[2] - self.min[2]) * 0.5]
    }
}

/// `cross(a, b)` in the component order the game's basis needs (a.y·b.z − a.z·b.y, …), f32.
#[inline]
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

#[inline]
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// How a vector is normalised: divide by sqrt(len²), or multiply by 1/sqrt(len²).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Normalise {
    /// v / sqrt(len²)
    DivSqrt,
    /// v · (1 / sqrt(len²))
    MulRsqrt,
    /// v · f32(1 / sqrt(f64(len²)))
    MulRsqrtF64,
    /// v · rsqrt_approx(len²) refined by one Newton step: r = r₀·(1.5 − 0.5·len²·r₀²) — the SSE `rsqrtss` idiom (r₀
    /// here the correctly rounded 1/√ so the step only perturbs the last bits)
    MulRsqrtNewton,
    /// v · f32(1/√) with len² summed as (x² + z²) + y²
    MulRsqrtXZ,
    /// the software rsqrt: r₀ = bits(0x5f3759df − (bits(len²) >> 1)), then N Newton steps r = r·(1.5 − 0.5·len²·r²) (Magic1 = one, Magic2 = two)
    Magic1,
    Magic2,
}

pub fn normalise(v: [f32; 3], how: Normalise) -> [f32; 3] {
    let l2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    match how {
        Normalise::DivSqrt => {
            let l = l2.sqrt();
            [v[0] / l, v[1] / l, v[2] / l]
        }
        Normalise::MulRsqrt => {
            let r = 1.0 / l2.sqrt();
            [v[0] * r, v[1] * r, v[2] * r]
        }
        Normalise::MulRsqrtF64 => {
            let r = (1.0 / (l2 as f64).sqrt()) as f32;
            [v[0] * r, v[1] * r, v[2] * r]
        }
        Normalise::MulRsqrtNewton => {
            let r0 = 1.0 / l2.sqrt();
            let r = r0 * (1.5 - 0.5 * l2 * r0 * r0);
            [v[0] * r, v[1] * r, v[2] * r]
        }
        Normalise::Magic1 | Normalise::Magic2 => {
            let mut r = f32::from_bits(0x5f37_59dfu32.wrapping_sub(l2.to_bits() >> 1));
            let n = if how == Normalise::Magic1 { 1 } else { 2 };
            for _ in 0..n {
                r = r * (1.5 - 0.5 * l2 * r * r);
            }
            [v[0] * r, v[1] * r, v[2] * r]
        }
        Normalise::MulRsqrtXZ => {
            let l2 = (v[0] * v[0] + v[2] * v[2]) + v[1] * v[1];
            let r = 1.0 / l2.sqrt();
            [v[0] * r, v[1] * r, v[2] * r]
        }
    }
}

/// The light basis from a direction: right = normalize(Y × D), up = D × right, forward = D.
pub fn basis_from_dir(d: [f32; 3], how: Normalise) -> ([f32; 3], [f32; 3], [f32; 3]) {
    basis_from_dir_opts(d, how, false, false)
}

/// The basis with the two optional normalisations the capture does not settle by itself: `norm_forward` normalises
/// D first (forward = D/|D|), `norm_up` normalises the up vector after the cross product.
pub fn basis_from_dir_opts(d: [f32; 3], how: Normalise, norm_forward: bool, norm_up: bool) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let fwd = if norm_forward { normalise(d, how) } else { d };
    let right = normalise(cross([0.0, 1.0, 0.0], fwd), how);
    let up = cross(fwd, right);
    let up = if norm_up { normalise(up, how) } else { up };
    (right, up, fwd)
}

/// How a dot product / a three-term sum is evaluated: separately rounded products summed left to right, or a chain
/// of fused multiply-adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DotOrder {
    Separate,
    Fma,
}

#[inline]
pub fn dot3(a: [f32; 3], b: [f32; 3], o: DotOrder) -> f32 {
    match o {
        DotOrder::Separate => (a[0] * b[0] + a[1] * b[1]) + a[2] * b[2],
        DotOrder::Fma => a[2].mul_add(b[2], a[1].mul_add(b[1], a[0] * b[0])),
    }
}

/// The knobs of the fit (CHmsVolumeShadow::UpdateFrustum 0x140a4b1f0 per RE child 5, the constants read off the
/// capture): the far pad, the ±|min|·ε / ±|max|·ε expansion, and the f32 evaluation orders the decompile does not
/// spell out (the basis normalisation, the dot products of the world→light translation and of the extents).
#[derive(Clone, Copy, Debug)]
pub struct FitRules {
    /// FUN_140a52ad0 / FUN_140a52a70: min_i −= |min_i|·eps, max_i += |max_i|·eps (1e-4)
    pub expand_eps: f32,
    /// SetFar(far + far_pad) before the expansion (5.0; step (e) of UpdateFrustum)
    pub far_pad: f32,
    pub normalise: Normalise,
    /// the extents Σ_k |R_ik|·h_k and the translation −eye·axis
    pub dot: DotOrder,
    /// the expansion as a multiplication by (1 + eps) instead of the ±|v|·eps add
    pub expand_by_scale: bool,
    /// the eye = min + (max − min)·0.5 (the {centre, half} box form) instead of (min + max)·0.5
    pub centre_from_half: bool,
    pub norm_forward: bool,
    pub norm_up: bool,
}

impl Default for FitRules {
    fn default() -> FitRules {
        FitRules { expand_eps: 1e-4, far_pad: 5.0, normalise: Normalise::MulRsqrt, dot: DotOrder::Separate, expand_by_scale: false, centre_from_half: true, norm_forward: false, norm_up: false }
    }
}

/// An orthographic camera as the game's GbxV_* cbuffers describe it: the light frame and the light-space frustum
/// {cx, cy, cz, hx, hy, hz} (CHmsVolumeShadow +0x2d4: isOrtho, centre, half extents; near = cz − hz, far = cz + hz).
#[derive(Clone, Copy, Debug)]
pub struct OrthoCamera {
    pub eye: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    pub c: [f32; 3],
    pub h: [f32; 3],
    pub dot: DotOrder,
}

impl OrthoCamera {
    pub fn near(&self) -> f32 {
        self.c[2] - self.h[2]
    }
    pub fn far(&self) -> f32 {
        self.c[2] + self.h[2]
    }
    /// `GbxV_WorldToCamera` as the log prints it: 4 rows of 3 (the HLSL float3x4 rows; column k = the k-th axis),
    /// the last row −(eye·right, eye·up, eye·forward).
    pub fn world_to_camera(&self) -> [[f32; 3]; 4] {
        [
            [self.right[0], self.up[0], self.forward[0]],
            [self.right[1], self.up[1], self.forward[1]],
            [self.right[2], self.up[2], self.forward[2]],
            [-dot3(self.eye, self.right, self.dot), -dot3(self.eye, self.up, self.dot), -dot3(self.eye, self.forward, self.dot)],
        ]
    }
    /// `GbxV_CameraProjection`: FUN_140195960 with the ortho branch and depth mode 2 (reversed) — sx = 2/(hx + hx),
    /// sy = 2/(hy + hy), sz = −1/(hz + hz), oz = −sz·cz + 0.5, P30 = cx·−sx, P31 = cy·−sy — then the device's x
    /// mirror (FUN_1409dc650, flag bit 0): sx and P30 negated. Rows as the log prints them.
    pub fn projection(&self) -> [[f32; 4]; 4] {
        let sx = 2.0 / (self.h[0] + self.h[0]);
        let sy = 2.0 / (self.h[1] + self.h[1]);
        let two_hz = self.h[2] + self.h[2];
        let sz = -1.0 / two_hz;
        let oz = -sz * self.c[2] + 0.5;
        let p30 = self.c[0] * -sx;
        let p31 = self.c[1] * -sy;
        [[sx * -1.0, 0.0, 0.0, 0.0], [0.0, sy, 0.0, 0.0], [0.0, 0.0, sz, 0.0], [p30 * -1.0, p31, oz, 1.0]]
    }
    /// `GbxV_Camera_MinZ_MaxZ_InvRange_HasDeferredZ` = (cz − hz, cz + hz, 1/(hz + hz), 0) (FUN_140a4c9a0).
    pub fn min_max_inv(&self) -> [f32; 4] {
        [self.near(), self.far(), 1.0 / (self.h[2] + self.h[2]), 0.0]
    }
    /// `GbxV_WorldPrCamera` = WorldToCamera (as 4×4 with (0,0,0,1) in the last column) · Projection, row-vector.
    pub fn world_pr_camera(&self) -> [[f32; 4]; 4] {
        mat4_mul(&w2c_4x4(&self.world_to_camera()), &self.projection())
    }
    /// The projection WITHOUT the device's x mirror (what the shadow-lookup matrix is built from).
    pub fn projection_unmirrored(&self) -> [[f32; 4]; 4] {
        let mut p = self.projection();
        p[0][0] = -p[0][0];
        p[3][0] = -p[3][0];
        p
    }
    /// The peel's / shadow's lookup matrix `WorldPw01Shadow` (the accumulate's cbuffer; CHmsVolumeShadow +0x140 by
    /// FUN_140a4c9a0 / FUN_140195c90): WorldToCamera · Projection (unmirrored) · Bias with the Bias rows
    /// (sx, 0, 0, 0) / (0, sy, 0, 0) / (0, 0, 1, 0) / (0.5, 0.5, −zb, 1), sx = −0.5·(w − 2)/w, sy = −0.5·(h − 2)/h (the
    /// one-texel inset of the lookup), zb = 0 for the lightmapper's shadow group.
    pub fn world_pw01_shadow(&self, w: u32, h: u32) -> [[f32; 4]; 4] {
        let sx = ((w - 2) as f32 * -0.5) / w as f32;
        let sy = ((h - 2) as f32 * -0.5) / h as f32;
        let bias = [[sx, 0.0, 0.0, 0.0], [0.0, sy, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.5, 0.5, -0.0, 1.0]];
        let wpc = mat4_mul(&w2c_4x4(&self.world_to_camera()), &self.projection_unmirrored());
        mat4_mul(&wpc, &bias)
    }
}

/// A 3×4 (rows) affine as a 4×4 with (0, 0, 0, 1) in the last column.
pub fn w2c_4x4(w: &[[f32; 3]; 4]) -> [[f32; 4]; 4] {
    let mut o = [[0f32; 4]; 4];
    for i in 0..4 {
        o[i] = [w[i][0], w[i][1], w[i][2], if i == 3 { 1.0 } else { 0.0 }];
    }
    o
}

/// Row-vector 4×4 product a · b, each element summed left to right with separately rounded products.
pub fn mat4_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut o = [[0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            o[i][j] = ((a[i][0] * b[0][j] + a[i][1] * b[1][j]) + a[i][2] * b[2][j]) + a[i][3] * b[3][j];
        }
    }
    o
}

/// UpdateFrustum steps (b)–(f) for the lightmapper (no viewer camera): the focus box in the light frame centred on
/// its own centre (the light's position) → the frustum {0, |R|·h}; SetFar(far + pad); the ±|·|·eps expansion of the
/// min/max corners; c = (max + min)·0.5, h = (max − min)·0.5.
pub fn fit_camera(b: &Aabb, d: [f32; 3], r: &FitRules) -> OrthoCamera {
    let (right, up, forward) = basis_from_dir_opts(d, r.normalise, r.norm_forward, r.norm_up);
    let hb = b.half();
    let abs3 = |v: [f32; 3]| [v[0].abs(), v[1].abs(), v[2].abs()];
    // (b) h' = |R|·h — row k of the world→light rotation is the k-th axis
    let mut c = [0f32; 3];
    let mut h = [dot3(abs3(right), hb, r.dot), dot3(abs3(up), hb, r.dot), dot3(abs3(forward), hb, r.dot)];
    // (e) SetFar(far + pad): near kept, cz = (near + far)·0.5, hz = (far − near)·0.5
    let near = c[2] - h[2];
    let far = (c[2] + h[2]) + r.far_pad;
    c[2] = (near + far) * 0.5;
    h[2] = (far - near) * 0.5;
    // (f) the expansion of the light-space corners
    for i in 0..3 {
        let (mut mn, mut mx) = (c[i] - h[i], c[i] + h[i]);
        if r.expand_by_scale {
            mn *= 1.0 + r.expand_eps;
            mx *= 1.0 + r.expand_eps;
        } else {
            mn -= mn.abs() * r.expand_eps;
            mx += mx.abs() * r.expand_eps;
        }
        c[i] = (mx + mn) * 0.5;
        h[i] = (mx - mn) * 0.5;
    }
    OrthoCamera { eye: if r.centre_from_half { b.centre_from_half() } else { b.centre() }, right, up, forward, c, h, dot: r.dot }
}

/// The distance between two f32 values in ulps of the captured one (0 = bit-identical).
pub fn ulps(ours: f32, game: f32) -> i64 {
    if ours.to_bits() == game.to_bits() || (ours == 0.0 && game == 0.0) {
        return 0;
    }
    let key = |v: f32| -> i64 { let b = v.to_bits() as i32; if b < 0 { (i32::MIN - b) as i64 } else { b as i64 } };
    key(ours) - key(game)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basis_of_the_captured_sun_direction_reproduces_the_world_to_camera_columns() {
        // frame 127448 eid 347: DirInWorld and the WorldToCamera rows
        let d = [-0.22097179293632507f32, -0.9164636135101318, 0.33356550335884094];
        let (right, up, fwd) = basis_from_dir(d, Normalise::DivSqrt);
        let game_right = [0.8336676359176636f32, -0.0, 0.5522664189338684];
        let game_up = [-0.5061320662498474f32, 0.40011805295944214, 0.7640260457992554];
        for k in 0..3 {
            assert!(ulps(right[k], game_right[k]).abs() <= 1, "right[{k}] {} vs {}", right[k], game_right[k]);
            assert!(ulps(up[k], game_up[k]).abs() <= 1, "up[{k}] {} vs {}", up[k], game_up[k]);
        }
        assert_eq!(right[1].to_bits(), (-0.0f32).to_bits(), "the cross product's component order leaves −0");
        assert_eq!(fwd, d);
    }

    #[test]
    fn the_scene_box_gives_the_captured_sun_camera_bit_for_bit() {
        // S from the capture's casters (frustum-check): the tiles (y 3.9999785, x/z 0..2048) and the wall's top at 95.5
        let b = Aabb { min: [-1.5258789e-5, 3.9999785, 7.6293945e-6], max: [2048.0, 95.5, 2048.0] };
        let d = [-0.22097179293632507f32, -0.9164636135101318, 0.33356550335884094];
        let cam = fit_camera(&b, d, &FitRules::default());
        // frame 127448 eid 347 SceneV
        assert_eq!(cam.eye, [1024.0, 49.7499885559082, 1024.0]);
        let w2c = cam.world_to_camera();
        let game_w2c = [[0.8336676359176636f32, -0.5061320662498474, -0.22097179293632507], [-0.0, 0.40011805295944214, -0.9164636135101318], [0.5522664189338684, 0.7640260457992554, 0.33356550335884094], [-1419.196533203125, -283.98931884765625, -69.701904296875]];
        for i in 0..4 { for j in 0..3 { assert_eq!(w2c[i][j].to_bits(), game_w2c[i][j].to_bits(), "WorldToCamera[{i}][{j}] {} vs {}", w2c[i][j], game_w2c[i][j]); } }
        let p = cam.projection();
        let game_p = [[-0.0007045535603538156f32, 0.0, 0.0, 0.0], [0.0, 0.0007581046666018665, 0.0, 0.0], [0.0, 0.0, -0.000816545682027936, 0.0], [0.0, -0.0, 0.5020415782928467, 1.0]];
        for i in 0..4 { for j in 0..4 { assert_eq!(p[i][j].to_bits(), game_p[i][j].to_bits(), "CameraProjection[{i}][{j}] {} vs {}", p[i][j], game_p[i][j]); } }
        let mm = cam.min_max_inv();
        assert_eq!([mm[0].to_bits(), mm[1].to_bits(), mm[2].to_bits()], [(-609.8353881835938f32).to_bits(), 614.8358764648438f32.to_bits(), 0.000816545682027936f32.to_bits()]);
        let wpc = cam.world_pr_camera();
        let game_wpc = [[-0.0005873634945601225f32, -0.0003837010881397873, 0.0001804335624910891, 0.0], [0.0, 0.00030333135509863496, 0.0007483343943022192, 0.0], [-0.0003891012747772038, 0.0005792116862721741, -0.0002723714569583535, 0.0], [0.9998999834060669, -0.21529363095760345, 0.5589563846588135, 1.0]];
        for i in 0..4 { for j in 0..4 { assert_eq!(wpc[i][j].to_bits(), game_wpc[i][j].to_bits(), "WorldPrCamera[{i}][{j}] {} vs {}", wpc[i][j], game_wpc[i][j]); } }
    }

    #[test]
    fn the_world_box_gives_the_captured_world_peel_camera_bit_for_bit() {
        // frame 127448 eid 7272 (a world peel, D (0.44498, 0.29333, 0.84614)): W = S with y up to 138
        let b = Aabb { min: [-1.5258789e-5, 3.9999785, 7.6293945e-6], max: [2048.0, 138.0, 2048.0] };
        let d = [0.44497716426849365f32, 0.29332953691482544, 0.8461400866508484];
        let cam = fit_camera(&b, d, &FitRules::default());
        assert_eq!(cam.eye, [1024.0, 70.99998474121094, 1024.0]);
        let mm = cam.min_max_inv();
        assert_eq!([mm[0].to_bits(), mm[1].to_bits(), mm[2].to_bits()], [(-1341.8912353515625f32).to_bits(), 1346.8917236328125f32.to_bits(), 0.000371915491996333f32.to_bits()]);
        let p = cam.projection();
        assert_eq!(p[0][0].to_bits(), (-0.0007230261107906699f32).to_bits());
        assert_eq!(p[1][1].to_bits(), 0.002128763822838664f32.to_bits());
        assert_eq!(p[3][2].to_bits(), 0.5009298920631409f32.to_bits());
        let w2c = cam.world_to_camera();
        assert_eq!(w2c[3], [-429.6924133300781, 337.7796630859375, -1342.930419921875]);
    }

    #[test]
    fn peel_frusta_follow_the_direction_with_two_peels() {
        let fs = peel_frusta([0.34547687f32, 0.11707824, 0.93109524], &PeelBoxes::pwc_day(), 4096, &FitRules::default());
        assert_eq!(fs.len(), 2);
        assert!(fs[0].half[0] > 1000.0 && fs[1].half[0] < 20.0, "world then fitted: {:?} {:?}", fs[0].half, fs[1].half);
        for f in &fs { let c = f.forward[0] * 0.34547687 + f.forward[1] * 0.11707824 + f.forward[2] * 0.93109524; assert!(c > 0.99999, "{c}"); }
    }

    #[test]
    fn ulps_is_signed_and_zero_on_identity() {
        assert_eq!(ulps(1.0, 1.0), 0);
        assert_eq!(ulps(f32::from_bits(0x3f800001), 1.0), 1);
        assert_eq!(ulps(1.0, f32::from_bits(0x3f800001)), -1);
        assert_eq!(ulps(0.0, -0.0), 0);
    }
}

/// The two focus boxes of a direction's peels (CHmsVolumeShadow +0x80 per phase, RE 5): the WORLD peel's = the
/// scene box ∪ the zone / probe-grid box, the FITTED peel's = the lightmapped items' records' x/z (the tiling rule's
/// cell) with the scene box's y range.
#[derive(Clone, Copy, Debug)]
pub struct PeelBoxes {
    pub world: Aabb,
    pub fitted: Aabb,
}

impl PeelBoxes {
    /// The boxes read off passcap/pwc-day (frame 127448): the scene box S = the 4096 seabed tiles (y 3.9999785, x/z
    /// 0..2048) ∪ the three items (the wall's top at 95.5); W = S with y up to 138.0; F = the item records' x ∈
    /// [861.01, 880.0], z ∈ [336.98, 369.0] (the wall record's ±0.02 thickness) with S's y.
    pub fn pwc_day() -> PeelBoxes {
        let s_min = [-1.5258789e-5f32, 3.9999785, 7.6293945e-6];
        PeelBoxes { world: Aabb { min: s_min, max: [2048.0, 138.0, 2048.0] }, fitted: Aabb { min: [861.01, s_min[1], 336.98], max: [880.0, 95.5, 369.0] } }
    }
}

/// The ordered peel frusta of a direction (the world peel, then the fitted peel) as the port's `Frustum`s: the fit on
/// each box, its `WorldPw01Shadow` for a `size`² target, read back through `Frustum::from_pw01` (the same route the
/// captured frusta take, so the (w − 2)/w inset convention is shared).
pub fn peel_frusta(d: [f32; 3], boxes: &PeelBoxes, size: u32, rules: &FitRules) -> Vec<crate::passdump::Frustum> {
    [boxes.world, boxes.fitted]
        .iter()
        .filter_map(|b| {
            let cam = fit_camera(b, d, rules);
            crate::passdump::Frustum::from_pw01(&cam.world_pw01_shadow(size, size))
        })
        .collect()
}
