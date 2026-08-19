# Torment (1-DOWN) — the ending nobody drives

**Map:** `Fall 2024 - 08 Torment (1-DOWN)`, TMX 228811, by Bernkastel_. / Emelius.
**Author time 20.555 · human world record 22.637 (KappaRiley) · 48 records.**

Every run on the leaderboard ends the same way: arrive at the base of the end
wall at about 360 km/h, ride *up* the wall to y ≈ 142, flip 180°, and fly ~314 m
back to the line. The wall climb costs about 1.6 s and the flight 2.6–5.5 s.

**The author does not climb the wall.** He slides along the floor at its base
and hits something that fires him from 323 to 751 km/h in a single contact, then
glides to the line upside down. That is the entire 2.082 s gap, in one move.

This document says exactly what the move is, where it is, and how hard it is,
because the answer to the last one is *very*.

*(Everything below is measured from the map file and from the author's own
validation lap, which is embedded in the map and decodes to 412 telemetry
samples with its input columns. Nothing here has been submitted to a
leaderboard.)*

---

## The floor you already drive on is a booster

At the base of the end wall, y = 50, the floor from x = 32 to x = 128 is boost
platform:

```
x  32 ── 64 ── 96 ── 128
   [ Boost ][  Turbo   ]      (z 704 … 768, i.e. the full 64 m width)
```

All 48 runs cross onto it at x ≈ 63 and pick up an ordinary turbo there. Nobody
is missing a hidden object in a corner of the map. **Everybody drives the length
of this thing, every lap.**

## Somewhere in it there is a line, and crossing it correctly launches you

Running through the deck at z ≈ **709**, spanning at least x = 56 to x = 136,
there is a trigger about a metre wide. Cross it the right way and the game fires
your car along its own nose at 700–950 km/h.

It is not fussy about *where* along the line you hit it — we produced 1343
separate launches spread over 80 m of x. It is extremely fussy about *how*.

## The condition: you must arrive SIDEWAYS

This is the whole secret, and it is why 48 drivers have driven over this line
hundreds of times without ever seeing it fire.

> **Cross the line going in −z, at floor level, with the car turned across its
> own direction of travel — at least 85 m/s (≈ 300 km/h) of the car's speed
> pointing out of its side window.**

Everything else is irrelevant:

| what you do | fires? |
|---|---|
| Cross it at 360 km/h pointing where you're going (what everyone does) | **no** |
| Slide *along* the line, fully sideways at 102 m/s of side speed | **no** |
| Cross it downwards at 100 m/s, nose-first | **no** |
| Arrive at the author's exact contact point, within 30 cm, at his speed within 3 m/s, but pointing along your travel | **no** |
| Cross it downwards, sideways, ≥ 85 m/s of side speed | **YES** |

That fourth row is the one worth staring at. We built a run that reached the
author's contact point to within **0.3 m** with a velocity within **3 m/s** of
his, and nothing happened. **Position doesn't trigger it. Speed doesn't trigger
it. Which way the car is pointing does.**

When it fires, it converts your sideways speed into forward speed *along your
nose*. The author is pointing +x and about 27° up at contact, so he is fired
straight down the track at the finish. Point the nose the wrong way and you get
a magnificent 974 km/h launch vertically into the sky — we did that too, twice.

## The hard part is not the launcher. It's the checkpoint.

The last checkpoint is a 32 m gate at **x = 80**, posts at z = 720 and z = 752.
You have to go through it.

We built runs that fire the launcher at x = 112 and fly to **within 0.8 m of the
finish line** — and they are all worth exactly nothing, because at x = 112 you
are still upstream of the gate and the run doesn't count. 5 checkpoints of 6.

So the real shape of the technique is:

> **go through the gate at x = 80, and then get down onto the line at z ≈ 709
> and across it sideways, inside the next ten metres of x.**

The author threads precisely that. He crosses x = 80 at z ≈ 718.7 — hugging the
near post — and contacts the line at (70.2, 50.4, 708.9). Ten metres of track
between a gate you must not miss and a line you must cross sideways.

## One driver on the leaderboard is already doing it

**Rank 11 (26.715) does the author's scrub.** Not something like it — the same
thing:

| | at x = 80 | at x = 65 |
|---|---|---|
| rank 11 | side speed **87.6 m/s**, 331 km/h | side speed 81.7, 312 km/h, z = 711.8 |
| author | side speed ~89, 335 km/h | *(already fired at x = 71)* |

He is fully sideways, at the author's speed, sliding down towards the line —
and he crosses it at x ≈ 63 with his side speed decaying, and puts the car into
the end wall at 12 km/h. His run ends there; he finished 26.715.

He is the only one of the 48 who gets close, and he is not close by accident:
he is doing the move. What separates him from the author is a couple of metres
of x and a few m/s of side speed at the moment of crossing.

**Nobody else on the board exceeds 20 m/s of side speed anywhere near the line.**
Of 48 records, the number that reach the z 704–713 band at floor level with
≥ 85 m/s of side speed and a downward crossing is **zero**.

## What to actually do

From the author's own decoded inputs — 37 input events in the whole 20.5 s lap,
six distinct steer values, throttle held almost throughout:

> Come off the last section wide and **keep the car turned across its direction
> of travel through the long drop** — you are not steering, you are sliding.
> Cross the last checkpoint at its near post. Then, instead of running on to the
> wall, **hold full right lock with throttle and brake together** and let the
> car scrub left along the floor. ~19 m later, still sideways at ~320 km/h, you
> cross the line and get fired back down the track at 751 km/h. **Keep the lock
> held** — the reactor's thrust points up out of the inverted car and cancels
> gravity, so you glide from y = 53 to the line without ever touching the wall.

The commit at 18.15 s is a *hold*, not a stab: the author holds that one input
for the final 2.4 seconds of the run.

The author's approach, tick by tick, as the scrub develops:

| race time | x | z | speed | side speed | velocity angle off the track axis |
|---|---|---|---|---|---|
| 18.30 | 93.8 | 726.4 | 342 | 90.8 | 21° |
| 18.40 | 85.3 | 722.3 | 338 | 91.2 | 28° |
| 18.50 | 77.7 | 716.9 | 332 | 89.3 | 40° |
| 18.60 | 71.4 | 710.3 | **323** | **86.8** | **51°** |
| 18.65 | 75.6 | 708.3 | **751** | — | fired |

## How hard is it, honestly

**Brutal.** On our optimised run, shifting the entire final input sequence by
**one tick — 10 milliseconds — in either direction does not make the launch
worse. It removes it entirely.** No launch at all, at ±10 ms, ±20 ms, and every
step out to ±80 ms.

Two honest caveats on that number:

- It was measured on a machine-optimised line with 697 input changes, where
  every tick is load-bearing. That is the worst case.
- **The author's own lap is a 37-event script on six steer values** — closer to
  an action-key or pad run than a TAS. A line that coarse plausibly sits in a
  more forgiving pocket than ours does. We cannot measure his tolerance
  directly: a TM2020 validation ghost stores a state recording, not an input
  tape, so his run can be read but never replayed.

And rank 11 is the practical evidence on both sides: a human *can* get the car
into this state at speed on this map — he does it — and doing it two metres
further along puts you in the wall instead of at the finish.

## What it's worth

| | time |
|---|---|
| human world record | 22.637 |
| **author time** | **20.555** |
| our best validated run on this route | **20.237** |

Every run above was verified by re-simulating the input file from the start on
an untouched copy of the map, with the human world record re-simulated in the
same batch as a control (it returned 22.637 every time).

The 2.4 s between the world record and our run is not driving skill. It is this
one contact.
