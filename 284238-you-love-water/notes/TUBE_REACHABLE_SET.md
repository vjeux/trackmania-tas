# 284238 — the tube-exit reachable set, and three aligned per-tick state tables

`state_ADDENDUM_v3_tube_reachable_set.md`. Sidecar; supersedes nothing. Written
for the third arm (closed-loop policy transfer) as much as for the record.
Times in seconds. Every state is **re-simulated** and read out of the engine
(`fk btraj2` / `fk sweep`, controls in `state_RESULT_v1_launch_state.md` §0);
closing plain-oracle control this session: record **440.238**, `best_97325`
**97.325**, Yhomas on his own map **46.112** — all exact.

---

## 1. The answer to the reachable-set question: THE TUBE CAN DELIVER IT

The question was whether our tube can produce Yhomas's arc — he crosses the lane
peaking at canonical **z 929.8** while our record peaks at **917.2**. If +41 m/s
of lateral velocity is outside the reachable set, the tube is not the answer.

**It is inside, comfortably.** A steer delta over the arc (tick windows 2050–2210,
race 19.0–20.6), measured as peak canonical z on the lane with the speed there:

| Δsteer over the arc | lane peak z | speed at it | CP2 |
|---|---|---|---|
| 0 (the record) | **917.2** | 96.1 | collected, 45.80 |
| +5 | 919.8 | 92.7 | misses by 24.2 m |
| +10 | 924.4 | 95.7 | misses by 79.7 m |
| +15 | 928.0 | 96.9 | misses by 122.4 m |
| **+20** | **931.8** | **97.8** | misses by 103.0 m |
| +25 | 936.5 | 99.6 | misses by 82.3 m |
| +30 | 939.4 | 100.5 | misses by 84.4 m |
| +40 | 941.7 | 93.7 | misses by 135.0 m |
| **Yhomas** | **929.8** | **97.2** | **collected, 69.40** |

Δsteer between +15 and +20 reproduces **his crossing height and his speed at it**
— 928.0–931.8 at 96.9–97.8 against his 929.8 at 97.2. So the deficit is not a
capability of our tube, our lane, our water surface or our boost pads. **Our car
can arrive at the lane exactly as he does.**

## 2. And it still does not fly, for a reason that is now the whole problem

Every variant that reproduces his crossing **misses the checkpoint by 82–135 m**,
and the failure is no longer the wall slam (the one-tick loss falls from 8.71 to
0.03–5.05). The car reaches his lateral state at **the wrong place along the
lane**: peak z occurs at 22.0 or later, still rising, tens of metres past where
his peaks, so the kicker fires with the car in the wrong part of its swing.

Stated as a boundary-value problem, the launch needs **three** things matched and
a steer delta on the arc is one knob:

| | ours (record) | reachable with Δsteer | Yhomas |
|---|---|---|---|
| lane peak z | 917.2 | **929.8 ✓** | 929.8 |
| speed there | 96.1 | **97.2 ✓** | 97.2 |
| **x where the peak occurs** | ~852 | **~950+, still rising ✗** | ~855 |

The third row is the whole remaining gap. It is a *phase* mismatch, not an
energy or an attitude one, and that is a much better-posed problem than anything
this map has had: the crossing must peak **early**, over the same stretch of
lane he does, not merely reach the same height.

## 3. What is actually different, per tick — the divergence instant

Ours and Yhomas's cycle 1 in canonical coordinates, paired by phase (his times
are 7.400 earlier). **They are the same car through the tube and the first half
of the arc:**

| phase | ours: y, z, speed, vz | his: y, z, speed, vz |
|---|---|---|
| arc bottom | 1901.9, 881.16, 81.97, **+2.79** | 1903.2, 881.10, 81.27, **+0.49** |
| +0.2 | 1896.6, 882.01, 83.24, +12.11 | 1897.9, 881.70, 82.95, +9.58 |
| +0.4 | 1891.4, 883.85, 83.71, +21.27 | 1892.7, 883.28, 84.45, +19.07 |
| +0.6 | 1886.6, 886.63, **83.79**, +30.23 | 1887.8, 885.83, **85.54**, +27.80 |
| +0.8 | 1882.5, 890.14, **83.30**, +36.41 | 1883.6, 889.08, **86.42**, +33.26 |
| +1.0 | 1879.3, 894.08, **82.40**, +38.85 | 1880.4, 892.61, **87.65**, +34.05 |
| +1.2 | 1877.0, 898.13, **81.24**, +39.12 | 1878.0, 896.24, **89.00**, +36.96 |
| +1.4 | 1875.4, 902.09, **79.85**, +37.10 | 1876.1, 900.16, **90.21**, +39.38 |
| +1.6 | 1874.5, 905.72, 78.17, +33.19 | 1874.8, 904.25, 91.23, +40.84 |
| **deck (y 1874)** | 908.86, **76.21**, **+27.60** | 908.45, **92.03**, **+41.42** |

