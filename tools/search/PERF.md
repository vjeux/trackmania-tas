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

### Windows at 25 / 50 / 75 / 90 % of the tape (2-minute searches, 24 workers, seed 42, baseline and ladder concurrent)

| window (ticks) | baseline evals | ladder evals | ratio |
|---|---|---|---|
| 736–796 (25 %) | 63 060 | 88 500 | 1.40× |
| 1301–1361 (50 %) | 54 930 | 132 180 | 2.41× |
| 1867–1927 (75 %) | 46 350 | 117 090 | 2.53× |
| 2206–2266 (90 %) | 45 210 | 198 270 | **4.39×** |

Every candidate in these runs forked from a node (100 %); 24–33 nodes made per
run. The baseline gets SLOWER as the window moves later because a late edit
drives the whole track and finishes, while an early edit crashes the car and
makes the rest of its ticks cheap — the same effect as in the per-depth table.

---

## 2. The incumbent-lag predicate — `lag:ms=X`

### What

A candidate that reaches a point of the incumbent's line X ms after the
incumbent did is not going to beat it. That is the checkpoint split, judged at
every tick instead of at the checkpoints: the reference line the watchdog
already tracks is indexed by the incumbent's own tape tick, so the index of the
nearest point IS the tick the incumbent was here, and `10 × (tick − index)` ms
is how far behind the candidate is running. `pred_core::K_LAG`, one arm in the
same `feed` the other five predicates live in; judged only inside the corridor
(off the line the index means nothing, and `offref` is watching there);
`need` defaults to 10 consecutive ticks so one tick of tracking jitter is not
a verdict. `Summary::lag_max_ms` records the maximum lag of EVERY run, armed or
not, which is how the threshold is measured rather than chosen.

Score safety is the same argument as for every other predicate: aborting only
removes ticks, progress is a maximum over ticks, so `progress(aborted) ≤
progress(unarmed)` and a dead candidate cannot displace a live one. Measured
below, 2000/2000 at both checkpoints.

### Proof (`fk watch measure`, map 2 rank00001, 2000 candidates of the search's own operators over the whole editable tape, seed 7)

The unarmed distribution of `lag_max_ms` by outcome is the control that sets X:

| outcome (unarmed) | n | median | p90 | max lag |
|---|---|---|---|---|
| faster than the incumbent | 19 (tick 171) / 49 (tick 1200) | 0 ms | 0 ms | **0 ms** |
| same ms or up to +50 | 458 / 643 | 0 | 30–40 | 60 ms |
| +50 .. +200 ms | 70 / 131 | 80–90 | 160–170 | 200 ms |
| +200 ms or worse | 55 / 87 | 480–550 | 1400–1570 | 2020 ms |
| did not finish | 1398 / 1090 | 5490–7000 | 7760–20510 | 23470 ms |

No candidate that beat the incumbent was ever behind it; nothing within +50 ms
was ever more than 60 ms behind. So X = 100 ms has a 40 ms margin above the
whole "+50 or better" class, X = 200 ms a 140 ms margin. Armed:

| checkpoint | set | aborted | `lag` fired | faster-than-incumbent aborted | armed ms/cand | speedup vs observing | aborted after (% of tail) |
|---|---|---|---|---|---|---|---|
| tick 171 | crash+stuck+off | 64.6 % | — | — | 41.75 | 1.328× | 49 % |
| tick 171 | + `lag:ms=200` | 65.7 % | 41.1 % | **0** | 39.86 | 1.394× | 44 % |
| tick 171 | + `lag:ms=100` | 66.0 % | 48.4 % | **0** | 39.14 | 1.418× | 43 % |
| tick 1200 | crash+stuck+off | 41.7 % | — | — | 29.45 | 1.142× | 57 % |
| tick 1200 | + `lag:ms=200` | 43.6 % | 26.2 % | **0** | 28.29 | 1.196× | 48 % |

Exactness 0 differ armed vs unarmed on the non-tripping candidates, 0 disagree
with the full validation, 0 perturbed by watching, score safety 2000/2000 in
every row. Honest reading of the gain: the predicate mostly fires EARLIER on
candidates `crash`/`off` would have caught later — it moves the average abort
from 49 % to 43 % of the tail and takes 4–7 % off the armed cost per
candidate. It is a small lever with a measured zero false-positive rate, not a
large one.

### What to run

