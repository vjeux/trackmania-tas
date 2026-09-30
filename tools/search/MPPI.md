# MPPI.md — the policy as a proposal over the savestate tree, and a learned progress critic as an abort rule

Two designs the PERF arm was asked for (PERF.md items 8 and 9), written for the
LEARN arm of the player project. Both stand on machinery that exists and is
measured on `agentcloud/perf`: the savestate tree (`forkoracle::tree`,
`forkoracle::ladder`, `tools/search/branch`), the watchdog (`pred_core`,
`Summary`), the tick hook's timing page, the standby child and the sample ring
(PERF.md §3, §5). Nothing here is implemented; every number is one that was
measured tonight on a 96-core box and is cited where it comes from.

The principle both designs follow is the project's: **the fork path proposes,
the plain oracle decides.** A planner may rank on anything it likes; a result
is a FILE the oracle has agreed with, and the guard (`tmsearch::guard::Bank`)
stays the only way into the bank.

---

## 1. MPPI over the tree

### 1.1 What MPPI is, in this project's units

Model Predictive Path Integral control: from the current state, sample `M`
action sequences of horizon `H`, roll each forward in the simulator, score each
rollout with a cost `S_i`, and commit the **exponentially weighted average** of
the sampled first actions:

```
w_i = exp(-(S_i - min S) / λ)        u* = Σ w_i u_i / Σ w_i
```

then shift the horizon one macro step and repeat. The sampling distribution is
where a learned policy enters: instead of Gaussian noise around the previous
plan, the `M` sequences are **draws from the policy** `π(a | obs)` (with its own
temperature), so the planner searches where the policy already thinks the good
actions are and the policy is improved from what the planner found. This is the
standard "policy as proposal" arrangement (MPPI with a learned prior; the same
shape as AlphaZero's network-guided search, with rollouts instead of a value
tree), and the engine makes it cheap here because a rollout is a fork.

### 1.2 Why the tree is the right substrate

An MPPI iteration needs, from ONE state, `M` rollouts of `H` ticks. In this
engine a state is a paused process and a rollout is a child of it:

* the **node** (a `Node` in `forkoracle::tree`, made by `'B'`) is the paused
  engine at tick `t` holding the plan's prefix; it is exactly the ladder's rung
  (PERF.md §1), including the warm variant that carries the watchdog's state;
* a **rollout** is `Node::run` / `run_watched` from the node with the sampled
  tail — a `'R'`/`'W'` child, launched from the node's **standby** (PERF.md
  §3.2), simulating `H` ticks and answering with the `Summary`;
* **committing** the first macro step means making the next node: a `'B'`
  child of the current node at tick `t + k`, fed the committed action — the
  same operation the ladder performs to make a rung, 1–3 ms.

So the planner's memory IS the tree: the committed plan is a chain of nodes,
each one a savestate the next iteration's rollouts fork from, and backing up
(when every rollout from a node is bad) is forking from an earlier node in the
chain, which the ladder already knows how to do (`Ladder::prepare` picks the
deepest usable node by prefix equality).

### 1.3 Cost per iteration, from tonight's numbers

Per rollout from a warm node (PERF.md §3): fork → child alive 0.07 ms, child →
first tick 0.31–0.39 ms, then **24 µs/tick idle, 30 µs/tick with the box
loaded**, and 0.07 ms after the last tick. Per committed step: one `'B'` (a
node) at 1–3 ms including its probe.

| horizon `H` | ticks | one rollout | `M = 32` rollouts, serial per node | with 8 nodes' worth of parallel workers |
|---|---|---|---|---|
| 0.5 s | 50 | 1.7 ms | 54 ms | 7 ms |
| 1.0 s | 100 | 2.9 ms | 93 ms | 12 ms |
| 2.0 s | 200 | 5.3 ms | 170 ms | 21 ms |

A node serves one request at a time (one command pipe), so `M` rollouts from
ONE node are serial; parallelism comes from **replicating the node**: the
planner keeps `P` copies of the current node (P `'B'` children of its parent
with the same prefix — 1–3 ms each, once per step) and spreads the `M` rollouts
over them. At `P = 8`, `M = 32`, `H = 100` an iteration is ~12 ms of wall time,
i.e. **~80 planning steps a second per planner**, each committing `k = 10`
ticks: the planner drives at ~8× real time on one worker group. For
comparison the RL env's step today is 8–14 ms for `k = 10` ticks WITHOUT any
lookahead (RL.md §3.2).

The rollout count is the knob. MPPI's estimate quality goes as `M` and its
cost as `M·H`; with the policy as the proposal, `M = 16–32` is the usual range
because the samples are already concentrated.

### 1.4 The cost `S_i`, from the `Summary`

