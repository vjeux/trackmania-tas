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

/// One texel's light list as CS 7348 keeps it: 8 slots of (light id, weight8 = ftou(√w·255), lit8 = ftou(shadow·255)) —
/// TexLightId (4 R16G16 slices, two 16-bit ids each), TexLightW (2 RGBA8 slices), TexLightIsLit (2 RGBA8 slices per frame).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LightList {
    pub id: [u16; 8],
    pub w8: [u8; 8],
    pub lit8: [u8; 8],
}

/// CS 7348 at one texel: the 9-jitter sum `sum(x, y)` = TMapLightSum (light·OutScale summed, shadow·OutScale summed, coverage
/// = Σ OutScale) → the lamp's weight w = light / coverage and its lit fraction, a one-texel dilation over the 3×3 ring where
/// the texel itself was not drawn (coverage ≤ 0.01), the weight stored as ftou(√w · 255); the lamp then REPLACES the weakest
/// of the texel's 8 entries when that entry is weaker than it (ties keep the earlier slot; an empty list has weight 0 entries).
/// Returns the updated list (unchanged when the stored weight is 0).
pub fn cs_7348(sum: &dyn Fn(i32, i32) -> [f32; 3], x: i32, y: i32, light_id: u16, list: LightList) -> LightList {
    let s = sum(x, y);
    // 3–5 / 7–32
    let (w, sh) = if 0.01 < s[2] {
        ((s[0] / s[2]).clamp(0.0, 1.0), (s[1] / s[2]).clamp(0.0, 1.0))
    } else {
        let mut acc = [0.0f32; 2];
        let mut cov = 0.0f32;
        // the ring in the shader's order: (−1,−1), (0,−1), (1,−1), (−1,0), (1,0), (−1,1), (0,1), (1,1) — fused mads, the coverage summed in order
        let first = sum(x - 1, y - 1);
        let second = sum(x, y - 1);
        acc = [second[2] * second[0], second[2] * second[1]];
        acc = [first[0] * first[2] + acc[0], first[1] * first[2] + acc[1]];
        cov = first[2] + second[2];
        for (dx, dy) in [(1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let n = sum(x + dx, y + dy);
            acc = [n[0] * n[2] + acc[0], n[1] * n[2] + acc[1]];
            cov = n[2] + cov;
        }
        let ok = 0.01 < cov;
        let d = [(acc[0] / cov).clamp(0.0, 1.0), (acc[1] / cov).clamp(0.0, 1.0)];
        if ok { (d[0], d[1]) } else { (0.0, 0.0) }
    };
    // 34–39
    let w8 = (w.sqrt() * 255.0) as u32;
    if w8 == 0 {
        return list;
    }
    // 102–121: the weakest entry below the new weight (the first of equals)
    let mut slot = 8usize;
    let mut thr = w8;
    for j in 0..8 {
        if (list.w8[j] as u32) < thr {
            slot = j;
            thr = list.w8[j] as u32;
        }
    }
    let mut out = list;
    if slot < 8 {
        out.w8[slot] = w8 as u8;
        out.lit8[slot] = (sh * 255.0) as u32 as u8;
        out.id[slot] = light_id;
    }
    out
}

#[cfg(test)]
mod list_tests {
    use super::*;

    #[test]
    fn a_lamp_enters_the_weakest_slot_and_a_weaker_one_does_not() {
        let covered = |_: i32, _: i32| [0.5f32 * 9.0 / 9.0, 1.0, 1.0]; // w = 0.5, shadow 1, coverage 1 (nine jitters of 1/9)
        let l0 = LightList::default();
        let l1 = cs_7348(&covered, 5, 5, 243, l0);
        assert_eq!(l1.id[0], 243);
        assert_eq!(l1.w8[0], (0.5f32.sqrt() * 255.0) as u8);
        assert_eq!(l1.lit8[0], 255);
        // a full list of stronger entries: the new lamp is dropped
        let full = LightList { id: [1; 8], w8: [200; 8], lit8: [255; 8] };
        assert_eq!(cs_7348(&covered, 5, 5, 243, full), full);
        // one weak entry among strong ones: it is the one replaced
        let mut mixed = full;
        mixed.w8[5] = 10;
        let r = cs_7348(&covered, 5, 5, 243, mixed);
        assert_eq!(r.id[5], 243);
        assert_eq!(r.w8[5], (0.5f32.sqrt() * 255.0) as u8);
    }

    #[test]
    fn an_undrawn_texel_takes_its_neighbours_average() {
        // the centre undrawn (coverage 0), the ring drawn with w 0.25
        let f = |x: i32, y: i32| if (x, y) == (5, 5) { [0.0f32, 0.0, 0.0] } else { [0.25, 1.0, 1.0] };
        let l = cs_7348(&f, 5, 5, 7, LightList::default());
        assert_eq!(l.id[0], 7);
        assert_eq!(l.w8[0], (0.25f32.sqrt() * 255.0) as u8);
        // an isolated undrawn texel with an undrawn ring: nothing
        let g = |_: i32, _: i32| [0.0f32; 3];
        assert_eq!(cs_7348(&g, 5, 5, 7, LightList::default()), LightList::default());
    }
}

/// PS 7351's g_CBufferP (the probe light pass): the probe grid (cell, origin per axis), the flat-cube faces / the shadow
/// matrix, the light, the cone, the attenuation, cSamplePerAxe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeLightCb {
    /// ProbeStWorld.X / .Y / .Z = (cell, origin): world = probe · cell + origin
    pub st: [[f32; 2]; 3],
    pub faces: [FlatCubeFace; 6],
    pub z_scale: f32,
    pub z_trans: f32,
    pub world_pw01_shadow: [[f32; 4]; 4],
    pub light_pos_or_dir: [f32; 3],
    pub is_light_pos: bool,
    pub inv_radius2: f32,
    pub inv_cos_range: f32,
    pub cos_outer: f32,
    pub spot_dir_neg: [f32; 3],
    pub samples_per_axis: u32,
    pub att_hn2: [f32; 4],
    pub is_light_spot: bool,
    pub is_shadow_cube: bool,
    pub is_att_hn2: bool,
    pub is_att_1minus_d2: bool,
}

