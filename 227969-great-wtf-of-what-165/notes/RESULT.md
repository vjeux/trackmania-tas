# Great wtf of what #165 (TMX 227969) — the author time falls, and it falls on a keyboard

**Headline: 8075 ms with 14 input change events, 12 of them steering, using only
the three values `full-left / neutral / full-right`.** That is the same input
device, and essentially the same input budget, as the human world record
(8197 ms, 11 steering events, same three values) — 122 ms faster, and 52 ms
inside an author time that no human has ever beaten.

The unconstrained TAS floor is **7998 ms**. But the keyboard tape is the
deliverable: it is a run a person can be taught in one sentence and practise
tonight.

| tape | time | vs AT 8127 | vs human WR 8197 | steer events | distinct steer values | input device |
|---|---|---|---|---|---|---|
| **keyboard, 14 inputs** | **8075** | **−52** | **−122** | **12** | **3** | keyboard |
| keyboard | 8058 | −69 | −139 | 23 | 3 | keyboard |
| action keys, 8 detents | 8050 | −77 | −147 | 54 | 15 | pad / action keys |
| analog, event-thinned | 8021 | −106 | −176 | 62 | 50 | TAS only |
| analog, unconstrained | **7998** | −129 | −199 | 185 | 111 | TAS only |
| — human WR, Titoch_tm | 8197 | +70 | — | 11 | 3 | keyboard |
| — human #2, Hemiphsphere | 8228 | +101 | +31 | 63 | 55 | pad |

uid `LtSUTxJ71u7ayvLj57wUdVPyH2h` · author FrankTheHamster · 42 recorded runs,
all 42 downloaded and analysed. **Nothing here has been or will be submitted to
a Nadeo leaderboard.**

## Files

| file | what |
|---|---|
| `m165_TAS_keyboard_14inputs_8075ms.Ghost.Gbx` | **the drivable one.** 14 inputs, keyboard alphabet |
| `m165_TAS_keyboard_14inputs_8075ms.tick.txt` | its complete input script — 16 lines, reproduced in §5 |
| `m165_TAS_keyboard_8058ms.*` | same alphabet, 23 steer events, 17 ms faster |
| `m165_TAS_actionkeys8_8050ms.*` | 8 steer detents per side |
| `m165_TAS_analog_7998ms.*` | the unconstrained floor |
| `m165_TAS_analog_thinned_8021ms.*` | the floor with 3x fewer inputs |
| `m165_TAS_8010ms.*` | the first sub-AT analog tape (superseded by 7998) |
| `*_trajectory.csv` | per-tick (10 ms) position / velocity / attitude / inputs |
| `human_WR_8197_Titoch_tm.*` | the reference human run |
| `map165_227969.Map.Gbx` | the map as served by TMX |
| `validation_8010.txt` | raw oracle transcript |

`.tick.txt` files round-trip to their ghost byte-exact (`tmsite verify`).

---

## 1. Correctness — answered separately from difficulty

**Every number here came out of the plain oracle**, `TrackmaniaServer /nodaemon
/validatepath=`, which re-simulates the input bitstream and prints the time the
physics actually produces.

* **164 of 164** tapes written by any search arm this session re-validate to
  exactly the time in their file name, with the human WR as an identity control
  in every batch returning 8197 every time. **Zero phantoms.**
* The headline tapes were additionally re-validated **cold** — fresh throwaway
  directory, fresh server process, against a **re-downloaded byte-identical**
  copy of the map (md5 `e526da79…`): 8010 three times, 7998 twice, 8075, 8102,
  8083, all exact. `"NbRespawns": 0`, `"IsValid": true`, correct `MapUid`.
* Independently re-validated by the coordinating agent on this node.

**Is it a legitimate run of the intended track?** Yes, and this was checked
geometrically because the map declares only one waypoint pair (Spawn +
GateFinish, no intermediate checkpoints), so a checkpoint count proves nothing.

* **Maximum distance from the human world record's own trajectory, over the
  whole run: 2.57 m** (at 7.41 s, inside the final wall-ride). Against all 42
  human runs it stays within 3–13 m. It is the same route.
* At the decisive point (z = 855, entering the kicker) the run's state is
  **inside the 42-run field's range on every axis** — speed 276.0 km/h (field
  245–285), vz 76.1 m/s (field 58–78), pitch 0.51 rad (field 0.22–0.92), roll
  −0.06 rad (field −0.16…+0.65). **Two human runs pass that point faster than we
  do** (p08 at 282.6 km/h, p39 at 284.6). We are not somewhere humans cannot go.
