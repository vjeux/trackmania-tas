# Map 145875 — "unluckE - get jiggy with it" — RESULT

**The author time has fallen, and the fastest known tape needs no analog input.**

| | ms | vs AT |
|---|---|---|
| Author time (unbeaten since 2023-12-09) | 6343 | — |
| Best human online WR (xeap-.-, 46 records) | 6346 | +3 |
| **Our best, re-validated** | **6322** | **−21** |
| **Our best KEYBOARD-ONLY tape, re-validated** | **6323** | **−20** |
| Our best keyboard tape reduced to **23 inputs** | 6323 | −20 |
| The human WR's own tape with **ONE** input changed | 6342 | −1 |

uid `_GsJKvxawnKoIgkiWCpy9tRIMM0` · Nadeo mapId `56c24403-891e-4ffc-a9f0-2bd9ff98ae27`
· author **InfTM** · TMX id 145875. Nothing was submitted to any Nadeo
leaderboard; the deliverable is the replays and this document.

Artefacts in `~/persistent/private-30d/tm-unbeaten/145875/`:

| path | what |
|---|---|
| `best/BEST_6322.Ghost.Gbx` | the floor, 6322 ms |
| `best/BEST_KEYBOARD_6323.Ghost.Gbx` | 6323 ms, steering only ever −127 / 0 / +127 |
| `tapes/KEYBOARD_23ev_6323.Ghost.Gbx` | **the one to hand a human**: 6323 ms, 23 inputs, 3 values |
| `tapes/HUMANWR_plus_early_flick_6342.Ghost.Gbx` | the human WR + one changed input |
| `tapes/relax0_33ev_6330.Ghost.Gbx`, `tapes/floor37_6330.Ghost.Gbx` | the analog family |
| `map.Map.Gbx`, `ghosts/`, `btraj/`, `evidence/` | map, the 15 human seeds, decoded per-tick trajectories, every validation table |
| `PLAN.md` | pre-search analysis + the phantom incident |
| `tmtas-rs-src-patched.tgz` | the tool work done here |

Every number above was re-validated through the plain oracle, and again
independently under the hardened build:

```
tmtas validate --map …/map.Map.Gbx …/best/BEST_6322.Ghost.Gbx      -> 6322
tmtas validate --map …/map.Map.Gbx …/tapes/KEYBOARD_23ev_6323…     -> 6323
```

---

## 1. Headline findings

1. **6322 ms**, 21 ms under an author time nobody had touched in 20 months.
2. **A pure keyboard tape reaches 6323 ms** — 1 ms off the unconstrained floor.
   This map does not need a pad. That is unusual and it is the most useful thing
   here: 8 of the 13 fastest humans are already on a keyboard, and the best of
   them is 37 ms slower than a keyboard can go.
3. **The keyboard tape reduces to 23 change events for zero cost**, with the
   throttle held down for the entire run and the brake never touched.
4. **One single changed input takes the human WR's own tape from 6346 to 6342**,
   i.e. under the author time, and that input is the most forgiving one on the
   map. If you want the cheapest possible advice for the field, it is that.
5. Both bests are **exhaustively 1-move optimal**: 249,747 candidates at full
   tick and full 255-value resolution found nothing better than the analog
   floor, and 2,448 candidates found nothing better than the keyboard 6323.

## 2. Incident: phantom finishes — found, root-caused, fixed, reported

