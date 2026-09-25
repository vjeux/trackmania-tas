//! THE PROBE IMAGE TILING — the trailer's per-block per-level `slices` (x, y) and the 2D probe image size
//! (RE 7, 2026-09-25 21:40Z; FUN_1402814f0 with the knob DAT_14205c80c ≠ 0 → the merging path; the rect packer
//! FUN_1404532a0 → FUN_140452f70 over the game's binary-tree node packer FUN_140492930 = `pack::Packer`).
//!
//! 1. Global probe coordinates: with record 0's phase f = frac⁺(pos0 · inv0) per axis (modff, +1 when negative),
//!    every record gets g = lroundf((pos − f · cell) · inv) — its atlas index 0 in world probe units. An entry per
//!    (record, level y ∈ [amin.y, amax.y)) whose voxels are not all zero: {x0 = g.x + amin.x, z0 = g.z + amin.z,
//!    x1 = g.x + amax.x, z1 = g.z + amax.z, cell.x, cell.z, tile −1, flags 0}; the level's group key = g.y + y
//!    (groups in first-appearance order; members {slice index, entry, dx 0, dz 0} in record order). An empty level
//!    writes (−1, −1).
//! 2. MERGE, up to 3 passes while something merged, axis x then z (flag bit 1 << (2·pass + axis) on the anchor
//!    entry, never cleared — one merge per anchor per axis per pass, chains merge over the passes): for member A
//!    (entry unflagged for the bit) and the first member B ≠ A (entry unflagged) with equal
//!    cell.x and cell.z (1e-5 · max(1, |v|)), the SAME range on the other axis, A.start ≤ B.start ≤ A.end and
//!    A.end == B.start + 2 (the two shared margin probes): A.end = B.end; member(B).d = B.start − A.start along the
//!    axis and member(B).entry = A; members that pointed at B (B had merged) move to A with their offset shifted;
//!    B's entry is emptied (x0 = x1 = 0). One merge per anchor per axis pass.
//! 3. Tiles: entries in order with x1 ≠ x0 → (w = x1 − x0, h = z1 − z0). Packer (FUN_140452f70): Σ area (int) →
//!    W = H = (int)ceilf(sqrtf(Σ) · 1.1); stable radix order by area, placed from the LARGEST (equal areas: reverse
//!    entry order) into the binary-tree node packer `pack::Packer` (growable node array — no 4·n cap here); a failed
//!    insert grows the side that is not larger (W when W ≤ H, else H) by max(1, ceil(Σ unplaced areas / other side))
//!    and restarts; done: W = max(x + w), H = max(y + h) (the pow2 rounding is off for this caller).
//! 4. slices[member.slice] = (tile.x + dx, tile.y + dz).
//!
//! Every editor save at hand — pwc-day (21 × 21), hill4 (20 × 20), the three tiny 16 variants (8 blocks, 65 stored
//! levels, 198 × 194) and both giant20x2 bakes (44 blocks, 458 stored levels, 579 × 579) — reproduces every slice
//! pair and the image size (`lmtool probe-slices EDITOR.Map.Gbx…`).

use crate::pack::Packer;
use crate::volume::Block;

fn frac_pos(v: f32) -> f32 {
    let f = v - v.trunc();
    if f < 0.0 { f + 1.0 } else { f }
}

fn lroundf(v: f32) -> i32 {
    (v.abs() + 0.5).floor().copysign(v) as i32
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    lo: [i32; 2],
    hi: [i32; 2],
    cell: [f32; 2],
    tile: i32,
    flags: u32,
}

#[derive(Clone, Copy, Debug)]
struct Member {
    slice: usize,
    entry: usize,
    d: [i32; 2],
}

struct Group {
    key: i32,
    members: Vec<Member>,
}

