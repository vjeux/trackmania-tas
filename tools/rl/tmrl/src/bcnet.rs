//! The policy network, in two implementations that must agree (a7aa56c `net.rs`, generalised to chunked heads).
//!
//! `Trainable` is candle (autograd, AdamW). `Weights` is flat `Vec<f32>` with a hand-written forward that every
//! rollout worker holds privately — no locks, no shared tensors, no allocation per step. Two implementations of
//! one function is how this project got silent corruption before, so [`Weights::agrees_with`] runs both on random
//! inputs and requires them equal, and `tmrl selftest` runs the negative half: a perturbed copy must be REFUSED.
//!
//! # Heads
//! * `AnalogPedals` (BC default): per chunk tick `i` in `0..k` the raw outputs are `steer[i]` (tanh → ±1 → ±127,
//!   quantised to the engine's 1/127 grid), `gas[i]`, `brake[i]` (Bernoulli logits), then one value output.
//!   `n_out = 3k + 1`.
//! * `Categorical` (bins B, odd): steer is a B-way softmax per tick over centres on the i8 grid (B = 3 is the
//!   keyboard-legal head {−127, 0, 127}); pedals as above. `n_out = (B + 2)k + 1`.
//!
//! The value output is unused by BC and carried so the same file warm-starts PPO (L2).

use candle_core::{DType, Device, Result as CResult, Tensor};
use candle_nn::{linear, Linear, Module, VarBuilder, VarMap};
use tmstate::Action;

pub const HIDDEN: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadKind {
    AnalogPedals = 0,
    /// Steer as a softmax over `Shape::bins` centres on the i8 grid (odd `bins`, so 0 and ±127 are exact centres).
    /// `bins = 3` is the keyboard-legal head {−127, 0, 127}. This is the head for a target that is 93 % exactly
    /// {0, ±127} with a tail of partial values: a regression head hedges between modes, a categorical one does not.
    Categorical = 1,
}

