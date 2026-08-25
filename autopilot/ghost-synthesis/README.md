# From-scratch TM2020 ghost synthesis

This project constructs a `.Ghost.Gbx` from a `.Map.Gbx`, generated inputs, and documented format/map constants. A game recording is used only as a specification and causal oracle; no final output path reads one or copies bytes from one.

## Commands

```text
ghost manifest FILE
ghost manifest validation FILE
ghost manifest diff LEFT RIGHT
ghost tape bits FILE... [--events]

tmauto synth write --map MAP --tape TAPE --out FILE \
  --declared MS --cps N --record grid

tmauto synth ladder --map MAP --tape TAPE --out DIR \
  --declared MS --checkpoints N

tmexplore-real state --route ROUTE --map MAP --template GHOST \
  --server SERVER --shim LIBFORKSHIM --checkpoint-clock N \
  [--ticks N] [--position-only] [--expect-start] \
  [--expect-distance M] [--expect-opposite]
```

`ghost manifest` emits deterministic JSON containing the GBX header, structurally framed chunks, decoded validation metadata, input-archive versions, packet-mode/payload histograms, the complete parsed `CPlugEntRecordData` grammar, and the first controlled-car sample. `ghost tape bits --events` reports only state-word literals and their times, never the donor's driving inputs.

## Structural differential: Summer 2026 - 01

The same-map inputs were a game recording and the pre-existing 36.011 synthetic artifact. The machine-readable comparison is `evidence/summer01/structural-diff.json`.

| field | game recording | old synthetic |
|---|---:|---:|
| GBX class | `0x03092000` | `0x03092000` |
| nodes | 2 | 1 |
| top-level skippable chunks | 24 | 5 |
| `0x03092000` parent / `0x0911F000` record | yes / v11 | absent / absent |
| controlled-car samples | 391 × 116 bytes | absent |
| input packet modes | mode 2 only | mode 2 only |
| mode 12 / 13 packets | 0 / 0 | 0 / 0 |
| first state literal | `0x2` | `0x2` |
| archive format / field0 / start offset | 12 / 368950 / −1.550 s | 11 / 0 / 0.000 s |

The mode-12/13 hypothesis is false. Expanding all 2,109 packets of the recording—state words and vehicle fields—to explicit encoding preserves its correct start, so packet compression is also not the seed.

## The load-bearing field: validation U03

Validation chunk `0x0309202D` contains a word previously named only `U03`; chunk `0x0309202A` repeats it. It is the index of the semantic Spawn in the engine's checkpoint array.

TM2020 orders block waypoints for this array as non-Spawn blocks first and Spawn blocks next; item waypoints follow. The writer derives the index as the count of non-Spawn block waypoints. This is map data, not donor data.

Causal sweep on Summer 2026 - 01:

| U03 | authoritative state after the same 1.520 s input prefix | selected map waypoint |
|---:|---|---|
| 0 | `(1363.291, 10.019, 1094.383)` | checkpoint block #617 |
| 1 | `(1363.291, 10.019, 686.584)` | finish block #670 |
| 2 | `(1222.381, 10.019, 972.707)` | checkpoint block #683 |
| **3** | `(1580.710, 18.019, 798.431)` | **RoadTechStart block #681** |
| 4 | `(1168.430, 10.019, 1331.289)` | checkpoint item #1056 |
| 5 | invalid state / rejected by the structural state check | out of range |

Independent game recordings carry the same derived index:

- Summer 2026 - 01: three non-Spawn block waypoints → U03 `3`.
- `tools/testdata/map2.Map.Gbx`: two → U03 `2`.
- map 191465, `Training - 10 Long`: one → U03 `1`.

Changing only U03 from `0` to `3` moves the authoritative car from the last-checkpoint area to the real Summer start. Changing the map's RoadTechStart by +64 m then moves the authoritative car by exactly +64 m. The complete writer now derives U03 automatically; the legacy writer remains byte-identical for historical artifacts.

## Authoritative state controls

All state below comes only through:

```text
validator job → simulation → controlled participant → CGameVehiclePhy → state
```

No candidate scan or fallback participates.

