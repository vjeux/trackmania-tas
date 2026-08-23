# 285885 `finish is on the roof to your right` — the roof inverts a car that falls onto it, and a wall stops it 37 m short

Arm `roof285885`, 2026-08-22, node 78880.od. **The author time 43.079 was NOT
beaten and our 50.229 was NOT improved.** What this arm adds is a rotation
mechanism nobody had found — **the roof inverts a car that falls onto it**, in
1.0 s, with no input, at 100 km/h, and a wall stops it 37 m from the finish
(§8) — the first measurement of the *geometry* this map is played on, a
correction to the arithmetic the closure note states, and five enumerated
negatives, every one of them with a positive control in the same batch.

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

## 8. THE ROOF INVERTS A CAR THAT FALLS ONTO IT — and a wall stops it 37 m short

This is the arm's real finding and it is a mechanism nobody has named. `dropscan`
can put a car above the finish roof, which nothing else on this project could,
and what happens then is the state the finish has always needed.

**A car dropped onto the roof's south face lands INVERTED and slides at speed.**
Spawn (13, 33, 47) — world (432, 202, 1520) — drives off the block at ~55 km/h
and falls 36 m onto the roof at (430, 166, 1580). Its own trajectory, read live:

| race | position | km/h | `u_y` |
|---|---|---|---|
| 2.87 | (432.0, 176.5, 1569.1) | 152 | **+0.055** (falling, rotating) |
| 3.83 | (426.5, 165.2, 1596.3) | — | **−0.987** |
| 4.67 | (424.1, 160.4, 1617.7) | 91 | **−1.000** |
| 5.67 | (418.1, 155.7, 1639.1) | 75 | −0.986 |
| 6.87 | (410.8, 150.9, 1660.7) | 66 | −0.996 |
| **7.27** | **(408.5, 149.4, 1667.0)** | **60** | **−0.997** |
| 7.47 | (407.7, 149.2, 1667.6) | **13** | −0.998 |
| 8.07 → | (407.0, 149.1, 1667.7) | 2 | — |

The finish needs `u_y ≤ 0.895`. This car is at **−1.000** — completely inverted
— travelling at **60–100 km/h**, on the finish's own roof plane (the measured
heights match `410.5518 + 0.09211x − 0.17895z` to a few centimetres), heading
straight at the patch. Every previous inverted crossing on this map was a
4-to-14 km/h crawl bought with 8.42 s of driving.

**And it is stopped dead by a wall at z = 1667.6.** 60 → 13 km/h in one tick,
and the car sits there for the rest of the race. **The patch is at z = 1704.64:
it is 37 m short.**

### The wall, laddered

15 rungs along the slide line, the real Goal item moved, 1.5 m above the fitted
roof plane. The positive control is inside the ladder: the first nine rungs fire,
in strict time order.

| rung z | 1600 | 1616 | 1632 | 1648 | 1664 | **1672** | 1680 | 1688 | 1696 | 1704 | 1712 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| fires at | 3.905 | 4.525 | 5.232 | 6.044 | 6.968 | **DNF** | DNF | DNF | DNF | DNF | DNF |

### 892 tapes cannot cross it

Rectangular overrides on the probe tape over race 1.5–6.5 s (11 starts × 3
lengths × 27 steer/accel/brake combinations, 0 no-ops), scored on five rungs:

| rung | fires |
|---|---|
| z = 1664, the last one the plain probe reaches — **the positive control** | **521 / 892** |
| z = 1672 | **0** |
| z = 1680 | **0** |
| z = 1696 | **0** |
| the patch, raised 2 m | **0** |

### And the south face does not extend east to where the wall has a gap

The fast upright route crosses the wall's z on its own up-ramp at x ≈ 460–507,
so the barrier is not continuous in x. Six more spawn cells (x 496–560, two
heights) × 231 tapes = **1 386 evaluations** were scored on **the fast route's
own up-ramp line**, six rungs taken from its trajectory at 300 ms intervals:

