# FK.md — the oracle driver

`fk` drives the TM2020 dedicated server as a physics oracle: it stops the
simulation mid-run, resumes it with different inputs, and reads the car's own
state out of the running engine.

```
cd tools/fk && cargo build --release
cargo test --release                                   # 15 checks, ~1 s
FK_ENGINE=1 TM_SERVER=/path/to/server cargo test --release   # + the real engine
```

Times print as **seconds with a decimal** (`22.730`), never as raw
milliseconds.

---

## 1. What `fk` is for, and where its edge is

Three things need a live engine, and they are the whole of `fk`:

1. **Stop and resume.** `lroundf` is called ~25.5 times per simulated
   millisecond and nothing else in the simulation is non-deterministic, so an
   `LD_PRELOAD` interposer can count calls and halt at an exact point.
   `fork()` from there is a complete simulator at ~11 ms instead of a
   from-scratch run.
2. **Read the car's state.** Position, orientation and velocity per 10 ms tick,
   located by value at every server start because the heap layout is bimodal
   run to run.
3. **Watch a run inside the fork child and abort it early.**

Everything about a **file** — the input chunk, the telemetry record, the
declared time, the carried map, identity — belongs to `ghost` and `fk` calls it.
That boundary is not a preference: `fk` used to carry the second implementation
of the `0x0309201D` codec, and two implementations of one file format is how
this project got silent corruption before.

### THE REGIME LIMIT

The fork server was exact on **4700 of 4700** candidates — every one of which
perturbed a human reference tape by a few ticks at a checkpoint 48–99 % through
the run. Outside that regime it lies: on cold-start work, **0 of 312**
fork-reported finishes survived a full `/validatepath` of the byte-identical
bitstream. One tape gave DNF from boundary 170 and a finish from boundary 305
with the same inputs, and the file was a DNF.

A fork-reported time is a **measurement**. Only the plain oracle, on the file as
written to disk, is a **result**.

---

## 2. The commands

| command | what it answers |
|---|---|
| `fk server probe` | where did this server actually stop, and which tick is safe to rewrite? |
| `fk server check` | does a resume give the same answer as a full validation? **THE CONTROL** |
| `fk server bench` | how much faster is it, against a batched baseline? |
| `fk trace` | the car's own state per tick, as the 29-column CSV `tmtraj decode --csv` writes |
| `fk watch measure` | the early-abort watchdog: exactness, false positives, speedup |
| `fk watch replay` | the same evaluator with no server, against a trajectory CSV |
| `fk watch paths` | do the two in-child sampling paths judge identically? |
| `fk regen` | rewrite a ghost's telemetry from engine state (the engine half of `ghost regen`) |
| `fk carrier` | name the sample bytes a regenerated ghost inherits, and write them — see [CARRIER.md](tools/fk/CARRIER.md) |
| `fk ptr` | the engine's own pointer to the car — find it, check it; see [POINTER.md](tools/fk/POINTER.md) |

Engine flags, accepted by every command: `--tape`, `--map`, `--server`
(`$TM_SERVER`), `--shim` (`$FK_SHIM`), `--work`. Checkpoint: `--at tick:N`,
`--at clock:N`, `--at frac:F`.

`fk regen` and `fk watch` take `--template` rather than `--tape` and choose
their own checkpoints from a ladder. That is a real difference, not an
inconsistency: `regen` must run the ORIGINAL FILE verbatim, and `watch` measures
over a window rather than at a checkpoint you name.

### `fk server probe` is worth a command of its own

Because the answer is **not the same on two servers started the same way**.
`lroundf` is bit-identical only on an idle box; under load the count moves in
whole chunks of ~62 calls (a wall-clock catch-up branch), and 62 calls is about
a quarter of a tick. Measured: 5 of 400 quiet starts stopped one tick later than
the rest, and **104 of 150 workers did when 150 servers start at once**.
Anything derived from where a server stopped is per-server.

### `fk server check` is the acceptance test

It runs five things and each fails the run on its own: the identity resume, the
page-fault probe (hard abort, never a fallback), the boundary calibration, the
oracle's own repeatability on the same candidate set, and fork-vs-full
exactness including the DNFs.

Run it once before trusting a new map or fork configuration — and run a stress
window, not the production one. The production window gave **0 phantoms in 289
banked tapes**, which is exactly why the resume-boundary defect survived four
investigations. A quiet run is not a clean one.

