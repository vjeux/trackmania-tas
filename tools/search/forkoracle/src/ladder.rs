//! DEEP FORK POINTS: the savestate tree under the tape search.
//!
//! The fork evaluator used to have ONE fork point per worker -- the server's
//! checkpoint, a few ticks into the race -- and every candidate was a child of
//! it that re-simulated everything from there. A candidate that edits tick 1800
//! of a 2400-tick tape therefore paid 1600 ticks of a prefix it did not change,
//! at 27.6 us a tick, before it did anything the search wanted to know about.
//!
//! A [`Ladder`] keeps a few savestate nodes (`tree::Node`: a paused simulation
//! that can be forked again) along the tape the search is currently editing, on
//! a fixed grid of ticks, and forks each candidate from the DEEPEST node that
//! agrees with it on every tick the node has already consumed. The candidate
//! then pays only the ticks it can actually change.
//!
//! # What makes this exact rather than merely fast
//!
//! * **A node is usable for a candidate only if the candidate's tape equals the
//!   node's on `[0, node.tick)`.** Not "the candidate descends from the
//!   incumbent", not "the mutation is above the node": the bytes. The node's
//!   process consumed those records; a candidate that differs in any of them is
//!   a different run and gets a shallower node. [`Rung::usable`] is one prefix
//!   comparison per node per candidate, and it is the whole safety argument.
//! * **Every node probes its own boundary** (`tree::Node::probe`) and every run
//!   from it writes from that boundary, never from the tick it was asked to
//!   stop at. The forward-only refusal in `tree::Node` still stands underneath;
//!   this module never asks it for anything below.
//! * **Every run rewrites the whole tail** from the node's tick to the end of
//!   the tape, so whatever a node's process happens to hold above its tick is
//!   irrelevant.
//! * **Nodes are warm** ([`BranchReq::watched`]): a node made under an armed
//!   watchdog carries the evaluator's state -- speed history, progress, gate and
//!   event records -- up to its own tick, so a watched run from it is the
//!   continuation of the run the root would have made and returns the same
//!   summary. Without that, a fork at tick 1800 would start with an empty speed
//!   window and no memory of a gate the car went through at tick 900.
//!
//! # Policy
//!
//! Grid: `root_from + k * spacing`. A batch of candidates that share a prefix of
//! `common` ticks wants one node at the deepest grid tick `<= common`; if it is
//! missing it is made from the deepest usable node -- ONE branch per batch at
//! most, patched with the batch's own records, so making it costs the prefix the
//! batch's first candidate would have simulated anyway, plus a fork, a handshake
//! and a probe. Nodes are evicted least-recently-used at `cap`; a node whose
//! lineage the search has left behind ages out on its own. Every decision is
//! counted in [`Stats`] and a failure to make a node is never an error: the
//! candidate runs from the shallower point, exactly as before.

use crate::forksrv::{rec_of, BranchReq, ForkServer, Rec};
use crate::inputs::Inputs;
use crate::tree::{Node, Tree};
use std::path::Path;

/// Everything the ladder decided, for the record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub runs: u64,
    pub from_root: u64,
    pub from_rung: u64,
    /// Sum over runs of the ticks NOT re-simulated: fork tick minus the root's.
    pub ticks_saved: u64,
    pub rungs_made: u64,
    pub rungs_failed: u64,
    pub rungs_evicted: u64,
    /// Wall time spent making rungs, microseconds.
    pub make_us: u64,
    /// Rungs whose probe did not land on the grid tick asked for. Not an error
    /// -- the probe is the truth and the rung is used at its probed tick -- but
    /// worth knowing about.
    pub off_grid: u64,
}

struct Rung {
    node: Node,
    /// The first tick this rung has NOT consumed: its probed boundary, and the
    /// first tick a run from it rewrites.
    tick: usize,
    /// The tape this rung's process holds below `tick`. Only the prefix
    /// matters; the whole tape is kept because that is what the caller had.
    tape: Inputs,
    last_used: u64,
}

impl Rung {
    /// May `cand` be run from this rung? Only if the two tapes agree on every
    /// record the rung has already consumed.
    fn usable(&self, cand: &Inputs) -> bool {
        let t = self.tick;
        t <= cand.len()
            && t <= self.tape.len()
            && cand.steer[..t] == self.tape.steer[..t]
            && cand.gas[..t] == self.tape.gas[..t]
            && cand.brake[..t] == self.tape.brake[..t]
    }
}

