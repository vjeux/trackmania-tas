# 203330 — "Get in the Hole ( Impossible )": the author time is beaten

*Definitive. Supersedes `RESULT-v1.md` and `RESULT-v2.md`; §4 corrects a wrong
claim those made about the brake, and §4 is the more interesting part of the
report because of it.*

**AT 13.995 s · human WR 14.018 s (in-.-, 2026-08-11) · TAS 13.984 s, validated.**

> **13.984 s on a keyboard — three steering values, 31 inputs — matching the
> unconstrained analog optimum to the millisecond, on a map whose author time
> had never been beaten. The human world record is 14.018 s with 46 inputs.**
> And if you will give up 2 ms, **12 inputs** gets you 13.986, still inside the
> author time.

| | time | vs AT | inputs | steer values |
|---|---|---|---|---|
| human WR `r01` | 14.018 | +0.023 | 46 | 3 (keyboard) |
| **author time** | **13.995** | — | — | — |
| keyboard TAS, minimal | 13.986 | −0.009 | **12** | 3 |
| **keyboard TAS** | **13.984** | **−0.011** | **31** | **3** |
| **analog TAS** | **13.984** | **−0.011** | 424 | 192 |

```
best/an330_13984.Ghost.Gbx           md5 569e571648ac4242883c759eb380cfdc
best/kb330_13984.Ghost.Gbx           md5 6bdaa343ad9af5785eca11c0869a2bc0
lowinput/kb330_31ev_13984.Ghost.Gbx  the thinned keyboard optimum
lowinput/kb330_12ev_13986.Ghost.Gbx  twelve inputs, still under the AT
```

Validated repeatedly through the plain oracle, absolute paths, with `r01`
(14.018) as the identity control in every batch. Nothing was submitted to any
Nadeo leaderboard and nothing will be.

Two agents worked this map: the endgame, the instrument and this report are
mine; the seed screen, the hole arithmetic, the low-input line and the thinning
are the second agent's (`RESULT-lowinput-and-seeds-v1.md`), and the corrections
in §4 are theirs.

Best sub-millisecond measurement of the analog tape's true finish crossing:
**13.983306 s** — 0.31 ms above the next integer after ~5 million evaluations.

---

## 1. What the map is

Uploaded 2024-10-17 by **EvenOliveTM.exe**. Five records in twenty-two months.
Medals 15.000 / 17.000 / 21.000 against an AT of 13.995 — round thousands, so the
author typed the medals in by hand and left the AT as the editor's validation of
a lap they drove. It was always reachable; nobody had reached it. **No
intermediate checkpoints**: one Start, one Goal.

```
t=0.00   RoadTechStart at (1520, 66, 240) with a GateSpecial8mNoSteering on it
t=0.0-3.0 seven RoadTechSpecialTurbo2 blocks, x=1296..1488  ->  810 km/h
t=3.32   GateSpecial24mReset at x=1130
t=3.7    the road ends at x~1070; a 3.5 s dive, apex y=87 at t=5.19
t=7.40   a ramp at (360, 28, -61): 775 -> 813 km/h
t=7.60   a booster: 813 -> 941 km/h
t=8.05-8.50  ground contact, scrubbing 933 -> 841 km/h
t=8.51   THE CANNON at (170, 10, 135): speed is SET to exactly 1000 km/h and the
         car is fired down a 1370 m corridor in +z at (vx,vy,vz)=(2.5,60,273)
t=8.5-11.8  ballistic; apex y=75.3 at z=733
t=11.77  THE HOLE. A wall spans the corridor at z=976 from y=10 to y=138 with
         exactly one cell missing: x in [160,192], y in [64,72]. A 32 x 8 m
         window, entered at 891 km/h.
t=13.18  touchdown at z=1315, y=8.3; 876 -> 849 km/h
t=13.84  ride a 2 m platform lip at z=1472 (864 km/h)
t=13.984 cross the finish plane at z=1507.0, y=11.1, x=172.6, 859 km/h
```

`race_ms = tick*10 − 1500` for the analog lineage (its tape's own start offset;
the keyboard lineage is −1580). Assuming zero cost me forty minutes of searching
a window that excluded the entire landing.

## 2. What the five human records actually do

All five re-simulate to their exact recorded millisecond (the identity control).