impl ProbeLightCb {
    /// stpad f4936 eid 1081 (lamp B).
    pub fn stpad_f4936_eid1081() -> ProbeLightCb {
        let l = LightCb::stpad_f4936_eid34();
        ProbeLightCb {
            st: [[16.0, 408.0], [16.0, -582.0], [16.0, 1432.0]],
            faces: l.faces,
            z_scale: l.z_scale,
            z_trans: l.z_trans,
            world_pw01_shadow: [[-0.0007834314019419253, -0.0004416711162775755, 0.0003324841964058578, 0.0], [0.0, -0.0003677427303045988, -0.0013488430995494127, 0.0], [-0.0005080567207187414, 0.000681063742376864, -0.0005126958712935448, 0.0], [2.8060648441314697, 0.29622262716293335, 0.8521682024002075, 1.0]],
            light_pos_or_dir: [1512.0999755859375, 23.358840942382812, 1663.4326171875],
            is_light_pos: true,
            inv_radius2: 0.0006034570978954434,
            inv_cos_range: 3.9236578941345215,
            cos_outer: 0.08715580403804779,
            spot_dir_neg: [-1.0636256320140092e-09, 0.11013313382863998, 0.9939168691635132],
            samples_per_axis: 3,
            att_hn2: l.att_hn2,
            is_light_spot: true,
            is_shadow_cube: true,
            is_att_hn2: true,
            is_att_1minus_d2: false,
        }
    }
}