`--pred behind:lag:ms=100,need=10,after=200` beside the shipped three, for an
improvement-only search. With Metropolis annealing keep X at least twice the
temperature in ms (a +100 ms candidate is accepted with probability e^(−100/T)),
or the predicate kills what the temperature would have taken.

### What was deleted

Nothing was replaced; the predicate set gained a kind. `SUMMARY_BYTES` grew from
148 to 152 for `lag_max_ms` (shim and driver share the file).

---

## 3. The fixed cost of a candidate: the exit marker, the standby child, the census

Measured first (`fk tickhook cost`, map 2 rank00001, `tick:2313`, 119 ticks of
tail, 40 runs per phase, this box), then cut what the measurement named.

### 3.1 The parent was waiting for the kernel to tear the child down

A child that has its answer leaves with `_exit`, and the parent learned that
from EOF on the result pipe. EOF arrives when the kernel closes the child's
files — and `do_exit` runs `exit_mm()` FIRST: every COW'd page and every page
table of a ~150 MB process, freed while the parent sits in `poll`. With the
answer already in the shared page.

| phase (tick 2313, finisher, exit-at-finish armed) | before | after |
|---|---|---|
| last tick → parent has the answer | **2.34 ms** | **0.07 ms** |
| a candidate, end to end | 7.65 ms | 5.08 ms |

The fix is eight bytes: the child writes `FKEXIT` down its result pipe right
before `_exit` (`leave(1)`), the parent treats it exactly like `"IsValid"`, and
the teardown happens on whatever core the kernel puts it on while the next
candidate is already running. Every store into the shared pages precedes the
`write` syscall, so a parent that has seen the marker sees them. Used by the
finish exit, the watchdog's trip exit and the tape-exhausted exit.

### 3.2 The fork itself, off the critical path

`fork()` of the paused engine costs 1.0–1.9 ms (the kernel copying ~150 MB of
page tables) and the parent paid it between receiving a candidate and starting
it. Now every fork point — the root and every ladder node — keeps ONE child
forked ahead of time, blocked on a pipe. A candidate is handed to it (the
payload down the pipe, ~40 KB) and the next standby is forked right after the
hand-off, while the candidate simulates, on a core the parent was not using.

| phase | before | after |
|---|---|---|
| fork → child alive | 1.04–1.50 ms | **0.07 ms** |
| child → its first tick | 0.32 ms | 0.31 ms |
| a candidate, end to end (tick 2313) | 5.08 ms | **4.00 ms** |
| fixed cost as a share of that candidate | 54 % (7.65 ms build) | **11 %** |

What keeps it correct: a standby is a copy of the parent's inheritable state at
the moment it was forked, so **every command other than `'R'`/`'W'` kills the
standby first** — arming the watchdog, the chain, the finish word, a branch, a
probe — and the next candidate forks synchronously and pre-forks a fresh one
behind itself. A standby whose parent dies reads EOF on its command pipe and
exits (no orphan; 0 `TrackmaniaServer` processes after every run above). A
standby that died is detected at the hand-off and that candidate forks the old
way. `'R'` and `'W'` now share one launch path and one parent loop.

Across the evening, a late finisher went **13.22 ms (start of the tickhook
work) → 7.65 (exit-at-finish) → 5.08 (exit marker) → 4.00 ms (standby)**, of
which 3.54 ms is the 119 ticks of physics.

### 3.3 The census, and why huge pages and `MADV_DONTFORK` are not tried

`FKSHIM_CENSUS=1` makes the parent read the child's `/proc/<pid>/stat` and
`smaps_rollup` before killing it and append `minflt rss_kb pdirty_kb` to
`FKTIME`; `fk ladder check` reports the means. Map 2, a full run from tick 171
(2261 ticks): **a candidate child faults 2668 pages and dirties 11.8 MB** of a
144.7 MB resident set (root and ladder children alike: 2655 faults, 11.7 MB).

* COW is ~2.7 ms per full-length candidate (2668 faults at ~1 µs), about 4 %
  of the 66 ms of ticks, ~1.2 µs of the 29.4 µs/tick. Not a lever.
* Transparent huge pages (this box: `madvise` mode, so `MADV_HUGEPAGE` +
  `MADV_COLLAPSE` would be allowed) would cut the page-table copy — which the
  standby has already taken off the critical path — and turn every one of
  those ≥ 3000 dirtied 4 KB pages into a 2 MB copy-on-write. 11.8 MB dirtied
  in small pages spread over the heap is tens to hundreds of 2 MB regions;
  the arithmetic says slower, so it is not tried.
