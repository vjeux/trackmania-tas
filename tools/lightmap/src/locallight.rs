//! THE LOCAL-LIGHT PASS (frame 1 — the lamps of a Sunrise / Sunset / Night bake), transcribed from the stpad Sunrise capture
//! (passcap/stpad-sunrise/stsun-exp1.tgz, frame f4936; RE 7's NOTES 00:10Z). Per lamp the client clears the 3072×2048
//! RGBA16F accumulation, the 96×80×32 R8 probe volume and the 4096² D16 shadow target, draws the FLAT-CUBE shadow map
//! (PS 937 / GS 1113, six 173² faces laid flat), one probe draw (PS 7351), then 7 raster jitters × the record groups
//! additively (One / One, OutScale 1/9) with PS 7343 = `LmLBumpDirect` (spot / ball, hyperbolic attenuation, the flat-cube
//! shadow lookup, N·L) writing (light, shadow, 0, 1) — no colour: the colour is applied at runtime per light id — and
//! CS 7348 / 7357 resolve the per-texel light lists. This module holds PS 7343 line by line (`ps_7343`) and the capture's
//! cbuffer as the fixture; the shadow map, the jitter accumulate, the compute resolves and the frame-1 writer follow.
//!
//! The DXBC of the pass (stsun-exp1.tgz → exp/f4936/shaders): PS 7343 (77 lines, below); VS 7303 (the lighting draw's vertex
//! shader: the instance quaternion / translation / scale from per-vertex streams v6 / v7 / v8, the chart ST from g_TcLM_ST_LM01
//! or v8 when the chart index is 0xffff, o0.xy = ST · uv · LM01_Scale_RasterSS + LM01_Trans_RasterSS (the jitter, texel-ninths),
//! o1 = the world position, o3 = the rotated normal — the H-basis VS 17118's vertex path with the jitter table); the flat-cube
//! shadow: VS 1111 = `ret`, GS 1113 emits one full-slice triangle ((−1.01, −1.01), (3.03, −1.01), (−1.01, 3.03), z 0) per
//! primitive into render-target slice vPrim + iSliceStart (the per-slice depth fill; the casters' draws are the eids the baker
//! exports), PS 937 = `ret`; PS 7351 (184 lines) the probe light pass; CS 7348 (206) / CS 7357 (132) the light-list resolves.
//! The lamp's cbuffer (stpad f4936 eid 34) is `LightCb::stpad_f4936_eid34`; its (radius 40, hyper2, attHTnLR) → (AttHN2, the
//! attenuation zero 40.7077) map is RE 7's open formula.
//!
//! ```text
//!  PS 7343 (ps_5_0, cb0 g_CBufferP, SMapShadow comparison, TMapShadow; v1 = world position, v3 = world normal):
//!   0  r0.xyw = LightPos.xzy − v1.xzy                      (the light vector, components (x, z, y) in .x .y .w)
//!   1  r1.x = |L|²      2 r1.y = |L|
//!   3–6  r1.y = 1 / (AttHN2.x + AttHN2.y·|L| + AttHN2.z·|L|²) + AttHN2.w        (the hyperbolic attenuation)
//!   7  r1.z = 1 − |L|²·InvRadius2                                                 (the 1 − d² attenuation)
//!   8  max(·, 0) both     9  att = IsAtt_HN2 ? hyperbolic : 1 − d²
//!   10–17 spot: q = LightPos + SpotFalloffBackOffset·SpotDirNeg − v1, normalised; c = q·SpotDirNeg; s = sat((c − CosOuter)·InvCosRange)
//!   18–20 smoothstep: s²·(3 − 2s)     21 att·spot     22 att = IsLightSpot ? att·spot : att
//!   23–24 if att > 0:
//!   25–26 l = L / |L|  (the normalised light vector, components (x, z, y) in r1.x r1.z r1.w)
//!   27–38 the six flat-cube faces (Scale_MaxAbs, Trans) into an indexable array
//!   39–54 the dominant axis of the light vector (x, y, z in the ORIGINAL order via r0.x = Lx, r0.w = Lz, r0.y = Ly … see below)
//!         picks the face: index = 2·(axis pair) + sign, the face-plane coordinates = the two other components / |dominant|
//!   55–62 uv = Trans + clamp(coords·Scale, −MaxAbs, MaxAbs)
//!   63–65 depth = ((|dominant|·ZScale)·0.999 + ZTrans) / |dominant|
//!   66  shadow = sample_c_lz(TMapShadow, uv, depth)  (the comparison sampler)
//!   67–70 out.x = att · shadow · max(0, N·l),  out.y = shadow
//!   74–75 o0 = (out.x, out.y, 0, 1) · OutScale
//! ```

