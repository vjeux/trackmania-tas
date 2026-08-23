# Running an old build's physics

**Nadeo publishes dated dedicated-server archives, and the ones from 2022 still
run.** The 2022-06-21 build re-simulates a 2022 world record to the millisecond
— and to all five of its checkpoint splits — where the current build cannot
finish the same tape at all. Old-build validation is not something to build; it
is a directory and a `--server` flag.

Everything below is on **KEKL- SAUSAGE ICE**, the map that raised the question:
author time **58.687**, world record **63.546** by Roevhaal on a 2022 build,
best current-build human 68.442, our TAS 67.200. Times are seconds.

---

## 1. Where old builds come from

The setup script fetches
`http://files.v04.maniaplanet.com/server/TrackmaniaServer_Latest.zip`. The same
directory serves **dated** archives under
`TrackmaniaServer_YYYY-MM-DD.zip`.

**MEASURED** — a HEAD sweep of every date name from 2018-01-01 to 2026-12-31
(3 287 names, `tools/dsprobe`) returns **41 builds, from 2020-07-01 to
2022-06-21**, plus `_Latest.zip` (2026-05-15). *Control: the sweep is the same
code path that finds `_Latest.zip`, which is the build this project has been
using all along; and 2 018-2019 returns 0 of 730, which is the era before the
game existed.* Bucket listing is disabled, so a name that is not probed is not
seen — but the naming is dense enough to be convincing: 15 builds in 2020, 15 in
2021, 11 in the first half of 2022, then nothing.

**INFERRED** — Nadeo stopped publishing dated archives after 2022-06-21 and
publishes only `_Latest.zip` now. That is the one real limit of this route:
**there is no server binary available between 2022-06-21 and 2026-05-15**, so
the build that changed the physics cannot be bisected from this archive.

Full listing with sizes and dates: `tm-oldbuild/raw/availability.txt`.

## 2. The 2022 build reproduces the 2022 field exactly

Every ghost of the map's top 15, through both servers, unmodified
(`raw/field_cur.txt`, `raw/field_old.txt`):

| recording build | ghosts | current server 2026-05-15 / 128182 | 2022-06-21 server / 113135 |
|---|---|---|---|
| 2022-07-06 / **113150** | 10 | **0 of 10** — `wrong simu`, no time | **10 of 10 exact** |
| 2025-07-04 … 2026-02-02 | 5 | **5 of 5 exact** | not loadable — the file is never enumerated |

**MEASURED**, a clean 15-of-15 partition by recording build. Roevhaal's
**63.546 → 63.546**, `IsValid: true`. *Control: the five modern ghosts validate
exactly on the current server in the same batch, so the instrument is not
"everything passes"; and the ten 2022 ghosts fail on the current server in the
same batch, so it is not "everything passes on any server".*

### Five splits, not one number

The single finish time could be a coincidence of the map. It is not
(`raw/seg{1..5}_old.txt`): Roevhaal's tape through the five segment maps on the
2022 server crosses **13.492 / 31.143 / 42.452 / 59.582 / 63.546** — his file's
own declared splits, to the millisecond, five for five. *Control: those segment
maps were built by the `ksi2` arm and verified against two other runs' declared
splits before this arm existed.*

### It replicates on a second binary, and the negative fires

Six older builds, same two ghosts (`raw/era_*.txt`):

| build | git | loads the map | Roevhaal's 63.546 |
|---|---|---|---|
| 2020-07-01 | 105768 | — parses 0 ghosts | — |
| 2021-01-18 | | `Can't load map` | — |
| 2021-07-01 | | `Can't load map` | — |
| 2022-01-01 | | yes | **`wrong simu`** |
| 2022-03-19 | | yes | **`wrong simu`** |
| **2022-05-04** | 112906 | yes | **63.546 exact — and 10 of 10 on the whole 2022 field** |
| **2022-06-21** | 113135 | yes | **63.546 exact** |
| 2026-05-15 | 128182 | yes | `wrong simu` |

**MEASURED.** Two independently downloaded binaries, six weeks apart, reproduce
the record; two builds from earlier in 2022 load the same map and cannot.
*That negative is the control this needs: the instrument can tell builds apart,
so "the 2022 build reproduces it" is not a pass that any binary would give.*

**INFERRED** — the physics that recorded the 2022 field arrived between
**2022-03-19 and 2022-05-04** and left between **2022-06-21 and 2026-05-15**.

### What this settles about the map