* `MADV_DONTFORK` on never-touched regions would save page-table copying
  only, which is no longer on the path, at the price of a SIGSEGV the first
  time a candidate touches a region the census never saw. Not tried.

### Proof

* `fk tickhook cost` as above; `fk tickhook reads` PASS after every change.
* `fk ladder watched`, 300 candidates: 300/300 identical time and summary
  after the marker, 300/300 after the standby, 300/300 after the rebase onto
  tickhook @ 485319b.
* `fk server check` 50/50 (map 2 tick 1200) after each change; `fk ladder
  check` 200/200 on "get jiggy with it" after the standby.
* The whole suites: 157 search tests, 38 fk tests, 0 failures.
* Search, 5 minutes, 24 workers, seed 42, ladder-only build vs ladder + marker
  + standby, run concurrently: 328,560 evals / 10 confirmed / 0 phantoms vs
  **398,910 evals / 10 confirmed / 0 phantoms** (+21 %).

### What was deleted

The two copies of the candidate launch (the `'R'` child, the `'W'` child) and
the two parent loops: one `launch`, one `apply_candidate`, one loop. There is no
flag that disables the marker or the standby.

---

## 4. Frame overhead: measured, not material

The tick loop lives inside the validator's frame function, which has a
wall-clock budget exit and per-frame bookkeeping (TICKHOOK.md §1: the `lroundf`
drift was the frame partition). Is any of the 24–30 µs a tick something other
than physics?

Measured from inside the child (`FKSHIM_CENSUS=1`): the interval between
consecutive entries of the tick function, counting every gap over 80 µs (a
tick is ~24 µs idle, ~30 µs loaded) and summing what those gaps exceed 80 µs
by — a frame boundary inside the loop shows up there, and so does a
preemption. Map 2, 2261-tick candidates from tick 171:

| box | gaps > 80 µs per run | total excess per run | largest gap | share of the run |
|---|---|---|---|---|
| idle | 6.0 | 0.30 ms | 0.30 ms | 0.5 % of 54 ms |
| 48 search workers running | 11.5 | 0.35 ms | 0.27 ms | 0.5 % of 66 ms |

So at most six frame boundaries (or preemptions) per full-length candidate on
an idle box, costing 0.3 ms in total. The per-tick cost IS the physics; driving
the loop back-to-back or neutralising the budget exit in the child would buy
under 1 %, and nothing was changed. (The whole fixed cost outside the ticks is
now 0.5 ms of a 55 ms early candidate — `fk tickhook cost`, tick 171: 1 %.)

---

## 5. Per-tick sampling through a shared page — the sample ring

### What

