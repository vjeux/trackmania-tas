# Block infos: `CGameCtnBlockInfo*` — how a block model describes itself

The game's description of a BLOCK MODEL (`*.EDClassic.Gbx` etc. in the packs):
which prefabs it draws per variant, which cells it occupies, which CLIP block
infos it hangs on each free side, where its spawn and trigger are, what water
volume it carries. Reader `tools/mapgeom/src/blockinfo.rs` (typed; 9 998 of
10 006 `GameCtnBlockInfo` entries of the BlueBay + Stadium paks parse to the
last byte), placement logic `blockmap.rs`, the engine's clip algorithm
`bake.rs`, reports `tools/tmmaps/tiny/re/blockinfo-reader-REPORT.md`,
`tools/mapgeom/MAPGEOM.md` §3. Confidence **[FILE]** for layouts (GBX.NET's
`.chunkl` checked byte by byte against `RoadTechStraight.EDClassic.Gbx`),
**[EXE]** for §7, **[VERIFIED-GAME]** where marked.

## 1. Families, class ids, folders

| class | family | folder / extension | what |
|---|---|---|---|
| `0x0304E000` | CGameCtnBlockInfo (base) | | |
| `0x03051000` | Classic | `GameCtnBlockInfoClassic\*.EDClassic.Gbx` | roads, walls, platforms |
| `0x03051000` under `GameCtnBlockInfoPillar\` | Pillar | `.EDClassic.Gbx` | the supports under everything (103 Stadium entries; `is_pillar`) |
| `0x0304F000` | Flat | `.EDFlat.Gbx` | the terrain sheet — Stadium `Grass`; the other environments `Water`, `Lake`, `Sea`, `Dirt`, `Land` |
| `0x03050000` | Frontier | `.EDFrontier.Gbx` | cliffs and hills |
| `0x0314C000` | Transition | `.EDTransition.Gbx` | the joins |
| `0x03053000` | Clip | `.EDClip.Gbx` | the pieces hung on free sides |
| `0x0335B000` / `0x03340000` | ClipHorizontal / ClipVertical | `.EDHorizontalClip.Gbx` / `.EDVerticalClip.Gbx` | |
| `0x03055000`, `0x03052000`, `0x03054000`, `0x03056000` | Pylon, Road, Slope, RectAsym | | |
| `0x0315B000` / `0x0315C000` / `0x0315D000` | CGameCtnBlockInfoVariant / …Ground / …Air | inline | |
| `0x03122000` | CGameCtnBlockInfoMobil | inline | one drawn piece |
| `0x03036000` | CGameCtnBlockUnitInfo | inline | one cell of the footprint |
| `0x03120000` / `0x0311D000` | CGameCtnAutoTerrain / CGameCtnZoneGenealogy | inline | |
| `0x03121000`, `0x09128000`, `0x09160000` | CGameCtnSolidDecals, CPlugRoadChunk, CPlugPlacementPatch | inline | |
| `0x0303B000` | CGameCtnDecorationSize | | no reader; almost certainly carries the base height (`yoff`) |

Classic and Pillar each carry a `Theme\` subfolder with 122 more models
(`SnowRoadStraight`, `RallyCastleRoadStraight`). Looking only in Classic left
125 537 placements of the 33-map corpus with no geometry (92 619 of them
`DecoWallBasePillar`). A name is resolved Classic before Pillar before Clip
(`BlockInfoIndex::paths_for`); on the 25 Summer sources no name has two
candidate files of different kinds.

## 2. `CGameCtnBlockInfo` chunks

| chunk | content |
|---|---|
| `0x0304E009` | `bool32 isPillar` |
| `0x0304E00F` | `bool32 noRespawn` (object +0x230; the `GateCheckpoint` block info is true — NoRespawn is a BLOCK feature) |
| `0x0304E013` | `bool32 iconAutoUseGround` |
| `0x0304E015` | `ref`, `Iso4` |
| `0x0304E017` | `bool32` |
| `0x0304E020` | `u32 v; ref charPhySpecialProperty; [v<6 ref]; [v≥2 ref podiumInfo]; [v≥3 ref introInfo]; [v≥4 bool32]; [v==5 bool32]; [v≥8 && bool32: string, string]` — the `MatModifier` placement TAG (`("MatModifier", "Grass"\|"Dirt")`) |
| `0x0304E023` | the two BASE variants written DIRECTLY (no index, no class id): ground (`0x0315C000`) then air (`0x0315D000`) — the payload opens with chunk `0x0315B002` |
| `0x0304E026` | `i32 waypointType` (Start 0, Finish 1, Checkpoint 2, None 3, StartFinish 4, Dispenser 5; object +0x150) |
| `0x0304E027` / `0x0304E02C` | `ref[]` additional ground / air variants (selected by record flag bits 21..27) |
| `0x0304E028` | `Id symmetricalBlockInfoId, i32 dir` |
| `0x0304E029` | `ref fogVolumeBox` |
| `0x0304E02A` | `u32 v; ref sound1, ref sound2; Iso4s` (per version) |
| `0x0304E02B` | `u32 v; i32 baseType` (GBX.NET throws on v0; the files carry one int at every version) |
| `0x0304E02E` | `u32 v; i32 prodState` |
| `0x0304E02F` | `u32 v; u8 isPillar; u8 pillarShapeMultiDir; [v≥1 u8]` |
| `0x0304E031` | `u32 v; ref[3] materialModifier`: slot 0 = the `EDClassic` parent info it copies, slot 1 = the modifier file (class `0x0915D000`, `materials.md` §3), slot 2 (v≥1) |
| clip-only `0x03053002/4/5/6/7/8`, `0x0335B000`, `0x03340000` | §6 |
| `0x03050000` | Frontier flag |
| `0x0900501A` (skippable, 20 B) | `CPlugSolid` lod normal map, stepped over on the 26 `GateSpecial*` files |

## 3. `CGameCtnBlockInfoVariant` chunks

| chunk | content |
|---|---|
| `0x0315B002` | `i32 multiDir` (SameDir 0, SymmetricalDirs 1, AllDir 2, OpposedDirOnly 3, PerpendicularDirsOnly 4, NextDirOnly 5, PreviousDirOnly 6) |
| `0x0315B003` | `u32 v; i32 symmetricalVariantIndex; cardinalDir (i32, or u8 + u8 variantBaseType [+ u8 noPillarBelowIndex])` |
| `0x0315B004` | `u16` |
| `0x0315B005` | `u32 v; u32 n; n × MOBIL LIST (ref[])`; then helper refs (`helperSolid`, `facultativeHelperSolid`, [i32]) — the lists are indexed by record flag bits 0..5, the mobil inside by bits 6..11 |
| `0x0315B006` | `u32 v; [refs]; ref screenInteractionTriggerSolid; ref waypointTriggerSolid; [v≥11: ref[2] triggerShapes]; [i32]; ref gate; ref teleporter; ref captureZone; ref turbine; ref flockModel (+ FlockEmitterState); ref spawnModel; ref[] entitySpawners` — `triggerShapes` names e.g. `Gate\Checkpoint_Trigger.Shape.Gbx` (a 36-vertex disc) or `RoadTech\Checkpoint_Trigger.Shape.Gbx` (a 0.1 m PLANE at mid-block, x 3..29, y 1.84..8.28, z 15.95..16.05; the finish plane at z 3 = the block entry; RoadDirt from y 0.46; PlatformTech 2.0..7.84) — **a real trigger plane, not the unit volume** (the unit-box trigger fired half a block early and from the side) |
| `0x0315B007` | `u32 v; ref probe` |
| `0x0315B008` | `u32 v; ref[] blockUnits; i32; bool32[4] manualSymmetry; spawnLoc (Vec3 + 2 f32, or a 6-f32 box); string name` — `spawn_loc`: Granady's start `[8, 1, 8]`, `GateCheckpoint` ground `[16, 1.7, 11.2]`, the `WithPillar Air` variant has none |
| `0x0315B009` | placed pillars `(ref, i32[4])[]`, replaced pillars `(ref, i32[4], u8)[]` |
| `0x0315B00A` | `u32 v; refs; [Iso4]; ref compoundModel` |
| `0x0315B00B` | **water volumes**: `u32 v; u32 n; n × { [i32;6][] cellBoxes (x0,y0,z0,x1,y1,z1 in block units), u32[7] words, [v≥1] Id id ("Shallow") }` — §8 |
| `0x0315B00C` | `u32 v; i32` (GBX.NET throws when ≠ 0) |
| `0x0315B00D` | `i32[2]` |
| `0x0315C001` (ground) | `u32 v; ref[] autoTerrains; i32 autoTerrainHeightOffset; i32 autoTerrainPlaceType` |

## 4. `CGameCtnBlockUnitInfo` (`0x03036000`) and `CGameCtnBlockInfoMobil` (`0x03122000`)

Unit chunks: `0x03036000` (`i32 placePylons, bool, bool, Int3 offset, the flat
clip list`), `0x03036001` (`Id surface` — `"Water"` on the zone water blocks,
`i32 frontier`, `i32 dir`), `0x03036002` (`bool32 underground`), `0x03036004`
(`i32 acceptPylons`), `0x03036005` (`Id terrainModifierId`), `0x03036006`
(`i32[9]`), `0x03036007` (`i32[4] pylons`), `0x03036008`, `0x0303600B` (`ref
bottomClip, ref topClip, i32 bottomClipDir, i32 topClipDir`), `0x0303600C`
(`u32 v ≥ 1: six clip lists North, East, South, West, Top, Bottom; two trailing
words (v≥2 u16 each) = the top/bottom clips' 2-bit directions`), `0x0303600D`
(raw data). Cell rotation of a unit by `dir` is in `map-blocks.md` §2.

