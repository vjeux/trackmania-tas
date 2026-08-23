# 173691 "Spring 2023-15 (Underwater)" — the finish trigger, mapped; and a car that climbs 21 m

Arm **B2**, 2026-08-22, node `3089.od.fbinfra.net`, branch `uw173691-finish-b2`.
Rust only. Every number is read off the live engine re-simulating a written
`.Ghost.Gbx` (`fk trace`) or off the plain oracle (`tmmaps oracle`), and every
null in this page was run with a positive control in the same batch.

## HEADLINE

**No finish. But the map is provably completable, the target is now a measured
box rather than a guess, and — for the first time on this map — a car has
gained 20.6 m of height, which is more than the finish needs.**

| | |
|---|---|
| **the map is completable** | the `.Map.Gbx` header says `validated="1" authortime="2672290"`, with the auto-derived medals `gold 2833000 / silver 3207000 / bronze 4009000`. **Somebody finished this map in 2672.290** — 44m32s. A route exists; we have not found it. |
| **the finish trigger, measured** | not "a plane at z = 494.5, y 130…194". It is **three boxes**, one per `GateFinish` row, each **starting ≈ 1 m above its block's base**: live from **≈ 131 to 226**, with dead slivers at the 162 and 194 seams; **x within about ±12 m of each gate's centre** (1360, 1392, 1424, 1456, 1488) — the cell seams at 1376/1408/1440/1472 are DEAD; z **≈ 486 … 503**, not a plane. |
| **the wall, to a metre** | a car driving on a platform at **130.16** inside a gate cell does not fire in **480 blind tapes**; the same platform at **138.16** fires **70/480**. The lowest firing ever recorded is **131.27**. The deck is 114.16, so the wall is **16.0 – 17.1 m**. |
| **a car that climbs 20.6 m** | one blind run in 2 000 wedged itself at the deck's south-east corner and was thrown to **134.78** — 20.6 m above the deck, **3.5 m above the trigger's floor**. Hill-climbing that tape reaches **135.18**. It is reproducible, it re-simulates, and it is at the wrong x and z by ~32 m. |
| **what is closed** | the massif hop (grass caps the car at 8.5 m/s → 15 m of reach against a 29 m gap); water as a medium (no translation under throttle, brake, full steer or any pulse pattern); glide tuning (no in-flight input beats holding throttle); 17 068 blind oracle runs from 68 deck spawns under the gates, 0 finishes, control 13/251. |

---

## 1. THE MAP IS COMPLETABLE — read this before writing another null

```
<desc envir="Stadium" mood="Day" type="Race" ... validated="1" nblaps="0" .../>
<times bronze="4009000" silver="3207000" gold="2833000" authortime="2672290" authorscore="0"/>
```

That is 173691's own header. `validated="1"` is set by the editor's validation
run, and the three medals are the auto-derived multiples of the author time
(×1.06, ×1.20, ×1.50), which is what a validation writes — not a number a
mapper can type. **2672.290 is a real completion of this map.**

So every "impossible" on this page is a statement about what we have measured,
not about the map. If the measurements say the endgame cannot be reached, the
measurements are incomplete — which is why this arm spent its second half
looking for a mechanism rather than for a better trajectory.

## 2. THE TRIGGER, MAPPED — and it is not what was banked

The previous arm's model was a plane at `z = 494.5 ± 0.6`, `x 1344…1504`,
`y 130…194`, and could not explain why crossings at 152…158 did not fire. Both
halves of that are now resolved, with two instruments.

### 2a. A floor at a chosen height (`uwlab platladder`)

Nothing had ever crossed the trigger at a height anyone chose: every crossing
was wherever a falling car happened to be. So: move a spare **roof** slab
(block 5066, a `CanopyCenterFlatBase` at (1488, 250, 464)) into the gate cell at
cell-height `cy`, move the spawn one cell above it, and drive the car across the
trigger at that floor's height. 12 rungs × 2 headings × 49 tapes = 1 176 runs.

| floor y | fired |
|---|---|
| 114 | 0 / 98 |
| 122 | 0 / 98 |
| **130** | **0 / 98** |
| 138 | 13 / 98 |
| 146 | 24 / 98 |
| 154 | 7 / 98 |
| **162** | **0 / 98** |
| 170 | 16 / 98 |
| 178 | 24 / 98 |
| 186 | 11 / 98 |
| **194** | **0 / 98** |
| 202 | 12 / 98 |

The dead rungs are exactly the three block bases. **Each gate's trigger starts
about a metre above its own block's floor and runs to the top of the block**, so
the live set is ≈ `[131, 162] ∪ [163.5, 194] ∪ [195.5, 226]` — nearly
continuous, and 29 m lower than the banked "163…169" but 1 m higher than the
banked 130.