On a freshly generated Summer container with default archive values (format 11, field0 0, start offset 0), an early pre-race checkpoint reads:

```text
map start       1584.000000, 18.002001, 784.000000
vehicle state   1584.000000, 18.001501, 784.000000
error           0.000500 m
```

On the map whose RoadTechStart alone was moved +64 m in x:

```text
map start       1648.000000, 18.002001, 784.000000
vehicle state   1648.000000, 18.001501, 784.000000
error           0.000500 m
```

The same donor-free container shape on map 191465 derives U03 `1`. After 1.700 s of generated input, the authoritative vehicle is 22.367 m from its `(1520.000, 26.002, 816.000)` start and faces the map-derived direction-1 heading. Hard-left and hard-right branches have opposite signed lateral response (`+10.494 m` / `−7.643 m`).

A standalone 1,000-tick generated Summer artifact uses a 56-tick deterministic, zero-mean steering key followed by straight full throttle. Its pre-race state is 0.0005 m from RoadTechStart, and its authoritative straight branch reaches **137.854 m** from that start. Hard-left/right are opposite (`+10.612 m` / `−10.579 m`).

Raw outputs and generated files are under `evidence/summer01/authoritative/` and `evidence/191465/authoritative/`.

## Negative controls

The following each leave the wrong U03=0 start unchanged:

- record parent, descriptor, entity, first sample, full sample grid, and +64 m first-sample corruption;
- a +64 m first-sample edit in an otherwise game-recorded ghost;
- archive format 11 vs 12, field0 `0` vs `368950`, and all combinations;
- validation seeds `0`, `1`, `32611514`, and `u32::MAX`;
- declared time, declared checkpoint count, race-time chunk, and result chunk;
- account id length/content;
- the observed `0x404` state-word pulses;
- every missing skippable chunk and the complete donor record parent.

Replacing only validation chunk `0x0309202D` made the car map-start-sensitive; a one-field reconstruction isolated U03. Every other validation field remained inert in the authoritative state control.

## Record encoder provenance

| emitted field | provenance |
|---|---|
| GBX/node/chunk ids, v11 record grammar, columnar deltas | public GBX.NET schema plus this repository's byte-identical record round-trip |
| vehicle descriptor `0x0A018000`, vocabulary 864, schema 33 | stable TM2020 format constants seen across recordings |
| one 116-byte sample at t=0 / 50 ms grid | TM2020 `CSceneVehicleVis` sample schema |
| x/z | centre of the map's semantic `RoadTechStart` grid cell |
| y | decoration-specific vertical origin plus RoadTechStart local spawn height `2.002 m` |
| yaw/quaternion | map four-way grid direction; local +Z is forward |
| validation start index (U03) | count of non-Spawn block waypoints, verified by a five-value causal sweep and three independent recordings |
| velocity | chosen zero vector |
| steer/gas/brake echo | generated tape tick 0 |
| side-speed/suspension/slip/ground/gear/time coefficient | documented neutral constants; every other unnamed sample byte is explicit zero |
| parent metadata | public `CGameCtnGhost` v9 schema and anonymous constants |

No byte slice is copied from a recording.

## Ablation ladder

Each `record-ladder/` directory contains every generated `.Ghost.Gbx`, deterministic manifest, `report.tsv`, and raw server stdout/stderr. The ladder adds: no record; parent only; descriptor; empty car entity; first sample; full 50 ms grid; deliberately corrupted first-sample x (+64 m).

The record is render telemetry, not the validator's initial-physics state. Start selection is controlled by validation U03 whether the record is absent or complete.

## Remaining work

Correct start, moved-start sensitivity, two-map direction, left/right response, and 100 m are proven. Two five-minute CP1 searches made 447,323 fork evaluations; their best plain-oracle-confirmed tapes reached route stations 39 (780 m) and 25 (500 m) but both remained `cps0`, so no CP1 claim is made. Their full logs and best tapes are in `evidence/summer01/cp1-search/`. CP1 and finish have not yet been achieved from the corrected start; the historical 36.011 finish began at the last checkpoint and is not a success for this project. Client import/render remains untested.
