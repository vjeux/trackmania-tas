# 146612 — "Spaghetti Nights 2" — the AT did NOT fall; the missing technique did

AT **38530** · human WR **40223** (jujumasterr) · 181 records · gap **1693 ms**
Map sha256 `c6cca762e167eba6e969c07f306798c29c88d0da397b4744d4042c51b21526db`
(3 824 673 B, Nadeo-served), uid `jchzEcocJbNJreH4ebIoUYOt286`, TMX 146612.
Authors AmpelJoe10 + Wakawukwuk. Tags Race / Tech / Competitive.
Session 2026-08-18, node 46836, ~3 h.

---

## Headline

| tape | validated ms | vs human WR | vs AT | alphabet | steer events | device |
|---|---|---|---|---|---|---|
| `BEST_39961_v3` | **39961** | **−262** | +1431 | 76 values | 234 | pad/TAS |
| **`KEYBOARD_39996_v3`** | **39996** | **−227** | +1466 | **3 `{-127,0,127}`** | **119** | **keyboard** |
| `BEST_39973_v2` | 39973 | −250 | +1443 | 30 | 148 | pad/TAS |
| `KEYBOARD_40001_v2` | 40001 | −222 | +1471 | 3 | 117 | keyboard |
| `BEST_40040_v1` | 40040 | −183 | +1510 | 56 | 198 | pad/TAS |
| `KEYBOARD_40058_v1` | 40058 | −165 | +1528 | 3 | 114 | keyboard |
| human WR rank 1 (control) | 40223 | — | +1693 | 226 | 1157 | pad |
| human rank 2 (control) | 40226 | +3 | +1696 | 3 | 114 | keyboard |

Every row validated through the plain oracle against the untouched map, twice,
in cold batches carrying both human runs as known-answer controls (40223 and
40226 exact in every batch).

**The author time did not fall — we are 1431 ms short.** What did fall is the
question the AT was asking:

> **Sector 4 of this map contains a 190-metre gap jump that all 181 humans
> either avoid or take wrong, and taking it right is worth 1128 ms — two thirds
> of the entire unbeaten gap.**

That is measured, not inferred: `JUMP_cp5_32702_v1` reaches the last checkpoint
in **32702 ms** against the best human's 33830, and the plain oracle confirms on
the untouched map that it crossed all five real checkpoints (`cps=5`). It does
not finish: from that checkpoint-5 state the car is 1.1 s early, 12 m/s slower
and pointing differently, so the final 6.4 s has to be re-driven from nothing,
and that search did not converge before the session ended. **The technique is
established and validated to the checkpoint; the lap that carries it is not.**

And separately, a clean drivable result: **a pure-keyboard tape, 117 key
presses, three steering values, beats the analog human world record by 222 ms**
— on the field's own route, with no jump.

---

## 1. Controls, and a correction to the brief

* **Identity control, my own 181-ghost set: 176/181 exact.** Five fail — ranks
  57, 59, 100, 151, 173 — all `DNF cps=1`, **none returning a different
  millisecond**. They re-download byte-identical, so this is not ACQUISITION
  §8a's truncation artefact; ranks 151 and 173 contain mid-run **respawns**
  (223–433 m jumps back to a checkpoint) which an input-replay oracle cannot
  reproduce. **The whole top 56 reproduces exactly.** Unlike map 203072 — where
  the world record itself DNF'd and three runs came back with *different* times,
  making any improvement unfalsifiable — nothing here returns a wrong time.
  **§8: PASS**, with those five excluded from all analysis.
  *(The agent who handed me a 40/40 §8 pass sampled ranks 1–15, 21–25, 51–55,
  91–95, 131–135, 167–171. None of the five failures is in that sample. Their
  pass was real but lucky; 176/181 is the number to quote.)*
* Codec loop closed: ranks 1 and 2 rebuilt through the search's own encoder
  re-validate at 40223 / 40226.
