# `CSceneVehicleVisState` — the engine struct, from the engine's own reflection

Read out of the dedicated server `TrackmaniaServer_Latest`
(30 113 288 B, `date=2026-05-15_18_00 git=128182-0de74ece09e`, md5
`0f0f4b25f31f80c60c81404366c95e68`) at **`0x9d2ea0`**, the function that
registers class **`0x0A00C000`** with the script engine. Each row is one
`addMember(name, byteOffset)` call; the names are the game's, not ours, and
they are the names [`next.openplanet.dev/Scene/CSceneVehicleVisState`] lists.
Raw evidence: `raw/vvs_calls.txt`.

`sizeof == 0x360 == 864`, which is the stride of the array of copies
`CARRIER.md` measured and the `u01` field of a ghost's `EntRecordDesc` for
class `0x0A018000`.

**`Loc.translation` is at `0x50`, so `CARRIER.md`'s `car` anchor is
`state + 0x50`**: every `car+N` in that document is `state + 0x50 + N`.

| offset | car+ | member | note |
|---|---|---|---|
| 0x0a | -70 | `DiscontinuityCount` | |
| 0x10 | -64 | `InputSteer` | range [-1, 1] |
| 0x14 | -60 | `InputGasPedal` | [0, 1] |
| 0x18 | -56 | `InputBrakePedal` | [0, 1] |
| 0x1c | -52 | `InputVertical` | [-1, 1] |
| 0x20 | -48 | `InputIsBraking` | |
| 0x2c | -36 | `Loc` rotation | 3 rows of 3 f32; `Left` = (0x2c, 0x38, 0x44), `Up` = (0x30, 0x3c, 0x48), `Dir` = (0x34, 0x40, 0x4c) |
| **0x50** | **0** | **`Loc.translation`** (script name `Position`) | THE ANCHOR |
| 0x5c | 12 | `WorldVel` | |
| 0x68 | 24 | *(unnamed vec3)* | recorded, see sample bytes 69-72 |
| 0x74 | 36 | `FrontSpeed` | |
| 0x78 | 40 | *(unnamed — the lateral speed)* | |
| 0x88 | 56 | *(the flag word)* | see below |
| 0xa8 + 44k | 88 + 44k | `Wheels.Elems[k]` | k = 0..3, front-left first |
| … +0x00 | | `.DamperLength` | [0, 2] |
| … +0x04 | | `.Rot` | [0, 2π] |
| … +0x08 | | `.RotSpeed` | |
| … +0x0c | | `.SteerAngle` | [-0.5235988, 0.5235988] |
| … +0x10 | | ground contact material | a byte |
| … +0x14 | | `.SlipCoef` | [0, 1] |
| … +0x18 | | *(unnamed — the "dirt" slot)* | recorded at sample 93/95/97/99 |
| … +0x1c | | `.Icing01` | [0, 1] |
| … +0x20 | | `.TireWear01` | [0, 1] |
| … +0x24 | | `.BreakNormedCoef` | [0, 1] |
| … +0x28 | | *(the wheel's flag word)* | bit 1 = no contact, bit 2 = ? |
| 0x158 | 264 | *(unnamed)* | over [0, 2π] at sample byte 74 |
| 0x170 | 288 | *(unnamed, 3 bits)* | sample byte 90 |
| **0x174** | **292** | **`ReactorBoostLvl`** | enum `ESceneVehicleVisReactorBoostLvl`, 3 values |
| **0x178** | **296** | **`ReactorBoostType`** | enum `ESceneVehicleVisReactorBoostType`, 4 values |
| **0x180** | **304** | **`ReactorAirControl`** | a vec3 (0x180, 0x184, 0x188) |
| 0x18c | 316 | `WorldCarUp` | |
| 0x198 | 328 | *(unnamed — rpm)* | not reflected; the sample calls it over [0, 30000] |
| 0x19c | 332 | *(unnamed 3-bit enum)* | sample byte 31 bits 0-2 |
| 0x1a4 | 340 | `CurGear` | |
| 0x1ac | 348 | `TurboTime` | |
| 0x1b4 | 356 | `RaceStartTime` | |
| 0x1dc | 396 | `CamGrpStates` | |
| 0x218 | 456 | `GroundDist` | [-1, 15] |
| 0x224 | 468 | *(unnamed)* | sample byte 34 — **not populated by the server** |
| 0x228 | 472 | *(unnamed, [-1, 1])* | sample byte 19 — not populated by the server |
| 0x22c | 476 | *(unnamed, [-1, 1])* | sample byte 20 — not populated by the server |
| 0x230 | 480 | `SimulationTimeCoef` | |
| 0x234 | 484 | `BulletTimeNormed` | [0, 1] |
| 0x238 | 488 | `AirBrakeNormed` | [0, 1] |
| 0x23c | 492 | `SpoilerOpenNormed` | [0, 1] |
| 0x240 | 496 | `WingsOpenNormed` | [0, 1] |
| 0x314 | 708 | `WaterImmersionCoef` | |
| 0x318 | 712 | `WaterOverDistNormed` | |
| 0x31c | 716 | `WaterOverSurfacePos` | |
| **0x328** | **728** | **`WetnessValue01`** | sample byte 101 |

## The flag word at 0x88 (car+56)

The reflection registers seven members as accessors rather than offsets; each
accessor is a three-instruction thunk that reads one bit of the u32 at `0x88`.
Every one of them is used by the sample writer.

| bit | member | where it lands in the sample |
|---|---|---|
| 4 | `IsTopContact` | byte 76 bit 5 |
| 5 | `IsWheelsBurning` | byte 33 bit 7 |
| 6 | *(unnamed)* | byte 76 bit 7 |
| 7 | *(unnamed)* | byte 76 bit 3 |
| 8 | *(unnamed)* | byte 76 bit 2 |
| 9 | *(unnamed)* | byte 76 bit 1 |
| 10 | *(unnamed)* | byte 76 bit 0 |
| 12 | *(unnamed)* | byte 45 |
| 17 | *(unnamed)* | byte 76 bit 6 |
| **18** | **`ReactorInputsX`** | **byte 89 bit 2** |
| **19** | **`IsReactorGroundMode`** | **byte 89 bit 1** |
| 20 | `IsGroundContact` | byte 89 bit 0 |
| 23 | `EngineOn` | not recorded |
| 24 | `IsTurbo` | byte 31 bit 7 |

`GroundContactMaterial` is an accessor too (`0x9de9e0`): it returns **13** when
the wheel's flag bit 1 is set, **80** when its flag bit 2 is clear, and the raw
byte at wheel+0x10 otherwise. **The sample writer has only the first of those
two special cases** — see `SAMPLE-LAYOUT.md`.
