//! The trainer: AdamW on the three-head loss, cosine lr, early stopping on a
//! validation split by START ID inside the training maps (the held-out MAPS are
//! never looked at during training — they are the test).

use crate::data::{Rows, L_AANG, L_ADY, L_ASPEED, L_BAND, L_TICKS, L_Y, NLAB};
use crate::features::{self, DIM};
use crate::net::{Trainable, Weights, OUT, O_ANG, O_DY, O_LNTICKS, O_REACH, O_SPEED};
use candle_core::{Device, Tensor, D};
use candle_nn::{Optimizer, ParamsAdamW};

#[derive(Clone, Debug)]
pub struct TrainCfg {
    pub hidden: Vec<usize>,
    pub epochs: usize,
    pub batch: usize,
    pub lr: f64,
    pub weight_decay: f64,
    pub w_ticks: f64,
    pub w_band: f64,
    pub ablation: String,
    pub seed: u64,
    pub patience: usize,
}

impl Default for TrainCfg {
    fn default() -> TrainCfg {
        TrainCfg { hidden: vec![256, 256, 256], epochs: 40, batch: 2048, lr: 1e-3, weight_decay: 1e-4, w_ticks: 0.5, w_band: 0.2, ablation: "full".into(), seed: 1, patience: 6 }
    }
}

/// A flat training set: features (possibly ablation-masked) and labels.
pub struct Set {
    pub x: Vec<f32>,
    pub lab: Vec<f32>,
    pub n: usize,
}

impl Set {
    pub fn from_rows(rows: &[&Rows], keep: &[&str]) -> Set {
        let n: usize = rows.iter().map(|r| r.n).sum();
        let mut x = Vec::with_capacity(n * DIM);
        let mut lab = Vec::with_capacity(n * NLAB);
        for r in rows {
            for i in 0..r.n {
                let mut f = r.feat(i).to_vec();
                features::mask_blocks(&mut f, keep);
                x.extend_from_slice(&f);
                lab.extend_from_slice(r.lab(i));
            }
        }
        Set { x, lab, n }
    }
    pub fn feat(&self, i: usize) -> &[f32] {
        &self.x[i * DIM..(i + 1) * DIM]
    }
    pub fn lab(&self, i: usize) -> &[f32] {
        &self.lab[i * NLAB..(i + 1) * NLAB]
    }
    pub fn subset(&self, idx: &[usize]) -> Set {
        let mut x = Vec::with_capacity(idx.len() * DIM);
        let mut lab = Vec::with_capacity(idx.len() * NLAB);
        for &i in idx {
            x.extend_from_slice(self.feat(i));
            lab.extend_from_slice(self.lab(i));
        }
        Set { x, lab, n: idx.len() }
    }
    /// Per-feature mean / std (std 1 where constant).
    pub fn moments(&self) -> (Vec<f32>, Vec<f32>) {
        let mut mean = vec![0f64; DIM];
        let mut sq = vec![0f64; DIM];
        for i in 0..self.n {
            for (k, v) in self.feat(i).iter().enumerate() {
                mean[k] += *v as f64;
                sq[k] += (*v as f64) * (*v as f64);
            }
        }
        let n = self.n.max(1) as f64;
        let mut m = Vec::with_capacity(DIM);
        let mut s = Vec::with_capacity(DIM);
        for k in 0..DIM {
            let mu = mean[k] / n;
            let var = (sq[k] / n - mu * mu).max(0.0);
            m.push(mu as f32);
            s.push(if var.sqrt() < 1e-4 { 1.0 } else { var.sqrt() as f32 });
        }
        (m, s)
    }
}

struct Batch {
    x: Tensor,
    y: Tensor,
    tmask: Tensor,
    lnt: Tensor,
    bmask: Tensor,
    band: Tensor, // (B, 3): speed/100, dy/10, ang
}

fn batch(set: &Set, idx: &[usize], dev: &Device) -> candle_core::Result<Batch> {
    let b = idx.len();
    let mut x = Vec::with_capacity(b * DIM);
    let mut y = Vec::with_capacity(b);
    let mut tmask = Vec::with_capacity(b);
    let mut lnt = Vec::with_capacity(b);
    let mut bmask = Vec::with_capacity(b);
    let mut band = Vec::with_capacity(b * 3);
    for &i in idx {
        x.extend_from_slice(set.feat(i));
        let l = set.lab(i);
        y.push(l[L_Y]);
        let pos = l[L_Y] > 0.5 && l[L_TICKS] >= 0.0;
        tmask.push(if pos { 1.0 } else { 0.0 });
        lnt.push(if pos { (l[L_TICKS].max(1.0)).ln() } else { 0.0 });
        let bok = l[L_BAND] > 0.5;
        bmask.push(if bok { 1.0 } else { 0.0 });
        band.push(if bok { l[L_ASPEED] / 100.0 } else { 0.0 });
        band.push(if bok { l[L_ADY] / 10.0 } else { 0.0 });
        band.push(if bok { l[L_AANG] } else { 0.0 });
    }
    Ok(Batch {
        x: Tensor::from_vec(x, (b, DIM), dev)?,
        y: Tensor::from_vec(y, b, dev)?,
        tmask: Tensor::from_vec(tmask, b, dev)?,
        lnt: Tensor::from_vec(lnt, b, dev)?,
        bmask: Tensor::from_vec(bmask, b, dev)?,
        band: Tensor::from_vec(band, (b, 3), dev)?,
    })
}