---

## 3. The audit: 39 subcommands to 8

`fk` had 39 top-level subcommands (plus sub-modes inside `fs`, `state`, `pred`
and `whl`), 12,325 lines across 23 files, and **no tests**. Here is every one of
them and what happened to it.

### Load-bearing — kept

| was | is | used by |
|---|---|---|
| `fs --mode auto/test` | `server check` | the acceptance test for any new map |
| `fs --mode cal/edge` | folded into `server check`'s boundary sweep | — |
| `fs --mode bench` | `server bench` | throughput claims |
| `fsprobe` | `server probe` | the load studies |
| `btraj2` | `trace` | `tools/README.md` C-route; the C11b sweep |
| `pred --mode audit/offline/equiv` | `watch measure/replay/paths` | the watchdog's own controls |
| `regen` | `regen` | `ghost regen` calls it; the render pipeline |
| `clean` | folded into `fk::record` | it was only ever `regen`'s recorder |

### Moved to `ghost`, which owns the file format

`cand`, `layout`, `stats`, `tapeinputs`, `tapediff`, `tapecsv`, `tapeswap`,
`tapecut`, `graft`.

* `tapeswap` = `ghost tape extract` + `ghost tape inject` (whole packet list,
  bit for bit).
* `tapecut` = `ghost trim`.
* `graft` is the same operation done worse: it patched steer/accel/brake in
  place through the old codec and left the container's `state_seg`, `mouse_seg`,
  `mode` and `tri` alone. That is why the 165922 graft DNF'd at cps=1 while the
  source validated to the exact millisecond.
* `cand` / `layout` / `stats` = `ghost tape inject` / `ghost inspect` /
  `ghost tape stats`.
* `tapeinputs` / `tapediff` / `tapecsv` were `tmtraj intg`'s only reason to shell
  out to `fk`; they are `ghost tape` operations.

### One-off probes — deleted, having answered their question

