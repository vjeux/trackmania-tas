# 173691 "Spring 2023-15 (Underwater)" — the endgame is SOLVED: a car on the lower canopy finishes the map

Arm **B2**, 2026-08-22, node `3089.od.fbinfra.net`, branch `uw173691-finish-b2`.
Rust only. Every number is read off the live engine re-simulating a written
`.Ghost.Gbx`, and the headline is a **plain-oracle** verdict on the file as
written to disk.

## HEADLINE — THE FINISH FIRES, FROM THE DECK THE JUMP ALREADY REACHES

```
s46_23_15_d0.Map.Gbx   FINISH_from_deck.Ghost.Gbx   37.899   cps=1
```

A car that starts **on the lower canopy at y = 114.16** — the deck the published
landing puts our car on, 16.8 m below the lowest thing that had ever fired —
**reaches the finish gate and stops the clock at 37.899.**

**The 15.7 m wall this map has been closed by since 2026-08-20 is broken.**

### The control, in the same batch

| map | verdict |
|---|---|
| the rig, gates standing | **37.899, cps=1** |
| the same rig with all fourteen remaining `GateFinish` blocks moved 1.4 km away | **DNF, cps=0** |

So the clock is stopped by the map's own gate wall, not by the relocated block
that carries the rig's spawn. (That block sits at (1488, 122, 496) throughout
and fires nothing — which independently reproduces the earlier arm's "relocated
gates do not fire here".)

### What the car does

```
 t  1.0   (1472.1, 118.8, 480.0)   spawn, falling
 t  3.0   (1472.3, 114.1, 480.3)   on the lower canopy
 t  8.0   (1515.4, 114.16, 451.2)  26.6 m/s, heading for the deck's south-east corner
 t 11.0   (1532.2, 113.89, 446.8)  off the deck's south lip, 3.7 m/s — the wedge
 t 12–20  (1532…1533, ——, 443…448) CLIMBS 113.9 -> 194.07, peak rate 14.4 m/s
 t 21.0   (1531.8, 193.98, 448.7)  at rest ON THE STANDS
 t 22–29  (1517.0, 179.1, 453.9)   down the tiers
 t 30–36  (1503.5, 173.2, 488.5)   west along the stands, onto the canopy's edge
 t 37.9   (1495.4, 167.97, 494.76) FIRES gate #4649 (1488, 162, 496)
```

**The mechanism is an 80 m sustained wall climb** at the stadium's south-east
corner, x ≈ 1532, where the not-solid `CanopyCenterFlatHFC` ring (whose frame
sits ≈ 0.76 m below the deck), the deck's own south edge, and the east wall's
`StructurePillar` column all meet. The car drives off the lip at ~4 m/s, wedges,
and is driven up the corner at up to 14.4 m/s for eight seconds. Underwater
gravity is what makes it pay: at 2.2 m/s² and a 2.68 m/s terminal sink, the
coast at the top is worth another 20 m.

It was found by **one blind run in 2 000** (peak 121.25) and grown by a
hill-climb on height: 121 → 124.8 → 134.8 → 142.4 → 166.4 → 172.2 → 177.1 →
185.2 → 194.1, then a switch to a lexicographic goal score to steer the landed
car into the gate.

### What this is, and what it is NOT

* **It is**: the endgame of 173691, solved and oracle-verified, from a spawn on
  the lower canopy.
* **It is not a beaten map.** The rig is `map_end_cps_neutralised` with the
  spawn moved to cell (46,23,15); the checkpoints are neutralised and the car
  does not start at the map's own start. The real run must begin at block #84
  `PlatformTechStart` (1136, 18, 720), take the checkpoints, drive the course,
  and land the published jump.
* **The gap to a real beat is a graft, not a physics problem.** The published
  landing comes to rest at (1314.5, 114.0, 448.2); the wedge is at
  (1532, 113.9, 447) — **216 m of flat driving east along the same deck**, then
  this tape's last 28 seconds. Searches from a west-end spawn are running.

Artefacts (`~/persistent/private-30d/tm-unbeaten/173691/finish_b2/`):
`FINISH_from_deck.Ghost.Gbx` (md5 `af69ca3c5348274589bff6462ffbe10f`),
its `.gtape`, the rig map `map_FINISH_spawn_46_23_15.Map.Gbx`
(md5 `88575cea411e91983caa2c5a786a9eb7`), and `FINISH.csv`, the traced
trajectory.

---

