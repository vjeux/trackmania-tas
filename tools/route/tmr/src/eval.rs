//! The gate (ROUTE-PLAN §2.2 / BRIEF-MODEL): the two-gate test with its
//! distance-only control, calibration (10 bins + ECE), AUC, and the
//! expected-time error on human legs. Every number is computed by one function
//! over one set so the train-map and held-out-map figures are comparable.

use crate::data::{L_DIST, L_HUMAN, L_REC, L_TICKS, L_Y};
use crate::net::{decode, O_REACH};
use crate::train::Set;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct TwoGate {
    pub pairs: usize,
    pub records: usize,
    pub model_correct: usize,
    pub model_ties: usize,
    pub baseline_correct: usize,
    pub baseline_ties: usize,
    /// hardest pairing: the positive vs the NEAREST negative
    pub hard_pairs: usize,
    pub hard_model_correct: usize,
    pub hard_baseline_correct: usize,
    /// DISTANCE-WRONG pairs: every (positive, negative) pair of a record in which the negative is
    /// NEARER than (or as near as) the reached gate — the population the router exists for.
    pub dw_pairs: usize,
    pub dw_model_correct: usize,
    /// all (positive, negative) pairs, for the share the distance-wrong ones are
    pub all_pairs: usize,
    /// MATCHED-DISTANCE pairs: the negative of the same group whose distance is within ±10 % of the
    /// positive's (the nearest such) — distance carries no information here by construction.
    pub md_pairs: usize,
    pub md_model_correct: usize,
    pub md_baseline_correct: usize,
}

