//! PPO: generalised advantage estimation and the clipped surrogate update.
//!
//! # Why PPO and not Linesight's IQN
//!
//! Linesight runs IQN — distributional Q-learning with 8/32 quantile samples,
//! duelling heads, a soft-updated target network, n-step returns, prioritised
//! replay and two exploration schedules stacked on each other. That is the
//! right family when samples are scarce: they get ~124 env-steps/s across two
//! game instances, so every transition has to be squeezed, and they paid for it
//! with 52 numbered experiments to ship one configuration on one map.
//!
//! We measured **9,600 env-steps/s** across two boxes — about eighty times
//! their rate. That inverts the trade. On-policy PPO throws data away and does
//! not care, has no replay buffer to tune, no target network to destabilise,
//! and one exploration knob instead of three. Sample efficiency is the thing we
//! have the most of.
//!
//! This is a reason, not a preference, and it is falsifiable: if throughput
//! collapses (a much longer map, a much more expensive env) the argument goes
//! with it and IQN becomes the better choice again.

/// One transition, as a rollout worker produces it.
#[derive(Clone)]
pub struct Step {
    pub obs: Vec<f32>,
    pub action: usize,
    pub logp: f32,
    pub value: f32,
    pub reward: f32,
    /// True when the episode ended AT this step for a reason that means there
    /// is no future — a crash or the finish.
    pub terminal: bool,
    /// True when the episode was CUT at this step by a cap rather than ended by
    /// the world. The two are not the same and conflating them is a real bug:
    /// a truncated episode's value must be bootstrapped, and treating it as
    /// terminal teaches the policy that running out of tape is worth zero.
    pub truncated: bool,
    /// The value of the state AFTER this step, needed only when `truncated`.
    pub next_value: f32,
}

pub struct Batch {
    pub obs: Vec<f32>,
    pub actions: Vec<u32>,
    pub logp_old: Vec<f32>,
    pub adv: Vec<f32>,
    pub ret: Vec<f32>,
    pub n: usize,
    pub obs_dim: usize,
}

pub struct GaeCfg {
    pub gamma: f32,
    pub lambda: f32,
}

impl Default for GaeCfg {
    fn default() -> Self {
        // gamma over a 0.100 s action: 0.995 gives a horizon of ~200 actions =
        // 20 s, which is the length of the map. A discount that does not reach
        // the finish cannot value reaching it.
        GaeCfg { gamma: 0.995, lambda: 0.95 }
    }
}

/// Generalised advantage estimation over ONE episode's steps.
pub fn gae(steps: &[Step], cfg: &GaeCfg) -> (Vec<f32>, Vec<f32>) {
    let n = steps.len();
    let mut adv = vec![0f32; n];
    let mut ret = vec![0f32; n];
    let mut last = 0f32;
    for i in (0..n).rev() {
        let s = &steps[i];
        // The bootstrap value of the next state: zero at a true terminal (the
        // world ended, there is no future), the critic's own estimate when the
        // episode was merely cut.
        let next_v = if s.terminal {
            0.0
        } else if s.truncated || i + 1 >= n {
            s.next_value
        } else {
            steps[i + 1].value
        };
        let nonterminal = if s.terminal { 0.0 } else { 1.0 };
        let delta = s.reward + cfg.gamma * next_v - s.value;
        last = delta + cfg.gamma * cfg.lambda * nonterminal * last;
        // A cut episode ends the backward recursion too: what follows in the
        // buffer belongs to a different episode.
        if s.truncated {
            last = delta;
        }
        adv[i] = last;
        ret[i] = last + s.value;
    }
    (adv, ret)
}

/// Assemble episodes into one batch, with advantages normalised across it.
pub fn assemble(episodes: &[Vec<Step>], cfg: &GaeCfg, obs_dim: usize) -> Batch {
    let mut b = Batch {
        obs: Vec::new(),
        actions: Vec::new(),
        logp_old: Vec::new(),
        adv: Vec::new(),
        ret: Vec::new(),
        n: 0,
        obs_dim,
    };
    for ep in episodes {
        if ep.is_empty() {
            continue;
        }
        let (adv, ret) = gae(ep, cfg);
        for (i, s) in ep.iter().enumerate() {
            b.obs.extend_from_slice(&s.obs);
            b.actions.push(s.action as u32);
            b.logp_old.push(s.logp);
            b.adv.push(adv[i]);
            b.ret.push(ret[i]);
            b.n += 1;
        }
    }
    if b.n > 1 {
        let mean: f32 = b.adv.iter().sum::<f32>() / b.n as f32;
        let var: f32 = b.adv.iter().map(|a| (a - mean) * (a - mean)).sum::<f32>() / b.n as f32;
        let sd = var.sqrt().max(1e-6);
        for a in b.adv.iter_mut() {
            *a = (*a - mean) / sd;
        }
    }
    b
}

