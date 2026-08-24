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