* The map contains exactly **one** collision event — a wall throw at 6.68 s that
  redirects the car through ~100° and costs it 420 → 328 km/h. **All 42 human
  runs take it identically**, and so do we; our post-throw speed (327.8 km/h) is
  the same as the field's best (328.3). We are not exploiting it, we are
  surviving it better afterwards.
* No respawn, no skipped geometry, no out-of-bounds flight.

---

## 2. The map

23 blocks, no intermediate checkpoints, ~8 s. **33% of the run is airborne.**

| phase | race time | what happens |
|---|---|---|
| A. launch pad | 0 → 2.40 s | standing start, ~110 km/h on a short road |
| B. booster | 2.40 → 3.55 s | booster + curved ramp, 110 → 488 km/h while turning right |
| C. the long jump | 3.55 → 5.35 s | **1.81 s airborne, 240 m, ballistic.** Full right lock throughout — that is not steering, it is rotating the car to land square |
| D. the straight | 5.35 → 6.66 s | land at ~455 km/h, brake, coast to ~422 km/h |
| E. **the throw and the wall** | 6.66 → 7.40 s | thrown off a wall (422 → 328 km/h in one tick), arcs ~270° along a curved wall, climbs it |
| F. the kicker + finish flight | 7.40 s → finish | kicked into the air, then pure ballistics |

The finish is a **plane at z ≈ 906.4**, spanning about x ∈ [562, 582] and
y ∈ [44, 54] — a big gate the car flies through. **Nothing after the kicker
changes anything**: the car is ballistic, so the finish time is completely
determined by the state of the car the instant it leaves the kicker at ~7.4 s.
Phase C is identical in every run in the field — same entry (488 km/h), same
exit (455) — so it is worth zero.

---

## 3. Where the time is, and what nobody was doing

Measured along the reference line, in metres of track (total ~680 m). Negative
means the TAS is ahead:

| distance | TAS vs human WR |
|---|---|
| 0 → 520 m | **+10 ms — the TAS is BEHIND** |
| 560 m (entering the throw, 6.5 s) | +21 |
| 580 m | −5 |
| 600 m | −40 |
| 640 m | −100 |
| 660 m | −160 |
| finish | **−187** |

**Zero gain over the first 6.5 s. All of it in the last 1.4 s.** (The prefix of
the analog tape is literally the rank-2 human's own input, unmodified, and it is
10 ms slower than the WR's prefix. There is a little left there; it is not where
the story is.)

### The mechanism, in one table

Speeds are the last tick on the ground before the kicker's impulse, and the
first tick of free flight after it. `vz` is measured at z = 878, well inside the
final flight.

| | speed into the kicker | speed leaving it | **vz toward the finish** | lost |
|---|---|---|---|---|
| human WR | 73.6 m/s (265 km/h) | 61.5 m/s (221 km/h) | **57.3 m/s** | −12.1 |
| human #2 | 76.5 (275) | 58.5 (211) | 49.1 | −18.0 |
| best of all 42 humans (p33, an 8465 run) | — | — | 59.8 | — |
| **TAS (analog, 7998/8010)** | 75.4 (271) | **73.1 (263)** | **68.8** | **−2.3** |
| **TAS (keyboard, 8075)** | 74.2 (267) | 69.9 (252) | **65.9** | −4.3 |

Both launch at essentially the same *angle* (~19° up). The TAS simply does not
pay the kicker. 43 m of gate to cover: at 57.7 m/s that is 750 ms, at 69.2 m/s
it is 625 ms.

### Why the humans lose 12–18 m/s

Attitude at the instant the kicker is hit:

| | pitch | roll | sideways velocity |
|---|---|---|---|
| human WR | **0.99 rad (57° nose-up)** | 0.16 → 0.37 rad, rising to **1.49 rad (85°) in flight** | −4.9 m/s |
| **TAS** | **0.53 rad (30°)** | −0.05 rad (3°) | **−0.35 m/s** |

The whole field climbs the final wall **pinned at full left lock**, which rolls
the car onto its side and pitches the nose up. It reaches the kicker leaning and
crabbing, and the kicker converts a third of the forward speed into nothing. The
TAS reaches the kicker **flat and square**, travelling exactly along the
corridor.

### And the thing that produces the flat attitude is trivially simple

**Human WR through the corner — one long lock:**

```
6.69 s  LEFT ────────────────────────────────── 7.08 s  release   (390 ms)
7.16 s  LEFT ──── 7.20 s release                                   (40 ms)
7.69 s  LEFT (hold)
```

**Keyboard TAS through the corner — the same key, pumped:**