Mobil chunks: `0x03122002` (`ref[] solidDecals, i32`), `0x03122003` (v23:
`ref solidFid | ref prefabFid`, `Vec3 geomTranslation`, `Vec3 geomRotation`
(degrees), `ref oldSolidAggreg`, `ref railPath`, `ref[] roadChunks`, `ref vfxs`,
`u8`, floats…), `0x03122004` (`(Id, ref)[] dynaLinks`). The 4 BlueBay
`Road{Bump,Dirt,Ice,Tech}OnLandHillSlopeBase2x1` author their ground variant as
"OnLandHill 180°": translation `(32, 0, 64)` + rotation `(0, 180, 0)` —
translation alone misplaced a slope one cell east and two north.
`CPlugRoadChunk` "road chunks (33,33)/(2,2)" are edge POINT COUNTS, not
physics.

## 5. From a block name to triangles

```text
CGameCtnBlockInfoClassic (.EDClassic.Gbx)
  reference table names its .Prefab.Gbx files (under hashed names, pak-nadeopak.md §3)
    CPlugPrefab → entities with a quaternion and a position
      CPlugStaticObjectModel → CPlugSurface (collision, physics per face) / CPlugSolid2Model (visual)
```

Variant pick: the record's ground bit picks ground/air; bits 21..27 an
additional variant; bits 0..5 the mobil list; bits 6..11 the mobil. Against the
editor's own list of Summer 20 (7287 records) the pick matches 7280/7281 clip
records. A clip whose asked-for variant is PRESENT but EMPTY draws NOTHING
(`WaterShore1_Rocky_FCLeft` as an air clip); an ABSENT family still falls back.
Blocks with NO prefab in any variant: `DecoWallBasePillar` (its walls are its
unit's four side CLIP refs), `DecoCliffMidCornerOut`, the `DecoWall*`/
`PlatformBase` families — their faces ARE the generated FC fillers.

## 6. Clips: the pieces on free sides

Clip block info fields: `clipType` (ClassicClip 0, FreeClipSide 1, FreeClipTop
2, FreeClipBottom 3), `isFullFreeClip`, `isExclusiveFreeClip`,
`canBeDeletedByFullFreeClip`, `topBottomMultiDir`, `0x03053006` v2..4 bytes
(`IsAlwaysVisibleFreeClip`, `IsFCTOrFCBIgnoredByVFC`, `IsAntiClip`;
`WaterRampZoneStraightHFC` is always-visible), `asymClipId`, `passingPoint`,
`clipGroupId` / `symmetricalClipGroupId` (Left↔Right) / `clipGroupIdsV1` (a
REMOVE pair, e.g. `("PlatformFCSmallClipsRemove", "PlatformFCSmallClips")`),
`horizontalClipGroupId` (`"WaterHFC-Clips"`), `verticalClipGroupId` (every
`DecoWall*VFC` panel = `"DecoWallBaseVFC"`). FC pieces stand on the shared
face and OVERHANG INTO THE PARENT's footprint by design (`PlatformSpecialFCRight`
spans z 15.96..17.57 of a 16 m cell); FC end caps are 2 m tall; the yellow
"FC" dead-end caps and the blue checkpoint humps are EDITOR HELPERS
(`FC_Air_Helper.Prefab`, `Checkpoint_Helper.Prefab`, material
`Effects\Media\Material\EditorHelpers`), absent in play. Filler prefabs really
are plates: `Platform\Base_FCT`/`Base_FCB` 32×32 m ResonantMetal,
`TrackWall\Straight_FCB` 29×32 m, `DecoWallWater\Base_FCT` a 32×32 m WATER
plane (drop it and the channel is dry).

