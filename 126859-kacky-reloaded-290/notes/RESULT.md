# 126859 — "Kacky Reloaded #290" — the author time is beaten by 0.646 s

**AT 24.062 s · human online WR 24.342 s (`zetos.`) · 22 records · our validated
best 23.416 s.**

* **−0.646 s against the author time** — the largest margin this project has
  taken from an unbeaten AT.
* **−0.926 s against the best human**, and the gain splits cleanly in two:
  **0.480 s of it is a human's own driving**, copied unchanged from the rank-13
  run on the same leaderboard; **0.446 s is ours**, and it is one idea.
* `NbRespawns: 0`, `IsValid: true`, one checkpoint, on the untouched map file —
  byte-identical to Nadeo's own copy, re-downloaded cold mid-run and re-checked.

## The family

| tape | validated | vs AT | vs human WR | steer alphabet | change events | device |
|---|---|---|---|---|---|---|
| **`TAS_23416_v3_CHAMPION`** | **23.416** | **−0.646** | −0.926 | 241 values | 907 | TAS |
| `TAS_23418_v2` | 23.418 | −0.644 | −0.924 | 240 values | 899 | TAS |
| `TAS_23462_v1` | 23.462 | −0.600 | −0.880 | 239 values | 881 | TAS |
| `TAS_23508_thin318_v1` | 23.508 | −0.554 | −0.834 | 239 values | **318** | TAS |
| `TAS_23545_alphabet153_v1` | 23.545 | −0.517 | −0.797 | **153 values** | 315 | TAS |
| `TAS_24164_keyboard_v1` | 24.164 | **+0.102** | **−0.178** | **3 (`−127/0/+127`)** | **130** | **keyboard** |
| human WR `zetos.` | 24.342 | +0.280 | — | 3 (keyboard) | 110 | keyboard |
| our seed, `rank13` `TheWoreL` | 27.609 | +3.547 | +3.267 | 97 values | 189 | pad |

*(The thinned and alphabet-reduced tapes were derived from the 23.462 champion
before the last two search rounds found 23.418 and 23.416; the simplification is
of that lineage and was not redone. Nothing in the technique changed — the last
46 ms are more of the same endgame grinding.)*

Every row re-validated through the plain oracle against the untouched map, with
known-answer controls in the batch.

**The honest headline about human execution: no keyboard tape beats the author
time.** The best pure-keyboard run we can produce, 24.164, is 0.178 s faster than
the best keyboard human but still 0.102 s short of the AT. On four earlier maps
in this project a keyboard-constrained search beat the AT outright. Not here —
and §2 and §6 say why that is the expected answer on this particular map.

Full validation transcript: `VALIDATION.md`. Tapes: `tapes/`. Evidence:
`evidence/`. Plan as written before the search: `PLAN_v1.md`.

---

## 1. What this map is — established, not assumed

The brief's first instruction was to establish the map type before spending
anything, because a Kacky map is normally respawn content. **This one is not.**

### No checkpoints

`tmmaps list` finds exactly two waypoints among 1765 blocks and 8110 items:
`block#1174 RoadTechStart` (Spawn) and `block#1618 GateFinish` (Goal). Every one
of the 22 ghosts declares a single split equal to its own finish time. The
dedicated server confirms it from the other side: `"NbCheckpoints": 1` — the
finish itself — in the `ValidatedResult` of every run.

Two consequences that shaped everything after:

* **No shaping signal.** A DNF returns `"wrong simu"`, not a checkpoint count, so
  the search climbs only on runs that finish. Measured DNF rates: 69 % on a
  whole-tape mutation window, 7–27 % on an endgame-only window. That single fact
  is why every productive arm in this run was endgame-restricted.
* **Both defects reported mid-run against the hardened build are out of reach
  here.** The DNF-shaping score overtakes a finish at 11 checkpoints and the
  phantom guard misfires at 6; this map's maximum is 1. Checked, not assumed.

### No respawns — and the reason is structural

With no checkpoints, a respawn returns the car **to the start**. It is not a
strategy on this map, it is a restart.

Measured three independent ways rather than inferred:

1. `k290 jumps` over all 22 decoded trajectories: the largest position step
   between consecutive 50 ms samples anywhere in the field is **11.21 m**, which
   is exactly the map's top speed (805 km/h = 11.2 m per 50 ms). **Zero
   teleports; zero returns to within 12 m of the spawn point after t = 3 s.**
2. The dedicated server prints `NbRespawns` as a first-class field in **both**
   the `DeclaredResult` and the `ValidatedResult`. It reads **0** for the human
   WR and **0** for our tape (`evidence/rawvalidator_champion_v1.txt`).
3. The one exception proves the field is being read and not defaulted: the
   last-place run `rank22` (41.997, `Asvyl`) declares
   **`NbRespawns: 4294967295`** — `−1` as a `u32`.

So `NbRespawns: 0` here is **forced by the map's structure**, not a property of
the tapes we happen to have fed the validator. A respawn is expressible and
validatable in general; on this map it would simply be slower than anything on
the board.

### The whole-field re-simulation check — 21/22, and the miss is diagnosed

All 22 records were pulled — the entire leaderboard, not a sample — and
re-simulated:

| result | count |
|---|---|
| exact to the recorded millisecond | **21** (ranks 1–21, 24.342 … 32.189) |
| unvalidable | 1 — `rank22`, 41.997, last place |

`rank22` fails with `"wrong simu … had simulation hazards '0-1-0'"` alongside the
corrupt `NbRespawns` above, and its `GameBuild` is from **2024-01-10** — older
than every other ghost on the board. A broken 2.5-year-old ghost, not a physics
disagreement: ranks 1–21, including the world record and every run we seeded
from, are exact. Nothing like the 203072 failure mode. **Proceed.**

## 2. Is 24.062 a driven lap? Honest answer: probably not — and that changes what has to carry the argument

`unbeaten.at` flags this map `atSetByPlugin: true` and `inPlugin: true`. The
medal set (gold 26.000, silver 29.000, bronze 37.000) is round seconds —
template values — while the AT is not round, which on earlier maps in this
project argued *for* a driven validation lap. Here two further pieces of
evidence point the other way:

* the map header declares `validated="1"`, **but the map body contains no
  validation ghost** — and this negative is backed by a positive control, not
  asserted. Scanning the LZO-decompressed body for the `CGameCtnGhost`
  (`0x03092000`) and `CPlugEntRecordData` (`0x0911F000`) class ids:

  | map | `validated=` | `0x0911F000` (telemetry stream) | verdict |
  |---|---|---|---|
  | **126859 (this map)** | `"1"` | **0 hits** | **no embedded ghost** |
  | 228607 `Fall 2024 - 08 Torment`, known to embed one | `"1"` | **1 hit** | ghost present |

  In the control the id appears twice adjacently at body offset 607760, followed
  by chunk `0x0911F00A` and a `78 9c` **zlib header** — the compressed telemetry
  stream itself — with `0x0329F000` immediately before it. The identical scan of
  126859 returns nothing of the kind. (Both files also produce exactly one
  `0x03092000` "hit", and in both it lands inside the same repetitive
  item-index table — a shared false positive, which is itself a useful check
  that the two scans are seeing the same file structure.) **So `validated="1"`
  is true here and carries no ghost: the third outcome the fleet warned about.**
* Kacky campaign maps are published in bulk by an event organisation, and their
  ATs are set through Openplanet tooling as a matter of course.

**So I am not claiming "a human already drove 24.062".** That argument was
available on the maps this project beat earlier; it is not available here, and I
have not leaned on it anywhere below. Two things carry the reproducibility case
instead:

1. **0.480 s of our 0.926 s is literally a human's own inputs, unmodified** —
   not a technique we are asking anyone to learn, just a run already on the
   board.
2. **the measured input structure of the remaining 0.446 s** (§6), which is
   where the honest bad news is.

It also reframes the keyboard result. If the AT were a driven keyboard lap, a
keyboard tape 0.102 s short of it would be a failure of our search. Given the AT
is very likely plugin-set and the *best keyboard human is 0.280 s slower still*,
24.164 is more plausibly near the keyboard ceiling of this map.

## 3. The route

Decoded from the WR's own telemetry: 2709 m of track in 24.3 s, **34–46 % of
every run airborne**, top speed 776–805 km/h for the entire field.

