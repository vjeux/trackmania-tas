# tools/clip

The video publishing tools, in Rust, std only — no dependencies, builds offline
on the render box's WSL side like everything else in this workspace.

```
cd tools && cargo build --release      # -> target/release/{clip,playtest}
```

| | replaces | catches |
|---|---|---|
| `clip ship <file.mp4> <map-dir> [asset-name]` | `tools/ship-clip.sh` | *a clip that plays for you and 404s for everyone else* |
| `clip frames <in.mp4> <outdir> --at T,T,... \| -n N` | a hand-written ffmpeg line | *a re-shoot nobody looked at* |
| `clip split <l.mp4> <r.mp4> <l-label> <r-label> <out.mp4>` | `tools/splitscreen.sh` | *a "comparison" that is one car and a caption that lies* |
| `playtest [--trainer <dir>] [--chrome <path>]` | `trainer/playtest.sh` | *a trainer page that a stub DOM says is fine* |

## frames

FILMING.md §6 is "watch what you made", and it had no tool: every look was a
hand-written ffmpeg line, which on the render box means remembering that the
Windows binary cannot open a WSL path. So it got skipped. `--at` names the
instants the telemetry says something should be happening; `-n N` spreads N
stills over the clip. `-ss` goes before `-i` (a seek, not a decode from zero)
and the frame's real timestamp is read back afterwards, because a seek is not a
promise; a time past the end makes ffmpeg write nothing and exit 0, so every
still is confirmed to exist and to be non-empty.

## inventory

`--probe` measures the pages that do not say what they filmed. `--probe-all`
measures the ones that do say, as well, and prints `DISAGREES` where the clip
contradicts the prose — which is what a page describing a WITHDRAWN clip looks
like: 276877's note about a side-by-side is about a video that came down, and
the phrase match read its surviving 16:9 single-car clip as a split.

## ship

Five steps, in order, each one refusing rather than warning: settle and probe
the local file · upload the original to the `videos-v1` release · upload to
GitHub's user-attachments store · **register the URL in the release body** ·
fetch it back with no credential at all and require 200 and playable bytes.

Registration is the step that makes an asset public — a pushed commit does not.
19 clips were shipped before that was learned and 18 were 404 to everybody but
their author. The last step runs the fetch with a cleared environment (no
cookie jar, no `GH_TOKEN`, no netrc, no proxy) because a gate that runs with
credentials is not a gate, and it retries: registration takes up to ~45 s to
propagate, and one reading is not a verdict.

The user-attachments upload itself is still `ghvid.sh` (`$GHVID`, default
`~/bin/ghvid.sh`): it posts to a private endpoint with a live browser session
cookie and a CSRF token scraped from a rendered page. Its exit 3 means that
cookie has expired.

## split

Only for maps where a chase camera provably cannot hold both cars — 276877 (the
human 61.5 m away), 228607 (356.68 m). The shorter run is held on its final
frame so the gap reads as TIME rather than as a broken video, and both halves
are labelled, because nothing else in the picture says which driver is which.
An ffmpeg without libfreetype (the Mac's, and most `-static` Linux builds) has
no `drawtext` filter and is refused up front.

## platform

Two configurations, `CLIP_PLATFORM=native|wsl`, auto-detected but never mixed:
a native ffmpeg/ffprobe on PATH, or the render box's Windows build under
`/mnt/c`. **A Windows ffprobe.exe cannot read a WSL path**, so on `wsl` every
path is translated to its `C:/...` spelling and anything outside `/mnt/<drive>`
is staged onto the Windows drive first (`CLIP_STAGE_DIR`).

Other knobs: `CLIP_FFMPEG` `CLIP_FFPROBE` `CLIP_WINFF_BIN` `CLIP_FONT` `REPO`
`RELEASE` `GHVID` `CLIP_GH` `CLIP_CURL`.

## tests

`cargo test --release` from `tools/`. Everything pure is unit-tested beside the
code; the tests that drive `gh`, `ffmpeg` or Chrome live in
`tests/external.rs` and **skip out loud, naming what was missing**, rather than
passing quietly. The `gh` chain is never run end to end anywhere: the only
thing it could be run against is the live release.
