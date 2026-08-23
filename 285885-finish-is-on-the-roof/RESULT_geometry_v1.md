# 285885 `finish is on the roof to your right` — the map is played on the STADIUM, and the route is forced

Arm `roof285885`, 2026-08-22, node 78880.od. **The author time 43.079 was NOT
beaten and our 50.229 was NOT improved.** What this arm adds is the first
measurement of the *geometry* this map is played on, a correction to the
arithmetic the closure note states, an independent confirmation of the endgame
closure from a new instrument, and three enumerated negatives with positive
controls.

Rust only; no Python and no shell scripts. Every tool is a subcommand of an
existing CLI in `tools/`, on branch `roof285885`.

---

## 0. Controls — every one of them printed, every one of them passed

| control | result |
|---|---|
| `seedY` on the untouched map | **50.229** |
| human WR `bis197047_CONTROL_humanWR_61229` on the untouched map | **61.229** |
| `bis_418.6138_best` (the fast upright route) on the untouched map | **DNF** |
| `tmmaps ladder` return-to-origin, every invocation (4) | `control OK ... for all N ghosts` |
| ladder rung distinctness, every invocation | N rungs → N distinct file hashes |
| `dropscan` origin control (spawn moved to its OWN cell) | byte-identical decompressed body, 831 216 B |
| `dropscan` reference probe (a probe at the real spawn cell) | trace starts on the map's own spawn |
| `fk trace` self-check | reported per probe; a refusal is recorded as a refusal, never averaged in |
| md5 no-op census on every sweep family | printed (0/644, 24/828, 2/360) |
| the map file re-downloaded from Nadeo after an accident (§7) | md5 `1c902574afff5e48193928c4c3188ee8`, byte-identical to the store's copy |

Map md5 `1c902574afff5e48193928c4c3188ee8`.

---

## 1. What this map actually is: 113 blocks at y = 10, and a finish 500 m off the end of them

`tmmaps census` on the map file:

| | |
|---|---|
| waypoints | **2**: the spawn and the Goal. **NO CHECKPOINTS.** |
| unbaked blocks | 113, **every one of them at y = 10**, x 144…1296, z 624…1168 |
| baked blocks | 2 304 × `Grass` at y = 10, x 16…1520, z 16…1520 |
| the Goal item | `GateFinishCenter8mv2` at **(419.03, 144.00, 1704.64)** |

The Goal stands **500 m past the last block in z and 134 m above the highest
one**. Every surface the whole endgame is driven on — the ramp, the wall at
z ≈ 1668, the roof, the corner at x ≈ 507 — belongs to the **Stadium
decoration**, not to the map. That is why three arms could characterise the
endgame only by perturbing one tape and watching where the car happened to go:
nothing in the map file describes any of it.

The 113 blocks are one thing: a **walled corridor**. 111 `TechnicsScreen1x1Straight`
at y = 10 with 111 `PlatformWallStraightFCBGround` under them and 592
`TechnicsScreenFCLeft/Right` stacked to y = 34 on both sides, running diagonally
from the spawn at (144, 10, 656) to (1296, 10, 1168). One `RoadTechSpecialTurbo2`
at the second cell. **That corridor is the only accelerator on the map, and it
has exactly one exit.**

## 2. The new instrument: `tmmaps dropscan`

A map's spawn is an ordinary grid block. Move it to a cell, drive the car off it
under a fixed tape, and read the landing out of the live engine: one probe is
one map plus one `fk trace`. 530 probes were run. Two controls are built in and
both are refusals rather than warnings — the spawn moved to its own cell must
reproduce the untouched map byte-for-byte, and a probe at the real spawn cell
must start where the map's spawn is.

Three instrument facts came out of building it, all now in the code:

* **A constant input tape cannot be located.** The fork driver finds the decoded
  input array by value; a tape that is the same word 6 274 times matches a field
  of zeros elsewhere in the heap and fails the tape-identity control on
  6 274 of 6 274 ticks. The probe tape carries a fingerprint in the countdown
  ticks, which are inert.
* **`fk`'s car locator refuses on some fork points and not others, for the same
  tape and the same map** — `frac:0.20` fails where 0.10 and 0.35 pass. A probe
  tries several and takes the first trace that passes fk's own self-check.
* **A trace of ZEROES passes that self-check.** Position, velocity and
  quaternion are all self-consistent when they are all zero. 210 of 245 "ok"
  probes in the first wide scan were reporting the car at (0,0,0) for the whole
  race. They are now refused by name — see §3 for why they happen.