| race t | what happens | km/h | height y |
|---|---|---|---|
| 0 – 3.9 s | standing start, ramp, first bend | 0 → 190 | 46 → 38 |
| 3.9 – 6.2 s | booster chain #1 | 190 → **800** | 38 → 60 |
| 6.2 – 8.6 s | **launch #1**, long ballistic arc | 800 → 650 | 60 → 167 |
| 8.6 – 12.5 s | descend, land, run the mid-section | 650 → 330 | 167 → 93 |
| 12.5 – 16.0 s | climb, then **an upside-down run at y ≈ 160** (roll = π) | 330 → 470 | 93 → 160 |
| 16.0 – 17.0 s | over the edge and **a 94 m drop** | 445 → 465 | 160 → 66 |
| 17.0 – 19.1 s | flat run, booster chain #2 | 465 → **765** | 66 |
| 19.1 – 21.1 s | **launch #2** — the big one, airborne throughout | 765 → 675 | 66 → 143/170 |
| **21.1 s** | **the car strikes a tower wall at ~675 km/h and is thrown back** | → 255 | — |
| then | **free fall down the face**, never touching ground | 255 → 300 | → 75 |
| finish | gate on a platform at **(1522, 74, 1340)**, entered moving −x | — | 74 |

## 4. Where the 0.280 s of headroom actually was

### The field, sector by sector

24 arclength stations along the WR's line, all 22 runs timed at each:

| sector | race window | field spread | corr. with final time |
|---|---|---|---|
| 1–8 | 0 → 8.6 s | 0 – 0.150 s | ≈ 0 |
| 9–13 | 8.6 → 15.0 s | 0.484 – 1.652 s | 0.11 … 0.70 |
| 14–22 | 15.0 → 21.4 s | 0.050 – 0.150 s | 0.19 … 0.69 |
| **23–24** | **21.4 s → finish** | **0.600 / 14.155 s** | 0.29 / **0.97** |

**The last sector alone correlates 0.97 with the final result and carries the
whole spread of the field.** The WR needs 1.476 s from station 23 to the line;
the median needs 3–4 s; last place needs 15.6 s. Everything before 21.4 s is
essentially forced — the entire board is within 0.150 s of each other through
both boosters and both launches.

That is the opposite of what 227969 and 270051 found (there the spectacular
closing feature cost everybody the same and sorted nobody). **Here the closing
feature *is* the map.**

### The one measurement that decided the whole attack

Timing every run's **tower impact** — an unambiguous physical event, the largest
single-sample deceleration after 19 s — rather than trusting the arclength
projection through a long air phase:

| run | reaches the tower | at height y | finishes |
|---|---|---|---|
| `rank15` | **21.050** | 166.2 | 27.969 |
| **`rank13` `TheWoreL`** | **21.100** | 170.5 | 27.609 |
| `rank02` | 21.200 | 156.3 | 24.634 |
| `rank10` | 21.400 | 158.5 | 27.279 |
| **`rank01` (WR)** | 21.550 | **152.7** | **24.342** |
| the other 17 | 21.710 … 24.900 | 146 – 173 | 27.449 … 41.997 |

**`TheWoreL`, 3.3 s off the world record in 13th place, arrives at the final
obstacle 0.450 s BEFORE the world record does — and throws all of it away.**
`rank15` arrives 0.500 s early and throws away more. Meanwhile the world record
is the only run in the field that converts the fall into a 1.476 s flying entry
to the gate; everyone else lands short or long and crawls in at 15–90 km/h.

**Nobody on this leaderboard does both.** That gap is where the author time —
and 0.646 s more — was sitting.

## 5. What we did, and what the 0.926 s is made of

Seeded the search from `rank13` instead of the world record, froze its approach,
and searched only from race 17.45 s (tape tick 1900) onward.

Twenty seconds of search from that seed reached 24.428. Five minutes reached
23.639 — already under the author time. Everything after was grinding: about
1.1 M evaluations across nine arms converged on 23.462 and looked finished —
until a different operator regime (**4–6 operators per candidate with simulated
annealing at temperature 0.35–0.6**, rather than one operator per candidate)
broke the plateau immediately and took it to **23.416** over three more rounds
and 2.4 M evaluations. Two independent arms then stopped on 23.416 together.
A mid-section arm with 250 000 evaluations found *zero* improvements throughout.

