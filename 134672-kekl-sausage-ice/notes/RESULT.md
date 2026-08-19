# 134672 `KEKL- SAUSAGE ICE` — the author time did NOT fall

**AT 58687 · human WR 63546 (Roevhaal, 2022) · best today-legal human 68442 ·
our best validated 67404.**

uid `agH9XtjTZd8iZbuGp_KhC16jMO7` · TMX 134672 · author `Travis.TM`, uploaded by
"KEKL Archive" · TMX comment: **"Built in 15mins for KEKL"** · 15 records.

| tape | validated | vs AT | vs best today-legal human | steer alphabet | steer events | device |
|---|---|---|---|---|---|---|
| `C_67404` analog | **67404** | +8717 | **−1038** | 74 values | 193 | TAS |
| `D_kbd_67625` | **67625** | +8938 | **−817** | **3** (`−127/0/+127`) | 114 | **keyboard** |
| `A_67629` | 67629 | +8942 | −813 | analog | — | TAS |
| human rank 2 (best on a current build) | 68442 | +9755 | 0 | 3 | 101 | keyboard |
| human WR 2022 (does not re-simulate) | 63546 | +4859 | — | 3 | 111 | keyboard |

All numbers through the plain oracle, two cold passes each, with two
known-answer human controls (68442 and 94940) in every batch, against the
untouched map (`md5 e73cb7b4e201edd176be97566adffb4b`, sha256-identical between
the Nadeo CDN and TMX copies).

The author time did not fall, and the value of this map is the *evidence about
why*, which is unusually complete.

---

## 1. The map

317 blocks: **~252 `TrackWall*Pillar`** stacked five cells high (y-cells 9–13)
with a **custom-ice deck on top** (41 `FlinkIceBlocks\3-1-*-Ice-Light`
`_CustomBlock` at y-cell 14), 3 `RoadIceStraight`, 2 `RoadIceWithWallCurve3`, one
**`GateSpecialTurbo`**, four `GateCheckpoint`, one `GateFinish`, a
`RoadBumpStart`. The 199 items are scenery (54 rocks, 49 `zTrackSlopeLoopStart`
parked at y = 168 m, support bars, light rails, four palm trees) scattered over
the whole grid — not track.

A **narrow elevated ice ribbon**, ~2620 m, driven at 30–45 m/s with the car
permanently sideways. Waypoints:

| | block | cell | world |
|---|---|---|---|
| spawn | 311 `RoadBumpStart` | 27,14,20 | 880, ·, 656 |
| CP1 | 165 | 24,13,15 | 784, ·, 496 — 32 m past the **turbo gate** at x=752 |
| CP2 | 170 | 16,13,14 | 528, ·, 464 |
| CP3 | 243 | 7,13,17 | 240, ·, 560 |
| CP4 | 261 | 15,14,24 | 496, ·, 784 |
| finish | 244 | 17,13,23 | 560, ·, 752 — 8 m BELOW CP4, crossed airborne |

**Medals**: bronze 89000 / silver 71000 / gold 63000 / author 58687 — exactly
Nadeo's chain off a driven author time (gold ≈ AT × 1.0735, silver ≈ gold ×
1.127, bronze ≈ silver × 1.2535, rounded to the second), and the header says
`validated="1"`. **The AT is a hand-set driven validation lap, not a formula.**

**There is no embedded author ghost.** `tmtraj decode map.Map.Gbx` finds no
`CPlugEntRecordData`, and a scan of every skip chunk of the decompressed body
finds no `0x0911F000`, no `0x0309201D`, no `CGameCtnGhost` substring. So
`validated="1"` does **not** imply the validation lap is readable — worth
recording, because a sibling map's author ghost *was* embedded.

One number to hold on to: **the author's own online record is 69522, rank 3** —
10 835 ms slower than their own validation lap, on a map they built.

## 2. The measurement that explains everything: a 1/127 error e-folds in 0.7 s

rank02's tape with **one steer unit changed on one 10 ms tick**, timed at gates
along sector 1 against the unperturbed reference (plain oracle throughout):

| gate | ref | +1 unit @ 2.0 s | +1 unit @ 10.0 s |
|---|---|---|---|
| 1.9 s | 1916 | 1916 | 1916 |
| 2.9 s | 2927 | **2927 exact** | 2927 |
| 8.0 s | 7973 | 8037 (+64) | 7973 |
| 9.6 s | 9634 | 15716 (lost) | 9634 |
| 10.8 s | 10804 | — | 10803 (−1) |
| CP1 13.9 s | 13906 | — | **14079 (+173)** |
| finish | 68442 | DNF | DNF |