pub struct Ladder {
    tree: Tree,
    rungs: Vec<Rung>,
    /// The root's own tick (its probed boundary) and the first tick a run may
    /// write there. They differ only when a calibration pushed the floor later.
    root_tick: usize,
    root_from: usize,
    spacing: usize,
    cap: usize,
    watched: bool,
    seq: u64,
    stats: Stats,
    /// Failures to make a node are logged three times per ladder and counted
    /// after that: a worker that cannot branch should say so, not fill a log.
    failures_logged: u32,
}

/// How long to wait for a branch child to reach its tick and connect. A
/// 2400-tick branch is ~70 ms of simulation on an idle core; under a full box
/// a few hundred ms. A child that dies on the way (a predicate tripped during
/// warm-up, the base check failed) connects and says so, so this only bounds
/// a child that vanished.
const ACCEPT_TIMEOUT_MS: i32 = 30_000;

impl Ladder {
    /// `work` is the worker's own directory (the socket lives in it).
    /// `root_tick` is the root server's probed boundary and `root_from` the
    /// first tick a run may write there. `watched` makes every rung warm --
    /// pass it whenever runs will be [`Ladder::run_watched`].
    pub fn new(
        work: &Path,
        root_tick: usize,
        root_from: usize,
        spacing: usize,
        cap: usize,
        watched: bool,
    ) -> Result<Ladder, String> {
        if root_from < root_tick {
            return Err(format!(
                "a ladder whose write floor ({}) is below the root's own tick ({}) would write \
                 consumed records",
                root_from, root_tick
            ));
        }
        Ok(Ladder {
            tree: Tree::new(work)?,
            rungs: Vec::new(),
            root_tick,
            root_from,
            spacing: spacing.max(1),
            cap: cap.max(1),
            watched,
            seq: 0,
            stats: Stats::default(),
            failures_logged: 0,
        })
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn live(&self) -> usize {
        self.rungs.len()
    }

    pub fn spacing(&self) -> usize {
        self.spacing
    }

    /// The first tick at which any two candidates of a batch differ -- the
    /// prefix every one of them shares, and therefore the deepest tick a node
    /// can sit at and still serve the whole batch. A batch of one shares
    /// nothing with anybody, so it gets 0 and makes no node.
    pub fn common_prefix(cands: &[Inputs]) -> usize {
        if cands.len() < 2 {
            return 0;
        }
        let a = &cands[0];
        let mut common = a.len();
        for c in &cands[1..] {
            let n = a.len().min(c.len());
            let mut t = 0;
            while t < n && a.steer[t] == c.steer[t] && a.gas[t] == c.gas[t] && a.brake[t] == c.brake[t] {
                t += 1;
            }
            common = common.min(t);
        }
        common
    }

    /// The deepest grid tick at or below `t`, or `None` for the root itself.
    fn grid_at_or_below(&self, t: usize) -> Option<usize> {
        if t <= self.root_from {
            return None;
        }
        let k = (t - self.root_from) / self.spacing;
        if k == 0 {
            None
        } else {
            Some(self.root_from + k * self.spacing)
        }
    }

    /// Index of the deepest rung `cand` may run from, if any.
    fn deepest_usable(&self, cand: &Inputs) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, r) in self.rungs.iter().enumerate() {
            if r.usable(cand) && best.map(|b| r.tick > self.rungs[b].tick).unwrap_or(true) {
                best = Some(i);
            }
        }
        best
    }

    /// Make sure the node a batch wants exists: the deepest grid tick at or
    /// below `common` (the prefix the batch shares), along `cand`'s tape. At
    /// most one node is made per call, from the deepest usable one.
    ///
    /// Never fails: a node that cannot be made is counted and the batch runs
    /// from the shallower point.
    pub fn prepare(&mut self, root: &mut ForkServer, cand: &Inputs, common: usize) {
        let n = cand.len();
        let want = match self.grid_at_or_below(common.min(n.saturating_sub(1))) {
            Some(t) => t,
            None => return,
        };
        let base = self.deepest_usable(cand);
        let base_tick = base.map(|i| self.rungs[i].tick).unwrap_or(self.root_tick);
        if want <= base_tick {
            return;
        }
        self.seq += 1;
        if let Some(i) = base {
            self.rungs[i].last_used = self.seq;
        }
        if let Err(e) = self.make(root, base, cand, want) {
            self.stats.rungs_failed += 1;
            if self.failures_logged < 3 {
                self.failures_logged += 1;
                eprintln!(
                    "ladder: could not make a node at tick {} (running the batch from {} instead): {}",
                    want, base_tick, e
                );
            }
        }
    }

    fn make(
        &mut self,
        root: &mut ForkServer,
        base: Option<usize>,
        cand: &Inputs,
        want: usize,
    ) -> Result<(), String> {
        let t0 = std::time::Instant::now();
        // Room first. The base was just marked used and is never the victim;
        // `deepest_usable` is recomputed after every eviction because
        // `swap_remove` renumbers the rungs.
        while self.rungs.len() >= self.cap {
            let keep = base.and(self.deepest_usable(cand));
            let victim = (0..self.rungs.len())
                .filter(|&i| Some(i) != keep)
                .min_by_key(|&i| self.rungs[i].last_used);
            match victim {
                Some(v) => self.evict(v),
                None => break,
            }
        }
        let base = base.and(self.deepest_usable(cand));
        let (from, base_tick) = match base {
            Some(i) => (self.rungs[i].tick, self.rungs[i].tick),
            None => (self.root_from, self.root_tick),
        };
        if want <= base_tick || want >= cand.len() {
            return Err(format!("grid tick {} is not above the base ({}) and below the tape end", want, base_tick));
        }
        // The whole tail from the base's write floor, so the new node holds the
        // batch's records everywhere it can still read them.
        let recs: Vec<Rec> = (from..cand.len())
            .map(|t| rec_of(cand.steer[t] as u8, cand.gas[t] as u8, cand.brake[t] as u8))
            .collect();
        let sock = self.tree.sock_path();
        let req = BranchReq {
            from,
            recs: &recs,
            stop_after: (want - base_tick) as u64,
            sock: &sock,
            trace_path: "",
            segs: &[],
            sample_stride: 1,
            sample_max: 0,
            key: (0, 1),
            watched: self.watched,
        };
        let pid = match base {
            Some(i) => self.rungs[i].node.branch(&req)?,
            None => root.branch(&req)?,
        };
        let mut node = self.tree.accept(ACCEPT_TIMEOUT_MS)?;
        if node.pid != pid {
            // A node is only the node you asked for if it says so itself.
            let got = node.pid;
            node.destroy();
            self.tree.reaped(got);
            return Err(format!("asked for pid {} and pid {} arrived", pid, got));
        }
        let tick = match node.probe() {
            Ok(t) => t,
            Err(e) => {
                node.destroy();
                self.tree.reaped(pid);
                return Err(format!("node {} could not probe its own boundary: {}", pid, e));
            }
        };
        if tick != want {
            self.stats.off_grid += 1;
        }
        if tick <= base_tick || tick > cand.len() {
            node.destroy();
            self.tree.reaped(pid);
            return Err(format!(
                "node {} probed tick {}, which is not between its base ({}) and the tape end ({})",
                pid,
                tick,
                base_tick,
                cand.len()
            ));
        }
        self.rungs.push(Rung { node, tick, tape: cand.clone(), last_used: self.seq });
        self.stats.rungs_made += 1;
        self.stats.make_us += t0.elapsed().as_micros() as u64;
        Ok(())
    }

    fn evict(&mut self, i: usize) {
        let mut r = self.rungs.swap_remove(i);
        let pid = r.node.pid;
        r.node.destroy();
        self.tree.reaped(pid);
        self.stats.rungs_evicted += 1;
    }

    /// The tail of `cand` from `from`, in the engine's representation.
    fn tail(cand: &Inputs, from: usize) -> Vec<Rec> {
        (from..cand.len())
            .map(|t| rec_of(cand.steer[t] as u8, cand.gas[t] as u8, cand.brake[t] as u8))
            .collect()
    }

    /// Run `cand` to the finish from its deepest usable fork point. Returns
    /// the validator's JSON and the tick the run was forked at.
    pub fn run(&mut self, root: &mut ForkServer, cand: &Inputs) -> (String, usize) {
        self.seq += 1;
        self.stats.runs += 1;
        loop {
            match self.deepest_usable(cand) {
                Some(i) => {
                    let from = self.rungs[i].tick;
                    self.rungs[i].last_used = self.seq;
                    match self.rungs[i].node.run(from, &Self::tail(cand, from)) {
                        Ok(j) => {
                            self.stats.from_rung += 1;
                            self.stats.ticks_saved += (from - self.root_tick) as u64;
                            return (j, from);
                        }
                        Err(e) => {
                            // A node that stopped answering is gone; the next
                            // shallower point gets the candidate.
                            eprintln!("ladder: node at tick {} failed ({}); dropping it", from, e);
                            self.evict(i);
                        }
                    }
                }
                None => {
                    self.stats.from_root += 1;
                    return (root.run(self.root_from, &Self::tail(cand, self.root_from)), self.root_from);
                }
            }
        }
    }

    /// As [`Ladder::run`], with the watchdog: JSON, summary block, fork tick.
    pub fn run_watched(&mut self, root: &mut ForkServer, cand: &Inputs) -> (String, Vec<u8>, usize) {
        self.seq += 1;
        self.stats.runs += 1;
        loop {
            match self.deepest_usable(cand) {
                Some(i) => {
                    let from = self.rungs[i].tick;
                    self.rungs[i].last_used = self.seq;
                    match self.rungs[i].node.run_watched(from, &Self::tail(cand, from)) {
                        Ok((j, b)) => {
                            self.stats.from_rung += 1;
                            self.stats.ticks_saved += (from - self.root_tick) as u64;
                            return (j, b, from);
                        }
                        Err(e) => {
                            eprintln!("ladder: node at tick {} failed ({}); dropping it", from, e);
                            self.evict(i);
                        }
                    }
                }
                None => {
                    self.stats.from_root += 1;
                    let (j, b) = root.run_watched(self.root_from, &Self::tail(cand, self.root_from));
                    return (j, b, self.root_from);
                }
            }
        }
    }

    /// The ticks of the live rungs, shallowest first. For reports and tests.
    pub fn rung_ticks(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.rungs.iter().map(|r| r.tick).collect();
        v.sort_unstable();
        v
    }
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} runs, {} from a rung ({:.1}%), {} ticks not re-simulated; rungs made {} \
             (failed {}, evicted {}, off-grid {}), {:.1} ms making them",
            self.runs,
            self.from_rung,
            100.0 * self.from_rung as f64 / self.runs.max(1) as f64,
            self.ticks_saved,
            self.rungs_made,
            self.rungs_failed,
            self.rungs_evicted,
            self.off_grid,
            self.make_us as f64 / 1000.0
        )
    }
}

