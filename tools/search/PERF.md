# PERF.md — the fork oracle, made faster, one proven lever at a time

Branch `agentcloud/perf`, on top of `agentcloud/tickhook`. Every section is one
optimization: what it is, the proof with its controls, the numbers before and
after, and what it replaced (deleted, not flagged off — "don't keep old code").
The cost model every number is measured against is TICKHOOK.md §9: a candidate
was **9.9 ms fixed + 27.6 µs per simulated tick** before this work, and the
exit-at-finish lever (tickhook, 2997634) took ~3.7 ms off the fixed part for a
candidate that finishes.

Boxes: `devvm60585` (www, 96 cores, kernel 6.16, THP `madvise`), server build
128182 (`0f0f4b25…`). Maps: map 2 (`rank00001_22730`, 2432 ticks), 126859
(`rank01_24342`, 2598 ticks), 145875 (`r01_6346`, 789 ticks). Raw logs of every
run quoted here: `~/persistent/private-30d/tm-perf/evidence/`.

---

## 1. Deep fork points for the tape search — `forkoracle::ladder`

### What

The fork evaluator had ONE fork point per worker, the server's checkpoint at
tick 171, and every candidate re-simulated everything from it. A candidate that
edits tick 1800 of a 2400-tick tape paid 1600 ticks of a prefix it did not
change — 44 ms of the 60 it cost.

A `Ladder` keeps savestate nodes (`tree::Node`: a paused engine that can be
forked again) along the tape the worker is editing, on a grid every 100 ticks,
and forks each candidate from the **deepest node whose tape equals the
candidate's on every tick the node has consumed** — one prefix comparison per
node per candidate, and that comparison is the whole safety argument. Every
node probes its own boundary; every run rewrites the whole tail from the node's
tick. One node is made per batch at most (the search's batch shares a prefix:
the incumbent up to the earliest edit), from the deepest usable node, so making
it costs the prefix the first candidate would have simulated anyway plus a
fork, a handshake and a probe (~12 ms). Nodes are evicted least-recently-used
at 32 per worker.

**Nodes are warm.** The watchdog is stateful — a speed window, consecutive-tick
counters, progress along the line, the gate and event records — so a fork point
deep in the tape only returns the root's verdict if the node carries that state
up to its own tick. `BranchReq::watched` makes the branch child run its ticks
watched, on a report page of its own lineage; when it re-enters as a node it
keeps the evaluator (`NODE_WARM`), and every `'W'` child forked from it
continues that run instead of resetting. A node made from a warm node continues
its state. A lineage that trips a predicate during warm-up connects to the
driver's socket and says so instead of leaving it to time out.

### Proof

**Mechanism, no engine** (`forkshim/tests/ladder.rs`, against `shimhost`, whose
verdict is a hash of the records it consumed): a candidate run from a node 1300
ticks in hashes to exactly the records the root would have consumed; a node of
another lineage is never used below its own tick (and two nodes at the same
tick of two lineages give different verdicts for the same tail — which is why
one cannot stand in for the other); LRU eviction at the cap.

**Engine, unwatched** (`fk ladder check`, 1000 candidates per map, each one
edit of up to 60 ticks at a position uniform over the whole tape, run three
ways):

| map | full validation = root fork = ladder | finished / DNF |
|---|---|---|
| map 2 rank00001, root tick 171 | **1000 / 1000** | 289 / 711 |
| 145875 r01, root tick 200 | **1000 / 1000** | 350 / 650 |
| 126859 rank01, root tick 300 | 999 / 1000 — root fork **= ladder** on all 1000; the one candidate the plain oracle disagrees with is c0516, edited at tick 2106, which the fork (root AND ladder) calls DNF and a batch validation calls 26.839 — a finish 2.4 s **past the tape's own end** (2598 ticks, start offset −1550 → the tape ends at race 24.43). See "a finding about the plain oracle" below. | 203 / 797 |

Oracle repeatability 0 of 1000 differ on every map.

**Engine, watched** (`fk ladder watched`: the search's own predicate set armed,
exit-at-finish armed, root `'W'` against warm-node `'W'`, the time and the
152-byte summary compared byte for byte — trip, tick, value, progress,
travelled, speeds, plane crossing, gate, event):

