# [Turtle Trial] Leto

**Nobody on this map has ever needed to drive better — they needed to fail less.
Respawn the instant an attempt is dead, and the author time falls by 136.369.**

| tape | time | vs author time | vs human WR | steering |
|---|---|---|---|---|
| [`BEST_218812`](replays/BEST_218812.Ghost.Gbx) | **218.812** | **−136.369** | −222.190 | 3 values — keyboard |
| [`KEYBOARD_218877`](replays/KEYBOARD_218877.Ghost.Gbx) | 218.877 | −136.304 | −222.125 | 3 — keyboard |
| [`AUTHORCUT_220391`](replays/AUTHORCUT_220391_watchable.Ghost.Gbx) | 220.391 | −134.790 | −220.611 | the author's own driving, retries cut |
| [`TAS_235625`](replays/TAS_235625.Ghost.Gbx) | 235.625 | −119.556 | −205.377 | 26 |
| [`KEYBOARD_235939`](replays/KEYBOARD_235939.Ghost.Gbx) | 235.939 | −119.242 | −205.063 | 3 — keyboard |
| [`HUMANCUT_236972`](replays/HUMANCUT_236972_watchable.Ghost.Gbx) | 236.972 | −118.209 | −204.030 | the world record's own inputs, retries cut |
| human world record, Bald_tm | 441.002 | +85.821 | — | 3 — keyboard |

Author time **355.181** — faster than the human world record, and set by the same
person. unbeaten.at MapId 286279, uid `p0tVjdmb1DfkCVrDE_DfQN84kq8` · author
**BALDFROMSPB / Bald_tm**, who also holds the world record · tags Trial, Turtle ·
**5** recorded runs, all five analysed. Every time above was re-simulated against
Nadeo's own map file with the world record as a control.

## The leaderboard ranks failures, not pace

On a Trial map the clock keeps running through respawns, so a recorded time is
clean driving *plus every failed attempt*. On this map the failures are most of
the clock: across the five recorded runs there are **272 failed attempts costing
76.8 minutes** of race time, and no two runs differ meaningfully in speed while
the car is actually moving.

| run | time | failed attempts (S1 / S2 / S3 / S4) |
|---|---|---|
| rank 1 Bald_tm | 441.002 | 0 / 0 / **10** / 0 |
| rank 2 Quantiks | 977.690 | 0 / 4 / 18 / 18 |
| rank 3 Ta__Da | 1271.692 | 0 / 9 / 42 / 1 |
| rank 4 Schmaniol | 1371.430 | 0 / **45** / 2 / 5 |
| rank 5 Max_heyu | 1961.645 | 0 / 52 / 28 / 38 |
| the author's own run | 355.181 | 0 / 0 / **9** / 0 |

Nobody has ever failed sector 1.

Two of the tapes above contain **no new driving at all**. The 220.391 is the map
author's own author-time lap with their nine failed attempts cut out; the 236.972
is the world record holder's own inputs, in his own order, with ten of *his*
failures cut out. The author's own lap decomposes like this:

| | time |
|---|---|
| S1, start → CP1, clean | 42.036 |
| S2 → CP2, clean, no respawns | 79.882 |
| **S3 — nine failed attempts** | **134.618 — wasted** |
| S3, the attempt that worked | 45.961 |
| S4 → finish, clean | 52.081 |
| **the author time as recorded** | **355.181** |
| **the same lap without its own retries** | **220.563** |

Their S1, S2 and S4 are each faster than the world record's — 16.2 s in total —
and their winning S3 agrees with his to within 0.582. All of the 134.790 is
retries. Only the last 1.579, from 220.391 down to 218.812, came from anything a
human did not already drive.

## What the map is

3316 m, three checkpoints, about 237 s of clean driving. **The car spends 154.5
of 235.3 seconds — 66% of the run — upside down.** That is the map: "Turtle"
means you deliberately flip onto the roof and drive there, rocking between roll
+2.4 and −2.9 rad at 6–15 m/s with wheels leaving the ground on every swing.
Only 37.6 s of the run is above 20 m/s; 98.2 s is under 10 m/s.

