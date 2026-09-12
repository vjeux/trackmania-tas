# Map items: `0x03043040` (CGameCtnAnchoredObject) and the per-placement side tables

Reader/writer: `tools/tmmaps/src/map.rs` (`parse_items`, `ItemRec`, the
`set_item_*` patchers, `SnapTables`, `MacroblockRefs`), placement by
`tools/mapgeom/src/place.rs`. Confidence **[FILE]**; pivot/scale semantics
**[VERIFIED-GAME]**.

## 1. Chunk `0x03043040` (skippable)

```text
u32 version            8 on every TM2020 map read (asserted)
u32 u01
u32 sizeOfNode         = payload size − 12; the chunk carries TWO sizes (this one and the
                         PIKS size) and both must agree — rewritten on every build
u32 archiveVersion
u32 nbItems
nbItems × ItemRecord   each a "node with class id": u32 0x03101000, chunks…, 0xFACADE01
tail                   v7: Int2[] then the five int arrays; v8: the five int arrays (§3)
```

**The chunk opens its OWN lookback table**: it re-writes the version word 3 and
re-defines `Nadeo` even though the blocks chunk already defined it. Item model
ids and authors live in this private table.

### 1.1 One `CGameCtnAnchoredObject` record

```text
u32 class 0x03101000
u32 chunk 0x03101002, u32 version (8)
[u32 3]                    lookback version (first record)
Id     itemModel.id        the model name: a stock item ("GateCheckpointLeft32m", "Flag8m") or an
                           embedded file name relative to Items/ ("AC01123045.Item.Gbx")
Id     itemModel.collection   a plain collection number (26 Stadium, 0x1C BlueBay …)
Id     itemModel.author    ("Nadeo", or the embedding author)
f32    yaw, f32 pitch, f32 roll
u8[3]  blockUnitCoord      the cell (as is, no +1)
Id     anchorTreeId
Vec3   absolutePositionInMap
node   waypointSpecialProperty   written WITH class id 0x2E009000 and no index, or FFFFFFFF
u16    flags               bit 2 (0x4): a skin FileRef follows the scale; high byte = VARIANT index
Vec3   pivotPosition
f32    scale
[FileRef packDesc]         when flags & 4: the placement's skin file
Vec3, Vec3                 (24 bytes, kept verbatim)
[0x03101005 skippable, 9 B: u32 version 1, u32 word (4 on every Summer placement), u8]
0xFACADE01
```

Facts:

* **A placed item is positioned by its PIVOT, and the pivot belongs to the
  PLACEMENT [VERIFIED-GAME]**: an item can declare several pivots
  (`InflatableTubeCurve4` declares two, 28 m apart) and only the placement
  says which was used. The field is the vector from the pivot to the mesh
  origin (minus the model's own pivot): 197047's platform declares `(4,0,4)`
  and every placement records `(-4,0,-4)`. Placing by the mesh origin put a
  whole 100 s run 1.5 m off its road (`tools/mapgeom/MAPGEOM.md` §3).
* **`scale` is honoured only for mesh-modeler crystal items**
  (`CGameCommonItemEntityModelEdition` + `CPlugCrystal`); an item whose entity
  model is a `CPlugPrefab` or a `CPlugStaticObjectModel` renders at its
  authored size whatever the field says, and a `VegetTreeModel` ignores it too
  (`tools/mapgeom/src/rescale.rs`, `tree_clear.rs`) **[VERIFIED-GAME]**. The
  tiny converter therefore scales geometry inside the item files and keeps the
  placement scale at 1 (with the pivot halved).
* The variant byte (`flags >> 8`) selects an entry of a variant-list item
  (Summer 11's `Show` rigs: 4 = RigStraight32m, 23 = Light4Spots, 28 =
  Fogger16M; a `PalmForest` placement's variant is its palm species).
* The skin `FileRef` (`u8 version; [u8;32] checksum (v≥3); string path; string
  url`) names e.g. `Skins\Stadium\LightColors\WhiteCold.dds` (Summer 15's 462
  skinned lights) or the older `Skins\Stadium\LightTube\<Name>.zip` (an EMPTY
  zip; the name is the colour). The MODEL must declare a skin (header chunk
  `0x090F4000`, `item-cgameitemmodel.md` §4) for the game to apply it; **an
  embedded item is never re-skinned by the game** (screens thread, 2026-09-07),
  so light colours are baked into the item (`tools/mapgeom/src/light_skin.rs`).
* The `0x03101005` word is NOT the animation phase (probed 0 vs 4, nothing
  changed); the phase is the `0x03043063` byte (§4).
* The waypoint node's tag is ignored by the game, exactly as for blocks
  (`map-blocks.md` §5); its `order` matters for linked checkpoints.

### 1.2 Placement in world **[VERIFIED-GAME]**

`world = absolutePositionInMap` (already world metres; the cell is informative
only), oriented by `yaw, pitch, roll`, the mesh offset by `−pivot` and scaled
by `scale` about the pivot. The tiny converter's transform is affine and
uniform (`p' = anchor_t + s·(p − anchor_s)`), and every world coordinate of the
MediaTracker goes through the same point transform (`map-mediatracker.md`).

## 2. Waypoints carried by items

Spawn, Checkpoint, LinkedCheckpoint and Goal items (the gates) carry the
`CGameWaypointSpecialProperty` inline. What fires is the MODEL's trigger
shape: `GateFinish32m`'s trigger is geometrically identical to
`GateCheckpointLeft32m`'s, so promoting a checkpoint gate to a finish reports
the declared split to the millisecond; a gate placed at the horizontal centre
of a checkpoint BLOCK's 32 m cell fires at exactly the block's split
(`tools/tmmaps/src/segments.rs`). Swapping a model to probe a route is
forbidden in this tree: on 285885 `GateFinish32m` quadruples the trigger
volume and the origin control then reads 50.589 instead of 61.229
(`tools/tmmaps/MAPS.md` §3).

## 3. The snapped-on tables (chunk tail, v7/v8)

Five `i32` arrays: `block_indexes[k]` (an authored-block word `(u8 tag, u24
index)` — Summer 02's shore plants snap on Water ZONE blocks with tag `0xFF`;
or −1), `item_indexes[k]`, `snap_groups[k]`, `u07[k]` (always −1), then
`snapped[i]` — one per item — the group item `i` hangs off, or −1. An item
placed ON a block or on another item in the editor is deleted with it; the
file records that as these groups. Version 7 has an `Int2[]` before them. The
five arrays must end exactly at the chunk end (asserted).

## 4. Side tables, one byte/word per placement

| chunk | per | encoding |
|---|---|---|
| `0x03043062` colour | `u32 version`, one byte per unbaked block, then per baked block, then per item | 0 Default, 1 White, 2 Green, 3 Blue, 4 Red, 5 Black |
| `0x03043068` lightmap quality | same order, one byte each | Normal 0, High 1, VeryHigh 2, Highest 3, Lowest 4, VeryLow 5, Low 6 (`MapElemLightmapQuality`) |
| `0x03043063` AnimPhaseOffset | `u32 version`, one byte per ITEM | eighths of the period (`EPhaseOffset`, 4 = Half) |
| `0x03043069` macroblock refs | `u32 version`, one `i32` per authored block, then per item, then `Int2[]` (id, flags) instance pairs | copied; shrunk with the block list |

What the colour byte does **[VERIFIED-GAME]**: a material tints where its
`_D_HueMask` texture's alpha says so, to the entry its `ColorTargetTable`
(`Stadium\Media\ColorTargetTables\*.ColorTable.gbx.json`) names for the byte.
`TrackWall` (mask 0.95 everywhere), `TrackBorders`, `Technics`,
`TrackWallClips`, `RoadTech`, `DecalPlatform` and the plastic floor tint;
`TechnicsTrims`, `PlatformTech`, `PoolBorders`, `WaterBorders`, `DecoCliff`,
the terrain skins' `TrackWallInWorld` carry no mask and never tint. The editor
holds the file's byte for every generated filler too (`/mapblocks2` prints
`color`). A wrong hue on a converted wall was never a wrong byte — it was the
wrong MATERIAL (`tools/tmmaps/TINY.md` "Placement colours").

What the phase byte does **[VERIFIED-GAME]** (2026-09-08, lineup PH1): the game
honours it for an embedded kinematic item exactly as for a stock one; Summer 15
is the only campaign map using it (facing channel pistons 0 and 4, Level1
rotors 4 and 2, sixty inflatable mats 2). `SInstanceParams.Phase01` inside the
item does NOT set the phase (three bakes with 0/0.25/0.5 move identically).

The lightmap-quality byte and the `PreLightGen` scale word changed nothing
visible on the tree lineups (probes listed in `TINY.md` "The pass-2 look").

## 5. Growing the item array (`append_item_clones`)

Extra records are cloned from a non-waypoint donor whose model and author are
lookback REFERENCES (so no table slot is added), the count patched, the
snapped-on tables' last array grown by one −1 per clone, and `0x03043062` /
`0x03043068` / `0x03043063` / `0x03043069` grown in step. A rename of a model
(a length change in the private table) forces a re-emit of the chunk with its
two size words fixed (`map.rs::write_to`).

## 6. Not known

* The two trailing `Vec3`s of the v8 record (kept verbatim).
* `u01`, `archiveVersion`, `anchorTreeId` semantics.
* `snap_groups` values beyond "ride along".