/// PS 7351 at one probe (x, y, z): the MAX over cSamplePerAxe³ sub-cell samples of shadow · attenuation · linear cone
/// term (1 when the light sits inside the sub-cell or on the sample); a sample outside the spot cone is moved toward the
/// cone within the sub-cell's half extent first (lines 56–87). `shadow(uv, ref)` is the comparison sample of TMapShadow.
pub fn ps_7351(cb: &ProbeLightCb, x: u32, y: u32, z: u32, shadow: &dyn Fn([f32; 2], f32) -> f32) -> f32 {
    let n = cb.samples_per_axis.max(1);
    let total = n * n * n;
    let (px, py, pz) = (x as f32, y as f32, z as f32);
    // 6–8: half a sub-cell per axis
    let half = [cb.st[0][0] * 0.5 / n as f32, cb.st[1][0] * 0.5 / n as f32, cb.st[2][0] * 0.5 / n as f32];
    // 9–14
    let nm1 = (n - 1) as f32;
    let cos_inner = 1.0 / cb.inv_cos_range + cb.cos_outer;
    let sin_inner = (1.0 - cos_inner * cos_inner).sqrt();
    let mut best = 0.0f32;
    let (mut i, mut j, mut l) = (0u32, 0u32, 0u32);
    for _ in 0..total {
        // 27–32: the next (i, j, l) — computed before use for the following iteration
        let (ni, nj, nl) = {
            let i1 = i + 1;
            let j1 = j + 1;
            let l1 = l + 1;
            if i1 >= n { (0, if j1 >= n { 0 } else { j1 }, if j1 >= n { l1 } else { l }) } else { (i1, j, l) }
        };
        // 33–39: the sample's world position
        let s = [(i as f32 - nm1 * 0.5) / n as f32 + px, (j as f32 - nm1 * 0.5) / n as f32 + py, (l as f32 - nm1 * 0.5) / n as f32 + pz];
        let p = [s[0] * cb.st[0][0] + cb.st[0][1], s[1] * cb.st[1][0] + cb.st[1][1], s[2] * cb.st[2][0] + cb.st[2][1]];
        // r9 = the light vector (x, y, z here; the shader keeps (x, z, y) in .xyw for the cube lookup)
        let mut d: [f32; 3];
        let mut inside = false;
        if cb.is_light_pos {
            d = [cb.light_pos_or_dir[0] - p[0], cb.light_pos_or_dir[1] - p[1], cb.light_pos_or_dir[2] - p[2]];
            // 42–52: the light inside this sub-cell
            if d[0].abs() < half[0] && d[1].abs() < half[1] && d[2].abs() < half[2] {
                best = 1.0f32.max(best);
                inside = true;
            }
            if !inside {
                // 53–87: the cone snap for spots — r10 = d̂ as (z, x, y); c = d̂·SpotDirNeg; outside = ((c − CosOuter)·InvCosRange) < 0.8
                let d2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                let rs = 1.0 / d2.sqrt();
                let dn = [d[0] * rs, d[1] * rs, d[2] * rs];
                let c = dn[0] * cb.spot_dir_neg[0] + dn[1] * cb.spot_dir_neg[1] + dn[2] * cb.spot_dir_neg[2];
                let outside = (c - cb.cos_outer) * cb.inv_cos_range < 0.8;
                let dist = d2.sqrt();
                let sin_c = (1.0 - c * c).sqrt();
                // 63–66: t = SpotDirNeg × (SpotDirNeg × d̂)  — the component of d̂ perpendicular to the axis, negated
                let cr = [dn[1] * cb.spot_dir_neg[2] - dn[2] * cb.spot_dir_neg[1], dn[2] * cb.spot_dir_neg[0] - dn[0] * cb.spot_dir_neg[2], dn[0] * cb.spot_dir_neg[1] - dn[1] * cb.spot_dir_neg[0]];
                let t = [cb.spot_dir_neg[1] * cr[2] - cb.spot_dir_neg[2] * cr[1], cb.spot_dir_neg[2] * cr[0] - cb.spot_dir_neg[0] * cr[2], cb.spot_dir_neg[0] * cr[1] - cb.spot_dir_neg[1] * cr[0]];
                // 67–69: the move = t · (dist·sin_c − sin_inner·dist)
                let m = dist * sin_c - sin_inner * dist;
                let mut mv = [t[0] * m, t[1] * m, t[2] * m];
                // 70–74: clamped to the sub-cell's half extent (the largest ratio)
                let r = [(mv[0].abs() / half[0]).max(1.0), (mv[1].abs() / half[1]).max(1.0), (mv[2].abs() / half[2]).max(1.0)];
                let rmax = r[2].max(r[1]).max(r[0]);
                mv = [mv[0] / rmax, mv[1] / rmax, mv[2] / rmax];
                // 75–79: only when outside and a spot
                let moved = [d[0] + mv[0], d[1] + mv[1], d[2] + mv[2]];
                let (d1, mv1) = if outside { (moved, mv) } else { (d, [0.0; 3]) };
                let (d1, mv1) = if cb.is_light_spot { (d1, mv1) } else { (d, [0.0; 3]) };
                // 80–87: the second clamp: (mv − d1) clamped to the half extent, then d = d1 + (that − mv)
                let e = [mv1[0] - d1[0], mv1[1] - d1[1], mv1[2] - d1[2]];
                let r2 = [(e[0].abs() / half[0]).max(1.0), (e[1].abs() / half[1]).max(1.0), (e[2].abs() / half[2]).max(1.0)];
                let r2max = r2[2].max(r2[1]).max(r2[0]);
                let e = [e[0] / r2max, e[1] / r2max, e[2] / r2max];
                let g = [e[0] - mv1[0], e[1] - mv1[1], e[2] - mv1[2]];
                d = [d1[0] + g[0], d1[1] + g[1], d1[2] + g[2]];
            }
        } else {
            d = [-cb.light_pos_or_dir[0], -cb.light_pos_or_dir[1], -cb.light_pos_or_dir[2]];
        }
        if !inside {
            // 91–93
            let d2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            if d2 < 0.0001 {
                best = 1.0f32.max(best);
            } else {
                let dist = d2.sqrt();
                let rs = 1.0 / d2.sqrt();
                let dn = [d[0] * rs, d[1] * rs, d[2] * rs];
                // 99–148: the shadow
                let sh = if cb.is_shadow_cube {
                    let lcb = LightCb { faces: cb.faces, z_scale: cb.z_scale, z_trans: cb.z_trans, light_pos: cb.light_pos_or_dir, inv_radius2: cb.inv_radius2, inv_cos_range: cb.inv_cos_range, cos_outer: cb.cos_outer, spot_dir_neg: cb.spot_dir_neg, spot_falloff_back_offset: 0.0, att_hn2: cb.att_hn2, out_scale: [1.0; 4], is_light_spot: cb.is_light_spot, is_att_hn2: cb.is_att_hn2 };
                    let (_f, uv, depth) = flat_cube_lookup(&lcb, d[0], d[1], d[2]);
                    shadow(uv, depth)
                } else {
                    let m = &cb.world_pw01_shadow;
                    let q = [p[0], p[1], p[2], 1.0];
                    let hx = q[0] * m[0][0] + q[1] * m[1][0] + q[2] * m[2][0] + q[3] * m[3][0];
                    let hy = q[0] * m[0][1] + q[1] * m[1][1] + q[2] * m[2][1] + q[3] * m[3][1];
                    let hz = q[0] * m[0][2] + q[1] * m[1][2] + q[2] * m[2][2] + q[3] * m[3][2];
                    let hw = q[0] * m[0][3] + q[1] * m[1][3] + q[2] * m[2][3] + q[3] * m[3][3];
                    shadow([hx / hw, hy / hw], hz / hw)
                };
                // 149–157: the attenuation
                let mut h = cb.att_hn2[1] * dist + cb.att_hn2[0];
                h = cb.att_hn2[2] * d2 + h;
                h = 1.0 / h + cb.att_hn2[3];
                let h = h.max(0.0);
                let q = (1.0 - d2 * cb.inv_radius2).max(0.0);
                let att = if cb.is_att_hn2 { h } else if cb.is_att_1minus_d2 { q } else { 1.0 };
                // 158–162: the linear cone term
                let c = dn[0] * cb.spot_dir_neg[0] + dn[1] * cb.spot_dir_neg[1] + dn[2] * cb.spot_dir_neg[2];
                let cone = ((c - cb.cos_outer) * cb.inv_cos_range).clamp(0.0, 1.0);
                let att = if cb.is_light_spot { att * cone } else { att };
                // 163–164
                best = (sh * att).max(best);
            }
        }
        i = ni;
        j = nj;
        l = nl;
    }
    best
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn a_probe_in_the_cone_is_lit_and_one_behind_the_lamp_is_dark() {
        let cb = ProbeLightCb::stpad_f4936_eid1081();
        let lit = |_: [f32; 2], _: f32| 1.0f32;
        // the lamp at (1512.1, 23.36, 1663.43) shines along −SpotDirNeg = (0, −0.11, −0.994): a probe 10 m along that
        let dir = [-cb.spot_dir_neg[0], -cb.spot_dir_neg[1], -cb.spot_dir_neg[2]];
        let target = [cb.light_pos_or_dir[0] + 10.0 * dir[0], cb.light_pos_or_dir[1] + 10.0 * dir[1], cb.light_pos_or_dir[2] + 10.0 * dir[2]];
        let probe = |k: usize| ((target[k] - cb.st[k][1]) / cb.st[k][0]).round() as u32;
        let v = ps_7351(&cb, probe(0), probe(1), probe(2), &lit);
        assert!(v > 0.3 && v <= 1.0, "{v}");
        // the probe holding the lamp itself: 1 (the light inside a sub-cell)
        let at = |k: usize| ((cb.light_pos_or_dir[k] - cb.st[k][1]) / cb.st[k][0]).round() as u32; // the probe whose samples surround the lamp
        let v1 = ps_7351(&cb, at(0), at(1), at(2), &lit);
        assert_eq!(v1, 1.0);
        // far behind the lamp (+SpotDirNeg, 60 m): beyond the radius and outside the cone → 0
        let behind = [cb.light_pos_or_dir[0] + 60.0 * cb.spot_dir_neg[0], cb.light_pos_or_dir[1] + 60.0 * cb.spot_dir_neg[1], cb.light_pos_or_dir[2] + 60.0 * cb.spot_dir_neg[2]];
        let pb = |k: usize| ((behind[k] - cb.st[k][1]) / cb.st[k][0]).round() as u32;
        assert_eq!(ps_7351(&cb, pb(0), pb(1), pb(2), &lit), 0.0);
    }
}

