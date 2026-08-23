# 173691 "Spring 2023-15 (Underwater)" — the endgame, measured to a number

Arm B ("find a way to FINISH the map"), 2026-08-22, node `84047.od.fbinfra.net`,
repo branch `uw173691-finish`. Rust only. Every number here is read off the
plain oracle (`tmmaps oracle`, the dedicated server validating a written
`.Ghost.Gbx`) or off a `fk trace` re-simulation of a written file — never off a
fork-server score.

## HEADLINE

**No finish.** But two of the things this map was thought to be are wrong, the
blocker is now a single measured number, and it is not the number anyone was
working against.

| | |
|---|---|
| the finish | **not "on the upper deck at y ≈ 170"** — it is a wall of ten `GateFinish` blocks at `z = 496`, `x 1344…1504`, in **two** rows: `y 130…162` and `y 162…194`. **Both fire**, and every firing measured in this arm crosses **z = 494.1…495.2** — the trigger is a PLANE, not a volume. Two cars finished at **y = 133.97 / 133.99**. |
| the lower canopy | **not "sealed"** — it is **LOW**. The deck the car already lands on is `y = 114.16`; the wall's live floor is ≈ 131. The gap is **17 m**, not the 48 m that "reach the upper deck" implies. |
| what is 17 m up | a **solid, drivable ledge at y ≈ 130.2 → 134** inside the gate slot (the `StructurePillar` tops / gate plinths in cells 42…46 × 15). Driving on it **finishes the map** — measured, `28.827` on a checkpoint-neutralised probe map, with the trajectory. |
| why it still fails | **nothing on the map connects 114.16 to 130.16, and nothing can fly there.** Complete block census of the stadium footprint between the two decks — 456 blocks, all of them: water fill, vertical `StructurePillar`, the stands' lowest slope whose bottom edge is 162, and the two gate rows. No ramp, no slope, no platform. |
| and from the air | the last road's exit is `(1345.4, 154.9, 387.1)`. The trigger plane is at `z ≈ 494.5`, so the nearest live point is **107 m downrange**. Underwater the glide's horizontal reach has a hard asymptote at `v0/k` — **47–59 m at the water speed cap, from four independent drag fits** — and the best flight ever found off that lip, the previous arm's 40 000-evaluation search, reaches `z = 448.2` at deck height and `z = 437.7` at y = 130. **Short by 46–57 m of a 62 m budget, against an asymptote.** |

So the map is closed by two independent walls, and the write-up below gives the
number for each.

---

## 1. WHAT THE FINISH ACTUALLY IS

`tmmaps waypoints` reports fifteen `GateFinish` blocks tagged `Goal`, in a
5 × 3 grid at `cz = 15` (z 480…512), `cx 42…46` (x 1344…1504), `cy ∈ {24, 28, 32}`
(y 130 / 162 / 194). They are **not** all the same thing — the census flags say
so:

```
U 4633 GateFinish 46 32 15  00300000     <- the cy=32 row: five START gates
U 4649 GateFinish 46 28 15  00510000     <- the cy=28 row: five FINISH gates
U 4650 GateFinish 46 24 15  00510000     <- the cy=24 row: five FINISH gates
```

Moving block **#4633 moves the spawn**, which is what the 2026-08-20 write-up
found empirically and could not explain; the flag word explains it. `cy = 32`
is the start wall, and the finish is the **ten** blocks at `cy = 24` and
`cy = 28`, i.e. **y 130 … 194**.

### Both finish rows fire — and the low one had never been tested

Measured finishes, each an oracle time with a `fk trace` of the same file. The
position is the last traced sample, i.e. the car at the instant the run ended:

| where the car was when the run ended | oracle |
|---|---|
| (1398.63, **133.99**, 494.53) at 1.5 m/s, driving a ledge | **28.827** |
| (1462.67, **133.97**, 494.55) — same tape, two cells further east | **28.857** |
| (1357.83, 163.54, 494.12) | 17.109 |
| (1364.33, 167.21, 494.87) · (1396.30, 167.20, 494.90) · (1419.93, 167.24, 494.70) · (1428.33, 167.22, 494.84) · (1451.93, 167.24, 494.70) · (1483.90, 167.26, 494.65) | 13.77 – 13.88 |
| (1492.70, 166.73, 495.24) | 15.509 |
| (1391.6, 168.7, 496.4) at 15.8 m/s, off the upper deck | 13.850 |
| (1391.8, 162.9, 493.5) — previous arm | 13.499 |
| (1416.2, 183.8, 492.8) — the original map's WR, in air at 200 m/s | 31.563 |

Two things fall out of that column of z values. **The trigger is a plane at
z ≈ 494.5 ± 0.6**, not the 32 m block volume — thirteen firings from four
different route families agree on it to about a metre. And **y = 133.97 is 29 m
below the lowest firing height anyone had recorded**, and 28 m below the
"the finish fires at y ≈ 163–169" that was banked. The banked range was a
property of the *trajectories that had been tried*, not of the gate.

One crossing of that plane did NOT fire: `(1356.5, 159.3, 494.4)` at 1.9 m/s.
So I traced the question properly — 128 runs from the start-wall spawns, every
one of the 39 that finished plus a systematic sample of those that did not,
each scored for where it was while inside `z 493…496, x 1344…1504`:

```
FIRED at y = 133.94, 133.97, 133.98   and   163.69 … 168.05   (24 more)
NOT FIRED, having crossed that window at y = 114.13…114.19 (twelve runs,
          28 to 1131 samples each), and also at 152.96, 155.64, 155.68,
          156.96…158.23, 157.19, and 154.41…164.81
```

**So the live set is not a y-interval, and I have not resolved the trigger's
shape** — it is presumably a proper per-gate box narrower in x and z than the
window I scored against. Two things are nonetheless settled, and they are the
two the route depends on:

* **firing happens at y ≈ 134**, three times, in two different cells, with
  trajectories — 29 m below anything previously recorded; and
* **y = 114.16 never fires, and that is now demonstrated rather than assumed**:
  twelve of the non-firing runs sat inside the trigger window at deck height for
  28 to 1 131 consecutive samples. The lower canopy's null is not a coverage
  failure.

### The control that mattered, and the null it killed

My first attempt at this question was three maps built with `tmmaps move`:
`L_low` (the cy=28 and cy=32 rows moved 2 km away, only the low row left),
`M_mid` (only the middle row), `N_none` (all rows away), each with the spawn on
the upper deck, run against 208 tapes.

```
P_all   28 finishes / 208      <- positive control
M_mid   28 finishes / 208      <- identical to P_all, tape for tape and ms for ms
L_low    0 finishes / 208
N_none   0 finishes / 208      <- negative control
```

Both controls behave, `M_mid ≡ P_all` says the middle row is what fires — and
**`L_low`'s zero is worth nothing**, because `uwlab box` on the trajectories
says the cars were never inside the low row's volume: the best of them missed by
4.2 m, `short by x 0.000 y 4.004 z 1.341`. Leaving the upper deck at 26 m/s they
crossed the slot's far face at `z = 513` while still at `y = 166`, and were 20 m
past it before they had sunk into the band.

> **A miss distance that is not resolved per axis cannot tell a wall from a
> coverage failure.** Four separate sweeps in this arm (208, 100, 3 520 and
> 2 720 runs) produced nulls that a single per-axis number then explained away.
> `uwlab box` prints `short by x / y / z` for exactly this reason.

What actually answered it was an accident of a plumb-probe lattice: one car
spawned on the start wall fell *inside* the slot, landed on a ledge at
y ≈ 131, drove east along it and finished at y = 133.97.

---

## 2. THE WATER, MEASURED

Fitted with `uwlab drag` over free-flight windows, three nested laws each time so
that "which law" is a residual and not an assumption — and on **four independent
glides**, so the bound below is not one tape's accident:

| glide | vertical g | terminal | horizontal law | k | reach at 28.6 m/s |
|---|---|---|---|---|---|
| GothMommy's own demo, steer 0 | 2.16 | 2.68 | linear (rms 1.30 vs 1.48) | 0.595 | 48.1 m |
| the same run, other decode | 1.99 | 2.68 | linear (3.56 vs 3.62) | 0.602 | 47.5 m |
| the ramp-climb `gt2`, steer 0 | 2.34 | 2.65 | linear (1.16 vs 1.17) | 0.533 | 53.7 m |
| the banked landing (wiggling) | 2.04 | 2.65 | linear (2.66 vs 2.77) | 0.489 | 58.5 m |

**Linear wins on all four**, and every quadratic fit wants a physically
impossible positive constant term. So:

| | |
|---|---|
| effective gravity in free water | **g ≈ 2.0 – 2.3 m/s²** (air is ≈ 24.6) |
| terminal sink | **2.65 – 2.68 m/s** |
| ⇒ a glide's horizontal reach | **asymptote `v0/k`, i.e. 47 – 59 m at the water speed cap**; 61.9 m observed on the real landing, so call the ceiling **62 m** |
| flat-water speed cap | 28.6 m/s (103 km/h) |
| turbo | 104 m/s (374 km/h) and it **persists ≈ 340 m / 4 s**, but the nearest of the five turbos is 559 m from the trigger plane in a straight line and ~1.1 km along the only drivable route to it |
| apex gain off the map's steepest ramp | **+3.0 m measured** (launch y 154.9 → apex 157.9), +5.2 m as the upper bound from the fitted law |

> **The horizontal law is fitted over `|vh|` 0.4 … 26.4 m/s and must not be
> extrapolated to turbo speed.** Over the turbo segment of the 1 831 m tape the
> car loses only 3.6 m/s² at 90 m/s, where `k·v` would be 44 — but the engine is
> boosting there, so that segment measures thrust-minus-drag and not drag, and I
> never found a clean ballistic sample above 30 m/s to separate them.
> `uwlab drag` prints the fitted span for this reason. It does not affect
> anything here (every launch point on this map is at the 28.6 m/s cap), but a
> reach quoted for a turbo-speed launch would be a guess.

**The car cannot translate at all in free water.** A car dropped in mid-water
holds `|vh| < 0.02 m/s` through a 24 m descent (trace `traj_rest_on_gate_ledge`
and every column of the plumb lattice). So every metre of horizontal travel is
either on a surface or ballistic from one, and the ballistic part is capped at
about 62 m.

---

## 3. THE GEOMETRY, MEASURED — not read off the census

`tmmaps region` gives a cell and a name. It does not say whether the thing is
solid, how much of its cell it fills, or where its drivable face is; and on this
map **height is the only ruler that works** (`--map` is inert for the replay
container, and relocated gates do not fire). `uwlab probemap` therefore builds
one spawn-moved map per 32 m column, drops a car down it and reports where it
stops. 154 columns over x 1152…1568, z 320…640.

What the lattice says, with the two 35-probe surfaces of the earlier arm and two
it had missed:

| surface | y | where |
|---|---|---|
| lower canopy | **114.16** | x 1312…1536, z 448…608 — all 35 cells, the deck the jump lands on |
| upper canopy / stands | **170.16** | the same 35 cells **minus** (42…46, 15) — that hole IS the gate slot |
| stadium roof | 250.16 | above everything |
| **the gate ledge** | **130.2 → 134** | inside cells 42…46 × 15 — the pillar tops and gate plinths. **New.** |
| **a landscaping massif** | **161.5 → 162.2** | x 1216…1290, z 500…610. **New.** |
| the last road's exit | 154.9 | (1345.4, 387.1) |
| the valley round the stadium | 63.4 … 63.9 | x 1216…1504, z 352…480 |

### Nothing connects 114.16 to 130.16

Every block in the stadium footprint (x 1312…1536, z 448…608) with
`114 < y < 170`, exhaustively:

```
336  DecoWallWaterBase                          the water fill
 95  StructurePillar                            vertical columns, 122 -> 170
 13  StandSlope2StraightFCB  + 2 ...CornerInFCB the stands' lowest slope; its
                                                bottom edge is 162
 10  GateFinish                                 the two rows
---
456  blocks, and that is all of them
```