```
6.68 s  LEFT ───────────── 6.90 s release      (220 ms)
6.95 s  LEFT ──── 7.03 s   release             (80 ms)
7.12 s  LEFT ── 7.16 s     release             (40 ms)
7.39 s  LEFT (hold)
```

That is the entire discovery. **The field holds one 390 ms lock where they
should be pumping three short taps, and they commit to the kicker 300 ms too
late.** Everything up to 5.24 s is identical between the two tapes.

### Verdict: UNDISCOVERED

Not "known but unheld" — the human field is not attempting this and losing it,
it is doing something else consistently: all 42 runs hold the lock and all 42
roll over. Not precision-bound either, at least not in its keyboard form: the
technique is four key presses, the ingredients (entry speed, vz) are already
present in the field, and two humans already pass the decisive point faster than
we do. Nobody has tried releasing the lock.

42 recorded attempts is the reason. On a map with 900 attempts this would have
been found.

---

## 4. How hard is it to hit? (tick tolerance)

Two different questions, and only the second one matters to a driver.

**Raw slack** — shift one input by one tick (10 ms) and change nothing else:
almost every input is 1-tick critical. That is a fact about an open-loop tape in
a chaotic simulator, not about the map, and it is the wrong measurement: a
driver who taps 20 ms late and keeps driving is not running our tape shifted,
they are running a different tape.

**Recoverable slack** — mistime one input, then let only the *later* inputs be
re-timed, which is what a driver actually does. Cost in ms relative to 8075;
anything up to +52 still beats the author time:

| input | race | what the driver is doing | −30 ms | −20 | −10 | +10 | +20 | +30 | verdict |
|---|---|---|---|---|---|---|---|---|---|
| brake ON | 5230 | brake in the air, before touchdown | +29 | +26 | +46 | DNF | DNF | DNF | **early is fine, late is fatal** |
| steer 0 | 5240 | straighten for the landing | DNF | +19 | +23 | +35 | +48 | +50 | forgiving both ways |
| LEFT | 6680 | commit into the wall throw | DNF | DNF | DNF | DNF | +88 | DNF | **tight — 10 ms** |
| release | 6900 | end of tap 1 | +49 | +23 | +70 | +23 | +23 | +9 | **forgiving, ±30 ms** |
| LEFT | 6950 | tap 2 | +70 | +118 | +10 | +157 | +27 | DNF | mixed |
| release | 7030 | end of tap 2 | DNF | DNF | +12 | +23 | +16 | +22 | forgiving late |
| LEFT | 7120 | tap 3 | +46 | +100 | +116 | DNF | DNF | DNF | **tight** |
| release | 7160 | end of tap 3 | DNF | DNF | DNF | +67 | +85 | DNF | **tight — 10 ms** |
| LEFT | 7390 | commit to the kicker | +76 | +72 | +90 | DNF | DNF | DNF | **tightest** |

So: **the two releases in the middle of the pump have ±20–30 ms of room. The
three commits — into the throw, into tap 3, and into the kicker — are the tight
ones**, and the kicker commit is the tightest: 10 ms early costs 90 ms, 10 ms
late does not finish at all.

Read the *whole* row rather than one cell: several single mistimings cost only
9–46 ms, and the margin to the author time is 52. A run that gets the technique
right and one commit slightly wrong still beats the AT.

Caveat stated plainly: this measures reproducing *our exact tape*. The early
inputs (260 / 1230 / 2450 ms) also show zero slack, which is obviously not a
real driving constraint — 42 people finish this map — it just means our tail was
optimised against one exact prefix. Treat the corner rows as the meaningful ones.

---

## 5. The driving guide

The complete keyboard tape. `race` is the on-screen clock; the first three lines
are the pre-start state.

```
race    260  steer LEFT          | as the car starts rolling
race   1230  steer  0            | straighten, ~68 km/h
race   2450  steer RIGHT         | on the booster, ~186 km/h - hold through the ramp
race   5230  brake ON            | STILL IN THE AIR, ~120 ms before touchdown
race   5240  steer 0             | land straight
race   6680  steer LEFT          | commit into the wall throw
race   6900  steer 0             |  \
race   6950  steer LEFT          |   |  the PUMP: three taps
race   7030  steer 0             |   |  220 ms / 80 ms / 40 ms
race   7120  steer LEFT          |   |
race   7160  steer 0             |  /
race   7390  steer LEFT          | commit to the kicker, hold to the finish
```

Accelerate is held from the start and never released. Brake goes on at 5.23 s
and stays on.

### Sector by sector, off what you can see

**Start → the booster (0 → 2.4 s).** Identical to the current world record. Left
off the line, straighten at about 68 km/h, then **full right as the booster
fires** and hold it. Nothing to gain here; do not experiment.