**MEASURED** — the world record on KEKL- SAUSAGE ICE is a real, reproducible
run under the build it was set on. The `ksi2` reading that "today's car rotates
less than the 2022 car did" is confirmed from the other side: it is not a
recording quantum, not a rounding seed and not our tape handling. **The author
time, 58.687, was stamped into the map file on 2022-07-31 — the same physics
era.**

## 3. It is a full oracle, not just a validator

The dedicated server's `/validatepath` re-simulates whatever tape it is given,
so an old server is a complete physics oracle for its own era — and **the
current toolchain drives it with `TM_SERVER` and nothing else**.

**MEASURED** — `TM_SERVER=/tmp/oldbuild/srv2022 tmsearch search --template
<Roevhaal's ghost> --map <the map> --seg 1..4`, the repo's own searcher,
unmodified:

* its **decoy test** passes on the old build: *"the do-nothing tape (6355
  editable ticks blanked) scores DNF cp0; the incumbent scores 63.546"* — a
  negative and a positive control in one line, run by the tool before it starts;
* its **segment ladder verifies itself** against the template: *"--seg 4:
  59.582 — and the template's own recorded split agrees exactly (59.582)"*;
* it improves the world record's own tape — **63.546 → 63.518 in 2m37s** on 16
  workers, *confirmed by the plain oracle, 0 phantoms*, and to **63.074** on a
  200-minute run with 64 workers (§8).

`ghost verify FILE --map M --server srv2022` also passes its oracle check
(`V7 oracle re-simulated the written file: 63.546 == the declared time`), so the
repo's acceptance gate works against a 2022 build too.

**INFERRED** — the search is only as good as its failure signal, and the old
server gives none (§4.1): every DNF looks the same depth. The segment ladder
restores it, because a segment map turns "failed" into "finished at CP k", which
the old server reports normally. Use `--seg` on an old build, always.


## 4. How to use it

```bash
export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080
mkdir -p /tmp/oldbuild/srv2022 && cd /tmp/oldbuild/srv2022
curl -sSL -o ts.zip http://files.v04.maniaplanet.com/server/TrackmaniaServer_2022-06-21.zip
unzip -o -q ts.zip && rm ts.zip     # 540 MB; TrackmaniaServer + Packs/

export TM_SERVER=/tmp/oldbuild/srv2022     # the whole toolchain reads this
tmsearch validate --map M.Map.Gbx G.Ghost.Gbx
tmsearch search --template G.Ghost.Gbx --map M.Map.Gbx --seg 1:seg1.Map.Gbx ...
ghost verify G.Ghost.Gbx --map M.Map.Gbx --server /tmp/oldbuild/srv2022
```

Three things bite, all measured:

1. **A DNF is encoded differently.** The current server writes
   `"ValidatedResult": null`; the 2022 server writes a *present*
   `ValidatedResult` holding `"NbCheckpoints": 0, "Time": -1`, and never prints
   the `reached some checkpoints (N out of M)` clause. A parser that takes the
   number at face value reads a DNF as **a finish at −1 ms — the best score
   there is** — and the search collapses onto DNFs in one generation (seen:
   `*** -1 ms (was 63546 ms)` on the second evaluation of the 2026-08-18
   `tmtas-rs` bundle, which needed a guard added). `tools/ghost`'s `sane_time()`
   drops negatives, so **the repo's own toolchain is already correct** — that is
   read from the code and confirmed by running it.
   **Consequence either way: an old server gives no DNF depth**, so run with
   `--seg` (§3).
2. **`--map` must be absolute** for the older bundle's searcher: the worker
   symlinks the map path into its own `UserData/Maps`, and a relative path makes
   a dangling link, the server validates zero files, and the search reports
   `incumbent: no progress` — which reads exactly like "the old build cannot run
   this tape". (The repo's `tmsearch` canonicalises.)
3. **Cross-build files do not travel.** The 2022 server does not enumerate a
   ghost recorded by a 2024+ client at all (`0 ghosts (in 0 maps)`, no error,
   and `tmsearch validate` simply prints one row instead of two) — file format,
   not physics. A tape has to travel in a container of its own era, which §6
   shows how to do.

## 5. Route A — an Openplanet plugin: no

The question was whether a plugin could make the local client run another
build's physics. The mechanism exists and this project already uses it: our own
`tools/openplanet-plugin` calls `Dev::SetOffset`, `Dev::GetOffsetNod`,
`Dev::SafeReadUInt64` and `Reflection::GetType` to write engine memory by
member offset. What is missing is the target.

