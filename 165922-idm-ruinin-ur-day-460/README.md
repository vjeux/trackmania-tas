# idm ruinin ur day #460

**The whole map is the first five seconds — get down the chute and take both
booster gates. After that, hold the gas, let go of the wheel, and land long: nine
of the fifteen seconds need no steering at all.**

| run | time | vs author time | what it is |
|---|---|---|---|
| **TAS, keyboard steering from 4.56 s** | **15.217** | **−0.426** | the fastest run on the map |
| TAS, analog | 15.224 | −0.419 | the analog champion, which the keyboard tape beats |
| TAS, keyboard from 2.56 s | 15.285 | −0.358 | 70 input events in the whole run |
| TAS, low input | 15.290 | −0.353 | 86 input changes, 39 distinct steering values |
| TAS, deep landing | 15.382 | −0.261 | forced to land 40–80 m further into the pad |
| Keyboard, coarse launch | 16.276 | +0.633 | outside the author time on purpose — the forgiving version |
| Author time | 15.643 | — | — |
| The only human record | 8790.769 | — | 2 h 26 m — see below |

TMX map [165922](https://trackmania.exchange/maps/165922) · **one recorded run**.

## The map in one paragraph

You start on a platform 1.88 km up, drop through a narrow chute, cross the map's
one checkpoint at about 1.7 s inside the start structure, ride a short booster
ramp and leave it at about 5 s doing 180–200 m/s. Then there is **nothing** for
1.9 km: an unpowered ballistic glide onto a pad of 132 finish gates on the
ground, 88 m × 352 m. The map is a launch and a fall.

## Where the time is

The board's single record is not a slow lap. It is one session of 930 attempts
with the clock running through all of them, and the driver never converted one.
Their best attempt was worth 18.85 s on its own — and **45 m of that was spent
crawling.** They landed short of the first gate row and dragged along the ground
for the last **3.770 s** into a gate. The fast tapes cross the pad's near edge at
the exact millisecond they finish.

So nobody needs to discover the technique here — one person performed it 930
times in a sitting. What they never put together is the two things the author
time asks for: a clean launch, and a landing that reaches the pad instead of
stopping short of it.

The launch is the only lever that matters, because the glide is unpowered:

| launch speed off the ramp | flight time to the pad |
|---|---|
| 140 m/s | 13.490 — the slowest launch that reaches the pad at all |
| 182.5 m/s (what the human managed) | 10.470 |
| 200 m/s | 9.930 |
| 230 m/s | 9.040 |

**A launch at 200 m/s instead of 182.5 buys 0.540 s for nothing.** The two
booster gates are what set that number — they add the equivalent of 417 m of
extra height during the first five seconds, in two discrete kicks. Everything
after them is gravity and drag.

And the glide really is free. Force the steering to zero from a given moment to
the finish, on the fastest tape:

| steering zeroed from | finish |
|---|---|
| race 4.50 | DNF |
| race 5.50 | 15.276 (+0.052) |
| race 6.46 | 15.231 (+0.007) |
| race 8.46 onward | 15.225 (+0.001) |

Holding the throttle flat from 4.46 s to the finish is free in the same way.

## The run, as keys

The board's only human plays on a keyboard, and so does the fastest tape here.
Across the record, 94.2 % of steering values are exactly `0`, `−127` or `+127`,
and the winning attempt is 102 input events with the gas held the whole way and
one 20 ms brake tap:

```
off the start block   full lock RIGHT ─────────── 2.7 s
through the chute     full lock LEFT  ────── 1.2 s
onto the ramp         full lock RIGHT
after the boosters    nothing at all — hands off to the finish
```

That is the shape to copy. From about 6.4 s the human's tape reads `steer 0, gas
held` and never changes again.

### Sector by sector, off what you can see

**The chute (0 → about 3.5 s).** The opening is close to free fall — 168 m of it
— and it is a **feel** section, not a pattern. Do not try to memorise a
millisecond-exact sequence; the fast tapes do not even transfer to themselves.
Get down it cleanly and pointed at the ramp.

**The ramp and the boosters (about 3.5 → 5 s).** This is where the run is won or
lost, and it is the only part that punishes a small error. You want to leave the
structure at 5 s doing as close to 200 m/s as you can hold.

**The glide (5 s → the pad).** Hands off. Any steering input here is worth at
most a thousandth of a second and can kill the run. The car is capped at
277.55 m/s and covers 2 665 m, so it cannot go under about 9.600 s no matter
what you do.

**The landing.** Land **on** the pad, long rather than short. Short costs
seconds, not tenths — that is the entire difference between the record and the
author time.

## How forgiving it is

Move any one input a single tick (10 ms) earlier or later, and re-drive:

| window | shifts tested | still finish |
|---|---|---|
| race 0.00–2.96 | 52 | **0 %** |
| race 2.96–3.96 | 30 | 30 % |
| race 3.96–4.96 | 54 | 93 % |
| after race 4.96 | 1 202 | **100 %** |

Read the top row carefully: it is a fact about a frozen tape, not about a
driver — a recorded tape cannot notice it is 30 cm off, and a player corrects by
eye. Run the same test on the **human's own winning attempt** and 17 of 42 shifts
survive, **40.5 %**, three of them faster than the original. A launch with real
one-tick tolerance exists on this map.

What it costs: a coarser keyboard launch is about 10 % tolerant and runs 16.276.
So there are three points on the curve — 15.2 s at no tolerance, 16.3 s at 10 %,
the human's 18.8 s attempt at 40 %. **The forgiving program exists and it costs
about a second**, which puts the author's 15.643 exactly where a driven
validation lap should sit, between the two.

**What will take real practice:** the ramp entry and the boost sequence, and
nothing else. The shifted runs get down the chute perfectly well — 52 of 52 —
and then crash on the ramp. Everything after 5 s is ballistic and forgiving to
the point of being free.

## Files

| file | what |
|---|---|
| `replays/TAS_15217_clean.Ghost.Gbx` | **the fastest run — keyboard steering from 4.56 s** |
| `replays/TAS_15224_analog.Ghost.Gbx` | the analog champion |
| `replays/TAS_15285_keyboard.Ghost.Gbx` | keyboard from 2.56 s, 70 input events |
| `replays/TAS_15290_lowinput.Ghost.Gbx` | 86 input changes, zero steering through the glide |
| `replays/TAS_15382_deep_landing.Ghost.Gbx` | lands 40–80 m deeper into the pad |
| `replays/KEYBOARD_16276_tolerant.Ghost.Gbx` | **the forgiving launch — the one to learn from** |
| `inputs/tolerance_ramp.txt` | every launch input shifted ±80 ms, with what it costs |

`replays/` also holds three earlier tapes at 15.230, 15.240 and 15.549.
