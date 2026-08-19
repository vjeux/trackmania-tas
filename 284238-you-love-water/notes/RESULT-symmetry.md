# 284238 "YOU LOVE WATER" — the map is ONE cycle repeated four times, and the
# thing that blocks the author time is measured, not guessed

Sidecar, write-once. Does not supersede `RESULT-v1.md` (still correct on
everything it measured) and does not touch `GEOMETRY_v1.md` (another agent's).
This is the **oracle-side** report: every number below went through the plain
oracle (`tmtas validate`, absolute paths, a known-answer control in the same
batch). Times in seconds.

AT **50.459** · one human record **440.238** · best validated tape on this map
is now **97.325** (this session, from 97.461 — see §6b).

---

## 0. One-paragraph summary

The map is a **screw-symmetric repetition**: one 40-block module placed four
times, each copy the previous one rotated **−120° about the vertical axis
through (x 772.286, z 821.428)** and dropped **56 m**. The four checkpoint gates
are exact images of each other under that transform (0.00 m). So the human's one
clean cycle (13.163 s, CP1→CP2) is, in module coordinates, the *same task* as
the three slow ones (24.428 / 25.788 / 23.738). That makes the obvious attack
"drive the fast cycle four times", and I built it and ran it: **105+ spliced
tapes across every phase, with and without a respawn press, and every one of
them dies in the same place — 2 to 3 seconds into the transplanted cycle.** The
reason is measured: the state the car is handed at the start of a cycle differs
between copies by ~10 m of position, ~2 m/s of velocity and — the killer —
**~1.7 rad (99°) of yaw**. The map's one hard obstacle is a 71 m gap that needs
≥300 km/h at its lip, and it does not tolerate that.

Second result, and the more transferable one: **the segment-map / promoted-gate
objective everybody on this project uses for shaping is not safe to search
against.** Promoting a checkpoint to a finish swaps the item model, and the
finish model's trigger volume is much larger. A 14-minute search against
`map_seg3` produced a tape that "finishes" at 34.784 — a 14-second gain — and
that tape collects **only CP1 and CP2 on the untouched map** and fails a
*checkpoint*-model gate placed at CP3's exact position at all four yaws. The
search had found the enlarged volume, not a route.

---

## 1. The geometry, from the map file (new capability)

Every one of this map's 186 blocks is a **free block**: the block record carries
flag `0x20000000` and the cell sentinel `(-1,0,-1)`, so a normal block lister
prints a geometrically empty dump, and the cell→world rule from
`tm2020-coldstart.md` says nothing here. The placements live in skippable chunk
**`0x0304305F`**: `u32 version`, then per free block in block-list order
`Vec3 pos` + `Vec3 pitchYawRoll`, 24 bytes each (186 × 24 + 4 = 4468 bytes
consumed of a 20740-byte payload; the rest is not needed).

New subcommand `tmmaps freeblocks MAP [--chunks]` prints it.

**Acceptance control:** the `PlatformWaterStart` (Spawn) block reads
(776, 1872, 943) and the ghost's t=0 sample is (792.0, 1873.1, 927.0) — exactly
one half-cell (16 m, 16 m) away, i.e. the car sits at the centre of the block
whose anchor was read. The six `GateFinish` blocks land on the last samples of
the record.

Block census: 84 `PlatformTechLoopStart`, 32 `PlatformPlasticLoopOutStartCurve1`,
16 `PlatformTechLoopStartCurve0OutFull`, 15 `PlatformWaterRampBase` (3 groups of
5), 8 each of `PlatformTechWallCurve3x4` / `PlatformTechSlope2Start` /
`PlatformTechLoopStartCurve1In`, 6 `GateFinish`, 4
`PlatformIceLoopStartCurve0Out`, 2 `PlatformTechSpecialTurbo`, 2
`PlatformTechBase`, 1 `PlatformWaterStart`.

## 2. The screw symmetry S, and its control

`tmmaps bowl sym --map M [--csv wr.csv --times ...]` derives the transform from
two block correspondences and then checks it against everything:

```
S: rotate -2.094395 rad (-120.000 deg) about axis (x=772.2857, z=821.4282), dy=-56.0000
block images: 129 land on a same-model block (worst 0.846 m), 57 do not
```

The 57 that do not are exactly the ones with no image: the last copy (its image
would be a fifth module), the start furniture (spawn, two turbo blocks, two
bases) and the six finish gates. **The instrument can say no** — that is what
the 57 are.

The checkpoints are exact images of one another:

```
S(CP1 1049.0,1946.0,960.0) -> 753.9,1890.0,512.5   nearest waypoint 0.00 m = CP2
S(CP2  753.9,1890.0,512.5) -> 513.9,1834.0,991.8   nearest waypoint 0.00 m = CP3
S(CP3  513.9,1834.0,991.8) -> 1049.0,1778.0,960.0  nearest waypoint 0.00 m = CP4
S(CP4) -> 168 m from any waypoint (the finish cluster sits lower instead)
```

Gate yaws follow: 0.524 → 2.618 → 4.712 → 0.524 (+2.094 per copy). The two boost
pads of each module transform with it; copy 0's launcher is the start platform
instead of a water ramp, which is why the standing start is the only sector
nobody fails.

**What one module is, physically** (read off the human's clean cycle):
cross the checkpoint airborne → land on the slope → ride the chute, gaining
speed → **fly the 71 m gap between the two `LoopStartCurve1In` blocks at
canonical (1051,1848,1062) and (980,1816,1066)** → run the tube → cross the
water run over two boost pads at 84→96.5 m/s → launch off the ramp → ride the
wall curve up → cross the next checkpoint airborne at ~46-53 m/s.

## 3. The transplant experiment — the headline negative, with its enumeration

The hypothesis: the inputs are body-relative, the physics is invariant under S,
so copy 0's clean cycle replayed from copy 1's checkpoint should drive copy 1.

Everything below was validated on the **untouched map**; `tmpk asm` splices are
exact (control: an identity rebuild of the record's prefix returns 24.213 on
`map_seg2`, the record's own CP2 split).

| construction | tapes | result |
|---|---|---|
| prefix→t2 + 4× cycle-1 inputs, phase (t1,t2) swept | 70 | all DNF, cps=2 |
| same, phases chosen to minimise **attitude** mismatch | 15 | all DNF, cps=2 |
| prefix→CP2 + **respawn press** + 1..3× cycle-1 inputs | 6 | all DNF, cps=2 |
| same, start-of-copy phase swept 10.650…12.550 in 0.1 s | 20 | all DNF, cps=2 |
| the best S-matched period in the whole record (sector 3, dpos 4.68 m, dyaw 0.001), ×1..3 | 4 | all DNF, cps=3 (i.e. the prefix's own CP3, nothing more) |

That is 115 tapes and the sweep is exhaustive in phase over ±1 s at 10 ms
resolution, which is the parameter the hypothesis actually has.

**Why it fails, measured.** Fold the record's checkpoint crossings into the
module frame and compare state(CP_{k+1}) with S(state(CP_k)):

| phase choice | Δposition | Δvelocity | Δyaw | Δattitude (yaw+pitch+roll) |
|---|---|---|---|---|
| best position/velocity match | 10.65 m | 2.11 m/s | **+1.758 rad** | 4.63 |
| best attitude match | 12.96 m | 6.72 m/s | −0.019 rad | 1.99 |

There is no phase where both are small. A car handed to the chute 10 m off and
rotated 99° does not ride it.

**Where the transplant dies.** Position-only checkpoint rungs (§5) placed on the
S-image of copy 0's clean line show the transplanted tape passing
S(13.0 s) — 2.1 s after the checkpoint — and missing S(14.0 s) and everything
after. It leaves the line inside the first three seconds, in the chute, well
before the gap.

## 4. Respawn semantics, corrected and sharpened

`RESULT-v1` §6b said a soft respawn restores "your own crossing state". The
useful correction:

* **The crossing state is NOT the post-respawn state.** Appending a sector's
  inputs at the bare checkpoint crossing (no press) fails: 5 phases tried
  (±20 ms), all DNF at cps=2. The record's own respawn point sits ~0.95 s of
  driving *before* the gate.
* **With a synthesised press it is exact**, reproducing `RESULT-v1` §4:
  `keep:-2000:24220, resp:51780, keep:51790:76230` validates on `map_seg3` at
  **48.759** (the record needs 76.228 for the same checkpoint), and the full
  four-sector composition
  `…,resp:82680,keep:158830:184650,resp:188340,keep:416510:440250` validates on
  the real map at **98.268** — the independent rebuild of `clean_best`'s
  97.898, 0.37 s off it only because I did not tune the press phases.
* **Sectors are coupled, and violently.** Removing the brake from the last
  1.2 s of cycle 1 — which moves the CP2 crossing by **13 ms** — makes the
  *unchanged* downstream sector DNF. Four different cycle-1 edits (no-brake,
  no-steer, throttle-lift, extra throttle in the chute), all of them ≤ 13 ms of
  crossing-time difference, all break the tail. Left-to-right is not a
  preference here, it is the only legal order.
* Forcing full throttle through the chute (10 windows tried) does not make the
  gap: every variant DNFs at cps=1. The human's throttle lifts in the chute are
  load-bearing.

## 5. THE INSTRUMENT TRAP (most transferable finding)

A checkpoint gate can be relocated two ways, and they are not interchangeable.

**Position-only** (`tmmaps rung`, new): `MapFile::move_item` only, no model
swap. Round-trip control: move CP4's gate to its own position, rebuild, and the
record validates at **440.238** and `best_97461` at **97.461** — exact.

**Promoted** (`tmmaps finrung`, new; the same thing `segments::move_gate` and
every `map_seg*` does): the item model is swapped to `GateFinish32m` first.
Round-trip control also passes — the gate at CP4's own position returns the
record's CP4 split **184.638** to the millisecond.

Both controls pass, and **the promoted gate is still not a safe search
objective**, because its trigger volume is bigger than the checkpoint's:

```
search on map_seg3 (CP3 promoted to finish), 14.7 min, seed 48.759
  -> "34.784"     (-13.975 s, and it looks like the route everyone wants)
the same tape:
  untouched map                      DNF, cps=2   (CP3 never collected)
  checkpoint-model gate at CP3's own position, yaw 0.524 / 2.095 / 3.666 / 4.712
                                     DNF, cps=2   (all four)
  position-only rungs on the target line: passes 2 of 7
```

So the tape was not near the route at all; it was clipping the enlarged finish
volume. **A segment map is a fine RULER for a tape that genuinely crosses the
checkpoint; it is not a safe OBJECTIVE for a search, which will find the
volume.** The honest substitutes, both used here:

* `tmmaps rung` — position-only, checkpoint stays a checkpoint, progress read
  off the validator's `cps`. All triggers real.
* `tmmaps finrung --at <point ~15-50 m past the real gate> --keep <the real
  checkpoints>` — the finish is beyond the gate, so the real gate must still be
  collected for the run to count. Controls: the record returns 76.574 for a
  finish 15 m past CP3 (its CP3 is 76.228), 186.071 for one 48 m past CP4 (CP4
  at 184.638).

Two smaller gate facts, both measured: the trigger **is** directional (a
water-run rung fires at yaw 2.095 and not at 0.524/3.666/5.236), and **crossing
a finish gate before the remaining checkpoints are collected voids the rest of
the run** — which is why a rung must be placed where the car cannot reach it
early, and why `map_seg2` reports `DNF cps=1` for any tape that respawns after
crossing CP2.

## 6. Honest search yield, measured twice

Cycle 2 (CP2→CP3), seed = the record's own sector-3 line spliced after a press
(48.759 to the finish 15 m past CP3):

```
14.7 min, 80 workers, 54 080 evaluations, 62-137 evals/s
48.759 -> 48.644     -0.115 s   (best-of-two searches: -0.38 s from 49.024 on the same instrument)
```

i.e. **0.2-0.8 %**, in the same range `RESULT-v1` §6.3 measured (0.45 %), on an
objective that is now 7× cheaper per evaluation. Evaluation rate is **not** the
binding constraint (128-137 evals/s sustained on a 40 s tape, and 70 spliced
tapes validate in 3.8 s); the binding constraint is that local search cannot
change which side of the gap the car lands on.

## 6a. THE MEASUREMENT THAT EXPLAINS THE MAP: the cycle is lossy, and the loss
## is in the LAUNCH

The obstacle is a 71 m gap whose lip is the `LoopStartCurve1In` block at
canonical (1051,1848,1062). `tmmaps bowl lips` asks the record, for every one of
its 32 respawn-delimited attempts and every copy, how fast the car was at its
closest approach to that copy's lip:

```
lip copy 0 (1051.0,1848.0,1062.0)   lip copy 1 (841.3,1792.0,459.8)
lip copy 2 ( 424.6,1736.0, 942.5)   lip copy 3 (1051.0,1680.0,1062.0)

copy 0:  299 km/h @9.250   302 km/h @14.500          <- the two that fly it
copy 1:  121 km/h  135 km/h
copy 2:  113 km/h  101 km/h  115 km/h  ...  254 km/h @109.700, 255 km/h @157.750
copy 3:  215 km/h  212 km/h  236 km/h  245 km/h  and 14 more below 100 km/h
```

**Every approach in copies 1-3 — 23 of them — is slower than every approach in
copy 0.** The best non-copy-0 attempt in the entire record is 255 km/h against
the ~300 km/h the jump needs. This is not a driver making an occasional
mistake; it is a systematic deficit that grows with every cycle, and it is visible
in one column of numbers: the checkpoint crossing speeds are

```
CP1 52.8 m/s   CP2 45.8   CP3 41.1   CP4 37.4       (monotone decay)
```

Each copy sits 56 m lower than the last, which is worth +10.5 m/s of gravity at
these speeds. Breaking even would put CP2 at ~62 m/s; the record arrives at
45.8. **So a cycle of this map is an energy balance, and the human's line loses
about 16 m/s of it per lap.** Once you are below the gap threshold you fall in,
pay 15-20 s, and enter the next copy slower still — which is exactly the 8.7×
record.

Where does copy 0 get its advantage? Not from speed on the run-up: the standing
start crosses its water run at **89.8 m/s** and crosses CP1 at **52.8**, while
cycle 1 crosses the same (S-imaged) run at **96.5 m/s** and crosses CP2 at
**45.8**. Seven metres per second *more* on the run, seven *less* at the
checkpoint — a 14 m/s swing, and all of it happens in the **launch**: ramp →
wall curve → airborne crossing. In the record's cycle-1 launch the car loses
16.6 m/s of *horizontal* speed between 23.0 s and 23.5 s while the telemetry
says it is airborne — it is scraping up the wall curve, with full left steer and
the brake held from 23.02 s through the crossing.

**That, not the gap and not the chute, is where the author time is.** The gap is
a threshold, and the launch is what decides which side of it you are on.

## 6b. The two gains that DID validate: 97.461 → 97.325

Both are composable — they touch nothing that the downstream sectors depend on —
and both were confirmed cold on the untouched map in a batch carrying the
record (440.238), `clean_best` (97.898) and the previous best (97.461) as
known-answer controls, all three exact.

**(1) An exhaustive prefix sweep at CP1: −0.100 s.** `RESULT-v1` §4 reported
that a respawn press inserted after the CP1 crossing DNFs from +43 ms to +3.5 s
while 10.440 / 10.540 / 10.840 are exact, mechanism unknown, and
`TRIAL-CUTTING-RULES-v1` Rule 2 says a cut to a standing respawn works at **one
non-periodic phase** that a coarse sweep will miss. So I swept the prefix cut
point at **tick resolution**: 375 tapes, every 10 ms from 6.800 to 10.540, graft
fixed at the existing press (10.550).

```
survivors: 10.440 … 10.540 only — 11 contiguous phases, arithmetic exact
           (97.461 − 10·(10540 − X)/10 ms)
dead:      6.800 … 10.430 — 364 tapes, all DNF
```

So the phase rule does not rescue anything here, and that is now an
**exhaustive** negative at the resolution the tape has, not a sampled one. The
best cut yields **97.361**. The 3.64 s of CP1 flail before 10.440 is genuinely
unrecoverable by cutting.

**(2) A final-sector search: −0.036 s.** Two 24.5-minute searches on the
**untouched map** (the finish objective is honest by construction — no gate
surgery anywhere), 80 workers each, seeded with `best_97461`:

```
window ticks 7568-9940 (last sector)   121 360 evals, 82 evals/s -> 97.432
window ticks 4900-9940 (last two)      109 800 evals, 75 evals/s -> 97.425
```

Composing the two: **97.325**, validated. That is 1.93× the author time — the
gain is real and measured, and it is also a fair statement of what polishing
this tape is worth: 231 000 evaluations bought 0.036 s.

## 7. What I believe now, and what I would do next



The author time is **not** out of reach and the budget is unchanged:
`6.797 + 4 × 10.9 ≈ 50.4`, against a cycle a human has already driven in
13.163. Everything hangs on one question, and this session narrowed it to a
question a next agent can attack directly:

> **From the state a copy hands you (post-crossing or post-respawn), what
> inputs carry the car through the chute at ≥300 km/h to the lip?**

Copy 0's answer does not transfer, and now we know why (99° of yaw, 10 m of
position). Three things I would try, in order:

1. **Search the LAUNCH, not the cycle.** The handover state is set by the
   previous cycle's last ~1.5 s (ramp → wall curve → airborne crossing). That is
   a 150-tick window and the objective is a state match, not a time: minimise
   |state(CP_{k+1}) − S(state(CP_k))| including attitude. That needs per-tick
   state, which means the fork server — with the boundary at the cycle start,
   where the candidate shares the template's prefix exactly, which is the regime
   the reliability work showed to be exact. I could not use it tonight: `fk
   btraj`'s blind locate aborted twice (`vel_err 7.34` and `9.26 m/s`,
   "refusing to guess") at fork ticks 100 and 2496 on this map, ~5.5 min each.
   **Fixing the locator on this map is the single highest-value piece of
   tooling.**
2. **A rung ladder along the S-image line, honest gates only** (§5), used as a
   curriculum: reward S(13 s), then S(14 s), then S(14.5 s) — the lip. The
   ladder positions are printed by `tmmaps bowl simg --n 1|2|3 --times …`, and
   the calibration of which yaw fires is in §5.
3. **Left-to-right, and re-derive after every change** — §4's 13 ms result means
   any cycle-2 gain invalidates cycles 3 and 4 outright.

What I would **not** do again: transplanting copy 0's tape (115 tapes say no),
and searching against a promoted-gate objective (§5).

## 8. Tools built this session (all in `tmtas-hard`, Rust, no Python)

| tool | what |
|---|---|
| `tmmaps freeblocks MAP [--chunks]` | free-block placements from chunk `0x0304305F` |
| `tmmaps bowl geom\|at\|attempts\|path` | geometry dump; what is near the car at time t; the record's 32 respawn-delimited attempts; an annotated trajectory window |
| `tmmaps bowl sym\|period\|period2\|simg` | derive S and check it; find the phase at which a segment is an S-period (position, velocity **and** attitude); print S^n of a trajectory as rung coordinates |
| `tmmaps rung` | **position-only** gate relocation (no model swap) |
| `tmmaps finrung` | relocated finish gate + `--keep` of the real checkpoints |
| `tmpk` ops `gas/nobrk/nosteer/lift:A:B` | force input fields over an original-ms window, composable with `keep`/`from` (provenance tracked) |

Artefacts under `/tmp/w284` on `125408.od.fbinfra.net` (node-local, will die with
the node): `data/freeblocks.tsv`, `rungs/*.Map.Gbx` (calibrated instruments),
`RP/ctrl_s3.Ghost.Gbx` (48.759 to CP3, the cleanest cycle-2 search template),
`comp/clean.Ghost.Gbx` (98.268, the independent `clean_best` rebuild),
`comp/final.Ghost.Gbx` (**97.325**, banked beside this file as
`best_97325.Ghost.Gbx`), and `tools_284238_v1.tgz` (the sources of every tool in
the table above: `bowl.rs`, `main.rs`, `tmpk.rs` — drop into `tmmaps/src/` and
`tmsearch/src/bin/` of a `tmtas-rs-hardened` tree).

**Nothing was submitted anywhere.** No Nadeo leaderboard interaction of any kind.
