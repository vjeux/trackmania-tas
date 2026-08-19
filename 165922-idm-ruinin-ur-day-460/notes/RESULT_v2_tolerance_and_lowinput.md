# 165922 — vj4 results v2 (supersedes `vj4_RESULTS_165922_v1.md`)

Agent vj4, 2026-08-19, node 64455.od.fbinfra.net. **v1 is superseded, not
deleted.** What changed: one instrument reading in v1's §2 was wrong, my own
tool caused it, and correcting it changes the tolerance conclusion. Everything
else in v1 stands and is repeated here.

## 0. The correction, first

v1 said: *"0 of 52 shifted variants reach the y = 1800 chute net"* — i.e. a
one-tick error kills the car in the start chute. **That is wrong. 52 of 52 clear
the chute.** The reading came from a defect in my own `vj4tol sweep`: it named
each oracle worker `w{:03}` and `oracle::Worker::new` creates the
`UserData/Maps` symlink **only if it is missing**, so a loop that sweeps the same
tape against several maps under one `--root` silently validates every map after
the first **against the first map**. Fixed by putting the map name in the worker
tag; every instrument reading below is from the fixed tool, and the fleet notice
carries the trap.

The corrected localisation, with the map-per-worker fix in place — where does a
one-tick-shifted variant of our 15.224 tape actually die?

| instrument | what it means | shifted variants that reach it |
|---|---|---|
| `chute` net, y = 1800 | got down the start chute (~3.2 s) | **52 / 52** |
| `vj4_air900`, x = 900 curtain | left the ramp and is flying (~6.5 s) | **2 / 52** |
| `vj4_air1400`, x = 1400 curtain | mid-glide (~9.4 s) | 0 / 52 |
| `vj4_curtWIDE`, x = 2300, z 400–1072 | arrived at the pad, low | 0 / 52 |
| the real map | finished | 0 / 52 |

**A 10 ms shift anywhere in the first 3 s does not miss the landing and does not
crash in the chute — it crashes on the booster ramp**, between the chute exit
and x = 900.

## 1. Times (all on the untouched map, human record 8790.769 in the same batch)

| tape | time | what it is |
|---|---|---|
| v3 arm's banked beater | 15.549 | re-validated by me on a second node |
| **`vj4_best_15217`** | **15.217** | best; keyboard steering from race 4.56 s onward |
| `vj4_best_15224` | 15.224 | pure-analog champion |
| `vj4_clean_15230` | 15.230 | first improvement, banked early |
| `vj4_keyboard_15285` | 15.285 | keyboard from race 2.56 s; **70 input events total** |
| `vj4_padfar_15382` | 15.382 | forced to land deep in the pad |
| `vj4_kb310_16276` | 16.276 | keyboard from race 1.56 s — over the AT, kept for §3 |

Author time 15.643, so the margin is **0.426 s**.

## 2. Tolerance — measured, localised, and now with the key comparison

`vj4tol sweep` moves each input-change boundary one tick (10 ms) earlier and one
tick later and re-simulates every variant.

**Whole race window, the 15.224 tape: 1338 shifts, 1261 survive — 94.2 %.** By
region, and the same shape on all four of our tapes:

| window | shifts (15.224) | survive | pooled over 4 tapes |
|---|---|---|---|
| race 0.00–2.96 s | 52 | **0 %** | 0 of 164 |
| race 2.96–3.96 s | 30 | 30 % | 41 of 104 |
| race 3.96–4.96 s | 54 | 93 % | 173 of 174 |
| after race 4.96 s | 1202 | **100 %** | 1346 of 1346 |

Further perturbation families on the fastest tape, race 0.00–2.96 s:

| perturbation | tested | survive |
|---|---|---|
| two boundaries, every pair and direction | **1300** | **0** |
| one tick, steering ±1 unit (1/127 of full lock) | 352 | 45 (12.8 %) |

(The same one-unit nudge over race 2.96–4.96 s survives 97.7 %, and over
4.96–7.46 s, 500 of 500.)

### The comparison that matters: the human's launch IS tolerant

The same instrument, run on the **human record's own winning attempt** (record
packets 877346–877700, its launch), validating the whole 8790.769 s record each
time:

> **42 boundary shifts tested, 17 survive — 40.5 %.**
> (And three of them are faster: one shift returns 8787.643, i.e. their final
> attempt 3.1 s quicker.)

So a launch program with real one-tick tolerance **exists on this map** — the
human's — and every tape our search produced has none. Tolerance is also
partially recoverable by coarsening: the keyboard-from-1.56 s tape (16.276, over
the AT) is **10.3 % tolerant** over race 0–0.96 s where all our AT-beaters are
0 %. Tolerance here costs time, and we have three points on that curve:
15.2 s → 0 %, 16.3 s → 10 %, the human's (18.8 s attempt) → 40 %.

