# PLAN — map 199100 "Spring 2023 - 24 (2-UP)" (uid `zFw7p8IFpSWwZcZMxLy4rTpX7o2`)

AT **51602**, human WR **52202** (JuntaoTM), gap **600 ms**, 6 records on the
board (the list said 5). Tags Reactor / Plastic / Altered Nadeo.
Author `.ar`; uploaded to Nadeo 2023-04-13.

## 0. Controls (done first, before any search)

* **§4 identity** — all six ghosts downloaded to `.part`, size- and
  `GBX`-checked, renamed on success, into a directory I created.
* **§8 field reproduction — 5 of 6 exact**, and the 6th characterised (below).

| rank | time | validated | respawns | game build |
|---|---|---|---|---|
| 1 | 52202 | **52202** | 0 | 2026-01-18 git 128114 |
| 2 | 52495 | **52495** | 0 | 2026-01-18 git 128114 |
| 3 | 52599 | **52599** | 0 | 2026-02-02 git 128149 |
| 4 | 54918 | **54918** | 1 | 2026-02-02 git 128149 |
| 5 | 57358 | **57358** | 1 | 2026-02-02 git 128149 |
| 6 | 113448 | **wrong simu, 7/10 cps** | declared `4294967295` (unset) | **2023-03-31 git 120733** |

Why I am treating this as a PASS and not a 203072 repeat:

* the failing run is the **only pre-2026 record** — set on the game build of
  2023-03-31, two days before the map was uploaded to Nadeo, three years and
  ~7400 build numbers older than every other record;
* it declares `NbRespawns = 0xFFFFFFFF` (the field is unset in that old ghost
  format) and its input stream contains `_` tokens no other tape has, i.e. it
  is an **old-format respawn-heavy run** — 113 s on a 51 s map, 71.8 s of it in
  the last sector alone;
* **respawns are not what breaks it**: ranks 4 and 5 each contain a real
  respawn (`NbRespawns: 1`) and both reproduce to the exact millisecond;
* **build 128149 is not what breaks it**: that is the build 9/18 of 203072's
  records failed on, and here **3 of 3** records set on it reproduce exactly;
* the file is not truncated — re-downloaded, sha256-identical, 118 338 bytes,
  `GBX` header (§8a).

Everything I will search from (ranks 1–5) re-simulates exactly, including the
WR. Reported to the user as a partial mismatch with this reasoning.

## 1. What the map is

Nine checkpoints, ten splits. Sector times, from the ghosts' own declared
splits (ms):

| sector | r1 52202 | r2 52495 | r3 52599 | r4 54918 | r5 57358 | best | best-by |
|---|---|---|---|---|---|---|---|
| 1 start→cp1 | **4211** | 4230 | 4230 | 4230 | 4228 | 4211 | r1 |
| 2 | 6915 | **6864** | 6916 | 6981 | 7146 | 6864 | r2 |
| 3 | 5872 | 5814 | 5982 | **5720** | 6016 | 5720 | r4 |
| 4 | 6739 | 6708 | 6917 | **6577** | 6836 | 6577 | r4 |
| 5 | 6125 | 5977 | **5943** | 6008 | 6082 | 5943 | r3 |
| 6 | 2231 | 2181 | 2193 | 2201 | **2130** | 2130 | r5 |
| 7 | 2950 | 2904 | **2887** | 2891 | 2889 | 2887 | r3 |
| 8 | 3122 | 3062 | **2982** | 2995 | 3025 | 2982 | r3 |
| 9 | **2391** | 2506 | 2698 | 2485 | 2692 | 2391 | r1 |
| 10 cp9→fin | **11646** | 12249 | 11851 | 14830 | 16314 | 11646 | r1 |

**Sum of sector bests = 51351 — that is 251 ms INSIDE the author time**, using
nothing but sectors five humans have already driven. The AT is not a wall here;
it is a combination nobody has assembled.

Structure of the run, from telemetry (`tmtraj decode-all`, 10 ms samples):

* 0–42.5 s: ground driving, `is_turbo` true almost throughout, 300–580 km/h,
  one long descent/climb complex per sector.
* ~42.5 s: **the launch** — the car leaves the ground at ~600 z, ~160 y and
  everything after that is a **powered reactor flight**: it climbs from y≈160
  to y≈340 while *gaining* speed (130 → 400 km/h), then arcs over and dives.
* Finish is crossed **airborne, descending**, at ≈ (148, 237, 1263),
  vy ≈ −69 m/s, vz ≈ +56 m/s.
* Sector 10 is 11.6 s long — 22 % of the run — and it is where the field
  falls apart: ranks 4 and 5 each **respawn** there (they blow the launch and
  restart from cp9, +3.2 s and +4.7 s). The launch is the map's filter.

Consequences:
* **The finish is airborne with attitude spread across the field → the
  sub-tick plane surrogate is presumed INVALID here** (the 227969 failure
  mode, defect 4). Measure the crossing-coordinate spread before even
  considering it; prefer the gate-ladder (`tmmaps gate`) if a vernier is
  needed at all. With a 600 ms gap, whole-millisecond scoring is plenty.
* Sector 10 is *powered*, not ballistic — steering during the flight is worth
  real time (the WR steers all through it and brakes twice mid-air). So this
  is not a "the launch is everything" map like 270051's jump; the flight is
  searchable.

## 2. Medals

author 51602 · gold 55000 · silver 62000 · bronze 78000. Gold/silver/bronze
are round thousands = hand-entered by the author; the AT is not round and is
the only medal that looks driven. Treat the AT as a **driven validation lap**:
a human did it, so a human-repeatable technique exists.

## 3. Attack order

1. Controls (done).
2. Sector table + route comparison across the five valid ghosts (done/ongoing)
   — which sectors correlate with the final time, which are wall-clips.
3. **Search, sector by sector, seeded from more than one human.** r1 is the
   best in sectors 1, 9, 10; r4 is 271 ms better than r1 over sectors 3–4;
   r3 owns 5, 7, 8. Test all five seeds — basins may not merge (270051).
4. Because sector 10 is huge and late, the fork server's resume matters most
   there; but per defect 2 the fork's answer is never evidence — every
   improvement goes through the plain oracle (the guard does it by default).
5. Once under 51602: robustness search (`worst time over a ±1–2 tick window`),
   then `tmsimp --mode kbx` per sector for the low-input family, and the
   sector-by-sector visual-cue guide.

## 4. Rules I am holding to

Nothing is ever submitted to a Nadeo leaderboard. Every claimed time is
re-validated through the plain oracle. A failed re-validation is a stop and an
incident. Rust only. Downloads rate-limited with a descriptive User-Agent.
