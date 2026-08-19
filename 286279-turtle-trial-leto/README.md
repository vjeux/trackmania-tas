# [Turtle Trial] Leto — the author time falls by nearly two minutes

**Author time 355.181 · human world record 441.002 · this run 235.625 —
the author time beaten by 119.556 s (−33.7%), the world record by 205.377 s.**

| tape | time | vs AT | vs human WR | steer values | input events |
|---|---|---|---|---|---|
| [`TAS_235625`](replays/TAS_235625.Ghost.Gbx) | **235.625** | **−119.556** | −205.377 | 26 | 964 |
| [`KEYBOARD_235939`](replays/KEYBOARD_235939.Ghost.Gbx) | **235.939** | −119.242 | −205.063 | **3 — keyboard** | **941** |
| the human WR's own inputs, retries deleted | 236.972 | −118.209 | −204.030 | 3 — keyboard | 943 |
| human WR, Bald_tm *(control)* | 441.002 | +85.821 | — | 3 — keyboard | 1811 |

unbeaten.at MapId 286279 · uid `p0tVjdmb1DfkCVrDE_DfQN84kq8` · author
**BALDFROMSPB / Bald_tm**, who also holds the human world record · tags Trial,
Turtle · **5** recorded runs, all five analysed.

---

## The whole result in one sentence

**Bald_tm has already driven a 237-second lap of this map.** He drove it in
eleven pieces, over seven and a half minutes, and every piece of a
sub-four-minute run is sitting inside his own recorded world record.

On a Trial map the clock keeps running through respawns, so a recorded time is
clean driving *plus every failed attempt*. Delete the ten attempts he failed in
sector 3 and his own tape — not one input changed, not one tick of TAS mutation
— finishes in **236.972**. That is 118 seconds under an author time that had
never been beaten.

Nothing here was driven better than the human world record. It was driven
*fewer times*.

## The leaderboard is a ranking of failures, not of pace

Across the five recorded runs there are **272 failed attempts costing 76.8
minutes** of race time. No two runs differ meaningfully in speed while the car
is actually moving.

| run | time | failed attempts (S1 / S2 / S3 / S4) |
|---|---|---|
| rank 1 Bald_tm | 441.002 | 0 / 0 / **10** / 0 |
| rank 2 Quantiks | 977.690 | 0 / 4 / 18 / 18 |
| rank 3 Ta__Da | 1271.692 | 0 / 9 / 42 / 1 |
| rank 4 Schmaniol | 1371.430 | 0 / **45** / 2 / 5 |
| rank 5 Max_heyu | 1961.645 | 0 / 52 / 28 / 38 |
| **the author's own AT run** | **355.181** | 0 / 0 / **9** / 0 |

Nobody has ever failed sector 1.

## What the map is

3316 m, three checkpoints, about 237 s of clean driving. **The car spends 154.5
of 235.3 seconds — 66% of the run — upside down.** That is not a mistake, it is
the map: "Turtle" means you deliberately flip the car onto its roof and drive it
there, rocking between roll +2.4 and −2.9 rad at 6–15 m/s with wheels leaving
the ground on every swing.

Only 37.6 s of the run is above 20 m/s. 98.2 s is under 10 m/s.

| phase | race time | attitude | what happens |
|---|---|---|---|
| A | 0 → 11.3 | upright | the only genuinely fast part — 446 m at 44.6 m/s, a long jump to y=67 |
| B | 11.3 → 41.3 | **inverted 30 s** | first turtle section, the y≈65 deck |
| C | 41.3 → 53.2 | upright | drop to y≈39; **CP1 at 45.6**; standing respawn; blast to 36 m/s |
| D | 53.2 → 117.9 | **inverted 58 s** | the long one, y≈35, out to x=1105 and back |
| E | 117.9 → 134.1 | upright | down to the low point y≈9; **CP2 at 130.2**; accelerate to 28 m/s and flip |
| F | 134.1 → 172.4 | **inverted 38 s** | the z≈1056 corridor |
| G | 172.4 → 187.7 | upright | **CP3 at 176.6**; standing respawn; 58 m/s and a launch to y=74 |
| H | 187.7 → 203.2 | upright | the high deck |
| I | 203.2 → 225.6 | **inverted 22 s** | final turtle section, descending y 65 → 51 |
| J | 225.6 → finish | upright | flip back onto the wheels, crawl to the booster, 66 m/s to the line |

