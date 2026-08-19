# 284238 "YOU LOVE WATER" — map geometry: **the three bowls are ONE construction, built four times**

Sidecar to `RESULT-v1.md` (2026-08-18 20:58), which closed this map as
route-construction. **This measurement reopens it.** Nothing in `RESULT-v1.md`
is wrong; it was written without the block layout, and the block layout changes
the conclusion.

Written 2026-08-19 04:2x UTC. Everything below is measured from
`map.Map.Gbx` (sha256 `ecbeca11…f60d8`) and `wr.csv` (the one human record,
440.238, decoded). No search was run and nothing was submitted anywhere.

---

## 0. The answer

> **Same construction. Not three bowls — FOUR copies of one 40-block module,
> related by exact rigid transforms: yaw +120°, +240°, and a pure −168 m
> vertical translation. Position residual 0.03–0.20 m RMS over 40 blocks.
> The checkpoint gates and the boost pads are part of the repeated unit too
> (0.03 m and <1 m under the same transform), so it is not only the scenery
> that repeats — the TASK repeats.**

And the follow-on, which is where the value is:

> **The human already solved it once, in 13.163 s, in copy 0 — and folded into
> the module's own frame, his four traversals are the same line. The
> difference between the 13.163 s traversal and the 23.7–25.8 s ones is a
> single measurable quantity: the speed carried over one 71 m gap.
> ≥300 km/h clears it (5.3 s to the exit); 61–255 km/h does not (14.5–20.4 s).
> That is an optimisation problem on ONE 13-second obstacle, solved once and
> transported three times by a known exact transform.**

Confidence: **high** on the geometry (four independent controls, below);
**medium-high** on the reopening argument (the budget arithmetic is sound but
the transport of a line between copies is a hypothesis that has not been put
through the oracle yet — see §7).

---

## 1. Why nobody had read the layout: this map has NO placed blocks

`listall.txt` (banked 19:38) reports **every one of the 186 blocks at
`cell=(-1,0,-1)`**. That is not a parse bug and it is not a degenerate map: all
186 blocks are **free blocks** (`flags & 0x20000000`), and a free block does not
store its cell. Its world placement lives in a separate skippable chunk. The
dump was therefore geometrically empty, which is exactly why the three "bowls"
had only ever been described by the bounding box of one lost driver's wandering.

**Chunk `0x0304305F`**, at body offset 2140568, payload 20740 B:
`u32 version (=0)` then **864 records of 24 bytes** — `Vec3 absolutePosition`,
`Vec3 pitchYawRoll`. The first 186 records are the main block list in order; the
remaining 678 belong to the baked-block list (`0x03043048`).

New Rust subcommand `tmmaps freeblocks` reads it (source banked at
`tools/freeblocks.rs`; add to `tmmaps/src/`). Output: `GEOMETRY_v1_blocks.tsv`
(186 blocks + 276 items with world positions and angles).

### The instrument's four controls

Per §0.4 — an instrument that can only say yes is not an instrument.

1. **Id-free structural scan agrees with the chunk-id read.** The run of
   plausible 24-byte records is also located by scanning the body for the
   longest run of (finite position in 1..32768, three angles in ±2π) with no
   reference to `0x0304305F`. `structural_run == chunk payload+4`, `agree=true`.
   (First attempt at this said 16724 records at offset 45980 — denormal float
   garbage that prints as `0.00` passes a naive "finite and in range" test. The
   lower bound of 1.0 m fixes it. Worth knowing: **a float-plausibility scan
   without a lower bound finds junk everywhere.**)
2. **Spawn ↔ ghost start.** `PlatformWaterStart` (B89) parses to
   (776, 1872, 943). The human ghost's sample at t=0 is (792, 1873.1, 927) —
   the block origin plus exactly half a cell in x and −half a cell in z. ✔
3. **Goal ↔ ghost end.** The six `GateFinish` blocks parse to a scatter over
   x 717–794, y 1578–1662, z 556–690; the ghost's last samples fall through
   that scatter at 269 km/h. ✔
