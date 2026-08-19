# Torment (1-DOWN)

**The floor at the base of the end wall is a launcher, and it fires only if you
cross it sideways — go through the last checkpoint, then scrub across the line
ten metres later with the car turned across its own direction of travel.**

| run | time | vs author time | vs human WR |
|---|---|---|---|
| **TAS** — [`TAS_20237`](replays/TAS_20237.Ghost.Gbx) | **20.237** | **−0.318** | −2.400 |
| Author time (never beaten by a human) | 20.555 | — | −2.082 |
| Human WR — KappaRiley | 22.637 | +2.082 | — |

TMX map [228811](https://trackmania.exchange/maps/228811) · by **Bernkastel_. /
Emelius.** · **48 recorded runs**.

This is the same map as [Torment (1-UP)](../228607-torment-1-up) with the finish
64 m lower.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## Every run on the board ends the same way. The author's does not.

All 48 records arrive at the base of the end wall at about 360 km/h, ride *up*
the wall to y ≈ 142, flip 180°, and fly ~314 m back to the line. The wall climb
costs about 1.6 s and the flight 2.6–5.5 s.

**The author never climbs the wall.** He slides along the floor at its base and
hits something that fires him from **323 to 751 km/h in a single contact**, then
glides to the line upside down. That is the entire 2.082 s gap, in one move.

### The floor everyone already drives on is the launcher

At the base of the wall, the floor from x = 32 to x = 128 is boost platform. All
48 runs cross onto it at x ≈ 63 and pick up an ordinary turbo there. Nobody is
missing a hidden object in a corner of the map — everybody drives the length of
this thing, every lap.

Running through the deck at **z ≈ 709**, spanning at least x = 56 to x = 136,
there is a trigger about a metre wide. It is **not** fussy about where along the
line you hit it. It is extremely fussy about how.

### The condition: you must arrive sideways

> **Cross the line going in −z, at floor level, with the car turned across its
> own direction of travel — at least 85 m/s (≈300 km/h) of your speed pointing
> out of the side window.**

| what you do | fires? |
|---|---|
| cross at 360 km/h pointing where you are going — *what everyone does* | **no** |
| slide **along** the line at 102 m/s of side speed | **no** |
| cross it downwards at 100 m/s, nose-first | **no** |
| arrive at the author's exact contact point, within 0.3 m, at his speed within 3 m/s, but pointing along your travel | **no** |
| cross **downwards, body lateral, ≥85 m/s of side speed** | **yes** |

That fourth row is the one worth staring at. **Position does not trigger it,
speed does not trigger it, and the two together do not either.** Which way the
car is pointing does.

When it fires it converts your sideways speed into forward speed *along your
nose*. The author is pointing +x and about 27° up at contact, so he is fired
straight down the track at the finish. Point the nose the wrong way and you get a
magnificent 974 km/h launch vertically into the sky.

### The hard part is the checkpoint, not the launcher

The last checkpoint is a 32 m gate at **x = 80**, posts at z = 720 and z = 752,
and you have to go through it. A launch upstream of the gate flies beautifully —
there are lines that fire at x = 112 and pass within 0.8 m of the finish — and
they are worth nothing, because the run skipped the gate.

> **Go through the gate at x = 80, and then get down onto the line at z ≈ 709 and
> across it sideways, inside the next ten metres of x.**

The author threads exactly that: x = 80 at z = 718.7, hugging the near post, and
contact at (70.2, 50.4, 708.9). Ten metres of track between a gate you must not
miss and a line you must cross sideways — that, and not the launcher's
obscurity, is why nobody has it.

## The run, as inputs

From the author's own lap: 37 input events in the whole run, six distinct steer
values, and one input held for the last 2.4 seconds.

> Come off the last section wide and **keep the car turned across its direction
> of travel through the long drop** — you are not steering, you are sliding.
> Cross the last checkpoint at its near post. Then, instead of running on to the
> wall, **hold full right lock with throttle and brake together** and let the car
> scrub left along the floor. About 19 m later, still sideways at ~320 km/h, you
> cross the line and are fired back down the track at 751 km/h. **Keep the lock
> held** — the thrust points up out of the inverted car and cancels gravity, so
> you glide from y = 53 to the line without ever touching the wall.

The scrub, as it develops:

| race time | x | z | speed | side speed | velocity off the track axis |
|---|---|---|---|---|---|
| 18.300 | 93.8 | 726.4 | 342 | 90.8 | 21° |
| 18.400 | 85.3 | 722.3 | 338 | 91.2 | 28° |
| 18.500 | 77.7 | 716.9 | 332 | 89.3 | 40° |
| 18.600 | 71.4 | 710.3 | **323** | **86.8** | **51°** |
| 18.650 | 75.6 | 708.3 | **751** | — | fired |

The commit at about 18.15 is a *hold*, not a stab.

**Keeping the lock held after the launch is specific to this map.** It works
because the finish here is low and the car is supposed to go broadside and shed
speed on the way down to it. On [Torment (1-UP)](../228607-torment-1-up) — the
same map with the finish 64 m higher — the same instruction sends you broadside
at 562 km/h and you do not reach the line.

## How forgiving it is

**Brutal.** Shift the final input sequence by one tick in either direction and
the launch does not get worse, it does not happen at all — no launch at ±10 ms,
±20 ms, and every step out to ±80 ms.

Two things temper that. It was measured on a machine-optimised line with 697
input changes where every tick is load-bearing, which is the worst case; the
author's own lap is a 37-event script on six steer values, coarse enough that it
plausibly sits in a more forgiving pocket, and his tolerance cannot be measured
because his run is a recording rather than an input tape.

And somebody on the leaderboard is already doing the move. **Rank 11 (26.715)
crosses x = 80 with 87.6 m/s of body-lateral speed at 331 km/h** — the author's
own signature, and by a wide margin the largest on the board. He is still at
81.7 m/s at x = 65, sliding down toward the line, and then he puts the car into
the end wall at 12 km/h. Every other record is under 20 m/s of side speed
anywhere near the line, and **0 of 48 satisfy the full condition.**

So: a human can get the car into this state at speed on this map, because one of
them does it every lap he drives — and doing it two metres further along puts you
in the wall instead of at the finish. **That is what will take real practice**:
not the lock, which is one held input, but arriving at the deck already sideways
at 320 km/h in the ten metres after the gate.

## Files

| file | what |
|---|---|
| `replays/TAS_20237.Ghost.Gbx` | the run |

<!-- VIDEOS:START -->

## Videos

Chase-cam recordings rendered in-game (Player camera targeting the ghost, no effects). Each clip runs for the ghost's exact race time. Click to play (GitHub serves the file); each corresponds to a `replays/*.Ghost.Gbx` in this folder.

- [`TAS_20237`](videos/TAS_20237.mp4)

<!-- VIDEOS:END -->