An `'S'` child (the RL env's per-tick car state, `fk trace`, every locate scan)
used to `write` each sample down a pipe: a syscall and a wake-up of the polling
parent per tick. Now the parent maps a fresh `MAP_SHARED` buffer —
`[u64 used][samples…]` — before every `'S'` fork, the child copies each sample
into it, and the parent reads it once after the child has said it is done. The
blob is byte-for-byte what the pipe carried, in the same order; when the ring
(capped at 64 MB) fills the child goes on down the pipe and the blob is
`ring ++ pipe`, so nothing is lost. The wire protocol and the drivers are
unchanged — `tmenv`'s `Forest`, `fk trace`, `locate` all get it for free.

**A fresh mapping per run, never a reused one.** A child that has been told to
die can still be on another core for a few microseconds, and a reused ring
whose header the parent has just reset is exactly where its last sample would
land: on top of the next run's first. Each child keeps its own pages until it
is gone; the parent unmaps its reference after reading (~20 µs a run).

The child also marks its exit (§3.1) when it leaves at its sample budget or
deadline, and the parent stops waiting for the sample pipe's EOF once the child
has said it is done — it drains what the pipe already holds and moves on.

### Proof

* `forkshim/tests/ladder.rs::sampled_records_arrive_intact_and_in_order_through_the_ring`
  (shimhost, every `cargo test`): 500 samples of a known input record — count,
  consecutive clocks, every field of every record against the tape, and a
  second run on the same server sees none of the first's samples.
* `fk trace` CSVs (2260 rows, 29 columns) byte-identical to the previous build:
  map 2, three runs of each build, one md5; "get jiggy with it", identical.
  On Kacky Reloaded #290 the first 1694 rows are identical and the trace itself
  is intermittent in BOTH builds (the finish-word calibration succeeds on some
  runs and the armed child then leaves at the finish; on others the locate's
  self-check fails) — a `fk trace`/locate matter, reported to the tickhook arm,
  not a sampling one.

### Cost

Map 2, the trajectory run of `fk trace` (a 3159-tick sampled child, 2260 rows
after dedup), three runs each: **64.6 ms → 57.4 ms** (62.7/66.4/64.7 vs
58.1/56.0/58.1), i.e. 3.2 µs per sample. For a k-tick RL macro that is
3.2 µs × k; for the route project's 200–400-tick rollouts with a per-tick read,
0.6–1.3 ms of the 23 ms measured per rollout.

### What was deleted

The per-sample `write` is gone from the common path; the pipe remains only as
the overflow beyond 64 MB of samples, which no caller reaches.

---

## 6. Box saturation: where the knee is, and that pinning does not help

`tmsearch search` on map 2 from tick 171, the shipped three predicates, seed
42, 2 minutes per point, each point alone on this 96-core box, on the build
with §1–§5 in it:

| workers | evals | evals/s | per worker |
|---|---|---|---|
| 16 | 102,540 | 850 | 53.1 |
| 32 | 204,480 | 1,695 | 53.0 |
| 48 | 312,120 | 2,578 | 53.7 |
| 64 | 412,560 | 3,419 | 53.4 |
| 80 | 464,850 | 3,839 | 48.0 |
| 88 | 470,520 | 3,898 | 44.3 |

Linear to two thirds of the cores, +12 % for the next sixth, +1.5 % for the
sixth after that. A worker is one core simulating plus the standby's fork, the
dying child's teardown and the driver thread's parse beside it — about 1.3
cores' worth at full tilt — which puts the knee where it is measured.

**Pinning** (`sched_setaffinity`, one core per worker's server and so for every
child it forks, round-robin over the box) at 80 workers: **444,120 evals vs
465,000 unpinned** in the same two minutes, −4.5 %. The kernel places the fork
and the teardown better than a mask does; the experiment's code was deleted.

**Written into the launcher:** `tmsearch --workers` now defaults to **three
quarters of the cores** (was: all of them). For the RL env's launcher the same
number is the recommendation until the ENV arm measures its own curve with
this branch merged; on the route project's 166-core box the fan-out saturated
at 32–96 workers on the old fork path, which this branch changes (§3, §5), so
that curve is worth re-taking too.

---

## 7. Batched certification

### What

The plain oracle's cost is the launch — 2.3 s to boot the engine — and not the
file (tens of ms), and the search sent every claim to it alone. `Bank::offer`
is now the one-claim case of `Bank::offer_many`, which writes N tapes and asks
the oracle about all of them in ONE launch, matching each answer to its file by
the unique name the file was given. The search's main loop keeps a
**certification queue**: a claim waits up to `CERT_LATENCY` (2 s) for company,
or until `CERT_BATCH` (15, the server's own internal batch) are waiting, and
the batch goes to the bank together; the incumbent moves to the best confirmed.
Why it matters: a search leaving a weak seed produces claims faster than one
launch at a time can certify them, and the global incumbent then lags by
minutes while the workers migrate toward a stale best.

**A finish after the tape's own last record is refused** (SEARCH.md §3, §1 of
this file): `offer_many` compares the oracle's millisecond with
`start_offset + 10 × ticks` and keeps such a claim as `PHANTOM_pastend_*`, with
the oracle's time in the log, rather than bank a number that depends on the
batch it was measured in. `tmauto verdict` marks the same class `AFTER-TAPE`
(with the tape's end read from the file itself) and says so in a footnote.

### Proof

* `tmsearch/tests/oracle_e2e.rs::offer_many_gives_every_claim_the_verdict_offer_gives_it_alone`:
  N mutated tapes (one to three local edits in the last third: finishes, DNFs
  and past-the-end finishes all occur), claims = the oracle's own time (or a
  lie of 22.000 on every third tape, so phantoms are in the same batch, or the
  DNF), certified one at a time and in batches of 15; every verdict — confirmed
  time, or phantom with the same oracle answer and the same kind — must match.
  N = 30 on every `cargo test`; the proof run below used `TM_CERT_N=500`.
* `the_guard_refuses_a_finish_after_the_tapes_own_end`: the template braked
  over its last 0.3 s finishes after its own end, the oracle prints a time, the
  guard refuses it as `PHANTOM_pastend_*`.
* `a_mutated_candidate_is_banked_only_under_the_time_it_actually_does` now
  states the rule: a mutant inside its tape banks under the oracle's time, one
  past its end is refused.
