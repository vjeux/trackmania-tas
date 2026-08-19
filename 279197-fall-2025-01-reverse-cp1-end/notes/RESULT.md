# 279197 "Fall 2025 - 01 Reverse CP1 End" — RESULT

**The author time has fallen.**

| | time | note |
|---|---|---|
| best human online WR (ShcrTM, rank 1 of 561) | 10.602 s | |
| **author time (AT)** | **10.598 s** | unbeaten by 561 human runs |
| **this TAS, validated on the untouched map** | **10.594 s** | AT − 4 ms, human WR − 8 ms |

Reproduce the claim:

```bash
tmtas validate --map ~/persistent/private-30d/tm-unbeaten/279197/map.Map.Gbx \
               ~/persistent/private-30d/tm-unbeaten/279197/best/real_10594.Ghost.Gbx
```

→ `10594` (the oracle prints raw ms) = **10.594 s**. Run the human WR in the
same batch as an identity control and it returns 10.602 s. Done twice, in
separate processes; md5 of the tape `f62e7c61872ed27e81352937198ff384`.

Nothing was submitted anywhere. The deliverable is the replay and this writeup.

### What the replay file is, and is not

`real_10594.Ghost.Gbx` is an **input tape**. The 1061 per-tick
steer/accel/brake values in it are ours; the `CPlugEntRecordData` telemetry
block it carries is still the human WR's, inherited from the template the
candidate was stamped from. So the file re-simulates to 10.594 s in the oracle,
but **loading it in-game as a ghost would replay the human's motion, not this
run's**. To see this run drive you have to re-simulate the inputs — the
dedicated server's `/validatepath`, or `best/real_10594.tick.txt` imported into
the TICK TAS tool. Producing a ghost with true telemetry needs the fork server's
in-child state reader, which does not work on this map (see the dead ends).

## Artefacts

All under `~/persistent/private-30d/tm-unbeaten/279197/`. Filenames keep raw
milliseconds; the prose does not.

| path | what |
|---|---|
| `best/real_10594.Ghost.Gbx` | **the run**. Validated 10.594 s |
| `best/real_10594.tick.txt` | the same run as a TICK input script, 442 events over 1061 ticks; `tmsite verify` round-trips it EXACT (0 steer / 0 accel / 0 brake mismatches) |
| `best/real_10595.Ghost.Gbx` + `.tick.txt` | the previous milestone, also validated |
| `best/best_1059x.Ghost.Gbx` | the ladder down (10.596 / 10.597 / 10.598 / 10.600 / 10.601 s), each validated |
| `map.Map.Gbx`, `map.json` | the map and its metadata |
| `ghosts/*.Ghost.Gbx` | 27 human ghosts, ranks 1-15 / 51-53 / 151-153 / 301-303 / 501-503 |
| `lb_*.json` | the leaderboard slices they came from |
| `PLAN.md`, `NOTES.md` | the plan argued from evidence, and the running notes |
| `tmtas-rs-279197-patched.tgz` | the toolchain with this map's additions (see below) |
| `tools/*.sh` | `rank.sh` (the vernier), `ratchet*.sh` (the loop) |
| `logs/control_realmap_w1a.txt` | the concurrent control run, in full |

## How it was obtained

**1. A plain search from the human WR got 6 ms in seven minutes.** `tmsearch`
seeded with the rank-1 ghost, 96 workers, `--ops mix2`, T = 12 ms: 10.601 s at
8 s of wall clock, **10.598 s (= the AT) at 28 s**, 10.597 s at 6.5 min,
10.596 s at 7.0 min. Then it stopped, completely, for the next 1.7 M
evaluations.

**2. It stopped because the objective ran out of resolution, not because the
run ran out of road.** The oracle answers in whole milliseconds. On this map the
car crosses the line at its terminal speed of 94.9167 m/s, so one millisecond is
9.49 cm — and the reported millisecond is quantised into *uneven* bins up to
15 cm wide (measured: 0.042 / 0.042 / 0.144 / 0.096 m for consecutive values on
the human WR, with 10.599 s unreachable altogether). Any real improvement
smaller than the current bin is invisible, and the search spends its whole life
random-walking a plateau.