* **The physics model is not reflected.** In the Openplanet class reference,
  `GameData::CGameVehicleModel` exposes exactly one member —
  `UnnamedType PhyModel`. An unnamed type has no members to read or write.
  `Scene::CSceneVehicleVisState` exposes inputs and *visual* state;
  `Plug::CPlugSurface` exposes one method and no friction or grip field. There
  is no documented tuning surface to patch. (`raw/openplanet_*.txt`.)
* **The difference we would be patching is not a named coefficient.** `ksi2`
  measured it as a **step at a single contact** — 5 cm within 0.05 s at a
  `RoadBumpCurve1` at 138 km/h and 15 m/s of slip, where a whole steering unit
  does not move the car 5 cm in 3.6 s. That is the shape of a contact-solver
  change, not of a grip constant.
* **Data-versus-code could not be settled by swapping the data.** Running the
  2026 binary against the 2022 `Packs/` makes *every* ghost invalid in zero
  seconds, including the modern one that validates normally; the 2022 binary
  with 2026 `Packs/` exits immediately with status 100. **UNKNOWN**, and the
  swap is not a usable lever either way.
* **And it would not be an oracle.** Openplanet hooks the *client*. Our search
  needs 60+ headless re-simulations per second from `/validatepath`, which the
  client does not offer at any signature mode; Developer mode also enables
  School Mode, which disables leaderboards. Route B gives the exact 2022
  physics, verified 15 of 15, at full search speed, with no memory patching.

**The one thing Route A would still be for**: watching a 2022-physics run in a
modern client. A 2022 tape does not re-simulate today, so it cannot be rendered
faithfully by today's game — see `FILMING.md` before promising anyone a video.

## 6. Moving a tape between eras: chunk `0x0309202D` travels with it

A tape transplanted into another run's container **goes nowhere** — 0
checkpoints, on either build — even with matched tick counts, matched archive
start offsets and `ghost tape inject`'s read-back control passing. The tape
inside the written file is bit-identical to the donor's (`ghost tape diff`: 0 of
8203 ticks differ), and the donor's own file validates. So the container binds
something.

**MEASURED — it is chunk `0x0309202D`, and nothing else.** Bisect by
`tools/chunkswap` (copy one body chunk from file B into file A), on the current
server with modern files so every step has a live control:

| what was moved into rank10's file from rank14's | result |
|---|---|
| the whole 95 787-byte recording chunk `0x03092000` (telemetry, samples) | **80.534 — inert** |
| all 23 chunks except the input tape | **`wrong simu`, no time** |
| the 11 low chunks (`0x0303F007` … `0x03092023`) | 80.534 — inert |
| the 5 chunks `0x03092024` … `0x03092028` | 80.534 — inert |
| `0x03092029`, `0x0309202A`, `0x0309202C`, `0x0309202E`, one at a time | 80.534 — inert |
| `0x0309202B` (the splits) alone | 80.534 — inert (declared time differs, sim does not) |
| **`0x0309202D` (211 bytes) alone** | **`wrong simu`, no time** |

*Controls: `chunkswap` from a file into itself reproduces 80.534; `ghost trim`,
`ghost tape inject`, `ghost identity set` and `ghost declare --cps 4` are each
verified inert on the same file in the same way.* Also eliminated: tape distance
(the search's own 67.200 tape differs from its template in **71.7 %** of ticks
and validates), the declared checkpoint count, the respawn bits, and the archive
start offset.

`0x0309202D` is a provenance block: the recording build stamp, a wall-clock
pair, the title name, a ~36-byte per-session token, and — in one leaderboard
ghost — `Openplanet 1.28.0 (next, Public, 2025-08-16)`. **UNKNOWN**: which field
the validator gates on. The wall-clock pair is the leading candidate, because
the server prints `unexcepted walltime (103s)` when a container is trimmed
without it. *What would settle it: patch one field of the chunk at a time and
re-validate, the same way this bisect moved one chunk at a time.*

### The recipe, and it is verified

**Move `0x0309201D` and `0x0309202D` together.** Then a tape runs in any
container:

```bash
chunkswap --into <era container>.Ghost.Gbx --from <the tape's own file> \
          --id 0x0309201D --id 0x0309202D --out run.Ghost.Gbx
```

**MEASURED** — our 67.200 TAS tape and rank02's 68.442, both moved into
Roevhaal-era container `rank15` (a 2022 file) and run through the five segment
maps on the **current** server:

| tape in a 2022 container | CP1 | CP2 | CP3 | CP4 | finish |
|---|---|---|---|---|---|
| rank02 68.442 | 13.906 | 33.106 | 45.437 | 63.812 | **68.442** |
| our TAS 67.200 | 12.475 | 31.492 | 45.396 | 61.703 | **67.200** |

