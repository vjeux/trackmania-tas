# 287431 "blame yumi for blaming me" — THE FINISH NOW GOES THROUGH THE RING

**2026-08-29, box 129490.** Task: vjeux — *"find a finish that goes inside of
the ring, going outside is way too finicky."*

**Done, and it is FASTER, not slower.**

| | time | crossing offset from the ring's centre | ring can be moved by |
|---|---|---|---|
| old, outside | 20.852 | **27.3 m** — outside the 22.7 m opening | **±1 m** |
| new, through | **20.767** | **12.3 m** -- inside, >10 m of clearance | **y −30…+8, z −18…+14** |

Author time 21.445, human WR 24.092 (ITZYNO1FAN). **20.767 is 0.678 under the
AT** and is also faster than the best OUTSIDE line this box ever produced
(20.792, arm z2, 1.39 M evals) — so going inside costs nothing.

`tapes/ring20767.Ghost.Gbx` md5 `294fe9e6180015f43cc90278c4b7cd8c`,
plain oracle on the stock map from its own written bytes: **20.767**.

---

## 1. The ring: where it is and which way it faces

The finish is **free block 1591, `Bigfin.Block.Gbx_CustomBlock`, tagged Goal**,
at position `(1091.470, 209.000, 884.510)` with rotation
`(yaw π/2, pitch π, roll 0)`. It is the only Goal within 250 m; the census's
other Goal families (grid Bigfins and the `GateExpandableFinish` curtain) sit at
x 1392–1456, 787 m away.

**The rotation is the whole story and it is easy to get wrong.** The model is a
ring standing in its own local XY, 9 m deep in local z. `mapgeom model` alone
therefore says "a ring facing north-south", which is FALSE. Applying the stored
rotation the way `mapgeom::place::turned` does gives
`world = (px − lz, py − ly, pz − lx)`, i.e.

* the ring stands in the **world Y–Z plane and faces X** — you drive through it
  going **east**, which is what our lap already does;
* collidable mesh: **x 1105.3 … 1114.3**, y 177.3 … 232.5, z 852.8 … 908.0;
* the **opening** is a circle of clear radius ≈ **22.7 m** centred at
  **(y 205.0, z 880.5)**;
* the `RaceTriggerFXFinish` disc (material 61, `NotCollidable`) is a filled
  circle of the same radius, concentric with the opening, 8.2 m deep in x
  (**x 1105.7 … 1113.7**).

Tool: `tools/ringscan.rs` (std-only, `rustc -O`). It rasterises a `mapgeom`
`.obj` onto a plane and reports the largest empty box — that is how the opening
was located. `--only-mat NotCollidable` isolates the trigger disc.
`tmmaps census` now prints `rx ry rz` (patch below); it did not before, and
without the rotation every coordinate in this section is wrong by 90°.

## 2. THE POSITIVE CONTROL — and it is the map, not the model

The bank's own warning ("`mapgeom`'s offsets are PER-STRUCTURE; query a surface
the car demonstrably touches before trusting a null") is answered here by an
engine-truth control, not by a mesh argument:

**Move the ring by a known amount with `tmmaps move` and re-validate the tape
on the plain oracle.** The mover writes position only — no cell bytes, no model
swap, no `tmmaps gate` — and the origin control (rewrite 1591 at its own
position) reproduces 20.852 exactly, 2 body bytes changed.

Every prediction the geometry made was then confirmed:

| displacement of block 1591 | predicted for the 20.852 | oracle |
|---|---|---|
| +4, +9, +14 in y | miss the trigger | DNF cp6 ✓ |
| +14 in z | miss the trigger | DNF cp6 ✓ |
| −14 in y, −14 in z | still inside | 20.851 / 20.945 ✓ |
| **−2 in y** | the car is in a 4 m GAP in the rim; move the rim and it hits | **DNF** ✓ |
| **−4 in y** | grazes the rim | **21.528** — 0.68 s of collision ✓ |

That last pair is the finding in one line: **the 20.852 does not go round the
ring, it threads a gap in the ring's rim, and the gap is about 2 m wide.**