* **"181 records means the field is settled" is measurably wrong here, and it
  matters.** Only **5 runs are within 1 s of the WR**, and there is an **849 ms
  cliff between rank 2 (40226) and rank 3 (41075)**. This is two players who
  duelled and 179 who did not. A 1693 ms unbeaten gap on a map with a two-person
  top is far less surprising than the record count suggests — and it is why a
  whole technique was still lying on the floor.

## 2. The map, measured

* 5 checkpoints + finish. Gate items: CP1 `#439` (624,42,991) · CP2 `#494`
  (1104,10,774) · CP3 `#440` (672,10,816) · CP4 `#633` (768,19,590) ·
  CP5 `#492` (1170,42,736). Rank-1 splits 7311 / 15718 / 19980 / 27834 /
  33584 / 40223.
* `tmmaps list` panics here (`unhandled inline node class 0x40000000`);
  `TMMAPS_NO_BAKED=1` parses it. Block world centre =
  `(32·cx+16, 8·cy−62, 32·cz+16)`, calibrated on the spawn and confirmed
  against every checkpoint item's own recorded position.
* **The map has FOUR finish gates in a diagonal staircase:** G1 (720,34,976),
  G2 (688,34,944), G3 (656,34,912), G4 (656,34,880). The last straight runs
  in −x, so G1 is the first one reachable:

  | gate | n | mean sector 5 | best sector 5 | mean v at CP5 | best final |
  |---|---|---|---|---|---|
  | **G1** | **16** | 7331 | **6396** | 72.2 | **40223** |
  | G2 | 8 | 7860 | 7121 | 65.9 | 41561 |
  | G3 | 101 | 8886 | 7421 | 63.2 | 43616 |
  | G4 | 56 | 8502 | 7558 | 63.3 | 43054 |

  Only 16 of 181 reach G1; 11 of those are in the top 15. It is not simply a
  consequence of speed — rank 16 arrives at CP5 at 75.2 m/s, matching the world
  record, and still takes G3 and pays about a second. **Reaching G1 is a
  teachable ~1 s for anyone in the 43–45 s range.**
* Physics: ~22 % airborne, 0→445 km/h (mean 318), roll to 1.0–2.0 rad, no
  reactor or boost. **The finish is crossed airborne with roll ≈ 0.7 rad**, so
  the in-child sub-tick *plane* surrogate is contraindicated (the 227969
  incident). Never needed: the gap is 1693 ms, not sub-tick.
* Medals AT 38530 / gold 42440 (AT×1.1015) / silver 58350 / bronze 62260 — not
  an auto-generated ladder. `inPlugin:false` on unbeaten.at, so no
  `atSetByPlugin` evidence either way. Both authors are on the board at 41795
  (rank 7) and 42498 (rank 12), 3.3 and 4.0 s off their own AT.

## 3. Where the time is

Sectors ranked by **correlation** with the final time, not spread (270051's
finding). Top 20:

| sector | mean | min | spread | corr(final) | path/chord | mean speed |
|---|---|---|---|---|---|---|
| 0 start→CP1 | 7490 | 7295 | 532 | +0.72 | 1.19 | 55.7 |
| 1 CP1→CP2 | 8672 | 8401 | 947 | +0.43 | 1.37 | 86.6 |
| 2 CP2→CP3 | 4447 | 3784 | 1062 | +0.43 | 1.01 | 100.0 |
| 3 CP3→CP4 | 8458 | 7854 | 1681 | +0.71 | **3.36** | 98.7 |
| **4 CP4→CP5** | 6109 | 5674 | 1035 | **+0.76** | 1.27 | 91.4 |
| 5 CP5→finish | 7220 | 6396 | 1586 | **+0.87** | 1.21 | 86.9 |

* **The ideal splice of the whole field's best sectors is 39404 — still +874
  over the AT.** The AT is not reachable by recombining what humans drive; a
  technique is missing.
* **And that splice is not even achievable.** `tmtas splice` over ranks
  1/2/6/9 at all five checkpoints: **all 60 cross-splices DNF.** On this map an
  open-loop tape cannot be handed between drivers at any checkpoint. 39404 is a
  bound-shaped statistic, not a time.

