# 279197 "Fall 2025 - 01 Reverse CP1 End" — RESULT

**The author time has fallen.**

| | ms | note |
|---|---|---|
| best human online WR (ShcrTM, rank 1 of 561) | 10602 | |
| **author time (AT)** | **10598** | unbeaten by 561 human runs |
| **this TAS, validated on the untouched map** | **10595** | AT − 3 ms, human WR − 7 ms |

Reproduce the claim:

```bash
tmtas validate --map ~/persistent/private-30d/tm-unbeaten/279197/map.Map.Gbx \
               ~/persistent/private-30d/tm-unbeaten/279197/best/real_10595.Ghost.Gbx
```

→ `10595`. Run the human WR in the same batch as an identity control and it
returns `10602`.

Nothing was submitted anywhere. The deliverable is the replay and this writeup.

## Artefacts

All under `~/persistent/private-30d/tm-unbeaten/279197/`:

| path | what |
|---|---|
| `best/real_10595.Ghost.Gbx` | **the run**. Validated 10595 ms |
| `best/best_10596.Ghost.Gbx`, `best_10597`, `best_10598` | the ladder down, each validated |
| `map.Map.Gbx`, `map.json` | the map and its metadata |
| `ghosts/*.Ghost.Gbx` | 27 human ghosts, ranks 1-15 / 51-53 / 151-153 / 301-303 / 501-503 |
| `lb_*.json` | the leaderboard slices they came from |
| `PLAN.md`, `NOTES.md` | the plan argued from evidence, and the running notes |
| `tmtas-rs-279197-patched.tgz` | the toolchain with this map's additions (see below) |
| `tools/*.sh` | `rank.sh` (the vernier), `ratchet.sh` / `ratchet2.sh` (the loop) |
| `logs/control_realmap_w1a.txt` | the concurrent control run, in full |

## How it was obtained

**1. A plain search from the human WR got 6 ms in seven minutes.** `tmsearch`
seeded with the rank-1 ghost, 96 workers, `--ops mix2`, T = 12 ms: 10601 at 8 s,
**10598 (= the AT) at 28 s**, 10597 at 6.5 min, 10596 at 7.0 min. Then it
stopped, completely, for the next 1.7 M evaluations.

**2. It stopped because the objective ran out of resolution, not because the
run ran out of time.** The oracle answers in whole milliseconds. On this map the
car crosses the line at its terminal speed of 94.9167 m/s, so one millisecond is
9.49 cm — and the reported millisecond is quantised into *uneven* bins up to
15 cm wide (measured: 0.042 / 0.042 / 0.144 / 0.096 m for consecutive values on
the human WR, with the value 10599 unreachable altogether). Any real improvement
smaller than the current bin is invisible, and the search spends its whole life
random-walking a plateau.

**3. So the finish plane was turned into a ruler.** These "CP1 End" community
maps have exactly one waypoint and it is a *relocatable item* — here `cp1end`,
a repurposed roadborder at (800, 56, 768), yaw = −π. `tmmaps` can move it, which
means the finish plane is ours to place, and a tape can be re-timed against a
plane put wherever we like. Ranking tapes by *the largest plane offset at which
each still reports ≤ T* measures who is genuinely further along, at 0.05 ms
resolution — twenty times finer than the oracle's own answer, and with no model,
no interpolation and no calibration anywhere: every number is the game's own
body-based trigger firing on the game's own physics.

**4. The ratchet.** Each cycle: measure the champion's staircase edge; build a
map with the plane a hair *past* it, so the champion sits one millisecond above
the threshold and the smallest true gain reads as a whole millisecond; run
several arms from the champion on that map; rank the survivors on the fine
ladder; adopt the furthest-along tape; re-aim. The plane only ever measures —
every claim is re-made with `tmtas validate` on the untouched map, which is also
how the ratchet knows when a real millisecond has fallen.

**The controlled comparison** (concurrent, same box, same seed tape, PROTOCOL
discipline): a real-map arm of 44 workers ran alongside the ratchet for
**41.9 minutes and 1,337,400 evaluations and never left 10596**. In the same
wall-clock window the ratchet advanced its champion four times and took the real
map to 10595. The vernier's prediction record is 4 for 4: ladder position below
the real plane → real map unchanged (twice), past it → real map dropped a
millisecond, and back below it against the new target → unchanged again.

## What the run does, and how it differs from the human WR

The map is a 10.6 s, 597 m, standing-start sprint that ends at what was the
campaign map's first checkpoint. **Full throttle the entire way: one single tick
of throttle-off (tick 0, which every human tape also has) and no brake at all,
in the human WR and in ours alike.** Everything is in the steering.

