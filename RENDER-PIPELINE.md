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

## The ghost import, scripted

```
ImportGhosts()                       raise the file dialog
DialogSaveAs_Path = "_shoot/1_TAS.Ghost.Gbx"    (a path RELATIVE to Replays/)
DialogSaveAs_OnValidate()            accept the file
ImportGhosts_OnOk()                  perform the import
DialogSaveAs_OnCancel()              close the dialog it leaves open
```

Two things about that sequence cost hours each.

**Leave out `ImportGhosts_OnOk` and the dialog closes having done nothing.** No
error, no ghost. That is what "no ghost imported" looked like.

**`ImportGhosts_OnOk` leaves `FrameDialogSaveAs` OPEN.** Invisibly — the
MediaTracker draws over it — and it holds keyboard focus. Nothing downstream
could accept a dialog while it was there. Cancelling it afterwards is safe: the
ghost is already in the clip, and the import reports the ghost-block count
before and after, so success is never inferred.

The path must be **relative to `Replays/`**. A full `C:/...` path is accepted by
the field, closes the dialog, and imports nothing.

## The shoot dialog, scripted

This was the last click, and it was the hardest thing to find. The dialog is a
`CGameDialogShootParams`, whose `OnOk` is exactly what the OK button calls — but
nothing in the game's whole class dump declares a member of that type, so there
is no path to it through declared members at any depth.

**It is a frame of the GAME dialog menu.** `CGameCtnMenus` carries four
`CGameMenu`s — `Menus`, `InGameDialogs`, `Dialogs`, `SystemDialogs` — and the
shoot dialog lives in `Dialogs` (MenuOrder 5) as `FrameDialogShootVideo`. Every
earlier search looked in `BasicDialogs.Dialogs`, which is a different menu
(MenuOrder 11). `CGameMenu::CurrentFocusedControl` hands over the focused
control, and `CControlBase::Nod` is the dialog nod it is bound to:

```
game: frames=43 current=FrameDialogShootVideo
      focus=EnumFileFormat [CControlEnum] -> CGameDialogShootParams
```

So the accept is `sp.OnOk()`, and the settings are readable as numbers first —
`{"fps":30,"w":1280,"h":720,"ext":1,"hq":true,"estimated":"00:07:04"}` — which
is worth checking before committing to a render that takes minutes.

### Why not a keystroke

Because it silently corrupts the output. **The dialog does not open with OK
focused — it opens on `EnumFileFormat`**, so an Enter sent at it cycles the file
format. That is how a render came out as AVI when nobody asked for one. Also
worth recording: `wscript.shell` `SendKeys` does nothing at all here (it posts
window messages; Trackmania reads raw input), so only `SendInput` with
`KEYEVENTF_SCANCODE` reaches the game — which makes a stray keystroke both
possible and invisible. Neither is in the pipeline any more.

### The route that must never be tried again

**A memory scan for the dialog nod crashes the game.** Twice, on two different
implementations — including one that filtered candidates to aligned pointers
whose target begins with a vtable inside the module range and read them through
`Dev::SafeReadUInt64`. Findings from that attempt, so it does not have to be
repeated: `Dev::SafeReadUInt64` does **not** return 0 on unmapped memory, it
throws (killing the request, not the game); making the scan resumable across
those throws got about 800 nods in and hard-crashed the process. There is a
comment saying so in `ShootNod.as`.

## What "no clicks" now means

A cold start to a finished video, one command, nothing synthetic:

```
launch → EditMap → MediaTracker → stage ghosts → import each by name
       → create the camera track → aim it (ent=1, cam=2)
       → rewind → ShootVideo → read the params → OnOk → wait on the file
```

Measured end to end on 2026-08-22 from a killed game: setup 16 s, render
52.934 s of 1280×720 VP8 out in about three minutes, `shoot rc=0`. Repeated
from a second cold start with the same result.

Every gate is a fact from the game, never a duration and never a pixel:

| step | gate |
|---|---|
| map loaded / MediaTracker open | context number (`/ctx`) |
| ghost imported | ghost-block count before vs after |
| camera aimed | the camera block's `ClipEntId` and target name, read back |
| shoot dialog up | the `CGameDialogShootParams` nod itself |
| accepted | that nod going away |
| render finished | the file existing and its size unchanged |

The one thing still not gated on the game's own word is render completion —
`Operation_InProgress` reads false throughout a shoot, so the pipeline falls
back to the output file's size going stable.