| run | time | what happens |
|---|---|---|
| r01 | 14.018 | through the hole at x≈176.5; lands at x≈182 and **smashes the platform lip**: 800 → 312 km/h at t=14.00 — and still wins the map |
| r02 | 14.031 | same line, lands a tick earlier, bleeds more speed |
| r03 | 15.478 | **clips the wall** at z=976 (896 → 621 km/h in mid-air) |
| r04 | 21.230 | **clips the wall** (896 → 492) |
| r05 | 23.153 | clean flight — at z=1504 at t=14.00 doing 786 km/h — and **overshoots**, never triggers the finish, bounces for 9 s |

Two of five are wall clips and one is an overshoot. Through t=8.5 s the five
trajectories are identical to three decimals (at t=5.000 four of them report
x=838.842, y=86.585, z=134.432): four players cannot drive identically, so the
approach is on rails, and every human's race is decided in the 5.5 s after the
cannon.

## 3. Where the 34 ms came from — one place

Sector times from true per-tick trajectories read out of the simulator
(`fk btraj`, then `tmtraj gates`, both extended for this map):

```
run              z=500     z=976    z=1200    z=1291    z=1400    z=1507
r01 (14.018)    9.8945   11.7734   12.6958   13.0779   13.5423   14.0177
TAS (13.985)    9.8934   11.7714   12.6931   13.0749   13.5380   13.9804
TAS (13.984)    9.8934   11.7713   12.6930   13.0750   13.5374   13.9837
```

**The flight is fixed to 2-3 ms and cannot be improved.** The launch is a clamped
1000 km/h from a state no input reaches, and from there the car is ballistic —
gravity and drag only, no control over the centre of mass. Every millisecond of
the 34 is in the last 106 metres.

The mechanism in one line: **the human arrives at the platform lip rolled and
loses 490 km/h; the TAS arrives flat and loses nothing.** Roll at the lip
(z=1472) orders the whole field by finish time — flat candidates carry 856-864
km/h through it, the human WR arrives at roll −1.48 rad and carries 822. The
same story explains the wall clips: r03 and r04 clip the hole at y=66-67,
*higher* than the fast line's clean pass at 63.1 — they clip because they are
rolled 0.6-1.8 rad, not because they are low.

## 4. What the engine ignores — and the mistake that is worth more than the map

Forcing a constant through every phase and re-simulating (391 window overrides,
then a 5-tick scan of the transition) gives:

| phase | measured on the analog lineage |
|---|---|
| **t < 2.90 s** | **steering does nothing at all** — left, right and centre give the identical millisecond. Confirmed on the keyboard lineage too |
| t > 4.10 s | forcing the brake ON, or OFF, over any 20-tick window changes nothing |
| t = 13.50-13.95 s | forcing a constant steer changes nothing |

From the middle row I wrote, in v1 and v2 of this report, *"the brake does
nothing, anywhere, after 4.1 s — including through the ground contact that feeds
the cannon, where the human holds it down"*, and I told the other agent to
delete every brake event in their tape.

**That is false, and it is false in an instructive way.** They tested it instead
of taking it, and then I reproduced it on my own lineage:

```
brake removed over ticks [450,1550)  -> DNF
brake removed over any single sub-window (4.1-6.6, 6.6-9.0, 9.0-11.0, 11.0-end)
                                      -> 13.984, free, every time
brake held ON from 6.53 s to the end  -> 13.991
```

**Window-local inertness does not compose into global inertness.** The brake is
genuinely load-bearing; it just tolerates ~70 ms of slack in *when* it comes on,
and a window sweep reads exactly that tolerance as "no effect". Any sweep that
perturbs one window at a time can only ever prove "no single window matters" —
which is a much weaker statement than the one it looks like.

Two of the three dead zones also turned out to be **lineage properties, not map
properties**: on the keyboard tape the late-steering zone costs 1 ms rather than
nothing. This is the same lesson as the timing plane in §6 — *arm it per
lineage, not per map* — and it applies to every claim of the form "the engine
ignores this axis here".

The safe residue, verified on both lineages: **steering before 2.90 s is free**,
and the tape's only real job is between **2.9 s and 13.5 s**, with the decisive
part between **6.1 s and 8.5 s**: aiming the cannon.

## 5. The route a human can copy

**Phase 1 — 0 to 6.2 s.** Hold accelerate. Hold left, then right at ~1.6 s.
Steering does nothing before 2.9 s, so the only input that matters here is the
throttle.