## 4. THE TECHNIQUE — the angled ramp jump in sector 4

### What the field does

After CP4 the track **branches** at `RoadTechBranchStraightX4Right` (816,10,592).

* **The loop line** — the 9 fastest sector 4s, including the entire top 7 —
  takes the branch, swings out to z≈520, comes back and climbs a ramp at
  x≈1075–1140 to CP5. Best: **rank 2, 5674 ms**.
* **The ramp line** — 144 of the 181 runs instead stay straight and hit the
  up-slope at `RoadTechSlopeBase` (944,10,592), which launches them ~190 m
  **straight down the z≈590 corridor**; they land at (1130,37,594) at 70 m/s and
  then crawl round a slow turn to CP5 at 51–60 m/s. Best: **rank 8, 6113 ms**.
  **In the top 40, every single ramp run is slower than every single loop run.**

So the field has tried the ramp and concluded it is a trap. It is a trap **the
way they take it**.

### What our tape does

Identical ramp, taken at an angle:

| | our tape | rank 13 (typical ramp run) |
|---|---|---|
| heading at the lip | **+21° across the corridor** | 0 to +1.5° (square) |
| speed at the lip | 104 m/s | 117 m/s |
| vertical kick | +33 m/s | +42 m/s |
| flight | 1.9 s, ~190 m, apex y 36.1 | 2.2 s, ~190 m, apex y 42.5 |
| lands at | **(1100, 33, 688)** | (1130, 37, 594) |
| landing speed | **81 m/s** | 70 m/s, decaying to 51 |
| **CP4 → CP5** | **4546 ms** | 6407 ms |

Because the car leaves the lip pointing 21° across, the two seconds of flight
carry it **90 m sideways**, onto the raised `PlatformDirtSlope2Curve2Out`
(1104,26,688) / `PlatformTechCurve1` (1136,42,688) platform — about 50 m from
CP5 and already pointing at it. The square jump lands on the wrong side of the
same structure and has to be driven round it.

**Nobody has ever landed there: 0 of 181 runs come within 30 m of
(1100,33,688).** (144 of 181 do pass within 25 m of the launch slope — they all
take it square.)

Cost of the angle: 13 m/s of entry speed, because you turn while you climb.
Worth it by more than a second.

| sector 4 | ms | vs ours |
|---|---|---|
| **angled jump (this work, `cps=5` on the real map)** | **4546** | — |
| best human, loop line (rank 2) | 5674 | +1128 |
| best human, square ramp (rank 8) | 6113 | +1567 |
| field mean | 6042 | +1496 |

### Verdict: **UNDISCOVERED — and actively mis-discovered**

Not "known but unheld": the field found the ramp, took it square, found it slow,
and went back to the loop. The one variable that decides it — the yaw at the lip
— nobody varied. This needs no new precision, only the knowledge that you should
be crossing the road while the ramp lifts you.

### How a human drives it

Full throttle throughout; no braking on the ground.

1. Leave CP4 on the **four-lane straight**. Do **not** follow the field's branch
   out to the low loop. You are at y=18 dropping to y=10, ~110 m/s.
2. Run the straight along its **low-z edge** (the side the branch leaves from).
3. About **80 m before the up-ramp** — roughly two block lengths of run-up; our
   tape starts at x≈905 — go to **full lock toward the high-z side** and hold
   it. You cross ~14 m of the road's width while the ramp lifts you. The cue is
   the ramp lip: you want to be *still turning* as you leave it.
4. Leave the lip at about **20° across the corridor**, not square (our tape
   passes +20.6° at the lip and +22.7° a fifth of a second later). Release as
   the car goes light.
5. Fly ~2 s. You land on the raised platform on the far side at ~80 m/s, lined
   up on the last checkpoint. (In the air, brake = pitch control; our tape uses
   two short brake pulses at 30.08–30.19 s and 30.61–30.91 s to set the landing
   attitude.)

**Tolerance on the decisive input:** the takeoff heading moves ≈ 0.5° per 10 ms
tick, so ±4 ticks (±40 ms) of release timing is ±2° of flight direction, which
over 190 m is ±7 m of landing point on a platform ~32 m wide. That is a
practisable margin — much wider than anything this project has had to report on
a map whose gap was sub-tick.