## 3. `--must`: a hard constraint made of engine truth

New `tmsearch` flag (patch `tools/must_constraint.patch`, 434 lines, against the
tree at the top of this box's `/tmp/tm/repo`):

```
--must MAP          a VARIANT map every finisher must ALSO finish on (repeatable)
--must-window S     how far a variant finish may sit from the real one [0.150]
```

A variant is the real map with **one object moved by a known amount**, so
"this tape still finishes there" is a statement about **where the car was**,
measured by the same oracle that decides the result. It is not a shaped reward
and there is nothing in it for a search to game.

Design points worth carrying to the next map:

* **Only finishers are checked, and the variants short-circuit in a fixed
  order.** A stock-map DNF costs one oracle run; cost is ~1× while the search is
  off-route and ~5× once it is compliant. Throughput measured: 38–70 eval/s per
  20-worker arm against ~190 unconstrained.
* **The ladder is `MUST_RUNG + (variants passed)`**, far above any real
  checkpoint count, with the stock map's millisecond as `seg_ms` so runs on one
  rung are still ordered by lap time. A run that passes them all becomes a true
  `Finish`. This gives a genuine staircase: DNF cp0…cp6 → cp50…cp55 → Finish.
* **The SEED must be demoted too.** A compliant-looking seed entering as
  `Finish` freezes the search: nothing can displace it but a candidate that is
  both compliant and faster. `check_must_maps` puts the seed on the rung it has
  earned, and prints each variant's answer as a startup control.
* **The guard had to learn about it.** It re-validates a banked claim on the
  real map, saw `Dnf(cp54)` claimed against `Finish(21.041)` measured, and
  called every demoted finisher a PHANTOM — halting two arms. A demoted
  finisher is checked like a finisher: the millisecond must reproduce.

### ⚠ THE TRAP, AND IT COST A WRONG ANSWER BEFORE `--must-window` EXISTED

**A variant-map finish proves nothing unless it happens at the same INSTANT.**
A tape can miss the moved trigger completely, carry on, and be caught by it —
or by another Goal — a second later. The oracle says "finished" and the
constraint has measured nothing.

`y2_best_20_804` did exactly this: it passed all six variants and was reported
by the tool as *"the seed already satisfies every variant map"* — while
finishing them at **22.085**, 1.281 s late. It is not through the ring at all.
`--must-window 0.12` kills it. A compliant tape returns the SAME millisecond on
every variant, because it enters the trigger cylinder at the same x in all of
them; that identity is the signature to look for.

### The constraint set used here

Six maps, block 1591 translated, `--must-window 0.12`:

```
m0_yp4   1091.470, 213.000, 884.510     m3_zp14  1091.470, 209.000, 898.510
m1_yp9   1091.470, 218.000, 884.510     m4_ym14  1091.470, 195.000, 884.510
m2_yp14  1091.470, 223.000, 884.510     m5_zm14  1091.470, 209.000, 870.510
```

m0/m1 are a **climbing staircase** for an arm starting from the non-compliant
fast line; m2–m5 are the four axis constraints that do the bounding. With
trigger radius R ≈ 27.5 measured off the flip point, passing all four bounds the
crossing to `(|Δy|+14)² + Δz² ≤ R²` and its mirror, i.e. **|Δ| ≤ 15.7 m inside a
45 m aperture**. Once a seed is compliant, drop m0/m1 — they cost two launches
per batch and buy nothing.

## 4. What it bought — the robustness numbers

**(a) How far the ring can be translated before the lap stops finishing at the
same millisecond.** Free block 1591 moved, plain oracle, `traces/`:

| | +y | −y | +z | −z |
|---|---|---|---|---|
| **20.852 outside** | dies at **+2** | dies at **−2** (collision at −4) | dies at **+6** | dies at **−6** |
| **20.767 through** | **+8** (still finishes to +14) | **−30** | **+14** | **−18** |

**(b) Identical wide fuzz from a fixed incumbent** — `tmsearch dump`, 1200
candidates, same `--ops wide --nops-upto 14 --lo 1700 --hi 2768 --window 100
--stride 13 --seed 4242`, nothing accepted, so the sample is unbiased:

| seed | finishes the stock map | AND still through the ring |
|---|---|---|
| 20.852 outside | 63.8 % | n/a — it is not through the ring |
| 20.831 through | **78.6 %** | **63.2 %** |

**(c) Live search finish rate under the full constraint** (a "finish" here means
a lap that finishes the stock map AND all four axis variants at the same
millisecond): sA 54 %, sF 80 %, sE 33 %, sB 25 %.

**(d) Where the two laps cross the ring plane**, from regenerated telemetry
(`ghost declare --from-oracle` → `ghost regen` → `tmtraj export`), at
x = 1105.71, the trigger's near face — independent of the constraint machinery:

| | y | z | offset from centre (205.0, 880.5) |
|---|---|---|---|
| 20.852 | 181.6 | 866.5 | **27.3 m — outside the 22.7 m opening** |
| 20.831 | 191.4 | 880.8 | **13.6 m**, ~9 m of clearance to the rim |
| **20.767** | **193.2** | **877.0** | **12.3 m** -- and 11.0 m at the exit face, so >10 m of clearance to the rim all the way through |

The trajectory measurement and the constraint's own bound (<= 15.7 m) agree --
two independent instruments on the same quantity. A later arm reached **20.763**
(md5 `68d8462bca925918e6096bb409c7daff`), also compliant on all six.

## 5. How it was found, and how fast

Round 1 launched at 16:03Z. **The first fully compliant lap appeared in ~2400
evaluations, inside 90 seconds** (rD, 22.865), and by 16:04 three arms had
compliant laps at 20.831 / 20.945 / 20.982 — the 20.831 already faster than the
20.852 it replaced. Round 2, re-seeded on the compliant laps, reached 20.821 in
20 minutes and **20.767** in 35.

The route provenance is intact: these are edits to the ITZYNO1FAN-derived
lineage (`--template BEST20852.Ghost.Gbx`), in the last ~1.5 s only. Nothing
upstream of tick 1200 was touched by the arm that found 20.767 (`--lo 1700`).

**Method note.** This is the third time on this project that a HARD constraint
moved a route where a soft objective did not (cf. *"a rung is an objective, not
a seed"*). The difference is that `--must` cannot be satisfied by scoring well —
it is the oracle's own yes/no on a map whose geometry has moved, so the only way
to pass is to actually be somewhere else.

## 6. Contents

```
tapes/    BEST20852.Ghost.Gbx      the old outside line, 20.852
          ring20831.Ghost.Gbx      first through-the-ring lap, 20.831
          ring20821.Ghost.Gbx      20.821
          ring20767.Ghost.Gbx      BEST, 20.767   md5 294fe9e6180015f43cc90278c4b7cd8c
          r20831.Ghost.Gbx         regen of the 20.831 (real telemetry, for film/trace)
traces/   r20831.csv b20852.csv    decoded trajectories
          dump_outside.jsonl       1200 unbiased perturbations of the 20.852
          dump_ring.jsonl          the same 1200 of the 20.831
          dump_ringc.jsonl         the same 1200, scored under the constraint
tools/    must_constraint.patch    --must / --must-window + the guard fix + census rx/ry/rz
          ringscan.rs              find a ring's opening from a mapgeom .obj
          launch_ring.sh           round 1 (climbing ladder from the outside line)
          launch_ring2.sh          round 2 (refine from a compliant seed)
```

Every hash in this file was taken by copying the file OFF the persistent mount
into `/tmp` and hashing it there.

## 7. Open, and what I would do next

* **The descent is not finished.** sA was still improving (20.764) when this was
  written; the constraint costs ~5× per eval once compliant, so there is a lot
  of unspent gradient.
* **The constraint could be tightened** to d = 18–20 to force a crossing within
  ~8 m of dead centre, now that a compliant seed exists to start from.
* **`fk trace` cannot locate the car on this map** — 542 s to fail its own
  self-check (mean speed 3.1 m/s, non-unit quaternion). `ghost regen` +
  `tmtraj export` is the working path for a trajectory here.
