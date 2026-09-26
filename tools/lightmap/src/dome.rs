//! The game's precomputed sphere point sets (`Techno\Media\PointsInSphere\Std.PointsInSphere.Gbx`,
//! class 0x09066000; RE child 2026-09-23): 135 sets of unit vectors, n = 4…132, 256, 512, 1032,
//! 2040, 4112, 8192. The lightmapper's sky cone = the points of a set inside the cone of
//! half-angle A around +y; using the same points reproduces its sampling pattern.

pub struct PointSets {
    pub sets: Vec<Vec<[f32; 3]>>,
}

impl PointSets {
    /// Parse the uncompressed .Gbx (header, then the 0x09066000 chunk: u32 count, count × {u32 n,
    /// u32 offset}, u32 total, total × float3).
    pub fn load(path: &str) -> Result<PointSets, String> {
        let d = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let u32_at = |o: usize| -> Option<u32> { d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
        // the body starts with the chunk id; find it after the header
        let mut o = 0;
        while o + 4 <= d.len() && u32_at(o) != Some(0x0906_6000) {
            o += 1;
        }
        // the first occurrence is the header's class id; the body chunk id follows the header
        let mut oc = o + 4;
        while oc + 4 <= d.len() && u32_at(oc) != Some(0x0906_6000) {
            oc += 1;
        }
        if oc + 8 > d.len() {
            return Err(format!("{path}: no 0x09066000 body chunk"));
        }
        let count = u32_at(oc + 4).ok_or("count")? as usize;
        let mut ranges = Vec::with_capacity(count);
        let mut p = oc + 8;
        for _ in 0..count {
            let n = u32_at(p).ok_or("n")? as usize;
            let off = u32_at(p + 4).ok_or("offset")? as usize;
            ranges.push((n, off));
            p += 8;
        }
        let total = u32_at(p).ok_or("total")? as usize;
        p += 4;
        if p + total * 12 > d.len() {
            return Err(format!("{path}: {total} vectors do not fit"));
        }
        let f32_at = |o: usize| f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
        let pts: Vec<[f32; 3]> = (0..total).map(|i| [f32_at(p + i * 12), f32_at(p + i * 12 + 4), f32_at(p + i * 12 + 8)]).collect();
        let sets = ranges.iter().map(|&(n, off)| pts[off.min(total)..(off + n).min(total)].to_vec()).collect();
        Ok(PointSets { sets })
    }

    /// The set with exactly `n` points, else the smallest with at least `n` (else the largest).
    pub fn set(&self, n: usize) -> Option<&Vec<[f32; 3]>> {
        self.sets.iter().find(|s| s.len() == n).or_else(|| self.sets.iter().filter(|s| s.len() >= n).min_by_key(|s| s.len())).or_else(|| self.sets.iter().max_by_key(|s| s.len()))
    }

    /// The game's pick for a requested count (FUN_14045fbd0): the first set with count ≥ n, or its
    /// predecessor when that is nearer (ties → the smaller): 1024 → 1032, 2048 → 2040, 4096 → 4112.
    pub fn nearest(&self, n: usize) -> Option<&Vec<[f32; 3]>> {
        let mut sizes: Vec<usize> = self.sets.iter().map(|s| s.len()).collect();
        sizes.sort_unstable();
        let up = sizes.iter().position(|&c| c >= n)?;
        let pick = if up > 0 && (sizes[up - 1] as i64 - n as i64).abs() <= (sizes[up] as i64 - n as i64).abs() { sizes[up - 1] } else { sizes[up] };
        self.sets.iter().find(|s| s.len() == pick)
    }

    /// The points of the `n`-set inside the cone of half-angle `deg` around +y.
    pub fn cone(&self, n: usize, deg: f32) -> Vec<[f32; 3]> {
        let c = deg.to_radians().cos();
        self.set(n).map(|s| s.iter().copied().filter(|p| p[1] >= c).collect()).unwrap_or_default()
    }
}

/// The banked copy's default location.
pub fn default_path() -> String {
    format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/client-re/Std.PointsInSphere.Gbx", std::env::var("HOME").unwrap_or_default())
}

/// The lightmapper's fixed rotation matrix, BIT-EXACT (RE child 5, `FUN_140236da0` 0x140236e67–0x140236e9d):
/// `M = I; M ← Rx(0.124326788)·M; M ← Ry(0.0599014498)·M; M ← Rz(0.313338965)·M`, row-major `[m00 m01 m02
/// m10 … m22]`, with the game's own three row-mixing helpers (`FUN_140188c70` Rx: row1' = c·row1 − s·row2,
/// row2' = s·row1 + c·row2; `FUN_140188d50` Ry: row0' = c·row0 + s·row2, row2' = −s·row0 + c·row2;
/// `FUN_140188e30` Rz: row0' = c·row0 − s·row1, row1' = s·row0 + c·row1), each entry `fl(fl(b·±s) + fl(a·c))`
/// in f32 with no FMA. sin/cos come from the CRT `sinf`/`cosf` (0x14195d240 / 0x14195c010: a double-precision
/// core rounded once), which for these three angles is the correctly rounded f32 (the exact values sit ≥ 0.088
/// ulp from every rounding midpoint) — `sin_cos_f32` reproduces them and the test pins the bit patterns.
pub fn rotation_matrix() -> [f32; 9] {
    let mut m = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    // Rx (0x140188c70): mixes rows 1 and 2
    let (s, c) = sin_cos_f32(f32::from_bits(0x3dfe_9f0b));
    for i in 0..3 {
        let (a, b) = (m[3 + i], m[6 + i]);
        m[3 + i] = b * -s + a * c;
        m[6 + i] = b * c + a * s;
    }
    // Ry (0x140188d50): mixes rows 0 and 2
    let (s, c) = sin_cos_f32(f32::from_bits(0x3d75_5b39));
    for i in 0..3 {
        let (a, b) = (m[i], m[6 + i]);
        m[i] = b * s + a * c;
        m[6 + i] = b * c + a * -s;
    }
    // Rz (0x140188e30): mixes rows 0 and 1
    let (s, c) = sin_cos_f32(f32::from_bits(0x3ea0_6df7));
    for i in 0..3 {
        let (a, b) = (m[i], m[3 + i]);
        m[i] = b * -s + a * c;
        m[3 + i] = b * c + a * s;
    }
    m
}

/// `(sinf(a), cosf(a))` as the game's CRT returns them: the double-precision sine/cosine rounded once to f32
/// (the UCRT core is accurate to well under a float ulp, so the two agree wherever the exact value is not within
/// ~1e-8 ulp of a midpoint — the three lightmapper angles are far from one).
pub fn sin_cos_f32(a: f32) -> (f32, f32) {
    let d = a as f64;
    (d.sin() as f32, d.cos() as f32)
}

/// One point through the matrix exactly as the rotation loop does it (0x140236f30…): per row
/// `fl(fl(m0·x + m1·y) + m2·z)` — three `mulss`, then `addss` left to right, no FMA.
#[inline]
pub fn rotate_point(m: &[f32; 9], p: [f32; 3]) -> [f32; 3] {
    let row = |r: usize| -> f32 { (m[3 * r] * p[0] + m[3 * r + 1] * p[1]) + m[3 * r + 2] * p[2] };
    [row(0), row(1), row(2)]
}

/// The lightmapper's fixed rotation of every table point (RE child 2): M = Rz(0.313338965)·Ry(0.0599014498)·Rx(0.124326788),
/// applied as d = M·p — now bit-exact with the game's list at `CHmsLightMap+0x4c8` (`rotation_matrix`, `rotate_point`).
pub fn rotate_set(points: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let m = rotation_matrix();
    points.iter().map(|&p| rotate_point(&m, p)).collect()
}

/// The hemisphere fold the list gets when `CHmsLightMapMood+0xcc ≠ 0 && FUN_14020cde0(lm) == 0`
/// (0x140236da0 tail): every direction's y is forced negative (`y = −|y|`, sign-bit ops on the f32).
/// The pwc-day capture (BlueBay Day) shows the fold OFF (its directions have both signs of y).
pub fn fold_down(points: &mut [[f32; 3]]) {
    for p in points {
        p[1] = f32::from_bits((p[1].to_bits() & 0x7fff_ffff) ^ 0x8000_0000);
    }
}

/// The lightmapper's raster supersample factor per quality enum (0x141e6f260 = {1, 2, 3, 3, 3, 3}, the game's
/// enum 0..=5; `FUN_140201560`), unless the mood/params carry their own (`CHmsLightMapParam+0x128`).
/// The dome sweeps interleave their directions into `ss²` groups (`FUN_140201580` = ss·ss).
pub fn supersample_enum(quality_enum: u32) -> u32 {
    [1u32, 2, 3, 3, 3, 3][quality_enum.min(5) as usize]
}

/// The same for a tinyctl quality (1..=5 → enum 0..=4; the numbering `sweep_counts` takes).
pub fn supersample(quality: u32) -> u32 {
    supersample_enum(quality.saturating_sub(1))
}

/// THE ISSUE ORDER of a dome sweep — `SPlugGroupOfPointInSphere::Compute` (0x140460270) +
/// `SPlugGroupOfPointInSphere::InsertPointAt_Group_IndexInGroup` (0x14045fdc0), transcribed (RE child 5,
/// 2026-09-25). `points` = the sweep's direction list exactly as the game holds it at `CHmsLightMap+0x4c8`
/// (the table set ROTATED by `rotate_set`, folded by `fold_down` when the mood asks) — the grouping runs on
/// that list (`RenderLightIndirectBounces` 0x140230ac0 l.926–940 hands it `lm+0x4c8` / `lm+0x4d0` and
/// `groups = ss²`), and its dot products are on the ROTATED f32 values, which decides the near-ties.
///
/// Returns `order[issue] = index into `points`` — `RenderLightIndirectDome` (0x140233b50 l.238–246) issues
/// direction `points[order[i]]` at issue index `i = lm+0x4a0` whenever `lm+0x48c ≥ 2 || lm+0x490 ≥ 2`
/// (ss ≥ 2), else the identity.
///
/// The rule (G = `groups`, N = points, group sizes `N/G + (g < N%G)`, slot `G·k + g` for the k-th point of
/// group g — the groups are round-robin interleaved, so issue index `i` is group `i mod G`, rank `i div G`):
///   * rounds `k = 0, 1, …`; within a round `g = 0..G`, skipping full groups;
///   * `(k = 0, g = 0)`: point 0;
///   * `(k = 0, g > 0)`: the unassigned point with the LARGEST dot with point 0 (strict `>`, first wins);
///   * `(k ≥ 1)`: the unassigned point whose LARGEST dot with the points already in group g is SMALLEST
///     (a farthest-point greedy per group; strict `<`, the first minimal wins; `maxss` over the group).
///   Dot products are `fl(fl(y·Y + x·X) + z·Z)` in f32 (three `mulss`, two `addss`; commutative, so equal to
///   the x-first spelling of the k = 0 branch).
pub fn group_issue_order(points: &[[f32; 3]], groups: u32) -> Vec<u32> {
    let n = points.len();
    let g_count = groups.max(1) as usize;
    let mut order = vec![u32::MAX; n]; // grp+0x18: slot → point
    let mut slot_of = vec![u32::MAX; n]; // grp+0x28: point → slot (−1 = unassigned)
    let group_size = |g: usize| n / g_count + usize::from(g < n % g_count);
    let dot = |a: [f32; 3], b: [f32; 3]| -> f32 { (a[1] * b[1] + a[0] * b[0]) + a[2] * b[2] };
    // The game recomputes `max over group g's members of dot(p, member)` from scratch for every candidate
    // p at every insertion (O(N²·k)); the max of f32s is exact and order-free, so keeping it as a running
    // value per (group, point) — raised by the new member's dot at each insertion — yields the same bits
    // with O(G·N²) work (2040-point sweeps in milliseconds instead of seconds).
    let mut max_dot = vec![-100.0f32; g_count * n];
    let mut counters = vec![0usize; g_count];
    let mut total = 0usize;
    while total < n {
        for g in 0..g_count {
            let k = counters[g];
            if k >= group_size(g) {
                continue;
            }
            let slot = g_count * k + g;
            let pick: usize = if k == 0 {
                if g == 0 {
                    0
                } else {
                    // the closest unassigned point to point 0 (`comiss d, best; jbe skip` → strict >)
                    let mut best = -100.0f32;
                    let mut bi = usize::MAX;
                    for p in 0..n {
                        if slot_of[p] != u32::MAX {
                            continue;
                        }
                        let d = dot(points[p], points[0]);
                        if d > best {
                            best = d;
                            bi = p;
                        }
                    }
                    bi
                }
            } else {
                // the unassigned point farthest from group g's members: min over p of the max over the members
                // (`comiss best, m; jbe skip` → strict <, the first minimal p wins)
                let row = &max_dot[g * n..(g + 1) * n];
                let mut best = 100.0f32;
                let mut bi = usize::MAX;
                for p in 0..n {
                    if slot_of[p] != u32::MAX {
                        continue;
                    }
                    if best > row[p] {
                        best = row[p];
                        bi = p;
                    }
                }
                bi
            };
            debug_assert!(pick != usize::MAX);
            order[slot] = pick as u32;
            slot_of[pick] = slot as u32;
            // the new member of group g raises every point's max-dot with the group (`maxss`)
            let q = points[pick];
            let row = &mut max_dot[g * n..(g + 1) * n];
            for p in 0..n {
                let d = dot(points[p], q);
                if d > row[p] {
                    row[p] = d;
                }
            }
            counters[g] += 1;
            total += 1;
        }
    }
    order
}

/// The literal O(N²·k) form of `group_issue_order` (the game's loop shape), for the test that pins the two
/// equal.
#[cfg(test)]
fn group_issue_order_literal(points: &[[f32; 3]], groups: u32) -> Vec<u32> {
    let n = points.len();
    let g_count = groups.max(1) as usize;
    let mut order = vec![u32::MAX; n];
    let mut slot_of = vec![u32::MAX; n];
    let group_size = |g: usize| n / g_count + usize::from(g < n % g_count);
    let dot = |a: [f32; 3], b: [f32; 3]| -> f32 { (a[1] * b[1] + a[0] * b[0]) + a[2] * b[2] };
    let mut counters = vec![0usize; g_count];
    let mut total = 0usize;
    while total < n {
        for g in 0..g_count {
            let k = counters[g];
            if k >= group_size(g) {
                continue;
            }
            let slot = g_count * k + g;
            let pick = if k == 0 {
                if g == 0 {
                    0
                } else {
                    let (mut best, mut bi) = (-100.0f32, usize::MAX);
                    for p in (0..n).filter(|&p| slot_of[p] == u32::MAX) {
                        let d = dot(points[p], points[0]);
                        if d > best {
                            best = d;
                            bi = p;
                        }
                    }
                    bi
                }
            } else {
                let (mut best, mut bi) = (100.0f32, usize::MAX);
                for p in (0..n).filter(|&p| slot_of[p] == u32::MAX) {
                    let mut m = -100.0f32;
                    for j in 0..k {
                        let d = dot(points[p], points[order[g_count * j + g] as usize]);
                        if d > m {
                            m = d;
                        }
                    }
                    if best > m {
                        best = m;
                        bi = p;
                    }
                }
                bi
            };
            order[slot] = pick as u32;
            slot_of[pick] = slot as u32;
            counters[g] += 1;
            total += 1;
        }
    }
    order
}

/// A sweep's directions IN ISSUE ORDER for a tinyctl quality: the table set nearest the sweep's count,
/// rotated bit-exactly, permuted by `group_issue_order` with `ss²` groups (identity when ss < 2).
/// `dirs[i]` is the direction the game issues at issue index `i` — so the raster sub-sample of that pass is
/// `raster_subsample(i, ss)` and the peel-camera LCG block is the i-th draw of the chain.
pub fn sweep_directions(sets: &PointSets, quality: u32, sweep: usize, fold: bool) -> Option<Vec<[f32; 3]>> {
    let n = *sweep_counts(quality).get(sweep)?;
    let mut list = rotate_set(sets.nearest(n)?);
    if fold {
        fold_down(&mut list);
    }
    let order = issue_order(&list, supersample(quality));
    Some(order.iter().map(|&i| list[i as usize]).collect())
}

/// `group_issue_order` with the game's gate: ss < 2 → the identity (no `SPlugGroupOfPointInSphere` is built).
pub fn issue_order(list: &[[f32; 3]], ss: u32) -> Vec<u32> {
    if ss < 2 {
        (0..list.len() as u32).collect()
    } else {
        group_issue_order(list, ss * ss)
    }
}

/// The LM raster sub-sample of the pass at issue index `i` (`FUN_14023dde0`, run at the top of every dome
/// direction): `g = i mod ss²`, `ix = g mod ss`, `iy = g div ss` (`lm+0x494`, `lm+0x498`) — i.e. the
/// interleave group of the direction. The offset then comes from `raster_offset_rotated` (when the
/// supersampled raster bitmap `lm+0x708` is absent: H-basis moods, the pwc-day case) or
/// `raster_offset_grid` (when it exists), scaled to ST units as `2·off/W`, `2·off/H` (`FUN_14023de40`).
pub fn raster_subsample(issue: usize, ss: u32) -> (u32, u32) {
    let g = (issue % (ss * ss).max(1) as usize) as u32;
    (g % ss.max(1), g / ss.max(1))
}

/// `FUN_140436170`: the regular sub-texel grid, `((ix − 0.5·(nx−1))/nx, (iy − 0.5·(ny−1))/ny)` texels.
pub fn raster_offset_grid(ix: u32, iy: u32, nx: u32, ny: u32) -> [f32; 2] {
    [(ix as f32 - 0.5 * (nx - 1) as f32) / nx as f32, (iy as f32 - 0.5 * (ny - 1) as f32) / ny as f32]
}

/// `FUN_140436200`: the ROTATED sub-texel grid — `a, b` = the regular offsets, `x = a + b/nx`, `y = b − a/nx`
/// (texels, raster ST sense: +y up). For ss = 3 in ninths of a texel: g = 0..9 → (−4,−2) (−1,−3) (2,−4)
/// (−3,1) (0,0) (3,−1) (−2,4) (1,3) (4,2) — the capture's LM01_Trans_RasterSS cycle with its y flipped
/// (texel rows run down).
pub fn raster_offset_rotated(ix: u32, iy: u32, nx: u32, ny: u32) -> [f32; 2] {
    let a = (ix as f32 - 0.5 * (nx - 1) as f32) / nx as f32;
    let b = (iy as f32 - 0.5 * (ny - 1) as f32) / ny as f32;
    [a + b / nx as f32, b - a / nx as f32]
}

/// The dome direction counts per sweep for a tinyctl quality (1..=5 → the game's enum 0..=4):
/// Fast {64, 32}; Default {256, 128}; High {1024, 512, 256, 128}; Ultra {2048, 1024, 1024, 512, 256, 128}.
pub fn sweep_counts(quality: u32) -> Vec<usize> {
    match quality {
        0 | 1 => vec![],
        2 => vec![64, 32],
        3 => vec![256, 128],
        4 => vec![1024, 512, 256, 128],
        _ => vec![2048, 1024, 1024, 512, 256, 128],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dome_oracle::*;

    fn set(bits: &[[u32; 3]]) -> Vec<[f32; 3]> {
        bits.iter().map(|b| [f32::from_bits(b[0]), f32::from_bits(b[1]), f32::from_bits(b[2])]).collect()
    }

    #[test]
    fn crt_sin_cos_and_the_rotation_matrix_bits() {
        // sinf/cosf of the three angles (the exact values are ≥ 0.088 ulp from a rounding midpoint, so the
        // correctly rounded f32 is what the game's double-core CRT returns)
        let sc = |b: u32| { let (s, c) = sin_cos_f32(f32::from_bits(b)); (s.to_bits(), c.to_bits()) };
        assert_eq!(sc(0x3dfe_9f0b), (0x3dfd_f740, 0x3f7e_0627));
        assert_eq!(sc(0x3d75_5b39), (0x3d75_35ab, 0x3f7f_8a75));
        assert_eq!(sc(0x3ea0_6df7), (0x3e9d_d135, 0x3f73_8908));
        let m = rotation_matrix();
        let bits: Vec<u32> = m.iter().map(|v| v.to_bits()).collect();
        assert_eq!(bits, [0x3f73_1936, 0xbe98_fbb3, 0x3dc2_0438, 0x3e9d_88bf, 0x3f72_3dc7, 0xbdcc_19bc, 0xbd75_35ab, 0x3dfd_82a4, 0x3f7d_9184]);
        // and it is a rotation: the rows are orthonormal to f32 accuracy
        for r in 0..3 {
            let n = (0..3).map(|c| m[3 * r + c] * m[3 * r + c]).sum::<f32>();
            assert!((n - 1.0).abs() < 1e-6, "row {r} norm {n}");
        }
    }

    #[test]
    fn embedded_sets_are_the_banked_table_when_it_is_here() {
        let Ok(ps) = PointSets::load(&default_path()) else {
            eprintln!("(banked Std.PointsInSphere.Gbx absent — the embedded copies stand unverified here)");
            return;
        };
        let s256 = ps.set(256).expect("256-set");
        let s128 = ps.set(128).expect("128-set");
        assert_eq!(ps.nearest(256).unwrap().len(), 256);
        assert_eq!(ps.nearest(128).unwrap().len(), 128);
        for (a, b) in s256.iter().zip(SET256_BITS.iter()) { assert_eq!([a[0].to_bits(), a[1].to_bits(), a[2].to_bits()], *b); }
        for (a, b) in s128.iter().zip(SET128_BITS.iter()) { assert_eq!([a[0].to_bits(), a[1].to_bits(), a[2].to_bits()], *b); }
    }

    #[test]
    fn rotated_point_0_is_the_captures_first_direction() {
        let r = rotate_set(&set(&SET256_BITS));
        // the capture's first issued direction (issue-order.json sweep 0 issue 0): (0.345477, 0.117078, 0.931095)
        let d = [0.345477f32, 0.117078, 0.931095];
        for k in 0..3 { assert!((r[0][k] - d[k]).abs() < 1e-5, "{:?} vs {:?}", r[0], d); }
    }

    #[test]
    fn sweep0_issue_order_matches_the_capture() {
        let list = rotate_set(&set(&SET256_BITS));
        let order = group_issue_order(&list, 9);
        assert_eq!(order.len(), 256);
        let mut seen = vec![false; 256];
        for &o in &order { assert!(!seen[o as usize], "point {o} issued twice"); seen[o as usize] = true; }
        // the first round (issue 0..9): point 0 then the eight nearest to it — the capture's counter still
        // agrees with the issue index there
        assert_eq!(&order[..9], &[0, 1, 65, 17, 241, 240, 16, 18, 2]);
        // pwc6's tail: the true issue indices 246..=255
        for &(i, s) in SWEEP0_TAIL.iter() { assert_eq!(order[i as usize], s, "issue {i}"); }
        // pwc2: an ordered subsequence of the true order (its counter skipped 42 uncaptured directions)
        let mut j = 0usize;
        let mut true_index = Vec::new();
        for (i, &o) in order.iter().enumerate() {
            if j < SWEEP0_PWC2.len() && SWEEP0_PWC2[j].1 == o { true_index.push(i); j += 1; }
        }
        assert_eq!(j, SWEEP0_PWC2.len(), "only {j} of the pwc2 draws appear in order");
        assert_eq!(*true_index.last().unwrap(), 165, "the 124 captured draws span the first 166 issues");
        assert_eq!(&true_index[..12], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 13]);
    }

    #[test]
    fn sweep1_issue_order_matches_the_capture_from_its_second_direction() {
        let list = rotate_set(&set(&SET128_BITS));
        let order = group_issue_order(&list, 9);
        assert_eq!(order.len(), 128);
        assert_eq!(order[0], 0, "the first issued direction of every sweep is set point 0 (pwc6 missed it)");
        for &(k, s) in SWEEP1_PWC6.iter() { assert_eq!(order[k as usize + 1], s, "capture counter {k} = issue {}", k + 1); }
        // 128 = 9·14 + 2: groups 0 and 1 have 15 points, the rest 14 — the last round holds only slots 126, 127
        assert_eq!(&order[9..18], &[18, 124, 6, 55, 17, 96, 33, 75, 2]);
    }

    #[test]
    fn running_max_equals_the_literal_loop() {
        for (bits, g) in [(&SET256_BITS[..], 9u32), (&SET128_BITS[..], 9), (&SET128_BITS[..], 4), (&SET256_BITS[..], 4), (&SET128_BITS[..], 7)] {
            let list = rotate_set(&set(bits));
            assert_eq!(group_issue_order(&list, g), group_issue_order_literal(&list, g), "{} points, {g} groups", bits.len());
        }
    }

    #[test]
    fn issue_order_is_the_identity_below_two_supersamples() {
        let list = rotate_set(&set(&SET128_BITS));
        assert_eq!(issue_order(&list, 1), (0..128).collect::<Vec<u32>>());
        assert_eq!(supersample(3), 3);
        assert_eq!(supersample(2), 2);
        assert_eq!(supersample(1), 1);
        assert_eq!(supersample_enum(5), 3);
    }

    #[test]
    fn raster_subsample_cycles_with_the_issue_index_and_the_rotated_grid_is_the_captured_cycle() {
        // the capture's LM01_Trans_RasterSS cycle (bake.rs jitter_cycle), texel rows down
        let captured = [[-4.0f32, 2.0], [-1.0, 3.0], [2.0, 4.0], [-3.0, -1.0], [0.0, 0.0], [3.0, 1.0], [-2.0, -4.0], [1.0, -3.0], [4.0, -2.0]];
        for i in 0..40usize {
            let (ix, iy) = raster_subsample(i, 3);
            assert_eq!((ix, iy), ((i % 9 % 3) as u32, (i % 9 / 3) as u32));
            let o = raster_offset_rotated(ix, iy, 3, 3);
            let c = captured[i % 9];
            assert!((o[0] * 9.0 - c[0]).abs() < 1e-4 && (o[1] * 9.0 + c[1]).abs() < 1e-4, "issue {i}: {:?}·9 vs captured {:?} (y flipped)", o, c);
        }
        assert_eq!(raster_offset_grid(0, 2, 3, 3), [-1.0 / 3.0, 1.0 / 3.0]);
    }

    #[test]
    fn sweep_directions_are_the_rotated_set_in_issue_order() {
        let Ok(ps) = PointSets::load(&default_path()) else { return; };
        let d0 = sweep_directions(&ps, 3, 0, false).expect("sweep 0");
        let d1 = sweep_directions(&ps, 3, 1, false).expect("sweep 1");
        assert_eq!((d0.len(), d1.len()), (256, 128));
        let r = rotate_set(&set(&SET256_BITS));
        assert_eq!(d0[0], r[0]);
        assert_eq!(d0[1], r[1]);
        assert_eq!(d0[2], r[65]);
        assert_eq!(d0[255], r[4]);
        assert!(sweep_directions(&ps, 3, 2, false).is_none());
    }
}

#[cfg(test)]
mod subset_probe {
    use super::*;
    /// Are the smaller sweeps' direction sets subsets of sweep 0's? (The sweep-reuse lever needs it.)
    #[test]
    #[ignore]
    fn smaller_sets_versus_the_1024_set() {
        let ps = PointSets::load(&default_path()).expect("point sets");
        let big = rotate_set(ps.nearest(1024).unwrap());
        for n in [512usize, 256, 128] {
            let small = rotate_set(ps.nearest(n).unwrap());
            let mut exact = 0usize;
            let mut near = 0usize;
            let mut worst = 0.0f32;
            for s in &small {
                let mut best = f32::INFINITY;
                for b in &big {
                    let d = ((s[0] - b[0]).powi(2) + (s[1] - b[1]).powi(2) + (s[2] - b[2]).powi(2)).sqrt();
                    if d < best { best = d; }
                }
                if best == 0.0 { exact += 1; }
                if best < 1e-4 { near += 1; }
                worst = worst.max(best);
            }
            eprintln!("set {n}: {} points; exact matches in the 1024-set {exact}, within 1e-4 {near}, worst nearest distance {worst:.4}", small.len());
        }
    }
}
