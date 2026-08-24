# Welcome to wiggles

**Wiggle slower than you think: hold each side for a quarter of a second — 250 ms
— keep it metronomic, and press respawn the instant you touch the far gate.**

**Welcome to wiggles** — TAS **95.839** (−4.945) | AT 100.784 | WR 101.794 by Beagle.3

https://github.com/user-attachments/assets/547aa511-63e2-4eee-a4b3-018f95d9a168

**Our 95.839 and Beagle.3's world record, both in one camera, shot on our car.**
A hundred seconds of the wiggle, side by side. They separate steadily and
enormously — **591.5 m** at the widest, mean **20.7 m**, with 1690 of 1917
samples in the band where both read as cars — because 5.955 s of margin on a
22 km/h crawl is most of the straight. His ghost is drawn until 101.100 and ours
stops at 95.850, so the clip is **trimmed at our own last live frame**, 95.867.

Filmed from `replays/gen_TAS_95839_analog.Ghost.Gbx`, generated from our input
tape through the engine.

> ### NOT re-shot in the 2026-08-24 sweep — the regeneration cannot write this map's orientation
>
> Every other map in that sweep was regenerated from its own inputs on the
> current pipeline and re-filmed. This one was refused, four times, on the same
> check: **byte 59, the orientation angle, comes out `0x00` on all 1917
> samples**, where `replays/TAS_95839_analog.Ghost.Gbx` has it live and varying
> from the second sample (`253, 233, 189, 124, 64 …`). A dead orientation on a
> map that is nothing but facing is not something to ship.
>
> **It is this map, not the pipeline, and the control says so.** On Torment
> (1-UP) the regeneration reproduces the published file's orientation block —
> bytes 59–62 — **byte for byte on every sample**. Nineteen maps in that sweep
> regenerated and shipped clean. The tell here is in the locate: the accepted
> anchor on this map reports `pos +196, quat +252`, with the quaternion **56
> bytes AFTER** the position, where every map that works puts it 16 to 36 bytes
> **before** (`+180`, `+192`, `+160`, `+44`).
>
> **`--quat-kind` silences the check without fixing anything, and that is worth
> knowing before someone reaches for it.** Kinds 1 and 2 both pass with zero
> refusals and a trajectory 0.0000 m from this file's. Their orientation
> reproduces *nothing*: at 0.050 the published file reads `253 127 255 255`,
> kind 1 reads `0 128 0 0` and kind 2 reads `25 85 0 0`. The default at least
> fails loudly. **A flag that turns a refusal into a pass is not a fix unless
> the thing the refusal was about got better**, and here it did not.
>
> So this page keeps the clip it had. What would settle it is the answer key
> this project already uses elsewhere: regenerate a DOWNLOADED recording of
> this map (`ghosts/rank01_101794`) through the same path and grade its
> orientation against its own recorded bytes.

**The facing is checked, because facing is what put the last clip on this page
in the bin.** The withdrawn file's spawn attitude reads **|dot| 0.0000** against
Beagle.3's — a rotation 180° from the human spawn, on a map that is nothing but
sliding sideways for a hundred seconds — and this file reads **|dot| 1.0000**.
Same map, same reference, same instrument: a positive control on exactly the
defect that caused the withdrawal.

The opponent is Beagle.3's downloaded rank-1 recording, board re-pulled the day
this was filmed, and the comparison was run against **that** file rather than
taken from the tape's certificate.


| run | time | vs author time | vs human WR | steering |
|---|---|---|---|---|
| **TAS** | **95.839** | **−4.945** | **−5.955** | pad |
| **keyboard, two keys** | **96.412** | **−4.372** | **−5.382** | **`{−127, +127}`** |
| keyboard, metronome | 96.759 | −4.025 | −5.035 | `{−127, 0, +127}` |
| earlier tape | 96.852 | −3.932 | −4.942 | pad |
| Author time | 100.784 | — | −1.010 | — |
| Human WR — Beagle.3 | 101.794 | +1.010 | — | 229 values, pad |

