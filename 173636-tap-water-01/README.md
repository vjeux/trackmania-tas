# Tap water 01

**This map is one number: the fraction of the descent you spend with the
accelerator down. Push it toward 67 %, and tap through the drop-in as well as the
ramp — the first 4.7 seconds are worth more than the whole nineteen-second
glide.**

| run | time | vs author time | vs human WR |
|---|---|---|---|
| **TAS** | **22.072** | **−1.253** | **−1.566** |
| TAS at a 40 ms input grain | 23.125 | −0.200 | −0.513 |
| 1-minimal, 747 input events | 23.183 | −0.142 | −0.455 |
| **a uniform 2-on/1-off tap, nothing else** | **23.335** | +0.010 | **−0.303** |
| Author time (never beaten by a human) | 23.325 | — | −0.313 |
| Human WR — Reddnox, who is also the author | 23.638 | +0.313 | — |

TMX map [173636](https://trackmania.exchange/maps/173636) · author **Reddnox** ·
**602 recorded runs** — one of the most hunted maps here.

## What the map is

One straight 1:2 ramp at 26.6°, no checkpoints, a 191 m drop, and essentially
one dimension: the whole field's lateral spread is about 18 m of side-to-side
over a 400 m descent. Speed saturates at 89–94 km/h and the gear never leaves
first. It is a very high-drag, low-grip surface, which is what the name and the
Underwater tag are about.

Steering through the glide is **provably inert** — zeroing it over the *entire*
glide returns the identical millisecond. There is nothing to drive. There is only
the accelerator.

## What the field does wrong

Throttle duty over the glide orders the whole leaderboard almost perfectly:

| rank | throttle duty |
|---|---|
| 1 | **66.9 %** |
| 20 | 58.5 % |
| 120 | 48.1 % |

**Nobody exceeds 67 %.** They cannot simply hold it down: forcing the
accelerator on for even 0.2 s in the middle of the glide ends the run, while
forcing it *off* is merely slow. That is a traction limit, and it is the whole
map — find the highest duty the surface tolerates.

**Tap rate orders nothing**, and this is the trap:

| | duty | presses per lap |
|---|---|---|
| rank 3 | 66.6 % | **24** |
| rank 9 | 60.0 % | **381** |

Sixteen times the input, less duty, and 0.700 s slower. Anyone chasing "tap
quicker" is optimising the wrong variable.

**And the part everybody optimises is not where the time is.** All 30 sampled
records drive the same line, including the author's own validation lap — his
23.325 is 0.313 better than his own online record because he caught a better duty
and phase, not because he did anything different. Against the world record:

| step | time | won |
|---|---|---|
| human WR | 23.638 | — |
| + a uniform tap over the glide | 23.335 | 0.303 |
| + optimised glide throttle | 23.112 | 0.223 |
| + optimised **first 4.7 s** | 22.277 | **0.835** |
| + further rounds on both | 22.072 | 0.205 |

**Perfecting the entire nineteen-second glide is worth 0.526 s. Modulating the
throttle through the drop-in is worth 0.835 s.** Every sampled human, the author
included, goes into the drop with an essentially unmodulated throttle.

## The run, as inputs

The fast tape drives the world record's own start line — it only modulates the
throttle through it:

```
race  0.150  full RIGHT        | off the line
race  1.370  full LEFT
race  1.620  gas + brake        | the scrub, both together
race  2.370  full RIGHT
race  2.340 – 3.090             | the drop-in — TAP THROUGH IT. This is the 0.835 s.
race  4.600  onward             | the glide: steering does nothing, tap for duty
```

About 0.580 s of that 0.835 s is pure throttle timing; the rest needs the
steering to move with it.

**The tap that wins, and it needs no search at all:** two ticks on, one tick off
— 20 ms down, 10 ms up, 33 Hz, 66.7 % duty — held over the glide. That metronome
alone is 23.335, which beats every one of the 602 humans on the board, and it
lands 0.010 outside the author time.

**Drivable advice, in descending order of value:** tap through the drop-in and
the first second of ramp, not just on the ramp; push glide duty toward 67 % by
holding longer and releasing shorter, at whatever rate stays phase-stable; and if
a slow rhythm — about 1 s on, 0.5 s off, which is how ranks 3 and 4 drive — gives
you a steadier duty than a fast one, use it, because rate itself buys nothing.

## How forgiving it is

**Coarse timing is fine. Phase is brutal.**

How coarse the tap may be, and whether the author time still falls:

| input grain | best time | inside the author time? |
|---|---|---|
| 20 ms | 23.272 | no |
| 30 ms | 23.173 | no |
| **40 ms** | **23.125** | **yes** |
| 50 ms | 23.578 | no |
| 100 ms | nothing finishes at all | — |

**The author time still falls at a 40 ms grain**, which is inside what the field
already does with its hands — rank 5's own medians are 50 ms on, 70 ms off.
Below 100 ms of grain, no rhythm gets down the ramp at all.

Phase and start point are the tight part. On the winning metronome, shifting the
pattern by one tick costs 0.141 s and shifting it the other way does not finish.
Starting it at 4.300 s costs 0.119, at 4.900 s costs 0.153, and starting it
before 4.300 s does not finish.

**What will take real practice:** nothing about the line, everything about the
rhythm. There is no hidden route, no unfired feature and no attitude trick here —
this is precision on a technique the map's own name tells you to use. The extra
second from 23.125 down to 22.072 does need 10 ms control, and that part is
machine work.

## Files

| file | what |
|---|---|
| `replays/TAS_22072.Ghost.Gbx` | the fastest run |
| `replays/UNIFORM_2on1off_23335.Ghost.Gbx` | **the metronome — beats the world record with no search at all** |
| `replays/GRAIN40MS_23125.Ghost.Gbx` | inside the author time at a 40 ms input grain |
| `replays/DDMIN_747ev_23183.Ghost.Gbx` | under the author time on 747 input events |
