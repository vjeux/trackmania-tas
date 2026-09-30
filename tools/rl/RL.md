# RL.md — the reinforcement-learning system, and what is proven about it

Agent G, 2026-08-24. **`tools/rl/`**, two crates: `tmenv` (the environment)
and `tmrl` (PPO over it).

This file is the state of record. A fresh box plus this repo plus
`SETUP.md` is a complete recovery.

---

## 0. What this is

The system vjeux asked for: an RL agent of the kind YoshTM's videos and
**Linesight** (`pb4git/linesight`) use, running on our instrument instead of
theirs. Not a search. A policy network, trained by gradient descent on
rollouts, driving the car.

What we have that they did not, and what it changes:

| they have | we have | what it buys |
|---|---|---|
| a screenshot, 160×120 grey, through a 4-layer CNN — 5,632 of their 5,888 features and most of their 6.5 M parameters | the car's **own physics state**, read out of engine memory per tick | the observation is 80 floats; the network is small enough that a GPU would buy nothing |
| ~124 env-steps/s across two game instances (3.076× wall clock in their shipped log) | **9,600 env-steps/s** measured across two boxes (≈960× wall clock) | on-policy PPO instead of IQN + replay + target net + prioritised sampling |
| rewind to t = 0 only | a savestate **tree** — branch from any mid-run state | mid-episode resets to arbitrary states (built, not yet used by the trainer) |
| a human replay resampled every 0.5 m as the progress line | the map's own geometry, via `mapgeom` | **the no-ghost rule holds with nothing given up** |

---

## 1. Run it

```bash
export TM_SERVER=/tmp/tmoracle/server
export FK_SHIM=/tmp/tmtas/tools/search/target/release/libforkshim.so
cd tools/rl && cargo build --release
M=/tmp/tmwork/maps/buNzfsVlp2NF2oWtHM3729dEylg.Map.Gbx   # Summer 2026 - 01

./target/release/tmenv setup --map $M --ticks 4000 --declared 40000 --out /tmp/tmenv
./target/release/tmenv known-answer --map $M --ref /tmp/tmenv/reference.Ghost.Gbx  # THE PROOF
./target/release/tmenv bench --map $M --ref … --workers 96                          # throughput
./target/release/tmrl  selftest                                                     # net + PPO controls
./target/release/tmrl  train --map $M --ref … --workers 56 --iters 4000
```

Other `tmenv` subcommands: `track`, `spawn`, `trace`, `checktraj`, `locate`,
`verdict`, `seek`, `calibrate`, `rollout`.

---

## 2. The environment, and the two controls that license it

`reset` / `step(action) -> (obs, reward, done, info)` over `branch::Forest` —
each step forks a paused engine, runs `k` ticks, and reads the car's state per
tick out of its memory.

**`tmenv known-answer` is the proof, and it has both axes and both negative
halves.** Run it before believing anything trained on this env.

| | what it certifies | result |
|---|---|---|
| **CONTROL A** | the env's reading of the run is true. The **plain oracle** validates the written tape: a separate dedicated server, **no shim, no fork, no memory readout, no `branch`** — it cannot agree with the env by sharing a bug | PASS |
| **CONTROL B** | the env's *trajectory* is that tape's trajectory. Same tape re-simulated in ONE child, no per-step forking. Shares the readout, so it certifies the **stepping** | PASS — **max \|Δp\| 0.000000 m** over 360 ticks |
| negative ×2 | change one macro: the runs must **differ** (they do — 12.3 m) AND the perturbed run must still equal **its own** re-simulation (0.000000 m) | PASS |

Either control alone is decoration. A comparison that always says "same"
passes B on a broken rig; one verdict on one tape says nothing about
discrimination.

### Observation — 80 floats

