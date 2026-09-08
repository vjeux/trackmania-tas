//! The detail ladder of a baked item: how a source part's LOD levels and
//! switch distances merge into the item's, the `--lod-pick` size lever, and
//! the cap the client imposes (`MAX_LOD_LEVELS`).

use super::solid2::ShadedGeom;

/// `--lod-pick N [--lod-pick-min-verts V]`: keep ONE detail level of every
/// source model — level N (its geoms; a part without a level N keeps its
/// nearest) — and write no switch distances, so every tiny item draws that
/// level at every distance. `--lod-pick 0` is the bake of before 2026-09-07
/// (the nearest level everywhere); a higher N is the size lever for a map
/// Nadeo refuses to store (Summer 21 at 36 MB: HTTP 413; the visual meshes of
/// the nearest level are a third of the bytes). `min_verts`: a part whose
/// nearest level has fewer vertices keeps that level — small parts stay
/// sharp, only the heavy ones (a 50 000-vertex gate arch) go coarser.
///
/// Without it every level rides with its ladder. How the game reads the
/// ladder (measured 2026-09-07 on Summer 15's grass with `tmmaps lineup`
/// probes of a half-size GateCheckpointCenter24m, ladder [32, 64, 128]): a
/// geom draws when its mask has the current level's bit — an item whose geoms
/// all lack bit 0 is invisible near, one with level-0 geoms only is culled
/// far — and the level advances with the CAMERA distance times the game's LOD
/// bias: the level-0-only item was still drawn at 100 m and gone at 150 m,
/// i.e. the 32 m step fired between 100 and 150 m (~x4; a stock ShowLights rig
/// whose own ladder culls at 256 m was still drawn at 400 m, so the bias is
/// the game's, not ours). Both the item editor's CGameCommonItemEntityModel
/// form and the pack's CPlugPrefab form behave the same, in the editor and in
/// play. The switch distances are scaled with the geometry (the half-size
/// object subtends the same angle at half the distance); the box A/B of
/// 2026-09-08 on tiny 05 (13 views, ×0.5 vs ×1.0) showed 12 identical and one
/// differing in a distant stand's detail.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodPick {
    pub level: u32,
    pub min_verts: i32,
}

static LOD_PICK: std::sync::OnceLock<LodPick> = std::sync::OnceLock::new();

/// Install the pick for this process (the `--lod-pick` flag, once).
pub fn set_lod_pick(pick: LodPick) -> Result<(), String> {
    LOD_PICK.set(pick).map_err(|_| "--lod-pick given twice".to_string())
}

/// The pick, when one was asked for.
pub fn lod_pick() -> Option<LodPick> {
    LOD_PICK.get().copied()
}

/// One level only, no ladder (a pick is in force).
pub fn lod0_only() -> bool {
    lod_pick().is_some()
}

/// A source part's detail ladder length: one more level than it has switch
/// distances, and at least one past its highest mask bit (a mask bit with no
/// distance is the unbounded last level).
pub fn lod_levels_of(lod_max_dist: &[f32], geoms: &[ShadedGeom]) -> u32 {
    let by_dist = lod_max_dist.len() as u32 + 1;
    let by_mask = geoms.iter().map(|g| if g.lod_mask <= 0 { 1 } else { 32 - (g.lod_mask as u32).leading_zeros() }).max().unwrap_or(1);
    by_dist.max(by_mask)
}

/// Two switch distances that are the same step (the parts of one item scale
/// alike, so equal source distances stay equal; the slack absorbs float noise).
fn same_dist(a: f32, b: f32) -> bool {
    if a.is_infinite() || b.is_infinite() {
        return a == b;
    }
    (a - b).abs() <= 1e-3 * a.abs().max(b.abs()).max(1.0)
}

/// `d` merged into a sorted, deduplicated ladder.
pub fn merge_lod_ladder(ladder: &mut Vec<f32>, d: &[f32]) {
    for x in d {
        if !x.is_finite() || *x <= 0.0 || ladder.iter().any(|y| same_dist(*x, *y)) {
            continue;
        }
        let at = ladder.iter().position(|y| *y > *x).unwrap_or(ladder.len());
        ladder.insert(at, *x);
    }
}