## 1. THE MAP IS COMPLETABLE — and now we know one way

```
<desc ... validated="1" .../>
<times bronze="4009000" silver="3207000" gold="2833000" authortime="2672290"/>
```

173691's own header. `validated="1"` is written by the editor's validation run
and the three medals are the auto-derived ×1.06 / ×1.20 / ×1.50 of the author
time. **Somebody finished this map in 2672.290** — 44m32s. That fact is what
kept this arm looking for a mechanism after the geometry said no.

## 2. THE FINISH TRIGGER, MAPPED

The banked model was a plane at `z = 494.5 ± 0.6`, `y 130…194`, and it could not
explain why crossings at 152…158 do not fire. Two instruments settle it.

### A floor at a height you choose (`uwlab platladder`)

Move a spare **roof** slab (block 5066, `CanopyCenterFlatBase` at
(1488, 250, 464)) into the gate cell at cell-height `cy`, move the spawn one
cell above it, drive the car across the trigger at that floor's height.

| floor y | fired (trace, 98 runs) | fired (oracle, 480 blind tapes) |
|---|---|---|
| 114 | 0 / 98 | — |
| 122 | 0 / 98 | 0 / 480 |
| **130** | **0 / 98** | **0 / 480** |
| 138 | 13 / 98 | **70 / 480** |
| 146 | 24 / 98 | — |
| 154 | 7 / 98 | — |
| **162** | **0 / 98** | — |
| 170 | 16 / 98 | — |
| 178 | 24 / 98 | — |
| 186 | 11 / 98 | — |
| **194** | **0 / 98** | — |
| 202 | 12 / 98 | — |

The dead rungs are exactly the three block bases. **Each gate's trigger starts
about a metre above its own block's floor and runs to the top of the block**:
live ≈ `[131, 162] ∪ [163.5, 194] ∪ [195.5, 226]`. The lowest firing ever
recorded is **131.27**; a car free to drive anywhere on a floor at **130.16**
never fires in 480 blind tapes. **The wall from the deck was 16.0 – 17.1 m.**

### x, and the seams (481 scored crossings)

* Every non-firing crossing at a live height is at **x = 1376, 1408, 1440 or
  1472 ± 0.2** — the gate **cell seams**.
* The 91 firings span **x 1349.75 … 1494.86**, the cell interiors; the closest
  non-firing interior crossing is 10.1 m from a gate centre, so the half-width
  is **7 – 12 m** around 1360 / 1392 / 1424 / 1456 / 1488.
* Firings happen at **z = 488.4** as well as 495.9, so the trigger is a box
  ≈ `z ∈ [486, 503]`, not a plane; the thirteen banked firings were all falling
  cars, which enter at the near face.

A car driving straight off a cell-corner spawn crosses at exactly the seam.
**That single mistake accounts for four of the previous arm's nulls and both of
my first two trigger sweeps.**

## 3. WHAT IS CLOSED, EACH WITH ITS CONTROL

**The massif hop is dead on a speed measurement.** The landscaped massif west of
the stadium tops out at 162.16 and the stadium's west rim is 22–29 m away at
the same height — but the massif is grass, and grass gives about a quarter of
the road's thrust (1.4 m/s² against 14.3), so its equilibrium speed is
**8.5 m/s** and its glide reach **15 m**. 1 216 directed launches (64 spawns ×
19 turn tapes, all four spawn headings): the highest crossing of x = 1310 is
**151.7**. The massif is also an island — `DecoWallCurve2Pillar` walls from
y = 10 to 146 — and nothing on the map at ≥ 162 is within a glide of it.

**Water is not a medium.** A car in open water, 8 input families (nothing,
throttle, brake, both, full left, full right, pulsed, late throttle), two
columns, 47 s each: horizontal displacement **≤ 0.28 m**. Full steer buys a
2.6 % slower sink from tumbling. There is no swimming.

**No in-flight input beats holding the throttle.** Off the deck's north edge at
the water cap: throttle 55.9 m, flutter 53.9, coast/brake 34.3.

**Blind search from the deck does not find the gate.** 17 068 blind oracle runs
from 68 deck spawns around the gate row: **0 finishes**; positive control in the
same instrument, from a spawn inside the slot, **13/251**. Plus a 12 500-run
spawn ladder (cells 42…46 × cy 22…31 × cz 15): nothing below cy 30 finishes.
**The wall did not fall to volume — it fell to a continuous objective**, which
is the transferable lesson: 2 000 runs scored by *height* found the one seed
that 29 568 runs scored by *finished / did not* could not see.

