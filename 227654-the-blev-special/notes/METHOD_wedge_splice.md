> **SUPERSESSION.** This file supersedes the verdict in this directory's
> `RESULT.md` and `RESULTS-entry.md`, both of which read "the advertised gap is
> an artefact; the author time was NOT beaten, 2.059 s short at 59.912". That
> was **correct when written** and is kept deliberately — its diagnosis of the
> blocker is what made this session's approach possible. It is now superseded:
> **the author time IS beaten, validated at 57.573 (later 57.537) on the
> untouched map**, and independently re-validated 7/7 from the banked files on
> another agent's build (`aud_VERIFICATION_227654_independent_v1.md`).
> A later correction to one claim below is in `blev2_GATE_GEOMETRY_v2.md`.

# 227654 "The Blev Special" — AUTHOR TIME BEATEN

**AT 57.853 · best validated 57.573 · margin −0.280 s** (agent `blev2`, 2026-08-19)

Prior state: 59.912, closed 2.059 s short, reopened because its blocker had been
solved elsewhere. This is the write-up of how it was closed.

## The validated result

Plain oracle, **untouched** `map.Map.Gbx`
(sha256 `a5768448d61edfc32da243a74c098b18314724342f9e0ce1895a872eb05b8d82`),
one batch, known-answer controls in the same batch:

| file | sim_time | note |
|---|---|---|
| `blev2_tas_57573_v1.Ghost.Gbx` | **57573** | new best, beats the AT by 280 ms |
| `blev2_tas_57577_v1.Ghost.Gbx` | 57577 | |
| `blev2_tas_57580_v1.Ghost.Gbx` | 57580 | |
| `clean_64871.Ghost.Gbx` | 64871 | control — prior agent's respawn-spliced WR |
| `tas_59912.Ghost.Gbx` | 59912 | control — prior agent's best |
| `rank00001_147031.Ghost.Gbx` | 147031 | control — human WR, exact leaderboard time |
| `rank00002_676640.Ghost.Gbx` | 676640 | control — human #2, exact leaderboard time |

sha256 of the winner: `365d822130e49379ea8eb47d3c5477ab4135a7bcc60490968da2d301d694af41`.
Nothing was submitted to any Nadeo leaderboard.

## What was actually blocking it

The inherited diagnosis was right about the symptom and wrong about the cure.
The tail did not re-derive from a searched prefix because **past CP2 the
searcher had no gradient**, and the two instruments that normally supply one
(`fk traj`, `tmmaps probe`) are both unavailable on this map. The proposed
cures were to fix the fork state locator or to build a corridor ladder out of
renamed blocks.

Both of those are dead ends here, and I established that cheaply:

* **`fk traj` still cannot locate the state, and now we know why.** With the
  merged all-entity telemetry as ground truth the best candidate address tracks
  the reference for **1028 of 13780 in-race ticks at 0.94 m RMS** and then goes
  stale. 1028 ticks is exactly the distance from the fork to the next
  `CSceneVehicleVis` boundary at 19.48 s. The vehicle state is **reallocated at
  every entity boundary** — and on this map the boundaries are at 19.48 s,
  CP1 (36.31 s), CP2 (54.33 s) and every respawn. The locate step's whole-run
  RMS gate can never pass. It is not a bad reference; it is a moving target.
* **A renamed block cannot be a rung.** Renaming any spare block into a
  waypoint model (`GateFinish`, `RoadTechFinish`, `RoadTechCheckpoint`) makes
  **every** run fail, wherever the block is put — the ghost declares
  `NbCheckpoints: 3` and a fourth waypoint is never collected. Renaming to a
  *non*-waypoint model is harmless (147031 unchanged), so the machinery is
  sound; the waypoint COUNT is the invariant.
* **A hole cannot be plugged.** Deleting a road block and putting a renamed
  spare free block at exactly its position, with its rotation, DNFs — tested
  with two different donor models over a deleted mid-road block, plus the
  CP1-cell case. So no rung can be bought by relocating CP1 or CP2 past their
  own cells.

## The instrument that does work

**Move the map's own finish gate (block #854) by cell.** It changes no waypoint
count and leaves no hole, because the finish platform is only ever reached at
the end of the run. `tmmaps moveblockcell --block 854 --cell cx,cy,cz`.

* Origin control: cell (45,34,10) → **147031 / 64871, exact**.
* Negative control: a cell off the route → DNF.
* On the ground it fires reliably. **In mid-air it never fires**: 53 cells along
  the 717 m flight arc and the full 3×7×3 neighbourhood of a mid-flight cell,
  **0 of 116**. So the arc cannot be laddered — but it does not need to be.

That gave a five-rung ladder from CP2 to the bowl exit, calibrated on
`clean_64871`:

| rung (cell) | world | clean_64871 |
|---|---|---|
| 27,33,18 | 880,202,592 | 55275 |
| 26,33,18 | 848,202,592 | 56190 |
| 25,31,18 | 816,186,592 | 57065 |
| 24,30,18 | 784,178,592 | 57940 |
| 23,30,18 | 752,178,592 | 58755 |
| 23,30,20 | 752,178,656 | 59458 |
| **26,31,20** | 848,186,656 | **60007** — the bowl exit, the launch |
| finish 45,34,10 | 1456,210,336 | 64871 |

