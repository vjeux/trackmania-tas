# Great wtf of what #165

**The author time is 8.127 and no human has beaten it. It falls on a keyboard,
with the same three steering values and one more key press than the world
record holder uses.**

**Great WTF of what #165** — TAS **7.998** (−0.129) | AT 8.127 | WR 8.197 by Titoch_tm

https://github.com/user-attachments/assets/7ee7a8cc-b5ed-449c-9906-95912809a5c5

**Great WTF of what #165** — TAS **8.050** (−0.077) | AT 8.127 | WR 8.197 by Titoch_tm

https://github.com/user-attachments/assets/ce212204-01ef-437e-9f2b-631967f19249

**Our 8-level action-key run against Titoch_tm's world record, both on screen.**
Ours is magenta. They are never far apart — **max 7.41 m, mean 0.47 m** — and the
gap opens where the run is decided rather than steadily: watch 4.4 s onward,
where the two cars take visibly different lines through the grass section and
ours arrives at the loop already ahead. We finish 0.147 in front.

This clip is also the one that cost the most to make honest. The file it is shot
from went through **three generations** before it was ours: the first declared
Titoch's 8.197 in its header, the second was repaired but still imported into the
game wearing his nickname, and only the third reads `Ghost:TAS`. The trajectory
never changed through any of it — 162 of 162 positions identical, and independent
of his recording at every alignment. **The driving was always ours; the file was
not.**

| run | time | vs author time | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, keyboard only** | **8.075** | **−0.052** | **−0.122** | 14 changes, 12 of them steering |
| TAS, 8-level action keys | 8.050 | −0.077 | −0.147 | — |
| TAS, unconstrained | **7.998** | −0.129 | −0.199 | analog, 111 steering values |
| Author time | 8.127 | — | −0.070 | — |
| Human WR — Titoch_tm | 8.197 | +0.070 | — | keyboard, 11 steering events |