/// One probe's light list as CS 7357 keeps it: 8 (light id, weight8) slots (TexLightIds: four R16G16 volumes, two ids each;
/// TexLightWs: two RGBA8 volumes) — no lit byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProbeLightList {
    pub id: [u16; 8],
    pub w8: [u8; 8],
}

/// CS 7357 at one probe: the lamp's probe weight (PS 7351's R8 volume value) as ftou(w · 255) — no square root here —
/// replaces the weakest of the 8 entries when that entry is weaker (ties keep the earlier slot); 0 leaves the list.
pub fn cs_7357(weight: f32, light_id: u16, list: ProbeLightList) -> ProbeLightList {
    let w8 = (weight * 255.0) as u32;
    if w8 == 0 {
        return list;
    }
    let mut slot = 8usize;
    let mut thr = w8;
    for j in 0..8 {
        if (list.w8[j] as u32) < thr {
            slot = j;
            thr = list.w8[j] as u32;
        }
    }
    let mut out = list;
    if slot < 8 {
        out.w8[slot] = w8 as u8;
        out.id[slot] = light_id;
    }
    out
}

#[cfg(test)]
mod probe_list_tests {
    use super::*;

    #[test]
    fn the_probe_list_takes_the_linear_weight() {
        let l = cs_7357(0.5, 243, ProbeLightList::default());
        assert_eq!((l.id[0], l.w8[0]), (243, 127));
        assert_eq!(cs_7357(0.001, 9, l), l);
        let full = ProbeLightList { id: [1; 8], w8: [40; 8] };
        let r = cs_7357(0.5, 243, full);
        assert_eq!((r.id[0], r.w8[0]), (243, 127));
        assert_eq!(&r.w8[1..], &[40; 7]);
    }
}