Separately, and worth keeping: CP1 and CP2 are **free blocks** (positions in
chunk `0x0304305F`, not in the block record), so on `map_seg2` the finish can be
slid along the road with **float precision** — x=929 → 53903 (= `map_seg2`
exactly), 937 → 53620, 945 → 53308, 953 → 52907, 956 → 52703 — and rotated to
yaw 0 it reads the *other* axis, which is how the wedge slide was measured
(z=578.0 → 48788, z=578.5 → 48963). New `tmmaps` verbs for all of this:
`freeblocks`, `blocks`, `bchunks`, `moveblock`, `moveblockcell`.

## What the ladder immediately revealed

Grafting p3 (the best searched prefix, 46646 on `map_seg2`) onto the human's
post-CP2 program and measuring each rung:

* rungs 1–3 reached, 6.5–7.0 s ahead of `clean_64871`;
* **rung 4 never reached** — the run dies on the drop into the bowl.

The prior agent's 104 652-candidate (s, a, b) tail sweep was therefore searching
the bowl program while the run was dying 40 m before the bowl. No amount of
release timing could have found anything.

## What actually fixed it: align the graft by STATE, not by tick

The graft `p3[0,k) ++ clean_64871[j,end)` had been done with j chosen so that
the *clock* lined up. That is wrong: it hands the car inputs written for a car
45 m further down the road. Sweeping (k, j) **independently** at one-tick
resolution — 7826 combinations, k ∈ [4770,4860], j ∈ [5460,5545] — the ladder
lights up:

| rung | grafts reaching it |
|---|---|
| 24,30,18 | 1794 / 7826 |
| 23,30,18 | 975 |
| 23,30,20 | 264 |
| **26,31,20 (launch)** | **427** |
| finish | 0 |

355 of those 427 launch with an arithmetic floor under the author time. So the
problem reduced to the ballistic arc, and one more parameter closed it: the
**release tick**. Crossing all 355 good grafts with the release tick
b ∈ [natural−32, natural+4] — 13 135 candidates — gave **78 finishers**, best
57573. A wider refinement (36 859 more) confirmed the same optimum.

Two details that matter for anyone repeating this:

* **The hold start `a` is inert** — 452 finishers across a ±60-tick sweep of it
  all return the identical time. Only the release moves the arc.
* **One tick of release moves the landing 30–60 m.** The landing scan (finish
  gate swept over 320 ground cells, 27 candidate tapes) put the near misses at
  cells (44,33,10), (43,33,12), (44,33,8) — i.e. one cell short of the finish,
  and spread over four cells of z. The window is a couple of ticks wide, which
  is exactly what the human failed eleven times.

## A clean negative worth having: the respawn does NOT canonicalise

A TM2020 respawn restores **the run's own CP2 crossing state**, not a canonical
one. Grafting the WR's last respawn plus its winning 10.531 s tail onto the WR's
own prefix finishes exactly on the arithmetic (5590 → 65951, +200 ms per +20
packets, identity control 147031). Grafting the same tail onto **p3** or **p1**:
0 / 31 and 0 / 31, and 0 / 124 with a tail time-shift added. So a respawn cannot
be used to make one tape's tail portable to another line. New tool `blevcat`
does the cross-tape join.

## Also measured, not needed in the end

* **The wedge is a state collapse.** 47.000 → 51.750 s the car is pinned at
  x = 959.83 ± 0.01, y = 210.96 ± 0.02, under 4 km/h, steer −127, gas on,
  sliding only in z (577.86 → 578.88, decelerating). It sits 2 s before CP2 and
  the approach to it wastes nine seconds (x = 1040 at 198 km/h → eighty metres
  in nine seconds).
* A 2144-cut two-parameter sweep through that crawl reproduces the prior agent's
  `p1` exactly (k=4770, j=5250 → 49107 on `map_seg2` ✔) and finds a better one,
  **k=4710, j=5310 → 48404**, 703 ms better than `p1`. 119 of 2144 cuts reach
  CP2; none finishes. Not on the winning path, but it is the best cut-only
  prefix known and it is banked.

## Tools added (all Rust, in `/tmp/tmtas-blev`, sources banked here)

| tool | what |
|---|---|
| `tmmaps freeblocks / blocks / bchunks` | read chunk `0x0304305F`; every block with flags and world cell; the body's skippable chunk map |
| `tmmaps moveblock` | float-precision position/rotation patch of a FREE block (+ optional model rename) |
| `tmmaps moveblockcell` | cell patch of a cell-placed block — this is the rung generator |
| `blevcat` | cross-tape packet join `A[0,k) ++ B[j,end)`, with `--mid` and `--pad` |
| `blevwedge` | three-phase "brake into the corner" candidate builder |
| `blevpatch` | Factory-speed sweep of a throttle/brake/steer window over a fixed tape |
