# Collision: `CPlugSurface` (`0x0900C000`), the physics and gameplay ids

What the car feels. Reader/writer `tools/mapgeom/src/static_item/surface.rs`
(byte-exact), the crystal-era reader in `classes.rs`, the enum in
`tools/mapgeom/src/scene.rs`, physics-per-link tables in
`static_item/materials.rs`, the census tools `mapgeom surf`, `surfhist`,
`triggers`, `collhash`. Confidence **[FILE]** for layouts, **[COMMUNITY]** for
the enum names (`next.openplanet.dev/MetaNotPersistent/GmSurfaceIds`, build
2026-01-26), **[VERIFIED-GAME]** for every behaviour in §5.

## 1. Chunk `0x0900C003`

```text
u32 version
[v≥2] u32 surfVersion
GmSurf surf                                   (§2; the archive version is surfVersion, or 1 when version == 1)
Vec3 gameplayMainDir                          (a turbo pad's +z)
SurfMaterial[] materials  { bool32 isNode; node ref (CPlugMaterial) | i16 id }
[u05: i32, version/material dependent]
[v<3] i32[] u01
u16[] materialIds                             ← the ID TABLE: entry = physics | gameplay << 8
[v≥1] ref skel
[v≥5] Id[] u06
0xFACADE01
```

## 2. `GmSurf` shapes (`Surf::type_id`)

| id | shape | payload |
|---|---|---|
| 0 | Sphere | `f32 size`, `i16 surfaceIndex` |
| 1 | Ellipsoid | `f32[3]`, `i16` |
| 6 | Box | `f32[6]` (centre, half extents — axis aligned, no rotation), `i16` |
| 7 | Mesh | `u32 version (6/7)`, `Vec3[] vertices`, `Triangle[]` |
| 10 | ConvexPolyhedron | `u32 version, u32 u01, Vec3 centre, Vec3 half, Vec3[] vertices, u32[] indices, (u32 start, u32 count)[] faces, i16 surfaceIndex` — a pusher's `MoveShape` (20 vertices, 18 faces) |
| 13 | Compound | `Surf[]` children with `Iso4[]` placements and optional `i16[]` joints — a rotor's move shape is a compound of nine; block waypoint triggers are compounds |

A `GmSurfMeshTri` is `u32[3] indices, u8 physicsId, u8 gameplayId, i16
surfaceIndex` (the index into `materialIds`). `Surf::triangulate` flattens any
shape to triangles for a bake (a box as 12 triangles, a sphere as a 16×8 UV
sphere); a compound trigger that is NOT flattened makes the bake fall back to
the unit box and a finish fires on its whole 32 m cube.

## 3. `EPlugSurfaceMaterialId` — the physics ids (`scene.rs::physics_name`)

**The list this replaced was right to 25 and wrong from 26 on**: it called 27
`RoadIce` (27 is `Bumper_Deprecated`, RoadIce is **74**) and 28 `Bumper` (28
is **NotCollidable**), so triangles the car cannot touch counted as surface
under it (`tools/mapgeom/MAPGEOM.md` §4.6).