## 5. The drivable lap: keyboard beats the world record

Independently of the jump, on the field's own route:

* **`KEYBOARD_40001_v2` — 40001 ms, pure `{-127, 0, +127}`, 117 steer events,
  18 throttle events, 140 brake ticks.** 222 ms faster than the analog human
  world record, and it uses the same number of key presses as the human
  keyboard run it descends from (rank 2: 114 events, 40226 ms).
* Ground truth for the alphabet came from the humans: **6 of the top 15 are pure
  3-value keyboard runs**, including rank 2 at 3 ms off the WR. Hold floor from
  their tapes: shortest press 10 ms, p10 60–80 ms, median 110–170 ms; ranks 7
  and 8 never go below 30 ms.
* Confirming a standing project finding: **quantising the analog tape does not
  work** — snapping `BEST_40040_v1`'s tail onto the keyboard alphabet DNFs; the
  keyboard tape had to be *searched for* under `--qlevels 1`. It then overtook
  the unconstrained tape (40001 keyboard vs 40040 analog), and re-seeding an
  unconstrained arm from the keyboard tape produced the overall best, 39973.
* The world record itself **never lifts the throttle** (0 gas events in 4023
  ticks) and brakes for 186 ticks.

### Per-input tolerance, WITH the human control (270051's rule)

`tmsimp --mode tol`, mistime one input, re-time only the later ones:

| tape | steer events | 1-tick slack | ≥2 ticks |
|---|---|---|---|
| ours, keyboard 40001 | 144 | 117 (81 %) | 27 |
| **human rank 2, 40226 (control)** | 140 | **103 (74 %)** | 37 |

**Our tape is about as forgiving as the human's own run.** On this map an
open-loop tape is brittle whoever wrote it — three quarters of the human world
record's own key presses DNF the run if moved a single 10 ms tick. That is a
property of a 40 s chaotic tech map replayed open-loop, not a verdict on
executability; a driver is closed-loop. The inputs with real slack in our tape
are the ones worth naming to a driver: tick 2312 (21.58 s, 10 ticks), 3338
(31.84 s, 6 ticks), 3492 (33.38 s, 8 ticks), 3938 (37.84 s, 9 ticks) and
everything after 38.0 s (13 ticks — the run-in is free).

## 6. What is NOT delivered, and exactly why

The jump tape does not finish: from its CP5 state the car overshoots the first
corner of sector 5, runs on to (1245,27,814) and falls off. Everything tried:

* **Tail time-shift** — `tapeshift --sweep` over 95..125 ticks: every shift DNFs
  and most also lose CP5. The tail is not late, it is wrong.
* **Grafting the humans' own final sectors** — 65 tapes (13 G1-crossing humans ×
  ±8 ticks, aligned at each donor's own CP5). All 65 DNF.
* **Searching sector 5 on the full map** — structural: the score makes a
  finisher strictly better than any DNF, and with CP5 already crossed before the
  mutation window **every candidate scores identically**. No gradient, only a
  needle. 80 islands neutral-drifting (segment shaping capped at depth 4 so
  equal scores are accepted) for 21 minutes, 207 000 evaluations: nothing.
* **Intermediate finish gates in sector 5** are the correct unlock and I added
  `tmmaps gate` for it — but **gate relocation does not work on this map**.
  The identity probe is decisive: parking the gate at CP1's own position
  (`--at 624,42,991 --cell 19,13,30`) reports **15718**, i.e. CP2's time, not
  7311 — the relocated gate never fires anywhere, whatever cell you give it.
  (`build_segment` already carries a "map 2: it never does" fallback for the
  same mechanism.) The block-rename alternative also fails here: renaming any
  of four sector-5 road blocks to `RoadTechFinish` produced maps on which even
  the reference ghost returns `DNF cps=0` — the lookback table grows and
  downstream indices stop resolving (`tmmaps` warns about exactly this).
  **Making a gate fire mid-sector-5 is the first thing to fix on this map.**

