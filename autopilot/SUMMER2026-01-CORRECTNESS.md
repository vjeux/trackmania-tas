# Summer 2026 - 01 correctness investigation

Date: 2026-08-24
Node: `110999.od.fbinfra.net`
Map UID: `buNzfsVlp2NF2oWtHM3729dEylg`
Map SHA-256: `17cfebb228862cc4fe3d6cbb81c80d59e5f87bca9bafb5c2aa5e4136a10d145b`

## Raw waypoint decode

`tools/tmmaps` decoded the untouched map directly. `--yoff -40` only converts grid cells to world coordinates; item positions are stored as floats in the map.

| source | class | tag | raw placement | orientation | world position |
|---|---|---|---|---|---|
| block 617 | RoadTechCheckpoint | Checkpoint | cell (42,6,34) | dir 2, gate-axis yaw 0 | (1360,8,1104) |
| block 670 | RoadTechFinish | Goal | cell (42,6,21) | dir 2, gate-axis yaw 0 | (1360,8,688) |
| block 681 | RoadTechStart | Spawn | cell (49,7,24) | dir 0, gate-axis yaw 0 | (1584,16,784) |
| block 683 | RoadTechCheckpoint | Checkpoint | cell (38,6,30) | dir 1, gate-axis yaw 1.570796 | (1232,8,976) |
| item 1056 | GateCheckpointLeft32m | Checkpoint | absolute item position | yaw 1.570796 | (1154,10,1328) |

MapPack chooses `(1584,16,784)` mechanically: decode all waypoint records, retain `tag == "Spawn"`, take the sole matching record (block 681), and convert a grid record with `x=32*cx+16`, `y=8*cy+yoff`, `z=32*cz+16`. The map-only/pack geometry estimator reports `yoff=-40`.

The byte-level source table is in `waypoints.txt`.

## Start-block causal controls

`tools/tmmaps origin` replayed all 1,708 movable placements at their own coordinates: zero failures. Rewriting block 681 to its own `(49,7,24)/dir0` produced a byte-identical file (same SHA-256 as the untouched map). Moving only block 681 to `(47,7,24)/dir0`, exactly 64 m west, changed exactly one byte in the decompressed map body, attributed by `bodydiff` to `block#681 RoadTechStart cell`.

The self-contained finishing input below validates at `36.618`, `ValidatedResult.NbCheckpoints=4` on the untouched map. Against every tested start relocation (including the CP1 neighbourhood and the finish straight, all four orientations), the same input becomes `DNF`, measured cps 0. This establishes that the `RoadTechStart` record is causally consumed by validation rather than being decorative.

An exact coordinate-delta trace is not yet claimed. The existing blind locator returned the same `(1360,10,~1108)` object for both the untouched and west-shifted maps, then failed its own quaternion check (`p99.5 |q|-1 = 4.66e-2`). Those CSVs are banked only as a negative control. They prove why the apparent checkpoint start cannot be used as car state.

## Prefix recovery and the `cps 3` contradiction

The original four-column search tape omits its frame. A fresh-process sweep kept the tape and horizon fixed and changed only the prefix:

| prefix | 0 | 60 | 74 | 100 | 150 | 152 | 153 | 154 | 155 | 200 | 300 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| measured cps | 0 | 0 | 0 | 2 | 1 | 1 | **3** | 1 | 1 | 0 | 0 |

Therefore prefix 153 is recovered by measurement, not inferred from a filename. The `cps 3` value is also real: the raw server `Desc` at prefix 153 is `wrong simu, but reached some checkpoints (3 out of 2)`. The 24 x 120 m trajectory is not the source of that count; it came from the independently refuted blind memory locator.

## Fresh plain-oracle finish

`summer01-finish-36618.artifact.tsv` carries the full 9,000-tick input array and explicit `prefix=153`, map UID/hash, declaration, input hash, and rebuilt-container hash. A fresh process on this node produced:

- byte-identical rebuilt container SHA-256 `fa0b9e6648d3de2b...`
- `ValidatedResult.Time = 36618`
- `ValidatedResult.NbCheckpoints = 4`
- `ValidatedResult.NbRespawns = 0`
- map UID `buNzfsVlp2NF2oWtHM3729dEylg`
- `DeclaredResult.Time = 41800`, `DeclaredResult.NbCheckpoints = 2`

The deliberate mismatch demonstrates that the reported finish and cps are measured fields, not authored fields. The verbatim server transcript is `fresh-replay-36618.transcript.txt`.

## Code fixes in this branch

- `Waypoint` now exposes block direction and yaw; `tmmaps waypoints --yoff` prints source index, class, tag, raw placement, orientation, and derived world position.
- Ladder output labels and prints `measured_cps` from `ValidatedResult`/`Desc`; it never presents `DeclaredResult` as measured.
- The exact observed bare-`wrong simu` transcript with `DeclaredResult.NbCheckpoints=4` is a regression test in both the plain and fork parsers.
- `dropscan` no longer accepts a sample hundreds of metres away as its positive control. It requires an actual tick-zero sample and <=1 m horizontal error, and exits nonzero on failure.
- A one-point horizon no longer claims the car began on top of gates; it reports the range as insufficient.

## Remaining identity work

The production trace must resolve from the validator/player ownership chain to the vehicle state and then pass left/right mirror controls. No candidate-ranking heuristic is accepted. Until that path is integrated, the banked blind-locator CSVs remain negative evidence only.