Velocity in the car frame + speed (4), map-up in the car frame (3), angular
velocity differenced from the quaternions (3), tyre wetness (1), signed lateral
offset / height / corridor half-width / on-route (4), progress fraction / cap /
distance owed / time fraction (4), gates collected one-hot (5), **the track
ahead — 9 route points at 5–240 m in the car frame with their half-widths
(36)**, previous action one-hot (20).

The lookahead block is the single most load-bearing thing in the corpus:
Neinders' ablation moved 44.413 → 38.402 by adding curvature lookahead alone.

**Not yet in the observation, and it is a task rather than an absence**
(RULES §4): per-wheel ground contact, `is_sliding`, damper travel, contact
material, gear, RPM, the gearbox transient counter. Linesight has all of it and
`fk`'s readout does not expose it yet. The engine computes it; go widen the
readout.

### Action space — 20

Five analog steer rungs `{-127, -48, 0, 48, 127}` × four throttle states
`{gas, coast, brake, gas+brake}`. Linesight's keyboard-legal twelve is a strict
subset (tested). They have no analog steer because TMNF keyboard input is
three-valued — a property of their input path, not a finding.

### Reward

```
r = 0.01 · Δ(new maximum progress, m) − 0.30 · Δt(s)   [+10 finish] [−1 crash]
```

Break-even is **30 m/s**. Linesight's is 120 m/s = 432 km/h, so *every* step of
theirs is negative and an episode-return objective would reward
self-termination; they buy that back with a fixed 7.000 s horizon that makes Q
an undiscounted sum over a window. We do not need that machinery — putting
break-even below any speed a moving car sustains removes the incentive to die,
and the episode return is then `c_prog·L − c_time·T` plus the bonus, which is
minimised-T by construction.

Progress is the **new maximum**, so driving back and forth over a stretch is
not paid twice.

---

## 3. What is MEASURED, and it is not small

### 3.1 The engine does not start the car where the map says

**The validated car begins at (1360.00, 10.00, 1108.73)**, at rest, facing −z —
**389 m from the map's own `RoadTechStart` block at (1584, 16, 784)** on
Summer 2026 - 01.

Four independent legs, two-sided:

* the trajectory readout says so, and passes its own self-check
  (|d(pos)/dt − v| median 0.020 m/s, |q|−1 p99.5 1.06e-7);
* a sweep of **all 2308 mapped memory windows** finds **no** moving,
  velocity-consistent car anywhere near the map's start block, and finds one
  here — the negative half and the positive half of the same instrument;
* the **plain oracle** corroborates from a separate path: a straight run
  covering **848 m of path collects zero checkpoints**, impossible from the
  map's start, which is 511 m short of CP1;
* the map file really does contain a `RoadTechStart` at (1584, 16, 784)
  (`mapgeom where`), so this is not `mapgeom` misreading the file.

Invariant to the container's declared time and declared checkpoint list
(tested at 3000 / 8000 / 30000 / 40000 ms and with 0, 3 and 4 declared CPs).

**WHY the two differ is UNKNOWN and is a task, not a conclusion.** Untested
candidates: the dedicated server's own map-loading path, waypoint `Order`, the
container's `race_settings` / `settings_flags`.

**What it cost, and the fix.** A route solved from the wrong origin has the
wrong arc length, the wrong leg order and the wrong first gate — silently,
because the geometry is self-consistent either way and only the reward is
nonsense. `mapgeom::packrun::Opts::spawn_override` lets the tour be solved from
a measured origin, and `tmenv::load_track_measured` measures it per map and
caches it.

| route from | length | implied m/s over the author time | spawn lands at |
|---|---:|---:|---|
| the map's `Spawn` waypoint | 1900.3 m | 82.1 | s = 1479.8 m, lateral 1.93 m |
| **the measured spawn** | **1639.2 m** | **70.8** | **s = 0.3 m, lateral −1.00 m** |

### 3.2 Throughput — milestone 2

