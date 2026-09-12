# The map lightmap: chunk `0x0304305B`, the lightmap-quality bytes, and the load-time bake

Readers/writers: `tools/tmmaps/src/map.rs::strip_lightmap`, `tmmaps
striplightmap`, `tools/chunkswap` (transplant), `tinyctl lightmap` /
`shootctl lightmap` (compute in the editor through the plugin's `/shadows`),
`mapgeom static-item` (the items' lightmap UVs). Confidence **[FILE]** for the
chunk head, **[VERIFIED-GAME]** for everything about what the game does with it
(memory `tm2020-tiny-campaign.md`, entries of 2026-09-07, 09-08, 09-09 and
09-12).

## 1. Chunk `0x0304305B` (skippable)

```text
u32 version
u32 hasLightmaps           0 = never computed (the chunk is then 16 bytes)
u32 u01
u32 u02
[u32 lightmapVersion, the frames…]   ~0.5–1.3 MB: WEBP atlases + the CHmsLightMapCache
```

572 190 B on `testdata/map2.Map.Gbx`; ~950 KB on the Summer 2026 sources;
~1.0–1.3 MB for a computed tiny lightmap. Header XML `lightmap="8"` is the
lightmap VERSION the client must load. `strip_lightmap` rewrites the chunk to
`version, 0, u01, u02` (16 bytes — the form of a map whose shadows were never
computed) and fixes the PIKS size word. The internal layout of the frames is
NOT decoded here.

## 2. What the game does with it **[VERIFIED-GAME]**

* **The stored lightmap is applied BY OBJECT INDEX.** A block-deleted map
  whose every item still loads reads as "VALIDATED"/unmodified to the editor,
  which applied the source's lightmap to it (Summer 05: the full-size
  structures' shadows on the deck); a parked build of Summer 15 rendered every
  converted-block item BLACK in play (dirt walls, water columns) because the
  appended items fall outside the lightmap's tables, while the original items
  (screens, palms, flags, gate signs) were lit. The published 01–17 were such
  builds until the strip became default (2026-09-07).
* **A map with 0 authored blocks has its stored lightmap REJECTED by the game
  and a default-settings lightmap recomputed at every load**
  (`CGameCtnApp::HmsLightMapBindAndComputeCurrentChallengeLightMapWithDefaultSettings
  → HmsLightMapComputeCurrentChallenge → CHmsLightMap::ComputeLighting →
  NHmsLightMap::RenderLighting_Frames/RenderLightDirect → RenderLightBall*/
  RenderLightSpot*`): 0.39 s in the editor, 0.44 s in play on a 0-block build;
  an A/B of the original Poland with and without its stored chunk renders
  IDENTICALLY. That load-time bake is coarse and per item: lighter/darker
  rectangles on big flat items, black undersides — the "shadow seam" a player
  reported on tiny 11 (2026-09-12). The same compute took **334 s** on a
  parked build whose 2 116 authored + 4 513 baked records were stacked in
  cell (0,0,0) under the name `RoadDirtCheckpoint` (a LIT checkpoint arch —
  the point-light pass evaluated the whole pile per light; renamed to the
  light-less `DecoWallBasePillar`: 12.6 s).
* **A 0-block build WITHOUT a lightmap crashes the EDITOR** (STACK_OVERFLOW at
  load, 3 of 4 opens of tiny 09, `Trackmania.exe+0x96d7e0`, the editor's
  automatic lightmap pass over a blockless map); `/shadows` on one crashes the
  same way. So the deleted-block path KEEPS the stale chunk (harmless: the
  game rejects it at block count 0), and a lightmap is computed on a build
  with ONE authored block kept (`tmmaps tiny --keep-zone-block`, lightmap
  stripped).
* **The editor's re-save (`SaveMap`) drops every embedded `.dds`** (46 tree
  atlases on 11), drops the validation ghost and re-mints the uid — so the
  computed lightmap is TRANSPLANTED: only chunk `0x0304305B` is copied from
  the re-saved file into ours (`chunkswap`, the default of `tinyctl lightmap`).
  The game accepts the transplanted chunk in play (item order unchanged; zone
  tiles regenerated identically): 11's ramp uniform, real shadows; the chunk
  fits under the 25 MiB Nadeo upload cap (11: 22.25 → ~23.3 MB).
* **Quality**: `/shadows?q=3` (Default) is a real 90 s compute on 11; `q=2`
  and `q=5` returned without the editor ever reporting busy and IDENTICAL
  chunks — the editor skips a quality it already holds or that is out of
  range, so `tinyctl lightmap` refuses a bake that never went busy
  (`/shadowsq` reports `ready:true` between the request and the start of the
  work; accept `ready` only after `busy`, or after 20 s never seeing it).
* **The editor's Fast pass (`--shadows 2`) = play's load-time bake**, so
  same-camera editor frames with it are what play shows; without it the tiny
  side has no lightmap in the editor and the whole scene reads brighter.
* The game caches a map's lightmap AND embedded items by UID for the session:
  a rebuilt test map under the same uid may show stale items (DDS files are
  re-read); A/Bs need `tmmaps setuid`.
* Baked GI bounce from bright grass/jungle items turns bridge undersides
  lime-green (original mid-grey) — the one side effect of a real bake seen
  and not yet attributed (2026-09-12).

## 2.1 The lightmap CACHE (measured 2026-09-12, coordinator session, on the render box)

* `C:\ProgramData\Trackmania\Cache\` holds one file per computed lightmap,
  named `<64-hex>_<16-hex>_<Collection>_<mood-tag>.Bump.LightMap.zip` (e.g.
  `…_15B977D0E822A29C_BlueBay_Day64.Bump.LightMap.zip`,
  `…_GreenCoast_199A.Bump.LightMap.zip`).
* The editor's `ComputeShadows` REUSES a matching entry and reports the
  request as already satisfied — the plugin's `/shadowsq` shows `ready:true`
  and never `busy`. **The match is by map CONTENT, not uid**: a copy with a
  fresh uid was still served from the cache. Deleting the `*.LightMap.zip`
  entries forces a real compute.
* A map with NO lightmap chunk (`hasLightmaps = 0`) computes, but the editor
  then drops to the main menu without saving (twice on tiny 11).
* GreenCoast tiny builds with 0 blocks crash the client 6 s into
  `ComputeShadows`; BlueBay ones (2061 baked `Sea` records) do not.
* `ComputeShadows1(EShadowsQuality)`: q=3 Default took 94 s and q=4 High
  442 s on an 8022-item map; the `0x0304305B` chunk grew from 950 KB (the
  stale source) to 1.00 MB (q=3) / 1.18 MB (q=4).

## 3. Per-item lightmapping

* `0x03043068` carries one lightmap-quality byte per placement (Normal 0,
  High 1, VeryHigh 2, Highest 3, Lowest 4, VeryLow 5, Low 6); on the tree
  lineups 0/1/2/3 changed nothing visible.
* An item's `CGameItemModel.DisableLightmap` (+0x278) is body chunk
  `0x2E00202A` = one `bool32` — the only per-item-model lightmap switch in the
  reflection table (`tools/tmmaps/tiny/re/material-collection-folder-whitelist.md` §3).
* The per-item bake uses the mesh's TexCoord1 (uv1) as the lightmap chart. A
  visual without a TexCoord1 set is NOT DRAWN under any valid material; every
  card of a tree crown given uv0 as uv1 sampled ONE lightmap texel (a
  sunlit-white bush at night, 38.7 % saturated pixels) — `tree_lightmap_uv1`
  gives every card its own chart; merged items get one lightmap cell per PART
  (`repack_lightmap_parts`: a pack Solid2 lays its atlas over the whole unit
  square, so stacked parts wrote every part's light into the same texels)
  (`mesh-cplugsolid2model.md` §5).
* `PreLightGen` (Solid2) carries the lightmap generation parameters and UV
  groups; its scale word (8 / 128 / none) changed nothing visible.
* Lights baked into an item (`CPlugLight` inline in the Solid2) are honoured by
  the bake: a half-size `Lamp` with radii × 0.5 and intensity unchanged lights
  the ground at half height like the stock one (the falloff is a function of
  d/R). The item-editor light form (`CPlugLightUserModel` `0x090F9000` +
  `light_insts`) CRASHES the client at map load (`Trackmania.exe+0x4c9062`).

## 4. Not known

* The frame layout inside the chunk (WEBP atlases, the cache tables).
* What the two hex fields of the cache file name hash (a content digest of
  the map and of the settings, presumably; §2.1 measured only that a uid
  change does not miss the cache).
* Why the game rejects the stored lightmap exactly at block count 0 (the
  observation is on 0- vs 1-block builds; the threshold in between is not
  measured).
