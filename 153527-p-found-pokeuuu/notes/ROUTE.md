# 153527 `P-Found - Pokeuuu` — REOPENED: the author's route **is** shorter, and it is in the human's own ghost

Worked 2026-08-18/19 on node 145855. **Verdict: the previous closure does not
stand.** No time claimed, no search run, nothing validated on the oracle (this
map has no working oracle control — see §6).

> Prefix `route_`. This does not modify, replace or contradict the arithmetic in
> `RESULT.md`; it replaces the **telemetry those numbers were computed from**.
> `RESULT.md` §1 (no embedded author ghost) and §2 (the sole record is a
> dead-build ghost) are unaffected and I did not re-do them.

---

## 0. The answer, in one paragraph

The question was "is the author's route shorter than the 26 km the human
drives, and by how much". The answer is **yes, by about 17 %, and you do not
need any map geometry to see it — it is already inside the human's own
trajectory.** Take the human's surviving line (every respawn-bracketed retry
already deleted) and cut out every place where the car returns to a point it
has already occupied, going the same way, at the same speed, with the same
attitude. What is left is **892.148 s and 20 342 m** against the human's
1 214.465 s and 24 546 m — and the author time is **939.283 s**. The de-looped
line is **47.135 s inside the author time**. Across the whole range of splice
tolerances it runs 745–966 s, i.e. it straddles the author time. **The author
time is not a pace nobody can produce. It is approximately this human's own
driving with the wandering taken out.**

---

## 1. First, a correction: the published telemetry is a different car

`evidence/wr_telemetry.csv.gz` (85 811 samples) is what the previous residue
table, the 26 023 m, the 77.1 km/h and the "path/displacement 17× at CP3 and
20× at CP12" were computed from. It is byte-identical to what
`tmtraj decode` produces today, so this is not a mistake anyone made by hand —
**the decoder returns the wrong entity on this ghost.**

`CPlugEntRecordData` here holds **49 `CSceneVehicleVis` entities**. The stock
decoder takes the one with the most samples (#9, 85 811). The player is
somewhere else entirely: **46 further entities that tile the race back to back
at 50 ms with zero overlap and zero gaps.**

The referee is the map's own waypoint blocks, and it is not close:

| | entity #9 (what the decoder returns) | the 46 per-life entities |
|---|---|---|
| inside the right checkpoint cell at the 12 declared splits | **2 / 12** | **12 / 12** |
| first sample | (785.4, 78.5, 541.8), **falling at 81.4 km/h**, 50 m under the spawn | (784.0, 82.0, 592.0) at 1.6 km/h — dead centre of spawn block cell (24,18,18) |
| last sample | (485.6, 178.2, 1036.0) — nowhere near the goal | (304.0, 234.2, 527.8) — inside goal block cell (9,37,16) |
| visits the spawn / CP10 / CP11 / goal cells | **never** | yes, at 0.0 / 4850.5 / 5231.1 / 5661.3 s |
| coverage | 23 110 gaps, **545.1 s of the race unrecorded** | complete, 0 gaps |

The two tracks disagree by up to 1 084 m at shared timestamps: 0 of 17 257
shared samples agree within 5 cm.

**What #9 is:** a second car that follows the route far behind (waypoint 7 at
2011.0 s where the player is at 928.9 s), never reaches the spawn, CP10, CP11
or the goal, and passes within 0.35 m of the player at t = 602.930. Almost
certainly **another player on the server**, recorded into the same
`EntRecordData`. Its one qualification for being picked is that it is a single
long entity, while the player's car is destroyed and recreated 45 times.

This is a **decoder defect, not a map defect, and it is not specific to this
map** — see `../ACQUISITION_addendum_ghost_entity_selection_v1.md`.

### What the correction changes, and what it does not

Reassuringly little of §3 of `RESULT.md` moves, because respawn timing was read
off the packet stream, which is real:

| | previous (wrong car) | corrected (player) |
|---|---|---|
| respawns | 111 presses (`word0 = 34`) | **110 teleports** in the player track |
| residue window starts | 112.630 / 265.820 / 397.480 / 551.720 / 720.290 / 2840.200 / 4719.110 / 5077.360 / 5577.220 | 112.630 / 265.820 / 397.480 / 551.730 / 720.330 / 2840.230 / 4719.130 / 5077.380 / 5577.220 |
| retry-deletion floor | 1 214.585 s | **1 214.465 s** |
| residue path | 26 023 m | **24 546 m** |
| residue average speed | 77.1 km/h | **72.8 km/h** |
| **path / displacement** | **17× at CP3, 20× at CP12** | **3.4× at CP3, 3.7× at CP12**; range 1.6–13.9× |

Two independent respawn detectors agreeing to 10 ms on nine of nine window
starts is a good control on both. But **the specific measurement I was sent to
chase — the 17× and 20× ratios — is an artefact of the wrong car and does not
exist.** The reopening was right for a reason that turned out to be wrong, and
the map still reopens, for a better one.

Respawn structure, corrected: the 110 teleports land at **9 distinct sites**
(3 m clustering), each 1.7–53.8 m from the corresponding checkpoint crossing —
i.e. every one is a checkpoint respawn, and the two hard segments carry 39
(CP9) and 38 (CP10) of them.

---

## 2. The instrument: forward splicing

Nothing about map geometry. Take the human's residue — the surviving attempt at
each checkpoint, retries already deleted — resample it to 0.10 m, and allow
exactly two moves:

* **advance** one sample, costing the human's own elapsed time; and
* **splice forward** from sample *u* to a strictly later sample *v* when
  * |pos(u) − pos(v)| ≤ **r**
  * heading(u)·heading(v) within **align**
  * |speed(u) − speed(v)| ≤ **dv**
  * attitude quaternions within **dq**
  * both speeds ≥ 5 km/h (never splice across a standstill)

Then take the minimum-time path from each checkpoint crossing to the next. The
result is a **sub-sequence of the human's own samples in their own order**: at
each splice the car is in the same place, pointing the same way, going the same
speed, in the same attitude — so what was cut out is a closed loop the car
itself proved was a detour.

### Result

```
  seg   driven_s  driven_m  spliced_s  spliced_m   t_saved  l_saved
    1     11.600     407.9     11.473      407.9        1%       0%
    2     58.150    1680.5     57.532     1680.5        1%       0%
    3    111.900    1452.5    103.230     1452.5        8%       0%
    4     57.160    1987.8     44.832     1603.1       22%      19%
    5    106.750    2171.6    103.433     2171.5        3%       0%
    6     54.150    1201.3     53.423     1201.3        1%       0%
    7     95.300    2063.8     79.833     2006.8       16%       3%
    8    113.950    1294.6    103.500     1292.9        9%       0%
    9    235.250    3249.3    116.465     2171.9       50%      33%
   10    132.250    3204.8     78.160     1939.0       41%      39%
   11    154.050    3025.5     57.215     1608.1       63%      47%
   12     84.160    2806.3     83.050     2806.3        1%       0%

DRIVEN RESIDUE   1214.670 s   24546 m
SPLICED ROUTE     892.148 s   20342 m
AUTHOR TIME       939.283 s   ->  47.135 s UNDER
```
(r = 0.25 m, align 20°, dv 5 km/h, dq 15°, resample 0.10 m.)

The biggest single cuts, each an unambiguous closed loop:

```
   t_enter    t_leave   skip_s     loop_m   jump_m   dv_kmh  attitude
  2907.920   3010.780  102.860     1066.7    0.164      0.4    6.9d
  5093.441   5169.573   76.132     1123.8    0.172      0.4    4.4d
  4725.505   4762.959   37.454      905.5    0.211      0.5   12.4d
  5181.648   5200.007   18.359      292.6    0.218      2.8    7.6d
  4763.768   4778.158   14.390      360.8    0.167      4.8    1.8d
   296.650    308.742   12.091      384.8    0.185      3.8   14.0d
```