| phase | race time | attitude | what happens |
|---|---|---|---|
| A | 0 → 11.3 | upright | the only genuinely fast part — 446 m at 44.6 m/s, a long jump to y=67 |
| B | 11.3 → 41.3 | **inverted 30 s** | first turtle section, the y≈65 deck |
| C | 41.3 → 53.2 | upright | drop to y≈39; **CP1 at 45.6**; standing respawn; blast to 36 m/s |
| D | 53.2 → 117.9 | **inverted 58 s** | the long one, y≈35, out to x=1105 and back |
| E | 117.9 → 134.1 | upright | down to the low point y≈9; **CP2 at 130.2**; accelerate to 28 m/s and flip |
| F | 134.1 → 172.4 | **inverted 38 s** | the z≈1056 corridor, y≈25 |
| G | 172.4 → 187.7 | upright | **CP3 at 176.6**; standing respawn; 58 m/s and a launch to y=74 |
| H | 187.7 → 203.2 | upright | the high deck |
| I | 203.2 → 225.6 | **inverted 22 s** | final turtle section, descending y 65 → 51 |
| J | 225.6 → finish | upright | flip back onto the wheels, crawl to the booster, 66 m/s to the line |

## Where the field dies

Eight of the ten most expensive spots on the map are **slow-speed balance
failures at roll 2.4–2.9 rad**: the car is on its roof, rocks past the balance
point, and lands back on wheels where there is no road under wheels. All at
5–12 m/s. Only one of the top ten is a fast crash.

| # | sector | where a clean run is | field time lost | tries | speed | roll |
|---|---|---|---|---|---|---|
| 1 | 3 | 159.9 | 606.350 | 17 | 10.0 | 2.41 |
| 2 | 2 | 99.8 | 486.280 | 9 | 5.5 | 2.41 |
| 3 | 3 | 140.6 | 364.220 | 29 | 8.4 | 2.57 |
| 4 | 3 | 170.4 | 338.370 | 7 | 12.2 | 1.73 |
| 5 | 3 | 134.8 | 216.460 | **40** | 6.9 | 2.39 |
| 6 | 2 | 54.8 | 214.570 | 24 | 6.6 | 1.69 |
| 7 | 2 | 52.4 | 187.990 | **30** | **43.0** | 0.26 |

**The author's own nine failures are all in sector 3, five of them at the same
place everybody else fails.** The best player on the map dies where you do.

### The single most useful sentence

Through the z≈1056 corridor — the most expensive spot on the map, 606 seconds of
field time burned there — the clean run holds **full left lock almost
continuously for 2.6 s**, from 160.1 to 162.7, and reaches 16.0 m/s, the fastest
sustained inverted speed anywhere in the run. All 17 failures there are at
10 m/s.

**Speed is what keeps you on the roof. Creeping is what tips you over.**

## The run as inputs

### The flip-in — the most-attempted spot on the map, 40 tries

Just after CP2, race 132.6–135.0. This is where you deliberately turn the car
over, and it is the skill the whole map is built around.

```
132.6   28.8 m/s   roll 0.07   gas 1    hit the ramp square and flat, full throttle
133.0   27.4       roll 0.22   gas 0    RELEASE at the crest, nose 30° up
133.4   19.4       roll 1.14   gas 0    airborne, rolling over
133.8   15.4       roll 1.77   steer +0.80    feed the roll in
134.2   12.1       roll 2.30   steer +1.00    full lock as the nose drops
134.4    9.8       roll 2.74   ground         land on the roof
134.8    5.3       roll −2.55                 settled; now drive
```

Arrive at 28–29 m/s **dead flat** (roll under 0.1), full throttle to the crest,
release the throttle exactly at the crest, feed progressively more steering lock
into the air phase, and land on the roof at about 10 m/s. Throttle stays off from
the crest until the car settles.

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
to come back onto its wheels, which is exactly what the 52 failures in this part
of the map look like.

### The two standing respawns are a technique, not a mistake

At CP1 and CP3 the world record crosses at 25 and 35 m/s, respawns about 250 ms
later, and sits frozen for roughly 850 ms. It looks like waste. It is not: the
alternative is braking to a controlled standstill, which costs more, and a
standing respawn hands you a **perfectly known** entry state — square, level,
stationary — for a section where attitude is everything. Every run on the
leaderboard does it. Copy it.

The freeze itself is inert: throttle, brake and full lock all do exactly nothing
during it, so there is no input to get right there.