| id | name | id | name | id | name |
|---|---|---|---|---|---|
| 0 | Concrete | 27 | Bumper_Deprecated | 54 | Energy |
| 1 | Pavement | 28 | **NotCollidable** | 55 | TechMagnetic |
| 2 | Grass | 29 | FreeWheeling_Deprecated | 56 | TurboTechMagnetic_Deprecated |
| 3 | Ice | 30 | TurboRoulette_Deprecated | 57 | Turbo2TechMagnetic_Deprecated |
| 4 | Metal | 31 | WallJump | 58 | TurboWood_Deprecated |
| 5 | Sand | 32 | MetalTrans | 59 | Turbo2Wood_Deprecated |
| 6 | Dirt | 33 | Stone | 60 | FreeWheelingTechMagnetic_Deprecated |
| 7 | Turbo_Deprecated | 34 | Player | 61 | FreeWheelingWood_Deprecated |
| 8 | DirtRoad | 35 | Trunk | 62 | TechSuperMagnetic |
| 9 | Rubber | 36 | TechLaser | 63 | TechNucleus |
| 10 | SlidingRubber | 37 | SlidingWood | 64 | TechMagneticAccel |
| 11 | Test | 38 | PlayerOnly | 65 | MetalFence |
| 12 | Rock | 39 | Tech | 66 | TechGravityChange |
| 13 | **Water** | 40 | TechArmor | 67 | TechGravityReset |
| 14 | Wood | 41 | TechSafe | 68 | RubberBand |
| 15 | Danger | 42 | OffZone | 69 | Gravel |
| 16 | Asphalt | 43 | Bullet | 70 | Hack_NoGrip_Deprecated |
| 17 | WetDirtRoad | 44 | TechHook | 71 | Bumper2_Deprecated |
| 18 | WetAsphalt | 45 | TechGround | 72 | NoSteering_Deprecated |
| 19 | WetPavement | 46 | TechWall | 73 | NoBrakes_Deprecated |
| 20 | WetGrass | 47 | TechArrow | 74 | **RoadIce** |
| 21 | Snow | 48 | TechHook2 | 75 | RoadSynthetic |
| 22 | ResonantMetal | 49 | Forest | 76 | Green |
| 23 | GolfBall | 50 | Wheat | 77 | **Plastic** |
| 24 | GolfWall | 51 | TechTarget | 78 | DevDebug |
| 25 | GolfGround | 52 | PavementStair | 79 | Free3 |
| 26 | Turbo2_Deprecated | 53 | TechTeleport | 80 | XXX_Null (= no contact in the wheel readout) |

`is_collidable` excludes `NotCollidable` and `OffZone` from the probe index —
on the 33-map corpus that changed nothing (no sample rests on either; worth
knowing and not the same as not checking).

## 4. Gameplay ids (the high byte of an id-table entry)

1 Turbo, 2 Turbo2, 3 TurboRoulette, 4 NoEngine (FreeWheeling), 5 NoGrip, 6
NoSteering, 7 ForceAcceleration, 8 Reset, 9 SlowMotion, 10 Bumper, 11
Bumper2, 12 ReactorBoost, 13 Fragile, 14 ReactorBoost2, 15 Bouncy, 16
NoBrake, 17 Cruise, 18 Boost (ReactorBoost_Oriented), 19 Boost2
(ReactorBoost2_Oriented) (`tools/mapgeom/src/main.rs`).

Where a gate's ids come from: the kind's `Modifier\<Kind>\Collision.Material.Gbx`
chunk `0x09079017 = { u32 version 1, [u8 physics, u8 gameplay, u8, u8 0x80],
f32, u32, string }` — Boost `00 12` (Concrete, 18), NoEngine `00 04`, Reset
`00 08`, Turbo2 `00 02`, Boost\CollisionGrass `(76, 12)`, Boost\CollisionDirt
`(6, 12)`, Fragile 13, NoBrake 16; `RoadTech`'s chunk is `10 00` (Asphalt,
none); Turbo has no Collision file and the prefab slab's own `(0, 1)` stands.
Older material files carry `0x0907900E = { u16 physics, u16 }` instead
(`materials.md` §3). Hull id-table entries seen: Reset `[2048]` (= 8 << 8),
grass Boost `[3148]` (= 76 | 12 << 8), Boost 4608, NoEngine 1024, Turbo 256,
Turbo2 512, Fragile road 3328, NoBrake road 4096, (RoadIce 74, Reset 8) 2122.
**Same geometry, only the gameplay id differs**: a pad with table entry 256
peaked at 63.2 m/s (Turbo), with 512 at 95.7 m/s (Turbo2) against the
original's 120.4.

The physics id of a game material link, harvested from the 26 reference items
(`MATERIAL_PHYSICS`): `RoadTech 16`, `PlatformTech 16`, `TrackWall 14`,
`TrackWallClips 22`, `TrackBorders 9`, `Technics*/RaceArchFinish/Speedometer
4`, `DecoHill 2`, `ChronoFinish/LightSpot/RaceAd6x1/RaceScreenStart/Sign/
SignOff 32`, `Modifier\PlatformGrass\PlatformTech 76`, `PlatformDirt\PlatformTech
6`, `PlatformIce\PlatformTech 74`, `PlatformIce\DecoHill 21`; `Decal*`,
`SpecialFX*`, `RaceTriggerFX*` → 28; the `.Material.Gbx` files themselves
carry NONE for these (their surface id reads 0).

## 5. What the engines do with an id **[VERIFIED-GAME]** (client and dedicated server unless stated)