## 3. The hard limit the scanner found, and it is the first fact about the map

**A spawn cell outside the map grid produces no car at all.** Every probe with
cz ≥ 48 (z ≥ 1552) returns a trace of zeros. The grid is 48 × 48 cells, so it
ends at z = 1536 — and **the entire Stadium structure, including the finish,
stands beyond it**. The scanner can therefore probe the field and its own north
edge, and nothing further. That is a real limit and it is stated rather than
worked around.

What it can see, it saw. From cells on the north edge, dropped from y = 202:

| spawn | the car comes to rest at | what that is |
|---|---|---|
| x 144…560, z 1456…1488 | y ≈ 9, z 1528…1628 | the field's own ground, which runs on to z ≈ 1630 |
| **x 592…944, z 1488** | **y 49.9, z 1585.0** (eleven probes, all identical) | a flat DECK at y = 50 on the stadium's near rim |
| x 368…432, z 1520 | **y 144.0 / 155.9 / 149.2**, z 1621…1668 | the FINISH ROOF, at and above the finish's own height |
| x 1008…1168, z 1520 | y 149.5 / 153.7 / 158.3, z 1618…1668 | the same high rim, 700 m further east |
| x 1424, z 1488 | y 110.3, z 1667.5 | the rim's east end, 40 m lower |
| x 1456, z 1520 | y 98.9, z 1626 | ditto |

Two of those rows land on the plane the previous arm fitted to the finish roof
(`y = 410.5518 + 0.09211x − 0.17895z`) to within 0.05 m at (385, 1621) and
0.5 m at (408, 1668) — an independent confirmation of that fit from a
completely different instrument, 60–90 m from where it was measured.

**So the finish roof is one face of a high rim that runs east–west at
z ≈ 1620…1670, y ≈ 145…158, from x ≈ 340 to x ≈ 1180**, dropping to y ≈ 100 at
its east end near x ≈ 1450. The finish sits just below and just south of it.

## 4. The route is FORCED, and this is why

Three measurements, and together they close the "a route that does not climb
this ramp" option that the closure note listed as one of the three things that
would reopen the map.

1. **Grass is not a road.** 24 probes driven north from the corridor's own row
   at full throttle covered 120–440 m in 62 s — **about 6 m/s**. The car
   accelerates on the corridor to 415 km/h in the same tape. Off-road from a
   standstill the map is unusable; the WR crosses 400 m of grass at 620 km/h
   because it is *coasting*, not accelerating.
2. **The corridor is walled for its whole length** (592 screens, both sides,
   stacked to y = 34). A probe spawned 70 m south of it drives north and stops
   against the wall. The only exit is the far end at (1296, 1168).
3. **The stadium's near face is a cliff.** At x ≈ 400 the field ends at
   z ≈ 1536 at y = 8 and the roof is at **y ≈ 156 only 85 m further north**.
   There is nothing to climb.

Hence: every run must accelerate east along the corridor, leave it at
(1296, 1168) at ~620 km/h heading about 24° north of east, and cross the grass
to whatever it can reach — which is the north-east corner, where the WR
launches at (1570, 1500). The reachable launch region is roughly x ∈ [1350,
1600]; a 130° turn at 600 km/h to reach the deck at x ≈ 700 is not a thing a
car does. **The human route is not one of several; it is the only one.**

## 5. The arithmetic the closure note states is right, and its framing is not

`roof_CLOSURE_READ_FIRST_v1.md` §1 says *"the flip route cannot reach the author
time at ANY upstream speed"* on the ground that the earliest arrival at the top
of the climb by any tape is 37.972 and a perfect inverted crawl from there lands
at ≈ 46.4. Both numbers are right. The framing hides what the requirement
actually is, and the requirement is much harsher than the "2.042 s budget" the
same document quotes:

Measured here on a 9-rung arrival ladder (the real Goal item moved along the
final climb, 4.5 m above the surface, so a rung means *was here*, at any
attitude — every rung fires for the reference tapes, and the origin control
passes):

| | fast upright route | seedY (our record) | human WR |
|---|---|---|---|
| base of the final climb (302, 1758) | **37.978** | 41.279 | 41.279 |
| the patch (419, 1704.6) | **41.037** | 50.209 | 50.639 |
| 21 m up-ramp (440, 1695) | 41.584 | — | 55.036 |
| 46 m up-ramp (465, 1685) | 41.964 | — | — |

