//! The policy: an actor-critic MLP, in two implementations that must agree.
//!
//! # Why two
//!
//! Training wants autograd; rollout wants to run a batch of one, in a hundred
//! threads, inside a loop whose other statement costs ten milliseconds. Those
//! are different jobs. `Trainable` is `candle`, which owns the parameters and
//! the gradients. `Weights` is a flat `Vec<f32>` and a hand-written forward,
//! which every rollout worker holds a private copy of — no locks, no shared
//! tensors, no `Send` gymnastics, and no allocation per step.
//!
//! **Two implementations of one function is exactly how this project has got
//! silent corruption before**, so they are checked against each other rather
//! than trusted: [`Weights::agrees_with`] runs both on random inputs and
//! requires them equal. A policy whose rollout forward has drifted from its
//! training forward collects data under one function and updates another, and
//! the symptom is "PPO does not learn", which is indistinguishable from a
//! hundred other things.

use candle_core::{DType, Device, Result as CResult, Tensor};
use candle_nn::{linear, Linear, Module, VarBuilder, VarMap};

/// Hidden width. Linesight's net is 6,578,333 parameters, almost all of it the
/// CNN over a 160x120 greyscale frame. We do not have a frame — we have the
/// engine's own state — so the whole apparatus that dwarfs their parameter
/// count is absent and what is left is small.
pub const HIDDEN: usize = 256;

pub struct Trainable {
    pub varmap: VarMap,
    l1: Linear,
    l2: Linear,
    pi: Linear,
    v: Linear,
    pub obs_dim: usize,
    pub n_actions: usize,
}

impl Trainable {
    pub fn new(obs_dim: usize, n_actions: usize, dev: &Device) -> CResult<Trainable> {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, dev);
        Ok(Trainable {
            l1: linear(obs_dim, HIDDEN, vb.pp("l1"))?,
            l2: linear(HIDDEN, HIDDEN, vb.pp("l2"))?,
            pi: linear(HIDDEN, n_actions, vb.pp("pi"))?,
            v: linear(HIDDEN, 1, vb.pp("v"))?,
            varmap,
            obs_dim,
            n_actions,
        })
    }

    /// `(logits, value)` for a batch of observations.
    pub fn forward(&self, obs: &Tensor) -> CResult<(Tensor, Tensor)> {
        let h = self.l1.forward(obs)?.tanh()?;
        let h = self.l2.forward(&h)?.tanh()?;
        Ok((self.pi.forward(&h)?, self.v.forward(&h)?.squeeze(1)?))
    }

    /// Flatten the parameters into the form the rollout workers run.
    pub fn snapshot(&self) -> CResult<Weights> {
        let g = |n: &str| -> CResult<Vec<f32>> {
            let d = self.varmap.data().lock().unwrap();
            let t = d.get(n).unwrap_or_else(|| panic!("no parameter {n}"));
            t.flatten_all()?.to_vec1::<f32>()
        };
        Ok(Weights {
            obs_dim: self.obs_dim,
            n_actions: self.n_actions,
            w1: g("l1.weight")?,
            b1: g("l1.bias")?,
            w2: g("l2.weight")?,
            b2: g("l2.bias")?,
            wpi: g("pi.weight")?,
            bpi: g("pi.bias")?,
            wv: g("v.weight")?,
            bv: g("v.bias")?,
        })
    }
}

/// The rollout-side policy: flat weights and a hand-written forward.
#[derive(Clone, Debug)]
pub struct Weights {
    pub obs_dim: usize,
    pub n_actions: usize,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: Vec<f32>,
    wpi: Vec<f32>,
    bpi: Vec<f32>,
    wv: Vec<f32>,
    bv: Vec<f32>,
}

/// `y = tanh(W x + b)`, with `W` row-major `[out, in]` as candle stores it.
fn dense_tanh(w: &[f32], b: &[f32], x: &[f32], out: &mut [f32]) {
    let n_in = x.len();
    for (o, y) in out.iter_mut().enumerate() {
        let row = &w[o * n_in..(o + 1) * n_in];
        let mut a = b[o];
        for i in 0..n_in {
            a += row[i] * x[i];
        }
        *y = a.tanh();
    }
}