TMX map [227969](https://trackmania.exchange/maps/227969) · author
**FrankTheHamster** · 42 recorded runs.

## What the whole field is doing wrong

Nothing is won in the first 6.5 seconds — over the first 520 m this run is
actually 0.010 behind the world record. **All of it is made in the last 1.4 s.**

The map ends with the car thrown off a wall at 420 km/h, arcing along a curved
wall, then kicked into a ballistic flight through the finish gate.

**Every one of the 42 recorded runs holds full lock through that wall-ride.**
Holding it rides the car up the curve and rolls it over: by the kicker the world
record is nose-up 57° and rolled almost onto its side. A tipped, crabbing car
pays the kicker about a third of its speed.

| | speed into the kicker | speed leaving it | speed toward the finish |
|---|---|---|---|
| Human WR | 73.6 m/s | 61.5 m/s | **57.3 m/s** |
| Best of all 42 humans | — | — | 59.8 m/s |
| **TAS, keyboard** | 74.2 | 69.9 | **65.9** |
| **TAS, analog** | 75.4 | 73.1 | **68.8** |

Arriving **flat** (roll under 5°) and **square** (sideways speed 0.35 m/s)
costs 3 m/s instead of 12. There are 43 m of gate to cover: at 57.7 m/s that is
750 ms, at 69.2 m/s it is 625 ms.

The difference between the two is four key presses:

```
Human WR through the corner — one long lock
  6.69  LEFT ────────────────────────────── 7.08 release   (390 ms)
  7.16  LEFT ──── 7.20 release                              (40 ms)
  7.69  LEFT (hold into the kicker)

Keyboard TAS — the same key, pumped
  6.68  LEFT ───────────── 6.90 release                    (220 ms)
  6.95  LEFT ──── 7.03 release                              (80 ms)
  7.12  LEFT ── 7.16 release                                (40 ms)
  7.39  LEFT (hold into the kicker)
```

**The field holds one 390 ms lock where it should be pumping three short taps,
and commits to the kicker 300 ms too late.** Everything before 5.24 s is
identical between the two tapes.

The line never leaves the human racing corridor — at most 2.57 m from the world
record's own trajectory over the whole run — and it takes the map's one
collision, the wall throw, exactly as all 42 humans take it.

## The run, as keys

`replays/kb_8075.Ghost.Gbx`. Accelerate is held from the countdown to the line
and never released; the brake goes on at 5.23 s and stays on. Everything else is
steering.

```
race  0.260  steer LEFT      | as the car starts rolling
race  1.230  steer 0         | straighten, ~68 km/h
race  2.450  steer RIGHT     | on the booster, ~186 km/h — hold through the ramp
race  5.230  brake ON        | STILL IN THE AIR, ~120 ms before touchdown
race  5.240  steer 0         | land straight
race  6.680  steer LEFT      | commit into the wall throw
race  6.900  steer 0         |  \
race  6.950  steer LEFT      |   |  the PUMP: three taps
race  7.030  steer 0         |   |  220 ms / 80 ms / 40 ms
race  7.120  steer LEFT      |   |
race  7.160  steer 0         |  /
race  7.390  steer LEFT      | commit to the kicker, hold to the finish
```

## Sector by sector, off what you can see

**Start → the booster (0 → 2.4 s).** Identical to the current world record. Left
off the line, straighten at about 68 km/h, then full right as the booster fires
and hold it. Nothing to gain here; do not experiment.

**The big jump (3.55 → 5.35 s).** Keep full right lock through the entire
flight. You are not steering, you are spinning the car so it lands pointing down
the road. Every good run does this identically.

**The landing (5.23 s).** *Brake while you are still in the air*, about an
eighth of a second before the wheels touch, and centre the wheel at the same
moment. The world record brakes at 5.23 s too — this part is already right.
Braking early is safe; braking late does not finish.

**The straight (5.35 → 6.66 s).** Nothing to do. You coast 455 → 422 km/h. Get
your hand ready.

**THE CORNER — this is the whole map (6.68 → 7.16 s).** You cannot miss the
throw: you are doing 420 km/h in a straight line and the car is slammed sideways
and whipped through more than half a turn, speed collapsing to ~325. Commit
**full left as it happens** — tie this input to the impact, not to a clock.

Then **do not hold it.** Three taps, and the cue matters far more than the
millisecond:

| tap | cue to act on | speedo |
|---|---|---|
| **release tap 1** (~6.90) | **the bottom of the swing** — after the throw you travel *away* from the finish while the car swings round; release the instant that stops and the car starts being flung forward up the corridor | **292 km/h** |
| **tap 2** (~6.95) | half a beat later, a short stab of ~80 ms | 289 |
| **release tap 2** (~7.03) | as the car swings onto the corridor and the nose comes round | 283 |
| **tap 3** (~7.12) | a flick, ~40 ms, as the car is nearly straight | 278 |
| **release tap 3** (~7.16) | straight away — you are pointed at the finish | 276 |

Rhythm, in one phrase: after the throw, **hooold – tap – tick** (220 / 80 /
40 ms), with a short gap between each.

**The cue that needs no instrument: the horizon must stay level.** Hold the lock
and the car rides up the curved wall and the whole world tips. Our run never
rolls past 5°; each release lets the car drop back flat. If your horizon rolls,
you are driving the old line.

**The kicker (7.39 s).** Full left again, hold it to the finish. Cue: **commit
as the nose stops rising** — the car crests the curved wall onto the last ramp
and the pitch stops climbing about two car lengths before the lip. The world
record takes this input at 7.69 s, three tenths later, which is exactly why it
launches steep and slow.

**Check your run on the speedo** as you are thrown into the final flight
(~7.4 s):

* **221 km/h** — you drove it like the current world record (≈8.20)
* **240 km/h** — about the author time
* **252 km/h** — 8.075, the keyboard run
* **263 km/h** — 7.998, the TAS floor

## How forgiving it is

Mistime one input and keep driving — the cost in seconds, against 8.075.
Anything up to +0.052 still beats the author time. These are recoverable costs:
you mistime one input and re-time the rest, which is what a driver actually
does.

| input | race | −0.020 | −0.010 | +0.010 | +0.020 | verdict |
|---|---|---|---|---|---|---|
| brake ON | 5.230 | +0.026 | +0.046 | DNF | DNF | early is fine, late is fatal |
| straighten | 5.240 | +0.019 | +0.023 | +0.035 | +0.048 | forgiving both ways |
| LEFT into the throw | 6.680 | DNF | DNF | DNF | +0.088 | **tight** |
| release tap 1 | 6.900 | +0.023 | +0.070 | +0.023 | +0.023 | forgiving, ±0.030 |
| release tap 2 | 7.030 | DNF | +0.012 | +0.023 | +0.016 | forgiving late |
| tap 3 | 7.120 | +0.100 | +0.116 | DNF | DNF | tight |
| LEFT into the kicker | 7.390 | +0.072 | +0.090 | DNF | DNF | **tightest** |

**The two releases in the middle of the pump have 20–30 ms of room. The three
commits — into the throw, into tap 3, and into the kicker — are the tight ones**,
and the kicker commit is the worst: 10 ms early costs 0.090, 10 ms late does not
finish.

**Realistic expectation.** The pump is four key presses on a rhythm. A keyboard
player who learns it and gets the kicker commit approximately right should land
in the 8.05–8.12 range — under an author time that has stood since February
2025. The last 0.050 to 7.998 is machine work: the analog run holds the car flat
with 111 distinct steering values, and that part is not worth copying. Expect to
grind the kicker commit; use the crest of the wall as the cue, not the clock.

## Files

| file | what |
|---|---|
| `replays/kb_8075.Ghost.Gbx` | **keyboard only, 14 inputs — the one worth studying** |
| `replays/ak8_8050.Ghost.Gbx` | 8-level action-key steering |
| `replays/best_7998.Ghost.Gbx` | fastest run, unconstrained |
| `replays/best_8010.Ghost.Gbx` | the first tape to beat the author time |
| `inputs/m165_TAS_8010ms.tick.txt` | that run as a readable input script |