/// THE FLAT-CUBE SHADOW MAP's six face cameras (the casters' VS 5397: world · GbxV_WorldPrCamera → clip; viewport 173² per face
/// at the face's tile of the 4096² D16 target; depth Greater on a 0 clear, DepthBias −1 / SlopeScaled −1.0, cull Back):
/// face 0 looks +X (viewport (0, 0)), 1 −X ((173, 0)), 2 +Y ((346, 0)), 3 −Y ((0, 173)), 4 +Z ((173, 173)), 5 −Z ((346, 173)).
/// With L the lamp, R the attenuation zero, s = 1/999: the clip w = the distance along the face axis, (x', y') the two other
/// coordinates as the capture orders them, z' = −s·coord + (R·s + s·L_coord) → z'/w = R·s / dom − s (the depth PS 7343 rebuilds
/// as ZTrans/dom + ZScale). The matrices below equal the stpad f4936 eids 651 / 722 / 804 / 847 / 906 / 960 (lamp B) bit for bit.
pub fn flat_cube_face_matrix(face: usize, l: [f32; 3], r_eff: f32) -> [[f32; 4]; 4] {
    let s = 1.0f32 / 999.0;
    let zt = r_eff * s;
    // rows = the world axes (x, y, z, 1) → columns (x', y', z', w')
    match face {
        0 => [[0.0, 0.0, -s, 1.0], [0.0, 1.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 0.0], [l[2], -l[1], zt + s * l[0], -l[0]]],
        1 => [[0.0, 0.0, s, -1.0], [0.0, 1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], [-l[2], -l[1], zt - s * l[0], l[0]]],
        2 => [[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, -s, 1.0], [0.0, -1.0, 0.0, 0.0], [-l[0], l[2], zt + s * l[1], -l[1]]],
        3 => [[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, s, -1.0], [0.0, 1.0, 0.0, 0.0], [-l[0], -l[2], zt - s * l[1], l[1]]],
        4 => [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, -s, 1.0], [-l[0], -l[1], zt + s * l[2], -l[2]]],
        _ => [[-1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, s, -1.0], [l[0], -l[1], zt - s * l[2], l[2]]],
    }
}

