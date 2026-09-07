//! `tmr` — the reachability model R of the route project (MODEL arm).
//!
//! R(state, target, local geometry, h) → (P(reach), expected ticks, arrival
//! band). Features are car-frame (`features`), the trainer is candle (`train`),
//! the planner's copy is flat f32 (`net::Weights`, checked against candle by
//! `Weights::agrees_with`), the artefact is `.tmw` (INTERFACES §4), and the
//! evaluation is the held-out two-gate test with its distance control (`eval`).

pub mod data;
pub mod estimator;
pub mod eval;
pub mod features;
pub mod frame;
pub mod net;
pub mod train;

/// Milliseconds as seconds with a decimal.
pub fn secs(ms: i64) -> String {
    tmreach::secs(ms)
}
