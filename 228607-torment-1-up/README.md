# Torment (1-UP) — the author time falls, and the technique was on a leaderboard nobody had connected to the map

**Author time 20.258 · human world record 24.902 · best validated 19.907.**

| tape | validated | vs AT | vs human WR |
|---|---|---|---|
| [`TAS_19907`](replays/TAS_19907.Ghost.Gbx) | **19.907** | **−0.351** | **−4.995** |
| [`FORGIVING_19948`](replays/FORGIVING_19948.Ghost.Gbx) | 19.948 | −0.310 | −4.954 |
| [`TAS_19910`](replays/TAS_19910.Ghost.Gbx) | 19.910 | −0.348 | −4.992 |
| [`TAS_19927`](replays/TAS_19927.Ghost.Gbx) | 19.927 | −0.331 | −4.975 |
| [`TAS_19936`](replays/TAS_19936.Ghost.Gbx) | 19.936 | −0.322 | −4.966 |
| [`TAS_20070`](replays/TAS_20070.Ghost.Gbx) | 20.070 | −0.188 | −4.832 |
| [`LOWINPUT_20070_16values`](replays/LOWINPUT_20070_16values.Ghost.Gbx) | 20.070 | −0.188 | −4.832 |
| [`TAS_20083`](replays/TAS_20083.Ghost.Gbx) | 20.083 | −0.175 | −4.819 |
| [`SPLICE_24854`](replays/SPLICE_24854.Ghost.Gbx) | 24.854 | +4.596 | −0.048 |
| author time | 20.258 | — | −4.644 |
| human WR, on the altered board | 24.902 | +4.644 | — |
| [**the author's own lap**](replays/AUTHOR_LAP_20258_watchable.Ghost.Gbx) | *20.258* | — | *watchable only — see below* |

**Two independent official seeds land 17 ms apart, and that says more than either
tape.** 19.910 descends from official **rank 1**'s line (a separate search
island, 236 320 evaluations); 19.927 descends from **rank 10**. A third line sits
at 19.940, and two earlier islands converged on 19.927 from different mutation
windows.

A single best tape can be a lucky corner of the landscape. **Four arrivals from
four directions — two of them from different human seeds — is evidence about the
map's floor**, and it puts that floor at about 19.91–19.93.