The watchdog's `Summary` (pred_core, 152 bytes, byte-identical between a
root's child and a warm node's child — PERF.md §1 proof) already carries what
the cost needs, computed inside the child per tick with no extra readout:

* `progress` (metres along the reference line, corridor-projected),
  `travelled`, `last_speed`, `max_speed`;
* `off_max` (the worst distance from the line), `lag_max_ms` (PERF.md §2 — how
  far behind the incumbent at the same line point, worst case);
* `trip_pred` / `trip_tick` if a predicate fired (crash, stuck, off-line), and
  the finish crossing (`cross_tick`, `cross_frac`) if the rollout finished.

A cost of the RL reward's shape (RL.md §2), per rollout:

```
S = -0.01·Δprogress(m) + 0.30·Δt(s) + 1.0·[tripped] − 10·[finished] + c_lag·lag_max_ms/1000
```

with `Δt = H` ticks × 10 ms for a rollout that did not finish, and the finish
time for one that did. `c_lag` is the term that keeps the planner honest
against the incumbent: the incumbent-lag predicate (§2 of PERF.md) measured
that **no candidate which beat the incumbent was ever more than 0 ms behind it
at the same line point**, so lag is a strong signal, cheap to include, and
already in the page.

Nothing new is read from the engine for this. If a term is wanted that the
`Summary` does not carry, it is a K_* predicate's job to compute it per tick
inside the child (pred_core's `feed`), which is where every existing term lives.

### 1.5 Interfaces

The planner is a driver-side program (Rust, in `tools/rl/tmenv` or a sibling
crate the ENV arm owns); the engine side needs no new command:

```
// what exists
Node::run_watched(from, recs) -> (json, summary_bytes)      // a rollout
ForkServer::branch(BranchReq { at, patches, watched, .. })  // a node
Ladder::prepare / Ladder::run_watched                       // node reuse by prefix
Summary::decode(&bytes)                                     // the cost's inputs
'S' sampling (per-tick car state through the ring)         // the policy's obs

// the planner's loop, per step
obs   = observe(node)                     // one 'S' rollout of k ticks, or the last rollout's samples
seqs  = π.sample(obs, M, temperature)     // M action sequences of H ticks
S_i   = cost(node.run_watched(seqs_i))    // over P replicas of the node, in parallel
u*    = mppi_average(seqs, S, λ)          // the committed first macro (k ticks)
node  = node.branch(u*)                   // the next savestate; the plan grows by k
data += (obs, u*, S)                      // for the policy's next fit
```

Two details that are decisions and are made here:

1. **The proposal is the policy's OWN distribution, not noise around it.**
   Sampling 32 sequences from a categorical policy over 20 actions per macro
   with temperature `τ` is the exploration; MPPI's `λ` then decides how greedy
   the average is. Both are tuned on the throughput curve, not chosen.
2. **What is committed is a tape, and the tape is what the oracle sees.** The
   planner writes the committed prefix as a Ghost.Gbx (the `Patcher` in
   `tmsearch::tape`) whenever the plan finishes or beats the incumbent, and
   offers it to `Bank::offer_many` — batched (PERF.md §7), refused if it
   finished after its own end. The planner's `S_i` never enters the bank.

### 1.6 Where the policy improvement comes from

MPPI produces, at every step, a **distribution over actions that is better than
the proposal** (the weighted samples). Fitting the policy to those weights —
the AWR / MPO family, `π ← argmax Σ w_i log π(u_i | obs)` — is the training
signal, and it needs no reward shaping beyond the cost above and no value
function. The trainer in `tmrl` already has the data path (per-step
observation and action records, 148-byte `CarState + Action` shards — the DATA
arm's note); a planning step adds `M` weighted actions per observation instead
of one, which is `M` times more signal per env step at `M` times the cost —
the trade the rollout table in §1.3 prices.

### 1.7 What must be proven before any of it is believed

In the project's order:

* **Exactness**: a rollout from a node equals the same tape run from the root
  (`fk ladder check` already does this for the ladder; the planner's nodes are
  the same nodes) — 1000/1000 on three maps, as PERF.md §1.
* **The cost is the file's**: a plan the planner scored `S` and committed,
  written out and run through the plain oracle, must have the finish time the
  `Summary` said (the guard does this; the planner's own control is to run the
  committed tape as ONE child and compare `progress`/`cross_tick` with the
  rollouts' — the same as RL.md's CONTROL B).
* **A negative control on the proposal**: with the policy replaced by the
  uniform distribution, the planner must be measurably worse at the same `M`
  and `H` (if it is not, the policy is not proposing anything).

---

## 2. A learned progress critic as an abort rule

### 2.1 What the watchdog can and cannot do today

`pred_core` judges a running candidate every tick, inside the child, on
quantities of the trajectory itself: speed drop (crash), speed floor (stuck),
distance from the line (off), and since tonight **lag behind the incumbent at
the same line point** (`lag:ms=X`, PERF.md §2). Each rule is a threshold with a
`need` (consecutive ticks) and an `after` (grace period), fixed for the run.

The measured picture (PERF.md §2, 2000 candidates at tick 171): candidates
that beat the incumbent had `lag_max_ms = 0`; those within 50 ms of it, at most
60 ms; the abort rules together stop 64–66 % of candidates early with **zero
false positives** in the corpus, and the remaining ~34 % run to the end. The
question a critic answers is: **of the candidates that run to the end and do
not improve, how many could have been stopped, how early, at what false
positive rate.**

### 2.2 The critic is a per-tick lag threshold, learned

The right first critic is not a network. It is the lag predicate with a
**threshold that depends on where the candidate is**: `X(t)` instead of `X`.
Early in the run a candidate 30 ms behind may still win (it can make it up over
2000 ticks); at tick 2000 of 2400, 30 ms behind is lost. The constant
`lag:ms=100` was chosen as the value that is safe everywhere, i.e. it is the
loosest point of the curve `X(t)` applied everywhere.

Learn `X(t)` from the search's own corpus: for every candidate that was
confirmed better than the incumbent it replaced (the bank's `best_*` files and
the log), record its lag trace `lag(t)` along the incumbent's line; `X(t)` is
the `(1−ε)` quantile over those traces at tick `t` plus a margin, with `ε` the
false-positive rate the search is willing to pay (PERF.md §2 measured the whole
corpus at 0; `ε = 10⁻³` is a reasonable first cap). Then the rule is:

```
abort at the first t ≥ after such that lag(t) > X(t) for `need` consecutive ticks
```

This is a critic in the honest sense — a learned function of the state that
predicts "this run will not improve" — with three properties a network would
have to earn: it is monotone in the one quantity that matters, its false
positive rate is a measured number with a control, and it costs one table
lookup per tick in the child.

### 2.3 How it ships

* **Engine side**: the shim already carries line-indexed arrays for the
  corridor (`plane_x`, corridor points); `X(t)` is one more `f32` per line
  point, published with the `'A'` arm message like the others, and `K_LAG`
  reads `X[refidx]` instead of `p[0]` when the table is present. ~30 lines in
  `pred_core::feed`, no protocol change beyond the arm payload's length.
* **Learning side** (`tmsearch`): a `--lagtable` fitted from `--log` JSON lines
  of a previous run on the same map — needs the per-tick lag trace of confirmed
  improvements, which today is not recorded (only `lag_max_ms`); recording it
  is a 'W' with the sample ring on (PERF.md §5) for the ~10 confirmed
  candidates of a run, at 3 µs a sample. Cheap; do it at confirmation time in
  the guard.
* **Control**: `fk watch measure` already reports, per lag predicate, "aborted /
  would have finished / FASTER than the incumbent" on ≥ 2000 candidates. That
  line with the table armed is the acceptance test, and its third number must
  stay 0.

### 2.4 The network version, and when it is worth it

Once `X(t)` is in, a critic `V(obs) → P(improves)` on the 80-float observation
can only add what lag does not carry: the state's *potential* (speed, heading,
position in the corridor relative to the line) rather than its lateness. It
would run in the child, per tick, on a fixed-size MLP — 80→64→1 is ~5 kFLOP,
~2 µs on one core, within the 24 µs tick — reading the observation the sampler
already builds (§1.5) and the weights from a shared page armed with `'A'`. The
abort rule stays a threshold on the output with `need` and `after`, and the
acceptance test stays `fk watch measure`'s third number.

Do it only if the table leaves something on the floor: measure, after §2.3,
how many of the run-to-the-end non-improvers the table still lets through and
how many ticks they cost. If that is under 10 % of evaluation time, the
network is not worth its weights.

---

## 3. What the LEARN arm can take from tonight without waiting

* Rollouts from warm nodes are byte-identical to root rollouts (PERF.md §1):
  any planner may fork from the tree and trust the `Summary`.
* A rollout's fixed cost is 0.5 ms and the tick is 24–30 µs (PERF.md §3); the
  per-tick observation costs 3 µs through the ring (PERF.md §5). Size `M` and
  `H` from these, not from the RL env's old 8–14 ms per step.
* Every result goes through `Bank::offer_many`, batched, and a finish after
  the tape's own end is refused there and marked `AFTER-TAPE` by
  `tmauto verdict` (PERF.md §7).
* Three quarters of the cores is where the box's throughput knee is for
  fork-per-candidate workloads on this build (PERF.md §6); pinning cost 4.5 %.