**They arrive at the deck within 0.4 m of each other in z and 15.8 m/s apart in
speed.** Our lateral velocity peaks at +39.1 and is already decaying; his is
still climbing. The record **lifts the throttle from 19.65 to 20.6**, right
through the arc; he is on the gas.

Our lateral velocity then decays to +0.3 by the lane peak while his only falls to
+5.1, and I believe the mechanism is time rather than grip: the sideslip scrubs
off per second on the ground, and at 76–84 m/s we are on that lane appreciably
longer than he is at 90–97.

**But restoring the throttle alone does not fix it** — 6 windows spanning ticks
2060–2240 forced to full gas: lane speed rises to 100.9–109.7 (so the deficit is
genuinely the lift), lateral velocity at the lane is still +0.1 to +2.9, and
every one misses the checkpoint by 107–129 m. Speed on the same arc goes into
+x, not +z.

## 4. The three tables, banked for the policy-transfer arm

`state_align_v1.tgz` → `state_align/`, all in **canonical module coordinates**
(the −120° / −56 m screw undone, so copy k is directly comparable with copy 0 and
with the sibling map), one row per 10 ms tick, columns
`time_ms,x,y,z,speed_ms,vx,vy,vz`:

| file | what | ticks |
|---|---|---|
| `state_yhomas_cycle1_canonical.csv` | Yhomas 46.112 on 279008, re-simulated | 683 |
| `state_ours_cycle1_canonical.csv` | our record's cycle 1, re-simulated | 941 |
| `state_ours_copy0_standingstart_canonical.csv` | our standing start (a launch that WORKS on our map) | 1070 |

These are re-simulations, not decodes, so they are valid for synthesised tapes
too — which is the point. The third file matters for a controller: it is a
working launch **on our geometry, with our boost pads**, so a policy fitted on
279008 can be checked against a local positive example before it is trusted.

Key states, for a controller's boundary conditions (canonical):

```
tube exit / arc bottom   y 1901.9   z 881.2   v 82.0   vz  +2.8      (both cars)
deck, target             y 1874.0   z 908.5   v 92.0   vz +41.4      (Yhomas)
deck, ours               y 1874.0   z 908.9   v 76.2   vz +27.6
lane peak, target                   z 929.8   v 97.2   at x ~855
kicker, target                      z 922.5   v 98.2   vz -24.4
wall plane y=1918, target  x 980.2  z 913.9   v 80.8   -> CP at 69.40
wall plane y=1918, ours    x 980.2  z 923.4   v 77.4   -> CP at 45.80
copy 0 standing start (works): kicker vz -18.8, wall plane x 969.7 z 915.4
```

## 5. Kill criteria and the next distinct lever

The tube lever is **not** dead — §1 says the reachable set contains the target —
so its kill criterion is now specific: *if no single arc input can make the
lateral crossing peak before canonical x ≈ 880 while reaching z ≥ 928, the arc's
phase cannot be corrected from the arc and the lever dies.* That is one grid
(peak position × peak height) and it is the next thing I would run.

The distinct lever after it, and the one I would name now so it is on the record:

> **Do not drive the four copies the same way.** Copy 0's launch works and never
> comes out of the tube — it accelerates across the deck from a standstill and
> enters the lane from the outside, which is exactly how Yhomas enters all four
> of his. The sibling map replaces the water ramps with tech blocks in every
> copy, i.e. the author's own remix removes the tube-fed launcher. So the route
> question is: **for each copy, is there an entry that avoids the tube**, and
> what does it cost? That is a geometry question about the map, answerable
> without a search, and it is the one thing nobody has asked.

## 6. Enumerations for every negative in this file

* arc steer delta: 5 windows (2050:2120, 2080:2150, 2110:2180, 2140:2210,
  2050:2210) × 12 values (±5, ±10, ±15, ±20, ±25, ±30, ±40, ±80, ±127 as listed)
  = 60 variants, all re-simulated on the untouched map. Exhaustive over that
  window family at 0.3 s granularity and those magnitudes; nothing else.
* throttle restoration: 6 windows spanning 2060–2240 (§3).
* Earlier, in the other sidecars: 60+ single-window lane variants
  (`state_RESULT_v1` §6) and 36 two-window pulse variants
  (`state_ADDENDUM_v1`).
