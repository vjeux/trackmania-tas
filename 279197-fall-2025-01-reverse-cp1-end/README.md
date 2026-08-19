# Fall 2025 - 01 Reverse CP1 End — author time beaten by 3 ms

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **10595 ms** | **−3** | **−7** |
| earlier validated tapes | 10596, 10597, 10598 | — | — |
| Author time (never beaten by a human) | 10598 ms | — | −4 |
| Human WR | 10602 ms | +4 | — |

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

No low-input family is published for this map, because none of the
simplifications survive. Saying so is better than dressing up a tape that does
not work.

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
untouched oracle correctly four times out of four, including the cycle where
pushing past the real plane correctly flipped the real map from 10596 to 10595.

This map's work used no fork-resume path and distinct search roots throughout,
so none of the corruption defects found elsewhere in this project apply to it.

## Files

| file | what |
|---|---|
| `replays/real_10595.Ghost.Gbx` | fastest validated run |
| `replays/best_10596.Ghost.Gbx` … `best_10601.Ghost.Gbx` | the ladder of validated intermediates |
| `inputs/real_10595.tick.txt` | the run as a readable input script |
| `notes/RESULT.md`, `notes/NOTES.md`, `notes/PLAN.md` | full write-up and measurements |
