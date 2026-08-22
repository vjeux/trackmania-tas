# The render pipeline, rebuilt

`render2.sh` drove the game by clicking fixed screen coordinates and then
sleeping a guessed number of seconds. Every failure this week came from that
layer: a click landing on a screen that had not appeared, a picker row index
computed from `ls | sort` that moved when a file was added, a 45-second map wait
that was not long enough, a camera aimed at nobody.

This replaces it with API calls against the game's own object graph.

## What runs it

* `tools/shootctl` — Rust, std only, no dependencies. The driver.
* `tools/openplanet-plugin/` — the Openplanet plugin (AngelScript, because that
  is Openplanet's language; everything on our side of the line is Rust).
* `tools/gsdev` — three-line wrapper over `shootctl` for the plugin dev loop.

## The three rules

**1. One folder, one map, one set of replays.** The map is loaded by absolute
path (`EditMap`), so it needs no folder at all. Ghosts stage into
`Replays/_shoot/`, which holds exactly the files for this render — nothing to
page through and nothing to mis-index. A leftover `old/` subdirectory inside the
picker's folder silently shifted every row and imported the wrong car; that
class of bug is gone with the folder.

**2. No pointer clicks.** Replaced so far, each verified against the object
graph rather than against a screenshot:

| was | now |
|---|---|
| click EDIT at (3343, 2075), then `DialogEditCutScenes_OnInGameEdit`, 3 retries | `CGameEditorPluginMap::EditMediatrackIngame()` |
| four menu clicks + a tile hunt to load a map | `ManiaTitleControlScriptAPI::EditMap(path)` |
| click "Tracks +", "Player camera", then cycle a button up to 12× re-reading | `Dev::SetOffset` on `ClipEntId` / `GameCam`, offsets looked up **by name** at runtime |
| click rows in a 12-row paged picker, twice | `DialogSaveAs_Files` — the entries by name, with their selected flag |

The old `DialogEditCutScenes_OnInGameEdit` call is worth calling out: it is a
dialog button handler, so with no dialog up it did nothing at all and returned
"ok". The shell covered for it with three retries and 13 s of sleeps.

**3. No sleeps.** `shootctl wait --ctx N` polls the game's own context number
(0 menu, 1 track editor, 2 MediaTracker, 3 racing) and returns the moment it
changes, with a deadline that reports **what it last saw** instead of carrying
on regardless. Map load went from a blind 45–180 s wait to a measured 16.4 s,
and the MediaTracker from 13 s of sleeps to 0.1 s.

## The dev loop

`Meta::ReloadPlugin` (found in the strings of `Openplanet.dll`) lets the plugin
reload itself: write the file, `gsdev install`, done in about a second. No
restart, no menus.

The thing that actually costs a restart is a **compile error** — Openplanet
unloads the plugin, the HTTP server goes with it, and there is no `/reload` left
to call. So `shootctl lint` checks the source against the game's own class dump
(`OpenplanetNext.json`) before the game sees it, and catches all three mistakes
that cost restarts today:

```
unknown class: CGameDialogFileBrowser
CGameCtnMediaBlockCameraGame.GameCam is const -- 'the property has no set accessor'
route calls undefined function: DumpDialogLists()
```

`gsdev install` refuses to install if the linter objects, and if a reload fails
anyway it restores the last good plugin and restarts the game by itself.

## Two traps worth keeping

**The plugin must bind `0.0.0.0`, not `127.0.0.1`.** The game runs on Windows;
the driver is a Linux binary in WSL, and WSL's loopback is a different machine.
Bound to loopback, a Windows `curl.exe` reported the plugin healthy one line
before the driver got "connection refused" — the same symptom as a dead plugin.

**`/proc/net/route` stores the gateway little-endian**, so its bytes are already
in network order. Reversing them produced `1.0.18.172`, and another connection
refused that looked exactly like the plugin being down.

## Not finished

The ghost import is the one step still short of the goal. The dialog is
`FrameDialogSaveAs` and its file list is readable by name
(`{"name":"_shoot\\","sel":false}`), but `CGameFid::Selected` is exposed
read-only to AngelScript and its Openplanet offset reads 65535, so it cannot be
set the way the camera fields were. Options not yet tried: find the real memory
offset of the selection flag, or drive the selection through the list control
that owns it. Until then the import still needs its clicks — which is why the
isolated folder matters: with exactly the files for this render in it, there is
only ever one row to hit.
