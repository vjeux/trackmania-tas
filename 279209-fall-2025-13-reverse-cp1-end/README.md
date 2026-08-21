# Fall 2025 - 13 Reverse CP1 End

**On the ice run-down, about 1.7 seconds in, where the slope flattens out and
the nose goes light, lift off the throttle for 40 ms and get back on it: that is
worth 0.012, it is the easiest input in the lap to get right, and nobody in a
field of 334 does it.**

**Video: withdrawn.** The clip published here was filmed from a copy of our ghost
that is not the file this page ships — a stale staging copy, superseded before the
clip was shot. The run and the time are unaffected. A replacement will be filmed
from a regenerated ghost, whose telemetry is verified against a re-simulation of
its own inputs.

| run | time | vs author time | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, unconstrained** | **6.578** | **−0.017** | **−0.026** | analog, 111 values |
| TAS, 7-value action keys | 6.591 | −0.004 | −0.013 | 45 events |
| **TAS, keyboard only** | **6.595** | **±0** | **−0.009** | **19 events, 3 values** |
| TAS, 5-value action keys | 6.595 | ±0 | −0.009 | 38 events |
| Author time (never beaten by a human) | 6.595 | — | −0.009 | — |
| Human WR — jujumasterr | 6.604 | +0.009 | — | — |
| Best human keyboard run (rank 3) | 6.608 | +0.013 | +0.004 | 17 events, 3 values |