### The two searches for tolerance, and why they failed

* **Tolerance as the objective** (`vj4tol search3`): pass/fail has no gradient
  (everything scores 0), so failures are graded — 3 finished, 2 reached the pad's
  near edge, 1 cleared the chute, 0 died — and **every** boundary shift is
  evaluated deterministically (53 simulations per candidate). An earlier sampled
  version froze at 18/30 in 13 seconds, which is what a noisy objective does to a
  hill climber. Two runs (70 workers × 35 min from the analog tape, 110 workers ×
  14 min from the keyboard tape, ~110k evaluations each) never moved off the
  seed's score of 52/156 — that is, every shifted variant clears the chute and
  none of them ever reaches the pad.
* **Landing deep on purpose**: `vj4_padfar` (the 132 gates re-hung on the far
  half of the pad's own positions, origin control exact) produced a legal
  15.382 that lands 40–80 m deeper. Launch tolerance **unchanged at 0 %**.

**What I would do next, given the human comparison:** seed the tolerance search
from the human's own ramp inputs rather than from any tape of ours, and score
the perturbation a player actually makes — a correlated timing error across the
whole launch, not one boundary at a time. The basin exists; ours is not near it.

## 3. Low input (§0.7.2)

The board's only human is on a keyboard: 94.2 % of the record's 879 231 ticks
have steer ∈ {0, ±127}, throttle held 100 % inside the winning attempt, 102
input events. So `--qlevels 1` is that board's own alphabet.

Straight quantisation DNFs, and a whole-tape constraint from a DNF seed has no
gradient (the quantised tape failed before the map's only checkpoint). The fix
is a **windowed constraint ladder** — `tmsearch --qlo/--qhi` (my patch) applies
the alphabet only inside a window, and the window grows **backward from the
finish** because the chute is the fragile end, so the incumbent is a finisher at
every rung:

| keyboard steering from | result |
|---|---|
| race 13.56 s | 15.224 (free) |
| race 9.56 s | 15.221 |
| race 6.56 s | 15.220 |
| race 4.56 s | **15.217 — the session's best time** |
| race 3.56 s | 15.292 |
| race 2.56 s | **15.285** (43 events after the boundary, 70 in the whole run) |
| race 1.56 s | 16.276 — over the AT |

**The keyboard constraint found time rather than costing it.** And a rung that
reports "no finisher" is not a negative until it is resourced: rung 1.56 s
returned nothing at 2 min / 60 workers and produced a finisher at 8 min / 90.

Other channels: steering forced to zero from race 6.46 s costs 7 ms, from
8.46 s 1 ms, from 4.50 s DNF — **9 of the 15.2 s need no steering at all**.
Throttle forced to `accel=1, brake=0` from race 4.46 s onward is completely free;
over any 1 s window before that, DNF.

## 4. Instruments (position-only relocation, chunk `0x0304305F`, records 34..165)

Origin control: rebuilding the 132 gates at their own positions reproduces
8790769 and 15549 exactly. `vj4_maps/`: `vj4_origin`, `vj4_curtC` (x = 2300,
y −1..159, z 576..928), `vj4_curtF/Zlo/Zhi` (16 m z bands), `vj4_curtWIDE`
(x = 2300, z 400..1072), `vj4_air900`, `vj4_air1400`, `vj4_padfar`,
`vj4_padfar2`, `vj4_exit690` (**does not fire for any tape — do not use**).

Where our line goes: it arrives at the pad's near edge inside z ∈ [752, 928]
(not [560, 736]; the human arrives at z ≈ 690) and **finishes at the same
millisecond it arrives** — no ground crawl, against 3.77 s of crawling in the
human's attempt and 0.463 s in v3's 15.549. It passes x = 900 at 6.554 s and
x = 1400 at 9.388 s.

## 5. Throughput

The two defects in `ACQUISITION_addendum_oracle_throughput_and_respawn_input.md`
were **already fixed in the tape I inherited**: 5263 bytes, declared time 16000
at all five sites (`m165 findu32`). Measured here: 350 cand/s at 60 workers,
600–770 at 70–110, ~70 % finishers. Re-declaring just above the incumbent would
prune ~5 % more and was not worth another artefact set.

## 6. Files

`165922/vj4_*.Ghost.Gbx` (six tapes), `165922/vj4_maps/` (instruments),
`165922/vj4_tools/` (`vj4in`, `vj4patch`, `vj4tol`, the two ladder scripts, and
the `--qlo/--qhi/--gaslo/--gashi` patches), `vj4_VERIFICATION_v1.md`,
`vj4_HUMAN_TECHNIQUE_v3.md` (v1/v2 superseded — they carry the chute error),
and `FLEET_NOTICE_tolerance_and_constraint_ladders_v2.md`.
