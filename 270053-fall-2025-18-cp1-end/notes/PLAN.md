# Map 270053 — `Fall 2025 - 18 CP1 End` — working plan and state

uid `6r7HjKPCuImnLMBfqiKwWpGK1U1` · TMX id 270053 · author `in-.-` (Uruguay)
**AT 4492 ms · best human ever 4495 ms over 973 records · AT never beaten.**

## STATUS (2026-08-19T01:20Z)

**4492 ms validated — the author time EQUALLED, the human record beaten by 3 ms.**
Not yet beaten outright; 4491 is the target.

Banked in this directory:

| file | what | validated |
|---|---|---|
| `map18_270053.Map.Gbx` | the map, from trackmania.exchange | loads |
| `ghosts/p000NN_TTTT.Ghost.Gbx` | the 15 fastest human runs | all 15 re-simulate to their exact leaderboard ms |
| `tas_4492_v1.Ghost.Gbx` | **the result** | 4492, three times, cold |
| `tas_4493_singletick_v1.Ghost.Gbx` | the single-tick-only optimum | 4493 |
| `validation_transcript_v1.txt` | cold run incl. known-answer controls | — |
| `tools/tmlayer-src-v1.tgz` | the search tool written for this map | — |

## The map, measured

- 450 race ticks (4.5 s). Tape is 650 ticks including a 152-tick countdown.
- **One waypoint only**: a custom item `cp1end\sausagecpfin.Gbx.Item.Gbx`. No
  intermediate checkpoints, so no segment maps and no shaping are possible or
  needed.
- Full throttle from lights to line, **no brake, no lift, ground contact the
  whole way**. Long downhill: y 132.5 -> 114.7. Speed 0 -> 216 km/h monotonic.
- Geometry: one long left-hand sweep (start heading -x, finishing heading +x),
  an 86 m net displacement in +z, arriving back at the start's x.
- **The car never slides**: side speed is ~0.2 m/s all the way round. The corner
  is STEERING-ANGLE limited, not grip limited — full lock is held for ~128 ticks
  and the arc through it is geometrically forced.
- Oracle throughput on 176 cores: **2500 candidate simulations/second**, 100%
  of single-tick perturbations finish. A whole-tape exhaustive single-tick sweep
  (450 ticks x 255 steer values = 114,751 sims) costs **46 seconds**.

## What produced the 4492

Seed: the human WR (`p00001`, 4495). Two mechanisms, in order:

1. **Exhaustive single-tick sweep** (every tick, every one of the 255 steer
   values): 4495 -> 4493 in 80 seconds. Then it dies: zero improving single-tick
   or block edits exist at 4493.
2. **A raised-cosine bump family** — smooth shape changes 10-320 ticks wide —
   found 4492. The winning edit was a +30 bump 40 ticks wide at tick 561.

Physically it is one manoeuvre: **release the steering at the corner exit about
0.19 s earlier than the human does, and unwind it progressively rather than
holding full lock to the last moment.** Human holds -127 to race 4350; the TAS
starts unwinding at 4160 and is at -52 by 4300, then flicks right sooner. Every
other tick of the tape is within a few units of the human record.

## The instrument that made it possible: a sub-tick vernier

The oracle reports an integer millisecond, so thousands of distinct candidates
tie and the search sees a plateau. `tmmaps gateshift` translates the goal ITEM
(keeping its model, so the author's own trigger geometry is preserved) a few
centimetres back along the direction of travel. Measured exchange rate on this
map: **1 ms = 8.75 cm of gate travel** near the crossing (11.4 ms/m).

- A ladder of shifted gates ranks tied candidates. **Auto-calibrated** each
  round — binary-searching the incumbent's own crossing and re-aiming the ladder
  at it — because a fixed ladder goes blind after one accepted edit.
- **Two-sided** (half the rungs above the incumbent's crossing, half below), or
  a worse candidate is indistinguishable from an equal one and a beam search
  sorts by array index and marches into full lock.
- **Cascaded**: a candidate that fails the easiest rung never needs the rest.
- Current resolution: 12 rungs over 0.030 m = **29 microseconds**.

Current incumbent's true crossing: **4492.88 ms**. To report 4491 it must lose
0.89 ms = 7.8 cm of progress at the line.

## What has been ruled out, with numbers

| idea | result |
|---|---|
| every single-tick steer value, whole tape | 0 improving at 4493 and at 4492 |
| uniform blocks 2-34 ticks, +-8 | 0 improving |
| throttle lifts 1-90 ticks, anywhere | 0 improving; long lifts finish only 23% |
| braking 1-60 ticks, anywhere | 0 improving; finish only 30% |
| scale/shift/doublet families | only sub-microsecond nudges |
| **the entire corner exit as a 5-parameter shape** (169,793 candidates: release tick x rate x level x flick tick x flick rate) | **0 improving; 54,777 of them tie at 4492** |
| lateral margin at the finish trigger | none: the gate has a hard edge, the run already crosses at its earliest point, +-0.5 m either way is slower and +2 m does not finish at all |

The exit is solved. Whatever is left is created **before** tick 545.

## Open lines

1. **Multi-start** from all 14 other human seeds with the full operator set —
   running. The human field's early phase is NOT converged (7 degrees of yaw
   spread at race 1000 ms) even though the lines converge later, so the seeds
   are genuinely different basins.
2. **The entry** (ticks 272-440, race 1200-2880): the only stretch with real
   steering freedom left. Wants the same near-exhaustive parametric treatment
   the exit got.
3. **Beam sweep** with the two-sided vernier — built and working, ~35 s/layer.

## Is the AT legitimate?

Yes, so far as the evidence goes. The map header says `validated="1"` and the
author drove it, so a human-sized 4492 exists. Our own independently-found 4492
is a different tape from any human's and lands in the same millisecond, which is
corroboration rather than coincidence. No evidence of a physics-build mismatch
has appeared: all 15 downloaded human ghosts re-simulate to their exact recorded
milliseconds on the current server build.
