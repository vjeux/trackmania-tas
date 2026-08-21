# unluckE - get jiggy with it

**Take the world record's own run and change one thing: fire the last flick to
full right about 0.1 s earlier and hold it at ~80 % instead of 100 %. That
validates at 6.342 — one millisecond under the author time.**

**Video: withdrawn.** The side-by-side clip published here showed a second car that
clips through the ground and rotates wrongly. The file used as the opponent was
`HUMANWR_plus_early_flick_6342` — which is **not a human recording**: it is the
world record's tape with one input changed, i.e. a run of ours, and it carries
its carrier's telemetry rather than its own. A replacement will be filmed against
a genuine downloaded recording, or this page will carry a single-car clip.

| run | time | vs author time | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, unconstrained** | **6.322** | **−0.021** | −0.024 | analog, 186 events |
| **TAS, pure keyboard** | **6.323** | **−0.020** | −0.023 | 23 changes, 3 values |
| **Human WR + ONE changed input** | **6.342** | **−0.001** | −0.004 | the WR's own tape |
| Author time | 6.343 | — | −0.003 | — |
| Human WR — xeap-.- | 6.346 | +0.003 | — | pad |

TMX map [145875](https://trackmania.exchange/maps/145875) · author **InfTM** ·
46 recorded runs.

## The cheapest advice on this map

The one-input change above is not a knife edge. Enumerating that single input
over timing and stick position gives a **6.342 plateau spanning 50 ms of timing
and most of the top half of the stick**:

```
  fire\hold      90     100     110     118     124     127
       -10    6.342   6.342   6.343   6.382   6.356
        -9    6.343   6.342   6.342   6.343   6.348
        -8    6.343   6.342   6.342   6.342   6.342    <- a broad 6.342 plateau
        -7    6.344   6.343   6.342   6.342   6.342
        -6    6.345   6.343   6.343   6.343   6.342
        +0    6.348   6.347   6.347   6.347   6.346    <- what the WR does now
```

This is the single most forgiving input on the map, and nobody in 46 runs has
aimed at it.

## Where the time is: the finish trigger is tilted

The fast tape and the world record are the same line, within a metre, for 96 %
of the lap. They are exactly level at every plane through the fall and the
landing, and the split only opens in the last 1.5 s of the climb — and even at
the last plane before the gate it is only −0.005. The rest of the margin is
geometric.

**Arriving higher trips the finish earlier.** Across the field, each extra metre
of height at the gate buys roughly a metre of x:

| run | x at finish | y at finish | km/h |
|---|---|---|---|
| **TAS** | **1228.78** | **154.62** | 612 |
| r10 6.440 | 1229.93 | 154.72 | 599 |
| r08 6.413 | 1229.92 | 153.39 | 600 |
| r01 6.346 (WR) | 1230.50 | 153.62 | 609 |
| r15 6.478 | 1231.34 | 152.51 | 600 |

This run is 1.0 m higher and 4 km/h faster at the line, so it trips the finish
1.7 m earlier in x. **The whole margin is one metre of height and 4 km/h at the
gate. Aim the last climb high.** The rest of the line is the field's own.

It matters that the last 2.9 s is a thrust phase, not a normal drive: all four
dampers are unloaded, and the car is pushed by a force of constant magnitude
fixed in its own frame while it rolls through most of a revolution. **In the
climb, steering does not steer the car — it points the thrust.** That is why an
attitude difference set down at the bottom kicker turns into a metre of height
at the gate.

## This map does not need a pad

**8 of the 13 fastest humans are already on a keyboard.** Measured off their
tapes: two of them move from full left to full right inside a single tick, so
the game does not ramp a held key, and `{−127, 0, +127}` is the real alphabet.

A keyboard reaches **6.323** — one millisecond off the unconstrained
floor, and 0.020 under the author time. The best human keyboard run is 0.037
slower than that.

## The run as inputs

`replays/KEYBOARD_23ev_6323.Ghost.Gbx`. **Accelerate is held from the countdown
to the line and the brake is never touched.** Everything below is steering.

| # | when | input | the cue |
|---|---|---|---|
| 1 | before the lights | **hold LEFT** | you are aimed at a near-vertical drop — hold left through the launch and all the way down the face |
| 2 | 1.45 s (~110 km/h, the wall runs out and the car goes light) | **RIGHT**, hold 0.5 s | you are now falling |
| 3 | 1.97 s | release to **centre** | |
| 4 | 2.09 s | **LEFT** into the landing | the landing is violent — the car is thrown along the road |
| 5–9 | 2.25–2.51 s | **five short LEFT/centre taps** (50, 30, 30, 10, 10 ms) | the keyboard's way of holding a partial steer through the landing: pulse, don't hold |
| 10 | 2.51 s | **centre**, 0.2 s | |
| 11 | 2.71 s | **RIGHT**, 80 ms | the flick — a short stab |
| 12 | 2.79 s | **LEFT**, 0.32 s | |
| 13–14 | 3.11 s | brief **centre**, then **RIGHT** 0.21 s | you are near the low point of the map |
| 15–20 | 3.34–3.59 s | **LEFT with two short releases** (20 ms each, at 3.48 and 3.57 s) | the surface turns up under you — this is the kicker |
| 21 | 3.59 s | **hold LEFT for 1.6 s** | through the kicker and the first half of the climb; the car goes inverted and the thrust takes over |
| 22 | **5.21 s** | **RIGHT, and hold it to the line** | the aim into the gate — **aim high** |

## How forgiving it is

- **Input 22 is generous**: ±3 to +6 ticks, up to 90 ms, with no change in time,
  and the equivalent input on the world record's own tape has a 50 ms plateau.
- **Inputs 1–4 and 21 are holds.** Start them off the visual cue and they are
  fine.
- **Inputs 5–20, the pulsing between 2.25 s and 3.59 s, are the hard part**:
  eleven short inputs in 1.3 s, several of them 10–30 ms long. Frozen and
  replayed they have no slack at all — but so do the world record's own tape and
  the best human keyboard run, measured the same way, and 46 people finish this
  map. It is drivable by reacting, not by counting. Expect that stretch to be
  where the attempts die, and expect to learn it by feel.

**If you only want the author time and not the record, practise the one-input
change.** It is the world record's run with the last flick a tenth of a second
earlier.

## Files

| file | what |
|---|---|
| `replays/HUMANWR_plus_early_flick_6342.Ghost.Gbx` | **the world record with one input changed** — beats the author time |
| `replays/KEYBOARD_23ev_6323.Ghost.Gbx` | **keyboard only, 23 inputs** — the one worth studying |
| `replays/BEST_KEYBOARD_6323.Ghost.Gbx` | the same time on 47 inputs, before event reduction |
| `replays/BEST_6322.Ghost.Gbx` | fastest, unconstrained |
| `replays/tas_6330.Ghost.Gbx` | the analog family, 6.330 |
