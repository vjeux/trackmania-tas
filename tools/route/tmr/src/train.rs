//! The trainer: AdamW on the three-head loss, cosine lr, early stopping on a
//! validation split by START ID inside the training maps (the held-out MAPS are
//! never looked at during training — they are the test).

use crate::data::{Rows, L_AANG, L_ADY, L_ASPEED, L_BAND, L_TICKS, L_Y, NLAB};
use crate::feat;
use crate::data::L_DIST;
use crate::net::{Trainable, Weights, MAX_MEAN_SPEED, OUT, O_ANG, O_DY, O_MEANSPEED, O_REACH, O_SPEED};
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
    /// Global gradient-norm clip.
    pub clip: f64,
    /// Input Gaussian noise (normalised units) and hidden dropout, training only.
    pub noise: f64,
    pub dropout: f64,
}

impl Default for TrainCfg {
    fn default() -> TrainCfg {
        TrainCfg { hidden: vec![256, 256, 256], epochs: 40, batch: 2048, lr: 5e-4, weight_decay: 1e-4, w_ticks: 0.5, w_band: 0.1, ablation: "full".into(), seed: 1, patience: 8, clip: 5.0, noise: 0.0, dropout: 0.0 }
    }
}

/// A flat training set: features (possibly ablation-masked) and labels.
pub struct Set {
    pub fv: u32,
    pub dim: usize,
    pub x: Vec<f32>,
    pub lab: Vec<f32>,
    pub n: usize,
}