* **Water 13 on an ITEM plate is NOTHING to the car**: it falls through with
  no deceleration (vy −12.7 → −13.5 m/s across the plane) and rests on the
  floor; on both engines. **NotCollidable 28 is a SOLID** on both (a nose-down
  car at 25 m/s stops at the plane; a spawned car rests at plane + 0.0–0.04 m)
  — "Water → 28" turned every water plate into a lid. Physics 28 does not
  make HULL triangles pass-through either (the item editor strips them at
  compile). The engine's water is a BLOCK VOLUME (`blockinfo-…` §6): inside it
  every wheel reports material 13 whatever the floor is, the car sinks to the
  floor and drives underwater with strong drag (original 05: 31.7 → 22.5 m/s
  over 33 m in a 3 m body, ≈ −8 m/s²); **the car does not float** — every
  "0.9 m draft" reading was a 1 m-deep `WaterBase` floor. No floor physics id
  reproduces water drag ("no pack id decelerates a car under throttle at
  30 m/s").
* `WaterRampZone` prefabs carry BOTH a Plastic (77) deck and a Water (13)
  surface; the car rides the deck with FX. RoadWater decks are `Underwater`
  (Plastic 77) at the plane.
* Wheel readout (Openplanet `VehicleState::ViewingPlayerState()` →
  `CSceneVehicleVisState.FL/FR/RL/RRGroundContactMaterial`): 16 Asphalt, 9
  Rubber, 2 Grass, 80 = no contact. A tiny road IS Asphalt: identical
  acceleration to the original over 8 s (1.5 → 17.3 vs 1.6 → 17.6 m/s).
  `RoadTech\Straight_Air`'s hull: 256 Asphalt triangles = the whole deck
  (851 m² up) + 640 small Rubber (lips, bumper walls); the TrackBorders
  sub-prefab 3 840 Rubber (154 m² up) — **use area, not count**. Wheel
  material 0 on a turbo pad is normal (Concrete 0 + Turbo 1).
* Pack floor ids: Dirt/Grass/Sand/Snow cap speed and acceleration, Ice and
  Plastic lower grip; grass under a deck capped a road at 50 km/h where the
  original does 280 (the tile-under-unit rule, `map-blocks.md` §8).
* Look and physics are ONE material file: a clip filler beside a `Platform*`
  block wears `Modifier\Platform*\TrackWall` → physics 0 Concrete where
  `Material\TrackWall` = 14 Wood; `TrackWall*InWorld` (the terrain
  collections' StadiumOnTerrain skin) = Concrete, `TrackBorders*InWorld` keep
  Rubber; modifier FOLDERS shadow materials by NAME (`PlatformIceBase` →
  RoadIce 74) — ice/dirt/grass/snow platforms drove as Asphalt before that
  was applied (2026-09-08). Re-dressing a `Deco` slope (DecoHillPy_D, physics
  2) as PlatformTech made grass slopes drive as asphalt.
* Ghost-mode blocks (flag bit 28) ARE solid (the authors of 12/13/24 rest on
  ghost-mode road pieces).
* A collision surface with EMPTY triangles gets the item dropped; the bake
  keeps a 1 mm `NotCollidable` sentinel 4 m under the origin for an item with
  no collision.
* Gravity is about **−24.6 m/s²** (−24.3…−24.9 on several maps); 9.81 appears
  nowhere in the game (`tools/pkz2/src/gravity.rs`, `mapgeom coverage`).

## 6. `mapgeom collhash` — the physics fingerprint

One FNV-1a hash over exactly the physics-bearing bytes: every placement (model,
position, yaw/pitch/roll, pivot, scale, waypoint tag; floats quantised 1e-4 m
/ 1e-6 rad), every embedded item's collision surfaces (vertices, triangles,
per-triangle physics id, the id table), waypoint type, spawn iso, trigger
shape, and the map's authored + baked BLOCKS. Not hashed: visuals, materials,
lights, LOD, skins, MediaTracker, name, uid, lightmap. A lap is valid only for
the exact surface it was driven on: the fp1 rebuild moved the car 1 cm at
2.15 s and 48 m by 6 s.

## 7. Not known

* `u05`, `u01`, `u06`, `skel` semantics; whether `surfaceIndex` on the
  primitive shapes is honoured as a table index by the engine (the bake maps
  it through the table like a mesh triangle's byte).
* The physics of ids 31–79 beyond their names (never measured here).
