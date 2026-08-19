# 279197 — "Fall 2025 - 01 Reverse CP1 End" — plan, from evidence

MapId (TMX) 279197 · uid `_jkbEKnkKNw1B_TOgzbm5IYlkfc` · Nadeo mapId
`250b5dc0-3f5f-4c80-97a3-afd5399f4d8e` · author `in-.-`.
AT 10598 · human online WR 10602 (ShcrTM) · 561 records · gap 4 ms.

Everything below was measured on this map on 2026-08-18, not assumed.

## 1. Acquisition and the identity control — PASSED

`ACQUISITION.md` recipe followed exactly (fwdproxy, descriptive UA, 1.6 s
between ghost downloads). Pulled the `.Map.Gbx` and **27 finishing human
ghosts**: the top 15 (10602…10615) plus deliberately slower slices at
`?offset=50/150/300/500` (ranks 51-53, 151-153, 301-303, 501-503, out to
10800).

```
tmtas validate --map <ABS map> --jobs 27 <ABS ghosts>
```

**All 27 re-simulated to their exact recorded millisecond** (10602, 10603,
10605 … 10798, 10800, 10800). The map loads, the ghosts decode, and the oracle
agrees with the public leaderboard on 27 independent runs. Carry one of these
in every batch from here on.

Factory control: `tmsearch --template r001 --verify id.Ghost.Gbx` →
`tmtas validate` = 10602. The candidate generator round-trips.

`tmtas selftest` on this node: 10/10.

## 2. Reading the medals — the AT is a driven lap, not a formula

| | value |
|---|---|
| author | 10598 |
| gold | 12000 |
| silver | 13000 |
| bronze | 16000 |

Gold/silver/bronze are round thousands — the map maker did not hand-tune them,
they are the template values these "CP1 End" community variants ship with. The
author time is **not** round, and it is 4 ms faster than the best of 561 human
attempts. `in-.-` is the map's author and is the same person who authored
`Training - 10 Long` (the other 1 ms unbeaten AT), i.e. a maker who validates
his own maps by driving them. Conclusion: **10598 is a real driven validation
lap**, so it is reachable by definition, and it is only 4 ms — one twentieth of
a car length at this map's finish speed — beyond the human field.

## 3. What this map actually is

`tmmaps list` gives the whole waypoint set — there are exactly two:

```
block#2543 PlatformTechStart tag=Spawn cell=(8,16,20)
item#799  "cp1end"  blocks\roadborder.Gbx.Item.Gbx  tag=Goal
          cell=(25,15,24) pos=(800,56,768) yaw=-3.141593
```

So this is not a normal map with a Finish block. The maker took the campaign
map, deleted everything past CP1 and **placed a custom item as the Goal** — a
repurposed road-border, axis-aligned (yaw = −π). There are **no checkpoints at
all**: one sector, start to gate. That has two consequences:

* `tmtas splits` returns a single split (= the finish), so the classic
  "diff the splits across the population" has nothing to bite on. Sector
  analysis has to come from the ghosts' own telemetry instead.
* the search gets **no shaping signal**: a DNF returns "no progress", not a
  checkpoint count. Every gradient comes from runs that finish. Measured DNF
  rate under `--ops mix2`: **58%**. That is survivable (42% of evals score) but
  it means operators that keep the car on the road are worth more here than
  operators that explore.

Route, from the decoded WR telemetry (`tmtraj decode`, 213 samples @ 50 ms):

| t (s) | what happens |
|---|---|
| 0.0 | spawn at (272, 66.0, 656), gas on, gear 1 |
| 0.7–3.4 | left-hand bend, then a hard steer flip at 3.2 s (−0.80 → +0.69) |
| 3.6–5.8 | **downhill** y 66 → 58, gear 3, 160 → 235 km/h |
| 5.8–7.8 | flat straight, gear 4, 235 → 281 km/h |
| 7.8–10.6 | **one long right-hand sweeper**, radius ≈ 140 m, taken flat out; still accelerating 286 → 341.7 km/h |
| 10.602 | crosses the gate at ≈ (772.6, 58.0, 750.7) heading +z |