## 7. Method notes that generalise

* **On a 4179-tick chaotic tech map, whole-run search cannot touch the first 33
  seconds.** Two 56-worker arms (analog and keyboard seeds), 10 min each with
  full segment shaping: every accepted operator landed in ticks 3491–3997. The
  cause is structural — the incumbent is a finisher and any upstream change
  DNFs the open-loop tail, which scores far below any finisher.
* **This map's tail cannot absorb ANY upstream change.** A sector-0 tape only
  **29 ms** faster at CP1 already returns `DNF cps=1` on the full map — all 13
  did, and a 0..10-tick tail shift rescued none. Forward chaining therefore pays
  the full cost of re-driving every downstream sector: the sector-1+2 repair
  phase was still **2.1 s worse** than the human after 8 minutes on 168 workers.
  **Chain backward from the finish**, so every round ends with a validated
  finisher and there is always something to report.
* **A segment-map gain is not real until the real map confirms the checkpoint
  depth.** See 8b — this cost 206 phantom milliseconds on this map.
* **The pre-registered roll result does NOT replicate here.** Runs aligned by
  normalised path length within each sector, reference = median roll of the top
  5, phases classified by the reference's own ground contact; 40 runs × 360
  samples: `corr(mean |roll deviation|, final) SURFACE = +0.724, BALLISTIC =
  +0.785`. The prediction is a correlation on surface-transacting ticks and none
  in ballistic ones; here both are strong and the ballistic figure is the
  larger. The charitable reading — and the sector-4 finding is exactly this —
  is that the air phases are short and immediately follow the surface that
  launched them, so "roll deviation in the air" is mostly *takeoff attitude*,
  which is set on a surface. Both figures are confounded with simply being
  slower. Per sector (surface only): 0 +0.53, 1 +0.56, 2 +0.46, 3 +0.43,
  4 +0.44, 5 +0.68. Airborne fraction vs final: −0.16; max speed vs final:
  −0.38.
* **Keyboard-constrained search beat the unconstrained search on this map**
  (40001 vs 40040 at the same point in the session), and the best overall tape
  came from re-seeding an unconstrained arm *from the keyboard result*. Fifth
  map in this project where searching under the human's own alphabet paid.

## 8. Defects found and fixed (all in my tree, sources banked under `tools/`)

### 8a. `tmmaps build` mis-orders the checkpoints, silently
`order_checkpoints` neutralises a candidate gate *plus the already-identified
tail* and takes the largest returned time as "the last checkpoint". When a
candidate's own gate never re-fires, the run finishes at a **tail** gate
instead — several candidates then report the identical tail time, tie, and the
tie-break picks an arbitrary one. Here it produced `633,439,440,494,492`
instead of the true `439,494,440,633,492`, and four of the six segment maps were
duplicates that all returned CP5's time. Nothing warns you.
**Fix: `tmmaps build --order <item indices>`.** With
`--order 439,494,440,633,492` all six segment maps are exact against the
reference ghost: 7311 / 15718 / 19980 / 27834 / 33584 / 40223.

### 8b. A swapped finish gate is not the same trigger as the checkpoint
`build_segment` swaps a checkpoint item's model for `GateFinish32m` in place. On
this map's four `GateCheckpointRight32m` gates that is faithful — tapes
optimised against those segment maps come back `cps = k` from the full map.
**On CP2 it is not.** CP2 (`item#494`) is the map's only
`GateCheckpointLeft32m`. A sector-1 arm scored 15718 → **15512 (−206 ms)**
against seg2, and every one of those tapes returns `DNF cps=1` on the real map:
the line drifts out of the real checkpoint's trigger and keeps scoring against
the wider finish gate. **The reference-ghost identity control cannot catch this,
because the reference line sits inside both volumes.** Mitigations used here:
every phase's output depth-checked on the real map, and sector 1 scored against
**seg3** with CP2 left as a real checkpoint.