/// The face's viewport origin in the 4096² target (173² tiles: (0,0) (173,0) (346,0) (0,173) (173,173) (346,173)).
pub fn flat_cube_face_viewport(face: usize, size: u32) -> (u32, u32) {
    ((face as u32 % 3) * size, (face as u32 / 3) * size)
}

#[cfg(test)]
mod face_tests {
    use super::*;

    #[test]
    fn the_six_face_cameras_are_the_captured_ones() {
        let l = [1512.0999755859375f32, 23.358840942382812, 1663.4326171875];
        let r = 40.707722f32;
        let captured: [[[f32; 4]; 4]; 6] = [
            [[0.0, 0.0, -0.0010010009864345193, 1.0], [0.0, 1.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 0.0], [1663.4326171875, -23.358840942382812, 1.5543620586395264, -1512.0999755859375]],
            [[0.0, 0.0, 0.0010010009864345193, -1.0], [0.0, 1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], [-1663.4326171875, -23.358840942382812, -1.472865104675293, 1512.0999755859375]],
            [[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, -0.0010010009864345193, 1.0], [0.0, -1.0, 0.0, 0.0], [-1512.0999755859375, 1663.4326171875, 0.06413069367408752, -23.358840942382812]],
            [[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0010010009864345193, -1.0], [0.0, 1.0, 0.0, 0.0], [-1512.0999755859375, -1663.4326171875, 0.01736624725162983, 23.358840942382812]],
            [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, -0.0010010009864345193, 1.0], [-1512.0999755859375, -23.358840942382812, 1.7058461904525757, -1663.4326171875]],
            [[-1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.0010010009864345193, -1.0], [1512.0999755859375, -23.358840942382812, -1.6243492364883423, 1663.4326171875]],
        ];
        for f in 0..6 {
            let m = flat_cube_face_matrix(f, l, r);
            for r_ in 0..4 {
                for c in 0..4 {
                    let (a, b) = (m[r_][c], captured[f][r_][c]);
                    // the z' translation is the one derived term (R·s + s·L): within 2 f32 ulps of the captured
                    let tol = if c == 2 && r_ == 3 { 4.0 * f32::EPSILON * b.abs().max(1.0) } else { 0.0 };
                    assert!((a - b).abs() <= tol, "face {f} [{r_}][{c}]: {a} vs {b}");
                }
            }
        }
        assert_eq!(flat_cube_face_viewport(4, 173), (173, 173));
    }
}