### The last obstacle is where the remaining time is

The flip-back at race 225.6–231.5: the car arrives inverted at 9.5 m/s, unwinds
to upright, and then spends **4.0 seconds rocking on the spot under 3 m/s**
before it gets moving to the finishing booster. That is the largest single piece
of dead time left in the run, and it is where ranks 2, 4 and 5 lost 83.8, 64.3
and 59.2 on single attempts.

## Keyboard costs almost nothing here

`KEYBOARD_235939` is the same run with steering restricted to
`{left, nothing, right}`: **235.939 with exactly three steer values**, against
235.625 for the unrestricted version. **Keyboard costs 0.314 out of 236 seconds —
0.13%.** The fastest tape on the map is 3-valued as well, and so is every human
tape on it, including the author's. On a low-speed technical map, analog steering
is not where the time is.

## How forgiving it is

The map itself is forgiving in the only way that matters on a Trial: a mistake
costs a retry, never the run. What it punishes is *hesitating* about the retry.
The one number to take away is that failing an attempt and respawning
immediately is worth more than any line improvement anyone has found here.

What is genuinely tight:

- **The flip-in entry.** Roll under 0.1 at the ramp at 28–29 m/s. Any roll there
  is a bad landing 1.5 s later, and nothing in the air will save it.
- **The throttle release at the crest**, which sets the pitch for the whole
  rollover.
- **The corridor at z≈1056.** Carry 16 m/s; the field's 17 failures there are all
  at 10.

What will take real practice is the inverted crawl rhythm — 154 seconds of it,
held between roll 2.3 and 2.9, where the failure mode is drifting *below* 2.3
rather than anything dramatic. There is no shortcut for that section; the whole
field grinds it.

The exact tapes here are not something a human executes: **90% of their inputs
have a single-tick window — 10 ms early or late and the run dies.** Take the
method and the map knowledge, not the timings.

## Files

Most tapes in this project are input streams a validator re-simulates but the
game will not load. The two pure-cut results have been rebuilt so they **load and
play**.

| file | what it is |
|---|---|
| [`replays/BEST_218812.Ghost.Gbx`](replays/BEST_218812.Ghost.Gbx) | the fastest run, 218.812, keyboard steering |
| [`replays/AUTHORCUT_220391_watchable.Ghost.Gbx`](replays/AUTHORCUT_220391_watchable.Ghost.Gbx) | the author's own lap with its nine retries cut — watchable, and the best thing to study |
| [`replays/AUTHORCUT_220391.Ghost.Gbx`](replays/AUTHORCUT_220391.Ghost.Gbx) | the same cut of the author's own lap before it was made watchable — his driving, not ours, kept for comparison |
| [`replays/HUMANCUT_236972_watchable.Ghost.Gbx`](replays/HUMANCUT_236972_watchable.Ghost.Gbx) | the world record's own inputs with ten retries cut — watchable |
| [`replays/KEYBOARD_218877.Ghost.Gbx`](replays/KEYBOARD_218877.Ghost.Gbx) | keyboard-only version of the fastest run, 218.877 |
| [`replays/TAS_235625.Ghost.Gbx`](replays/TAS_235625.Ghost.Gbx) | the world-record lineage, 26 steer values |
| [`replays/KEYBOARD_235939.Ghost.Gbx`](replays/KEYBOARD_235939.Ghost.Gbx) | the same, keyboard-only — the 0.314 comparison |
| [`replays/HUMANCUT_237122_watchable.Ghost.Gbx`](replays/HUMANCUT_237122_watchable.Ghost.Gbx) | the first cut of the world record, before the checkpoint trims |

## Withdrawn: `AUTHORMIN_831ev_354781.Ghost.Gbx`

That replay file has been removed. It is a **splice** rather than a whole-file
copy — 8 of 7087 samples bit-identical to Bald_tm's recording and **707 m of
shared path** — so most of what it shows is its own driving with somebody else's
stitched into it. The rule it fails: *a file whose recorded trajectory contains a
human's, in whole or in part, is not ours to publish.*

The time and the analysis are unaffected; the result on this map comes from the
author's own ghost recovered from inside the `.Map.Gbx`, and that provenance is
unchanged. A regenerated replacement will be published.