Per-sector against the human world record, from memory-read trajectories of both
(`fk btraj`, verified in §7b):

| sector | geometry | Δ vs WR |
|---|---|---|
| 1 (start → 10.2 s) | to (1661, 154, 425) | −0.010 |
| 2–5 | landing and the mid-section run | −0.150 |
| **6** | **the upside-down section, x 1490 → 1409 at y = 160** | **−0.120** |
| 7–8 | end of the ceiling, over the edge | −0.060 |
| 9–12 | the 94 m drop, flat run, booster #2 | −0.120 |
| 13–17 | launch #2 and the flight | −0.030 |
| **18** | **the tower strike** | **−0.110** |
| **19–20** | **the fall and the gate** | **−0.326** |
| | | **−0.926 s** |

Split by authorship, at the last station before the tower:

* **−0.480 s: sectors 1–17, and every input in them is `TheWoreL`'s own,
  byte-for-byte unmodified.** Our search never touched a tick below 1900; the
  seed reaches that station 0.480 s ahead of the world record on its own. The
  largest single piece, −0.120 s, is in the upside-down section at y = 160.
* **−0.446 s: the tower and the fall, ours.** One idea, below.

## 6. The technique — verdict: UNDISCOVERED, and it is one thing

**Fly the second launch flatter, and hit the finish tower ten metres lower.**

At the tower:

| | reaches the tower | height y | speed | fall to the gate |
|---|---|---|---|---|
| human WR | 21.560 | **152.8** | 669 → 252 km/h | **2.792 s** |
| our seed `rank13` | 21.150 | **172.8** | 674 → 180 km/h | 6.459 s |
| **our champion** | **21.060** | **141.8** | 674 → 266 km/h | **2.356 s** |

The car strikes the tower essentially at the apex of its second flight, is thrown
back along −x at ~265 km/h, and then falls 70–100 m to a gate on a platform at
y = 74. **The fall is ballistic, and its duration is set almost entirely by the
height you hit at.** Eleven metres lower is 0.436 s.

How the height gets set, tick by tick through flight #2:

| btraj t | WR (y) | seed `rank13` (y) | **ours (y)** |
|---|---|---|---|
| 19.20 | 71.7 | 103.4 | **96.8** |
| 20.00 | 115.7 | 143.5 | **127.9** |
| 20.60 | 137.5 | 162.4 | **140.2** |
| 21.00 | 146.8 | 169.9 | **143.3** |

Same speed as the seed to within 1 %, **27 metres less apex**, and *further*
along the track in z at every instant. The difference is the steering held
during the launch: the car leaves the booster rolled onto its side (roll ≈ −1.8
rad; roll reaches π on every run in the field), so **steering into the roll
pitches the nose down**. Our tape holds a hard left through the launch (race
18.41–18.67 s and 19.00–19.22 s at full lock, then repeatedly to −127 at
20.60–20.80 s) where both the seed and the world record are near zero. The field
lets the car fly; we steer it flat.

The world record does part of this by accident — it is 20 m flatter than
`rank13` — which is exactly why it is the world record despite reaching the
tower 0.450 s later.

**In one sentence a driver would recognise:** *everybody flies the last jump and
waits; hold full left all the way through it, arrive at the tower a car's height
lower, and the drop into the finish is a third of a second shorter.*

### Is the line legitimate?

Yes, with one honest exception. Same route, same boosters, same launch, same
tower, same gate, entered from the same direction. The impact point sits inside
the field's own range in x (1686.7 against 1688–1706) and z (1356–1369 against
1331–1369). **The one axis where our tape leaves the field is the impact height:
141.8 m, against a field minimum of 146.4 m (`rank11`) and a median of ~161 m —
4.6 m below the lowest human and 11.0 m below the world record.** That is the
discovery, and it is a difference of degree along an axis the field already
varies by 27 m, not a new mechanism. No geometry is reached that no human
reaches, nothing is skipped, and there is no respawn anywhere.

### How hard is it? The measured bad news

