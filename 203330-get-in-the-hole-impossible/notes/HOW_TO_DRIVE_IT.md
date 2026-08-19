# 203330 "Get in the Hole ( Impossible )" — how to drive it

The author time is **13.995 s**. The world record is **14.018 s**. This is a
route that does **13.984 s on three keys**, and a shorter one that does
**13.986 s on twelve inputs** — both still inside the author time.

Everything below is validated against the game's own physics (the dedicated
server re-simulates the input tape and returns the time). No leaderboard
submission was made or will be.

---

## The map in one picture

You are fired out of a booster cannon at exactly **1000 km/h** and flown 1370 m
down a walled corridor. At the far end the corridor is sealed by a wall from the
floor to well above the roofline, with **one 32 m × 8 m window in it**. Through
the window, down onto the floor, and slide into the finish.

```
  0.0 s   lights out, full throttle. Steering does NOTHING for 2.9 s
  0.0-3.0 seven boosters take you to 810 km/h
  3.7     the road ends; you fly, and fall, for 3.5 s
  7.4     a ramp, then a booster: 941 km/h
  8.1-8.5 you touch down and scrub to 841 km/h
  8.51    THE CANNON: your speed is set to 1000 km/h and you are pointed +z
  11.77   THE HOLE at z=976 -- the window is x 160..192, y 64..72
  13.18   touchdown, 876 -> 849 km/h
  13.84   ride a 2 m step at z=1472
  13.98   finish
```

## The only thing you actually steer

**Between 6.2 s and 8.5 s you are aiming the cannon.** That is the whole map.

- Before **2.9 s** steering is disabled — measured, not assumed: full left, full
  right and centre all give the identical millisecond.
- After the cannon you are ballistic. Steering turns the car but does not move
  it: 9113 forced-steering variants through the descent and touchdown changed
  the finish time by **zero**.

Aim to cross the hole at **x ≈ 171**, a little LEFT of the corridor centre
(176), and **flat**.

## The three things that beat the world record

1. **Land left of centre.** x ≈ 171-175 at the finish, not x ≈ 182. There is a
   2 m step at z=1472 just before the finish platform. Hit it at x=182 and you
   lose 490 km/h; ride it at x=172 and you lose nothing. **The world record hits
   it** — it drops from 800 km/h to 312 km/h at t=14.00 and still wins the map.
2. **Stay flat.** Roll ≈ 0 through the hole and over the step. Every human
   record arrives rolled 1.2-2.8 rad, and speed at the step orders the entire
   field by finish time. The two records that clip the wall (15.478 and 21.230)
   clip it while rolled 0.6-1.8 rad — at a *higher* pass height than the fast
   line. **It is an attitude limit, not a height limit.**
3. **Do not try to save it late.** After the hole there is nothing left to do.

## The 12-input route — 13.986 s, nine milliseconds under the author time

Times are the on-screen race clock. `left`/`right` are the steering keys.

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

Twelve inputs. The world record uses 46 and is 32 ms slower.

**The brake is load-bearing, and it is the counter-intuitive part.** Holding it
from 6.53 s through the cannon is what settles the car; removing it entirely
does not finish the map. The timing is forgiving to about 70 ms, so it is a hold
you can learn rather than a frame-perfect tap.

## The 31-input route — 13.984 s, the optimum

Same first nine inputs, then a run of small corrections that are worth two more
milliseconds:

```
  0.000   accelerate; steer LEFT
  1.600   steer RIGHT
  5.110   release ;  5.150 steer LEFT
  6.210   steer RIGHT ;  6.340 release ;  6.360 steer LEFT
  6.530   BRAKE ON
  9.340   release steering
  9.520   BRAKE OFF, steer LEFT
  9.560   steer RIGHT ;  9.630 release ;  9.700 steer RIGHT
 10.500   release ; 10.560 steer RIGHT
 11.830   release ; 11.840 steer RIGHT
 12.650   release ; 12.680 steer RIGHT ; 12.690 release ; 12.700 steer RIGHT
 12.740   release ; 12.750 steer RIGHT
 13.170   release
 13.430   steer LEFT ; 13.460 release ; 13.500 steer LEFT ; 13.530 release
 13.680   steer RIGHT ; 13.690 release ; 13.870 steer LEFT
```

The taps from 12.6 s on are attitude trim in the air — they are what keeps the
car flat over the step. On the analog side the same two milliseconds cost 424
inputs and 192 distinct steering values, so this is the version worth learning.

## What the field is losing, and where

| run | time | what happens |
|---|---|---|
| **this route** | **13.984** | through the hole flat at x≈171; rides the step at 864 km/h |
| author time | 13.995 | — |
| r01 (WR) | 14.018 | lands at x≈182, **smashes the step**: 800 → 312 km/h |
| r02 | 14.031 | same line, bleeds more speed |
| r03 | 15.478 | **clips the wall** — rolled, not low |
| r04 | 21.230 | **clips the wall** — rolled 1.35-1.79 rad |
| r05 | 23.153 | clean flight, **overshoots the finish**, bounces for 9 s |

Two of the five records on this board are wall clips and one is an overshoot.
The map has had almost no human search, and the world record still crashes into
the last obstacle on the way past the finish line.

## Files

```
lowinput/kb330_12ev_13986.Ghost.Gbx  + .tick.txt   13.986, 12 inputs
lowinput/kb330_31ev_13984.Ghost.Gbx  + .tick.txt   13.984, 31 inputs
best/an330_13984.Ghost.Gbx                         13.984, analog (424 inputs)
```

Technical report: `RESULT-v3.md`. Controls: `CONTROLS-v1.md`.