Invisible for about a second, then a factor of **e every 0.6–0.8 s**. An input
error eight seconds from the line is amplified ~10⁵ times. Five of five
single-unit single-tick changes (ticks 200/300/1000/1500/2000) DNF the run.

Corollaries, all observed: 15 records spread over **40 seconds**; three of them
contain respawns; and §3.

## 3. §8 field reproduction: 5 of 15 — a GO, not a stop

Perfect separation by **game build**:

| build | ranks | reproduce |
|---|---|---|
| 2022-07-06 git 113150 | 1,3,4,5,6,7,8,9,12,15 | **0 / 10** |
| 2025-07-04 · 2026-01-18 · 2026-02-02 | 2,10,11,13,14 | **5 / 5 exact** |

Ruled out by measurement: truncated downloads (§8a — every file GBX-magic and
full length); decoder/format (all archives fv 12, every decoded tape time-aligned
with its own telemetry); the start-offset convention (both `0` and `≈ −1550` on
both sides of the split); an edited map (TMX one version, `UploadedAt ==
UpdatedAt`, Nadeo copy sha256-identical); respawns (split across both groups).

On a map with a 0.7 s e-folding time, *any* build difference — one ULP — is fatal
to a 60-second open-loop replay. The oracle is nonetheless exact for today's
game: three distinct 2025–2026 builds reproduce to the millisecond **including a
101 259 ms run**, and `fk traj`'s state locator tracks rank02's own recorded
telemetry to **rms 0.008 m over the whole 68 s**. That is the opposite of the
203072 failure, where a quarter of the field on the oracle's own build returned
*different finish times*.

### The 2022 world record: where it dies, and why it cannot be repaired

Gate ladder over sector 1 (rank01, 63546, pure keyboard):

| cell | gate says | its own recording |
|---|---|---|
| 27,21 | 1915 | ~1900 ✓ |
| 27,22 | 2924 | ~2900 ✓ |
| 23,20 | 7434 | ~7400 ✓ |
| 23,19 | 7969 | ~7900 ✓ |
| 23,18 | **8871** | ~8700 ✓ |
| 22,17 | 12207 | ~9560 ✗ |
| 22,16 | 37435 | ~10600 ✗ |

**Exact for 8.9 s, lost by 9.6 s, at the map's one air phase** (wheels off the
ground at t ≈ 8.5 s, y climbing 52 → 60). Repair attempts, both conclusive:

* **Exhaustive single-move neighbourhood over the entire break** — every steer
  value on a 4-unit ladder at every tick in 800–980, plus every accel and brake
  flip: **11 869 candidates, 0 finished**, none reached even CP2.
* A 110-worker, 20-minute search over the same window, full-map objective with
  checkpoint shaping: never reached CP2 either.

So the 63546 line is not available, and everything below starts from rank02's
68442.

## 4. Is there a route the field has not found? Not proven either way

The natural hypothesis for a 4859 ms gap over 15 records is a cut. What was
tested:

* 54 synthetic full-throttle tapes from the start (steer −127…+127 held
  0.5–3.0 s) reach no gate in any cell adjacent to the start and no mid-field
  cell;
* rank01's diverged tape, which wanders for 30 s after it is lost, reaches no
  mid-field gate either — it stays inside the corridor.

**Both are weak negatives and should be read as such.** A relocated gate is a
small asymmetric trigger box: a gate no tape has ever fired has no yes-control,
so its silence is not evidence of absence (the failure two other agents in this
project burned thousands of runs on). My mid-field gates had no yes-control. What
*is* established is that gates fire over blockless terrain cells the field
actually drives through — (24,22) and (22,16) both fire — so blockless does not
mean void, and the corridor does include unbuilt ground.

**Unresolved, and the first thing to do if this map is picked up again:** place a
mid-field gate, drive a tape to it deliberately (or fire it with a spliced tape),
and only then read a zero as a zero.

## 5. What the field's own data says

Sectors (start→CP1→CP2→CP3→CP4→finish), 15 records:

| sector | best | mean | sd | corr with finish |
|---|---|---|---|---|
| S1 | 13209 | 14924 | 1237 | 0.89 |
| S2 | 17651 | 21477 | 2244 | 0.75 |
| S3 | 11309 | 15504 | 4247 | 0.79 |
| S4 | 17130 | 20987 | 3281 | 0.84 |
| S5 | 3964 | 7572 | 4345 | 0.61 |

Every sector correlates 0.61–0.89 with the final time: a field separated by
general control, not by one feature. And:

> **The best sector time in the whole field, summed, is 63263 ms — still 4576 ms
> slower than the author time.** A perfect splice of everybody's best driving
> does not reach the AT.

Grip: mean |lateral velocity| **13.8–23.2 m/s over the whole lap**, and
**monotone in pace** (WR 23.2, last place 13.8). Airborne 3.4–6.1 %; throttle
78–92 %; brake 0.5–12 %; gas-and-brake-together 1.4–6.3 %.

### Steering saturation: a clean negative for "don't pin full lock on ice"

From the input tapes, race ticks only:

| class | n | mean lock % | corr(lock %, finish time) |
|---|---|---|---|
| all | 15 | 66.4 | **−0.40** |
| pure keyboard (3 values) | 8 | 73.1 | **−0.77** |
| pad (127–254 values) | 7 | 58.1 | **−0.47** |

More full lock goes with a **faster** time in both device classes. Eight of
fifteen records are pure `{−127, 0, +127}` keyboard, including the top three;
median steer hold 170–290 ms for keyboard against 10–20 ms for pad, and the pad
runs are the slow half. On this map the fast line is a *committed continuous
drift* and the steering is for rotation, not grip. Sibling agents found the
opposite on long ice sweepers (285268, 279209) — so the rule is corner-class
specific, not surface specific: **back off lock where you are trying to keep the
car pointed and accelerating; keep it pinned where you are trying to rotate.**

## 6. The instrument this map needed: `tmmaps gateladder`

Parks every checkpoint off the track (renamed to a finish so it is not required,
moved to cells 1..4,9,1) and relocates the real Goal block to **any 32 m cell**,
keeping the first N checkpoints real so a gate cannot be reached by cutting past
one. New primitives `MapFile::set_block_cell` / `set_block_dir` (blocks now carry
`coord_off`).

**Verified exact with yes-controls**: a gate at CP2's cell returns 33106 for
rank02 and 36146 for rank10 — each run's own declared split, to the millisecond;
CP3's cell returns 45437 / 49728. Orientation matters (`dir` 1/3 for crossings
along x, 0/2 along z); generate both and use the one that fires.

It converts a DNF into "reached cell (x,z) at t" for the price of one
validation, and every localisation in this document rests on it.

Also fixed here: **`tmmaps build` derived this map's checkpoint order wrong**
(243,165,170,261 instead of 165,170,243,261), so its `map_seg2/3/4` are all
really a CP4 gate. Both are still exact, they just measure something other than
what they are named.

## 7. How far the search got, and the ceiling it implies

**Per-sector ceilings.** Each sector optimised against its own gate with a
cumulative objective, 40 min × 42 workers, seeded from rank02, other sectors left
at rank02's pace:

| sector | rank02 | ours | gain | field's best |
|---|---|---|---|---|
| S1 | 13906 | **12552** | −1354 | 13209 |
| S2 | 19200 | 18542 | −658 | 17651 |
| S3 | 12331 | 11945 | −386 | 11309 |
| S4 | 18375 | 16597 | **−1778** | 17130 |
| S5 | 4630 | 3856 | −774 | 3964 |
| total | 68442 | **63492** | −4950 | 63263 |

Three independent estimates of what this route is worth — the field's
best-sector recombination (63263), the 2022 human world record (63546), and our
own per-sector optima summed (63492) — **land within 283 ms of each other, and
the author time is 4.8 seconds below all three.**

**But the sector gains do not compose.** Because of §2, a change at tick t
invalidates everything after it, so a sector optimum is only reachable if the
remaining 40+ seconds can be re-derived. Measured:

* a 42-minute, 528 000-evaluation search with the mutable window opened to ticks
  2500–7000 and the **true full-run objective** found **nothing at all** better
  than 67404 — the 1778 ms available in sector 4 is not compatible with
  finishing;
* a staged gate-to-gate chain (18 gates, ~3 s apart) degraded instead of
  improving: each stage arrived at its gate sooner and in a state from which the
  next sector was worse, ending 18 s down by the middle of the lap.

