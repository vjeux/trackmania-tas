# The render box

Everything that makes a clip — the game, the recorder, ffmpeg — runs on one
Windows machine ("WhiteStick") inside its WSL2 Ubuntu 22.04. This is the
snapshot of how that box is set up and what it can do on its own.

## Reaching it

It is not on the network directly. Commands go through a bridge binary on the
devserver:

```
~/bin/whitestick '<command>'
```

The command lands in the WSL distro as user `vjeux`, working directory
`/mnt/c/Users/vjeux`, **shell is `/bin/sh` (dash), not bash** — no herestrings,
no `[[`. `stdin is not forwarded`, so anything that needs input has to read a
file. Windows-side paths are visible under `/mnt/c/`; the Linux home is
`/home/vjeux`.

## What lives where

| | |
|---|---|
| repo checkout | `~/trackmania-tas` (Linux home — **not** under `/mnt/c`, which is slow and mangles modes) |
| toolkit | `~/trackmania-tas/tools/tmtraj`, built with `PATH=$HOME/.cargo/bin:$PATH cargo build --release` |
| rust | 1.97.1, already installed at `~/.cargo/bin` |
| `gh` | 2.63.2 at `~/bin/gh`, installed from the release tarball — **`sudo` needs a password on this box**, so nothing can be `apt install`ed |
| `liblzo2` | `/lib/x86_64-linux-gnu/liblzo2.so.2`, present — `tmtraj`'s GBX reader dlopens it |
| ffmpeg | Windows build at `/mnt/c/Users/vjeux/ffmpeg_extracted/.../bin/ffmpeg.exe` |
| replays | `/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Replays/` |

## Git: a deploy key, not an account token

The box pushes with an ed25519 **deploy key** generated on the box itself
(`~/.ssh/id_ed25519_tmtas`, titled `whitestick-render-box`, read-write). The
private half has never left the machine and no account-wide credential was
copied onto it: the key's blast radius is this one repo.

Port 22 is open from there, but the config uses `ssh.github.com:443` anyway
because it survives more networks:

```
# ~/.ssh/config  ->  Include ~/.ssh/config.tmtas
Host github-tmtas
  HostName ssh.github.com
  Port 443
  User git
  IdentityFile ~/.ssh/id_ed25519_tmtas
  IdentitiesOnly yes
```

Remote is `github-tmtas:vjeux/trackmania-tas.git`. Clone, commit, and push all
work unattended — verified by pushing a throwaway branch, deleting it, and
confirming `main` was the only remaining head.

To revoke: `gh repo deploy-key list --repo vjeux/trackmania-tas`, then
`gh repo deploy-key delete <id>`.

## The one thing it still cannot do: upload an inline video

Clips appear in the READMEs as `github.com/user-attachments/assets/...`. Those
URLs come from the drag-and-drop uploader, whose three-request dance needs the
**edit page's CSRF token, which is only served to a logged-in browser session**
(VIDEO-UPLOAD-NOTES §4). A deploy key cannot get one, and neither can a
personal access token — it is a web session or nothing.

So a clip made on this box still crosses to a machine with a live GitHub
session for its final step. Everything before that — render, encode, gate,
commit, push — is now local to the box.
