# Map 203330 — running notes

Everything here was measured on 65139.od.fbinfra.net (176 cores) on 2026-08-18.
`PLAN.md` has the acquisition, the geometry and the pre-search evidence; this
file is what happened after, including the things that were wrong.

---

## 1. Timeline of validated results

| time | how | validated by the plain oracle |
|---|---|---|
| 14018 | human WR, in-.-, the seed | yes (identity control, all 5 records exact) |
| 13995 | **the author time** | — |
| 13994 | 5 s of the first scout search | yes |
| 13987 | 10 s | yes |
| 13986 | 25 min scout | yes |
| 13985 | 18 min production search from 13986 | yes |

`tmtas validate --map <ABS>/map.Map.Gbx <ABS>/best_13985.Ghost.Gbx` -> 13985.

## 2. The map, in one paragraph

Start, steering disabled by a `GateSpecial8mNoSteering`, 7 `RoadTechSpecialTurbo2`
blocks take the car to 810 km/h in 3 s. Off the end of the road, a 3.5 s dive,
a redirect ramp at t=7.4 s, a scrubbing ground contact, and then a **cannon** at
t=8.51 s that sets the speed to exactly **1000 km/h** and fires the car down a
1370 m corridor in +z. At z=976 a wall spans the corridor from y=10 to y≈138
with **one empty cell**: x∈[160,192], y∈[64,72]. That is the hole. Clear it,
fall, land at z≈1315 on the corridor floor at y≈8, slide, and cross the finish
plane at **z = 1507.0**.

## 3. What is fixed and what is not — measured, not assumed

`tmtraj gates` (new subcommand) on true per-tick trajectories read out of the
simulator with `fk btraj`:

```
run                z=500     z=976    z=1200    z=1291    z=1400    z=1507
r01 (14018)       9894.5   11773.4   12695.8   13077.9   13542.3   14017.7
best_13987        9893.6   11771.8   12693.8   13075.8   13538.9   13982.7
best_13986        9893.6   11771.8   12693.8   13075.8   13538.9   13981.6
best_13985        9893.4   11771.4   12693.1   13074.9   13538.0   13980.4
```

- **The whole flight is fixed to ~2-3 ms.** Time at the hole varies by 2.0 ms
  across a 33 ms spread of finish times; time at z=1291 by 3.0 ms.
- **34 of the 37 ms won so far came from the last 106 m** (z=1400 -> 1507).
- Mechanism: the human WR lands at x≈182 and hits the lip of the finish
  platform at z≈1472 — its speed collapses from 800 to 312 km/h at t=14.00 s and
  it still finishes at 14018. The TAS lands at x≈171-175 and rides the same lip
  at 858 km/h. That is the whole margin.

Corroborating evidence from the humans: r05 (23153) flies the *same* arc and is
at z=1504 at t=14000 doing 786 km/h — it simply overshoots and never triggers
the finish. r03 (15478) and r04 (21230) clip the wall at z=976. Two of the five
records on this board are wall clips and one is an overshoot.

## 4. Why the approach cannot be improved

- 15 000 unbiased random moves in ticks 0-620: **zero** improvements, ever.
- An exhaustive window sweep (`tmsearch --sweep win`, new) forcing brake=0 over
  ticks [675,835) — the entire ground contact that feeds the cannon, where the
  human holds the brake down — changes the finish time by **+0 ms**. The brake
  is inert there.
- The cannon output is 999.8 km/h for every human and every candidate measured.
  The launch state is not reachable by the input tape.

## 5. Things our tools got WRONG on this map (all fixed, all worth knowing)

This map's car flies at 278 m/s. Every threshold in the trajectory stack was
calibrated on a 100 m/s ground car and four of them silently rejected the truth:

| tool | gate | what happened | fix |
|---|---|---|---|
| `fkdrv::blind` locate | `step/dt > 200 m/s => not a position` | the real vehicle state was rejected as physically impossible; "state not located" | `FK_MAXSPEED`, default unchanged |
| `fkdrv::blind` locate | swept `base-603616 ± 1.5 MB` | on this map the vehicle struct is at **base − 5 778 064**; the sweep never covered it | `FK_BLIND_CENTRE`, `FK_BLIND_SPAN` |
| `fkdrv::blind` locate | `mean_speed > 1.0` + first hit with `vel_err < 1.5` | a nearly static decoy has a trivially small `vel_err`, so it won the race against the real 226 m/s state | `FK_MINSPEED`, `FK_VELERR` |
| `fk traj` qualify | `rms < 0.05 m && max < 1.0 m` | reference telemetry is 50 ms apart = 13.9 m of chord here; the TRUE state measures rms 0.122 m / max 1.74 m (decoys: 566-1088 m) | `FK_RMS_GATE`, `FK_WORST_GATE` |
| `layout::check_rows` | `vel_err > 2.0 m/s => reject` | the same relative fidelity that gives 0.1 m/s at 100 m/s gives 2.05 m/s here | now `max(2.0, 0.02*mean_speed)` |
| fork-server clock | `clock = 36141 + 25.483*ms` (fitted on map 2) | this map is `clock = 5431 + 26.49*ms`; the map-2 formula puts a "tick 600" checkpoint ~110 ticks late | `FK_CLOCK_A`, `FK_CLOCK_B` |
| sub-tick plane | crossing tested only for travel in **-x** | this map's finish is approached in **+z**; the plane would never fire and the objective would silently stay integer-ms | `plane_axis` + `plane_dir` |
| `fk fs --mode cal` | binary-searches down from the LAST tape tick | the tape is 1552 ticks and the race ends at ~1402, so the last tick is inert and cal reports "even the last tick is already consumed" | use `--mode scan` / `--mode test` instead |

**None of these were physics problems and all of them read like one.** The
lesson for the next map: before trusting a measurement stack on a map that is
faster, bigger or stranger than the one it was tuned on, run its own acceptance
test first (`fk fs --mode test`, `fk traj` on a ghost with known telemetry).

## 6. The fork server on this map

- **Exact**: 200/200 at boundary tick 620, 250/250 at 1000, 250/250 at 1200.
  Zero mismatches, ground truth = full `/validatepath` of the same tapes.
- **Throughput**: 0.83x at tick 620 (SLOWER than the classic path — the resume
  only skips 39% of a run that is cheap anyway), 1.06x at 1000, 1.25x at 1200.
  On a 14 s map with the action in the last 15% the fork server is not a
  throughput win worth having on its own.
- It is worth having anyway, because the **sub-tick timing plane** lives in it.

## 7. The sub-tick objective (adopted from the 191465 agent)

Integer milliseconds are a 23.5 cm ruler at this map's finish speed, and the
search had visibly plateaued on ties: an exhaustive 13 861-candidate window
sweep from the 13985 incumbent returned dozens of `+0` and nothing better.

Armed at the measured finish plane (`--plane 1507.05 --plane-axis 2
--plane-dir 1`) all 8 smoke-test workers calibrated to the same whole-tick
offset (-10) and the incumbent's continuous time came out 13984.70 ms against
the validator's 13985. Within 12 s the search was at 13984.49 — moves the
integer objective could not see at all.

## 8. Artefacts

- `map.Map.Gbx`, `map.json`, `lb.json`, `ghosts/` — the map and all five records
- `best/` — validated candidate tapes
- `traj/` — true per-tick (10 ms) trajectories, 29-column CSV
- `tools/` — the toolchain as patched for this map