### 8c. The hardened build's guard cannot tell a phantom from a failed write
`--bestdir` is never created and the incumbent tape is written with
`let _ = std::fs::write(...)`. With a `--bestdir` that does not exist the guard
validates a file that was never written, gets `None`, declares **PHANTOM
INCUMBENT** and aborts with exit 7. Two 68-worker arms died in 18 s that way
here. The tell is that the "preserved" `PHANTOM_*` specimen does not exist
afterwards. **Fix: create `--bestdir` at startup; treat a write failure as
`ABORT … exit 8`.** (No real phantom occurred in this session; the guard was on
by default for every arm and never fired otherwise.)

### 8d. `fk btraj --allow-dnf` is documented but rejected
`blind.rs` checks for the flag; `state::parse` panics on it as unknown. Fixed.
Workaround without the fix: measure the tape on a segment map where it finishes.

### 8e. Gate relocation is inert on this map
See §6. `tmmaps gate` / `probe`'s `move_gate` writes position, yaw and a 3-byte
cell, and on this map the relocated gate simply never fires — proven by an
identity probe that parks it where it already is. Unfixed.

## 9. Artefacts — `~/persistent/private-30d/tm-unbeaten/146612/`

| file | what |
|---|---|
|  `tapes/BEST_39961_v3.Ghost.Gbx` | best validated lap, **39973**, md5 `a8b4b2a245770922ab2569c61c478e8e` |
| `tapes/KEYBOARD_40001_v2.Ghost.Gbx` | **pure keyboard, 40001**, 117 events, md5 `b7ef5165899d27d607533ad93a6e3f6e` |
| `tapes/KEYBOARD_40058_v1.Ghost.Gbx` | earlier keyboard, 40058, md5 `74439685c4d7e8ec0b931ff15722a44a` |
| `tapes/BEST_40040_v1.Ghost.Gbx` | earlier analog, 40040, md5 `5b5c5d2a029b8fc2c18f3d6c853ebc81` |
| `tapes/JUMP_cp5_32702_v1.Ghost.Gbx` | **the jump**: CP5 32702, `cps=5` on the untouched map, md5 `078edd57d9447bb3f5b15b67c9da4556`. Does not finish. |
| `tapes/*.tick.csv` | per-tick `race_ms,steer,accel,brake` for the two headline tapes |
| `traj/JUMP_cp5_32702_trajectory-v1.csv` | the jump measured with `fk btraj` (self-check clean, validated 32702 on seg5) |
| `traj/human_WR_40223_*`, `human_rank2_40226_kbd_*`, `human_rank13_42768_straightjump_*` | the three comparison trajectories |
| `tolerance_ours_keyboard-v1.txt`, `tolerance_human_rank2_control-v1.txt` | §5's tolerance table and its control |
| `FINDINGS-v1.md`, `PLAN-v1.md`, `splits_all-v1.txt`, `blocks-v1.tsv`, `lbfull-v1.tsv`, `val_all-v1.txt`, `tools/` | evidence and the analysis binaries |

## 10. Next, in order

1. **Make a finish gate fire mid-sector-5.** Nothing else matters until sector 5
   has a gradient. Options: fix `move_gate` (the cell field looks like the
   suspect — three bytes for what may not be a three-byte field), or find a
   block-rename that keeps the lookback table intact.
2. Re-drive sector 5 from `JUMP_cp5_32702_v1`. If it re-drives to anything near
   the field's own 6396, the lap is **≈39100** and two more sectors close the AT.
3. Then chain backward: sector 3 (path/chord 3.36 — look for a second cut),
   then 2, then 1. Sector 1 must be scored against **seg3**, never seg2 (8b).
4. Re-run the low-input ladder on whatever finishes. This map's world record is
   3 ms off a pure keyboard run and our keyboard tape already beats it, so a
   **keyboard tape under the author time** is the realistic end state here, not
   a consolation prize.

---

## Addendum: a lead on the dead gate (§6, §8e)

