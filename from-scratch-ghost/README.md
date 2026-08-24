# From-scratch TM2020 `.Ghost.Gbx`

This project constructs a ghost from a `.Map.Gbx`, generated inputs, and named format/map constants. A game-recorded ghost is permitted only as a specification oracle during reverse engineering; no final output path reads one.

## Commands

```text
ghost manifest FILE
ghost manifest diff LEFT RIGHT

tmauto synth write --map MAP --tape TAPE --out FILE \
  --declared MS --cps N --record sample

tmauto synth ladder --map MAP --tape TAPE --out DIR \
  --declared MS --checkpoints N
```

`ghost manifest` emits deterministic JSON containing the GBX header, structurally framed chunks, input archive versions, exact packet-mode histograms and payload shapes, the complete parsed `CPlugEntRecordData` grammar, and the first controlled-car sample (raw and decoded). The diff form embeds both manifests and an equality verdict; no shell parser is part of the comparison.

## Structural differential: Summer 2026 - 01

The same-map inputs were:

- game recording: `tools/testdata/decoder-goldens/ghosts/p00001_19538.Ghost.Gbx`;
- current synthetic 36.011, rebuilt byte-identically from `summer01-finish-36011.artifact.tsv`.

The machine-readable result is `evidence/summer01/structural-diff.json`.

| field | game recording | old synthetic |
|---|---:|---:|
| GBX class | `0x03092000` | `0x03092000` |
| nodes | 2 | 1 |
| top-level skippable chunks | 24 | 5 |
| `0x03092000` parent chunk | yes | no |
| `0x0911F000` record | v11 | absent |
| descriptors | 7 | absent |
| vehicle descriptor | class `0x0A018000`, vocabulary 864, schema 33 | absent |
| entities | 3 | absent |
| controlled car | 391 samples × 116 bytes, first at t=0 | absent |
| notices | 82 | absent |
| input packet modes | mode 2 only | mode 2 only |
| mode 12 / 13 packets | 0 / 0 | 0 / 0 |
| first state literal | `0x2` | `0x2` |
| input archive start offset | −1.550 s | 0.000 s |

The normal same-map recording therefore refutes the leading mode-12/13 hypothesis: neither mode appears. Its first state literal is also the synthetic writer's literal `0x2`.

## Field provenance in the new record

| emitted field | provenance |
|---|---|
| GBX/node/chunk ids, v11 record grammar, columnar deltas | public GBX.NET schema plus this repository's byte-identical record round-trip |
| vehicle descriptor `0x0A018000`, vocabulary 864, schema 33 | stable TM2020 format constants seen across recordings |
| one 116-byte sample at t=0 | TM2020 `CSceneVehicleVis` sample schema |
| x/z | centre of the map's semantic `RoadTechStart` grid cell |
| y | decoration-specific vertical origin plus the RoadTechStart model's 2.002 m local spawn height |
| yaw/quaternion | the map's four-way grid direction; local +Z is forward |
| velocity | chosen zero vector |
| steer/gas/brake echo | generated tape tick 0 |
| side-speed/suspension/slip/ground/gear/time coefficient | documented neutral constants; every other unnamed sample byte is explicit zero |
| parent metadata | public `CGameCtnGhost` v9 schema, `CarSport`/Stadium/Nadeo constants, anonymous `TAS` identity |

No byte slice is copied from a recording.

## Ablation ladder

`evidence/summer01/record-ladder/` contains every generated `.Ghost.Gbx`, its manifest, `report.tsv`, and raw server stdout/stderr.

The ladder adds, in order: no record; parent only; descriptor; empty car entity; first sample; a full 50 ms sample grid; deliberately corrupted first-sample x (+64 m). All seven are parsed and simulated. With the 36.011 tape all seven reproduce **36.011 / cps 4** on the stock map.

The +64 m first-sample corruption also reproduces **36.011 / cps 4**. A stronger control edits only the first sample of an otherwise game-recorded 19.538 ghost by +64 m; both original and edited files simulate to **19.538 / cps 4**. Therefore `CPlugEntRecordData` is render telemetry, not the validator's initial-physics state. This is a causal negative with a positive control, not an inference from the synthetic file.

The validation seed *is* causal: the same 36.011 tape gives 36.011 at seed 0, DNF/cps2 at seed 1, and DNF/cps0 at seed 32611514. It does not restore moved-start sensitivity by itself.

## Current boundary

The complete record is structurally valid and independently parsed, but it has not yet produced a correct-start validator state. The authoritative validator-job → simulation → participant → vehicle → state resolver is required for the next rung. Until that reader is integrated, accepted rows remain classified `start_unmeasured`; no inference from `IsValid` or a finish is promoted to a start-coordinate claim.

## Cross-map structural control

Summer 2026 - 01 matches its independent recording to **0.000136 m / 0.000000000 rad**. Map 191465 (`Training - 10 Long`) is deliberately unlike it:
its `RoadTechStart` is waypoint 0 instead of waypoint 2 and has direction 1
instead of direction 0. The map-only writer produces `(1520.000, 26.002,
816.000)` and yaw −90°, versus `(1584.000, 18.002, 784.000)` and yaw 0° on
Summer 2026 - 01. The 191465 position/orientation agree with an independent
game-recorded tick-0 sample to **0.000136 m / 0.000098826 rad**; that recording
is used only as a test oracle, never as output input. `evidence/191465/`
contains the two start checks and generated straight/left/right containers,
manifests, ladder, and raw server transcript. The plain server parses and simulates all three
(DNF/cps0 at the deliberately short 10.000 s horizon); signed response still
awaits the authoritative live-state resolver.
