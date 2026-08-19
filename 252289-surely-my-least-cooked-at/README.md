# surely my least cooked at

**Add two key presses to the world record's run — a tap and a lift, both in the
last corner — and the author time falls.**

| run | time | vs author time | vs human WR | input changes | steering |
|---|---|---|---|---|---|
| Human WR — KevinMagPizza | 3.867 | +0.016 | — | 8 | keyboard |
| **WR + two key actions** | **3.848** | **−0.003** | −0.019 | 12 | keyboard |
| TAS, keyboard | **3.844** | −0.007 | −0.023 | 18 | keyboard |
| TAS, 8-value | **3.836** | −0.015 | −0.031 | 18 | 8 values |
| Author time | 3.851 | — | −0.016 | — | — |

TMX map [252289](https://trackmania.exchange/maps/252289) · author **in-.-** ·
706 recorded runs.

The record holder is himself on keyboard — exactly three steering values, eight
input changes for the whole lap. This is not a machine out-precising a human; it
is two extra key presses in a corner nobody treated as a corner.

## What the map is

Four blocks, and the start and the finish are side by side, 32 m apart. You
spawn facing up the start lane and the finish plane is 16 m sideways and 11 m
further up the neighbouring lane. Straight-line that is 19 m and the car can do
28 m/s, yet the record is 3.867 — because it is not a distance problem, it is a
**rotation** problem. A single forward arc cannot work: to reach the other lane
you need a radius of at least 16 m, and with that radius you have already
crossed the finish plane while still in the start lane, where the gate does not
exist. You have to spend the sideways metres first and the forward metres
second, which is why nobody drives it forwards.

## The two actions

Drive KevinMagPizza's run exactly, and add:

1. **At 2.63 s — one-tick tap of RIGHT.** You are in the neutral-steer phase
   between the rotation and the last corner, doing **66 km/h**, pointed across
   the track. Tap and release. (Yes, a right tap 50 ms before you turn left — it
   sets the car's attitude for the corner.)
2. **At 2.89 s — lift the throttle for 50 ms**, then full throttle again at
   2.94 s. You are at **73 km/h**, at full left lock, and the seam between the
   two lanes is about to pass under the car (you cross it at 2.97 s).

That is **3.848**, three milliseconds under the author time.

**The lift is the trick.** On its own, added to the world record with nothing
else changed, it is worth 0.009. The right tap on its own is catastrophic
(4.189) — it only makes sense once the lift follows it. They are a pair.

## Why it works

All of the margin is in the final left sweep from 2.68 s, which the whole field
takes at full lock and full throttle.

| t | TAS speed | WR speed |
|---|---|---|
| 2.60 | 65.4 | 64.5 |
| 2.70 | 65.9 | 68.0 |
| 3.00 | 73.1 | 73.7 |
| 3.10 | 70.9 | **67.5** |
| 3.20 | 72.5 | 67.5 |
| 3.50 | 88.4 | 83.0 |
| 3.80 | 101.7 | 98.5 |

Between 3.02 s and 3.12 s **the human's car loses 6.2 km/h and never gets it
back**; the TAS loses 2.2. Around 3.0 s the car crosses the seam between the two
road blocks with the suspension already loaded by the turn — both runs dip to a
body height of 9.95 m at 3.02 s and rebound, but the human takes the rebound at
73.7 km/h and the tyres let go. It is not a drift (slip angle stays under 3° in
both runs); it is the contact patch being unloaded and reloaded while the car is
asking for maximum grip. Arriving about 1 km/h slower, with the suspension a
fraction of a cycle further along, rides it out.

From 3.1 s to the line the TAS is 5 km/h faster every tick and crosses **1.11 m
further down the road**. Textbook slow-in / fast-out, on a corner nobody thought
of as a corner.

## The run, sector by sector

The shape is the world record's, and up to 2.6 s the two runs are the same run —
8 cm and 0.9 km/h apart after two and a half seconds of driving.

| race | what you do | what you see |
|---|---|---|
| 0.00 | brake on (reverse), wheel straight | standing start |
| 0.12 | full LEFT | the car rotates as it backs out |
| 0.72 | brake off, then straighten at 0.79 | you are 4.6 m back down the lane, turned 37° |
| 0.82 | full RIGHT, throttle on at 0.84 | the car scrubs off the reverse speed |
| 1.30 | (nothing) | dead stop, pointing across the track |
| 1.30–2.44 | hold throttle and full right | rotating to face across the lanes, up to 55 km/h |
| 2.44 | straighten | 55 → 69 km/h, crossing the lanes |
| **2.63** | **tap RIGHT, one tick** | **66 km/h, pointed across the track** |
| 2.68 | full LEFT, hold to the line | the long 100° sweep back to the finish |
| **2.89–2.93** | **throttle off, 50 ms** | **73 km/h, the seam about to pass under you** |
| 2.94 | throttle back on | you cross the seam at 2.97 |
| 3.848 | — | the line, at about 101 km/h |

## The full keyboard run — 3.844

Nine ticks differ from the world record, all between 2.63 s and 2.98 s, all of
them releases:

| time | action | note |
|---|---|---|
| 2.63 s | tap RIGHT for 1 tick | attitude set-up |
| 2.72 s | release LEFT for 1 tick | feather |
| 2.74 s | release LEFT for 1 tick | feather |
| **2.89–2.93 s** | **throttle off for 5 ticks** | the money |
| 2.98 s | release LEFT for 1 tick | feather |

Nothing before 2.63 s changes at all. The reverse, the stop, the rotation and
the cross-track run are exactly the human's. The two extra feathers are worth
0.002 each on top of the two-action version.

The 3.836 tape needs steering values a keyboard cannot produce — 2 % of lock
held for 110 ms while reversing, an eased lock mid-rotation, three odd values in
the last corner. That is the structural edge a machine has, and it is worth
0.008. It is not the lesson.

## Learn the line, not the tape

Mistime one input on the frozen tape and the run detonates: every boundary
shifted by a single tick costs 0.280–1.900. That is a property of replaying a
fixed tape, not of driving — shift one of KevinMagPizza's own eight inputs by a
tick and his 3.867 costs 0.276–2.871 in exactly the same way, and a person drove
that run with 706 people chasing it.

The fair test is mistiming an input and then *driving the rest of the corner*,
which is what a person does:

> **40 of 40 mistimings recover to exactly 3.836** (one to 3.839), and every one
> tried on the keyboard tape recovers to exactly 3.844.

Shifting the very first steering input two ticks early turns a DNF into 3.836
once the rest of the run is re-driven. **The line is robust; the tape is not.**
Learn the shape: reverse-rotate, stop, rotate under power, cross the lanes, and
then *breathe* in the last corner instead of holding it flat.

**The one input that genuinely needs precision is the lift.** Start it within ±1
tick of 2.89 s and hold it at least 50 ms: a tick late costs 0.034, a tick early
0.242, and holding it longer than 50 ms costs 0.100–0.130. It has a speed cue
you can read off the dashboard — **73 km/h** — and a visual cue under the wheels
— **the seam**. Taps at a cue are learnable; this is the same class of input as
a brake tap into a landing.

## Files

| file | what |
|---|---|
| `replays/tas_twoinputs_3848.Ghost.Gbx` | **the world record plus two key presses** — the one worth studying |
| `replays/tas_keyboard_3844.Ghost.Gbx` | keyboard only, 18 inputs |
| `replays/tas_3836.Ghost.Gbx` | fastest, 8-value alphabet |
| `inputs/tas_min2_3848.tick.txt` | the two-action run as a readable input script |
| `inputs/tas_kb_3844.tick.txt` | the keyboard run |
| `inputs/tas_3836.tick.txt` | the fastest run |

<!-- VIDEOS:START -->

## Videos

Chase-cam recordings rendered in-game (Player camera targeting the ghost, no effects). Each clip runs for the ghost's exact race time. Click to play (GitHub serves the file); each corresponds to a `replays/*.Ghost.Gbx` in this folder.

- [`tas_3836`](videos/tas_3836.mp4)

<!-- VIDEOS:END -->
