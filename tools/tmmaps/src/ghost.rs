//! The one thing map surgery needs out of a `.Ghost.Gbx`: the checkpoint
//! splits the ghost DECLARES, which are the ground truth every segment map and
//! every ladder rung is checked against.
//!
//! **The chunk walk is `tools/ghost`'s, not ours.** This used to be a 43-line
//! copy of the `0x0309202B` walk, and a 43-line copy is exactly the shape of
//! the bug this audit is about: two implementations of one format, agreeing
//! until the day they do not. `ghost` owns the ghost format; `tmmaps` owns
//! `.Map.Gbx`. The cost is that this crate is no longer dependency-free, and
//! that is the right price.
//!
//! What is left here is the *interpretation*, because
//! `ghost::Container::splits()` returns the chunk's **raw u32 array** rather
//! than the checkpoint list. On the map-1 WR that is
//! `[1, 19538, 0, 0, 3, 4, 7617, 2, 13308, 4, 16316, 0, 19538, 1, 4294967295]`
//! — fifteen words of which four are the splits. Swapping the copy out for the
//! call surfaced the difference immediately, because the segment builder
//! refused: *"the map declares 3 checkpoints, so the ghost should declare 4
//! splits; it declares 15"*. That refusal is the control for this change, and
//! it fired the first time.
//!
//! Layout, from the project's own measurements:
//! `[version, race_time, ?, ?, n_checkpoints, count, (time, tag) × count, …]`

/// The checkpoint splits a ghost declares, in driving order, in milliseconds —
/// the checkpoints and then the finish.
///
/// `None` when the file carries no split chunk, which is a real state (some
/// synthesised containers have none) and not an error.
pub fn splits(path: &std::path::Path) -> Option<Vec<u32>> {
    let c = ghost::Container::load(&path.to_string_lossy()).ok()?;
    decode(&c.splits())
}

/// Pull the checkpoint list out of the chunk's raw word array.
///
/// Refuses rather than guessing when the count word and the array length
/// disagree: a short array here would silently produce a shorter list of
/// splits, and the segment builder would then verify each segment against the
/// wrong checkpoint.
pub fn decode(raw: &[u32]) -> Option<Vec<u32>> {
    if raw.len() < 6 {
        return None;
    }
    let count = raw[5] as usize;
    if count == 0 || raw.len() < 6 + 2 * count {
        return None;
    }
    Some((0..count).map(|k| raw[6 + 2 * k]).collect())
}

#[cfg(test)]
mod tests {
    use super::decode;

    /// The map-1 WR's real chunk, word for word.
    const WR: &[u32] =
        &[1, 19538, 0, 0, 3, 4, 7617, 2, 13308, 4, 16316, 0, 19538, 1, 4294967295];

    #[test]
    fn decodes_the_checkpoint_list() {
        assert_eq!(decode(WR), Some(vec![7617, 13308, 16316, 19538]));
    }

    #[test]
    fn refuses_a_short_array() {
        // count says 4, the array holds two pairs: refuse rather than return
        // a two-entry list the segment builder would happily verify against.
        assert_eq!(decode(&WR[..10]), None);
        assert_eq!(decode(&[1, 19538, 0, 0, 3]), None);
    }
}
