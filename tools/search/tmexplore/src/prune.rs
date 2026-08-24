//! The no-progress prune, in the library so its test tests the real rule.
//!
//! **"No new station for a while" is not a dead run in this game.** The
//! community's own cutoff is 2.000 s of no ground progress, and a TM2020
//! launch exceeds it: the car is airborne, making no arc-length progress, and
//! doing exactly the right thing. The prior attempt's map has a 3.5 s, 259 m
//! flight that dominates a whole sector.
//!
//! So the condition is a CONJUNCTION, and the second conjunct is the whole
//! point: no new station **and** the car is on the ground.

/// Should this rollout be abandoned?
pub fn should_prune(ticks_since_new_station: u32, wheels: u8, limit_ticks: u32) -> bool {
    ticks_since_new_station >= limit_ticks && wheels != 0
}