## 7. The engine's free-clip bake **[EXE]** (`bake.rs`; `Trackmania.exe`, anchored on the profiler strings)

At load the client REGENERATES every free clip from the authored blocks' unit
faces (`CGameCtnChallenge::InitChallengeData_Clips` `0x140b91af7` →
`0x140f373a0`; `0x140f37110` allocates a 184-byte clip block) with the same
code the editor used (`CreateFreeClips` `0x140f36416`,
`InitChallengeData_FreeClipsBaked` `0x140b8e460`) — **the file's BakedBlocks
are exactly what is drawn**; a record only donates attributes (variant
alternate, lightmap id, colour) when the grid holds one for that owner face:

```text
for each B in the clips registered at (cell + step(face), opposite(face)):
    if !ClipsConnect(A, B) continue                 // 0x140d23610
    if IsFreeClipDeletedBy(A, B) deletedA = true    // 0x140d236e0
    if IsFreeClipDeletedBy(B, A) remove(B)          // anti-clip: instantiate
keep(A) = A.IsAntiClip ? deletedA : !deletedA
IsFreeClip(c) = c.ClipType != 0                                          // 0x140d23600
ClipsConnect(a,b) = both free and both non-exclusive → true; else grounds equal, keys equal, Match either way
IsFreeClipDeletedBy(a,b): both free; groundA == groundB; key(a) == key(b); a.IsAlwaysVisibleFreeClip → NOT
    deleted; b.IsFullFreeClip && a.CanBeDeletedByFullFreeClip → deleted; else Top(2) needs b Bottom(3) and
    Bottom(3) needs b Top(2) with DirsCompatible(dirA, dirB, a.TopBottomMultiDir) both ways (0x140f42200,
    jump table = GBX.NET EMultiDir order); a Side clip needs dirA == opposite(dirB); then Match(a,b)
Match(a,b) = a's symmetrical group ids ∩ b's group ids if a has any; else a's group ids ∩ b's; else
    a.SymmetricalClipId == b.name; else same name        // 0x140d23820 (id2 ignored when id1 unset)
Opposite: 0↔2, 1↔3, 4↔5                                                  // 0x140f40dd0
```

