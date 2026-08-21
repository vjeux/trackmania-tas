# unluckE - get jiggy with it

**Take the world record's own run and change one thing: fire the last flick to
full right about 0.1 s earlier and hold it at ~80 % instead of 100 %. That
validates at 6.342 — one millisecond under the author time.**

**unluckE - get jiggy with it** — TAS **6.342** (−0.001) | AT 6.343 | WR 6.346 by xeap-.-

https://github.com/user-attachments/assets/eb055e0b-f715-4bd9-b12e-ed2de3f26b65

**Thirteen inputs. Nothing held for less than 80 ms. Three steering values, gas
never released, brake never touched — and it beats the author time by a
millisecond and the world record by four.** Ours is the magenta car; xeap-.-'s
record is the one the camera follows. The two runs share their first 1.35 s
exactly — the launch is deterministic, so any two runs that hold the same opening
are bit-identical through it — and then part company for good.

The point of this tape is that **a person could press it.** The nine human
keyboard runs on this board have minimum holds of 30–90 ms; this lap never goes
below 80 ms, which makes it stricter than every human on the leaderboard while
finishing ahead of all of them.

**It is playable, which is not the same as forgiving.** Jitter every input by
±10 ms and it dies: 0 of 120 trials survive. The world record fails the same test
identically, 0 of 120. That control is published beside the number because the
number means nothing without it.

**Video: previously withdrawn, now replaced.** The clip published here before was
filmed from a copy of our ghost that is not the file this page ships — a stale
staging copy, superseded before the clip was shot. The run and the time were
unaffected. The clip above is filmed from the current tape, in a container
verified as ours.
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

## A version a person can actually press

> **A note on the 6.360 you may see in these files.** Every tape in this
> lineage declares **6360** in its header — including the 6.323 and the 6.322
> above — because that is the time of `r03`, the human keyboard run the whole
> family was searched out of, and its container has been carried down ever
> since. The oracle returns 6.322, 6.323, 6.338, 6.342 and 6.345 from those same
> five containers, so the header is a fact about the seed and the simulation is
> the fact about the run. Confirmed by transplant: these inputs written into a
> different human's container, declaring 6346, still simulate to **6.342**,
> with a lossless control and a negative control in the same batch, on two
> independently built binaries.
>
> **Never read a synthesised tape's time off its header, filename or manifest.
> Re-simulate it.**

The tape above is a machine's: it holds inputs for 10 ms and stabs three of them
for a single tick. Nobody can do that. So the same line was **searched again from
scratch under a minimum-hold constraint** — never quantised afterwards, which
does not work here and is worth saying why: at a floor of only 20 ms, **all 112
minimal repairs of the 6.323 keyboard tape fail to finish.** Those single-tick
events are load-bearing. A machine tape cannot be smoothed into a human one; it
has to be re-searched under the constraint.

The result is that the human floor is nearly free:

| minimum hold and gap | best time | vs author time 6.343 | inputs |
|---|---|---|---|
| 10 ms — the machine tape | 6.323 | −0.020 | 21 |
| **50 ms** | **6.338** | −0.005 | **11** |
| **80 ms** | **6.342** | **−0.001** | **13** |
| 120 ms | 6.345 | +0.002 | **10** |

**Going from a 10 ms grain to an 80 ms grain costs 19 ms and still beats the
author time.** At a 120 ms floor — ten presses, nothing held under an eighth of a
second — the run lands 1 ms under the human world record. For scale, the nine
human keyboard runs on this board have minimum holds of 30–90 ms, so **80 ms is
stricter than every human here.** (The two 10 ms holds in the field are not
presses at all: they are the rollover between opposite arrow keys.)

Every rung is the same line as the machine's — right at 1.45, the short right
stab at ~2.7, a second right at ~3.1, the long left hold from ~3.4, the flick to
the line — with the 2.25–3.59 flutter replaced by three or four ordinary presses.

