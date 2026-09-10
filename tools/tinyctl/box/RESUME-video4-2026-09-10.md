# Video session 4 — RESUME NOTE (2026-09-10, 04:40Z–08:25Z+, box devvm63095)

Successor of video 3 (`RESUME-video3-2026-09-09.md`). **The page is finished:
at 08:20Z all 24 maps that have a lap show their newest certified ship15 lap
with the controls overlay (24 rows with a video, 0 'video pending' lines); 20
has no lap.** The loops keep it that way: every lap the player project
improves overnight is rendered, eyeballed by whoever is on watch, and shipped
(~5–15 min render + a 7-min upload slot each). At 08:11Z three more marginal
laps arrived (15 48.821, 21 122.318, 23 102.148) and were in the pipeline.

## What was published today (cookie #8, ONE session, 23 uploads, no 302)

16 39.302 bb3919e7 · 01 17.417 36aefd65 · 15 49.097 a5db8c65 · 18 44.593 2843bda8 ·
19 46.445 60c8d4e7 · 04 18.476 44b55e8a · 03 19.793 6d68e84c · 21 Argentina 122.432
1c7205f2 (first video on 21) · 06 22.653 e4dee331 · 11 34.291 f7b83d4e · 22 Saudi
Arabia 96.300 adc9eaae (first video on 22) · 12 20.514 8d1cf689 · 23 Norway 102.162 ·
10 24.552 · 17 38.426 · 24 Poland 147.654 b09f275d · then the overnight pass:
09 28.292 022627c4 · 15 48.834 8ad5e39f · 18 43.413 cb510efb · 19 46.362 852cae15 ·
21 122.376 7186c2a4 · 22 96.298 2bb4c108 · 25 Japan 119.115 89547580.
(ships.tsv has every URL.) The pacing that held: 5-min COOLDOWN + a page view
after each upload, one client under the flock — where back-to-back died at 5.

Every sheet was eyeballed: chase cam on the car in every tile, no static or
black tile (a sheet of a <32-s lap has black tail tiles: 4×4 at 0.5 fps). 24's
LAP has a 68-s stall between cp3 (22.49) and cp4 (90.50) with one respawn — the
clip shows ~35 s at a wall; certified lap, not a render defect.

Superseded before shipping (the README moved): 21 122.892, 22 99.912,
23 115.244, 24 149.593, 25 119.353 (uploaded as 62979c25, never on the page).
## What changed in the tools

- **Bridge**: navi is dead; `~/bin/whitestick` is the new client
  (`cargo build --release -p whitestick`), config `~/.whitestick/config.toml`
  from `devvm42752.vll0.facebook.com` (relay 195.154.114.196:8443, token, pin).
  `wsx` moves a file in ONE streamed call (142e9dd1); `tinyctl` polls a detached
  box job once per 30 s with the log line folded in. ≤ 2 bridge calls/min per
  loop is hygiene, not a quota, on our relay.
- **shipwatch** (79d74f94): relaunches a pending clip with no verdict file (only
  when nothing runs on the box); the ghosts README outranks ships.tsv (a staged
  clip whose lap the README replaced on the same build → `superseded`); rsync
  23/24 during a ghost sync is a warning.
- **page** (218fd858): a row swap works on the row's whole block — one URL, the
  old video gone, the pending line dropped; `page-status` finds/repairs pending
  and asset lines anywhere in the block. (Before: 7 rows showed two videos and a
  stale pending line for half an hour.)
- **ghsession** (8f29d58c, c138b52; `tools/ghsession`, built on the box): the
  uploader's OWN GitHub session — see `UPLOADER-OWN-SESSION.md`. Not seeded yet;
  the one-time step is vjeux's (private-window login → Cookie header →
  `ghsession seed`). `tinyship.sh` switches to it automatically when
  `~/.gh-upload/session.json` exists on the box.

## The loops (devvm63095; all local paths, the store mounted there)

