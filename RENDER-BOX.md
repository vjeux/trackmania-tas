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
it the `C:/...` spelling. `tools/ship-clip.sh` does this for you.

## Publishing a clip, end to end on this box

```
export PATH=$HOME/bin:$PATH
export GH_COOKIE="$(cat ~/.gh-upload/cookie)"
cd ~/trackmania-tas && tools/ship-clip.sh <clip.mp4> <map-dir>
```

That runs the whole chain: settle and probe the local file, upload the
full-quality original to the `videos-v1` release, upload to the
`user-attachments` store for the inline player, register the URL in the release
body (**this is the step that makes it public** — a pushed commit does not),
then fetch it back under `env -i` with no credential at all and require 200 and
playable bytes.

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