Map 279197's write-up records that **`move_gate` DELETES the finish unless
`--keep-model` is passed**, and map 197047's records `tmmaps gate --at x,y,z`
successfully manufacturing checkpoints on a map that has none. So gate
relocation is not broken in general — it works elsewhere — and the failure here
is specific. The `--keep-model` finding is the obvious first thing to try:
`segments::move_gate` unconditionally calls `set_item_model(gate.index,
FINISH_GATE)` before moving, and on this map the relocated item then never
fires at all, which is exactly the "gate deleted" symptom 279197 describes.
Start there before suspecting the cell encoding.

---

## Final tape inventory (all validated twice, human controls exact in every batch)

| file | ms | md5 |
|---|---|---|
| `tapes/BEST_39961_v3.Ghost.Gbx` | **39961** | `4388e3e088d5122ee7d1db3d2e6ea4b1` |
| `tapes/KEYBOARD_39996_v3.Ghost.Gbx` | **39996** (pure keyboard, 119 presses) | `80fbd842814b6c0c3f6a021070dc8802` |
| `tapes/BEST_39973_v2.Ghost.Gbx` | 39973 | `a8b4b2a245770922ab2569c61c478e8e` |
| `tapes/KEYBOARD_40001_v2.Ghost.Gbx` | 40001 (keyboard) | `b7ef5165899d27d607533ad93a6e3f6e` |
| `tapes/BEST_40040_v1.Ghost.Gbx` | 40040 | `5b5c5d2a029b8fc2c18f3d6c853ebc81` |
| `tapes/KEYBOARD_40058_v1.Ghost.Gbx` | 40058 (keyboard) | `74439685c4d7e8ec0b931ff15722a44a` |
| `tapes/JUMP_cp5_32702_v1.Ghost.Gbx` | CP5 32702, `cps=5`, does not finish | `078edd57d9447bb3f5b15b67c9da4556` |

**A pure-keyboard lap is under 40 seconds on this map** — 39996 with 119 key
presses and three steering values, 227 ms faster than the analog human world
record and 3 ms faster than our own best analog tape from an hour earlier. The
author time is still 1466 ms away from it.

---

## Addendum 2: the dead gate is a KNOWN cross-map trap (resolved diagnosis)

The project's cross-map index says it outright: **never probe with
`tmmaps gate` / `segments::move_gate` — it swaps the item model first.** On map
285885 that quadruples the trigger volume and fabricates discoveries; on 279197
it deletes a custom Goal item and everything DNFs. The prescribed tools are
**`moveitem` / `ladder`, which abort unless a rebuild at the gate's origin
reproduces the untouched map** — exactly the identity probe that failed here
(park the gate at CP1's own position, get CP2's time). So §8e is not a new
defect on this map; it is the known one, and my `tmmaps gate` subcommand
reproduces it. **Do not use `tmmaps gate` on this map. Port `moveitem`/`ladder`
from the 285885 / 279197 work and use that for the sector-5 gradient.** That is
now the single concrete blocker between the validated jump and a finished lap.

---

# PART 2 — the ladder was ported, and it settled the jump

The fleet's `moveitem` / `ladder` tools (from the 285885 / 197047 work) were
ported into this tree. `ladder` moves ONE item's position only — no model swap —
and refuses to report anything until rebuilding the gate at its own origin
reproduces the untouched map to the millisecond. **On map_seg5, whose CP5 gate
is already a faithful `GateFinish32m`, the origin control PASSES and the ladder
works.** §8e/Addendum 1 stand as diagnosis: the failure was the model swap in
`tmmaps gate`, exactly as the cross-map index warned.

## The sector-5 ladder (23 stations, every 25 m along the WR's own line)

Human WR arrival times, `lad25/st00..st22`: 33.581 · 33.931 · 34.281 · 34.632 ·
34.982 · 35.282 · 35.578 · 35.875 · 36.171 · 36.456 · 36.724 · 37.002 · 37.301 ·
37.588 · 37.958 · 38.245 · 38.495 · 38.765 · 39.072 · 39.396 · 39.674 · 39.798 ·
40.178 s. Stations in `ladder_stations25-v1.txt`.

