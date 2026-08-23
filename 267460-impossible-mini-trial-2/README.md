# Impossible Mini Trial 2 (TMX 267460)

**Author time 16.888 · World record 23.068 (Wirtual, the only record ever set on this map) · Our best 21.022**

This is **one of only two maps in this repo where we have not beaten the author
time.** We are 2.046 under the human world record and still **4.134 over the
author time**. This page is the best account anyone has of why — including a
technique that saves five and a half seconds and that we still cannot convert
into a lap.

**Impossible Mini Trial 2** — TAS **21.022** (+4.134) | AT 16.888 | WR 23.068 by Wirtual

https://github.com/user-attachments/assets/dfc7c1cd-f2fa-4ed1-ada0-3f38c5be8f64

**Ours against Wirtual's world record, and the clip runs past both finishes.**
We cross at 21.022 and his car keeps driving for another two seconds — that gap
is the 2.046 we are ahead of the only human who has ever recorded a time here.
The two runs are properly apart for most of it: **max 50.16 m, mean 18.13 m**,
180 of 421 samples in the band where two cars read as two cars.

The car path in the clip is regenerated from engine memory and accurate to **half
a millimetre**, so the driving you see is exactly what the simulator validated.
One byte is still the carrier's — the surface-contact flag, which reads "on" for
15 of 39 samples where the car is provably airborne — so some of the *effects*,
dirt spray, sparks and wheel behaviour at the edges, are not ours.

---

## The map

A chain of floating platforms threaded through two enormous stadium screens.
One checkpoint (the finish), no respawns — "Trial" here is a building style, not
a checkpoint mechanic, so a respawn sends you back to the start with the clock
running.

The lap has three parts:

1. **The ice run (0 – 4 s).** You spawn on a turbo pad and are fired west along
   a strip of ice at 143 km/h. You do not get a choice about this: we tried
   twelve maximally different opening programs — every combination of full left,
   full right and straight, with gas and brake on and off — and *all twelve*
   cross the same point 9 m away between **0.422 and 0.455**. The human is at
   0.422. The first half-second of this map is the same whatever you press.
2. **The pit (4 – 13 s).** You fall off the west end onto tilted dirt. Wirtual
   descends, U-turns at the bottom, climbs back north, U-turns again at the top,
   and only then charges east — **nine seconds, of which about seven net zero
   displacement.** He is not being slow: the second descent is where his run-up
   speed comes from, 58 → 155 km/h in 1.6 s.
3. **The launch and the flight (13 – 23 s).** Through a big turbo gate at
   155 km/h, off a ramp, a long dive to 268 km/h, land on the grass, U-turn,
   come back west past a no-engine gate, and coast into the flag. Wirtual
   crosses the line at 8.5 km/h.

Our 21.022 drives Wirtual's route and wins in the last six seconds — mostly by
**not braking into the finish**. He is on the brake from 20.0 s and takes 0.829
for the final ten metres; carrying speed instead takes 0.258.

## The dirt ice flick — the pit's nine seconds are not forced

