# 228607 "Fall 2024 - 08 Torment (1-UP)" — the answer key is 228811, and it is the SAME MAP with the finish 64 m higher

Write-once sidecar, `key_` prefix (answer-key agent, the sibling-map sweep).
Supersedes nothing. Times in seconds. **Nothing was submitted to any Nadeo or
official leaderboard**; every network call was a read against TMX (~1 req/1.5 s)
and trackmania.io (~1 req/1.6 s) with a descriptive User-Agent.

---

## 0. Verdict

| question | answer |
|---|---|
| siblings found | **228811 "Fall 2024 - 08 Torment (1-DOWN)"**, plus 234048 and 290085 further out |
| block-identity fraction | **12 485 of 228607's 12 492 records byte-identical** (99.94 %) |
| a human who beats that sibling's AT? | **no** — 228811's field tops out at 22.637 vs its AT 20.555 |
| but a validated tape that beats it? | **yes, ours** — the fleet's `v2_best_20237` (228811 arm, AT 20.555 beaten by 0.318) |
| does it re-simulate on 228607? | **no. Measured DNF**, and I can say exactly why (§3) |
| embedded author ghost | **record-data only, no input archive** — watch-only, cannot be re-simulated |

**The one sentence worth carrying: 228607 and 228811 are the same map with the
Goal moved eight cells (64 m) up, so every metre of 228811's launcher analysis
is already an analysis of 228607 — and the reason our 228811 tape does not
simply win here is that it crosses the 1-UP gate's altitude ~150 m short in x.**

## 1. The identity, exhaustively

Author Bernkastel_. (TMX user 160972) has **22 maps**; all 22 were downloaded and
parsed. Per map I emit one canonical line per record — `P|name|cx|cy|cz` for a
placed block, `F|name|x|y|z|rx|ry|rz` for a free block, `I|model|x|y|z` for an
item — sort, and intersect the multisets with 228607's 12 492 records
(33 free + 10 876 placed + 1 583 items). Full table:
`key_siblings/key_identity_table_v1.tsv`.

| TMX | name | identical | name-matched | AT | human WR |
|---|---|---|---|---|---|
| **228811** | **Torment (1-DOWN)** | **12 485** | 12 489 | 20.555 | 22.637 KappaRiley |
| 234048 | goneHiking | 4 398 | 10 142 | — | — |
| 290085 | A08 | 2 088 | 10 020 | 18.928 | **18.774** THCwasTaken |
| 227657 | Snow08 | 972 | 2 239 | — | — |
| 272439 | Summer 2023 - 08 Torment | 667 | 1 088 | 11.838 | **11.815** lasyoppwtf |
| 303966 | A08 #2 | 415 | 2 163 | 13.910 | **13.508** Toppish146 |
| 228503 | Winter 2025 - 06 - Torment | 0 | 0 | 22.703 | **22.227** SSanoTM |
| 282970 | Impossible A08 | 0 | 0 | 14.275 | **12.110** Aluji. |

(228607 itself: AT **20.258**, human WR **24.902** Falco_TM_, 23 records.)

**The complete difference between 228607 and 228811 — 7 records out of 12 492:**

```
only on 228607 (1-UP)                  only on 228811 (1-DOWN)
P|GateFinish|11|27|21                  P|GateFinish|11|19|21
P|GateFinish|11|27|24                  P|GateFinish|11|19|24
P|GateFinish|12|27|22                  P|GateFinish|12|19|22
P|GateFinish|12|27|23                  P|GateFinish|12|19|23
I|...Red\Collection_68/77/80           I|...Green\Collection_125/134/137   (the "UP"/"DOWN" lettering)
                                       I|SupportTubeStraightX1 ×4 (368/400,150,688..784)
```

`tmmaps list` agrees at the waypoint level: **identical Spawn** (`RoadTechStart`
… in fact `PlatformTechStart` cell (46,37,22)) and an **identical checkpoint
set at identical world coordinates** — (1102,93,720), (1102,93,752),
(736,82,720/752), (80,50,720/752), (432,18,720/752), (959,57,720/752),
(1342,218,748). Only the four Goal blocks move, from cell y=19 to y=27:
**+64 m of altitude at the same x,z footprint** (x cells 11–12 ≈ 352–416 m,
z cells 21–24 ≈ 672–800 m).

That also explains the ATs: 1-UP 20.258 is *faster* than 1-DOWN 20.555 because
the run ends in a long descent and the higher gate is crossed **earlier** on the
way down.

## 2. Controls (an instrument that can only say yes is not an instrument)