| box | workers | env-steps/s | per worker | per step |
|---|---:|---:|---:|---:|
| 176-core | 32 | 3,903 | 122.0 | 8.20 ms |
| 176-core | **96** | **7,035** | 73.3 | 13.65 ms |
| 176-core | 160 | 4,936 | 30.8 | 32.4 ms — oversubscribed |
| 80-core | **64** | **2,577** | 40.3 | 24.8 ms |

**Fleet peak ≈ 9,600 env-steps/s = 96,000 game-ticks/s ≈ 960× wall clock.**

Steady state excludes per-worker setup (server launch + car locate, 5–11 s,
paid once per worker *lifetime*). D's single-node figure was 6.953 ms for a
k = 10 branch on an idle box; we hold 8.2 ms at 32-way and degrade past ~55 %
core occupancy, because each branch runs a child process alongside its parent.

**No GPU is needed and here is the arithmetic.** The policy is 80→256→256→20:
~180 kFLOP per forward, so 9,600 steps/s is **1.7 GFLOP/s** of inference and
~20 GFLOP/s including PPO's backward passes. The env costs ~10 ms per step and
the network costs microseconds. **The network would have to grow ~1000× before
it rivalled the env.** Re-measure before asking for one; a recurrent core or a
much wider lookahead encoder would change the answer.

---

## 4. The trainer

PPO. **The reason is falsifiable, not a preference:** Linesight's IQN — replay
buffer, target network with soft update, prioritised sampling, n-step returns,
two stacked exploration schedules — is the right family when samples are
scarce, and at ~124 env-steps/s theirs are. At 9,600 it inverts: on-policy PPO
throws data away and does not care, has no replay to tune and no target net to
destabilise, and one exploration knob instead of three. **If throughput ever
collapses, the argument goes with it and IQN becomes the better choice again.**

`candle`, one line of justification: pure-Rust build (no 2 GB libtorch fetch
through the forward proxy — it builds in 41 s), PyTorch-shaped autograd and
AdamW, and a CUDA backend already there if a GPU ever becomes worth it.

### The network exists twice, and the two are checked against each other

Training wants autograd; rollout wants batch-of-one in a hundred threads inside
a loop whose other statement costs ten milliseconds. So `Trainable` is candle
and `Weights` is flat `Vec<f32>` with a hand-written forward that every worker
holds privately — no locks, no shared tensors, no allocation per step.

**Two implementations of one function is how this project got silent corruption
before.** `Weights::agrees_with` runs both on random inputs and requires them
equal (**measured: worst 1.67e-6 over 64 inputs**), and `tmrl selftest` runs
the negative half — a deliberately perturbed copy must be **REFUSED**, because
a check that cannot fail certifies nothing.

### PPO's three known-answer tests (`cargo test --release -p tmrl`)

The sharp one: **a TRUNCATED episode bootstraps its value and a TERMINATED one
does not.** Conflating them teaches the policy that running out of tape is
death. The test requires the two to give *different* advantages for the same
transition.

### Every new best is re-simulated

A rollout's reward is the environment's reading. **A result is a tape the plain
oracle re-simulates.** The trainer writes the tape at every new best and prints
the oracle's verdict beside its own, never instead of it.

---

## 5. What is NOT settled — read this before trusting the reward

### 5.1 The gate detector does not agree with the oracle

`tmenv calibrate` drives 40 varied tapes and compares the geometric detector
with the server's own checkpoint count. **The oracle credits a checkpoint on 20
of 40; no configuration reproduces that split**:

* proximity, radius 6–26 m: fires on all 40 (17 over-counts)
* radius 4 m: misses 21
* an arc-length window: **actively wrong** — on a route that reuses a road the
  car passes a gate at a completely different arc length than the tour assigns
  it, so the window rejected every real crossing and no radius could
  discriminate
* plane crossing, forward / reverse / either: fires on 1–3 of 40

**Which geometric event the validator credits is UNKNOWN.** It is a task.
Leads: the credited crossing may happen in the fixed prefix before the fork's
earliest checkpoint (race ≈ 0.95 s), which the env never sees; and two tapes
with byte-identical prefixes have been observed to get different counts, which
that explanation does not cover.