* **Control A — are the rungs real?** The fast tape on the untouched map fires
  all six in order: 41.024 / 41.324 / 41.654 / 41.954 / 42.244 / 42.544. They
  sit exactly on its own run.
* **Control B — do the probes reach anything?** A rung on the deck at
  (512, 52, 1610) fires for 12 of 231 at 6.904.
* **Result: 0 of 1 386 fire any rung on the up-ramp line**, and rungs on the
  *roof* at x 450 and x 500 fire nothing either — those probes fall past to the
  deck at y ≈ 50.

**So the roof's south face exists only over x ≈ 340–460, it is walled at
z = 1667.6, and east of x ≈ 460 there is no roof above the deck to fall onto.**

### What this changes, and what it does not

It does not beat the map. It replaces "there is no rotation source near the
patch" with something sharper:

> **There IS a rotation source at the patch — the roof itself, if you arrive
> from above. It inverts the car completely, in about 1.0 s, with no input at
> all, and leaves it doing 100 km/h. The map separates it from the finish by
> 37 m of wall, and the only part of the roof with clear air above it is on the
> wrong side of that wall.**

The previous arm's 797-probe survey is not contradicted. It asked whether a car
*on the roof* can leave it near the patch, and the answer is still no. This is
the opposite question — whether a car *above the roof* can arrive on it — and the
answer is yes, spectacularly, in the one place that does not connect.

For a successor that is a far better-posed target than "find 26° of tilt":
**find any way for the real run to be above the finish face (z 1668–1704,
x 419–507) with air under it.** The rest of the endgame is then already
measured.

## 9. The launcher has a ceiling, and it is 13 m below the rim


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
structure is outside the map grid (§3). **§8 does put a car on it** — from
above, by falling — and what it finds there is the inverted slide and the wall.

## 10. I built an airborne detector with a control, and it was still a decoy

§8 leaves one question: can the run get above the finish face? The natural
instrument is a gate placed some metres above the surface, because the trigger
has a floor 6.3 m below the item — a car ON the surface is below that floor and
cannot fire it, so a gate at surface + 8 m should fire only for a car in the air.

It looked properly controlled:

* 12 rungs, 8 m above the fast route's own trajectory, across the whole finish
  face. **The reference tape fires none of them.**
* 3 rungs 0.4 m above the same line. **The reference tape fires all three**, at
  39.719 / 40.924 / 42.149 — exactly where its own run is.

Scored over 920 rectangular overrides of the fast route across race 37.6–41.2
(0 no-ops among the families, 4 no-ops in the batch), **26 candidates fired the
8 m rungs**, including 7 that fired the rung 8 m above the patch itself.

**Every one of them is on the ground.** Traced live, they are simply driving a
line further up the ramp, where the surface itself is 8 m higher than the
reference line was: `s40800_l500_st-127_ax_b1` fires the +8 m patch rung at
41.229 while climbing smoothly through (423.4, 146.1, 1697.3) at 154 km/h with
`u_y` 0.982, y rising 0.9 m every 0.12 s with no ballistic signature at all.

> A gate above a sloping surface is a **height detector wherever the candidate's
> line differs from the line the gate was placed on**. The control that passes —
> the reference fires nothing — cannot exclude it, because the reference is the
> one line the rungs were fitted to. This is the same family as the "tilt
> detector at `plane(x,z) − Δ`" that cost an earlier arm six climb rounds, in a
> new disguise, and it survived a control that looked sufficient.

### What replaces it: `tmtraj airborne`

A reference-free detector. Free fall from the trajectory alone: a window in
which the second difference of `y` matches the map's own gravity, taken from a
5-point least-squares quadratic (a 3-point difference quantises to a ~10 m/s²
comb at 1 mm / 10 ms). No reference line, no contact bit — which on a
synthesised tape is the carrier's anyway.

**Positive control**: on the fast route it recovers the airborne window §6b
found by hand, 35.060–35.400 at (380, 112, 1842) at 167 km/h, and the run's fall
off the world at x = 507 after 44.25 — 17 episodes, all of them real.