| deleted | the question it answered, which is now a constant or a document |
|---|---|
| `memfind`, `memmap`, `locate`, `poke`, `dump`, `diff`, `diff3`, `cmp`, `scanseq`, `inputs` | *where does the engine keep the decoded inputs?* One 32-byte record per 10 ms tick, in tick order: `+0` f32 steer = `i8/127`, `+4` gas, `+8` brake, `+12` 0, `+16` device-segment const, `+24` 2, `+28` packet mode. Exactly one copy in the address space. **Heap diffing does not work** — two identical runs can differ by 87 MB — which is why `dump`/`diff`/`diff3` were dead ends and are not worth rebuilding. |
| `state --mode locate/track/counter/fields` | *where is the car?* `P-16..P-4` = qw,qx,qy,qz · `P+0..P+8` = x,y,z · `P+12..P+20` = vx,vy,vz. The race clock is a u32 advancing by exactly 10 per tick, **not at a fixed offset** from the position (P-7916 / P-11268 / P-14780 on three runs) — locate it behaviourally, never by offset. |
| `velscan` | *which slot is the velocity?* `P+12`, best match to d(pos)/dt in ±16 KB (0.58 m/s; runners-up 40 m/s). Components match the ghost's own only to 0.4–0.9 m/s, but the extracted velocity is MORE self-consistent with its own positions (0.94 / 0.24) than the ghost's is with the ghost's (1.44), so that is the telemetry's encoding, not the readout. |
| `obs` | *does an independent ManiaScript readout agree?* Partially, and the comparison was never available whole because `observe`'s `mkinputs` re-bases the tape, so the two input streams are not the same tape. |
| `verify`, `traj` | the reference-matched acceptance test. It is now a **test**, not a subcommand: `engine_trace_lands_on_the_reference_path`. |
| `btraj` (the v1 blind locator's driver) | superseded by the clock-first locator. The v1 locator itself stays in `forkoracle::blind`, because the SEARCH calls it on every candidate. |
| `traj2` | a third trajectory driver differing only in how it was invoked. |
| `arc`, `sweep` | map-specific state-matching drivers for 284238. Their finding is worth keeping and their code is not: **the launch is a trilemma** — an arc steer delta buys any two of (cross far enough / cross early enough / still be fast) and never all three, because turning early scrubs. The human pays for the turn out of a RISING speed profile (82.9 → 92.0, on the gas through the arc); our record pays out of a falling one (82.0 → 76.2). Two channels have to move together and no time-based objective can see any of it. |
| `fields`, `fit`, `probe`, `grade`, `whl`, `fieldmap` | see below — the largest deletion. |

### The field-map apparatus: ~3,300 lines that were not on any live path

`fk fields` swept the whole writable address space against a real ghost's own
recorded columns. `fk fit` turned "this slot correlates with rpm" into an
encoding. `fk probe` printed the near misses so a silence could not read as an
absence. `fk whl` found the wheel-rotation block — four f32 at stride 44, each
accumulating distance / one shared radius, |corr| > 0.9999 — so the offsets
would be a property of the GAME rather than of the run, which mattered because
two runs of the same map land on different copies of the car state and an offset
from the position anchor reproduced rpm on 0.2 % of samples on a second map.

It worked. Sample byte 5 is `round(0.008489 * slot@pos-240812)`, exact on 439 of
474 recorded samples and within one quantisation step on all 474.

**Both halves of that sentence have since been reproduced and both were half
right** (`fk carrier`, see CARRIER.md). The scale is real — the independent fit
lands on 0.0085 for the same byte — but byte 5 is the HIGH HALF of a 16-bit rpm
at bytes 4,5, and read as one `u16` it is exact on 96.9–100 % rather than 92.6 %.
And `pos-240812` was an offset from an anchor, which is why it reproduced on
0.2 % of samples on a second map: the same field is at **car+328**, and the
difference between "the anchor" and "the car" is the whole reason the old
offsets did not transfer.

**And nothing used it.** Every production recipe in this project runs
`fk regen --fieldmap none`, or with a "neutral map" that is nothing but a list
of 49 byte offsets to zero. The measurements are above and in the source of
`record::NEUTRALISE`; the 3,300 lines are gone. `--fieldmap` is replaced by
`--neutralise`, which needs no file.

### Deleted because it was retracted

`fk regen --recshift`. It shifted the pairing between engine instants and record
instants by a whole tick, on the strength of C11b reporting every regenerated
file as a clean `speed × 0.010 m` stale-buffer offset. Nine files were rebuilt
on it. The measurement was right and the conclusion was wrong: C11b reports a
MAGNITUDE, so it cannot see which side of the tick a file is on, and a
DOWNLOADED human ghost the game recorded itself reads exactly the same
(267460 human WR 0.4538 m at 45.42 m/s = 10.004 ms, 98 % tick-shaped; 227969
human WR 1.1931 m at 119.34 m/s = 10.022 ms, 100 %). The offset is also per-map
(−10 on 267460/227969, 0 on 203072), so no constant could have been right.

The general form, which is in the source where the flag used to be: **a negative
result requires a positive control, including when the negative agrees with a
measurement you made yourself and liked.**

### `--need-wheels`

Deleted with `whl`. It was only ever a precondition for a wheel-anchored field
map, and there are none.

---

## 4. Internals

### The dependency knot, untied

`fk` depended on `tmsearch` for `ghost::Factory`, `replay::Replay`,
`oracle::Worker` and `gbx::lzo_init` — i.e. on the SEARCH, for a FILE FORMAT.
That dependency is why `fkdrv` had to exist at all: the search and `fk` both
needed the fork-server driver, and with `fk → tmsearch` the shared code could
live in neither.

`fk` now depends on `ghost` and `tmtraj`. `forkoracle` (was `fkdrv`) stays,
because the cycle is not the only reason for it — the search runs `forksrv`,
`pred`, `pred_core`, `layout`, `blind` and `procmem` on every candidate, and
`pred_core` is `#[path]`-included by both the driver and the `LD_PRELOAD` shim
so a predicate has exactly one definition on both sides of the fork. The
clock-first locator (`locate2`) came the other way, into `fk`, because nothing
in the search calls it.

### The god-struct, removed

Every one of the 39 subcommands took the same `Cfg`: twenty-four fields
(`template map server work shim csv tick ckpt tol mode out every n addr span len
tape difftick obs obstag steerdiv diffmag nth`), parsed by one function, of which
any given command read four or five. `--obstag` was accepted by the memory
poker; `--difftick` by the tape cutter. A flag being accepted said nothing about
whether it did anything.

What is actually shared is five things — which engine, which map, which tape,
where to work, where to stop — and that is `session::Engine` +
`session::Checkpoint`. Everything else is an argument to one command, parsed by
that command. Unknown flags are now an error rather than either a panic or
silence depending on which command you were in.

### Modes behind a flag, replaced by verbs

`fs --mode auto|edge|test|bench|cal|scan`, `state --mode locate|track|counter|fields`,
`pred --mode audit|offline|equiv`. Three of `fs`'s six modes were subsets of
another reachable only by reading the source. A `--mode` that changes what the
command *is* should be a verb, and now is.

### The identity control is no longer optional

`Session::start` runs `layout::verify_tape` — read the engine's own decoded input
array back out of `/proc/<pid>/mem` and compare it tick for tick — before
returning, and there is no flag to skip it. It costs one 70 KB read. It is the
only thing that can see a swapped replay: two runs sharing a work directory
produce a genuine, self-consistent trajectory of the wrong car, and it happened
on 17–35 % of profile refreshes in production.

---

## 5. Defects found while doing this

**1. The oracle parser reported the file's own claim on a DNF.** The server
prints two results per file: `ValidatedResult` (what it simulated) and
`DeclaredResult` (what the file claims). `fk`'s regen path read *the first line
starting with `"Time"`*, which is correct only by the order the server happens to
print in — and on a DNF `"ValidatedResult" : null` carries no `Time` at all, so
the first one is the declaration. A run that reached 2 of 3 checkpoints read back
as "finished at 22.730". That value became `race_end`, which decides which
recorded instants count as inside the race and therefore which samples a
regenerated ghost may inherit from its donor. Fixed by calling `ghost::oracle`,
which parses the two into separate fields. Pinned by a captured DNF from the
real server.

**How far it actually reached — and it is fail-safe, for a reason worth having
written down.** The guard downstream is *is any IN-RACE instant missing an
engine instant*, and a DNF makes instants missing under either window:

* correct (`race_end = None`) → the code treats **every** recorded instant as
  in-race, prints `WARNING: no finish time known`, finds the instants past the
  point the clean run reached have no engine data → **ABORT, no file written**;
* buggy (`race_end = the declared finish`) → instants between the DNF point and
  the declared finish are still in-race, still have no engine data → **ABORT, no
  file written**.

Both abort. It stops being fail-safe only under `--allow-partial` or
`--inherit-outside`, whose entire purpose is to say "yes, write a part-carrier
file", and both print that on their own line. **What was lost is the
diagnostic**: the operator never saw the warning, and the printed `validator
Time` line asserted a finish for a run that did not finish. On a transplanted
container — 165922's carrier declares 8787.035 against a 15.085 run — that line
would have read 8787.035 and looked like a container problem rather than a
clean-run failure.

**Was any published ghost regenerated through it? No known one, and this is the
artefact.** `whl_regen_corpus.sh` keys every file on `nan_validate_after_v1.tsv`
and skips anything without a validated time (`[ "$ms" = "DNF" ] && SKIP`). That
table has 171 rows, 3 of them DNF, and those 3 were skipped by construction — so
no corpus regeneration ever ran on a template the oracle refused.

**The residual exposure I could not close.** A clean run that DNF'd for a
*run-specific* reason — a bad map link, a shared work directory, a handover that
perturbed the run — on a file that validates perfectly well on its own. The
banked `tg_regen_results_v1.tsv` cannot distinguish that case: a finishing run
and a fabricated window print the same `Some(N)`, because by then N is the same
number. **The artefact that would settle it is the raw server output of each
clean run, and it is not banked.** The cheap way to settle it is to re-run
`ghost regen`'s gate over the published corpus with the fixed binary and diff the
coverage lines; the expensive way is to re-regenerate.

**2. The input echo was a `round` where the game writes a `floor`.**
`fk regen --inputs` wrote `round((steer_i8 / 127 + 1) / 2 * 255)` into sample
byte 14. The game writes `floor((steer_i8 + 127) * 255 / 254)`. Measured on a
regenerated map-2 ghost: `ghost verify` V6 kappa **0.467 before, 1.000 after**,
100 % of 455 samples exact. It was invisible because the only consumer of those
bytes is the contamination detector, and a file that fails it looks like a file
with a contaminated RECORD rather than one with a mis-encoded ECHO — and because
`ghost regen` had already been forced to rewrite the channels itself afterwards.

**3. The old codec could not write every tick.** It emitted mode-12
same-as-previous packets as a one-bit form and silently dropped writes to them.
Template `seed_rank10000` has three (ticks 0, 1, 2). It never bit because those
ticks sat below every resume boundary — a property of the boundaries we happen to
use, not a guarantee. `ghost::tape` expands them; a test writes a distinct value
into all 2432 ticks and reads them all back.

**4. Two API defects that only appear when you RUN the command.** `fk watch`'s
`--template` defaulted to `/tmp/spot/inc.Ghost.Gbx` and its `--map` to
`/tmp/m2/map2.Map.Gbx` — one agent's scratch paths. A missing flag therefore ran
a whole measurement **against somebody else's incumbent** instead of failing.
That is the same failure family as everything else in this list: it fails toward
"it worked". In a swarm it is a correctness bug, not an ergonomics one. And
`fk watch replay` read its input from a flag called `--out`, named for the
workflow that usually produces the file rather than for what the command does
with it.

Neither is visible in the source. Both appeared in the first minute of using the
commands. **Which is the argument for running everything.**

**5. The equivalence control was reporting a diagnostic as a verdict.**
`fk watch paths` asks whether the two in-child sampling paths reach the same
answer. It counted `off_max` — a reported diagnostic that nothing decides on —
as part of "the same verdict", and the fast path evaluates one more tick than
the slow one on a run that reaches the end, so `off_max` grows by construction.
The control read **"2 of 8 REALLY DIFFER"** on eight candidates whose every
verdict was identical. A control that cries wolf gets ignored, which costs more
than the thing it was guarding. The verdict is now the finish time, the
checkpoint count, the trip (predicate, tick, value) and the progress; `off_max`
and `travelled` are compared under "identical in every field" instead.

**6. `r165_tools_v5.tgz` is two lineages in one tarball.** Its `tmsearch` is
pre-hardening — `FINISH_BASE = 100_000_000` in `main.rs`, `forksearch.rs` and
`bin/tmtas.rs`, no `claim_root`, no phantom guard — while its `fk` and `fkdrv`
have every reliability fix: per-pid `default_work_dir()`, the `.fkowner` lock,
`pipe2(O_CLOEXEC)`, `Drop for ForkServer`, `layout::verify_tape`. So restoring it
gives you **a hardened oracle driver and a broken search**, which is the worst
combination to have to notice, because the driver's own controls all pass. Check
the constant, not the version.

---

## 6. The suite, and what the commands measure

### The suite

15 pure checks and 3 engine checks, one command. The fixtures are `ghost`'s —
`tools/ghost/testdata/` already holds anonymised ghosts, a replay carrying its
own map, and a map, and a second copy of them here would be one more thing to
keep in step. What lives in `tools/fk/fk/testdata/` is only what is `fk`'s: two
captured dedicated-server outputs, and one reference trajectory.

The three that are worth reading:

* **`oracle_reports_what_was_simulated_not_what_the_file_claims`** — the fixture
  simulates 22.754 and declares 22.730. **No fixture where the two agree can ever
  fail this test**, and until now every fixture anyone had was a passing file
  where they agree exactly.
* **`engine_locate_fails_loudly_when_the_key_does_not_match`** — the locate must
  refuse when the game binary moves rather than return garbage. The same server,
  map and tape are started twice, once with the tape's own key and once with a
  key that describes nothing in memory. The negative alone would prove nothing —
  a run that aborts for an unrelated reason looks the same — so the positive
  control is half the test.
* **`steer_echo_matches_a_real_recording_byte_for_byte`** — and its own positive
  control, that the old encoding visibly fails on the same data. Its first
  version asserted that floor and round differ at exactly steer 0 and 60, which
  I had taken out of a write-up and turned into a claim about all 255 values
  without measuring it; they differ at 127 of them. It now measures.

### THE SCORE-SAFETY INVARIANT

The watchdog is safe to put in a search because of one property, and the search
should depend on it by name:

> `progress(aborted candidate) <= progress(the same candidate with nothing
> armed)`

Progress is a max over ticks and aborting only removes ticks, so arming can only
LOWER a score — and therefore **a dead candidate can never displace a live
one**. It is not an argument, it is checked per candidate in every
`fk watch measure` run: 24 of 24, 0 violations on the run below, 2000 of 2000 in
the original audit.

### Say how far the candidates were from the reference, or the numbers do not
transfer

Every exactness number the fork server has produced is a number about a
**regime**. `fk watch measure` and `fk server check` both now report
`Distance` — earliest divergence, how many ticks differ, largest steering move —
because "0 false positives" without that is a number that cannot be applied to
anything.

It earned itself the first time it ran. On the audit below the line reads
*earliest divergence tick 172 (race 0.140), median 36 of 2432 ticks differ,
worst 1607* — so **a tick-60 watchdog audit is not in the late-perturbation
regime at all**; its candidates diverge from the fourteenth of a second and one
of them differs on two thirds of the tape. That is the regime where cold-start
work found 0 of 312 fork-reported finishes surviving a full validation.

On this run the fork agreed with the full validation on all 24 anyway. **One run
of 24 is not evidence against a 312-case failure**, and the two are not in
contradiction: the cold-start finding was about reported FINISHES, and 17 of
these 24 are DNFs. The point is not that either number is wrong. It is that
until now neither was reported next to the distance that makes it mean
something.

### What one `fk watch measure` run reports

24 candidates, tick-60 checkpoint, map 2, two predicates armed:

```
IDENTITY CONTROL  unarmed 22.730  armed 22.730  reference 22.730  PASS, no trip
EXACTNESS         10 of 10 non-tripping candidates identical armed vs unarmed
                  0 disagree with the full validation
                  0 perturbed by watching alone
TRIPS             14 of 24 aborted (58.3%); 0 of the 7 that would have finished
FALSE POSITIVES   0 / 24
SCORE SAFETY      24 of 24, 0 violations
THROUGHPUT        1.269x vs the observing control, 2.626x vs full validation
                  aborted candidates stopped after 52% of the tail
```

The identity control is the row that makes the rest mean anything: the reference
tape through both paths must return the reference's own millisecond and must not
trip.

### The other measurements, as run

* `fk server check` — 12/12 exact at a 95.6 % checkpoint, oracle repeatability
  0 of 12, **15.5×** against a batched baseline. The *batched* part matters:
  nearly all of a validation's cost is the server launch, so a
  one-file-at-a-time baseline would inflate the speedup by most of itself.
* `fk trace` — median **0.0068 m** against the reference ghost's own telemetry
  over 2259 compared ticks, p90 0.0150, max 0.373, 99.34 % within 5 cm; whole-run
  self-check |q|−1 p99.5 1.25e−7, 0 clock gaps.
* `fk watch replay` — the reference line against itself: `off_max` **0.00 m**
  over 455 ticks. That is the positive control for the nearest-point tracker,
  which measured 1123 m of apparent deviation for the reference against itself
  before it was changed from a hill-descent to an argmin over a small window
  with ties broken to the later index.
* `fk regen` — 455 of 455 samples regenerated, 100 % coverage, and the written
  file re-simulates through the plain oracle to its declared 22.730.

---

## 7. Gaps worth closing

Concrete, and each one is a thing that is missing rather than a thing that is
hard.

### G1. The fork can express three input channels. The tape has four.

`forksrv::Rec` is `{ steer, gas, brake }` — three `f32` written into the
engine's 32-byte per-tick record. **A respawn is an editable input** (bit 31 of
the state literal) and `ghost::tape` models it as one, but a fork resume cannot
toggle one. On any map where a respawn is part of the fast line, the search is
not searching that dimension — not because anyone decided against it, but
because the harness has no word for it.

