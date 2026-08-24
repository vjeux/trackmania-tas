# 146612 "Spaghetti Nights 2" cannot be opened in the in-game editor

**Status, 2026-08-24: the CLIENT LOADS THIS MAP FINE. `PlayMap` has it in a
playground in 6.2 s and `EditGhosts` in 5.7 s, both with the right uid. Only the
TRACK EDITOR entry — `EditMap`, and every variant of it — hangs.** So this is
not a map that cannot be read, and it is not a client that cannot load it: it is
one door out of three, and the two that work do not lead to the MediaTracker.

Nothing has been filmed here yet, and the remaining route to a clip is named at
the bottom.

## The symptom, measured rather than described

`ManiaTitleControlScriptAPI::EditMap()` accepts the path and returns. Then, for
as long as anyone has waited (180 s):

| what | 146612 | 285885, the control, same session |
|---|---|---|
| `ctx` | **0 forever** | 1 (track editor) at 6.1–8.6 s |
| `IsReady` | **false forever** — the title never accepts another command; only a relaunch clears it | false while loading, editor open at the end |
| `LatestResult` | **0 = Success** | 0 = Success |
| `CustomResultType` / `Data` | empty / `[]` | empty / `[]` |
| dialog | **none** | `FrameWaitMessage` while it loads |
| process memory | **+60 MB, then flat** | **+940 MB** (1.52 → 2.46 GB) |
| CPU across the wait | ~7 s per 20 s wall — the same rate as an idle client | the same rate, after it opened |

Read together: the load **starts** (the title goes busy, 60 MB is taken) and
then **stops**, without an error, without a dialog, and without doing the work
an editor entry does. The game is not busy. It is not slow. It has given up
silently, and it has taken the title API with it.

`LatestResult` is the title API's own error channel and this page's first new
instrument — `EditMap` returns `void`, so that enum is the only thing it can
ever say about a map it declined. **It says Success.**

## What opens it, and what does not

Every row on a fresh game, every row with 285885 as the control in the same
session (`shootctl probe --map M --how …`).

| door | 146612 | control |
|---|---|---|
| `PlayMap(map, "", "")` | **playground in 6.2 s**, right uid | playground in 3.5 s |
| `EditGhosts(map)` | **playground in 5.7 s**, right uid | playground in 2.9 s |
| `EditMap(map, "", "")` | hangs, as above | editor in 6.1 s |
| `EditMap2(map, "", …)` — decoration left empty | hangs identically | editor in 7.4 s |
| `EditMap2(map, "48x48Day", …)` | rejected outright: `IsReady` stays **true**, nothing loads | — |
| `EditMap3(…, UpgradeToAdvancedEditor)` | wired, not yet run | — |

## What has been ruled out, and how

| hypothesis | test | result |
|---|---|---|
| slow load, timeout too short | 90 s, 120 s, 150 s, 180 s | never opens |
| accumulated editor state | first map loaded on a fresh process | never opens |
| wrong or mangled path | bytes dumped; `Test-Path` on the Windows side | correct |
| file missing, truncated, corrupt | 3 824 673 B, header intact; **the dedicated server loads it and re-simulates 8 of 8 ghosts to their exact times**, with `SEGMENT_…_DO_NOT_PUBLISH` returning DNF as the negative control | not corrupt |
| a stale copy | md5 `16e7220f2128587c0d0018626feacb0f` on the render box and on the shared store, **checked again 2026-08-24** | identical |
| size | 285268 is 4.65 MB, 210218 is 10.0 MB; both open | not size |
| **anything structural only this map has** | `tmmaps header --tsv` over all 36 corpus maps, 35 of which open — container version, class, header chunk table, external refs (0 everywhere), title, exever, exebuild, envir, mood, maptype, mapstyle, validated, lightmap version, ghost blocks, declared `<dep>` files, embedded zip, block/item counts, and every body chunk id | **nothing is unique to it.** Its exebuild is shared with 145875, its mood with 238835, its two embedded custom BLOCKS are dwarfed by 210218's 83, its 12 532 blocks by 208024's 35 538, and no body chunk id it carries is absent from every other map |
| the container, i.e. how the bytes are stored | `tmmaps rewrite --reemit`: same content, whole body recompressed, 3 824 673 → 3 880 918 B, 0 content bytes changed | **hangs identically** — it is the content, not the container |
| **this install's file-id cache** (`Config/User.FidCache.Gbx`) | moved aside, game relaunched, cache rebuilt from scratch | **hangs identically**, and the control opens on the same rebuilt cache |
| the embedded object zip | chunk `0x03043054` replaced with an empty one from 227654 (383 889 B → 24 B), nothing else touched | **the failure CHANGES**: instead of silence, the editor raises `FrameAskYesNo` within 2 s. Answering yes dismisses it and returns to the menu; the editor still does not open |

That last row is the one lead with a live signal in it. The embedded objects are
one custom item and **two custom blocks** (`Magnet_Blocks\M2PlatformTechSlopeBase.Block.Gbx`,
`M2_PlatformTechSlope2End.Block.Gbx`); with them present the editor dies quietly,
with them absent it asks a question and then declines. Both variants are on the
render box beside the stock map, as `146612_reemit.Map.Gbx` and
`146612_nozip.Map.Gbx`, so neither has to be rebuilt.

## A trap worth knowing, because it invalidated three earlier attempts

Do **not** build a Windows path with `printf`: `\v` is a vertical tab and
**`\146` is an octal escape for `f`**, so `\tas\146612` was handed over as
`\tasf612`. Use a quoted heredoc, which interprets nothing.

## Why there is still no clip, and what would produce one

The render pipeline reaches the MediaTracker through the **track editor**
(`ctx 1` → `CGameEditorPluginMap::EditMediatrackIngame()`), which is precisely
the door this map does not have. The two doors that do open it land in a
playground, and `DialogEditCutScenes_OnInGameEdit` (`/mtingame`) does nothing
from a playground — **measured on the control as well**, so that is a fact about
the call and not about this map.

The map's own tape is ready: `TAS_39183.Ghost.Gbx` carries another run's
telemetry (kappa 0.476), and the regenerated
`TAS_39183_carrier.Ghost.Gbx` — kappa 1.000, oracle 39.183, V1–V11 clean — is on
the shared store at `tm-nomovie-20260824/146612/`. There is nothing wrong with
what would be filmed.

**The one route left is `EditReplay2(ReplayList, EReplayEditType::Shoot)`** —
the game's own "open this replay in the MediaTracker", which needs no track
editor at all. It needs two things nobody has built:

1. a `.Replay.Gbx` wrapper around a pure ghost (`ghost map set` refuses: a pure
   ghost carries no embedded map, and there is no `ghost replay wrap` yet);
2. the AngelScript call itself, whose parameter is a `MwFastBuffer<wstring>` —
   whether Openplanet lets a plugin construct one is unknown, and a compile
   error there unloads the plugin and takes the render box down with it, so it
   is a lock-held experiment with `shootctl install`'s rollback behind it.

Until then this map is **not filmable by this pipeline**, which is a much
narrower statement than the one this page used to make.