This is where the map is unlike the four earlier ones.

* **The launch cannot be flown on a keyboard.** Searched under the constraint,
  never projected (projection DNFs: `u10cand project` on `rank13`, `rank15` and
  on our champion all fail at `{−127,0,+127}`). Keyboard-constrained arms seeded
  from `rank13`, quantising only from tick T onward:

  | keyboard from race | finish rate | best |
  |---|---|---|
  | 17.48 s | **0 %** | — (never finishes) |
  | 18.08 s | **0 %** | — (never finishes) |
  | 18.68 s | 33 % | 24.312 |
  | 19.08 s | 39 % | 24.278 |
  | 19.48 s | 91 % | 24.285 |

  **The boundary is between 18.08 s and 18.68 s — the moment the car leaves the
  ground on launch #2.** Everything after takeoff is keyboard-drivable;
  the run-up through booster chain #2 is not, on this line.
* **The analog ramps are load-bearing.** The simplifier's ramp-collapse pass
  tried to replace each of 25 multi-tick analog sweeps with a single instant
  step at every placement inside it — 13 to 52 placements per ramp, several
  hundred in total — and **not one produced a finishing run**. Our champion is
  genuinely an analog tape, not a keyboard tape wearing analog clothes.
* **Thinning is cheap; alphabet reduction is not.** Greedy event deletion took
  the champion from 881 change events to **318 for 0.046 s** (23.462 → 23.508),
  which is a real simplification and the tape a TAS-curious human would study.
  Reducing the *alphabet* is what fails: the quantize-by-walking pass, given a
  0.588 s budget and 45 000 oracle evaluations, converted **17 of 324 held runs**
  onto `{−127,0,+127}` and left 424 off-alphabet ticks. Five per cent.

### Tolerance — and the control that stops it being read the wrong way

Recoverable tolerance was measured for every change event on the simplified
champion (mistime one input, re-time only the later ones, re-measure against the
real oracle, ±10 ticks scanned):

| tape | events | 0 ms slack | 10 ms slack | more |
|---|---|---|---|---|
| our simplified champion (23.545, budget 24.050) | 315 | **312** | 3 | 0 |
| **human WR `zetos.` 24.342 (budget 24.400) — the control** | 99 | **97** | 1 | 1 (the post-finish event, 200 ms) |

**Read on its own, "312 of 315 inputs have zero slack" says our tape is
unteachable. The control says otherwise: the human world record's own tape, a
keyboard run a person actually drove, is 97 of 99 at zero slack on the same
measurement.** So this number is a property of *this map under open-loop
replay* — a 24 s chaotic run with two 800 km/h launches and a wall collision —
and not a property of our tape. A driver is closed-loop and does not replay a
tape; the honest statement is that **the map is unforgiving for everybody, and
our tape is no worse than the world record on the only comparison that controls
for that.**

**Verdict: the 0.480 s half is free — go and copy `TheWoreL`. The 0.446 s half is
precision-bound on a pad, and out of reach on a keyboard.** The deliverable that
actually helps a keyboard player is `TAS_24164_keyboard_v1`: three values, 130
presses, 0.178 s faster than the best keyboard human on the board, and the
fastest keyboard tape 700 000 evaluations could find.

## 7. Three defects found in the toolchain, and one in the simplifier

### 7a. `--quant` is silently ignored on the classic search path (hardened build)

`tmtas-rs-hardened.tgz` parses `--quant` into `Args` and then hands it **only to
the fork configuration**. On the classic (non-fork) path nothing applies it, so
every "keyboard-constrained" arm launched without `--fork` is an ordinary analog
search that reports no error. Two arms were lost to this before a "keyboard"
tape turned out to have 239 distinct steer values.

Restored from the pre-hardening tree with one change: the snap now covers only
the **search window** `[flo, fhi)` rather than the whole tape. That matters here
— the fast basin's approach is a human's *analog* tape, and projecting it onto a
keyboard alphabet DNFs — and it is what made the keyboard-boundary table above
measurable at all. Patch in `tools/`.

### 7b. `fk btraj`'s self-check rejects fast maps