**Everything we did gain is in the last 7.5 seconds**, where the tail is short
enough to re-derive:

| gate | rank02 | ours (67404) |
|---|---|---|
| cell 16,22 (61.6 s) | 61570 | 61583 |
| CP4 | 63812 | 63942 (+130) |
| cell 16,25 | 65495 | 65456 (−39) |
| finish | 68442 | **67404 (−1038)** |

Our final sector is **3462 ms against the field's best of 3964** — 502 ms faster
than any human's closing sector, bought by giving up 130 ms into CP4 and then
carrying the drop differently. That is the one genuine technique this map gave
up, and it is the only part of our tape that is not rank02's driving.

## 8. Honest reading of the 4.8 s

Our sector optima are **lower bounds** — local hill climbs from a human seed on a
chaotic tape, not global optima — so "the AT is unreachable" is **not proved**.
But three facts sit together uncomfortably:

1. the AT is 4.8 s beyond everything this route has ever produced: 15 human
   records, a best-sector splice of all of them, and a TAS optimising each sector
   independently;
2. the author's own online record is 10.8 s slower than their own validation lap,
   on their own map;
3. the map was **built in 15 minutes** out of stacked wall blocks and *embedded
   custom ice blocks*, and saved on the 2022-07-06 build.

The hypothesis that fits all three, and that cannot be tested without the
author's own files, is that **the validation lap was driven on a state of the map
that is not the state that shipped** — most plausibly before the custom ice
blocks were placed, or before they behaved as ice. The two alternatives we *could*
test are negative: the map file is byte-identical to Nadeo's own copy with one
TMX version, and no route cut was found (§4, with the caveat that the negative is
weak).

We do not claim it. We record that on the map as published, under physics the
oracle reproduces to the millisecond for every record set on a current build,
**58687 is 4.8 s beyond the best line anyone — human or machine — has produced.**

## 9. The human deliverable

### 9a. The tape family

| tape | time | events (steer / gas / brake) | alphabet | notes |
|---|---|---|---|---|
| `C_67404.Ghost.Gbx` | 67404 | 193 / 99 / 28 | 74 steer values | unconstrained floor |
| `D_kbd_67625.Ghost.Gbx` | **67625** | 114 / 101 / 32 | **3** | **pure keyboard, searched under the constraint** |
| `A_67629.Ghost.Gbx` | 67629 | — | analog | independent arm, different basin |

Sector splits, keyboard tape: 13906 / 33106 / 45437 / 63925 / **67625** — closing
sector **3700 ms**, still 264 ms faster than the best closing sector any human
has driven (3964). So the finish technique survives the keyboard constraint
almost intact.

The keyboard tape costs **221 ms** over the analog floor and is directly
drivable: `{−127, 0, +127}` steering, digital throttle, digital brake. It was
searched with `--qlevels 1` from a human keyboard seed, never projected —
projection does not work on this map any more than it did on 227969.

### 9b. Simplification: none is available

`tmsimp`'s pipeline on the 67404 tape, **83 319 oracle evaluations**, could not
delete a **single one** of the 319 input events for a budget of 40 ms. For
comparison, on 227969 the same pass took an analog tape from 185 steer events to
62 for 23 ms. **On this map every input is load-bearing.** That is the 0.7 s
e-folding time expressed as a property of the tape.

### 9c. Per-input tolerance (recoverable: mistime one input, re-time only the later ones)

| input | usable window | cost at the edge |
|---|---|---|
| 59.16 s (full left) | **1 tick** | any mistiming DNFs |
| 59.22 s (release) | **1 tick** | any mistiming DNFs |
| 59.47 s (full right) | **1 tick** | every shift DNFs |
| 59.54 s (release) | **1 tick** | any mistiming DNFs |
| 59.71 s (full right) | 4 ticks (−1…+2) | +7 to +263 ms |
| 59.86 s onward | 9 ticks (±40 ms) | 0 to +18 ms |

So the closing technique has **four 10 ms-tight commitments between 59.16 and
59.54 s**, and everything after 59.86 s has ±40 ms of slack. A driver has to hit
the entry to the last complex within one tick; the drop itself is forgiving.
This is not a verdict of "impossible" — the four tight inputs are *our* route
into the drop, and the forgiving 1038 ms is mostly in what happens after them.

