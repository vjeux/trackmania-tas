# Embedded objects: chunk `0x03043054` (the custom items and blocks a map carries)

Reader/writer: `tools/tmmaps/src/map.rs` (`embedded_objects_payload`,
`replace_embedded_objects`, `replace_embedded_zip_keep_manifest`),
`tools/tmmaps/src/header.rs` (`embedded_zip`, `zip_entries`, `deflated_zip`,
`item_ident_author`, `set_ident_collection`), `tools/mapgeom/src/embedded.rs`,
`zipcheck.rs`, `tools/tmmaps/src/zippad.rs`. Confidence **[FILE]** for the
layout; every rule in §3 is **[VERIFIED-GAME]** (each cost a control run,
memory `tm2020-embedded-items-rules.md`).

## 1. Layout

```text
u32 version (1)
u32 0
u32 byteCount             of everything that follows
u32 nItems                the MANIFEST
[u32 3]                   lookback version (when nItems > 0; the chunk has its OWN table)
nItems × { Id ident, u32 collection, Id author }
u32 zipLength
u8[zipLength] zip         "PK\x03\x04" …
u32 0
```

A map with nothing embedded carries the 24-byte form (`version, 0, 12, 0,
zipLength 0, 0`). The manifest lists item Idents only; support files
(prefabs, materials, `.dds`) are ordinary ZIP entries with no manifest row.
The ident is the file name relative to `Items/`, verbatim — it is what the
placements name and what the ZIP entry is called.

## 2. The ZIP

