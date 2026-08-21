# Fall 2025 - 22 Reverse CP1 End

**Start accelerating on the second tick, not the first. It is worth 2 ms here —
and the only reason it works is that the later start opens a line through the
dip that does not exist from a first-tick start.**

The trick came from **Matik_K**, who holds the world record on this map and
asked whether we had tried it. We had not. His own record does not use it, and
neither did any of the 40 runs on the leaderboard.

**Fall 2025 - 22 reverse (CP1 end)** — keyboard **5.352** (+0.002) | AT 5.350 | WR 5.355

https://github.com/user-attachments/assets/94309fe3-6a88-430d-931c-abf09fba0a49

Single car: against the human 5.355 the two stay within 0.42 m for the whole run, so a side-by-side would show one car.

| run | time | vs author time | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, start trick** | **5.345** | **−0.005** | **−0.010** | analog |
| TAS, previous best | 5.347 | −0.003 | −0.008 | analog, 114 changes |
| TAS, earlier tape | 5.348 | −0.002 | −0.007 | analog |
| **TAS, keyboard only** | **5.350** | **±0** | **−0.005** | 15 changes, 3 steer values |
| **the drivable one** | **5.351** | +0.001 | **−0.004** | 19 changes, 7 steer values |
| TAS, keyboard, smallest | 5.352 | +0.002 | −0.003 | 11 changes, 3 values |
| Author time | 5.350 | — | −0.005 | — |
| Human WR — Matik_K | 5.355 | +0.005 | — | pad, 107 changes |
| Human rank 5 | 5.358 | +0.008 | +0.003 | keyboard, 11 changes |

## The start trick

Hold the throttle off for one tick — 10 ms — then floor it. That is the whole
technique.

**Why it works is not what it looks like.** The two starts are *positionally
identical for two full seconds*: same place, same speed, to the millimetre. The
difference appears coming out of the first 1→2 upshift, worth about 0.4 km/h,
and the car keeps it from there. So it is an engine-phase effect, not a launch
one — which means **it is worth checking per map**, since what you gain depends
on where your first gear change falls relative to the finish.

On a flat practice map built for the technique (TMX 325723) the whole
leaderboard splits on exactly this: seven runs at 2.590 start on the second
tick, five at 2.592 start on the first.

**The 2 ms here is not the launch.** Measured on a vernier fine enough to
separate three other runs at quarter-millisecond resolution, the two launches
are identical to within 0.25 ms at corner entry. What the later start buys is a
*different line through the dip* — a corner the first-tick car cannot take. The
ablations:

| edit | result |
|---|---|
| the 5.345, unchanged | **5.345** |
| put the first tick's throttle back, change nothing else | **DNF** |
| apply the winning steering move to the old 5.347 tape | **DNF** |
| revert the steering, keep the trick | 6.109 |

Neither half works alone. And a matched-budget control search from a first-tick
start, same machinery and same seeds, stayed flat at 5.347 through ~930,000
evaluations while the second-tick arm found 5.345 in three rounds.