### 9d. Sector-by-sector guide, off visual cues

Read from the human WR's own tape (`m134 guide`), which is pure keyboard and the
right thing for a person to copy; times are its own.

* **Start → the north loop (0–4.2 s).** Full throttle from the line, dead
  straight for 1.8 s up the start ramp. One 50 ms left dab at 1.78 s to settle
  the car, then **full right at 2.60 s** with a 60 ms release at 2.72 — you are
  turning left-handed round the top of the map with the wall on your right.
  Lift the throttle for 250 ms at 3.37 s as the nose comes round; that lift is
  what stops the slide widening.
* **The long left descent (4.2–8.4 s).** **Full left from 4.24 s and hold it for
  a whole second** — this is the map's biggest single input, and the car is at
  36–44 m/s and 25–40 m/s *sideways* through it. Coast (no gas) 5.30–5.43.
  Straighten at 6.03, then alternate short full-lock stabs (110–320 ms) down the
  ridge; you are aiming at the gap where the deck rises.
* **The jump and the hairpin into CP1 (8.4–13.9 s).** You leave the ground at
  about 8.5 s. **Gas off through the air, brake dab at 8.76 for 470 ms**, land
  under full right lock. Then the slowest corner on the map: you scrub to
  19–20 m/s at 11.7–12.1 s. Full left all the way round it.
  **Cross the turbo gate at x = 752 pointing straight down the ice straight** —
  it takes you from 33 m/s to 50 m/s in 250 ms, and CP1 is 32 m past it. Getting
  this exit straight is worth more than the corner itself.
* **The east loop (13.9–31.1 s).** Ice straight at 56–61 m/s, one long full-left
  hold of **2.57 s** from 15.96 s round the far end, peak 72 m/s at 20 s. This is
  the fastest part of the map and the least fiddly: two long holds, not many
  small ones.
* **The west run and CP2 (23.5–31.1 s).** The field loses about a second here to
  the WR: the WR carries **46.6 m/s where rank 2 carries 36.6**. It is not a
  trick — it is not scrubbing on entry. Short right stabs, gas on, no brake.
* **CP2 → CP3 (31.1–42.5 s).** Full-lock left, a 1.82 s hold from 32.82, then a
  1.0 s coast from 34.64 — the only long coast on the map. Full-lock right
  through 39–42 s with the car at 43–47 m/s and **43–47 m/s sideways**, i.e.
  completely square to its own velocity.
* **CP3 → CP4 (42.5–59.6 s).** The longest sector, and the one where our search
  found time it could not keep. Nothing discrete: hold the drift, do not lift.
* **The finish (59.6–67.4 s) — the one thing we can teach that nobody does.**
  The field arrives at CP4 as fast as possible and then loops wide via the
  z = 800 side, taking 2.9 s from the (16,25) cell to the line. **Give up
  130 ms into CP4** — arrive slower and rotated — and the drop to the finish
  gate, which sits 8 m *below* CP4, can be taken in **1.95 s instead of 2.95 s**.
  Our closing sector is 3462 ms; the best human closing sector in three years is
  3964 ms. The four commitments listed in §9c are the entry to it, and they are
  one tick each; everything after has ±40 ms.

**Verdict on the technique: known-but-unheld everywhere except the finish, where
it is undiscovered.** The 4896 ms between the 2022 WR and rank 2 is diffuse
carry speed — the cumulative delta grows almost linearly with distance, with no
single feature worth more than a second. The finish is the exception, and it is
the only place our machine found something no human has done.

## 10. Artefacts

`~/persistent/private-30d/tm-unbeaten/134672/`

| file | what |
|---|---|
| `acq-v1.tgz` | map, mapinfo, leaderboard, all 15 ghosts |
| `tapes-v2.tgz` | `keep/` — the validated tapes above, plus the cold-validation transcripts |
| `tools-v2.tgz` | `m134.rs` (field/grip/sat/cells/jumps/guide/vs/synth/enum1/chunks), `tmmaps` gateladder + `set_block_cell`, `chain.sh`, `chain2.sh`, `back.sh`, `validate.sh`, block dump |
| `NOTES-v2.md` | the working log, in the order things were found |
| `PLAN-v1.md` | the plan written before searching, for comparison |
| `field-v1.txt` | sector table, grip table, saturation table, checkpoint positions |
| `RESULT.md` | this file |
