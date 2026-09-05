# Block info reader — REPORT

Deliverable: `/tmp/tiny-bi/blockinfo.patch` (`diff -ruN` against
`/home/vjeux/trackmania-tas-tiny/tools`, seven files under `mapgeom/src`:
`lib.rs`, `node.rs`, `classes.rs`, `geom.rs`, `main.rs`, new `blockinfo.rs`,
new `blockmap.rs`). `patch -p3` dry-runs clean on a fresh copy of the
checkout's `mapgeom/`. Rust only; builds with
`~/.cargo/bin/cargo build --release -p mapgeom` with no new warnings.

The checkout moved while I worked (`tiny_assets.rs`, `tmmaps/Cargo.toml`,
`Cargo.lock`, 14:32–14:38); none of those are mine and none are in the patch.

## What it does

```
mapgeom --pak BlueBay.pak:KEY --pak current-Stadium.pak:KEY \
    blockinfo <logical>...                  # one file, fully typed, to stdout
    blockinfo-all [<substring>] [--out TSV] # every block info in the packs, parse status
    blockinfo-map <Map.Gbx> --out TSV [--report TSV] [--no-baked] [--collection BlueBay]
```

`dump` and `model` also work on block-info files now (`model` draws the base
ground variant's prefabs).

Typed structs (`mapgeom::blockinfo`): `BlockInfo { kind, name, waypoint_type,
variant_base_ground/air, additional_ground/air: Vec<Variant>, clip:
Option<ClipInfo>, is_pillar, symmetrical_block_info_id, ... }`,
`Variant { name, cardinal_dir, symmetrical_variant_index, block_units:
Vec<BlockUnit>, mobils: Vec<Vec<Mobil>>, spawn_loc, placed_pillars,
auto_terrains, ... }`, `BlockUnit { offset, clips: [Vec<path>; 6]
(N,E,S,W,Top,Bottom), terrain_modifier_id, ... }`, `Mobil { prefab, solid,
translation, rotation, ... }`, `ClipInfo { clip_type, asym_clip_id,
clip_group_id, symmetrical_clip_group_id, is_full_free_clip, ... }`. Every
external node reference is resolved to its logical path through the file's
reference table (`Graph::external`).

## Verification

* **All 10 006 `GameCtnBlockInfo` entries in BlueBay + Stadium paks: 9 998
  parse to the last byte with no recovery scan** (`blockinfo-all`, 4.6 s).
  The 8 failures are the hash-named non-GBX files (`B1A3…`, `61CF…`, `816F…`,
  `C1D9…`, classes 0x03346000/0x03348000/0x03356000 — not block infos, not
  even GBX containers). One skippable chunk is stepped over everywhere it
  appears: `0x0900501A` (CPlugSolid lod normal map, 20 bytes; GBX.NET marks
  it "ignore") on the 26 `GateSpecial*` files.
* **Summer 2026 - 01: all 46 authored models + every clip they (and the
  baked blocks) reference — 102 files — parse to the end**
  (`/tmp/tiny-bi/summer01-parse-report.tsv`, 102 × OK).
* **RoadTechStraight** names `Stadium\Media\Prefab\RoadTech\Straight_Air.Prefab.Gbx`
  for both variants; unit clips N/S `RoadTechFC`, bottom
  `TrackToDecoStraightFCBGround` (ground) and `TrackWallStraightFCB` +
  `StructurePillarToFlatACB` (air); the air variant's placed pillar is
  `StructurePillar` — exactly the six entries `mapgeom refs` lists.
  (`/tmp/tiny-bi/example-RoadTechStraight.txt`; also `example-Land.txt`,
  `example-LandHill3.txt`.)
* **Cross-check against the game itself.** The map's baked chunk
  (0x03043048) contains the clip blocks the game generated. On Summer 01,
  758 clips hang on unit-sides of authored blocks; the reader's rule says 413
  connect to a matching clip across and 345 are drawn. **Of the 345 drawn, the
  game has a baked clip block of that very name in that very cell for 316;
  of the 413 connected, the game baked 0.** Residue (29) below.

## Layout notes (where GBX.NET's chunkl was not enough)

Read off the bytes with `MAPGEOM_TRACE=1 mapgeom dump … --body F`:

| chunk | finding |
|---|---|
| `0x0304E023` | the two base variants are DIRECT nodes: no index word, no class id, the `0x0315B002` chunk starts right away; ground then air. Given fresh slots past the declared table. |
| `0x0304E02B` | GBX.NET throws on v0; TM2020 files carry `version, int BaseType` — read as such. |
| `0x03122003` v23 | GBX.NET's transcription is right including the byte-sized `U09` (=3), `U13/U14` vec3, `U15` float, `U16` = 6 `CPlugPlacementPatch` nodes (class 0x09160000, body = one `CPlugRoadChunk` 0x000 chunk v12), `U17` node. |
| `0x0303600C` v2 | clip counts packed 3 bits per side (N,E,S,W,Top,Bottom), then the refs, then two u16. |
| `0x0303600D` | GBX.NET "version, data": version word + length-prefixed byte blob (0 bytes on most files, 11 on `DecoCliff…SlopeBaseCornerIn`). |
| `0x0315B00C` | GBX.NET throws if U01 > 0; never seen > 0 in 9 998 files. Still refused if it happens. |
| `CPlugSolid 0x09005017` v3 | chunkl's indentation misleads: the `filetime` at the end applies to v3 too (8 bytes). PreLightGen = version, int, float, bool, 8 floats, int2, box[], v1+ uvgroup[]. |
| `CPlugSolid 0x09005019` v5 | GBX.NET stops at v3; the 26 `GateSpecial*` files carry 8 more bytes (`0x00019312, 0xFFFFFFFF`) before `0x0900501A`. Consumed as such, meaning unknown. |
| `CPlugTree 0x0904F006/00D/011/016/01A`, `CPlugMediaClipList 0x09189000`, `CGamePodiumInfo 0x03168000`, `CGameCtnSolidDecals`, `CGameCtnAutoTerrain`, `CGameCtnZoneGenealogy` | per chunkl, verified by every file reaching its FACADE and its end. |

## Semantics found in the data (converter-relevant)

1. **Flags.** `variant = flags & 63`, `subvariant = (flags >> 6) & 63`,
   ground bit 12, clip 13, pillar 14, skinnable 15, replacement 16, decal 17,
   waypoint 20, bit 21, ghost 28, free 29 (GBX.NET `CGameCtnBlock`).
2. **The map's variant index selects a MOBIL LIST of the base variant, not an
   additional variant.** `Beach.EDFrontier` has no additional variants and 14
   mobil lists; the map stores Beach with variant 0/5/9 and subvariant 0/1 —
   `mobils[5] = [DeadendA, DeadendB]`, subvariant 1 = DeadendB. Same for every
   `Land*`/`LandHill*`/transition (12–14 lists). The **additional** variants
   (`NPB`, `InPillar`, the 22 `StructurePillar` shapes) are not addressed by
   the flags; `blockinfo-map` uses the base variant and writes the count into
   `additional_ground/air` + `notes` so the ambiguity is visible (affects 63
   of 2 430 placements: pillars, supports, stands, beach trees).
3. **Sides.** Unit-local **North is +z, East is −x** (RoadTechCurve1's clips
   are N+E and its road-chunk path runs from the x=0 edge to the z=32 edge;
   RoadTechStraight's N/S path runs z=0..32). Rotation is `place.rs`'s:
   local side `s` faces world side `(s + dir) % 4`; cell offsets turn by
   dir=1 `(D-1-z, x)`, dir=2 `(W-1-x, D-1-z)`, dir=3 `(z, W-1-x)`.
4. **Clip connection rule** (from the EDClip metadata, validated above): two
   facing clips connect — and neither is drawn — when they are the same
   file, share a non-empty `ClipGroupId`, one's `SymmetricalClipGroupId` is
   the other's `ClipGroupId` (`…FCT`↔`…FCB`, `…FCLeft`↔`…FCRight`), or one
   names the other as `ASymmetricalClipId`. Evaluated per clip: a side with
   `[X_FCB, StructurePillarToFlatACB]` above a pillar draws `X_FCB` while the
   `ACB` connects. A **pillar's** top/bottom clips are drawn only into empty
   cells (130 pillar-on-pillar/road cells, zero baked clips).
5. **Where a drawn clip goes**: the neighbour cell (side clips), the cell
   above/below (FCT/FCB), as a block of the clip's name with the parent's
   dir; the game stores these as baked blocks, so for a map that has them the
   baked chunk is the authoritative answer and the TSV prints it beside the
   prediction (`game-placed … [agrees|DIFFERS]`).
6. **Two packs, two answers.** `RoadTechStraight` exists in both paks and
   differs (BlueBay: `TrackToDecoStraightFCBGround`, zone `VoidToLand`;
   Stadium: `TrackToGrass…`, `VoidToGrass`). The map's baked clips name the
   BlueBay one, so `--collection BlueBay` is the default; the other candidate
   is in the `alt_blockinfo` column.
7. `StructurePillarFCBGround`: ground variant has no units and an empty mobil
   list; the geometry is the air variant's prefab with GeomTranslation
   (0, 8, 0). `pick_placement` falls back to the variant that has content.

## `blockinfo-map` TSV columns

`index kind(grid|free|baked) name blockinfo alt_blockinfo cell_x cell_y cell_z
dir flags ground variant subvariant variant_picked variant_name mobil_list
mobil_lists additional_ground additional_air notes cells prefabs North East
South West Top Bottom error`. Side cells: `u<unit>->(x,y,z) FREE|<occupants>
clips <Clip> DRAW|CONNECTED(<who>)=><clip prefab> [game-placed <baked clip>
[agrees|DIFFERS]]`, one entry per unit, joined by ` ; `. Sides are WORLD sides.
Output for Summer 01: `/tmp/tiny-bi/summer01-blockinfo.tsv` (4 645 rows:
2 430 authored + 2 214 baked).

## Not verified / residue

* The 29 clips the rule says DRAW but the game did not bake as such:
  `StructurePillarToFlatACB`/`…ACB` under `PlatformBase`/`DecoPlatform*`
  (attach clips: drawn only when there is something to attach to, it seems),
  `StructurePillarFCBSimple` under `StructureSupportCurve3*` (over a
  `TechnicsScreen4x1Curve3In` or free), and 4 free `StructureSupportCurve3In`
  bottoms where the game put a `TechnicsScreenFC*` instead. Not modelled; the
  TSV shows both sides of each case.
* Which ADDITIONAL variant (`NPB`, `InPillar`, pillar shapes) the game uses is
  decided at load time from surroundings; not derivable from the map record.
  Bit 21 ("hill ground variant" in GBX.NET) appears on `StandStraight`,
  `StructureSupportCurve3In`, `StructurePillar` here and may be that switch —
  unverified.
* `0x09005019` v5 trailing 8 bytes and `0x0303600D`'s blob: consumed, not
  interpreted.
* Meanings of `CGameCtnBlockInfoVariant` `0x004` (i16), `0x00D` (2 ints),
  `CGameCtnBlockUnitInfo 0x006` (9 words), Mobil `U09/U13/U14/U15`: read and
  kept raw.
* Clip prefab per side is taken from the clip's own base variant (ground when
  the parent is ground, else air, with the content fallback); the game may
  pick the clip's variant by other rules — the baked clip blocks carry
  `flags = 0` (air) even for `…Ground` clips.
* `Sea.EDFlat` is referenced only by baked blocks; parsed fine, not on the
  46-model list.

## Files

* `/tmp/tiny-bi/blockinfo.patch` — the change.
* `/tmp/tiny-bi/summer01-blockinfo.tsv`, `summer01-parse-report.tsv` — map walk.
* `/tmp/tiny-bi/all-blockinfo.tsv` — parse status of all 10 006 entries.
* `/tmp/tiny-bi/example-*.txt` — `blockinfo` output for the example inputs.
* GBX.NET sources used: `/tmp/gbxnet/*.chunkl|cs` plus the full tree
  extracted to `/tmp/tiny-bi/gbxnet-src/` (CPlugRoadChunk, CPlugSolid,
  CPlugTree, CGameCtnCollector, CGameCtnBlock flags).