`tmtraj stats` over the 27: pairwise RMS lateral separation mean 1.74 m
(min 0.23, max 4.58); the WR is *typical*, rank 8 of 27 by centrality. The WR's
speed is only **+0.6 km/h above the field median** at every one of 12 stations.
Nobody is driving a different line, and nobody is meaningfully faster anywhere
in particular. This is a flat-out acceleration run that the field has ground
completely flat — exactly the "sub-tick, not a new line" prediction.

**The finish speed is a hard cap.** The WR's speed is *exactly* 341.7 km/h
(94.9167 m/s) from t = 10.45 s to the flag, to every decimal the decoder
prints, and every one of the 27 humans is at 341.7 km/h at the gate. So in the
last ~150 ms the car cannot go faster; time can only be bought as **distance
already covered**. At 94.9167 m/s, **1 ms = 9.49 cm** and the 4 ms to the AT is
**38 cm** of extra progress over 597 m of route (0.06%).

## 4. The finish plane, and the vernier

The vernier is `tmmaps probe`, which relocates the Goal gate and re-times every
ghost against it. It needed one fix for this map, now in the tree:
`segments::move_gate` *swaps in the stock finish-gate item model*, which on a
map whose only Goal is a custom `cp1end` item deletes the finish outright — the
first sweep returned DNF at every offset **including the identity placement**,
which is what caught it. Added `--keep-model` (`probe.rs::gate_map_opt`) to
move the map's own item and leave its model alone.

With that, `--at 800,56,768 --cell 25,15,24 --yaw -3.141593 --axis z` at offset
0 returns **10602 / 10598 / 10800** for r001 / a candidate / r503 — the gate
machinery reproduces the unmodified map exactly. Identity control for the
vernier: PASSED.

Coarse sweep ±16 m: **10.5 ms per metre**, i.e. 95.2 m/s — the finish is a
z-normal plane and the car meets it at the capped speed. Fine sweep (6 mm
steps) shows the staircase is **not** a uniform 9.49 cm/ms: bin widths for r001
were 0.042 / 0.042 / 0.144 / 0.096 m and the value 10599 is **skipped
entirely**. The gate-position → reported-time map is monotone but quantised on
a ≈4.8 cm grid. Not yet explained (see §6); it does not affect correctness of
anything above, and the vernier is still a valid *ranking* instrument, because
what it ranks is the δ at which a given run's staircase steps.

## 5. What I expect each operator to be worth

* **Plain full-tape search from the human WR** — the humans steer with a pad:
  the WR's steer trace uses coarse analog values and holds them for tens of
  ticks through a 2.8 s sweeper. A per-tick 8-bit steer trace should recover
  several ms with no new line at all. *Expected: the 4 ms to the AT, and
  probably more.*
  **MEASURED: 10601 at 8 s, 10598 (= AT) at 28 s, 10597 at 6.5 min, 10596 at
  7.0 min, all from 96 workers at ~1900 eval/s. Re-validated through the plain
  oracle: 10596. The AT is beaten.**
* **Sweeper-localised search** (`--lo/--hi` over ticks 780–1061): the last 2.8 s
  is 26% of the tape but all of the steering authority, and a change there
  invalidates less of the run. *Expected: the best ms-per-eval of any arm.*
* **The vernier as a tie-breaker.** On a 1 ms-quantised objective the search
  spends most of its time on a plateau of candidates that all report the same
  integer. Ranking a shortlist by the δ at which its staircase steps turns that
  plateau into a gradient. *Expected: worth ~1 ms at the point where the plain
  search stalls, not before.*
* **Fresh seeds from slower humans** (r051, r151, r301, r501): the map-2 finding
  is that independent basins do not communicate and cannot be spliced, so
  diversity must come from seeds. *Expected: mostly worse, but cheap insurance
  against the WR's basin being a local optimum.*
* **Bigger random search / more cores**: last resort, lowest expected value per
  the map-2 experience (a converged neighbourhood is ~empty).

## 6. Open questions

* Why is the gate-offset staircase quantised at ≈4.8 cm and why is 10599
  unreachable for r001? Suspect the crossing-time interpolation, not the
  geometry. Only matters for how finely the vernier can rank.
* Whether the 341.7 km/h cap is gear-limited (the WR is in gear 5 for the last
  50 ms) — if a different gear-change timing raises the cap, that is a
  different and much larger prize than 4 ms.