**Therefore `CoreCfg::gate_cap` is OFF by default**, and the reason is written
into the field. A cap driven by a detector that under-reports pins progress at
the first gate's arc length forever, and the policy can then never be paid past
the first sixth of the map — silently, with a reward that looks fine. That is a
worse failure than the one the cap prevents.

**What replaces it: the corridor.** Off-route termination fires when the car
leaves the road by more than its half-width plus a margin, and on this map the
road physically passes through every gate, so a car that stays on it cannot
skip one. **This substitution is map-specific and is stated rather than
assumed** — on a map with a genuine shortcut it does not hold, and the cap must
come back with a detector the oracle agrees with.

Underneath it all the standing rule is untouched: a shaping term that flatters
a shortcut costs signal quality and **can never make a false result true**,
because the oracle adjudicates every banked tape.

### 5.2 The first 0.95 s is not the policy's

The earliest the fork server can be stopped is ~tick 95. Before that the car
runs the reference tape (full throttle, jittered steer, straight). That is 4 %
of a 23.144 s map and it is **fixed** — the policy cannot change it, and any
tape we bank contains it. Lowering it means stopping the engine earlier than
`lroundf` 14000, which has not been made to work reliably (the branch child
returns one row or none at the very earliest stops; `measure_spawn` walks a
ladder and reports which rung answered).

### 5.3 Other gaps

* Per-wheel state is not in the observation (§2).
* The savestate **tree** is built and proven but the trainer only ever resets to
  the root. Mid-episode resets from an archive of visited states — a backward
  curriculum — is the obvious next use of the one capability nobody in the
  reference corpus has.
* `mapgeom`'s route from the measured spawn climbs to y = 15.5 m at s = 100 m
  where the car is at y = 10 m; possibly an overpass the grid took. Not chased.
* `tmauto`'s own `synth write --declared` sets `declared_ms` directly instead of
  calling `set_declared`, so it writes a container the server REFUSES
  (`wrong simu unexcepted walltime (0s)`) rather than a DNF. `tmenv setup` does
  it correctly. **The CLI has the bug; fix it there.**

---

## 6. Traps paid for here

1. **`GhostMeta::set_declared`, never `meta.declared_ms = …`.** The declared
   time and the walltime pair are one fact and the server checks them against
   each other. Move one alone and you get `wrong simu unexcepted walltime (0s)`,
   which is a **REFUSAL** (`simulated() == false`, verdict `None`) and not a
   DNF. They look identical if you collapse them and they mean opposite things.
2. **A `Layout`'s address is only valid inside the process that was swept.** The
   server is position-independent and its base moves every start. What
   transfers is the offset from that process's own base — which is what
   `FK_STATE_OFF` is. Reusing the raw address reads out a plausible trajectory
   of nothing.
3. **`debug_assert_eq!` is compiled out of a release build.** The observation
   width formula said 77 and the code produced 80; the first thing that noticed
   was a slice-index panic in a rollout worker after the fleet had spent 45 s
   coming up. Two statements of one fact with nothing forcing them to agree.
4. **Nearest point on the whole polyline is not progress.** On a route that
   reuses a road it jumps between legs. Measured: the arc length read the LAST
   leg while the car was on the first straight, saturating the progress cap
   inside a second.
5. **Snapping to route vertices makes arc length a staircase.** The route is
   Douglas-Peucker simplified. Before segment projection: s = 0, 0, 0, 44, 44,
   44, 78 down a straight, lateral 13.3 m on a car 1 m off centre, and the
   off-route cut fired on it.
6. **The oracle renames files on a name collision** (`seek.Ghost.Gbx` →
   `1_seek.Ghost.Gbx`). Match answers by ORDER, and assert the count.

---

## 7. Milestones

