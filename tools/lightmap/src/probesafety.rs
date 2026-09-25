//! `NHmsLightMap::ProbeCpt_SafetyOffset_Compute` (0x1402285e0, marker string 0x141b67528) — the CPU pass that fills
//! `TMapProbeSafetyOffset` (texture 17045: SNORM16 ×4 per probe, in CELLS) before the probe passes (RE 7,
//! 2026-09-25 20:30Z; decomp34). Transcribed from the decompile + asm; the ray query is the lightmapper's own
//! triangle ray-cast (FUN_1401edf40 over the scene tree at lm+0x568 + the fitted-tile tree at lm+0x6e0), which this
//! module takes as a closure so the port can plug its BVH in.
//!
//! Per probe block (stride 0x54 records: cell ranges +0x14..+0x20, cell size +0x24/+0x28/+0x2c, origin +0x30/+0x34/
//! +0x38), per probe (x, y, z) in the block's cell range:
//!
//! 1. p = origin + cell · (x, y, z) (world metres); L = 0.45 · cell.x; r = max(0.5, 0.5·L) (the probe "radius").
//! 2. Cast the SIX axis rays from p: d ∈ {+r x̂, −r x̂, +r ŷ, −r ŷ, +r ẑ, −r ẑ} (the ray vector IS the segment, length
//!    r). A hit whose reported distance t < 1.5 is kept (the ray query reports t in units of the segment: t < 1 = on
//!    the segment, the extra 0.5 accepts hits just beyond); the hit record = (normal xyz, t, ray dir xyz).
//! 3. No hit → the probe keeps offset 0. Else: mean normal n̄ = Σ normals / count (a zero normal in the set — a
//!    "no normal" hit — sets a flag), t_min = min t over the hits (start 2.0).
//!    a. |n̄| > 0.8 and no zero normal: walk along n̄/|n̄| in steps k = 1..8 of k · L · 0.125 (= k · cell · 0.05625):
//!       at each step recast the six rays; count hits with |n|² > 0.5 whose normal faces the ray (n · d > 0, i.e.
//!       we are BEHIND the surface — inside geometry); if any → stop the walk (the step is rejected); else if that
//!       step's t_min > the best so far → the step becomes the candidate (and stop when t_min ≥ 0.999 = the rays
//!       are clear).
//!    b. else (ambiguous normals): try FOUR fixed directions — (0, 1, 0), (0.94280905, −1/3, 0), (−0.47140452,
//!       −1/3, −0.8164966), (−0.47140452, −1/3, 0.8164966) — a tetrahedron (up + three down) — for k = 1..8; a
//!       direction that hits a back face is disabled for the remaining steps; the step with the best t_min wins,
//!       stop at ≥ 0.999.
//! 4. If a candidate improved t_min: offset = (q − p) / cell per axis → lroundf(· 32767) → SNORM16 into the volume
//!    at (x, y, z) (w stays 0).
//!
//! On pwc-day exactly one probe (25, 5, 23) of the 32×16×32 world block is offset: it sits at (872, 34, 360) m —
//! between the three items — and is pushed +0.16874 cells (+2.70 m) up: step k = 3 of L·0.125 = 3 · 7.2 · 0.125 = 2.7 m
//! along n̄ = +ŷ (the item tops). 0.16874·32767 = 5529 ✓ (the captured value).

/// One ray hit as the query reports it: the surface normal (zero when unknown), the distance in units of the ray
/// vector, and the ray direction (the ray vector).
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub normal: [f32; 3],
    pub t: f32,
    pub dir: [f32; 3],
}

/// The scene's ray query: `cast(origin, ray_vector)` → the nearest hit with t = distance / |ray_vector| (any t;
/// the caller filters at 1.5), or None. FUN_1401edf40 also feeds the fitted tiles' tree.
pub trait RayQuery {
    fn cast(&self, origin: [f32; 3], ray: [f32; 3]) -> Option<RayHit>;
}

fn six_rays(r: f32) -> [[f32; 3]; 6] {
    [[r, 0.0, 0.0], [-r, 0.0, 0.0], [0.0, r, 0.0], [0.0, -r, 0.0], [0.0, 0.0, r], [0.0, 0.0, -r]]
}

