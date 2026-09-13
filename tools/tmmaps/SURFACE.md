# Surface swap: a map's blocks re-surfaced (`tinyctl surface`, 2026-09-13)

vjeux: "input 'ice' and it'll swap all the road, dirt… blocks into their icy
version. If one doesn't exist it'll create a custom item version." The first
25 U10S maps (Everios96) as "Ice U10S", campaign 156122 in club 43788.

## The pipeline

```text
tinyctl surface 01 02 … --src-dir DIR --out-root /tmp/surface-ice [--surface ice]
                --campaign 156122 --campaign-name "Ice U10S" [--no-shoot] [--no-publish]
                [--reuse-build] [--views-only start,finish,top] [--lod-pick 0|-]
tinyctl surface-report --out-root /tmp/surface-ice [--out REPORT.md]
```

Per map, four tools in order (each one usable on its own):

1. **`mapgeom surface-plan MAP --surface ice --out models.tsv --records records.tsv`**
   (`mapgeom/src/surface.rs`): every authored block MODEL gets a verdict, then
   every RECORD. The candidate twin comes from the name (table below), and is
   a twin only when the pack has it, it agrees on kind / waypoint type /
   footprint (base ground and air variants), the record's variant bits resolve
   in it the way they resolve in the source (ground/air, additional variant,
   mobil list, mobil), and its base prefab is the SAME MESH (same file, or the
   same file name in another per-surface folder with the deck at the same
   height — the platform specials). Verdicts: `rename`, `same` (already that
   surface), `keep` (pillars, structures, stands, gates, canopies, decoration —
   not a surface), `bake` (a surface block with geometry of its own and no
   same-shape twin), `none` (prefab-less and twinless: `DecoWallWaterBase`).
2. **`mapgeom --lod-pick 0 tiny-library MAP --scale 1 --only <bake names>`** with
   the surface dress in the environment: `TINY_FORCE_MODIFIER=Stadium\Media\
   Modifier\PlatformIce.TerrainModifier.Gbx` (every baked block wears the ice
   platform modifier folder too: PlatformTech → ice 74, TrackWall → the ice
   wall dress (Concrete 0), DecoHill → Snow 21, Deco / DecoGrass /
   OpenTechBorders / Penalty / DecalPlatform / DecoCliff), `TINY_REDRESS=
   RoadTech=Stadium\Media\Material\RoadIce;RoadDirt=…;RoadBump=…` (the road
   decks, which no modifier folder shadows: look AND collision physics from
   the replacement material, RoadIce = 74). (`TINY_KEEP_WATER=0` would drop the Water sheets; the driver keeps them: a RoadWater
   piece baked as ice carries no water sheet). Full-size static-item copies
   of the block's own prefab — the tiny converter's bake at scale 1.
