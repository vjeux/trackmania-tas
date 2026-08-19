# Fall 2025 - 01 Reverse CP1 End — author time beaten by 4 ms

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **10.594** | **−0.004** | **−0.008** |
| earlier validated tapes | 10.595, 10.596, 10.597, 10.598 | — | — |
| Author time (never beaten by a human) | 10.598 | — | −0.004 |
| Human WR | 10.602 | +0.004 | — |

TMX map [279197](https://trackmania.exchange/maps/279197) · uid
`_jkbEKnkKNw1B_TOgzbm5IYlkfc` · author **in-.-** · **561 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The finding: the part that looks hard is not where the time is

Intermediate gates were placed across the whole field — every rank from 1 to
502 — and the closing stretch timed:

```
time from the z=655 plane to the flag:
  human WR  1100 ms      rank 52   1103      rank 502  1102
  rank 8    1103         rank 152  1110      this TAS  1103
  rank 15   1103         rank 302  1106
```

**The closing sweeper costs everyone the same.** A 198 ms spread across the
field compresses to 10 ms over the final 1.1 seconds. 95 % of the entire
field's spread is already decided by t = 9.50 s — and this TAS's advantage is
banked there too (10 ms up on the world record at z=655, 7 ms at the flag).

The dramatic-looking final corner is not worth practising. That is the useful
result from this map.

## The finish trigger has an invisible edge — but it is not the pace-setter

The Goal on these CP1-End maps is a relocatable item, and sliding it sideways
brackets exactly where each run crosses. The trigger is a plane with a **finite
lateral window**, and its inside edge here is at world **x = 772.18**. The human
world record crosses 0.35 m outside it; one top-15 run passes only **5 cm**
outside it. Cut inside and the run does not finish — no partial credit, no
leaderboard entry, no feedback.

It is worth knowing that boundary exists. It is **not** where the time is:
measuring the clean margin for all top-15 runs at 10 cm resolution gives times
spanning 13 ms against margins spanning 1.40 m, **with no relationship**. The
tightest run in the field (5 cm from the edge) is 12 ms *slower* than the world
record; the widest (1.45 m) is 10 ms slower. This TAS crosses 0.24 m tighter
than the world record and loses 3 ms over the last 1.1 s doing it.

*(An earlier version of this page claimed "tighter is faster, ~10 ms per metre
of arc". That was arithmetic about a circle, not a measurement, and the
measurement contradicts it. Corrected.)*

## This route has zero open-loop tolerance

Quantising the steer trace to a step of 2 — a change of at most half of one of
255 steering units per tick — makes the run DNF. So does sample-and-hold at
2 ticks. **Both of those also kill the human world record's own tape**, and they
kill it mid-route, well before the gate.

That is a strong statement about the map: every input matters everywhere. It is
**not** a statement that a human cannot drive it — a test that destroys a run
561 people have on the board is measuring the fragility of a recorded tape
replayed blind, not human skill. A driver is a closed loop; they see the car
drift and correct on the next frame.

### The low-input ladder — measured, and it does not close

*(An earlier version of this page said no low-input family exists here. That was
concluded by **converting** the finished analog tape, which is the one method
measured not to work on any map in this collection. Searching **under** the
constraint from a compatible seed does work, and the corrected answer is below —
but keyboard still does not reach the author time, and this section says so.)*

| rung | seed | time | vs AT | events | alphabet |
|---|---|---|---|---|---|
| analog | — | **10.594** | **−0.004** | — | 255 |
| human WR, for scale | — | 10.602 | +0.004 | — | 255 |
| [16 detents](replays/DETENT16_10602.Ghost.Gbx) | analog champion | **10.602** | +0.004 | 264 | 32 |
| 8 detents | analog champion | 10.608 | +0.010 | 137 | 17 |
| 8 detents, reduced | " | 10.618 | +0.020 | **85** | 17 |
| [**5 detents = the real action-key ladder**](replays/ACTIONKEY_5detent_10643.Ghost.Gbx) | human r152 | 10.643 | +0.045 | 76 | 11 |
| [**keyboard**](replays/KEYBOARD_10636.Ghost.Gbx) | human r301 | **10.636** | +0.038 | 66 | 3 |
| [keyboard, 35 events](replays/KEYBOARD_35ev_10646.Ghost.Gbx) | human r152 | 10.646 | +0.048 | **35** | 3 |
| 5 detents | analog champion | 10.702 | +0.104 | 162 | 10 |
| keyboard / 4 detents | **converted** from the analog champion | **0% finish** | — | — | — |

**The 5-detent rung is not an arbitrary choice.** Every digital human on this
board uses only multiples of **20% lock** (observed 0.2 / 0.4 / 0.6 / 0.7961 /
1.0), so five detents *is* the action-key setup people actually have. The 8- and
16-detent rungs are finer than anything a human is observed to press — they are
here to bound the cost of the alphabet, as measurement rather than advice.

**Why it does not close, and it is the opposite of another map in this
collection.** This board is all pad at the top: ranks 1–15 have a median
non-zero steer change of 0.02–0.08 per tick, and **every digital human sits at
rank 152 or worse.** The digital deficit is diffuse carry — 1 to 4 km/h down at
every station from t = 8 s, with no single corner to fix.

The 16-detent rung's 4 ms was checked against the sub-millisecond vernier rather
than assumed to be a quantisation plateau: it is **genuinely 0.85 m behind along
the finish axis**, so it would not have fallen to a ratchet. A further 2.7 hours
and 1.66 M evaluations across three window widths bought **1 ms**.

**Still useful to that leaderboard:** action keys at 10.643 is **15 ms faster
than the best keyboard human on the board**, and pure keyboard at 10.646 is
12 ms faster **in 35 inputs**. Both would place top-150 of 561. Neither is the
author time.

## Other measurements

- Exchange rate at the gate: **10.5 ms per metre** (1 ms = 9.49 cm). All 27
  measured humans cross at the same terminal 341.7 km/h.
- **No shaping signal exists**: one waypoint means every failed run returns the
  same "reached 1 checkpoint", so a search cannot tell a near-miss from a
  catastrophe and cannot cross a DNF valley.
- Every single-tick throttle lift tested (ticks 2, 5, 10, 20, 40, 80, 150, 300,
  500) causes a DNF, as does throttle on tick 0.

## Validation

**27 of 27** downloaded human ghosts re-simulate to their exact leaderboard
millisecond. Every published tape returns exactly the time in its filename on
the untouched map.

The instrument used to get sub-millisecond resolution deserves a note: rather
than modelling a finish plane, the map's own Goal item is physically relocated
and the **plain oracle** re-run, so every number is the game's own body-based
trigger firing on the game's own physics. The ratchet built on it predicted the
untouched oracle correctly every cycle, including the ones that flipped the real map from 10.596
to 10.595 to 10.594.

**Why the ruler was necessary.** A plain search seeded with the human world
record reached the author time in **28 seconds** and 10.596 in seven minutes,
then stopped dead for 1.7 million evaluations. It had not run out of road, it
had run out of *resolution*: the car crosses at its terminal 94.9167 m/s, so one
reported millisecond is 9.49 cm, and the oracle's integer answer is quantised
into uneven bins up to **15 cm** wide — 10.599 is unreachable entirely. Any real
gain smaller than the current bin is invisible. The concurrent control makes the
point: a real-map arm sat at 10.596 for **41.9 minutes and 1,337,400
evaluations** while the ratchet went 10.596 → 10.595 → 10.594.

This map's work used no fork-resume path and distinct search roots throughout,
so none of the corruption defects found elsewhere in this project apply to it.

## Files

| file | what |
|---|---|
| `replays/real_10594.Ghost.Gbx` | **fastest validated run — 10.594** |
| `replays/real_10595.Ghost.Gbx` | the 10.595 |
| `replays/best_10596.Ghost.Gbx` … `best_10601.Ghost.Gbx` | the ladder of validated intermediates |
| `inputs/real_10594.tick.txt`, `inputs/real_10595.tick.txt` | the runs as readable input scripts |
| `notes/RESULT.md`, `notes/NOTES.md`, `notes/PLAN.md` | full write-up and measurements |