The ground bit of a clip block is its owner block's (record flag bit 12 —
copied from the file when a record exists; from terrain otherwise, and NOT
computable from the pack); the direction of a side clip block is the face it
hangs on, of a top/bottom clip block `(owner dir + the clip's 2-bit dir from
`0x0303600C`) mod 4`. `mapgeom bake MAP --diff`: 0 stale records on all 25
Summer 2026 maps (46 304 records). **What the runtime HIDES of that list is
nothing by any record property**: a filler is invisible in the original only
when the neighbouring block's MESH covers it (a night of probes,
`TINY.md` "Which generated fillers the game draws"). A BLOCK deck always wins
a coplanar z-fight over a baked clip in the game.

## 8. Water volumes **[VERIFIED-GAME]**

`WaterBase` "Shallow" 32 × 1 × 32 m at +4…+6 (band relative to the block origin
+3..+7: a car at origin +3.5 reads Water, at +0.5 nothing), `DecoWallWaterBase`
"Shallow" 32 × 8 × 32 and NO prefab (the volume IS the block; band −1..+7;
carries a COLLIDABLE ResonantMetal clip cap at origin +8), `PlatformWaterBase`
32 × 2 × 32, `RoadWaterStraight` a 1 × 2 × 1 box per road unit; `WaterWallBase`
and `WaterRampZone*` have none. The engine renders the surface, the underwater
tint, the overflow lips and sheet, gives the DRAG and overrides the wheel
material to Water inside the volume (`collision-cplugsurface.md` §5). No item
carries a volume; an embedded CUSTOM BLOCK (§9) instantiates its archetype's
volume at the block's own position, full 32 m size.