| | | |
|---|---|---|
| 1 | the environment, proven by known-answer test | **DONE** — §2 |
| 2 | measured rollout throughput at parallelism | **DONE** — 9,600 env-steps/s, §3.2 |
| 3 | a trained policy reaching CP1, confirmed by the plain oracle | in progress |
| 4 | the finish | |
| 5 | under 23.144 | |

**Calibrate expectations to the corpus.** Linesight took **52 numbered
experiments** to ship one configuration on one map; their curve is 300 k steps
to learn to press forward, 500 k to finish once, 1 M to finish regularly, 3–5 M
for a good time. PedroAI: 169 days, 100 maps, average below bronze, plateaued.
**Nothing in the corpus has ever beaten author times across a whole campaign, by
any method, on either game.**

Milestone 3's target is stated as **the plain oracle reporting `cps ≥ 1`**,
which is instrument-independent — deliberately, because "the 512 m gate" is
defined on a route whose origin turned out to be wrong.

---

## 8. The start-state investigation — root cause, fix, and acceptance

*Added after the lead stopped training on 2026-08-24 to root-cause this. Right
call: a policy trained before it learns nonsense.*

### 8.1 What the start is, established WITHOUT the MapPack

Measured from the engine's own memory. `mapgeom`'s `MapPack` is not involved in
the measurement at any point.

**Summer 2026 - 01: the validated car begins at (1360.00, 10.0015, 1108.80),
velocity exactly (0, 0, 0), at race −0.010 s** — 389 m from the map's own
`RoadTechStart` block at (1584, 16, 784).

### 8.2 It is NOT our container: the minimal-pair sweep (`tmenv fieldsweep`)

Fourteen arms, each changing exactly ONE field from a baseline re-measured in
the same batch, same tape, every arm banking its container, trajectory and the
server's raw transcript.

| arm | start moved? |
|---|---|
| baseline | — |
| `declared_ms` 2500 | no (0.23 m) |
| `declared_cps` = 1 entry | no (0.24 m) |
| walltime pair left at zero length | no (0.23 m) |
| no result chunk | no (0.53 m) |
| no racetime chunk | no (4.43 m) |
| no login | no (0.03 m) |
| `declared_ms` 0 · `declared_cps` ×4 · `start_offset_ms` −3000 · `validation_seed` · no validation chunk · class = Replay · `uid_enc` plain | UNMEASURED (locate or server refusal), not evidence |

**No field moves the start.** The container is not the cause. That is a result,
not a dead end: it says the investigation belongs in the engine's map loading,
not in our writer.

### 8.3 The empirical rule, across maps (`tmenv startmap`)

Two hypotheses that disagree, tested on a campaign where 7 maps have
`waypoint[0] == Spawn` and 18 do not:

| map | waypoint[0] | measured start | d(Spawn) | d(waypoint[0]) |
|---|---|---|---:|---:|
| Summer 2026 - 01 | Checkpoint | (1360.00, 10.00, 1108.7) | 394.4 m | **4.9 m** |
| Summer 2026 - 12 | **Goal** | (944.00, 0.50, 1116.9) | 583.7 m | **12.9 m** |
| Summer 2026 - 02 | Spawn | (1680.0, 8.5, 848.9) | 316.8 m | 316.8 m — but **0.9 m** from waypoint[1], the first *non-Spawn* |

**Three for three on "the first NON-Spawn waypoint in file order".** On
Summer 2026 - 12 that is a `Goal`, which is about as sharp as a prediction gets.
Two more maps came back UNMEASURED (locator budget) and are not counted.

**WHY the engine does this is still UNKNOWN and is still a task.** The rule is a
description with three data points, not a mechanism, and it is written here as
such.

### 8.4 The fix, and the residual it closed

The environment's root was `Checkpoint::Tick(0)`, which goes through a line
fitted on three segment maps and lands at **tick ~95** — race 0.95 s, 14.8 m/s,
5 m downroad, all of it driven by the reference tape and untouchable by the
policy.

