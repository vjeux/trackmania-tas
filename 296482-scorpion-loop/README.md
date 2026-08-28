# Scorpion Loop

**A 5.5 minute course with 18 records, and the world record falls to brake
pulses in the final segment.**

**Scorpion Loop** — TAS **339.216** | AT 331.908 | WR **330.222** by SmithyTM

https://github.com/user-attachments/assets/90c835c9-d702-4612-bf1c-c81492e52850

*Five and a half minutes, two cars, with this run own inputs drawn on. Both of the record holder crashes are gone.*

> **2026-08-28 — this map is no longer unbeaten, and not by us.** SmithyTM
> improved the world record three times in one day — 349.453 → 336.963 →
> **330.222** — and that last one is **1.686 s inside the author time**. When
> we started, the record was Quantiks' 349.453 and no human had beaten the
> author time. Our 339.216 beat the old record by 10.237 s and is now 9.006 s
> behind the new one. **The clip below is our run against Quantiks, the record
> that stood when it was filmed.**

**Both of the world record two crashes have been cut out.** The author time is **not** beaten — and it is **plugin-set**, so there is no
evidence anybody ever drove it. The human world record is beaten by **10.237**, and we are **1.704 under the best
human pace with his mistakes arithmetically deleted** (341.103).

## Removing the crashes: splice on STATE, not on time

The record contains two respawns — race **52.690** (seg03) and **304.750**
(seg16) — costing **3.510 + 4.840 = 8.350 s** between them. They are not driving
skill to out-perform; they are dead time a human paid for two mistakes.

`ghost splice --rule retries` cuts at the CHECKPOINT, which is matching an
*instant*: on seg03 that puts the junction **40.38 m** from where the car
actually was. Matching on **STATE** — position, velocity and attitude together,
scored separately — puts it at **0.392 m**.

- seg03: cut 452 ticks -> 344.229
- seg16: cut 484 ticks -> **339.399**

**The cut LENGTH is the whole answer and the start point is irrelevant.** Twelve
start points on seg03 and eight on seg16 give byte-identical times, and a
2,356-cell sweep says 452 is the only length that works.

## The whole lap is a tightrope

**51 % of this lap (17,512 of 34,163 ticks) rejects every input change we can
make.** Not unexplored - measured, with positive controls:

| probe in the dead regions | result |
|---|---|
| `steer` +/-24 over 40 ticks, 20 sites | **40/40 DNF** |
| `steer` +/-1 and +/-2, 3 sites | **12/12 DNF** |
| throttle-off 12 ticks / brake 8 ticks, 6 sites | **24/24 DNF** |
| the same short pedal edits at a LIVE site | **339.401 / 339.399 - they finish** |

So it is not magnitude and not the lateral channel: **a single tick of any input
either side of the line loses the lap.** The failure deepens along the route -
`cps None` before tick 11000, `cps 12-14` by tick 23000 - the car gets
progressively further before it dies.

No operator family ever placed candidates in that half because everything that
tried died immediately and the region dropped out of every search. **The 49 %
that responds is not where we happened to look; it is the only part of the lap
that admits input at all.**

On that responsive half, **six operator families over ~220 oracle cells** -
smooth bump, compound two-bump, pedal delta, asymmetric delta, multiplicative,
and a discrete generator sampled from the incumbent's own change statistics
(one change per 2.78 ticks, 68 % single-tick holds, 63.6 % negative). Closest
approaches 339.224 and 339.226. **339.223 is a local optimum for every operator
family and both input channels.**

## What does NOT work, measured

Grafting *another driver* inputs in fails everywhere, however well the state
matches. Tested with the confound removed — same tick index, zero time shift:
tick 26890 -> `cps 14`, 20000 -> `cps 10`, 10000 -> `None`. **Eleven handovers,
states matched to 0.017 m, every recorded channel equal**, including pairs at
zero engine-load difference. All fail.

**A human input tape is a closed loop around their own trajectory** — 10 ms of
someone else steering puts the car where their next 10 ms does not expect it.
But **the same driver own earlier lap IS a legitimate donor**, subject to exact
tick alignment: our tape is Quantiks inputs with two cuts in it, and grafting
his tail back onto it returns `cps 16` at every point tested.

So the ~6.4 s of field-best slack is a **bound, not a plan** — reachable only by
search, never by assembly.

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