TMX map [279209](https://trackmania.exchange/maps/279209) · author **in-.-** ·
**334 recorded runs**.

## The map

Two waypoints, no checkpoints, 6.6 s, on the ground for every tick, gas held
essentially throughout, brake never used. A standing start **on ice** down a
steep straight (0 → 130 km/h in 2.5 s, with under a metre of lateral movement),
a stab of full right, then **one 77 m-radius left-hand sweeper held at full lock
for 3.0 seconds** to the flag, accelerating from 161 to 212 km/h.

The car meets the finish plane at 58 m/s: **1 millisecond is 5.8 cm.**

## Where the time is

The field is identical for two seconds and then fans out. Speed at any moment
before 3.200 tells you nothing about a run's finish time; from 3.400 onward it
tells you most of it, and the car's yaw at 3.800 is the single best predictor of
anybody's lap. **The ice start and the straight are solved; the spread is in the
sweeper.**

Against the human world record the unconstrained run is dead level for the first
two seconds and then takes one to four thousandths every half-second all the way
through the sweeper — nothing dramatic, just carry:

- at 2.500–2.750 it holds **15 of 127 units of right lock** where the human
  holds none;
- from 4.250 on it is **2–3 km/h faster** through the whole corner;
- through the middle it runs up to **1.1 m wider**, converging back onto the
  human's line by the flag.

That is the analog run. The part a person can use is the keyboard tape, and its
0.013 over the rank-3 human decomposes into exactly two changes:

| tape | time |
|---|---|
| the rank-3 human as driven | 6.608 |
| the same steering with the turn-in at 1.690 instead of 1.730 | 6.607 |
| ...plus the 40 ms throttle lift at 1.760 | **6.595** |

**One millisecond is the earlier turn-in. Twelve are the lift.**

**Why the lift works.** The rank-3 human's own telemetry shows the front
suspension unloading right there — the front dampers go from compressed to fully
extended between 1.650 and 1.850 while the rears stay loaded, and the car stops
descending. The downhill flattens out and the nose goes light. Closing the
throttle for 40 ms puts weight back on the front wheels exactly as the
left-hander is being asked for, the front bites, and the car takes a better line
into the corner. It is ordinary weight transfer, on the one part of the track
where the surface is ice and the front is unloaded at the same time.

## The run, as keys

`replays/KB_SIMPLE_6595.Ghost.Gbx`. Three steering values, no brake, throttle on
for every tick except the lift.

| race | input | held |
|---|---|---|
| **0.030** | full LEFT | 710 ms |
| 0.740 | centre | 60 |
| 0.800 | full RIGHT | 100 |
| 0.900 | centre | 120 |
| 1.020 | full RIGHT | 150 |
| 1.170 | centre | 80 |
| 1.250 | full RIGHT | 190 |
| 1.440 | centre | 60 |
| 1.500 | full RIGHT | 100 |
| 1.600 | centre | 90 |
| **1.690** | full LEFT | held through the lift |
| **1.760** | **THROTTLE OFF** (steering stays full left) | 40 |
| 1.800 | throttle back ON | 610 |
| **2.410** | full RIGHT | 150 |
| 2.560 | centre | 230 |
| 2.790 | full RIGHT | 830 |
| 3.620 | centre | 60 |
| 3.680 | full LEFT | 2930 ms, to the flag |

## Sector by sector, off what you can see

**S1 — the ice run-down (0 → 1.700, 0 → 99 km/h).** You spawn on ice pointing
down a steep straight; the road falls about 5.5 m over the first 20 m. Throttle
to the floor and leave it there. **Immediately at the lights, full LEFT, held
for about seven tenths.** Late is free; early is not. The car barely moves
sideways in this whole section — under a metre — so the steering here is holding
the car straight on ice, not aiming it.

**S2 — the wiggle (0.740 → 1.600, 32 → 96 km/h).** Four short right stabs with
the wheel centred between them, roughly at 0.740, 1.020, 1.250 and 1.500, each
about a tenth of a second. This is counter-steering on ice: the whole field does
something like it and the exact pattern is not critical. **Copy the rhythm, not
the ticks.**

**S3 — the crest, and the twelve free milliseconds (1.690 → 1.800, 99 km/h).**
This is the discovery. **Watch for the moment the downhill stops falling away
and the road goes flat** — the horizon steadies and the nose comes up. Right
there: **full LEFT, and about half a tenth later blip off the throttle for
40 ms, then back on.** Do not turn in *early* here: 10 ms early costs 0.062.

**S4 — the crux flick (2.410 → 2.560, 113 → 120 km/h).** Still on the run-out,
before the corner proper. **Full RIGHT for exactly 150 ms, then centre.** This
is the hardest input on the map and the one that separates the leaderboard. If
you are early, shorten the flick; if you are late, lengthen it — that diagonal
keeps you within about 0.010.

**S5 — the long right (2.790 → 3.620, 130 → 160 km/h).** **Full RIGHT, held for
eight tenths.** Release it as the road starts to swing left. Never late on the
release.

**S6 — the sweeper (3.680 → the flag, 160 → 214 km/h).** **Full LEFT and hold it
for the remaining 2.9 seconds.** One input. The road rises about 11 m and the
car accelerates the whole way. Do not correct, do not lift.

**The flag.** Crossed at 214 km/h. The gate's trigger window is 49 m wide
laterally and every human crosses 17–18 m inside its near edge, so unlike some
maps in this family there is **no invisible boundary to shave here** and nothing
to gain by tightening the exit. Aim for speed, not for the edge.

## How forgiving it is

Mistime one input by a tick (10 ms) — the cost against 6.595.

| race | input | 1 tick early | 1 tick late | verdict |
|---|---|---|---|---|
| 0.030 | LEFT | +0.005 | **0** | very forgiving; late is free out to +30 ms |
| 0.740–1.600 | the wiggle | +0.010…+0.019 | +0.010…+0.022 | recoverable |
| 1.690 | LEFT | **+0.062** | +0.011 | never early |
| **1.760** | **throttle OFF** | **+0.007** | **+0.007** | **the most forgiving input in the lap** |
| **1.800** | **throttle ON** | **+0.007** | **+0.006** | same |
| **2.410** | **full RIGHT** | **+0.030** | **+0.116** | **the crux** |
| 2.560 | centre | +0.106 | +0.029 | tight |
| 2.790 | RIGHT | +0.012 | +0.154 | never late |
| 3.620 | centre | +0.073 | +0.009 | never early |
| 3.680 | LEFT | +0.064 | +0.013 | never early |

**The lift is easy.** A 40 ms lift starting anywhere between 1.730 and 1.790
gives 6.595 — a 70 ms window — and any lift of 10–40 ms anywhere between 1.690
and 1.990 is worth at least 7 of the 12 thousandths. Being 60 ms out costs under
0.010. There is no way to hurt yourself with it.

**The flick at 2.410 is the crux, and it always was.** Five ticks early or six
ticks late and the run does not finish; one tick late costs 0.116. There is no
forgiving alternative: the best off-nominal setting costs 0.006 and everything
else costs 0.009 or more. This is not something the fast tape introduced — the
rank-3 human's flick has the same ±1 tick window, and it is the reason 334
people are stacked between 6.604 and 7.029.

**What will take real practice: the flick at 2.410, and nothing else.** S1, S2,
S3, S5 and S6 are all tolerant to ±20 ms or better, and S6 is a single held key
for 2.9 seconds.

## On the alphabet

The keyboard rung here is not a theoretical construct — **rank 3 on this
leaderboard is a pure keyboard run with 17 input changes in the whole lap**, and
across sampled tapes from rank 1 to 265 the steering is −127 (46%), 0 (42%) and
+127 (11%), with the brake appearing in one tape for six ticks.

Intermediate steering values buy almost nothing here. Five values is the same
6.595 as three and needs 38 events instead of 19, so it is strictly worse to
drive; seven values finds 0.004 but needs 45 events. **The alphabet is not what
costs you on this map, the event count is** — 19 digital decisions get within
0.017 of a 198-event analog tape.

The unconstrained 6.578 tape is not drivable and is not meant to be: 198 change
events and 111 distinct steering values, a dense analog ramp where the keyboard
tape has one flick.

## Files

| file | what |
|---|---|
| `replays/KB_SIMPLE_6595.Ghost.Gbx` | **keyboard only, 19 inputs — matches the author time, the one to learn** |
| `replays/AK7_6591.Ghost.Gbx` | 7-value action keys, 6.591 |
| `replays/AK5_6595.Ghost.Gbx` | 5-value action keys, 6.595 |
| `replays/BEST_6578_ratcheted.Ghost.Gbx` | the fastest run, unconstrained |
| `replays/champ_6578.Ghost.Gbx` | the first tape to reach 6.578 |
| `replays/kb2_best_6595.Ghost.Gbx`, `kb20.Ghost.Gbx`, `kb_gasfull.Ghost.Gbx` | the rest of the low-input family |