/// The packer of FUN_140452f70: (W, H, positions).
pub fn pack_tiles(tiles: &[(u32, u32)]) -> (u32, u32, Vec<(u32, u32)>) {
    let n = tiles.len();
    if n == 0 {
        return (0, 0, Vec::new());
    }
    let areas: Vec<u32> = tiles.iter().map(|&(w, h)| w * h).collect();
    let total: u32 = areas.iter().sum();
    // W0 = H0 = (int)ceilf(sqrtf(Σ area) · 1.1) (asm 0x140453000–0x14045303c; 0x141d1f420 = 1.1, 0x1419022e0 = ceilf)
    let s = ((total as f32).sqrt() * 1.1f32).ceil() as u32;
    let (mut w_bin, mut h_bin) = (s, s);
    // stable ascending order by area (LSD radix on the u32 = a stable sort)
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| areas[i]);
    let mut pos = vec![(0u32, 0u32); n];
    loop {
        let mut packer = Packer::new(w_bin.min(u16::MAX as u32) as u16, h_bin.min(u16::MAX as u32) as u16);
        let mut placed = 0usize;
        for k in 0..n {
            let i = order[n - 1 - k];
            let r = packer.insert(0, tiles[i].0 as u16, tiles[i].1 as u16);
            if r < 0 {
                break;
            }
            let nd = packer.nodes[r as usize];
            pos[i] = (nd.x as u32, nd.y as u32);
            placed += 1;
        }
        if std::env::var_os("LMTOOL_TILES_TRACE").is_some() { eprintln!("  pack_tiles: bin {w_bin}×{h_bin}: placed {placed} of {n}"); }
        if placed == n {
            let mut w_out = 0u32;
            let mut h_out = 0u32;
            for (i, &(x, y)) in pos.iter().enumerate() {
                w_out = w_out.max(x + tiles[i].0);
                h_out = h_out.max(y + tiles[i].1);
            }
            return (w_out, h_out, pos);
        }
        // the growth term: the areas not yet placed (asm 0x140453100–0x140453178), divided by the other side
        let rest: u32 = (placed..n).map(|k| areas[order[n - 1 - k]]).sum();
        if h_bin < w_bin {
            h_bin += ((w_bin - 1 + rest) / w_bin).max(1);
        } else {
            w_bin += ((h_bin - 1 + rest) / h_bin).max(1);
        }
    }
}

