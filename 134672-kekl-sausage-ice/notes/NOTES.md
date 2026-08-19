# NOTES — map 134672 `KEKL- SAUSAGE ICE` (working log)

Node 10003.od.fbinfra.net · scratch /tmp/m134672 · started 2026-08-18 19:03 PT

## Acquisition (banked: `acq-v1.tgz`, md5 3555eb7a66d31c0249bb7127741c04bb)

* uid `agH9XtjTZd8iZbuGp_KhC16jMO7` (not in `unbeaten_maps.json` for this map —
  the record has no `ubisoftId`; the uid was read out of the `.Map.Gbx` header
  after fetching the file by `OnlineMapId` from Nadeo).
* map: Nadeo CDN copy == TMX copy, sha256
  `1cc10011a9882145333afcfc4acf2b85e20548e0ec035ccfcfd7e85e9010b703`,
  2 313 037 B. TMX: one version, `UploadedAt == UpdatedAt`, comment
  **"Built in 15mins for KEKL"**, EmbeddedItemsSize 1 024 133.
* 15 ghosts (the whole leaderboard), all downloaded `.part`+rename, GBX-verified.
* **No embedded author ghost.** `tmtraj decode map.Map.Gbx` → no
  `CPlugEntRecordData`; a full skip-chunk scan of the decompressed body finds no
  `0x0911F000`, no `0x0309201D`, no `CGameCtnGhost` string. (Answering the
  sibling agent's §9 tip: it does not apply to this map.)

## §8 field reproduction: 5/15, perfectly explained by game build

| build | ranks | reproduce |
|---|---|---|
| 2022-07-06 git 113150 | 1,3,4,5,6,7,8,9,12,15 | 0/10 |
| 2025-07-04 / 2026-01-18 / 2026-02-02 | 2,10,11,13,14 | **5/5 exact** |

Ruled out: truncated files; decoder/format (all fv=12, and every tape is
time-aligned with its own telemetry); start offset (both conventions on both
sides); edited map; respawns.

## The map is violently chaotic — measured

One steer unit (1/127) changed on one 10 ms tick of rank02's tape, at tick 200,
300, 1000, 1500 or 2000 → **5 of 5 DNF**. Sensitivity is strongly
time-dependent: the search's own mutations, which mostly land late, finish 34 %
of the time.

That is why the 2022 ghosts cannot re-simulate, and it is not evidence against
the oracle: the oracle tracks rank02's own telemetry to **rms 0.008 m over the
whole 68 s run** (measured by `fk traj`'s state locator).

## The map

317 blocks: ~252 **TrackWall*Pillar** (the narrow elevated "sausage"), 41
`FlinkIceBlocks\3-1-*-Ice-Light` custom blocks, 3 `RoadIceStraight`, 2
`RoadIceWithWallCurve3`, 1 **GateSpecialTurbo**, 4 GateCheckpoint, 1 GateFinish,
RoadBumpStart. Everything sits at ground level (y ≈ 50–61 m).

Waypoints (world centres):

| | block | cell | world | note |
|---|---|---|---|---|
| spawn | 311 RoadBumpStart | 27,14,20 | 880,·,656 | |
| CP1 | 165 | 24,13,15 | 784,·,496 | 32 m past the **turbo gate** at 752,·,496 |
| CP2 | 170 | 16,13,14 | 528,·,464 | |
| CP3 | 243 | 7,13,17 | 240,·,560 | |
| CP4 | 261 | 15,14,24 | 496,·,784 | |
| finish | 244 | 17,13,23 | 560,·,752 | 8 m BELOW CP4; crossed airborne |

**`tmmaps build` derived the checkpoint order WRONG** (243,165,170,261 instead
of 165,170,243,261), so its `map_seg2/3/4` are all really a CP4 gate. seg1 and
seg2..4 are still exact, they just measure CP1 and CP4.

## Field analysis

Sector table, spread and correlation: see `field.txt`. Every sector correlates
0.61–0.89 with the final time — a field where the fast drivers are faster
everywhere, not one separated by a single feature.

**Best-sector recombination over the whole field = 63263 ms, still 4576 ms
slower than the AT.**

Grip: mean |lateral velocity| 13.8–23.2 m/s (the whole map is a permanent
drift), and it is *monotone in pace* — the WR has the highest (23.2), the last
place the lowest (13.8). Airborne 3.4–6.1 %. Throttle 78–92 %, brake 0.5–12 %.

### Steering saturation (answering the 285268 sibling's question)

From the TAPES, not the telemetry:

| class | n | mean lock% | corr(lock%, finish time) |
|---|---|---|---|
| all | 15 | 66.4 | **−0.40** |
| pure keyboard (3 values) | 8 | 73.1 | **−0.77** |
| pad (127–254 values) | 7 | 58.1 | **−0.47** |

**On this map more full lock goes with a faster time, not less** — the opposite
of the 285268/279209 finding, in both device classes. 8 of 15 records are pure
keyboard `{−127,0,+127}` including the top 3 (63546, 68442, 69522); their median
steer hold is 170–290 ms, against 10–20 ms for the pad runs. So the ice rule
"don't ask the surface for maximum" does not generalise to this map's corners at
the human level.

## The gate ladder (new tooling, this map)

`tmmaps gateladder MAP --cells x,y,z;... --keepcp N --cporder ... --dir D`
parks every checkpoint off the track (renamed to a finish so it is not required,
moved to cells 1..4,9,1) and relocates the real Goal block to each requested
cell. Needed `MapFile::set_block_cell` / `set_block_dir` (new: blocks now carry
`coord_off`). **Verified exact**: a gate at CP2's cell returns 33106 for rank02
and 36146 for rank10 — each run's own declared CP2 split, to the millisecond;
CP3's cell returns 45437 / 49728. Orientation matters: `dir` 1/3 fires for
gates whose crossing is along x, 0/2 for along z; generate both and take the one
that fires.

This turns "DNF" into "reached cell (x,z) at t", at 32 m resolution, entirely
through the plain oracle. It is the instrument this map needed.

## Where the 2022 WR dies

Gate ladder over sector 1, rank01 (63546, 2022, pure keyboard):

| cell | rank01 gate time | rank01's own recorded crossing |
|---|---|---|
| 27,21 | 1915 | ~1900 ✓ |
| 27,22 | 2924 | ~2900 ✓ |
| 23,20 | 7434 | ~7400 ✓ |
| 23,19 | 7969 | ~7900 ✓ |
| 23,18 | 8871 | ~8700 ✓ |
| 22,17 | 12207 | ~9560 ✗ |
| 22,16 | 37435 | ~10600 ✗ |

**The 2022 world record re-simulates exactly for 8.9 seconds and then diverges,
at the map's one air phase** (the car is off the ground at t ≈ 8.5 s, climbing
y 52→60). Landings are the classic chaos amplifier. So the 2022 physics is not
globally different: it is the same physics plus an amplifier.

## Baselines (plain oracle, guard on)

* `rank02` (68442, best today-legal seed) → local search, 100 workers, 11 min,
  190 eval/s, 34 % finishers: **67816**.
* Repair of rank01 at its divergence (mutating ticks 800–1100 only, objective =
  time at the cell-(22,16) gate): **37435 → 11740 ms in 3 minutes**, i.e. the
  tape is back on its own recorded line to about a second.

## Running: the staged gate chain (`chain.sh`)

18 gates along the WR's route, ~3 s apart, each keeping the checkpoints already
passed as real checkpoints. At each stage the search may mutate only the ticks
between the previous gate and this one, and the objective is the time at this
gate. Everything adjudicated by the plain oracle.

## Lyapunov: how fast a one-unit steering error grows (gate ladder, plain oracle)

rank02's tape with ONE steer unit (1/127) changed on ONE tick, timed at the
sector-1 gates against the unperturbed reference:

| gate | ref | +1 unit @ tick 200 (2.0 s) | +1 unit @ tick 1000 (10.0 s) |
|---|---|---|---|
| 27,21 (1.9 s) | 1916 | 1916 | 1916 |
| 27,22 (2.9 s) | 2927 | **2927 (exact)** | 2927 |
| 23,19 (8.0 s) | 7973 | 8037 (+64) | 7973 |
| 22,17 (9.6 s) | 9634 | 15716 (lost) | 9634 |
| 22,16 (10.8 s) | 10804 | — | 10803 (−1) |
| CP1 (13.9 s) | 13906 | — | **14079 (+173)** |
| full run | 68442 | DNF | DNF |

Both perturbations are **invisible for ~1 s**, then grow by roughly a factor of
e every **0.6–0.8 s**: 1 ms at +0.8 s → 173 ms at +4 s → lost by +6 s. An input
error made 8 s before the line is amplified about 10^5 times.

That single number explains the whole map: 15 records spread over 40 s; an
author time nobody has matched in three years; and 10 of 15 ghosts failing to
re-simulate because a 2022 build differs from a 2026 build by *anything at all*
somewhere in the run. It also bounds what an open-loop TAS tape can be: ours is
exactly as fragile as theirs.

## The 2022 world record is unrecoverable — closed, with evidence

* Ladder localisation: rank01 re-simulates exactly to **8871 ms** and is lost by
  9.6 s, at the map's one air phase (car off the ground t ≈ 8.5 s, y 52→60).
* **Exhaustive single-move neighbourhood over the whole break**: every steer
  value on a 4-unit ladder at every tick in 800–980, plus every accel/brake
  flip — **11 869 candidates, 0 finished**, none reached even CP2.
* A 110-worker, 20-minute search over the same window (full-map objective,
  CP-shaped) never reached CP2 either.

So the 63546 line is not available to us, and the best today-legal seed is
rank02's 68442.

## Route: is there a cut? No.

The map's 317 blocks are ~252 TrackWall*Pillar stacked 5 cells high (y-cells
9–13) with an ice deck on top (y-cell 14) — a narrow elevated ribbon. The 199
items are scenery (54 rocks, 49 `zTrackSlopeLoopStart` at y=168, support bars,
light rails, palm trees), scattered over the whole 0–36 cell grid, not track.

Tested and refuted:
* 54 synthetic full-throttle tapes from the start (steer −127…+127 held 0.5–3 s)
  reach **no** gate in any cell neighbouring the start, nor any mid-field cell.
* rank01's diverged tape, which wanders for 30 s after it is lost, reaches
  **no** mid-field gate either — it stays inside the corridor.

The corridor is the corridor. The 4859 ms is not a shortcut.
