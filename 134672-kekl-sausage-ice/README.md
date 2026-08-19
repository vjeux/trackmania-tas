# KEKL- SAUSAGE ICE

**The one thing here that nobody drives is the finish: give up 0.130 into the
last checkpoint, arrive slower and rotated, and the drop to the gate takes
1.950 instead of 2.950.**

The author time did not fall, and it is not close. Everything else on this page
is about the 1.038 that was taken off the best human lap anyone can drive today.

| run | time | vs author time | vs best human on a current build | inputs |
|---|---|---|---|---|
| **TAS** | **67.404** | +8.717 | **−1.038** | 74 steering values, 193 events |
| **TAS, keyboard only** | **67.625** | +8.938 | −0.817 | 3 values, 114 events |
| Author time | 58.687 | — | −9.755 | — |
| Human WR — Roevhaal, set on the 2022 game build | 63.546 | +4.859 | −4.896 | keyboard |
| Best human on a current build | 68.442 | +9.755 | — | keyboard |
| The author's own online record | 69.522 | +10.835 | +1.080 | — |

TMX map [134672](https://trackmania.exchange/maps/134672) · author
**Travis.TM**, uploaded by "KEKL Archive" · 15 recorded runs · the map's own TMX
comment: *"Built in 15mins for KEKL"*.

## What the map is

A narrow elevated ice ribbon — the "sausage" — about 2620 m long, driven at
30–45 m/s with the car permanently sideways. Wall pillars stacked five cells
high with an ice deck on top, one turbo gate, four checkpoints, and a finish
crossed airborne 8 m below the last checkpoint.

## Why the author time is not in reach

Three independent estimates of what this route is worth land within 0.283 of
each other, and the author time is 4.8 s below all three:

| estimate | value |
|---|---|
| the best sector times in the whole 15-run field, summed | 63.263 |
| the 2022 human world record | 63.546 |
| per-sector machine optima, summed | 63.492 |

A perfect splice of everybody's best driving does not reach it. That is not a
proof — the machine optima are local, from a human seed — but the author's own
online record is 10.8 s slower than their own validation lap, on a map they
built in fifteen minutes, and the honest statement is that **58.687 is 4.8 s
beyond the best line anyone, human or machine, has produced on the map as it
ships.**

## Where the time actually goes: this map punishes tiny errors

Change one steering unit, one 1/127th, on a single 10 ms tick of a human's lap:

| gate | reference | with the one-unit change at 2.0 s |
|---|---|---|
| 1.9 s | 1.916 | 1.916 |
| 2.9 s | 2.927 | **2.927 — exact** |
| 8.0 s | 7.973 | 8.037 |
| 9.6 s | 9.634 | **15.716 — the run is gone** |

The error is invisible for a second, worth +0.064 six seconds later, and fatal
by eight. **Errors grow by a factor of e roughly every 0.7 s.** That single
number is why 15 records are spread over 40 seconds, and why every sector
correlates 0.61 to 0.89 with the final time: this is a field separated by
general control, not by one feature. There is no trick corner to learn. There is
no shortcut either — the corridor is the corridor.

**Pin the lock, and drift.** Across the field, more time at full lock goes with
a *faster* lap, not a slower one — correlation −0.40 overall, −0.77 among the
eight pure-keyboard runs and −0.47 among the seven pad runs — and the top three
records are keyboard, holding steer for 170–290 ms at a time against 10–20 ms
for the pad runs. Mean sideways speed runs 13.8–23.2 m/s over the whole lap and
is monotone in pace: the world record is the most sideways run on the board.
This is a committed continuous drift, and the steering is for rotation, not for
grip. Back off lock where you are trying to keep the car pointed and
accelerating; keep it pinned where you are trying to rotate.

## The run, sector by sector

Read off the human world record's own lap, which is pure keyboard and the right
thing for a person to copy. Times are its own.

**Start → the north loop (0 → 4.2 s).** Full throttle from the line, dead
straight for 1.8 s up the start ramp. One 50 ms left dab at 1.78 s to settle the
car, then **full right at 2.60 s** with a 60 ms release at 2.72 — you are
turning left-handed round the top of the map with the wall on your right. Lift
the throttle for 250 ms at 3.37 s as the nose comes round; that lift is what
stops the slide widening.

**The long left descent (4.2 → 8.4 s).** **Full left from 4.24 s and hold it for
a whole second** — the biggest single input on the map, with the car at 36–44 m/s
and 25–40 m/s sideways through it. Coast, no gas, from 5.30 to 5.43. Straighten
at 6.03, then alternate short full-lock stabs of 110–320 ms down the ridge,
aiming at the gap where the deck rises.

**The jump and the hairpin into CP1 (8.4 → 13.9 s).** You leave the ground at
about 8.5 s. **Gas off through the air, brake dab at 8.76 s for 470 ms**, land
under full right lock. Then the slowest corner on the map: you scrub to
19–20 m/s at 11.7–12.1 s, full left all the way round. **Cross the turbo gate
pointing straight down the ice straight** — it takes you from 33 m/s to 50 m/s
in 250 ms, and CP1 is 32 m past it. Getting that exit straight is worth more
than the corner itself.

**The east loop (13.9 → 23.5 s).** Ice straight at 56–61 m/s, then one long
full-left hold of **2.570** from 15.96 s round the far end, peaking at 72 m/s at
20 s. The fastest part of the map and the least fiddly: two long holds, not many
small ones.

**The west run and CP2 (23.5 → 31.1 s).** The field loses about a second here to
the world record, which carries **46.6 m/s where second place carries 36.6**. It
is not a trick — it is not scrubbing on entry. Short right stabs, gas on, no
brake.

**CP2 → CP3 (31.1 → 42.5 s).** Full-lock left, a 1.820 hold from 32.82 s, then a
1.0 s coast from 34.64 — the only long coast on the map. Full-lock right through
39–42 s with the car at 43–47 m/s and 43–47 m/s sideways, completely square to
its own velocity.

**CP3 → CP4 (42.5 → 59.6 s).** The longest sector. Nothing discrete: hold the
drift, do not lift.

**The finish (59.6 → 67.4 s) — the one thing that can be taught here.** The
field arrives at CP4 as fast as possible and then loops wide, taking 2.9 s from
the last cell to the line. **Give up 0.130 into CP4** — arrive slower and
rotated — and the drop to the finish gate, which sits 8 m *below* CP4, can be
taken in **1.950 instead of 2.950**. This closing sector is 3.462 against a best
human closing sector of 3.964 in three years, and it survives the keyboard
constraint almost intact: 3.700 on three steering values.

## How forgiving it is

Per-input slack in the closing sector, measured by mistiming one input and
re-timing only the ones after it:

| input | usable window | cost at the edge |
|---|---|---|
| 59.16 s, full left | **1 tick** | any mistiming and you do not finish |
| 59.22 s, release | **1 tick** | any mistiming and you do not finish |
| 59.47 s, full right | **1 tick** | every shift fails |
| 59.54 s, release | **1 tick** | any mistiming and you do not finish |
| 59.71 s, full right | 4 ticks (−1 … +2) | +0.007 to +0.263 |
| 59.86 s onward | 9 ticks (±40 ms) | 0 to +0.018 |

**Four 10 ms-tight commitments between 59.16 and 59.54, and then ±40 ms of room
for the rest.** You have to hit the entry to the last complex within one tick;
the drop itself is forgiving. Those four inputs are this particular route into
the drop, and most of the 1.038 lives in what happens after them.

**What will take real practice: the whole lap.** No single input outside the
finish is worth more than a second, the 4.9 s between the 2022 record and the
best current-build human is diffuse carry speed that grows almost linearly with
distance, and every input on the machine lap is load-bearing — not one of 319
input events can be removed for a 40 ms budget. The keyboard lap costs 0.221
over the analog one and is directly drivable: three steering values, digital
throttle, digital brake.

## Files

| file | what |
|---|---|
| `replays/TAS_67404.Ghost.Gbx` | **the fastest lap** — 74 steering values |
| `replays/KEYBOARD_67625.Ghost.Gbx` | **keyboard only, 3 values, 114 events** — the one worth studying |
