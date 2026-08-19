# 153527 — **YES. The map validates.** First `ValidatedResult` ever obtained from `P-Found - Pokeuuu`

Written 2026-08-19 by the 153527 validation agent (node 34881), answering the one
question put to it: *can this map produce a `ValidatedResult` at all?*
Write-once sidecar, prefix `val_`. Nothing here modifies `RESULT.md`,
`route_RESULT_v1.md` or `route_SEED_v1.md`. Raw server output for every run
quoted below is in `val_evidence_v1/`.

---

## The answer

**Yes.** Six independent relocated-finish placements returned a real, non-null
`ValidatedResult` block from map 153527, driven by **the only tape that exists**
— `rank00001_5661335.Ghost.Gbx`, the dead-build ghost with
`NbRespawns = 4294967295`:

| Goal moved to cell | recorded car first enters at | `ValidatedResult.Time` |
|---|---|---|
| (23,18,17) | 1.980 | **8.729** |
| (23,18,18) | 1.530 | **531.122** |
| (23,18,19) | 5.480 | **1049.307** |
| (22,17,19) | 6.130 | **1064.969** |
| (22,18,19) | 5.980 | **1242.529** |
| (21,17,19) | 6.680 | **2716.208** |

Raw, from `val_evidence_v1/val_Y2_153527_goal_23_18_17.txt`:

```
  "ValidatedResult" : {
    "NbCheckpoints" : 12,
    "NbRespawns" : 0,
    "Time" : 8729,
    "Score" : 0
  },
  "Desc" : "validated time is actually better! (5661335 > 8729)\nhad simulation hazards '0-1-0'\n",
  "IsValid" : false,
  "DeclaredResult" : { "NbCheckpoints" : 12, "NbRespawns" : 4294967295, "Time" : 5661335, ... }
  "MapUid" : "4ympwQ3XZfX8balg2UcVJBL_pnf",
```

**These are not echoes of the declared time** — the harness trap in the brief.
The declared time is 5 661.335 in every one of them and the validated times are
8.729 … 2 716.208. A raw `ValidatedResult` block carrying a number the tape does
not contain cannot be an echo, and that is exactly why the table above is read
from the raw blocks and not from any summary row.

So:

* the map is **not** refusing to validate; there is no map-level defect;
* the ghost's input tape **does** simulate on this map;
* `RESULT.md` §2's mechanism was already refuted by `route_SEED_v1.md`; the
  fallback worry it left behind — *"something about this ghost or this map
  defeats the simulator"* — is now **bounded**, see §4;
* **bisection is possible, and I ran the first pass of it.**

---

## 1. The experiment as specified would have produced a FALSE NEGATIVE

This is the most important thing in this file and it is not a quibble about
method — it is the difference between the answer above and a confident,
published, wrong "the ghost is broken".

The brief says: relocate the finish gates onto the spawn, re-run the tape, and
if it *still* says `wrong simu` with the finish two seconds away, the ghost is at
fault. I ran exactly that on the **positive control**, 152940 — the sibling map
whose ghost validates to the millisecond — and it says `wrong simu`:

```
152940, untouched                     ValidatedResult.Time 886277   IsValid true
152940, Goal moved to (28,18,31),
        a cell its own car crosses
        at 1.850 s                    ValidatedResult null   "wrong simu"
```
(`val_C1_152940_goal_on_path.txt`)

**A map that validates perfectly returns the 153527 signature under the
specified surgery.** The probe as briefed cannot say yes, so its "no" carries no
information at all.

The mechanism is not subtle once seen: **a finish only counts once every
checkpoint has been collected.** 165922 — where this recipe was invented and
where it worked — has two waypoints, a spawn and a goal, and no checkpoints.
153527 declares **12** and 152940 **11**. Move only the goal on such a map and
the car crosses it having collected nothing; the game ignores it, the run never
finishes, and the validator emits `wrong simu`.

To get an early finish on a checkpointed map, **every checkpoint has to be
collected first** — so they have to be relocated too. That is what §3 does.

## 2. Correction to §11 of ACQUISITION: on this map the gates are **not** free blocks

