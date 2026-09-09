# Publishing the tiny clips from vjeux's Mac

Written 2026-09-09, when the render box's bridge went down with thirteen
finished clips staged and the only GitHub session that GitHub accepts living in
vjeux's own browser. **Nothing here builds or runs a compiled binary** — the
Mac's antivirus kills freshly built ones (`SIGKILL`, rc 137), so this is shell,
`curl`, a system `gh`, and `ffprobe`. A Homebrew/system install is fine; a
binary copied off a devserver is not.

Everything is already made: each clip below is cut to its lap, carries the
controls overlay, has had its timing checked against the picture, and is stamped
so `clip ship` (and the script here) can prove it. Publishing is the only step
left.

## 1. What to publish (13 clips, 695.5 MB)

The newest lap of each map whose page row does not yet show it — or shows it
without the overlay. From the shared store,
`~/persistent/private-30d/tm-player/tiny/videos-ship15/`:

| map | page shows now | this clip | size |
|---|---|---|---|
| 01 | 19.381 | `01-ghost-17.417-ship15.mp4` | 41.7 MB |
| 03 | 28.989 | `03-ghost-19.793-ship15.mp4` | 33.3 MB |
| 04 | 29.474 | `04-ghost-18.476-ship15.mp4` | 38.8 MB |
| 06 | 23.038 | `06-ghost-22.653-ship15.mp4` | 45.3 MB |
| 10 | 25.304 | `10-ghost-24.552-ship15.mp4` | 42.9 MB |
| 11 | 34.348 | `11-ghost-34.291-ship15.mp4` | 50.6 MB |
| 12 | 20.752 | `12-ghost-20.514-ship15.mp4` | 38.5 MB |
| 16 | 39.302, no overlay | `16-ghost-39.302-ship15.mp4` | 52.2 MB |
| 17 | 50.479 | `17-ghost-38.426-ship15.mp4` | 59.3 MB |
| 21 Argentina | *no video* | `21-ghost-122.892-ship15.mp4` | 74.2 MB |
| 22 Saudi Arabia | *no video* | `22-ghost-99.912-ship15.mp4` | 75.2 MB |
| 23 Norway | 115.244, no overlay | `23-ghost-115.244-ship15.mp4` | 74.0 MB |
| 24 Poland | 149.593, no overlay | `24-ghost-149.593-ship15.mp4` | 69.5 MB |

Laps certified after these were cut — 09 28.292, 15 49.825, 18 45.333,
19 46.857, 21 122.510, 23 102.541, 24 147.654, 25 119.588 — need a RENDER first,
which needs the render box. The page already says so under each row
(`latest lap … — video pending`).

## 2. Fetch them

The store is a Manifold mount; the Mac does not have it. Copy from a box that
does — `devvm65800` has both the store and the clips (**not** `devvm42752`: its
user x509 expired at 20:32Z on 2026-09-09, so its mount answers I/O errors).

```sh
mkdir -p ~/tinyclips && cd ~/tinyclips
scp devvm65800.cln0.facebook.com:'~/persistent/private-30d/tm-player/tiny/videos-ship15/{01-ghost-17.417,03-ghost-19.793,04-ghost-18.476,06-ghost-22.653,10-ghost-24.552,11-ghost-34.291,12-ghost-20.514,16-ghost-39.302,17-ghost-38.426,21-ghost-122.892,22-ghost-99.912,23-ghost-115.244,24-ghost-149.593}-ship15.mp4' .
ls -la           # 13 files, ~700 MB
```

If that box is gone, any OD can serve them: `mkdir -p ~/persistent &&
persistent-storage mount private-30d`, then scp out of
`~/persistent/private-30d/tm-player/tiny/videos-ship15/`.

## 3. What the Mac needs

| thing | check | if missing |
|---|---|---|
| `ffprobe` | `ffprobe -version \| head -1` | `brew install ffmpeg` |
| `gh`, logged in | `gh auth status` | `brew install gh && gh auth login` |
| the repo | `cd ~/trackmania-tas && git pull` | `git clone git@github.com:vjeux/trackmania-tas.git` |
| `ghvid.sh` | it is in the repo: `tools/tinyctl/box/ghvid.sh` | — |
| the browser cookie | see below | — |

**The cookie.** `ghvid.sh` posts to the same private upload endpoint the web
editor uses, so it needs the `Cookie:` header of a logged-in github.com tab:
DevTools → Network → any github.com request → Request Headers → copy the whole
`cookie:` value into `~/.gh-upload/cookie` (one line, `chmod 600`).

> ⛔ **That file is INPUT. Nothing here writes it.** On 2026-09-09 a tool of
> mine wrote a cookie jar back over it and replaced a live session with 33 bytes
> of `_octo`, costing a renewal. If a step ever offers to "refresh" it, it is
> broken.

Sessions have been dying after a handful of uploads all day. Two things seem to
help and neither is proven: don't sign in to GitHub again while the queue runs
(a new sign-in appears to invalidate the one in use), and leave a gap between
uploads. The script does one clip per run so you control the pace.

## 4. Publish one clip

```sh
cd ~/trackmania-tas
sh tools/tinyctl/box/tiny-publish.sh ~/tinyclips/22-ghost-99.912-ship15.mp4 tiny/README.md
```

It does exactly what `clip ship` does, refusing at each step:

1. **the overlay** — the clip's `comment` tag must read `tas-overlay v1 …`, else
   it stops (exit 3). A clip without the overlay is not published, ever.
2. **upload** to user-attachments via `ghvid.sh` → the inline-player URL.
3. **register** that URL in the `videos-v1` release body — *this* is what makes
   an attachment public; a pushed commit does not. (19 clips were shipped before
   that was learned and 18 were 404 to everybody but their author.)
4. **the anonymous gate** — fetch it back with `env -i /usr/bin/curl`, no
   cookie, no token, and require 200 with real bytes. Retries for ~2.5 min,
   because registration takes up to ~45 s to propagate.
5. **the page** — swaps the map's line to this lap with `(build ship15, controls
   overlay)`, replaces its video, drops its "video pending" line, commits and
   pushes. `NO_PUSH=1` stops before the push; `DRY=1` only checks the file.

Try one first (`DRY=1 sh tools/tinyctl/box/tiny-publish.sh ~/tinyclips/22-*.mp4`
prints the overlay marker and the size and touches nothing), then do 22 for
real — it is a map with no video at all, so it is the most visible win.

## 5. Publish the rest

One at a time, stopping if anything fails:

```sh
cd ~/trackmania-tas
for f in ~/tinyclips/*.mp4; do
  sh tools/tinyctl/box/tiny-publish.sh "$f" tiny/README.md || { echo "STOPPED at $f"; break; }
  sleep 300      # a gap between uploads; drop it if sessions stop dying
done
```

**If the uploader says `no upload CSRF token` or a step 302s to /login**, the
session is gone: replace `~/.gh-upload/cookie` and re-run the loop — every clip
already published is skipped cheaply (the registration step notices the URL is
already in the body), and **nothing is ever re-uploaded** by re-running.

**If the gate never turns 200** the clip is uploaded and registered anyway; big
assets have taken up to 80 minutes to go public. Do not re-upload — re-run just
the gate:

```sh
env -i /usr/bin/curl -s -o /dev/null -w '%{http_code}\n' -L <the asset url>
```

## 6. When it is done

The page should show, for every map with a lap, a caption ending
`(build ship15, controls overlay)` and no `video pending` line under it. Two
things will still be pending, correctly: the eight laps certified after these
clips were cut (they need renders), and map 20, which has no lap at all.