Three changes took the root to **tick 0**:

1. **A root ladder** of raw `lroundf` counts, earliest first, instead of the
   fitted tick line.
2. **`locate_windows`** — the inherited sweep budget of 24 windows is tuned for
   a checkpoint half-way through a run. Near the start it runs out and reports
   *"best candidate is not self-consistent enough after 24 windows"*, which is a
   statement about the **budget** and reads like a statement about the car. At
   600 it locates.
3. **`reset_is_coherent`** gates the `FK_STATE_OFF`-style rebase. Rebasing a
   layout from another server is not free — the heap is bimodal run to run — and
   the failure is a smooth-looking trajectory of nothing. Measured once:
   position (1085.23, 0.69, 5.33) at race 1155135.336 s and 496.01 m/s. The gate
   rejected four bad rebases in the accepted run and let the good one through.

### 8.5 `tmenv reset-control` — the permanent acceptance gate

Run it before any training run.

| clause | env's root (ladder) | the OLD root (must fail) |
|---|---|---|
| 1 · position within 6 m of the independently measured start | **0.07 m** PASS | 6.81 m FAIL |
| 2 · speed at or below 4 m/s | **0.22 m/s** PASS | 14.81 m/s FAIL |
| 3 · no checkpoint already collected | 0 PASS | 0 PASS |
| 4 · fresh-process reconstruction agrees | **359 ticks, max \|Δp\| 0.000000 m, 0 clock mismatches** PASS | — |
| | **ACCEPTANCE PASS** | fails 1–3, **as it must** |

`--also-check-old-root` re-runs clauses 1–3 against the old root and requires
them to FAIL; if they pass, the tool aborts, because a control that passes on
the setup it was written to catch certifies nothing.

Banked per acceptance run, under `--bank`: the written tape, the env
trajectory, the fresh-process reconstruction, the independently measured start
trajectory, and the server's raw transcript and stderr.

The env now resets at **race −0.010 s, (1360.0000, 10.0015, 1108.8000),
velocity exactly zero** — the car at rest on the line — and the reconstruction
is identical row for row from that tick.

### 8.6 The risk this leaves open, and the test that settles it

**If `/validatepath` starts the car at a checkpoint rather than at the start
line, the race we optimise is not the race the author time describes**, and
"under 23.144" would not mean what it says. Milestones 1–3 are unaffected;
milestone 5 is not.

Neither route settles it by plausibility — 1639.2 m needs 70.8 m/s over the
author time and 1900.3 m needs 82.1 m/s, and a Stadium car does both.

**The test:** the first tape that finishes. If the validator is running a
shortened race, our finish time will be implausibly far under 23.144 for a
policy that is visibly not driving well. Do not report any time against the
author time until that check has been made.

---

## 9. CORRECTION to §8.2, and the resolver migration

### 9.1 My minimal-pair conclusion was over-stated

§8.2 says "**No field moves the start.** The container is not the cause."

**The second sentence is wrong and I am striking it.** The sweep varied the
fields `GhostMeta` and `ChunkSet` expose — declared time, declared splits,
walltime, the result / racetime / validation / login chunks, class id, uid
encoding, validation seed, start offset — and **did not vary the container's
record / sample state**, because I had read `ChunkSet` as carrying no sample
block and stopped there. That was a scope I never stated, so the conclusion
read as broader than the evidence.

**The RE agent's finding (2026-08-24, via the lead): the synthesized
container's record/sample state seeds the validator's car near CP3.** The
authoritative validator → participant → vehicle pointer is traced, the
hard-left/right mirror passes on it, and moving the map's start block 64 m
leaves the actual car unchanged.

So the correct statement of my sweep is: **no field the container *writer's*
current API exposes moves the start** — which is a fact about our API surface,
not about the container. The container is the cause.

This is RULES §3 exactly — *ask what else could have produced this output* — and
I did not ask it hard enough about my own negative result. A negative from a
sweep is only as wide as the sweep, and mine did not say how wide it was.