/// (total, bce, ticks_mse, band_nll) on one batch.
fn losses(t: &Trainable, bt: &Batch, cfg: &TrainCfg) -> candle_core::Result<(Tensor, f32, f32, f32)> {
    let out = t.forward(&bt.x)?;
    let logit = out.narrow(1, O_REACH, 1)?.squeeze(1)?;
    let bce = candle_nn::loss::binary_cross_entropy_with_logit(&logit, &bt.y)?;
    let eps = 1e-3f64;
    let lnt = out.narrow(1, O_LNTICKS, 1)?.squeeze(1)?;
    let tm = (((lnt - &bt.lnt)?.sqr()? * &bt.tmask)?.sum_all()? / (bt.tmask.sum_all()? + eps)?)?;
    // band: gaussian NLL per component, masked
    let mut nll: Option<Tensor> = None;
    for (k, o) in [O_SPEED, O_DY, O_ANG].iter().enumerate() {
        let mu = out.narrow(1, *o, 1)?.squeeze(1)?;
        let ls = out.narrow(1, *o + 1, 1)?.squeeze(1)?.clamp(-4.0, 3.0)?;
        let yk = bt.band.narrow(1, k, 1)?.squeeze(1)?;
        let z = ((mu - yk)? * ls.neg()?.exp()?)?;
        let term = ((z.sqr()? * 0.5)? + &ls)?;
        nll = Some(match nll {
            None => term,
            Some(a) => (a + term)?,
        });
    }
    let bn = ((nll.unwrap() * &bt.bmask)?.sum_all()? / (bt.bmask.sum_all()? + eps)?)?;
    let total = ((&bce + (&tm * cfg.w_ticks)?)? + (&bn * cfg.w_band)?)?;
    Ok((total, bce.to_scalar::<f32>()?, tm.to_scalar::<f32>()?, bn.to_scalar::<f32>()?))
}

fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

pub fn shuffle(idx: &mut [usize], seed: &mut u64) {
    for i in (1..idx.len()).rev() {
        let j = (xorshift(seed) % (i as u64 + 1)) as usize;
        idx.swap(i, j);
    }
}

pub struct TrainReport {
    pub weights: Weights,
    pub epochs_run: usize,
    pub best_epoch: usize,
    pub best_val: f32,
    pub log: Vec<String>,
}

