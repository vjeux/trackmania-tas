# Can an embedded item reference a PACK prefab? (AR, 2026-10-11 → measured 2026-10-17/18)

**No.** A `.Item.Gbx` carried in a map's archive cannot reach `GameData\Stadium\…` through its GBX
reference table, whatever the spelling. An embedded item's references resolve only AT OR BELOW its
own folder inside the archive — the same reference form that fails into the pack resolves to a prefab
file carried next to the item.

## The experiment (`jumprig prefabtest`, build 128130, box WhiteStick, play mode via `/playmap`)

Instruments: the game's own `Stadium\Items\ObstaclePusher16m.Item.Gbx` (468-byte body, EntityModel =
EXTERNAL node `Media\Prefab\Items\ObstaclePusher\ObstaclePusher16m.Prefab.Gbx`, ancestor 1, flags 1,
useFile 0) and `LightCone2mX2.Item.Gbx` (one external node, the prefab) — body kept verbatim, ident
renamed, only the reference table rewritten (`mapgeom reref`); plus our own baked pusher item
(`static-item`) split by `mapgeom split-prefab` into a 639-byte item + a 229 KB `.Prefab.Gbx`.
Every item passes `mapgeom item-check` before a run (see the lesson below). Readout: the Fid Probe
plugin's `map` (every AnchoredObject's model + EntityModel class, the mounted archive files and their
nod state) over a lineup row beside a stock item; the host map carries 866 items of its own.

| map / item | reference | result |
|---|---|---|
| 55 pusher-body wrappers (run11, combined) | anc 0/1/2/3/4/6 × {`Media\…`, `Stadium\Media\…`, `GameData\Stadium\Media\…`, `Game\Stadium\Media\…`} × useFile 0/1; absolute `C:\…\GameData\Stadium\…`, `\GameData\Stadium\…`, `Packs\Stadium.pak\Stadium\…`; flags 0 | map CRASHED at load (below) |
| `REnatA1U0` alone (run13) | pusher body, the pack's own table (anc 1, `Media\Prefab\…`) | map loads, item DROPPED |
| `LcA1pack` alone (run14) | LightCone body, the pack's own table | DROPPED |
| `SplitPackA1` / `A2std` / `A4gd` (run9) | our body → pack prefab path at anc 1 / `Stadium\…` anc 2 / `GameData\Stadium\…` anc 4 | DROPPED |
| `CtlPusherSplit` (run3/5/12) | our body → `CtlPusherSplit.Prefab.Gbx`, anc 0, bare, in `Items/` | **KEPT**, EntityModel `CPlugPrefab` |
| `SplitA0dir` (run8) | our body → `Sub\CtlPusherSplit.Prefab.Gbx`, anc 0 (`Items/Sub/`) | **KEPT** |
| `SplitA1` (run8) | our body → `Prefabs\CtlPusherSplit.Prefab.Gbx`, anc 1 (`Prefabs/` at the archive root, file present and mounted) | DROPPED |
| `MirA0bare` (run12) | pusher body, anc 0: our prefab + the pack's modifier (its own table rewritten to anc 0) + GameSkin + PlaceParam, all bare in `Items/` | **KEPT** — the game's own item body works embedded when its files travel with it |
| `LcA0self` (run12) | LightCone body, anc 0 → our prefab | **KEPT** |
| `MirA0dirs` (run12) | pusher body, anc 0 with the native folder paths under `Items/`, but the modifier's ORIGINAL anc-2 table | DROPPED — one climb in the chain kills the item |
| `LcA0pack` / `LcA0bare` (run15) | LightCone body, anc 0 → a COPY of the pack prefab whose own refs (material, light) point into the pack | DROPPED, no crash |

Dropped = absent from `AnchoredObjects`, no dialog in play, nothing in `UGCErrorsLog.txt`; the file
sits in the mounted archive with `nod = null`. Kept controls' files carry `nod = CGameItemModel`, the
sidecar `nod = CPlugPrefab`.

