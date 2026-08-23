# 134672 KEKL- SAUSAGE ICE — the author time is not 8.6 s of driving, and the 2022 field is not our car

Arm `ksi2`, node 48080.od.fbinfra.net, 2026-08-22. Times as seconds.
Nothing was submitted to any Nadeo leaderboard.

**Map record improved: 67.319 → 67.200** (`ksi2_67200_watchable_v1.Ghost.Gbx`,
md5 `baa76201ea8095fe32b7691b312c5774`). All of it is the closing sector —
5.616 → **5.497** — with CP1–CP4 unchanged to the millisecond. Validated on the
plain oracle twice, from two processes, against two separately obtained copies
of the map, with rank02 (68.442) and rank13 (94.940) exact in the same batch.
The author time is 58.687 and is not beaten.

**Headline, in one line:** the route is worth far more than the author time —
the field's own per-10 m best on this route is **50.978** against an AT of
**58.687** — and the reason nobody gets near it is not the route and not a
missing shortcut: our TAS is **3.773 s slower than a human already drove this
map**, and the human who drove it was on a **build whose car turns more than
ours does**, which is now measured rather than asserted.

---

## 1. There is no shortcut, and the cell census was the wrong test

The README's argument that "there is no secret route" counts *undriven cells*:
99 of 117 drivable surface cells have been driven by some record. That is the
wrong question. **A shortcut does not need an undriven cell — it needs to SKIP
driven ones.**

`tmtraj geom selfcut` asks the right one: every place the line comes back near
itself after a long interval, which is every cut the geometry offers.

Over the whole 2615 m lap, at a 40 m bar, there is **exactly one**:

| from | to | saves | gap | Δy | verdict |
|---|---|---|---|---|---|
| 34.600 | 55.200 | 20.600 | 9.73 m | −1.84 m | **skips CP3 — void** |
| 60.500 | 67.250 | 6.750 | 53.67 m | −1.36 m | **skips CP4 — void** (at an 90 m bar) |

Both folds of the sausage cross a checkpoint. Nothing else on the lap comes
within 90 m of itself except the trivial 2 s neighbours of the current point.
**The route is forced, and its length is forced with it**: every clean run in
the field travels 2615–2623 m (`tmtraj geom path`; the four runs that travel
2650–3850 m are the ones that spun).

## 2. What the route is worth: 50.978, which is 7.709 UNDER the author time

The standing bound on this map is a sum of best sectors — five numbers on a
67 s lap, so it can only see a swap between whole sectors — and it says 63.263.

`tmtraj geom envelope` asks the same question at a **10 m** grain. Every run is
projected onto one centreline by banded monotone alignment (DTW), so a run that
spins is charged for the detour instead of being credited with speed at a place
it was not; then each 10 m bin is priced at the **shortest time anyone has ever
taken to cross it**.

```
RAW ENVELOPE      = 50.978
FEASIBLE ENVELOPE = 51.567   (forward-backward under this field's own accel limits)
target 58.687: raw -7.709, feasible -7.120
```

**Controls, all three of them:**

* `--self-control`: the identical pipeline on ONE run's own data must return
  that run's own lap. **16 of 16 do**, to 0.006–0.36 s (bin-edge interpolation);
  the median miss from the reference line is 2–8 m for the whole field.
* Monotonicity: drop our own TAS from the population and the bound must get
  *slower*. It does — **52.589**, still 6.098 under the AT on human driving
  alone.
* A single run in, that run's own lap out: rank01 → 63.561 against a real
  63.546.

This is an optimistic bound and cannot be driven — it stitches together cars in
states that cannot be reached from one another. That is exactly what makes it
useful as a NEGATIVE: **the author time does not require a speed nobody has
reached anywhere on this route.** Whatever the AT is, it is not a speed
problem, and the framing "the AT is 4.8 s below every estimate of the route"
is an artefact of estimating the route from five numbers.

## 3. Our TAS is 3.773 s slower than a human, and the deficit has addresses

Sector by sector against rank01 (Roevhaal, 63.546, real recorded telemetry):

| | S1 | S2 | S3 | S4 | S5 | lap |
|---|---|---|---|---|---|---|
| ours | **12.475** | 19.017 | 13.904 | **16.307** | 5.616 | 67.319 |
| rank01 | 13.492 | **17.651** | **11.309** | 17.130 | **3.964** | 63.546 |
| ours − rank01 | **+1.017** | −1.366 | **−2.595** | **+0.823** | −1.652 | **−3.773** |

