# SEARCH.md — the search and oracle-driver layer

The part of the toolchain that actually finds faster runs, and the one place a
result is allowed to leave it.

```
cd tools/search
cargo build --release
TM_SERVER=/path/to/TrackmaniaServer-dir cargo test --release    # 41 checks
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
| `--fork --forktick --refcsv\|--refghost --shim --pred --finishmargin --corridor` | the fast evaluator and its watchdog |

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
**checked in beside `tools/ghost`**, addressed by relative path, so a missing
one is a panic rather than a skip. The engine-dependent tests do skip, and say
so — and `TM_REQUIRE_ENGINE=1` turns that skip into a failure, so a box with an
engine cannot quietly stop running them.

### The controls

Everything below was run on this box, against the real dedicated server, with
the fixtures checked in beside `tools/ghost`.

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

## 5. What the search still cannot express

Concrete, in the order I would close them.

**1. A state objective.** When finish time cannot cross a valley, the thing to
score is the STATE at a place. Three maps have needed this and each one
hand-rolled it in a private fork of the search, which is the definition of a
missing feature. A sketch, because the shape matters more than the code:

* **A third outcome variant**, so the bands cannot overlap by construction the
  way the two ladders already cannot:

  ```rust
  pub enum Outcome {
      Finish { ms: i64 },
      /// Reached the place the search was pointed at, but did not finish.
      Reached { band: u8, key: f64 },
      Dnf(Progress),
  }
  ```

  with `Finish > Reached > Dnf`, and within `Reached`, band first and key
  second. The whole ordering discipline is already in `score.rs`; this is one
  more variant and one more test.

* **The gate is a `box` predicate that records instead of aborting.** The
  watchdog language already has `box`, the child already gathers the 44-byte
  record every tick, and `R_QUAT / R_POS / R_VEL` are all in it. What is
  missing is a slot in the 48-byte summary for *the whole record at first
  entry*. That is the cheap half.

* **The key must be a function of the WHOLE state.** On 228811 position and
  velocity together were not enough — attitude was the trigger. So the key is
  named over the recorded record (`speed`, `speed along a direction`, `distance
  to a point`, `slip angle`, a weighted sum), not over a fixed pair of fields.

* **The key must extend continuously OUTSIDE the box.** `-(500 + miss)`, never
  `-miss`, or a near miss outscores an arrival. This is band 0, and it is what
  gives a search that has never once fired the gate something to climb.

* **The identity control changes shape and gets stronger.** In gate mode the
  classic "does the fork reproduce the seed's millisecond" check is
  unavailable, and the replacement is better: the fork's measured gate state
  for the SEED must equal the seed's own decoded telemetry at that place —
  position, velocity *and* quaternion. One comparison that validates the record
  layout, the locate and the labelling at once.

* **Print the decoy test at startup.** An objective that can be maximised
  without achieving the goal is not a proxy, it is a decoy, and one map met
  three in a row. Before the first candidate, print the key of the incumbent
  and the key of the do-nothing tape. An objective the parked car scores well
  on is visible in the first line of output instead of after four hours.

* The guard is unaffected and still governs: a band-2 result is a finish and is
  re-validated like any other; a band-0 or band-1 result is a *state*, not a
  time, so there is no time for the oracle to contradict — the bank records the
  gate state beside the tape so the claim can be checked by hand, and the file
  never acquires a millisecond it did not earn.

**2. Rungs along our own line.** `Progress::Checkpoints` only understands real
checkpoints, and on a map with four of them across 95 seconds that ladder has
almost no gradient. Dense rungs placed along the incumbent's own trajectory
(~1.3 s apart) are what made a sectional search work on 210218, and they were
built outside the search. This wants to be `--rung` plus the strictness switch
that arm needed: a DNF must not collect a depth bonus for a rung it never
fired, or the first wreck to stumble deep outranks every on-line tape.

**3. A drift bound on the fork.** The search now REPORTS how far a candidate
is from the fork's reference; it cannot BOUND it. `--max-drift N` would keep a
fork search inside the regime where the fork is known exact instead of relying
on the guard to catch it afterwards. Cheap, and it turns a property we monitor
into one we enforce.

**4. The fork clock fit is per map and this one is map 2's.**
`clock_for_tick` uses `clock = 36141 + 25.483 * race_ms`; another map fitted
`5431 + 26.49 * race_ms`, and using the wrong one there put a requested race
1.200 at race 4.325. Today that costs a checkpoint in the wrong place and is
visible (the tick the server actually stopped at is probed and printed), so it
is not a wrong answer — but `--forktick` means different things on different
maps. Two probes at two requested clocks solve the fit in about a minute.

**5. A keyboard-legal search.** Many of this project's published results have a
"keyboard" sibling, and restricting steering to a human-reachable alphabet is a
constraint the search should carry (`--alphabet`), not something to filter for
afterwards. This existed once, in the `lowinput` overlays, and never came back
into the maintained lineage — see [`../LINEAGE.md`](../LINEAGE.md).

**6. The banked file still declares the template's time.** Every file this
search writes says 23.013 and does 22.923, because a patched tape inherits its
template's header. `tmsearch validate` prints the disagreement, and
`ghost declare --from-oracle` fixes it, but the search should do it on the way
into the bank. That needs a library entry point in `ghost`; every other piece
is here. The whole "our run can be ours and the file somebody else's" family
starts with this field.

**7. `--start-from` demands an identical tick count.** A tape of a different
length cannot seed a search at all, though a tape can be lengthened. That
forecloses seeding from a sibling map's run, which is how one map's best lap
was found.

**8. A segment map is trusted, not checked.** Swapping a checkpoint gate for a
finish gate is not a faithful trigger — one map paid 0.206 s of phantom gain
for it — and the reference-ghost identity control cannot catch it, because the
reference line is inside both volumes. The search takes `--seg K:MAP` on faith.
It could at least require that the segment map return the reference's own split
for the reference tape.

---

## 6. What I deliberately did not touch

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
