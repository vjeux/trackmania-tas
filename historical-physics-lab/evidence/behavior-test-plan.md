# Deterministic measurement plan, tooling, and micro-map matrix

Purpose: turn every "Unresolved" row in `BEHAVIOR_CHANGELOG.md` into a Measured
one, reproducibly, with no invented numbers. Everything here is Rust or a stock
binutils invocation — no Python anywhere.

---

## 1. The two oracles, and which epochs each can reach

| Oracle | What it is | Determinism | Epoch reach | Readout |
|---|---|---|---|---|
| **Dedicated server** | `TrackmaniaServer /nodaemon /validatepath=.` re-simulates the input bitstream inside a `.Replay.Gbx`/`.Ghost.Gbx` and prints the simulated finish time. Native Linux ELF, no GPU, no auth. | Exact and repeatable; results independent of batch composition. | **2020-07-01 … 2022-06-21 only.** 43 dated builds retrieved, 39 distinct executables. Nothing is published after 2022-06-21. | Finish time in ms; on failure a coarse "reached N of M checkpoints". No splits, no telemetry. |
| **Exact client** (WhiteStick) | The real game binary with an Openplanet trace. | Exact per build. | Any archived client, including all Snow/Rally/Desert epochs and everything after 2022-06-21. | Full per-tick trace: position, speed, airtime, contact — the numbers §3 asks for. |

**This node can run neither end-to-end**: the faas box has no outbound network,
and the devserver has no exact-client execution. The server oracle *was* run here
and is blocked one step earlier — see §5.

## 2. Traps that invalidate results (all previously paid for)

1. **A DNF is reported as a present `ValidatedResult` with `Time: -1`** on 2022
   builds. An unguarded parser scores that as the best finish possible. Any
   non-positive time is a DNF. `tmphys` enforces this and has a test for it.
2. **Launch the server from the cell's own working directory** (`./TrackmaniaServer`).
   Invoking it by absolute path makes it validate *its own* directory's
   `UserData` and silently produce the wrong answer.
3. **`NotAvail replay: …` is cosmetic**; it prints next to replays that validate
   perfectly.
4. **`unexpected walltime` is the real cause of "Unvalidable"**: every ghost
   stores a `(t_start, t_end)` unix-second pair; the validator refuses unless
   `|walltime − racetime| ≲ 12 s`. Rewrite `t_end = t_start + round(race/1000)`.
5. **Some replays carry two `0x0309201D` chunks** (player ghost + embedded map).
   Only touch chunk index 0.
6. **A synthesised candidate carries its template's telemetry.** Only real
   recordings have true trajectories; never read a synthesised file's stored
   trace as a measurement.
7. **Always include an identity candidate** in any sweep and assert it
   reproduces the baseline exactly. A sweep bug once made 916 candidates DNF.
8. **A negative control is mandatory**: a build that must *not* reproduce the
   run. Without one, "everything matches" proves only that nothing ran.

## 3. Micro-map matrix

Each row is one isolated behavior with one observable. Every map is a straight,
minimal fixture with a start, the feature under test, and a finish plane at a
**known distance**, so that time-to-finish alone yields the quantity when only
the server oracle is available.

| # | Micro-map | Input tape | Primary observable | Quantity recovered | Oracle needed | Current status |
|---|---|---|---|---|---|---|
| M-01 | Flat straight, 400 m, plain road | full throttle, no steer | time to each of 8 finish planes at 50 m spacing | straight-line acceleration curve and top speed, m/s² and km/h | server (2020-2022) + client (all) | not run |
| M-02 | Ice booster + jump (the existing control map, SHA-256 `89a303c0…`) | full throttle, no steer, 22.000 s keyhold | entry/exit speed, take-off speed, airtime, landing z, finish plane | booster entry/exit speed, airtime, landing height | client | **run on current build (REF-01)**; needed per epoch |
| M-03 | Flat ice pad, 200 m | full throttle, fixed steer step at a fixed tick | lateral displacement at the finish plane | ice grip/slip threshold and drift onset, m and deg | client (trace) | not run |
| M-04 | Bobsleigh channel, 200 m | full throttle, steer sweep −1.0…+1.0 in fixed steps | time and exit lateral offset | bobsleigh steering authority | client | not run |
| M-05 | Water pool entry at a fixed speed | full throttle to entry, then fixed | bounce count, apex height, speed retained | water bounce response, m and km/h | client | not run |
| M-06 | Perpendicular wall at a fixed distance | full throttle, no steer | speed and position after contact | wall repulsion impulse, km/h and m | client | not run |
| M-07 | Reactor pad + free air | full throttle, then air-control steer step | vertical speed profile, angular rate | reactor response and air control, m/s and deg/s | client | not run |
| M-08 | Flat straight | steering sweep: hold each of 256 analog values for 1 s | steady-state yaw rate per input value | steering response curve, deg/s per unit input | client | not run |
| M-09 | Flat straight, analog ramp | ramp the analog axis to a target and hold | stored-vs-target residual at convergence | **analog snap** — confirms the 1×10⁻⁵ bound behaviorally | client, pre- and post-2024-05-22 | constant measured statically; behavioral confirmation not run |
| M-10 | Snow action-routing fixture | each action key pressed alone, then in pairs | which actions register per frame | Snow action-key routing/re-ranging | client, 2024-01-10 vs 2024-02-26 | not run |
| M-11 | Snow collision fixture: fixed obstacle at a fixed height | full throttle, no steer, press forward into it | contact/no-contact and post-contact speed | **confirms SN-1 hitbox change behaviorally**; the −0.095 m to −0.169 m height loss predicts a specific obstacle height at which contact flips | client, pre- and post-2024-02-27 | geometry measured; behavioral confirmation not run |
| M-12 | Rally custom-ice pad | full throttle, fixed steer | lateral displacement, speed retained | Rally custom-ice behavior | client, 2024-03-19 vs 2024-04-30 | not run |
| M-13 | Desert controls fixture | full throttle, steer sweep | steady-state yaw rate, grip onset | Desert control baseline | client, 2024-04-30 and current | not run |

