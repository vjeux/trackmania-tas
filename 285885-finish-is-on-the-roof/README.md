# finish is on the roof to your right — the author time did not fall, and here is exactly what stands in the way

**Author time 43.079 · human world record 61.229 · best validated run 50.229 —
11.000 s faster than any human, 7.150 s short of the author time.**

| tape | validated | vs human WR | vs AT |
|---|---|---|---|
| [`TAS_50229`](replays/TAS_50229.Ghost.Gbx) | **50.229** | **−11.000** | +7.150 |
| [`TRIGGERPOKE_50469`](replays/TRIGGERPOKE_50469.Ghost.Gbx) | 50.469 | −10.760 | +7.390 |
| [`POKE_1input_50659`](replays/POKE_1input_50659.Ghost.Gbx) | 50.659 | −10.570 | +7.580 |
| author time | 43.079 | — | — |
| human WR *(control)* | 61.229 | — | +18.150 |

TMX map [285885](https://trackmania.exchange/maps/285885) · **3 recorded runs**,
all three re-simulate exactly · field reproduction **3/3**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

This map is published as a **negative** — but a precise one. The 18-second gap
turns out not to be what it looks like, and the thing blocking a sub-author-time
run is measured to the millimetre.

---

## The map's joke, and it is a good one

**The finish gate is an item sunk about a metre below the roof the car sits on.**
The trigger fires only when the car's *origin* drops to or below
`gate_y + 1.25 m` — so a car resting on the roof is above it, and driving over
the finish does nothing at all.

**All three humans drive over it repeatedly without finishing.** The world
record's lowest point inside the gate's horizontal footprint is

```
y = 145.2506      against a ceiling of      y = 145.2500
```

**It misses by under five millimetres.** Rank 2 misses by ~15 mm and rank 3 by
~20 mm. That near-miss costs rank 1 **10.6 s** and rank 2 **37.2 s** of driving
past, turning round and coming back.

Proof by map surgery — raise the gate 1 m and all three finish about ten seconds
earlier, with the control at the original position reproducing the untouched map
exactly:

| gate y | rank 1 | rank 2 | rank 3 |
|---|---|---|---|
| 144.000 (true) | 61.229 | 88.209 | 97.769 |
| **144.005** | **51.059** | — | — |
| 144.150+ | 50.639 | 51.009 | 56.309 |

**Five millimetres of gate height is worth 10.17 seconds.**

## So the real gap is ~7.4 s, not 18.2 s

And the 18.150 s figure was misleading twice over: the "human world record" is
the author's own alt account, 18.150 s slower than the author time they
themselves set.

## What a human should take from this

**One steering input at the right moment is worth 10.5 seconds.** Applied to the
world record's own tape, a single change at race ~50.9 s takes 61.229 → 50.659.

And it is **extremely forgiving**. Of twenty-three held values tried at that
instant, **sixteen fire the gate ten seconds early — including neutral, i.e.
doing nothing at all.** Only hard left (≤ −30) loses the run. The timing window
is ~550 ms and hold durations from 0.08 to 0.5 s all work.

So the instruction is not a frame-perfect input. It is:

> **Arrive, and stop turning right.**

One of the few values that does *not* fire the gate is exactly what the world
record does.

*(Precision note: the published poke tape is a rectangular override of the
steering channel over 25 ticks, not a single key press. The durable claim is the
tolerance result above, not that tape's literal contents.)*

## Why the author time still stands: 70 millimetres

A search seeded from the world record found a genuinely faster route — full
throttle up the roofs at 150–190 km/h where the field lifts to 30–100 —
**reaching the finish patch at ~41.04 s, 9.6 s earlier than any human.** The
first 35 s is the world record's line within a metre; everything after the
landing is new. **That would be ~2 s under the author time.**

It misses the trigger by **70 mm of height** — established by 1 mm bisection
(clears at gate y 144.070, refuses at 144.069) — or equivalently **0.42 m along
the (+x, −z) diagonal**.

What has been thrown at that 70 mm:

- ~50,000 designed tapes: steer and pedal overrides, pulses, neutral holds,
  single-tick nudges across ticks 3000–4170
- ~1.5 M shaped search evaluations
- a **height ladder**: a single 10 mm step below the clearance, seeded from the
  fast route, 168 workers — **50,430 evaluations, zero finishers**
- a five-rung **lateral ladder** that stalls exactly on the measured footprint
  boundary

The crossing geometry of this route is **locked**: fixed clearance, fixed ~37 ms
inside the footprint, immune to local mutation. The last 70 mm is not a gradient
that can be walked down.

## The one idea nobody has run

Stop trying to arrive *lower*. **Arrive along the footprint diagonal and buy
time inside it instead** — a different currency, and one the clearance
trade-curve says is available.

That needs a search whose objective is **time-in-footprint with arrival as a
constraint**: a different climb line, not a perturbation of this one. Every
ladder built here scores arrival *at* a station; this one has to score *duration
between two stations*.

## Where the remaining time is, if someone takes it further

An arrival-time ladder — 16 stations along the route, with a control that aborts
unless returning the gate to its origin reproduces the untouched map — gives:

| station | rank1 | rank2 | rank3 | our route |
|---|---|---|---|---|
| 0 — landing (343, 106, 1835) | **36.049** | – | – | 36.049 |
| 4 — hairpin apex (301, 123, 1763) | **40.074** | – | – | 40.074 |
| 8 — (343, 132, 1741) | **44.789** | – | 48.488 | 44.789 |
| 13 — (409, 144, 1710) | 49.569 | 50.809 | 55.189 | 49.549 |
| 14 — (415, 145, 1705) | 50.579 | 51.009 | 56.299 | 50.439 |
| 15 — the real gate | 61.229 | 88.209 | 97.769 | 50.509 |

**The hairpin is the entire deficit, and it is larger than the deficit.** The
record lands at station 0 at 36.049, drives *away* from the finish — west and
downhill — to the apex at 40.074, turns round, and does not regain the same x
(25 m higher, station 8) until 44.789. **That U-turn costs 8.74 s. The map needs
7.4 s.**

So this is a **landing problem, not a crawl problem**: land on station 5 at
~36 s instead of 41.709 and it is worth 5.7 s; land on station 8 and it is worth
8.7 s — the whole map — without touching the rooftop crawl at all.

**And rank 2 is doing something nobody has looked at.** It never enters stations
0–10, reaches station 13 at 50.809 and station 14 at 51.009 — within 0.4 s of the
world record — and only passes stations 11 and 12 at 70.289 / 71.338, twenty
seconds later, coming back down. It reaches the patch by a completely different
approach, almost as fast, while being 27 s slower overall. **If that approach
skips the hairpin, it may already be most of the answer.**

## Two instrument findings from this map that outlived it

**A gate probe is only an instrument if it can return to its origin.** The
standard gate mover swaps the item model before moving it, changing the trigger
volume — on this map it *enlarges* it, so everything fires early including the
null case. Caught because putting the gate back at its original position
returned 50.589 instead of the true 61.229. A broken search fails to find
something; a broken gate probe **fabricates a discovery**.

**A synthesised tape carries its template's telemetry, not its own.** Decoding a
candidate here returned byte-identical data to its seed — reporting the world
record parked at a hairpin 120 m from where the candidate actually was. An agent
read a climb rate of +9.2 m/s off it when the truth was a *descent* of 1.1 m/s,
and nearly redirected the search on that basis. Both findings are written up in
this repo's [`FINDINGS.md`](../FINDINGS.md).

## Validation

Field reproduction 3/3 exact (61.229 / 88.209 / 97.769). Encoder identity
control passes. Every number here came from the plain oracle on the untouched
map with a human ghost as a known-answer control in the same batch. **No
phantoms**; one wrong model published during the work (a "floor" on the trigger)
was retracted within minutes of being measured against.

There is **no author ghost** in this map file — verified on the LZO-decompressed
body with another map's author lap decoding in the same tool as a positive
control.

## Files

| file | what |
|---|---|
| `replays/TAS_50229.Ghost.Gbx` | the fastest validated run — 11.0 s faster than any human |
| `replays/POKE_1input_50659.Ghost.Gbx` | the world record with one steering change: 61.229 → 50.659 |
| `replays/TRIGGERPOKE_50469.Ghost.Gbx` | the same idea, searched |
| `notes/TRIGGER.md` | the sunken gate, the 5 mm human miss, the tolerance sweep |
| `notes/LADDER.md` | the arrival-time ladder and the hairpin finding |
| `notes/CLEARANCE.md` | the 70 mm / 0.42 m clearance and its trade curve |
| `notes/FINDINGS.md` | the route half: search, negatives, telemetry trap |
