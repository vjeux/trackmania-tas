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

## A record with an entity REMOVED kills the client on import

Measured on 227654 on 2026-08-24, six sessions, the unedited container importing
as a control before and after every reading:

* Re-encode a ghost's `CPlugEntRecordData` with every entity intact — even
  overwriting every car sample byte — and it **imports**.
* Remove **one** entity and the game is gone mid-import, `read: Connection
  reset by peer`. Any entity: a tail car, the scene record, or a placeholder
  that never had a sample. `ghost trim` does this whenever its window empties an
  entity, and its output then crashes the client.

The crash is a null-pointer read at `Trackmania.exe+0xd3788a`, in the
MediaTracker routine that formats `"Ghost:%1"` and makes one ghost block per
element of an array it never null-checks. `tools/wincrash` reads the WER
minidump and the PE; the Application event log has the fault offset for free
(`powershell.exe -Command "Get-WinEvent ..."`).

**To film a run whose own record the client refuses**, do not trim and do not
rebuild: write the samples into the container the client already accepts, with
`ghost record resample --all-cars --mixed-run`. That changes no entity, and it
is how The Blev Special was finally shot.

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
The Magnet Trial as half its evidence. The Magnet Trial has since been rendered
in full and it disproves that half.** What is left is smaller, still real, and
worth the same care.

### What is measured

| map | longest ghost STAGED | what the game actually wrote | the map's own author ghost |
|---|---|---|---|
| [Turtle Trial] Leto | 219.000 s (two ghosts, WR trimmed) | **441.000 s** | 441.002 s |
| The Magnet Trial | 793.893 s (one ghost) | **793.866 s** | 2540.641 s |

Leto is the finding: `clip cut` read the file the game wrote as
`441.000s -> 218.812s`, with both staged ghosts ending by 219.000. **A render
can be twice as long as anything in the scene, and everything past your run is
rendered and then thrown away.** Budget for it, and cut afterwards.

The Magnet Trial is the retraction. Its author ghost is 2540.641 s, its shoot
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
