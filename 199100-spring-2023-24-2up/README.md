# Spring 2023 - 24 (2-UP)

**One 10 ms brake tap, six tenths of a second into the reactor climb, is worth
0.730 on a keyboard run — three quarters of everything on this page.**

| run | time | vs author time | vs human WR | device |
|---|---|---|---|---|
| **TAS, unconstrained** | **49.778** | **−1.824** | **−2.424** | analog |
| **keyboard** | **51.062** | **−0.540** | **−1.140** | **3 values** |
| **a human keyboard run + seven actions** | **51.575** | **−0.027** | **−0.627** | **keyboard** |
| Author time | 51.602 | — | −0.600 | — |
| Human WR — JuntaoTM | 52.202 | +0.600 | — | pad |

TMX map [199100](https://trackmania.exchange/maps/199100) · author **.ar** ·
tags **Reactor, Plastic, Altered Nadeo** · **6 recorded runs**.

This is an Altered Nadeo copy of **Spring 2023 - 24**, with the geometry and the
surfaces preserved, so the 200 000 people who drive the official map are driving
the same road — their lines are directly usable here. That matters below.

## What the map is

Nine checkpoints and 42.5 s of ordinary ground driving, then a **launcher
captures the car**: horizontal speed collapses from 300 to 121 km/h in a tenth of
a second, the car ends up inverted, and from there it is **under thrust while
airborne for about seven seconds** — climbing 180 m while re-accelerating to
400 km/h — before arcing over and diving through a finish gate hanging in the
air. You cross it nose-down at about 320 km/h.

The flight is powered and steerable, not ballistic, so air control is worth
seconds. The fastest tape here is byte-identical to the world record for the
whole ground run: **100% of its gain comes from inputs after 42.330.** The
launcher is also the field's filter — two of the five clean records respawn
there, at a cost of 3.2 and 4.7 seconds.

## Where the time is, part one: the brake tap

The technique is stated as a modification of a run a human already drove —
**uelen.'s rank-3 keyboard run, 52.599**, three steering values, 190 input
events, no respawn. Keep every one of those inputs and add seven actions:

| # | race time | action | duration |
|---|---|---|---|
| 1 | **43.23** | **tap brake** | 10 ms |
| 2 | 43.65 | release gas | 110 ms |
| 3 | 43.82 | tap brake | 30 ms |
| 4 | 47.80 | tap right | 20 ms |
| 5 | 48.28 | tap right | 50 ms |
| 6 | 49.17 | hold brake | 90 ms |
| 7 | 49.27 | hold right | 470 ms |

Cumulatively:

| through action | time | gain |
|---|---|---|
| none | 52.599 | — |
| **#1 alone — one 10 ms brake tap** | **51.869** | **−0.730** |
| #1–#3 | 51.724 | −0.875 |
| #1–#5 | 51.638 | −0.961 |
| #1–#7 | **51.575** | −1.024 |

The tap is pitch control: it decides where the thrust points for the whole climb.
And it is not a lottery ticket. Applied to the untouched human run across a grid
of 217 start times and durations, **every variant that finishes lands between
51.764 and 52.529** — a tap anywhere in a window of roughly 400 ms is worth 0.100
to 0.800, typically about 0.700. Short taps are what work: 10–30 ms finish, 50 ms
and longer usually miss the gate.

**A miss is recoverable.** Four tap times were tried, including two that miss the
gate outright, with only the inputs after 44.0 s re-aimed: they land at 51.598,
51.744, 51.748 and 51.795 — within 0.051 of each other. The exact tap time does
not decide the run; the aiming afterwards does.

The full keyboard tape (51.062) is the same idea taken further: **pump the brake
through the first 900 ms of the climb**, then aim in the last two seconds.

| race | action |
|---|---|
| 43.23 | brake 10 ms |
| 43.48 | brake 20 ms |
| 43.58 | brake 70 ms |
| 43.65 | **gas off 110 ms** |
| 43.79 | brake 60 ms |
| 43.93 | brake 120 ms |
| 44.10 | brake 20 ms |
| 44.82 | release the brake 10 ms earlier than uelen. does |
| 47.41 | let go of left 30 ms earlier |
| 47.80 | right 20 ms |
| 48.14 → 48.37 | right (uelen. holds 40 ms here; hold 230) |
| 48.43 | left |
| 48.57 | brake 40 ms |
| 49.17 | brake 90 ms |
| 49.37 → 49.85 | **hold right ~480 ms** |
| 49.85 | left into the gate |
| 49.88 | brake 70 ms |

**Why the author time stood:** the same tap on the pad world record does nothing
at all — it costs 0 to 0.060 and never misses. The two fastest humans here are on
pads, where the technique is invisible, and the fastest keyboard player is rank 3.

## Where the time is, part two: there are two roads through sector 3

Sector 3 runs CP2 → CP3, and those two checkpoints are **241 m apart in a
straight line**. Split 88 recordings on this geometry by whether they climb over
the tower:

| line | n | mean sector 3 | best | mean path length |
|---|---|---|---|---|
| over the top | 27 | 5.664 | 5.516 | **710 m** |
| **low and short** | **61** | **4.721** | **3.901** | **306 m** |

**All five humans who have driven this map take the long one**, and so does every
tape in our own lineage. That is not driving quality, it is a different road.

The long line looks like the good one: off CP2 it turns hard left, drives *away*
from the checkpoint, climbs a wall, arcs over the top and drops down the far
side, never going below 350 km/h and averaging 430. It also covers 701 m to
travel 241 m of map. The short line carries straight on, leaves the ground and
*falls* 21 m, lands already braking, scrubs down to **150 km/h** through a tight
left, and then takes a boost from 150 to 501 km/h in 0.8 s. Its average speed is
276 km/h — far slower — over 299 m, and it wins by **1.971**.

Substitute the field's short line into the world record's lap and change nothing
else:

| | sector 3 | lap |
|---|---|---|
| the human WR as driven | 5.872 | 52.202 |
| WR + a **median** short line | 4.721 | **51.051** |
| WR + the **best** short line | 3.901 | **50.231** |

A median execution of a route that 61 of 88 sampled players already drive puts
the human world record 0.551 inside the author time, with no reactor trick and
nothing changed after CP3. The two lines arrive at CP3 in the same place at the
same speed, so the gain is additive rather than borrowed, and the slow phase is a
braking phase — the most forgiving input there is.

**Stated plainly: nobody has driven this here.** It is two drivers' sectors added
together, and a sector sum is a bound until someone drives it. One practical
note if you try it: **the decision is made at CP2**, about 1.9 s before the low
line's landing and brake, and by then it is too late to choose.

## The run as inputs, sector by sector

Sectors 1–9 are exactly uelen.'s rank-3 run, unmodified: drive the keyboard run
that already exists. Rank 4 does the same line 0.468 faster to CP9, so that time
is on the table for a human too. What is new starts at CP9, and the cues are what
you can see:

1. **CP9 → the launcher (40.6 → 42.6).** Off the fast deck, up the climb, slowing
   to ~350 km/h. Nothing changes here.
2. **The capture (42.6).** The launcher takes the car: speed collapses to
   ~120 km/h and the car rolls inverted. uelen. is already holding **full left**
   from 42.63 — keep holding. That rotation is what the next input acts on.
3. **The brake tap (≈43.2 — about six tenths after you feel the grab, while the
   nose is coming down through level and before the car starts swinging toward
   the tower).** One short tap, 1–3 ticks. This is the 0.730. Anywhere in
   43.05–43.45 works, and if it feels early or late, do not abandon the run.
4. **Pump it (43.5 → 44.1).** For the full version: gas off around 43.65 for a
   tenth, then three or four more short brake taps through 44.1. You are choosing
   where the thrust points for the whole climb.
5. **The climb (44 → 47.4).** As uelen. drives it: left releases, right at 44.16
   and 44.77, the brake/gas pair at 44.52–44.83, then alternating left and right
   through the apex at about 342 m, around 49 s.
6. **The aim (47.4 → 49.9).** Two short rights, a left, brake at 48.57 and again
   at 49.17, then **hold right for about half a second from 49.37** while the car
   dives at the gate, flicking left into it. This is the part you steer by eye.
7. **Finish** — airborne, nose-down, ~320 km/h.

## How forgiving it is

The aiming inputs at the end are genuinely loose, and the early climb is where
the practice goes:

| input | usable window | cost at ±1 tick |
|---|---|---|
| #6, brake at 49.17 | **±60 ms, zero cost** | 0 / 0 |
| #7, right hold from 49.27 | **±60 ms, zero cost** | 0 / 0 |
| #4, right tap at 47.80 | 8 ticks | +0.019 / +0.039 |
| #5, right tap at 48.28 | 1 tick | +0.077 / +0.432 |

The finish is a gate you must hit in the air, so a recording replayed blind is
brutally fragile — shift any single input by one tick and the car misses the
gate, and that is just as true of the human's own tape as of ours. It is a
statement about tapes, not about driving: what a driver does is watch the flight
and correct it. **The timing of what you do is loose; the requirement to correct
afterwards is absolute.**

A 10–30 ms tap is well inside what the field already does — uelen.'s own shortest
steering hold is 10 ms, and the pad world record steers with a 20 ms median.

## Files

| file | what |
|---|---|
| `replays/HUMANPLUS7_51575.Ghost.Gbx` | **the deliverable — a human's keyboard run plus seven actions** |
| `replays/KEYBOARD_51062.Ghost.Gbx` | the full keyboard tape |
| `replays/TAS_49778.Ghost.Gbx` | the unconstrained floor |
| `replays/A2_50738.Ghost.Gbx` | an earlier tape in the analog lineage |
| `replays/A3_50224.Ghost.Gbx` | a later one |
