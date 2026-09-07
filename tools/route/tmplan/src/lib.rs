//! `tmplan` — the route planner skeleton (BRIEF-GEOM R5).
//!
//! * `surface`   — the cartographer's surface graph over the FULL checkpoint set of a `gates.json`
//! * `estimator` — the `EdgeEstimator` seam; `Geometric` is the first implementation, R is the second
//! * `planner`   — beam search over (visited, gate group, arrival bucket), finish last
//! * `export`    — a plan → `router-plan` route file with `Predicted` legs

pub mod estimator;
pub mod export;
pub mod planner;
pub mod surface;