**3. So the finish plane was turned into a ruler.** These "CP1 End" community
maps have exactly one waypoint and it is a *relocatable item* — here `cp1end`,
a repurposed roadborder at (800, 56, 768), yaw = −π. `tmmaps` can move it, which
means the finish plane is ours to place, and a tape can be re-timed against a
plane put wherever we like. Ranking tapes by *the largest plane offset at which
each still reports ≤ T* measures who is genuinely further along, to 2 mm
(0.02 ms) — with no model, no interpolation and no calibration anywhere: every
number is the game's own body-based trigger firing on the game's own physics.

**4. The ratchet.** Each cycle: measure the champion's staircase edge; build a
map with the plane a hair *past* it, so the champion sits one millisecond above
the threshold and the smallest true gain reads as a whole millisecond; run four
arms from the champion on that map; rank the survivors on the fine ladder; adopt
the furthest-along tape; re-aim. The plane only ever measures — every claim is
re-made with `tmtas validate` on the untouched map, which is also how the
ratchet knows when a real millisecond has fallen.

It converted 10.596 → 10.595 s in three 5-minute cycles, then crawled the
champion from ladder position 767.935 to 768.000 over twelve more, at which
point the real map returned **10.594 s**. The plane's own reading and the
untouched oracle agreed at every step.

**Where it stopped, honestly.** Against the next target the ladder edge fell
back to 767.905 (a new millisecond is 95 mm of ladder away) and the rate decayed
from ~10 mm per 10-minute cycle to ~2 mm: after nineteen cycles the champion had
recovered only 20 of those 95 mm. Extrapolating the observed rate, 10.593 s is
several more hours of the same machine. Two independent signs say the map is
close to its floor for this lineage: the champion's clean gate margin is down to
**0.075 m** from the human WR's 0.35 m, so there is very little line left to
take; and the last 1.1 s of the run is already within 3 ms of the whole field's
common value, so there is nothing to reclaim there either. The remaining
headroom looks like **one more millisecond, not five**.

**The controlled comparison** (concurrent, same box, same seed tape, PROTOCOL
discipline): a real-map arm of 44 workers ran alongside the ratchet for
**41.9 minutes and 1,337,400 evaluations and never left 10.596 s**. In the same
window the ratchet advanced its champion four times and took the real map to
10.595 s. The vernier's prediction record over the whole session is perfect:
every time the ladder put the champion past the real plane the untouched oracle
dropped a millisecond, and every time it did not, it did not.

## What the run does, and how it differs from the human WR

The map is a 10.6 s, 597 m, standing-start sprint that ends at what was the
campaign map's first checkpoint. **Full throttle the entire way: no brake at
all, and no throttle lift anywhere, in the human WR and in ours alike.**
Everything is in the steering.

| t | what happens |
|---|---|
| 0 | launch at (272, 66.0, 656), gear 1 |
| 0.7–3.0 s | a long left-hand bend |
| **3.0–4.2 s** | the chicane: a hard flick from −102 to +61 steer at 3.2–3.6 s |
| 3.6–5.8 s | downhill, y 66 → 58, gear 3, 160 → 235 km/h |
| 5.8–7.8 s | flat straight, gear 4, 235 → 281 km/h |
| **7.8–10.6 s** | one 140 m-radius right-hand sweeper, flat out, 286 → 341.7 km/h |
| 10.594 s | crosses the plane at ≈ (772.25, 58.0, 750.7), still accelerating |

Our tape differs from the human WR on **339 of its 1061 ticks**, and the changes
are not spread evenly:

| second | ticks changed |
|---|---|
| 0-1 | 0 |
| 2 | 2 |
| **3** | **55** |
| 4 | 30 |
| 5-6 | 0 |
| **7** | **48** |
| **8** | **72** |
| **9** | **86** |
| 10 | 46 |

Two regions: **the chicane flick (3-4 s)** and **the whole sweeper (7 s to the
flag)**. The bend, the downhill and the straight are driven exactly as the human
drove them — the TAS found nothing there in tens of millions of evaluations.
Both tapes are fully analog (157 and 166 distinct steer values of 255); this is
not a keyboard-vs-pad story.

