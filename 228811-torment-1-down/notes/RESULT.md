# 228811 "Fall 2024 - 08 Torment (1-DOWN)" — the author time is beaten

**AT 20555 · human WR 22637 · best validated this session 20237 ms.**
**318 ms inside the author time, 2400 ms inside the human world record.**

This supersedes the endgame sections of `RESULT.md` (which ended at 22575, 62 ms
inside the human WR and 2020 ms outside the AT) and **corrects its account of
the launcher** in three places. Everything in `RESULT.md` §1–§2 — the 48/48
field reproduction, the decoded author ghost — stands unchanged and was the
foundation for all of this.

**Nothing here has been or will be submitted to a Nadeo leaderboard.**

---

## 1. The claim, and its controls

| tape | validated | vs AT 20555 | vs human WR 22637 |
|---|---|---|---|
| `v2_best_20237` | **20237** | **−318** | −2400 |
| `v2_best_20250` | 20250 | −305 | −2387 |
| `v4_best_20260` / `v3_best_20263` | 20260 / 20263 | −295 / −292 | −2377 / −2374 |
| `v3_best_20267` / `v4_best_20267` | 20267 | −288 | −2370 |
| `v1_best_20273` / `u3_best_FIRE_fin20281` | 20273 / 20281 | −282 / −274 | −2364 / −2356 |
| `u3_best_FIRE_fin20295` | 20295 | −260 | −2342 |
| `v1_best_20308` | 20308 | −247 | −2329 |
| `u3_best_FIRE_fin20343` | 20343 | −212 | −2294 |
| `u3_best_FIRE_fin20357` | 20357 | −198 | −2280 |
| `v2_best_20442` / `v2_best_20446` | 20442 / 20446 | −113 / −109 | −2195 / −2191 |
| `v1_best_20544` / `v1_best_20545` | 20544 / 20545 | −11 / −10 | −2093 / −2092 |
| `u3_best_FIRE_fin20612` (first finisher under the WR) | 20612 | +57 | −2025 |
| control — human WR | 22637 | — | — |

Every row is a full plain-oracle validation (`tmtas validate` against the
untouched map, simulating the tape from tick 0), run in a fresh process with the
human world record carried in the same batch as a known-answer control. **The
control returned 22637 in every batch. Zero phantoms, zero failed
re-validations, no incident to file.** Eighteen tapes were validated across four batches; all eighteen reproduced
their claimed millisecond exactly.

The `v*` tapes additionally passed the fork search's own phantom guard, which
re-validates every banked incumbent as it is banked (`grep -c PHANTOM` on those
logs: 0).

---

## 2. What the launcher actually is

`RESULT.md` had it as *"a launcher at (71, 50, 710) that fires the car
323 → 750 km/h, guarded by velocity direction, which zero of 71 recorded human
runs have ever touched."* The first half is right. The rest needed correcting,
and the corrections are what made it reachable.

### 2.1 It is a boost deck that every single run drives across

From the map file (`tmmaps blocks`, new in this session):

```
PlatformTechSpecialBoost   cell (1,14,22) (1,14,23)   x 32..64,  y 50, z 704..768
PlatformTechSpecialTurbo   cell (2,14,22) (2,14,23)
                           cell (3,14,22) (3,14,23)   x 64..128, y 50, z 704..768
```

The floor at the base of the end wall is 96 m of boost platform. All 48 human
runs cross the **Boost** platform's leading edge at x ≈ 63 and pick up an
ordinary turbo there (measured from the `turbo_time` column of all 48 decoded
ghosts: every one of them starts its third turbo episode between x = 61.8 and
x = 63.1). They drive the length of the **Turbo** platform first, every lap, and
nothing happens.

### 2.2 The launcher is a LINE at z ≈ 709, and it spans at least 80 m of x

**1343 launches were produced and logged this session.** Their positions:

| | |
|---|---|
| x | **56.1 … 135.9** (spread over 80 m) |
| z | **705.3 … 709.5**, and 858 of the first 931 sit at z = 709 exactly, 71 at z = 708 |
| largest one-tick speed rise | 5007 m/s (a solver blow-up; the useful ones are 20–130 m/s) |

So it is not a spot a few metres to the side of the racing line. It is a line
running the length of the deck, about a metre wide in z, roughly 6–13 m to the
−z side of where the field drives. The author's contact at (70.2, 50.4, 708.9)
is one point on it; the search's launches are strung out along it.

### 2.3 Position and velocity are NOT the trigger — attitude is

