# Get in the Hole ( Impossible )

**The map is named after the hole, but the hole is not what decides it: cross it
flat and a little left of centre, at x ≈ 171, and you will ride the 2 m step
before the finish that costs the world record 490 km/h.**

| run | time | vs author time | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, keyboard, 12 inputs** | **13.986** | **−0.009** | **−0.032** | 12 change events |
| TAS, keyboard, 31 inputs | 13.984 | −0.011 | −0.034 | 31 |
| TAS, keyboard, 19 inputs | 13.985 | −0.010 | −0.033 | 19 |
| TAS, unconstrained | 13.984 | −0.011 | −0.034 | 424 events, 192 steer values |
| Author time | 13.995 | — | −0.023 | — |
| Human WR — in-.- | 14.018 | +0.023 | — | 46 |

TMX map [203330](https://trackmania.exchange/maps/203330) · author
**EvenOliveTM.exe** · **only 5 recorded runs**.

## What the map is

Steering is disabled at the start. Seven turbo blocks take the car to 810 km/h
in three seconds, then a 3.5 s dive, a redirect ramp, a scrubbing ground
contact, and a **cannon** at 8.51 s that sets the speed to exactly 1000 km/h and
fires the car down a 1370 m corridor.

At z = 976 a wall spans the corridor with **one empty cell** — 32 m wide and 8 m
tall. That is the hole. Clear it, fall, land at z ≈ 1315, slide, and cross the
finish at z = 1507.

## The whole map is one 2.3-second aim

**Between 6.2 s and 8.5 s you are aiming the cannon.** Everything else is
already decided:

- Before **2.9 s** steering is disabled — full left, full right and centre all
  give the identical millisecond.
- After the cannon you are ballistic. Steering turns the car but does not move
  it: forcing the wheel anywhere through the descent and touchdown changes the
  finish time by **zero**.
- The cannon outputs 999.8 km/h for everyone, and the flight from it is fixed to
  2–3 ms across a 33 ms spread of finish times.

Aim to cross the hole at **x ≈ 171** — a little left of the corridor centre at
176 — and **flat**.

## Where the time is, and it is not where you would look

**34 of the 37 ms won came from the last 106 m.** There is a **2 m step at
z = 1472**, just before the finish platform:

- Hit it at x ≈ 182 and you lose 490 km/h.
- Ride it at x ≈ 172 and you lose nothing.

**The world record hits it.** Its speed collapses from 800 to 312 km/h at
t = 14.00 — and it still wins the map. Our run rides the same lip at around
860 km/h.

So the coaching point on the "impossible" map is not the hole, not the cannon,
and not the dive. **It is where you land afterwards.**

Second point, from the two runs that crash: both clip the wall while **rolled**
0.6–1.8 rad, at a *higher* pass height than the fast line. **It is an attitude
limit, not a height limit.** Stay flat and the hole is generous.

## What the field is losing, and where

| run | time | what happens |
|---|---|---|
| **this route** | **13.984** | through the hole flat at x ≈ 171; rides the step at speed |
| author time | 13.995 | — |
| r01 (WR) | 14.018 | lands at x ≈ 182, **smashes the step**: 800 → 312 km/h |
| r02 | 14.031 | same line, bleeds more speed |
| r03 | 15.478 | **clips the wall** — rolled, not low |
| r04 | 21.230 | **clips the wall** — rolled 1.35–1.79 rad |
| r05 | 23.153 | clean flight, **overshoots the finish**, bounces for 9 s |

Two of the five records are wall clips and one is an overshoot. The map has had
almost no human search, and the world record still crashes into the last
obstacle on the way past the finish line.

## The route — twelve inputs, 13.986

Race-clock times. `left`/`right` are the steering keys; throttle is held from the
lights to the line.

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

Twelve inputs. **The world record uses 46 and is 0.032 slower.**

Twelve is the floor rather than just where we stopped: no eleven-input version
of this line exists under the author time, and the same minimisation started
from the 31-input route — a different line, 19 more inputs, 0.002 faster —
converges to twelve events as well. One for the practice session: an
eleven-input version finishes at 13.996, one millisecond outside the author
time.

The 31-input route (`inputs/kb330_31ev_13984.tick.txt`) adds a run of small
attitude trims from 12.6 s on — they keep the car flat over the step — and is
worth two more milliseconds. On the analog side those same two milliseconds cost
424 inputs and 192 distinct steering values, which is why the twelve-input
version is the one worth learning.

## Sector by sector

**Start → 6.2 s.** Hold accelerate. Hold left, then right at about 1.6 s.
Steering does nothing before 2.9 s, so the only input that matters here is the
throttle.

**6.2 → 11.0 s — the launch, the only thing you really steer.** A handful of
taps around the redirect ramp, then **brake on at about 6.5 s and off at about
9.5 s**. What you are aiming for is to leave the cannon pointed so that you
cross the wall at x ≈ 171, left of the corridor centre, and flat.

**The hole, and then ride it out.** You clear the wall at y ≈ 63, land at
z ≈ 1315 and slide in. There is nothing to do after that — the run is decided
before you get there, so do not try to save it late.

## How forgiving it is

- **The brake is load-bearing, and it is the counter-intuitive part.** Holding it
  from 6.53 s through the cannon is what settles the car; removing it entirely
  does not finish the map. Its timing tolerates about 70 ms, so it is a hold you
  can learn rather than a frame-perfect tap.
- **Steering before 2.90 s is free** — you cannot get it wrong, because the map
  will not let you steer.
- **After the hole there is no authority at all.** Every forced input over the
  whole landing and slide changes the finish by nothing. That cuts both ways: you
  cannot lose the run there either.
- **What will take real practice** is the 6.2–8.5 s window that aims the cannon,
  and the attitude you carry through it. Roll is what kills runs here: it is what
  puts two of the five records into the wall, and it is what makes the world
  record hit the step.

## Files

| file | what |
|---|---|
| `replays/kb330_12ev_13986.Ghost.Gbx` | **twelve inputs, 13.986 — the one to learn** |
| `replays/kb330_31ev_13984.Ghost.Gbx` | 31 inputs, the keyboard optimum |
| `replays/an330_13984.Ghost.Gbx` | fastest run, unconstrained |
| `replays/kb330_19ev_13985.Ghost.Gbx` | 19 inputs |
| `replays/kb330_22ev_13985.Ghost.Gbx` | 22 inputs |
| `replays/kb330_15ev_13990.Ghost.Gbx` | 15 inputs, 13.990 |
| `replays/best_13985.Ghost.Gbx` | 13.985 |
| `inputs/kb330_12ev_13986.tick.txt` | the twelve-input route as a readable input script |
| `inputs/kb330_31ev_13984.tick.txt` | the 31-input route, likewise |