/// The flat-cube shadow map of one lamp: six `size`² D16 tiles (face f at `flat_cube_face_viewport(f, size)` of a
/// 3·size × 2·size sheet), depth = the caster's z'/w' (R·s/dom − s) with the casters' D3D11 depth bias (DepthBias −1 unit,
/// SlopeScaledDepthBias −1.0 × the triangle's max depth slope per pixel) and the D16 rounding, Greater on a 0 clear.
pub struct FlatCubeMap {
    pub size: u32,
    /// row-major, 3·size wide, 2·size high
    pub depth: Vec<f32>,
}

impl FlatCubeMap {
    pub fn width(&self) -> u32 {
        self.size * 3
    }
    /// The texel the receivers' uv (in the 4096² target's normalised coordinates, tile at Trans ± MaxAbs) addresses:
    /// the target is `target` wide (4096); the sheet occupies its top-left 3·size × 2·size texels.
    pub fn texel(&self, uv: [f32; 2], target: u32) -> (i64, i64) {
        (((uv[0] * target as f32).floor()) as i64, ((uv[1] * target as f32).floor()) as i64)
    }
    /// The point comparison sample GreaterEqual (ref ≥ stored → 1): the receiver's rebuilt depth against the caster's.
    pub fn sample_cmp_ge(&self, uv: [f32; 2], reference: f32, target: u32) -> f32 {
        let (x, y) = self.texel(uv, target);
        let (w, h) = (self.width() as i64, (self.size * 2) as i64);
        let (x, y) = (x.clamp(0, w - 1), y.clamp(0, h - 1));
        let stored = self.depth[(y * w + x) as usize];
        if reference >= stored { 1.0 } else { 0.0 }
    }
}

/// Render the casters (world triangles, the model's winding) into the lamp's flat-cube map. Clipping against the D3D
/// z range 0 ≤ z' ≤ w' (the near plane at dom = R·s/(1 + s), the far at dom = R), the perspective divide, screen-space
/// linear z, the top-left rule of `raster::triangle`, cull Back with the clockwise front (frontCCW false).
pub fn render_flat_cube(l: [f32; 3], r_eff: f32, size: u32, tris: &[[[f32; 3]; 3]], cull_back: bool) -> FlatCubeMap {
    let (w, h) = ((size * 3) as usize, (size * 2) as usize);
    let mut depth = vec![0.0f32; w * h];
    let one_unit = 1.0 / 65535.0;
    for face in 0..6 {
        let m = flat_cube_face_matrix(face, l, r_eff);
        let (ox, oy) = flat_cube_face_viewport(face, size);
        for t in tris {
            // clip coordinates
            let clip: Vec<[f32; 4]> = t.iter().map(|p| {
                let q = [p[0], p[1], p[2], 1.0f32];
                [q[0] * m[0][0] + q[1] * m[1][0] + q[2] * m[2][0] + q[3] * m[3][0], q[0] * m[0][1] + q[1] * m[1][1] + q[2] * m[2][1] + q[3] * m[3][1], q[0] * m[0][2] + q[1] * m[1][2] + q[2] * m[2][2] + q[3] * m[3][2], q[0] * m[0][3] + q[1] * m[1][3] + q[2] * m[2][3] + q[3] * m[3][3]]
            }).collect();
            // Sutherland–Hodgman against z' ≥ 0 and z' ≤ w'
            let clip_plane = |poly: &[[f32; 4]], inside: &dyn Fn(&[f32; 4]) -> f32| -> Vec<[f32; 4]> {
                let mut out = Vec::new();
                let n = poly.len();
                for i in 0..n {
                    let (a, b) = (poly[i], poly[(i + 1) % n]);
                    let (da, db) = (inside(&a), inside(&b));
                    if da >= 0.0 { out.push(a); }
                    if (da >= 0.0) != (db >= 0.0) {
                        let tt = da / (da - db);
                        out.push([a[0] + (b[0] - a[0]) * tt, a[1] + (b[1] - a[1]) * tt, a[2] + (b[2] - a[2]) * tt, a[3] + (b[3] - a[3]) * tt]);
                    }
                }
                out
            };
            let poly = clip_plane(&clip, &|v| v[2]);
            if poly.len() < 3 { continue; }
            let poly = clip_plane(&poly, &|v| v[3] - v[2]);
            if poly.len() < 3 { continue; }
            // the divide → window coordinates in the face tile (y down), z in [0, 1]
            let win: Vec<[f32; 3]> = poly.iter().map(|v| { let iw = 1.0 / v[3]; [(v[0] * iw * 0.5 + 0.5) * size as f32, (0.5 - v[1] * iw * 0.5) * size as f32, v[2] * iw] }).collect();
            // culling on the polygon's winding (all fan triangles share it)
            let area = { let mut a = 0.0f32; for i in 0..win.len() { let (p, q) = (win[i], win[(i + 1) % win.len()]); a += p[0] * q[1] - q[0] * p[1]; } a };
            // window y is down: a clockwise triangle on screen has a positive signed area here
            if cull_back && area <= 0.0 { continue; }
            // the depth slope of the primitive (plane fit on the first fan triangle: z is affine in window x, y)
            let slope = {
                let (a, b, c) = (win[0], win[1], win[2]);
                let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
                if det.abs() < 1e-12 { 0.0 } else { let dzdx = ((b[2] - a[2]) * (c[1] - a[1]) - (c[2] - a[2]) * (b[1] - a[1])) / det; let dzdy = ((c[2] - a[2]) * (b[0] - a[0]) - (b[2] - a[2]) * (c[0] - a[0])) / det; dzdx.abs().max(dzdy.abs()) }
            };
            let bias = -one_unit - 1.0 * slope;
            for k in 1..win.len() - 1 {
                let (a, b, c) = (win[0], win[k], win[k + 1]);
                crate::raster::triangle(size, size, [[a[0], a[1]], [b[0], b[1]], [c[0], c[1]]], |x, y, bc| {
                    let z = a[2] * bc[0] + b[2] * bc[1] + c[2] * bc[2];
                    let zq = ((z + bias).clamp(0.0, 1.0) * 65535.0).round() / 65535.0;
                    let i = (oy as usize + y as usize) * w + ox as usize + x as usize;
                    if zq > depth[i] { depth[i] = zq; }
                });
            }
        }
    }
    FlatCubeMap { size, depth }
}

