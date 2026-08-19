# 284238 `YOU LOVE WATER` — why this author time is hard, measured

`state_RESULT_v3_final.md`. The closing account. Supersedes nothing:
`state_RESULT_v1_launch_state.md`, `state_RESULT_v2_closing_account.md` and
addenda v1–v10 stand as written and carry the enumerations. Times in seconds.

Controls, plain oracle, untouched maps, every batch this session: record
**440.238**, `best_97325` **97.325**, Yhomas 46.112 on sibling 279008 — all
exact. **No time gain is claimed; the best validated tape on this map is
97.325.** **A `cps` number from a rung map is not a time.** Nothing was submitted
to any leaderboard.

---

## 1. The answer, in five lines

* The route that beats this author time **exists and is 3.0 s better than it**:
  a human's line on the byte-identical sibling map, carried onto ours with the
  launcher penalty paid, is **47.4 against an AT of 50.459**.
* That line needs each copy's launch entered from **a long flat run-up**.
* **Only copy 0 has one.** Its launcher is the start platform and ~100 m of flat
  deck.
* Copies 1–3 are fed by the tube, which — by construction of the map's screw
  symmetry — is **the only connection between one copy and the next**. They
  arrive on the lane 100 m late, with 0.6 s of flat before the kicker.
* In 0.6 s the car cannot build the lateral velocity the wall contact needs, and
  **that, not speed, energy, grip or the boost pads, is the map.**

## 2. The one number the map turns on

The launch is kicker → flight → wall curve → checkpoint, and it is decided by the
canonical z at which the car meets the wall curve:

| | at the wall's height (y 1918) | one-tick loss | checkpoint |
|---|---|---|---|
| our record, cycle 1 | x 980.2, **z 923.4**, v 77.4 | **8.71** | **45.80** |
| our standing start (works) | 969.7, **915.4**, 73.1 | — | — |
| Yhomas 46.112, all four copies | 980.2, **913.9**, 80.8 | **0.75** | **69.40** |

**9.5 metres.** Everything downstream follows: the 1630-vs-311 energy loss, the
crossing-speed decay 52.8 → 45.8 → 41.1 → 37.4, whether the next cycle clears
the 71 m gap (≥300 km/h at the lip; the record manages 299 and 302 only from the
standing start, and 255 at best in 23 other approaches), and therefore the 8.7×
record.

## 3. The mechanism, and the bound

z at contact is set by **lateral velocity accumulated on the flat before the
kicker**, and that is bounded by (time on the flat) × (lateral acceleration
available):

| | time on the flat | vz achieved |
|---|---|---|
| copy 0 (start platform, ~100 m of deck) | ~2 s | **−17.9** |
| copies 1–3 (tube → descending arc → lane) | ~0.6 s | −1.9 (record) … **−15.7** (full lock, the most that fits) |
| Yhomas, all four copies (tech-block launchers) | flat run-up in each | **−24 … −25** |

Three things it is **not**, each measured rather than argued:

* **not grip** — full lock buys **13.4 m/s** on our water lane and **13.2** on
  his tech lane;
* **not speed** — the kicker is crossed at 97.2 (ours, fails), 99.1 (his, works),
  90.9 (our standing start, works); and copy 0 is **not slow at the lane
  either**, reaching 90.7 m/s there;
* **not the six extra boost pads** — they sit on the flat *after* the aim is
  decided, restoring speed the arc lost, one second too late to affect where the
  car is pointing.

## 4. The positive result at the centre of it

**Our own standing start flies Yhomas's launch to within 2–7 metres, point for
point, in order:** R1 2.78 m, R2 2.20, R3 4.06, R4 5.93, R5 6.78, measured per
tick on the untouched map (`state_ADDENDUM_v7`). The target line is not exotic,
is not beyond this car, and is **in our own record at 4.2–5.2 s**. What the map
withholds is the approach that produces it.

## 5. Every lever, and how each closed