**Phase 2 — 6.2 to 11.0 s: the launch, the only thing you really steer.** A
handful of taps around the redirect ramp, then **brake on at ~6.5 s and off at
~9.5 s** (both are load-bearing; the timing tolerates about 70 ms). What you are
aiming for is to leave the cannon pointed so that you cross z=976 at **x ≈ 171**
— a little LEFT of the corridor centre (176) — and **flat**.

**Phase 3 — the hole, and then ride it out.** You clear the wall at y≈63, land
at z≈1315 and slide in. 8513 forced-input variants over the whole landing and
slide, and 9113 more with graded steering over the touchdown, changed the finish
time by **zero**. The run is decided before you get there.

The three things that separate 13.984 from 14.018:

1. **Land LEFT of centre** — x≈171-175 at the finish, not x≈182. At 182 you hit
   the platform lip nose-on and lose 490 km/h. The record holder does exactly
   that and still wins, which is how much slack was in this map.
2. **Stay flat.** Roll ≈ 0 through the hole and over the lip. Every human
   arrives rolled 1.2-2.8 rad.
3. **Do not try to save it late.** There is nothing there.

Full input scripts: `lowinput/kb330_31ev_13984.tick.txt` (the optimum, 31
inputs) and `lowinput/kb330_12ev_13986.tick.txt` (12 inputs, 13.986, still
inside the AT). The human WR uses 46 inputs and is 34 ms slower than the first.

## 6. What was ruled out, and how

| claim | evidence |
|---|---|
| the approach (ticks 0-620) can be improved | 15 000 unbiased random moves, **zero** improvements |
| the cannon can be entered faster | it CLAMPS: 999.8 km/h for every human and every candidate |
| passing the hole higher lengthens the arc and pays | **backwards**: +1.5 ms per metre of extra height, measured three ways; the fast line already passes at the population floor (63.1 m) |
| the 22 ms lost at touchdown can be flown instead | needs 47 m more altitude at z=1300 = 3-4° of launch angle = 70-90 ms of vz, and it does not fit through the hole. 13.963 is not a reachable bound |
| the Spring items at z=1186-1274 are a route | they sit at y=8-18; the arc is at y=21-32 there. Recovery aids, not a line |
| a different seed finds a different basin | all five records converge to 13.986 within 2.5 min, including the 21.230 wall-clipper and the 23.153 overshoot. **One basin** |
| a better lateral line exists | 5601 finishing variants spanning the reachable launch-direction range: none below 13.984 |
| the endgame has control authority | 8513 + 9113 single-window overrides: best **+0 ms** |
| some move worth a millisecond remains | exhaustive one-move neighbourhoods: **212 341** analog perturbations at full tick and full steer resolution over the whole endgame, and **2282** covering the entire keyboard neighbourhood of the 31-input tape — adjudicated by the plain oracle, none beats 13.984 |

## 7. The instrument that mattered, and how to know when to trust it