The first pair of search arms produced five sub-AT bests of which **four did not
re-validate** (three DNF, one that re-simulated to 6346 — the untouched
template's own time). Treated as a STOP; specimens preserved in
`~/persistent/private-30d/tm-loop/phantoms/m145875_20260818/`.

Cause: two `tmsearch` processes launched without `--root`, both using the
hardcoded default `/dev/shm/tmsearch`. Worker directories are named by index, so
each process's server validated whichever tape the other had just written and
the time came back attached to the wrong state. Controlled A/B:

| staging root | bests | re-validate | phantoms |
|---|---|---|---|
| shared | 13 | 6 | **7** |
| distinct | 8 | **8** | **0** |

Fixed in the tool (per-pid default root; `O_EXCL` claim; exit 9 rather than
wiping a live search; claim before mode dispatch so the fork path is covered;
same guard on `--bestdir`). Verified: 3 simultaneous launches on one root →
exactly 1 proceeds. Adopted upstream into the hardened build with the four
review amendments applied.

**Second, self-inflicted:** the tape clock and the race clock differ by
`start_offset_ms = −1540 ms`, so the 6.34 s race ends at tape tick **787 of
789**, not 634. My first three arms used `--hi 660` and never mutated the final
1.27 s. Re-running over the full range found nothing further, so nothing was
lost — but the sign of that offset is a trap.

**Third, checked and clean:** after the 252289 report of an alphabet transform
being applied in the evaluator but not to the stored incumbent, every
keyboard-constrained best produced here was re-validated. **13 of 13 returned
the exact time in their filename** (6323, 6330, 6331, 6332, 6333, 6335, 6337,
6338, …), and each best is byte-identical after re-snapping to
{−127, 0, +127}. `--qlevels` does not have that defect on these runs.

## 3. What kind of map this is

**No checkpoints at all.** One Spawn (`PlatformTechStart`) and one Goal
(`GateExpandableFinish` + two `GateFinishCenter8mv2` items at (1230,158,820) and
(1232,158,820)). Every ghost declares a single split equal to its finish time.
So a DNF carries no information — there is no checkpoint ladder to fall back on
— and the finish is a **trigger volume**, not a plane, which turns out to be
where the time is.

Medals: author 6343, gold **7000**, silver **8000**, bronze **10000**. Round
numbers to the second: the author hand-typed the lower three and left the author
medal as what the TM2020 editor always makes it — the time they actually drove.
6343 is a lap a person completed, 3 ms better than the best of 46 online
attempts by everybody else.

The route (`fk btraj`, 10 ms per tick):

| phase | race ms | what happens | km/h | y (up) |
|---|---|---|---|---|
| S0 | 0–1370 | roll off the start block and drop a near-vertical face; x moves 5 m while y falls 22 m | 1 → 111 | 137 → 115 |
| S1 | 1370–2200 | airborne, falling | 111 → 239 | 115 → 76 |
| S2 | 2200–3400 | violent landing (+317 m/s² along x in one tick) redirects the car into +x; gear 1→4; still descending | 239 → 299 | 76 → 42 |
| S3 | ~3400–3450 | the bottom kicker: +254 m/s² vertical in one tick; the climb starts | 299 → 269 | 42 (min) |
| **S4** | **3450–6330** | **airborne throughout, rolled ~180°, climbing 115 m while accelerating to 612 km/h** | 269 → **612** | 43 → 155 |

S4 is neither ballistic nor engine-on-tarmac. All four dampers read fully
extended (no wheel load) for its whole duration, yet the net acceleration has
**constant magnitude ≈ 50 m/s²** (9.81 removed) and — the decisive measurement —
**constant direction in the car's own frame: 65.6°–72.1° off the nose over 2.3 s
while the car rotates through most of a roll revolution.** A body-fixed force of
fixed magnitude is a reactor/boost push, and it means the car's ATTITUDE aims
it. In S4 steering does not steer the car; it points the thrust.

## 4. Where the time is, and what a human would have to do

### The line is not the answer — the gate entry is

Our tape and the human WR are the same line, within a metre, for 96 % of the
lap. Plane-crossing splits, both from `fk btraj`:

| | y=130 | y=110 | y=90 | y=70 | x=900 | x=1000 | x=1100 | x=1150 | x=1200 | x=1229 |
|---|---|---|---|---|---|---|---|---|---|---|
| ours − WR (ms) | +0 | +0 | +0 | +0 | +0 | +0 | +1 | −1 | −3 | −5 |

Nothing in the fall, nothing through the landing; the gap opens only in the last
1.5 s of the climb, and even at x = 1229 it is only −5 ms. The rest is
geometric. **The finish trigger is tilted, and arriving higher trips it earlier.**
Interpolating each run's state at its own finish time, over the eight runs that
cross cleanly at 550+ km/h:

| run | x at finish | y at finish | km/h |
|---|---|---|---|
| **ours (6330 tape)** | **1228.78** | **154.62** | **612** |
| r13 6452 | 1229.49 | 154.80 | 552 |
| r10 6440 | 1229.93 | 154.72 | 599 |
| r08 6413 | 1229.92 | 153.39 | 600 |
| r01 6346 (WR) | 1230.50 | 153.62 | 609 |
| r07 6408 | 1230.45 | 153.85 | 619 |
| r15 6478 | 1231.34 | 152.51 | 600 |

Across the field, each extra metre of height at the gate buys roughly a metre of
x. Our run is 1.0 m higher and 4 km/h faster at the line than the human WR, and
therefore trips the finish **1.7 m earlier in x** — about 10 ms at 167 m/s — on
top of the 5 ms it was already ahead.

### What we press differently — measured, and ablated

`tmtraj inputdiff` over the two per-tick input streams: 26 differing stretches
in 659 ticks, in four clusters. A full 2⁵ ablation, grafting each subset from
our tape into the human WR's:

| grafted | ms | vs WR |
|---|---|---|
| (nothing — the WR) | 6346 | +0 |
| two brake taps at 0.58 s and 0.76 s | 6346 | **+0** |
| final left→right flip fired 1 tick later | 6347 | +1 |
| the 2.54 s right-hand flick, alone | 6373 | **+27** |
| 98 % instead of 100 % left lock at 3.52 s, alone | 6347 | +1 |
| **the 2.54 s flick AND the 3.52 s lock level, together** | **6330** | **−16** |

The brake taps the search invented are worth exactly nothing — search noise, not
technique. The whole 16 ms of the analog tape is a **non-separable interaction
between two small analog details** whose only effect is to leave the bottom
kicker at a marginally different attitude, which 2.8 s of body-fixed thrust then
amplifies into a metre of height at the gate. Either one alone is worse; one of
them by 27 ms. That is why nobody found it.

### Classification: it looked precision-bound, and it is not

The analog route to the time is genuinely precision-bound — two stick positions
2 % off the stops, 1 s apart, that only pay together. But that is our tape's
route, not the map's. Two measurements settle it:

**(a) The brittleness is the map's, not ours.** Measuring open-loop timing
tolerance — slide one gesture, or one gesture and everything after it, and
re-simulate — the human WR's own tape fails at ±1 tick on 5 of its 6 gestures,
and the best human keyboard run fails at ±1 tick on 9 of its 11. Our tape is not
twitchier than theirs; it is slightly *less* so. A tape is an open-loop
recording, and 46 people finish this map by reacting to what they see. So
open-loop tolerance is the wrong instrument for the first 3.4 s, and "our tape
needs 10 ms precision" is not a statement about the technique.

**(b) A keyboard does it.** `tmtraj slew` over the 13 top human runs measures how
fast the recorded steer value moves per 10 ms tick and splits the field cleanly:

| population | runs | max Δsteer/tick | median non-zero Δ |
|---|---|---|---|
| **keyboard / digital** | r03, r06, r08, r09, r10, r11, r14, r15 (8 of 13) | **1.0, twice 2.0** | **1.00** |
| pad / analog | r01 (the WR), r05, r07, r12, r13 | 0.44–0.79 | 0.04–0.15 |

Two runs move from −127 to +127 inside a single tick, so TM2020 does not ramp a
held key: `{−127, 0, +127}` is the real keyboard alphabet, not an idealisation
(control: snapping r03's tape to those three values changes its time by 0 ms).
Searching **under** that constraint from a keyboard seed reached **6323 ms** — 1
ms off the unconstrained floor and 20 ms under the author time.

So the honest classification is **known-but-unheld, plus one undiscovered
detail**: the field already drives this map on a keyboard and already drives
this line; what none of them do is aim the last climb high into the tilted
finish trigger. There is no analog magic required.

*(A negative worth recording: converting our analog tape to keyboard does not
work. Replacing each of its four analog sweeps with the single instantaneous
step a keyboard physically produces — 82 placements across the four sweeps —
produced not one finishing run. The pad and keyboard lines are different basins,
as cross-splicing was on maps 1 and 2. A keyboard strat has to be searched as
one.)*

## 5. The low-input family

All re-validated through the plain oracle. "Events" counts input CHANGE events;
a value held 40 ticks is one event.

| tape | ms | vs AT | events | steer alphabet | notes |
|---|---|---|---|---|---|
| `BEST_6322` | **6322** | −21 | 186 | 125 analog values | the floor; 1-move optimal over 249,747 candidates at full resolution |
| `BEST_KEYBOARD_6323` | **6323** | −20 | 47 | **3** | keyboard only; 1-move optimal over 2,448 candidates |
| **`KEYBOARD_23ev_6323`** | **6323** | **−20** | **23** | **3** | **the drivable one — reduction cost 0 ms** |
| `relax0_33ev_6330` | 6330 | −13 | 33 | 26 | analog, reduction cost 0 ms |
| `floor37_6330` | 6330 | −13 | 37 | 29 | analog, zero-loss reduction of the raw search output |
| `HUMANWR_plus_early_flick_6342` | 6342 | −1 | WR's + 0 | WR's | **one changed input on the human WR's own tape** |
| (raw search output, for scale) | 6330 | −13 | 185 | 125 | 148 of those 185 events are deletable noise |

### The cheapest advice in the whole document

Take the human WR's tape and change **one thing**: fire the final flick to full
right about 0.1 s earlier, and hold ~80 % rather than 100 %. Nothing else. Full
enumeration of that single input, tick × value:

```
  fire\hold     90    100    110    118    124    127
       -10    6342   6342   6343   6382   6356
        -9    6343   6342   6342   6343   6348
        -8    6343   6342   6342   6342   6342     <- a broad 6342 plateau
        -7    6344   6343   6342   6342   6342
        -6    6345   6343   6343   6343   6342
        ...
        +0    6348   6347   6347   6347   6346     <- what the WR does
```

6342 beats the author time, and it is reachable from a whole block of
(timing, value) pairs spanning 50 ms and most of the top half of the stick. This
is the single most forgiving input on the map.

## 6. Sector-by-sector guide — `KEYBOARD_23ev_6323`, off visual cues

**6323 ms · 23 inputs · keyboard only · accelerate held from the countdown to
the line · brake never touched.** Everything below is steering.

| # | when | input | the cue |
|---|---|---|---|
| 1 | before the lights | **hold LEFT** | you are aimed at a near-vertical drop; hold left through the launch and all the way down the face |
| 2 | 1.45 s (~110 km/h, the wall runs out and the car goes light) | **RIGHT**, hold 0.5 s | you are now falling |
| 3 | 1.97 s | release to **centre** | |
| 4 | 2.09 s | **LEFT** into the landing | the landing is violent — the car is thrown along +x |
| 5–9 | 2.25–2.51 s | **five short LEFT/centre taps** (50, 30, 30, 10, 10 ms) | this is the keyboard's way of holding a partial steer through the landing: pulse, don't hold |
| 10 | 2.51 s | **centre**, 0.2 s | |
| 11 | 2.71 s | **RIGHT**, 80 ms — the flick | short stab |
| 12 | 2.79 s | **LEFT**, 0.32 s | |
| 13–14 | 3.11 s | brief **centre**, then **RIGHT** 0.21 s | you are near the low point of the map |
| 15–20 | 3.34–3.59 s | **LEFT with two short releases** (20 ms each, at 3.48 and 3.57 s) | the surface turns up under you — this is the kicker |
| 21 | 3.59 s | **hold LEFT for 1.6 s** | through the kicker and the first half of the climb; the car goes inverted and the thrust takes over |
| 22 | **5.21 s** | **RIGHT, and hold it to the line** | the aim into the gate — **aim high.** The trigger is tilted; every metre of height trips the clock about a metre further back. Our whole margin is 1 m of height and 4 km/h at the gate. |

**Which sections are humanly realistic.** Input 22 is generous: measured
tolerance ±3 to +6 ticks (up to 90 ms) with no change in time, and on the human
WR's tape the equivalent input has a 50 ms × wide-value plateau. Inputs 1–4 and
21 are holds — start them off the visual cue and they are fine. **Inputs 5–20,
the pulsing between 2.25 s and 3.59 s, are the hard part**: eleven short inputs
in 1.3 s, several of them 10–30 ms long, and open-loop they have no slack at
all. That said, the human WR's own tape and the best human keyboard run measure
exactly the same way through that stretch, and 46 people finish the map — so
this section is drivable by reacting, not by counting. Expect it to be where the
attempts die, and expect to learn it by feel.

**What to practise first, if you only want the author time and not the record:**
the one-input change in §5. It is the human WR's run with the last flick a
tenth of a second earlier.

## 7. Negative results, recorded so nobody repeats them

- **Multi-operator search does not beat single-operator here.** Motivated by the
  ablation (the analog gain is a two-part interaction that a single move cannot
  propose), `--nops -3` was run against a `--nops 1` control, concurrently, same
  box, 45 min, 45 workers each, both seeded at 6330: **both ended at 6330, zero
  improvements, ~540 k evaluations each.**
- **A second seed basin is worse.** An arm seeded from r04 (6373), whose S4 line
  reaches x = 1150 seventy ms earlier than the WR's, converged to 6353 while the
  r01-seeded arms were at 6330. Being further along x is not being closer to the
  gate.
- **Searching only the final 1.9 s is barren.** An arm restricted to ticks
  600–789 had a 95 % finish rate and produced no improvement in 285 k
  evaluations — the aim at the gate is set before it.
- **Quantizing a finished analog tape does not work**, at any level: 3, 5 and 9
  levels all DNF outright, and greedy per-run substitution converted 0 of 25
  runs. Independently confirmed on 227969 down to a 64-level ladder.
- **Robustness hill-climbing found nothing to buy.** Maximising total timing
  slack subject to the time budget converged in one round with no improvement.
- **The fork server was not used.** On a 6.3 s map a full re-simulation is cheap
  (~1,400 evals/s across the box), all four defects found in this project's
  fork path were live during this work, and the classic path with distinct roots
  measured 33 of 33 banked bests re-validating exactly. Independently confirmed
  on 252289: below ~4 s the fork server is slower than starting over.

## 8. Tool work produced here

In `tmtas-rs-src-patched.tgz`; the staging-root fix is already upstream in the
hardened build.

| addition | what it does |
|---|---|
| `tmsearch` per-pid `--root` + `claim_root` | the phantom fix (§2) |
| `tmsearch --simplify` | tail freeze, alphabet walk, greedy event deletion, grid snap, per-event and per-**gesture** timing tolerance |
| `tmsearch --ablate` | full 2ᵏ subset grafting between two tapes — the tool that found the two-part interaction |
| `tmsearch --fsweep` | enumerate the last gesture's tick × hold value; produced the one-input human strat |
| `tmsearch --digital` | keyboard-alphabet search (superseded by the add-on's `--qlevels`, which is hooked better) |
| `tmtraj planes` | plane-crossing splits — virtual sectors for a map with no checkpoints |
| `tmtraj finishpt` | each run's state at its own finish time — how the trigger volume was mapped |
| `tmtraj thrust` | differentiate velocity, remove gravity, report the residual force's angle to the car's own axes — how S4 was identified |
| `tmtraj inputdiff` | tick-by-tick input diff between two runs, collapsed to stretches |
| `tmtraj slew` | steer change per tick — how the field was split into keyboard and pad |