/// A part's mask moved onto the merged ladder by DISTANCE RANGE: the part's
/// level k spans `part[k-1]..part[k]` (0 below the first distance, unbounded
/// past the last), and every merged level whose span STARTS inside that
/// range takes the bit. The merged ladder normally holds every distance of
/// every part, so the spans tile exactly: a part keeps drawing precisely
/// where its own ladder drew it, whatever the other parts' steps (Flag16m: a
/// pole with [16, 64, 512] and a cloth with [16, 64, 128, 512] merge into
/// [16, 64, 128, 512] with the pole's third level on bits 2 and 3 — the
/// per-level max of before made a zero-width level [16, 64, 512, 512]). When
/// the ladder was capped (`cap_lod_ladder`) a merged level may straddle a
/// part's step; the level active at its start is drawn through it. A part
/// without a ladder, or a mask of 0, draws at every level; a part whose
/// ladder is longer than its masks (Nadeo's cull idiom: Sparkler8m lists
/// [16, 128, 256] but draws nothing past bit 2) stays culled past its last
/// distance, since no level of it starts there.
pub fn remap_lod_mask(mask: u32, part: &[f32], merged: &[f32]) -> u32 {
    let levels = merged.len() + 1;
    let all = if levels >= 32 { u32::MAX } else { (1u32 << levels) - 1 };
    if mask == 0 || part.is_empty() {
        return all;
    }
    let mut out = 0u32;
    for k in 0..32usize {
        if mask & (1 << k) == 0 {
            continue;
        }
        let lo = if k == 0 { 0.0 } else { part.get(k - 1).copied().unwrap_or(f32::INFINITY) };
        let hi = part.get(k).copied().unwrap_or(f32::INFINITY);
        if lo >= hi {
            continue;
        }
        for j in 0..levels {
            let jlo = if j == 0 { 0.0 } else { merged[j - 1] };
            let starts_inside = (jlo > lo || same_dist(jlo, lo)) && jlo < hi && !same_dist(jlo, hi);
            if starts_inside {
                out |= 1 << j;
            }
        }
    }
    out
}

/// The most detail levels a static object's Solid2 may carry. Every pack
/// static model surveyed (93 laddered Solid2s, 2026-09-07) has at most 3
/// switch distances = 4 levels; the one 5-level model is a dyna object's
/// mesh (Flag.Mesh.Gbx, [16, 64, 128, 512]). A static item merged to 5
/// levels (Flag16m: pole + cloth, [8, 32, 64, 256]) crashed the client at
/// map load with an assertion (ud2 at Trackmania.exe+0x1e9947, rax = 5,
/// r9 = 4) — twice, once per ladder variant — so the merged ladder is capped
/// at 4 levels.
pub const MAX_LOD_LEVELS: usize = 4;

/// Collapse a ladder to at most `max_dists` switch distances: while it is
/// longer, the two closest adjacent steps (smallest ratio) become one, the
/// larger distance dropped — the finer level then draws on through the
/// removed step (see `remap_lod_mask`), which costs a little detail budget
/// rather than any geometry.
pub fn cap_lod_ladder(ladder: &mut Vec<f32>, max_dists: usize) {
    while ladder.len() > max_dists && ladder.len() >= 2 {
        let mut best = 1usize;
        let mut best_ratio = f32::INFINITY;
        for i in 1..ladder.len() {
            let r = ladder[i] / ladder[i - 1].max(1e-6);
            if r < best_ratio {
                best_ratio = r;
                best = i;
            }
        }
        ladder.remove(best);
    }
}


#[cfg(test)]
mod lod_tests {
    use super::*;

    fn geom(mask: i32) -> ShadedGeom {
        ShadedGeom { visual_index: 0, material_index: 0, u01: -1, lod_mask: mask, u02: 0 }
    }

    #[test]
    fn ladder_length_is_distances_plus_one_or_highest_bit() {
        // RoadTech Straight_Air: [64, 128] with masks 1/2/4
        assert_eq!(lod_levels_of(&[64.0, 128.0], &[geom(1), geom(2), geom(4)]), 3);
        // Sparkler8m: [16, 128, 256] with masks 1/2/4 only — the 4th level is
        // empty (culled past 256 m)
        assert_eq!(lod_levels_of(&[16.0, 128.0, 256.0], &[geom(1), geom(2), geom(4)]), 4);
        // a mask bit past the distances counts as an unbounded last level
        assert_eq!(lod_levels_of(&[64.0], &[geom(1), geom(2), geom(4)]), 3);
        // no ladder at all
        assert_eq!(lod_levels_of(&[], &[geom(1), geom(1)]), 1);
        assert_eq!(lod_levels_of(&[], &[geom(0)]), 1);
    }