**The big jump (3.55 → 5.35 s).** Keep full right lock through the entire
flight. You are not steering, you are spinning the car so it lands pointing down
the road. Every good run does this identically.

**The landing (5.23 s).** *Brake while you are still in the air*, about an eighth
of a second before the wheels touch, and centre the wheel at the same moment.
The current WR brakes at 5.23 s too — this part is already right. Braking early
is safe (up to 30 ms early costs under 30 ms); braking late does not finish.

**The straight (5.35 → 6.66 s).** Nothing to do. You will coast 455 → 422 km/h.
Use it to get your hand ready.

**THE CORNER — this is the whole map (6.68 → 7.16 s).**
You get thrown off the wall at ~6.68 s. You cannot miss it: you are doing
420 km/h in a straight line and the car is slammed sideways and whipped through
more than half a turn, speed collapsing to ~325. Commit **full left as it
happens** — this input is 10 ms-critical, so tie it to the impact, not to a
clock.

Then **do not hold it.** Three taps, and here is the cue for each — the cue
matters far more than the millisecond:

| tap | cue to act on | what the speedo reads |
|---|---|---|
| **release tap 1** (~6.90 s) | **the bottom of the swing.** After the throw you are travelling *away* from the finish while the car swings round. Release at the instant that stops — the moment you stop sliding backwards and the car starts being flung forward up the corridor. (Measured: our release is at the exact tick the forward velocity crosses zero, and at the lowest point of the arc.) | **292 km/h** |
| **tap 2** (~6.95 s) | half a beat later — a short stab, ~80 ms, immediately after the release | 289 |
| **release tap 2** (~7.03 s) | as the car swings onto the corridor and the nose comes round towards the finish | 283 |
| **tap 3** (~7.12 s) | a flick, ~40 ms, as the car is nearly straight | 278 |
| **release tap 3** (~7.16 s) | straight away — you are now pointed at the finish | 276 |

Rhythm, if you want one phrase: after the throw, **hooold – tap – tick**
(220 ms, 80 ms, 40 ms), with a short gap between each.

**And the cue that tells you it is working, which you can see without looking at
anything: THE HORIZON MUST STAY LEVEL.** If you hold the lock, the car rides up
the curved wall and rolls over — the whole world tips, and by the kicker the
human world record is lying at 57° nose-up and rolled almost 60°, ending up
nearly on its side (85°) in the air. Our run never rolls past **5°**. Each
release lets the car drop back flat. **A tipped car is what pays 50 km/h to the
kicker.** If your horizon rolls, you are driving the old line.

**The kicker (7.39 s).** Full left again, and hold it to the finish. Cue:
**commit as the nose stops rising** — the car crests the curved wall onto the
last ramp and the pitch stops climbing about two car lengths before the lip.
This is the tightest input on the map (10 ms early costs 90 ms, 10 ms late does
not finish), so use the crest, not the clock. The current WR takes this input at
7.69 s, three tenths later, which is exactly why it launches steep and slow.

**The check on your run.** Watch the speed as you are thrown into the final
flight, ~7.4 s:

* **221 km/h** — you drove it like the current world record. ~8.20.
* **240 km/h** — about the author time.
* **252 km/h** — 8.075, keyboard.
* **263 km/h** — 7.998, the TAS floor.

That single number tells you whether you got the corner right, and it is
visible on the speedo.

### What is realistic, and what is not

**Realistic.** The pump is four key presses on a rhythm, and the two releases in
the middle have ±20–30 ms of slack. The technique is the discovery; you do not
need our exact timings to get most of it, because the field's problem is not
that it is 30 ms off — it is that it is holding a lock for 390 ms.

**Hard.** The kicker commit at 7.39 s. 10 ms early costs 90 ms, 10 ms late fails.
Expect to grind that one, and use the base of the ramp as the cue rather than
the clock.

**Not worth copying.** Everything in the analog tapes below 8050 — the 111
distinct steering values in the 7998 run are a machine holding the car flat with
per-tick corrections. The keyboard tape gets 121 of the 199 ms with three
values.

**Honest expectation.** A keyboard player who learns the pump and gets the
kicker commit approximately right should land in the 8.05–8.12 range, which
beats an author time that has stood since February 2025. The last 50 ms to 7998
is machine work.

---

## 6. Method, and what generalises

* Whole 42-run field downloaded (trackmania.io leaderboard, unauthenticated) and
  re-simulated. 41 of 42 reproduce their leaderboard time to the millisecond.
  **One does not: `p37`, leaderboard 8610, re-simulates to 8477** — flagged, not
  used, worth someone's attention.