/// One flat-cube face of the shadow map: the face-plane coordinates are scaled, clamped to ±MaxAbs and offset by Trans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlatCubeFace {
    pub scale: [f32; 2],
    pub max_abs: [f32; 2],
    pub trans: [f32; 2],
}

/// PS 7343's g_CBufferP (the fields the shader reads).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightCb {
    pub faces: [FlatCubeFace; 6],
    /// FlatCubeShadow.ZScale_ZTrans_00.xy
    pub z_scale: f32,
    pub z_trans: f32,
    pub light_pos: [f32; 3],
    pub inv_radius2: f32,
    pub inv_cos_range: f32,
    pub cos_outer: f32,
    pub spot_dir_neg: [f32; 3],
    pub spot_falloff_back_offset: f32,
    pub att_hn2: [f32; 4],
    pub out_scale: [f32; 4],
    pub is_light_spot: bool,
    pub is_att_hn2: bool,
}

impl LightCb {
    /// The stpad Sunrise capture, frame f4936 eid 34 (the WaterFCCenter lamp of block 97: RoadBorderSpot, radius 40 → the
    /// attenuation zero 40.7077, cone 140° / 170°).
    pub fn stpad_f4936_eid34() -> LightCb {
        let f = |s: [f32; 2], m: [f32; 2], t: [f32; 2]| FlatCubeFace { scale: s, max_abs: m, trans: t };
        let m = [0.02099609375, 0.02099609375];
        let a = 0.0211181640625;
        LightCb {
            faces: [
                f([-a, -a], m, [a, a]),
                f([a, -a], m, [0.0633544921875, a]),
                f([a, a], m, [0.1055908203125, a]),
                f([a, -a], m, [a, 0.0633544921875]),
                f([a, -a], m, [0.0633544921875, 0.0633544921875]),
                f([-a, -a], m, [0.1055908203125, 0.0633544921875]),
            ],
            z_scale: -0.0010010009864345193,
            z_trans: 0.0407484695315361,
            light_pos: [1504.5673828125, 23.358840942382812, 1655.9000244140625],
            inv_radius2: 0.0006034570978954434,
            inv_cos_range: 3.9236578941345215,
            cos_outer: 0.08715580403804779,
            spot_dir_neg: [-0.9939168691635132, 0.11013312637805939, 3.9683811792201595e-09],
            spot_falloff_back_offset: 0.3336227238178253,
            att_hn2: [0.6794761419296265, 0.014063496142625809, 0.0005237546865828335, -0.4717220067977905],
            out_scale: [0.1111111119389534; 4],
            is_light_spot: true,
            is_att_hn2: true,
        }
    }
}

