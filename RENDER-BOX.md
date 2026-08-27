# The render box

Everything that makes a clip — the game, the recorder, ffmpeg — runs on one
Windows machine ("WhiteStick") inside its WSL2 Ubuntu 22.04. **It renders,
encodes, gates, uploads, commits and pushes on its own.** No other machine is
in the loop.

## Reaching it

It is not on the network directly. Commands go through a bridge binary on the
devserver:

```
~/bin/whitestick '<command>'
```

The command lands in the WSL distro as user `vjeux`, working directory
`/mnt/c/Users/vjeux`, **shell is `/bin/sh` (dash), not bash** — no herestrings,
no `[[`. **stdin is not forwarded**, so move a file in by base64 in the command
string and compare md5s at both ends. Windows-side paths are visible under
`/mnt/c/`; the Linux home is `/home/vjeux`.

## What lives where

| | |
|---|---|
| repo checkout | `~/trackmania-tas` (Linux home — **not** under `/mnt/c`, which is slow and mangles modes) |
| toolkit | `~/trackmania-tas/tools/tmtraj`, built with `PATH=$HOME/.cargo/bin:$PATH cargo build --release` |
| rust | 1.97.1 at `~/.cargo/bin` |
| `gh`, `jq` | `~/bin`, both installed from release tarballs — **`sudo` needs a password on this box**, so nothing can be `apt install`ed |
| uploader | `~/bin/ghvid.sh`, cookie at `~/.gh-upload/cookie` (mode 600) |
| `liblzo2` | `/lib/x86_64-linux-gnu/liblzo2.so.2`, present — `tmtraj`'s GBX reader dlopens it |
| ffmpeg | Windows build at `/mnt/c/Users/vjeux/ffmpeg_extracted/.../bin/`; there is **no Linux ffmpeg or ffprobe** |
| replays | `/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Replays/` |

`ffprobe.exe` cannot read a WSL path — stage the file under `/mnt/c` and hand
it the `C:/...` spelling. `clip ship` does this for you.

## Publishing a clip, end to end on this box

```
export PATH=$HOME/bin:$PATH
export GH_COOKIE="$(cat ~/.gh-upload/cookie)"
cd ~/trackmania-tas/tools && cargo run --release -p clip -- ship <clip.mp4> <map-dir>
```

That runs the whole chain: settle and probe the local file, upload the
full-quality original to the `videos-v1` release, upload to the
`user-attachments` store for the inline player, register the URL in the release
body (**this is the step that makes it public** — a pushed commit does not),
then fetch it back under `env -i` with no credential at all and require 200 and
playable bytes.

## Working from an on-demand box, and the six traps that keep costing an hour

Every trap below was hit and diagnosed by an agent, more than one of them by
three separate agents on the same day. None is interesting; all are expensive,
because a fresh box starts with none of this and the symptom never points at
the cause. Read this section before touching the box.

**1. `git clone` into `~/persistent` ALWAYS fails.** It dies with `premature end
of pack file`, at a different byte count each time, which reads as a flaky
network. It is not the proxy, `http.postBuffer`, HTTP/2, or `--filter`; three
agents tuned all of those. **Clone into `/tmp`** — it works first time, in
seconds — and copy artefacts to `~/persistent` afterwards. Work in `/tmp`, bank
to persistent.

**2. A fresh OD has no navi credential, so the bridge is dead.** `~/bin/whitestick`
is already installed and fails with *read navi credentials … (is navi-node set
up?)*. Copy the ~161-byte `~/.navi/credentials.json` from devvm42752 to
`$HOME/.navi/credentials.json` on the OD and it answers **directly** — devvm is
not a required hop, and one agent reinstalled the whole navi CLI before working
that out. Test with `~/bin/whitestick 'echo hello'` before anything else.

**3. The bridge cuts a command off at about 2 minutes.** A render or a slow
editor probe run inline looks like it DIED. Detach anything long on the box and
poll its log:

```
~/bin/whitestick 'nohup sh -c "<long thing>" > /tmp/x.log 2>&1 & echo started'
~/bin/whitestick 'tail -20 /tmp/x.log'
```

