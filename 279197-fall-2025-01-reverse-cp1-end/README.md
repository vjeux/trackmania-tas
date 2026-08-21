# Fall 2025 - 01 Reverse CP1 End

**The dramatic-looking final corner is not where this map is won: 95% of the
whole field's spread is already decided by 9.500, and the run home costs
everybody the same.**

**Video: withdrawn.** The clip published here was filmed from a copy of our ghost
that is not the file this page ships — a stale staging copy, superseded before the
clip was shot. The run and the time are unaffected. A replacement will be filmed
from a regenerated ghost, whose telemetry is verified against a re-simulation of
its own inputs.

| run | time | vs author time | vs human WR | inputs |
|---|---|---|---|---|
| **TAS** | **10.594** | **−0.004** | **−0.008** | analog |
| TAS, 16 detents | 10.602 | +0.004 | ±0 | 264 events, 32 values |
| TAS, action keys (20% detents) | 10.643 | +0.045 | +0.041 | 76 events, 11 values |
| TAS, keyboard | 10.636 | +0.038 | +0.034 | 66 events, 3 values |
| TAS, keyboard, 35 events | 10.646 | +0.048 | +0.044 | **35 events**, 3 values |
| Author time (never beaten by a human) | 10.598 | — | −0.004 | — |
| Human WR — ShcrTM | 10.602 | +0.004 | — | pad |

TMX map [279197](https://trackmania.exchange/maps/279197) · author **in-.-** ·
**561 recorded runs**.

## The map

A 10.6 s, 597 m standing-start sprint. **Full throttle the whole way: no brake
at all and no lift anywhere**, in the world record and in this run alike.
Everything is in the steering.

| race | what happens |
|---|---|
| 0 | launch, gear 1 |
| 0.700–3.000 | a long left-hand bend |
| **3.000–4.200** | the chicane: a hard flick from left to right at 3.200–3.600 |
| 3.600–5.800 | downhill, gear 3, 160 → 235 km/h |
| 5.800–7.800 | flat straight, gear 4, 235 → 281 km/h |
| **7.800–10.600** | one 140 m-radius right-hand sweeper, flat out, 286 → 341.7 km/h |

Speed rises monotonically and saturates at 341.7 km/h about 0.150 before the
line — every one of the 27 measured humans is at exactly that speed at the gate.
The endgame has no speed left to find: **time is distance, at 9.49 cm per
millisecond.**

This run differs from the world record on 339 of its 1061 ticks, and they are
not spread evenly: **the chicane flick (3–4 s) and the whole sweeper (7 s to the
flag)**. The bend, the downhill and the straight are driven exactly as the human
drove them.

## Where the time is — and is not

Timing the whole field through intermediate gates, from rank 1 to rank 502:

```
time from the z=655 plane to the flag:
  human WR  1.100      rank 52   1.103      rank 502  1.102
  rank 8    1.103      rank 152  1.110      this TAS  1.103
  rank 15   1.103      rank 302  1.106
```

**The closing sweeper costs everyone the same.** A 0.198 spread across the field
compresses to 0.010 over the final 1.1 seconds.

The same is true of this run's own margin. Against the world record it is level
for the first 4.5 seconds and within 0.002 for the first eight, then:

| gate | human WR | this run |
|---|---|---|
| x = 640 (≈8.070) | 8.067 | 8.065 (−0.002) |
| z = 672 (≈9.500) | 9.502 | 9.492 (**−0.010**) |
| z = 717 | 10.058 | 10.051 (−0.007) |
| z = 757 | 10.490 | 10.481 (−0.009) |
| flag | 10.602 | **10.594 (−0.008)** |

**The entire advantage is made between 8.100 and 9.500** — the entry and first
half of the closing sweeper — and 0.002 of it is handed back over the run home.
The 85 rewritten ticks in the chicane buy no time at all on their own; they only
set the car up for that entry.

So: **practise the first 9.5 seconds — the chicane at 3.200 and the entry to the
sweeper. The run home is free, and identical for everybody.**

The sector-by-sector guide with visual cues for this map is not written yet.

## The finish trigger has an invisible edge

The finish is a plane with a **finite lateral window**, and its inside edge is
at world x = 772.18. The world record crosses 0.35 m outside it; one top-15 run
passes only **5 cm** outside. Cut inside and the run simply does not finish — no
partial credit, no leaderboard entry, no feedback. Runs killed that way never
reach a leaderboard, so the public field cannot tell you how often it happens.
There is no visible cue for the edge: it lines up with no seam, kerb or scenery
edge anywhere in the map.

**It is a hazard, not a pace-setter.** Measuring the clean margin of all fifteen
of the top 15 gives 0.013 of time against 1.40 m of margin, with no relationship
between them: the tightest run in the field (5 cm from the edge) is 0.012
*slower* than the world record, and the widest (1.45 m) is 0.010 slower. Do not
chase the edge — being tighter does not make you faster, it only makes you more
likely to lose the run.

## How forgiving it is

**This route has no open-loop tolerance anywhere.** Rounding the steering trace
to even values — a change of at most half of one of 255 units per tick — makes
the run fail, as does holding each input for two ticks instead of one, and both
fail mid-route rather than at the gate. Every input matters everywhere.

That is a statement about the map, not about you: **the same tests destroy the
human world record's own tape**, because a recorded tape replayed blind has no
eyes. A driver is a closed loop who sees the car drift and corrects on the next
frame, and 561 people have this route on the board. What it means in practice is
that this map needs continuous correction rather than a memorised script — there
is no coasting stretch where a small error washes out.

**What will take real practice** is the chicane flick and the sweeper entry,
held to a standard the whole field already meets, plus the discipline to leave
the last corner alone.

## On a keyboard or action keys

The top of this board is all pad: ranks 1–15 steer with small continuous
corrections, and **every digital human sits at rank 152 or worse**. The deficit
is diffuse — the digital runs are 1–4 km/h down at every station from 8 s
onward, with no single corner to fix, because a coarse alphabet cannot hold the
small steady steering angles the sweeper rewards.

So the honest answer is that **a keyboard does not reach the author time here**.
What is available is still worth having:

- **Action keys (20% detents), 76 inputs: 10.643** — this is the rung that
  matches a real bindable setup, and it is 0.015 faster than the best keyboard
  human on the board.
- **Pure keyboard, 35 inputs: 10.646** — 0.012 faster than the best keyboard
  human, in half the inputs.

Both would place top-150 of 561.

## Files

| file | what |
|---|---|
| `replays/real_10594.Ghost.Gbx` | **the fastest run — 10.594** |
| `replays/real_10595.Ghost.Gbx` | the 10.595; identical until the final 40 m, where it crosses 3.5 cm wider |
| `replays/best_10596.Ghost.Gbx`, `best_10597.Ghost.Gbx`, `best_10598.Ghost.Gbx` | earlier validated runs |
| `replays/DETENT16_10602.Ghost.Gbx` | 16-detent steering, 10.602 |
| `replays/ACTIONKEY_5detent_10643.Ghost.Gbx` | **the action-key run — 76 inputs on the 20% ladder people actually bind** |
| `replays/KEYBOARD_10636.Ghost.Gbx` | pure keyboard, 66 events |
| `replays/KEYBOARD_35ev_10646.Ghost.Gbx` | pure keyboard in **35 inputs** — the one to learn |
| `inputs/real_10594.tick.txt`, `inputs/real_10595.tick.txt` | those two runs as readable input scripts |