3. **`tmmaps surface MAP --plan records.tsv --out F --name "Ice <name>" --uid-prefix
   Ice2 [--library lib.zip --mapping placements.tsv]`** (`tmmaps/src/surface.rs`):
   stage 1 deletes the baked blocks (and any pillar a shifted twin would
   swallow), stage 2 renames (the Id table re-encoded: the body is re-emitted
   — the game loads a re-emitted body; MAPS.md's refused rename named a model
   no library has), stage 3 moves the shifted records and writes the uid
   (`Ice2` + the source uid's tail, same byte length), stage 3b appends the
   items (clones of an existing record; model = author = the library ident,
   the map's collection, the block's origin corner and quarter turn — a free
   block its position and full rotation —, scale 1, the block's waypoint tag
   and colour byte; archive + manifest), stage 4 removes the password, sets
   the name, drops the validation ghost (an author run recorded on another
   surface), keeps the medal times. The output is reloaded and every planned
   record checked.
4. **views + shoot + publish**: `tinyctl views` (the tiny cameras: start, finish,
   top), `tinyctl shoot … --anchor a:a --scale 1` (both maps from the same
   editor cameras, side-by-side JPGs), `tinyctl publish-map … --any-uid`
   (item-check gate over the baked items, Nadeo upload, campaign playlist
   position NN-1, stored-bytes md5 readback).

## Where a Stadium block spells its surface

| family | Tech form | Ice form | rule |
|---|---|---|---|
| `Road…` | `RoadTechStraight` | `RoadIceStraight` | the word after `Road` (Tech, Dirt, Bump, Water, Ice) |
| `Platform…` | `PlatformTechBase` | `PlatformIceBase` | the word after `Platform` (Tech, Dirt, Grass, Plastic, Ice) |
| `DecoPlatform…` | `DecoPlatformBase` | `DecoPlatformIceBase` | Tech has NO word: insert / replace after `DecoPlatform` |
| `DecoHill…` | `DecoHillSlope2Straight` | `DecoHillIceSlope2Straight` | same, after `DecoHill` |
| `Open…Road` | `OpenTechRoadCurve1` | `OpenIceRoadCurve1` | the word after `Open` |
| `DecoWall…` | `DecoWallBase` | `DecoWallBaseIce` | SUFFIX (pillars excepted) |
| `Water…` | `WaterBase` | `WaterBaseIce` | SUFFIX — the pool keeps its water volume, the rim wears the PlatformIce modifier |
| `Road<A>ToRoad<B>` | `RoadTechToRoadDirt` | `RoadIceStraight` | a one-cell join of two surfaces is a straight of the one |

## Why every ROAD is a baked copy and not a rename

`RoadIce` is not a re-dressed tech road. Measured on the pack prefabs
(`mapgeom deck`, `mapgeom model`): `RoadTech\Straight_Air` is a flat deck at
y 2.0 with 1 m lips (verts 0..3); `RoadIce\Straight_Air` is a **bobsleigh
trough** — floor at y 4.5 in the middle (x 13..18), rising through 5.7 (x 6/25),
7.5 (x 4/27) to 10.8 at the edges (x 2/29), with 8 m wooden bank faces at
x 1/30 (verts 0..11), two units tall (`(0,0,0)` + `(0,1,0)`). `RoadIceStart`
is a pad at y 10 (tech 2.0); the slopes 12.5 vs 10.0. RoadDirt's deck is 0.5,
RoadBump's 2.75, RoadWater's 2.0. No cell shift makes a 2.5 m difference drop
in, a rename would raise the driving line, break every join to a platform
(2.0) and turn a start pad into an 8 m drop. So the whole Road family
(Tech / Dirt / Bump / Water) is baked: the block's own geometry, deck material
`RoadIce` (physics 74, the ice wheel material), borders kept. The same for
`TrackWall*` (no surface variants at all), the Tilt / Wave / RampLow pieces
RoadIce lacks, and the stepped joins (`RoadTechToRoadIceDown`).

Same-mesh families, renamed in place: Platform*, DecoPlatform*, DecoHill*,
Open*Road* / Open*Zone*, DecoWall*, Water* (their ice twins name the very same
prefab under another modifier folder, or — DecoWall — no prefab at all: their
faces are the generated fillers, which follow the block info).

## Measured on the first 25 U10S maps

12 675 authored records: 2 139 renamed (131 models), 108 baked (51 models),
617 already ice, 84 `none` (DecoWallWaterBase, map 10 — deep water in a deco
wall, stays water), 9 727 kept (pillars, structures, stands, canopies, gates,
screens). Editor loads of 01 (renames only), 02 (10 road items) and 03 (9 dirt
road items): every item loaded, blocks resolved, the same layout in white.
Play on Ice U10S_02 (`tinyctl play … --drive-ms 6000 --wheels-ms 11000`): the
wheel log reads material **74 (RoadIce)** under all four wheels on the ice
deck, slip flags set while accelerating; the original reads 16 (Asphalt), no
slip. The turbo pad (a baked `RoadTechSpecialTurbo`) still boosts (the
special's gameplay id rides on its collision material).

## Knobs

- `--surface dirt|grass|plastic|tech`: the same machinery (`surface_env`
  picks the Platform<S> modifier folder and the road deck material); only
  `ice` has been run end to end.
- `--lod-pick -` keeps every detail level of the baked copies (default: the
  nearest level only — the copies are few, beside the track, and every byte
  counts against the 7 MB server cap; a road piece is 0.2–1 MB deflated).
- `--strip-lightmap`: the source's stored lightmap dropped (default kept: for
  a renamed block the mesh and its uv are the same, the lightmap still fits;
  a map with baked items has a different object count and the game recomputes
  at load anyway).
- `--fresh`: restart the game before the first load (the model cache is per
  session; the alias base `NN700000` keeps the item names unique per map).