* **The keyboard alphabet was established, not assumed.** The human WR's own
  input tape contains exactly three steer values — `-127`, `0`, `+127` — with 11
  change events. Two other runs in the field are the same. Pad runs use 55–94
  distinct values. So `{-127, 0, +127}` is ground truth for "what a keyboard
  emits", and the ladders for action keys are N evenly spaced levels per side of
  that same signed-byte range (the tape stores one `i8` per 10 ms tick; 255
  values, not TMNF's ±65536).
* **Quantising an optimised analog tape does not work at any resolution** — even
  a 64-level ladder (max change ±1/127) makes the 7998 tape DNF. The tape is
  chaotic at the least-significant-bit level. Low-input tapes have to be
  *searched for* under the constraint, not simplified into existence. Seeding
  the constrained search from the human keyboard WR (already legal) and letting
  it optimise inside the alphabet reached 8102 in 80 seconds and 8058 in 25
  minutes.
* **Simplification after the fact works fine within an alphabet**: greedy event
  deletion took the keyboard tape from 20 events to 14 for zero cost, and the
  analog tape from 185 steer events to 62 for 23 ms.
* **Seed choice beat search effort.** Arms seeded from the keyboard WR converged
  near 8.14 unconstrained; the arm seeded from the rank-2 *pad* run — 31 ms
  slower as a human run, but with a steering tape a search can deform — produced
  everything below 8.13.
* The **mid-simulation fork server** (`--fork --forktick 700`, resuming just
  after the landing) was what made the tail searchable: 8125 → 8010 in 65 s.
* The **sub-tick timing plane** (from the 191465 agent) broke the millisecond
  plateau: after 1.7 M evaluations stuck at 7998, scoring on the interpolated
  crossing of the finish plane moved it in 90 seconds. This map's finish is a
  **z-plane crossed with z increasing**, not an x-plane, so `pred_core.rs` now
  takes a negative plane value to mean "z-plane at |v|, crossed +z"; and the
  plane must be placed at the z the seed crosses **at its own validated
  millisecond**, or the whole-tick calibration guard correctly aborts every
  worker.
* Tooling written for this map, all Rust: `tmsimp` (quantise / thin / grid-snap /
  constrained polish / tolerance, all against the real oracle) and `an`
  (trajectory phases, arclength-accumulated deltas, airborne detection from the
  −24.7 m/s² plateau, corridor deviation, plane crossings).

---

## 7. Incident: the sub-tick plane surrogate is unusable on this map

Logged because it is a fleet-relevant precondition, not a local mishap.
Specimen: `~/persistent/private-30d/tm-loop/phantoms/m165-subtick-plane-20260818-1752/`.

I adopted the 191465 agent's sub-tick timing plane to break a millisecond
plateau (1.7 M evaluations stuck at 7998). It worked immediately by its own
score — 7990.705 ms in 90 seconds — and **it does not re-validate: the plain
oracle returns 8004 ms.** The "improvement" is in fact 6 ms slower than its own
seed. Caught on first validation; the arms ran four minutes and nothing they
produced was banked.

**Root cause, measured: the finish trigger is body-based, not a plane through
the car's centre, and on this map the car crosses the gate AIRBORNE with a
strongly varying attitude.** z at each tape's own validated finish millisecond:

| tape | z | x | y |
|---|---|---|---|
| analog 7998 | 906.220 | 567.59 | 45.50 |
| the phantom | **907.120** | 567.52 | 45.68 |
| analog 8010 | 906.658 | 570.92 | 44.64 |
| keyboard 8075 | 906.258 | 570.53 | 45.26 |
| human WR | 907.350 | 570.89 | 45.12 |
| human #2 | 906.055 | 574.94 | 53.91 |

**1.30 m of spread, i.e. up to ~19 ms** — far larger than the gains the
surrogate is being used to resolve. The phantom and its seed cross at
essentially the same x and y, so this is not gate geometry in the transverse
plane: it is that a differently oriented car presents a different leading point,
and ~0.9 m is about a car half-length. Roll varies over 1.5 rad across this
field, which is why.

Per-seed calibration was *perfect* (offset −10 ticks, residual 0.002 ms), and the
whole-tick calibration guard passed on every surviving worker. **That is what
makes it dangerous: every internal check succeeds.**

**Rule for anyone reusing the technique: the sub-tick plane requires the finish
to be crossed with a repeatable attitude.** It is sound on a ground finish; it
is not applicable to a flying one. On this map, score on the validator's
millisecond.

(The `pred_core.rs` axis convention added here — a negative plane value meaning
"z-plane at |v|, crossed with z increasing" — is correct and worth keeping for
maps whose run axis is z; it was not the cause.)