The brief (following ACQUISITION §11) says the surgery is a pure float rewrite in
chunk `0x0304305F`, because a free block's position lives there. **That is not
this map.** 153527 is grid-placed:

```
tmmaps list / valgate info m153527.Map.Gbx
      0 PlatformTechStart       Spawn        cell 24,18,18
    547 PlatformTechCheckpoint  Checkpoint   cell 17,17,21
    ... 11 checkpoints ...
  40964 PlatformTechFinish      Goal         cell  9,37,16
```

All 13 waypoints are grid blocks; the 104 free blocks in `0x0304305F` are
`CanopyCenterFlatBase` decoration (`val_free_153527.txt` reproduces
`route_SEED_v1.md`'s count). `tmex movegates` has nothing to move here.

The grid-regime equivalent is just as safe and is what `valgate` does: **the
three cell bytes in the block's own record in chunk `0x0304301F`, immediately
after its `dir` byte.** Overwritten in place. No field changes length, the
Id/lookback table is untouched, no chunk changes size, nothing is re-encoded —
the same "safest surgery available" property, in the other placement regime.
The map is parsed only to locate the bytes; the file written is the original
decompressed body with those bytes changed, recompressed.

Position-only by construction, per
`FLEET_NOTICE_origin_control_insufficient_v1.md`: `valgate` never swaps a model,
never promotes a checkpoint to a finish, never adds or removes a block. **The
block that lands on the early path is the map's own finish block, unmodified
except for where it sits**, so its trigger volume is the volume it always had.

### Origin control, both halves

*Byte-for-byte*, on the decompressed body (file-level sha256 is useless: LZO
recompression is not bit-reproducible, which is itself worth knowing):

```
153527  12 waypoints moved, then all 12 moved back   BODIES IDENTICAL (2702856 bytes)
152940  11 waypoints moved, then all 11 moved back   BODIES IDENTICAL (3140430 bytes)
152940  1 waypoint moved (the Goal)                  BODIES DIFFER at exactly 3 bytes
```

*Time-for-time*:

```
152940 round-tripped map + its ghost   ValidatedResult.Time 886277, IsValid true   (val_C4rt_...)
153527 round-tripped map + its ghost   null, "wrong simu\nhad simulation hazards '0-1-0'\n",
                                       Unvalidable 100%   -- the untouched map's result, exactly
```
(`val_ORIGIN_153527_roundtrip.txt`)

### The instrument can say yes, and it is calibrated in milliseconds

On 152940, moving **only** the Goal to three cells along the final approach —
where all ten real checkpoints have already been collected:

| Goal moved to | recorded car enters at | `ValidatedResult.Time` |
|---|---|---|
| (45,17,30) | 885.650 | 885.684 |
| (44,17,30) | 884.950 | 884.999 |
| (43,17,30) | 884.150 | 884.199 |
| (41,17,30) | 882.900 | null — did not fire |

Three of four agree with the car's own telemetry to **within one 50 ms
sample**. A relocated finish gate is a faithful position-only proxy for "when
did the car cross here". (The fourth is a *placement miss*, §5 — and it is the
reason a single "no" from this probe means nothing.)

## 3. What actually worked, and why

Both maps' cars were located with the per-life merged track, not the stock
decoder — `ACQUISITION_addendum_ghost_entity_selection_v1.md` was written on this
very ghost. The waypoint referee agrees: the merged track enters cell (17,17,21)
at 11.230 s and the map's CP1 block *is* at (17,17,21), with the ghost's declared
CP1 split at 11.613 s.

The working configuration on 153527:

* **all 11 checkpoint blocks stacked into one cell — (23,18,18), which the car
  crosses at 1.530 s at 83.8 km/h.** Eleven waypoint blocks in a single cell is
  legal in the file format, the map loads (`Can't load: 0%`), and **all eleven
  triggers fire**;
* **the Goal moved to a second cell** further along the first seconds.

First shot, before the Goal was moved at all:

```
Y1: 11 CPs at (23,18,18), Goal left at (9,37,16)
    "wrong simu, but reached some checkpoints (11 out of 12)"
```