impl HeadKind {
    pub fn from_u32(x: u32) -> Option<HeadKind> {
        match x {
            0 => Some(HeadKind::AnalogPedals),
            1 => Some(HeadKind::Categorical),
            _ => None,
        }
    }
    pub fn parse(s: &str) -> Option<HeadKind> {
        match s {
            "analog" => Some(HeadKind::AnalogPedals),
            "keyboard" | "cat" | "categorical" => Some(HeadKind::Categorical),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            HeadKind::AnalogPedals => "analog",
            HeadKind::Categorical => "categorical",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub obs_dim: usize,
    pub hidden: usize,
    /// Number of hidden layers (tanh), each `hidden` wide. 2 is the original net; the policy.tmw tensor count
    /// (2 × layers + 2) carries it, so 2-layer files load unchanged.
    pub layers: usize,
    pub k: usize,
    pub head: HeadKind,
    /// Steer bins for the categorical head (odd, >= 3); 0 for the analog head.
    pub bins: usize,
}

/// Bin centres on the i8 grid: linspace(−127, 127, bins), rounded. `bins` odd so the middle one is exactly 0.
pub fn bin_centres(bins: usize) -> Vec<i8> {
    assert!(bins >= 3 && bins % 2 == 1, "bins must be odd and >= 3, got {bins}");
    (0..bins).map(|i| (-127.0 + 254.0 * i as f32 / (bins - 1) as f32).round() as i8).collect()
}

/// Nearest bin of a steer value.
pub fn bin_of(steer: i8, centres: &[i8]) -> usize {
    let mut best = (i32::MAX, 0usize);
    for (i, c) in centres.iter().enumerate() {
        let d = (steer as i32 - *c as i32).abs();
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

impl Shape {
    pub fn n_out(&self) -> usize {
        match self.head {
            HeadKind::AnalogPedals => 3 * self.k + 1,
            HeadKind::Categorical => (self.bins + 2) * self.k + 1,
        }
    }
    /// Raw-output slice offsets: (steer, gas, brake, value).
    pub fn offsets(&self) -> (usize, usize, usize, usize) {
        match self.head {
            HeadKind::AnalogPedals => (0, self.k, 2 * self.k, 3 * self.k),
            HeadKind::Categorical => (0, self.bins * self.k, (self.bins + 1) * self.k, (self.bins + 2) * self.k),
        }
    }
}

pub const TENSOR_NAMES: [&str; 6] = ["l1.weight", "l1.bias", "l2.weight", "l2.bias", "head.weight", "head.bias"];

/// Tensor names for a shape: l1.weight, l1.bias, …, lN.weight, lN.bias, head.weight, head.bias.
pub fn tensor_names(layers: usize) -> Vec<String> {
    let mut v = Vec::new();
    for i in 1..=layers {
        v.push(format!("l{i}.weight"));
        v.push(format!("l{i}.bias"));
    }
    v.push("head.weight".into());
    v.push("head.bias".into());
    v
}

pub struct Trainable {
    pub varmap: VarMap,
    hidden: Vec<Linear>,
    head: Linear,
    pub shape: Shape,
}

impl Trainable {
    pub fn new(shape: Shape, dev: &Device) -> CResult<Trainable> {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, dev);
        let mut hidden = Vec::new();
        for i in 1..=shape.layers.max(1) {
            let n_in = if i == 1 { shape.obs_dim } else { shape.hidden };
            hidden.push(linear(n_in, shape.hidden, vb.pp(format!("l{i}")))?);
        }
        Ok(Trainable {
            hidden,
            head: linear(shape.hidden, shape.n_out(), vb.pp("head"))?,
            varmap,
            shape,
        })
    }

    /// Load `Weights` into a fresh trainable (fine-tuning / PPO warm start).
    pub fn from_weights(w: &Weights, dev: &Device) -> CResult<Trainable> {
        let t = Trainable::new(w.shape, dev)?;
        {
            let data = t.varmap.data().lock().unwrap();
            for (name, (vals, dims)) in tensor_names(w.shape.layers).iter().zip(w.tensors()) {
                let var = data.get(name.as_str()).unwrap_or_else(|| panic!("no parameter {name}"));
                let src = Tensor::from_vec(vals.to_vec(), dims.as_slice(), dev)?;
                var.set(&src)?;
            }
        }
        Ok(t)
    }

    /// Raw outputs `[batch, n_out]` (pre-activation).
    pub fn forward(&self, obs: &Tensor) -> CResult<Tensor> {
        let mut h = obs.clone();
        for l in &self.hidden {
            h = l.forward(&h)?.tanh()?;
        }
        self.head.forward(&h)
    }

    pub fn snapshot(&self) -> CResult<Weights> {
        let g = |n: &str| -> CResult<Vec<f32>> {
            let d = self.varmap.data().lock().unwrap();
            let t = d.get(n).unwrap_or_else(|| panic!("no parameter {n}"));
            t.flatten_all()?.to_vec1::<f32>()
        };
        let mut hidden = Vec::new();
        for i in 1..=self.shape.layers.max(1) {
            hidden.push((g(&format!("l{i}.weight"))?, g(&format!("l{i}.bias"))?));
        }
        Ok(Weights { shape: self.shape, hidden, wh: g("head.weight")?, bh: g("head.bias")? })
    }

    pub fn n_params(&self) -> usize {
        let s = &self.shape;
        s.obs_dim * s.hidden + s.hidden + (s.layers.max(1) - 1) * (s.hidden * s.hidden + s.hidden) + s.hidden * s.n_out() + s.n_out()
    }
}

/// The rollout-side policy: flat weights and a hand-written forward.
#[derive(Clone, Debug)]
pub struct Weights {
    pub shape: Shape,
    /// Hidden layers in order: (weight [hidden, in], bias [hidden]).
    pub hidden: Vec<(Vec<f32>, Vec<f32>)>,
    pub wh: Vec<f32>,
    pub bh: Vec<f32>,
}

fn dense(w: &[f32], b: &[f32], x: &[f32], out: &mut [f32], tanh: bool) {
    let n_in = x.len();
    for (o, y) in out.iter_mut().enumerate() {
        let row = &w[o * n_in..(o + 1) * n_in];
        let mut a = b[o];
        for i in 0..n_in {
            a += row[i] * x[i];
        }
        *y = if tanh { a.tanh() } else { a };
    }
}

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Steer in ±1 → the engine's i8 grid.
pub fn quantise_steer(s: f32) -> i8 {
    (s.clamp(-1.0, 1.0) * 127.0).round() as i8
}

impl Weights {
    /// Tensors in `TENSOR_NAMES` order with their candle shapes (`[out, in]` for weights).
    pub fn tensors(&self) -> Vec<(&[f32], Vec<usize>)> {
        let s = &self.shape;
        let mut v = Vec::new();
        for (i, (w, b)) in self.hidden.iter().enumerate() {
            let n_in = if i == 0 { s.obs_dim } else { s.hidden };
            v.push((&w[..], vec![s.hidden, n_in]));
            v.push((&b[..], vec![s.hidden]));
        }
        v.push((&self.wh[..], vec![s.n_out(), s.hidden]));
        v.push((&self.bh[..], vec![s.n_out()]));
        v
    }

    pub fn from_tensors(shape: Shape, t: Vec<Vec<f32>>) -> Result<Weights, String> {
        let want_n = 2 * shape.layers + 2;
        if t.len() != want_n {
            return Err(format!("{} tensors, expected {want_n} for {} hidden layers", t.len(), shape.layers));
        }
        let mut it = t.into_iter();
        let mut hidden = Vec::new();
        for _ in 0..shape.layers {
            let w = it.next().unwrap();
            let b = it.next().unwrap();
            hidden.push((w, b));
        }
        let w = Weights { shape, hidden, wh: it.next().unwrap(), bh: it.next().unwrap() };
        for (name, (vals, dims)) in tensor_names(shape.layers).iter().zip(w.tensors()) {
            let want: usize = dims.iter().product();
            if vals.len() != want {
                return Err(format!("{name}: {} values, shape {:?} wants {want}", vals.len(), dims));
            }
        }
        Ok(w)
    }

    /// Raw outputs for one observation.
    pub fn forward(&self, obs: &[f32]) -> Vec<f32> {
        assert_eq!(obs.len(), self.shape.obs_dim);
        let mut x: Vec<f32> = obs.to_vec();
        for (w, b) in &self.hidden {
            let mut h = vec![0f32; self.shape.hidden];
            dense(w, b, &x, &mut h, true);
            x = h;
        }
        let mut out = vec![0f32; self.shape.n_out()];
        dense(&self.wh, &self.bh, &x, &mut out, false);
        out
    }

    /// The deterministic (mode) action chunk from raw outputs, plus the value estimate.
    pub fn decode(&self, raw: &[f32]) -> (Vec<Action>, f32) {
        let s = &self.shape;
        let (so, go, bo, vo) = s.offsets();
        let mut acts = Vec::with_capacity(s.k);
        for i in 0..s.k {
            let steer = match s.head {
                HeadKind::AnalogPedals => quantise_steer(raw[so + i].tanh()),
                HeadKind::Categorical => {
                    let b = s.bins;
                    let l = &raw[so + b * i..so + b * i + b];
                    let mut best = 0;
                    for j in 1..b {
                        if l[j] > l[best] {
                            best = j;
                        }
                    }
                    bin_centres(b)[best]
                }
            };
            acts.push(Action { steer, gas: raw[go + i] > 0.0, brake: raw[bo + i] > 0.0 });
        }
        (acts, raw[vo])
    }

    /// A SAMPLED action chunk: categorical steer from softmax(logits / temp), pedals Bernoulli(sigmoid(logit / temp));
    /// the analog head adds Gaussian noise of sd `0.1 * temp` to the tanh steer. `u()` returns uniforms in [0,1).
    pub fn sample(&self, raw: &[f32], temp: f32, u: &mut dyn FnMut() -> f32) -> Vec<Action> {
        let s = &self.shape;
        let (so, go, bo, _) = s.offsets();
        let t = temp.max(1e-3);
        let mut acts = Vec::with_capacity(s.k);
        for i in 0..s.k {
            let steer = match s.head {
                HeadKind::AnalogPedals => {
                    // Box-Muller
                    let (u1, u2) = (u().max(1e-7), u());
                    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
                    quantise_steer(raw[so + i].tanh() + 0.1 * t * z)
                }
                HeadKind::Categorical => {
                    let b = s.bins;
                    let l = &raw[so + b * i..so + b * i + b];
                    let m = l.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let p: Vec<f32> = l.iter().map(|x| ((x - m) / t).exp()).collect();
                    let z: f32 = p.iter().sum();
                    let r = u() * z;
                    let mut acc = 0.0;
                    let mut j = b - 1;
                    for (idx, q) in p.iter().enumerate() {
                        acc += q;
                        if r < acc {
                            j = idx;
                            break;
                        }
                    }
                    bin_centres(b)[j]
                }
            };
            let gas = u() < sigmoid(raw[go + i] / t);
            let brake = u() < sigmoid(raw[bo + i] / t);
            acts.push(Action { steer, gas, brake });
        }
        acts
    }

    pub fn act(&self, obs: &[f32]) -> Vec<Action> {
        self.decode(&self.forward(obs)).0
    }

    /// THE CONTROL on having two implementations of one function. Two-sided by construction: fails if the
    /// hand-written forward differs from candle's on any of `n` random inputs.
    pub fn agrees_with(&self, t: &Trainable, dev: &Device, n: usize, tol: f32) -> Result<f32, String> {
        if t.shape != self.shape {
            return Err(format!("shape mismatch: {:?} vs {:?}", t.shape, self.shape));
        }
        let mut rng = 0x2026_09_06u64;
        let mut worst = 0f32;
        for _ in 0..n {
            let obs: Vec<f32> = (0..self.shape.obs_dim)
                .map(|_| {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    ((rng >> 11) as f32 / (1u64 << 53) as f32) * 4.0 - 2.0
                })
                .collect();
            let mine = self.forward(&obs);
            let x = Tensor::from_vec(obs.clone(), (1, self.shape.obs_dim), dev).map_err(|e| e.to_string())?;
            let theirs = t.forward(&x).and_then(|t| t.flatten_all()).and_then(|t| t.to_vec1::<f32>()).map_err(|e| e.to_string())?;
            if theirs.len() != mine.len() {
                return Err(format!("output width {} vs {}", theirs.len(), mine.len()));
            }
            for (a, b) in mine.iter().zip(theirs.iter()) {
                worst = worst.max((a - b).abs());
            }
        }
        if worst > tol {
            return Err(format!(
                "the rollout forward and the training forward disagree by {worst:.3e} (tol {tol:.1e}). Data would be \
                 collected under one function and the update applied to another."
            ));
        }
        Ok(worst)
    }

    /// Nudge one parameter, for the negative half of [`Weights::agrees_with`].
    pub fn perturb_first_bias(&mut self, d: f32) {
        if let Some(b) = self.hidden[0].1.first_mut() {
            *b += d;
        }
    }
}
