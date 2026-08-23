# SEARCH.md — the search and oracle-driver layer

The part of the toolchain that actually finds faster runs, and the one place a
result is allowed to leave it.

```
cd tools/search
cargo build --release
TM_SERVER=/path/to/TrackmaniaServer-dir cargo test --release    # 85 checks
```

> **2026-08-22: this workspace did not compile, and its end-to-end tests had
> never run.** `tmsearch` still imported `ghost::container`, `ghost::tape` and
> `tmtraj::entrec`, none of which have existed since the audit reorganised those
> crates (the record decoder is `gbx::record`; `Container`, `Tape`, `Encoding`
> and `secs` are re-exported at `ghost`'s root). And the five `oracle_e2e`
> checks named their fixtures `../../ghost/testdata/...` — a CWD-relative path
> into a directory the audit merged into `tools/testdata` — so on a box with no
> server they skipped, and on a box with one they died on a missing file. Both
> are fixed: the fixtures resolve from `CARGO_MANIFEST_DIR`, and all 41 checks
> pass, with the five e2e ones actually talking to the server. `TM_REQUIRE_ENGINE=1`
> turns a skipped e2e check into a failure; use it on any box that has a server.
>
> `--seg` also compared a segment map's answer against `splits[k - 1]`, which
> was reading the ghost-result chunk's **version word** for checkpoint 1:
> `Container::splits()` used to return the raw chunk. It returns the decoded
> checkpoint list now, and a `0.000` entry (what `ghost declare --cps` writes
> for a checkpoint the file does not know) is skipped rather than compared.

Three crates, one workspace, one `cargo test`:

| | |
|---|---|
| `tmsearch` | the search: the candidate writer, the objective, the island/Metropolis loop, the two evaluators, and the guard |
| `forkoracle` | the fork oracle: protocol, watchdog, record layout, the car locator, and the mutation operators — shared with `fk`, which measures the watchdog's false-positive rate against this exact mix |
| `forkshim` | the LD_PRELOAD half of the fork oracle, loaded into the game server |

`forkoracle` and `forkshim` are in the same workspace because the shim
`#[path]`-includes `forkoracle/src/pred_core.rs`: a predicate has exactly one
definition in the parent that arms it and in the fork child that evaluates it.

---

## 1. The surface

```
tmsearch search   --template G --map M [--fork ...] [flags]
tmsearch dump     --template G --map M --n N [--out F.jsonl]
tmsearch analyze  --log F.jsonl --base SECONDS
tmsearch validate --map M GHOST...
```

Times print as **seconds with a decimal** — `36.049`, never `36049` — and every
flag that takes a time takes seconds too (`--temp 0.030`, `--base 23.000`).

### What each knob is for, and which ones were experiments

**Real, and load-bearing:**

| flag | what it decides |
|---|---|
| `--template` / `--start-from` | the file whose bit layout every candidate shares, and the inputs to start from |
| `--map`, `--seg K:MAP` | the true objective, and the segment maps that give failures a gradient |
| `--workers --batch` | parallelism, and how many candidates share one oracle launch |
| `--nops N` / `--nops-upto N` | operators per candidate |
| `--ops local\|wide\|doublet\|retime\|scale` | the move set; naming one is what makes an A/B of it possible |
| `--lo --hi --window --stride` | which ticks may be edited, and the sliding window over them |
| `--temp SECONDS --migrate P` | Metropolis temperature and island migration |
| `--root --bestdir --log` | where candidates, confirmed results and the audit trail go |
| `--fork --forktick T --refcsv\|--refghost --shim --pred --finishmargin --corridor` | the fast evaluator and its watchdog |
| `--gate --gate-key --gate-min-key --gate-seed-state` | the state objective: score the car's STATE at a place when finish time cannot cross the valley. See §5 |
| `--fire --fire-at --fire-where --after-key` | the event: a thing that HAPPENS, and what to score after it. A place and an event are not the same shape. See §5.9 |

**Added, because the behaviour existed but was not addressable:**

* `--full-window-every N` — the old loop had `if wc % 8 == 7 { whole range }`
  hard-coded. It is a real and useful behaviour (a search that has settled
  inside one window can still make a global move) and it was a magic number.
* `--refghost G` — take the reference line from the incumbent's own telemetry
  instead of a separately measured CSV, behind a check that the telemetry
  belongs to that file's tape. See §4.
* `--nops-upto N` — the old `--nops` took a NEGATIVE number to mean "up to";
  a flag whose sign changes its meaning is a flag with two meanings.

**Deleted as one-off experiments:**

| gone | what it was |
|---|---|
| `--sweep lift\|turn\|steer` (+ `sweep.rs`) | an exhaustive sweep over map 2's sector-2 corner: the first contiguous throttle-lift group, the turn-in tick, and flat steering magnitudes. It answered its question (the two-tick lift at ~9.8 s, slow-in/fast-out) and every line of it is map-2-shaped. **The lesson it paid for is kept**: an earlier version cleared every lift tick in the window and put back a single group, which silently deleted the run's SECOND lift and made all 916 candidates DNF — including the ones that should have reproduced the baseline. A sweep whose baseline does not reproduce is measuring its own bug. |
| `--bench N` | a micro-benchmark of the candidate writer. It is now a fact in this file (26 000 candidates/s/core) and a test that the writer agrees with the codec. |
| `--verify OUT` | wrote the start state to a file. `ghost tape inject` does that. |
| `--fix-walltime` | patched the walltime chunk. `ghost` owns the container. |
| `tmtas splits` | printed the declared splits. **This is the trap, not the tool**: `splits` reads the header, so a synthesised tape reports its donor's numbers. `ghost inspect` prints them next to what the file actually does. |
| `tmtas trace` | per-tick input dump → `ghost tape extract`. |
| `tmtas splice` | cross-spliced two runs at a checkpoint to measure how not-modular they are. The measurement is real and its answer is recorded (§5); splicing tapes is `ghost tape` territory now. |
| `tmtas selftest` | `cargo test`. |
| `fkcount` (a whole crate) | an LD_PRELOAD census of every libc entry point the server called, looking for one whose per-ghost count was a clean multiple of the tick count. **It found `lroundf`** — ~25.5 calls per simulated millisecond, bit-identical across runs on an idle box — and that answer is the foundation of the whole fork oracle. It also proved `rand()` is called only during init, so there is no RNG in the simulation. The scaffolding is deleted; both facts are in `forkoracle`'s module docs. |

**Two flags I was told to delete and could not find.** `FK_BUDGET_MUL` (in
`budget_for`) and `FK_SAMPLE_CENSUS` (in `gather_ticks`) do not exist in any
archived tree: both functions are present in `locate2.rs`, neither reads an
environment variable, and a scan of every `.rs` file inside every `.tgz` in
`tm-map2/` and `tm-unbeaten/` finds zero occurrences of either name. They were
either never banked or already removed. Every other environment knob in the
layer (`FKDBG`, `FK_QERR_MAX`, `FK_LOCATE_STRIDE`, `FK_ANCHOR`, …) belongs to
the car locator, which now lives in `fk`.

---

## 2. What moved into `tools/ghost`

`tmsearch` carried **a complete second implementation of the `0x0309201D`
codec** — `ghost.rs`, `replay.rs`, `gbx.rs`, `bits.rs`, about 1 100 lines. All
four are deleted. `tools/ghost` owns the format.

That was not only tidiness. The old writer emitted a mode-12 "same as the
previous tick" packet as a single bit and marked the tick `Slot::FROZEN`, and
**a write to such a tick was silently dropped** — the search could not express
those candidates and nothing said so. It had never bitten only because the
affected ticks sat below every resume boundary in use.

### The fast path, and why it is not a third copy

The search writes ~26 000 candidate files/s/core by patching bits in one base
image rather than re-encoding. That needs bit offsets, which is exactly the
knowledge that must not be duplicated. So `tmsearch::tape`:

1. asks `ghost` for the whole file with every vehicle field written explicitly
   (`Tape::inject_into(&container, Encoding::Explicit)`) — which fixes the bit
   layout, and expands the mode-12 packets as a side effect;
2. **finds each tick's bits by probing that encoder** — flip every steer field,
   ask `ghost` to encode again, read the positions off the difference. Three
   encodes, once, at startup;
3. patches `memcpy` + one 8-bit and two 1-bit writes per tick.

`tests/patcher.rs` closes the loop: a patched image must equal a full re-encode
of the same inputs, byte for byte, on random states. The probe cannot drift
from the codec, because the encoder is what it measures — and it caught a real
error on the first run: the codec is **LSB-first** within a byte, and the
patcher's first draft was MSB-first. The probe reported run lengths of 1, 2, 11
and 14 bits where an 8-bit field was flipped and refused to build, instead of
writing 26 000 wrong files a second.

Ticks that genuinely cannot take an 8-bit steering value — a 32-bit steer
field, a packet with no vehicle fields, a trigger-only packet — are listed at
startup, and `Patcher::check_window` **refuses a search window containing one**.
A limit that is stated is a task; a limit that is silent is a defect.

The plain oracle is also `ghost`'s (`ghost::oracle`), for the same reason: the
server prints two results per file and the second is the file's own claim.

---

## 3. The objective, and the guard

### There is no `FINISH_BASE`

The old score was a bare `i64`: `FINISH_BASE - ms` for a finisher,
`cps * SEG_UNIT - cp_time` for a failure, with `1e8` and `1e7`. On an
eleven-checkpoint map a DNF at checkpoint 11 scores `1.05e8` and beats a
finishing 96.281. Raising the constant fixes the arithmetic and leaves the
shape: two meanings in one integer, ordered by luck.

`Outcome` is now an enum whose `Ord` puts every finisher above every
non-finisher **by construction**, for any checkpoint count, any map length and
any progress measure. `no_dnf_ever_outranks_a_finisher` sweeps depths 0..64
against an hour-long finisher. Metropolis acceptance is defined only between
two finishers, because that is the only case where the delta is a number of
seconds — there is no temperature in units of "checkpoints".

### The guard is a type, not a step

Four separate defects have made this search report a time for a tape that does
not achieve it. The guard is the only defence that does not care which one it
is, including a fifth nobody has found: it takes the bytes that were actually
written, hands them to the plain oracle, and compares. About 0.1 s per
improvement.

So it is not a function anyone can forget to call. `Bank` owns the output
directory and the only method that puts a file in it is `Bank::offer`, which
validates first. There is no code path that banks an unconfirmed time. A
refusal preserves the tape as `PHANTOM_*.Ghost.Gbx`, logs the disagreement,
leaves the incumbent where it was, and stops the run. **An oracle that cannot
answer is also a refusal** — the guard fails closed.

Every banked result carries its **provenance**: whether the score came from a
fork, which tick that fork resumed from, and how far the tape is from the
reference the fork checkpointed on (first differing tick, how many ticks
differ, largest steering move). That is the number that decides whether a fork
answer means anything: 0 of 312 fork-reported finishes survived a plain
re-validation when the tape was not a small, late perturbation of its
reference.

### The resume floor

The `lroundf` checkpoint is not a fixed simulation point, so each worker's
server stops where it stops. An edit below a worker's own resume tick is a
silent no-op — invisible to the evaluator, present in the written file, scoring
exactly the incumbent's score, accepted at `delta == 0`, contaminating that
worker's lineage for free.

Every worker now probes its own server, publishes `max(calibration, probe + 1)`,
and a startup barrier holds the fleet until all of them have. The mutation
floor is the **maximum** over workers — it must be the maximum, because
migration moves a state made by one worker into another.
`tests/loop_invariants.rs` runs the loop against a fake oracle with per-worker
floors of 100/140/170/181 and fails if any candidate differs from the reference
below 181. Reverting the floor to each worker's own makes it fail at tick 100.

---

## 4. The controls, and what each one proved

### First: did the old suite pin anything?

`tmsearch/tests/invariants.rs` had six tests and they are well chosen — the
factory-identity one pins a real bug that cost an afternoon (a `FROZEN`
sentinel index added to a stream base offset wrapped into a real bit position
and clobbered byte 0 of every candidate, silently, and only for the one seed
that had such packets).

**And on this box, with no fixtures present, the whole suite reports `6 passed`
in 0.00 s having asserted nothing.** Every test is guarded by
`if !Path::new(p).exists() { continue }` against absolute paths that live
outside the repo — `/tmp/m2/ghosts/rank00001_22730.Ghost.Gbx`,
`/tmp/tmoracle/replays/r4167.Replay.Gbx`. So it pins behaviour on a machine
that happens to have someone's scratch directory, and is decoration everywhere
else. The `0.00s` is the tell.

Both halves are fixed here. The fixtures are the two human ghosts and the map
**checked in under `tools/testdata`**, addressed from `CARGO_MANIFEST_DIR` so
they do not depend on where the test is run from, and so a missing one is a
panic rather than a skip. The engine-dependent tests do skip, and say
so — and `TM_REQUIRE_ENGINE=1` turns that skip into a failure, so a box with an
engine cannot quietly stop running them.

### The controls

Everything below was run on this box, against the real dedicated server, with
the fixtures checked in under `tools/testdata`.

| control | result |
|---|---|
| **the patcher against the codec** | a patched image equals a full re-encode of the same inputs, byte for byte, on 24 random states across two fixtures |
| **the patcher through the engine** | the template rewritten by the patcher re-simulates to 22.730, the time the original file does |
| **the guard accepts a true claim** | banked, and the file is on disk |
| **the guard refuses a false one** | claiming 22.230 for a tape that does 22.730 → `PHANTOM_22_230_*.Ghost.Gbx`, no `best_` file, `phantoms = 1`. *A guard that cannot fail is not a guard* |
| **the guard refuses a kind mismatch** | a DNF claim for a tape that finishes is refused |
| **the resume-floor test can fail** | reverting to the per-worker floor makes it fail at tick 100; the fix passes |
| **the arm payload can fail** | swapping one field offset in the shim's parser makes the round-trip test fail; the shim as written passes |
| **a real search, plain oracle** | 23.013 → **22.935** in 2 minutes, 24 workers, 22 350 evaluations, 178 eval/s. **10 improvements, 10 confirmed, 0 phantoms** |
| **re-validation from a separate process** | all 10 banked files re-simulate to exactly the time in their name |
| **a real search, fork oracle** | 23.013 → **22.923** in 2 minutes, 6 workers, 11 700 evaluations. **12 improvements, 12 confirmed by the plain oracle, 0 phantoms** — the fork agreeing with the plain oracle 12 times inside its own regime (late-window perturbation of a human seed) |
| **the reference line taken from a ghost itself** | gated on the engine re-simulating that file's own tape: **0.0005 m** mean over 461 samples, no phase shift — and the fork search then ran on that line |
| **the state objective's key against the eleven modes it replaces** | seven expressions against the arithmetic they were, transcribed, over 400 states — agreement to 1e-3 relative |
| **the state objective through the engine, on the map it was proven on** | the fork's measured gate state for the seed against the seed's own recorded telemetry: **0.0002 m, 0.067 m/s of speed, 0.965° of heading, 0.009° of attitude** — and the author's own contact, measured the same way, scores **86.81**, reproducing the published 86.8 m/s of body-lateral speed |
| **the decoy test on a real map** | fired first time out and was right: a tape that stops driving drifts into the tight box (key 0.014) while the seed misses it by 1.53 m. The run stopped before the first candidate |
| **the search climbing a state key** | 228811, seeded with the human world record: key **0.97 → 57.4**, and the state it scores moves from z = 714.9 to **z = 709.1**, the launcher line |
| **the launch detector against ground truth** | armed on the author's own lap it fires at **+118.68 m/s** in one tick (published: 323 → 751 km/h = 118.9) and the after-key puts him **5 mm** from the finish; on the human world record the same clause never fires |
| **the whole ladder, on a map with known ground truth** | 228811 (already beaten -- incumbent 20.237, AT 20.555 -- which is why it is the right place to prove an instrument). From the human world record as its seed: state → launch → aim → **a validated finish on the launcher route no human drives**, in one hour against the hand-built private fork's 2 h 43 min. 216 improvements confirmed, **0 phantoms** |
| **peak speed is not a launch detector** | a smooth run to 151 m/s -- the speed the world record itself reaches -- does not fire the rise detector, while the speed-thresholded control in the same test does |

### One check that did not work, one that did, and a false negative I nearly published

`--refghost` takes the reference line from a ghost's own telemetry. That is
only safe if the telemetry belongs to that file's tape, because **a synthesised
tape carries its template's telemetry** and a search output's recorded
trajectory is the seed's, byte for byte.

**The cheap test does not work, and here is the measurement.** A ghost holds
its driver's inputs twice — the 10 ms input chunk and byte 14 of every 50 ms
sample — and `ghost::verify` scores their agreement as chance-corrected Cohen's
kappa, published as 1.000 for a recording of its own run against 0.120 for a
wholesale-contaminated file. That looks like a free gate. Measured here on
`human_23013.Ghost.Gbx`, an ordinary game recording: **kappa 0.919**, while
lightly-grafted search tapes score around 0.83. Nine hundredths apart with a
sample of one on the good side. No threshold on that statistic separates them,
so it decides nothing and is reported as context only.

**The decisive test does work.** The real engine re-simulates the file's own
tape and the trajectory it produces must match the one the file records
(`ghost::regen::engine_trajectory_agreement`), with a separate check for a
whole-sample phase shift, because a one-tick offset is a pure time shift that
hides inside a small mean. On this fixture:

```
fork: reference line from human_23013.Ghost.Gbx's own telemetry: 461 samples,
      and the engine's own run of its tape sits 0.0005 m from it (kappa 0.919)
```

**And the false negative.** The first time I ran that gate it failed with *"the
engine readout did not identify the car in 24 attempts"*, and I wrote it up as
a harness limit of this box. It was not. That check shells out to `fk regen`,
`fk` was not on `PATH` and `FK_BIN` was unset, so all 24 attempts failed to
launch a binary — and a failure to launch is reported in the same words as a
failed locate. The same trap is documented inside `ghost::regen` itself for the
shim ("pointing the fork server at a shim that is not there fails with a bare
`NotFound` six times in a row and looks like six bad locates"), and I walked
into the neighbouring version of it and nearly published *map 2's locate is
flaky* on the strength of it.

Two things came out of that. The failure message now names the wiring as the
first thing to check, in the words above. And the general rule this project
already pays for is worth restating: **when a control fails, the null is about
your instrument until you have shown otherwise** — a negative result needs a
positive control even when the negative is about your own tooling.

### The identity control the search was never running

`forkoracle::layout::verify_tape` reads the decoded input array back out of the
server's memory and compares it tick for tick with the tape we meant to
measure. It is called by four of `fk`'s commands and was called by **nothing in
the search**, which is the process that runs 150 servers at once. It now runs
at every fork worker's startup, on the tape the server was started with — not
on an incumbent read from the bank, because staggered workers read an
already-improved incumbent and then abort on a control testing the wrong tape.

---

## 5. The state objective

Built here. Three maps had needed it and each one hand-rolled it in a private
fork of the search, which is what a missing feature looks like. §5.1 of the
previous version of this file was a sketch; this is what it turned into, what
each part cost, and the two places the sketch was wrong.

**When finish time cannot cross a valley, score the car's STATE at a place.**

```
tmsearch search --fork ... \
  --gate 'xmin=56,xmax=136,ymin=48,ymax=54,zmin=704,zmax=715,minspeed=60' \
  --gate-key 'min(abs(bodyright), 5*(-vz))' \
  --gate-min-key 60 \
  --gate-seed-state humanWR.Ghost.Gbx
```

That is the objective that found map 228811's launcher, written down instead of
compiled in. It needs `--fork`: the plain oracle returns a time and a checkpoint
count and cannot see the car at all, so `--gate` without `--fork` is refused
rather than silently scoring every candidate as a miss.

### 5.1 The bands are variants, not numbers

```rust
pub enum Outcome {
    Finish { ms: i64 },
    Gate(GateState),
    Dnf(Progress),
}

pub enum GateState {
    Missed { miss_m: f64 },   // never got in; ranked by closest approach
    Reached { key: f64 },     // got in; ranked by the state key
    Finished { ms: i64 },     // got in well enough, and finished; ranked by time
}
```

The failure this shape exists to prevent is **a near miss outscoring an
arrival**, and it is not hypothetical: the working version extended the key
below the box as `-miss`, the in-box key was itself large and negative, and
grazing the boundary at −0.001 beat every candidate that got inside. The search
then spent 100 000 evaluations perfecting a miss.

The fix at the time was `-(500 + miss)`. That is a convention, a constant, and
one more number to get wrong — the same shape as the `FINISH_BASE` this crate
deleted. Here the bands are variants: `Reached` outranks `Missed` for every pair
of values either can hold, there is no arithmetic between them and no constant
to tune. `a_near_miss_never_outranks_an_arrival` sweeps misses from 0 to
infinity against keys from −1e9 to +1e9.

**The sketch had two bands and it needed three**, because the third is where
"did it and finished" lives, and that band is a time again — so it is
re-validated by the plain oracle exactly like any other time.

### 5.2 The key is a program, not a mode number

The version that worked had eleven hard-coded `gate_mode` integers, each one a
match arm added to the shim that runs inside the game server. Two of them turned
out to be decoys and one was a sign error, and none of them could be tried
without rebuilding the shim.

So `--gate-key` is an expression, compiled in the parent into a fixed-size
postfix program that the child runs on a 16-slot stack: no allocation, no
strings, nothing to add to `pred_core.rs` the next time a map wants a different
quantity.

| term | value |
|---|---|
| `speed` `vx` `vy` `vz` `px` `py` `pz` | the plain ones |
| `bodyright` `bodyup` `bodyfwd` | velocity in the CAR's frame — `bodyright` is the ghost format's `side_speed` |
| `along(x,y,z)` | speed along a world direction |
| `nose(x,y,z)` `roof(x,y,z)` `flank(x,y,z)` | how well the car's forward / up / right axis points along a direction |
| `dist(x,y,z)` `vdist(vx,vy,vz)` | metres from a point, m/s from a target velocity |
| `abs()` `min()` `max()` `+ - * /` and parentheses | |

Bigger is always better, so a quantity to be minimised is negated in the open by
whoever writes it. `the_key_language_reproduces_the_objectives_it_replaces`
checks the expressions for seven of the eleven old modes against the arithmetic
they were, transcribed, over 400 states — two implementations agreeing is the
only evidence that deleting one changed nothing.

**Not expressible: `atan2`.** Old mode 2 was "the angle of the velocity off the
−x axis, in degrees", and there is no inverse tangent in the language. It was
one of the decoys, so nothing was lost here, but a map that wants an angle wants
either `nose()`/`along()` (a cosine, which is monotone in the angle over the
half-turn that matters) or a new opcode.

### 5.3 The key must be a function of the WHOLE state

Position and velocity together were not enough on 228811: the launcher ignores
both and triggers on which way the car is pointing. So the child's record now
carries the quaternion, the 48-byte summary grew to 108, and
`the_key_can_tell_two_identical_velocities_apart_by_attitude` pins it — two
states identical in every metre and every metre per second, differing only in
attitude, must score differently.

The quaternion was already in the gathered record (`R_QUAT`, 16 bytes at
`pos − 16`); what was missing was passing its offset to the child and four loads
per tick. The whole cost of the "whole state" requirement was that.

### 5.4 Outside the box, and inside it

The key is a **maximum over ticks** inside the box, and the whole state at the
tick that achieved it is what is recorded — not the state at first entry, which
is what the sketch said. Maximum-over-ticks is what makes the mode safe to run
with the watchdog armed: aborting a candidate only removes ticks, so an aborted
run's key is never higher than the same run left alone
(`aborting_can_only_lower_the_key` checks every prefix).

Outside the box the measure is the closest approach in metres, over the same
ticks the key would have been measured on, so "how close did it come" and "did
it arrive" are one measurement and cannot disagree. That is the gradient that
points a search which has never once fired the gate towards it — without it
every run outside the box scores the same and the search is a random walk
(measured on 228811: 160k evaluations, no gradient, nothing found).

### 5.5 `--gate-min-key`, which the sketch did not have and the map needed

**Entering a box is not doing the thing.** 228811's gate sits on 96 m of boost
deck that all 48 runs on the leaderboard drive across. The human world record
clips it with a key of **0.06** — doing none of the thing — and then finishes at
22.637. With no bar that is a top-band result: the seed is
unbeatable except by a faster ordinary lap, and the state hunt is a finish-time
search with extra steps, which is exactly the moat the mode exists to cross.

This was not foreseen; it was measured, on the first run of the feature on that
map. `--gate-min-key K` is the bar: a state under it does not count as having
done the thing, so a tape that clips the box and finishes still ranks as a
state. It defaults to no bar. See §5.10 for what turned out to be derivable
about it and what did not -- and for why, once an event clause is armed, the
event does this job better than any threshold on the key.

### 5.6 The identity control gets stronger, and it found a clock

In gate mode "did the fork reproduce the seed's millisecond" is unavailable: the
seed is normally aborted by a predicate long before the finish. The replacement
is the fork's measured gate state for the SEED against the seed's own recorded
telemetry — position, velocity **and** the quaternion. One comparison validates
the record layout, the car locator, the clock labelling, the box arithmetic and
the key at once.

On 228811, measured:

```
seed identity control at race 18.580 (PASS):
  position 0.0002 m (bar 1.7442)   speed 0.0669 m/s (bar 1.0839)
  heading 0.965 deg (bar 2.209)    attitude 0.009 deg (bar 4.339)
  clock: best fit at a shift of -1 tick(s); unshifted the position residual is 1.1954 m
```

Two things came out of building it, and both are properties of the data rather
than of this code:

* **The two clocks are one tick apart.** The child labels a state by the clock
  value it was gathered at; the sampler's own `sample_ms` labels the first
  record of tick `t` as the END of tick `t − 1`. At 118 m/s that one tick is
  **1.20 m**, and with the shift assumed to be zero the control failed on a
  measurement that is otherwise exact to 0.0002 m. So the shift is measured over
  ±2 ticks, reported, and allowed up to one tick; more than one is not a
  labelling convention and fails.
* **A ghost stores the velocity DIRECTION in two signed bytes.** `read_transform`
  decodes it as `vh = i8/127 * pi`, `vp = i8/127 * pi/2` — a quantisation step of
  1.42° — while the speed is `exp(i16/1000)`, a tenth of a per cent. So a single
  "velocity error in m/s" bar either fails a perfect measurement at speed or
  passes a bad one at walking pace: at 118 m/s one step of that byte is 1.5 m/s
  per axis, and the first version of this control duly failed at 1.99 m/s
  against a 1.95 bar while the position matched to 0.2 mm. Speed and direction
  are now checked separately, each against a bar derived from how the recording
  stores that quantity.

The bars are derived, not chosen: a floor plus a quarter of what that quantity
changed across the 50 ms sample interval the state falls in (the interpolation),
plus one quantisation step where the format has one.

If the seed never enters the box, the control cannot run and the search refuses
to start — "this recording never enters the gate box above 60 m/s, so it cannot
say what the fork should have measured". That is worth knowing in the first
second rather than the first hour.

**A shim that ignored the gate would score every candidate "never reached it"**,
which is a perfectly plausible answer. So the ARM ack now reports how many key
operations it installed and `ForkEval` refuses a mismatch: an `libforkshim.so`
older than the binary arming it is an abort, not a silent zero.

### 5.7 The decoy test, printed before the first candidate

> An objective that can be maximised without achieving the goal is not a proxy,
> it is a decoy.

The laziest tape the search can write is the one with every editable tick set to
no steering, no throttle and no brake. It is evaluated first, through the same
evaluator, and its score is printed next to the incumbent's before anything else
happens. If it wins, the run stops there.

**In fork mode this is not a parked car, and that is the point.** The server has
already consumed the seed's prefix, so the do-nothing tape is "the incumbent up
to the resume boundary, then hands off the wheel" — which is exactly the laziest
tape inside the search's real action space. It is therefore measured between the
two startup barriers, after the fleet's mutation floor is known: a probe that
blanks ticks below the floor is measuring edits the engine silently drops.

**It fired on the real map, first time out, and it was right.** With the tight
box (z ≤ 713) the human world record misses the gate by 1.53 m, while a tape
that simply stops driving at tick 1850 drifts into the corner of it and scores
0.014 — so a non-arrival was the incumbent and doing nothing beat it:

```
decoy test: the do-nothing tape (300 editable ticks blanked) scores GATE key
+0.0140; the incumbent scores no gate, 1.53 m away -- THE DO-NOTHING TAPE WINS.
This objective can be maximised without driving the map: it is a decoy, not a
proxy. Nothing was searched.
```

That is a true statement about that box: a search would have hill-climbed from a
tape that had thrown the race away. The cure is the box — a gate the incumbent
is outside and a blank tape is inside is a gate measuring the wrong event — and
the run that follows uses z ≤ 715, where the seed is inside and the key does the
work.

**What it does not catch.** The three decoys 228811 actually met were not of
this class: `-vz` alone, body-lateral speed alone, and progress along the
author's line are all objectives a *fast, driven* tape maximises without firing
anything. A parked car scores nothing on any of them. This check catches the
family where doing less scores more — a sign error, a box round the spawn, a key
that rewards slowness — and it catches it in the first line of output. It is not
a proxy for thinking about what the laziest way to maximise the objective is.

### 5.8 What the guard does with a state

A band-2 result is a finish and is re-validated exactly like any other: the
oracle's millisecond must equal the claim, and the bank writes the oracle's
number. A band-0 or band-1 result is a **state**, not a time, so there is
nothing for the oracle to contradict — a candidate the watchdog aborted has no
time by construction, and one that finishes without clearing the bar is what
band 0 and band 1 are for. The tape is banked with its state written out beside
it as `best_gate_*.state.json` (position, velocity, quaternion, body-frame
velocity, the key, the tick), so the claim can be checked by hand, and the file
never acquires a millisecond it did not earn.

A tape that turns out to finish while ranking at the bottom is called out on
stderr rather than hidden: the search ranked it low on purpose, and the time is
still real.

**And a defect this walked into, which was not the gate's.** The bank used to
return the ORACLE's answer for every confirmation, including failures — but the
plain oracle only ever reports checkpoints, while a fork search ranks failures
by metres along the reference line and a plain search with segment maps ranks
them by checkpoints *with a time*. Handing either back as a bare
`Checkpoints { cps, seg_ms: None }` returns a value from a different ladder;
`confirmed > incumbent` then compares two unrelated numbers and the improvement
is confirmed, written to disk, and **never adopted**. The first gate run on
228811 showed it as 49 confirmations and an incumbent that never moved. A
failure is now banked on the ladder the search ranks on — the guard's job there
is the kind check, *it did not finish*, and the rank is the search's own
measurement — and `a_failure_is_banked_on_the_ladder_the_search_ranks_on` pins
it. Any fork search that has not yet found a finisher was affected.

### 5.9 The event: a place and a thing that happens are not the same shape

The gate takes the car to a state. On 228811 that state is worth having only
because the map then fires the car from 323 to 751 km/h in one contact — and
nothing about a box can see that happen, or aim it. So there is a second clause:

```
--fire dspeed --fire-at 10 \
--fire-where 'xmin=40,xmax=80,ymin=45,ymax=60,zmin=700,zmax=760' \
--after-key '-dist(366.07,95.11,693.99)'
```

* **`dspeed`** is the one-tick rise in speed, and the one term in the key
  language that is not a property of a single instant. It is here because a
  launch is a discontinuity. **Peak speed cannot do this job**: the human world
  record on this map reaches 151 m/s at the finish under its own power, and
  `peak_speed_is_not_a_launch_detector_but_the_rise_is` pins exactly that — a
  smooth run to 151 m/s does not fire the rise detector, while the
  speed-thresholded control in the same test does.
* **The event is the FIRST tick the condition holds**, so a candidate that
  crosses the threshold twice does not get to choose.
* **`--fire-where` is not decoration.** A launch fired upstream of a checkpoint
  the run still has to collect flies beautifully, passes within a metre of the
  finish, and can never validate — measured on this map as 5 of 6 checkpoints,
  DNF. Without the box the band is a trap.
* **`--after-key` is measured only after the event.** The ordinary route passes
  within 99 m of the finish on its way down the track, so the same quantity
  measured from tick 0 pins every candidate at 99 m and flattens the objective
  exactly where it has to bite.

The bands become four and stay **cumulative**: `missed < reached < fired <
finished`, each requiring the one below. With a clause armed, a run that
finishes without firing is a `reached` — it drove the ordinary route, which is
the local optimum the mode exists to escape.

**GROUND TRUTH, with no server at all.** Armed on the author's own lap, decoded
out of the map:

```
fk watch replay --trajectory at_ghost.csv --fire dspeed --fire-at 10 ...
  fire: at tick 2020 (+118.68) at (75.63, 53.19, 708.33); after -0.0054 at tick 2210
  gate: key +86.8105 at tick 2015 -- pos (71.38, 50.36, 710.34) ...
```

`TECHNIQUE.md` puts his contact at 323 → 751 km/h, which is **118.9 m/s**; the
detector reads **118.68**. The after-key then places him 5 mm from the finish.
On the human world record the same clause never fires. One positive, one
negative, on real recordings, before any search ran.

**And a trap in the after-key that this control walks straight past.** The point
above is the author's own LAST TELEMETRY SAMPLE, so "5 mm from it" is a
statement about arithmetic, not about finishing: he is at that point by
definition. An after-key of `-dist(reference's own endpoint)` is a decoy of the
same family as any other -- the reference maximises it trivially, and it can
look like a validated objective when it is a tautology. It is still a usable
GRADIENT for a search that is nowhere near, which is what it was used for here,
but the number a reference scores on it is not evidence that the key means what
you want. What settles that is the band above: a candidate that actually crosses
the line becomes a `Finished` and meets the plain oracle.

### 5.10 `--gate-min-key`, and what is derivable about it

The bar was the one number in this feature that somebody had to choose, and
choosing it wrong turns gate mode quietly back into a finish-time search. Two
things改 that, and one does not.

**Derivable: its failure.** The bar exists to keep the SEED out of the top
bands. If the seed clears it, nothing the search finds can outrank the seed
except a faster ordinary lap, and the moat is still there. That is checkable
before anything runs, against the seed's own recording, and it is now a refusal
with the two numbers in it. `--gate-min-key auto` sets the smallest bar that
excludes the seed and says in the same breath that it is a **floor and not a
target**.

**Not derivable: the right value.** The right bar is near the key of the thing
being hunted, and if that number were known the search would be most of the way
there. `auto` on 228811 gives +0.078 against a target of 86.8.

**But with an event armed the bar mostly stops mattering**, and that is the real
answer. The event is a better anti-moat than any threshold on the key: the world
record finishes at 22.637 having fired nothing, so it lands in `reached` however
the bar is set. When there is a `--fire` clause, `auto` is the right setting;
when there is not, the bar is doing the event's job by hand and deserves the
suspicion the flag's help text gives it.

### 5.11 The second decoy family, met on the way

> An objective that can be maximised without achieving the goal is not a proxy,
> it is a decoy.

The startup decoy test catches one family: **doing less scores more**. It fired
on this map first time out and was right. It cannot catch the other family — a
fast, driven tape that maximises the key **somewhere useless** — and the three
decoys 228811 originally met were all of that kind.

It happened again here, live, and it is worth the space. With the gate box
spanning the whole 80 m deck (`x 56..136`), the search took the firing
conjunction to **100.5 — well above the author's own 86.8 — at x = 122.7**,
forty metres upstream of the checkpoint the run still has to collect. A perfect
state in a place where it can never pay.

**The symptom is not "against a face".** That winner sat 83% of the way across
its box, comfortably inside it. What is diagnostic is how far the optimum
**migrated from where the seed itself crossed**: `x +63.7 m (80% of the box's
80 m)`. So every improvement now prints its state's displacement from the seed's,
per axis, as a fraction of the box.

And it is a **report, not a verdict**, because the measurement says it has to be:
the decoy migrated 80% of the box on x, and the *correct* answer — the author's
own contact — migrates **51%** of the box on z, because that axis is only 9 m
thick and he legitimately crosses low. A threshold fitted between 51% and 80% is
a threshold fitted to two points. The numbers are printed; a person decides.

The cure for the decoy itself is the box: narrowing x to `56..80` puts the gate
where a launch can pay. That box has to include the seed's own crossing or the
identity control cannot run — which is itself a useful constraint, and the
search refuses rather than proceeding without the control.

### 5.12 What it cost, and what it bought

85 checks, up from 41. New: `forkoracle/tests/gate.rs` (19, of which 8 are the
event) and `tmsearch/tests/seed_state.rs` (5), plus the band and decoy checks in
`score.rs`, `loop_invariants.rs` and `oracle_e2e.rs`. Everything but the
engine-backed tests runs with no server, on the fixtures in `tools/testdata`,
anchored on `CARGO_MANIFEST_DIR`.

On 228811, seeded with the human world record:

| arm | objective | result |
|---|---|---|
| C | `min(abs(bodyright), 5*(-vz))`, wide box | key **0.97 → 57.4**; state walks onto the launcher line, z 714.9 → 709.1. 1 049 160 evals, 99 confirmed, 0 phantoms |
| D | the author's whole contact state | **−43.7 → −5.11**: 0.29 m from his contact, 3.60 m/s from his velocity, **53.8° away in attitude**. 870 570 evals, 159 confirmed, 0 phantoms |
| K | the same key, box narrowed to where a launch pays, launch clause armed | **the whole ladder, and the launcher fires.** `GATE key +1.11` (the seed) → `+98.40` → `FIRED, after −298.75` → `−19.99` → **a validated finish on the launcher route**. 2 732 610 evals, 216 confirmed, **0 phantoms**. Not a record: see below |

Arm D reproduced the map's central finding from a cold start, and
`TECHNIQUE.md`, written six weeks earlier from the private fork, says the same
two numbers:

> We built a run that reached the author's contact point to within **0.3 m**
> with a velocity within **3 m/s** of his, and nothing happened. Position
> doesn't trigger it. Speed doesn't trigger it. Which way the car is pointing
> does.

**Arm K is the feature working end to end**, on the map it was designed for,
from a cold start, with the whole objective written on the command line. The
band transitions are one line each in the log, and each one is the search
changing what it is optimising:

```
*** GATE key +1.1058                         <- the seed
*** GATE key +92.9766                        <- the state
*** GATE FIRED, after -298.7544              <- the launcher fires
*** GATE FIRED, after -19.9934                <- aimed at the finish
*** GATE and finished, 21.510                <- and across it
*** GATE and finished, 21.223
```

**THE RESULT HERE IS NOT A TIME, AND 21.223 IS NOT A RECORD.** This map is
already beaten: the project's incumbent is **20.237** and the author time is
**20.555**. Quoting 21.223 next to those invites exactly the wrong reading.

What arm K demonstrates is the instrument. **From the human world record as its
seed, a cold search found the launcher route that no human on the leaderboard
drives, and crossed the line on it, in one hour** — where the private fork,
hand-built for this map, took **2 h 43 min** to its first validated finisher on
the same route. 2 732 610 evaluations, 216 improvements confirmed by the plain
oracle, **zero phantoms**; every banked finisher re-simulates to exactly the
millisecond in its name from a fresh process, with the human world record
carried in the same batch and returning 22.637 exactly.

228811 was chosen precisely *because* it is solved: a map with known ground
truth is the only place an instrument like this can be shown to work rather than
merely to produce numbers. The 20.237 came from a further finish-time search
seeded from tapes like these, and nothing here attempts to reproduce it.

### 5.13 A finding about the map, not about the tool

**228811's published firing condition is NECESSARY AND NOT SUFFICIENT.**

`TECHNIQUE.md` and `RESULT-AT-BEATEN.md` both state the trigger as a
conjunction: body-lateral speed ≥ 85 m/s, crossing z downwards at ≥ ~17 m/s.
That was measured over 1343 launches and it is right about what every launch
has. It is not the whole condition, and two arms of this session reached that
from opposite directions:

* **Arm K** drove the conjunction to **side 98.2 with −vz 30.9 at
  (79.5, 50.1, 712.7)** — both components comfortably above their thresholds, on
  the deck, downstream of the checkpoint — and **nothing fired**.
* **Arm D**, aiming at the author's whole 6-D contact state, closed position to
  **0.29 m** and velocity to **3.60 m/s** and stopped **53.8° away in
  attitude** — which is the same wall `TECHNIQUE.md` describes hitting with the
  private fork ("within 0.3 m … within 3 m/s … and nothing happened").

The two arms used different objectives, different windows and different seeds
and landed on the same missing ingredient. That is what makes it a finding
rather than a null: the conjunction is a property every launch has, not a
condition that produces one, and the third term is the car's attitude.

**Arm K's own finish went around this, it did not solve it.** Once the event
clause was armed the search stopped optimising the conjunction and started
optimising *did it fire*, found a state that does, and never had to know why.
That is the correct behaviour for a search and a poor substitute for knowing the
trigger — anyone quoting the conjunction as the answer should quote this
alongside it.

---

## 6. What the search still cannot express

Concrete, in the order I would close them.

**1. Rungs along our own line.** `Progress::Checkpoints` only understands real
checkpoints, and on a map with four of them across 95 seconds that ladder has
almost no gradient. Dense rungs placed along the incumbent's own trajectory
(~1.3 s apart) are what made a sectional search work on 210218, and they were
built outside the search. This wants to be `--rung` plus the strictness switch
that arm needed: a DNF must not collect a depth bonus for a rung it never
fired, or the first wreck to stumble deep outranks every on-line tape.

**2. A drift bound on the fork.** The search now REPORTS how far a candidate
is from the fork's reference; it cannot BOUND it. `--max-drift N` would keep a
fork search inside the regime where the fork is known exact instead of relying
on the guard to catch it afterwards. Cheap, and it turns a property we monitor
into one we enforce.

**3. The fork clock fit is per map and this one is map 2's.**
`clock_for_tick` uses `clock = 36141 + 25.483 * race_ms`; another map fitted
`5431 + 26.49 * race_ms`, and using the wrong one there put a requested race
1.200 at race 4.325. Today that costs a checkpoint in the wrong place and is
visible (the tick the server actually stopped at is probed and printed), so it
is not a wrong answer — but `--forktick` means different things on different
maps. Two probes at two requested clocks solve the fit in about a minute.

**4. A keyboard-legal search.** Many of this project's published results have a
"keyboard" sibling, and restricting steering to a human-reachable alphabet is a
constraint the search should carry (`--alphabet`), not something to filter for
afterwards. This existed once, in the `lowinput` overlays, and never came back
into the maintained lineage — see [`../LINEAGE.md`](../LINEAGE.md).

**5. The banked file still declares the template's time.** Every file this
search writes says 23.013 and does 22.923, because a patched tape inherits its
template's header. `tmsearch validate` prints the disagreement, and
`ghost declare --from-oracle` fixes it, but the search should do it on the way
into the bank. That needs a library entry point in `ghost`; every other piece
is here. The whole "our run can be ours and the file somebody else's" family
starts with this field.

**6. `--start-from` demands an identical tick count.** A tape of a different
length cannot seed a search at all, though a tape can be lengthened. That
forecloses seeding from a sibling map's run, which is how one map's best lap
was found.

**7. A segment map is trusted, not checked.** Swapping a checkpoint gate for a
finish gate is not a faithful trigger — one map paid 0.206 s of phantom gain
for it — and the reference-ghost identity control cannot catch it, because the
reference line is inside both volumes. The search takes `--seg K:MAP` on faith.
It could at least require that the segment map return the reference's own split
for the reference tape.

**8. A SECOND event, chained.** `--fire` is one event. Map 210218's backward
chain wants a ladder of them -- reach here, then there, then there -- and the
shape is already right: an event clause whose `--fire-where` box is armed only
once the previous one has fired. That is a `Vec<Fire>` and an index in the
summary, not a new idea.

**9. `atan2`.** The key language has no inverse tangent, so "the angle of the
velocity off an axis, in degrees" cannot be written. `nose()`/`along()` give the
cosine, which is monotone in the angle over the half-turn that matters, and the
one old mode that wanted an angle was a decoy. One opcode if a map ever needs
it.

---

## 7. What I deliberately did not touch

* **The car locator (`forkoracle::blind`) and the shim's memory scanning.**
  Delicate, hard-won, and covered by `fk`'s own suite. I deleted two dead
  functions and two parameters that were accepted and ignored, and left the
  algorithms alone.
* **`fk`, `tools/ghost`, `tools/tmtraj`.** Other arms' crates. The one
  exception is `fk/src/cmd/watch.rs`, which had to follow the mutation
  operators when they moved into `forkoracle` — it builds, and the change is
  mechanical.
* **The sub-tick timing plane.** It is a gradient, never a score, and the
  search does not use it. The rule (`plane error ≈ spread of the crossing
  coordinate / speed`; 0.98 ms grounded, ~19 ms airborne) belongs with whoever
  arms it.
* **`analyze`'s statistics.** Ported as-is: operator and tick-bucket tallies
  plus the best-of-k curve. The tallies are what retuned a stalled search from
  its own log once; I did not add anything nobody has used.
