# 267460 — "Impossible Mini Trial 2" — the map, measured

Everything below was measured on this node, 2026-08-18, against the untouched
map file (Nadeo's own copy, sha256 `4f0db768…`). Times in seconds.

## 1. The map is a chain of floating platforms inside two solid screens

31 blocks, 17 items. **22 of the 31 blocks are `CanopyCenterFlatBase`** — the
big flat stadium screens, rotated vertical. They are not decoration: they are
the walls that make this map what it is, and every route question on it is
"which hole in which wall".

Free blocks keep their world position in chunk **`0x0304305F`** (24 bytes each:
Vec3 pos + Vec3 pitch/yaw/roll), *not* in the block record — the block record
carries cell `(-1,0,-1)` for every free block, so `tmmaps list`/`listall` alone
tells you nothing about where they are. The first 24 records of that chunk are
the 24 free blocks **in block order**, which is what lets them be named:

| block | model | position | what it is |
|---|---|---|---|
| B0 | PlatformIceStart | (925,128,745) | the start platform (roll 45°) |
| B23,B24 | PlatformIceBase | (893,128,745), (861,128,745) | the ice run west (roll 45°) |
| B25,B26,B27 | PlatformDirtBase | (736,112,737), (736,96,709), (768,96,709) | **the pit** (roll 30°) |
| B28 | PlatformDirtTiltTransition1UpLeft | (768,112,737) | pit entry |
| B29 | PlatformDirtBase | (830,112,736) | the run-up to the turbo gate |
| B30 | PlatformDirtSlope2Base | (990,56,704) | slope by the finish level |
| B1–B5, B13–B22 | CanopyCenterFlatBase | see walls below | the two screens |

Plus 7 fixed `PlatformGrass*` blocks from the cell list: the landing L at
**y=40, x∈[1056,1120], z∈[640,736]**, and **the finish platform at y=48,
x∈[992,1024], z∈[640,672]**.

Items: turbo on the start line (909,141,755); **the big turbo gate
`GateSpecial32mTurbo` at (846,114,720)**; **the finish `GateFinishCenter32mv2`
at (990,58,656)**; `GateSpecial32mNoEngine` at (1056,49,672); four
`ObstaclePillar2m` at x=1023, z=641/649/657/665, y=50; nine inflatable borders
on the ice run at x 784–816, y 133–136.

## 2. The two screen walls, and their holes

Each panel is 32 m square. Taking the stored position as the panel CENTRE fits
every observation below.

**Wall at z=740** (between the start platform and everything else):

| panel row | y covered | x covered |
|---|---|---|
| y=87 | 71–103 | 781–941 |
| y=119 | 103–135 | 717–909 |
| y=151 | 135–167 | 941–973 |
| y=183 | 167–199 | 749–909 |

**Wall at z=686** (between the flight corridor and the finish):

| panel row | y covered | x covered |
|---|---|---|
| y=40 | 24–56 | 880–976 |
| y=72 | **56–88** | **816–1072** |
| y=104 | 88–120 | 784–928 and 1040–1072 |
| y=136 | 120–152 | 912–1008 |
| y=168 | 152–184 | 816–928 |

Two consequences decide the whole map:

* **You cannot drop south off the start platform into the turbo gate.** The
  start is at y≈135–141 and x≈845–925; the z=740 wall covers y 103–135 for
  x 717–909, so the 21 m fall from the ice run onto the turbo gate is behind a
  screen. Tested exhaustively, not inferred: **2600 hand-built tapes** (drive
  the human's line to tick T, then hold steer S for D ticks, over
  T ∈ [60,320], S ∈ ±{32…127}, D ∈ {15…120}) — 1114 of them still reach the
  first progress gate, and **0 reach any gate within 32 m of the turbo gate.**
  The pit trip is not a mistake; it is the only way west of the wall's edge at
  x≈717, and the human crosses z=740 at x≈726, y≈116.
* **The only doorway to the finish is `y < 56` at `x > 976`** (the gap in the
  y=40 row, under the y=72 row). That is why the human's last five seconds look
  the way they do: it lands on the grass at y≈43, drives to x≈1085 — past the
  wall's east edge at x=1072 — and comes back on the far side.

## 3. What the human run does, gate by gate

Splits from relocated finish gates, each its own map, one map per worker root:

| gate | s | what |
|---|---|---|
| (835.5,135.7,749.8) | 1.985 | flat out west on the ice, 167 km/h |
| (758.4,133,749) | 3.946 | off the west end of the ice, airborne, braking |
| (716.3,109.4,728.4) | 5.979 | bottom of the pit, 69 km/h |
| (719.3,128.9,762.1) | 9.825 | top of the climb back out |
| (740.6,108.8,727.5) | 12.969 | back down, charging east on B29 |
| (840,114.3,711) | **15.239** | **through the big turbo gate** |
| (995.7,57.8,712.4) | 18.018 | mid-dive, 257 km/h |
| (1091.4,42.6,693.2) | 19.435 | far end of the grass, U-turn |
| (1020,52,680) | 21.408 | back west, past the no-engine gate |
| finish | **23.068** | crosses at 8.5 km/h |

**Nine seconds of the 23 are spent in the pit** (3.9 → 12.9), covering 151 m at
45–100 km/h on 30°-rolled dirt. **Four more are the endgame** (19.0 → 23.068):
land, overshoot, U-turn, cross the no-engine gate, jump the 32 m gap from the
y=40 grass up to the y=48 finish platform, thread the pillars, and coast in
with the engine dead. The human crosses the line at 8.5 km/h after losing
75.9 → 8.5 km/h in the last half second — it arrives into the flag structure.

## 4. The near miss that is not one

At 18.018 the car is at (995.7, 57.83, 712.37) at 257 km/h and the finish is at
(990, 58, 656): **same x, same y to 0.2 m, 56 m adrift in z.** That looks like
the whole answer, and it is worth writing down why it is not.

* The finish trigger is generous but finite: relocating the gate onto the flight
  line and sweeping z, it fires for z ∈ [700,716] against a car at z=712.4 —
  about ±14 m. In y it is asymmetric: a gate at origin y fires a car in roughly
  **[y−9, y+1]**, measured against 49 tapes at a known height.
* Aiming the dive south works, up to a point. A ratchet of relocated gates
  pulled the corridor from z≈712 to **z≈688**, and 49 tapes out of a 1980-tape
  launch sweep get **through** the z=686 wall, at (1010, 46, 680), at 17.96 s.
* But the doorway is at x > 976 and the finish is at x = 990, so a car that
  crosses the wall going east arrives on the far side already past the flag,
  four metres too low, and falls into the void. To fire the gate in the air it
  would have to cross z=686 at x ≈ 980 and then travel −34 m in z for +10 m in
  x, i.e. fly almost due south — which the launch cannot produce. Measured
  ceiling on the aim: Δz ≈ −40 m over the flight, against the −60 m needed.
  Turning during the ground run-up to buy more (2450 tapes, steer +32…+127 held
  20–130 ticks from t ∈ [11.5,15.0]) throws the car off B29: **1 survivor.**

So the route is forced, and the author time has to come out of doing the same
route faster — chiefly the nine seconds in the pit.