Where the eight milliseconds are, measured against the human WR on relocated
intermediate planes. Gates on the +x legs use yaw = −π/2 and gates on the
closing +z leg yaw = −π (the trigger is directional; see the dead ends):

| plane | t (WR) | WR 10.602 | our 10.594 |
|---|---|---|---|
| x = 300 | 1.87 s | 1.872 | 1.872 (0) |
| x = 350 | 3.39 s | 3.389 | 3.389 (0) |
| x = 400 | 4.51 s | 4.509 | 4.509 (0) |
| x = 550 | 6.88 s | 6.878 | 6.877 (−1 ms) |
| x = 600 | 7.56 s | 7.555 | 7.554 (−1) |
| x = 640 | 8.07 s | 8.067 | 8.065 (−2) |
| z = 672 | 9.50 s | 9.502 | 9.492 (**−10**) |
| z = 697 | 9.83 s | 9.826 | 9.818 (−8) |
| z = 717 | 10.06 s | 10.058 | 10.051 (−7) |
| z = 737 | 10.28 s | 10.278 | 10.270 (−8) |
| z = 757 | 10.49 s | 10.490 | 10.481 (−9) |
| flag | | 10.602 | **10.594 (−8)** |

**The entire advantage is made between t ≈ 8.1 s and t ≈ 9.5 s** — the entry and
first half of the closing sweeper. For the first eight seconds the TAS is within
2 ms of the human, and for the first 4.5 it is exactly level, despite having
rewritten 85 ticks of the chicane at 3-4 s: those changes buy no time at all,
they only set the car up. Then it takes 8 ms in 1.4 seconds, and hands 2 back
over the run home.

The previous milestone, 10.595, is identical to 10.594 through z = 717 and
differs only over the final 40 m, where it crosses 3.5 cm wider.

## The three things worth knowing about this map

### 1. Ninety-five per cent of the human field's spread is decided before t = 9.5 s

Timing the whole population through the intermediate planes:

| run | at the z=655 plane | → flag |
|---|---|---|
| r001 (WR) 10.602 | 9.502 | **1.100 s** |
| r008 10.608 | 9.505 | 1.103 |
| r015 10.615 | 9.512 | 1.103 |
| r052 10.628 | 9.525 | 1.103 |
| r152 10.658 | 9.548 | 1.110 |
| r302 10.724 | 9.618 | 1.106 |
| r502 10.800 | 9.698 | 1.102 |
| our 10.595 | 9.492 | 1.103 |

From the world record to rank 502 — a 198 ms spread — **the last 1.1 seconds
costs everybody the same 1.100 to 1.110 s.** The closing sweeper, which is the
part that looks like the hard bit, is worth nothing to practise. It is all
already lost or won by the time you reach that plane.

### 2. The finish trigger has an invisible inside edge — a DNF trap, not a pace-setter

The trigger is a plane with a **finite lateral window** (~23 m in x). Sliding the
gate sideways brackets where each run crosses it. The window's inside edge is at
**world x ≈ 772.18**, and the human WR crosses 0.35 m outside it. Cut inside and
the run simply does not finish.

That much is a real hazard. But it is *not* what limits the field's pace, and an
earlier version of this note said it was — wrongly. Measuring the clean margin
for all fifteen of the top 15:

```
r001 10.602: 0.35 m   r006 10.608: 1.15 m   r011 10.612: 1.45 m
r002 10.603: 0.15     r007 10.608: 0.25     r012 10.613: 1.35
r003 10.605: 0.35     r008 10.608: 0.85     r013 10.614: 0.05
r004 10.606: 0.45     r009 10.611: 0.75     r014 10.614: 0.55
r005 10.607: 0.35     r010 10.611: 0.35     r015 10.615: 0.15
```

Thirteen milliseconds of time; **1.40 m of margin, uncorrelated**. The tightest
run in the field (r013, five centimetres from the edge) is 12 ms *slower* than
the WR; the widest (r011, 1.45 m) is 10 ms slower. Our own tapes make the same
point from the other side: best_10596 crosses *wider* than the WR and is 6 ms
faster. Cutting the last corner harder is worth something at the very margin —
it is where our final millisecond came from — but it is not what separates fast
humans from slow ones, and it is not worth the risk to a driver.