#[cfg(test)]
mod cube_tests {
    use super::*;

    #[test]
    fn a_wall_in_front_of_the_lamp_shadows_the_point_behind_it() {
        let l = [100.0f32, 10.0, 100.0];
        let r = 40.707722f32;
        // a 6 m square wall at x = 110 facing the lamp (both windings, so culling cannot drop it)
        let quad = [[110.0f32, 7.0, 97.0], [110.0, 13.0, 97.0], [110.0, 13.0, 103.0], [110.0, 7.0, 103.0]];
        let tris = vec![[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]], [quad[2], quad[1], quad[0]], [quad[3], quad[2], quad[0]]];
        let map = render_flat_cube(l, r, 173, &tris, false);
        // face 0 (+X) holds the wall: its tile has written texels, the others none
        let tile_sum = |f: usize| { let (ox, oy) = flat_cube_face_viewport(f, 173); let mut n = 0; for y in 0..173 { for x in 0..173 { if map.depth[((oy + y) * map.width() + ox + x) as usize] > 0.0 { n += 1; } } } n };
        assert!(tile_sum(0) > 100, "{}", tile_sum(0));
        for f in 1..6 { assert_eq!(tile_sum(f), 0, "face {f}"); }
        // a receiver at x = 120 behind the wall: its cube lookup (through LightCb's faces = the capture's layout) is shadowed;
        // one at x = 105 in front is lit
        let cb = {
            let mut c = LightCb::stpad_f4936_eid34();
            c.light_pos = l;
            c.z_trans = r / 999.0;
            c
        };
        let shadow = |uv: [f32; 2], reference: f32| map.sample_cmp_ge(uv, reference, 4096);
        let (_f, uv_b, ref_b) = flat_cube_lookup(&cb, l[0] - 120.0, l[1] - 10.0, l[2] - 100.0);
        let (_f2, uv_f, ref_f) = flat_cube_lookup(&cb, l[0] - 105.0, l[1] - 10.0, l[2] - 100.0);
        assert_eq!(shadow(uv_b, ref_b), 0.0, "behind the wall: uv {uv_b:?} ref {ref_b}");
        assert_eq!(shadow(uv_f, ref_f), 1.0, "in front of the wall: uv {uv_f:?} ref {ref_f}");
    }
}
