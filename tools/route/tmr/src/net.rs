//! R twice: `Trainable` (candle, autograd) and `Weights` (flat f32, hand-written
//! forward — the planner's and the workers' copy). RL-agentG §4's discipline:
//! `Weights::agrees_with` runs both on random inputs and requires them equal;
//! `tmr selftest` runs the negative half — a perturbed copy must be REFUSED.
//!
//! Heads (one trunk): `[0]` p_reach logit, `[1]` ln(ticks) of the crossing,
//! `[2..4]` arrival speed (mu/100, ln sd), `[4..6]` arrival height above the
//! gate centre (mu/10, ln sd), `[6..8]` angle to the gate normal (mu rad, ln sd).

use candle_core::{DType, Device, Module, Result as CResult, Tensor};
use candle_nn::{linear, Linear, VarBuilder, VarMap};

pub const R_VERSION: u32 = 1;
pub const OUT: usize = 8;
pub const O_REACH: usize = 0;
/// Head 1 parameterises the crossing time as a MEAN SPEED over the straight distance:
/// s = 150 m/s · σ(u); ticks = 100 · dist / s. Bounded by construction (a ln-ticks
/// head extrapolated to 1e18 s on a held-out map), and it transfers: a speed is
/// a car property, a tick count is a map property.
pub const O_MEANSPEED: usize = 1;
pub const MAX_MEAN_SPEED: f32 = 150.0;
pub const O_SPEED: usize = 2;
pub const O_DY: usize = 4;
pub const O_ANG: usize = 6;

pub struct Trainable {
    pub varmap: VarMap,
    layers: Vec<Linear>,
    pub dims: Vec<usize>, // [in, h1, .., out]
    pub mean: Tensor,
    pub inv_std: Tensor,
}

impl Trainable {
    pub fn new(dims: &[usize], mean: &[f32], std: &[f32], dev: &Device) -> CResult<Trainable> {
        assert!(dims.len() >= 2 && dims[0] == mean.len());
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, dev);
        let mut layers = Vec::new();
        for i in 0..dims.len() - 1 {
            layers.push(linear(dims[i], dims[i + 1], vb.pp(format!("l{i}")))?);
        }
        let inv: Vec<f32> = std.iter().map(|s| 1.0 / s.max(1e-6)).collect();
        Ok(Trainable {
            varmap,
            layers,
            dims: dims.to_vec(),
            mean: Tensor::from_slice(mean, (1, mean.len()), dev)?,
            inv_std: Tensor::from_slice(&inv, (1, inv.len()), dev)?,
        })
    }

    /// From flat weights (fine-tuning / resuming).
    pub fn from_weights(w: &Weights, dev: &Device) -> CResult<Trainable> {
        let t = Trainable::new(&w.dims, &w.mean, &w.std, dev)?;
        {
            let mut d = t.varmap.data().lock().unwrap();
            for (i, (wm, b)) in w.layers.iter().enumerate() {
                let (o, n) = (w.dims[i + 1], w.dims[i]);
                d.get_mut(&format!("l{i}.weight")).unwrap().set(&Tensor::from_slice(wm, (o, n), dev)?)?;
                d.get_mut(&format!("l{i}.bias")).unwrap().set(&Tensor::from_slice(b, o, dev)?)?;
            }
        }
        Ok(t)
    }

    /// `(B, OUT)` for a `(B, in)` batch.
    pub fn forward(&self, x: &Tensor) -> CResult<Tensor> {
        let mut h = x.broadcast_sub(&self.mean)?.broadcast_mul(&self.inv_std)?;
        let n = self.layers.len();
        for (i, l) in self.layers.iter().enumerate() {
            h = l.forward(&h)?;
            if i + 1 < n {
                h = h.relu()?;
            }
        }
        Ok(h)
    }

    pub fn snapshot(&self) -> CResult<Weights> {
        let d = self.varmap.data().lock().unwrap();
        let mut layers = Vec::new();
        for i in 0..self.layers.len() {
            let w = d.get(&format!("l{i}.weight")).unwrap().flatten_all()?.to_vec1::<f32>()?;
            let b = d.get(&format!("l{i}.bias")).unwrap().flatten_all()?.to_vec1::<f32>()?;
            layers.push((w, b));
        }
        let mean = self.mean.flatten_all()?.to_vec1::<f32>()?;
        let std = self.inv_std.flatten_all()?.to_vec1::<f32>()?.iter().map(|v| 1.0 / v).collect();
        Ok(Weights { dims: self.dims.clone(), mean, std, layers, meta: String::new() })
    }

    pub fn n_params(&self) -> usize {
        self.dims.windows(2).map(|w| w[0] * w[1] + w[1]).sum()
    }
}