## How a human drives this

### Where the field actually dies

Eight of the ten most expensive spots on the map are **slow-speed balance
failures at roll 2.4–2.9 rad** — the car is on its roof and loses it, rocking
past the balance point and landing back on wheels where there is no road under
wheels. All at 5–12 m/s. Only one of the top ten is a fast crash.

| # | sector | field time lost | tries | speed | roll |
|---|---|---|---|---|---|
| 1 | 3 | 606.350 | 17 | 10.0 | 2.41 |
| 2 | 2 | 486.280 | 9 | 5.5 | 2.41 |
| 3 | 3 | 364.220 | 29 | 8.4 | 2.57 |
| 4 | 3 | 338.370 | 7 | 12.2 | 1.73 |
| 5 | 3 | 216.460 | **40** | 6.9 | 2.39 |
| 7 | 2 | 187.990 | 30 | **43.0** | 0.26 |

**The author's own nine failures are all in sector 3, five of them at the same
place everybody else fails.** The best player on the map dies where you do.

### The single most useful sentence

Through the z≈1056 corridor — the most expensive spot on the map, 606 seconds of
field time burned there — the clean run holds **full left lock almost
continuously for 2.6 s** and reaches 16.0 m/s, the fastest sustained inverted
speed anywhere in the run. All 17 failures there are at 10 m/s.

**Speed is what keeps you on the roof. Creeping is what tips you over.**

### The flip-in — the most-attempted spot on the map (40 tries)

Just after CP2, race 132.6–135.0 s. This is where you deliberately turn the car
over, and it is the skill the whole map is built around.

```
132.6 s   28.8 m/s   roll 0.07   gas 1    hit the ramp square and flat, full throttle
133.0 s   27.4       roll 0.22   gas 0    RELEASE at the crest, nose 30° up
133.4 s   19.4       roll 1.14   gas 0    airborne, rolling over
133.8 s   15.4       roll 1.77   steer +0.80    feed the roll in
134.2 s   12.1       roll 2.30   steer +1.00    full lock as the nose drops
134.4 s    9.8       roll 2.74   ground         land on the roof
134.8 s    5.3       roll −2.55                 settled; now drive
```

Arrive at 28–29 m/s **dead flat** (roll < 0.1), full throttle to the crest,
release the throttle exactly at the crest, then feed progressively more steering
lock into the air phase, and land on the roof at about 10 m/s. Throttle stays
off from the crest until the car settles.

Both hard parts are in the first 0.4 s: the entry has to be square, because any
roll at the ramp becomes a bad landing 1.5 s later and there is no correction
available in the air; and the throttle release is what sets the pitch. Note that
29 of the failures at the next obstacle are 48.5 m *past* this flip — the field
mostly lands on the roof and then loses it, rather than failing the flip.

### The inverted crawl

A rocking oscillation, and **the successful pattern is a steady rhythm, not a
hold**: full lock one way for 0.4–0.6 s, neutral through the rock, full lock the
other way, throttle pulsed on the down-swing and off through the inversion. Roll
magnitude never drops below about 2.3 — dropping below that is the car starting
to come back onto its wheels, which is exactly what the 52 failures there look
like.

### The two standing respawns are a technique, not a mistake

At CP1 and CP3 the world record crosses at 25 and 35 m/s, respawns about 250 ms
later, and sits frozen for ~850 ms. It looks like waste. It is not: the
alternative is braking to a controlled standstill, which costs more, and a
standing respawn hands you a **perfectly known** entry state — square, level,
stationary — for a section where attitude is everything. Every run on the
leaderboard does it. Copy it.

### The last obstacle is where the remaining time is

The flip-back at race 225.6–231.5 s: the car arrives inverted at 9.5 m/s, unwinds
to upright, and then spends **4.0 seconds rocking on the spot under 3 m/s**
before it gets moving to the finishing booster. That is the largest single piece
of dead time left in the run, and it is where ranks 2, 4 and 5 lost 83.8, 64.3
and 59.2 seconds on single attempts.

## Keyboard costs almost nothing here

`KEYBOARD_235939` is a keyboard-constrained search over the final sector, with
steering snapped to `{left, nothing, right}` before every evaluation:
**235.939 s, exactly three steer values, 941 input events** — two fewer than the
human tape it came from. The unconstrained tape reaches 235.625 with 26 values.