impl Stats {
    pub const ZERO: Stats = Stats {
        runs: 0,
        from_root: 0,
        from_rung: 0,
        ticks_saved: 0,
        rungs_made: 0,
        rungs_failed: 0,
        rungs_evicted: 0,
        make_us: 0,
        off_grid: 0,
    };

    /// Fold another worker's stats into this one.
    pub fn add(&mut self, o: &Stats) {
        self.runs += o.runs;
        self.from_root += o.from_root;
        self.from_rung += o.from_rung;
        self.ticks_saved += o.ticks_saved;
        self.rungs_made += o.rungs_made;
        self.rungs_failed += o.rungs_failed;
        self.rungs_evicted += o.rungs_evicted;
        self.make_us += o.make_us;
        self.off_grid += o.off_grid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tape(n: usize, seed: u8) -> Inputs {
        Inputs {
            steer: (0..n).map(|t| ((t * 7 + seed as usize) % 200) as i8).collect(),
            gas: (0..n).map(|t| t % 3 != 0).collect(),
            brake: (0..n).map(|t| t % 11 == 0).collect(),
        }
    }

    /// The prefix a batch shares is the first tick any two of them differ at,
    /// on any of the three axes -- and a batch of one shares nothing.
    #[test]
    fn the_common_prefix_is_the_first_tick_any_two_candidates_differ_at() {
        let a = tape(100, 1);
        let mut b = a.clone();
        b.steer[60] = b.steer[60].wrapping_add(1);
        let mut c = a.clone();
        c.brake[40] = !c.brake[40];
        let mut d = a.clone();
        d.gas[90] = !d.gas[90];
        assert_eq!(Ladder::common_prefix(&[a.clone(), b.clone()]), 60);
        assert_eq!(Ladder::common_prefix(&[a.clone(), b.clone(), c.clone()]), 40);
        assert_eq!(Ladder::common_prefix(&[a.clone(), d.clone()]), 90);
        assert_eq!(Ladder::common_prefix(&[a.clone(), a.clone()]), 100, "identical tapes share everything");
        assert_eq!(Ladder::common_prefix(&[a.clone()]), 0, "a batch of one shares nothing");
        assert_eq!(Ladder::common_prefix(&[]), 0);
    }

    /// The grid is anchored at the root's write floor, and the root itself is
    /// never a grid tick.
    #[test]
    fn the_grid_sits_above_the_root_floor() {
        let dir = std::env::temp_dir().join(format!("ladder-grid-{}", std::process::id()));
        let l = Ladder::new(&dir, 171, 171, 100, 8, false).unwrap();
        assert_eq!(l.grid_at_or_below(171), None);
        assert_eq!(l.grid_at_or_below(270), None);
        assert_eq!(l.grid_at_or_below(271), Some(271));
        assert_eq!(l.grid_at_or_below(370), Some(271));
        assert_eq!(l.grid_at_or_below(1800), Some(1771));
        assert_eq!(l.grid_at_or_below(0), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A write floor below the root's own tick is refused up front.
    #[test]
    fn a_floor_below_the_root_tick_is_refused() {
        let dir = std::env::temp_dir().join(format!("ladder-floor-{}", std::process::id()));
        assert!(Ladder::new(&dir, 171, 170, 100, 8, false).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