Everything above is the *route*, and the route really is forced (see "Two things
that are genuinely closed"). But **being forced to drive somewhere is not the
same as being forced to take nine seconds doing it**, and that is where this
map has been hiding.

The car can be **flicked** across the first dirt turn instead of looping around
it. Measured against three checkpoints along the eastward deck, required in
order so nothing can score by falling past them:

| | first deck marker | second | third (x = 767) |
|---|---|---|---|
| Wirtual | 12.954 | 13.297 | 13.560 |
| our previous best line | 12.254 | 12.652 | 12.933 |
| **the flick** | **6.119** | **6.848** | **7.283** |

**5.650 seconds earlier to the same point**, and it is not a fluke: 415 of
24,300 candidate runs arrive there in order.

### Why it is a flick and not a bug slide

A Trackmania player who saw our first write-up pointed out that you cannot bug
slide here, because the car is on **icy wheels**. He is right, and the map's own
recording carries per-wheel surface state, so it can be shown:

| race time | ice, four wheels | dirt | where the car is |
|---|---|---|---|
| 2.0 s | **1.00 — saturated** | 0.00 | on the ice run |
| 4.6 s | 0.46 / 0.56 / 0.44 / 0.56 | 0.000 | airborne off the west end |
| **4.7 s — the contact** | **0.44 / 0.54 / 0.43 / 0.55** | **0.008** | **the flick** |
| 6.5 s | 0.01 – 0.09 | 0.11 – 0.16 | pit floor |
| 14.0 s | 0.00 | **1.00** | the deck |

**Ice on the wheels, dirt underneath.** One thing falls out of that table that we
have not seen written down anywhere: **ice is a decaying clock, not a state.** It
saturates by 2.0 s and bleeds away to nothing by 6.5 s, roughly 0.2 per second.
So how icy your wheels are when you hit the dirt depends on *when* you hit it.

### The gain is route, not grip

It would be a nicer story to say the flick keeps your speed through a corner
that should scrub it. **It does not.** Speed carried through the contact, read
from engine memory across our 200 best candidates:

| | before contact | after (5.3 s) | lost |
|---|---|---|---|
| **Wirtual** | 143.6 | **77.9** | 65.7 |
| **best of our 200** | 147.6 | **79.7** | 67.8 |
| our median | ~146 | ~55 | ~91 |

The best flick we have keeps **1.8 km/h more** than Wirtual keeps through his
own contact. He scrubs just as hard as we do. The five and a half seconds come
entirely from **not driving the two loops** — it is a shortcut, not a grip
trick.

## Where it stops

Skipping the loops also skips the drop that pays for them, so we arrive at the
launch **50 km/h short**. Wirtual converts 78 → 175 km/h in 6.7 s using two
loops and a 15 m descent; the flick converts 78 → 119 in 2.3 s with 10 m to work
with. Two ways out, both measured shut:

**The lower platforms.** There are two dirt platforms 16 m below the deck that
**no run in this project had ever touched**, and the fast flick line runs
straight over them at **157.8 km/h** — 5 km/h off Wirtual's speed, 5.5 s
earlier. The energy is there. What is missing is a way back up: those platforms
end at x ≈ 784 and the next drivable surface east is
`OpenTechRoadSlope2FCLeft` at x ≈ 816, so the return is a **32 m gap that has to
be crossed while climbing 7 m — departing from a surface rolled 30° downward.**
At 157.8 km/h the crossing takes 0.73 s and gravity alone costs 6.4 m; you
arrive about 13 m low. **Zero of roughly 135,000 evaluations reach the far
side.** (A marker 16 m further east, built the same way in the same batch, fires
normally — so the test can succeed; this route just never gets there.)

**Under the flag instead of over it.** The fast line crosses the flag's exact
height at z = 653 against the flag's z = 656 — dead on in two coordinates — but
at **x = 826 against the flag's x = 990**. Closing 164 m at that height needs
about **416 km/h**. The fastest anything has ever gone on this map, in any run
we or anyone else has produced, is 294.7.

## Two things that *are* genuinely closed

**You cannot leave the start going east.** There is a large hole in the first
screen east of the spawn, and an ice tile pointing at it, and it would be worth
about ten seconds. It is unreachable: the spawn turbo fires you west and the
first 0.45 s is input-independent, so 3,840 turn-around programs and a detector
wall two metres from the spawn produce nothing.

**You cannot finish out of the air.** We re-measured the second screen by
sliding it across a run that crosses it exactly once (1,685 real validations)
and mapped where it is solid. The only opening a launch can reach puts the car
*past* the flag — which is why every run on this map, ours and Wirtual's, lands
on the grass, U-turns, and comes back.

## What is left

We can estimate what a perfect run-up would be worth — **an estimate, not a
bound.** A 2.5 s run-up returning Wirtual's conversion rate puts the deck arrival
near 9.8 s at 175 km/h, and our existing ending from there is 8.09 s: **about
17.9.** Three seconds better than our published run, and still over the author
time.

So the open question is a single number:

> **A deck arrival at ≈ 8.8 s carrying ≈ 175 km/h.**

Get that and the author time is in range. Every other part of this lap has now
been measured.

## Files

The 21.022 tape the clip is shot from is not committed here. What is in
`replays/`:

| file | what |
|---|---|
| `replays/TAS_21918_analog.Ghost.Gbx` | the fastest tape on this page — a **different run** from the 21.022, 0.896 slower |
| `replays/TAS_22290_thinned.Ghost.Gbx` | the same line at 84 input changes |
| `replays/TAS_22698_lowinput.Ghost.Gbx` | **ten steering values — the one worth studying** |
| `inputs/m267460_TAS_lowinput_76inputs.script.txt` | the low-input run as a readable script |
| `inputs/m267460_TAS_thinned_82inputs.script.txt` | the thinned run |

TMX map [267460](https://trackmania.exchange/maps/267460).

**Not submitted to any Nadeo leaderboard, and it never will be.**

### What these recordings are

Every file here carries **its own** telemetry. Each sample's position,
orientation, speed and velocity direction was read out of the dedicated
server's engine while it drove that file's own input tape, and its steer / gas /
brake bytes come from the tape itself — so opening one as a ghost in game
replays *this* run, and the two channels of inputs a ghost carries agree
exactly. The regenerator's tick alignment on this map was checked against a
recording the game made itself: regenerating that download reproduces it to
0.0005 m, as the mode of five runs, so these records sit on the game's own
physics tick.

Nine of the 116 bytes in each sample are ours and 91 are still the donor
container's — rpm, gear, wheel rotation, suspension and the surface effects,
byte 89 (the ground-contact flag) among them. The car's motion is this run's;
some of the dressing around it is not.

### One check refuses these three files, and the engine clears them

The publish gate's near-copy test (C12) refuses all three: after each tape parts
from Wirtual's world record — at 2.180 s, 3.970 s and 4.100 s — the recorded
trajectory stays **0.000633 m** from his for the remaining 375 samples. Two runs
that stay six tenths of a millimetre apart for eighteen seconds after their
inputs diverge is the shape of a copy, and that check exists because bit-exact
tests cannot see one.

It is not a copy here, and the thing that settles it is the engine rather than
another comparison. Each file's record was rewritten from the dedicated server's
own engine driving **that file's own input tape**, and every sample came back
identical to what the file already carried, to 0.000000 m. The server simulates
one ghost at a time, so there is no second car in the process for a locate to
have found: the engine, given our inputs, is what puts the car within a
millimetre of his line. On a Trial map an input that differs while the car is
wedged against geometry changes nothing, and these tapes differ mostly in
exactly that regime.

So the refusal is honest and the file is too. It stays on the record as a note
rather than being tuned away, because the same measurement on a map without a
wedged car would mean the opposite.