* The upright route crosses the patch **3.06 s** after the base of the climb.
* The inverted crawl takes **8.95 s** over the same 133 m.
* So the flip is worth **−5.89 s**, and the finish time from the base of the
  climb is `T + 8.95`.

**To beat 43.079 with the only flip anyone has ever made work, a run must reach
the base of the final climb at 34.13 — 3.85 s earlier than the earliest arrival
ever recorded there (37.978), on a route whose whole first 38 s is forced.**
That is the number a successor should be told; "2.042 s of budget after the
patch" is the same fact stated in a way that makes it sound reachable.

## 6. Three enumerations, each with its positive control in the same batch

New tool `ghost tape sweep` writes a family of rectangular-override candidates
and prints the md5 no-op census against IDENT. Scoring is the oracle ladder —
the only honest scorer on this map (`roof_RESULT_v1` §8), never the published
trigger model.

**(a) After the miss — can the corner flip ever come back? 644 candidates, 0.**
Template: the fast upright route, which crosses the patch at 41.037 and then
runs on up-ramp into the corner at (507, 1660) at 210 km/h. Overrides over race
40.5–43.5 s (7 starts × 4 lengths × 23 steer/accel/brake combinations, 0 no-ops).
**0 of 644 finish on the untouched map.**

`fk trace` says exactly what happens and it confirms the previous arm's account
from a new instrument: the car crosses the patch at 41.11 with `u_y` 0.981,
accelerates up-ramp to 210 km/h, hits the corner where the x ≈ 507 edge meets
the z ≈ 1660 wall at 42.91, loses 209 → 81 km/h in one tick, tips to `u_y`
0.788 (38°, past the 26° the trigger needs) — and stops there, 100 m from the
gate, at 5–18 km/h. The only flipper within reach of the patch wedges the car.

**(b) The airborne tumble at race 34.8–35.4 — a tilt source the survey never
saw, and it does not convert. 828 candidates, 0.**
The fast route is **AIRBORNE from 34.86 to 35.43**, at (389→365, 112→108,
1846→1837), at a constant **167 km/h**, rolled **74°** (`u_y` 0.447 → **0.262**)
and pitching at ~1.5 rad/s. `dv_y/dt` over that stretch is **−24.5 m/s²**, the
map's own gravity (−24.308) — it is a ballistic flight, not a scrape.

That is a real rotation source, at speed, that reaches three times the tilt the
finish needs, and **the previous arm's 797-probe rotation survey could not see
it**: that survey covers race 36.5–41.5 and a ±80 m disc around the patch, and
this event is at race 34.9, 141 m away. So it was worth attacking.

It does not convert. 828 overrides over race 33.0–35.4 (9 starts × 4 lengths ×
23 combinations; 24 no-ops, 691 distinct tapes), scored on three rungs:

| rung | fires | earliest |
|---|---|---|
| base of the climb (302, 128.3, 1758) — "got there" | 109 / 830 | **37.974** vs IDENT's 37.978 |
| the same place at 122.4 — "got there tilted past ~50°" | **0 / 830** | — |
| the patch (419, 148.5, 1704.6) | 107 / 830 | **41.037 = IDENT itself** |
| the untouched map — a finish | **0 / 828** | control `seedY` 50.229 in the same batch |

Nothing arrives at the base of the climb inverted; nothing reaches the patch
earlier than the unperturbed tape; nothing finishes. The tumble is 141 m too
early to be worth anything, and it cannot be moved.

**(c) The launch — see §8.**

## 7. An accident worth writing down

`dropscan`'s `--tapes` mode reuses the caller's map instead of writing one, and
the scratch-cleaning branch removed `mp` unconditionally: it **deleted
`map.Map.Gbx` out of the shared store**. Fixed (only a map the scan wrote may be
removed), and the file was restored by re-downloading it from
`core.trackmania.nadeo.live` — which returned **md5
`1c902574afff5e48193928c4c3188ee8`, byte-identical to the copy that had been in
the store**. That is worth keeping for its own sake: *the map is recoverable
from Nadeo, bit-for-bit, in one command with no authentication.*

## 8. The launcher has a ceiling, and it is 13 m below the rim