Our 10.594 now crosses with **0.075 m of margin**: three quarters of the WR's
cushion is gone, and what is left is the map's remaining geometric headroom.

There is no visible cue for the edge, and inventing one would be worse than
admitting it: the sweeper is a single stock `RoadTechCurve5` block (block #797,
cell (20,15,18)), and x = 772.18 is 4.2 m outside the x = 768 cell boundary,
lining up with no seam, kerb or scenery edge in the map's block list.

### 3. The route has zero open-loop tolerance

Measured on the champion **and on the human world record's own tape**:

* every single-tick throttle lift tried — ticks 2, 5, 10, 20, 40, 80, 150, 300,
  500 — DNFs;
* sliding the whole tape one tick either way DNFs;
* rounding the steer trace to **even values** (a change of at most half of one
  of 255 steer units per tick) DNFs;
* sample-and-hold at 2 ticks DNFs.

And these are *mid-route* failures, not gate misses: the quantised tapes never
even reach the z=697 plane at t ≈ 9.8 s. (Tick 0 is a separate story and is not
an input at all — see the dead ends.)

**What this does and does not mean.** It means a recorded tape replayed blind
has no tolerance anywhere on this route — every input matters, which is a strong
statement about the map and the reason a search here DNFs 58% of the time. It
does **not** mean a human cannot drive it: the test kills the human WR's own
tape, and a human is a closed loop who sees the car drift and corrects on the
next frame, which is exactly the feedback that perturb-and-replay destroys. The
author drove 10.598. This is a route that needs continuous correction, not one
that is impossible.

For the same reason **converting this tape into a low-input one does not work**:
every simplification of the finished tape — coarser steering, held inputs —
fails. But that is a statement about *conversion*, and conversion is the method
measured not to work anywhere. **Searching under an alphabet constraint from a
digital human seed is a different method and it does produce finishing runs on
this map.** Another agent's ladder, run while this was being written and *not*
re-validated by me — treat as provisional and see their write-up for the
authoritative numbers:

| alphabet | time |
|---|---|
| 16 detents | 10.606 s |
| 8 detents | 10.610 s |
| keyboard | 10.640 s |
| 5 detents, from a human action-key run | 10.644 s |

None of those beat the author time or the human WR, but they are drivable, and
they are the right artefact for a person who wants to *practise* this map rather
than watch a TAS. So the honest form of this section is: **no low-input family
can be obtained by simplifying our 10.594 tape, and a constrained search is the
way to get one.**

## Honest statement of the risk, for anyone driving this

* The failure mode at the finish is **binary and invisible**. There is no warning
  and no partial credit: cross a few centimetres inside x ≈ 772.2 and the run
  does not register at all. Runs killed that way never reach a leaderboard, so
  the public field cannot tell you how often it happens.
* Do not chase that edge. The measurement above says being tighter does not make
  you faster — it only makes you more likely to lose the run.
* Practise the **first 9.5 seconds**: the chicane at 3.2 s and the entry to the
  sweeper. The run home is free, and identical for everybody.

## Negatives, dead ends and things deliberately not used

* **No fork server, on purpose.** `fk fsprobe` works on this map but its
  `lroundf`-per-tick calibration is 308/tick here against map 2's ~857, and with
  the checkpoint corrected the blind locator shortlists 0 candidate vehicle
  states, so `fk btraj` cannot run. It was not pursued: the fork path's known
  phantom history is not worth risking the integrity of a 4 ms answer, and three
  times the throughput would not have changed the outcome — the binding
  constraint was objective resolution, not evaluations. Every number here comes
  from the plain oracle at ~1950 eval/s, which is the physics cost.
* **The hardened build was not adopted mid-session.** It landed while the
  ratchet was converging; swapping toolchains would have cost the run, and the
  patched tree here carries the CP1-End-specific fixes it does not have. Its
  fork-path items are irrelevant to a run that never used the fork path; the
  per-pid root is already satisfied by explicit distinct `--root` on every arm;
  and the guard is exactly what the ratchet already does — `tmtas validate` on
  the untouched map, every cycle.