4. **The rigid fit can say NO.** Residual of the copy-0→copy-1 fit as a
   function of the trial rotation: **0.18 m at 120°, and 18–181 m at every
   other angle sampled at 10° spacing.** A negative control (copy 1 displaced
   ±20 m alternating in x, a warp no rigid transform can undo) refits at
   **29.562 m** against the unmodified **0.182 m**. ✔

§9b's warning (item `cell_y = floor(y/8) + 8`) was checked and holds here
(item at y=1792 ↔ cell y=232), but nothing in this analysis uses cells: free
blocks and items both carry absolute float positions.

---

## 2. The parts accounting: 186 = 4×40 + 4×5 + 6, exactly

| group | blocks | what |
|---|---|---|
| **core module ×4** | 4 × 40 | the obstacle |
| **launcher ×4** | 4 × 5 | what fires you into a copy |
| finish net | 6 | `GateFinish` |
| **total** | **186** | ✔ |

The core module, 40 blocks, identical in all four copies:

```
 1 x PlatformIceLoopStartCurve0Out
 2 x PlatformTechWallCurve3x4          (a vertical pair, 32 m apart)
 2 x PlatformTechSlope2Start           (a vertical pair, 32 m apart)
 2 x PlatformTechLoopStartCurve1In     (the two LIPS of the gap -- see §5)
 8 x PlatformPlasticLoopOutStartCurve1 (4 spots x 2 stacked = the ring)
21 x PlatformTechLoopStart             (the tube / "bowl")
 4 x PlatformTechLoopStartCurve0OutFull
```

Copy → block indices, and the fitted transform from copy 0:

| copy | blocks | world centroid | transform from copy 0 | rms | worst |
|---|---|---|---|---|---|
| **0** | 2,3,4,6–42 | (965.2, 1859.1, 933.2) | identity | — | — |
| **1** | 43–81, 95 | (579.0, 1747.1, 932.7) | **yaw +120°** | **0.182 m** | 1.13 m |
| **2** | 96–134, 140 | (965.3, 1691.1, 933.2) | **pure translation (0, −168, 0)** | **0.196 m** | 1.22 m |
| **3** | 146–185 | (772.6, 1803.1, 598.5) | **yaw +240°** | **0.182 m** | 1.14 m |

Copies 1↔2 fit at **0.029 m**. Pitch and roll are preserved by every fit; the
apparent 120° "pitch differences" the fit reports are a greedy-pairing artefact
on the **3 pairs of co-located same-type blocks per copy** (this map builds a
closed tube by stacking two half-loops at one position with different
orientations), not a geometric difference.

### The convention-free proof

The copy 0 → copy 2 transform is a **pure vertical translation**, so no Euler
convention needs to be assumed to test identity exactly:

> For every one of copy 0's 40 blocks, copy 2 holds the **same block type**, at
> **position + (0, −168, 0)**, with the **same raw (pitch, yaw, roll) triple**.
> **40/40. Worst position error 1.22 m** (one `IceLoopStartCurve0Out` placed
> 169 m rather than 168 m down — the author nudged one block).

Overlaying all four copies in copy 0's frame collapses 160 blocks onto **40
canonical positions**, every one shared by all four copies to ≤0.2 m
(`GEOMETRY_v1_canonical_layout.tsv`).

### The items repeat too

| item | copy 0 → 1 (yaw 120°) | copy 0 → 2 (0,−168,0) | copy 0 → 3 (yaw 240°) |
|---|---|---|---|
| `GateCheckpointCenter32mv2` | **0.03 m** | **0.03 m** | **0.03 m** |
| `GateSpecial32mTurbo2` (boost pad) | 0.91 m | n/a¹ | 0.83 m |

¹ copy 2's boost pair is the one that fires into copy 2; the run ends at the
finish net instead of a fifth copy, so there is no pad at the predicted place.
Nothing is missing — the six pads are three pairs, one per transition.