TMX map [197047](https://trackmania.exchange/maps/197047) · author **CatBagasm**
· tags **Endurance, Race, Educational** · **27 recorded runs** (board
2026-08-24; the field measurements on this page were taken over the 22 recorded
then).

## What the map is

Not an endurance course. It is 100 seconds of **the wiggle**: gas and brake held
*together*, steering flipped full-left / full-right, creeping a car that cannot
otherwise drive along 620 m of flat straight at 242 m altitude, at 22 km/h. Two
checkpoints — one 6 m from the spawn, one at the far end — and nothing in between
to cue off. "Educational" means it teaches exactly one thing.

So there is no trick at a feature. The objective is the mean speed of a periodic
rhythm, which is why the margin is so large: a technique found once applies 400
times over.

## Where the time is: the whole field wiggles about 15% too fast

The world record's median half-cycle is **21 ticks**. The fastest rhythm is
**25 ticks — hold each side 250 ms**, two flips per second instead of the record
holder's 2.4. Measured over 120 m of steady-state corridor with a perfect
metronome at each rhythm:

| half-cycle | speed |
|---|---|
| 200 ms | 6.24 m/s |
| 210 ms | 6.33 |
| 220 ms | 6.42 |
| **250 ms** | **6.62** |
| 290 ms | 5.33 |

The second loss is consistency. Only **53% of the world record's own flips land
within one tick of its own median**; the tapes above are dead constant, and it
shows in the sector times:

| tape | time per 40 m sector | speed |
|---|---|---|
| human WR | 6.403 – 6.784, wandering | 5.9 – 6.25 m/s |
| keyboard metronome | **6.230 every sector, ±0.003** | 6.42 m/s |
| the 95.839 tape | **6.030 every sector, ±0.003** | **6.63 m/s** |

**Amplitude is irrelevant.** Steering at 70, 90, 110 and 127 of full lock over
the same rhythm lands within 0.001 of itself. A keyboard is not a handicap on
this map; two keys are the entire alphabet you need, and the leaderboard's own
keyboard runs already flip straight from one lock to the other without passing
through zero on about 90% of flips.

**The last 1.5 seconds is a respawn, and most of the field is late on it.** The
far gate is not the end: you respawn there, get teleported to the start line and
cross the finish about 1.5 s later, and nothing you do in between matters. Our
run presses at **94.250**, the instant it touches the gate; the car returns to
within 6.6 m of the spawn, sits still for 1.05 s, then drives across the finish.
The world record presses 75 ms after touching the gate; the best in the field
presses after 22 ms, and the difference is exactly the difference in their tails.
Pressing on the first tick is worth about 0.075 for free.

## The run as inputs

The map has no features to cue off — that is the point of it — so the cues are
the countdown, the gate, and a count in your head.

1. **Start → the checkpoint gate (0 → ~0.8 s).** Full gas, no brake, straight
   down the platform. The gate is 6 m away; you are through it immediately.
2. **The run-up (0.8 → ~1.9 s).** Keep full gas. The car reaches ~100 km/h and
   then the surface takes it away — you feel it stop pulling about a car-length
   past the gate structure.
3. **Enter the wiggle (~1.95 s).** **Add the brake and keep the gas.** Both held
   for the next ninety-eight seconds. Never release either — one tick of release
   kills the run.
4. **The wiggle (2 → ~94 s).** Alternate full left and full right, **a quarter of
   a second each side**. Two flips per second: a 120 bpm metronome with a flip on
   every beat and every off-beat. Steer strength does not matter, so use whichever
   input you can time better.
5. **Hold the line.** The corridor is a few metres wide and the car creeps
   sideways. Correct with the *length* of a half-cycle, not with a partial steer:
   lengthen the side you want to come back from by 10–20 ms, then go straight back
   to the rhythm.
6. **The far gate (~94 s).** The instant you touch it — not after you have read
   the split — **press respawn.** Every millisecond you wait is a millisecond on
   your time.

## How forgiving it is

Measured by mistiming one flip and keeping the spacing of every flip after it,
which is what a driver actually does:

- **53% of the 431 flips take ±30 ms of mistiming for free**, and a mistiming
  that does not kill the run costs between −0.005 and 0. Several are marginally
  faster than the nominal tape.
- **Sensitivity decays with how much track is left for an error to grow.** In the
  first 35 seconds a bad correction is expensive; after the first minute, no
  single mistimed flip can lose the run at all.

So the shape is friendly: the part that punishes error is the part you practise
most. The rhythm itself is the easy half. **What will take real practice is
holding it for 92 seconds without a drift that walks you off the edge** — which
is exactly what this leaderboard is already struggling with, and why it has 22
entries rather than 900.

## Files

| file | what |
|---|---|
| `replays/KEYBOARD_96412_twokey.Ghost.Gbx` | **two keys, `{−127, +127}`** — the one to practise |
| `replays/KEYBOARD_96759_metronome.Ghost.Gbx` | three-value metronome variant |
| `replays/TAS_95839_analog.Ghost.Gbx` | the fastest run — the tape whose recorded facing is wrong; the inputs and the time are unaffected |
| `replays/TAS_96852_v1.Ghost.Gbx` | the first tape under the author time |
| `inputs/KEYBOARD_96412_twokey.tick.txt` | the two-key run as an input script |
| `inputs/KEYBOARD_96759_metronome.tick.txt` | the metronome run as an input script |
| `inputs/TAS_95839_analog.tick.txt` | the fastest run as an input script |