* **No shaping.** With one waypoint every DNF returns the same "reached 1
  checkpoint" and nothing else, so a DNF carries no gradient. Intermediate
  planes could supply one, but the trigger is **directional**: a yaw = −π gate
  only fires on a +z crossing and a yaw = 0 gate only on a −z crossing.
  Sideways gates DO work, but only at **yaw = −π/2** (fires on a +x crossing);
  yaw = +π/2 returns nothing anywhere along a 60 m sweep. With
  O = P + (17.3, −2.03, −16) a −π/2 gate on the long +x straight reads
  6.428 → 7.270 s over 60 m, i.e. 14.03 ms/m = 71.3 m/s, matching the car's
  measured 71.6 m/s there exactly. That gave the full-route ladder above; it was
  found late, so it informed the analysis rather than the search.
* **The "free tick 0" was a mirage, and it took three measurements to kill.**
  Every human tape appears to idle the throttle on its first packet, which
  looked like up to ~10 ms — far more than the AT gap — waiting to be claimed.
  It is not there.
  1. The tapes are not even on one grid: of 27 ghosts, **15 are
     countdown-prefixed** (first race_ms ≈ −1.5 s, 1210-1230 ticks) and 12 start
     at race 0. Of the zero-start ones, six *do* carry accel = 1 at race 0 —
     including r501/r502/r503 at 10.798-10.800 s, and r013/r015 which are
     12-13 ms *slower* than the WR. Tick index is not race_ms/10, and "everyone
     idles tick 0" was simply false.
  2. **The bit does nothing to the car.** Speed at t = 0/50/100/150/200 ms is
     bit-identical to six significant figures — 0.810531, 2.77579, 5.64592,
     8.51589, 11.3581 km/h — between the tapes that carry accel = 1 at race 0
     and those that carry 0. Different players, identical launch.
  3. **Flipping it DNFs the run in either direction**, one byte changed:
     r001 0→1 DNFs (control 10.602), r015 1→0 DNFs (control 10.615), while a
     true no-op patch reproduces both exactly. An ordinary throttle input cannot
     be catastrophic both switched on and switched off. The first packet is not
     a normal analog triple (`mode = word0 & 0xF`, 0 = "not started"), so the
     bit the tooling calls "accel" there is structural, and writing it
     desynchronises the bitstream.
  Side effect worth knowing: `tmsearch`'s operators do write tick 0, and every
  such candidate DNFs spuriously. Harmless (only finishers are banked) but
  wasted.
* **Sweeper-localised search was the wrong place to look.** An arm confined to
  ticks 700+ has a much healthier finish rate (69% vs 37%) but the ladder says
  the time is earlier; retargeting arms to `--hi 720`, and later one arm to the
  last 1.8 s, is what restarted progress.
* **A `move_gate` trap that silently DNFs everything**: `tmmaps
  segments::move_gate` swaps in the stock finish-gate item model, which on a map
  whose only Goal *is* a custom item deletes the finish outright. The first
  sweep returned DNF at every offset **including the identity placement**, which
  is what caught it. Fixed with `--keep-model`.

## Controls, all passing

1. **27/27 human ghosts** re-simulate to their exact leaderboard millisecond
   (10.602, 10.603, 10.605 … 10.798, 10.800, 10.800 s).
2. `tmsearch --verify` round-trips the WR tape to 10.602 s (candidate factory).
3. `tmtas patch` with no edit reproduces its input exactly (tape editor).
4. Relocated-gate maps put back at the item's own position return 10.602 /
   10.598 / 10.800 s for three different runs (gate machinery is a no-op when it
   should be).
5. `tmtas selftest` 10/10 on this node.
6. Finish attitude, the precondition for trusting a moved plane: 27/27 grounded,
   speed exactly 94.9167 m/s for 23/27, roll spread 0.2°, pitch spread 3.0°,
   y spread 46 mm — worst-case leading-point shift ~6 cm.
7. Every arm after the first carried an explicit distinct `-