The same experiment run through the oracle instead of a trace, with 120 blind
tapes per rung and four spawn headings, is the cleaner statement:

```
platform floor 122.16   0 / 480 finishes
platform floor 130.16   0 / 480 finishes
platform floor 138.16  70 / 480 finishes      <- positive control, same batch
```

**A car free to drive anywhere on a floor at 130.16 inside a gate cell never
fires.** That is the wall, measured rather than inferred: **the finish's live
floor is between 130.06 and 131.27**, and the deck is 114.16.

### 2b. Why crossings inside the window did not fire (x, not y)

481 scored crossings of the plane, from canopy launches and platform runs.
Sorted by where they crossed:

* every non-firing crossing at a live height is at **x = 1375.96…1376.09,
  1407.77…1408.09, 1439.29…1440.15, 1471.77…1472.09** — the **cell seams**;
* the 91 firings span **x 1349.75 … 1494.86**, i.e. the cell interiors;
* the closest non-firing interior crossing is x = 1445.91 (10.1 m from the gate
  centre at 1456), so the half-width is **between 7 and 12 m**.

A car that drives straight off a cell-corner spawn crosses at exactly the seam,
which is dead. **Four of the previous arm's nulls and both of my first two
trigger sweeps are that one mistake**; the fix is a turning tape family that
fans the crossing across x.

Firings also happen at **z = 488.4** (a car driving on a platform at 170) and at
495.9, so the trigger is a box roughly `z ∈ [486, 503]`, not the plane the 13
banked firings suggested — they were all falling cars, which enter at the near
face.

## 3. A MECHANISM THAT GAINS HEIGHT — 20.6 m, reproducible

2 000 blind tapes from 20 deck spawns under the gates, traced and scored by
`maxy`. 1 999 of them peak at 118.8, which is the spawn's own fall. One does
this:

```
 t 12.34   (1531.88, 113.460, 446.88)   creeping east at 1.4 m/s, half a metre BELOW the deck
 t 13.57   (1532.75, 116.029, 446.98)
 t 14.61   (1532.56, 122.267, 447.74)   rising at 12 m/s
 t 15.41   (1533.81, 131.032, 450.95)   8.2 m/s
 t 16.05   (1533.34, 133.957, 453.24)   4.3 m/s
 t 17.05   (1532.36, 134.783, 456.12)   apex — 20.6 m above the deck
```

The car has driven off the deck's south edge onto the not-solid
`CanopyCenterFlatHFC` ring (whose frame sits ≈ 0.76 m below the deck), crept
east along it, and at the corner where the ring, the deck's edge and the east
wall's `StructurePillar` column all meet, it is **extruded upward at ~12–15 m/s**
and coasts to 134.78. `fk trace` signs the trajectory (`self-check ok`, mean
speed 3.9 m/s, |d(pos)/dt − v| median 0.072 m/s) and it reproduces from the
written tape.

Hill-climbing the tape (`uwlab climb`, mutate a tick window, keep what climbs)
took it to **135.18**.

**This is the first thing measured on this map that beats the 16.8 m wall.**
Underwater gravity is what makes it worth so much: 2.2 m/s² and a 2.68 m/s
terminal sink mean a 12 m/s ejection is worth 15 m of rise, where in air it
would be 2.9 m.

### Where it is, and why that is not yet a finish

The apex is at **(1532, 134.8, 456)**. The nearest live trigger corner is
(1500, 131, 486) — **32 m west and 30 m north**, and at the apex the car has
2–3 m/s. So the ejection has to happen somewhere else.

The junction that produces it is *ring + deck edge + pillar column*, and the
census says where else that exists: the **whole west edge** (pillar cells
(41,14…18) against the ring at cell 40), the **whole east edge** ((47,14…18)
against cell 48), and the **whole north edge** ((42…46,18) against cz 19). The
gate cells' own pillars are in the middle of the deck, with no ring beside them.

The best of those is the **east edge at cz = 15**: x ≈ 1532, z 480…512 — the
trigger's own z band, 32 m from its live x edge. An ejection there that keeps
westward speed would cross the gate at 131+. That search is running.

## 4. WHAT IS CLOSED, WITH ITS CONTROL

**The massif hop is dead on a speed measurement, not a search result.** The
landscaped massif west of the stadium tops out at 162.16, the stadium's west
rim is 22–29 m away, and the two are within a metre in height — but the massif
is grass, and grass gives the car about a quarter of the road's thrust: 1.4 m/s²
against 14.3, so its equilibrium speed is **8.5 m/s** and its glide reach is
**15 m**. 1 216 directed launches (64 spawns × 19 turn-tapes, all four spawn
headings): the highest any of them crosses x = 1310 is **151.7**, 10 m below the
rim. The massif is also an island — its walls are `DecoWallCurve2Pillar` from
y = 10 to 146 — and nothing on the map at ≥ 162 is within a glide of it.