At t = 2907.920 the car is somewhere; 102.860 s and 1 066.7 m later it is back
within **16 cm** of that point, within 0.4 km/h and 6.9°. That is not a
measurement subtlety. That is a lap of a platform.

### Sensitivity — reported in full, because it matters

```
     r  align    dv    dq   step |  spliced_s    len_m      vs AT
  2.00     60    30    90   0.25 |    745.245    18763  -194.038
  1.00     45    20    60   0.25 |    778.157    19097  -161.126
  0.50     45    20    60   0.10 |    790.923    19178  -148.360
  0.50     30    10    30   0.10 |    836.196    20072  -103.087
  0.25     20     5    15   0.10 |    892.148    20342   -47.135
  0.15     15     3    10   0.05 |    926.047    21392   -13.236
  0.10     10     2     8   0.05 |    942.967    21753    +3.684
  0.05      8     1     5   0.02 |    965.543    22072   +26.260
```

The answer is tolerance-dependent and I am not going to pretend otherwise: at
5 cm the de-looped line is 26.260 s **over** the author time, at 25 cm it is
47.135 s under it, at a metre it is 161.126 s under it. **The honest statement
is that the author time sits inside the band the instrument produces, at the
tolerance where a car-sized object is in the same place.** What it is not, at
any tolerance, is 275 s out of reach.

*(Resampling to 0.05–0.25 m is not cosmetic. At 100 km/h a 50 ms sample step is
1.39 m, so two passes through literally the same point are sampled up to 0.7 m
apart: a point-to-point radius below that under-detects loops by construction.
An earlier run without resampling reported 1 073.093 s at r = 0.10 m — a
statement about the sample grid, not the driving.)*

---

## 3. Controls

**The instrument can say no, from inside the same measurement.** Segments 1, 2,
5, 6 and 12 — 314.810 s and 8 268 m of the residue — come back at 1–3 % time
and **0 % length**. Five of twelve segments are already clean lines.

**External, on other maps' ghosts** (same code, same parameters, checkpoint
order respected):

```
ghost                            cps  driven_s  deloop_s  t_saved  l_saved
rank01_24342 (126859 WR)           1    24.400    24.304     0.4%     0.0%
rank02_24634 (126859)              1    24.650    24.554     0.4%     0.0%
rank00001_40223 (146612 WR)        6    40.200    40.103     0.2%     0.0%
ATREC_228811 (author lap)          7    32.900    32.803     0.3%     0.0%
ATREC_203330 (author lap)          1    16.950    16.769     1.1%     0.0%
rank22_41997 (126859, a crash)     1    42.050    38.655     8.1%     0.0%
AUTHOR_238835_462982               5   462.950   209.361    54.8%    54.8%
rank00001_1964933 (238835 human)   5  1964.900   199.580    89.8%    90.1%
```

Clean racing lines: **0.0 % length, 0.2–0.4 % time.** The instrument does not
manufacture savings.

The 238835 rows are the **positive** control and they are the strongest thing
in this document. 238835's author lap de-loops by **54.8 %** — and that map's
author time was independently **beaten by 48.3 %** by a completely different
method (respawn-bracketed retry deletion, validated on the oracle,
`238835/RESULT.md`). Two unrelated instruments agree within six points on a map
where the answer is known. That is the control 153527 itself cannot provide.

**A control that failed first, and how.** My first version of the external
control de-looped start→finish without requiring the checkpoints, and reported
the same 54.8 % — for the wrong reason: a start→finish shortcut can skip whole
sections of a map. The number only became meaningful once the control was made
to respect checkpoint order exactly as the real measurement does. A control
that is not the same instrument is not a control.

---

## 4. The map geometry, which turns out to be the weakest evidence here

I built the block graph anyway. It is uninformative and I am reporting it so
nobody spends the day on it.

`tmmaps list` returns all 13 waypoints with real grid cells — **the free-block
hazard does not bite on this map** (its free blocks, flag `0x20000000`, are
gates and springs, not the checkpoints). Cell→world calibrates cleanly at
`world_y = 8*(cell_y − 8)`: the car's first sample is at y = 82.0 on a spawn
block anchored at 80, its last at y = 234.2 on a goal block anchored at 232,
and items with both a cell and an absolute position agree.

