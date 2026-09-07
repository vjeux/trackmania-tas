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
    beam_from(nodes, est, width, top_k, start_bucket, 0, None)
}

/// `beam` from an arbitrary start node, with the finish restricted to `finish_only` (node indices) when given —
/// a lap race's first lap ends at the lap line, its later laps start there.
pub fn beam_from(nodes: &Nodes, est: &dyn EdgeEstimator, width: usize, top_k: usize, start_bucket: StateBucket, start: usize, finish_only: Option<&[usize]>) -> Vec<Plan> {
    let n_cp = nodes.n_cp;
    assert!(n_cp <= 60, "beam mask is u64");
    let full = if n_cp == 0 { 0 } else { (1u64 << n_cp) - 1 };
    let mut frontier = vec![Partial { mask: 0, at: start, bucket: start_bucket, visit: vec![start], edges: vec![], ms: 0, logp: 0.0 }];
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
            if let Some(only) = finish_only {
                if !only.contains(&f) {
                    continue;
                }
            }
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

/// A LAP race (`gates.laps` ≥ 2): lap 1 spawn → every checkpoint → the lap line; laps 2..n lap line → the same
/// checkpoint order → the lap line, the last one ending at a plain finish when the map has one. The checkpoint
/// order is planned once (from the spawn) and repeated — a per-lap order is a refinement for later.
pub fn beam_laps(nodes: &Nodes, kinds: &[tmroute::gates::WpKind], laps: u32, est: &dyn EdgeEstimator, width: usize, top_k: usize, start_bucket: StateBucket) -> Vec<Plan> {
    let lap_lines: Vec<usize> = nodes.finish_range().filter(|&f| kinds[f] == tmroute::gates::WpKind::Multilap).collect();
    let plain: Vec<usize> = nodes.finish_range().filter(|&f| kinds[f] == tmroute::gates::WpKind::Finish).collect();
    if laps < 2 || lap_lines.is_empty() {
        return beam(nodes, est, width, top_k, start_bucket);
    }
    let firsts = beam_from(nodes, est, width, top_k, start_bucket, 0, Some(&lap_lines));
    let last_finish: Vec<usize> = if plain.is_empty() { lap_lines.clone() } else { plain.clone() };
    firsts
        .into_iter()
        .map(|p1| {
            let cps: Vec<usize> = p1.visit[1..p1.visit.len() - 1].to_vec();
            let lap_line = *p1.visit.last().unwrap();
            let mut visit = p1.visit.clone();
            let mut edges = p1.edges.clone();
            let mut ms = p1.total_ms;
            let mut logp = p1.p_reach.ln();
            let mut bucket = edges.last().map(|e| e.arrival).unwrap_or(start_bucket);
            for lap in 2..=laps {
                let mut at = lap_line;
                let mut prev = Some(visit[visit.len() - 2]);
                let ends: &[usize] = if lap == laps { &last_finish } else { std::slice::from_ref(&lap_line) };
                for &cp in cps.iter().chain(std::iter::once(&ends[0])) {
                    let e = est.estimate(bucket, prev, at, cp);
                    ms += e.expected_ms.max(0);
                    logp += e.p_reach.max(1e-6).ln();
                    bucket = e.arrival;
                    prev = Some(at);
                    at = cp;
                    visit.push(cp);
                    edges.push(e);
                }
            }
            Plan { visit, edges, total_ms: ms, p_reach: logp.exp(), score: ms as f32 - PENALTY_MS * logp }
        })
        .collect()
}