/// The flat copy. `meta` is a JSON provenance string stored in the .tmw trailer.
#[derive(Clone, Debug)]
pub struct Weights {
    pub dims: Vec<usize>,
    pub mean: Vec<f32>,
    pub std: Vec<f32>,
    pub layers: Vec<(Vec<f32>, Vec<f32>)>,
    pub meta: String,
}

fn dense(w: &[f32], b: &[f32], x: &[f32], out: &mut [f32], relu: bool) {
    let n_in = x.len();
    for (o, y) in out.iter_mut().enumerate() {
        let row = &w[o * n_in..(o + 1) * n_in];
        let mut a = b[o];
        for i in 0..n_in {
            a += row[i] * x[i];
        }
        *y = if relu && a < 0.0 { 0.0 } else { a };
    }
}

/// What R says about one (state, target, h): the heads decoded.
#[derive(Clone, Copy, Debug)]
pub struct Estimate {
    pub p_reach: f32,
    pub expected_ticks: f32,
    pub speed_mu: f32,
    pub speed_sd: f32,
    pub dy_mu: f32,
    pub dy_sd: f32,
    pub ang_mu: f32,
    pub ang_sd: f32,
}

/// `dist_m` = straight distance start → gate centre (the feature block carries it too).
pub fn decode(o: &[f32], dist_m: f32) -> Estimate {
    let s = MAX_MEAN_SPEED / (1.0 + (-o[O_MEANSPEED]).exp());
    Estimate {
        p_reach: 1.0 / (1.0 + (-o[O_REACH]).exp()),
        expected_ticks: (100.0 * dist_m.max(0.0) / s.max(0.5)).min(6000.0),
        speed_mu: o[O_SPEED] * 100.0,
        speed_sd: o[O_SPEED + 1].exp() * 100.0,
        dy_mu: o[O_DY] * 10.0,
        dy_sd: o[O_DY + 1].exp() * 10.0,
        ang_mu: o[O_ANG],
        ang_sd: o[O_ANG + 1].exp(),
    }
}