The empirical "first non-Spawn waypoint" rule in §8.3 survives as a
**description of the symptom** — it is what a CP3-seeded car looks like from
outside — and is not a mechanism. The mechanism is the pointer chain.

### 9.2 What the environment does next

Per the user directive, and **not** to be implemented ahead of the RE agent's
typed resolver commit:

* migrate the env to the **validator-owned car resolver**;
* **delete every old location path** — `locate_v2`, the candidate scans and
  their ranking, `FK_STATE_OFF`, `control::locate` / `locate_windows` /
  `locate_offsets` / `rebase`, and the cached spawn files;
* add a **compile-time typed boundary**, so nothing can reach a car address
  except through the resolver — the point being that the old paths cannot come
  back by accident;
* add runtime **start**, **moved-start** and **left-right** controls.

**Training stays stopped until that migration passes.**

### 9.3 Evidence from this side that the migration is right

The value-based locator is not merely inelegant, it is **unreliable**, and the
env's own gate now measures how unreliable. With the root required to be the
start (`RootCfg::require_start`), three identical repeats gave:

| repeat | outcome |
|---|---|
| 1 | **REFUSED to build** — every rung of the ladder, three tries each, landed 1.5–6.8 m downroad at 7.1–14.8 m/s |
| 2 | root at `lroundf` 11000, probe tick **0**, race 0.000 |
| 3 | root at `lroundf` 14000, probe tick **11**, race 0.110 |

Refusing is the right failure — far better than silently rooting half a second
into the race — but a one-in-three refusal rate is a property of
`locate_v2` + the bimodal-heap rebase, i.e. exactly the machinery being
deleted. A resolver that reads the validator's own pointer has no candidate
ranking to get wrong and no offset to go stale.

### 9.4 What the acceptance gate needs from the fix

`tmenv reset-control` compares the env's reset against a start measured from
the engine. **That measurement is currently of the WRONG start** (the
CP3-seeded one), so the gate as it stands certifies "the env resets where the
broken container puts the car".

Two things make it catch the fix instead:

1. **The cache is keyed on the container's content** (FNV-1a over the reference
   file), not on the map. A cache keyed on the map alone would go stale the
   instant the container writer changes, and everything downstream would keep
   passing against a start that is no longer where the car is. That is done.
2. **A semantic clause**: the measured start must agree with the map's own
   `Spawn` waypoint. It **FAILS today** (394 m out on Summer 2026 - 01) and must
   PASS after the container fix. That is the clause that turns this gate from
   "the env agrees with itself" into "the env starts the race where the map
   says the race starts".

---

## 10. The container-template boundary

**Ruling (2026-08-24):** RL may use a game-recorded ghost as an **opaque file
container / startup-state template**. It may **not** learn from or inspect that
ghost's driving inputs or trajectory. Route, reward and progress stay
map-derived; observations come from authoritative validator-owned vehicle
state; evaluation is the plain oracle on our generated tape.

A narrow carve-out from RULES §1 is exactly the kind of rule that decays into
"we meant to", so it is a **type**, not a convention.

### 10.1 Compile-time: `tmenv::template::Template`

The donor's bytes are **private**. The whole public surface is three items:

| | |
|---|---|
| `load(path)` | the ONLY place in the RL tree that opens a game-recorded file, so `grep` finds all of it |
| `facts()` | `ticks` and `origin`. Tick COUNT is a property of the file's length, not of how the car was driven |
| `write_with_inputs(steer, gas, brake, out)` | writes a container carrying **our** archive in the donor's wrapper |

There is no accessor for the donor's steer, accelerate, brake or respawn
channels, none for its samples or telemetry, and none for its raw bytes. Not
"we don't call them" — **they do not exist**, so a future edit that reaches for
one does not compile.

### 10.2 Runtime: `tmenv template-control`

