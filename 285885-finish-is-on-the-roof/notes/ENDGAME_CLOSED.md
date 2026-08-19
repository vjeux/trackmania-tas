# 285885 — the 28.3° "bank" is a wall face, and the corridor above the finish is the same plane. The map is closed.

*Third agent. Final measurement file. Read with `att_TRIGGER_IS_BODY_POINT_v2.md`
(corrected trigger model), `att_DIRECTION3_INVERTED_ARRIVAL_v1.md` (the flip's
value and cost) and `att_DISPLACEMENT_IS_INERT_v1.md` (the retired lever).
Everything here is from live `fk btraj` trajectories or the plain oracle.*

---

## 1. The 28.3° "bank" is the face of the wall that stops you

It was the last named exception in my own data, so it had to be checked. It is
not tiltable ground. It is the **impact**:

| t | position | speed |
|---|---|---|
| 60.500 | (404.20, 148.095, 1666.50) | **273.7 km/h** |
| **60.550** | (405.14, 148.432, 1667.40) | **76.0 km/h** |
| 60.600 | (405.85, 149.030, 1667.56) | 60.8 km/h |

**274 → 76 km/h in one 50 ms tick.** The 28.3° reading is the car's attitude
*during* the impact, not the slope of a surface it was driving on. You cannot
leave that feature carrying speed, because that feature is what removes the
speed. The tilt and the speed loss are the same event.

What it does do is flip the car, fast: `u_y` runs 0.914 → 0.416 → −0.076 →
−0.416 → −0.806 in **0.52 s** — about **3.8 rad/s**, twice the ramp-edge rate.
But in those 0.52 s the car travels **4.2 m** of the 39.7 m to the finish, at
50 km/h and falling. Measured cost of the whole conversion, wall to a footprint
crossing: **rank 2 hits at 60.550 and its lowest footprint crossing is at
79.900 — 19.4 s**, against a **2.042 s** budget. 9.5× over, and it is the
cheapest flip on the map.

## 2. The flight the coordinator asked me to compute

Costed anyway, because a clean geometric negative is worth having. From the
feature toward the finish the direction is **(+13.9, +37.2) — downhill**, and
the launch point is genuinely above the finish roof, so the arithmetic is
favourable: a horizontal launch at **≈ 48 m/s (172 km/h)** falls the 3.4 m in
0.83 s and arrives exactly at roof level over the footprint, and 0.83 s at even
a quarter of the measured tumble rate is far more than the 26° needed.

**The launch does not exist.** The only tape that travels that line arrives at
76 m/s pointed straight down it — direction (0.372, 0.928) against the
corridor's (0.349, 0.937), within 1.4° — and is stopped dead by the wall at
s = 0. **The wall sits at the mouth of the corridor, square across the one
approach that would work.**

## 3. And the corridor is the same plane

Rank 3 drives the whole corridor inverted and grounded, so its contact patch is
its tested point — which makes it a surface probe. Calibrating the offset on 322
samples near the finish (where the plane fit is interpolation) and then
extrapolating **up** the corridor:

> up-corridor samples, s = 19.6 … 35.0: residual vs the extrapolated finish
> plane **mean −0.123 m, sd 0.160 m** (n = 12)

**It is one continuous roof**, from the wall at (405, 1667) to past the finish —
the same 11.4° plane, same gradient, no join, no step, no change of slope for a
car to be tilted by. That kills the last variant of the idea: there is no
transition between two differently-tilted surfaces anywhere in reach of the
footprint. The "corridor" is not a second surface; it is the same roof, further
up.

## 4. The map, closed

Every route to the trigger now has a measured price, and the budget is 2.042 s:

| what the trigger needs | why it is unavailable |
|---|---|
| arrive lower | impossible: the deficit is **144.092 ± 0.024 for every fast line whatever it does** (7 tapes, 3 families, 0.913 m of spread, slope +0.003 m/m) |
| tilt ≥ 26° on the approach | the car takes the surface normal; full lock **flattens** it; suspension ≤ 5 mm |
| leave the surface and rotate | ramp radius ~1500 m ⇒ needs 440 km/h against 190; and a launched car is **1.23 m under the rising roof after 0.5 s** |
| clip an edge or a step | the roof is flat to **0.026 m/m** over x ∈ [405,428] on three lines, and the corridor above is the **same plane** |
| flip early and drive on | **11.2 s** (rank 1) — inverted the car crosses at 20–45 km/h against 190 |
| flip on the wall | **19.4 s** (rank 2), and the wall blocks the only usable approach |

> **285885 is characterised, not merely unbeaten.** The finish tests a point
> 0.84 m above the origin in the car's body frame. Every upright crossing of the
> footprint, by any line, misses by 70–140 mm. Closing it requires ~26° of body
> tilt, the map contains exactly three sources of tilt, and the cheapest costs
> 5.5× the entire time budget. **The author's 43.079 is not reachable by any
> route through this finish that we can construct, and we can now say why in one
> sentence rather than by exhaustion.**

The one thing that would overturn this is a rotation source we have not found —
and the search space for it is now small and specific: **something within ~40 m
of (419, 1704.6) that is not on the 11.4° plane and is not the wall.** Three
agents have not found one; the plane fit says the region is flat to 38 mm over
its whole extent; and the author's own two accounts both go the 11.2 s way
round, which is itself evidence that the author had no better source either.

## 5. What I would tell the next agent to do instead

**Re-examine whether 41.037 s is really the floor for reaching the patch.** The
whole of this analysis takes the arrival time as given and asks how to fire the
gate. But the budget is `43.079 − arrival`, so **every 100 ms saved upstream is
100 ms of flip budget**, and the 11.2 s route only needs to come down by a factor
of 5. Nobody has optimised the first 35 s: the previous agents measured it as
"within a metre of the WR's line" and moved on, and the WR is a human. If the
approach can be driven in 30 s rather than 41, the humans' own flip route
finishes the map at ~41 s and beats the author time without any new physics.

That is a route-search problem on a section nobody has searched, with a working
objective (arrival at the patch), a known-good instrument, and 11 s of slack to
find. It is a much better bet than another attempt on the trigger.

## 6. Files

`att_firestates.txt` (direction of travel and attitude at every fire in the
dataset) · `att_corridor.txt` (the corridor traverse) · `att_bt_*.csv` (six live
trajectories) · `att_tools.tgz` (adds `corridor.rs`, `extend2.rs`,
`flightcalc.rs`, `dirfire.rs`).
