//! The game's per-instance vegetation variation, bit for bit.
//!
//! Every `.VegetTreeModel.Gbx` instance the scene creates goes through
//! `NHmsForestVis` 0x14026b4f0 (Trackmania.exe, Aug 24 2025 build): the 28-byte
//! pose {quat w,x,y,z ; pos x,y,z} is hashed (MurmurHash2, seed 0x57489862,
//! 0x1401262f0) into a Nadeo LCG seed; ONE draw scales the tree
//! (`1 - (k/7) * Params.ScaleVar01`, k in 0..=7); and, when the spawner asks for
//! the variation (the map-ITEM path 0x141081910 does, the block/prefab spawners
//! do not), the same seed state is stepped again for a random world-Y yaw
//! (`Params.EnableRandomRotationY`) and two tilts in ±`Params.AngleMax_RotXZ_Deg`
//! about the world X and Z axes. BlueBay palms: ScaleVar01 0.1, tilt 1°, yaw on
//! — so every placed palm's real orientation is pseudo-random, decided by the
//! bits of its pose.
//!
//! The pose of a map item (CGameCtnAnchoredObject, chunk 0x03101002) reaches
//! the hash through this exact chain, all f32 SSE scalar ops, no FMA:
//!
//! 1. `q0 = ypr_to_quat(yaw, pitch, roll)` — 0x140193710, half angles through
//!    the double-precision Cephes sincos 0x14018cf70.
//! 2. `M = quat_to_mat(q0)` — 0x1401886d0.
//! 3. Iso4 product 0x140183fd0 of the object's `[I | pivot]` (0x50 in the
//!    object; identity from 0x140186d10, pivot = the placement's pivotPosition)
//!    with `[M | absolutePosition]`: `R = M` exactly, `t = M·pivot + pos` in the
//!    game's operand order.
//! 4. `q1 = mat_to_quat(M)` — 0x140194860 (the trace form), read off the item
//!    record by 0x141081910; `t` copied as is.
//! 5. `seed = murmur2(bytes(q1) ‖ bytes(t), 0x57489862)`.
//!
//! Then the draws (0x14026b4f0): `k = randint(seed, 0, 7)` on a COPY of the
//! seed (0x14026b480), `yaw = randf(&seed, 0, 2π)` if EnableRandomRotationY,
//! `tx = randf(&seed, -a, a)`, `tz = randf(&seed, -a, a)`, and the rotation
//! `q' = Rz(tz) ⊗ Rx(tx) ⊗ Ry(yaw) ⊗ q1` with the axis quaternions built from
//! the f32 sincos 0x14018ce30 (0x140193880/0x140193830/0x1401938e0) and the
//! product 0x140193a80 (r ⊗ q: the new rotation applied AFTER q, in world).
//!
//! Angles: `a = (AngleMax_RotXZ_Deg * 3.1415927) / 180`, `2π = (360 *
//! 3.1415927) / 180`, both f32 (0x14026b4f0's prologue).

/// Nadeo's LCG (0x14018cc50 / 0x14018cb60): `x' = (0x3039 - 0x3e39b193·x) & 0x7fffffff`.
pub fn lcg_step(s: &mut u32) -> u32 {
    *s = 0x3039u32.wrapping_sub(s.wrapping_mul(0x3e39b193)) & 0x7fff_ffff;
    *s
}

/// 0x14018cc50: an int in `lo..=hi`, `lo + ((n · (x' >> 16)) >> 15)`.
pub fn lcg_int(s: &mut u32, lo: u32, hi: u32) -> u32 {
    let x = lcg_step(s);
    lo + ((((hi - lo + 1) as u64) * ((x >> 16) as u64)) >> 15) as u32
}

/// 0x14018cb60: a float in `[lo, hi]`, `lo + (hi - lo) · ((x' >> 16) / 32767)`.
pub fn lcg_f32(s: &mut u32, lo: f32, hi: f32) -> f32 {
    let range = hi - lo;
    let x = lcg_step(s);
    let r = ((x >> 16) as i64 as f32) / 32767.0;
    r * range + lo
}