impl Set {
    pub fn from_rows(rows: &[&Rows], keep: &[&str]) -> Set {
        Self::from_rows_aug(rows, keep, false)
    }
    /// `mirror`: append the left/right mirror of every row (features::mirror) — doubles the set.
    pub fn from_rows_aug(rows: &[&Rows], keep: &[&str], mirror: bool) -> Set {
        let fv = rows.first().map_or(1, |r| r.fv);
        let dim = feat::dim_of(fv);
        assert!(rows.iter().all(|r| r.fv == fv), "rows of mixed feature versions");
        let mirror = mirror && fv == 1;
        let n: usize = rows.iter().map(|r| r.n).sum::<usize>() * if mirror { 2 } else { 1 };
        let mut x = Vec::with_capacity(n * dim);
        let mut lab = Vec::with_capacity(n * NLAB);
        // L_REC (the pairing group) is per map: offset it so pooled sets never pair rows of different maps
        let mut rec_base = 0f32;
        for r in rows {
            let mut rec_max = 0f32;
            for i in 0..r.n {
                let mut f = r.feat(i).to_vec();
                feat::mask_blocks(fv, &mut f, keep);
                let mut l = r.lab(i).to_vec();
                rec_max = rec_max.max(l[crate::data::L_REC]);
                l[crate::data::L_REC] += rec_base;
                x.extend_from_slice(&f);
                lab.extend_from_slice(&l);
                if mirror {
                    feat::mirror(fv, &mut f);
                    x.extend_from_slice(&f);
                    lab.extend_from_slice(&l);
                }
            }
            rec_base += rec_max + 1.0;
        }
        Set { fv, dim, x, lab, n }
    }
    pub fn feat(&self, i: usize) -> &[f32] {
        &self.x[i * self.dim..(i + 1) * self.dim]
    }
    pub fn lab(&self, i: usize) -> &[f32] {
        &self.lab[i * NLAB..(i + 1) * NLAB]
    }
    pub fn subset(&self, idx: &[usize]) -> Set {
        let mut x = Vec::with_capacity(idx.len() * self.dim);
        let mut lab = Vec::with_capacity(idx.len() * NLAB);
        for &i in idx {
            x.extend_from_slice(self.feat(i));
            lab.extend_from_slice(self.lab(i));
        }
        Set { fv: self.fv, dim: self.dim, x, lab, n: idx.len() }
    }
    /// Per-feature mean / std (std 1 where constant).
    pub fn moments(&self) -> (Vec<f32>, Vec<f32>) {
        let dim = self.dim;
        let mut mean = vec![0f64; dim];
        let mut sq = vec![0f64; dim];
        for i in 0..self.n {
            for (k, v) in self.feat(i).iter().enumerate() {
                mean[k] += *v as f64;
                sq[k] += (*v as f64) * (*v as f64);
            }
        }
        let n = self.n.max(1) as f64;
        let mut m = Vec::with_capacity(dim);
        let mut s = Vec::with_capacity(dim);
        for k in 0..dim {
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
    let dim = set.dim;
    let mut x = Vec::with_capacity(b * dim);
    let mut y: Vec<f32> = Vec::with_capacity(b);
    let mut tmask: Vec<f32> = Vec::with_capacity(b);
    let mut lnt: Vec<f32> = Vec::with_capacity(b);
    let mut bmask: Vec<f32> = Vec::with_capacity(b);
    let mut band: Vec<f32> = Vec::with_capacity(b * 3);
    for &i in idx {
        x.extend_from_slice(set.feat(i));
        let l = set.lab(i);
        y.push(l[L_Y]);
        let pos = l[L_Y] > 0.5 && l[L_TICKS] >= 0.0;
        tmask.push(if pos { 1.0 } else { 0.0 });
        // target: ln of the mean speed over the straight distance, m/s, clamped to what a car does
        lnt.push(if pos { (l[L_DIST] / (l[L_TICKS].max(1.0) * 0.01)).clamp(0.5, MAX_MEAN_SPEED).ln() } else { 0.0 });
        let bok = l[L_BAND] > 0.5;
        bmask.push(if bok { 1.0 } else { 0.0 });
        band.push(if bok { l[L_ASPEED] / 100.0 } else { 0.0 });
        band.push(if bok { l[L_ADY] / 10.0 } else { 0.0 });
        band.push(if bok { l[L_AANG] } else { 0.0 });
    }
    Ok(Batch {
        x: Tensor::from_vec(x, (b, dim), dev)?,
        y: Tensor::from_vec(y, b, dev)?,
        tmask: Tensor::from_vec(tmask, b, dev)?,
        lnt: Tensor::from_vec(lnt, b, dev)?,
        bmask: Tensor::from_vec(bmask, b, dev)?,
        band: Tensor::from_vec(band, (b, 3), dev)?,
    })
}

/// (total, bce, ticks_mse, band_nll) on one batch.
fn losses(t: &Trainable, bt: &Batch, cfg: &TrainCfg, training: bool) -> candle_core::Result<(Tensor, f32, f32, f32)> {
    let out = if training { t.forward_reg(&bt.x, cfg.noise, cfg.dropout)? } else { t.forward(&bt.x)? };
    let logit = out.narrow(1, O_REACH, 1)?.squeeze(1)?;
    // numerically stable BCE with logits: max(x,0) − x·y + ln(1 + e^{−|x|})
    // (candle_nn::loss::binary_cross_entropy_with_logit goes through sigmoid→log and
    // returns NaN once a logit saturates; it did, at epoch 11 of the first v0 run)
    let soft = (logit.abs()?.neg()?.exp()? + 1.0)?.log()?;
    let bce = ((logit.relu()? - (&logit * &bt.y)?)? + soft)?.mean_all()?;
    let eps = 1e-3f64;
    // ln(150·σ(u)) = ln 150 − softplus(−u) = ln 150 − ln(1 + e^{−u})
    let u = out.narrow(1, O_MEANSPEED, 1)?.squeeze(1)?;
    let lnt = ((u.neg()?.exp()? + 1.0)?.log()?.neg()? + (MAX_MEAN_SPEED as f64).ln())?;
    let tm = (((lnt - &bt.lnt)?.sqr()? * &bt.tmask)?.sum_all()? / (bt.tmask.sum_all()? + eps)?)?;
    // band: gaussian NLL per component, masked
    let mut nll: Option<Tensor> = None;
    for (k, o) in [O_SPEED, O_DY, O_ANG].iter().enumerate() {
        let mu = out.narrow(1, *o, 1)?.squeeze(1)?;
        let ls = out.narrow(1, *o + 1, 1)?.squeeze(1)?.clamp(-2.0, 2.0)?;
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
    let mut dims = vec![train_set.dim];
    dims.extend(&cfg.hidden);
    dims.push(OUT);
    let t = Trainable::new(&dims, &mean, &std, dev).map_err(|e| e.to_string())?;
    let fv = train_set.fv;
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
    let mut log = vec![format!("train: {} rows fit, {} rows val (by start id), dims {:?}, {} params, ablation {}, batch {}, lr {}, wd {}, w_ticks {}, w_band {}, noise {}, dropout {}", fit.len(), val.len(), dims, t.n_params(), cfg.ablation, cfg.batch, cfg.lr, cfg.weight_decay, cfg.w_ticks, cfg.w_band, cfg.noise, cfg.dropout)];
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
        let (mut clipped, mut gmax) = (0usize, 0f64);
        for chunk in fit.chunks(cfg.batch) {
            let bt = batch(train_set, chunk, dev).map_err(|e| e.to_string())?;
            let (loss, b, tk, bn) = losses(&t, &bt, cfg, true).map_err(|e| e.to_string())?;
            // global-norm gradient clipping (cfg.clip), then the AdamW step
            let mut grads = loss.backward().map_err(|e| e.to_string())?;
            let vars = t.varmap.all_vars();
            let mut sq = 0f64;
            for v in &vars {
                if let Some(g) = grads.get(v.as_tensor()) {
                    sq += g.sqr().and_then(|s| s.sum_all()).and_then(|s| s.to_scalar::<f32>()).map_err(|e| e.to_string())? as f64;
                }
            }
            let gnorm = sq.sqrt();
            if gnorm > cfg.clip {
                let scale = cfg.clip / gnorm;
                for v in &vars {
                    if let Some(g) = grads.get(v.as_tensor()) {
                        let g2 = (g * scale).map_err(|e| e.to_string())?;
                        grads.insert(v.as_tensor(), g2);
                    }
                }
                clipped += 1;
            }
            gmax = gmax.max(gnorm);
            opt.step(&grads).map_err(|e| e.to_string())?;
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
            let (loss, b, tk, bn) = losses(&t, &bt, cfg, false).map_err(|e| e.to_string())?;
            let w = chunk.len() as f64;
            vl += loss.to_scalar::<f32>().map_err(|e| e.to_string())? as f64 * w;
            vb += b as f64 * w;
            vt += tk as f64 * w;
            vn += bn as f64 * w;
            vnb += chunk.len();
        }
        let vn_ = vnb.max(1) as f64;
        // early stopping on the PRIMARY loss (bce + ticks): the band head is auxiliary and its
        // NLL is noisy on 300-odd rows
        let vloss = ((vb + vt * cfg.w_ticks) / vn_) as f32;
        let _ = vl;
        let line = format!(
            "epoch {:>3} lr {:.2e}  fit loss {:.4} (bce {:.4} ticks {:.4} band {:.3})  val loss {:.4} (bce {:.4} ticks {:.4} band {:.3})  grad norm max {:.1}, {} of {} steps clipped  {:.0} s",
            ep, lr, sl / nb.max(1) as f64, sb / nb.max(1) as f64, st / nb.max(1) as f64, sn / nb.max(1) as f64, vloss, vb / vn_, vt / vn_, vn / vn_, gmax, clipped, nb, t0.elapsed().as_secs_f64()
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
    weights.fv = fv;
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
        let d = set.dim;
        let x = Tensor::from_slice(&set.x[chunk[0] * d..(chunk[chunk.len() - 1] + 1) * d], (chunk.len(), d), dev).map_err(|e| e.to_string())?;
        let o = t.forward(&x).map_err(|e| e.to_string())?;
        let v = o.to_vec2::<f32>().map_err(|e| e.to_string())?;
        out.extend(v);
    }
    let _ = D::Minus1;
    Ok(out)
}