/// The flat-cube lookup of PS 7343 lines 39–66 without the sample: which face, the face uv and the comparison depth for a
/// light vector `l0 = (Lx, Lz, Ly)` as the shader holds it (r0.x = Lx, r0.y = Lz, r0.w = Ly).
pub fn flat_cube_lookup(cb: &LightCb, lx: f32, ly: f32, lz: f32) -> (usize, [f32; 2], f32) {
    // 39: r2.xyz = |r0.wyy w| < |r0.xxw x|  →  r2.x = |Ly| < |Lx|, r2.y = |Lz| < |Lx|, r2.z = |Lz| < |Ly|
    let r2 = [ly.abs() < lx.abs(), lz.abs() < lx.abs(), lz.abs() < ly.abs()];
    // 40: r3.xyz = (−r0.xyw) < 0  →  Lx > 0, Lz > 0, Ly > 0
    let r3 = [-lx < 0.0, -lz < 0.0, -ly < 0.0];
    // 41: r4.x = r3.x ? 1 : 0   (the ±x face index: 0 for Lx > 0, 1 for Lx < 0 … the sign bit selects the face within the pair)
    let r4x = if r3[0] { 1.0f32 } else { 0.0 };
    // 42–43: r3.xy = bfi(1 bit at 4 / 2) of r3.yz  →  (Lz > 0 ? 4 : 0) + ?, computed as: r3.x = (r3.y & 1) << 0 | 4? — the bfi
    //        inserts 1 bit of r3.y at offset 4?? no: bfi(width 1, offset 0, src r3.yz, base (4, 2)) = (base & ~1) | (src & 1)
    //        → r3.x = 4 | (Lz > 0), r3.y = 2 | (Ly > 0)
    let r3x = 4.0 + if r3[1] { 1.0 } else { 0.0 };
    let r3y = 2.0 + if r3[2] { 1.0 } else { 0.0 };
    // 44: r5 = (−r0.y, −r0.x, −r0.w, −r0.w) / |r0.xyyx| = (−Lz/|Lx|, −Lx/|Lz|, −Ly/|Lz|, −Ly/|Lx|)
    let r5 = [-lz / lx.abs(), -lx / lz.abs(), -ly / lz.abs(), -ly / lx.abs()];
    // 45–48: r4 = (r4.x, r5.x, r5.w, |Lx|) — the ±x faces: index bit, coords (−Lz/|Lx|, −Ly/|Lx|), the dominant |Lx|
    let r4 = [r4x, r5[0], r5[3], lx.abs()];
    //        r5' = (r3.x, r5.y, r5.z, |Lz|) — the ±z faces (4 | sign): coords (−Lx/|Lz|, −Ly/|Lz|)
    let r5b = [r3x, r5[1], r5[2], lz.abs()];
    // 49: r4 = r2.y (|Lz| < |Lx|) ? r4 : r5'   — between the x and z faces the larger of |Lx|, |Lz| wins
    let r4 = if r2[1] { r4 } else { r5b };
    // 50–52: r0 = (r3.y, −Lx/|Ly|, −Lz/|Ly|, |Ly|) — the ±y faces (2 | sign)
    let r0 = [r3y, -lx / ly.abs(), -lz / ly.abs(), ly.abs()];
    // 53: r0 = r2.z (|Lz| < |Ly|) ? r0 : r5'   54: r0 = r2.x (|Ly| < |Lx|) ? r4 : r0
    let r0 = if r2[2] { r0 } else { r5b };
    let r0 = if r2[0] { r4 } else { r0 };
    // 55–56: face = ftou(r0.x) << 1 (two float4 per face in the indexable array)
    let face = (r0[0] as u32 as usize).min(5);
    let f = &cb.faces[face];
    // 59–62: uv = Trans + clamp(coords·Scale, −MaxAbs, MaxAbs)
    let mut uv = [r0[1] * f.scale[0], r0[2] * f.scale[1]];
    uv = [(-f.max_abs[0]).max(uv[0]).min(f.max_abs[0]), (-f.max_abs[1]).max(uv[1]).min(f.max_abs[1])];
    uv = [f.trans[0] + uv[0], f.trans[1] + uv[1]];
    // 63–65: depth = ((dominant·ZScale)·0.999 + ZTrans) / dominant
    let dom = r0[3];
    let depth = ((dom * cb.z_scale) * 0.999 + cb.z_trans) / dom;
    (face, uv, depth)
}