## Why (Fid Probe `tree`)

The game's file system is a set of DISJOINT roots, each with `ParentFolder = null`:

| drive | root |
|---|---|
| Game | `C:\Program Files (x86)\Steam\steamapps\common\Trackmania\` — `GameData` + `Packs`; pack contents are mounted UNDER `GameData` (`GameData\Stadium\Items\ObstaclePusher16m.Item.Gbx`, container `Stadium.pak`) |
| User | `C:\Users\<you>\Documents\Trackmania\` |
| Fake | `<fake>\` — the map archive is mounted whole at `<fake>\MemoryTemp\CurrentMap_EmbeddedFiles\ContentLoaded\` (Items, Media, GameSkin, Prefabs… every zip folder appears) |
| ProgramData | `C:\ProgramData\Trackmania\` |
| Resource | `…\Trackmania\Resource\` |

A reference table is resolved RELATIVE to the referencing file's folder: `ancestorLevel` parents up,
then the named folders down. For embedded content the climb is refused even inside the archive
(`SplitA1`), and `GameData` lives on another root, so no ancestor count and no spelling reaches it;
the table has no absolute or drive form (the only other entry kind is a numeric resource index,
flags bit 2).

## Crash hazard

The combined 68-item map (the 55 wrappers + the mirrors above) CRASHED the client while loading:
`HEAP_CORRUPTION 0xc0000374`, stack `CGameCtnApp::InitChallengeDataWithLoadScreen` →
`InitChallengeData` → `[Game] init challenge` → `CGameCtnChallenge::InitEmbeddedItemModels` /
`LoadEmbededItems` (0x140b8fc20) → 0x140ab1610 → 0x1409009a0 → 0x1409052f0 →
`CSystemArchiveNod::DoLoadFromFid(Gbx)` (0x140904730) → 0x140903d30 → ntdll (dump
`run11/crash-17616.dmp`, read with `mapgeom crash`). Every single-wrapper map loaded and dropped its
wrapper cleanly, so an unresolved reference is not always benign: enough of them in one map take the
client down.

## Dedicated server

`jumprig srvload` (Linux server, port 3461) answers `...Load succeeds` for every one of these maps —
including a control whose sidecar prefab was deliberately left out of the archive — so the server's
load verdict does not exercise embedded entity models (only script-requested items produce
`Missing Items`, cf. the RL-ball runs). Not an oracle for this question.

## What this leaves

- Several items in one map can share ONE geometry file in the archive (`CtlPusherSplit`'s shape):
  N wrapper items of ~600 B + one prefab/mesh file, bare-name refs at ancestor 0 or a sub-folder of
  `Items/`. The game's own item bodies work that way too once every file they name travels with
  them at ancestor 0 (`MirA0bare`) — but a pack prefab's own tree (meshes, dyna objects, constraints,
  materials by file, textures) is pack refs all the way down.
- A custom BLOCK over a native block (`CGameBlockItem` + `ArchetypeBlockInfoId`, AR's Stade blocks)
  is an id lookup, not a file reference — that path stays open.

## Instrument lesson

The first three runs' wrappers were corrupt: the author rewrite (`tiny_assets::replace_lp`,
"Nadeo" → a 22-char login) left the header chunk-table sizes stale (+17 bytes); the game refused
those files for THAT, which looked exactly like a reference failure. `mapgeom item-check` caught it
("header chunks use 1669 of 1686 user-data bytes"); the author now goes through
`crystal::rename_ident`. Run `item-check` on every generated item before a game run — a control made
by a different path does not cover the probes. Also: `Fids::Preload` of a well-formed wrapper whose
prefab cannot be resolved HANGS the game; test through a map load.

Evidence on the box: `~/reactor/prefab/run{3,5,8,9,11,12,13,14,15}/` (map-report.txt, tree-*.txt,
fake-drive.txt, crash-17616.dmp); maps `Maps/Probe/{prefabref,prefabref2,mirror*,mirA,crash*}.Map.Gbx`.
