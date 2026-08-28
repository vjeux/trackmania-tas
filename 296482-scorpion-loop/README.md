# Scorpion Loop

**A 5.5 minute course with 18 records, and the world record falls to brake
pulses in the final segment.**

**Scorpion Loop** — TAS **348.739** (+16.831) | AT 331.908 | WR 349.453 by Quantiks

https://github.com/user-attachments/assets/d359cdda-2ae1-4990-8ec1-8795a96cad8e

*Five and a half minutes, two cars, with this run own inputs drawn on.*

The author time is **not** beaten — and it is **plugin-set**, so there is no
evidence anybody ever drove it. The human world record is beaten by **0.714**.

## The map

17 checkpoint segments, 11 892 m of driving, no repetition — segment durations
run 11.1 s to 29.8 s. Tagged Trial, but it is not a completion problem: the
world record contains **two respawn presses**, so there are no failed attempts
to delete. Cutting every retry from every record in the field leaves the best
human at **341.103**, still 9.195 over the author time. **The time has to be
driven.**

## Where the field is soft

Six of the 17 segments have measured slack, and four of those are best-driven
by ranks 3-4 rather than by the record holder:

| segment | WR (retries cut) | field best | WR gives away | who has it |
|---|---|---|---|---|
| seg15 | 29.843 | 26.345 | **3.498** | SmithyTM |
| seg17 | 16.545 | 15.163 | 1.382 | Gazorpalse. |
| seg03 | 13.115 | 12.218 | 0.897 | winged_TM |
| seg08 | 14.057 | 13.246 | 0.811 | SmithyTM |
| seg10 | 16.635 | 16.107 | 0.528 | SmithyTM |
| seg14 | 19.766 | 19.344 | 0.422 | SmithyTM |

Summing the best human on every segment gives **332.990 — only 1.082 over the
author time.** The field has collectively demonstrated almost all of it.

## Segments do not compose

Grafting SmithyTM segment-15 inputs onto Quantiks run at checkpoint 14
re-simulates to **DNF at cps 14**: the handover is clean and the driving after
it is not. Control: the same graft path with Quantiks onto himself returns
349.453 exactly. So the composite is a target, not a plan — each segment has to
be **re-driven from the state the previous one leaves**.

## Segment 15 rejects everything

That is where the biggest human gap sits, and it is the one place nothing works:

- 208 single-move probes across four operator classes: **0.000 s**
- then 20 520 evals across compound depths 3/5/6/8: **0 improvements**
- the identical operator on segment 17, same seed and clock: **29 improvements,
  28 distinct times**

Every surviving candidate in segment 15 returns the incumbent time to the
millisecond. The mechanism: **depth trades survival for expressiveness**, and it
pays only where the region has structure to find.

## Where the 0.714 came from

Brake pulses in the final segment, monotone in length and coherent across
neighbouring start times: 349.453 → 348.930 → 348.846, then **348.739** out of
the segment-17 control batch. Validated 5x on the untouched map.

## Files

`replays/tas_348739_regen.Ghost.Gbx` — oracle 348.739 (3x independent), kappa
1.000 at lag 0 over all 6 975 samples, first in-race sample this map own spawn,
identity neutralised.
