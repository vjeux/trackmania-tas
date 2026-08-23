# 267460 `Impossible Mini Trial 2` — **18.234**, and the ladder was never saturating

Arm `ladder`, 2026-08-23, node `102237.od.fbinfra.net`, branch `ladder`.
Times in **seconds**, speeds in **m/s**.

AT **16.888** · human WR **23.068** (Wirtual) · previous incumbent **21.022**.

> ## **A validated lap: 18.234.**
>
> **2.788 s inside the previous incumbent, 4.834 s inside the human world
> record, 1.346 s over the author time.** Full `ghost verify` PASS — *"V7 oracle
> re-simulated the written file: 18.234 == the declared time"*, kappa 1.000 —
> on the file as written to disk. Across the nine arms, **109 improvements
> confirmed by the plain oracle, 0 phantoms**.
>
> It is the fast pit of `CREST.md` (turbo pad at race 11.44 against the
> incumbent's 14.79) with the whole ending re-driven, exactly as `ENDING.md`
> prescribed. The 3.31 s of pit advantage is no longer being thrown away: 2.79
> of it is now on the clock.

---

## 0. The headline finding, and it corrects the last handover's

`ENDING.md` closed with *"the arclength gradient saturates before the flag, and
once it does the ladder needs a second rung"*. **It does not saturate. The line
was cut off 63.3 m short of the flag, and the cut was a function of the
CANDIDATE TAPE's length.**

`RefLineData::from_samples` dropped every reference sample at or past `nticks`,
and `nticks` is the tick count of the tape being searched. A graft that reaches
the same place sooner is shorter by exactly the time it saved — so its reference
line was short by exactly the part of the route still to be driven, and the
line's own last index then sits somewhere the car can reach **without crossing
anything**.

The fingerprint is in the last arm's own logs, and it is decisive: *the
reference line's length tracked the tape's tick count.*

| search | template | tape ticks | "reference line" | outcome |
|---|---|---|---|---|
| `en_endingsearch` | `p2` | **2462** | **560 m** | **finished: 21.617** |
| `en_graftsearch` | `cv_GRAFT` | 2131 | 471 m | 100 %, DNF |
| `en_graftsearch2` | `cv_GRAFT2` | 2129 | 470 m | 100 %, DNF |

Two ticks of tape, one metre of line. Nothing about a map does that.

On the incumbent's own trace, **470 m of arclength is race 19.730 at
(1053.1, 52.1, 650.5)** — on the return leg, still 63.3 m of driving from the
flag. **The flag is at 533.3 m** (race 21.020, (991.0, 50.6, 642.8)). So "100 %
of the reference line" was a point the car reaches at 42 m/s in mid-air with a
row of pillars and a jump still to come. Two searches sat on it for 500 000
evaluations, and the one search that had the full line — the only one whose
template was the incumbent's own 2462-tick tape — is the one that finished.

**The fix**: the resampling grid is now long enough for every sample the tape's
clock can address, whatever the tape's own length; and the line reports its race
span and how far it runs past the tape's end, so a truncation cannot be silent
again.

```
fork: reference line 560 m over race 10.770..21.790, 29 tick(s) of it past the
      tape's own end; predicates disarmed after 0 m
```

## 1. Controls

| control | result |
|---|---|
| plain oracle re-simulates the two references | human **23.068**, incumbent **21.022**, exact |
| **`ghost tape graft` incumbent head + own tail at tick 1630, injected, re-simulated** | **21.022** — the graft pipeline is still a no-op when it should be |
| **`ghost trim --to 25000` on the incumbent** (the tape-lengthening this arm relies on) | **21.022** — appending 194 ticks after the flag changes nothing |
| **the same tape and the same reference, before and after the line fix** | `cv_GRAFT2`'s seed scored **311 m of 470 (66 %)**; it now scores **311 m of 560 (55 %)**. The same metres, against the true line. The decoy tape is 245 m in both |
| **the new regression test can fail** | reverting `nticks.max(need)` makes `a_short_candidate_tape_does_not_shorten_the_reference_line` fail with half the line; the fix passes |
| the whole search suite | **95 checks pass**, `TM_REQUIRE_ENGINE=1`, up from 93 |
| every trace quoted here | `fk trace`'s own self-check ok |
| the decoy test | printed before the first candidate on all nine arms — **and it caught a live decoy of its own, §4** |
| the banked lap | `ghost verify` **OK**, all checks, on the written file |

## 2. What else had to change, and why each one was load-bearing

### 2a. The calibrated flick was frozen by the fork boundary, not by choice

`ENDING.md` §6: the incumbent's ticks **1633–1637 are five ticks of full lock
calibrated to 43.7 m/s**, and a car arriving at 40.7 turns much harder on them.
In `cv_GRAFT`'s coordinates those are ticks **1302–1306** — and the last arm's
window started at **1321**, because a fork at tick 1250 stops at probe **1318**
and *an edit below a worker's own resume tick is a silent no-op*.

So the flick was not left editable-but-unsearched; it was **unreachable**.
Forking at tick **1200** stops at probe **1271**, which puts the pad entry
(tick 1299) and the whole flick inside the window. Every arm here mutates
`[1273, 2306)`.

### 2b. The tape has to be long enough to finish in

`cv_GRAFT` is 2131 ticks and its last tick is race 19.76. A tape that runs out
of inputs is a car with no throttle. `ghost trim --to 21500` lengthens it to
2306 ticks; the control above says lengthening is inert.

## 3. Where the ending actually dies — the map says it in one command

`mapgeom where map_267460.Map.Gbx --at 991,643`:

```
item  GateFinishCenter32mv2   at (990.00, 58.00, 656.00) yaw -1.571
item  ObstaclePillar2m        at (1023.00, 50.00, 641.00)
item  ObstaclePillar2m        at (1023.00, 50.00, 649.00)
item  ObstaclePillar2m        at (1023.00, 50.00, 657.00)
item  ObstaclePillar2m        at (1023.00, 50.00, 665.00)
```

**A row of four 2 m pillars at x = 1023, y = 50, every 8 m of z.** The incumbent
passes them at (1024.8, **54.3**, 646.8) — over the top and in a gap. Everything
this arm searched died on them, and the two failures are worth keeping because
they are two different decoys and the corridor is what let each one through:

| corridor | best | what the tape actually did |
|---|---|---|
| 10 | 507 m of 560 | **airborne over the return leg and falling.** It scored 507 m while sitting **8.6 m below** the line, because 8.6 < 10. It hits y = 8 at x = 980 |
| 6 | 534 m of 560 | **hits the pillar at z = 649 dead centre** at (1023.7, 50.9, 649.0): 42.1 → 25.7 → 12.9 m/s in two ticks. It then **crawls west at 7–9 m/s along y = 50 for three seconds**, tracking the line closely enough to bank 534 m of 560, and stops at (992.3, 49.6, 639.5) — **0.5 m outside the finish gate's z span** |

> **The crawler is a local optimum on arclength, and it is 1 m from the flag.**
> An objective that measures "how far along the line did you get" cannot tell a
> car that is going to finish from one that is going to arrive. That is the
> real second rung, and it is not the one either candidate in the handover
> named.

## 4. The rung that produced the lap — and the decoy the tool caught on the way

The pillars say what the objective is: **be past x = 1023 at speed, on the
line.** A gate box just west of them, keyed on westward velocity:

```
--gate 'xmin=1014,xmax=1020,ymin=49,ymax=58,zmin=640,zmax=652'
--gate-key '-vx'
```

The crawler seed scores **+10.4**. A tape that clears the pillars scores **39**.
There is nothing to tune: the bands do the rest, and `Finished` is a time again.

**The first version of that gate was a decoy and the search found it in 270
evaluations.** With the box opened to `y 42..60, z 634..670` and the key left as
plain `speed`, the winner was a car **falling through the bottom corner of the
box at 20 m/s downward** — `v (-0.14, -19.18, -6.93)`, `|v|` 20.4, at y = 42.2.
Maximal speed, wrong direction, off the map. The startup decoy test cannot catch
this family (§5.11 of `SEARCH.md`: *a fast, driven tape that maximises the key
somewhere useless*); what caught it is that every improvement prints its state,
and the state was a fall. Narrowing y and z to the line and keying `-vx` instead
of `speed` fixed it, and that arm's first improvement was already a finish.

### The run

| arm | seed | window | objective | result |
|---|---|---|---|---|
| rt1 | the graft | `[1273,2306)` | arclength, corridor 10 | 322 → **508 m** — past the old 470 m ceiling in 90 s |
| A | the graft | `[1273,2306)` | arclength, corridor 4 | 266 → 475 m |
| B | the graft | `[1273,2306)` | arclength, corridor 6 | 310 → **534 m** (the crawler) |
| C | the crawler | `[1890,2306)` | finish-gate box | stuck in the crawl basin — 416 ticks is not enough run-up to dodge a pillar |
| D | the crawler | `[1700,2306)` | the pillar gate's **first, wrong** box (`y 42..60, z 634..670`, key `speed`) | **the decoy above** — killed at 270 evaluations |
| **D2** | the crawler | `[1700,2306)` | **the pillar gate** | **first finish 19.489 at 2 490 evaluations; 18.309 at 972 420. 41 confirmed, 0 phantoms** |
| E | the crawler | `[1273,2306)` | the pillar gate | 18.337, 37 confirmed, 0 phantoms |
| **F** | 18.309 | `[1273,2306)` | **plain finish time** — the band is a time now, so the gate has done its job and comes off | **18.234**, 16 confirmed, 0 phantoms |
| G | 18.309 | `[1273,2306)` | finish time, `--temp 0.030` | 18.285, 15 confirmed, 0 phantoms |

## 5. The lap, traced

`fk trace`, self-check ok:

| race | x | y | z | speed | what |
|---|---|---|---|---|---|
| 11.15 | 832.7 | 114.3 | 706.1 | 38.7 | on the deck, the fork's resume |
| 11.55 | 849.2 | 114.0 | 704.9 | 48.6 | boosted off the **turbo pad** — the incumbent gets here at 14.79 |
| 14.95 | 1053.5 | 45.0 | 709.1 | 72.4 | the bottom of the 60 m drop |
| 15.75 | 1088.5 | 42.1 | 683.1 | 59.1 | the wall-ride, deepest point |
| 16.33 | 1069.9 | 46.9 | 660.1 | 48.5 | out of the hairpin, `vy` **+16.9** — the second jump |
| **17.41** | **1023.2** | 51.5 | **653.3** | **43.2** | **through the pillar row, in the gap between z = 649 and z = 657 — no speed lost at all** |
| 18.13 | 993.6 | 50.0 | 650.6 | 39.8 | into the finish gate at 39.8 |
| **18.234** | | | | | **the flag** |

**It threads the pillars.** The 18.309 lap of arm D2 still grazed the z = 649
one at (1023.7, 51.2, 650.1) and paid 3.7 m/s for it; twenty-five minutes of
plain finish-time search moved it 3.2 m across into the gap, and that is most of
the 0.075 s between the two.

## 6. Tooling

| what | why |
|---|---|
| **`RefLineData::from_samples` no longer lets the candidate tape truncate the reference line** | §0. The grid covers every sample the tape's clock can address; the line carries its race span and `past_tape`, and the search prints both. `a_short_candidate_tape_does_not_shorten_the_reference_line` pins it and fails on the old code. `samples_before_the_tapes_first_tick_are_dropped_and_the_head_clamps` pins the other end, which is correct behaviour and was worth stating |
| **`tmtraj route` grows a derived `s` column** | cumulative path length from the first row — the same quantity the fork search reports progress in. `--where 's>533' --first 1` is how "470 m is 63 m short of the flag" was measured, and it is one command instead of a spreadsheet |

Nothing else needed adding. The pillar gate, the `-vx` key and the box are all
`--gate` / `--gate-key` written on the command line.

## 7. What the next arm should do

1. **The pit is still worth 9 m of energy height** (`ENDING.md` §3), and that
   debt is now paid in the ending rather than at the pad: our car reaches the
   wall-ride 8 m/s slower than the incumbent (59.1 against 66.3 at x = 1088) and
   exits the hairpin at 48.5 against 54.4. Buying one more turn of the spiral
   costs ~1.8 s on the human's timings and would return a faster, higher second
   jump — **the author time is 1.346 away.**
2. **The finish gate itself is a 21 m/s wall.** The lap arrives at x = 993 at
   39.8 and the flag is 2 m later; the incumbent arrives at 47.4. Nothing here
   optimised the last thirty metres on purpose — the pillar gate stopped
   scoring at x = 1014 and finish time took over — and it is the cheapest
   remaining window to search on its own.
3. **Do not re-run an ending search with a corridor over ~5 m on this map.** The
   two failures of §3 are both a corridor admitting an error the map does not:
   the pillar gaps are 8 m apart and the finish gate's z edge is at 640.

## 8. Artefacts

Banked to `~/persistent/private-30d/tm-unbeaten/267460/ld_20260823/`, with
`ld_MANIFEST_v1.md5`.

| file | what |
|---|---|
| **`ld_LAP_18234.Ghost.Gbx`** | **the lap: 18.234, full `ghost verify` PASS on the written file, telemetry regenerated (kappa 1.000), declared time from the oracle** |
| `ld_LAP_18234_raw.Ghost.Gbx` | the search's own output before `regen`/`declare`, for provenance |
| `ld_LAP_18309_gatearm.Ghost.Gbx` | arm D2's own best, the first lap the pillar gate produced |
| `ld_lap18234_trace.csv` | §5's trace |
| `ld_b534_crawler.Ghost.Gbx` + `ld_b534_trace.csv` | the corridor-6 crawler: hits the pillar at z = 649 and banks 534 m of 560 at 8 m/s |
| `ld_b507_faller.Ghost.Gbx` + `ld_b507_trace.csv` | the corridor-10 faller: 507 m while 8.6 m below the line |
| `ld_gA_graft.Ghost.Gbx` | the seed: `cv_GRAFT` lengthened to 2306 ticks |
| `ld_r*.log` | all nine searches, decoy lines and confirmations included |
| `ld_ladder_commits.bundle` | the branch |
