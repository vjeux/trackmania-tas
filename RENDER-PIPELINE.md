# The render pipeline

One command takes a killed game to a finished video:

```
shootctl run --map <map.Map.Gbx> --name <output> <tas.Ghost.Gbx> [opponent.Ghost.Gbx]
```

**No clicks, no keystrokes, no sleeps — anywhere in it.** Every wait is a fact
from the game or the OS, and every step is proved before the next one starts.

Measured cold start, 2026-08-22: launch 11.5 s, scene 12 s, render 121 s for
52.934 s of 1280×720 VP8.

## What runs it

* `tools/shootctl` — Rust, std only, no dependencies. The whole driver: launch,
  scene setup, render, and the plugin dev loop.
* `tools/openplanet-plugin/` — the Openplanet plugin (AngelScript, because that
  is Openplanet's language; everything on our side of the line is Rust).

`shootctl install` is the dev loop (lint → reload → prove); `shootctl save`
records the current plugin as the one to fall back to.

## Waiting without sleeping: `/await`

This is the mechanism the rest of the document rests on. `HttpServer` calls its
request handler from inside a coroutine, so **`yield()` in a handler returns
control to the game and resumes on the next frame**. A route can therefore
simply not answer until a condition holds. The driver makes one HTTP call and
blocks on the socket: nothing polls, nothing sleeps, the answer arrives on the
frame the thing actually happened, and the reply says how many milliseconds and
frames it took.

```
/await?c=ctx:2        the MediaTracker is open
/await?c=ready        ManiaTitleControlScriptAPI::IsReady
/await?c=ghosts:2     two ghost blocks in the clip
/await?c=shootdlg     the shoot dialog exists     (…noshootdlg: it is gone)
/awaitfile            the game has closed the output file
```

Two rules learned immediately:

* **Conditions use a colon, never `=`.** The plugin's query splitter does no URL
  decoding, so `ctx%3D0` arrives literally, matches nothing, and times out
  looking exactly like "the game never got there".
* **An unknown condition is rejected up front**, not waited on. A typo that
  waits out the deadline and reports `ok:false` sends you debugging the game.

## Launching

`launch_tm.sh` took 38 seconds and almost all of it was guesses: `sleep 6` after
the kill, a 3-second process poll, a **blind `sleep 25`** for the splash, a
**synthetic Enter** to clear it, then a 5-second `/ping` poll. Measured with all
of that removed: **the plugin answers 11.9 s after the kill, and the Enter was
never needed** — the game reaches the menu by itself.

Nothing in the replacement sleeps. Each wait is a blocking operation whose
duration *is* the wait: `tasklist` for the processes, and `connect()` for the
plugin — a WSL connect to a Windows port with nothing listening is dropped
rather than refused, so it blocks its full timeout and paces the retry itself.

**Why the launch goes through Explorer.** Openplanet is a `dinput8.dll` proxy
beside `Trackmania.exe`. The `PreferSystem32` process-creation mitigation makes
the loader take System32's real `dinput8.dll` instead — the game runs perfectly
and the plugin never loads. The mitigation is *inherited*, and every shell we
have has it ON while Explorer has it OFF, so `explorer.exe <path>` gets the
running Explorer to create the process. (Measured 2026-08-21: launched from cmd,
the game loads `SYSTEM32\DINPUT8.dll`; launched via Explorer, `<gamedir>\DINPUT8.dll`.)

**And the title must be READY.** `EditMap` on a not-ready
`ManiaTitleControlScriptAPI` returns without error and loads nothing — that is
the failure that reads as "the map did not open".

## The scene

The map is loaded by absolute path (`EditMap`), so it needs no folder at all.
Ghosts are staged into `Replays/_shoot/`, which holds exactly the files for this
render — nothing to page through, nothing to mis-index. A leftover `old/`
subdirectory inside the picker's folder once shifted every row and imported the
wrong car; the isolated folder ends that class of bug.

| was | now |
|---|---|
| click EDIT at (3343, 2075), then `DialogEditCutScenes_OnInGameEdit`, 3 retries | `CGameEditorPluginMap::EditMediatrackIngame()` |
| four menu clicks and a tile hunt to load a map | `ManiaTitleControlScriptAPI::EditMap(path)` |
| click "Tracks +", "Player camera", then cycle a button up to 12× re-reading it | `Dev::SetOffset` on `ClipEntId` / `GameCam`, offsets resolved **by name** at runtime |
| click rows in a 12-row paged picker, twice | `DialogSaveAs_Path` + the import API below |

The old `DialogEditCutScenes_OnInGameEdit` call is worth calling out: it is a
dialog button handler, so with no dialog up it did nothing at all and returned
"ok". The shell covered for it with three retries and 13 s of sleeps.

### The ghost import

```
ImportGhosts()                                   raise the file dialog
DialogSaveAs_Path = "_shoot/1_TAS.Ghost.Gbx"     RELATIVE to Replays/
DialogSaveAs_OnValidate()                        accept the file
ImportGhosts_OnOk()                              perform the import
DialogSaveAs_OnCancel()                          close the dialog it leaves open
```

Three things there cost hours each.

**Leave out `ImportGhosts_OnOk` and the dialog closes having done nothing.** No
error, no ghost. That is what "no ghost imported" looked like.

**`ImportGhosts_OnOk` leaves `FrameDialogSaveAs` OPEN** — invisibly, because the
MediaTracker draws over it — and it holds keyboard focus. Nothing downstream
could accept a dialog while it was there. Cancelling afterwards is safe: the
ghost is already in the clip, and the import reports the ghost-block count
before and after, so success is never inferred.

**The path must be relative to `Replays/`.** A full `C:/...` path is accepted by
the field, closes the dialog, and imports nothing.

## The shoot dialog

The last click, and the hardest thing here to find. The dialog is a
`CGameDialogShootParams`, whose `OnOk` is exactly what the OK button calls — but
**nothing in the game's entire class dump declares a member of that type**, so
no walk over declared members reaches it at any depth.

**It is a frame of the GAME dialog menu.** `CGameCtnMenus` carries four
`CGameMenu`s — `Menus`, `InGameDialogs`, `Dialogs`, `SystemDialogs` — and the
shoot dialog lives in `Dialogs` (MenuOrder 5) as `FrameDialogShootVideo`. Every
earlier search looked in `BasicDialogs.Dialogs`, MenuOrder 11: a different menu.
`CGameMenu::CurrentFocusedControl` hands over the focused control and
`CControlBase::Nod` is the dialog nod it is bound to:

```
game: frames=43 current=FrameDialogShootVideo
      focus=EnumFileFormat [CControlEnum] -> CGameDialogShootParams
```

So the accept is `sp.OnOk()`, and the settings read back as numbers first —
`{"fps":30,"w":1280,"h":720,"ext":1,"hq":true,"estimated":"00:07:04"}` — worth
checking before committing to a render that takes minutes.

### Why not a keystroke

Because it silently corrupts the output. **The dialog does not open with OK
focused — it opens on `EnumFileFormat`**, so an Enter sent at it cycles the file
format. That is how a render came out as AVI when nobody asked for one. Also
recorded, since it looks like a bug otherwise: `wscript.shell` `SendKeys` does
nothing here at all (it posts window messages; Trackmania reads raw input), so
only `SendInput` with `KEYEVENTF_SCANCODE` reaches the game — which makes a
stray keystroke both possible and invisible.

### The route that must never be tried again

**A memory scan for the dialog nod crashes the game.** Twice, on two different
implementations — including one that filtered candidates to aligned pointers
whose target begins with a vtable inside the module range and read them through
`Dev::SafeReadUInt64`. Findings from that attempt, so it need not be repeated:
`Dev::SafeReadUInt64` does **not** return 0 on unmapped memory, it throws
(killing the request, not the game); making the scan resumable across those
throws got about 800 nods in and hard-crashed the process. There is a comment
saying so in `ShootNod.as`.

## Knowing when the render is finished

The game will not tell you. Measured across a whole 53-second render at
3-second intervals: `Operation_InProgress` false throughout, `MTApi::IsPlaying()`
false, `CurrentTimer` 0, `PlaySpeed` 1, no dialog, no progress bar, no menu
frame. There is no render-in-progress signal in the object graph at all.

**The encoder's file handle is the signal.** The game holds the output open
while it writes, so a read-open that also denies writers fails until it closes.
That is the writer's own release — exact, and it distinguishes a finished render
from a stalled one, which "the size has not changed for three polls" cannot.

The check runs **in the plugin**, once per frame (`/awaitfile`). It began as a
PowerShell call from the driver, which meant spawning a process four times a
second for the whole render, competing with the encoder for the machine; moving
it in-game took the render from 124 s to 121 s and made the wait a long poll
like everything else. `IO::FileMode::Read` never truncates — the obvious Write
mode would destroy the render it is waiting for.

## Finding the output file

Three traps, all measured, all of them silent.

**`ShootName` does not reliably name the file.** It is writable and reads back —
the dialog reported `"name":"uw_deck_v1"` — and the game wrote `Video54.webm`
off its own counter anyway. On a later run with identical code it *did* honour
the name. Both cases have to work.

**The game OVERWRITES existing files.** It takes the lowest free `VideoNN`, and
if that name already exists it rewrites it in place: `Video56.webm` was
overwritten and the folder listing never changed length. So "wait for a new
filename to appear" hangs on a render that is running perfectly, and "the newest
`.webm`" is a guess that lands on the wrong file whenever anything else has
written one. What works: note the time before accepting, then take the one
`.webm` whose mtime is newer than that — one file, or it is an error.

**Never copy a file onto itself.** Because the game sometimes honours the name,
the "keep a named copy" step can have identical source and destination, and
`fs::copy` then TRUNCATES. It destroyed a finished 24 MB render, and the only
reason it was caught is that the tool printed `done: 0 bytes`. The paths are now
compared by asking the filesystem, not by comparing strings.

## Every gate

| step | gate |
|---|---|
| game gone | the process list |
| plugin injected | a TCP connect that succeeds |
| title usable | `ManiaTitleControlScriptAPI::IsReady` |
| map loaded / MediaTracker open | the context number |
| ghost imported | ghost-block count before vs after |
| camera aimed | the camera block's `ClipEntId` and target name, read back |
| shoot dialog up | the `CGameDialogShootParams` nod itself |
| accepted | that nod going away |
| render running | the one `.webm` touched since we started |
| render finished | the encoder closing its file handle |

## The dev loop

`Meta::ReloadPlugin` (found in the strings of `Openplanet.dll`) lets the plugin
reload itself: write the file, `shootctl install`, done in about a second.

What actually costs a 90-second restart is a **compile error** — Openplanet
unloads the plugin, the HTTP server goes with it, and there is no `/reload` left
to call. So `shootctl lint` checks the source against the game's own class dump
(`OpenplanetNext.json`) before the game sees it. Every rule in it exists because
that mistake cost a restart:

```
unknown class: CGameDialogFileBrowser
CGameCtnMediaBlockCameraGame.GameCam is const -- 'the property has no set accessor'
route calls undefined function: DumpDialogLists()
'out' is a reserved word and cannot be a variable name
CGameDialogShootParams has no EExtVideo (it declares no named enum -- the dump calls it UnnamedEnum)
```

`install` refuses when the linter objects, and if a reload fails anyway it
restores the last good plugin and relaunches the game by itself. A rule that
fires on correct code gets deleted: an earlier version of the reserved-word
check flagged eleven good lines, and a linter that cries wolf is worse than none.

## Two networking traps worth keeping

**The plugin must bind `0.0.0.0`, not `127.0.0.1`.** The game runs on Windows;
the driver is a Linux binary in WSL, and WSL's loopback is a different machine.
Bound to loopback, a Windows `curl.exe` reported the plugin healthy one line
before the driver got "connection refused".

**Never cache a guessed address.** The driver picks the first candidate address
that answers — and the first version cached the `127.0.0.1` *fallback* when
nothing answered. A `run` that probed while the game was still starting locked
itself to the WSL loopback and dialled it for three minutes while the plugin sat
answering on the real address. The cache is now written only on success.

(`/proc/net/route` stores the gateway little-endian, so its bytes are already in
network order. Reversing them produced `1.0.18.172` and another connection
refused that looked exactly like the plugin being down.)
