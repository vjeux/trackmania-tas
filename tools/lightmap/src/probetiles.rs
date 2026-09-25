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
//! 2. MERGE, up to 3 passes while something merged, axis x then z (flag bit 1 << axis on the anchor entry, never
//!    cleared): for member A (entry unflagged for the axis) and the first member B ≠ A (entry unflagged) with equal
//!    cell.x and cell.z (1e-5 · max(1, |v|)), the SAME range on the other axis, A.start ≤ B.start ≤ A.end and
//!    A.end == B.start + 2 (the two shared margin probes): A.end = B.end; member(B).d = B.start − A.start along the
//!    axis and member(B).entry = A; members that pointed at B (B had merged) move to A with their offset shifted;
//!    B's entry is emptied (x0 = x1 = 0). One merge per anchor per axis pass.
//! 3. Tiles: entries in order with x1 ≠ x0 → (w = x1 − x0, h = z1 − z0). Packer: Σ area (int) → W = H =
//!    (int)sqrtf(Σ); stable radix order by area, placed from the LARGEST (equal areas: reverse entry order) into
//!    `Packer::new(W, H)` (node cap 4·n); a failed insert grows the smaller side by max(1, ceil(rest / other)) —
//!    rest = the areas not yet placed — and restarts; done: W = max(x + w), H = max(y + h) (no pow2 here).
//! 4. slices[member.slice] = (tile.x + dx, tile.y + dz).
//!
//! tiny 16's editor save (8 blocks, 62 stored levels, image 198 × 194) and pwc-day's (8 slices, 21 × 21): every
//! slice pair bit-identical (`lmtool probe-slices EDITOR.Map.Gbx`).

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

/// Study switch: grow by the sum of the unplaced areas instead of the failed tile's area.
pub static GROW_REMAINING_SUM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The packer of FUN_140452f70: (W, H, positions).
pub fn pack_tiles(tiles: &[(u32, u32)]) -> (u32, u32, Vec<(u32, u32)>) {
    let n = tiles.len();
    if n == 0 {
        return (0, 0, Vec::new());
    }
    let areas: Vec<u32> = tiles.iter().map(|&(w, h)| w * h).collect();
    let total: u32 = areas.iter().sum();
    // W0 = H0 = (int)ceilf(sqrtf(Σ area) · 1.1) (asm 0x140453000–0x14045303c; 0x141d1f420 = 1.1, 0x1419022e0 = ceilf)
    let s0 = (total as f32).sqrt() * 1.1f32;
    let s = match std::env::var("LMTOOL_TILES_ROUND").ok().as_deref() { Some("floor") => s0.floor(), Some("round") => s0.round(), _ => s0.ceil() } as u32;
    let (mut w_bin, mut h_bin) = (s, s);
    // stable ascending order by area (LSD radix on the u32 = a stable sort)
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| areas[i]);
    if std::env::var_os("LMTOOL_TILES_FWD").is_some() { order.sort_by_key(|&i| (areas[i], std::cmp::Reverse(i))); }
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
        // the growth term: the FAILED tile's area (pwc-day: 19×19 → +3 → 22×19 → +3 → 22×22 → the saved 21×21;
        // the sum of the remaining areas would give 30×19 → 28×14)
        let mode = std::env::var("LMTOOL_TILES_GROW").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(if GROW_REMAINING_SUM.load(std::sync::atomic::Ordering::Relaxed) { 1 } else { 1 });
        let rest: u32 = match mode { 1 => (placed..n).map(|k| areas[order[n - 1 - k]]).sum(), 2 => 0, _ => areas[order[n - 1 - placed]] };
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
                let bit = 1u32 << axis;
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

/// Re-tile a saved trailer from its own stored-level pattern and compare with its slices: (W, H, differences).
pub fn check_against(v: &crate::volume::Volume) -> (u32, u32, Vec<String>) {
    let stored: Vec<Vec<bool>> = v.blocks.iter().map(|b| b.slices.iter().map(|s| s.is_some()).collect()).collect();
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
