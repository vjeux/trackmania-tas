# unluckE - get jiggy with it

**Take the world record's own run and change one thing: fire the last flick to
full right about 0.1 s earlier and hold it at ~80 % instead of 100 %. That
validates at 6.342 — one millisecond under the author time.**

**Video: withdrawn.** The clip published here was filmed from a copy of our ghost
that is not the file this page ships — a stale staging copy, superseded before the
clip was shot. The run and the time are unaffected. A replacement will be filmed
from a regenerated ghost, whose telemetry is verified against a re-simulation of
its own inputs.
against the human world record **6.346** (xeap-.-).

*An earlier clip on this page was withdrawn twice over, and both corrections are
worth keeping.* The first version used `HUMANWR_plus_early_flick_6342` as the
opponent — which is **not a human recording**: it is the world record's tape
with one input changed, a run of ours. It belongs in the table below, not in a
clip captioned "against the human world record", and the caption also gave the
human's time as 6.342 when the real record is 6.346.

The second correction was ours to make against ourselves. This page then claimed
that every recording of this map, **including Nadeo's own download**, carried
dead wheel telemetry — "zero usable rolling steps over a 495 m path". That was
false, and the fault was in our checker: it only measured samples classed
`Supported`, and the car on this map descends for its entire run, so it had
nothing to measure and reported the blindness as a defect. Measured directly,
the world record's ghost carries **122 distinct wheel values across 127 samples**.
Nothing was ever missing.

The rule that came out of it, now enforced in the render gate: **absence of
signal is not evidence of correctness — and it is not evidence of a fault
either.** A check that cannot see tells you nothing in either direction.

The runs and the times are unaffected: 6.322, 6.323 and 6.342 all validate on
the untouched map.

**unluckE - get jiggy with it** — TAS **6.330** (−0.013) | AT 6.343 | WR 6.346 by xeap-.-

https://github.com/user-attachments/assets/ae6e57c3-4f44-4bca-881f-361f573b1571

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

**[Drive it yourself in the browser](../trainer/) — the falling-note trainer is
built from this tape.** Notes fall, you hold the keys, and it judges you in
milliseconds. Nothing below is transcribed by hand; the trainer re-derives all
of it from `kb6323.csv` when you open it.

`replays/KEYBOARD_23ev_6323.Ghost.Gbx`. **Accelerate is held from the countdown
to the line and never released.** There is exactly one brake input in the whole
race, and everything else is steering.

*Provenance of the numbers below:* they are measured off the input tape
`kb6323.csv`, extracted from `KEYBOARD_6323.Ghost.Gbx`
(md5 `25e04568c299eccdb867c107f40ed650`) — 793 rows at 10 ms. That tape agrees
with every timing this page previously listed, so it is the same run; but it is
a *different file* from the shipped `KEYBOARD_23ev_6323.Ghost.Gbx`, and no one
has yet decoded the shipped ghost's inputs to confirm the two are identical
byte for byte.

**The brake tap is a brake-turn.** 0.750 → 0.880 — 0.130 s — taken at *full
left with the gas still down*. It is not a slowdown and you never lift; it
rotates the car. An earlier version of this page said the brake is never
touched, which was the input-reduction's steering-only count leaking into the
prose: the reduction to "23 events" counts **steer segments**, and the brake tap
is not one of them.

| # | when | input | the cue |
|---|---|---|---|
| 1 | before the lights | **hold LEFT** | you are aimed at a near-vertical drop — hold left through the launch and all the way down the face |
| — | 0.750, for 0.130 | **BRAKE, still at full left, gas still down** | the brake-turn — the only brake input in the run |
| 2 | 1.45 s (~110 km/h, the wall runs out and the car goes light) | **RIGHT**, hold 0.520 | you are now falling |
| 3 | 1.97 s | release to **centre** | |
| 4 | 2.09 s | **LEFT** into the landing | the landing is violent — the car is thrown along the road |
| 5–9 | 2.25–2.51 s | **five short LEFT/centre taps** (50, 30, 30, 10, 10 ms) | the keyboard's way of holding a partial steer through the landing: pulse, don't hold |
| 10 | 2.51 s | **centre**, 0.200 | |
| 11 | 2.71 s | **RIGHT**, 0.080 | the flick — a short stab |
| 12 | 2.79 s | **LEFT**, 0.320 | |
| 13–14 | 3.11 s | brief **centre**, then **RIGHT** 0.210 | you are near the low point of the map |
| 15–20 | 3.34–3.59 s | **LEFT with two short releases** (20 ms each, at 3.48 and 3.57 s) | the surface turns up under you — this is the kicker |
| 21 | 3.59 s | **hold LEFT for 1.520** | through the kicker and the first half of the climb; the car goes inverted and the thrust takes over |
| 22 | **5.230** | **RIGHT, and hold it to the line** | the aim into the gate — **aim high** |

Measured over the race window 0.000 → 6.323: **left 3.820 s · right 1.910 s ·
centre 0.600 s · brake 0.130 s.**

### The burst is not eighteen inputs

The stretch from 2.090 to 3.590 reads as eighteen steer segments, and read that
way it is unlearnable. It is not eighteen things. **It is left held with the
finger twitching off it.**

From 2.250 the centre gaps run **50 → 30 → 10 ms**, with left presses of
**30 → 10 ms** between them: a converging flutter, like a ball settling, not a
rhythm. The only two rights in the whole burst — 0.080 at 2.710 and 0.210 at
3.130 — bracket a 0.320 left. Then the second cluster stutters the same way,
centre gaps of **20 / 10 / 20 / 20 ms** between left presses of 0.130 and 0.070.

**The shape to hold in your head is four things, not eighteen:**

> **flutter, blip right, long left, blip right, flutter, commit.**

The commit is the 1.520 s of left from 3.590 — the only place in the run where
you can breathe — and then right at 5.230 to the line.

Three events in the run last a single tick: **left 10 ms at 2.360, centre 10 ms
at 2.370, centre 10 ms at 3.340.** At full speed nobody hits those; in the
trainer at 0.15× they are 67 real milliseconds, which is the only honest way to
learn them.

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
  where the attempts die, and expect to learn it by feel. **Read it as a
  flutter, not as a list** — see the section above — and practise it on its own
  in [the trainer](../trainer/), which has that stretch as a section and will
  run it at a sixth speed.

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