impl TwoGate {
    pub fn model_pct(&self) -> f64 {
        100.0 * self.model_correct as f64 / self.pairs.max(1) as f64
    }
    pub fn baseline_pct(&self) -> f64 {
        100.0 * self.baseline_correct as f64 / self.pairs.max(1) as f64
    }
    pub fn hard_model_pct(&self) -> f64 {
        100.0 * self.hard_model_correct as f64 / self.hard_pairs.max(1) as f64
    }
    pub fn hard_baseline_pct(&self) -> f64 {
        100.0 * self.hard_baseline_correct as f64 / self.hard_pairs.max(1) as f64
    }
    pub fn md_model_pct(&self) -> f64 {
        100.0 * self.md_model_correct as f64 / self.md_pairs.max(1) as f64
    }
    pub fn md_baseline_pct(&self) -> f64 {
        100.0 * self.md_baseline_correct as f64 / self.md_pairs.max(1) as f64
    }
    pub fn dw_model_pct(&self) -> f64 {
        100.0 * self.dw_model_correct as f64 / self.dw_pairs.max(1) as f64
    }
    pub fn margin(&self) -> f64 {
        self.model_pct() - self.baseline_pct()
    }
    /// A held-out map where the ruler is ≥ 99 % right cannot separate R from the ruler.
    pub fn informative(&self) -> bool {
        self.pairs > 0 && self.baseline_pct() < 99.0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Calib {
    pub bins: Vec<(usize, f64, f64)>, // (n, mean p, frac positive)
    pub ece: f64,
    pub auc: f64,
    pub brier: f64,
    pub n: usize,
    pub pos_rate: f64,
}

#[derive(Clone, Debug, Default)]
pub struct TimeErr {
    pub n: usize,
    pub mae_s: f64,
    pub bias_s: f64,
    pub median_abs_s: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub two_gate: TwoGate,
    pub calib: Calib,
    pub time_human: TimeErr,
    pub time_all_pos: TimeErr,
}

fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// `pred[i]` = raw head outputs of row i.
pub fn evaluate(set: &Set, pred: &[Vec<f32>], seed: u64) -> Report {
    let mut rng = seed.max(1) ^ 0xdead_beef_cafe_f00d;
    // group rows by record
    let mut by_rec: HashMap<u32, Vec<usize>> = HashMap::new();
    for i in 0..set.n {
        by_rec.entry(set.lab(i)[L_REC] as u32).or_default().push(i);
    }
    let mut tg = TwoGate::default();
    let mut recs: Vec<&u32> = by_rec.keys().collect();
    recs.sort();
    for r in recs {
        let rows = &by_rec[r];
        let pos: Vec<usize> = rows.iter().cloned().filter(|&i| set.lab(i)[L_Y] > 0.5).collect();
        let neg: Vec<usize> = rows.iter().cloned().filter(|&i| set.lab(i)[L_Y] <= 0.5).collect();
        if pos.is_empty() || neg.is_empty() {
            continue;
        }
        tg.records += 1;
        for &p in &pos {
            // random negative
            let n = neg[(xorshift(&mut rng) % neg.len() as u64) as usize];
            tg.pairs += 1;
            let (sp, sn) = (pred[p][O_REACH], pred[n][O_REACH]);
            if sp > sn { tg.model_correct += 1 } else if sp == sn { tg.model_ties += 1 }
            let (dp, dn) = (set.lab(p)[L_DIST], set.lab(n)[L_DIST]);
            if dp < dn { tg.baseline_correct += 1 } else if dp == dn { tg.baseline_ties += 1 }
            // nearest negative
            let n2 = *neg.iter().min_by(|a, b| set.lab(**a)[L_DIST].partial_cmp(&set.lab(**b)[L_DIST]).unwrap()).unwrap();
            tg.hard_pairs += 1;
            if sp > pred[n2][O_REACH] { tg.hard_model_correct += 1 }
            if dp < set.lab(n2)[L_DIST] { tg.hard_baseline_correct += 1 }
            // matched-distance pair
            let mut best: Option<(f32, usize)> = None;
            for &m in &neg {
                let dm = set.lab(m)[L_DIST];
                let rel = (dm - dp).abs() / dp.max(1.0);
                if rel <= 0.10 && best.map_or(true, |(b, _)| rel < b) {
                    best = Some((rel, m));
                }
            }
            if let Some((_, m)) = best {
                tg.md_pairs += 1;
                if sp > pred[m][O_REACH] { tg.md_model_correct += 1 }
                if dp < set.lab(m)[L_DIST] { tg.md_baseline_correct += 1 }
            }
            // distance-wrong pairs: every negative at least as near as the positive
            for &m in &neg {
                tg.all_pairs += 1;
                if set.lab(m)[L_DIST] <= dp {
                    tg.dw_pairs += 1;
                    if sp > pred[m][O_REACH] { tg.dw_model_correct += 1 }
                }
            }
        }
    }
    // calibration + AUC + Brier
    let mut cal = Calib { bins: vec![(0, 0.0, 0.0); 10], ..Default::default() };
    let mut scored: Vec<(f32, bool)> = Vec::with_capacity(set.n);
    let mut brier = 0f64;
    let mut npos = 0usize;
    for i in 0..set.n {
        let p = decode(&pred[i], set.lab(i)[L_DIST]).p_reach;
        let y = set.lab(i)[L_Y] > 0.5;
        let b = ((p * 10.0) as usize).min(9);
        cal.bins[b].0 += 1;
        cal.bins[b].1 += p as f64;
        cal.bins[b].2 += if y { 1.0 } else { 0.0 };
        brier += (p as f64 - if y { 1.0 } else { 0.0 }).powi(2);
        if y { npos += 1 }
        scored.push((p, y));
    }
    cal.n = set.n;
    cal.pos_rate = npos as f64 / set.n.max(1) as f64;
    cal.brier = brier / set.n.max(1) as f64;
    let mut ece = 0.0;
    for b in cal.bins.iter_mut() {
        if b.0 > 0 {
            b.1 /= b.0 as f64;
            b.2 /= b.0 as f64;
            ece += (b.1 - b.2).abs() * b.0 as f64 / set.n as f64;
        }
    }
    cal.ece = ece;
    // AUC by rank (Mann–Whitney), ties averaged
    scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let nneg = set.n - npos;
    if npos > 0 && nneg > 0 {
        let mut rank_sum = 0f64;
        let mut i = 0;
        while i < scored.len() {
            let mut j = i;
            while j + 1 < scored.len() && scored[j + 1].0 == scored[i].0 {
                j += 1;
            }
            let avg_rank = (i + j) as f64 / 2.0 + 1.0;
            for k in i..=j {
                if scored[k].1 {
                    rank_sum += avg_rank;
                }
            }
            i = j + 1;
        }
        cal.auc = (rank_sum - npos as f64 * (npos as f64 + 1.0) / 2.0) / (npos as f64 * nneg as f64);
    }
    // time error
    let time_of = |filter: &dyn Fn(&[f32]) -> bool| -> TimeErr {
        let mut errs: Vec<f64> = Vec::new();
        let mut bias = 0f64;
        for i in 0..set.n {
            let l = set.lab(i);
            if l[L_Y] <= 0.5 || l[L_TICKS] < 0.0 || !filter(l) {
                continue;
            }
            let pred_ticks = decode(&pred[i], l[L_DIST]).expected_ticks as f64;
            let e = (pred_ticks - l[L_TICKS] as f64) * 0.010;
            bias += e;
            errs.push(e.abs());
        }
        let n = errs.len();
        errs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        TimeErr { n, mae_s: errs.iter().sum::<f64>() / n.max(1) as f64, bias_s: bias / n.max(1) as f64, median_abs_s: errs.get(n / 2).cloned().unwrap_or(0.0) }
    };
    Report { two_gate: tg, calib: cal, time_human: time_of(&|l| l[L_HUMAN] > 0.5), time_all_pos: time_of(&|_| true) }
}

pub fn render(name: &str, r: &Report) -> String {
    let mut s = String::new();
    let tg = &r.two_gate;
    s.push_str(&format!(
        "[{name}] two-gate: {} pairs from {} records — (R {:.1} %, distance {:.1} %) margin {:+.1} pts [{}]; ties R {} / distance {}. Nearest-negative pairing: R {:.1} % vs distance {:.1} % over {} pairs. DISTANCE-WRONG pairs (negative at least as near as the reached gate): {} of {} pos×neg pairs ({:.1} %), R correct on {:.1} %. MATCHED-DISTANCE pairs (negative within ±10 % of the positive's distance): {} pairs, R {:.1} % vs distance {:.1} %\n",
        tg.pairs, tg.records, tg.model_pct(), tg.baseline_pct(), tg.margin(), if tg.informative() { "informative" } else { "UNINFORMATIVE: distance ≥ 99 %" }, tg.model_ties, tg.baseline_ties, tg.hard_model_pct(), tg.hard_baseline_pct(), tg.hard_pairs, tg.dw_pairs, tg.all_pairs, 100.0 * tg.dw_pairs as f64 / tg.all_pairs.max(1) as f64, tg.dw_model_pct(), tg.md_pairs, tg.md_model_pct(), tg.md_baseline_pct()
    ));
    let c = &r.calib;
    s.push_str(&format!("[{name}] calibration over {} rows (positive rate {:.3}): ECE {:.4}, Brier {:.4}, AUC {:.4}\n", c.n, c.pos_rate, c.ece, c.brier, c.auc));
    s.push_str(&format!("[{name}]   bin      n   mean p   frac +\n"));
    for (b, (n, mp, fp)) in c.bins.iter().enumerate() {
        if *n > 0 {
            s.push_str(&format!("[{name}]   {:.1}-{:.1} {:>7}   {:.3}    {:.3}\n", b as f64 / 10.0, (b + 1) as f64 / 10.0, n, mp, fp));
        }
    }
    let t = &r.time_human;
    s.push_str(&format!("[{name}] expected time vs the HUMAN leg time: {} legs, MAE {:.3} s, median |err| {:.3} s, bias {:+.3} s\n", t.n, t.mae_s, t.median_abs_s, t.bias_s));
    let t = &r.time_all_pos;
    s.push_str(&format!("[{name}] expected time vs the crossing tick, all positives: {} rows, MAE {:.3} s, median |err| {:.3} s, bias {:+.3} s\n", t.n, t.mae_s, t.median_abs_s, t.bias_s));
    s
}
