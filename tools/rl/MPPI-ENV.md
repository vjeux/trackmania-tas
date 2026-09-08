# MPPI-ENV.md — what the env must expose for a policy-as-proposal search over the tree

A design note, no code (coordinator, 2026-09-07 04:15). Reading: PERF's
`tools/search/MPPI.md` (the planner: MPPI with the policy as the sampling
distribution, rollouts as fork children, the committed plan as a chain of
nodes) against what `tmenv` (`ForkEnv`, `Core`, `branch::Forest`) exposes today
on `agentcloud/player-env @ d1b12c57`. Written by the ENV arm.

## 1. What the planner needs, and what exists

| planner need (MPPI.md §1.5) | env today | gap |
|---|---|---|
| a NODE = paused engine at tick t with the plan's prefix | `ForkEnv::snapshot() -> SnapId` (pins the current node; LRU `pin_budget`, evicted states re-materialised by replaying the prefix, bit-equal — G4 50/50) | none for one plan; a planner that keeps P replicas of one node needs `clone_node(h) -> h'` (a `'B'` child with the same prefix, 1–3 ms) — Forest has it (`advance_or_end(h, &[], from, 1)` is a 1-tick clone; a 0-tick clone is a shim change, PERF's) |
| M ROLLOUTS of H ticks from a node, scored, no node made | nothing: `step_ticks` always makes a node; `coast(k)` is a node too | **`ForkEnv::rollout(h, actions: &[Action]) -> Rollout`** = `Node::run_watched(from, recs)` (an `'R'`/`'W'` child from the node's standby, PERF §3.2) with the sample ring on, decoded by the env into rows → `Core`-shaped result |
| the COST from the rollout | `Core::ingest(rows)` computes progress along DATA's geom.json, gates from the engine counter, off-route, Done — per row, driver-side | the `Rollout` must carry exactly what `Core` computes for a step, so the cost is the RL reward's own shape (§3); PERF's `Summary` (pred_core: progress along ITS corridor line, lag vs incumbent, trips) is a second definition of progress and must not be the planner's unless DATA's line and PERF's are one |
| the OBSERVATION at a node | `Core::observe` / `tmobs::observe_version(v, geom, state, prev)` for the CURRENT node only | per-node state: the snapshot already stores the `CoreState`; expose `ForkEnv::state_at(h) -> CarState` + `obs_at(h, version)` from it (no engine read) |
| P rollouts in PARALLEL from replicas | one `Forest` per worker, `&mut`, one command pipe per node — a node's requests are serial, DIFFERENT nodes have their own sockets | the Forest's node table must hand out `NodeHandle`s usable from P threads (each `Node` owns its socket; only the table and the root are shared) — a refactor of `branch::Forest` into `Root + Arc<Mutex<NodeTable>> + Node` |
| COMMIT the first macro = make the next node | `step_ticks(&[Action])` (k ticks, a node, leave the parent) | none; the parent must stay alive while the planner may back up: `step_ticks` today releases the parent (`leave_cur`) — the planner uses `snapshot()` before stepping, or a `step_keep_parent` flag |
| BACK UP to an earlier node | `reset_to(SnapId)` | none |
| the TAPE for the oracle | `banked_tape(&tape)` → `write_candidate` | none; the planner writes the committed prefix and offers it to the bank (`Bank::offer_many`, PERF §7) |
| exactness control | G4 (`reset-anywhere-control`: 50 `reset_to` bit-equal to a fresh run) | add: a rollout from a node == the same tape from the root (rows bit-equal), 1000/1000 on 3 maps — the `fk ladder check` shape |

## 2. The one new primitive: `rollout`

```rust
pub struct Rollout {
    pub rows: Vec<Row>,          // per tick (sample ring), the env's layout: pos/quat/vel/wet/cps/vis
    pub done: Option<Done>,      // Finished / OffRoute / NoProgress / Crash / RunEnded / TickCap, per Core's rules
    pub gates: usize,            // engine counter at the end
    pub progress_m: f32,         // best_s along geom.json (Core's), at the end
    pub finish_ms: Option<i64>,  // the engine's finish word, when it crossed
    pub reward: f32,             // Σ of Core's per-step reward over the rollout (the RL reward, same code)
    pub ticks: usize,
}
impl ForkEnv {
    pub fn rollout(&mut self, h: Handle, actions: &[Action]) -> Result<Rollout, String>;
}
```

* Engine side: `Node::run_watched` + the `'S'` ring (PERF: byte-identical blobs, 3.2 µs a sample). No new command.
* Env side: a SECOND `Core` instance per rollout, seeded from the node's `CoreState` (the snapshot's), fed the rows in
  k-tick chunks so `Done`/reward/progress are computed by the code the RL env uses. Cost: `Core::ingest` is ~1 µs a row.
* The rollout never becomes a node and never writes the tape; the planner commits with `step_ticks`.

## 3. The cost must be the reward

MPPI.md §1.4 proposes `S = −0.01·Δprogress + 0.30·Δt + [tripped] − 10·[finished] + c_lag·lag`. Two of its terms come
from PERF's watchdog `Summary` (progress along PERF's corridor line; lag behind the INCUMBENT at the same line point).
The env's reward is progress along DATA's geom.json (the field-median line; `Core::ingest`), gates from the engine
counter, and `Done` from the corridor rule. A planner scored on one line and a policy trained on another disagree on
every map where the lines differ (Summer 2026 - 01: the cartographer's first 150 m are on another road). Decision for
this note: **the planner's cost is computed by `Core` from the rollout's rows** (the same function, the same geometry
as the reward); PERF's lag-vs-incumbent term is a planner extra computed from `rows` against the incumbent's trace
(the env has the incumbent's rows from `open-loop-control`'s dump), not from the `Summary`.

## 4. Throughput this implies (from measured numbers)

Per rollout from a warm node (PERF §3, box A): 0.07 ms fork + 0.35 ms to the first tick + 24–30 µs/tick + 0.07 ms
after → H = 100 ticks ≈ 3.3 ms; `Core::ingest` of 100 rows ≈ 0.1 ms. M = 32 over P = 8 replicas: ~13 ms an
iteration; one commit (`step_ticks`, a `'B'` node) 2.0 ms (perf -f). **~65 planning steps a second per planner**, k =
10 ticks each — 6.5× real time on one planner, against the RL env's 14,600 env-steps/s at 96 workers with no lookahead
(G5 tmpfs table). P replicas cost P forks per commit (2 ms each) — with the 0-tick clone they are 0.07 ms.

## 5. What is NOT needed

* No new shim command: `'B'`, `'R'/'W'`, `'S'`, the ring and the standby cover it.
* No new geometry: geom.json is the one line (§3).
* No value network for the first version: MPPI.md §1.6's AWR fit from the weighted samples needs only `(obs, u*, S)`
  per step — the env's `obs_at` and the `Rollout` carry both.

## 6. Order of work, when it is asked for

1. `Forest` node table across threads (P replicas in parallel) — the refactor with the widest blast radius; G4 and
   known-answer must hold after it.
2. `ForkEnv::rollout` + the rollout exactness control (node rollout ≡ root replay, bit-equal rows, 1000/1000 on
   Summer 2026 - 01/02 and Spring 2026 - 12).
3. `state_at` / `obs_at` from snapshots; `step_ticks` keeping the parent on request.
4. The planner itself (LEARN's or a `tmmppi` sibling crate): policy sampling, MPPI average, commit, bank offer.

## 7. Cost estimate (asked for 2026-09-07 07:00; no API is approved yet)

| piece | what | effort | risk |
|---|---|---|---|
| `Forest` node table across threads | `Root + Arc<Mutex<NodeTable>> + Node` with per-node sockets usable from P threads; the root's command pipe stays single-owner | ~1 day | widest blast radius: every control (known-answer, G4, cp-oracle, reset-control) re-run after it |
| `ForkEnv::rollout(h, actions)` | a watched `'R'/'W'` child from the node's standby with the sample ring; rows → a second `Core` seeded from the node's `CoreState` → `Rollout` | ~0.5 day | the ring's byte-identity is PERF's proof; the second `Core` is the RL code unchanged |
| per-node `state_at` / `obs_at`; `step_ticks` keeping the parent | from `SnapKept` (persisted archive already carries the prefix; the state after the first re-materialisation) | ~0.25 day | none |
| the exactness control | node rollout ≡ root replay, rows bit-equal, 1000/1000 on Summer 2026 - 01/02 and Spring 2026 - 12; `fk ladder check`'s shape | ~0.25 day | a disagreement here stops everything |
| **total, env side** | | **~2 days** | |

Runtime cost per planning iteration at M = 32, H = 100, P = 8 (measured numbers, §4): ~13 ms wall on one worker
group (~2.5 core·s per second of planning per planner: 8 replicas × 24–30 µs/tick × 100 ticks × 4 rollouts each +
forks); 96 planners on box A would be ~6,200 planning steps/s at 10 ticks committed each = ~60k committed
game-ticks/s, against 144k game-ticks/s for the plain RL env at k = 10 — the planner spends ~2.4× the compute per
committed tick for M = 32 samples of lookahead. The proposal quality has to buy that back; the negative control
(uniform proposal vs the policy) is what says whether it does.