§3 raises the one route idea the geometry does not immediately forbid. The
finish is on a rim at y ≈ 145…158 whose **east end is at y ≈ 110 (x 1449) and
y ≈ 99 (x 1591)** — and that east end is exactly where the run already launches.
The human route flies *under* it: apex **74.9 m** at (1554, 1635), passing about
24 m below the rim's face, and lands on the deck behind it at y ≈ 53, from where
it drives 850 m west and 12.4 s to reach the foot of the climb. **Land on the
rim instead and the car would start at the height of the top of the climb.**

Measured, over two sweeps on the launch approach, scored by `dropscan --tapes`
(apex, resting place, and closest approach to the patch, read from a live trace
per candidate):

| sweep | window | candidates | apex ceiling (x > 1350) | best approach to the patch |
|---|---|---|---|---|
| e3, steering only | race 11.0–14.5 | 360 (2 no-ops) | 82.5 m at (1575, 1605) | 89.2 m |
| e4, steering + throttle-off + brake | race 11.5–14.3 | 608 (0 no-ops) | **86.1 m** at (1557, 1564) | 259.5 m |
| both | | **968**, 887 of them apexing in the launch region | **86.1 m** | — |

Positive control, in both invocations: the unperturbed tape's own probe reports
apex 145.7 m and closest approach **4.1 m** — the instrument sees a car that
reaches the patch, and it reports 4.1 m rather than 0 because the trigger is a
box, not a point.

> **The launcher's ceiling under 968 single-window overrides is 86.1 m. The
> lowest point of the rim that carries the finish is 98.9 m. It is 12.8 m
> short, and no candidate other than the identity twins gets within 259 m of
> the patch.**

This does not prove the rim is unreachable — a two-operator move, or a
different entry to the corridor's exit, was not enumerated — but it prices the
idea: the shortfall is 13 m of apex, ~15 %, against a launcher that the
previous arm measured to have *negative* gain in exit speed above an entry of
136.5 m/s. It also leaves the second half of the idea untested: **nobody knows
whether the rim is drivable westward at all**, and no instrument in this
project can find out without first putting a car on it, because the whole
structure is outside the map grid (§3).

## 9. What is left, and what a successor should not repeat

**Do not re-run:**

* the endgame. Four independent instruments now agree: the trigger needs ~26°
  of body tilt at the patch; the only flipper within reach is the corner at
  (507, 1660), 100 m away, and it wedges the car at 5 km/h (traced tick by tick
  here, §6a); 644 post-patch overrides finish nothing.
* the rotation survey. 797 probes over race 36.5–41.5 (previous arm) plus 828
  over race 33.0–35.4 (here, §6b) — two disjoint windows, and the second one
  covers the only ballistic tumble the route contains.
* a search for a shorter route on the ground. §4: one walled corridor, grass at
  6 m/s from a standstill, and a 148 m cliff between the field and the roof.
* the "2.042 s budget" framing. The real requirement is **arrival at the base of
  the final climb by 34.13 s**, against 37.978 for the fastest tape ever built
  and 41.279 for our record (§5).

**The two things that are genuinely open**, in the order I would try them:

1. **Land on the rim.** 13 m of apex, and then the unknown question of whether
   the rim is drivable west. The next move is not another single-window sweep —
   it is two coupled operators (an entry-speed change plus a steering change) at
   the launcher, scored on `dropscan --tapes`'s apex, with the rim's own height
   (98.9 m at x 1591, 110.3 at x 1449) as the bar. A candidate that clears the
   bar is worth a full trace immediately: if it lands on the rim at ~15 s at
   y ≈ 110, the whole 12.4 s traverse and the 4 s climb come off the run, which
   is far more than the 3.85 s §5 says the author time needs.
2. **The AT's provenance.** The medals are Nadeo's own derivation from 43.079
   (×1.07 / ×1.2 / ×1.5, rounded to the second: 46 / 52 / 65), so the author
   time was set by a validation and not typed in. But the author's own alt
   account takes **50.639 to reach the patch for the first time**, and our
   heavily-TASed upright route takes **41.037** — so 43.079 sits 2.04 s off a
   TAS and 7.56 s ahead of the only human line on the map. Either the author has
   a route nobody has found, or the map was validated in a state that is not the
   state it shipped in. TMX has one version, uploaded 2025-12-29 and never
   updated, no replays and no comments, so the question cannot be settled from
   the outside — but it should be recorded next to the map's "unbeaten" status,
   because it changes what "unbeaten" means here.