**And one human on the leaderboard is already doing the scrub.** Rank 11
(26.715) crosses x = 80 with **87.6 m/s of body-lateral speed** at 331 km/h --
the author's own signature, and by a distance the largest on the board -- is
still at 81.7 m/s at x = 65, z = 711.8, and then puts the car into the end wall
at 12 km/h. He crosses the line at x ~ 63 with his side speed decaying. He is
not close by accident: he is doing the move, and he is a couple of metres of x
and a few m/s short. Every other record is under 20 m/s of side speed anywhere
near the line, and **0 of 48 reach the z 704-713 band at floor level with >= 85
m/s of side speed and a downward crossing.**

This is the correction that mattered. `RESULT.md` concluded the trigger was
velocity *direction*, from a spliced tape that passed within a metre at
396 km/h and did not fire. That experiment was right and the conclusion was too
narrow. Measured this session, from the search's own candidates:

| candidate | at | body-lateral speed | −vz | fires? |
|---|---|---|---|---|
| author | (71.4, 50.4, 710.3) | **86.8** | 69.8 | **yes** |
| search, mode 1 | (66.2, 50.1, 706.3) | 0.7 | **100.3** | no |
| search, mode 3 | (87.8, 50.4, 713.1) | **102.5** | ~0 | no |
| search, mode 9 | (77.6, 50.0, 711.2) | **97.5** | **30.9** | no |
| **search, mode 10** | **(71.4, 50.1, 710.3)** | 20.3 | **67.8** | **no** |
| search (fires) | (70.5, 50.8, 709.0) | 94.5 | 43.5 | **yes** |

The fifth row is the sharp one. **A tape that reaches the author's contact
point to within 0.3 m, with a velocity within 3 m/s of his, does not fire the
launcher** — because its nose is pointed along its direction of travel instead
of 56° across it. Position is not the trigger, velocity is not the trigger,
and position *and* velocity together are not the trigger.

What is required, measured over 1343 launches and many thousands of near
misses:

> **cross the z ≈ 709 line, downwards, at deck level, with the car's body
> lateral to its own motion — 85 m/s or more of body-frame side speed.**

Sliding *along* the line at 102 m/s of side speed does nothing. Crossing it at
100 m/s pointing where you are going does nothing. Both together fire it.

### 2.4 What the launch does

The boost converts body-lateral speed into forward speed, along the car's own
forward axis, and adds a great deal of energy doing it:

| | before contact | one tick after |
|---|---|---|
| author, body frame | right **+86.8**, up −5.1, fwd 22.6 | right −26.4, up 8.4, **fwd 206.7** |
| speed | 89.8 m/s (323 km/h) | 208.5 m/s (751 km/h) |

The resulting *direction* is the car's forward axis, which is why the author's
nose attitude at contact is the whole game: he is pointing +x and 27° up, so he
is fired down the track at the finish. The search's first launches pointed the
nose along −z and were fired straight up — one reached 270 m/s (974 km/h) and
climbed to y = 288 m, which is spectacular and completely useless.

The body-lateral figure is the ghost format's own `side_speed` field: computed
from the quaternion it agrees with the recorded value to better than 0.2 m/s at
90 m/s, checked at four instants of the author's lap.

### 2.5 The launch must happen AFTER the last checkpoint, and that is the whole difficulty

The final checkpoint is a 32 m gate at **x = 80**, its posts at
(80, 50, 720) and (80, 50, 752). A launch upstream of it flies beautifully —
this session produced tapes that fire at x = 112 and pass **within 0.8 m of the
finish** — and every one of them is worthless: `tmtas validate` returns
**DNF, 5 of 6 checkpoints**. The run skipped the gate.

So the technique is not "hit the launcher". It is:

> pass the checkpoint gate at x = 80 (which needs z ≳ 718 there), and then,
> within the next ten metres of x, get down to z ≈ 709 and across the line with
> the car sideways at 85+ m/s.

The author threads exactly that: x = 80 at z = 718.7, contact at
(70.2, 50.4, 708.9). It is a ten-metre window between a checkpoint you must not
miss and a line you must cross sideways, and that — not the launcher's
obscurity — is why nobody has it.

---

## 3. What a driver actually does

The author's own recovered script (`RESULT.md` §4) is still the executable
version of this: 37 input events, six steer values, and one held input for the
last 2.4 seconds. In words, and now with the geometry behind it:

> Come down the last drop wide and **keep the car turned across its direction
> of travel** — you want it sliding, not pointing. Cross the last checkpoint
> gate at its near edge. Then, instead of running on to the wall, hold **full
> right lock with throttle and brake together** and let the car scrub left
> along the deck. About ten metres past the checkpoint, still sliding almost
> perfectly sideways at ~320 km/h, the car crosses an invisible line in the
> floor and is fired back down the track at 700–950 km/h. Keep the lock on: the
> reactor's thrust holds the nose up and you glide from y = 53 to the finish
> without touching the wall at all.

