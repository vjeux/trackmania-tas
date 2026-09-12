# Lessons learned: what the game requires, rejects, silently ignores, rewrites, caches, and crashes on

Each line is a measured behaviour of Trackmania 2020 (client `Trackmania.exe`
2026 builds on the render box, or `TrackmaniaServer` `_Latest` 2026-05-15)
with the date it was learned and where the evidence lives. "Silently" means:
no dialog, nothing in `UGCErrorsLog.txt`, nothing in the log — the only tell is
a count or a pixel. Times are seconds.

## 1. The game REQUIRES (it refuses, or nothing works, without)

| requirement | evidence |
|---|---|
| A MAP body must be LZO-compressed (`'C'`): the dedicated server says `Can't load map` on a `'U'` map. A ghost/replay may be `'U'` | 2026-08, `tools/tmmaps/src/gbx.rs` tests; 21 maps of the 285-map sweep re-written (`MAPS.md` §1a) |
| The shared lookback table (`0x0304301F` … `0x03043048`, `0x03043051`) must keep its LENGTH; append at the tail, never renumber | 2026-08, `Can't load map` bisect, memory `gbx-map-surgery-lookback-tables.md` |
| `numNodes` in the container header must be the game's own count (10 instead of 39 → `Can't load map`) | same |
| `/validatepath` reads only `*.Ghost.Gbx` / `*.Replay.Gbx`; it takes a directory RELATIVE to `UserData/Replays/`; a replay's map is the one INSIDE it | 2026-08-2x, `tools/ghost/src/oracle.rs`; 2026-09-03 memory `tm2020-dedicated-server-validatepath.md` |
| A tape validates only in the container it was built in: chunk `0x0309202D` must travel with `0x0309201D` | 2026-08-2x bisect with `chunkswap`, `OLDBUILD.md` §6 |
| A ghost's `0x03092010` uid must name a map the server has (`Can't load` otherwise); two maps with one uid in `UserData/Maps` bind to the first found | `tools/tmmaps/src/oracle.rs` |
| An embedded item's ident (header `0x2E001003` AND body `0x2E00100B`) = the placement's ident = the manifest row; the placement's COLLECTION word = the manifest row's | 2026-09-05/06 (Lineup7), 2026-09-08 (3f793de: the "Missing Items: AC00000000" dialog on every load of 21–24) |
| An embedded custom BLOCK is paired with its manifest row by the FILE'S OWN IDENT, not the archive path | 2026-09-10, d9549f05 |
| Every visual of ONE material in an item must have the SAME vertex declaration (the client merges them with the first's) | 2026-09-06, crash `0x140456c35`, rule SH-01/02 |
| A visual must carry a TexCoord1 set to be drawn under any valid material | 2026-09-08, tree bake (cards drew RED under an unknown model) |
| A user picture must be BLOCK-COMPRESSED WITH MIPS (DXT5); A8R8G8B8 no-mips draws the checkerboard | 2026-09-10, d5f41f57 |
| Materials of an embedded item are `CPlugMaterialUserInst` LINKS whose first folder is in the environment's compiled whitelist (`Test`/`Editors`/`Effects` + the Stadium family in Stadium maps; BlueBay has NO branch) | 2026-09-05 EXE read, `tiny/re/material-collection-folder-whitelist.md` |
| A special gate's EFFECT must be an `NPlugTrigger_SGateSpecial` ENTITY of a `CPlugPrefab` directly under `CGameItemModel` (the entity-model `TriggerShape` does nothing; the slab in the hull is a wall) | 2026-09-08, 3f5da2a; engine-measured on the tiny-20 Boost gate |
| A waypoint item's trigger surface must be re-emitted as a canonical mesh (a verbatim pack `*_Trigger.Shape.Gbx` gets the item dropped) | 2026-09-06 |
| A moving item = prefab DIRECTLY under `CGameItemModel` with an inline dyna + kinematic constraint; wrapped in a `CGameCommonItemEntityModel` it is dropped | 2026-09-07, 67f2be7 |
| A dyna object needs a non-NULL `DynaShape` (the constraint dereferences it) | 2026-09-07, crash `+0xb7088c` |
| The Nadeo core upload must carry author, authorScore, bronze/silver/goldScore, collectionName, mapStyle, mapType, name, mapUid, isPlayable, and be ≤ 25 MiB (26 214 400 B) | 2026-09-06/09/11 (HTTP 400 "not a valid file" at 26.7 MB, 413 at 35.9 MB) |
| A 0-block map needs SOME lightmap chunk or the editor stack-overflows at load; a lightmap compute needs ≥ 1 authored block (GreenCoast 0-block builds crash 6 s into `ComputeShadows`) | 2026-09-07 (62c3586), 2026-09-12 coordinator |
| `ComputeShadows` must go BUSY to have computed anything; `ready:true` without `busy` = served from the content-keyed cache | 2026-09-12, `tinyctl lightmap` |

## 2. The game REJECTS LOUDLY (a dialog or a printed reason)

| behaviour | evidence |
|---|---|
| `Updating data... $<$> (???)` with the bar at 0 = a block model the client cannot resolve; the map's own name renders `(???)`; the same dialog with EMPTY text hangs a fresh-uid load — retry with a fresh uid | 2026-08-23 `MAPS.md` §6; 2026-09-07 |
| `Missing Items: X … load anyway?` (`FrameAskYesNo` 1.3–2.8 s after `/playmap`): the FULL ident (name, collection, author) of a placement was not found; lists ≤ 18 lines in file order | 2026-09-08 |
| Play mode from the STORED file refuses foreign stock names (`Missing Items: - PlantSmallA`, a BlueBay species in RedIsland) where the editor loads the map fine | 2026-09-07, 0416baf5 |
| `Material: unknown collection folder Stadium` in `Documents\Trackmania\UGCErrorsLog.txt`; `File not found: 'SignLogoTurbo.dds.png'` (cosmetic) | 2026-09-05/08 |
| Server: `wrong simu[, but reached some checkpoints (n out of m)]`, `not finished, with N respawns`, `using unsupported respawns (N)`, `using known-flawed game exe`, `Can't load replay`, `unexcepted walltime (103s)` | `dedicated-server-oracle.md` §2 |
| A repacked `.pak` with any of its four digests wrong: the process EXITS after "Initializing meta…" | 2026-09, memory `nadeo-pak-container-digests.md` |
| A record that runs past its declared time: soft-rejected on import (`0 → 0` ghost blocks, a `FrameMessage`); so is a multi-client carrier (7+ entities, mode-15 packets) | 2026-08, `tools/ghost/src/film.rs`; 2026-09-09 bd33f978 |

## 3. The game SILENTLY DROPS or IGNORES (count or look at pixels)

| behaviour | evidence |
|---|---|
| Any embedded item it cannot instantiate: wrong collection/author, a pack-file reference, an FxSystem emitter with a model, a prefab wrapped in an entity model, an EMPTY collision surface, a wrong-case stock ident (`ShowFogger8M` vs `ShowFogger8m`), a verbatim pack trigger shape, a missing referenced file. Geometry goes with it. `/mapitems` count is the tell | 2026-09-05…09, memory `tm2020-embedded-items-rules.md`; FX verdict 2026-09-08 d752bbe |
| Exact duplicate placements (2252 of 2259 kept) | 2026-09-07 |
| Waypoint TAGS in the map (`Checkpoint` → `Goal`, or the node deleted): the MODEL decides | 2026-08, `tools/tmmaps/src/segments.rs` (four experiments, 19.538 / 4 CPs each) |
| Placement `scale` on prefab / static-object / VegetTreeModel items (only crystal items scale) | 2026-09-05/06, `rescale.rs`, `tree_clear.rs` |
| `SInstanceParams.Phase01`, the in-record `0x03101005` word (the phase is the `0x03043063` byte) | 2026-09-08 |
| The declared checkpoint COUNT and the declared time (echoed as `DeclaredResult`, compared, never gating) | `oracle.cps_does_not_gate` |
| The two ghost hashes `0x0309200E` / `0x0309201C`, the build stamp, block model names, the map uid vs content, the header, `lightmap=`, the telemetry (server) | `GHOSTFORMAT.md`; `tmmaps renamecheck`; `strpatch` |
| A stored lightmap on a map with 0 authored blocks (recomputed at load, 0.4 s); the lightmap-quality byte on trees; `PreLightGen`'s scale word; `IsNatural` on lights | 2026-09-07…10 |
| Physics 13 (Water) on an ITEM plate — the car falls through on both engines; the placement colour byte on `TDSN`/`TDOSN` (`colour0` unread); a flag's colour byte (the ENVIRONMENT skin shows) | 2026-09-10; 2026-09-08 |
| Bare `NoRespawn` on an ITEM's `SWaypoint` (the server re-places at the crossing) | 2026-09-09 |
| A `.webm`/png/jpg as a user texture, `Skins\…`/`Items\…`/subfolder paths (missing-texture pattern) | 2026-09-07, ad94d72 |
| A `Deco`-layout visual's colour tint on a BLOCK path an item never reaches | 2026-09-07 (hypothesis) |
| An item's lightmap-quality byte on the tree lineups; `0x090BB002` words; `u13` of the Solid2; the collector flags word 8 vs 0x10 | 2026-09-08 probes |

## 4. The game CRASHES on (client unless stated)

| trigger | site / evidence |
|---|---|
| a later visual of a material lacking an element the first has (TangentU read through NULL); a Deco-layout visual under `PlatformTech` | `0x140456c35`, 2026-09-06/08 (rules SH-02/03) |
| thirteen custom materials of one NAME whose models differ | `0x140456513`, 2026-09-08 |
| a static item with 5 LOD levels | `ud2` at `+0x1e9947`, 2026-09-07 |
| a dyna object with a NULL `DynaShape` | `+0xb7088c`, 2026-09-07 |
| the `0x0917B000` companion node of a spawn entity (any body) | 2026-09-10/11 bisect, `ship16-diagnostics/ring-bisect/` |
| the item-editor light form `CPlugLightUserModel` + `light_insts` | `+0x4c9062`, 2026-09-07 |
| a sub-visual (frame) table on a STATIC visual, drawn | `0x140a9c174`, 2026-09-06/07 |
| an inline `CPlugBitmap`; a bare `.dds` named as a Gbx; a NULL texture ref or particle model in an FxSystem | 2026-09-08, `LogCrash_00000000002EDBA8`, FX-01/02 |
| a crystal group whose parent is itself (infinite loading spinner) | 2026-09-05 |
| a game-made block item embedded as an ITEM | 2026-09-04 |
| a 0-block map with NO lightmap chunk (editor STACK_OVERFLOW `+0x96d7e0`); GreenCoast 0-block `ComputeShadows` | 2026-09-07; 2026-09-12 |
| a house item floating 2.5 m over the lane after a Fragile gate killed the game ~10 s after crossing, twice, no dump — unexplained | 2026-09-11 |
| client `ImportGhosts` of a regenerated vehicle-only record (some files, not others) | 2026-08, `ghost verify` V11 |
| server: an era-mismatched replay on the 2020-07-01 build (SIGSEGV) | 2026-09-03 |

## 5. What a SAVE / RE-SAVE rewrites

| behaviour | evidence |
|---|---|
| The editor's `SaveMap` drops every loose `.dds`, the validation ghost, re-mints the uid, renames the map to the file stem, regenerates parked blocks' neighbours (8581 Lake blocks), drops items with an empty collision mesh, writes a fresh `0x0304305B` | 2026-09-07, 2026-09-12 (`tinyctl lightmap` transplants the chunk) |
| The uid changes on every editor save; the game's caches are keyed by map NAME (`MTAuthorGhost<name>`, `<name>.LaunchedCP.gbx`, `<login>_<name>_PersonalBest…`) | 2026-09-09 |
| Renaming a block model to a plain block in the file leaves its collision and waypoint in place (parked ≠ removed) | 2026-09-06/07, `collhash` |
| A cell write on a FREE block is silent (no error, no effect); a per-block rotation of a multi-piece structure makes a staircase | 2026-08, `MAPS.md` §3 |
| LZO recompression is never bit-reproducible: compare decompressed bodies (`tmmaps bodydiff`), never file md5s | 2026-08/09 |

## 6. What the game CACHES (and how to defeat it)

| cache | key | defeat |
|---|---|---|
| embedded item models and textures | FILE NAME, per session | per-build aliases `AC{map}{minute%1000}{idx}`, `TINY_PICTURE_SUFFIX`, or restart (`tinyctl shoot --fresh`) |
| a map's embedded items / lightmap in the editor | map UID | `tmmaps setuid` before re-shooting a rebuilt test map |
| lightmaps | map CONTENT (`C:\ProgramData\Trackmania\Cache\*_<Collection>_<mood>.Bump.LightMap.zip`) | delete the entries |
| the MediaTracker's "Author ghost", the LaunchedCP list, the play-mode autosave | map NAME | render from a renamed copy (`Render NN <name>`) |
| mods, thumbnails, banners, sign packs | hash-named files under `Cache\` | — |
| the Nadeo token | ~hourly rotation (401 → retry; `/nadeotoken`) | — |

## 7. Client vs dedicated server — where they differ

| | client | server |
|---|---|---|
| start position | the Spawn item (+ the block's (8,1,8) or the gate's spawn point 5.3 m along its axis) | the waypoint INDEX in `0x0309202D` |
| reads | telemetry samples (a render PLAYS samples; it does not re-simulate), byte 32 (chase camera), byte 73 (wireframe car), dirt bytes | the input tape only |
| physics 13 / 28 | transparent / solid | transparent / solid (parity) |
| gate specials in prefab form | fire | fire (turbo 01/04/05/08/16, reactor 05/07) |
| respawn | at the item's pivot/StartYaw when no spawn entity ("drops you to the side") | re-places at the crossing state |
| the map's embedded ghost, the MediaTracker | used | never read |
| the byte floor between them | 0.48–0.52 mm was two copies of the car struct in server memory, not a physics floor (`tools/README.md`) | |

## 8. Method lessons (how the facts above were won, and how they were lost)

* **Count, don't look.** Every silent drop was found by a count (`/mapitems`,
  `items=N loaded=K`, `ghost record show`), never by a screenshot that "looked
  fine". A playcheck that only LOADS proved nothing about play (the parked
  builds with black structures shipped for two days).
* **Two results per file**: the server's `DeclaredResult` is the file's own
  claim; a parser that takes `"Time"` lines as they come makes a stale
  declaration look correct.
* **Compare decompressed bodies, never compressed files**; a no-edit write must
  be byte-identical (`tmmaps roundtrip`), and every write is verified by
  re-reading before it is a file.
* **A byte-scan for `PIKS` sees phantoms and misses inline chunks** — the
  account id, the map uid and a replay's whole embedded map are inline. Walk
  the structure.
* **Never fabricate stateful encodings**: the lookback table and the node index
  table are body-wide graphs; a reader that invents them reads plausible
  garbage (the `#40000001` author, the "0 ghosts" load).
* **A harness limit is not a physics limit**: when we run the real engine every
  quantity it computes is in memory (the 116-byte sample was fully transcribed
  from the server binary once someone went to look; the "40-byte readout" was
  our window, not the game's).
* **Name every byte** (`ghost synth`, `SAMPLE-LAYOUT.md`): the 61 "unnamed"
  bytes of a ghost were the account id string and the uid MwId; a layout claim
  is tested by regenerating a DOWNLOADED recording from its own inputs
  (`ghost roundtrip`) with the class balance of every bit checked first.
* **Ground truth is the game's own list**: `/mapblocks2` for the bake,
  `/wheels` for the surface, the server transcript for the time, a game-written
  archive for the zip, the exe's reflection table (`mapgeom exe-class`) for a
  struct — before any GBX.NET field name is trusted (27 was not RoadIce; the
  `Color` array is not a colour; `nbRespawns` is not a respawn count).
* **Superseded findings are kept with a pointer, not deleted**: the "ident must
  carry the map's collection" rule (nuanced by the TME items), the "declared-only
  tile" rule (wrong; the UNIT rule holds), the ≥757-entry flakiness (the
  collection word), the parking path (deleted), the "ghost Ids are numbered
  first" belief (chunk-local). A reader of this folder should expect the same
  of anything dated before its own measurement.
* **A control for every knob**: an A/B with a fresh uid and fresh alias bases,
  or the cache decides the picture; probes ≥ 60 m from any stock light; the
  same camera on both sides; the original as the yardstick.
* **Read the game's logs first**: `UGCErrorsLog.txt` names rejected classes;
  `LogCrash_<offset>.txt` is ONE file per fault offset (never rewritten by a
  new crash at the same site — the Windows Event Log has the timeline); WER
  minidumps have no heap.
* **A session-wide cache reads as nondeterminism** (1 of 2 on a mirror item, one
  load in three): make the name change with the bytes.
* **When the ask is "why does X look wrong", the answer was never the byte we
  suspected first**: not the colour byte but the material; not the intensity
  but the uv1; not the lightmap but the stale chunk applied by index; not the
  entry count but the collection word.
