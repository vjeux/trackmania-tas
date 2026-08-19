# Map 126859 — "Kacky Reloaded #290" — attack plan, argued from this map

TMX/unbeaten id **126859** · uid `NTU3ZGRlMzEtYzNiOC00YzJmLTk` · Nadeo mapId
`d762d097-7279-4c4e-a170-cad510919e32` · authors **Kacky** + **SuperrKuzco** ·
uploaded 2023-08-19 · AT **24062** · best human online WR **24342** (`zetos.`) ·
**22** records · gap **280 ms**.

Everything below was measured on this map on 2026-08-18. Nothing is assumed
from the other maps in this project.

---

## 1. Acquisition and the identity control

`ACQUISITION.md` recipe, fwdproxy, descriptive UA, 1.7 s between ghost pulls.

* The uid in `unbeaten_maps.json` looks truncated (`NTU3ZGRlMzEtYzNiOC00YzJmLTk`
  is base64 for a half GUID) — **it is not truncated, that is the real uid**,
  confirmed by trackmania.io returning the map for it. The map file itself came
  from Nadeo's own public endpoint keyed by the `OnlineMapId` GUID:
  `core.trackmania.nadeo.live/maps/d762d097-.../file`, 1 938 877 bytes,
  sha256 `ecb6a29674fcb62b6da55db2bfe18f19bca746317d5c9fd9857fb30961ba97fc`.
* **All 22 records on the board were pulled** (the whole field, not a sample):
  24342, 24634, 25379, 25459, 26219 … 32089, 32189, 41997.

### Whole-field re-simulation — 21/22 exact, and the one failure is diagnosed

```
tmtas validate --map map.Map.Gbx --jobs 22 ghosts/*.Ghost.Gbx
```

| result | count |
|---|---|
| re-simulated to the exact recorded millisecond | **21** |
| DNF in our oracle | 1 (`rank22`, 41997, `Asvyl`, last place) |

95.5%, and the single miss is the slowest run on the board by 9.8 s. Ranks
1–21, including the world record and every run we would ever seed from, are
exact. This is not the 203072 failure mode (29% mismatch *including the WR*):
the physics our oracle runs is the physics the field drove. **Proceed.**

Factory round-trip (`tmsearch --verify` → `tmtas validate`): 24342 exactly.
`tmtas selftest`: 10/10.

## 2. RESPAWNS — the question the brief asked first. Answer: **there are none.**

This matters more here than anywhere else in the project, because a Kacky map
is *supposed* to be respawn content. It is not, here.

* **The map has no checkpoints at all.** `tmmaps list` finds exactly two
  waypoints in 1765 blocks and 8110 items: `block#1174 RoadTechStart` (Spawn)
  and `block#1618 GateFinish` (Goal). Every one of the 22 ghosts declares a
  single split equal to its own finish time.
* **Therefore a respawn would return the car to the START**, not to a
  checkpoint, and would cost the whole run. Nobody does it.
* Measured directly rather than assumed (`k290 jumps`): across all 22 runs the
  largest position step between consecutive 50 ms telemetry samples is
  **11.21 m**, which is exactly the map's top speed (805 km/h = 11.2 m per
  50 ms). **Zero teleports, zero returns to the spawn point after t=3 s, in
  every one of the 22 runs.**

So `NbRespawns: 0` here is not a lucky property of the tapes — **it is forced by
the map's structure**, and the cost model is the ordinary one. The Kacky label
on this map is about difficulty, not about respawn routing.

## 3. Reading the medals

| medal | ms |
|---|---|
| **author** | **24062** |
| gold | 26000 |
| silver | 29000 |
| bronze | 37000 |

Gold/silver/bronze are round thousands — template values, not hand-tuned. The
author time is not round.

**But `atSetByPlugin` is true on this map**, and `inPlugin` is true: unbeaten.at
records that the AT was written by an Openplanet plugin rather than produced by
the editor's own validation drive. On the maps this project has already beaten,
the medal pattern argued *for* the AT being a driven lap. Here it does not, and
I will not claim it does. Two honest readings, and the evidence cannot yet
separate them:

1. Kacky campaign maps are assembled and published in bulk by an event
   organisation; setting the AT from a driven run through a plugin is normal
   there, and the time is still a time somebody drove.
2. The AT is a number typed by a tool and never driven.

This changes the framing of the deliverable, not the work: the useful output is
still a validated tape faster than 24062 plus a technique a human can practise.
It does mean "a human already drove this" is not available to me as an argument
that a given technique must be executable, so **the tolerance measurements have
to carry that weight instead**.

## 4. What kind of map this is — from the telemetry, not from the name

Route, decoded from the WR's own `CPlugEntRecordData` (489 samples @ 50 ms),
total arclength **2709 m** in 24.3 s:

| race t | what happens | km/h | y |
|---|---|---|---|
| 0–3.9 s | standing start, roll down a ramp, first bend | 0 → 190 | 46 → 38 |
| 3.9–6.2 s | **booster chain** — the map's first speed injection | 190 → **800** | 38 → 60 |
| 6.2–8.6 s | **launch #1**, long ballistic arc, airborne | 800 → 650 | 60 → 167 |
| 8.6–12.5 s | descend, land, run through the mid-section | 650 → 330 | 167 → 93 |
| 12.5–17.0 s | climb, then a **drop of 90 m** at 17 s | 330 → 447 | 93 → 160 → 66 |
| 17.0–19.1 s | flat run, **booster chain #2** | 447 → **751** | 66 |
| 19.1–21.5 s | **launch #2**, the big one, airborne the whole way | 751 → 670 | 66 → 153 |
| **21.5 s** | **the car hits a wall at ~670 km/h and is thrown back** | 670 → **223** | 153 |
| 21.5–24.3 s | **fall** down the face, ~78 m, never touching ground | 223 → 298 | 153 → 75 |
| 24.34 s | finish gate at (1521, 75, 1338), crossed moving −x | ~250 | 75 |

Measured field properties, all 22 runs (`k290 field`):

* **airborne 34–46 %** of every run. Two long flights plus the closing fall.
* top speed **776–805 km/h** for everybody — the boosters are not optional and
  nobody varies them.
* roll reaches **π** and pitch **π/2** on every single run: the car tumbles.
* **six of the 22 humans steer in exactly `{−127, 0, +127}` — pure keyboard —
  and they are ranks 1, 2, 5, 11, 16, 21.** The world record is one of them:
  110 change events, three values, gas held down for all but ~30 ticks, brake
  touched twice. That is ground truth for the alphabet, not a guess.

## 5. WHERE THE 280 ms IS — the cheap and decisive measurement

24 arclength stations along the WR's line, every run timed at each
(`k290 stations`, 50 ms resolution). Sector duration, field spread, and the
correlation of that sector with the final result:

| sector | race window | spread | corr with final time |
|---|---|---|---|
| 1–8 (start → 8.6 s) | 0 → 8.6 s | 0–150 ms | ≤ 0.54, mostly ~0 |
| 9–13 (the mid-section) | 8.6 → 15.0 s | 484–1652 ms | 0.11 … **0.70** |
| 14–22 (drop, booster 2, launch 2) | 15.0 → 21.4 s | 50–150 ms | 0.19 … 0.69 |
| **23–24 (the wall and the fall)** | **21.4 s → finish** | **600 / 14155 ms** | 0.29 / **0.97** |

**The last sector alone correlates 0.97 with the final time and carries the
entire spread of the field.** The WR takes 1476 ms from station 23 to the line;
the median takes 3–4 s; last place takes 15.6 s. Everything before 21.4 s is
essentially forced — the whole board is within 150 ms of each other through the
two boosters and both launches.

That is the opposite of the 227969/270051 finding (there, the spectacular
closing feature cost everyone the same). Here the closing feature *is* the map.

**Consequence for the search:** budget goes into the endgame — the state going
into the wall at 21.5 s, the bounce, and the fall — not into the approach. And
the resume-from-tick fork server is worth real setup cost, because a candidate
only needs the last ~3 s re-simulated.

Caveat recorded honestly: the mid-section spreads (sectors 9–13) are measured by
projection onto the WR's line, and those sectors contain a long air phase where
runs fly genuinely different arcs, so part of that spread is projection
artefact. The endgame result does not depend on them.

## 6. The plan

1. **Seed from rank01** (the WR, pure keyboard, and our exact identity control).
   Also test rank02 (24634, keyboard) and rank04 (25459, analog, 116 steer
   values) as independent basins — a pad seed beat a keyboard seed for
   unconstrained search on 227969, so do not assume the WR is the best seed.
2. **Three arms from the start**, each with its own `--root` (defect 1 in the
   brief), on the hardened build with the guard on:
   * analog unconstrained, whole tape;
   * **keyboard-constrained `--quant -127,0,127`, whole tape** — searched under
     the constraint, never projected afterwards;
   * analog, endgame only (`--lo 2000`, i.e. race ≥ 18.5 s).
3. **Then the fork server** resuming just before the wall, once the classic arms
   have shown where the gain is. Mutation floor comes from the build's own
   barrier — no hand-set margin.
4. **Robustness, not speed, for the deliverable**: score by the worst time over
   a ±1–2 tick window on the decisive inputs, and measure recoverable tolerance
   per input.
5. **The sub-tick plane is almost certainly INVALID here** and must be measured
   before use. The finish is crossed *airborne after a fall*, with roll varying
   across the field — that is the 227969 configuration exactly, where the plane
   lied by 19 ms. `u10an spread` / `finishcal` first; if the spread over a
   millisecond of travel is not small, do not use it at all.

## 7. First result (recorded at plan time, 60 s of search)

All three arms improved on the world record within 20 seconds:

| arm | best after 1 min |
|---|---|
| a — analog, whole tape | 24295 |
| **b — keyboard `{−127,0,+127}`** | **24252** |
| c — analog, endgame only | 24278 |

Finish rates 30 % (whole tape) / 54 % (endgame only), ~140 evals/s per arm.
The AT needs 24061. 190 ms to go.