pub struct PpoCfg {
    pub clip: f32,
    pub epochs: usize,
    pub minibatch: usize,
    pub lr: f64,
    pub vf_coef: f32,
    pub ent_coef: f32,
    pub max_grad_norm: f32,
}

impl Default for PpoCfg {
    fn default() -> Self {
        PpoCfg {
            clip: 0.2,
            epochs: 4,
            minibatch: 1024,
            lr: 3e-4,
            vf_coef: 0.5,
            // Twenty actions and a sparse-ish objective: exploration has to be
            // paid for explicitly early on. Annealed by the trainer.
            ent_coef: 0.01,
            max_grad_norm: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(r: f32, v: f32, terminal: bool) -> Step {
        Step {
            obs: vec![0.0],
            action: 0,
            logp: 0.0,
            value: v,
            reward: r,
            terminal,
            truncated: false,
            next_value: 0.0,
        }
    }

    #[test]
    fn gae_on_a_terminated_episode_is_the_discounted_return_when_the_critic_is_zero() {
        // A known answer: with V = 0 everywhere and lambda = 1, the advantage is
        // exactly the discounted sum of future rewards.
        let cfg = GaeCfg { gamma: 0.9, lambda: 1.0 };
        let eps = vec![step(1.0, 0.0, false), step(1.0, 0.0, false), step(1.0, 0.0, true)];
        let (adv, ret) = gae(&eps, &cfg);
        assert!((adv[2] - 1.0).abs() < 1e-6);
        assert!((adv[1] - (1.0 + 0.9)).abs() < 1e-6);
        assert!((adv[0] - (1.0 + 0.9 + 0.81)).abs() < 1e-6);
        assert_eq!(adv, ret, "with a zero critic the return IS the advantage");
    }

    #[test]
    fn a_truncated_episode_bootstraps_and_a_terminated_one_does_not() {
        // THE two-sided control on the distinction. One reward of 0, one step.
        // Terminated: the future is worth nothing, so the advantage is -V.
        // Truncated: the future is worth next_value, so the advantage is
        // gamma*next_value - V. If the code conflated them these two would be
        // equal, and the policy would learn that running out of tape is death.
        let cfg = GaeCfg { gamma: 0.9, lambda: 1.0 };

        let mut t = step(0.0, 2.0, true);
        t.next_value = 100.0; // must be IGNORED
        let (a_term, _) = gae(&[t], &cfg);

        let mut u = step(0.0, 2.0, false);
        u.truncated = true;
        u.next_value = 100.0; // must be USED
        let (a_trunc, _) = gae(&[u], &cfg);

        assert!((a_term[0] - (-2.0)).abs() < 1e-6, "terminal bootstrapped: {}", a_term[0]);
        assert!((a_trunc[0] - (0.9 * 100.0 - 2.0)).abs() < 1e-4, "truncated did not: {}", a_trunc[0]);
        assert!((a_term[0] - a_trunc[0]).abs() > 1.0, "the two are indistinguishable");
    }

    #[test]
    fn advantages_are_normalised_across_the_batch_and_the_batch_is_not_reordered() {
        let cfg = GaeCfg { gamma: 0.9, lambda: 1.0 };
        let eps = vec![
            vec![step(1.0, 0.0, true)],
            vec![step(3.0, 0.0, true)],
        ];
        let b = assemble(&eps, &cfg, 1);
        assert_eq!(b.n, 2);
        let mean: f32 = b.adv.iter().sum::<f32>() / 2.0;
        assert!(mean.abs() < 1e-5, "not zero-mean: {mean}");
        // Order preserved: the smaller reward stays first.
        assert!(b.adv[0] < b.adv[1]);
        // And the RETURNS are not normalised -- the critic regresses on real
        // returns, and normalising them would make the value head fit noise.
        assert!((b.ret[0] - 1.0).abs() < 1e-6);
        assert!((b.ret[1] - 3.0).abs() < 1e-6);
    }
}