Measured on a stand-in container (the code path is identical for a
game-recorded one):

| clause | result |
|---|---|
| 1 · the written archive is exactly our tape's length | 4000 vs 4000 **PASS** |
| 2 · it decodes to OUR inputs, tick for tick | all ticks **PASS** |
| 3 · a DIFFERENT input set decodes differently | **PASS** |
| 4 · a SHORT tape is refused, never padded from the donor | **PASS** |
| | **BOUNDARY PASS** |

Two things worth saying about the shape of this test.

**It never reads the donor's inputs.** A test that had to read them to prove
they had not leaked would *be* the leak. Clause 2 works instead by determinism:
an input archive is fully determined by its ticks, so an output that equals
ours contains none of anybody else's.

**Clause 3 is the negative half.** Without it, "the output equals our inputs"
is satisfied by a comparison that cannot fail. Clause 4 closes the one place
the donor could still get in — a short tape padded from the template would let
its driving into the tail — and the refusal message says so.

With no `--template` the tool exits **UNMEASURED**, not PASS: a boundary test
that reports green when it examined nothing is the worst kind of green.

### 10.3 Still to do, gated on the resolver

1. Migrate the env to the validator-owned car resolver and **delete** every old
   location path (§9.2).
2. Take a **game-recorded Summer 2026 - 01 container**, replace the entire input
   archive through `Template`, and **prove live start at `RoadTechStart`** —
   which is the semantic clause of §9.4 and fails by 394 m today.
3. Re-run `tmenv reset-control` and `tmenv template-control` together.
4. Only then re-measure throughput and resume training.

## 11. Operating rules learned 2026-09-07 (ENV arm)

* **Work dirs on tmpfs.** `--work /dev/shm/<name>` for every `tmenv`/`tmrl` run. On a disk filesystem (`/tmp` on
  the devvms is btrfs on a virtual disk) the per-step trace-file churn caps a whole 166-core box at ~7.8k env-steps/s
  whatever the worker count, CPUs idle; on `/dev/shm` the same box gives 12.9k (k=1) to 14.6k (k=10) env-steps/s at
  96 workers — the reference table for the launcher is
  `env/controls/2026-09-07-merge-locate-perf-9f81d1d0/G5-reference-96workers-tmpfs-cfa96ecf.log`. Both binaries
  WARN when `--work` is not on tmpfs (`tmenv::warn_if_not_tmpfs`).
* **The label convention** (tmstate `LABEL CONVENTION`): env row T ≡ `fk regen --dump-truth` row T ≡ the ghost's
  telemetry sample T, all the physics at race T (three-way check, 0.5 mm at Δt = 0 on Summer 2026 - 01/02). No lag
  constant anywhere; `Layout.clock = sim+0x48`, `clock_bias = race_start` (LOCATE.md).
* **The car is derived, not located** (`forkoracle::car::locate`, LOCATE.md); the env re-derives the driven vehicle
  in the paused node before every fork and redoes a step that crossed a car-switch gate tick by tick.
* **Templates:** a policy template has validation seed 0 (`tmenv from-template`); an identity replay keeps the donor's
  seed and countdown records (`--keep-seed`); a map with no ghost gets `--map M --declare-ms T --cps N` (a borrowed
  recorded container, INPUT's recipe). Human tapes reproduce only in their own container (the validation seed
  quantizes inputs).
* **Probes:** the boundary comes from the node's own hello clock (PERF); the page-fault probe is a 1-in-50 control.
  Its SIGSEGV handler runs on an alternate stack and allocates nothing — the input array is a heap chunk whose edge
  pages hold neighbouring chunk headers, so the engine faults on them from inside malloc (that was the 10 % start-up
  flake and the `main_arena` hang; both gone).
* **Before consuming another box's fresh bank files** run `persistent-storage remount private-30d` INTERACTIVELY (over a
  non-interactive ssh it fails for want of an identity and leaves the store unmounted).