/// Split the training rows into fit / validation by start id (hash), train,
/// keep the best validation epoch.
pub fn train(train_set: &Set, cfg: &TrainCfg, dev: &Device, verbose: bool) -> Result<TrainReport, String> {
    let (mean, std) = train_set.moments();
    let mut dims = vec![DIM];
    dims.extend(&cfg.hidden);
    dims.push(OUT);
    let t = Trainable::new(&dims, &mean, &std, dev).map_err(|e| e.to_string())?;
    let mut opt = candle_nn::AdamW::new(t.varmap.all_vars(), ParamsAdamW { lr: cfg.lr, weight_decay: cfg.weight_decay, ..Default::default() }).map_err(|e| e.to_string())?;
    // fit / val by start id
    let mut fit = Vec::new();
    let mut val = Vec::new();
    for i in 0..train_set.n {
        let sid = train_set.lab(i)[crate::data::L_START] as u64;
        let h = crate::data::fnv1a64(&format!("s{sid}"));
        if h % 10 == 0 { val.push(i) } else { fit.push(i) }
    }
    let val_set = train_set.subset(&val);
    let mut log = vec![format!("train: {} rows fit, {} rows val (by start id), dims {:?}, {} params, ablation {}, batch {}, lr {}, wd {}, w_ticks {}, w_band {}", fit.len(), val.len(), dims, t.n_params(), cfg.ablation, cfg.batch, cfg.lr, cfg.weight_decay, cfg.w_ticks, cfg.w_band)];
    if verbose {
        eprintln!("{}", log[0]);
    }
    let mut seed = cfg.seed.max(1) ^ 0x9e3779b97f4a7c15;
    let mut best: Option<(f32, Weights, usize)> = None;
    let mut since_best = 0usize;
    let mut epochs_run = 0;
    let t0 = std::time::Instant::now();
    for ep in 0..cfg.epochs {
        epochs_run = ep + 1;
        let lr = cfg.lr * 0.5 * (1.0 + (std::f64::consts::PI * ep as f64 / cfg.epochs as f64).cos());
        opt.set_learning_rate(lr.max(cfg.lr * 0.02));
        shuffle(&mut fit, &mut seed);
        let (mut sl, mut sb, mut st, mut sn, mut nb) = (0f64, 0f64, 0f64, 0f64, 0usize);
        for chunk in fit.chunks(cfg.batch) {
            let bt = batch(train_set, chunk, dev).map_err(|e| e.to_string())?;
            let (loss, b, tk, bn) = losses(&t, &bt, cfg).map_err(|e| e.to_string())?;
            opt.backward_step(&loss).map_err(|e| e.to_string())?;
            sl += loss.to_scalar::<f32>().map_err(|e| e.to_string())? as f64;
            sb += b as f64;
            st += tk as f64;
            sn += bn as f64;
            nb += 1;
        }
        // validation
        let (mut vl, mut vb, mut vt, mut vn, mut vnb) = (0f64, 0f64, 0f64, 0f64, 0usize);
        let vidx: Vec<usize> = (0..val_set.n).collect();
        for chunk in vidx.chunks(8192) {
            let bt = batch(&val_set, chunk, dev).map_err(|e| e.to_string())?;
            let (loss, b, tk, bn) = losses(&t, &bt, cfg).map_err(|e| e.to_string())?;
            let w = chunk.len() as f64;
            vl += loss.to_scalar::<f32>().map_err(|e| e.to_string())? as f64 * w;
            vb += b as f64 * w;
            vt += tk as f64 * w;
            vn += bn as f64 * w;
            vnb += chunk.len();
        }
        let vn_ = vnb.max(1) as f64;
        let vloss = (vl / vn_) as f32;
        let line = format!(
            "epoch {:>3} lr {:.2e}  fit loss {:.4} (bce {:.4} ticks {:.4} band {:.3})  val loss {:.4} (bce {:.4} ticks {:.4} band {:.3})  {:.0} s",
            ep, lr, sl / nb.max(1) as f64, sb / nb.max(1) as f64, st / nb.max(1) as f64, sn / nb.max(1) as f64, vloss, vb / vn_, vt / vn_, vn / vn_, t0.elapsed().as_secs_f64()
        );
        if verbose {
            eprintln!("{line}");
        }
        log.push(line);
        let improved = best.as_ref().map_or(true, |(b, _, _)| vloss < *b);
        if improved {
            let w = t.snapshot().map_err(|e| e.to_string())?;
            best = Some((vloss, w, ep));
            since_best = 0;
        } else {
            since_best += 1;
            if since_best >= cfg.patience {
                log.push(format!("early stop at epoch {ep}: no val improvement for {} epochs", cfg.patience));
                break;
            }
        }
    }
    let (best_val, mut weights, best_epoch) = best.ok_or("no epochs")?;
    // the two-implementation control, on the final weights
    let tb = Trainable::from_weights(&weights, dev).map_err(|e| e.to_string())?;
    let worst = weights.agrees_with(&tb, dev, 64, 1e-4)?;
    log.push(format!("agrees_with: flat vs candle forward, worst |Δ| {worst:.3e} over 64 random inputs (tol 1e-4) → PASS"));
    weights.meta = String::new();
    Ok(TrainReport { weights, epochs_run, best_epoch, best_val, log })
}

/// Predictions (decoded p_reach and raw heads) for every row of a set.
pub fn predict_all(w: &Weights, set: &Set) -> Vec<Vec<f32>> {
    (0..set.n).map(|i| w.forward(set.feat(i))).collect()
}

/// Batched prediction through candle (faster on big sets) — same numbers as the flat forward.
pub fn predict_all_candle(w: &Weights, set: &Set, dev: &Device) -> Result<Vec<Vec<f32>>, String> {
    let t = Trainable::from_weights(w, dev).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(set.n);
    let idx: Vec<usize> = (0..set.n).collect();
    for chunk in idx.chunks(16384) {
        let x = Tensor::from_slice(&set.x[chunk[0] * DIM..(chunk[chunk.len() - 1] + 1) * DIM], (chunk.len(), DIM), dev).map_err(|e| e.to_string())?;
        let o = t.forward(&x).map_err(|e| e.to_string())?;
        let v = o.to_vec2::<f32>().map_err(|e| e.to_string())?;
        out.extend(v);
    }
    let _ = D::Minus1;
    Ok(out)
}