/// The slices of the trailer for `blocks` (their +0..+0x3c fields) given which levels carry content
/// (`stored[b][y − min.y]`): returns (image W, H, per block the (x, y) or None per level).
pub fn probe_slices(blocks: &[Block], stored: &[Vec<bool>]) -> (u32, u32, Vec<Vec<Option<(u32, u32)>>>) {
    if blocks.is_empty() {
        return (0, 0, Vec::new());
    }
    let b0 = &blocks[0];
    let inv = |b: &Block| [1.0 / b.cell[0], 1.0 / b.cell[1], 1.0 / b.cell[2]];
    let inv0 = inv(b0);
    let f = [frac_pos(b0.pos[0] * inv0[0]), frac_pos(b0.pos[1] * inv0[1]), frac_pos(b0.pos[2] * inv0[2])];
    let mut entries: Vec<Entry> = Vec::new();
    let mut groups: Vec<Group> = Vec::new();
    let mut slices: Vec<Vec<Option<(u32, u32)>>> = Vec::new();
    let mut slice_idx = 0usize;
    for (bi, b) in blocks.iter().enumerate() {
        let iv = inv(b);
        let g = [
            lroundf((b.pos[0] - f[0] * b.cell[0]) * iv[0]),
            lroundf((b.pos[1] - f[1] * b.cell[1]) * iv[1]),
            lroundf((b.pos[2] - f[2] * b.cell[2]) * iv[2]),
        ];
        let mut sl = Vec::new();
        for (li, y) in (b.min[1]..b.max[1]).enumerate() {
            let has = stored.get(bi).and_then(|v| v.get(li)).copied().unwrap_or(false);
            if has {
                let key = g[1] + y as i32;
                let gi = match groups.iter().position(|gr| gr.key == key) {
                    Some(i) => i,
                    None => {
                        groups.push(Group { key, members: Vec::new() });
                        groups.len() - 1
                    }
                };
                let ei = entries.len();
                entries.push(Entry {
                    lo: [g[0] + b.min[0] as i32, g[2] + b.min[2] as i32],
                    hi: [g[0] + b.max[0] as i32, g[2] + b.max[2] as i32],
                    cell: [b.cell[0], b.cell[2]],
                    tile: -1,
                    flags: 0,
                });
                groups[gi].members.push(Member { slice: slice_idx, entry: ei, d: [0, 0] });
                sl.push(Some((0, 0)));
            } else {
                sl.push(None);
            }
            slice_idx += 1;
        }
        slices.push(sl);
    }
    // the merge passes
    let close = |a: f32, b: f32| -> bool { (a - b).abs() <= a.abs().max(b.abs()).max(1.0) * 1e-5 };
    for gr in groups.iter_mut() {
        let mut pass = 0;
        loop {
            let mut merged = 0;
            for axis in 0..2usize {
                // the flag bit is per (pass, axis): bit = 1 << (2·pass + axis) — so an anchor merges once per axis per
                // pass and chains of blocks merge over the passes (the giant's 4-chains: pass 0 pairs, pass 1 the pairs)
                let bit = 1u32 << (2 * pass + axis);
                let other = 1 - axis;
                let n = gr.members.len();
                for ai in 0..n {
                    let ea = gr.members[ai].entry;
                    if entries[ea].flags & bit != 0 {
                        continue;
                    }
                    for bi in 0..n {
                        if bi == ai {
                            continue;
                        }
                        let eb = gr.members[bi].entry;
                        if eb == ea || entries[eb].flags & bit != 0 {
                            continue;
                        }
                        let (a, b) = (entries[ea], entries[eb]);
                        if !close(a.cell[0], b.cell[0]) || !close(a.cell[1], b.cell[1]) {
                            continue;
                        }
                        if a.lo[other] != b.lo[other] || a.hi[other] != b.hi[other] {
                            continue;
                        }
                        if b.lo[axis] < a.lo[axis] || a.hi[axis] < b.lo[axis] || a.hi[axis] != b.lo[axis] + 2 {
                            continue;
                        }
                        // merge B into A
                        let shift = b.lo[axis] - a.lo[axis];
                        entries[ea].hi[axis] = b.hi[axis];
                        entries[ea].flags |= bit;
                        gr.members[bi].entry = ea;
                        gr.members[bi].d[axis] = shift;
                        if b.flags != 0 {
                            for m in gr.members.iter_mut() {
                                if m.entry == eb {
                                    m.entry = ea;
                                    m.d[axis] += shift;
                                }
                            }
                        }
                        entries[eb] = Entry { lo: [0, 0], hi: [0, 0], cell: b.cell, tile: -1, flags: b.flags };
                        merged += 1;
                        break;
                    }
                }
            }
            pass += 1;
            if merged == 0 || pass >= 3 {
                break;
            }
        }
    }
    // tiles
    let mut tiles: Vec<(u32, u32)> = Vec::new();
    for e in entries.iter_mut() {
        if e.hi[0] == e.lo[0] {
            e.tile = -1;
        } else {
            e.tile = tiles.len() as i32;
            tiles.push(((e.hi[0] - e.lo[0]) as u32, (e.hi[1] - e.lo[1]) as u32));
        }
    }
    if std::env::var_os("LMTOOL_TILES_TRACE").is_some() {
        eprintln!("probetiles: {} entries, {} tiles: {:?}", entries.len(), tiles.len(), tiles);
        for (gi, gr) in groups.iter().enumerate() { eprintln!("  group {gi} key {}: {:?}", gr.key, gr.members.iter().map(|m| (m.slice, m.entry, m.d)).collect::<Vec<_>>()); }
    }
    if RECORD_STRUCTURE.load(std::sync::atomic::Ordering::Relaxed) {
        let mut st: Vec<((u32, u32), Vec<(usize, i32, i32)>)> = tiles.iter().map(|&t| (t, Vec::new())).collect();
        for gr in &groups { for m in &gr.members { let e = &entries[m.entry]; if e.tile >= 0 { st[e.tile as usize].1.push((m.slice, m.d[0], m.d[1])); } } }
        TILE_STRUCTURE.with(|c| *c.borrow_mut() = st);
    }
    let (w, h, pos) = pack_tiles(&tiles);
    if std::env::var_os("LMTOOL_TILES_TRACE").is_some() { eprintln!("probetiles: image {w}×{h}; positions {:?}", pos); }
    // the slices
    let mut flat: Vec<Option<(u32, u32)>> = slices.iter().flatten().copied().collect();
    for gr in &groups {
        for m in &gr.members {
            let e = &entries[m.entry];
            if e.tile >= 0 {
                let p = pos[e.tile as usize];
                flat[m.slice] = Some(((p.0 as i32 + m.d[0]) as u32, (p.1 as i32 + m.d[1]) as u32));
            }
        }
    }
    let mut k = 0usize;
    for sl in slices.iter_mut() {
        for v in sl.iter_mut() {
            *v = flat[k];
            k += 1;
        }
    }
    (w, h, slices)
}