TMX map [279218](https://trackmania.exchange/maps/279218) · author **in-.-** ·
339 recorded runs. It is a reversed copy of the official Fall 2025 - 22 layout.

## The map

201.5 m in 5.355 s. A road start, no checkpoints, and a free-standing finish
gate. **Gas is on and the brake is off on every tick of all 40 downloaded human
runs** — steering is the only control that does anything. The whole map is one
long full-lock left-hander into a short straight.

Two measurements shape everything:

- **1 ms = 1.7 cm at the finish gate.** The gate times a plane crossing at
  17.0 ms per metre, so the whole 0.005 gap to the author time is **8.5 cm of
  reach**. This is a geometry problem, not a timing problem.
- **All the dispersion is in the corner, 110–190 m.** Ranks 1 through 344 are
  within a few milliseconds and about a metre of each other over the first 90 m.
  Nothing is to be gained in the first third of the map.

## Where the time is

Almost the whole field drives the corner at **full lock** — `steer = −1` held for
about 165 consecutive ticks. Full lock at 200 km/h scrubs: the car pins at
**203.15 km/h** for roughly a second in the middle of the turn, which is the
grip/drag equilibrium *at that steering angle*. Less lock means less scrub and a
higher plateau, as long as the car still makes the exit.

The world record is the one run that does not hold full lock — it uses
intermediate steering values through the corner — and it does not win by being
fast through the turn. **Among the top 15, ranks 5 and 9 are about 0.002 ahead
of the record for most of the corner and give it all back in the last 20 m.**
Slow in, fast out: the record wins on **exit**.

So the time lives in the last third of the corner and the run onto the finish
straight, traded against how much lock you carry through the middle. Speed over
the exit climbs 209 → 213 km/h into the gate plane.

## The run, as inputs

Full throttle from the lights, brake never touched, three keys total. This is
the 5.350 keyboard tape.

```
race  0.120  RIGHT            | hold through the opening bend
race  0.660  release          |
race  0.850  RIGHT ~70 ms     | a short tap, then let go
race  1.640  RIGHT ~50 ms     | the shortest tap of the run
      ...                     | hands off for a full second down the descent
race  2.760  RIGHT            | hold into the approach
race  3.130  release 1 frame  | THE FLICKER — off and straight back on
race  3.400  release          | coast ~100 ms
race  3.500  FULL LEFT        | the corner — hold it to the line
race  3.910  throttle off     | THE LIFT — 20 ms, steering unchanged
race  5.360  release          | free either way
```

**Steps at 3.13 s and 3.91 s are worth 0.008 together and are a package.** The
flicker alone is worth 0.003; the lift alone *loses* 0.018. Each half is
punished when tried on its own, which is why a person grinding this map never
stumbles onto them. Practise them as one move or not at all.

### Sector by sector, off what you can see

**Launch (0 → 1.75 s).** Standing start, straight and slightly falling,
0 → 100 km/h. Two short right taps to hold the line. Nothing to win here.

**The descent (1.75 → 3.50 s).** Still straight, the road dropping away,
100 → 176 km/h. The whole leaderboard is within a metre of each other here. Hands
off after the 1.64 s tap, then the long right hold from 2.76 s as the corner
comes up — with the single-frame flicker in the middle of it.

**The corner (3.50 → 5.20 s).** One continuous left-hander. The car dips, then
climbs back, the heading swings through a quarter turn, and the speedo sticks at
203 km/h. There is a brief moment of air at about 4.80 s. The throttle lift lands
about 400 ms into this hold, steering unchanged.

**The exit (5.20 → 5.355 s).** Straight, steering released, speed climbing again
into the gate. This short stretch is where the record was won over ranks 5 and 9.

## How forgiving it is

Measured the pessimistic way: move one input boundary by a single frame, change
nothing else, and see whether the run still finishes within 0.050 of its own
time. Recovering with the next input is not allowed, so these are floors.

| tape | inputs | survives **both** directions | at least one |
|---|---|---|---|
| **TAS 5.347, analog** | 114 | **62 %** | 81 % |
| **the drivable 5.351** | 19 | **42 %** | 95 % |
| human WR 5.355 | 107 | 24 % | 85 % |
| human rank 5, 5.358 | 11 | 27 % | 45 % |
| keyboard 5.350 | 15 | 7 % | 60 % |
| keyboard 5.352 | 11 | 0 % | 82 % |

Two things fall out. **The analog TAS line is the most forgiving thing on this
map**, more than twice as tolerant as the world record — it is not a knife-edge.
And **fewer inputs is not safer here**: the keyboard tapes are the twitchiest of
the lot, and the fair comparison for them is the human keyboard lap they came
from, which is only a little better. A keyboard lap on this map is intrinsically
twitchy, for the human too.

So the tape to actually learn is **the drivable 5.351** — 42 % two-sided on 19
inputs, still 0.004 inside the human world record.

### Which way to fail

Every input of the 5.350 tape moved 10 ms each way:

| moment | 10 ms EARLY | 10 ms LATE |
|---|---|---|
| 0.660 release right | 5.733 | **DNF** |
| 0.850 tap right | **DNF** | 5.728 |
| 2.760 hold right | 5.391 | **DNF** |
| 3.130 the flicker | 5.709 | 5.376 |
| 3.400 release right | **DNF** | 5.386 |
| 3.500 turn in left | 6.288 | 5.371 |
| 3.910 the throttle lift | 6.018 | 5.371 |
| 5.360 final release | 5.350 | 5.350 — free |

**The direction alternates**, so there is no single "when in doubt" rule on this
map: the early right taps want to be late, the holds want to be early. What is
worth carrying into muscle memory is the end of it — **the turn-in and the lift
both cost about 0.65 s early and about 0.02 s late. At the corner, late is cheap
and early is ruinous.**

**Realistic expectation.** This is a 5.35-second sprint in which one mistimed
frame costs 0.4 s or the run, and the two blips are one and two frames long. A
keyboard player who gets everything but the blips still has the rank-5 lap of
5.358, which is 11 inputs and already inside the field — **the blips are the last
0.008, not the technique.**

## Is the author time drivable?

The medals are 5.350 / 6.000 / 7.000 / 9.000 — gold, silver and bronze are round
thousands, i.e. placeholders, so the author time is not derived from them. The
author also sits rank 5 on their own board with 5.358, eight milliseconds slower
than the time they published. That reads as a lap driven in the editor by
someone who could retry indefinitely, which is evidence rather than proof — the
lap itself is not stored in replayable form, so nobody can check it.

Nothing here depends on settling it: the drivable tape is measurably more
forgiving than the driving humans have actually done on this map.

## Files

| file | what |
|---|---|
| `replays/TAS_5345_starttrick.Ghost.Gbx` | **the fastest run** — the start trick plus a corner line only it can reach. ⚠️ **This file's telemetry is Matik_K's 5.355, not its own** — the 5.345 is real and re-simulates on the oracle, but the recording plays back as the human's run, so do not load it as a replay until it is regenerated. Every other file on this page carries its own trajectory. |
| `replays/DRIVABLE_5351_5detents.Ghost.Gbx` | **the one to hand a person** — 42 % two-sided, still inside the human WR |
| `replays/KEYBOARD_5350_equals_AT.Ghost.Gbx` | **the author time on three steer values** — a human's own lap plus two blips |
| `replays/KEYBOARD_5352_11events.Ghost.Gbx` | the smallest tape that still beats the human WR |
| `replays/best_pF_5347_32087.Ghost.Gbx` | the fastest run, analog |
| `replays/best_pC_5348_32098.Ghost.Gbx` | an independently produced 5.348 |

Every tape above re-simulates to the time in its filename.