Every exhaustive sweep above returned "+0 ms", and I read that as "the physics
has no authority here". **It meant "no move worth a whole millisecond", and the
millisecond was the ruler, not the physics.** At this map's finish speed 1 ms is
23.8 cm. Re-scoring finishers on a sub-tick timing plane at the finish
(`--plane 1507.012 --plane-axis 2 --plane-dir 1`, generalised from the 191465
agent's -x-only version) turned the objective into microseconds and 13.985 →
13.984 fell out of a search that had been flat for 2 million evaluations.

**The test for whether a timing plane is trustworthy is one command with a
budget.** The validator reports `ceil(t_true)`, so measuring the crossing
coordinate at each tape's own validated millisecond costs a uniform
`[0, v × 1 ms)` of spread *by construction*:

| map | v at the finish | budget `v × 1 ms` | measured spread | verdict |
|---|---|---|---|---|
| 227969 | 67 m/s | 0.067 m | 1.30 m (19x) | the plane lied by ~19 ms |
| **203330** | **238 m/s** | **0.238 m** | **0.233 m** | **the plane is a measurement** |

With the excess removed the plane agreed with the truth to **±0.02 ms** across
six tapes, and every plane reading predicted the validator's integer correctly.
It is still a gradient and not a score: every promotion in this report is a
plain-oracle millisecond.

Two limits found the hard way:

* **The finish trigger is not one z-plane for the whole map.** The keyboard
  lineage crosses at x≈183 and triggers at z≈1507.20; the analog lineage crosses
  at x≈172.6 and triggers at z≈1507.01 — ~0.2 m, ~0.9 ms, monotone in lateral
  offset. Arm the plane **per lineage**, and re-derive it when the search moves
  the crossing point sideways.
* **A guard tuned on a surrogate can quarantine your best result.** With the
  plane armed the score is microseconds and the validator returns its `ceil`, so
  the phantom guard's equality test fires on every improvement. Relaxed to the
  `ceil` relation it still fired on the keyboard lineage — because of that
  lateral dependence — and quarantined **35 tapes that all validate at 13.984**.
  They survived only because `--phantom-continue` was on. Keep the guard; make
  it quarantine rather than delete; harvest the quarantine.

## 8. Our tools were wrong on this map in eight places

None were physics and every one read like physics. This car flies at **278 m/s**;
every threshold in the trajectory stack was tuned on a 100 m/s ground car.

| tool | the gate | what it did | fix |
|---|---|---|---|
| `blind::locate` | `step/dt > 200 m/s ⇒ not a position` | rejected the real vehicle state as impossible | `FK_MAXSPEED` |
| `blind::locate` | swept `base − 603 616 ± 1.5 MB` | here the vehicle struct is at **base − 5 778 064**; never covered | `FK_BLIND_CENTRE`, `FK_BLIND_SPAN` |
| `blind::locate` | first hit with `vel_err < 1.5`, `mean_speed > 1.0` | a nearly static decoy beats a 226 m/s state | `FK_MINSPEED`, `FK_VELERR` |
| `blind::locate` | `vel_err > 5.0 ⇒ refuse` | the true state measures 5.8 (2% of speed); **all 56 workers aborted** | scaled to `max(5, 0.03·v)` |
| `traj::qualify` | `rms < 0.05 m && max < 1.0 m` | reference telemetry is 13.9 m apart here; the TRUE state measures rms 0.122 / max 1.74 (decoys 566-1088 m) | `FK_RMS_GATE`, `FK_WORST_GATE` |
| `layout::check_rows` | `vel_err > 2.0 m/s` | the same relative fidelity gives 2.05 m/s at this speed | `max(2.0, 0.02·v)` |
| fork-server clock | `clock = 36141 + 25.483·ms` (map 2) | this map is `5431 + 26.49·ms`; a "tick 600" checkpoint lands ~110 ticks late | `FK_CLOCK_A`, `FK_CLOCK_B` |
| sub-tick plane | crossing tested only in **−x** | this finish is approached in **+z**; the plane never fires and the objective silently stays integer-ms | `plane_axis`, `plane_dir` |

**Before trusting a measurement stack on a map faster, bigger or stranger than
the one it was tuned on, run its own acceptance test first** — `fk fs --mode
test`, and `fk traj` against a ghost whose telemetry you already have.

## 9. Correctness

- Identity control in every batch; the candidate factory round-trips.
- The fork server is **exact** here: 200/200 at boundary tick 620, 250/250 at
  1000, 250/250 at 1200, 200/200 at 1450, against full `/validatepath` of the
  same tapes. Throughput 0.83x-1.25x, so it was adopted for the plane, not speed.
- Every claimed improvement re-validated through the plain oracle with absolute
  paths before adoption. **Zero phantoms** in ~90 banked tapes, including all 35
  the guard quarantined.
- Searches ran on the hardened build (barrier-computed mutation floor, per-pid
  roots, phantom guard on).

## 10. Artefacts

```
203330/
  map.Map.Gbx  map.json  lb.json           the map and its metadata
  ghosts/                                  all five human records
  best/an330_13984.Ghost.Gbx               THE RESULT, analog        13.984
  best/kb330_13984.Ghost.Gbx  + .tick.txt  keyboard, as found        13.984
  lowinput/kb330_31ev_13984.Ghost.Gbx      keyboard, thinned, 31 in  13.984
  lowinput/kb330_12ev_13986.Ghost.Gbx      keyboard, 12 inputs       13.986
  traj/*.csv                               true per-tick trajectories
  tools/                                   the toolchain as patched for this map
  PLAN.md  NOTES.md                        the pre-search argument and the log
  RESULT-lowinput-and-seeds-v1.md          the second agent's half
  RESULT-v3.md                             this file (v1/v2 superseded)
```