`tmtraj geom pace` at a 25 m grain puts each loss at a place, and every one of
them is the same event — **our lap arrives too fast and craters**:

| arc | our speed through it | rank01 | we lose |
|---|---|---|---|
| 1050–1200 m | 196 → **64** → 106 km/h | 169 → 135 → 133 | **1.895** |
| 1550–1700 m | 120 → **72** → 101 → **77** | 181 → 177 → 155 → 132 | **2.026** |
| 1900–2050 m | 155 → **107** → 121 | 161 → 142 → 152 | 0.830 |
| 2450–2620 m | the closing descent, min **59** | min 109 | ~1.100 |

Our peak is 272.0 km/h and our troughs are 59–77; rank01 peaks at ~254 and
troughs at 109–135. The four wall impacts the previous arm found are these.

### The deficit is not reachable by local search — measured, with a budget

Two windowed searches, plain oracle, guard on, aimed at exactly the two worst
places, scored at the next real checkpoint through a verified segment map:

| window | scored at | was | got | against a deficit of |
|---|---|---|---|---|
| ticks 3400–4450 (the 1550–1700 m loss) | CP3 | 45.396 | **45.174** (−0.222, 107 confirmed improvements) | 2.595 |
| ticks 2050–2900 (the 1050–1200 m loss) | CP2 | 31.492 | **31.441** (−0.051) | 1.366 |

Two and a half hours each, 40 workers. **The incumbent's line at those places is
a local optimum and the human's 4 s is a different line, not a nearby one.**

### And a faster line into CP3 cannot be reconnected to a finish

Two of the sector-3 winners were taken as seeds and the whole tail rewritten —
ticks 4450–6845, real map, `--seg 3` for gradient, 24 workers, 100 minutes
each, ~135 000 evaluations apiece. **Neither produced a single finisher.** Both
ended where they started: `DNF cp3`.

So the 0.205 s a reseeded sector-3 basin gains at CP3 does not merely convert
badly, as the 3.8 % rule says — it does not convert at all inside a budget that
comfortably finds 24 improvements when the tail is searched from the incumbent
itself. Reseeding on new basins is what broke 208024 open; here it does not.

### Where the 0.119 s came from

The one thing that did pay was the plain closing-sector search from the
incumbent: ticks 5900–6845, real map, 24 workers, 103 minutes, 135 450
evaluations, **24 improvements confirmed by the plain oracle and 0 phantoms** —
67.319 → **67.200**, entirely in the last 5.5 s (S5 5.616 → 5.497) with
CP1–CP4 unchanged to the millisecond. The previous arm's five endgame searches
had settled at 67.319; the difference here is the `--seg 3` ladder giving the
DNFs a gradient and a fresh seed, not a new idea.

The segment maps used as rulers are verified twice: they reproduce our own
ghost's declared splits exactly (12.475 / 31.492 / 45.396 / 61.703), and they
reproduce **rank02's** own splits exactly (13.906 / 33.106 / 45.437 / 63.812) —
an independent run that was not used to build them.

## 4. The finding: the 2022 build's car turns MORE than ours, and it happens in one corner

Ten of the fifteen records were set on build `113150` (2022) and none of them
replays. The project's standing reading, from arm `a672`, is that the build's
physics ARE ours and the map merely amplifies a recording quantum. That reading
is **half right, and the half that is wrong is the half that matters.**

`fk trace` gives the engine's own per-tick run of a tape; comparing it with the
trajectory the same file records — **with the whole-tick lag scanned, not
assumed** — measures the divergence directly. The instrument's floor is set by
a current-build recording:

* **rank02, which replays exactly: 0.0002 m, for the whole 68 s lap.** At lag 0
  it reads 0.42 m at every point, which is one 10 ms tick at 150 km/h; scan the
  lag and it is two ten-thousandths of a metre.
* Two different fork points (tick 60 and tick 120) give bit-identical
  divergence curves, so the resume is not the thing being measured.

Against that floor:

| ghost | build | tracks its own recording to | first departs |
|---|---|---|---|
| rank02 68.442 | current | **0.0002 m over the whole lap** | never |
| rank01 63.546 (WR) | 2022 | 0.0003 m to 3.56 s | **3.990** |
| rank05 73.922 | 2022 | — | **4.040** |
| rank09 79.967 | 2022 | — | **4.140** |
| rank04 70.543 | 2022 | — | **4.240** |
| rank08 76.919 | 2022 | — | **4.240** |
| rank15 103.785 | 2022 | — | **4.240** |

