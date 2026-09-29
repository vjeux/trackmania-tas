//! tmrl as a library (ROLLOUT-WORKER.md §4): the policy artefact, the network forward and sampler, the PPO step
//! layout and the tracking reference — so ENV's `tmroll` worker runs the same forward the master trains. No behaviour
//! lives here that the `tmrl` binary does not also use.
pub mod bc;
pub mod bcnet;
pub mod eval;
pub mod md5;
pub mod net;
pub mod policy;
pub mod ppo;
pub mod ppo_bc;
pub mod ppo_train;
pub mod refs;
pub mod rollout;
pub mod shard;
pub mod synth;

pub use bcnet::{bin_centres, HeadKind, Shape, Weights};
pub use ppo_bc::{sample_chunk, Chunk, Rng};