### What it says about the finish face

The 22 candidates that fired the decoy rungs, re-read with it, restricted to the
finish face (x 380–505, z 1660–1760):

* **17 of 22 do have a real ballistic episode there** — so the detector is not
  simply silent.
* Every one of them is **0.10–0.15 s long, at 170–210 km/h, and sits at
  z ≈ 1665–1669**: they are the car hopping the same z ≈ 1668 lip that stops the
  inverted slide from the other side, taken at speed from the up-ramp side.
* The apex gain is **0.5–1.0 m**, and every episode is at race **42.1–43.0** —
  *after* the patch, on the way past it.

At 1.7 rad/s (the measured ramp-edge tumble rate) 0.15 s is 15°, against the 26°
the trigger needs — and these hops rotate essentially not at all, because the
car barely leaves the surface. **There is no usable air over the patch in this
family.**

So the z ≈ 1668 feature is now characterised from both sides: **a lip that an
inverted 60 km/h slide from the south hits head-on and dies against, and that a
200 km/h upright car from the north skims over with 0.1 s of air.**

## 11. What is left, and what a successor should not repeat

**Do not re-run:**

* the endgame *as a search for tilt on the roof*. The trigger needs ~26° of
  body tilt at the patch; the only flipper reachable *along* the roof is the
  corner at (507, 1660), 100 m away, and it wedges the car at 5 km/h (traced
  tick by tick here, §6a); 644 post-patch overrides finish nothing. **§8
  reframes this**: the tilt is free if you arrive from above, so the open
  question is altitude, not rotation.
* the rotation survey. 797 probes over race 36.5–41.5 (previous arm) plus 828
  over race 33.0–35.4 (here, §6b) — two disjoint windows, and the second one
  covers the only ballistic tumble the route contains.
* a search for a shorter route on the ground. §4: one walled corridor, grass at
  6 m/s from a standstill, and a 148 m cliff between the field and the roof.
* the "2.042 s budget" framing. The real requirement is **arrival at the base of
  the final climb by 34.13 s**, against 37.978 for the fastest tape ever built
  and 41.279 for our record (§5).

**The three things that are genuinely open**, in the order I would try them:

1. **Get above the finish face with air under it** (§8; §10 says how NOT to look
   for it). This is the arm's
   recommendation and it supersedes "land on the rim" as the way to state the
   problem. The fall does the whole endgame for free: 1.0 s, no input, `u_y`
   −1.000, 100 km/h, on the finish's own plane. What is needed is not tilt and
   not budget — it is **altitude over z 1668–1704, x 419–507**. The south face,
   which is the natural place to fall from, is walled 37 m short (892 tapes,
   0 crossings, 521-firing control) and east of x ≈ 460 there is no roof there
   at all (1 386 evaluations, two controls).
2. **Land on the rim** (§9), which is the same idea from the other end: 13 m of
   apex short, from a coupled entry-speed + steering move at the launcher. Note
   what §8 adds to it — landing on the rim's *south* face buys the inverted
   slide and then the wall, so the landing must be east of x ≈ 460 or north of
   z ≈ 1668.
3. **The AT's provenance**, worth an hour and no more. The medals are Nadeo's
   own derivation from 43.079 (×1.07 / ×1.2 / ×1.5, rounded to the second:
   46 / 52 / 65), so the author time came from a validation and was not typed
   in — but the map carries **no embedded author ghost** (no chunk
   `0x0305B00F`), its header says `validated="1"` and it also names
   `EPP_EditorPluginLoads`. Meanwhile the author's own alt account takes
   **50.639** to reach the patch for the first time and our heavily-TASed
   upright route takes **41.037**, so 43.079 sits 2.04 s off a TAS and 7.56 s
   ahead of the only human line on the map. TMX has one version, uploaded
   2025-12-29 and never updated, no replays and no comments, so the question
   cannot be settled from outside — but it belongs next to the map's "unbeaten"
   status, because it changes what "unbeaten" means here.