`fkdrv/src/layout.rs` requires `|d(pos)/dt − v| ≤ 2.0 m/s`, absolute. That
residual scales with speed. Here a **correctly located** vehicle struct reads
2.32 m/s at a mean speed of 113 m/s — 2 % — so the check aborts and no
search-produced tape can be measured at all.

Changed to `tol = max(2.0, 0.03 × mean_speed)`, which leaves every slower map
exactly as it was, and then **verified against ground truth rather than
assumed**: `fk btraj` on the human WR versus that ghost's own decoded
`CPlugEntRecordData`,

| shift applied | mean \|Δpos\| |
|---|---|
| −10 ms | 2.3003 m |
| 0 | 1.1514 m |
| **+10 ms** | **0.0007 m** |
| +20 ms | 1.1513 m |

**0.7 mm over 304 samples.** The locator is exact; the only discrepancy is a
**whole-tick clock-label offset — `fk btraj` timestamps read 10 ms early** —
fleet defect 3 surfacing where it is harmless (it cancels in any comparison) but
would silently bias an absolute reading. The next-best decoy triple was 1.7 m/s
at a mean speed of 1.2 m/s, i.e. 140 % off: the margin is not close.

### 7c. Search-produced tapes declare their seed's time (`IsValid: false`)

A candidate is a patched copy of its seed, so it still declares the seed's race
time; the server then reports
`"validated time is actually better!"` and `IsValid: false`.
Nothing is wrong with the run, but a published replay that says 27.609 is useless
to a human and makes a clean re-check impossible.

`k290 retime <ghost> --ms N --out F` rewrites the declared time in body chunks
`0x03092005` and `0x0309202B` and in the header. The champion now validates
**`IsValid: true`, `NbRespawns: 0`** and declares its own time. Both the retimed and the raw
tape are banked; they simulate identically.

### 7d. `simplify.rs` phase 2b can loop forever

A *successful* ramp collapse does not add the span to `refused`, and the step it
writes leaves a short run between two held runs — so the scan re-detects the same
span, collapses it identically, and never terminates. Observed here as 187
identical `collapsed ramp 2197..2202` lines and a 110-worker run that would never
have finished. One-line fix (`refused.push(span_lo)` after a successful collapse)
in `tools/`.

## 8. Method notes worth keeping

* **Rank the field by the physical event, not by the projection.** The
  arclength-projection table said `rank13` was 0.500 s ahead at the last station,
  but part of that is projection artefact through a long air phase. Timing the
  *tower impact* — one unambiguous event — gave the same ordering for free and is
  not arguable. That table chose the seed, and the seed decided the map.
* **Seed from the fastest APPROACH, not the fastest RUN.** The world-record basin
  converged to 24.205 and stopped. The rank-13 basin passed it in five minutes
  and finished 0.789 s ahead. On a map whose spread lives in one obstacle, the
  run to seed from is the one that is fastest *arriving* at that obstacle,
  however bad its finish. This is the transferable finding from this map.
* **The endgame-only window is not a shortcut, it is the whole search.** 250 000
  evaluations mutating race 9–17.5 s produced zero improvements; the same box
  aimed at race ≥ 17.5 s produced 0.926 s.
* **All seeds tested; the basins do not merge.** `rank01` → 24.205, `rank15`
  (which reaches the tower *earliest* of all) → 24.248, `rank13` → 23.462. Being
  earliest to the obstacle is not sufficient: `rank15` strikes a different part
  of the tower (z ≈ 1332 rather than ≈ 1365) and that bounce is worse.
* **The sub-tick plane is INVALID here and was not used.** Precondition measured
  first, as the brief requires: extrapolating all 22 runs to their own validated
  finish millisecond gives a crossing-coordinate spread of 8.5 m in x, 1.6 m in
  y, 8.8 m in z at a median crossing speed of 16 m/s — **~101 ms of systematic
  error against a 1 ms budget.** The 227969 configuration, only far worse,
  because most of the field lands and drives in while the WR flies in at
  230 km/h. Not used; should not be used here.
* **The fork server was not needed.** Its blind locator does work here after 7b,
  but the classic path ran at ~150 evaluations/s per 45-worker arm with a 70–93 %
  finish rate on the endgame window, and the whole result landed in under an hour
  of search. Every number in this document comes from the plain oracle.