fn dense(w: &[f32], b: &[f32], x: &[f32], out: &mut [f32]) {
    let n_in = x.len();
    for (o, y) in out.iter_mut().enumerate() {
        let row = &w[o * n_in..(o + 1) * n_in];
        let mut a = b[o];
        for i in 0..n_in {
            a += row[i] * x[i];
        }
        *y = a;
    }
}

impl Weights {
    /// `(logits, value)` for one observation.
    pub fn forward(&self, obs: &[f32]) -> (Vec<f32>, f32) {
        let mut h1 = vec![0f32; HIDDEN];
        let mut h2 = vec![0f32; HIDDEN];
        let mut logits = vec![0f32; self.n_actions];
        let mut v = [0f32; 1];
        dense_tanh(&self.w1, &self.b1, obs, &mut h1);
        dense_tanh(&self.w2, &self.b2, &h1, &mut h2);
        dense(&self.wpi, &self.bpi, &h2, &mut logits);
        dense(&self.wv, &self.bv, &h2, &mut v);
        (logits, v[0])
    }

    /// THE CONTROL on having two implementations of one function.
    ///
    /// Two-sided by construction: it fails if the hand-written forward differs
    /// from candle's on any of `n` random inputs, and it would be satisfied by
    /// nothing else — a forward returning a constant fails, and a forward
    /// returning candle's answer for a different input fails.
    pub fn agrees_with(&self, t: &Trainable, dev: &Device, n: usize, tol: f32) -> Result<f32, String> {
        let mut rng = 0x2026_08_24u64;
        let mut worst = 0f32;
        for _ in 0..n {
            let obs: Vec<f32> = (0..self.obs_dim)
                .map(|_| {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    ((rng >> 11) as f32 / (1u64 << 53) as f32) * 4.0 - 2.0
                })
                .collect();
            let (lm, vm) = self.forward(&obs);
            let x = Tensor::from_vec(obs.clone(), (1, self.obs_dim), dev)
                .map_err(|e| e.to_string())?;
            let (lc, vc) = t.forward(&x).map_err(|e| e.to_string())?;
            let lc = lc.flatten_all().and_then(|t| t.to_vec1::<f32>()).map_err(|e| e.to_string())?;
            let vc = vc.flatten_all().and_then(|t| t.to_vec1::<f32>()).map_err(|e| e.to_string())?;
            for (a, b) in lm.iter().zip(lc.iter()) {
                worst = worst.max((a - b).abs());
            }
            worst = worst.max((vm - vc[0]).abs());
        }
        if worst > tol {
            return Err(format!(
                "the rollout forward and the training forward disagree by {worst:.3e} (tol \
                 {tol:.1e}). Data would be collected under one function and the update applied \
                 to another; the symptom is 'PPO does not learn'."
            ));
        }
        Ok(worst)
    }
}

/// Softmax in place, numerically stable.
pub fn softmax(logits: &mut [f32]) {
    let m = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut s = 0.0;
    for l in logits.iter_mut() {
        *l = (*l - m).exp();
        s += *l;
    }
    for l in logits.iter_mut() {
        *l /= s;
    }
}

/// Sample an index from a probability vector.
pub fn sample(p: &[f32], u: f32) -> usize {
    let mut acc = 0.0;
    for (i, q) in p.iter().enumerate() {
        acc += q;
        if u < acc {
            return i;
        }
    }
    p.len() - 1
}

impl Weights {
    /// Nudge one parameter, for the negative half of [`Weights::agrees_with`].
    ///
    /// A check that two things agree is worthless if nothing could make it
    /// disagree, so the self-test perturbs a copy and requires a REFUSAL.
    pub fn perturb_first_bias(&mut self, d: f32) {
        if let Some(b) = self.b1.first_mut() {
            *b += d;
        }
    }
}