**Six of six, in a 0.25 s window, in the same corner** — cell (27, 14, 23),
`RoadBumpCurve1`, a STOCK block, at the onset of the lap's first big slide
(side speed climbing 0 → 30 m/s). Not on a custom ice block: there is no
`FlinkIceBlock` within four cells of it.

### It has a sign, and the sign is the same in all six

`tmtraj geom track` reports the SIGNED lateral offset — which side of the
recording's own direction of travel the engine ends up on. About 90 % of the
divergence is lateral, and in all six it is the same sign:

```
rank01  4.060 lat -0.041   4.560 -0.404   5.060 -0.866   6.060 -1.408
rank04  4.060 lat +0.001   4.560 -0.281   5.060 -0.525   5.560 -0.813
rank05  4.060 lat -0.021   4.560 -0.312   5.060 -0.856
rank08  4.060 lat +0.001   4.560 -0.025   5.060 -0.164   5.560 -0.510
rank09  4.060 lat -0.001   4.560 -0.288   5.060 -0.570   5.560 -1.249
rank15  4.060 lat +0.002   4.560 -0.187   5.060 -0.505   5.560 -0.878
rank02  4.050 lat +0.000   4.550 +0.000   5.050 +0.000   6.050 -0.000
```

Negative is the outside of the corner. **Today's car rotates LESS than the 2022
car did, on the same inputs, at the same place** — and it does so whether that
recording was holding full lock (rank01, rank15: steer +127) or no steering at
all (rank05, rank08: steer 0). An arbitrary rounding seed amplified by a chaotic
map would take a random side: six of six agreeing is 1 in 64, and they agree on
the place as well.

### It is a STEP, not a drift, and it is far bigger than one steering unit

The scale that settles what kind of thing it is:

* rank01's discrepancy crosses **5 cm within 0.05 s of being born** (born
  3.970, over 5 cm at 4.020).
* **A full steering unit, at one tick, on the same map at the same moment, does
  not move the car 5 cm off its own line in the next 3.6 s** (rank02, break of
  −1 at tick 400: horizon at a 5 cm bar is 7.590 with and without the break).

So the divergence is not a small input-scale difference accumulating. It is a
step in the car's state at a single contact — which is what a bump-road curve
taken at 138 km/h and 15 m/s of slip is made of.

## 5. Can it be repaired? No — and the control says that is about the tape, not the tool

### 4b. The state at the descent: we arrive in the air, straight, and facing the wrong way

`tmtraj geom at`, at matched distance rather than matched time, on the run-in to
the closing descent:

| arc 2400 m | t | km/h | lateral m/s | ground | yaw |
|---|---|---|---|---|---|
| **ours 67.319** | 59.750 | 149.6 | **1.28** | **AIRBORNE** | +127.9° |
| rank01 63.546 | 56.860 | 134.5 | 37.32 | yes | −97.8° |
| rank02 68.442 | 61.350 | 125.2 | 29.14 | yes | −67.8° |
| rank03 69.522 | 62.510 | 133.9 | −21.64 | yes | +164.4° |

and 75 m later, at the descent entry itself, our lateral speed is **+34.67**
where all three humans are at **−27.9 to −31.9** — the opposite sign.

The previous arm's "22.2 m/s of lateral speed for the clean tape and 0.3 for
ours" understates it. We are not merely pointed straighter: at 2400 m we are
**off the ground**, with essentially no lateral speed, at a heading 130–220°
from every human on the map, and by the descent entry we are sliding the other
way. That is the state a chained search has to hit, and it is a long way from
where the incumbent's basin sits.

### The repair

`fk resync` (new) does one locate and then thousands of candidate repairs
against a recorded line, scored on the **sync horizon**: the race time at which
the engine's run of the candidate first leaves the recording by more than
`--tol`.

**Positive control**, same machinery, same budget: take rank02 — which tracks
its own recording for 68.390 — break it by ten steering units at one tick, and
repair it.

```
CONTROL: broke tick 420; horizon 68.390 -> 8.370
  round 1: 20.390   round 2: 35.760   round 3: 55.410      (4000 evals)
```

