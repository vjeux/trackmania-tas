# CGameItemModel — the item (`.Item.Gbx`, class `0x2E002000`)

The custom objects a map embeds and the pack's own items. Typed reader/writer
(byte-exact round trip on the 26 reference items): `tools/mapgeom/src/static_item.rs`
and `static_item/{item,file,assemble,build,check}.rs`; the crystal form is in
`crystal-cplugcrystal.md`; the mesh, collision and materials have their own
pages. Confidence **[FILE]** for layouts, **[VERIFIED-GAME]** for every rule in
§6 (each cost a load on the render box; memories `tm2020-embedded-items-rules.md`,
`tm2020-static-item-format-rules.md`, `tm2020-tiny-campaign.md`).

## 1. The two shapes the game accepts from an embedded item

```text
STATIC (Granady's tiny blocks, the pack's items — what tiny-library writes):
  CGameItemModel 0x2E002000
    chunk 0x2E002019 (Model) → CGameCommonItemEntityModel 0x2E027000 (v6)
        → static_object: CPlugStaticObjectModel 0x09159000 (archive class)
              mesh:  CPlugSolid2Model 0x090BB000  (visuals, materials, lights, LODs)
              shape: CPlugSurface 0x0900C000     (collision; absent when the mesh is collidable)
        → trigger_shape: CPlugSurface           (waypoint items)
      — or, for a MOVING/effect item, the entity model is a CPlugPrefab 0x09145000 DIRECTLY
        under the model chunk (the pack's layout): entities = static objects, dyna objects,
        kinematic constraints, NPlugTrigger_* records, FxSystems

CRYSTAL (mesh-modeler items; the ONLY kind whose placement SCALE is honoured):
  CGameItemModel → 0x2E002019 → CGameCommonItemEntityModelEdition 0x2E026000 → CPlugCrystal 0x09003000
```

Wrapping a prefab in a `CGameCommonItemEntityModel` gets the item dropped
silently; the pack item re-embedded under a new ident (external refs) is
dropped too — everything must be inline (`prefab-and-dyna.md`).

## 2. Header chunks

| chunk | content |
|---|---|
| `0x2E001003` | collector description: lookback version, `Id ident` (the file name relative to `Items/`), `Id collection` (plain number), `Id author`, `u32 8`, `string "Items"` (page), `Id null`, `i32 flags` (8 on every item-editor item, 0x10 on every pack item), `i16 1` (catalog position; 101/233 on pack items), `string "New Item"`, `u8 3` (prod state) |
| `0x090F4000` | game skin declaration — only on skinnable models (§4) |
| `0x2E001004` | icon (64×64 RGBA on pack items) |
| `0x2E001006` | file time (`u64`, 8 zero bytes when written here) |
| `0x2E002000` | `u32 1` |
| `0x2E002001` | `u32 0` |

The header ident and the body ident (`0x2E00100B`) must BOTH equal the
placement's ident (`map-embedded-objects.md` §3). The ident's author is read
as a lookback: a back-referenced author (`0x40000001`) that a reader prints as
`#40000001` and writes into the manifest makes the game instantiate the item
and never render it (2026-09-05).

## 3. Body chunks, in file order (`static_item/item.rs`)

