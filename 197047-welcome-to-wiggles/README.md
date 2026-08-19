# Welcome to wiggles

**Wiggle slower than you think: hold each side for a quarter of a second — 250 ms
— keep it metronomic, and press respawn the instant you touch the far gate.**

| run | time | vs author time | vs human WR | steering |
|---|---|---|---|---|
| **TAS** | **95.839** | **−4.945** | **−5.955** | pad |
| **keyboard, two keys** | **96.412** | **−4.372** | **−5.382** | **`{−127, +127}`** |
| keyboard, metronome | 96.759 | −4.025 | −5.035 | `{−127, 0, +127}` |
| earlier tape | 96.852 | −3.932 | −4.942 | pad |
| Author time | 100.784 | — | −1.010 | — |
| Human WR — Beagle.3 | 101.794 | +1.010 | — | 229 values, pad |

TMX map [197047](https://trackmania.exchange/maps/197047) · author **CatBagasm**
· tags **Endurance, Race, Educational** · **22 recorded runs**.

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
cross the finish about 1.5 s later, and nothing you do in between matters. The
world record presses 75 ms after touching the gate; the best in the field presses
after 22 ms, and the difference is exactly the difference in their tails.
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
| `replays/TAS_95839_analog.Ghost.Gbx` | the fastest run |
| `replays/TAS_96852_v1.Ghost.Gbx` | the first tape under the author time |
| `inputs/KEYBOARD_96412_twokey.tick.txt` | the two-key run as an input script |
| `inputs/KEYBOARD_96759_metronome.tick.txt` | the metronome run as an input script |
| `inputs/TAS_95839_analog.tick.txt` | the fastest run as an input script |