`play_TAPE_80ms_6342.Ghost.Gbx` is the 13-input version: gas held throughout,
brake never touched, three steering values.

### But do not read "playable" as "forgiving"

**This map tolerates almost no mistiming, and that is true of the world record
too.** Jitter every input independently by ±10 ms and nothing survives — not our
tape (0 of 120 trials), and **not the human world record (0 of 120)**. Two other
human runs manage 1 of 120. Those are positive controls, not excuses: they are
published here because a number like ours means nothing without them.

Per input, the picture is sharper. **Every tape on this map — ours and all nine
human keyboard runs — has a zero-millisecond window on every input between about
1.4 s and 3.5 s**, and ±20–30 ms on everything after 5 s. So a survival
*percentage* mostly counts how many presses sit in the free endgame. A robustness
search here duly "improved" one tape from 9.1 % to 45.8 % by adding ten events
after 4.98 s while leaving all ten knife-edge inputs untouched. **Report the worst
window and where the zero-window inputs are, never the mean.**

The honest axis is **recoverable** tolerance — mistime one press, then re-time the
later ones to compensate. Our 6.342 saves 10 of 26 perturbations at ±10 ms, four
of them inside the hard window; the human r10 saves 11 of 26, about eight of them
inside it. **The human line recovers roughly twice as often where it matters.**
That is a property of the route, it is measurable, and it is the thing worth
improving next.


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

`replays/BEST_KEYBOARD_6323.Ghost.Gbx`. **Accelerate is held from the countdown
to the line and never released.** There is exactly one brake input in the whole
race, and everything else is steering.

*Provenance of the numbers below, now settled:* they are measured off the input
tape `kb6323.csv`, and that tape is **793 of 793 rows identical to
`replays/BEST_KEYBOARD_6323.Ghost.Gbx`'s own input archive** (md5
`40472ddf8733aeaa9ec9a9a5322be21a`), 10 ms per row. The check discriminates: the
other candidate, `KEYBOARD_23ev_6323.Ghost.Gbx`, matches only 679 of 793 and
first disagrees at race −1.460. An earlier version of this page attributed the
table to a `KEYBOARD_6323.Ghost.Gbx` with a different md5 — no such file exists
in this repo, and the table describes the 47-input tape above, not the
23-event one.

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

## An hour spent deciding whether this tape is a 6.342 or a 6.360

It is a **6.342**. The episode is recorded because the wrong answer was reached
by three independent readers agreeing, which is normally the strongest evidence
there is.

Every tape in this lineage declares **6360** in its header. That is the time of
`r03`, the human keyboard run the whole family was searched out of, and the
container has been inherited ever since. Five different tapes declare it while
simulating to 6.322, 6.323, 6.338, 6.342 and 6.345 — **a stored field cannot give
five answers**, so the header describes the seed and the simulation describes the
run.

What went wrong: a report inverted the server's two columns — writing
*"declares 6342, validates to 6360"* when the file says the reverse — and once
that was in the record, three further readings were fitted to it. The three
readers (`inputcount --meta`, a ghost checker, and a byte census) all agreed on
`6360`, and all three were reading **the stored field**. The census's `6360 ×6`
*is* the declared-time sites, by construction. **Three readers of one field are
one reader.**

And *"the value 6342 appears nowhere in the file"* — which read as damning — is
exactly what a correct file looks like. The simulated time is not stored anywhere.

The disambiguator needed no field-order judgement at all. The server writes it in
a sentence:

```
"Desc" : "validated time is actually better! (6360 > 6342)"
```

Three rules came out of it, and the third is the one that generalises furthest:

> **Never read a synthesised tape's time off its header, its filename, or its
> manifest. Re-simulate the file you are holding.**

> **`DeclaredResult` and `ValidatedResult` must never be quoted without their
> labels.** A mislabelled operand is worse than an invented one, because an
> invented number gets checked and a mislabelled one gets corroborated.

> **Agreement between readers of the same field is not independent evidence.**
> Before treating three sources as three, ask what each one read.