### The one asymmetry — and it is not in the obstacle

The four **launchers** also overlay (canonical y = 1872, lane running
x 776 → 936), but copy 0's is built differently:

| copy | launcher (canonical) |
|---|---|
| 1, 2, 3 | 5 × `PlatformWaterRampBase` at x 807 / 839 / 871 / 903 / 903, z ≈ 911, plus 2 boost-pad **items** |
| **0** | `PlatformWaterStart` at x 776, 2 × `PlatformTechSpecialTurbo` **blocks** at x 808 / 840, 2 × `PlatformTechBase` at x 904 / 936 — all at z 943 (a 32 m offset) |

**Copy 0's odd launcher is used only for the standing start.** Each cycle
CP_k → CP_{k+1} traverses copy k's obstacle and finishes on copy (k+1)'s
launcher, so **every one of the four timed cycles uses a water ramp.**

---

## 3. What the module actually is, in canonical coordinates

Route order, measured from the ghost: **copy 0 → copy 3 → copy 1 → copy 2 →
finish**. `CP_k` is copy k's own gate; the launcher of copy k fires you at it.
One canonical cycle:

```
  launcher lane      y 1872, x 776 -> 936, two boost pads at x 808 / 840
      |  boost, ~300 km/h
      v  ~3.1 s of flight
  CHECKPOINT         (1049, 1946, 960)
      |  a curved entry chute, dropping ~100 m over ~3.5 s
      v
  LIP  (1051, 1848, 1062)   PlatformTechLoopStartCurve1In
      |
      |   *** THE GAP: 71 m across, 32 m down ***
      v
  FAR LIP (980, 1816, 1066) PlatformTechLoopStartCurve1In
      |
      v  ride the TUBE south-west and down
  the tube: two parallel chains of PlatformTechLoopStart 63.5 m apart,
  blocks every 32 m along a 60-degree diagonal, from (1019,1848,1007) down
  to (820,1848,789) -- a half-pipe ~320 m long
      |
      v
  exit lane          y ~1818, z falling 870 -> 730
      |
      v  onto the NEXT copy's launcher
```

The **"bowl"** every earlier note refers to is the closed north-east end of that
tube, canonical x 970–1035, z 1005–1077. It is where you land **if you fall
short of the gap**. The three bowl bounding boxes in the brief are that same
canonical volume seen through three different rigid transforms:

| brief's "bowl" | copy | canonical |
|---|---|---|
| CP2→CP3, x 730–870, y 1760–1800 | copy 3 | the tube plane |
| CP3→CP4, x 427–500, y 1706–1740 | copy 1 | the tube plane |
| CP4→finish, x 965–1030, y 1650–1665 | copy 2 | the tube plane |

And "CP1→CP2, not identified as a bowl" is **copy 0's tube** — the same bowl.
The human just did not fall into it that time.

