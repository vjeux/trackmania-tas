# Record forensics — the wall-clock method and its tools

How the September 2026 fair-play investigations were done, with the tools in
this repo that do each step. Three write-ups came out of it; this folder is
the method behind them, so the next investigation starts from a working
pipeline instead of a memory of one.

| write-up | what it covers |
|---|---|
| [Wall-clock audit of unbeaten-AT records](https://gist.github.com/vjeux/591e8bceae5c87f4c5fe1de6ca60eb0b) | the instrument, its calibration against Nadeo's validator, base rates over 41,711 records, the deeperjungle / AshenBeast1985 bounty cases, and Part B: every world record of the seven unbeaten-map holders with a slow record |
| [Sokrat. — wall-clock rate vs run quality](https://gist.github.com/vjeux/cf8e9c7b0b296ae4bb539644ef18baca) | 674 world records + 296 non-WR finishes of one player; the slow rate rises with run quality, the ratios sit at exactly ×2.0 / ×2.5, two records ran faster than real time |
| [Fair-play audit of the global top 50](https://gist.github.com/vjeux/c75ef861f96dd15cc2793b8ed04edcd9) | 1,872 WR ghosts + 4,678 corpus records across 50 players; what a clean population looks like, and how isolated stalls differ from a tool |

Every number in those documents comes out of the commands below.

---

## 1. The instrument

Every TM2020 ghost (`.Ghost.Gbx`) carries a **validation chunk**
(`0x0309202D`) written by the client: the game build, an executable checksum,
an OS and CPU class, the Openplanet version string — and two unix timestamps,
the **real-time start and end of the attempt**. The game simulates in real
time, so for an honest run

```
walltime_end − walltime_start   ==   race time + countdown     (± 1 s)
```

Define, per file:

```
excess = wall span − (declared race time + countdown)      seconds
ratio  = wall span ÷ (declared race time + countdown)
```

- The stamps are whole seconds, so every excess carries ±1 s of rounding.
  99.2 % of 41,711 campaign/TOTD records sit within ±2 s.
- A game that ran **slower** than real time — a slowdown tool, a speedhack
  stepping the simulation, a freeze — leaves a *longer* span. **Nadeo's own
  dedicated server refuses such a file**: `IsValid false`, `Desc "unexcepted
  walltime (39s)"`. Its threshold is undocumented; measured against the corpus
  it sits between +10.8 s (accepted) and +11.6 s (refused) on 12–14 s runs,
  and around +14–15 s on 40–50 s runs.
- A game that ran **faster** than real time leaves a *shorter* span. The
  server does not check that direction; the stamps are the only instrument.
  No stall produces it. (Razz3r_25 ×0.25; B0NES-TM ×0.30 and ×0.02; Sokrat.
  ×0.062 and ×0.033 — a 7-hour run recorded in 14 minutes.)
- Identical start/end stamps (a 0-second span) are a third population, 0.2 %
  of records, cause undocumented; the server refuses those too. They carry no
  timing information and are reported as *stamps absent*, never as a ratio.

**The flag used throughout:** `excess ≥ 10 s`, or `ratio ≥ 1.15 with excess
≥ 5 s` (a 12-second run at half speed never reaches +10 s). Slightly more
inclusive than Nadeo's threshold, on purpose; every verdict quoted for a named
player in the write-ups is the validator's own, re-run on that file.

**What separates a tool from a stall** is never one file; it is the pattern
across a player's records:

| signature | tool | stall |
|---|---|---|
| ratio across many maps | snaps to a fixed fraction — ×2.00, ×2.50, ×4.00 — on maps from 3 s to 58 min | varies with the map and the moment |
| which runs are slow | the good ones (Sokrat.: 98 % of WRs, 39 % of runs 15 %+ off the WR) | whatever run was in progress; the slow record is no better than the player's median |
| same-day records | toggled: ×4.0 at 07:54, ×1.00 at 11:16 (Shade_.TM) | clean records minutes either side, slow one scattered |
| direction | can also run *faster* than real time | never faster |
| the field on that map | at ×1.00 | if the map is the cause, the field is slow too |

---

## 2. The tools

All Rust, in `tools/`. Build with `cd tools && cargo build --release -p ghost`
(the dedicated server is a separate download, §2.4).

### 2.1 `ghost wallclock files` — read the clock out of a pile of ghosts

```
ghost wallclock files DIR|FILE... [--tsv OUT]
```

One row per file, straight from the bytes: declared time, countdown, wall
span, excess, ratio, the two stamps, client build, Openplanet, and the
machine fingerprint (OS class, CPU class, exe checksum, settings flags).
Sorted by ratio, so the suspicious files are at the top. Unreadable files are
listed on stderr — never silently dropped.

```
$ ghost wallclock files ghosts/ --tsv wall.tsv
wrote wall.tsv (1872 rows, 0 unreadable)
$ head -3 wall.tsv | cut -f1-6
file                                   declared_s  countdown_s  wall_s  excess_s  ratio
Sokrat__SCWPCB__46808.Ghost.Gbx        46.808      1.50         329     +280.65   6.805
AshenBeast1985_208024_9075.Ghost.Gbx   9.075       1.55         39      +28.38    3.671
```

About the fingerprint columns: `exe_checksum` is the **client build**, not
the machine — every player on one build shares it — so it cannot separate two
PCs behind one account. `cpu_kind` takes two values in practice and 19 of the
top 50 show both over the years. Reported as context, not as evidence.

### 2.2 `ghost wallclock census` — the report

```
ghost wallclock census WALL.tsv [--records REC.tsv] [--flag-s 10] [--top N]
```

Reads a `files` table and prints: the excess distribution and base rate; the
ratio histogram of the flagged records (peaks at ×2.0 / ×2.5 / ×4.0 are a
setting, scatter is stalls); flag rate by client build and by Openplanet
presence; the flagged list.

With `--records REC.tsv` — a TSV with columns `file`, `player`, `rank`,
`set_utc`, `map`, joined on the file name — it adds: flag rate **by rank** (a
tool pays at the top; stalls are rank-independent), **by year**, the
**per-player table** split into *chronic* (≥3 flagged and ≥50 % of their
records; the ratios then decide between a setting and a slow machine) and
*sporadic* (the flagged records' ranks against the player's own median), the
**overlap test** (two runs of one account whose real-time windows intersect
were not driven by one person), and **selectivity** (flag rate per rank
bucket per player — the Sokrat. gradient).

### 2.3 `ghost verify` gate V13 — the server's own verdict

`ghost verify FILE [--map MAP] [--server DIR]` runs the file through the
dedicated server. V7 compares the re-simulated time with the declared one;
**V13 reports `IsValid` and `Desc`**. The two differ exactly on the files
this method is about: a slow-clock ghost re-simulates to the declared time
(V7 passes) and is still refused (`V13 FAIL: unexcepted walltime (60s)`).
Before V13 existed, V7 called every one of those files good.

Reproduced 2026-10-27 on the live record (the 9.075 on *Miru's Hell 2* is
still the world record), with the server downloaded fresh from Nadeo:

```
$ ghost verify r01_9075_AshenBeast1985.Ghost.Gbx --map 6BbaQo4A6VbrpPiTNVbbP0uE5Vd.Map.Gbx --server ./server
PASS V6   tape/record agreement: kappa 1.000 (100.0% of 182 samples exact, best lag 0 ms)
PASS V7   oracle re-simulated the written file: 9.075 == the declared time
FAIL V13  the server re-simulated the time but says IsValid FALSE: unexcepted walltime (39s)
```

### 2.4 The dedicated server — Nadeo's validator, run in bulk

`TrackmaniaServer_Latest.zip` from
`https://nadeo-download.cdn.ubi.com/trackmania/TrackmaniaServer_Latest.zip`
(Nadeo's free dedicated server). The build used throughout — and still served
on 2026-10-27 — is 2026-05-15, Linux binary md5
`0f0f4b25f31f80c60c81404366c95e68`. Unzipped it is a
`TrackmaniaServer` binary plus `Packs/`. The validator mode re-simulates every
ghost in `UserData/Replays` on the map in `UserData/Maps` and prints one JSON
block per file: `Time`, `IsValid`, `Desc`, checkpoints, the account id, the
inputs.

```
./TrackmaniaServer /nodaemon /validatepath=.
```

`ghost::oracle::validate_many` is the library wrapper (one launch for a whole
batch — the launch is what costs, not the files); `ghost verify` and
`ghost tape shifts` use it; set `TM_SERVER` or pass `--server DIR`.

Things the server does that matter here: it refuses a map it cannot load
(`Can't load map`), fails a file whose physics no longer re-simulate (`wrong
simu`, common on maps whose items changed), and **hangs forever on some maps
that use custom old-car items** — always run it under `timeout`. A batch of
60 files on 60 maps takes a few minutes.

### 2.5 `ghost tape shifts` — how fragile is a run?

```
ghost tape shifts --template RUN.Ghost.Gbx --out DIR --map MAP [--server DIR]
                  [--max-shift 1] [--until-ms MS] [--no-validate]
                  [--stations NAME=MAP,...]
```

Every input event of the template (each tick where steer/accel/brake changes)
moved by −k…+k ticks, one file each, plus `IDENT` (the template re-encoded,
the control), validated in one launch. Prints per-event survival and, with a
**station ladder** (copies of the map with the finish relocated to landmarks,
in track order), *where* each neighbour died.

What it is for, and what it is not: on the bounty run the record kept 0 of 26
±10 ms neighbours while the field's ghosts kept 3–52 %. That is **not**
evidence of a tool — a landing 3 m from the pad edge has zero tolerance by
construction, and the slower humans landed deep in the pad. The shift family
measures slack; slack is set by the line, not by who drove it.

```
$ ghost tape shifts --template r01_9075_AshenBeast1985.Ghost.Gbx --out shifts --map 6Bba….Map.Gbx --server ./server
template …: 1063 ticks, start offset -1550 ms, declared 9.075; 6 events; shifts +/-1 tick(s)
IDENT: FINISH 9.075  == declared
  e01  race    3.610 s  accel 1->0, brake 0->1       0/2
  e02  race    3.630 s  steer 0->-127                0/2
  e05  race    5.020 s  steer 127->-127              2/2
SURVIVAL: 4 of 12 neighbours finish (33%)
```

### 2.6 Reading one file by hand

```
ghost inspect FILE            # declared time, tape, telemetry span, identity
ghost manifest FILE | jq .validation
                              # walltime_start / walltime_end / exe_version / race_settings ...
ghost verify FILE --map MAP   # V1 codec identity .. V7 re-simulated time .. V13 IsValid/Desc
tmtraj FILE                   # trajectory landmarks (ramp exit speed, landing point, trigger entry)
```

`ghost verify` V6 (tape vs telemetry agreement, κ) and `tmtraj` were what
cleared the bounty run itself: κ = 1.000 over 309 samples, a keyboard tape
shaped like every other finisher's, a landing 3 m short of the pad edge rolled
in at 64 km/h — a near-miss that barely counted, not a tool's line.

### 2.7 `ghost tape cursor` — the OS cursor inside the input tape

A second instrument, for runs the clock does not catch. Every input packet
carries one bit "cursor unchanged"; when the OS cursor moved since the
previous tick the packet carries its new **absolute position**, two 16-bit
axes in Windows' normalised virtual-desktop space (0..65535 across the primary
monitor; a monitor to the left shows up as a wrap through 65535). The game
writes it for every player, mouse-steerer or not: most runs have one sample at
tick 0 — where the cursor sat when the countdown ended — and nothing after.

```
ghost tape cursor FILE|DIR... [--events] [--tsv OUT]
```

One row a file: `tick0_cursor`, `midrun_samples`, `streams` (runs of
consecutive-tick samples), `singletons` (no sample within 50 ms on either
side), `max_isolated_jump` (the largest move that started from rest, measured
from the previous known position), `edge` (x = 0 or y = 0), `corner` (0,0).
`--events` lists every mid-run sample with its gaps and jump. The footer is
the pile's base rates.

What a hand does and what a program does are different shapes:

- **A hand** moves in streams — one sample per tick while the mouse moves, a
  few hundred units per tick at the fastest — and at rest produces nudges of a
  few units (a pad hit shaking the desk). A big move always has a run-up and a
  run-down because a physical mouse accelerates and stops over several ticks.
- **A program** (`SetCursorPos` and everything wrapping it: AutoHotkey
  `MouseMove`, pywinauto, a tool clicking the game window to focus it, a tool
  parking the cursor out of the way) moves the cursor in **one tick, from
  rest, by any distance, to a round place** — the corner (0,0) is the classic.

Read the result against the base rates in §4. A keyboard or pad player with
no cursor sample at all is the normal case; a mouse-steerer has hundreds in
streams; one or two singletons that land on a screen edge after a large jump
from rest are the shape no hand produced in the measured population.

### 2.8 `ghost record samples` — what the car was doing at that tick

```
ghost record samples FILE [--from MS] [--to MS]
```

The driver's telemetry as a table (race time, position, speed, velocity,
attitude, ground/top contact, gear, the sampled steer/gas/brake). The question
it answers: an input anomaly sits at tick N — what was the car doing at that
moment, and in the 100 ms around it? A cursor warp that lands within a few
ticks of the run's decisive physics event (a launch, a collision, a landing)
is a different fact from one in the middle of a straight.

---

## 3. The data: where records and ghosts come from

Public endpoints, no login. Pace every request (≥ 1 s; the investigations
ran at 1.2 s and were asked once not to hammer trackmania.io). One command per
step; the orchestration is a loop over a TSV, not a program.

| need | source |
|---|---|
| a player's current world records | `https://tmworldrecords.net/api/user?userId=<displayName>` → `nrOfWr`; `…/api/user/maps?userId=<name>&offset=N&limit=100` → `trackID` (TMX id), `onlineRecordTime`, `trackName` |
| TMX id → map uid, author time | `https://trackmania.exchange/api/maps?id=<id,id,…>&fields=MapId%2CMapUid%2CMedals.Author%2CName%2COnlineRecordCount&count=100` (50 ids a call) |
| a board, with ghost download urls | `https://trackmania.io/api/leaderboard/map/<uid>?offset=0&length=100` → `tops[].position/time/timestamp/url/player.id` — top 100 only, no per-player endpoint |
| the ghost itself | `https://trackmania.io<url>` (`/api/download/ghost/<id>`) |
| the map file | `https://trackmania.io/api/map/<uid>` → `fileUrl` |
| the global top 50 | `https://trackmania.io/api/top/trophies/0` |
| a player's profile | `https://trackmania.io/api/player/<accountId>` (creation date, trophies, matchmaking) |
| a player's PB on any map, including non-top-100 | Nadeo core `GET prod.trackmania.core.nadeo.online/v2/mapRecords/?accountIdList=<id>&mapId=<nadeoMapId>` — needs a game-session token (`NadeoServices` audience); the record's `url` serves the ghost from `core.trackmania.nadeo.live/mapRecords/<id>/replay` without auth |

The 41,711-record **corpus** behind the base rates is the tm-player crawl of
every seasonal campaign, Track of the Day and Weekly Shorts board (ranks
1…10,000, 2,305 maps). Any pile of ghosts plus a records table reproduces its
numbers through `census`.

### Controls that go with every flag

1. **The field on the same map** — the other top-10 finishers' ghosts through
   `files`. If they are slow too, it is the map (heavy geometry) or the run
   length (long trial maps accumulate stalls: 35 % of >15-minute WRs are
   flagged), not the player.
2. **The same player's other records**, especially the same day — a machine
   that has become slow is slow on all of them.
3. **A deep sample** before any judgement: 20 WRs is a screen; a verdict needs
   the player's full list (the Part B deep dives read 1,033 world records of
   seven players, every one).
4. **The validator's own verdict** on every flagged file.

---

## 4. Base rates and thresholds measured

| population | slow (≥ 10 s) | notes |
|---|---|---|
| 41,711 campaign/TOTD records, ranks 1…10,000 | 0.41 % | flat across rank (0.40 % at rank 1, 0.37 % at 1001+); rising by year 0.09 % (2022) → 0.76 % (2026) |
| stamps absent / 0-span | 0.21 % | refused by the server |
| 1,872 WR ghosts of the global top 50 | 0.53 % | a clean elite population |
| unbeaten.at rank-1 ghosts, runs < 3 min | 1.4 % | 3.5× the ordinary rank-1 rate |
| unbeaten.at rank-1 ghosts, runs > 15 min | 35 % | long runs accumulate stalls; the threshold is absolute |
| Openplanet | no year-controlled correlation | 86 % of flagged vs 74 % of clean carry it, but per-year rates are equal; DEVMODE no effect |
| flagged WRs' lead over #2 | median 0.08 % vs 0.10 % for clean WRs | a slow record is not an outsized record |

Nadeo's refusal threshold, from accepted/refused pairs in the corpus: +10.8 s
accepted / +11.6 s refused at 12–14 s runs; +15.02 s accepted / +14.22 s
refused at 40–50 s runs (ratio ≈ 1.33–1.34). Not a single number; the 1-s
stamp resolution blurs it. The tolerance grows with the run: an 83.5-s run
with +13.0 s (×1.153) is accepted.

**Cursor base rates** (`ghost tape cursor`, the top 300 of *Summer 2021 - 17*,
2026-10-27 — 2021 keyboard/pad players through 2026 builds):

| measure | value |
|---|---|
| files with any mid-run cursor sample | 6 of 300 (2 mouse-active players with 697 and 226 samples in streams; 4 with 1–23 nudges) |
| mid-run cursor samples | 966 |
| moves that started from rest (> 50 ms since the previous sample) | 140; the largest **76 units**; 112 of them ≤ 10 |
| mid-run samples on a screen edge (x = 0 or y = 0) | **0**; at the corner (0,0): **0** |
| tick-0 cursors on an edge | 4 of 207 (a cursor parked in a corner before the run is real, (5,0) twice) |

So a mid-run singleton that moved ≥ 300 units from rest, or that landed on an
edge, is outside everything 300 honest runs on the same map produced. One such
sample is a lead; several, each within a few ticks of the run's decisive
physics events, is the shape of a tool's hand on the window — still to be read
together with the splits (where the time was gained) and the player's other
records (does the cursor ever move in them?).

---

## 5. Limits — what the files cannot show

- **An input macro replaying a perfect line at normal speed.** Deterministic
  physics makes it validate perfectly; the stamps read ×1.00. This is the most
  plausible remaining cheat and nothing in the clock sees it. The cursor
  instrument (§2.7) sees the tool's window interactions when there are any; a
  tool that never touches the cursor leaves nothing but the run itself. Only a
  POV recording does.
- **A save-state tool that restores the seed.** The tape re-simulates, the
  clock reads real time, V13 says valid: the file is indistinguishable from a
  live run except by what the tool did around the game — the cursor, the
  Openplanet signature, the pattern over records.
- **Account sharing without overlap.** The overlap test catches two people
  driving *at the same time*; alternating sharers leave nothing.
- **Which tool, or whether it was a tool, from one file.** A single refused
  record is a lead, never a verdict (p ≈ 4 % of one slow record among nine by
  chance alone). The verdict comes from the pattern in §1.
- **Nadeo's threshold is one-sided and lenient**: +10 s on a short run passes
  it; fast-forwarded runs pass it. The `files` table is the stricter
  instrument.

---

## 6. A complete walkthrough — one player, start to finish

```sh
# 1. their world records
curl -s 'https://tmworldrecords.net/api/user/maps?userId=NAME&offset=0&limit=100&query=&sort=trackName-asc' \
  | jq -r '.[] | [.trackID, .onlineRecordTime, .trackName] | @tsv' > wrs.tsv       # repeat with offset=100,200,… up to nrOfWr

# 2. TMX ids → map uids (50 a call)
curl -s "https://trackmania.exchange/api/maps?id=$(cut -f1 wrs.tsv | head -50 | paste -sd,)&fields=MapId%2CMapUid%2CMedals.Author&count=100" \
  | jq -r '.Results[] | [.MapId, .MapUid, .Medals.Author] | @tsv' > id2uid.tsv

# 3. per map: the board (their rank, timestamp, ghost url), then the ghost — 1.2 s between calls
curl -s "https://trackmania.io/api/leaderboard/map/$UID?offset=0&length=10" > lb/$UID.json
curl -s -o "ghosts/${NAME}__${TMX}__${RANK}__${MS}.Ghost.Gbx" "https://trackmania.io$(jq -r --arg p "$ACCOUNT" '.tops[] | select(.player.id==$p) | .url' lb/$UID.json)"

# 4. the clock, every file
ghost wallclock files ghosts/ --tsv wall.tsv

# 5. the records table for the census (file, player, rank, set_utc, map) — from the boards you already have
#    then the report
ghost wallclock census wall.tsv --records records.tsv --top 30

# 6. every flagged file through Nadeo's validator, with its map
ghost verify ghosts/FLAGGED.Ghost.Gbx --map maps/$UID.Map.Gbx --server /path/to/TrackmaniaServer
#    → V7 (time) and V13 (IsValid / Desc)

# 7. the field control: the other top-10 ghosts on each flagged map, through step 4 again
```

Then write down, per flagged record: the ratio, the validator's `Desc`, how
many other finishers on that map were slow, and the player's same-day
records. That table — not the flag — is the finding.

---

## 7. Evidence discipline

- The per-record tables (every ghost's stamps, ratio and validator verdict) are
  the annexes of the three gists above. Ghost ids and map uids are in them, so
  any file can be re-fetched from Nadeo and re-read.
- Raw ghost banks kept on a 30-day store **expired**; the gists are the record
  of the investigation. Bank anything meant to outlive a month in a place
  without a TTL.
- Never name a player on one file. The write-ups rank players by *pattern*,
  state a confidence, and say what would confirm or clear each case.
