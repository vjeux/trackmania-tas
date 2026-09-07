//! Beam search over (visited set, current gate group, arrival bucket): every
//! checkpoint once, finish last (any finish group). Ranked by
//! `Σ expected_ms + PENALTY_MS · Σ −ln p_reach`.

use crate::estimator::{Edge, EdgeEstimator, EdgeKind, StateBucket};
use crate::surface::Nodes;

pub const PENALTY_MS: f32 = 3000.0;

#[derive(Clone, Debug)]
pub struct Plan {
    /// Node indices visited, spawn first, finish last.
    pub visit: Vec<usize>,
    pub edges: Vec<Edge>,
    pub total_ms: i32,
    pub p_reach: f32,
    pub score: f32,
}

#[derive(Clone)]
struct Partial {
    mask: u64,
    at: usize,
    bucket: StateBucket,
    visit: Vec<usize>,
    edges: Vec<Edge>,
    ms: i32,
    logp: f32,
}

impl Partial {
    fn score(&self) -> f32 {
        self.ms as f32 - PENALTY_MS * self.logp
    }
}

pub fn beam(nodes: &Nodes, est: &dyn EdgeEstimator, width: usize, top_k: usize, start_bucket: StateBucket) -> Vec<Plan> {
    let n_cp = nodes.n_cp;
    assert!(n_cp <= 60, "beam mask is u64");
    let full = if n_cp == 0 { 0 } else { (1u64 << n_cp) - 1 };
    let mut frontier = vec![Partial { mask: 0, at: 0, bucket: start_bucket, visit: vec![0], edges: vec![], ms: 0, logp: 0.0 }];
    for _ in 0..n_cp {
        let mut next: Vec<Partial> = Vec::new();
        for p in &frontier {
            for cp in 0..n_cp {
                if p.mask & (1 << cp) != 0 {
                    continue;
                }
                let to = 1 + cp;
                let prev = if p.visit.len() >= 2 { Some(p.visit[p.visit.len() - 2]) } else { None };
                let e = est.estimate(p.bucket, prev, p.at, to);
                if e.kind == EdgeKind::None || e.p_reach <= 0.0 {
                    continue;
                }
                let mut q = p.clone();
                q.mask |= 1 << cp;
                q.at = to;
                q.bucket = e.arrival;
                q.visit.push(to);
                q.edges.push(e);
                q.ms += e.expected_ms;
                q.logp += e.p_reach.ln();
                next.push(q);
            }
        }
        // dedup identical (mask, at, bucket) keeping the best score, then cut to width
        next.sort_by(|a, b| a.score().partial_cmp(&b.score()).unwrap());
        let mut seen = std::collections::HashSet::new();
        next.retain(|q| seen.insert((q.mask, q.at, q.bucket)));
        next.truncate(width);
        frontier = next;
        if frontier.is_empty() {
            return vec![];
        }
    }
    let mut plans: Vec<Plan> = Vec::new();
    for p in &frontier {
        debug_assert_eq!(p.mask, full);
        for f in nodes.finish_range() {
            let prev = if p.visit.len() >= 2 { Some(p.visit[p.visit.len() - 2]) } else { None };
            let e = est.estimate(p.bucket, prev, p.at, f);
            if e.kind == EdgeKind::None || e.p_reach <= 0.0 {
                continue;
            }
            let mut visit = p.visit.clone();
            visit.push(f);
            let mut edges = p.edges.clone();
            edges.push(e);
            let ms = p.ms + e.expected_ms;
            let logp = p.logp + e.p_reach.ln();
            plans.push(Plan { visit, edges, total_ms: ms, p_reach: logp.exp(), score: ms as f32 - PENALTY_MS * logp });
        }
    }
    plans.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
    plans.truncate(top_k);
    plans
}

/// Exact Held–Karp over the COST matrix (the cartographer's objective): spawn
/// first, every checkpoint once, one finish group last (each tried). For the
/// beam's control on maps with ≤ 16 checkpoints. Returns the visit order and cost.
pub fn exact_cost(nodes: &Nodes, d: &[Vec<f32>]) -> Option<(Vec<usize>, f32)> {
    let n_cp = nodes.n_cp;
    if n_cp > mapgeom::surf::EXACT_MAX {
        return None;
    }
    let mut best: Option<(Vec<usize>, f32)> = None;
    for f in nodes.finish_range() {
        // sub-matrix: 0 = spawn, 1..=n_cp = checkpoints, last = this finish
        let idx: Vec<usize> = std::iter::once(0).chain(1..=n_cp).chain(std::iter::once(f)).collect();
        let sub: Vec<Vec<f32>> = idx.iter().map(|&i| idx.iter().map(|&j| d[i][j]).collect()).collect();
        let o = mapgeom::surf::order_gates(&sub);
        if o.cost >= mapgeom::surf::UNREACHABLE {
            continue;
        }
        let visit: Vec<usize> = o.visit.iter().map(|&k| idx[k]).collect();
        if best.as_ref().map_or(true, |b| o.cost < b.1) {
            best = Some((visit, o.cost));
        }
    }
    best
}