The four bytes are accounted for and none of them is it: `+12` is `f32 0`,
`+16` a device-segment constant, `+24` the value 2, `+28` the packet mode. So
the next step is to find where the engine keeps the respawn input, or to
establish that it is consumed somewhere other than this array. **That is a task,
not a conclusion.**

### G2. Nothing scores a candidate below a millisecond, safely

The finish is adjudicated to 1 ms and the search spends most of its time on
plateaus that are one millisecond wide. Two levers exist, and neither is in `fk`:

* the **sub-tick plane** is a GRADIENT, not a score. Measured: 0.98 ms error on
  a grounded finish, ~19 ms on an airborne one, where it fabricated a 7.990 that
  was really 8.004. Per-seed calibration is exact (residual 0.002 ms), which is
  what makes it dangerous — every internal check passes.
* the **gate-relocation vernier** does not have that failure mode, because a
  relocated gate is still adjudicated by the real trigger against the real car
  body, and it resolves 0.05 ms. It lives in `tmmaps` and costs a plain-oracle
  run per rank.

Scoring against a relocated gate *inside the fork* would turn the vernier from
an offline ranking pass into the search's objective. It is the largest single
lever on a millisecond plateau.

### G3. Nothing measures whether a checkpoint is inside the validated regime

`fk server check` generates its candidates by perturbing the reference — which
is precisely the regime the 4700/4700 already covers. The regime where the fork
lied on 312 of 312 is a tape that differs from its template *early or
structurally*, and nothing measures that.