Ten gates, ten exact reproductions of each run's own splits. *That is the
control that makes the next line a measurement rather than an artefact:*

**Neither tape reaches CP1 under the 2022 build.** Both die inside the first
13.5 seconds — which is where `ksi2` measured the divergence, at 3.99 s in the
lap's first big slide. The incompatibility is symmetric: the 2022 field does not
run today, and today's runs do not run in 2022.

## 7. The server does not honour the build stamp

Worth testing directly, because the ghost carries the build that recorded it and
the server prints it back: **it is decoration.**

**MEASURED** — rewrite the stamp inside the compressed body
(`tools/strpatch`), same length, nothing else touched:

| file | stamp | current server | 2022 server |
|---|---|---|---|
| rank02 68.442, stamped **2022**-07-06/113150 | patched | **68.442, valid** | still not loadable |
| Roevhaal 63.546, stamped **2026**-02-02/128149 | patched | still `wrong simu` | **63.546, valid** |

*Control: the server reports the patched value in its `GameBuild` field, so it
reads the byte we changed — and neither outcome moves.* Two occurrences of the
string are patched per file, so the copy in `0x0309202D` and the copy the server
echoes are the same string.

**INFERRED** — the physics come from the binary, not from anything in the file.
There is no era switch to invoke, which is exactly why §1 is the route.


## 8. What this does not explain: the author time

The build explains why the record does not replay. It does not explain the
author time.

**MEASURED** — the best-sector sum over the ten build-113150 runs, whose splits
the 2022 server now reproduces exactly, is **63.263** (13.209 from rank03, then
17.651 + 11.309 + 17.130 + 3.964, all four Roevhaal's). That is **4.576 above
the author time of 58.687 — on the very build the author time was set on**.
*Control: it is the same number `ksi2` reported for the whole 15-run field, and
the modern half contributes no sector best, so the bound is entirely 2022
driving.*

**MEASURED** — a local search under 2022 physics from Roevhaal's own tape (§3,
64 workers, `--seg 1..4`) reaches **63.074 in 200 minutes / 714 480
evaluations**: 27 improvements, every one re-validated by the plain oracle,
**0 phantoms**. Its splits, read back through the segment maps on a fresh
process, are **13.492 / 31.143 / 42.452 / 59.424 / 63.074** — Roevhaal's own
first three checkpoints to the millisecond, with all 0.472 of the gain in the
last two sectors (17.130 → 16.972 and 3.964 → 3.650). The basin around the world
record does not fall away toward the author time either: 63.074 is still
**4.387** above it. *Control: the same file DNFs on the current server, as every
2022-physics tape does; the tape is banked as
`tm-oldbuild/tapes/oldbuild_2022physics_63074_SEARCHTAPE_declares_63546.Ghost.Gbx`,
md5 `35d7644a582be904d1d5e9aab75703cb`. Nothing was submitted anywhere.*

**INFERRED** — the author time is not "the 2022 car is simply faster". Whatever
it is, it is still 4.6 s below the best combination of sectors anyone drove on
that build. What the old server changes is that the question can now be asked
*under the right physics*: `ksi2`'s 10 m envelope over this field says the route
is worth 50.978, and that envelope is built from lines which, as of tonight, can
be re-simulated and searched instead of only read.

## 9. Beyond this map: how much of the corpus needs this

**MEASURED** — the recording build of every ghost we hold, read offline
(`chunkswap --show FILE 0x0309202D | grep date=`, no server needed) across 685
files in `tm-unbeaten`: **457 are build 128149, and all but four are 126529 or
later** (2024 onward). Only KEKL- SAUSAGE ICE's records carry **113150**, and
one file on **Spring 2023 - 24 (2-UP)** carries **120733**.

So the old-build problem is rare in this corpus — which is itself the reason it
went unexplained for so long: on most maps the whole field is modern and
everything re-simulates. *Control: Kacky Reloaded #290's top six, all 2024-2025
builds, all validate exactly on the current server in one batch.*

**MEASURED, and a limit** — the rank-6 ghost on **Spring 2023 - 24 (2-UP)**,
recorded **2023-03-31 on build 120733**, does **not** re-simulate today, while
the five 2026 ghosts on the same map validate exactly in the same batch. The
same symptom, a different map, a *later* build. It is also out of reach: the
2022-06-21 server will not load a 2023 client's file, and Nadeo publishes no
dedicated server between 2022-06-21 and 2026-05-15.

**INFERRED** — the physics that recorded build 120733 are neither today's nor
(as far as we can test) 2022's, and no archived binary can run them. Old-build
validation is available for the 2020-2022 era and for nothing since.
