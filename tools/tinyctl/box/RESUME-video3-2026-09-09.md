# Video session 3 — RESUME NOTE (2026-09-09, 23:20Z)

The controls overlay is the renderer's default now, and the page is half-way
through being re-published with it. This is what a successor needs.

## Where the work stands

- **Page (`tiny/README.md` on main):** 14 rows carry `(build ship15, controls
  overlay)` — 02 16.746 · 05 18.298 · 07 19.002 · 08 22.298 · 09 28.572 ·
  13 24.769 · 14 27.589 · 15 50.336 · 18 46.335 (first video on 18) ·
  19 48.377 · 25 Tiny Japan 2026 121.235 · and 01 19.381 + 12 20.752 on their
  older laps.
- **Staged and ready to publish, newest lap per map (13):** 01 17.417 ·
  03 19.793 · 04 18.476 · 06 22.653 · 10 24.552 · 11 34.291 · 12 20.514 ·
  16 39.302 · 17 38.426 · 21 Tiny Argentina 2026 122.892 · 22 Tiny Saudi Arabia
  2026 99.912 (first lap ever on 22) · 23 115.244 · 24 149.593. Each is cut,
  overlaid, timing-checked and stamped; they need UPLOADING, nothing else.
- **Newer laps that arrived after the box went away (need a render first):**
  23 = 103.207 and 24 = 147.656 (ghost folder refreshed ~15:59–16:07 PDT).
  25's row went back to the ship10 149.619, which `--build ship15` correctly
  skips, so the page keeps the ship15 121.235.
- **20** is the only map with no ship15 lap at all.

## Everything is banked

`~/persistent/private-30d/tm-player/tiny/videos-ship15/`: 44 mp4s, 44 webms,
sheets, `REPORT.md` (session-2 rows + the session-3 table, one row per lap with
its overlay column). Also on devvm65800 in `~/tinyvid3/out`.

**Mount the store on a FRESH OD**, not devvm42752: that devserver's user x509
expired at 20:32Z, its manifoldfs answers `Input/output error`, and
`persistent-storage remount` there fails with "No accepted numeric user identity
found among [MACHINE, MACHINE_TIER]". On a fresh OD:
`mkdir -p ~/persistent && persistent-storage mount private-30d`.

## Two things are down, for different reasons

1. **The WhiteStick bridge** — every `wsx`/`whitestick` call answers
   `Too many requests … more than 6000 (code: api-connex-oauth-authorize-client)`.
   vjeux, 23:18Z: *"the bridge is down and won't come back up. Please stop
   trying."* NOBODY calls it until he says otherwise. My share of the burn: the
   watcher used one `wsx cat` per pending clip per minute (~15 calls/min);
   fixed in a822b4d4 (one batched call, 120 s tick, ~2/min).
2. **The GitHub upload session.** A copied `Cookie:` header publishes a handful
   of clips and then every request 302s to `/login`. Measured today: 2–3 per
   session before the fixes, 5 after. The 302 carries no revocation — only a
   fresh anonymous `_gh_sess` — so the server simply stops recognising
   `user_session`. Yesterday ONE session published 20 clips untouched, with
   ~10 min between uploads; the current uploader paces at 5 min and makes an
   ordinary page view after each upload (b4810723), which is the untested
   hypothesis a successor should evaluate first.

## Resuming, once the bridge is back and a session is on the box

```
# one render/cut/ship loop, one watcher — both are manual, no cron
~/tinyvid3/loop-up.sh          # renders new ghosts, cuts WITH the overlay, stages, --ship
~/tinyvid3/shipwatch-up.sh     # collects URLs, swaps page rows, commits + pushes
~/tinyvid3/one-shipwatch.sh    # leaves exactly one watcher alive
```

The loop reads the store's ghost folder and the store's renders directly (local
paths on a box that has the store mounted), so a lap another thread rendered is
cut, not re-rendered. A single clip by hand:

```
tinyctl video --map NN --maps-dir ~/tinyvid3/maps --ghosts-dir <ghosts> \
  --out ~/tinyvid3/out --suffix ship15 [--from-webm <render.webm>] --store <dir> --ship
```

## Rules this session learned the hard way

- **`~/.gh-upload/cookie` is INPUT: read it, never write it.** A jar write-back
  replaced a live 1749-byte session with 33 bytes of `_octo` and cost a renewal.
- **One client on the GitHub session at a time** — the ship script takes the
  flock BEFORE the probe, so a released queue cannot fire a dozen parallel
  replays (that killed several sessions).
- **Don't push into the OneDrive tree**: a chunked push into `Maps\Tiny\videos`
  dies on a temp-chunk rename. Stage in `/mnt/c/Users/vjeux/tinyvid/mp4`.
- **`ships.tsv`** is appended by the loop and merged by the watcher; a
  read-modify-write from both sides dropped a map's newest lap.
- The page must converge on the newest lap per map: a superseded row is marked
  `superseded` and never shipped.