```
~/tinyvid4/loop-up.sh          # tinyctl video --all --watch 120 --ghosts-sync <store ghosts> --build ship15 --ship
~/tinyvid4/shipwatch-up.sh     # tinyctl shipwatch --commit (page clone ~/tt-page)
~/tinyvid4/pagestatus-loop.sh  # tinyctl page-status --write --commit every 10 min
~/tinyvid4/bank.sh             # state files → store videos-ship15/*-video4.* every 10 min
```

State: `~/tinyvid4/out/{videos.tsv,ships.tsv,REPORT.md}` (also on the store as
`videos-video4.tsv`, `ships-video4.tsv`, `REPORT-video4.md`). Maps:
`~/tinyvid4/maps` = the store's `incoming/ship15-53383427/*.Map.Gbx`.

## Open

- **09**: (resolved 23:56 PDT — the file caught up and 28.292 is published.) Earlier: the ghosts README said 28.292 but `09.Ghost.Gbx` (md5 7df51987) was
  the old 28.572 lap → the page keeps the 28.572 video with a *latest lap 28.292
  — video pending* line until the input arm replaces the file. The loop renders
  it the moment the file changes.
- **ghsession seed** — the one-time step, once vjeux does the private-window
  login (do it only when no cookie drain is running).
- The store's canonical `REPORT.md` is `REPORT-video4.md` (a superset of
  video 3's); copy it over when this session ends.

## Fresh box checklist (5 min)

`~/.whitestick/config.toml` (from devvm42752.vll0), the GitHub deploy key
`~/.ssh/id_ed25519_tmtas` + `~/.ssh/config.tmtas` + an `Include ~/.ssh/config.tmtas`
line, `git clone github-tmtas:vjeux/trackmania-tas.git`, rustup with
`~/.cargo/config.toml` (`[http] proxy = "fwdproxy:8080"`, `[net]
git-fetch-with-cli = true`), `cargo build --release -p tinyctl -p shootctl
-p tmmaps -p clip -p ghost -p wsx -p whitestick -p ghsession`, copy
`target/release/{wsx,whitestick}` to `~/bin`, `mkdir -p ~/persistent &&
persistent-storage mount private-30d`. `~/.navi/credentials.json` is no longer
needed by anything.

## Addendum, 17:55Z — the afternoon's defaults (all on main, loops on devvm68451 since 14:12Z)

- **Re-render gate** `--min-gain-s 0.1` (d31034ce): a map is rendered again only when
  the certified lap beats the PUBLISHED clip by 0.1 s (or first lap / build change);
  skips are one REPORT row + `skips.tsv`; page-status writes "within 0.1 s of the
  published clip" for a sliver, "video pending" for a lap that will render (cac9fc36).
- **Ghost archive** (8bbbe084): before any render the input ghost goes to the store's
  `tm-player/tiny/ghost-archive/<md5>.Ghost.Gbx` + `<md5>.json`; REPORT row and
  `<clip>.mp4.json` name the archive file. The alias `ghosts-for-video/NN.Ghost.Gbx`
  is mutable — cite the archive md5, never the alias.
- **Stamp** carries `ghost_md5=` beside the FNV (004e4a7b).
- **Transition guard** (2284cc81): a ghost file renders only when the README names its
  lap (the input arm rewrites README first, alias second).
- **Publish hold** (f41cf0ee): `~/tinyvid4/out/holds.tsv` (`nn<TAB>reason`); a held map
  is archived-not-rendered, never launched/swapped, page note "held (reason)".
  **21 Argentina is HELD** since 17:55Z (vjeux: the opening is bad) — lift by deleting
  the line.
- **Box**: `Maps\_shoot` is a junction → `C:\tm\_shoot` (load 835 s → 6.6 s); the
  idle-quit closes the game after 30 idle minutes, under the render lock only.
- `review-private/` on the store: 24 100.116, 22 96.297, 15 48.738 for the parent
  project's private review (never published).