/// Cast the six rays from `p`; keep the hits with t < 1.5 (at most 6).
fn probe_hits<Q: RayQuery>(q: &Q, p: [f32; 3], r: f32) -> Vec<RayHit> {
    let mut out = Vec::with_capacity(6);
    for d in six_rays(r) {
        if let Some(h) = q.cast(p, d) {
            if h.t < 1.5 {
                out.push(RayHit { normal: h.normal, t: h.t, dir: d });
            }
        }
    }
    out
}

/// The step test of the walk: (a back-face hit exists, t_min over the hits (2.0 when none)).
fn step_eval(hits: &[RayHit]) -> (bool, f32) {
    let mut back = false;
    let mut tmin = 2.0f32;
    for h in hits {
        let n = h.normal;
        let n2 = n[1] * n[1] + n[0] * n[0] + n[2] * n[2];
        if 0.5 < n2 && 0.0 < n[0] * h.dir[0] + n[1] * h.dir[1] + n[2] * h.dir[2] {
            back = true;
        }
        if h.t <= tmin {
            tmin = h.t;
        }
    }
    (back, tmin)
}

/// The four fallback directions (a tetrahedron: up, then three down at 120°).
const TETRA: [[f32; 3]; 4] = [
    [0.0, 1.0, 0.0],
    [0.94280905, -0.33333334, 0.0],
    [-0.47140452, -0.33333334, -0.8164966],
    [-0.47140452, -0.33333334, 0.8164966],
];

/// The safety offset of one probe at world position `p` in a block of cell size `cell` (metres per cell, per
/// axis). Returns the offset in CELLS (before the SNORM16 quantisation), or None when the probe is left alone.
pub fn probe_safety_offset<Q: RayQuery>(q: &Q, p: [f32; 3], cell: [f32; 3]) -> Option<[f32; 3]> {
    let l = cell[0] * 0.45;
    let r = (l * 0.5).max(0.5);
    let hits = probe_hits(q, p, r);
    if hits.is_empty() {
        return None;
    }
    // mean normal, zero-normal flag, t_min
    let mut zero = false;
    let mut sum = [0f32; 3];
    let mut t0 = 2.0f32;
    for h in &hits {
        let n = h.normal;
        if n[0] * n[0] + n[1] * n[1] + n[2] * n[2] == 0.0 {
            zero = true;
        }
        if h.t <= t0 {
            t0 = h.t;
        }
        sum[0] += n[0];
        sum[1] += n[1];
        sum[2] += n[2];
    }
    let inv = 1.0 / hits.len() as f32;
    let mean = [sum[0] * inv, sum[1] * inv, sum[2] * inv];
    let len = (mean[1] * mean[1] + mean[0] * mean[0] + mean[2] * mean[2]).sqrt();
    let mut best_t = t0;
    let mut best: Option<[f32; 3]> = None;
    if len <= 0.8 || zero {
        // the tetrahedron search
        let mut disabled = [false; 4];
        let mut k = 0u32;
        'outer: loop {
            for (di, d) in TETRA.iter().enumerate() {
                if disabled[di] {
                    continue;
                }
                let s = (k + 1) as f32 * l * 0.125;
                let c = [p[0] + s * d[0], p[1] + s * d[1], p[2] + s * d[2]];
                let hs = probe_hits(q, c, r);
                let (back, tmin) = step_eval(&hs);
                if back {
                    disabled[di] = true;
                    continue;
                }
                if best_t < tmin {
                    best = Some(c);
                    best_t = tmin;
                    if 0.999 <= tmin {
                        break 'outer;
                    }
                }
            }
            k += 1;
            if !(best_t < 0.999 && k < 8) {
                break;
            }
        }
    } else {
        let inv = 1.0 / len;
        let n = [mean[0] * inv, mean[1] * inv, mean[2] * inv];
        let mut k = 0u32;
        loop {
            k += 1;
            let s = k as f32 * l * 0.125;
            let c = [p[0] + n[0] * s, p[1] + n[1] * s, p[2] + n[2] * s];
            let hs = probe_hits(q, c, r);
            let (back, tmin) = step_eval(&hs);
            if back {
                break;
            }
            if best_t < tmin {
                best = Some(c);
                best_t = tmin;
                if 0.999 <= tmin {
                    break;
                }
            }
            if k >= 8 {
                break;
            }
        }
    }
    let c = best?;
    Some([(1.0 / cell[0]) * (c[0] - p[0]), (1.0 / cell[1]) * (c[1] - p[1]), (1.0 / cell[2]) * (c[2] - p[2])])
}