**Tolerance — and it is brutal.** Measured on this session's own 20317 ms tape
by shifting the entire input stream from race 17.45 s onwards by whole ticks:

| shift | result |
|---|---|
| −80 ms … −10 ms | DNF (6 of 6 checkpoints), **launcher does not fire at all** |
| **0** | **20317 ms** |
| +10 ms … +80 ms | DNF (6 of 6 checkpoints), **launcher does not fire at all** |

Ten milliseconds either way and there is no launch — not a worse launch, no
launch. The optimised TAS line has **zero ticks of slack**. Two caveats, both
honest: this measures *this* tape, which is a machine-optimised line with 697
input events and every tick load-bearing; and the author's own lap is a 37-event
script on six steer values, which is coarse enough that his particular line may
sit in a more forgiving pocket than mine. His tolerance cannot be measured,
because a TM2020 validation ghost stores a state record and not an input tape,
so his run can be read but not replayed (`RESULT.md` §2).

---

## 4. How it was found: score the STATE, not the time

`RESULT.md` §5 called it exactly right — *"the right objective is not arrival at
a relocated gate, it is a continuous property of the car's velocity at a place,
which is smooth and monotone and something a hill-climb can follow across the
valley, unlike finish time"* — and named the piece of tooling needed. That was
built, and then rebuilt four times, because the first three objectives were all
satisfiable without firing anything.

### 4.1 The tooling

`Summary` in `shared/pred_core.rs` (the struct the fork child fills in and the
parent reads out of a shared page) grew from 56 to 148 bytes:

| field | what it is for |
|---|---|
| `cross_vx/vy/vz` | velocity at the sub-tick timing plane |
| `gate_key`, `gate_tick`, `gate_x/y/z`, `gate_vx/vy/vz`, `gate_speed` | the best value of a scored quantity inside an armed box, and the full state where it happened |
| `gate_side`, `gate_fwd` | **body-frame** lateral and forward speed there — the quantity this map's booster converts |
| `gate_miss` | closest approach to the box, so the objective is continuous outside it |
| `max_jump`, `jump_tick`, `jump_x/y/z`, `jump_speed` | the largest ONE-TICK speed rise and where it happened: the launch detector |
| `max_x_post` | furthest x reached after a launch |
| `fin_dist` | closest approach to the finish, after a launch |

Ten gate modes were tried; the ones that matter are **3** (body-lateral speed),
**9** (`min(|side|, 5·−vz)`, the measured firing conjunction) and **10** (signed
distance to a complete 6-D target state: position, velocity and nose).

Scoring is three non-overlapping bands, compared lexicographically, so a search
hunting a state can never trade one for another:
`key` (did not fire) < `fired, ranked by how close to the finish` <
`fired and finished, ranked by time`. All bands sit far below `FINISH_BASE/2`,
so the phantom guard stays out of the way while the search is hunting a state
rather than a time — which is exactly why every tape it produced was
re-validated by hand before it was believed.

`FINISH_BASE` was raised 1e8 → 1e12 in `forksearch.rs`, `main.rs` and
`bin/tmtas.rs` as the brief instructed; with 6 checkpoints on this map the old
value would have let a deep DNF outrank a real finish.

### 4.2 The identity control, which is better than the one it replaced

In gate mode the seed is normally aborted by a predicate before it finishes, so
"re-ran at the same millisecond" is not available. The replacement is stronger:
**the fork's measured gate state is checked against the seed's own decoded
telemetry.**

```
human WR, gate mode 10, computed offline from its decoded ghost : -36.1224
the same quantity, measured inside the fork server              : -35.9890
```

One number that validates the record layout, the position, the velocity **and
the quaternion** handling in the child, against the ghost's own recording. It
was armed on every arm for the rest of the session and never failed.

### 4.3 Four ways an objective can be satisfied without doing what you want

Recorded because each one cost time and each looked like progress:

1. **`−vz` alone** ran the car to the corner of the gate box at 99 m/s of
   crossing speed with the nose pointed along the motion. Nothing fires.
2. **Body-lateral speed alone** reached 102 m/s — more than the author's 86.8 —
   sliding *along* the launcher line instead of across it. Nothing fires.
3. **Progress along the author's line**, used to rank candidates that had
   fired, plateaued dead at 86.9%: a launch fired at the sky leaves the
   reference corridor immediately, and so does every neighbour of it.