Concrete: `fk server check --like TAPE`, deriving the candidate set from how
*that* tape differs from the reference (where the first difference is, how many
ticks differ, how far below the boundary they sit), so the exactness number
covers the regime you are about to rely on. `phdiag prefix-audit` was a version
of this as a screen — 11/11 recall, 11/34 precision — and it should be a
first-class control rather than a tool somebody remembers.

### G4. The per-server resume floor cannot be measured here

`fk server probe` starts one server and reports one probe tick. The defect
behind most of map 2's phantoms is that the tick is **per-server**: 104 of 150
workers stopped later than a single calibration when 150 servers start at once.
The fix — floor = MAX over workers, behind a startup barrier — lives in the
search, and a fleet has no way to ask what its floor should be.

`fk server probe --servers N` would start N at once and print the distribution
and the floor. Forty lines, and it turns a rule of thumb into a measurement.

### G5. CLOSED — the readout was 40 bytes wide and is now 1.25 MB

*Was: twelve of the twenty-nine trajectory columns are empty, and two of them are
what a landing is made of.* The claim in this section was that `gear`,
`rpm_raw`, `side_speed`, `is_turbo`, `turbo_time` and the four wheel-dampen
columns were **not unavailable, merely unread** — that the engine computes every
one of them and nobody had widened the window.