This is exactly the gradient sector 5 did not have. It let a staircase crawl
climb **13 rungs** from the jump's checkpoint-5 state — a state from which plain
search had produced 0 finishers in 207 000 evaluations.

## The decisive measurement: where the jump's advantage dies

| station | z | human WR | jump tape | crawl tape | re-aimed jump |
|---|---|---|---|---|---|
| st01 | 762 | 33.931 | **33.272** | **33.292** | **33.303** |
| st02 | 790 | 34.281 | — | 34.376 | 34.123 |
| st03 | 817 | 34.632 | — | 35.393 | 34.909 |
| st04 | 845 | 34.982 | — | 36.214 | — |

**The jump is 0.639 s ahead of the world record 26 m past the checkpoint, and
the whole advantage is gone 55 m later.** Cause, measured from the crawl tape's
own trajectory (`traj/CRAWL_st4_trajectory-v1.csv`): between 33.3 s and 34.2 s
the car goes from **74.5 m/s to 22.6 m/s** at (1179, 44, 760–785). It lands at
(1100, 33, 688) travelling **across** the sector-5 road, not along it, reaches
CP5 from the inside with its velocity pointing at the outside wall, hits it, and
never recovers: by st04 it is doing 38.6 m/s where the world record is doing
89.9.

So the honest statement of the finding is sharper than Part 1's:

> **The angled jump genuinely saves 1.128 s to checkpoint 5 and still holds
> 0.639 s of it 26 m later — and then the landing geometry gives it all back.
> The jump is real; the exit from it is not solved.**

Two 22-minute re-aim searches (84 workers each) with the objective moved PAST
the checkpoint — optimise arrival at st03 and at st06, mutating the jump itself
(ticks 2740–3600) — did not find a landing that both keeps the gain and exits
along the road: st03 improved to 34.909 (0.277 s *behind* the WR there, but a
much cleaner state than the crawl's 35.393), and the st06 arm never reached its
station at all. That is the open problem, and it is now a well-posed one: **find
a launch that lands aligned with the +z run of sector 5 rather than across it.**
The trade is three-way — takeoff angle vs landing point vs exit heading — and
21° is optimal only for *reaching* CP5.

## Crawl station-by-station (for whoever picks this up)

st02 −0.501 · st03 −0.231 · **st04 +1.232** · st05 +1.202 · st06 +1.416 ·
st07 +1.589 · st08 +1.728 · st09 +1.779 · st10 +1.891 · st11 +2.026 ·
st12 +2.161 · st13 +2.304 · st14 +2.601 (vs the human WR at each station), then
stalled at st15. The single event is st03→st04: 1.813 s for 28 m of track, i.e.
the wall contact above. Everything after it is the car nursing a dead run.
`tapes/CRAWL_st14_40559_v1.Ghost.Gbx` is that tape;
`tapes/AIM3_st3_34909.Ghost.Gbx` (md5 `befe4203bdfc9d8a1df119d592e67ae6`) is the
cleaner re-aimed one and is the better seed to continue from.

## Method notes added in Part 2

* **A ladder makes a plateau searchable.** Same map, same seed, same search: 0
  finishers in 207 000 evaluations without it; 13 stations climbed with it. If a
  search reports a flat landscape, the objective is more likely missing than the
  landscape flat.
* **A greedy per-station crawl locks in its own accidents.** The st03→st04
  collapse was never revisited and every later station inherited it. A crawl
  should keep the best *k* tapes per station, or re-run a station whose delta
  jumps, rather than taking one winner.
* **Optimise arrival PAST the checkpoint, not at it.** "Fastest to CP5" bought a
  state that cannot use its own speed. The ladder makes the better objective —
  fastest to a station 50–100 m *beyond* — just as cheap to evaluate.
* Confirmed for the fleet notice: with a finishing incumbent, shaping is inert;
  here the incumbent did NOT finish, shaping was live, and the finish rate from
  the CP5 state was still **0 %** over 207 000 evaluations. So "shaping is live"
  is necessary, not sufficient — the DNF basin here is many mutations deep, and
  only a nearer objective (the ladder) crossed it.