4. **Furthest x reached after the launch** rewards a launch fired 250 m
   straight up exactly as much as one fired 250 m down the track. Replaced by
   closest approach to the finish, measured *only after a real launch* — the
   ordinary approach passes within 99 m of the finish on its way down the
   track, and measuring from tick 0 pins every candidate at 99 m.

### 4.4 Three bugs that stalled the search silently

1. **Peak speed is not a launch detector.** The human world record itself
   reaches 151 m/s at the finish, so a 160 m/s threshold catches ordinary runs
   and misses nothing useful. A **one-tick speed rise of ≥ 10 m/s** has exactly
   one cause on this map: ordinary driving gains ~2 m/s per tick and a flight
   gains 0.1.
2. **A near miss outscored an arrival.** The continuous extension below the
   gate box was `-miss`, and for a mode whose in-box key is itself large and
   negative (mode 10 scores −36 for the human line), grazing the boundary at
   −0.001 beat every candidate that got inside. The search sat on the edge of
   the gate for 100 000 evaluations perfecting a miss. Non-arrivals now live
   below −500, always.
3. **The identity control was testing a moving target.** Workers start
   staggered, and a worker that starts late reads a `best` another worker has
   already improved; controlling on that compares this server's answer against
   the *seed's* expected value for a tape that is no longer the seed. About
   half the fleet was aborting on a control that was testing the wrong tape.
   The control now always runs on the seed.

All three are fixed in the banked build.

### 4.5 The path, for the record

| stage | objective | got to |
|---|---|---|
| human-route local search (previous session, and three arms still running at the start of this one) | finish time | 22575 |
| gate, `−vz`, wide box | state | gamed the corner, no launch |
| gate, body-lateral speed, tight box on the deck | state | first launch, 168 m/s, fired at the sky |
| + launch detector, + "furthest x" | state → distance | launches to x = 380, all DNF at 5 of 6 checkpoints |
| + launch must be at x ≤ 80 (after the checkpoint) | state → distance | first launch downstream of the gate |
| + closest approach to the finish | state → finish | 30 m, then 17.9 m, then **first finishers at 20627 / 20625** |
| plain finish-time fork search seeded from those, phantom guard ON | time | **20237** |

The first validated finisher on this route came 2 h 43 min after the session
started, and the author time fell 4 minutes later.

---

## 5. What this says about the target list

`RESULT.md` §6 recommended running `tmtraj decode <map>.Map.Gbx` on every
unbeaten target before anything else. That stands and is reinforced: the decoded
author ghost is what made every measurement in this document possible —
it supplied the contact state that became the search's target, and the
known-answer control that validated the instrument.

Add to it: **`tmmaps blocks --near X,Y,Z` on the region the author's ghost does
something inexplicable in.** One command turned "a launcher nobody has visited"
into "96 m of boost platform that everybody drives across", which is what
reframed the whole problem. The new `tmmaps items` / `blocks` / `free`
subcommands are in the banked build.

And the general lesson, which cost most of the session: **an objective that
targets a *state* has to target the whole state.** Position, velocity, and
attitude — this map's launcher ignores the first two and needs the third, and
every partial objective was satisfiable by a car doing the wrong thing at high
speed in the right place.

## 6. The 1-UP sibling, 228607

The same feature is on 228607 (author fires 339.7 → 768.9 km/h, zero of 23
records touch it). Everything in §2 should transfer: look for
`PlatformTechSpecialTurbo` blocks at the wall base, find the launcher line's z,
and check where its last checkpoint gate is — on this map that checkpoint is
the entire difficulty, and it is the reason a launch that flies perfectly can
still be worth nothing.

## 7. Artefacts

`~/persistent/private-30d/tm-unbeaten/228811/`

| file | what |
|---|---|
| `claims-launcher/` | every tape claimed here, all re-validated through the plain oracle |
| `BEST.Ghost.Gbx` | the best validated tape, 20237 ms |
| `RESULT.md` | the previous session's result (22575) — §1–§4 still stand |
| `RESULT-AT-BEATEN.md` | this document |
| `TECHNIQUE.md` | the driver-facing write-up of the move, its tolerance, and rank 11 |
| `at_ghost.csv` | the author's decoded validation lap |
| `tolerance.txt` | the ±80 ms shift sweep |
| `validate-launcher.txt` | the validation batches, with the human-WR control |
| `tmtas-rs-228811-gate.tgz`, `fk-228811-gate.tgz` | the build: gate objective, launch detector, `tmmaps blocks/items/free` |