/// The SNORM16 the volume stores: lroundf(v · 32767) per axis (the game's FUN_1418f6954 = lroundf).
pub fn offset_to_snorm16(o: [f32; 3]) -> [i16; 3] {
    let q = |v: f32| ((v * 32767.0).abs() + 0.5).floor().copysign(v * 32767.0) as i32 as i16;
    [q(o[0]), q(o[1]), q(o[2])]
}

/// The port's BVH as the ray query: the hit normal = the triangle's geometric normal (p0, e1, e2 — the face normal,
/// as the game's tree stores per-triangle planes); t in units of the ray vector.
impl RayQuery for crate::bvh::Bvh {
    fn cast(&self, origin: [f32; 3], ray: [f32; 3]) -> Option<RayHit> {
        // closest() takes a direction and tmax in its units: give it the ray vector itself with tmax 1.5
        let h = self.closest(origin, ray, 1.5)?;
        let t = &self.tris[h.tri as usize];
        let n = crate::geometry::norm(crate::geometry::cross(t.e1, t.e2));
        Some(RayHit { normal: n, t: h.t, dir: ray })
    }
}

/// Every probe of a block (cells `min..max`, world `pos`, cell size `cell`): the offsets as (x, y, z, snorm16 xyz)
/// for the probes that get one.
pub fn block_offsets<Q: RayQuery>(q: &Q, min: [u32; 3], max: [u32; 3], pos: [f32; 3], cell: f32) -> Vec<(u32, u32, u32, [i16; 3])> {
    let mut out = Vec::new();
    for z in min[2]..max[2] {
        for y in min[1]..max[1] {
            for x in min[0]..max[0] {
                let p = [pos[0] + cell * x as f32, pos[1] + cell * y as f32, pos[2] + cell * z as f32];
                if let Some(o) = probe_safety_offset(q, p, [cell, cell, cell]) {
                    out.push((x, y, z, offset_to_snorm16(o)));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor at y = 33.0 (normal +ŷ) under the probe, nothing else: the −ŷ ray hits at t = (p.y − 33)/r.
    struct Floor(f32);
    impl RayQuery for Floor {
        fn cast(&self, o: [f32; 3], d: [f32; 3]) -> Option<RayHit> {
            if d[1] >= 0.0 {
                return None;
            }
            let t = (o[1] - self.0) / -d[1];
            if t < 0.0 {
                return None;
            }
            Some(RayHit { normal: [0.0, 1.0, 0.0], t, dir: d })
        }
    }

    #[test]
    fn pwc_day_probe_25_5_23() {
        // the world block: pos (472, −46, −8), cell 16 → probe (25, 5, 23) at (872, 34, 360); the items' tops just
        // below. r = max(0.5, 0.45·16·0.5) = 3.6; a floor at 34 − 3.6·t0 with t0 = 0.75 (hit on the segment) gives
        // t_min 0.75 → the walk goes up along n̄ = +ŷ in steps of 0.9 m until the −ŷ ray clears (t ≥ 0.999·… ) —
        // the capture says the chosen step is k = 3 (+2.7 m = 0.16874 cells): t after k steps = (34 + 0.9k − y0)/3.6
        let y0 = 34.0 - 3.6 * 0.26;
        let off = probe_safety_offset(&Floor(y0), [872.0, 34.0, 360.0], [16.0, 16.0, 16.0]).expect("offset");
        assert_eq!(offset_to_snorm16(off), [0, 5529, 0]);
        // a probe in the open keeps no offset
        assert!(probe_safety_offset(&Floor(0.0), [872.0, 34.0, 360.0], [16.0, 16.0, 16.0]).is_none());
        assert_eq!(offset_to_snorm16([0.0, 0.16874, 0.0]), [0, 5529, 0]);
    }
}
