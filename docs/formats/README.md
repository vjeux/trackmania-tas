# docs/formats — every Trackmania 2020 structure and file format this project knows

One page per structure, written from what the code reads and writes and what
the game was measured to do with it. Nothing here is copied from a spec: where
a layout comes from a community transcription (GBX.NET, gbx-py, the Openplanet
class reference) it says so, and where it was checked against real bytes or
against the running game it says that too.

## How to read a page

Every fact carries one of these tags (a page-level tag applies to everything
not marked otherwise):

| tag | means |
|---|---|
| **[VERIFIED-GAME]** | measured in the running game: the client on the render box, or the dedicated server's `/validatepath` — a count, a time, a pixel, a crash |
| **[EXE]** | read out of the game binary (disassembly, the reflection tables, a transcribed writer) |
| **[FILE]** | read from real files and round-tripped: the reader consumes every byte to `0xFACADE01` and the writer reproduces the input byte for byte |
| **[INFERRED]** | follows from the above but was not itself measured |
| **[COMMUNITY]** | GBX.NET / gbx-py / openplanet.dev naming, not independently checked here |

Conventions: chunk ids in hex (`0x0304301F`); byte layouts as tables (offset,
type, name, meaning) or as an indented grammar; times in SECONDS with a
decimal (`22.730`) except inside a grammar the game owns (ms fields); every
page ends with **Not known** — what is partial, and what test would settle it.
Source pointers are `tools/<crate>/src/<file>.rs` (the repo's tools workspace),
root notes (`GHOSTFORMAT.md`, `GHOSTS.md`, `OLDBUILD.md`, `tools/tmmaps/TINY.md`,
`tools/mapgeom/MAPGEOM.md`), the `tools/tmmaps/tiny/re/*.md` reverse-engineering
reports, and memory keys by name (`tm2020-…`). Dates are when a fact was
learned; a later date on a contradicting fact wins, and the pages say which.

## The pages

### The container

| page | covers |
|---|---|
| [gbx-container.md](gbx-container.md) | the GBX file: header, header chunk table, reference table, body, `'C'`/`'U'`, LZO, skippable chunk framing (`PIKS`), **lookback strings** and their table scoping, **node references**, archive classes, primitive types (FileRef, Iso4, GbxLoc, optimized ints), the class-id index |
| [lzo-body-splice.md](lzo-body-splice.md) | editing a compressed map without recompressing it: the LZO1X stream grammar and the four write methods |

### The map — `CGameCtnChallenge` `0x03043000`

| page | covers |
|---|---|
| [map-cgamectnchallenge.md](map-cgamectnchallenge.md) | header chunks `0x03043002/003/005/007` and the XML, the uid rules, the body chunk catalogue with real sizes, coordinates and `yoff`, what the client and server do with a map |
| [map-blocks.md](map-blocks.md) | `0x0304301F` block records, the flag bits (FREE `0x20000000`, GHOST bit 28, variant/mobil bits), placement rotation, `0x0304305F` free positions (yaw/pitch/roll), `CGameCtnBlockSkin`, `CGameWaypointSpecialProperty`, `0x03043048` baked blocks and the shared lookback table |
| [map-items.md](map-items.md) | `0x03043040` anchored objects v8 (model, rotation, position, pivot, scale, variant, skin FileRef), snapped-on tables, the side tables `0x03043062` colour / `0x03043068` lightmap quality / `0x03043063` phase / `0x03043069` macroblocks |
| [map-embedded-objects.md](map-embedded-objects.md) | `0x03043054` manifest + zip, what the game keeps, drops, caches and refuses, the collection-word dialog, the re-save |
| [map-genealogy.md](map-genealogy.md) | `0x03043043` zone genealogy, what the game regenerates from it, the fixed water planes, the clear/fill/keep policies |
| [map-lightmap.md](map-lightmap.md) | `0x0304305B`, the stored lightmap applied by object index, the load-time bake, the editor compute (`/shadows`), the content-keyed cache under `ProgramData\Trackmania\Cache`, the transplant |
| [map-validation-ghost.md](map-validation-ghost.md) | `0x0305B00F`, the skeleton, chunk-local Ids, the missing node-index word, the stale uid, `tmmaps validate` and the medal words |
| [map-mediatracker.md](map-mediatracker.md) | `0x03043049`, clips/tracks/groups, CameraCustom/Path/Orbital, Triangles, Fog, triggers and the trigger grid, what fires when |

### Ghosts and replays

| page | covers |
|---|---|
| [ghost-cgamectnghost.md](ghost-cgamectnghost.md) | `CGameCtnGhost` chunk by chunk with offsets, the identity layout, the result chunk `0x0309202B`, copies of the declared time and uid, the provenance block `0x0309202D` (container binding + start waypoint index), container rules, what the client does that the server cannot see |
| [ghost-input-tape.md](ghost-input-tape.md) | `0x0309201D`: archives, the packet grammar (state word codings, mouse, per-mode fields, respawn bit), the gtape/CSV/TICK/event text forms |
| [ghost-telemetry-cplugentrecorddata.md](ghost-telemetry-cplugentrecorddata.md) | `CPlugEntRecordData` grammar (descs, entities, columnar delta coding), the 116-byte `CSceneVehicleVis` sample as the decoder reads it AND as the server writes it, confidence tiers, neutral/dead bytes, the recorded input channel |
| [replay-cgamectnreplayrecord.md](replay-cgamectnreplayrecord.md) | `CGameCtnReplayRecord`: the map INSIDE the replay, the three header chunks, the driver copies, unwrap, extensions, game-written replays |
| [ghost-caches.md](ghost-caches.md) | `LaunchedCP.gbx` (`0x03262000`) layout, `MTAuthorGhost`, Autosaves, the lightmap cache folder |
| [dedicated-server-oracle.md](dedicated-server-oracle.md) | `/validatepath`: invocation, the output shape (`ValidatedResult` vs `DeclaredResult`), what the server checks and ignores, era studies |

### Items, meshes, materials

| page | covers |
|---|---|
| [item-cgameitemmodel.md](item-cgameitemmodel.md) | `CGameItemModel`: the two accepted shapes (static / crystal), header chunks incl. the game-skin chunk `0x090F4000`, body chunks incl. `0x2E00201F` waypoint and `0x2E00202A` DisableLightmap, `CGameCommonItemEntityModel`, `CPlugStaticObjectModel`, triggers/spawns/gates, the rules the client enforces |
| [mesh-cplugsolid2model.md](mesh-cplugsolid2model.md) | `CPlugSolid2Model` v34 fields, `CPlugVisualIndexedTriangles` chunks, vertex declarations and streams (names/types/spaces, Dec3N), the SH rules, LOD ladders and masks, lightmap UV1 atlases, light sockets |
| [collision-cplugsurface.md](collision-cplugsurface.md) | `CPlugSurface` and its shapes, the id table `physics \| gameplay << 8`, the FULL physics enum 0–80, the gameplay ids, per-link physics, what each engine does with 13 / 28 / water volumes, `collhash` |
| [materials.md](materials.md) | `CPlugMaterialUserInst` reflection layout and chunk, the user-texture slot enum, which slot each model reads, `CPlugMaterial` `0x09079017`, modifiers and game skins (StadiumOnTerrain, Specials), the per-environment whitelist, hue masks and colour tables, light and screen skins, mods |
| [crystal-cplugcrystal.md](crystal-cplugcrystal.md) | `CPlugCrystal`: chunks, layer types, the Crystal archive v37, optimized-int sizing, the group tree, what a written crystal item needs |
| [prefab-and-dyna.md](prefab-and-dyna.md) | `CPlugPrefab`, `CPlugStaticObjectModel`, `CPlugDynaObjectModel` + `NPlugDyna_SKinematicConstraint` (moving obstacles), the vertex-tween wall, `CPlugFxSystem` and particles, `CPlugLight`/`GxLight*`, `CPlugVegetTreeModel` |
| [textures-dds-skins.md](textures-dds-skins.md) | DDS as the game wants it, `.Texture.gbx`/`CPlugBitmap`, skins (car, advertisement, light colours, flags, mods, block skins, game skins), where pictures are cached |

### Blocks, packs, eras

| page | covers |
|---|---|
| [blockinfo-cgamectnblockinfo.md](blockinfo-cgamectnblockinfo.md) | `CGameCtnBlockInfo*` families and chunks, variants, units, mobils, clips, the engine's free-clip bake algorithm (EXE), water volumes `0x0315B00B`, custom blocks (`CGameBlockItem`, archetypes), terrain-tile rule |
| [pak-nadeopak.md](pak-nadeopak.md) | NadeoPak v18 layout, keys (`MD5(base ++ "NadeoPak")`), Blowfish, LZ4 + the 1006-byte dictionary, hashed names (`MD5.Compute136`), the dummy-write IV perturbation, the four integrity digests, what we extract |
| [eras-and-old-builds.md](eras-and-old-builds.md) | reading a file's build, the dated server archives (2020-07-01 … 2022-06-21), physics across eras, the client archive traps, what the era does not change |

### Ours

| page | covers |
|---|---|
| [openplanet-plugin-api.md](openplanet-plugin-api.md) | the GhostShooter plugin's HTTP routes as the box API: contexts, the game's own block/item lists, wheels and car rows, lightmap compute, save, cameras, MediaTracker, the held run, Nadeo services |
| [pipeline-files.md](pipeline-files.md) | gtape / CSV / TICK / event scripts, the placements mapping rows, views.tsv, loadloop, MANIFEST / holds / approvals / ships / rowbuilds / lidrows, done-files, the store layout |
| [LESSONS.md](LESSONS.md) | what the game requires, rejects, silently drops, crashes on, rewrites and caches — each with date and evidence — and the method lessons |

## Where the readers live

| crate | role |
|---|---|
| `tools/gbx` | the GBX/ghost byte layer: container, header, record (telemetry), tape, sample vocabulary, recwrite |
| `tools/ghost` | every ghost mutation and the oracle driver; `synth`, `ident`, `hdr`, `lcp`, `unwrap`, `oracle`, `verify` |
| `tools/tmtraj` | reads ghosts (trajectories, fields, provenance) |
| `tools/tmmaps` | the map: `map.rs` surgery reader, `header.rs`, `mediatracker.rs`, `splice.rs`, `tiny.rs`, `census.rs`, `segments.rs` |
| `tools/mapgeom` | geometry and the packs: `blockinfo.rs`, `bake.rs`, `static_item/*`, `crystal_model.rs`, `veget.rs`, `pak*.rs`, `names.rs`, `parents.rs` |
| `tools/fk` | the server-transcribed sample writer (`SAMPLE-LAYOUT.md`, `vislayout.rs`) |
| `tools/openplanet-plugin` | the in-game HTTP API |
| `tools/tinyctl`, `tools/shootctl`, `tools/clip`, `tools/haul` | the pipeline that drives the box and publishes |
| `tools/chunkswap`, `tools/strpatch`, `tools/dsprobe` | one-purpose instruments used in the era and binding experiments |

## Gaps this folder makes explicit (things we use but had not written down)

* `CGameCtnDecorationSize` (`0x0303B000`): read by nothing; it is where the
  per-environment `yoff` should come from (measured per map instead).
* The server's compact `Inputs` string grammar in the `/validatepath` output.
* Which field of `0x0309202D` binds a tape to its container.
* The internal layout of the lightmap chunk's frames and of the
  `*.Bump.LightMap.zip` cache entries.
* The non-vehicle telemetry entity classes (`0x2D001000` and five others) —
  the client wants one of them and nobody knows what it holds.
* `CPlugSkel` skinned meshes (the expandable finish horns).
* MediaTracker `CameraOrbital` keys and every opaque media block class.
* The car-skin zip's contents.
* A dozen `u0x` fields per class, listed in each page's **Not known**.