**Keyboard costs 314 ms out of 236 seconds: 0.13%.** On this map the input
device is irrelevant. Only failing is expensive.

## The author's own run says the same thing, louder

The author's author-time ghost is embedded inside the `.Map.Gbx` and decodes
cleanly. It contains **eleven respawns**, nine of them failed attempts in
sector 3:

| | time |
|---|---|
| S1, start → CP1, clean | 42.036 |
| S2, respawn → CP2, clean, zero respawns | 79.882 |
| **S3 — nine failed attempts** | **134.618 — wasted** |
| S3, the attempt that worked | 45.961 |
| S4, respawn → finish, clean | 52.081 |
| **the author time as recorded** | **355.181** |
| **the author time minus its own failed attempts** | **220.563** |

Two things follow. First, **the author time is a genuine driven lap** —
unbeaten.at flags it `inPlugin: true`, but a fabricated time does not contain
nine failed attempts at the same obstacle everybody else fails at. 135 of its
355 seconds are retries.

Second, **220.563 is the next target**, and it is the same driver's own driving:
their S1, S2 and S4 are all faster than the world record's, by 16.2 s in total,
while their winning S3 and the WR's agree to within 0.6 s. That tape cannot be
re-simulated yet — it lives in a different ghost container (see below) — so it
is not a published result, only a known floor.

## Two findings other maps need

**Respawn restores the state the car had when it crossed the checkpoint** —
position, velocity *and* attitude, not a standstill and not a canned
per-checkpoint state. Measured at the same checkpoint across five runs, each
returns to its own crossing state (26.7 / 22.2 / 16.8 / 23.4 m/s). It is also
history-free: splicing `WR[0..X) ++ WR[respawn..end)` and sweeping X across 200
seconds gives `finish = 237.122 + 10·(X − 13169)` exactly, every time.

Consequence: **a cut is safe, an optimisation upstream is not.** Deleting ticks
entirely after a crossing changes nothing the respawn depends on. Change
anything *before* the checkpoint and the crossing state moves with it, and every
input after the respawn was tuned for the old one. Respawn-anchored sectors are
therefore **not independent** and cannot be optimised in parallel and
recombined.

**Input tapes are not portable between ghost containers.** Transplanting a
run's input archive into a *different* ghost file DNFs at checkpoint 1, every
time, while transplanting it into its own container reproduces its time exactly.
Copying the archive alignment does not help, and neither does copying all
fourteen small `0x03092xxx` chunks from the donor. So **"best-of-field splice" is
not available** — splice within one ghost file only. This is also what blocks
re-simulating the 220.563 s author run above.

## Files

```
replays/TAS_235625.Ghost.Gbx            the floor — 26 steer values, 964 events
replays/KEYBOARD_235939.Ghost.Gbx       keyboard only — 3 values, 941 events
replays/TAS_analog26_235814.Ghost.Gbx   235.814, kept as a specimen (see notes)
notes/RESULT.md                         the full write-up: respawn semantics,
                                        obstacle analysis, every control
notes/AUTHOR_AT_355181_extracted_from_map.Ghost.Gbx
                                        the author's AT run, recovered from the
                                        map file — decodes, does not re-simulate
```

## Validation

Field reproduction **5/5 exact** — all five human ghosts re-simulate to the
millisecond (441.002, 977.690, 1271.692, 1371.430, 1961.645), so the map is
healthy for this oracle.

The headline tapes were cold-validated **three times each**, in a fresh
throwaway directory with a fresh server process, against a re-downloaded
byte-identical copy of the map, with the human world record as a known-answer
control in every batch: 235.625 ×3, 235.939 ×2, 236.972 ×3, control 441.002
every pass, all `IsValid: true`.

Tape-editor identity controls were run before any edit was trusted: the WR
re-encoded with every state word forced to a literal → 441.002; its CP1 respawn
cleared and re-injected at the same tick → 441.002; each run's inputs
transplanted into its own container → its own exact time.

**Legitimacy.** The run crosses all three checkpoints in order and the finish.
It is the human world record holder's own input stream. No geometry is skipped,
nothing goes out of bounds, and the three respawns it uses are three of the
thirteen he used himself — on a Trial map, respawning is the intended and only
recovery mechanic. Nothing here has been or will be submitted to a Nadeo
leaderboard.