That line alone retired the map's history: every previous run on 153527 printed a
bare `wrong simu`, which per `oracle.rs` means **0 or 1** checkpoints. Then the
Goal sweep produced the six finishes in the table at the top.

Determinism: `(23,18,17)` re-run from a fresh staging root → 8729 again;
`(23,18,18)` → 531122 again (`val_REVAL_*`).

**The instrument can also say no**: six of the twelve Goal placements in the same
sweep returned `ValidatedResult: null`. It is pinned from both sides.

### What the six times are, and are not

They are the simulated car's crossing times, **not** measurements of the human's
run. Eleven stacked platform blocks are real geometry: from 1.530 s the
simulated car is being deflected by them, and 531.122 s or 2 716.208 s is where
that deflected car happened to wander back across the relocated goal. Do not read
them as anything about the human's line. The only claim they support is the one
being made: **the map, this ghost's inputs, and `/validatepath` together produce
a validated finish.**

## 4. Bisection, first pass: the tape simulates faithfully for **at least 96.180 s**

Same instrument, read through the checkpoint counter instead of the finish.
Stack the checkpoints at a cell the recorded car crosses at time T; if the
counter comes back full, the simulated car was in that cell too, so the
simulation matched the recording out to T. Calibrated on 152940, where moving K
checkpoints into one early cell reports exactly K (K = 1,2,3 print bare, 5 → "5
out of 11", 8 → "8 out of 11", 10 → "10 out of 11").

Confirmed faithful at, in seconds of race time:

```
1.530  1.980  2.730  5.480  6.680  11.230  12.080  12.830  13.930
27.730  32.430  52.180  66.080  73.930  96.180
```

**96.180 s is the deepest confirmed point.** By then the simulated car has
crossed the real CP1 (11.613), the real CP2 (69.769) and survived the tape's
**first respawn press at 78.670 s**. Everything the plain oracle's bare
`wrong simu` implied — a divergence inside the first checkpoint — is wrong: the
run is faithful for a minute and a half.

Probes at 97.680, 144.680, 145.980, 148.130 and 226.580 came back empty. **That
is not evidence of divergence** (§5). The honest statement is a lower bound:
the divergence that stops this tape validating happens **later than 96.180 s**,
and the next agent's job is to push the bound up, not to trust the first empty
rung.

### Two confounds this probe has, both found the hard way

**(a) A relocated checkpoint leaves a HOLE in the track.** CP1's block sits at
(17,17,21) and the car *drives on it* at 11.230 s. My first depth sweep moved
all 11 checkpoints to the probe cell, so every rung past 11.230 s had a hole
where CP1 used to be — and produced a beautiful, entirely fictitious "cliff":
faithful to 11.23 s, nothing after. Leaving CP1 in place and stacking only the
other ten dissolved it instantly (13.930 s and 27.730 s went from empty to full).

> **Leave in place every checkpoint whose real crossing precedes the probe
> time.** Its block is load-bearing track. Probes past 69.769 s keep CP1 *and*
> CP2; past 224.486 s keep CP3 as well.

**(b) Relocating a checkpoint ends the useful part of the run at that cell.** On
152940, moving a *single* checkpoint into the car's early path drops the run
from a clean 886.277 to "1 checkpoint reached, nothing else" — the added
platform is solid and the car does not come out of it the way the tape expects.
A relocated *finish* is harmless in exactly the same situation because crossing
it ends the run. So the probe is **one-shot**: each rung measures faithfulness up
to its own cell and tells you nothing past it.

## 5. A gate relocated onto a cell the car demonstrably drives through often does not fire

Measured, adjacent cells, 1.5 s apart on the recorded line, both with the car
passing within 2 m of the cell centre and sitting on its surface:

```
96.180  cell (26,20,12)  mindist 1.2 m  ->  11 of 12   FIRES
97.680  cell (26,20,13)  mindist 1.7 m  ->  bare wrong simu   DOES NOT FIRE
```

and on the calibrated map, the Goal at (41,17,30) missing while (43/44/45,17,30)
all fire to the millisecond.

> **This probe's "yes" is worth everything and its "no" is worth nothing.**
> Never conclude divergence, or absence, from an empty rung; move one cell and
> try again. Roughly a third of well-chosen placements here simply do not
> trigger.

A second placement rule, measured: a gate one cell **below** the driving surface
(8 m down) does **not** fire — 10 checkpoints at (29,16,30) under a path at
(29,17,30) returned a bare `wrong simu`, while the same ten *in* (29,17,30)
returned 10 of 11 (`val_T1_152940.txt`, `val_T2` in the same sweep). There is no
"hang it underneath where it cannot obstruct" trick.

## 6. What this changes for 153527

* The map is **healthy** and it is **debuggable for the first time**. Both of
  `RESULT.md` §2's successors are now settled: the build string does not explain
  it (`route_SEED_v1.md`), and neither the map nor the ghost is fundamentally
  unsimulable (this file).
* The one human record **does re-simulate faithfully for at least 96.180 s of
  its 5 661.335 s**, through two checkpoints and one respawn. Whatever breaks it
  is a localised event later in the tape, not a property of the file.
* `RESULT.md` §3 — the retry-deletion floor of 1 214.585 s against a 939.283 s
  author time — is untouched by any of this and is still the reason the map is
  not a target. **Nothing here reopens 153527 as a search target.** What it
  retires is the "unsimulable, therefore unknowable" story.
* The obvious next step, and it is now an ordinary debugging problem: push the
  faithfulness bound with the §4 probe (keeping earlier checkpoints in place,
  and never trusting a single empty rung) until the divergence is bracketed to a
  few seconds, then look at what the car is doing there. A respawn landing, an
  item, or a physics-build difference in one obstacle are the candidates.

## Files

```
val_RESULT_v1.md                       this file
val_evidence_v1/
  val_P1_152940_plain.txt              positive control, my node, my harness: 886277, IsValid true
  val_P2_153527_plain.txt              the known negative, reproduced exactly
  val_C1_152940_goal_on_path.txt       THE FALSE NEGATIVE: the briefed experiment on a healthy map
  val_C0_152940_roundtrip.txt          origin control, 1 block:  886277
  val_C4_152940_all_gates_early.txt    11 gates along the path -> 4 of 11
  val_C4rt_152940_roundtrip.txt        origin control, 11 blocks / 30 bytes: 886277
  val_C5_152940_stacked.txt            10 checkpoints in ONE cell all fire -> 10 of 11
  val_C6_152940_g_*.txt                Goal sweep behind a stack: 11 rungs, no finish
  val_C7_152940_g_*.txt                Goal-only sweep: 885684 / 884999 / 884199 / null
  val_K{1,2,3,5,8}_152940.txt          K relocated checkpoints -> exactly K, run dies there
  val_T1_152940.txt                    a gate 8 m below the path does not fire
  val_T3_152940_highclear.txt          nor does 5.7 m of clearance save the run
  val_Y1_153527_cps_at_1p53s.txt       FIRST EVER non-bare result from 153527: 11 of 12
  val_Y2_153527_goal_*.txt             THE ANSWER: 12 rungs, 6 with a ValidatedResult
  val_REVAL_153527_goal_*.txt          re-validation of two of them, identical
  val_ORIGIN_153527_roundtrip.txt      origin control on 153527: the untouched result, exactly
  val_D_153527_t*.txt                  depth probe v1 -- CONFOUNDED by the CP1 hole, kept
  val_E_153527_t*.txt  val_F_153527_t*.txt   depth probe v2/v3, CP1 (and CP2/CP3) left in place
  val_track_153527_240s.txt            the merged per-life track, first 240 s
  val_valgate_v1.rs                    the surgery tool (grid-cell, position-only, 3 bytes)
  val_valstart_v1.rs                   per-life merged track dump
  val_raw_sh_v1.txt                    the raw /validatepath runner (own staging root per run)
  val_SHA256_inputs.txt                sha256 of both maps and both ghosts
```

Tools built in a forked tree, `/tmp/tmtas-val` (never a shared one); one staging
root per validation under `/tmp/val_stage/<tag>`; every sweep asserted N rungs →
N files → N distinct sha256 before any of it was read, per
`FLEET_NOTICE_submetre_probe_sweeps_invalid_v1.md`.