No ramp. No slope. No platform. The only non-vertical solid between the decks
is the stand slope, whose bottom edge is 48 m above the lower deck.

And the empirical companion, on the checkpoint-neutralised map so that a finish
would be reported: **7 700 runs from 35 lower-deck spawns × 220 tapes, zero
finishes** (`lower_deck_7700_runs.tsv`), on top of the previous arm's 2 400.
Positive control in the same instrument on the same day: spawns one storey up
finish in 39 of 1 100.

And the same question asked with a continuous objective rather than a binary
one. 108 traced runs from six interior deck spawns scored by `uwlab box` against
the gate box, then 158 more (96 of which the car locator resolved) driving
STRAIGHT LINES on 32 headings from five spawns — a constant steer makes the car
circle, so the first sweep never rammed anything — scored by `uwlab maxy`:

```
d_43_15__a127        MISS 15.839 m at (1394.57, 114.16, 484.34)
                     short by  x 0.000   y 15.839   z 0.000
d_45_15__st_260_-127 MAXY  114.267 at (1531.56, 498.72)     <- the highest of all 204
```

**Across 204 scored trajectories the car never gets above y = 114.267**, and
that 0.107 m is the car riding up the pillar base at the deck's east edge
(x ≈ 1531) — every one of the six runs over 114.20 is at that same x. So the
deck is flat to a decimetre, the pillars are vertical, and the height deficit is
**15.73 m** to the gate row's floor and **19.70 m** to the lowest height that
has actually been seen to fire.

That is what "nothing connects the two decks" looks like when it is measured
rather than read off a census.

---

## 4. THE TWO WALLS, AS NUMBERS

**Wall 1 — from the road, the trigger plane is 107 m downrange and the glide is
62 m.**