## 9. Custom blocks: `CGameBlockItem` (`.Block.Gbx`) **[VERIFIED-GAME]**

The TMX custom-block form: a `CGameItemModel` with block-info chunks whose
`0x2E00201E` names an ARCHETYPE stock block (`archetype_ref`) it borrows its
block info from. What it takes to embed one (`tools/tmmaps/TINY.md` "Water,
solved for the tiny"; `mapgeom waterblocks`, `blockitem-archetype`,
`vstream-shift`, `rename_ident`, `tmmaps addblock`):

1. a block file re-pointed at the archetype (every length-prefixed occurrence
   of the archetype string; header chunk sizes and header size grow with the
   delta — the body strings sit in non-skippable chunks) and RENAMED to the
   manifest ident (header + body): the game pairs an embedded block with its
   manifest row by the FILE'S OWN IDENT, not the archive path — unrenamed, the
   records are silently dropped;
2. archive entry `Blocks/Water/<Archetype>.Block.Gbx` + manifest row
   (`Water\<Archetype>.Block.Gbx`, collection, the file's author uid); block
   record `Water\<Archetype>.Block.Gbx_CustomBlock`, flags `0x10208000 | FREE
   (0x20000000)`, author = the manifest author, null skin ref, one `0x0304305F`
   entry (position + zero rotation); FREE and grid placement both work;
3. `CGameBlockItem` version 1 hides its geometry in a SECOND table: a NULL node
   in its variant list, then a byte saying the table is present, per variant a
   byte of flags, then whichever of mesh / collision hull / box / offset it
   claims — reading that byte as a 32-bit word stepped two bytes late and lost
   210218's 83 embedded wood platforms (`MAPGEOM.md` §5);
4. a game-made block item embedded as an ITEM crashed the game (2026-09-04);
   as a custom BLOCK it works. A block's mesh-collidable byte follows the whole
   mesh body (a shape ref would have to be inserted to clear it — the deck was
   moved −200 m instead).

## 10. Not known

* `CGameCtnDecorationSize` (`0x0303B000`): no reader; it is the missing
  source of `yoff` (`MAPGEOM.md` §7.8).
* Most `u0xx` fields of the variant/unit/mobil chunks; `CPlugPlacementPatch`.
* Whether the engine reads `0x0315B00B` from an EMBEDDED block file itself, or
  only from the archetype (the archetype's volume is what was measured).
