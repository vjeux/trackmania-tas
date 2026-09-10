# Video session 4 — RESUME NOTE (2026-09-10, 04:40Z–07:00Z, box devvm63095)

Successor of video 3 (`RESUME-video3-2026-09-09.md`). The page is finished for
every map that has a lap: 23 of 25 rows carry the newest certified ship15 lap
with the controls overlay; 20 has no lap; 09's ghost FILE is still the old lap.

## What was published today (cookie #8, one session, 15 uploads, no 302)

| map | lap | asset |
|---|---|---|
| 16 | 39.302 | bb3919e7 |
| 01 | 17.417 | 36aefd65 |
| 15 | 49.097 (new render) | a5db8c65 |
| 18 | 44.593 (new render) | 2843bda8 |
| 19 | 46.445 (new render) | 60c8d4e7 |
| 04 | 18.476 | 44b55e8a |
| 03 | 19.793 | 6d68e84c |
| 21 Argentina | 122.432 (new render, first video on 21) | 1c7205f2 |
| 06 | 22.653 | e4dee331 |
| 11 | 34.291 | f7b83d4e |
| 22 Saudi Arabia | 96.300 (new render, first video on 22) | adc9eaae |
| 12 | 20.514 | 8d1cf689 |
| 23 Norway | 102.162 (new render) | (see ships.tsv) |
| 10 | 24.552 | (see ships.tsv) |
| 17 | 38.426 | (see ships.tsv) |
| 24 Poland | 147.654 (new render) | b09f275d |
| 25 Japan | 119.353 (new render) | (see ships.tsv) |

Every sheet was eyeballed: chase cam on the car in every tile, no static or
black tile. 24's lap itself has a 68-s stall between cp3 (22.49) and cp4
(90.50) with one respawn — the clip shows ~35 s of the car against a wall; that
is the certified lap, not a render defect (told the coordinator).

Superseded before shipping (the README moved): 21 122.892, 22 99.912,
23 115.244, 24 149.593.

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

- **09**: the ghosts README says 28.292 but `09.Ghost.Gbx` (md5 7df51987) is
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
