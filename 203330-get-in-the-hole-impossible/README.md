# Get in the Hole ( Impossible ) — author time beaten, on a keyboard, with twelve inputs

| | time | vs AT | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, keyboard, 12 inputs** | **13.986** | **−0.009** | **−0.032** | **12 change events** |
| TAS, keyboard, 31 inputs | **13.984** | −0.011 | −0.034 | 31 |
| TAS, unconstrained | 13.984 | −0.011 | −0.034 | 424 events, 192 steer values |
| TAS, keyboard, 19 inputs | 13.985 | −0.010 | −0.033 | 19 |
| Author time (never beaten by a human) | 13.995 | — | −0.023 | — |
| Human WR — in-.- | 14.018 | +0.023 | — | 46 |

TMX map [203330](https://trackmania.exchange/maps/203330) · uid
`RL64wn0vFhuqHfKGLnMOql2SMaj` · **only 5 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## Why this map was the hardest target on the list

Five recorded runs. Two clip a wall, one overshoots the finish entirely — so the
"field" is effectively two finishing attempts. There is almost no human
knowledge to build on.

The map: steering is **disabled** at the start by a `GateSpecial8mNoSteering`,
seven turbo blocks take the car to 810 km/h in 3 seconds, then a 3.5 s dive, a
redirect ramp, a scrubbing ground contact, and a **cannon** at 8.51 s that sets
the speed to exactly **1000 km/h** and fires the car down a 1370 m corridor.

At z = 976 a wall spans the corridor with **one empty cell** — x ∈ [160,192],
y ∈ [64,72]. That is the hole. Clear it, fall, land at z ≈ 1315, slide, cross
the finish at z = 1507.

## The whole map is one 2.3-second aim

**Between 6.2 s and 8.5 s you are aiming the cannon.** Everything else is
already decided:

- Before **2.9 s** steering is disabled — measured, not assumed: full left, full
  right and centre give the identical millisecond.
- After the cannon you are ballistic. Steering turns the car but does not move
  it: **9,113 forced-steering variants** through the descent and touchdown
  changed the finish time by **zero**.
- The cannon outputs 999.8 km/h for everyone. The flight is fixed to 2–3 ms
  across a 33 ms spread of finish times.

Aim to cross the hole at **x ≈ 171** — a little left of the corridor centre at
176 — and **flat**.

## Where the time is, and it is not where you would look

**34 of the 37 ms won came from the last 106 m.** There is a **2 m step at
z = 1472**, just before the finish platform:

- Hit it at x ≈ 182 and you lose 490 km/h.
- Ride it at x ≈ 172 and you lose nothing.

**The world record hits it.** Its speed collapses from 800 to 312 km/h at
t = 14.00 — and it still wins the map. Our run rides the same lip at 858 km/h.

So the coaching point on the "impossible" map is not the hole, not the cannon,
and not the dive. **It is where you land afterwards.**

Second point, from the two runs that crash: both clip the wall while **rolled**
0.6–1.8 rad, at a *higher* pass height than the fast line. **It is an attitude
limit, not a height limit.** Stay flat and the hole is generous.

## The route — twelve inputs, 13.986

Race-clock times. `left`/`right` are the steering keys; throttle is held from
the lights to the line.

```
  0.000   accelerate  (hold to the end)
  0.000   steer LEFT
  1.600   steer RIGHT
  5.110   release steering
  5.150   steer LEFT
  6.210   steer RIGHT
  6.340   release steering
  6.360   steer LEFT
  6.530   BRAKE ON     (hold)
  9.560   BRAKE OFF, and steer RIGHT
  9.630   release steering
  9.700   steer RIGHT
 13.170   release steering
```

Twelve inputs. **The world record uses 46 and is 32 ms slower.**

**Twelve is the floor, not just where we stopped.** Delta debugging proves the
tape **1-minimal** within the budget: all eleven single deletions refused, and no
contiguous pair or run of five removable either. **No 11-input version of this
line exists under 13.995.** And the same minimisation started from the 31-input
optimum — a different line, 19 more inputs, 2 ms faster — converges to twelve
events as well. Two starting points, one floor.

One for the practice session: **an eleven-input version finishes at 13.996** —
drop the input at race 13.2 s and the whole route survives, one millisecond
outside the author time.

**The brake is load-bearing, and it is the counter-intuitive part.** Holding it
from 6.53 s through the cannon is what settles the car; **removing it entirely
does not finish the map.** The timing is forgiving to about 70 ms, so it is a
hold you can learn rather than a frame-perfect tap.

That instruction nearly did not make it into this guide. Ablation had shown the
brake was deletable over *every individual sub-window* with no cost, which reads
as "the brake does nothing" — and it is wrong. It tolerates ~70 ms of slack, so
each local deletion is absorbed by its neighbour, and only removing all of them
at once reveals that it is essential. **Window-local inertness does not compose
into global inertness.**

The 31-input route ([`inputs/kb330_31ev_13984.tick.txt`](inputs/kb330_31ev_13984.tick.txt))
adds a run of small attitude trims from 12.6 s on — they keep the car flat over
the step — and is worth two more milliseconds. On the analog side those same two
milliseconds cost **424 inputs and 192 distinct steering values**, which is why
the twelve-input version is the one worth learning.

## What the field is losing, and where

| run | time | what happens |
|---|---|---|
| **this route** | **13.984** | through the hole flat at x≈171; rides the step at 864 km/h |
| author time | 13.995 | — |
| r01 (WR) | 14.018 | lands at x≈182, **smashes the step**: 800 → 312 km/h |
| r02 | 14.031 | same line, bleeds more speed |
| r03 | 15.478 | **clips the wall** — rolled, not low |
| r04 | 21.230 | **clips the wall** — rolled 1.35–1.79 rad |
| r05 | 23.153 | clean flight, **overshoots the finish**, bounces for 9 s |

Two of the five records are wall clips and one is an overshoot. The map has had
almost no human search, and the world record still crashes into the last
obstacle on the way past the finish line.

## Validation

All five human records re-simulate to their exact leaderboard millisecond as the
identity control (14.018 / 14.031 / 15.478 / 21.230 / 23.153). Every banked tape
re-validates through the plain oracle against the untouched map. No phantoms.

The alphabet constraint was itself verified with a zero-ladder control, and this
map is where a **precondition on that control** was discovered: a one-level
ladder run over a window where steering is already inert reports `finish 100%`
and looks exactly like a broken constraint. Confirm the constrained window is
live — force a constant through it and check the time moves — before trusting
`finish 0%`. Details in [`notes/CONTROLS.md`](notes/CONTROLS.md).

## Files

| file | what |
|---|---|
| `replays/kb330_12ev_13986.Ghost.Gbx` | **twelve inputs, 13.986** — the one to learn |
| `replays/kb330_31ev_13984.Ghost.Gbx` | 31 inputs, the keyboard optimum |
| `replays/an330_13984.Ghost.Gbx` | fastest run, unconstrained |
| `replays/kb330_19ev_13985.Ghost.Gbx`, `kb330_22ev_13985.Ghost.Gbx`, `kb330_15ev_13990.Ghost.Gbx` | the rest of the low-input family |
| `inputs/*.tick.txt` | every keyboard run as a readable input script |
| `notes/HOW_TO_DRIVE_IT.md` | the full driving guide |
| `notes/RESULT.md`, `notes/NOTES.md`, `notes/PLAN.md` | measurements, and what the tools got wrong here |
| `notes/CONTROLS.md` | the zero-ladder precondition |