That was right, and `fk carrier` widened it. **Thirty of the 91 inherited
sample bytes are now written from engine memory**, each confirmed on eight
recordings across six maps with frozen coefficients and no refit:
`side_speed`, `rpm` (a 16-bit field the corpus census had refuted), all four
wheel rotations, all four suspension-travel bytes, all four ground-material
bytes, `gear`, `is_turbo`, and a 16-bit quantity at bytes 0,1 that nothing had
named. Result, method and the
enumerated remainder: **[tools/fk/CARRIER.md](tools/fk/CARRIER.md)**.

**`is_ground_contact` (byte 89) is NOT among them and was not attacked.** Three
arms have failed on it from three directions — an ±8 KB threshold search (+2.2
points over a constant), a record-internal suspension rule (+24.5 on its own key
and NEGATIVE on two of four others), and an engine-dump fit (17.8 points WORSE
than a constant). It stays closed. Landing quality still has no contact signal;
it now has four suspension channels at 100 %, which is the next best thing and
is what a viewer sees.

### G6. `--inputs` is a flag and should not be

Writing the tape echo into a regenerated sample is always right: those three
bytes are the run's own inputs and need no engine reading at all. Leaving it
optional means a regenerated file can silently disagree with the tape it
carries, which is the cheapest contamination check there is. Make it
unconditional; add `--no-inputs` only when someone can name the case.