* Entries RELATIVE (`Items/Foo.Item.Gbx`, `Items/Sub/Foo.Item.Gbx`, or under
  `Blocks/` — 197047 stores its models under `Blocks/`, so a reader must match
  the placement's spelling as a SUFFIX of the zip path, shortest match); no
  directory rows; no extra fields; deflate method 8 (the game writes its own
  archives deflated; `deflated_zip`, level 6) — stored (method 0) also loads.
  Absolute stored entries (`C:/Users/...`) ⇒ everything dropped.
* `zipcheck` verifies local headers against the central directory, CRCs,
  sizes, duplicate names, ordering, the manifest against the entries, and
  each item file's own header ident against the manifest row — the same
  report on a game-written archive (an original with club items) is the
  reference to diff against.
* The map-embedded custom **item** is a `.Item.Gbx` (`item-cgameitemmodel.md`);
  a custom **block** is a `.Block.Gbx` (`CGameBlockItem`, the TMX form —
  `blockinfo-cgamectnblockinfo.md` §9) and is counted separately because the
  editor treats it differently (`header.rs::zip_blocks`).

## 3. What the game requires, and what it does when a rule is broken

| rule | symptom when broken |
|---|---|
| The ident inside the file — header chunk `0x2E001003` AND body chunk `0x2E00100B` — must equal the placement ident and the manifest row | header-only match ⇒ found but silently dropped; body without a name (archive crystals, game-made block items) ⇒ never found ("Missing Items" dialog) |
| The ident's COLLECTION (header, body, manifest) must equal the map's (BlueBay 0x1C, Stadium 0x1A) | found-but-dropped, no dialog (`set_ident_collection`) |
| The author string must be resolvable — a back-referenced author that reads as `#40000001` is written into the manifest by a naive reader | the game instantiates such items and never renders them (2026-09-05) |
| The game silently DROPS any embedded item it cannot instantiate | absent from `Challenge.AnchoredObjects`; ALWAYS probe counts (`/mapitems`, `items=N` under the placement count) |
| "Missing Items: X" dialog | the ident was not found at all; it lists at most 18 lines in FILE ORDER — it is everything, not a pattern |
| A stand-in stock item is named by the FILE'S header ident, case-exact (`ShowFogger8m`, not the pack path `ShowFogger8M`) | a `ShowFogger8M` placement is dropped without a dialog |
| The game caches an embedded item MODEL by FILE NAME for the whole session — and embedded TEXTURES too | two maps (or two builds of one map) embedding different pieces as `AC00000200.Item.Gbx` show the FIRST-loaded model in the second; a re-encoded `ScreenLogo.dds` drew the old bytes until renamed. Hence per-map, per-build aliases `AC{map:02}{minute%1000:03}{idx:03}` and `tinyctl shoot --fresh` for A/Bs |
| Big archives were flaky to load: "Missing Items: AC00000000.Item.Gbx … load anyway?" (a FrameAskYesNo 1.3–2.8 s after `/playmap`, on EVERY load of 21/22/23/24) | **SOLVED 2026-09-08 (3f793de): a placement whose COLLECTION word (26, inherited from the source club-item placements) differs from its manifest row's (the map's, 0x1C) — "the game resolves a placed item by the FULL ident (name, collection, author)"; answering yes opens the map without those placements (20 mismatches in 21, 23 in 22, 17 in 23, 15 in 24, 0 in 25). The entry-count suspicion (≤ 697 fine, ≥ 757 flaky) is DEAD as a cause (942 entries fine). `MapFile::set_item_collection` patches the 4-byte word; `mapgeom zipcheck` flags mismatches; `tinyctl loadloop` is the reliability instrument |
| A hard SIZE cap exists in the exe: `Reason: The size of embedded files (%1) must be lower than %2.` (`CGameCtnChallenge::LoadEmbededItems` / `InitEmbeddedItemModels`, `CGameCtnApp::CheckCollectorsAvailability` → "Missing Items:") | not measured; the Nadeo UPLOAD cap is 25 MiB = 26 214 400 B (26.7 MB → HTTP 400 "not a valid file", 35.9 MB → 413) |
| Play mode from the STORED file refuses foreign stock names ("Missing Items: - PlantSmallA", a BlueBay species in a RedIsland map) while the editor loads the same map fine (all packs installed) | a stand-in species must exist in the map's collection (2026-09-07) |
| Loose `.dds` files in the archive at LOAD are harmless (kept); a bare `.dds` named as a Gbx in a reference table crashes (class id `0x40000000` read off the DDS bytes); a rewritten `.Texture.gbx` + `Image/<name>.dds` beside the item is the form an item's ref table resolves | (2026-09-08, eb48b49) |
| The editor's `SaveMap` (a re-save) DROPS every embedded `.dds` (46 tree atlases on 11), drops the validation ghost, re-mints the uid, regenerates the parked blocks' neighbours, and drops every item whose collision mesh is EMPTY (`_FC_Ground` decals) | so a computed lightmap is TRANSPLANTED by chunk, never taken from the re-saved file (`map-lightmap.md` §2) |
| Exact duplicate placements (28 `PalmTreeSmallA` at identical positions from overlapping prefab vegetation, 2 pillar blocks per cell) are dropped by the game | harmless: 2252 of 2259 items kept (2026-09-07) |
| Any reference from an embedded item to a PACK file (an external `.Material.Gbx`, a prefab) | the item is dropped; its files are mounted at `<fake>\MemoryTemp\CurrentMap_EmbeddedFiles\ContentLoaded\Items\` (`/fids`), a Fid tree with no path to `GameData` |
| An item whose prefab has an FxSystem entity with a model (a live particle emitter) | dropped, geometry included, no dialog (`item-check` FX-03) |
| A game-made block item (`CGameBlockItem`) embedded as an ITEM | crashed the game (2026-09-04); as a custom BLOCK with an archetype it works (`blockinfo-cgamectnblockinfo.md` §9) |
| Map upload cap | ~25 MB; the detail-level pick (`--lod-pick`) moves a map under it, the deflate level does not (6 vs 10: ~2 % smaller at several times the time) |
| The editor's re-save | see the SaveMap row above |

**Read the game's own log first**: `Documents\Trackmania\UGCErrorsLog.txt`
names every rejected material (`Material: unknown collection folder Stadium`);
`LogCrash_<offset>.txt` has registers and callers, ONE file per fault offset
(the Windows Event Log has the real timeline).

## 4. Mounting and lookup

At load the game unpacks the archive into a virtual `MemoryTemp\
CurrentMap_EmbeddedFiles\ContentLoaded\` tree and pairs each placement's model
id with an entry by name. An embedded item's materials can only be
`CPlugMaterialUserInst` records resolved by NAME against the game's material
library, whitelisted per environment (`materials.md` §4).

## 5. Open questions

* The exe's embedded-files size cap (`%2` in the message above): the value is
  not measured; every shipped map stayed under the 25 MiB upload cap.
* Whether the TME club items' loading with collection 26 in a BlueBay map
  (they do, when placement and manifest agree) means the collection rule is
  "placement = manifest = file" rather than "= the map's": the two readings
  agree on every shipped set, so it has not been separated.