| map | identical time AND byte-identical summary | tripped / finished / forked deeper than the root |
|---|---|---|
| map 2 | **1000 / 1000** | 650 / 187 / 966 |
| 126859 | **1000 / 1000** | 608 / 271 / 954 |
| 145875 | **1000 / 1000** | 302 / 359 / 840 |

So an aborted candidate aborts at the same tick on the same predicate with the
same value, a finisher has the same time, and progress — the DNF score — is the
same number. The score-safety invariant (`progress(aborted) ≤ progress(unarmed)`)
is untouched because the summaries are identical to the root's, whose invariant
was measured 2000/2000.

**A search, end to end.** Map 2, `--fork --forktick 171`, 24 workers, 10
minutes, seed 42, guard on, the boundary-stress window (`--lo 171 --window 60
--stride 400`), baseline (`agentcloud/tickhook` @ 361d288) and this branch run
CONCURRENTLY on the same box:

| build | evals | eval/s | improvements confirmed by the plain oracle | phantoms | best |
|---|---|---|---|---|---|
| tickhook @ 361d288 | 324 300 | 538 | 8 | **0** | 22.711 |
| + deep fork points | **637 830** | **1060** | 12 | **0** | **22.710** |

82.2 % of candidates forked from a node; 576 M ticks not re-simulated; 1522
nodes made (754 evicted), 31.4 s making them out of 240 worker-minutes.

### Cost, per depth of the edit (map 2, `fk ladder check`, root tick 171)

| first edit in | n | root fork ms/cand | ladder ms/cand | speedup |
|---|---|---|---|---|
| 0–25 % of the tape | 237 | 47.3 | 41.9 | 1.13× |
| 25–50 % | 265 | 68.5 | 33.9 | 2.02× |
| 50–75 % | 253 | 60.8 | 27.8 | 2.19× |
| 75–90 % | 142 | 63.6 | 22.5 | 2.82× |
| 90–100 % | 103 | 62.8 | 12.4 | 5.06× |

(126859: 1.10 / 1.51 / 1.98 / 2.93 / 3.92×; 145875, a 789-tick tape: 1.02 /
1.11 / 1.32 / 1.62 / 1.88×.) The root fork's cost varies with the edit's
position because an early edit that crashes the car makes the rest of its
ticks cheap; a late edit drives the whole track.

### What was deleted

The evaluator's single fork point: `ForkEval::evaluate` no longer calls
`srv.run_watched(self.from, …)`; every candidate goes through the ladder, and
the root is simply the ladder's shallowest point. There is no flag that
restores the old behaviour. `Provenance::resume_tick` now reports the tick the
candidate was actually forked at.

### A finding about the plain oracle, not about the fork

The one 126859 disagreement is a class, not a fluke: **a tape that runs off its
own end can get a different answer from the plain oracle depending on what
else is validated in the same server launch.** c0516 is DNF when validated
alone or in a batch of 8, and 26.839 in a batch of 520 or 1000 (deterministic
given the batch: the harness's two full runs agreed with each other). Six more
candidates in that 520 got times past the tape's end (24.458 … 26.289). The
engine keeps simulating past the last record, and what it drives on depends on
the process's heap. The fork (root and ladder alike) says DNF, which is what a
single-file validation says. Consequences: (1) a fork-vs-oracle comparison must
treat "finishes after its own tape ends" as a separate class; (2) the batched
certification of §7 must not change a verdict for a claim inside its tape, and
its proof checks exactly that; (3) nothing the search banks is in this class —
a claim is a finish inside the tape, and the guard validates the written file
alone.

### Cleanup and rebase notes

* New: `forkoracle/src/ladder.rs`, `forkshim/tests/ladder.rs`, `fk ladder
  check|watched`. `BranchReq` has a `watched` field (the `'B'` payload gained a
  trailing flag word; an older shim ignores it); `Node::run_watched`;
  `payload_watched` is the one encoder `'W'` has.
* `pred_core::Summary` gained `lag_max_ms` (§2) — `SUMMARY_BYTES` is 152. Shim
  and driver share the file, so both sides moved together.
* Interface for the RL env (`branch::Forest`): unchanged; `watched: false`.