* the identity instrument scores 228607 against itself **12 492 / 12 492**, and
  returns **0** for 3 of this author's own maps and for 170 maps of an unrelated
  author (285885's sweep);
* the oracle, on my own server copy and my own staging root:
  **228607's own `m228607_splice_24854` returns 24854 exactly** on my untouched
  copy of the map, in the same batch as every foreign tape below;
* my copy of 228607 came from TMX and is byte-size-identical to the copy already
  in `tm-unbeaten/228607/map.Map.Gbx` (1 247 289 bytes).

## 3. The transfer test, and the measured reason it fails

Batch on the **untouched** 228607 map (plain oracle, fresh server process):

| tape (from 228811) | its time on 228811 | on 228607 |
|---|---|---|
| `v2_best_20237` (fleet TAS, AT-beating) | 20.237 | **DNF** |
| `v2_best_20250` | 20.250 | DNF |
| `v3_best_20263` | 20.263 | DNF |
| `v1_best_20273` | 20.273 | DNF |
| `u3_best_FIRE_fin20281` | 20.281 | DNF |
| `CONTROL_humanWR_22637` (human) | 22.637 | DNF |
| **`m228607_splice_24854` (228607's own)** | — | **24854 — control passes** |

Why: I decoded the launcher tape's telemetry and read off where it is when it
passes the 1-UP gate's altitude (world y ≈ 154, i.e. cell y = 27; the cell↔world
fit from this map's own waypoints is `y_world ≈ 8·cy − 62`, `x_world ≈ 32·cx`,
`z_world ≈ 32·cz`):

```
t = 21.220  x = 188.2  y = 153.3  z = 698.8   428 km/h
t = 21.420  x = 212.1  y = 150.2  z = 698.4   441 km/h      <- crossing y≈152
...
t = 22.420  x = 338.6  y = 111.1  z = 687.4   524 km/h
t = 22.670  x = 372.3  y =  96.3  z = 682.2   542 km/h      <- 1-DOWN gate (y≈93)
```

The line crosses the 1-UP gate's altitude at **x ≈ 200**, while the gate footprint
is **x ≈ 352–416** — about **150 m short**, and it is still nearly flat there
(0.7 m of drop per 50 ms against 6 m of x). Over the next 160 m of x it loses
54 m of altitude and arrives exactly at the 1-DOWN gate.

> **So 1-UP is not a different route. It is the same flight carried ~60 m
> higher through its last 150 m of x** — which is also why its author time is
> 0.297 s quicker.

## 4. What I hand the 228607 arm

1. **Everything in `228811/RESULT-AT-BEATEN.md`, `TECHNIQUE.md` and the launcher
   claims is about your map.** Same spawn, same seven checkpoints, same
   geometry to 99.94 %. Do not re-derive the launcher.
2. **A concrete objective that is not a finish time**: the flight must arrive at
   x ≈ 352–416, z ≈ 672–800 at **y ≈ 154** instead of y ≈ 93. Everything before
   the last 150 m of x can be taken from `v2_best_20237` unchanged; that is a
   1-parameter re-aim, not a re-search.
3. **The 20237 tape as the seed** (`228811/claims-launcher/v2_best_20237.Ghost.Gbx`),
   with the honest caveat that it DNFs unmodified here.
4. **A caution the fleet should carry**: my parser-free byte screen scored this
   pair at **0.13 containment** while the structured record comparison said
   **0.9994**. Byte-window containment *understates* identity badly when the two
   maps serialise their lookback tables differently. Screen with bytes if you
   must; **decide on records**.

## 5. Embedded author ghost — present, watch-only

`ct probe` finds a `CPlugEntRecordData` node at body offset 607 759 and **no**
`CGameCtnGhost` blob; `ct mapghost` confirms `NO EMBEDDED GHOST`. So 228607
carries the author's recorded trajectory with **no input archive** — it can be
made watchable (`ct recghost`) but can never be re-simulated. This reproduces
`ACQUISITION_addendum_embedded_author_ghost.md` exactly, and 228607 is that
addendum's own positive control (406 samples, 50 ms, 0 → 20.290), so the scanner
used in this sweep is verified against a known answer.

## 6. Artefacts (all under `228607/key_siblings/`)

* `key_228811_torment_1down.Map.Gbx` — the sibling map as downloaded from TMX
* `key_identity_table_v1.tsv` — every one of the 22 maps with its identity count
* `key_author_160972_corpus.tsv` — the catalogue (MapId, name, uid, uploaded)

Re-fetch any sibling deterministically:
`https://trackmania.exchange/maps/download/<MapId>` at ~1 req/1.5 s with a
descriptive User-Agent. The tapes cited in §3 are the 228811 arm's own files in
`tm-unbeaten/228811/claims-launcher/` and `claims/`; I copied nothing of theirs
into my own directory and edited nothing they own.