| chunk | content |
|---|---|
| `0x2E001009` | `string pageName`, `bool32 hasIcon` [+ icon ref], `Id u01` |
| `0x2E00100B` | **Ident**: `Id path`, `Id collection`, `Id author` |
| `0x2E00100C` | `string name` |
| `0x2E00100D` | `string description` |
| `0x2E00100E` | `bool32 iconUseAutoRender`, `i32 iconQuarterRotationY` |
| `0x2E001010` | `u32 version`, `ref defaultSkin`, `string skinDirectory`, [v≥2 and empty dir: `ref`] — EMPTY on every Nadeo screen item; the skin declaration lives in the header chunk |
| `0x2E001011` | `u32 version`, `bool32 isInternal`, `bool32 isAdvanced`, `i32 catalogPosition`, [v≥1 `u8 prodState`] |
| `0x2E001012` | four `i32` |
| `0x2E002008` | `ref[] nadeoSkinFids` |
| `0x2E002009` | `i32`, `ref[] cameras` |
| `0x2E00200C` | `ref raceInterface` |
| `0x2E002012` | `Vec3 groundPoint`, four `f32` (painter ground margin, orbital centre height, orbital radius base, orbital preview angle) |
| `0x2E002015` | `i32 itemType` (Undefined 0, Ornament 1, PickUp 2, Character 3, Vehicle 4, Spot 5, Cannon 6, Group 7, Decal 8, Turret 9, Wagon 10, Block 11, EntitySpawner 12, DeformationDecal 13) |
| `0x2E002019` | **Model**: `u32 version`; [old models when version < the item type's own version (1,2→9; 4→10; 5→9; else 12; Block 11: none)]; v≥3 `Id defaultWeaponName`; v≥4 `ref phyModelCustom`; v≥5 `ref visModelCustom`; v≥6 `ref[] actions`; v≥7 `i32 defaultCam`; v≥8 `ref entityModelEdition` and, when it is null, `ref entityModel`; v≥13 `ref vfx`; v≥15 `ref materialModifier` |
| `0x2E00201A`, `0x2E00201B` | node refs |
| `0x2E00201C` | `u32 version (≥5)`, `ref placement` → `CGameItemPlacementParam` `0x2E020000` (all-skippable chunks: grid/pivot/placement class) |
| `0x2E00201D` | `i16` |
| `0x2E00201E` | **Archetype**: `u32 version`; v≥2 `string archetypeRef`; v≥5 and empty ref: `ref archetypeFid`; v≥6 `string skinDir`; v≥7 `i32` — how a custom BLOCK (`CGameBlockItem`) names the stock block info it borrows (`blockinfo-cgamectnblockinfo.md` §9) |
| `0x2E00201F` | **Waypoint**: `u32 version (≥8, 13 in the current pack — GBX.NET documents up to 12; 13 drops the `scriptWithSettings` ref)`, `i32 waypointType`, `bool32 disableLightmap`, [v 9..12 `ref`], [v≥11 `u8`], [v≥12 `i32, i32`]. WaypointType: Start 0, Finish 1, Checkpoint 2, None 3, StartFinish 4, Dispenser 5 (`CGameItemModel.WaypointType` +0x180) |
| `0x2E002020` | `u32 version (≥2)`, `string iconFid`, [v≥3 `u8`] |
| `0x2E002023` | `u32 version`, `u8`, `i32` |
| `0x2E00202A` | `bool32 DisableLightmap` (+0x278) — the only per-item-model lightmap switch in the reflection table |
| skippable chunks | kept raw |

`CGameCommonItemEntityModel` `0x2E027000` (v6): v0 `(ref phyModel, ref
visModel)`; v3 two strings; v≥4 `ref staticObject`; v≥2 `ref triggerShape`,
`Iso4 spawnLoc` (12 f32), `ref particleEmitter`, `ref[] actions`, [v<6 `ref`],
5 strings, `Iso4`, `i32 exprValidator`, [v≥5 `u8`]; `0xFACADE01`.

`CPlugStaticObjectModel` (archive, no chunks): `u32 version`, `ref mesh`,
`u8 isMeshCollidable`, and `ref shape` only when NOT collidable. Every stock
road block in the DEDICATED SERVER's pack reports `mesh = -1` (collision only);
a map-embedded item carries the visual mesh and often marks it collidable.

## 4. The game-skin header chunk `0x090F4000` (`CPlugGameSkin`)

`u8 version (8)`, `string dir` (the skin folder under `Skins\`: `RaceScreen6x1`
declares `Any\Advertisement6x1\`, the `TechnicsScreen155Straight` block info
`Any\Advertisement16x9\`), `string parent`, `string u03`, `u8 count`, `count ×
SkinFid { string classId?, string name, string file }` (e.g. `*Image` →
`Stadium\Media\Texture\Image\RaceAd6x1.dds`: `*` = every material's `Image`
sampler, the file = the default texture the skin replaces), then 16 trailing
bytes (items `0,0,0,1`, the screen block info `0,1,0,1`). Measured on every
skinned Stadium item and block info (`mapgeom skins`). It is what makes the
game feed the model's `Image` texture with the current advertisement or a
placement's own skin file; a model without it draws the material's default
(the yellow `RaceAd6x1.dds` NADEO panel). Light skins: the header lists the
`_I` textures (`LightSpot_I`, `ItemLamp_I`, `LightShape_I`, `LightTube_I`,
`LightTubeRefract_I`) and the placement's 16×16 DXT1 one-colour swatch
replaces each (`textures-dds-skins.md`).

## 5. Triggers, waypoints, spawns, gates (prefab entities) **[VERIFIED-GAME]**

| record | layout | facts |
|---|---|---|
| `NPlugTrigger_SWaypoint` `0x09178000` | `{ u32 version, i32 type, ref shape, u32 }` (engine struct 0x28 B: TriggerShape +0x18, NoRespawn +0x1c, StartYaw +0x20) | the pack ring items carry `NoRespawn 0`; NoRespawn is a BLOCK feature (`CGameCtnBlockInfo.NoRespawn`, chunk `0x0304E00F`); whether the runtime honours it on an ITEM was not found in the exe and the dedicated server says no |
| `NPlugTrigger_SSpawn` `0x0917A000` | CHUNKED: chunk `0x0917A000` v3 = identity `Iso4` + 24 bytes `0,0,0,0,-1.0,0`, FACADE | the client respawns at the waypoint's spawn = the item's origin/StartYaw when there is none ("drops you to the side"); the pack ring prefab carries it at pos 0 with the spawn in the Iso4 |
| `0x0917B000` companion | 8 bytes plain, body `(0, 11)` in the pack | **crashes the client at map load** whatever its body (bisected 2026-09-11, four builds of Summer 15: with it → process gone at 22 s / 52 s; either spawn form without it → PASS). Shipped sets carry the trigger-only form |
| `NPlugTrigger_SGateSpecial` `0x09179000` | `{ u32 version 2, ref triggerShape, u32 }` | a gameplay gate's EFFECT is this entity and nothing else: the same slab as the entity model's `TriggerShape` does nothing, in the hull as NotCollidable+gameplay it is a wall. Fires from an item for mode ids too (NoEngine measured: engine cut from the next frame). Effect id = the kind's `Modifier\<Kind>\Collision.Material.Gbx` ids (`materials.md` §3): Boost 18, NoEngine 4, Reset 8, Fragile 13; Turbo has no Collision file and the prefab slab's own (physics 0, gameplay 1) stands |
| trigger shapes | `CPlugSurface` (`collision-cplugsurface.md`) | block waypoint triggers are COMPOUND surfaces (`Gate\Checkpoint_Trigger.Shape.Gbx`: one 36-vertex disc; platform checkpoints a 0.1 m PLANE — "a real trigger plane, not the unit volume") and must be flattened to a mesh (`Surf::triangulate`), or the bake falls back to the unit box and a finish fires on its whole 32 m cube. A trigger surface copied VERBATIM from the pack's `*_Trigger.Shape.Gbx` gets the item dropped; re-emit it through `CPlugSurface::mesh` |
| spawn iso | `CGameCommonItemEntityModel.spawnLoc` / the block info's `spawn_loc` × scale | a block-derived start carries the block info's spawn (`[8, 1, 8]` on Granady's start = 11.4 m from the cell corner); a start GATE item spawns a metre or two off its pivot |
| the gate's icon panels | `SpecialSign<Kind>` materials on `Tech3_Block_TDSN_CubeOut_DispIn` | lit by a runtime DISPLAY INPUT fed by the live gate entity; in a static item the pack's own material draws the icon by itself (a generated picture drew the missing-texture checkerboard); the trigger CURTAIN (`Modifier\<Kind>\TriggerFX`) cannot be fed by an item |

## 6. Rules the client enforces on an embedded item **[VERIFIED-GAME]**

* Silently DROPPED (no dialog; probe counts): wrong ident/collection/author;
  any reference to a pack file; an FxSystem entity with a model (a live
  emitter — an FxSystem with NO emitters is kept); a prefab wrapped in an
  entity model; a verbatim pack trigger shape; a bare `.dds` in the
  reference table (an item's ref table DOES resolve against its own archive
  folder: a rewritten `.Texture.gbx` + `Image/*.dds` is the form that loads).
* CRASHES the client: a dyna object with a NULL DynaShape
  (`Trackmania.exe+0xb7088c`); the `0x0917B000` companion; thirteen custom
  materials of ONE name whose models differed (`0x140456513`); a later visual
  of the same material and v6 block lacking elements the first declares
  (`0x140456c35`, the TangentU NULL read: rule SH-02); a Deco-layout visual
  (position, normal, uv0) under a PlatformTech material (SH-03, the plastic
  loader reads TangentU); the item-editor light form (`CPlugLightUserModel`,
  `0x4c9062`); a crystal group whose parent is itself (infinite spinner);
  more than 8 user textures on a material (the reader drops them all;
  SH-04/05); a game-made block item embedded as an item (2026-09-04).
* HONOURED: `scale` only on crystal items; the colour byte through the
  material's hue mask; the `AnimPhaseOffset` byte on kinematic parts; embedded
  lights (kept and drawn); `DisableLightmap`; the LOD ladder (client cap
  `MAX_LOD_LEVELS = 4`; a source part's switch distances merge into the
  item's, unscaled).
* IGNORED: a `VegetTreeModel`'s placement scale; `SInstanceParams.Phase01`;
  the in-record `0x03101005` word; a material's placement colour on `TDOSN`/
  `TDSN` models (`colour0` unread).
* Materials are WHITELISTED per environment by the first folder of the link
  (`materials.md` §4); a rejected material CULLS the face set (the item draws
  nothing, no red) and `UGCErrorsLog.txt` says so.
* A kinematic part animates embedded exactly like a stock one (pushers,
  rotors); a vertex-TWEEN cloth cannot (the tween mark needs a real
  `CPlugMaterial` in the mesh's plain `Materials` list, i.e. a pack file
  reference an embedded item cannot make) — `prefab-and-dyna.md` §3.

## 7. Items vs blocks

A map's block list also holds ITEMS placed on the grid (gates, rotors,
seasonal props), so a name that resolves as neither a block info nor an
embedded model gets one more lookup as an item (`tools/mapgeom/MAPGEOM.md` §3).
A `Sheep.Item.Gbx` from the community and every RuurdBijlsma `Nadeo.zip` item
(2222 files) is a mesh-modeler CRYSTAL (`0x2E026000`), useless as a Solid2
corpus; the pack's 10 268 prefabs hold the 7989 real `CPlugSolid2Model`s.

## 8. Not known

* `0x2E00201A/1B`, `0x2E00201D`, `0x2E002023` meanings; `CGameItemPlacementParam`'s
  chunk layouts (kept raw).
* The 24-byte tail of `NPlugTrigger_SSpawn` and the `(0, 11)` companion body.
* Whether the `0x2E001003` flags word (8 vs 0x10) gates anything at runtime
  (probe knob `TINY_ITEM_DESC_FLAGS`, no measured effect yet).