| lever | enumeration | outcome |
|---|---|---|
| lane steer, one window | 60+ variants, starts 2251–2331, ends 2321–2356, ±10…±127 | one locus, `z = 923.4 − 0.224(980.2 − x)`; target 9.5 m below it |
| two-window pulse + counter-pulse | 36 | every one destroys the run |
| throttle restoration | 6 windows, 2060–2240 | lane speed 100.9–109.7, lateral still +0.1…+2.9, misses by 107–129 m |
| arc steer (phase grid) | 78 | the **trilemma**: peak height ≥928, peak x ≤880, peak speed ≥95 — any two, never three |
| arc steer + throttle | 6500 evals, 2 seeds, seeded on his trade curve | z_peak tops out at **922** inside the CP2-collecting basin; all winners `DNF cps=2` |
| steer + brake (scrub) | 16 | contact height bought by spending speed: best is 37.9 m/s at CP2 against the baseline's 45.8 |
| per-copy entry (geometry) | placement data, no search | the tube **is** the connection between copies; no copy 1–3 entry avoids it |
| respawn delivery | 31 presses, 4 measured per tick | restores the **crossing** state at full speed; freeze exactly **1.010 s** |
| slow arrival (brake) | 12 windows, two families | monotonically worse — and it corrected the hypothesis: copy 0 is slow on the **deck**, not at the lane |
| previous cycle's exit | 20 | 5 inert (already at full lock), 15 destroy the run, 0 improve |
| **CP2-free rung ladder march** | 10 000 evals at rtol 8 / 4 / 2 m | **the experiment built to break this account, and it did not**: at discriminating tolerance the march stalls at depth 3 and depth 1 |

## 6. What would change the answer

**A launcher that gives copies 1–3 a flat run-up.** That is not a hypothetical:
it is the substitution the author himself made in his own remix. Sibling map
**279008 "Keep dropping"** shares **167 of 186 block records byte-identical**
with 284238 — same names, same absolute positions, same angles, the four
checkpoint gates at identical coordinates — with the water ramps replaced by tech
blocks in **all four copies** and 284238's six extra boost pads absent. On that
map **Yhomas_TM holds 46.112 and beats its author time**; on this one, nobody has
beaten 50.459.

**Falsifiable form:** any tape entering a copy 1–3 launch out of the tube meets
the wall above canonical z ≈ 920 and loses the cycle, because the lateral
velocity obtainable in 0.6 s at ~96 m/s is bounded below what the contact needs.
Every measurement above is consistent with it, including the two that looked
most like counterexamples — the arc *can* reach the target crossing height (and
arrives at the wrong x with the wrong speed), and the yaw *is* available on our
lane (and 0.6 s is not enough of it).

## 7. Instruments this map produced, all banked

* **`state_fk_locate2_v1.tgz`** — the per-tick trajectory reader, fixed and made
  general. Three defects, none map-specific; the whole 440.8 s run now reads in
  3.3 s at **median 0.0075 m** against ground truth, where it previously aborted.
  Notice: `FLEET_NOTICE_trajectory_reader_clock_first_v1.md`. Adds `fk btraj2`,
  `fk traj2`, `fk sweep`, `fk arc`.
* **`state_tmsearch_bestdir_fix_v1.tgz`** — the phantom epidemic on this map was
  a **missing `--bestdir`**: the banking write was silent on failure, so the
  guard validated a file that was never written and accused every candidate.
  Notice v2 carries the two-edit local fix.
* **`state_ladder_v2.tgz`** — 14 calibrated rung maps on the successful branch
  plus 3 negative-control tapes.
* **`state_align_v1.tgz`** — three per-tick canonical trajectories (Yhomas, our
  cycle 1, our standing start), re-simulations rather than decodes.
* **The tolerance rule** — `FLEET_NOTICE_a_rungs_TOLERANCE_must_be_smaller_than_the_effect_v1.md`.
  An 8 m ladder on a 9.5 m effect reported depth 7 of 7 with the winner on the
  wrong branch. Calibrate a detector against what you want to **exclude**.

## 8. If someone picks this map up again

The two things I would do, in order, and neither is a variant of anything above:

1. **Give arm 3's closed-loop policy the copy 1–3 problem.** It tracks the
   sibling human's line on copy 0 to 0.02 m median lateral error and crosses CP1
   at 64.4 m/s. The open question is whether a controller can exceed **vz −15.7**
   inside copies 1–3's 0.6 s of flat. My ladder adjudicates that claim; check it
   at ≤3 m tolerance.
2. **Re-run one representative family with `fk pol strip`.** `ghost::Factory`
   cannot see or remove a respawn, so a whole-tape search inherits a frozen retry
   schedule. It does not affect anything in this account — every launch family
   perturbs a 0.4–4.0 s window inside cycle 1, between the record's respawns at
   11.040 and 51.780 — but it is the right caveat to carry into a whole-tape
   search.