**The census cannot see the terrain.** The map's biggest hill reads as
`Grass@10` in a block census because it is terraformed ground, which carries no
block row. A measured plumb lattice over **all 2 352 columns of the map**
(`lattice2.tsv`) finds a second 162 m massif at x 1280…1344, z 864…928 that
nothing had recorded, and confirms the corridor between the course and the
stadium is empty above y ≈ 70.

**Two corrections to the map's own description.** The spawn is block **#84
`PlatformTechStart` at (1136, 18, 720)**; (752, 90, 400) is one of **four**
`PlatformTechCheckpoint*` blocks. And **all fifteen `GateFinish` blocks are
tagged `Goal`**, the cy = 32 row included — it is a third finish row (live
≈ 195.5 … 226), not "the start wall". Moving #4633 moves the spawn only on the
CP-neutralised rig, where the real start is neutralised too.

## 4. WHAT IS STILL OPEN

1. **The graft.** Drive the 216 m from the published landing point
   (1314.5, 114.0, 448.2) to the wedge at (1532, 113.9, 447) and re-solve the
   last 28 s from there. The whole run fits inside the 47.86 s the rig
   simulates, and the map's budget is 2672.290.
2. **The checkpoints.** The finish on the real map needs all of them; CP c
   (#2180 at (976, 186, 1072)) has still never fired.
3. **The wedge's ceiling.** 194.07 was where the search stopped, not where the
   mechanism does; the last improvement was still +9 m.

## 5. HOW TO REPRODUCE

```bash
cd tools && cargo build --release
export TM_SERVER=/path/to/TrackmaniaServer-dir
export FK_SHIM=tools/search/target/release/libforkshim.so

# the finish, and its control
tmmaps oracle --map map_FINISH_spawn_46_23_15.Map.Gbx --ghosts FINISH_from_deck.Ghost.Gbx -j 1
tmmaps move map_FINISH_spawn_46_23_15.Map.Gbx --out ctl_nogates.Map.Gbx \
   --move 4649:10,20,10 --move 4650:10,21,10 … --move 5103:16,21,10
tmmaps oracle --map ctl_nogates.Map.Gbx --ghosts FINISH_from_deck.Ghost.Gbx -j 1   # DNF

# the trigger's live band, with the floor you choose
uwlab platladder --map $M --carrier … --template … --cx 44 --cy 22:33 --jobs 48
uwlab blitz --map $M --carrier … --template … --extra 5066:44,24,15 \
            --spawns 44,25,15,0:3 --tapes 120 --seed 31337

# the height hill-climb, and the lexicographic goal score that lands it
uwlab climb --map … --tape r92.gtape --iters 400 --pop 24 --window 1450,400,1560,620
uwlab climb --map … --tape b_t803.gtape --goalbox 1348,131,486:1500,226,503 \
            --mut-range 2200:4000
```

New `uwlab` subcommands from this arm: `sweep` (spawn-with-a-heading × tape,
traced and scored per axis, with `--random`, `--extra`, `--cross`), `skyline`
and `lattice` (the census view and the measured one), `chain` (reachability
under the fitted water law), `blitz` (oracle-scored blind search), `platladder`
(a floor at a chosen height inside the gate), `climb` (hill-climb on height,
with a lexicographic goal box), `shift` (move a solved manoeuvre later in a
longer tape).

### Three traps this arm paid for

* **`fk trace` exits non-zero when its self-check will not CERTIFY a
  trajectory** — usually "the car never moves" — but it writes the CSV anyway.
  Reading it back turns the previous arm's "62 of 158 traces died in the car
  locator" into 158 scored runs. The seed for everything above first arrived
  marked `unsigned`; re-run an outlier standalone before believing *or*
  discarding it.
* **Every concurrent engine run needs its own `--work`.** Two hill-climbers
  sharing `/tmp/uwclimb-<i>` silently ran each other's tapes; the tell was a
  saved best that re-scored 0.5 m lower.
* **A miss distance that is not resolved per axis chooses the wrong parent.**
  A car on the deck is 17 m from the gate box in *y*; a car on the stands is
  31 m from it in *xz*. Scored on plain distance, the search gives the height
  back. The goal score is lexicographic and non-overlapping for exactly that
  reason: height first, then horizontal distance among samples that already
  have the height, then inside, then finished.