    #[test]
    fn masks_move_onto_the_merged_ladder_by_range() {
        // a 3-level part [32, 64] on a 4-level item [32, 64, 128]: its last
        // level (past 64) spans bits 2 and 3
        assert_eq!(remap_lod_mask(1, &[32.0, 64.0], &[32.0, 64.0, 128.0]), 1);
        assert_eq!(remap_lod_mask(2, &[32.0, 64.0], &[32.0, 64.0, 128.0]), 2);
        assert_eq!(remap_lod_mask(4, &[32.0, 64.0], &[32.0, 64.0, 128.0]), 4 | 8);
        // a 2-level part [32] on the same item
        assert_eq!(remap_lod_mask(2, &[32.0], &[32.0, 64.0, 128.0]), 2 | 4 | 8);
        // Flag16m: pole [8, 32, 256] + cloth [8, 32, 64, 256] -> [8, 32, 64, 256];
        // the pole's third level (32..256) spans bits 2 and 3, its fourth
        // (past 256) bit 4
        let mut merged = Vec::new();
        merge_lod_ladder(&mut merged, &[8.0, 32.0, 256.0]);
        merge_lod_ladder(&mut merged, &[8.0, 32.0, 64.0, 256.0]);
        assert_eq!(merged, vec![8.0, 32.0, 64.0, 256.0]);
        assert_eq!(remap_lod_mask(4, &[8.0, 32.0, 256.0], &merged), 4 | 8);
        assert_eq!(remap_lod_mask(8, &[8.0, 32.0, 256.0], &merged), 16);
        assert_eq!(remap_lod_mask(8, &[8.0, 32.0, 64.0, 256.0], &merged), 8);
        // a culled part (Sparkler8m: [8, 64, 128], geoms up to bit 2) stays
        // culled past 128 on a longer ladder
        assert_eq!(remap_lod_mask(4, &[8.0, 64.0, 128.0], &[8.0, 64.0, 128.0, 256.0]), 4);
        // a one-level part, or mask 0: every level
        assert_eq!(remap_lod_mask(1, &[], &[32.0, 64.0]), 7);
        assert_eq!(remap_lod_mask(0, &[32.0], &[32.0, 64.0]), 7);
        // a multi-bit mask
        assert_eq!(remap_lod_mask(7, &[32.0, 64.0], &[32.0, 64.0, 128.0]), 15);
        // same ladder: unchanged
        assert_eq!(remap_lod_mask(4, &[32.0, 64.0], &[32.0, 64.0]), 4);
        // merging ignores duplicates and keeps the order
        let mut l = vec![32.0, 128.0];
        merge_lod_ladder(&mut l, &[64.0, 128.0, 32.0]);
        assert_eq!(l, vec![32.0, 64.0, 128.0]);
    }

    #[test]
    fn ladders_are_capped_at_four_levels() {
        // Flag16m's union [8, 32, 64, 256]: the closest step pair is
        // (32, 64), the larger goes
        let mut l = vec![8.0, 32.0, 64.0, 256.0];
        cap_lod_ladder(&mut l, MAX_LOD_LEVELS - 1);
        assert_eq!(l, vec![8.0, 32.0, 256.0]);
        // the cloth [8, 32, 64, 256] on the capped ladder: its level 2
        // (32..64) is active at the start of the merged level 32..256, so it
        // draws through it; level 3 (64..256) is never drawn
        assert_eq!(remap_lod_mask(4, &[8.0, 32.0, 64.0, 256.0], &l), 4);
        assert_eq!(remap_lod_mask(8, &[8.0, 32.0, 64.0, 256.0], &l), 0);
        assert_eq!(remap_lod_mask(16, &[8.0, 32.0, 64.0, 256.0], &l), 8);
        // the pole [8, 32, 256] is exact on it
        assert_eq!(remap_lod_mask(4, &[8.0, 32.0, 256.0], &l), 4);
        assert_eq!(remap_lod_mask(8, &[8.0, 32.0, 256.0], &l), 8);
        // a short ladder is left alone
        let mut s = vec![32.0, 64.0];
        cap_lod_ladder(&mut s, 3);
        assert_eq!(s, vec![32.0, 64.0]);
    }
}