The last road's drivable exit is `(1345.4, 154.9, 387.1)` at 25.5 m/s
(`traj_ramp_climb_gt2.csv`: a run that climbs the banked curve from y = 139 to
y = 155 and leaves at 17° of pitch — the first tape in this project's store that
does; the road is byte-identical between the stock map and the author's copy).
The trigger plane is `z ≈ 494.5`, so the nearest live point is 107 m of z away.
The asymptotic reach is 47–59 m (four fits) and the best measured glide from that lip is
61.9 m; the previous arm's 40 000-evaluation flight search on exactly this
launch reaches `z = 448.2` at deck height and `z = 437.7` at y = 130.
**Short by 46–57 m**, and the shortfall is bounded by an asymptote, so no
amount of launch tuning closes it. It would need ~183 km/h at the lip against a
103 km/h cap, and the nearest turbo is 559 m away in a straight line (≈1.1 km by road) against a measured 340 m half-life.

**Wall 2 — from the deck, the plane's live band is 16–20 m up and there is no
ramp.** 15.73 m to the gate row's floor at 130, 19.70 m to the lowest height
that has actually been seen to fire (133.97). See §3: 10 100 blind oracle runs
with zero finishes, 204 scored trajectories whose highest point anywhere on the
224 × 160 m deck is **114.267**, and a complete 456-block census of the volume
in between.

**The one surface that is nearly in range is unreachable.** The massif at
y ≈ 162 has its east edge at x ≈ 1290. That is 54–64 m from the trigger plane —
inside the most generous of the four asymptotes (58.5 m) and outside the other
three — and it is also only 22 m and 8 m of climb from the
stands' lowest slope at (1312, 170.16, 512), against a 5.2 m apex, so it is the
one place on the map from which either target is nearly on. But it is an island:
the plumb lattice reads 63.4–63.9 m for **every** column between it and the road
(x 1216…1304, z 352…480), a 128 m valley, and the road's own exit is 168 m away
and 7 m below it. Measured anyway: **2 720 oracle runs from four spawns on it,
zero finishes**; 34 scored trajectories, best approach **35.6 m short in x** —
the blind tapes fall off its edge rather than launching east off it.

---

## 5. WHAT A NEXT ARM SHOULD AND SHOULD NOT DO

**Do not** work the flight off the last road again. The bound is an asymptote,
not a search result.

**Do not** treat "reach the upper deck at 170.16" as the goal. The goal is
**17 m above the lower canopy deck, over x 1344…1504, z 481…511** — a completely
different and much smaller problem, and it is the one worth attacking if anyone
attacks this map again.

**The crack that is left, and it is narrow.** The ledge at 130.2 is not a block
in the census — it is the *shape* of `StructurePillar` and of the `GateFinish`
plinth, and a car found it by falling on it. So the census is not a complete
description of what is solid between 114.16 and 130.16 either, and the
straight-line sweep did find the one non-flat thing down there: the pillar base
at the deck's east edge is worth **0.107 m** of ride-up. If some other piece of
collision geometry is worth 16 m, no tape in 10 100 oracle runs and 204 scored
trajectories has touched it. Settling it properly wants a max-height search on
the fork server with a continuous objective (`uwlab maxy` / `uwlab box` against
the ledge box give one) rather than the blind and straight-line tape families
used here — but the prior after this arm is poor.

**Two loose ends I did not close.** The trigger's SHAPE: firing is observed at
y ~ 134 and 163.7-168.1 and not at 114.16, but there are non-firing crossings of
the same z-window at 153-165, so it is a per-gate box narrower than the window I
scored against and I did not map it. And 62 of 158 straight-line traces failed
in the car locator (`fk trace` refusing to guess at low speed) rather than
returning a number, so that sweep is 96 scored, not 158.

**Untouched by this arm:** CP c (#2180) at (976, 186, 1072), still never fired,
and the final curve that subagent A's tape falls off at x ≈ 1269. Both are moot
while §4 stands — a route that cannot finish does not need its checkpoints — but
if the crack above opens, they come straight back.

---

## 6. HOW TO REPRODUCE ANY OF IT

```bash
cd tools && cargo build --release            # uwlab is in the workspace now
export TM_SERVER=/path/to/TrackmaniaServer-dir
export FK_SHIM=tools/search/target/release/libforkshim.so
M=map_end.Map.Gbx          # the stock map, 4 CPs neutralised, spawn = block 4633

# the finish wall, and which rows are which
tmmaps waypoints stock.Map.Gbx
tmmaps region  stock.Map.Gbx --box 1340,120,478:1510,230,514 --filter GateFinish

# a spawn ladder: one map per 32 m column, then one oracle batch over all of them
tmmaps move $M --out s_44_30_15.Map.Gbx --move 4633:44,30,15
tmmaps oracle --map s_44_30_15.Map.Gbx --ghosts g*.Ghost.Gbx -j 40

# a tape (ALWAYS --from a real one; see the uwlab commit message for why)
uwlab tape --from t.gtape --out x.gtape --ticks 4000 --seg 0:500:0:1:0 --seg 500:4000:24:0:1
ghost tape inject carrier.Ghost.Gbx out.Ghost.Gbx --tape x.gtape

# what a run did
FK_VERR_MAX=3.0 fk trace --tape out.Ghost.Gbx --map $M --work /tmp/w --at tick:1200 --out t.csv
uwlab box  t.csv --box 1344,131,481:1504,194,511      # per-axis miss
uwlab maxy t.csv                                       # the height it ever reached
uwlab drag t.csv --t 26.0:35.5                         # the water's law
uwlab probemap --map $M --tape probe.Ghost.Gbx --tmmaps <p> --fk <p> \
               --cx 36:49 --cz 10:20 --cy 32 --jobs 40 # the plumb lattice
uwlab launch census.tsv --box 1344,130,481:1504,194,511 --v 28.6 --filter Platform
```

Two traps that cost this arm time and are now commented in the source:
`uwlab tape` **must** be given `--from` a real tape (a from-scratch header says
`bits_used = 0` and the oracle DNFs a tape whose decoded ticks are identical to
a working one), and a tape whose steer is exactly **0** on every tick is decoded
by the engine as no input at all — use ±1.