Even the **finish net** sits on the canonical line: the six `GateFinish` blocks
map (copy 2's frame) to canonical x 717–794, y 1746–1830, z 556–690 — strung
out along the continuation of the exit lane past (828, 1818, 732). The author
placed a catcher on the same trajectory rather than a fifth checkpoint.

The map is a sky map: all terrain/lake/rock scenery sits at y ≈ −56…400,
1200 m below the playfield. The four copies are in physically identical
conditions — uniform gravity, nothing else within reach.

---

## 4. The human's four traversals ARE the same line

Folded into the module frame, the four successful traversals overlay. From
t+14 to t+22 the last three agree to a few metres over eight seconds:

| t+ | copy 3 | copy 1 | copy 2 |
|---|---|---|---|
| 16.0 | (943,1837,880) | (940,1837,878) | (942,1836,880) |
| 17.0 | (903,1818,873) | (903,1819,881) | (904,1819,883) |
| 18.0 | (870,1847,872) | (871,1845,873) | (872,1845,874) |
| 19.0 | (858,1819,805)* | (860,1851,851) | (860,1851,852) |
| 20.0 | (841,1818,759) | (852,1832,831) | (851,1831,828) |
| 22.0 | — | (831,1818,734) | (823,1817,730) |

(*copy 3 is ~1 s ahead through this stretch.) Every respawn also restores to
the same canonical point, ≈ (1015, 1926, 930), in all four copies.

**Copy 0's traversal is the odd one out — because it is fast.** It reaches the
exit lane at t+8.8 where the others reach it at t+18 to t+22.

---

## 5. The whole 40 seconds is ONE mistake, repeated: the gap

Canonical trace at the lip:

| traversal | speed at the lip | what happened next |
|---|---|---|
| **copy 0 (13.163 s)** | **305 km/h** | flew the gap: (1034,1847,1063) → (1004,1822,1064) in 0.5 s, landed on the tube at 306 km/h and rode it down |
| copy 3 (24.428 s) | 240 km/h | fell in at (1033,1842,1056), 129 km/h |
| copy 1 (25.788 s) | 251 km/h | fell in at (1030,1850,1066), 89 km/h |
| copy 2 (23.738 s) | 210 km/h | fell in at (1037,1848,1046), 109 km/h |

Across **all 23 attempts in the record** (`tmmaps lip`, every attempt printed —
successes and failures — so the correlation can be read rather than asserted):

* lip speed **302 and 305 km/h** (2 attempts): cleared the gap. The one that
  went on to finish the sector reached the exit lane **5.300 s** after the lip.
* lip speed **61–255 km/h** (21 attempts): did not clear. When they reached the
  exit lane at all it took **14.5 / 15.6 / 15.6 / 16.0 / 16.7 / 18.4 / 20.4 s**.

One attempt (t=367.550, lip 245 km/h) is scored `cleared=true` by the crude
predicate but died 4.4 s later without reaching the exit; treat the threshold as
**bracketed between 255 and 302 km/h, not measured**. That bracket is the single
most valuable unmeasured number on this map and one sweep would pin it.

**Cost of missing the gap: 9–15 s, three times over.** That is the 40 s.

---

## 6. The budget — why this reopens the map

Human clean-equivalent (from `RESULT-v1.md` §3, splices verified there):

```
 6.797  standing start -> CP1   (launcher + flight only)
13.163  cycle 1  (copy 0)   <-- cleared the gap
24.428  cycle 2  (copy 3)
25.788  cycle 3  (copy 1)
23.738  cycle 4  (copy 2, ends at the finish net: ~3 s shorter than a full cycle)
------
93.914
```

Author time **50.459**. Take the start as given (6.797) and let F be a full
cycle, with cycle 4 ≈ F − 3:

```
6.797 + 3F + (F - 3) = 50.459   ->   F = 11.67 s
```

**The author time needs 11.67 s per cycle. A human already did 13.163 —
on his first try, entering at only 218 km/h off a checkpoint restore, and
dawdling to 190 km/h in the entry chute before the gap.** That is a **−11.4 %**
optimisation of one 13-second obstacle, not a route discovery.

Compare `RESULT-v1.md` §7: *"the conceivable ceiling of removing every bowl is
~45 s of savings against 47.4 s needed"* — same magnitude, opposite conclusion,
because that estimate assumed the fast line had to be **invented three times**.
It does not have to be invented at all: it is in the human's own tape, and the
transform to the other three copies is exact.

Three of `RESULT-v1.md`'s three blockers weaken or dissolve:

| blocker (RESULT-v1 §6) | after this measurement |
|---|---|
| car model explains 2.7 % of yaw-rate variance | still true, and still says steering priors are worthless here — but the target is no longer "steer well", it is "arrive at one point above a threshold speed", which the model is not needed for |
| evaluation ~100× slower (99 s tape, 67 evals/s) | you search **one 13-second cycle** on a segment map, not the 99 s tape. The segment maps are built and proven exact (`RESULT-v1` §6a) |
| sectors are not independent (§6b, respawn carries the crossing state) | still true — but all four sectors now have the **same** target state and the **same** line, so it is one problem sequenced four times, not four problems |

---

## 7. What to do next, in order — and what could still kill it

1. **Pin the gap threshold.** Sweep entry speed at the lip on one segment map
   and find where "clears" flips. One cheap sweep; it converts §5's bracket
   (255–302 km/h) into a number. This is also the **early-abort predicate** the
   project already has machinery for (`tm2020-predicates.md`): a sector can be
   killed 3.5 s in, on the map where evaluation cost was the stated blocker.
2. **Try the transport directly.** Take the input sub-tape of copy 0's
   traversal (ghost times 11.050 → 24.213) and replay it as cycle 2/3/4 with
   `tmpk asm`. Physics is invariant under rotation about the vertical and the
   copies differ only by yaw and height, so if the canonical entry state
   matches, the canonical trajectory must. **This is the experiment that
   decides everything and it is one oracle batch.**
3. **Then search one cycle** on `map_seg*`, left to right, seeded with copy 0's
   line, targeting 11.7 s.

**What could kill it, honestly:**

* **§6b bites here.** A soft respawn restores *your own* crossing state, so
  cycle k+1's entry state is whatever cycle k's exit produced. The transported
  line is only valid if the entry state can be reproduced — the transform
  guarantees the *geometry* matches, not that a given entry state is reachable.
  Step 2 above tests exactly this and nothing else should be believed until it
  passes.
* **The threshold may sit above what a cycle can deliver.** If clearing the gap
  needs ~300 km/h at the lip and the launcher can only deliver an entry that
  reaches ~305 with a perfect chute, the margin is thin and every cycle is a
  precision problem. The human hit ≥300 twice in 23 attempts.
* **11.67 s may not be reachable** even with the gap cleared every time; copy 0's
  13.163 decomposes as 3.5 s chute + 5.3 s lip→exit + 1.2 s exit→launcher +
  3.16 s launcher→CP, and the slack is mostly in the chute.
* I have run **no oracle validation** in this session. Every number here is
  read off the map file and the already-validated `wr.csv`. Nothing here is a
  time claim.

---

## 8. Artefacts

| file | what |
|---|---|
| `GEOMETRY_v1_blocks.tsv` | all 186 blocks + 276 items, world positions and angles |
| `GEOMETRY_v1_canonical_layout.tsv` | all four copies folded into copy 0's frame |
| `tools/freeblocks.rs` | `tmmaps freeblocks` — free-block placements from chunk `0x0304305F`, with the id-free structural cross-check |
| `tools/bowls.rs` | `tmmaps bowls` — cluster into constructions, rigid-fit them pairwise, item correspondence, and `tmmaps control` (the rotation sweep + warped negative control) |
| `tools/canon.rs` | `tmmaps canon / attempts / extra / lip` — fold a ghost into the canonical frame, split it at respawns, and measure the lip |

Drop the three `.rs` files into `tmmaps/src/`, add
`mod freeblocks; mod bowls; mod canon;` and the six match arms. The crate has no
dependencies and builds standalone in under a second.

**Transferable, for `ACQUISITION.md`:**

> **§6 addendum — a map whose blocks all report `cell=(-1,0,-1)` is not
> broken, it is a FREE-BLOCK map, and its geometry is in chunk `0x0304305F`:
> `u32 version`, then 24-byte records (`Vec3 position`, `Vec3 pitchYawRoll`),
> main block list first, then the baked list. `tmmaps listall` prints nothing
> useful on such a map and this cost this project a closed target for four
> hours. Check the free flag (`flags & 0x20000000`) before believing an empty
> geometry dump.**
>
> **And: before concluding a long map is route-construction, cluster its blocks
> and rigid-fit the clusters against each other. A map built from one repeated
> module is a short map wearing a long map's clothes.**