**47 of the 60 lost seconds recovered, and still climbing when the budget ran
out.**

The subject, same machinery:

| run | window | input space | evals | horizon |
|---|---|---|---|---|
| rank01 | ticks 320–420 | steer, span ≤ 2, ±16 | 12 928 | 5.060 → **5.140** |
| rank01 | ticks 350–400 | steer, span 1, ±12 | 1 224 | 5.060 → **5.060** |
| rank01 | ticks 350–405, tol 0.30 | steer, span 1, ±12 | 1 344 | 4.220 → **4.220** |
| rank01 | ticks 370–402, tol 0.50 | steer ±8 span ≤ 4 **+ brake + lift** | 4 752 | 4.610 → **4.790** |
| rank01 | ticks 330–410 | steer ±20 span ≤ 3 **+ brake + lift** | **40 824** | 5.060 → **8.670** |

**Steering alone recovers nothing.** rank01 holds **full lock (+127) for every
tick of the window**, so one of the two steering directions is a clamped no-op
by construction and a steering-only sweep searches half the input space —
which is why `--pedals` was added rather than reporting the steering null as
the answer.

With the pedals in, the repair does bite: **5.060 → 8.670**, and *both* of the
large accepted moves were **brake taps**. That is the physically right
compensation for the mechanism in section 4 — braking on ice rotates the car,
and section 4 says today's car rotates too little — so the search independently
found the correction the measurement predicts.

**And it is still not a repair.** 3.6 s recovered out of 58 s lost, from 40 824
candidates, against a control that recovers 47 s of 60 from a deliberate break
in 4 000. The 2022 world record's lap is not available to us by putting its own
tape back on its own line.


## 6. What this means for the author time

The AT is 58.687 and it is **`authorScore: 58687` inside the map file, which
Nadeo stamped at upload on 2022-07-31** (`acq/mapinfo.json`; the medal chain
below it is 63.000 / 71.000 / 89.000). The ten records that do not replay are
all from build `113150`, dated 2022-07-06, and the world record was set on
2022-08-06. **The author time and the whole non-replaying half of the field are
the same build**, and section 4 measures that that build's car rotates more
than ours in this map's defining manoeuvre.

So the author time is a time set by a car that corners better than the one we
are searching with — which is the mechanism behind the 4.9 s the project has
been unable to explain between the two populations, and it is not "the map
amplifies a recording quantum".

Two things follow, and they point in opposite directions, which is why both
should be said:

* **Against reaching it:** the AT is not a target our car has been shown to be
  able to hit. Every "the route is worth X" number anchored on 2022 driving —
  including the 63.546 world record and the field-best-sector sum of 63.263 —
  is a number about a different car.
* **For reaching it:** the route's own envelope, computed at a 10 m grain from
  runs the current engine reproduces, is **50.978**, and the project's tightened
  splice bound of 61.204 is assembled from parts that all re-simulate today.
  **58.687 is inside what the current car has already done, metre by metre.**
  The obstacle is our search, which is 3.773 s slower than a human.

## 7. A repo bug worth its own line: the guard logged 234 files it never wrote

`tmsearch`'s bank is the one place a result becomes a result — `Bank::offer`
validates a tape through the plain oracle and then puts it in `--bestdir`. That
last step was:

```rust
let _ = std::fs::rename(&tmp, &path);
```

three times over, once for each thing the guard writes. `tmp` lives in the
per-pid scratch root, which defaults to `/dev/shm`; a `--bestdir` on ordinary
disk is a different filesystem, so the rename fails with `EXDEV` — **and the
error is discarded.** The log then records

```json
{"confirmed":"45.140","provenance":"…","file":"…/best_45_140.Ghost.Gbx"}
```

for a file that does not exist. Two of this arm's searches ran for two and a
half hours, logged 234 and 6 confirmed improvements between them, and left
**empty bank directories**.

Nothing about the guard's validation is wrong; what was wrong is that its own
record was unfalsifiable. Fixed here: `install()` renames, falls back to
copy-and-remove across a device boundary, and **panics** if the file cannot be
put in the bank at all, because a confirmed result that is not on disk is not a
result. Verified by re-running the same search: 9 files in the bank in four
minutes where the previous run left none.

## 8. What I would do next, in order