/// MurmurHash2 (0x1401262f0) over `data` with `seed`.
pub fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let len = data.len() as u32;
    let mut h = seed ^ len;
    let mut i = 0usize;
    while i + 4 <= data.len() {
        let mut k = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
        i += 4;
    }
    let rest = data.len() - i;
    if rest >= 3 {
        h ^= (data[i + 2] as u32) << 16;
    }
    if rest >= 2 {
        h ^= (data[i + 1] as u32) << 8;
    }
    if rest >= 1 {
        h ^= data[i] as u32;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

/// The engine's f32 sincos 0x14018ce30: range-reduced to ±π/2 with the
/// nearest whole turn, then two short polynomials. Returns (sin, cos).
pub fn sincos_f32(x: f32) -> (f32, f32) {
    let mut k = x * 0.159_154_94f32;
    if x < 0.0 {
        k -= 0.5;
    } else {
        k += 0.5;
    }
    let n = k as i32; // cvttss2si (truncation)
    let mut r = x - (n as f32) * 6.283_185_5f32;
    let s;
    if r > 1.570_796_4f32 {
        s = -1.0f32;
        r = 3.141_592_7f32 - r;
    } else if -1.570_796_4f32 > r {
        s = -1.0f32;
        r = -3.141_592_7f32 - r;
    } else {
        s = 1.0f32;
    }
    let r2 = r * r;
    let q0 = r2 * 2.605_161_5e-7f32;
    let mut p = 2.752_556_2e-6f32 - r2 * 2.388_985_9e-8f32;
    p = p * r2 - 1.984_087_4e-4f32;
    p = p * r2 + 8.333_331e-3f32;
    p = p * r2 - 0.166_666_67f32;
    p = p * r2 + 1.0;
    let sin = p * r;
    let mut q = 2.476_049_5e-5f32 - q0;
    q = q * r2 - 1.388_837_8e-3f32;
    q = q * r2 + 4.166_663_8e-2f32;
    q = q * r2 - 0.5;
    q = q * r2 + 1.0;
    let cos = q * s;
    (sin, cos)
}

/// The engine's double-core sincos 0x14018cf70 (Cephes: octant reduction by
/// 4/π with a three-part π/4, degree-13/14 polynomials in f64, f32 in and out).
/// Returns (sin, cos).
pub fn sincos_d(x: f32) -> (f32, f32) {
    const DP1: f64 = f64::from_bits(0x3fe9_21fb_4000_0000);
    const DP2: f64 = f64::from_bits(0x3e64_442d_0000_0000);
    const DP3: f64 = f64::from_bits(0x3ce8_4698_98cc_5170);
    const FOUR_OVER_PI: f64 = f64::from_bits(0x3ff4_5f30_6dc9_c882);
    const S: [f64; 6] = [
        f64::from_bits(0x3de5_d8fd_1fd1_9ccd), // 1/13!
        f64::from_bits(0x3e5a_e5e5_a929_1f5d), // 1/11!
        f64::from_bits(0x3ec7_1de3_567d_48a1), // 1/9!
        f64::from_bits(0x3f2a_01a0_19bf_df03), // 1/7!
        f64::from_bits(0x3f81_1111_1110_f7d0), // 1/5!
        f64::from_bits(0x3fc5_5555_5555_5548), // 1/3!
    ];
    const C: [f64; 6] = [
        f64::from_bits(0x3da8_fa49_a086_1a9b), // 1/14!
        f64::from_bits(0x3e21_ee9d_7b4e_3f05), // 1/12!
        f64::from_bits(0x3e92_7e4f_7eac_4bc6), // 1/10!
        f64::from_bits(0x3efa_01a0_19c8_44f5), // 1/8!
        f64::from_bits(0x3f56_c16c_16c1_4f91), // 1/6!
        f64::from_bits(0x3fa5_5555_5555_554b), // 1/4!
    ];
    let mut neg = 0.0f32 > x; // comiss 0, x ; seta  (false for -0.0 and NaN)
    let xd = f32::from_bits(x.to_bits() & 0x7fff_ffff) as f64;
    let mut yf = ((xd * FOUR_OVER_PI) as f32).floor();
    let mut j = yf as i32;
    if j & 1 != 0 {
        yf += 1.0;
        j += 1;
    }
    j &= 7;
    let mut swap = false;
    if j > 3 {
        j -= 4;
        swap = true;
        neg = !neg;
    }
    let yd = yf as f64;
    let cos_neg = if j <= 1 { swap } else { !swap };
    let z = ((xd - yd * DP1) - yd * DP2) - yd * DP3;
    let zz = z * z;
    let zzz = zz * z;
    let mut p = zz * S[0];
    p -= S[1];
    p *= zz;
    p += S[2];
    p *= zz;
    p -= S[3];
    p *= zz;
    p += S[4];
    p *= zz;
    p -= S[5];
    p *= zzz;
    p += z;
    let sin_f = p as f32;
    let c0 = zz * C[0];
    let mut q = C[1] - c0;
    let zz2 = zz * zz;
    q *= zz;
    q -= C[2];
    q *= zz;
    q += C[3];
    q *= zz;
    q -= C[4];
    q *= zz;
    let half = zz * 0.5;
    q += C[5];
    q *= zz2;
    q += 1.0 - half;
    let cos_f = q as f32;
    let flip = |v: f32, f: bool| if f { f32::from_bits(v.to_bits() ^ 0x8000_0000) } else { v };
    if (j - 1) as u32 > 1 {
        // octants 0 and 3: the polynomials stand
        (flip(sin_f, neg), flip(cos_f, cos_neg))
    } else {
        // octants 1 and 2: swapped
        (flip(cos_f, neg), flip(sin_f, cos_neg))
    }
}

fn neg(v: f32) -> f32 {
    f32::from_bits(v.to_bits() ^ 0x8000_0000)
}

/// 0x140193710: a placement's yaw/pitch/roll to the game's quaternion
/// (w, x, y, z), operand order preserved.
pub fn ypr_to_quat(yaw: f32, pitch: f32, roll: f32) -> [f32; 4] {
    let (sy, cy) = sincos_d(yaw * 0.5);
    let (sr, cr) = sincos_d(roll * 0.5);
    let (sp, cp) = sincos_d(pitch * 0.5);
    let srsy = sr * sy;
    let crsy = cr * sy;
    let srcy = sr * cy;
    let crcy = cr * cy;
    let w = srsy * sp - crcy * cp;
    let x = neg(srsy) * cp - crcy * sp;
    let y = neg(srcy) * sp - crsy * cp;
    let z = crsy * sp - srcy * cp;
    [w, x, y, z]
}

/// 0x1401886d0: quaternion (w,x,y,z) to the row-major 3×3 the engine stores.
pub fn quat_to_mat(q: [f32; 4]) -> [f32; 9] {
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    let z2 = z + z;
    let y2 = y + y;
    let wx2 = w * (x + x);
    let one_xx2 = 1.0 - x * (x + x);
    let mut m = [0f32; 9];
    m[0] = (1.0 - y * y2) - z * z2;
    m[3] = x * y2 + w * z2;
    m[6] = x * z2 - w * y2;
    m[1] = x * y2 - w * z2;
    m[4] = one_xx2 - z * z2;
    m[7] = y * z2 + wx2;
    m[2] = x * z2 + w * y2;
    m[5] = y * z2 - wx2;
    m[8] = one_xx2 - y * y2;
    m
}

/// 0x140194860: the 3×3 back to a quaternion (w,x,y,z) — the trace form when
/// the trace is positive, else the largest-diagonal form (next index 1,2,0).
pub fn mat_to_quat(m: &[f32; 9]) -> [f32; 4] {
    let tr = m[0] + m[4] + m[8];
    if 0.0 < tr {
        let s = (tr + 1.0).sqrt();
        let f = 0.5 / s;
        return [s * 0.5, (m[7] - m[5]) * f, (m[2] - m[6]) * f, (m[3] - m[1]) * f];
    }
    let next = [1usize, 2, 0];
    let mut i = usize::from(m[0] < m[4]);
    if m[i * 4] < m[8] {
        i = 2;
    }
    let j = next[i];
    let k = next[j];
    let s = ((m[i * 4] - (m[k * 4] + m[j * 4])) + 1.0).sqrt();
    let f = 0.5 / s;
    let mut q = [0f32; 4];
    q[i + 1] = s * 0.5;
    q[0] = (m[k * 3 + j] - m[j * 3 + k]) * f;
    q[j + 1] = (m[i * 3 + j] + m[j * 3 + i]) * f;
    q[k + 1] = (m[i * 3 + k] + m[k * 3 + i]) * f;
    q
}

/// 0x140183fd0's translation row for `[I | pivot]` then `[m | pos]`:
/// `t_i = ((m[3i+1]·pv.y + pv.x·m[3i]) + m[3i+2]·pv.z) + pos_i`.
pub fn iso4_translation(m: &[f32; 9], pivot: [f32; 3], pos: [f32; 3]) -> [f32; 3] {
    let mut t = [0f32; 3];
    for i in 0..3 {
        t[i] = ((m[3 * i + 1] * pivot[1] + pivot[0] * m[3 * i]) + m[3 * i + 2] * pivot[2]) + pos[i];
    }
    t
}

/// Steps 1–5 of the chain: the 28 hashed bytes' seed plus the quaternion and
/// position the instance starts from, for a map item's placement fields.
pub fn item_pose(yaw: f32, pitch: f32, roll: f32, pos: [f32; 3], pivot: [f32; 3]) -> ([f32; 4], [f32; 3], u32) {
    let q0 = ypr_to_quat(yaw, pitch, roll);
    let m = quat_to_mat(q0);
    let t = iso4_translation(&m, pivot, pos);
    let mut q1 = mat_to_quat(&m);
    // RUNTIME 2026-09-23 (S16, 1969 trees vs the live NHmsForestVis records):
    // the game's quaternion carries +0 where this chain yields −0 (exact quadrant
    // yaws: sin/cos = 0 exactly); the hash covers the bytes, so canonicalise
    for v in q1.iter_mut() {
        if *v == 0.0 {
            *v = 0.0;
        }
    }
    let mut bytes = Vec::with_capacity(28);
    for v in q1.iter().chain(t.iter()) {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    (q1, t, murmur2(&bytes, 0x5748_9862))
}

/// The instance-variation inputs of a `.VegetTreeModel.Gbx` (STreeModel +0/+4/+8).
#[derive(Clone, Copy, Debug)]
pub struct TreeParams {
    pub scale_var01: f32,
    pub angle_max_rot_xz_deg: f32,
    pub enable_random_rotation_y: bool,
}

/// One placed tree as the forest renderer sees it.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub quat: [f32; 4],
    pub pos: [f32; 3],
    pub scale: f32,
    pub seed: u32,
    /// The draws that were made: (yaw, tilt x, tilt z), radians; None when the
    /// spawner did not ask for the rotation variation or the model disables it.
    pub rotation: Option<(f32, f32, f32)>,
}

/// Quaternion about world Y by `a` (0x140193880): (cos a/2, 0, sin a/2, 0).
fn quat_y(a: f32) -> [f32; 4] {
    let (s, c) = sincos_f32(a * 0.5);
    [c, 0.0, s, 0.0]
}
fn quat_x(a: f32) -> [f32; 4] {
    let (s, c) = sincos_f32(a * 0.5);
    [c, s, 0.0, 0.0]
}
fn quat_z(a: f32) -> [f32; 4] {
    let (s, c) = sincos_f32(a * 0.5);
    [c, 0.0, 0.0, s]
}

/// 0x140193a80 and kin: `q ← r ⊗ q` in the engine's operand order.
fn rotate_after(q: &mut [f32; 4], r: [f32; 4]) {
    let (qw, qx, qy, qz) = (q[0], q[1], q[2], q[3]);
    let (rw, rx, ry, rz) = (r[0], r[1], r[2], r[3]);
    // asm of 0x140193a80: w = qw·rw − ((ry·qy + qx·rx) + rz·qz)
    let w = qw * rw - ((ry * qy + qx * rx) + rz * qz);
    let x = ((qx * rw + qw * rx) + qz * ry) - qy * rz;
    let y = ((qy * rw + qw * ry) + qx * rz) - qz * rx;
    let z = ((qz * rw + qw * rz) + qy * rx) - qx * ry;
    *q = [w, x, y, z];
}

/// 0x14026b4f0 for a pose that already went through [`item_pose`] (or any
/// 28-byte pose the scene hands the forest): the scale draw always, the
/// rotation draws when `with_rotation` (the item spawner's flag) and the model
/// enables either.
pub fn variation(quat: [f32; 4], pos: [f32; 3], seed: u32, params: TreeParams, with_rotation: bool) -> Instance {
    let two_pi = (360.0f32 * 3.141_592_7f32) / 180.0f32;
    let a = (params.angle_max_rot_xz_deg * 3.141_592_7f32) / 180.0f32;
    let mut copy = seed;
    let k = lcg_int(&mut copy, 0, 7);
    let scale = 1.0 - ((k as f32) / 7.0) * params.scale_var01;
    let mut q = quat;
    let mut rotation = None;
    let none = !params.enable_random_rotation_y && a == 0.0;
    if with_rotation && !none {
        let mut s = seed;
        let yaw = if params.enable_random_rotation_y { lcg_f32(&mut s, 0.0, two_pi) } else { 0.0 };
        let tx = lcg_f32(&mut s, neg(a), a);
        let tz = lcg_f32(&mut s, neg(a), a);
        rotate_after(&mut q, quat_y(yaw));
        rotate_after(&mut q, quat_x(tx));
        rotate_after(&mut q, quat_z(tz));
        rotation = Some((yaw, tx, tz));
    }
    Instance { quat: q, pos, scale, seed, rotation }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn murmur2_reference() {
        // the canonical MurmurHash2 test: "" with seed 0 -> 0; a known vector
        assert_eq!(murmur2(b"", 0), 0);
        assert_eq!(murmur2(b"hello", 0x57489862) != 0, true);
    }

    #[test]
    fn lcg_matches_notes() {
        // NOTES.md: seed 0x7d3fb6ac step -> (0x3039 - x*0x3e39b193) & 0x7fffffff
        let mut s = 0x7d3f_b6acu32;
        let x = lcg_step(&mut s);
        assert_eq!(x, 0x3039u32.wrapping_sub(0x7d3f_b6acu32.wrapping_mul(0x3e39_b193)) & 0x7fff_ffff);
    }

    #[test]
    fn sincos_agree_with_libm() {
        for i in -2000..2000 {
            let x = i as f32 * 0.00731;
            let (s, c) = sincos_d(x);
            assert!((s - x.sin()).abs() < 2e-7, "sin {x}: {s} vs {}", x.sin());
            assert!((c - x.cos()).abs() < 2e-7, "cos {x}: {c} vs {}", x.cos());
            // the f32 one reduces by n·2π in f32: past a couple of turns it drifts
            if x.abs() < 7.0 {
                let (s2, c2) = sincos_f32(x);
                assert!((s2 - x.sin()).abs() < 6e-7, "sinf {x}: {s2} vs {}", x.sin());
                assert!((c2 - x.cos()).abs() < 6e-7, "cosf {x}: {c2} vs {}", x.cos());
            }
        }
    }

    #[test]
    fn quat_round_trip() {
        let q0 = ypr_to_quat(-0.0761, -0.0, -0.0);
        let m = quat_to_mat(q0);
        let mut q1 = mat_to_quat(&m);
    // RUNTIME 2026-09-23 (S16, 1969 trees vs the live NHmsForestVis records):
    // the game's quaternion carries +0 where this chain yields −0 (exact quadrant
    // yaws: sin/cos = 0 exactly); the hash covers the bytes, so canonicalise
    for v in q1.iter_mut() {
        if *v == 0.0 {
            *v = 0.0;
        }
    }
        // same rotation up to sign
        let dot: f32 = q0.iter().zip(q1.iter()).map(|(a, b)| a * b).sum();
        assert!((dot.abs() - 1.0).abs() < 1e-5, "{q0:?} vs {q1:?}");
    }
}

/// Compare computed instances against the GhostShooter `/treeinst` dump of the
/// same map (`i model flag qw qx qy qz x y z scale`, `%.9g` floats, one row per
/// live NHmsForestVis record — the decoration's own forests included; the
/// record quaternion is stored (x, y, z, w)). Every
/// computed instance is matched to the runtime record at the same position
/// (|Δ| ≤ 0.002 m on each axis; the variation never moves a tree) and its
/// quaternion and scale are compared as f32 BITS. Prints the tally and the
/// first mismatches.
pub fn compare_runtime(computed: &[([f32; 3], [f32; 4], f32, String)], runtime_tsv: &str) {
    let text = std::fs::read_to_string(runtime_tsv).unwrap_or_else(|e| panic!("{runtime_tsv}: {e}"));
    let mut rows: Vec<([f32; 3], [f32; 4], f32, u32, u32)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 11 || f[0].parse::<u32>().is_err() {
            continue;
        }
        let p = |k: usize| f[k].parse::<f32>().unwrap_or(f32::NAN);
        // the record stores the quaternion as (x, y, z, w); ours is (w, x, y, z)
        rows.push(([p(7), p(8), p(9)], [p(6), p(3), p(4), p(5)], p(10), f[1].parse().unwrap_or(255), f[2].parse().unwrap_or(255)));
    }
    // a coarse grid on x/z for the position lookup
    let key = |p: [f32; 3]| ((p[0] / 0.5).floor() as i64, (p[2] / 0.5).floor() as i64);
    let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (i, r) in rows.iter().enumerate() {
        grid.entry(key(r.0)).or_default().push(i);
    }
    let (mut matched, mut exact, mut scale_ok, mut quat_ok, mut unmatched) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut shown = 0;
    let mut max_q = 0f32;
    let mut quat_neg = 0usize;
    for (pos, q, s, label) in computed {
        let (kx, kz) = key(*pos);
        let mut best: Option<(usize, usize)> = None;
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(v) = grid.get(&(kx + dx, kz + dz)) {
                    for &i in v {
                        let r = &rows[i];
                        if (r.0[0] - pos[0]).abs() <= 0.002 && (r.0[1] - pos[1]).abs() <= 0.002 && (r.0[2] - pos[2]).abs() <= 0.002 {
                            // several trees can share a position (stacked placements):
                            // prefer the record whose quaternion bits are ours
                            // (stacked items of different species share the pose, so the
                            // seed and the rotation draws too: the scale tells them apart)
                            let same_q = (0..4).all(|k| r.1[k].to_bits() == q[k].to_bits());
                            let same_s = r.2.to_bits() == s.to_bits();
                            let rank = usize::from(same_q) + usize::from(same_q && same_s);
                            if best.map_or(true, |(_, br)| rank > br) {
                                best = Some((i, rank));
                            }
                        }
                    }
                }
            }
        }
        let Some((i, _)) = best else {
            unmatched += 1;
            if shown < 10 {
                println!("UNMATCHED {label} at {:?}", pos);
                shown += 1;
            }
            continue;
        };
        matched += 1;
        let r = &rows[i];
        let same_q = (0..4).all(|k| r.1[k].to_bits() == q[k].to_bits());
        let same_q_neg = (0..4).all(|k| r.1[k].to_bits() == (-q[k]).to_bits());
        let same_s = r.2.to_bits() == s.to_bits();
        if same_q_neg && !same_q {
            quat_neg += 1;
        }
        if same_q || same_q_neg {
            quat_ok += 1;
        }
        if same_s {
            scale_ok += 1;
        }
        if (same_q || same_q_neg) && same_s {
            exact += 1;
        } else if shown < 25 {
            let dq = (0..4).map(|k| (r.1[k] - q[k]).abs().min((r.1[k] + q[k]).abs())).fold(0f32, f32::max);
            max_q = max_q.max(dq);
            let same_p = (0..3).all(|k| r.0[k].to_bits() == pos[k].to_bits());
            println!(
                "DIFF {label}: pos_bits_equal={same_p} runtime p=({}, {}, {}) computed p=({}, {}, {}) | runtime q=({}, {}, {}, {}) s={} model={} flag={} | computed q=({}, {}, {}, {}) s={} | max|dq|={dq:e}",
                r.0[0], r.0[1], r.0[2], pos[0], pos[1], pos[2], r.1[0], r.1[1], r.1[2], r.1[3], r.2, r.3, r.4, q[0], q[1], q[2], q[3], s
            );
            shown += 1;
        }
    }
    println!(
        "runtime records {} | computed {} | matched by position {} | unmatched {} | quaternion bit-exact {} (of which sign-flipped {}) | scale bit-exact {} | BOTH exact {}",
        rows.len(),
        computed.len(),
        matched,
        unmatched,
        quat_ok,
        quat_neg,
        scale_ok,
        exact
    );
}