### G7-adjacent, CLOSED: the field gather no longer searches for the car

*Was: the carrier fields come from a 1.25 MB window swept at every 4-byte
offset, 1.36 GB of disk per regeneration.* The engine keeps its vehicles in an
array of four objects reachable from a single global, with
`CSceneVehicleVisState` a member of each at +0x848, so the gather reads a
pointer and takes 864 bytes. **Measured: byte-identical output, and the disk
traffic of 24 parallel regenerations goes from 8.86 GB to 0.12 GB.** Result,
method, controls and how to recalibrate on a new build:
**[tools/fk/POINTER.md](tools/fk/POINTER.md)**.

The locate the CLEAN run does is untouched, so G7 below stands as written.

### G7. Nothing measures how often the locate picks a decoy on a given map

The ghost arm measured about 1 in 8 on its fixture map and responded by running
a dozen attempts at once. `fk regen` has an anchor ladder and retries, but there
is no way to ask "on this map, what is the success rate" — which is the number
that decides whether a corpus regeneration takes an hour or a day.

### G8. The search still carries the second codec

`tmsearch/src/{ghost,replay,gbx,bits}.rs` is a complete second implementation of
the `0x0309201D` codec, and it is the copy with the defect: its writer emits
mode-12 same-input packets as a one-bit form and **silently drops writes to
those ticks**, so the search cannot express certain candidates and nothing can
tell that it failed to. `fk` no longer uses it. Moving `tmsearch` onto
`ghost::tape` is out of this audit's scope and is ruled to be done.