A deliberately permissive cell graph — all 44 190 occupied cells traversable,
edges to any cell within |dx|,|dz| ≤ 1 and |dy| ≤ 1, cost = centre-to-centre —
gives a **lower bound of 4 681 m** for the whole route, against a straight-line
checkpoint chain of 4 370 m. The map is 44 388 blocks dense; at 32 m cell
resolution it is effectively free space, so the bound only says "no legal route
is shorter than about 4.7 km". The real constraint lives in the fine geometry of
`Platform*` blocks, and the extracted block library (`tm2020-blockgeom.md`)
covers `Road*` only — **no `Platform*` block has a loft at all**, and this map
is built almost entirely from them.

So: the geometry does **not** force the long way (that closure is not
available), and it also cannot tell you how short a route could be. The
informative bound is the empirical one in §2.

---

## 5. What this makes the map

**A route-construction problem with a known target and a reference line.** The
spliced route is not a hypothesis about the map — it is 20 342 m of positions,
headings, speeds and attitudes that the human's car actually held, in order,
with a time of 892.148 s attached. That is exactly the input
`tm-loop/DRIVING.md`'s controller wants, and the 6 large splices are natural
segment boundaries.

Where the slack is: segments **9, 10 and 11** carry 269.710 s of the 322.522 s
saved. Segments 1, 2, 5, 6 and 12 have nothing in them.

---

## 6. The honest complication, unchanged and load-bearing

**This map cannot be seeded, and nothing here is oracle-validated.**

* The sole human record is a dead-build ghost (`2024-01-10 git=126731`,
  `NbRespawns = 4294967295`) and returns `wrong simu`. `RESULT.md` §2 is right.
* There is no embedded author ghost. `RESULT.md` §1 is right, and I did not
  re-open it.
* TMX has zero replays.

So **there is no known-answer control from this map's own field**, and I could
not have validated a tape even if I had built one. Every number above is
geometry read out of a ghost that does not re-simulate. The 238835 cross-check
in §3 is the closest thing to a substitute and it is on a different map.

Re-simulating my own output from a fresh process would not be a control either
(§0.4), and I did not do it.

**"Here is the shorter route and here is why we cannot yet drive it" is the
result.** The remaining blocker is a seed, exactly as `RESULT.md` said — but the
thing a seed would now be used for has changed completely, from "find 275 s of
pace that probably is not there" to "drive a line we can already write down".

---

## 7. A by-product for another map

The same instrument, run on **284238**'s human record as a control: 440.200 s
driven → **79.188 s** spliced, 77.8 % of its length redundant. 284238's best
validated run is 97.461 s against an author time of 50.459 s. The human record
on that map appears to contain a line ~18 % faster than the best TAS run anyone
has validated there. That map is "characterised but not beaten" — this is worth
someone's afternoon.

---

## Files (all new, all `route_`-prefixed)

```
route_PLAN_v1.md                     written before anything was computed
route_evidence/
  entity_audit.txt                   49 vehicle entities, coverage and gaps
  perlife_tiling.txt                 the 46 per-life tracks, 0 overlaps 0 gaps
  which_car_is_the_player.txt        2/12 vs 12/12 against the waypoint blocks
  the_other_car.txt                  what entity #9 visits, and when
  respawns_and_residue.txt           110 teleports; corrected residue table
  respawn_landing_sites.txt          9 landing sites, all at checkpoints
  splice_detail.txt                  the headline run, 25 largest loops cut
  splice_sensitivity.txt             the full tolerance sweep
  controls_other_maps.txt            clean lines 0.0 %; 238835 54.8 %/89.8 %
  cellgraph_lower_bound.txt          4 681 m, and why it is useless
route_tools/                         Rust, std only (+ tmtraj for the decode)
```

Reproduce: `cargo build --release -p route` inside a `tmtas-hard` workspace with
`route/` added to `members`, then
`route dense GHOST -r 0.25 --align 20 --dv 5 --dq 15 --step 0.10`.