TMX map [228607](https://trackmania.exchange/maps/228607) · **23 recorded runs**
· and, as of tonight, an **official field of 400 000** — this map is
[Fall 2024 - 08 with the Goal moved 64 m](../_altered).

**Not submitted to any Nadeo leaderboard, and it never will be.**

**This map is closed.** Four revisions of the headline, four independent
arrivals, and a final pair of multi-knot searches that could not find anything
better than 19.907 — the detail is in *The corner is not available*, below. The
tape to hand a person is not the fastest one:
[`FORGIVING_19948`](replays/FORGIVING_19948.Ghost.Gbx) has the *same* 37 %
tolerance as the record tape but with its window re-centred — 18 ticks of slack
early and 15 late, instead of a grid pinned against one edge — for 41 ms.
---

## The technique was never undiscovered. It was on a leaderboard nobody had connected to the map.

This map's own board has 23 players and **not one of them ever fires the
launcher**. That looks like an undiscovered trick — and it is not.

228607 is an Altered Nadeo copy of the official **Fall 2024 - 08**, which has a
field of **400 000**. On that board:

* **the top 15 all fire the launcher**, at 692–997 km/h in a single 50 ms sample;
* **not one of them holds the coast afterwards.**

So the map splits cleanly into a technique that is common knowledge on one
leaderboard and invisible on the other, and a second technique that **nobody has
ever held** — including the 400 000.

The author sits **17–33 m above the entire visible top of that field** at the
Goal band, with **vy +79.5** at ignition + 0.45 s against their +49.8 to +68.6.

> The launcher is known-and-held by the official field. **The hold is held by
> nobody but the author.**

## What the author actually does — read out of the map file

228607 embeds the author's own recorded lap: 406 samples at 50 ms, 0 → 20.290,
declared **20.258**, which is the author time. It had been *counted* many times
— it is the fleet's positive control for embedded-ghost scanners — and, as far
as this project's records go, **never read**.

It is now read, and it is published above as a watchable replay.

**Caveat, and it is important:** a record-data node carries **no input archive**.
This lap can never be re-simulated and is not a seed. It is a per-tick record of
the run that set the time — a reference trajectory, nothing more. (Its rebuilt
file also inherits the carrier's checkpoint list; ignore that field.)

Its sibling [Torment (1-DOWN)](../228811-torment-1-down) embeds *its* author lap
too, declaring 20.550. So we have **both author laps of the same map**, one
finishing at the high Goal and one at the low, and the difference between them is
the whole of this map's problem:

| | **228607 (1-UP)** | 228811 (1-DOWN) |
|---|---|---|
| reactor fires | t = 18.540 at (77.7, 51.3, 708.9) | t = 18.650 at (75.6, 53.2, 708.3) |
| at ignition | 340 → **769 km/h**, vy **+92.0** | 323 → **751 km/h**, vy **+94.1** |
| 0.45 s later | **742 km/h, vy +79.5**, y = 91.0 | 548 km/h, **vy +22.5**, y = 81.9 |
| at x ≈ 360–405 | y = **160 → 173**, still climbing, 671–688 km/h | y = **95**, flat, 521 km/h |

**Ignition is nearly identical — within 0.11 s and 2 m.** What differs is the
next second: the 1-UP lap **holds** the climb, while the 1-DOWN lap's vy
collapses 94 → 22 and its speed 751 → 548. By the gate band the 1-UP car is 78 m
higher and 150 km/h faster.

That is why **you must not carry 1-DOWN's driving guide across.** Its closing
instruction is *keep the lock held*, which is right for a low Goal and sends you
broadside at 562 km/h here. Both pages now carry that warning; the mechanism is
in [`TECHNIQUE.md`](../228811-torment-1-down/notes/TECHNIQUE.md).

## For a driver

The objective is not a time and not a tape. It is **a state at a place**:

> **Cross x ∈ [352, 416], z ≈ 672–800, at y ≈ 155–175, ascending at
> vy ≈ +45…+80 m/s, at ≥ 670 km/h, by t ≈ 20.2.**

and the upstream condition that produces it:

> **Fire the reactor at x ≈ 78, y ≈ 51, z ≈ 709 at ≥ 340 km/h — then hold
> vy ≈ 80 m/s for the next ~1.5 s instead of levelling off.**

Everything before ignition, the official field already does. The 0.3 s that was
left sits entirely in what the car does *after* the reactor lights, which makes
this an air-control problem in a reactor flight — the same class that was worth
1.824 s on [Spring 2023 - 24](../199100-spring-2023-24-2up).

## For a driver: early is free, late is fatal

This is the most useful thing on the page, and it is a single sentence a player
can act on.

Slip everything after tick T by one tick and re-simulate. Through the decisive
window — **race 18.70 to 19.30**, which is the release and the counter-steer:

| steering slipped from | **10 ms EARLY** | **10 ms LATE** |
|---|---|---|
| race 18.70 … 19.20 | **19.936** — keeps the run, and is 11 ms *faster* | **loses the Goal** |
| race 19.40 … 19.70 | — | 20.065 — survives, merely slower |
| race 19.80 … 19.90 | 19.946 / 19.941 | 20.172 |
| after race 20.10 | no change | no change — the flight is committed |

> **Release the lock a touch early rather than a touch late.**

It is consistent with the mechanism: the flight is ballistic after ignition, so
stopping the roll sooner leaves more of the launch's vertical velocity intact,
while stopping it later has already spent the climb.

**And the budget is exactly one tick.** −2, −3, −4, −5, −7 and −10 all lose the
Goal from every T tested. "Early is free" means *one* tick early, not "the
earlier the better".

That probe is also where **19.936** came from: it is the 19.947 tape with its
steering slipped 10 ms earlier. The tolerance measurement produced the incumbent.

## "Fewer inputs is easier to drive" is measured false here — twice

The low-input member of this family is published above, and **it is the worse
thing to hand a human despite looking simpler.**

| | analog | low-input |
|---|---|---|
| time | 20.070 | **20.070 — identical** |
| steer values | 200+ | **16** |
| input events | ~140 | **47** |
| survives a 10 ms early slip? | **yes, across a 600 ms window** | **no — neither direction, at any T** |
| whole-block probe | 1 survivor of 12 | **0 of 12** |

A third of the inputs, the same time, and **no tolerance at all**. Two
independent probes agree.

So the low-input tape is published as a *result*, not as a recommendation: on
this map the drivable artefact is the analog one, because it is the one that
forgives a human-sized error. Each member needed its own tolerance number, and
assuming the simpler-looking tape was the friendlier one would have been wrong.

## Mining the official field: rank 1 is the wrong seed here

The [official field of 400 000](../_altered) is this map's best resource, and
its **ranking is anti-correlated with what we need.**

The alteration moves the Goal *up*. On the official map flying high is a
**penalty**, so the board sorts players by how well they avoid precisely the
thing this map requires. Our winning seed was **rank 10, not rank 1** — rank 10
fires a y = 146 rung where rank 1 only reaches y = 138 — and the field has a
1.08 s cliff after rank 16.

> **When a map has been altered, the official leaderboard's order is a ranking
> for the *original* objective.** Rank by the quantity your map actually needs
> before picking a seed, not by finishing position.

## This map has ten hidden waypoints, and they are five gates in disguise

A tag audit found **ten `LinkedCheckpoint` waypoints** here that none of this
project's tooling had ever enumerated — the waypoint filter matched only
`Checkpoint`. That sounds alarming and turned out to be a clean piece of
detective work with a reassuring answer.

**They are five coincident pairs**: `Left32m` + `Right32m` at the same x and y,
32 m apart in z. **One 64 m gate assembled from two 32 m item pieces.** With the
one plain checkpoint, that is **six logical checkpoints**, which is exactly what
the map has always behaved as having.

Three independent confirmations, none of them needing a new simulation:

* human ghosts declare **7 splits** — six checkpoints plus the finish;
* every non-finisher returns `cps=6`, never 7 or 10;
* and decisively: **a car cannot be at z = 720 and z = 752 at the same instant,
  yet 15 of 15 official humans finish.** If each member were separately
  required, nobody could ever complete this map.

**Crossing one member satisfies the set.**

The tempting inference — *a set of alternative required waypoints means
unexplored routing* — is **false here**, and it is checkable in minutes off the
item positions with no oracle calls at all: **for each set, are the members
adjacent or scattered?** Adjacent means one wide gate and no routing freedom
whatsoever. On this map every set is adjacent, so this is a **confident negative
rather than an open gap.**

**No number on this page is affected.** Every figure here is a game-scored lap on
the untouched map — the game sees every waypoint whether our tools enumerate it
or not — and the search apparatus for this map used no segmentation at all: every
rung was the whole untouched map with the four `GateFinish` blocks relocated
position-only and every other waypoint left required. The gap bites only on
things cut from a *segment* map, and nothing here is.

## What our time is being compared *against*, stated precisely

The author time on this map is **a declaration**, and a reader should know both
halves of that before reading the −0.348.

**Nobody can re-simulate the author's lap.** The record embedded in the map is
**telemetry only — there is no input archive** — so it can be watched and
measured but never re-driven, by us or anyone. unbeaten.at also reports
`atSetByPlugin: true`.

**But the declaration is consistent with a genuinely driven recording**, and that
is the stronger half:

* 406 samples at 50 ms, spawning at t = 0.040;
* ending on the 1-UP Goal block at (405.5, 172.9, 715.0) at **20.290** —
  **32 ms past the declared 20.258**, which is exactly the post-finish rolling
  window every ghost on every map shows.

A fabricated number would have no reason to land 32 ms inside that window on the
correct Goal block. So the comparison this page is making is: **our validated,
re-simulable lap against a declared time backed by a driven telemetry record we
cannot replay.**

That does not weaken the result — 19.907 is a game-scored lap on the untouched
map through a single-file gate on three independently built toolchains. It
changes what the *author* side of the comparison is, and it is why everything
this page says about his flight is **inferred from telemetry rather than
reconstructed from inputs**. It is also why the official field matters so much
here: 15 official human tapes **can** be re-simulated on this geometry, and every
one returns its own official time to the millisecond.

## The corner is not available — and that is a result

An earlier reading of this map's launch/coast decomposition suggested about 12 m
above the author's line was available, at the corner of the two knobs. **It is
not.** That was a true statement about the arithmetic of the field's per-axis
range and a false one about what can be driven, and the difference has now been
settled by measurement rather than argued. A reader who followed the old reading
would spend a night chasing a corner that does not exist.

Two search islands were seeded from the field's extremes:

| seed | launch | coast | converged at |
|---|---|---|---|
| official rank 8 — the steepest launch | vy **101.6** | −45.5 | **20.000** |
| official rank 3 — the best coast | vy 74.6 | **−26.3** | **20.007** |

Neither approached the implied corner at y ≈ 173. And they could not, because
**the two knobs are coupled at the launcher**: what makes rank 8's launch steep is
the same attitude that carries the roll past inverted (−2.49, wrapping to +2.25).
**You cannot keep that launch angle and acquire rank 3's coast** — the input that
buys one spends the other.

Then the last phase that had never had a multi-knot control got one. Two
approach searches, each about **147 000 evaluations**, reached **19.908 and
19.907** — no different flight, nothing anywhere near y ≈ 173. Put together with
the scrub attractor and the two extreme-seed islands, **every phase of this run
has now been searched with a multi-knot control, and the corner does not open.**

So the page can say plainly *why* 400 000 players trade launch angle against
coast quality and not one of them does both: the attitude that steepens the
launch is the attitude that rolls the car past inverted. **That is an answer, not
a shrug** — the coupling is physical, and the negative is a mechanism rather than
an exhaustion.

The "corner" was an artefact of treating two coupled quantities as independent
axes. Publishing it as a bound would have sent the next arm after something that
does not exist, which is why the correction sits here beside the analysis rather
than replacing it quietly.

## One map, six findings that outlived it

Unusual enough to say in the repo's own voice: **more transferable methodology
came out of this one map than any other in the project.** Six results here are
general rather than local, and all six are in [`FINDINGS.md`](../FINDINGS.md):

1. **The corner method** — decompose a field into per-axis extremes to price
   what is theoretically available, *and* the decoupling test that has to be run
   before that price is real.
2. **The scrub attractor** — independent searches converging on the same
   physical behaviour from unrelated seeds.
3. **"Early is free, late is fatal"** — the direction of a timing error matters
   more than its size, stated here with a measured bound.
4. **A forgiving variant is not a slower tape.** The deliverable came from
   *re-centring* the tolerance window, not from giving up time.
5. **Rank an official field by the property you consume**, not by finish time —
   the rule for mining official records on a Goal-moved map, where the board's
   own ordering answers a different question.
6. **"Fewer inputs is easier to drive" is false**, measured twice on this map
   alone.

## Validation — and the strongest yes-control in the project

`TAS_19907` sha256 `7af61820ebc9c73a7d98641d3cab9e7e136e1f0914c9a49db80498551d889d6c`;
`TAS_19910` sha256 `f9ea0b209db48b36140c845963c0ee26396bc1a1f5ec64ec352db367782edd3b`,
map sha256 `2c6d500aa73e3e86c1b9c64c61e5801c04b1b9d757687a9054ecc0fb118976e5`
(md5 `65b6b7bcf4808070383e6e9ff9de28f1`).

**Every headline here cleared a single-file gate**: `--jobs 1`, one file per
invocation, a fresh process each time, an empty staging directory, and copies
taken from the archive rather than any working tree — so nothing about the batch,
the scheduler or a neighbouring tape can contribute to the number. Reproduced
here on a separate toolchain, one file at a time:

```
tor_BEST_19907.Ghost.Gbx      19907
tor_FORGIVING_19948.Ghost.Gbx 19948
tor_BEST_19910.Ghost.Gbx      19910
tor_BEST_19927.Ghost.Gbx      19927
CTRL_ident_24854.Ghost.Gbx    24854
CTRL_splice_24854.Ghost.Gbx   24854
```

with the controls' own hashes matching the archive byte for byte
(`2188261a…e62db`, `86eb254f…6371a`). The 19.907, 19.910 and 19.927 tapes carry
**zero respawn packets**, audited by enumerating bit 31 rather than assumed.

Before that, **13 of 13 tapes exact and both controls exact** on an auditor's
independently built tree, store-only inputs, hashes taken before validation. That
makes three independently built toolchains on this map.

The tape carries **zero respawn packets** — audited by enumerating bit 31 of each
input packet's state literal, not assumed.

And separately, the identification of this map as Fall 2024 - 08 is backed by
what is probably the strongest control this project has produced: **all fifteen
grafted official Fall 2024 - 08 humans return their own official times to the
millisecond** on this map's rung geometry. Fifteen foreign tapes, fifteen exact,
untunable.

## Also on this map: the within-map control that failed

This page previously published a **negative** — a pre-registered attitude
experiment whose predictions failed here, on a map whose decisive sector does not
order the field. That result stands unchanged and is worth keeping next to this
one: the sector table showed the last sector carrying all the spread while the
*fastest* closing sector in the field finished 11th.

## Notes

* [`AUTHOR_LAP_READ.md`](notes/AUTHOR_LAP_READ.md) — the author's own lap, both laps compared,
  and the state objective
* [`SIBLING_IS_THE_SAME_MAP.md`](notes/SIBLING_IS_THE_SAME_MAP.md) — 228607 and 228811 are one map,
  exhaustively
* [`VALIDATION_independent.txt`](notes/VALIDATION_independent.txt) — the independent auditor's transcript
* [`TWO_KNOBS.md`](notes/TWO_KNOBS.md) — the launch/coast decomposition, and the author on the curve
* [`LINKED_CHECKPOINT_SEMANTICS.md`](notes/LINKED_CHECKPOINT_SEMANTICS.md) — the ten hidden waypoints, settled
* [`RESULT.md`](notes/RESULT.md) — the attitude experiment that failed here

## This map is an Altered Nadeo copy of **Fall 2024 - 08**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **400 000
players** on this geometry.

Geometry and surface are identical (`name_agree` 1.0000); the alteration is the Goal moved 64 m. Its sibling [Torment (1-DOWN)](../228811-torment-1-down) resolves independently to the same official map, which is right — they are one map with the finish in two places. A detail no matcher could have arranged: the official world record on Fall 2024 - 08 is held by **Emelius.**, the person this map's own header name credits.

**All fifteen** grafted official Fall 2024 - 08 humans return **their own official times to the millisecond** on this map's rung geometry — fifteen foreign tapes, fifteen exact, untunable. The recipe here is the three-chunk form, chosen by which lossless control passed in the same batch.

That field also answers the technique question this map poses. The official top 15 **all** fire the launcher, at 692–997 km/h — while **0 of the 23** players on the altered board ever found it.

## Do not follow 1-DOWN's driving guide on this map

[Torment (1-DOWN)](../228811-torment-1-down) is the **same official map with the
Goal moved 64 m**, and its write-up ends with a driving instruction: after the
launcher, *keep the lock held*. **That is correct there and wrong here**, and a
player who carries it across goes broadside at 562 km/h and does not reach the
finish.

The two author laps are **identical up to the launcher** — same contact state
(pitch 0.26, roll 0.06, ~330 km/h, y = 50.4), both leaving at vy 92–94. The
launch is not what separates them:

| | 1-DOWN | **1-UP (this map)** |
|---|---|---|
| after the launch | holds full lock to the end | **holds the lock ~200 ms, releases to centre at 18.740**, then counter-steers progressively to full left by 19.390 |
| roll | runs to −3.1 — the car goes broadside | stops at −1.6 and **returns to −0.18** |
| speed | 751 → **562 km/h** | 769 → **720 km/h** |
| vy | 94 → **22** | 92 → **68** |
| why | the Goal is low, so shedding speed is the point | the nose falls in line with the 25°-up flight path, and **height** is the point |

Both hold throttle and brake together throughout. **The entire difference between
the two maps is what you do with the lock after the launcher** — and it is a good
illustration of why a technique note has to name the map it belongs to. The
correction is also carried on 1-DOWN's own page.