---

## 8. Migrating off the old command line

There are no callers outside this project, so nothing was preserved for
compatibility. Anything driving the old `fk` needs these substitutions:

| old | new |
|---|---|
| `fk fs --mode auto` / `--mode test` | `fk server check` |
| `fk fs --mode bench` | `fk server bench` |
| `fk fs --mode cal` / `--mode edge` | nothing — `fk server check` calibrates and reports the boundary |
| `fk fsprobe` | `fk server probe` |
| `fk btraj2` | `fk trace` |
| `fk btraj` / `fk traj` / `fk traj2` | `fk trace` (one trajectory command, not four) |
| `fk clean` | `fk regen` — it was only ever `regen`'s recorder |
| `fk pred --mode audit` | `fk watch measure` |
| `fk pred --mode offline --out CSV` | `fk watch replay --trajectory CSV` |
| `fk pred --mode equiv` | `fk watch paths` |
| `--template` (on the fork-server and trace commands) | `--tape` — `regen` and `watch` still say `--template`, because they run the file rather than perturb it |
| `--tick N` / `--ckpt N` / `--frac F` | `--at tick:N` / `--at clock:N` / `--at frac:F` |
| `--ref CSV` (watch) | `--reference CSV` |
| `--reftime MS` | `--reference-ms MS` |
| `--out CSV` as `watch`'s INPUT | `--trajectory CSV` |
| `fk regen --fieldmap none` | drop the flag |
| `fk regen --fieldmap <a zero-list TSV>` | `--neutralise` (the list is built in) |
| `fk regen --fieldmap <a slot-mapped TSV>` | gone; see §3, nothing ever wrote one |
| `fk regen --recshift MS` | gone; it was retracted (§3) |
| `fk regen --need-wheels` | gone with `fk whl` |
| every tape and container command | `ghost` — see GHOSTS.md |

`--at frac:F` also changed meaning for the better: it used to be F of a line
fitted on three segment maps of one ghost, and it now MEASURES the run's real
`lroundf` total, so it means F of the run on any map. It costs one extra
validation (~0.5 s).

### The shell scripts

The old workspace carried `build.sh` and nine `whl_*.sh`. None is carried here.
`build.sh` was `cargo build --release` with an `ls`. The `whl_*.sh` scripts
drove `fk whl` / `fk fit` / `fk probe` corpus runs and go with the commands they
drove. Three per-map pipelines in the toolchain tree (`r165_regen_v4.sh`,
`r165_ticklate_repair.sh`, `r165_c11b_sweep.sh`) call `fk regen` and will need
the substitutions above; their other steps are all `ghost` and `tmtraj`
commands now, and they should be rewritten as such rather than repaired.
**There is no shell script in `tools/fk` or `tools/forkoracle` and there should
not be one.**

---

## 9. What I could not make safe

* **`fk regen --allow-partial` and `--inherit-outside`** are the only paths
  where a wrong race window is not fail-safe. They exist to write a
  part-carrier file on purpose and they say so on their own line, but they are
  the sharp edge left in that command.
* **`fk watch replay` and `fk watch paths` are ported but only compiled, not
  run.** `fk watch measure` is exercised. The other two verbs are the
  cross-checks on the in-child evaluator and on the two sampling paths, and I
  have not re-run either since moving them.
* **`forkoracle::blind` has no test.** It is the locator the search runs on every
  candidate, and it came with none; I added tests for the clock-first locator
  (`fk::locate`) because that is the one `fk` drives.
* **The `--anchor` calibrated-anchor escape hatch** is carried from the ghost
  arm's patch and I have not exercised it. Its acceptance test still runs, so a
  stale calibration should fail rather than produce a wrong file — but "should"
  is doing work in that sentence.
* **`ghost regen`'s `write_input_channels`** now duplicates what
  `fk regen --inputs` does correctly. The ghost arm has settled and cannot drop
  it. Recommendation for whoever next touches that path: keep it as a belt until
  someone re-runs the corpus, then remove it.