| t | what happens |
|---|---|
| 0 | launch at (272, 66.0, 656), gear 1 |
| 0.7–3.0 s | a long left-hand bend |
| **3.0–4.2 s** | the chicane: a hard flick from −102 to +61 steer at 3.2–3.6 s |
| 3.6–5.8 s | downhill, y 66 → 58, gear 3, 160 → 235 km/h |
| 5.8–7.8 s | flat straight, gear 4, 235 → 281 km/h |
| **7.8–10.6 s** | one 140 m-radius right-hand sweeper, flat out, 286 → 341.7 km/h |
| 10.595 | crosses the plane at ≈ (772.3, 58.0, 750.7), still accelerating |

Our tape differs from the human WR on **290 of its 1061 ticks**, and the changes
are not spread evenly:

| second | ticks changed |
|---|---|
| 0-1 | 0 |
| 2 | 2 |
| **3** | **55** |
| 4 | 30 |
| 5-6 | 0 |
| **7** | **48** |
| **8** | **42** |
| **9** | **66** |
| 10 | 47 |

Two regions: **the chicane flick (3-4 s)** and **the whole sweeper (7 s to the
flag)**. The bend, the downhill and the straight are driven exactly as the human
drove them — the TAS found nothing there. Largest single steer change: 96 of
255. Both tapes are fully analog (157 and 166 distinct steer values); this is
not a keyboard-vs-pad story.

## The three things worth knowing about this map

### 1. Ninety-five per cent of the human field's spread is decided before t = 9.5 s

Relocating the Goal to build a ladder of intermediate planes and timing the
whole population through it:

| run | z=655 plane | → flag |
|---|---|---|
| r001 (WR) 10602 | 9502 | **1100 ms** |
| r008 10608 | 9505 | 1103 |
| r015 10615 | 9512 | 1103 |
| r052 10628 | 9525 | 1103 |
| r152 10658 | 9548 | 1110 |
| r302 10724 | 9618 | 1106 |
| r502 10800 | 9698 | 1102 |
| **our 10595** | **9492** | 1103 |

From the world record to rank 502 — a 198 ms spread — **the last 1.1 seconds
costs everybody the same 1100-1110 ms.** The closing sweeper, which is the part
that looks like the hard bit, is worth nothing to practise. It is all already
lost or won by the time you reach that plane. Our TAS is 10 ms up on the WR
there and gives 3 ms back over the run home.

### 2. The finish trigger has an invisible inside edge — a DNF trap, not a pace-setter

The trigger is a plane with a **finite lateral window** (~23 m in x). Sliding the
gate sideways brackets where each run crosses it. The window's inside edge is at
**world x ≈ 772.18**, and the human WR crosses 0.35 m outside it. Cut inside and
the run simply does not finish.

That much is a real hazard. But it is *not* what limits the field's pace, and an
earlier version of this note said it was — wrongly. Measuring the clean margin
for all fifteen of the top 15:

```
r001 10602: 0.35 m   r006 10608: 1.15 m   r011 10612: 1.45 m
r002 10603: 0.15     r007 10608: 0.25     r012 10613: 1.35
r003 10605: 0.35     r008 10608: 0.85     r013 10614: 0.05
r004 10606: 0.45     r009 10611: 0.75     r014 10614: 0.55
r005 10607: 0.35     r010 10611: 0.35     r015 10615: 0.15
```

Thirteen milliseconds of time; **1.40 m of margin, uncorrelated**. The tightest
run in the field (r013, five centimetres from the edge) is 12 ms *slower* than
the WR. Our own 10595 crosses 0.24 m tighter than the WR and loses 3 ms over the
final 1.1 s doing it. Cutting the last corner harder is not the lever.

There is no visible cue for the edge, and inventing one would be worse than
admitting it: the sweeper is a single stock `RoadTechCurve5` block (block #797,
cell (20,15,18)), and x = 772.18 is 4.2 m outside the x = 768 cell boundary,
lining up with no seam, kerb or scenery edge in the map's block list.

### 3. The route has zero open-loop tolerance

Measured on the champion **and on the human world record's own tape**:

* every single-tick throttle lift tried — ticks 2, 5, 10, 20, 40, 80, 150, 300,
  500 — DNFs;
* throttle on tick 0 DNFs; sliding the whole tape one tick either way DNFs;
* rounding the steer trace to **even values** (a change of at most half of one
  of 255 steer units per tick) DNFs;
* sample-and-hold at 2 ticks DNFs.

And these are *mid-route* failures, not gate misses: the quantised tapes never
even reach the z=697 plane at t≈9.8 s.

