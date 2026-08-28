# Great Wtf of What #523

**Every human holds full steering lock through the collision. Letting a third
of it go, for the three ticks that span contact, is worth 0.43 s.**

**Great Wtf of What #523** — TAS **9.006** (+0.279) | AT 8.727 | WR 10.439 by BilboHaggins96

https://github.com/user-attachments/assets/0bbc4f27-1340-4362-9a67-c937b38d2bd8

*The 9.006, with its own inputs drawn on, against BilboHaggins96 world record.*

The author time is **not** beaten. The human world record is, by **1.433**.

## The three ticks

The map launches the car off a finish-gate rail and flies it to a gate in the
sky. Everyone in the field arrives at that rail holding **steer −127**, full
lock, and holds it through the impact.

**This run comes off to −84 for the three ticks that span contact.** It leaves
the rail with **−145 m/s of southward speed** where the world record leaves with
−106, and it is causal rather than incidental: put the world record's −127 back
over exactly those three ticks and the lap loses **0.43 s**.

It is also not the approach. The drift into the rail is the world record's own —
76.6° of yaw against his 78.7°, the same brake window opening at 5.590. The
whole difference is three ticks of steering at the moment of contact.

## Why it stops at 9.006

**The launcher is a 2.4 m lip.** The gate's collision surface is one
`CPlugSurface` of 2586 triangles; the rail the car hits spans y 7.18–9.60, with
**4.4 m of open air above it** before the arch bar. The car sits at y 8.10 and
rides over a metre and a half of steel at 145 m/s. Anything higher passes
through the fence untouched.

The rail is **continuous** — items every 2.00 m spanning ±0.99, so it tiles with
2 cm seams. There is no special place along it. Where the launch happens is a
property of how the car arrives, not of the geometry.

**And the sky gate is a keep-out.** Release more lock and the launch gets
stronger *and misses*: the six strongest launches in the corpus are all DNF,
crossing the gate plane at x 546.0 against its eastern edge at 544. Leaving
harder means arriving sooner means less time to drift west. **Launch strength
and flight aim are the same variable.**

The envelope, over ~50,000 candidates:

| exit vy | exit vz | reach the gate |
|---:|---:|---:|
| 100 | **−140** | **341** |
| 100 | −120 | 39 |
| 110 | −90 | 61 |
| 50 | −140 | **0** |
| 60 | −50 | **0** |

**Every launcher that reaches the gate has vy 90–110, and the best vz among them
is −140 — this run's.** The author time needs roughly −159 while still holding
vy ≥ 90, and that quadrant is empty.

## What is left

The run reaches the gate **42 m higher than it needs to**. Trading that surplus
climb for southward speed is the one operator nothing in this search could
express — every family here moves the launch, and moving the launch moves the
aim with it.

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

Then **`830-870 steer=0`** — releasing an inherited full lock in the flight,
in the region where steering supposedly does nothing: **10.258 → 10.255**.

## The sign was inherited too

Every sweep for eleven leases explored magnitudes *around* the human `steer=-127`
through the collision, or releases toward 0. **Positive steer was never in an
operator set** — not tested and rejected, simply never generated, because the
ranges were built around the value the human used. Eleven leases searching a
half-space.

**`steer=+30` through the collision ticks: 10.255 → 10.013.** Then 10.008,
then **10.007**.

*A region excluded by an assumption is not a region searched — and the
assumption can live in the SHAPE of the operator rather than in anything
written down.* Every gain on this map came the same way: an inherited input
nobody questioned.

Closed since, each with a mechanism rather than a bare null: shaped operators
(ramp/cosine/triangle/pump, 139 of 144 DNF — the amplitudes shapes need are
wider than this collision tolerates), the single-tick collision sweep (266
evaluable, 0 improvements — saturated), and the 103-tick approach block, which
turns out to be a **forced corridor** (875 of 900 DNF) rather than an
unexamined one.

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
