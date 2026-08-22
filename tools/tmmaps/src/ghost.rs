//! The one thing map surgery needs out of a `.Ghost.Gbx`: the checkpoint
//! splits the ghost DECLARES, which are the ground truth every segment map and
//! every ladder rung is checked against.
//!
//! **There is no decoding here, and there used to be twice.** This file was a
//! 43-line copy of the `0x0309202B` chunk walk; replacing that copy with a call
//! to `tools/ghost` is what found the defect the call itself then inherited --
//! `Container::splits()` handed back the chunk's fifteen RAW words, so this
//! module kept a second-stage decoder to turn them into a checkpoint list, with
//! the layout written down here for the third time in the tree.
//!
//! Both stages now live once, in `gbx::container::GhostResult`, and
//! `Container::splits()` returns the checkpoint list. What is left here is the
//! part that is genuinely tmmaps': a missing or unreadable result chunk is
//! `None` rather than an empty list, because a segment builder that reads an
//! empty list as "zero checkpoints" verifies its segments against nothing.

/// The checkpoint splits a ghost declares, in driving order, in milliseconds —
/// the checkpoints and then the finish.
///
/// `None` when the file cannot be read or carries no split chunk, which is a
/// real state (some synthesised containers have none) and not an error.
pub fn splits(path: &std::path::Path) -> Option<Vec<i32>> {
    let c = ghost::Container::load(&path.to_string_lossy()).ok()?;
    let s = c.splits();
    if s.is_empty() {
        return None;
    }
    Some(s)
}