**What this does and does not mean.** It means a recorded tape replayed blind
has no tolerance anywhere on this route — every input matters, which is a strong
statement about the map and the reason a search here DNFs 58% of the time. It
does **not** mean a human cannot drive it: the test kills the human WR's own
tape, and a human is a closed loop who sees the car drift and corrects on the
next frame, which is exactly the feedback that perturb-and-replay destroys. The
author drove 10598. This is a route that needs continuous correction, not one
that is impossible.

For the same reason there is **no low-input family to publish**. Every
simplification of the tape — coarser steering, held inputs — fails. That is a
result, not an omission.

## Honest statement of the risk, for anyone driving this

* The failure mode at the finish is **binary and invisible**. There is no warning
  and no partial credit: cross a few centimetres inside x ≈ 772.2 and the run
  does not register at all. Runs killed that way never reach a leaderboard, so
  the public field cannot tell you how often it happens.
* But do not chase that edge: the measurement above says being tighter does not
  make you faster. It only makes you more likely to lose the run.
* Practise the **first 9.5 seconds** — the chicane at 3.2 s and the entry to the
  sweeper. The run home is free.

## Negatives, dead ends and things deliberately not used

* **No fork server.** `fk fsprobe` works on this map but its `lroundf`-per-tick
  calibration is 308/tick here against map 2's ~857, and with the checkpoint
  corrected the blind locator shortlists 0 candidate vehicle states, so
  `fk btraj` cannot run. It was not pursued: the fork path's known open incident
  is that searches on it bank improvements that do not re-validate, and three
  times the throughput is not worth risking the integrity of a 4 ms answer.
  Every number here comes from the plain oracle.
* **No shaping.** With one waypoint every DNF returns the same "reached 1
  checkpoint" and nothing else, so a DNF carries no gradient. Intermediate
  planes could supply one, but the trigger is **directional**: a yaw = −π gate
  only fires on a +z crossing, which restricts usable planes to the final leg
  (z ≥ ~655, t ≳ 9.5 s). Rotating the gate to ±π/2 to catch the +x leg does not
  work — the trigger does not present a usable face at those yaws. So the deep
  DNF valley that guards e.g. "start one tick earlier" (worth up to ~10 ms in
  principle, since every human tape idles tick 0) is not crossable with the
  tools available. Recorded as the map's biggest unexploited lever.
* **Sweeper-localised search was the wrong place to look.** An arm confined to
  ticks 700+ has a much healthier finish rate (69% vs 37%) but the ladder says
  the time is earlier; that arm was retargeted to `--hi 720`.
* **A `move_gate` trap that silently DNFs everything**: `tmmaps
  segments::move_gate` swaps in the stock finish-gate item model, which on a map
  whose only Goal *is* a custom item deletes the finish outright. The first
  sweep returned DNF at every offset **including the identity placement**, which
  is what caught it. Fixed with `--keep-model`.

## Controls, all passing

1. **27/27 human ghosts** re-simulate to their exact leaderboard millisecond
   (10602, 10603, 10605 … 10798, 10800, 10800).
2. `tmsearch --verify` round-trips the WR tape to 10602 (candidate factory).
3. `tmtas patch` with no edit reproduces its input exactly (tape editor).
4. Relocated-gate maps put back at the item's own position return 10602 / 10598
   / 10800 for three different runs (gate machinery is a no-op when it should
   be).
5. `tmtas selftest` 10/10 on this node.
6. Finish attitude, the precondition for trusting a moved plane: 27/27 grounded,
   speed exactly 94.9167 m/s for 23/27, roll spread 0.2°, pitch spread 3.0°,
   y spread 46 mm — worst-case leading-point shift ~6 cm.
7. Every arm after the first carried an explicit distinct `--root`; the
   shared-`/dev/shm/tmsearch` phantom bug cannot apply.
8. **Zero failed re-validations across the whole session.** Nothing was written
   to `tm-loop/phantoms/`.

## Toolchain additions (in `tmtas-rs-279197-patched.tgz`)

| where | what |
|---|---|
| `tmmaps probe --keep-model` | move the map's own gate item instead of swapping its model — required on any CP1-End map |
| `tmmaps places` | build gate maps at arbitrary explicit placements and time many ghosts through all of them at once |
| `tmmaps places --rank <ms>` | the vernier: rank tapes by the largest plane offset at which each still reports ≤ ms |
| `tmmaps blocks [--near x,y,z --radius r]` | list the map's blocks, to ask what a driver can actually see somewhere |
| `tmmaps list` | now prints gate yaw |
| `tmtas patch` | surgical tape edits: `--set A[-B]:steer\|accel\|brake=V`, `--shift`, `--quant`, `--hold` |