1. **Stop searching the incumbent's basin sideways.** Two 2.5-hour windowed
   searches at the two worst places bought 0.273 s against a 3.961 s deficit,
   and two 100-minute tail searches from those winners produced **zero
   finishers**. The line is a local optimum and its neighbours do not
   reconnect.
2. **Seed from rank01's LINE, not its tape.** The tape cannot be repaired
   (§5), but its recorded trajectory is a real target: build a search whose
   objective is arclength along that recording — `Progress::Metres` with
   `--refcsv` already exists in `tmsearch` — and cold-drive it sector by
   sector. Two things block that today and both are small: `tmsearch`'s fork
   mode **cannot locate on a tape that DNFs** on this map ("no u32 advances by
   exactly 10 every tick near the vehicle state"), while `fk`'s own locate can
   — port `fk::locate::locate_v2` into the search's fork worker; and the state
   objective SEARCH.md §5.1 describes is still unbuilt.
3. **The state to aim at is in `bank/state_at_descent.txt`**, and it is not the
   one the previous arm named. At arc 2400 m every human is on the ground with
   27–37 m/s of lateral speed; we are **airborne, straight, and 130–220° from
   their heading**. Score that, not arrival time.
4. **Ask the sibling maps whether the build difference is the bump road or the
   ice.** The divergence is on `RoadBumpCurve1`, a stock block, at 15 m/s of
   slip. `a672`'s sibling-map control (134682, 41.5 s reproduced exactly) had a
   mean side speed of 5.68 m/s. One old recording taken at high slip on a stock
   bump curve, on any map, decides whether §4 is a property of this map or of
   the build — and if it is the build, it is a fleet-wide fact about every
   pre-2023 time this project uses as a reference.
5. Do **not** re-derive: the route length, the absence of a shortcut, the
   envelope, the divergence onset and its sign, or the repair null. They are
   measured here with controls.

## Files

| file | what |
|---|---|
| `ksi2_RESULT_v1.md` | this |
| **`ksi2_67200_watchable_v1.Ghost.Gbx`** | **the new map record, 67.200**, md5 `baa76201ea8095fe32b7691b312c5774`. Regenerated: position, orientation and speed are read out of the engine, and the plain oracle re-simulates the WRITTEN file to 67.200. `ghost verify` V1–V7 pass. **`tmtraj check` REFUSES it at C5 and C7, exactly as the 67.319 file was refused**: `ghost regen` rewrites 25 of the 116 bytes per sample and the per-wheel ground-contact and surface-material channels stay the carrier's, so tyre effects fire at rank02's flight times. **Do not film it.** Its stored SPLIT LIST is also still the carrier's (13.906 / 33.106 / …): `ghost declare` writes the time and has no way to write the splits — the deleted `u02 declare --splits` could, and that is a regression. The true splits are 12.475 / 31.492 / 45.396 / 61.703 / 67.200, measured on the verified segment maps. |
| `ksi2_67200_SEARCHTAPE_inputs_only_DO_NOT_PUBLISH_declares_68442.Ghost.Gbx` | the raw search output, md5 `b0bd63c331e2abf2a01cf7dad4031dcd`. Its decoded telemetry is **byte-identical to rank02's** (md5 of `tmtraj export --csv`, both `1150f151…`) — the two-second poisoning test, run and reported. Fine as a tape, never as a render. |
| `ksi2_bank_v1.tgz` | every table below, the five verified segment maps, the search logs |
| `bank/path_table.txt` | arclength per run and per sector |
| `bank/pace_25m.txt` | where our lap loses, at 25 m |
| `bank/envelope_perbin_v2.txt` | the 10 m envelope, per bin, with its owner |
| `bank/selfcut.txt` | every self-approach of the ribbon |
| `bank/divergence.txt` | the six 2022 ghosts, onset and sign |
| `bank/state_at_descent.txt` | the state every run arrives at the descent in |
| `bank/resync_log.txt` | the repair runs and their controls |
| `bank/s3loss_log.jsonl`, `bank/s2loss_log.jsonl` | the two windowed searches |

Tooling is in the repo, branch `ksi134672`: `tmtraj geom {path,pace,at,selfcut,
envelope,track}`, `fk resync`, `tmmaps rungspec --cells --curtain`, plus four
repo bugs fixed on the way — `tmsearch` did not compile; `tmtraj export --csv`
was unreachable; `tmmaps segments` refused a rung that was exactly right; and
**the search guard logged 234 banked files it never wrote**.
