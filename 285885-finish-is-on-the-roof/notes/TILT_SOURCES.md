# 285885 — where the tilt could come from: three measurements

*Companion to `att_TRIGGER_IS_BODY_POINT_v2.md` (same session, same agent).
That sidecar establishes that the finish tests a point ~0.84 m above the car's
origin in the BODY frame, that the best route is 70 mm short at its binding
point, and that the only way to close 70 mm is ~26° of body tilt. This one
records what I measured about **where tilt can and cannot be obtained.** All
three are `fk btraj` live readouts (reference-free, self-check clean) on tapes I
built — not any tape's embedded telemetry.*

## 1. Steering produces no body roll at all on the finish ramp

Hold **full left lock for 80 ticks (0.8 s)** from 1.1 s before the finish, at
185–195 km/h. The car turns hard enough to leave the finish line entirely, and
its attitude does this:

| t (ms) | identity `u_y` | full-lock `u_y` |
|---|---|---|
| 40700 | 0.9762 | 0.9856 |
| 40900 | 0.9744 | 0.9858 |
| 41100 | 0.9809 | 0.9838 |
| 41300 | 0.9829 | 0.9894 |

Under full lock `u_y` is **0.984–0.989 — flatter than the unperturbed tape**, and
nowhere near the 0.895 the finish needs. Suspension roll at ~2 g of cornering is
worth **under 1°**. With the ≤ 5 mm of available suspension compression and the
nil response to accel/brake (v2 §4), that closes it: **on the finish ramp the
car's attitude IS the ramp's attitude, and no input changes it.**

## 2. The car cannot leave that ramp either — it would need 440 km/h

Fit the ramp's own profile along the route's path (the car is grounded at a
constant ride height for 131.7 m, so its origin height *is* the surface):

> `y = 127.176 + 0.18539·s − 3.278e−4·s²`, rms 0.196 m
> ⇒ d²y/ds² = −6.6e−4 /m, **radius of curvature ≈ 1500 m**
> ⇒ the car leaves a crest of that radius at **122 m/s = 440 km/h**.

The route crosses the finish at **190 km/h**. The last 130 m is a smooth convex
ramp with no crest, no lip and no bump (the plane fit in v2 §3 holds to
rms 38 mm across the footprint), so **the car is glued to it and cannot become
airborne anywhere on the approach.**

## 3. A ramp EDGE flips the car in one second — that is the rotation source

The same full-lock tape carries on up a second, steeper ramp (11.5 m/s of climb
at 200 km/h) and leaves it at **(418, 172, 1550) at 43.55 s**. What follows is
exactly what the map needs, in the wrong place:

| t (ms) | `u_y` | |
|---|---|---|
| 43550 | 0.849 | leaves the surface |
| 43700 | 0.586 | |
| 43850 | 0.271 | |
| 44000 | −0.042 | **on its side** |
| 44300 | −0.541 | |
| 44600 | −0.797 | **upside down**, 1.05 s after the edge |

A continuous ~1.7 rad/s tumble — the same rate as the human flip at
(295, 122, 1772) that two of the three humans use. So the rotation source is
generic: **leave a ramp at speed and the car tumbles at ~1.7 rad/s, passing
through every attitude within a second. 26° of tilt is 0.26 s of that tumble.**

## 4. What that makes the problem

Putting §1–§3 together, a winning run must **already be tumbling when it reaches
the finish**, and it cannot start tumbling on the last 130 m. So the edge has to
be at least 130 m upstream and **the car has to fly the whole remaining ramp**,
or the tilt has to be bought at a wall.

The three known rotation sources and their prices:

| source | where | price |
|---|---|---|
| the humans' flipper | (295, 122, 1772), 130 m out | the car lands and must then climb the ramp on its roof: **+10.6 s** (rank 1: flips at 39.5 s, reaches the patch at 50.6 s) |
| the 270 km/h wall | (405, 149, 1666), 39 m up-ramp | takes all the speed; the car must then travel 39 m back down-ramp on its roof |
| any ramp edge above the finish | e.g. (418, 172, 1550) | 2.5 s past the finish and 27 m above it |

None of them fits inside the author's ~2 s of margin over the 41.0 s arrival.
**Either there is a fourth source we have not found, or the author's route does
not climb this ramp at all.** Both are worth an hour before anyone re-runs a
search: the trigger half of this map is now fully instrumented and the answer is
not in it.

## 5. Files

`att_lock.csv` — the full-lock tape's live 100 Hz trajectory (the flip in §3) ·
`att_fastb.csv`, `att_d1b.csv` — the fast route and its best steering-poked
variant · analysis sources in `att_tools.tgz` (`flight.rs`, `fastlook.rs`,
`curv.rs`).