**Precision budget.** Speeds on M-02 must beat the measured discrimination floor
of **0.080 km/h**; timings are good to 10⁻⁵ s. On the server oracle the only
readout is the finish time in whole milliseconds, so M-01 recovers acceleration
at 1 ms granularity per gate.

## 4. Tooling: `tmphys`

One Rust binary, std only, no crates, builds offline
(`cargo build --release --offline`). 6 unit tests, all passing.

| Subcommand | Use |
|---|---|
| `tmphys validate <spec.tsv> <workdir> [timeout_s]` | Runs the server-oracle matrix. Each row is `build_id⇥server_dir⇥map⇥ghost`. Stages an isolated cell per row (symlinks `Packs`/`GameData`, copies the executable, places map and ghost), launches from inside the cell, parses the report, writes TSV plus the raw log per cell. Enforces the `Time: -1` DNF guard. |
| `tmphys tunings <binary>…` | Extracts Nadeo physics-tuning identifiers (`IceDrift200624`, `06/12/2019_TurboAirControl_Ice`, `WallRepulse`, …) with file offsets, ASCII and UTF-16LE, decoding each embedded date. |
| `tmphys ledger <dir> [exe-name]` | First-appearance ledger of tuning identifiers across a `<date>/<exe>` tree, plus a per-build added/removed diff. |
| `tmphys find <binary> <hex> [ctx]` | Locates an exact byte pattern and prints file offsets with context — how an RVA from a report is pinned to real bytes. |
| `tmphys f32scan <binary> <lo> <hi>` | Enumerates float constants in a range. Used to read the analog tolerance constants. |
| `tmphys strings <binary> <substr>` | Offset-tagged string grep, both encodings. |

Worked example — recovering the analog constants (this is how S-2 was measured):

```
tmphys find  Trackmania.exe "F30F1164 8D74" 48        # -> unique file offset 0x2c2a0e
objdump -D -b binary -m i386:x86-64 \
        --start-address=0x2c2900 --stop-address=0x2c2a30 Trackmania.exe
tmphys f32scan Trackmania.exe 0x1d1d134 0x1d1d138      # -> 1e-5   (relative tolerance)
tmphys f32scan Trackmania.exe 0x1d1d7c8 0x1d1d7cc      # -> 1.0    (tolerance floor)
```

Note the section-delta correction: objdump in raw-binary mode prints
RIP-relative targets in file space, but `.text` and `.rdata` have different
VA↔file deltas in this image (`0x140000c00` vs `0x140001c00`), so a `.rdata`
target printed from a `.text` instruction is **0x1000 high**. Subtract it. The
abs-mask at the corrected `0x1d20000` reading exactly `ff ff ff 7f` is the
control that proves the correction.

Server-matrix example:

```
printf '%s\t%s\t%s\t%s\n' \
  2022-03-25 /srv/2022-03-25 /maps/KEKL_SAUSAGE_ICE.Map.Gbx /tapes/roevhaal_63546.Ghost.Gbx \
  > spec.tsv
tmphys validate spec.tsv /work 180
```

## 5. Why the server matrix did not produce new epochs here

Measured on this node, 2026-09-03, with three staged builds:

| Build | Result on a current-era map + tape |
|---|---|
| 2020-07-01 | ghost parsed (`1 replays parsed`) then **`Can't load map for the ghost`** |
| 2021-07-01 | **`validation of 0 ghosts (in 0 maps)`** |
| 2022-01-21 | **`validation of 0 ghosts (in 0 maps)`** |

The blocker is the **map container format**, not physics: a 2024/2026-era
`.Map.Gbx` cannot be loaded by a 2020-2022 server, so the ghost has nothing to
run on. Cross-epoch measurement in that window therefore needs an **era-matched
map plus an era-matched recording** — which is exactly what the parallel
2020-2021 audit is assembling (it is fetching 2020-era TMX maps and replays).
The three staged server trees and the `tmphys validate` harness are ready to
consume them: one spec row per build, no code change.

## 6. Order of work when the oracles are available

1. M-01 on all 39 distinct server executables — cheapest, gives the acceleration
   curve per code identity and will resolve the 2020-2021 epoch grid.
2. M-02 on each archived client from 2021-07-08 to current — this is the one
   micro-map already proven to produce clean event numbers.
3. M-11 and M-09 — they confirm two already-measured static deltas behaviorally,
   so they are the highest-confidence pair to run first on the client oracle.
4. M-03 … M-08, M-10, M-12, M-13.

Every run must carry: an identity control (a build known to reproduce the
reference), a negative control (a build known not to), and the run's map/ghost
SHA-256 recorded in `measurements.json`.