**Water is not a medium.** A car dropped in open water, 8 input families
(nothing, throttle, brake, both, full left, full right, pulsed, late throttle),
2 columns, 47 s each: horizontal displacement **≤ 0.28 m**, and the only effect
of full steer is a 2.6 % slower sink from tumbling. There is no swimming.

**No in-flight input beats holding the throttle.** Off the deck's north edge at
the water cap: throttle held **55.9 m**, flutter 53.9, coast/brake 34.3. The
glide ratio is not an attitude problem.

**The deck cannot reach the gate.** 17 068 blind oracle runs from 68 spawns on
the deck around the gate row (all four headings, 251 tapes each): **0 finishes**.
Positive control in the same instrument on the same day, from a spawn inside the
slot: **13/251**. Add the 12 500-run spawn ladder (cells 42…46 × cy 22…31 × cz
15): nothing below cy 30 finishes.

**The census cannot see the terrain.** The map's biggest hill reads as
`Grass@10` in a block census because it is terraformed ground, which carries no
block row at all. A measured plumb lattice over **all 2 352 columns of the map**
(`uwlab sweep` with a no-input tape, one spawn-moved map per 32 m cell) is
banked as `lattice2.tsv`; it finds a second 162 m massif at x 1280…1344,
z 864…928 that nothing had recorded, and confirms that the corridor between the
course and the stadium is empty above y ≈ 70.

**And two facts about the map itself that the write-ups had wrong.** The map's
own waypoint list says the spawn is block **#84 `PlatformTechStart` at
(1136, 18, 720)** — not (752, 90, 400), which is one of **four**
`PlatformTechCheckpoint*` blocks; and **all fifteen `GateFinish` blocks are
tagged `Goal`**, including the cy = 32 row. That row is not "the start wall":
it is a third finish row, live from ≈ 195.5 to 226. (Moving #4633 moves the
spawn only on the CP-neutralised rig, where the real start has been neutralised
too.)

## 5. WHAT IS STILL OPEN

1. **Port the ejection to the east or west edge at cz = 15.** The mechanism
   exists and clears the height; it needs to happen within 32 m of the gate,
   with horizontal speed. Running.
2. **The ejection's ceiling.** 135.18 came from a 12–15 m/s extrusion. If the
   entry can be tuned to 25 m/s the rise is 22 m; at 40 m/s it is 33 m, which
   from the deck would reach the stands at 162 and open the whole stadium.
3. **The 2672.290 s route.** Whatever the author did, it is not on this page.
   44 minutes is a lot of respawns, but it is also a lot of time for a slow
   mechanism.

## 6. HOW TO REPRODUCE

```bash
cd tools && cargo build --release          # uwlab gained sweep/skyline/chain/lattice/blitz/platladder/climb
export TM_SERVER=/path/to/TrackmaniaServer-dir
export FK_SHIM=tools/search/target/release/libforkshim.so
M=map_end_cps_neutralised.Map.Gbx

# the trigger's live band, with the floor you choose
uwlab platladder --map $M --carrier tape_a-87_finishes_at_y134.Ghost.Gbx \
                 --template tpl.gtape --cx 44 --cy 22:33 --jobs 48 --dir plat

# the same question through the oracle, with blind tapes on the platform
uwlab blitz --map $M --carrier … --template … --extra 5066:44,24,15 \
            --spawns 44,25,15,0:3 --tapes 120 --seed 31337

# a directed launch sweep: spawn WITH A HEADING x tape, traced and scored per axis
uwlab sweep --map $M --carrier … --template … --spawns 37:40,29,15:18,0:3 \
            --tape E=160:4000:1:1:0 --box 1344,130,481:1504,194,511 --cross x:1310

# the measured surface of every 32 m column of the map
uwlab sweep --spawns 0:48,32,0:47,0 --tape P=0:4000:1:0:0 --jobs 64
uwlab lattice lattice2.tsv --col 4 --ymin 0

# the height hill-climb
uwlab climb --map deckmax/maps/s46_23_15_d0.Map.Gbx --carrier … \
            --tape se_best_135.gtape --iters 400 --pop 24 --window 1450,400,1560,620
```

Two traps this arm paid for:

* **`fk trace` exits non-zero when its self-check will not CERTIFY a
  trajectory** (usually "the car never moves"), but it writes the CSV anyway.
  Reading it back is how the previous arm's "62 of 158 traces died in the car
  locator" becomes 158 scored runs — the status column says which the engine
  would not sign. Re-run a signed-looking outlier standalone before believing
  it.
* **Every concurrent engine run needs its own `--work`.** Two hill-climbers
  sharing `/tmp/uwclimb-<i>` silently ran each other's tapes; the tell was a
  saved best that re-scored 0.5 m lower.
