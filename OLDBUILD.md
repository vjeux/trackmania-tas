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
  workers, *confirmed by the plain oracle, 0 phantoms*, and further on a longer
  run.

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
   not physics. A tape has to travel in a container of its own era. Which
   raises §6.

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

## 6. UNKNOWN: a tape does not survive transplant into another run's container

Moving our own inputs onto a 2022 container is the obvious next step, and it
does not work — for a reason that has nothing to do with builds.

**MEASURED** (`raw/g14_cur.txt`, `raw/gt_old.txt`): with `ghost tape extract` /
`trim` / `tape inject` — same build, same map, same tick count, same archive
start offset, read-back control OK on every write —

| file | server | result |
|---|---|---|
| rank14's own tape, trimmed to 80.530, in its own container | current | reaches **4 of 5** checkpoints |
| rank10's tape (validates 80.534 in its own file) in rank14's container | current | **0 checkpoints** |
| rank15's own tape re-injected into rank15 | 2022 | **103.785 exact** |
| rank02's tape, and our 67.200 TAS tape, in rank15's container | 2022 and current | **0 checkpoints**, both |

*The controls are in the table: the self-transplants pass on both servers, so
the tooling writes a file the server can drive.* Four cross-transplants, two
builds, both tool paths (`ghost tape inject`, and a from-scratch re-carry that
copies only steer/accel/brake into the container's own packet stream) — all
reach no checkpoint at all.

This is not the search's usual edit: a template patched in place by
`Factory::apply` works with **71.7 % of its ticks changed** (our 67.200 tape
against its rank02 template) — so it is not about how different the inputs are.
Something the container holds is bound to the run that recorded it.

**What would settle it**: bisect the container. Take rank10's own working file
and move it one chunk at a time toward rank14's — declared time, splits,
identity, telemetry record, archive `field0` — validating after each step; the
step that reaches 0 checkpoints names the field. `ghost` already owns most of
those edits as first-class commands.

**Why it matters**: until it is settled, a tape can only be run under 2022
physics if it is *already* in a 2022 container — which means searching from a
2022 ghost as template (that works, §3) rather than porting a modern tape back.
And it means our 67.200 cannot yet be scored against 2022 physics.