**4. `$HOME` on an fbsource OD is `/var/svcscm`, not `/home/vjeux`.** So
`~/persistent/...` is `/var/svcscm/persistent/...`. The map corpus
(`~/persistent/private-30d/tm-unbeaten/`) lives on the OD and is **not** on
devvm42752 — an agent planning to regenerate there finds an empty tree.

**5. `~/bin/whitestick` is a 2.4 MB stripped binary, not a shell script.**
`cat`ting it to see its interface dumps a megabyte into the transcript. Use
`--help`.

**6. Never build a Windows path with `printf`.** `printf` reads `\v` as a
vertical tab and **`\146` as an octal escape for `f`**, so
`\tas\146612.Map.Gbx` is silently handed over as `\tasf612.Map.Gbx` — a path to
a file that does not exist. Three "diagnoses" of Spaghetti Nights 2 were made
against that nonexistent path. Use a quoted heredoc, which interprets nothing.
The game also cannot open a `/home/...` path at all: stage under `/mnt/c` and
hand it the `C:/...` spelling.

## One game, one driver

The box runs a SINGLE Trackmania instance, and several agents drive it at once.
Two concurrent drivers do not fail — they SUCCEED WRONGLY: one's `setup` lands
its ghosts in the other's scene, and whichever `shoot` finishes second picks up
the other's `.webm`. Two plausible clips, at least one of the wrong run, and
nothing anywhere says so. It has already happened once, on 2026-08-24, and only
`shootctl setup`'s uid gate turned it into lost time rather than a wrong clip.

So take the mutex, every time, around the game-driving only:

```
shootctl lock acquire --owner <who> --wait 3600 --max-age 2700
  ... launch / setup / shoot / editor probes ...
shootctl lock release --owner <who>
```

Regenerate and verify OUTSIDE the lock — that is CPU on your own node and it is
where the time goes. Take it, run one bounded experiment, release, think, take
it again; never hold it while reading logs or editing code. `acquire` names the
holder and its age when it refuses. `--max-age 2700` breaks a 45-minute-old
lock, which means a dead agent, and says so loudly. Never delete the lock
directory by hand.

## A record with too FEW entities kills the client on import

Measured on 227654 on 2026-08-24, ten sessions, the unedited container importing
as a control before and after every reading:

* That file's record must hold **at least 29 entities**. 28 crashes the game
  mid-import (`read: Connection reset by peer`); 29 imports. **Which entities
  they are does not matter** — 29 copies of one entity of ours does it, and the
  container with one entity dropped and another duplicated back to 29 imports
  while still missing the one that was dropped.
* So a rebuilt record (1 entity) dies, and so does anything `ghost trim` leaves
  when its window empties entities.
* **The number is per-file or per-map, not a constant**: on 279209 the same
  container survives being cut to 2 entities.

The crash is a null-pointer read at `Trackmania.exe+0xd3788a`, in the
MediaTracker routine that formats `"Ghost:%1"` and makes one ghost block per
element of an array it never null-checks. `tools/wincrash` reads the WER
minidump and the PE; the Application event log has the fault offset for free
(`powershell.exe -Command "Get-WinEvent ..."`).

**To film a run whose own record the client refuses**: `ghost record ents IN OUT
--pad N` appends duplicate entities until the count is N and changes nothing
else, and `ghost record resample --all-cars --mixed-run` puts our samples into a
container that already passes. The Blev Special was finally shot with the first.

### ...and on Haystack 6 the padding did NOT fix it. It is the CONTAINER.

Measured 2026-08-26 on `ONCE278.Ghost.Gbx` (the 278-move Haystack 6 route),
eleven client launches, every reading with its own control. The import crash is
the same `Trackmania.exe+0xd3788a`, and **`--pad N` does not touch it**:

| file | entities | samples | verdict |
|---|---|---|---|
| `ONCE278` unpadded | 1 | 1 | crash |
| `--pad 2 / 3 / 5 / 9 / 17 / 29` | 2..29 | 1 | **crash, every one** |
| regridded to the run's span, unpadded | 1 | 28259 | crash |
| ...`--pad 29` | 29 | 28259 | crash |
| ...`graft-scene --from` a game ghost | 3 (1 live non-vehicle) | 28259 | crash |
| ...trimmed to 30 s / 120 s / 600 s | 1 | 601..12001 | crash |