## 9. A driving guide

Cues below are what the driver can actually perceive — speed on the HUD, the car
leaving the ground, the tower arriving — not tick numbers. Times are race
elapsed, for orientation only.

**Sectors 1–5 (0 → 14 s): copy `TheWoreL` (rank 13 on the leaderboard).**
Standing start, ramp, bend, booster chain, the big first jump, the landing and
the mid-section. Nothing we did improves on it, and it is 0.39 s ahead of the
world record by the time you reach the ceiling. This part is already public.

**Sector 6 (14.1 → 16.0 s) — the upside-down run: the biggest human-vs-human
difference on the map, 0.120 s.** You come up onto the inverted section rolled
fully over (roll = π) at around 400 km/h. The world record arrives at 255 km/h
and spends the section rebuilding speed; `TheWoreL` arrives at 409 km/h and
never gives it up. **Carry the speed onto the ceiling; do not let the transition
scrub it.** Everything downstream — how early you reach booster chain #2, how
early you reach the tower — is set here.

**Sectors 7–9 (16.0 → 17.0 s) — over the edge, 94 m down to y = 66.** Ballistic
and forgiving. The field varies by 0.020 s.

**Sectors 10–12 (17.0 → 19.1 s) — the flat run and booster chain #2.** Full
throttle. `TheWoreL` enters at 604 km/h where the world record enters at 464 and
tops out at 765 vs 752. This is where the mid-section gain becomes a speed
advantage.

**THE INPUT (19.1 → 21.1 s) — launch #2, and the only thing you must learn.**
The car leaves the ground at about 765 km/h, rolled onto its side. **Hold full
left through the entire flight.** Everyone in the field lets go and flies; the
lock pitches the nose down and flattens the arc. You will pass the same
landmarks lower and slightly further along.
Honest warning: this input is analog on our tape and could not be reproduced on
a keyboard at any placement we tried — the search finishes 0 % of the time if the
run-up is restricted to three values. On a pad, it is one long hold, not a
sequence of flicks, which is the good news about it.

**The tower (≈ 21.1 s).** You strike the wall at ~675 km/h and are thrown
straight back. **The target is to arrive LOW.** Reference heights: world record
152.8, field median ~161, `TheWoreL` 172.8, ours 141.8. Eleven metres lower is
0.436 s. Coming in low is the whole trick, and it is bought entirely during the
flight, not at the wall.

**The fall (≈ 21.1 → 23.5 s).** Free fall down the face at ~255 km/h backwards,
gaining to ~300. Steering here matters only for attitude; **this part IS
keyboard-drivable** (91 % of keyboard-constrained candidates finish once past
takeoff). Aim to fall through the gate rather than land beside it and drive in —
that alone is what separates the world record from the other 20 runs on the
board.

**The gate** sits on a platform at (1522, 74, 1340) and is entered moving −x.
The world record crosses it airborne at 230 km/h; ours at ~290; everyone else
between 15 and 90 km/h after landing.

## 10. Validation

* **Ten cold passes in total** (five on the 23.462 champion, five on the final
  23.416), fresh processes, each carrying known-answer controls — the human WR
  24.342, rank02 24.634 and the seed 27.609. Every row identical in every pass,
  and all six deliverable tapes re-validated together in each of the final five.
  `VALIDATION.md`, `VALIDATION_FINAL.md`.
* **Cold map re-download** from Nadeo's public endpoint mid-run:
  sha256 `ecb6a296…97fc`, **byte-identical** to the file the whole search used,
  and the champion validates to 23.416 against the freshly downloaded copy.
* Champion `TAS_23416_v3_CHAMPION.Ghost.Gbx` sha256
  `ba015a6ddac620eaf9fd0403ad61f05a6e5ba23760f17e0dab9a5e01bbdb6e81`; all tape
  hashes in `tapes/SHA256SUMS.txt`, final transcript in `VALIDATION_FINAL.md`.
* Guard on throughout (hardened build): every banked improvement re-validated
  through the plain oracle before acceptance. **No phantom fired in this run.**
* `tmtas selftest` 10/10 on this node; candidate-factory round-trip exact.
* Nothing was ever submitted to a Nadeo leaderboard.
