# Great Wtf of What #523

**The whole field coasts through the flight. Holding the throttle there is
worth 65 ms, and it takes the world record.**

**Great Wtf of What #523** — TAS **10.268** (+1.541) | AT 8.727 | WR 10.439 by BilboHaggins96

https://github.com/user-attachments/assets/bf02dbf0-dfaf-4656-a497-07569a2a6407

*The clip is the 10.374, with its own inputs drawn on. The 10.268 is validated but not yet filmed.*

The author time is **not** beaten. The human world record is, by **0.171**.

## What the map is

There is no launcher block. The car drives into a wall of 52 `GateFinish8m`
arches at x=578 at 162 m/s and **bounces** into the sky, where four gate points
at (496/528, 106/138, 816) each carry a checkpoint *and* a finish at the same
place — cross once and take both. All 47 checkpoint gates are one linked
checkpoint. That bounce is the entire 4.185 s cliff between rank 6 (10.994) and
rank 7 (15.179): the fast six fly it, the other 180 drive the ground route.

## Where the time came from

**The flight is drag-limited, not ballistic** — horizontal speed decays
114.9 → 50.6 m/s across the arc and vx reverses +44 → −13 in 200 ms. On a
ballistic arc the throttle does nothing; on this one there is something to push
against. Rank 1 coasts for **91 of its 379 airborne ticks**.

- `1000-1180 accel=1` — hold the throttle through the coast: **10.439 → 10.374**
- then three operators inside 50 ms of the launch (`826-828 brake=1`,
  `775-776 accel=1`, `824-827 accel=0`): **10.374 → 10.268**

The second is a compound — no single-lever sweep keeps any of the three.

## What is measured and closed

| lever | result |
|---|---|
| launch steer, off the stop | a **switch**, not a dial: 40 cells, 2 outcomes. Mode B flies 83 m further and crosses 0.281 s earlier but lands 21 m right; aimed back into the gate it scores **10.440**, one millisecond *slower*. Both modes converge. |
| the other gate columns | dead. At its own finish instant every run is 16-24 m from (528,138) and 48 m from the y=106 level, against a trigger volume of ~22 m. |
| which of the 52 arches to strike | 9 distinct strike indices, max range 39.9-361.9 m across them — **the humans sit on index 17, which is the maximum**. |
| flight brake | 21 windows, all inert. Only tick 821, the collision tick, is load-bearing. |
| interior steer magnitudes | the big right-hander (ticks 465-673 at +127) has load-bearing ends and a wholly inert middle. |
| sub-tick timing plane | **inapplicable, measured**: the finish is airborne, z spread 0.751 m against a 0.068 m per-ms budget — 11.1x over. The integer millisecond is the finest ruler this finish admits. |

## Files

`replays/TAS_10374.Ghost.Gbx` is the watchable ghost — oracle 10.374, kappa
1.000 (208/208 exact), first in-race sample this map own spawn. The 10.268 is
validated 5/5 but not yet regenerated for film.