impl Weights {
    pub fn in_dim(&self) -> usize {
        self.dims[0]
    }
    /// Raw head outputs for one feature vector.
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        let n = self.layers.len();
        let mut cur: Vec<f32> = x.iter().zip(&self.mean).zip(&self.std).map(|((v, m), s)| (v - m) / s.max(1e-6)).collect();
        for (i, (w, b)) in self.layers.iter().enumerate() {
            let mut next = vec![0f32; self.dims[i + 1]];
            dense(w, b, &cur, &mut next, i + 1 < n);
            cur = next;
        }
        cur
    }
    pub fn estimate(&self, x: &[f32], dist_m: f32) -> Estimate {
        decode(&self.forward(x), dist_m)
    }

    /// THE CONTROL on two implementations of one function (RL-agentG §4).
    pub fn agrees_with(&self, t: &Trainable, dev: &Device, n: usize, tol: f32) -> Result<f32, String> {
        let mut rng = 0x2026_09_07u64;
        let mut worst = 0f32;
        let d = self.in_dim();
        for _ in 0..n {
            // inputs in the DATA domain (mean ± 2 std), so the activations are O(1) and an
            // absolute tolerance means something
            let x: Vec<f32> = (0..d)
                .map(|k| {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    let u = ((rng >> 11) as f32 / (1u64 << 53) as f32) * 4.0 - 2.0;
                    self.mean[k] + self.std[k] * u
                })
                .collect();
            let a = self.forward(&x);
            let xt = Tensor::from_vec(x.clone(), (1, d), dev).map_err(|e| e.to_string())?;
            let b = t.forward(&xt).and_then(|t| t.flatten_all()).and_then(|t| t.to_vec1::<f32>()).map_err(|e| e.to_string())?;
            for (p, q) in a.iter().zip(b.iter()) {
                worst = worst.max((p - q).abs());
            }
        }
        if worst > tol {
            return Err(format!("the flat forward and the candle forward disagree by {worst:.3e} (tol {tol:.1e}): the planner would run one function and the trainer fit another"));
        }
        Ok(worst)
    }

    /// The negative half's perturbation.
    pub fn perturb_first_bias(&mut self, d: f32) {
        if let Some(b) = self.layers[0].1.first_mut() {
            *b += d;
        }
    }

    pub fn n_params(&self) -> usize {
        self.layers.iter().map(|(w, b)| w.len() + b.len()).sum()
    }

    /// `.tmw`: `TMW0`, R_VERSION, FEATURE_VERSION, n_dims, dims[], activation
    /// (1 = relu), mean[in], std[in], per layer W[out×in] then b[out], then
    /// meta_len u32 + JSON meta.
    pub fn save(&self, p: &std::path::Path) -> Result<(), String> {
        let mut b = Vec::new();
        b.extend_from_slice(b"TMW0");
        b.extend_from_slice(&R_VERSION.to_le_bytes());
        b.extend_from_slice(&crate::features::FEATURE_VERSION.to_le_bytes());
        b.extend_from_slice(&(self.dims.len() as u32).to_le_bytes());
        for d in &self.dims {
            b.extend_from_slice(&(*d as u32).to_le_bytes());
        }
        b.extend_from_slice(&1u32.to_le_bytes());
        for v in self.mean.iter().chain(&self.std) {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for (w, bias) in &self.layers {
            for v in w.iter().chain(bias) {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        let m = self.meta.as_bytes();
        b.extend_from_slice(&(m.len() as u32).to_le_bytes());
        b.extend_from_slice(m);
        std::fs::write(p, b).map_err(|e| format!("{}: {e}", p.display()))
    }

    pub fn load(p: &std::path::Path) -> Result<Weights, String> {
        let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let bad = |m: &str| Err(format!("{}: {m}", p.display()));
        if b.len() < 16 || &b[0..4] != b"TMW0" {
            return bad("not a TMW0 file");
        }
        let mut o = 4usize;
        let u32n = |o: &mut usize| -> u32 {
            let v = u32::from_le_bytes(b[*o..*o + 4].try_into().unwrap());
            *o += 4;
            v
        };
        let ver = u32n(&mut o);
        if ver != R_VERSION {
            return bad(&format!("R_VERSION {ver}, this build reads {R_VERSION}"));
        }
        let fv = u32n(&mut o);
        if fv != crate::features::FEATURE_VERSION {
            return bad(&format!("FEATURE_VERSION {fv}, this build computes {}", crate::features::FEATURE_VERSION));
        }
        let nd = u32n(&mut o) as usize;
        let mut dims = Vec::with_capacity(nd);
        for _ in 0..nd {
            dims.push(u32n(&mut o) as usize);
        }
        let act = u32n(&mut o);
        if act != 1 {
            return bad(&format!("activation {act} unknown"));
        }
        let f32s = |o: &mut usize, n: usize| -> Result<Vec<f32>, String> {
            if *o + 4 * n > b.len() {
                return Err(format!("{}: truncated", p.display()));
            }
            let v = b[*o..*o + 4 * n].chunks(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
            *o += 4 * n;
            Ok(v)
        };
        let mean = f32s(&mut o, dims[0])?;
        let std = f32s(&mut o, dims[0])?;
        let mut layers = Vec::new();
        for i in 0..nd - 1 {
            let w = f32s(&mut o, dims[i] * dims[i + 1])?;
            let bias = f32s(&mut o, dims[i + 1])?;
            layers.push((w, bias));
        }
        if o + 4 > b.len() {
            return bad("no meta trailer");
        }
        let ml = u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
        o += 4;
        if o + ml != b.len() {
            return bad(&format!("trailer says {ml} bytes of meta, {} remain", b.len() - o));
        }
        let meta = String::from_utf8_lossy(&b[o..]).to_string();
        Ok(Weights { dims, mean, std, layers, meta })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_and_agreement() {
        let dev = Device::Cpu;
        let d = 7;
        let mean = vec![0.1f32; d];
        let std = vec![2.0f32; d];
        let t = Trainable::new(&[d, 16, OUT], &mean, &std, &dev).unwrap();
        let w = t.snapshot().unwrap();
        let worst = w.agrees_with(&t, &dev, 16, 1e-5).unwrap();
        assert!(worst < 1e-5);
        let p = std::env::temp_dir().join(format!("tmr-test-{}.tmw", std::process::id()));
        let mut w2 = w.clone();
        w2.meta = "{\"test\":1}".into();
        w2.save(&p).unwrap();
        let r = Weights::load(&p).unwrap();
        assert_eq!(r.dims, w.dims);
        assert_eq!(r.layers[0].0, w.layers[0].0);
        assert_eq!(r.meta, "{\"test\":1}");
        let _ = std::fs::remove_file(&p);
        let mut bad = w.clone();
        bad.perturb_first_bias(1e-2);
        assert!(bad.agrees_with(&t, &dev, 16, 1e-5).is_err());
    }
}
