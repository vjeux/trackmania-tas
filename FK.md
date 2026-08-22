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
as "finished at 22.730". That value fed `race_end`, which decides which recorded
instants count as inside the race and therefore which samples a regenerated ghost
may inherit from its donor. Fixed by calling `ghost::oracle`, which parses the two
into separate fields. Pinned by a captured DNF from the real server.

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

**4. `r165_tools_v5.tgz` is two lineages in one tarball.** Its `tmsearch` is
pre-hardening — `FINISH_BASE = 100_000_000` in `main.rs`, `forksearch.rs` and
`bin/tmtas.rs`, no `claim_root`, no phantom guard — while its `fk` and `fkdrv`
have every reliability fix: per-pid `default_work_dir()`, the `.fkowner` lock,
`pipe2(O_CLOEXEC)`, `Drop for ForkServer`, `layout::verify_tape`. So restoring it
gives you **a hardened oracle driver and a broken search**, which is the worst
combination to have to notice, because the driver's own controls all pass. Check
the constant, not the version.

---

## 6. The suite

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
* **`engine_locate_fails_loudly_when_the_key_does_not_match`** — the brief's
  "fails loudly when the game binary moves". The same server, map and tape are
  started twice, once with the tape's own key and once with a key that describes
  nothing in memory. The negative alone would prove nothing — a run that aborts
  for an unrelated reason looks the same — so the positive control is half the
  test.
* **`steer_echo_matches_a_real_recording_byte_for_byte`** — and its own positive
  control, that the old encoding visibly fails on the same data. Its first
  version asserted that floor and round differ at exactly steer 0 and 60, which
  I had taken out of a write-up and turned into a claim about all 255 values
  without measuring it; they differ at 127 of them. It now measures.

---

## 7. Gaps worth closing

See the report. In short: `--inputs` should not be a flag; the resume boundary
should be per-server everywhere rather than only in the search; there is no
command that answers "is this checkpoint inside the validated regime"; 91 of each
sample's 116 bytes are still the carrier's and the engine has all of them; and
the search still carries the second codec.