/// The tile structure (for the study): per tile (w, h) and its members as (slice index, dx, dz).
pub fn tile_structure(blocks: &[Block], stored: &[Vec<bool>]) -> Vec<((u32, u32), Vec<(usize, i32, i32)>)> {
    // re-run the grouping part of probe_slices with a recording hook
    TILE_STRUCTURE.with(|c| c.borrow_mut().clear());
    RECORD_STRUCTURE.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = probe_slices(blocks, stored);
    RECORD_STRUCTURE.store(false, std::sync::atomic::Ordering::Relaxed);
    TILE_STRUCTURE.with(|c| c.borrow().clone())
}
thread_local! { static TILE_STRUCTURE: std::cell::RefCell<Vec<((u32, u32), Vec<(usize, i32, i32)>)>> = std::cell::RefCell::new(Vec::new()); }
static RECORD_STRUCTURE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Re-tile a saved trailer from its own stored-level pattern and compare with its slices: (W, H, differences).
pub fn check_against(v: &crate::volume::Volume) -> (u32, u32, Vec<String>) {
    let stored: Vec<Vec<bool>> = v.blocks.iter().map(|b| b.slices.iter().map(|s| s.is_some()).collect()).collect();
    if std::env::var_os("LMTOOL_TILES_STRUCT").is_some() {
        // does the save's layout agree with our tile structure? every member of a tile must sit at the save's tile origin + d
        let flat: Vec<Option<(u32, u32)>> = v.blocks.iter().flat_map(|b| b.slices.iter().copied()).collect();
        let st = tile_structure(&v.blocks, &stored);
        let mut bad = 0;
        for (ti, ((w, h), members)) in st.iter().enumerate() {
            let origins: Vec<Option<(i32, i32)>> = members.iter().map(|&(si, dx, dz)| flat[si].map(|(x, y)| (x as i32 - dx, y as i32 - dz))).collect();
            let first = origins[0];
            if origins.iter().any(|o| *o != first) { bad += 1; eprintln!("  tile {ti} {w}×{h}: members {:?} → save origins {:?}", members, origins); }
        }
        eprintln!("tile structure: {} tiles, {bad} inconsistent with the save", st.len());
        // the save's tiles in placement-ish order: by (y, x) of the tile origin
        let mut rows: Vec<(i32, i32, u32, u32, usize)> = st.iter().enumerate().filter_map(|(ti, ((w, h), members))| { let (si, dx, dz) = members[0]; flat[si].map(|(x, y)| (y as i32 - dz, x as i32 - dx, *w, *h, ti)) }).collect();
        rows.sort();
        for r in rows.iter().take(40) { eprintln!("  save tile at ({}, {}) {}×{} area {} [tile {}]", r.1, r.0, r.2, r.3, r.2 * r.3, r.4); }
        if let Some(t) = std::env::var("LMTOOL_TILES_SHOW").ok() {
            let mut cum = 0usize;
            let mut owner: Vec<(usize, usize)> = Vec::new(); // slice → (block, level)
            for (bi, b) in v.blocks.iter().enumerate() { for li in 0..b.slices.len() { owner.push((bi, b.min[1] as usize + li)); cum += 1; } }
            let _ = cum;
            for ti in t.split(',').filter_map(|x| x.parse::<usize>().ok()) {
                let ((w, h), members) = &st[ti];
                eprintln!("  tile {ti} {w}×{h}: {:?}", members.iter().map(|&(si, dx, dz)| { let (bi, lv) = owner[si]; let b = &v.blocks[bi]; format!("block {bi} (min {:?} max {:?} pos {:?}) level {lv} d ({dx},{dz}) save {:?}", b.min, b.max, b.pos, flat[si]) }).collect::<Vec<_>>());
            }
        }
        let mut sizes: std::collections::BTreeMap<(u32, u32), usize> = std::collections::BTreeMap::new();
        for ((w, h), _) in &st { *sizes.entry((*w, *h)).or_default() += 1; }
        eprintln!("  sizes: {:?}", sizes);
        // also: the save's tiles that we did not merge — pairs of slices at the same y whose x differ by w−2
    }
    let (w, h, ours) = probe_slices(&v.blocks, &stored);
    let mut d = Vec::new();
    for (bi, b) in v.blocks.iter().enumerate() {
        for (li, s) in b.slices.iter().enumerate() {
            let o = ours[bi][li];
            if *s != o {
                d.push(format!("block {bi} level {}: save {:?} ours {:?}", b.min[1] + li as u32, s, o));
            }
        }
    }
    let cells = ((w + 3) / 4) * ((h + 3) / 4);
    if cells as usize != v.cell4.len() {
        d.push(format!("image {w}×{h} → {} 4×4 cells, the save has {}", cells, v.cell4.len()));
    }
    (w, h, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pwc_day_eight_levels_of_7x7_pack_into_21x21_in_the_saved_order() {
        let b = Block { origin: [0, 0, 0], min: [22, 4, 20], max: [29, 12, 27], cell: [16.0; 3], pos: [472.0, -46.0, -8.0], slices: Vec::new() };
        let (w, h, s) = probe_slices(&[b], &[vec![true; 8]]);
        assert_eq!((w, h), (21, 21));
        let want = [(7, 14), (14, 7), (7, 7), (0, 14), (0, 7), (14, 0), (7, 0), (0, 0)];
        assert_eq!(s[0], want.iter().map(|&p| Some(p)).collect::<Vec<_>>());
    }

    #[test]
    fn a_four_chain_merges_over_two_passes() {
        // the giant's blocks 9, 10, 11, 12 at one level: pairs in pass 0, the pairs in pass 1 → one 122-wide tile with
        // the members at 0 / 30 / 60 / 90 (the save's (0,0), (30,0), (60,0), (90,0))
        let mk = |min: [u32; 3], max: [u32; 3], pos: [f32; 3]| Block { origin: [0; 3], min, max, cell: [16.0; 3], pos, slices: Vec::new() };
        let b9 = mk([128, 19, 0], [160, 32, 32], [-1576.0, -374.0, 952.0]);
        let b10 = mk([0, 35, 0], [32, 48, 32], [952.0, -630.0, 952.0]);
        let b11 = mk([32, 34, 0], [64, 48, 32], [920.0, -630.0, 952.0]);
        let b12 = mk([64, 32, 0], [96, 48, 32], [888.0, -630.0, 952.0]);
        let one = |n: usize, k: usize| { let mut v = vec![false; n]; v[k] = true; v };
        let (w, h, s) = probe_slices(&[b9, b10, b11, b12], &[one(13, 12), one(13, 12), one(14, 13), one(16, 15)]);
        assert_eq!((w, h), (122, 32));
        assert_eq!((s[0][12], s[1][12], s[2][13], s[3][15]), (Some((0, 0)), Some((30, 0)), Some((60, 0)), Some((90, 0))));
    }

    #[test]
    fn two_x_adjacent_blocks_merge_into_one_44_wide_tile() {
        // tiny 16's blocks 4 and 5 at level 45
        let b4 = Block { origin: [0, 32, 0], min: [0, 34, 0], max: [32, 46, 32], cell: [16.0; 3], pos: [952.0, -558.0, 472.0], slices: Vec::new() };
        let b5 = Block { origin: [32, 32, 0], min: [32, 34, 0], max: [46, 46, 32], cell: [16.0; 3], pos: [920.0, -558.0, 472.0], slices: Vec::new() };
        let mut st = vec![false; 12];
        st[11] = true;
        let (w, h, s) = probe_slices(&[b4.clone(), b5], &[st.clone(), st]);
        assert_eq!((w, h), (44, 32));
        assert_eq!(s[0][11], Some((0, 0)));
        assert_eq!(s[1][11], Some((30, 0)));
    }
}