/// PS 7343 at one fragment: `p` the world position (v1), `n` the world normal (v3), `shadow(uv, ref)` the comparison sample
/// of TMapShadow (1 = lit). Returns o0 = (att·shadow·max(0, N·l), shadow, 0, 1) · OutScale.
pub fn ps_7343(cb: &LightCb, p: [f32; 3], n: [f32; 3], shadow: &dyn Fn([f32; 2], f32) -> f32) -> [f32; 4] {
    // 0: the light vector (x, z, y)
    let lx = cb.light_pos[0] - p[0];
    let lz = cb.light_pos[2] - p[2];
    let ly = cb.light_pos[1] - p[1];
    // 1–2
    let d2 = lx * lx + lz * lz + ly * ly;
    let d = d2.sqrt();
    // 3–6: the hyperbola
    let mut h = cb.att_hn2[1] * d + cb.att_hn2[0];
    h = cb.att_hn2[2] * d2 + h;
    h = 1.0 / h;
    h += cb.att_hn2[3];
    // 7
    let q = 1.0 - d2 * cb.inv_radius2;
    // 8–9
    let h = h.max(0.0);
    let q = q.max(0.0);
    let mut att = if cb.is_att_hn2 { h } else { q };
    // 10–22: the spot cone
    let bx = cb.spot_falloff_back_offset * cb.spot_dir_neg[0] + cb.light_pos[0] - p[0];
    let by = cb.spot_falloff_back_offset * cb.spot_dir_neg[1] + cb.light_pos[1] - p[1];
    let bz = cb.spot_falloff_back_offset * cb.spot_dir_neg[2] + cb.light_pos[2] - p[2];
    let inv = 1.0 / (bx * bx + by * by + bz * bz).sqrt();
    let (bx, by, bz) = (bx * inv, by * inv, bz * inv);
    let c = bx * cb.spot_dir_neg[0] + by * cb.spot_dir_neg[1] + bz * cb.spot_dir_neg[2];
    let s = ((c - cb.cos_outer) * cb.inv_cos_range).clamp(0.0, 1.0);
    let sm = (s * s) * (s * -2.0 + 3.0);
    if cb.is_light_spot {
        att *= sm;
    }
    // 23–24
    let (mut out_x, mut out_y) = (0.0f32, 0.0f32);
    if att > 0.0 {
        // 25–26: the normalised light vector (x, z, y)
        let rs = 1.0 / d2.sqrt();
        let (nx, nz, ny) = (lx * rs, lz * rs, ly * rs);
        let (_face, uv, depth) = flat_cube_lookup(cb, lx, ly, lz);
        // 66
        let sh = shadow(uv, depth);
        // 67–70
        let ndl = (n[0] * nx + n[1] * ny + n[2] * nz).max(0.0);
        out_x = att * (sh * ndl);
        out_y = sh;
    }
    // 74–75
    [out_x * cb.out_scale[0], out_y * cb.out_scale[1], 0.0, cb.out_scale[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lamp_lights_its_cone_and_nothing_behind_it() {
        let cb = LightCb::stpad_f4936_eid34();
        let lit = |_: [f32; 2], _: f32| 1.0f32;
        // a point 10 m in front of the lamp along its direction (SpotDirNeg points AT the lamp), floor normal up
        let dir = [-cb.spot_dir_neg[0], -cb.spot_dir_neg[1], -cb.spot_dir_neg[2]];
        let p = [cb.light_pos[0] + 10.0 * dir[0], cb.light_pos[1] + 10.0 * dir[1], cb.light_pos[2] + 10.0 * dir[2]];
        let o = ps_7343(&cb, p, [0.0, 1.0, 0.0], &lit);
        // the hyperbola at 10 m: 1/(0.67948 + 0.14063 + 0.05238) − 0.47172 = 0.674 → × N·l (0.11) × 1/9
        assert!(o[0] > 0.0 && o[0] < 0.1, "{o:?}");
        assert!((o[1] - 1.0 / 9.0).abs() < 1e-6, "{o:?}");
        assert_eq!(o[3], cb.out_scale[3]);
        // a point behind the lamp: the cone term is 0 → (0, 0, 0, OutScale)
        let pb = [cb.light_pos[0] - 10.0 * dir[0], cb.light_pos[1], cb.light_pos[2] - 10.0 * dir[2]];
        let ob = ps_7343(&cb, pb, [0.0, 1.0, 0.0], &lit);
        assert_eq!(ob[0], 0.0);
        assert_eq!(ob[1], 0.0);
        // beyond the attenuation zero (40.7 m): 0
        let pf = [cb.light_pos[0] + 45.0 * dir[0], cb.light_pos[1] + 45.0 * dir[1], cb.light_pos[2] + 45.0 * dir[2]];
        let of = ps_7343(&cb, pf, [0.0, 1.0, 0.0], &lit);
        assert_eq!(of[0], 0.0);
    }

    #[test]
    fn the_flat_cube_face_follows_the_dominant_axis() {
        let cb = LightCb::stpad_f4936_eid34();
        // −x dominant → the x face pair, +y dominant → the y pair (index 2 | sign), +z → the z pair (4 | sign)
        let (fx, _, _) = flat_cube_lookup(&cb, -5.0, 1.0, 1.0);
        let (fy, _, _) = flat_cube_lookup(&cb, 1.0, 5.0, 1.0);
        let (fz, _, _) = flat_cube_lookup(&cb, 1.0, 1.0, 5.0);
        assert!(fx <= 1, "{fx}");
        assert!(fy == 2 || fy == 3, "{fy}");
        assert!(fz == 4 || fz == 5, "{fz}");
        // every face uv lands inside its tile: Trans ± MaxAbs
        for (l, f) in [((-5.0f32, 1.0f32, 1.0f32), fx), ((1.0, 5.0, 1.0), fy), ((1.0, 1.0, 5.0), fz)] {
            let (_, uv, depth) = flat_cube_lookup(&cb, l.0, l.1, l.2);
            let face = &cb.faces[f];
            assert!((uv[0] - face.trans[0]).abs() <= face.max_abs[0] + 1e-6 && (uv[1] - face.trans[1]).abs() <= face.max_abs[1] + 1e-6, "{uv:?} {face:?}");
            assert!(depth.is_finite());
        }
    }
}