So it is not the entity count, not the sample count, not the record span, not
the missing non-vehicle record (V11's repair), and not the length.

**The two controls that located it**, same session, same map file:

| | verdict |
|---|---|
| Haystack 6 + a game-recorded ghost (`279218_WR_5355`) | **imports**, camera written, scene ready |
| the control map + our `ONCE278`-shaped file | **crash** |

The map is innocent and the client renders Haystack 6 fine. The file is guilty
on any map. `ghost chunks` says why in one line:

```
ours  body 830639 B,  6 skippable chunks
WR    body  12941 B, 24 skippable chunks
```

Our synthesised container carries 6 of the 24 chunks the game writes —
`0x0303F007`, `0x03092008`, `0x0309200A/B`, `0x03092013`, `0x0309201A/B` and
the whole `0x03092022`..`0x03092029` run are simply absent. The MediaTracker
routine at `+0xd3788a` reads one of them and does not null-check it.

**The fix is the container, and it is remedy 2 from the Blev Special, not
remedy 1**: put the run in a container the client already accepts.

```sh
ghost record rebuild <a game-recorded ghost> grid.Ghost.Gbx --span <run ms> --period 50
ghost record from-csv grid.Ghost.Gbx SHOT.Ghost.Gbx --csv <trajectory>.csv
```

`ghost record from-csv` is new and exists for this: it writes a trajectory
CSV's position, orientation and velocity into a container's car record, instant
for instant, and reads the file back before it claims to have done it. The
control that the transplant is the run: `tmtraj export` the written file and
`tmtraj csvdiff` it against the trace — on Haystack 6, **28235 shared instants,
position median / p95 / max all 0.000 m**.

The regridded donor is worth keeping as a control of its own: a game-recorded
container **regridded to 1412.900 s and 28259 samples imports cleanly**, which
is what rules out the length and the sample count in the table above.

**`ghost regen` cannot rebuild a DNF run's record on its own.** Step 0 asks the
oracle for the run's simulated time so it can regrid the record; on a DNF it
says "the span is left as it is" and passes the template through untouched — so
on a tape-only ghost (1 sample at t=0) all 24 attempts regenerate into a
one-sample grid. Run `ghost record rebuild --span <tape ms>` yourself first.
And its in-process locate is the wrong tool for a route that starts stationary:
on this tape it spent **14 minutes** before reporting "this tape is probably not
moving at the handover". `--no-inprocess` skips it.

## Credentials on this box, and their blast radius

| what | scope | used for |
|---|---|---|
| deploy key `~/.ssh/id_ed25519_tmtas` | **this repo only**, read-write | clone, commit, push |
| `gh` OAuth token | **the whole account** (`repo`, `gist`, `workflow`, `read:org`) | release upload / release body edit |
| session cookie `~/.gh-upload/cookie` | **the whole account, as a logged-in browser** | the `user-attachments` upload |

The deploy key was generated on this box and its private half has never left
it. The other two were copied here and are account-wide; the cookie in
particular is a live browser session. Revoke, in order of how much they can do:
`gh auth logout` and delete the cookie file, then
`gh repo deploy-key list --repo vjeux/trackmania-tas` and `... delete <id>`.

**The cookie expires.** When `ghvid.sh` exits 3 with *no upload CSRF token — is
the cookie still valid?*, that is what has happened: replace
`~/.gh-upload/cookie` with a fresh `Cookie:` header from a logged-in browser.
Nothing else in the pipeline needs renewing.

## The render can be longer than everything you staged — and I got the reason wrong once

**RETRACTED, and kept in place. The section here previously read "the
MediaTracker's clip length is the MAP's author ghost, not the scene's" and gave
186935 `[object Object]` as half its evidence. That map has since been rendered
in full and it disproves that half.** What is left is smaller, still real, and
worth the same care.

### What is measured

| map | longest ghost STAGED | what the game actually wrote | the map's own author ghost |
|---|---|---|---|
| [Turtle Trial] Leto | 219.000 s (two ghosts, WR trimmed) | **441.000 s** | 441.002 s |
| [object Object] | 793.893 s (one ghost) | **793.866 s** | 2540.641 s |

Leto is the finding: `clip cut` read the file the game wrote as
`441.000s -> 218.812s`, with both staged ghosts ending by 219.000. **A render
can be twice as long as anything in the scene, and everything past your run is
rendered and then thrown away.** Budget for it, and cut afterwards.

186935 `[object Object]` is the retraction. Its author ghost is 2540.641 s, its shoot
dialog estimated **01:45:52** for a thirty-second slice, and 2540.641 s of video
at the ~2.5x realtime this box renders at is 6352 s, which is 01:45:52 to the
second. That arithmetic is exact and the conclusion drawn from it was still
wrong: rendered in full at `--cam 1`, the map produced **793.866 s** — its own
run's length — in about forty minutes.

### The lesson, which is not about MediaTracker at all

**The shoot dialog's `estimated` is not a measurement of the clip's length, and
I treated it as one.** No slice render was ever allowed to finish; every one was
killed early, so the only number behind "2540 s" was an estimate that happens to
divide neatly. An arithmetic coincidence that lands on the second is *more*
persuasive than a rough one and no more evidential. The one number that came
from a finished file — Leto's 441.000 — is the only part that survived.

If you need the length of a render before paying for it, the honest instruments
are a finished file, or `clip frames --stream` on a partial one, which is what
the camera comparison on this map used.

### `/clipend` does not work on this build

`shootctl get /clipend?ms=N` answers **`could not resolve
CGameCtnMediaBlock::End`**: `MemberOffset` does not find that member in this
build's class dump. The route and its implementation are both in HEAD
(`Main.as`, `Camera.as:79`), and the member lookup is what fails, so a caller
gets a 200 with an error string in it rather than a shortened clip — the dialog
still estimated 01:45:52 immediately afterwards. Not chased further; recorded so
nobody re-treads it.

## 2026-08-24: the box lost its launcher, and three things that misled me on the way

**Trackmania will not start at all when Ubisoft Connect is unhealthy**, and the
symptom points nowhere near the launcher: the game process appears and is gone
within two seconds, `OpenplanetHook.log` does not grow by a byte, and
`shootctl launch` reports *"Openplanet hung on the Nadeo login"* three times and
gives up. The launcher had crash-looped (`upc.exe_169.6.13045_..._09-41-13.dmp`)
behind a modal error dialog nobody could see.

**The fastest instrument for "the game will not start" is a screenshot of the
desktop.** Two modal dialogs — a Ubisoft Connect crash box and a Visual C++
runtime error — explained an hour of log-reading in one image. Take the
screenshot first next time.

**The controls that mattered**, each of which killed a hypothesis:

| hypothesis | control | verdict |
|---|---|---|
| Openplanet's `dinput8.dll` proxy kills the game | move it aside, launch again | still dies — **not Openplanet** |
| the launch inherited `PreferSystem32=ON` | read the loaded module path | the process I measured was **my own stray shell launch**, not the Explorer one |
| the session is locked (capture came back blank white) | capture with the game killed | **also white** — the game was holding the display; `LogonUI` absent, DWM up |

### `shootctl`'s "Openplanet hung on the Nadeo login" can be a lie

`openplanet_stage()` reads `Openplanet.log` and infers a login stall from
"started but no Loop entry". When the game dies *before* Openplanet writes
anything, that file is STALE — it still holds the last run's header — so the
driver reports a login stall for a game that never got as far as loading the
DLL. The honest signal is `OpenplanetHook.log` **growing**; if its byte count is
unchanged after a launch, Openplanet never attached and the login has nothing to
do with it. Worth fixing in the driver; recorded here so the next reader does
not spend ten minutes where I did.

### This desktop is at 150 % DPI and is really 3840x2160

Any script that clicks, reads a window rect, or maps a screenshot coordinate to
the screen **must** call `SetProcessDpiAwarenessContext(-4)` before anything
else. Without it `GetWindowRect` and `MoveWindow` speak *logical* pixels while
`CopyFromScreen` gives *physical* ones, everything is 1.5x out, and clicks land
in empty space — the form fills silently do nothing and `^a` selects the page
text instead of a field. Screenshot after every field so you see what landed;
that is the only reason this was caught rather than becoming a mystery.

### What is still broken as of 2026-08-24 19:00 UTC

Ubisoft Connect accepts the credentials (`ConnectSecureStorage.dat` grew
5151 -> 7228 B, `user.dat` rewritten, no 2FA, no captcha) and then fails at
**`dolphin-034`**, deterministically, across three clean restarts and a cache
clear. Installed build is **169.6.13045**; a **173.0.0.13316** installer was
downloaded into the Trackmania folder at 03:55 that day and never applied — a
mandatory update that failed, most likely because C: was at 99 %. Applying it
needs a UAC prompt and that Windows session had gone non-interactive, so it
cannot be done over the bridge. **A human has to run that installer.**

`GW523-READ-ME-FIRST.txt` and `gw523_go.sh` in the box's home are the hand-off:
Great Wtf of What #523's progress clip at 9.024 is staged, verified and one
command away from being rendered.

## 2026-08-26: the map must live under the Trackmania user tree

`EditMap`/`PlayMap` load a map **only from a path underneath
`C:/Users/vjeux/OneDrive/Documents/Trackmania/`**. Anywhere else — `C:/tmshoot/`,
`C:/Users/vjeux/h6/`, any `/home/...` — the call returns `ok` and **nothing
happens, forever**. It is not the file: one map, one md5, same game session:

| path | result |
|---|---|
| `C:/Users/vjeux/h6/control.Map.Gbx` | ctx stays 0 |
| `C:/tmshoot/control.Map.Gbx` | ctx stays 0 |
| `.../Trackmania/Maps/My Maps/control.Map.Gbx` | **opens** |
| `.../Trackmania/Maps/_shoot/control.Map.Gbx` (dir made after boot) | **opens** |

The last row rules out startup indexing. Stage maps in `Maps/_shoot/`, the way
`gw523_go.sh` always did.

**The signal that says so is `latestResult`, and it is in `/ready` already.**
`Openplanet.h` (search `CGameManiaTitleControlScriptAPI::EResult`) decodes it:
`0` Success, `1` Error_Internal, **`2` Error_DataMgr**, then the network ones.
A fresh boot reads 0; a rejected `EditMap` flips it to 2 within a second. Three
sessions reported `"latestResult":2` verbatim without decoding it and went
looking at Nadeo, the GPU and the title pack instead. **Decode it first.**

## 2026-08-26: the bridge daemon can fill the disk by itself

`navi-node` — the WhiteStick bridge — crash-loops with SIGABRT, and it holds
~74 GB of virtual address space, so WSL writes a **~6.7 GB core dump per crash**
into `%LOCALAPPDATA%\Temp\wsl-crashes`. On 2026-08-26 ten dumps took C: from
"nearly full" to **1.1 MB free in three hours**, and the game then failed in
ways that point nowhere near a disk: `cp` returning *Input/output error*, a map
copied into `Maps/` silently **truncated**, `shootctl launch` reporting
*"Openplanet hung on the Nadeo login"* twice out of three, and an
`Updating data...` dialog stuck at 0.

**`df -h /mnt/c` is the first command of any WhiteStick investigation.**

Guards now in place: `C:\Users\vjeux\.wslconfig` sets `crashDumpCount=0` (needs a
`wsl --shutdown` to take effect, which kills the bridge — so it lands at the
next restart), and an hourly cron runs `~/bin/prune-wsl-crashes.sh`, deleting
dumps over 200 MB. The SIGABRT itself is **not fixed**; three small dumps are
kept in that folder for whoever wants it.

C: is 937 GB and ~97 % of it is games (Steam 196, Diablo IV 82, WoW 77) — that
is vjeuxs,
## 2026-08-26: the map must live under the Trackmania user tree

`EditMap`/`PlayMap` load a map **only from a path underneath
`C:/Users/vjeux/OneDrive/Documents/Trackmania/`**. Anywhere else — `C:/tmshoot/`,
`C:/Users/vjeux/h6/`, any `/home/...` — the call returns `ok` and **nothing
happens, forever**. It is not the file: one map, one md5, same game session:

| path | result |
|---|---|
| `C:/Users/vjeux/h6/control.Map.Gbx` | ctx stays 0 |
| `C:/tmshoot/control.Map.Gbx` | ctx stays 0 |
| `.../Trackmania/Maps/My Maps/control.Map.Gbx` | **opens** |
| `.../Trackmania/Maps/_shoot/control.Map.Gbx` (dir made after boot) | **opens** |

The last row rules out startup indexing. Stage maps in `Maps/_shoot/`, the way
`gw523_go.sh` always did.

**The signal that says so is `latestResult`, and it is in `/ready` already.**
`Openplanet.h` (search `CGameManiaTitleControlScriptAPI::EResult`) decodes it:
`0` Success, `1` Error_Internal, **`2` Error_DataMgr**, then the network ones.
A fresh boot reads 0; a rejected `EditMap` flips it to 2 within a second. This
investigation quoted `"latestResult":2` verbatim for half an hour, and went
looking at Nadeo, the GPU and the title pack, before decoding the one number
that was already in every reply. **Decode it first.**

### And the control has to differ in the variable under test

The control map that "also failed" had been copied into the SAME directory as
the subject. It shared the defect with the thing it was controlling, so
"the control fails too" read as *the machine is broken* when it meant
*the location is guilty*. A control that differs in every variable except the
one under test is not a control.

## 2026-08-26: the bridge daemon can fill the disk by itself

`navi-node` — the WhiteStick bridge — crash-loops with SIGABRT, and it holds
~74 GB of virtual address space, so WSL writes a **~6.7 GB core dump per crash**
into `%LOCALAPPDATA%\Temp\wsl-crashes`. On 2026-08-26 ten dumps took C: from
"nearly full" to **1.1 MB free in three hours**, and the game then failed in
ways that point nowhere near a disk: `cp` returning *Input/output error*, a map
copied into `Maps/` silently **truncated**, `shootctl launch` reporting
*"Openplanet hung on the Nadeo login"* twice out of three, and an
`Updating data...` dialog stuck at 0.

**`df -h /mnt/c` is the first command of any WhiteStick investigation.**

Guards now in place: `C:\Users\vjeux\.wslconfig` sets `crashDumpCount=0` (needs a
`wsl --shutdown` to take effect, which kills the bridge — so it lands at the
next restart), and an hourly cron runs `~/bin/prune-wsl-crashes.sh`, deleting
dumps over 200 MB. The SIGABRT itself is **not fixed**; three small dumps are
kept in that folder for whoever wants it.

C: is 937 GB and ~97 % of it is games (Steam 196, Diablo IV 82, WoW 77) — those
belong to vjeux, do not touch them. Reclaimable if ever desperate: 13.8 GB of
Windows Update cache, and ~63 GB of slack inside the 106 GB `ext4.vhdx`
(compacting needs `wsl --shutdown`, i.e. somebody at the console).

### The screenshot tool under-captures by a third

`shot.ps1` reads `Screen.PrimaryScreen.Bounds` with no DPI awareness, so at
150 % it saves a 2560x1440 image of the **top-left corner** of the 3840x2160
screen — the player card and the whole right side are simply missing, and the
menu looks half-initialised. `shotdpi.ps1` (beside it, written 2026-08-26)
calls `SetProcessDpiAwarenessContext(-4)` and `GetSystemMetrics` first, and
captures the real thing. Use that one.

### `shootctl get /editmap?path=...` ignores the query string

The plugin reads the path from `editmap.txt` in its storage folder, so a
hand-run `/editmap` silently uses whatever the last `probe`/`run` left there
and echoes it back — which reads exactly like it honoured your argument.
Use `shootctl probe --map`.

### GW523-READ-ME-FIRST.txt is stale

It says a human must run `UbisoftConnectInstaller.exe` before anything renders.
Not true any more: the game runs from the **Steam** copy
(`C:\Program Files (x86)\Steam\steamapps\common\Trackmania\Trackmania.exe`,
2026.2.2.1751), is signed in as VJEUX and online, and rendered fine on
2026-08-26.
